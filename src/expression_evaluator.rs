//! Bounded heap continuations for expression evaluation.
//!
//! Native callbacks, statement bodies and compound-literal helpers still enter
//! through Runtime::eval/statement and retain their native reentry guards. An
//! unfinished expression is data, not another Rust call. All simultaneous
//! drivers share frame and scratch-capacity quotas, including callback reentry.

use super::optional_chaining::{CallTarget, Completion, Step};
use super::{
    logical_short_circuits, Argument, ExprId, ExprKind, Fault, Host, Program, PropertyKey, Reference,
    Runtime, Value, MAX_EVALUATION_FRAMES, MAX_EVALUATION_FRAME_SLOTS, MAX_EVAL_DEPTH,
};
use std::rc::Rc;
use super::function_reachability::Edge;
use super::transient_roots::Marker;

#[derive(Clone, Copy)]
enum Mode {
    Value,
    Reference,
    Chain,
}

#[cfg(test)]
mod root_tests {
    use super::*;

    #[test]
    fn suspended_heap_snapshot_contains_receivers_operands_references_and_outer_arguments() {
        let mut runtime = Runtime::for_capacity_test();
        let objects: Vec<_> = (0..12).map(|_| runtime.new_object(None).unwrap()).collect();
        let functions: Vec<_> = (0..4).map(|_| {
            let Value::Function(id) = runtime.native_function(super::super::Builtin::IsNaN, "rootFixture").unwrap() else { unreachable!() }; id
        }).collect();
        let frame = |state| Frame { id: 0, state, annotate: true };
        let scratch = Scratch { frames: vec![
            frame(State::BinaryRight { op: "+", left: Value::Object(objects[0]) }),
            frame(State::ReadKey { receiver: Value::Object(objects[1]), end: ReadEnd::Value }),
            frame(State::MemberKey { receiver: Value::Object(objects[2]), arguments: &[] }),
            frame(State::ChainKey { receiver: Value::Object(objects[3]), steps: &[], next: 0, end: ChainEnd::Value }),
            frame(State::AssignmentValue { op: "+=", left: 0, right: 0,
                reference: Reference::ComputedProperty(Value::Object(objects[4]), Value::Object(objects[5])), before: Some(Value::Function(functions[0])) }),
            frame(State::Argument(Call { target: Target::Member { receiver: Value::Object(objects[6]), key: PropertyKey::String("f".into()), function: Some(Value::Function(functions[1])) },
                arguments: &[], index: 0, start: 1, after: AfterCall::Value })),
        ], arguments: vec![Value::Object(objects[7]), Value::Function(functions[2]), Value::Number(1.0)] };
        let target = Target::Chain(CallTarget { receiver: Value::Object(objects[9]), function: Some(Value::Function(functions[3])), name: None });
        let reference = Reference::Binding(0, "fixture".into());
        let completed = Completion::Reference(Reference::ComputedProperty(Value::Object(objects[10]), Value::Object(objects[11])));
        let before = (runtime.functions.len(), runtime.objects.len(), runtime.value_bytes, runtime.job_usage());
        let marker = runtime.publish_heap_roots(&scratch, 0, &Value::Object(objects[8]), Some(&target), Some(&reference), Some(&completed)).unwrap();
        for &id in &objects { assert!(runtime.transient_roots.roots.iter().any(|edge| matches!(edge, Edge::Object(found) if *found == id)), "object {id}"); }
        for &id in &functions { assert!(runtime.transient_roots.roots.iter().any(|edge| matches!(edge, Edge::Function(found) if *found == id)), "function {id}"); }
        assert_eq!(runtime.transient_roots.evaluation_frames, scratch.frames.len() + 1);
        assert_eq!(runtime.transient_roots.frame_slots, scratch.frames.capacity());
        assert_eq!(runtime.transient_roots.argument_slots, scratch.arguments.capacity());
        runtime.restore_transient_marker(marker);
        assert!(runtime.transient_registration_is_empty());
        assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.value_bytes, runtime.job_usage()), before);
    }

    #[test]
    fn evaluator_registration_restores_after_nested_callbacks_csp_errors_and_fatal_limits() {
        struct NoHost;
        impl Host for NoHost {
            fn allow_eval(&mut self) -> Result<bool, String> { Ok(false) }
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
            fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
        }
        for (source, expected_error) in [
            ("var o={value:17,get method(){return function(a){return this.value+a;};}};if(o.method((function(){return 2;})())!==19)throw 'snapshot';0;", None),
            ("var o={get value(){throw 23;}};o.value;", Some("23")),
            ("Function('return 1;');", Some("Content Security Policy")),
            ("for(var i=0;i<1100;i++)(function(){})();", Some("Function limit reached (1024)")),
        ] {
            let mut runtime = Runtime::for_capacity_test();
            let result = runtime.execute(source, &mut NoHost);
            if let Some(expected) = expected_error { assert!(result.unwrap_err().contains(expected), "{source}"); } else { result.unwrap(); }
            assert!(runtime.transient_registration_is_empty(), "{source}");
            assert!(runtime.transient_roots.snapshots > 0);
            assert!(runtime.transient_roots.peak > 0);
            assert!(!runtime.persistent_allocation_report().contains("unavailable"), "{source}");
            runtime.begin_job();
            assert_eq!(runtime.execute("6*7;", &mut NoHost).unwrap().to_number(), 42.0);
        }
    }
}

#[derive(Clone, Copy)]
enum Request {
    Value(ExprId),
    Reference(ExprId),
    Chain(ExprId),
    Operand(ExprId),
}

impl Request {
    fn resolve(self, program: &Program) -> (ExprId, Mode) {
        match self {
            Self::Value(id) => (id, Mode::Value),
            Self::Reference(id) => (id, Mode::Reference),
            Self::Chain(id) => (id, Mode::Chain),
            Self::Operand(id) => (id, match &program.exprs[id].kind {
                ExprKind::Name(_) | ExprKind::Member(..) | ExprKind::SuperMember(_) => Mode::Reference,
                ExprKind::OptionalChain(..) => Mode::Chain,
                _ => Mode::Value,
            }),
        }
    }
}

#[derive(Clone, Copy)]
enum ReadEnd { Value, Reference }
#[derive(Clone, Copy)]
enum ChainEnd { Value, Reference }

// Targets own exactly the receiver/callee values evaluated before arguments.
// No string or reference is cloned merely to suspend the caller.
enum Target {
    Value { function: Value, direct_eval: bool },
    Member { receiver: Value, key: PropertyKey, function: Option<Value> },
    Chain(CallTarget),
    Construct(Value),
    Super(Box<super::derived_context::SuperCallTarget>),
}

enum ResumedExpression {
    Value(Value),
    MemberArgument {
        target: (Value, PropertyKey, Option<Value>),
        first: Value,
    },
}

impl Target {
    fn root_edges(&self, add: &mut impl FnMut(Edge)) {
        match self {
            Self::Value { function, .. } | Self::Construct(function) => Edge::value(function, add),
            Self::Member { receiver, function, .. } => {
                Edge::value(receiver, add);
                if let Some(function) = function { Edge::value(function, add); }
            }
            Self::Chain(target) => {
                Edge::value(&target.receiver, add);
                if let Some(function) = &target.function { Edge::value(function, add); }
            }
            Self::Super(target) => {
                Edge::value(&target.constructor, add);
                add(Edge::Function(target.new_target));
                add(Edge::Environment(target.environment));
            }
        }
    }
}

enum AfterCall<'p> {
    Value,
    Chain { steps: &'p [Step], next: usize, end: ChainEnd },
}

struct Call<'p> {
    target: Target,
    arguments: &'p [Argument],
    index: usize,
    start: usize,
    after: AfterCall<'p>,
}

enum State<'p> {
    Begin(Mode),
    Unary(&'static str),
    Delete,
    BinaryLeft { op: &'static str, right: ExprId },
    BinaryRight { op: &'static str, left: Value },
    Choose { yes: ExprId, no: ExprId },
    Sequence(ExprId),
    ReturnValue,
    ReadBase { key: ExprId, end: ReadEnd },
    ReadKey { receiver: Value, end: ReadEnd },
    AssignmentReference { op: &'static str, left: ExprId, right: ExprId },
    AssignmentValue { op: &'static str, left: ExprId, right: ExprId,
                      reference: Reference, before: Option<Value> },
    PatternValue(super::binding_patterns::Id),
    UpdateReference { increase: bool, prefix: bool },
    Callee { arguments: &'p [Argument], direct_eval: bool, construct: bool },
    StartSuper { target: super::derived_context::SuperCallTarget, arguments: &'p [Argument] },
    SuperCallee(&'p [Argument]),
    MemberBase { key: ExprId, arguments: &'p [Argument] },
    MemberKey { receiver: Value, arguments: &'p [Argument] },
    GroupedCall(&'p [Argument]),
    Argument(Call<'p>),
    ChainNext { steps: &'p [Step], next: usize, end: ChainEnd },
    ChainKey { receiver: Value, steps: &'p [Step], next: usize, end: ChainEnd },
}

impl State<'_> {
    fn root_edges(&self, add: &mut impl FnMut(Edge)) {
        match self {
            Self::BinaryRight { left, .. } => Edge::value(left, add),
            Self::ReadKey { receiver, .. } | Self::MemberKey { receiver, .. }
                | Self::ChainKey { receiver, .. } => Edge::value(receiver, add),
            Self::AssignmentValue { reference, before, .. } => {
                Edge::reference(reference, add);
                if let Some(value) = before { Edge::value(value, add); }
            }
            Self::Argument(call) => call.target.root_edges(add),
            Self::StartSuper { target, .. } => {
                Edge::value(&target.constructor, add);
                add(Edge::Function(target.new_target));
                add(Edge::Environment(target.environment));
            }
            _ => {}
        }
    }
}

struct Frame<'p> {
    id: ExprId,
    state: State<'p>,
    // Reference construction historically leaves location attribution to its
    // caller; evaluated receivers/keys still retain their own error locations.
    annotate: bool,
}

// A single argument buffer serves every suspended call in this driver. Inner
// calls append a suffix and truncate it after invocation; earlier outer values
// remain untouched. Capacity is shared-accounted even while a callback runs.
struct Scratch<'p> {
    frames: Vec<Frame<'p>>,
    arguments: Vec<Value>,
}

enum Transition<'p> {
    Child(Request, State<'p>),
    Resume(State<'p>, Completion),
    Tail(ExprId),
    Done(Completion),
    // Callback-capable terminal actions run only after the large transition
    // builder has returned. Pending native calls must not retain its locals.
    Invoke(Call<'p>),
    Spread(Call<'p>, Value),
    Read(Reference, ReadEnd),
    Immediate(ExprId),
}

fn value(completion: Completion) -> Value {
    match completion {
        Completion::Value(value) => value,
        Completion::Reference(_) => unreachable!("value continuation received a reference"),
    }
}

fn done<'p>(value: Value) -> Transition<'p> {
    Transition::Done(Completion::Value(value))
}

fn reserve<T>(items: &mut Vec<T>, total: &mut usize, limit: usize) -> Result<(), Fault> {
    if items.len() < items.capacity() { return Ok(()); }
    let old = items.capacity();
    let available = limit.saturating_sub(*total);
    if available == 0 {
        return Err(Fault::new("Evaluation scratch storage limit reached."));
    }
    let additional = old.max(8).min(available);
    items.try_reserve_exact(additional)
        .map_err(|_| Fault::new("Evaluation scratch allocation failed."))?;
    let added = items.capacity() - old;
    if added > available {
        return Err(Fault::new("Evaluation scratch storage limit reached."));
    }
    *total += added;
    Ok(())
}

fn compact<T>(items: &mut Vec<T>, total: &mut usize) {
    if items.capacity() > (items.len().saturating_mul(2)).max(64) {
        let old = items.capacity();
        items.shrink_to(items.len().max(8));
        *total -= old - items.capacity();
    }
}

impl Runtime {
    fn publish_heap_roots(&mut self, scratch: &Scratch<'_>, env: usize, this: &Value,
        target: Option<&Target>, reference: Option<&Reference>, completed: Option<&Completion>) -> Result<Marker, Fault> {
        let marker = self.transient_marker();
        let mut failure = None;
        let mut add = |edge| {
            if failure.is_none() {
                if let Err(error) = self.push_transient_root(edge) { failure = Some(error); }
            }
        };
        add(Edge::Environment(env));
        Edge::value(this, &mut add);
        for frame in &scratch.frames { frame.state.root_edges(&mut add); }
        for value in &scratch.arguments { Edge::value(value, &mut add); }
        if let Some(target) = target { target.root_edges(&mut add); }
        if let Some(reference) = reference { Edge::reference(reference, &mut add); }
        if let Some(completed) = completed {
            match completed { Completion::Value(value) => Edge::value(value, &mut add), Completion::Reference(reference) => Edge::reference(reference, &mut add) }
        }
        drop(add);
        if let Some(error) = failure { self.restore_transient_marker(marker); return Err(error); }
        // The popped terminal frame remains in the shared count until Done.
        self.transient_roots.evaluation_frames += scratch.frames.len() + 1;
        self.transient_roots.frame_slots += scratch.frames.capacity();
        self.transient_roots.argument_slots += scratch.arguments.capacity();
        self.transient_roots.snapshots = self.transient_roots.snapshots.saturating_add(1);
        Ok(marker)
    }

    pub(super) fn eval_heap(
        &mut self, program: &Rc<Program>, id: ExprId, env: usize,
        this: &Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if let ExprKind::Function(template) = program.exprs[id].kind {
            return self.eval_heap_function_literal(program, template, env, this);
        }
        if let ExprKind::Class(class) = program.exprs[id].kind {
            return self.eval_heap_class_literal(program, class, env, this, host);
        }
        // Compound helpers already own their traversal state. Do not retain
        // this driver's large Frame/Transition temporaries while they invoke
        // iterator getters or other callbacks. Children still reenter eval
        // through its native guard and share the heap-frame quota.
        if matches!(&program.exprs[id].kind, ExprKind::Literal(_) | ExprKind::Name(_)
            | ExprKind::This | ExprKind::NewTarget | ExprKind::RegExp(_)
            | ExprKind::Array(_) | ExprKind::Object(_) | ExprKind::TemplateLiteral(..)
            | ExprKind::TemplateObject(_)) {
            if self.evaluation_frames >= MAX_EVALUATION_FRAMES {
                return Err(Fault::new("Pending evaluation limit reached (4096)."));
            }
            self.evaluation_frames += 1;
            let marker = self.inhibit_transient_collection_for("immediate-expression");
            let result = self.eval_heap_immediate(program, id, env, this, host);
            self.restore_transient_marker(marker);
            self.evaluation_frames -= 1;
            return result;
        }
        self.eval_heap_continuations(program, id, None, env, this, host)
    }

    pub(super) fn eval_resumed_expression(
        &mut self, program: &Rc<Program>, id: ExprId, value: Value, env: usize,
        this: &Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.eval_resumed(program, id, ResumedExpression::Value(value), env, this, host)
    }

    pub(super) fn eval_resumed_member_argument(
        &mut self, program: &Rc<Program>, id: ExprId, target: (Value, PropertyKey, Option<Value>),
        first: Value, env: usize, this: &Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.eval_resumed(program, id, ResumedExpression::MemberArgument { target, first }, env, this, host)
    }

    fn eval_resumed(
        &mut self, program: &Rc<Program>, id: ExprId, mut expression: ResumedExpression, env: usize,
        this: &Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        if self.eval_depth >= MAX_EVAL_DEPTH {
            return Err(Fault::new("Evaluation nesting limit reached (128)."));
        }
        self.eval_depth += 1;
        self.transient_roots.owned_eval_depth += 1;
        let result = self.eval_heap_continuations(program, id, Some(&mut expression), env, this, host);
        self.transient_roots.owned_eval_depth -= 1;
        self.eval_depth -= 1;
        result
    }

    #[inline(always)]
    fn eval_heap_continuations(
        &mut self, program: &Rc<Program>, id: ExprId, resumed: Option<&mut ResumedExpression>,
        env: usize, this: &Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let baseline = (self.evaluation_frames, self.evaluation_frame_slots,
                        self.evaluation_argument_slots);
        let result = self.eval_heap_driver(program, id, resumed, env, this, host);
        // Each driver has dropped only its own scratch. Restore the enclosing
        // driver's reservations on success, catchable errors and fatal exits.
        self.evaluation_frames = baseline.0;
        self.evaluation_frame_slots = baseline.1;
        self.evaluation_argument_slots = baseline.2;
        result
    }

    #[inline(never)]
    fn eval_heap_function_literal(&mut self, program: &Rc<Program>, template: usize,
        env: usize, this: &Value) -> Result<Value, Fault> {
        if self.evaluation_frames >= MAX_EVALUATION_FRAMES {
            return Err(Fault::new("Pending evaluation limit reached (4096)."));
        }
        self.evaluation_frames += 1;
        // A standalone literal has no suspended heap state of its own.
        // Publish its actual one frame without introducing driver storage.
        // Keep this scratch out of every other expression's native activation.
        let scratch = Scratch { frames: Vec::new(), arguments: Vec::new() };
        let result = match self.publish_heap_roots(&scratch, env, this, None, None, None) {
            Ok(marker) => {
                let result = self.collect_before_function_literal()
                    .and_then(|()| self.make_function(program, template, env, this));
                self.restore_transient_marker(marker);
                result
            }
            Err(error) => Err(error),
        };
        self.evaluation_frames -= 1;
        result
    }

    // Keep this dispatch independent from the recursive test oracle. That
    // oracle handles every expression kind and retains unnecessary temporary
    // state during callback reentry in unoptimized Windows builds.
    #[inline(never)]
    fn eval_heap_class_literal(&mut self, program: &Rc<Program>, class: usize,
        env: usize, this: &Value, host: &mut dyn Host) -> Result<Value, Fault> {
        if self.evaluation_frames >= MAX_EVALUATION_FRAMES {
            return Err(Fault::new("Pending evaluation limit reached (4096)."));
        }
        self.evaluation_frames += 1;
        let scratch = Scratch { frames: Vec::new(), arguments: Vec::new() };
        let result = match self.publish_heap_roots(&scratch, env, this, None, None, None) {
            Ok(marker) => {
                let result = self.collect_before_function_literal()
                    .and_then(|()| self.make_class(program, class, env, this, host));
                self.restore_transient_marker(marker);
                result
            }
            Err(error) => Err(error),
        };
        self.evaluation_frames -= 1;
        result
    }

    #[inline(never)]
    fn eval_heap_immediate(
        &mut self, program: &Rc<Program>, id: ExprId, env: usize,
        this: &Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match &program.exprs[id].kind {
            ExprKind::Literal(value) => self.copy_value(value),
            ExprKind::Name(name) => match self.lookup(env, name)? {
                Some(scope) => self.read_binding(scope, name, host),
                None if self.global_exists(name, host)? => self.global_value(name, host),
                None => Err(Fault::exception("ReferenceError", format!("{name} is not defined."))),
            },
            ExprKind::This => self.context_this(this),
            ExprKind::NewTarget => self.new_target_value(),
            ExprKind::Function(template) => self.make_function(program, *template, env, this),
            ExprKind::Class(class) => self.make_class(program, *class, env, this, host),
            ExprKind::SuperMember(name) => {
                let reference = self.super_reference(name)?;
                self.read_reference(&reference, host)
            }
            ExprKind::RegExp(expression) => self.regexp_object(expression.clone()),
            ExprKind::Array(elements) => self.eval_array_literal(program, elements, env, this, host),
            ExprKind::Object(properties) => self.eval_object_literal(program, properties, env, this, host),
            ExprKind::TemplateLiteral(head, spans) => self.eval_template(program, head, spans, env, this, host),
            ExprKind::TemplateObject(site) => self.eval_template_object(program, site),
            ExprKind::Pattern(_) => Err(Fault::new("A destructuring target is not an expression.")),
            _ => unreachable!("continuation expression routed to an immediate helper"),
        }
    }

    fn push_evaluation<'p>(
        &mut self, scratch: &mut Scratch<'p>, program: &Program,
        request: Request, already_charged: bool,
    ) -> Result<(), Fault> {
        if !already_charged { self.step()?; }
        if self.evaluation_frames >= MAX_EVALUATION_FRAMES {
            return Err(Fault::new("Pending evaluation limit reached (4096)."));
        }
        reserve(&mut scratch.frames, &mut self.evaluation_frame_slots,
                MAX_EVALUATION_FRAME_SLOTS)?;
        let (id, mode) = request.resolve(program);
        scratch.frames.push(Frame {
            id,
            state: State::Begin(mode),
            annotate: !matches!(mode, Mode::Reference),
        });
        self.evaluation_frames += 1;
        Ok(())
    }

    #[inline(never)]
    fn eval_heap_driver(
        &mut self, program: &Rc<Program>, id: ExprId, resumed: Option<&mut ResumedExpression>, env: usize,
        this: &Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let mut scratch = Scratch { frames: Vec::new(), arguments: Vec::new() };
        // Runtime::eval already charged the root expression and checked native
        // reentry. Children on this driver never recurse into Runtime::eval.
        self.push_evaluation(&mut scratch, program, Request::Value(id), true)?;
        let mut completed = None;
        if let Some(expression) = resumed {
            Self::start_resumed_expression(program, id, expression, &mut scratch, &mut completed);
        }
        while let Some(mut frame) = scratch.frames.pop() {
            compact(&mut scratch.frames, &mut self.evaluation_frame_slots);
            compact(&mut scratch.arguments, &mut self.evaluation_argument_slots);
            let at = program.exprs[frame.id].at;
            let error_at = if frame.annotate {
                Some(at)
            } else {
                scratch.frames.iter().rev().find(|parent| parent.annotate)
                    .map(|parent| program.exprs[parent.id].at)
            };
            let located = |error: Fault| match error_at {
                Some(position) => error.at(&program.source, position),
                None => error,
            };
            let inhibited = self.inhibit_transient_collection_for("evaluator-dispatch");
            let transition = match frame.state {
                State::Begin(mode) => self.begin_heap_expression(
                    program, frame.id, mode, env, this, host),
                state => self.resume_heap_expression(
                    program, state, completed.take().expect("completed child"),
                    &mut scratch, env, this, host),
            };
            self.restore_transient_marker(inhibited);
            let transition = transition.map_err(located)?;
            // Finish callback-capable actions outside begin/resume dispatch.
            // This loop is iterative even when a spread ends directly in a call.
            let mut pending = transition;
            let transition = loop {
                pending = match pending {
                    Transition::Invoke(call) => {
                        let marker = self.publish_heap_roots(&scratch, env, this, Some(&call.target), None, completed.as_ref()).map_err(located)?;
                        let result = self.finish_heap_call(call, &mut scratch, env, this, host);
                        self.restore_transient_marker(marker);
                        result.map_err(located)?
                    }
                    Transition::Spread(call, result) => {
                        let marker = self.inhibit_transient_collection_for("spread-arguments");
                        let result = self.finish_heap_spread(call, result, &mut scratch, env, this, host);
                        self.restore_transient_marker(marker);
                        result.map_err(located)?
                    }
                    Transition::Immediate(id) => {
                        let result = self.finish_heap_immediate(program, id, &scratch, env, this, completed.as_ref(), host);
                        done(result.map_err(located)?)
                    }
                    Transition::Read(reference, end) => {
                        let marker = self.publish_heap_roots(&scratch, env, this, None, Some(&reference), completed.as_ref()).map_err(located)?;
                        let result = match end {
                            ReadEnd::Value => self.read_reference(&reference, host).map(done),
                            ReadEnd::Reference => Ok(Transition::Done(Completion::Reference(reference))),
                        };
                        self.restore_transient_marker(marker);
                        result.map_err(located)?
                    }
                    ready => break ready,
                };
            };
            match transition {
                Transition::Child(request, state) => {
                    frame.state = state;
                    // The popped parent needs a slot again after compaction.
                    reserve(&mut scratch.frames, &mut self.evaluation_frame_slots,
                            MAX_EVALUATION_FRAME_SLOTS)?;
                    scratch.frames.push(frame);
                    self.push_evaluation(&mut scratch, program, request, false)
                        .map_err(located)?;
                }
                Transition::Resume(state, result) => {
                    frame.state = state;
                    reserve(&mut scratch.frames, &mut self.evaluation_frame_slots,
                            MAX_EVALUATION_FRAME_SLOTS)?;
                    scratch.frames.push(frame);
                    completed = Some(result);
                }
                Transition::Tail(next) => {
                    self.step().map_err(|error| error.at(&program.source, program.exprs[next].at))?;
                    frame.id = next;
                    frame.state = State::Begin(Mode::Value);
                    frame.annotate = true;
                    reserve(&mut scratch.frames, &mut self.evaluation_frame_slots,
                            MAX_EVALUATION_FRAME_SLOTS)?;
                    scratch.frames.push(frame);
                }
                Transition::Done(result) => {
                    self.evaluation_frames -= 1;
                    completed = Some(result);
                }
                Transition::Invoke(_) | Transition::Spread(..) | Transition::Read(..)
                    | Transition::Immediate(_) => unreachable!(),
            }
        }
        debug_assert!(scratch.arguments.is_empty());
        Ok(value(completed.expect("root expression completion")))
    }

    // Keep resumed-value setup out of ordinary evaluator frames.
    #[inline(never)]
    fn start_resumed_expression<'p>(
        program: &'p Program, id: ExprId, resumed: &mut ResumedExpression,
        scratch: &mut Scratch<'p>, completed: &mut Option<Completion>,
    ) {
        let (state, value) = match std::mem::replace(resumed, ResumedExpression::Value(Value::Undefined)) {
            ResumedExpression::Value(value) => {
                let state = match &program.exprs[id].kind {
                    ExprKind::OptionalChain(_, steps) => State::ChainNext { steps, next: 0, end: ChainEnd::Value },
                    ExprKind::Call(callee, arguments) => match program.exprs[*callee].kind {
                        ExprKind::Member(_, key) => State::MemberBase { key, arguments },
                        // A function-valued result has no receiver or direct eval reference.
                        _ => State::Callee { arguments, direct_eval: false, construct: false },
                    },
                    _ => unreachable!("a resumed value needs a call or optional chain"),
                };
                (state, value)
            }
            ResumedExpression::MemberArgument { target: (receiver, key, function), first } => {
                let ExprKind::Call(_, arguments) = &program.exprs[id].kind else {
                    unreachable!("a resumed member argument needs a call");
                };
                let call = Call {
                    target: Target::Member { receiver, key, function },
                    arguments, index: 0, start: 0, after: AfterCall::Value,
                };
                (State::Argument(call), first)
            }
        };
        scratch.frames[0].state = state;
        *completed = Some(Completion::Value(value));
    }

    #[inline(never)]
    fn finish_heap_immediate(&mut self, program: &Rc<Program>, id: ExprId,
        scratch: &Scratch<'_>, env: usize, this: &Value, completed: Option<&Completion>,
        host: &mut dyn Host) -> Result<Value, Fault> {
        // Keep all terminal-only scratch out of every recursive Invoke. Only
        // Function literals publish a complete snapshot and become safepoints;
        // compound/native helpers retain their existing blanket inhibitor.
        if let ExprKind::Function(template) = program.exprs[id].kind {
            let marker = self.publish_heap_roots(scratch, env, this, None, None, completed)?;
            let result = self.collect_before_function_literal()
                .and_then(|()| self.make_function(program, template, env, this));
            self.restore_transient_marker(marker);
            result
        } else if let ExprKind::Class(class) = program.exprs[id].kind {
            let marker = self.publish_heap_roots(scratch, env, this, None, None, completed)?;
            let result = self.collect_before_function_literal()
                .and_then(|()| self.make_class(program, class, env, this, host));
            self.restore_transient_marker(marker);
            result
        } else {
            let marker = self.inhibit_transient_collection_for("immediate-expression");
            let result = self.eval_heap_immediate(program, id, env, this, host);
            self.restore_transient_marker(marker);
            result
        }
    }

    #[inline(never)]
    fn begin_heap_expression<'p>(
        &mut self, program: &'p Rc<Program>, id: ExprId, mode: Mode,
        env: usize, this: &Value, host: &mut dyn Host,
    ) -> Result<Transition<'p>, Fault> {
        if matches!(mode, Mode::Reference) {
            return match &program.exprs[id].kind {
                ExprKind::Name(name) => Ok(Transition::Done(Completion::Reference(
                    match self.lookup(env, name)? {
                        Some(scope) => Reference::Binding(scope, name.clone()),
                        None if self.global_exists(name, host)? => Reference::Global(name.clone()),
                        None => Reference::Missing(name.clone()),
                    }))),
                ExprKind::Member(base, key) => Ok(Transition::Child(Request::Value(*base),
                    State::ReadBase { key: *key, end: ReadEnd::Reference })),
                ExprKind::SuperMember(name) => Ok(Transition::Done(Completion::Reference(self.super_reference(name)?))),
                _ => Err(Fault::new("Invalid assignment reference.")),
            };
        }
        if matches!(mode, Mode::Chain) {
            let ExprKind::OptionalChain(base, steps) = &program.exprs[id].kind else { unreachable!() };
            return Ok(Transition::Child(Request::Operand(*base),
                State::ChainNext { steps, next: 0, end: ChainEnd::Reference }));
        }
        let next = match &program.exprs[id].kind {
            ExprKind::Binary(op, left, right) => Transition::Child(Request::Value(*left),
                State::BinaryLeft { op: *op, right: *right }),
            ExprKind::Conditional(condition, yes, no) => Transition::Child(Request::Value(*condition),
                State::Choose { yes: *yes, no: *no }),
            ExprKind::Sequence(first, last) => Transition::Child(Request::Value(*first), State::Sequence(*last)),
            ExprKind::Unary(op, argument) => {
                if *op == "delete" {
                    let request = match &program.exprs[*argument].kind {
                        ExprKind::Name(_) | ExprKind::Member(..) | ExprKind::SuperMember(_) => Request::Reference(*argument),
                        ExprKind::OptionalChain(..) => Request::Chain(*argument),
                        _ => Request::Value(*argument),
                    };
                    Transition::Child(request, State::Delete)
                } else {
                    if *op == "typeof" {
                        if let ExprKind::Name(name) = &program.exprs[*argument].kind {
                            if self.lookup(env, name)?.is_none() && !self.global_exists(name, host)? {
                                return Ok(done(Value::String("undefined".into())));
                            }
                        }
                    }
                    Transition::Child(Request::Value(*argument), State::Unary(*op))
                }
            }
            ExprKind::Member(base, key) => {
                if matches!(&program.exprs[*base].kind, ExprKind::Name(_))
                    && matches!(&program.exprs[*key].kind, ExprKind::Literal(Value::String(key)) if key == "length") {
                    return self.eval_binding_length(program, *base, *key, env, this, host).map(done);
                }
                self.step()?; // reference construction, before receiver evaluation
                Transition::Child(Request::Value(*base), State::ReadBase { key: *key, end: ReadEnd::Value })
            }
            ExprKind::SuperMember(name) => Transition::Read(self.super_reference(name)?, ReadEnd::Value),
            ExprKind::SuperCall(arguments) => Transition::Resume(
                State::StartSuper { target: self.prepare_super_call()?, arguments }, Completion::Value(Value::Undefined)),
            ExprKind::Assign(op, left, right) => {
                if let ExprKind::Pattern(pattern) = program.exprs[*left].kind {
                    Transition::Child(Request::Value(*right), State::PatternValue(pattern))
                } else {
                    Transition::Child(Request::Reference(*left), State::AssignmentReference {
                        op: *op, left: *left, right: *right,
                    })
                }
            }
            ExprKind::Update(target, increase, prefix) => Transition::Child(Request::Reference(*target),
                State::UpdateReference { increase: *increase, prefix: *prefix }),
            ExprKind::Call(callee, arguments) => match &program.exprs[*callee].kind {
                ExprKind::Member(base, key) => {
                    if let Some(result) = self.try_binding_character(program, *base, *key, arguments, env, host) {
                        return result.map(done);
                    }
                    Transition::Child(Request::Value(*base), State::MemberBase { key: *key, arguments })
                }
                ExprKind::OptionalChain(..) => Transition::Child(Request::Chain(*callee), State::GroupedCall(arguments)),
                ExprKind::SuperMember(_) => Transition::Child(Request::Reference(*callee), State::SuperCallee(arguments)),
                kind => Transition::Child(Request::Value(*callee), State::Callee { arguments,
                    direct_eval: matches!(kind, ExprKind::Name(name) if name == "eval"), construct: false }),
            },
            ExprKind::Construct(callee, arguments) => Transition::Child(Request::Value(*callee),
                State::Callee { arguments, direct_eval: false, construct: true }),
            ExprKind::OptionalChain(base, steps) => {
                self.step()?; // chain_reference, in addition to the enclosing eval
                Transition::Child(Request::Operand(*base),
                    State::ChainNext { steps, next: 0, end: ChainEnd::Value })
            }
            // Return before entering a helper that can call page code. In
            // particular array-spread iteration must not suspend this builder.
            _ => Transition::Immediate(id),
        };
        Ok(next)
    }

    #[inline(never)]
    fn resume_heap_expression<'p>(
        &mut self, program: &'p Rc<Program>, state: State<'p>, completion: Completion,
        scratch: &mut Scratch<'p>, env: usize, this: &Value, host: &mut dyn Host,
    ) -> Result<Transition<'p>, Fault> {
        match state {
            state @ (State::Callee { .. } | State::MemberBase { .. }
                | State::MemberKey { .. } | State::GroupedCall(_)
                | State::StartSuper { .. } | State::SuperCallee(_)
                | State::Argument(_) | State::ChainNext { .. } | State::ChainKey { .. }) =>
                self.resume_heap_call(program, state, completion, scratch, env, this, host),
            state => self.resume_heap_value(program, state, completion, scratch, env, this, host),
        }
    }

    #[inline(never)]
    fn resume_heap_value<'p>(
        &mut self, program: &'p Rc<Program>, state: State<'p>, completion: Completion,
        _scratch: &mut Scratch<'p>, env: usize, this: &Value, host: &mut dyn Host,
    ) -> Result<Transition<'p>, Fault> {
        match state {
            State::Begin(_) => unreachable!(),
            State::Unary(op) => self.heap_unary(op, value(completion), host).map(done),
            State::Delete => match completion {
                Completion::Reference(reference) => self.delete_reference(reference, host).map(done),
                Completion::Value(_) => Ok(done(Value::Bool(true))),
            },
            State::BinaryLeft { op, right } => {
                let left = value(completion);
                if logical_short_circuits(op, &left) {
                    return Ok(done(left));
                }
                Ok(Transition::Child(Request::Value(right), State::BinaryRight { op, left }))
            }
            State::BinaryRight { op, left } => {
                let right = value(completion);
                let result = match op {
                    "&&" | "||" | "??" => Ok(right),
                    "in" => self.has_property_value(left, right, host),
                    "instanceof" => self.instance_of(left, right, host),
                    _ => self.binary_value(op, left, right, host),
                };
                result.map(done)
            }
            State::Choose { yes, no } => Ok(Transition::Tail(if value(completion).truthy() { yes } else { no })),
            State::Sequence(last) => Ok(Transition::Child(Request::Value(last), State::ReturnValue)),
            State::ReturnValue => Ok(Transition::Done(completion)),
            State::ReadBase { key, end } => Ok(Transition::Child(Request::Value(key),
                State::ReadKey { receiver: value(completion), end })),
            State::ReadKey { receiver, end } => {
                let reference = Reference::ComputedProperty(receiver, value(completion));
                Ok(Transition::Read(reference, end))
            }
            State::AssignmentReference { op, left, right } => {
                let Completion::Reference(reference) = completion else { unreachable!() };
                let logical = matches!(op, "&&=" | "||=" | "??=");
                let reference = if logical || op == "**=" {
                    self.resolved_operator_reference(reference, host)?
                } else { reference };
                let before = if op == "=" { None } else {
                    let before = self.read_reference(&reference, host)?;
                    if (op == "&&=" && !before.truthy()) || (op == "||=" && before.truthy())
                        || (op == "??=" && !matches!(before, Value::Null | Value::Undefined)) {
                        return Ok(done(before));
                    }
                    (!logical).then_some(before)
                };
                Ok(Transition::Child(Request::Value(right), State::AssignmentValue {
                    op, left, right, reference, before,
                }))
            }
            State::AssignmentValue { op, left, right, reference, before } => {
                let mut result = value(completion);
                if op == "=" {
                    if let ExprKind::Name(name) = &program.exprs[left].kind {
                        self.infer_class_name(program, right, name, &result)?;
                    }
                }
                if matches!(op, "&&=" | "||=" | "??=") {
                    self.name_logical_assignment_result(program, left, right, &result)?;
                }
                if let Some(before) = before {
                    result = self.binary_value(&op[..op.len() - 1], before, result, host)?;
                }
                let returned = self.copy_value(&result)?;
                let candidate = if op == "=" {
                    self.direct_object_closure_assignment(program, right, &reference, &result)
                } else { None };
                let scope_candidate = if op == "=" && matches!(&reference, Reference::Binding(_, _)) {
                    self.fresh_anonymous_scope_closure(program, right, &result)
                } else { None };
                self.write_reference(reference, result, host)?;
                if let Some(id) = candidate { self.object_assigned_closures.insert(id); }
                if let Some(id) = scope_candidate { self.scope_assigned_closures.insert(id); }
                Ok(done(returned))
            }
            State::PatternValue(pattern) => {
                let result = value(completion);
                let returned = self.copy_value(&result)?;
                self.initialize_pattern(program, pattern, result,
                    super::binding_patterns::Mode::Assign, env, this, host)?;
                Ok(done(returned))
            }
            State::UpdateReference { increase, prefix } => {
                let Completion::Reference(reference) = completion else { unreachable!() };
                let previous = self.read_reference(&reference, host)?;
                let previous = self.primitive_with_hint(previous, false, host)?;
                let before = previous.script_number()?;
                let after = before + if increase { 1.0 } else { -1.0 };
                self.write_reference(reference, Value::Number(after), host)?;
                Ok(done(Value::Number(if prefix { after } else { before })))
            }
            _ => unreachable!(),
        }
    }

    #[inline(never)]
    fn resume_heap_call<'p>(
        &mut self, _program: &'p Rc<Program>, state: State<'p>, completion: Completion,
        scratch: &mut Scratch<'p>, env: usize, this: &Value, host: &mut dyn Host,
    ) -> Result<Transition<'p>, Fault> {
        match state {
            State::StartSuper { target, arguments } => {
                self.work_as(std::mem::size_of::<super::derived_context::SuperCallTarget>(), super::WorkKind::ArgumentCapture)?;
                self.next_heap_argument(Call {
                    target: Target::Super(Box::new(target)), arguments, index: 0, start: scratch.arguments.len(), after: AfterCall::Value,
                }, scratch, env, this, host)
            }
            State::SuperCallee(arguments) => {
                let Completion::Reference(reference) = completion else { unreachable!() };
                let function = self.read_reference(&reference, host)?;
                let Reference::SuperProperty { receiver, name, .. } = reference else { unreachable!() };
                self.next_heap_argument(Call { target: Target::Member {
                    receiver, key: PropertyKey::String(name.into_string()), function: Some(function),
                }, arguments, index: 0, start: scratch.arguments.len(), after: AfterCall::Value }, scratch, env, this, host)
            }
            State::Callee { arguments, direct_eval, construct } => {
                let function = value(completion);
                let target = if construct { Target::Construct(function) }
                    else { Target::Value { function, direct_eval } };
                self.next_heap_argument(Call { target, arguments, index: 0,
                    start: scratch.arguments.len(), after: AfterCall::Value }, scratch, env, this, host)
            }
            State::MemberBase { key, arguments } => Ok(Transition::Child(Request::Value(key),
                State::MemberKey { receiver: value(completion), arguments })),
            State::MemberKey { receiver, arguments } => {
                let (receiver, key, function) = self.resolve_member_call(receiver, value(completion), host)?;
                self.next_heap_argument(Call { target: Target::Member { receiver, key, function },
                    arguments, index: 0, start: scratch.arguments.len(), after: AfterCall::Value },
                    scratch, env, this, host)
            }
            State::GroupedCall(arguments) => {
                let target = self.prepare_chain_call(completion, false, host)?;
                self.next_heap_argument(Call { target: Target::Chain(target), arguments, index: 0,
                    start: scratch.arguments.len(), after: AfterCall::Value }, scratch, env, this, host)
            }
            State::Argument(mut call) => {
                let argument = call.arguments[call.index];
                let result = value(completion);
                if argument.spread {
                    return Ok(Transition::Spread(call, result));
                }
                self.push_heap_argument(scratch, result)?;
                call.index += 1;
                self.next_heap_argument(call, scratch, env, this, host)
            }
            State::ChainNext { steps, next, end } => {
                self.next_heap_chain(steps, next, end, completion, scratch, env, this, host)
            }
            State::ChainKey { receiver, steps, next, end } => Ok(Transition::Resume(
                State::ChainNext { steps, next, end },
                Completion::Reference(Reference::ComputedProperty(receiver, value(completion))))),
            _ => unreachable!(),
        }
    }

    fn heap_unary(&mut self, op: &str, value: Value, host: &mut dyn Host) -> Result<Value, Fault> {
        match op {
            "!" => Ok(Value::Bool(!value.truthy())),
            "void" => Ok(Value::Undefined),
            "typeof" => Ok(Value::String(match value {
                Value::Undefined => "undefined", Value::Null | Value::Object(_) | Value::Host(_) => "object",
                Value::Bool(_) => "boolean", Value::Number(_) => "number", Value::String(_) | Value::StringUnits(_) => "string",
                Value::Symbol(_) => "symbol", Value::Function(_) => "function",
            }.into())),
            "+" | "-" | "~" => {
                let primitive = self.primitive_with_hint(value, false, host)?;
                let number = primitive.script_number()?;
                Ok(Value::Number(match op { "+" => number, "-" => -number, _ => f64::from(!super::int32(number)) }))
            }
            _ => Err(Fault::new("Unsupported unary operator.")),
        }
    }

    fn push_heap_argument(&mut self, scratch: &mut Scratch<'_>, value: Value) -> Result<(), Fault> {
        let limit = self.argument_scratch_limit()?;
        reserve(&mut scratch.arguments, &mut self.evaluation_argument_slots,
                limit)?;
        scratch.arguments.push(value);
        Ok(())
    }

    #[inline(never)]
    fn append_heap_spread(
        &mut self, scratch: &mut Scratch<'_>, start: usize, value: Value, host: &mut dyn Host,
    ) -> Result<(), Fault> {
        let mut iterator = self.spread_iterator(value, host)?;
        while let Some(value) = self.iterator_next(&mut iterator, host)? {
            if scratch.arguments.len() - start == self.limits.capacity.arguments {
                return Err(Fault::new(format!("Call argument limit reached ({}).", self.limits.capacity.arguments)));
            }
            self.push_heap_argument(scratch, value)?;
        }
        Ok(())
    }

    #[inline(never)]
    fn next_heap_argument<'p>(
        &mut self, call: Call<'p>, scratch: &mut Scratch<'p>, _env: usize,
        _this: &Value, _host: &mut dyn Host,
    ) -> Result<Transition<'p>, Fault> {
        if let Some(argument) = call.arguments.get(call.index) {
            if !argument.spread && scratch.arguments.len() - call.start == self.limits.capacity.arguments {
                return Err(Fault::new(format!("Call argument limit reached ({}).", self.limits.capacity.arguments)));
            }
            return Ok(Transition::Child(Request::Value(argument.expression), State::Argument(call)));
        }
        Ok(Transition::Invoke(call))
    }

    #[inline(never)]
    fn finish_heap_spread<'p>(
        &mut self, mut call: Call<'p>, value: Value, scratch: &mut Scratch<'p>,
        env: usize, this: &Value, host: &mut dyn Host,
    ) -> Result<Transition<'p>, Fault> {
        self.append_heap_spread(scratch, call.start, value, host)?;
        call.index += 1;
        self.next_heap_argument(call, scratch, env, this, host)
    }

    #[inline(never)]
    fn finish_heap_call<'p>(
        &mut self, call: Call<'p>, scratch: &mut Scratch<'p>, env: usize,
        this: &Value, host: &mut dyn Host,
    ) -> Result<Transition<'p>, Fault> {
        let result = self.invoke_heap_target(call.target, &scratch.arguments[call.start..], env, this, host);
        scratch.arguments.truncate(call.start);
        let result = Completion::Value(result?);
        Ok(match call.after {
            AfterCall::Value => Transition::Done(result),
            AfterCall::Chain { steps, next, end } => Transition::Resume(
                State::ChainNext { steps, next, end }, result),
        })
    }

    #[inline(never)]
    fn invoke_heap_target(
        &mut self, target: Target, arguments: &[Value], env: usize,
        this: &Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match target {
            Target::Value { function, direct_eval } => {
                self.invoke_plain_call(function, arguments, direct_eval.then_some((env, this)), host)
            }
            Target::Member { receiver, key, function } => self.invoke_member_call(receiver, key, function, arguments, host),
            Target::Chain(target) => self.invoke_chain_call(target, arguments, host),
            Target::Construct(constructor) => self.construct_value(constructor, arguments, host),
            Target::Super(target) => self.invoke_heap_super_target(target, arguments, host),
        }
    }

    #[inline(never)]
    fn invoke_heap_super_target(&mut self, target: Box<super::derived_context::SuperCallTarget>, arguments: &[Value], host: &mut dyn Host) -> Result<Value, Fault> {
        // Keep unboxing and construction-only scratch outside the shared target
        // dispatcher. Its super arm carries a pointer instead of an inline
        // constructor Value through every ordinary Invoke activation.
        self.finish_super_call(*target, arguments, host)
    }

    fn finish_heap_chain<'p>(&mut self, current: Completion, end: ChainEnd, _host: &mut dyn Host)
        -> Result<Transition<'p>, Fault> {
        Ok(match (end, current) {
            (ChainEnd::Value, Completion::Reference(reference)) =>
                Transition::Read(reference, ReadEnd::Value),
            (_, current) => Transition::Done(current),
        })
    }

    #[inline(never)]
    fn next_heap_chain<'p>(
        &mut self, steps: &'p [Step], next: usize, end: ChainEnd, current: Completion,
        scratch: &mut Scratch<'p>, env: usize, this: &Value, host: &mut dyn Host,
    ) -> Result<Transition<'p>, Fault> {
        let Some(step) = steps.get(next) else { return self.finish_heap_chain(current, end, host); };
        self.step()?;
        match step {
            Step::Member { key, optional } => {
                let receiver = self.chain_value(current, host)?;
                if *optional && matches!(receiver, Value::Null | Value::Undefined) {
                    return self.finish_heap_chain(Completion::Value(Value::Undefined), end, host);
                }
                Ok(Transition::Child(Request::Value(*key), State::ChainKey {
                    receiver, steps, next: next + 1, end,
                }))
            }
            Step::Call { args, optional } => {
                let target = self.prepare_chain_call(current, *optional, host)?;
                if *optional && matches!(target.function.as_ref(), Some(Value::Null | Value::Undefined)) {
                    return self.finish_heap_chain(Completion::Value(Value::Undefined), end, host);
                }
                self.next_heap_argument(Call { target: Target::Chain(target), arguments: args,
                    index: 0, start: scratch.arguments.len(), after: AfterCall::Chain { steps, next: next + 1, end } },
                    scratch, env, this, host)
            }
        }
    }
}
