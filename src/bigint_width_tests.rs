//! Original exact-width conversion witnesses; no captured application code.
use super::*;
struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("native read")
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("native write")
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("native call")
    }
}
fn succeeds(source: &str) {
    assert_eq!(
        Runtime::with_limits(super::super::limits::BROWSER)
            .execute(source, &mut NoHost)
            .unwrap()
            .to_text(),
        "ok"
    );
}
#[test]
fn bigint_width_signed_unsigned_exact_boundaries_and_large_values() {
    succeeds(
        r#"
var s=BigInt.asIntN,u=BigInt.asUintN;
for(var row of [[0,'123','0','0'],[1,'1','-1','1'],[8,'255','-1','255'],[8,'256','0','0'],[8,'-129','127','127'],[8,'-128','-128','128'],[32,'4294967295','-1','4294967295'],[64,'18446744073709551615','-1','18446744073709551615'],[64,'-9223372036854775809','9223372036854775807','9223372036854775807'],[65,'18446744073709551616','-18446744073709551616','18446744073709551616'],[128,'340282366920938463463374607431768211455','-1','340282366920938463463374607431768211455'],[129,'680564733841876926926749214863536422912','0','0']]){if(String(s(row[0],BigInt(row[1])))!==row[2]||String(u(row[0],BigInt(row[1])))!==row[3])throw 'exact width '+row[0]+' '+row[1];}
if(u(8,BigInt('-340282366920938463463374607431768211457'))!==BigInt(255)||s(8,BigInt('340282366920938463463374607431768211457'))!==BigInt(1))throw 'truncate arbitrary input';
if(s(Number.MAX_SAFE_INTEGER,BigInt(-17))!==BigInt(-17)||u(Number.MAX_SAFE_INTEGER,BigInt(17))!==BigInt(17))throw 'huge width identity';
'ok';
"#,
    );
}
#[test]
fn bigint_width_conversions_order_abrupt_completion_and_zero_width() {
    succeeds(
        r#"
var log=[],s=BigInt.asIntN,mark={};
if(s({valueOf:function(){log.push('bits');return 8.9;}},{valueOf:function(){log.push('value');return '255';}})!==BigInt(-1)||log.join(',')!=='bits,value')throw 'ordered conversions';
for(var width of [undefined,NaN,null,-0.5])if(s(width,{valueOf:function(){return true;}})!==BigInt(0))throw 'zero still converts';
function rejects(f,name){try{f();}catch(e){if(e.name===name)return;throw e;}throw 'missing '+name;}
for(var value of [undefined,null,17,NaN,Symbol('x')])rejects(function(){s(0,value);},'TypeError');
rejects(function(){s(8,'not an integer');},'SyntaxError');
for(var width of [-1,Infinity,9007199254740992])rejects(function(){s(width,{valueOf:function(){throw 'late';}});},'RangeError');
rejects(function(){s(BigInt(8),{valueOf:function(){throw 'late';}});},'TypeError');
try{s({valueOf:function(){throw mark;}},{valueOf:function(){throw 'late';}});throw 'missing';}catch(e){if(e!==mark)throw e;}
if(s('8',Object(BigInt(255)))!==BigInt(-1)||BigInt.asUintN(8,false)!==BigInt(0)||s(8,'0xff')!==BigInt(-1))throw 'real ToBigInt';
'ok';
"#,
    );
}
#[test]
fn bigint_width_static_metadata_borrowing_and_descriptors() {
    succeeds(
        r#"
for(var key of ['asIntN','asUintN']){var f=BigInt[key],d=Object.getOwnPropertyDescriptor(BigInt,key);if(typeof f!=='function'||f.name!==key||f.length!==2||!d.writable||d.enumerable||!d.configurable||Object.hasOwn(f,'prototype'))throw 'static metadata';if(f.call(null,8,'255')!==f.apply({},[8,'255'])||f.bind(17,8)('255')!==f(8,'255'))throw 'unbound';try{new f(8,'1');throw 'constructed';}catch(e){if(!(e instanceof TypeError))throw e;}}
'ok';
"#,
    );
}

#[test]
fn bigint_width_matches_independent_machine_integer_modular_results() {
    let mut runtime = Runtime::with_limits(super::super::limits::BROWSER);
    for bits in [
        0u32, 1, 2, 7, 8, 16, 31, 32, 33, 63, 64, 65, 95, 96, 127, 128,
    ] {
        for input in [
            i128::MIN,
            i128::MIN + 1,
            -18446744073709551617,
            -257,
            -256,
            -129,
            -128,
            -1,
            0,
            1,
            127,
            128,
            255,
            256,
            257,
            18446744073709551617,
            i128::MAX,
        ] {
            let mask = if bits == 128 {
                u128::MAX
            } else {
                (1u128 << bits) - 1
            };
            let residue = input as u128 & mask;
            let signed = if bits != 0 && residue & (1u128 << (bits - 1)) != 0 {
                (residue | !mask) as i128
            } else {
                residue as i128
            };
            for (mode, expected) in [(false, residue.to_string()), (true, signed.to_string())] {
                runtime.begin_job();
                let before = runtime.value_bytes;
                let args = [
                    Value::Number(f64::from(bits)),
                    Value::BigInt(Arc::from(input.to_string())),
                ];
                assert_eq!(
                    runtime
                        .big_int_width(mode, &args, &mut NoHost)
                        .unwrap()
                        .to_text(),
                    expected,
                    "{mode}/{bits}/{input}"
                );
                assert_eq!(runtime.value_bytes, before);
            }
        }
    }
}

#[test]
fn bigint_width_workspace_refusal_output_work_and_steps_refund_and_recover() {
    let args = [
        Value::Number(128.0),
        Value::BigInt(Arc::from("-340282366920938463463374607431768211457")),
    ];
    let mut probe = Runtime::with_limits(super::super::limits::BROWSER);
    probe.begin_job();
    let start = (probe.steps, probe.copy_bytes);
    probe.big_int_width(false, &args, &mut NoHost).unwrap();
    let cost = (start.0 - probe.steps, start.1 - probe.copy_bytes);
    assert!(cost.0 > 4 && cost.1 > 4);
    for (kind, allowance) in [
        ("heap", 0),
        ("heap", 8),
        ("copy", 0),
        ("copy", cost.1 / 2),
        ("copy", cost.1 - 1),
        ("steps", cost.0 / 2),
    ] {
        let mut runtime = Runtime::with_limits(super::super::limits::BROWSER);
        runtime.begin_job();
        let before = runtime.value_bytes;
        let heap = runtime.limits.heap.retained_values;
        match kind {
            "heap" => runtime.limits.heap.retained_values = before + allowance,
            "copy" => runtime.copy_bytes = allowance,
            _ => runtime.steps = allowance,
        }
        assert!(
            runtime.big_int_width(false, &args, &mut NoHost).is_err(),
            "{kind}/{allowance}"
        );
        assert_eq!(runtime.value_bytes, before, "{kind}/{allowance}");
        runtime.limits.heap.retained_values = heap;
        runtime.begin_job();
        assert_eq!(
            runtime
                .big_int_width(
                    true,
                    &[Value::Number(8.0), Value::BigInt(Arc::from("255"))],
                    &mut NoHost
                )
                .unwrap()
                .to_text(),
            "-1"
        );
        assert_eq!(runtime.value_bytes, before);
    }
    probe.begin_job();
    let before = probe.value_bytes;
    probe.limits.capacity.string_bytes = 8;
    assert!(probe.big_int_width(false, &args, &mut NoHost).is_err());
    assert_eq!(probe.value_bytes, before);
    probe.begin_job();
    assert!(probe
        .big_int_width(
            false,
            &[
                Value::Number(9007199254740991.0),
                Value::BigInt(Arc::from("-1"))
            ],
            &mut NoHost
        )
        .is_err());
    assert_eq!(probe.value_bytes, before);
}

#[test]
fn bigint_width_owner_intrinsics_errors_and_caller_state_restore() {
    let mut runtime = Runtime::with_limits(super::super::limits::BROWSER);
    let realm = runtime.allocate_intrinsic_realm().unwrap();
    let values = runtime
        .with_intrinsic_realm(Some(realm), |runtime| {
            [
                "BigInt",
                "TypeError",
                "RangeError",
                "SyntaxError",
                "Function",
            ]
            .iter()
            .map(|name| {
                runtime
                    .binding_value(runtime.global_environment, name)
                    .map(|value| (*name, value))
            })
            .collect::<Result<Vec<_>, _>>()
        })
        .unwrap();
    for (name, value) in values {
        runtime
            .define_global(&format!("Child{name}"), value)
            .unwrap();
    }
    runtime.execute(r#"
var child=ChildBigInt.asIntN,main=BigInt.asIntN,count=0;
if(child===main||Object.getPrototypeOf(child)!==ChildFunction.prototype||child(64,BigInt('18446744073709551615'))!==BigInt(-1)||main(64,ChildBigInt('18446744073709551615'))!==BigInt(-1))throw 'actual owner roots';
try{child(-1,'0');}catch(e){if(Object.getPrototypeOf(e)===ChildRangeError.prototype)count++;}
try{child(0,1);}catch(e){if(Object.getPrototypeOf(e)===ChildTypeError.prototype)count++;}
try{child(8,'invalid');}catch(e){if(Object.getPrototypeOf(e)===ChildSyntaxError.prototype)count++;}
var marker={},value={valueOf:function(){if(BigInt.asUintN(8,'-1')!==BigInt(255))throw 'nested conversion';throw marker;}};
try{child(8,value);}catch(e){if(e===marker)count++;}
if(count!==4||main(8,'255')!==BigInt(-1)||Object.getPrototypeOf([])!==Array.prototype)throw 'caller restoration';
'ok';
"#, &mut NoHost).unwrap();
    assert_eq!(runtime.active_realm, None);
    assert_eq!(runtime.global_environment, 0);
    assert!(runtime.transient_registration_is_empty());
}
