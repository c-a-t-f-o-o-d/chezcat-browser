//! Original logical assignment and Number exponentiation support.
//! Right-associated power chains share the existing pending-expression quota;
//! evaluation and coercion retain all existing job/call/value limits.
//! https://tc39.es/ecma262/#sec-assignment-operators
//! https://tc39.es/ecma262/#sec-exp-operator
//! https://tc39.es/ecma262/#sec-numeric-types-number-exponentiate
//! https://tc39.es/ecma262/#sec-getvalue

use super::{
    ExprId, ExprKind, Fault, Host, Ordinary, Program, Reference, Runtime, Value,
};

#[cfg(test)]
use super::{Parser, MAX_EXPRESSION_FRAMES};

impl Runtime {
    pub(super) fn resolved_operator_reference(
        &mut self,
        reference: Reference,
        host: &mut dyn Host,
    ) -> Result<Reference, Fault> {
        // GetValue caches ToPropertyKey in the current specification. The new
        // compound operators must reuse that key after the RHS, even when a
        // user conversion changes its return value. Simple '=' retains its
        // specified deferred key conversion. Older compound paths are left
        // unchanged in this focused compatibility pass.
        if let Reference::ComputedProperty(object, key) = reference {
            Self::require_property_receiver(&object)?;
            let key = self.property_key(key, host)?;
            Ok(key.reference(object))
        } else {
            Ok(reference)
        }
    }

    pub(super) fn name_logical_assignment_result(
        &mut self,
        program: &Program,
        left: ExprId,
        right: ExprId,
        value: &Value,
    ) -> Result<(), Fault> {
        if let (ExprKind::Name(name), ExprKind::Function(template), Value::Function(function)) =
            (&program.exprs[left].kind, &program.exprs[right].kind, value)
        {
            if program.templates[*template].name.is_none() {
                self.work(name.len())?;
                let object = self.ordinary_id(Ordinary::Function(*function))?;
                // An inferred name is metadata, not a named-expression lexical
                // binding. Future reassignment remains visible to the closure.
                self.store_property(
                    object,
                    "name".into(),
                    Value::String(name.clone()),
                    Some((false, false, true)),
                )?;
            }
        }
        Ok(())
    }
}

pub(super) fn number_exponentiate(base: f64, exponent: f64) -> f64 {
    // Handle every required special case explicitly. Only finite, non-zero
    // inputs reach the standard library's implementation-approximated powf;
    // no loop or allocation depends on the exponent's magnitude.
    if exponent.is_nan() {
        return f64::NAN;
    }
    if exponent == 0.0 {
        return 1.0;
    }
    if base.is_nan() {
        return f64::NAN;
    }
    let odd = exponent.is_finite() && exponent.fract() == 0.0 && exponent.abs() % 2.0 == 1.0;
    if base.is_infinite() || base == 0.0 {
        let magnitude = if base.is_infinite() == (exponent > 0.0) {
            f64::INFINITY
        } else {
            0.0
        };
        return if base.is_sign_negative() && odd {
            -magnitude
        } else {
            magnitude
        };
    }
    if exponent.is_infinite() {
        if base.abs() == 1.0 {
            return f64::NAN;
        }
        return if (base.abs() > 1.0) == (exponent > 0.0) {
            f64::INFINITY
        } else {
            0.0
        };
    }
    if base < 0.0 && exponent.fract() != 0.0 {
        return f64::NAN;
    }
    base.powf(exponent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestHost {
        records: Vec<String>,
        writes: usize,
    }
    impl Host for TestHost {
        fn has(&mut self, _: u32, property: &str) -> Result<bool, String> {
            Ok(matches!(property, "record" | "value"))
        }
        fn get(&mut self, _: u32, property: &str) -> Result<Value, String> {
            Ok(if property == "value" {
                Value::Number(0.0)
            } else {
                Value::Undefined
            })
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            self.writes += 1;
            Err("Operator test native write denied.".into())
        }
        fn call(&mut self, _: u32, method: &str, args: &[Value]) -> Result<Value, String> {
            if method == "record" {
                self.records.push(args[0].to_text());
                Ok(Value::Undefined)
            } else {
                Err("Operator test native call denied.".into())
            }
        }
    }
    fn run(source: &str) -> String {
        let mut runtime = Runtime::new();
        runtime
            .execute(source, &mut TestHost::default())
            .unwrap()
            .to_text()
    }

    #[test]
    fn logical_assignment_short_circuits_and_preserves_values() {
        assert_eq!(
            run(r#"
var calls=0;function rhs(){calls++;return 9;}
var a=0,b='kept',c=false,d=null;
if((a&&=rhs())!==0||(b||=rhs())!=='kept'||(c??=rhs())!==false||calls)throw 'skip';
if((d??=rhs())!==9||d!==9||calls!==1)throw 'write';
const x=0,y=1;
if((x&&=9)!==0||(y||=9)!==1||(y??=9)!==1)throw 'constant skip';
var caught=0;try{x||=2;}catch(e){if(e.name==='TypeError')caught++;}
try{y&&=2;}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==2)throw 'constant write';
var o={valueOf(){throw 'truthiness coercion';}};if((o||=9)!==o)throw 'identity';'ok';
"#),
            "ok"
        );
    }

    #[test]
    fn new_assignments_cache_computed_references_before_rhs() {
        assert_eq!(
            run(r#"
var log='',keyReads=0,stored=0;
var o={get x(){log+='g';return stored;},set x(v){log+='s';stored=v;}};
function base(){log+='b';return o;}
function key(){log+='e';return {toString(){keyReads++;log+='k';return keyReads===1?'x':'wrong';}};}
if((base()[key()]||=(log+='r',3))!==3||stored!==3||log!=='bekgrs'||keyReads!==1)throw 'logical reference';
log='';keyReads=0;
if((base()[key()]**=(log+='r',2))!==9||stored!==9||log!=='bekgrs'||keyReads!==1)throw 'power reference';
log='';keyReads=0;
if((base()[key()]||=(log+='wrong',2))!==9||log!=='bekg'||keyReads!==1)throw 'read only';
'ok';
"#),
            "ok"
        );
    }

    #[test]
    fn logical_assignment_infers_only_direct_anonymous_function_names() {
        assert_eq!(
            run(r#"
var a,b,c=true,d,e;a||=function(){return a;};b??=()=>2;c&&=(function(){});d||=function kept(){};e||=a;
if([a.name,b.name,c.name,d.name,e.name].join(':')!=='a:b:c:kept:a')throw 'names';
var old=a;a=9;if(old()!==9)throw 'name changed lexical binding';
var x,y;x||=y??=function(){};if(x.name!=='y'||y.name!=='y')throw 'chained name';
var o={};o.x||=function(){};if(o.x.name!=='')throw 'property target';
var desc=Object.getOwnPropertyDescriptor(b,'name');if(desc.writable||desc.enumerable||!desc.configurable)throw 'metadata';'ok';
"#),
            "ok"
        );
    }

    #[test]
    fn logical_and_power_assignments_keep_strict_errors_and_order() {
        assert_eq!(
            run(r#"
'use strict';var log='',o={};Object.defineProperty(o,'x',{value:2});
if((o.x||=3)!==2||(o.x??=3)!==2)throw 'strict skip';
try{o.x&&=(log+='r',3);}catch(e){if(e.name!=='TypeError')throw e;log+='t';}
if(log!=='rt')throw 'strict write';
log='';var key={toString(){log+='k';return 'x';}};
try{null[key]||=(log+='r',3);}catch(e){if(e.name!=='TypeError')throw e;log+='t';}
if(log!=='t')throw 'null receiver precedes key coercion';
var rhs={valueOf(){log+='v';return 3;}};log='';
try{Symbol('x')**(log+='r',rhs);}catch(e){if(e.name!=='TypeError')throw e;log+='t';}
if(log!=='rt')throw 'left numeric error precedes right coercion';'ok';
"#),
            "ok"
        );
    }

    #[test]
    fn exponentiation_has_right_associativity_and_update_precedence() {
        assert_eq!(
            run(r#"
if(2**3**2!==512||(2**3)**2!==64||2*3**2!==18||3**2*2!==18)throw 'precedence';
if(2**-3!==0.125||(-2)**3!==-8||-(2**2)!==-4)throw 'unary';
var x=1;if(++x**2!==4||x++**2!==4||x!==3)throw 'updates';
var a=2,b=3;a**=b**=2;if(a!==512||b!==9)throw 'assignment chain';
var log='';function item(n){log+='e'+n;return {valueOf(){log+='v'+n;return n;}};}
if(item(2)**item(3)**item(2)!==512||log!=='e2e3e2v3v2v2')throw 'coercion order';'ok';
"#),
            "ok"
        );
    }

    #[test]
    fn power_special_cases_have_required_signs_and_nan_order() {
        for (base, exponent, expected) in [
            (f64::NAN, 0.0, 1.0),
            (f64::NAN, -0.0, 1.0),
            (-0.0, 3.0, -0.0),
            (-0.0, -3.0, f64::NEG_INFINITY),
            (-0.0, 2.0, 0.0),
            (-0.0, -2.0, f64::INFINITY),
            (f64::NEG_INFINITY, 3.0, f64::NEG_INFINITY),
            (f64::NEG_INFINITY, -3.0, -0.0),
            (f64::NEG_INFINITY, 0.5, f64::INFINITY),
            (f64::NEG_INFINITY, -0.5, 0.0),
            (0.5, f64::INFINITY, 0.0),
            (0.5, f64::NEG_INFINITY, f64::INFINITY),
            (-2.0, f64::INFINITY, f64::INFINITY),
            (-2.0, f64::NEG_INFINITY, 0.0),
            (-1.0, 9_007_199_254_740_991.0, -1.0),
            (-1.0, 9_007_199_254_740_992.0, 1.0),
        ] {
            assert_eq!(
                number_exponentiate(base, exponent).to_bits(),
                expected.to_bits(),
                "{base} ** {exponent}"
            );
        }
        for (base, exponent) in [
            (1.0, f64::NAN),
            (0.0, f64::NAN),
            (1.0, f64::INFINITY),
            (-1.0, f64::NEG_INFINITY),
            (-2.0, 0.5),
            (f64::NAN, 1.0),
        ] {
            assert!(
                number_exponentiate(base, exponent).is_nan(),
                "{base} ** {exponent}"
            );
        }
        assert_eq!(
            run("(1/((-0)**3)===1/Math.pow(-0,3))+':'+(NaN**0===Math.pow(NaN,0));"),
            "true:true"
        );
    }

    #[test]
    fn invalid_power_and_assignment_syntax_has_no_early_effects() {
        for expression in [
            "-2**2",
            "+2**2",
            "!true**2",
            "~2**2",
            "typeof x**2",
            "void 0**2",
            "delete x.y**2",
            "2**-3**2",
            "x?.y||=1",
            "1&&=2",
            "[x]??=[]",
            "({x}&&={})",
            "2***3",
            "2**=3",
        ] {
            let mut runtime = Runtime::new();
            let mut host = TestHost::default();
            runtime.define_global("test", Value::Host(1)).unwrap();
            let source = format!("test.record('early');function unused(){{{expression};}}");
            let mut regexp_budget = super::super::MAX_REGEXP_WORK;
            let fault = Parser::parse(&source, &mut regexp_budget).err().unwrap();
            assert!(
                matches!(
                    fault.exception.as_deref(),
                    Some(super::super::Exception::Error("SyntaxError"))
                ),
                "{expression}: {}",
                fault.message
            );
            runtime.execute(&source, &mut host).unwrap_err();
            assert!(host.records.is_empty());
            assert!(runtime.programs.is_empty());
        }
    }

    #[test]
    fn flat_power_chains_and_logical_chains_share_bounded_parser_frames() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                for expression in [
                    format!("{}1", "1**".repeat(MAX_EXPRESSION_FRAMES)),
                    format!("{}1", "x||=".repeat(MAX_EXPRESSION_FRAMES)),
                ] {
                    let source = format!("function unused(){{var x;return {expression};}}'ok';");
                    assert_eq!(run(&source), "ok");
                }
                for expression in [
                    format!("{}1", "1**".repeat(MAX_EXPRESSION_FRAMES + 1)),
                    format!("{}1", "x||=".repeat(MAX_EXPRESSION_FRAMES + 1)),
                    format!("{}({}1)", "1**".repeat(2048), "x??=".repeat(2049)),
                ] {
                    let mut runtime = Runtime::new();
                    let mut host = TestHost::default();
                    runtime.define_global("test", Value::Host(1)).unwrap();
                    let source = format!(
                        "test.record('early');function unused(){{var x;return {expression};}}"
                    );
                    let error = runtime.execute(&source, &mut host).unwrap_err();
                    assert!(
                        error.contains("Pending expression limit reached (4096)"),
                        "{error}"
                    );
                    assert!(host.records.is_empty());
                    assert!(runtime.programs.is_empty());
                    runtime.begin_job();
                    assert_eq!(runtime.execute("2**3;", &mut host).unwrap().to_text(), "8");
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn operator_depth_and_native_write_failures_remain_fatal() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            for source in [
                format!("try{{{}1;}}catch(e){{test.record('caught');}}finally{{test.record('finally');}}", "1**".repeat(256)),
                "try{test.value||=1;}catch(e){test.record('caught');}finally{test.record('finally');}".into(),
            ] {
                let mut runtime = Runtime::new();
                let mut host = TestHost::default();
                runtime.define_global("test", Value::Host(1)).unwrap();
                let error = runtime.execute(&source, &mut host).unwrap_err();
                assert!(error.contains("limit reached") || error.contains("native write denied"), "{error}");
                assert!(host.records.is_empty());
                assert_eq!(runtime.eval_depth, 0);
                assert_eq!(runtime.call_depth, 0);
                runtime.begin_job();
                assert_eq!(runtime.execute("2**3;", &mut host).unwrap().to_text(), "8");
            }
        }).unwrap().join().unwrap();
    }

    #[test]
    fn logical_assignment_step_exhaustion_is_fatal_and_next_job_recovers() {
        let mut runtime = Runtime::new();
        let mut host = TestHost::default();
        runtime.define_global("test", Value::Host(1)).unwrap();
        runtime.steps = 300;
        let error = runtime.execute("var x=0;try{while(true)x ||= 1;}catch(e){test.record('caught');}finally{test.record('finally');}", &mut host).unwrap_err();
        assert!(error.contains("Execution step limit reached"), "{error}");
        assert!(host.records.is_empty());
        assert_eq!(runtime.eval_depth, 0);
        assert_eq!(runtime.call_depth, 0);
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("var y=0;y ||= 7;", &mut host)
                .unwrap()
                .to_text(),
            "7"
        );
    }
}
