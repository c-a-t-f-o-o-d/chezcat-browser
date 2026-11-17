//! Original bounded expression continuations. Groups, calls, computed keys and
//! operators suspend here rather than on the native stack. Compound literals,
//! function headers, prefix unary and new still use the guarded native helpers.
//! Grouping preserves references; optional chains end at the grouping boundary.
//! https://tc39.es/ecma262/multipage/ecmascript-language-expressions.html

use super::{
    optional_chaining::Step, template_literals::Part, Argument, ExprId, ExprKind, Fault, Parser,
    TokenKind, MAX_EXPRESSION_FRAMES,
};

const POWER: u8 = 13;

#[derive(Clone, Copy)]
struct Goal {
    minimum: u8,
    calls: bool,
    updates: bool,
    prefixes: bool,
}

impl Goal {
    fn expression(minimum: u8) -> Self {
        Self {
            minimum,
            calls: true,
            updates: true,
            prefixes: true,
        }
    }
    fn member(calls: bool) -> Self {
        Self {
            minimum: POWER + 1,
            calls,
            updates: false,
            prefixes: false,
        }
    }
}

#[derive(Clone, Copy)]
struct Context {
    goal: Goal,
    at: usize,
    allow_in: bool,
}

// Only a postfix owner retains these steps. In particular, (base?.key).next
// creates a new owner after the grouped child has finished its optional chain.
struct Postfix {
    value: ExprId,
    chain: Option<ExprId>,
    steps: Vec<Step>,
}

impl Postfix {
    fn new(value: ExprId) -> Self {
        Self {
            value,
            chain: None,
            steps: Vec::new(),
        }
    }
}

enum Frame {
    Group(Context),
    Sequence(Context, ExprId),
    Binary(Context, ExprId, &'static str),
    Assign(Context, ExprId, &'static str),
    Yes(Context, ExprId),
    No(Context, ExprId, ExprId),
    Key(Context, Postfix, bool),
    Call(Context, Postfix, Vec<Argument>, bool, bool),
}

enum Phase {
    Start,
    Postfix(Postfix),
    Infix(ExprId),
    AssignmentDone(ExprId),
    Finish(ExprId),
}

// Reserve geometrically but do not request capacity beyond an existing limit.
// No source/token/tree is cloned into a continuation. Allocation failure follows
// the fatal resource path rather than publishing an incomplete program.
fn reserve_one<T>(values: &mut Vec<T>, limit: usize) -> Result<(), Fault> {
    if values.len() >= limit {
        return Err(Fault::new("Expression scratch storage limit reached."));
    }
    if values.len() == values.capacity() {
        let next = values.capacity().saturating_mul(2).max(4).min(limit);
        values
            .try_reserve_exact(next - values.len())
            .map_err(|_| Fault::new("Expression scratch allocation failed."))?;
    }
    Ok(())
}

impl Parser {
    pub(super) fn continuation_assignment(&mut self) -> Result<ExprId, Fault> {
        self.expression_goal(Goal::expression(1))
    }

    pub(super) fn left_hand_side_expression(&mut self) -> Result<ExprId, Fault> {
        self.expression_goal(Goal::member(true))
    }

    fn expression_goal(&mut self, goal: Goal) -> Result<ExprId, Fault> {
        let previous_in = self.allow_in;
        let mut pending = Vec::new();
        let result = self.expression_loop(goal, &mut pending);
        // Every normal pop already released its slot. These are precisely this
        // invocation's outstanding slots, not its caller's suspended work.
        self.expression_frames -= pending.len();
        self.allow_in = previous_in;
        result
    }

    fn trim_expression_frames(&self, pending: &mut Vec<Frame>) -> Result<(), Fault> {
        // A native helper can reenter this machine. Do not retain an old 4096-
        // slot high-water allocation in each of 64 otherwise-empty callers.
        // Geometric contraction bounds suspended capacity by live slots plus a
        // constant per native entry; moving frames neither clones nor recharges.
        if pending.capacity() > pending.len().saturating_mul(2).max(4) {
            let mut compact = Vec::new();
            compact
                .try_reserve_exact(pending.len())
                .map_err(|_| Fault::new("Expression scratch allocation failed."))?;
            compact.append(pending);
            *pending = compact;
        }
        Ok(())
    }

    fn push_expression_frame(
        &mut self,
        pending: &mut Vec<Frame>,
        frame: Frame,
    ) -> Result<(), Fault> {
        if self.expression_frames >= MAX_EXPRESSION_FRAMES {
            return Err(self.unsupported("Pending expression limit reached (4096)."));
        }
        reserve_one(pending, MAX_EXPRESSION_FRAMES)?;
        pending.push(frame);
        self.expression_frames += 1;
        Ok(())
    }

    pub(super) fn append_argument(
        &mut self,
        args: &mut Vec<Argument>,
        argument: Argument,
    ) -> Result<(), Fault> {
        if args.len() >= self.capacity.arguments {
            return Err(self.unsupported(format!("Call argument limit reached ({}).", self.capacity.arguments)));
        }
        // Each call argument has a distinct parsed expression root. Counting all
        // entries, including those already in the AST, bounds aggregate buffers
        // across suspended calls and guarded parser reentry, not only one call.
        if self.argument_entries >= self.capacity.ast_nodes {
            return Err(self.unsupported("Call argument storage limit reached (131072)."));
        }
        reserve_one(args, self.capacity.arguments)?;
        args.push(argument);
        self.argument_entries += 1;
        Ok(())
    }

    fn append_postfix(
        &mut self,
        owner: &mut Postfix,
        step: Step,
        at: usize,
    ) -> Result<(), Fault> {
        if owner.chain.is_some() {
            if owner.steps.len() >= MAX_EXPRESSION_FRAMES {
                return Err(self.unsupported("Optional-chain step limit reached (4096)."));
            }
            self.check_nodes()?;
            reserve_one(&mut owner.steps, MAX_EXPRESSION_FRAMES)?;
            self.program.chain_steps += 1;
            owner.steps.push(step);
        } else {
            owner.value = self.expr(
                match step {
                    Step::Member { key, .. } => ExprKind::Member(owner.value, key),
                    Step::Call { args, .. } => ExprKind::Call(owner.value, args),
                },
                at,
            )?;
        }
        Ok(())
    }

    fn finish_postfix(&mut self, owner: Postfix, at: usize) -> Result<ExprId, Fault> {
        if let Some(base) = owner.chain {
            self.expr(ExprKind::OptionalChain(base, owner.steps), at)
        } else {
            Ok(owner.value)
        }
    }

    fn check_update_target(&self, value: ExprId) -> Result<(), Fault> {
        if !self.assignable(value) {
            return Err(self.error("Invalid update target."));
        }
        if let ExprKind::Name(name) = &self.program.exprs[value].kind {
            self.strict_binding(name, self.strict)?;
        }
        Ok(())
    }

    // These two helpers intentionally remain recursive and guarded. They are
    // separate functions so the continuation loop never grants a fresh depth
    // allowance to unary/new, literals, function headers or their descendants.
    fn prefix_expression(&mut self, op: &'static str, at: usize) -> Result<ExprId, Fault> {
        self.current += 1;
        self.enter()?;
        let result = self.expression_goal(Goal::expression(POWER + 1));
        self.depth -= 1;
        let value = result?;
        if op == "delete"
            && self.strict
            && matches!(self.program.exprs[value].kind, ExprKind::Name(_))
        {
            return Err(self.error("Deleting an unqualified identifier is forbidden in strict code."));
        }
        if matches!(op, "++" | "--") {
            self.check_update_target(value)?;
            self.expr(ExprKind::Update(value, op == "++", true), at)
        } else {
            self.expr(ExprKind::Unary(op, value), at)
        }
    }

    fn constructor_expression(&mut self, at: usize) -> Result<ExprId, Fault> {
        self.need("new")?;
        if self.eat(".") {
            if !self.is("target") {
                return Err(self.error("Expected target after new."));
            }
            self.current += 1;
            if !self.new_target_allowed {
                return Err(self.error("new.target requires an enclosing ordinary function."));
            }
            return self.expr(ExprKind::NewTarget, at);
        }
        self.enter()?;
        let result = (|| {
            let constructor = self.expression_goal(Goal::member(false))?;
            if self.is("?.") {
                return Err(self.error("An optional chain cannot be the constructor in new."));
            }
            let args = if self.is("(") {
                self.arguments()?
            } else {
                Vec::new()
            };
            Ok((constructor, args))
        })();
        self.depth -= 1;
        let (constructor, args) = result?;
        self.expr(ExprKind::Construct(constructor, args), at)
    }

    // Keep the driver small on the native stack. In debug builds, large match
    // arms can reserve their scratch locals for the entire invocation, including
    // while a compound literal/default/unary helper recursively reenters us.
    fn expression_loop(
        &mut self,
        goal: Goal,
        pending: &mut Vec<Frame>,
    ) -> Result<ExprId, Fault> {
        let mut context = Context { goal, at: self.at(), allow_in: self.allow_in };
        let mut phase = Phase::Start;
        loop {
            self.allow_in = context.allow_in;
            (context, phase) = match phase {
                Phase::Start => self.start_expression(context, pending)?,
                Phase::Postfix(owner) => self.postfix_expression(context, owner, pending)?,
                Phase::Infix(value) => self.infix_expression(context, value, pending)?,
                Phase::AssignmentDone(value) => self.finish_assignment(context, value, pending)?,
                Phase::Finish(value) => {
                    let Some(frame) = pending.pop() else {
                        return Ok(value);
                    };
                    self.expression_frames -= 1;
                    self.resume_expression(frame, value, pending)?
                }
            };
        }
    }

    fn start_expression(
        &mut self,
        mut context: Context,
        pending: &mut Vec<Frame>,
    ) -> Result<(Context, Phase), Fault> {
        self.trim_expression_frames(pending)?;
        let phase = if context.goal.minimum <= 1 && self.arrow_ahead() {
            // A block-bodied arrow is a complete assignment expression;
            // a following binary operator cannot attach outside it.
            Phase::AssignmentDone(self.arrow()?)
        } else if context.goal.minimum <= 1 && self.pattern_ahead() {
            let pattern = self.binding_pattern(false)?;
            Phase::Infix(self.expr(ExprKind::Pattern(pattern), context.at)?)
        } else if context.goal.prefixes && prefix_operator(&self.token().kind).is_some() {
            let op = prefix_operator(&self.token().kind).unwrap();
            Phase::Infix(self.prefix_expression(op, context.at)?)
        } else if self.is("new") {
            Phase::Postfix(Postfix::new(self.constructor_expression(context.at)?))
        } else if self.eat("(") {
            self.push_expression_frame(pending, Frame::Group(context))?;
            context = Context { goal: Goal::expression(0), at: self.at(), allow_in: true };
            Phase::Start
        } else {
            Phase::Postfix(Postfix::new(self.primary()?))
        };
        Ok((context, phase))
    }

    fn postfix_expression(
        &mut self,
        mut context: Context,
        mut owner: Postfix,
        pending: &mut Vec<Frame>,
    ) -> Result<(Context, Phase), Fault> {
        let optional = context.goal.calls && self.eat("?.");
        if optional && owner.chain.is_none() {
            owner.chain = Some(owner.value);
        }
        let phase = if (context.goal.calls && self.is("(")) || optional && self.is("(") {
            self.need("(")?;
            if self.eat(")") {
                self.append_postfix(&mut owner, Step::Call { args: Vec::new(), optional }, context.at)?;
                Phase::Postfix(owner)
            } else {
                let spread = self.eat("...");
                self.push_expression_frame(pending, Frame::Call(context, owner, Vec::new(), optional, spread))?;
                context = Context { goal: Goal::expression(1), at: self.at(), allow_in: true };
                Phase::Start
            }
        } else if self.eat("[") {
            self.push_expression_frame(pending, Frame::Key(context, owner, optional))?;
            context = Context { goal: Goal::expression(0), at: self.at(), allow_in: true };
            Phase::Start
        } else if optional || self.eat(".") {
            let key = self.chain_property()?;
            self.append_postfix(&mut owner, Step::Member { key, optional }, context.at)?;
            Phase::Postfix(owner)
        } else if matches!(self.token().kind, TokenKind::Template(_, Part::NoSubstitution | Part::Head)) {
            return Err(if owner.chain.is_some() {
                self.error("An optional chain cannot be a tagged template.")
            } else {
                self.unsupported("Tagged template literals are not implemented.")
            });
        } else {
            let mut value = self.finish_postfix(owner, context.at)?;
            if context.goal.updates && !self.token().newline && (self.is("++") || self.is("--")) {
                self.check_update_target(value)?;
                let increment = self.eat("++");
                if !increment {
                    self.need("--")?;
                }
                value = self.expr(ExprKind::Update(value, increment, false), context.at)?;
            }
            Phase::Infix(value)
        };
        Ok((context, phase))
    }

    fn infix_expression(
        &mut self,
        mut context: Context,
        value: ExprId,
        pending: &mut Vec<Frame>,
    ) -> Result<(Context, Phase), Fault> {
        let phase = if context.goal.minimum <= 2 && self.eat("?") {
            self.push_expression_frame(pending, Frame::Yes(context, value))?;
            context = Context { goal: Goal::expression(1), at: self.at(), allow_in: true };
            Phase::Start
        } else if let Some(op) = assignment_operator(&self.token().kind).filter(|_| context.goal.minimum <= 1) {
            if !self.assignable(value)
                && !(op == "=" && matches!(self.program.exprs[value].kind, ExprKind::Pattern(_)))
            {
                return Err(self.error("Invalid assignment target."));
            }
            if let ExprKind::Name(name) = &self.program.exprs[value].kind {
                self.strict_binding(name, self.strict)?;
            }
            self.current += 1;
            self.push_expression_frame(pending, Frame::Assign(context, value, op))?;
            context = Context { goal: Goal::expression(1), at: self.at(), allow_in: context.allow_in };
            Phase::Start
        } else if let Some((op, precedence)) = binary_operator(&self.token().kind, context.allow_in)
            .filter(|(_, precedence)| *precedence >= context.goal.minimum)
        {
            if op == "**" && matches!(self.program.exprs[value].kind, ExprKind::Unary(_, _))
                && !self.program.exprs[value].parenthesized
            {
                return Err(self.error("A unary expression before ** must be parenthesized."));
            }
            self.current += 1;
            self.push_expression_frame(pending, Frame::Binary(context, value, op))?;
            let minimum = precedence + u8::from(op != "**");
            context = Context { goal: Goal::expression(minimum), at: self.at(), allow_in: context.allow_in };
            Phase::Start
        } else {
            Phase::AssignmentDone(value)
        };
        Ok((context, phase))
    }

    fn finish_assignment(
        &mut self,
        mut context: Context,
        value: ExprId,
        pending: &mut Vec<Frame>,
    ) -> Result<(Context, Phase), Fault> {
        let phase = if context.goal.minimum == 0 && self.eat(",") {
            self.push_expression_frame(pending, Frame::Sequence(context, value))?;
            context = Context { goal: Goal::expression(1), at: self.at(), allow_in: context.allow_in };
            Phase::Start
        } else {
            Phase::Finish(value)
        };
        Ok((context, phase))
    }

    fn resume_expression(
        &mut self,
        frame: Frame,
        value: ExprId,
        pending: &mut Vec<Frame>,
    ) -> Result<(Context, Phase), Fault> {
        let context;
        let phase = match frame {
            Frame::Group(parent) => {
                self.need(")")?;
                self.program.exprs[value].parenthesized = true;
                context = parent;
                Phase::Postfix(Postfix::new(value))
            }
            Frame::Sequence(parent, left) => {
                context = parent;
                Phase::AssignmentDone(self.expr(ExprKind::Sequence(left, value), parent.at)?)
            }
            Frame::Binary(parent, left, op) => {
                self.check_coalescing(op, left, value)?;
                let at = if op == "**" {
                    self.program.exprs[left].at
                } else {
                    parent.at
                };
                context = parent;
                Phase::Infix(self.expr(ExprKind::Binary(op, left, value), at)?)
            }
            Frame::Assign(parent, left, op) => {
                context = parent;
                Phase::AssignmentDone(self.expr(ExprKind::Assign(op, left, value), parent.at)?)
            }
            Frame::Yes(parent, condition) => {
                self.need(":")?;
                self.push_expression_frame(pending, Frame::No(parent, condition, value))?;
                context = Context { goal: Goal::expression(1), at: self.at(), allow_in: parent.allow_in };
                Phase::Start
            }
            Frame::No(parent, condition, yes) => {
                context = parent;
                Phase::AssignmentDone(self.expr(ExprKind::Conditional(condition, yes, value), parent.at)?)
            }
            Frame::Key(parent, mut owner, optional) => {
                self.need("]")?;
                self.append_postfix(&mut owner, Step::Member { key: value, optional }, parent.at)?;
                context = parent;
                Phase::Postfix(owner)
            }
            Frame::Call(parent, mut owner, mut args, optional, spread) => {
                self.append_argument(&mut args, Argument { expression: value, spread })?;
                if self.eat(",") && !self.is(")") {
                    if args.len() == self.capacity.arguments {
                        return Err(self.unsupported(format!("Call argument limit reached ({}).", self.capacity.arguments)));
                    }
                    let spread = self.eat("...");
                    self.push_expression_frame(pending, Frame::Call(parent, owner, args, optional, spread))?;
                    context = Context { goal: Goal::expression(1), at: self.at(), allow_in: true };
                    Phase::Start
                } else {
                    self.need(")")?;
                    self.append_postfix(&mut owner, Step::Call { args, optional }, parent.at)?;
                    context = parent;
                    Phase::Postfix(owner)
                }
            }
        };
        Ok((context, phase))
    }
}

fn prefix_operator(token: &TokenKind) -> Option<&'static str> {
    match token {
        TokenKind::Punct(op @ ("!" | "~" | "+" | "-" | "++" | "--")) => Some(*op),
        TokenKind::Word(word) if word == "typeof" => Some("typeof"),
        TokenKind::Word(word) if word == "void" => Some("void"),
        TokenKind::Word(word) if word == "delete" => Some("delete"),
        _ => None,
    }
}

fn assignment_operator(token: &TokenKind) -> Option<&'static str> {
    match token {
        TokenKind::Punct(op @ ("=" | "+=" | "-=" | "*=" | "**=" | "&&=" | "||=" | "??=" | "/=" | "%="
            | "&=" | "|=" | "^=" | "<<=" | ">>=" | ">>>=")) => Some(*op),
        _ => None,
    }
}

fn binary_operator(token: &TokenKind, allow_in: bool) -> Option<(&'static str, u8)> {
    let op = match token {
        TokenKind::Punct(op) => *op,
        TokenKind::Word(word) if word == "in" && allow_in => "in",
        TokenKind::Word(word) if word == "instanceof" => "instanceof",
        _ => return None,
    };
    let precedence = match op {
        "||" | "??" => 3,
        "&&" => 4,
        "|" => 5,
        "^" => 6,
        "&" => 7,
        "==" | "!=" | "===" | "!==" => 8,
        "<" | ">" | "<=" | ">=" | "in" | "instanceof" => 9,
        "<<" | ">>" | ">>>" => 10,
        "+" | "-" => 11,
        "*" | "/" | "%" => 12,
        "**" => POWER,
        _ => return None,
    };
    Some((op, precedence))
}
