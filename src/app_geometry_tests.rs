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
        self.open_with_limits(markup,crate::script_geometry::Limits::default())
    }
    fn open_with_limits(&self, markup: &str,limits:crate::script_geometry::Limits) -> App {
        let path = self.0.join("page.html");
        std::fs::write(&path, markup).unwrap();
        let mut app = App::new();
        app.geometry_limits=limits;
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
fn domrect_renderer_snapshots_include_real_borders_mutation_and_no_box_cases() {
    let fixture = Fixture::new();
    let app = fixture.open(r#"<html><head><style>
body{margin:0}#box{width:120px;height:30px;padding:5px 7px;border:2px solid black;border-top:3px solid black;border-right:7px solid black;border-bottom:11px solid black}
</style></head><body><div id=box></div><div style='display:none'><div id=hidden></div></div><div id=zero style='position:absolute;left:20px;top:30px;width:0;height:0'></div><img id=image width=10 height=12 alt=Image style='padding:3px 5px;border:2px solid black'><button id=control>Control</button><script>
var box=document.getElementById('box'),r=box.getBoundingClientRect(),x=r.x;
if(!(r instanceof DOMRect)||!(r instanceof DOMRectReadOnly)||r.width!==143||r.height!==54||r.right-r.left!==143||r.bottom-r.top!==54)throw 'real border rectangle';
if(r===box.getBoundingClientRect()||box.getBoundingClientRect.length!==0)throw 'fresh snapshot/method';
var zero=document.getElementById('zero').getBoundingClientRect();if(zero.x!==20||zero.y!==30||zero.width!==0||zero.height!==0)throw 'empty box preserves position';
var image=document.getElementById('image').getBoundingClientRect();if(image.width!==24||image.height!==22)throw 'actual replaced border box';
var control=document.getElementById('control').getBoundingClientRect();if(control.width<=0||control.height<=0)throw 'native control box';
box.style.width='200px';var changed=box.getBoundingClientRect();if(changed.width!==223||r.width!==143)throw 'live layout and stable snapshot';
r.x=987;if(box.getBoundingClientRect().x!==x||changed.x!==x)throw 'snapshot cannot move DOM';
var originalCtor=DOMRect;DOMRect=function(){throw 'mutable binding used';};if(!(box.getBoundingClientRect() instanceof originalCtor))throw 'private canonical prototype';DOMRect=originalCtor;
for(var node of [document.getElementById('hidden'),document.createElement('div')]){var empty=node.getBoundingClientRect();if(empty.x!==0||empty.y!==0||empty.width!==0||empty.height!==0)throw 'genuine no box';}
var caught=false;try{box.getBoundingClientRect.call(document);}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'wrong receiver';
document.title='renderer-domrect-ok';
</script></body></html>"#);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0, "{}", report(&app));
    assert_eq!(app.document.title, "renderer-domrect-ok");
    let actual = app.page.border_rects[id(&app, "box")].unwrap();
    assert_eq!((actual.width, actual.height), (223, 54));
}

#[test]
fn domrect_scroll_zoom_and_alternate_view_use_the_underlying_rendered_page() {
    let fixture = Fixture::new();
    let mut app = fixture.open(r#"<html><head><style>body{margin:0}#box{width:101px;height:31px;border:1px solid black}#fixed{position:fixed;left:1px;top:7px;width:9px;height:11px}</style></head><body><div id=box></div><div id=fixed></div><div style='height:2000px'></div><script>
function measureRect(){var r=document.getElementById('box').getBoundingClientRect(),f=document.getElementById('fixed').getBoundingClientRect();document.title=r.x+','+r.y+','+r.width+','+r.height+'|'+f.x+','+f.y+','+f.width+','+f.height;}
</script></body></html>"#);
    app.set_zoom(1.5);
    let normal = app.page.border_rects[id(&app, "box")].unwrap();
    let fixed = app.page.border_rects[id(&app, "fixed")].unwrap();
    assert!(fixed.fixed); assert!(!normal.fixed);
    app.set_scroll(123);
    let expected = [normal.x as f64/1.5, (normal.y-123) as f64/1.5, normal.width as f64/1.5, normal.height as f64/1.5,
        fixed.x as f64/1.5, fixed.y as f64/1.5, fixed.width as f64/1.5, fixed.height as f64/1.5];
    for alternate in [false, true] {
        if alternate { app.show_resources(); app.set_scroll(45); }
        let outcome = app.scripting.as_mut().unwrap().console_eval(&mut app.document, &mut app.forms, "measureRect();");
        assert_eq!(outcome.errors, 0, "{}", report(&app));
        let actual: Vec<f64> = app.document.title.split([',','|']).map(|v|v.parse().unwrap()).collect();
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.into_iter().zip(expected) { assert!((actual-expected).abs()<1e-9, "{actual} != {expected}"); }
    }
}

#[test]
fn domrect_unmeasured_inline_is_explicit_and_layout_flush_limit_still_recovers() {
    let fixture = Fixture::new();
    let app = fixture.open("<html><body><span id=s>Real inline</span><script>document.getElementById('s').getBoundingClientRect();document.title='fake zero';</script></body></html>");
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 1);
    assert!(report(&app).contains("unmeasured inline")); assert_ne!(app.document.title, "fake zero");
    let mut app = fixture.open_with_limits(r#"<html><body><div id=box style='width:10px;height:10px'></div><script>
var box=document.getElementById('box');setTimeout(function(){document.title='Recovered '+box.getBoundingClientRect().width;},0);
for(var i=0;i<5;i++){box.style.width=(10+i)+'px';box.getBoundingClientRect();}
</script></body></html>"#,crate::script_geometry::Limits {layouts_per_job:4});
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 1, "{}", report(&app));
    assert!(report(&app).contains("DOM geometry layout limit reached"));
    assert!(app.poll_script_timer()); assert_eq!(app.document.title, "Recovered 14");
}

#[test]
fn offset_dimensions_use_actual_asymmetric_border_boxes_and_live_zoomed_layout() {
    let fixture=Fixture::new();
    let mut app=fixture.open(r#"<html><head><style>
#box{width:120px;height:30px;padding:5px 7px;border:2px solid black;border-top:3px solid black;border-right:7px solid black;border-bottom:11px solid black}
</style></head><body><div id=box></div><div style='display:none'><div id=hidden></div></div><button id=read type=button>Read</button><script>
var box=document.getElementById('box');
if(box.clientWidth!==134||box.offsetWidth!==143||box.offsetHeight!==54)throw 'asymmetric border box';
if(document.getElementById('hidden').offsetWidth!==0)throw 'hidden no box';
var detached=document.createElement('div');detached.style.width='800px';if(detached.offsetWidth!==0)throw 'detached no box';
box.offsetWidth=1;if(box.offsetWidth!==143)throw 'readonly';
box.style.width='200px';if(box.offsetWidth!==223)throw 'live width';
box.style.width='120px';document.getElementById('read').onclick=function(){document.title=box.offsetWidth+','+box.offsetHeight;};document.title='offset-ready';
</script></body></html>"#);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",report(&app));
    assert_eq!(app.document.title,"offset-ready");
    app.set_zoom(2.0);
    app.activate_control(id(&app,"read"));
    assert_eq!(app.document.title,"143,54","{}",report(&app));
    let client=app.page.client_boxes[id(&app,"box")].unwrap();
    assert_eq!(client.left,4);
    assert_eq!(client.right,14);
    assert_eq!(client.bottom,22);
}

#[test]
fn offset_dimensions_hidden_root_is_zero_and_unmeasured_inline_rejects() {
    let fixture=Fixture::new();
    let app=fixture.open(r#"<html style='display:none'><head><script>
document.title=document.documentElement.offsetWidth+','+document.documentElement.offsetHeight;
</script></head><body>Hidden</body></html>"#);
    assert_eq!(app.document.title,"0,0","{}",report(&app));
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0);
    let inline=fixture.open(r#"<html><body><span id=s>Ordinary inline text</span><script>document.title=document.getElementById('s').offsetWidth;</script></body></html>"#);
    assert_eq!(inline.scripting.as_ref().unwrap().error_count(),1);
    assert!(report(&inline).contains("unmeasured inline"),"{}",report(&inline));
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
    let mut app = fixture.open_with_limits(
        r#"<html><body><div id=box style='width:10px;height:10px'></div><script>
var box=document.getElementById('box');
setTimeout(function(){document.title='Recovered '+box.clientWidth;},0);
for(var i=0;i<5;i++){box.style.width=(10+i)+'px';var measured=box.clientWidth;}
</script></body></html>"#,crate::script_geometry::Limits {layouts_per_job:4},
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

#[test]
fn generous_normal_geometry_policy_measures_many_mutations_without_maximum_preallocation(){
    let fixture=Fixture::new();let app=fixture.open(r#"<html><body><div id=box style='width:10px;height:10px'></div><script>
var box=document.getElementById('box');for(var i=0;i<128;i++){box.style.width=(10+i)+'px';if(box.clientWidth!==10+i||box.getBoundingClientRect().width!==10+i)throw 'measured mutation';}document.title='Generous geometry ready';
</script></body></html>"#);assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",report(&app));assert_eq!(app.document.title,"Generous geometry ready");assert!(app.geometry_limits.layouts_per_job>=1024);
}
