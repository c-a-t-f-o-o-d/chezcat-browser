//! Original DOMException internal slots and canonical per-realm Web IDL roots.
//! Primary contracts: https://webidl.spec.whatwg.org/#idl-DOMException,
//! https://webidl.spec.whatwg.org/#es-DOMException and #es-constants.
//! The inherited Error.prototype.toString remains the ordinary live-field path.
use super::*;

/// Standard input validation exceptions from the supported native factories.
pub(super) fn factory_validation(method: &str, name: &str) -> bool {
    match method {
        "createElement" | "createElementNS" | DOM_IMPLEMENTATION_CREATE_DOCUMENT |
        DOCUMENT_CREATE_ATTRIBUTE | DOCUMENT_CREATE_ATTRIBUTE_NS => matches!(name, "InvalidCharacterError" | "NamespaceError"),
        DOM_IMPLEMENTATION_CREATE_DOCTYPE | CREATE_PROCESSING_INSTRUCTION | "ProcessingInstruction" =>
            name == "InvalidCharacterError",
        CREATE_CDATA_SECTION => matches!(name, "InvalidCharacterError" | "NotSupportedError"),
        _ => false,
    }
}

pub(super) struct Data { name:String, message:String }
#[derive(Default)]
pub(super) struct Roots { prototype:Option<usize>, constructor:Option<usize> }
impl Roots {
    pub(super) fn reachability_edges(&self, add:&mut impl FnMut(function_reachability::Edge)) {
        if let Some(id)=self.prototype {add(function_reachability::Edge::Object(id));}
        if let Some(id)=self.constructor {add(function_reachability::Edge::Function(id));}
    }
}
#[derive(Clone,Copy)]
pub(super) enum Op { Constructor, Name, Message, Code }
impl Op {pub(super) fn arity(self)->usize {0}}

const CONSTANTS:[(&str,u16);25]=[
    ("INDEX_SIZE_ERR",1),("DOMSTRING_SIZE_ERR",2),("HIERARCHY_REQUEST_ERR",3),
    ("WRONG_DOCUMENT_ERR",4),("INVALID_CHARACTER_ERR",5),("NO_DATA_ALLOWED_ERR",6),
    ("NO_MODIFICATION_ALLOWED_ERR",7),("NOT_FOUND_ERR",8),("NOT_SUPPORTED_ERR",9),
    ("INUSE_ATTRIBUTE_ERR",10),("INVALID_STATE_ERR",11),("SYNTAX_ERR",12),
    ("INVALID_MODIFICATION_ERR",13),("NAMESPACE_ERR",14),("INVALID_ACCESS_ERR",15),
    ("VALIDATION_ERR",16),("TYPE_MISMATCH_ERR",17),("SECURITY_ERR",18),
    ("NETWORK_ERR",19),("ABORT_ERR",20),("URL_MISMATCH_ERR",21),
    ("QUOTA_EXCEEDED_ERR",22),("TIMEOUT_ERR",23),("INVALID_NODE_TYPE_ERR",24),
    ("DATA_CLONE_ERR",25),
];
fn legacy_code(name:&str)->u16 {
    match name {
        "IndexSizeError"=>1,"HierarchyRequestError"=>3,"WrongDocumentError"=>4,
        "InvalidCharacterError"=>5,"NoModificationAllowedError"=>7,"NotFoundError"=>8,
        "NotSupportedError"=>9,"InUseAttributeError"=>10,"InvalidStateError"=>11,
        "SyntaxError"=>12,"InvalidModificationError"=>13,"NamespaceError"=>14,
        "InvalidAccessError"=>15,"TypeMismatchError"=>17,"SecurityError"=>18,
        "NetworkError"=>19,"AbortError"=>20,"URLMismatchError"=>21,
        "QuotaExceededError"=>22,"TimeoutError"=>23,"InvalidNodeTypeError"=>24,
        "DataCloneError"=>25,_=>0,
    }
}

impl Runtime {
    pub(super) fn initialize_dom_exception(&mut self)->Result<(),Fault> {
        let prototype=self.new_object(None)?;
        self.objects[prototype].prototype=self.error_prototypes.get("Error").copied().map(Ordinary::Object);
        // ErrorData ancestry is distinct from implementing the interface: its
        // prototype has ErrorData, but never the branded name/message slots.
        self.objects[prototype].error_name=Some("DOMException");
        self.dom_exception_roots.prototype=Some(prototype);
        let constructor=self.native_function(Builtin::DomException(Op::Constructor),"DOMException")?;
        let Value::Function(id)=constructor else {unreachable!()};
        self.dom_exception_roots.constructor=Some(id);
        self.put_binding(self.global_environment,"DOMException".into(),Some(constructor),Declaration::Var,false)?;
        self.constructor_prototype("DOMException",Value::Object(prototype))?;
        for (name,op) in [("name",Op::Name),("message",Op::Message),("code",Op::Code)] {
            let Value::Function(get)=self.native_function(Builtin::DomException(op),&format!("get {name}"))? else {unreachable!()};
            self.store_accessor_property(prototype,name.into(),Accessors {get:Some(get),set:None},(true,true))?;
        }
        self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),
            Value::String("DOMException".into()),Some((false,false,true)))?;
        let properties=self.function_properties[id];
        for (name,code) in CONSTANTS {
            for target in [properties,prototype] {
                self.store_property(target,name.into(),Value::Number(f64::from(code)),Some((true,false,false)))?;
            }
        }
        Ok(())
    }
    fn dom_exception_copy(&mut self,text:&str)->Result<String,Fault> {
        if text.len()>self.limits.capacity.string_bytes {return Err(self.string_limit_fault());}
        self.work(text.len())?;
        let mut result=String::new();
        result.try_reserve_exact(text.len()).map_err(|_|Fault::new("Could not retain DOMException text."))?;
        result.push_str(text);Ok(result)
    }
    fn new_dom_exception_value(&mut self,name:String,message:String)->Result<Value,Fault> {
        self.ensure_global_intrinsics("DOMException")?;
        let bytes=name.len().checked_add(message.len()).ok_or_else(||Fault::new("DOMException retained accounting overflow."))?;
        let retained=self.value_bytes.checked_add(bytes).ok_or_else(||Fault::new("DOMException retained accounting overflow."))?;
        if retained>self.limits.heap.retained_values {return Err(Fault::new("DOMException retained-value limit reached."));}
        let object=self.new_object(None)?;
        self.objects[object].prototype=self.dom_exception_roots.prototype.map(Ordinary::Object);
        self.objects[object].error_name=Some("DOMException");
        self.objects[object].dom_exception=Some(Data {name,message});
        self.value_bytes=retained;Ok(Value::Object(object))
    }
    /// Native consumers create the genuine exception in the active canonical
    /// realm. Global replacement and user-written prototype properties are not
    /// consulted and no user code is called to choose its brand or text.
    pub(super) fn native_dom_exception(&mut self,name:&str,message:&str)->Result<Value,Fault> {
        self.ensure_global_intrinsics("DOMException")?;
        let name=self.dom_exception_copy(name)?;
        let message=self.dom_exception_copy(message)?;
        self.new_dom_exception_value(name,message)
    }
    fn dom_exception_string(&mut self,value:&Value,host:&mut dyn Host)->Result<String,Fault> {
        let value=self.copy_value(value)?;
        let primitive=self.primitive_with_hint(value,true,host)?;
        let text=primitive.script_text()?;
        self.work(text.len())?;
        if text.len()>self.limits.capacity.string_bytes {return Err(self.string_limit_fault());}
        Ok(text)
    }
    pub(super) fn dom_exception_construct(&mut self,args:&[Value],host:&mut dyn Host)->Result<Value,Fault> {
        self.step()?;
        self.check_native_call_stack("DOMException conversion call")?;
        self.call_depth+=1;
        let result=(|| {
            // Web IDL defaults apply to missing/undefined values. Conversion is
            // message first, name second; object-produced undefined is a string.
            let message=match args.first().filter(|v|!matches!(v,Value::Undefined)) {
                Some(value)=>self.dom_exception_string(value,host)?,None=>String::new(),
            };
            let name=match args.get(1).filter(|v|!matches!(v,Value::Undefined)) {
                Some(value)=>self.dom_exception_string(value,host)?,None=>"Error".into(),
            };
            self.new_dom_exception_value(name,message)
        })();
        self.call_depth-=1;result
    }
    pub(super) fn dom_exception_builtin(&mut self,op:Op,this:Value)->Result<Value,Fault> {
        self.step()?;
        if matches!(op,Op::Constructor) {
            return Err(Fault::exception("TypeError","DOMException constructor requires new."));
        }
        let Value::Object(id)=this else {return Err(Fault::exception("TypeError","DOMException getter has an incompatible receiver."));};
        let data=self.object(id)?.dom_exception.as_ref()
            .ok_or_else(||Fault::exception("TypeError","DOMException getter has an incompatible receiver."))?;
        match op {
            Op::Code=>Ok(Value::Number(f64::from(legacy_code(&data.name)))),
            Op::Name|Op::Message=>{
                let bytes=if matches!(op,Op::Name) {data.name.len()}else{data.message.len()};
                self.work(bytes)?;
                let data=self.objects[id].dom_exception.as_ref().unwrap();
                let text=if matches!(op,Op::Name) {data.name.clone()}else{data.message.clone()};
                self.checked_value(Value::String(text))
            }
            Op::Constructor=>unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self,_:u32,_:&str)->Result<Value,String>{Err("Unexpected host read".into())}
        fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{Err("Unexpected host write".into())}
        fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{Err("Unexpected host call".into())}
    }
    #[test]
    fn dom_exception_original_slots_descriptors_constants_and_error_ancestry() {
        let mut runtime=Runtime::for_test();
        runtime.execute(r#"
var e=new DOMException('stopped','AbortError'),empty=new DOMException();
if(!(e instanceof DOMException)||!(e instanceof Error)||Object.getPrototypeOf(e)!==DOMException.prototype)throw 'instance ancestry';
if(Object.getPrototypeOf(DOMException)!==Function.prototype||Object.getPrototypeOf(DOMException.prototype)!==Error.prototype)throw 'interface ancestry';
if(empty.name!=='Error'||empty.message!==''||empty.code!==0||e.name!=='AbortError'||e.message!=='stopped'||e.code!==20)throw 'stored slots/defaults';
if(Object.keys(e).length!==0||Object.hasOwn(e,'name')||Object.hasOwn(e,'message')||Object.hasOwn(e,'code'))throw 'internal slots';
if(DOMException.length!==0||DOMException.name!=='DOMException')throw 'constructor metadata';
var p=Object.getOwnPropertyDescriptor(DOMException,'prototype');if(p.writable||p.enumerable||p.configurable)throw 'prototype property';
for(var k of ['name','message','code']){var d=Object.getOwnPropertyDescriptor(DOMException.prototype,k);if(typeof d.get!=='function'||d.get.length!==0||d.set!==undefined||!d.enumerable||!d.configurable)throw 'getter descriptor';}
var d=Object.getOwnPropertyDescriptor(DOMException.prototype,'constructor');if(!d.writable||d.enumerable||!d.configurable||d.value!==DOMException)throw 'constructor descriptor';
var names=['INDEX_SIZE_ERR','DOMSTRING_SIZE_ERR','HIERARCHY_REQUEST_ERR','WRONG_DOCUMENT_ERR','INVALID_CHARACTER_ERR','NO_DATA_ALLOWED_ERR','NO_MODIFICATION_ALLOWED_ERR','NOT_FOUND_ERR','NOT_SUPPORTED_ERR','INUSE_ATTRIBUTE_ERR','INVALID_STATE_ERR','SYNTAX_ERR','INVALID_MODIFICATION_ERR','NAMESPACE_ERR','INVALID_ACCESS_ERR','VALIDATION_ERR','TYPE_MISMATCH_ERR','SECURITY_ERR','NETWORK_ERR','ABORT_ERR','URL_MISMATCH_ERR','QUOTA_EXCEEDED_ERR','TIMEOUT_ERR','INVALID_NODE_TYPE_ERR','DATA_CLONE_ERR'];
for(var i=0;i<names.length;i++){for(var owner of [DOMException,DOMException.prototype]){var c=Object.getOwnPropertyDescriptor(owner,names[i]);if(c.value!==i+1||c.writable||!c.enumerable||c.configurable)throw 'legacy constant';}}
if(Object.prototype.toString.call(e)!=='[object DOMException]'||Object.prototype.toString.call(DOMException.prototype)!=='[object DOMException]')throw 'interface tag';
if(Object.hasOwn(DOMException.prototype,'toString')||e.toString!==Error.prototype.toString||e.toString()!=='AbortError: stopped')throw 'inherited Error toString';
var old=Object.getOwnPropertyDescriptor(DOMException.prototype,'name').get;
for(var receiver of [{},DOMException.prototype,Object.create(DOMException.prototype),new Error()]){var caught=false;try{old.call(receiver);}catch(x){caught=x instanceof TypeError;}if(!caught)throw 'brand';}
var effects=0;e.name={toString:function(){effects++;return 'bad';}};if(e.name!=='AbortError'||effects!==0)throw 'readonly no conversion';
var caught=false;try{(function(){'use strict';e.message='bad';})();}catch(x){caught=x instanceof TypeError;}if(!caught||e.message!=='stopped')throw 'strict readonly';
Object.defineProperty(e,'name',{value:'Changed',configurable:true});Object.defineProperty(e,'message',{value:'message',configurable:true});
if(e.code!==20||e.toString()!=='Changed: message'||old.call(e)!=='AbortError')throw 'shadow versus stored slots';
for(var n of ['Error','NotAllowedError','DOMStringSizeError','NoDataAllowedError','ValidationError','Unlisted','aborterror']){if(new DOMException('',n).code!==0)throw 'unknown or obsolete name';}
if(new DOMException('','NotSupportedError').code!==9||new DOMException('','DataCloneError').code!==25)throw 'mapped code';
"#,&mut NoHost).unwrap();
    }
    #[test]
    fn dom_exception_domstring_conversion_order_defaults_abrupt_identity_and_new() {
        let mut runtime=Runtime::for_test();
        runtime.execute(r#"
var order='',marker={},arg=function(k,v){return {toString:function(){order+=k;return v;}};};
var e=new DOMException(arg('M','message'),arg('N','name'),arg('X','extra'));if(order!=='MN'||e.message!=='message'||e.name!=='name')throw 'ordered conversions/extras';
if(new DOMException(undefined,undefined).name!=='Error'||new DOMException(undefined).message!==''||new DOMException(null,null).name!=='null'||new DOMException(null).message!=='null')throw 'defaults/null';
if(new DOMException({toString:function(){return undefined;}}).message!=='undefined')throw 'produced undefined';
order='';var caught=false;try{new DOMException({toString:function(){throw marker;}},arg('N','bad'));}catch(x){caught=x===marker;}if(!caught||order!=='')throw 'abrupt first conversion';
order='';caught=false;try{new DOMException(Symbol(),arg('N','bad'));}catch(x){caught=x instanceof TypeError;}if(!caught||order!=='')throw 'Symbol first';
order='';caught=false;try{DOMException(arg('M','bad'),arg('N','bad'));}catch(x){caught=x instanceof TypeError;}if(!caught||order!=='')throw 'new before coercion';
var get=Object.getOwnPropertyDescriptor(DOMException.prototype,'name').get;caught=false;try{new get();}catch(x){caught=x instanceof TypeError;}if(!caught)throw 'getter nonconstructor';
var C=DOMException.bind(null,'bound');if(new C('AbortError').message!=='bound'||!(new C() instanceof DOMException))throw 'bound constructor';
"#,&mut NoHost).unwrap();
    }
    #[test]
    fn native_dom_exception_uses_canonical_slots_without_global_or_accessor_calls() {
        let mut runtime=Runtime::for_test();
        runtime.execute("var C=DOMException,P=C.prototype,get=Object.getOwnPropertyDescriptor(P,'name').get,calls=0;Object.defineProperty(P,'name',{get:function(){calls++;throw 'user getter';},configurable:true});DOMException=function(){calls++;throw 'global constructor';};",&mut NoHost).unwrap();
        let value=runtime.native_dom_exception("AbortError","native stop").unwrap();
        runtime.define_global("nativeReason",value).unwrap();
        runtime.execute("if(calls!==0||Object.getPrototypeOf(nativeReason)!==P||!(nativeReason instanceof C)||!(nativeReason instanceof Error)||get.call(nativeReason)!=='AbortError'||nativeReason.message!=='native stop'||nativeReason.code!==20)throw 'canonical native reason';",&mut NoHost).unwrap();
    }
    #[test]
    fn dom_exception_realms_have_distinct_canonical_roots_and_saved_constructor_routing() {
        let mut runtime=Runtime::for_capacity_test();
        let main=runtime.native_dom_exception("AbortError","main").unwrap();
        let realm=runtime.allocate_intrinsic_realm().unwrap();
        let (child,constructor,error)=runtime.with_intrinsic_realm(Some(realm),|runtime| {
            let child=runtime.native_dom_exception("NotAllowedError","child")?;
            let constructor=runtime.binding_value(runtime.global_environment,"DOMException")?;
            let error=runtime.binding_value(runtime.global_environment,"Error")?;
            Ok((child,constructor,error))
        }).unwrap();
        runtime.define_global("mainReason",main).unwrap();runtime.define_global("childReason",child).unwrap();
        runtime.define_global("ChildException",constructor).unwrap();runtime.define_global("ChildError",error).unwrap();
        runtime.execute("if(!(mainReason instanceof DOMException)||childReason instanceof DOMException||!(childReason instanceof ChildException)||!(childReason instanceof ChildError)||childReason instanceof Error)throw 'realm brands';var made=new ChildException('saved','AbortError');if(Object.getPrototypeOf(made)!==ChildException.prototype||made instanceof DOMException||made.code!==20)throw 'saved callee realm';var get=Object.getOwnPropertyDescriptor(DOMException.prototype,'name').get;if(get.call(childReason)!=='NotAllowedError')throw 'cross-realm genuine brand';",&mut NoHost).unwrap();
        runtime.collect_environments();
        assert_eq!(runtime.active_realm,None);
        runtime.execute("if(childReason.code!==0||new ChildException().name!=='Error')throw 'retained child roots';",&mut NoHost).unwrap();
    }
    #[test]
    fn dom_exception_canonical_roots_survive_global_and_constructor_link_removal() {
        let mut runtime=Runtime::for_test();
        runtime.execute("var canonicalProto=DOMException.prototype;delete canonicalProto.constructor;DOMException=17;",&mut NoHost).unwrap();
        let constructor=runtime.dom_exception_roots.constructor.unwrap();
        runtime.collect_environments();
        assert!(matches!(runtime.functions[constructor],Function::Native(Builtin::DomException(Op::Constructor))));
        let reason=runtime.native_dom_exception("AbortError","retained").unwrap();
        runtime.define_global("reason",reason).unwrap();
        runtime.execute("if(DOMException!==17||Object.getPrototypeOf(reason)!==canonicalProto||reason.name!=='AbortError'||reason.code!==20||reason.message!=='retained')throw 'canonical root or global resurrection';",&mut NoHost).unwrap();
    }
    #[test]
    fn native_dom_exception_retained_failure_is_atomic_and_recovers() {
        let mut runtime=Runtime::for_test();
        runtime.native_dom_exception("Error","").unwrap();
        let before=runtime.objects.len();let old=runtime.limits.heap.retained_values;
        runtime.limits.heap.retained_values=runtime.value_bytes;
        assert!(runtime.native_dom_exception("AbortError","stop").unwrap_err().message.contains("retained"));
        assert_eq!(runtime.objects.len(),before);
        runtime.limits.heap.retained_values=old;runtime.begin_job();
        assert!(runtime.native_dom_exception("AbortError","stop").is_ok());
    }
    #[test]
    fn dom_exception_recursive_coercion_is_bounded_and_restores_depth() {
        std::thread::Builder::new().stack_size(1024*1024).spawn(|| {
            let mut runtime=Runtime::for_test();
            let error=runtime.execute("var v={toString:function(){return new DOMException(v).message;}};new DOMException(v);",&mut NoHost).unwrap_err();
            assert!(error.contains("call limit"),"{error}");assert_eq!(runtime.call_depth,0);
            runtime.begin_job();assert_eq!(runtime.execute("6*7;",&mut NoHost).unwrap().to_number(),42.0);
        }).unwrap().join().unwrap();
    }
}
