//! Form navigation checks through actual controls, history and reload prompts.
use super::*;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
struct Seen {
    method: String,
    path: String,
    headers: String,
    body: String,
}

struct Site {
    base: String,
    seen: Arc<Mutex<Vec<Seen>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Site {
    fn new() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let recorded = seen.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let ending = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut clients = Vec::new();
            while !ending.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // Accepted Winsock sockets inherit the polling listener mode.
                        stream.set_nonblocking(false).unwrap();
                        let recorded = recorded.clone();
                        clients.push(std::thread::spawn(move || {
                            stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                            let mut bytes = Vec::new();
                            let mut byte = [0u8; 1];
                            while !bytes.ends_with(b"\r\n\r\n") && bytes.len() < 32 * 1024 {
                                if stream.read(&mut byte).unwrap_or(0) == 0 { return; }
                                bytes.push(byte[0]);
                            }
                            let headers = String::from_utf8(bytes).unwrap();
                            let method = headers.split_whitespace().next().unwrap().to_owned();
                            let path = headers.split_whitespace().nth(1).unwrap().to_owned();
                            let length: usize = headers.lines().find_map(|line| line.strip_prefix("Content-Length: "))
                                .map(|text| text.parse().unwrap()).unwrap_or(0);
                            assert!(length <= crate::http::MAX_FORM_BYTES);
                            let mut body = vec![0; length];
                            if stream.read_exact(&mut body).is_err() { return; }
                            let body = String::from_utf8(body).unwrap();
                            recorded.lock().unwrap().push(Seen { method: method.clone(), path: path.clone(), headers, body });
                            let mut status = 200;
                            let mut extra = "";
                            let mut mime = "text/html; charset=utf-8";
                            let body = match (method.as_str(), path.as_str()) {
                                ("GET", "/form" | "/slow-form" | "/redirect-form" | "/prevent") => {
                                    let action = match path.as_str() {
                                        "/slow-form" => "/slow",
                                        "/redirect-form" => "/redirect303",
                                        _ => "/submit?keep=yes",
                                    };
                                    let script = if path == "/prevent" { "<script>document.getElementById('f').onsubmit=function(event){event.preventDefault();document.title='Stayed here';};</script>" } else { "" };
                                    format!("<title>POST form</title><link rel=stylesheet href='/style.css'><form id=f action='{action}' method=post><input id=q name=q value=seed><input type=hidden name=token value=fixture><button id=send name=action value=proceed>Send</button></form>{script}")
                                }
                                ("GET", "/style.css") => { mime = "text/css"; "body{margin:20px} input{width:250px}".into() }
                                ("GET", "/image-get" | "/image-post" | "/image-scroll" | "/image-fixed" | "/image-tall" | "/image-fallback" | "/image-required" | "/image-cancel-click" | "/image-cancel-submit" | "/image-disabled" | "/image-disable-click") => image_form(&path),
                                ("GET", "/control.ppm") => { mime = "image/x-portable-pixmap"; "P3\n2 1\n255\n255 0 0 0 255 0\n".into() }
                                ("GET" | "POST", path) if path.starts_with("/image-result?") => "<title>Image result</title><h1>Received</h1>".into(),
                                ("POST", "/redirect303") => { status = 303; extra = "Location: /get-result\r\n"; String::new() }
                                ("POST", "/slow") => {
                                    std::thread::sleep(Duration::from_millis(400));
                                    "<title>Slow POST result</title><h1>Received</h1>".into()
                                }
                                ("POST", "/submit?keep=yes") => { extra = "Set-Cookie: tpag=do-not-record-this; Path=/\r\nSet-Cookie: ppag=also-private; Path=/\r\n"; "<title>POST result</title><link rel=stylesheet href='/style.css'><h1>Received</h1>".into() },
                                ("GET", "/submit?keep=yes") => "<title>Explicit GET result</title><p>Fetched again</p>".into(),
                                ("GET", "/get-result") => "<title>GET result</title><h1>Received</h1>".into(),
                                ("GET", "/next") => "<title>Next page</title><p>Next</p>".into(),
                                _ => { status = 404; "<title>Wrong request</title>".into() }
                            };
                            let reply = format!("HTTP/1.1 {status} Test\r\nContent-Type: {mime}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{body}", body.len());
                            let _ = stream.write_all(reply.as_bytes());
                        }));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("Loopback accept failed: {error}"),
                }
            }
            for client in clients {
                client.join().unwrap();
            }
        });
        Self {
            base: format!("http://127.0.0.1:{port}"),
            seen,
            stop,
            thread: Some(thread),
        }
    }

    fn open(&self, app: &mut App, path: &str) {
        app.open_address(&format!("{}{path}", self.base));
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
    }

    fn take(&self) -> Vec<Seen> {
        std::mem::take(&mut *self.seen.lock().unwrap())
    }

    fn wait_for_posts(&self, count: usize) {
        let started = Instant::now();
        while self
            .seen
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.method == "POST")
            .count()
            < count
        {
            assert!(
                started.elapsed() < Duration::from_secs(3),
                "POST did not arrive"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for Site {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            if let Err(failure) = thread.join() {
                if !std::thread::panicking() {
                    std::panic::resume_unwind(failure);
                }
            }
        }
    }
}

fn id(app: &App, value: &str) -> usize {
    app.document
        .nodes
        .iter()
        .enumerate()
        .find_map(|(node, _)| (app.document.attr(node, "id") == Some(value)).then_some(node))
        .unwrap()
}

fn submit(app: &mut App) {
    let node = id(app, "send");
    app.activate_control(node);
}

fn loaded_post(site: &Site) -> App {
    let mut app = App::new();
    site.open(&mut app, "/form");
    submit(&mut app);
    app.wait_for_load();
    assert_eq!(app.document.title, "POST result");
    assert!(app.current_request.is_post());
    assert!(!app.failed, "{}", app.status);
    site.take();
    app
}

fn image_form(path: &str) -> String {
    let method = if path == "/image-post" { "post" } else { "get" };
    let action = if method == "post" {
        "/image-result?keep=yes"
    } else {
        "/image-result?replace=yes"
    };
    let source = if path == "/image-fallback" {
        "/missing.ppm"
    } else {
        "/control.ppm"
    };
    let value = if path == "/image-required" {
        " required"
    } else {
        " value=seed"
    };
    let disabled = if path == "/image-disabled" {
        " disabled"
    } else {
        ""
    };
    let position = match path {
        "/image-fixed" => "#picture{position:fixed;left:60px;top:90px}",
        "/image-tall" => "#picture{height:700px}",
        _ => "",
    };
    let lead = if path == "/image-scroll" {
        "<div style='height:700px'></div>"
    } else {
        ""
    };
    let script = match path {
        "/image-cancel-click" => "document.getElementById('picture').onclick=function(event){if(document.title==='Image form'){event.preventDefault();document.title='Click canceled';}};",
        "/image-cancel-submit" => "document.getElementById('f').onsubmit=function(event){if(document.title==='Image form'){event.preventDefault();document.title='Submit canceled';}};",
        "/image-disable-click" => "document.getElementById('picture').onclick=function(event){document.getElementById('picture').disabled=true;document.title='Disabled by click';};",
        _ => "",
    };
    format!(
        "<title>Image form</title><style>body{{margin:20px}} #picture{{width:120px;height:60px;padding:8px;border:2px solid #234567;image-rendering:crisp-edges}} {position}</style>{lead}<form id=f action='{action}' method={method}><input id=q name=q{value}><input type=hidden name=token value=fixture><p><label for=picture>Activate</label></p><input id=picture type=image name=picture src='{source}' alt='Send image' width=16 height=8 value=unused{disabled}></form><div style='height:1600px'></div><script>{script}</script>"
    )
}

fn image_rect(app: &App) -> &crate::layout::ControlBox {
    let node = id(app, "picture");
    app.page
        .controls
        .iter()
        .find(|rect| rect.node == node)
        .unwrap()
}

fn image_view_point(app: &App, x: i32, y: i32) -> (i32, i32) {
    let rect = image_rect(app);
    let image = rect.image.as_ref().expect("Loaded native image control");
    (
        rect.x + image.x + (x as f32 * app.zoom).round() as i32,
        rect.y + image.y + app.content_top() - app.scroll + (y as f32 * app.zoom).round() as i32,
    )
}

fn click(app: &mut App, point: (i32, i32)) {
    assert!(point.1 >= app.content_top() && point.1 < app.height - FOOT_HEIGHT);
    app.pointer_down(point.0, point.1, false);
    app.pointer_up(point.0, point.1);
}

fn assert_image_submission(site: &Site, app: &mut App, post: bool, value: &str, point: (i32, i32)) {
    app.wait_for_load();
    assert!(!app.failed, "{}", app.status);
    assert_eq!(app.document.title, "Image result");
    assert_eq!(app.current_request.is_post(), post);
    let seen = site.take();
    assert_eq!(seen.len(), 1, "{seen:?}");
    let entries = format!(
        "q={value}&token=fixture&picture.x={}&picture.y={}",
        point.0, point.1
    );
    if post {
        assert_eq!(seen[0].method, "POST");
        assert_eq!(seen[0].path, "/image-result?keep=yes");
        assert_eq!(seen[0].body, entries);
    } else {
        assert_eq!(seen[0].method, "GET");
        assert_eq!(seen[0].path, format!("/image-result?{entries}"));
        assert!(seen[0].body.is_empty());
    }
}

#[test]
fn image_pointer_submits_css_coordinates_through_zoom_scroll_and_fixed_layout() {
    let site = Site::new();
    for (path, zoom, point) in [
        ("/image-get", 1.0, (12, 8)),
        ("/image-post", 1.5, (20, 10)),
        ("/image-scroll", 1.25, (16, 12)),
        ("/image-fixed", 1.5, (-4, -2)),
        ("/image-tall", 1.0, (12, 8)),
    ] {
        let mut app = App::new();
        app.resize(700, 650);
        site.open(&mut app, path);
        app.set_zoom(zoom);
        let image = image_rect(&app).image.as_ref().unwrap();
        let height = if path == "/image-tall" { 700.0 } else { 60.0 };
        assert_eq!(
            (image.width, image.height),
            ((120.0 * zoom) as i32, (height * zoom) as i32),
            "{path}"
        );
        // Intrinsic pixels and HTML dimensions differ from the CSS render size.
        let bitmap = &app.images[&id(&app, "picture")];
        assert_eq!((bitmap.width, bitmap.height), (2, 1));
        if path == "/image-scroll" {
            let top = image_rect(&app).y;
            app.set_scroll(top - 100);
            assert!(app.scroll > 0);
        } else if path == "/image-fixed" {
            let before = image_view_point(&app, point.0, point.1);
            app.set_scroll(350);
            assert_eq!(app.scroll, 350);
            assert_eq!(image_view_point(&app, point.0, point.1), before);
        }
        // Both bitmap colors must be painted at the rendered content position.
        let canvas = app.draw();
        for (x, color) in [(30, 0xff0000), (90, 0x00ff00)] {
            let pixel = image_view_point(&app, x, 30);
            assert_eq!(
                canvas.pixels[pixel.1 as usize * canvas.width + pixel.0 as usize],
                color,
                "{path}"
            );
        }
        site.take();
        let pixel = image_view_point(&app, point.0, point.1);
        click(&mut app, pixel);
        assert_image_submission(&site, &mut app, path == "/image-post", "seed", point);
    }
}

#[test]
fn image_fallback_and_keyboard_activation_use_initial_zero_coordinates() {
    let site = Site::new();
    for path in ["/image-fallback", "/image-get", "/image-post"] {
        let mut app = App::new();
        site.open(&mut app, path);
        site.take();
        let node = id(&app, "picture");
        assert_eq!(app.forms.controls[&node].caption, "Send image");
        if path == "/image-fallback" {
            let rect = image_rect(&app);
            assert!(rect.image.is_none());
            let point = (
                rect.x + rect.width / 2,
                rect.y + rect.height / 2 + app.content_top() - app.scroll,
            );
            click(&mut app, point);
        } else {
            let field = id(&app, "q");
            app.focus_control(field);
            if path == "/image-post" {
                app.key(0x09, false, false, false);
                assert!(app.focus == Focus::Control(node));
                app.key(0x20, false, false, false);
            } else {
                // Enter in the text field activates the default image submitter.
                app.key(0x0d, false, false, false);
            }
        }
        assert_image_submission(&site, &mut app, path == "/image-post", "seed", (0, 0));
    }
}

#[test]
fn image_validation_keeps_the_selected_point_for_label_and_keyboard_activation() {
    let site = Site::new();
    for label in [true, false] {
        let mut app = App::new();
        site.open(&mut app, "/image-required");
        site.take();
        let point = image_view_point(&app, 18, 12);
        click(&mut app, point);
        assert!(!app.is_loading());
        assert!(app.failed, "Required empty field should block submission");
        assert!(site.take().is_empty());
        let field = id(&app, "q");
        app.focus_control(field);
        app.paste("filled");
        if label {
            let run = app
                .page
                .runs
                .iter()
                .find(|run| run.text == "Activate")
                .unwrap();
            let point = (run.x + 2, run.y + app.content_top() - app.scroll + 2);
            assert!(app
                .image_point_at(id(&app, "picture"), point.0, point.1)
                .is_none());
            click(&mut app, point);
        } else {
            app.key(0x0d, false, false, false);
        }
        assert_image_submission(&site, &mut app, false, "filled", (18, 12));
    }
}

#[test]
fn image_click_cancellation_precedes_point_selection_but_submit_cancellation_preserves_it() {
    let site = Site::new();
    for (path, title, retained) in [
        ("/image-cancel-click", "Click canceled", (0, 0)),
        ("/image-cancel-submit", "Submit canceled", (18, 12)),
    ] {
        let mut app = App::new();
        site.open(&mut app, path);
        site.take();
        let point = image_view_point(&app, 18, 12);
        click(&mut app, point);
        assert_eq!(app.document.title, title);
        assert!(!app.is_loading());
        assert!(site.take().is_empty());
        app.key(0x0d, false, false, false);
        assert_image_submission(&site, &mut app, false, "seed", retained);
    }
}

#[test]
fn image_disabled_controls_and_release_outside_never_submit() {
    let site = Site::new();
    for path in ["/image-get", "/image-disabled", "/image-disable-click"] {
        let mut app = App::new();
        site.open(&mut app, path);
        site.take();
        let point = image_view_point(&app, 18, 12);
        if path == "/image-get" {
            app.pointer_down(point.0, point.1, false);
            app.pointer_up(0, app.content_top() + 1);
        } else {
            click(&mut app, point);
            assert!(app.forms.controls[&id(&app, "picture")].disabled);
            app.key(0x0d, false, false, false);
        }
        assert!(!app.is_loading());
        assert!(site.take().is_empty());
        if path == "/image-get" {
            // Releasing elsewhere must not select a point for later keyboard use.
            app.key(0x0d, false, false, false);
            assert_image_submission(&site, &mut app, false, "seed", (0, 0));
        } else if path == "/image-disable-click" {
            assert_eq!(app.document.title, "Disabled by click");
        }
    }
}

#[test]
fn native_post_controls_keep_action_query_and_resources_use_get() {
    let site = Site::new();
    let mut app = App::new();
    site.open(&mut app, "/form");
    site.take();
    let field = id(&app, "q");
    app.focus_control(field);
    app.key(0x41, true, false, false);
    app.paste("private words & é");
    submit(&mut app);
    app.wait_for_load();
    assert!(!app.failed, "{}", app.status);
    assert_eq!(app.document.title, "POST result");
    assert!(app.current_request.is_post());
    assert!(app.location.display().ends_with("/submit?keep=yes"));
    let requests = site.take();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/submit?keep=yes");
    assert_eq!(
        requests[0].body,
        "q=private+words+%26+%C3%A9&token=fixture&action=proceed"
    );
    assert!(requests[0]
        .headers
        .contains(&format!("Origin: {}\r\n", site.base)));
    assert_eq!(requests[1].method, "GET");
    assert_eq!(requests[1].path, "/style.css");
    assert!(requests[1].body.is_empty());
    assert!(!app.events.join("\n").contains("private+words"));
    assert!(app
        .events
        .join("\n")
        .contains("Set-Cookie headers: 2 processed by the bounded session store; 0 declined"));
    assert!(!app.events.join("\n").contains("do-not-record-this"));
    assert!(!requests[0].headers.contains("\r\nCookie:"));
    assert!(requests[1]
        .headers
        .contains("\r\nCookie: tpag=do-not-record-this; ppag=also-private\r\n"));
    assert!(requests.iter().all(|r| !r.headers.contains("\r\nReferer:")));
}

#[test]
fn canceled_script_submit_never_sends_post() {
    let site = Site::new();
    let mut app = App::new();
    site.open(&mut app, "/prevent");
    site.take();
    submit(&mut app);
    assert_eq!(app.document.title, "Stayed here");
    assert!(!app.is_loading());
    assert!(!app.failed, "{}", app.status);
    assert!(site.take().is_empty());
}

#[test]
fn full_reload_post_requires_a_fresh_accepted_prompt_before_sending() {
    let site = Site::new();
    let mut app = loaded_post(&site);
    app.reload_with_mode(crate::http::FetchMode::FullReload);
    assert!(!app.is_loading());
    let (declined, _) = app.take_resubmission_prompt().expect("POST refresh prompt");
    assert!(app.take_resubmission_prompt().is_none());
    assert!(site.take().is_empty());
    app.confirm_resubmission(declined, false);
    assert!(!app.is_loading());
    assert_eq!(app.document.title, "POST result");
    assert!(site.take().is_empty());
    app.reload_with_mode(crate::http::FetchMode::FullReload);
    let (accepted, _) = app.take_resubmission_prompt().unwrap();
    assert_ne!(accepted, declined);
    app.confirm_resubmission(declined, true);
    assert!(!app.is_loading());
    assert!(site.take().is_empty());
    app.confirm_resubmission(accepted, true);
    app.wait_for_load();
    assert_eq!(app.document.title, "POST result");
    let requests = site.take();
    assert_eq!(requests.iter().filter(|r| r.method == "POST").count(), 1);
    assert_eq!(requests.len(), 2);
    for request in requests {
        assert!(request
            .headers
            .contains("Cache-Control: no-cache\r\nPragma: no-cache\r\n"));
    }
    app.confirm_resubmission(accepted, true);
    assert!(site.take().is_empty());
}

#[test]
fn post_history_confirmation_is_invalidated_by_new_navigation() {
    let site = Site::new();
    let mut app = loaded_post(&site);
    site.open(&mut app, "/next");
    site.take();
    app.history_move(-1);
    let (old, _) = app.take_resubmission_prompt().expect("POST history prompt");
    assert!(!app.is_loading());
    assert_eq!(app.document.title, "Next page");
    assert!(site.take().is_empty());
    site.open(&mut app, "/form");
    site.take();
    app.confirm_resubmission(old, true);
    assert!(!app.is_loading());
    assert_eq!(app.document.title, "POST form");
    assert!(site.take().is_empty());
}

#[test]
fn refreshing_pending_post_does_not_silently_resend_or_commit_stale_result() {
    let site = Site::new();
    let mut app = App::new();
    site.open(&mut app, "/slow-form");
    site.take();
    submit(&mut app);
    site.wait_for_posts(1);
    app.reload_with_mode(crate::http::FetchMode::FullReload);
    let (token, _) = app
        .take_resubmission_prompt()
        .expect("Pending POST refresh prompt");
    assert!(!app.is_loading());
    app.confirm_resubmission(token, false);
    assert_eq!(app.address.text(), app.location.display());
    assert_eq!(site.take().iter().filter(|r| r.method == "POST").count(), 1);
    site.open(&mut app, "/next");
    std::thread::sleep(Duration::from_millis(450));
    app.poll_load();
    assert_eq!(app.document.title, "Next page");
    assert!(!site.take().iter().any(|r| r.method == "POST"));
}

#[test]
fn post_redirect_to_get_refreshes_without_resubmission() {
    let site = Site::new();
    let mut app = App::new();
    site.open(&mut app, "/redirect-form");
    site.take();
    submit(&mut app);
    app.wait_for_load();
    assert_eq!(app.document.title, "GET result");
    assert!(!app.current_request.is_post());
    let requests = site.take();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[1].method, "GET");
    app.reload();
    assert!(app.take_resubmission_prompt().is_none());
    app.wait_for_load();
    let requests = site.take();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/get-result");
}

#[test]
fn explicit_navigation_to_post_url_uses_get_but_fragments_keep_the_document() {
    let site = Site::new();
    let mut app = loaded_post(&site);
    app.follow("#section");
    assert!(app.current_request.is_post());
    assert!(site.take().is_empty());
    let url = app.location.url.as_ref().unwrap().display();
    app.open_address(&url);
    app.wait_for_load();
    assert!(!app.current_request.is_post());
    assert_eq!(app.document.title, "Explicit GET result");
    let seen = site.take();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].method, "GET");
    assert!(seen[0].body.is_empty());
    app.reload();
    assert!(app.take_resubmission_prompt().is_none());
    app.wait_for_load();
    site.take();
    site.open(&mut app, "/form");
    submit(&mut app);
    app.wait_for_load();
    assert!(app.current_request.is_post());
    site.take();
    app.follow("/submit?keep=yes");
    app.wait_for_load();
    assert!(!app.current_request.is_post());
    assert_eq!(site.take()[0].method, "GET");
}
