//! WHATWG HTML processing-instruction open, target, after-target, data and
//! questionable states. The token borrows the normalized source until the
//! document has admitted a real node. This is independent of XML parsing.
use super::*;

#[derive(Debug)]
pub(super) enum Token<'a> {
    Instruction { target: &'a str, data: &'a str },
    Comment(&'a str),
    Eof,
}

fn check_at(offset: usize, check: Option<&dyn Fn() -> Result<(), String>>) -> Result<(), String> {
    if offset % 8192 == 0 {
        if let Some(check) = check {
            check()?;
        }
    }
    Ok(())
}

fn delimiter(
    input: &[u8],
    start: usize,
    check: Option<&dyn Fn() -> Result<(), String>>,
) -> Result<Option<usize>, String> {
    for (index, chunk) in input[start..].chunks(8192).enumerate() {
        if let Some(check) = check {
            check()?;
        }
        if let Some(at) = chunk.iter().position(|&b| b == b'>') {
            return Ok(Some(start + index * 8192 + at));
        }
    }
    Ok(None)
}

fn bogus<'a>(
    source: &'a str,
    at: usize,
    check: Option<&dyn Fn() -> Result<(), String>>,
) -> Result<(Token<'a>, usize), String> {
    let end = delimiter(source.as_bytes(), at, check)?.unwrap_or(source.len());
    // The question mark and all buffered target characters are comment data.
    Ok((
        Token::Comment(&source[1..end]),
        end + usize::from(end < source.len()),
    ))
}

pub(super) fn read<'a>(
    source: &'a str,
    check: Option<&dyn Fn() -> Result<(), String>>,
) -> Result<(Token<'a>, usize), String> {
    debug_assert!(source.starts_with("<?"));
    let bytes = source.as_bytes();
    let Some(&first) = bytes.get(2) else {
        return Ok((Token::Eof, 2));
    };
    if !first.is_ascii_alphabetic() && first != b'_' {
        return bogus(source, 2, check);
    }
    let mut at = 2;
    while bytes
        .get(at)
        .is_some_and(|b| b.is_ascii_alphanumeric() || matches!(*b, b'-' | b'_'))
    {
        check_at(at - 2, check)?;
        at += 1;
    }
    let Some(&next) = bytes.get(at) else {
        return Ok((Token::Eof, at));
    };
    let target = &source[2..at];
    if !(matches!(next, 9 | 10 | 12 | 13 | 32 | b'?' | b'>'))
        || target.eq_ignore_ascii_case("xml")
        || target.eq_ignore_ascii_case("xml-stylesheet")
    {
        return bogus(source, at, check);
    }
    let start = at;
    while bytes
        .get(at)
        .is_some_and(|b| matches!(*b, 9 | 10 | 12 | 13 | 32))
    {
        check_at(at - start, check)?;
        at += 1;
    }
    let Some(end) = delimiter(bytes, at, check)? else {
        return Ok((Token::Eof, source.len()));
    };
    let data_end = if end > at && bytes[end - 1] == b'?' {
        end - 1
    } else {
        end
    };
    Ok((
        Token::Instruction {
            target,
            data: &source[at..data_end],
        },
        end + 1,
    ))
}

fn copy(text: &str) -> Result<String, String> {
    let mut value = String::new();
    value
        .try_reserve_exact(text.len())
        .map_err(|_| "Cannot reserve HTML processing instruction data.")?;
    value.push_str(text);
    Ok(value)
}

impl Token<'_> {
    pub(super) fn into_kind(self) -> Result<Option<NodeKind>, String> {
        Ok(match self {
            Self::Instruction { target, data } => Some(NodeKind::ProcessingInstruction {
                target: copy(target)?,
                data: copy(data)?.into(),
            }),
            Self::Comment(data) => Some(NodeKind::Comment(comments::copy_data(data)?.into())),
            Self::Eof => None,
        })
    }
}

#[cfg(test)]
#[path="html_processing_instruction_tests.rs"]
mod tests;
