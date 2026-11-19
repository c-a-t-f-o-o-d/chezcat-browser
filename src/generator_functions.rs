//! Owned generator activations for plain yield statements.
//! https://tc39.es/ecma262/#sec-generatorfunction-definitions
//! https://tc39.es/ecma262/#sec-generator-objects
use super::function_reachability::Edge;
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
    scopes: [Option<usize>; 4],
    this: Value,
    invocation: Option<InvocationContext>,
    realm: Option<usize>,
    native_caller: Option<Option<u32>>,
}

enum Progress {
    Yield(Value),
    Complete(Value),
}

impl Activation {
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.this.bytes())
    }

    fn reachability_edges(&self, add: &mut impl FnMut(Edge)) {
        add(Edge::Function(self.callee));
        for scope in self.scopes.into_iter().flatten() {
            add(Edge::Environment(scope));
        }
        Edge::value(&self.this, add);
    }
}

impl State {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(Edge)) {
        if let Self::SuspendedStart(frame) | Self::SuspendedYield(frame) = self {
            frame.reachability_edges(add);
        }
    }

    pub(super) fn reachability_cost(&self) -> usize {
        if matches!(self, Self::SuspendedStart(_) | Self::SuspendedYield(_)) { 6 } else { 0 }
    }

    pub(super) fn environment_roots(&self, roots: &mut Vec<usize>) {
        if let Self::SuspendedStart(frame) | Self::SuspendedYield(frame) = self {
            roots.extend(frame.scopes.into_iter().flatten());
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
                program: Rc::clone(program), template, callee, environment, next: 0, scopes,
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
        if retained > self.limits.heap.retained_values.saturating_sub(self.value_bytes) {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        self.work(retained)?;
        let id = self.new_object(None)?;
        self.objects[id].prototype = Some(prototype);
        self.objects[id].generator = Some(State::SuspendedStart(Box::new(frame)));
        self.value_bytes += retained;
        Ok(Value::Object(id))
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
        let progress = match operation {
            Op::Next => self.resume_generator(&mut frame, host),
            Op::Return | Op::Throw => self.generator_abrupt_value(operation, args).map(Progress::Complete),
        };
        let value = match progress {
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
        self.release_generator_scopes(frame.scopes);
        self.generator_result(value?, true)
    }

    fn generator_receiver_error() -> Fault {
        Fault::exception("TypeError", "Generator method requires a generator receiver.")
    }

    fn completed_generator(&mut self, operation: Op, args: &[Value]) -> Result<Value, Fault> {
        let value = match operation {
            Op::Next => Value::Undefined,
            Op::Return | Op::Throw => self.generator_abrupt_value(operation, args)?,
        };
        self.generator_result(value, true)
    }

    fn generator_abrupt_value(&mut self, operation: Op, args: &[Value]) -> Result<Value, Fault> {
        let value = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
        if matches!(operation, Op::Throw) { Err(Fault::thrown(value)) } else { Ok(value) }
    }

    fn generator_result(&mut self, value: Value, done: bool) -> Result<Value, Fault> {
        let result = self.new_object(None)?;
        self.store_property(result, "value".into(), value, Some((true, true, true)))?;
        self.store_property(result, "done".into(), Value::Bool(done), Some((true, true, true)))?;
        Ok(Value::Object(result))
    }

    #[inline(never)]
    fn resume_generator(&mut self, frame: &mut Activation, host: &mut dyn Host) -> Result<Progress, Fault> {
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
            let result = rooted.and_then(|()| runtime.run_generator_body(frame, host));
            runtime.restore_root_marker(marker);
            result
        })
    }

    #[inline(never)]
    fn run_generator_body(&mut self, frame: &mut Activation, host: &mut dyn Host) -> Result<Progress, Fault> {
        let previous_invocation = self.invocation;
        let previous_native_caller = self.native_access_caller;
        let previous_strict = self.strict;
        self.invocation = frame.invocation;
        self.native_access_caller = frame.native_caller;
        self.strict = frame.program.templates[frame.template].strict;
        self.call_depth += 1;
        self.transient_roots.owned_call_depth += 1;
        let result = self.drive_generator_body(frame, host);
        self.transient_roots.owned_call_depth -= 1;
        self.call_depth -= 1;
        self.invocation = previous_invocation;
        self.native_access_caller = previous_native_caller;
        self.strict = previous_strict;
        result
    }

    fn drive_generator_body(&mut self, frame: &mut Activation, host: &mut dyn Host) -> Result<Progress, Fault> {
        let body = &frame.program.templates[frame.template].body;
        while let Some(&statement) = body.get(frame.next) {
            frame.next += 1;
            if let StmtKind::Yield(operand) = frame.program.stmts[statement].kind {
                let value = (|| {
                    self.step()?;
                    match operand {
                        Some(operand) => self.eval(&frame.program, operand, frame.environment, &frame.this, host),
                        None => Ok(Value::Undefined),
                    }
                })().map_err(|error: Fault| error.at(&frame.program.source, frame.program.stmts[statement].at))?;
                return Ok(Progress::Yield(value));
            }
            match self.statement(&frame.program, statement, frame.environment, &frame.this, host)? {
                Control::Normal(_) => {}
                Control::Return(value) => return Ok(Progress::Complete(value)),
                _ => return Err(Fault::new("Unexpected loop control outside a loop.")),
            }
        }
        Ok(Progress::Complete(Value::Undefined))
    }
}
