//! Finite typed length-percentage arithmetic, evaluated without recursive parsing.
//! https://drafts.csswg.org/css-values-4/#calc-syntax
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CalculatedLength {
    pub(crate) coefficients: [f32; 6], // px, percentage, em, rem, vw, vh
    pub(crate) nonnegative: bool,
    pub(crate) percentage: bool,
}
impl CalculatedLength {
    pub(crate) fn resolve(self, reference: f32, font: f32, root: f32) -> Option<f64> {
        let c = self.coefficients;
        if c[4] != 0.0 || c[5] != 0.0 {
            return None;
        }
        let value = c[0] as f64
            + c[1] as f64 * reference as f64 / 100.0
            + c[2] as f64 * font as f64
            + c[3] as f64 * root as f64;
        Some(if self.nonnegative {
            value.max(0.0)
        } else {
            value
        })
    }
    pub(crate) fn text(self) -> String {
        let c = self.coefficients;
        format!(
            "calc({}px + {}% + {}em + {}rem + {}vw + {}vh)",
            c[0], c[1], c[2], c[3], c[4], c[5]
        )
    }
    pub(super) fn viewport(mut self, environment: Option<MediaEnvironment>) -> Self {
        if self.coefficients[4] == 0.0 && self.coefficients[5] == 0.0 {
            return self;
        }
        if let Some(env) = environment.filter(|env| {
            env.width.is_finite() && env.height.is_finite() && env.width >= 0.0 && env.height >= 0.0
        }) {
            let c = &mut self.coefficients;
            let pixels =
                c[0] as f64 + c[4] as f64 * env.width / 100.0 + c[5] as f64 * env.height / 100.0;
            if pixels.is_finite() && pixels.abs() <= f32::MAX as f64 {
                c[0] = pixels as f32;
                c[4] = 0.0;
                c[5] = 0.0;
            }
        }
        self
    }
    pub(super) fn inherited(mut self, parent: &Style) -> Self {
        let c = &mut self.coefficients;
        c[0] = (c[0] as f64
            + c[2] as f64 * parent.font_size as f64
            + c[3] as f64 * parent.root_font_size as f64)
            .clamp(-(f32::MAX as f64), f32::MAX as f64) as f32;
        c[2] = 0.0;
        c[3] = 0.0;
        self
    }
}
#[derive(Clone, Copy)]
enum Value {
    Number(f64),
    Length([f64; 6], bool),
}
impl Value {
    fn scale(self, factor: f64) -> Option<Self> {
        let value = match self {
            Self::Number(value) => Self::Number(value * factor),
            Self::Length(c, p) => Self::Length(c.map(|v| v * factor), p),
        };
        match value {
            Self::Number(v) if v.is_finite() => Some(value),
            Self::Length(c, _) if c.iter().all(|v| v.is_finite()) => Some(value),
            _ => None,
        }
    }
    fn combine(self, other: Self, op: u8) -> Option<Self> {
        match (self, other, op) {
            (Self::Number(a), Self::Number(b), b'+') => Self::Number(a + b).scale(1.0),
            (Self::Number(a), Self::Number(b), b'-') => Self::Number(a - b).scale(1.0),
            (Self::Length(a, pa), Self::Length(b, pb), b'+' | b'-') => {
                let sign = if op == b'+' { 1.0 } else { -1.0 };
                Self::Length(std::array::from_fn(|i| a[i] + sign * b[i]), pa || pb).scale(1.0)
            }
            (value, Self::Number(factor), b'*') | (Self::Number(factor), value, b'*') => {
                value.scale(factor)
            }
            (value, Self::Number(divisor), b'/') if divisor != 0.0 => value.scale(1.0 / divisor),
            _ => None,
        }
    }
}
fn reduce(values: &mut Vec<Value>, op: u8) -> Option<()> {
    let right = values.pop()?;
    let left = values.pop()?;
    values.push(left.combine(right, op)?);
    Some(())
}
fn precedence(op: u8) -> u8 {
    if matches!(op, b'*' | b'/') {
        2
    } else {
        1
    }
}
fn expression(source: &str) -> Option<Value> {
    let bytes = source.as_bytes();
    let mut index = 0;
    let mut operand = true;
    let mut values = Vec::new();
    let mut operators = Vec::new();
    // Existing declaration bytes bound the maximum token/stack storage.
    values.try_reserve_exact(source.len() / 3 + 2).ok()?;
    operators.try_reserve_exact(source.len()).ok()?;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if operand {
            if bytes[index] == b'(' {
                operators.push(b'(');
                index += 1;
                continue;
            }
            if source[index..].starts_with("calc(") {
                operators.push(b'(');
                index += 5;
                continue;
            }
            let end = index + translate::number_end(&source[index..])?;
            let number = source[index..end]
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())?;
            index = end;
            let start = index;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphabetic() || bytes[index] == b'%')
            {
                index += 1;
            }
            let unit = &source[start..index];
            let value = if unit.is_empty() {
                Value::Number(number)
            } else {
                let (slot, factor) = match unit {
                    "px" => (0, 1.0),
                    "pt" => (0, 4.0 / 3.0),
                    "pc" => (0, 16.0),
                    "%" => (1, 1.0),
                    "em" => (2, 1.0),
                    "rem" => (3, 1.0),
                    "vw" => (4, 1.0),
                    "vh" => (5, 1.0),
                    _ => return None,
                };
                let mut c = [0.0; 6];
                c[slot] = number * factor;
                Value::Length(c, slot == 1)
            };
            values.push(value);
            operand = false;
        } else if bytes[index] == b')' {
            while operators.last().is_some_and(|&op| op != b'(') {
                reduce(&mut values, operators.pop()?)?;
            }
            if operators.pop() != Some(b'(') {
                return None;
            }
            index += 1;
        } else {
            let op = bytes[index];
            if !matches!(op, b'+' | b'-' | b'*' | b'/') {
                return None;
            }
            if matches!(op, b'+' | b'-')
                && (!index
                    .checked_sub(1)
                    .and_then(|i| bytes.get(i))
                    .is_some_and(u8::is_ascii_whitespace)
                    || !bytes.get(index + 1).is_some_and(u8::is_ascii_whitespace))
            {
                return None;
            }
            while operators
                .last()
                .is_some_and(|&prior| prior != b'(' && precedence(prior) >= precedence(op))
            {
                reduce(&mut values, operators.pop()?)?;
            }
            operators.push(op);
            index += 1;
            operand = true;
        }
    }
    if operand {
        return None;
    }
    while let Some(op) = operators.pop() {
        if op == b'(' {
            return None;
        }
        reduce(&mut values, op)?;
    }
    (values.len() == 1).then(|| values[0])
}
pub(super) fn cost(source: &str) -> (usize, usize) {
    let values = (source.len() / 3 + 2).saturating_mul(std::mem::size_of::<Value>());
    (
        source.len().saturating_mul(16),
        values.saturating_add(source.len()).saturating_mul(2),
    )
}
pub(super) fn viewport_literal(source: &str, negative: bool) -> Option<Length> {
    let source = source.trim_ascii();
    if !source.is_ascii() || source.bytes().any(|b| b.is_ascii_whitespace()) {
        return None;
    }
    let source = source.to_ascii_lowercase();
    let (number, slot) = if let Some(number) = source.strip_suffix("vw") {
        (number, 4)
    } else if let Some(number) = source.strip_suffix("vh") {
        (number, 5)
    } else {
        return None;
    };
    if translate::number_end(number)? != number.len() {
        return None;
    }
    let number = number
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite() && (negative || *value >= 0.0))?;
    let mut c = [0.0; 6];
    c[slot] = number.clamp(-10_000.0, 10_000.0);
    Some(Length::Calc(CalculatedLength {
        coefficients: c,
        nonnegative: !negative,
        percentage: false,
    }))
}
pub(super) fn resolve_viewport(value: Length, environment: Option<MediaEnvironment>) -> Length {
    if let Length::Calc(value) = value {
        Length::Calc(value.viewport(environment))
    } else {
        value
    }
}

pub(super) fn length(source: &str, negative: bool) -> Option<Length> {
    let source = source.trim_ascii();
    if source.len() > MAX_GROUPED_BOX_BYTES || !source.is_ascii() {
        return None;
    }
    let source = source.to_ascii_lowercase();
    let body = source.strip_prefix("calc(")?.strip_suffix(')')?;
    let Value::Length(c, percentage) = expression(body)? else {
        return None;
    };
    if !c
        .iter()
        .all(|v| v.is_finite() && v.abs() <= f32::MAX as f64)
    {
        return None;
    }
    let nonzero = c.iter().filter(|&&v| v != 0.0).count();
    if nonzero <= 1 {
        let slot = c
            .iter()
            .position(|&v| v != 0.0)
            .unwrap_or(if percentage { 1 } else { 0 });
        if slot < 4 && (!percentage || slot == 1) {
            let value = if negative { c[slot] } else { c[slot].max(0.0) };
            return length_with_unit(
                value.clamp(-10_000.0, 10_000.0) as f32,
                ["px", "%", "em", "rem"][slot],
            );
        }
    }
    Some(Length::Calc(CalculatedLength {
        coefficients: c.map(|v| v as f32),
        nonnegative: !negative,
        percentage,
    }))
}
