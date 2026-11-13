//! ID-only registrations for suspended interpreter state. No collection occurs.
use super::{Control, Exception, Fault, Runtime, Value, MAX_ARGUMENTS, MAX_CALL_DEPTH, MAX_EVAL_DEPTH,
    MAX_EVALUATION_ARGUMENT_SLOTS, MAX_EVALUATION_FRAME_SLOTS};
use super::function_reachability::Edge;

const MAX_ROOT_REFERENCES: usize = 4 * MAX_EVALUATION_FRAME_SLOTS
    + MAX_EVALUATION_ARGUMENT_SLOTS + MAX_CALL_DEPTH * (MAX_ARGUMENTS + 4)
    + 4 * MAX_EVAL_DEPTH;

#[derive(Default)]
pub(super) struct State {
    pub(super) roots: Vec<Edge>,
    pub(super) evaluation_frames: usize,
    pub(super) frame_slots: usize,
    pub(super) argument_slots: usize,
    pub(super) inhibited: usize,
    pub(super) calls: usize,
    pub(super) call_scopes: usize,
    pub(super) completion_scopes: usize,
    pub(super) script_entries: usize,
    pub(super) uninhibited_script_entries: usize,
    pub(super) snapshots: usize,
    pub(super) peak: usize,
}

#[derive(Clone, Copy)]
pub(super) struct Marker {
    length: usize,
    evaluation_frames: usize,
    frame_slots: usize,
    argument_slots: usize,
    inhibited: usize,
    calls: usize,
}

#[derive(Clone, Copy)]
// Pure root scopes do not own evaluator/call counters. Keep their token small:
// statement recursion must retain the existing one-MiB native-stack boundary.
pub(super) struct RootMarker(usize);

impl Runtime {
    pub(super) fn root_marker(&self) -> RootMarker { RootMarker(self.transient_roots.roots.len()) }

    pub(super) fn restore_root_marker(&mut self, marker: RootMarker) {
        self.transient_roots.roots.truncate(marker.0);
    }

    pub(super) fn transient_marker(&self) -> Marker {
        let state = &self.transient_roots;
        Marker { length: state.roots.len(), evaluation_frames: state.evaluation_frames,
            frame_slots: state.frame_slots, argument_slots: state.argument_slots, inhibited: state.inhibited, calls: state.calls }
    }

    pub(super) fn inhibit_transient_collection(&mut self) -> Marker {
        let marker = self.transient_marker();
        self.transient_roots.inhibited += 1;
        marker
    }

    pub(super) fn restore_transient_marker(&mut self, marker: Marker) {
        let state = &mut self.transient_roots;
        state.roots.truncate(marker.length);
        state.evaluation_frames = marker.evaluation_frames;
        state.frame_slots = marker.frame_slots;
        state.argument_slots = marker.argument_slots;
        state.inhibited = marker.inhibited;
        state.calls = marker.calls;
    }

    pub(super) fn finish_transient_inhibition(&mut self, marker: Marker) {
        // Parameter preparation publishes new environment roots while inhibited.
        // Retain those roots until the body finishes, restoring only inhibition.
        self.transient_roots.inhibited = marker.inhibited;
    }

    pub(super) fn push_transient_value(&mut self, value: &Value) -> Result<(), Fault> {
        match value { Value::Object(id) => self.push_transient_root(Edge::Object(*id)),
            Value::Function(id) => self.push_transient_root(Edge::Function(*id)), _ => Ok(()) }
    }

    pub(super) fn push_completion_roots(&mut self, completion: &Result<Control, Fault>) -> Result<(), Fault> {
        let value = match completion {
            Ok(Control::Normal(value) | Control::Break(_, value) | Control::Continue(_, value)) => value.as_ref(),
            Ok(Control::Return(value)) => Some(value),
            Err(error) => match error.exception.as_deref() { Some(Exception::Value(value)) => Some(value), _ => None },
        };
        if let Some(value) = value { self.push_transient_value(value)?; }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn with_statement_roots<T>(&mut self, environment: usize, this: &Value,
        previous: Option<&Value>, action: impl FnOnce(&mut Self) -> Result<T, Fault>) -> Result<T, Fault> {
        let marker = self.publish_statement_inputs(environment, this, previous)?;
        let result = action(self);
        self.restore_root_marker(marker);
        result
    }

    pub(super) fn publish_statement_inputs(&mut self, environment: usize, this: &Value,
        previous: Option<&Value>) -> Result<RootMarker, Fault> {
        let marker = self.root_marker();
        let result = (|| {
            self.push_transient_root(Edge::Environment(environment))?;
            self.push_transient_value(this)?;
            if let Some(value) = previous { self.push_transient_value(value)?; }
            self.transient_roots.completion_scopes = self.transient_roots.completion_scopes.saturating_add(1);
            Ok(())
        })();
        if let Err(error) = result { self.restore_root_marker(marker); return Err(error); }
        Ok(marker)
    }

    pub(super) fn publish_call_inputs(&mut self, function: &Value, this: &Value, args: &[Value]) -> Result<Marker, Fault> {
        let marker = self.transient_marker();
        let result = (|| {
            self.push_transient_value(function)?;
            self.push_transient_value(this)?;
            if args.len() <= MAX_ARGUMENTS {
                for value in args { self.push_transient_value(value)?; }
            } else {
                // Preserve the original dispatch's argument-limit validation.
                self.transient_roots.inhibited += 1;
            }
            self.transient_roots.calls += 1;
            self.transient_roots.call_scopes = self.transient_roots.call_scopes.saturating_add(1);
            Ok(())
        })();
        if let Err(error) = result { self.restore_transient_marker(marker); return Err(error); }
        Ok(marker)
    }

    pub(super) fn push_transient_root(&mut self, edge: Edge) -> Result<(), Fault> {
        let roots = &mut self.transient_roots.roots;
        if roots.len() >= MAX_ROOT_REFERENCES { return Err(Fault::new("Transient root-reference limit reached.")); }
        if roots.len() == roots.capacity() {
            let additional = roots.capacity().max(8).min(MAX_ROOT_REFERENCES - roots.len());
            roots.try_reserve_exact(additional).map_err(|_| Fault::new("Unable to allocate bounded transient-root scratch."))?;
        }
        roots.push(edge);
        self.transient_roots.peak = self.transient_roots.peak.max(roots.len());
        Ok(())
    }

    pub(super) fn transient_registration_is_empty(&self) -> bool {
        let state = &self.transient_roots;
        state.roots.is_empty() && state.evaluation_frames == 0 && state.frame_slots == 0
            && state.argument_slots == 0 && state.inhibited == 0 && state.calls == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EvalHost;
    impl super::super::Host for EvalHost {
        fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("unexpected get".into()) }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("unexpected set".into()) }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Ok(Value::Undefined) }
        fn dictionary_argument(&self, _: u32, _: &str) -> Option<(usize, &'static [&'static str])> { Some((0, &["first", "second"])) }
        fn call_with_dictionary(&mut self, _: u32, _: &str, _: &[Value], fields: Option<&[(&str, Value)]>) -> Result<Value, String> {
            let fields = fields.expect("dictionary fields");
            assert!(matches!(fields[0], ("first", Value::Function(_))));
            assert!(matches!(fields[1], ("second", Value::Object(_))));
            Ok(Value::Undefined)
        }
    }

    #[test]
    fn completion_roots_cover_all_control_and_throw_variants_and_restore_after_failure() {
        let mut runtime = Runtime::new();
        for completion in [
            Ok(Control::Normal(Some(Value::Object(1)))), Ok(Control::Return(Value::Object(1))),
            Ok(Control::Break(0, Some(Value::Object(1)))), Ok(Control::Continue(0, Some(Value::Object(1)))),
            Err(Fault::thrown(Value::Object(1))),
        ] {
            let marker = runtime.transient_marker();
            runtime.push_completion_roots(&completion).unwrap();
            assert!(matches!(runtime.transient_roots.roots.as_slice(), [Edge::Object(1)]));
            runtime.restore_transient_marker(marker);
        }
        for completion in [Ok(Control::Normal(None)), Err(Fault::new("fatal")), Err(Fault::exception("TypeError", "lazy"))] {
            runtime.push_completion_roots(&completion).unwrap();
            assert!(runtime.transient_registration_is_empty());
        }
        let receiver = Value::Object(2);
        let previous = Value::Function(3);
        let result: Result<(), Fault> = runtime.with_statement_roots(0, &receiver, Some(&previous), |runtime| {
            assert!(matches!(runtime.transient_roots.roots.as_slice(), [Edge::Environment(0), Edge::Object(2), Edge::Function(3)]));
            Err(Fault::new("nested failure"))
        });
        assert_eq!(result.unwrap_err().message, "nested failure");
        assert!(runtime.transient_registration_is_empty());
    }

    #[test]
    fn statement_and_loop_completions_replace_roots_and_finally_preserves_values() {
        let mut runtime = Runtime::new();
        let source = r#"
function item(n){return {read:function(){return n;}};}function cleanup(){return 0;}
var fromList=eval('item(1);var ignored=cleanup();');if(fromList.read()!==1)throw 'list';
function returning(){try{return item(2);}finally{cleanup();}}if(returning().read()!==2)throw 'return';
try{try{throw item(3);}finally{cleanup();}}catch(e){if(e.read()!==3)throw 'throw';}
try{throw {part:item(4)};}catch({part:p}){if(p.read()!==4)throw 'catch pattern';}
var payload={},n=0;while(n++<2000){payload;}do{payload;}while(n++<4000);
for(var i=0;i<2000;i++){payload;}for(let j=0;j<20;j++){payload;}
var choice=item(5);switch(choice){case (cleanup(),choice):if(choice.read()!==5)throw 'switch';break;default:throw 'selection';}
'ok';
"#;
        assert_eq!(runtime.execute(source, &mut EvalHost).unwrap().to_text(), "ok");
        assert!(runtime.transient_roots.completion_scopes > 10_000);
        assert!(runtime.transient_roots.peak < 128, "root stack accumulated: {}", runtime.transient_roots.peak);
        assert!(runtime.transient_registration_is_empty());
        for source in [
            "try{throw {get part(){throw 9;}};}catch({part:p}){}",
            "try{throw 7;}finally{missing();}",
            "for(let i=0;i<2;i++){missing();}",
        ] {
            runtime.begin_job();
            assert!(runtime.execute(source, &mut EvalHost).is_err());
            assert!(runtime.transient_registration_is_empty(), "{source}");
        }
    }

    #[test]
    fn direct_eval_host_dictionary_and_apply_helpers_inhibit_nested_script_entries() {
        let mut runtime = Runtime::new();
        let value = runtime.string_eval(&Value::String("function f(){return 7;}f();".into()), None, &mut EvalHost).unwrap();
        assert_eq!(value.to_number(), 7.0);
        assert!(runtime.transient_roots.script_entries > 0);
        assert_eq!(runtime.transient_roots.uninhibited_script_entries, 0);
        let dictionary = runtime.execute("({get first(){return function(){return 1;};},get second(){return {};}});", &mut EvalHost).unwrap();
        runtime.call_member(Value::Host(7), "consume", &[dictionary], &mut EvalHost).unwrap();
        assert_eq!(runtime.transient_roots.uninhibited_script_entries, 0);
        let function = runtime.execute("(function(x){return x;});", &mut EvalHost).unwrap();
        let array_like = runtime.execute("({length:1,get 0(){return 19;}});", &mut EvalHost).unwrap();
        let value = runtime.call_member(function.clone(), "apply", &[Value::Null, array_like], &mut EvalHost).unwrap();
        assert_eq!(value.to_number(), 19.0);
        assert_eq!(runtime.transient_roots.uninhibited_script_entries, 0);
        runtime.call_value(function, &[Value::Number(23.0)], Value::Null, &mut EvalHost).unwrap();
        assert_eq!(runtime.transient_roots.uninhibited_script_entries, 1);
        assert!(runtime.transient_registration_is_empty());
    }

    #[test]
    fn call_input_roots_include_callee_receiver_and_all_arguments_with_bounded_validation() {
        let mut runtime = Runtime::new();
        let receiver = Value::Object(runtime.new_object(None).unwrap());
        let argument = Value::Object(runtime.new_object(None).unwrap());
        let function = Value::Function(0);
        let before = (runtime.job_usage(), runtime.job_deadline, runtime.value_bytes);
        let marker = runtime.publish_call_inputs(&function, &receiver, &[argument.clone(), function.clone()]).unwrap();
        assert_eq!(runtime.transient_roots.calls, 1);
        for value in [&receiver, &argument] {
            let Value::Object(id) = value else { unreachable!() };
            assert!(runtime.transient_roots.roots.iter().any(|edge| matches!(edge, Edge::Object(found) if found == id)));
        }
        assert!(runtime.transient_roots.roots.iter().any(|edge| matches!(edge, Edge::Function(0))));
        runtime.restore_transient_marker(marker);
        assert!(runtime.transient_registration_is_empty());
        let oversized = vec![Value::Undefined; MAX_ARGUMENTS + 1];
        let marker = runtime.publish_call_inputs(&function, &receiver, &oversized).unwrap();
        assert!(runtime.transient_roots.inhibited > 0);
        runtime.restore_transient_marker(marker);
        assert_eq!((runtime.job_usage(), runtime.job_deadline, runtime.value_bytes), before);
    }

    #[test]
    fn calls_activations_and_construction_restore_roots_and_preserve_captured_receivers() {
        struct NoHost;
        impl super::super::Host for NoHost {
            fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
            fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
        }
        let mut runtime = Runtime::new();
        runtime.execute("function factory(a=17,read=()=>a){var a=23;return read;}var saved=factory();if(saved()!==17)throw 'parameter capture';function receiver(){return ()=>this.valueOf();}var held=receiver.call(7);if(held()!==7)throw 'boxed receiver';function Box(x=31){this.x=x;}if(new Box().x!==31)throw 'constructor';var named=function self(n){return n?self(n-1):41;};if(named(3)!==41)throw 'private environment';0;", &mut NoHost).unwrap();
        assert!(runtime.transient_registration_is_empty());
        assert!(runtime.transient_roots.call_scopes > 0);
        for source in [
            "function bad(a=missing){}bad();",
            "function recursive(a=recursive()){}recursive();",
            "new (function(){throw 17;})();",
        ] {
            runtime.begin_job();
            assert!(runtime.execute(source, &mut NoHost).is_err());
            assert!(runtime.transient_registration_is_empty(), "{source}");
            assert!(!runtime.persistent_allocation_report().contains("unavailable"), "{source}");
        }
        runtime.begin_job();
        assert_eq!(runtime.execute("saved()+held();", &mut NoHost).unwrap().to_number(), 24.0);
    }

    #[test]
    fn transient_root_bound_and_nested_markers_restore_without_refunding_work() {
        let mut runtime = Runtime::new();
        let before = (runtime.job_usage(), runtime.job_deadline, runtime.value_bytes);
        let outer = runtime.transient_marker();
        runtime.push_transient_root(Edge::Environment(0)).unwrap();
        let inner = runtime.inhibit_transient_collection();
        runtime.push_transient_root(Edge::Function(0)).unwrap();
        runtime.restore_transient_marker(inner);
        assert_eq!(runtime.transient_roots.roots.len(), 1);
        assert_eq!(runtime.transient_roots.inhibited, 0);
        for _ in 1..MAX_ROOT_REFERENCES { runtime.push_transient_root(Edge::Environment(0)).unwrap(); }
        assert!(runtime.push_transient_root(Edge::Environment(0)).unwrap_err().message.contains("root-reference limit"));
        runtime.restore_transient_marker(outer);
        assert!(runtime.transient_registration_is_empty());
        assert_eq!((runtime.job_usage(), runtime.job_deadline, runtime.value_bytes), before);
    }
}
