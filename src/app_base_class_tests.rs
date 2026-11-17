//! Original App startup/class state, trusted native input and isolated paint.
//! No desktop input, clipboard, network or framework program is involved.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn node(app: &App, id: &str) -> usize {
    (0..app.document.nodes.len()).find(|&node| app.document.attr(node, "id") == Some(id)).unwrap()
}

fn clean(app: &App) {
    assert!(!app.failed, "{}", app.status);
    assert!(!app.is_loading());
    let scripting = app.scripting.as_ref().unwrap();
    assert_eq!(scripting.error_count(), 0, "{}", scripting.report());
    assert!(!scripting.has_navigation_request());
}

fn click(app: &mut App, id: &str) {
    let node = node(app, id);
    let rect = app.page.controls.iter().find(|rect| rect.node == node).unwrap();
    let (x, page_y) = (rect.x + rect.width / 2, rect.y + rect.height / 2);
    assert_eq!(app.page.control_at(x, page_y), Some(node));
    let y = app.content_top() + page_y - app.scroll;
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    clean(app);
}

fn editor(app: &App) -> (String, usize, usize, usize, bool, Option<usize>) {
    let control = &app.forms.controls[&node(app, "neighbor")];
    (control.value.text(), control.value.cursor, control.value.anchor, control.value.offset, control.dirty_value, control.owner)
}

fn result_pixels(app: &App, expected: &str) -> Vec<u32> {
    let rect = app.page.border_rects[node(app, "out")].unwrap();
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (20, 100, 300, 40));
    let runs: Vec<_> = app.page.runs.iter().filter(|run|
        run.x >= rect.x && run.x < rect.x + rect.width && run.y >= rect.y && run.y < rect.y + rect.height).collect();
    assert_eq!(runs.iter().map(|run| run.text.as_str()).collect::<Vec<_>>(), [expected]);
    assert!(runs.iter().all(|run| run.color == 0x123456));
    let canvas = app.draw();
    let top = (app.content_top() + rect.y - app.scroll) as usize;
    let pixels: Vec<_> = (top..top + 40).flat_map(|y|
        canvas.pixels[y * canvas.width + 20..y * canvas.width + 320].iter().copied()).collect();
    assert!(pixels.iter().any(|&pixel| pixel != 0xffffff), "The class result must paint glyphs");
    assert_eq!(pixels.last(), Some(&0xffffff));
    pixels
}

fn stable_hits(app: &App) {
    let neighbor = node(app, "neighbor");
    let rect = app.page.controls.iter().find(|rect| rect.node == neighbor).unwrap();
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
fn base_class_actual_app_startup_and_trusted_button_paint_state_preserve_neighbor() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); } }
    let fixture = Fixture(std::env::temp_dir().join(format!("chezcat-base-class-app-{}-{}.html",
        std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
    std::fs::write(&fixture.0, r#"<!doctype html><html><head><title>Class pending</title><style>
html,body{margin:0;padding:0;background:#ffffff}
#neighbor{display:block;position:absolute;left:20px;top:20px;width:220px;height:32px;margin:0}
#host{position:absolute;left:280px;top:20px;width:180px;height:32px}
#advance{display:block;width:180px;height:32px;margin:0;background:#e3edf2;color:#0b2233}
#out{position:absolute;left:20px;top:100px;width:300px;height:40px;margin:0;padding:0;border:0;background:#ffffff;color:#123456;font-size:24px;line-height:32px}
#stable-link{display:block;position:absolute;left:20px;top:180px;width:100px;height:24px;background:#336699;color:white}
</style></head><body><form id=f><input id=neighbor name=neighbor value="neighbor seed"><div id=host></div></form>
<p id=out>Fallback</p><a id=stable-link href=#destination>Go</a><div id=destination></div><script>
class Counter {
 constructor(host,output,observed=new.target){
  if(observed!==Counter)throw 'genuine class construction';
  this.value=1;this.clicks=0;this.output=output;this.read=()=>this.value;this.target=()=>new.target;
  var receiver=this.receiver;if(receiver()!==undefined)throw 'class method must be strict';
  this.button=document.createElement('button');this.button.id='advance';this.button.setAttribute('name','advance');this.button.setAttribute('type','button');this.button.value='class payload';
  this.button.appendChild(document.createTextNode('Advance'));
  this.button.onclick=(event)=>{
   if(!event.isTrusted||this!==counter||event.currentTarget!==this.button||this.target()!==Counter)throw 'trusted button and lexical class instance';
   this.clicks++;this.advance();
  };
  host.appendChild(this.button);this.paint();
 }
 receiver(){return this;}
 advance(){this.value++;this.paint();}
 paint(){if(this.read()!==this.value||this.value!==this.clicks+1)throw 'persistent class state';this.output.textContent='Count'+this.value;document.title='ClassCount'+this.value;}
}
var counter=new Counter(document.getElementById('host'),document.getElementById('out'));
</script></body></html>"#).unwrap();
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    clean(&app);
    assert_eq!(app.document.title, "ClassCount1");
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    stable_hits(&app);
    let dynamic = node(&app, "advance");
    let button = &app.forms.controls[&dynamic];
    assert_eq!(button.kind, Kind::Button);
    assert_eq!(button.name, "advance");
    assert_eq!(button.value.text(), "class payload");
    assert_eq!(button.caption, "Advance");
    assert_eq!(button.owner, Some(node(&app, "f")));
    assert_eq!(app.forms.controls.len(), 2);
    let rect = app.page.controls.iter().find(|rect| rect.node == dynamic).unwrap();
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (280, 20, 180, 32));
    let canvas = app.draw();
    let top = (app.content_top() + 20 - app.scroll) as usize;
    assert!((top + 5..top + 27).any(|y|
        canvas.pixels[y * canvas.width + 285..y * canvas.width + 455].iter().any(|&pixel| pixel != 0xe3edf2)),
        "The native class-created button must paint its caption away from the border");

    click(&mut app, "neighbor");
    assert!(matches!(app.key(0x41, true, false, false), PlatformAction::None));
    app.paste("neighbor kept");
    assert!(matches!(app.key(0x24, false, false, false), PlatformAction::None));
    for _ in 0..2 { assert!(matches!(app.key(0x27, false, false, false), PlatformAction::None)); }
    for _ in 0..3 { assert!(matches!(app.key(0x27, false, true, false), PlatformAction::None)); }
    let preserved = editor(&app);
    assert_eq!((preserved.0.as_str(), preserved.1, preserved.2), ("neighbor kept", 5, 2));
    assert!(preserved.4);
    let initial = result_pixels(&app, "Count1");
    click(&mut app, "advance");
    assert_eq!(app.document.title, "ClassCount2");
    let second = result_pixels(&app, "Count2");
    assert_ne!(second, initial, "The trusted class callback must change the isolated text pixels");
    assert_eq!(editor(&app), preserved);
    stable_hits(&app);
    click(&mut app, "advance");
    assert_eq!(app.document.title, "ClassCount3");
    assert_ne!(result_pixels(&app, "Count3"), second, "Class state persists across real activation jobs");
    assert_eq!(editor(&app), preserved);
    stable_hits(&app);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    clean(&app);
}
