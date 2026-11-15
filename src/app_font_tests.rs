use super::*;
use std::sync::Arc;

fn app(markup: &str) -> App {
    let mut app = App::new();
    app.base_fonts
        .add(
            "Geometry".into(),
            400,
            false,
            Arc::new(crate::font::Font::parse(&crate::font::tests::fixture_font()).unwrap()),
        )
        .unwrap();
    app.fonts = app.base_fonts.clone();
    app.document = html::parse(&format!("<html><body>{markup}</body></html>"));
    app.forms = Forms::new(&app.document);
    app.reflow();
    app
}

#[test]
fn page_font_metrics_drive_pointer_selection_find_and_zoom() {
    let mut app =
        app("<style>body{margin:10px;font:40px Geometry}p{margin:0}</style><p>AV oé AV</p>");
    let index = app
        .page
        .runs
        .iter()
        .position(|run| run.text == "AV")
        .unwrap();
    let run = &app.page.runs[index];
    let shape = run.shaped.as_ref().unwrap();
    let (x, y, width, first) = (run.x, run.y, run.width(), shape.x(1));
    assert!(first < GLYPH_ADVANCE * run.scale);
    assert_eq!(
        app.point_at(x + first, y + app.content_top() + 5),
        Some(TextPoint {
            run: index,
            offset: 1
        })
    );
    app.selection = Some(Selection {
        start: TextPoint {
            run: index,
            offset: 0,
        },
        end: TextPoint {
            run: index,
            offset: 1,
        },
    });
    assert_eq!(app.selected_text(), "A");
    let canvas = app.draw();
    let highlight_y = (y + app.content_top() - 1) as usize;
    assert_eq!(
        canvas.pixels[highlight_y * canvas.width + x as usize],
        0xb4d8fa
    );
    assert_ne!(
        canvas.pixels[highlight_y * canvas.width + (x + first) as usize],
        0xb4d8fa
    );
    app.find.set("AV");
    app.find_open = true;
    app.update_find(false);
    assert_eq!(app.matches.len(), 2);
    app.set_zoom(1.5);
    let larger = app.page.runs.iter().find(|run| run.text == "AV").unwrap();
    assert!((larger.width() - (width as f32 * 1.5).round() as i32).abs() <= 1);
}

#[test]
fn wrapping_removes_cross_line_kerning_and_preserves_fractional_advances() {
    let wrapped = app("<style>body{font:100px Geometry}p{width:124px;margin:0}</style><p>oAV</p>");
    assert!(wrapped.page.runs.iter().all(|run| run.width() <= 124));
    assert!(!wrapped.page.runs.iter().any(|run| run.text == "oA"));
    let preserved = app("<style>body{font:20px Geometry}p{width:26px;white-space:pre-wrap;margin:0}</style><p>AV</p>");
    assert_eq!(preserved.page.runs.len(), 1);
    assert_eq!(preserved.page.runs[0].text, "AV");
    assert_eq!(preserved.page.runs[0].width(), 26);
}

#[test]
fn proportional_input_caret_scrolling_and_paint_use_the_same_positions() {
    let mut app = app(
        "<style>body{font:40px Geometry}input{width:92px}</style><input id=q value='AVAVAVAV'>",
    );
    let node = app.page.controls[0].node;
    let rect = &app.page.controls[0];
    let shape = rect.text_font.as_ref().unwrap().shape("AVAVAVAV");
    let x = rect.x + 6 + shape.x(1);
    let y = rect.y + app.content_top() + rect.height / 2;
    app.pointer_down(x, y, false);
    app.resize(app.width, app.height);
    app.pointer_up(x, y);
    assert_eq!(app.forms.controls[&node].value.cursor, 1);
    app.forms
        .controls
        .get_mut(&node)
        .unwrap()
        .value
        .move_to(8, false);
    app.sync_editors();
    let value = &app.forms.controls[&node].value;
    assert!(value.offset > 0);
    let rect = &app.page.controls[0];
    assert!(shape.x(value.cursor) - shape.x(value.offset) <= rect.width - 14);
    let mut canvas = Canvas::new(300, 100);
    canvas.fill_rect(0, 0, 300, 100, 0xabcdef);
    let clip = crate::paint::ClipRect::new(0, 0, 300, 100);
    canvas.set_clip(Some(clip));
    draw_control(
        &mut canvas,
        rect,
        10,
        &app.forms.controls[&node],
        "",
        true,
        None,
    );
    assert_eq!(canvas.clip(), clip);
    for (index, pixel) in canvas.pixels.iter().enumerate() {
        let (x, y) = ((index % canvas.width) as i32, (index / canvas.width) as i32);
        if x < rect.x || x >= rect.x + rect.width || y < 10 || y >= 10 + rect.height {
            assert_eq!(*pixel, 0xabcdef);
        }
    }
}
