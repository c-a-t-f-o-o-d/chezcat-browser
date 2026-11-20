//! History uses the native session-entry authority; no interpreter identities enter App.
use super::*;
use crate::{
    navigation::Location,
    session_history::{self, Entry},
};
pub use session_history::Data as HistoryData;

impl HistoryData {
    fn value(&self) -> Result<Value, String> {
        Ok(match self {
            Self::Null => Value::Null,
            Self::Undefined => Value::Undefined,
            Self::Bool(value) => Value::Bool(*value),
            Self::Number(value) => Value::Number(*value),
            Self::String(value) => Value::String(script_clone_node::copy_text(value)?),
            Self::StringUnits(units) => {
                let mut next = Vec::new();
                next.try_reserve_exact(units.len())
                    .map_err(|_| "Could not reserve History state code units.")?;
                next.extend_from_slice(units);
                Value::StringUnits(next)
            }
            Self::BigInt(text) => Value::BigInt(text.clone()),
        })
    }
    fn bytes(&self) -> usize {
        match self {
            Self::String(value) => value.len(),
            Self::StringUnits(units) => units.len().saturating_mul(2),
            Self::BigInt(text) => text.len(),
            _ => 0,
        }
    }
}
#[derive(Default)]
pub(super) struct HistoryState {
    pub active: Option<session_history::Shared>,
    pub document: u64,
    pub document_base: bool,
    pub pending: bool,
}
impl Page {
    pub(crate) fn attach_session_history(
        &mut self,
        history: session_history::Shared,
        document: u64,
        document_base: bool,
    ) {
        self.state.history.active = Some(history);
        self.state.history.document = document;
        self.state.history.document_base = document_base;
    }
    pub fn take_history_change(&mut self) -> bool {
        std::mem::take(&mut self.state.history.pending)
    }
    #[cfg(test)]
    pub fn set_history_context(&mut self, data: HistoryData, document_base: bool, length: usize) {
        let mut history = session_history::Stack::new(session_history::BYTE_ALLOWANCE);
        let location = self
            .state
            .location
            .as_ref()
            .map_or_else(Location::home, |location| location.source.clone());
        for _ in 0..length {
            history
                .update(
                    Entry {
                        location: location.clone(),
                        scroll: 0,
                        request: crate::http::Request::Get,
                        document: 0,
                        data: data.clone(),
                    },
                    false,
                )
                .unwrap();
        }
        self.attach_session_history(
            session_history::Shared::from_stack(history),
            0,
            document_base,
        );
    }
}
impl Bridge<'_> {
    fn active_history(&self) -> Result<session_history::Shared, (&'static str, String)> {
        let shared = self.state.history.active.as_ref().ok_or((
            "SecurityError",
            "History requires a fully active document session entry.".into(),
        ))?;
        let history = shared.try_borrow().map_err(|_| {
            (
                "InvalidStateError",
                "History entry authority is busy.".into(),
            )
        })?;
        if history
            .current()
            .is_none_or(|entry| entry.document != self.state.history.document)
        {
            return Err((
                "SecurityError",
                "History requires a fully active document session entry.".into(),
            ));
        }
        Ok(shared.clone())
    }
    pub(super) fn validate_history_active(&self) -> Result<(), (&'static str, String)> {
        self.active_history().map(|_| ())
    }
    pub(super) fn history_target(
        &self,
        args: &[Value],
    ) -> Result<Location, (&'static str, String)> {
        self.active_history()?;
        let current = &self
            .state
            .location
            .as_ref()
            .ok_or((
                "SecurityError",
                "History has no active document URL.".into(),
            ))?
            .source;
        let source = match args.get(2) {
            None | Some(Value::Null | Value::Undefined) => return Ok(current.clone()),
            Some(Value::String(source)) if source.is_empty() => return Ok(current.clone()),
            Some(Value::String(source)) => source,
            _ => {
                return Err((
                    "TypeError",
                    "History URL requires nullable string conversion.".into(),
                ))
            }
        };
        let target =
            crate::navigation::resolve(source, self.state.dynamic.base.as_ref().unwrap_or(current))
                .map_err(|error| ("SecurityError", error))?;
        let allowed = match (&current.url, &target.url) {
            (Some(origin), Some(url)) => origin.same_origin(url),
            (None, None) => current.path == target.path && current.root == target.root,
            _ => false,
        };
        if !allowed {
            return Err((
                "SecurityError",
                "History URL must preserve the document origin and local-file scope.".into(),
            ));
        }
        Ok(target)
    }
    pub(super) fn history_get(&mut self, property: &str) -> Result<Value, String> {
        if matches!(property, "length" | "state") {
            let shared = self.active_history().map_err(|(_, error)| error)?;
            let history = shared
                .try_borrow()
                .map_err(|_| "History entry authority is busy.")?;
            return if property == "length" {
                Ok(Value::Number(history.len() as f64))
            } else {
                let data = &history.current().unwrap().data;
                self.state.copies(data.bytes())?;
                data.value()
            };
        }
        match property {
            "constructor" | "__proto__" | "scrollRestoration" | "go" | "back" | "forward" => {
                Err(format!("History property {property:?} is not implemented."))
            }
            _ => Ok(Value::Undefined),
        }
    }
    pub(super) fn history_replace(&mut self, args: &[Value]) -> Result<Value, String> {
        self.history_change(args, false)
    }
    pub(super) fn history_push(&mut self, args: &[Value]) -> Result<Value, String> {
        self.history_change(args, true)
    }
    fn history_change(&mut self, args: &[Value], push: bool) -> Result<Value, String> {
        let shared = self.active_history().map_err(|(_, error)| error)?;
        let data=match args.first(){Some(Value::Null)=>HistoryData::Null,Some(Value::Undefined)=>HistoryData::Undefined,Some(Value::Bool(value))=>HistoryData::Bool(*value),Some(Value::Number(value))=>HistoryData::Number(*value),Some(Value::String(value))=>{self.state.copies(value.len())?;HistoryData::String(script_clone_node::copy_text(value)?)},Some(Value::StringUnits(units))=>{self.state.copies(units.len().saturating_mul(2))?;let mut next=Vec::new();next.try_reserve_exact(units.len()).map_err(|_|"Could not reserve History state code units.")?;next.extend_from_slice(units);HistoryData::StringUnits(next)},Some(Value::BigInt(text))=>{self.state.copies(text.len())?;HistoryData::BigInt(text.clone())},_=>return Err("History object/native state serialization is not implemented; no session entry was changed.".into())};
        let target = self.history_target(args).map_err(|(_, error)| error)?;
        self.state.work(1)?;
        self.state
            .copies(target.display().len().saturating_mul(8))?;
        let location = LocationView::new(&target);
        let mut history = shared
            .try_borrow_mut()
            .map_err(|_| "History entry authority is busy.")?;
        let old = history
            .current()
            .ok_or("History has no active session entry.")?;
        let scroll = self
            .state
            .geometry
            .as_ref()
            .map_or(old.scroll, |geometry| geometry.device_scroll_y());
        self.state
            .copies(session_history::request_copy_work(&old.request)?)?;
        let entry = Entry {
            location: target.clone(),
            scroll,
            request: session_history::copy_request(&old.request)?,
            document: self.state.history.document,
            data,
        };
        let index = history.index();
        let change = history
            .prepare(entry, !push)?
            .preserve_scroll(index, scroll);
        self.state.copies(change.movement_bytes())?;
        history.commit(change)?;
        self.state.history.pending = true;
        self.state.location = Some(location);
        if self.state.history.document_base {
            self.state.dynamic.base = Some(target);
        }
        Ok(Value::Undefined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_owned_storage_and_unsupported_data_preserve_current_and_recover() {
        let mut doc = crate::html::parse("<html><body><p>History data fixture</p></body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let location =
            crate::navigation::address("https://example.test/page", &Location::home()).unwrap();
        let mut page = Page::new();
        page.set_location(&location);
        page.set_resource_base(&location);
        page.set_history_context(HistoryData::Null, true, 1);
        page.start(&mut doc, &mut forms, &[]);
        let shared = page.state.history.active.as_ref().unwrap().clone();
        let mut bridge = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        };
        bridge
            .history_replace(&[Value::Number(-0.0), Value::String(String::new())])
            .unwrap();
        let snapshot = shared.borrow().current().unwrap().location.clone();
        for value in [Value::Object(0), Value::Host(DOCUMENT)] {
            assert!(bridge
                .history_replace(&[
                    value,
                    Value::String(String::new()),
                    Value::String("/must-not-commit".into())
                ])
                .is_err());
            assert_eq!(shared.borrow().current().unwrap().location, snapshot);
            assert!(bridge.state.history.pending);
            assert_eq!(bridge.state.location.as_ref().unwrap().source, location);
            assert!(
                matches!(bridge.history_get("state").unwrap(),Value::Number(value)if value==0.0&&value.is_sign_negative())
            );
        }
        let large = "x".repeat(1024 * 1024 + 1);
        bridge
            .history_replace(&[Value::String(large.clone()), Value::String(String::new())])
            .unwrap();
        assert!(
            matches!(&shared.borrow().current().unwrap().data,HistoryData::String(value)if value==&large)
        );
        for _ in 0..5000 {
            bridge
                .history_replace(&[Value::Null, Value::String(String::new())])
                .unwrap();
        }
        assert_eq!(shared.borrow().len(), 1);
        let before = shared.borrow().storage_bytes();
        shared.borrow_mut().set_limit_for_test(before);
        assert!(bridge
            .history_replace(&[
                Value::String(large),
                Value::String(String::new()),
                Value::String("/quota".into())
            ])
            .is_err());
        assert_eq!(shared.borrow().storage_bytes(), before);
        assert_eq!(bridge.state.location.as_ref().unwrap().source, location);
        shared
            .borrow_mut()
            .set_limit_for_test(session_history::BYTE_ALLOWANCE);
        bridge
            .history_replace(&[Value::Number(-0.0), Value::String(String::new())])
            .unwrap();
        bridge.state.history.active = None;
        assert!(bridge.history_get("length").is_err());
        assert_eq!(
            bridge
                .validate_call(
                    HISTORY,
                    "replaceState",
                    &[Value::Function(0), Value::String(String::new())]
                )
                .unwrap_err()
                .0,
            "SecurityError"
        );
    }
    #[test]
    fn history_push_post_snapshot_copy_refusal_is_prepublication_and_recovers() {
        let mut doc = crate::html::parse("<html><body>POST snapshot</body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let location =
            crate::navigation::address("https://history-post.test/form", &Location::home())
                .unwrap();
        let mut page = Page::new();
        page.set_location(&location);
        page.set_resource_base(&location);
        page.set_history_context(HistoryData::Null, true, 1);
        assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
        let shared = page.state.history.active.as_ref().unwrap().clone();
        let mut entry = shared.borrow().current().unwrap().clone();
        entry.request = crate::http::Request::FormPost {
            body: vec![b'x'; 4096],
            origin: location.url.clone(),
        };
        shared.borrow_mut().update(entry, true).unwrap();
        let before = shared.borrow().storage_bytes();
        page.state.copy_work = 4095;
        assert!(Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state
        }
        .history_push(&[
            Value::Number(1.0),
            Value::String(String::new()),
            Value::String("/next".into())
        ])
        .is_err());
        assert_eq!(shared.len(), 1);
        assert_eq!(shared.borrow().storage_bytes(), before);
        assert_eq!(page.state.location.as_ref().unwrap().source, location);
        assert!(!page.state.history.pending);
        page.begin_job();
        Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        }
        .history_push(&[
            Value::Number(1.0),
            Value::String(String::new()),
            Value::String("/next".into()),
        ])
        .unwrap();
        assert_eq!(shared.len(), 2);
        assert!(
            matches!(&shared.borrow().current().unwrap().request,crate::http::Request::FormPost{body,..}if body==&vec![b'x';4096])
        );
    }
}
