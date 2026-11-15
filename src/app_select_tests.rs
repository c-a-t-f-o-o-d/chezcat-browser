//! Native single-select interaction and actual loopback form requests.
use super::*;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

static SERIAL: AtomicUsize = AtomicUsize::new(0);
struct File(PathBuf);
impl Drop for File {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn opened(source: &str) -> (File, App) {
    let file = File(std::env::temp_dir().join(format!(
        "own-select-{}-{}.html",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )));
    std::fs::write(&file.0, source).unwrap();
    let mut app = App::new();
    app.open(&file.0);
    assert!(!app.failed, "{}", app.status);
    (file, app)
}
fn id(app: &App, name: &str) -> usize {
    app.document
        .nodes
        .iter()
        .enumerate()
        .find_map(|(node, _)| (app.document.attr(node, "id") == Some(name)).then_some(node))
        .unwrap_or_else(|| panic!("Missing fixture element {name:?}; title={:?}; status={}", app.document.title, app.status))
}
fn click(app: &mut App, x: i32, y: i32) {
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
}
fn open_popup(app: &mut App) {
    let node = id(app, "s");
    let rect = app
        .page
        .controls
        .iter()
        .find(|rect| rect.node == node)
        .unwrap();
    let point = (
        rect.x + 8,
        rect.y + app.content_top() - app.scroll + rect.height / 2,
    );
    click(app, point.0, point.1);
    assert!(app.select_popup.is_some());
}
fn click_row(app: &mut App, row: usize) {
    let popup = app.select_popup.unwrap();
    let bounds = app.select_popup_box().unwrap();
    assert!(row >= popup.first && row < popup.first + bounds.visible);
    click(
        app,
        bounds.x + 10,
        bounds.y + 1 + (row - popup.first) as i32 * bounds.row_height + bounds.row_height / 2,
    );
}
fn events(app: &App) -> String {
    app.document.nodes[id(app, "events")]
        .children
        .iter()
        .filter_map(|&node| match &app.document.nodes[node].kind {
            html::NodeKind::Text(value) => Some(value.as_str()),
            _ => None,
        })
        .collect()
}
const CONTROLS: &str = "<title>Select test</title><style>body{margin:20px}p{margin:0}</style><form id=f><select id=s name=choice><option value=a selected>Alpha<optgroup label='Disabled group' disabled><option value=x>Cannot choose</optgroup><optgroup label=Choices><option value=b>Beta<option value=c>Gamma</optgroup></select><button id=reset type=reset>Reset</button></form><p id=events></p><script>var s=document.getElementById('s');s.oninput=function(){var p=document.getElementById('events');p.textContent=p.textContent+'I';};s.onchange=function(){var p=document.getElementById('events');p.textContent=p.textContent+'C';};</script>";

#[test]
fn select_mouse_chooses_only_enabled_rows_and_emits_input_then_change() {
    let (_file, mut app) = opened(CONTROLS);
    let node = id(&app, "s");
    open_popup(&mut app);
    click_row(&mut app, 2); // Disabled group's option.
    assert!(app.select_popup.is_some());
    assert_eq!(app.forms.controls[&node].value.text(), "a");
    assert_eq!(events(&app), "");
    let bounds = app.select_popup_box().unwrap();
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[bounds.y as usize * canvas.width + bounds.x as usize],
        0x526d80
    );
    assert_eq!(canvas.pixels[0], ui::NAVY);
    if let Ok(directory) = std::env::var("CHEZCAT_SELECT_QA") {
        let path = Path::new(&directory);
        std::fs::create_dir_all(path).unwrap();
        let mut output = std::fs::File::create(path.join("select-popup.ppm")).unwrap();
        write!(output, "P6\n{} {}\n255\n", canvas.width, canvas.height).unwrap();
        for pixel in &canvas.pixels {
            output
                .write_all(&[(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8])
                .unwrap();
        }
    }
    click_row(&mut app, 4);
    assert!(app.select_popup.is_none());
    assert_eq!(app.forms.controls[&node].value.text(), "b");
    assert_eq!(app.forms.controls[&node].caption, "Beta");
    assert_eq!(events(&app), "IC");
    open_popup(&mut app);
    click_row(&mut app, 4);
    assert_eq!(
        events(&app),
        "IC",
        "unchanged selection must not fire change"
    );
}

#[test]
fn select_keyboard_preview_cancel_commit_and_reset_are_separate() {
    let (_file, mut app) = opened(CONTROLS);
    let node = id(&app, "s");
    app.focus_control(node);
    app.key(0x20, false, false, false);
    app.key(0x28, false, false, false);
    assert_eq!(app.select_popup.unwrap().highlighted, Some(4));
    assert_eq!(app.forms.controls[&node].value.text(), "a");
    app.key(0x1b, false, false, false);
    assert!(app.select_popup.is_none());
    assert_eq!(events(&app), "");
    app.key(0x0d, false, false, false);
    app.key(0x28, false, false, false);
    app.key(0x0d, false, false, false);
    assert_eq!(app.forms.controls[&node].value.text(), "b");
    assert_eq!(events(&app), "IC");
    app.key(0x28, false, false, false);
    assert_eq!(app.forms.controls[&node].value.text(), "c");
    assert_eq!(events(&app), "ICIC");
    app.key(0x24, false, false, false);
    assert_eq!(app.forms.controls[&node].value.text(), "a");
    app.key(0x23, false, false, false);
    assert_eq!(app.forms.controls[&node].value.text(), "c");
    let before = events(&app);
    app.activate_control(id(&app, "reset"));
    assert_eq!(app.forms.controls[&node].value.text(), "a");
    assert_eq!(events(&app), before);
}

#[test]
fn select_long_popup_stays_inside_viewport_and_scrolls_without_page_movement() {
    let source = format!("<body style='height:1000px'><select id=s style='position:fixed;bottom:10px;left:15px;width:180px'>{}</select></body>", (0..40).map(|i| format!("<option value={i}>Option {i}")).collect::<String>());
    let (_file, mut app) = opened(&source);
    app.resize(480, 420);
    app.set_zoom(1.5);
    open_popup(&mut app);
    let bounds = app.select_popup_box().unwrap();
    assert!(bounds.y >= app.content_top());
    assert!(bounds.y + bounds.height <= app.height - FOOT_HEIGHT);
    app.key(0x23, false, false, false);
    assert_eq!(app.select_popup.unwrap().highlighted, Some(39));
    let first = app.select_popup.unwrap().first;
    assert!(first > 0);
    let scroll = app.scroll;
    app.wheel(120, false);
    assert!(app.select_popup.unwrap().first < first);
    assert_eq!(app.scroll, scroll);
    app.key(0x1b, false, false, false);
    assert_eq!(app.forms.controls[&id(&app, "s")].value.text(), "0");
    open_popup(&mut app);
    app.resize(640, 480);
    assert!(app.select_popup.is_none());
    open_popup(&mut app);
    app.key(0x4c, true, false, false);
    assert!(app.select_popup.is_none());
    assert!(app.focus == Focus::Address);
}

#[test]
fn select_click_cancellation_and_outside_dismissal_do_not_change_value() {
    let source = CONTROLS.replace(
        "</script>",
        "s.onclick=function(event){event.preventDefault();};</script>",
    );
    let (_file, mut app) = opened(&source);
    let node = id(&app, "s");
    app.focus_control(node);
    app.key(0x20, false, false, false);
    assert!(app.select_popup.is_none());
    assert_eq!(events(&app), "");
    let (_file, mut app) = opened(CONTROLS);
    open_popup(&mut app);
    click(&mut app, 450, 450);
    assert!(app.select_popup.is_none());
    assert_eq!(app.forms.controls[&id(&app, "s")].value.text(), "a");
    assert_eq!(events(&app), "");
}

struct Site {
    base: String,
    requests: Arc<Mutex<Vec<(String, String, String)>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Site {
    fn new(method: &'static str) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let ending = stop.clone();
        let thread = std::thread::spawn(move || {
            while !ending.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // Accepted Winsock sockets inherit the polling listener mode.
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut bytes = Vec::new();
                        let mut byte = [0];
                        while !bytes.ends_with(b"\r\n\r\n") && bytes.len() < 32 * 1024 {
                            if stream.read(&mut byte).unwrap_or(0) == 0 {
                                break;
                            }
                            bytes.push(byte[0]);
                        }
                        // Cancellation/EOF is not a request for an empty path.
                        if !bytes.ends_with(b"\r\n\r\n") {
                            continue;
                        }
                        let headers = String::from_utf8(bytes).unwrap();
                        let request_method =
                            headers.split_whitespace().next().unwrap_or("").to_owned();
                        let path = headers.split_whitespace().nth(1).unwrap_or("").to_owned();
                        let length = headers
                            .lines()
                            .find_map(|line| line.strip_prefix("Content-Length: "))
                            .map(|value| value.parse::<usize>().unwrap())
                            .unwrap_or(0);
                        assert!(length <= crate::http::MAX_FORM_BYTES);
                        let mut body = vec![0; length];
                        stream.read_exact(&mut body).unwrap();
                        seen.lock().unwrap().push((
                            request_method,
                            path.clone(),
                            String::from_utf8(body).unwrap(),
                        ));
                        let body = if path == "/form" {
                            format!("<title>Select form</title><form method={method} action='/result?old=1'><input type=hidden name=token value=fixture><select id=s name=choice><option value=a>Alpha<option value='café + &amp;'>Beta</select><button id=send name=action value=send>Send</button></form>")
                        } else {
                            "<title>Received select</title>Received".into()
                        };
                        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                        let _ = stream.write_all(response.as_bytes());
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("Loopback select fixture: {error}"),
                }
            }
        });
        Self {
            base,
            requests,
            stop,
            thread: Some(thread),
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

#[test]
fn select_actual_get_and_post_requests_contain_the_user_choice() {
    for method in ["get", "post"] {
        let site = Site::new(method);
        let mut app = App::new();
        app.open_address(&format!("{}/form", site.base));
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
        open_popup(&mut app);
        click_row(&mut app, 1);
        assert_eq!(app.forms.controls[&id(&app, "s")].caption, "Beta");
        app.activate_control(id(&app, "send"));
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
        assert_eq!(app.document.title, "Received select");
        let requests = site.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        let (actual_method, path, body) = &requests[1];
        let encoded = "token=fixture&choice=caf%C3%A9+%2B+%26&action=send";
        if method == "get" {
            assert_eq!(actual_method, "GET");
            assert_eq!(path, &format!("/result?{encoded}"));
            assert!(body.is_empty());
        } else {
            assert_eq!(actual_method, "POST");
            assert_eq!(path, "/result?old=1");
            assert_eq!(body, encoded);
        }
    }
}

#[test]
fn select_fixture_waits_for_fragmented_requests_and_ignores_cancelled_connections() {
    let site = Site::new("get");
    let address = site.base.strip_prefix("http://").unwrap();
    // Empty accepts occur when the worker is cancelled before sending HTTP.
    drop(std::net::TcpStream::connect(address).unwrap());
    let mut client = std::net::TcpStream::connect(address).unwrap();
    client.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    client.write_all(b"GET /fo").unwrap();
    // Let the fixture accept the socket before the remaining request arrives.
    std::thread::sleep(Duration::from_millis(30));
    client.write_all(b"rm HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    assert!(response.contains("<select id=s"), "{response}");
    assert_eq!(site.requests.lock().unwrap().as_slice(),
        &[("GET".to_owned(), "/form".to_owned(), String::new())]);
}
