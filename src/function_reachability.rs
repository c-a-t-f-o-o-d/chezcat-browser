//! Mark-only research for persistent roots at an inactive-JavaScript boundary.
//! No sweeping, slot reuse or production collection is enabled by this module.
use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) enum Edge { Object(usize), Function(usize), Environment(usize) }

impl Edge {
    pub(super) fn value(value: &Value, add: &mut impl FnMut(Edge)) {
        match value { Value::Object(id) => add(Self::Object(*id)), Value::Function(id) => add(Self::Function(*id)), _ => {} }
    }
}

struct Graph {
    objects: Vec<bool>,
    functions: Vec<bool>,
    environments: Vec<bool>,
    pending: Vec<Edge>,
}

impl Graph {
    fn add(&mut self, edge: Edge) {
        let slot = match edge {
            Edge::Object(id) => self.objects.get_mut(id),
            Edge::Function(id) => self.functions.get_mut(id),
            Edge::Environment(id) => self.environments.get_mut(id),
        }.expect("persistent edge must have a valid arena ID");
        if !*slot { *slot = true; self.pending.push(edge); }
    }
}

impl Runtime {
    fn mark_persistent_functions(&self, exported: &[Value]) -> Result<Graph, &'static str> {
        if self.call_depth != 0 || self.eval_depth != 0 || self.evaluation_frames != 0 {
            return Err("Mark-only snapshot requires inactive JavaScript; transient roots are not covered.");
        }
        let mut graph = Graph { objects: vec![false; self.objects.len()], functions: vec![false; self.functions.len()],
            environments: vec![false; self.environments.len()], pending: Vec::new() };
        graph.add(Edge::Environment(0));
        graph.add(Edge::Environment(self.global_environment));
        for realm in &self.intrinsic_realms { graph.add(Edge::Environment(realm.global_environment)); }
        for (id, function) in self.functions.iter().enumerate() {
            if !matches!(function, Function::Script { .. }) { graph.add(Edge::Function(id)); }
        }
        for &id in self.host_objects.values() { graph.add(Edge::Object(id)); }
        for value in self.host_method_values.values().chain(exported) { Edge::value(value, &mut |edge| graph.add(edge)); }
        // Conservatively preserve all Promise metadata and pending rejection
        // reporting. This is intentionally broader than eventual precise roots.
        for (id, object) in self.objects.iter().enumerate() {
            if object.promise.is_some() { graph.add(Edge::Object(id)); }
        }
        self.promises.reachability_edges(&mut |edge| graph.add(edge));
        self.reachability_intrinsic_edges(&mut |edge| graph.add(edge));
        while let Some(edge) = graph.pending.pop() {
            let mut add = |edge| graph.add(edge);
            match edge {
                Edge::Environment(id) => {
                    let environment = self.environments[id].as_ref().expect("marked environment is retained");
                    if let Some(parent) = environment.parent { add(Edge::Environment(parent)); }
                    for binding in environment.bindings.values() {
                        if let Some(value) = &binding.value { Edge::value(value, &mut add); }
                    }
                    if let Some(pending) = &environment.arguments { pending.reachability_edges(&mut add); }
                }
                Edge::Function(id) => {
                    add(Edge::Object(self.function_properties[id]));
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
        Ok(graph)
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
    let graph = runtime.mark_persistent_functions(&[exported, exported_function.clone()]).unwrap();
    let scripts = runtime.functions.iter().enumerate().filter(|(id, function)| graph.functions[*id] && matches!(function, Function::Script { .. })).count();
    let total = runtime.functions.iter().filter(|function| matches!(function, Function::Script { .. })).count();
    assert!(total >= 134 && scripts < 16, "total={total}; marked={scripts}");
    let Value::Function(id) = exported_function else { panic!("function export") };
    assert!(graph.functions[id]);
    assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage()), before);
    assert_eq!(runtime.invoke(Value::Function(id), &[], Value::Undefined, &mut NoHost).unwrap().to_number(), 100.0);
    println!("Original replaced-cell graph: total Script={total}, persistent/export-root reachable={scripts}; no slots collected or reused.");
    runtime.call_depth = 1;
    assert!(runtime.mark_persistent_functions(&[]).is_err());
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
        let graph = runtime.mark_persistent_functions(&[]).unwrap();
        assert!(graph.functions[id], "lost {label}");
        assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage()), before, "{label}");
    }
}
