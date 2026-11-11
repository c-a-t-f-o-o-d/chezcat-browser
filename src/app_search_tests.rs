//! Original loopback search workflow fixtures. These exercise the real App,
//! form events, HTTP navigation, rendered links and history. They are not
//! captured Google responses and do not establish live Google compatibility.
use super::*;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct Site {
    address: SocketAddr,
    stopped: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<String>>>,
    posts: Arc<Mutex<Vec<(String, String)>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Site {
    fn new(result_origin: Option<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let ending = stopped.clone();
        let seen = requests.clone();
        let posts = Arc::new(Mutex::new(Vec::new()));
        let post_requests = posts.clone();
        let worker = std::thread::spawn(move || {
            for incoming in listener.incoming() {
                let Ok(mut stream) = incoming else { break };
                if ending.load(Ordering::Relaxed) {
                    break;
                }
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut byte = [0u8];
                while bytes.len() < 16384 && !bytes.ends_with(b"\r\n\r\n") {
                    if stream.read_exact(&mut byte).is_err() {
                        break;
                    }
                    bytes.push(byte[0]);
                }
                if !bytes.ends_with(b"\r\n\r\n") {
                    continue;
                }
                let header = String::from_utf8_lossy(&bytes);
                let mut request = header.lines().next().unwrap().split_whitespace();
                let method = request.next().unwrap();
                assert!(matches!(method, "GET" | "POST"));
                let path = request.next().unwrap().to_owned();
                seen.lock().unwrap().push(path.clone());
                if method == "POST" {
                    let length = header
                        .lines()
                        .filter_map(|line| line.split_once(':'))
                        .find_map(|(key, value)| {
                            key.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    assert!(length <= crate::http::MAX_FORM_BYTES);
                    let mut body = vec![0; length];
                    stream.read_exact(&mut body).unwrap();
                    post_requests
                        .lock()
                        .unwrap()
                        .push((path.clone(), String::from_utf8(body).unwrap()));
                }
                let (status, extra, body) = if path == "/post-result?keep=1" {
                    (
                        200,
                        String::new(),
                        "<title>Script POST result</title><h1>POST received</h1>".to_owned(),
                    )
                } else if path == "/result" {
                    (200, String::new(), "<title>External fixture result</title><h1>Visited the actual result page</h1>".to_owned())
                } else if path == "/visit?entry=original" || path.starts_with("/form-redirect?") {
                    (
                        302,
                        format!("Location: {}/result\r\n", result_origin.as_ref().unwrap()),
                        String::new(),
                    )
                } else if path.starts_with("/home-") || path.starts_with("/search-") {
                    let area = path.starts_with("/home-area") || path.starts_with("/search-area");
                    let results = path.starts_with("/search-");
                    let query = path
                        .split_once('?')
                        .map_or("", |(_, query)| query)
                        .split('&')
                        .find_map(|entry| entry.strip_prefix("q="))
                        .unwrap_or("");
                    let value = decode(query);
                    let scripted = path.contains("-script") || path.contains("-post");
                    let post = path.contains("-post");
                    let long_dispatch = path.contains("-long");
                    let deferred_body = path.contains("-deep");
                    let symbol_bootstrap = path.contains("-symbol");
                    let accessor_bootstrap = path.contains("-accessor");
                    let callable_bootstrap = path.contains("-callable");
                    let kind = if callable_bootstrap {
                        "area-script-callable"
                    } else if accessor_bootstrap {
                        "area-script-accessor"
                    } else if symbol_bootstrap {
                        "area-script-symbol"
                    } else if deferred_body {
                        "area-script-deep"
                    } else if long_dispatch {
                        "area-script-long"
                    } else if scripted {
                        "area-script"
                    } else if area {
                        "area"
                    } else {
                        "input"
                    };
                    let required = if path.contains("-required") {
                        "required"
                    } else {
                        ""
                    };
                    let field = if area {
                        format!(
                            "<textarea id=q name=q {required} rows=2 cols=32>{}</textarea>",
                            escape(&value)
                        )
                    } else {
                        format!(
                            "<input id=q name=q type=search value='{}' size=32>",
                            escape(&value)
                        )
                    };
                    let results_html = if results {
                        format!("<main id=results><h1>Results for {}</h1><h2><a href='/visit?entry=original'>Original fixture result</a></h2><p>Independent text shown beneath the clickable result.</p></main>", escape(&value))
                    } else {
                        "<p>Search the original loopback fixture.</p>".to_owned()
                    };
                    let title = if results {
                        "Fixture results"
                    } else {
                        "Fixture home"
                    };
                    let method = if post { "post" } else { "get" };
                    let action = if path.contains("-cross") {
                        format!("{}/result", result_origin.as_ref().unwrap())
                    } else if path.contains("-redirect") {
                        "/form-redirect".to_owned()
                    } else if post {
                        "/post-result?keep=1".to_owned()
                    } else {
                        format!("/search-{kind}?discard=old")
                    };
                    let submit_script = if scripted {
                        "document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};document.f.q.onkeydown=function(event){if(event.key==='Enter'){event.preventDefault();this.form.submit();document.f.q.value='changed after submit';}};"
                    } else {
                        ""
                    };
                    let bootstrap = if callable_bootstrap {
                        // Original standard-helper bootstrap. The resulting
                        // bound handler must survive actual event dispatch.
                        r#"var hiddenReads=0,settings={first:'search',second:'ready'};
function hiddenSetting(){hiddenReads++;return 'hidden';}
Object.defineProperty(settings,'hidden',{get:hiddenSetting,configurable:true});
var descriptors=Object.getOwnPropertyDescriptors(settings);
if(hiddenReads!==0||descriptors.hidden.get!==hiddenSetting||descriptors.first.value!=='search')throw Error('descriptor bootstrap');
var entries=Object.entries(settings),values=Object.values(settings);
if(entries.length!==2||entries[0][0]!=='first'||entries[0][1]!=='search'||entries[1][0]!=='second'||values.join('-')!=='search-ready'||hiddenReads!==0)throw Error('enumeration bootstrap');
var replacementCalls=0;
var label=values.join('-').replace(/(^|-)([a-z])/g,function(match,separator,letter,offset,original){
'use strict';
if(this!==undefined||arguments.length!==5||original!=='search-ready'||offset!==(replacementCalls===0?0:6))throw Error('replacement bootstrap');
replacementCalls++;return separator+letter.toUpperCase();});
if(label!=='Search-Ready'||replacementCalls!==2)throw Error('replacement result');
document.f.onsubmit=function(){throw Error('bound script submit dispatched a submit event');};
function readyEnter(marker,event){
if(marker!=='Search-Ready'||this!==document.f.q)throw Error('bound event receiver');
if(event.key==='Enter'){event.preventDefault();this.form.submit();document.f.q.value='changed after submit';}}
document.f.q.onkeydown=readyEnter.bind(document.f.q,label);
document.f.setAttribute('data-callable','ready');"#
                            .to_owned()
                    } else if accessor_bootstrap {
                        // Original passive-support probe. The inert event is
                        // registered and removed; only its option is queried.
                        format!(
                            r#"var passiveQueried=false,passiveReads=0;
var probeOptions=Object.defineProperty({{}},'passive',{{get:function(){{
if(this!==probeOptions)throw Error('option getter receiver');
passiveQueried=true;passiveReads++;return true;}}}});
function emptyFunction(){{}}
window.addEventListener('test',emptyFunction,probeOptions);
window.removeEventListener('test',emptyFunction,probeOptions);
if(!passiveQueried||passiveReads!==1)throw Error('passive option query');
Object.defineProperty(navigator,'fixtureAccessor',{{configurable:true,get:function(){{
if(this!==navigator)throw Error('native expando getter receiver');return passiveQueried;}}}});
if(!navigator.fixtureAccessor)throw Error('native expando getter');
var windowAccessorValue=0;
Object.defineProperty(window,'fixtureWindowAccessor',{{configurable:true,
get:function(){{if(this!==window)throw Error('window getter receiver');return windowAccessorValue;}},
set:function(value){{if(this!==window)throw Error('window setter receiver');windowAccessorValue=value;}}}});
fixtureWindowAccessor=1;if(window.fixtureWindowAccessor!==1)throw Error('window member read');
window.fixtureWindowAccessor=2;if(fixtureWindowAccessor!==2)throw Error('window name read');
delete window.fixtureWindowAccessor;
document.f.setAttribute('data-accessor','ready');
{submit_script}"#
                        )
                    } else if symbol_bootstrap {
                        // Original explicit calls: no automatic disposal or
                        // native iterator protocol is needed by this fixture.
                        format!(
                            r#"if(!Symbol.iterator)Symbol.iterator=Symbol('fixture iterator');
if(!Array.prototype[Symbol.iterator])Array.prototype[Symbol.iterator]=function(){{
var list=this,index=0;return {{next:function(){{return index<list.length?
{{value:list[index++],done:false}}:{{value:undefined,done:true}};}}}};}};
var iterator=[3,4][Symbol.iterator]();
if(iterator.next().value!==3||iterator.next().value!==4||!iterator.next().done)throw Error('iterator initialization');
function SearchSession(){{this.closed=false;}}
SearchSession.prototype[Symbol.dispose]=function(){{this.closed=true;}};
var session=new SearchSession();session[Symbol.dispose]();
if(!session.closed)throw Error('explicit disposal');
var symbolState=Symbol('search fixture state');navigator[symbolState]=session;
if(navigator[symbolState]!==session)throw Error('symbol native expando');
document.f.setAttribute('data-symbol','ready');
{submit_script}"#
                        )
                    } else if deferred_body {
                        // The original initializer is created inside 44 call
                        // arguments, then runs a separate 20-call expression.
                        // Its body must finish before the Enter handler exists.
                        format!(
                            "function keep(value){{return value;}}var initialize={}function(){{var mode={}7{};if(mode!==7)throw Error('function initialization');document.f.setAttribute('data-function-body','ready');{submit_script}}}{};initialize();",
                            "keep(".repeat(44),
                            "keep(".repeat(20),
                            ")".repeat(20),
                            ")".repeat(44)
                        )
                    } else if long_dispatch {
                        // Original dispatcher fixture: no captured third-party
                        // source, challenge code, challenge URL or response.
                        let mut source = String::from("function selectSearchMode(mode){");
                        for arm in 0..300 {
                            if arm > 0 {
                                source.push_str("else ");
                            }
                            source.push_str(&format!("if(mode==={arm})return {arm};"));
                        }
                        source.push_str("else return -1;}if(selectSearchMode(299)!==299)throw Error('dispatcher initialization');document.f.setAttribute('data-dispatch','ready');");
                        source
                    } else {
                        String::new()
                    };
                    let enter_script = if deferred_body
                        || symbol_bootstrap
                        || accessor_bootstrap
                        || callable_bootstrap
                    {
                        ""
                    } else {
                        submit_script
                    };
                    let policy = if path.contains("-csp") {
                        "Content-Security-Policy: form-action 'none'\r\n"
                    } else {
                        ""
                    };
                    let body = format!(
                        r#"<!doctype html><html><head><title>{title}</title>
<style>body{{margin:20px}}h1{{font-size:24px}} textarea,input{{width:320px}} #results{{display:block}}</style></head><body>
<form id=f name=f method={method} action='{action}'>
{field}<input type=hidden name=source value=fixture><button id=send name=go value=search>Search</button></form>
{results_html}<script>
var changes=0;
document.f.q.oninput=function(){{changes++;}};
document.f.q.onkeydown=function(event){{if(event.key==='Enter' && this.value==='prevent newline')event.preventDefault();}};
document.f.onsubmit=function(){{if(document.f.q.value==='cancel')return false;}};
{bootstrap}
{enter_script}
</script></body></html>"#
                    );
                    (200, policy.to_owned(), body)
                } else {
                    (
                        404,
                        String::new(),
                        "<title>Fixture route missing</title>".to_owned(),
                    )
                };
                let response = format!("HTTP/1.1 {status} Fixture\r\nContent-Type: text/html;charset=utf-8\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let _ = stream.write_all(response.as_bytes());
            }
        });
        Self {
            address,
            stopped,
            requests,
            posts,
            worker: Some(worker),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    fn seen(&self, path: &str) -> bool {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .any(|request| request == path)
    }
}

impl Drop for Site {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.address);
        self.worker.take().unwrap().join().unwrap();
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&#39;")
}

fn decode(encoded: &str) -> String {
    let mut result = Vec::new();
    let mut bytes = encoded.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let a = bytes.next().unwrap();
            let b = bytes.next().unwrap();
            let hex = |value: u8| (value as char).to_digit(16).unwrap() as u8;
            result.push(hex(a) * 16 + hex(b));
        } else {
            result.push(if byte == b'+' { b' ' } else { byte });
        }
    }
    String::from_utf8(result).unwrap()
}

fn id(app: &App, id: &str) -> usize {
    app.document
        .nodes
        .iter()
        .enumerate()
        .find_map(|(node, _)| (app.document.attr(node, "id") == Some(id)).then_some(node))
        .unwrap()
}

fn click_control(app: &mut App, id_value: &str) {
    let node = id(app, id_value);
    let rect = app
        .page
        .controls
        .iter()
        .find(|rect| rect.node == node)
        .unwrap();
    let (x, y) = (rect.x + 6, rect.y + app.content_top() - app.scroll + 6);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
}

fn edit_query(app: &mut App, value: &str) {
    click_control(app, "q");
    assert!(app.focus == Focus::Control(id(app, "q")));
    app.key(0x41, true, false, false);
    app.paste(value);
    assert_eq!(app.forms.controls[&id(app, "q")].value.text(), value);
}

fn wait(app: &mut App) {
    app.wait_for_load();
    assert!(!app.failed, "{}", app.status);
    if let Some(script) = &app.scripting {
        assert_eq!(script.error_count(), 0, "{}", script.report());
    }
}

fn run_search_flow(area: bool, scripted: bool, bootstrap: &str) {
    let long_dispatch = bootstrap == "long";
    let deferred_body = bootstrap == "deep";
    let symbol_bootstrap = bootstrap == "symbol";
    let accessor_bootstrap = bootstrap == "accessor";
    let callable_bootstrap = bootstrap == "callable";
    let result = Site::new(None);
    let search = Site::new(Some(result.url("")));
    let kind = if callable_bootstrap {
        "area-script-callable"
    } else if accessor_bootstrap {
        "area-script-accessor"
    } else if symbol_bootstrap {
        "area-script-symbol"
    } else if deferred_body {
        "area-script-deep"
    } else if long_dispatch {
        "area-script-long"
    } else if scripted {
        "area-script"
    } else if area {
        "area"
    } else {
        "input"
    };
    let submitter = if scripted { "" } else { "&go=search" };
    let mut app = App::new();
    app.resize(1000, 700);
    app.open_address(&search.url(&format!("/home-{kind}")));
    wait(&mut app);
    assert_eq!(app.document.title, "Fixture home");
    if long_dispatch {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-dispatch"),
            Some("ready")
        );
    }
    if deferred_body {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-function-body"),
            Some("ready")
        );
    }
    if symbol_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-symbol"),
            Some("ready")
        );
    }
    if accessor_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-accessor"),
            Some("ready")
        );
    }
    if callable_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-callable"),
            Some("ready")
        );
    }
    edit_query(&mut app, "alpha");
    if area && !scripted {
        click_control(&mut app, "send");
    } else {
        app.key(0x0d, false, false, false);
    }
    wait(&mut app);
    let first = format!("/search-{kind}?q=alpha&source=fixture{submitter}");
    assert!(search.seen(&first));
    assert_eq!(app.location.display(), search.url(&first));
    assert_eq!(app.document.title, "Fixture results");
    let rendered = app
        .page
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(rendered.contains("Original fixture result"), "{rendered}");
    assert!(rendered.contains("Independent text shown"), "{rendered}");
    let run = app
        .page
        .runs
        .iter()
        .find(|run| run.link.is_some() && run.text.contains("Original"))
        .unwrap();
    let (x, y) = (run.x + 3, run.y + app.content_top() - app.scroll + 3);
    assert_eq!(
        app.page.link_at(x, y - app.content_top() + app.scroll),
        Some("/visit?entry=original")
    );
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    wait(&mut app);
    assert!(search.seen("/visit?entry=original"));
    assert!(result.seen("/result"));
    assert_eq!(app.location.display(), result.url("/result"));
    assert_eq!(app.document.title, "External fixture result");
    app.key(0x25, false, false, true); // Actual Alt+Left browser action.
    wait(&mut app);
    assert_eq!(app.location.display(), search.url(&first));
    edit_query(&mut app, "beta + café");
    if scripted {
        app.key(0x0d, false, false, false);
    } else {
        click_control(&mut app, "send");
    }
    wait(&mut app);
    let second = format!("/search-{kind}?q=beta+%2B+caf%C3%A9&source=fixture{submitter}");
    assert!(search.seen(&second));
    assert_eq!(app.location.display(), search.url(&second));
    assert_eq!(
        app.forms.controls[&id(&app, "q")].value.text(),
        "beta + café"
    );
    assert_eq!(app.document.title, "Fixture results");
    if long_dispatch {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-dispatch"),
            Some("ready")
        );
    }
    if deferred_body {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-function-body"),
            Some("ready")
        );
    }
    if symbol_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-symbol"),
            Some("ready")
        );
    }
    if accessor_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-accessor"),
            Some("ready")
        );
    }
    if callable_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-callable"),
            Some("ready")
        );
    }
    let image = app.draw();
    assert_eq!(image.width, 1000);
    assert_eq!(image.height, 700);
}

#[test]
fn input_search_renders_results_follows_cross_origin_result_and_searches_again() {
    run_search_flow(false, false, "");
}

#[test]
fn textarea_search_renders_results_follows_cross_origin_result_and_searches_again() {
    run_search_flow(true, false, "");
}

#[test]
fn textarea_enter_edits_lines_and_submit_cancellation_preserves_the_current_document() {
    let search = Site::new(None);
    let mut app = App::new();
    app.open_address(&search.url("/home-area"));
    wait(&mut app);
    edit_query(&mut app, "first");
    app.key(0x0d, false, false, false);
    app.paste("second");
    let q = id(&app, "q");
    assert_eq!(app.forms.controls[&q].value.text(), "first\nsecond");
    assert_eq!(app.location.display(), search.url("/home-area"));
    assert_eq!(search.requests.lock().unwrap().len(), 1);
    edit_query(&mut app, "prevent newline");
    app.key(0x0d, false, false, false);
    assert_eq!(app.forms.controls[&q].value.text(), "prevent newline");
    edit_query(&mut app, "cancel");
    click_control(&mut app, "send");
    assert!(!app.is_loading());
    assert_eq!(app.document.title, "Fixture home");
    assert_eq!(search.requests.lock().unwrap().len(), 1);
    edit_query(&mut app, "allowed");
    click_control(&mut app, "send");
    wait(&mut app);
    assert!(search.seen("/search-area?q=allowed&source=fixture&go=search"));
}

#[test]
fn textarea_scripted_enter_submits_snapshots_and_subsequent_searches() {
    run_search_flow(true, true, "");
}

#[test]
fn script_form_submit_bypasses_required_but_stop_discards_the_queued_request() {
    let search = Site::new(None);
    let mut app = App::new();
    app.open_address(&search.url("/home-area-script-required"));
    wait(&mut app);
    click_control(&mut app, "send");
    assert!(
        !app.is_loading(),
        "native button must retain required-field validation"
    );
    assert_eq!(search.requests.lock().unwrap().len(), 1);
    click_control(&mut app, "q");
    app.key(0x0d, false, false, false);
    assert!(
        app.is_loading(),
        "script submit should queue an empty required field"
    );
    app.stop_loading();
    app.poll_load();
    assert_eq!(
        search.requests.lock().unwrap().len(),
        1,
        "Stop must discard the unsent snapshot"
    );
    assert_eq!(
        app.location.display(),
        search.url("/home-area-script-required")
    );
    edit_query(&mut app, "");
    app.key(0x0d, false, false, false);
    wait(&mut app);
    assert!(search.seen("/search-area-script?q=&source=fixture"));
}

#[test]
fn form_action_csp_blocks_native_submits_with_scripting_enabled_or_disabled_and_script_submits() {
    let search = Site::new(None);
    for scripts in [true, false] {
        let mut app = App::new();
        app.set_javascript_enabled(scripts);
        app.open_address(&search.url("/home-area-csp"));
        wait(&mut app);
        edit_query(&mut app, "private");
        let count = search.requests.lock().unwrap().len();
        click_control(&mut app, "send");
        app.poll_load();
        assert!(!app.is_loading());
        assert_eq!(search.requests.lock().unwrap().len(), count);
        assert!(
            app.status.contains("Content Security Policy"),
            "{}",
            app.status
        );
    }
    let mut app = App::new();
    app.open_address(&search.url("/home-area-script-csp"));
    wait(&mut app);
    edit_query(&mut app, "private");
    let count = search.requests.lock().unwrap().len();
    app.key(0x0d, false, false, false);
    app.poll_load();
    assert_eq!(search.requests.lock().unwrap().len(), count);
    let page = app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(), 1, "{}", page.report());
    assert!(
        page.report().contains("Content Security Policy"),
        "{}",
        page.report()
    );
}

#[test]
fn scripted_post_preserves_body_and_reload_confirmation() {
    let search = Site::new(None);
    let mut app = App::new();
    app.open_address(&search.url("/home-area-post"));
    wait(&mut app);
    edit_query(&mut app, "café +&");
    app.key(0x0d, false, false, false);
    wait(&mut app);
    assert_eq!(app.document.title, "Script POST result");
    assert!(app.current_request.is_post());
    assert_eq!(
        search.posts.lock().unwrap().as_slice(),
        &[(
            "/post-result?keep=1".into(),
            "q=caf%C3%A9+%2B%26&source=fixture".into()
        )]
    );
    app.reload();
    assert!(
        app.resubmission.is_some(),
        "scripted POST must not silently replay on reload"
    );
    assert_eq!(search.posts.lock().unwrap().len(), 1);
}

#[test]
fn script_form_cross_origin_action_and_redirect_make_no_forbidden_request() {
    let sink = Site::new(None);
    let search = Site::new(Some(sink.url("")));
    for path in ["/home-area-script-cross", "/home-area-script-redirect"] {
        let mut app = App::new();
        app.open_address(&search.url(path));
        wait(&mut app);
        edit_query(&mut app, "private");
        app.key(0x0d, false, false, false);
        app.wait_for_load();
        assert_eq!(app.location.display(), search.url(path));
        assert!(
            sink.requests.lock().unwrap().is_empty(),
            "automatic form work must never reach the other origin"
        );
        if path.ends_with("cross") {
            let page = app.scripting.as_ref().unwrap();
            assert_eq!(page.error_count(), 1, "{}", page.report());
            assert!(
                page.report().contains("scheme, host and port"),
                "{}",
                page.report()
            );
        } else {
            assert!(search.seen("/form-redirect?q=private&source=fixture"));
            assert!(app.failed, "the origin-changing redirect must fail");
            assert!(app.status.contains("origin"), "{}", app.status);
        }
    }
}

#[test]
fn long_else_if_bootstrap_initializes_scripted_search_and_repeated_search() {
    run_search_flow(true, true, "long");
}

#[test]
fn deferred_function_bootstrap_initializes_scripted_search_and_repeated_search() {
    run_search_flow(true, true, "deep");
}

#[test]
fn symbol_bootstrap_initializes_scripted_search_and_repeated_search() {
    run_search_flow(true, true, "symbol");
}

#[test]
fn passive_accessor_probe_initializes_scripted_search_and_repeated_search() {
    run_search_flow(true, true, "accessor");
}

#[test]
fn callable_object_helpers_bootstrap_bound_search_and_repeated_search() {
    run_search_flow(true, true, "callable");
}
