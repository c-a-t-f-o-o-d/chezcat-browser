//! Original decoded-image controls for height-derived intrinsic widths.
use super::*;

const TILE: &[u8] = b"P6\n2 2\n255\n\xff\x00\x00\x00\xff\x00\x00\x00\xff\xff\xff\x00";
const CLEAR: u32 = 0x123456;
const VIEW_WIDTH: i32 = 800;
const VIEW_HEIGHT: i32 = 480;
const BASE: &str = "html,body,div,a,img{margin:0;padding:0;border:0}body{font-size:16px;line-height:20px}#row{display:flex;width:300px;align-items:flex-start;gap:8px}img{display:block;height:120px;image-rendering:pixelated}";
const DIRECT: &str = "<div id=row><img id=target src=tile.ppm alt=''></div>";
const WRAPPED: &str = "<div id=row><a id=card href=detail style='overflow:hidden'><img id=target src=tile.ppm alt=''></a></div>";

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn tile(width: usize, height: usize) -> crate::image::Bitmap {
    let original = crate::image::decode(TILE).unwrap();
    assert_eq!((original.width, original.height), (2, 2));
    assert_eq!(original.pixels, [0xffff0000, 0xff00ff00, 0xff0000ff, 0xffffff00]);
    if (width, height) == (2, 2) { return original; }
    assert!(matches!((width, height), (4, 2) | (2, 4)));
    // Original wide/tall Netpbm variants duplicate columns or rows of the
    // same four-color tile. Natural dimensions still come from the decoder.
    let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
    for y in 0..height {
        for x in 0..width {
            let color = original.pixels[(y * 2 / height) * 2 + x * 2 / width];
            bytes.extend_from_slice(&[(color >> 16) as u8, (color >> 8) as u8, color as u8]);
        }
    }
    let decoded = crate::image::decode(&bytes).unwrap();
    assert_eq!((decoded.width, decoded.height), (width, height));
    decoded
}

fn page(body: &str, rules: &str, bitmap: Option<&crate::image::Bitmap>, zoom: f32) -> (Document, Layout) {
    let doc = crate::html::parse(&format!("<!doctype html><html><head><style>{BASE}{rules}</style></head><body>{body}</body></html>"));
    let sizes = bitmap.map_or_else(HashMap::new, |bitmap| {
        (0..doc.nodes.len()).filter(|&node| {
            matches!(&doc.nodes[node].kind, NodeKind::Element { tag, .. } if tag == "img")
        }).map(|node| (node, (bitmap.width, bitmap.height))).collect()
    });
    let page = build_viewport(&doc, VIEW_WIDTH, VIEW_HEIGHT, zoom, &HashMap::new(), &sizes);
    (doc, page)
}

fn image_box(doc: &Document, page: &Layout, name: &str) -> (usize, i32, i32, i32, i32) {
    let node = id(doc, name);
    let mut images = page.items.iter().enumerate().filter_map(|(index, item)| match item {
        PaintItem::Image { node: owner, x, y, width, height, .. } if *owner == node =>
            Some((index, *x, *y, *width, *height)),
        _ => None,
    });
    let image = images.next().expect("decoded image paint item");
    assert!(images.next().is_none(), "one paint item per original image");
    image
}

fn geometry(doc: &Document, page: &Layout, name: &str, expected: (i32, i32, i32, i32)) {
    let (_, x, y, width, height) = image_box(doc, page, name);
    assert_eq!((x, y, width, height), expected, "{name}: image content rectangle");
    border_box(doc, page, name, expected);
}

fn border_box(doc: &Document, page: &Layout, name: &str, expected: (i32, i32, i32, i32)) {
    let node = id(doc, name);
    let border = page.border_rects[node].expect("final border rectangle");
    assert_eq!((border.x, border.y, border.width, border.height), expected, "{name}: border rectangle");
    let client = page.client_boxes[node].expect("final client rectangle");
    assert_eq!((client.width, client.height), (expected.2, expected.3), "{name}: client dimensions");
    assert_eq!((client.left, client.top, client.right, client.bottom), (0, 0, 0, 0));
}

fn link_box(page: &Layout, rect: (i32, i32, i32, i32), href: &str) {
    let (x, y, width, height) = rect;
    for (px, py) in [(x, y), (x + width / 2, y + height / 2), (x + width - 1, y + height - 1)] {
        assert_eq!(page.link_at(px, py), Some(href), "link interior at ({px},{py})");
    }
    for (px, py) in [(x - 1, y + 1), (x + width, y + 1), (x + 1, y - 1), (x + 1, y + height)] {
        assert_eq!(page.link_at(px, py), None, "exclusive link edge at ({px},{py})");
    }
}

fn paint(page: &Layout, bitmap: &crate::image::Bitmap) -> crate::paint::Canvas {
    let mut canvas = crate::paint::Canvas::new(VIEW_WIDTH as usize, VIEW_HEIGHT as usize);
    canvas.pixels.fill(CLEAR);
    for (index, item) in page.items.iter().enumerate() {
        canvas.set_clip(page.paint_clip(index));
        match item {
            PaintItem::Image { x, y, width, height, presentation, .. } =>
                canvas.fitted_image(ClipRect::new(*x, *y, *width, *height), bitmap, *presentation),
            PaintItem::HitBox { .. } => {}
            _ => panic!("unexpected paint item in the original image fixture: {item:?}"),
        }
    }
    canvas
}

fn color_samples(canvas: &crate::paint::Canvas, rect: (i32, i32, i32, i32)) {
    let (x, y, width, height) = rect;
    for (dx, dy, color) in [(width / 4, height / 4, 0xff0000), (3 * width / 4, height / 4, 0x00ff00),
        (width / 4, 3 * height / 4, 0x0000ff), (3 * width / 4, 3 * height / 4, 0xffff00)] {
        assert_eq!(canvas.pixels[((y + dy) * VIEW_WIDTH + x + dx) as usize], color,
            "decoded quadrant at ({dx},{dy}) must remain visible");
    }
    assert_eq!(canvas.pixels[(y * VIEW_WIDTH + x + width) as usize], CLEAR, "right paint edge");
    assert_eq!(canvas.pixels[((y + height) * VIEW_WIDTH + x) as usize], CLEAR, "bottom paint edge");
}

#[test]
fn row_flex_height_transfers_square_wide_and_tall_natural_ratios() {
    for (width, height, expected_width) in [(2, 2, 120), (4, 2, 240), (2, 4, 60)] {
        let bitmap = tile(width, height);
        let (doc, page) = page(DIRECT, "#target{width:auto}", Some(&bitmap), 1.0);
        geometry(&doc, &page, "target", (0, 0, expected_width, 120));
    }
}

#[test]
fn clipped_flex_links_measure_the_height_derived_image_width() {
    for (width, height, expected_width) in [(2, 2, 120), (4, 2, 240), (2, 4, 60)] {
        let bitmap = tile(width, height);
        let (doc, page) = page(WRAPPED, "#target{width:auto}", Some(&bitmap), 1.0);
        let expected = (0, 0, expected_width, 120);
        geometry(&doc, &page, "target", expected);
        border_box(&doc, &page, "card", expected);
        let (item, ..) = image_box(&doc, &page, "target");
        let clip = page.paint_clip(item).expect("the link clips its contents");
        assert_eq!((clip.left, clip.top, clip.right, clip.bottom), (0, 0, expected_width, 120));
        link_box(&page, expected, "detail");
    }
}

#[test]
fn decoded_quadrants_and_far_edge_hits_survive_clipped_flex_sizing() {
    let bitmap = tile(2, 2);
    let (doc, page) = page(WRAPPED, "", Some(&bitmap), 1.0);
    geometry(&doc, &page, "target", (0, 0, 120, 120));
    color_samples(&paint(&page, &bitmap), (0, 0, 120, 120));
    link_box(&page, (0, 0, 120, 120), "detail");
}

#[test]
fn definite_literal_variable_math_and_em_heights_share_integer_zoom_geometry() {
    let bitmap = tile(4, 2);
    for zoom in [1.0, 2.0] {
        for value in ["120px", "var(--height)", "calc(60px + 60px)", "7.5em"] {
            for body in [DIRECT, WRAPPED] {
                let (doc, page) = page(body, &format!("#target{{--height:120px;width:auto;height:{value}}}"), Some(&bitmap), zoom);
                let expected = (0, 0, (240.0 * zoom) as i32, (120.0 * zoom) as i32);
                geometry(&doc, &page, "target", expected);
                if body == WRAPPED { border_box(&doc, &page, "card", expected); }
            }
        }
    }
}

#[test]
fn explicit_css_width_is_not_replaced_by_the_height_ratio() {
    let bitmap = tile(4, 2);
    for body in [DIRECT, WRAPPED] {
        for value in ["30px", "var(--width)"] {
            let (doc, page) = page(body, &format!("#target{{--width:30px;width:{value}}}"), Some(&bitmap), 1.0);
            geometry(&doc, &page, "target", (0, 0, 30, 120));
            if body == WRAPPED { border_box(&doc, &page, "card", (0, 0, 30, 120)); }
        }
    }
}

#[test]
fn existing_html_width_hints_remain_distinct_from_auto_natural_width() {
    let bitmap = tile(2, 2);
    for body in [DIRECT, WRAPPED] {
        // No author width declaration is present: the existing HTML hint wins.
        let body = body.replace("id=target", "id=target width=40");
        let (doc, page) = page(&body, "", Some(&bitmap), 1.0);
        geometry(&doc, &page, "target", (0, 0, 40, 120));
        if body.contains("id=card") { border_box(&doc, &page, "card", (0, 0, 40, 120)); }
    }
}

#[test]
fn auto_height_preserves_decoded_natural_sizes() {
    for (width, height) in [(2, 2), (4, 2), (2, 4)] {
        let bitmap = tile(width, height);
        for zoom in [1.0, 2.0] {
            for body in [DIRECT, WRAPPED] {
                let (doc, page) = page(body, "#target{height:auto}", Some(&bitmap), zoom);
                let expected = (0, 0, (width as f32 * zoom) as i32, (height as f32 * zoom) as i32);
                geometry(&doc, &page, "target", expected);
                if body == WRAPPED { border_box(&doc, &page, "card", expected); }
            }
        }
    }
}

#[test]
fn missing_decoded_dimensions_do_not_invent_an_intrinsic_ratio() {
    // The host placeholder for empty alt at16px is64px wide. Its automatic
    // height is the20px line-height plus12px. These are placeholder controls,
    // not evidence of a decoded image or a natural aspect ratio.
    for (rules, expected_height) in [("", 120), ("#target{height:auto}", 32)] {
        let (doc, page) = page(DIRECT, rules, None, 1.0);
        geometry(&doc, &page, "target", (0, 0, 64, expected_height));
    }
}

#[test]
fn unresolved_percentage_height_keeps_the_natural_size() {
    let bitmap = tile(2, 2);
    for value in ["50%", "var(--height)"] {
        for body in [DIRECT, WRAPPED] {
            let (doc, page) = page(body, &format!("#target{{--height:50%;height:{value}}}"), Some(&bitmap), 1.0);
            geometry(&doc, &page, "target", (0, 0, 2, 2));
            if body == WRAPPED { border_box(&doc, &page, "card", (0, 0, 2, 2)); }
        }
    }
}

#[test]
fn neighboring_clipped_image_cards_keep_their_gap_pixels_and_link_targets() {
    let bitmap = tile(2, 2);
    let body = "<div id=row><a id=card href=detail style='overflow:hidden'><img id=target src=tile.ppm alt=''></a><a id=other href=second style='overflow:hidden'><img id=next src=tile.ppm alt=''></a></div>";
    for zoom in [1.0, 2.0] {
        let (doc, page) = page(body, "", Some(&bitmap), zoom);
        let extent = (120.0 * zoom) as i32;
        let next = (128.0 * zoom) as i32;
        let first = (0, 0, extent, extent);
        let second = (next, 0, extent, extent);
        geometry(&doc, &page, "target", first);
        geometry(&doc, &page, "next", second);
        border_box(&doc, &page, "card", first);
        border_box(&doc, &page, "other", second);
        link_box(&page, first, "detail");
        link_box(&page, second, "second");
        assert_eq!(page.link_at(extent + (4.0 * zoom) as i32, 1), None, "the gap is not a link");
        let canvas = paint(&page, &bitmap);
        color_samples(&canvas, first);
        color_samples(&canvas, second);
    }
}
