//! Mark-only diagnostics for persistent roots at an inactive-JavaScript boundary.
//! No sweeping, slot reuse or production collection is enabled.
use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) enum Edge { Object(usize), Function(usize), Environment(usize) }

impl Edge {
    pub(super) fn value(value: &Value, add: &mut impl FnMut(Edge)) {
        match value { Value::Object(id) => add(Self::Object(*id)), Value::Function(id) => add(Self::Function(*id)), _ => {} }
    }

    pub(super) fn reference(reference: &Reference, add: &mut impl FnMut(Edge)) {
        match reference {
            Reference::Binding(id, _) => add(Self::Environment(*id)),
            Reference::Property(receiver, _) | Reference::SymbolProperty(receiver, _) => Self::value(receiver, add),
            Reference::ComputedProperty(receiver, key) => { Self::value(receiver, add); Self::value(key, add); }
            Reference::Missing(_) | Reference::Global(_) => {}
        }
    }
}

struct Graph {
    objects: Vec<bool>,
    functions: Vec<bool>,
    environments: Vec<bool>,
    pending: Vec<Edge>,
    invalid: bool,
}

impl Graph {
    fn new(runtime: &Runtime) -> Result<Self, &'static str> {
        fn bitmap(count: usize) -> Result<Vec<bool>, &'static str> {
            let mut bits = Vec::new();
            bits.try_reserve_exact(count).map_err(|_| "Unable to allocate bounded reachability scratch.")?;
            bits.resize(count, false);
            Ok(bits)
        }
        let mut pending = Vec::new();
        pending.try_reserve_exact(runtime.objects.len().saturating_add(runtime.functions.len()).saturating_add(runtime.environments.len()))
            .map_err(|_| "Unable to allocate bounded reachability scratch.")?;
        Ok(Self { objects: bitmap(runtime.objects.len())?, functions: bitmap(runtime.functions.len())?,
            environments: bitmap(runtime.environments.len())?, pending, invalid: false })
    }

    fn add(&mut self, edge: Edge) {
        let slot = match edge {
            Edge::Object(id) => self.objects.get_mut(id),
            Edge::Function(id) => self.functions.get_mut(id),
            Edge::Environment(id) => self.environments.get_mut(id),
        };
        let Some(slot) = slot else { self.invalid = true; return; };
        if !*slot { *slot = true; self.pending.push(edge); }
    }
}

impl Runtime {
    pub(super) fn direct_object_closure_assignment(&self, program: &Rc<Program>, right: ExprId, reference: &Reference, value: &Value) -> Option<usize> {
        if !matches!(reference, Reference::Property(Value::Object(_), _) | Reference::SymbolProperty(Value::Object(_), _)
            | Reference::ComputedProperty(Value::Object(_), _)) { return None; }
        let ExprKind::Function(template) = &program.exprs[right].kind else { return None; };
        let Value::Function(id) = value else { return None; };
        matches!(self.functions.get(*id), Some(Function::Script { program: actual, template: actual_template, .. })
            if Rc::ptr_eq(program, actual) && template == actual_template).then_some(*id)
    }

    // Raw public Value IDs can be retained by an embedder after a call returns.
    // Pins are permanent until document/runtime teardown. ID-only sets are bounded
    // by the existing object/function arenas and never copy script payloads.
    pub(super) fn pin_exported_value(&mut self, value: &Value) {
        match value {
            Value::Function(id) if *id < self.functions.len() => { self.exported_function_roots.insert(*id); }
            Value::Object(id) if *id < self.objects.len() => { self.exported_object_roots.insert(*id); }
            _ => {}
        }
    }

    pub(crate) fn persistent_allocation_report(&self) -> String {
        match self.mark_persistent_functions(&[], true) {
            Ok(graph) => {
                let allocated = self.functions.iter().filter(|function| matches!(function, Function::Script { .. })).count();
                let reachable = self.functions.iter().enumerate().filter(|(id, function)| graph.functions[*id] && matches!(function, Function::Script { .. })).count();
                drop(graph);
                let graph = match self.mark_root_graph(&[], true, true) {
                    Ok(graph) => graph,
                    Err(message) => return format!("Callable reachability unavailable: {message}\n"),
                };
                let candidates = self.object_assigned_closures.len();
                let unmarked = self.object_assigned_closures.iter().filter(|&&id| !graph.functions[id]).count();
                format!("Callable reachability (inactive stack; conservative native/Promise/API roots): Script allocated={allocated}; reachable={reachable}; exported function/object pins={}/{}. Object-assigned closure candidates={candidates}; unmarked with all other functions/exotic objects rooted={unmarked}. No collection or slot reuse.\nEvaluator root snapshots: {}; call-root scopes={}; completion-root scopes={}; peak ID references={}; active registrations=0. Script entries={}; uninhibited={}. Active collection coverage remains under audit.\n",
                    self.exported_function_roots.len(), self.exported_object_roots.len(), self.transient_roots.snapshots, self.transient_roots.call_scopes, self.transient_roots.completion_scopes, self.transient_roots.peak, self.transient_roots.script_entries, self.transient_roots.uninhibited_script_entries)
            }
            Err(message) => format!("Callable reachability unavailable: {message}\n"),
        }
    }

    fn mark_persistent_functions(&self, exported: &[Value], include_registered_exports: bool) -> Result<Graph, &'static str> {
        self.mark_root_graph(exported, include_registered_exports, false)
    }

    fn mark_root_graph(&self, exported: &[Value], include_registered_exports: bool, conservative_candidates: bool) -> Result<Graph, &'static str> {
        if self.call_depth != 0 || self.eval_depth != 0 || self.evaluation_frames != 0
            || self.evaluation_frame_slots != 0 || self.evaluation_argument_slots != 0
            || !self.active_array_joins.is_empty() || !self.promises.reachability_is_inactive()
            || !self.transient_registration_is_empty() {
            return Err("Mark-only snapshot requires inactive JavaScript; transient roots are not covered.");
        }
        let mut graph = Graph::new(self)?;
        graph.add(Edge::Environment(0));
        graph.add(Edge::Environment(self.global_environment));
        for realm in &self.intrinsic_realms { graph.add(Edge::Environment(realm.global_environment)); }
        for (id, function) in self.functions.iter().enumerate() {
            if !matches!(function, Function::Script { .. }) || conservative_candidates && !self.object_assigned_closures.contains(&id) {
                graph.add(Edge::Function(id));
            }
        }
        for &id in self.host_objects.values() { graph.add(Edge::Object(id)); }
        for value in self.host_method_values.values().chain(exported) { Edge::value(value, &mut |edge| graph.add(edge)); }
        if include_registered_exports {
            for &id in &self.exported_function_roots { graph.add(Edge::Function(id)); }
            for &id in &self.exported_object_roots { graph.add(Edge::Object(id)); }
        }
        // Conservatively preserve all Promise metadata and pending rejection
        // reporting. This is intentionally broader than eventual precise roots.
        for (id, object) in self.objects.iter().enumerate() {
            let plain = object.length.is_none() && object.date.is_none() && object.url.is_none() && object.primitive.is_none()
                && object.promise.is_none() && object.arguments.is_none() && object.binary.is_none() && object.collection.is_none()
                && object.iterator.is_none() && object.text.is_none() && object.regexp.is_none() && object.error_name.is_none();
            if object.promise.is_some() || conservative_candidates && !plain { graph.add(Edge::Object(id)); }
        }
        self.promises.reachability_edges(&mut |edge| graph.add(edge));
        self.reachability_intrinsic_edges(&mut |edge| graph.add(edge));
        while let Some(edge) = graph.pending.pop() {
            let mut add = |edge| graph.add(edge);
            match edge {
                Edge::Environment(id) => {
                    let Some(environment) = self.environments[id].as_ref() else { add(Edge::Environment(usize::MAX)); continue; };
                    if let Some(parent) = environment.parent { add(Edge::Environment(parent)); }
                    for binding in environment.bindings.values() {
                        if let Some(value) = &binding.value { Edge::value(value, &mut add); }
                    }
                    if let Some(pending) = &environment.arguments { pending.reachability_edges(&mut add); }
                }
                Edge::Function(id) => {
                    let Some(&properties) = self.function_properties.get(id) else { add(Edge::Object(usize::MAX)); continue; };
                    add(Edge::Object(properties));
                    match &self.functions[id] {
                        Function::Script { environment, lexical_this, .. } => {
                            add(Edge::Environment(*environment));
                            if let Some(value) = lexical_this { Edge::value(value, &mut add); }
                        }
                        Function::Bound(bound) => bound.reachability_edges(&mut add),
                        Function::Promise(closure) => closure.reachability_edges(&mut add),
                        _ => {}
                    }
                }
                Edge::Object(id) => {
                    let object = &self.objects[id];
                    if let Some(prototype) = object.prototype {
                        add(match prototype { Ordinary::Object(id) => Edge::Object(id), Ordinary::Function(id) => Edge::Function(id) });
                    }
                    for property in object.properties.values().chain(object.symbol_properties.values()) {
                        if let StoredValue::Value(value) = &property.value { Edge::value(value, &mut add); }
                        if let Some(accessors) = property.accessor {
                            for id in [accessors.get, accessors.set].into_iter().flatten() { add(Edge::Function(id)); }
                        }
                    }
                    if let Some(arguments) = &object.arguments {
                        if !arguments.mapped.is_empty() { add(Edge::Environment(arguments.environment)); }
                    }
                    if let Some(binary) = &object.binary { binary.reachability_edges(&mut add); }
                    if let Some(collection) = &object.collection { collection.reachability_edges(&mut add); }
                    if let Some(iterator) = &object.iterator { iterator.reachability_edges(&mut add); }
                    if let Some(url) = &object.url { url.reachability_edges(&mut add); }
                    if let Some(promise) = &object.promise { promise.reachability_edges(&mut add); }
                }
            }
        }
        if graph.invalid { Err("A retained root has no valid arena entry; snapshot rejected.") } else { Ok(graph) }
    }
}

#[test]
fn replaced_cell_cycles_are_unreachable_without_losing_exported_or_live_state() {
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    let mut runtime = Runtime::new();
    runtime.execute("var slots=[],first;var model=Object.create(Object.prototype,{payload:{get:function(){return this.read();}}});function replace(value){var state=value;var cell=Object.create(model);cell.write=function(next){state=next;};cell.read=function(){return state;};slots[0]=cell;return cell;}for(var i=0;i<64;i++){var current=replace(i);if(i===0)first=current;current.write(i*2);if(current.payload!==i*2)throw 'state';}", &mut NoHost).unwrap();
    let exported = runtime.execute("var saved=replace(99);var outward=saved.read;saved=null;outward=null;slots[0]=current;", &mut NoHost).unwrap();
    // Capture an actual handle separately, then remove its script-visible name.
    let exported_function = runtime.execute("var escaped=replace(100).read;escaped;", &mut NoHost).unwrap();
    runtime.execute("escaped=null;slots[0]=current;", &mut NoHost).unwrap();
    let before = (runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage());
    let graph = runtime.mark_persistent_functions(&[exported, exported_function.clone()], false).unwrap();
    let scripts = runtime.functions.iter().enumerate().filter(|(id, function)| graph.functions[*id] && matches!(function, Function::Script { .. })).count();
    let total = runtime.functions.iter().filter(|function| matches!(function, Function::Script { .. })).count();
    assert!(total >= 134 && scripts < 16, "total={total}; marked={scripts}");
    let Value::Function(id) = exported_function else { panic!("function export") };
    assert!(graph.functions[id]);
    assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage()), before);
    assert_eq!(runtime.invoke(Value::Function(id), &[], Value::Undefined, &mut NoHost).unwrap().to_number(), 100.0);
    println!("Original replaced-cell graph: total Script={total}, persistent/export-root reachable={scripts}; no slots collected or reused.");
    runtime.call_depth = 1;
    assert!(runtime.mark_persistent_functions(&[], false).is_err());
}

#[test]
fn persistent_graph_keeps_opaque_storage_accessors_arguments_and_pending_jobs() {
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    for (label, setup) in [
        ("symbol", "var holder={};holder[Symbol('slot')]=candidate;"),
        ("accessor", "var holder={};Object.defineProperty(holder,'x',{get:candidate});"),
        ("map key", "var holder=new Map([[candidate,17]]);"),
        ("set value", "var holder=new Set([candidate]);"),
        ("iterator source", "var holder=[candidate].values();"),
        ("bound receiver", "var holder=(function(){return this.value;}).bind({value:candidate});"),
        ("mapped arguments", "var holder=(function(f){var a=arguments;f=candidate;return a;})(0);"),
        ("pending strict arguments", "var holder=(function(f){'use strict';f=null;return ()=>arguments[0];})(candidate);"),
        ("view buffer", "var holder=(function(f){var b=new ArrayBuffer(2);b.saved=f;return new DataView(b);})(candidate);"),
        ("query owner", "var holder=(function(f){var u=new URL('https://example.com/?a=b');u.saved=f;return u.searchParams;})(candidate);"),
        ("fulfilled promise", "var holder=Promise.resolve(candidate);"),
        ("unreported rejection", "Promise.reject(candidate);"),
        ("finally closure", "Promise.resolve(0).finally(candidate);"),
        ("queued callback", "queueMicrotask(candidate);"),
    ] {
        let mut runtime = Runtime::new();
        let candidate = runtime.execute(&format!("var candidate=function(){{return 42;}};{setup}candidate;"), &mut NoHost).unwrap();
        let Value::Function(id) = candidate else { panic!("candidate") };
        runtime.execute("candidate=null;", &mut NoHost).unwrap();
        let before = (runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage());
        let graph = runtime.mark_persistent_functions(&[], false).unwrap();
        assert!(graph.functions[id], "lost {label}");
        assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage()), before, "{label}");
    }
}

#[test]
fn api_exports_are_permanent_deduplicated_roots_without_payload_or_work_copies() {
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    let mut runtime = Runtime::new();
    let function = runtime.execute("(function(){return 17;});", &mut NoHost).unwrap();
    let Value::Function(id) = function else { panic!("export") };
    assert!(runtime.exported_function_roots.contains(&id));
    assert!(runtime.mark_persistent_functions(&[], true).unwrap().functions[id]);
    assert!(!runtime.mark_persistent_functions(&[], false).unwrap().functions[id]);
    let before = (runtime.functions.len(), runtime.objects.len(), runtime.value_bytes, runtime.job_usage(), runtime.exported_function_roots.len());
    runtime.pin_exported_value(&function);
    runtime.pin_exported_value(&function.clone());
    let report = runtime.persistent_allocation_report();
    assert!(report.contains("No collection or slot reuse"), "{report}");
    assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.value_bytes, runtime.job_usage(), runtime.exported_function_roots.len()), before);
    let handler = runtime.compile_handler("return function(){return 23;};").unwrap();
    let returned = runtime.invoke(handler, &[], Value::Undefined, &mut NoHost).unwrap();
    let Value::Function(returned_id) = returned else { panic!("invoke export") };
    assert!(runtime.exported_function_roots.contains(&returned_id));
    assert_eq!(runtime.invoke(returned, &[], Value::Undefined, &mut NoHost).unwrap().to_number(), 23.0);
    let object = runtime.execute("(function(){var state=29;return {read:function(){return state;}};})();", &mut NoHost).unwrap();
    let Value::Object(object_id) = object else { panic!("object export") };
    let Value::Function(method_id) = *runtime.objects[object_id].properties["read"].value.as_value().unwrap() else { panic!("method") };
    assert!(runtime.exported_object_roots.contains(&object_id));
    assert!(runtime.mark_persistent_functions(&[], true).unwrap().functions[method_id]);
    assert!(!runtime.mark_persistent_functions(&[], false).unwrap().functions[method_id]);
    runtime.define_global("retained", object).unwrap();
    assert_eq!(runtime.execute("retained.read();", &mut NoHost).unwrap().to_number(), 29.0);
    let before = (runtime.functions.len(), runtime.exported_function_roots.clone());
    assert!(runtime.define_host_function("invalid name", 1, "unused").is_err());
    assert_eq!((runtime.functions.len(), runtime.exported_function_roots.clone()), before);
    assert!(runtime.mark_persistent_functions(&[], true).is_ok());
}

#[test]
fn reachability_snapshot_rejects_unregistered_transient_state_and_invalid_roots() {
    let mut runtime = Runtime::new();
    runtime.call_depth = 1;
    assert!(runtime.persistent_allocation_report().contains("unavailable"));
    runtime.call_depth = 0;
    runtime.eval_depth = 1;
    assert!(runtime.persistent_allocation_report().contains("unavailable"));
    runtime.eval_depth = 0;
    runtime.evaluation_frames = 1;
    assert!(runtime.persistent_allocation_report().contains("unavailable"));
    runtime.evaluation_frames = 0;
    runtime.evaluation_frame_slots = 1;
    assert!(runtime.persistent_allocation_report().contains("unavailable"));
    runtime.evaluation_frame_slots = 0;
    runtime.evaluation_argument_slots = 1;
    assert!(runtime.persistent_allocation_report().contains("unavailable"));
    runtime.evaluation_argument_slots = 0;
    runtime.active_array_joins.push(0);
    assert!(runtime.persistent_allocation_report().contains("unavailable"));
    runtime.active_array_joins.clear();
    runtime.exported_function_roots.insert(usize::MAX);
    let before = (runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage());
    assert!(runtime.persistent_allocation_report().contains("snapshot rejected"));
    assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage()), before);
}

#[test]
fn host_validation_dictionary_and_failed_hoist_exports_remain_roots() {
    use std::cell::RefCell;
    struct Retain { mode: &'static str, held: RefCell<Vec<Value>> }
    impl Host for Retain {
        fn owns_window_property(&self, object: Option<u32>, name: &str) -> bool { object == Some(77) && name == "captured" }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn validate_set(&self, _: u32, _: &str, value: &Value) -> Result<(), (&'static str, String)> {
            self.held.borrow_mut().push(value.clone()); Err(("TypeError", "retained rejected value".into()))
        }
        fn set(&mut self, _: u32, _: &str, value: Value) -> Result<(), String> { self.held.borrow_mut().push(value); Err("retained rejected declaration".into()) }
        fn validate_call(&self, _: u32, _: &str, args: &[Value]) -> Result<(), (&'static str, String)> {
            if matches!(self.mode, "call" | "object") { self.held.borrow_mut().push(args[0].clone()); Err(("TypeError", "retained rejected argument".into())) } else { Ok(()) }
        }
        fn dictionary_argument(&self, _: u32, _: &str) -> Option<(usize, &'static [&'static str])> { (self.mode == "dictionary").then_some((0, &["callback"])) }
        fn call_with_dictionary(&mut self, _: u32, _: &str, _: &[Value], fields: Option<&[(&str, Value)]>) -> Result<Value, String> {
            self.held.borrow_mut().push(fields.unwrap()[0].1.clone()); Ok(Value::Undefined)
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    for (mode, source) in [
        ("call", "try{native(function(){return 17;});}catch(e){}0;"),
        ("object", "try{native({read:function(){return 17;}});}catch(e){}0;"),
        ("set", "try{window.captured=function(){return 17;};}catch(e){}0;"),
        ("dictionary", "native({get callback(){return function(){return 17;};}});0;"),
        ("hoist", "function captured(){return 17;}"),
    ] {
        let mut runtime = Runtime::new();
        runtime.bind_browser_global(77).unwrap();
        runtime.define_global("window", Value::Host(77)).unwrap();
        runtime.define_host_function("native", 1, "retain").unwrap();
        let mut host = Retain { mode, held: RefCell::new(Vec::new()) };
        let result = runtime.execute(source, &mut host);
        if mode == "hoist" { assert!(result.unwrap_err().contains("retained rejected declaration")); } else { result.unwrap(); }
        let held = host.held.borrow();
        let id = match held[0] {
            Value::Function(id) => { assert!(runtime.exported_function_roots.contains(&id), "{mode}"); id }
            Value::Object(id) => {
                assert!(runtime.exported_object_roots.contains(&id));
                let Value::Function(method) = *runtime.objects[id].properties["read"].value.as_value().unwrap() else { panic!("method") };
                method
            }
            _ => panic!("export: {mode}"),
        };
        assert!(runtime.mark_persistent_functions(&[], true).unwrap().functions[id], "{mode}");
        assert!(!runtime.mark_persistent_functions(&[], false).unwrap().functions[id], "{mode}");
    }
}

#[test]
fn object_assignment_candidates_preserve_aliases_exports_and_constructor_cycles() {
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    let mut runtime = Runtime::new();
    runtime.execute("var cell={};cell.read=function(){return 1;};var alias=cell.read;cell.read=function(){return 2;};var ordinary=function(){return 3;};(function(){return 4;})();var literal={read(){return 5;}};0;", &mut NoHost).unwrap();
    assert_eq!(runtime.object_assigned_closures.len(), 2);
    let candidates: Vec<_> = runtime.object_assigned_closures.iter().copied().collect();
    let graph = runtime.mark_root_graph(&[], true, true).unwrap();
    assert!(candidates.iter().all(|&id| graph.functions[id]));
    runtime.execute("alias=null;cell.read=null;0;", &mut NoHost).unwrap();
    let graph = runtime.mark_root_graph(&[], true, true).unwrap();
    assert!(candidates.iter().all(|&id| !graph.functions[id]));
    // Automatic prototype.constructor backlinks did not tag the other functions.
    for (id, function) in runtime.functions.iter().enumerate() {
        if matches!(function, Function::Script { .. }) && !runtime.object_assigned_closures.contains(&id) { assert!(graph.functions[id]); }
    }

    runtime.execute("cell.read=function(){return 17;};0;", &mut NoHost).unwrap();
    let exported = runtime.execute("cell.read;", &mut NoHost).unwrap();
    let Value::Function(id) = exported else { panic!("export") };
    runtime.execute("cell.read=null;0;", &mut NoHost).unwrap();
    assert!(!runtime.mark_root_graph(&[], false, true).unwrap().functions[id]);
    assert!(runtime.mark_root_graph(&[], true, true).unwrap().functions[id]);
    assert_eq!(runtime.invoke(exported, &[], Value::Undefined, &mut NoHost).unwrap().to_number(), 17.0);
}

#[test]
fn candidate_tagging_observes_successful_writes_without_reconverting_keys_or_replaying_setters() {
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    let mut runtime = Runtime::new();
    runtime.execute("var conversions=0,sets=0,escaped=null;var target={set value(f){sets++;escaped=f;}};var key={toString(){conversions++;return 'value';}};target[key]=function(){return 17;};if(conversions!==1||sets!==1||escaped()!==17)throw 'key/set order';0;", &mut NoHost).unwrap();
    assert_eq!(runtime.object_assigned_closures.len(), 1);
    let id = *runtime.object_assigned_closures.iter().next().unwrap();
    assert!(runtime.mark_root_graph(&[], true, true).unwrap().functions[id]);
    runtime.execute("escaped=null;0;", &mut NoHost).unwrap();
    assert!(!runtime.mark_root_graph(&[], true, true).unwrap().functions[id]);
    runtime.execute("var rejected={set value(f){throw 23;}};try{rejected.value=function(){return 99;};throw 'missing';}catch(e){if(e!==23)throw e;}0;", &mut NoHost).unwrap();
    assert_eq!(runtime.object_assigned_closures.len(), 1);
    runtime.execute("var symbol=Symbol('slot'),holder={};holder[symbol]=function(){return 31;};if(holder[symbol]()!==31)throw 'symbol';0;", &mut NoHost).unwrap();
    assert_eq!(runtime.object_assigned_closures.len(), 2);
}
