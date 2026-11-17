//! Original strict PutValue controls, paired with the existing fatal sloppy
//! implicit-global boundary. No native capability is created by these cases.
use super::*;

#[derive(Default)]
struct Probe {
    writes: usize,
    calls: usize,
}

impl Host for Probe {
    fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
    fn owns_global_property(&self, name: &str) -> bool { name == "lockedNative" }
    fn get(&mut self, _: u32, name: &str) -> Result<Value, String> {
        if name == "lockedNative" { Ok(Value::Number(19.0)) }
        else { Err("Unexpected native read.".into()) }
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        self.writes += 1;
        Err("Native write remains refused.".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        self.calls += 1;
        Err("Unexpected native call.".into())
    }
}

#[test]
fn strict_missing_assignment_ordinary_and_direct_eval_throw_real_errors_after_rhs_once() {
    for operation in [
        "(function(){'use strict';strictMissing=(rhs++,7);})();",
        "eval(\"'use strict';strictMissing=(rhs++,7);\");",
        "(function(){'use strict';eval('strictMissing=(rhs++,7);');})();",
    ] {
        let mut runtime = Runtime::for_test();
        let mut host = Probe::default();
        let source = format!(r#"
var rhs=0,caught=0,finalized=0,after=0;
try{{{operation}after++;}}catch(e){{
 if(e.name!=='ReferenceError'||!(e instanceof ReferenceError)||Object.getPrototypeOf(e)!==ReferenceError.prototype)throw 'wrong error identity';
 caught++;
}}finally{{finalized++;}}
after++;
if(rhs!==1||caught!==1||finalized!==1||after!==1||typeof strictMissing!=='undefined')throw 'strict assignment order or binding';
'ok';
"#);
        assert_eq!(runtime.execute(&source, &mut host).unwrap().to_text(), "ok", "{operation}");
        assert!(!runtime.environment(0).bindings.contains_key("strictMissing"));
        assert_eq!((host.writes, host.calls), (0, 0));
        assert!(!runtime.invocation_failed_fatally());
        assert!(!runtime.strict);
        assert!(runtime.invocation.is_none());
        assert!(runtime.transient_registration_is_empty());
    }
}

#[test]
fn strict_missing_assignment_sloppy_pair_is_fatal_and_only_explicit_next_job_recovers() {
    let mut runtime = Runtime::for_test();
    let mut host = Probe::default();
    runtime.execute("var rhs=0,caught=0,finalized=0,after=0;", &mut host).unwrap();
    let error = runtime.execute("try{sloppyMissing=(rhs++,7);after++;}catch(e){caught++;}finally{finalized++;}after++;", &mut host).unwrap_err();
    assert!(error.contains("implicit globals"), "{error}");
    assert!(runtime.invocation_failed_fatally());
    for (name, expected) in [("rhs", 1.0), ("caught", 0.0), ("finalized", 0.0), ("after", 0.0)] {
        assert_eq!(runtime.binding_value(0, name).unwrap().to_number(), expected, "{name}");
    }
    assert!(!runtime.environment(0).bindings.contains_key("sloppyMissing"));
    assert_eq!((host.writes, host.calls), (0, 0));
    assert!(runtime.transient_registration_is_empty());
    runtime.begin_job();
    assert_eq!(runtime.execute("after++;after===1&&rhs===1&&caught===0&&finalized===0&&typeof sloppyMissing==='undefined';", &mut host).unwrap().to_text(), "true");
    assert!(!runtime.invocation_failed_fatally());
}

#[test]
fn strict_missing_assignment_bound_window_keeps_backing_and_native_write_guards() {
    let mut runtime = Runtime::for_test();
    let mut host = Probe::default();
    runtime.bind_browser_global(7).unwrap();
    let (_, backing) = runtime.browser_global.unwrap();
    runtime.execute("var rhs=0,caught=0,finalized=0,after=0;function strict(){'use strict';try{windowMissing=(rhs++,7);}catch(e){if(e.name!=='ReferenceError'||!(e instanceof ReferenceError))throw e;caught++;}finally{finalized++;}}strict();", &mut host).unwrap();
    assert_eq!((host.writes, host.calls), (0, 0));
    assert!(!runtime.environment(0).bindings.contains_key("windowMissing"));
    assert!(!runtime.objects[backing].properties.contains_key("windowMissing"));
    assert_eq!(runtime.execute("rhs===1&&caught===1&&finalized===1&&typeof windowMissing==='undefined';", &mut host).unwrap().to_text(), "true");
    // The same strict context must still use the native write refusal for an
    // actually owned global; it must not be reclassified as a missing name.
    let error = runtime.execute("(function(){'use strict';try{lockedNative=23;}catch(e){after++;}finally{after++;}})();after++;", &mut host).unwrap_err();
    assert!(error.contains("Native write remains refused"), "{error}");
    assert_eq!(host.writes, 1);
    assert_eq!(runtime.binding_value(0, "after").unwrap().to_number(), 0.0);
    assert!(!runtime.objects[backing].properties.contains_key("lockedNative"));
    runtime.begin_job();
    assert_eq!(runtime.execute("lockedNative;", &mut host).unwrap().to_number(), 19.0);
}
