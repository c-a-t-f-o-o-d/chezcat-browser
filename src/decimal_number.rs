//! Shared original decimal prefix grammar and HTML floating-point parsing.
pub(crate) fn prefix_end(bytes: &[u8], signed: bool) -> Option<usize> {
    let mut at = usize::from(signed && matches!(bytes.first(), Some(b'+' | b'-')));
    let begin = at;
    while bytes.get(at).is_some_and(u8::is_ascii_digit) {
        at += 1;
    }
    let integer_digits = at - begin;
    let mut fraction_digits = 0;
    if bytes.get(at) == Some(&b'.') {
        at += 1;
        let start = at;
        while bytes.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
        fraction_digits = at - start;
    }
    if integer_digits + fraction_digits == 0 {
        return None;
    }
    let end = at;
    if matches!(bytes.get(at), Some(b'e' | b'E')) {
        at += 1;
        if matches!(bytes.get(at), Some(b'+' | b'-')) {
            at += 1;
        }
        let digits = at;
        while bytes.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
        if at == digits {
            return Some(end);
        }
    }
    Some(at)
}

/// HTML admits only ASCII leading whitespace and finite results, with +0.
/// Callers charge the complete source scan before entering this allocation-free path.
pub(crate) fn html_float(text:&str)->Option<f64>{
 let text=text.trim_start_matches(|c|matches!(c,'\t'|'\n'|'\u{c}'|'\r'|' '));
 let end=prefix_end(text.as_bytes(),true)?;let value=text[..end].parse::<f64>().ok()?;
 if !value.is_finite(){None}else{Some(if value==0.0{0.0}else{value})}
}

#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn html_decimal_prefix_ascii_space_exponent_and_finite_rounding_boundaries(){
  for(text,want)in[(" \t\n\r\u{c}+.5e2tail",Some(50.0)),("1.e2",Some(100.0)),("1e+tail",Some(1.0)),("1.25.9",Some(1.25)),("0x10",Some(0.0)),("-0",Some(0.0)),("1e-99999",Some(0.0)),("0e99999",Some(0.0)),("9007199254740993",Some(9007199254740992.0)),("4.9406564584124654e-324",Some(f64::from_bits(1))),("1e309",None),("-1e309",None),("Infinity",None),("NaN",None),("\u{a0}1",None),("\u{b}1",None),("\u{feff}1",None),(".",None),("+",None),("",None)]{let result=html_float(text);assert_eq!(result,want,"{text:?}");if result==Some(0.0){assert_eq!(result.unwrap().to_bits(),0,"{text:?}");}}
 }
}
