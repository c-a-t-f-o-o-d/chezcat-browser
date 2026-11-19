//! Borrowed attribute comparisons with bounded, linear substring matching.
use super::{AttributeOperator, AttributeSelector, MatchContext};

fn equal_byte(a: u8, b: u8, insensitive: bool) -> bool {
    if insensitive { a.eq_ignore_ascii_case(&b) } else { a == b }
}

/// The selector's existing byte limit bounds this one table. Exact, token,
/// prefix, suffix and presence selectors retain an empty, allocation-free Vec.
pub(super) fn prefix_table(value: &str, insensitive: bool) -> Option<Vec<usize>> {
    let bytes = value.as_bytes();
    let mut prefix = Vec::new();
    prefix.try_reserve_exact(bytes.len()).ok()?;
    prefix.resize(bytes.len(), 0);
    for i in 1..bytes.len() {
        let mut matched = prefix[i - 1];
        while matched > 0 && !equal_byte(bytes[i], bytes[matched], insensitive) {
            matched = prefix[matched - 1];
        }
        if equal_byte(bytes[i], bytes[matched], insensitive) { matched += 1; }
        prefix[i] = matched;
    }
    Some(prefix)
}

pub(super) fn matches(
    selector: &AttributeSelector,
    actual: &str,
    expected: &str,
    context: &MatchContext<'_>,
    budget: &mut usize,
) -> bool {
    let (actual, expected) = (actual.as_bytes(), expected.as_bytes());
    let insensitive = selector.ascii_insensitive;
    let equals = |a: &[u8], b: &[u8]| {
        if insensitive { a.eq_ignore_ascii_case(b) } else { a == b }
    };
    match selector.operator {
        AttributeOperator::Dash => {
            if actual.len() < expected.len() {
                context.text_work(1, budget);
                return false;
            }
            context.text_work(expected.len().saturating_add(1), budget)
                && (actual.len() == expected.len() || actual[expected.len()] == b'-')
                && equals(&actual[..expected.len()], expected)
        }
        AttributeOperator::Prefix | AttributeOperator::Suffix => {
            // Unlike |=, the three substring operators never match an empty
            // operand, including when the actual attribute is also empty.
            if expected.is_empty() || actual.len() < expected.len() {
                context.text_work(1, budget);
                return false;
            }
            if !context.text_work(expected.len(), budget) { return false; }
            let slice = if matches!(selector.operator, AttributeOperator::Prefix) {
                &actual[..expected.len()]
            } else {
                &actual[actual.len() - expected.len()..]
            };
            equals(slice, expected)
        }
        AttributeOperator::Substring => {
            if expected.is_empty() || actual.len() < expected.len() {
                context.text_work(1, budget);
                return false;
            }
            // Prefix fallback visits at most twice as many input bytes. Charge
            // before the scan; repeated prefixes do not trigger N*M windows or
            // allocate/lowercase the actual DOM attribute.
            let cost = actual.len().saturating_mul(2).saturating_add(expected.len());
            if !context.text_work(cost, budget) { return false; }
            let mut matched = 0;
            for &byte in actual {
                while matched > 0 && !equal_byte(byte, expected[matched], insensitive) {
                    matched = selector.substring_prefix[matched - 1];
                }
                if equal_byte(byte, expected[matched], insensitive) { matched += 1; }
                if matched == expected.len() { return true; }
            }
            false
        }
        AttributeOperator::Exact | AttributeOperator::Token => unreachable!(),
    }
}
