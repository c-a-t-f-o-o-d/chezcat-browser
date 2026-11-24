//! Finite physical image positions and their companion presentation properties.
//! https://www.w3.org/TR/css-images-3/#the-object-position
//! https://www.w3.org/TR/css-backgrounds-3/#background-position
use super::{
    border_parts, inset_length, sizing_math, BackgroundSize, ImageRendering, Length,
    MediaEnvironment, ObjectFit, ObjectPosition, Style, MAX_GROUPED_BOX_BYTES,
};

pub(super) fn property(name: &str) -> bool {
    matches!(
        name,
        "object-position" | "background-position" | "object-fit" | "image-rendering"
            | "background-repeat" | "background-size"
    )
}

fn fit(value: &str) -> Option<ObjectFit> {
    Some(match value {
        "fill" => ObjectFit::Fill,
        "contain" => ObjectFit::Contain,
        "cover" => ObjectFit::Cover,
        "none" => ObjectFit::None,
        "scale-down" => ObjectFit::ScaleDown,
        _ => return None,
    })
}

fn rendering(value: &str) -> Option<ImageRendering> {
    Some(match value {
        "auto" => ImageRendering::Auto,
        "smooth" | "optimizequality" => ImageRendering::Smooth,
        "high-quality" => ImageRendering::HighQuality,
        "crisp-edges" | "optimizespeed" => ImageRendering::CrispEdges,
        "pixelated" => ImageRendering::Pixelated,
        _ => return None,
    })
}

fn repeat(value: &str) -> Option<[bool; 2]> {
    Some(match value {
        "repeat" => [true, true],
        "no-repeat" => [false, false],
        "repeat-x" => [true, false],
        "repeat-y" => [false, true],
        _ => return None,
    })
}

fn size(value: &str) -> Option<BackgroundSize> {
    Some(match value {
        "auto" => BackgroundSize::Auto,
        "contain" => BackgroundSize::Contain,
        "cover" => BackgroundSize::Cover,
        _ => return None,
    })
}

pub(super) fn valid(name: &str, value: &str) -> bool {
    let value = value.trim_ascii().to_ascii_lowercase();
    match name {
        "object-position" => position(&value, false).is_some(),
        "background-position" => position(&value, true).is_some(),
        "object-fit" => fit(&value).is_some(),
        "image-rendering" => rendering(&value).is_some(),
        "background-repeat" => repeat(&value).is_some(),
        "background-size" => size(&value).is_some(),
        _ => false,
    }
}

fn keyword(value: &str) -> Option<(usize, f32)> {
    match value {
        "left" => Some((0, 0.0)),
        "right" => Some((0, 1.0)),
        "top" => Some((1, 0.0)),
        "bottom" => Some((1, 1.0)),
        _ => None,
    }
}

fn component(value: &str) -> Option<ObjectPosition> {
    if value == "center" {
        return Some(ObjectPosition::CENTER);
    }
    let (fraction, offset) = match inset_length(value)? {
        Length::Auto => return None,
        Length::Percent(value) => (value / 100.0, Length::Px(0.0)),
        Length::Calc(mut value) => {
            // Painting applies fractions to the free space after image sizing.
            // Its separate length offset is evaluated with a zero reference.
            let fraction = value.coefficients[1] / 100.0;
            value.coefficients[1] = 0.0;
            value.percentage = false;
            (fraction, Length::Calc(value))
        }
        offset => (0.0, offset),
    };
    Some(ObjectPosition { fraction, offset })
}

fn negate(offset: Length) -> Length {
    match offset {
        Length::Px(value) => Length::Px(-value),
        Length::Em(value) => Length::Em(-value),
        Length::Rem(value) => Length::Rem(-value),
        Length::Percent(value) => Length::Percent(-value),
        Length::Calc(mut value) => {
            value.coefficients = value.coefficients.map(|coefficient| -coefficient);
            Length::Calc(value)
        }
        Length::Auto => Length::Auto,
    }
}

fn edge(value: &str, offset: Option<&str>) -> Option<(usize, ObjectPosition)> {
    let (axis, fraction) = keyword(value)?;
    let result = if let Some(offset) = offset {
        if offset == "center" {
            return None;
        }
        let mut position = component(offset)?;
        if fraction == 1.0 {
            position.fraction = 1.0 - position.fraction;
            position.offset = negate(position.offset);
        }
        position
    } else {
        ObjectPosition { fraction, offset: Length::Px(0.0) }
    };
    Some((axis, result))
}

fn axes(first: (usize, ObjectPosition), second: (usize, ObjectPosition)) -> Option<[ObjectPosition; 2]> {
    if first.0 == second.0 {
        return None;
    }
    let mut result = [ObjectPosition::CENTER; 2];
    result[first.0] = first.1;
    result[second.0] = second.1;
    Some(result)
}

fn three(edge_name: &str, offset: &str, single: &str) -> Option<[ObjectPosition; 2]> {
    let first = edge(edge_name, Some(offset))?;
    let second = if single == "center" {
        (1 - first.0, ObjectPosition::CENTER)
    } else {
        edge(single, None)?
    };
    axes(first, second)
}

pub(super) fn position(value: &str, background: bool) -> Option<[ObjectPosition; 2]> {
    let value = value.trim_ascii();
    if value.len() > MAX_GROUPED_BOX_BYTES || !value.is_ascii() {
        return None;
    }
    let value = value.to_ascii_lowercase();
    let words = border_parts(&value)?;
    let mut result = [ObjectPosition::CENTER; 2];
    match words.as_slice() {
        [value] => {
            if let Some((axis, position)) = edge(value, None) {
                result[axis] = position;
            } else {
                result[0] = component(value)?;
            }
        }
        [first, second] => match (keyword(first), keyword(second)) {
            (Some((a, _)), Some((b, _))) if a != b => {
                result[a] = edge(first, None)?.1;
                result[b] = edge(second, None)?.1;
            }
            (Some((0, _)), None) => {
                result[0] = edge(first, None)?.1;
                result[1] = component(second)?;
            }
            (Some((1, _)), None) if *second == "center" => {
                result[1] = edge(first, None)?.1;
            }
            (None, Some((1, _))) => {
                result[0] = component(first)?;
                result[1] = edge(second, None)?.1;
            }
            (None, Some((0, _))) if *first == "center" => {
                result[0] = edge(second, None)?.1;
            }
            (None, None) => result = [component(first)?, component(second)?],
            _ => return None,
        },
        [first, second, third] if background => {
            return three(first, second, third).or_else(|| three(second, third, first));
        }
        [first, first_offset, second, second_offset] => {
            return axes(edge(first, Some(first_offset))?, edge(second, Some(second_offset))?);
        }
        _ => return None,
    }
    Some(result)
}

pub(super) fn reset(style: &mut Style, parent: &Style, name: &str, inherit: bool) {
    match name {
        "object-position" => {
            style.object_position = if inherit { parent.object_position } else { [ObjectPosition::CENTER; 2] };
        }
        "background-position" => {
            style.background_position = if inherit { parent.background_position } else { [ObjectPosition::START; 2] };
        }
        "object-fit" => style.object_fit = if inherit { parent.object_fit } else { ObjectFit::Fill },
        "image-rendering" => style.image_rendering = if inherit { parent.image_rendering } else { ImageRendering::Auto },
        "background-repeat" => style.background_repeat = if inherit { parent.background_repeat } else { [true; 2] },
        "background-size" => style.background_size = if inherit { parent.background_size } else { BackgroundSize::Auto },
        _ => {}
    }
}

pub(super) fn apply(style: &mut Style, parent: &Style, name: &str, value: &str, has_element_parent: bool) {
    let value = value.trim_ascii().to_ascii_lowercase();
    if matches!(value.as_str(), "inherit" | "initial" | "unset") {
        let inherit = has_element_parent
            && (value == "inherit" || value == "unset" && name == "image-rendering");
        reset(style, parent, name, inherit);
        return;
    }
    match name {
        "object-position" => {
            if let Some(value) = position(&value, false) { style.object_position = value; }
        }
        "background-position" => {
            if let Some(value) = position(&value, true) { style.background_position = value; }
        }
        "object-fit" => {
            if let Some(value) = fit(&value) { style.object_fit = value; }
        }
        "image-rendering" => {
            if let Some(value) = rendering(&value) { style.image_rendering = value; }
        }
        "background-repeat" => {
            if let Some(value) = repeat(&value) { style.background_repeat = value; }
        }
        "background-size" => {
            if let Some(value) = size(&value) { style.background_size = value; }
        }
        _ => {}
    }
}

fn computed_axis(mut position: ObjectPosition, style: &Style, environment: Option<MediaEnvironment>) -> ObjectPosition {
    position.offset = match sizing_math::resolve_viewport(position.offset, environment) {
        Length::Em(value) => Length::Px((value as f64 * style.font_size as f64)
            .clamp(-(f32::MAX as f64), f32::MAX as f64) as f32),
        Length::Rem(value) => Length::Px((value as f64 * style.root_font_size as f64)
            .clamp(-(f32::MAX as f64), f32::MAX as f64) as f32),
        Length::Calc(value) => Length::Calc(value.inherited(style)),
        value => value,
    };
    position
}

pub(super) fn computed(style: &mut Style, environment: Option<MediaEnvironment>) {
    // Parents finish this step before descendants enter the cascade. Explicit
    // inheritance, including the background shorthand, then copies computed
    // offsets without applying the descendant's font metrics a second time.
    let object = style.object_position.map(|position| computed_axis(position, style, environment));
    let background = style.background_position.map(|position| computed_axis(position, style, environment));
    style.object_position = object;
    style.background_position = background;
}
