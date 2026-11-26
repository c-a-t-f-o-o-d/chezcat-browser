//! Original Unicode character-set algebra. Set expressions are reduced to
//! normalized code-point intervals before entering the shared bytecode matcher.
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
        ClassPart::Digit(inverted) => (&[(48,57)], *inverted),
        ClassPart::Word(inverted) => (&[(48,57),(65,90),(95,95),(97,122)], *inverted),
        ClassPart::Space(inverted) => (&[(9,13),(32,32),(0xa0,0xa0),(0x1680,0x1680),(0x2000,0x200a),(0x2028,0x2029),(0x202f,0x202f),(0x205f,0x205f),(0x3000,0x3000),(0xfeff,0xfeff)], *inverted),
    };
    let mut out = Vec::new();
    for &(low,high) in parts { push(&mut out, low, high, budget)?; }
    if inverted { complement(&out, budget) } else { Ok(out) }
}

impl Parser<'_> {
    fn set_operator(&self) -> Option<u16> {
        let unit = self.peek()?;
        (matches!(unit, 38 | 45) && self.units.get(self.at + 1) == Some(&unit)).then_some(unit)
    }

    fn set_operand(&mut self) -> Result<(Ranges, bool), Error> {
        let unit = self.peek().ok_or(Error::Syntax("Unterminated Unicode set class."))?;
        self.at += 1;
        if unit == 91 {
            let class = self.unicode_class()?;
            let mut ranges = Vec::new();
            for part in class.parts {
                let ClassPart::Range(low, high) = part else { unreachable!() };
                push(&mut ranges, low, high, self.budget)?;
            }
            if class.inverted { ranges = complement(&ranges, self.budget)?; }
            return Ok((ranges, false));
        }
        let part = if unit == 92 {
            if self.peek() == Some(113) && self.units.get(self.at + 1) == Some(&123) {
                return Err(Error::Unsupported("Unicode regexp string sets are not implemented."));
            }
            self.escape(true)?
        } else {
            if b"(){}|/-]".iter().any(|&syntax| unit == u16::from(syntax))
                || b"&!#$%*+,.:;<=>?@^`~".iter().any(|&punct| unit == u16::from(punct)) && self.peek() == Some(unit)
            {
                return Err(Error::Syntax("Reserved Unicode set class punctuation must be escaped."));
            }
            let character = self.raw_character();
            ClassPart::Range(character, character)
        };
        let character = matches!(part, ClassPart::Range(_, _));
        Ok((part_ranges(part, self.budget)?, character))
    }

    fn set_union_term(&mut self) -> Result<(Ranges, bool), Error> {
        let (mut ranges, character) = self.set_operand()?;
        if self.peek() == Some(45) && self.set_operator().is_none() {
            self.at += 1;
            let (end, end_character) = self.set_operand()?;
            if !character || !end_character || ranges[0].0 > end[0].0 {
                return Err(Error::Syntax("Unicode set ranges require ordered character endpoints."));
            }
            ranges[0].1 = end[0].0;
            return Ok((ranges, true));
        }
        Ok((ranges, false))
    }

    pub(super) fn unicode_class(&mut self) -> Result<Class, Error> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(Error::Limit("Regular-expression nesting limit reached (32)."));
        }
        let inverted = self.eat(94);
        let mut ranges = Vec::new();
        if self.peek() != Some(93) {
            let (first, range) = self.set_union_term()?;
            ranges = first;
            if let Some(operator) = self.set_operator() {
                if range { return Err(Error::Syntax("A Unicode set operation requires class operands.")); }
                while self.set_operator() == Some(operator) {
                    self.at += 2;
                    let (right, _) = self.set_operand()?;
                    ranges = if operator == 38 { intersection(&ranges, &right, self.budget)? }
                        else { subtraction(&ranges, &right, self.budget)? };
                }
            } else {
                while self.peek() != Some(93) {
                    if self.set_operator().is_some() {
                        return Err(Error::Syntax("Unicode set union cannot mix with intersection or subtraction."));
                    }
                    let (mut other, _) = self.set_union_term()?;
                    reserve(&mut ranges, other.len(), self.budget)?;
                    spend(self.budget, other.len())?;
                    ranges.append(&mut other);
                }
                normalize(&mut ranges, self.budget)?;
            }
        }
        if !self.eat(93) { return Err(Error::Syntax("Invalid Unicode set class operation.")); }
        let mut parts = Vec::new();
        reserve(&mut parts, ranges.len(), self.budget)?;
        for (low, high) in ranges { parts.push(ClassPart::Range(low, high)); }
        self.depth -= 1;
        Ok(Class { inverted, parts })
    }
}
