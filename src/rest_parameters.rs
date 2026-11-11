//! Focused regressions for original identifier-rest parameter support.
//! https://tc39.es/ecma262/#sec-function-definitions-static-semantics-early-errors
//! https://tc39.es/ecma262/#sec-functiondeclarationinstantiation

use super::{Host, Runtime, Value};

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        Err("No native reads.".into())
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        Err("No native writes.".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        Err("No native calls.".into())
    }
    fn allow_eval(&mut self) -> Result<bool, String> {
        Ok(true)
    }
}

fn succeeds(source: &str) {
    assert_eq!(
        Runtime::new()
            .execute(source, &mut NoHost)
            .unwrap()
            .to_text(),
        "ok"
    );
}

#[test]
fn rest_parameters_bind_fresh_arrays_and_exclude_rest_from_length() {
    succeeds(
        r#"
function all(...tail){return tail;}
function fixed(a,b,...tail){if(a!==1||b!==2)throw 'fixed bindings';return tail;}
if(all.length!==0||fixed.length!==2)throw 'function lengths';
var first=all(3,4),second=all(3,4),empty=all();
if(!Array.isArray(first)||first.join('|')!=='3|4'||first===second||empty.length!==0||empty===all())throw 'fresh arrays';
first[0]=9;if(second[0]!==3)throw 'independent array';
if(fixed(1,2,5,6).join('|')!=='5|6'||fixed(1,2).length!==0)throw 'rest range';
function short(a,b,...tail){return a+':'+b+':'+tail.length;}
if(short(7)!=='7:undefined:0')throw 'missing fixed argument';
var descriptor=Object.getOwnPropertyDescriptor(second,'0');
if(!descriptor.writable||!descriptor.enumerable||!descriptor.configurable||second[Symbol.iterator]!==Array.prototype.values)throw 'array shape';
'ok';
"#,
    );
}

#[test]
fn rest_parameters_methods_constructors_bind_and_arrow_lexical_context() {
    succeeds(
        r#"
function Pack(head,...tail){this.head=head;this.tail=tail;}
var Bound=Pack.bind(null,7),instance=new Bound(8,9);
if(Pack.length!==1||Bound.length!==0||!(instance instanceof Pack)||instance.head!==7||instance.tail.join()!=='8,9')throw 'bound construction';
var object={n:10,method(head,...tail){return this.n+head+tail[0];}};
if(object.method.length!==1||object.method(2,3)!==15||object.method.call({n:20},2,3)!==25)throw 'method rest';
function outer(head,...tail){return (...inner)=>this.n+':'+arguments[0]+':'+tail.join()+':'+inner.join();}
var arrow=outer.call({n:5},6,7,8);
if(arrow.length!==0||arrow.call({n:99},1,2)!=='5:6:7,8:1,2')throw 'lexical context';
var expression=(head,...tail)=>head+tail.length;
if(expression.length!==1||expression(4,5,6)!==6)throw 'expression arrow';
var caught=0;try{new arrow();}catch(e){if(e.name==='TypeError')caught++;}
try{new object.method();}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==2)throw 'nonconstructible rest methods';
'ok';
"#,
    );
}

#[test]
fn rest_parameters_arguments_are_unmapped_with_restricted_callee() {
    succeeds(
        r#"
function inspect(a,...tail){
var original=arguments;a=7;tail[0]=8;
if(original[0]!==1||original[1]!==2)throw 'mapped parameter';
original[0]=3;original[1]=4;
if(a!==7||tail[0]!==8||original.length!==2)throw 'mapped argument';
var d=Object.getOwnPropertyDescriptor(original,'callee'),caught=0;
if(typeof d.get!=='function'||typeof d.set!=='function'||d.enumerable||d.configurable)throw 'callee descriptor';
try{original.callee;}catch(e){if(e.name==='TypeError')caught++;}
try{original.callee=1;}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==2)throw 'callee restrictions';
arguments=9;if(arguments!==9)throw 'sloppy arguments binding';return original;
}
var kept=inspect(1,2);if(kept[0]!==3||kept[1]!==4)throw 'retained arguments';
function mapped(a){a=7;if(arguments[0]!==7)throw 'simple mapping';arguments[0]=8;return a;}
if(mapped(1)!==8)throw 'simple mappings retained';
'ok';
"#,
    );
}

#[test]
fn rest_parameters_arguments_names_and_body_bindings_keep_scope_rules() {
    succeeds(
        r#"
function named(...arguments){if(!Array.isArray(arguments))throw 'named rest';return ()=>arguments;}
var arrow=named(2,3);if(arrow().join()!=='2,3')throw 'rest arguments shadow';
function lexical(a,...tail){let arguments=8;return ()=>arguments+tail[0]+a;}
if(lexical(1,2)()!==11)throw 'body lexical arguments';
function replaced(...tail){var tail;return tail;}
if(replaced(4,5).join()!=='4,5')throw 'var rest binding';
function nested(...tail){return function(){return tail;};}
var one=nested(1),two=nested(2);if(one()[0]!==1||two()[0]!==2)throw 'closure identity';
function overwrite(...tail){function tail(){return 4;}return tail();}
if(overwrite(8)!==4)throw 'body function replacement';
'ok';
"#,
    );
}

#[test]
fn rest_parameters_strict_context_and_directive_prologue_are_distinct() {
    succeeds(
        r#"
function strictOuter(){'use strict';return function(a,...tail){
if(this!==undefined)throw 'inherited strict receiver';
a=4;if(arguments[0]!==1)throw 'strict arguments';return tail;
};}
if(strictOuter()(1,2,3).join()!=='2,3')throw 'strict inheritance';
function harmless(...tail){('use strict');return tail.length;}
function escaped(...tail){'use\x20strict';return tail.length;}
function late(...tail){var x=1;'use strict';return tail.length+x;}
if(harmless(1)!==1||escaped(1,2)!==2||late(1)!==2)throw 'non-directives';
var caught=0;for(var text of [
"function f(...tail){'use strict';}",
"'use strict';function f(...tail){'use strict';}",
"var f=(...tail)=>{'use strict';};",
"var o={m(...tail){'other';'use strict';}}",
"'use strict';function f(...arguments){}",
"'use strict';function f(...eval){}"
]){try{eval(text);}catch(e){if(e.name==='SyntaxError')caught++;}}
if(caught!==6)throw 'strict early errors';
'ok';
"#,
    );
}

#[test]
fn rest_parameters_reject_invalid_lists_before_any_execution() {
    for tail in [
        "function f(a,...a){}",
        "function f(a,a,...tail){}",
        "function f(...tail,next){}",
        "function f(...tail,){}",
        "function f(...tail=[]){}",
        "var f=(a,...a)=>a;",
        "var f=(...tail,)=>tail;",
        "var o={set x(...tail){}};",
        "var o={get x(...tail){}};",
        "function f(...tail){let tail;}",
        "function outer(){function invalid(...tail){'use strict';}}",
    ] {
        let mut runtime = Runtime::new();
        runtime.execute("var ran=false;", &mut NoHost).unwrap();
        let source = format!("ran=true;{tail}");
        let mut parse_work = 0;
        let error = match super::Parser::parse(&source, &mut parse_work) {
            Err(error) => error,
            Ok(_) => panic!("Accepted invalid rest syntax: {tail}"),
        };
        assert!(
            matches!(
                error.exception.as_deref(),
                Some(super::Exception::Error("SyntaxError"))
            ),
            "{tail}: {error:?}"
        );
        runtime.execute(&source, &mut NoHost).unwrap_err();
        runtime.begin_job();
        assert_eq!(
            runtime.execute("ran;", &mut NoHost).unwrap().to_text(),
            "false",
            "{tail}"
        );
    }
}

#[test]
fn rest_parameters_original_array_pattern_program_is_now_supported() {
    // Exact 0.39 rejection program preserved as a positive 0.40 regression.
    let mut runtime = Runtime::new();
    runtime
        .execute("function f(...[tail]){}", &mut NoHost)
        .unwrap();
    assert_eq!(
        runtime.execute("f.length;", &mut NoHost).unwrap().to_text(),
        "0"
    );
    succeeds("function values(...[first,...tail]){return first+':'+tail.join();}if(values(1,2,3)!=='1:2,3')throw 'rest pattern values';'ok';");
}

#[test]
fn rest_parameters_keep_argument_and_object_arena_limits() {
    let mut runtime = Runtime::new();
    runtime
        .execute(
            "function rest(...tail){return tail;}var caught=false,finalized=false;",
            &mut NoHost,
        )
        .unwrap();
    assert_eq!(
        runtime
            .execute("rest.apply(null,Array(256)).length;", &mut NoHost)
            .unwrap()
            .to_text(),
        "256"
    );
    let error = runtime
        .execute(
            "try{rest.apply(null,Array(257));}catch(e){caught=true;}finally{finalized=true;}",
            &mut NoHost,
        )
        .unwrap_err();
    assert!(error.contains("Call argument limit"), "{error}");
    runtime.begin_job();
    assert_eq!(
        runtime
            .execute("!caught&&!finalized;", &mut NoHost)
            .unwrap()
            .to_text(),
        "true"
    );
    let mut runtime = Runtime::new();
    runtime
        .execute(
            "var caught=false,finalized=false;function rest(...tail){return tail;}",
            &mut NoHost,
        )
        .unwrap();
    // Leave exactly one arena slot. The first rest array fits; the second
    // reaches the existing object cap without first exhausting scope bindings.
    while runtime.objects.len() + 1 < super::MAX_OBJECTS {
        runtime.new_object(None).unwrap();
    }
    runtime.begin_job();
    let error = runtime
        .execute(
            "try{rest();rest();}catch(e){caught=true;}finally{finalized=true;}",
            &mut NoHost,
        )
        .unwrap_err();
    assert!(error.contains("Object identity limit"), "{error}");
    runtime.begin_job();
    assert_eq!(
        runtime
            .execute("!caught&&!finalized;", &mut NoHost)
            .unwrap()
            .to_text(),
        "true"
    );
}
