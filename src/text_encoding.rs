//! Original bounded text/binary conversion. UTF-8, UTF-16LE/BE and Windows-1252
//! decoding includes streaming fragments, BOM handling and fatal errors. No
//! platform codec, network, filesystem or device capability is used here.

use super::{
    binary_data::Kind as BinaryKind, Builtin, Declaration, Fault, Host, NativeGetter, Ordinary,
    Runtime, Value, MAX_CALL_DEPTH,
};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Windows1252,
}
impl Encoding {
    fn name(self) -> &'static str {
        match self {
            Self::Utf8 => "utf-8",
            Self::Utf16Le => "utf-16le",
            Self::Utf16Be => "utf-16be",
            Self::Windows1252 => "windows-1252",
        }
    }
    fn label(label: &str) -> Option<Self> {
        match label {
            "unicode-1-1-utf-8" | "unicode11utf8" | "unicode20utf8" | "utf-8" | "utf8"
            | "x-unicode20utf8" => Some(Self::Utf8),
            "unicodefffe" | "utf-16be" => Some(Self::Utf16Be),
            "csunicode" | "iso-10646-ucs-2" | "ucs-2" | "unicode" | "unicodefeff" | "utf-16"
            | "utf-16le" => Some(Self::Utf16Le),
            "ansi_x3.4-1968" | "ascii" | "cp1252" | "cp819" | "csisolatin1" | "ibm819"
            | "iso-8859-1" | "iso-ir-100" | "iso8859-1" | "iso88591" | "iso_8859-1"
            | "iso_8859-1:1987" | "l1" | "latin1" | "us-ascii" | "windows-1252" | "x-cp1252" => {
                Some(Self::Windows1252)
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy)]
pub(super) struct Decoder {
    encoding: Encoding,
    fatal: bool,
    ignore_bom: bool,
    bom_seen: bool,
    streaming: bool,
    pending: [u8; 4],
    pending_len: usize,
}
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

// The Windows-1252 C1 table is normative code-point data, not implementation
// code. Its five legacy undefined positions decode to their control values.
const WINDOWS_C1: [u16; 32] = [
    0x20ac, 0x0081, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021, 0x02c6, 0x2030, 0x0160, 0x2039,
    0x0152, 0x008d, 0x017d, 0x008f, 0x0090, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014,
    0x02dc, 0x2122, 0x0161, 0x203a, 0x0153, 0x009d, 0x017e, 0x0178,
];

enum Scalar {
    Character(char, usize),
    Invalid(usize),
    Incomplete,
}
fn utf8_scalar(bytes: &[u8], stream: bool) -> Scalar {
    let first = bytes[0];
    if first < 128 {
        return Scalar::Character(char::from(first), 1);
    }
    let (count, mut value) = match first {
        0xc2..=0xdf => (2, u32::from(first & 31)),
        0xe0..=0xef => (3, u32::from(first & 15)),
        0xf0..=0xf4 => (4, u32::from(first & 7)),
        _ => return Scalar::Invalid(1),
    };
    for index in 1..count {
        let Some(&byte) = bytes.get(index) else {
            return if stream {
                Scalar::Incomplete
            } else {
                Scalar::Invalid(bytes.len())
            };
        };
        let (lower, upper) = if index == 1 {
            match first {
                0xe0 => (0xa0, 0xbf),
                0xed => (0x80, 0x9f),
                0xf0 => (0x90, 0xbf),
                0xf4 => (0x80, 0x8f),
                _ => (0x80, 0xbf),
            }
        } else {
            (0x80, 0xbf)
        };
        if byte < lower || byte > upper {
            return Scalar::Invalid(index);
        }
        value = (value << 6) | u32::from(byte & 63);
    }
    Scalar::Character(char::from_u32(value).unwrap(), count)
}
fn utf16_scalar(bytes: &[u8], little: bool, stream: bool) -> Scalar {
    if bytes.len() < 2 {
        return if stream {
            Scalar::Incomplete
        } else {
            Scalar::Invalid(1)
        };
    }
    let unit = |at| {
        if little {
            u16::from_le_bytes([bytes[at], bytes[at + 1]])
        } else {
            u16::from_be_bytes([bytes[at], bytes[at + 1]])
        }
    };
    let first = unit(0);
    if (0xd800..=0xdbff).contains(&first) {
        if bytes.len() < 4 {
            return if stream {
                Scalar::Incomplete
            } else {
                Scalar::Invalid(bytes.len())
            };
        }
        let second = unit(2);
        if !(0xdc00..=0xdfff).contains(&second) {
            return Scalar::Invalid(2);
        }
        let scalar = 0x10000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00);
        Scalar::Character(char::from_u32(scalar).unwrap(), 4)
    } else if (0xdc00..=0xdfff).contains(&first) {
        Scalar::Invalid(2)
    } else {
        Scalar::Character(char::from_u32(u32::from(first)).unwrap(), 2)
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
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Function call limit reached (32)."));
        }
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
                let label = label
                    .trim_matches(|ch| matches!(ch, '\t' | '\n' | '\u{c}' | '\r' | ' '))
                    .to_ascii_lowercase();
                let encoding = Encoding::label(&label).ok_or_else(|| Fault::exception("RangeError",
                    "TextDecoder supports UTF-8, UTF-16LE/BE and Windows-1252 labels in this subset."))?;
                Data::Decoder(Decoder {
                    encoding,
                    fatal: options[0],
                    ignore_bom: options[1],
                    bom_seen: false,
                    streaming: false,
                    pending: [0; 4],
                    pending_len: 0,
                })
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
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Function call limit reached (32)."));
        }
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
                    decoder.pending_len = 0;
                    decoder.bom_seen = false;
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
        let mut at = 0;
        let mut output = String::new();
        while at < bytes.len() {
            let scalar = match decoder.encoding {
                Encoding::Utf8 => utf8_scalar(&bytes[at..], stream),
                Encoding::Utf16Le | Encoding::Utf16Be => {
                    utf16_scalar(&bytes[at..], decoder.encoding == Encoding::Utf16Le, stream)
                }
                Encoding::Windows1252 => {
                    let byte = bytes[at];
                    let value = if (128..160).contains(&byte) {
                        WINDOWS_C1[usize::from(byte - 128)]
                    } else {
                        u16::from(byte)
                    };
                    Scalar::Character(char::from_u32(u32::from(value)).unwrap(), 1)
                }
            };
            let (character, consumed) = match scalar {
                Scalar::Incomplete => {
                    let remaining = &bytes[at..];
                    if remaining.len() > decoder.pending.len() {
                        return Err(Fault::new(
                            "Text decoder fragment exceeded its fixed state.",
                        ));
                    }
                    decoder.pending[..remaining.len()].copy_from_slice(remaining);
                    decoder.pending_len = remaining.len();
                    break;
                }
                Scalar::Character(character, consumed) => (character, consumed),
                Scalar::Invalid(consumed) => {
                    if decoder.fatal {
                        decoder.pending_len = 0;
                        decoder.streaming = false;
                        return Err(Fault::exception(
                            "TypeError",
                            "The encoded input is not valid for this TextDecoder.",
                        ));
                    }
                    ('\u{fffd}', consumed)
                }
            };
            at += consumed;
            if decoder.encoding != Encoding::Windows1252 && !decoder.ignore_bom && !decoder.bom_seen
            {
                decoder.bom_seen = true;
                if character == '\u{feff}' {
                    continue;
                }
            }
            let mut encoded = [0; 4];
            self.append_string(&mut output, character.encode_utf8(&mut encoded))?;
        }
        Ok(output)
    }
}
