//! Geometry reads use actual loaded CSS, browser viewport and live form state.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-geometry-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn open(&self, markup: &str) -> App {
        let path = self.0.join("page.html");
        std::fs::write(&path, markup).unwrap();
        let mut app = App::new();
        app.open(&path);
        assert!(!app.failed, "{}", app.status);
        app
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn id(app: &App, value: &str) -> usize {
    app.document
        .nodes
        .iter()
        .enumerate()
        .find_map(|(node, _)| (app.document.attr(node, "id") == Some(value)).then_some(node))
        .unwrap()
}
fn report(app: &App) -> String {
    app.scripting.as_ref().unwrap().report()
}

#[test]
fn startup_geometry_uses_loaded_css_and_same_job_mutations() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("geometry.css"),
        "#box{width:120px;height:30px;padding:5px 7px;border:3px solid black}#atomic{display:inline-block;width:40px;height:12px}").unwrap();
    let app = fixture.open(r#"<html><head><link rel=stylesheet href=geometry.css></head><body>
<div id=box></div><span id=inline>Inline text</span><span id=atomic></span><div style='display:none'><div id=hidden>Hidden</div></div>
<script>
var root=document.documentElement, box=document.getElementById('box');
var viewport=root.clientWidth+','+root.clientHeight;
var original=box.clientWidth+','+box.clientHeight+','+box.clientLeft+','+box.clientTop;
for(var i=0;i<50;i++){if(box.clientWidth!==134)throw Error('changed cached width');}
var others=document.getElementById('inline').clientWidth+','+document.getElementById('atomic').clientWidth+','+document.getElementById('hidden').clientHeight;
box.style.width='200px'; var changed=box.clientWidth;
box.style.display='none'; var hidden=box.clientWidth+','+box.clientHeight;
box.style.display='block'; var restored=box.clientWidth;
document.title=viewport+'|'+original+'|'+others+'|'+changed+'|'+hidden+'|'+restored;
</script></body></html>"#);
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        report(&app)
    );
    assert_eq!(
        app.document.title,
        format!(
            "{},{}|134,40,3,3|0,40,0|214|0,0|214",
            app.width - 12,
            app.viewport_height()
        )
    );
    assert_eq!(app.page.client_boxes[id(&app, "box")].unwrap().width, 214);
}

#[test]
fn geometry_tracks_resize_zoom_and_resource_view_without_measuring_the_report() {
    let fixture = Fixture::new();
    let mut app = fixture.open(r#"<html><body><button id=read type=button>Measure</button><script>
function measure(){document.title=document.documentElement.clientWidth+','+document.documentElement.clientHeight;}
document.getElementById('read').onclick=measure;
setTimeout(measure,0);
</script></body></html>"#);
    app.resize(812, 600);
    app.set_zoom(2.0);
    app.activate_control(id(&app, "read"));
    assert_eq!(
        app.document.title,
        format!("400,{}", app.viewport_height() / 2)
    );
    app.show_resources();
    app.resize(912, 700);
    app.set_zoom(1.5);
    assert!(app.poll_script_timer());
    assert_eq!(
        app.document.title,
        format!("600,{}", (app.viewport_height() as f64 / 1.5).round())
    );
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        report(&app)
    );
}

#[test]
fn native_checkbox_events_and_canceled_clicks_observe_current_css() {
    let fixture = Fixture::new();
    let mut app=fixture.open(r#"<html><head><style>#box{width:10px;height:10px}#c:checked + #box{width:20px}</style></head><body>
<input id=c type=checkbox><div id=box></div><button id=read type=button>Read</button><button id=change type=button>Change</button>
<script>
var c=document.getElementById('c'),box=document.getElementById('box');
var old=box.clientWidth;
c.onclick=function(event){document.title=box.clientWidth; if(!c.checked)event.preventDefault();};
document.getElementById('read').onclick=function(){document.title=box.clientWidth;};
document.getElementById('change').onclick=function(){c.checked=false;document.title=box.clientWidth;};
</script></body></html>"#);
    let checkbox = id(&app, "c");
    app.activate_control(checkbox);
    assert_eq!(app.document.title, "20", "{}", report(&app));
    app.activate_control(checkbox);
    assert_eq!(app.document.title, "10");
    assert!(
        app.forms.controls[&checkbox].checked,
        "canceled click restores checked state"
    );
    app.activate_control(id(&app, "read"));
    assert_eq!(app.document.title, "20");
    app.activate_control(id(&app, "change"));
    assert_eq!(app.document.title, "10");
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        report(&app)
    );
}

#[test]
fn excessive_script_layout_flushes_stop_the_job_and_next_timer_recovers() {
    let fixture = Fixture::new();
    let mut app = fixture.open(
        r#"<html><body><div id=box style='width:10px;height:10px'></div><script>
var box=document.getElementById('box');
setTimeout(function(){document.title='Recovered '+box.clientWidth;},0);
for(var i=0;i<5;i++){box.style.width=(10+i)+'px';var measured=box.clientWidth;}
</script></body></html>"#,
    );
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        1,
        "{}",
        report(&app)
    );
    assert!(report(&app).contains("DOM geometry layout limit reached"));
    assert!(app.poll_script_timer());
    assert_eq!(app.document.title, "Recovered 14");
}
