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
    parent: Option<&crate::image::SvgStyle>,
    parent_known: bool,
    font: f32,
    root: f32,
    font_known: bool,
    declarations: impl Iterator<Item = &'a Declaration>,
    variables: &custom_properties::Variables,
    budget: &mut custom_properties::Budget,
) -> Option<crate::image::SvgStyle> {
    if !font_known {
        return None;
    }
    let mut value = crate::image::SvgStyle::new(font as f64, root as f64);
    if let NodeKind::Element { attrs, .. } = &doc.nodes[node].kind {
        for attr in attrs {
            if attr.name.namespace.is_some()
                || !crate::image::SvgStyle::property(&attr.name.qualified)
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
        if !crate::image::SvgStyle::property(&d.name) {
            continue;
        }
        budget.work(d.value.len().checked_add(1)?).ok()?;
        let substituted;
        let text = if custom_properties::has_var(&d.value) {
            substituted = variables.substitute(&d.value, budget).ok()?;
            match substituted.as_deref() {
                Some(text) => match crate::image::SvgStyle::valid(tag, &d.name, text, outer) {
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
        match crate::image::SvgStyle::valid(tag, &d.name, text, outer) {
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

// SVG presentation font size is a zero-specificity author hint, before the
// matched stylesheet/inline cascade. Unitless attribute numbers are user units.
pub(super) fn font_hint(
    style: &mut Style,
    parent: &Style,
    text: &str,
    variables: &custom_properties::Variables,
    budget: &mut custom_properties::Budget,
    known: &mut bool,
    parent_known: bool,
    root_known: bool,
) {
    if text.len() > 2048
        || budget
            .copy(
                text.len()
                    .saturating_add(std::mem::size_of::<Declaration>())
                    .saturating_add(2),
            )
            .is_err()
    {
        *known = false;
        return;
    }
    let text = text.trim();
    let value = if text.parse::<f64>().is_ok_and(|n| n.is_finite()) {
        format!("{text}px")
    } else {
        text.to_owned()
    };
    let declaration = Declaration {
        name: "font-size".into(),
        value,
        important: false,
        stylesheet: None,
    };
    custom_properties::apply_font_size(
        style,
        parent,
        &declaration,
        variables,
        budget,
        known,
        parent_known,
        root_known,
    );
}
