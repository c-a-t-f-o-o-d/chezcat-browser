//! Original App containing-block geometry and retained native offsetParent
//! identities across trusted class changes. No network or physical input.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-offset-parent-app-{}-{}.html",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, r#"<!doctype html><html><head><title>Offset pending</title><style>
html,body{margin:0;padding:0;background:#ffffff}
#neighbor{display:block;position:absolute;left:20px;top:20px;width:220px;height:32px;margin:0}
#change{display:block;position:absolute;left:280px;top:20px;width:180px;height:32px;margin:0;padding:0}
#status{position:absolute;left:280px;top:80px;margin:0;color:#123456}
#outer{position:absolute;left:20px;top:140px;box-sizing:border-box;width:500px;height:200px;margin:0;padding:40px 0 0 80px;border:0;background:#336699}
#ancestor{position:static;width:220px;height:120px;margin:0;padding:0;border:0;background:#f0e0c0}
#ancestor.positioned{position:relative}
#subject{display:block;position:absolute;left:20px;top:20px;width:60px;height:20px;margin:0;padding:0;border:0;background:#cc3300}
</style></head><body><form id=f><input id=neighbor name=neighbor value="neighbor seed"><button id=change name=change type=button>Toggle ancestor</button></form><p id=status>Offset_ready</p>
<div id=outer><div id=ancestor><a id=subject href=#destination></a></div></div><div id=destination></div><script>
var outer=document.getElementById('outer'),inner=document.getElementById('ancestor'),subject=document.getElementById('subject');
var changes=0,rememberedOuter=null,rememberedInner=null;
document.getElementById('change').onclick=function(event){
 if(!event.isTrusted||event.currentTarget!==this||this.form!==document.getElementById('f'))throw 'trusted owned native button required';
 if(changes===0){
  rememberedOuter=subject.offsetParent;
  if(rememberedOuter!==outer||rememberedOuter.ownerDocument!==document||rememberedOuter.id!=='outer')throw 'initial native containing block identity';
  inner.className='positioned';rememberedInner=subject.offsetParent;
  if(rememberedInner!==inner||rememberedInner.ownerDocument!==document||rememberedOuter!==outer)throw 'live native containing block identity';
  document.getElementById('status').textContent='Offset_inner';document.title='Offset_inner';
 }else if(changes===1){
  if(subject.offsetParent!==rememberedInner)throw 'retained inner identity before restoration';
  inner.className='';
  if(subject.offsetParent!==rememberedOuter||rememberedOuter!==outer||rememberedInner!==inner||rememberedInner.ownerDocument!==document)throw 'restored and retained native containing blocks';
  document.getElementById('status').textContent='Offset_outer';document.title='Offset_outer';
 }else throw 'unexpected extra activation';
 if(inner.parentNode!==outer||subject.parentNode!==inner)throw 'offsetParent lookup changed the physical tree';
 changes++;
};
document.title='Offset_ready';
</script></body></html>"#).unwrap();
        Self(path)
    }
    fn open(&self) -> App {
        let mut app = App::new();
        app.resize(640, 480);
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
        .find(|&node| app.document.attr(node, "id") == Some(name))
        .unwrap()
}
fn clean(app: &App) {
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let scripting = app.scripting.as_ref().unwrap();
    assert_eq!(scripting.error_count(), 0, "{}", scripting.report());
    assert!(!scripting.has_navigation_request());
}
fn click(app: &mut App, name: &str) {
    let node = id(app, name);
    let rect = app
        .page
        .controls
        .iter()
        .find(|rect| rect.node == node)
        .unwrap();
    let (x, page_y) = (rect.x + rect.width / 2, rect.y + rect.height / 2);
    assert_eq!(app.page.control_at(x, page_y), Some(node));
    let y = app.content_top() + page_y - app.scroll;
    assert!(matches!(
        app.pointer_down(x, y, false),
        PlatformAction::None
    ));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    clean(app);
}
fn editor(app: &App) -> (String, usize, usize, usize, bool, Option<usize>) {
    let c = &app.forms.controls[&id(app, "neighbor")];
    (
        c.value.text(),
        c.value.cursor,
        c.value.anchor,
        c.value.offset,
        c.dirty_value,
        c.owner,
    )
}
fn native_controls(app: &App) {
    let neighbor = id(app, "neighbor");
    assert_eq!(app.forms.controls.len(), 2);
    assert_eq!(app.forms.controls[&neighbor].kind, Kind::Text);
    assert_eq!(app.forms.controls[&neighbor].owner, Some(id(app, "f")));
    assert_eq!(
        app.forms.text_default_value(neighbor).unwrap(),
        "neighbor seed"
    );
    assert_eq!(app.forms.controls[&id(app, "change")].kind, Kind::Button);
    assert_eq!(
        app.forms.controls[&id(app, "change")].owner,
        Some(id(app, "f"))
    );
    assert_eq!(app.page.control_at(130, 36), Some(neighbor));
    assert_eq!(app.page.control_at(19, 36), None);
    assert_eq!(app.page.control_at(240, 36), None);
}
fn pixel(app: &App, canvas: &Canvas, x: i32, page_y: i32) -> u32 {
    let y = app.content_top() + page_y - app.scroll;
    assert!(x >= 0 && y >= 0 && (x as usize) < canvas.width && (y as usize) < canvas.height);
    canvas.pixels[y as usize * canvas.width + x as usize]
}
fn boxes_and_link(app: &App, x: i32, y: i32) {
    for (name, expected) in [
        ("outer", (20, 140, 500, 200)),
        ("ancestor", (100, 180, 220, 120)),
        ("subject", (x, y, 60, 20)),
    ] {
        let rect = app.page.border_rects[id(app, name)].unwrap();
        assert_eq!(
            (rect.x, rect.y, rect.width, rect.height),
            expected,
            "{name}"
        );
    }
    let subject = id(app, "subject");
    let canvas = app.draw();
    for (dx, dy) in [(0, 0), (30, 10), (59, 19)] {
        assert_eq!(pixel(app, &canvas, x + dx, y + dy), 0xcc3300);
        assert_eq!(app.page.link_node_at(x + dx, y + dy), Some(subject));
        assert_eq!(app.page.link_at(x + dx, y + dy), Some("#destination"));
    }
    assert_eq!(app.page.link_node_at(x + 60, y + 10), None);
    assert_eq!(app.page.link_node_at(x + 30, y + 20), None);
}

#[test]
fn offset_parent_actual_app_containing_block_identity_tracks_trusted_ancestor_position_changes() {
    let fixture = Fixture::new();
    let mut app = fixture.open();
    assert_eq!(app.document.title, "Offset_ready");
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    // These real boxes, paints and hit targets precede the first offsetParent.
    native_controls(&app);
    boxes_and_link(&app, 40, 160);
    assert_eq!(pixel(&app, &app.draw(), 130, 210), 0xf0e0c0);
    assert_eq!(app.page.link_node_at(130, 210), None);
    click(&mut app, "neighbor");
    assert!(matches!(
        app.key(0x41, true, false, false),
        PlatformAction::None
    ));
    app.paste("neighbor kept");
    assert!(matches!(
        app.key(0x24, false, false, false),
        PlatformAction::None
    ));
    for _ in 0..2 {
        assert!(matches!(
            app.key(0x27, false, false, false),
            PlatformAction::None
        ));
    }
    for _ in 0..3 {
        assert!(matches!(
            app.key(0x27, false, true, false),
            PlatformAction::None
        ));
    }
    let preserved = editor(&app);
    assert_eq!(
        (preserved.0.as_str(), preserved.1, preserved.2),
        ("neighbor kept", 5, 2)
    );
    assert!(preserved.4);

    click(&mut app, "change");
    assert_eq!(app.document.title, "Offset_inner");
    assert!(app.page.runs.iter().any(|run| run.text == "Offset_inner"));
    boxes_and_link(&app, 120, 200);
    assert_eq!(pixel(&app, &app.draw(), 50, 170), 0x336699);
    assert_eq!(app.page.link_node_at(50, 170), None);
    assert_eq!(editor(&app), preserved);
    native_controls(&app);

    click(&mut app, "change");
    assert_eq!(app.document.title, "Offset_outer");
    assert!(app.page.runs.iter().any(|run| run.text == "Offset_outer"));
    boxes_and_link(&app, 40, 160);
    assert_eq!(pixel(&app, &app.draw(), 130, 210), 0xf0e0c0);
    assert_eq!(app.page.link_node_at(130, 210), None);
    assert_eq!(editor(&app), preserved);
    native_controls(&app);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    clean(&app);
}
