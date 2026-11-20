//! Owned await activations for the parser's finite statement plan. A resumed
//! instruction never evaluates the prefix again. Synchronous subtrees use
//! the ordinary evaluator and its native access checks.
use super::async_plan::{followup_receiver, ArgumentAwait, FollowupCalls, Instruction, LiteralAwait, Resume};
use super::expression_evaluator::{argument_prefix_retained_bytes, PreparedPlainArguments};
use super::function_reachability::Edge;
use super::*;

enum ExitScope {
    Try(TryScope),
    ForOf(Box<ForOfScope>),
    Switch { statement: StmtId, after: usize, environments: usize },
    ClassicFor(ClassicForScope),
}

struct ClassicForScope {
    statement: StmtId,
    advance: usize,
    after: usize,
    environments: usize,
    iteration_depth: usize,
    bindings: Vec<String>,
}

struct ForOfScope {
    record: iterator_protocol::Record,
    asynchronous: bool,
    environments: usize,
    statement: StmtId,
    next: usize,
    after: usize,
}

enum IteratorWait {
    Next(StmtId),
    Close { completion: Box<FinallyCompletion>, after_break: Option<usize> },
}

struct TryScope {
    statement: StmtId,
    after: usize,
    environments: usize,
    phase: TryPhase,
}

enum TryPhase {
    Body { catch_entry: Option<usize>, finally_entry: Option<usize> },
    Finally(Option<Box<FinallyCompletion>>),
}

enum FinallyCompletion {
    Abrupt(Control),
    Throw(Fault),
}

impl FinallyCompletion {
    fn value(&self) -> Option<&Value> {
        match self {
            Self::Abrupt(Control::Return(value)) => Some(value),
            Self::Abrupt(Control::Normal(value) | Control::Break(_, value) | Control::Continue(_, value)) => value.as_ref(),
            Self::Throw(fault) => match fault.exception.as_deref() {
                Some(Exception::Value(value)) => Some(value),
                _ => None,
            },
        }
    }

    fn retained_bytes(&self) -> usize {
        let mut bytes = std::mem::size_of::<Self>()
            .saturating_add(self.value().map_or(0, Value::bytes));
        if let Self::Throw(fault) = self {
            bytes = bytes.saturating_add(fault.message.len());
            if fault.exception.is_some() {
                bytes = bytes.saturating_add(std::mem::size_of::<Exception>());
            }
            if let Some(location) = &fault.location {
                bytes = bytes.saturating_add(std::mem::size_of::<FaultLocation>())
                    .saturating_add(location.context.len());
            }
        }
        bytes
    }
}

struct PendingAwait {
    statement: StmtId,
    action: Resume,
    call: Option<PendingCall>,
    argument_call: Option<Box<PreparedPlainArguments>>,
    method: Option<Box<PendingMethod>>,
    followup_call: Option<FollowupCalls>,
    member_key: Option<ExprId>,
    chain: Option<ExprId>,
    literal: Option<Box<PendingLiteral>>,
    negations: usize,
    assignment: Option<Reference>,
}

struct PendingLiteral {
    object: usize,
    site: LiteralAwait,
    target: PendingLiteralTarget,
}

enum PendingLiteralTarget {
    Property(PropertyKey),
    Element(usize),
}

impl PendingLiteral {
    fn retained_bytes(&self) -> usize {
        let key_bytes = match &self.target {
            PendingLiteralTarget::Property(key) => property_key_bytes(key),
            PendingLiteralTarget::Element(_) => 0,
        };
        std::mem::size_of::<Self>().saturating_add(key_bytes)
    }
}

struct PendingCall {
    function: Value,
    direct_eval: bool,
}

struct PendingMethod {
    call: ExprId,
    index: usize,
    receiver: Value,
    key: PropertyKey,
    function: Option<Value>,
    prefix: Vec<Value>,
}

impl PendingMethod {
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.receiver.bytes())
            .saturating_add(property_key_bytes(&self.key))
            .saturating_add(self.function.as_ref().map_or(0, Value::bytes))
            .saturating_add(argument_prefix_retained_bytes(&self.prefix))
    }
}

fn property_key_bytes(key: &PropertyKey) -> usize {
    match key {
        PropertyKey::String(name) => name.len(),
        PropertyKey::Symbol(_) => 0,
    }
}

fn apply_logical_not(value: Value, negations: usize) -> Value {
    if negations == 0 {
        value
    } else {
        // After the first ToBoolean, pairs of negations cancel.
        Value::Bool(value.truthy() == (negations % 2 == 0))
    }
}

pub(super) struct Activation {
    program: Rc<Program>,
    template: usize,
    callee: usize,
    outer: usize,
    this: Value,
    realm: Option<usize>,
    invocation: Option<InvocationContext>,
    native_caller: Option<Option<u32>>,
    strict: bool,
    next: usize,
    environments: Vec<usize>,
    exit_scopes: Vec<ExitScope>,
    pending: Option<PendingAwait>,
    iterator_wait: Option<IteratorWait>,
    expression_value: Option<Value>,
    comparison_operands: Vec<Value>,
    assignments: Vec<Reference>,
    plain_calls: Vec<Box<PreparedPlainArguments>>,
}

impl Activation {
    fn scope_storage_bytes(&self) -> usize {
        self.environments.capacity() * std::mem::size_of::<usize>()
            + self.exit_scopes.capacity() * std::mem::size_of::<ExitScope>()
    }

    pub(super) fn realm(&self) -> Option<usize> {
        self.realm
    }

    pub(super) fn retained_bytes(&self) -> usize {
        let references = self.assignment_references().fold(0usize, |bytes, reference| {
            bytes.saturating_add(reference.assignment_bytes())
        });
        let comparisons = self.comparison_operands.iter().fold(
            self.comparison_operands.capacity().saturating_mul(std::mem::size_of::<Value>()),
            |bytes, value| bytes.saturating_add(value.bytes()),
        );
        std::mem::size_of::<Self>()
            .saturating_add(self.this.bytes())
            .saturating_add(self.expression_value.as_ref().map_or(0, Value::bytes))
            .saturating_add(comparisons)
            .saturating_add(self.pending_call().map_or(0, |call| call.function.bytes()))
            .saturating_add(self.pending_argument_call().map_or(0, PreparedPlainArguments::retained_bytes))
            .saturating_add(self.pending_method().map_or(0, PendingMethod::retained_bytes))
            .saturating_add(self.pending_literal().map_or(0, PendingLiteral::retained_bytes))
            .saturating_add(self.pending_iterator_close().map_or(0, FinallyCompletion::retained_bytes))
            .saturating_add(references)
            .saturating_add(self.assignments.capacity().saturating_mul(std::mem::size_of::<Reference>()))
            .saturating_add(self.plain_calls.iter().fold(
                self.plain_calls.capacity().saturating_mul(std::mem::size_of::<Box<PreparedPlainArguments>>()),
                |bytes, call| bytes.saturating_add(call.retained_bytes()),
            ))
            .saturating_add(self.scope_storage_bytes())
            .saturating_add(self.finally_completions().fold(0usize, |bytes, completion| {
                bytes.saturating_add(completion.retained_bytes())
            }))
            .saturating_add(self.active_iterators().fold(0usize, |bytes, scope| {
                bytes.saturating_add(std::mem::size_of::<ForOfScope>())
                    .saturating_add(scope.record.retained_bytes())
            }))
            .saturating_add(self.exit_scopes.iter().fold(0usize, |bytes, scope| match scope {
                ExitScope::ClassicFor(scope) => bytes.saturating_add(classic_for_binding_bytes(&scope.bindings)),
                _ => bytes,
            }))
    }

    pub(super) fn reachability_cost(&self) -> usize {
        // Include primitive slots as a conservative, constant-size traversal
        // charge. Program storage is already charged and retained by callee.
        self.environments
            .len()
            .saturating_add(self.exit_scopes.len())
            .saturating_add(self.active_iterators().count())
            .saturating_add(self.comparison_operands.len())
            .saturating_add(5 + usize::from(self.pending_call().is_some())
                + usize::from(self.expression_value.is_some())
                + 2 * usize::from(self.pending_method().is_some())
                + usize::from(self.pending_literal().is_some())
                + usize::from(self.pending_iterator_close().is_some())
                + 2 * usize::from(self.pending_assignment().is_some()))
            .saturating_add(self.assignments.len().saturating_mul(2))
            .saturating_add(self.pending_argument_call().map_or(0, |call| 1 + call.prefix.len()))
            .saturating_add(self.pending_method().map_or(0, |method| method.prefix.len()))
            .saturating_add(self.plain_calls.iter().fold(0usize, |cost, call| cost.saturating_add(1 + call.prefix.len())))
    }

    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(Edge)) {
        add(Edge::Function(self.callee));
        add(Edge::Object(self.outer));
        Edge::value(&self.this, add);
        if let Some(value) = &self.expression_value {
            Edge::value(value, add);
        }
        for value in &self.comparison_operands {
            Edge::value(value, add);
        }
        if let Some(call) = self.pending_call() {
            Edge::value(&call.function, add);
        }
        if let Some(call) = self.pending_argument_call() {
            Edge::value(&call.function, add);
            for value in &call.prefix { Edge::value(value, add); }
        }
        for call in &self.plain_calls {
            Edge::value(&call.function, add);
            for value in &call.prefix { Edge::value(value, add); }
        }
        if let Some(method) = self.pending_method() {
            Edge::value(&method.receiver, add);
            if let Some(function) = &method.function {
                Edge::value(function, add);
            }
            for value in &method.prefix { Edge::value(value, add); }
        }
        if let Some(literal) = self.pending_literal() {
            add(Edge::Object(literal.object));
        }
        for reference in self.assignment_references() {
            Edge::reference(reference, add);
        }
        for completion in self.finally_completions() {
            if let Some(value) = completion.value() {
                Edge::value(value, add);
            }
        }
        if let Some(value) = self.pending_iterator_close().and_then(FinallyCompletion::value) {
            Edge::value(value, add);
        }
        for scope in self.active_iterators() {
            scope.record.reachability_edges(add);
        }
        if let Some(context) = self.invocation {
            add(Edge::Environment(context.environment));
        }
        for &environment in &self.environments {
            add(Edge::Environment(environment));
        }
    }

    pub(super) fn environment_roots(&self, roots: &mut Vec<usize>) {
        roots.extend(self.environments.iter().copied());
        if let Some(context) = self.invocation {
            roots.push(context.environment);
        }
        for reference in self.assignment_references() {
            if let Reference::Binding(scope, _) = reference {
                roots.push(*scope);
            }
        }
    }

    fn environment(&self) -> Result<usize, Fault> {
        self.environments
            .last()
            .copied()
            .ok_or_else(|| Fault::new("Missing async lexical environment."))
    }

    fn pending_call(&self) -> Option<&PendingCall> {
        self.pending.as_ref().and_then(|pending| pending.call.as_ref())
    }

    fn pending_iterator_close(&self) -> Option<&FinallyCompletion> {
        match &self.iterator_wait {
            Some(IteratorWait::Close { completion, .. }) => Some(completion),
            _ => None,
        }
    }

    fn pending_argument_call(&self) -> Option<&PreparedPlainArguments> {
        self.pending.as_ref().and_then(|pending| pending.argument_call.as_deref())
    }

    fn pending_method(&self) -> Option<&PendingMethod> {
        self.pending.as_ref().and_then(|pending| pending.method.as_deref())
    }

    fn pending_literal(&self) -> Option<&PendingLiteral> {
        self.pending.as_ref().and_then(|pending| pending.literal.as_deref())
    }

    fn take_expression_value(&mut self) -> Result<Value, Fault> {
        self.expression_value.take().ok_or_else(|| Fault::new("Async expression has no pending value."))
    }

    fn pending_assignment(&self) -> Option<&Reference> {
        self.pending.as_ref().and_then(|pending| pending.assignment.as_ref())
    }

    fn assignment_references(&self) -> impl Iterator<Item = &Reference> {
        self.assignments.iter().chain(self.pending_assignment())
    }

    fn finally_completions(&self) -> impl Iterator<Item = &FinallyCompletion> {
        self.exit_scopes.iter().filter_map(|scope| match scope {
            ExitScope::Try(TryScope { phase: TryPhase::Finally(completion), .. }) => completion.as_deref(),
            _ => None,
        })
    }

    fn active_iterators(&self) -> impl Iterator<Item = &ForOfScope> {
        self.exit_scopes.iter().filter_map(|scope| match scope {
            ExitScope::ForOf(iterator) => Some(iterator.as_ref()),
            _ => None,
        })
    }
}

enum Progress {
    Continue,
    Abrupt(Control),
    Suspend(usize),
}

impl Runtime {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn start_await_body(
        &mut self,
        program: &Rc<Program>,
        template: usize,
        callee: usize,
        environment: usize,
        this: &Value,
        outer: usize,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.step()?;
        let bytes = std::mem::size_of::<Activation>()
            .saturating_add(std::mem::size_of::<usize>())
            .saturating_add(this.bytes());
        self.check_await_storage(bytes, self.value_bytes)?;
        self.work_as(bytes.saturating_sub(this.bytes()), WorkKind::ArgumentCapture)?;
        let mut environments = Vec::new();
        environments
            .try_reserve_exact(1)
            .map_err(|_| Fault::new("Unable to allocate bounded async activation."))?;
        environments.push(environment);
        let frame = Box::new(Activation {
            program: Rc::clone(program),
            template,
            callee,
            outer,
            this: self.copy_value(this)?,
            realm: self.active_realm,
            invocation: self.invocation,
            native_caller: self.native_access_caller,
            strict: self.strict,
            next: 0,
            environments,
            exit_scopes: Vec::new(),
            pending: None,
            iterator_wait: None,
            expression_value: None,
            comparison_operands: Vec::new(),
            assignments: Vec::new(),
            plain_calls: Vec::new(),
        });
        self.drive_await_body(frame, None, host)
    }

    pub(super) fn resume_await_body(
        &mut self,
        frame: Box<Activation>,
        value: Value,
        reject: bool,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.step()?;
        if frame.realm != self.active_realm {
            return Err(Fault::new("Async activation resumed in a different realm."));
        }
        self.check_native_call_stack("Function call")?;
        if value.bytes() > self.limits.capacity.string_bytes {
            return Err(self.string_limit_fault());
        }
        // Invocation data lives in its retained genuine function environment;
        // arrows retain their original lexical context, including new.target.
        if let Some(context) = frame.invocation {
            self.invocation_data(context)?;
        }
        let previous_invocation = self.invocation;
        let previous_caller = self.native_access_caller;
        let previous_strict = self.strict;
        self.invocation = frame.invocation;
        self.native_access_caller = frame.native_caller;
        self.strict = frame.strict;
        self.call_depth += 1;
        self.transient_roots.owned_call_depth += 1;
        let result = self.drive_await_body(frame, Some((value, reject)), host);
        self.transient_roots.owned_call_depth -= 1;
        self.call_depth -= 1;
        self.strict = previous_strict;
        self.native_access_caller = previous_caller;
        self.invocation = previous_invocation;
        result
    }

    fn check_await_storage(&self, retained: usize, occupied: usize) -> Result<(), Fault> {
        if retained
            > self
                .limits
                .heap
                .retained_values
                .saturating_sub(occupied)
        {
            return Err(Fault::new(format!(
                "Retained value limit reached ({} bytes).",
                self.limits.heap.retained_values
            )));
        }
        Ok(())
    }

    fn await_storage(&mut self, retained: usize, work: usize) -> Result<(), Fault> {
        // Replace only this driver's reservation. Reentrant frames see all
        // outer activations, including their retained operands and values.
        let occupied = self.value_bytes - self.active_await_storage_bytes;
        self.check_await_storage(retained, occupied)?;
        self.work_as(work, WorkKind::ArgumentCapture)?;
        self.value_bytes = occupied + retained;
        self.active_await_storage_bytes = retained;
        Ok(())
    }

    fn publish_await_activation(
        &mut self,
        frame: &Activation,
    ) -> Result<transient_roots::RootMarker, Fault> {
        self.await_storage(frame.retained_bytes(), 0)?;
        let marker = self.root_marker();
        let result = (|| {
            self.work(
                frame
                    .reachability_cost()
                    .saturating_mul(std::mem::size_of::<Edge>()),
            )?;
            let mut result = Ok(());
            frame.reachability_edges(&mut |edge| {
                if result.is_ok() {
                    result = self.push_transient_root(edge);
                }
            });
            result
        })();
        if let Err(error) = result {
            self.restore_root_marker(marker);
            return Err(error);
        }
        Ok(marker)
    }

    fn drive_await_body(
        &mut self,
        frame: Box<Activation>,
        resumed: Option<(Value, bool)>,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.check_await_storage(frame.retained_bytes(), self.value_bytes)?;
        let bytes = frame.retained_bytes();
        let outer = std::mem::replace(&mut self.active_await_storage_bytes, bytes);
        self.value_bytes += bytes;
        let result = self.drive_active_await_body(frame, resumed, host);
        self.value_bytes -= std::mem::replace(&mut self.active_await_storage_bytes, outer);
        result
    }

    fn drive_active_await_body(
        &mut self,
        mut frame: Box<Activation>,
        mut resumed: Option<(Value, bool)>,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        loop {
            let result = self.await_instruction(&mut frame, resumed.take(), host);
            let result = match result {
                Ok(Progress::Abrupt(control)) => {
                    self.unwind_await_exit_scopes(&mut frame, Ok(control), host)
                }
                Err(error) => {
                    // An abrupt result abandons the current expression before
                    // a handler can start another one.
                    frame.assignments.clear();
                    frame.comparison_operands.clear();
                    frame.plain_calls.clear();
                    frame.expression_value = None;
                    self.unwind_await_exit_scopes(&mut frame, Err(error), host)
                }
                other => other,
            };
            match result {
                Ok(Progress::Continue) => {}
                Ok(Progress::Suspend(promise)) => {
                    let result = self.publish_await_activation(&frame).and_then(|marker| {
                        let result = (|| {
                            self.push_transient_root(Edge::Object(promise))?;
                            // Existing call-scope cleanup may now run. The
                            // frame's explicit roots govern later collection.
                            if let Some(&environment) = frame.environments.last() {
                                self.capture_await_environment(environment)?;
                            }
                            if let Some(context) = frame.invocation {
                                self.capture_await_environment(context.environment)?;
                            }
                            for reference in frame.assignment_references() {
                                if let Reference::Binding(scope, _) = reference {
                                    self.capture_await_environment(*scope)?;
                                }
                            }
                            // Transfer ownership of the capacity to the
                            // reaction/job accounting before admitting it.
                            self.value_bytes -= std::mem::take(&mut self.active_await_storage_bytes);
                            self.suspend_await(promise, frame)
                        })();
                        self.restore_root_marker(marker);
                        result
                    });
                    return result;
                }
                completion => {
                    self.unwind_await_environments(&mut frame, 1);
                    let marker = self.publish_await_activation(&frame)?;
                    let result = match completion {
                        Ok(Progress::Abrupt(Control::Return(value))) => self
                            .push_transient_value(&value)
                            .and_then(|()| self.finish_async_result(frame.outer, Ok(value), host)),
                        Err(error) => {
                            let rooted = match error.exception.as_deref() {
                                Some(Exception::Value(value)) => self.push_transient_value(value),
                                _ => Ok(()),
                            };
                            rooted.and_then(|()| {
                                self.finish_async_result(frame.outer, Err(error), host)
                            })
                        }
                        _ => unreachable!(),
                    };
                    self.restore_root_marker(marker);
                    return result;
                }
            }
        }
    }

    fn await_instruction(
        &mut self,
        frame: &mut Activation,
        resumed: Option<(Value, bool)>,
        host: &mut dyn Host,
    ) -> Result<Progress, Fault> {
        self.step()?;
        let marker = self.publish_await_activation(frame)?;
        let result = (|| {
            if let Some((value, reject)) = resumed {
                self.push_transient_value(&value)?;
                if let Some(wait) = frame.iterator_wait.take() {
                    return self.resume_iterator_wait(frame, wait, value, reject, host);
                }
                let PendingAwait {
                    statement, action, call, argument_call, method, followup_call, member_key, chain, literal, negations, assignment,
                } = frame
                    .pending
                    .take()
                    .ok_or_else(|| Fault::new("Async activation has no pending await."))?;
                let result = if reject {
                    self.work(value.bytes())?;
                    Err(Fault::thrown(value))
                } else {
                    (|| {
                        let value = if let Some(call) = call {
                            let direct = call.direct_eval.then_some((frame.environment()?, &frame.this));
                            let result = self.invoke_plain_call(call.function, &[value], direct, host)?;
                            self.push_transient_value(&result)?;
                            result
                        } else {
                            value
                        };
                        let value = if let Some(arguments) = argument_call {
                            let result = self.eval_resumed_plain_argument(
                                &frame.program, *arguments, value, frame.environment()?, &frame.this, host,
                            )?;
                            self.push_transient_value(&result)?;
                            result
                        } else {
                            value
                        };
                        let value = if let Some(method) = method {
                            let PendingMethod { call, index, receiver, key, function, prefix } = *method;
                            let result = self.eval_resumed_member_argument(
                                &frame.program, call, (receiver, key, function), (index, prefix), value,
                                frame.environment()?, &frame.this, host,
                            )?;
                            self.push_transient_value(&result)?;
                            result
                        } else {
                            value
                        };
                        let value = if let Some(calls) = followup_call {
                            let result = self.eval_await_followup_calls(frame, calls, value, host)?;
                            self.push_transient_value(&result)?;
                            result
                        } else {
                            value
                        };
                        let value = if let Some(key) = member_key {
                            let key = self.eval(
                                &frame.program, key, frame.environment()?, &frame.this, host,
                            )?;
                            self.push_transient_value(&key)?;
                            let result = self.read_reference(&Reference::ComputedProperty(value, key), host)?;
                            self.push_transient_value(&result)?;
                            result
                        } else {
                            value
                        };
                        let value = if let Some(chain) = chain {
                            let result = self.eval_resumed_expression(
                                &frame.program, chain, value, frame.environment()?, &frame.this, host,
                            )?;
                            self.push_transient_value(&result)?;
                            result
                        } else {
                            value
                        };
                        let value = apply_logical_not(value, negations);
                        let value = if let Some(literal) = literal {
                            self.finish_await_literal(frame, *literal, value, host)?
                        } else {
                            value
                        };
                        let value = if let Some(reference) = assignment {
                            self.finish_await_assignment(reference, value, host)?
                        } else {
                            value
                        };
                        self.await_resume_action(frame, action, value, host)
                    })()
                };
                return result.map_err(|error| {
                    error.at(&frame.program.source, frame.program.stmts[statement].at)
                });
            }
            let code = &frame.program.templates[frame.template].resumption_plan;
            let Some(instruction) = code.get(frame.next).copied() else {
                return Ok(Progress::Abrupt(Control::Return(Value::Undefined)));
            };
            frame.next += 1;
            let environment = frame.environment()?;
            match instruction {
                Instruction::Yield { .. } | Instruction::YieldReturn { .. } => unreachable!("yield instruction in async function"),
                Instruction::Statement(statement) => {
                    let control =
                        self.statement(&frame.program, statement, environment, &frame.this, host)?;
                    Ok(Self::await_control(control))
                }
                Instruction::Declare { statement, index } => {
                    self.evaluate_resumption_declarator(&frame.program, statement, index, environment, &frame.this, host)?;
                    Ok(Progress::Continue)
                }
                Instruction::Evaluate { statement, expression, action } => {
                    let result = (|| {
                        let value = self.eval(&frame.program, expression, environment, &frame.this, host)?;
                        self.push_transient_value(&value)?;
                        self.await_resume_action(frame, action, value, host)
                    })();
                    result.map_err(|error: Fault| {
                        error.at(&frame.program.source, frame.program.stmts[statement].at)
                    })
                }
                Instruction::CaptureAssignment { statement, target } => {
                    let result = (|| {
                        self.reserve_await_assignment(frame)?;
                        let reference = self.prepare_await_assignment(frame, target, host)?;
                        frame.assignments.push(reference);
                        Ok(Progress::Continue)
                    })();
                    result.map_err(|error: Fault| {
                        error.at(&frame.program.source, frame.program.stmts[statement].at)
                    })
                }
                Instruction::CapturePlainCall { statement, site } => {
                    let result = (|| {
                        let call = self.prepare_await_argument_call(frame, site, None, host)?
                            .expect("compiled non-optional async call");
                        frame.plain_calls.push(call);
                        Ok(Progress::Continue)
                    })();
                    result.map_err(|error: Fault| error.at(&frame.program.source, frame.program.stmts[statement].at))
                }
                Instruction::CaptureOptionalPlainCall { statement, site, skip, action } => {
                    let result = (|| {
                        if let Some(call) = self.prepare_await_argument_call(frame, site, None, host)? {
                            frame.plain_calls.push(call);
                            Ok(Progress::Continue)
                        } else {
                            frame.next = skip;
                            self.await_resume_action(frame, action, Value::Undefined, host)
                        }
                    })();
                    result.map_err(|error: Fault| error.at(&frame.program.source, frame.program.stmts[statement].at))
                }
                Instruction::InvokePlainCall { statement, action } => {
                    let result = (|| {
                        let argument = frame.take_expression_value()?;
                        let call = frame.plain_calls.pop().ok_or_else(|| Fault::new("Async expression has no captured call."))?;
                        let value = self.eval_resumed_plain_argument(&frame.program, *call, argument, environment, &frame.this, host)?;
                        self.push_transient_value(&value)?;
                        self.await_resume_action(frame, action, value, host)
                    })();
                    result.map_err(|error: Fault| error.at(&frame.program.source, frame.program.stmts[statement].at))
                }
                Instruction::WriteAssignment { statement, action } => {
                    let result = (|| {
                        let value = frame.take_expression_value()?;
                        let reference = frame.assignments.pop()
                            .ok_or_else(|| Fault::new("Async assignment has no captured target."))?;
                        let value = self.finish_await_assignment(reference, value, host)?;
                        self.await_resume_action(frame, action, value, host)
                    })();
                    result.map_err(|error: Fault| {
                        error.at(&frame.program.source, frame.program.stmts[statement].at)
                    })
                }
                Instruction::EnterBlock(statement) => {
                    self.reserve_await_environment(frame)?;
                    let StmtKind::Block(body) = &frame.program.stmts[statement].kind else {
                        return Err(Fault::new("Invalid async block instruction."));
                    };
                    let scope = self.new_environment(environment, false)?;
                    frame.environments.push(scope);
                    self.push_transient_root(Edge::Environment(scope))?;
                    self.prepare(&frame.program, body, scope, false, host)?;
                    Ok(Progress::Continue)
                }
                Instruction::LeaveBlock => {
                    if frame.environments.len() <= 1 {
                        return Err(Fault::new("Invalid async lexical exit."));
                    }
                    let scope = frame.environments.pop().expect("checked lexical stack");
                    self.release_environment(scope);
                    Ok(Progress::Continue)
                }
                Instruction::EnterSwitch { statement, leave } => {
                    self.enter_await_switch(frame, statement, leave, environment, host)
                        .map_err(|error| error.at(&frame.program.source, frame.program.stmts[statement].at))
                }
                Instruction::LeaveSwitch => {
                    let Some(ExitScope::Switch { environments, .. }) = frame.exit_scopes.pop() else {
                        return Err(Fault::new("Invalid async switch exit."));
                    };
                    self.unwind_await_environments(frame, environments);
                    Ok(Progress::Continue)
                }
                Instruction::EnterFor { statement, advance, after } => {
                    self.enter_await_classic_for(frame, statement, advance, after, host)
                        .map_err(|error| error.at(&frame.program.source, frame.program.stmts[statement].at))
                }
                Instruction::RefreshForBindings => {
                    self.refresh_await_for_bindings(frame)?;
                    Ok(Progress::Continue)
                }
                Instruction::LeaveFor => {
                    let Some(ExitScope::ClassicFor(scope)) = frame.exit_scopes.pop() else {
                        return Err(Fault::new("Invalid async for exit."));
                    };
                    self.unwind_await_environments(frame, scope.environments);
                    Ok(Progress::Continue)
                }
                Instruction::EnterForOf { statement, after } => {
                    self.enter_await_for_of(frame, statement, after, environment, host)
                        .map_err(|error| error.at(&frame.program.source, frame.program.stmts[statement].at))
                }
                Instruction::NextForOf(statement) => {
                    self.next_await_for_of(frame, statement, environment, host)
                        .map_err(|error| error.at(&frame.program.source, frame.program.stmts[statement].at))
                }
                Instruction::BranchFalse {
                    statement,
                    condition,
                    target,
                } => {
                    let value = self
                        .eval(&frame.program, condition, environment, &frame.this, host)
                        .map_err(|error| {
                            error.at(&frame.program.source, frame.program.stmts[statement].at)
                        })?;
                    if !value.truthy() {
                        frame.next = target;
                    }
                    Ok(Progress::Continue)
                }
                Instruction::BranchOnValueFalse(target) => {
                    if !frame.take_expression_value()?.truthy() {
                        frame.next = target;
                    }
                    Ok(Progress::Continue)
                }
                Instruction::Jump(target) => {
                    frame.next = target;
                    Ok(Progress::Continue)
                }
                Instruction::PushTry { statement, after, catch_entry, finally_entry } => {
                    self.reserve_await_exit_scope(frame)?;
                    frame.exit_scopes.push(ExitScope::Try(TryScope {
                        statement,
                        after,
                        environments: frame.environments.len(),
                        phase: TryPhase::Body { catch_entry, finally_entry },
                    }));
                    Ok(Progress::Continue)
                }
                Instruction::FinishTry => self.finish_await_try_body(frame, environment, host),
                Instruction::FinishFinally => Self::finish_await_finally(frame),
                Instruction::Logical { statement, operator, target, action } => {
                    let result = (|| {
                        let value = frame.take_expression_value()?;
                        if !logical_short_circuits(operator, &value) {
                            return Ok(Progress::Continue);
                        }
                        frame.next = target;
                        self.await_resume_action(frame, action, value, host)
                    })();
                    result.map_err(|error: Fault| {
                        error.at(&frame.program.source, frame.program.stmts[statement].at)
                    })
                }
                Instruction::BeginLogicalNot(negations) => {
                    for _ in 1..negations {
                        self.step()?;
                    }
                    Ok(Progress::Continue)
                }
                Instruction::FinishLogicalNot { statement, negations, action } => {
                    let value = apply_logical_not(frame.take_expression_value()?, negations);
                    self.await_resume_action(frame, action, value, host)
                        .map_err(|error| error.at(&frame.program.source, frame.program.stmts[statement].at))
                }
                Instruction::CaptureComparison(statement) => {
                    self.capture_await_comparison(frame)
                        .map_err(|error| error.at(&frame.program.source, frame.program.stmts[statement].at))
                }
                Instruction::Comparison { statement, operator, action } => {
                    let result = (|| {
                        let right = frame.take_expression_value()?;
                        let left = frame.comparison_operands.pop()
                            .ok_or_else(|| Fault::new("Async comparison has no left operand."))?;
                        let value = self.binary_value(operator, left, right, host)?;
                        self.await_resume_action(frame, action, value, host)
                    })();
                    result.map_err(|error: Fault| {
                        error.at(&frame.program.source, frame.program.stmts[statement].at)
                    })
                }
                Instruction::ReadAccess { statement, expression, action } => {
                    let result = (|| {
                        let receiver = frame.take_expression_value()?;
                        let value = self.eval_resumed_expression(
                            &frame.program, expression, receiver, environment, &frame.this, host,
                        )?;
                        self.push_transient_value(&value)?;
                        self.await_resume_action(frame, action, value, host)
                    })();
                    result.map_err(|error: Fault| {
                        error.at(&frame.program.source, frame.program.stmts[statement].at)
                    })
                }
                Instruction::AwaitComputed { statement, action } => {
                    let result = (|| {
                        let value = frame.take_expression_value()?;
                        self.push_transient_value(&value)?;
                        let promise = self.promise_for_await(value, host)?;
                        frame.pending = Some(PendingAwait {
                            statement, action, call: None, argument_call: None, method: None,
                            followup_call: None, member_key: None, chain: None, literal: None,
                            negations: 0, assignment: None,
                        });
                        Ok(Progress::Suspend(promise))
                    })();
                    result.map_err(|error: Fault| error.at(&frame.program.source, frame.program.stmts[statement].at))
                }
                Instruction::Await {
                    statement,
                    operand,
                    callee,
                    method,
                    argument_call,
                    followup_call,
                    member_key,
                    chain,
                    literal,
                    negations,
                    assignment,
                    action,
                } => {
                    let result = (|| {
                        let assignment = if let Some(target) = assignment {
                            Some(self.prepare_await_assignment(frame, target, host)?)
                        } else {
                            None
                        };
                        let mut retained = frame.retained_bytes()
                            .saturating_add(assignment.as_ref().map_or(0, Reference::assignment_bytes));
                        let literal = if let Some(site) = literal {
                            let prepared = self.prepare_await_literal(frame, site, retained, host)?;
                            retained = retained.saturating_add(prepared.retained_bytes());
                            Some(prepared)
                        } else {
                            None
                        };
                        for _ in 0..negations {
                            self.step()?;
                        }
                        if member_key.is_some() {
                            self.step()?; // Ordinary member reference construction.
                        }
                        if chain.is_some() {
                            self.step()?; // Optional-chain reference construction.
                        }
                        if let Some(calls) = followup_call {
                            if calls.count > MAX_EVALUATION_FRAMES.saturating_sub(self.evaluation_frames) {
                                return Err(Fault::new("Pending evaluation limit reached (4096)."));
                            }
                            for _ in 0..calls.count {
                                self.step()?; // Enter each call before evaluating its callee.
                            }
                        }
                        let method = if let Some(method) = method {
                            let prepared = self.prepare_await_method(frame, method, retained, host)?;
                            retained = retained.saturating_add(prepared.retained_bytes());
                            Some(prepared)
                        } else {
                            None
                        };
                        let argument_call = if let Some(site) = argument_call {
                            let prepared = self.prepare_await_argument_call(frame, site, Some(retained), host)?
                                .expect("compiled non-optional async argument");
                            retained = retained.saturating_add(prepared.retained_bytes());
                            Some(prepared)
                        } else {
                            None
                        };
                        let call = if let Some(callee) = callee {
                            let function = self.eval(&frame.program, callee, environment, &frame.this, host)?;
                            self.push_transient_value(&function)?;
                            self.await_storage(
                                retained.saturating_add(function.bytes()),
                                function.bytes(),
                            )?;
                            let direct_eval = matches!(&frame.program.exprs[callee].kind, ExprKind::Name(name) if name == "eval");
                            Some(PendingCall { function, direct_eval })
                        } else {
                            None
                        };
                        let value =
                            self.eval(&frame.program, operand, environment, &frame.this, host)?;
                        self.push_transient_value(&value)?;
                        let promise = self.promise_for_await(value, host)?;
                        frame.pending = Some(PendingAwait {
                            statement, action, call, argument_call, method, followup_call, member_key, chain, literal, negations, assignment,
                        });
                        Ok(Progress::Suspend(promise))
                    })();
                    result.map_err(|error: Fault| {
                        error.at(&frame.program.source, frame.program.stmts[statement].at)
                    })
                }
            }
        })();
        self.restore_root_marker(marker);
        result
    }

    fn reserve_await_assignment(&mut self, frame: &mut Activation) -> Result<(), Fault> {
        if frame.assignments.len() >= MAX_EVALUATION_FRAMES {
            return Err(Fault::new("Async assignment nesting limit reached (4096)."));
        }
        if frame.assignments.len() == frame.assignments.capacity() {
            let slot = std::mem::size_of::<Reference>();
            self.await_storage(frame.retained_bytes().saturating_add(slot),
                (frame.assignments.len() + 1).saturating_mul(slot))?;
            frame.assignments.try_reserve_exact(1)
                .map_err(|_| Fault::new("Unable to allocate bounded async assignment stack."))?;
        }
        Ok(())
    }

    fn reserve_await_plain_call(&mut self, frame: &mut Activation) -> Result<(), Fault> {
        if frame.plain_calls.len() >= MAX_EVALUATION_FRAMES {
            return Err(Fault::new("Async call nesting limit reached (4096)."));
        }
        if frame.plain_calls.len() == frame.plain_calls.capacity() {
            let slot = std::mem::size_of::<Box<PreparedPlainArguments>>();
            self.await_storage(frame.retained_bytes().saturating_add(slot),
                (frame.plain_calls.len() + 1).saturating_mul(slot))?;
            frame.plain_calls.try_reserve_exact(1)
                .map_err(|_| Fault::new("Unable to allocate bounded async call stack."))?;
        }
        Ok(())
    }

    #[inline(never)]
    fn capture_await_comparison(&mut self, frame: &mut Activation) -> Result<Progress, Fault> {
        let value = frame.take_expression_value()?;
        let operands = &frame.comparison_operands;
        if operands.len() >= MAX_EVALUATION_FRAMES {
            return Err(Fault::new("Async comparison nesting limit reached (4096)."));
        }
        let retained = frame.retained_bytes().saturating_add(value.bytes());
        if operands.len() == operands.capacity() {
            let slot = std::mem::size_of::<Value>();
            self.await_storage(retained.saturating_add(slot),
                (operands.len() + 1).saturating_mul(slot).saturating_add(value.bytes()))?;
            frame.comparison_operands.try_reserve_exact(1)
                .map_err(|_| Fault::new("Unable to allocate bounded async comparison stack."))?;
        } else {
            self.await_storage(retained, value.bytes())?;
        }
        frame.comparison_operands.push(value);
        Ok(Progress::Continue)
    }

    fn finish_await_assignment(
        &mut self, reference: Reference, value: Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let result = self.copy_value(&value)?;
        self.write_reference(reference, value, host)?;
        Ok(result)
    }

    #[inline(never)]
    fn prepare_await_argument_call(
        &mut self, frame: &mut Activation, site: ArgumentAwait, retained: Option<usize>, host: &mut dyn Host,
    ) -> Result<Option<Box<PreparedPlainArguments>>, Fault> {
        self.step()?;
        let (callee, _, kind) = async_plan::plain_call_parts(&frame.program, site.call)
            .expect("compiled async argument call");
        let environment = frame.environment()?;
        let function = self.eval(&frame.program, callee, environment, &frame.this, host)?;
        if matches!(kind, async_plan::PlainCallKind::Optional) {
            self.step()?;
            if matches!(function, Value::Null | Value::Undefined) {
                return Ok(None);
            }
        }
        self.push_transient_value(&function)?;
        let retained = if let Some(retained) = retained {
            retained
        } else {
            self.reserve_await_plain_call(frame)?;
            frame.retained_bytes()
        };
        let (_, arguments, _) = async_plan::plain_call_parts(&frame.program, site.call)
            .expect("compiled async argument call");
        let direct_eval = matches!(kind, async_plan::PlainCallKind::Ordinary)
            && matches!(&frame.program.exprs[callee].kind, ExprKind::Name(name) if name == "eval");
        let prefix = self.eval_await_argument_prefix(
            &frame.program, &arguments[..site.index], environment, &frame.this, host,
        )?;
        let prepared = PreparedPlainArguments { call: site.call, index: site.index, function, direct_eval, prefix };
        let bytes = prepared.retained_bytes();
        self.await_storage(retained.saturating_add(bytes), bytes)?;
        Ok(Some(Box::new(prepared)))
    }

    #[inline(never)]
    fn eval_await_followup_calls(
        &mut self, frame: &Activation, calls: FollowupCalls, mut value: Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let environment = frame.environment()?;
        if calls.count == 1 {
            return self.eval_resumed_expression(&frame.program, calls.outer, value, environment, &frame.this, host);
        }
        let bytes = calls.count.saturating_mul(std::mem::size_of::<ExprId>());
        self.await_storage(frame.retained_bytes().saturating_add(bytes), bytes)?;
        let mut suffix = Vec::new();
        suffix.try_reserve_exact(calls.count)
            .map_err(|_| Fault::new("Unable to allocate bounded async call chain."))?;
        let mut expression = calls.outer;
        for _ in 0..calls.count {
            suffix.push(expression);
            expression = followup_receiver(&frame.program, expression).expect("compiled async call chain");
        }
        for expression in suffix.into_iter().rev() {
            let marker = self.transient_marker();
            let result = self.push_transient_value(&value).and_then(|()| {
                self.eval_resumed_expression(&frame.program, expression, value, environment, &frame.this, host)
            });
            self.restore_transient_marker(marker);
            value = result?;
        }
        Ok(value)
    }

    #[inline(never)]
    fn prepare_await_literal(
        &mut self,
        frame: &Activation,
        site: LiteralAwait,
        retained: usize,
        host: &mut dyn Host,
    ) -> Result<Box<PendingLiteral>, Fault> {
        self.step()?;
        let expression = &frame.program.exprs[site.expression].kind;
        let object = self.new_object(matches!(expression, ExprKind::Array(_)).then_some(0))?;
        self.push_transient_root(Edge::Object(object))?;
        let environment = frame.environment()?;
        let target = match expression {
            ExprKind::Object(members) => {
                self.eval_object_literal_members(
                    &frame.program, object, &members[..site.member], environment, &frame.this, host,
                )?;
                let object_literals::Member::Property { key, .. } = members[site.member] else {
                    unreachable!("compiled async literal property");
                };
                let key = self.eval(&frame.program, key, environment, &frame.this, host)?;
                self.push_transient_value(&key)?;
                PendingLiteralTarget::Property(self.property_key(key, host)?)
            }
            ExprKind::Array(elements) => {
                let index = self.eval_array_literal_elements(
                    &frame.program, object, 0, &elements[..site.member], environment, &frame.this, host,
                )?;
                self.step()?;
                self.check_array_literal_index(index)?;
                PendingLiteralTarget::Element(index)
            }
            _ => unreachable!("compiled async literal"),
        };
        let literal = PendingLiteral { object, site, target };
        let bytes = literal.retained_bytes();
        self.await_storage(retained.saturating_add(bytes), bytes)?;
        Ok(Box::new(literal))
    }

    #[inline(never)]
    fn finish_await_literal(
        &mut self, frame: &Activation, literal: PendingLiteral, value: Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let PendingLiteral { object, site, target } = literal;
        let environment = frame.environment()?;
        match (&frame.program.exprs[site.expression].kind, target) {
            (ExprKind::Object(members), PendingLiteralTarget::Property(key)) => {
                self.define_literal_member(object, key, value, object_literals::Kind::Data)?;
                self.eval_object_literal_members(
                    &frame.program, object, &members[site.member + 1..], environment, &frame.this, host,
                )?;
            }
            (ExprKind::Array(elements), PendingLiteralTarget::Element(index)) => {
                self.define_array_literal_element(object, index, value)?;
                let length = self.eval_array_literal_elements(
                    &frame.program, object, index + 1, &elements[site.member + 1..], environment, &frame.this, host,
                )?;
                self.set_array_length(object, length)?;
            }
            _ => unreachable!("compiled async literal target"),
        }
        Ok(Value::Object(object))
    }

    #[inline(never)]
    fn prepare_await_assignment(
        &mut self,
        frame: &Activation,
        target: ExprId,
        host: &mut dyn Host,
    ) -> Result<Reference, Fault> {
        let reference = self.reference(
            &frame.program, target, frame.environment()?, &frame.this, host,
        )?;
        let bytes = reference.assignment_bytes();
        self.await_storage(frame.retained_bytes().saturating_add(bytes), bytes)?;
        self.push_transient_reference(&reference)?;
        Ok(reference)
    }

    #[inline(never)]
    fn prepare_await_method(
        &mut self,
        frame: &Activation,
        site: ArgumentAwait,
        retained: usize,
        host: &mut dyn Host,
    ) -> Result<Box<PendingMethod>, Fault> {
        let ExprKind::Call(method, arguments) = &frame.program.exprs[site.call].kind else {
            unreachable!("prepared method needs a call expression");
        };
        let ExprKind::Member(receiver, key) = frame.program.exprs[*method].kind else {
            unreachable!("suspended member call needs a member expression");
        };
        let environment = frame.environment()?;
        let receiver = self.eval(&frame.program, receiver, environment, &frame.this, host)?;
        self.push_transient_value(&receiver)?;
        let key = self.eval(&frame.program, key, environment, &frame.this, host)?;
        self.push_transient_value(&key)?;
        let (receiver, key, function) = self.resolve_member_call(receiver, key, host)?;
        if let Some(function) = &function {
            self.push_transient_value(function)?;
        }
        let prefix = self.eval_await_argument_prefix(
            &frame.program, &arguments[..site.index], environment, &frame.this, host,
        )?;
        let method = PendingMethod { call: site.call, index: site.index, receiver, key, function, prefix };
        let bytes = method.retained_bytes();
        self.await_storage(retained.saturating_add(bytes), bytes)?;
        Ok(Box::new(method))
    }

    fn await_resume_action(
        &mut self,
        frame: &mut Activation,
        action: Resume,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<Progress, Fault> {
        match action {
            Resume::Discard => Ok(Progress::Continue),
            Resume::Return => Ok(Progress::Abrupt(Control::Return(value))),
            Resume::StoreExpressionValue => {
                debug_assert!(frame.expression_value.is_none());
                frame.expression_value = Some(value);
                Ok(Progress::Continue)
            }
            Resume::BranchFalse(target) => {
                if !value.truthy() {
                    frame.next = target;
                }
                Ok(Progress::Continue)
            }
            Resume::Initialize { kind, pattern } => {
                let environment = frame.environment()?;
                let mode = self.declaration_mode(kind, environment)?;
                self.initialize_pattern(
                    &frame.program,
                    pattern,
                    value,
                    mode,
                    environment,
                    &frame.this,
                    host,
                )?;
                Ok(Progress::Continue)
            }
        }
    }

    fn await_control(control: Control) -> Progress {
        match control {
            Control::Normal(_) => Progress::Continue,
            abrupt => Progress::Abrupt(abrupt),
        }
    }

    #[inline(never)]
    fn reserve_await_exit_scope(&mut self, frame: &mut Activation) -> Result<(), Fault> {
        if frame.exit_scopes.len() == frame.exit_scopes.capacity() {
            let retained = frame.retained_bytes();
            self.grow_await_scope_stack(&mut frame.exit_scopes, retained)?;
        }
        Ok(())
    }

    #[inline(never)]
    fn enter_await_switch(
        &mut self, frame: &mut Activation, statement: StmtId, leave: usize, environment: usize, host: &mut dyn Host,
    ) -> Result<Progress, Fault> {
        let program = Rc::clone(&frame.program);
        let StmtKind::Switch { discriminant, cases, body } = &program.stmts[statement].kind else {
            return Err(Fault::new("Invalid async switch instruction."));
        };
        let value = self.eval(&program, *discriminant, environment, &frame.this, host)?;
        self.push_transient_value(&value)?;
        self.reserve_await_environment(frame)?;
        self.reserve_await_exit_scope(frame)?;
        let scope = self.new_environment(environment, false)?;
        frame.exit_scopes.push(ExitScope::Switch {
            statement, after: leave + 1, environments: frame.environments.len(),
        });
        frame.environments.push(scope);
        self.push_transient_root(Edge::Environment(scope))?;
        self.prepare(&program, body, scope, false, host)?;
        let selected = self.select_switch_case(&program, cases, &value, scope, &frame.this, host)?;
        frame.next = selected.map_or(leave, |index| frame.next + index);
        Ok(Progress::Continue)
    }

    #[inline(never)]
    fn enter_await_classic_for(
        &mut self, frame: &mut Activation, statement: StmtId, advance: usize, after: usize, host: &mut dyn Host,
    ) -> Result<Progress, Fault> {
        let StmtKind::For(initial, _, _, _) = frame.program.stmts[statement].kind else {
            unreachable!("compiled async classic loop");
        };
        self.reserve_await_exit_scope(frame)?;
        let environments = frame.environments.len();
        frame.exit_scopes.push(ExitScope::ClassicFor(ClassicForScope {
            statement, advance, after, environments,
            iteration_depth: environments + usize::from(initial.is_some()),
            bindings: Vec::new(),
        }));
        if let Some(initial) = initial {
            self.reserve_await_environment(frame)?;
            let environment = self.new_environment(frame.environment()?, false)?;
            frame.environments.push(environment);
            self.push_transient_root(Edge::Environment(environment))?;
            let bindings = self.initialize_classic_for(&frame.program, initial, environment, &frame.this, host)?;
            let bytes = classic_for_binding_bytes(&bindings);
            self.await_storage(frame.retained_bytes().saturating_add(bytes), bytes)?;
            let Some(ExitScope::ClassicFor(scope)) = frame.exit_scopes.last_mut() else {
                unreachable!("compiled async classic loop scope");
            };
            scope.bindings = bindings;
            self.refresh_await_for_bindings(frame)?;
        }
        Ok(Progress::Continue)
    }

    fn refresh_await_for_bindings(&mut self, frame: &mut Activation) -> Result<(), Fault> {
        let Some(ExitScope::ClassicFor(scope)) = frame.exit_scopes.last() else {
            return Err(Fault::new("Invalid async for iteration."));
        };
        if !scope.bindings.is_empty() {
            let slot = scope.environments;
            let old = frame.environments[slot];
            let parent = frame.environments[slot - 1];
            frame.environments[slot] = self.iteration_environment(old, parent, &scope.bindings)?;
        }
        Ok(())
    }

    #[inline(never)]
    fn enter_await_for_of(
        &mut self, frame: &mut Activation, statement: StmtId, after: usize, environment: usize, host: &mut dyn Host,
    ) -> Result<Progress, Fault> {
        let Some((kind, pattern, expression)) = async_plan::lexical_for_of(&frame.program, statement) else {
            return Err(Fault::new("Invalid async for-of binding."));
        };
        self.reserve_await_exit_scope(frame)?;
        let scope = self.new_lexical_iteration_scope(&frame.program, environment, kind, pattern)?;
        let marker = self.root_marker();
        let source = (|| {
            self.push_transient_root(Edge::Environment(scope))?;
            self.eval(&frame.program, expression, scope, &frame.this, host)
        })();
        self.restore_root_marker(marker);
        self.release_environment(scope);
        let source = source?;
        self.push_transient_value(&source)?;
        let asynchronous = matches!(frame.program.stmts[statement].kind, StmtKind::ForOf(_, _, _, true));
        let record = if asynchronous {
            let method = self.async_iterator_method(&source, host)?
                .ok_or_else(|| Fault::new("for await over synchronous iterables is not implemented."))?;
            self.push_transient_value(&method)?;
            self.iterator_from_method(source, method, host)?
        } else {
            let LoopIterator::Values(record) = self.loop_iterator(source, true, host)? else {
                unreachable!("for-of requires a value iterator");
            };
            record
        };
        let bytes = std::mem::size_of::<ForOfScope>().saturating_add(record.retained_bytes());
        self.await_storage(frame.retained_bytes().saturating_add(bytes), bytes)?;
        frame.exit_scopes.push(ExitScope::ForOf(Box::new(ForOfScope {
            record, asynchronous, environments: frame.environments.len(), statement, next: frame.next, after,
        })));
        Ok(Progress::Continue)
    }

    #[inline(never)]
    fn next_await_for_of(
        &mut self, frame: &mut Activation, statement: StmtId, environment: usize, host: &mut dyn Host,
    ) -> Result<Progress, Fault> {
        let Some(ExitScope::ForOf(iterator)) = frame.exit_scopes.last_mut() else {
            return Err(Fault::new("Async for-of has no active iterator."));
        };
        if iterator.asynchronous {
            // A rejected step or abrupt result read does not close the iterator.
            // A successfully read value re-enables closing before binding it.
            iterator.record.done = true;
            let value = self.iterator_next_result(&iterator.record, host)?;
            self.push_transient_value(&value)?;
            let promise = self.promise_for_await(value, host)?;
            frame.iterator_wait = Some(IteratorWait::Next(statement));
            return Ok(Progress::Suspend(promise));
        }
        let Some(value) = self.iterator_next(&mut iterator.record, host)? else {
            frame.next = iterator.after;
            frame.exit_scopes.pop();
            return Ok(Progress::Continue);
        };
        self.bind_await_iteration(frame, statement, environment, value, host)
    }

    fn bind_await_iteration(
        &mut self, frame: &mut Activation, statement: StmtId, environment: usize,
        value: Value, host: &mut dyn Host,
    ) -> Result<Progress, Fault> {
        self.step()?;
        self.push_transient_value(&value)?;
        let Some((kind, pattern, _)) = async_plan::lexical_for_of(&frame.program, statement) else {
            return Err(Fault::new("Invalid async for-of binding."));
        };
        self.reserve_await_environment(frame)?;
        let scope = self.new_lexical_iteration_scope(&frame.program, environment, kind, pattern)?;
        frame.environments.push(scope);
        self.push_transient_root(Edge::Environment(scope))?;
        self.initialize_pattern(
            &frame.program, pattern, value, binding_patterns::Mode::Initialize(scope), scope, &frame.this, host,
        )?;
        Ok(Progress::Continue)
    }

    fn resume_iterator_wait(
        &mut self, frame: &mut Activation, wait: IteratorWait, value: Value, rejected: bool, host: &mut dyn Host,
    ) -> Result<Progress, Fault> {
        match wait {
            IteratorWait::Next(statement) => {
                let next = (|| {
                    if rejected { return Err(Fault::thrown(value)); }
                    let Some(ExitScope::ForOf(iterator)) = frame.exit_scopes.last_mut() else {
                        return Err(Fault::new("No iterator for the resumed step."));
                    };
                    let Some(value) = self.iterator_result_value(&value, host)? else {
                        frame.next = iterator.after;
                        frame.exit_scopes.pop();
                        return Ok(Progress::Continue);
                    };
                    iterator.record.done = false;
                    self.bind_await_iteration(frame, statement, frame.environment()?, value, host)
                })();
                next.map_err(|error| error.at(&frame.program.source, frame.program.stmts[statement].at))
            }
            IteratorWait::Close { completion, after_break } => {
                if let FinallyCompletion::Throw(error) = *completion {
                    return Err(error);
                }
                if rejected { return Err(Fault::thrown(value)); }
                if !matches!(value, Value::Object(_) | Value::Function(_) | Value::Host(_)) {
                    return Err(Fault::exception("TypeError", "Async iterator return must resolve to an object."));
                }
                if let Some(after) = after_break {
                    frame.next = after;
                    return Ok(Progress::Continue);
                }
                let FinallyCompletion::Abrupt(control) = *completion else { unreachable!() };
                Ok(Progress::Abrupt(control))
            }
        }
    }

    #[inline(never)]
    fn finish_await_try_body(
        &mut self, frame: &mut Activation, environment: usize, host: &mut dyn Host,
    ) -> Result<Progress, Fault> {
        let Some(ExitScope::Try(scope)) = frame.exit_scopes.pop() else {
            return Err(Fault::new("Invalid async try exit."));
        };
        let TryPhase::Body { finally_entry, .. } = scope.phase else {
            return Err(Fault::new("Invalid async try body exit."));
        };
        if let Some(entry) = finally_entry {
            return self.begin_await_finally(frame, scope, entry, Ok(Control::Normal(None)));
        }
        frame.next = scope.after;
        let completion = self.finish_try_statement(
            &frame.program, scope.statement, Ok(Control::Normal(None)),
            environment, &frame.this, host,
        )?;
        Ok(Self::await_control(completion))
    }

    #[inline(never)]
    fn begin_await_finally(
        &mut self,
        frame: &mut Activation,
        mut scope: TryScope,
        entry: usize,
        completion: Result<Control, Fault>,
    ) -> Result<Progress, Fault> {
        let pending = match completion {
            Ok(Control::Normal(_)) => None,
            Ok(abrupt) => Some(FinallyCompletion::Abrupt(abrupt)),
            Err(error) => Some(FinallyCompletion::Throw(error)),
        };
        let pending = if let Some(pending) = pending {
            let bytes = pending.retained_bytes();
            self.await_storage(frame.retained_bytes().saturating_add(bytes), bytes)?;
            Some(Box::new(pending))
        } else {
            None
        };
        scope.phase = TryPhase::Finally(pending);
        // The scope just popped for this try owns the same stack slot.
        frame.exit_scopes.push(ExitScope::Try(scope));
        frame.next = entry;
        Ok(Progress::Continue)
    }

    #[inline(never)]
    fn finish_await_finally(frame: &mut Activation) -> Result<Progress, Fault> {
        let Some(ExitScope::Try(scope)) = frame.exit_scopes.pop() else {
            return Err(Fault::new("Invalid async finally exit."));
        };
        let TryPhase::Finally(completion) = scope.phase else {
            return Err(Fault::new("Invalid async finally body exit."));
        };
        frame.next = scope.after;
        match completion.map(|completion| *completion) {
            None => Ok(Progress::Continue),
            Some(FinallyCompletion::Abrupt(control)) => Ok(Progress::Abrupt(control)),
            Some(FinallyCompletion::Throw(error)) => Err(error),
        }
    }

    fn reserve_await_environment(&mut self, frame: &mut Activation) -> Result<(), Fault> {
        if frame.environments.len() == frame.environments.capacity() {
            let retained = frame.retained_bytes();
            self.grow_await_scope_stack(&mut frame.environments, retained)?;
        }
        Ok(())
    }

    fn grow_await_scope_stack<T>(&mut self, stack: &mut Vec<T>, retained: usize) -> Result<(), Fault> {
        let slot = std::mem::size_of::<T>();
        let before = stack.capacity();
        let occupied = self.value_bytes - self.active_await_storage_bytes;
        let spare = self.limits.heap.retained_values.saturating_sub(occupied).saturating_sub(retained);
        self.check_await_storage(retained.saturating_add(slot), occupied)?;
        self.work_as((before + 1).saturating_mul(slot), WorkKind::ArgumentCapture)?;
        let added = limits::grow_suspension_stack(stack, spare)?;
        self.await_storage(retained.saturating_add(added), 0)
    }

    fn unwind_await_environments(&mut self, frame: &mut Activation, depth: usize) {
        while frame.environments.len() > depth {
            let scope = frame.environments.pop().expect("owned lexical environment");
            self.release_environment(scope);
        }
    }

    fn capture_await_environment(&mut self, mut id: usize) -> Result<(), Fault> {
        loop {
            self.step()?;
            let environment = self.environment_mut(id);
            if environment.captured {
                return Ok(());
            }
            environment.captured = true;
            match environment.parent {
                Some(parent) => id = parent,
                None => return Ok(()),
            }
        }
    }

    fn unwind_await_exit_scopes(
        &mut self,
        frame: &mut Activation,
        mut completion: Result<Control, Fault>,
        host: &mut dyn Host,
    ) -> Result<Progress, Fault> {
        while !completion.as_ref().is_err_and(|error| error.exception.is_none()) {
            let Some(scope) = frame.exit_scopes.pop() else {
                break;
            };
            let scope = match scope {
                ExitScope::Try(scope) => scope,
                ExitScope::Switch { statement, after, environments } => {
                    self.unwind_await_environments(frame, environments);
                    if matches!(completion, Ok(Control::Break(target, _)) if target == statement) {
                        frame.next = after;
                        return Ok(Progress::Continue);
                    }
                    continue;
                }
                ExitScope::ClassicFor(scope) => {
                    if matches!(completion, Ok(Control::Continue(target, _)) if target == scope.statement) {
                        self.unwind_await_environments(frame, scope.iteration_depth);
                        frame.next = scope.advance;
                        frame.exit_scopes.push(ExitScope::ClassicFor(scope));
                        return Ok(Progress::Continue);
                    }
                    self.unwind_await_environments(frame, scope.environments);
                    if matches!(completion, Ok(Control::Break(target, _)) if target == scope.statement) {
                        frame.next = scope.after;
                        return Ok(Progress::Continue);
                    }
                    continue;
                }
                ExitScope::ForOf(mut iterator) => {
                    self.unwind_await_environments(frame, iterator.environments);
                    if matches!(completion, Ok(Control::Continue(target, _)) if target == iterator.statement) {
                        frame.next = iterator.next;
                        frame.exit_scopes.push(ExitScope::ForOf(iterator));
                        return Ok(Progress::Continue);
                    }
                    let break_here = matches!(completion, Ok(Control::Break(target, _)) if target == iterator.statement);
                    let marker = self.publish_await_activation(frame)?;
                    if iterator.asynchronous {
                        let close = (|| {
                            if iterator.record.done { return Ok(None); }
                            self.push_completion_roots(&completion)?;
                            let guard = self.inhibit_transient_collection();
                            let result = (|| {
                                let Some(value) = self.iterator_return_result(&iterator.record, host)? else {
                                    return Ok(None);
                                };
                                self.push_transient_value(&value)?;
                                self.promise_for_await(value, host).map(Some)
                            })();
                            self.restore_transient_marker(guard);
                            result
                        })();
                        self.restore_root_marker(marker);
                        match close {
                            Ok(Some(promise)) => {
                                let saved = match completion {
                                    Ok(control) => FinallyCompletion::Abrupt(control),
                                    Err(error) => FinallyCompletion::Throw(error),
                                };
                                let bytes = saved.retained_bytes();
                                self.await_storage(frame.retained_bytes().saturating_add(bytes), bytes)?;
                                frame.iterator_wait = Some(IteratorWait::Close {
                                    completion: Box::new(saved), after_break: break_here.then_some(iterator.after),
                                });
                                return Ok(Progress::Suspend(promise));
                            }
                            Ok(None) => {}
                            Err(error) if error.exception.is_none() || completion.is_ok() => completion = Err(error),
                            Err(_) => {}
                        }
                    } else {
                        completion = self.iterator_close(&mut iterator.record, completion, host);
                        self.restore_root_marker(marker);
                    }
                    if break_here && completion.is_ok() {
                        frame.next = iterator.after;
                        return Ok(Progress::Continue);
                    }
                    continue;
                }
            };
            self.unwind_await_environments(frame, scope.environments);
            let TryPhase::Body { catch_entry, finally_entry } = scope.phase else {
                // An abrupt result from finally replaces its saved completion.
                continue;
            };
            if let Some(entry) = finally_entry {
                return self.begin_await_finally(frame, scope, entry, completion);
            }
            let marker = self.publish_await_activation(frame)?;
            let catch_entry = catch_entry.filter(|_| completion.is_err());
            completion = (|| {
                self.push_completion_roots(&completion)?;
                if let Some(entry) = catch_entry {
                    let StmtKind::Try(_, Some((binding, _)), None) = frame.program.stmts[scope.statement].kind else {
                        unreachable!("compiled catch needs a try without its own finally");
                    };
                    let Err(error) = &mut completion else { unreachable!("compiled catch needs an exception") };
                    self.reserve_await_environment(frame)?;
                    let environment = self.enter_catch_scope(
                        &frame.program, binding, error, frame.environment()?, &frame.this, host,
                    )?;
                    frame.environments.push(environment);
                    frame.next = entry;
                    return Ok(Control::Normal(None));
                }
                self.finish_try_statement(
                    &frame.program, scope.statement, completion,
                    frame.environment()?, &frame.this, host,
                )
            })();
            self.restore_root_marker(marker);
            if matches!(completion, Ok(Control::Normal(_))) {
                if catch_entry.is_none() {
                    frame.next = scope.after;
                }
                break;
            }
        }
        match completion {
            Ok(Control::Break(..) | Control::Continue(..)) => {
                Err(Fault::new("Unexpected control completion across an await boundary."))
            }
            other => other.map(Self::await_control),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn await_activation_structural_roots_and_storage_cover_owned_state() {
        let program = Parser::parse(
            "async function original(p){let value=await p;return value;}",
            &mut MAX_REGEXP_WORK.clone(),
        )
        .unwrap();
        let mut frame = Activation {
            program: Rc::new(program),
            template: 0,
            callee: 11,
            outer: 12,
            this: Value::Function(13),
            realm: Some(2),
            invocation: Some(InvocationContext { environment: 14 }),
            native_caller: Some(Some(77)),
            strict: true,
            next: 1,
            environments: vec![15, 16],
            exit_scopes: vec![ExitScope::Try(TryScope {
                statement: 0,
                after: 2,
                environments: 1,
                phase: TryPhase::Finally(Some(Box::new(FinallyCompletion::Abrupt(
                    Control::Return(Value::Function(28)),
                )))),
            })],
            expression_value: Some(Value::Function(21)),
            comparison_operands: vec![Value::Function(29), Value::String("left".into())],
            assignments: vec![Reference::ComputedProperty(Value::Object(25), Value::Function(26))],
            plain_calls: Vec::new(),
            iterator_wait: None,
            pending: Some(PendingAwait {
                statement: 0,
                action: Resume::Discard,
                call: Some(PendingCall { function: Value::Function(17), direct_eval: false }),
                argument_call: None,
                method: Some(Box::new(PendingMethod {
                    call: 0,
                    index: 0,
                    receiver: Value::Function(19),
                    key: PropertyKey::String("invoke".into()),
                    function: Some(Value::Function(20)),
                    prefix: Vec::new(),
                })),
                followup_call: None,
                member_key: None,
                chain: None,
                literal: Some(Box::new(PendingLiteral {
                    object: 27,
                    target: PendingLiteralTarget::Property(PropertyKey::String("entry".into())),
                    site: LiteralAwait { expression: 0, member: 0 },
                })),
                negations: 0,
                assignment: Some(Reference::Binding(18, "target".into())),
            }),
        };
        let mut edges = Vec::new();
        frame.reachability_edges(&mut |edge| edges.push(edge));
        for function in [11, 13, 17, 19, 20, 21, 26, 28, 29] {
            assert!(edges
                .iter()
                .any(|edge| matches!(edge, Edge::Function(id) if *id == function)));
        }
        assert!(edges.iter().any(|edge| matches!(edge, Edge::Object(12))));
        assert!(edges.iter().any(|edge| matches!(edge, Edge::Object(25))));
        assert!(edges.iter().any(|edge| matches!(edge, Edge::Object(27))));
        for environment in [14, 15, 16, 18] {
            assert!(edges
                .iter()
                .any(|edge| matches!(edge, Edge::Environment(id) if *id == environment)));
        }
        let mut environments = Vec::new();
        frame.environment_roots(&mut environments);
        assert_eq!(environments, [15, 16, 14, 18]);
        assert_eq!(frame.realm(), Some(2));
        assert!(frame.reachability_cost() >= edges.len());
        assert_eq!(
            frame.retained_bytes(),
            std::mem::size_of::<Activation>()
                + frame.this.bytes()
                + "target".len()
                + std::mem::size_of::<PendingMethod>() + "invoke".len()
                + std::mem::size_of::<PendingLiteral>() + "entry".len()
                + std::mem::size_of::<FinallyCompletion>()
                + frame.comparison_operands.capacity() * std::mem::size_of::<Value>() + "left".len()
                + frame.assignments.capacity() * std::mem::size_of::<Reference>()
                + frame.environments.capacity() * std::mem::size_of::<usize>()
                + frame.exit_scopes.capacity() * std::mem::size_of::<ExitScope>()
        );
        frame.pending.as_mut().unwrap().assignment = Some(Reference::ComputedProperty(
            Value::Object(22), Value::Function(23),
        ));
        edges.clear();
        frame.reachability_edges(&mut |edge| edges.push(edge));
        assert!(edges.iter().any(|edge| matches!(edge, Edge::Object(22))));
        assert!(edges.iter().any(|edge| matches!(edge, Edge::Function(23))));
        assert!(frame.reachability_cost() >= edges.len());
        assert_eq!(Reference::ComputedProperty(
            Value::String("base".into()), Value::String("key".into()),
        ).assignment_bytes(), 7);
    }
}
