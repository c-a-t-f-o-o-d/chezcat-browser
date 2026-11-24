//! Shared boundaries and refusal propagation for functional selector lists.
use super::{selector_comments, AttributeSelector, Compound, Relation, Selector, SelectorTarget,
    MAX_SELECTOR_BYTES, MAX_SELECTOR_NESTING};

#[derive(Default)]
pub(super) struct ParseState {
    pub(super) refused: bool,
}

impl ParseState {
    pub(super) fn admit(&mut self, allowed: bool) -> bool {
        self.refused |= !allowed;
        allowed
    }

    pub(super) fn reserve<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Option<()> {
        if values.try_reserve(additional).is_err() {
            self.refused = true;
            return None;
        }
        Some(())
    }
}

struct Boundaries {
    ends: [usize; 16],
    count: usize,
}

// Escapes are consumed only to locate boundaries. The selector grammar still
// decides whether an escaped member belongs to its supported subset.
fn escape(bytes: &[u8], position: &mut usize) {
    *position += 1;
    if bytes.get(*position).is_some_and(u8::is_ascii_hexdigit) {
        let mut digits = 0;
        while digits < 6 && bytes.get(*position).is_some_and(u8::is_ascii_hexdigit) {
            *position += 1;
            digits += 1;
        }
        if !bytes.get(*position).is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0c)) {
            return;
        }
    }
    if bytes.get(*position) == Some(&b'\r') && bytes.get(*position + 1) == Some(&b'\n') {
        *position += 2;
    } else if *position < bytes.len() {
        *position += 1;
    }
}

// Ordinary lists and function bodies use the same component boundaries. An
// unmatched closer or bad string cannot be repaired by forgiving a member.
// Complete tokenization and EOF block recovery remain outside this grammar.
fn boundaries(source: &str, start: usize, function: bool, depth: usize,
    state: &mut ParseState) -> Option<Boundaries> {
    let bytes = source.as_bytes();
    let mut ends = [0; 16];
    let mut count = 0;
    // An accepted member has at most MAX_SELECTOR_BYTES effective bytes, so
    // its opening delimiters fit here without allocating for ordinary queries.
    let mut stack = [0; MAX_SELECTOR_BYTES];
    let mut stack_len = 0;
    let mut parentheses = depth;
    let mut quote = 0;
    let mut position = start;
    while let Some(&byte) = bytes.get(position) {
        if quote != 0 {
            if byte == b'\\' {
                escape(bytes, &mut position);
                continue;
            }
            if matches!(byte, b'\n' | b'\r' | 0x0c) { return None; }
            if byte == quote { quote = 0; }
            position += 1;
            continue;
        }
        if selector_comments::skip(bytes, &mut position) { continue; }
        match byte {
            b'\\' => {
                escape(bytes, &mut position);
                continue;
            }
            b'\'' | b'"' => quote = byte,
            b'(' | b'[' | b'{' => {
                if byte == b'(' {
                    parentheses += 1;
                    if !state.admit(parentheses <= MAX_SELECTOR_NESTING) { return None; }
                }
                if !state.admit(stack_len < stack.len()) { return None; }
                stack[stack_len] = match byte { b'(' => b')', b'[' => b']', _ => b'}' };
                stack_len += 1;
            }
            b')' if stack_len == 0 && function => {
                ends[count] = position;
                return Some(Boundaries { ends, count: count + 1 });
            }
            b')' | b']' | b'}' => {
                if stack_len == 0 || stack[stack_len - 1] != byte { return None; }
                stack_len -= 1;
                if byte == b')' { parentheses -= 1; }
            }
            b',' if stack_len == 0 => {
                // Count attempted members, including discarded and empty ones.
                if !state.admit(count < ends.len() - 1) { return None; }
                ends[count] = position;
                count += 1;
            }
            _ => {}
        }
        position += 1;
    }
    if function || quote != 0 || stack_len != 0 { return None; }
    ends[count] = position;
    Some(Boundaries { ends, count: count + 1 })
}

fn members(source: &str, mut start: usize, bounds: &Boundaries, pseudo_targets: bool,
    depth: usize, forgiving: bool, state: &mut ParseState) -> Option<Vec<Selector>> {
    let mut selectors = Vec::new();
    if selectors.try_reserve_exact(bounds.count).is_err() {
        state.refused = true;
        return None;
    }
    for &end in &bounds.ends[..bounds.count] {
        let selector = super::parse_selector_at_depth(&source[start..end], depth, state);
        if state.refused { return None; }
        match selector {
            Some(selector) if pseudo_targets || selector.target == SelectorTarget::Element => selectors.push(selector),
            _ if forgiving => {}
            _ => return None,
        }
        start = end + 1;
    }
    Some(selectors)
}

pub(super) fn list(source: &str, pseudo_targets: bool, state: &mut ParseState) -> Option<Vec<Selector>> {
    if !state.admit(selector_comments::within_limit(source, MAX_SELECTOR_BYTES * 16, false)) { return None; }
    let bounds = boundaries(source, 0, false, 0, state)?;
    members(source, 0, &bounds, pseudo_targets, 0, false, state)
}

pub(super) fn arguments(source: &str, position: &mut usize, depth: usize,
    forgiving: bool, state: &mut ParseState) -> Option<Vec<Selector>> {
    if source.as_bytes().get(*position) != Some(&b'(') { return None; }
    if !state.admit(depth < MAX_SELECTOR_NESTING) { return None; }
    let start = *position + 1;
    let bounds = boundaries(source, start, true, depth + 1, state)?;
    let arguments = members(source, start, &bounds, false, depth + 1, forgiving, state)?;
    *position = bounds.ends[bounds.count - 1] + 1;
    Some(arguments)
}

/// Potential group syntax prepays the full AST and repeated boundary scans.
/// Each AST slot or owned leaf is anchored at a source token. Four slots per
/// source byte conservatively cover Vec minimum capacities and growth; the
/// scalar payload/table and structural scratch allowances are additional.
pub(super) fn query_cost(source: &str) -> (usize, usize) {
    let bytes = source.as_bytes();
    let mut position = 0;
    let mut quote = 0;
    while let Some(&byte) = bytes.get(position) {
        if quote == 0 && selector_comments::skip(bytes, &mut position) { continue; }
        if byte == b':' {
            let mut name = position + 1;
            selector_comments::skip(bytes, &mut name);
            if [b"is(".as_slice(), b"where(".as_slice()].iter().any(|wanted|
                bytes.get(name..name + wanted.len()).is_some_and(|part| part.eq_ignore_ascii_case(wanted))) {
                let slots = std::mem::size_of::<Selector>() + std::mem::size_of::<Compound>()
                    + std::mem::size_of::<Relation>() + std::mem::size_of::<Vec<Selector>>()
                    + std::mem::size_of::<String>() + std::mem::size_of::<AttributeSelector>()
                    + std::mem::size_of::<super::structural::Predicate>();
                let scratch = 2 * (usize::MAX.ilog10() as usize + 2);
                let storage = source.len().saturating_mul(slots * 4 + 1 + std::mem::size_of::<usize>() + scratch);
                let work = source.len().saturating_mul(4 * (MAX_SELECTOR_NESTING + 1));
                return (work, storage);
            }
        }
        if byte == b'\\' { escape(bytes, &mut position); continue; }
        if quote == byte { quote = 0; }
        else if quote == 0 && matches!(byte, b'\'' | b'"') { quote = byte; }
        position += 1;
    }
    (0, 0)
}
