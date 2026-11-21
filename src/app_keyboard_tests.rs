use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> (Self, App) {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "chezcat-keys-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("keys.html");
        std::fs::write(&path, source).unwrap();
        let mut app = App::new();
        app.open(&path);
        assert!(!app.failed, "{}", app.status);
        (Self(directory), app)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn input(down: bool, key: &str, code: &str, key_code: u32) -> KeyboardInput {
    KeyboardInput {
        down,
        key: key.into(),
        code: code.into(),
        key_code,
        ..KeyboardInput::default()
    }
}
fn node(app: &App, id: &str) -> usize {
    app.document
        .nodes
        .iter()
        .enumerate()
        .find_map(|(node, _)| (app.document.attr(node, "id") == Some(id)).then_some(node))
        .unwrap()
}
fn attr<'a>(app: &'a App, id: &str, name: &str) -> Option<&'a str> {
    app.document.attr(node(app, id), name)
}

#[test]
fn tab_index_html_prefix_values_drive_actual_app_keyboard_order() {
    let (_fixture, mut app) = Fixture::new("<body><input id=third tabindex=' +3tail'><input id=first tabindex='1e3'><input id=skip tabindex=' -1tail'><input id=zero tabindex=invalid><input id=disabled disabled tabindex=0><script>document.title=document.getElementById('third').tabIndex+','+document.getElementById('first').tabIndex+','+document.getElementById('skip').tabIndex+','+document.getElementById('zero').tabIndex;</script></body>");
    assert_eq!(app.document.title, "3,1,-1,0");
    app.focus = Focus::Page;
    for id in ["first", "third", "zero"] {
        app.key_event(input(true, "Tab", "Tab", 0x09));
        assert!(app.focus == Focus::Control(node(&app, id)), "{id}");
    }
    let mut previous = input(true, "Tab", "Tab", 0x09);
    previous.shift = true;
    app.key_event(previous);
    assert!(app.focus == Focus::Control(node(&app, "third")));
}

#[test]
fn prevented_page_arrows_change_live_dom_without_scrolling_and_still_receive_keyup() {
    let (_fixture, mut app) = Fixture::new("<body><div id=box style='position:relative;left:0px;width:20px;height:20px;background:red'></div><div style='height:1800px'></div><script>var x=0; document.addEventListener('keydown',function(e){ if(e.key === 'ArrowRight'){e.preventDefault(); x+=10; document.getElementById('box').style.left=x+'px';} if(e.key === 'ArrowDown'){e.preventDefault(); document.getElementById('box').setAttribute('data-down','yes');}}); document.addEventListener('keyup',function(e){document.getElementById('box').setAttribute('data-up', e.key);});</script></body>");
    let (_, canceled) = app.key_event(input(true, "ArrowRight", "ArrowRight", 0x27));
    assert!(canceled);
    assert!(app
        .document
        .attr(node(&app, "box"), "style")
        .unwrap()
        .contains("left:10px"));
    assert!(app.key_event(input(true, "ArrowDown", "ArrowDown", 0x28)).1);
    assert_eq!(app.scroll, 0);
    assert_eq!(attr(&app, "box", "data-down"), Some("yes"));
    assert!(
        !app.key_event(input(false, "ArrowDown", "ArrowDown", 0x28))
            .1
    );
    assert_eq!(attr(&app, "box", "data-up"), Some("ArrowDown"));
}

#[test]
fn unprevented_keyboard_keeps_default_scrolling_and_reports_native_metadata() {
    let (_fixture, mut app) = Fixture::new("<body><div id=report></div><div style='height:1800px'></div><script>document.addEventListener('keydown', function(e){var r=document.getElementById('report');r.setAttribute('data-key', e.key);r.setAttribute('data-code',e.code);r.setAttribute('data-repeat',e.repeat);r.setAttribute('data-shift',e.shiftKey);r.setAttribute('data-location',e.location);r.setAttribute('data-trusted',e.isTrusted);});</script></body>");
    assert!(!app.key_event(input(true, "ArrowDown", "ArrowDown", 0x28)).1);
    assert!(app.scroll > 0);
    let mut key = input(true, "A", "KeyQ", 0x41);
    key.shift = true;
    key.repeat = true;
    app.key_event(key);
    assert_eq!(attr(&app, "report", "data-key"), Some("A"));
    assert_eq!(attr(&app, "report", "data-code"), Some("KeyQ"));
    assert_eq!(attr(&app, "report", "data-repeat"), Some("true"));
    assert_eq!(attr(&app, "report", "data-shift"), Some("true"));
    assert_eq!(attr(&app, "report", "data-location"), Some("0"));
    assert_eq!(attr(&app, "report", "data-trusted"), Some("true"));
}

#[test]
fn native_text_is_suppressed_only_when_the_focused_form_cancels_that_keydown() {
    let (_fixture, mut app) = Fixture::new("<input id=entry value=''><p id=report></p><script>var preview=new KeyboardEvent('preview',{key:'x'});if(preview.key!=='x'||preview.isTrusted)throw 'real untrusted App keyboard';document.getElementById('entry').addEventListener('keydown',function(e){if(!(e instanceof KeyboardEvent)||Object.getPrototypeOf(e)!==KeyboardEvent.prototype||!e.isTrusted)throw 'real trusted App keyboard';document.getElementById('report').setAttribute('data-target',e.target.id);if(e.key==='x') e.preventDefault();}); document.getElementById('entry').addEventListener('keyup',function(e){document.getElementById('report').setAttribute('data-up',e.key);});</script>");
    let entry = node(&app, "entry");
    app.focus_control(entry);
    let (_, canceled) = app.key_event(input(true, "x", "KeyX", 0x58));
    assert!(canceled);
    if !canceled {
        app.text_input('x');
    }
    app.key_event(input(false, "x", "KeyX", 0x58));
    let (_, canceled) = app.key_event(input(true, "y", "KeyY", 0x59));
    assert!(!canceled);
    if !canceled {
        app.text_input('y');
    }
    assert_eq!(app.forms.controls[&entry].value.text(), "y");
    assert_eq!(attr(&app, "report", "data-target"), Some("entry"));
    assert_eq!(attr(&app, "report", "data-up"), Some("x"));
}

#[test]
fn browser_shortcuts_and_own_editors_never_deliver_keys_to_the_page() {
    let (_fixture, mut app) = Fixture::new("<body id=report><input id=entry><script>document.addEventListener('keydown',function(e){document.body.setAttribute('data-key',e.key);e.preventDefault();});document.addEventListener('keyup',function(e){document.body.setAttribute('data-up',e.key);});</script></body>");
    app.key(0x4c, true, false, false);
    assert!(app.focus == Focus::Address);
    app.key_event(input(true, "s", "KeyS", 0x53));
    app.key_event(input(false, "s", "KeyS", 0x53));
    assert_eq!(attr(&app, "report", "data-key"), None);
    assert_eq!(attr(&app, "report", "data-up"), None);
    app.focus = Focus::Page;
    app.open_inspector();
    app.key_event(input(true, "a", "KeyA", 0x41));
    assert_eq!(attr(&app, "report", "data-key"), None);
    app.close_inspector();
    app.show_settings();
    app.key_event(input(true, "b", "KeyB", 0x42));
    assert_eq!(attr(&app, "report", "data-key"), None);
}

#[test]
fn altgr_is_visible_to_page_keys_but_does_not_trigger_browser_shortcuts() {
    let (_fixture, mut app) = Fixture::new("<body id=report><script>document.addEventListener('keydown',function(e){document.body.setAttribute('data-key',e.key);document.body.setAttribute('data-ctrl',e.ctrlKey);document.body.setAttribute('data-alt',e.altKey);});</script></body>");
    let mut key = input(true, "ł", "KeyL", 0x4c);
    key.ctrl = true;
    key.alt = true;
    app.key_event(key);
    assert_eq!(attr(&app, "report", "data-key"), Some("ł"));
    assert_eq!(attr(&app, "report", "data-ctrl"), Some("true"));
    assert_eq!(attr(&app, "report", "data-alt"), Some("true"));
    assert!(app.focus == Focus::Page);
}

#[test]
fn altgr_text_stays_in_the_focused_browser_or_inspector_editor() {
    let (_fixture, mut app) = Fixture::new("<input id=entry value=unchanged><script>document.addEventListener('keydown',function(){document.title='page received key';});</script>");
    app.focus_control(node(&app, "entry"));
    app.open_inspector();
    assert!(app.inspector_tab_by_name("console"));
    let mut key = input(true, "ł", "KeyL", 0x4c);
    key.ctrl = true;
    key.alt = true;
    assert!(!app.key_event(key.clone()).1);
    app.text_input('ł');
    assert!(app.inspector.focused);
    assert_eq!(
        app.forms.controls[&node(&app, "entry")].value.text(),
        "unchanged"
    );
    assert_ne!(app.document.title, "page received key");
    app.close_inspector();
    app.focus_address();
    app.key_event(key);
    app.text_input('ł');
    assert!(app.focus == Focus::Address);
    assert_eq!(app.address.text(), "ł");
    assert_ne!(app.document.title, "page received key");
}

#[test]
fn physical_codes_distinguish_main_keys_numpad_and_right_modifiers() {
    assert_eq!(physical_code(0x41, 0x10, false), "KeyQ");
    assert_eq!(physical_code(0x0d, 0x1c, false), "Enter");
    assert_eq!(physical_code(0x0d, 0x1c, true), "NumpadEnter");
    assert_eq!(physical_code(0x26, 0x48, false), "Numpad8");
    assert_eq!(physical_code(0x26, 0x48, true), "ArrowUp");
    assert_eq!(physical_code(0x11, 0x1d, true), "ControlRight");
    assert_eq!(key_location("ControlRight"), 2);
    assert_eq!(key_location("ShiftLeft"), 1);
    assert_eq!(key_location("Numpad8"), 3);
    assert_eq!(key_location("ArrowUp"), 0);
    assert_eq!(physical_code(0xff, 0xff, true), "Unidentified");
}

#[test]
fn shipped_game_plays_through_with_frames_keys_pause_restart_and_browser_focus_isolation() {
    let mut app = App::new();
    app.open(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("pages/game-demo.html"));
    assert!(!app.failed, "{}", app.status);
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    app.activate_control(node(&app, "start-game"));
    let due = app
        .next_script_timer()
        .expect("starting the game schedules an animation frame");
    let wait = due.saturating_duration_since(std::time::Instant::now())
        + std::time::Duration::from_millis(1);
    assert!(wait < std::time::Duration::from_secs(1));
    std::thread::sleep(wait);
    assert!(app.poll_script_timer());
    assert!(app.document.attr(node(&app, "patrol"), "style").is_some());

    app.key(0x27, false, false, false); // ArrowRight, x24 ->40
    app.key(0x44, false, false, false); // D, x40 ->56
    app.key(0x57, false, false, false); // W, y112 ->96
    let position = app
        .document
        .attr(node(&app, "player"), "style")
        .unwrap()
        .to_owned();
    assert!(position.contains("left:56px"), "{position}");
    assert!(position.contains("top:96px"), "{position}");
    app.key(0x20, false, false, false); // Space pauses and does not activate the focused button.
    assert!(app.next_script_timer().is_none());
    app.key(0x27, false, false, false);
    assert_eq!(
        app.document.attr(node(&app, "player"), "style"),
        Some(position.as_str())
    );

    app.open_inspector();
    app.key(0x44, false, false, false);
    assert_eq!(
        app.document.attr(node(&app, "player"), "style"),
        Some(position.as_str())
    );
    app.close_inspector();
    app.key(0x4c, true, false, false);
    app.key(0x25, false, false, false);
    app.text_input('a');
    assert_eq!(
        app.document.attr(node(&app, "player"), "style"),
        Some(position.as_str())
    );
    app.key(0x1b, false, false, false);
    app.activate_control(node(&app, "pause-game"));
    assert!(app.next_script_timer().is_some());
    app.activate_control(node(&app, "start-game"));
    let restarted = app.document.attr(node(&app, "player"), "style").unwrap();
    assert!(
        restarted.contains("left:24px") && restarted.contains("top:112px"),
        "{restarted}"
    );

    // Complete all five parcels with the actual native-key App path. These
    // movements exercise event cancellation, live styles, collection and win.
    for (key, steps) in [
        (0x26, 4),
        (0x27, 17),
        (0x25, 15),
        (0x28, 8),
        (0x27, 15),
        (0x25, 15),
        (0x26, 4),
        (0x27, 8),
    ] {
        for _ in 0..steps {
            app.key(key, false, false, false);
        }
    }
    let rendered = app
        .page
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(rendered.contains("Parcels: 5 / 5"), "{rendered}");
    assert!(rendered.contains("All five delivered"));
    assert!(app.next_script_timer().is_none());
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    app.activate_control(node(&app, "start-game"));
    assert!(app.next_script_timer().is_some());
    app.activate_control(node(&app, "pause-game"));
    assert!(app.next_script_timer().is_none());
}

#[test]
fn focus_event_actual_app_tab_transition_keeps_native_editor_and_new_target_live() {
    let(_fixture,mut app)=Fixture::new(r#"<body><input id=a value=original><textarea id=b>second</textarea><p id=out>old</p><script>
var a=document.getElementById('a'),b=document.getElementById('b'),log='';
for(var type of ['blur','focusout','focus','focusin']){document.addEventListener(type,function(e){if(!(e instanceof FocusEvent)||!e.isTrusted||e.cancelable||e.view!==window)throw 'native focus';log+=e.type+':'+e.target.id+':'+(e.relatedTarget?e.relatedTarget.id:'null')+';';document.getElementById('out').textContent=log;},true);}
</script></body>"#);
    let a=node(&app,"a");let b=node(&app,"b");app.script_begin_job();app.focus_control(a);
    app.forms.controls.get_mut(&a).unwrap().value.move_to(3,false);
    app.key_event(input(true,"Tab","Tab",9));
    assert!(app.focus==Focus::Control(b));assert_eq!(app.forms.controls[&a].value.text(),"original");assert_eq!(app.forms.controls[&a].value.cursor,3);
    let page=app.scripting.as_mut().unwrap();assert_eq!(page.console_eval(&mut app.document,&mut app.forms,"if(log!=='focus:a:null;focusin:a:null;blur:a:b;focusout:a:b;focus:b:a;focusin:b:a;')throw log;").errors,0,"{}",page.report());
    assert!(app.page.runs.iter().any(|text|text.text.contains("focusout:a:b")));
}

#[test]
fn focus_event_actual_pointer_commits_caret_before_callback_and_handles_removed_control(){
    let(_fixture,mut app)=Fixture::new(r#"<body><input id=a value=original><input id=b value=second><script>
var a=document.getElementById('a'),b=document.getElementById('b'),log='';
a.onfocus=function(e){if(e.relatedTarget!==null||!e.isTrusted)throw 'pointer focus';log+='a;';};
a.onblur=function(e){if(e.relatedTarget!==b)throw 'pointer related';log+='blur;';};
b.onfocus=function(e){if(e.relatedTarget!==a)throw 'pointer related new';log+='b;';b.parentNode.removeChild(b);};
</script></body>"#);
    for id in ["a","b"] {
        let node=node(&app,id);let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();
        let x=rect.x+rect.width/2;let y=rect.y+rect.height/2+app.content_top()-app.scroll;
        app.pointer_down(x,y,false);app.pointer_up(x,y);
        if id=="a" {assert!(app.forms.controls[&node].value.cursor>0);assert_eq!(app.forms.controls[&node].value.text(),"original");}
    }
    let page=app.scripting.as_mut().unwrap();assert_eq!(page.console_eval(&mut app.document,&mut app.forms,"if(log!=='a;blur;b;'||b.isConnected)throw log;").errors,0,"{}",page.report());
    assert!(app.focus==Focus::Page);
}
