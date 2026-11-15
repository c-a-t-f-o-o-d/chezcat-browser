//! Bounded, read-only diagnostics. Never invoke page getters or conversions.
use super::{Exception, Fault, Runtime, StoredValue, Value};

#[derive(Default)]
pub(super) struct Data {
    caught: Vec<String>,
    omitted: usize,
    entries: [Entries; 2],
    last_fault: Option<(&'static str, &'static str)>,
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
        if let Some(exception) = &fault.exception {
            if let Exception::Value(value @ Value::Object(id)) = &**exception {
                if self.objects.get(*id).is_some_and(|object| object.error_name.is_some()) {
                    fault.message = format!("Uncaught exception: {}", self.diagnostic_value(value));
                }
            }
        }
        fault.display()
    }
}
