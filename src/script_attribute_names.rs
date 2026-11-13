//! XML Name validation shared by attribute operations. No namespace is inferred.
//! https://www.w3.org/TR/xml/#NT-Name
pub(super) fn name_start(c: char) -> bool {
    matches!(c, ':' | '_' | 'A'..='Z' | 'a'..='z' | '\u{c0}'..='\u{d6}'
        | '\u{d8}'..='\u{f6}' | '\u{f8}'..='\u{2ff}' | '\u{370}'..='\u{37d}'
        | '\u{37f}'..='\u{1fff}' | '\u{200c}'..='\u{200d}' | '\u{2070}'..='\u{218f}'
        | '\u{2c00}'..='\u{2fef}' | '\u{3001}'..='\u{d7ff}' | '\u{f900}'..='\u{fdcf}'
        | '\u{fdf0}'..='\u{fffd}' | '\u{10000}'..='\u{effff}')
}
pub(super) fn name_char(c: char) -> bool {
    name_start(c) || matches!(c,'-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}')
}
pub(super) fn valid_name(name: &str) -> bool {
    let mut chars=name.chars();
    chars.next().is_some_and(name_start) && chars.all(name_char)
}
