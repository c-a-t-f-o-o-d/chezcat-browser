//! Structural predicates over the caller's real topology. Decimal coefficients
//! remain exact without a machine-integer cutoff or per-candidate allocation.
use super::*;
use std::cell::RefCell;
use std::cmp::Ordering;
const INDEX_DIGITS: usize = usize::MAX.ilog10() as usize + 1;
#[derive(Debug)]
struct Integer {
    negative: bool,
    digits: Vec<u8>,
}
impl Integer {
    fn magnitude(source: &str, negative: bool, refused: &mut bool) -> Option<Self> {
        if source.is_empty() || !source.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let source = source.trim_start_matches('0');
        let mut digits = Vec::new();
        if digits.try_reserve_exact(source.len()).is_err() {
            *refused = true;
            return None;
        }
        digits.extend(source.bytes().rev().map(|b| b - b'0'));
        Some(Self {
            negative: negative && !digits.is_empty(),
            digits,
        })
    }
    fn parse(source: &str, refused: &mut bool) -> Option<Self> {
        if let Some(s) = source.strip_prefix('-') {
            Self::magnitude(s, true, refused)
        } else {
            Self::magnitude(source.strip_prefix('+').unwrap_or(source), false, refused)
        }
    }
}
#[derive(Debug)]
struct Scratch {
    delta: Vec<u8>,
    remainder: Vec<u8>,
}
#[derive(Debug)]
pub(super) struct Nth {
    a: Integer,
    b: Integer,
    reverse: bool,
    typed: bool,
    scratch: RefCell<Scratch>,
}
#[derive(Debug)]
pub(super) enum Predicate {
    Nth(Nth),
    Only(bool),
    Empty,
}
fn pay(budget: &mut usize, amount: usize) -> bool {
    match budget.checked_sub(amount) {
        Some(left) if left != 0 => {
            *budget = left;
            true
        }
        _ => {
            *budget = 0;
            false
        }
    }
}
fn compare(a: &[u8], b: &[u8], budget: &mut usize) -> Option<Ordering> {
    if !pay(
        budget,
        if a.len() == b.len() {
            a.len().max(1)
        } else {
            1
        },
    ) {
        return None;
    }
    Some(
        a.len()
            .cmp(&b.len())
            .then_with(|| a.iter().rev().cmp(b.iter().rev())),
    )
}
fn trim(a: &mut Vec<u8>) {
    while a.last() == Some(&0) {
        a.pop();
    }
}
fn difference(out: &mut Vec<u8>, a: &[u8], b: &[u8], budget: &mut usize) -> Option<()> {
    if !pay(budget, a.len().max(1)) {
        return None;
    }
    out.clear();
    let mut borrow = 0i8;
    for (i, &x) in a.iter().enumerate() {
        let v = x as i8 - b.get(i).copied().unwrap_or(0) as i8 - borrow;
        out.push(if v < 0 { (v + 10) as u8 } else { v as u8 });
        borrow = i8::from(v < 0);
    }
    trim(out);
    Some(())
}
fn subtract(a: &mut Vec<u8>, b: &[u8], budget: &mut usize) -> Option<()> {
    if !pay(budget, a.len().max(1)) {
        return None;
    }
    let mut borrow = 0i8;
    for (i, x) in a.iter_mut().enumerate() {
        let v = *x as i8 - b.get(i).copied().unwrap_or(0) as i8 - borrow;
        *x = if v < 0 { (v + 10) as u8 } else { v as u8 };
        borrow = i8::from(v < 0);
    }
    trim(a);
    Some(())
}
impl Nth {
    fn new(a: Integer, b: Integer, reverse: bool, typed: bool, refused: &mut bool) -> Option<Self> {
        let Some(length) = a
            .digits
            .len()
            .max(b.digits.len())
            .max(INDEX_DIGITS)
            .checked_add(1)
        else {
            *refused = true;
            return None;
        };
        let mut delta = Vec::new();
        let mut remainder = Vec::new();
        if delta.try_reserve_exact(length).is_err() {
            *refused = true;
            return None;
        }
        if remainder.try_reserve_exact(length).is_err() {
            *refused = true;
            return None;
        }
        Some(Self {
            a,
            b,
            reverse,
            typed,
            scratch: RefCell::new(Scratch { delta, remainder }),
        })
    }
    fn ordinal(&self, index: usize, budget: &mut usize) -> bool {
        let mut digits = [0; INDEX_DIGITS];
        let (mut value, mut length) = (index, 0);
        while value != 0 {
            if !pay(budget, 1) {
                return false;
            }
            digits[length] = (value % 10) as u8;
            length += 1;
            value /= 10;
        }
        let digits = &digits[..length];
        if self.a.digits.is_empty() {
            return !self.b.negative
                && compare(digits, &self.b.digits, budget) == Some(Ordering::Equal);
        }
        let mut scratch = self.scratch.borrow_mut();
        let Scratch { delta, remainder } = &mut *scratch;
        let negative = if self.b.negative {
            let count = digits.len().max(self.b.digits.len());
            if !pay(budget, count + 1) {
                return false;
            }
            delta.clear();
            let mut carry = 0;
            for i in 0..count {
                let n = digits.get(i).copied().unwrap_or(0)
                    + self.b.digits.get(i).copied().unwrap_or(0)
                    + carry;
                delta.push(n % 10);
                carry = n / 10;
            }
            if carry != 0 {
                delta.push(carry);
            }
            false
        } else {
            match compare(digits, &self.b.digits, budget) {
                Some(Ordering::Less) => {
                    if difference(delta, &self.b.digits, digits, budget).is_none() {
                        return false;
                    }
                    true
                }
                Some(_) => {
                    if difference(delta, digits, &self.b.digits, budget).is_none() {
                        return false;
                    }
                    false
                }
                None => return false,
            }
        };
        if delta.is_empty() {
            return true;
        }
        if negative != self.a.negative {
            return false;
        }
        remainder.clear();
        for &digit in delta.iter().rev() {
            if !pay(budget, remainder.len() + 1) {
                return false;
            }
            if !remainder.is_empty() || digit != 0 {
                remainder.insert(0, digit);
            }
            loop {
                match compare(remainder, &self.a.digits, budget) {
                    Some(Ordering::Less) => break,
                    Some(_) => {
                        if subtract(remainder, &self.a.digits, budget).is_none() {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
        }
        remainder.is_empty()
    }
}
pub(super) fn name(name: &str) -> bool {
    [
        "first-child",
        "last-child",
        "only-child",
        "first-of-type",
        "last-of-type",
        "only-of-type",
        "empty",
        "nth-child",
        "nth-last-child",
        "nth-of-type",
        "nth-last-of-type",
    ]
    .iter()
    .any(|n| name.eq_ignore_ascii_case(n))
}
pub(super) fn parse(name: &str, source: &str, position: &mut usize, refused: &mut bool) -> Option<Predicate> {
    let name = name.to_ascii_lowercase();
    let typed = name.ends_with("of-type");
    let reverse = name.contains("last");
    if name == "empty" {
        return Some(Predicate::Empty);
    }
    if name.starts_with("only-") {
        return Some(Predicate::Only(typed));
    }
    let (a, b) = if name.starts_with("nth-") {
        if source.as_bytes().get(*position) != Some(&b'(') {
            return None;
        }
        let start = *position + 1;
        let bytes = source.as_bytes();
        let mut end = start;
        while end < bytes.len() {
            if selector_comments::skip(bytes, &mut end) { continue; }
            if bytes[end] == b')' { break; }
            end += 1;
        }
        if bytes.get(end) != Some(&b')') { return None; }
        *position = end + 1;
        let raw = &source[start..end];
        let mut first = 0;
        skip_selector_space(raw.as_bytes(), &mut first);
        let mut plus_tail = None;
        if raw.as_bytes().get(first) == Some(&b'+') {
            let mut after = first + 1;
            if selector_comments::skip(raw.as_bytes(), &mut after)
                && matches!(raw.as_bytes().get(after), Some(b'n' | b'N')) {
                plus_tail = Some(after);
            }
        }
        // An+B permits a '+' delimiter directly before an n identifier token.
        // Comments can separate those tokens; actual whitespace cannot.
        let normalized = if let Some(after) = plus_tail {
            let mut body = String::from("+");
            body.push_str(&without_comments(&raw[after..]));
            body
        } else {
            without_comments(raw)
        };
        let body = normalized.trim_ascii();
        if body.eq_ignore_ascii_case("odd") {
            (Integer::parse("2", refused)?, Integer::parse("1", refused)?)
        } else if body.eq_ignore_ascii_case("even") {
            (Integer::parse("2", refused)?, Integer::parse("0", refused)?)
        } else if let Some(n) = body.bytes().position(|b| matches!(b, b'n' | b'N')) {
            let prefix = &body[..n];
            let a = Integer::parse(match prefix {
                "" | "+" => "1",
                "-" => "-1",
                _ => prefix,
            }, refused)?;
            let suffix = body[n + 1..].trim_ascii();
            let b = if suffix.is_empty() {
                Integer::parse("0", refused)?
            } else {
                let sign = *suffix.as_bytes().first()?;
                if !matches!(sign, b'+' | b'-') {
                    return None;
                }
                Integer::magnitude(suffix[1..].trim_ascii_start(), sign == b'-', refused)?
            };
            (a, b)
        } else {
            (Integer::parse("0", refused)?, Integer::parse(body, refused)?)
        }
    } else {
        (Integer::parse("0", refused)?, Integer::parse("1", refused)?)
    };
    Some(Predicate::Nth(Nth::new(a, b, reverse, typed, refused)?))
}
impl Predicate {
    pub(super) fn matches(
        &self,
        context: &MatchContext<'_>,
        node: usize,
        budget: &mut usize,
    ) -> bool {
        if let Self::Empty = self {
            for &child in &context.doc.nodes[node].children {
                if !pay(budget, 1) {
                    return false;
                }
                match context.doc.nodes.get(child).map(|n| &n.kind) {
                    Some(NodeKind::Element { .. }) => return false,
                    Some(NodeKind::Text(t) | NodeKind::Cdata(t)) if !t.is_empty() => return false,
                    None => return false,
                    _ => {}
                }
            }
            return true;
        }
        let typed = match self {
            Self::Only(typed) => *typed,
            Self::Nth(nth) => nth.typed,
            _ => false,
        };
        let Some(NodeKind::Element { tag, .. }) = context.doc.nodes.get(node).map(|n| &n.kind)
        else {
            return false;
        };
        let (mut count, mut ordinal) = (0usize, 0usize);
        if let Some(parent) = context.parents.get(node).copied().flatten() {
            let Some(parent) = context.doc.nodes.get(parent) else {
                return false;
            };
            for &child in &parent.children {
                if !pay(budget, 1) {
                    return false;
                }
                let Some(candidate) = context.doc.nodes.get(child) else {
                    return false;
                };
                let NodeKind::Element { tag: other, .. } = &candidate.kind else {
                    continue;
                };
                if typed {
                    if !pay(
                        budget,
                        tag.len().saturating_add(other.len()).max(1).div_ceil(64),
                    ) {
                        return false;
                    }
                    if candidate.namespace != context.doc.nodes[node].namespace || other != tag {
                        continue;
                    }
                }
                count += 1;
                if child == node {
                    ordinal = count;
                }
            }
        } else {
            count = 1;
            ordinal = 1;
        }
        if ordinal == 0 {
            return false;
        }
        match self {
            Self::Only(_) => count == 1,
            Self::Nth(nth) => nth.ordinal(
                if nth.reverse {
                    count - ordinal + 1
                } else {
                    ordinal
                },
                budget,
            ),
            Self::Empty => unreachable!(),
        }
    }
}

/// Potential structural syntax prepays a source-sized upper bound before the
/// parser allocates coefficients and reusable arithmetic storage. Quoted text
/// containing contiguous pseudo names can conservatively pay the same cost.
pub(super) fn query_cost(source: &str) -> usize {
    let bytes = source.as_bytes();
    let names = [
        "first-child",
        "last-child",
        "only-child",
        "first-of-type",
        "last-of-type",
        "only-of-type",
        "empty",
        "nth-",
    ];
    let structural_name = |at: usize| {
        bytes.get(at..).is_some_and(|tail| names.iter().any(|name| {
            tail.get(..name.len()).is_some_and(|part| part.eq_ignore_ascii_case(name.as_bytes()))
        }))
    };
    let mut at = 0;
    let mut quote = 0;
    while let Some(&byte) = bytes.get(at) {
        if quote != 0 {
            if byte == b':' && structural_name(at + 1) {
                return source.len().saturating_mul(64);
            }
            if byte == b'\\' { at = (at + 2).min(bytes.len()); continue; }
            if byte == quote { quote = 0; }
            at += 1;
            continue;
        }
        if selector_comments::skip(bytes, &mut at) { continue; }
        if matches!(byte, b'\'' | b'"') {
            quote = byte;
        } else if byte == b':' {
            at += 1;
            selector_comments::skip(bytes, &mut at);
            if structural_name(at) { return source.len().saturating_mul(64); }
            continue;
        }
        at += 1;
    }
    0
}
