//! DOM attribute name validation. Namespace strings confer no network authority.
//! https://dom.spec.whatwg.org/#validate-and-extract
pub(super) fn valid_attribute_name(name: &str) -> bool {
    !name.is_empty() && !name.chars().any(|c| matches!(c, '\0' | '\t' | '\n' | '\u{c}' | '\r' | ' ' | '/' | '=' | '>'))
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
