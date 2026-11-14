//! Original loopback images through the real App, resource worker and decoder.
use super::*;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

struct Site {
    address: SocketAddr,
    stopped: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<String>>>,
    gate: Arc<(Mutex<bool>, Condvar)>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Site {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let (stop, seen, held) = (stopped.clone(), requests.clone(), gate.clone());
        let worker = std::thread::spawn(move || {
            let mut workers = Vec::new();
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                if stop.load(Ordering::Relaxed) { break; }
                let (seen, held) = (seen.clone(), held.clone());
                workers.push(std::thread::spawn(move || {
                    stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                    stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                    let mut request = Vec::new();
                    let mut byte = [0u8; 1];
                    while !request.ends_with(b"\r\n\r\n") && request.len() < 32768 {
                        if stream.read_exact(&mut byte).is_err() { return; }
                        request.push(byte[0]);
                    }
                    let request = String::from_utf8_lossy(&request);
                    let path = request.split_ascii_whitespace().nth(1).unwrap_or("").to_owned();
                    seen.lock().unwrap().push(path.clone());
                    let mut status = "200 OK";
                    let mut headers = String::new();
                    let mut mime = "text/html";
                    let body = match path.as_str() {
                        "/one.ppm" | "/assets/one.ppm" | "/two.ppm" | "/held.ppm" | "/forbidden.ppm" => {
                            if path == "/held.ppm" {
                                let _ = held.1.wait_timeout_while(held.0.lock().unwrap(), Duration::from_secs(8), |open| !*open).unwrap();
                            }
                            mime = "image/x-portable-pixmap";
                            let (w, h, color) = if path == "/two.ppm" { (1, 3, [0, 255, 0]) }
                                else if path == "/held.ppm" { (4, 2, [0, 0, 255]) }
                                else { (2, 1, [255, 0, 0]) };
                            let mut bytes = format!("P6\n{w} {h}\n255\n").into_bytes();
                            bytes.extend(std::iter::repeat_n(color, w * h).flatten());
                            bytes
                        }
                        "/motion.gif" => { mime = "image/gif"; include_bytes!("../pages/images/format-animated.gif").to_vec() }
                        "/bad.ppm" => { mime = "image/x-portable-pixmap"; b"not an image".to_vec() }
                        "/missing.ppm" => { status = "404 Not Found"; b"missing".to_vec() }
                        "/redirect.ppm" => {
                            status = "302 Found";
                            headers = format!("Location: http://localhost:{}/forbidden.ppm\r\n", address.port());
                            Vec::new()
                        }
                        "/fresh" => b"<html><head><title>Fresh document</title></head><body><p>Fresh</p></body></html>".to_vec(),
                        _ => {
                            let source = if path == "/parser-held" { "/held.ppm" } else if path == "/start" { "/one.ppm" } else { "" };
                            let base = if path == "/base" { "<base href='/assets/'>" } else { "" };
                            let meta = if path == "/meta" { "<meta http-equiv='Content-Security-Policy' content=\"img-src 'none'\">" } else { "" };
                            if path == "/blocked" { headers = "Content-Security-Policy: script-src 'unsafe-inline'; img-src 'none'\r\n".into(); }
                            let action = match path.as_str() {
                                "/parser-held" => "img.src='/two.ppm';",
                                "/blocked" | "/meta" | "/disabled" => "img.src='/forbidden.ppm';",
                                "/base" => "img.src='one.ppm';",
                                "/redirect" => "img.src='/redirect.ppm';",
                                _ => "",
                            };
                            let denied = matches!(path.as_str(), "/blocked" | "/meta" | "/redirect" | "/disabled");
                            let window_check = if denied { "if(loads||errors!==1||windows!==1||!img.complete||img.naturalWidth)throw Error('resource denial');document.title+='-window-'+windows;" } else { "" };
                            format!(r#"<html><head><title>Waiting</title>{base}</head><body><img id=picture src='{source}'>{meta}<script>
var img=document.getElementById('picture'),loads=0,errors=0,windows=0,order='';
img.onload=function(event){{if(this!==img||event.target!==img||event.type!=='load'||!img.complete||!img.naturalWidth)throw Error('image completion');loads++;order+='L';document.title='loaded-'+loads;}};
img.onerror=function(event){{if(this!==img||event.target!==img||!img.complete||img.naturalWidth!==0)throw Error('broken image');errors++;order+='E';document.title='error-'+errors;}};
window.onload=function(){{windows++;order+='W';{window_check}}};
{action}
</script></body></html>"#).into_bytes()
                        }
                    };
                    let response = format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n", body.len());
                    let _ = stream.write_all(response.as_bytes()).and_then(|_| stream.write_all(&body));
                }));
            }
            for worker in workers { worker.join().unwrap(); }
        });
        Self { address, stopped, requests, gate, worker: Some(worker) }
    }
    fn location(&self, path: &str) -> Location {
        navigation::address(&format!("http://{}{path}", self.address), &Location::home()).unwrap()
    }
    fn release(&self) { *self.gate.0.lock().unwrap() = true; self.gate.1.notify_all(); }
    fn requested(&self, path: &str) -> bool { self.requests.lock().unwrap().iter().any(|seen| seen == path) }
}
impl Drop for Site {
    fn drop(&mut self) {
        self.release();
        self.stopped.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.address);
        self.worker.take().unwrap().join().unwrap();
    }
}
fn wait(app: &mut App, condition: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(6);
    while !condition(app) && Instant::now() < deadline { app.poll_load(); std::thread::sleep(Duration::from_millis(2)); }
    assert!(condition(app), "{}\n{:?}\n{}", app.document.title, app.events, app.scripting.as_ref().map(|page| page.report()).unwrap_or_default());
}
fn open(site: &Site, path: &str) -> App {
    let mut app = App::new();
    app.resize(640, 480);
    app.navigate_location(site.location(path), true);
    app
}
fn eval(app: &mut App, source: &str) {
    let outcome = app.scripting.as_mut().unwrap().console_eval(&mut app.document, &mut app.forms, source);
    assert_eq!(outcome.errors, 0, "{}", app.scripting.as_ref().unwrap().report());
    if outcome.layout_changed { app.reflow(); }
    app.poll_load();
}
fn image(app: &App) -> usize {
    (0..app.document.nodes.len()).find(|&node| app.document.attr(node, "id") == Some("picture")).unwrap()
}

#[test]
fn connected_images_fetch_real_pixels_and_commit_before_events() {
    let site = Site::new();
    let mut app = open(&site, "/start");
    wait(&mut app, |app| !app.is_loading());
    let node = image(&app);
    assert_eq!(app.images[&node].pixels, [0xffff0000; 2]);
    eval(&mut app, "if(loads!==1||windows!==1||order!=='LW'||img.naturalWidth!==2||img.naturalHeight!==1||img.getBoundingClientRect().width!==2)throw Error('initial');var source=img.getAttributeNode('src'),snapshot=img.getBoundingClientRect(),old=img.currentSrc;img.src='/two.ppm';if(img.complete||img.currentSrc!==old||img.naturalWidth!==2||source.value!=='/two.ppm')throw Error('pending');");
    wait(&mut app, |app| !app.is_loading());
    assert_eq!(app.images[&node].pixels, [0xff00ff00; 3]);
    eval(&mut app, "if(loads!==2||errors||windows!==1||!img.complete||img.naturalWidth!==1||img.naturalHeight!==3||img.currentSrc!==location.origin+'/two.ppm'||img.getBoundingClientRect().height!==3||snapshot.width!==2)throw Error('replacement');source.value='/one.ppm';");
    wait(&mut app, |app| !app.is_loading());
    eval(&mut app, "if(loads!==3||errors||windows!==1||img.naturalWidth!==2)throw Error('Attr path');img.setAttribute('src','/two.ppm');");
    wait(&mut app, |app| !app.is_loading());
    eval(&mut app, "if(loads!==4||errors||windows!==1)throw Error('raw attr path');img.removeAttributeNS(null,'src');if(!img.complete||img.currentSrc!==''||img.naturalWidth!==0||source.ownerElement!==null)throw Error('remove');");
    assert!(!app.images.contains_key(&node));
    assert!(site.requested("/one.ppm") && site.requested("/two.ppm"));
    eval(&mut app, "img.onload=null;img.src='/motion.gif';");
    wait(&mut app, |app| !app.is_loading());
    assert!(app.animations.contains_key(&node));
    eval(&mut app, "img.src='/two.ppm';");
    wait(&mut app, |app| !app.is_loading());
    assert!(!app.animations.contains_key(&node));
    assert!(app.next_image_frame().is_none());
    assert_eq!(app.images[&node].pixels, [0xff00ff00; 3]);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0);
}

#[test]
fn connected_images_cancel_superseded_requests_and_discard_parser_results() {
    let site = Site::new();
    let mut app = open(&site, "/start");
    wait(&mut app, |app| !app.is_loading());
    eval(&mut app, "img.src='/held.ppm';");
    wait(&mut app, |_| site.requested("/held.ppm"));
    eval(&mut app, "img.src='/two.ppm';");
    wait(&mut app, |app| !app.is_loading());
    eval(&mut app, "if(loads!==2||errors||windows!==1||img.naturalHeight!==3)throw Error('supersession');");
    site.release();
    for _ in 0..10 { app.poll_load(); }
    assert_eq!(app.images[&image(&app)].pixels, [0xff00ff00; 3]);

    let site = Site::new();
    let mut app = open(&site, "/parser-held");
    wait(&mut app, |app| app.document.title == "loaded-1");
    assert!(site.requested("/held.ppm") && site.requested("/two.ppm"));
    site.release();
    wait(&mut app, |app| !app.is_loading());
    eval(&mut app, "if(loads!==1||errors||windows!==1||img.naturalHeight!==3||img.currentSrc!==location.origin+'/two.ppm')throw Error('stale parser delivery');");
    assert_eq!(app.images[&image(&app)].pixels, [0xff00ff00; 3]);
}

#[test]
fn connected_images_fail_honestly_recover_and_preserve_policy_boundaries() {
    let site = Site::new();
    let mut app = open(&site, "/start");
    wait(&mut app, |app| !app.is_loading());
    for (index, source) in ["/missing.ppm", "/bad.ppm"].iter().enumerate() {
        eval(&mut app, &format!("img.src='{source}';"));
        wait(&mut app, |app| !app.is_loading());
        assert!(!app.images.contains_key(&image(&app)));
        eval(&mut app, &format!("if(errors!=={}||loads!==1||windows!==1||!img.complete||img.naturalWidth!==0||img.currentSrc!==location.origin+'{source}')throw Error('real failure');", index + 1));
    }
    eval(&mut app, "var marker={},caught=false;try{img.src={toString:function(){throw marker;}};}catch(e){caught=e===marker;}if(!caught||img.getAttribute('src')!=='/bad.ppm')throw Error('coercion');img.src='/two.ppm';");
    wait(&mut app, |app| !app.is_loading());
    eval(&mut app, "if(loads!==2||errors!==2||windows!==1||img.naturalHeight!==3)throw Error('recovery');");
    eval(&mut app, "img.src='data:image/x-portable-pixmap,P3%0A1%201%0A255%0A12%2034%2056';");
    wait(&mut app, |app| !app.is_loading());
    assert_eq!(app.images[&image(&app)].pixels, [0xff0c2238]);
    eval(&mut app, "if(loads!==3||errors!==2||img.naturalWidth!==1||img.naturalHeight!==1||img.currentSrc!==img.getAttribute('src'))throw Error('embedded image');");
    for path in ["/blocked", "/meta", "/redirect", "/disabled"] {
        let mut app = App::new();
        app.images_enabled = path != "/disabled";
        app.navigate_location(site.location(path), true);
        wait(&mut app, |app| !app.is_loading());
        assert_eq!(app.document.title, "error-1-window-1", "{path}\n{:?}", app.events);
        assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0, "{}", app.scripting.as_ref().unwrap().report());
    }
    assert!(!site.requested("/forbidden.ppm"));
    let mut app = open(&site, "/base");
    wait(&mut app, |app| !app.is_loading());
    assert!(site.requested("/assets/one.ppm"));
    eval(&mut app, "if(img.src!==location.origin+'/assets/one.ppm'||img.currentSrc!==img.src)throw Error('base URL');");
}

#[test]
fn connected_images_keep_detached_identity_and_cancel_on_stop_or_navigation() {
    let site = Site::new();
    let mut app = open(&site, "/start");
    wait(&mut app, |app| !app.is_loading());
    eval(&mut app, "img.src='/held.ppm';document.body.removeChild(img);if(img.isConnected||img.ownerDocument!==document)throw Error('detached owner');");
    wait(&mut app, |_| site.requested("/held.ppm"));
    site.release();
    wait(&mut app, |app| !app.is_loading());
    eval(&mut app, "if(loads!==2||errors||img.naturalWidth!==4||!img.complete||img.isConnected||img.getBoundingClientRect().width!==0)throw Error('detached completion');document.body.appendChild(img);if(img.naturalWidth!==4)throw Error('reattach');");
    assert!(site.requests.lock().unwrap().iter().filter(|path| *path == "/held.ppm").count() == 1);
    for stop in [false, true] {
        let site = Site::new();
        let mut app = open(&site, "/start");
        wait(&mut app, |app| !app.is_loading());
        eval(&mut app, "img.src='/held.ppm';");
        wait(&mut app, |_| site.requested("/held.ppm"));
        if stop { app.stop_loading(); assert!(!app.is_loading()); }
        app.navigate_location(site.location("/fresh"), true);
        site.release();
        wait(&mut app, |app| !app.is_loading());
        assert_eq!(app.document.title, "Fresh document");
        assert!(app.images.is_empty() && app.pending_images.is_empty());
    }
}

#[test]
fn connected_images_start_created_elements_without_fake_detached_loads() {
    let site = Site::new();
    let mut app = open(&site, "/fresh");
    wait(&mut app, |app| !app.is_loading());
    eval(&mut app, "var img=new Image(),loaded=0;img.src='/two.ppm';if(img.complete||img.currentSrc!=='')throw Error('detached boundary');img.onload=function(){loaded++;document.title='created-loaded';};document.body.appendChild(img);");
    wait(&mut app, |app| !app.is_loading());
    eval(&mut app, "if(loaded!==1||!img.complete||img.naturalHeight!==3||!img.isConnected)throw Error('created image');");
    assert!(site.requested("/two.ppm"));
}

#[test]
fn connected_images_stop_flushes_source_removal_and_geometry() {
    let site = Site::new();
    let mut app = open(&site, "/start");
    wait(&mut app, |app| !app.is_loading());
    let node = image(&app);
    let outcome = app.scripting.as_mut().unwrap().console_eval(&mut app.document, &mut app.forms, "img.removeAttribute('src');if(img.naturalWidth!==0||img.currentSrc!==''||!img.complete)throw Error('immediate removal');");
    assert_eq!(outcome.errors, 0);
    assert!(outcome.layout_changed && app.images.contains_key(&node));
    app.reflow(); // A native input job may reflow before the resource queue drains.
    let outcome = app.scripting.as_mut().unwrap().console_eval(&mut app.document, &mut app.forms, "if(img.naturalWidth!==0)throw Error('resurrected geometry');");
    assert_eq!(outcome.errors, 0, "{}", app.scripting.as_ref().unwrap().report());
    app.stop_loading();
    assert!(!app.is_loading() && !app.images.contains_key(&node));
    eval(&mut app, "if(img.naturalWidth!==0||img.naturalHeight!==0||loads!==1||errors)throw Error('stopped removal');");
}

#[test]
fn connected_images_shared_pixels_do_not_consume_the_document_budget_twice() {
    let mut app = App::new();
    for node in 1..=4 {
        app.images.insert(node, Arc::new(Bitmap { width: 2048, height: 2048, pixels: vec![0xff123456; 2048 * 2048] }));
    }
    let mut reused = crate::image::DecodedImage { bitmap: app.images[&1].clone(), animation: None, notice: None };
    app.fit_document_image(5, &mut reused).unwrap();
    assert!(Arc::ptr_eq(&reused.bitmap, &app.images[&1]));
    let mut extra = crate::image::DecodedImage::still(Bitmap { width: 1, height: 1, pixels: vec![0xff123456] });
    assert!(app.fit_document_image(5, &mut extra).is_err());
}

#[test]
fn inner_html_images_use_real_resources_and_detached_old_requests_keep_ownership() {
    let site = Site::new();
    let mut app = open(&site, "/fresh");
    wait(&mut app, |app| !app.is_loading());
    eval(&mut app, "var holder=document.createElement('div');document.body.appendChild(holder);var loads=0;holder.innerHTML='<img id=old src=\"/held.ppm\"><script id=inert src=\"/must-not-fetch.js\">document.title=\"WRONG\";</script>';var old=document.getElementById('old'),inert=document.getElementById('inert');old.onload=function(){if(old.isConnected||old.ownerDocument!==document||old.naturalWidth!==4||!old.complete)throw Error('old resource owner');loads++;};");
    wait(&mut app, |_| site.requested("/held.ppm"));
    eval(&mut app, "var saved=old.getBoundingClientRect();holder.innerHTML='<p id=caption>Replacement image</p><img id=picture src=\"/two.ppm\">';var img=document.getElementById('picture');img.onload=function(){if(!img.complete||img.naturalHeight!==3||img.getBoundingClientRect().height!==3||old.parentNode!==null||old.isConnected)throw Error('fragment image');loads++;document.title='fragment-loaded';};inert.src='/still-must-not-fetch.js';document.body.appendChild(inert);");
    site.release();
    wait(&mut app, |app| !app.is_loading());
    assert_eq!(app.document.title, "fragment-loaded");
    assert!(!site.requested("/must-not-fetch.js") && !site.requested("/still-must-not-fetch.js"));
    assert!(site.requested("/two.ppm"));
    eval(&mut app, "if(loads!==2||old.naturalWidth!==4||img.naturalHeight!==3||document.getElementById('old')!==null||inert.parentNode!==document.body||document.getElementById('caption').textContent!=='Replacement image')throw Error('fragment result');");
    let pixels = app.images[&image(&app)].clone();
    assert_eq!(pixels.pixels, [0xff00ff00; 3]);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0);
}
