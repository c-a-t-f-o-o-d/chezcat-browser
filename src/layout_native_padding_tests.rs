//! Original native button padding consumers. No styled-border/theme expansion.
use super::*;
use crate::html::parse;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}

fn build_page(source: &str, width: i32, height: i32, zoom: f32) -> (Document, Layout) {
    let doc = parse(source);
    let page = build_viewport(&doc, width, height, zoom, &HashMap::new(), &HashMap::new());
    (doc, page)
}

fn control<'a>(doc: &Document, page: &'a Layout, name: &str) -> &'a ControlBox {
    let node = id(doc, name);
    page.controls.iter().find(|control| control.node == node).unwrap()
}

fn geometry(control: &ControlBox) -> (i32, i32, i32, i32) {
    (control.x, control.y, control.width, control.height)
}

fn client(page: &Layout, control: &ControlBox, padding: [i32; 4]) {
    let client = page.client_boxes[control.node].unwrap();
    assert_eq!(control.padding, padding);
    assert_eq!(client.padding, padding);
    // Existing native theme borders remain physical one-pixel edges.
    assert_eq!((client.left, client.top, client.right, client.bottom), (1, 1, 1, 1));
    assert_eq!((client.width, client.height), ((control.width - 2).max(0), (control.height - 2).max(0)));
    let border = page.border_rects[control.node].unwrap();
    assert_eq!((border.x, border.y, border.width, border.height), geometry(control));
}

#[test]
fn native_padding_layout_button_submit_reset_content_and_border_boxes() {
    for (tag, kind) in [
        ("<button id=target type=button>Proceed</button>", Kind::Button),
        ("<input id=target type=submit value=Proceed>", Kind::Submit),
        ("<input id=target type=reset value=Proceed>", Kind::Reset),
    ] {
        for (sizing, width, height) in [("content-box", 80, 24), ("border-box", 100, 36)] {
            let source = format!(
                "<style>body{{margin:0}}#target{{position:absolute;left:20px;top:15px;box-sizing:{sizing};width:{width}px;height:{height}px;padding:6px 10px;margin:0}}</style><body>{tag}</body>"
            );
            let (doc, page) = build_page(&source, 300, 160, 1.0);
            assert_eq!(page.controls.len(), 1);
            let control = control(&doc, &page, "target");
            assert_eq!(forms::kind(&doc, control.node), Some(kind));
            assert_eq!(geometry(control), (20, 15, 100, 36), "{tag}: {sizing}");
            assert!(control.image.is_none());
            client(&page, control, [6, 10, 6, 10]);
            for (dx, dy) in [(1, 1), (90, 12), (50, 30), (99, 35)] {
                assert_eq!(page.control_at(control.x + dx, control.y + dy), Some(control.node));
            }
            for (dx, dy) in [(-1, 12), (100, 12), (50, -1), (50, 36)] {
                assert_eq!(page.control_at(control.x + dx, control.y + dy), None);
            }
        }
    }
}

#[test]
fn native_padding_layout_auto_preserves_zero_metrics_and_adds_asymmetric_edges_once() {
    let markup = "<style>body{margin:0}button,input{position:absolute;top:10px;margin:0;PADDING}#b{left:10px}#s{left:200px}#r{left:390px}</style><body><button id=b type=button>Proceed</button><input id=s type=submit value=Proceed><input id=r type=reset value=Proceed></body>";
    for zoom in [1.0, 2.0] {
        let (plain_doc, plain) = build_page(&markup.replace("PADDING", ""), 1200, 240, zoom);
        let (zero_doc, zero) = build_page(&markup.replace("PADDING", "padding:0"), 1200, 240, zoom);
        let (padded_doc, padded) = build_page(&markup.replace("PADDING", "padding:3px 7px 5px 11px"), 1200, 240, zoom);
        let factor = zoom as i32;
        for name in ["b", "s", "r"] {
            let a = control(&plain_doc, &plain, name);
            let z = control(&zero_doc, &zero, name);
            let p = control(&padded_doc, &padded, name);
            assert_eq!(geometry(z), geometry(a), "unset/zero {name}, zoom {zoom}");
            assert_eq!(a.padding, [0; 4]);
            assert_eq!((p.x, p.y), (a.x, a.y));
            assert_eq!((p.width - a.width, p.height - a.height), (18 * factor, 8 * factor), "auto {name}, zoom {zoom}");
            client(&padded, p, [3, 7, 5, 11].map(|edge| edge * factor));
        }
    }
}

#[test]
fn native_padding_layout_positioned_percent_uses_containing_width_and_zoomed_outer_size() {
    for zoom in [1.0, 2.0] {
        for (sizing, width, height) in [("content-box", 80, 24), ("border-box", 120, 44)] {
            let source = format!(
                "<body style='margin:0'><div style='position:relative;margin:20px 0 0 30px;width:200px;height:120px'><button id=target type=button style='position:absolute;right:10px;bottom:8px;box-sizing:{sizing};width:{width}px;height:{height}px;padding:5% 10%;margin:0'>Go</button></div></body>"
            );
            let (doc, page) = build_page(&source, 800, 500, zoom);
            let control = control(&doc, &page, "target");
            let factor = zoom as i32;
            assert_eq!(geometry(control), (100 * factor, 88 * factor, 120 * factor, 44 * factor), "{sizing}, zoom {zoom}");
            client(&page, control, [10, 20, 10, 20].map(|edge| edge * factor));
            assert_eq!(page.control_at(control.x + control.width - 1, control.y + control.height - 1), Some(control.node));
            assert_eq!(page.control_at(control.x + control.width, control.y + control.height - 1), None);
        }
    }
}

#[test]
fn native_padding_layout_flex_and_intrinsic_boxes_count_asymmetric_edges_once() {
    let (doc, page) = build_page(
        "<body style='margin:0'><div style='display:flex;width:200px;gap:8px;align-items:flex-start'><button id=a type=button style='flex:0 0 auto;width:80px;height:24px;padding:3px 7px 5px 11px;margin:0'>A</button><input id=b type=submit value=B style='flex:0 0 auto;width:60px;height:20px;padding:2px 4px 6px 8px;margin:0'></div></body>",
        400, 200, 1.0,
    );
    let a = control(&doc, &page, "a");
    let b = control(&doc, &page, "b");
    assert_eq!(geometry(a), (0, 0, 98, 32));
    assert_eq!(geometry(b), (106, 0, 72, 28));
    client(&page, a, [3, 7, 5, 11]);
    client(&page, b, [2, 4, 6, 8]);
    assert_eq!(page.control_at(97, 31), Some(a.node));
    assert_eq!(page.control_at(100, 10), None);
    assert_eq!(page.control_at(106, 0), Some(b.node));

    let markup = "<body style='margin:0'><div id=wrapper style='display:inline-block'><button id=child type=button style='width:80px;height:24px;margin:0;PADDING'>A</button></div></body>";
    let (plain_doc, plain) = build_page(&markup.replace("PADDING", "padding:0"), 400, 200, 1.0);
    let (padded_doc, padded) = build_page(&markup.replace("PADDING", "padding:3px 7px 5px 11px"), 400, 200, 1.0);
    let plain_wrapper = plain.border_rects[id(&plain_doc, "wrapper")].unwrap();
    let padded_wrapper = padded.border_rects[id(&padded_doc, "wrapper")].unwrap();
    assert_eq!(plain_wrapper.width, 80);
    assert_eq!(padded_wrapper.width, 98);
    let child = control(&padded_doc, &padded, "child");
    assert_eq!((child.width, child.height), (98, 32));
    assert_eq!(child.x, padded_wrapper.x);
    assert!(child.y >= padded_wrapper.y);
    assert!(child.y + child.height <= padded_wrapper.y + padded_wrapper.height);
}

#[test]
fn native_padding_layout_zero_content_bounded_edges_and_other_control_consumers() {
    let (doc, page) = build_page(
        "<body style='margin:0'><button id=zero type=button style='position:absolute;left:10px;top:10px;width:0;height:0;padding:3px 5px'>X</button><button id=bounded type=button style='position:absolute;left:0;top:50px;width:32768px;height:24px;padding:1000000000px'>X</button></body>",
        400, 200, 1.0,
    );
    let zero = control(&doc, &page, "zero");
    assert_eq!(geometry(zero), (10, 10, 10, 6));
    client(&page, zero, [3, 5, 3, 5]);
    assert_eq!(page.control_at(19, 15), Some(zero.node));
    let bounded = control(&doc, &page, "bounded");
    assert!((1..=MAX_WIDTH).contains(&bounded.width));
    assert!((1..=MAX_WIDTH).contains(&bounded.height));
    assert!(bounded.padding.into_iter().all(|edge| (0..=MAX_WIDTH).contains(&edge)));

    let markup = "<style>body{margin:0}input,textarea,select{display:block;width:80px;height:24px;margin:0;PADDING}</style><body><input id=text><textarea id=area>Text</textarea><select id=select><option>Choice</option></select><input id=checkbox type=checkbox><input id=radio type=radio></body>";
    let (plain_doc, plain) = build_page(&markup.replace("PADDING", "padding:0"), 400, 300, 1.0);
    let (padded_doc, padded) = build_page(&markup.replace("PADDING", "padding:12px 25px"), 400, 300, 1.0);
    assert_eq!(padded.controls.len(), plain.controls.len());
    for name in ["text", "area", "select", "checkbox", "radio"] {
        let a = control(&plain_doc, &plain, name);
        let b = control(&padded_doc, &padded, name);
        assert_eq!(geometry(b), geometry(a), "separate consumer: {name}");
        client(&padded, b, [0; 4]);
    }

    // Image padding already has its own replaced-image path; the new native
    // caption padding field must remain zero and must not add those edges twice.
    let doc = parse("<body style='margin:0'><input id=image type=image src=original.ppm style='width:24px;height:12px;padding:2px 3px;border:0;margin:0'></body>");
    let node = id(&doc, "image");
    let images = HashMap::from([(node, (24, 12))]);
    let page = build_viewport(&doc, 400, 200, 1.0, &HashMap::new(), &images);
    let image = control(&doc, &page, "image");
    assert_eq!((image.width, image.height), (30, 16));
    assert_eq!(image.padding, [0; 4]);
    let content = image.image.as_ref().unwrap();
    assert_eq!((content.x, content.y, content.width, content.height), (3, 2, 24, 12));
}
