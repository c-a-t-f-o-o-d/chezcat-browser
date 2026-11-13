//! Live class token lists over real HTML Elements. No second token cache or
//! independent mutation authority; raw attribute edits are immediately visible.
//! https://dom.spec.whatwg.org/#interface-domtokenlist
use super::*;
use std::collections::BTreeSet;

pub(super) const BASE: u32 = 0x8000_0000;
const END: u32 = BASE + STYLE_BASE;
pub(super) fn element(handle: u32) -> Option<u32> {
    (BASE..END)
        .contains(&handle)
        .then(|| handle - BASE)
        .filter(|handle| node_index(*handle).is_some())
}
pub(super) fn handle(element: u32) -> u32 {
    BASE + element
}
fn whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}')
}
pub(super) fn method(name: &str) -> Option<&'static str> {
    match name {
        "remove" => Some("remove"),
        "contains" => Some("contains"),
        "item" => Some("item"),
        "toString" => Some("toString"),
        _ => None,
    }
}

impl Bridge<'_> {
    pub(super) fn class_list_element_receiver(&self, object: u32) -> bool {
        self.attr_element_receiver(object)
            && node_index(object)
                .is_some_and(|node| self.doc.nodes[node].namespace == crate::html::Namespace::Html)
    }
    pub(super) fn class_list_receiver(&self, object: u32) -> bool {
        element(object).is_some_and(|element| self.class_list_element_receiver(element))
    }
    fn class_list_node(&mut self, object: u32) -> Result<usize, String> {
        let element = element(object).ok_or("Invalid class token list.")?;
        let node = self.node(element)?;
        if self.doc.nodes[node].namespace != crate::html::Namespace::Html {
            return Err("Class token list requires known attribute namespace provenance.".into());
        }
        Ok(node)
    }
    fn class_value(&mut self, node: usize) -> Result<Value, String> {
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else {
            unreachable!()
        };
        self.state.work(attrs.len())?;
        self.state
            .copies(attrs.iter().map(|(name, _)| name.len()).sum())?;
        let raw = self.doc.attr(node, "class").unwrap_or("");
        if raw.len() > self.state.limits.host.string_bytes {
            return Err("Class attribute exceeds the host string bound.".into());
        }
        self.state.copies(raw.len())?;
        Ok(Value::String(raw.to_owned()))
    }
    fn class_tokens(&mut self, node: usize) -> Result<Vec<String>, String> {
        let Value::String(raw) = self.class_value(node)? else {
            unreachable!()
        };
        self.state.work(raw.len())?;
        self.state.copies(raw.len())?;
        let mut seen = BTreeSet::new();
        let mut result = Vec::new();
        for token in raw.split(whitespace).filter(|token| !token.is_empty()) {
            if seen.insert(token) {
                result.push(token.to_owned());
            }
        }
        Ok(result)
    }
    pub(super) fn class_list_validate(
        &self,
        object: u32,
        method: &str,
        args: &[Value],
    ) -> Result<(), (&'static str, String)> {
        if !self.class_list_receiver(object) {
            return Err((
                "TypeError",
                "DOMTokenList requires a real Element token list.".into(),
            ));
        }
        if method == "remove" {
            for value in args {
                let Value::String(token) = value else {
                    return Err((
                        "TypeError",
                        "DOMTokenList requires converted DOMString arguments.".into(),
                    ));
                };
                if token.is_empty() {
                    return Err(("SyntaxError", "Class token must not be empty.".into()));
                }
                if token.chars().any(whitespace) {
                    return Err((
                        "InvalidCharacterError",
                        "Class token contains ASCII whitespace.".into(),
                    ));
                }
            }
        }
        Ok(())
    }
    pub(super) fn class_list_get(&mut self, object: u32, property: &str) -> Result<Value, String> {
        let node = self.class_list_node(object)?;
        if property == "value" {
            return self.class_value(node);
        }
        if property == "length" || collection_index(property).is_some() {
            let tokens = self.class_tokens(node)?;
            return if property == "length" {
                Ok(Value::Number(tokens.len() as f64))
            } else {
                Ok(collection_index(property)
                    .and_then(|index| tokens.get(index))
                    .map_or(Value::Undefined, |token| Value::String(token.clone())))
            };
        }
        match property {
            "add" | "toggle" | "replace" | "supports" | "entries" | "keys" | "values"
            | "forEach" => Err(format!(
                "DOMTokenList operation {property:?} is not implemented."
            )),
            // Ordinary absent properties can inherit from Object.prototype.
            // Missing standard token operations above remain explicit failures.
            _ => Ok(Value::Undefined),
        }
    }
    pub(super) fn class_list_set(
        &mut self,
        object: u32,
        property: &str,
        value: Value,
    ) -> Result<(), String> {
        let node = self.class_list_node(object)?;
        if property != "value" {
            return Err("DOMTokenList property write is not implemented.".into());
        }
        let value = self.text(&value)?;
        self.attribute(node, "class", Some(value))
    }
    pub(super) fn class_list_call(
        &mut self,
        object: u32,
        method: &str,
        args: &[Value],
    ) -> Result<Value, String> {
        let node = self.class_list_node(object)?;
        self.class_list_validate(object, method, args)
            .map_err(|(_, message)| message)?;
        if method == "toString" {
            return self.class_value(node);
        }
        let mut tokens = self.class_tokens(node)?;
        match method {
            "remove" => {
                let removed: BTreeSet<&str> = args
                    .iter()
                    .map(|value| match value {
                        Value::String(token) => token.as_str(),
                        _ => unreachable!(),
                    })
                    .collect();
                tokens.retain(|token| !removed.contains(token.as_str()));
                if !tokens.is_empty() || self.doc.attr(node, "class").is_some() {
                    let length = tokens.iter().map(String::len).sum::<usize>()
                        + tokens.len().saturating_sub(1);
                    self.state.copies(length)?;
                    self.attribute(node, "class", Some(tokens.join(" ")))?;
                }
                Ok(Value::Undefined)
            }
            "contains" => {
                let token = self.text(args.first().ok_or("contains requires a token.")?)?;
                Ok(Value::Bool(tokens.contains(&token)))
            }
            "item" => {
                let value = args.first().ok_or("item requires an index.")?.to_number();
                let index = if value.is_finite() {
                    value.trunc().rem_euclid(4_294_967_296.0) as usize
                } else {
                    0
                };
                Ok(tokens
                    .get(index)
                    .map_or(Value::Null, |token| Value::String(token.clone())))
            }
            _ => Err(format!(
                "DOMTokenList method {method:?} is not implemented."
            )),
        }
    }
    pub(super) fn class_list_keys(&mut self, object: u32) -> Result<Vec<String>, String> {
        let node = self.class_list_node(object)?;
        let tokens = self.class_tokens(node)?;
        self.state.work(tokens.len())?;
        self.state.copies(
            tokens
                .len()
                .checked_mul(20)
                .ok_or("Class token index accounting overflow.")?,
        )?;
        Ok((0..tokens.len()).map(|index| index.to_string()).collect())
    }
}
