//! Exact signed integer z levels within the existing declaration/storage guards.
use super::*;
use std::cmp::Ordering;
use std::rc::Rc;
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Value {
    text: String,
}
impl Value {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }
    pub(crate) fn sign(&self) -> i8 {
        if self.text == "0" {
            0
        } else if self.text.starts_with('-') {
            -1
        } else {
            1
        }
    }
    fn magnitude(&self) -> &str {
        self.text.strip_prefix('-').unwrap_or(&self.text)
    }
}
impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        let sign = self.sign().cmp(&other.sign());
        if sign != Ordering::Equal {
            return sign;
        }
        let a = self.magnitude();
        let b = other.magnitude();
        let order = a.len().cmp(&b.len()).then_with(|| a.cmp(b));
        if self.sign() < 0 {
            order.reverse()
        } else {
            order
        }
    }
}
impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
fn parts(source: &str) -> Option<(bool, &str)> {
    let source = source.trim_ascii();
    let negative = source.starts_with('-');
    let digits = source.strip_prefix(['+', '-']).unwrap_or(source);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let digits = digits.trim_start_matches('0');
    Some((
        negative && !digits.is_empty(),
        if digits.is_empty() { "0" } else { digits },
    ))
}
fn integer(source: &str) -> Option<String> {
    let (negative, digits) = parts(source)?;
    let bytes = digits.len().checked_add(usize::from(negative))?;
    if bytes > MAX_GROUPED_BOX_BYTES {
        return None;
    }
    let mut text = String::new();
    text.try_reserve_exact(bytes).ok()?;
    if negative {
        text.push('-');
    }
    text.push_str(digits);
    Some(text)
}
pub(crate) fn valid(source: &str) -> bool {
    let value = source.trim_ascii();
    [
        "auto",
        "inherit",
        "initial",
        "unset",
        "revert",
        "revert-layer",
    ]
    .iter()
    .any(|s| value.eq_ignore_ascii_case(s))
        || custom_properties::has_var(value) && custom_properties::valid_syntax(value)
        || parts(value).is_some_and(|(negative, digits)| {
            digits.len() + usize::from(negative) <= MAX_GROUPED_BOX_BYTES
        })
}
pub(crate) fn specified(source: &str) -> Option<String> {
    if !valid(source) {
        return None;
    }
    if custom_properties::has_var(source) {
        return Some(source.trim_ascii().to_owned());
    }
    integer(source).or_else(|| Some(source.trim_ascii().to_ascii_lowercase()))
}
pub(super) fn apply(
    style: &mut Style,
    parent: &Style,
    decl: &Declaration,
    inline: bool,
    sheet: &mut Option<Rc<Value>>,
    variables: &custom_properties::Variables,
    budget: &mut custom_properties::Budget,
) -> bool {
    if decl.name != "z-index" {
        return false;
    }
    let substituted = custom_properties::has_var(&decl.value);
    let text;
    let source = if substituted {
        match variables.substitute(&decl.value, budget) {
            Ok(Some(value)) => {
                text = value;
                text.as_str()
            }
            _ => "unset",
        }
    } else {
        decl.value.as_str()
    };
    let value = source.trim_ascii();
    let parsed = if inline && value.eq_ignore_ascii_case("revert-layer") {
        Some(sheet.clone())
    } else if value.eq_ignore_ascii_case("inherit") {
        Some(parent.z_index.clone())
    } else if ["auto", "initial", "unset", "revert", "revert-layer"]
        .iter()
        .any(|s| value.eq_ignore_ascii_case(s))
    {
        Some(None)
    } else if parts(value).is_some() {
        if budget
            .work(value.len())
            .and_then(|_| {
                budget.copy(value.len().saturating_add(
                    std::mem::size_of::<Value>() + 2 * std::mem::size_of::<usize>(),
                ))
            })
            .is_err()
        {
            Some(None)
        } else {
            integer(value).map(|text| Some(Rc::new(Value { text })))
        }
    } else {
        None
    };
    if let Some(value) = parsed.or_else(|| substituted.then_some(None)) {
        style.z_index = value;
        if !inline {
            *sheet = style.z_index.clone();
        }
    }
    true
}
