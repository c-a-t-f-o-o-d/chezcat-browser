//! Original ISO-2022-JP decoder from the pinned WHATWG Encoding algorithms.
//!
//! A completed designation is retained here. An unfinished escape or JIS pair
//! stays in the caller's fixed prefix buffer, so an empty final call can process
//! EOF without hiding an unfinished byte in a separate state machine. Failed
//! escapes consume only ESC and restore the rest in the last designated mode.

use super::multibyte::jis0208_index;

#[cfg(test)]
#[path = "text_encoding_iso2022jp_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Ascii,
    Roman,
    Katakana,
    Jis0208,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Decoder {
    mode: Mode,
    output: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Step {
    Character(char, usize),
    Continue(usize),
    Invalid(usize),
    Incomplete,
}

impl Decoder {
    pub(super) fn new() -> Self {
        Self { mode: Mode::Ascii, output: false }
    }

    pub(super) fn step(&mut self, bytes: &[u8], stream: bool) -> Step {
        let Some(&first) = bytes.first() else { return Step::Incomplete; };
        if first == 0x1b {
            return self.escape(bytes, stream);
        }
        // Reading a JIS lead also clears this flag, even when its trail belongs
        // to a later chunk. The retained prefix reproduces the pending state.
        self.output = false;
        match self.mode {
            Mode::Ascii | Mode::Roman => {
                if first > 0x7f || matches!(first, 0x0e | 0x0f) {
                    Step::Invalid(1)
                } else {
                    let scalar = match (self.mode, first) {
                        (Mode::Roman, 0x5c) => '\u{a5}',
                        (Mode::Roman, 0x7e) => '\u{203e}',
                        _ => char::from(first),
                    };
                    Step::Character(scalar, 1)
                }
            }
            Mode::Katakana => {
                if (0x21..=0x5f).contains(&first) {
                    Step::Character(char::from_u32(0xff61 + u32::from(first - 0x21)).unwrap(), 1)
                } else {
                    Step::Invalid(1)
                }
            }
            Mode::Jis0208 => {
                if !(0x21..=0x7e).contains(&first) {
                    return Step::Invalid(1);
                }
                let Some(&trail) = bytes.get(1) else {
                    return if stream { Step::Incomplete } else { Step::Invalid(1) };
                };
                if trail == 0x1b {
                    // The unmatched lead fails, and ESC begins the next
                    // designation. Reprocessing ESC preserves exactly that
                    // transition without swallowing the following bytes.
                    return Step::Invalid(1);
                }
                if (0x21..=0x7e).contains(&trail) {
                    let pointer = usize::from(first - 0x21) * 94 + usize::from(trail - 0x21);
                    if let Some(scalar) = jis0208_index(pointer) {
                        return Step::Character(scalar, 2);
                    }
                }
                Step::Invalid(2)
            }
        }
    }

    fn escape(&mut self, bytes: &[u8], stream: bool) -> Step {
        let Some(&leading) = bytes.get(1) else {
            if stream { return Step::Incomplete; }
            self.output = false;
            return Step::Invalid(1);
        };
        if !matches!(leading, 0x24 | 0x28) {
            self.output = false;
            return Step::Invalid(1);
        }
        let Some(&last) = bytes.get(2) else {
            if stream { return Step::Incomplete; }
            self.output = false;
            // EOF restores the saved '$' or '(' as data in the prior mode.
            return Step::Invalid(1);
        };
        let mode = match (leading, last) {
            (0x28, 0x42) => Mode::Ascii,
            (0x28, 0x4a) => Mode::Roman,
            (0x28, 0x49) => Mode::Katakana,
            (0x24, 0x40 | 0x42) => Mode::Jis0208,
            _ => {
                self.output = false;
                return Step::Invalid(1);
            }
        };
        let repeated = self.output;
        self.mode = mode;
        self.output = true;
        if repeated { Step::Invalid(3) } else { Step::Continue(3) }
    }
}
