//! Original App software-input/paint proof for the finite literal prerequisite.
//! This does not exercise a physical desktop or initialize a framework.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

const MARKUP: &str = r#"<!doctype html><html><head><title>Null bag ready</title>
<style>html,body{margin:0;background:#ffffff}#go{position:absolute;left:20px;top:20px;width:160px;height:32px}#out{position:absolute;left:20px;top:80px;width:240px;height:40px;margin:0;padding:0;border:0;background:#ffffff;color:#123456;font-size:24px;line-height:32px}</style>
</head><body><button id=go type=button>Update bag</button><p id=out>I</p><script>
var button=document.getElementById('go'),bag,alias,ordinary={marker:7};
button.onclick=function(){
    if(this!==button)throw 'actual button receiver';
    if(bag===undefined){bag={__proto__:null,count:0,label:''};alias=bag;}
    if(bag!==alias||Object.getPrototypeOf(bag)!==null||bag.toString!==undefined||('toString' in bag))throw 'retained null root';
    if(Object.getPrototypeOf(ordinary)!==Object.prototype||ordinary.marker!==7||typeof ordinary.toString!=='function')throw 'ordinary object changed';
    bag.count++;bag.label=bag.count===1?'WWW':'III';
    if(Object.keys(bag).join()!=='count,label'||!Object.hasOwn(bag,'count')||!Object.hasOwn(bag,'label'))throw 'ordinary own fields';
    var descriptor=Object.getOwnPropertyDescriptor(bag,'count');
    if(descriptor.value!==bag.count||!descriptor.writable||!descriptor.enumerable||!descriptor.configurable||Object.getPrototypeOf(bag)!==null)throw 'ordinary field descriptor';
    document.getElementById('out').textContent=bag.label;
    document.title='Null bag '+bag.count;
};
</script></body></html>"#;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-literal-prototype-app-{}-{}.html",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, MARKUP).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
}

fn node(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len())
        .find(|&node| app.document.attr(node, "id") == Some(name)).unwrap()
}

fn output_text(app: &App, output: usize) -> String {
    app.document.nodes[output].children.iter().filter_map(|&child| {
        if let html::NodeKind::Text(text) = &app.document.nodes[child].kind {
            Some(text.as_str())
        } else { None }
    }).collect()
}

fn text_pixels(app: &mut App, output: usize, expected: &str) -> Vec<u32> {
    assert_eq!(output_text(app, output), expected);
    let rect = app.page.border_rects[output].expect("Output owns a real layout box");
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (20, 80, 240, 40));
    let runs: Vec<_> = app.page.runs.iter().filter(|run| {
        run.x >= rect.x && run.x < rect.x + rect.width
            && run.y >= rect.y && run.y < rect.y + rect.height
    }).collect();
    assert_eq!(runs.iter().map(|run| run.text.as_str()).collect::<Vec<_>>(), [expected]);
    assert!(runs.iter().all(|run| run.color == 0x123456));
    let left = rect.x as usize;
    let top = (app.content_top() + rect.y - app.scroll) as usize;
    let canvas = app.draw();
    assert!(left + rect.width as usize <= canvas.width);
    assert!(top + rect.height as usize <= canvas.height);
    let pixels: Vec<_> = (top..top + rect.height as usize).flat_map(|y| {
        canvas.pixels[y * canvas.width + left..y * canvas.width + left + rect.width as usize].iter().copied()
    }).collect();
    assert!(pixels.iter().any(|&pixel| pixel != 0xffffff), "Text must really paint in the isolated white region");
    assert_eq!(pixels.last(), Some(&0xffffff));
    pixels
}

#[test]
fn literal_prototype_actual_app_pointer_click_keeps_null_bag_ordinary_fields_and_changes_text_pixels() {
    let fixture = Fixture::new();
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0, "{:?}", app.events);
    assert_eq!(app.document.title, "Null bag ready");
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history_index = app.history_index;
    let history_len = app.history.len();
    let epoch = app.script_epoch;
    let output = node(&app, "out");
    let button = node(&app, "go");
    assert!(matches!(app.forms.controls[&button].kind, Kind::Button));
    let rect = app.page.controls.iter().find(|rect| rect.node == button)
        .expect("Authored button must be an actual App hit target");
    let x = rect.x + rect.width / 2;
    let y = app.content_top() + rect.y + rect.height / 2 - app.scroll;
    assert!(x >= 0 && x < app.width && y >= app.content_top() && y < app.content_bottom());
    assert_eq!(app.page.control_at(x, y - app.content_top() + app.scroll), Some(button));
    let initial = text_pixels(&mut app, output, "I");

    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert_eq!(output_text(&app, output), "I", "Button down alone must not run click");
    assert_eq!(app.document.title, "Null bag ready");
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0, "{:?}", app.events);
    assert_eq!(app.document.title, "Null bag 1");
    let first = text_pixels(&mut app, output, "WWW");
    assert_ne!(initial, first, "Text pixels must change independently of DOM/layout assertions; no region background or style changed");

    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert_eq!(output_text(&app, output), "WWW");
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0, "{:?}", app.events);
    assert_eq!(app.document.title, "Null bag 2", "The same null-root state survives another actual input job");
    let second = text_pixels(&mut app, output, "III");
    assert_ne!(first, second);
    assert!(!app.failed && !app.is_loading());
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!(app.history_index, history_index);
    assert_eq!(app.history.len(), history_len);
    assert_eq!(app.script_epoch, epoch);
}
