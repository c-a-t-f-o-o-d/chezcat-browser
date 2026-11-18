use super::*;

fn node(doc: &Document, id: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(id))
        .unwrap()
}

fn document(body: &str, css: &str) -> Document {
    crate::html::parse(&format!(
        "<html id=root><head><style>body{{margin:0}}{css}</style></head><body id=body>{body}</body></html>"
    ))
}

fn measure(doc: &Document) -> Layout {
    build_viewport(doc, 320, 200, 1.0, &HashMap::new(), &HashMap::new())
}

fn visual(page: &Layout, doc: &Document, id: &str) -> VisualBorderRect {
    page.visual_border_rects[node(doc, id)].expect(id)
}

fn rectangle(page: &Layout, color: u32) -> (i32, i32, i32, i32) {
    page.items
        .iter()
        .find_map(|item| match item {
            PaintItem::Rect {
                x,
                y,
                width,
                height,
                color: actual,
                ..
            } if *actual == color => Some((*x, *y, *width, *height)),
            _ => None,
        })
        .unwrap()
}

#[test]
fn layout_translate_fractional_nested_percentage_keeps_raw_boxes_and_flow_siblings() {
    let doc = document(
        "<div id=parent style='width:100px;height:40px;padding:3px;border:2px solid;transform:translate(10.25px,5.5px)'><div id=child style='width:20px;height:10px;background:red;transform:translate(50%,-25%)'></div></div><div id=sibling style='width:20px;height:10px;background:blue'></div>", "");
    let page = measure(&doc);
    let raw = page.border_rects[node(&doc, "child")].unwrap();
    assert_eq!((raw.x, raw.y), (5, 5));
    let parent = visual(&page, &doc, "parent");
    let child = visual(&page, &doc, "child");
    assert_eq!((parent.x, parent.y), (10.25, 5.5));
    assert_eq!((child.x, child.y), (25.25, 8.0));
    assert_eq!(
        page.transform_translations[node(&doc, "child")],
        Some([10.0, -2.5])
    );
    assert_eq!(rectangle(&page, 0xff0000), (25, 8, 20, 10));
    let sibling = page.border_rects[node(&doc, "sibling")].unwrap();
    assert_eq!((sibling.x, sibling.y), (0, 50));
    assert_eq!(
        (
            visual(&page, &doc, "sibling").x,
            visual(&page, &doc, "sibling").y
        ),
        (0.0, 50.0)
    );
    assert_eq!(rectangle(&page, 0x0000ff), (0, 50, 20, 10));
}

#[test]
fn layout_translate_paint_link_and_native_control_hits_follow_the_same_displacement() {
    let doc = document(
        "<div style='transform:translate(20px,30px)'><a id=link href=one style='display:block;width:50px;height:20px;background:red'>Go</a><input id=control type=button value=Push style='width:60px;height:20px'></div>", "");
    let page = measure(&doc);
    assert_eq!(rectangle(&page, 0xff0000), (20, 30, 50, 20));
    assert_eq!(page.link_node_at(22, 40), Some(node(&doc, "link")));
    assert_eq!(page.link_node_at(2, 10), None);
    let control = page
        .controls
        .iter()
        .find(|control| control.node == node(&doc, "control"))
        .unwrap();
    let raw = page.border_rects[control.node].unwrap();
    assert_eq!((control.x, control.y), (raw.x + 20, raw.y + 30));
    assert_eq!(
        page.control_at(control.x + 2, control.y + 2),
        Some(control.node)
    );
    assert_eq!(page.control_at(raw.x + 2, raw.y + 2), None);
}

#[test]
fn layout_translate_identity_establishes_absolute_and_fixed_padding_containing_blocks() {
    let doc = document(
        "<div id=parent style='margin:40px 50px;width:100px;height:80px;border:4px solid;padding:6px;transform:translate(0)'><div id=absolute style='position:absolute;left:7px;top:9px;width:10px;height:10px;background:red'></div><div id=captured style='position:fixed;left:11px;top:13px;width:10px;height:10px;background:blue'></div></div><div id=viewport style='position:fixed;left:3px;top:5px;width:10px;height:10px;background:yellow'></div><div style='height:800px'></div>", "");
    let mut page = measure(&doc);
    let parent = node(&doc, "parent");
    for id in ["absolute", "captured"] {
        assert_eq!(
            page.offset_parents[node(&doc, id)],
            OffsetParent::Element(parent)
        );
    }
    let absolute = page.border_rects[node(&doc, "absolute")].unwrap();
    let captured = page.border_rects[node(&doc, "captured")].unwrap();
    assert_eq!((absolute.x, absolute.y), (61, 53));
    assert_eq!((captured.x, captured.y), (65, 57));
    assert!(!captured.fixed);
    assert!(!visual(&page, &doc, "captured").fixed);
    assert_eq!(
        page.offset_parents[node(&doc, "viewport")],
        OffsetParent::None
    );
    assert!(visual(&page, &doc, "viewport").fixed);
    page.set_scroll(100);
    assert_eq!(rectangle(&page, 0x0000ff), (65, 57, 10, 10));
    assert_eq!(rectangle(&page, 0xffff00), (3, 105, 10, 10));
}

#[test]
fn layout_translate_ancestor_clip_stays_put_and_transformed_fixed_uses_its_actual_clip_parent() {
    let doc = document(
        "<div style='width:80px;height:40px;overflow:clip'><a id=link href=one style='display:block;width:50px;height:20px;transform:translate(60px,10px);background:red'>Go</a></div>", "");
    let page = measure(&doc);
    assert_eq!(page.link_node_at(70, 15), Some(node(&doc, "link")));
    assert_eq!(page.link_node_at(90, 15), None);
    let doc = document(
        "<div id=parent style='width:80px;height:40px;overflow:clip;transform:translate(10px,20px)'><div style='width:10px;height:10px;overflow:hidden'><a id=link href=one style='position:fixed;left:30px;top:5px;display:block;width:40px;height:10px;background:red'>Go</a></div></div>", "");
    let page = measure(&doc);
    assert_eq!(rectangle(&page, 0xff0000), (40, 25, 40, 10));
    assert_eq!(page.link_node_at(45, 27), Some(node(&doc, "link")));
    assert_eq!(page.link_node_at(95, 27), None);
}

#[test]
fn layout_translate_nested_contexts_are_atomic_and_dom_order_beats_arena_identity() {
    let mut doc = document(
        "<a id=later href=later style='position:absolute;left:0;top:0;display:block;width:20px;height:20px;background:blue'>B</a><div id=context style='width:20px;height:20px;transform:translate(0)'><a id=inside href=inside style='position:absolute;left:0;top:0;display:block;width:20px;height:20px;background:red'>A</a></div>", "");
    let body = node(&doc, "body");
    let context = node(&doc, "context");
    let later = node(&doc, "later");
    doc.nodes[body].children = vec![context, later];
    let page = measure(&doc);
    let colors: Vec<_> = page
        .items
        .iter()
        .filter_map(|item| match item {
            PaintItem::Rect { color, .. } if matches!(*color, 0xff0000 | 0x0000ff) => Some(*color),
            _ => None,
        })
        .collect();
    assert_eq!(colors, vec![0xff0000, 0x0000ff]);
    assert_eq!(page.link_node_at(5, 5), Some(later));
}

#[test]
fn layout_translate_inline_noop_and_unsupported_transform_fallback_remain_honest() {
    let doc = document(
        "<span style='transform:translate(40px)'><input id=control type=button value=Push></span>",
        "",
    );
    let page = measure(&doc);
    assert!(page.visual_border_rects.is_empty());
    assert_eq!(
        page.controls[0].x,
        page.border_rects[node(&doc, "control")].unwrap().x
    );
    let doc = document(
        "<a id=unknown href=one style='display:block;width:40px;height:20px;background:red;transform:rotate(5deg)'>Go</a>", "");
    let page = measure(&doc);
    assert!(page.visual_unavailable[node(&doc, "unknown")]);
    assert!(page.visual_border_rects[node(&doc, "unknown")].is_none());
    assert_eq!(rectangle(&page, 0xff0000), (0, 0, 40, 20));
    assert_eq!(page.link_node_at(5, 10), Some(node(&doc, "unknown")));
}

#[test]
fn layout_translate_blockified_inline_positioned_and_flex_items_are_transformable() {
    let doc = document(
        "<span id=positioned style='position:absolute;left:10px;top:20px;width:20px;height:10px;background:red;transform:translate(5px,7px)'></span><div style='display:flex'><span id=flex-item style='width:20px;height:10px;background:blue;transform:translate(30px,40px)'>X</span></div>", "");
    let page = measure(&doc);
    assert_eq!(rectangle(&page, 0xff0000), (15, 27, 20, 10));
    let flex = page.border_rects[node(&doc, "flex-item")].unwrap();
    assert_eq!(
        (
            visual(&page, &doc, "flex-item").x,
            visual(&page, &doc, "flex-item").y
        ),
        (f64::from(flex.x + 30), f64::from(flex.y + 40))
    );
}

#[test]
fn layout_translate_root_body_geometry_moves_while_propagated_canvas_background_stays() {
    let doc = document(
        "<div id=child style='width:20px;height:20px;background:red'></div>",
        "#root{background:#123456;transform:translateX(10px)}#body{transform:translateX(20px)}",
    );
    let page = measure(&doc);
    assert_eq!(rectangle(&page, 0x123456), (0, 0, 320, MAX_HEIGHT));
    assert_eq!(
        (visual(&page, &doc, "root").x, visual(&page, &doc, "body").x),
        (10.0, 30.0)
    );
    assert_eq!(visual(&page, &doc, "child").x, 30.0);
    assert_eq!(page.border_rects[node(&doc, "child")].unwrap().x, 0);
    assert_eq!(rectangle(&page, 0xff0000), (30, 0, 20, 20));
}

#[test]
fn layout_translate_budget_only_adds_cost_for_transform_sources_and_active_geometry() {
    let doc = document("<div style='width:20px;height:10px'></div>", "");
    let (mut work, mut copies) = (0, 0);
    let plain = build_viewport_with_fonts_budgeted(
        &doc,
        320,
        200,
        1.0,
        &HashMap::new(),
        &HashMap::new(),
        None,
        &Book::empty(),
        &mut css::CssBudget::new(&mut work, &mut copies),
    )
    .unwrap();
    assert!(plain.visual_border_rects.is_empty());
    assert_eq!((work, copies), (0, 0));
    let doc = document(
        "<div id=target style='width:20px;height:10px;transform:translate(5px,7px)'></div>",
        "",
    );
    let (mut work, mut copies) = (0, 1_000_000);
    assert!(build_viewport_with_fonts_budgeted(
        &doc,
        320,
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
    let (mut work, mut copies) = (1_000_000, 1_000_000);
    let page = build_viewport_with_fonts_budgeted(
        &doc,
        320,
        200,
        1.0,
        &HashMap::new(),
        &HashMap::new(),
        None,
        &Book::empty(),
        &mut css::CssBudget::new(&mut work, &mut copies),
    )
    .unwrap();
    assert_eq!(
        (
            visual(&page, &doc, "target").x,
            visual(&page, &doc, "target").y
        ),
        (5.0, 7.0)
    );
    assert!(work < 1_000_000 && copies < 1_000_000);
}

#[test]
fn layout_translate_unboxed_reference_and_capped_translation_flag_visual_fallback_without_cooking_raw_boxes(
) {
    let body = "<table><tbody><tr id=unboxed style='transform:translateY(50%)'><td><div id=row-child style='width:20px;height:10px'></div></td></tr></tbody></table><div id=capped style='height:10000px;padding:10px;transform:translateY(10000%)'><div id=child style='width:20px;height:10px;background:red'></div></div><div id=neighbor style='width:20px;height:10px;background:blue'></div>";
    let doc = document(body, "");
    let page = measure(&doc);
    let baseline_doc = document(
        &body
            .replace("transform:translateY(50%)", "")
            .replace("transform:translateY(10000%)", ""),
        "",
    );
    let baseline = measure(&baseline_doc);
    assert!(page.client_boxes[node(&doc, "unboxed")].is_some());
    assert!(page.border_rects[node(&doc, "unboxed")].is_none());
    for id in ["unboxed", "row-child", "capped", "child"] {
        assert!(page.visual_unavailable[node(&doc, id)], "{id}");
        assert!(page.visual_border_rects[node(&doc, id)].is_none(), "{id}");
    }
    assert!(page.transform_translations[node(&doc, "capped")].is_none());
    for id in ["row-child", "capped", "child", "neighbor"] {
        let actual = page.border_rects[node(&doc, id)].unwrap();
        let expected = baseline.border_rects[node(&baseline_doc, id)].unwrap();
        assert_eq!(
            (
                actual.x,
                actual.y,
                actual.width,
                actual.height,
                actual.fixed
            ),
            (
                expected.x,
                expected.y,
                expected.width,
                expected.height,
                expected.fixed
            ),
            "{id}"
        );
    }
    assert!(!page.visual_unavailable[node(&doc, "neighbor")]);
    assert_eq!(rectangle(&page, 0xff0000), rectangle(&baseline, 0xff0000));
}
