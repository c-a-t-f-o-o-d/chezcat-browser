//! Live class token lists over real HTML Elements. No second token cache or
//! independent mutation authority; raw attribute edits are immediately visible.
//! https://dom.spec.whatwg.org/#interface-domtokenlist
use super::*;
use std::collections::BTreeSet;

pub(super) const BASE: u32 = 0x8000_0000;
const END: u32 = BASE + NODE_HANDLES;
const SANDBOX_BASE:u32=0x8100_0000;
const SANDBOX_END:u32=SANDBOX_BASE+NODE_HANDLES;
const INERT_BASE:u32=0xc200_0000;
const INERT_END:u32=INERT_BASE+(script_inert_documents::ENTRY_END-script_inert_documents::MAIN_IMPLEMENTATION);
const OUTPUT_BASE:u32=0xc300_0000;
const OUTPUT_END:u32=OUTPUT_BASE+NODE_HANDLES;
const INERT_OUTPUT_BASE:u32=0xc400_0000;
const INERT_OUTPUT_END:u32=INERT_OUTPUT_BASE+(script_inert_documents::ENTRY_END-script_inert_documents::MAIN_IMPLEMENTATION);
const AREA_REL_BASE:u32=0xc700_0000;
const AREA_REL_END:u32=AREA_REL_BASE+NODE_HANDLES;
const INERT_AREA_REL_BASE:u32=0xc800_0000;
const INERT_AREA_REL_END:u32=INERT_AREA_REL_BASE+(script_inert_documents::ENTRY_END-script_inert_documents::MAIN_IMPLEMENTATION);
pub(super) fn attribute(handle:u32)->Option<&'static str> {
    if (BASE..END).contains(&handle)||(INERT_BASE+1..INERT_END).contains(&handle){Some("class")}
    else if (SANDBOX_BASE..SANDBOX_END).contains(&handle){Some("sandbox")}
    else if (OUTPUT_BASE..OUTPUT_END).contains(&handle)||(INERT_OUTPUT_BASE+1..INERT_OUTPUT_END).contains(&handle){Some("for")}
    else if (AREA_REL_BASE..AREA_REL_END).contains(&handle)||(INERT_AREA_REL_BASE+1..INERT_AREA_REL_END).contains(&handle){Some("rel")}
    else {None}
}
pub(super) fn element(handle:u32)->Option<u32> {
    if (INERT_AREA_REL_BASE+1..INERT_AREA_REL_END).contains(&handle){return Some(script_inert_documents::MAIN_IMPLEMENTATION+(handle-INERT_AREA_REL_BASE));}
    if (INERT_BASE+1..INERT_END).contains(&handle){return Some(script_inert_documents::MAIN_IMPLEMENTATION+(handle-INERT_BASE));}
    if (INERT_OUTPUT_BASE+1..INERT_OUTPUT_END).contains(&handle){return Some(script_inert_documents::MAIN_IMPLEMENTATION+(handle-INERT_OUTPUT_BASE));}
    let base=match attribute(handle)? {"class"=>BASE,"for"=>OUTPUT_BASE,"rel"=>AREA_REL_BASE,_=>SANDBOX_BASE};
    let element=node_handle((handle-base)as usize);node_index(element).map(|_|element)
}
pub(super) fn area_rel_handle(element:u32)->u32 {
    if node_index(element).is_some(){AREA_REL_BASE+(element-NODE_BASE)}else{INERT_AREA_REL_BASE+(element-script_inert_documents::MAIN_IMPLEMENTATION)}
}
pub(super) fn output_for_handle(element:u32)->u32 {
    if node_index(element).is_some(){OUTPUT_BASE+(element-NODE_BASE)}else{INERT_OUTPUT_BASE+(element-script_inert_documents::MAIN_IMPLEMENTATION)}
}
pub(super) fn sandbox_handle(element:u32)->u32 {SANDBOX_BASE+(element-NODE_BASE)}
pub(super) fn handle(element: u32) -> u32 {
    if node_index(element).is_some(){BASE+(element-NODE_BASE)}else{INERT_BASE+(element-script_inert_documents::MAIN_IMPLEMENTATION)}
}
fn whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}')
}
pub(super) fn supported_tokens(object:u32)->Option<&'static [&'static str]>{
    match attribute(object){Some("sandbox")=>Some(script_iframe_sandbox::SUPPORTED_TOKENS),Some("rel")=>Some(&[]),_=>None}
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
        "\0DOMTokenList.toggle"=>"toggle",
        "\0DOMTokenList.replace"=>"replace",
        "\0DOMTokenList.supports"=>"supports",
        "\0DOMTokenList.toString"=>"toString",
        _=>return None,
    })
}
pub(super) fn method(name: &str) -> Option<&'static str> {
    match name {
        "add" => Some("add"),
        "remove" => Some("remove"),
        "toggle" => Some("toggle"),
        "replace" => Some("replace"),
        "contains" => Some("contains"),
        "supports" => Some("supports"),
        "item" => Some("item"),
        "toString" => Some("toString"),
        _ => None,
    }
}

impl Bridge<'_> {
    pub(super) fn html_class_element(&self,node:usize)->bool {
        self.doc.nodes.get(node).is_some_and(|entry|entry.namespace==crate::html::Namespace::Html&&matches!(entry.kind,NodeKind::Element{..}))
    }
    pub(super) fn html_class_attribute(&self,node:usize,name:&crate::html::AttributeName)->bool {
        self.html_class_element(node)&&name.html_is("class")
    }
    pub(super) fn html_class_set(&mut self,node:usize,value:&Value)->Result<(),String> {
        let value=self.dom_text(value)?;
        self.state.copies("class".len())?;
        let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("class")?);
        self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)
    }
    pub(super) fn class_list_element_receiver(&self, object: u32) -> bool {
        self.attr_element_receiver(object)
            && node_index(object)
                .is_some_and(|node| matches!(self.doc.nodes[node].namespace,crate::html::Namespace::Html|crate::html::Namespace::Svg))
    }
    pub(super) fn class_list_receiver(&self, object: u32) -> bool {
        element(object).is_some_and(|element| (self.class_list_element_receiver(element)||self.inert_class_element_receiver(element)) && match attribute(object){Some("sandbox")=>self.iframe_sandbox_receiver(element),Some("for")=>self.html_output_receiver(element),Some("rel")=>self.area_rel_receiver(element),Some("class")=>true,_=>false})
    }
    pub(super) fn exact_class_token_receiver(&self,object:u32)->bool {
        self.class_list_receiver(object)&&matches!(attribute(object),Some("class"|"for"|"rel"))&&element(object).is_some_and(|owner|self.inert_class_element_receiver(owner)||node_index(owner).is_some_and(|node|self.html_class_element(node)))
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
    pub(super) fn class_value(&mut self, node: usize, attribute:&str) -> Result<Value, String> {
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
        if method=="replace" {
            let args=args.get(..2).ok_or_else(||("TypeError","replace requires two converted DOMString tokens.".into()))?;
            if !self.exact_class_token_receiver(object)&&args.iter().any(|value|!matches!(value,Value::String(_))){return Err(("TypeError","This token attribute requires scalar DOMString arguments.".into()));}
            let old=token_list_reads::argument(&args[0]).map_err(|message|("TypeError",message))?;
            let new=token_list_reads::argument(&args[1]).map_err(|message|("TypeError",message))?;
            // Both empty checks precede either whitespace check.
            if old.is_empty()||new.is_empty(){return Err(("SyntaxError","Replacement tokens must not be empty.".into()));}
            if old.code_units().chain(new.code_units()).any(|unit|matches!(unit,0x09|0x0a|0x0c|0x0d|0x20)){return Err(("InvalidCharacterError","Replacement tokens contain ASCII whitespace.".into()));}
            return Ok(());
        }
        if matches!(method, "add" | "remove" | "toggle") {
            for value in args.iter().take(if method=="toggle"{1}else{usize::MAX}) {
                if self.exact_class_token_receiver(object) {
                    let token=token_list_reads::argument(value).map_err(|message|("TypeError",message))?;
                    if token.is_empty(){return Err(("SyntaxError","Class token must not be empty.".into()));}
                    if token.code_units().any(|unit|matches!(unit,0x09|0x0a|0x0c|0x0d|0x20)){return Err(("InvalidCharacterError","Class token contains ASCII whitespace.".into()));}
                    continue;
                }
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
        if element(object).is_some_and(|owner|self.inert_class_element_receiver(owner)){return self.inert_class_list_get(object,property);}
        let node = self.class_list_node(object)?;
        let attribute=attribute(object).ok_or("Invalid token attribute kind.")?;
        if property == "value" {
            return self.class_value(node,attribute);
        }
        if property == "length" {return self.read_class_tokens(node,attribute,token_list_reads::Read::Length);}
        if let Some(index)=collection_index(property){return self.read_class_tokens(node,attribute,token_list_reads::Read::Index(index,true));}
        match property {
            "entries" | "keys" | "values" => Err(
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
        if element(object).is_some_and(|owner|self.inert_class_element_receiver(owner)){
            return self.inert_class_list_set(object,property,&value);
        }
        let node = self.class_list_node(object)?;
        let attribute=attribute(object).ok_or("Invalid token attribute kind.")?;
        if property != "value" {
            return Err("DOMTokenList property write is not implemented.".into());
        }
        if attribute=="for"{return self.output_for_value_set(node_handle(node),&value);}
        if attribute=="rel"{return self.area_rel_set(node_handle(node),&value);}
        if attribute=="class"&&self.html_class_element(node){return self.html_class_set(node,&value);}
        let value = self.text(&value)?;
        self.attribute(node, attribute, Some(value))
    }
    pub(super) fn class_list_call(
        &mut self,
        object: u32,
        method: &str,
        args: &[Value],
    ) -> Result<Value, String> {
        if element(object).is_some_and(|owner|self.inert_class_element_receiver(owner)){return self.inert_class_list_call(object,method,args);}
        let node = self.class_list_node(object)?;
        let attribute=attribute(object).ok_or("Invalid token attribute kind.")?;
        self.class_list_validate(object, method, args)
            .map_err(|(_, message)| message)?;
        if method == "toString" {
            return self.class_value(node,attribute);
        }
        if method=="replace" {
            let exact=self.exact_class_token_receiver(object);
            if !exact&&self.doc.dom_attr(node,attribute).is_some_and(|value|value.raw_units().is_some()){return Err("Raw UTF-16 token mutation requires exact serialization integration for this attribute.".into());}
            let token=token_list_reads::argument(&args[0])?;
            let Value::Bool(present)=self.read_class_tokens(node,attribute,token_list_reads::Read::Contains(token))? else{unreachable!()};
            if !present{return Ok(Value::Bool(false));}
            let source=self.doc.dom_attr(node,attribute).map(DomString::view).unwrap_or(crate::dom_string::DomStringRef::Scalar(""));
            let value=token_list_mutation::replace(self.state,source,&args[..2])?;
            self.state.copies(attribute.len())?;
            let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text(attribute)?);
            if exact{self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)?;}
            else{self.attribute_dom_named_prepared(node,name,Some(value),script_attr::InputPreparation::Pending)?;}
            return Ok(Value::Bool(true));
        }
        if method=="toggle" {
            let value=args.first().ok_or("toggle requires a converted DOMString token.")?;
            let token=token_list_reads::argument(value)?;
            let Value::Bool(present)=self.read_class_tokens(node,attribute,token_list_reads::Read::Contains(token))? else{unreachable!()};
            let desired=args.get(1).map_or(!present,Value::truthy);
            // A forced no-op retains the original spelling and generation.
            // Mutation observation publishes only the actual nested commit.
            if desired!=present {self.class_list_call(object,if desired{"add"}else{"remove"},std::slice::from_ref(value))?;}
            return Ok(Value::Bool(desired));
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
        if matches!(method,"add"|"remove")&&self.exact_class_token_receiver(object)
            &&(self.doc.dom_attr(node,attribute).is_some_and(|value|value.raw_units().is_some())||args.iter().any(|value|matches!(value,Value::StringUnits(_)))) {
            let source=self.doc.dom_attr(node,attribute).map(DomString::view).unwrap_or(crate::dom_string::DomStringRef::Scalar(""));
            let value=token_list_mutation::serialize(self.state,source,args,method=="add")?;
            if value.is_empty()&&self.doc.dom_attr(node,attribute).is_none(){return Ok(Value::Undefined);}
            self.state.copies(attribute.len())?;
            let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text(attribute)?);
            self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)?;
            return Ok(Value::Undefined);
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
        let Value::Number(count)=self.class_list_get(object,"length")? else{unreachable!()};
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
