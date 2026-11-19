//! Owned generator activations for plain yield statements.
//! https://tc39.es/ecma262/#sec-generatorfunction-definitions
//! https://tc39.es/ecma262/#sec-generator-objects
use super::function_reachability::Edge;
use super::async_plan::{GeneratorCallDestination as Destination, GeneratorCallYield, Instruction, Resume};
use super::iterator_protocol::{Delegated, Record};
use super::*;

#[derive(Clone, Copy)]
pub(super) enum Op {
    Next,
    Return,
    Throw,
}

pub(super) enum State {
    SuspendedStart(Box<Activation>),
    SuspendedYield(Box<Activation>),
    Executing,
    Completed,
}

pub(super) struct Activation {
    program: Rc<Program>,
    template: usize,
    callee: usize,
    environment: usize,
    next: usize,
    expression_operand: Option<LogicalOperand>,
    delegate: Option<Record>,
    pending_call: Option<Box<PendingCall>>,
    scopes: [Option<usize>; 4],
    lexical_scopes: Vec<usize>,
    try_scopes: Vec<TryScope>,
    this: Value,
    invocation: Option<InvocationContext>,
    realm: Option<usize>,
    native_caller: Option<Option<u32>>,
}

struct TryScope {
    statement: StmtId,
    lexical_depth: usize,
    after: usize,
}

struct PendingCall {
    function: Value,
    direct_eval: bool,
    destination: Destination<Reference>,
}

impl PendingCall {
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.function.bytes())
            .saturating_add(self.destination.assignment().map_or(0, Reference::assignment_bytes))
    }
}

enum Progress {
    Yield(Value),
    Forwarded(Value),
    Complete(Value),
}

impl Activation {
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.this.bytes())
            .saturating_add(self.delegate.as_ref().map_or(0, Record::retained_bytes))
            .saturating_add(self.pending_call.as_deref().map_or(0, PendingCall::retained_bytes))
            .saturating_add(self.lexical_scopes.capacity().saturating_mul(std::mem::size_of::<usize>()))
            .saturating_add(self.try_scopes.capacity().saturating_mul(std::mem::size_of::<TryScope>()))
    }

    fn environment(&self) -> usize {
        self.lexical_scopes.last().copied().unwrap_or(self.environment)
    }

    fn reachability_edges(&self, add: &mut impl FnMut(Edge)) {
        add(Edge::Function(self.callee));
        for scope in self.scopes.into_iter().flatten() {
            add(Edge::Environment(scope));
        }
        for &scope in &self.lexical_scopes {
            add(Edge::Environment(scope));
        }
        Edge::value(&self.this, add);
        if let Some(delegate) = &self.delegate {
            delegate.reachability_edges(add);
        }
        if let Some(call) = &self.pending_call {
            Edge::value(&call.function, add);
            if let Some(reference) = call.destination.assignment() {
                Edge::reference(reference, add);
            }
        }
    }
}

impl State {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(Edge)) {
        if let Self::SuspendedStart(frame) | Self::SuspendedYield(frame) = self {
            frame.reachability_edges(add);
        }
    }

    pub(super) fn reachability_cost(&self) -> usize {
        match self {
            Self::SuspendedStart(frame) | Self::SuspendedYield(frame) => {
                6 + frame.lexical_scopes.len() + 2 * usize::from(frame.delegate.is_some())
                    + frame.pending_call.as_ref().map_or(0, |call| 1 + 2 * usize::from(call.destination.assignment().is_some()))
            }
            _ => 0,
        }
    }

    pub(super) fn environment_roots(&self, roots: &mut Vec<usize>) {
        if let Self::SuspendedStart(frame) | Self::SuspendedYield(frame) = self {
            roots.extend(frame.scopes.into_iter().flatten());
            roots.extend(frame.lexical_scopes.iter().copied());
            if let Some(Reference::Binding(environment, _)) = frame.pending_call.as_ref()
                .and_then(|call| call.destination.assignment())
            {
                roots.push(*environment);
            }
        }
    }
}

#[derive(Default)]
pub(super) struct Roots {
    function_prototype: Option<usize>,
    generator_prototype: Option<usize>,
    constructor: Option<usize>,
    failed: bool,
}

impl Roots {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(Edge)) {
        for prototype in [self.function_prototype, self.generator_prototype].into_iter().flatten() {
            add(Edge::Object(prototype));
        }
        if let Some(constructor) = self.constructor {
            add(Edge::Function(constructor));
        }
    }
}

impl Runtime {
    pub(super) fn generator_function_prototype(&mut self) -> Result<usize, Fault> {
        if self.generator_roots.failed {
            return Err(Fault::new("Generator intrinsics are unavailable after a bounded initialization failure."));
        }
        if let Some(prototype) = self.generator_roots.function_prototype {
            return Ok(prototype);
        }
        let marker = self.inhibit_transient_collection();
        let result = self.initialize_generator_intrinsics();
        if result.is_err() {
            self.generator_roots.failed = true;
        }
        self.restore_transient_marker(marker);
        result
    }

    fn initialize_generator_intrinsics(&mut self) -> Result<usize, Fault> {
        let function = self.async_function_roots.function_constructor
            .ok_or_else(|| Fault::new("Canonical Function constructor is unavailable."))?;
        let iterator = self.collection_roots.iterator_prototype
            .ok_or_else(|| Fault::new("Canonical iterator prototype is unavailable."))?;
        let tag = self.symbol_to_string_tag
            .ok_or_else(|| Fault::new("Generator tag intrinsic is unavailable."))?;
        let function_prototype = self.new_object(None)?;
        self.objects[function_prototype].prototype = self.function_prototype.map(Ordinary::Function);
        let generator_prototype = self.new_object(None)?;
        self.objects[generator_prototype].prototype = Some(Ordinary::Object(iterator));
        let Value::Function(constructor) = self.native_function(Builtin::GeneratorFunction, "GeneratorFunction")? else {
            unreachable!()
        };
        let properties = self.ordinary_id(Ordinary::Function(constructor))?;
        self.objects[properties].prototype = Some(Ordinary::Function(function));
        self.store_property(properties, "prototype".into(), Value::Object(function_prototype), Some((false, false, false)))?;
        self.store_property(function_prototype, "prototype".into(), Value::Object(generator_prototype), Some((false, false, true)))?;
        self.store_property(function_prototype, "constructor".into(), Value::Function(constructor), Some((false, false, true)))?;
        self.store_property(generator_prototype, "constructor".into(), Value::Object(function_prototype), Some((false, false, true)))?;
        for (prototype, name) in [(function_prototype, "GeneratorFunction"), (generator_prototype, "Generator")] {
            self.store_symbol_property(prototype, tag, Value::String(name.into()), Some((false, false, true)))?;
        }
        for (name, operation) in [("next", Op::Next), ("return", Op::Return), ("throw", Op::Throw)] {
            self.prototype_method(generator_prototype, name, Builtin::Generator(operation))?;
        }
        self.generator_roots.function_prototype = Some(function_prototype);
        self.generator_roots.generator_prototype = Some(generator_prototype);
        self.generator_roots.constructor = Some(constructor);
        Ok(function_prototype)
    }

    fn generator_prototype(&mut self) -> Result<Ordinary, Fault> {
        self.generator_function_prototype()?;
        Ok(Ordinary::Object(self.generator_roots.generator_prototype.expect("initialized generator prototype")))
    }

    pub(super) fn initialize_generator_function(&mut self, callee: usize) -> Result<(), Fault> {
        let parent = self.generator_prototype()?;
        let prototype = self.new_object(None)?;
        self.objects[prototype].prototype = Some(parent);
        let properties = self.ordinary_id(Ordinary::Function(callee))?;
        self.store_property(properties, "prototype".into(), Value::Object(prototype), Some((false, true, false)))
    }

    pub(super) fn dynamic_generator_function_prototype(
        &mut self, new_target: Option<usize>, host: &mut dyn Host,
    ) -> Result<Ordinary, Fault> {
        let target = match new_target {
            Some(target) => target,
            None => return self.generator_function_prototype().map(Ordinary::Object),
        };
        let prototype = self.get_ordinary_property_with_host(Ordinary::Function(target), "prototype", host)?;
        if let Some(prototype) = Ordinary::from_value(&prototype) {
            return Ok(prototype);
        }
        if matches!(prototype, Value::Host(_)) {
            return Err(Fault::new("Native host object prototypes are not implemented."));
        }
        let realm = self.function_realms.get(target).copied()
            .ok_or_else(|| Fault::new("Invalid GeneratorFunction new target."))?;
        self.with_intrinsic_realm(realm, |runtime| runtime.generator_function_prototype().map(Ordinary::Object))
    }

    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn start_generator(
        &mut self, program: &Rc<Program>, template: usize, callee: usize, parent: usize,
        args: &[Value], this: &Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let mut scopes = [None; 4];
        let marker = self.transient_marker();
        let inhibited = self.inhibit_transient_collection();
        let prepared = self.prepare_script_call(program, template, callee, parent, args, this, host, &mut scopes);
        self.finish_transient_inhibition(inhibited);
        let result = prepared.and_then(|environment| {
            let frame = Activation {
                program: Rc::clone(program), template, callee, environment, next: 0, delegate: None, scopes,
                pending_call: None, expression_operand: None,
                lexical_scopes: Vec::new(), try_scopes: Vec::new(),
                this: self.copy_value(this)?,
                invocation: self.invocation, realm: self.active_realm, native_caller: self.native_access_caller,
            };
            self.create_suspended_generator(frame, host)
        });
        self.restore_transient_marker(marker);
        if result.is_err() {
            self.release_generator_scopes(scopes);
        }
        result
    }

    fn create_suspended_generator(&mut self, frame: Activation, host: &mut dyn Host) -> Result<Value, Fault> {
        let prototype = self.get_ordinary_property_with_host(Ordinary::Function(frame.callee), "prototype", host)?;
        let prototype = match Ordinary::from_value(&prototype) {
            Some(prototype) => prototype,
            None if matches!(prototype, Value::Host(_)) => {
                return Err(Fault::new("Native host object prototypes are not implemented."));
            }
            None => self.generator_prototype()?,
        };
        let retained = frame.retained_bytes();
        self.check_generator_retention(retained)?;
        let id = self.new_object(None)?;
        self.objects[id].prototype = Some(prototype);
        self.objects[id].generator = Some(State::SuspendedStart(Box::new(frame)));
        self.value_bytes += retained;
        Ok(Value::Object(id))
    }

    fn check_generator_retention(&mut self, bytes: usize) -> Result<(), Fault> {
        if bytes > self.limits.heap.retained_values.saturating_sub(self.value_bytes) {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        self.work(bytes)
    }

    fn release_generator_scopes(&mut self, scopes: [Option<usize>; 4]) {
        for scope in scopes.into_iter().rev().flatten() {
            self.release_environment(scope);
        }
    }

    #[inline(never)]
    pub(super) fn generator_builtin(
        &mut self, operation: Op, args: &[Value], receiver: Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let Value::Object(id) = receiver else { return Err(Self::generator_receiver_error()) };
        let Some(state) = self.objects.get_mut(id).and_then(|object| object.generator.as_mut()) else {
            return Err(Self::generator_receiver_error());
        };
        match state {
            State::Executing => return Err(Fault::exception("TypeError", "Generator is already executing.")),
            State::Completed => return self.completed_generator(operation, args),
            State::SuspendedStart(_) | State::SuspendedYield(_) => {}
        }
        let (State::SuspendedStart(mut frame) | State::SuspendedYield(mut frame)) =
            std::mem::replace(state, State::Executing) else { unreachable!() };
        let input = args.first().unwrap_or(&Value::Undefined);
        let progress = if matches!(operation, Op::Next) || frame.delegate.is_some() || !frame.try_scopes.is_empty() {
            self.resume_generator(&mut frame, operation, input, host)
        } else {
            self.generator_abrupt_value(operation, input).map(Progress::Complete)
        };
        let value = match progress {
            Ok(Progress::Forwarded(result)) => {
                self.objects[id].generator = Some(State::SuspendedYield(frame));
                return Ok(result);
            }
            Ok(Progress::Yield(value)) => match self.generator_result(value, false) {
                Ok(result) => {
                    self.objects[id].generator = Some(State::SuspendedYield(frame));
                    return Ok(result);
                }
                Err(error) => Err(error),
            },
            Ok(Progress::Complete(value)) => Ok(value),
            Err(error) => Err(error),
        };
        self.objects[id].generator = Some(State::Completed);
        self.value_bytes -= frame.retained_bytes();
        self.unwind_generator_scopes(&mut frame, 0);
        self.release_generator_scopes(frame.scopes);
        self.generator_result(value?, true)
    }

    fn generator_receiver_error() -> Fault {
        Fault::exception("TypeError", "Generator method requires a generator receiver.")
    }

    fn completed_generator(&mut self, operation: Op, args: &[Value]) -> Result<Value, Fault> {
        let value = match operation {
            Op::Next => Value::Undefined,
            Op::Return | Op::Throw => self.generator_abrupt_value(operation, args.first().unwrap_or(&Value::Undefined))?,
        };
        self.generator_result(value, true)
    }

    fn generator_abrupt_value(&mut self, operation: Op, input: &Value) -> Result<Value, Fault> {
        let value = self.copy_value(input)?;
        if matches!(operation, Op::Throw) { Err(Fault::thrown(value)) } else { Ok(value) }
    }

    fn generator_result(&mut self, value: Value, done: bool) -> Result<Value, Fault> {
        let result = self.new_object(None)?;
        self.store_property(result, "value".into(), value, Some((true, true, true)))?;
        self.store_property(result, "done".into(), Value::Bool(done), Some((true, true, true)))?;
        Ok(Value::Object(result))
    }

    #[inline(never)]
    fn resume_generator(
        &mut self, frame: &mut Activation, operation: Op, input: &Value, host: &mut dyn Host,
    ) -> Result<Progress, Fault> {
        self.step()?;
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Function call limit reached (32)."));
        }
        self.with_intrinsic_realm(frame.realm, |runtime| {
            let marker = runtime.root_marker();
            let mut rooted = Ok(());
            frame.reachability_edges(&mut |edge| {
                if rooted.is_ok() {
                    rooted = runtime.push_transient_root(edge);
                }
            });
            let result = rooted.and_then(|()| runtime.run_generator_body(frame, operation, input, host));
            runtime.restore_root_marker(marker);
            result
        })
    }

    #[inline(never)]
    fn run_generator_body(
        &mut self, frame: &mut Activation, operation: Op, input: &Value, host: &mut dyn Host,
    ) -> Result<Progress, Fault> {
        let previous_invocation = self.invocation;
        let previous_native_caller = self.native_access_caller;
        let previous_strict = self.strict;
        self.invocation = frame.invocation;
        self.native_access_caller = frame.native_caller;
        self.strict = frame.program.templates[frame.template].strict;
        self.call_depth += 1;
        self.transient_roots.owned_call_depth += 1;
        let incoming = (|| {
            if frame.delegate.is_some() {
                return self.advance_generator_delegate(frame, operation, input, host);
            } else if !matches!(operation, Op::Next) {
                return self.generator_abrupt_value(operation, input).map(|value| Some(Progress::Complete(value)));
            } else if frame.pending_call.is_some() {
                let value = self.copy_value(input)?;
                self.finish_generator_argument(frame, value, host)?;
            }
            Ok(None)
        })();
        let result = self.resume_generator_instructions(frame, incoming, host);
        self.transient_roots.owned_call_depth -= 1;
        self.call_depth -= 1;
        self.invocation = previous_invocation;
        self.native_access_caller = previous_native_caller;
        self.strict = previous_strict;
        result
    }

    #[inline(never)]
    fn resume_generator_instructions(
        &mut self, frame: &mut Activation, incoming: Result<Option<Progress>, Fault>, host: &mut dyn Host,
    ) -> Result<Progress, Fault> {
        let mut progress = match incoming {
            Ok(Some(progress)) => Ok(progress),
            Ok(None) => self.drive_generator_body(frame, host),
            Err(error) => Err(error),
        };
        loop {
            let completion = match progress {
                Ok(Progress::Complete(value)) => Ok(Control::Return(value)),
                Err(error) => Err(error),
                suspended => return suspended,
            };
            if let Some(completed) = self.finish_generator_body(frame, completion, host)? {
                return Ok(completed);
            }
            progress = self.drive_generator_body(frame, host);
        }
    }

    fn drive_generator_body(&mut self, frame: &mut Activation, host: &mut dyn Host) -> Result<Progress, Fault> {
        let template = &frame.program.templates[frame.template];
        if template.resumption_plan.is_empty() {
            let control = self.statements(&frame.program, &template.body, frame.environment, &frame.this, host)?;
            return Self::generator_completion(control);
        }
        while let Some(&instruction) = frame.program.templates[frame.template].resumption_plan.get(frame.next) {
            frame.next += 1;
            let control = match instruction {
                Instruction::Statement(statement) => {
                    self.statement(&frame.program, statement, frame.environment(), &frame.this, host)?
                }
                Instruction::Declare { statement, index } => {
                    self.step()?;
                    self.evaluate_resumption_declarator(
                        &frame.program, statement, index, frame.environment(), &frame.this, host,
                    )?;
                    continue;
                }
                Instruction::Yield { statement, call } => {
                    if let Some(progress) = self.yield_generator_statement(frame, statement, call, host)? {
                        return Ok(progress);
                    }
                    continue;
                }
                Instruction::Evaluate { statement, expression, action } => {
                    self.evaluate_generator_expression(frame, statement, expression, action, host)?
                }
                Instruction::BranchFalse { statement, condition, target } => {
                    self.evaluate_generator_expression(frame, statement, condition, Resume::BranchFalse(target), host)?
                }
                Instruction::BranchOnValueFalse(target) => {
                    self.step()?;
                    let operand = frame.expression_operand.take().expect("compiled generator branch operand");
                    if !matches!(operand, LogicalOperand::Truthy) {
                        frame.next = target;
                    }
                    continue;
                }
                Instruction::Logical { operator, target, action, .. } => {
                    let operand = frame.expression_operand.take().expect("compiled generator logical operand");
                    if operand.short_circuits(operator) {
                        frame.next = target;
                        Self::apply_generator_expression_action(frame, action, operand);
                    }
                    continue;
                }
                Instruction::Jump(target) => {
                    self.step()?;
                    frame.next = target;
                    continue;
                }
                Instruction::EnterBlock(statement) => {
                    self.enter_generator_block(frame, statement, host)?;
                    continue;
                }
                Instruction::LeaveBlock => {
                    self.step()?;
                    let scope = frame.lexical_scopes.pop().expect("compiled generator block exit");
                    self.release_environment(scope);
                    continue;
                }
                Instruction::PushTry { statement, after, .. } => {
                    self.step()?;
                    self.reserve_generator_stack(&mut frame.try_scopes)?;
                    frame.try_scopes.push(TryScope { statement, after, lexical_depth: frame.lexical_scopes.len() });
                    continue;
                }
                Instruction::FinishTry => {
                    self.step()?;
                    self.finish_generator_try(frame, Ok(Control::Normal(None)), host)?
                }
                _ => unreachable!("instruction outside the generator subset"),
            };
            if !matches!(control, Control::Normal(_)) {
                return Self::generator_completion(control);
            }
        }
        Ok(Progress::Complete(Value::Undefined))
    }

    fn generator_completion(control: Control) -> Result<Progress, Fault> {
        match control {
            Control::Normal(_) => Ok(Progress::Complete(Value::Undefined)),
            Control::Return(value) => Ok(Progress::Complete(value)),
            _ => Err(Fault::new("Unexpected loop control outside a loop.")),
        }
    }

    #[inline(never)]
    fn evaluate_generator_expression(
        &mut self, frame: &mut Activation, statement: StmtId, expression: ExprId, action: Resume, host: &mut dyn Host,
    ) -> Result<Control, Fault> {
        self.step()?;
        let value = self.eval(&frame.program, expression, frame.environment(), &frame.this, host)
            .map_err(|error| error.at(&frame.program.source, frame.program.stmts[statement].at))?;
        if matches!(action, Resume::Return) {
            return Ok(Control::Return(value));
        }
        Self::apply_generator_expression_action(frame, action, LogicalOperand::classify(&value));
        Ok(Control::Normal(None))
    }

    fn apply_generator_expression_action(frame: &mut Activation, action: Resume, operand: LogicalOperand) {
        match action {
            Resume::StoreExpressionValue => frame.expression_operand = Some(operand),
            Resume::Discard => {}
            Resume::BranchFalse(target) => {
                if !matches!(operand, LogicalOperand::Truthy) {
                    frame.next = target;
                }
            }
            _ => unreachable!("compiled generator expression action"),
        }
    }

    #[inline(never)]
    fn yield_generator_statement(
        &mut self, frame: &mut Activation, statement: StmtId, call: Option<GeneratorCallYield>, host: &mut dyn Host,
    ) -> Result<Option<Progress>, Fault> {
        let marker = self.root_marker();
        let result = (|| {
            self.step()?;
            let (operand, delegate) = if let Some(call) = call {
                self.capture_generator_call(frame, &call, host)?;
                (call.operand, call.delegate)
            } else {
                match frame.program.stmts[statement].kind {
                    StmtKind::Yield(operand) => (operand, false),
                    StmtKind::YieldFrom(operand) => (Some(operand), true),
                    _ => unreachable!("compiled yield statement"),
                }
            };
            if delegate {
                self.begin_generator_delegate(frame, operand.expect("delegating yield operand"), host)
            } else {
                let value = match operand {
                    Some(expr) => self.eval(&frame.program, expr, frame.environment(), &frame.this, host)?,
                    None => Value::Undefined,
                };
                Ok(Some(Progress::Yield(value)))
            }
        })();
        self.restore_root_marker(marker);
        result.map_err(|error: Fault| error.at(&frame.program.source, frame.program.stmts[statement].at))
    }

    #[inline(never)]
    fn capture_generator_call(
        &mut self, frame: &mut Activation, site: &GeneratorCallYield, host: &mut dyn Host,
    ) -> Result<(), Fault> {
        let destination = match site.destination {
            Destination::Discard => Destination::Discard,
            Destination::Assign(target) => {
                self.step()?;
                let reference = self.reference(&frame.program, target, frame.environment(), &frame.this, host)?;
                self.push_transient_reference(&reference)?;
                Destination::Assign(reference)
            }
            Destination::Initialize { kind, pattern } => {
                self.step()?;
                Destination::Initialize { kind, pattern }
            }
        };
        let function = self.eval(&frame.program, site.callee, frame.environment(), &frame.this, host)?;
        self.push_transient_value(&function)?;
        let direct_eval = matches!(&frame.program.exprs[site.callee].kind, ExprKind::Name(name) if name == "eval");
        let call = PendingCall { function, direct_eval, destination };
        let bytes = call.retained_bytes();
        self.check_generator_retention(bytes)?;
        frame.pending_call = Some(Box::new(call));
        self.value_bytes += bytes;
        Ok(())
    }

    #[inline(never)]
    fn finish_generator_argument(&mut self, frame: &mut Activation, value: Value, host: &mut dyn Host) -> Result<(), Fault> {
        let Some(call) = frame.pending_call.take() else { return Ok(()) };
        self.value_bytes -= call.retained_bytes();
        let PendingCall { function, direct_eval, destination } = *call;
        let environment = frame.environment();
        let direct = direct_eval.then_some((environment, &frame.this));
        let marker = self.root_marker();
        let result = (|| {
            self.push_transient_value(&function)?;
            self.push_transient_value(&value)?;
            if let Some(reference) = destination.assignment() {
                self.push_transient_reference(reference)?;
            }
            let result = self.invoke_plain_call(function, &[value], direct, host)?;
            match destination {
                Destination::Discard => Ok(()),
                Destination::Assign(reference) => {
                    self.push_transient_value(&result)?;
                    self.write_reference(reference, result, host)
                }
                Destination::Initialize { kind, pattern } => {
                    self.push_transient_value(&result)?;
                    let mode = self.declaration_mode(kind, environment)?;
                    self.initialize_pattern(&frame.program, pattern, result, mode, environment, &frame.this, host)
                }
            }
        })();
        self.restore_root_marker(marker);
        result
    }

    #[inline(never)]
    fn reserve_generator_stack<T>(&mut self, stack: &mut Vec<T>) -> Result<(), Fault> {
        if stack.len() >= MAX_EVAL_DEPTH {
            return Err(Fault::new("Generator scope nesting limit reached (128)."));
        }
        if stack.len() == stack.capacity() {
            let before = stack.capacity();
            self.check_generator_retention(std::mem::size_of::<T>())?;
            stack.try_reserve_exact(1).map_err(|_| Fault::new("Unable to allocate bounded generator scopes."))?;
            let added = (stack.capacity() - before).saturating_mul(std::mem::size_of::<T>());
            self.value_bytes += added;
            if self.value_bytes > self.limits.heap.retained_values {
                return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
            }
        }
        Ok(())
    }

    #[inline(never)]
    fn enter_generator_block(
        &mut self, frame: &mut Activation, statement: StmtId, host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.step()?;
        self.reserve_generator_stack(&mut frame.lexical_scopes)?;
        let scope = self.new_environment(frame.environment(), false)?;
        frame.lexical_scopes.push(scope);
        let StmtKind::Block(body) = &frame.program.stmts[statement].kind else { unreachable!() };
        self.prepare(&frame.program, body, scope, false, host)
    }

    fn unwind_generator_scopes(&mut self, frame: &mut Activation, depth: usize) {
        while frame.lexical_scopes.len() > depth {
            self.release_environment(frame.lexical_scopes.pop().expect("owned generator scope"));
        }
    }

    #[inline(never)]
    fn finish_generator_try(
        &mut self, frame: &mut Activation, completion: Result<Control, Fault>, host: &mut dyn Host,
    ) -> Result<Control, Fault> {
        let scope = frame.try_scopes.pop().expect("compiled generator try exit");
        self.unwind_generator_scopes(frame, scope.lexical_depth);
        frame.next = scope.after;
        self.finish_try_statement(&frame.program, scope.statement, completion, frame.environment(), &frame.this, host)
    }

    #[inline(never)]
    fn finish_generator_body(
        &mut self, frame: &mut Activation, mut completion: Result<Control, Fault>, host: &mut dyn Host,
    ) -> Result<Option<Progress>, Fault> {
        if let Some(call) = frame.pending_call.take() {
            self.value_bytes -= call.retained_bytes();
        }
        if let Some(delegate) = frame.delegate.take() {
            self.value_bytes -= delegate.retained_bytes();
        }
        while !frame.try_scopes.is_empty() && !completion.as_ref().is_err_and(|error| error.exception.is_none()) {
            completion = self.finish_generator_try(frame, completion, host);
            if matches!(completion, Ok(Control::Normal(_))) {
                // A synchronous catch handled the throw. Continue at the end
                // of this try without unwinding any enclosing scopes.
                return Ok(None);
            }
        }
        completion.and_then(Self::generator_completion).map(Some)
    }

    #[inline(never)]
    fn begin_generator_delegate(
        &mut self, frame: &mut Activation, operand: ExprId, host: &mut dyn Host,
    ) -> Result<Option<Progress>, Fault> {
        let marker = self.root_marker();
        let result = (|| {
            let value = self.eval(&frame.program, operand, frame.environment(), &frame.this, host)?;
            self.push_transient_value(&value)?;
            let LoopIterator::Values(record) = self.loop_iterator(value, true, host)? else {
                unreachable!("yield delegation requires a value iterator");
            };
            let bytes = record.retained_bytes();
            self.check_generator_retention(bytes)?;
            frame.delegate = Some(record);
            self.value_bytes += bytes;
            self.advance_generator_delegate(frame, Op::Next, &Value::Undefined, host)
        })();
        self.restore_root_marker(marker);
        result
    }

    #[inline(never)]
    fn advance_generator_delegate(
        &mut self, frame: &mut Activation, operation: Op, input: &Value, host: &mut dyn Host,
    ) -> Result<Option<Progress>, Fault> {
        let record = frame.delegate.as_mut().expect("active yield delegate");
        let result = self.delegate_iterator(record, operation, input, host)?;
        if let Delegated::Yield(result) = result {
            return Ok(Some(Progress::Forwarded(result)));
        }
        self.value_bytes -= record.retained_bytes();
        frame.delegate = None;
        match result {
            Delegated::Done(value) => {
                self.finish_generator_argument(frame, value, host)?;
                Ok(None)
            }
            Delegated::Return(value) => Ok(Some(Progress::Complete(value))),
            Delegated::Yield(_) => unreachable!(),
        }
    }
}
