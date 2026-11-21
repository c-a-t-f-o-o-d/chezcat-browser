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
fn c06_flat_workspace_shallow_operations_from_supported_native_evaluator_reentry(){
    std::thread::Builder::new().stack_size(8*1024*1024).spawn(||{
        for method in ["[[7]].flat(Infinity)[0]","[1].flatMap(function(){return [7];})[0]"]{
            check(&format!("function deep(n){{if(n>0)return 0+deep(n-1);return {method};}}if(deep(72)!==7)throw 'deep supported caller';'ok';"));
        }
    }).unwrap().join().unwrap();
}
