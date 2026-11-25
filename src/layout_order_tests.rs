use super::*;

const BASE: &str = "html,body,div,a,span{margin:0;padding:0;border:0}body{font-size:16px;line-height:20px}#row{display:flex;width:160px;align-items:flex-start}#row>.item{flex:none;width:20px;height:20px}#a{order:2;background:#ff0000}#b{order:-1;background:#0000ff}#c{order:2;background:#00ff00}#d{order:0;background:#ffff00}";
const FOUR: &str = "<div id=row><a class=item id=a href=a></a><a class=item id=b href=b></a><a class=item id=c href=c></a><a class=item id=d href=d></a></div>";

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(name)).unwrap()
}
fn document(body: &str, rules: &str) -> Document {
    crate::html::parse(&format!("<!doctype html><html><head><style>{BASE}{rules}</style></head><body>{body}</body></html>"))
}
fn page(doc: &Document) -> Layout {
    build_viewport(doc, 320, 240, 1.0, &HashMap::new(), &HashMap::new())
}
fn cached(doc: &Document, cache: Option<&css::StylesheetCache>) -> Layout {
    build_viewport_with_fonts_cached(doc, 320, 240, 1.0, &HashMap::new(), &HashMap::new(), None, &Book::empty(), cache)
}
fn rect(doc: &Document, page: &Layout, name: &str) -> [i32; 4] {
    let r = page.border_rects[id(doc, name)].unwrap();
    [r.x, r.y, r.width, r.height]
}
fn positions(doc: &Document, page: &Layout, names: &[&str], vertical: bool, start: i32, step: i32) {
    for (index, name) in names.iter().enumerate() {
        let r = rect(doc, page, name);
        assert_eq!(r[usize::from(vertical)], start + step * index as i32, "{name}");
        assert_eq!((r[2], r[3]), (20, 20), "{name}");
        assert_eq!(page.link_node_at(r[0] + 3, r[1] + 3), Some(id(doc, name)), "{name} hit");
    }
}
fn set_attr(doc: &mut Document, node: usize, name: &str, value: &str) {
    let NodeKind::Element { attrs, .. } = &mut doc.nodes[node].kind else { panic!("element") };
    if let Some(attr) = attrs.iter_mut().find(|attr| attr.name.html_is(name)) {
        attr.value = value.to_owned().into();
    } else {
        attrs.push(crate::html::Attribute::unqualified(name.to_owned(), value.to_owned()));
    }
}
fn pixels(page: &Layout) -> Vec<u32> {
    let mut canvas = crate::paint::Canvas::new(320, 240);
    canvas.pixels.fill(0xffffff);
    let viewport = crate::paint::ClipRect::new(0, 0, 320, 240);
    let regions = vec![Some(viewport); page.opacity_groups.len()];
    let mut opacity = crate::paint::OpacityPainter::new(&canvas, &page.opacity_groups, &regions, viewport, page.items.len(), 0).unwrap();
    for (index, item) in page.items.iter().enumerate() {
        opacity.advance(&mut canvas, index);
        if opacity.hidden() { continue; }
        canvas.set_clip(page.paint_clip(index));
        match item {
            PaintItem::Rect { x, y, width, height, color, alpha, .. } => canvas.fill_rect_alpha(*x, *y, *width, *height, *color, *alpha),
            PaintItem::HitBox { .. } => {},
            _ => panic!("non-rectangle in blank order witness: {item:?}"),
        }
    }
    opacity.finish(&mut canvas);
    canvas.pixels
}

#[test]
fn order_row_sorts_signed_integers_stably_without_losing_large_values() {
    for rules in ["".to_owned(), format!("#a{{order:1{}}}#b{{order:-1{}}}#c{{order:1{}}}#d{{order:{}}}", "0".repeat(70), "0".repeat(70), "0".repeat(70), "9".repeat(70))] {
        let doc = document(FOUR, &rules);
        positions(&doc, &page(&doc), &["b", "d", "a", "c"], false, 0, 20);
    }
}

#[test]
fn order_row_reverse_column_and_column_reverse_keep_ordinal_groups() {
    for (direction, vertical, start, step) in [("row-reverse", false, 60, -20), ("column", true, 0, 20), ("column-reverse", true, 60, -20)] {
        let doc = document(FOUR, &format!("#row{{width:80px;height:80px;flex-direction:{direction}}}"));
        positions(&doc, &page(&doc), &["b", "d", "a", "c"], vertical, start, step);
    }
}

#[test]
fn order_precedes_wrapping_and_preserves_each_line_gap() {
    let doc = document(FOUR, "#row{width:45px;flex-wrap:wrap;gap:5px}");
    let p = page(&doc);
    for (name, x, y) in [("b", 0, 0), ("d", 25, 0), ("a", 0, 25), ("c", 25, 25)] {
        assert_eq!(rect(&doc, &p, name), [x, y, 20, 20]);
        assert_eq!(p.link_node_at(x + 3, y + 3), Some(id(&doc, name)));
    }
    assert_eq!(rect(&doc, &p, "row"), [0, 0, 45, 45]);
}

#[test]
fn order_inline_flex_keeps_outer_line_and_shrink_width() {
    let body = format!("<span id=before></span>{FOUR}<span id=after></span>");
    let doc = document(&body, "#before,#after{display:inline-block;width:10px;height:20px}#row{display:inline-flex;width:auto;gap:5px}");
    let p = page(&doc);
    positions(&doc, &p, &["b", "d", "a", "c"], false, 10, 25);
    assert_eq!(rect(&doc, &p, "row")[2], 95);
    assert_eq!(rect(&doc, &p, "after")[0], 105);
    assert_eq!(rect(&doc, &p, "before")[1], rect(&doc, &p, "after")[1]);
}

#[test]
fn order_anonymous_flex_text_stays_zero_and_retains_separate_items() {
    let doc = document("<div id=row>L<div id=front style='order:-1;flex:none;width:20px;height:20px'></div>R</div>", "#row{gap:10px;order:7}");
    let p = page(&doc);
    assert_eq!(rect(&doc, &p, "front")[0], 0);
    let left = p.runs.iter().find(|r| r.text == "L").unwrap();
    let right = p.runs.iter().find(|r| r.text == "R").unwrap();
    assert_eq!(left.x, 30);
    assert_eq!(right.x, left.x + left.width() + 10);
    assert_eq!(left.y, right.y);
    assert_eq!(p.runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(), ["L", "R"]);
}

#[test]
fn order_grid_auto_rows_follow_stable_groups() {
    let doc = document(FOUR, "#row{display:grid;width:80px;gap:5px}");
    let p = page(&doc);
    positions(&doc, &p, &["b", "d", "a", "c"], true, 0, 25);
    assert_eq!(rect(&doc, &p, "row"), [0, 0, 80, 95]);
}

#[test]
fn order_grid_preserves_anonymous_rows_separated_in_source() {
    let doc = document("<div id=row>L<div id=front style='order:-1;height:20px'></div>R</div>", "#row{display:grid;gap:0}");
    let p = page(&doc);
    assert_eq!(rect(&doc, &p, "front")[1], 0);
    let left = p.runs.iter().find(|r| r.text == "L").unwrap();
    let right = p.runs.iter().find(|r| r.text == "R").unwrap();
    assert!(left.y >= 20);
    assert_eq!(right.y - left.y, 20);
    assert_eq!(rect(&doc, &p, "row")[3], 60);
    assert_eq!(p.runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(), ["L", "R"]);
}

#[test]
fn order_does_not_reorder_normal_flow_or_inherit_by_default() {
    let doc = document(FOUR, "#row{display:block;order:-9}.item{display:block}");
    positions(&doc, &page(&doc), &["a", "b", "c", "d"], true, 0, 20);
    let doc = document("<div id=row style='order:-5'><a class=item id=one href=one></a><a class=item id=two href=two style='order:-1'></a></div>", "");
    positions(&doc, &page(&doc), &["two", "one"], false, 0, 20);
}

#[test]
fn order_paint_and_hits_follow_groups_with_relative_and_equal_z_items() {
    for extra in ["", "position:relative", "position:relative;z-index:0", "z-index:7"] {
        let doc = document("<div id=row><a id=front href=front></a><a id=back href=back></a></div>", &format!("#row{{width:80px}}#row>a{{flex:none;width:40px;height:40px;{extra}}}#front{{order:1;margin-left:-40px;background:#ff0000}}#back{{order:-1;background:#0000ff}}"));
        let p = page(&doc);
        assert_eq!(rect(&doc, &p, "front"), [0, 0, 40, 40], "{extra}");
        assert_eq!(rect(&doc, &p, "back"), [0, 0, 40, 40], "{extra}");
        assert_eq!(p.link_node_at(5, 5), Some(id(&doc, "front")), "{extra}");
        assert_eq!(pixels(&p)[5 * 320 + 5], 0xff0000, "{extra}");
    }
}

#[test]
fn order_transform_and_opacity_contexts_keep_paint_and_hits_consistent() {
    for (extra, expected) in [("transform:translateX(0px)", 0xff0000), ("opacity:.5", 0xc04080), ("opacity:.5;transform:translateX(0px)", 0xc04080)] {
        let doc = document("<div id=row><a id=front href=front></a><a id=back href=back></a></div>", &format!("#row{{width:80px}}#row>a{{flex:none;width:40px;height:40px;{extra}}}#front{{order:1;margin-left:-40px;background:#ff0000}}#back{{order:-1;background:#0000ff}}"));
        let p = page(&doc);
        assert!(!p.opacity_unavailable);
        assert_eq!(p.link_node_at(5, 5), Some(id(&doc, "front")), "{extra}");
        assert_eq!(pixels(&p)[5 * 320 + 5], expected, "{extra}");
    }
}

#[test]
fn order_absolute_and_fixed_children_have_effective_zero_for_painting() {
    for position in ["absolute", "fixed"] {
        for (floating_order, item_order, expected) in [("999", "1", "item"), ("-999", "-1", "floating")] {
            let doc = document("<div id=row><a id=item href=item></a><a id=floating href=floating></a></div>", &format!("#row{{position:relative;z-index:0;width:80px;height:40px}}#row>a{{width:40px;height:40px;flex:none;z-index:0}}#item{{order:{item_order};background:#ff0000}}#floating{{position:{position};left:0;top:0;order:{floating_order};background:#0000ff}}"));
            let p = page(&doc);
            assert_eq!(rect(&doc, &p, "item"), [0, 0, 40, 40]);
            assert_eq!(p.link_node_at(5, 5), Some(id(&doc, expected)), "{position} order {floating_order}");
            assert_eq!(pixels(&p)[5 * 320 + 5], if expected == "item" {0xff0000} else {0x0000ff});
        }
    }
}

#[test]
fn order_does_not_override_explicit_unequal_z_levels() {
    let doc = document("<div id=row><a id=front href=front></a><a id=back href=back></a></div>", "#row>a{flex:none;width:40px;height:40px}#front{order:1;z-index:0;margin-left:-40px;background:#ff0000}#back{order:-1;z-index:2;background:#0000ff}");
    let p = page(&doc);
    assert_eq!(p.link_node_at(5, 5), Some(id(&doc, "back")));
    assert_eq!(pixels(&p)[5 * 320 + 5], 0x0000ff);
}

#[test]
fn order_nested_containers_keep_independent_ordinal_groups() {
    let doc = document("<div id=outer><div id=left><a id=a href=a></a><a id=b href=b></a></div><div id=right><a id=c href=c></a><a id=d href=d></a></div></div>", "#outer,#left,#right{display:flex;align-items:flex-start}#outer{width:160px}#left,#right{flex:none;width:40px;height:20px}#left{order:1}#right{order:-1}#outer a{flex:none;width:20px;height:20px}#a,#c{order:1}#b,#d{order:-1}");
    positions(&doc, &page(&doc), &["d", "c", "b", "a"], false, 0, 20);
}

#[test]
fn order_style_and_custom_property_mutation_invalidate_warm_layout() {
    let mut doc = document(FOUR, "#row{--rank:2}#a{order:var(--rank)}");
    let first = cached(&doc, None);
    positions(&doc, &first, &["b", "d", "a", "c"], false, 0, 20);
    let row = id(&doc, "row");
    set_attr(&mut doc, row, "style", "--rank:-2");
    let second = cached(&doc, first.stylesheet_cache.as_ref());
    positions(&doc, &second, &["a", "b", "d", "c"], false, 0, 20);
    let a = id(&doc, "a");
    set_attr(&mut doc, a, "style", "order:3");
    let third = cached(&doc, second.stylesheet_cache.as_ref());
    positions(&doc, &third, &["b", "d", "c", "a"], false, 0, 20);
}

#[test]
fn order_ties_follow_current_dom_children_after_move_not_node_ids() {
    let mut doc = document(FOUR, "#a,#b,#c,#d{order:1}");
    let row = id(&doc, "row");
    let first = cached(&doc, None);
    positions(&doc, &first, &["a", "b", "c", "d"], false, 0, 20);
    doc.nodes[row].children.swap(0, 3);
    let children = doc.nodes[row].children.clone();
    let second = cached(&doc, first.stylesheet_cache.as_ref());
    positions(&doc, &second, &["d", "b", "c", "a"], false, 0, 20);
    assert_eq!(doc.nodes[row].children, children);
}

#[test]
fn order_preserves_source_reading_native_control_sequence_and_dom() {
    let doc = document("<div id=row><a class=item id=a href=a>A</a><a class=item id=b href=b>B</a><a class=item id=c href=c>C</a><a class=item id=d href=d>D</a></div>", "");
    let children = doc.nodes[id(&doc, "row")].children.clone();
    let p = page(&doc);
    positions(&doc, &p, &["b", "d", "a", "c"], false, 0, 20);
    assert_eq!(p.runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(), ["A", "B", "C", "D"]);
    assert_eq!(doc.nodes[id(&doc, "row")].children, children);
    let doc = document("<div id=row><input id=first value=first style='order:1'><input id=second value=second style='order:-1'></div>", "input{flex:none;width:60px;height:20px}");
    let p = page(&doc);
    assert!(rect(&doc, &p, "second")[0] < rect(&doc, &p, "first")[0]);
    assert_eq!(p.controls.iter().map(|control| control.node).collect::<Vec<_>>(), [id(&doc, "first"), id(&doc, "second")]);
}

#[test]
fn order_invalid_substitution_resets_to_zero_without_reactivating_earlier_value() {
    let doc = document(FOUR, "#a{order:-9;order:var(--missing)}#c{--bad:2px;order:var(--bad)}");
    positions(&doc, &page(&doc), &["b", "a", "c", "d"], false, 0, 20);
}

#[test]
fn order_budget_refusal_does_not_publish_a_partial_success_and_recovers() {
    let doc = document(FOUR, "#row{display:grid}");
    for (mut work, mut copies) in [(0, 4_000_000), (1_000_000, 0)] {
        assert!(build_viewport_with_fonts_budgeted(&doc, 320, 240, 1.0, &HashMap::new(), &HashMap::new(), None, &Book::empty(), &mut css::CssBudget::new(&mut work, &mut copies)).is_err());
    }
    let (mut work, mut copies) = (1_000_000, 4_000_000);
    let p = build_viewport_with_fonts_budgeted(&doc, 320, 240, 1.0, &HashMap::new(), &HashMap::new(), None, &Book::empty(), &mut css::CssBudget::new(&mut work, &mut copies)).unwrap();
    positions(&doc, &p, &["b", "d", "a", "c"], true, 0, 20);
}
