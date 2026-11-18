//! Context-none canvas dimensions. There is no allocated rendering context.
//! https://html.spec.whatwg.org/multipage/canvas.html#the-canvas-element
use crate::html::{Document, Namespace, NodeKind};

pub(crate) fn element(doc: &Document, node: usize) -> bool {
    doc.nodes.get(node).is_some_and(|item| {
        item.namespace == Namespace::Html
            && matches!(&item.kind, NodeKind::Element { tag, .. } if tag == "canvas")
    })
}

pub(crate) fn default_dimension(name: &str) -> u32 {
    if name == "width" { 300 } else { 150 }
}

/// A bounded decimal representation retains the natural aspect ratio even when
/// both dimensions exceed the renderer's coordinate cap. Eighteen significant
/// digits suffice for its floating-point geometry; the exponent is bounded by
/// the admitted attribute string, without a proportional integer allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NaturalDimension {
    significant: u64,
    exponent: usize,
}

impl NaturalDimension {
    pub(crate) fn is_zero(self) -> bool { self.significant == 0 }

    pub(crate) fn bounded_value(self, maximum: f64) -> f64 {
        if self.is_zero() { return 0.0; }
        if self.exponent > 308 { return maximum; }
        ((self.significant as f64) * 10f64.powi(self.exponent as i32)).min(maximum)
    }

    pub(crate) fn scaled_ratio(self, denominator: Self, scale: f64) -> Option<f64> {
        if denominator.is_zero() { return None; }
        if self.is_zero() || scale == 0.0 { return Some(0.0); }
        let exponent = self.exponent as i64 - denominator.exponent as i64;
        Some(if exponent > 308 { f64::INFINITY }
            else if exponent < -308 { 0.0 }
            else { scale * (self.significant as f64 / denominator.significant as f64) * 10f64.powi(exponent as i32) })
    }
}

/// HTML's integer parser accepts leading ASCII whitespace/sign and a digit
/// prefix. In particular, -0 is a non-negative integer.
fn nonnegative_integer(raw: &str) -> Option<NaturalDimension> {
    let raw = raw.trim_start_matches([' ', '\t', '\n', '\r', '\u{000c}']);
    let (negative, raw) = match raw.as_bytes().first() {
        Some(b'-') => (true, &raw[1..]),
        Some(b'+') => (false, &raw[1..]),
        _ => (false, raw),
    };
    if !raw.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        return None;
    }
    let mut value = NaturalDimension { significant: 0, exponent: 0 };
    for digit in raw.bytes().take_while(u8::is_ascii_digit) {
        if value.significant < 100_000_000_000_000_000 {
            value.significant = value.significant * 10 + u64::from(digit - b'0');
        } else {
            value.exponent += 1;
        }
    }
    (!negative || value.is_zero()).then_some(value)
}

pub(crate) fn reflected_dimension(raw: Option<&str>, name: &str) -> u32 {
    raw
        .and_then(nonnegative_integer)
        .filter(|value| value.exponent == 0 && value.significant <= i32::MAX as u64)
        .map_or_else(|| default_dimension(name), |value| value.significant as u32)
}

/// Natural dimensions use the raw canvas attribute algorithm, independently of
/// the reflected IDL range. The renderer applies its established finite cap.
pub(crate) fn natural_dimension(raw: Option<&str>, name: &str) -> NaturalDimension {
    raw.and_then(nonnegative_integer).unwrap_or_else(|| NaturalDimension {
        significant: u64::from(default_dimension(name)), exponent: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_dimensions_raw_integer_and_reflected_ranges_are_distinct() {
        for (raw, parsed) in [
            ("  +0020px", Some(20)), ("-0", Some(0)), ("0x20", Some(0)),
            ("1.9", Some(1)), ("-1", None), ("", None), ("\u{00a0}2", None),
        ] {
            assert_eq!(nonnegative_integer(raw).map(|value| value.significant), parsed, "{raw:?}");
        }
        let doc = crate::html::parse("<canvas width=2147483648 height=-0></canvas>");
        let node = (0..doc.nodes.len()).find(|&node| element(&doc, node)).unwrap();
        assert_eq!(reflected_dimension(doc.attr(node, "width"), "width"), 300);
        assert_eq!(reflected_dimension(doc.attr(node, "height"), "height"), 0);
        assert_eq!(natural_dimension(doc.attr(node,"width"),"width").bounded_value(f64::INFINITY), 2147483648.0);
        assert!(natural_dimension(doc.attr(node,"height"),"height").is_zero());
        let width = natural_dimension(Some("1000000000000000000000000000000000000000"), "width");
        let height = natural_dimension(Some("2000000000000000000000000000000000000000"), "height");
        assert_eq!(height.scaled_ratio(width, 20.0), Some(40.0));
    }
}
