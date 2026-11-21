//! Original capacity witnesses construct data iteratively, not deep source syntax.
use super::*;
fn check(source: &str) {
    let mut runtime = Runtime::with_limits(super::super::limits::BROWSER);
    assert_eq!(
        runtime.execute(source, &mut NoHost).unwrap().to_text(),
        "ok"
    );
}
struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        Err("No native read".into())
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        Err("No native write".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        Err("No native call".into())
    }
}
#[test]
fn c06_flat_workspace_deep_data_infinity_and_finite_depth_follow_actual_arrays() {
    check("var data=17,layers=[];for(var i=0;i<256;i++){data=[data];layers.push(data);}var deep=data.flat(Infinity);if(deep.length!==1||deep[0]!==17||data.flat(64)[0]!==layers[190]||data.flat(0)[0]!==layers[254])throw 'real finite or infinite depth';'ok';");
}
#[test]
fn c06_flat_workspace_shallow_flat_and_flat_map_ignore_supported_deep_evaluator_context() {
    for method in [
        "[[7]].flat(Infinity)[0]",
        "[1].flatMap(function(){return [7];})[0]",
    ] {
        let expression = format!("{}{}{}", "0+(".repeat(96), method, ")".repeat(96));
        check(&format!(
            "if(({expression})!==7)throw 'deep evaluator changed shallow flatten';'ok';"
        ));
    }
}

#[test]
fn c06_flat_workspace_shallow_operations_from_supported_native_evaluator_reentry() {
    std::thread::Builder::new().stack_size(8*1024*1024).spawn(||{
        for method in ["[[7]].flat(Infinity)[0]","[1].flatMap(function(){return [7];})[0]"]{
            check(&format!("function deep(n){{if(n>0)return 0+deep(n-1);return {method};}}if(deep(72)!==7)throw 'deep supported caller';'ok';"));
        }
    }).unwrap().join().unwrap();
}

#[test]
fn c06_flat_workspace_growth_and_copy_refusal_refund_exactly_and_next_call_recovers() {
    for copies in [false, true] {
        let mut runtime = Runtime::for_test();
        runtime.execute("var source=17;for(var i=0;i<32;i++)source=[source];var method=Array.prototype.flat,ctor=source.constructor,species=ctor[Symbol.species];0;",&mut NoHost).unwrap();
        let source = runtime.execute("source;", &mut NoHost).unwrap();
        let before = runtime.value_bytes;
        let heap = runtime.limits.heap.retained_values;
        if copies {
            runtime.copy_bytes = 128;
        } else {
            runtime.limits.heap.retained_values = before + 4 * std::mem::size_of::<FlattenFrame>();
        }
        let error = runtime
            .array_flat(
                source.clone(),
                false,
                &[Value::Number(f64::INFINITY)],
                &mut NoHost,
            )
            .unwrap_err();
        assert!(
            error.message.contains("storage") || error.message.contains("limit"),
            "{}",
            error.message
        );
        assert_eq!(runtime.array_flat_workspace_bytes, 0);
        assert_eq!(runtime.value_bytes, before);
        assert!(runtime.transient_registration_is_empty());
        runtime.limits.heap.retained_values = heap;
        runtime.begin_job();
        let value = runtime
            .array_flat(source, false, &[Value::Number(f64::INFINITY)], &mut NoHost)
            .unwrap();
        let Value::Object(id) = value else {
            panic!("real array")
        };
        assert_eq!(
            runtime
                .array_get_index(id, 0, &mut NoHost)
                .unwrap()
                .to_number(),
            17.
        );
        assert_eq!(runtime.array_flat_workspace_bytes, 0);
        assert!(runtime.transient_registration_is_empty());
    }
}
#[test]
fn c06_flat_workspace_cycles_stop_on_real_work_without_truncation_and_finite_revisits_expand() {
    let mut runtime = Runtime::for_test();
    runtime.execute("var cyclic=[];cyclic[0]=cyclic;var child=[3],same=[child,child].flat(Infinity);if(same.join()!=='3,3'||cyclic.flat(3)[0]!==cyclic)throw 'finite traversal or repeated identity';0;",&mut NoHost).unwrap();
    let source = runtime.execute("cyclic;", &mut NoHost).unwrap();
    let before = runtime.value_bytes;
    runtime.begin_job();
    runtime.steps = 1000;
    let error = runtime
        .array_flat(source, false, &[Value::Number(f64::INFINITY)], &mut NoHost)
        .unwrap_err();
    assert!(
        error.message.contains("Execution step limit"),
        "{}",
        error.message
    );
    assert_eq!(runtime.value_bytes, before);
    assert_eq!(runtime.array_flat_workspace_bytes, 0);
    assert!(runtime.transient_registration_is_empty());
    runtime.begin_job();
    assert_eq!(
        runtime
            .execute("[1,[2]].flat(Infinity).join();", &mut NoHost)
            .unwrap()
            .to_text(),
        "1,2"
    );
}
#[test]
fn c06_flat_workspace_abrupt_species_mapper_and_live_captured_lengths_keep_observable_effects() {
    let mut runtime = Runtime::for_test();
    runtime.execute("var marker={},target={},log='',source=[1,2];source.constructor={};source.constructor[Symbol.species]=function(n){log+='s'+n;return target;};try{source.flatMap(function(v,i){log+='m'+i;if(i===1)throw marker;return [v];});}catch(e){if(e!==marker)throw 'marker identity';}if(log!=='s0m0m1'||target[0]!==1||Object.hasOwn(target,'1'))throw 'partial target and callback order';var speciesCalls=0,a=[1];Object.defineProperty(a,'constructor',{get:function(){speciesCalls++;throw marker;}});try{a.flat();}catch(e){if(e!==marker)throw 'species identity';}if(speciesCalls!==1)throw 'species count';var inner=[1,2],parent={1:9};Object.setPrototypeOf(inner,parent);Object.defineProperty(inner,'0',{get:function(){inner.length=1;return 7;}});if([inner].flat().join()!=='7,9')throw 'captured length and inherited live hole';'ok';",&mut NoHost).unwrap();
    assert_eq!(runtime.array_flat_workspace_bytes, 0);
    assert!(runtime.transient_registration_is_empty());
}
thread_local! {static EXPECTED:std::cell::Cell<(usize,usize)>=const{std::cell::Cell::new((usize::MAX,usize::MAX))};static ROOT_CHECKED:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
fn inspect_flat_roots(runtime: &Runtime, callee: usize, _: &[Value], _: &Value) {
    let name=runtime.objects[runtime.function_properties[callee]].properties["name"].value.as_value().unwrap();
    if !matches!(name,Value::String(name) if name=="collect"){return;}
    let (parent, keep) = EXPECTED.with(|value| value.get());
    let contains = |id| {
        runtime.transient_roots.roots.iter().any(|edge|matches!(edge,super::super::function_reachability::Edge::Object(value) if *value==id))
    };
    assert!(
        contains(parent),
        "suspended flatten parent must remain an explicit root"
    );
    let target=runtime.objects.iter().enumerate().find_map(|(id,object)|{if object.length!=Some(1){return None;}let property=object.properties.get("0")?;matches!(&property.value,super::super::StoredValue::Value(Value::Function(value)) if *value==keep).then_some(id)}).expect("actual partial output array");
    assert!(
        contains(target),
        "partial output must remain rooted across callback collection"
    );
    assert!(runtime.array_flat_workspace_bytes > 0);
    ROOT_CHECKED.with(|value| value.set(true));
}
#[test]
fn c06_flat_workspace_detached_parent_and_partial_target_are_rooted_across_getter_reentry() {
    let mut runtime = Runtime::with_limits(super::super::limits::BROWSER);
    runtime.execute("var source=[[function(){return 42;},[0],function(){return 43;}]];function collect(){}Object.defineProperty(source[0][1],'0',{get:function(){source[0]=0;collect();return 1;}});0;",&mut NoHost).unwrap();
    let Value::Object(outer) = runtime.execute("source;", &mut NoHost).unwrap() else {
        panic!("source array")
    };
    let Value::Object(parent) = runtime.array_get_index(outer, 0, &mut NoHost).unwrap() else {
        panic!("parent array")
    };
    let Value::Function(keep) = runtime.array_get_index(parent, 0, &mut NoHost).unwrap() else {
        panic!("kept function")
    };
    EXPECTED.with(|value| value.set((parent, keep)));
    ROOT_CHECKED.with(|value| value.set(false));
    runtime.transient_roots.script_input_observer = Some(inspect_flat_roots);
    runtime.begin_job();
    runtime.execute("var result=source.flat(Infinity);if(result.length!==3||result[0]()!==42||result[1]!==1||result[2]()!==43)throw 'live functions after source detachment';0;",&mut NoHost).unwrap();
    runtime.transient_roots.script_input_observer = None;
    assert!(ROOT_CHECKED.with(|value| value.get()));
    assert_eq!(runtime.array_flat_workspace_bytes, 0);
    assert!(runtime.transient_registration_is_empty());
}
