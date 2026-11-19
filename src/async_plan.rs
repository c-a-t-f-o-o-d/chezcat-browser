//! Owned resumption points for bounded async and generator statement forms.
use super::*;
use std::ops::Range;

#[derive(Clone, Copy)]
pub(super) enum Resume {
    Discard,
    Return,
    Initialize { kind: Declaration, pattern: binding_patterns::Id },
    BranchFalse(usize),
    StoreExpressionValue,
}

#[derive(Clone, Copy)]
pub(super) enum Instruction {
    Statement(StmtId),
    Yield { statement: StmtId, call: Option<GeneratorCallYield> },
    Declare { statement: StmtId, index: usize },
    Evaluate { statement: StmtId, expression: ExprId, action: Resume },
    CaptureAssignment { statement: StmtId, target: ExprId },
    WriteAssignment { statement: StmtId, action: Resume },
    EnterBlock(StmtId),
    LeaveBlock,
    EnterSwitch { statement: StmtId, leave: usize },
    EnterForOf { statement: StmtId, after: usize },
    NextForOf(StmtId),
    BranchFalse { statement: StmtId, condition: ExprId, target: usize },
    BranchOnValueFalse(usize),
    Jump(usize),
    PushTry { statement: StmtId, after: usize, catch_entry: Option<usize>, finally_entry: Option<usize> },
    FinishTry,
    FinishFinally,
    Logical { statement: StmtId, operator: &'static str, target: usize, action: Resume },
    BeginLogicalNot(usize),
    FinishLogicalNot { statement: StmtId, negations: usize, action: Resume },
    CaptureComparison(StmtId),
    Comparison { statement: StmtId, operator: &'static str, action: Resume },
    ReadAccess { statement: StmtId, expression: ExprId, action: Resume },
    Await {
        statement: StmtId,
        operand: ExprId,
        callee: Option<ExprId>,
        method: Option<ExprId>,
        followup_call: Option<FollowupCalls>,
        member_key: Option<ExprId>,
        chain: Option<ExprId>,
        literal: Option<LiteralAwait>,
        negations: usize,
        assignment: Option<ExprId>,
        action: Resume,
    },
}

fn storage_error() -> Fault {
    Fault::new("Unable to allocate bounded resumption plan.")
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

pub(super) fn lexical_for_of(program: &Program, id: StmtId) -> Option<(Declaration, binding_patterns::Id, ExprId)> {
    let StmtKind::ForOf(initial, source, _) = program.stmts[id].kind else {
        return None;
    };
    let StmtKind::Declare(kind @ (Declaration::Let | Declaration::Const), declarations) =
        &program.stmts[initial].kind
    else {
        return None;
    };
    Some((*kind, declarations[0].0, source))
}

#[derive(Clone, Copy)]
pub(super) struct GeneratorCallYield {
    pub(super) callee: ExprId,
    pub(super) operand: Option<ExprId>,
    pub(super) delegate: bool,
    pub(super) destination: GeneratorCallDestination<ExprId>,
}

#[derive(Clone, Copy)]
pub(super) enum GeneratorCallDestination<T> {
    Discard,
    Assign(T),
    Initialize { kind: Declaration, pattern: binding_patterns::Id },
}

impl<T> GeneratorCallDestination<T> {
    pub(super) fn assignment(&self) -> Option<&T> {
        match self {
            Self::Assign(target) => Some(target),
            _ => None,
        }
    }
}

fn generator_expression_yield(
    program: &Program, expression: ExprId, action: Resume,
) -> Option<GeneratorCallYield> {
    let destination = match action {
        Resume::Discard => GeneratorCallDestination::Discard,
        Resume::Initialize { kind, pattern } => GeneratorCallDestination::Initialize { kind, pattern },
        _ => return None,
    };
    generator_call_yield(program, expression, destination)
}

fn generator_call_yield(
    program: &Program, expression: ExprId, destination: GeneratorCallDestination<ExprId>,
) -> Option<GeneratorCallYield> {
    use GeneratorCallDestination::{Assign, Discard};
    let (call, destination) = match program.exprs[expression].kind {
        ExprKind::Assign("=", target, right)
            if matches!(destination, Discard)
                && matches!(program.exprs[target].kind, ExprKind::Name(_) | ExprKind::Member(..)) =>
        {
            (right, Assign(target))
        }
        _ => (expression, destination),
    };
    let ExprKind::Call(callee, arguments) = &program.exprs[call].kind else { return None };
    if !matches!(program.exprs[*callee].kind, ExprKind::Name(_)) {
        return None;
    }
    let [Argument { expression, spread: false }] = arguments.as_slice() else { return None };
    let ExprKind::Yield { operand, delegate } = program.exprs[*expression].kind else { return None };
    Some(GeneratorCallYield { callee: *callee, operand, delegate, destination })
}

struct AwaitSite {
    operand: ExprId,
    callee: Option<ExprId>,
    method: Option<ExprId>,
    followup_call: Option<FollowupCalls>,
    member_key: Option<ExprId>,
    chain: Option<ExprId>,
    literal: Option<LiteralAwait>,
    negations: usize,
    assignment: Option<ExprId>,
}

#[derive(Clone, Copy)]
pub(super) struct LiteralAwait {
    pub(super) expression: ExprId,
    pub(super) member: usize,
}

#[derive(Clone, Copy)]
pub(super) struct FollowupCalls {
    pub(super) outer: ExprId,
    pub(super) count: usize,
}

pub(super) fn followup_receiver(program: &Program, expression: ExprId) -> Option<ExprId> {
    let ExprKind::Call(callee, _) = program.exprs[expression].kind else { return None };
    match program.exprs[callee].kind {
        ExprKind::Member(receiver, _) => Some(receiver),
        ExprKind::Call(..) | ExprKind::Unary("await", _) => Some(callee),
        _ => None,
    }
}

fn logical_not_operand(program: &Program, mut expression: ExprId) -> (ExprId, usize) {
    let mut negations = 0;
    while let ExprKind::Unary("!", operand) = program.exprs[expression].kind {
        negations += 1;
        expression = operand;
    }
    (expression, negations)
}

fn await_expression(program: &Program, expression: ExprId) -> Option<AwaitSite> {
    let (expression, assignment) = match program.exprs[expression].kind {
        ExprKind::Assign("=", left, right) if matches!(program.exprs[left].kind, ExprKind::Name(_) | ExprKind::Member(..)) => {
            (right, Some(left))
        }
        _ => (expression, None),
    };
    let literal_site = |member, value| {
        let mut site = await_value(program, value)?;
        site.literal = Some(LiteralAwait { expression, member });
        Some(site)
    };
    let mut site = match &program.exprs[expression].kind {
        ExprKind::Object(properties) => properties.iter().enumerate().find_map(|(member, property)| {
            let object_literals::Member::Property { value, kind: object_literals::Kind::Data, .. } = property else {
                return None;
            };
            literal_site(member, *value)
        }),
        ExprKind::Array(elements) => elements.iter().enumerate().find_map(|(member, element)| {
            let Some(Argument { expression, spread: false }) = element else { return None; };
            literal_site(member, *expression)
        }),
        _ => await_value(program, expression),
    }?;
    site.assignment = assignment;
    Some(site)
}

fn await_value(program: &Program, expression: ExprId) -> Option<AwaitSite> {
    let (expression, negations) = logical_not_operand(program, expression);
    let (expression, chain) = match program.exprs[expression].kind {
        ExprKind::OptionalChain(base, _) => (base, Some(expression)),
        _ => (expression, None),
    };
    let (expression, member_key) = match program.exprs[expression].kind {
        ExprKind::Member(receiver, key) => (receiver, Some(key)),
        _ => (expression, None),
    };
    let mut candidate = expression;
    let mut count = 0;
    let mut nested_operand = None;
    while let Some(receiver) = followup_receiver(program, candidate) {
        count += 1;
        candidate = receiver;
        if let Some(parts) = await_operand(program, candidate) {
            nested_operand = Some(parts);
            break;
        }
    }
    let ((operand, callee, method), followup_call) = match nested_operand {
        Some(parts) => (parts, Some(FollowupCalls { outer: expression, count })),
        None => (await_operand(program, expression)?, None),
    };
    Some(AwaitSite {
        operand, callee, method, followup_call, member_key, chain, literal: None, negations, assignment: None,
    })
}

fn await_operand(program: &Program, expression: ExprId) -> Option<(ExprId, Option<ExprId>, Option<ExprId>)> {
    // If the receiver is synchronous, the await can still be in the argument
    // of the ordinary member call.
    let (expression, method) = match &program.exprs[expression].kind {
        ExprKind::Call(method, arguments) if matches!(program.exprs[*method].kind, ExprKind::Member(..)) => {
            let Some(Argument { expression: first, spread: false }) = arguments.first() else { return None };
            (*first, Some(expression))
        }
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
    Some((operand, callee, method))
}

enum ExpressionPart {
    Value(ExprId, Resume),
    CaptureComparison,
    Comparison(&'static str, Resume),
    ReadAccess(ExprId, Resume),
    LogicalTest(&'static str, Resume),
    LogicalEnd,
    BeginLogicalNot(usize),
    FinishLogicalNot(usize, Resume),
    ConditionalTest,
    ConditionalElse,
    ConditionalEnd,
    CaptureAssignment(ExprId),
    WriteAssignment(Resume),
}

fn is_comparison(operator: &str) -> bool {
    matches!(operator, "===" | "!==" | "<" | "<=" | ">" | ">=")
}

fn is_composed_expression(kind: &ExprKind) -> bool {
    matches!(kind, ExprKind::Conditional(..) | ExprKind::Sequence(..) | ExprKind::Binary("&&" | "||" | "??", _, _))
        || matches!(kind, ExprKind::Binary(operator, _, _) if is_comparison(operator))
}

fn visit_expression(
    program: &Program,
    expression: ExprId,
    action: Resume,
    generator: bool,
    mut visit: impl FnMut(ExpressionPart) -> Result<(), Fault>,
) -> Result<(), Fault> {
    let mut pending = Vec::new();
    let mut part = ExpressionPart::Value(expression, action);
    loop {
        if let ExpressionPart::Value(expression, action) = part {
            match program.exprs[expression].kind {
                ExprKind::Sequence(left, right) => {
                    push(&mut pending, ExpressionPart::Value(right, action))?;
                    part = ExpressionPart::Value(left, Resume::Discard);
                    continue;
                }
                // The generator's short-circuit slot stores the operand class.
                // Returning or initializing needs the actual value.
                ExprKind::Binary(operator @ ("&&" | "||" | "??"), left, right)
                    if !generator || !matches!(action, Resume::Return | Resume::Initialize { .. }) =>
                {
                    push(&mut pending, ExpressionPart::LogicalEnd)?;
                    push(&mut pending, ExpressionPart::Value(right, action))?;
                    push(&mut pending, ExpressionPart::LogicalTest(operator, action))?;
                    part = ExpressionPart::Value(left, Resume::StoreExpressionValue);
                    continue;
                }
                ExprKind::Binary(operator, left, right) if !generator && is_comparison(operator) => {
                    push(&mut pending, ExpressionPart::Comparison(operator, action))?;
                    push(&mut pending, ExpressionPart::Value(right, Resume::StoreExpressionValue))?;
                    push(&mut pending, ExpressionPart::CaptureComparison)?;
                    part = ExpressionPart::Value(left, Resume::StoreExpressionValue);
                    continue;
                }
                ExprKind::Conditional(test, yes, no) => {
                    push(&mut pending, ExpressionPart::ConditionalEnd)?;
                    push(&mut pending, ExpressionPart::Value(no, action))?;
                    push(&mut pending, ExpressionPart::ConditionalElse)?;
                    push(&mut pending, ExpressionPart::Value(yes, action))?;
                    push(&mut pending, ExpressionPart::ConditionalTest)?;
                    part = ExpressionPart::Value(test, Resume::StoreExpressionValue);
                    continue;
                }
                ExprKind::Unary("!", _) if !generator => {
                    let (operand, negations) = logical_not_operand(program, expression);
                    if is_composed_expression(&program.exprs[operand].kind) {
                        push(&mut pending, ExpressionPart::FinishLogicalNot(negations, action))?;
                        push(&mut pending, ExpressionPart::Value(operand, Resume::StoreExpressionValue))?;
                        part = ExpressionPart::BeginLogicalNot(negations);
                    }
                }
                ExprKind::Member(receiver, _) | ExprKind::OptionalChain(receiver, _)
                    if !generator && is_composed_expression(&program.exprs[receiver].kind) =>
                {
                    push(&mut pending, ExpressionPart::ReadAccess(expression, action))?;
                    part = ExpressionPart::Value(receiver, Resume::StoreExpressionValue);
                    continue;
                }
                ExprKind::Assign("=", target, right)
                    if !generator && matches!(program.exprs[target].kind, ExprKind::Name(_) | ExprKind::Member(..))
                        && is_composed_expression(&program.exprs[right].kind) =>
                {
                    // These right-hand shapes combine expression results; direct
                    // literal naming and other ordinary assignments stay in eval.
                    push(&mut pending, ExpressionPart::WriteAssignment(action))?;
                    push(&mut pending, ExpressionPart::Value(right, Resume::StoreExpressionValue))?;
                    part = ExpressionPart::CaptureAssignment(target);
                    continue;
                }
                _ => {}
            }
        }
        visit(part)?;
        let Some(next) = pending.pop() else { return Ok(()) };
        part = next;
    }
}

fn expression_size(
    program: &Program, expression: ExprId, action: Resume, generator: bool,
) -> Result<(usize, usize), Fault> {
    let (mut instructions, mut sites) = (0, 0);
    visit_expression(program, expression, action, generator, |part| {
        match part {
            ExpressionPart::Value(expression, action) => {
                instructions += 1;
                sites += usize::from(if generator {
                    generator_expression_yield(program, expression, action).is_some()
                } else {
                    await_expression(program, expression).is_some()
                });
            }
            ExpressionPart::CaptureComparison | ExpressionPart::Comparison(..) | ExpressionPart::ReadAccess(..)
            | ExpressionPart::BeginLogicalNot(_) | ExpressionPart::FinishLogicalNot(..)
            | ExpressionPart::LogicalTest(_, _) | ExpressionPart::ConditionalTest
            | ExpressionPart::ConditionalElse | ExpressionPart::CaptureAssignment(_)
            | ExpressionPart::WriteAssignment(_) => instructions += 1,
            ExpressionPart::LogicalEnd | ExpressionPart::ConditionalEnd => {}
        }
        Ok(())
    })?;
    Ok((instructions, sites))
}

fn statement_size(program: &Program, id: StmtId, generator: bool) -> Result<(usize, usize), Fault> {
    match &program.stmts[id].kind {
        StmtKind::Yield(_) | StmtKind::YieldFrom(_) if generator => Ok((1, 1)),
        StmtKind::Declare(kind, declarations) => {
            let (mut instructions, mut sites) = (0, 0);
            for &(pattern, initial) in declarations {
                let action = Resume::Initialize { kind: *kind, pattern };
                let (length, suspensions) = initial.map(|expression| expression_size(program, expression, action, generator))
                    .transpose()?.unwrap_or((1, 0));
                instructions += if suspensions == 0 { 1 } else { length };
                sites += suspensions;
            }
            Ok((instructions, sites))
        }
        _ => {
            if let Some((expression, action)) = statement_expression(program, id) {
                expression_size(program, expression, action, generator)
            } else {
                Ok((1, 0))
            }
        }
    }
}

fn emit_expression(
    program: &Program, id: StmtId, expression: ExprId, action: Resume,
    generator: bool, code: &mut Vec<Instruction>,
) -> Result<(), Fault> {
    let mut branches = Vec::new();
    visit_expression(program, expression, action, generator, |part| {
        let instruction = match part {
            ExpressionPart::Value(expression, action) if generator => {
                if let Some(call) = generator_expression_yield(program, expression, action) {
                    Instruction::Yield { statement: id, call: Some(call) }
                } else {
                    Instruction::Evaluate { statement: id, expression, action }
                }
            }
            ExpressionPart::Value(expression, action) => {
                if let Some(AwaitSite {
                    operand, callee, method, followup_call, member_key, chain, literal, negations, assignment,
                }) = await_expression(program, expression)
                {
                    Instruction::Await {
                        statement: id, operand, callee, method, followup_call, member_key,
                        chain, literal, negations, assignment, action,
                    }
                } else {
                    Instruction::Evaluate { statement: id, expression, action }
                }
            }
            ExpressionPart::LogicalTest(operator, action) => {
                push(&mut branches, code.len())?;
                Instruction::Logical { statement: id, operator, target: 0, action }
            }
            ExpressionPart::CaptureComparison => Instruction::CaptureComparison(id),
            ExpressionPart::Comparison(operator, action) => {
                Instruction::Comparison { statement: id, operator, action }
            }
            ExpressionPart::ReadAccess(expression, action) => {
                Instruction::ReadAccess { statement: id, expression, action }
            }
            ExpressionPart::LogicalEnd => {
                let test = branches.pop().expect("logical expression must have a test");
                let end = code.len();
                let Instruction::Logical { target, .. } = &mut code[test] else { unreachable!() };
                *target = end;
                return Ok(());
            }
            ExpressionPart::BeginLogicalNot(negations) => Instruction::BeginLogicalNot(negations),
            ExpressionPart::FinishLogicalNot(negations, action) => {
                Instruction::FinishLogicalNot { statement: id, negations, action }
            }
            ExpressionPart::ConditionalTest => {
                push(&mut branches, code.len())?;
                Instruction::BranchOnValueFalse(0)
            }
            ExpressionPart::ConditionalElse => {
                let test = branches.pop().expect("conditional expression must have a test");
                let jump = code.len();
                push(code, Instruction::Jump(0))?;
                code[test] = Instruction::BranchOnValueFalse(code.len());
                push(&mut branches, jump)?;
                return Ok(());
            }
            ExpressionPart::ConditionalEnd => {
                let jump = branches.pop().expect("conditional expression must have a jump");
                code[jump] = Instruction::Jump(code.len());
                return Ok(());
            }
            ExpressionPart::CaptureAssignment(target) => Instruction::CaptureAssignment { statement: id, target },
            ExpressionPart::WriteAssignment(action) => Instruction::WriteAssignment { statement: id, action },
        };
        push(code, instruction)
    })
}

enum Visit {
    Statement { id: StmtId, eligible: bool, switch_parent: Option<StmtId> },
    Finish { id: StmtId, before: usize, condition_extra: usize },
}

// Count await sites in supported statement and expression forms. Each
// statement belongs to one template. Ineligible subtrees still contribute
// jump targets, but never await sites; nested function bodies are separate
// roots. The parser's independent expression count catches *all* embedded
// awaits, including unsupported conditions, pattern defaults and unselected branches.
fn split_statements(program: &Program, template: &Template, expected: usize) -> Result<(HashSet<StmtId>, usize), Fault> {
    let generator = template.kind == FunctionKind::Generator;
    let mut pending = Vec::new();
    for &id in template.body.iter().rev() {
        push(&mut pending, Visit::Statement { id, eligible: true, switch_parent: None })?;
    }
    let mut split = HashSet::new();
    let mut jumps = Vec::new();
    let mut sites = 0;
    let mut instructions = template.body.len();
    while let Some(visit) = pending.pop() {
        let (id, eligible, switch_parent) = match visit {
            Visit::Finish { id, before, condition_extra } => {
                if sites > before {
                    split.try_reserve(1).map_err(|_| storage_error())?;
                    split.insert(id);
                    // Each statement already accounts for one instruction.
                    // Opening a container adds its children and control flow.
                    instructions += condition_extra + match &program.stmts[id].kind {
                        StmtKind::Block(body) => body.len() + 1,
                        StmtKind::Switch { cases, body, .. } => cases.len() + body.len() + 1,
                        StmtKind::If(_, _, Some(_)) => 3,
                        StmtKind::If(_, _, None) => 1,
                        StmtKind::For(None, condition, advance, _) => {
                            1 + usize::from(condition.is_some()) + usize::from(advance.is_some())
                        }
                        StmtKind::ForOf(..) => 4,
                        StmtKind::Try(_, catch, finally) => {
                            2 + 2 * usize::from(catch.is_some_and(|(_, body)| split.contains(&body)))
                                + 2 * usize::from(finally.is_some_and(|body| split.contains(&body)))
                        }
                        _ => unreachable!(),
                    };
                }
                continue;
            }
            Visit::Statement { id, eligible, switch_parent } => (id, eligible, switch_parent),
        };
        let kind = &program.stmts[id].kind;
        if eligible {
            let (length, suspensions) = statement_size(program, id, generator)?;
            if suspensions != 0 {
                instructions += length - 1;
                sites += suspensions;
                split.try_reserve(1).map_err(|_| storage_error())?;
                split.insert(id);
                continue;
            }
        }
        let children_eligible = eligible && (
            matches!(kind, StmtKind::Block(_) | StmtKind::If(..) | StmtKind::Try(_, _, _) | StmtKind::For(None, _, _, _))
                || (!generator && (matches!(kind, StmtKind::Switch { .. }) || lexical_for_of(program, id).is_some()))
        );
        if children_eligible {
            let before = sites;
            let mut condition_extra = 0;
            if let (false, StmtKind::If(condition, _, _)) = (generator, kind) {
                let (length, awaits) = expression_size(program, *condition, Resume::BranchFalse(0), false)?;
                if awaits != 0 {
                    sites += awaits;
                    condition_extra = length - 1;
                }
            }
            push(&mut pending, Visit::Finish { id, before, condition_extra })?;
        }
        let parent = matches!(kind, StmtKind::Switch { .. }).then_some(id);
        let mut child = |id, eligible| push(&mut pending, Visit::Statement { id, eligible, switch_parent: parent });
        match kind {
            StmtKind::Block(body) | StmtKind::Switch { body, .. } => {
                for &item in body.iter().rev() { child(item, children_eligible)?; }
            }
            StmtKind::If(_, yes, no) => {
                if let Some(no) = no { child(*no, children_eligible)?; }
                child(*yes, children_eligible)?;
            }
            StmtKind::Try(body, catch, finally) => {
                if let Some(finally) = finally { child(*finally, !generator && children_eligible && catch.is_none())?; }
                if let Some((_, catch)) = catch {
                    child(*catch, !generator && children_eligible && finally.is_none())?;
                }
                child(*body, children_eligible)?;
            }
            StmtKind::While(_, body) | StmtKind::DoWhile(body, _) => child(*body, false)?,
            StmtKind::For(initial, _, _, body) => {
                if let Some(initial) = initial { child(*initial, false)?; }
                child(*body, children_eligible)?;
            }
            StmtKind::ForIn(initial, _, body) | StmtKind::ForOf(initial, _, body) => {
                child(*initial, false)?;
                child(*body, children_eligible)?;
            }
            StmtKind::Break(target) | StmtKind::Continue(target) => {
                let direct_switch_break = matches!(kind, StmtKind::Break(_)) && switch_parent == Some(*target);
                push(&mut jumps, (id, *target, direct_switch_break))?;
            }
            _ => {}
        }
    }
    if sites != expected {
        let message = if generator { "Yield in this statement position is not implemented." }
            else { "Await in this expression or statement position is not implemented." };
        return Err(Fault::new(message)
            .at(&program.source, template.source_start));
    }
    // Direct case breaks and iterator loop jumps have their own exits.
    // Other split ancestors still need explicit subtree unwinding.
    for (id, target, direct_switch_break) in jumps {
        if split.contains(&target) && !direct_switch_break && lexical_for_of(program, target).is_none() {
            let message = if generator { "A jump to a statement split by yield is not implemented." }
                else { "A jump to a statement split by await is not implemented." };
            return Err(Fault::new(message)
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
    SwitchBreak(usize),
    FinishSwitch(usize),
    FinishFor { statement: StmtId, start: usize, test: bool, advance: Option<ExprId> },
    FinishForOf(usize),
    FinishTry { handler: usize, catch: Option<StmtId>, finally: Option<StmtId> },
    FinishCatch(usize),
    FinishFinally(usize),
}

fn set_branch_target(code: &mut [Instruction], destination: usize) {
    for instruction in code {
        match instruction {
            Instruction::BranchFalse { target, .. }
            | Instruction::Await { action: Resume::BranchFalse(target), .. }
            | Instruction::Logical { action: Resume::BranchFalse(target), .. }
            | Instruction::FinishLogicalNot { action: Resume::BranchFalse(target), .. }
            | Instruction::Comparison { action: Resume::BranchFalse(target), .. }
            | Instruction::ReadAccess { action: Resume::BranchFalse(target), .. }
            | Instruction::WriteAssignment { action: Resume::BranchFalse(target), .. }
            | Instruction::Evaluate { action: Resume::BranchFalse(target), .. } => {
                *target = destination;
            }
            _ => {}
        }
    }
}

pub(super) fn compile(program: &Program, template: usize, expected_suspensions: usize, ast_limit: usize) -> Result<Vec<Instruction>, Fault> {
    let template = &program.templates[template];
    let generator = template.kind == FunctionKind::Generator;
    let (split, instructions) = split_statements(program, template, expected_suspensions)?;
    if instructions > ast_limit.saturating_sub(program.node_count()) {
        let operation = if generator { "yield" } else { "await" };
        return Err(Fault::new(format!("AST node limit reached ({ast_limit}) compiling {operation} plan."))
            .at(&program.source, template.source_start));
    }
    let mut code = Vec::new();
    code.try_reserve_exact(instructions).map_err(|_| storage_error())?;
    let mut pending = Vec::new();
    let mut switch_breaks = Vec::new();
    for &id in template.body.iter().rev() { push(&mut pending, Emit::Statement(id))?; }
    // Container statements emit bounded control instructions. Split expressions
    // emit instructions for their leaves and branches, within the same AST limit.
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
            Emit::SwitchBreak(owner) => {
                push(&mut switch_breaks, (owner, code.len()))?;
                push(&mut code, Instruction::Jump(0))?;
                continue;
            }
            Emit::FinishSwitch(owner) => {
                let leave = code.len();
                let Instruction::EnterSwitch { leave: exit, .. } = &mut code[owner] else { unreachable!() };
                *exit = leave;
                while let Some(&(current, jump)) = switch_breaks.last() {
                    if current != owner {
                        break;
                    }
                    code[jump] = Instruction::Jump(leave);
                    switch_breaks.pop();
                }
                push(&mut code, Instruction::LeaveBlock)?;
                continue;
            }
            Emit::FinishFor { statement, start, test, advance } => {
                if let Some(expression) = advance {
                    push(&mut code, Instruction::Evaluate {
                        statement, expression, action: Resume::Discard,
                    })?;
                }
                push(&mut code, Instruction::Jump(start))?;
                if test {
                    let destination = code.len();
                    set_branch_target(&mut code[start..start + 1], destination);
                }
                continue;
            }
            Emit::FinishForOf(entry) => {
                push(&mut code, Instruction::LeaveBlock)?;
                push(&mut code, Instruction::Jump(entry + 1))?;
                let end = code.len();
                let Instruction::EnterForOf { after, .. } = &mut code[entry] else { unreachable!() };
                *after = end;
                continue;
            }
            Emit::FinishTry { handler, catch, finally } => {
                push(&mut code, Instruction::FinishTry)?;
                let end = code.len();
                let Instruction::PushTry { after, catch_entry, finally_entry, .. } = &mut code[handler] else { unreachable!() };
                if let Some(catch) = catch {
                    *catch_entry = Some(end);
                    push(&mut pending, Emit::FinishCatch(handler))?;
                    push(&mut pending, Emit::Statement(catch))?;
                } else if let Some(finally) = finally {
                    *finally_entry = Some(end);
                    push(&mut pending, Emit::FinishFinally(handler))?;
                    push(&mut pending, Emit::Statement(finally))?;
                } else {
                    *after = end;
                }
                continue;
            }
            Emit::FinishCatch(handler) => {
                push(&mut code, Instruction::LeaveBlock)?;
                let end = code.len();
                let Instruction::PushTry { after, .. } = &mut code[handler] else { unreachable!() };
                *after = end;
                continue;
            }
            Emit::FinishFinally(handler) => {
                push(&mut code, Instruction::FinishFinally)?;
                let end = code.len();
                let Instruction::PushTry { after, .. } = &mut code[handler] else { unreachable!() };
                *after = end;
                continue;
            }
        };
        if !split.contains(&id) {
            push(&mut code, Instruction::Statement(id))?;
        } else if let Some((expression, action)) = statement_expression(program, id) {
            emit_expression(program, id, expression, action, generator, &mut code)?;
        } else {
            match &program.stmts[id].kind {
                StmtKind::Yield(_) | StmtKind::YieldFrom(_) => push(&mut code, Instruction::Yield { statement: id, call: None })?,
                StmtKind::Declare(kind, declarations) => {
                    for (index, &(pattern, initial)) in declarations.iter().enumerate() {
                        if let Some(expression) = initial {
                            let action = Resume::Initialize { kind: *kind, pattern };
                            if expression_size(program, expression, action, generator)?.1 != 0 {
                                emit_expression(program, id, expression, action, generator, &mut code)?;
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
                    if generator || expression_size(program, *condition, Resume::BranchFalse(0), false)?.1 == 0 {
                        push(&mut code, Instruction::BranchFalse { statement: id, condition: *condition, target: 0 })?;
                    } else {
                        emit_expression(program, id, *condition, Resume::BranchFalse(0), false, &mut code)?;
                    }
                    let branch = start..code.len();
                    let next = match no {
                        Some(no) => Emit::Else { branch, alternative: *no },
                        None => Emit::FinishBranch(branch),
                    };
                    push(&mut pending, next)?;
                    push(&mut pending, Emit::Statement(*yes))?;
                }
                StmtKind::For(None, condition, advance, body) => {
                    let start = code.len();
                    if let Some(condition) = condition {
                        push(&mut code, Instruction::BranchFalse {
                            statement: id, condition: *condition, target: 0,
                        })?;
                    }
                    push(&mut pending, Emit::FinishFor {
                        statement: id, start, test: condition.is_some(), advance: *advance,
                    })?;
                    push(&mut pending, Emit::Statement(*body))?;
                }
                StmtKind::ForOf(_, _, body) => {
                    let entry = code.len();
                    push(&mut code, Instruction::EnterForOf { statement: id, after: 0 })?;
                    push(&mut code, Instruction::NextForOf(id))?;
                    push(&mut pending, Emit::FinishForOf(entry))?;
                    push(&mut pending, Emit::Statement(*body))?;
                }
                StmtKind::Switch { cases, body, .. } => {
                    let owner = code.len();
                    push(&mut code, Instruction::EnterSwitch { statement: id, leave: 0 })?;
                    // Case selection uses the original case ordinal. Its jump
                    // entry maps that ordinal to the compiled statement offset.
                    let table = code.len();
                    for _ in cases {
                        push(&mut code, Instruction::Jump(0))?;
                    }
                    push(&mut pending, Emit::FinishSwitch(owner))?;
                    let mut entries = cases.iter().enumerate().rev().peekable();
                    for offset in (0..=body.len()).rev() {
                        if let Some(&item) = body.get(offset) {
                            let task = match program.stmts[item].kind {
                                StmtKind::Break(target) if target == id => Emit::SwitchBreak(owner),
                                _ => Emit::Statement(item),
                            };
                            push(&mut pending, task)?;
                        }
                        while let Some(&(case, (_, entry))) = entries.peek() {
                            if *entry != offset {
                                break;
                            }
                            entries.next();
                            push(&mut pending, Emit::FinishJump(table + case))?;
                        }
                    }
                    debug_assert!(entries.next().is_none());
                }
                StmtKind::Try(body, catch, finally) => {
                    let handler = code.len();
                    let catch = catch.map(|(_, body)| body).filter(|body| split.contains(body));
                    let finally = finally.filter(|body| split.contains(body));
                    push(&mut code, Instruction::PushTry {
                        statement: id, after: 0, catch_entry: None, finally_entry: None,
                    })?;
                    push(&mut pending, Emit::FinishTry { handler, catch, finally })?;
                    push(&mut pending, Emit::Statement(*body))?;
                }
                _ => unreachable!(),
            }
        }
    }
    debug_assert_eq!(code.len(), instructions);
    debug_assert!(switch_breaks.is_empty());
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Result<Program, Fault> {
        Parser::parse(source, &mut MAX_REGEXP_WORK.clone())
    }

    #[test]
    fn generator_resumption_plan_bounds_and_validates_the_admitted_suspensions() {
        let source = "function* g(){try{yield 1;}finally{cleanup();}}";
        let program = parse(source).unwrap();
        assert_eq!(program.resumption_plan_nodes, 5);
        assert!(matches!(program.templates[0].resumption_plan.as_slice(), [
            Instruction::PushTry { .. }, Instruction::EnterBlock(_),
            Instruction::Yield { .. }, Instruction::LeaveBlock, Instruction::FinishTry,
        ]));
        for source in [
            "function* g(){if(take(yield 1)){yield 2;}}",
            "function* g(){try{}finally{yield 1;}}",
            "function* g(){try{}catch(error){yield 1;}}",
            "function* g(){let value=yield 1;}",
            "function* g(){receiver.call(yield 1);}",
            "function* g(){take(...yield 1);}",
            "function* g(){take(yield 1,2);}",
        ] {
            let error = parse(source).err().expect("unsupported yield placement was accepted");
            assert!(error.message.contains("Yield in this statement position"), "{}", error.display());
        }
        let source = "function* g(){escape:{yield 1;break escape;}}";
        let error = parse(source).err().expect("jump to a split scope was accepted");
        assert!(error.message.contains("split by yield"), "{}", error.display());
    }

    #[test]
    fn async_plan_jumps_scope_exits_and_catches_point_to_owned_instructions() {
        let program = parse("async function go(flag,p){before();\
            if(flag){let x=await p;use(x);}else{await p;}\
            try{return await p;}catch(e){return handle(e);}}").unwrap();
        let code = &program.templates[0].resumption_plan;
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
            "return receiver.method(1, await p);",
            "return receiver.method(1 + await p);",
            "receiver[await p] = await p;",
            "value += await p;",
            "value = flag && (1+await p);",
            "return (await p)[await p];",
            "return (await p)?.[await p];",
            "return (await p)?.method(await p);",
            "return (await p)(await p);",
            "return (await p).method(await p);",
            "return use(...await p);",
            "return (use(), 1 + await p);",
            "return (1+await p) && use(await p);",
            "return true ? await p : use(1+await p);",
            "if(1+await p){}",
            "if(!(1+await p)){}",
            "return await p + 1;",
            "return use()!=await p;",
            "return {[await p]:await p};",
            "let a=await p,b=1+await p;",
            "let {value=await p}=await p;",
            "while(false){await p;}",
            "for(let i=0;i<2;i++){await p;}",
            "for(;await p;){await p;}",
            "for(;true;await p){await p;}",
            "for(var item of p){await item;}",
            "for(let item of await p){await item;}",
            "switch(await p){case 0:await p;}",
            "switch(0){case await p:await p;}",
            "try{}catch(e){await p;}finally{}",
            "try{}catch{}finally{await p;}",
            "await (await p);",
        ] {
            let source = format!("async function go(p){{{body}}}");
            let error = parse(&source).err().expect("unsupported await was accepted");
            assert!(error.message.contains("Await in this expression or statement position"), "{body}: {}", error.display());
        }
        let source = "async function go(p){exit:{try{await p;}catch(e){break exit;}}}";
        let error = parse(source).err().expect("jump to split ancestor was accepted");
        assert!(error.message.contains("jump to a statement split by await"), "{}", error.display());
        let error = parse("async function go(p){switch(0){case 0:await p;{break;}}}")
            .err().expect("a break inside a nested case block was accepted");
        assert!(error.message.contains("jump to a statement split by await"), "{}", error.display());
        let error = parse("async function go(p){for(;;){await p;continue;}}")
            .err().expect("jump to a split loop was accepted");
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
            let code = &template.resumption_plan;
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
            ("async function f(p){return {entry:use(await p)};}", 1),
            ("async function f(p){if(!use(await p)){done();}}", 2),
            ("async function f(p){if(!(allowed()&&use(await p)||other())){done();}}", 8),
            ("async function f(p){if(use(await p)!==flag){done();}}", 5),
            ("async function f(p){return use(await p)===(original!==use(await p));}", 7),
            ("async function f(p){return use(await p).value > use(await p);}", 4),
            ("async function f(p){return ((use(await p)??base)?.item??fallback).done;}", 7),
            ("async function f(p){try{}finally{await p;}}", 7),
            ("async function f(p){for(;test();advance()){await p;}}", 6),
            ("async function f(p){for(;;)return await p;}", 2),
            ("async function f(p){for(let value of p){await value;}}", 7),
            ("async function f(p){for(let value of p){await value;if(value)continue;break;}}", 9),
            ("async function f(p){switch(0){case 0:case 1:await p;break;default:done();}}", 8),
            ("async function f(p){return yes() && use(await p);}", 3),
            ("async function f(p){return yes() && use(await p).ready;}", 3),
            ("async function f(p){if(yes() && use(await p).ready){noop();}}", 4),
            ("async function f(p){return yes() && use(await p) && after();}", 5),
            ("async function f(p){return choose() ? await p : other();}", 5),
            ("async function f(p){return choose() ? one() : make(await p)(1);}", 5),
            ("async function f(p){target=use(await p).read(1);}", 1),
            ("async function f(p){try{throw 1;}catch(e){await p;}}", 7),
            ("async function f(p){value=flag ? left(await p) : right(await p);}", 7),
            ("async function f(p){let value; value=use(await p);}", 2),
            ("async function f(p){receiver.field=use(await p);}", 1),
            ("async function f(p){let value=receiver.method(use(await p));}", 1),
            ("async function f(p){return receiver.method(use(await p), 1, ...others);}", 1),
        ] {
            let mut runtime = Runtime::for_test();
            let program = runtime.register_program(source, false).unwrap();
            let required = program.node_count();
            assert_eq!(program.resumption_plan_nodes, instructions);
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
