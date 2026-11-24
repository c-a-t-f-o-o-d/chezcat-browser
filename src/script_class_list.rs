//! Live class token lists over real HTML Elements. No second token cache or
//! independent mutation authority; raw attribute edits are immediately visible.
//! https://dom.spec.whatwg.org/#interface-domtokenlist
use super::*;
use std::collections::BTreeSet;

pub(super) const BASE: u32 = 0x8000_0000;
const END: u32 = BASE + NODE_HANDLES;
const SANDBOX_BASE:u32=0x8100_0000;
const SANDBOX_END:u32=SANDBOX_BASE+NODE_HANDLES;
pub(super) fn attribute(handle:u32)->Option<&'static str> {
    if (BASE..END).contains(&handle){Some("class")}
    else if (SANDBOX_BASE..SANDBOX_END).contains(&handle){Some("sandbox")}
    else {None}
}
pub(super) fn element(handle:u32)->Option<u32> {
    let base=match attribute(handle)? {"class"=>BASE,_=>SANDBOX_BASE};
    let element=node_handle((handle-base)as usize);node_index(element).map(|_|element)
}
pub(super) fn sandbox_handle(element:u32)->u32 {SANDBOX_BASE+(element-NODE_BASE)}
pub(super) fn handle(element: u32) -> u32 {
    BASE + (element - NODE_BASE)
}
fn whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}')
}
pub(super) fn supported_tokens(object:u32)->Option<&'static [&'static str]>{
    (attribute(object)==Some("sandbox")).then_some(script_iframe_sandbox::SUPPORTED_TOKENS)
}
pub(super) fn operation(name: &str) -> Option<&'static str> {
    Some(match name {
        "\0DOMTokenList.get.length"=>"get length",
        "\0DOMTokenList.get.value"=>"get value",
        "\0DOMTokenList.set.value"=>"set value",
        "\0DOMTokenList.item"=>"item",
        "\0DOMTokenList.contains"=>"contains",
        "\0DOMTokenList.add"=>"add",
        "\0DOMTokenList.remove"=>"remove",
        "\0DOMTokenList.supports"=>"supports",
        "\0DOMTokenList.toString"=>"toString",
        _=>return None,
    })
}
pub(super) fn method(name: &str) -> Option<&'static str> {
    match name {
        "add" => Some("add"),
        "remove" => Some("remove"),
        "contains" => Some("contains"),
        "supports" => Some("supports"),
        "item" => Some("item"),
        "toString" => Some("toString"),
        _ => None,
    }
}

impl Bridge<'_> {
    pub(super) fn class_list_element_receiver(&self, object: u32) -> bool {
        self.attr_element_receiver(object)
            && node_index(object)
                .is_some_and(|node| matches!(self.doc.nodes[node].namespace,crate::html::Namespace::Html|crate::html::Namespace::Svg))
    }
    pub(super) fn class_list_receiver(&self, object: u32) -> bool {
        element(object).is_some_and(|element| self.class_list_element_receiver(element) && (attribute(object)!=Some("sandbox") || self.iframe_sandbox_receiver(element)))
    }
    fn class_list_node(&mut self, object: u32) -> Result<usize, String> {
        let element = element(object).ok_or("Invalid class token list.")?;
        let node = self.node(element)?;
        if !matches!(self.doc.nodes[node].namespace,crate::html::Namespace::Html|crate::html::Namespace::Svg) {
            return Err("Class token list requires known attribute namespace provenance.".into());
        }
        if !self.class_list_receiver(object){return Err("Invalid attribute token list receiver.".into());}
        Ok(node)
    }
    fn class_value(&mut self, node: usize, attribute:&str) -> Result<Value, String> {
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else {
            unreachable!()
        };
        self.state.work(attrs.len())?;
        self.state
            .copies(attrs.iter().map(|attr| attr.name.bytes()).sum())?;
        let Some(raw)=self.doc.dom_attr(node,attribute) else{return Ok(Value::String(String::new()));};
        Self::dom_string_value(self.state,raw)
    }
    fn class_tokens(&mut self, node: usize, attribute:&str) -> Result<Vec<String>, String> {
        let Value::String(raw) = self.class_value(node,attribute)? else {
            return Err("Raw UTF-16 token mutation requires exact serialization integration.".into());
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
        if method=="supports" && supported_tokens(object).is_none(){return Err(("TypeError","This token attribute does not define supported tokens.".into()));}
        if matches!(method, "add" | "remove") {
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
        let attribute=attribute(object).ok_or("Invalid token attribute kind.")?;
        if property == "value" {
            return self.class_value(node,attribute);
        }
        if property == "length" {return self.read_class_tokens(node,attribute,token_list_reads::Read::Length);}
        if let Some(index)=collection_index(property){return self.read_class_tokens(node,attribute,token_list_reads::Read::Index(index,true));}
        match property {
            "toggle" | "replace" | "entries" | "keys" | "values" | "forEach" => Err(
                format!("DOMTokenList operation {property:?} is not implemented."),
            ),
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
        let attribute=attribute(object).ok_or("Invalid token attribute kind.")?;
        if property != "value" {
            return Err("DOMTokenList property write is not implemented.".into());
        }
        let value = self.text(&value)?;
        self.attribute(node, attribute, Some(value))
    }
    pub(super) fn class_list_call(
        &mut self,
        object: u32,
        method: &str,
        args: &[Value],
    ) -> Result<Value, String> {
        let node = self.class_list_node(object)?;
        let attribute=attribute(object).ok_or("Invalid token attribute kind.")?;
        self.class_list_validate(object, method, args)
            .map_err(|(_, message)| message)?;
        if method == "toString" {
            return self.class_value(node,attribute);
        }
        if method=="supports" {
            let token=token_list_reads::argument(args.first().ok_or("supports requires a converted DOMString token.")?)?;
            let supported=supported_tokens(object).ok_or("This token attribute has no supported token set.")?;
            self.state.work(token.logical_bytes().saturating_mul(supported.len().max(1)))?;
            let fold=|unit:u16|if (0x41..=0x5a).contains(&unit){unit+0x20}else{unit};
            return Ok(Value::Bool(supported.iter().any(|name|token.code_units().map(fold).eq(name.encode_utf16().map(fold)))));
        }
        if method=="contains" {
            let token=token_list_reads::argument(args.first().ok_or("contains requires a converted DOMString token.")?)?;
            return self.read_class_tokens(node,attribute,token_list_reads::Read::Contains(token));
        }
        if method=="item" {
            let value=args.first().ok_or("item requires an index.")?.to_number();
            let index=if value.is_finite(){value.trunc().rem_euclid(4_294_967_296.0)as usize}else{0};
            return self.read_class_tokens(node,attribute,token_list_reads::Read::Index(index,false));
        }
        let mut tokens = self.class_tokens(node,attribute)?;
        match method {
            "add" => {
                for value in args {
                    let Value::String(token) = value else {
                        unreachable!()
                    };
                    self.state.work(tokens.len())?;
                    self.state.copies(tokens.iter().map(String::len).sum())?;
                    if !tokens.contains(token) {
                        self.state.copies(token.len())?;
                        tokens.push(token.clone());
                    }
                }
                self.update_class_tokens(node, attribute, &tokens)?;
                Ok(Value::Undefined)
            }
            "remove" => {
                let removed: BTreeSet<&str> = args
                    .iter()
                    .map(|value| match value {
                        Value::String(token) => token.as_str(),
                        _ => unreachable!(),
                    })
                    .collect();
                tokens.retain(|token| !removed.contains(token.as_str()));
                self.update_class_tokens(node, attribute, &tokens)?;
                Ok(Value::Undefined)
            }
            _ => Err(format!(
                "DOMTokenList method {method:?} is not implemented."
            )),
        }
    }
    fn update_class_tokens(&mut self, node: usize, attribute:&str, tokens: &[String]) -> Result<(), String> {
        if tokens.is_empty() && self.doc.attr(node, attribute).is_none() {
            return Ok(());
        }
        let length = tokens
            .iter()
            .try_fold(tokens.len().saturating_sub(1), |n, token| {
                n.checked_add(token.len())
            })
            .ok_or("Class token serialization size overflow.")?;
        if length > self.state.limits.host.string_bytes {
            return Err("Class token serialization exceeds the host string bound.".into());
        }
        self.state.copies(length)?;
        self.attribute(node, attribute, Some(tokens.join(" ")))
    }
    pub(super) fn class_list_keys(&mut self, object: u32) -> Result<Vec<String>, String> {
        let node = self.class_list_node(object)?;
        let attribute=attribute(object).ok_or("Invalid token attribute kind.")?;
        let Value::Number(count)=self.read_class_tokens(node,attribute,token_list_reads::Read::Length)? else{unreachable!()};
        let count=count as usize;
        self.state.work(count)?;
        self.state.copies(
            count
                .checked_mul(20)
                .ok_or("Class token index accounting overflow.")?,
        )?;
        let available=self.state.retained_limit.saturating_sub(self.state.retained);
        let _required=count.checked_mul(size_of::<String>()+20).filter(|&n|n<=available).ok_or("Token read keys retained allowance exhausted.")?;
        let mut keys=Vec::new();keys.try_reserve_exact(count).map_err(|_|"Cannot reserve token read keys.")?;
        let mut storage=keys.capacity().checked_mul(size_of::<String>()).filter(|&n|n<=available).ok_or("Token read keys allocation exceeds allowance.")?;
        for index in 0..count{
            let mut key=String::new();key.try_reserve_exact(20).map_err(|_|"Cannot reserve token read key spelling.")?;
            storage=storage.checked_add(key.capacity()).filter(|&n|n<=available).ok_or("Token read key spelling exceeds allowance.")?;
            use std::fmt::Write;
            write!(&mut key,"{index}").map_err(|_|"Cannot format token read key spelling.")?;
            keys.push(key);
        }
        Ok(keys)
    }
}
