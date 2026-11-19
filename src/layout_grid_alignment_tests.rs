//! Physical LTR grid inline alignment over the actual layout, paint and hit records.
use super::*;
use crate::paint::Canvas;

fn page(markup: &str, rules: &str) -> (Document, Layout) {
    let doc = crate::html::parse(&format!(
        "<html><head><style>html,body{{margin:0;padding:0}}{rules}</style></head><body>{markup}</body></html>"
    ));
    let layout = build_viewport(&doc, 640, 480, 1.0, &HashMap::new(), &HashMap::new());
    (doc, layout)
}

fn node(doc: &Document, id: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some(id))
        .expect(id)
}

fn rect(doc: &Document, layout: &Layout, id: &str) -> [i32; 4] {
    let r = layout.border_rects[node(doc, id)].expect(id);
    [r.x, r.y, r.width, r.height]
}

#[test]
fn grid_inline_alignment_action_stack_positions_paint_and_real_hit_bounds() {
    let markup="<div id=actions><a id=first href='first'>Open</a><a id=middle href='middle'>Hold</a><a id=last href='last'>Send</a><a id=auto href='auto'>Close</a><a id=full href='full'>Review</a></div>";
    let rules="#actions{display:grid;position:absolute;left:16px;top:20px;width:192px;gap:8px;justify-items:end}
               a{display:inline-block;width:64px;height:32px;background:#2468ac}
               #first{justify-self:start}#middle{justify-self:center}#last{justify-self:end}#full{width:100%}";
    let (doc, layout) = page(markup, rules);
    assert_eq!(rect(&doc, &layout, "actions"), [16, 20, 192, 192]);
    let mut canvas = Canvas::new(240, 224);
    canvas.fill_rect(0, 0, 240, 224, 0xffffff);
    for (index, item) in layout.items.iter().enumerate() {
        if let PaintItem::Rect {
            x,
            y,
            width,
            height,
            color,
            alpha,
        } = item
        {
            canvas.set_clip(layout.paint_clip(index));
            canvas.fill_rect_alpha(*x, *y, *width, *height, *color, *alpha);
        }
    }
    for (row, (id, left, width)) in [
        ("first", 16, 64),
        ("middle", 80, 64),
        ("last", 144, 64),
        ("auto", 144, 64),
        ("full", 16, 192),
    ]
    .into_iter()
    .enumerate()
    {
        let top = 20 + 40 * row as i32;
        assert_eq!(rect(&doc, &layout, id), [left, top, width, 32], "{id}");
        let glyph = layout
            .runs
            .iter()
            .find(|run| {
                run.link
                    .is_some_and(|index| layout.link_targets().nth(index) == Some(id))
            })
            .expect(id);
        assert!(
            glyph.x >= left && glyph.x + glyph.width() <= left + width,
            "{id} text did not follow its box"
        );
        for x in 15..=208 {
            let inside = (left..left + width).contains(&x);
            assert_eq!(
                canvas.pixels[(top as usize + 29) * canvas.width + x as usize],
                if inside { 0x2468ac } else { 0xffffff },
                "{id} pixel {x}"
            );
            assert_eq!(
                layout.link_at(x, top + 29),
                inside.then_some(id),
                "{id} hit {x}"
            );
        }
        assert!(layout.link_at(left + 1, top + 33).is_none());
    }
}

#[test]
fn grid_inline_alignment_auto_width_fits_content_and_respects_edges_constraints_and_percentages() {
    let markup="<div id=grid><div class=fit id=center><div class=inner></div></div><div class=fit id=capped><div class=inner></div></div><div class=fit id=minimum><div class=inner></div></div><div class=fit id=stretch><div class=inner></div></div><div id=percent></div></div>";
    let rules="#grid{display:grid;width:200px;justify-items:center}
               .fit{padding:0 6px;border:2px solid black;margin-left:10px;margin-right:6px}.inner{width:54px;height:12px}
               #capped{justify-self:end;max-width:40px}#minimum{justify-self:end;min-width:80px}
               #stretch{justify-self:stretch}#percent{width:50%;height:4px}";
    let (doc, layout) = page(markup, rules);
    for (id, x, width) in [
        ("center", 67, 70),
        ("capped", 138, 56),
        ("minimum", 98, 96),
        ("stretch", 10, 184),
        ("percent", 50, 100),
    ] {
        let r = rect(&doc, &layout, id);
        assert_eq!((r[0], r[2]), (x, width), "{id}");
    }
    let child = doc.nodes[node(&doc, "center")].children[0];
    let inner = layout.border_rects[child].unwrap();
    assert_eq!(
        (inner.x, inner.width),
        (75, 54),
        "fit-content child's position and used width"
    );
}

#[test]
fn grid_inline_alignment_auto_margins_take_precedence_and_physical_margins_define_alignment_area() {
    let markup="<div id=grid><div id=leftauto></div><div id=rightauto></div><div id=both></div><div id=middle></div><div id=end></div></div>";
    let rules = "#grid{display:grid;width:200px;justify-items:end}#grid div{width:40px;height:6px}
               #leftauto{margin:0 10px 0 auto;justify-self:start}#rightauto{margin:0 auto 0 8px}
               #both{margin:0 auto;justify-self:start}#middle,#end{margin:0 20px 0 10px}
               #middle{justify-self:center}";
    let (doc, layout) = page(markup, rules);
    for (id, x) in [
        ("leftauto", 150),
        ("rightauto", 8),
        ("both", 80),
        ("middle", 75),
        ("end", 140),
    ] {
        let r = rect(&doc, &layout, id);
        assert_eq!((r[0], r[2]), (x, 40), "{id}");
    }
}

#[test]
fn grid_inline_alignment_shifted_overflow_clip_and_native_descendant_hit_match_their_boxes() {
    let markup="<div id=grid><div id=clip><a href='inside'>Go</a></div><div id=visible><a href='outside'>Go</a></div><div id=panel><input id=field value=kept></div></div>";
    let rules="#grid{display:grid;width:120px;justify-items:end;row-gap:3px}
               #clip,#visible{width:40px;height:16px}#clip{overflow:hidden}#visible{overflow:visible}
               a{display:block;width:80px;height:16px;background:#2468ac}#panel{width:48px}#field{width:24px}";
    let (doc, layout) = page(markup, rules);
    assert_eq!(rect(&doc, &layout, "clip"), [80, 0, 40, 16]);
    assert_eq!(rect(&doc, &layout, "visible"), [80, 19, 40, 16]);
    assert_eq!(layout.link_at(100, 8), Some("inside"));
    assert_eq!(layout.link_at(140, 8), None);
    assert_eq!(layout.link_at(140, 27), Some("outside"));
    let field = node(&doc, "field");
    let native = layout.controls.iter().find(|c| c.node == field).unwrap();
    assert_eq!(native.x, 72);
    assert_eq!(layout.control_at(native.x + 2, native.y + 2), Some(field));
    assert_eq!(layout.control_at(2, native.y + 2), None);
}

#[test]
fn grid_inline_alignment_does_not_reposition_ordinary_block_or_flex_items() {
    let markup = "<div id=plain><div id=block></div></div><div id=flex><div id=item></div></div>";
    let rules="#plain,#flex{width:180px;justify-items:end}#block,#item{width:48px;height:12px;justify-self:center}
               #flex{display:flex;flex-direction:column;align-items:start}";
    let (doc, layout) = page(markup, rules);
    assert_eq!(rect(&doc, &layout, "block"), [0, 0, 48, 12]);
    assert_eq!(rect(&doc, &layout, "item"), [0, 12, 48, 12]);
}

#[test]
fn grid_inline_alignment_nested_grid_uses_its_own_default_for_direct_items() {
    let markup="<div id=outer><div id=inner><div id=local></div></div><div id=other><div id=chosen></div><div id=override></div></div></div>";
    let rules =
        "#outer{display:grid;width:180px;justify-items:end}#inner,#other{display:grid;width:100px}
               #other{justify-items:center}#local,#chosen,#override{width:36px;height:7px}
               #chosen{justify-self:auto}#override{justify-self:end;justify-self:potato}";
    let (doc, layout) = page(markup, rules);
    assert_eq!(rect(&doc, &layout, "inner"), [80, 0, 100, 7]);
    assert_eq!(rect(&doc, &layout, "local"), [80, 0, 36, 7]);
    assert_eq!(rect(&doc, &layout, "other"), [80, 7, 100, 14]);
    assert_eq!(rect(&doc, &layout, "chosen"), [112, 7, 36, 7]);
    assert_eq!(rect(&doc, &layout, "override"), [144, 14, 36, 7]);
}

#[test]
fn grid_inline_alignment_substitution_and_rollback_follow_the_item_cascade() {
    let markup = "<div id=grid><div id=literal></div><div id=variable></div><div id=missing></div><div id=rollback style='justify-self:revert-layer'></div></div>";
    let rules = "#grid{display:grid;width:160px;--side:center;justify-items:end}
        #grid div{width:40px;height:12px}
        #literal{justify-self:start;justify-self:bogus}
        #variable{justify-self:var(--side)}
        #missing{justify-self:start;justify-self:var(--absent)}
        #rollback{justify-self:center}";
    let (doc, layout) = page(markup, rules);
    for (id, x) in [("literal", 0), ("variable", 60), ("missing", 120), ("rollback", 60)] {
        assert_eq!(rect(&doc, &layout, id)[0], x, "{id}");
    }
}
