//! Shared paint cascade renders through the original SVG implementation.
use super::*;
fn render(css: &str, fragment: &str) -> Result<crate::image::Bitmap, String> {
    let doc = crate::html::parse(&format!(
        "<html><head><style>{css}</style></head><body>{fragment}</body></html>"
    ));
    let output = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    let root = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("icon"))
        .unwrap();
    let mut stack = vec![root];
    let mut context = Vec::new();
    while let Some(id) = stack.pop() {
        if matches!(doc.nodes[id].kind, NodeKind::Element { .. }) {
            context.push(
                output
                    .computed_properties
                    .read_svg_geometry(id)
                    .map_err(|_| "unavailable")?
                    .ok_or("absent")?
                    .clone(),
            );
        }
        stack.extend(doc.nodes[id].children.iter().rev().copied());
    }
    crate::image::decode_svg_context(fragment.as_bytes(), Some((20, 10)), &context, &|| Ok(()))
}
fn shape() -> &'static str {
    "<svg id='icon' width='20' height='10'><rect id='shape' width='20' height='10' fill='green'/></svg>"
}
#[test]
fn svg_shared_paint_inherits_html_values_and_resolves_current_color_at_final_element_with_alpha() {
    let image = render(
        "body{fill:#506070;color:#203040}#shape{fill:var(--missing)}",
        shape(),
    )
    .unwrap();
    assert_eq!(image.pixels[5 * 20 + 10], 0xff506070);
    let image=render("body{fill:currentColor;color:#203040}#icon{color:red}#shape{fill:inherit;color:rgba(255,0,0,.5)}",shape()).unwrap();
    assert_eq!(image.pixels[5 * 20 + 10], 0x80ff0000);
    let image = render(
        "body{fill:currentColor;color:#203040}#shape{color:blue;fill:initial}",
        shape(),
    )
    .unwrap();
    assert_eq!(image.pixels[5 * 20 + 10], 0xff000000);
}
#[test]
fn svg_shared_paint_priority_late_color_and_local_gradient_use_real_definitions() {
    let image=render("#shape{fill:currentColor!important;color:#2040c0}","<svg id='icon' width='20' height='10'><rect id='shape' width='20' height='10' style='fill:red' fill='green'/></svg>").unwrap();
    assert_eq!(image.pixels[5 * 20 + 10], 0xff2040c0);
    let fragment="<svg id='icon' width='20' height='10'><defs><linearGradient id='g'><stop offset='0' stop-color='red'/><stop offset='1' stop-color='blue'/></linearGradient></defs><rect id='shape' width='20' height='10'/></svg>";
    let image = render("body{--paint:url(#g)}#shape{fill:var(--paint)}", fragment).unwrap();
    let left = image.pixels[5 * 20 + 2];
    let right = image.pixels[5 * 20 + 17];
    assert!((left >> 16 & 255) > (left & 255));
    assert!((right & 255) > (right >> 16 & 255));
    let current_stops = fragment.replace("stop-color='red'", "stop-color='currentColor'");
    assert!(
        render("#g{color:lab(50% 0 0)}#shape{fill:url(#g)}", &current_stops)
            .unwrap_err()
            .contains("unavailable"),
        "gradient currentColor preserves unknown color rather than synthesizing transparent"
    );
}
#[test]
fn svg_shared_paint_unavailable_winners_refuse_and_later_values_recover() {
    assert!(
        render("#shape{fill:url(https://unrequested.invalid/p)}", shape())
            .unwrap_err()
            .contains("unavailable")
    );
    assert!(render("#shape{fill:context-fill}", shape())
        .unwrap_err()
        .contains("unavailable"));
    let image = render("#shape{fill:context-fill;fill:#a02040}", shape()).unwrap();
    assert_eq!(image.pixels[5 * 20 + 10], 0xffa02040);
    let image = render(
        "#icon{color:lab(50% 0 0)}#shape{fill:#a02040;stroke:none}",
        shape(),
    )
    .unwrap();
    assert_eq!(
        image.pixels[5 * 20 + 10],
        0xffa02040,
        "unavailable color is irrelevant to solid fill and absent stroke"
    );
}
#[test]
fn svg_shared_paint_copy_refusal_stays_unknown_and_recovery_uses_real_cascade() {
    let doc = crate::html::parse("<svg id=icon><rect id=shape fill=red></rect></svg>");
    let node = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("shape"))
        .unwrap();
    let parent = Style::default();
    let vars = custom_properties::Variables::default();
    let mut denied = custom_properties::Budget::default().operation(0, 0);
    let (paint, known) =
        svg_paint::compute(&doc, node, &parent, std::iter::empty(), &vars, &mut denied);
    assert!(paint.is_none() && !known);
    let mut available = custom_properties::Budget::default();
    let (paint, known) = svg_paint::compute(
        &doc,
        node,
        &parent,
        std::iter::empty(),
        &vars,
        &mut available,
    );
    assert!(paint.is_some() && known);
    let image = render("#shape{fill:#a02040}", shape()).unwrap();
    assert_eq!(image.pixels[5 * 20 + 10], 0xffa02040);
}

#[test]
fn svg_shared_paint_keeps_unrelated_html_and_inert_template_branches_without_paint_allocations() {
    let doc=crate::html::parse("<html><head><style>body{color:red;fill:blue}p{color:green}</style></head><body><p id=ordinary>text</p><template><svg><rect></rect></svg></template></body></html>");
    let output = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    assert!(output.styles.iter().all(|style| style.svg_paint.is_none()));
    let doc=crate::html::parse("<html><head><style>body{color:red;fill:blue}p{color:green}</style></head><body><p id=ordinary>text</p><svg id=icon width=20 height=10><rect width=20 height=10></rect></svg></body></html>");
    let output = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    let ordinary = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("ordinary"))
        .unwrap();
    let icon = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("icon"))
        .unwrap();
    assert!(
        std::sync::Arc::ptr_eq(
            output.styles[ordinary].svg_paint.as_ref().unwrap(),
            output.styles[icon].svg_paint.as_ref().unwrap()
        ),
        "unrelated HTML color does not allocate SVG paint state"
    );
}
