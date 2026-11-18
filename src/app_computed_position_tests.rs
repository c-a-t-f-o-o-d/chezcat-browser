//! Original App position-variable movement through trusted input, real pixels
//! and link-hit geometry. No library, network or physical desktop interaction.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-computed-position-app-{}-{}.html",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, r#"<!doctype html><html><head><title>Position pending</title><style>
html,body{margin:0;padding:0;background:#ffffff}
#theme{--Mode:static}#theme.changed{--Mode:relative}
#neighbor{display:block;position:absolute;left:20px;top:20px;width:220px;height:32px;margin:0}
#change{display:block;position:absolute;left:280px;top:20px;width:180px;height:32px;margin:0;padding:0}
#status{position:absolute;left:20px;top:80px;margin:0;color:#123456}
.panel{position:absolute;top:140px;width:220px;height:120px;margin:0;padding:0;border:0;background:#336699}
#variable-panel{left:20px}#literal-panel{left:320px}
.link{display:block;left:80px;top:40px;width:60px;height:20px;margin:0;padding:0;border:0;background:#cc3300}
#moving{position:var(--Mode)}#literal{position:relative}
</style></head><body><form id=f><input id=neighbor name=neighbor value="neighbor seed"><button id=change name=change type=button>Move original link</button></form><p id=status>Position_ready</p>
<div id=theme><div id=variable-panel class=panel><a id=moving class=link href=#destination></a></div></div>
<div id=literal-panel class=panel><a id=literal class=link href=#destination></a></div><div id=destination></div><script>
var moving=document.getElementById('moving'),saved=getComputedStyle(moving),readPosition=saved.getPropertyValue,activations=0;
function verifyPosition(expected){
 if(saved.position!==expected||readPosition.call(saved,'position')!==expected||saved.getPropertyValue('--Mode').trim()!==expected)throw 'live computed position and custom value disagree';
}
document.getElementById('change').onclick=function(event){
 if(!event.isTrusted||event.currentTarget!==this||this.form!==document.getElementById('f')||activations!==0)throw 'one trusted owned button activation required';
 verifyPosition('static');
 document.getElementById('theme').className='changed';
 verifyPosition('relative');
 activations++;document.getElementById('status').textContent='Position_relative';document.title='Position_relative';
};
document.title='Position_ready';
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
fn link(app: &App, name: &str, x: i32, y: i32) {
    let node = id(app, name);
    let rect = app.page.border_rects[node].unwrap();
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (x, y, 60, 20));
    let canvas = app.draw();
    for (dx, dy) in [(0, 0), (30, 10), (59, 19)] {
        assert_eq!(pixel(app, &canvas, x + dx, y + dy), 0xcc3300);
        assert_eq!(app.page.link_node_at(x + dx, y + dy), Some(node));
        assert_eq!(app.page.link_at(x + dx, y + dy), Some("#destination"));
    }
    assert_eq!(pixel(app, &canvas, x + 60, y + 10), 0x336699);
    assert_eq!(app.page.link_node_at(x + 60, y + 10), None);
    assert_eq!(app.page.link_node_at(x + 30, y + 20), None);
}

#[test]
fn computed_position_actual_app_custom_property_moves_link_on_trusted_class_change() {
    let fixture = Fixture::new();
    let mut app = fixture.open();
    assert_eq!(app.document.title, "Position_ready");
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    native_controls(&app);
    // Direct literal positioning must work before the variable-driven path.
    link(&app, "literal", 400, 180);
    link(&app, "moving", 20, 140);
    assert_eq!(pixel(&app, &app.draw(), 110, 190), 0x336699);
    assert_eq!(app.page.link_node_at(110, 190), None);

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
    assert_eq!(app.document.title, "Position_relative");
    assert!(app
        .page
        .runs
        .iter()
        .any(|run| run.text == "Position_relative"));
    assert_eq!(
        app.document.attr(id(&app, "theme"), "class"),
        Some("changed")
    );
    link(&app, "moving", 100, 180);
    link(&app, "literal", 400, 180);
    let canvas = app.draw();
    assert_eq!(pixel(&app, &canvas, 30, 150), 0x336699);
    assert_eq!(app.page.link_node_at(30, 150), None);
    for y in 140..260 {
        for dx in 0..220 {
            assert_eq!(
                pixel(&app, &canvas, 20 + dx, y),
                pixel(&app, &canvas, 320 + dx, y),
                "Live literal twin pixel at ({dx},{y})"
            );
        }
    }
    assert_eq!(editor(&app), preserved);
    native_controls(&app);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    clean(&app);
}
