//! Document byte decoding through actual App navigation, native layout and input.
//! These original loopback/local fixtures do not exercise an external browser.
use crate::app::{App, PlatformAction};
use crate::html::NodeKind;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Response {
    path: &'static str,
    content_type: &'static str,
    headers: &'static str,
    body: Vec<u8>,
}

struct Site {
    address: SocketAddr,
    stopped: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<String>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Site {
    fn new(responses: Vec<Response>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (stop, seen) = (stopped.clone(), requests.clone());
        let worker = std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                if stop.load(Ordering::Acquire) {
                    break;
                }
                stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut request = Vec::new();
                let mut byte = [0u8; 1];
                while !request.ends_with(b"\r\n\r\n") && request.len() < 32768 {
                    if stream.read_exact(&mut byte).is_err() {
                        break;
                    }
                    request.push(byte[0]);
                }
                if !request.ends_with(b"\r\n\r\n") {
                    continue;
                }
                let path = String::from_utf8_lossy(&request)
                    .split_ascii_whitespace().nth(1).unwrap_or("").to_owned();
                seen.lock().unwrap().push(path.clone());
                let response = responses.iter().find(|response| response.path == path);
                let (status, content_type, headers, body) = match response {
                    Some(response) => ("200 OK", response.content_type, response.headers, response.body.as_slice()),
                    None => ("404 Not Found", "text/plain", "", &b"Unexpected resource"[..]),
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                if stream.write_all(head.as_bytes()).is_ok() {
                    let _ = stream.write_all(body);
                }
            }
        });
        Self { address, stopped, requests, worker: Some(worker) }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for Site {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        let _ = TcpStream::connect(self.address);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}

struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-document-encoding-app-{}-{}", std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn wait(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while app.is_loading() && Instant::now() < deadline {
        app.poll_load();
        if app.is_loading() {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    assert!(!app.is_loading(), "{}", app.activity_report());
}

fn app() -> App {
    let mut app = App::new();
    app.resize(480, 320);
    app.set_javascript_enabled(true);
    app.set_images_enabled(true);
    app
}

fn open(site: &Site, path: &str) -> App {
    let mut app = app();
    app.open_address(&site.url(path));
    wait(&mut app);
    assert!(!app.failed, "{}", app.activity_report());
    app
}

fn document(meta: &str, text: &str, script: bool) -> String {
    r##"<!doctype html><html><head><title>Policy guarded</title>_META_<style>
html,body{margin:0;background:#ffffff}#field{position:absolute;left:10px;top:10px;width:160px;height:30px}#probe{position:absolute;left:10px;top:65px;display:block;width:200px;height:40px;background:#123456;color:white}#go{position:absolute;left:250px;top:10px;display:block;width:100px;height:30px;background:#2040e0}#done{position:absolute;left:250px;top:65px;width:100px;height:30px}
</style></head><body><input id=field value=seed><div id=probe>_TEXT_</div><a id=go href=#done>Real link</a><div id=done>Destination</div>_SCRIPT_</body></html>"##
        .replace("_META_", meta)
        .replace("_TEXT_", text)
        .replace("_SCRIPT_", if script {
            "<script>document.title=document.getElementById('probe').textContent;</script>"
        } else { "" })
}

fn replace_bytes(source: &[u8], marker: &[u8], bytes: &[u8]) -> Vec<u8> {
    let at = source.windows(marker.len()).position(|part| part == marker).unwrap();
    assert!(!source[at + marker.len()..].windows(marker.len()).any(|part| part == marker));
    let mut result = source[..at].to_vec();
    result.extend_from_slice(bytes);
    result.extend_from_slice(&source[at + marker.len()..]);
    result
}

fn utf16(source: &str, little: bool) -> Vec<u8> {
    let mut bytes = if little { vec![0xff, 0xfe] } else { vec![0xfe, 0xff] };
    for unit in source.encode_utf16() {
        bytes.extend_from_slice(&if little { unit.to_le_bytes() } else { unit.to_be_bytes() });
    }
    bytes
}

fn id(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len())
        .find(|&node| app.document.attr(node, "id") == Some(name)).unwrap()
}

fn native_text(app: &App, root: usize) -> String {
    let mut text = String::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        let node = &app.document.nodes[node];
        if let NodeKind::Text(value) = &node.kind {
            text.push_str(value.scalar().unwrap());
        }
        pending.extend(node.children.iter().rev().copied());
    }
    text
}

fn assert_painted_document(app: &App, expected: &str) {
    let probe = id(app, "probe");
    assert_eq!(native_text(app, probe), expected);
    let rect = app.page.border_rects[probe].unwrap();
    assert_eq!((rect.width, rect.height), (200, 40));
    let canvas = app.draw();
    let x = rect.x + rect.width - 8;
    let y = app.content_top() + rect.y + rect.height - 8 - app.scroll;
    assert_eq!(canvas.pixels[y as usize * canvas.width + x as usize] & 0xffffff, 0x123456);
}

fn click_field(app: &mut App) {
    let field = id(app, "field");
    let rect = app.page.controls.iter().find(|rect| rect.node == field).unwrap();
    let (x, y) = (rect.x + 12, app.content_top() + rect.y + 12 - app.scroll);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
}

fn copy_field(app: &mut App) -> String {
    click_field(app);
    app.key(0x41, true, false, false);
    match app.key(0x43, true, false, false) {
        PlatformAction::Copy(text) => text,
        _ => panic!("Native editor did not produce its selected value"),
    }
}

fn follow_link(app: &mut App) {
    let go = id(app, "go");
    let rect = app.page.border_rects[go].unwrap();
    let (x, document_y) = (rect.x + 12, rect.y + 12);
    assert_eq!(app.page.link_node_at(x, document_y), Some(go));
    let y = app.content_top() + document_y - app.scroll;
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    wait(app);
    assert!(app.activity_report().lines().any(|line|
        line.starts_with("DOCUMENT: ") && line.ends_with("#done")), "{}", app.activity_report());
}

fn exercise_editor_and_link(app: &mut App) {
    click_field(app);
    app.key(0x41, true, false, false);
    app.paste("edited Ω");
    assert_eq!(copy_field(app), "edited Ω");
    follow_link(app);
    assert_eq!(copy_field(app), "edited Ω", "Same-document navigation retained the native editor");
}

#[test]
fn document_encoding_app_aliases_read_the_committed_decoder_and_factory_defaults() {
    let source = r#"<!doctype html><title>waiting</title><body><script>
var names = ['characterSet', 'charset', 'inputEncoding'];
var inert = document.implementation.createHTMLDocument('');
var xml = document.implementation.createDocument(null, 'root');
var frame = document.createElement('iframe');
frame.hidden = true;
document.body.appendChild(frame);
for (var i = 0; i < names.length; i++) {
    var key = names[i];
    var d = Object.getOwnPropertyDescriptor(Document.prototype, key);
    if (!d || !d.enumerable || !d.configurable || d.set !== undefined) throw 'descriptor ' + key;
    if (Object.prototype.hasOwnProperty.call(document, key)) throw 'own ' + key;
    if (document[key] !== 'Shift_JIS' || d.get.call(document) !== 'Shift_JIS') throw 'loaded ' + key;
    if (inert[key] !== 'UTF-8' || xml[key] !== 'UTF-8' || d.get.call(frame.contentDocument) !== 'UTF-8') throw 'factory ' + key;
    var rejected = false;
    try { d.get.call(document.body); } catch (error) { rejected = error instanceof TypeError; }
    if (!rejected) throw 'receiver ' + key;
}
document.title = document.characterSet;
</script></body>"#;
    let site = Site::new(vec![Response {
        path: "/document", content_type: "text/html; charset=ms_kanji",
        headers: "", body: source.as_bytes().to_vec(),
    }]);
    let app = open(&site, "/document");
    assert_eq!(app.document.title, "Shift_JIS", "{}", app.activity_report());
    assert_eq!(site.requests(), ["/document"]);
}

#[test]
fn document_encoding_app_transport_meta_and_utf8_bom_preserve_real_controls() {
    let transport = document("<meta charset=windows-1252>", "_ENCODED_", true);
    let meta = document("<meta http-equiv=content-type content='text/html; charset=Shift_JIS'>", "_ENCODED_", true);
    let bom = document("<meta charset=Shift_JIS>", "あ", true);
    let recovered = document("<meta charset=utf-8>", "Recovered", true);
    let mut bom_bytes = vec![0xef, 0xbb, 0xbf];
    bom_bytes.extend_from_slice(bom.as_bytes());
    let site = Site::new(vec![
        Response { path: "/transport", content_type: "text/html; charset=Shift_JIS", headers: "", body: replace_bytes(transport.as_bytes(), b"_ENCODED_", &[0x82, 0xa0]) },
        Response { path: "/meta", content_type: "text/html; charset=unknown-encoding", headers: "", body: replace_bytes(meta.as_bytes(), b"_ENCODED_", &[0x82, 0xa0]) },
        Response { path: "/bom", content_type: "text/html; charset=windows-1252", headers: "", body: bom_bytes },
        // Encoded bytes fit the network allowance, but decoded UTF-8 does not.
        Response { path: "/expanded", content_type: "text/html; charset=windows-1252", headers: "", body: vec![0x80; 700_000] },
        Response { path: "/recovered", content_type: "text/html; charset=utf-8", headers: "", body: recovered.as_bytes().to_vec() },
    ]);
    for (path, expected) in [
        ("/transport", transport.replace("_ENCODED_", "あ")),
        ("/meta", meta.replace("_ENCODED_", "あ")),
        ("/bom", bom),
    ] {
        let mut app = open(&site, path);
        let encoding = if path == "/bom" { "UTF-8" } else { "Shift_JIS" };
        assert_eq!(app.document.encoding.document_name(), encoding);
        assert_eq!(app.document_source(), expected);
        assert_eq!(app.document.title, "あ", "{}", app.activity_report());
        assert_painted_document(&app, "あ");
        exercise_editor_and_link(&mut app);
        assert_eq!(app.document_source(), expected);
        assert_eq!(site.requests().iter().filter(|seen| seen.as_str() == path).count(), 1);
        if path == "/bom" {
            app.open_address(&site.url("/expanded"));
            wait(&mut app);
            assert!(app.failed, "Expanded source exceeded the decoded document allowance");
            assert!(app.status.contains("Decoded text exceeds the remaining 2097152 byte allowance"), "{}", app.activity_report());
            assert_eq!(app.document_source(), expected, "No truncated source was committed");
            assert_eq!(app.document.encoding.document_name(), encoding);
            assert_eq!(app.document.title, "あ");
            assert_painted_document(&app, "あ");
            assert_eq!(copy_field(&mut app), "edited Ω", "Refused navigation retained the actual editor value");
            follow_link(&mut app);
            assert_eq!(copy_field(&mut app), "edited Ω");
            assert_eq!(app.document_source(), expected);

            app.open_address(&site.url("/recovered"));
            wait(&mut app);
            assert!(!app.failed, "{}", app.activity_report());
            assert_eq!(app.document_source(), recovered);
            assert_eq!(app.document.encoding.document_name(), "UTF-8");
            assert_eq!(app.document.title, "Recovered", "{}", app.activity_report());
            assert_painted_document(&app, "Recovered");
            assert_eq!(copy_field(&mut app), "seed", "The subsequent document committed fresh controls");
            exercise_editor_and_link(&mut app);
        }
    }
    assert_eq!(site.requests(), ["/transport", "/meta", "/bom", "/expanded", "/recovered"]);
}

#[test]
fn document_encoding_app_utf16_network_local_and_plain_keep_source_and_paint() {
    let source = document("<meta charset=windows-1252>", "ΩЖ中", true);
    let site = Site::new(vec![Response {
        path: "/utf16", content_type: "text/html; charset=ISO-2022-JP", headers: "", body: utf16(&source, true),
    }]);
    let mut network = open(&site, "/utf16");
    assert_eq!(network.document_source(), source);
    assert_eq!(network.document.encoding.document_name(), "UTF-16LE");
    assert_eq!(network.document.title, "ΩЖ中", "{}", network.activity_report());
    assert_painted_document(&network, "ΩЖ中");
    exercise_editor_and_link(&mut network);
    assert_eq!(site.requests(), ["/utf16"]);

    let folder = Folder::new();
    let path = folder.file("utf16.html", &utf16(&source, false));
    let mut local = app();
    local.open(&path);
    wait(&mut local);
    assert!(!local.failed, "{}", local.activity_report());
    assert_eq!(local.document_source(), source);
    assert_eq!(local.document.encoding.document_name(), "UTF-16BE");
    assert_eq!(local.document.title, "ΩЖ中", "{}", local.activity_report());
    assert_painted_document(&local, "ΩЖ中");
    exercise_editor_and_link(&mut local);

    let plain = "\u{feff}Ω <script>document.title='activated'</script>";
    let path = folder.file("utf16.txt", &utf16(plain, true));
    local.open(&path);
    wait(&mut local);
    assert!(!local.failed, "{}", local.activity_report());
    assert_eq!(local.document_source(), plain, "Only the encoding signature was consumed");
    assert_eq!(local.document.content_type, "text/plain");
    assert_eq!(local.document.encoding.document_name(), "UTF-16LE");
    assert_eq!(local.document.title, "Plain text");
    let pre = local.document.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "pre")).unwrap();
    assert_eq!(native_text(&local, pre), plain);
    assert!(!local.document.nodes.iter().any(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "script")));
    assert!(!local.draw().pixels.is_empty());
}

#[test]
fn document_encoding_app_malformed_ascii_delimiters_preserve_csp_guards() {
    let source = document("<meta charset=utf-8>", "_ENCODED_", false)
        .replace("<div id=probe>", "<div id=probe data-broken=\"_BROKEN_\">")
        .replace("</body>", "<script src='/blocked.js'></script><script>document.title='ACTIVATED';</script><img src='/blocked.png'></body>");
    let body = replace_bytes(source.as_bytes(), b"_ENCODED_", &[0x82, 0xa0, 0x82]);
    let body = replace_bytes(&body, b"_BROKEN_", &[0x82]);
    let expected = source.replace("_ENCODED_", "あ�").replace("_BROKEN_", "�");
    let site = Site::new(vec![Response {
        path: "/guarded", content_type: "text/html; charset=Shift_JIS",
        headers: "Content-Security-Policy: default-src 'none'; script-src 'none'; style-src 'unsafe-inline'\r\n",
        body,
    }]);
    let mut app = open(&site, "/guarded");
    assert_eq!(app.document_source(), expected);
    assert_eq!(app.document.title, "Policy guarded");
    assert_eq!(app.document.attr(id(&app, "probe"), "data-broken"), Some("�"));
    assert_painted_document(&app, "あ�");
    assert!(app.document.nodes.iter().any(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "script")));
    assert!(app.document.nodes.iter().any(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "img")));
    exercise_editor_and_link(&mut app);
    assert_eq!(app.document.title, "Policy guarded", "Neither decoded inline nor external script bypassed CSP");
    assert_eq!(site.requests(), ["/guarded"], "CSP stopped resources before a request");
}

#[test]
fn document_encoding_app_plain_text_and_refused_documents_never_activate_markup() {
    let template = "_ENCODED_ <meta charset=Shift_JIS><script src='/blocked.js'>document.title='ACTIVATED'</script><img src='/blocked.png'>";
    let body = replace_bytes(template.as_bytes(), b"_ENCODED_", &[0xcf, 0xf0, 0xe8, 0xe2, 0xe5, 0xf2]);
    let expected = template.replace("_ENCODED_", "Привет");
    let fallback = replace_bytes(template.as_bytes(), b"_ENCODED_", b"caf\xe9");
    let fallback_expected = template.replace("_ENCODED_", "café");
    let xml = "<?xml version='1.0'?><html xmlns='http://www.w3.org/1999/xhtml'><script src='/blocked.js'/><img src='/blocked.png'/></html>";
    let site = Site::new(vec![
        Response { path: "/plain", content_type: "text/plain; charset=windows-1251", headers: "", body },
        Response { path: "/xml", content_type: "application/xhtml+xml; charset=utf-16le", headers: "", body: utf16(xml, true) },
        Response { path: "/recovered", content_type: "text/plain", headers: "", body: fallback },
    ]);
    let mut app = open(&site, "/plain");
    assert_eq!(app.document_source(), expected);
    assert_eq!(app.document.content_type, "text/plain");
    assert_eq!(app.document.title, "Plain text");
    let pre = app.document.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "pre")).unwrap();
    assert_eq!(native_text(&app, pre), expected);
    assert!(!app.document.nodes.iter().any(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if matches!(tag.as_str(), "meta" | "script" | "img"))));
    assert!(!app.draw().pixels.is_empty());

    app.open_address(&site.url("/xml"));
    wait(&mut app);
    assert!(app.failed, "XML loading remains an explicit unsupported entry path");
    assert!(app.status.contains("cannot display document type application/xhtml+xml"), "{}", app.activity_report());
    assert_eq!(app.document_source(), expected, "Rejected XML retained the committed document");
    assert_eq!(app.document.title, "Plain text");
    assert_eq!(native_text(&app, pre), expected);

    app.open_address(&site.url("/recovered"));
    wait(&mut app);
    assert!(!app.failed, "{}", app.activity_report());
    assert_eq!(app.document_source(), fallback_expected, "Plain text did not sniff its literal meta element");
    assert_eq!(app.document.encoding.document_name(), "windows-1252");
    assert_eq!(app.document.title, "Plain text");
    assert_eq!(site.requests(), ["/plain", "/xml", "/recovered"]);
}
