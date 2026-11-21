use super::*;
use crate::paint::Canvas;

fn page(markup: &str, rules: &str, width: i32, zoom: f32) -> (Document, Layout) {
    let doc = crate::html::parse(&format!(
        "<html><head><style>html,body{{margin:0;padding:0}}{rules}</style></head><body>{markup}</body></html>"
    ));
    let layout = build_viewport(&doc, width, 480, zoom, &HashMap::new(), &HashMap::new());
    (doc, layout)
}

fn node(doc: &Document, id: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&index| doc.attr(index, "id") == Some(id))
        .expect(id)
}

fn rect(doc: &Document, layout: &Layout, id: &str) -> [i32; 4] {
    let r = layout.border_rects[node(doc, id)].expect(id);
    [r.x, r.y, r.width, r.height]
}

#[test]
fn implicit_grid_action_rows_match_literal_and_flex_geometry_and_solid_paint() {
    let markup = "<div id=grid class=stack><a id=g1 href='#a'>One</a> <a id=g2 href='#b'>Two</a></div>
                  <div id=literal class=stack><a id=l1 href='#c'>One</a> <a id=l2 href='#d'>Two</a></div>
                  <div id=flex class=stack><a id=f1 href='#e'>One</a> <a id=f2 href='#f'>Two</a></div>";
    for viewport in [640, 1280] {
        for gap in [8, 16] {
            let rules = format!(
                ".stack{{position:absolute;top:12px;width:160px;background:#ffffff}}
                 .stack a{{display:inline-block;height:40px;background:#2468ac}}
                 #grid{{left:8px;display:grid;gap:{gap}px 31px}}
                 #literal{{left:200px}}#literal a{{display:block;width:160px}}#l2{{margin-top:{gap}px}}
                 #flex{{left:392px;display:flex;flex-direction:column;gap:{gap}px}}"
            );
            let (doc, layout) = page(markup, &rules, viewport, 1.0);
            for (container, first, second, x) in [
                ("grid", "g1", "g2", 8),
                ("literal", "l1", "l2", 200),
                ("flex", "f1", "f2", 392),
            ] {
                assert_eq!(
                    rect(&doc, &layout, container),
                    [x, 12, 160, 80 + gap],
                    "{container}, {viewport}, {gap}"
                );
                assert_eq!(rect(&doc, &layout, first), [x, 12, 160, 40], "{first}");
                assert_eq!(
                    rect(&doc, &layout, second),
                    [x, 52 + gap, 160, 40],
                    "{second}"
                );
            }

            // The samples sit at the far right of the link, away from glyphs.
            // Rasterize the layout's actual solid paint commands in paint order.
            let mut canvas = Canvas::new(viewport as usize, 128);
            canvas.fill_rect(0, 0, viewport, 128, 0xffffff);
            for (index, item) in layout.items.iter().enumerate() {
                if let PaintItem::Rect {
                    x,
                    y,
                    width,
                    height,
                    color,
                    alpha, .. } = item
                {
                    canvas.set_clip(layout.paint_clip(index));
                    canvas.fill_rect_alpha(*x, *y, *width, *height, *color, *alpha);
                }
            }
            for x in [167, 359, 551] {
                for y in 11..=92 + gap {
                    let painted = (12..52).contains(&y) || (52 + gap..92 + gap).contains(&y);
                    let expected = if painted { 0x2468ac } else { 0xffffff };
                    assert_eq!(
                        canvas.pixels[y as usize * canvas.width + x],
                        expected,
                        "{viewport}, {gap}, ({x},{y})"
                    );
                }
            }
        }
    }
}

#[test]
fn implicit_grid_item_box_sizing_margins_and_excluded_slots_scale_with_zoom() {
    let markup = "<div id=grid><div id=stretch></div><div id=hidden></div>
                  <div id=overlay></div><div id=content></div><div id=boxed></div></div>
                  <div id=after></div>";
    let rules = "#grid{position:relative;display:grid;width:120px;padding:4px 8px;border:2px solid black;row-gap:5px}
                 #stretch,#content,#boxed{padding:2px 3px;border:1px solid black}
                 #stretch{height:10px;margin:3px 7px 4px 5px}
                 #content{width:40px;height:8px;margin:2px 0;box-sizing:content-box}
                 #boxed{width:40px;height:14px;box-sizing:border-box}
                 #hidden{display:none;height:100px}
                 #overlay{position:absolute;left:2px;top:2px;width:3px;height:7px}
                 #after{width:1px;height:1px}";
    for scale in [1, 2] {
        let (doc, layout) = page(markup, rules, 640, scale as f32);
        for (id, expected) in [
            ("grid", [0, 0, 140, 77]),
            ("stretch", [15, 9, 108, 16]),
            ("content", [10, 36, 48, 14]),
            ("boxed", [10, 57, 40, 14]),
            ("after", [0, 77, 1, 1]),
        ] {
            assert_eq!(
                rect(&doc, &layout, id),
                expected.map(|n| n * scale),
                "{id}, zoom {scale}"
            );
        }
        assert!(layout.border_rects[node(&doc, "hidden")].is_none());
        let overlay = rect(&doc, &layout, "overlay");
        assert_eq!([overlay[2], overlay[3]], [3 * scale, 7 * scale]);
    }
}

#[test]
fn implicit_grid_empty_single_and_ordinary_flow_keep_their_own_spacing() {
    let markup = "<div id=empty><div style='display:none'></div></div>
                  <div id=single><div id=only></div></div>
                  <div id=ordinary><span id=left></span><span id=right></span></div>";
    let rules = "#empty,#single{display:grid;width:120px;gap:24px;padding:2px 0}
                 #only{height:9px}#ordinary{width:120px;gap:24px}
                 #ordinary span{display:inline-block;width:10px;height:9px}";
    let (doc, layout) = page(markup, rules, 640, 1.0);
    assert_eq!(rect(&doc, &layout, "empty"), [0, 0, 120, 4]);
    assert_eq!(rect(&doc, &layout, "single"), [0, 4, 120, 13]);
    assert_eq!(rect(&doc, &layout, "only"), [0, 6, 120, 9]);
    let a = rect(&doc, &layout, "left");
    let b = rect(&doc, &layout, "right");
    assert_eq!([a[0], b[0]], [0, 10]);
    assert_eq!(a[1], b[1]);
}

#[test]
fn implicit_grid_adjacent_text_nodes_share_one_anonymous_row() {
    let mut doc = crate::html::parse(
        "<style>body{margin:0}#grid{display:grid;width:120px;row-gap:6px;line-height:24px}
         #middle{height:12px}</style><body><div id=grid>One <div id=middle></div>Last</div></body>",
    );
    // The tokenizer coalesces neighboring text. Model the separate CharacterData
    // nodes that DOM insertions can produce, with an intervening real Comment.
    let grid = node(&doc, "grid");
    let comment = doc.nodes.len();
    for kind in [
        NodeKind::Comment("between".into()),
        NodeKind::Text("Two".into()),
    ] {
        doc.nodes.push(crate::html::Node {
            kind,
            namespace: crate::html::Namespace::None,
            children: Vec::new(),
        });
    }
    doc.nodes[grid]
        .children
        .splice(1..1, [comment, comment + 1]);
    let layout = layout(&doc, 640);
    let run = |word: &str| layout.runs.iter().find(|r| r.text == word).expect(word);
    let (one, two, last) = (run("One"), run("Two"), run("Last"));
    let middle = rect(&doc, &layout, "middle");
    assert_eq!(one.y, two.y);
    assert!(two.x > one.x);
    assert!(middle[1] > one.y);
    assert_eq!([middle[0], middle[2], middle[3]], [0, 120, 12]);
    assert!(last.y >= middle[1] + middle[3] + 6);
}

#[test]
fn implicit_grid_definite_height_stretches_tracks_and_only_auto_height_items() {
    let markup = "<div id=auto class=grid><div id=a></div><div id=b></div></div>
                  <div id=fixed class=grid><div id=c></div><div id=d></div></div>
                  <div id=outer><div id=nested class=grid><div id=e></div><div id=f></div></div></div>";
    let rules = ".grid{display:grid;width:90px;row-gap:10px}
                 #auto,#fixed,#outer{height:100px}#fixed div{height:20px}
                 #outer{display:flex;align-items:stretch}";
    let (doc, layout) = page(markup, rules, 640, 1.0);
    for (container, first, second, top, item_height) in [
        ("auto", "a", "b", 0, 45),
        ("fixed", "c", "d", 100, 20),
        ("nested", "e", "f", 200, 45),
    ] {
        assert_eq!(
            rect(&doc, &layout, container),
            [0, top, 90, 100],
            "{container}"
        );
        assert_eq!(
            rect(&doc, &layout, first),
            [0, top, 90, item_height],
            "{first}"
        );
        assert_eq!(
            rect(&doc, &layout, second),
            [0, top + 55, 90, item_height],
            "{second}"
        );
    }
}

#[test]
fn implicit_grid_scrollable_auto_minimum_does_not_widen_its_column() {
    let markup =
        "<div id=grid><div id=wrapper><div id=wide></div></div><div id=sibling></div></div>";
    for (overflow, column) in [
        ("hidden", 100),
        ("auto", 100),
        ("scroll", 100),
        ("visible", 200),
        ("clip", 200),
    ] {
        let rules = format!("#grid{{display:grid;width:100px}}#wrapper{{overflow:{overflow}}}#wide{{min-width:200px;height:10px}}#sibling{{height:4px}}");
        let (doc, layout) = page(markup, &rules, 640, 1.0);
        assert_eq!(rect(&doc, &layout, "wrapper")[2], column, "{overflow}");
        assert_eq!(rect(&doc, &layout, "sibling")[2], column, "{overflow}");
        assert_eq!(rect(&doc, &layout, "wide")[2], 200);
    }
    let (doc, layout) = page(markup, "#grid{display:grid;width:100px}#wrapper{min-width:120px;overflow:hidden}#wide{min-width:200px;height:10px}", 640, 1.0);
    assert_eq!(rect(&doc, &layout, "sibling")[2], 120);
}

#[test]
fn implicit_grid_budget_charges_layout_work_and_storage_on_success_and_refusal() {
    fn build(doc: &Document, work: &mut usize, copies: &mut usize) -> Result<Layout, String> {
        build_viewport_with_fonts_budgeted(
            doc,
            640,
            480,
            1.0,
            &HashMap::new(),
            &HashMap::new(),
            None,
            &Book::empty(),
            &mut css::CssBudget::new(work, copies),
        )
    }
    let doc = |unrelated: &str| {
        crate::html::parse(&format!(
        "<body style='margin:0'><div style='display:grid;width:100px'><div id=row style='height:4px'></div></div><p>{unrelated}</p></body>"
    ))
    };
    let allowance = 500_000;
    let small = doc("word");
    let large = doc(&"word ".repeat(820));
    let mut used = Vec::new();
    for doc in [&small, &large] {
        let (mut work, mut copies) = (allowance, allowance);
        let layout = build(doc, &mut work, &mut copies).unwrap();
        assert_eq!(rect(doc, &layout, "row"), [0, 0, 100, 4]);
        used.push((allowance - work, allowance - copies));
    }
    assert_eq!(
        used[0], used[1],
        "unrelated text is not an intrinsic grid measurement"
    );
    let (units, bytes) = used[0];
    assert!(units > 1 && bytes >= std::mem::size_of::<grid::GridRow>());
    let (mut work, mut copies) = (units, bytes);
    assert!(build(&small, &mut work, &mut copies).is_ok());
    assert_eq!((work, copies), (0, 0));
    for (mut work, mut copies) in [(units - 1, bytes), (units, bytes - 1)] {
        let former = (work, copies);
        assert!(build(&small, &mut work, &mut copies).is_err());
        if former.0 == units - 1 {
            assert_eq!(
                (work, copies),
                (0, 0),
                "row storage was allocated before positioning failed"
            );
        } else {
            assert_eq!(copies, 0);
            assert!(
                work < former.0,
                "the column was inspected before row storage was refused"
            );
        }
    }
}

#[test]
fn implicit_grid_incomplete_tracks_withhold_descendant_metrics_and_keep_outside_sibling() {
    let markup = format!(
        "<div id=grid><div id=first><div id=nested></div></div>{}</div><div id=outside></div>",
        "<div></div>".repeat(MAX_FLEX_ITEMS)
    );
    let (doc, layout) = page(
        &markup,
        "#grid{display:grid;height:4098px;width:100px}#outside{width:9px;height:7px}",
        640,
        1.0,
    );
    let mut geometry = crate::script_geometry::Geometry::new(
        640,
        480,
        1.0,
        &HashMap::new(),
        HashMap::new(),
        &Book::empty(),
    );
    geometry.sync(640, 480, 1.0, HashMap::new(), &layout);
    let forms = forms::Forms::new(&doc);
    let (mut work, mut copies) = (1_000_000, 1_000_000);
    for name in ["grid", "first", "nested"] {
        let n = node(&doc, name);
        assert!(layout.border_rects[n].is_none() && layout.client_boxes[n].is_none());
        assert!(layout.offset_unavailable[n] && layout.scroll_heights[n].is_none());
        assert!(geometry
            .bounding_rect(&doc, &forms, n, &mut work, &mut copies)
            .is_err());
        for getter in ["clientWidth", "offsetHeight", "scrollHeight"] {
            assert!(
                geometry
                    .read(&doc, &forms, n, false, getter, &mut work, &mut copies)
                    .is_err(),
                "{name} {getter}"
            );
        }
    }
    assert!(layout.viewport_scroll_unavailable);
    assert_eq!(rect(&doc, &layout, "outside"), [0, 4098, 9, 7]);
    assert_eq!(
        geometry
            .read(
                &doc,
                &forms,
                node(&doc, "outside"),
                false,
                "offsetWidth",
                &mut work,
                &mut copies
            )
            .unwrap(),
        9.0
    );
}

#[test]
fn implicit_grid_auto_percentage_gap_overflows_and_retains_bottom_padding_in_scroll_height() {
    let (doc, layout) = page("<div id=grid><div id=first></div><div id=last></div></div><div id=after></div>",
        "#grid{display:grid;width:100px;row-gap:25%;padding:3px 0 7px;border:2px solid black;overflow:hidden}#grid div{height:20px}#after{height:1px}",640,1.0);
    assert_eq!(rect(&doc, &layout, "grid"), [0, 0, 104, 54]);
    assert_eq!(rect(&doc, &layout, "first"), [2, 5, 100, 20]);
    assert_eq!(rect(&doc, &layout, "last"), [2, 35, 100, 20]);
    assert_eq!(rect(&doc, &layout, "after")[1], 54);
    let grid = node(&doc, "grid");
    assert_eq!(layout.client_boxes[grid].unwrap().height, 50);
    assert_eq!(layout.scroll_heights[grid], Some(60));
}
