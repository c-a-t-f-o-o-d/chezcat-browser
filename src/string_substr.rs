//! Annex B substring-by-count, using the engine's explicit UTF-16 result boundary.
//! https://tc39.es/ecma262/#sec-string.prototype.substr
use super::{Fault, Host, Runtime, Value, WorkKind};

impl Runtime {
    pub(super) fn string_substr(&mut self, text: &str, args: &[Value], host: &mut dyn Host) -> Result<Value,Fault> {
        self.work_as(text.len().saturating_mul(3),WorkKind::StringMethodInput)?;
        let units: Vec<_> = text.encode_utf16().collect();
        let size = units.len();
        let start = self.binary_relative(args.first(),size,0,host)?;
        let length = self.binary_integer(args.get(1),size as f64,host)?.clamp(0.0,size as f64) as usize;
        let end = start.saturating_add(length).min(size);
        self.units_string(&units[start..end]).map(Value::String)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self,_:u32,_:&str)->Result<Value,String>{Err("No native reads".into())}
        fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{Err("No native writes".into())}
        fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{Err("No native calls".into())}
    }
    #[test]
    fn annex_b_substr_clamps_start_and_count_by_utf16_units() {
        let mut runtime=Runtime::for_test();
        runtime.execute(r#"
var s='012345',tests=[
[s.substr(),'012345'],[s.substr(undefined,undefined),'012345'],[s.substr(2,3),'234'],
[s.substr(-2),'45'],[s.substr(-20,3),'012'],[s.substr(20,3),''],
[s.substr(1.9,2.9),'12'],[s.substr(-2.9,1.9),'4'],[s.substr(-0,2),'01'],
[s.substr(NaN,NaN),''],[s.substr(-Infinity,Infinity),'012345'],[s.substr(Infinity,Infinity),''],
[s.substr(2,-Infinity),''],[s.substr(2,-1),''],[s.substr(2,null),''],[s.substr(2,true),'2'],
[s.substr(2,{valueOf:function(){return undefined;}}),''],
['A😀Z'.substr(1,2),'😀'],['A😀Z'.substr(-1,1),'Z'],['A😀Z'.substr(3,Infinity),'Z'],['A😀Z'.substr(2,0),'']
];for(var i=0;i<tests.length;i++)if(tests[i][0]!==tests[i][1])throw 'case '+i;
var f=String.prototype.substr;if(f.call(12345,1,2)!=='23'||f.call(new String('abc'),1)!=='bc')throw 'generic/wrapper';
if(f.call(true,1,2)!=='ru'||f.call({toString:function(){return 'custom';}},2,2)!=='st')throw 'generic string conversion';
"#,&mut NoHost).unwrap();
    }
    #[test]
    fn annex_b_substr_converts_receiver_start_length_in_order_even_for_empty_results() {
        let mut runtime=Runtime::for_test();
        runtime.execute(r#"
var f=String.prototype.substr,order='',marker={};
var receiver={[Symbol.toPrimitive]:function(h){order+='receiver:'+h+';';return 'abcde';}};
var start={[Symbol.toPrimitive]:function(h){order+='start:'+h+';';return -3;}};
var length={[Symbol.toPrimitive]:function(h){order+='length:'+h+';';return 2;}};
if(f.call(receiver,start,length)!=='cd'||order!=='receiver:string;start:number;length:number;')throw order;
order='';var bad={valueOf:function(){order+='length;';throw marker;}};
var caught=false;try{f.call('',Infinity,bad);}catch(e){caught=e===marker;}
if(!caught||order!=='length;')throw 'empty still converts count';
order='';caught=false;try{f.call(null,start,length);}catch(e){caught=e instanceof TypeError;}
if(!caught||order!=='')throw 'receiver brand first';
order='';caught=false;try{f.call('abc',Symbol('bad'),length);}catch(e){caught=e instanceof TypeError;}
if(!caught||order!=='')throw 'start abrupt completion';
caught=false;try{f.call('',0,Symbol('bad'));}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'length Symbol';
caught=false;try{f.call(Symbol('bad'),0,1);}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'receiver Symbol';
if('abc'.substr({valueOf:function(){return undefined;}},undefined)!=='abc')throw 'start undefined conversion';
if('abc'.substr(0,1,{valueOf:function(){throw 'extra converted';}})!=='a')throw 'extra argument';
"#,&mut NoHost).unwrap();
    }
    #[test]
    fn annex_b_substr_has_real_native_metadata_and_mutable_prototype_property() {
        let mut runtime=Runtime::for_test();
        runtime.execute(r#"
var f=String.prototype.substr,d=Object.getOwnPropertyDescriptor(String.prototype,'substr');
if(f!==String.prototype.substr||f.name!=='substr'||f.length!==2||d.value!==f||d.enumerable||!d.writable||!d.configurable)throw 'native metadata';
if(Function.prototype.toString.call(f).indexOf('[native code]')<0)throw 'native source';
String.prototype.substr=function(){return 'replacement';};if('abc'.substr(0,1)!=='replacement')throw 'replacement';
if(!delete String.prototype.substr||'substr' in String.prototype||'abc'.substr!==undefined)throw 'deletion';
String.prototype.substr=f;if('abc'.substr(1,1)!=='b')throw 'restore';
"#,&mut NoHost).unwrap();
    }
    #[test]
    fn annex_b_substr_keeps_work_and_unrepresentable_surrogate_failures_explicit() {
        let mut runtime=Runtime::for_test();
        let error=runtime.execute("'A😀Z'.substr(1,1);",&mut NoHost).unwrap_err();
        assert!(error.contains("lone UTF-16 surrogate"),"{error}");
        runtime.begin_job();
        assert_eq!(runtime.execute("'A😀Z'.substr(1,2);",&mut NoHost).unwrap().to_text(),"😀");
        runtime.constrain_job(usize::MAX,0,usize::MAX,std::time::Instant::now()+std::time::Duration::from_secs(10));
        let error=runtime.string_substr("abc",&[],&mut NoHost).unwrap_err().display();
        assert!(error.contains("String/copy work limit"),"{error}");
        runtime.begin_job();
        assert_eq!(runtime.string_substr("abc",&[Value::Number(1.0)],&mut NoHost).unwrap().to_text(),"bc");
    }
}
