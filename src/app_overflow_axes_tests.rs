//! Original App proof of independent overflow clips, live computed reads and
//! trusted class mutation. Native editor and real link-hit controls stay live.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-overflow-axes-app-{}-{}.html",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, r#"<!doctype html><html><head><title>Axes pending</title><style>
html,body{margin:0;padding:0;background:#ffffff}
#neighbor{display:block;position:absolute;left:20px;top:20px;width:220px;height:32px;margin:0}
#swap{display:block;position:absolute;left:280px;top:20px;width:180px;height:32px;margin:0;padding:0}
#status{position:absolute;left:20px;top:80px;width:560px;height:28px;margin:0;padding:0;color:#123456}
.panel{position:absolute;top:140px;width:80px;height:60px;margin:0;padding:0;border:0;background:#336699}
.spill{display:block;position:absolute;left:20px;top:20px;width:100px;height:80px;margin:0;padding:0;border:0;background:#cc3300}
#axis{left:20px;overflow-x:clip;overflow-y:visible}
#axis.swapped{overflow-x:visible;overflow-y:clip}
#hidden{left:220px;overflow:hidden}
#visible{left:420px;overflow:visible}
</style></head><body><form id=f><input id=neighbor name=neighbor value="neighbor seed"><button id=swap name=swap type=button>Swap clip axes</button></form><p id=status>Axes_ready</p>
<div id=axis class=panel><a id=axis-link class=spill href=#destination></a></div>
<div id=hidden class=panel><a id=hidden-link class=spill href=#destination></a></div>
<div id=visible class=panel><a id=visible-link class=spill href=#destination></a></div>
<div id=destination></div><script>
var axis=document.getElementById('axis'),saved=getComputedStyle(axis);
var hiddenStyle=getComputedStyle(document.getElementById('hidden'));
var visibleStyle=getComputedStyle(document.getElementById('visible'));
var readOverflow=saved.getPropertyValue,activations=0;
function pair(style,x,y){
 var shorthand=x===y?x:x+' '+y;
 if(style.overflow!==shorthand||style.overflowX!==x||style.overflowY!==y)throw 'computed overflow named fields';
 if(readOverflow.call(style,'overflow')!==shorthand||readOverflow.call(style,'overflow-x')!==x||readOverflow.call(style,'overflow-y')!==y)throw 'computed overflow CSS names';
}
document.getElementById('swap').onclick=function(event){
 if(!event.isTrusted||event.currentTarget!==this||this.form!==document.getElementById('f')||activations!==0)throw 'one trusted owned button activation required';
 pair(hiddenStyle,'hidden','hidden');pair(visibleStyle,'visible','visible');
 pair(saved,'clip','visible');
 axis.className='panel swapped';
 pair(saved,'visible','clip');
 activations++;document.getElementById('status').textContent='Axes_swapped';document.title='Axes_swapped';
};
document.title='Axes_ready';
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
    let control = &app.forms.controls[&id(app, "neighbor")];
    (
        control.value.text(),
        control.value.cursor,
        control.value.anchor,
        control.value.offset,
        control.dirty_value,
        control.owner,
    )
}
fn native_controls(app: &App) {
    let neighbor = id(app, "neighbor");
    assert_eq!(app.forms.controls.len(), 2);
    assert_eq!(app.forms.controls[&neighbor].kind, Kind::Text);
    assert_eq!(app.forms.controls[&neighbor].name, "neighbor");
    assert_eq!(app.forms.controls[&neighbor].owner, Some(id(app, "f")));
    assert_eq!(
        app.forms.text_default_value(neighbor).unwrap(),
        "neighbor seed"
    );
    assert_eq!(app.forms.controls[&id(app, "swap")].kind, Kind::Button);
    assert_eq!(
        app.forms.controls[&id(app, "swap")].owner,
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
fn panel(app: &App, name: &str, left: i32, clip_x: bool, clip_y: bool) {
    let rect = app.page.border_rects[id(app, name)].unwrap();
    assert_eq!(
        (rect.x, rect.y, rect.width, rect.height),
        (left, 140, 80, 60)
    );
    let link = id(app, &format!("{name}-link"));
    let child = app.page.border_rects[link].unwrap();
    assert_eq!(
        (child.x, child.y, child.width, child.height),
        (left + 20, 160, 100, 80)
    );
    let canvas = app.draw();
    assert_eq!(pixel(app, &canvas, left + 5, 145), 0x336699);
    assert_eq!(app.page.link_node_at(left + 5, 145), None);
    for (dx, dy, admitted) in [
        (40, 40, true),
        (79, 40, true),
        (80, 40, !clip_x),
        (100, 40, !clip_x),
        (40, 59, true),
        (40, 60, !clip_y),
        (40, 80, !clip_y),
        (100, 80, !clip_x && !clip_y),
    ] {
        let (x, y) = (left + dx, 140 + dy);
        assert_eq!(
            pixel(app, &canvas, x, y),
            if admitted { 0xcc3300 } else { 0xffffff },
            "{name} pixel at relative ({dx},{dy})"
        );
        assert_eq!(
            app.page.link_node_at(x, y),
            admitted.then_some(link),
            "{name} link node at relative ({dx},{dy})"
        );
        assert_eq!(
            app.page.link_at(x, y),
            admitted.then_some("#destination"),
            "{name} href at relative ({dx},{dy})"
        );
    }
}

#[test]
fn overflow_axes_actual_app_pixels_hits_and_live_computed_swap_preserve_native_editor() {
    let fixture = Fixture::new();
    let mut app = fixture.open();
    assert_eq!(app.document.title, "Axes_ready");
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    native_controls(&app);
    // Existing both-axis clipping and visible overflow must pass first.
    panel(&app, "hidden", 220, true, true);
    panel(&app, "visible", 420, false, false);

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
    clean(&app);

    panel(&app, "axis", 20, true, false);
    click(&mut app, "swap");
    assert_eq!(app.document.title, "Axes_swapped");
    assert!(app.page.runs.iter().any(|run| run.text == "Axes_swapped"));
    assert_eq!(
        app.document.attr(id(&app, "axis"), "class"),
        Some("panel swapped")
    );
    panel(&app, "axis", 20, false, true);
    panel(&app, "hidden", 220, true, true);
    panel(&app, "visible", 420, false, false);
    assert_eq!(editor(&app), preserved);
    native_controls(&app);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    clean(&app);
}
