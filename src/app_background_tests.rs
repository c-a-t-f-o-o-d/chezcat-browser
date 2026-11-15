//! Original real-App background image controls. These are not screen/device or
//! live-site tests; requests use owned loopback origins and original PPM pixels.
use super::*;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

struct BackgroundSite {
    address: SocketAddr,
    stopped: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<String>>>,
    gate: Arc<(Mutex<bool>, Condvar)>,
    file_source: Arc<Mutex<Option<String>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

fn background_ppm(color: [u8; 3]) -> Vec<u8> {
    let mut bytes = b"P6\n2 2\n255\n".to_vec();
    for _ in 0..4 {
        bytes.extend_from_slice(&color);
    }
    bytes
}

impl BackgroundSite {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let file_source = Arc::new(Mutex::new(None::<String>));
        let (stop, seen, held, file) = (
            stopped.clone(),
            requests.clone(),
            gate.clone(),
            file_source.clone(),
        );
        let worker = std::thread::spawn(move || {
            let mut children = Vec::new();
            for stream in listener.incoming().take(64) {
                let Ok(mut stream) = stream else {
                    break;
                };
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let (seen, held, file) = (seen.clone(), held.clone(), file.clone());
                children.push(std::thread::spawn(move || {
                    stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                    stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                    let mut request = Vec::new();
                    let mut byte = [0u8; 1];
                    while !request.ends_with(b"\r\n\r\n") && request.len() < 32768 {
                        if stream.read_exact(&mut byte).is_err() { return; }
                        request.push(byte[0]);
                    }
                    if !request.ends_with(b"\r\n\r\n") { return; }
                    let path = String::from_utf8_lossy(&request).split_ascii_whitespace()
                        .nth(1).unwrap_or("").to_owned();
                    seen.lock().unwrap().push(path.clone());
                    if path == "/held.ppm" {
                        let _ = held.1.wait_timeout_while(held.0.lock().unwrap(), Duration::from_secs(8), |open| !*open).unwrap();
                    }
                    let mut status = "200 OK";
                    let mut mime = "text/html";
                    let mut headers = String::new();
                    let body = match path.as_str() {
                        "/Red.PPM?Token=Keep" | "/Styles/Case/Image.PPM?Token=Keep" | "/red.ppm" => {
                            mime = "image/x-portable-pixmap"; background_ppm([255, 0, 0])
                        }
                        "/green.ppm" => { mime = "image/x-portable-pixmap"; background_ppm([0, 255, 0]) }
                        "/blue.ppm" | "/held.ppm" | "/forbidden.ppm" => {
                            mime = "image/x-portable-pixmap"; background_ppm([0, 0, 255])
                        }
                        "/Alias.css" => {
                            status = "302 Found"; headers = "Location: /Styles/Final.css\r\n".into(); Vec::new()
                        }
                        "/Styles/Final.css" => {
                            mime = "text/css";
                            b"#target{width:32px;height:16px;background-image:url('Case/Image.PPM?Token=Keep')}".to_vec()
                        }
                        "/Held.css" => {
                            mime = "text/css";
                            b"#target{width:32px;height:16px;background-image:url('/held.ppm')}".to_vec()
                        }
                        "/Other.css" => {
                            mime = "text/css";
                            b"#target{width:32px;height:16px;background-image:url('/green.ppm')}".to_vec()
                        }
                        "/sheet" => b"<head><link rel=stylesheet href=/Alias.css></head><body><div id=target></div></body>".to_vec(),
                        "/dynamic-sheet" | "/held-sheet" => {
                            let href = if path == "/held-sheet" { "/Held.css" } else { "/Alias.css" };
                            format!("<head></head><body><div id=target></div><script>var sheet=document.createElement('link');sheet.rel='stylesheet';sheet.href='{href}';document.head.appendChild(sheet);</script></body>").into_bytes()
                        }
                        "/fresh" => b"<head><title>Fresh background document</title></head><body><div id=target style='width:32px;height:16px'></div></body>".to_vec(),
                        "/img" => b"<body><div id=target style='width:32px;height:16px;padding:4px;background-image:url(/red.ppm)'><img id=picture src=/green.ppm width=4 height=4></div><script>var loads=0,errors=0;var picture=document.getElementById('picture');picture.onload=function(){loads++;};picture.onerror=function(){errors++;};</script></body>".to_vec(),
                        _ => {
                            let source = match path.as_str() {
                                "/held" => "/held.ppm".into(),
                                "/foreign" => format!("http://localhost:{}/forbidden.ppm", address.port()),
                                "/file" => file.lock().unwrap().clone().expect("Owned local image URL was not provided."),
                                "/csp" => { headers = "Content-Security-Policy: img-src 'none'\r\n".into(); "/forbidden.ppm".into() }
                                "/disabled" => "/forbidden.ppm".into(),
                                _ => "/Red.PPM?Token=Keep".into(),
                            };
                            format!("<head><title>Original background page</title></head><body><div id=target style=\"width:32px;height:16px;background-image:url('{source}')\"></div></body>").into_bytes()
                        }
                    };
                    let head = format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n", body.len());
                    let _ = stream.write_all(head.as_bytes()).and_then(|_| stream.write_all(&body));
                }));
            }
            for child in children {
                child.join().unwrap();
            }
        });
        Self {
            address,
            stopped,
            requests,
            gate,
            file_source,
            worker: Some(worker),
        }
    }
    fn location(&self, path: &str) -> Location {
        navigation::address(&format!("http://{}{path}", self.address), &Location::home()).unwrap()
    }
    fn count(&self, path: &str) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.as_str() == path)
            .count()
    }
    fn release(&self) {
        *self.gate.0.lock().unwrap() = true;
        self.gate.1.notify_all();
    }
}

impl Drop for BackgroundSite {
    fn drop(&mut self) {
        self.release();
        self.stopped.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_secs(1));
        self.worker.take().unwrap().join().unwrap();
    }
}

fn background_open(site: &BackgroundSite, path: &str, scripts: bool, images: bool) -> App {
    let mut app = App::new();
    app.resize(640, 480);
    app.scripts_enabled = scripts;
    app.images_enabled = images;
    app.navigate_location(site.location(path), true);
    app
}
fn background_wait(app: &mut App, condition: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(6);
    while !condition(app) {
        assert!(
            Instant::now() < deadline,
            "Background App timeout: {:?}",
            app.events
        );
        app.poll_load();
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn background_poll_for(app: &mut App, duration: Duration) {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        app.poll_load();
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn background_node(app: &App) -> usize {
    (0..app.document.nodes.len())
        .find(|&node| app.document.attr(node, "id") == Some("target"))
        .unwrap()
}
fn background_eval(app: &mut App, source: &str) {
    let page = app.scripting.as_mut().unwrap();
    let outcome = page.console_eval(&mut app.document, &mut app.forms, source);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    if outcome.layout_changed {
        app.reflow();
    }
    app.poll_load();
}
fn background_pixels(app: &App, color: u32) {
    let node = background_node(app);
    let bitmap = app
        .backgrounds
        .get(&node)
        .expect("No real decoded background bitmap.");
    assert_eq!((bitmap.width, bitmap.height), (2, 2));
    assert_eq!(bitmap.pixels, [color; 4]);
    let rect = app.page.border_rects[node].expect("Background owner has no actual layout box.");
    let x = rect.x + 1;
    let y = rect.y + app.content_top() - app.scroll + 1;
    let canvas = app.draw();
    assert!(x >= 0 && y >= 0 && (x as usize) < canvas.width && (y as usize) < canvas.height);
    assert_eq!(
        canvas.pixels[y as usize * canvas.width + x as usize],
        color & 0xffffff,
        "Decoded image did not reach the actual owner box's paint."
    );
}

#[test]
fn css_background_no_script_inline_and_redirected_sheet_reach_actual_paint_with_original_url_case()
{
    for path in ["/inline", "/sheet"] {
        let site = BackgroundSite::new();
        let mut app = background_open(&site, path, false, true);
        background_wait(&mut app, |app| !app.is_loading());
        assert!(!app.scripts_enabled);
        assert!(app.pending_backgrounds.is_empty());
        background_pixels(&app, 0xffff0000);
        let expected = if path == "/sheet" {
            "/Styles/Case/Image.PPM?Token=Keep"
        } else {
            "/Red.PPM?Token=Keep"
        };
        assert_eq!(site.count(expected), 1);
        assert_eq!(
            site.count("/Case/Image.PPM?Token=Keep"),
            0,
            "Document/request URL was used instead of final CSS URL."
        );
        assert_eq!(
            site.count("/red.ppm?token=keep"),
            0,
            "Case-sensitive URL payload was lowercased."
        );
    }
}

#[test]
fn css_background_dynamic_redirected_stylesheet_uses_final_base_before_real_image_request() {
    let site = BackgroundSite::new();
    let mut app = background_open(&site, "/dynamic-sheet", true, true);
    background_wait(&mut app, |app| !app.is_loading());
    background_pixels(&app, 0xffff0000);
    assert_eq!(site.count("/Alias.css"), 1);
    assert_eq!(site.count("/Styles/Final.css"), 1);
    assert_eq!(site.count("/Styles/Case/Image.PPM?Token=Keep"), 1);
    assert_eq!(site.count("/Case/Image.PPM?Token=Keep"), 0);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0);
}

#[test]
fn css_block_background_and_child_img_keep_independent_current_sources_pixels_and_events() {
    let site = BackgroundSite::new();
    let mut app = background_open(&site, "/img", true, true);
    background_wait(&mut app, |app| !app.is_loading());
    let picture = (0..app.document.nodes.len())
        .find(|&node| app.document.attr(node, "id") == Some("picture"))
        .unwrap();
    background_pixels(&app, 0xffff0000);
    assert_eq!(app.images[&picture].pixels, [0xff00ff00; 4]);
    background_eval(&mut app, "if(loads!==1||errors!==0||picture.currentSrc.indexOf('/green.ppm')<0)throw Error('initial actual image events/source');document.getElementById('target').setAttribute('style','width:32px;height:16px;padding:4px;background-image:url(/blue.ppm)');");
    background_wait(&mut app, |app| !app.is_loading());
    background_pixels(&app, 0xff0000ff);
    assert_eq!(app.images[&picture].pixels, [0xff00ff00; 4]);
    background_eval(&mut app, "if(loads!==1||errors!==0||picture.currentSrc.indexOf('/green.ppm')<0)throw Error('background changed child events/source');picture.src='/red.ppm';");
    background_wait(&mut app, |app| !app.is_loading());
    background_pixels(&app, 0xff0000ff);
    assert_eq!(app.images[&picture].pixels, [0xffff0000; 4]);
    background_eval(&mut app, "if(loads!==2||errors!==0||picture.currentSrc.indexOf('/red.ppm')<0)throw Error('element source event isolation');");
}

#[test]
fn css_background_csp_foreign_origin_and_disabled_images_refuse_without_image_contact_or_retry() {
    for (path, images) in [("/csp", true), ("/foreign", true), ("/disabled", false)] {
        let site = BackgroundSite::new();
        let mut app = background_open(&site, path, false, images);
        background_wait(&mut app, |app| !app.is_loading());
        assert!(
            app.backgrounds.is_empty(),
            "Refused background produced pixels: {path}"
        );
        assert!(app.pending_backgrounds.is_empty());
        assert_eq!(
            site.count("/forbidden.ppm"),
            0,
            "Refused resource contacted the image endpoint: {path}"
        );
        app.reflow();
        let _ = app.draw();
        app.reflow();
        background_poll_for(&mut app, Duration::from_millis(50));
        assert_eq!(
            site.count("/forbidden.ppm"),
            0,
            "Unchanged denied CSS retried: {path}"
        );
    }
}

static BACKGROUND_LOCAL_NEXT: AtomicU64 = AtomicU64::new(0);
struct BackgroundLocalFile {
    directory: std::path::PathBuf,
    file: std::path::PathBuf,
}
impl BackgroundLocalFile {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "chezcat-c1-background-{}-{}",
            std::process::id(),
            BACKGROUND_LOCAL_NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let file = directory.join("owned.ppm");
        std::fs::write(&file, background_ppm([0, 0, 255])).unwrap();
        Self { directory, file }
    }
    fn url(&self) -> String {
        let path = self.file.to_string_lossy().replace('\\', "/");
        let mut encoded = String::new();
        for byte in path.bytes() {
            if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
                encoded.push(char::from(byte));
            } else {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
        format!("file:///{encoded}")
    }
}
impl Drop for BackgroundLocalFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.file);
        let _ = std::fs::remove_dir(&self.directory);
    }
}

#[test]
fn css_background_web_document_cannot_publish_an_existing_valid_local_file() {
    let local = BackgroundLocalFile::new();
    let decoded =
        crate::image::decode_asset_checked(&std::fs::read(&local.file).unwrap(), &|| Ok(()))
            .unwrap();
    assert_eq!(
        decoded.bitmap.pixels, [0xff0000ff; 4],
        "Owned denial fixture itself must be valid."
    );
    let site = BackgroundSite::new();
    *site.file_source.lock().unwrap() = Some(local.url());
    let mut app = background_open(&site, "/file", false, true);
    background_wait(&mut app, |app| !app.is_loading());
    assert!(app.backgrounds.is_empty());
    assert!(app.pending_backgrounds.is_empty());
    assert_eq!(site.requests.lock().unwrap().as_slice(), ["/file"]);
    // This actual-output witness plus the implementation's pre-open origin
    // guard proves refusal; it is not a filesystem syscall trace.
}

#[test]
fn css_background_held_old_stylesheet_image_cannot_replace_new_cascade_pixels() {
    let site = BackgroundSite::new();
    let mut app = background_open(&site, "/held-sheet", true, true);
    background_wait(&mut app, |_| site.count("/held.ppm") == 1);
    background_eval(&mut app, "sheet.href='/Other.css';");
    // Keep the old response held until the real replacement sheet is applied.
    // Releasing immediately after href assignment would not prove retirement.
    background_wait(&mut app, |app| {
        app.styles
            .values()
            .any(|source| source.contains("url('/green.ppm')"))
    });
    site.release();
    let node = background_node(&app);
    let deadline = Instant::now() + Duration::from_secs(6);
    while app.is_loading() {
        assert!(
            Instant::now() < deadline,
            "Replacement background did not settle: {:?}",
            app.events
        );
        app.poll_load();
        assert!(
            app.backgrounds
                .get(&node)
                .is_none_or(|image| image.pixels != [0xff0000ff; 4]),
            "Old response painted after the replacement cascade was already applied."
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    background_pixels(&app, 0xff00ff00);
    background_poll_for(&mut app, Duration::from_millis(50));
    background_pixels(&app, 0xff00ff00);
    assert_eq!(site.count("/held.ppm"), 1);
    assert_eq!(site.count("/green.ppm"), 1);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0);
}

#[test]
fn css_background_stop_and_document_navigation_reject_held_pixels_without_automatic_retry() {
    for navigate in [false, true] {
        let site = BackgroundSite::new();
        let mut app = background_open(&site, "/held", false, true);
        background_wait(&mut app, |_| site.count("/held.ppm") == 1);
        if navigate {
            app.navigate_location(site.location("/fresh"), true);
        } else {
            app.stop_loading();
        }
        site.release();
        background_wait(&mut app, |app| !app.is_loading());
        app.reflow();
        let _ = app.draw();
        app.reflow();
        background_poll_for(&mut app, Duration::from_millis(75));
        assert!(
            app.backgrounds.is_empty(),
            "Retired held image was published after Stop/navigation."
        );
        assert!(app.pending_backgrounds.is_empty());
        assert_eq!(
            site.count("/held.ppm"),
            1,
            "Unchanged retired CSS source was automatically retried."
        );
        if navigate {
            assert_eq!(app.document.title, "Fresh background document");
        }
    }
}

#[test]
fn css_background_adjustable_attempt_limit_refuses_without_retry_and_resets_per_document() {
    let site = BackgroundSite::new();
    let mut app = background_open(&site, "/inline", true, true);
    assert_eq!(app.background_request_limit, 65_536);
    app.background_request_limit = 2;
    background_wait(&mut app, |app| !app.is_loading());
    background_pixels(&app, 0xffff0000);
    assert_eq!(site.count("/Red.PPM?Token=Keep"), 1);
    assert_eq!(app.background_generation, 1);

    background_eval(&mut app, "document.getElementById('target').setAttribute('style','width:32px;height:16px;background-image:url(/green.ppm)');");
    background_wait(&mut app, |app| !app.is_loading());
    background_pixels(&app, 0xff00ff00);
    assert_eq!(site.count("/green.ppm"), 1);
    assert_eq!(app.background_generation, 2);

    background_eval(&mut app, "document.getElementById('target').setAttribute('style','width:32px;height:16px;background-image:url(/blue.ppm)');");
    let node = background_node(&app);
    assert_eq!(
        app.background_generation, 2,
        "Refusal must not consume or wrap an admission ID."
    );
    assert!(
        app.backgrounds.is_empty(),
        "The denied new source retained old pixels."
    );
    assert!(
        app.pending_backgrounds.is_empty(),
        "A refused source became a pending job."
    );
    assert_eq!(
        app.background_requests[&node].key.0, "/blue.ppm",
        "Failed source identity was not retained."
    );
    assert_eq!(
        site.count("/blue.ppm"),
        0,
        "Ceiling refusal happened after image contact."
    );
    assert_eq!(
        app.events
            .iter()
            .filter(|line| line.contains("CSS image request limit reached"))
            .count(),
        1
    );
    app.reflow();
    let _ = app.draw();
    app.reflow();
    background_poll_for(&mut app, Duration::from_millis(75));
    assert_eq!(site.count("/blue.ppm"), 0);
    assert_eq!(app.background_generation, 2);
    assert_eq!(
        app.events
            .iter()
            .filter(|line| line.contains("CSS image request limit reached"))
            .count(),
        1,
        "Reflow retried/logged an unchanged denied source."
    );

    app.navigate_location(site.location("/fresh"), true);
    background_wait(&mut app, |app| !app.is_loading());
    assert_eq!(app.background_generation, 0);
    assert_eq!(
        app.background_request_limit, 2,
        "Navigation changed host policy."
    );
    assert!(app.background_requests.is_empty());
    background_eval(&mut app, "document.getElementById('target').setAttribute('style','width:32px;height:16px;background-image:url(/blue.ppm)');");
    background_wait(&mut app, |app| !app.is_loading());
    background_pixels(&app, 0xff0000ff);
    assert_eq!(
        site.count("/blue.ppm"),
        1,
        "Fresh document did not recover its allowance."
    );
    assert_eq!(app.background_generation, 1);

    // The maximum policy still uses checked ID growth; no huge allocation or
    // request loop is needed to exercise its overflow boundary.
    app.background_request_limit = u64::MAX;
    app.background_generation = u64::MAX;
    background_eval(&mut app, "document.getElementById('target').setAttribute('style','width:32px;height:16px;background-image:url(/red.ppm)');");
    assert_eq!(app.background_generation, u64::MAX);
    assert!(app.backgrounds.is_empty());
    assert!(app.pending_backgrounds.is_empty());
    assert_eq!(site.count("/red.ppm"), 0);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0);
}
