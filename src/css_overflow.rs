//! Physical overflow axes from the same cascade that supplies rendering.
//! https://www.w3.org/TR/css-overflow-3/#overflow-properties
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverflowValue {
    #[default]
    Visible,
    Clip,
    Hidden,
    Auto,
    Scroll,
}
impl OverflowValue {
    pub fn as_css(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::Clip => "clip",
            Self::Hidden => "hidden",
            Self::Auto => "auto",
            Self::Scroll => "scroll",
        }
    }
    pub fn clips(self) -> bool {
        self != Self::Visible
    }
    fn scrollable(self) -> bool {
        matches!(self, Self::Hidden | Self::Auto | Self::Scroll)
    }
    fn computed(self) -> Self {
        match self {
            Self::Visible => Self::Auto,
            Self::Clip => Self::Hidden,
            other => other,
        }
    }
    fn parse(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("visible") {
            Some(Self::Visible)
        } else if value.eq_ignore_ascii_case("clip") {
            Some(Self::Clip)
        } else if value.eq_ignore_ascii_case("hidden") {
            Some(Self::Hidden)
        } else if value.eq_ignore_ascii_case("auto") || value.eq_ignore_ascii_case("overlay") {
            Some(Self::Auto)
        } else if value.eq_ignore_ascii_case("scroll") {
            Some(Self::Scroll)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Overflow {
    pub x: OverflowValue,
    pub y: OverflowValue,
}
impl Overflow {
    /// Canonical shorthand: identical axes use one token. Callers can charge
    /// exact output bytes from x/y.as_css() before allocating this String.
    pub fn to_css(self) -> String {
        if self.x == self.y {
            self.x.as_css().into()
        } else {
            format!("{} {}", self.x.as_css(), self.y.as_css())
        }
    }
    pub fn clips_x(self) -> bool {
        self.x.clips()
    }
    pub fn clips_y(self) -> bool {
        self.y.clips()
    }
    pub(super) fn computed(self) -> Self {
        if self.x.scrollable() || self.y.scrollable() {
            Self {
                x: self.x.computed(),
                y: self.y.computed(),
            }
        } else {
            self
        }
    }
    pub(super) fn computed_availability(self, known: &mut [bool; 2]) {
        let original = *known;
        if !original[0] && !self.y.scrollable() {
            known[1] = false;
        }
        if !original[1] && !self.x.scrollable() {
            known[0] = false;
        }
    }
}

pub(super) fn property(name: &str) -> bool {
    matches!(name, "overflow" | "overflow-x" | "overflow-y")
}

pub(super) fn set_availability(name: &str, known: &mut [bool; 2], source: [bool; 2]) {
    match name {
        "overflow" => *known = source,
        "overflow-x" => known[0] = source[0],
        "overflow-y" => known[1] = source[1],
        _ => unreachable!(),
    }
}

pub(super) fn apply_with_availability(
    style: &mut Style,
    parent: &Style,
    name: &str,
    value: &str,
    known: &mut [bool; 2],
    parent_known: [bool; 2],
) -> bool {
    if !apply(style, parent, name, value) {
        return false;
    }
    let source = if value.eq_ignore_ascii_case("inherit") {
        parent_known
    } else {
        [true; 2]
    };
    set_availability(name, known, source);
    true
}

/// Apply a specified value without cross-axis normalization. Only the final
/// cascade pair computes; normalizing an earlier declaration would keep an
/// obsolete promotion after a later longhand overrides the other axis.
pub(super) fn apply(style: &mut Style, parent: &Style, name: &str, value: &str) -> bool {
    if !property(name) {
        return false;
    }
    let source = if value.eq_ignore_ascii_case("inherit") {
        Some(parent.overflow)
    } else if value.eq_ignore_ascii_case("initial") || value.eq_ignore_ascii_case("unset") {
        Some(Overflow::default())
    } else {
        None
    };
    if let Some(source) = source {
        match name {
            "overflow" => style.overflow = source,
            "overflow-x" => style.overflow.x = source.x,
            "overflow-y" => style.overflow.y = source.y,
            _ => unreachable!(),
        }
        return true;
    }
    let mut words = value.split_ascii_whitespace();
    let Some(first) = words.next().and_then(OverflowValue::parse) else {
        return false;
    };
    match name {
        "overflow" => {
            let second = match words.next() {
                Some(word) => match OverflowValue::parse(word) {
                    Some(value) => value,
                    None => return false,
                },
                None => first,
            };
            if words.next().is_some() {
                return false;
            }
            style.overflow = Overflow {
                x: first,
                y: second,
            };
        }
        "overflow-x" | "overflow-y" => {
            if words.next().is_some() {
                return false;
            }
            if name == "overflow-x" {
                style.overflow.x = first;
            } else {
                style.overflow.y = first;
            }
        }
        _ => unreachable!(),
    }
    true
}
