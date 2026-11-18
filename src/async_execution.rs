//! Owned await activations for the parser's finite statement plan. A resumed
//! instruction never evaluates the prefix again. Synchronous subtrees and catch
//! bodies use the ordinary evaluator, including its native/DOM access checks.
use super::async_plan::{Instruction, Resume};
use super::function_reachability::Edge;
use super::*;

#[derive(Clone, Copy)]
struct Catch {
    statement: StmtId,
    after: usize,
    environments: usize,
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
    catches: Vec<Catch>,
    pending: Option<(StmtId, Resume)>,
}

impl Activation {
    pub(super) fn realm(&self) -> Option<usize> {
        self.realm
    }

    pub(super) fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.this.bytes())
            .saturating_add(
                self.environments
                    .capacity()
                    .saturating_mul(std::mem::size_of::<usize>()),
            )
            .saturating_add(
                self.catches
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Catch>()),
            )
    }

    pub(super) fn reachability_cost(&self) -> usize {
        // Include primitive slots as a conservative, constant-size traversal
        // charge. Program storage is already charged and retained by callee.
        self.environments
            .len()
            .saturating_add(self.catches.len())
            .saturating_add(5)
    }

    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(Edge)) {
        add(Edge::Function(self.callee));
        add(Edge::Object(self.outer));
        Edge::value(&self.this, add);
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
    }

    fn environment(&self) -> Result<usize, Fault> {
        self.environments
            .last()
            .copied()
            .ok_or_else(|| Fault::new("Missing async lexical environment."))
    }
}

enum Progress {
    Continue,
    Return(Value),
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
        self.await_storage(bytes, bytes.saturating_sub(this.bytes()))?;
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
            catches: Vec::new(),
            pending: None,
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
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Function call limit reached (32)."));
        }
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

    fn await_storage(&mut self, retained: usize, work: usize) -> Result<(), Fault> {
        if retained
            > self
                .limits
                .heap
                .retained_values
                .saturating_sub(self.value_bytes)
        {
            return Err(Fault::new(format!(
                "Retained value limit reached ({} bytes).",
                self.limits.heap.retained_values
            )));
        }
        self.work_as(work, WorkKind::ArgumentCapture)
    }

    fn publish_await_activation(
        &mut self,
        frame: &Activation,
    ) -> Result<transient_roots::RootMarker, Fault> {
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
        mut frame: Box<Activation>,
        mut resumed: Option<(Value, bool)>,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        loop {
            let result = self.await_instruction(&mut frame, resumed.take(), host);
            let result = match result {
                Err(error) => self.await_catch(&mut frame, error, host),
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
                        Ok(Progress::Return(value)) => self
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
                let (statement, action) = frame
                    .pending
                    .take()
                    .ok_or_else(|| Fault::new("Async activation has no pending await."))?;
                let result = if reject {
                    self.work(value.bytes())?;
                    Err(Fault::thrown(value))
                } else {
                    self.await_resume_action(frame, action, value, host)
                };
                return result.map_err(|error| {
                    error.at(&frame.program.source, frame.program.stmts[statement].at)
                });
            }
            let code = &frame.program.templates[frame.template].await_plan;
            let Some(instruction) = code.get(frame.next).copied() else {
                return Ok(Progress::Return(Value::Undefined));
            };
            frame.next += 1;
            let environment = frame.environment()?;
            match instruction {
                Instruction::Statement(statement) => {
                    let control =
                        self.statement(&frame.program, statement, environment, &frame.this, host)?;
                    Self::await_control(control)
                }
                Instruction::EnterBlock(statement) => {
                    if frame.environments.len() >= MAX_EVAL_DEPTH {
                        return Err(Fault::new("Async lexical nesting limit reached (128)."));
                    }
                    if frame.environments.len() == frame.environments.capacity() {
                        self.await_storage(
                            frame
                                .retained_bytes()
                                .saturating_add(std::mem::size_of::<usize>()),
                            (frame.environments.len() + 1)
                                .saturating_mul(std::mem::size_of::<usize>()),
                        )?;
                        frame.environments.try_reserve_exact(1).map_err(|_| {
                            Fault::new("Unable to allocate bounded async lexical stack.")
                        })?;
                    }
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
                Instruction::Jump(target) => {
                    frame.next = target;
                    Ok(Progress::Continue)
                }
                Instruction::PushCatch { statement, after } => {
                    if frame.catches.len() >= MAX_EVAL_DEPTH {
                        return Err(Fault::new("Async catch nesting limit reached (128)."));
                    }
                    if frame.catches.len() == frame.catches.capacity() {
                        self.await_storage(
                            frame
                                .retained_bytes()
                                .saturating_add(std::mem::size_of::<Catch>()),
                            (frame.catches.len() + 1).saturating_mul(std::mem::size_of::<Catch>()),
                        )?;
                        frame.catches.try_reserve_exact(1).map_err(|_| {
                            Fault::new("Unable to allocate bounded async catch stack.")
                        })?;
                    }
                    frame.catches.push(Catch {
                        statement,
                        after,
                        environments: frame.environments.len(),
                    });
                    Ok(Progress::Continue)
                }
                Instruction::PopCatch => {
                    frame
                        .catches
                        .pop()
                        .ok_or_else(|| Fault::new("Invalid async catch exit."))?;
                    Ok(Progress::Continue)
                }
                Instruction::Await {
                    statement,
                    operand,
                    action,
                } => {
                    let result = (|| {
                        let value =
                            self.eval(&frame.program, operand, environment, &frame.this, host)?;
                        self.push_transient_value(&value)?;
                        let promise = self.promise_for_await(value, host)?;
                        frame.pending = Some((statement, action));
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

    fn await_resume_action(
        &mut self,
        frame: &Activation,
        action: Resume,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<Progress, Fault> {
        match action {
            Resume::Discard => Ok(Progress::Continue),
            Resume::Return => Ok(Progress::Return(value)),
            Resume::Initialize { kind, pattern } => {
                let environment = frame.environment()?;
                let mode = if kind == Declaration::Var {
                    binding_patterns::Mode::Var(self.var_environment(environment)?)
                } else {
                    binding_patterns::Mode::Initialize(environment)
                };
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

    fn await_control(control: Control) -> Result<Progress, Fault> {
        match control {
            Control::Normal(_) => Ok(Progress::Continue),
            Control::Return(value) => Ok(Progress::Return(value)),
            _ => Err(Fault::new(
                "Unexpected control completion across an await boundary.",
            )),
        }
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

    fn await_catch(
        &mut self,
        frame: &mut Activation,
        mut error: Fault,
        host: &mut dyn Host,
    ) -> Result<Progress, Fault> {
        loop {
            if error.exception.is_none() {
                return Err(error);
            }
            let Some(handler) = frame.catches.pop() else {
                return Err(error);
            };
            self.unwind_await_environments(frame, handler.environments);
            let marker = self.publish_await_activation(frame)?;
            let result = (|| {
                if let Some(Exception::Value(value)) = error.exception.as_deref() {
                    self.push_transient_value(value)?;
                }
                let StmtKind::Try(_, Some((binding, body)), None) =
                    frame.program.stmts[handler.statement].kind
                else {
                    return Err(Fault::new("Invalid async catch instruction."));
                };
                let control = self.catch_statement(
                    &frame.program,
                    binding,
                    body,
                    &mut error,
                    frame.environment()?,
                    &frame.this,
                    host,
                )?;
                Self::await_control(control)
            })();
            self.restore_root_marker(marker);
            match result {
                Ok(Progress::Continue) => {
                    frame.next = handler.after;
                    return Ok(Progress::Continue);
                }
                Err(next) => error = next,
                other => return other,
            }
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
        let frame = Activation {
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
            catches: vec![Catch {
                statement: 0,
                after: 2,
                environments: 1,
            }],
            pending: Some((0, Resume::Discard)),
        };
        let mut edges = Vec::new();
        frame.reachability_edges(&mut |edge| edges.push(edge));
        for function in [11, 13] {
            assert!(edges
                .iter()
                .any(|edge| matches!(edge, Edge::Function(id) if *id == function)));
        }
        assert!(edges.iter().any(|edge| matches!(edge, Edge::Object(12))));
        for environment in [14, 15, 16] {
            assert!(edges
                .iter()
                .any(|edge| matches!(edge, Edge::Environment(id) if *id == environment)));
        }
        let mut environments = Vec::new();
        frame.environment_roots(&mut environments);
        assert_eq!(environments, [15, 16, 14]);
        assert_eq!(frame.realm(), Some(2));
        assert!(frame.reachability_cost() >= edges.len());
        assert_eq!(
            frame.retained_bytes(),
            std::mem::size_of::<Activation>()
                + frame.this.bytes()
                + frame.environments.capacity() * std::mem::size_of::<usize>()
                + frame.catches.capacity() * std::mem::size_of::<Catch>()
        );
    }
}
