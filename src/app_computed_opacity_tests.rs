//! Opacity readback controls an explicit visible branch. Group compositing is
//! deliberately not an acceptance claim of this computed-style test.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-computed-opacity-app-{}-{}.html",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path,r#"<!doctype html><html><head><title>Opacity_ready</title><style>
html,body{margin:0;padding:0;background:#ffffff}html{opacity:40%}html.changed{opacity:75%}
#neighbor{position:absolute;left:20px;top:20px;width:220px;height:32px;margin:0}
#change{position:absolute;left:280px;top:20px;width:180px;height:32px;margin:0;padding:0}
#status{position:absolute;left:20px;top:80px;margin:0}
.panel{position:absolute;top:140px;width:160px;height:60px;margin:0;padding:0;border:0}
#marker{left:20px;background:#993300}#literal{left:220px;background:#228844}
</style></head><body><form id=f><input id=neighbor value='neighbor seed'><button id=change type=button>Read actual opacity</button></form><p id=status>Opacity_ready</p><div id=marker class=panel></div><div id=literal class=panel></div><script>
var activations=0;
document.getElementById('change').onclick=function(event){
 if(!event.isTrusted||event.currentTarget!==this||this.form!==document.getElementById('f')||activations!==0)throw 'trusted owned activation';
 var root=document.documentElement;
 if(getComputedStyle(root).getPropertyValue('opacity')!=='0.4')throw 'root opacity first read';
 var saved=getComputedStyle(root);
 if(saved.opacity!=='0.4'||getComputedStyle(document.getElementById('neighbor')).opacity!=='1')throw 'named opacity and native noninheritance';
 root.className='changed';
 if(saved.opacity!=='0.75')throw 'live class opacity';
 root.setAttribute('style','opacity:25%');
 if(saved.opacity!=='0.25'||saved.getPropertyValue('OPACITY')!=='0.25')throw 'live inline opacity';
 document.getElementById('marker').setAttribute('style','background:#228844');
 document.getElementById('status').textContent='Opacity_verified';document.title='Opacity_verified';activations++;
};
</script></body></html>"#).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn id(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len())
        .find(|&n| app.document.attr(n, "id") == Some(name))
        .unwrap()
}
fn clean(app: &App) {
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let p = app.scripting.as_ref().unwrap();
    assert_eq!(p.error_count(), 0, "{}", p.report());
    assert!(!p.has_navigation_request());
}
fn click(app: &mut App, name: &str) {
    let n = id(app, name);
    let r = app.page.controls.iter().find(|r| r.node == n).unwrap();
    let (x, py) = (r.x + r.width / 2, r.y + r.height / 2);
    assert_eq!(app.page.control_at(x, py), Some(n));
    let y = app.content_top() + py - app.scroll;
    assert!(matches!(
        app.pointer_down(x, y, false),
        PlatformAction::None
    ));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    clean(app);
}
fn pixel(app: &App, canvas: &Canvas, x: i32, y: i32) -> u32 {
    canvas.pixels[(app.content_top() + y - app.scroll) as usize * canvas.width + x as usize]
}

#[test]
fn computed_opacity_actual_app_root_branch_trusted_input_pixels_and_native_neighbor() {
    let fixture = Fixture::new();
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    clean(&app);
    assert_eq!(app.document.title, "Opacity_ready");
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    let marker = id(&app, "marker");
    let literal = id(&app, "literal");
    let original_rect = app.page.border_rects[marker].unwrap();
    assert_eq!(
        (
            original_rect.x,
            original_rect.y,
            original_rect.width,
            original_rect.height
        ),
        (20, 140, 160, 60)
    );
    assert_eq!(pixel(&app, &app.draw(), 30, 150), 0x993300);
    assert_eq!(pixel(&app, &app.draw(), 230, 150), 0x228844);
    let native = app.page.controls.clone();
    click(&mut app, "neighbor");
    assert!(matches!(
        app.key(0x41, true, false, false),
        PlatformAction::None
    ));
    app.paste("neighbor kept");
    let neighbor = id(&app, "neighbor");
    let c = &app.forms.controls[&neighbor];
    let kept = (
        c.value.text(),
        c.value.cursor,
        c.value.anchor,
        c.value.offset,
        c.dirty_value,
        c.owner,
    );
    assert_eq!(kept.0, "neighbor kept");
    assert!(kept.4);
    click(&mut app, "change");
    assert_eq!(app.document.title, "Opacity_verified");
    assert!(app.page.runs.iter().any(|r| r.text == "Opacity_verified"));
    let current = app.page.border_rects[marker].unwrap();
    assert_eq!(
        (current.x, current.y, current.width, current.height),
        (
            original_rect.x,
            original_rect.y,
            original_rect.width,
            original_rect.height
        )
    );
    let twin = app.page.border_rects[literal].unwrap();
    assert_eq!(
        (twin.x, twin.y, twin.width, twin.height),
        (220, 140, 160, 60)
    );
    let canvas = app.draw();
    for y in 140..200 {
        for dx in 0..160 {
            assert_eq!(pixel(&app, &canvas, 20 + dx, y), 0x228844);
            assert_eq!(
                pixel(&app, &canvas, 20 + dx, y),
                pixel(&app, &canvas, 220 + dx, y)
            );
        }
    }
    assert_eq!(pixel(&app, &canvas, 19, 150), 0xffffff);
    assert_eq!(pixel(&app, &canvas, 180, 150), 0xffffff);
    let c = &app.forms.controls[&neighbor];
    assert_eq!(
        (
            c.value.text(),
            c.value.cursor,
            c.value.anchor,
            c.value.offset,
            c.dirty_value,
            c.owner
        ),
        kept
    );
    assert_eq!(
        app.forms.text_default_value(neighbor).unwrap(),
        "neighbor seed"
    );
    assert_eq!(c.owner, Some(id(&app, "f")));
    assert_eq!(app.page.controls.len(), native.len());
    for (a, b) in app.page.controls.iter().zip(&native) {
        assert_eq!(
            (a.node, a.x, a.y, a.width, a.height),
            (b.node, b.x, b.y, b.width, b.height)
        );
    }
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    clean(&app);
}
