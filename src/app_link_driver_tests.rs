//! Rendered link actions must use the native guarded primary dispatch seam.
use super::*;
use std::path::PathBuf;

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        let mut random = [0u8; 8];
        crate::random::fill(&mut random).unwrap();
        let suffix: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let root = std::env::temp_dir().join(format!("chezcat-link-driver-{suffix}"));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(
            root.join("target.html"),
            "<!doctype html><title>Link target</title><p id=destination>Destination</p>",
        )
        .unwrap();
        std::fs::write(root.join("source.html"),r#"<!doctype html><html><head><title>Link ready</title><style>
body{margin:0}a{display:block;width:180px;height:32px}#marker{position:absolute;left:20px;top:180px;width:40px;height:20px;background:#336699}#marker.done{background:#22aa44}
</style></head><body><a id=cancel href=#never>Cancel original</a><a id=ordinary href=target.html#destination>Ordinary original</a><input id=neighbor name=neighbor value=seed><p id=out>Waiting</p><div id=marker></div><script>
var cancel=document.getElementById('cancel'),ordinary=document.getElementById('ordinary');
var out=document.getElementById('out'),trace='',calls=0,observed=null;
window.addEventListener('click',function(e){trace+='W';},true);
document.addEventListener('click',function(e){trace+='D';},true);
document.body.addEventListener('click',function(e){trace+='B';});
function record(e,node){
 calls++;trace+='A';
 if(this!==node||e.target!==node||e.currentTarget!==node)throw 'real original anchor receiver';
 observed=[e.type,e.isTrusted,e.button,e.buttons,e.ctrlKey,e.shiftKey,e.altKey,e.metaKey,e.clientX,e.clientY,e.screenX,e.screenY];
}
cancel.onclick=function(e){record.call(this,e,cancel);e.preventDefault();out.textContent='Clicked marker';document.getElementById('marker').className='done';};
ordinary.onclick=function(e){record.call(this,e,ordinary);};
</script></body></html>"#).unwrap();
        Self(root)
    }
    fn app(&self) -> App {
        let mut app = App::new();
        app.resize(640, 480);
        app.set_javascript_enabled(true);
        app.open(&self.0.join("source.html"));
        assert!(!app.failed && !app.is_loading(), "{}", app.status);
        clean(&app);
        app
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn id(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len())
        .find(|&node| app.document.attr(node, "id") == Some(name))
        .unwrap()
}
fn clean(app: &App) {
    let page = app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(), 0, "{}", page.report());
    assert!(!page.has_navigation_request());
}
fn eval(app: &mut App, source: &str) {
    let result =
        app.scripting
            .as_mut()
            .unwrap()
            .console_eval(&mut app.document, &mut app.forms, source);
    assert_eq!(
        result.errors,
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    if result.layout_changed {
        app.reflow();
    }
}
fn log_point(log: &str) -> (i32, i32) {
    let line = log
        .lines()
        .find(|line| line.starts_with("Pointer link text="))
        .unwrap();
    let get = |key: &str| {
        line.split_whitespace()
            .find_map(|word| word.strip_prefix(key))
            .unwrap()
            .parse::<i32>()
            .unwrap()
    };
    (get("x="), get("y="))
}
fn marker_pixel(app: &App) -> u32 {
    let rect = app.page.border_rects[id(app, "marker")].unwrap();
    let canvas = app.draw();
    let x = rect.x + 3;
    let y = app.content_top() + rect.y - app.scroll + 3;
    canvas.pixels[y as usize * canvas.width + x as usize]
}

#[test]
fn link_driver_primary_cancel_runs_real_capture_target_bubble_once_and_keeps_effects() {
    let files = Files::new();
    let mut app = files.app();
    let source = app.location.clone();
    let raw = app.document_source().to_owned();
    let neighbor = id(&app, "neighbor");
    app.forms
        .script_set_value(neighbor, "edited neighbor")
        .unwrap();
    let value = &mut app.forms.controls.get_mut(&neighbor).unwrap().value;
    value.cursor = 2;
    value.anchor = 5;
    value.offset = 1;
    let before = (
        value.chars.clone(),
        value.cursor,
        value.anchor,
        value.offset,
    );
    assert_eq!(marker_pixel(&app), 0x336699);
    let mut log = Vec::new();
    app.input_click_link("Cancel", false, &mut log).unwrap();
    let log = String::from_utf8(log).unwrap();
    let (x, y) = log_point(&log);
    assert!(log.contains("Primary link outcome=cancelled"), "{log}");
    assert!(
        log.contains("button=0 modifiers=none software_screen_x="),
        "{log}"
    );
    assert_eq!(app.location, source);
    assert_eq!(app.document_source(), raw);
    assert!(matches!(app.drag, super::super::Drag::None));
    assert_eq!(marker_pixel(&app), 0x22aa44);
    let client_x = f64::from(x) / f64::from(app.zoom);
    let client_y = f64::from(y - app.content_top()) / f64::from(app.zoom);
    eval(
        &mut app,
        &format!(
            r#"
        if(calls!==1||trace!=='WDAB'||out.textContent!=='Clicked marker'||
           observed[0]!=='click'||observed[1]!==true||observed[2]!==0||observed[3]!==0||
           observed[4]||observed[5]||observed[6]||observed[7]||
           observed[8]!=={}||observed[9]!=={}||observed[10]!=={x}||observed[11]!=={y})
            throw 'software/native shared mouse delivery';
    "#,
            client_x, client_y
        ),
    );
    let value = &app.forms.controls[&neighbor].value;
    assert_eq!(
        (
            value.chars.clone(),
            value.cursor,
            value.anchor,
            value.offset
        ),
        before
    );
    assert!(app.activity_report().contains("matched_press=1"));
    clean(&app);
}

#[test]
fn link_driver_primary_default_navigates_current_after_author_callback_and_href_effects() {
    let files = Files::new();
    let mut app = files.app();
    let source_root = app.location.root.clone();
    // Author behavior is installed normally before input. Default must follow
    // this same original node's live href after its callback and microtasks.
    eval(
        &mut app,
        r#"
        ordinary.href='target.html#before';
        ordinary.onclick=function(e){record.call(this,e,ordinary);
            Promise.resolve().then(function(){ordinary.href='target.html#destination';
                document.title='Callback ran';});};
    "#,
    );
    let mut log = Vec::new();
    app.input_click_link("target.html", true, &mut log).unwrap();
    app.wait_for_load();
    assert!(!app.failed, "{}", app.status);
    let log = String::from_utf8(log).unwrap();
    assert!(log.contains("Primary link outcome=current"), "{log}");
    assert_eq!(app.document.title, "Link target");
    assert_eq!(app.location.fragment, "destination");
    assert_eq!(app.location.root, source_root);
    assert!(app
        .location
        .path
        .as_ref()
        .is_some_and(|path| path.ends_with("target.html")));
    assert!(matches!(app.drag, super::super::Drag::None));
    clean(&app);
}

#[test]
fn link_driver_primary_preserves_ordinary_capture_overrides_and_missing_target_refusal() {
    let files = Files::new();
    let mut app = files.app();
    let source = app.location.clone();
    eval(
        &mut app,
        r#"
        cancel.onclick=null;
        document.addEventListener('click',function(e){
            if(e.target===cancel){e.preventDefault();out.textContent='Capture canceled';}
        },true);
    "#,
    );
    let mut log = Vec::new();
    app.input_click_link("#never", true, &mut log).unwrap();
    eval(&mut app,"if(calls!==0||trace!=='WDB'||out.textContent!=='Capture canceled')throw 'ordinary capture or removed handler';");
    assert_eq!(app.location, source);
    assert!(String::from_utf8(log)
        .unwrap()
        .contains("outcome=cancelled"));
    let mut missing = Vec::new();
    assert!(app.input_click_link("#absent", true, &mut missing).is_err());
    assert!(missing.is_empty());
    eval(
        &mut app,
        "if(calls!==0||trace!=='WDB')throw 'missing target dispatched';",
    );
    clean(&app);
}
