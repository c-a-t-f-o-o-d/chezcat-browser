//! Original software-App opacity paint tests. Specified/computed opacity and
//! the independent normal painter are controls, not substitutes for pixels.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-opacity-paint-{}-{}.html",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path,r#"<!doctype html><html><head><title>Opacity_paint_ready</title><style>
html,body{margin:0;padding:0;background:#ffffff;color:#010101}
#neighbor{position:absolute;left:20px;top:20px;width:220px;height:28px;margin:0}
#toggle{position:absolute;left:280px;top:20px;width:180px;height:32px;margin:0;padding:0}
.tile{position:absolute;width:72px;height:48px;margin:0;padding:0;border:0;background:#010101}
#half{left:20px;top:80px;opacity:.5}#half-ref{left:120px;top:80px;background:#808080}
#zero{left:220px;top:80px;opacity:0}#zero-ref{left:320px;top:80px;background:#ffffff}#full{left:420px;top:80px}
#zero-link{display:block;width:72px;height:48px;background:#010101}
#group{left:20px;top:160px;opacity:.5;background:none}#group-ref{left:120px;top:160px;background:#808080}
.overlap{position:absolute;top:0;width:48px;height:48px;background:#010101}#overlap-a{left:0}#overlap-b{left:24px}
#nested{left:220px;top:160px;opacity:.5;background:none}#nested-child{position:absolute;left:0;top:0;width:72px;height:48px;opacity:.5;background:#010101}
#nested-ref{left:320px;top:160px;background:#c0c0c0}
.clip{position:absolute;top:240px;width:72px;height:48px;overflow:hidden;background:#335577}#clip{left:20px}#clip-ref{left:120px}
#clipped{position:absolute;left:-24px;top:0;width:72px;height:48px;opacity:.5;background:#010101}
#clipped-ref{position:absolute;left:-24px;top:0;width:72px;height:48px;background:#1a2b3c}
#translated{left:220px;top:240px;opacity:.5;transform:translate(7px,3px)}#translated-ref{left:327px;top:243px;background:#808080}
#zero-controls{left:20px;top:320px;opacity:0}#zero-button{width:72px;height:32px;padding:0;margin:0}
.mixed{position:absolute;top:400px;width:140px;height:80px;background:#010101;color:#010101}
#mixed{left:20px;opacity:.5}#mixed-ref{left:220px}
.mixed p{position:absolute;left:10px;top:4px;width:100px;margin:0;font-size:12px;line-height:16px;color:#010101}
.mixed button{position:absolute;left:10px;top:30px;width:100px;height:28px;padding:0;margin:0}
</style></head><body>
<form id=f><input id=neighbor value='neighbor seed'><button id=toggle type=button>Change real opacity</button></form>
<div id=half class=tile></div><div id=half-ref class=tile></div><div id=zero class=tile><a id=zero-link href=#kept></a></div><div id=zero-ref class=tile></div><div id=full class=tile></div>
<div id=group class=tile><div id=overlap-a class=overlap></div><div id=overlap-b class=overlap></div></div><div id=group-ref class=tile></div>
<div id=nested class=tile><div id=nested-child></div></div><div id=nested-ref class=tile></div>
<div id=clip class=clip><div id=clipped></div></div><div id=clip-ref class=clip><div id=clipped-ref></div></div>
<div id=translated class=tile></div><div id=translated-ref class=tile></div>
<div id=zero-controls class=tile><button id=zero-button type=button>Hidden hit</button></div>
<div id=mixed class=mixed><p>Real glyphs</p><button type=button>Inner</button></div><div id=mixed-ref class=mixed><p>Real glyphs</p><button type=button>Inner</button></div>
<script>
var activations=0;
document.getElementById('toggle').onclick=function(e){
 if(!e.isTrusted||e.currentTarget!==this||this.form!==document.getElementById('f')||activations!==0)throw 'one trusted owned button';
 var box=document.getElementById('half');if(getComputedStyle(box).opacity!=='0.5')throw 'computed prefix';
 box.style.opacity='0';if(getComputedStyle(box).opacity!=='0')throw 'live computed zero';
 activations++;document.title='Opacity_zero_changed';
};
document.getElementById('zero-button').onclick=function(e){if(!e.isTrusted||e.currentTarget!==this)throw 'real zero-opacity native hit';document.title='Opacity_zero_native_hit';};
</script></body></html>"#).unwrap();
        Self(path)
    }
    fn open(&self) -> App {
        let mut app = App::new();
        app.resize(640, 720);
        app.set_javascript_enabled(true);
        app.open(&self.0);
        clean(&app);
        app
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
    let page = app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(), 0, "{}", page.report());
    assert!(!page.has_navigation_request());
}
fn pixel(app: &App, canvas: &Canvas, x: i32, y: i32) -> u32 {
    let py = y + app.content_top() - app.scroll;
    assert!(x >= 0 && py >= 0 && (x as usize) < canvas.width && (py as usize) < canvas.height);
    canvas.pixels[py as usize * canvas.width + x as usize] & 0xffffff
}
fn uniform(app: &App, x: i32, y: i32, w: i32, h: i32, color: u32) {
    let canvas = app.draw();
    for dy in 0..h {
        for dx in 0..w {
            assert_eq!(
                pixel(app, &canvas, x + dx, y + dy),
                color,
                "actual page pixel ({},{})",
                x + dx,
                y + dy
            );
        }
    }
}
fn rect(app: &App, name: &str) -> (i32, i32, i32, i32) {
    let r = app.page.border_rects[id(app, name)].unwrap();
    (r.x, r.y, r.width, r.height)
}
fn click(app: &mut App, name: &str) {
    let node = id(app, name);
    let r = app.page.controls.iter().find(|r| r.node == node).unwrap();
    let (x, y) = (r.x + r.width / 2, r.y + r.height / 2);
    assert_eq!(app.page.control_at(x, y), Some(node));
    let py = y + app.content_top() - app.scroll;
    assert!(matches!(
        app.pointer_down(x, py, false),
        PlatformAction::None
    ));
    assert!(matches!(app.pointer_up(x, py), PlatformAction::None));
    clean(app);
}

#[test]
fn opacity_paint_actual_app_static_flat_overlapping_and_nested_groups() {
    let fixture = Fixture::new();
    let app = fixture.open();
    assert_eq!(rect(&app, "half"), (20, 80, 72, 48));
    assert_eq!(rect(&app, "group"), (20, 160, 72, 48));
    // Independent literal/full-opacity rendering must work before new pixels.
    uniform(&app, 120, 80, 72, 48, 0x808080);
    uniform(&app, 320, 80, 72, 48, 0xffffff);
    uniform(&app, 420, 80, 72, 48, 0x010101);
    uniform(&app, 320, 160, 72, 48, 0xc0c0c0);
    uniform(&app, 20, 80, 72, 48, 0x808080);
    uniform(&app, 220, 80, 72, 48, 0xffffff);
    uniform(&app, 20, 160, 72, 48, 0x808080);
    uniform(&app, 220, 160, 72, 48, 0xc0c0c0);
    assert_eq!(app.page.link_node_at(256, 104), Some(id(&app, "zero-link")));
    assert_eq!(app.page.link_at(256, 104), Some("#kept"));
}

#[test]
fn opacity_paint_actual_app_clipping_translation_and_text_native_ownership() {
    let fixture = Fixture::new();
    let app = fixture.open();
    let canvas = app.draw();
    for dy in 0..48 {
        for dx in 0..72 {
            assert_eq!(
                pixel(&app, &canvas, 20 + dx, 240 + dy),
                pixel(&app, &canvas, 120 + dx, 240 + dy),
                "clipped group {dx},{dy}"
            );
        }
    }
    uniform(&app, 227, 243, 72, 48, 0x808080);
    assert_eq!(pixel(&app, &canvas, 220, 240), 0xffffff);
    assert_eq!(
        rect(&app, "translated"),
        (220, 240, 72, 48),
        "canonical geometry stays untransformed"
    );
    // Compare the real glyph/control painter's opaque twin against a single
    // half-opacity composite; control borders and glyph coverage are included.
    for dy in 0..80 {
        for dx in 0..140 {
            let source = pixel(&app, &canvas, 220 + dx, 400 + dy);
            let mut expected = 0;
            for shift in [0, 8, 16] {
                expected |= ((((source >> shift) & 255) + 255 + 1) / 2) << shift;
            }
            assert_eq!(
                pixel(&app, &canvas, 20 + dx, 400 + dy),
                expected,
                "mixed group at {dx},{dy}"
            );
        }
    }
}

#[test]
fn opacity_paint_actual_app_live_zero_keeps_hit_and_edited_native_neighbor() {
    let fixture = Fixture::new();
    let mut app = fixture.open();
    let raw = app.raw.clone();
    let location = app.location.clone();
    let before = (rect(&app, "half"), rect(&app, "zero-controls"));
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
    uniform(&app, 20, 80, 72, 48, 0x808080);
    click(&mut app, "toggle");
    assert_eq!(app.document.title, "Opacity_zero_changed");
    uniform(&app, 20, 80, 72, 48, 0xffffff);
    assert_eq!((rect(&app, "half"), rect(&app, "zero-controls")), before);
    uniform(&app, 20, 320, 72, 48, 0xffffff);
    click(&mut app, "zero-button");
    assert_eq!(app.document.title, "Opacity_zero_native_hit");
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
    clean(&app);
}

#[test]
fn opacity_paint_actual_app_unsupported_element_retains_fallback_and_independent_group() {
    let fixture = Fixture::new();
    std::fs::write(&fixture.0, r#"<!doctype html><title>Opacity_unsupported_ready</title>
<style>html,body{margin:0;background:white}.tile{position:absolute;top:80px;width:72px;height:48px;background:#010101}#bad{left:20px}#half{left:120px;opacity:.5}#neighbor{position:absolute;left:20px;top:20px;width:200px;height:28px}#toggle{position:absolute;left:280px;top:20px;width:180px;height:32px}</style>
<form id=f><input id=neighbor value='neighbor seed'><button id=toggle type=button>Change supported group</button></form>
<div id=bad class=tile style='opacity:calc(.5)'></div><div id=half class=tile></div>
<script>getComputedStyle(document.getElementById('bad')).opacity;document.title='Unreached_unsupported';</script>
<script>
if(document.title!=='Opacity_unsupported_ready'||getComputedStyle(document.getElementById('half')).opacity!=='0.5')throw 'independent following script and computed value';
document.title='Opacity_unsupported_following';
document.getElementById('toggle').onclick=function(e){
 if(!e.isTrusted||e.currentTarget!==this||this.form!==document.getElementById('f'))throw 'real trusted owned button';
 document.getElementById('half').style.opacity='0';document.title='Opacity_unsupported_continued';
};</script>"#).unwrap();
    // Cascade unavailability is an existing fatal host-stage boundary, not a
    // catchable author exception. Keep that one exact report, then prove the
    // independent following script and trusted handler still execute.
    fn expected_fault(app: &App) {
        assert!(!app.failed && !app.is_loading(), "{}", app.status);
        let page = app.scripting.as_ref().unwrap();
        let report = page.report();
        assert_eq!(page.error_count(), 1, "{report}");
        let script = app.document.nodes.iter().position(|node| {
            matches!(&node.kind,crate::html::NodeKind::Element {tag,..} if tag=="script")
        }).unwrap();
        let fault = format!("inline node {script} (script node {script}): JavaScript error at line 1, column 1: Computed opacity unavailable within the CSS cascade limits.");
        assert_eq!(report.matches(&fault).count(), 1, "{report}");
        assert!(!page.has_navigation_request());
    }
    fn click_after_fault(app: &mut App, name: &str) {
        let node = id(app, name);
        let r = app.page.controls.iter().find(|r| r.node == node).unwrap();
        let (x, y) = (r.x + r.width / 2, r.y + r.height / 2);
        assert_eq!(app.page.control_at(x, y), Some(node));
        let py = y + app.content_top() - app.scroll;
        assert!(matches!(
            app.pointer_down(x, py, false),
            PlatformAction::None
        ));
        assert!(matches!(app.pointer_up(x, py), PlatformAction::None));
        expected_fault(app);
    }
    let mut app = App::new();
    app.resize(640, 720);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    expected_fault(&app);
    assert_eq!(app.document.title, "Opacity_unsupported_following");
    assert!(app.page.opacity_unsupported);
    assert!(!app.page.opacity_unavailable);
    assert!(app
        .page
        .computed_properties
        .read_opacity(id(&app, "bad"))
        .is_err());
    uniform(&app, 20, 80, 72, 48, 0x010101);
    uniform(&app, 120, 80, 72, 48, 0x808080);
    let canvas = app.draw();
    assert_eq!((canvas.width, canvas.height), (640, 720));
    assert!(!app.page.opacity_draw_refused.get());
    assert!(app
        .activity_report()
        .contains("unsupported computed opacity retains prior painting"));
    let original = (
        app.raw.clone(),
        app.location.clone(),
        rect(&app, "bad"),
        rect(&app, "half"),
    );
    click_after_fault(&mut app, "neighbor");
    app.key(0x41, true, false, false);
    app.paste("neighbor kept");
    let neighbor = id(&app, "neighbor");
    let value = &app.forms.controls[&neighbor].value;
    let kept = (value.cursor, value.anchor, value.offset);
    click_after_fault(&mut app, "toggle");
    assert_eq!(app.document.title, "Opacity_unsupported_continued");
    uniform(&app, 20, 80, 72, 48, 0x010101);
    uniform(&app, 120, 80, 72, 48, 0xffffff);
    assert_eq!(app.forms.controls[&neighbor].value.text(), "neighbor kept");
    let current = &app.forms.controls[&neighbor].value;
    assert_eq!((current.cursor, current.anchor, current.offset), kept);
    assert_eq!(
        (
            app.raw.clone(),
            app.location.clone(),
            rect(&app, "bad"),
            rect(&app, "half")
        ),
        original
    );
    expected_fault(&app);
}

#[test]
fn opacity_paint_actual_app_resource_refusal_keeps_chrome_inspector_and_diagnostic() {
    let fixture = Fixture::new();
    let layers = format!(
        "{}{}",
        "<div class=layer>".repeat(100),
        "</div>".repeat(100)
    );
    std::fs::write(&fixture.0,format!(r#"<!doctype html><title>Opacity_refusal_ready</title>
<style>html,body{{margin:0;background:white}}.layer{{position:absolute;left:0;top:0;width:1280px;height:900px;background:#010101;opacity:.5}}#scene{{position:absolute;left:0;top:80px}}#marker{{position:absolute;left:400px;top:50px;width:50px;height:20px;background:#cc3300}}#neighbor{{position:absolute;left:20px;top:20px;width:200px;height:28px}}</style>
<form id=f><input id=neighbor value='neighbor seed'></form><div id=marker></div><div id=scene>{layers}</div><script>var resourceControl=true;</script>"#)).unwrap();
    let mut app = fixture.open();
    app.resize(1280, 900);
    click(&mut app, "neighbor");
    app.key(0x41, true, false, false);
    app.paste("neighbor kept");
    app.open_inspector();
    let neighbor = id(&app, "neighbor");
    let original = (app.raw.clone(), app.location.clone(), rect(&app, "marker"));
    assert_eq!(app.page.opacity_groups.len(), 100);
    assert!(
        !app.page.opacity_unavailable,
        "the native paint preflight, not the layout scene cap, must refuse"
    );
    let refused = app.draw();
    assert_eq!((refused.width, refused.height), (1280, 900));
    assert!(app.page.opacity_draw_refused.get());
    assert!(app
        .activity_report()
        .contains("document painting refused before authored pixels"));
    assert_eq!(
        pixel(&app, &refused, 410, 55),
        0xffffff,
        "even independent earlier authored paint is withheld atomically"
    );
    assert_eq!(
        refused.pixels[app.content_bottom() as usize * refused.width],
        ui::TEAL,
        "inspector top border remains drawn"
    );
    assert_eq!(
        refused.pixels[(app.height - 1) as usize * refused.width],
        ui::ERROR,
        "visible browser status records refusal"
    );
    let warning_ink = (app.content_top() + 16..app.content_top() + 23)
        .flat_map(|y| {
            let pixels = &refused.pixels;
            (16..180).map(move |x| pixels[y as usize * refused.width + x])
        })
        .filter(|&color| color == ui::ERROR)
        .count();
    assert!(
        warning_ink > 0,
        "visible document refusal diagnostic is painted"
    );
    assert_eq!(app.forms.controls[&neighbor].value.text(), "neighbor kept");
    app.toggle_view(View::Resources);
    assert!(app.alternate.as_ref().unwrap().nodes.iter().any(|node| {
        matches!(&node.kind,crate::html::NodeKind::Text(text) if text.contains("document painting refused before authored pixels"))
    }));
    let resources = app.draw();
    assert_eq!((resources.width, resources.height), (1280, 900));
    assert!(
        !app.page.opacity_draw_refused.get(),
        "the independent resource view remains paintable"
    );
    app.toggle_view(View::Resources);
    let _ = app.draw();
    assert!(app.page.opacity_draw_refused.get());
    // Removing the resource demand restores normal document paint without a
    // reload, and the exact toolbar/address pixels remain independent of it.
    let result = app.scripting.as_mut().unwrap().console_eval(&mut app.document,&mut app.forms,
        "var layers=document.querySelectorAll('.layer');for(var i=0;i<layers.length;i++)layers[i].style.opacity='1';");
    assert_eq!(result.errors, 0, "{}", app.script_activity());
    assert!(result.layout_changed);
    app.reflow();
    let restored = app.draw();
    assert!(!app.page.opacity_draw_refused.get());
    assert!(app.page.opacity_groups.is_empty());
    assert_eq!(pixel(&app, &restored, 410, 55), 0xcc3300);
    let chrome = app.content_top() as usize * restored.width;
    assert_eq!(&refused.pixels[..chrome], &restored.pixels[..chrome]);
    assert_eq!(
        (app.raw.clone(), app.location.clone(), rect(&app, "marker")),
        original
    );
    assert_eq!(app.forms.controls[&neighbor].value.text(), "neighbor kept");
    clean(&app);
}
