//! Genuine XMLHttpRequest state and original document-owned request snapshots.
use super::*;
use crate::javascript::xhr_interface::{Call, Error, Notification, Progress};
use crate::script_request_core as core;
use crate::script_storage::Lease;
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
#[path = "script_xhr_mime.rs"]
mod mime;
const BASE: u32 = 0x8e00_0000;
const UPLOAD: u32 = 0x8f00_0000;
pub(super) struct Received {
    pub response: crate::http::Response,
    pub text: String,
    pub received_bytes: u64,
    pub _storage: Lease,
}
pub(super) struct Record {
    pub owner: u32,
    pub generation: u64,
    pub ready: u8,
    pub sent: bool,
    pub url: Option<crate::url::HttpUrl>,
    pub outgoing: crate::http::ScriptRequest,
    pub timeout: u32,
    pub credentials: bool,
    pub response_type: String,
    pub mime: String,
    pub request: Option<Arc<core::Request>>,
    pub response: Option<Received>,
    pub upload_active: bool,
}
pub(super) struct State {
    pub records: Vec<Record>,
    pub pending: VecDeque<Arc<core::Request>>,
    pub used: Arc<AtomicUsize>,
    pending_storage: Lease,
}
impl Default for State {
    fn default() -> Self {
        let used = Arc::new(AtomicUsize::new(0));
        Self {
            records: Vec::new(),
            pending: VecDeque::new(),
            used: used.clone(),
            pending_storage: Lease::empty(used),
        }
    }
}
fn exception(kind: &'static str, message: impl Into<String>) -> Error {
    Error::Exception(kind, message.into())
}
fn copy_text(text: &str) -> Result<String, String> {
    let mut next = String::new();
    next.try_reserve_exact(text.len())
        .map_err(|_| "Could not reserve request text.")?;
    next.push_str(text);
    Ok(next)
}
fn url_bytes(url: &crate::url::HttpUrl) -> usize {
    url.host.capacity() + url.path.capacity() + url.query.as_ref().map_or(0, String::capacity)
}
fn zero() -> Progress {
    Progress {
        length_computable: false,
        loaded: 0,
        total: 0,
    }
}
impl Bridge<'_> {
    pub(super) fn xhr_index(&self, object: u32) -> Option<(usize, bool)> {
        let (id, upload) = if (BASE..UPLOAD).contains(&object) {
            (object - BASE, false)
        } else if (UPLOAD..0x9000_0000).contains(&object) {
            (object - UPLOAD, true)
        } else {
            return None;
        };
        ((id as usize) < self.state.xhr.records.len()).then_some((id as usize, upload))
    }
    pub(super) fn xhr_owner(&self, object: u32) -> Option<u32> {
        self.xhr_index(object)
            .map(|(id, _)| self.state.xhr.records[id].owner)
    }
    pub(super) fn xhr_interface(&self, object: u32) -> Option<(u32, &'static str)> {
        self.xhr_index(object).map(|(id, upload)| {
            (
                self.state.xhr.records[id].owner,
                if upload {
                    "XMLHttpRequestUpload"
                } else {
                    "XMLHttpRequest"
                },
            )
        })
    }
    pub(super) fn xhr_handler_event<'a>(&self, object: u32, property: &'a str) -> Option<&'a str> {
        let (_, upload) = self.xhr_index(object)?;
        let event = property.strip_prefix("on")?;
        (matches!(
            event,
            "loadstart" | "progress" | "abort" | "error" | "load" | "timeout" | "loadend"
        ) || !upload && event == "readystatechange")
            .then_some(event)
    }
    pub(super) fn xhr_member(&self, object: u32, name: &str) -> bool {
        let Some((_, upload)) = self.xhr_index(object) else {
            return false;
        };
        script_element_properties::ordinary_interface_member(name)
            || matches!(
                name,
                "addEventListener" | "removeEventListener" | "dispatchEvent"
            )
            || self.xhr_handler_event(object, name).is_some()
            || !upload
                && matches!(
                    name,
                    "readyState"
                        | "timeout"
                        | "withCredentials"
                        | "upload"
                        | "responseURL"
                        | "status"
                        | "statusText"
                        | "responseType"
                        | "response"
                        | "responseText"
                        | "responseXML"
                        | "UNSENT"
                        | "OPENED"
                        | "HEADERS_RECEIVED"
                        | "LOADING"
                        | "DONE"
                        | "open"
                        | "send"
                        | "abort"
                        | "setRequestHeader"
                        | "getResponseHeader"
                        | "getAllResponseHeaders"
                        | "overrideMimeType"
                )
    }
    pub(super) fn xhr_get(&mut self, object: u32, name: &str) -> Option<Result<Value, String>> {
        let (id, upload) = self.xhr_index(object)?;
        Some((|| {
            self.state.work(1)?;
            if self.xhr_handler_event(object, name).is_some() {
                return self
                    .handler(object, name)
                    .map(|value| value.unwrap_or(Value::Null));
            }
            if upload {
                return Ok(Value::Undefined);
            }
            let record = &self.state.xhr.records[id];
            let number = match name {
                "UNSENT" => Some(0),
                "OPENED" => Some(1),
                "HEADERS_RECEIVED" => Some(2),
                "LOADING" => Some(3),
                "DONE" => Some(4),
                "readyState" => Some(u32::from(record.ready)),
                "timeout" => Some(record.timeout),
                "status" => Some(
                    record
                        .response
                        .as_ref()
                        .filter(|_| record.ready >= 2)
                        .map_or(0, |received| u32::from(received.response.status)),
                ),
                _ => None,
            };
            if let Some(number) = number {
                return Ok(Value::Number(f64::from(number)));
            }
            if name == "withCredentials" {
                return Ok(Value::Bool(record.credentials));
            }
            if name == "upload" {
                return Ok(Value::Host(UPLOAD + id as u32));
            }
            if name == "responseXML" {
                return Ok(Value::Null);
            }
            if name == "responseURL" {
                let text = record
                    .response
                    .as_ref()
                    .filter(|_| record.ready >= 2)
                    .map(|received| received.response.url.display())
                    .unwrap_or_default();
                self.state.copies(text.len())?;
                return Ok(Value::String(text));
            }
            let text = match name {
                "responseType" => record.response_type.as_str(),
                "responseText" => record
                    .response
                    .as_ref()
                    .filter(|_| record.ready >= 3)
                    .map_or("", |received| received.text.as_str()),
                "response" => record
                    .response
                    .as_ref()
                    .filter(|_| record.ready >= 3)
                    .map_or("", |received| received.text.as_str()),
                "statusText" => record
                    .response
                    .as_ref()
                    .filter(|_| record.ready >= 2)
                    .map_or("", |received| received.response.status_text.as_str()),
                _ => return Ok(Value::Undefined),
            };
            let bytes = text.len();
            self.state.copies(bytes)?;
            let record = &self.state.xhr.records[id];
            let text = match name {
                "responseType" => record.response_type.as_str(),
                "responseText" => record
                    .response
                    .as_ref()
                    .filter(|_| record.ready >= 3)
                    .map_or("", |r| r.text.as_str()),
                "response" => record
                    .response
                    .as_ref()
                    .filter(|_| record.ready >= 3)
                    .map_or("", |r| r.text.as_str()),
                "statusText" => record
                    .response
                    .as_ref()
                    .filter(|_| record.ready >= 2)
                    .map_or("", |r| r.response.status_text.as_str()),
                _ => unreachable!(),
            };
            Ok(Value::String(copy_text(text)?))
        })())
    }
    pub(super) fn xhr_new(&mut self, owner: u32) -> Result<Value, String> {
        if self.event_target_count_refused(2) {
            return Err("XMLHttpRequest/EventTarget allowance exhausted.".into());
        }
        let id = u32::try_from(self.state.xhr.records.len())
            .ok()
            .filter(|id| *id < UPLOAD - BASE)
            .ok_or("XMLHttpRequest identity space exhausted.")?;
        let (replacement, added) = retained_storage::grow(
            &self.state.xhr.records,
            1,
            self.state
                .retained_limit
                .saturating_sub(self.state.retained),
        )?;
        self.state.work(1)?;
        if replacement.is_some() {
            self.state
                .copies(self.state.xhr.records.len() * std::mem::size_of::<Record>())?;
        }
        if let Some(mut next) = replacement {
            next.append(&mut self.state.xhr.records);
            self.state.xhr.records = next;
        }
        self.state.retained += added;
        self.state.xhr.records.push(Record {
            owner,
            generation: 0,
            ready: 0,
            sent: false,
            url: None,
            outgoing: crate::http::ScriptRequest {
                method: crate::http::ScriptMethod::Get,
                headers: Vec::new(),
                body: Vec::new(),
            },
            timeout: 0,
            credentials: false,
            response_type: String::new(),
            mime: String::new(),
            request: None,
            response: None,
            upload_active: false,
        });
        Ok(Value::Host(BASE + id))
    }
    pub(super) fn xhr_validate_member(
        &self,
        object: u32,
        name: &str,
    ) -> Result<(), (&'static str, String)> {
        let Some((id, upload)) = self.xhr_index(object) else {
            return Err((
                "TypeError",
                "XMLHttpRequest requires a genuine receiver.".into(),
            ));
        };
        if upload && self.xhr_handler_event(object, name).is_none() {
            return Err((
                "TypeError",
                "This operation requires XMLHttpRequest, not its upload target.".into(),
            ));
        }
        let record = &self.state.xhr.records[id];
        if name == "responseXML" && record.response_type == "text" {
            return Err((
                "InvalidStateError",
                "responseXML is unavailable for responseType text.".into(),
            ));
        }
        if name == "responseXML"
            && record.ready == 4
            && record.response.as_ref().is_some_and(|received| {
                let mime = if record.mime.is_empty() {
                    received.response.header("content-type").unwrap_or("")
                } else {
                    &record.mime
                };
                mime.split(';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase()
                    .ends_with("xml")
            })
        {
            return Err((
                "NotSupportedError",
                "XHR XML document parsing is not implemented.".into(),
            ));
        }
        Ok(())
    }
    pub(super) fn xhr_validate_set(
        &self,
        object: u32,
        name: &str,
        value: &Value,
    ) -> Result<(), (&'static str, String)> {
        self.xhr_validate_member(object, name)?;
        let (id, _) = self.xhr_index(object).unwrap();
        let record = &self.state.xhr.records[id];
        if self.xhr_handler_event(object, name).is_some() {
            return if matches!(value, Value::Function(_) | Value::Null | Value::Undefined) {
                Ok(())
            } else {
                Err((
                    "TypeError",
                    "Request handler attributes require a function or null.".into(),
                ))
            };
        }
        if name == "withCredentials" && (record.ready > 1 || record.sent) {
            return Err((
                "InvalidStateError",
                "withCredentials must be set before send.".into(),
            ));
        }
        if name == "timeout" && record.sent {
            return Err((
                "NotSupportedError",
                "Changing an in-flight request timeout is not implemented.".into(),
            ));
        }
        if name == "responseType" {
            if record.ready >= 3 {
                return Err((
                    "InvalidStateError",
                    "responseType cannot change during loading or after completion.".into(),
                ));
            }
            let Value::String(kind) = value else {
                return Err((
                    "TypeError",
                    "responseType requires a converted string.".into(),
                ));
            };
            if !matches!(
                kind.as_str(),
                "" | "text" | "json" | "document" | "arraybuffer" | "blob"
            ) {
                return Err((
                    "TypeError",
                    "Invalid XMLHttpRequestResponseType value.".into(),
                ));
            }
            if !matches!(kind.as_str(), "" | "text") {
                return Err((
                    "NotSupportedError",
                    "This response type has no implemented request adapter yet.".into(),
                ));
            }
        }
        Ok(())
    }
    pub(super) fn xhr_set(
        &mut self,
        object: u32,
        name: &str,
        value: Value,
    ) -> Option<Result<(), String>> {
        let (id, _) = self.xhr_index(object)?;
        Some((|| {
            if self.xhr_handler_event(object, name).is_some() {
                return self.set_handler(object, name, value).map(|_| ());
            }
            match name {
                "timeout" => {
                    let timeout = timer_integer(&value)? as u32;
                    self.state.xhr.records[id].timeout = timeout;
                }
                "withCredentials" => self.state.xhr.records[id].credentials = value.truthy(),
                "responseType" => {
                    let Value::String(kind) = value else {
                        return Err("responseType requires converted text.".into());
                    };
                    let kind = copy_text(&kind)?;
                    let old = self.state.xhr.records[id].response_type.capacity();
                    self.state
                        .replace_storage(old, kind.capacity(), kind.len())?;
                    self.state.xhr.records[id].response_type = kind;
                }
                _ => return Err("This XMLHttpRequest property has no native setter.".into()),
            }
            Ok(())
        })())
    }
    fn xhr_event(
        &mut self,
        target: u32,
        name: &'static str,
        progress: Option<Progress>,
    ) -> Result<usize, String> {
        let owner = self
            .xhr_owner(target)
            .ok_or("Native request event target is invalid.")?;
        let args = [Value::String(name.into())];
        let value = if let Some(data) = progress {
            self.construct_progress_event(
                owner,
                &args,
                Some(&[
                    ("lengthComputable", Value::Bool(data.length_computable)),
                    ("loaded", Value::Number(data.loaded as f64)),
                    ("total", Value::Number(data.total as f64)),
                ]),
            )?
        } else {
            self.construct_base_event(owner, &args, None)?
        };
        let Value::Host(event) = value else {
            unreachable!()
        };
        let token = match self.begin_synthetic_dispatch(target, &[Value::Host(event)]) {
            Ok(token) => token,
            Err(error) => {
                self.rollback_base_event(event)?;
                return Err(error);
            }
        };
        let id = self.created_event_index(event).unwrap();
        self.state.created_events[id].trusted = true;
        Ok(token)
    }
    pub(super) fn xhr_notification(
        &mut self,
        notification: Notification,
    ) -> Result<Option<usize>, String> {
        let Some((id, _)) = self.xhr_index(notification.target) else {
            return Ok(None);
        };
        if self.state.xhr.records[id].generation != notification.generation {
            return Ok(None);
        }
        self.xhr_event(
            notification.target,
            notification.event,
            notification.progress,
        )
        .map(Some)
    }
    pub(super) fn xhr_finish_abort(&mut self, target: u32, generation: u64) {
        if let Some((id, _)) = self.xhr_index(target) {
            let record = &mut self.state.xhr.records[id];
            if record.generation == generation && record.ready == 4 {
                record.ready = 0;
            }
        }
    }
    pub(super) fn xhr_call(
        &mut self,
        object: u32,
        method: &str,
        args: &[Value],
        binary: Option<Vec<u8>>,
    ) -> Result<Call, Error> {
        let (id, upload) = self
            .xhr_index(object)
            .ok_or_else(|| exception("TypeError", "XMLHttpRequest requires a genuine receiver."))?;
        if upload {
            return Err(exception(
                "TypeError",
                "Upload targets have no request operations.",
            ));
        }
        let operation = crate::javascript::xhr_interface::operation(method)
            .ok_or_else(|| exception("TypeError", "Unknown native request operation."))?;
        self.state.work(1)?;
        match operation {
            "open" => {
                let Some(Value::String(name)) = args.first() else {
                    return Err(exception("TypeError", "open requires a converted method."));
                };
                let method = crate::http::ScriptMethod::parse(name)
                    .map_err(|(kind, message)| exception(kind, message))?;
                let Some(Value::String(reference)) = args.get(1) else {
                    return Err(exception("TypeError", "open requires a converted URL."));
                };
                if args.get(2).is_some_and(|value| !value.truthy()) {
                    return Err(exception(
                        "NotSupportedError",
                        "Synchronous XHR is not implemented; use the asynchronous request path.",
                    ));
                }
                if args
                    .iter()
                    .skip(3)
                    .take(2)
                    .any(|value| !matches!(value, Value::Null | Value::Undefined))
                {
                    return Err(exception(
                        "NotSupportedError",
                        "Explicit XHR username/password authentication is not implemented.",
                    ));
                }
                let origin = self
                    .state
                    .location
                    .as_ref()
                    .map(|view| &view.source)
                    .ok_or_else(|| {
                        exception("InvalidStateError", "No document location is installed.")
                    })?;
                if origin.url.is_none() {
                    return Err(exception(
                        "NotSupportedError",
                        "Opaque/local document script networking is not implemented.",
                    ));
                }
                let target = crate::navigation::resolve(
                    reference,
                    self.state.dynamic.base.as_ref().unwrap_or(origin),
                )
                .map_err(|error| exception("SyntaxError", error))?;
                let url = target.url.ok_or_else(|| {
                    exception("NotSupportedError", "XHR requires an HTTP or HTTPS URL.")
                })?;
                let generation = self.state.xhr.records[id]
                    .generation
                    .checked_add(1)
                    .ok_or_else(|| {
                        exception(
                            "QuotaExceededError",
                            "Request generation identity space exhausted.",
                        )
                    })?;
                let old = self.state.xhr.records[id].url.as_ref().map_or(0, url_bytes)
                    + self.state.xhr.records[id].outgoing.payload_bytes();
                let new = url_bytes(&url);
                self.state.copies(new)?;
                let retained = self
                    .state
                    .retained
                    .checked_sub(old)
                    .and_then(|bytes| bytes.checked_add(new))
                    .filter(|bytes| *bytes <= self.state.retained_limit)
                    .ok_or_else(|| Error::Fatal("XHR URL storage allowance exhausted.".into()))?;
                let fire = self.state.xhr.records[id].ready != 1;
                self.state.retained = retained;
                let token = if fire {
                    match self.xhr_event(object, "readystatechange", None) {
                        Ok(token) => Some(token),
                        Err(error) => {
                            self.state.retained = self.state.retained - new + old;
                            return Err(error.into());
                        }
                    }
                } else {
                    None
                };
                let record = &mut self.state.xhr.records[id];
                if let Some(request) = record.request.take() {
                    request.abort();
                }
                record.response = None;
                record.url = Some(url);
                record.outgoing = crate::http::ScriptRequest {
                    method,
                    headers: Vec::new(),
                    body: Vec::new(),
                };
                record.generation = generation;
                record.ready = 1;
                record.sent = false;
                record.upload_active = false;
                let mut call = Call::value(Value::Undefined);
                if fire {
                    call.notifications[0] = Some(Notification {
                        target: object,
                        generation,
                        event: "readystatechange",
                        progress: None,
                    });
                }
                call.initial_token = token;
                Ok(call)
            }
            "setRequestHeader" => {
                let record = &self.state.xhr.records[id];
                if record.ready != 1 || record.sent {
                    return Err(exception(
                        "InvalidStateError",
                        "Request headers require an opened, unsent request.",
                    ));
                }
                let (Some(Value::String(name)), Some(Value::String(value))) =
                    (args.first(), args.get(1))
                else {
                    return Err(exception(
                        "TypeError",
                        "setRequestHeader requires converted ByteStrings.",
                    ));
                };
                let value = value.trim_matches([' ', '\t']);
                if !crate::http::script_request::token(name)
                    || value.bytes().any(|byte| matches!(byte, 0 | b'\r' | b'\n'))
                {
                    return Err(exception(
                        "SyntaxError",
                        "Invalid HTTP request header name/value.",
                    ));
                }
                if crate::http::script_request::forbidden_header(name) {
                    return Ok(Call::value(Value::Undefined));
                }
                if !value.is_ascii() {
                    return Err(exception(
                        "NotSupportedError",
                        "Non-ASCII request header wire values are not implemented.",
                    ));
                }
                crate::http::script_request::validate_header(name, value)
                    .map_err(|message| exception("SyntaxError", message))?;
                let old = record.outgoing.payload_bytes();
                let mut next = record.outgoing.try_copy()?;
                if let Some((_, previous)) = next
                    .headers
                    .iter_mut()
                    .find(|(key, _)| key.eq_ignore_ascii_case(name))
                {
                    previous
                        .try_reserve(2 + value.len())
                        .map_err(|_| "Could not combine request headers.".to_string())?;
                    previous.push_str(", ");
                    previous.push_str(value);
                } else {
                    next.headers
                        .try_reserve(1)
                        .map_err(|_| "Could not reserve request header entry.".to_string())?;
                    next.headers.push((copy_text(name)?, copy_text(value)?));
                }
                let bytes = next.payload_bytes();
                self.state.replace_storage(old, bytes, bytes)?;
                self.state.xhr.records[id].outgoing = next;
                Ok(Call::value(Value::Undefined))
            }
            "send" => {
                let record = &self.state.xhr.records[id];
                if record.ready != 1 || record.sent {
                    return Err(exception(
                        "InvalidStateError",
                        "send requires an opened, unsent request.",
                    ));
                }
                if record.owner != WINDOW {
                    return Err(exception(
                        "NotSupportedError",
                        "Child-window XHR dispatch is not implemented.",
                    ));
                }
                let origin = self
                    .state
                    .location
                    .as_ref()
                    .and_then(|view| view.source.url.as_ref())
                    .ok_or_else(|| {
                        exception("NotSupportedError", "XHR requires a web document origin.")
                    })?;
                let mut outgoing = record.outgoing.try_copy()?;
                let ignored = matches!(
                    outgoing.method,
                    crate::http::ScriptMethod::Get | crate::http::ScriptMethod::Head
                );
                let has_body = !ignored
                    && (binary.is_some()
                        || args
                            .first()
                            .is_some_and(|value| !matches!(value, Value::Null | Value::Undefined)));
                let text_body = binary.is_none() && has_body;
                if !ignored {
                    outgoing.body = match binary {
                        Some(bytes) => bytes,
                        None => match args.first() {
                            Some(Value::String(text)) => {
                                let mut body = Vec::new();
                                body.try_reserve_exact(text.len())
                                    .map_err(|_| "Could not reserve request body.".to_string())?;
                                body.extend_from_slice(text.as_bytes());
                                body
                            }
                            None | Some(Value::Null | Value::Undefined) => Vec::new(),
                            _ => {
                                return Err(exception(
                                    "NotSupportedError",
                                    "This request body has no implemented adapter.",
                                ))
                            }
                        },
                    };
                }
                if text_body
                    && !outgoing
                        .headers
                        .iter()
                        .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
                {
                    outgoing
                        .headers
                        .try_reserve(1)
                        .map_err(|_| "Could not reserve body content type.".to_string())?;
                    outgoing
                        .headers
                        .push(("Content-Type".into(), "text/plain;charset=UTF-8".into()));
                }
                let policy = self.state.dynamic.policy.clone();
                let target = record
                    .url
                    .as_ref()
                    .ok_or_else(|| {
                        exception("InvalidStateError", "Opened request URL is missing.")
                    })?
                    .clone();
                let origin = origin.clone();
                let generation = record.generation;
                let timeout = record.timeout;
                let credentials = if record.credentials {
                    crate::http::ScriptCredentials::Include
                } else {
                    crate::http::ScriptCredentials::SameOrigin
                };
                if text_body {
                    if let Some((_, value)) = outgoing
                        .headers
                        .iter_mut()
                        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
                    {
                        self.state.work(value.len())?;
                        if let Some(mut parsed) = mime::parse(value)? {
                            if parsed.force_utf8() {
                                *value = parsed.serialize()?;
                            }
                        }
                    }
                }
                let copied = outgoing.payload_bytes()
                    + policy.owned_bytes()
                    + url_bytes(&target)
                    + url_bytes(&origin);
                self.state.copies(copied)?;
                let limit = self.state.limits.capacity.script_storage_bytes;
                let mut request = core::Request::prepare(
                    core::Id {
                        receiver: object,
                        generation,
                    },
                    target,
                    origin,
                    outgoing,
                    credentials,
                    policy,
                    timeout,
                    self.state.limits.host.string_bytes.min(limit),
                    self.state.xhr.used.clone(),
                    limit,
                )?;
                let upload_listeners = self
                    .state
                    .listeners
                    .iter()
                    .any(|listener| listener.target == UPLOAD + id as u32);
                Arc::get_mut(&mut request).unwrap().force_preflight = upload_listeners;
                let growth = self.prepare_xhr_pending()?;
                let token = self.xhr_event(object, "loadstart", Some(zero()))?;
                if let Some((mut next, storage)) = growth {
                    next.append(&mut self.state.xhr.pending);
                    self.state.xhr.pending = next;
                    self.state.xhr.pending_storage.merge(storage);
                }
                self.state.xhr.pending.push_back(request.clone());
                let record = &mut self.state.xhr.records[id];
                record.sent = true;
                record.request = Some(request);
                record.upload_active = has_body && upload_listeners;
                let mut call = Call::value(Value::Undefined);
                call.notifications[0] = Some(Notification {
                    target: object,
                    generation,
                    event: "loadstart",
                    progress: Some(zero()),
                });
                call.initial_token = Some(token);
                if has_body && upload_listeners {
                    call.notifications[1] = Some(Notification {
                        target: UPLOAD + id as u32,
                        generation,
                        event: "loadstart",
                        progress: Some(Progress {
                            length_computable: record.request.as_ref().unwrap().outgoing.body.len()
                                != 0,
                            loaded: 0,
                            total: record.request.as_ref().unwrap().outgoing.body.len() as u64,
                        }),
                    });
                }
                Ok(call)
            }
            "abort" => {
                let record = &self.state.xhr.records[id];
                let active = record.sent && matches!(record.ready, 1..=3);
                if !active {
                    let record = &mut self.state.xhr.records[id];
                    if record.ready == 4 {
                        record.ready = 0;
                        record.response = None;
                    }
                    return Ok(Call::value(Value::Undefined));
                }
                let generation = record.generation.checked_add(1).ok_or_else(|| {
                    exception(
                        "QuotaExceededError",
                        "Request generation identity space exhausted.",
                    )
                })?;
                let upload = record.upload_active;
                let token = self.xhr_event(object, "readystatechange", None)?;
                let record = &mut self.state.xhr.records[id];
                if let Some(request) = record.request.take() {
                    request.abort();
                }
                record.response = None;
                record.sent = false;
                record.ready = 4;
                record.generation = generation;
                record.upload_active = false;
                let mut call = Call::value(Value::Undefined);
                call.initial_token = Some(token);
                call.finish = Some((object, generation));
                call.notifications[0] = Some(Notification {
                    target: object,
                    generation,
                    event: "readystatechange",
                    progress: None,
                });
                let mut at = 1;
                if upload {
                    for event in ["abort", "loadend"] {
                        call.notifications[at] = Some(Notification {
                            target: UPLOAD + id as u32,
                            generation,
                            event,
                            progress: Some(zero()),
                        });
                        at += 1;
                    }
                }
                for event in ["abort", "loadend"] {
                    call.notifications[at] = Some(Notification {
                        target: object,
                        generation,
                        event,
                        progress: Some(zero()),
                    });
                    at += 1;
                }
                Ok(call)
            }
            "getResponseHeader" | "getAllResponseHeaders" => {
                let record = &self.state.xhr.records[id];
                let Some(received) = record.response.as_ref().filter(|_| record.ready >= 2) else {
                    return Ok(Call::value(if operation == "getResponseHeader" {
                        Value::Null
                    } else {
                        Value::String(String::new())
                    }));
                };
                self.state.work(received.response.headers.len())?;
                if operation == "getResponseHeader" {
                    let Some(Value::String(name)) = args.first() else {
                        return Err(exception(
                            "TypeError",
                            "getResponseHeader requires a converted ByteString.",
                        ));
                    };
                    self.state.work(name.len())?;
                    if !crate::http::script_request::token(name) {
                        return Ok(Call::value(Value::Null));
                    }
                    let headers = &self.state.xhr.records[id]
                        .response
                        .as_ref()
                        .unwrap()
                        .response
                        .headers;
                    let copied: usize = headers
                        .iter()
                        .filter(|(key, _)| key.eq_ignore_ascii_case(name))
                        .map(|(_, value)| value.len() + 2)
                        .sum();
                    let scan: usize = headers.iter().map(|(key, _)| key.len()).sum();
                    self.state.work(scan)?;
                    self.state.copies(copied)?;
                    let mut text = String::new();
                    text.try_reserve_exact(copied)
                        .map_err(|_| "Could not reserve response header value.".to_string())?;
                    let mut found = false;
                    for (key, value) in &self.state.xhr.records[id]
                        .response
                        .as_ref()
                        .unwrap()
                        .response
                        .headers
                    {
                        if key.eq_ignore_ascii_case(name) {
                            if found {
                                text.push_str(", ");
                            }
                            text.push_str(value);
                            found = true;
                        }
                    }
                    return Ok(Call::value(if found {
                        Value::String(text)
                    } else {
                        Value::Null
                    }));
                }
                let copied: usize = self.state.xhr.records[id]
                    .response
                    .as_ref()
                    .unwrap()
                    .response
                    .headers
                    .iter()
                    .map(|(name, value)| name.len() + value.len() + 4)
                    .sum();
                self.state.copies(
                    copied.checked_mul(3).ok_or_else(|| {
                        Error::Fatal("Response header copy work overflow.".into())
                    })?,
                )?;
                let mut values = std::collections::BTreeMap::<String, String>::new();
                for (name, value) in &self.state.xhr.records[id]
                    .response
                    .as_ref()
                    .unwrap()
                    .response
                    .headers
                {
                    if let Some(previous) = values.get_mut(name) {
                        previous
                            .try_reserve(2 + value.len())
                            .map_err(|_| "Could not combine response header values.".to_string())?;
                        previous.push_str(", ");
                        previous.push_str(value);
                    } else {
                        values.insert(copy_text(name)?, copy_text(value)?);
                    }
                }
                let result = if operation == "getResponseHeader" {
                    let Some(Value::String(name)) = args.first() else {
                        return Err(exception(
                            "TypeError",
                            "getResponseHeader requires a converted ByteString.",
                        ));
                    };
                    if !crate::http::script_request::token(name) {
                        Value::Null
                    } else {
                        values
                            .remove(&name.to_ascii_lowercase())
                            .map_or(Value::Null, Value::String)
                    }
                } else {
                    let size = values
                        .iter()
                        .map(|(name, value)| name.len() + value.len() + 4)
                        .sum();
                    let mut text = String::new();
                    text.try_reserve_exact(size)
                        .map_err(|_| "Could not reserve response header text.".to_string())?;
                    for (name, value) in values {
                        text.push_str(&name);
                        text.push_str(": ");
                        text.push_str(&value);
                        text.push_str("\r\n");
                    }
                    Value::String(text)
                };
                self.state.copies(result.bytes())?;
                Ok(Call::value(result))
            }
            "overrideMimeType" => {
                if self.state.xhr.records[id].ready >= 3 {
                    return Err(exception(
                        "InvalidStateError",
                        "MIME override cannot change during loading or after completion.",
                    ));
                }
                let Some(Value::String(mime)) = args.first() else {
                    return Err(exception(
                        "TypeError",
                        "overrideMimeType requires converted text.",
                    ));
                };
                self.state.work(mime.len())?;
                let mime = match mime::parse(mime)? {
                    Some(mime) => mime.serialize()?,
                    None => "application/octet-stream".into(),
                };
                let old = self.state.xhr.records[id].mime.capacity();
                self.state
                    .replace_storage(old, mime.capacity(), mime.len())?;
                self.state.xhr.records[id].mime = mime;
                Ok(Call::value(Value::Undefined))
            }
            _ => Err(exception("TypeError", "Unknown XMLHttpRequest operation.")),
        }
    }
    fn prepare_xhr_pending(
        &mut self,
    ) -> Result<Option<(VecDeque<Arc<core::Request>>, Lease)>, String> {
        let before = self.state.xhr.pending.capacity();
        if self.state.xhr.pending.len() < before {
            return Ok(None);
        }
        let limit = self.state.limits.capacity.script_storage_bytes;
        let available = limit.saturating_sub(self.state.xhr.used.load(Ordering::Acquire));
        let slot = std::mem::size_of::<Arc<core::Request>>();
        let growth = before.max(4).min(available / slot);
        let capacity = before
            .checked_add(growth)
            .filter(|_| growth > 0)
            .ok_or("Pending script request backing exhausted.")?;
        let mut next = VecDeque::new();
        next.try_reserve_exact(capacity)
            .map_err(|_| "Could not reserve pending script requests.")?;
        let bytes = next
            .capacity()
            .checked_sub(before)
            .and_then(|slots| slots.checked_mul(slot))
            .ok_or("Pending script request storage overflow.")?;
        let storage = Lease::new(self.state.xhr.used.clone(), bytes, limit)?;
        self.state.copies(self.state.xhr.pending.len() * slot)?;
        Ok(Some((next, storage)))
    }
    pub(super) fn xhr_cancel_all(&mut self) {
        self.state.xhr.cancel();
    }
}

impl State {
    pub(super) fn cancel(&mut self) {
        for record in &mut self.records {
            if let Some(request) = record.request.take() {
                request.abort();
            }
            record.sent = false;
            record.ready = 0;
            record.response = None;
            record.upload_active = false;
            record.generation = record.generation.saturating_add(1);
        }
        self.pending.clear();
    }
}
fn response_encoding(record: &Record, response: &crate::http::Response) -> Result<String, String> {
    let value = if record.mime.is_empty() {
        response.header("content-type").unwrap_or("")
    } else {
        &record.mime
    };
    Ok(mime::parse(value)?
        .and_then(|mime| mime.charset().map(str::to_ascii_lowercase))
        .unwrap_or_else(|| "utf-8".into()))
}

impl Page {
    pub(crate) fn has_native_script_requests(&self) -> bool {
        self.state.xhr.records.iter().any(|record| record.sent)
    }
    pub(crate) fn native_request_pending_count(&self) -> usize {
        self.state.xhr.pending.len()
    }
    pub(crate) fn take_native_request(&mut self) -> Option<Arc<core::Request>> {
        self.state.xhr.pending.pop_front()
    }
    pub(crate) fn native_request_storage(&self) -> (Arc<AtomicUsize>, usize) {
        (
            self.state.xhr.used.clone(),
            self.state.limits.capacity.script_storage_bytes,
        )
    }
    fn current_xhr_request(&self, request: &Arc<core::Request>) -> Option<usize> {
        let id = request.id.receiver.checked_sub(BASE)? as usize;
        let record = self.state.xhr.records.get(id)?;
        (record.sent
            && record.generation == request.id.generation
            && record
                .request
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, request))
            && !request.cancel.load(Ordering::Acquire))
        .then_some(id)
    }
    fn dispatch_request_notification(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        notification: Notification,
    ) -> bool {
        let token = match (Bridge {
            doc,
            forms,
            state: &mut self.state,
        })
        .xhr_notification(notification)
        {
            Ok(Some(token)) => token,
            Ok(None) => return true,
            Err(error) => {
                self.state.error("XHR event admission", &error);
                return false;
            }
        };
        let result = self.runtime.invoke_native_dispatch(
            token,
            &mut Bridge {
                doc,
                forms,
                state: &mut self.state,
            },
        );
        if let Err(error) = result {
            self.state.error("XHR event delivery", &error);
            (Bridge {
                doc,
                forms,
                state: &mut self.state,
            })
            .xhr_cancel_all();
            return false;
        }
        if !self.microtask_checkpoint(doc, forms) {
            (Bridge {
                doc,
                forms,
                state: &mut self.state,
            })
            .xhr_cancel_all();
            return false;
        }
        true
    }
    pub(crate) fn complete_native_request(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        mut completion: core::Completion,
    ) -> Outcome {
        let before = self.state.errors;
        let Some(id) = self.current_xhr_request(&completion.request) else {
            return Outcome::default();
        };
        if !self.state.enabled || !doc.scripting_enabled {
            return Outcome::default();
        }
        self.begin_job();
        let generation = completion.request.id.generation;
        let target = completion.request.id.receiver;
        let result = match completion.result {
            Ok(mut response) => {
                self.state.log(&format!(
                    "XHR native response: status {}; {} encoded body bytes, {} decoded body bytes.",
                    response.status,
                    response.encoded_body_bytes,
                    response.body.len()
                ));
                let label = response_encoding(&self.state.xhr.records[id], &response);
                match label
                    .and_then(|label| self.runtime.decode_request_text(&response.body, &label))
                {
                    Ok(text) if text.len() <= self.state.limits.host.string_bytes => {
                        match completion._storage.take() {
                            Some(mut storage) => match storage.replace(
                                response.body.capacity(),
                                text.capacity(),
                                self.state.limits.capacity.script_storage_bytes,
                            ) {
                                Ok(()) => {
                                    let received_bytes = response.body.len() as u64;
                                    response.body = Vec::new();
                                    Ok(Received {
                                        received_bytes,
                                        response,
                                        text,
                                        _storage: storage,
                                    })
                                }
                                Err(error) => Err(core::Failure::Network(error)),
                            },
                            None => Err(core::Failure::Storage),
                        }
                    }
                    Ok(_) => Err(core::Failure::Storage),
                    Err(error) => Err(core::Failure::Network(error)),
                }
            }
            Err(error) => Err(error),
        };
        let terminal = match result {
            Ok(received) => {
                let loaded = received.received_bytes;
                let total = if completion.request.outgoing.method == crate::http::ScriptMethod::Head
                {
                    0
                } else {
                    received
                        .response
                        .header("content-length")
                        .and_then(|value| value.parse::<u64>().ok())
                        .unwrap_or(0)
                };
                let progress = Progress {
                    length_computable: total != 0,
                    loaded,
                    total,
                };
                self.state.xhr.records[id].response = Some(received);
                if self.state.xhr.records[id].upload_active {
                    self.state.xhr.records[id].upload_active = false;
                    let count = completion.request.outgoing.body.len() as u64;
                    let upload_progress = Progress {
                        length_computable: count != 0,
                        loaded: count,
                        total: count,
                    };
                    for event in ["progress", "load", "loadend"] {
                        if self.current_xhr_request(&completion.request).is_none() {
                            break;
                        }
                        if !self.dispatch_request_notification(
                            doc,
                            forms,
                            Notification {
                                target: UPLOAD + id as u32,
                                generation,
                                event,
                                progress: Some(upload_progress),
                            },
                        ) {
                            break;
                        }
                    }
                }
                for ready in [2, 3] {
                    if ready == 3 && loaded == 0 {
                        continue;
                    }
                    if self.current_xhr_request(&completion.request).is_none() {
                        break;
                    }
                    self.state.xhr.records[id].ready = ready;
                    if !self.dispatch_request_notification(
                        doc,
                        forms,
                        Notification {
                            target,
                            generation,
                            event: "readystatechange",
                            progress: None,
                        },
                    ) {
                        break;
                    }
                }
                if self.current_xhr_request(&completion.request).is_some() {
                    self.dispatch_request_notification(
                        doc,
                        forms,
                        Notification {
                            target,
                            generation,
                            event: "progress",
                            progress: Some(progress),
                        },
                    );
                }
                if self.current_xhr_request(&completion.request).is_some() {
                    Some(("load", progress))
                } else {
                    None
                }
            }
            Err(error) => {
                let event = if matches!(error, core::Failure::Timeout) {
                    "timeout"
                } else {
                    "error"
                };
                match error {
                    core::Failure::Network(message) => {
                        self.state.log(&format!("XHR request failed: {message}"))
                    }
                    core::Failure::Storage => self.state.error(
                        "XHR completion",
                        "Native request response storage was refused.",
                    ),
                    core::Failure::Timeout => {}
                }
                self.state.xhr.records[id].response = None;
                Some((event, zero()))
            }
        };
        if let Some((event, progress)) = terminal {
            if self.current_xhr_request(&completion.request).is_some() {
                self.state.xhr.records[id].ready = 4;
                self.state.xhr.records[id].sent = false;
                self.state.xhr.records[id].request = None;
                if self.dispatch_request_notification(
                    doc,
                    forms,
                    Notification {
                        target,
                        generation,
                        event: "readystatechange",
                        progress: None,
                    },
                ) && self.state.xhr.records[id].generation == generation
                {
                    let upload = self.state.xhr.records[id].upload_active;
                    self.state.xhr.records[id].upload_active = false;
                    if upload {
                        for kind in [event, "loadend"] {
                            if self.state.xhr.records[id].generation != generation {
                                break;
                            }
                            if !self.dispatch_request_notification(
                                doc,
                                forms,
                                Notification {
                                    target: UPLOAD + id as u32,
                                    generation,
                                    event: kind,
                                    progress: Some(progress),
                                },
                            ) {
                                break;
                            }
                        }
                    }
                    for kind in [event, "loadend"] {
                        if self.state.xhr.records[id].generation != generation {
                            break;
                        }
                        if !self.dispatch_request_notification(
                            doc,
                            forms,
                            Notification {
                                target,
                                generation,
                                event: kind,
                                progress: Some(progress),
                            },
                        ) {
                            break;
                        }
                    }
                }
            }
        }
        Outcome {
            layout_changed: std::mem::take(&mut self.state.changed),
            errors: self.state.errors.saturating_sub(before),
            ..Outcome::default()
        }
    }
}
