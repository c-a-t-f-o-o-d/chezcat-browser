//! Original finite Script-derived class acceptance. Native/builtin subclassing,
//! fields, computed/private keys and other deferred syntax stay outside scope.
use super::*;

#[derive(Default)]
struct NoHost { calls: usize, writes: usize }
impl Host for NoHost {
    fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("Unexpected native read.".into()) }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        self.writes += 1;Err("Unexpected native write.".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        self.calls += 1;Err("Native base construction remains unavailable.".into())
    }
}

fn succeeds(source: &str) {
    let mut host=NoHost::default();
    assert_eq!(Runtime::for_test().execute(source,&mut host).unwrap().to_text(),"ok");
    assert_eq!((host.calls,host.writes),(0,0));
}

#[test]
fn derived_class_explicit_default_and_bound_construction_forward_real_new_target() {
    succeeds(r#"
class Base {constructor(first,second){this.first=first;this.second=second;this.target=new.target;}}
class Middle extends Base {}
class Leaf extends Middle {}
var a=new Leaf(3,5),Bound=Leaf.bind({ignored:true},7),b=new Bound(11);
if(a.first!==3||a.second!==5||b.first!==7||b.second!==11||a.target!==Leaf||b.target!==Leaf)throw 'default forwarding and target';
if(!(a instanceof Leaf)||!(a instanceof Middle)||!(a instanceof Base)||Object.getPrototypeOf(a)!==Leaf.prototype)throw 'instance chain';
if(Object.getPrototypeOf(Leaf)!==Middle||Object.getPrototypeOf(Leaf.prototype)!==Middle.prototype||Leaf.length!==0)throw 'default class metadata';
var defaults=0;
class Explicit extends Base {constructor(value=(defaults++,13),target=new.target){if(target!==Explicit)throw 'default target';super(value,17);this.extra=19;}}
var rejected=0;try{Explicit();}catch(e){if(e.name==='TypeError')rejected++;else throw e;}
if(rejected!==1||defaults!==0)throw 'class call before parameter evaluation';
var c=new Explicit();if(c.first!==13||c.second!==17||c.target!==Explicit||c.extra!==19||defaults!==1)throw 'explicit super construction';
function Plain(value){this.value=value;this.target=new.target;}
class FromPlain extends Plain {}
var plain=new FromPlain(23);if(plain.value!==23||plain.target!==FromPlain||Object.getPrototypeOf(FromPlain)!==Plain||!(plain instanceof Plain))throw 'ordinary Script base';
'ok';
"#);
}

#[test]
fn derived_class_this_is_uninitialized_and_pre_super_arrows_capture_the_live_binding() {
    succeeds(r#"
class Base {constructor(value){this.value=value;}}
class Child extends Base {
 constructor(value,read=()=>this){
  var errors=0,nested=()=>()=>this,inner=nested(),target=()=>new.target;
  for(var probe of [read,inner,()=>this.value,()=>typeof this]){
   try{probe();}catch(e){if(e.name==='ReferenceError'&&e instanceof ReferenceError)errors++;else throw e;}
  }
  if(errors!==4||target()!==Child)throw 'uninitialized this or available new.target';
  super(value);this.read=read;this.inner=inner;this.target=target;
 }
}
var child=new Child(29);
if(child.value!==29||child.read()!==child||child.read.call({})!==child||child.inner.bind({})()!==child||child.target()!==Child)throw 'live lexical binding after super';
class ArrowSuper extends Base {constructor(value){var initialize=()=>super(value);initialize();this.read=()=>this;}}
var arrow=new ArrowSuper(31);if(arrow.value!==31||arrow.read()!==arrow)throw 'lexical super call';
'ok';
"#);
}

#[test]
fn derived_class_duplicate_super_constructs_again_then_refuses_to_rebind_this() {
    succeeds(r#"
var calls=0,trace='';
class Base {constructor(){this.serial=++calls;trace+='B';}}
class Child extends Base {constructor(){super();var original=this;trace+='1';try{super();}catch(e){if(e.name!=='ReferenceError'||!(e instanceof ReferenceError))throw e;trace+='R';}if(this!==original)throw 'second binding replaced this';this.ready=true;}}
var child=new Child();
if(calls!==2||trace!=='B1BR'||child.serial!==1||!child.ready)throw 'duplicate super side-effect order';
'ok';
"#);
}

#[test]
fn derived_class_object_primitive_and_uninitialized_return_rules_are_distinct() {
    succeeds(r#"
var calls=0,object={replacement:true},callable=function(){return 37;};
class Base {constructor(){calls++;this.base=true;}}
class ObjectOnly extends Base {constructor(){return object;}}
class FunctionOnly extends Base {constructor(){return callable;}}
if(new ObjectOnly()!==object||new FunctionOnly()!==callable||calls!==0)throw 'object return without super';
class CapturedMissing extends Base {constructor(){return {read:()=>this};}}
var returned=new CapturedMissing(),referenceErrors=0,typeErrors=0;
try{returned.read();}catch(e){if(e.name==='ReferenceError')referenceErrors++;else throw e;}
class Missing extends Base {constructor(){}}
class UndefinedOnly extends Base {constructor(){return undefined;}}
for(var C of [Missing,UndefinedOnly])try{new C();}catch(e){if(e.name==='ReferenceError')referenceErrors++;else throw e;}
class PrimitiveOnly extends Base {constructor(){return 3;}}
class NullOnly extends Base {constructor(){return null;}}
class PrimitiveAfter extends Base {constructor(){super();return 5;}}
for(var C of [PrimitiveOnly,NullOnly,PrimitiveAfter])try{new C();}catch(e){if(e.name==='TypeError')typeErrors++;else throw e;}
class UndefinedAfter extends Base {constructor(){super();return undefined;}}
var initialized=new UndefinedAfter();
if(referenceErrors!==3||typeErrors!==3||calls!==2||!initialized.base||!(initialized instanceof UndefinedAfter))throw 'derived completion rules';
function ReplacingBase(){return {fromBase:true};}
class UsesReplacement extends ReplacingBase {constructor(){super();this.after=true;}}
var replacement=new UsesReplacement();if(!replacement.fromBase||!replacement.after||replacement instanceof UsesReplacement)throw 'super binds actual constructed return';
'ok';
"#);
}

#[test]
fn derived_class_instance_super_uses_home_object_and_borrowed_getter_setter_receiver() {
    succeeds(r#"
class Base {
 constructor(value){this.value=value;}
 read(delta){return this.value+delta;}
 get score(){return this.value+1;}
 set score(value){this.value=value*2;}
}
class Child extends Base {
 constructor(value){super(value);this.fromConstructor=super.read(1);}
 read(delta){return super.read(delta)+100;}
 get score(){return super.score+10;}
 set score(value){super.score=value+1;}
 arrow(){return ()=>super.read(3);}
}
var child=new Child(2);if(child.fromConstructor!==3)throw 'constructor member home object';
child.score=4;if(child.value!==10||child.score!==21||child.read(2)!==112||Object.hasOwn(Base.prototype,'value'))throw 'super receiver';
var method=Child.prototype.read,borrowed={value:7},descriptor=Object.getOwnPropertyDescriptor(Child.prototype,'score');
if(method.call(borrowed,2)!==109||descriptor.get.call(borrowed)!==18)throw 'borrowed super receiver';
descriptor.set.call(borrowed,5);if(borrowed.value!==12)throw 'borrowed super setter';
var arrow=Child.prototype.arrow.call({value:20});if(arrow.call({value:99})!==23)throw 'lexical super receiver';
class Other {read(delta){return this.value+delta+1000;}}
Object.setPrototypeOf(Child.prototype,Other.prototype);
if(method.call(borrowed,2)!==1114||arrow()!==1023)throw 'live home-object prototype lookup';
'ok';
"#);
}

#[test]
fn derived_class_static_super_uses_constructor_home_object_and_receiver() {
    succeeds(r#"
class Base {static label(suffix){return this.tag+suffix;}static get amount(){return this.count;}static set amount(value){this.count=value;}}
class Child extends Base {static label(suffix){return super.label(suffix)+'D';}static get amount(){return super.amount+1;}static set amount(value){super.amount=value*2;}static arrow(){return ()=>super.label('!');}}
Child.tag='C';Child.amount=3;
if(Child.count!==6||Child.amount!==7||Base.count!==undefined||Child.label('!')!=='C!D')throw 'static super receiver';
var method=Child.label,receiver={tag:'X'},descriptor=Object.getOwnPropertyDescriptor(Child,'amount');
descriptor.set.call(receiver,5);if(receiver.count!==10||descriptor.get.call(receiver)!==11||method.call(receiver,'?')!=='X?D')throw 'borrowed static super';
var arrow=Child.arrow.call(receiver);if(arrow.call({tag:'ignored'})!=='X!')throw 'lexical static super';
class Other {static label(suffix){return this.tag+'O'+suffix;}}
Object.setPrototypeOf(Child,Other);
if(method.call(receiver,'?')!=='XO?D'||arrow()!=='XO!')throw 'live static home-object prototype';
'ok';
"#);
}

#[test]
fn derived_class_super_constructor_target_is_live_and_distinct_from_instance_prototype() {
    succeeds(r#"
class First {constructor(value){this.which='first';this.value=value;this.target=new.target;}}
class Second {constructor(value){this.which='second';this.value=value*2;this.target=new.target;}}
class Child extends First {constructor(value){super(value);}}
var prototype=Child.prototype;Object.setPrototypeOf(Child,Second);
var child=new Child(41);
if(child.which!=='second'||child.value!==82||child.target!==Child||Object.getPrototypeOf(child)!==prototype||Object.getPrototypeOf(prototype)!==First.prototype||!(child instanceof First)||child instanceof Second)throw 'super constructor versus instance home object';
'ok';
"#);
}

#[test]
fn derived_class_invalid_script_heritage_is_typed_without_invoking_constructors() {
    succeeds(r#"
var errors=0;
for(var value of [1,{},()=>0])try{class Invalid extends value {}}catch(e){if(e.name==='TypeError'&&e instanceof TypeError)errors++;else throw e;}
function BadPrototype(){}BadPrototype.prototype=3;
try{class InvalidPrototype extends BadPrototype {}}catch(e){if(e.name==='TypeError')errors++;else throw e;}
if(errors!==4)throw 'invalid heritage values';
function NullPrototype(value){this.value=value;}NullPrototype.prototype=null;
class FromNullPrototype extends NullPrototype {}
var instance=new FromNullPrototype(43);
if(instance.value!==43||Object.getPrototypeOf(FromNullPrototype.prototype)!==null||Object.getPrototypeOf(instance)!==FromNullPrototype.prototype)throw 'Script base with null prototype';
'ok';
"#);
}

#[test]
fn derived_class_native_and_host_bases_remain_refused_without_native_operations() {
    for base in ["Array","Object","Error","Promise","NativeBase","HostObject"] {
        let mut runtime=Runtime::for_test();let mut host=NoHost::default();
        runtime.define_host_constructor("NativeBase",71,"nativeBase").unwrap();
        runtime.define_global("HostObject",Value::Host(73)).unwrap();
        runtime.execute("var body=0,after=0;",&mut host).unwrap();
        let source=format!("class Denied extends {base} {{constructor(){{body++;super();}}}}after++;");
        assert!(runtime.execute(&source,&mut host).is_err(),"native heritage admitted: {base}");
        assert_eq!((host.calls,host.writes),(0,0),"{base}");
        assert_eq!(runtime.binding_value(0,"body").unwrap().to_number(),0.0,"{base}");
        assert_eq!(runtime.binding_value(0,"after").unwrap().to_number(),0.0,"{base}");
    }
    let mut runtime=Runtime::for_test();let mut host=NoHost::default();
    runtime.define_host_constructor("NativeBase",71,"nativeBase").unwrap();
    assert!(runtime.execute("class Base{}class Child extends Base{}Object.setPrototypeOf(Child,NativeBase);new Child();",&mut host).is_err());
    assert_eq!((host.calls,host.writes),(0,0),"a live super-constructor change must not grant native construction");
}

#[test]
fn derived_class_super_early_errors_and_deferred_syntax_precede_program_effects() {
    for source in [
        "effects++;super();",
        "effects++;super.value;",
        "effects++;()=>super.value;",
        "effects++;function invalid(){super();}",
        "effects++;class Base{constructor(){super();}}",
        "effects++;class Base{method(){super();}}",
        "effects++;class Child extends Base{method(){super();}}",
        "effects++;class Child extends Base{constructor(){function invalid(){super();}super();}}",
    ] {
        let fault=Parser::parse(source,&mut MAX_REGEXP_WORK.clone()).err().expect("invalid super context");
        assert!(matches!(fault.exception.as_deref(),Some(Exception::Error("SyntaxError"))),"{source}: {fault:?}");
        no_effects(source);
    }
    for source in [
        "effects++;class Child extends Base{field=effects++;}",
        "effects++;class Child extends Base{#private(){}}",
        "effects++;class Child extends Base{[effects++](){}}",
        "effects++;class Child extends Base{static{effects++;}}",
        "effects++;class Child extends Base{async method(){}}",
        "effects++;class Child extends Base{*method(){}}",
    ] { no_effects(source); }
}

fn no_effects(source: &str) {
    let mut runtime=Runtime::for_test();let mut host=NoHost::default();
    runtime.execute("var effects=0;class Base{}",&mut host).unwrap();
    assert!(runtime.execute(source,&mut host).is_err(),"unsupported context admitted: {source}");
    assert_eq!(runtime.binding_value(0,"effects").unwrap().to_number(),0.0,"{source}");
    assert_eq!((host.calls,host.writes),(0,0));
}
