//! Bounded translations from the real author cascade, without matrix shims.
//! https://www.w3.org/TR/css-transforms-1/#transform-property
use super::*;

pub(crate) const MAX_TRANSFORM_BYTES: usize = 2048;
const MAX_FUNCTIONS: usize = 32;

fn trim(source: &str) -> &str {
    source.trim_matches(|ch| matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{000c}'))
}

pub(crate) fn source_cost(bytes: usize) -> (usize, usize) {
    (
        bytes.max(1).div_ceil(64).saturating_mul(512),
        bytes.saturating_mul(64),
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transform {
    None,
    Translate { x: Length, y: Length },
}

impl Transform {
    pub fn is_none(self) -> bool {
        matches!(self, Self::None)
    }

    pub fn establishes_containing_block(self) -> bool {
        !self.is_none()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransformSyntax {
    Invalid,
    Unsupported,
}

#[derive(Clone, Copy)]
enum Function {
    Translate(bool),
    X,
    Y,
    Three,
}

#[derive(Clone, Copy)]
struct Translation {
    function: Function,
    x: Length,
    y: Length,
}

#[derive(Clone, Copy)]
enum Value {
    Translation(Translation),
    None,
    Initial,
    Inherit,
    Revert,
    RevertLayer,
    Unsupported,
}

// A CSS number, not Rust's wider floating-point spelling grammar. The suffix
// remains separate, so an exponent cannot consume the 'e' in an em unit.
pub(super) fn number_end(source: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut end = usize::from(matches!(bytes.first(), Some(b'+') | Some(b'-')));
    let start = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    let integral = end != start;
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        let start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if end == start {
            return None;
        }
    } else if !integral {
        return None;
    }
    if matches!(bytes.get(end), Some(b'e') | Some(b'E')) {
        let mut exponent = end + 1;
        if matches!(bytes.get(exponent), Some(b'+') | Some(b'-')) {
            exponent += 1;
        }
        let start = exponent;
        while bytes.get(exponent).is_some_and(u8::is_ascii_digit) {
            exponent += 1;
        }
        if exponent != start {
            end = exponent;
        }
    }
    Some(end)
}

pub(super) fn length(source: &str, percentage: bool) -> Result<Length, TransformSyntax> {
    let source = trim(source);
    if source.contains('(') {
        math_value(source)?;
        return Err(TransformSyntax::Unsupported);
    }
    if !source.is_ascii() || source.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(TransformSyntax::Invalid);
    }
    let end = number_end(source).ok_or(TransformSyntax::Invalid)?;
    let number = source[..end]
        .parse::<f32>()
        .map_err(|_| TransformSyntax::Unsupported)?;
    if !number.is_finite() || number.abs() > 10_000.0 {
        return Err(TransformSyntax::Unsupported);
    }
    let unit = &source[end..];
    if unit == "%" {
        return if percentage {
            Ok(Length::Percent(number))
        } else {
            Err(TransformSyntax::Invalid)
        };
    }
    if unit.is_empty() {
        return if number == 0.0 {
            Ok(Length::Px(0.0))
        } else {
            Err(TransformSyntax::Invalid)
        };
    }
    if ["px", "em", "rem", "pt", "pc"]
        .iter()
        .any(|name| unit.eq_ignore_ascii_case(name))
    {
        return parse_length(source, false, true).ok_or(TransformSyntax::Invalid);
    }
    // Known CSS lengths outside the existing finite length consumer are a
    // capability boundary, rather than successful identity translations.
    if [
        "cm", "mm", "q", "in", "ex", "ch", "cap", "ic", "lh", "rlh", "rex", "rch", "rcap", "ric",
        "vw", "vh", "vi", "vb", "vmin", "vmax", "svw", "svh", "svi", "svb", "svmin", "svmax",
        "lvw", "lvh", "lvi", "lvb", "lvmin", "lvmax", "dvw", "dvh", "dvi", "dvb", "dvmin", "dvmax",
        "cqw", "cqh", "cqi", "cqb", "cqmin", "cqmax",
    ]
    .iter()
    .any(|name| unit.eq_ignore_ascii_case(name))
    {
        return Err(TransformSyntax::Unsupported);
    }
    Err(TransformSyntax::Invalid)
}

fn function(source: &str) -> Result<(&str, &str, &str), TransformSyntax> {
    let open = source.find('(').ok_or(TransformSyntax::Invalid)?;
    let name = &source[..open];
    if name.is_empty()
        || !name.bytes().all(|byte| byte.is_ascii_alphanumeric())
        || !name.as_bytes()[0].is_ascii_alphabetic()
    {
        return Err(TransformSyntax::Invalid);
    }
    let mut depth = 1usize;
    for (index, byte) in source.as_bytes().iter().copied().enumerate().skip(open + 1) {
        match byte {
            b'(' => {
                depth += 1;
                if depth > 32 {
                    return Err(TransformSyntax::Unsupported);
                }
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Ok((name, &source[open + 1..index], &source[index + 1..]));
                }
            }
            b'\'' | b'"' | b'\\' | b'[' | b']' | b'{' | b'}' => {
                return Err(TransformSyntax::Invalid)
            }
            _ => {}
        }
    }
    Err(TransformSyntax::Invalid)
}

fn combine<T, U>(
    first: Result<T, TransformSyntax>,
    second: Result<U, TransformSyntax>,
) -> Result<(T, U), TransformSyntax> {
    match (first, second) {
        (Err(TransformSyntax::Invalid), _) | (_, Err(TransformSyntax::Invalid)) => {
            Err(TransformSyntax::Invalid)
        }
        (Err(error), _) | (_, Err(error)) => Err(error),
        (Ok(first), Ok(second)) => Ok((first, second)),
    }
}

struct Arguments<'a> {
    values: [Option<&'a str>; 16],
    count: usize,
}
impl<'a> Arguments<'a> {
    fn push(&mut self, value: &'a str) -> Result<(), TransformSyntax> {
        let value = trim(value);
        if value.is_empty() || self.count == self.values.len() {
            return Err(TransformSyntax::Invalid);
        }
        self.values[self.count] = Some(value);
        self.count += 1;
        Ok(())
    }
    fn get(&self, index: usize) -> &'a str {
        self.values[index].expect("arity checked before argument access")
    }
}

fn arguments(source: &str) -> Result<Arguments<'_>, TransformSyntax> {
    let mut result = Arguments {
        values: [None; 16],
        count: 0,
    };
    if trim(source).is_empty() {
        return Ok(result);
    }
    let mut start = 0;
    let mut depth = 0usize;
    for (index, byte) in source.bytes().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => depth = depth.checked_sub(1).ok_or(TransformSyntax::Invalid)?,
            b',' if depth == 0 => {
                result.push(&source[start..index])?;
                start = index + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(TransformSyntax::Invalid);
    }
    result.push(&source[start..])?;
    Ok(result)
}

// Arithmetic grammar remains unsupported; admit only a balanced known math
// function boundary, never an arbitrary nested function as a length/number.
fn math_value(source: &str) -> Result<(), TransformSyntax> {
    let (name, body, rest) = function(source)?;
    if !trim(rest).is_empty()
        || trim(body).is_empty()
        || ![
            "calc", "min", "max", "clamp", "round", "mod", "rem", "sin", "cos", "tan", "asin",
            "acos", "atan", "atan2", "pow", "sqrt", "hypot", "log", "exp", "abs", "sign",
        ]
        .iter()
        .any(|known| name.eq_ignore_ascii_case(known))
    {
        return Err(TransformSyntax::Invalid);
    }
    Ok(())
}

fn numeric(source: &str, percentage: bool) -> Result<f64, TransformSyntax> {
    let source = trim(source);
    if source.contains('(') {
        math_value(source)?;
        return Err(TransformSyntax::Unsupported);
    }
    let end = number_end(source).ok_or(TransformSyntax::Invalid)?;
    if end != source.len() && !(percentage && &source[end..] == "%") {
        return Err(TransformSyntax::Invalid);
    }
    let value = source[..end]
        .parse::<f64>()
        .map_err(|_| TransformSyntax::Unsupported)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(TransformSyntax::Unsupported)
    }
}

fn angle(source: &str) -> Result<(), TransformSyntax> {
    let source = trim(source);
    if source.contains('(') {
        math_value(source)?;
        return Err(TransformSyntax::Unsupported);
    }
    let end = number_end(source).ok_or(TransformSyntax::Invalid)?;
    let value = numeric(&source[..end], false)?;
    let unit = &source[end..];
    if unit.is_empty() && value == 0.0
        || ["deg", "grad", "rad", "turn"]
            .iter()
            .any(|known| unit.eq_ignore_ascii_case(known))
    {
        Ok(())
    } else {
        Err(TransformSyntax::Invalid)
    }
}

fn unsupported_function(name: &str, arguments: &Arguments<'_>) -> Result<(), TransformSyntax> {
    let (minimum, maximum) = if name.eq_ignore_ascii_case("matrix") {
        (6, 6)
    } else if name.eq_ignore_ascii_case("matrix3d") {
        (16, 16)
    } else if name.eq_ignore_ascii_case("scale3d") {
        (3, 3)
    } else if name.eq_ignore_ascii_case("rotate3d") {
        (4, 4)
    } else if name.eq_ignore_ascii_case("scale") || name.eq_ignore_ascii_case("skew") {
        (1, 2)
    } else {
        (1, 1)
    };
    if !(minimum..=maximum).contains(&arguments.count) {
        return Err(TransformSyntax::Invalid);
    }
    for index in 0..arguments.count {
        let source = arguments.get(index);
        let result = if name.eq_ignore_ascii_case("translateZ") {
            length(source, false).map(|_| ())
        } else if name.eq_ignore_ascii_case("perspective") {
            if source.eq_ignore_ascii_case("none") {
                Ok(())
            } else {
                if let Some(end) = number_end(source) {
                    if numeric(&source[..end], false).is_ok_and(|value| value < 0.0) {
                        return Err(TransformSyntax::Invalid);
                    }
                }
                length(source, false).map(|_| ())
            }
        } else if name
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("skew"))
            || name
                .get(..6)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("rotate"))
                && (!name.eq_ignore_ascii_case("rotate3d") || index == 3)
        {
            angle(source)
        } else {
            numeric(
                source,
                name.get(..5)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("scale")),
            )
            .map(|_| ())
        };
        if matches!(result, Err(TransformSyntax::Invalid)) {
            return result;
        }
    }
    Ok(())
}

fn translation(name: &str, arguments: &str) -> Result<Translation, TransformSyntax> {
    let arguments = self::arguments(arguments)?;
    let function = if name.eq_ignore_ascii_case("translate") {
        Function::Translate(false)
    } else if name.eq_ignore_ascii_case("translateX") {
        Function::X
    } else if name.eq_ignore_ascii_case("translateY") {
        Function::Y
    } else if name.eq_ignore_ascii_case("translate3d") {
        Function::Three
    } else if [
        "matrix",
        "matrix3d",
        "translateZ",
        "scale",
        "scaleX",
        "scaleY",
        "scaleZ",
        "scale3d",
        "rotate",
        "rotateX",
        "rotateY",
        "rotateZ",
        "rotate3d",
        "skew",
        "skewX",
        "skewY",
        "perspective",
    ]
    .iter()
    .any(|known| name.eq_ignore_ascii_case(known))
    {
        unsupported_function(name, &arguments)?;
        return Err(TransformSyntax::Unsupported);
    } else {
        return Err(TransformSyntax::Invalid);
    };
    if arguments.count == 0 {
        return Err(TransformSyntax::Invalid);
    }
    let first = arguments.get(0);
    let second = (arguments.count > 1).then(|| arguments.get(1));
    let third = (arguments.count > 2).then(|| arguments.get(2));
    if arguments.count > 3 {
        return Err(TransformSyntax::Invalid);
    }
    let zero = Length::Px(0.0);
    match (function, second, third) {
        (Function::Translate(_), second, None) => {
            let (x, y) = combine(
                length(first, true),
                second.map_or(Ok(zero), |source| length(source, true)),
            )?;
            Ok(Translation {
                function: Function::Translate(second.is_some()),
                x,
                y,
            })
        }
        (Function::X, None, None) => Ok(Translation {
            function,
            x: length(first, true)?,
            y: zero,
        }),
        (Function::Y, None, None) => Ok(Translation {
            function,
            x: zero,
            y: length(first, true)?,
        }),
        (Function::Three, Some(second), Some(third)) => {
            let ((x, y), z) = combine(
                combine(length(first, true), length(second, true)),
                length(third, false),
            )?;
            if z.resolve(0.0, 1.0, 1.0) != Some(0.0) {
                return Err(TransformSyntax::Unsupported);
            }
            Ok(Translation { function, x, y })
        }
        _ => Err(TransformSyntax::Invalid),
    }
}

fn value(source: &str) -> Result<Value, TransformSyntax> {
    let source = trim(source);
    if source.len() > MAX_TRANSFORM_BYTES {
        return Err(TransformSyntax::Unsupported);
    }
    for (keyword, value) in [
        ("none", Value::None),
        ("initial", Value::Initial),
        ("unset", Value::Initial),
        ("inherit", Value::Inherit),
        ("revert", Value::Revert),
        ("revert-layer", Value::RevertLayer),
    ] {
        if source.eq_ignore_ascii_case(keyword) {
            return Ok(value);
        }
    }
    let mut remaining = source;
    let mut count = 0;
    let mut first = None;
    let mut unsupported = false;
    while !remaining.is_empty() {
        if count == MAX_FUNCTIONS {
            return Ok(Value::Unsupported);
        }
        let (name, arguments, rest) = function(remaining)?;
        match translation(name, arguments) {
            Ok(parsed) => first = Some(parsed),
            Err(TransformSyntax::Unsupported) => unsupported = true,
            Err(error) => return Err(error),
        }
        count += 1;
        remaining = trim(rest);
    }
    match (count, unsupported, first) {
        (1, false, Some(parsed)) => Ok(Value::Translation(parsed)),
        (0, _, _) => Err(TransformSyntax::Invalid),
        _ => Ok(Value::Unsupported),
    }
}

pub(super) fn valid_declaration(source: &str) -> bool {
    !matches!(value(source), Err(TransformSyntax::Invalid)) || custom_properties::has_var(source)
}

fn length_text(length: Length) -> String {
    let (number, unit) = match length {
        Length::Px(number) => (number, "px"),
        Length::Percent(number) => (number, "%"),
        Length::Em(number) => (number, "em"),
        Length::Rem(number) => (number, "rem"),
        Length::Auto => unreachable!("transform grammar never admits auto"),
    };
    format!("{}{unit}", if number == 0.0 { 0.0 } else { number })
}

fn single_value(source: &str) -> bool {
    let mut quote = '\0';
    let mut escaped = false;
    let mut depth = 0usize;
    for ch in source.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote != '\0' {
            if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                quote = '\0';
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = ch,
            '(' | '[' => depth += 1,
            '{' if depth == 0 => return false,
            '{' => depth += 1,
            ')' | ']' | '}' => {
                let Some(next) = depth.checked_sub(1) else {
                    return false;
                };
                depth = next;
            }
            ';' | '!' if depth == 0 => return false,
            _ => {}
        }
    }
    quote == '\0' && depth == 0 && !escaped
}

/// Specified declaration normalization for the style bridge. Before calling,
/// charge the existing bounded CSS source preflight for at most 2048 bytes;
/// it covers all temporary strings and the one returned string. Custom names
/// and var() token spelling remain case-sensitive and unchanged.
pub(crate) fn normalize_transform(source: &str) -> Result<String, TransformSyntax> {
    if source.len() > MAX_TRANSFORM_BYTES {
        return Err(TransformSyntax::Unsupported);
    }
    let raw = trim(source);
    if raw.is_empty() {
        return Ok(String::new());
    }
    let cleaned = without_comments(raw);
    if !single_value(&cleaned) {
        return Err(TransformSyntax::Invalid);
    }
    if custom_properties::has_var(&cleaned) {
        return Ok(raw.to_owned());
    }
    let parsed = value(&cleaned)?;
    Ok(match parsed {
        Value::Translation(parsed) => {
            let x = length_text(parsed.x);
            let y = length_text(parsed.y);
            match parsed.function {
                Function::Translate(false) => format!("translate({x})"),
                Function::Translate(true) => format!("translate({x}, {y})"),
                Function::X => format!("translateX({x})"),
                Function::Y => format!("translateY({y})"),
                Function::Three => format!("translate3d({x}, {y}, 0px)"),
            }
        }
        Value::None => "none".into(),
        Value::Initial => if trim(&cleaned).eq_ignore_ascii_case("unset") {
            "unset"
        } else {
            "initial"
        }
        .into(),
        Value::Inherit => "inherit".into(),
        Value::Revert => "revert".into(),
        Value::RevertLayer => "revert-layer".into(),
        Value::Unsupported => return Err(TransformSyntax::Unsupported),
    })
}

fn computed(parsed: Translation, font: f32, root: f32) -> Option<Transform> {
    let resolve = |length| match length {
        Length::Percent(_) => Some(length),
        Length::Px(value) => {
            (value.is_finite() && value.abs() <= 100_000.0).then_some(Length::Px(value))
        }
        Length::Em(value) | Length::Rem(value) => {
            let basis = if matches!(length, Length::Em(_)) {
                font
            } else {
                root
            };
            let pixels = value * basis;
            (pixels.is_finite() && pixels.abs() <= 100_000.0).then_some(Length::Px(pixels))
        }
        Length::Auto => None,
    };
    Some(Transform::Translate {
        x: resolve(parsed.x)?,
        y: resolve(parsed.y)?,
    })
}

/// Highest-priority-first traversal of the existing selector cascade. Invalid
/// literals are excluded at parse time; unsupported winners remain unknown.
pub(super) fn cascade<'a>(
    parent: Transform,
    parent_known: bool,
    font: f32,
    root: f32,
    declarations: impl Iterator<Item = (&'a Declaration, bool)>,
    variables: &custom_properties::Variables,
    budget: &mut custom_properties::Budget,
) -> (Transform, bool) {
    let mut skip_inline = false;
    for (declaration, is_inline) in declarations {
        if skip_inline && is_inline {
            continue;
        }
        let substituted = custom_properties::has_var(&declaration.value);
        let parsed = if substituted {
            match variables.substitute(&declaration.value, budget) {
                Ok(Some(text)) => value(&text),
                Ok(None) => return (Transform::None, true),
                Err(()) => return (Transform::None, false),
            }
        } else {
            value(&declaration.value)
        };
        match parsed {
            Ok(Value::Translation(parsed)) => {
                return computed(parsed, font, root)
                    .map_or((Transform::None, false), |value| (value, true));
            }
            Ok(Value::None | Value::Initial | Value::Revert) => return (Transform::None, true),
            Ok(Value::Inherit) => return (parent, parent_known),
            Ok(Value::RevertLayer) if is_inline => skip_inline = true,
            Ok(Value::RevertLayer) => return (Transform::None, true),
            Ok(Value::Unsupported) | Err(TransformSyntax::Unsupported) => {
                return (Transform::None, false)
            }
            Err(TransformSyntax::Invalid) if substituted => return (Transform::None, true),
            Err(TransformSyntax::Invalid) => {}
        }
    }
    (Transform::None, true)
}
