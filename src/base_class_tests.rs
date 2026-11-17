//! Original finite base-class controls, independent of framework source.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("Unexpected native read.".into()) }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("Unexpected native write.".into()) }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("Unexpected native call.".into()) }
}

fn succeeds(source: &str) {
    assert_eq!(Runtime::for_test().execute(source, &mut NoHost).unwrap().to_text(), "ok");
}

#[test]
fn base_class_default_explicit_constructors_return_values_and_bound_new_target() {
    succeeds(r#"
class Empty {}
var empty=new Empty(1,2);
if(!(empty instanceof Empty)||Object.getPrototypeOf(empty)!==Empty.prototype||Empty.length!==0)throw 'default constructor';
class Explicit {constructor(first,second=7,target=new.target){this.first=first;this.second=second;this.target=target;}}
var one=new Explicit(5),bound=Explicit.bind({ignored:true},11),twice=bound.bind(null,13),two=new twice();
if(one.first!==5||one.second!==7||one.target!==Explicit||Explicit.length!==1)throw 'explicit constructor';
if(two.first!==11||two.second!==13||two.target!==Explicit||Object.getPrototypeOf(two)!==Explicit.prototype)throw 'bound construction';
var marker={kept:true};
class ObjectResult {constructor(){return marker;}}
class PrimitiveResult {constructor(){this.kept=true;return 17;}}
if(new ObjectResult()!==marker||!(new PrimitiveResult()).kept)throw 'base return completion';
'ok';
"#);
}

#[test]
fn base_class_calls_require_construction_before_defaults_but_after_argument_effects() {
    succeeds(r#"
var defaults=0,argumentsSeen=0,bodies=0;
class Target {constructor(value=(defaults++,23)){bodies++;this.value=value;}}
var rejected=0,receiver={};
try{Target((argumentsSeen++,undefined));}catch(e){if(e.name==='TypeError')rejected++;else throw e;}
try{Target.call(receiver);}catch(e){if(e.name==='TypeError')rejected++;else throw e;}
try{Target.apply(receiver,[]);}catch(e){if(e.name==='TypeError')rejected++;else throw e;}
var bound=Target.bind(receiver);
try{bound();}catch(e){if(e.name==='TypeError')rejected++;else throw e;}
if(rejected!==4||argumentsSeen!==1||defaults!==0||bodies!==0||receiver.value!==undefined)throw 'class called before refusal';
var instance=new bound();
if(instance.value!==23||defaults!==1||bodies!==1||!(instance instanceof Target))throw 'new after refused calls';
class Plain {}
try{Plain();throw 'default class was callable';}catch(e){if(e.name!=='TypeError')throw e;}
'ok';
"#);
}

#[test]
fn base_class_canonical_prototypes_descriptors_and_nonconstructible_methods() {
    succeeds(r#"
class Shape {
 constructor(value=3){this.value=value;}
 read(first,second=0){return this.value+first+second;}
 get size(){return this.value;}
 set size(value){this.value=value;}
 static make(value){return new Shape(value);}
}
if(Object.getPrototypeOf(Shape)!==Function.prototype||Object.getPrototypeOf(Shape.prototype)!==Object.prototype)throw 'canonical prototypes';
var p=Object.getOwnPropertyDescriptor(Shape,'prototype'),c=Object.getOwnPropertyDescriptor(Shape.prototype,'constructor');
if(p.value!==Shape.prototype||p.writable||p.enumerable||p.configurable)throw 'class prototype descriptor';
if(c.value!==Shape||!c.writable||c.enumerable||!c.configurable)throw 'constructor backlink';
var method=Object.getOwnPropertyDescriptor(Shape.prototype,'read'),accessor=Object.getOwnPropertyDescriptor(Shape.prototype,'size'),stat=Object.getOwnPropertyDescriptor(Shape,'make');
if(!method.writable||method.enumerable||!method.configurable||method.value.name!=='read'||method.value.length!==1)throw 'method descriptor';
if(accessor.enumerable||!accessor.configurable||'value' in accessor||accessor.get.name!=='get size'||accessor.get.length!==0||accessor.set.name!=='set size'||accessor.set.length!==1)throw 'accessor descriptor';
if(!stat.writable||stat.enumerable||!stat.configurable||stat.value.name!=='make'||stat.value.length!==1)throw 'static method descriptor';
if(Object.keys(Shape).length!==0||Object.keys(Shape.prototype).length!==0)throw 'enumerable class members';
var errors=0;
for(var fn of [method.value,accessor.get,accessor.set,stat.value]){
 if(Object.getPrototypeOf(fn)!==Function.prototype||Object.hasOwn(fn,'prototype'))throw 'method function prototype';
 var n=Object.getOwnPropertyDescriptor(fn,'name'),l=Object.getOwnPropertyDescriptor(fn,'length');
 if(n.writable||n.enumerable||!n.configurable||l.writable||l.enumerable||!l.configurable)throw 'method metadata descriptors';
 try{new fn();}catch(e){if(e.name==='TypeError')errors++;else throw e;}
}
if(errors!==4)throw 'constructible class member';
var kept=Shape.prototype;Shape.prototype={};if(Shape.prototype!==kept)throw 'sloppy prototype assignment';
try{(function(){'use strict';Shape.prototype={};})();}catch(e){if(e.name==='TypeError')errors++;else throw e;}
if(errors!==5||Shape.prototype!==kept)throw 'strict prototype assignment';
var named=Object.getOwnPropertyDescriptor(Shape,'name');
if(Shape.name!=='Shape'||named.writable||named.enumerable||!named.configurable)throw 'class name metadata';
if(Shape.make(9).read(2,1)!==12)throw 'method remains usable';
'ok';
"#);
}

#[test]
fn base_class_literal_public_keys_static_accessors_and_borrowed_receivers() {
    succeeds(r#"
class Values {
 constructor(value){this.value=value;}
 get item(){return this.value;}
 set item(value){this.value=value;}
 'a b'(delta){return this.value+delta;}
 7(){return this.value+7;}
 __proto__(){return this.value+1;}
 get(){return 41;}
 set(value){return value+1;}
 static(){return 43;}
 async(){return 47;}
 static get total(){return this.count;}
 static set total(value){this.count=value;}
 static get(){return 53;}
 static set(value){return value+2;}
 static static(){return 59;}
}
var a=new Values(5),b=new Values(9);a.item=13;
if(a.item!==13||b.item!==9||a['a b'](2)!==15||a[7]()!==20||a.__proto__()!==14)throw 'literal instance members';
if(a.get()!==41||a.set(2)!==3||a.static()!==43||a.async()!==47)throw 'keyword method names';
if(Object.getPrototypeOf(Values.prototype)!==Object.prototype||!Object.hasOwn(Values.prototype,'__proto__'))throw 'ordinary proto-named method';
Values.total=17;if(Values.total!==17||Values.get()!==53||Values.set(2)!==4||Values.static()!==59)throw 'static public members';
var getter=Object.getOwnPropertyDescriptor(Values.prototype,'item').get,setter=Object.getOwnPropertyDescriptor(Values.prototype,'item').set,receiver={value:19};
setter.call(receiver,23);
if(getter.call(receiver)!==23||Values.prototype['a b'].call(receiver,3)!==26)throw 'borrowed receiver';
'ok';
"#);
}

#[test]
fn base_class_declaration_tdz_mutable_outer_and_immutable_private_self_binding() {
    succeeds(r#"
var before=0;
try{Declared;}catch(e){if(e.name==='ReferenceError')before++;else throw e;}
class Declared {self(){return Declared;}change(){Declared=17;}}
var original=Declared,instance=new Declared();Declared=91;
if(before!==1||Declared!==91||instance.self()!==original)throw 'outer binding versus private name';
var denied=0;try{instance.change();}catch(e){if(e.name==='TypeError')denied++;else throw e;}
if(denied!==1||instance.self()!==original||Declared!==91)throw 'immutable class self binding';
var Public=class Secret {self(){return Secret;}change(){Secret=null;}};
var named=new Public();if(Public.name!=='Secret'||typeof Secret!=='undefined'||named.self()!==Public)throw 'named expression scope';
try{named.change();}catch(e){if(e.name==='TypeError')denied++;else throw e;}
if(denied!==2||named.self()!==Public)throw 'named expression self binding';
var Anonymous=class {self(){return Anonymous;}},anonymousClass=Anonymous,anonymous=new Anonymous();Anonymous=37;
if(anonymousClass.name!=='Anonymous'||anonymous.self()!==37)throw 'inferred name is not a private binding';
{class BlockOnly{}if(typeof BlockOnly!=='function')throw 'block class';}
if(typeof BlockOnly!=='undefined')throw 'class binding escaped block';
'ok';
"#);
}

#[test]
fn base_class_strict_parameters_methods_and_lexical_new_target_arrows() {
    succeeds(r#"
class Strict {
 constructor(value=new.target){this.value=value;this.arrow=()=>new.target;}
 receiver(){return this;}
 parameter(value=(function(){return this;})()){return value;}
 target(){return new.target;}
 capture(){return ()=>new.target;}
 undeclared(){classUnbound=1;}
}
var instance=new Strict(),method=instance.receiver;
if(instance.value!==Strict||instance.arrow.call({})!==Strict||method()!==undefined||method.call(null)!==null||method.call(7)!==7)throw 'strict this and constructor capture';
if(instance.parameter()!==undefined||instance.target()!==undefined||instance.capture().call(instance)!==undefined)throw 'method parameter or target scope';
var rejected=0;try{instance.undeclared();}catch(e){if(e.name==='ReferenceError')rejected++;else throw e;}
if(rejected!==1||typeof classUnbound!=='undefined')throw 'strict method assignment';
'ok';
"#);
}

#[test]
fn base_class_repeated_members_replace_accessor_halves_without_running_getters() {
    succeeds(r#"
var calls=0;
class Pair {
 get value(){throw 'obsolete getter';}
 set value(v){this.n=v;}
 get value(){calls++;return this.n+1;}
 static get value(){throw 'obsolete static getter';}
 static value(){return 29;}
 get replaced(){throw 'obsolete accessor';}
 replaced(){return 31;}
 data(){throw 'obsolete method';}
 set data(v){this.n=v;}
}
var desc=Object.getOwnPropertyDescriptor(Pair.prototype,'value');
if(calls!==0||typeof desc.get!=='function'||typeof desc.set!=='function')throw 'definition ran getter or lost pair';
var pair=new Pair();pair.value=7;
if(pair.value!==8||calls!==1||Pair.value()!==29||pair.replaced()!==31)throw 'member override';
desc=Object.getOwnPropertyDescriptor(Pair.prototype,'data');
if(desc.get!==undefined||typeof desc.set!=='function'||pair.data!==undefined)throw 'data-to-accessor reset';
pair.data=11;if(pair.n!==11)throw 'replacement setter';
'ok';
"#);
}

#[test]
fn base_class_dynamic_function_and_direct_eval_keep_genuine_class_scopes() {
    succeeds(r#"
var Dynamic=Function('return class Inner {constructor(value=new.target){this.value=value;}self(){return Inner;}};')();
var created=new Dynamic();
if(Dynamic.name!=='Inner'||created.value!==Dynamic||created.self()!==Dynamic||typeof Inner!=='undefined')throw 'dynamic class expression';
function Factory(seed){return eval('class Local {constructor(value=seed){this.value=value;}self(){return Local;}};Local;');}
var First=Factory(17),Second=Factory(19);
if(First===Second||(new First()).value!==17||(new Second()).value!==19||(new First()).self()!==First||typeof Local!=='undefined')throw 'eval class lexical scope';
var Inferred=eval('(class {})');if(Inferred.name!=='')throw 'eval completion does not infer an outer name';
if(eval('class EvalOnly{};typeof EvalOnly;')!=='function'||typeof EvalOnly!=='undefined')throw 'eval class declaration leak';
'ok';
"#);
}

#[test]
fn base_class_anonymous_names_follow_identifier_defaults_and_property_named_evaluation() {
    succeeds(r#"
var VarClass=class{};let LetClass=class{};
if(VarClass.name!=='VarClass'||LetClass.name!=='LetClass')throw 'declaration inference';
var Assigned;Assigned=class{};
var OrClass=null,AndClass=true,NullishClass=undefined;
OrClass||=class{};AndClass&&=class{};NullishClass??=class{};
if(Assigned.name!=='Assigned'||OrClass.name!=='OrClass'||AndClass.name!=='AndClass'||NullishClass.name!=='NullishClass')throw 'assignment inference';
var unused=0;OrClass||=(unused++,class{});AndClass&&=class{};
if(unused!==0||AndClass.name!=='AndClass')throw 'logical assignment short circuit or name';
function parameter(DefaultClass=class{}){return DefaultClass;}
var {ObjectDefault=class{},source:RenamedDefault=class{}}={},[ArrayDefault=class{}]=[];
if(parameter().name!=='DefaultClass'||ObjectDefault.name!=='ObjectDefault'||RenamedDefault.name!=='RenamedDefault'||ArrayDefault.name!=='ArrayDefault')throw 'default initializer inference';
var token=Symbol('token'),empty=Symbol(''),anonymous=Symbol();
var properties={plain:class{},'a b':class{},7:class{},[token]:class{},[empty]:class{},[anonymous]:class{}};
if(properties.plain.name!=='plain'||properties['a b'].name!=='a b'||properties[7].name!=='7'||properties[token].name!=='[token]'||properties[empty].name!=='[]'||properties[anonymous].name!=='')throw 'object property inference';
var Named=class Explicit{};
if(Named.name!=='Explicit')throw 'explicit name overwritten';
'ok';
"#);
}

#[test]
fn base_class_static_name_members_override_inference_without_getter_effects() {
    succeeds(r#"
var getterCalls=0;
var MethodClass=class{static name(){return 'method';}},GetterClass=class{static get name(){getterCalls++;return 'getter';}};
var Assigned;Assigned=class{static name(){return 'assigned';}};
var Logical=null;Logical||=class{static get name(){getterCalls++;return 'logical';}};
function parameter(Default=class{static name(){return 'default';}}){return Default;}
var values={member:class{static name(){return 'property';}},getMember:class{static get name(){getterCalls++;return 'property getter';}}};
if(getterCalls!==0)throw 'inference invoked static getter';
if(typeof MethodClass.name!=='function'||MethodClass.name.name!=='name'||MethodClass.name()!=='method'||Assigned.name()!=='assigned'||parameter().name()!=='default'||values.member.name()!=='property')throw 'static method overwritten';
if(GetterClass.name!=='getter'||Logical.name!=='logical'||values.getMember.name!=='property getter'||getterCalls!==3)throw 'static getter overwritten or invocation count';
'ok';
"#);
}

#[test]
fn base_class_exact_source_reflection_preserves_class_constructor_methods_and_accessors() {
    for (source, expected) in [
        ("class /*é*/ Named {\r\n /*before*/constructor(value=1){this.value=value;}\r\n read(){return this.value;}\r\n}\r\nNamed.toString();", "class /*é*/ Named {\r\n /*before*/constructor(value=1){this.value=value;}\r\n read(){return this.value;}\r\n}"),
        ("var C=class /*anonymous*/ { /*empty*/ };C.toString();", "class /*anonymous*/ { /*empty*/ }"),
        ("class C {constructor(value){this.value=value;}}C.prototype.constructor.toString();", "class C {constructor(value){this.value=value;}}"),
        ("class C {read /*gap*/ (value=1){ /*body*/return value; }}C.prototype.read.toString();", "read /*gap*/ (value=1){ /*body*/return value; }"),
        ("class C {static method /*gap*/ (value){return value;}}C.method.toString();", "method /*gap*/ (value){return value;}"),
        ("class C {get /*gap*/ 'a b'(){return 1;}}Object.getOwnPropertyDescriptor(C.prototype,'a b').get.toString();", "get /*gap*/ 'a b'(){return 1;}"),
        ("class C {static set /*gap*/ value(item){this.n=item;}}Object.getOwnPropertyDescriptor(C,'value').set.toString();", "set /*gap*/ value(item){this.n=item;}"),
    ] {
        let mut runtime = Runtime::for_test();
        assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_text(), expected, "{source}");
    }
}

#[test]
fn base_class_invalid_and_unsupported_syntax_is_rejected_before_program_effects() {
    let syntax_errors = [
        "effects++;class C{constructor(){}constructor(){}}",
        "effects++;class C{get constructor(){return 1;}}",
        "effects++;class C{set constructor(value){}}",
        "effects++;class C{static prototype(){}}",
        "effects++;class C{get value(argument){}}",
        "effects++;class C{set value(){}}",
        "effects++;class C{set value(...items){}}",
        "effects++;class C{method(first,first){}}",
        "effects++;class C{constructor(eval){}}",
        "effects++;class C{method(arguments){}}",
        "effects++;class eval{}",
        "effects++;class{}",
        "effects++;label:class Labeled{}",
    ];
    for source in syntax_errors {
        let mut regexp_work=MAX_REGEXP_WORK;
        let fault=Parser::parse(source,&mut regexp_work).err().expect("invalid class must fail parsing");
        assert!(matches!(fault.exception.as_deref(),Some(Exception::Error("SyntaxError"))),"{source}: {fault:?}");
        assert_no_effects(source);
    }
    for source in [
        "effects++;class C extends Object{}",
        "effects++;class C{method(){return super.value;}}",
        "effects++;class C{[effects++](){}}",
        "effects++;class C{#private(){}}",
        "effects++;class C{value=effects++;}",
        "effects++;class C{static value=effects++;}",
        "effects++;class C{static{effects++;}}",
        "effects++;class C{async method(){}}",
        "effects++;class C{*method(){}}",
        "effects++;@decorator class C{}",
        "effects++;export class C{}",
    ] {
        let mut regexp_work=MAX_REGEXP_WORK;
        assert!(Parser::parse(source,&mut regexp_work).is_err(),"accepted unsupported class: {source}");
        assert_no_effects(source);
    }
}

fn assert_no_effects(source: &str) {
    let mut runtime=Runtime::for_test();
    runtime.execute("var effects=0;",&mut NoHost).unwrap();
    assert!(runtime.execute(source,&mut NoHost).is_err(),"accepted invalid class: {source}");
    runtime.begin_job();
    assert_eq!(runtime.execute("effects;",&mut NoHost).unwrap().to_text(),"0","{source}");
}
