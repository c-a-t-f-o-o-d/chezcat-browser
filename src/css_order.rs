use super::*;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::rc::Rc;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Value(Option<Rc<z_index::Value>>);

impl Value {
    pub(crate) fn text(&self) -> &str {
        self.0.as_ref().map_or("0", |value| value.text())
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0.is_none()
    }

    fn parse(source: &str) -> Option<Self> {
        let value = z_index::Value::from_integer(source)?;
        Some(if value.sign() == 0 { Self::default() } else { Self(Some(Rc::new(value))) })
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => a.cmp(b),
            (Some(a), None) => a.sign().cmp(&0),
            (None, Some(b)) => 0.cmp(&b.sign()),
            (None, None) => Ordering::Equal,
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn wide(value: &str) -> bool {
    ["inherit", "initial", "unset", "revert", "revert-layer"]
        .iter().any(|keyword| value.eq_ignore_ascii_case(keyword))
}

pub(crate) fn valid(source: &str) -> bool {
    let source = cleaned(source);
    let value = source.trim_ascii();
    wide(value) || z_index::valid_integer(value)
        || custom_properties::has_var(value) && custom_properties::valid_syntax(value)
}

pub(crate) fn specified(source: &str) -> Option<String> {
    let source = cleaned(source);
    let value = source.trim_ascii();
    if wide(value) {
        Some(value.to_ascii_lowercase())
    } else if z_index::valid_integer(value) {
        z_index::integer(value)
    } else if custom_properties::has_var(value) && custom_properties::valid_syntax(value) {
        Some(value.to_owned())
    } else {
        None
    }
}

fn cleaned(source: &str) -> Cow<'_, str> {
    if source.contains("/*") { Cow::Owned(without_comments(source)) }
    else { Cow::Borrowed(source) }
}

pub(crate) fn source_cost(bytes: usize) -> (usize, usize) {
    let copies = bytes.saturating_mul(64);
    let work = bytes.max(1).div_ceil(64).saturating_mul(512).saturating_add(copies);
    (work, copies)
}

fn parser_storage(source: &str, budget: &mut custom_properties::Budget) -> Result<(), ()> {
    budget.work(source.len().max(1).div_ceil(64).saturating_mul(512))?;
    budget.copy(source.len().saturating_mul(64))
}

pub(super) fn apply(
    style: &mut Style,
    parent: &Style,
    declaration: &Declaration,
    inline: bool,
    sheet: &mut Value,
    variables: &custom_properties::Variables,
    budget: &mut custom_properties::Budget,
) -> Result<(), ()> {
    parser_storage(&declaration.value, budget)?;
    let variable = custom_properties::has_var(&declaration.value);
    let substituted;
    let source = if variable {
        substituted = match variables.substitute(&declaration.value, budget) {
            Ok(Some(value)) => Some(value),
            Ok(None) => None,
            Err(()) => return Err(()),
        };
        if let Some(value) = substituted.as_deref() {
            parser_storage(value, budget)?;
        }
        substituted.as_deref().unwrap_or("unset")
    } else {
        declaration.value.as_str()
    };
    let value = source.trim_ascii();
    let next = if inline && value.eq_ignore_ascii_case("revert-layer") {
        Some(sheet.clone())
    } else if value.eq_ignore_ascii_case("inherit") {
        Some(parent.order.clone())
    } else if wide(value) {
        Some(Value::default())
    } else if z_index::valid_integer(value) {
        budget.copy(value.len().saturating_add(
            std::mem::size_of::<z_index::Value>() + 2 * std::mem::size_of::<usize>(),
        ))?;
        Some(Value::parse(value).ok_or(())?)
    } else if variable {
        Some(Value::default())
    } else {
        None
    };
    if let Some(next) = next {
        style.order = next;
        if !inline { *sheet = style.order.clone(); }
    }
    Ok(())
}
