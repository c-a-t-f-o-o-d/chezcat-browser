//! Original App translation proof through real CSSOM, paint and trusted input.
use super::*;
use super::anchor::AnchorDefault;
use crate::scripting::AnchorMouseInput;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-translate-app-{}-{}.html", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, r#"<!doctype html><html><head><title>Translate pending</title><style>
html,body,form{margin:0;padding:0;border:0;background:#ffffff}
#neighbor{display:block;position:fixed;left:20px;top:20px;width:180px;height:32px;margin:0}
#change{display:block;position:fixed;left:220px;top:20px;width:160px;height:32px;margin:0;padding:0}
#flow{display:block;width:100px;height:24px;margin:100px 0 0 20px;padding:0;border:0;background:#228844}
#menu{position:absolute;left:220px;top:160px;width:180px;height:80px;margin:0;padding:0;border:0;background:#336699}
#subject{display:block;position:absolute;left:10px;top:10px;width:60px;height:20px;margin:0;padding:0;border:0;background:#cc3300}
#choice{display:block;position:absolute;left:90px;top:10px;width:80px;height:28px;margin:0;padding:0}
#menu.sheet{transform:translate(30px, 20px)}
#menu.sheet #subject{transform:translate(7px, 3px)}
</style></head><body><div id=flow></div><form id=f><input id=neighbor name=neighbor value="neighbor seed"><button id=change name=change type=button>Translate menu</button><div id=menu><a id=subject href=#destination></a><button id=choice name=choice type=button>Choose</button></div></form><div id=destination></div><script>
var menu=document.getElementById('menu'),subject=document.getElementById('subject'),changes=0,links=0,choices=0;
function measured(x,y,lx,ly){
 if(menu.offsetLeft!==220||menu.offsetTop!==160||subject.offsetLeft!==10||subject.offsetTop!==10||subject.offsetParent!==menu)throw 'translation changed layout offsets';
 var m=menu.getBoundingClientRect(),s=subject.getBoundingClientRect(),flow=document.getElementById('flow').getBoundingClientRect();
 if(m.x!==x||m.y!==y||m.width!==180||m.height!==80||s.x!==lx||s.y!==ly||s.width!==60||s.height!==20)throw 'translation missing from actual DOMRect';
 if(flow.x!==20||flow.y!==100||flow.width!==100||flow.height!==24)throw 'translation moved normal flow';
 if(subject.parentNode!==menu||document.getElementById('choice').form!==document.getElementById('f'))throw 'translation changed native ownership';
}
document.getElementById('change').onclick=function(event){
 if(!event.isTrusted||event.currentTarget!==this||this.form!==document.getElementById('f'))throw 'trusted owned button required';
 if(changes===0){
  var before=menu.getBoundingClientRect();if(before.x!==220||before.y!==160)throw 'initial rendered menu';
  Object.assign(menu.style,{transform:'translate(100px, 40px)'});
  measured(320,200,330,210);document.title='Translated';
 }else if(changes===1){
  if(menu.style.removeProperty('transform')!=='translate(100px, 40px)')throw 'clear must remove the authored transform';
  measured(220,160,230,170);document.title='Cleared';
 }else if(changes===2){
  menu.className='sheet';measured(250,180,267,193);document.title='Sheet';
 }else if(changes===3){
  menu.className='';measured(220,160,230,170);document.title='Restored';
 }else throw 'unexpected activation';
 changes++;
};
subject.addEventListener('click',function(event){
 if(!event.isTrusted||event.currentTarget!==this||this!==subject)throw 'trusted moved link required';
 event.preventDefault();links++;document.title='Link'+links;
});
document.getElementById('choice').onclick=function(event){
 if(!event.isTrusted||event.currentTarget!==this||this.form!==document.getElementById('f'))throw 'trusted moved native choice required';
 choices++;document.title='Choice'+choices;
};
document.title='Translate_ready';
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
fn click_control(app: &mut App, name: &str) {
    let node = id(app, name);
    let r = app.page.controls.iter().find(|r| r.node == node).unwrap();
    let (x, page_y) = (r.x + r.width / 2, r.y + r.height / 2);
    assert_eq!(app.page.control_at(x, page_y), Some(node));
    let y = app.content_top() + page_y - app.scroll;
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    clean(app);
}
fn click_link(app: &mut App, point: (i32, i32)) {
    let (action, hit) = app.primary_anchor_down(point.0, point.1, false);
    assert!(matches!(action, PlatformAction::None));
    let hit = hit.expect("Translated visible link must admit the actual down");
    assert!(app.anchor_hit_valid(&hit, point.0, point.1, false, 0));
    app.pointer_cancel();
    let input = AnchorMouseInput { screen_x: f64::from(point.0), screen_y: f64::from(point.1), ..AnchorMouseInput::default() };
    assert!(matches!(app.activate_anchor(hit, point.0, point.1, false, input).unwrap(), AnchorDefault::Cancelled));
    assert!(matches!(app.drag, Drag::None));
    clean(app);
}
fn editor(app: &App) -> (String, usize, usize, usize, bool, Option<usize>) {
    let c = &app.forms.controls[&id(app, "neighbor")];
    (c.value.text(), c.value.cursor, c.value.anchor, c.value.offset, c.dirty_value, c.owner)
}
fn native_controls(app: &App) {
    assert_eq!(app.forms.controls.len(), 3);
    for (name, kind) in [("neighbor", Kind::Text), ("change", Kind::Button), ("choice", Kind::Button)] {
        let c = &app.forms.controls[&id(app, name)];
        assert_eq!(c.kind, kind);
        assert_eq!(c.name, name);
        assert_eq!(c.owner, Some(id(app, "f")));
    }
    assert_eq!(app.forms.text_default_value(id(app, "neighbor")).unwrap(), "neighbor seed");
    assert_eq!(app.page.control_at(110, 36), Some(id(app, "neighbor")));
}
fn pixel(app: &App, x: i32, page_y: i32) -> u32 {
    let canvas = app.draw();
    let y = app.content_top() + page_y - app.scroll;
    assert!(x >= 0 && y >= 0 && (x as usize) < canvas.width && (y as usize) < canvas.height);
    canvas.pixels[y as usize * canvas.width + x as usize]
}
fn scene(app: &App, menu: (i32, i32), link: (i32, i32)) -> (i32, i32) {
    for (name, raw_origin, visual_origin, width, height) in [
        ("menu", (220, 160), menu, 180, 80),
        ("subject", (230, 170), link, 60, 20),
        ("flow", (20, 100), (20, 100), 100, 24),
        ("choice", (310, 170), (menu.0 + 90, menu.1 + 10), 80, 28),
    ] {
        let node = id(app, name);
        let raw = app.page.border_rects[node].unwrap();
        assert_eq!((raw.x, raw.y), raw_origin, "{name} layout origin");
        assert_eq!((raw.width, raw.height), (width, height), "{name} layout size");
        if app.page.visual_border_rects.is_empty() {
            assert_eq!(raw_origin, visual_origin, "{name} untransformed origin");
        } else {
            assert!(!app.page.visual_unavailable[node], "{name} visual geometry");
            let visual = app.page.visual_border_rects[node].unwrap();
            assert_eq!((visual.x, visual.y), (f64::from(visual_origin.0), f64::from(visual_origin.1)), "{name} visual origin");
            assert_eq!((visual.width, visual.height), (f64::from(width), f64::from(height)), "{name} visual size");
        }
    }
    assert_eq!(pixel(app, 30, 110), 0x228844);
    assert_eq!(pixel(app, menu.0 + 5, menu.1 + 5), 0x336699);
    for (dx, dy) in [(0, 0), (30, 10), (59, 19)] {
        assert_eq!(pixel(app, link.0 + dx, link.1 + dy), 0xcc3300);
        assert_eq!(app.page.link_node_at(link.0 + dx, link.1 + dy), Some(id(app, "subject")));
    }
    assert_eq!(app.page.link_node_at(link.0 + 60, link.1 + 10), None);
    assert_eq!(app.page.control_at(menu.0 + 130, menu.1 + 24), Some(id(app, "choice")));
    let point = (link.0 + 30, app.content_top() + link.1 + 10 - app.scroll);
    assert!(app.anchor_hit(point.0, point.1, 0).is_some());
    point
}

#[test]
fn translate_transform_actual_app_assign_clear_stylesheet_nested_pixels_hits_and_stable_offsets() {
    let fixture = Fixture::new();
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    clean(&app);
    assert_eq!(app.document.title, "Translate_ready");
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    native_controls(&app);
    let initial = scene(&app, (220, 160), (230, 170));
    click_control(&mut app, "neighbor");
    assert!(matches!(app.key(0x41, true, false, false), PlatformAction::None));
    app.paste("neighbor kept");
    assert!(matches!(app.key(0x24, false, false, false), PlatformAction::None));
    for _ in 0..2 { assert!(matches!(app.key(0x27, false, false, false), PlatformAction::None)); }
    for _ in 0..3 { assert!(matches!(app.key(0x27, false, true, false), PlatformAction::None)); }
    let preserved = editor(&app);
    assert_eq!((preserved.0.as_str(), preserved.1, preserved.2), ("neighbor kept", 5, 2));
    assert!(preserved.4);

    // First new capability write: Object.assign to genuine element.style.
    click_control(&mut app, "change");
    assert_eq!(app.document.title, "Translated");
    let translated = scene(&app, (320, 200), (330, 210));
    assert_eq!(pixel(&app, 260, 180), 0xffffff);
    assert!(app.anchor_hit(initial.0, initial.1, 0).is_none());
    assert_eq!(app.page.control_at(350, 184), None);
    click_link(&mut app, translated);
    assert_eq!(app.document.title, "Link1");
    click_control(&mut app, "choice");
    assert_eq!(app.document.title, "Choice1");
    assert_eq!(editor(&app), preserved);
    native_controls(&app);

    click_control(&mut app, "change");
    assert_eq!(app.document.title, "Cleared");
    assert_eq!(scene(&app, (220, 160), (230, 170)), initial);
    assert_eq!(pixel(&app, 360, 220), 0x336699);
    assert!(app.anchor_hit(translated.0, translated.1, 0).is_none());
    assert_eq!(app.page.control_at(450, 224), None);

    click_control(&mut app, "change");
    assert_eq!(app.document.title, "Sheet");
    let nested = scene(&app, (250, 180), (267, 193));
    assert_eq!(pixel(&app, 260, 180), 0x336699);
    assert!(app.anchor_hit(initial.0, initial.1, 0).is_none());
    click_link(&mut app, nested);
    assert_eq!(app.document.title, "Link2");
    click_control(&mut app, "choice");
    assert_eq!(app.document.title, "Choice2");
    assert_eq!(editor(&app), preserved);

    click_control(&mut app, "change");
    assert_eq!(app.document.title, "Restored");
    assert_eq!(scene(&app, (220, 160), (230, 170)), initial);
    assert_eq!(pixel(&app, 297, 203), 0x336699);
    assert!(app.anchor_hit(nested.0, nested.1, 0).is_none());
    assert_eq!(editor(&app), preserved);
    native_controls(&app);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    clean(&app);
}
