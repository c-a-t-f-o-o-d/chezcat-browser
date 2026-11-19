//! Finite specified easing values. This does not evaluate or schedule motion.
//! https://www.w3.org/TR/css-easing-1/#easing-functions
use std::fmt::Write as _;

const MAX_BYTES: usize = 2048;
const MAX_ITEMS: usize = 32;

pub(crate) fn trim(value: &str) -> &str {
    value.trim_matches([' ', '\t', '\r', '\n', '\u{c}'])
}

pub(crate) fn priority(value: &str) -> (&str, bool) {
    let value = trim(value);
    if let Some(bang) = value.rfind('!') {
        if trim(&value[bang + 1..]).eq_ignore_ascii_case("important") {
            return (trim(&value[..bang]), true);
        }
    }
    (value, false)
}

fn number(value: &str) -> Option<f64> {
    let value = trim(value);
    if super::translate::number_end(value)? != value.len() { return None; }
    value.parse::<f64>().ok().filter(|value| value.is_finite())
}

#[derive(Clone, Copy)]
enum Numbers { Specified, Cssom }

struct Output { text: Option<String>, bytes: usize, numbers: Numbers }
impl std::fmt::Write for Output {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let bytes = self.bytes.checked_add(value.len()).ok_or(std::fmt::Error)?;
        if bytes > MAX_BYTES { return Err(std::fmt::Error); }
        if let Some(out) = &mut self.text { out.push_str(value); }
        self.bytes = bytes;
        Ok(())
    }
}

fn text(out: &mut Output, value: &str) -> Option<()> { out.write_str(value).ok() }

// Finite f64 specified text also covers the smallest subnormal's decimal form.
// Validation formats into this small bounded buffer and allocates nothing.
struct NumberText { bytes: [u8; 384], length: usize }
impl std::fmt::Write for NumberText {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let end = self.length.checked_add(value.len()).ok_or(std::fmt::Error)?;
        self.bytes.get_mut(self.length..end).ok_or(std::fmt::Error)?.copy_from_slice(value.as_bytes());
        self.length = end;
        Ok(())
    }
}

fn number_text(out: &mut Output, number: f64) -> Option<()> {
    let mut buffer = NumberText { bytes: [0; 384], length: 0 };
    match out.numbers {
        Numbers::Specified => write!(&mut buffer, "{number}").ok()?,
        Numbers::Cssom => {
            write!(&mut buffer, "{number:.6}").ok()?;
            while buffer.length != 0 && buffer.bytes[buffer.length - 1] == b'0' { buffer.length -= 1; }
            if buffer.bytes.get(buffer.length.wrapping_sub(1)) == Some(&b'.') { buffer.length -= 1; }
        }
    }
    let number = std::str::from_utf8(&buffer.bytes[..buffer.length]).ok()?;
    text(out, if number == "-0" { "0" } else { number })
}

fn item(value: &str, out: &mut Output) -> Option<()> {
    let value = trim(value);
    for keyword in ["linear", "ease", "ease-in", "ease-out", "ease-in-out"] {
        if value.eq_ignore_ascii_case(keyword) { return text(out, keyword); }
    }
    if value.eq_ignore_ascii_case("step-start") {
        return text(out, "steps(1, start)");
    }
    if value.eq_ignore_ascii_case("step-end") {
        return text(out, "steps(1)");
    }
    let (name, args) = value.split_once('(')?;
    let args = args.strip_suffix(')')?;
    if args.contains(['(', ')']) { return None; }
    let mut args = args.split(',');
    if name.eq_ignore_ascii_case("cubic-bezier") {
        let values = [number(args.next()?)?, number(args.next()?)?, number(args.next()?)?, number(args.next()?)?];
        if args.next().is_some() || !(0.0..=1.0).contains(&values[0]) || !(0.0..=1.0).contains(&values[2]) { return None; }
        text(out, "cubic-bezier(")?;
        for (i, value) in values.into_iter().enumerate() {
            if i != 0 { text(out, ", ")?; }
            number_text(out, value)?;
        }
        return text(out, ")");
    }
    if name.eq_ignore_ascii_case("steps") {
        let raw = trim(args.next()?);
        let digits = raw.strip_prefix('+').unwrap_or(raw);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) { return None; }
        let count = digits.parse::<u32>().ok().filter(|n| *n > 0 && *n <= i32::MAX as u32)?;
        let position = args.next().map(trim).unwrap_or("end");
        if args.next().is_some() { return None; }
        let position = ["start", "end", "jump-start", "jump-end", "jump-none", "jump-both"]
            .into_iter().find(|name| position.eq_ignore_ascii_case(name))?;
        if position == "jump-none" && count < 2 { return None; }
        text(out, "steps(")?;
        write!(out, "{count}").ok()?;
        if !matches!(position, "end" | "jump-end") { text(out, ", ")?; text(out, position)?; }
        return text(out, ")");
    }
    None
}

fn parse(value: &str, out: &mut Output) -> Option<()> {
    if value.len() > MAX_BYTES || !value.is_ascii() || value.bytes().any(|b| b.is_ascii_control() && !matches!(b, b'\t' | b'\n' | b'\r' | 12)) { return None; }
    let value = trim(value);
    for keyword in ["inherit", "initial", "unset", "revert", "revert-layer"] {
        if value.eq_ignore_ascii_case(keyword) { return text(out, keyword); }
    }
    let mut start = 0;
    let mut depth = 0usize;
    let mut count = 0;
    for (i, byte) in value.bytes().chain(std::iter::once(b',')).enumerate() {
        match byte {
            b'(' => { depth += 1; if depth > 1 { return None; } }
            b')' => depth = depth.checked_sub(1)?,
            b',' if depth == 0 => {
                if count == MAX_ITEMS { return None; }
                if count != 0 { text(out, ", ")?; }
                item(&value[start..i], out)?;
                count += 1; start = i + 1;
            }
            _ => {}
        }
    }
    (depth == 0 && count != 0).then_some(())
}

/// Validation allocates nothing; callers supply comment-free CSS tokens.
pub(crate) fn valid(value: &str) -> bool { parse(value, &mut Output { text: None, bytes: 0, numbers: Numbers::Specified }).is_some() }

fn normalized(value: &str, numbers: Numbers) -> Option<String> {
    if value.len() > MAX_BYTES { return None; }
    let clean;
    let value = if value.contains("/*") { clean = super::without_comments(value); clean.as_str() } else { value };
    let mut out = Output { text: Some(String::new()), bytes: 0, numbers };
    parse(value, &mut out)?;
    out.text
}

/// Specified storage and equality retain each parsed f64 number's identity;
/// six-decimal CSSOM presentation must never become the stored value or key.
pub(crate) fn specified(value: &str) -> Option<String> { normalized(value, Numbers::Specified) }
pub(crate) fn normalize(value: &str) -> Option<String> { normalized(value, Numbers::Cssom) }

/// Reserve new parsing/formatting costs before any allocation. A short exponent
/// may expand to 309 decimal digits, so source bytes alone are insufficient.
pub(crate) fn source_cost(bytes: usize) -> (usize, usize) {
    if bytes > MAX_BYTES { (1, 0) } else { (bytes.saturating_mul(128).saturating_add(1), bytes.saturating_mul(128).saturating_add(256)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inline_easing_parser_normalizes_finite_level_one_lists() {
        for (input, expected) in [
            (" LiNeAr ", "linear"), ("EASE-IN", "ease-in"),
            ("step-start", "steps(1, start)"), ("step-end", "steps(1)"),
            ("steps(+004, END)", "steps(4)"), ("steps(2, jump-none)", "steps(2, jump-none)"),
            ("steps(3,jump-start)", "steps(3, jump-start)"),
            ("cubic-bezier(+0.25, .1, 2.5e-1, 1)", "cubic-bezier(0.25, 0.1, 0.25, 1)"),
            ("cubic-bezier(-0, -2, 1, 3)", "cubic-bezier(0, -2, 1, 3)"),
            ("cubic-bezier(.123456789,0,1,1)", "cubic-bezier(0.123457, 0, 1, 1)"),
            ("cubic-bezier(1e-7,-.0000001,1,-1.12345678)", "cubic-bezier(0, 0, 1, -1.123457)"),
            ("ease, steps(2,start),linear", "ease, steps(2, start), linear"),
            ("revert-LAYER", "revert-layer"), ("linear/**/", "linear"),
        ] { assert_eq!(normalize(input).as_deref(), Some(expected), "{input}"); }
        for input in ["", " ", "linear,", ",linear", "linear,,ease", "inherit,linear", "cubic-bezier(0,0,2,1)", "cubic-bezier(0,0,1)", "cubic-bezier(0px,0,1,1)", "cubic-bezier(0.,0,1,1)", "cubic-bezier(NaN,0,1,1)", "cubic-bezier(0,0,1,1e999)", "steps(0)", "steps(1,jump-none)", "steps(2.0)", "steps(2e1)", "steps(2,unknown)", "steps(2,start,end)", "steps(2147483648)", "steps (2)", "linear(0,1)", "var(--timing)", "ease\u{b}", "url(x)", "linear; width:2px"] { assert!(normalize(input).is_none(), "{input}"); }
        assert!(normalize(&vec!["linear"; MAX_ITEMS].join(",")).is_some());
        assert!(normalize(&vec!["linear"; MAX_ITEMS + 1].join(",")).is_none());
        assert!(normalize(&" ".repeat(MAX_BYTES + 1)).is_none());
        let expanded = vec!["cubic-bezier(0,1e100,1,-1e100)"; MAX_ITEMS].join(",");
        assert!(expanded.len() < MAX_BYTES);
        assert!(!valid(&expanded), "canonical output is subject to the same finite bound");
        assert!(normalize(&expanded).is_none());
        let large = normalize("cubic-bezier(0, 1e100, 1, -1e100)").unwrap();
        assert!(!large.contains("e+"));
        assert!(valid(&large));
        assert_eq!(normalize(&large).as_ref(), Some(&large));
        let first = specified("cubic-bezier(.1234561,0,1,1)").unwrap();
        let second = specified("cubic-bezier(.1234564,0,1,1)").unwrap();
        assert_ne!(first, second);
        assert_eq!(normalize(&first), normalize(&second));
        assert_eq!(specified("cubic-bezier(+.12345640,0.0,1e0,1)"), Some(second));
        let subnormal = specified("cubic-bezier(5e-324,0,1,1)").unwrap();
        assert!(valid(&subnormal));
        assert_eq!(specified(&subnormal).as_ref(), Some(&subnormal));
    }
}
