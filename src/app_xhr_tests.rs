//! Original actual App/network/state/pixel checks over owned loopback servers.
use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
struct Route {
    path: String,
    status: u16,
    headers: String,
    body: Vec<u8>,
    delay: Duration,
}
struct Site {
    base: String,
    routes: Arc<Mutex<Vec<Route>>>,
    seen: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Site {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let routes = Arc::new(Mutex::new(Vec::<Route>::new()));
        let served = routes.clone();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let recorded = seen.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let ending = stop.clone();
        let thread = std::thread::spawn(move || {
            while !ending.load(Ordering::Acquire) {
                let (mut stream, _) = match listener.accept() {
                    Ok(pair) => pair,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(error) => panic!("owned App XHR listener: {error}"),
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut head = Vec::new();
                let mut byte = [0];
                while !head.ends_with(b"\r\n\r\n") {
                    if stream.read_exact(&mut byte).is_err() {
                        break;
                    }
                    head.push(byte[0]);
                    assert!(head.len() < 65536);
                }
                let head = String::from_utf8(head).unwrap();
                let length = head
                    .lines()
                    .find_map(|line| {
                        line.split_once(':')
                            .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                            .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                assert!(length < 1024 * 1024);
                let mut body = vec![0; length];
                stream.read_exact(&mut body).unwrap();
                let path = head.split_whitespace().nth(1).unwrap_or("").to_owned();
                recorded.lock().unwrap().push((head, body));
                let routes = served.lock().unwrap();
                let (status, headers, body, delay) = routes
                    .iter()
                    .find(|route| route.path == path)
                    .map(|route| {
                        (
                            route.status,
                            route.headers.as_str(),
                            route.body.as_slice(),
                            route.delay,
                        )
                    })
                    .unwrap_or((404, "", b"Missing", Duration::ZERO));
                if !delay.is_zero() {
                    std::thread::sleep(delay);
                }
                let head=format!("HTTP/1.1 {status} Native\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",body.len());
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body);
            }
        });
        Self {
            base,
            routes,
            seen,
            stop,
            thread: Some(thread),
        }
    }
    fn put(&self, path: &str, status: u16, headers: &str, body: &[u8]) {
        self.routes.lock().unwrap().push(Route {
            path: path.into(),
            status,
            headers: headers.into(),
            body: body.to_vec(),
            delay: Duration::ZERO,
        });
    }
    fn delayed(&self, path: &str, body: &[u8]) {
        self.routes.lock().unwrap().push(Route {
            path: path.into(),
            status: 200,
            headers: "Content-Type: text/plain\r\n".into(),
            body: body.to_vec(),
            delay: Duration::from_millis(70),
        });
    }
    fn page(&self, path: &str, script: &str, headers: &str) {
        let body=format!("<!doctype html><html><head></head><body><p id=out>original</p><script>{script}</script></body></html>");
        self.put(
            path,
            200,
            &format!("Content-Type: text/html;charset=utf-8\r\n{headers}"),
            body.as_bytes(),
        );
    }
    fn open(&self, app: &mut App, path: &str) {
        app.open_address(&format!("{}{path}", self.base));
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
    }
    fn count(&self, path: &str) -> usize {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|(head, _)| head.split_whitespace().nth(1) == Some(path))
            .count()
    }
}
impl Drop for Site {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            if let Err(error) = thread.join() {
                if !std::thread::panicking() {
                    std::panic::resume_unwind(error);
                }
            }
        }
    }
}
fn until(app: &mut App, mut ready: impl FnMut(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while !ready(app) {
        app.poll_load();
        app.poll_page_timer();
        assert!(
            Instant::now() < deadline,
            "{}",
            app.scripting
                .as_ref()
                .map(|page| page.report())
                .unwrap_or_else(|| app.status.clone())
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn clean(app: &App) {
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
}
fn node(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len())
        .find(|&node| app.document.attr(node, "id") == Some(name))
        .unwrap()
}
#[test]
fn xhr_actual_app_request_paints_real_response_and_preserves_editor_and_link_targets() {
    let site = Site::new();
    let html = r#"<!doctype html><html><head><style>
html,body{margin:0;background:white}#neighbor{position:absolute;left:20px;top:20px;width:200px;height:32px}#go{position:absolute;left:260px;top:20px;width:120px;height:32px}#out{position:absolute;left:20px;top:100px;width:300px;height:40px;margin:0;color:#123456;background:white;font-size:24px}#stable{display:block;position:absolute;left:20px;top:180px;width:100px;height:24px;background:#336699;color:white}
</style></head><body><input id=neighbor value=seed><button id=go>Request</button><p id=out>Fallback</p><a id=stable href=#destination>Go</a><div id=destination></div><script>
var xhr,states='';document.getElementById('go').onclick=function(e){if(!e.isTrusted)throw 'real activation';xhr=new XMLHttpRequest();xhr.onreadystatechange=function(e){if(!e.isTrusted||e.target!==xhr||this!==xhr)throw 'native ready';states+=xhr.readyState+';';if(xhr.readyState===2&&(xhr.status!==200||xhr.getResponseHeader('X-Original')!=='live'))throw 'real headers';if(xhr.readyState===3&&xhr.response!==xhr.responseText)throw 'live text response';};xhr.onload=function(e){if(!(e instanceof ProgressEvent)||!e.isTrusted||xhr.status!==200||xhr.statusText!=='Native'||xhr.responseURL.indexOf('/actual')<0||xhr.responseText!=='Actual network')throw 'real completion';document.getElementById('out').textContent=xhr.responseText;document.title='Native XHR painted';};xhr.open('GET','/jump');xhr.send();};
</script></body></html>"#;
    site.put(
        "/home",
        200,
        "Content-Type: text/html;charset=utf-8\r\nContent-Security-Policy: connect-src 'self'\r\n",
        html.as_bytes(),
    );
    site.put("/jump", 303, "Location: /actual\r\n", b"");
    site.put(
        "/actual",
        200,
        "Content-Type: text/plain;charset=utf-8\r\nX-Original: live\r\n",
        b"Actual network",
    );
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    site.open(&mut app, "/home");
    clean(&app);
    let neighbor = node(&app, "neighbor");
    app.forms
        .script_set_value(neighbor, "edited neighbor")
        .unwrap();
    {
        let editor = &mut app.forms.controls.get_mut(&neighbor).unwrap().value;
        editor.cursor = 5;
        editor.anchor = 2;
        editor.offset = 1;
    }
    let before = app.draw().pixels;
    let go = node(&app, "go");
    let rect = app
        .page
        .controls
        .iter()
        .find(|rect| rect.node == go)
        .unwrap();
    let (x, y) = (
        rect.x + rect.width / 2,
        app.content_top() + rect.y + rect.height / 2 - app.scroll,
    );
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    until(&mut app, |app| app.document.title == "Native XHR painted");
    clean(&app);
    let page = app.scripting.as_mut().unwrap();
    assert_eq!(
        page.console_eval(
            &mut app.document,
            &mut app.forms,
            "if(states!=='1;2;3;4;'||document.getElementById('out').textContent!=='Actual network')throw states;"
        )
        .errors,
        0,
        "{}",
        page.report()
    );
    let text: String = app
        .page
        .runs
        .iter()
        .filter(|run| run.x >= 20 && run.x < 320 && run.y >= 100 && run.y < 140)
        .map(|run| run.text.as_str())
        .collect();
    assert_eq!(text, "Actualnetwork");
    let first = app
        .page
        .runs
        .iter()
        .find(|run| run.text == "Actual")
        .unwrap();
    let second = app
        .page
        .runs
        .iter()
        .find(|run| run.text == "network")
        .unwrap();
    assert!(
        second.x > first.x + first.width(),
        "The rendered words retain their real space gap."
    );
    let after = app.draw();
    assert_ne!(before, after.pixels);
    let top = (app.content_top() + 100 - app.scroll) as usize;
    assert!((top..top + 40)
        .any(|y| after.pixels[y * after.width + 20..y * after.width + 320].contains(&0x123456)));
    let editor = &app.forms.controls[&neighbor].value;
    assert_eq!(
        (editor.text(), editor.cursor, editor.anchor, editor.offset),
        ("edited neighbor".into(), 5, 2, 1)
    );
    assert_eq!(app.page.link_node_at(70, 192), Some(node(&app, "stable")));
    assert_eq!(app.page.link_at(70, 192), Some("#destination"));
    assert_eq!(site.count("/jump"), 1);
    assert_eq!(site.count("/actual"), 1);
}
#[test]
fn xhr_actual_app_post_cookie_headers_progress_and_non_success_status_are_real() {
    let site = Site::new();
    site.page("/home",r#"
var r=new XMLHttpRequest(),uploaded=false;r.upload.onload=function(e){if(!e.isTrusted||!(e instanceof ProgressEvent)||e.loaded!==3||e.total!==3)throw 'upload bytes';uploaded=true;};r.onload=function(e){if(!uploaded||r.status!==418||r.statusText!=='Native'||r.responseText!=='{\"actual\":true}'||r.getResponseHeader('Set-Cookie')!==null||r.getAllResponseHeaders().indexOf('set-cookie')>=0||e.loaded!==15||e.total!==15)throw 'real POST result';document.title='Native POST complete';};r.open('POST','/post');r.withCredentials=true;r.setRequestHeader('X-Original','live');r.setRequestHeader('Cookie','script=forbidden');r.send(new Uint8Array([65,0,66]));
"#,"");
    site.put(
        "/post",
        418,
        "Content-Type: application/json\r\nSet-Cookie: server_http=real; Path=/; HttpOnly\r\n",
        b"{\"actual\":true}",
    );
    let mut app = App::new();
    app.set_javascript_enabled(true);
    let url = crate::url::parse(&format!("{}/home", site.base)).unwrap().0;
    app.cookies
        .set_http(&url, "browser_cookie=kept; Path=/; SameSite=Strict")
        .unwrap();
    site.open(&mut app, "/home");
    until(&mut app, |app| app.document.title == "Native POST complete");
    clean(&app);
    let seen = site.seen.lock().unwrap();
    let (head, body) = seen
        .iter()
        .find(|(head, _)| head.starts_with("POST /post"))
        .unwrap();
    assert_eq!(body, b"A\0B");
    assert!(head.contains("Cookie: browser_cookie=kept"));
    assert!(!head.contains("script=forbidden"));
    assert!(head.contains("X-Original: live"));
    drop(seen);
    assert!(app
        .cookies
        .request_header(&url, true, false)
        .contains("server_http=real"));
    assert!(!app.cookies.document(&url).contains("server_http="));
}
#[test]
fn xhr_actual_app_csp_denial_is_async_real_error_without_contact() {
    let site = Site::new();
    site.page("/home",r#"
var r=new XMLHttpRequest(),returned=false;r.onerror=function(e){if(!returned||!e.isTrusted||!(e instanceof ProgressEvent)||r.readyState!==4||r.status!==0||r.responseText!==''||r.responseURL!=='')throw 'native request failure';document.title='CSP request blocked';};r.open('GET','/never');r.send();returned=true;
"#,"Content-Security-Policy: connect-src 'none'\r\n");
    let mut app = App::new();
    app.set_javascript_enabled(true);
    site.open(&mut app, "/home");
    app.wait_for_script_timers(Duration::from_millis(500));
    assert_eq!(app.document.title, "CSP request blocked");
    clean(&app);
    assert_eq!(site.count("/never"), 0);
}
#[test]
fn xhr_actual_app_reopen_during_headers_cancels_old_delivery_and_loads_only_new_generation() {
    let site = Site::new();
    site.page("/home",r#"
var r=new XMLHttpRequest(),switched=false,loads=0;r.onreadystatechange=function(){if(r.readyState===2&&!switched){switched=true;r.open('GET','/second');r.send();}};r.onload=function(e){loads++;if(r.responseText!=='SECOND'||r.responseURL.indexOf('/second')<0||loads!==1)throw 'old generation delivered';document.title='Current generation only';};r.open('GET','/first');r.send();
"#,"");
    site.put("/first", 200, "Content-Type: text/plain\r\n", b"FIRST");
    site.put("/second", 200, "Content-Type: text/plain\r\n", b"SECOND");
    let mut app = App::new();
    app.set_javascript_enabled(true);
    site.open(&mut app, "/home");
    until(&mut app, |app| {
        app.document.title == "Current generation only"
    });
    clean(&app);
    assert_eq!(site.count("/first"), 1);
    assert_eq!(site.count("/second"), 1);
}
#[test]
fn xhr_actual_app_navigation_cancels_old_document_request_and_new_document_keeps_its_completion() {
    let site = Site::new();
    site.page("/old",r#"var r=new XMLHttpRequest();r.onload=function(){document.title='OLD escaped';};r.open('GET','/slow');r.send();"#,"");
    site.delayed("/slow", b"OLD");
    site.page("/new",r#"var r=new XMLHttpRequest();r.onload=function(){if(this.responseText!=='NEW')throw 'wrong document data';document.title='NEW native';};r.open('GET','/new-data');r.send();"#,"");
    site.put("/new-data", 200, "Content-Type: text/plain\r\n", b"NEW");
    let mut app = App::new();
    app.set_javascript_enabled(true);
    site.open(&mut app, "/old");
    until(&mut app, |_| site.count("/slow") == 1);
    site.open(&mut app, "/new");
    until(&mut app, |app| app.document.title == "NEW native");
    app.wait_for_script_timers(Duration::from_millis(100));
    assert_eq!(app.document.title, "NEW native");
    clean(&app);
    assert_eq!(site.count("/new-data"), 1);
}

#[test]
fn xhr_actual_app_timeout_is_not_success_and_head_never_enters_loading() {
    let site = Site::new();
    site.page("/timeout",r#"var r=new XMLHttpRequest();r.ontimeout=function(e){if(!e.isTrusted||r.readyState!==4||r.status!==0||r.responseText!=='')throw 'real timeout';document.title='Native timeout';};r.onload=function(){throw 'timeout fabricated success';};r.open('GET','/slow');r.timeout=1;r.send();"#,"");
    site.delayed("/slow", b"late");
    site.page("/head",r#"var r=new XMLHttpRequest(),states='';r.onreadystatechange=function(){states+=r.readyState+';';};r.onload=function(e){if(r.responseText!==''||r.status!==204||states!=='1;2;4;'||e.loaded!==0)throw 'real HEAD state';document.title='Native HEAD';};r.open('HEAD','/head-data');r.send('ignored');"#,"");
    site.put("/head-data", 204, "Content-Type: text/plain\r\n", b"");
    let mut app = App::new();
    app.set_javascript_enabled(true);
    site.open(&mut app, "/timeout");
    until(&mut app, |app| app.document.title == "Native timeout");
    clean(&app);
    site.open(&mut app, "/head");
    until(&mut app, |app| app.document.title == "Native HEAD");
    clean(&app);
}
#[test]
fn xhr_actual_app_mime_override_before_open_decodes_native_bytes() {
    let site = Site::new();
    site.page("/home",r#"var r=new XMLHttpRequest();r.overrideMimeType('text/plain; charset="windows-1252"');r.open('GET','/text');r.onload=function(){if(r.responseText!=='A\u20ac')throw 'real charset override';document.getElementById('out').textContent=r.responseText;document.title='Native charset';};r.send();"#,"");
    site.put(
        "/text",
        200,
        "Content-Type: text/plain;charset=utf-8\r\n",
        &[b'A', 0x80],
    );
    let mut app = App::new();
    app.set_javascript_enabled(true);
    site.open(&mut app, "/home");
    until(&mut app, |app| app.document.title == "Native charset");
    clean(&app);
}

#[test]
fn xhr_actual_app_done_callback_resend_keeps_new_upload_and_terminal_events() {
    let site = Site::new();
    site.page("/home",r#"
var r=new XMLHttpRequest(),second=false,uploads=0,loads=0;
r.upload.onload=function(e){if(e.loaded!==3||e.total!==3)throw 'upload byte count';uploads++;};
r.onreadystatechange=function(){if(r.readyState===4&&!second){second=true;r.open('POST','/second');r.send('NEW');}};
r.onload=function(){loads++;if(r.responseText!=='SECOND'||uploads!==1||loads!==1)throw 'new generation upload lost';document.title='Resent upload complete';};
r.open('GET','/first');r.send();
"#,"");
    site.put("/first", 200, "Content-Type: text/plain\r\n", b"FIRST");
    site.put("/second", 200, "Content-Type: text/plain\r\n", b"SECOND");
    let mut app = App::new();
    app.set_javascript_enabled(true);
    site.open(&mut app, "/home");
    until(&mut app, |app| {
        app.document.title == "Resent upload complete"
    });
    clean(&app);
    assert_eq!(site.count("/second"), 1);
    assert!(site
        .seen
        .lock()
        .unwrap()
        .iter()
        .any(|(head, body)| head.starts_with("POST /second ") && body == b"NEW"));
}

#[test]
fn fetch_headers_actual_app_shared_xhr_boundary_preserves_transport_and_cookie_guards() {
    let site=Site::new();
    site.page("/headers-home",r#"
var h=new Headers([['X-Original','one'],['x-original','two'],['Content-Type','application/json'],['Cookie','author=forbidden'],['Host','forbidden.test']]);
var r=new XMLHttpRequest();r.onload=function(){if(r.status!==201||r.responseText!=='header-response'||r.getResponseHeader('Set-Cookie')!==null)throw 'real filtered response';document.getElementById('out').textContent=r.responseText;document.title='Headers transport complete';};
r.open('POST','/headers-post');h.forEach(function(value,name){r.setRequestHeader(name,value);});r.send('{"real":true}');
"#,"");
    site.put("/headers-post",201,"Content-Type: text/plain\r\nSet-Cookie: private_http=server; Path=/; HttpOnly\r\n",b"header-response");
    let mut app=App::new();app.set_javascript_enabled(true);
    let url=crate::url::parse(&format!("{}/headers-home",site.base)).unwrap().0;
    app.cookies.set_http(&url,"browser_cookie=kept; Path=/; SameSite=Strict").unwrap();
    site.open(&mut app,"/headers-home");until(&mut app,|app|app.document.title=="Headers transport complete");clean(&app);
    let seen=site.seen.lock().unwrap();let (head,body)=seen.iter().find(|(head,_)|head.starts_with("POST /headers-post")).unwrap();let lower=head.to_ascii_lowercase();
    assert_eq!(body,b"{\"real\":true}");assert!(lower.contains("x-original: one, two"));assert!(lower.contains("content-type: application/json"));assert!(lower.contains("cookie: browser_cookie=kept"));assert!(!lower.contains("author=forbidden")&&!lower.contains("forbidden.test"));drop(seen);
    assert!(app.page.runs.iter().any(|run|run.text=="header-response"));
    assert!(!app.cookies.document(&url).contains("private_http"));
}
