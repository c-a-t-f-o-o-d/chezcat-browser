//! Actual App proof for derived construction, live lexical this and super.
//! Pointer/key actions use literal fixture data; no desktop or clipboard use.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn node(app: &App, id: &str) -> usize {
    (0..app.document.nodes.len()).find(|&n| app.document.attr(n, "id") == Some(id)).unwrap()
}

fn clean(app: &App) {
    assert!(!app.failed, "{}", app.status);
    assert!(!app.is_loading());
    let scripting = app.scripting.as_ref().unwrap();
    assert_eq!(scripting.error_count(), 0, "{}", scripting.report());
    assert!(!scripting.has_navigation_request());
}

fn click(app: &mut App, id: &str) {
    let target = node(app, id);
    let rect = app.page.controls.iter().find(|r| r.node == target).unwrap();
    let (x, page_y) = (rect.x + rect.width / 2, rect.y + rect.height / 2);
    assert_eq!(app.page.control_at(x, page_y), Some(target));
    let y = app.content_top() + page_y - app.scroll;
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    clean(app);
}

fn editor(app: &App) -> (String, usize, usize, usize, bool, Option<usize>) {
    let c = &app.forms.controls[&node(app, "neighbor")];
    (c.value.text(), c.value.cursor, c.value.anchor, c.value.offset, c.dirty_value, c.owner)
}

fn pixels(app: &App, expected: &str) -> Vec<u32> {
    let rect = app.page.border_rects[node(app, "out")].unwrap();
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (20, 100, 300, 40));
    let runs: Vec<_> = app.page.runs.iter().filter(|r|
        r.x >= 20 && r.x < 320 && r.y >= 100 && r.y < 140).collect();
    assert_eq!(runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(), [expected]);
    assert!(runs.iter().all(|r| r.color == 0x123456));
    let canvas = app.draw();
    let top = (app.content_top() + 100 - app.scroll) as usize;
    let result: Vec<_> = (top..top + 40).flat_map(|y|
        canvas.pixels[y * canvas.width + 20..y * canvas.width + 320].iter().copied()).collect();
    assert!(result.iter().any(|&pixel| pixel == 0x123456), "The derived result must paint its foreground");
    assert_eq!(result.last(), Some(&0xffffff));
    result
}

fn stable_hits(app: &App) {
    let neighbor = node(app, "neighbor");
    let rect = app.page.controls.iter().find(|r| r.node == neighbor).unwrap();
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (20, 20, 220, 32));
    assert_eq!(app.page.control_at(130, 36), Some(neighbor));
    assert_eq!(app.page.control_at(240, 36), None);
    let link = node(app, "stable-link");
    let rect = app.page.border_rects[link].unwrap();
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (20, 180, 100, 24));
    for (x, y) in [(20, 180), (70, 192), (119, 203)] {
        assert_eq!(app.page.link_node_at(x, y), Some(link));
        assert_eq!(app.page.link_at(x, y), Some("#destination"));
    }
    assert_eq!(app.page.link_node_at(19, 192), None);
    assert_eq!(app.page.link_node_at(120, 192), None);
}

#[test]
fn derived_class_actual_app_super_and_live_this_paint_across_trusted_jobs() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); } }
    let fixture = Fixture(std::env::temp_dir().join(format!("chezcat-derived-class-app-{}-{}.html",
        std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
    std::fs::write(&fixture.0, r#"<!doctype html><html><head><title>Derived pending</title><style>
html,body{margin:0;padding:0;background:#ffffff}
#neighbor{display:block;position:absolute;left:20px;top:20px;width:220px;height:32px;margin:0}
#host{position:absolute;left:280px;top:20px;width:180px;height:32px}
#advance{display:block;width:180px;height:32px;margin:0;background:#e3edf2;color:#0b2233}
#out{position:absolute;left:20px;top:100px;width:300px;height:40px;margin:0;padding:0;border:0;background:#ffffff;color:#123456;font-size:24px;line-height:32px}
#stable-link{display:block;position:absolute;left:20px;top:180px;width:100px;height:24px;background:#336699;color:white}
</style></head><body><form id=f><input id=neighbor name=neighbor value="neighbor seed"><div id=host></div></form>
<p id=out>Fallback</p><a id=stable-link href=#destination>Go</a><div id=destination></div><script>
class BaseCounter {
 constructor(host,output){
  if(new.target!==Counter)throw 'derived new target';
  this.value=1;this.setterCalls=0;this.output=output;this.target=()=>new.target;
  this.button=document.createElement('button');this.button.id='advance';this.button.name='advance';this.button.type='button';this.button.value='derived payload';
  this.button.appendChild(document.createTextNode('Advance'));
  this.button.onclick=(event)=>{
   if(!event.isTrusted||this!==counter||event.currentTarget!==this.button||this.target()!==Counter)throw 'trusted derived instance';
   this.advance();
  };
  host.appendChild(this.button);
 }
 get count(){return this.value;}
 set count(value){this.value=value;this.setterCalls++;}
 read(){return this.value;}
}
class Counter extends BaseCounter {
 constructor(host,output){
  var live=()=>this,early=0;
  try{live();}catch(error){if(error.name==='ReferenceError')early++;else throw error;}
  if(early!==1)throw 'uninitialized this';
  super(host,output);
  if(live()!==this)throw 'live this after super';
  this.readLive=live;this.clicks=0;this.paint();
 }
 advance(){super.count=super.count+1;this.clicks++;this.paint();}
 paint(){
  if(super.read()!==this.value||this.readLive()!==this||this.setterCalls!==this.clicks||this.value!==this.clicks+1)throw 'super receiver and persistent state';
  this.output.textContent='Count'+this.value;document.title='DerivedCount'+this.value;
 }
}
var counter=new Counter(document.getElementById('host'),document.getElementById('out'));
</script></body></html>"#).unwrap();
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    clean(&app);
    assert_eq!(app.document.title, "DerivedCount1");
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    let stable_nodes = (node(&app, "neighbor"), node(&app, "stable-link"), node(&app, "out"));
    stable_hits(&app);
    let dynamic = node(&app, "advance");
    let button = &app.forms.controls[&dynamic];
    assert_eq!(button.kind, Kind::Button);
    assert_eq!(button.name, "advance");
    assert_eq!(button.value.text(), "derived payload");
    assert_eq!(button.caption, "Advance");
    assert_eq!(button.owner, Some(node(&app, "f")));
    assert_eq!(app.forms.controls.len(), 2);
    let rect = app.page.controls.iter().find(|r| r.node == dynamic).unwrap();
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (280, 20, 180, 32));
    click(&mut app, "neighbor");
    assert!(matches!(app.key(0x41, true, false, false), PlatformAction::None));
    app.paste("neighbor kept");
    assert!(matches!(app.key(0x24, false, false, false), PlatformAction::None));
    for _ in 0..2 { assert!(matches!(app.key(0x27, false, false, false), PlatformAction::None)); }
    for _ in 0..3 { assert!(matches!(app.key(0x27, false, true, false), PlatformAction::None)); }
    let preserved = editor(&app);
    assert_eq!((preserved.0.as_str(), preserved.1, preserved.2), ("neighbor kept", 5, 2));
    assert!(preserved.4);
    let initial = pixels(&app, "Count1");
    click(&mut app, "advance");
    assert_eq!(app.document.title, "DerivedCount2");
    let second = pixels(&app, "Count2");
    assert_ne!(second, initial);
    assert_eq!(editor(&app), preserved);
    stable_hits(&app);
    click(&mut app, "advance");
    assert_eq!(app.document.title, "DerivedCount3");
    assert_ne!(pixels(&app, "Count3"), second);
    assert_eq!(editor(&app), preserved);
    stable_hits(&app);
    assert_eq!(app.forms.text_default_value(node(&app, "neighbor")).unwrap(), "neighbor seed");
    assert_eq!((node(&app, "neighbor"), node(&app, "stable-link"), node(&app, "out")), stable_nodes);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    clean(&app);
}
