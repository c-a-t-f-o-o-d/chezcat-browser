//! Original native-host CreateListFromArrayLike controls, not a library shim.
use super::*;

struct ApplyHost {
    length: Value,
    values: Vec<Value>,
    log: Vec<String>,
    fail: Option<usize>,
    prototype: bool,
}
impl Default for ApplyHost {
    fn default()->Self {Self {length:Value::Number(3.0),values:vec![Value::Number(17.0),Value::Undefined,Value::String("third".into())],log:Vec::new(),fail:None,prototype:false}}
}
impl Host for ApplyHost {
    fn get(&mut self,_:u32,name:&str)->Result<Value,String> {
        self.log.push(name.into());
        if name=="length" {return Ok(self.length.clone());}
        if let Ok(index)=name.parse::<usize>() {
            if self.fail==Some(index) {return Err("Original host indexed capability refusal".into());}
            return Ok(self.values.get(index).cloned().unwrap_or(Value::Undefined));
        }
        Ok(Value::Undefined)
    }
    fn has(&mut self,_:u32,name:&str)->Result<bool,String> {self.log.push(format!("has:{name}"));Ok(false)}
    fn ordinary_prototype(&self,_:u32)->bool {self.prototype}
    fn method(&self,_:u32,name:&str)->Option<&'static str> {(name=="mark").then_some("mark")}
    fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String> {Err("No original host writes".into())}
    fn call(&mut self,_:u32,method:&str,_:&[Value])->Result<Value,String> {
        if method!="mark" {return Err("No original host method".into());}
        self.log.push("mark".into());self.values[2]=Value::String("changed".into());Ok(Value::Undefined)
    }
}
fn runtime()->Runtime {let mut r=Runtime::for_test();r.define_global("native",Value::Host(1234)).unwrap();r}

#[test]
fn host_apply_gets_length_once_and_each_position_with_undefined_not_holes() {
    let mut r=runtime();let mut host=ApplyHost::default();
    let out=r.execute("var receiver={},called=0;function capture(a,b,c){called++;if(this!==receiver||arguments.length!==3||a!==17||b!==undefined||c!=='third'||!(1 in arguments))throw 'host args';return 'ok';}capture.apply(receiver,native);",&mut host).unwrap();
    assert_eq!(out.to_text(),"ok");assert_eq!(host.log,["length","0","1","2"]);
    host.log.clear();
    assert_eq!(r.execute("var failed=false;try{Function.prototype.apply.call({},null,native);}catch(e){failed=e instanceof TypeError;}if(!failed||called!==1)throw 'callable target first';'ok';",&mut host).unwrap().to_text(),"ok");
    assert!(host.log.is_empty(),"Noncallable target must reject before touching argArray");
}

#[test]
fn host_apply_tolength_coercion_is_ordered_and_observes_live_values() {
    let mut r=runtime();let mut host=ApplyHost::default();
    host.length=r.execute("var conversions=0;({valueOf:function(){conversions++;native.mark();return 3.9;}});",&mut host).unwrap();
    assert_eq!(r.execute("function f(a,b,c){if(conversions!==1||c!=='changed'||arguments.length!==3)throw 'length conversion';return 'ok';}f.apply(null,native);",&mut host).unwrap().to_text(),"ok");
    assert_eq!(host.log,["length","mark","0","1","2"]);
    for length in [f64::NAN,-1.0,-0.0,0.4] {
        r.begin_job();host.log.clear();host.length=Value::Number(length);
        assert_eq!(r.execute("(function(){return arguments.length;}).apply(null,native);",&mut host).unwrap().to_number(),0.0);
        assert_eq!(host.log,["length"]);
    }
}

#[test]
fn host_apply_real_reentrant_getter_and_abrupt_completion_preserve_partial_values() {
    let mut r=runtime();let mut host=ApplyHost {prototype:true,..ApplyHost::default()};
    host.values[0]=r.execute("({sentinel:17});",&mut host).unwrap();
    assert_eq!(r.execute("Object.defineProperty(Object.prototype,'1',{configurable:true,get:function(){var discarded;for(var i=0;i<300;i++)discarded={n:i};return {kept:31};}});function capture(a,b,c){if(a.sentinel!==17||b.kept!==31||c!=='third')throw 'partial values';return 'ok';}var result=capture.apply(null,native);delete Object.prototype[1];result;",&mut host).unwrap().to_text(),"ok");
    r.begin_job();host.log.clear();
    assert_eq!(r.execute("var marker={},called=false,caught=false;Object.defineProperty(Object.prototype,'1',{configurable:true,get:function(){throw marker;}});try{(function(){called=true;}).apply(null,native);}catch(e){caught=e===marker;}delete Object.prototype[1];if(!caught||called)throw 'getter abrupt identity';'ok';",&mut host).unwrap().to_text(),"ok");
    assert!(!host.log.iter().any(|name|name=="2"),"No later Get after abrupt completion");
}

#[test]
fn host_apply_capacity_and_host_fatal_refusal_never_call_target_and_recover() {
    let mut r=runtime();let mut host=ApplyHost {length:Value::Number(9.0),..ApplyHost::default()};
    r.limits.capacity.arguments=8;
    let error=r.execute("var called=0,caught=false,finished=false;function target(){called++;return arguments.length;}try{target.apply(null,native);}catch(e){caught=true;}finally{finished=true;}",&mut host).unwrap_err();
    assert!(error.contains("argument limit"),"{error}");assert_eq!(host.log,["length"]);
    r.begin_job();host.length=Value::Number(3.0);host.fail=Some(1);host.log.clear();
    let error=r.execute("try{target.apply(null,native);}catch(e){caught=true;}finally{finished=true;}",&mut host).unwrap_err();
    assert!(error.contains("capability refusal"),"{error}");assert_eq!(host.log,["length","0","1"]);
    r.begin_job();host.fail=None;host.log.clear();
    assert_eq!(r.execute("if(called||caught||finished)throw 'fatal became ordinary or target ran';if(target.apply(null,native)!==3||called!==1)throw 'fresh job recovery';'ok';",&mut host).unwrap().to_text(),"ok");
}

#[test]
fn host_apply_small_work_allowance_restores_construction_guard() {
    let mut r=runtime();let mut host=ApplyHost::default();
    r.steps=0;
    assert!(r.apply_arguments(&Value::Host(1234),&mut host).is_err());
    assert!(!host.log.iter().any(|name|name=="0"),"No indexed Get after work refusal");
    r.begin_job();host.log.clear();
    assert_eq!(r.execute("(function(){return arguments.length;}).apply(null,native);",&mut host).unwrap().to_number(),3.0);
}
