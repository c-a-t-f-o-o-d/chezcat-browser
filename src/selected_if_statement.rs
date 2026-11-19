//! Selected-if continuations kept out of ordinary recursive statement frames.
use super::*;

impl Runtime {
    #[inline(never)]
    pub(super) fn selected_if_statement(
        &mut self,
        program: &Rc<Program>,
        id: StmtId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Control, Fault> {
        let mut current = id;
        // Selected if branches need only target IDs and UpdateEmpty(undefined)
        // after their child returns. Keep that bounded metadata on the heap and
        // enter the child in this actual activation, with its original step.
        let mut if_parents = Vec::new();
        loop {
            #[cfg(test)]
            let tail = !self.transient_roots.disable_if_statement_tail;
            #[cfg(not(test))]
            let tail = true;
            if tail {
                if let StmtKind::If(condition, yes, no) = program.stmts[current].kind {
                    let at = program.stmts[current].at;
                    let selected = self
                        .eval(program, condition, env, this, host)
                        .map_err(|error| error.at(&program.source, at))?
                        .truthy();
                    let branch = if selected { Some(yes) } else { no };
                    let Some(branch) = branch else {
                        return Ok(Control::Normal(Some(Value::Undefined)));
                    };
                    if if_parents.len() == MAX_IF_BRANCHES {
                        return Err(Fault::new("Pending if-branch limit reached (4096).")
                            .at(&program.source, at));
                    }
                    // Unlike a retained native child frame, this ID is a
                    // real heap copy and pays the finite copy allowance.
                    self.work_as(std::mem::size_of::<StmtId>(), WorkKind::ConditionalTarget)
                        .map_err(|error| error.at(&program.source, at))?;
                    if if_parents.len() == if_parents.capacity() {
                        let count = if_parents
                            .capacity()
                            .max(8)
                            .min(MAX_IF_BRANCHES - if_parents.len());
                        if_parents.try_reserve_exact(count).map_err(|_| {
                            Fault::new("Unable to allocate bounded if-branch storage.")
                                .at(&program.source, at)
                        })?;
                    }
                    if_parents.push(current);
                    // The existing flat false-if handoff locates its entry step
                    // at the next if; ordinary child entry failures belong to
                    // the containing if, before that child owns an activation.
                    let entry_at = if !selected
                        && matches!(program.stmts[branch].kind, StmtKind::If(_, _, _))
                    {
                        program.stmts[branch].at
                    } else {
                        at
                    };
                    self.step()
                        .map_err(|error| error.at(&program.source, entry_at))?;
                    current = branch;
                    continue;
                }
            }
            // Expr/Return reuse the owning statement activation for heap eval.
            let expression = match program.stmts[current].kind {
                StmtKind::Expr(expression) => Some((expression, false)),
                StmtKind::Return(Some(expression)) => Some((expression, true)),
                _ => None,
            };
            let result = if matches!(program.stmts[current].kind, StmtKind::Try(_, _, _)) {
                self.try_statement(program, current, env, this, host)
            } else if let Some((expression, returning)) = expression {
                self.step().and_then(|_| {
                    #[cfg(test)]
                    let value = if self.recursive_evaluation_oracle {
                        self.eval_inner(program, expression, env, this, host)
                    } else {
                        self.eval_heap(program, expression, env, this, host)
                    };
                    #[cfg(not(test))]
                    let value = self.eval_heap(program, expression, env, this, host);
                    value
                        .map_err(|error| error.at(&program.source, program.exprs[expression].at))
                        .map(|value| {
                            if returning {
                                Control::Return(value)
                            } else {
                                Control::Normal(Some(value))
                            }
                        })
                })
            } else {
                self.statement_inner(program, current, env, this, host)
            }
            .map(|control| match control {
                Control::Break(target, value) if target == current => Control::Normal(value),
                other => other,
            })
            .map_err(|error| error.at(&program.source, program.stmts[current].at));
            return result.map(|control| {
                if if_parents.is_empty() {
                    return control;
                }
                match control.update_empty(Some(Value::Undefined)) {
                    Control::Break(target, value) if if_parents.contains(&target) => {
                        Control::Normal(value)
                    }
                    other => other,
                }
            });
        }
    }
}
