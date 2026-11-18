//! Original real-App theme-rule reduction with an independent literal control.
//! Border radius remains outside this test; bordered corners stay square.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!("chezcat-root-theme-{}-{}.html",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::write(&path, r#"<!doctype html><html><head><title>Original root theme</title><style>
html,body{margin:0;padding:0;background:#ffffff}
:root,[data-bs-theme=light]{--bs-border-width:1px;--bs-border-color-translucent:rgba(0,0,0,.175);--bs-body-color:#212529}
:root.changed{--bs-border-width:2px;--bs-body-color:#445566}
.panel{display:block;position:absolute;top:20px;width:80px;height:40px;margin:0;padding:0;background:white;font-size:14px}
#variable{left:20px;border:var(--bs-border-width) solid var(--bs-border-color-translucent);color:var(--bs-body-color)}
#literal{left:160px;border:1px solid rgba(0,0,0,.175);color:#212529}
#literal.changed{border-width:2px;color:#445566}
</style></head><body><div id=variable class=panel>Theme</div><div id=literal class=panel>Theme</div></body></html>"#).unwrap();
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
    app.page.border_rects[id(app, name)].unwrap()
}
fn pixel(app: &App, canvas: &Canvas, x: i32, y: i32) -> u32 {
    let y = y + app.content_top() - app.scroll;
    assert!(x >= 0 && y >= 0 && (x as usize) < canvas.width && (y as usize) < canvas.height);
    canvas.pixels[y as usize * canvas.width + x as usize]
}
fn assert_twins(app: &App, edge: i32, foreground: u32) {
    let literal = rect(app, "literal");
    let variable = rect(app, "variable");
    let canvas = app.draw();
    let (width, height) = (80 + 2 * edge, 40 + 2 * edge);
    // Independent literal geometry/pixels qualify the new theme-rule result.
    assert_eq!((literal.x, literal.y, literal.width, literal.height), (160, 20, width, height));
    assert_eq!(pixel(app, &canvas, literal.x, literal.y), 0xd2d2d2, "Border coverage is composited once");
    assert_eq!(pixel(app, &canvas, literal.x + 60, literal.y + 20), 0xffffff);
    assert_eq!(pixel(app, &canvas, literal.x - 1, literal.y + 20), 0xffffff);
    assert_eq!((variable.x, variable.y, variable.width, variable.height), (20, 20, width, height));
    for y in -1..height + 1 { for x in -1..width + 1 {
        assert_eq!(pixel(app, &canvas, variable.x + x, variable.y + y),
            pixel(app, &canvas, literal.x + x, literal.y + y), "Theme/literal pixel ({x},{y})");
    }}
    let runs: Vec<_> = app.page.runs.iter().filter(|run| run.text == "Theme").collect();
    assert_eq!(runs.len(), 2);
    assert!(runs.iter().all(|run| run.color == foreground));
}

#[test]
fn css_root_actual_app_grouped_theme_border_foreground_and_live_class_match_literal() {
    let fixture = Fixture::new();
    for width in [640, 1280] {
        let mut app = fixture.open(width);
        assert_twins(&app, 1, 0x212529);
        let outcome = app.scripting.as_mut().unwrap().console_eval(&mut app.document, &mut app.forms, r#"
var root=document.documentElement,body=document.body,target=document.getElementById('variable');
if(document.querySelector(':root')!==root||document.querySelectorAll(':root').length!==1)throw 'document root query';
if(root.querySelector(':root')!==null||body.querySelector(':root')!==null)throw 'element query excludes its root';
if(root.querySelector(':root > body')!==body||body.querySelector(':root #variable')!==target)throw 'real ancestor root query';
var detached=document.createElement('div'),child=document.createElement('span');detached.appendChild(child);
if(detached.querySelector(':root')!==null||detached.querySelector(':root span')!==null)throw 'detached subtree is not a document root';
root.className='changed';document.getElementById('literal').className='panel changed';
"#);
        assert_eq!(outcome.errors, 0, "{}", app.scripting.as_ref().unwrap().report());
        assert!(outcome.layout_changed);
        app.reflow();
        assert_twins(&app, 2, 0x445566);
        assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
        assert!(app.forms.controls.is_empty());
    }
}
