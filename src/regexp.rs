//! Original bounded regular-expression bytecode for the JavaScript subset.
//! No external engine. Matching uses UTF-16 positions and an explicit, bounded
//! backtracking stack. Compiler visits, instructions and copied checkpoints
//! spend one shared document-job work budget. Unsupported syntax never becomes a broader match.

const MAX_PATTERN: usize = 4096;
const MAX_CODE: usize = 4096;
const MAX_DEPTH: usize = 32;
const MAX_GROUPS: usize = 16;
const MAX_MARKS: usize = 64;
const MAX_STACK: usize = 1024;

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
                'y' => &mut flags.sticky,
                'u' | 'v' | 'd' => {
                    return Err(Error::Unsupported(
                        "Unicode and indices regexp flags are not implemented.",
                    ))
                }
                _ => return Err(Error::Syntax("Invalid regular-expression flag.")),
            };
            if *entry {
                return Err(Error::Syntax("Duplicate regular-expression flag."));
            }
            *entry = true;
        }
        Ok(flags)
    }

    pub fn text(self) -> String {
        [
            (self.global, 'g'),
            (self.insensitive, 'i'),
            (self.multiline, 'm'),
            (self.dot_all, 's'),
            (self.sticky, 'y'),
        ]
        .into_iter()
        .filter_map(|(set, ch)| set.then_some(ch))
        .collect()
    }
}

#[derive(Clone, Debug)]
enum ClassPart {
    Range(u16, u16),
    Digit(bool),
    Word(bool),
    Space(bool),
}

#[derive(Clone, Debug)]
struct Class {
    inverted: bool,
    parts: Vec<ClassPart>,
}

fn line(unit: u16) -> bool {
    matches!(unit, 10 | 13 | 0x2028 | 0x2029)
}
fn word(unit: u16) -> bool {
    matches!(unit, 48..=57 | 65..=90 | 95 | 97..=122)
}
fn space(unit: u16) -> bool {
    matches!(unit, 9..=13 | 32 | 0xa0 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000 | 0xfeff)
}
fn ascii_fold(unit: u16) -> u16 {
    if (97..=122).contains(&unit) {
        unit - 32
    } else {
        unit
    }
}

impl Class {
    fn matches(&self, unit: u16, insensitive: bool, budget: &mut usize) -> Result<bool, Error> {
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
    Character(u16),
    Class(Class),
    Any,
    Start,
    End,
    Boundary(bool),
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

struct Parser {
    units: Vec<u16>,
    at: usize,
    nodes: Vec<Node>,
    groups: usize,
    depth: usize,
    insensitive: bool,
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
        let mut branches = vec![self.sequence()?];
        while self.eat(b'|' as u16) {
            branches.push(self.sequence()?);
        }
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
                Some(123) => Some(self.counted()?),
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
                    } else if self.eat(b'=' as u16) {
                        assertion = Some((true, false));
                        None
                    } else if self.eat(b'!' as u16) {
                        assertion = Some((false, false));
                        None
                    } else if self.eat(b'<' as u16) {
                        let positive = if self.eat(b'=' as u16) {
                            true
                        } else if self.eat(b'!' as u16) {
                            false
                        } else {
                            return Err(Error::Unsupported(
                                "Named regexp capture groups are not implemented.",
                            ));
                        };
                        assertion = Some((positive, true));
                        None
                    } else {
                        return Err(Error::Unsupported(
                            "Regexp inline modifiers or this group extension are not implemented.",
                        ));
                    }
                } else {
                    self.groups += 1;
                    if self.groups > MAX_GROUPS {
                        return Err(Error::Limit(
                            "Regular-expression capture limit reached (16).",
                        ));
                    }
                    Some(self.groups)
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
                let part = self.escape(false)?;
                match part {
                    ClassPart::Range(low, high) if low == high => self.character(low),
                    other => self.push(Node::Class(Class {
                        inverted: false,
                        parts: vec![other],
                    })),
                }
            }
            42 | 43 | 63 | 123 | 125 | 93 => Err(Error::Syntax(
                "Unexpected regular-expression quantifier or delimiter.",
            )),
            _ => self.character(unit),
        }
    }
    fn character(&mut self, unit: u16) -> Result<usize, Error> {
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
    fn escape(&mut self, in_class: bool) -> Result<ClassPart, Error> {
        let unit = self
            .peek()
            .ok_or(Error::Syntax("Trailing regular-expression escape."))?;
        self.at += 1;
        let literal = match unit {
            100 | 68 => return Ok(ClassPart::Digit(unit == 68)),
            119 | 87 => return Ok(ClassPart::Word(unit == 87)),
            115 | 83 => return Ok(ClassPart::Space(unit == 83)),
            110 => 10,
            114 => 13,
            116 => 9,
            118 => 11,
            102 => 12,
            98 if in_class => 8,
            120 => self.hex(2)?,
            117 => self.hex(4)?,
            99 => {
                let next = self
                    .peek()
                    .ok_or(Error::Syntax("Incomplete regexp control escape."))?;
                if !matches!(next, 65..=90 | 97..=122) {
                    return Err(Error::Syntax("Invalid regexp control escape."));
                }
                self.at += 1;
                next % 32
            }
            48 => {
                if self.peek().is_some_and(|u| (48..=57).contains(&u)) {
                    return Err(Error::Unsupported(
                        "Legacy regexp octal escapes are not implemented.",
                    ));
                }
                0
            }
            49..=57 | 107 => {
                return Err(Error::Unsupported(
                    "Regular-expression backreferences are not implemented.",
                ))
            }
            112 | 80 => {
                return Err(Error::Unsupported(
                    "Unicode regexp properties are not implemented.",
                ))
            }
            _ => unit,
        };
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
        let mut parts = Vec::new();
        while self.peek() != Some(b']' as u16) {
            let start = self.class_atom()?;
            if self.peek() == Some(b'-' as u16)
                && self.units.get(self.at + 1) != Some(&(b']' as u16))
            {
                self.at += 1;
                let end = self.class_atom()?;
                let (ClassPart::Range(low, _), ClassPart::Range(_, high)) = (start, end) else {
                    return Err(Error::Unsupported(
                        "Character-class escapes cannot form ranges in this subset.",
                    ));
                };
                if low > high {
                    return Err(Error::Syntax("Reversed regular-expression range."));
                }
                parts.push(ClassPart::Range(low, high));
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
    Character(u16),
    Class(Class),
    Any,
}
impl Atom {
    fn matches(&self, unit: u16, flags: Flags, budget: &mut usize) -> Result<bool, Error> {
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
    Character(u16),
    Class(Class),
    Any,
    Start,
    End,
    Boundary(bool),
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
}
impl Compiler<'_> {
    fn emit(&mut self, instruction: Instruction) -> Result<usize, Error> {
        spend(self.budget, 1)?;
        if self.code.len() == MAX_CODE {
            return Err(Error::Limit(
                "Regular-expression bytecode limit reached (4096).",
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
                    Node::Character(unit) => Some(Atom::Character(*unit)),
                    Node::Class(class) => {
                        spend(
                            self.budget,
                            class.parts.len() * std::mem::size_of::<ClassPart>(),
                        )?;
                        Some(Atom::Class(class.clone()))
                    }
                    Node::Any => Some(Atom::Any),
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
    groups: usize,
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
}
impl Match {
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

impl Regex {
    pub fn compile(pattern: &str, flags: &str, budget: &mut usize) -> Result<Self, Error> {
        if pattern.len() > MAX_PATTERN {
            return Err(Error::Limit(
                "Regular-expression pattern limit reached (4 KiB).",
            ));
        }
        spend(budget, pattern.len().saturating_add(flags.len()))?;
        let flags = Flags::parse(flags)?;
        let mut parser = Parser {
            units: pattern.encode_utf16().collect(),
            at: 0,
            nodes: Vec::new(),
            groups: 0,
            depth: 0,
            insensitive: flags.insensitive,
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
        };
        compiler.emit(Instruction::Save(0))?;
        compiler.node(&parser.nodes, root, false)?;
        compiler.emit(Instruction::Save(1))?;
        compiler.emit(Instruction::Accept)?;
        let mut source = String::new();
        let mut escaped = false;
        for ch in pattern.chars() {
            if escaped {
                source.push(ch);
                escaped = false;
                continue;
            }
            match ch {
                '\\' => {
                    source.push(ch);
                    escaped = true;
                }
                '/' => source.push_str("\\/"),
                '\n' => source.push_str("\\n"),
                '\r' => source.push_str("\\r"),
                '\u{2028}' => source.push_str("\\u2028"),
                '\u{2029}' => source.push_str("\\u2029"),
                _ => source.push(ch),
            }
        }
        if source.is_empty() {
            source.push_str("(?:)");
        }
        let retained_bytes = source.len()
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
                    _ => 0,
                })
                .sum::<usize>();
        Ok(Self {
            code: compiler.code,
            flags,
            source,
            groups: parser.groups,
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
        for beginning in start..=if sticky { start } else { input.len() } {
            spend(budget, (self.groups + 1) * 2 + self.marks + 1)?;
            let state = State {
                pc: 0,
                position: beginning,
                captures: vec![None; (self.groups + 1) * 2],
                marks: vec![None; self.marks],
            };
            if let Some(state) = self.run(input, state, false, budget, 0, 0)? {
                let captures = state
                    .captures
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| match (pair[0], pair[1]) {
                        (Some(a), Some(b)) => Some((a, b)),
                        _ => None,
                    })
                    .collect();
                return Ok(Some(Match { captures }));
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
            let following = input.get(state.position).copied();
            let current = if backwards {
                state
                    .position
                    .checked_sub(1)
                    .and_then(|index| input.get(index))
                    .copied()
            } else {
                following
            };
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
                            && line(input[state.position - 1])
                }
                Instruction::End => {
                    state.position == input.len()
                        || self.flags.multiline && following.is_some_and(line)
                }
                Instruction::Boundary(inverted) => {
                    let before = state.position > 0 && word(input[state.position - 1]);
                    let after = following.is_some_and(word);
                    (before != after) != *inverted
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
                        if !atom.matches(input[index], self.flags, budget)? {
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
                    state.position = if backwards {
                        state.position - 1
                    } else {
                        state.position + 1
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
