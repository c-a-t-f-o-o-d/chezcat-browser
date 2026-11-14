//! Evidence-backed History state/replaceState bridge. Session data owns its
//! primitive payload; no realm-local object/function identities enter App.
use super::*;
use crate::navigation::Location;

#[derive(Clone, Debug, Default)]
pub enum HistoryData {
    #[default]
    Null,
    Undefined,
    Bool(bool),
    Number(f64),
    String(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_replace_state_quota_and_unsupported_data_preserve_current_and_pending_entry() {
        let mut doc = crate::html::parse("<html><body><p>History data fixture</p></body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let location = crate::navigation::address("https://example.test/page", &Location::home()).unwrap();
        let mut page = Page::for_test();
        page.set_location(&location);
        page.set_resource_base(&location);
        page.set_history_context(HistoryData::Null, true);
        page.start(&mut doc, &mut forms, &[]);
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        bridge.history_replace(&[Value::Number(-0.0), Value::String(String::new())]).unwrap();
        assert!(matches!(bridge.history_get("state").unwrap(), Value::Number(value) if value == 0.0 && value.is_sign_negative()));
        let pending = bridge.state.history.pending.as_ref().unwrap().location.clone();
        for value in [Value::String("x".repeat(1024 * 1024 + 1)), Value::Object(0), Value::Host(DOCUMENT)] {
            assert!(bridge.history_replace(&[value, Value::String(String::new()), Value::String("/must-not-commit".into())]).is_err());
            assert_eq!(bridge.state.history.pending.as_ref().unwrap().location, pending);
            assert_eq!(bridge.state.location.as_ref().unwrap().source, location);
            assert!(matches!(bridge.history_get("state").unwrap(), Value::Number(value) if value == 0.0 && value.is_sign_negative()));
        }
        bridge.state.history.updates = 4096;
        assert!(bridge.history_replace(&[Value::Null, Value::String(String::new()), Value::String("/quota".into())]).is_err());
        assert_eq!(bridge.state.history.pending.as_ref().unwrap().location, pending);
        assert_eq!(bridge.state.location.as_ref().unwrap().source, location);
        bridge.state.history.current = None;
        assert_eq!(bridge.validate_call(HISTORY, "replaceState", &[Value::Function(0), Value::String(String::new())]).unwrap_err().0, "SecurityError");
    }
}

impl HistoryData {
    fn value(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Undefined => Value::Undefined,
            Self::Bool(value) => Value::Bool(*value),
            Self::Number(value) => Value::Number(*value),
            Self::String(value) => Value::String(value.clone()),
        }
    }
    fn bytes(&self) -> usize { if let Self::String(value) = self { value.len() } else { 0 } }
}

#[derive(Clone)]
pub struct HistoryReplace {
    pub location: Location,
    pub data: HistoryData,
}

#[derive(Default)]
pub(super) struct HistoryState {
    pub current: Option<HistoryData>,
    pub pending: Option<HistoryReplace>,
    pub document_base: bool,
    updates: usize,
}

impl Page {
    pub fn set_history_context(&mut self, data: HistoryData, document_base: bool) {
        self.state.history.current = Some(data);
        self.state.history.document_base = document_base;
    }
    pub fn take_history_replacement(&mut self) -> Option<HistoryReplace> { self.state.history.pending.take() }
}

impl Bridge<'_> {
    pub(super) fn history_target(&self, args: &[Value]) -> Result<Location, (&'static str, String)> {
        let current = self.state.location.as_ref().map(|location| &location.source)
            .filter(|_| self.state.history.current.is_some())
            .ok_or(("SecurityError", "History requires a fully active document session entry.".into()))?;
        let source = match args.get(2) {
            None | Some(Value::Null | Value::Undefined) => return Ok(current.clone()),
            Some(Value::String(source)) if source.is_empty() => return Ok(current.clone()),
            Some(Value::String(source)) => source,
            _ => return Err(("TypeError", "History URL requires nullable string conversion.".into())),
        };
        let target = crate::navigation::resolve(source, self.state.dynamic.base.as_ref().unwrap_or(current))
            .map_err(|error| ("SecurityError", error))?;
        let allowed = match (&current.url, &target.url) {
            (Some(origin), Some(url)) => origin.same_origin(url),
            (None, None) => current.path == target.path && current.root == target.root,
            _ => false,
        };
        if !allowed { return Err(("SecurityError", "History URL must preserve the document origin and local-file scope.".into())); }
        Ok(target)
    }

    pub(super) fn history_get(&mut self, property: &str) -> Result<Value, String> {
        match property {
            "state" => {
                let data = self.state.history.current.as_ref().ok_or("History has no active App session entry.")?;
                self.state.copies(data.bytes())?;
                Ok(self.state.history.current.as_ref().unwrap().value())
            }
            "constructor" | "__proto__" | "length" | "scrollRestoration" | "go" | "back" | "forward" | "pushState" => Err(format!("History property {property:?} is not implemented.")),
            _ => Ok(Value::Undefined),
        }
    }

    pub(super) fn history_replace(&mut self, args: &[Value]) -> Result<Value, String> {
        // Validation has already returned named URL/security errors through
        // Host::validate_call after Web IDL scalar conversions.
        let data = match args.first() {
            Some(Value::Null) => HistoryData::Null,
            Some(Value::Undefined) => HistoryData::Undefined,
            Some(Value::Bool(value)) => HistoryData::Bool(*value),
            Some(Value::Number(value)) => HistoryData::Number(*value),
            Some(Value::String(value)) => {
                if value.len() > 1024 * 1024 { return Err("Primitive History state exceeds the 1 MiB session-entry payload limit.".into()); }
                self.state.copies(value.len().saturating_mul(2))?;
                HistoryData::String(value.clone())
            }
            _ => return Err("History object/native state serialization is not implemented; no session entry was changed.".into()),
        };
        let target = self.history_target(args).map_err(|(_, error)| error)?;
        if self.state.history.updates >= 4096 { return Err("History replacement work limit reached (4096 per document).".into()); }
        self.state.work(1)?;
        self.state.copies(target.display().len().saturating_mul(8))?;
        let location = LocationView::new(&target);
        let pending = HistoryReplace { location: target.clone(), data: data.clone() };
        self.state.history.current = Some(data);
        self.state.history.pending = Some(pending);
        self.state.history.updates += 1;
        self.state.location = Some(location);
        if self.state.history.document_base { self.state.dynamic.base = Some(target); }
        Ok(Value::Undefined)
    }
}
