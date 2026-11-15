use super::*;
use crate::html::parse;

fn node(doc: &Document, id: &str) -> usize {
    doc.nodes
        .iter()
        .enumerate()
        .find_map(|(index, _)| (doc.attr(index, "id") == Some(id)).then_some(index))
        .unwrap()
}

fn background(
    page: &Layout,
    node: usize,
) -> (
    usize,
    ClipRect,
    [i32; 4],
    crate::paint::BackgroundPresentation,
) {
    page.items
        .iter()
        .enumerate()
        .find_map(|(index, item)| match item {
            PaintItem::Background {
                node: owner,
                x,
                y,
                width,
                height,
                border,
                presentation,
                ..
            } if *owner == node => Some((
                index,
                ClipRect::new(*x, *y, *width, *height),
                *border,
                *presentation,
            )),
            _ => None,
        })
        .unwrap()
}

fn border(page: &Layout, node: usize) -> ClipRect {
    let value = page.border_rects[node].as_ref().unwrap();
    ClipRect::new(value.x, value.y, value.width, value.height)
}

#[test]
fn background_has_final_block_height_and_paints_between_color_borders_and_content() {
    let doc = parse("<body style='margin:0'><div id=target style='width:60px;height:40px;padding:3px;border:2px solid blue;background-color:red;background-image:url(Pixels/Case.ppm)'>Text</div></body>");
    let page = build_viewport(&doc, 300, 200, 1.0, &HashMap::new(), &HashMap::new());
    let target = node(&doc, "target");
    let (index, rect, edges, _) = background(&page, target);
    assert_eq!(rect, ClipRect::new(0, 0, 70, 50));
    assert_eq!(rect, border(&page, target));
    assert_eq!(edges, [2; 4]);
    assert!(matches!(
        page.items[index - 1],
        PaintItem::Rect {
            color: 0xff0000,
            ..
        }
    ));
    assert!(page.items[index + 1..index + 5].iter().all(|item| matches!(
        item,
        PaintItem::Rect {
            color: 0x0000ff,
            ..
        }
    )));
    assert!(page.items[index + 5..]
        .iter()
        .any(|item| matches!(item, PaintItem::Text(_))));
    if let PaintItem::Background { image, .. } = &page.items[index] {
        assert_eq!(image.url, "Pixels/Case.ppm");
        assert_eq!(image.stylesheet, None);
    }
}

#[test]
fn background_does_not_change_intrinsic_layout_or_create_replaced_atoms() {
    let markup = "<body style='margin:0'><div style='display:flex;width:220px'><div id=target style='flex:1;min-width:0;padding:3px;BACKGROUND'>Original wrapping words</div><div style='width:40px'>B</div></div></body>";
    let plain = parse(&markup.replace("BACKGROUND", ""));
    let decorated = parse(&markup.replace("BACKGROUND", "background-image:url(HugeUndecoded.ppm)"));
    let a = layout(&plain, 300);
    let b = layout(&decorated, 300);
    assert_eq!(a.height, b.height);
    assert_eq!(
        border(&a, node(&plain, "target")),
        border(&b, node(&decorated, "target"))
    );
    assert_eq!(
        a.runs
            .iter()
            .map(|run| (&run.text, run.x, run.y, run.width(), run.height()))
            .collect::<Vec<_>>(),
        b.runs
            .iter()
            .map(|run| (&run.text, run.x, run.y, run.width(), run.height()))
            .collect::<Vec<_>>()
    );
    assert_eq!(a.controls.len(), b.controls.len());
    assert_eq!(a.targets, b.targets);
    assert!(!b
        .items
        .iter()
        .any(|item| matches!(item, PaintItem::Image { .. })));
}

#[test]
fn background_follows_flex_stretch_relative_offsets_and_resolved_zoom_positions() {
    let doc = parse("<body style='margin:0'><div style='display:flex;width:200px;height:100px'><div id=target style='flex:1;min-width:0;padding:2px;border:1px solid red;position:relative;left:5px;top:7px;background-image:url(Tile.ppm);background-position:right 2em bottom 1rem;background-size:contain;background-repeat:repeat-y'>A</div><div style='flex:1;min-width:0'>B</div></div></body>");
    let page = build_viewport(&doc, 600, 400, 2.0, &HashMap::new(), &HashMap::new());
    let target = node(&doc, "target");
    let (_, rect, edges, presentation) = background(&page, target);
    assert_eq!(rect, border(&page, target));
    assert_eq!(rect.bottom - rect.top, 200);
    assert_eq!((rect.left, rect.top), (10, 14));
    assert_eq!(edges, [2; 4]);
    assert_eq!(presentation.scale, 2.0);
    assert_eq!(presentation.size, css::BackgroundSize::Contain);
    assert_eq!(presentation.repeat, [false, true]);
    assert_eq!(presentation.position, [(1.0, -64.0), (1.0, -32.0)]);
}

#[test]
fn background_table_cell_stretch_keeps_decoration_out_of_valign_content_translation() {
    let doc = parse("<body style='margin:0'><table cellpadding=0 cellspacing=0 style='width:200px'><tr><td style='height:70px'>A</td><td id=cell valign=bottom style='padding:2px;border:1px solid blue;background-image:url(Cell.ppm)'><div id=child style='height:10px'>B</div></td></tr></table></body>");
    let page = layout(&doc, 400);
    let cell = node(&doc, "cell");
    let (_, rect, _, _) = background(&page, cell);
    assert_eq!(rect, border(&page, cell));
    assert_eq!(rect.bottom - rect.top, 70);
    assert_eq!(border(&page, node(&doc, "child")).bottom, rect.bottom - 3);
}

#[test]
fn background_uses_ancestor_clips_and_fixed_scroll_round_trips() {
    let doc = parse("<body style='margin:0'><div style='width:80px;height:40px;border:2px solid black;overflow:hidden'><div id=clipped style='width:100px;height:80px;position:relative;left:10px;top:5px;background-image:url(Clip.ppm)'></div><div id=fixed style='position:fixed;left:100px;top:20px;width:30px;height:20px;background-image:url(Fixed.ppm)'></div></div></body>");
    let mut page = build_viewport(&doc, 400, 200, 1.0, &HashMap::new(), &HashMap::new());
    let clipped = node(&doc, "clipped");
    let (index, rect, _, _) = background(&page, clipped);
    assert_eq!(rect, border(&page, clipped));
    assert_eq!(page.paint_clip(index), Some(ClipRect::new(2, 2, 80, 40)));
    let fixed = node(&doc, "fixed");
    let (index, start, _, _) = background(&page, fixed);
    assert_eq!(page.paint_clip(index), None);
    assert!(page.paint_is_positioned(index));
    page.set_scroll(123);
    assert_eq!(background(&page, fixed).1, start.translate_y(123));
    page.set_scroll(0);
    assert_eq!(background(&page, fixed).1, start);
}

#[test]
fn background_blocks_clickthrough_without_inventing_a_link_or_native_target() {
    let doc = parse("<body style='margin:0'><a href=behind><img id=media src=fixture.ppm width=100 height=60></a><div id=cover style='position:absolute;left:0;top:0;width:50px;height:30px;background-image:url(Cover.ppm)'></div></body>");
    let page = build_viewport(&doc, 300, 200, 1.0, &HashMap::new(), &HashMap::new());
    let media = node(&doc, "media");
    assert!(!page.image_topmost_at(media, 10, 10));
    assert!(page.image_topmost_at(media, 60, 10));
    assert_eq!(page.link_at(10, 10), None);
    assert_eq!(page.link_at(60, 10), Some("behind"));
    assert_eq!(page.control_at(10, 10), None);
    assert_eq!(page.targets, ["behind"]);
}

#[test]
fn background_reuses_inline_block_box_but_defers_inline_fragments_and_replaced_decorations() {
    let doc = parse("<body style='margin:0'><span id=inline style='background-image:url(Inline.ppm)'>A</span><span id=box style='display:inline-block;width:40px;height:20px;background-image:url(Box.ppm)'>B</span><img id=image src=fixture.ppm width=20 height=20 style='background-image:url(Replaced.ppm)'></body>");
    let page = layout(&doc, 300);
    let owners: Vec<_> = page
        .items
        .iter()
        .filter_map(|item| match item {
            PaintItem::Background { node, .. } => Some(*node),
            _ => None,
        })
        .collect();
    assert_eq!(owners, [node(&doc, "box")]);
    assert_eq!(
        background(&page, node(&doc, "box")).1,
        border(&page, node(&doc, "box"))
    );
}
