//! Original visible App control proof; all fixtures remain local and task-owned.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
static SERIAL: AtomicUsize = AtomicUsize::new(0);
struct File(PathBuf);
impl Drop for File { fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); } }
fn id(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len()).find(|&node| app.document.attr(node, "id") == Some(name)).unwrap()
}
fn click(app: &mut App, x: i32, y: i32) { app.pointer_down(x, y, false); app.pointer_up(x, y); }
#[test]
fn app_select_fragment_paints_real_native_popup_and_commits_actual_choice_and_events() {
    let file = File(std::env::temp_dir().join(format!("original-select-fragment-{}-{}.html", std::process::id(), SERIAL.fetch_add(1, Ordering::Relaxed))));
    std::fs::write(&file.0, r#"<!doctype html><title>Created select</title><style>body{margin:20px}</style><body><form id=f action='/result'><div id=host></div></form><p id=events>ready</p><script>
var host=document.getElementById('host');host.innerHTML='<select id=s name=choice><option value=a selected>Alpha<optgroup label=Disabled disabled><option value=x>Unavailable</optgroup><optgroup label=Choices><option value=b>Beta</optgroup></select>';
var s=document.getElementById('s');s.oninput=function(){var p=document.getElementById('events');p.textContent=p.textContent+'I';};s.onchange=function(){var p=document.getElementById('events');p.textContent=p.textContent+'C';};
</script></body>"#).unwrap();
    let mut app = App::new();
    app.resize(900, 640);
    app.open(&file.0);
    assert!(!app.failed, "{}", app.status);
    assert!(app.scripting.as_ref().unwrap().report().contains("0 error(s)."), "{}", app.scripting.as_ref().unwrap().report());
    let node = id(&app, "s");
    let rect = app.page.controls.iter().find(|rect| rect.node == node).expect("New select must own an actual painted control");
    let point = (rect.x + 8, rect.y + app.content_top() - app.scroll + rect.height / 2);
    click(&mut app, point.0, point.1);
    assert!(app.select_popup.is_some());
    let bounds = app.select_popup_box().unwrap();
    let canvas = app.draw();
    assert_eq!(canvas.pixels[bounds.y as usize * canvas.width + bounds.x as usize], 0x526d80);
    let row_y = |row: usize| bounds.y + 1 + (row - app.select_popup.unwrap().first) as i32 * bounds.row_height + bounds.row_height / 2;
    let disabled_y = row_y(2);
    click(&mut app, bounds.x + 10, disabled_y);
    assert!(app.select_popup.is_some());
    assert_eq!(app.forms.controls[&node].value.text(), "a");
    let popup = app.select_popup.unwrap();
    let chosen_y = bounds.y + 1 + (4 - popup.first) as i32 * bounds.row_height + bounds.row_height / 2;
    click(&mut app, bounds.x + 10, chosen_y);
    assert!(app.select_popup.is_none());
    assert_eq!(app.forms.controls[&node].value.text(), "b");
    let events = id(&app, "events");
    let text: String = app.document.nodes[events].children.iter().filter_map(|&node| match &app.document.nodes[node].kind { html::NodeKind::Text(text) => Some(text.as_str()), _ => None }).collect();
    assert_eq!(text, "readyIC");
    // This constructs an HTTP form result without contacting any host.
    let current = navigation::address("http://fixture.test/start", &Location::home()).unwrap();
    let submitted = app.forms.submit_request(&app.document, id(&app, "f"), None, &current, &current).unwrap();
    assert_eq!(submitted.location.url.unwrap().query.as_deref(), Some("choice=b"));
}

fn retirement_control_point(app: &App, node: usize) -> (i32, i32) {
    let rect = app.page.controls.iter().find(|rect| rect.node == node).expect("Connected select must have a real control box");
    (rect.x + 8, rect.y + app.content_top() - app.scroll + rect.height / 2)
}
fn retirement_queue(app: &mut App, source: &str) {
    let page = app.scripting.as_mut().unwrap();
    let outcome = page.console_eval(&mut app.document, &mut app.forms, source);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert!(!outcome.layout_changed, "Queuing the timer must not perform the mutation or retire the popup");
}
fn retirement_events(app: &App) -> String {
    app.document.nodes[id(app, "events")].children.iter().filter_map(|&node| {
        if let html::NodeKind::Text(text) = &app.document.nodes[node].kind { Some(text.as_str()) } else { None }
    }).collect()
}

#[test]
fn app_select_fragment_detach_retires_active_popup_focus_and_hits_then_reconnects_retained_native_state() {
    let file = File(std::env::temp_dir().join(format!("original-select-retirement-{}-{}.html", std::process::id(), SERIAL.fetch_add(1, Ordering::Relaxed))));
    std::fs::write(&file.0, r#"<!doctype html><title>Connected native select</title><style>body{background:#fff;margin:20px}#keep{position:absolute;left:20px;top:20px;width:220px}#host{position:absolute;left:20px;top:90px}</style><body><form id=f action='/result'><input id=keep name=keep value=seed><div id=host></div></form><p id=events></p><script>
var host=document.getElementById('host');host.innerHTML='<select id=s name=choice><option value=a selected>Alpha<option value=b>Beta<option value=c>Gamma</select>';
var s=document.getElementById('s'),savedSelect=s,savedOption=s.firstChild;
s.oninput=function(){var p=document.getElementById('events');p.textContent=p.textContent+'I';};s.onchange=function(){var p=document.getElementById('events');p.textContent=p.textContent+'C';};
</script></body>"#).unwrap();
    let mut app = App::new();
    app.resize(900, 640);
    app.open(&file.0);
    assert!(!app.failed, "{}", app.status);
    assert!(app.scripting.as_ref().unwrap().report().contains("0 error(s)."));
    let node = id(&app, "s");
    let keep = id(&app, "keep");
    let owner = id(&app, "f");
    let keep_point = retirement_control_point(&app, keep);
    click(&mut app, keep_point.0, keep_point.1);
    assert!(app.focus == Focus::Control(keep));
    app.key(0x41, true, false, false);
    for ch in "owner-edited".chars() { app.text_input(ch); }
    app.key(0x25, false, true, false);
    app.key(0x25, false, true, false);
    let editor = &app.forms.controls[&keep].value;
    let edited = (editor.text(), editor.cursor, editor.anchor, editor.offset);
    assert_eq!(edited.0, "owner-edited");
    assert_ne!(edited.1, edited.2, "Fixture must retain a real nonempty editor selection");

    let point = retirement_control_point(&app, node);
    click(&mut app, point.0, point.1);
    let bounds = app.select_popup_box().unwrap();
    let popup = app.select_popup.unwrap();
    let row_y = bounds.y + 1 + (1 - popup.first) as i32 * bounds.row_height + bounds.row_height / 2;
    click(&mut app, bounds.x + 10, row_y);
    assert_eq!(app.forms.controls[&node].value.text(), "b");
    assert_eq!(retirement_events(&app), "IC");
    click(&mut app, point.0, point.1);
    assert!(app.focus == Focus::Control(node));
    assert_eq!(app.select_popup.unwrap().node, node);
    let old_bounds = app.select_popup_box().unwrap();
    let old_row_point = (old_bounds.x + 10, old_bounds.y + 1 + old_bounds.row_height / 2);
    let canvas = app.draw();
    assert_eq!(canvas.pixels[old_bounds.y as usize * canvas.width + old_bounds.x as usize], 0x526d80);
    let native_keys: Vec<_> = app.forms.controls.keys().copied().collect();
    let option_nodes: Vec<_> = app.forms.controls[&node].select.as_ref().unwrap().options.iter().map(|option| option.node).collect();

    // A separate removal-button click would dismiss the popup before mutation.
    // Queue only; normal App timer dispatch must perform mutation and cleanup.
    retirement_queue(&mut app, "setTimeout(function(){host.parentNode.removeChild(host);if(s!==savedSelect||s.firstChild!==savedOption||s.isConnected||s.form!==null||document.getElementById('s')!==null||s.value!=='b')throw 'retired select state';document.title='Detached native select';},0);");
    assert!(app.select_popup.is_some());
    assert!(app.focus == Focus::Control(node));
    app.wait_for_script_timers(std::time::Duration::from_secs(1));
    assert_eq!(app.document.title, "Detached native select", "The actual App timer must consume the mutation");
    assert!(app.scripting.as_ref().unwrap().report().contains("0 error(s)."));
    assert!(app.select_popup.is_none());
    assert!(app.select_popup_box().is_none());
    assert!(app.focus == Focus::Page);
    assert!(!app.page.controls.iter().any(|rect| rect.node == node));
    assert_ne!(app.page.control_at(point.0, point.1 - app.content_top() + app.scroll), Some(node));
    assert!(app.select_row_at(old_row_point.0, old_row_point.1).is_none());
    let detached_canvas = app.draw();
    assert_ne!(detached_canvas.pixels[old_bounds.y as usize * detached_canvas.width + old_bounds.x as usize], 0x526d80, "Retired popup must not remain painted");
    click(&mut app, point.0, point.1);
    click(&mut app, old_row_point.0, old_row_point.1);
    assert!(app.select_popup.is_none());
    assert!(app.focus != Focus::Control(node));
    assert_eq!(app.forms.controls[&node].value.text(), "b", "Old control/popup points cannot change a detached native control");
    assert_eq!(app.forms.controls[&node].owner, None);
    assert_eq!(retirement_events(&app), "IC");
    // Construct form results only; there is no host request or account action.
    let current = navigation::address("http://fixture.test/start", &Location::home()).unwrap();
    let detached = app.forms.submit_request(&app.document, owner, None, &current, &current).unwrap();
    assert_eq!(detached.location.url.unwrap().query.as_deref(), Some("keep=owner-edited"));

    retirement_queue(&mut app, "setTimeout(function(){document.getElementById('f').appendChild(host);if(s!==savedSelect||s.firstChild!==savedOption||!s.isConnected||s.form!==document.getElementById('f')||document.getElementById('s')!==s||s.value!=='b')throw 'reconnected select state';document.title='Reconnected native select';},0);");
    app.wait_for_script_timers(std::time::Duration::from_secs(1));
    assert_eq!(app.document.title, "Reconnected native select");
    assert!(app.scripting.as_ref().unwrap().report().contains("0 error(s)."));
    assert_eq!(app.forms.controls.keys().copied().collect::<Vec<_>>(), native_keys);
    assert_eq!(app.forms.controls[&node].select.as_ref().unwrap().options.iter().map(|option| option.node).collect::<Vec<_>>(), option_nodes);
    assert_eq!(app.forms.controls[&node].value.text(), "b");
    assert_eq!(app.forms.controls[&node].owner, Some(owner));
    assert!(app.focus == Focus::Page, "Reconnect must not silently restore retired focus");
    assert!(app.select_popup.is_none());
    let reconnected = retirement_control_point(&app, node);
    assert_eq!(app.page.control_at(reconnected.0, reconnected.1 - app.content_top() + app.scroll), Some(node));
    click(&mut app, reconnected.0, reconnected.1);
    assert!(app.focus == Focus::Control(node));
    let bounds = app.select_popup_box().unwrap();
    let popup = app.select_popup.unwrap();
    let canvas = app.draw();
    assert_eq!(canvas.pixels[bounds.y as usize * canvas.width + bounds.x as usize], 0x526d80);
    let row_y = bounds.y + 1 + (2 - popup.first) as i32 * bounds.row_height + bounds.row_height / 2;
    click(&mut app, bounds.x + 10, row_y);
    assert!(app.select_popup.is_none());
    assert_eq!(app.forms.controls[&node].value.text(), "c");
    assert_eq!(retirement_events(&app), "ICIC");
    let submitted = app.forms.submit_request(&app.document, owner, None, &current, &current).unwrap();
    assert_eq!(submitted.location.url.unwrap().query.as_deref(), Some("keep=owner-edited&choice=c"));
    let editor = &app.forms.controls[&keep].value;
    assert_eq!((editor.text(), editor.cursor, editor.anchor, editor.offset), edited);
}
