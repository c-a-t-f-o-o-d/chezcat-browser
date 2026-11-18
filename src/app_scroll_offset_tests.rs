//! Original App proof of live root scroll offsets and actual page movement.
use super::*;
use super::anchor::AnchorDefault;
use crate::scripting::AnchorMouseInput;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-scroll-offset-app-{}-{}.html",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, r#"<!doctype html><html><head><title>Scroll pending</title><style>
html,body{margin:0;padding:0;background:#ffffff}
body{height:1000px}
#neighbor{display:block;position:fixed;left:20px;top:20px;width:180px;height:32px;margin:0}
#action{display:block;position:fixed;left:220px;top:20px;width:140px;height:32px;margin:0;padding:0}
#box{position:absolute;left:400px;top:200px;width:200px;height:180px;overflow:visible;background:#336699;margin:0;padding:0;border:0}
#subject{display:block;position:absolute;left:20px;top:60px;width:80px;height:24px;background:#cc3300;margin:0;padding:0;border:0}
#top-marker{position:absolute;left:540px;top:0;width:50px;height:40px;background:#9933cc}
#bottom-marker{position:absolute;left:420px;top:900px;width:80px;height:40px;background:#228844}
</style></head><body><form id=f><input id=neighbor name=neighbor value="neighbor seed"><button id=action name=action type=button>Root scroll</button></form><div id=box><a id=subject href=#destination></a></div><div id=top-marker></div><div id=bottom-marker></div><div id=destination></div><script>
var root=document.documentElement,box=document.getElementById('box'),body=document.body;
var stage=0,linkClicks=0,action=document.getElementById('action');
function zeroOffsets(){
 if(body.scrollTop!==0||body.scrollLeft!==0||box.scrollTop!==0||box.scrollLeft!==0||root.scrollLeft!==0)throw 'non-scrollcontainer offsets must stay zero';
}
document.getElementById('subject').addEventListener('click',function(event){
 if(!event.isTrusted||event.currentTarget!==this)throw 'trusted link required';
 event.preventDefault();linkClicks++;document.title='Trusted_link';
});
action.onclick=function(event){
 if(!event.isTrusted||event.currentTarget!==this||this!==action||this.form!==document.getElementById('f'))throw 'trusted owned button required';
 zeroOffsets();
 if(stage===0){
  if(root.scrollTop!==72||linkClicks!==1)throw 'real wheel readback or trusted link';
  document.title='Wheel_read';
 }else if(stage===1){
  if(root.scrollTop!==120)throw 'real App scroll readback';
  root.scrollTop=240;document.title='Script_write';
 }else if(stage===2){
  if(root.scrollTop!==240)throw 'root write readback';
  root.scrollTop=1000000;document.title='Clamped_write';
 }else if(stage===3){
  if(root.scrollTop<=240)throw 'positive clamped readback';
  root.scrollTop=-500;document.title='Clamped_zero';
 }else throw 'unexpected trusted action';
 stage++;
};
document.title='Scroll_ready';
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
fn click_control(app: &mut App, name: &str) {
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
fn preserved_controls(app: &App) {
    let neighbor = id(app, "neighbor");
    let action = id(app, "action");
    assert_eq!(app.forms.controls.len(), 2);
    for (node, kind, name) in [(neighbor, Kind::Text, "neighbor"), (action, Kind::Button, "action")] {
        let control = &app.forms.controls[&node];
        assert_eq!(control.kind, kind);
        assert_eq!(control.name, name);
        assert_eq!(control.owner, Some(id(app, "f")));
        assert!(app.page.control_is_fixed(node));
        let rect = app.page.controls.iter().find(|rect| rect.node == node).unwrap();
        assert_eq!(rect.y - app.scroll, 20);
        assert_eq!(app.page.control_at(rect.x + 2, rect.y + 2), Some(node));
    }
    assert_eq!(app.forms.text_default_value(neighbor).unwrap(), "neighbor seed");
}
fn screen_pixel(canvas: &Canvas, x: i32, y: i32) -> u32 {
    assert!(x >= 0 && y >= 0 && (x as usize) < canvas.width && (y as usize) < canvas.height);
    canvas.pixels[y as usize * canvas.width + x as usize]
}
fn link_point(app: &App) -> (i32, i32) {
    let rect = app.page.border_rects[id(app, "subject")].unwrap();
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (420, 260, 80, 24));
    (rect.x + rect.width / 2, app.content_top() + rect.y + rect.height / 2 - app.scroll)
}
fn visible_link(app: &App) -> (i32, i32) {
    let point = link_point(app);
    assert!(point.1 >= app.content_top() && point.1 < app.content_bottom());
    assert_eq!(app.page.link_node_at(point.0, point.1 - app.content_top() + app.scroll), Some(id(app, "subject")));
    assert!(app.anchor_hit(point.0, point.1, 0).is_some());
    assert_eq!(screen_pixel(&app.draw(), point.0, point.1), 0xcc3300);
    point
}
fn readback(app: &mut App, expected: i32) {
    // Compare the real VM read against independently observed App scroll.
    assert_eq!(app.scroll, expected);
    assert!(expected >= 0);
    let source = format!(
        "if(document.documentElement.scrollTop!=={expected}||document.documentElement.scrollTop<0)throw 'live CSS pixel root read';zeroOffsets();"
    );
    let page = app.scripting.as_mut().unwrap();
    let result = page.console_eval(&mut app.document, &mut app.forms, &source);
    assert_eq!(result.errors, 0, "{}", page.report());
    assert!(!result.layout_changed, "Read-only offset access must not mutate layout");
    clean(app);
}

#[test]
fn scroll_offsets_actual_app_wheel_root_write_pixels_trusted_hit_preserve_editor() {
    let fixture = Fixture::new();
    let mut app = fixture.open();
    assert_eq!(app.document.title, "Scroll_ready");
    assert_eq!(app.zoom, 1.0);
    assert_eq!(app.scroll, 0);
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    preserved_controls(&app);
    let original_link = visible_link(&app);
    let top = app.content_top();
    let canvas = app.draw();
    assert_eq!(screen_pixel(&canvas, 550, top + 10), 0x9933cc);
    let chrome = screen_pixel(&canvas, 550, top - 1);
    let box_rect = app.page.border_rects[id(&app, "box")].unwrap();
    assert_eq!((box_rect.x, box_rect.y, box_rect.width, box_rect.height), (400, 200, 200, 180));
    assert!(app.max_scroll() > 240 && app.max_scroll() < 1_000_000);

    click_control(&mut app, "neighbor");
    assert!(matches!(app.key(0x41, true, false, false), PlatformAction::None));
    app.paste("neighbor kept");
    assert!(matches!(app.key(0x24, false, false, false), PlatformAction::None));
    for _ in 0..2 {
        assert!(matches!(app.key(0x27, false, false, false), PlatformAction::None));
    }
    for _ in 0..3 {
        assert!(matches!(app.key(0x27, false, true, false), PlatformAction::None));
    }
    clean(&app);
    let preserved = editor(&app);
    assert_eq!((preserved.0.as_str(), preserved.1, preserved.2), ("neighbor kept", 5, 2));
    assert!(preserved.4);

    app.wheel(-120, false);
    assert_eq!(app.scroll, 72);
    let moved_link = visible_link(&app);
    assert_eq!(moved_link, (original_link.0, original_link.1 - 72));
    let canvas = app.draw();
    assert_eq!(screen_pixel(&canvas, 550, top + 10), 0xffffff);
    assert_eq!(screen_pixel(&canvas, 550, top - 1), chrome);
    assert_eq!(screen_pixel(&canvas, original_link.0, original_link.1), 0x336699);
    assert!(app.anchor_hit(original_link.0, original_link.1, 0).is_none());
    let (action, hit) = app.primary_anchor_down(moved_link.0, moved_link.1, false);
    assert!(matches!(action, PlatformAction::None));
    let hit = hit.expect("Real primary down must capture the moved visible anchor");
    assert!(app.anchor_hit_valid(&hit, moved_link.0, moved_link.1, false, 0));
    app.pointer_cancel();
    let input = AnchorMouseInput {
        screen_x: f64::from(moved_link.0),
        screen_y: f64::from(moved_link.1),
        ..AnchorMouseInput::default()
    };
    assert!(matches!(
        app.activate_anchor(hit, moved_link.0, moved_link.1, false, input).unwrap(),
        AnchorDefault::Cancelled
    ));
    assert!(matches!(app.drag, Drag::None));
    clean(&app);
    assert_eq!(app.document.title, "Trusted_link");
    assert_eq!(app.scroll, 72);
    assert_eq!(editor(&app), preserved);
    click_control(&mut app, "action");
    assert_eq!(app.document.title, "Wheel_read");
    readback(&mut app, 72);
    preserved_controls(&app);

    app.scroll_by(48);
    assert_eq!(app.scroll, 120);
    let before_write = visible_link(&app);
    click_control(&mut app, "action");
    assert_eq!(app.document.title, "Script_write");
    readback(&mut app, 240);
    let after_write = visible_link(&app);
    assert_eq!(after_write, (before_write.0, before_write.1 - 120));
    assert!(app.anchor_hit(before_write.0, before_write.1, 0).is_none());
    assert_eq!(screen_pixel(&app.draw(), before_write.0, before_write.1), 0xffffff);
    assert_eq!(editor(&app), preserved);
    preserved_controls(&app);

    let max = app.max_scroll();
    click_control(&mut app, "action");
    assert_eq!(app.document.title, "Clamped_write");
    readback(&mut app, max);
    let clipped = link_point(&app);
    assert!(clipped.1 < app.content_top());
    assert!(app.anchor_hit(clipped.0, clipped.1, 0).is_none());
    let bottom = app.page.border_rects[id(&app, "bottom-marker")].unwrap();
    assert_eq!((bottom.x, bottom.y, bottom.width, bottom.height), (420, 900, 80, 40));
    let bottom_y = app.content_top() + bottom.y + 10 - app.scroll;
    assert!(bottom_y >= app.content_top() && bottom_y < app.content_bottom());
    assert_eq!(screen_pixel(&app.draw(), 450, bottom_y), 0x228844);
    assert_eq!(editor(&app), preserved);
    preserved_controls(&app);

    click_control(&mut app, "action");
    assert_eq!(app.document.title, "Clamped_zero");
    readback(&mut app, 0);
    assert_eq!(visible_link(&app), original_link);
    let canvas = app.draw();
    assert_eq!(screen_pixel(&canvas, 550, top + 10), 0x9933cc);
    assert_eq!(screen_pixel(&canvas, 550, top - 1), chrome);
    assert_eq!(editor(&app), preserved);
    preserved_controls(&app);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    clean(&app);
}

#[test]
fn scroll_offsets_actual_app_resources_timer_keeps_report_position_and_restores_scrolled_document() {
    let fixture = Fixture::new();
    let mut app = fixture.open();
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    preserved_controls(&app);
    visible_link(&app);
    click_control(&mut app, "neighbor");
    assert!(matches!(app.key(0x41, true, false, false), PlatformAction::None));
    app.paste("neighbor kept");
    clean(&app);
    let preserved = editor(&app);
    assert_eq!(preserved.0, "neighbor kept");
    assert!(preserved.4);
    app.scroll_by(72);
    readback(&mut app, 72);

    // This real timer changes no DOM or CSS. Its scroll effect must survive
    // the Resources snapshot refresh independently of report scrolling.
    let page = app.scripting.as_mut().unwrap();
    let outcome = page.console_eval(&mut app.document, &mut app.forms,
        "var resourceTimerFired=0;setTimeout(function(){document.documentElement.scrollTop=240;resourceTimerFired++;},0);");
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert!(!outcome.layout_changed);
    app.show_resources();
    assert!(matches!(app.view, View::Resources));
    assert!(app.max_scroll() >= 48);
    app.scroll_by(48);
    let report_scroll = app.scroll;
    assert_eq!(report_scroll, 48);
    assert_eq!(app.page_scroll, 72);
    assert!(app.poll_script_timer());
    clean(&app);
    assert!(matches!(app.view, View::Resources));
    assert_eq!(app.scroll, report_scroll);
    assert_eq!(app.page_scroll, 240);
    let page = app.scripting.as_mut().unwrap();
    let outcome = page.console_eval(&mut app.document, &mut app.forms,
        "if(resourceTimerFired!==1||document.documentElement.scrollTop!==240)throw 'Resources timer must retain live document scroll';zeroOffsets();");
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert!(!outcome.layout_changed);
    assert_eq!(editor(&app), preserved);

    app.show_resources();
    assert!(matches!(app.view, View::Page));
    readback(&mut app, 240);
    let link = visible_link(&app);
    assert_eq!(link.1, app.content_top() + 32);
    assert_eq!(editor(&app), preserved);
    preserved_controls(&app);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    clean(&app);
}

#[test]
fn scroll_offsets_actual_app_real_media_seek_callback_settles_before_later_wheel() {
    struct MediaSource(PathBuf);
    impl Drop for MediaSource {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let fixture = Fixture::new();
    let media_source = MediaSource(fixture.0.with_extension("ivf"));
    std::fs::write(&media_source.0, include_bytes!("../tools/fixtures/scene-intra.ivf")).unwrap();
    let source = std::fs::read_to_string(&fixture.0).unwrap();
    let source = source.replacen("</body>", &format!(
        "<video id=scroll-media src='{}' width=32 height=24 style='position:absolute;left:10px;top:600px'></video></body>",
        media_source.0.file_name().unwrap().to_str().unwrap()
    ), 1);
    std::fs::write(&fixture.0, source).unwrap();
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    app.wait_for_load();
    clean(&app);
    let player = app.media.as_ref().expect("Actual local VIDEO must be admitted");
    assert_eq!(player.node, id(&app, "scroll-media"));
    preserved_controls(&app);
    visible_link(&app);
    click_control(&mut app, "neighbor");
    assert!(matches!(app.key(0x41, true, false, false), PlatformAction::None));
    app.paste("neighbor kept");
    clean(&app);
    let preserved = editor(&app);
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());

    let page = app.scripting.as_mut().unwrap();
    let outcome = page.console_eval(&mut app.document, &mut app.forms, r#"
var scrollVideo=document.getElementById('scroll-media'),seekScrollCalls=0;
if(scrollVideo.readyState!==4||!scrollVideo.paused||scrollVideo.seeking||scrollVideo.currentTime!==0)throw 'real local media must be ready and paused';
scrollVideo.addEventListener('seeking',function(event){
 if(!event.isTrusted||event.target!==scrollVideo||event.currentTarget!==this||this!==scrollVideo)throw 'real media seek callback required';
 document.documentElement.scrollTop=240;seekScrollCalls++;
});
scrollVideo.currentTime=1.25;
if(!scrollVideo.seeking||!scrollVideo.paused||scrollVideo.currentTime!==1.25)throw 'real paused seek must be admitted';
"#);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert!(!outcome.layout_changed);
    assert!(app.scripting.as_ref().unwrap().has_media_tasks());
    let layout_before = app.stage_report().split_whitespace()
        .find(|field| field.starts_with("layout=")).unwrap().to_owned();
    let dom_before = app.scripting.as_ref().unwrap().dom_commit_generation();

    // The normal App timer path consumes the actual Seek control and emits
    // its media task. No manually queued event or fabricated decoder result.
    assert!(app.poll_page_timer());
    clean(&app);
    // Pending native media tasks can wake the event loop after this event.
    // The scroll must result from the trusted listener within this turn.
    assert!(app.scripting.as_ref().unwrap().report().lines().any(|line|
        line == "0 pending timer(s), 0 timer callback(s) executed."));
    assert_eq!(app.scripting.as_ref().unwrap().dom_commit_generation(), dom_before);
    assert_eq!(app.stage_report().split_whitespace()
        .find(|field| field.starts_with("layout=")).unwrap(), layout_before);
    readback(&mut app, 240);
    visible_link(&app);
    let page = app.scripting.as_mut().unwrap();
    let outcome = page.console_eval(&mut app.document, &mut app.forms,
        "if(seekScrollCalls!==1)throw 'seek callback must run once before input';");
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert!(!outcome.layout_changed);

    app.wheel(-120, false);
    readback(&mut app, 312);
    app.poll_load();
    readback(&mut app, 312);
    let old_link = link_point(&app);
    assert!(old_link.1 < app.content_top());
    assert!(app.anchor_hit(old_link.0, old_link.1, 0).is_none());
    assert_eq!(screen_pixel(&app.draw(), 460, app.content_top() + 10), 0x336699);
    assert_eq!(editor(&app), preserved);
    preserved_controls(&app);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    clean(&app);
}
