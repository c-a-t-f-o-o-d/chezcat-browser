//! Original App proof of offset coordinates against real border/padding boxes.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-offset-coordinate-app-{}-{}.html",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, r#"<!doctype html><html><head><title>Coordinates pending</title><style>
html,body{margin:0;padding:0;background:#ffffff}body{height:1000px}
#neighbor{display:block;position:fixed;left:20px;top:20px;width:220px;height:32px;margin:0}
#change{display:block;position:fixed;left:280px;top:20px;width:180px;height:32px;margin:0;padding:0}
#outer{position:absolute;left:20px;top:140px;box-sizing:border-box;width:500px;height:200px;margin:0;padding:40px 0 0 80px;border:10px solid #225588;background:#336699}
#ancestor{position:static;box-sizing:border-box;width:220px;height:120px;margin:0;padding:8px 0 0 12px;border:6px solid #886622;background:#f0e0c0}
#ancestor.positioned{position:relative}
#subject{display:block;position:absolute;left:20px;top:20px;width:60px;height:20px;margin:0;padding:4px 6px;border:3px solid #112233;background:#cc3300}
#ancestor.positioned #subject{left:30px;top:14px}
</style></head><body><form id=f><input id=neighbor name=neighbor value="neighbor seed"><button id=change name=change type=button>Toggle ancestor</button></form>
<div id=outer><div id=ancestor><a id=subject href=#destination></a></div></div><div id=destination></div><script>
var outer=document.getElementById('outer'),inner=document.getElementById('ancestor'),subject=document.getElementById('subject'),changes=0;
function offsets(left,top,parent){
 if(subject.offsetParent!==parent||subject.offsetLeft!==left||subject.offsetTop!==top)throw 'relative border edge must use offset parent padding edge';
 if(inner.offsetLeft!==80||inner.offsetTop!==40||inner.offsetParent!==outer)throw 'normal ancestor offsets must include outer padding and exclude its border';
 if(subject.parentNode!==inner||inner.parentNode!==outer||subject.ownerDocument!==document)throw 'coordinate read changed native tree';
}
document.getElementById('change').onclick=function(event){
 if(!event.isTrusted||event.currentTarget!==this||this.form!==document.getElementById('f'))throw 'trusted owned native button required';
 if(changes===0){
  offsets(20,20,outer);
  var before=subject.getBoundingClientRect();
  if(before.x!==50||before.y!==170)throw 'initial screen and relative coordinates must differ';
  inner.className='positioned';offsets(30,14,inner);document.title='Coordinates_inner';
 }else if(changes===1){
  if(document.documentElement.scrollTop!==72)throw 'real wheel must precede second activation';
  offsets(30,14,inner);
  var scrolled=subject.getBoundingClientRect();
  if(scrolled.x!==146||scrolled.y!==138)throw 'screen rectangle must move independently of relative offsets';
  inner.className='';offsets(20,20,outer);document.title='Coordinates_outer';
 }else throw 'unexpected activation';
 changes++;
};
document.title='Coordinates_ready';
</script></body></html>"#).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
}
fn id(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len()).find(|&node| app.document.attr(node, "id") == Some(name)).unwrap()
}
fn clean(app: &App) {
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let page = app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(), 0, "{}", page.report());
    assert!(!page.has_navigation_request());
}
fn click(app: &mut App, name: &str) {
    let node = id(app, name);
    let rect = app.page.controls.iter().find(|rect| rect.node == node).unwrap();
    let (x, page_y) = (rect.x + rect.width / 2, rect.y + rect.height / 2);
    assert_eq!(app.page.control_at(x, page_y), Some(node));
    let y = app.content_top() + page_y - app.scroll;
    assert!(y >= app.content_top() && y < app.content_bottom());
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    clean(app);
}
fn editor(app: &App) -> (String, usize, usize, usize, bool, Option<usize>) {
    let c = &app.forms.controls[&id(app, "neighbor")];
    (c.value.text(), c.value.cursor, c.value.anchor, c.value.offset, c.dirty_value, c.owner)
}
fn native_controls(app: &App) {
    assert_eq!(app.forms.controls.len(), 2);
    for (name, kind) in [("neighbor", Kind::Text), ("change", Kind::Button)] {
        let node = id(app, name);
        let control = &app.forms.controls[&node];
        assert_eq!(control.kind, kind);
        assert_eq!(control.name, name);
        assert_eq!(control.owner, Some(id(app, "f")));
        let rect = app.page.controls.iter().find(|rect| rect.node == node).unwrap();
        assert!(app.page.control_is_fixed(node));
        assert_eq!(rect.y - app.scroll, 20);
        assert_eq!(app.page.control_at(rect.x + 2, rect.y + 2), Some(node));
    }
    assert_eq!(app.forms.text_default_value(id(app, "neighbor")).unwrap(), "neighbor seed");
}
fn screen_pixel(canvas: &Canvas, x: i32, y: i32) -> u32 {
    assert!(x >= 0 && y >= 0 && (x as usize) < canvas.width && (y as usize) < canvas.height);
    canvas.pixels[y as usize * canvas.width + x as usize]
}
fn page_pixel(app: &App, canvas: &Canvas, x: i32, y: i32) -> u32 {
    screen_pixel(canvas, x, app.content_top() + y - app.scroll)
}
fn painted_link(app: &App, x: i32, y: i32) -> (i32, i32) {
    for (name, expected) in [
        ("outer", (20, 140, 500, 200)),
        ("ancestor", (110, 190, 220, 120)),
        ("subject", (x, y, 78, 34)),
    ] {
        let rect = app.page.border_rects[id(app, name)].unwrap();
        assert_eq!((rect.x, rect.y, rect.width, rect.height), expected, "{name}");
    }
    let canvas = app.draw();
    assert_eq!(page_pixel(app, &canvas, 25, 145), 0x225588);
    assert_eq!(page_pixel(app, &canvas, 40, 155), 0x336699);
    assert_eq!(page_pixel(app, &canvas, x, y), 0x112233);
    for (dx, dy) in [(3, 3), (39, 17), (74, 30)] {
        assert_eq!(page_pixel(app, &canvas, x + dx, y + dy), 0xcc3300);
        assert_eq!(app.page.link_node_at(x + dx, y + dy), Some(id(app, "subject")));
        assert_eq!(app.page.link_at(x + dx, y + dy), Some("#destination"));
    }
    assert_eq!(app.page.link_node_at(x + 78, y + 17), None);
    let point = (x + 39, app.content_top() + y + 17 - app.scroll);
    assert!(app.anchor_hit(point.0, point.1, 0).is_some());
    point
}

#[test]
fn offset_coordinates_actual_app_padding_edge_live_ancestor_change_and_scroll_preserve_editor() {
    let fixture = Fixture::new();
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    clean(&app);
    assert_eq!(app.document.title, "Coordinates_ready");
    assert_eq!(app.zoom, 1.0);
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    // Real geometry, border/padding pixels and native input precede offset reads.
    native_controls(&app);
    let initial = painted_link(&app, 50, 170);
    click(&mut app, "neighbor");
    assert!(matches!(app.key(0x41, true, false, false), PlatformAction::None));
    app.paste("neighbor kept");
    assert!(matches!(app.key(0x24, false, false, false), PlatformAction::None));
    for _ in 0..2 { assert!(matches!(app.key(0x27, false, false, false), PlatformAction::None)); }
    for _ in 0..3 { assert!(matches!(app.key(0x27, false, true, false), PlatformAction::None)); }
    let preserved = editor(&app);
    assert_eq!((preserved.0.as_str(), preserved.1, preserved.2), ("neighbor kept", 5, 2));
    assert!(preserved.4);

    click(&mut app, "change");
    assert_eq!(app.document.title, "Coordinates_inner");
    let positioned = painted_link(&app, 146, 210);
    assert_eq!(page_pixel(&app, &app.draw(), 80, 180), 0x336699);
    assert!(app.anchor_hit(initial.0, initial.1, 0).is_none());
    assert_eq!(editor(&app), preserved);
    native_controls(&app);

    app.wheel(-120, false);
    assert_eq!(app.scroll, 72);
    let scrolled = painted_link(&app, 146, 210);
    assert_eq!(scrolled, (positioned.0, positioned.1 - 72));
    assert_eq!(screen_pixel(&app.draw(), positioned.0, positioned.1), 0xf0e0c0);
    assert!(app.anchor_hit(positioned.0, positioned.1, 0).is_none());
    click(&mut app, "change");
    assert_eq!(app.document.title, "Coordinates_outer");
    assert_eq!(app.scroll, 72);
    let restored = painted_link(&app, 50, 170);
    assert_eq!(restored, (initial.0, initial.1 - 72));
    assert_eq!(page_pixel(&app, &app.draw(), 185, 227), 0xf0e0c0);
    assert!(app.anchor_hit(scrolled.0, scrolled.1, 0).is_none());
    assert_eq!(editor(&app), preserved);
    native_controls(&app);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    clean(&app);
}
