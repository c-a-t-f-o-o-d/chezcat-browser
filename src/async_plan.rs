//! Owned resumption points for a bounded set of async statement forms.
use super::*;
use std::ops::Range;

#[derive(Clone, Copy)]
pub(super) enum Resume {
    Discard,
    Return,
    Initialize { kind: Declaration, pattern: binding_patterns::Id },
    BranchFalse(usize),
}

#[derive(Clone, Copy)]
pub(super) enum Instruction {
    Statement(StmtId),
    Declare { statement: StmtId, index: usize },
    Evaluate { statement: StmtId, expression: ExprId, action: Resume },
    EnterBlock(StmtId),
    LeaveBlock,
    BranchFalse { statement: StmtId, condition: ExprId, target: usize },
    Jump(usize),
    PushTry { statement: StmtId, after: usize },
    FinishTry,
    GuardAwait { statement: StmtId, left: ExprId, operator: &'static str, action: Resume },
    Await {
        statement: StmtId,
        operand: ExprId,
        callee: Option<ExprId>,
        member_key: Option<ExprId>,
        action: Resume,
    },
}

fn storage_error() -> Fault {
    Fault::new("Unable to allocate bounded async statement plan.")
}

fn push<T>(items: &mut Vec<T>, value: T) -> Result<(), Fault> {
    items.try_reserve(1).map_err(|_| storage_error())?;
    items.push(value);
    Ok(())
}

fn statement_expression(program: &Program, id: StmtId) -> Option<(ExprId, Resume)> {
    Some(match &program.stmts[id].kind {
        StmtKind::Expr(expression) => (*expression, Resume::Discard),
        StmtKind::Return(Some(expression)) => (*expression, Resume::Return),
        _ => return None,
    })
}

struct AwaitSite {
    operand: ExprId,
    callee: Option<ExprId>,
    guard: Option<(ExprId, &'static str)>,
    member_key: Option<ExprId>,
}

fn await_expression(program: &Program, expression: ExprId) -> Option<AwaitSite> {
    let (expression, guard) = match program.exprs[expression].kind {
        ExprKind::Binary(operator @ ("&&" | "||" | "??"), left, right) => {
            (right, Some((left, operator)))
        }
        _ => (expression, None),
    };
    let (expression, member_key) = match program.exprs[expression].kind {
        ExprKind::Member(receiver, key) => (receiver, Some(key)),
        _ => (expression, None),
    };
    let (expression, callee) = match &program.exprs[expression].kind {
        ExprKind::Call(callee, arguments) if matches!(&program.exprs[*callee].kind, ExprKind::Name(_)) => {
            let [Argument { expression, spread: false }] = arguments.as_slice() else { return None };
            (*expression, Some(*callee))
        }
        _ => (expression, None),
    };
    let ExprKind::Unary("await", operand) = program.exprs[expression].kind else { return None };
    Some(AwaitSite { operand, callee, guard, member_key })
}

fn visit_sequence(
    program: &Program,
    mut expression: ExprId,
    mut visit: impl FnMut(ExprId, bool) -> Result<(), Fault>,
) -> Result<(), Fault> {
    let mut pending = Vec::new();
    loop {
        if let ExprKind::Sequence(left, right) = program.exprs[expression].kind {
            push(&mut pending, right)?;
            expression = left;
        } else {
            visit(expression, pending.is_empty())?;
            let Some(next) = pending.pop() else { return Ok(()) };
            expression = next;
        }
    }
}

fn expression_size(program: &Program, expression: ExprId) -> Result<(usize, usize), Fault> {
    let (mut instructions, mut sites) = (0, 0);
    visit_sequence(program, expression, |expression, _| {
        instructions += 1;
        if let Some(site) = await_expression(program, expression) {
            instructions += usize::from(site.guard.is_some());
            sites += 1;
        }
        Ok(())
    })?;
    Ok((instructions, sites))
}

fn statement_size(program: &Program, id: StmtId) -> Result<(usize, usize), Fault> {
    if let StmtKind::Declare(_, declarations) = &program.stmts[id].kind {
        let (mut instructions, mut sites) = (0, 0);
        for &(_, initial) in declarations {
            let (length, awaits) = initial.map(|expression| expression_size(program, expression))
                .transpose()?.unwrap_or((1, 0));
            instructions += if awaits == 0 { 1 } else { length };
            sites += awaits;
        }
        Ok((instructions, sites))
    } else if let Some((expression, _)) = statement_expression(program, id) {
        expression_size(program, expression)
    } else {
        Ok((1, 0))
    }
}

fn emit_expression(
    program: &Program, id: StmtId, expression: ExprId, action: Resume,
    code: &mut Vec<Instruction>,
) -> Result<(), Fault> {
    visit_sequence(program, expression, |expression, last| {
        let action = if last { action } else { Resume::Discard };
        let instruction = if let Some(AwaitSite { operand, callee, guard, member_key }) = await_expression(program, expression) {
            if let Some((left, operator)) = guard {
                // A successful short circuit skips this site's next instruction.
                push(code, Instruction::GuardAwait { statement: id, left, operator, action })?;
            }
            Instruction::Await { statement: id, operand, callee, member_key, action }
        } else {
            Instruction::Evaluate { statement: id, expression, action }
        };
        push(code, instruction)
    })
}

enum Visit {
    Statement { id: StmtId, eligible: bool },
    Finish { id: StmtId, before: usize, condition_extra: usize },
}

// Count await sites in supported statement and expression forms. Each
// statement belongs to one template. Ineligible subtrees still contribute
// jump targets, but never await sites; nested function bodies are separate
// roots. The parser's independent expression count catches *all* embedded
// awaits, including unsupported conditions, pattern defaults and unselected branches.
fn split_statements(program: &Program, template: &Template, expected: usize) -> Result<(HashSet<StmtId>, usize), Fault> {
    let mut pending = Vec::new();
    for &id in template.body.iter().rev() {
        push(&mut pending, Visit::Statement { id, eligible: true })?;
    }
    let mut split = HashSet::new();
    let mut jumps = Vec::new();
    let mut sites = 0;
    let mut instructions = template.body.len();
    while let Some(visit) = pending.pop() {
        let (id, eligible) = match visit {
            Visit::Finish { id, before, condition_extra } => {
                if sites > before {
                    split.try_reserve(1).map_err(|_| storage_error())?;
                    split.insert(id);
                    // Each statement already accounts for one instruction.
                    // Opening a container adds its children and control flow.
                    instructions += condition_extra + match &program.stmts[id].kind {
                        StmtKind::Block(body) => body.len() + 1,
                        StmtKind::If(_, _, Some(_)) => 3,
                        StmtKind::If(_, _, None) => 1,
                        StmtKind::Try(_, _, _) => 2,
                        _ => unreachable!(),
                    };
                }
                continue;
            }
            Visit::Statement { id, eligible } => (id, eligible),
        };
        let kind = &program.stmts[id].kind;
        if eligible {
            let (length, awaits) = statement_size(program, id)?;
            if awaits != 0 {
                instructions += length - 1;
                sites += awaits;
                split.try_reserve(1).map_err(|_| storage_error())?;
                split.insert(id);
                continue;
            }
        }
        let children_eligible = eligible && matches!(kind,
            StmtKind::Block(_) | StmtKind::If(_, _, _) | StmtKind::Try(_, _, _));
        if children_eligible {
            let before = sites;
            let mut condition_extra = 0;
            if let StmtKind::If(condition, _, _) = kind {
                let (length, awaits) = expression_size(program, *condition)?;
                if awaits != 0 {
                    sites += awaits;
                    condition_extra = length - 1;
                }
            }
            push(&mut pending, Visit::Finish { id, before, condition_extra })?;
        }
        let mut child = |id, eligible| push(&mut pending, Visit::Statement { id, eligible });
        match kind {
            StmtKind::Block(body) | StmtKind::Switch { body, .. } => {
                for &item in body.iter().rev() { child(item, children_eligible)?; }
            }
            StmtKind::If(_, yes, no) => {
                if let Some(no) = no { child(*no, children_eligible)?; }
                child(*yes, children_eligible)?;
            }
            StmtKind::Try(body, catch, finally) => {
                if let Some(finally) = finally { child(*finally, false)?; }
                if let Some((_, catch)) = catch { child(*catch, false)?; }
                child(*body, children_eligible)?;
            }
            StmtKind::While(_, body) | StmtKind::DoWhile(body, _) => child(*body, false)?,
            StmtKind::For(initial, _, _, body) => {
                if let Some(initial) = initial { child(*initial, false)?; }
                child(*body, false)?;
            }
            StmtKind::ForIn(initial, _, body) | StmtKind::ForOf(initial, _, body) => {
                child(*initial, false)?;
                child(*body, false)?;
            }
            StmtKind::Break(target) | StmtKind::Continue(target) => push(&mut jumps, (id, *target))?,
            _ => {}
        }
    }
    if sites != expected {
        return Err(Fault::new("Await in this expression or statement position is not implemented.")
            .at(&program.source, template.source_start));
    }
    // The ordinary statement evaluator owns jumps inside synchronous
    // subtrees. A label on a split ancestor no longer has that native frame.
    for (id, target) in jumps {
        if split.contains(&target) {
            return Err(Fault::new("A jump to a statement split by await is not implemented.")
                .at(&program.source, program.stmts[id].at));
        }
    }
    Ok((split, instructions))
}

enum Emit {
    Statement(StmtId),
    LeaveBlock,
    Else { branch: Range<usize>, alternative: StmtId },
    FinishBranch(Range<usize>),
    FinishJump(usize),
    FinishTry(usize),
}

fn set_branch_target(code: &mut [Instruction], destination: usize) {
    for instruction in code {
        match instruction {
            Instruction::BranchFalse { target, .. }
            | Instruction::Await { action: Resume::BranchFalse(target), .. }
            | Instruction::GuardAwait { action: Resume::BranchFalse(target), .. }
            | Instruction::Evaluate { action: Resume::BranchFalse(target), .. } => {
                *target = destination;
            }
            _ => {}
        }
    }
}

pub(super) fn compile(program: &Program, template: usize, expected_await_count: usize, ast_limit: usize) -> Result<Vec<Instruction>, Fault> {
    let template = &program.templates[template];
    let (split, instructions) = split_statements(program, template, expected_await_count)?;
    if instructions > ast_limit.saturating_sub(program.node_count()) {
        return Err(Fault::new(format!("AST node limit reached ({ast_limit}) compiling await plan."))
            .at(&program.source, template.source_start));
    }
    let mut code = Vec::new();
    code.try_reserve_exact(instructions).map_err(|_| storage_error())?;
    let mut pending = Vec::new();
    for &id in template.body.iter().rev() { push(&mut pending, Emit::Statement(id))?; }
    // Container statements emit bounded control instructions. A split comma
    // expression emits one instruction per leaf, within the same AST limit.
    while let Some(task) = pending.pop() {
        let id = match task {
            Emit::Statement(id) => id,
            Emit::LeaveBlock => { push(&mut code, Instruction::LeaveBlock)?; continue; }
            Emit::Else { branch, alternative } => {
                let jump = code.len();
                push(&mut code, Instruction::Jump(0))?;
                let start = code.len();
                set_branch_target(&mut code[branch], start);
                push(&mut pending, Emit::FinishJump(jump))?;
                push(&mut pending, Emit::Statement(alternative))?;
                continue;
            }
            Emit::FinishBranch(branch) => {
                let end = code.len();
                set_branch_target(&mut code[branch], end);
                continue;
            }
            Emit::FinishJump(jump) => {
                code[jump] = Instruction::Jump(code.len());
                continue;
            }
            Emit::FinishTry(handler) => {
                push(&mut code, Instruction::FinishTry)?;
                let end = code.len();
                let Instruction::PushTry { after, .. } = &mut code[handler] else { unreachable!() };
                *after = end;
                continue;
            }
        };
        if !split.contains(&id) {
            push(&mut code, Instruction::Statement(id))?;
        } else if let Some((expression, action)) = statement_expression(program, id) {
            emit_expression(program, id, expression, action, &mut code)?;
        } else {
            match &program.stmts[id].kind {
                StmtKind::Declare(kind, declarations) => {
                    for (index, &(pattern, initial)) in declarations.iter().enumerate() {
                        if let Some(expression) = initial {
                            if expression_size(program, expression)?.1 != 0 {
                                let action = Resume::Initialize { kind: *kind, pattern };
                                emit_expression(program, id, expression, action, &mut code)?;
                                continue;
                            }
                        }
                        push(&mut code, Instruction::Declare { statement: id, index })?;
                    }
                }
                StmtKind::Block(body) => {
                    push(&mut code, Instruction::EnterBlock(id))?;
                    push(&mut pending, Emit::LeaveBlock)?;
                    for &item in body.iter().rev() { push(&mut pending, Emit::Statement(item))?; }
                }
                StmtKind::If(condition, yes, no) => {
                    let start = code.len();
                    if expression_size(program, *condition)?.1 == 0 {
                        push(&mut code, Instruction::BranchFalse { statement: id, condition: *condition, target: 0 })?;
                    } else {
                        emit_expression(program, id, *condition, Resume::BranchFalse(0), &mut code)?;
                    }
                    let branch = start..code.len();
                    let next = match no {
                        Some(no) => Emit::Else { branch, alternative: *no },
                        None => Emit::FinishBranch(branch),
                    };
                    push(&mut pending, next)?;
                    push(&mut pending, Emit::Statement(*yes))?;
                }
                StmtKind::Try(body, _, _) => {
                    let handler = code.len();
                    push(&mut code, Instruction::PushTry { statement: id, after: 0 })?;
                    push(&mut pending, Emit::FinishTry(handler))?;
                    push(&mut pending, Emit::Statement(*body))?;
                }
                _ => unreachable!(),
            }
        }
    }
    debug_assert_eq!(code.len(), instructions);
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Result<Program, Fault> {
        Parser::parse(source, &mut MAX_REGEXP_WORK.clone())
    }

    #[test]
    fn async_plan_jumps_scope_exits_and_catches_point_to_owned_instructions() {
        let program = parse("async function go(flag,p){before();\
            if(flag){let x=await p;use(x);}else{await p;}\
            try{return await p;}catch(e){return handle(e);}}").unwrap();
        let code = &program.templates[0].await_plan;
        use Instruction::*;
        assert!(matches!(code.as_slice(), [
            Statement(_), BranchFalse { target: 7, .. },
            EnterBlock(_), Await { action: Resume::Initialize { kind: Declaration::Let, .. }, .. },
            Statement(_), LeaveBlock, Jump(10),
            EnterBlock(_), Await { action: Resume::Discard, .. }, LeaveBlock,
            PushTry { after: 15, .. }, EnterBlock(_), Await { action: Resume::Return, .. },
            LeaveBlock, FinishTry,
        ]));
    }

    #[test]
    fn async_plan_rejects_unreachable_embedded_await_and_cross_scope_jumps_during_parse() {
        for body in [
            "if(false){use(1,await p);}",
            "return receiver.method(await p);",
            "return (await p)[await p];",
            "return use(...await p);",
            "return (use(), 1 + await p);",
            "return (await p) && use(await p);",
            "if(1+await p){}",
            "return await p + 1;",
            "let a=await p,b=1+await p;",
            "let {value=await p}=await p;",
            "while(false){await p;}",
            "try{}catch(e){await p;}",
            "try{}finally{await p;}",
            "await (await p);",
        ] {
            let source = format!("async function go(p){{{body}}}");
            let error = parse(&source).err().expect("unsupported await was accepted");
            assert!(error.message.contains("Await in this expression or statement position"), "{body}: {}", error.display());
        }
        let source = "async function go(p){exit:{try{await p;}catch(e){break exit;}}}";
        let error = parse(source).err().expect("jump to split ancestor was accepted");
        assert!(error.message.contains("jump to a statement split by await"), "{}", error.display());
    }

    #[test]
    fn async_plan_leaves_local_jumps_and_nested_function_sites_independent() {
        let program = parse("async function outer(p){\
            async function inner(){return await p;}\
            local:{if(p)break local;}\
            try{await p;}catch(e){for(;;){break;}}\
        }").unwrap();
        for template in &program.templates {
            let code = &template.await_plan;
            assert_eq!(code.iter().filter(|op| matches!(op, Instruction::Await { .. })).count(), 1);
        }
        assert_eq!(program.templates.len(), 2);
    }

    #[test]
    fn async_plan_instructions_share_the_ast_capacity_and_retention_budget() {
        for (source, instructions) in [
            ("async function f(p){if(p){await p;}}", 4),
            ("async function f(p){return (before(), await p, after());}", 3),
            ("async function f(p){let a,b=await p,c=4;}", 3),
            ("async function f(p){return yes() && use(await p);}", 2),
            ("async function f(p){return yes() && use(await p).ready;}", 2),
            ("async function f(p){if(yes() && use(await p).ready){noop();}}", 3),
        ] {
            let mut runtime = Runtime::for_test();
            let program = runtime.register_program(source, false).unwrap();
            let required = program.node_count();
            assert_eq!(program.await_plan_nodes, instructions);
            assert_eq!(runtime.retained_ast, required);
            let parse_at = |ast_nodes| {
                let capacity = limits::Capacities { ast_nodes, ..limits::REGRESSION_CAPACITIES };
                Parser::parse_context_with_capacity(source, &mut MAX_REGEXP_WORK.clone(), false, 0, capacity, limits::REGRESSION_HEAP)
            };
            assert_eq!(parse_at(required).unwrap().node_count(), required);
            let error = parse_at(required - 1).err().unwrap();
            assert!(error.message.contains("compiling await plan"), "{}", error.display());
        }
    }
}
