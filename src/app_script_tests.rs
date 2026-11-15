//! Integration checks for actual user input, document lifetimes and the shipped
//! JavaScript demonstration. Included as a child module of app.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[test]
fn animation_wrapper_lifetimes_follow_navigation_script_setting_and_wake_failure() {
    let fixture=ScriptFixture::new();
    let markup=r#"<title>Original frame lifetime</title><p id=out>waiting</p><script>
var nativeFrame=requestAnimationFrame;requestAnimationFrame=function(callback){return nativeFrame.call(null,callback);};
function again(){document.getElementById('out').textContent='frame';requestAnimationFrame(again);}requestAnimationFrame(again);
</script>"#;
    let destination=fixture.write("destination.html","<title>Destination</title><p id=out>destination unchanged</p>");
    let mut app=fixture.open(markup);assert!(app.next_script_timer().is_some());
    app.script_timer_unavailable();assert!(app.next_script_timer().is_none());
    app.wait_for_script_timers(Duration::from_millis(30));assert_eq!(text(&app,"out"),"waiting");
    let mut app=fixture.open(markup);app.set_javascript_enabled(false);assert!(app.next_script_timer().is_none());assert!(app.scripting.is_none());
    app.wait_for_script_timers(Duration::from_millis(30));assert_eq!(text(&app,"out"),"waiting");
    app.set_javascript_enabled(true);assert!(app.next_script_timer().is_some());
    app.open(&destination);assert!(app.next_script_timer().is_none());app.wait_for_script_timers(Duration::from_millis(40));
    assert_eq!(text(&app,"out"),"destination unchanged");assert_eq!(app.document.title,"Destination");
}

struct ScriptFixture(PathBuf);

impl ScriptFixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-app-script-review-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, value: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, value).unwrap();
        path
    }

    fn demo(&self) -> App {
        self.write("styles.css", include_str!("../pages/styles.css"));
        self.write(
            "scripts/javascript-demo.js",
            include_str!("../pages/scripts/javascript-demo.js"),
        );
        self.open(include_str!("../pages/javascript-demo.html"))
    }

    fn open(&self, markup: &str) -> App {
        let path = self.write("page.html", markup);
        let mut app = App::new();
        app.open(&path);
        assert!(!app.failed, "{}", app.status);
        assert!(!app.is_loading());
        app
    }
}

impl Drop for ScriptFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn node(app: &App, id: &str) -> usize {
    app.document
        .nodes
        .iter()
        .enumerate()
        .find_map(|(i, _)| (app.document.attr(i, "id") == Some(id)).then_some(i))
        .unwrap_or_else(|| panic!("Missing test element {id}"))
}

fn text(app: &App, id: &str) -> String {
    let mut stack = vec![node(app, id)];
    let mut value = String::new();
    while let Some(index) = stack.pop() {
        if let html::NodeKind::Text(part) = &app.document.nodes[index].kind {
            value.push_str(part);
        }
        stack.extend(app.document.nodes[index].children.iter().rev().copied());
    }
    value
}

fn rendered(app: &App) -> String {
    app.page
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

fn click(app: &mut App, id: &str) {
    let node = node(app, id);
    app.activate_control(node);
}

fn enter_value(app: &mut App, id: &str, value: &str) {
    let node = node(app, id);
    app.focus_control(node);
    assert!(app.focus == Focus::Control(node));
    app.key(0x41, true, false, false);
    app.paste(value);
}

fn script_report(app: &App) -> String {
    app.scripting
        .as_ref()
        .expect("document has a realm")
        .report()
}

#[test]
fn location_tracks_local_files_fragments_history_and_failed_navigation() {
    let fixture = ScriptFixture::new();
    let markup = r#"<html><body>
<button id=read type=button>Read location</button><p id=part>Anchor</p>
<script>
var saved=location, stringify=location.toString;
function read(){
  if(saved!==window.location || saved!==document.location || stringify!==location.toString)throw Error('location identity');
  if(document.URL!==saved.href || document.documentURI!==saved.href)throw Error('document URL');
  document.title=stringify.call(saved);
}
read();document.getElementById('read').onclick=read;
</script></body></html>"#;
    let source = fixture.write("page #one.html", markup);
    let mut app = App::new();
    app.open(&source);
    let original = app.location.display();
    assert_eq!(app.document.title, original, "{}", script_report(&app));
    assert!(original.contains("page%20%23one.html"));
    app.open_address(&format!("{original}#part"));
    click(&mut app, "read");
    assert_eq!(app.document.title, format!("{original}#part"));
    app.history_move(-1);
    click(&mut app, "read");
    assert_eq!(app.document.title, original);
    app.history_move(1);
    click(&mut app, "read");
    let fragment = app.location.display();
    assert_eq!(app.document.title, fragment);
    app.follow("missing.html");
    assert!(app.failed);
    click(&mut app, "read");
    assert_eq!(app.document.title, fragment);
    let other = fixture.write("other.html", markup);
    app.open(&other);
    assert_eq!(
        app.document.title,
        app.location.display(),
        "{}",
        script_report(&app)
    );
    assert_ne!(app.document.title, original);
    app.history_move(-1);
    assert_eq!(app.document.title, fragment, "{}", script_report(&app));
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        script_report(&app)
    );
}

#[test]
fn timer_callbacks_update_the_real_page_and_controls_can_cancel_them() {
    let fixture = ScriptFixture::new();
    let mut app = fixture.open(r#"<html><body><p id="out">Waiting</p><button id="cancel" type="button">Cancel</button><button id="run" type="button">Run</button><script>
var pending=setTimeout(()=>{document.getElementById('out').textContent='Unexpected';},0);
document.getElementById('cancel').onclick=()=>{clearTimeout(pending);};
document.getElementById('run').onclick=()=>{setTimeout(()=>{document.getElementById('out').textContent='Timer ran';document.title='Timer complete';},0);};
</script></body></html>"#);
    assert_eq!(text(&app, "out"), "Waiting");
    click(&mut app, "cancel");
    assert!(!app.poll_script_timer());
    click(&mut app, "run");
    assert_eq!(text(&app, "out"), "Waiting");
    assert!(app.poll_script_timer());
    assert_eq!(app.document.title, "Timer complete");
    assert!(rendered(&app).contains("Timer ran"));
    assert!(!app.poll_script_timer());
}

#[test]
fn timers_survive_fragments_but_not_document_replacement_or_script_disable() {
    let fixture = ScriptFixture::new();
    let markup = r#"<html><body><p id="anchor">Here</p><script>setTimeout(()=>{document.title='Timer ran';},0);</script></body></html>"#;
    let mut app = fixture.open(markup);
    app.follow("#anchor");
    assert!(app.poll_script_timer());
    assert_eq!(app.document.title, "Timer ran");
    let source = fixture.write("timers.html", markup);
    app.open(&source);
    assert!(app.next_script_timer().is_some());
    let replacement = fixture.write(
        "replacement.html",
        "<title>Replacement</title><p>New page</p>",
    );
    app.open(&replacement);
    assert!(!app.poll_script_timer());
    assert_eq!(app.document.title, "Replacement");
    app.open(&source);
    app.set_javascript_enabled(false);
    assert!(app.next_script_timer().is_none());
    assert!(!app.poll_script_timer());
}

#[test]
fn pending_promises_survive_fragments_and_failed_navigation_but_not_replacement() {
    let fixture = ScriptFixture::new();
    let markup = r#"<title>Waiting</title><p id=anchor>Anchor</p><script>
var complete;new Promise(function(resolve){complete=resolve;}).then(function(value){document.title=value;});
setTimeout(function(){complete('Promise completed');},0);
</script>"#;
    let mut app = fixture.open(markup);
    app.follow("#anchor");
    assert!(app.poll_script_timer());
    assert_eq!(
        app.document.title,
        "Promise completed",
        "{}",
        script_report(&app)
    );
    let original = fixture.write("promises.html", markup);
    app.open(&original);
    app.follow("missing-promise-page.html");
    assert!(app.failed);
    assert!(app.poll_script_timer());
    assert_eq!(
        app.document.title,
        "Promise completed",
        "{}",
        script_report(&app)
    );
    app.open(&original);
    let replacement = fixture.write("replacement-promise.html", "<title>Replacement</title>");
    app.open(&replacement);
    assert!(!app.poll_script_timer());
    assert_eq!(app.document.title, "Replacement");
    app.open(&original);
    app.set_javascript_enabled(false);
    assert!(!app.poll_script_timer());
    assert_eq!(app.document.title, "Waiting");
}

#[test]
fn browser_namespaces_queries_arrows_and_exceptions_run_through_real_controls() {
    let fixture = ScriptFixture::new();
    let mut app = fixture.open(r#"<html><body><div id="scope" class="panel"><input id="toggle" type="checkbox"><p id="first" data-state="ready">First</p><p id="second" class="target">Second</p></div><p id="outside" class="target">Outside</p><p id="out">Waiting</p><button id="go" type="button">Run</button><script>
window.pageState = window.pageState || {count:0};
var sharedState = pageState;
document.querySelector('#go').onclick = () => {
    try {
        sharedState.count++;
        if (!document.querySelector('input:checked')) { throw Error('Check first'); }
        let panel = document.getElementById('scope');
        let selected = panel.querySelector('.target, [data-state="ready"]');
        if (panel.querySelector('#scope') !== null || panel.querySelector('#outside') !== null) { throw Error('Wrong scope'); }
        document.getElementById('out').textContent = selected.id + ':' + document.documentElement.tagName + ':' + selected.nodeType + ':' + pageState.count;
    } catch (error) {
        document.getElementById('out').textContent = error.name + ': ' + error.message;
    } finally {
        document.title = 'Attempts ' + window.pageState.count;
    }
};
</script></body></html>"#);
    assert!(
        script_report(&app).contains("0 error(s)"),
        "{}",
        script_report(&app)
    );
    click(&mut app, "go");
    assert_eq!(text(&app, "out"), "Error: Check first");
    assert_eq!(app.document.title, "Attempts 1");
    click(&mut app, "toggle");
    click(&mut app, "go");
    assert_eq!(text(&app, "out"), "first:HTML:1:2");
    assert_eq!(app.document.title, "Attempts 2");
    assert!(
        script_report(&app).contains("0 error(s)"),
        "{}",
        script_report(&app)
    );
}

#[test]
fn csp_allows_nonce_script_callbacks_but_not_unapproved_html_handlers() {
    let fixture = ScriptFixture::new();
    let mut app = fixture.open(r#"<html><head><meta http-equiv="Content-Security-Policy" content="script-src 'nonce-own-demo'"></head><body><noscript><p>Scripts are disabled by the user</p></noscript><p id="out">Waiting</p><button id="blocked" type="button" onclick="document.title='forbidden'">Blocked handler</button><button id="allowed" type="button">Allowed listener</button><script nonce="own-demo">document.title='Allowed startup';document.querySelector('#allowed').addEventListener('click',()=>{document.getElementById('out').textContent='Allowed callback';});</script><script>document.title='forbidden script';</script></body></html>"#);
    assert!(app.document.scripting_enabled);
    assert_eq!(app.document.title, "Allowed startup");
    assert!(!rendered(&app).contains("Scripts are disabled by the user"));
    click(&mut app, "blocked");
    assert_eq!(app.document.title, "Allowed startup");
    click(&mut app, "allowed");
    assert_eq!(text(&app, "out"), "Allowed callback");
    assert!(script_report(&app).contains("Content Security Policy"));
    app.key(0x4a, true, false, false);
    assert!(!app.document.scripting_enabled);
    assert!(rendered(&app).contains("Scripts are disabled by the user"));
}

#[test]
fn shipped_object_workshop_keeps_aliases_and_explicit_receivers() {
    let fixture = ScriptFixture::new();
    let mut app = fixture.demo();
    assert_eq!(text(&app, "pieces"), "Browser pieces (2): HTML | CSS");
    click(&mut app, "add-piece");
    assert_eq!(
        text(&app, "pieces"),
        "Browser pieces (3): HTML | CSS | JavaScript"
    );
    assert!(text(&app, "piece-keys").contains("Shared array length: 3."));
    click(&mut app, "undo-piece");
    enter_value(&mut app, "piece", "TLS");
    click(&mut app, "add-piece");
    assert_eq!(text(&app, "pieces"), "Browser pieces (3): HTML | CSS | TLS");
    click(&mut app, "reset-pieces");
    assert_eq!(text(&app, "pieces"), "Browser pieces (2): HTML | CSS");
    assert!(text(&app, "piece-keys").contains("Shared array length: 2."));
    assert!(script_report(&app).contains("0 error(s)"));
}

#[test]
fn checked_css_tracks_native_click_reset_script_and_cancellation() {
    let fixture = ScriptFixture::new();
    let mut app = fixture.open("<style>#choice:checked ~ #panel {display:block} #panel {display:none} #one:checked + label {color:red} #two:checked + label {color:blue}</style><form id=f><input id=choice type=checkbox><div id=panel><a href=page.html>Visible details</a></div><input id=one type=radio name=g checked><label for=one>One</label><input id=two type=radio name=g><label for=two>Two</label><button id=reset type=reset>Reset</button></form><button id=program type=button>Program</button><script>document.getElementById('program').onclick=function(){document.getElementById('choice').checked=true;};document.getElementById('two').onclick=function(event){event.preventDefault();};</script>");
    assert!(!rendered(&app).contains("Visible details"));
    click(&mut app, "choice");
    assert!(rendered(&app).contains("Visible details"));
    click(&mut app, "reset");
    assert!(!rendered(&app).contains("Visible details"));
    click(&mut app, "program");
    assert!(rendered(&app).contains("Visible details"));
    click(&mut app, "two");
    assert!(app.forms.controls[&node(&app, "one")].checked);
    assert!(!app.forms.controls[&node(&app, "two")].checked);
    assert!(app
        .page
        .runs
        .iter()
        .any(|run| run.text == "One" && run.color == 0xff0000));
    assert!(!app
        .page
        .runs
        .iter()
        .any(|run| run.text == "Two" && run.color == 0x0000ff));
    assert!(script_report(&app).contains("0 error(s)"));
}

#[test]
fn shipped_script_demo_handles_counter_typing_calculation_and_styles() {
    let fixture = ScriptFixture::new();
    let mut app = fixture.demo();
    assert_eq!(
        text(&app, "ready"),
        "Ready: inline script, external script, then DOMContentLoaded. Starting value: 10."
    );
    assert!(script_report(&app).contains("0 error(s)"));
    let expected = "Sum: 6; rounds: 3; bit mask: 3; match: Cat; eval: 16.";
    assert_eq!(text(&app, "compat-result"), expected);
    click(&mut app, "compat-run");
    assert_eq!(text(&app, "compat-result"), expected);
    assert!(text(&app, "navigator-result").contains(crate::http::USER_AGENT));
    assert_eq!(text(&app, "json-result"), r#"{"mode":"sum","result":10}"#);
    enter_value(
        &mut app,
        "json-source",
        r#"{"mode":"count","items":[2,3,5]}"#,
    );
    click(&mut app, "json-run");
    assert_eq!(text(&app, "json-result"), r#"{"mode":"count","result":3}"#);
    enter_value(&mut app, "json-source", "{broken}");
    click(&mut app, "json-run");
    assert!(text(&app, "json-result").starts_with("Could not read the JSON: "));
    enter_value(&mut app, "json-source", r#"{"mode":"sum","items":[4,8]}"#);
    click(&mut app, "json-run");
    assert_eq!(text(&app, "json-result"), r#"{"mode":"sum","result":12}"#);
    enter_value(&mut app, "json-source", r#"{"mode":"other","items":[]}"#);
    click(&mut app, "json-run");
    assert_eq!(text(&app, "json-result"), "Choose mode sum or count.");
    click(&mut app, "more");
    assert_eq!(text(&app, "count"), "11");
    click(&mut app, "less");
    assert_eq!(text(&app, "count"), "10");
    click(&mut app, "less");
    click(&mut app, "zero");
    assert_eq!(text(&app, "count"), "10");

    enter_value(&mut app, "name", "Avery");
    assert_eq!(text(&app, "greeting"), "Hello, Avery.");
    app.key(0x08, false, false, false);
    assert_eq!(text(&app, "greeting"), "Hello, Aver.");
    app.key(0x41, true, false, false);
    let PlatformAction::Cut(copied) = app.key(0x58, true, false, false) else {
        panic!("Ctrl+X should ask the platform to copy the selected text")
    };
    assert_eq!(copied, "Aver");
    app.cut_copied(&copied);
    assert_eq!(text(&app, "greeting"), "Hello, visitor.");

    enter_value(&mut app, "limit", "1000");
    click(&mut app, "calculate");
    assert_eq!(text(&app, "sum"), "1 through 1000 adds up to 500500.");
    enter_value(&mut app, "limit", "1.5");
    click(&mut app, "calculate");
    assert_eq!(text(&app, "sum"), "Enter a whole number from 1 to 1000.");

    click(&mut app, "color");
    let preview = node(&app, "preview");
    assert_eq!(app.document.attr(preview, "class"), Some("preview mint"));
    assert!(app
        .document
        .attr(preview, "style")
        .unwrap()
        .contains("#184d41"));
    click(&mut app, "visibility");
    assert!(!rendered(&app).contains("The page updates this element's class"));
    click(&mut app, "visibility");
    assert!(rendered(&app).contains("The page updates this element's class"));
    assert!(script_report(&app).contains("0 error(s)"));
    assert!(!app.failed);
}

#[test]
fn script_cancellation_controls_checkbox_submission_and_native_reset() {
    let fixture = ScriptFixture::new();
    let mut app = fixture.demo();
    let checkbox = node(&app, "locked");
    assert!(!app.forms.controls[&checkbox].checked);
    click(&mut app, "locked");
    assert!(!app.forms.controls[&checkbox].checked);
    assert!(text(&app, "check-result").contains("checked = true"));

    let location = app.location.clone();
    enter_value(&mut app, "message", "Handled locally");
    app.key(0x0d, false, false, false);
    assert_eq!(
        text(&app, "form-result"),
        "Kept on this page: Handled locally"
    );
    assert_eq!(app.location, location);
    assert!(!app.failed && !app.is_loading());
    let owner = node(&app, "local-form");
    let reset = app
        .forms
        .controls
        .iter()
        .find_map(|(&id, c)| (c.kind == Kind::Reset && c.owner == Some(owner)).then_some(id))
        .unwrap();
    app.activate_control(reset);
    assert_eq!(
        app.forms.controls[&node(&app, "message")].value.text(),
        "Made from scratch"
    );
    assert_eq!(
        text(&app, "form-result"),
        "Kept on this page: Handled locally"
    );
    assert!(!app.failed);
}

#[test]
fn canceled_choice_activation_preserves_unrelated_script_changes() {
    let fixture = ScriptFixture::new();
    let mut app = fixture.open(
        "<input id=cancel type=checkbox onclick=\"\
         document.getElementById('other').checked=true;event.preventDefault();\">\
         <input id=other type=checkbox>\
         <input id=first type=radio name=choice checked>\
         <input id=second type=radio name=choice onclick=\"\
         document.getElementById('other').checked=false;event.preventDefault();\">",
    );
    click(&mut app, "cancel");
    assert!(!app.forms.controls[&node(&app, "cancel")].checked);
    assert!(app.forms.controls[&node(&app, "other")].checked);
    click(&mut app, "second");
    assert!(app.forms.controls[&node(&app, "first")].checked);
    assert!(!app.forms.controls[&node(&app, "second")].checked);
    assert!(!app.forms.controls[&node(&app, "other")].checked);
    assert!(!app.failed);
}

#[test]
fn infinite_handler_stops_reports_and_leaves_other_controls_usable() {
    let fixture = ScriptFixture::new();
    let mut app = fixture.demo();
    click(&mut app, "loop");
    assert!(!app.failed && !app.is_loading());
    assert!(app.status.contains("JavaScript subset"), "{}", app.status);
    assert!(script_report(&app).contains("limit"));
    assert!(text(&app, "loop-result").starts_with("Loop entered."));
    app.key(0x49, true, false, false);
    assert!(app.view == View::Resources);
    assert!(rendered(&app).contains("limit"));
    app.key(0x49, true, false, false);
    assert!(app.view == View::Page);
    click(&mut app, "more");
    assert_eq!(text(&app, "count"), "11");
    assert!(!app.failed);
}

#[test]
fn script_realm_survives_view_changes_but_reload_and_toggle_replace_it() {
    let fixture = ScriptFixture::new();
    let mut app = fixture.demo();
    click(&mut app, "more");
    app.resize(820, 620);
    app.set_zoom(1.25);
    app.follow("#preview");
    assert_eq!(text(&app, "count"), "11");
    assert_eq!(app.location.fragment, "preview");
    click(&mut app, "more");
    assert_eq!(text(&app, "count"), "12");

    app.reload();
    assert_eq!(text(&app, "count"), "10");
    click(&mut app, "more");
    app.key(0x74, true, false, false);
    assert_eq!(text(&app, "count"), "10");
    click(&mut app, "more");
    app.key(0x4a, true, false, false);
    assert!(!app.scripts_enabled && !app.document.scripting_enabled);
    assert!(app.scripting.is_none());
    assert_eq!(text(&app, "ready"), "Waiting for the page scripts.");
    assert!(rendered(&app).contains("JavaScript is disabled"));
    click(&mut app, "more");
    assert_eq!(text(&app, "count"), "10");

    app.key(0x4a, true, false, false);
    assert!(app.scripts_enabled && app.document.scripting_enabled);
    assert!(text(&app, "ready").starts_with("Ready:"));
    assert!(!rendered(&app).contains("JavaScript is disabled"));
    click(&mut app, "more");
    assert_eq!(text(&app, "count"), "11");
    assert!(!app.failed);
}

#[test]
fn input_events_require_real_user_edits_and_ignore_script_value_assignment() {
    let fixture = ScriptFixture::new();
    let mut app = fixture.open(
        "<input id=field><p id=count>0</p><button id=write type=button>Script value</button>\
         <script>var edits=0;document.getElementById('field').oninput=function(){\
         edits++;document.getElementById('count').textContent=String(edits);};\
         document.getElementById('write').onclick=function(){\
         document.getElementById('field').value='script';};</script>",
    );
    enter_value(&mut app, "field", "A");
    assert_eq!(text(&app, "count"), "1");
    enter_value(&mut app, "field", "A");
    assert_eq!(text(&app, "count"), "1");
    app.key(0x08, false, false, false);
    assert_eq!(text(&app, "count"), "2");
    app.key(0x08, false, false, false);
    assert_eq!(text(&app, "count"), "2");
    app.paste("AB");
    assert_eq!(text(&app, "count"), "3");
    app.key(0x41, true, false, false);
    app.cut_copied("AB");
    assert_eq!(text(&app, "count"), "4");
    click(&mut app, "write");
    assert_eq!(
        app.forms.controls[&node(&app, "field")].value.text(),
        "script"
    );
    assert_eq!(text(&app, "count"), "4");
    assert!(!app.failed);
}

#[test]
fn enter_activates_the_default_submit_buttons_cancelable_click_first() {
    let fixture = ScriptFixture::new();
    let mut app = fixture.open(
        "<p id=out>idle</p><form action='/form-result' onsubmit=\"\
         document.getElementById('out').textContent='submit should not run';event.preventDefault();\">\
         <input id=field value=test><button id=send type=submit onclick=\"\
         document.getElementById('out').textContent='click canceled';event.preventDefault();\">Send</button></form>",
    );
    let location = app.location.clone();
    app.focus_control(node(&app, "field"));
    app.key(0x0d, false, false, false);
    assert_eq!(text(&app, "out"), "click canceled");
    assert_eq!(app.location, location);
    assert!(!app.failed && !app.is_loading());
}

#[test]
fn unsupported_startup_script_preserves_the_page_and_valid_handlers() {
    let fixture = ScriptFixture::new();
    let mut app = fixture.open(
        "<title>Kept page</title><p id=out>Still here</p>\
         <button id=good type=button onclick=\"document.getElementById('out').textContent='working';\">Try</button>\
         <script>document.title='must not execute';class Unsupported {}</script>",
    );
    assert_eq!(app.document.title, "Kept page");
    assert_eq!(text(&app, "out"), "Still here");
    assert!(rendered(&app).contains("Still here"));
    assert!(app.status.contains("script error"), "{}", app.status);
    assert!(script_report(&app).contains("1 error(s)"));
    assert!(!app.failed);
    click(&mut app, "good");
    assert_eq!(text(&app, "out"), "working");
    assert!(!app.failed);
}

#[test]
fn wrapped_timer_capabilities_survive_fragments_not_document_replacement_or_disable() {
    let fixture = ScriptFixture::new();
    let markup = r#"<title>Waiting</title><p id=anchor>Anchor</p><script>
var nativeTimer=setTimeout;
window.setTimeout=function(){return nativeTimer.apply(window,arguments);};
setTimeout(function(){document.title='wrapped callback';},0);
</script>"#;
    let mut app = fixture.open(markup);
    app.follow("#anchor");
    assert!(app.poll_script_timer());
    assert_eq!(app.document.title, "wrapped callback");
    let source = fixture.write("wrapped-timers.html", markup);
    app.open(&source);
    let replacement = fixture.write("wrapped-replacement.html", r#"<title>Replacement</title><script>
if(typeof nativeTimer!=='undefined')throw Error('previous realm leaked');
if(Object.getOwnPropertyDescriptor(window,'setTimeout').value!==setTimeout)throw Error('timer slot');
setTimeout(function(){document.title='replacement callback';},0);
</script>"#);
    app.open(&replacement);
    assert!(app.poll_script_timer());
    assert_eq!(app.document.title, "replacement callback");
    assert!(!app.poll_script_timer());
    app.open(&source);
    app.set_javascript_enabled(false);
    assert!(app.next_script_timer().is_none());
    assert!(!app.poll_script_timer());
}
