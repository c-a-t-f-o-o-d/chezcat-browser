//! Original ordinary uniform-border pixel, geometry and input-region controls.
//! No native desktop, framework replacement or rounded hit/overflow claim.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!("chezcat-rounded-border-{}-{}.html",
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
        assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0, "{}", app.scripting.as_ref().unwrap().report());
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
    app.page.border_rects[id(app, name)].expect("A real measured border box")
}
fn pixel(app: &App, canvas: &Canvas, x: i32, y: i32) -> u32 {
    let y = y + app.content_top() - app.scroll;
    assert!(x >= 0 && y >= 0 && (x as usize) < canvas.width && (y as usize) < canvas.height);
    canvas.pixels[y as usize * canvas.width + x as usize]
}

#[test]
fn rounded_border_actual_app_uniform_cards_adjacent_seams_origin_geometry_and_hits() {
    let fixture = Fixture::new(r#"<!doctype html><html><head><style>
html,body{margin:0;padding:0;background:white}
:root{--edge:1px solid rgba(0,0,0,.175);--radius:.375rem}
#origin{width:32px;height:4px;background:magenta}
.card{display:block;position:absolute;box-sizing:border-box;width:80px;height:40px;background:white;border:var(--edge);border-radius:var(--radius)}
#a{left:20px;top:20px}#b{left:100px;top:20px}#c{left:20px;top:100px}#d{left:20px;top:140px}
#square{left:220px;top:20px;border:1px solid rgba(0,0,0,.175);border-radius:0}
</style></head><body><div id=origin></div><a id=a class=card href=#a-target></a><a id=b class=card href=#b-target></a>
<div id=c class=card></div><div id=d class=card></div><div id=square class=card></div></body></html>"#);
    for width in [640, 1280] {
        let app = fixture.open(width);
        let canvas = app.draw();
        let origin = rect(&app, "origin");
        assert_eq!((origin.x, origin.y, origin.width, origin.height), (0, 0, 32, 4));
        assert_eq!(pixel(&app, &canvas, 0, 0), 0xff00ff);
        let square = rect(&app, "square");
        assert_eq!((square.x, square.y, square.width, square.height), (220, 20, 80, 40));
        assert_eq!(pixel(&app, &canvas, 220, 20), 0xd2d2d2, "Independent square stroke control");
        assert_eq!(pixel(&app, &canvas, 260, 40), 0xffffff);
        for (name, x, y) in [("a", 20, 20), ("b", 100, 20), ("c", 20, 100), ("d", 20, 140)] {
            let r = rect(&app, name);
            assert_eq!((r.x, r.y, r.width, r.height), (x, y, 80, 40));
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (79, 0), (78, 0), (79, 1),
                (0, 39), (1, 39), (0, 38), (79, 39), (78, 39), (79, 38)] {
                assert_eq!(pixel(&app, &canvas, x + dx, y + dy), 0xffffff, "{name}: fully outside 6px corner ({dx},{dy})");
            }
            for (dx, dy) in [(40, 0), (40, 39), (0, 20), (79, 20)] {
                assert_eq!(pixel(&app, &canvas, x + dx, y + dy), 0xd2d2d2);
            }
        }
        for (x, y) in [(99, 40), (100, 40), (60, 139), (60, 140)] {
            assert_eq!(pixel(&app, &canvas, x, y), 0xd2d2d2, "Adjoining strokes composite once on separate pixels");
        }
        for (x, y) in [(19, 40), (180, 40), (60, 99), (60, 180)] {
            assert_eq!(pixel(&app, &canvas, x, y), 0xffffff);
        }
        assert_eq!(app.page.link_node_at(60, 40), Some(id(&app, "a")));
        assert_eq!(app.page.link_node_at(140, 40), Some(id(&app, "b")));
        assert_eq!(app.page.link_node_at(99, 40), Some(id(&app, "a")));
        assert_eq!(app.page.link_node_at(100, 40), Some(id(&app, "b")));
        assert_eq!(app.page.link_node_at(19, 40), None);
        assert!(app.forms.controls.is_empty());
    }
}

#[test]
fn rounded_border_actual_app_nonwhite_boundary_and_transparent_background_fallback() {
    let fixture = Fixture::new(r#"<!doctype html><html><head><style>
html,body{margin:0;padding:0;background:#c86432}
.box{display:block;position:absolute;box-sizing:border-box;top:20px;width:80px;height:40px;border-radius:8px}
#actual{left:20px;background:#64c832;border:4px solid rgba(0,0,0,.2)}
#equal{left:120px;background:#1464dc;border:4px solid rgba(20,100,220,.5)}
#reference{left:220px;background:#1464dc;border:0}
.under,.clear{display:block;position:absolute;box-sizing:border-box;top:100px;width:80px;height:40px}
.under{background:#336699}#under,#clear{left:20px}#under2,#clear2{left:120px}
.clear{border:4px solid rgba(0,0,0,.2)}#clear{border-radius:8px}#clear2{border-radius:0}
</style></head><body><div id=actual class=box></div><div id=equal class=box></div><div id=reference class=box></div>
<a id=under class=under href=#one></a><a id=under2 class=under href=#two></a><div id=clear class=clear></div><div id=clear2 class=clear></div></body></html>"#);
    let app = fixture.open(640);
    let canvas = app.draw();
    for name in ["actual", "equal", "reference", "clear", "clear2"] {
        let r = rect(&app, name);
        assert_eq!((r.width, r.height), (80, 40), "{name}");
    }
    assert_eq!(pixel(&app, &canvas, 20, 20), 0xc86432);
    assert_eq!(pixel(&app, &canvas, 60, 20), 0x50a028, "The CSS background continues beneath a translucent border");
    assert_eq!(pixel(&app, &canvas, 60, 40), 0x64c832);
    assert_eq!(pixel(&app, &canvas, 22, 22), 0x718f2b,
        "Known partial outer coverage composites the colored material over the actual nonwhite destination once");
    let mut partial = false;
    for y in 0..40 { for x in 0..80 {
        let expected = pixel(&app, &canvas, 220 + x, 20 + y);
        assert_eq!(pixel(&app, &canvas, 120 + x, 20 + y), expected,
            "Equal RGB background/stroke must remain invariant under border alpha, including antialiased edges");
        partial |= expected != 0x1464dc && expected != 0xc86432;
        assert_eq!(pixel(&app, &canvas, 20 + x, 100 + y), pixel(&app, &canvas, 120 + x, 100 + y),
            "Default transparent backgrounds retain the existing square fallback");
    }}
    assert!(partial, "The invariant must exercise actual partial boundary coverage");
    assert_eq!(pixel(&app, &canvas, 20, 100), 0x29527a);
    assert_eq!(pixel(&app, &canvas, 60, 120), 0x336699);
    assert_eq!(app.page.link_node_at(60, 120), Some(id(&app, "under")), "No full-box hit occluder may be introduced for a transparent background");
    assert_eq!(app.page.link_node_at(160, 120), Some(id(&app, "under2")));
}

#[test]
fn rounded_border_actual_app_final_flex_size_translation_clip_and_live_zero() {
    let fixture = Fixture::new(r#"<!doctype html><html><head><style>
html,body{margin:0;padding:0;background:white}
#clip{position:absolute;left:20px;top:20px;width:60px;height:50px;overflow:hidden}
#moved{display:block;box-sizing:border-box;width:80px;height:40px;background:#64c832;border:2px solid rgba(0,0,0,.2);border-radius:8px;transform:translate(12px,5px)}
#moved.square{border-radius:0}
#row{position:absolute;left:120px;top:20px;display:flex;align-items:stretch;width:80px;height:60px}
#stretch{box-sizing:border-box;width:30px;background:#64c832;border:2px solid rgba(0,0,0,.2);border-radius:1000px}
</style></head><body><div id=clip><a id=moved href=#target></a></div><div id=row><div id=stretch></div></div></body></html>"#);
    let mut app = fixture.open(640);
    let check_geometry = |app: &App| {
        let r = rect(app, "moved");
        assert_eq!((r.x, r.y, r.width, r.height), (20, 20, 80, 40));
        let visual = app.page.visual_border_rects[id(app, "moved")].unwrap();
        assert_eq!((visual.x, visual.y), (32.0, 25.0));
        let r = rect(app, "stretch");
        assert_eq!((r.x, r.y, r.width, r.height), (120, 20, 30, 60));
    };
    check_geometry(&app);
    let canvas = app.draw();
    assert_eq!(pixel(&app, &canvas, 32, 25), 0xffffff);
    assert_eq!(pixel(&app, &canvas, 72, 25), 0x50a028);
    assert_eq!(pixel(&app, &canvas, 72, 45), 0x64c832);
    assert_eq!(pixel(&app, &canvas, 80, 45), 0xffffff);
    assert_eq!(app.page.link_node_at(72, 45), Some(id(&app, "moved")));
    assert_eq!(app.page.link_node_at(80, 45), None);
    assert_eq!(pixel(&app, &canvas, 120, 20), 0xffffff);
    assert_eq!(pixel(&app, &canvas, 120, 50), 0x50a028, "Use a fully covered side midpoint, not the rounded apex");
    assert_eq!(pixel(&app, &canvas, 135, 50), 0x64c832);
    let outcome = app.scripting.as_mut().unwrap().console_eval(&mut app.document, &mut app.forms,
        "document.getElementById('moved').className='square'");
    assert_eq!(outcome.errors, 0, "{}", app.scripting.as_ref().unwrap().report());
    assert!(outcome.layout_changed);
    app.reflow();
    check_geometry(&app);
    let canvas = app.draw();
    assert_eq!(pixel(&app, &canvas, 32, 25), 0x50a028, "Zero radius returns to the unchanged square path");
    assert_eq!(pixel(&app, &canvas, 72, 45), 0x64c832);
    assert_eq!(pixel(&app, &canvas, 80, 45), 0xffffff);
}
