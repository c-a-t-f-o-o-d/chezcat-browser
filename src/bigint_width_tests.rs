//! Original exact-width conversion witnesses; no captured application code.
use super::*;
struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("native read") }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("native write") }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("native call") }
}
fn succeeds(source: &str) {
    assert_eq!(Runtime::with_limits(super::super::limits::BROWSER).execute(source,&mut NoHost).unwrap().to_text(),"ok");
}
#[test]
fn bigint_width_signed_unsigned_exact_boundaries_and_large_values() {
    succeeds(r#"
var s=BigInt.asIntN,u=BigInt.asUintN;
for(var row of [[0,'123','0','0'],[1,'1','-1','1'],[8,'255','-1','255'],[8,'256','0','0'],[8,'-129','127','127'],[8,'-128','-128','128'],[32,'4294967295','-1','4294967295'],[64,'18446744073709551615','-1','18446744073709551615'],[64,'-9223372036854775809','9223372036854775807','9223372036854775807'],[65,'18446744073709551616','-18446744073709551616','18446744073709551616'],[128,'340282366920938463463374607431768211455','-1','340282366920938463463374607431768211455'],[129,'680564733841876926926749214863536422912','0','0']]){if(String(s(row[0],BigInt(row[1])))!==row[2]||String(u(row[0],BigInt(row[1])))!==row[3])throw 'exact width '+row[0]+' '+row[1];}
if(u(8,BigInt('-340282366920938463463374607431768211457'))!==BigInt(255)||s(8,BigInt('340282366920938463463374607431768211457'))!==BigInt(1))throw 'truncate arbitrary input';
if(s(Number.MAX_SAFE_INTEGER,BigInt(-17))!==BigInt(-17)||u(Number.MAX_SAFE_INTEGER,BigInt(17))!==BigInt(17))throw 'huge width identity';
'ok';
"#);
}
#[test]
fn bigint_width_conversions_order_abrupt_completion_and_zero_width() {
    succeeds(r#"
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
"#);
}
#[test]
fn bigint_width_static_metadata_borrowing_and_descriptors() {
    succeeds(r#"
for(var key of ['asIntN','asUintN']){var f=BigInt[key],d=Object.getOwnPropertyDescriptor(BigInt,key);if(typeof f!=='function'||f.name!==key||f.length!==2||!d.writable||d.enumerable||!d.configurable||Object.hasOwn(f,'prototype'))throw 'static metadata';if(f.call(null,8,'255')!==f.apply({},[8,'255'])||f.bind(17,8)('255')!==f(8,'255'))throw 'unbound';try{new f(8,'1');throw 'constructed';}catch(e){if(!(e instanceof TypeError))throw e;}}
'ok';
"#);
}
