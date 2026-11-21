//! Explicit paint ordering preserves ancestor context boundaries and native hits.
use super::*;
fn id(d: &Document, name: &str) -> usize {
    (0..d.nodes.len())
        .find(|&n| d.attr(n, "id") == Some(name))
        .unwrap()
}
fn measure(source: &str) -> (Document, Layout) {
    let d = crate::html::parse(&format!(
        "<html><head><style>html,body{{margin:0}}</style></head><body>{source}</body></html>"
    ));
    let p = build_viewport(&d, 300, 200, 1.0, &HashMap::new(), &HashMap::new());
    (d, p)
}
#[test]
fn stacking_explicit_sibling_levels_override_source_order_and_keep_native_target() {
    let(d,p)=measure("<a id=high href=high style='position:absolute;left:20px;top:20px;width:80px;height:40px;background:red;z-index:20'></a><a id=low href=low style='position:absolute;left:20px;top:20px;width:80px;height:40px;background:blue;z-index:2'></a>");
    assert_eq!(p.element_node_at(40, 30), Some(id(&d, "high")));
    assert_eq!(p.link_node_at(40, 30), Some(id(&d, "high")));
}
#[test]
fn stacking_auto_allows_descendant_escape_but_explicit_zero_isolates_it() {
    for (value, wanted) in [("auto", "inner"), ("0", "sibling")] {
        let(d,p)=measure(&format!("<div style='position:absolute;left:0;top:0;width:150px;height:80px;z-index:{value}'><a id=inner href=inner style='position:absolute;left:20px;top:20px;width:80px;height:40px;background:red;z-index:1000'></a></div><a id=sibling href=sibling style='position:absolute;left:20px;top:20px;width:80px;height:40px;background:blue;z-index:1'></a>"));
        assert_eq!(
            p.link_node_at(40, 30),
            Some(id(&d, wanted)),
            "parent {value}"
        );
    }
}
#[test]
fn stacking_negative_levels_stay_above_context_background_but_below_normal_flow() {
    let(d,p)=measure("<div style='position:relative;z-index:0;width:100px;height:60px;background:green'><a id=negative href=n style='position:absolute;left:20px;top:20px;width:60px;height:30px;background:red;z-index:-1'></a></div>");
    assert_eq!(p.link_node_at(40, 30), Some(id(&d, "negative")));
    let(d,p)=measure("<div id=flow style='margin-left:20px;margin-top:20px;width:80px;height:40px;background:blue'></div><a id=negative href=n style='position:absolute;left:20px;top:20px;width:80px;height:40px;background:red;z-index:-1'></a>");
    assert_eq!(p.element_node_at(40, 30), Some(id(&d, "flow")));
    assert_eq!(p.link_node_at(40, 30), None);
}
#[test]
fn stacking_large_exact_integer_levels_and_static_flex_item_eligibility_are_real() {
    let high = format!("1{}", "0".repeat(70));
    let low = "9".repeat(70);
    let(d,p)=measure(&format!("<a id=high href=h style='position:absolute;left:20px;top:20px;width:80px;height:40px;z-index:{high}'></a><a id=low href=l style='position:absolute;left:20px;top:20px;width:80px;height:40px;z-index:{low}'></a>"));
    assert_eq!(p.link_node_at(40, 30), Some(id(&d, "high")));
    let(d,p)=measure("<div style='display:flex;width:160px;height:40px'><a id=high href=h style='display:block;flex:none;width:80px;z-index:2'></a><a id=low href=l style='display:block;flex:none;width:80px;margin-left:-80px;z-index:1'></a></div>");
    assert_eq!(p.link_node_at(20, 20), Some(id(&d, "high")));
}

#[test]
fn stacking_transform_and_opacity_contexts_isolate_descendants_and_static_nonitems_ignore_levels() {
    for context in ["transform:translateX(0px)", "opacity:.5"] {
        let(d,p)=measure(&format!("<div style='position:absolute;left:0;top:0;width:150px;height:80px;{context}'><a id=inner href=i style='position:absolute;left:20px;top:20px;width:80px;height:40px;background:red;z-index:1000'></a></div><a id=sibling href=s style='position:absolute;left:20px;top:20px;width:80px;height:40px;background:blue;z-index:1'></a>"));
        assert_eq!(p.link_node_at(40, 30), Some(id(&d, "sibling")), "{context}");
        assert!(!p.opacity_unavailable);
    }
    let(d,p)=measure("<a id=first href=f style='display:block;width:80px;height:40px;background:red;z-index:99'></a><a id=second href=s style='display:block;width:80px;height:40px;margin-top:-40px;background:blue;z-index:1'></a>");
    assert_eq!(p.link_node_at(20, 20), Some(id(&d, "second")));
}
#[test]
fn stacking_order_does_not_change_source_reading_order_and_budget_refusal_recovers() {
    let(d,p)=measure("<a id=first href=f style='position:absolute;left:20px;top:20px;width:100px;height:40px;z-index:99'>First</a><a id=second href=s style='position:absolute;left:20px;top:20px;width:100px;height:40px;z-index:1'>Second</a>");
    assert_eq!(
        p.runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
        vec!["First", "Second"]
    );
    let (mut work, mut copies) = (0, 1_000_000);
    assert!(build_viewport_with_fonts_budgeted(
        &d,
        300,
        200,
        1.0,
        &HashMap::new(),
        &HashMap::new(),
        None,
        &Book::empty(),
        &mut css::CssBudget::new(&mut work, &mut copies)
    )
    .is_err());
    assert_eq!(work, 0);
    let (mut work, mut copies) = (1_000_000, 0);
    assert!(build_viewport_with_fonts_budgeted(
        &d,
        300,
        200,
        1.0,
        &HashMap::new(),
        &HashMap::new(),
        None,
        &Book::empty(),
        &mut css::CssBudget::new(&mut work, &mut copies)
    )
    .is_err());
    assert_eq!(copies, 0);
    let (mut work, mut copies) = (1_000_000, 4_000_000);
    let recovered = build_viewport_with_fonts_budgeted(
        &d,
        300,
        200,
        1.0,
        &HashMap::new(),
        &HashMap::new(),
        None,
        &Book::empty(),
        &mut css::CssBudget::new(&mut work, &mut copies),
    )
    .unwrap();
    assert_eq!(recovered.link_node_at(40, 30), Some(id(&d, "first")));
}
