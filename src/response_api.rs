//! Native buffered Response values and genuine intrinsic Body promises.
//! Streams, Blob/FormData and network Fetch are separate guarded increments.
//! https://fetch.spec.whatwg.org/#response-class
use super::function_reachability::Edge;
use super::headers_api::{self, Guard};
use super::*;
use std::rc::Rc;
#[derive(Default)]
pub(super) struct Roots {
    prototype: Option<usize>,
}
impl Roots {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(Edge)) {
        if let Some(id) = self.prototype {
            add(Edge::Object(id));
        }
    }
}
pub(super) struct Data {
    status: u16,
    status_text: String,
    headers: usize,
    body: Option<Rc<Vec<u8>>>,
    has_body: bool,
    used: bool,
    url: String,
    kind: &'static str,
    redirected: bool,
}
impl Data {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(Edge)) {
        add(Edge::Object(self.headers));
    }
}
#[derive(Clone, Copy)]
pub(super) enum Op {
    Constructor,
    Get(&'static str),
    Clone,
    Consume(&'static str),
    Error,
    Json,
}
impl Op {
    pub(super) fn arity(self) -> usize {
        if matches!(self, Self::Json) {
            1
        } else {
            0
        }
    }
}
enum Input {
    Null,
    Text(String),
    Binary(Value),
    Params(usize),
}
struct Init {
    status: u16,
    status_text: String,
    headers: Vec<headers_api::Pair>,
}
fn type_error(message: &str) -> Fault {
    Fault::exception("TypeError", message)
}
fn body_storage(bytes: &Vec<u8>) -> Result<usize, Fault> {
    bytes
        .capacity()
        .checked_add(std::mem::size_of::<Vec<u8>>() + 2 * std::mem::size_of::<usize>())
        .ok_or_else(|| Fault::new("Response body backing overflow."))
}
impl Runtime {
    pub(super) fn initialize_response_api(&mut self) -> Result<(), Fault> {
        let prototype = self.new_object(None)?;
        self.response_roots.prototype = Some(prototype);
        let constructor = self.native_function(Builtin::Response(Op::Constructor), "Response")?;
        self.put_binding(
            self.global_environment,
            "Response".into(),
            Some(constructor.clone()),
            Declaration::Var,
            false,
        )?;
        self.constructor_prototype("Response", Value::Object(prototype))?;
        for name in [
            "type",
            "url",
            "redirected",
            "status",
            "ok",
            "statusText",
            "headers",
            "body",
            "bodyUsed",
        ] {
            let Value::Function(get) =
                self.native_function(Builtin::Response(Op::Get(name)), &format!("get {name}"))?
            else {
                unreachable!()
            };
            self.store_accessor_property(
                prototype,
                name.into(),
                Accessors {
                    get: Some(get),
                    set: None,
                },
                (true, true),
            )?;
        }
        for name in ["text", "json", "arrayBuffer", "bytes"] {
            let f = self.native_function(Builtin::Response(Op::Consume(name)), name)?;
            self.store_property(prototype, name.into(), f, Some((true, true, true)))?;
        }
        let clone = self.native_function(Builtin::Response(Op::Clone), "clone")?;
        self.store_property(prototype, "clone".into(), clone, Some((true, true, true)))?;
        self.store_symbol_property(
            prototype,
            self.symbol_to_string_tag.unwrap(),
            Value::String("Response".into()),
            Some((false, false, true)),
        )?;
        let constructor = self.ordinary_id(Ordinary::from_value(&constructor).unwrap())?;
        for (name, op) in [("error", Op::Error), ("json", Op::Json)] {
            let f = self.native_function(Builtin::Response(op), name)?;
            self.store_property(constructor, name.into(), f, Some((true, true, true)))?;
        }
        Ok(())
    }
    fn response_room(&self, bytes: usize) -> Result<usize, Fault> {
        self.value_bytes
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.heap.retained_values)
            .ok_or_else(|| Fault::new("Response retained storage allowance exhausted."))
    }
    fn response_copy(&mut self, text: &str) -> Result<String, Fault> {
        self.work(text.len())?;
        self.response_room(text.len())?;
        let mut out = String::new();
        out.try_reserve_exact(text.len())
            .map_err(|_| Fault::new("Could not reserve Response text."))?;
        out.push_str(text);
        Ok(out)
    }
    fn response_field_text(&mut self, id: usize, url: bool) -> Result<String, Fault> {
        let data = self.objects[id].response.as_ref().unwrap();
        let len = if url {
            data.url.len()
        } else {
            data.status_text.len()
        };
        self.work(len)?;
        self.response_room(len)?;
        let mut text = String::new();
        text.try_reserve_exact(len)
            .map_err(|_| Fault::new("Could not copy native Response text."))?;
        let data = self.objects[id].response.as_ref().unwrap();
        text.push_str(if url { &data.url } else { &data.status_text });
        Ok(text)
    }
    fn response_usv(&mut self, value: Value, host: &mut dyn Host) -> Result<String, Fault> {
        let primitive = self.primitive_with_hint(value, true, host)?;
        self.work(primitive.bytes())?;
        if let Value::StringUnits(units) = primitive {
            let mut text = String::new();
            for c in std::char::decode_utf16(units.iter().copied()) {
                let mut bytes = [0; 4];
                self.append_string(&mut text, c.unwrap_or('\u{fffd}').encode_utf8(&mut bytes))?;
            }
            return Ok(text);
        }
        primitive.script_text()
    }
    fn response_input(&mut self, value: Value, host: &mut dyn Host) -> Result<Input, Fault> {
        if matches!(value, Value::Null | Value::Undefined) {
            return Ok(Input::Null);
        }
        if let Value::Object(id) = value {
            if matches!(
                self.objects[id].binary,
                Some(
                    binary_data::Data::Buffer(_)
                        | binary_data::Data::Typed(_)
                        | binary_data::Data::View(_)
                )
            ) {
                return Ok(Input::Binary(value));
            }
            if matches!(self.objects[id].url, Some(url_api::Data::Params { .. })) {
                return Ok(Input::Params(id));
            }
        }
        Ok(Input::Text(self.response_usv(value, host)?))
    }
    fn response_member(
        &mut self,
        init: &Value,
        name: &str,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if matches!(init, Value::Null | Value::Undefined) {
            return Ok(Value::Undefined);
        }
        if Ordinary::from_value(init).is_none() && !matches!(init, Value::Host(_)) {
            return Err(type_error("ResponseInit requires a dictionary object."));
        }
        let receiver = self.copy_value(init)?;
        self.read_reference(&Reference::Property(receiver, name.into()), host)
    }
    fn response_init(&mut self, init: &Value, host: &mut dyn Host) -> Result<Init, Fault> {
        // Web IDL dictionary members are read and converted in lexical order.
        let value = self.response_member(init, "headers", host)?;
        let headers = self.headers_input(value, host)?;
        self.headers_hold(headers_api::bytes(&headers)?, |runtime| {
            let value = runtime.response_member(init, "status", host)?;
            let status = if matches!(value, Value::Undefined) {
                200
            } else {
                let primitive = runtime.primitive_with_hint(value, false, host)?;
                runtime.work(primitive.bytes())?;
                let value = primitive.script_number()?;
                if !value.is_finite() || value == 0.0 {
                    0
                } else {
                    value.trunc().rem_euclid(65536.0) as u16
                }
            };
            let value = runtime.response_member(init, "statusText", host)?;
            let status_text = if matches!(value, Value::Undefined) {
                String::new()
            } else {
                runtime.headers_byte_string(value, host)?
            };
            Ok(Init {
                status,
                status_text,
                headers,
            })
        })
    }
    fn response_extract(
        &mut self,
        input: Input,
        host: &mut dyn Host,
    ) -> Result<(Option<Rc<Vec<u8>>>, Option<&'static str>, usize), Fault> {
        let (bytes, content_type) = match input {
            Input::Null => return Ok((None, None, 0)),
            Input::Text(text) => (text.into_bytes(), Some("text/plain;charset=UTF-8")),
            Input::Params(id) => {
                let text = self.url_builtin(
                    url_api::Op::Params("toString"),
                    &[],
                    Value::Object(id),
                    host,
                )?;
                let Value::String(text) = text else {
                    unreachable!()
                };
                (
                    text.into_bytes(),
                    Some("application/x-www-form-urlencoded;charset=UTF-8"),
                )
            }
            Input::Binary(value) => {
                let (buffer, offset, length) = self.binary_source_range(&value)?;
                self.work(length)?;
                self.response_room(length)?;
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(length)
                    .map_err(|_| Fault::new("Could not reserve Response body bytes."))?;
                bytes.extend_from_slice(&self.buffer_bytes(buffer)?[offset..offset + length]);
                (bytes, None)
            }
        };
        let storage = body_storage(&bytes)?;
        self.response_room(storage)?;
        Ok((Some(Rc::new(bytes)), content_type, storage))
    }
    fn response_new(&mut self, data: Data, body_storage: usize) -> Result<Value, Fault> {
        self.ensure_global_intrinsics("Response")?;
        let storage = std::mem::size_of::<Data>()
            .checked_add(data.status_text.capacity())
            .and_then(|n| n.checked_add(data.url.capacity()))
            .and_then(|n| n.checked_add(body_storage))
            .ok_or_else(|| Fault::new("Response native storage overflow."))?;
        self.work(storage)?;
        self.response_room(storage)?;
        let id = self.new_object(None)?;
        let retained = self.response_room(storage)?;
        self.objects[id].response = Some(Box::new(data));
        self.objects[id].prototype = self.response_roots.prototype.map(Ordinary::Object);
        self.value_bytes = retained;
        Ok(Value::Object(id))
    }
    fn response_finish(
        &mut self,
        mut init: Init,
        body: Option<Rc<Vec<u8>>>,
        content_type: Option<&str>,
        storage: usize,
    ) -> Result<Value, Fault> {
        if !(200..=599).contains(&init.status) {
            return Err(Fault::exception(
                "RangeError",
                "Response status must be between 200 and 599.",
            ));
        }
        if init.status_text.chars().any(|c| {
            c != '\t'
                && !(0x20..=0x7e).contains(&u32::from(c))
                && !(0x80..=0xff).contains(&u32::from(c))
        }) {
            return Err(type_error("Invalid Response status text."));
        }
        if let Some(content_type) = content_type.filter(|_| {
            !init
                .headers
                .iter()
                .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        }) {
            self.response_room(headers_api::bytes(&init.headers)?.saturating_add(
                std::mem::size_of::<headers_api::Pair>() + content_type.len() + 12,
            ))?;
            init.headers
                .try_reserve_exact(1)
                .map_err(|_| Fault::new("Could not reserve Response Content-Type."))?;
            init.headers.push((
                self.response_copy("content-type")?,
                self.response_copy(content_type)?,
            ));
        }
        let Value::Object(headers) = self.headers_from_converted(init.headers, Guard::Response)?
        else {
            unreachable!()
        };
        if body.is_some() && matches!(init.status, 204 | 205 | 304) {
            return Err(type_error(
                "A null-body Response status cannot have a body.",
            ));
        }
        self.response_new(
            Data {
                status: init.status,
                status_text: init.status_text,
                headers,
                has_body: body.is_some(),
                body,
                used: false,
                url: String::new(),
                kind: "default",
                redirected: false,
            },
            storage,
        )
    }
    pub(super) fn response_construct(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let value = self.copy_value(args.first().unwrap_or(&Value::Null))?;
        let input = self.response_input(value, host)?;
        let held = if let Input::Text(text) = &input {
            text.capacity()
        } else {
            0
        };
        let init = self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?;
        let (init, body, kind, storage) = self.headers_hold(held, |runtime| {
            let init = runtime.response_init(&init, host)?;
            let (body, kind, storage) = runtime.response_extract(input, host)?;
            Ok((init, body, kind, storage))
        })?;
        self.response_finish(init, body, kind, storage)
    }
    fn response_id(&self, value: &Value) -> Result<usize, Fault> {
        if let Value::Object(id) = value {
            if self.object(*id)?.response.is_some() {
                return Ok(*id);
            }
        }
        Err(type_error(
            "Response operation has an incompatible receiver.",
        ))
    }
    fn response_clone(&mut self, id: usize) -> Result<Value, Fault> {
        if self.objects[id].response.as_ref().unwrap().used {
            return Err(type_error("Response body is already used."));
        }
        let realm = self.objects[id].realm;
        self.with_intrinsic_realm(realm, |runtime| {
            let data = runtime.objects[id].response.as_ref().unwrap();
            let status = data.status;
            let body = data.body.clone();
            let kind = data.kind;
            let redirected = data.redirected;
            let headers = data.headers;
            let text = runtime.response_field_text(id, false)?;
            let url = runtime.response_field_text(id, true)?;
            let Value::Object(headers) = runtime.headers_clone_native(headers)? else {
                unreachable!()
            };
            runtime.response_new(
                Data {
                    status,
                    status_text: text,
                    headers,
                    has_body: body.is_some(),
                    body,
                    used: false,
                    url,
                    kind,
                    redirected,
                },
                0,
            )
        })
    }
    fn response_consume(
        &mut self,
        this: Value,
        kind: &str,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let id = self.response_id(&this)?;
        let data = self.objects[id].response.as_ref().unwrap();
        if data.used {
            return Err(type_error("Response body is already used."));
        }
        let has_body = data.has_body;
        let body = self.objects[id].response.as_mut().unwrap().body.take();
        if has_body {
            self.objects[id].response.as_mut().unwrap().used = true;
        }
        // A consumed branch releases its source. The last branch refunds the
        // shared immutable byte backing after processing, including failures.
        let release = body
            .as_ref()
            .filter(|bytes| Rc::strong_count(bytes) == 1)
            .map(|bytes| body_storage(bytes))
            .transpose()?
            .unwrap_or(0);
        let bytes = body.as_deref().map_or(&[][..], |bytes| bytes.as_slice());
        let result = (|| {
            self.work(bytes.len())?;
            if matches!(kind, "arrayBuffer" | "bytes") {
                let realm = self.objects[id].realm;
                return self.with_intrinsic_realm(realm, |runtime| {
                    let id = if kind == "arrayBuffer" {
                        runtime.new_buffer(bytes.len())?
                    } else {
                        runtime.new_typed(binary_data::Kind::U8, bytes.len())?
                    };
                    let buffer = if kind == "arrayBuffer" {
                        id
                    } else {
                        runtime.typed_view(id).unwrap().buffer
                    };
                    runtime.buffer_bytes_mut(buffer)?.copy_from_slice(bytes);
                    Ok(Value::Object(id))
                });
            }
            let text = self.decode_body_utf8(bytes)?;
            if kind == "json" {
                self.json_parse_builtin(&[Value::String(text)], host)
            } else {
                self.checked_value(Value::String(text))
            }
        })();
        drop(body);
        self.value_bytes = self
            .value_bytes
            .checked_sub(release)
            .ok_or_else(|| Fault::new("Response body storage ownership mismatch."))?;
        result
    }
    pub(super) fn response_builtin(
        &mut self,
        operation: Op,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match operation {
            Op::Constructor => Err(type_error("Response requires new.")),
            Op::Error => {
                let Value::Object(headers) =
                    self.headers_from_converted(Vec::new(), Guard::Immutable)?
                else {
                    unreachable!()
                };
                self.response_new(
                    Data {
                        status: 0,
                        status_text: String::new(),
                        headers,
                        body: None,
                        has_body: false,
                        used: false,
                        url: String::new(),
                        kind: "error",
                        redirected: false,
                    },
                    0,
                )
            }
            Op::Json => {
                if args.is_empty() {
                    return Err(type_error("Response.json requires a data argument."));
                }
                let init = self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?;
                let init = self.response_init(&init, host)?;
                let held = headers_api::bytes(&init.headers)?
                    .checked_add(init.status_text.capacity())
                    .ok_or_else(|| Fault::new("Response init scratch overflow."))?;
                let (body, storage) = self.headers_hold(held, |runtime| {
                    let value = runtime.json_stringify(&args[..1], host)?;
                    let Value::String(text) = value else {
                        return Err(type_error("Response JSON data is not serializable."));
                    };
                    let (body, _, storage) = runtime.response_extract(Input::Text(text), host)?;
                    Ok((body, storage))
                })?;
                self.response_finish(init, body, Some("application/json"), storage)
            }
            Op::Consume(kind) => {
                let result = self.response_consume(this, kind, host);
                self.resolved_native_operation(result, host)
            }
            Op::Clone => {
                let id = self.response_id(&this)?;
                self.response_clone(id)
            }
            Op::Get(name) => {
                let id = self.response_id(&this)?;
                let data = self.objects[id].response.as_ref().unwrap();
                match name {
                    "status" => Ok(Value::Number(f64::from(data.status))),
                    "ok" => Ok(Value::Bool((200..=299).contains(&data.status))),
                    "redirected" => Ok(Value::Bool(data.redirected)),
                    "bodyUsed" => Ok(Value::Bool(data.used)),
                    "headers" => Ok(Value::Object(data.headers)),
                    "type" => {
                        let kind = data.kind;
                        self.checked_value(Value::String(kind.into()))
                    }
                    "body" => {
                        if !data.has_body {
                            Ok(Value::Null)
                        } else {
                            Err(Fault::new(
                                "Response ReadableStream access is not implemented.",
                            ))
                        }
                    }
                    "url" | "statusText" => {
                        let text = self.response_field_text(id, name == "url")?;
                        self.checked_value(Value::String(text))
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "response_api_tests.rs"]
mod tests;
