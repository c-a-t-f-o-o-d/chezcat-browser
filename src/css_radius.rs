//! Finite circular radius state from the real ordered cascade. Painting admits
//! only one equal absolute radius on borderless block backgrounds. Per-corner
//! slots preserve shorthand/longhand precedence without inventing a circle for
//! an elliptical, percentage, unequal or unavailable winning value.
//! https://www.w3.org/TR/css-backgrounds-3/#border-radius
use super::*;

pub(super) fn property(name: &str) -> bool {
    corner(name).is_some() || name == "border-radius"
}

fn corner(name: &str) -> Option<usize> {
    match name {
        "border-top-left-radius" => Some(0),
        "border-top-right-radius" => Some(1),
        "border-bottom-right-radius" => Some(2),
        "border-bottom-left-radius" => Some(3),
        _ => None,
    }
}

fn length(source: &str) -> Option<Option<Length>> {
    // Reuse the existing complete ASCII CSS number/dimension grammar. Negative
    // radii are invalid even when their units exceed this renderer's subset.
    if translate::number_end(source).and_then(|end| source[..end].parse::<f32>().ok()).is_some_and(|value| value < 0.0) {
        return None;
    }
    match translate::length(source, true) {
        Ok(Length::Px(value) | Length::Em(value) | Length::Rem(value)) if value < 0.0 => None,
        Ok(Length::Percent(value)) if value < 0.0 => None,
        Ok(Length::Percent(_)) | Err(TransformSyntax::Unsupported) => Some(None),
        Ok(value) => Some(Some(value)),
        Err(TransformSyntax::Invalid) => None,
    }
}

fn axis(source: &str) -> Option<[Option<Length>; 4]> {
    let parts = border_parts(source)?;
    let mut lengths = [None; 4];
    for (index, part) in parts.iter().enumerate() {
        lengths[index] = length(part)?;
    }
    four(&lengths[..parts.len()])
}

fn radii(source: &str, single_corner: bool) -> Option<[Option<Length>; 4]> {
    // Keep the existing grouped border-component limits. A refused source is
    // unavailable, rather than permission to resurrect a lower circular value.
    if source.len() > 2048 { return Some([None; 4]); }
    let mut depth = 0usize;
    let mut slash = None;
    for (index, byte) in source.bytes().enumerate() {
        match byte {
            b'(' => {
                depth += 1;
                if depth > 8 { return Some([None; 4]); }
            }
            b')' => depth = depth.checked_sub(1)?,
            b'/' if depth == 0 => {
                if slash.replace(index).is_some() { return None; }
            }
            _ => {}
        }
    }
    if depth != 0 { return None; }
    // A corner longhand has two space-separated axes; the shorthand uses '/'.
    // border_parts preserves whole functions, including internal whitespace and
    // division. Only the top-level slash found above separates radius axes.
    if single_corner {
        if slash.is_some() { return None; }
        let parts = border_parts(source)?;
        let (x, y) = match parts.as_slice() {
            [x] => { let x = length(x)?; (x, x) }
            [x, y] => (length(x)?, length(y)?),
            _ => return None,
        };
        return Some([if x == y { x } else { None }; 4]);
    }
    let (horizontal, vertical) = if let Some(index) = slash {
        (axis(&source[..index])?, axis(&source[index + 1..])?)
    } else {
        let horizontal = axis(source)?;
        (horizontal, horizontal)
    };
    Some(std::array::from_fn(|index| {
        if horizontal[index] == vertical[index] { horizontal[index] } else { None }
    }))
}

pub(super) fn cascade<'a>(
    parent: [Option<Length>; 4],
    font: f32,
    root_font: f32,
    declarations: impl Iterator<Item = (&'a Declaration, bool)>,
    variables: &custom_properties::Variables,
    budget: &mut custom_properties::Budget,
) -> [Option<Length>; 4] {
    let mut result = [Some(Length::Px(0.0)); 4];
    let mut done = [false; 4];
    let mut skip_inline = [false; 4];
    for (declaration, is_inline) in declarations {
        if !property(&declaration.name) { continue; }
        let affected: [bool; 4] = std::array::from_fn(|index| {
            !done[index] && !(skip_inline[index] && is_inline)
                && corner(&declaration.name).is_none_or(|corner| corner == index)
        });
        if !affected.iter().any(|value| *value) { continue; }
        let substituted = custom_properties::has_var(&declaration.value);
        let text = if substituted {
            match variables.substitute(&declaration.value, budget) {
                Ok(Some(value)) => value,
                Ok(None) => "initial".into(),
                Err(()) => {
                    for index in 0..4 { if affected[index] { result[index] = None; done[index] = true; } }
                    continue;
                }
            }
        } else {
            // The generic cascade source charge covers this bounded copy.
            declaration.value.clone()
        };
        let text = text.trim().to_ascii_lowercase();
        if text == "revert-layer" && is_inline {
            for index in 0..4 { if affected[index] { skip_inline[index] = true; } }
            continue;
        }
        let value = match text.as_str() {
            "inherit" => Some(parent),
            "initial" | "unset" | "revert" | "revert-layer" => Some([Some(Length::Px(0.0)); 4]),
            _ => radii(&text, corner(&declaration.name).is_some()),
        };
        let Some(value) = value.or_else(|| substituted.then_some([Some(Length::Px(0.0)); 4])) else { continue; };
        for index in 0..4 {
            if affected[index] {
                result[index] = value[index].and_then(|length| length.resolve(0.0, font, root_font)).map(Length::Px);
                done[index] = true;
            }
        }
        if done == [true; 4] { break; }
    }
    result
}

pub(super) fn uniform(radii: [Option<Length>; 4]) -> Option<f32> {
    let Some(Length::Px(radius)) = radii[0] else { return None; };
    (radius.is_finite() && radius > 0.0 && radii == [Some(Length::Px(radius)); 4]).then_some(radius)
}

#[cfg(test)]
#[path = "css_radius_tests.rs"]
mod tests;
