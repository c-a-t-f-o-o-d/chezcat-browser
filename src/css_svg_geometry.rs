//! SVG geometry uses the document's matched, priority-sorted declarations and
//! variable environment. The image painter receives typed values, never a
//! stylesheet rewritten into DOM attributes. Unsupported winners stay unknown.
use super::*;
#[allow(clippy::too_many_arguments)]
pub(super) fn compute<'a>(
    doc: &Document,
    node: usize,
    tag: &str,
    outer: bool,
    parent: Option<&crate::image::SvgGeometry>,
    parent_known: bool,
    font: f32,
    root: f32,
    font_known: bool,
    declarations: impl Iterator<Item = &'a Declaration>,
    variables: &custom_properties::Variables,
    budget: &mut custom_properties::Budget,
) -> Option<crate::image::SvgGeometry> {
    if !font_known {
        return None;
    }
    let mut value = crate::image::SvgGeometry::new(font as f64, root as f64);
    if let NodeKind::Element { attrs, .. } = &doc.nodes[node].kind {
        for attr in attrs {
            if attr.name.namespace.is_some()
                || !crate::image::SvgGeometry::property(&attr.name.qualified)
            {
                continue;
            }
            budget.work(attr.value.len().checked_add(1)?).ok()?;
            value
                .apply(parent, tag, &attr.name.qualified, &attr.value, false, outer)
                .ok()?;
        }
    }
    for d in declarations {
        if !crate::image::SvgGeometry::property(&d.name) {
            continue;
        }
        budget.work(d.value.len().checked_add(1)?).ok()?;
        let substituted;
        let text = if custom_properties::has_var(&d.value) {
            substituted = variables.substitute(&d.value, budget).ok()?;
            match substituted.as_deref() {
                Some(text) => match crate::image::SvgGeometry::valid(tag, &d.name, text, outer) {
                    Ok(true) => text,
                    Ok(false) => "initial",
                    Err(_) => {
                        value.mark_unknown(&d.name);
                        continue;
                    }
                },
                None => "initial",
            }
        } else {
            d.value.as_str()
        };
        match crate::image::SvgGeometry::valid(tag, &d.name, text, outer) {
            Ok(false) => continue,
            Err(_) => {
                value.mark_unknown(&d.name);
                continue;
            }
            Ok(true) => {}
        }
        if text.trim().eq_ignore_ascii_case("inherit") && !parent_known {
            value.mark_unknown(&d.name);
            continue;
        }
        value.apply(parent, tag, &d.name, text, true, outer).ok()?;
    }
    Some(value)
}
