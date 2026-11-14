//! Ordinary browser-default witnesses crossing old development-policy ceilings.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
}

#[test]
fn browser_capacity_keeps_live_closures_environments_and_bindings_beyond_old_caps() {
    let mut runtime = Runtime::new();
    let result = runtime.execute(concat!(
        "var held=[];function make(n){let state=n;return function(v){if(v!==undefined)state=v;return state;};}",
        "for(var i=0;i<4200;i++)held.push(make(i));",
        "if(held[0]===held[1]||held[4199]()!==4199)throw 'identity/state';",
        "held[0](91);held[0]()+held[4199]();"
    ), &mut NoHost).unwrap();
    assert_eq!(result.to_number(), 4290.0);
    assert!(runtime.function_arena.occupied > 1024);
    assert!(runtime.environments.len() - runtime.free_environments.len() > 4096);
    assert!(runtime.bindings > 4096);
}

#[test]
fn browser_capacity_retains_more_than_128_independent_live_programs() {
    let mut runtime = Runtime::new();
    let mut held = Vec::new();
    for i in 0..140 {
        runtime.begin_job();
        held.push(runtime.execute(&format!("(function(){{return {i};}});"), &mut NoHost).unwrap());
    }
    assert!(runtime.programs.len() > 128);
    for (i, function) in held.into_iter().enumerate() {
        runtime.begin_job();
        assert_eq!(runtime.invoke(function, &[], Value::Undefined, &mut NoHost).unwrap().to_number(), i as f64);
    }
}

#[test]
fn browser_capacity_source_strings_templates_and_literals_cross_old_parser_caps() {
    let mut runtime = Runtime::new();
    let source = format!("/*{}*/42;", "x".repeat(513 * 1024));
    assert_eq!(runtime.execute(&source, &mut NoHost).unwrap().to_number(), 42.0);
    for quote in ["'", "`"] {
        runtime.begin_job();
        let text = "x".repeat(128 * 1024 + 1);
        assert_eq!(runtime.execute(&format!("{quote}{text}{quote};"), &mut NoHost).unwrap().to_text(), text);
    }
    runtime.begin_job();
    let object = (0..4097).map(|i| format!("k{i}:{i}")).collect::<Vec<_>>().join(",");
    assert_eq!(runtime.execute(&format!("var object={{{object}}};object.k4096;"), &mut NoHost).unwrap().to_number(), 4096.0);
    runtime.begin_job();
    let array = vec!["1"; 8193].join(",");
    assert_eq!(runtime.execute(&format!("[{array}].length;"), &mut NoHost).unwrap().to_number(), 8193.0);
}

#[test]
fn browser_capacity_direct_spread_apply_arguments_exceed_old_scratch_and_call_caps() {
    let mut runtime = Runtime::new();
    let args = vec!["65"; 20_000].join(",");
    for expression in [
        format!("String.fromCharCode({args}).length;"),
        format!("String.fromCharCode(...[{args}]).length;"),
        format!("String.fromCharCode.apply(null,[{args}]).length;"),
    ] {
        runtime.begin_job();
        assert_eq!(runtime.execute(&expression, &mut NoHost).unwrap().to_number(), 20_000.0);
        assert!(runtime.transient_registration_is_empty());
    }
    runtime.limits.capacity.arguments = usize::MAX;
    assert!(runtime.argument_scratch_limit().unwrap_err().message.contains("overflow"));
}
