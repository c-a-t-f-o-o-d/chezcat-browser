//! Bounded, read-only diagnostics. Never invoke page getters or conversions.
use super::{Exception, Fault, FaultLocation, Runtime, StoredValue, Value};
use std::collections::VecDeque;
use std::fmt::Write;

const MAX_ERROR_ORIGINS: usize = 128;

#[derive(Default)]
pub(super) struct Data {
    caught: Vec<String>,
    omitted: usize,
    entries: [Entries; 2],
    last_fault: Option<(&'static str, &'static str)>,
    origins: VecDeque<NativeOrigin>,
}

struct NativeOrigin {
    identity: usize,
    kind: &'static str,
    message: String,
    location: Box<FaultLocation>,
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoHost;
    impl super::super::Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("No host read".into()) }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("No host write".into()) }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("No host call".into()) }
    }
    #[test]
    fn native_error_rethrow_reports_original_location_without_reading_page_getters() {
        let mut runtime = Runtime::for_test();
        let script = "var saved,hits=0;\n\
            function fail(){return null.missing;}\n\
            try{fail();}catch(e){saved=e;Object.defineProperty(e,'message',{get:function(){hits++;return 'altered';}});}\n\
            try{throw saved;}catch(e){if(e!==saved)throw 'identity changed';}\n\
            throw saved;";
        let error = runtime.execute(script, &mut NoHost).unwrap_err();
        assert!(error.contains("JavaScript error at line 5,"), "{error}");
        assert!(error.contains("Original native error at line 2,"), "{error}");
        assert!(error.contains("Cannot access a property of null or undefined."), "{error}");
        assert!(error.contains("null.missing"), "{error}");
        runtime.begin_job();
        assert_eq!(runtime.execute("hits;", &mut NoHost).unwrap().to_number(), 0.0);
    }

    #[test]
    fn console_native_error_bounded_utf8_output_copy_refusal_and_recovery_do_not_allocate_script_state() {
        let mut runtime=Runtime::for_test();
        let error=runtime.execute("var e;try{null.missing;}catch(x){e=x;}e;",&mut NoHost).unwrap();
        let objects=runtime.objects.len();let functions=runtime.functions.len();let retained=runtime.value_bytes;
        runtime.begin_job();runtime.copy_bytes=0;
        assert!(runtime.console_message(&[error.clone()],2048,&NoHost).is_err());
        assert_eq!((runtime.objects.len(),runtime.functions.len(),runtime.value_bytes),(objects,functions,retained));
        runtime.begin_job();let text=runtime.console_message(&[error],2048,&NoHost).unwrap();assert!(text.contains("Original native error at line 1"),"{text}");
        let text=runtime.console_message(&[Value::String("ab\u{1f431}tail".into()),Value::String("later".into())],5,&NoHost).unwrap();assert!(text.len()<=5);assert!(text.starts_with("ab"));assert!(!text.contains("tail"));
        assert_eq!((runtime.objects.len(),runtime.functions.len(),runtime.value_bytes),(objects,functions,retained));
    }

    #[test]
    fn console_native_error_host_dispatch_refusal_publishes_nothing_and_unrelated_args_keep_identity() {
        struct Printer {printed:Vec<String>,ordinary:Option<Value>}
        impl super::super::Host for Printer {
            fn get(&mut self,_:u32,_:&str)->Result<Value,String>{Err("No read".into())}
            fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{Err("No write".into())}
            fn call(&mut self,_:u32,_:&str,args:&[Value])->Result<Value,String>{self.ordinary=args.first().cloned();Ok(Value::Undefined)}
            fn console_message_limit(&self,object:u32,method:&str)->Option<usize>{(object==7&&method=="error").then_some(2048)}
            fn print_console(&mut self,_:u32,_:&str,message:&str)->Result<Value,String>{self.printed.push(message.to_owned());Ok(Value::Undefined)}
        }
        let mut host=Printer {printed:Vec::new(),ordinary:None};let mut runtime=Runtime::for_test();
        let error=runtime.execute("TypeError('dispatch');",&mut NoHost).unwrap();let objects=runtime.objects.len();
        runtime.begin_job();runtime.copy_bytes=0;
        assert!(runtime.call_host(7,"error",&[error.clone()],&mut host).is_err());assert!(host.printed.is_empty());assert!(host.ordinary.is_none());assert_eq!(runtime.objects.len(),objects);
        runtime.begin_job();runtime.call_host(7,"error",&[error.clone()],&mut host).unwrap();assert_eq!(host.printed,["TypeError: dispatch"]);
        runtime.call_host(7,"ordinary",&[error.clone()],&mut host).unwrap();assert!(matches!((host.ordinary,error),(Some(Value::Object(a)),Value::Object(b)) if a==b));
    }

    #[test]
    fn stage_observations_classify_typed_faults_without_user_property_reads() {
        let mut runtime = Runtime::for_test();
        assert!(runtime.caught_error_report().contains("stage=unknown kind=unknown"));
        runtime.execute("var hits=0;var e=TypeError('typed');Object.defineProperty(e,'name',{get:function(){hits++;return 'fake';}});throw e;", &mut NoHost).unwrap_err();
        let snapshot = runtime.caught_error_report();
        assert!(snapshot.contains("execute: attempted=1 succeeded=0 failed=1"));
        assert!(snapshot.contains("stage=execute kind=TypeError"));
        assert_eq!(snapshot, runtime.caught_error_report());
        runtime.begin_job();
        let function = runtime.execute("(function(){throw {name:'TypeError',toString:function(){hits++;return 'fake';}};});", &mut NoHost).unwrap();
        runtime.invoke(function, &[], Value::Undefined, &mut NoHost).unwrap_err();
        assert!(runtime.caught_error_report().contains("invoke: attempted=1 succeeded=0 failed=1"));
        assert!(runtime.caught_error_report().contains("stage=invoke kind=thrown-value"));
        runtime.begin_job();
        runtime.execute("queueMicrotask(function(){throw RangeError('queued');});", &mut NoHost).unwrap();
        assert_eq!(runtime.microtask_checkpoint(&mut NoHost).unwrap().len(), 1);
        assert!(runtime.caught_error_report().contains("stage=microtask kind=RangeError"));
        runtime.begin_job();
        runtime.execute("unimplementedCapability();", &mut NoHost).unwrap_err();
        // A typed ReferenceError is distinct from a fatal engine capability failure.
        assert!(runtime.caught_error_report().contains("kind=ReferenceError"));
        runtime.execute("hits;", &mut NoHost).map(|value| assert_eq!(value.to_number(), 0.0)).unwrap();
        let fault = Fault::new("TypeError-looking text is not a JS exception");
        runtime.record_stage_fault("execute", &fault);
        assert!(runtime.caught_error_report().contains("kind=fatal"));
    }
}

struct Entries { attempted: Option<u64>, succeeded: Option<u64>, failed: Option<u64> }
impl Default for Entries {
    fn default() -> Self { Self { attempted: Some(0), succeeded: Some(0), failed: Some(0) } }
}
fn increment(value: &mut Option<u64>) { *value = value.and_then(|n| n.checked_add(1)); }
fn number(value: Option<u64>) -> String { value.map_or_else(|| "unknown".into(), |n| n.to_string()) }

fn bounded(text: &str) -> String {
    text.chars().take(512).flat_map(char::escape_default).take(2048).collect()
}

impl Runtime {
    // Error objects keep their existing public identity and fields. Origin
    // records are private, fixed-count diagnostics and never call page code.
    #[inline(never)]
    pub(super) fn materialize_native_error(
        &mut self, kind: &'static str, message: String, location: Option<Box<FaultLocation>>,
    ) -> Result<Value, Fault> {
        let detail = location.as_ref().map(|_| bounded(&message));
        let value = self.error_object(kind, Some(message))?;
        if let (Value::Object(identity), Some(location), Some(message)) = (&value, location, detail) {
            let origins = &mut self.diagnostics.origins;
            if origins.len() == MAX_ERROR_ORIGINS { origins.pop_front(); }
            if origins.try_reserve(1).is_ok() {
                origins.push_back(NativeOrigin { identity: *identity, kind, message, location });
            }
        }
        Ok(value)
    }

    // Only API entry outcomes are counted here; these are not statement counts.
    // Keep diagnostic scratch out of recursive evaluator/native-call frames.
    #[inline(never)]
    pub(super) fn record_entry_result(&mut self, invoke: bool, result: &Result<Value, Fault>) {
        let entries = &mut self.diagnostics.entries[usize::from(invoke)];
        increment(&mut entries.attempted);
        if result.is_ok() { increment(&mut entries.succeeded); }
        else { increment(&mut entries.failed); }
        if let Err(fault) = result { self.record_stage_fault(if invoke { "invoke" } else { "execute" }, fault); }
    }

    #[inline(never)]
    pub(super) fn record_stage_fault(&mut self, stage: &'static str, fault: &Fault) {
        let kind = match fault.exception.as_deref() {
            None => "fatal",
            Some(Exception::Error(kind)) => kind,
            Some(Exception::Value(Value::Object(id))) => self.objects.get(*id)
                .and_then(|object| object.error_name).unwrap_or("thrown-value"),
            _ => "thrown-value",
        };
        self.diagnostics.last_fault = Some((stage, kind));
    }

    #[inline(never)]
    pub(super) fn record_caught_error(&mut self, fault: &Fault, stage: &str) {
        let Some(exception) = &fault.exception else { return; };
        let native = match &**exception {
            Exception::Error(_) => true,
            Exception::Value(Value::Object(id)) => self.objects.get(*id).is_some_and(|object| object.error_name.is_some()),
            _ => false,
        };
        if !native { return; }
        if self.diagnostics.caught.len() == 8 {
            self.diagnostics.omitted = self.diagnostics.omitted.saturating_add(1);
            return;
        }
        let detail = match &**exception {
            Exception::Error(kind) => format!("{kind}: {}", bounded(&fault.message)),
            Exception::Value(value) => self.diagnostic_value(value),
        };
        self.diagnostics.caught.push(format!("Caught native Error sample ({stage}): {detail}\n"));
    }

    pub(crate) fn caught_error_report(&self) -> String {
        let mut text = String::new();
        for (index, stage) in ["execute", "invoke"].iter().enumerate() {
            let entry = &self.diagnostics.entries[index];
            text.push_str(&format!("Engine entry {stage}: attempted={} succeeded={} failed={}\n",
                number(entry.attempted), number(entry.succeeded), number(entry.failed)));
        }
        let (stage, kind) = self.diagnostics.last_fault.unwrap_or(("unknown", "unknown"));
        text.push_str(&format!("Last escaped engine fault: stage={stage} kind={kind}\n"));
        text.push_str(&self.diagnostics.caught.concat());
        if self.diagnostics.omitted != 0 {
            text.push_str(&format!("Further caught native Error samples omitted: {}.\n", self.diagnostics.omitted));
        }
        text
    }

    pub(super) fn diagnostic_value(&self, value: &Value) -> String {
        if let Value::Object(id) = value {
            if let Some(object) = self.objects.get(*id) {
                if let Some(kind) = object.error_name {
                    if let Some(property) = object.properties.get("message") {
                        if property.getter.is_none() && property.accessor.is_none() {
                            if let StoredValue::Value(Value::String(message)) = &property.value {
                                return format!("{kind}: {}", bounded(message));
                            }
                        }
                    }
                    return kind.into();
                }
            }
        }
        match value {
            Value::String(text) => bounded(text),
            _ => value.to_text(),
        }
    }

    #[inline(never)]
    pub(super) fn display_fault(&self, mut fault: Fault) -> String {
        let mut origin = None;
        if let Some(exception) = &fault.exception {
            if let Exception::Value(value @ Value::Object(id)) = &**exception {
                if self.objects.get(*id).is_some_and(|object| object.error_name.is_some()) {
                    fault.message = format!("Uncaught exception: {}", self.diagnostic_value(value));
                    origin = self.diagnostics.origins.iter().rev().find(|origin| origin.identity == *id);
                }
            }
        }
        let mut text = fault.display();
        if let Some(origin) = origin {
            let location = &origin.location;
            let _ = write!(text, "\nOriginal native error at line {}, column {}: {}: {}\nOriginal source: {}",
                location.line, location.column, origin.kind, origin.message, location.context);
        }
        text
    }
}

/// A two-pass bounded writer: count first, reserve under the job's copy budget,
/// then render identically. Formatting never calls page code or clones an
/// unbounded script string. The display limit is owned by the native printer.
struct ConsoleWriter<'a> { text:Option<&'a mut String>,remaining:usize,written:usize }
impl std::fmt::Write for ConsoleWriter<'_> {
    fn write_str(&mut self,text:&str)->std::fmt::Result {
        let mut end=text.len().min(self.remaining);
        while !text.is_char_boundary(end){end-=1;}
        if let Some(out)=&mut self.text {out.push_str(&text[..end]);}
        self.remaining-=end;self.written+=end;Ok(())
    }
}
impl Runtime {
    fn console_error_visible(&self,id:usize,host:&dyn super::Host)->bool {
        let Some(object)=self.objects.get(id) else{return false;};
        if object.error_name.is_none() || object.proxy.is_some(){return false;}
        let owner=self.window_realms.iter().find_map(|(&window,&realm)|(realm==object.realm).then_some(window));
        let caller=self.native_access_caller.unwrap_or_else(||self.browser_global.map(|(window,_)|window));
        match owner {
            Some(owner) if caller!=Some(owner)=>host.validate_window_access(caller,owner).is_ok(),
            Some(_)=>true,
            None=>object.realm==self.active_realm,
        }
    }
    fn console_arguments(&self,args:&[Value],out:&mut ConsoleWriter<'_>,host:&dyn super::Host) {
        for value in args {
            if out.remaining==0 {break;}
            if out.written!=0 {let _=out.write_char(' ');}
            match value {
                Value::String(text)=>{let _=out.write_str(text);},
                Value::BigInt(text)=>{let _=out.write_str(text);},
                Value::Object(id) if self.console_error_visible(*id,host)=>{
                    let object=&self.objects[*id];
                    let kind=object.error_name.unwrap();
                    if let Some(origin)=self.diagnostics.origins.iter().rev().find(|origin|origin.identity==*id) {
                        let _=write!(out,"{}: {} | Original native error at line {}, column {} | Original source: {}",origin.kind,origin.message,origin.location.line,origin.location.column,origin.location.context);
                    } else {
                        let _=out.write_str(kind);
                        if let Some(property)=object.properties.get("message") {
                            if property.getter.is_none()&&property.accessor.is_none() {
                                if let StoredValue::Value(Value::String(message))=&property.value {
                                    let _=out.write_str(": ");
                                    for ch in message.chars().take(512).flat_map(char::escape_default) {let _=out.write_char(ch);if out.remaining==0{break;}}
                                }
                            }
                        }
                    }
                },
                _=>{let _=out.write_str(&value.to_text());},
            }
        }
    }
    pub(super) fn console_message(&mut self,args:&[Value],limit:usize,host:&dyn super::Host)->Result<String,Fault> {
        self.work(args.len())?;
        let mut count=ConsoleWriter {text:None,remaining:limit,written:0};
        self.console_arguments(args,&mut count,host);
        // Count both bounded formatting passes and the destination copy.
        self.work(count.written.checked_mul(3).ok_or_else(||Fault::new("Console diagnostic copy work overflow."))?)?;
        let mut text=String::new();text.try_reserve_exact(count.written).map_err(|_|Fault::new("Console diagnostic reservation failed."))?;
        let mut out=ConsoleWriter {text:Some(&mut text),remaining:limit,written:0};
        self.console_arguments(args,&mut out,host);
        Ok(text)
    }
}
