//! Checked output for HTML fragment serialization. Stay in UTF-8 until a
//! source contributes raw DOMString units, then retain the exact UTF-16.
use super::*;
use crate::dom_string::DomStringRef;

pub(in crate::scripting) enum Markup {
    Scalar(String),
    Units(Vec<u16>),
}

impl Markup {
    pub(in crate::scripting) fn new() -> Self { Self::Scalar(String::new()) }

    fn admit(state: &mut State, current: usize, extra: usize) -> Result<(), String> {
        if current.checked_add(extra).is_none_or(|n| n > state.limits.host.string_bytes) {
            return Err("HTML serialization exceeds the host string limit.".into());
        }
        state.copies(extra)
    }

    pub(in crate::scripting) fn push(&mut self, state: &mut State, text: &str) -> Result<(), String> {
        self.append(state, DomStringRef::Scalar(text))
    }

    pub(in crate::scripting) fn append(&mut self, state: &mut State, value: DomStringRef<'_>) -> Result<(), String> {
        if value.is_empty() { return Ok(()); }
        match (self, value) {
            (Self::Scalar(out), DomStringRef::Scalar(text)) => {
                Self::admit(state, out.len(), text.len())?;
                if text.len() > out.capacity() - out.len() { state.copies(out.len())?; }
                out.try_reserve(text.len()).map_err(|_| "Cannot reserve HTML serialization.")?;
                out.push_str(text);
            }
            (output @ Self::Scalar(_), DomStringRef::Raw(raw)) => {
                let Self::Scalar(prefix) = output else { unreachable!() };
                state.work(prefix.len())?;
                let count = prefix.encode_utf16().count().checked_add(raw.len()).ok_or("HTML serialization size overflow.")?;
                Self::admit(state, 0, count.checked_mul(2).ok_or("HTML serialization size overflow.")?)?;
                let mut units = Vec::new();
                units.try_reserve_exact(count).map_err(|_| "Cannot reserve HTML serialization units.")?;
                units.extend(prefix.encode_utf16());
                units.extend_from_slice(raw);
                *output = Self::Units(units);
            }
            (Self::Units(out), text) => {
                state.work(text.logical_bytes())?;
                let count = match text {
                    DomStringRef::Scalar(s) => s.encode_utf16().count(),
                    DomStringRef::Raw(u) => u.len(),
                };
                Self::admit(state, out.len() * 2, count.checked_mul(2).ok_or("HTML serialization size overflow.")?)?;
                if count > out.capacity() - out.len() { state.copies(out.len() * 2)?; }
                out.try_reserve(count).map_err(|_| "Cannot reserve HTML serialization units.")?;
                out.extend(text.code_units());
            }
        }
        Ok(())
    }

    pub(in crate::scripting) fn escaped(&mut self, state: &mut State, value: DomStringRef<'_>, attribute: bool, raw: bool) -> Result<(), String> {
        state.work(value.logical_bytes())?;
        if raw { return self.append(state, value); }
        let entity = |ch: u32| match ch {
            38 => Some("&amp;"), 160 => Some("&nbsp;"), 60 => Some("&lt;"), 62 => Some("&gt;"),
            34 if attribute => Some("&quot;"), _ => None,
        };
        let mut start = 0;
        match value {
            DomStringRef::Scalar(s) => {
                for (index, ch) in s.char_indices() {
                    if let Some(replacement) = entity(ch as u32) {
                        self.push(state, &s[start..index])?;
                        self.push(state, replacement)?;
                        start = index + ch.len_utf8();
                    }
                }
                self.push(state, &s[start..])
            }
            DomStringRef::Raw(units) => {
                for (index, &unit) in units.iter().enumerate() {
                    if let Some(replacement) = entity(u32::from(unit)) {
                        self.append(state, DomStringRef::Raw(&units[start..index]))?;
                        self.push(state, replacement)?;
                        start = index + 1;
                    }
                }
                self.append(state, DomStringRef::Raw(&units[start..]))
            }
        }
    }

    pub(in crate::scripting) fn finish(self, state: &mut State) -> Result<Value, String> {
        match self {
            Self::Scalar(text) => Ok(Value::String(text)),
            Self::Units(units) => {
                state.work(units.len())?;
                Value::from_host_string_units(units, state.limits.host.string_bytes, |bytes| state.copies(bytes))
            }
        }
    }

    pub(in crate::scripting) fn wrapped(&mut self, state: &mut State, open: &str, data: &DomString, close: &str) -> Result<(), String> {
        self.push(state, open)?;
        self.append(state, data.view())?;
        self.push(state, close)
    }

    pub(in crate::scripting) fn processing_instruction(&mut self, state: &mut State, target: &str, data: &DomString) -> Result<(), String> {
        self.push(state, "<?")?;
        self.push(state, target)?;
        self.wrapped(state, " ", data, "?>")
    }

    pub(in crate::scripting) fn open_tag(&mut self, state: &mut State, local: DomStringRef<'_>, attrs: &[crate::html::Attribute]) -> Result<(), String> {
        self.push(state, "<")?;
        self.append(state, local)?;
        state.work(attrs.len())?;
        for attr in attrs {
            self.push(state, " ")?;
            let prefix = match attr.name.namespace_uri().and_then(|uri| uri.scalar().ok()) {
                Some("http://www.w3.org/XML/1998/namespace") => Some("xml:"),
                Some("http://www.w3.org/2000/xmlns/") if attr.name.local() != "xmlns" => Some("xmlns:"),
                Some("http://www.w3.org/2000/xmlns/") => Some(""),
                Some("http://www.w3.org/1999/xlink") => Some("xlink:"),
                _ => None,
            };
            if let Some(prefix) = prefix {
                self.push(state, prefix)?;
                self.append(state, attr.name.local().view())?;
            } else {
                self.append(state, attr.name.qualified.view())?;
            }
            self.push(state, "=\"")?;
            self.escaped(state, attr.value.view(), true, false)?;
            self.push(state, "\"")?;
        }
        self.push(state, ">")
    }

    pub(in crate::scripting) fn close_tag(&mut self, state: &mut State, local: DomStringRef<'_>) -> Result<(), String> {
        self.push(state, "</")?;
        self.append(state, local)?;
        self.push(state, ">")
    }
}
