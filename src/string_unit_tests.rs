use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        unreachable!()
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        unreachable!()
    }
    fn method(&self, _: u32, _: &str) -> Option<&'static str> {
        None
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        unreachable!()
    }
}

#[test]
fn utf16_string_units_quoted_escapes_are_primitive_code_units() {
    let mut vm = Runtime::for_test();
    let got = vm
        .execute(
            r#"
var text='\udb20\udb23-\udb58\udb5b-\udbff';
var codes=[56096,56099,45,56152,56155,45,56319];
if(typeof text!=='string'||!text||text.length!==7||Number(text)===Number(text)||
   String(text)!==text||text.valueOf()!==text||text.toString()!==text)throw 'primitive';
for(var i=0;i<codes.length;i++){
  if(text.charCodeAt(i)!==codes[i]||text[i].length!==1||text[i].charCodeAt(0)!==codes[i]||
     text.charAt(i)!==text[i])throw 'unit '+i;
}
if(text.charAt(8)!==''||text[8]!==undefined||text.codePointAt(8)!==undefined||
   !(text>'a')||text==='\ufffd'||text==='\\udb20'||text==='')throw 'boundaries';
var names='';for(var index in text){names+=index;}
if(names!=='0123456'||Object.prototype.toString.call(text)!=='[object String]')throw 'inspection';
if(Object.getPrototypeOf(text)!==String.prototype||!text.hasOwnProperty('length')||
   !text.propertyIsEnumerable('1')||text.propertyIsEnumerable('length')||
   Object.values(text)[3].charCodeAt(0)!==codes[3])throw 'string properties';
text[0]='x';
if((delete text[0])||(delete text.length)||!(delete text[9])||
   text.charCodeAt(0)!==codes[0])throw 'immutable string index';
var marker=Symbol('marker');String.prototype[marker]=19;
if(text[marker]!==19)throw 'symbol lookup';
if(JSON.stringify(text)!=='"\\udb20\\udb23-\\udb58\\udb5b-\\udbff"')throw 'JSON spelling';
true;
"#,
            &mut NoHost,
        )
        .unwrap();
    assert!(matches!(got, Value::Bool(true)));
}

#[test]
fn utf16_string_units_concat_can_repair_pairs_without_aliasing_keys() {
    let mut vm = Runtime::for_test();
    let got = vm
        .execute(
            r#"
var high='\ud83d',low='\ude00',emoji='😀',log='';
var tail={toString(){log+='t';return low;}};
if(high+low!==emoji||high.concat(tail)!==emoji||log!=='t'||
   'p'.concat(high,low,'s')!=='p😀s'||(high+low)[0]!==high||emoji.charAt(1)!==low)
    throw 'pair across operands';
var m=new Map();m.set(high,'unit');m.set('\ufffd','replacement');m.set('\\ud83d','ascii');
if(m.get(high)!=='unit'||m.get('\ufffd')!=='replacement'||m.size!==3)throw 'map key identity';
true;
"#,
            &mut NoHost,
        )
        .unwrap();
    assert!(matches!(got, Value::Bool(true)));
    vm.execute("var target={};target[high]=1;", &mut NoHost).unwrap();
    vm.begin_job();
    assert!(matches!(
        vm.execute("Object.keys(target).length===1 && Object.keys(target)[0]===high && target[high]===1 && target['\\ufffd']===undefined;", &mut NoHost)
            .unwrap(),
        Value::Bool(true)
    ));
}

#[test]
fn utf16_string_units_preserve_caps_and_recover_after_failed_storage() {
    let mut vm = Runtime::for_test();
    vm.limits.capacity.string_bytes = 6;
    assert!(matches!(vm.string_from_units(&[65; 6]).unwrap(),Value::String(v) if v=="AAAAAA"));
    let before = vm.value_bytes;
    assert!(vm
        .string_from_units(&[0x4e00; 3])
        .unwrap_err()
        .message
        .contains("String limit"));
    assert_eq!(vm.value_bytes, before);
    assert!(
        matches!(vm.string_from_units(&[0xd800,65,66]).unwrap(),Value::StringUnits(v) if v.len()==3)
    );
    assert!(vm
        .string_from_units(&[0xd800, 65, 66, 67])
        .unwrap_err()
        .message
        .contains("String limit"));
    assert!(vm
        .execute(r#"'\ud800xxx';"#, &mut NoHost)
        .unwrap_err()
        .contains("String limit"));
    vm.begin_job();
    vm.copy_bytes = 1;
    assert!(vm
        .string_from_units(&[0xd800])
        .unwrap_err()
        .message
        .contains("work limit"));
    vm.begin_job();
    assert!(matches!(
        vm.execute(r#"'\ud800'.length;"#, &mut NoHost).unwrap(),
        Value::Number(1.0)
    ));
}
