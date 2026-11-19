//! Original bounded regular-expression bytecode for the JavaScript subset.
//! No external engine. Matching uses UTF-16 positions and an explicit, bounded
//! backtracking stack. Compiler visits, instructions and copied checkpoints
//! spend one shared document-job work budget. Unsupported syntax never becomes a broader match.

// Flat alternation tables reach tens of KiB without deeper syntax or a
// deeper matching stack. Keep both source and expansion explicitly bounded.
const MAX_PATTERN: usize = 32 * 1024;
const MAX_CODE: usize = 32 * 1024;
const PATTERN_LIMIT: &str = "Regular-expression pattern limit reached (32 KiB).";
const MAX_DEPTH: usize = 32;
const MAX_GROUPS: usize = 16;
const MAX_GROUP_NAME_BYTES: usize = 256;
const MAX_MARKS: usize = 64;
const MAX_STACK: usize = 1024;

#[cfg(test)]
#[path = "regexp_utf16_tests.rs"]
mod utf16_tests;

#[derive(Debug)]
pub enum Error {
    Syntax(&'static str),
    Unsupported(&'static str),
    Limit(&'static str),
}

impl Error {
    pub fn message(&self) -> &'static str {
        match self {
            Self::Syntax(message) | Self::Unsupported(message) | Self::Limit(message) => message,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Flags {
    pub global: bool,
    pub insensitive: bool,
    pub multiline: bool,
    pub dot_all: bool,
    pub unicode: bool,
    pub sticky: bool,
}

impl Flags {
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut flags = Self::default();
        for ch in text.chars() {
            let entry = match ch {
                'g' => &mut flags.global,
                'i' => &mut flags.insensitive,
                'm' => &mut flags.multiline,
                's' => &mut flags.dot_all,
                'u' => &mut flags.unicode,
                'y' => &mut flags.sticky,
                'v' | 'd' => {
                    return Err(Error::Unsupported(
                        "Unicode sets and indices regexp flags are not implemented.",
                    ))
                }
                _ => return Err(Error::Syntax("Invalid regular-expression flag.")),
            };
            if *entry {
                return Err(Error::Syntax("Duplicate regular-expression flag."));
            }
            *entry = true;
        }
        if flags.unicode && flags.insensitive {
            return Err(Error::Unsupported("Unicode regexp case folding is not implemented."));
        }
        Ok(flags)
    }

    pub fn text(self) -> String {
        [
            (self.global, 'g'),
            (self.insensitive, 'i'),
            (self.multiline, 'm'),
            (self.dot_all, 's'),
            (self.unicode, 'u'),
            (self.sticky, 'y'),
        ]
        .into_iter()
        .filter_map(|(set, ch)| set.then_some(ch))
        .collect()
    }
}

fn surrogate(unit: u32) -> bool {
    (0xd800..=0xdfff).contains(&unit)
}

fn starts_surrogate_pair(input: &[u16], at: usize) -> bool {
    matches!(input.get(at..at.saturating_add(2)), Some([0xd800..=0xdbff, 0xdc00..=0xdfff]))
}

fn character_at(input: &[u16], at: usize, unicode: bool) -> Option<(u32, usize)> {
    let first = u32::from(*input.get(at)?);
    if unicode && starts_surrogate_pair(input, at) {
        let second = u32::from(input[at + 1]);
        Some((0x10000 + ((first - 0xd800) << 10) + second - 0xdc00, 2))
    } else {
        Some((first, 1))
    }
}

pub(super) fn advance_string_index(input: &[u16], at: usize, unicode: bool) -> usize {
    at + if unicode && starts_surrogate_pair(input, at) { 2 } else { 1 }
}

#[derive(Clone, Debug)]
enum ClassPart {
    Range(u32, u32),
    Digit(bool),
    Word(bool),
    Space(bool),
}

#[derive(Clone, Debug)]
struct Class {
    inverted: bool,
    parts: Vec<ClassPart>,
}

fn line(unit: u32) -> bool {
    matches!(unit, 10 | 13 | 0x2028 | 0x2029)
}
fn word(unit: u32) -> bool {
    matches!(unit, 48..=57 | 65..=90 | 95 | 97..=122)
}
fn space(unit: u32) -> bool {
    matches!(unit, 9..=13 | 32 | 0xa0 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000 | 0xfeff)
}
fn ascii_fold(unit: u32) -> u32 {
    if (97..=122).contains(&unit) {
        unit - 32
    } else {
        unit
    }
}

impl Class {
    fn matches(&self, unit: u32, insensitive: bool, budget: &mut usize) -> Result<bool, Error> {
        spend(budget, self.parts.len().max(1))?;
        let found = self.parts.iter().any(|part| match *part {
            ClassPart::Range(low, high) => {
                (low..=high).contains(&unit)
                    || insensitive
                        && (65..=90).contains(&ascii_fold(unit))
                        && ((low..=high).contains(&ascii_fold(unit))
                            || (low..=high).contains(&(ascii_fold(unit) + 32)))
            }
            ClassPart::Digit(invert) => (48..=57).contains(&unit) != invert,
            ClassPart::Word(invert) => word(unit) != invert,
            ClassPart::Space(invert) => space(unit) != invert,
        });
        Ok(found != self.inverted)
    }
}

#[derive(Debug)]
enum Node {
    Empty,
    Character(u32),
    Class(Class),
    Any,
    Start,
    End,
    Boundary(bool),
    Reference(Reference),
    Sequence(Vec<usize>),
    Alternate(Vec<usize>),
    Group {
        body: usize,
        capture: Option<usize>,
        first: usize,
        end: usize,
    },
    Look {
        body: usize,
        positive: bool,
        backwards: bool,
        first: usize,
        end: usize,
    },
    Repeat {
        body: usize,
        min: usize,
        max: Option<usize>,
        greedy: bool,
    },
}

#[derive(Debug)]
enum Reference {
    Number(usize),
    Name(String),
}

struct NamedCapture {
    name: String,
    index: usize,
    // Two equal names are permitted only when an enclosing disjunction puts
    // them in different alternatives. Each path is at most MAX_DEPTH long.
    alternatives: Vec<(usize, usize)>,
}

struct Parser {
    units: Vec<u16>,
    at: usize,
    nodes: Vec<Node>,
    groups: usize,
    depth: usize,
    insensitive: bool,
    unicode: bool,
    named_mode: bool,
    names: Vec<NamedCapture>,
    alternatives: Vec<(usize, usize)>,
    next_alternative: usize,
}

impl Parser {
    fn push(&mut self, node: Node) -> Result<usize, Error> {
        if self.nodes.len() == MAX_CODE {
            return Err(Error::Limit("Regular-expression syntax limit reached."));
        }
        let id = self.nodes.len();
        self.nodes.push(node);
        Ok(id)
    }
    fn peek(&self) -> Option<u16> {
        self.units.get(self.at).copied()
    }
    fn eat(&mut self, unit: u16) -> bool {
        if self.peek() == Some(unit) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn alternate(&mut self) -> Result<usize, Error> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(Error::Limit(
                "Regular-expression nesting limit reached (32).",
            ));
        }
        let id = self.next_alternative;
        self.next_alternative += 1;
        self.alternatives.push((id, 0));
        let mut branches = vec![self.sequence()?];
        while self.eat(b'|' as u16) {
            self.alternatives.last_mut().unwrap().1 += 1;
            branches.push(self.sequence()?);
        }
        self.alternatives.pop();
        self.depth -= 1;
        if branches.len() == 1 {
            Ok(branches[0])
        } else {
            self.push(Node::Alternate(branches))
        }
    }
    fn sequence(&mut self) -> Result<usize, Error> {
        let mut parts = Vec::new();
        while self
            .peek()
            .is_some_and(|u| u != b')' as u16 && u != b'|' as u16)
        {
            let atom = self.atom()?;
            let min_max = match self.peek() {
                Some(42) => {
                    self.at += 1;
                    Some((0, None))
                }
                Some(43) => {
                    self.at += 1;
                    Some((1, None))
                }
                Some(63) => {
                    self.at += 1;
                    Some((0, Some(1)))
                }
                Some(123) if self.has_counted(self.at) => Some(self.counted()?),
                _ => None,
            };
            if let Some((min, max)) = min_max {
                if matches!(
                    self.nodes[atom],
                    Node::Start
                        | Node::End
                        | Node::Boundary(_)
                        | Node::Look {
                            backwards: true,
                            ..
                        }
                ) {
                    return Err(Error::Syntax(
                        "A regular-expression assertion cannot be quantified.",
                    ));
                }
                let greedy = !self.eat(b'?' as u16);
                parts.push(self.push(Node::Repeat {
                    body: atom,
                    min,
                    max,
                    greedy,
                })?);
            } else {
                parts.push(atom);
            }
        }
        match parts.len() {
            0 => self.push(Node::Empty),
            1 => Ok(parts[0]),
            _ => self.push(Node::Sequence(parts)),
        }
    }
    // Annex B treats an opening brace as a literal unless the whole braced
    // quantifier is present. Recognize syntax before applying numeric bounds;
    // incomplete huge decimal strings are literals, not oversized repetitions.
    fn has_counted(&self, mut at: usize) -> bool {
        if self.units.get(at) != Some(&123) { return false; }
        at += 1;
        let start = at;
        while matches!(self.units.get(at), Some(48..=57)) { at += 1; }
        if at == start { return false; }
        if self.units.get(at) == Some(&44) {
            at += 1;
            while matches!(self.units.get(at), Some(48..=57)) { at += 1; }
        }
        self.units.get(at) == Some(&125)
    }

    fn decimal(&mut self) -> Result<usize, Error> {
        let start = self.at;
        let mut number = 0usize;
        while let Some(unit @ 48..=57) = self.peek() {
            number = number
                .saturating_mul(10)
                .saturating_add((unit - 48) as usize);
            self.at += 1;
            if number > 1000 {
                return Err(Error::Limit(
                    "Regular-expression repetition limit reached (1000).",
                ));
            }
        }
        if self.at == start {
            Err(Error::Syntax("A counted repetition needs a number."))
        } else {
            Ok(number)
        }
    }
    fn counted(&mut self) -> Result<(usize, Option<usize>), Error> {
        self.at += 1;
        let min = self.decimal()?;
        let max = if self.eat(b',' as u16) {
            if self.peek() == Some(b'}' as u16) {
                None
            } else {
                Some(self.decimal()?)
            }
        } else {
            Some(min)
        };
        if !self.eat(b'}' as u16) || max.is_some_and(|n| n < min) {
            return Err(Error::Syntax("Invalid regular-expression repetition."));
        }
        Ok((min, max))
    }
    fn atom(&mut self) -> Result<usize, Error> {
        let unit = self
            .peek()
            .ok_or(Error::Syntax("Missing regular-expression atom."))?;
        self.at += 1;
        match unit {
            46 if self.unicode => Err(Error::Unsupported("Unicode regexp dot is not implemented.")),
            46 => self.push(Node::Any),
            94 => self.push(Node::Start),
            36 => self.push(Node::End),
            91 => {
                let class = self.class()?;
                self.push(Node::Class(class))
            }
            40 => {
                let first = self.groups + 1;
                let mut assertion = None;
                let capture = if self.eat(b'?' as u16) {
                    if self.eat(b':' as u16) {
                        None
                    } else if self.unicode {
                        return Err(Error::Unsupported("Unicode regexp assertions and named groups are not implemented."));
                    } else if self.eat(b'=' as u16) {
                        assertion = Some((true, false));
                        None
                    } else if self.eat(b'!' as u16) {
                        assertion = Some((false, false));
                        None
                    } else if self.eat(b'<' as u16) {
                        if self.eat(b'=' as u16) {
                            assertion = Some((true, true));
                            None
                        } else if self.eat(b'!' as u16) {
                            assertion = Some((false, true));
                            None
                        } else {
                            let name = self.group_name()?;
                            Some(self.capture(Some(name))?)
                        }
                    } else {
                        return Err(Error::Unsupported(
                            "Regexp inline modifiers or this group extension are not implemented.",
                        ));
                    }
                } else {
                    Some(self.capture(None)?)
                };
                let body = self.alternate()?;
                if !self.eat(b')' as u16) {
                    return Err(Error::Syntax("Unterminated regular-expression group."));
                }
                if let Some((positive, backwards)) = assertion {
                    self.push(Node::Look {
                        body,
                        positive,
                        backwards,
                        first,
                        end: self.groups + 1,
                    })
                } else {
                    self.push(Node::Group {
                        body,
                        capture,
                        first,
                        end: self.groups + 1,
                    })
                }
            }
            92 => {
                if self.peek() == Some(b'b' as u16) {
                    self.at += 1;
                    return self.push(Node::Boundary(false));
                }
                if self.peek() == Some(b'B' as u16) {
                    self.at += 1;
                    return self.push(Node::Boundary(true));
                }
                if self.peek() == Some(b'k' as u16) && self.named_mode {
                    self.at += 1;
                    if !self.eat(b'<' as u16) {
                        return Err(Error::Syntax(
                            "A named regexp reference needs a group name.",
                        ));
                    }
                    let name = self.group_name()?;
                    return self.push(Node::Reference(Reference::Name(name)));
                }
                if self.peek().is_some_and(|unit| (49..=57).contains(&unit)) {
                    if self.unicode {
                        return Err(Error::Unsupported("Unicode regexp backreferences are not implemented."));
                    }
                    let mut number = 0usize;
                    while let Some(unit @ 48..=57) = self.peek() {
                        number = number
                            .saturating_mul(10)
                            .saturating_add((unit - 48) as usize);
                        self.at += 1;
                    }
                    return self.push(Node::Reference(Reference::Number(number)));
                }
                let part = self.escape(false)?;
                match part {
                    ClassPart::Range(low, high) if low == high => self.character(low),
                    other => self.push(Node::Class(Class {
                        inverted: false,
                        parts: vec![other],
                    })),
                }
            }
            // Complete braced quantifiers without a preceding atom are invalid.
            // Other braces/brackets are literal ExtendedPatternCharacters in
            // the supported non-Unicode grammar (Annex B).
            123 if self.unicode || self.has_counted(self.at - 1) => Err(Error::Syntax(
                "Unexpected regular-expression quantifier or delimiter.",
            )),
            93 | 125 if self.unicode => Err(Error::Syntax("Unexpected Unicode regexp delimiter.")),
            42 | 43 | 63 => Err(Error::Syntax(
                "Unexpected regular-expression quantifier or delimiter.",
            )),
            _ => {
                let character = self.raw_character();
                self.character(character)
            }
        }
    }
    fn capture(&mut self, name: Option<String>) -> Result<usize, Error> {
        self.groups += 1;
        if self.groups > MAX_GROUPS {
            return Err(Error::Limit(
                "Regular-expression capture limit reached (16).",
            ));
        }
        if let Some(name) = name {
            for previous in self.names.iter().filter(|capture| capture.name == name) {
                let separate = previous.alternatives.iter().any(|(id, branch)| {
                    self.alternatives
                        .iter()
                        .any(|(other_id, other_branch)| id == other_id && branch != other_branch)
                });
                if !separate {
                    return Err(Error::Syntax(
                        "Duplicate regexp group names may occur only in separate alternatives.",
                    ));
                }
            }
            self.names.push(NamedCapture {
                name,
                index: self.groups,
                alternatives: self.alternatives.clone(),
            });
        }
        Ok(self.groups)
    }

    fn group_name(&mut self) -> Result<String, Error> {
        let mut name = String::new();
        while self.peek() != Some(b'>' as u16) {
            let mut unit = self
                .peek()
                .ok_or(Error::Syntax("Unterminated regexp group name."))?;
            self.at += 1;
            let escaped = unit == b'\\' as u16;
            if escaped {
                if !self.eat(b'u' as u16) {
                    return Err(Error::Syntax(
                        "A regexp group name allows only Unicode escapes.",
                    ));
                }
                unit = if self.eat(b'{' as u16) {
                    let mut value = 0u32;
                    let mut digits = 0usize;
                    while self.peek() != Some(b'}' as u16) {
                        let digit = self
                            .peek()
                            .and_then(|u| char::from_u32(u as u32))
                            .and_then(|c| c.to_digit(16))
                            .ok_or(Error::Syntax("Invalid regexp group-name Unicode escape."))?;
                        self.at += 1;
                        digits += 1;
                        value = value.saturating_mul(16).saturating_add(digit);
                        if value > 0x10ffff {
                            return Err(Error::Syntax("Invalid regexp group-name Unicode scalar."));
                        }
                    }
                    self.at += 1;
                    if digits == 0 {
                        return Err(Error::Syntax("Empty regexp group-name Unicode escape."));
                    }
                    if (0xd800..=0xdfff).contains(&value) {
                        return Err(Error::Syntax("Unpaired surrogate in regexp group name."));
                    }
                    if value > 127 {
                        return Err(Error::Unsupported(
                            "Non-ASCII regexp group names are not implemented.",
                        ));
                    }
                    value as u16
                } else {
                    self.hex(4)?
                };
            }
            if (0xd800..=0xdbff).contains(&unit) {
                // A source code point consists of consecutive literal units.
                // A four-digit UnicodeEscapeSequence can pair only with another
                // such escape, never with a raw unit or braced escape.
                let paired = if escaped {
                    self.eat(b'\\' as u16) && self.eat(b'u' as u16)
                        && (0xdc00..=0xdfff).contains(&self.hex(4)?)
                } else {
                    self.peek().is_some_and(|low| (0xdc00..=0xdfff).contains(&low))
                };
                if !paired {
                    return Err(Error::Syntax("Unpaired surrogate in regexp group name."));
                }
            } else if (0xdc00..=0xdfff).contains(&unit) {
                return Err(Error::Syntax("Unpaired surrogate in regexp group name."));
            }
            if unit >= 128 {
                return Err(Error::Unsupported(
                    "Non-ASCII regexp group names are not implemented.",
                ));
            }
            let valid = matches!(unit, 36 | 95 | 65..=90 | 97..=122)
                || !name.is_empty() && matches!(unit, 48..=57);
            if !valid {
                return Err(Error::Syntax("Invalid regexp group name."));
            }
            if name.len() == MAX_GROUP_NAME_BYTES {
                return Err(Error::Limit("Regexp group-name limit reached (256 bytes)."));
            }
            name.push(unit as u8 as char);
        }
        self.at += 1;
        if name.is_empty() {
            return Err(Error::Syntax("A regexp group name cannot be empty."));
        }
        if name == "__proto__" {
            return Err(Error::Unsupported(
                "Protected regexp group names are not implemented.",
            ));
        }
        Ok(name)
    }
    fn raw_character(&mut self) -> u32 {
        let (character, width) = character_at(&self.units, self.at - 1, self.unicode).unwrap();
        self.at += width - 1;
        character
    }

    fn character(&mut self, unit: u32) -> Result<usize, Error> {
        if self.unicode && surrogate(unit) {
            return Err(Error::Unsupported("Unicode regexp surrogate literals are not implemented."));
        }
        if self.insensitive && unit >= 128 {
            return Err(Error::Unsupported(
                "Case-insensitive non-ASCII regexp literals/ranges are not implemented.",
            ));
        }
        self.push(Node::Character(unit))
    }
    fn hex(&mut self, count: usize) -> Result<u16, Error> {
        let mut value = 0;
        for _ in 0..count {
            let unit = self
                .peek()
                .ok_or(Error::Syntax("Incomplete regexp hexadecimal escape."))?;
            self.at += 1;
            let digit = char::from_u32(unit as u32)
                .and_then(|c| c.to_digit(16))
                .ok_or(Error::Syntax("Invalid regexp hexadecimal escape."))?;
            value = value * 16 + digit as u16;
        }
        Ok(value)
    }
    fn braced_unicode_escape(&mut self) -> Result<u32, Error> {
        let start = self.at;
        let mut value = 0;
        while let Some(digit) = self.peek().and_then(|unit| char::from_u32(u32::from(unit)))
            .and_then(|character| character.to_digit(16))
        {
            value = value * 16 + digit;
            if value > 0x10ffff {
                return Err(Error::Syntax("Unicode regexp escape exceeds U+10FFFF."));
            }
            self.at += 1;
        }
        if self.at == start || !self.eat(b'}' as u16) {
            return Err(Error::Syntax("Invalid braced Unicode regexp escape."));
        }
        Ok(value)
    }
    fn escape(&mut self, in_class: bool) -> Result<ClassPart, Error> {
        let unit = self
            .peek()
            .ok_or(Error::Syntax("Trailing regular-expression escape."))?;
        self.at += 1;
        let literal = match unit {
            68 | 87 | 83 if self.unicode => {
                return Err(Error::Unsupported("Unicode regexp complemented character sets are not implemented."));
            }
            100 | 68 => return Ok(ClassPart::Digit(unit == 68)),
            119 | 87 => return Ok(ClassPart::Word(unit == 87)),
            115 | 83 => return Ok(ClassPart::Space(unit == 83)),
            110 => 10,
            114 => 13,
            116 => 9,
            118 => 11,
            102 => 12,
            98 if in_class => 8,
            120 => u32::from(self.hex(2)?),
            117 if self.unicode && self.eat(b'{' as u16) => self.braced_unicode_escape()?,
            117 => u32::from(self.hex(4)?),
            99 => {
                let next = self
                    .peek()
                    .ok_or(Error::Syntax("Incomplete regexp control escape."))?;
                if !matches!(next, 65..=90 | 97..=122) {
                    return Err(Error::Syntax("Invalid regexp control escape."));
                }
                self.at += 1;
                u32::from(next % 32)
            }
            48 => {
                if self.peek().is_some_and(|u| (48..=57).contains(&u)) {
                    return Err(Error::Unsupported(
                        "Legacy regexp octal escapes are not implemented.",
                    ));
                }
                0
            }
            49..=57 => {
                return Err(Error::Unsupported(
                    "Legacy regexp octal/decimal class escapes are not implemented.",
                ))
            }
            107 if self.named_mode => {
                return Err(Error::Syntax(
                    "A named regexp escape is invalid in a character class.",
                ))
            }
            112 | 80 => {
                return Err(Error::Unsupported(
                    "Unicode regexp properties are not implemented.",
                ))
            }
            _ if self.unicode && (unit != b'-' as u16 || !in_class)
                && !b"^$\\.*+?()[]{}|/".iter().any(|&syntax| unit == u16::from(syntax)) =>
                return Err(Error::Syntax("Invalid Unicode regexp identity escape.")),
            _ => u32::from(unit),
        };
        if self.unicode && surrogate(literal) {
            return Err(Error::Unsupported("Unicode regexp surrogate literals are not implemented."));
        }
        if self.insensitive && literal >= 128 {
            return Err(Error::Unsupported(
                "Case-insensitive non-ASCII regexp literals/ranges are not implemented.",
            ));
        }
        Ok(ClassPart::Range(literal, literal))
    }
    fn class_atom(&mut self) -> Result<ClassPart, Error> {
        let unit = self
            .peek()
            .ok_or(Error::Syntax("Unterminated regular-expression class."))?;
        self.at += 1;
        if unit == 92 {
            self.escape(true)
        } else {
            let unit = self.raw_character();
            if self.unicode && surrogate(unit) {
                return Err(Error::Unsupported("Unicode regexp surrogate literals are not implemented."));
            }
            if self.insensitive && unit >= 128 {
                return Err(Error::Unsupported(
                    "Case-insensitive non-ASCII regexp literals/ranges are not implemented.",
                ));
            }
            Ok(ClassPart::Range(unit, unit))
        }
    }
    fn class(&mut self) -> Result<Class, Error> {
        let inverted = self.eat(b'^' as u16);
        if inverted && self.unicode {
            return Err(Error::Unsupported("Unicode regexp inverted classes are not implemented."));
        }
        let mut parts = Vec::new();
        while self.peek() != Some(b']' as u16) {
            let start = self.class_atom()?;
            if self.peek() == Some(b'-' as u16)
                && self.units.get(self.at + 1) != Some(&(b']' as u16))
            {
                self.at += 1;
                let end = self.class_atom()?;
                match (start, end) {
                    (ClassPart::Range(low, _), ClassPart::Range(_, high)) => {
                        if low > high {
                            return Err(Error::Syntax("Reversed regular-expression range."));
                        }
                        parts.push(ClassPart::Range(low, high));
                    }
                    (start, end) => {
                        if self.unicode {
                            return Err(Error::Syntax("Unicode regexp ranges require character endpoints."));
                        }
                        // Annex B CharacterRangeOrUnion. This parser receives
                        // classic patterns after flag validation.
                        parts.extend([start, ClassPart::Range(u32::from(b'-'), u32::from(b'-')), end]);
                    }
                }
            } else {
                parts.push(start);
            }
        }
        self.at += 1;
        Ok(Class { inverted, parts })
    }
}

#[derive(Debug)]
enum Atom {
    Character(u32),
    Class(Class),
    Any,
}
impl Atom {
    fn matches(&self, unit: u32, flags: Flags, budget: &mut usize) -> Result<bool, Error> {
        spend(budget, 1)?;
        match self {
            Self::Character(expected) => {
                Ok(unit == *expected
                    || flags.insensitive && ascii_fold(unit) == ascii_fold(*expected))
            }
            Self::Class(class) => class.matches(unit, flags.insensitive, budget),
            Self::Any => Ok(flags.dot_all || !line(unit)),
        }
    }
}

#[derive(Debug)]
enum Instruction {
    Character(u32),
    Class(Class),
    Any,
    Start,
    End,
    Boundary(bool),
    Reference(Vec<usize>),
    Save(usize),
    Clear(usize, usize),
    Mark(usize),
    Progress(usize),
    Scan {
        atom: Atom,
        min: usize,
        max: Option<usize>,
        greedy: bool,
    },
    Split(usize, usize),
    Jump(usize),
    Look {
        begin: usize,
        after: usize,
        positive: bool,
        backwards: bool,
    },
    Accept,
}

struct Compiler<'a> {
    code: Vec<Instruction>,
    marks: usize,
    budget: &'a mut usize,
    names: &'a [NamedCapture],
    groups: usize,
    unicode: bool,
}
impl Compiler<'_> {
    fn emit(&mut self, instruction: Instruction) -> Result<usize, Error> {
        spend(self.budget, 1)?;
        if self.code.len() == MAX_CODE {
            return Err(Error::Limit(
                "Regular-expression bytecode limit reached (32768).",
            ));
        }
        let at = self.code.len();
        self.code.push(instruction);
        Ok(at)
    }
    fn node(&mut self, nodes: &[Node], id: usize, backwards: bool) -> Result<(), Error> {
        // Empty counted groups emit no instructions. Count node visits as well
        // as emitted code so nested repetitions cannot expand without limit.
        spend(self.budget, 1)?;
        match &nodes[id] {
            Node::Empty => {}
            Node::Character(unit) => {
                self.emit(Instruction::Character(*unit))?;
            }
            Node::Class(class) => {
                spend(
                    self.budget,
                    class.parts.len() * std::mem::size_of::<ClassPart>(),
                )?;
                self.emit(Instruction::Class(class.clone()))?;
            }
            Node::Any => {
                self.emit(Instruction::Any)?;
            }
            Node::Start => {
                self.emit(Instruction::Start)?;
            }
            Node::End => {
                self.emit(Instruction::End)?;
            }
            Node::Boundary(inverted) => {
                self.emit(Instruction::Boundary(*inverted))?;
            }
            Node::Reference(reference) => {
                let indices = match reference {
                    Reference::Number(index) if *index <= self.groups => vec![*index],
                    Reference::Number(_) => {
                        return Err(Error::Unsupported(
                            "Legacy regexp octal/decimal escapes are not implemented.",
                        ))
                    }
                    Reference::Name(name) => {
                        let indices: Vec<_> = self
                            .names
                            .iter()
                            .filter(|capture| &capture.name == name)
                            .map(|capture| capture.index)
                            .collect();
                        if indices.is_empty() {
                            return Err(Error::Syntax("Unknown named regexp capture reference."));
                        }
                        indices
                    }
                };
                spend(self.budget, indices.len() * std::mem::size_of::<usize>())?;
                self.emit(Instruction::Reference(indices))?;
            }
            Node::Sequence(parts) => {
                if backwards {
                    for &part in parts.iter().rev() {
                        self.node(nodes, part, backwards)?;
                    }
                } else {
                    for &part in parts {
                        self.node(nodes, part, backwards)?;
                    }
                }
            }
            Node::Alternate(parts) => {
                let mut jumps = Vec::new();
                for (index, &part) in parts.iter().enumerate() {
                    let split = if index + 1 < parts.len() {
                        Some(self.emit(Instruction::Split(0, 0))?)
                    } else {
                        None
                    };
                    let begin = self.code.len();
                    self.node(nodes, part, backwards)?;
                    if let Some(split) = split {
                        jumps.push(self.emit(Instruction::Jump(0))?);
                        self.code[split] = Instruction::Split(begin, self.code.len());
                    }
                }
                for jump in jumps {
                    self.code[jump] = Instruction::Jump(self.code.len());
                }
            }
            Node::Group {
                body,
                capture,
                first,
                end,
            } => {
                if first < end {
                    self.emit(Instruction::Clear(first * 2, end * 2))?;
                }
                if let Some(capture) = capture {
                    self.emit(Instruction::Save(capture * 2 + usize::from(backwards)))?;
                }
                self.node(nodes, *body, backwards)?;
                if let Some(capture) = capture {
                    self.emit(Instruction::Save(capture * 2 + usize::from(!backwards)))?;
                }
            }
            Node::Look {
                body,
                positive,
                backwards,
                first,
                end,
            } => {
                if first < end {
                    self.emit(Instruction::Clear(first * 2, end * 2))?;
                }
                let look = self.emit(Instruction::Accept)?;
                let begin = self.code.len();
                self.node(nodes, *body, *backwards)?;
                self.emit(Instruction::Accept)?;
                self.code[look] = Instruction::Look {
                    begin,
                    after: self.code.len(),
                    positive: *positive,
                    backwards: *backwards,
                };
            }
            Node::Repeat {
                body,
                min,
                max,
                greedy,
            } => {
                let atom = match &nodes[*body] {
                    Node::Character(unit) if !self.unicode || *unit <= 0xffff => Some(Atom::Character(*unit)),
                    Node::Class(class) if !self.unicode || !class.inverted && class.parts.iter().all(|part| match *part {
                        ClassPart::Range(low, high) => high <= 0xffff && (high < 0xd800 || low > 0xdfff),
                        ClassPart::Digit(inverted) | ClassPart::Word(inverted) | ClassPart::Space(inverted) => !inverted,
                    }) => {
                        spend(
                            self.budget,
                            class.parts.len() * std::mem::size_of::<ClassPart>(),
                        )?;
                        Some(Atom::Class(class.clone()))
                    }
                    Node::Any if !self.unicode => Some(Atom::Any),
                    _ => None,
                };
                if let Some(atom) = atom {
                    self.emit(Instruction::Scan {
                        atom,
                        min: *min,
                        max: *max,
                        greedy: *greedy,
                    })?;
                    return Ok(());
                }
                for _ in 0..*min {
                    self.node(nodes, *body, backwards)?;
                }
                let count = max.map(|n| n - min);
                if count == Some(0) {
                    return Ok(());
                }
                let mark = self.marks;
                self.marks += 1;
                if self.marks > MAX_MARKS {
                    return Err(Error::Limit(
                        "Regular-expression repetition nesting limit reached.",
                    ));
                }
                let mut exits = Vec::new();
                for _ in 0..count.unwrap_or(1) {
                    let split = self.emit(Instruction::Split(0, 0))?;
                    let begin = self.code.len();
                    self.emit(Instruction::Mark(mark))?;
                    self.node(nodes, *body, backwards)?;
                    self.emit(Instruction::Progress(mark))?;
                    if count.is_none() {
                        self.emit(Instruction::Jump(split))?;
                    }
                    exits.push((split, begin));
                }
                for (split, begin) in exits {
                    self.code[split] = if *greedy {
                        Instruction::Split(begin, self.code.len())
                    } else {
                        Instruction::Split(self.code.len(), begin)
                    };
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Regex {
    code: Vec<Instruction>,
    pub flags: Flags,
    pub source: String,
    /// Escaped source when it contains unpaired UTF-16 units. The scalar
    /// representation stays unchanged for ordinary patterns.
    pub source_units: Option<Vec<u16>>,
    groups: usize,
    capture_names: Vec<Option<String>>,
    marks: usize,
    pub retained_bytes: usize,
}

#[derive(Clone)]
struct State {
    pc: usize,
    position: usize,
    captures: Vec<Option<usize>>,
    marks: Vec<Option<usize>>,
}

struct Choice {
    state: State,
    // Simple atom repetitions keep one checkpoint for a range of input
    // positions instead of allocating one checkpoint per matched character.
    positions: Option<(usize, usize, bool)>,
}

#[derive(Debug)]
pub struct Match {
    pub captures: Vec<Option<(usize, usize)>>,
    pub named_captures: Vec<(String, Option<(usize, usize)>)>,
}
impl Match {
    pub fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.captures.len() * std::mem::size_of::<Option<(usize, usize)>>()
            + self.named_captures.len() * std::mem::size_of::<(String, Option<(usize, usize)>)>()
            + self
                .named_captures
                .iter()
                .map(|(name, _)| name.len())
                .sum::<usize>()
    }
    pub fn start(&self) -> usize {
        self.captures[0].unwrap().0
    }
    pub fn end(&self) -> usize {
        self.captures[0].unwrap().1
    }
}

fn spend(budget: &mut usize, count: usize) -> Result<(), Error> {
    if count > *budget {
        *budget = 0;
        return Err(Error::Limit("Regular-expression work limit reached."));
    }
    *budget -= count;
    Ok(())
}

fn has_named_capture(units: &[u16]) -> bool {
    // NamedCaptureGroups is a whole-pattern grammar parameter: a forward \k
    // reference becomes meaningful even when its group appears later. Ignore
    // escaped characters and character classes during this bounded first scan.
    let mut at = 0;
    let mut in_class = false;
    while at < units.len() {
        match units[at] {
            92 => at += 2,
            91 if !in_class => {
                in_class = true;
                at += 1;
            }
            93 if in_class => {
                in_class = false;
                at += 1;
            }
            40 if !in_class
                && units.get(at + 1) == Some(&(b'?' as u16))
                && units.get(at + 2) == Some(&(b'<' as u16))
                && !matches!(units.get(at + 3), Some(61 | 33)) =>
            {
                return true
            }
            _ => at += 1,
        }
    }
    false
}

impl Regex {
    pub fn compile(pattern: &str, flags: &str, budget: &mut usize) -> Result<Self, Error> {
        if pattern.len() > MAX_PATTERN {
            return Err(Error::Limit(
                PATTERN_LIMIT,
            ));
        }
        spend(budget, pattern.len().saturating_add(flags.len()))?;
        let flags = Flags::parse(flags)?;
        let units: Vec<_> = pattern.encode_utf16().collect();
        spend(budget, units.len())?;
        Self::compile_prepared(units, flags, budget)
    }

    /// Classic patterns are sequences of code units, including unpaired
    /// surrogates. Never repair or escape those units before parsing: doing so
    /// would change character classes, ranges and their matched captures.
    pub fn compile_units(pattern: &[u16], flags: &str, budget: &mut usize) -> Result<Self, Error> {
        if pattern.len() > MAX_PATTERN {
            return Err(Error::Limit(PATTERN_LIMIT));
        }
        spend(budget, pattern.len())?;
        // Keep the scalar entry point's byte ceiling. A paired surrogate costs
        // four UTF-8 bytes; an unpaired one costs three bytes for this ceiling.
        let mut bytes = 0usize;
        let mut at = 0;
        while at < pattern.len() {
            let unit = pattern[at];
            bytes += match unit {
                0..=0x7f => 1,
                0x80..=0x7ff => 2,
                0xd800..=0xdbff if pattern.get(at + 1).is_some_and(|next| (0xdc00..=0xdfff).contains(next)) => {
                    at += 1;
                    4
                }
                _ => 3,
            };
            at += 1;
        }
        if bytes > MAX_PATTERN {
            return Err(Error::Limit(PATTERN_LIMIT));
        }
        spend(budget, bytes.saturating_add(flags.len()))?;
        let flags = Flags::parse(flags)?;
        Self::compile_prepared(pattern.to_vec(), flags, budget)
    }

    fn compile_prepared(units: Vec<u16>, flags: Flags, budget: &mut usize) -> Result<Self, Error> {
        // The VM retains UTF-16 capture offsets and Unicode search visits
        // only code-point boundaries.
        let named_mode = has_named_capture(&units);
        let mut parser = Parser {
            units,
            at: 0,
            nodes: Vec::new(),
            groups: 0,
            depth: 0,
            insensitive: flags.insensitive,
            unicode: flags.unicode,
            named_mode,
            names: Vec::new(),
            alternatives: Vec::new(),
            next_alternative: 0,
        };
        let root = parser.alternate()?;
        if parser.at != parser.units.len() {
            return Err(Error::Syntax(
                "Unexpected regular-expression closing delimiter.",
            ));
        }
        let mut compiler = Compiler {
            code: Vec::new(),
            marks: 0,
            budget,
            names: &parser.names,
            groups: parser.groups,
            unicode: flags.unicode,
        };
        compiler.emit(Instruction::Save(0))?;
        compiler.node(&parser.nodes, root, false)?;
        compiler.emit(Instruction::Save(1))?;
        compiler.emit(Instruction::Accept)?;
        let mut source = Vec::new();
        let mut escaped = false;
        for &unit in &parser.units {
            if escaped {
                source.push(unit);
                escaped = false;
                continue;
            }
            match unit {
                92 => {
                    source.push(unit);
                    escaped = true;
                }
                47 => source.extend("\\/".encode_utf16()),
                10 => source.extend("\\n".encode_utf16()),
                13 => source.extend("\\r".encode_utf16()),
                0x2028 => source.extend("\\u2028".encode_utf16()),
                0x2029 => source.extend("\\u2029".encode_utf16()),
                _ => source.push(unit),
            }
        }
        if source.is_empty() {
            source.extend("(?:)".encode_utf16());
        }
        let (source, source_units) = match String::from_utf16(&source) {
            Ok(source) => (source, None),
            Err(_) => (String::new(), Some(source)),
        };
        let mut capture_names = vec![None; parser.groups + 1];
        for capture in &parser.names {
            spend(compiler.budget, capture.name.len())?;
            capture_names[capture.index] = Some(capture.name.clone());
        }
        let retained_bytes = source.len()
            + source_units.as_ref().map_or(0, |units| units.len() * std::mem::size_of::<u16>())
            + capture_names.len() * std::mem::size_of::<Option<String>>()
            + capture_names
                .iter()
                .flatten()
                .map(String::len)
                .sum::<usize>()
            + compiler.code.len() * std::mem::size_of::<Instruction>()
            + compiler
                .code
                .iter()
                .map(|instruction| match instruction {
                    Instruction::Class(class)
                    | Instruction::Scan {
                        atom: Atom::Class(class),
                        ..
                    } => class.parts.len() * std::mem::size_of::<ClassPart>(),
                    Instruction::Reference(indices) => indices.len() * std::mem::size_of::<usize>(),
                    _ => 0,
                })
                .sum::<usize>();
        Ok(Self {
            code: compiler.code,
            flags,
            source,
            source_units,
            groups: parser.groups,
            capture_names,
            marks: compiler.marks,
            retained_bytes,
        })
    }

    pub fn find(
        &self,
        input: &[u16],
        start: usize,
        sticky: bool,
        budget: &mut usize,
    ) -> Result<Option<Match>, Error> {
        if start > input.len() {
            return Ok(None);
        }
        let start = if self.flags.unicode && start > 0 && starts_surrogate_pair(input, start - 1) {
            start - 1
        } else { start };
        for beginning in start..=if sticky { start } else { input.len() } {
            if self.flags.unicode && beginning > start && starts_surrogate_pair(input, beginning - 1) {
                continue;
            }
            spend(budget, (self.groups + 1) * 2 + self.marks + 1)?;
            let state = State {
                pc: 0,
                position: beginning,
                captures: vec![None; (self.groups + 1) * 2],
                marks: vec![None; self.marks],
            };
            if let Some(state) = self.run(input, state, false, budget, 0, 0)? {
                let captures: Vec<_> = state
                    .captures
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| match (pair[0], pair[1]) {
                        (Some(a), Some(b)) => Some((a, b)),
                        _ => None,
                    })
                    .collect();
                let mut named_captures: Vec<(String, Option<(usize, usize)>)> = Vec::new();
                for (index, name) in self.capture_names.iter().enumerate() {
                    if let Some(name) = name {
                        spend(budget, name.len() + named_captures.len() + 1)?;
                        if let Some((_, current)) =
                            named_captures.iter_mut().find(|(known, _)| known == name)
                        {
                            if captures[index].is_some() {
                                *current = captures[index];
                            }
                        } else {
                            named_captures.push((name.clone(), captures[index]));
                        }
                    }
                }
                return Ok(Some(Match {
                    captures,
                    named_captures,
                }));
            }
        }
        Ok(None)
    }

    // Assertions are atomic submatches: they can retain captures, but never
    // consume the caller's position or expose internal backtracking choices.
    // Lookbehind traverses characters and sequences in reverse while keeping
    // alternation priority and capture ranges in their original string order.
    fn run(
        &self,
        input: &[u16],
        mut state: State,
        backwards: bool,
        budget: &mut usize,
        depth: usize,
        outer_choices: usize,
    ) -> Result<Option<State>, Error> {
        if depth > MAX_DEPTH {
            return Err(Error::Limit(
                "Regular-expression assertion nesting limit reached (32).",
            ));
        }
        let mut alternatives = Vec::new();
        loop {
            spend(budget, 1)?;
            let following = input.get(state.position).copied().map(u32::from);
            let at = if backwards {
                if self.flags.unicode && state.position >= 2 && starts_surrogate_pair(input, state.position - 2) {
                    Some(state.position - 2)
                } else {
                    state.position.checked_sub(1)
                }
            } else {
                Some(state.position)
            };
            let decoded = at.and_then(|index| character_at(input, index, self.flags.unicode));
            let current = decoded.map(|(character, _)| character);
            let next = &self.code[state.pc];
            let succeeded = match next {
                Instruction::Character(unit) => current.is_some_and(|actual| {
                    actual == *unit
                        || self.flags.insensitive && ascii_fold(actual) == ascii_fold(*unit)
                }),
                Instruction::Class(class) => {
                    if let Some(unit) = current {
                        class.matches(unit, self.flags.insensitive, budget)?
                    } else {
                        false
                    }
                }
                Instruction::Any => current.is_some_and(|unit| self.flags.dot_all || !line(unit)),
                Instruction::Start => {
                    state.position == 0
                        || self.flags.multiline
                            && state.position > 0
                            && line(u32::from(input[state.position - 1]))
                }
                Instruction::End => {
                    state.position == input.len()
                        || self.flags.multiline && following.is_some_and(line)
                }
                Instruction::Boundary(inverted) => {
                    let before = state.position > 0 && word(u32::from(input[state.position - 1]));
                    let after = following.is_some_and(word);
                    (before != after) != *inverted
                }
                Instruction::Reference(indices) => {
                    spend(budget, indices.len())?;
                    let capture = indices.iter().find_map(|index| {
                        match (state.captures[index * 2], state.captures[index * 2 + 1]) {
                            (Some(start), Some(end)) => Some((start, end)),
                            _ => None,
                        }
                    });
                    if let Some((start, end)) = capture {
                        let length = end - start;
                        let target = if backwards {
                            state.position.checked_sub(length)
                        } else if length <= input.len() - state.position {
                            Some(state.position)
                        } else {
                            None
                        };
                        if let Some(target) = target {
                            spend(budget, length.saturating_mul(2))?;
                            if self.flags.insensitive
                                && input[start..end]
                                    .iter()
                                    .chain(&input[target..target + length])
                                    .any(|unit| *unit >= 128)
                            {
                                return Err(Error::Unsupported("Case-insensitive non-ASCII regexp backreferences are not implemented."));
                            }
                            let matched = input[start..end]
                                .iter()
                                .zip(&input[target..target + length])
                                .all(|(a, b)| {
                                    a == b
                                        || self.flags.insensitive
                                            && ascii_fold(u32::from(*a)) == ascii_fold(u32::from(*b))
                                });
                            if matched {
                                state.position = if backwards { target } else { target + length };
                            }
                            matched
                        } else {
                            false
                        }
                    } else {
                        true
                    }
                }
                Instruction::Save(slot) => {
                    state.captures[*slot] = Some(state.position);
                    true
                }
                Instruction::Clear(first, end) => {
                    spend(budget, end - first)?;
                    state.captures[*first..*end].fill(None);
                    true
                }
                Instruction::Mark(mark) => {
                    state.marks[*mark] = Some(state.position);
                    true
                }
                Instruction::Progress(mark) => state.marks[*mark] != Some(state.position),
                Instruction::Scan {
                    atom,
                    min,
                    max,
                    greedy,
                } => {
                    let start = state.position;
                    let available = if backwards {
                        start
                    } else {
                        input.len() - start
                    };
                    let limit = max.unwrap_or(available).min(available);
                    let mut matched = 0usize;
                    while matched < limit {
                        let index = if backwards {
                            start - matched - 1
                        } else {
                            start + matched
                        };
                        if !atom.matches(u32::from(input[index]), self.flags, budget)? {
                            break;
                        }
                        matched += 1;
                    }
                    if matched < *min {
                        false
                    } else {
                        let position = |count| {
                            if backwards {
                                start - count
                            } else {
                                start + count
                            }
                        };
                        state.position = position(if *greedy { matched } else { *min });
                        if *min < matched {
                            if outer_choices + alternatives.len() >= MAX_STACK {
                                return Err(Error::Limit(
                                    "Regular-expression backtracking limit reached (1024).",
                                ));
                            }
                            spend(budget, state.captures.len() + state.marks.len() + 1)?;
                            let mut alternative = state.clone();
                            alternative.pc += 1;
                            alternatives.push(Choice {
                                state: alternative,
                                positions: Some((
                                    position(if *greedy { matched - 1 } else { min + 1 }),
                                    position(if *greedy { *min } else { matched }),
                                    *greedy != backwards,
                                )),
                            });
                        }
                        true
                    }
                }
                Instruction::Jump(target) => {
                    state.pc = *target;
                    continue;
                }
                Instruction::Split(first, second) => {
                    if outer_choices + alternatives.len() >= MAX_STACK {
                        return Err(Error::Limit(
                            "Regular-expression backtracking limit reached (1024).",
                        ));
                    }
                    spend(budget, state.captures.len() + state.marks.len() + 1)?;
                    let mut alternative = state.clone();
                    alternative.pc = *second;
                    alternatives.push(Choice {
                        state: alternative,
                        positions: None,
                    });
                    state.pc = *first;
                    continue;
                }
                Instruction::Look {
                    begin,
                    after,
                    positive,
                    backwards: direction,
                } => {
                    spend(budget, state.captures.len() + state.marks.len() + 1)?;
                    let mut assertion = state.clone();
                    assertion.pc = *begin;
                    let result = self.run(
                        input,
                        assertion,
                        *direction,
                        budget,
                        depth + 1,
                        outer_choices + alternatives.len(),
                    )?;
                    let matched = result.is_some();
                    if matched == *positive {
                        if *positive {
                            state.captures = result.unwrap().captures;
                        }
                        state.pc = *after;
                        continue;
                    }
                    false
                }
                Instruction::Accept => return Ok(Some(state)),
            };
            if succeeded {
                if matches!(
                    next,
                    Instruction::Character(_) | Instruction::Class(_) | Instruction::Any
                ) {
                    let width = decoded.expect("Successful consuming instruction has a character").1;
                    spend(budget, width - 1)?;
                    state.position = if backwards {
                        state.position - width
                    } else {
                        state.position + width
                    };
                }
                state.pc += 1;
            } else if let Some(mut previous) = alternatives.pop() {
                if let Some((next, limit, backwards)) = previous.positions {
                    spend(
                        budget,
                        previous.state.captures.len() + previous.state.marks.len() + 1,
                    )?;
                    state = previous.state.clone();
                    state.position = next;
                    if next != limit {
                        previous.positions = Some((
                            if backwards { next - 1 } else { next + 1 },
                            limit,
                            backwards,
                        ));
                        alternatives.push(previous);
                    }
                } else {
                    state = previous.state;
                }
            } else {
                return Ok(None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_capture_alternatives_resolve_to_the_participating_group() {
        let mut budget = 100_000;
        let regex = Regex::compile(r"^(?:(?<x>a)|(?<x>b))\k<x>$", "", &mut budget).unwrap();
        for (text, expected) in [("aa", Some((0, 1))), ("bb", Some((0, 1))), ("ab", None)] {
            let input: Vec<_> = text.encode_utf16().collect();
            let found = regex.find(&input, 0, false, &mut budget).unwrap();
            assert_eq!(found.is_some(), expected.is_some(), "{text}");
            if let Some(found) = found {
                assert_eq!(found.named_captures, vec![("x".into(), expected)]);
                assert!(found.retained_bytes() >= found.captures.len() * 16 + 1);
            }
        }
        for pattern in [
            r"(?<x>a)(?<x>b)",
            r"(?<x>(?<x>a))",
            r"(?<x>a)|(?<x>b)(?<x>c)",
        ] {
            assert!(
                matches!(
                    Regex::compile(pattern, "", &mut budget),
                    Err(Error::Syntax(_))
                ),
                "{pattern}"
            );
        }
    }

    #[test]
    fn named_and_numbered_references_keep_direction_and_unmatched_semantics() {
        for (pattern, text, expected) in [
            (r"^\k<x>(?<x>a)$", "a", true),
            (r"^(?<x>\k<x>a)$", "a", true),
            (r"^(?<x>a)?\k<x>b$", "b", true),
            (r"^(?<x>a)(b)\2\1$", "abba", true),
            (r"^(a|ab)\1$", "abab", true),
            (r"(?<=\k<x>(?<x>ab))c", "ababc", true),
            (r"^(?:(?<x>a)|(?<x>b))+\k<x>$", "abb", true),
            (r"^(?:(?<x>a)|(?<x>b))+\k<x>$", "aba", false),
            (r"^(?<x>猫é)\k<x>$", "猫é猫é", true),
        ] {
            let mut budget = 100_000;
            let regex = Regex::compile(pattern, "", &mut budget).unwrap();
            let input: Vec<_> = text.encode_utf16().collect();
            assert_eq!(
                regex.find(&input, 0, false, &mut budget).unwrap().is_some(),
                expected,
                "{pattern}: {text}"
            );
        }
    }

    #[test]
    fn named_capture_grammar_is_whole_pattern_and_names_are_bounded() {
        let mut budget = 100_000;
        for (pattern, text) in [
            (r"\k<x>", "k<x>"),
            (r"[\k]", "k"),
            (r"(?<\u0061>a)\k<a>", "aa"),
            (r"(?<\u{61}>a)\k<\u0061>", "aa"),
        ] {
            let regex = Regex::compile(pattern, "", &mut budget).unwrap();
            assert!(regex
                .find(
                    &text.encode_utf16().collect::<Vec<_>>(),
                    0,
                    false,
                    &mut budget
                )
                .unwrap()
                .is_some());
        }
        for pattern in [
            r"(?<x>a)[\k]",
            r"(?<x>a)\k",
            r"\k<missing>(?<x>a)",
            r"(?<>a)",
            r"(?<1x>a)",
        ] {
            assert!(
                matches!(
                    Regex::compile(pattern, "", &mut budget),
                    Err(Error::Syntax(_))
                ),
                "{pattern}"
            );
        }
        for pattern in [r"(?<猫>a)", r"(?<__proto__>a)", r"(a)\2"] {
            assert!(
                matches!(
                    Regex::compile(pattern, "", &mut budget),
                    Err(Error::Unsupported(_))
                ),
                "{pattern}"
            );
        }
        assert!(matches!(
            Regex::compile(&format!("(?<{}>a)", "x".repeat(257)), "", &mut budget),
            Err(Error::Limit(_))
        ));
        let too_many = (0..17).map(|i| format!("(?<g{i}>a)")).collect::<String>();
        assert!(matches!(
            Regex::compile(&too_many, "", &mut budget),
            Err(Error::Limit(_))
        ));
    }

    #[test]
    fn backreferences_spend_work_and_preserve_case_folding_boundary() {
        let mut budget = 100_000;
        let regex = Regex::compile(r"(?<x>.+)\k<x>", "i", &mut budget).unwrap();
        assert!(regex
            .find(
                &"aBab".encode_utf16().collect::<Vec<_>>(),
                0,
                false,
                &mut budget
            )
            .unwrap()
            .is_some());
        assert!(matches!(
            regex.find(
                &"éé".encode_utf16().collect::<Vec<_>>(),
                0,
                false,
                &mut budget
            ),
            Err(Error::Unsupported(_))
        ));
        let regex = Regex::compile(r"(?<x>a{64})\k<x>", "", &mut budget).unwrap();
        assert!(matches!(
            regex.find(&vec![97; 128], 0, false, &mut 100),
            Err(Error::Limit(_))
        ));
        let plain = Regex::compile("(a)", "", &mut budget).unwrap();
        let named = Regex::compile("(?<entry>a)", "", &mut budget).unwrap();
        assert!(named.retained_bytes > plain.retained_bytes);
    }

    #[test]
    fn legacy_opening_braces_distinguish_literals_and_complete_quantifiers() {
        for pattern in ["{", "}", "{}", "{1", "{1,", "{,1}", "a{1", "a{1,", "a{1,2", "a{1,x}", "a{1001"] {
            let mut budget = 100_000;
            let regex = Regex::compile(pattern, "", &mut budget).unwrap();
            let input: Vec<_> = pattern.encode_utf16().collect();
            assert!(regex.find(&input, 0, false, &mut budget).unwrap().is_some(), "{pattern}");
        }
        for (pattern, text) in [(r"\{", "{"), ("[{]", "{"), ("^({)(})$", "{}"), ("^{{2}$", "{{")] {
            let mut budget = 100_000;
            let regex = Regex::compile(pattern, "", &mut budget).unwrap();
            assert!(regex.find(&text.encode_utf16().collect::<Vec<_>>(), 0, false, &mut budget).unwrap().is_some(), "{pattern}");
        }
        for pattern in ["{1}", "{1,}", "{1,2}", "a{2,1}", "a{1}*", "a{1}{2}", "*{", "("] {
            assert!(matches!(Regex::compile(pattern, "", &mut 100_000), Err(Error::Syntax(_))), "{pattern}");
        }
        assert!(matches!(Regex::compile("a{1001}", "", &mut 100_000), Err(Error::Limit(_))));
        assert!(matches!(Regex::compile("{", "", &mut 0), Err(Error::Limit(_))));
        assert!(matches!(Regex::compile("{", "u", &mut 100_000), Err(Error::Syntax(_))));
        assert!(matches!(Regex::compile("{", "v", &mut 100_000), Err(Error::Unsupported(_))));
        for (pattern, end) in [("a{2,3}", 3), ("a{2,3}?", 2)] {
            let mut budget = 100_000;
            let regex = Regex::compile(pattern, "", &mut budget).unwrap();
            assert_eq!(regex.find(&[97,97,97,97], 0, false, &mut budget).unwrap().unwrap().end(), end);
        }
    }

    #[test]
    fn wide_flat_alternation_keeps_order_and_bounded_utf16_matching() {
        use std::fmt::Write;

        let mut pattern = String::from("&(?:");
        for index in 0..2_125 {
            write!(pattern, "r{index:04}xy;|").unwrap();
        }
        pattern.push_str(r"#[xX][0-9a-fA-F]+;?|#[0-9]+;?)");
        assert!(pattern.len() > 18_390);
        let mut budget = 2_000_000;
        let regex = Regex::compile(&pattern, "g", &mut budget).unwrap();
        let input: Vec<_> = "x &r2124xy; y &#x4a; and &#12;".encode_utf16().collect();
        let mut start = 0;
        for expected in ["&r2124xy;", "&#x4a;", "&#12;"] {
            let found = regex.find(&input, start, false, &mut budget).unwrap().unwrap();
            assert_eq!(String::from_utf16(&input[found.start()..found.end()]).unwrap(), expected);
            start = found.end();
        }
        assert!(regex.find(&input, start, false, &mut budget).unwrap().is_none());
        assert!(matches!(Regex::compile(&pattern, "g", &mut 256), Err(Error::Limit(_))));
    }

    #[test]
    fn wide_pattern_admission_retains_source_and_bytecode_limits() {
        let over = "a".repeat(MAX_PATTERN + 1);
        assert!(matches!(Regex::compile(&over, "", &mut 2_000_000), Err(Error::Limit(PATTERN_LIMIT))));
        let over_units: Vec<_> = "😺".repeat(MAX_PATTERN / 4 + 1).encode_utf16().collect();
        assert!(matches!(Regex::compile_units(&over_units, "u", &mut 2_000_000), Err(Error::Limit(PATTERN_LIMIT))));
        let expanded = "(?:ab){1000}".repeat(17);
        assert!(matches!(Regex::compile(&expanded, "", &mut 2_000_000),
            Err(Error::Limit("Regular-expression bytecode limit reached (32768)."))));
    }

    #[test]
    fn legacy_closing_delimiters_match_literals_with_captures_and_quantifiers() {
        let mut budget = 100_000;
        let regex = Regex::compile("^(})]{2,3}}$", "", &mut budget).unwrap();
        for (text, expected) in [
            ("}]]}", true),
            ("}]]]}", true),
            ("}]}", false),
            ("}]]]]}", false),
        ] {
            let input: Vec<_> = text.encode_utf16().collect();
            let found = regex.find(&input, 0, false, &mut budget).unwrap();
            assert_eq!(found.is_some(), expected, "{text}");
            if let Some(found) = found {
                assert_eq!((found.start(), found.end()), (0, input.len()));
                assert_eq!(found.captures[1], Some((0, 1)));
            }
        }
        let regex = Regex::compile("}]", "g", &mut budget).unwrap();
        let input: Vec<_> = "a}]b".encode_utf16().collect();
        let found = regex.find(&input, 0, false, &mut budget).unwrap().unwrap();
        assert_eq!((found.start(), found.end()), (1, 3));
    }

    #[test]
    fn legacy_closing_delimiters_preserve_syntax_flags_and_work_limits() {
        for pattern in ["*}", "+]", "?]", "}**", "}a{2,1}", "}("] {
            assert!(
                matches!(
                    Regex::compile(pattern, "", &mut 100_000),
                    Err(Error::Syntax(_))
                ),
                "{pattern}"
            );
        }
        assert!(matches!(Regex::compile("}", "u", &mut 100_000), Err(Error::Syntax(_))));
        for flags in ["v", "d"] {
            assert!(matches!(
                Regex::compile("}", flags, &mut 100_000),
                Err(Error::Unsupported(_))
            ));
        }
        assert!(matches!(
            Regex::compile("}", "", &mut 0),
            Err(Error::Limit(_))
        ));
        let mut budget = 100_000;
        let regex = Regex::compile("}+", "", &mut budget).unwrap();
        assert!(matches!(
            regex.find(&[125], 0, false, &mut 0),
            Err(Error::Limit(_))
        ));
    }
}
