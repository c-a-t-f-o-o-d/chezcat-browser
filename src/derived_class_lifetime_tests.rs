//! Original derived-class lifetime, capacity and invocation-boundary controls.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn allow_eval(&mut self) -> Result<bool,String> {Ok(true)}
    fn get(&mut self,_:u32,_:&str)->Result<Value,String>{panic!("unexpected native read")}
    fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{panic!("unexpected native write")}
    fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{panic!("unexpected native call")}
}

#[test]
fn derived_class_extra_null_eval_snapshot_and_exact_original_source() {
    let mut runtime=Runtime::for_test();
    assert_eq!(runtime.execute(r#"
class NullBase extends null {constructor(){return {plain:true};}}
if(Object.getPrototypeOf(NullBase.prototype)!==null||Object.getPrototypeOf(NullBase)!==Function.prototype||!(new NullBase()).plain)throw 'null heritage';
class DefaultNull extends null {}
class ExplicitNull extends null {constructor(){super();}}
var errors=0;for(var C of [DefaultNull,ExplicitNull])try{new C();}catch(e){if(e.name==='TypeError')errors++;else throw e;}
if(errors!==2)throw 'null super is nonconstructor';
class First {constructor(value){this.kind='first';this.value=value;this.target=new.target;}read(){return this.value;}}
class Second {constructor(value){this.kind='second';this.value=value;this.target=new.target;}}
class Snapshot extends First {constructor(){super((Object.setPrototypeOf(Snapshot,Second),47));}}
var first=new Snapshot(),second=new Snapshot();
if(first.kind!=='first'||second.kind!=='second'||first.target!==Snapshot||second.target!==Snapshot)throw 'snapshot super constructor before arguments';
var early=0;
class Evaluated extends First {constructor(read=()=>eval('this')){try{read();}catch(e){if(e.name==='ReferenceError')early++;else throw e;}eval('super(53)');this.read=read;this.checked=eval('super.read()');}method(){return eval('super.read()');}}
var evaluated=new Evaluated();if(early!==1||evaluated.value!==53||evaluated.checked!==53||evaluated.method()!==53||evaluated.read()!==evaluated)throw 'direct eval live this/super';
var rejected=0;try{(0,eval)('super.read()');}catch(e){if(e.name==='SyntaxError')rejected++;else throw e;}if(rejected!==1)throw 'indirect eval super context';
'ok';
"#,&mut NoHost).unwrap().to_text(),"ok");
    for(source,expected)in[
        ("class B{}class /*original*/ D extends B {\r\nconstructor(value=1){super();this.value=value;}\r\nread(){return super.value;}\r\n}D.toString();","class /*original*/ D extends B {\r\nconstructor(value=1){super();this.value=value;}\r\nread(){return super.value;}\r\n}"),
        ("class B{read(){return 1;}}class D extends B{read /*gap*/ (value){return super.read()+value;}}D.prototype.read.toString();","read /*gap*/ (value){return super.read()+value;}"),
    ]{let mut runtime=Runtime::for_test();assert_eq!(runtime.execute(source,&mut NoHost).unwrap().to_text(),expected);}
}

#[test]
fn derived_class_extra_real_collection_keeps_live_this_and_saved_home_objects() {
    let mut runtime=Runtime::for_capacity_test();
    runtime.execute(r#"
var saved,live,missing;
function setup(){
 var B=class {read(){return this.value+1;}},D=class extends B {read(){return super.read()+2;}};
 saved=D.prototype.read;B=null;D=null;
 var L=class {},C=class extends L {constructor(value){var read=()=>this;super();this.value=value;this.read=read;}};
 live=new C(59);L=null;C=null;
 var M=class {},N=class extends M {constructor(){return {read:()=>this};}};
 missing=new N();M=null;N=null;
}
function pressure(){var trash;for(var n=0;n<800;n++)trash=function(){return n;};}
setup();pressure();
if(saved.call({value:61})!==64||live.read()!==live||live.value!==59)throw 'collected home object or live this';
var errors=0;try{missing.read();}catch(e){if(e.name==='ReferenceError')errors++;else throw e;}if(errors!==1)throw 'collected uninitialized this';
0;
"#,&mut NoHost).unwrap();
    assert!(runtime.function_arena.collections>0);
    assert!(runtime.function_arena.scope_retired>0);
    let retired=runtime.function_arena.retired;
    runtime.begin_job();
    runtime.execute("saved=null;live=null;missing=null;pressure();0;",&mut NoHost).unwrap();
    assert!(runtime.function_arena.retired>retired);
    assert!(runtime.invocation.is_none());assert!(runtime.transient_registration_is_empty());
}

#[test]
fn derived_class_extra_real_super_capacity_failure_is_fatal_and_next_job_recovers() {
    let mut runtime=Runtime::for_capacity_test();
    runtime.execute("var body=0,caught=0,finalized=0,after=0;class Base{constructor(){body++;}}class Child extends Base{}0;",&mut NoHost).unwrap();
    let objects=runtime.objects.len();let capacity=runtime.limits.heap.objects;
    runtime.limits.heap.objects=objects;
    let error=runtime.execute("try{new Child();after++;}catch(e){caught++;}finally{finalized++;}after++;",&mut NoHost).unwrap_err();
    assert!(error.contains("Object identity limit"),"{error}");
    assert!(runtime.invocation_failed_fatally());assert_eq!(runtime.objects.len(),objects);
    for name in["body","caught","finalized","after"]{assert_eq!(runtime.binding_value(0,name).unwrap().to_number(),0.0,"{name}");}
    assert!(runtime.invocation.is_none());assert!(runtime.transient_registration_is_empty());
    runtime.limits.heap.objects=capacity;runtime.begin_job();
    assert_eq!(runtime.execute("var child=new Child();body===1&&child instanceof Child&&caught===0&&finalized===0&&after===0;",&mut NoHost).unwrap().to_text(),"true");
}

#[test]
fn derived_class_extra_constructor_recursion_stops_on_the_original_one_mib_stack() {
    std::thread::Builder::new().stack_size(1024*1024).spawn(||{
        let mut runtime=Runtime::for_test();
        let source="var body=0,caught=0,finalized=0,after=0;class Base{constructor(){body++;new Child();}}class Child extends Base{}try{new Child();}catch(e){caught++;}finally{finalized++;}after++;";
        let error=runtime.execute(source,&mut NoHost).unwrap_err();
        assert!(error.contains("limit"),"{error}");assert!(runtime.invocation_failed_fatally());
        assert!(runtime.binding_value(0,"body").unwrap().to_number()>0.0);
        for name in["caught","finalized","after"]{assert_eq!(runtime.binding_value(0,name).unwrap().to_number(),0.0,"{name}");}
        assert!(runtime.invocation.is_none());assert!(runtime.transient_registration_is_empty());
    }).unwrap().join().unwrap();
}
