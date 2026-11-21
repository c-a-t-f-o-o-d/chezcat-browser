//! Shared matched declarations and variables for inherited SVG paint values.
use super::*;
use std::sync::Arc;
#[allow(clippy::too_many_arguments)]
pub(super) fn compute<'a>(
    doc: &Document,
    node: usize,
    parent: &Style,
    declarations: impl Iterator<Item = &'a Declaration> + Clone,
    variables: &custom_properties::Variables,
    budget: &mut custom_properties::Budget,
) -> (Option<Arc<crate::image::SvgPaint>>, bool) {
    let hints = if doc.nodes[node].namespace == crate::html::Namespace::Svg {
        match &doc.nodes[node].kind {
            NodeKind::Element { attrs, .. } => Some(attrs),
            _ => None,
        }
    } else {
        None
    };
    if !hints.is_some_and(|attrs| {
        attrs.iter().any(|a| {
            a.name.namespace.is_none() && crate::image::SvgPaint::property(&a.name.qualified)
        })
    }) && !declarations
        .clone()
        .any(|d| crate::image::SvgPaint::property(&d.name))
    {
        return (parent.svg_paint.clone(), parent.svg_paint_known);
    }
    if budget
        .copy(std::mem::size_of::<crate::image::SvgPaint>() + 2 * std::mem::size_of::<usize>())
        .is_err()
    {
        return (None, false);
    }
    let initial = 0xff000000 | INITIAL_COLOR;
    let inherited = if parent.svg_paint_known {
        parent
            .svg_paint
            .as_deref()
            .cloned()
            .unwrap_or_else(|| crate::image::SvgPaint::new(0xff000000 | parent.color))
    } else {
        crate::image::SvgPaint::unavailable()
    };
    let mut value = inherited.clone();
    if let Some(attrs) = hints {
        for attr in attrs {
            if attr.name.namespace.is_none()
                && crate::image::SvgPaint::property(&attr.name.qualified)
            {
                if budget.copy(parse_cost(&attr.value)).is_err() {
                    return (None, false);
                }
                value.apply(&inherited, &attr.name.qualified, &attr.value, initial);
            }
        }
    }
    for d in declarations {
        if crate::image::SvgPaint::property(&d.name) {
            if budget.work(d.value.len()).is_err() {
                return (None, false);
            }
            let substituted;
            let text = if custom_properties::has_var(&d.value) {
                substituted = match variables.substitute(&d.value, budget) {
                    Ok(value) => value,
                    Err(_) => return (None, false),
                };
                substituted.as_deref().unwrap_or("unset")
            } else {
                d.value.as_str()
            };
            if budget.copy(parse_cost(text)).is_err() {
                return (None, false);
            }
            value.apply(&inherited, &d.name, text, initial);
        }
    }
    (Some(Arc::new(value)), true)
}

// Lowercase/normalized strings and the original RGB parser's component vector,
// including spare geometric capacity and fixed allocation metadata.
fn parse_cost(text: &str) -> usize {
    let parts = if text
        .trim_start()
        .get(..3)
        .is_some_and(|s| s.eq_ignore_ascii_case("rgb"))
    {
        text.bytes()
            .filter(|&b| b == b',' || b == b'/' || b.is_ascii_whitespace())
            .count()
            .saturating_add(1)
    } else {
        1
    };
    text.len()
        .saturating_mul(3)
        .saturating_add(parts.saturating_mul(2 * std::mem::size_of::<&str>()))
}
// Only SVG content and its actual ancestors need inherited paint state. Ordinary
// HTML siblings do not acquire new per-element paint allocations or work.
pub(super) fn needed(
    doc: &Document,
    order: &[usize],
    parents: &[Option<usize>],
    inert: &[bool],
    mut meter: Option<&mut CssBudget<'_>>,
    mut transform_meter: Option<&mut CssBudget<'_>>,
) -> Result<Vec<bool>, String> {
    if !order
        .iter()
        .any(|&id| !inert[id] && doc.nodes[id].namespace == crate::html::Namespace::Svg)
    {
        return Ok(Vec::new());
    }
    let charge = |meter: &mut CssBudget<'_>| -> Result<(), String> {
        meter.work(order.len().saturating_mul(2))?;
        meter.copies(doc.nodes.len())
    };
    if let Some(meter) = meter.as_deref_mut() {
        charge(meter)?;
    } else if let Some(meter) = transform_meter.as_deref_mut() {
        charge(meter)?;
    }
    let mut needed = Vec::new();
    needed
        .try_reserve_exact(doc.nodes.len())
        .map_err(|_| "SVG paint ancestry reservation failed.")?;
    let extra = needed.capacity().saturating_sub(doc.nodes.len());
    if let Some(meter) = meter.as_deref_mut() {
        meter.copies(extra)?;
    } else if let Some(meter) = transform_meter.as_deref_mut() {
        meter.copies(extra)?;
    }
    needed.resize(doc.nodes.len(), false);
    for &id in order.iter().rev() {
        if !inert[id] && (needed[id] || doc.nodes[id].namespace == crate::html::Namespace::Svg) {
            needed[id] = true;
            if let Some(parent) = parents[id] {
                needed[parent] = true;
            }
        }
    }
    Ok(needed)
}
