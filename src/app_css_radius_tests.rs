//! Original ordinary-box radius pixel reductions. No native input, framework,
//! network, reference renderer or claim of rounded borders/descendant clipping.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

// Byte-exact source of the frozen EC two-width headless RED fixture.
const ORIGINAL: &str = r##"<!doctype html>
<html><head><meta charset="utf-8"><title>Original radius baseline</title>
<style>
html { background:#ffffff; }
body { margin:0; padding:0; background:#ffffff; color:#000000; font-size:14px; }
#origin { display:block; margin:0; padding:0; border:0; width:32px; height:4px; background:#ff00ff; }
.box { display:block; width:80px; height:40px; margin:16px 0 0 24px; padding:0; border:0; background:#1464dc; }
#literal { border-radius:8px; }
#variable { --r:8px; border-radius:var(--r); }
#square { border-radius:0; }
.metric { margin:8px 0 0 24px; }
</style></head><body>
<div id="origin"></div>
<div class="box" id="literal"></div>
<div class="box" id="variable"></div>
<div class="box" id="square"></div>
<p class="metric" id="m-origin">ORIGIN_PENDING</p>
<p class="metric" id="m-literal">LITERAL_PENDING</p>
<p class="metric" id="m-variable">VARIABLE_PENDING</p>
<p class="metric" id="m-square">SQUARE_PENDING</p>
<script>
function reportBox(id) {
  var rect = document.getElementById(id).getBoundingClientRect();
  document.getElementById('m-' + id).textContent = 'MEASURE ' + id + ' ' +
    rect.x + ' ' + rect.y + ' ' + rect.width + ' ' + rect.height;
}
reportBox('origin');
reportBox('literal');
reportBox('variable');
reportBox('square');
</script></body></html>
"##;

struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!("chezcat-radius-{}-{}.html",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::write(&path, source).unwrap();
        Self(path)
    }

    fn open(&self, width: i32) -> App {
        let mut app = App::new();
        app.resize(width, 480);
        app.set_javascript_enabled(true);
        app.open(&self.0);
        assert!(!app.failed && !app.is_loading(), "{}", app.status);
        assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0, "{:?}", app.events);
        app
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
}

fn id(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len()).find(|&node| app.document.attr(node, "id") == Some(name)).unwrap()
}
fn rect(app: &App, name: &str) -> crate::layout::BorderRect {
    app.page.border_rects[id(app, name)].expect("Original node has a real border box")
}
fn pixel(app: &App, canvas: &Canvas, x: i32, y: i32) -> u32 {
    let y = app.content_top() + y - app.scroll;
    assert!(x >= 0 && y >= 0 && (x as usize) < canvas.width && (y as usize) < canvas.height);
    canvas.pixels[y as usize * canvas.width + x as usize]
}
fn corners(app: &App, canvas: &Canvas, name: &str, color: u32) {
    let r = rect(app, name);
    for dx in [0, r.width - 2] { for dy in [0, r.height - 2] {
        for x in 0..2 { for y in 0..2 {
            assert_eq!(pixel(app, canvas, r.x + dx + x, r.y + dy + y), color,
                "{name}: relative corner ({},{})", dx + x, dy + y);
        }}
    }}
}

#[test]
fn css_radius_actual_app_original_two_width_pixels_and_live_zero() {
    let fixture = Fixture::new(ORIGINAL);
    // Keep the original baseline bytes unchanged. A separate live reduction
    // adds one initially inactive author rule, using the same supported class
    // mutation path as the existing custom-property App regression.
    let live_source = ORIGINAL.replace("</style>", "#variable.zero { --r:0; }\n</style>");
    let live_fixture = Fixture::new(&live_source);
    for width in [640, 1280] {
        let app = fixture.open(width);
        let canvas = app.draw();
        assert_eq!(pixel(&app, &canvas, 0, 0), 0xff00ff, "Actual page origin witness");
        for (name, y) in [("literal", 20), ("variable", 76), ("square", 132)] {
            let r = rect(&app, name);
            assert_eq!((r.x, r.y, r.width, r.height), (24, y, 80, 40), "{width}: {name}");
            assert_eq!(pixel(&app, &canvas, r.x + 40, r.y + 20), 0x1464dc);
            for (x, y) in [(r.x - 1, r.y + 20), (r.x + 80, r.y + 20),
                (r.x + 40, r.y - 1), (r.x + 40, r.y + 40)] {
                assert_eq!(pixel(&app, &canvas, x, y), 0xffffff);
            }
        }
        corners(&app, &canvas, "square", 0x1464dc);
        corners(&app, &canvas, "literal", 0xffffff);
        corners(&app, &canvas, "variable", 0xffffff);
        for y in -2..42 { for x in -2..82 {
            assert_eq!(pixel(&app, &canvas, 24 + x, 20 + y), pixel(&app, &canvas, 24 + x, 76 + y));
        }}
        let mut app = live_fixture.open(width);
        let canvas = app.draw();
        corners(&app, &canvas, "variable", 0xffffff);
        corners(&app, &canvas, "literal", 0xffffff);
        corners(&app, &canvas, "square", 0x1464dc);
        let geometry = |app: &App| ["literal", "variable", "square"].map(|name| {
            let r = rect(app, name);
            (r.x, r.y, r.width, r.height)
        });
        let expected = [(24, 20, 80, 40), (24, 76, 80, 40), (24, 132, 80, 40)];
        assert_eq!(geometry(&app), expected);
        let outcome = app.scripting.as_mut().unwrap().console_eval(&mut app.document, &mut app.forms,
            "document.getElementById('variable').className='box zero'");
        assert_eq!(outcome.errors, 0, "{}", app.scripting.as_ref().unwrap().report());
        assert!(outcome.layout_changed);
        app.reflow();
        assert_eq!(geometry(&app), expected);
        let canvas = app.draw();
        corners(&app, &canvas, "variable", 0x1464dc);
        corners(&app, &canvas, "literal", 0xffffff);
        corners(&app, &canvas, "square", 0x1464dc);
        assert_eq!(pixel(&app, &canvas, 64, 96), 0x1464dc);
        assert!(app.forms.controls.is_empty());
        assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
    }
}

#[test]
fn css_radius_actual_app_final_size_translation_clip_and_existing_fallbacks() {
    let fixture = Fixture::new(r#"<!doctype html><html><head><style>
html,body{margin:0;padding:0;background:white}
#clip{position:absolute;left:20px;top:20px;width:60px;height:50px;overflow:hidden}
#moved{width:80px;height:40px;background:#1464dc;border-radius:8px;transform:translate(12px,5px)}
#row{position:absolute;left:120px;top:20px;display:flex;align-items:stretch;width:80px;height:60px}
#stretch{width:30px;background:#1464dc;border-radius:1000px}
.bordered{position:absolute;top:20px;width:40px;height:24px;border:2px solid red;background:#1464dc}
#bordered{left:220px;border-radius:8px}#plain{left:280px;border-radius:0}
#unsupported{position:absolute;left:340px;top:20px;width:40px;height:24px;background:#1464dc;border-radius:50%}
#parent{position:absolute;left:340px;top:100px;width:40px;height:24px;background:#1464dc;border-radius:8px;overflow:hidden}
#child{width:3px;height:3px;background:red}
</style></head><body><div id=clip><div id=moved></div></div><div id=row><div id=stretch></div></div>
<div id=bordered class=bordered></div><div id=plain class=bordered></div><div id=unsupported></div>
<div id=parent><div id=child></div></div></body></html>"#);
    let app = fixture.open(640);
    let moved = rect(&app, "moved");
    assert_eq!((moved.x, moved.y, moved.width, moved.height), (20, 20, 80, 40), "Raw offset geometry is unchanged");
    let visual = app.page.visual_border_rects[id(&app, "moved")].unwrap();
    assert_eq!((visual.x, visual.y), (32.0, 25.0));
    let canvas = app.draw();
    assert_eq!(pixel(&app, &canvas, 32, 25), 0xffffff);
    assert_eq!(pixel(&app, &canvas, 72, 45), 0x1464dc);
    assert_eq!(pixel(&app, &canvas, 80, 45), 0xffffff, "The ancestor's existing rectangle clip still applies");
    let stretch = rect(&app, "stretch");
    assert_eq!((stretch.x, stretch.y, stretch.width, stretch.height), (120, 20, 30, 60));
    assert_eq!(pixel(&app, &canvas, 120, 20), 0xffffff);
    assert_eq!(pixel(&app, &canvas, 135, 50), 0x1464dc);
    let bordered = rect(&app, "bordered");
    let plain = rect(&app, "plain");
    assert_eq!((bordered.width, bordered.height), (plain.width, plain.height));
    for y in 0..plain.height { for x in 0..plain.width {
        assert_eq!(pixel(&app, &canvas, bordered.x + x, bordered.y + y),
            pixel(&app, &canvas, plain.x + x, plain.y + y), "Border painting retains its square fallback");
    }}
    corners(&app, &canvas, "unsupported", 0x1464dc);
    assert_eq!(pixel(&app, &canvas, 340, 100), 0xff0000, "Radius does not introduce descendant clipping");
}

#[test]
fn css_radius_actual_app_relative_length_and_zoom_keep_real_box_geometry() {
    let fixture = Fixture::new(r#"<!doctype html><html><head><style>
html{font-size:20px}body{margin:0;padding:0;background:white}
.box{position:absolute;top:20px;width:80px;height:40px;background:#1464dc}
#literal{left:20px;border-radius:8px}#relative{left:140px;border-radius:.4rem}
</style></head><body><div id=literal class=box></div><div id=relative class=box></div></body></html>"#);
    let mut app = fixture.open(640);
    for zoom in [1.0, 2.0] {
        app.set_zoom(zoom);
        let a = rect(&app, "literal");
        let b = rect(&app, "relative");
        assert_eq!((a.width, a.height), ((80.0 * zoom) as i32, (40.0 * zoom) as i32));
        assert_eq!((a.width, a.height), (b.width, b.height));
        let canvas = app.draw();
        corners(&app, &canvas, "literal", 0xffffff);
        corners(&app, &canvas, "relative", 0xffffff);
        for y in 0..a.height { for x in 0..a.width {
            assert_eq!(pixel(&app, &canvas, a.x + x, a.y + y), pixel(&app, &canvas, b.x + x, b.y + y));
        }}
    }
}
