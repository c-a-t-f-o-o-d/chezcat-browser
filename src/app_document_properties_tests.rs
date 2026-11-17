//! Actual Document-owned state across trusted App jobs and isolated paint.
//! Literal fixture data only; no desktop control, clipboard or network use.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn node(app: &App, id: &str) -> usize {
    (0..app.document.nodes.len()).find(|&n| app.document.attr(n, "id") == Some(id)).unwrap()
}
fn clean(app: &App) {
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let page = app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(), 0, "{}", page.report());
    assert!(!page.has_navigation_request());
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
    assert!(result.iter().any(|&pixel| pixel == 0x123456));
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
fn document_own_properties_actual_app_state_and_callable_paint_across_trusted_jobs() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); } }
    let fixture = Fixture(std::env::temp_dir().join(format!("chezcat-document-properties-app-{}-{}.html",
        std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
    std::fs::write(&fixture.0, r#"<!doctype html><html><head><title>Document pending</title><style>
html,body{margin:0;padding:0;background:#ffffff}
#neighbor{display:block;position:absolute;left:20px;top:20px;width:220px;height:32px;margin:0}
#advance{display:block;position:absolute;left:280px;top:20px;width:180px;height:32px;margin:0;background:#e3edf2;color:#0b2233}
#out{position:absolute;left:20px;top:100px;width:300px;height:40px;margin:0;padding:0;border:0;background:#ffffff;color:#123456;font-size:24px;line-height:32px}
#stable-link{display:block;position:absolute;left:20px;top:180px;width:100px;height:24px;background:#336699;color:white}
</style></head><body><form id=f name=ordinaryFixtureForm><input id=neighbor name=neighbor value="neighbor seed"><button id=advance name=advance type=button value=payload>Advance</button></form>
<p id=out>Fallback</p><a id=stable-link href=#destination>Go</a><div id=destination></div><script>
var nativeBody=document.body,nativeForm=document.getElementById('f');
if(document.ordinaryFixtureForm!==nativeForm)throw 'native named form prefix';
document.fixtureCounter=1;
document.fixtureState={count:1,owner:document};
Object.defineProperty(document,'fixtureLabel',{value:'DocumentCount',writable:true,enumerable:true,configurable:true});
document.fixturePaint=function(){
 if(this!==document||this.fixtureState.owner!==document||this.fixtureState.count!==this.fixtureCounter||this.body!==nativeBody||this.ordinaryFixtureForm!==nativeForm)throw 'genuine Document own state and native fallback';
 var descriptor=Object.getOwnPropertyDescriptor(this,'fixtureCounter');
 if(!descriptor||descriptor.value!==this.fixtureCounter||!descriptor.writable||!descriptor.enumerable||!descriptor.configurable||!Object.hasOwn(this,'fixtureState'))throw 'ordinary Document descriptor';
 this.getElementById('out').textContent='Count'+this.fixtureCounter;
 this.title=this.fixtureLabel+this.fixtureCounter;
};
document.getElementById('advance').onclick=function(event){
 if(!event.isTrusted||this!==document.getElementById('advance')||event.currentTarget!==this||this.form!==nativeForm)throw 'trusted native Document job';
 document.fixtureCounter++;document.fixtureState.count=document.fixtureCounter;document.fixturePaint();
};
document.fixturePaint();
</script></body></html>"#).unwrap();
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    clean(&app);
    assert_eq!(app.document.title, "DocumentCount1");
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    let stable_nodes = (node(&app, "neighbor"), node(&app, "stable-link"), node(&app, "out"));
    let controls: Vec<_> = app.forms.controls.keys().copied().collect();
    stable_hits(&app);
    let button = &app.forms.controls[&node(&app, "advance")];
    assert_eq!(button.kind, Kind::Button);
    assert_eq!(button.value.text(), "payload");
    assert_eq!(button.owner, Some(node(&app, "f")));
    click(&mut app, "neighbor");
    assert!(matches!(app.key(0x41, true, false, false), PlatformAction::None));
    app.paste("neighbor kept");
    assert!(matches!(app.key(0x24, false, false, false), PlatformAction::None));
    for _ in 0..2 { assert!(matches!(app.key(0x27, false, false, false), PlatformAction::None)); }
    for _ in 0..3 { assert!(matches!(app.key(0x27, false, true, false), PlatformAction::None)); }
    let preserved = editor(&app);
    assert_eq!((preserved.0.as_str(), preserved.1, preserved.2), ("neighbor kept", 5, 2));
    assert!(preserved.4);
    let first = pixels(&app, "Count1");
    click(&mut app, "advance");
    assert_eq!(app.document.title, "DocumentCount2");
    let second = pixels(&app, "Count2");
    assert_ne!(second, first);
    assert_eq!(editor(&app), preserved);
    stable_hits(&app);
    click(&mut app, "advance");
    assert_eq!(app.document.title, "DocumentCount3");
    assert_ne!(pixels(&app, "Count3"), second);
    assert_eq!(editor(&app), preserved);
    assert_eq!(app.forms.text_default_value(node(&app, "neighbor")).unwrap(), "neighbor seed");
    assert_eq!(app.forms.controls.keys().copied().collect::<Vec<_>>(), controls);
    assert_eq!((node(&app, "neighbor"), node(&app, "stable-link"), node(&app, "out")), stable_nodes);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    stable_hits(&app);
    clean(&app);
}
