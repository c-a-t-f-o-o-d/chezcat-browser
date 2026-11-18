use super::*;

fn node(doc: &Document, id: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(id))
        .unwrap()
}

fn measure(body: &str, css: &str, zoom: f32) -> (Document, Layout) {
    let doc = crate::html::parse(&format!(
        "<html id=root><head><style>body{{margin:0}}{css}</style></head><body id=body>{body}</body></html>"
    ));
    let page = build_viewport(&doc, 640, 480, zoom, &HashMap::new(), &HashMap::new());
    (doc, page)
}

fn expect(doc: &Document, page: &Layout, id: &str, parent: Option<&str>) {
    let expected = parent.map_or(OffsetParent::None, |id| {
        OffsetParent::Element(node(doc, id))
    });
    assert_eq!(page.offset_parents[node(doc, id)], expected, "{id}");
}

#[test]
fn layout_offset_parent_uses_nearest_positioned_box_and_real_body_root() {
    let (doc, page) = measure(
        "<div id=plain></div><div id=relative style='position:relative'><div id=static></div><div id=absolute style='position:absolute'></div><div id=fixed style='position:fixed'></div></div><div id=fixed-parent style='position:fixed'><div id=in-fixed></div></div><div id=sticky style='position:sticky'><div id=in-sticky></div></div>",
        "#root,#body{position:relative}",
        1.0,
    );
    for id in ["root", "body", "fixed", "fixed-parent"] {
        expect(&doc, &page, id, None);
    }
    for id in ["plain", "relative", "sticky"] {
        expect(&doc, &page, id, Some("body"));
    }
    for id in ["static", "absolute"] {
        expect(&doc, &page, id, Some("relative"));
    }
    expect(&doc, &page, "in-fixed", Some("fixed-parent"));
    expect(&doc, &page, "in-sticky", Some("sticky"));
}

#[test]
fn layout_offset_parent_static_table_exception_is_html_identity_not_display() {
    let (doc, page) = measure(
        "<table id=table><tbody><tr><td id=cell><div id=in-cell></div><div id=relative style='position:relative'></div><div id=absolute style='position:absolute'></div></td><th id=heading><div id=in-heading></div></th></tr></tbody></table><div id=css-table style='display:table'><div style='display:table-row'><div id=css-cell style='display:table-cell'><div id=in-css-cell></div></div></div></div>",
        "",
        1.0,
    );
    expect(&doc, &page, "in-cell", Some("cell"));
    expect(&doc, &page, "in-heading", Some("heading"));
    expect(&doc, &page, "cell", Some("table"));
    expect(&doc, &page, "heading", Some("table"));
    for id in ["relative", "absolute", "in-css-cell"] {
        expect(&doc, &page, id, Some("body"));
    }
}

#[test]
fn layout_offset_parent_hidden_and_zero_sized_boxes_have_distinct_evidence() {
    let (doc, page) = measure(
        "<div id=hidden style='display:none'><div id=hidden-child></div></div><input id=hidden-input type=hidden><div id=zero style='width:0;height:0'></div><span id=inline>text</span>",
        "",
        1.0,
    );
    for id in ["hidden", "hidden-child", "hidden-input"] {
        expect(&doc, &page, id, None);
    }
    assert!(page.client_boxes[node(&doc, "zero")].is_some());
    expect(&doc, &page, "zero", Some("body"));
    assert!(page.client_boxes[node(&doc, "inline")].is_none());
    assert!(page.runs.iter().any(|line| line.text == "text"));
    expect(&doc, &page, "inline", Some("body"));
}

#[test]
fn layout_offset_parent_positioned_inline_only_requires_an_associated_box() {
    let (doc, page) = measure(
        "<span id=inline style='position:relative'><button id=button>Choose</button></span><div id=neighbor></div>",
        "",
        1.0,
    );
    assert!(page.client_boxes[node(&doc, "button")].is_some());
    assert!(page.client_boxes[node(&doc, "inline")].is_none());
    expect(&doc, &page, "inline", Some("body"));
    expect(&doc, &page, "button", Some("inline"));
    expect(&doc, &page, "neighbor", Some("body"));
}

#[test]
fn layout_offset_parent_truncated_layout_does_not_fabricate_null_or_body() {
    let body = format!(
        "<div id=before></div>{}<div id=after></div>",
        "<div style='height:1000px'></div>".repeat((MAX_HEIGHT / 1000 + 2) as usize)
    );
    let (doc, page) = measure(&body, "", 1.0);
    expect(&doc, &page, "before", Some("body"));
    assert!(page.client_boxes[node(&doc, "after")].is_none());
    assert_eq!(
        page.offset_parents[node(&doc, "after")],
        OffsetParent::Unavailable
    );
}

#[test]
fn layout_offset_parent_refused_position_is_local_and_never_a_static_fallback() {
    let huge = "x".repeat(2048);
    let (doc, page) = measure(
        "<div id=unknown><div id=inside></div><div id=fresh style='position:relative'><div id=in-fresh></div></div></div><div id=known style='position:relative'><div id=neighbor></div></div>",
        &format!("#unknown{{--huge:{huge};position:var(--huge)}}"),
        1.0,
    );
    for id in ["unknown", "inside"] {
        assert_eq!(
            page.offset_parents[node(&doc, id)],
            OffsetParent::Unavailable,
            "{id}"
        );
    }
    expect(&doc, &page, "neighbor", Some("known"));
    expect(&doc, &page, "in-fresh", Some("fresh"));
}

#[test]
fn layout_offset_parent_identity_is_unchanged_by_uniform_browser_zoom() {
    for zoom in [0.5, 1.0, 1.5, 2.0] {
        let (doc, page) = measure(
            "<div id=outer style='position:relative'><div id=inner><button id=button>Choose</button></div></div><div id=plain></div>",
            "",
            zoom,
        );
        expect(&doc, &page, "button", Some("outer"));
        expect(&doc, &page, "plain", Some("body"));
    }
}
