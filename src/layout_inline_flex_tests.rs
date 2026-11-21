//! Inline outer formatting with genuine shrink-to-fit flex children.
use super::*;
fn node(d: &Document, id: &str) -> usize {
    (0..d.nodes.len())
        .find(|&n| d.attr(n, "id") == Some(id))
        .unwrap()
}
fn document() -> Document {
    crate::html::parse("<html><head><style>html,body{margin:0}#pre,#post{display:inline-block;width:20px;height:20px}#pre{background:#e03040}#post{background:#2040e0}#flex{display:inline-flex;gap:6px;background:#20a040}#a{width:40px;height:20px;flex:none}#b{width:30px;height:20px;flex:none}</style></head><body><div id=pre></div><div id=flex><a id=a href='#next'></a><div id=b></div></div><div id=post></div></body></html>")
}
#[test]
fn inline_flex_shrink_wraps_real_children_and_stays_on_its_outer_line() {
    let d = document();
    let p = build_viewport(&d, 300, 200, 1.0, &HashMap::new(), &HashMap::new());
    let rect = |id| p.border_rects[node(&d, id)].unwrap();
    let pre = rect("pre");
    let flex = rect("flex");
    let post = rect("post");
    let a = rect("a");
    let b = rect("b");
    assert_eq!((flex.width, flex.height), (76, 20));
    assert_eq!(flex.x, pre.x + pre.width);
    assert_eq!(post.x, flex.x + flex.width);
    assert_eq!((pre.y, post.y), (flex.y, flex.y));
    assert_eq!(b.x - a.x, 46);
    assert_eq!(p.link_node_at(a.x + 3, a.y + 3), Some(node(&d, "a")));
    let styles = css::compute(&d, &HashMap::new());
    assert_eq!(
        format!("{:?}", styles[node(&d, "flex")].display),
        "InlineFlex"
    );
}
#[test]
fn inline_flex_outer_wrap_and_zoom_preserve_real_flex_gaps() {
    let d = document();
    for (width, zoom) in [(80, 1.0), (160, 2.0)] {
        let p = build_viewport(&d, width, 300, zoom, &HashMap::new(), &HashMap::new());
        let rect = |id| p.border_rects[node(&d, id)].unwrap();
        let pre = rect("pre");
        let flex = rect("flex");
        let post = rect("post");
        assert!(flex.y >= pre.y + pre.height);
        assert!(post.y >= flex.y + flex.height);
        assert_eq!(flex.width, (76.0 * zoom) as i32);
        assert_eq!(rect("b").x - rect("a").x, (46.0 * zoom) as i32);
    }
}

#[test]
fn inline_flex_column_keeps_its_inner_flex_axis_and_computed_blockification() {
    let d=crate::html::parse("<style>html,body{margin:0}#flex{display:inline-flex;flex-direction:column;gap:6px}#a{width:40px;height:20px;flex:none}#b{width:30px;height:20px;flex:none}</style><div id=flex><div id=a></div><div id=b></div></div>");
    let index = node(&d, "flex");
    let p = build_viewport(&d, 300, 200, 1.0, &HashMap::new(), &HashMap::new());
    let a = p.border_rects[node(&d, "a")].unwrap();
    let b = p.border_rects[node(&d, "b")].unwrap();
    let outer = p.border_rects[index].unwrap();
    assert_eq!((outer.width, outer.height), (40, 46));
    assert_eq!(b.y - a.y, 26);
    for context in ["display:flex", "display:grid", "display:block"] {
        let absolute = if context == "display:block" {
            "position:absolute"
        } else {
            ""
        };
        let doc=crate::html::parse(&format!("<div style='{context}'><div id=target style='--display:inline-flex;display:var(--display);{absolute}'><div></div></div></div>"));
        let styles = css::compute(&doc, &HashMap::new());
        assert_eq!(
            format!("{:?}", styles[node(&doc, "target")].display),
            "Flex",
            "{context}"
        );
    }
}

#[test]
fn inline_flex_fixed_bases_contribute_to_auto_width_without_literal_child_widths() {
    let d=crate::html::parse("<style>html,body{margin:0}#flex{display:inline-flex;gap:6px}#a{flex:0 0 40px;height:20px}#b{flex:0 0 30px;height:20px}</style><div id=flex><div id=a></div><div id=b></div></div>");
    let p = build_viewport(&d, 300, 200, 1.0, &HashMap::new(), &HashMap::new());
    assert_eq!(p.border_rects[node(&d, "flex")].unwrap().width, 76);
}

#[test]
fn inline_flex_first_available_text_baseline_aligns_with_the_outer_line() {
    let d=crate::html::parse("<style>html,body{margin:0}#pre{display:inline-block;width:20px;height:20px}#flex{display:inline-flex}#a,#b{flex:none;width:40px;height:32px}#a{font-size:8px}#b{font-size:24px}</style><div id=pre></div><div id=flex><div id=a>a</div><div id=b>b</div></div>");
    let p = build_viewport(&d, 300, 200, 1.0, &HashMap::new(), &HashMap::new());
    let first = p.runs.iter().find(|r| r.text == "a").unwrap();
    let second = p.runs.iter().find(|r| r.text == "b").unwrap();
    let pre = p.border_rects[node(&d, "pre")].unwrap();
    assert_eq!(first.y + first.ascent(), pre.y + pre.height);
    assert_ne!(first.y + first.ascent(), second.y + second.ascent());
}
