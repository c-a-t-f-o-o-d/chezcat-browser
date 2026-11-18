//! Original shrink-to-fit controls with the authored geometric font. The
//! fractional advances recreate the one-pixel whole-string/word mismatch.
use super::*;
use crate::paint::Canvas;
use std::sync::Arc;

fn fonts() -> Book {
    let mut fonts = Book::empty();
    fonts
        .add(
            "Geometry".into(),
            400,
            false,
            Arc::new(crate::font::Font::parse(&crate::font::tests::fixture_font()).unwrap()),
        )
        .unwrap();
    fonts
}

fn spec(fonts: &Book, zoom: f32) -> FontSpec {
    FontSpec::new(fonts, &["Geometry".into()], 400, false, 16.0 * zoom).unwrap()
}

fn page(body: &str, rules: &str, width: i32, zoom: f32, fonts: &Book) -> (Document, Layout) {
    let document = crate::html::parse(&format!(
        "<html><head><style>html,body{{margin:0;padding:0;font:16px Geometry;color:#202020}}\
         .caption{{position:absolute;left:20px;top:20px;margin:0}}{rules}</style></head><body>{body}</body></html>"
    ));
    let layout = build_viewport_with_fonts(
        &document,
        width,
        480,
        zoom,
        &HashMap::new(),
        &HashMap::new(),
        None,
        fonts,
    );
    (document, layout)
}

fn rect(document: &Document, page: &Layout, id: &str) -> BorderRect {
    let node = (0..document.nodes.len())
        .find(|&node| document.attr(node, "id") == Some(id))
        .unwrap();
    page.border_rects[node].unwrap()
}

fn runs(page: &Layout, rect: BorderRect) -> Vec<&TextRun> {
    page.runs
        .iter()
        .filter(|run| {
            run.x >= rect.x
                && run.x < rect.x + rect.width
                && run.y >= rect.y
                && run.y < rect.y + rect.height
        })
        .collect()
}

fn paint_text(page: &Layout, rect: BorderRect) -> Vec<u32> {
    let mut canvas = Canvas::new(256, 128);
    canvas.fill_rect(0, 0, 256, 128, 0xffffff);
    for run in runs(page, rect) {
        run.shaped
            .as_ref()
            .unwrap()
            .draw(&mut canvas, run.x - rect.x, run.y - rect.y, run.color);
    }
    canvas.pixels
}

#[test]
fn absolute_caption_intrinsic_rounding_matches_word_placement_and_literal_paint() {
    let fonts = fonts();
    let font = spec(&fonts, 1.0);
    assert_eq!(font.measure("A o"), 25);
    assert_eq!(
        font.measure("A") + font.measure(" ") + font.measure("o"),
        26,
        "The original face must reproduce the measured rounding mismatch"
    );
    for width in [640, 1280] {
        for zoom in [0.75, 1.0, 1.25, 1.5, 2.0] {
            let (document, page) = page(
                "<p id=automatic class=caption>A o</p><p id=literal class=caption>A o</p>",
                "#literal{left:160px;width:120px}",
                width,
                zoom,
                &fonts,
            );
            let automatic = rect(&document, &page, "automatic");
            let literal = rect(&document, &page, "literal");
            let font = spec(&fonts, zoom);
            let expected = font.measure("A") + font.measure(" ").max(1) + font.measure("o");
            assert_eq!(automatic.width, expected, "width={width}, zoom={zoom}");
            let actual = runs(&page, automatic);
            let control = runs(&page, literal);
            assert_eq!(
                actual
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<Vec<_>>(),
                ["A", "o"]
            );
            assert_eq!(
                control
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<Vec<_>>(),
                ["A", "o"]
            );
            assert_eq!(
                actual[0].y, actual[1].y,
                "Auto width must fit its full collapsed line"
            );
            assert_eq!(
                actual[1].x - actual[0].x,
                font.measure("A") + font.measure(" ").max(1)
            );
            assert_eq!(
                actual.iter().map(|run| run.width()).collect::<Vec<_>>(),
                control.iter().map(|run| run.width()).collect::<Vec<_>>()
            );
            assert_eq!(automatic.height, literal.height);
            assert_eq!(
                paint_text(&page, automatic),
                paint_text(&page, literal),
                "zoom={zoom}"
            );
        }
    }
}

#[test]
fn absolute_caption_intrinsic_keeps_constrained_explicit_and_nowrap_width_rules() {
    let fonts = fonts();
    let (document, page) = page(
        "<div id=parent><p id=constrained class=caption>A o</p></div>\
         <p id=narrow class=caption>A o</p><p id=fit class=caption>A o</p>\
         <p id=nowrap class=caption>A o</p>",
        "#parent{position:absolute;left:20px;top:20px;width:25px;height:64px}\
         #constrained{left:0;top:0}#narrow{left:160px;width:25px}\
         #fit{left:300px;width:26px}#nowrap{left:20px;top:110px;width:25px;white-space:nowrap}",
        640,
        1.0,
        &fonts,
    );
    for name in ["constrained", "narrow"] {
        let r = rect(&document, &page, name);
        let text = runs(&page, r);
        assert_eq!((r.width, r.height), (25, 48), "{name}");
        assert_eq!(text.len(), 2);
        assert_eq!(text[1].y - text[0].y, 24, "{name} must still wrap");
    }
    let fit = rect(&document, &page, "fit");
    assert_eq!((fit.width, fit.height), (26, 24));
    let text = runs(&page, fit);
    assert_eq!(text[0].y, text[1].y, "Exact standalone-word sum fits");
    let nowrap = rect(&document, &page, "nowrap");
    let text = runs(&page, nowrap);
    assert_eq!((nowrap.width, nowrap.height), (25, 24));
    assert_eq!(text[0].y, text[1].y);
    assert_eq!(
        text[1].x + text[1].width(),
        nowrap.x + 26,
        "Explicit nowrap may overflow"
    );
}

#[test]
fn absolute_caption_intrinsic_preserves_collapsed_and_preserved_whitespace_modes() {
    let fonts = fonts();
    let (document, page) = page(
        "<p id=normal class=caption>A o</p><p id=collapsed class=caption>  A \t\r\n\u{000c} o  </p>\
         <p id=nowrap class=caption>A o</p><p id=nbsp class=caption>A&nbsp;o</p>\
         <p id=pre class=caption>A o</p><p id=prewrap class=caption>A o</p>",
        "#collapsed{left:160px}#nowrap{left:300px;white-space:nowrap}\
         #nbsp{top:110px}#pre{left:160px;top:110px;white-space:pre}\
         #prewrap{left:300px;top:110px;white-space:pre-wrap}",
        640,
        1.0,
        &fonts,
    );
    let normal = rect(&document, &page, "normal");
    for name in ["normal", "collapsed", "nowrap"] {
        let r = rect(&document, &page, name);
        let text = runs(&page, r);
        assert_eq!((r.width, r.height), (26, 24), "{name}");
        assert_eq!(
            text.iter().map(|run| run.text.as_str()).collect::<Vec<_>>(),
            ["A", "o"]
        );
        assert_eq!(text[1].x - text[0].x, 16);
        assert_eq!(text[1].separator_before, " ");
        assert_eq!(paint_text(&page, r), paint_text(&page, normal), "{name}");
    }
    let nonbreaking = rect(&document, &page, "nbsp");
    let text = runs(&page, nonbreaking);
    assert_eq!(text.len(), 1);
    assert_eq!(text[0].text, "A\u{a0}o");
    assert_eq!(nonbreaking.width, spec(&fonts, 1.0).measure("A\u{a0}o"));
    for name in ["pre", "prewrap"] {
        let r = rect(&document, &page, name);
        let text = runs(&page, r);
        assert_eq!(
            (r.width, r.height),
            (25, 24),
            "Continuous preserved-text pen stays unchanged"
        );
        assert_eq!(text.len(), 1);
        assert_eq!(text[0].text, "A o");
        assert_eq!(text[0].width(), 25);
    }
}

#[test]
fn absolute_caption_intrinsic_preserves_kerning_fallback_and_inline_block_sizing() {
    let fonts = fonts();
    let font = spec(&fonts, 1.0);
    let (document, layout) = page(
        "<p id=kern class=caption>AV</p><div id=inline>A o</div>",
        "#inline{display:inline-block;margin-top:100px}",
        640,
        1.0,
        &fonts,
    );
    let kern = rect(&document, &layout, "kern");
    assert!(font.measure("AV") < font.measure("A") + font.measure("V"));
    assert_eq!(
        kern.width,
        font.measure("AV"),
        "Do not sum independently rounded characters"
    );
    assert_eq!(
        rect(&document, &layout, "inline").width,
        26,
        "The shared intrinsic path also sizes inline-blocks"
    );
    let (document, fallback) = page(
        "<p id=fallback class=caption>A o</p>",
        "",
        640,
        1.0,
        &Book::empty(),
    );
    let r = rect(&document, &fallback, "fallback");
    assert_eq!(r.width, 36);
    let text = runs(&fallback, r);
    assert_eq!(text.len(), 2);
    assert_eq!(text[0].y, text[1].y);
    assert!(text.iter().all(|run| run.shaped.is_none()));
}
