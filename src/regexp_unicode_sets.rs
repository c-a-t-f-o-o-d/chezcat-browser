//! Original Unicode character and string-set algebra. Characters reduce to
//! normalized intervals; strings lower to the shared sequence/alternation VM.
use super::*;

type Ranges = Vec<(u32, u32)>;
const LAST: u32 = 0x10ffff;
const STORAGE: &str = "Unicode regexp set storage is unavailable.";

fn reserve<T>(values: &mut Vec<T>, additional: usize, budget: &mut usize) -> Result<(), Error> {
    let needed = values.len().checked_add(additional).ok_or(Error::Limit(STORAGE))?;
    if needed > values.capacity() {
        let target = needed.max(values.capacity().checked_mul(2).ok_or(Error::Limit(STORAGE))?);
        spend(budget, target.checked_mul(std::mem::size_of::<T>()).ok_or(Error::Limit(STORAGE))?)?;
        values.try_reserve_exact(target - values.len()).map_err(|_| Error::Limit(STORAGE))?;
        // Account for any allocator-provided capacity beyond the request too.
        spend(budget, (values.capacity() - target).checked_mul(std::mem::size_of::<T>()).ok_or(Error::Limit(STORAGE))?)?;
    }
    Ok(())
}

fn push(out: &mut Ranges, low: u32, high: u32, budget: &mut usize) -> Result<(), Error> {
    spend(budget, 1)?;
    reserve(out, 1, budget)?;
    out.push((low, high));
    Ok(())
}

fn normalize(values: &mut Ranges, budget: &mut usize) -> Result<(), Error> {
    let factor = (usize::BITS - values.len().leading_zeros()).max(1) as usize;
    spend(budget, values.len().checked_mul(factor).ok_or(Error::Limit(STORAGE))?)?;
    values.sort_unstable();
    let mut count = 0;
    for at in 0..values.len() {
        let (low, high) = values[at];
        if count > 0 && low <= values[count - 1].1 + 1 {
            values[count - 1].1 = values[count - 1].1.max(high);
        } else {
            values[count] = (low, high);
            count += 1;
        }
    }
    values.truncate(count);
    Ok(())
}

fn complement(values: &Ranges, budget: &mut usize) -> Result<Ranges, Error> {
    spend(budget, values.len())?;
    let mut out = Vec::new();
    let mut low = 0;
    for &(start, end) in values {
        if low < start { push(&mut out, low, start - 1, budget)?; }
        low = end + 1;
    }
    if low <= LAST { push(&mut out, low, LAST, budget)?; }
    Ok(out)
}

fn intersection(left: &Ranges, right: &Ranges, budget: &mut usize) -> Result<Ranges, Error> {
    spend(budget, left.len().checked_add(right.len()).ok_or(Error::Limit(STORAGE))?)?;
    let mut out = Vec::new();
    let (mut a, mut b) = (0, 0);
    while a < left.len() && b < right.len() {
        let low = left[a].0.max(right[b].0);
        let high = left[a].1.min(right[b].1);
        if low <= high { push(&mut out, low, high, budget)?; }
        if left[a].1 < right[b].1 { a += 1; } else { b += 1; }
    }
    Ok(out)
}

fn subtraction(left: &Ranges, right: &Ranges, budget: &mut usize) -> Result<Ranges, Error> {
    spend(budget, left.len().checked_add(right.len()).ok_or(Error::Limit(STORAGE))?)?;
    let mut out = Vec::new();
    let mut b = 0;
    for &(start, end) in left {
        let mut low = start;
        while b < right.len() && right[b].1 < low { b += 1; }
        let mut at = b;
        while at < right.len() && right[at].0 <= end {
            spend(budget, 1)?;
            if low < right[at].0 { push(&mut out, low, right[at].0 - 1, budget)?; }
            low = low.max(right[at].1 + 1);
            if low > end { break; }
            at += 1;
        }
        if low <= end { push(&mut out, low, end, budget)?; }
        b = at;
    }
    Ok(out)
}

fn part_ranges(part: ClassPart, budget: &mut usize) -> Result<Ranges, Error> {
    let (parts, inverted): (&[(u32,u32)], bool) = match &part {
        ClassPart::Range(low, high) => {
            let mut out = Vec::new();push(&mut out, *low, *high, budget)?;return Ok(out);
        }
        ClassPart::Property(ranges, inverted) => (*ranges, *inverted),
        ClassPart::Digit(inverted) => (&[(48,57)], *inverted),
        ClassPart::Word(inverted) => (&[(48,57),(65,90),(95,95),(97,122)], *inverted),
        ClassPart::Space(inverted) => (&[(9,13),(32,32),(0xa0,0xa0),(0x1680,0x1680),(0x2000,0x200a),(0x2028,0x2029),(0x202f,0x202f),(0x205f,0x205f),(0x3000,0x3000),(0xfeff,0xfeff)], *inverted),
    };
    let mut out = Vec::new();
    for &(low,high) in parts { push(&mut out, low, high, budget)?; }
    if inverted { complement(&out, budget) } else { Ok(out) }
}

#[derive(Default)]
struct Set {
    ranges: Ranges,
    // Only strings of zero or at least two code points; single characters
    // share the interval representation, including isolated surrogates.
    strings: Vec<Vec<u32>>,
    may_strings: bool,
}

fn normalize_strings(strings: &mut [Vec<u32>], budget: &mut usize) -> Result<(), Error> {
    let factor = (usize::BITS - strings.len().leading_zeros()).max(1) as usize;
    let width = strings.iter().map(Vec::len).max().unwrap_or(0).max(1);
    spend(budget, strings.len().checked_mul(factor).and_then(|n| n.checked_mul(width)).and_then(|n| n.checked_mul(2)).ok_or(Error::Limit(STORAGE))?)?;
    strings.sort_unstable();
    Ok(())
}

fn filter_strings(left: Vec<Vec<u32>>, right: &[Vec<u32>], common: bool, budget: &mut usize) -> Result<Vec<Vec<u32>>, Error> {
    let mut out = Vec::new();
    let mut b = 0;
    for value in left {
        let mut equal = false;
        while b < right.len() {
            spend(budget, value.len().min(right[b].len()).checked_add(1).ok_or(Error::Limit(STORAGE))?)?;
            match right[b].cmp(&value) {
                std::cmp::Ordering::Less => b += 1,
                std::cmp::Ordering::Equal => { equal = true; b += 1; break; }
                std::cmp::Ordering::Greater => break,
            }
        }
        if equal == common {
            reserve(&mut out, 1, budget)?;
            out.push(value);
        }
    }
    Ok(out)
}

fn operate(left: Set, right: Set, operator: u16, budget: &mut usize) -> Result<Set, Error> {
    let intersect = operator == 38;
    let ranges = if intersect { intersection(&left.ranges, &right.ranges, budget)? }
        else { subtraction(&left.ranges, &right.ranges, budget)? };
    let strings = filter_strings(left.strings, &right.strings, intersect, budget)?;
    Ok(Set { ranges, strings, may_strings: left.may_strings && (!intersect || right.may_strings) })
}

fn append(left: &mut Set, mut right: Set, budget: &mut usize) -> Result<(), Error> {
    reserve(&mut left.ranges, right.ranges.len(), budget)?;
    reserve(&mut left.strings, right.strings.len(), budget)?;
    spend(budget, right.ranges.len().checked_add(right.strings.len()).ok_or(Error::Limit(STORAGE))?)?;
    left.ranges.append(&mut right.ranges);
    left.strings.append(&mut right.strings);
    left.may_strings |= right.may_strings;
    Ok(())
}

impl Parser<'_> {
    fn set_operator(&self) -> Option<u16> {
        let unit = self.peek()?;
        (matches!(unit, 38 | 45) && self.units.get(self.at + 1) == Some(&unit)).then_some(unit)
    }

    fn set_character(&mut self) -> Result<ClassPart, Error> {
        let unit = self.peek().ok_or(Error::Syntax("Unterminated Unicode set character."))?;
        self.at += 1;
        if unit == 92 { return self.escape(true); }
        if b"()[]{}|/-".iter().any(|&syntax| unit == u16::from(syntax))
            || b"&!#$%*+,.:;<=>?@^`~".iter().any(|&punct| unit == u16::from(punct)) && self.peek() == Some(unit)
        {
            return Err(Error::Syntax("Reserved Unicode set class punctuation must be escaped."));
        }
        let character = self.raw_character();
        Ok(ClassPart::Range(character, character))
    }

    fn string_disjunction(&mut self) -> Result<Set, Error> {
        let mut set = Set::default();
        let mut value = Vec::new();
        loop {
            match self.peek() {
                Some(124 | 125) => {
                    let end = self.peek() == Some(125);
                    self.at += 1;
                    if value.len() == 1 {
                        push(&mut set.ranges, value[0], value[0], self.budget)?;
                        value.clear();
                    } else {
                        set.may_strings = true;
                        reserve(&mut set.strings, 1, self.budget)?;
                        set.strings.push(std::mem::take(&mut value));
                    }
                    if end { break; }
                }
                None => return Err(Error::Syntax("Unterminated Unicode regexp string disjunction.")),
                _ => {
                    if self.peek() == Some(92) && self.units.get(self.at + 1).is_some_and(|unit| b"dDsSwWpP".iter().any(|&escape| *unit == u16::from(escape))) {
                        return Err(Error::Syntax("A Unicode regexp string disjunction requires character escapes."));
                    }
                    let ClassPart::Range(character, _) = self.set_character()? else {
                        return Err(Error::Syntax("A Unicode regexp string disjunction requires character escapes."));
                    };
                    reserve(&mut value, 1, self.budget)?;
                    spend(self.budget, 1)?;
                    value.push(character);
                }
            }
        }
        normalize(&mut set.ranges, self.budget)?;
        normalize_strings(&mut set.strings, self.budget)?;
        set.strings.dedup();
        Ok(set)
    }

    fn set_operand(&mut self) -> Result<(Set, bool), Error> {
        if self.peek() == Some(91) {
            self.at += 1;
            return Ok((self.unicode_class()?, false));
        }
        if self.peek() == Some(92) && self.units.get(self.at + 1) == Some(&113)
            && self.units.get(self.at + 2) == Some(&123)
        {
            self.at += 3;
            return Ok((self.string_disjunction()?, false));
        }
        let part = self.set_character()?;
        let character = matches!(part, ClassPart::Range(_, _));
        Ok((Set { ranges: part_ranges(part, self.budget)?, ..Set::default() }, character))
    }

    fn set_union_term(&mut self) -> Result<(Set, bool), Error> {
        let (mut set, character) = self.set_operand()?;
        if self.peek() == Some(45) && self.set_operator().is_none() {
            self.at += 1;
            let (end, end_character) = self.set_operand()?;
            if !character || !end_character || set.ranges[0].0 > end.ranges[0].0 {
                return Err(Error::Syntax("Unicode set ranges require ordered character endpoints."));
            }
            set.ranges[0].1 = end.ranges[0].0;
            return Ok((set, true));
        }
        Ok((set, false))
    }

    fn unicode_class(&mut self) -> Result<Set, Error> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(Error::Limit("Regular-expression nesting limit reached (32)."));
        }
        let inverted = self.eat(94);
        let mut set = Set::default();
        if self.peek() != Some(93) {
            let (first, range) = self.set_union_term()?;
            set = first;
            if let Some(operator) = self.set_operator() {
                if range { return Err(Error::Syntax("A Unicode set operation requires class operands.")); }
                while self.set_operator() == Some(operator) {
                    self.at += 2;
                    let (right, _) = self.set_operand()?;
                    set = operate(set, right, operator, self.budget)?;
                }
            } else {
                while self.peek() != Some(93) {
                    if self.set_operator().is_some() {
                        return Err(Error::Syntax("Unicode set union cannot mix with intersection or subtraction."));
                    }
                    let (right, _) = self.set_union_term()?;
                    append(&mut set, right, self.budget)?;
                }
                normalize(&mut set.ranges, self.budget)?;
                normalize_strings(&mut set.strings, self.budget)?;
                set.strings.dedup();
            }
        }
        if !self.eat(93) { return Err(Error::Syntax("Invalid Unicode set class operation.")); }
        if inverted {
            if set.may_strings {
                return Err(Error::Syntax("A negated Unicode regexp class may not contain strings."));
            }
            set.ranges = complement(&set.ranges, self.budget)?;
        }
        self.depth -= 1;
        Ok(set)
    }

    pub(super) fn unicode_set_node(&mut self) -> Result<usize, Error> {
        let mut set = self.unicode_class()?;
        let mut parts = Vec::new();
        reserve(&mut parts, set.ranges.len(), self.budget)?;
        for (low, high) in set.ranges { parts.push(ClassPart::Range(low, high)); }
        let class = Class { inverted: false, parts };
        if set.strings.is_empty() { return self.push(Node::Class(class)); }
        // ECMAScript string sets try longer code-point sequences first, then
        // the single-character matcher, then the empty sequence. The existing
        // alternation continuations permit ordinary backtracking in either
        // direction; a class string does not create a capture or assertion.
        let factor = (usize::BITS - set.strings.len().leading_zeros()).max(1) as usize;
        spend(self.budget, set.strings.len().checked_mul(factor).ok_or(Error::Limit(STORAGE))?)?;
        set.strings.sort_unstable_by_key(|value| std::cmp::Reverse(value.len()));
        let mut branches = Vec::new();
        reserve(&mut branches, set.strings.len().checked_add(1).ok_or(Error::Limit(STORAGE))?, self.budget)?;
        let mut empty = false;
        for value in set.strings {
            if value.is_empty() { empty = true; continue; }
            let mut sequence = Vec::new();
            reserve(&mut sequence, value.len(), self.budget)?;
            for character in value {
                spend(self.budget, 1)?;
                sequence.push(self.character(character)?);
            }
            branches.push(self.push(Node::Sequence(sequence))?);
        }
        branches.push(self.push(Node::Class(class))?);
        if empty { branches.push(self.push(Node::Empty)?); }
        self.push(Node::Alternate(branches))
    }
}
