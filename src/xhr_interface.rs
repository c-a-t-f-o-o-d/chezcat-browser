//! Canonical request interface objects and synchronous native notification seam.
use super::{Declaration, Fault, Function, Host, Ordinary, Runtime, Value};
pub(crate) const CALL_PREFIX: &str = "\0XHR.call.";
#[derive(Clone, Copy)]
pub struct Progress {
    pub length_computable: bool,
    pub loaded: u64,
    pub total: u64,
}
#[derive(Clone, Copy)]
pub struct Notification {
    pub target: u32,
    pub generation: u64,
    pub event: &'static str,
    pub progress: Option<Progress>,
}
pub struct Call {
    pub value: Value,
    pub notifications: [Option<Notification>; 5],
    pub initial_token: Option<usize>,
    pub finish: Option<(u32, u64)>,
}
impl Call {
    pub(crate) fn value(value: Value) -> Self {
        Self {
            value,
            notifications: [None; 5],
            initial_token: None,
            finish: None,
        }
    }
}
pub enum Error {
    Exception(&'static str, String),
    Fatal(String),
}
impl From<String> for Error {
    fn from(error: String) -> Self {
        Self::Fatal(error)
    }
}
pub(crate) fn operation(method: &str) -> Option<&str> {
    method.strip_prefix(CALL_PREFIX)
}
pub(crate) fn getter(method: &str) -> Option<&str> {
    method.strip_prefix("\0XHR.get.")
}
pub(crate) fn setter(method: &str) -> Option<&str> {
    method.strip_prefix("\0XHR.set.")
}
const HANDLERS: &[(&str, &str, &str)] = &[
    (
        "onloadstart",
        "\0XHR.get.onloadstart",
        "\0XHR.set.onloadstart",
    ),
    ("onprogress", "\0XHR.get.onprogress", "\0XHR.set.onprogress"),
    ("onabort", "\0XHR.get.onabort", "\0XHR.set.onabort"),
    ("onerror", "\0XHR.get.onerror", "\0XHR.set.onerror"),
    ("onload", "\0XHR.get.onload", "\0XHR.set.onload"),
    ("ontimeout", "\0XHR.get.ontimeout", "\0XHR.set.ontimeout"),
    ("onloadend", "\0XHR.get.onloadend", "\0XHR.set.onloadend"),
];
impl Runtime {
    fn request_function(
        &mut self,
        window: u32,
        operation: &'static str,
        name: &str,
        length: usize,
    ) -> Result<Value, Fault> {
        let allocation = self
            .allocate_host_function_with_length(window, operation, length)
            .map_err(Fault::new)?;
        if let Some(Function::Host {
            receiver_required, ..
        }) = self.functions.get_mut(allocation.id)
        {
            *receiver_required = true;
        }
        self.store_property(
            self.function_properties[allocation.id],
            "name".into(),
            Value::String(name.into()),
            Some((false, false, true)),
        )?;
        Ok(Value::Function(allocation.id))
    }
    pub(crate) fn define_xhr_interfaces(&mut self, window: u32) -> Result<(), String> {
        let marker = self.inhibit_transient_collection();
        let result = (|| {
            for (name, parent, constructible) in [
                ("XMLHttpRequestEventTarget", "EventTarget", false),
                ("XMLHttpRequestUpload", "XMLHttpRequestEventTarget", false),
                ("XMLHttpRequest", "XMLHttpRequestEventTarget", true),
            ] {
                let parent = *self
                    .native_constructor_prototypes
                    .get(&(window, parent))
                    .ok_or_else(|| Fault::new("Request parent interface is unavailable."))?;
                let parent_constructor = self.objects[parent]
                    .properties
                    .get("constructor")
                    .ok_or_else(|| Fault::new("Request parent constructor is unavailable."))?
                    .value
                    .as_value()?
                    .clone();
                let Value::Function(parent_constructor) = parent_constructor else {
                    return Err(Fault::new("Invalid request parent constructor."));
                };
                let allocation = self
                    .allocate_host_function_with_length(window, name, 0)
                    .map_err(Fault::new)?;
                if let Some(Function::Host {
                    constructible: flag,
                    ..
                }) = self.functions.get_mut(allocation.id)
                {
                    *flag = constructible;
                }
                let constructor = Value::Function(allocation.id);
                self.objects[self.function_properties[allocation.id]].prototype =
                    Some(Ordinary::Function(parent_constructor));
                let prototype = self.new_object(None)?;
                self.objects[prototype].prototype = Some(Ordinary::Object(parent));
                self.store_property(
                    prototype,
                    "constructor".into(),
                    constructor.clone(),
                    Some((false, true, true)),
                )?;
                self.store_property(
                    self.function_properties[allocation.id],
                    "prototype".into(),
                    Value::Object(prototype),
                    Some((false, false, false)),
                )?;
                self.store_symbol_property(
                    prototype,
                    self.symbol_to_string_tag.unwrap(),
                    Value::String(name.into()),
                    Some((false, false, true)),
                )?;
                self.native_constructor_prototypes
                    .insert((window, name), prototype);
                self.put_binding(
                    self.global_environment,
                    name.into(),
                    Some(constructor),
                    Declaration::Var,
                    false,
                )?;
                self.global_attributes
                    .insert(name.into(), (false, true, true));
                if name == "XMLHttpRequestEventTarget" {
                    for &(name, get, set) in HANDLERS {
                        self.define_host_attribute(prototype, window, name, get, Some(set))?;
                    }
                }
                if name == "XMLHttpRequest" {
                    for (name, get, set) in [
                        (
                            "onreadystatechange",
                            "\0XHR.get.onreadystatechange",
                            Some("\0XHR.set.onreadystatechange"),
                        ),
                        ("readyState", "\0XHR.get.readyState", None),
                        ("timeout", "\0XHR.get.timeout", Some("\0XHR.set.timeout")),
                        (
                            "withCredentials",
                            "\0XHR.get.withCredentials",
                            Some("\0XHR.set.withCredentials"),
                        ),
                        ("upload", "\0XHR.get.upload", None),
                        ("responseURL", "\0XHR.get.responseURL", None),
                        ("status", "\0XHR.get.status", None),
                        ("statusText", "\0XHR.get.statusText", None),
                        (
                            "responseType",
                            "\0XHR.get.responseType",
                            Some("\0XHR.set.responseType"),
                        ),
                        ("response", "\0XHR.get.response", None),
                        ("responseText", "\0XHR.get.responseText", None),
                        ("responseXML", "\0XHR.get.responseXML", None),
                    ] {
                        self.define_host_attribute(prototype, window, name, get, set)?;
                    }
                    for (name, operation, length) in [
                        ("open", "\0XHR.call.open", 2),
                        ("setRequestHeader", "\0XHR.call.setRequestHeader", 2),
                        ("send", "\0XHR.call.send", 0),
                        ("abort", "\0XHR.call.abort", 0),
                        ("getResponseHeader", "\0XHR.call.getResponseHeader", 1),
                        (
                            "getAllResponseHeaders",
                            "\0XHR.call.getAllResponseHeaders",
                            0,
                        ),
                        ("overrideMimeType", "\0XHR.call.overrideMimeType", 1),
                    ] {
                        let function = self.request_function(window, operation, name, length)?;
                        self.store_property(
                            prototype,
                            name.into(),
                            function,
                            Some((true, true, true)),
                        )?;
                    }
                    for (name, value) in [
                        ("UNSENT", 0),
                        ("OPENED", 1),
                        ("HEADERS_RECEIVED", 2),
                        ("LOADING", 3),
                        ("DONE", 4),
                    ] {
                        for object in [prototype, self.function_properties[allocation.id]] {
                            self.store_property(
                                object,
                                name.into(),
                                Value::Number(f64::from(value)),
                                Some((true, false, false)),
                            )?;
                        }
                    }
                }
            }
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }
    fn request_fault(&mut self, error: Error) -> Result<Fault, Fault> {
        Ok(match error {
            Error::Fatal(error) => Fault::new(error),
            Error::Exception(kind, message) if matches!(kind, "TypeError" | "RangeError") => {
                Fault::exception(kind, message)
            }
            Error::Exception(kind, message) => {
                Fault::thrown(self.native_dom_exception(kind, &message)?)
            }
        })
    }
    fn request_string(
        &mut self,
        value: Value,
        byte_string: bool,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let primitive = self.primitive_with_hint(value, true, host)?;
        self.work(primitive.bytes())?;
        let text = match primitive {
            Value::StringUnits(units) => {
                if byte_string && units.iter().any(|&unit| unit > 255) {
                    return Err(Fault::exception(
                        "TypeError",
                        "ByteString contains a value above255.",
                    ));
                }
                let mut text = String::new();
                for character in std::char::decode_utf16(units.iter().copied()) {
                    let character = character.unwrap_or('\u{fffd}');
                    let mut bytes = [0; 4];
                    self.append_string(&mut text, character.encode_utf8(&mut bytes))?;
                }
                text
            }
            value => value.script_text()?,
        };
        if byte_string && text.chars().any(|value| u32::from(value) > 255) {
            return Err(Fault::exception(
                "TypeError",
                "ByteString contains a value above255.",
            ));
        }
        self.checked_value(Value::String(text))
    }
    pub(super) fn call_request_operation(
        &mut self,
        object: u32,
        method: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let mut converted = Vec::new();
        converted
            .try_reserve_exact(args.len())
            .map_err(|_| Fault::new("Could not reserve XHR arguments."))?;
        for value in args {
            converted.push(self.copy_value(value)?);
        }
        let operation = operation(method).unwrap_or("");
        let mut binary = None;
        if operation == "send" {
            if let Some(value) = converted.first().cloned() {
                if let Value::Object(id) = value {
                    if matches!(
                        self.object(id)?.binary,
                        Some(
                            super::binary_data::Data::Buffer(_)
                                | super::binary_data::Data::Typed(_)
                                | super::binary_data::Data::View(_)
                        )
                    ) {
                        let (buffer, offset, length) = self.binary_source_range(&value)?;
                        self.work(length)?;
                        let mut bytes = Vec::new();
                        bytes
                            .try_reserve_exact(length)
                            .map_err(|_| Fault::new("Could not reserve request body snapshot."))?;
                        bytes.extend_from_slice(
                            &self.buffer_bytes(buffer)?[offset..offset + length],
                        );
                        binary = Some(bytes);
                    }
                }
                if binary.is_none() && !matches!(value, Value::Null | Value::Undefined) {
                    if let Value::Host(handle) = value {
                        if host.tree_walker_node_type(handle) == Some(9) {
                            let error = Error::Exception(
                                "NotSupportedError",
                                "Document request serialization is not implemented.".into(),
                            );
                            return Err(self.request_fault(error)?);
                        }
                    }
                    converted[0] = self.request_string(value, false, host)?;
                }
            }
        } else {
            let indices: &[(usize, bool)] = match operation {
                "open" => &[(0, true), (1, false), (3, false), (4, false)],
                "setRequestHeader" => &[(0, true), (1, true)],
                "getResponseHeader" => &[(0, true)],
                "overrideMimeType" => &[(0, false)],
                _ => &[],
            };
            for &(index, bytes) in indices {
                if let Some(value) = converted.get(index).cloned() {
                    if operation == "open"
                        && index >= 3
                        && matches!(value, Value::Null | Value::Undefined)
                    {
                        converted[index] = Value::Null;
                    } else {
                        converted[index] = self.request_string(value, bytes, host)?;
                    }
                }
            }
        }
        let args = &converted;
        host.native_operation_caller(
            self.native_access_caller
                .unwrap_or_else(|| self.browser_global.map(|(window, _)| window)),
        );
        self.validate_native_access(object, host)?;
        let call = match host.prepare_request_call(object, method, args, binary) {
            Ok(call) => call,
            Err(error) => return Err(self.request_fault(error)?),
        };
        let result: Result<Value, Fault> = (|| {
            for (index, notification) in call.notifications.into_iter().enumerate() {
                let Some(notification) = notification else {
                    continue;
                };
                let token = if index == 0 {
                    match call.initial_token {
                        Some(token) => Some(token),
                        None => host
                            .begin_request_notification(notification)
                            .map_err(Fault::new)?,
                    }
                } else {
                    host.begin_request_notification(notification)
                        .map_err(Fault::new)?
                };
                if let Some(token) = token {
                    self.dispatch_native_event(token, host)?;
                }
            }
            Ok(call.value)
        })();
        if let Some((target, generation)) = call.finish {
            host.finish_request_abort(target, generation);
        }
        if result
            .as_ref()
            .is_err_and(|fault| fault.exception.is_none())
        {
            host.cancel_request_work();
        }
        result
    }
}
