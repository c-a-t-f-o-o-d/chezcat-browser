//! Original fixed-size ArrayBuffer, numeric typed arrays and DataView support.
//! Buffers are arena-owned bytes; views retain a buffer identity and a checked
//! range, never raw pointers. All allocations and bulk work have explicit caps.
//! Shared/resizable/detachable buffers, BigInt elements and species callbacks
//! are outside this first binary-data subset.

use super::{
    int32, number_text, string_number, Builtin, Declaration, Fault, Function, Host, Ordinary,
    Runtime, Value, MAX_CALL_DEPTH,
};
use std::cmp::Ordering;

const MAX_BUFFER_BYTES: usize = 4 * 1024 * 1024;
const MAX_RETAINED_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub(super) enum Kind {
    I8,
    U8,
    U8Clamped,
    I16,
    U16,
    I32,
    U32,
    F16,
    F32,
    F64,
}

const KINDS: [Kind; 10] = [
    Kind::I8,
    Kind::U8,
    Kind::U8Clamped,
    Kind::I16,
    Kind::U16,
    Kind::I32,
    Kind::U32,
    Kind::F16,
    Kind::F32,
    Kind::F64,
];

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::I8 => "Int8Array",
            Self::U8 => "Uint8Array",
            Self::U8Clamped => "Uint8ClampedArray",
            Self::I16 => "Int16Array",
            Self::U16 => "Uint16Array",
            Self::I32 => "Int32Array",
            Self::U32 => "Uint32Array",
            Self::F16 => "Float16Array",
            Self::F32 => "Float32Array",
            Self::F64 => "Float64Array",
        }
    }
    fn size(self) -> usize {
        match self {
            Self::I8 | Self::U8 | Self::U8Clamped => 1,
            Self::I16 | Self::U16 | Self::F16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::F64 => 8,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct TypedView {
    pub buffer: usize,
    pub offset: usize,
    pub length: usize,
    pub kind: Kind,
}
#[derive(Clone, Copy)]
pub(super) struct View {
    buffer: usize,
    offset: usize,
    length: usize,
}
pub(super) enum Data {
    Buffer(Vec<u8>),
    Typed(TypedView),
    View(View),
}
impl Data {
    pub(super) fn tag(&self) -> &'static str {
        match self {
            Self::Buffer(_) => "ArrayBuffer",
            Self::Typed(view) => view.kind.name(),
            Self::View(_) => "DataView",
        }
    }
}

#[derive(Default)]
pub(super) struct Roots {
    buffer: Option<usize>,
    view: Option<usize>,
    typed: [Option<usize>; 10],
    buffer_constructor: Option<usize>,
    constructors: [Option<usize>; 10],
}

#[derive(Clone, Copy)]
pub(super) enum Constructor {
    Buffer,
    View,
    Typed(Kind),
    AbstractTyped,
}
#[derive(Clone, Copy)]
pub(super) enum Field {
    BufferLength,
    BufferMaxLength,
    BufferResizable,
    BufferDetached,
    TypedLength,
    TypedByteLength,
    TypedOffset,
    TypedBuffer,
    ViewLength,
    ViewOffset,
    ViewBuffer,
}
#[derive(Clone, Copy)]
pub(super) enum Op {
    Constructor(Constructor),
    Getter(Field),
    BufferMethod(&'static str),
    TypedMethod(&'static str),
    TypedStatic(&'static str),
    ViewMethod(&'static str),
    IsView,
    NumberMethod(&'static str),
}
impl Op {
    pub(super) fn arity(self) -> usize {
        match self {
            Self::Constructor(Constructor::View) => 1,
            Self::Constructor(Constructor::Typed(_)) => 3,
            Self::Constructor(Constructor::AbstractTyped) | Self::Getter(_) => 0,
            Self::Constructor(_) | Self::IsView | Self::NumberMethod(_) => 1,
            Self::BufferMethod("slice") | Self::TypedMethod("slice" | "subarray" | "with") => 2,
            Self::TypedMethod("copyWithin") => 2,
            Self::TypedMethod(
                "reverse" | "toReversed" | "toString" | "keys" | "values" | "entries",
            )
            | Self::TypedStatic("of") => 0,
            Self::ViewMethod(name) if name.starts_with("set") => 2,
            _ => 1,
        }
    }
}

fn index_number(name: &str) -> Option<Option<usize>> {
    if name == "-0" {
        return Some(None);
    }
    let number = string_number(name);
    if number_text(number) != name {
        return None;
    }
    Some(
        (number.is_finite()
            && number >= 0.0
            && number.fract() == 0.0
            && number < usize::MAX as f64)
            .then_some(number as usize),
    )
}

fn encode(kind: Kind, value: f64) -> [u8; 8] {
    let mut result = [0; 8];
    match kind {
        Kind::I8 | Kind::U8 => result[0] = int32(value) as u8,
        Kind::U8Clamped => {
            result[0] = if value.is_nan() || value <= 0.0 {
                0
            } else if value >= 255.0 {
                255
            } else {
                value.round_ties_even() as u8
            }
        }
        Kind::I16 | Kind::U16 => result[..2].copy_from_slice(&(int32(value) as u16).to_le_bytes()),
        Kind::I32 | Kind::U32 => result[..4].copy_from_slice(&(int32(value) as u32).to_le_bytes()),
        Kind::F16 => result[..2].copy_from_slice(&encode_half(value).to_le_bytes()),
        Kind::F32 => result[..4].copy_from_slice(&(value as f32).to_le_bytes()),
        Kind::F64 => result.copy_from_slice(&value.to_le_bytes()),
    }
    result
}

fn decode(kind: Kind, bytes: &[u8]) -> f64 {
    match kind {
        Kind::I8 => f64::from(bytes[0] as i8),
        Kind::U8 | Kind::U8Clamped => f64::from(bytes[0]),
        Kind::I16 => f64::from(i16::from_le_bytes(bytes.try_into().unwrap())),
        Kind::U16 => f64::from(u16::from_le_bytes(bytes.try_into().unwrap())),
        Kind::I32 => f64::from(i32::from_le_bytes(bytes.try_into().unwrap())),
        Kind::U32 => f64::from(u32::from_le_bytes(bytes.try_into().unwrap())),
        Kind::F16 => decode_half(u16::from_le_bytes(bytes.try_into().unwrap())),
        Kind::F32 => f64::from(f32::from_le_bytes(bytes.try_into().unwrap())),
        Kind::F64 => f64::from_le_bytes(bytes.try_into().unwrap()),
    }
}

fn encode_half(value: f64) -> u16 {
    let sign = if value.is_sign_negative() { 0x8000 } else { 0 };
    let value = value.abs();
    if value.is_nan() {
        return sign | 0x7e00;
    }
    if value >= 65520.0 {
        return sign | 0x7c00;
    }
    if value < 0.00006103515625 {
        return sign | (value * 16777216.0).round_ties_even() as u16;
    }
    let mut exponent = ((value.to_bits() >> 52) & 0x7ff) as i32 - 1023;
    let mut significand = (value * 2.0f64.powi(10 - exponent)).round_ties_even() as u16;
    if significand == 2048 {
        exponent += 1;
        significand = 1024;
    }
    sign | (((exponent + 15) as u16) << 10) | (significand - 1024)
}
fn decode_half(bits: u16) -> f64 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = (bits >> 10) & 31;
    let fraction = bits & 1023;
    let value = match exponent {
        0 => f64::from(fraction) / 16777216.0,
        31 => {
            if fraction == 0 {
                f64::INFINITY
            } else {
                f64::NAN
            }
        }
        _ => f64::from(1024 + fraction) * 2.0f64.powi(i32::from(exponent) - 25),
    };
    sign * value
}

impl Runtime {
    fn binary_method_property(
        &mut self,
        owner: usize,
        name: &'static str,
        operation: Op,
    ) -> Result<(), Fault> {
        self.prototype_method(owner, name, Builtin::Binary(operation))
    }
    fn binary_accessor(&mut self, owner: usize, name: &str, field: Field) -> Result<(), Fault> {
        self.native_accessor(
            owner,
            name,
            Builtin::Binary(Op::Getter(field)),
            super::NativeGetter::Binary(field),
        )
    }
    fn binary_constructor(
        &mut self,
        name: &'static str,
        kind: Constructor,
        prototype: usize,
    ) -> Result<usize, Fault> {
        let value = self.native_function(Builtin::Binary(Op::Constructor(kind)), name)?;
        let Value::Function(function) = value else {
            unreachable!()
        };
        self.put_binding(0, name.into(), Some(value), Declaration::Var, false)?;
        self.constructor_prototype(name, Value::Object(prototype))?;
        Ok(function)
    }
    pub(super) fn initialize_binary(&mut self) -> Result<(), Fault> {
        let buffer = self.new_object(None)?;
        self.binary_roots.buffer = Some(buffer);
        let constructor = self.binary_constructor("ArrayBuffer", Constructor::Buffer, buffer)?;
        self.binary_roots.buffer_constructor = Some(constructor);
        let properties = self.ordinary_id(Ordinary::Function(constructor))?;
        self.binary_method_property(properties, "isView", Op::IsView)?;
        self.binary_method_property(buffer, "slice", Op::BufferMethod("slice"))?;
        for (name, field) in [
            ("byteLength", Field::BufferLength),
            ("maxByteLength", Field::BufferMaxLength),
            ("resizable", Field::BufferResizable),
            ("detached", Field::BufferDetached),
        ] {
            self.binary_accessor(buffer, name, field)?;
        }
        let typed = self.new_object(None)?;
        let abstract_constructor = self.native_function(
            Builtin::Binary(Op::Constructor(Constructor::AbstractTyped)),
            "TypedArray",
        )?;
        let Value::Function(abstract_constructor) = abstract_constructor else {
            unreachable!()
        };
        let properties = self.ordinary_id(Ordinary::Function(abstract_constructor))?;
        self.store_property(
            properties,
            "prototype".into(),
            Value::Object(typed),
            Some((false, false, false)),
        )?;
        self.store_property(
            typed,
            "constructor".into(),
            Value::Function(abstract_constructor),
            Some((false, true, true)),
        )?;
        for name in ["from", "of"] {
            self.binary_method_property(properties, name, Op::TypedStatic(name))?;
        }
        for (name, field) in [
            ("length", Field::TypedLength),
            ("byteLength", Field::TypedByteLength),
            ("byteOffset", Field::TypedOffset),
            ("buffer", Field::TypedBuffer),
        ] {
            self.binary_accessor(typed, name, field)?;
        }
        for name in [
            "at",
            "set",
            "subarray",
            "slice",
            "fill",
            "copyWithin",
            "reverse",
            "sort",
            "join",
            "indexOf",
            "lastIndexOf",
            "includes",
            "forEach",
            "map",
            "filter",
            "every",
            "some",
            "find",
            "findIndex",
            "findLast",
            "findLastIndex",
            "reduce",
            "reduceRight",
            "toReversed",
            "toSorted",
            "with",
            "keys",
            "values",
            "entries",
        ] {
            self.binary_method_property(typed, name, Op::TypedMethod(name))?;
        }
        let values = self.get_property(typed, "values")?;
        self.store_symbol_property(
            typed,
            self.symbol_iterator.unwrap(),
            values,
            Some((false, true, true)),
        )?;
        let to_string = self.get_property(self.array_prototype.unwrap(), "toString")?;
        self.store_property(
            typed,
            "toString".into(),
            to_string,
            Some((false, true, true)),
        )?;
        for kind in KINDS {
            let prototype = self.new_object(None)?;
            self.objects[prototype].prototype = Some(Ordinary::Object(typed));
            self.binary_roots.typed[kind as usize] = Some(prototype);
            let constructor =
                self.binary_constructor(kind.name(), Constructor::Typed(kind), prototype)?;
            self.binary_roots.constructors[kind as usize] = Some(constructor);
            let properties = self.ordinary_id(Ordinary::Function(constructor))?;
            self.objects[properties].prototype = Some(Ordinary::Function(abstract_constructor));
            for object in [prototype, properties] {
                self.store_property(
                    object,
                    "BYTES_PER_ELEMENT".into(),
                    Value::Number(kind.size() as f64),
                    Some((false, false, false)),
                )?;
            }
        }
        let view = self.new_object(None)?;
        self.binary_roots.view = Some(view);
        self.binary_constructor("DataView", Constructor::View, view)?;
        for (name, field) in [
            ("byteLength", Field::ViewLength),
            ("byteOffset", Field::ViewOffset),
            ("buffer", Field::ViewBuffer),
        ] {
            self.binary_accessor(view, name, field)?;
        }
        for name in [
            "getInt8",
            "getUint8",
            "getInt16",
            "getUint16",
            "getInt32",
            "getUint32",
            "getFloat16",
            "getFloat32",
            "getFloat64",
            "setInt8",
            "setUint8",
            "setInt16",
            "setUint16",
            "setInt32",
            "setUint32",
            "setFloat16",
            "setFloat32",
            "setFloat64",
        ] {
            self.binary_method_property(view, name, Op::ViewMethod(name))?;
        }
        let number = self.binding_value(0, "Number")?;
        let properties = self.ordinary_id(Ordinary::from_value(&number).unwrap())?;
        for (name, value) in [
            ("EPSILON", f64::EPSILON),
            ("MAX_SAFE_INTEGER", 9007199254740991.0),
            ("MIN_SAFE_INTEGER", -9007199254740991.0),
            ("MAX_VALUE", f64::MAX),
            ("MIN_VALUE", f64::from_bits(1)),
            ("NaN", f64::NAN),
            ("POSITIVE_INFINITY", f64::INFINITY),
            ("NEGATIVE_INFINITY", f64::NEG_INFINITY),
        ] {
            self.store_property(
                properties,
                name.into(),
                Value::Number(value),
                Some((false, false, false)),
            )?;
        }
        for name in ["isFinite", "isNaN", "isInteger", "isSafeInteger"] {
            self.binary_method_property(properties, name, Op::NumberMethod(name))?;
        }
        for (name, global) in [("parseInt", "parseInt"), ("parseFloat", "parseFloat")] {
            let value = self.binding_value(0, global)?;
            self.store_property(properties, name.into(), value, Some((false, true, true)))?;
        }
        Ok(())
    }

    pub(super) fn typed_view(&self, id: usize) -> Option<TypedView> {
        match self.objects.get(id)?.binary.as_ref()? {
            Data::Typed(view) => Some(*view),
            _ => None,
        }
    }
    pub(super) fn typed_numeric_key(
        &self,
        id: usize,
        name: &str,
    ) -> Result<Option<Option<usize>>, Fault> {
        self.object(id)?;
        Ok(if self.typed_view(id).is_some() {
            index_number(name)
        } else {
            None
        })
    }
    pub(super) fn typed_index_valid(&self, id: usize, index: Option<usize>) -> bool {
        index.is_some_and(|index| self.typed_view(id).is_some_and(|view| index < view.length))
    }
    pub(super) fn own_enumerable(&self, id: usize, name: &str) -> Result<bool, Fault> {
        if self.string_object_index(id, name).is_some() {
            return Ok(true);
        }
        if let Some(index) = self.typed_numeric_key(id, name)? {
            return Ok(self.typed_index_valid(id, index));
        }
        Ok(self
            .object(id)?
            .properties
            .get(name)
            .is_some_and(|property| property.enumerable))
    }
    pub(super) fn typed_index_value(
        &mut self,
        id: usize,
        index: Option<usize>,
    ) -> Result<Value, Fault> {
        let view = self
            .typed_view(id)
            .ok_or_else(|| Fault::new("Invalid typed-array handle."))?;
        if let Some(index) = index.filter(|index| *index < view.length) {
            self.typed_read(view, index).map(Value::Number)
        } else {
            Ok(Value::Undefined)
        }
    }
    pub(super) fn binary_source_range(
        &self,
        input: &Value,
    ) -> Result<(usize, usize, usize), Fault> {
        let Value::Object(id) = input else {
            return Err(Fault::exception(
                "TypeError",
                "A buffer or buffer view is required.",
            ));
        };
        match self.object(*id)?.binary.as_ref() {
            Some(Data::Buffer(bytes)) => Ok((*id, 0, bytes.len())),
            Some(Data::Typed(view)) => {
                Ok((view.buffer, view.offset, view.length * view.kind.size()))
            }
            Some(Data::View(view)) => Ok((view.buffer, view.offset, view.length)),
            _ => Err(Fault::exception(
                "TypeError",
                "A buffer or buffer view is required.",
            )),
        }
    }
    pub(super) fn buffer_bytes(&self, id: usize) -> Result<&[u8], Fault> {
        match self.object(id)?.binary.as_ref() {
            Some(Data::Buffer(bytes)) => Ok(bytes),
            _ => Err(Fault::exception("TypeError", "An ArrayBuffer is required.")),
        }
    }
    pub(super) fn buffer_bytes_mut(&mut self, id: usize) -> Result<&mut [u8], Fault> {
        match self
            .objects
            .get_mut(id)
            .and_then(|object| object.binary.as_mut())
        {
            Some(Data::Buffer(bytes)) => Ok(bytes),
            _ => Err(Fault::exception("TypeError", "An ArrayBuffer is required.")),
        }
    }
    pub(super) fn typed_read(&mut self, view: TypedView, index: usize) -> Result<f64, Fault> {
        self.step()?;
        if index >= view.length {
            return Err(Fault::new("Typed-array index escaped its checked view."));
        }
        let start = view.offset + index * view.kind.size();
        Ok(decode(
            view.kind,
            &self.buffer_bytes(view.buffer)?[start..start + view.kind.size()],
        ))
    }
    pub(super) fn typed_write(
        &mut self,
        view: TypedView,
        index: usize,
        value: f64,
    ) -> Result<(), Fault> {
        self.step()?;
        if index >= view.length {
            return Err(Fault::new("Typed-array index escaped its checked view."));
        }
        let bytes = encode(view.kind, value);
        let start = view.offset + index * view.kind.size();
        self.buffer_bytes_mut(view.buffer)?[start..start + view.kind.size()]
            .copy_from_slice(&bytes[..view.kind.size()]);
        Ok(())
    }
    fn new_buffer(&mut self, length: usize) -> Result<usize, Fault> {
        if length > MAX_BUFFER_BYTES {
            return Err(Fault::new("ArrayBuffer byte limit reached (4 MiB)."));
        }
        if self.retained_binary.saturating_add(length) > MAX_RETAINED_BYTES {
            return Err(Fault::new("Retained binary data limit reached (16 MiB)."));
        }
        self.work(length)?;
        let id = self.new_object(None)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Fault::new("Unable to allocate bounded ArrayBuffer."))?;
        bytes.resize(length, 0);
        self.objects[id].binary = Some(Data::Buffer(bytes));
        self.objects[id].prototype = self.binary_roots.buffer.map(Ordinary::Object);
        self.retained_binary += length;
        Ok(id)
    }
    fn new_typed_view(
        &mut self,
        kind: Kind,
        buffer: usize,
        offset: usize,
        length: usize,
    ) -> Result<usize, Fault> {
        let size = length
            .checked_mul(kind.size())
            .ok_or_else(|| Fault::exception("RangeError", "Typed-array length overflow."))?;
        let available = self.buffer_bytes(buffer)?.len();
        if !offset.is_multiple_of(kind.size())
            || offset.checked_add(size).is_none_or(|end| end > available)
        {
            return Err(Fault::exception(
                "RangeError",
                "Typed-array view exceeds or misaligns its ArrayBuffer.",
            ));
        }
        let id = self.new_object(None)?;
        self.objects[id].binary = Some(Data::Typed(TypedView {
            buffer,
            offset,
            length,
            kind,
        }));
        self.objects[id].prototype = self.binary_roots.typed[kind as usize].map(Ordinary::Object);
        Ok(id)
    }
    pub(super) fn new_typed(&mut self, kind: Kind, length: usize) -> Result<usize, Fault> {
        let bytes = length
            .checked_mul(kind.size())
            .ok_or_else(|| Fault::exception("RangeError", "Typed-array length overflow."))?;
        let buffer = self.new_buffer(bytes)?;
        self.new_typed_view(kind, buffer, 0, length)
    }
    pub(super) fn binary_getter(&mut self, field: Field, receiver: Value) -> Result<Value, Fault> {
        self.step()?;
        let Value::Object(id) = receiver else {
            return Err(Fault::exception(
                "TypeError",
                "Binary-data getter has an incompatible receiver.",
            ));
        };
        let data = self.object(id)?.binary.as_ref().ok_or_else(|| {
            Fault::exception(
                "TypeError",
                "Binary-data getter has an incompatible receiver.",
            )
        })?;
        let value = match (field, data) {
            (Field::BufferLength | Field::BufferMaxLength, Data::Buffer(bytes)) => {
                Value::Number(bytes.len() as f64)
            }
            (Field::BufferResizable | Field::BufferDetached, Data::Buffer(_)) => Value::Bool(false),
            (Field::TypedLength, Data::Typed(view)) => Value::Number(view.length as f64),
            (Field::TypedByteLength, Data::Typed(view)) => {
                Value::Number((view.length * view.kind.size()) as f64)
            }
            (Field::TypedOffset, Data::Typed(view)) => Value::Number(view.offset as f64),
            (Field::TypedBuffer, Data::Typed(view)) => Value::Object(view.buffer),
            (Field::ViewLength, Data::View(view)) => Value::Number(view.length as f64),
            (Field::ViewOffset, Data::View(view)) => Value::Number(view.offset as f64),
            (Field::ViewBuffer, Data::View(view)) => Value::Object(view.buffer),
            _ => {
                return Err(Fault::exception(
                    "TypeError",
                    "Binary-data getter has an incompatible receiver.",
                ))
            }
        };
        Ok(value)
    }
    pub(super) fn binary_number(
        &mut self,
        value: &Value,
        host: &mut dyn Host,
    ) -> Result<f64, Fault> {
        self.step()?;
        self.work(value.bytes())?;
        let value = self.copy_value(value)?;
        let primitive = self.primitive_with_hint(value, false, host)?;
        self.work(primitive.bytes())?;
        primitive.script_number()
    }
    fn binary_index(&mut self, value: Option<&Value>, host: &mut dyn Host) -> Result<usize, Fault> {
        let number = self.binary_number(value.unwrap_or(&Value::Undefined), host)?;
        let integer = if number.is_nan() { 0.0 } else { number.trunc() };
        if !(0.0..=9007199254740991.0).contains(&integer) {
            return Err(Fault::exception(
                "RangeError",
                "Binary-data index is outside the valid integer range.",
            ));
        }
        Ok(integer as usize)
    }
    pub(super) fn binary_integer(
        &mut self,
        value: Option<&Value>,
        default: f64,
        host: &mut dyn Host,
    ) -> Result<f64, Fault> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(default);
        };
        let value = self.binary_number(value, host)?;
        Ok(if value.is_nan() { 0.0 } else { value.trunc() })
    }
    pub(super) fn binary_relative(
        &mut self,
        value: Option<&Value>,
        length: usize,
        default: usize,
        host: &mut dyn Host,
    ) -> Result<usize, Fault> {
        let index = self.binary_integer(value, default as f64, host)?;
        Ok(if index < 0.0 {
            (length as f64 + index).max(0.0)
        } else {
            index.min(length as f64)
        } as usize)
    }
    pub(super) fn binary_source_length(
        &mut self,
        source: &Value,
        host: &mut dyn Host,
    ) -> Result<usize, Fault> {
        let length = match source {
            Value::Object(id) => self.get_property_with_host(*id, "length", host)?,
            Value::Function(id) => {
                self.get_ordinary_property_with_host(Ordinary::Function(*id), "length", host)?
            }
            Value::String(text) => {
                self.work(text.len())?;
                return Ok(text.encode_utf16().count());
            }
            Value::Null | Value::Undefined => {
                return Err(Fault::exception(
                    "TypeError",
                    "Cannot read binary-data input from null or undefined.",
                ))
            }
            Value::Host(_) => {
                return Err(Fault::new(
                    "Native host binary-data input is not implemented.",
                ))
            }
            _ => Value::Undefined,
        };
        let number = self.binary_number(&length, host)?;
        Ok(if number.is_nan() || number <= 0.0 {
            0
        } else {
            number.trunc().min(9007199254740991.0) as usize
        })
    }
    fn binary_source_item(
        &mut self,
        source: &Value,
        index: usize,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        match source {
            Value::Object(id) => self.get_property_with_host(*id, &index.to_string(), host),
            Value::Function(id) => self.get_ordinary_property_with_host(
                Ordinary::Function(*id),
                &index.to_string(),
                host,
            ),
            Value::String(text) => {
                self.work(text.len().saturating_mul(2))?;
                let unit = text.encode_utf16().nth(index).unwrap();
                self.units_string(&[unit]).map(Value::String)
            }
            _ => Ok(Value::Undefined),
        }
    }
    fn copy_binary_bytes(
        &mut self,
        target: usize,
        target_start: usize,
        source: usize,
        source_start: usize,
        length: usize,
    ) -> Result<(), Fault> {
        self.work(length)?;
        if target == source {
            self.buffer_bytes_mut(target)?
                .copy_within(source_start..source_start + length, target_start);
        } else {
            // Distinct arena slots cannot alias. Splitting the arena avoids a
            // second buffer allocation and makes full-size copies linear work.
            let (target_object, source_object) = if target < source {
                let (before, after) = self.objects.split_at_mut(source);
                (&mut before[target], &after[0])
            } else {
                let (before, after) = self.objects.split_at_mut(target);
                (&mut after[0], &before[source])
            };
            let Some(Data::Buffer(target)) = target_object.binary.as_mut() else {
                return Err(Fault::new("Invalid binary-copy destination."));
            };
            let Some(Data::Buffer(source)) = source_object.binary.as_ref() else {
                return Err(Fault::new("Invalid binary-copy source."));
            };
            target[target_start..target_start + length]
                .copy_from_slice(&source[source_start..source_start + length]);
        }
        Ok(())
    }
    pub(super) fn construct_binary(
        &mut self,
        constructor: Constructor,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Function call limit reached (32)."));
        }
        self.call_depth += 1;
        let result = self.construct_binary_inner(constructor, args, host);
        self.call_depth -= 1;
        result
    }
    fn construct_binary_inner(
        &mut self,
        constructor: Constructor,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match constructor {
            Constructor::AbstractTyped => Err(Fault::exception(
                "TypeError",
                "The abstract TypedArray constructor cannot be constructed.",
            )),
            Constructor::Buffer => {
                let length = self.binary_index(args.first(), host)?;
                if let Some(options) = args
                    .get(1)
                    .filter(|value| !matches!(value, Value::Undefined))
                {
                    if let Some(object) = Ordinary::from_value(options) {
                        if !matches!(
                            self.get_ordinary_property_with_host(object, "maxByteLength", host)?,
                            Value::Undefined
                        ) {
                            return Err(Fault::new("Resizable ArrayBuffers are not implemented."));
                        }
                    } else if matches!(options, Value::Host(_)) {
                        return Err(Fault::new(
                            "Native host ArrayBuffer options are not implemented.",
                        ));
                    }
                }
                self.new_buffer(length).map(Value::Object)
            }
            Constructor::View => {
                let Some(Value::Object(buffer)) = args.first() else {
                    return Err(Fault::exception(
                        "TypeError",
                        "DataView requires an ArrayBuffer.",
                    ));
                };
                let buffer = *buffer;
                let available = self.buffer_bytes(buffer)?.len();
                let offset = self.binary_index(args.get(1), host)?;
                if offset > available {
                    return Err(Fault::exception(
                        "RangeError",
                        "DataView offset exceeds its ArrayBuffer.",
                    ));
                }
                let length = if args
                    .get(2)
                    .is_none_or(|value| matches!(value, Value::Undefined))
                {
                    available - offset
                } else {
                    self.binary_index(args.get(2), host)?
                };
                if length > available - offset {
                    return Err(Fault::exception(
                        "RangeError",
                        "DataView length exceeds its ArrayBuffer.",
                    ));
                }
                let id = self.new_object(None)?;
                self.objects[id].binary = Some(Data::View(View {
                    buffer,
                    offset,
                    length,
                }));
                self.objects[id].prototype = self.binary_roots.view.map(Ordinary::Object);
                Ok(Value::Object(id))
            }
            Constructor::Typed(kind) => {
                let first = args.first().unwrap_or(&Value::Undefined);
                if let Value::Object(buffer) = first {
                    if matches!(self.object(*buffer)?.binary, Some(Data::Buffer(_))) {
                        let available = self.buffer_bytes(*buffer)?.len();
                        let offset = self.binary_index(args.get(1), host)?;
                        if !offset.is_multiple_of(kind.size()) || offset > available {
                            return Err(Fault::exception(
                                "RangeError",
                                "Typed-array offset is misaligned or outside its ArrayBuffer.",
                            ));
                        }
                        let length = if args
                            .get(2)
                            .is_none_or(|value| matches!(value, Value::Undefined))
                        {
                            if (available - offset) % kind.size() != 0 {
                                return Err(Fault::exception(
                                    "RangeError",
                                    "ArrayBuffer length is not a multiple of the element size.",
                                ));
                            }
                            (available - offset) / kind.size()
                        } else {
                            self.binary_index(args.get(2), host)?
                        };
                        return self
                            .new_typed_view(kind, *buffer, offset, length)
                            .map(Value::Object);
                    }
                    if let Some(source) = self.typed_view(*buffer) {
                        let result = self.new_typed(kind, source.length)?;
                        let target = self.typed_view(result).unwrap();
                        if source.kind == kind {
                            self.copy_binary_bytes(
                                target.buffer,
                                0,
                                source.buffer,
                                source.offset,
                                source.length * kind.size(),
                            )?;
                        } else {
                            for index in 0..source.length {
                                let number = self.typed_read(source, index)?;
                                self.typed_write(target, index, number)?;
                            }
                        }
                        return Ok(Value::Object(result));
                    }
                }
                if matches!(
                    first,
                    Value::Object(_) | Value::Function(_) | Value::Host(_)
                ) {
                    let items = self.collect_native_iterable(first, host)?;
                    let length = if let Some(items) = &items {
                        items.len()
                    } else {
                        self.binary_source_length(first, host)?
                    };
                    let result = self.new_typed(kind, length)?;
                    let target = self.typed_view(result).unwrap();
                    for index in 0..length {
                        let value = if let Some(items) = &items {
                            self.copy_value(&items[index])?
                        } else {
                            self.binary_source_item(first, index, host)?
                        };
                        let number = self.binary_number(&value, host)?;
                        self.typed_write(target, index, number)?;
                    }
                    Ok(Value::Object(result))
                } else {
                    let length = self.binary_index(Some(first), host)?;
                    self.new_typed(kind, length).map(Value::Object)
                }
            }
        }
    }
    pub(super) fn binary_builtin(
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
        let result = self.binary_builtin_inner(operation, args, this, host);
        self.call_depth -= 1;
        result
    }
    fn binary_builtin_inner(
        &mut self,
        operation: Op,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match operation {
            Op::Constructor(_) => Err(Fault::exception(
                "TypeError",
                "Binary-data constructors require new.",
            )),
            Op::Getter(field) => self.binary_getter(field, this),
            Op::IsView => Ok(Value::Bool(
                matches!(args.first(), Some(Value::Object(id)) if matches!(self.object(*id)?.binary, Some(Data::Typed(_) | Data::View(_)))),
            )),
            Op::BufferMethod(method) => self.buffer_method(method, this, args, host),
            Op::TypedMethod(method) => self.typed_method(method, this, args, host),
            Op::TypedStatic(method) => self.typed_static(method, this, args, host),
            Op::ViewMethod(method) => self.view_method(method, this, args, host),
            Op::NumberMethod(method) => {
                let value = args.first().unwrap_or(&Value::Undefined);
                let Value::Number(number) = value else {
                    return Ok(Value::Bool(false));
                };
                Ok(Value::Bool(match method {
                    "isFinite" => number.is_finite(),
                    "isNaN" => number.is_nan(),
                    "isInteger" => number.is_finite() && number.fract() == 0.0,
                    "isSafeInteger" => {
                        number.is_finite()
                            && number.fract() == 0.0
                            && number.abs() <= 9007199254740991.0
                    }
                    _ => return Err(Fault::new("Unsupported Number method.")),
                }))
            }
        }
    }
    fn checked_binary_species(
        &mut self,
        id: usize,
        expected: usize,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        let constructor = self.get_property_with_host(id, "constructor", host)?;
        if matches!(constructor, Value::Undefined)
            || matches!(constructor, Value::Function(function) if function == expected)
        {
            return Ok(());
        }
        if !matches!(
            constructor,
            Value::Object(_) | Value::Function(_) | Value::Host(_)
        ) {
            return Err(Fault::exception(
                "TypeError",
                "Binary-data constructor property is not an object.",
            ));
        }
        Err(Fault::new(
            "Custom binary-data species constructors are not implemented.",
        ))
    }
    fn buffer_method(
        &mut self,
        method: &str,
        this: Value,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let Value::Object(id) = this else {
            return Err(Fault::exception(
                "TypeError",
                "ArrayBuffer method requires an ArrayBuffer.",
            ));
        };
        let length = self.buffer_bytes(id)?.len();
        if method != "slice" {
            return Err(Fault::new("Unsupported ArrayBuffer method."));
        }
        let start = self.binary_relative(args.first(), length, 0, host)?;
        let end = self.binary_relative(args.get(1), length, length, host)?;
        self.checked_binary_species(id, self.binary_roots.buffer_constructor.unwrap(), host)?;
        let count = end.saturating_sub(start);
        let result = self.new_buffer(count)?;
        self.copy_binary_bytes(result, 0, id, start, count)?;
        Ok(Value::Object(result))
    }
    fn typed_receiver(&self, this: &Value) -> Result<(usize, TypedView), Fault> {
        let Value::Object(id) = this else {
            return Err(Fault::exception(
                "TypeError",
                "Typed-array method requires a typed array.",
            ));
        };
        Ok((
            *id,
            self.typed_view(*id).ok_or_else(|| {
                Fault::exception("TypeError", "Typed-array method requires a typed array.")
            })?,
        ))
    }
    fn typed_method(
        &mut self,
        method: &str,
        this: Value,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let (id, view) = self.typed_receiver(&this)?;
        let length = view.length;
        match method {
            "keys" | "values" | "entries" => {
                let mode = match method {
                    "keys" => super::collections::Mode::Keys,
                    "values" => super::collections::Mode::Values,
                    _ => super::collections::Mode::Entries,
                };
                self.new_native_iterator(id, super::collections::Family::Array, mode)
                    .map(Value::Object)
            }
            "at" => {
                let index = self.binary_integer(args.first(), 0.0, host)?;
                let index = if index < 0.0 {
                    length as f64 + index
                } else {
                    index
                };
                if index < 0.0 || index >= length as f64 {
                    Ok(Value::Undefined)
                } else {
                    self.typed_read(view, index as usize).map(Value::Number)
                }
            }
            "slice" | "subarray" => {
                let start = self.binary_relative(args.first(), length, 0, host)?;
                let end = self.binary_relative(args.get(1), length, length, host)?;
                let count = end.saturating_sub(start);
                self.checked_binary_species(
                    id,
                    self.binary_roots.constructors[view.kind as usize].unwrap(),
                    host,
                )?;
                if method == "subarray" {
                    self.new_typed_view(
                        view.kind,
                        view.buffer,
                        view.offset + start * view.kind.size(),
                        count,
                    )
                    .map(Value::Object)
                } else {
                    let result = self.new_typed(view.kind, count)?;
                    let target = self.typed_view(result).unwrap();
                    self.copy_binary_bytes(
                        target.buffer,
                        0,
                        view.buffer,
                        view.offset + start * view.kind.size(),
                        count * view.kind.size(),
                    )?;
                    Ok(Value::Object(result))
                }
            }
            "set" => {
                let offset = self.binary_integer(args.get(1), 0.0, host)?;
                if offset < 0.0 || offset == f64::INFINITY {
                    return Err(Fault::exception(
                        "RangeError",
                        "Typed-array set offset is outside its view.",
                    ));
                }
                let source = args.first().unwrap_or(&Value::Undefined);
                let source_view = if let Value::Object(source) = source {
                    self.typed_view(*source)
                } else {
                    None
                };
                let count = if let Some(source) = source_view {
                    source.length
                } else {
                    self.binary_source_length(source, host)?
                };
                if offset > length as f64 || count > length - offset as usize {
                    return Err(Fault::exception(
                        "RangeError",
                        "Typed-array set source exceeds its destination.",
                    ));
                }
                let offset = offset as usize;
                if let Value::Object(source) = source {
                    if let Some(source_view) = self.typed_view(*source) {
                        if source_view.kind == view.kind {
                            self.copy_binary_bytes(
                                view.buffer,
                                view.offset + offset * view.kind.size(),
                                source_view.buffer,
                                source_view.offset,
                                count * view.kind.size(),
                            )?;
                            return Ok(Value::Undefined);
                        }
                        if source_view.buffer == view.buffer {
                            self.work(count.saturating_mul(8))?;
                            let mut values = Vec::with_capacity(count);
                            for index in 0..count {
                                values.push(self.typed_read(source_view, index)?);
                            }
                            for (index, number) in values.into_iter().enumerate() {
                                self.typed_write(view, offset + index, number)?;
                            }
                            return Ok(Value::Undefined);
                        }
                    }
                }
                for index in 0..count {
                    let item = self.binary_source_item(source, index, host)?;
                    let number = self.binary_number(&item, host)?;
                    self.typed_write(view, offset + index, number)?;
                }
                Ok(Value::Undefined)
            }
            "fill" => {
                let number = self.binary_number(args.first().unwrap_or(&Value::Undefined), host)?;
                let start = self.binary_relative(args.get(1), length, 0, host)?;
                let end = self.binary_relative(args.get(2), length, length, host)?;
                let bytes = encode(view.kind, number);
                self.work(end.saturating_sub(start).saturating_mul(view.kind.size()))?;
                for chunk in self.buffer_bytes_mut(view.buffer)?[view.offset
                    + start * view.kind.size()
                    ..view.offset + end.max(start) * view.kind.size()]
                    .chunks_exact_mut(view.kind.size())
                {
                    chunk.copy_from_slice(&bytes[..view.kind.size()]);
                }
                Ok(this)
            }
            "copyWithin" => {
                let target = self.binary_relative(args.first(), length, 0, host)?;
                let start = self.binary_relative(args.get(1), length, 0, host)?;
                let end = self.binary_relative(args.get(2), length, length, host)?;
                let count = end.saturating_sub(start).min(length - target);
                self.copy_binary_bytes(
                    view.buffer,
                    view.offset + target * view.kind.size(),
                    view.buffer,
                    view.offset + start * view.kind.size(),
                    count * view.kind.size(),
                )?;
                Ok(this)
            }
            "reverse" | "toReversed" | "toSorted" => {
                let (result, target) = if method == "reverse" {
                    (this.clone(), view)
                } else {
                    let result = self.new_typed(view.kind, length)?;
                    let target = self.typed_view(result).unwrap();
                    self.copy_binary_bytes(
                        target.buffer,
                        0,
                        view.buffer,
                        view.offset,
                        length * view.kind.size(),
                    )?;
                    (Value::Object(result), target)
                };
                if method == "toSorted" {
                    return self.typed_method("sort", result, args, host);
                }
                self.work(length.saturating_mul(view.kind.size()))?;
                let bytes = self.buffer_bytes_mut(target.buffer)?;
                for index in 0..length / 2 {
                    for byte in 0..view.kind.size() {
                        bytes.swap(
                            target.offset + index * view.kind.size() + byte,
                            target.offset + (length - 1 - index) * view.kind.size() + byte,
                        );
                    }
                }
                Ok(result)
            }
            "with" => {
                let index = self.binary_integer(args.first(), 0.0, host)?;
                let number = self.binary_number(args.get(1).unwrap_or(&Value::Undefined), host)?;
                let index = if index < 0.0 {
                    length as f64 + index
                } else {
                    index
                };
                if index < 0.0 || index >= length as f64 {
                    return Err(Fault::exception(
                        "RangeError",
                        "Typed-array with index is outside its view.",
                    ));
                }
                let result = self.new_typed(view.kind, length)?;
                let target = self.typed_view(result).unwrap();
                self.copy_binary_bytes(
                    target.buffer,
                    0,
                    view.buffer,
                    view.offset,
                    length * view.kind.size(),
                )?;
                self.typed_write(target, index as usize, number)?;
                Ok(Value::Object(result))
            }
            "sort" => {
                if let Some(comparator) = args
                    .first()
                    .filter(|value| !matches!(value, Value::Undefined))
                {
                    if !matches!(comparator, Value::Function(_)) {
                        return Err(Fault::exception(
                            "TypeError",
                            "Typed-array comparator must be callable.",
                        ));
                    }
                    self.work(length.saturating_mul(std::mem::size_of::<Value>()))?;
                    let mut values = Vec::with_capacity(length);
                    for index in 0..length {
                        values.push(Value::Number(self.typed_read(view, index)?));
                    }
                    self.sort_native_values(&mut values, Some(comparator), true, host)?;
                    for (index, value) in values.into_iter().enumerate() {
                        let Value::Number(number) = value else {
                            unreachable!()
                        };
                        self.typed_write(view, index, number)?;
                    }
                    return Ok(this);
                }
                self.work(length.saturating_mul(8))?;
                let comparisons =
                    length.saturating_mul((usize::BITS - length.max(1).leading_zeros()) as usize);
                for _ in 0..comparisons {
                    self.step()?;
                }
                let mut values = Vec::with_capacity(length);
                for index in 0..length {
                    values.push(self.typed_read(view, index)?);
                }
                values.sort_by(|a, b| {
                    if a.is_nan() {
                        if b.is_nan() {
                            Ordering::Equal
                        } else {
                            Ordering::Greater
                        }
                    } else if b.is_nan() {
                        Ordering::Less
                    } else if *a == 0.0 && *b == 0.0 {
                        b.is_sign_negative().cmp(&a.is_sign_negative())
                    } else {
                        a.partial_cmp(b).unwrap()
                    }
                });
                for (index, value) in values.into_iter().enumerate() {
                    self.typed_write(view, index, value)?;
                }
                Ok(this)
            }
            "join" => {
                let separator = if args
                    .first()
                    .is_none_or(|value| matches!(value, Value::Undefined))
                {
                    ",".into()
                } else {
                    let value = self.copy_value(&args[0])?;
                    let value = self.primitive_with_hint(value, true, host)?;
                    self.work(value.bytes())?;
                    value.script_text()?
                };
                let mut result = String::new();
                for index in 0..length {
                    if index > 0 {
                        self.append_string(&mut result, &separator)?;
                    }
                    let number = self.typed_read(view, index)?;
                    self.append_string(&mut result, &number_text(number))?;
                }
                self.checked_value(Value::String(result))
            }
            "indexOf" | "includes" | "lastIndexOf" => {
                if length == 0 {
                    return Ok(if method == "includes" {
                        Value::Bool(false)
                    } else {
                        Value::Number(-1.0)
                    });
                }
                let needle = args.first().unwrap_or(&Value::Undefined);
                let start = if method == "lastIndexOf" && args.len() < 2 {
                    length as f64 - 1.0
                } else {
                    self.binary_integer(args.get(1), 0.0, host)?
                };
                let start = if start < 0.0 {
                    length as f64 + start
                } else {
                    start
                };
                if method == "lastIndexOf" {
                    if start >= 0.0 && length != 0 {
                        for index in (0..=start.min(length as f64 - 1.0) as usize).rev() {
                            let value = self.typed_read(view, index)?;
                            if matches!(needle, Value::Number(number) if *number == value) {
                                return Ok(Value::Number(index as f64));
                            }
                        }
                    }
                } else if start < length as f64 {
                    for index in start.max(0.0) as usize..length {
                        let value = self.typed_read(view, index)?;
                        if matches!(needle, Value::Number(number) if *number == value || method == "includes" && number.is_nan() && value.is_nan())
                        {
                            return Ok(if method == "includes" {
                                Value::Bool(true)
                            } else {
                                Value::Number(index as f64)
                            });
                        }
                    }
                }
                Ok(if method == "includes" {
                    Value::Bool(false)
                } else {
                    Value::Number(-1.0)
                })
            }
            "forEach" | "map" | "filter" | "every" | "some" | "find" | "findIndex" | "findLast"
            | "findLastIndex" | "reduce" | "reduceRight" => {
                self.typed_callback(method, id, view, args, host)
            }
            _ => Err(Fault::new("Unsupported typed-array method.")),
        }
    }
    fn typed_callback(
        &mut self,
        method: &str,
        id: usize,
        view: TypedView,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let callback = args.first().unwrap_or(&Value::Undefined);
        if !matches!(callback, Value::Function(_)) {
            return Err(Fault::exception(
                "TypeError",
                "Typed-array callback must be callable.",
            ));
        }
        let receiver = self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?;
        let reduce = matches!(method, "reduce" | "reduceRight");
        let reverse = matches!(method, "findLast" | "findLastIndex" | "reduceRight");
        let result = if method == "map" {
            self.checked_binary_species(
                id,
                self.binary_roots.constructors[view.kind as usize].unwrap(),
                host,
            )?;
            Some(self.new_typed(view.kind, view.length)?)
        } else {
            None
        };
        let mut kept = Vec::new();
        let mut accumulator = if reduce { args.get(1).cloned() } else { None };
        for next in 0..view.length {
            let index = if reverse {
                view.length - 1 - next
            } else {
                next
            };
            let number = self.typed_read(view, index)?;
            if reduce && accumulator.is_none() {
                accumulator = Some(Value::Number(number));
                continue;
            }
            let values = if reduce {
                vec![
                    self.copy_value(accumulator.as_ref().unwrap())?,
                    Value::Number(number),
                    Value::Number(index as f64),
                    Value::Object(id),
                ]
            } else {
                vec![
                    Value::Number(number),
                    Value::Number(index as f64),
                    Value::Object(id),
                ]
            };
            let output = self.call_value(
                callback.clone(),
                &values,
                if reduce {
                    Value::Undefined
                } else {
                    receiver.clone()
                },
                host,
            )?;
            match method {
                "map" => {
                    let number = self.binary_number(&output, host)?;
                    self.typed_write(self.typed_view(result.unwrap()).unwrap(), index, number)?;
                }
                "filter" if output.truthy() => {
                    self.work(8)?;
                    kept.push(number);
                }
                "some" if output.truthy() => return Ok(Value::Bool(true)),
                "every" if !output.truthy() => return Ok(Value::Bool(false)),
                "find" | "findLast" if output.truthy() => return Ok(Value::Number(number)),
                "findIndex" | "findLastIndex" if output.truthy() => {
                    return Ok(Value::Number(index as f64))
                }
                "reduce" | "reduceRight" => accumulator = Some(output),
                _ => {}
            }
        }
        Ok(match method {
            "map" => Value::Object(result.unwrap()),
            "filter" => {
                self.checked_binary_species(
                    id,
                    self.binary_roots.constructors[view.kind as usize].unwrap(),
                    host,
                )?;
                let result = self.new_typed(view.kind, kept.len())?;
                let target = self.typed_view(result).unwrap();
                for (index, number) in kept.into_iter().enumerate() {
                    self.typed_write(target, index, number)?;
                }
                Value::Object(result)
            }
            "reduce" | "reduceRight" => accumulator.ok_or_else(|| {
                Fault::exception(
                    "TypeError",
                    "Cannot reduce an empty typed array without an initial value.",
                )
            })?,
            "every" => Value::Bool(true),
            "some" => Value::Bool(false),
            "findIndex" | "findLastIndex" => Value::Number(-1.0),
            _ => Value::Undefined,
        })
    }
    fn typed_static(
        &mut self,
        method: &str,
        this: Value,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let Value::Function(function) = this else {
            return Err(Fault::exception(
                "TypeError",
                "Typed-array static method requires a constructor.",
            ));
        };
        let Some(Function::Native(Builtin::Binary(Op::Constructor(Constructor::Typed(kind))))) =
            self.functions.get(function)
        else {
            return Err(Fault::new(
                "Custom typed-array static constructors are not implemented.",
            ));
        };
        let kind = *kind;
        if method == "of" {
            let result = self.new_typed(kind, args.len())?;
            let view = self.typed_view(result).unwrap();
            for (index, value) in args.iter().enumerate() {
                let number = self.binary_number(value, host)?;
                self.typed_write(view, index, number)?;
            }
            return Ok(Value::Object(result));
        }
        let source = args.first().unwrap_or(&Value::Undefined);
        let mapper = args
            .get(1)
            .filter(|value| !matches!(value, Value::Undefined));
        if mapper.is_some_and(|value| !matches!(value, Value::Function(_))) {
            return Err(Fault::exception(
                "TypeError",
                "Typed-array mapping argument must be callable.",
            ));
        }
        // Iterable values are gathered before the first mapping callback.
        // Array-like inputs instead read each index immediately before mapping.
        let characters = self.collect_native_iterable(source, host)?;
        let count = if let Some(characters) = &characters {
            characters.len()
        } else {
            self.binary_source_length(source, host)?
        };
        let result = self.new_typed(kind, count)?;
        let view = self.typed_view(result).unwrap();
        for index in 0..count {
            let value = if let Some(characters) = &characters {
                self.copy_value(&characters[index])?
            } else {
                self.binary_source_item(source, index, host)?
            };
            let value = if let Some(mapper) = mapper {
                let receiver = self.copy_value(args.get(2).unwrap_or(&Value::Undefined))?;
                self.call_value(
                    mapper.clone(),
                    &[value, Value::Number(index as f64)],
                    receiver,
                    host,
                )?
            } else {
                value
            };
            let number = self.binary_number(&value, host)?;
            self.typed_write(view, index, number)?;
        }
        Ok(Value::Object(result))
    }
    fn view_method(
        &mut self,
        method: &str,
        this: Value,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let Value::Object(id) = this else {
            return Err(Fault::exception(
                "TypeError",
                "DataView method requires a DataView.",
            ));
        };
        let Some(Data::View(view)) = self.object(id)?.binary.as_ref() else {
            return Err(Fault::exception(
                "TypeError",
                "DataView method requires a DataView.",
            ));
        };
        let view = *view;
        let write = method.starts_with("set");
        let kind = match &method[3..] {
            "Int8" => Kind::I8,
            "Uint8" => Kind::U8,
            "Int16" => Kind::I16,
            "Uint16" => Kind::U16,
            "Int32" => Kind::I32,
            "Uint32" => Kind::U32,
            "Float16" => Kind::F16,
            "Float32" => Kind::F32,
            "Float64" => Kind::F64,
            _ => return Err(Fault::new("Unsupported DataView operation.")),
        };
        let index = self.binary_index(args.first(), host)?;
        let value = if write {
            Some(self.binary_number(args.get(1).unwrap_or(&Value::Undefined), host)?)
        } else {
            None
        };
        let little = args
            .get(if write { 2 } else { 1 })
            .is_some_and(Value::truthy);
        if index
            .checked_add(kind.size())
            .is_none_or(|end| end > view.length)
        {
            return Err(Fault::exception(
                "RangeError",
                "DataView access exceeds its view.",
            ));
        }
        let start = view.offset + index;
        if let Some(value) = value {
            let mut encoded = encode(kind, value);
            if !little {
                encoded[..kind.size()].reverse();
            }
            self.buffer_bytes_mut(view.buffer)?[start..start + kind.size()]
                .copy_from_slice(&encoded[..kind.size()]);
            Ok(Value::Undefined)
        } else {
            let mut bytes = [0; 8];
            bytes[..kind.size()]
                .copy_from_slice(&self.buffer_bytes(view.buffer)?[start..start + kind.size()]);
            if !little {
                bytes[..kind.size()].reverse();
            }
            Ok(Value::Number(decode(kind, &bytes[..kind.size()])))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_float_roundtrips_every_non_nan_encoding() {
        for bits in 0..=u16::MAX {
            let value = decode_half(bits);
            if value.is_nan() {
                assert!(decode_half(encode_half(value)).is_nan());
            } else {
                assert_eq!(encode_half(value), bits, "half encoding {bits:04x}");
            }
        }
        // A direct f64 conversion must not double-round through f32.
        assert_eq!(encode_half(1.0004882812500002), 0x3c01);
        assert_eq!(encode_half(1.00048828125), 0x3c00);
        assert_eq!(encode_half(65519.99999999), 0x7bff);
        assert_eq!(encode_half(65520.0), 0x7c00);
    }

    #[test]
    fn binary_allocations_have_per_buffer_and_document_caps() {
        let mut runtime = Runtime::for_test();
        let before = runtime.objects.len();
        assert!(runtime
            .new_buffer(MAX_BUFFER_BYTES + 1)
            .unwrap_err()
            .display()
            .contains("4 MiB"));
        assert_eq!(runtime.objects.len(), before);
        assert_eq!(runtime.retained_binary, 0);
        for _ in 0..MAX_RETAINED_BYTES / MAX_BUFFER_BYTES {
            runtime.begin_job();
            runtime.new_buffer(MAX_BUFFER_BYTES).unwrap();
        }
        runtime.begin_job();
        let before = runtime.objects.len();
        assert!(runtime
            .new_buffer(1)
            .unwrap_err()
            .display()
            .contains("16 MiB"));
        assert_eq!(runtime.retained_binary, MAX_RETAINED_BYTES);
        assert_eq!(runtime.objects.len(), before);
    }

    #[test]
    fn checked_views_and_copies_preserve_buffer_boundaries() {
        let mut runtime = Runtime::for_test();
        let source = runtime.new_buffer(16).unwrap();
        let target = runtime.new_buffer(16).unwrap();
        runtime
            .buffer_bytes_mut(source)
            .unwrap()
            .copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
        assert!(runtime.new_typed_view(Kind::U16, source, 1, 1).is_err());
        assert!(runtime.new_typed_view(Kind::U16, source, 14, 2).is_err());
        let ordinary = runtime.new_object(None).unwrap();
        assert!(runtime.new_typed_view(Kind::U8, ordinary, 0, 0).is_err());
        runtime.copy_binary_bytes(target, 3, source, 5, 4).unwrap();
        assert_eq!(
            &runtime.buffer_bytes(target).unwrap()[2..8],
            &[0, 5, 6, 7, 8, 0]
        );
        runtime.copy_binary_bytes(source, 3, target, 3, 4).unwrap();
        runtime.copy_binary_bytes(source, 4, source, 3, 4).unwrap();
        assert_eq!(
            &runtime.buffer_bytes(source).unwrap()[3..8],
            &[5, 5, 6, 7, 8]
        );
        let view = runtime.new_typed_view(Kind::U8, source, 3, 4).unwrap();
        let view = runtime.typed_view(view).unwrap();
        assert!(runtime.typed_write(view, 4, 9.0).is_err());
        assert_eq!(runtime.buffer_bytes(source).unwrap()[7], 8);
    }
}
