//! Original real-App custom-property geometry, opaque paint and link-hit gates.
//! No framework source, network probe, reference renderer or physical input.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "chezcat-css-variable-app-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("page.html"), r#"<!doctype html><html><head><title>Variable box pending</title><style>
html,body{margin:0;padding:0;background:#ffffff}
#theme{--Pad:6px 10px;--pad:99px;--Stroke:2px solid #003366}
#theme.changed{--Pad:8px 12px;--Stroke:3px solid #003366}
.panel{position:absolute;top:20px;width:80px;height:30px;box-sizing:content-box;background:#f0e0c0}
#variable{left:20px;padding:var(--Pad);border:var(--Stroke)}
#literal{left:180px;padding:6px 10px;border:2px solid #003366}
a{display:block;width:20px;height:10px;background:#cc3300}
</style></head><body><div id=theme><div id=variable class=panel><a id=variable-link href=#destination></a></div></div><div id=literal class=panel><a id=literal-link href=#destination></a></div><div id=destination></div><script>document.title='Variable box ready';</script></body></html>"#).unwrap();
        Self(directory)
    }
    fn open(&self) -> App {
        let mut app = App::new();
        app.resize(640, 480);
        app.set_javascript_enabled(true);
        app.open(&self.0.join("page.html"));
        assert!(!app.failed && !app.is_loading(), "{}", app.status);
        assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0, "{:?}", app.events);
        assert_eq!(app.document.title, "Variable box ready");
        app
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

fn id(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len()).find(|&node| app.document.attr(node, "id") == Some(name)).unwrap()
}
fn box_rect(app: &App, name: &str) -> crate::layout::BorderRect {
    app.page.border_rects[id(app, name)].expect("Original visible node must own a real border box")
}
fn pixel(app: &App, canvas: &Canvas, x: i32, y: i32) -> u32 {
    let y = app.content_top() + y - app.scroll;
    assert!(x >= 0 && y >= 0 && (x as usize) < canvas.width && (y as usize) < canvas.height);
    canvas.pixels[y as usize * canvas.width + x as usize]
}

fn assert_twins(app: &App, width: i32, height: i32, inset_x: i32, inset_y: i32) {
    let variable = box_rect(app, "variable");
    let literal = box_rect(app, "literal");
    // The independent literal control is checked before the variable result.
    assert_eq!((literal.x, literal.y, literal.width, literal.height), (180, 20, width, height));
    assert_eq!((variable.x, variable.y, variable.width, variable.height), (20, 20, width, height));
    let canvas = app.draw();
    for (name, panel) in [("variable-link", variable), ("literal-link", literal)] {
        let link = box_rect(app, name);
        assert_eq!((link.x - panel.x, link.y - panel.y, link.width, link.height), (inset_x, inset_y, 20, 10));
        for (x, y) in [(link.x, link.y), (link.x + 10, link.y + 5), (link.x + 19, link.y + 9)] {
            assert_eq!(app.page.link_node_at(x, y), Some(id(app, name)));
            assert_eq!(app.page.link_at(x, y), Some("#destination"));
            assert_eq!(pixel(app, &canvas, x, y), 0xcc3300);
        }
        for (x, y) in [(panel.x + 3, panel.y + 3), (link.x + 20, link.y + 5), (link.x + 10, link.y + 10)] {
            assert_eq!(app.page.link_node_at(x, y), None, "Padding/content outside the anchor cannot become a hit");
        }
        assert_eq!(pixel(app, &canvas, panel.x, panel.y), 0x003366);
        assert_eq!(pixel(app, &canvas, panel.x + 3, panel.y + 3), 0xf0e0c0);
    }
    // Compare the entire opaque box and a two-pixel white exterior. No text or
    // antialiasing can accidentally satisfy a sparse color-presence oracle.
    for y in -2..height + 2 {
        for x in -2..width + 2 {
            let variable_pixel = pixel(app, &canvas, variable.x + x, variable.y + y);
            let literal_pixel = pixel(app, &canvas, literal.x + x, literal.y + y);
            assert_eq!(variable_pixel, literal_pixel, "Relative box pixel ({x},{y})");
            if x < 0 || y < 0 || x >= width || y >= height {
                assert_eq!(variable_pixel, 0xffffff, "Outside the actual box at ({x},{y})");
            }
        }
    }
}

#[test]
fn css_variables_actual_app_inherited_box_pixels_hits_and_live_reflow_match_literal_twin() {
    let fixture = Fixture::new();
    let mut app = fixture.open();
    assert_twins(&app, 104, 46, 12, 8);
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    let nodes = app.document.nodes.as_ptr();
    // A supported DOM class mutation changes the inherited custom properties;
    // ordinary style setters update the independent literal control and width.
    let outcome = app.scripting.as_mut().unwrap().console_eval(&mut app.document, &mut app.forms, r#"
document.getElementById('theme').className='changed';
document.getElementById('variable').style.width='90px';
var literal=document.getElementById('literal');
literal.style.width='90px';literal.style.padding='8px 12px';literal.style.border='3px solid #003366';
document.title='Variable box changed';
"#);
    assert_eq!(outcome.errors, 0, "{}", app.scripting.as_ref().unwrap().report());
    assert!(outcome.layout_changed, "Live style/class mutation must invalidate actual layout");
    app.reflow();
    assert_eq!(app.document.title, "Variable box changed");
    assert_twins(&app, 120, 52, 15, 11);
    assert_eq!(app.page.link_node_at(32, 28), None, "The old anchor inset must not retain a stale hit");
    assert_eq!(app.document.nodes.as_ptr(), nodes);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
}
