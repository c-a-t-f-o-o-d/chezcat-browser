//! Bounded, read-only diagnostics. Never invoke page getters or conversions.
use super::{Exception, Fault, Runtime, StoredValue, Value};

#[derive(Default)]
pub(super) struct Data {
    caught: Vec<String>,
    omitted: usize,
}

fn bounded(text: &str) -> String {
    text.chars().take(512).flat_map(char::escape_default).take(2048).collect()
}

impl Runtime {
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
        let mut text = self.diagnostics.caught.concat();
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
