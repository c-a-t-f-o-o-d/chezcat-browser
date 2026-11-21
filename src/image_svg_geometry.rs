//! Element-specific SVG geometry values in user coordinates.
//! https://www.w3.org/TR/SVG2/geometry.html
use super::*;
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Value {
    Auto,
    Absolute(f64),
    Percent(f64),
    Em(f64),
    Rem(f64),
}
impl Value {
    fn font(self, font: f64, root: f64) -> Self {
        match self {
            Self::Em(n) => Self::Absolute(n * font),
            Self::Rem(n) => Self::Absolute(n * root),
            value => value,
        }
    }
    fn resolve(self, reference: f64) -> ImageResult<Option<f64>> {
        let value = match self {
            Self::Auto => return Ok(None),
            Self::Absolute(n) => n,
            Self::Percent(n) => n * reference / 100.,
            _ => return Err("Uncomputed SVG font-relative geometry.".into()),
        };
        if !value.is_finite() || value.abs() > 1e7 {
            return Err("SVG length exceeds its coordinate limit.".into());
        }
        Ok(Some(value))
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Geometry {
    values: [Value; 9],
}
impl Default for Geometry {
    fn default() -> Self {
        Self {
            values: [
                Value::Absolute(0.),
                Value::Absolute(0.),
                Value::Absolute(0.),
                Value::Absolute(0.),
                Value::Absolute(0.),
                Value::Auto,
                Value::Auto,
                Value::Auto,
                Value::Auto,
            ],
        }
    }
}
pub(super) fn property(key: &str) -> Option<usize> {
    Some(match key {
        "x" => 0,
        "y" => 1,
        "cx" => 2,
        "cy" => 3,
        "r" => 4,
        "rx" => 5,
        "ry" => 6,
        "width" => 7,
        "height" => 8,
        _ => return None,
    })
}
fn applies(tag: &str, index: usize, outer: bool) -> bool {
    match index {
        0 | 1 => matches!(tag, "svg" | "rect" | "image" | "foreignObject"),
        2 | 3 => matches!(tag, "circle" | "ellipse"),
        4 => tag == "circle",
        5 | 6 => matches!(tag, "rect" | "ellipse"),
        7 | 8 => tag == "svg" && outer || matches!(tag, "rect" | "image" | "foreignObject"),
        _ => false,
    }
}
fn parse(text: &str, css: bool, auto: bool) -> ImageResult<Option<Value>> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("auto") {
        return Ok(auto.then_some(Value::Auto));
    }
    if text.starts_with("var(") || text.starts_with("calc(") {
        return Err("SVG geometry expressions require the shared computed-value path.".into());
    }
    // Split a dimension's unit before reading its number: the e in em/rem
    // is not the beginning of an exponent (while 1e2px remains numeric).
    let lower = text.to_ascii_lowercase();
    let suffix = ["rem", "em", "px", "pt", "pc", "in", "cm", "mm", "q", "%"]
        .into_iter()
        .find(|unit| lower.ends_with(unit));
    let (numeric, unit) = if let Some(unit) = suffix {
        (&text[..text.len() - unit.len()], unit.to_owned())
    } else {
        (text, String::new())
    };
    if css && !numeric.is_empty() && numeric.ends_with(char::is_whitespace) {
        return Ok(None);
    }
    let mut reader = Numbers::new(numeric);
    let number = match reader.raw_number() {
        Ok(n) => n,
        Err(_) => return Ok(None),
    };
    if !numeric[reader.at..].trim().is_empty() {
        return Ok(None);
    }
    if !number.is_finite()||number.abs()>1e7{return Err("SVG coordinate exceeds its finite 10-million-unit limit.".into());}
    let value = match unit.as_str() {
        "" if !css || number == 0. => Value::Absolute(number),
        "" => return Ok(None),
        "px" => Value::Absolute(number),
        "%" => Value::Percent(number),
        "em" => Value::Em(number),
        "rem" => Value::Rem(number),
        "pt" => Value::Absolute(number * 96. / 72.),
        "pc" => Value::Absolute(number * 16.),
        "in" => Value::Absolute(number * 96.),
        "cm" => Value::Absolute(number * 96. / 2.54),
        "mm" => Value::Absolute(number * 96. / 25.4),
        "q" => Value::Absolute(number * 96. / 101.6),
        _ => return Err("Unsupported SVG geometry length unit.".into()),
    };
    Ok(Some(value))
}
pub(super) fn apply(
    style: &mut Style,
    parent: &Style,
    tag: &str,
    key: &str,
    text: &str,
    css: bool,
    outer: bool,
) -> ImageResult<()> {
    let Some(index) = property(key) else {
        return Ok(());
    };
    if !applies(tag, index, outer) {
        return Ok(());
    }
    let initial = Geometry::default().values[index];
    let text = text.trim();
    let value = if text.eq_ignore_ascii_case("inherit") {
        parent
            .geometry
            .as_deref()
            .map_or(initial, |g| g.values[index])
    } else if text.eq_ignore_ascii_case("initial") || text.eq_ignore_ascii_case("unset") {
        initial
    } else if matches!(text, "revert" | "revert-layer") {
        return Err("SVG geometry cascade rollback is not implemented.".into());
    } else {
        let Some(mut value) = parse(text, css, index >= 5)? else {
            return Ok(());
        };
        if index >= 4
            && match value {
                Value::Absolute(n) | Value::Percent(n) | Value::Em(n) | Value::Rem(n) => n < 0.,
                Value::Auto => false,
            }
        {
            value = initial;
        }
        value.font(style.font_size, style.root_font_size)
    };
    Arc::make_mut(
        style
            .geometry
            .get_or_insert_with(|| Arc::new(Geometry::default())),
    )
    .values[index] = value;
    Ok(())
}
pub(super) fn resolve(style: &Style, key: &str, reference: f64) -> ImageResult<Option<f64>> {
    let index = property(key).ok_or("Unknown SVG geometry property.")?;
    style
        .geometry
        .as_deref()
        .map_or_else(|| Geometry::default().values[index], |g| g.values[index])
        .resolve(reference)
}
pub(super) fn font_size(
    style: &mut Style,
    parent: &Style,
    text: &str,
    css: bool,
) -> ImageResult<()> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("inherit") || text.eq_ignore_ascii_case("unset") {
        style.font_size = parent.font_size;
        return Ok(());
    }
    if text.eq_ignore_ascii_case("initial") {
        style.font_size = 16.;
        return Ok(());
    }
    if let Some(value) = parse(text, css, false)? {
        if let Some(value) = value
            .font(parent.font_size, parent.root_font_size)
            .resolve(parent.font_size)?
        {
            if value >= 0. {
                style.font_size = value;
            }
        }
    }
    Ok(())
}

pub(super) fn valid(tag: &str, key: &str, text: &str, outer: bool) -> ImageResult<bool> {
    let Some(index) = property(key) else { return Ok(false); };
    if !applies(tag,index,outer) { return Ok(false); }
    if matches!(text.trim().to_ascii_lowercase().as_str(),"inherit"|"initial"|"unset"|"revert"|"revert-layer") { return Ok(true); }
    Ok(parse(text,true,index>=5)?.is_some())
}
