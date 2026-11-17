//! Original shared literal-method home objects and strict/sloppy super Set.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value,String> { panic!("unexpected native read") }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(),String> { panic!("unexpected native write") }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value,String> { panic!("unexpected native call") }
}

#[test]
fn super_home_object_literal_methods_accessors_and_arrows_keep_defining_object() {
    let source=r#"
var base={read(delta){return this.value+delta;},get score(){return this.value;},set score(value){this.value=value*2;}};
var original={__proto__:base,read(delta){return super.read(delta)+1;},get score(){return super.score+3;},set score(value){super.score=value+1;},arrow(){return ()=>super.read(5);}};
var receiver={value:4},getter=Object.getOwnPropertyDescriptor(original,'score').get,setter=Object.getOwnPropertyDescriptor(original,'score').set;
if(original.read.call(receiver,2)!==7||getter.call(receiver)!==7)throw 'literal borrowed receiver';
setter.call(receiver,2);if(receiver.value!==6||Object.hasOwn(base,'value'))throw 'literal super setter receiver';
var saved=original.read,copied={value:11,read:saved};Object.setPrototypeOf(copied,{read(){throw 'receiver prototype is not home';}});
var arrow=original.arrow.call(receiver);if(copied.read(2)!==14||arrow.call({value:99})!==11)throw 'copied method and lexical receiver';
Object.setPrototypeOf(original,{read(delta){return this.value+delta+100;}});
if(saved.call(receiver,2)!==109||copied.read(2)!==114||arrow()!==111)throw 'live original home-object prototype';
'ok';
"#;
    assert_eq!(Runtime::for_test().execute(source,&mut NoHost).unwrap().to_text(),"ok");
}

#[test]
fn super_home_object_set_preserves_strictness_own_descriptors_and_early_errors() {
    let source=r#"
var base={mutable:1},calls=0,receiver={};Object.defineProperty(base,'fixed',{value:3,writable:false});
var methods={__proto__:base,
 sloppy(value){super.fixed=value;return 'kept';},strict(value){'use strict';super.fixed=value;},
 sloppyData(value){super.mutable=value;return 'kept';},strictData(value){'use strict';super.mutable=value;}
};
if(methods.sloppy.call(receiver,7)!=='kept'||Object.hasOwn(receiver,'fixed')||base.fixed!==3)throw 'sloppy readonly write';
var errors=0;try{methods.strict.call(receiver,7);}catch(e){if(e.name==='TypeError')errors++;else throw e;}
Object.defineProperty(receiver,'mutable',{get:function(){calls++;return 2;},set:function(value){calls++;},configurable:true});
if(methods.sloppyData.call(receiver,9)!=='kept'||calls!==0)throw 'own accessor must not run';
try{methods.strictData.call(receiver,9);}catch(e){if(e.name==='TypeError')errors++;else throw e;}
var inherited={set mutable(value){calls++;}},fresh=Object.create(inherited);
methods.sloppyData.call(fresh,11);
if(!Object.hasOwn(fresh,'mutable')||fresh.mutable!==11||base.mutable!==1||calls!==0)throw 'create actual receiver data without inherited setter';
var sealed=Object.preventExtensions({});if(methods.sloppyData.call(sealed,13)!=='kept'||Object.hasOwn(sealed,'mutable'))throw 'sloppy nonextensible write';
try{methods.strictData.call(sealed,13);}catch(e){if(e.name==='TypeError')errors++;else throw e;}
if(errors!==3||calls!==0)throw 'strict write failures';
class Base {get receiver(){return this;}}
Base.prototype.constant=17;
class Child extends Base {data(){return super.constant;}receiver(){return super.receiver;}}
if(Child.prototype.data.call(null)!==17||Child.prototype.receiver.call(null)!==null)throw 'super reads do not coerce actual receiver';
'ok';
"#;
    assert_eq!(Runtime::for_test().execute(source,&mut NoHost).unwrap().to_text(),"ok");
    for source in ["effects++;({method(){super();}});","effects++;({method(){return ()=>super();}});"] {
        let fault=Parser::parse(source,&mut MAX_REGEXP_WORK.clone()).err().expect("literal super call must fail");
        assert!(matches!(fault.exception.as_deref(),Some(Exception::Error("SyntaxError"))),"{fault:?}");
        let mut runtime=Runtime::for_test();runtime.execute("var effects=0;",&mut NoHost).unwrap();
        assert!(runtime.execute(source,&mut NoHost).is_err());
        assert_eq!(runtime.binding_value(0,"effects").unwrap().to_number(),0.0);
    }
}
