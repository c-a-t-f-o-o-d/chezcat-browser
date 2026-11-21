//! Original finite MIME parsing/serialization for native XHR text handling.
use crate::http::script_request::token;
pub(super) struct Mime {
    pub essence: String,
    pub parameters: Vec<(String, String)>,
}
fn copy(text: &str) -> Result<String, String> {
    let mut next = String::new();
    next.try_reserve_exact(text.len())
        .map_err(|_| "Could not reserve MIME text.")?;
    next.push_str(text);
    Ok(next)
}
pub(super) fn parse(source: &str) -> Result<Option<Mime>, String> {
    let source = source.trim_matches([' ', '\t', '\r', '\n']);
    let Some((kind, tail)) = source.split_once('/') else {
        return Ok(None);
    };
    let end = tail.find(';').unwrap_or(tail.len());
    let subtype = tail[..end].trim_end_matches([' ', '\t', '\r', '\n']);
    if !token(kind) || !token(subtype) {
        return Ok(None);
    }
    let mut essence = copy(kind)?;
    essence.make_ascii_lowercase();
    essence
        .try_reserve(1 + subtype.len())
        .map_err(|_| "Could not reserve MIME essence.")?;
    essence.push('/');
    let mut lower = copy(subtype)?;
    lower.make_ascii_lowercase();
    essence.push_str(&lower);
    let mut parameters = Vec::<(String, String)>::new();
    let mut names = std::collections::HashSet::<String>::new();
    let bytes = tail.as_bytes();
    let mut at = end;
    while at < bytes.len() {
        at += 1;
        while at < bytes.len() && matches!(bytes[at], b' ' | b'\t' | b'\r' | b'\n') {
            at += 1;
        }
        let start = at;
        while at < bytes.len() && !matches!(bytes[at], b';' | b'=') {
            at += 1;
        }
        let mut name = copy(&tail[start..at])?;
        name.make_ascii_lowercase();
        if at == bytes.len() {
            break;
        }
        if bytes[at] == b';' {
            continue;
        }
        at += 1;
        let mut value = String::new();
        if at < bytes.len() && bytes[at] == b'"' {
            at += 1;
            while at < bytes.len() {
                if bytes[at] == b'"' {
                    at += 1;
                    break;
                }
                if bytes[at] == b'\\' && at + 1 < bytes.len() {
                    at += 1;
                }
                let character = tail[at..].chars().next().unwrap();
                value
                    .try_reserve(character.len_utf8())
                    .map_err(|_| "Could not reserve MIME parameter.")?;
                value.push(character);
                at += character.len_utf8();
            }
            while at < bytes.len() && bytes[at] != b';' {
                at += 1;
            }
        } else {
            let start = at;
            while at < bytes.len() && bytes[at] != b';' {
                at += 1;
            }
            value = copy(tail[start..at].trim_end_matches([' ', '\t', '\r', '\n']))?;
            if value.is_empty() {
                continue;
            }
        }
        if !token(&name)
            || !value.chars().all(|character| {
                character == '\t'
                    || (' '..='~').contains(&character)
                    || ('\u{80}'..='\u{ff}').contains(&character)
            })
            || names.contains(&name)
        {
            continue;
        }
        parameters
            .try_reserve(1)
            .map_err(|_| "Could not reserve MIME parameter entry.")?;
        names
            .try_reserve(1)
            .map_err(|_| "Could not reserve MIME parameter names.")?;
        names.insert(copy(&name)?);
        parameters.push((name, value));
    }
    Ok(Some(Mime {
        essence,
        parameters,
    }))
}
impl Mime {
    pub(super) fn charset(&self) -> Option<&str> {
        self.parameters
            .iter()
            .find(|(name, _)| name == "charset")
            .map(|(_, value)| value.as_str())
    }
    pub(super) fn force_utf8(&mut self) -> bool {
        if let Some((_, value)) = self
            .parameters
            .iter_mut()
            .find(|(name, _)| name == "charset")
        {
            if !value.eq_ignore_ascii_case("utf-8") {
                *value = "UTF-8".into();
                return true;
            }
        }
        false
    }
    pub(super) fn serialize(&self) -> Result<String, String> {
        let mut text = copy(&self.essence)?;
        for (name, value) in &self.parameters {
            let reserve = name
                .len()
                .checked_add(value.len().saturating_mul(2))
                .and_then(|size| size.checked_add(4))
                .ok_or("MIME serialization overflow.")?;
            text.try_reserve(reserve)
                .map_err(|_| "Could not reserve serialized MIME value.")?;
            text.push(';');
            text.push_str(name);
            text.push('=');
            if token(value) {
                text.push_str(value);
            } else {
                text.push('"');
                for character in value.chars() {
                    if matches!(character, '"' | '\\') {
                        text.push('\\');
                    }
                    text.push(character);
                }
                text.push('"');
            }
        }
        Ok(text)
    }
}
