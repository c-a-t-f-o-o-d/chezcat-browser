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
fn xhr_actual_app_charset_selection_fallback_replacement_and_bom_use_network_bytes() {
    let site = Site::new();
    // These requests explicitly use responseType text; default XML sniffing is separate.
    let cases: &[(&str, Option<&str>, &[u8], &str)] = &[
        ("text/plain;charset=windows-1252", None, &[65, 0x80], r#"'A\u20ac'"#),
        ("text/plain;charset=windows-1252", Some("text/plain"), &[65, 0x80], r#"'A\u20ac'"#),
        ("text/plain;charset=windows-1252", Some("text/plain;charset=unknown"), &[0xc3, 0xa9], r#"'\u00e9'"#),
        ("text/plain;charset=windows-1252", Some("text/plain;charset=\"\""), &[0xc3, 0xa9], r#"'\u00e9'"#),
        ("text/plain;charset=windows-1252", Some("text/plain;charset=replacement"), b"no source text", r#"'\ufffd'"#),
        ("text/plain;charset=unknown", None, &[0x80, 65], r#"'\ufffdA'"#),
        ("text/plain;charset=replacement", None, b"no source text", r#"'\ufffd'"#),
        ("text/plain;charset=iso-2022-cn", None, &[0xef, 0xbb, 0xbf, 0xc3, 0xa9], r#"'\u00e9'"#),
        ("text/plain;charset=iso-2022-kr", None, &[0xff, 0xfe, 65, 0], "'A'"),
        ("text/plain;charset=hz-gb-2312", None, &[0xfe, 0xff, 0, 65], "'A'"),
        ("text/plain;charset=replacement", None, b"", "''"),
        ("text/plain;charset=utf-8", Some("text/plain;charset=\" \tSHIFT_JIS \""), &[0x82, 0xa0], r#"'\u3042'"#),
        ("application/xml;charset=unknown", None, &[0xc3, 0xa9], r#"'\u00e9'"#),
    ];
    let mut app = App::new();
    app.set_javascript_enabled(true);
    for (index, &(content_type, override_mime, bytes, expected_source)) in cases.iter().enumerate() {
        let path = format!("/charset-{index}");
        let home = format!("/charset-home-{index}");
        let title = format!("Native charset matrix {index}");
        let override_source = override_mime
            .map(|mime| format!("r.overrideMimeType({mime:?});"))
            .unwrap_or_default();
        let states = if bytes.is_empty() { "1;2;4;" } else { "1;2;3;4;" };
        let script = format!(r#"
var r=new XMLHttpRequest(),states='',loads=0,errors=0;
r.onreadystatechange=function(e){{if(!e.isTrusted||e.target!==r)throw 'real charset state';states+=r.readyState+';';}};
r.onload=function(e){{if(!e.isTrusted||!(e instanceof ProgressEvent)||e.target!==r||r.readyState!==4||r.status!==200||r.statusText!=='Native'||r.responseText!=={expected_source}||r.response!==r.responseText||r.responseURL.indexOf({path:?})<0)throw 'real charset text';if(r.getResponseHeader('Content-Type')!=={content_type:?}||e.loaded!=={length}||e.total!=={length}||e.lengthComputable!=={computable})throw 'real charset headers and bytes';loads++;document.getElementById('out').textContent=r.responseText;}};
r.onerror=function(){{errors++;}};
r.onloadend=function(e){{if(!e.isTrusted||loads!==1||errors!==0||states!=={states:?}||e.loaded!=={length})throw 'real charset terminal events';document.title={title:?};}};
r.open('GET',{path:?});r.responseType='text';{override_source}r.send();
"#, length=bytes.len(), computable=!bytes.is_empty());
        site.put(&path, 200, &format!("Content-Type: {content_type}\r\n"), bytes);
        site.page(&home, &script, "Content-Security-Policy: connect-src 'self'\r\n");
        site.open(&mut app, &home);
        until(&mut app, |app| app.document.title == title || app.scripting.as_ref().is_some_and(|page| page.error_count() != 0));
        clean(&app);
        assert_eq!(app.document.title, title, "{content_type}, {override_mime:?}");
        let page = app.scripting.as_mut().unwrap();
        assert_eq!(page.console_eval(&mut app.document, &mut app.forms, &format!(
            "if(document.getElementById('out').textContent!=={expected_source}||loads!==1||errors!==0)throw 'charset actual document consumer';"
        )).errors, 0, "{}", page.report());
        assert_eq!(page.native_request_pending_count(), 0);
        assert_eq!(site.count(&path), 1);
    }
}

#[test]
fn xhr_xml_default_actual_app_decodes_declaration_and_paints_text() {
    let site = Site::new();
    let bytes = b"<?xml version='1.0' encoding='Shift_JIS'?><r>\x82\xa0</r>";
    site.put("/xml-text", 200, "Content-Type: application/xml\r\n", bytes);
    let html=r#"<!doctype html><style>html,body{margin:0;background:white}#out{position:absolute;left:20px;top:100px;width:300px;height:40px;margin:0;color:#123456;font-size:24px}#neighbor{position:absolute;left:20px;top:20px;width:200px;height:32px}#stable{position:absolute;left:20px;top:180px;width:100px;height:24px}</style><body><input id=neighbor value=seed><p id=out>waiting</p><a id=stable href=#destination>Go</a><div id=destination></div><script>
var r=new XMLHttpRequest(),loads=0;
r.onload=function(e){if(!e.isTrusted||e.loaded!==51||r.responseText!=="<?xml version='1.0' encoding='Shift_JIS'?><r>\u3042</r>"||r.response!==r.responseText||r.status!==200)throw 'XML network response';loads++;document.getElementById('out').textContent='XML '+r.responseText.slice(-5,-4);document.title='XML default decoded';};
r.open('GET','/xml-text');r.send();
</script></body>"#;
    site.put("/xml-home",200,"Content-Type: text/html;charset=utf-8\r\nContent-Security-Policy: connect-src 'self'\r\n",html.as_bytes());
    assert_eq!(bytes.len(), 51);
    let mut app=App::new(); app.resize(640,480); app.set_javascript_enabled(true);
    site.open(&mut app,"/xml-home");
    let neighbor=node(&app,"neighbor");
    app.forms.script_set_value(neighbor,"edited neighbor").unwrap();
    { let editor=&mut app.forms.controls.get_mut(&neighbor).unwrap().value;editor.cursor=5;editor.anchor=2;editor.offset=1; }
    let before=app.draw().pixels;
    until(&mut app, |app| app.document.title=="XML default decoded" || app.scripting.as_ref().is_some_and(|page| page.error_count()!=0));
    clean(&app); assert_eq!(app.document.title,"XML default decoded");
    let page=app.scripting.as_mut().unwrap();
    assert_eq!(page.console_eval(&mut app.document,&mut app.forms,"if(document.getElementById('out').textContent!=='XML \\u3042'||loads!==1)throw 'XML painted consumer';").errors,0);
    assert!(app.page.runs.iter().any(|run|run.text.contains('あ')));
    let after=app.draw();assert_ne!(before,after.pixels);
    let top=(app.content_top()+100-app.scroll) as usize;
    assert!((top..top+40).any(|y|after.pixels[y*after.width+20..y*after.width+320].contains(&0x123456)));
    let editor=&app.forms.controls[&neighbor].value;
    assert_eq!((editor.text(),editor.cursor,editor.anchor,editor.offset),("edited neighbor".into(),5,2,1));
    assert_eq!(app.page.link_node_at(70,192),Some(node(&app,"stable")));
    assert_eq!(app.page.link_at(70,192),Some("#destination"));
    assert_eq!(site.count("/xml-text"),1);
}

#[test]
fn xhr_actual_app_charset_replacement_response_paints_and_preserves_editor_and_link_targets() {
    let site = Site::new();
    let bytes = b"<script>source must not appear</script>";
    let html = r#"<!doctype html><html><head><style>
html,body{margin:0;background:white}#neighbor{position:absolute;left:20px;top:20px;width:200px;height:32px}#go{position:absolute;left:260px;top:20px;width:120px;height:32px}#out{position:absolute;left:20px;top:100px;width:300px;height:40px;margin:0;color:#123456;background:white;font-size:24px}#stable{display:block;position:absolute;left:20px;top:180px;width:100px;height:24px;background:#336699;color:white}
</style></head><body><input id=neighbor value=seed><button id=go>Request</button><p id=out>Fallback</p><a id=stable href=#destination>Go</a><div id=destination></div><script>
var xhr,states='',loads=0,errors=0,loaded=-1,total=-1;
document.getElementById('go').onclick=function(e){if(!e.isTrusted)throw 'real charset activation';xhr=new XMLHttpRequest();xhr.onreadystatechange=function(e){if(!e.isTrusted||e.target!==xhr)throw 'real replacement state';states+=xhr.readyState+';';};xhr.onload=function(e){if(!e.isTrusted||!(e instanceof ProgressEvent)||xhr.status!==200||xhr.statusText!=='Native'||xhr.responseText!=='\ufffd'||xhr.response!==xhr.responseText)throw 'real replacement result';loads++;loaded=e.loaded;total=e.total;document.getElementById('out').textContent='Decoded '+xhr.responseText;};xhr.onerror=function(){errors++;};xhr.onloadend=function(e){if(!e.isTrusted||loads!==1||errors!==0||states!=='1;2;3;4;')throw 'real replacement completion';document.title='Native replacement painted';};xhr.open('GET','/replacement');xhr.responseType='text';xhr.send();};
</script></body></html>"#;
    site.put(
        "/replacement-home",
        200,
        "Content-Type: text/html;charset=utf-8\r\nContent-Security-Policy: connect-src 'self'\r\n",
        html.as_bytes(),
    );
    site.put(
        "/replacement",
        200,
        "Content-Type: text/plain;charset=iso-2022-cn-ext\r\n",
        bytes,
    );
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    site.open(&mut app, "/replacement-home");
    clean(&app);
    let neighbor = node(&app, "neighbor");
    app.forms.script_set_value(neighbor, "edited neighbor").unwrap();
    {
        let editor = &mut app.forms.controls.get_mut(&neighbor).unwrap().value;
        editor.cursor = 5;
        editor.anchor = 2;
        editor.offset = 1;
    }
    let before = app.draw().pixels;
    let go = node(&app, "go");
    let rect = app.page.controls.iter().find(|rect| rect.node == go).unwrap();
    let (x, y) = (
        rect.x + rect.width / 2,
        app.content_top() + rect.y + rect.height / 2 - app.scroll,
    );
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    until(&mut app, |app| app.document.title == "Native replacement painted" || app.scripting.as_ref().is_some_and(|page| page.error_count() != 0));
    clean(&app);
    assert_eq!(app.document.title, "Native replacement painted");
    let page = app.scripting.as_mut().unwrap();
    assert_eq!(page.console_eval(&mut app.document, &mut app.forms, &format!(
        "if(document.getElementById('out').textContent!=='Decoded \\ufffd'||loads!==1||errors!==0||loaded!=={length}||total!=={length})throw 'replacement painted actual bytes';",
        length=bytes.len(),
    )).errors, 0, "{}", page.report());
    let text: String = app.page.runs.iter()
        .filter(|run| run.x >= 20 && run.x < 320 && run.y >= 100 && run.y < 140)
        .map(|run| run.text.as_str())
        .collect();
    assert_eq!(text, "Decoded\u{fffd}");
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
    assert_eq!(site.count("/replacement"), 1);
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

#[test]
fn fetch_response_actual_app_consumes_real_xhr_json_in_queued_native_body_promise() {
    let site=Site::new();site.page("/response-home",r#"
var out=document.getElementById('out');out.style.cssText='position:absolute;left:10px;top:10px;width:200px;height:30px;color:#123456';var neighbor=document.createElement('input');neighbor.id='body-neighbor';neighbor.value='seed';neighbor.style.cssText='position:absolute;left:10px;top:50px;width:180px;height:30px';document.body.appendChild(neighbor);var stable=document.createElement('a');stable.id='body-stable';stable.href='#destination';stable.style.cssText='position:absolute;display:block;left:10px;top:100px;width:100px;height:24px;background-color:#2040e0';document.body.appendChild(stable);var request=new XMLHttpRequest();request.onload=function(){var response=new Response(request.responseText,{status:request.status,headers:{'Content-Type':request.getResponseHeader('Content-Type')}}),copy=response.clone();if(response.headers.get('content-type')!=='application/json'||response.bodyUsed||copy.bodyUsed)throw 'real response before consumption';var promise=response.json();if(!response.bodyUsed||copy.bodyUsed)throw 'independent native use';promise.then(function(data){if(data.actual!==true)throw 'real response JSON';document.getElementById('out').textContent=data.title;document.title='Response body complete';});};request.open('GET','/response-json');request.send();
"#,"");
    site.put("/response-json",200,"Content-Type: application/json\r\n",b"{\"actual\":true,\"title\":\"NativeBody\"}");
    site.routes.lock().unwrap().iter_mut().find(|route|route.path=="/response-json").unwrap().delay=Duration::from_millis(100);
    let mut app=App::new();app.set_javascript_enabled(true);site.open(&mut app,"/response-home");
    let neighbor=node(&app,"body-neighbor");let stable=node(&app,"body-stable");let r=app.page.controls.iter().find(|r|r.node==neighbor).unwrap();let(x,y)=(r.x+10,app.content_top()+r.y+10-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);app.paste("edited");
    let v=&app.forms.controls[&neighbor].value;let editor=(v.text(),v.cursor,v.anchor,v.offset);
    until(&mut app,|app|app.document.title=="Response body complete");clean(&app);
    assert_eq!(site.count("/response-json"),1);assert!(app.page.runs.iter().any(|run|run.text=="NativeBody"));
    let v=&app.forms.controls[&neighbor].value;assert_eq!((v.text(),v.cursor,v.anchor,v.offset),editor);
    assert_eq!(app.page.link_node_at(25,110),Some(stable));let canvas=app.draw();let top=(app.content_top()+10-app.scroll) as usize;
    assert!((top..top+30).any(|y|canvas.pixels[y*canvas.width+10..y*canvas.width+210].contains(&0x123456)));
    assert_eq!(canvas.pixels[(app.content_top()+110-app.scroll) as usize*canvas.width+25]&0xffffff,0x2040e0);
    let y=app.content_top()+110-app.scroll;app.pointer_down(25,y,false);app.pointer_up(25,y);assert_eq!(app.location.fragment,"destination");
}

#[test]
fn xhr_headers_decoding_actual_app_callback_selects_text_and_native_paint() {
    let site = Site::new();
    let bytes = b"<?xml version='1.0' encoding='Shift_JIS'?><r>\x82\xa0</r>";
    site.put("/xml-text", 200, "Content-Type: text/plain;charset=utf-8\r\n", bytes);
    let html=r#"<!doctype html><style>html,body{margin:0;background:white}#out{position:absolute;left:20px;top:100px;width:300px;height:40px;margin:0;color:#123456;font-size:24px}#neighbor{position:absolute;left:20px;top:20px;width:200px;height:32px}#stable{position:absolute;left:20px;top:180px;width:100px;height:24px}</style><body><input id=neighbor value=seed><p id=out>waiting</p><a id=stable href=#destination>Go</a><div id=destination></div><script>
var r=new XMLHttpRequest(),loads=0;
r.onload=function(e){if(!e.isTrusted||e.loaded!==51||r.responseText!=="<?xml version='1.0' encoding='Shift_JIS'?><r>\u3042</r>"||r.response!==r.responseText||r.status!==200)throw 'XML network response';loads++;document.getElementById('out').textContent='XML '+r.responseText.slice(-5,-4);document.title='XML default decoded';};
r.onreadystatechange=function(){if(r.readyState===2){if(r.responseText!==''||r.getResponseHeader('Content-Type')!=='text/plain;charset=utf-8')throw 'headers before body';r.overrideMimeType('text/plain;charset=Shift_JIS');}};r.open('GET','/xml-text');r.send();
</script></body>"#;
    site.put("/xml-home",200,"Content-Type: text/html;charset=utf-8\r\nContent-Security-Policy: connect-src 'self'\r\n",html.as_bytes());
    assert_eq!(bytes.len(), 51);
    let mut app=App::new(); app.resize(640,480); app.set_javascript_enabled(true);
    site.open(&mut app,"/xml-home");
    let neighbor=node(&app,"neighbor");
    app.forms.script_set_value(neighbor,"edited neighbor").unwrap();
    { let editor=&mut app.forms.controls.get_mut(&neighbor).unwrap().value;editor.cursor=5;editor.anchor=2;editor.offset=1; }
    let before=app.draw().pixels;
    until(&mut app, |app| app.document.title=="XML default decoded" || app.scripting.as_ref().is_some_and(|page| page.error_count()!=0));
    clean(&app); assert_eq!(app.document.title,"XML default decoded");
    let page=app.scripting.as_mut().unwrap();
    assert_eq!(page.console_eval(&mut app.document,&mut app.forms,"if(document.getElementById('out').textContent!=='XML \\u3042'||loads!==1)throw 'XML painted consumer';").errors,0);
    assert!(app.page.runs.iter().any(|run|run.text.contains('あ')));
    let after=app.draw();assert_ne!(before,after.pixels);
    let top=(app.content_top()+100-app.scroll) as usize;
    assert!((top..top+40).any(|y|after.pixels[y*after.width+20..y*after.width+320].contains(&0x123456)));
    let editor=&app.forms.controls[&neighbor].value;
    assert_eq!((editor.text(),editor.cursor,editor.anchor,editor.offset),("edited neighbor".into(),5,2,1));
    assert_eq!(app.page.link_node_at(70,192),Some(node(&app,"stable")));
    assert_eq!(app.page.link_at(70,192),Some("#destination"));
    assert_eq!(site.count("/xml-text"),1);
}
