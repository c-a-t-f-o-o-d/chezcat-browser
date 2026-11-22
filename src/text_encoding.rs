//! Original bounded text/binary conversion. Unicode, legacy single-byte,
//! x-user-defined, legacy multibyte and internal replacement decoding include
//! streaming, BOM handling and fatal errors. No platform codec, network,
//! filesystem or device capability is used here.

#[cfg(test)]
#[path = "text_encoding_single_byte_tests.rs"]
mod single_byte_tests;
#[cfg(test)]
#[path = "text_encoding_east_asian_tests.rs"]
mod east_asian_tests;
#[cfg(test)]
#[path = "text_encoding_multibyte_tests.rs"]
mod multibyte_tests;
#[cfg(test)]
#[path = "text_encoding_replacement_tests.rs"]
mod replacement_tests;

use super::{
    binary_data::Kind as BinaryKind, Builtin, Declaration, Fault, Host, NativeGetter, Ordinary,
    Runtime, Value,
};

use crate::text_codec::{DecodeError, Decoder, Encoding};

#[derive(Clone, Copy)]
pub(super) enum Data {
    Encoder,
    Decoder(Decoder),
}
impl Data {
    pub(super) fn tag(&self) -> &'static str {
        match self {
            Self::Encoder => "TextEncoder",
            Self::Decoder(_) => "TextDecoder",
        }
    }
}
#[derive(Clone, Copy)]
pub(super) enum Field {
    EncoderEncoding,
    DecoderEncoding,
    Fatal,
    IgnoreBom,
}
#[derive(Clone, Copy)]
pub(super) enum Op {
    Constructor(bool),
    Getter(Field),
    Encode,
    EncodeInto,
    Decode,
}
impl Op {
    pub(super) fn arity(self) -> usize {
        if matches!(self, Self::EncodeInto) {
            2
        } else {
            0
        }
    }
}
#[derive(Default)]
pub(super) struct Roots {
    encoder: Option<usize>,
    decoder: Option<usize>,
}

impl Roots {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(super::function_reachability::Edge)) {
        for id in [self.encoder, self.decoder].into_iter().flatten() { add(super::function_reachability::Edge::Object(id)); }
    }
}

impl Runtime {
    pub(super) fn initialize_text_encoding(&mut self) -> Result<(), Fault> {
        for (name, encoder) in [("TextEncoder", true), ("TextDecoder", false)] {
            let prototype = self.new_object(None)?;
            if encoder {
                self.text_roots.encoder = Some(prototype);
            } else {
                self.text_roots.decoder = Some(prototype);
            }
            let constructor =
                self.native_function(Builtin::TextEncoding(Op::Constructor(encoder)), name)?;
            self.put_binding(self.global_environment, name.into(), Some(constructor), Declaration::Var, false)?;
            self.constructor_prototype(name, Value::Object(prototype))?;
            let fields: &[(&str, Field)] = if encoder {
                &[("encoding", Field::EncoderEncoding)]
            } else {
                &[
                    ("encoding", Field::DecoderEncoding),
                    ("fatal", Field::Fatal),
                    ("ignoreBOM", Field::IgnoreBom),
                ]
            };
            for (name, field) in fields {
                self.native_accessor(
                    prototype,
                    name,
                    Builtin::TextEncoding(Op::Getter(*field)),
                    NativeGetter::TextEncoding(*field),
                )?;
                self.objects[prototype]
                    .properties
                    .get_mut(*name)
                    .unwrap()
                    .enumerable = true;
            }
            let methods: &[(&str, Op)] = if encoder {
                &[("encode", Op::Encode), ("encodeInto", Op::EncodeInto)]
            } else {
                &[("decode", Op::Decode)]
            };
            for (name, operation) in methods {
                self.prototype_method(prototype, name, Builtin::TextEncoding(*operation))?;
                self.objects[prototype]
                    .properties
                    .get_mut(*name)
                    .unwrap()
                    .enumerable = true;
            }
        }
        Ok(())
    }
    fn text_receiver(&self, this: &Value, encoder: bool) -> Result<usize, Fault> {
        let Value::Object(id) = this else {
            return Err(Fault::exception(
                "TypeError",
                "Text codec method has an incompatible receiver.",
            ));
        };
        if self
            .object(*id)?
            .text
            .is_none_or(|data| matches!(data, Data::Encoder) != encoder)
        {
            return Err(Fault::exception(
                "TypeError",
                "Text codec method has an incompatible receiver.",
            ));
        }
        Ok(*id)
    }
    pub(super) fn text_getter(&mut self, field: Field, this: Value) -> Result<Value, Fault> {
        self.step()?;
        let id = self.text_receiver(&this, matches!(field, Field::EncoderEncoding))?;
        if matches!(field, Field::EncoderEncoding) {
            return self.checked_value(Value::String("utf-8".into()));
        }
        let Some(Data::Decoder(decoder)) = self.objects[id].text else {
            unreachable!()
        };
        Ok(match field {
            Field::DecoderEncoding => Value::String(decoder.encoding.name().into()),
            Field::Fatal => Value::Bool(decoder.fatal),
            Field::IgnoreBom => Value::Bool(decoder.ignore_bom),
            _ => unreachable!(),
        })
    }
    fn text_string(&mut self, input: &Value, host: &mut dyn Host) -> Result<String, Fault> {
        let input = self.copy_value(input)?;
        let input = self.primitive_with_hint(input, true, host)?;
        self.work(input.bytes())?;
        let text = input.script_text()?;
        self.work(text.len())?;
        Ok(text)
    }
    fn text_options(
        &mut self,
        input: Option<&Value>,
        names: &[&str],
        host: &mut dyn Host,
    ) -> Result<Vec<bool>, Fault> {
        let Some(input) = input.filter(|input| !matches!(input, Value::Null | Value::Undefined))
        else {
            return Ok(vec![false; names.len()]);
        };
        let object = self.ordinary_required(input)?;
        names
            .iter()
            .map(|name| {
                self.get_ordinary_property_with_host(object, name, host)
                    .map(|value| value.truthy())
            })
            .collect()
    }
    pub(super) fn construct_text_encoding(
        &mut self,
        encoder: bool,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.check_native_call_stack("Function call")?;
        self.call_depth += 1;
        let result = (|| {
            self.ensure_global_intrinsics("TextEncoder")?;
            let data = if encoder {
                Data::Encoder
            } else {
                let label = if args
                    .first()
                    .is_none_or(|arg| matches!(arg, Value::Undefined))
                {
                    "utf-8".into()
                } else {
                    self.text_string(&args[0], host)?
                };
                // WebIDL dictionary members are converted before constructor
                // steps validate the already-converted encoding label.
                let options = self.text_options(args.get(1), &["fatal", "ignoreBOM"], host)?;
                let encoding = Encoding::label(&label)
                    .filter(|encoding| *encoding != Encoding::REPLACEMENT)
                    .ok_or_else(|| {
                        Fault::exception("RangeError", "TextDecoder does not support this encoding label.")
                    })?;
                Data::Decoder(Decoder::new(encoding, options[0], options[1]))
            };
            let id = self.new_object(None)?;
            self.objects[id].text = Some(data);
            self.objects[id].prototype = if encoder {
                self.text_roots.encoder
            } else {
                self.text_roots.decoder
            }
            .map(Ordinary::Object);
            Ok(Value::Object(id))
        })();
        self.call_depth -= 1;
        result
    }
    pub(super) fn text_encoding_builtin(
        &mut self,
        operation: Op,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.check_native_call_stack("Function call")?;
        self.call_depth += 1;
        let result = self.text_encoding_inner(operation, args, this, host);
        self.call_depth -= 1;
        result
    }
    fn text_encoding_inner(
        &mut self,
        operation: Op,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match operation {
            Op::Constructor(_) => Err(Fault::exception(
                "TypeError",
                "Text codec constructors require new.",
            )),
            Op::Getter(field) => self.text_getter(field, this),
            Op::Encode | Op::EncodeInto => {
                self.text_receiver(&this, true)?;
                if matches!(operation, Op::EncodeInto) && args.len() < 2 {
                    return Err(Fault::exception(
                        "TypeError",
                        "encodeInto requires a source and destination.",
                    ));
                }
                let text = if matches!(operation, Op::Encode)
                    && args
                        .first()
                        .is_none_or(|arg| matches!(arg, Value::Undefined))
                {
                    String::new()
                } else {
                    self.text_string(args.first().unwrap_or(&Value::Undefined), host)?
                };
                if matches!(operation, Op::Encode) {
                    let result = self.new_typed(BinaryKind::U8, text.len())?;
                    let view = self.typed_view(result).unwrap();
                    self.work(text.len())?;
                    self.buffer_bytes_mut(view.buffer)?
                        .copy_from_slice(text.as_bytes());
                    return Ok(Value::Object(result));
                }
                let Some(Value::Object(destination)) = args.get(1) else {
                    return Err(Fault::exception(
                        "TypeError",
                        "encodeInto destination must be a Uint8Array.",
                    ));
                };
                let view = self
                    .typed_view(*destination)
                    .filter(|view| view.kind == BinaryKind::U8)
                    .ok_or_else(|| {
                        Fault::exception(
                            "TypeError",
                            "encodeInto destination must be a Uint8Array.",
                        )
                    })?;
                let mut read = 0;
                let mut written = 0;
                for scalar in text.chars() {
                    if scalar.len_utf8() > view.length - written {
                        break;
                    }
                    self.work(scalar.len_utf8())?;
                    let mut bytes = [0; 4];
                    let bytes = scalar.encode_utf8(&mut bytes).as_bytes();
                    self.buffer_bytes_mut(view.buffer)?
                        [view.offset + written..view.offset + written + bytes.len()]
                        .copy_from_slice(bytes);
                    written += bytes.len();
                    read += scalar.len_utf16();
                }
                let result = self.new_object(None)?;
                self.set_property(result, "read".into(), Value::Number(read as f64))?;
                self.set_property(result, "written".into(), Value::Number(written as f64))?;
                Ok(Value::Object(result))
            }
            Op::Decode => {
                let id = self.text_receiver(&this, false)?;
                let range = if args
                    .first()
                    .is_none_or(|arg| matches!(arg, Value::Undefined))
                {
                    None
                } else {
                    Some(self.binary_source_range(&args[0])?)
                };
                let stream = self.text_options(args.get(1), &["stream"], host)?[0];
                let Some(Data::Decoder(mut decoder)) = self.objects[id].text else {
                    unreachable!()
                };
                if !decoder.streaming {
                    decoder.reset();
                }
                decoder.streaming = stream;
                let length = range.map_or(0, |(_, _, length)| length);
                self.work(length + decoder.pending_len)?;
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(length + decoder.pending_len)
                    .map_err(|_| Fault::new("Unable to allocate bounded text decode input."))?;
                bytes.extend_from_slice(&decoder.pending[..decoder.pending_len]);
                if let Some((buffer, offset, length)) = range {
                    bytes.extend_from_slice(&self.buffer_bytes(buffer)?[offset..offset + length]);
                }
                decoder.pending_len = 0;
                let result = self.decode_text_bytes(&mut decoder, &bytes, stream);
                if result.is_err() {
                    // No output is returned and the unread local input is
                    // discarded. A later job must start a fresh decoder even
                    // when this refusal came from output/work admission.
                    decoder.streaming = false;
                    decoder.pending_len = 0;
                }
                self.objects[id].text = Some(Data::Decoder(decoder));
                result.map(Value::String)
            }
        }
    }
    fn decode_text_bytes(
        &mut self,
        decoder: &mut Decoder,
        bytes: &[u8],
        stream: bool,
    ) -> Result<String, Fault> {
        let mut output = String::new();
        decoder.decode_to(bytes, stream, |text| self.append_string(&mut output, text))
            .map_err(|error| match error {
                DecodeError::Invalid => Fault::exception(
                    "TypeError", "The encoded input is not valid for this TextDecoder.",
                ),
                DecodeError::State => Fault::new("Text decoder fragment exceeded its fixed state."),
                DecodeError::Sink(error) => error,
            })?;
        Ok(output)
    }
}

impl Runtime {
    /// Resolve Encoding's complete label set for native request algorithms.
    /// Unknown labels remain distinct from the recognized replacement decoder;
    /// the caller, rather than the codec, owns its default-encoding policy.
    pub(crate) fn request_text_encoding_label(label: &str) -> Option<&'static str> {
        Encoding::label(label).map(Encoding::name)
    }

    /// Original decoder shared with document-owned request adapters. This does
    /// not expose a new JS API or bypass the active job's work/string budgets.
    pub(crate) fn decode_request_text(&mut self, bytes: &[u8], label: &str) -> Result<String, String> {
        let encoding = Encoding::bom(bytes).or_else(|| Encoding::label(label))
            .ok_or_else(|| format!("Request text encoding {label} is not implemented."))?;
        self.work(bytes.len()).map_err(Fault::display)?;
        let mut decoder = Decoder::new(encoding, false, false);
        self.decode_text_bytes(&mut decoder, bytes, false).map_err(Fault::display)
    }
}

impl Runtime {
    /// Fetch Body always decodes UTF-8 (with its BOM), never XHR MIME sniffing.
    pub(super) fn decode_body_utf8(&mut self,bytes:&[u8])->Result<String,Fault> {
        self.work(bytes.len())?;
        let mut decoder = Decoder::new(Encoding::UTF8, false, false);
        self.decode_text_bytes(&mut decoder,bytes,false)
    }
}
