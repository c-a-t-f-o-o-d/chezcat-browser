//! Shared HTML tag-name and attribute states for document and fragment parsing.
//! https://html.spec.whatwg.org/multipage/parsing.html#tag-name-state
//! Recovery produces real token data; strict consumers retain separate syntax
//! evidence. No attribute value or character-reference output reenters scanning.

use super::{named_references, svg_namespace, Tag, TagToken};
use std::collections::HashSet;

const CANCEL_INTERVAL: usize = 4096;
type Cancel<'a> = Option<&'a dyn Fn() -> Result<(), String>>;

pub(super) fn space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')
}

fn poll(check: Cancel<'_>) -> Result<(), String> {
    if let Some(check) = check { check()?; }
    Ok(())
}

struct Scan<'a, 'c> {
    bytes: &'a [u8],
    position: usize,
    next_check: usize,
    check: Cancel<'c>,
    raw_clean: bool,
    nulls: usize,
}

impl Scan<'_, '_> {
    fn current(&self) -> Option<u8> { self.bytes.get(self.position).copied() }

    fn advance(&mut self) -> Result<(), String> {
        match self.bytes[self.position] {
            0 => { self.raw_clean = false; self.nulls += 1; }
            11 => self.raw_clean = false,
            _ => {}
        }
        self.position += 1;
        if self.position >= self.next_check {
            poll(self.check)?;
            self.next_check = self.position.saturating_add(CANCEL_INTERVAL);
        }
        Ok(())
    }

    fn spaces(&mut self) -> Result<(), String> {
        while self.current().is_some_and(space) { self.advance()?; }
        Ok(())
    }
}

// Names fold ASCII only. Literal NULL replacement also applies in every
// attribute-value state; entities are decoded separately in attribute context.
fn normalize(input: &str, fold_ascii: bool, check: Cancel<'_>) -> Result<String, String> {
    let mut length = input.len();
    for chunk in input.as_bytes().chunks(CANCEL_INTERVAL) {
        poll(check)?;
        let nulls = chunk.iter().filter(|&&byte| byte == 0).count();
        length = length.checked_add(2 * nulls).ok_or("HTML token text length overflow.")?;
    }
    let mut output = String::new();
    output.try_reserve_exact(length).map_err(|_| "Cannot reserve HTML token text.")?;
    let mut remaining = input;
    while !remaining.is_empty() {
        poll(check)?;
        let end = remaining.floor_char_boundary(remaining.len().min(CANCEL_INTERVAL));
        let (chunk, rest) = remaining.split_at(end);
        for ch in chunk.chars() {
            output.push(if ch == '\0' { '\u{fffd}' }
                else if fold_ascii { ch.to_ascii_lowercase() } else { ch });
        }
        remaining = rest;
    }
    debug_assert_eq!(output.len(), length);
    Ok(output)
}

fn copy(input: &str, check: Cancel<'_>) -> Result<String, String> {
    let mut output = String::new();
    output.try_reserve_exact(input.len()).map_err(|_| "Cannot reserve HTML token copy.")?;
    let mut remaining = input;
    while !remaining.is_empty() {
        poll(check)?;
        let end = remaining.floor_char_boundary(remaining.len().min(CANCEL_INTERVAL));
        let (chunk, rest) = remaining.split_at(end);
        output.push_str(chunk);
        remaining = rest;
    }
    Ok(output)
}

fn cached_attribute(attrs: &[(String, String)], name: &str, check: Cancel<'_>)
    -> Result<Option<String>, String>
{
    for (index, (key, value)) in attrs.iter().enumerate() {
        if index % CANCEL_INTERVAL == 0 { poll(check)?; }
        if key == name { return copy(value, check).map(Some); }
    }
    Ok(None)
}

pub(super) fn read(input: &str, start: usize, checked: bool, check: Cancel<'_>)
    -> Result<TagToken, String>
{
    poll(check)?;
    let mut scan = Scan { bytes: input.as_bytes(), position: start + 1,
        next_check: start.saturating_add(CANCEL_INTERVAL), check, raw_clean: true, nulls: 0 };
    let closing = scan.current() == Some(b'/');
    if closing { scan.advance()?; }
    if !scan.current().is_some_and(|byte| byte.is_ascii_alphabetic()) {
        return Ok(TagToken::NotATag);
    }

    let name_start = scan.position;
    while scan.current().is_some_and(|byte| !space(byte) && !matches!(byte, b'/' | b'>')) {
        scan.advance()?;
    }
    let name = normalize(&input[name_start..scan.position], true, check)?;
    let checked_native_fragment = checked && matches!(name.as_str(), "table" | "textarea");
    let strict = svg_namespace::shape_element(&name) || checked_native_fragment;
    let mut clean_syntax = true;
    let mut self_closing = false;
    let mut attrs: Vec<(String, String)> = Vec::new();
    let mut seen = HashSet::new();
    let mut duplicate_attributes = false;

    while scan.current().is_some() {
        scan.spaces()?;
        match scan.current() {
            None => break,
            Some(b'>') => {
                let id = cached_attribute(&attrs, "id", check)?;
                let href = cached_attribute(&attrs, "href", check)?;
                poll(check)?;
                return Ok(TagToken::Complete(Tag { name, id, href, attrs, closing, self_closing,
                    clean_syntax: strict && clean_syntax && scan.raw_clean && !duplicate_attributes,
                    end: scan.position + 1, duplicate_attributes }));
            }
            Some(b'/') => {
                scan.advance()?;
                self_closing = scan.current() == Some(b'>');
                clean_syntax &= self_closing;
                continue;
            }
            _ => {}
        }

        let attribute_start = scan.position;
        // Before-attribute-name '=' is the first name character. A subsequent
        // '=' enters before-attribute-value, including its real quote boundary.
        if scan.current() == Some(b'=') {
            clean_syntax = false;
            scan.advance()?;
        }
        while let Some(byte) = scan.current() {
            if space(byte) || matches!(byte, b'=' | b'/' | b'>') { break; }
            if matches!(byte, b'"' | b'\'' | b'<') { clean_syntax = false; }
            scan.advance()?;
        }
        let attribute_end = scan.position;
        scan.spaces()?;

        let mut value = "";
        let mut value_has_null = false;
        if scan.current() == Some(b'=') {
            scan.advance()?;
            scan.spaces()?;
            if let Some(quote @ (b'\'' | b'"')) = scan.current() {
                scan.advance()?;
                let value_start = scan.position;
                let previous_nulls = scan.nulls;
                while scan.current().is_some_and(|byte| byte != quote) { scan.advance()?; }
                value = &input[value_start..scan.position];
                value_has_null = scan.nulls != previous_nulls;
                if scan.current().is_some() {
                    scan.advance()?;
                    // After a quoted value, other input is reconsumed in the
                    // before-attribute-name state, but is not strict syntax.
                    clean_syntax &= scan.current().is_some_and(|byte| space(byte) || matches!(byte, b'/' | b'>'));
                }
            } else {
                let value_start = scan.position;
                let previous_nulls = scan.nulls;
                while let Some(byte) = scan.current() {
                    if space(byte) || byte == b'>' { break; }
                    if matches!(byte, b'"' | b'\'' | b'<' | b'=' | b'`') { clean_syntax = false; }
                    scan.advance()?;
                }
                value = &input[value_start..scan.position];
                value_has_null = scan.nulls != previous_nulls;
                clean_syntax &= !value.is_empty();
            }
        }

        let attribute = normalize(&input[attribute_start..attribute_end], true, check)?;
        if seen.contains(&attribute) {
            duplicate_attributes = true;
        } else {
            // Normalize NULL before reference decoding. Its replacement is
            // non-ASCII and cannot begin or extend a character reference.
            let normalized;
            let value = if value_has_null {
                normalized = normalize(value, false, check)?;
                normalized.as_str()
            } else { value };
            let value = named_references::decode(value, true, check)?;
            attrs.try_reserve(1).map_err(|_| "Cannot reserve HTML token attributes.")?;
            seen.try_reserve(1).map_err(|_| "Cannot reserve HTML token duplicate names.")?;
            // Hashing keeps duplicate detection linear in the input size.
            seen.insert(copy(&attribute, check)?);
            attrs.push((attribute, value));
        }
    }

    poll(check)?;
    Ok(if checked_native_fragment {
        TagToken::Unsupported(if name == "table" { "Incomplete HTML fragment table tag." }
            else { "Incomplete HTML fragment TEXTAREA tag." })
    } else { TagToken::Incomplete })
}
