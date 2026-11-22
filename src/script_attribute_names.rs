//! DOM attribute name validation. Namespace strings confer no network authority.
//! https://dom.spec.whatwg.org/#validate-and-extract
// XML 1.0 Name, used by Document processing instructions.
pub(super) fn valid_xml_name(name: &str) -> bool {
    fn start(ch: char) -> bool {
        matches!(ch,
            ':' | 'A'..='Z' | '_' | 'a'..='z' | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}'
            | '\u{F8}'..='\u{2FF}' | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}'
            | '\u{200C}'..='\u{200D}' | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}'
            | '\u{3001}'..='\u{D7FF}' | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}'
            | '\u{10000}'..='\u{EFFFF}')
    }

    let mut chars = name.chars();
    chars.next().is_some_and(start)
        && chars.all(|ch| start(ch) || matches!(ch,
            '-' | '.' | '0'..='9' | '\u{B7}' | '\u{300}'..='\u{36F}' | '\u{203F}'..='\u{2040}'))
}

pub(super) fn valid_attribute_name(name: &str) -> bool {
    !name.is_empty() && !name.chars().any(|c| matches!(c, '\0' | '\t' | '\n' | '\u{c}' | '\r' | ' ' | '/' | '=' | '>'))
}

pub(super) fn valid_element_local_name(local:&str)->bool{
    let forbidden=|c:char|matches!(c,'\0'|'\t'|'\n'|'\u{c}'|'\r'|' '|'/'|'>');
    let mut chars=local.chars();match chars.next(){Some(c) if c.is_ascii_alphabetic()=>!local.chars().any(forbidden),Some(c) if c==':'||c=='_'||c>='\u{80}'=>chars.all(|c|c.is_ascii_alphanumeric()||matches!(c,'-'|'.'|':'|'_')||c>='\u{80}'),_=>false}
}

// Element-context validation differs from attribute-context validation (for
// example, '=' is permitted after an ASCII alpha). Unsupported factory slices
// are checked separately, after the real name/namespace exceptions.
pub(super) fn validate_element_namespace(namespace:Option<&str>,qualified:&str)->Result<(),(&'static str,String)> {
    let namespace=namespace.filter(|s|!s.is_empty());
    validate_element_namespace_flags(namespace.is_some(),namespace==Some("http://www.w3.org/XML/1998/namespace"),namespace==Some("http://www.w3.org/2000/xmlns/"),qualified)
}
pub(super) fn validate_element_namespace_value(namespace:&crate::javascript::Value,qualified:&str)->Result<(),(&'static str,String)> {
    use crate::javascript::Value;
    let(present,xml,xmlns)=match namespace {
        Value::String(uri)=>(!uri.is_empty(),uri=="http://www.w3.org/XML/1998/namespace",uri=="http://www.w3.org/2000/xmlns/"),
        Value::StringUnits(uri)=>(!uri.is_empty(),uri.iter().copied().eq("http://www.w3.org/XML/1998/namespace".encode_utf16()),uri.iter().copied().eq("http://www.w3.org/2000/xmlns/".encode_utf16())),
        Value::Null|Value::Undefined=>(false,false,false),
        _=>return Err(("TypeError","Namespace must be an already converted nullable DOMString.".into())),
    };
    validate_element_namespace_flags(present,xml,xmlns,qualified)
}
fn validate_element_namespace_flags(present:bool,xml:bool,xmlns:bool,qualified:&str)->Result<(),(&'static str,String)> {
    let forbidden=|c:char|matches!(c,'\0'|'\t'|'\n'|'\u{c}'|'\r'|' '|'/'|'>');
    let (prefix,local)=qualified.split_once(':').map_or((None,qualified),|(p,l)|(Some(p),l));
    let valid=valid_element_local_name(local);
    if !valid||prefix.is_some_and(|p|p.is_empty()||p.chars().any(forbidden)) {return Err(("InvalidCharacterError","Invalid element local name or namespace prefix.".into()));}
    let declaration=qualified=="xmlns"||prefix==Some("xmlns");
    if prefix.is_some()&&!present||prefix==Some("xml")&&!xml
        ||declaration&&!xmlns||xmlns&&!declaration {
        return Err(("NamespaceError","Element prefix and namespace are inconsistent.".into()));
    }
    Ok(())
}

pub(super) fn validate_namespace(namespace: Option<&str>, qualified: &str)
    -> Result<(), (&'static str, String)> {
    // All Web IDL conversions precede this validation. The current DOM rules
    // split at the first colon; local names can themselves contain colons.
    let (prefix, local) = qualified.split_once(':').map_or((None, qualified), |(p, l)| (Some(p), l));
    let valid_prefix = |name: &str| !name.is_empty()
        && !name.chars().any(|c| matches!(c, '\0' | '\t' | '\n' | '\u{c}' | '\r' | ' ' | '/' | '>'));
    if !valid_attribute_name(local) || prefix.is_some_and(|p| !valid_prefix(p)) {
        return Err(("InvalidCharacterError", "Invalid attribute local name or namespace prefix.".into()));
    }
    const XML: &str = "http://www.w3.org/XML/1998/namespace";
    const XMLNS: &str = "http://www.w3.org/2000/xmlns/";
    let declaration = qualified == "xmlns" || prefix == Some("xmlns");
    if prefix.is_some() && namespace.is_none()
        || prefix == Some("xml") && namespace != Some(XML)
        || declaration && namespace != Some(XMLNS)
        || namespace == Some(XMLNS) && !declaration {
        return Err(("NamespaceError", "Attribute prefix and namespace are inconsistent.".into()));
    }
    Ok(())
}
