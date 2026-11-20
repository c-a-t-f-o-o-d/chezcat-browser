//! Owned App/panel input controls. No OS clipboard or external service is used.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!("chezcat-about-ui-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn app(&self) -> App {
        let mut app = App::new();
        app.attach_profile(Store::open(self.0.join("private-profile.txt")).unwrap());
        let markup = format!("<title>private-about-sentinel</title><style>p{{height:40px}}</style><body><p>private-page-information</p>{}</body>",
            "<p>Original scrollable page</p>".repeat(40));
        let path = self.0.join("original.html");
        std::fs::write(&path, markup).unwrap();
        app.open(&path);
        assert!(!app.failed, "{}", app.status);
        assert!(!app.is_loading());
        assert!(app.max_scroll() > 123);
        app.set_scroll(123);
        app
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

#[derive(Debug, PartialEq, Eq)]
struct PageState {
    raw: String,
    nodes: String,
    location: String,
    history: Vec<(String, i32, u64)>,
    history_index: usize,
    scroll: i32,
    epoch: u64,
}
fn page_state(app: &App) -> PageState {
    PageState { raw:app.raw.clone(), nodes:format!("{:?}", app.document.nodes),
        location:app.location.display(),
        history:app.history.borrow().entries().iter().map(|item| (item.location.display(), item.scroll, item.document)).collect(),
        history_index:app.history_index, scroll:app.scroll, epoch:app.script_epoch }
}
fn contents(app: &App) -> &about::Contents {
    let panel = app.browser_panel.as_ref().expect("About panel is open");
    assert!(matches!(panel.kind, PanelKind::About));
    panel.about.as_ref().expect("About has genuine compiled contents")
}
fn point(app: &App, action: Action) -> (i32, i32) {
    let control = app.browser_controls().into_iter().find(|control| control.action == action)
        .expect("actual native panel control exists");
    assert!(control.enabled);
    assert!(control.rect.w > 0 && control.rect.h > 0);
    (control.rect.x + control.rect.w / 2, control.rect.y + control.rect.h / 2)
}
fn click(app: &mut App, action: Action) -> PlatformAction {
    let (x, y) = point(app, action);
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    app.pointer_up(x, y)
}
fn focus_by_tab(app: &mut App, action: Action) {
    for _ in 0..16 {
        if app.browser_panel.as_ref().unwrap().focus == action { return; }
        assert!(matches!(app.key(0x09, false, false, false), PlatformAction::None));
    }
    panic!("Actual Tab order never reached the requested About control");
}
fn safe_copy(action: PlatformAction) -> String {
    match action {
        PlatformAction::CopyBuildInformation(text) => text,
        _ => panic!("Actual Copy activation did not return the typed build-information action"),
    }
}

#[test]
fn about_keyboard_menu_and_escape_preserve_the_underlying_local_document() {
    let fixture = Fixture::new();
    let mut app = fixture.app();
    let before = page_state(&app);
    let page_pixels = app.draw().pixels;
    assert!(matches!(app.key(0x46, false, false, true), PlatformAction::None)); // Alt+F
    assert!(app.menu);
    assert!(matches!(app.key(0x23, false, false, false), PlatformAction::None)); // End
    assert!(matches!(app.key(0x0d, false, false, false), PlatformAction::None));
    assert!(!app.menu);
    assert!(matches!(contents(&app).section, about::Section::Build));
    assert_eq!(page_state(&app), before);
    assert_eq!(contents(&app).text.as_ref(), crate::build_identity::current().safe_copy());
    let panel_pixels = app.draw();
    assert!(panel_pixels.width > 0 && panel_pixels.height > 0);
    assert_eq!(panel_pixels.pixels.len(), panel_pixels.width * panel_pixels.height);
    assert!(panel_pixels.pixels.iter().any(|pixel| *pixel != panel_pixels.pixels[0]));
    assert_ne!(panel_pixels.pixels, page_pixels, "About must actually paint over the page");
    assert!(matches!(app.key(0x1b, false, false, false), PlatformAction::None));
    assert!(app.browser_panel.is_none());
    assert_eq!(page_state(&app), before);
    assert_eq!(app.draw().pixels, page_pixels);
}

#[test]
fn about_native_sections_retain_the_exact_compiled_offline_legal_text() {
    let fixture = Fixture::new();
    let mut app = fixture.app();
    let before = page_state(&app);
    app.show_about();
    for (section, expected) in [
        (about::Section::License, crate::build_identity::license_text()),
        (about::Section::Scope, include_str!("../LICENSE_SCOPE.md")),
        (about::Section::Notices, crate::build_identity::THIRD_PARTY_NOTICES),
    ] {
        assert!(matches!(click(&mut app, Action::About(section)), PlatformAction::None));
        assert!(contents(&app).section == section);
        assert_eq!(contents(&app).text.as_ref(), expected);
        let document=&contents(&app).document;
        let pre=(0..document.nodes.len()).find(|&node|matches!(&document.nodes[node].kind,crate::html::NodeKind::Element{tag,..} if tag=="pre")).unwrap();
        let rendered_source:String=document.nodes[pre].children.iter().filter_map(|&node|match &document.nodes[node].kind {crate::html::NodeKind::Text(value)=>Some(value.as_str()),_=>None}).collect();
        assert_eq!(rendered_source,expected,"The actual read-only layout document must retain the complete exact text");
        assert!(!expected.is_empty());
        if section==about::Section::License {
            assert!(matches!(app.key(0x23,false,false,false),PlatformAction::None));
            let maximum=(app.browser_content_height()-app.browser_body().h).max(0);
            assert!(maximum>0);assert_eq!(app.browser_panel.as_ref().unwrap().scroll,maximum);
            app.resize(640,480);
            assert_eq!(contents(&app).text.as_ref(),expected);
            assert!(matches!(app.key(0x24,false,false,false),PlatformAction::None));
            assert_eq!(app.browser_panel.as_ref().unwrap().scroll,0);
        }
        let pixels = app.draw();
        assert!(!pixels.pixels.is_empty());
        assert!(pixels.pixels.iter().any(|pixel| *pixel != pixels.pixels[0]));
        assert_eq!(page_state(&app), before);
    }
    assert!(matches!(click(&mut app, Action::Close), PlatformAction::None));
    assert!(app.browser_panel.is_none());
    assert_eq!(page_state(&app), before);
}

#[test]
fn about_real_copy_button_and_keyboard_return_only_allowlisted_build_information() {
    let fixture = Fixture::new();
    let mut app = fixture.app();
    let before = page_state(&app);
    app.show_about();
    let expected = crate::build_identity::current().safe_copy();
    let pointer = safe_copy(click(&mut app, Action::AboutCopy));
    assert_eq!(pointer, expected);
    focus_by_tab(&mut app, Action::AboutCopy);
    let keyboard = safe_copy(app.key(0x0d, false, false, false));
    assert_eq!(keyboard, pointer);
    for forbidden in [fixture.0.to_string_lossy().as_ref(), "private-about-sentinel",
        "private-page-information", "private-profile.txt", "original.html", "http://", "https://"] {
        assert!(!keyboard.contains(forbidden), "Page/profile information entered safe build copy");
    }
    assert!(matches!(contents(&app).section, about::Section::Build));
    assert_eq!(page_state(&app), before);
    // Deliberately inspect the action only: no system clipboard backend is invoked.
}

#[test]
fn about_copy_requires_matching_release_and_closed_controls_cannot_copy() {
    let fixture = Fixture::new();
    let mut app = fixture.app();
    let before = page_state(&app);
    app.show_about();
    let (x, y) = point(&app, Action::AboutCopy);
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(4, app.content_bottom() - 1), PlatformAction::None));
    assert!(matches!(contents(&app).section, about::Section::Build));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None), "Unarmed release must not copy");
    assert!(matches!(app.key(0x1b, false, false, false), PlatformAction::None));
    assert!(app.browser_panel.is_none());
    assert!(app.browser_about_activation(Action::AboutCopy).is_none());
    assert_eq!(page_state(&app), before);
    app.show_about();
    assert_eq!(safe_copy(click(&mut app, Action::AboutCopy)), crate::build_identity::current().safe_copy(),
        "a fresh valid press/release remains usable after cancellation");
}

// Original owned loopback fixture. Both native stylesheet and image responses
// remain held while the real About menu/Escape paths run; no public network,
// device, clipboard, page evaluation or altered production worker is involved.
use std::io::{Read,Write};
use std::net::{SocketAddr,TcpListener,TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc,Condvar,Mutex};
use std::time::{Duration,Instant};

struct AboutPendingSite {
    address:SocketAddr,
    stop:Arc<AtomicBool>,
    seen:Arc<Mutex<Vec<String>>>,
    gate:Arc<(Mutex<bool>,Condvar)>,
    thread:Option<std::thread::JoinHandle<()>>,
}
impl AboutPendingSite {
    fn new()->Self {
        let listener=TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();
        let stop=Arc::new(AtomicBool::new(false));let seen=Arc::new(Mutex::new(Vec::new()));let gate=Arc::new((Mutex::new(false),Condvar::new()));
        let (stopped,requests,held)=(stop.clone(),seen.clone(),gate.clone());
        let thread=std::thread::spawn(move||{
            let mut children=Vec::new();
            for stream in listener.incoming().take(8) {
                let Ok(mut stream)=stream else {break;};if stopped.load(Ordering::Relaxed){break;}
                let (requests,held)=(requests.clone(),held.clone());
                children.push(std::thread::spawn(move||{
                    stream.set_nonblocking(false).unwrap();stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                    let mut request=Vec::new();let mut byte=[0u8;1];
                    while !request.ends_with(b"\r\n\r\n")&&request.len()<16384 {if stream.read_exact(&mut byte).is_err(){return;}request.push(byte[0]);}
                    if !request.ends_with(b"\r\n\r\n"){return;}
                    let path=String::from_utf8_lossy(&request).split_ascii_whitespace().nth(1).unwrap_or("").to_owned();requests.lock().unwrap().push(path.clone());
                    if matches!(path.as_str(),"/held.css"|"/held.ppm") {
                        let _=held.1.wait_timeout_while(held.0.lock().unwrap(),Duration::from_secs(8),|open|!*open).unwrap();
                    }
                    let (mime,body)=match path.as_str() {
                        "/page"=>("text/html",br#"<html><head><title>Pending About resources</title><style>html,body{margin:0}#target{width:41px;height:29px;background:#111111}</style></head><body><div id=target>Target</div><img id=picture src=/held.ppm width=8 height=8><script>
var aboutSheetLoads=0;var sheet=document.createElement('link');sheet.rel='stylesheet';sheet.href='/held.css';sheet.onload=function(){aboutSheetLoads++;if(aboutSheetLoads!==1||document.getElementById('target').clientWidth!==137)throw 'pending About stylesheet ownership';document.title='Pending About resources applied';};sheet.onerror=function(){throw 'pending About stylesheet canceled';};document.head.appendChild(sheet);
</script></body></html>"#.to_vec()),
                        "/held.css"=>("text/css",b"#target{width:137px;height:29px;background:#714dce}".to_vec()),
                        "/held.ppm"=>{let mut body=b"P6\n2 2\n255\n".to_vec();for _ in 0..4{body.extend_from_slice(&[0x1a,0xb2,0xc3]);}("image/x-portable-pixmap",body)},
                        _=>return,
                    };
                    let header=format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());let _=stream.write_all(header.as_bytes());let _=stream.write_all(&body);
                }));
            }
            for child in children {child.join().unwrap();}
        });
        Self{address,stop,seen,gate,thread:Some(thread)}
    }
    fn release(&self){*self.gate.0.lock().unwrap()=true;self.gate.1.notify_all();}
    fn wait(&self,app:&mut App,condition:impl Fn(&App)->bool){
        let deadline=Instant::now()+Duration::from_secs(7);
        while !condition(app) {assert!(Instant::now()<deadline,"Pending About fixture timeout: {:?}",app.events);app.poll_load();std::thread::sleep(Duration::from_millis(2));}
    }
}
impl Drop for AboutPendingSite {fn drop(&mut self){self.release();self.stop.store(true,Ordering::Relaxed);let _=TcpStream::connect_timeout(&self.address,Duration::from_secs(1));self.thread.take().unwrap().join().unwrap();}}

#[test]
fn about_escape_preserves_real_pending_stylesheet_and_image_workers_until_owned_completion(){
    let site=AboutPendingSite::new();let mut app=App::new();app.resize(640,480);
    let location=navigation::address(&format!("http://{}/page",site.address),&Location::home()).unwrap();app.navigate_location(location,true);
    site.wait(&mut app,|app|app.pending_stylesheets==1&&app.document.title=="Pending About resources"&&{
        let paths=site.seen.lock().unwrap();paths.iter().any(|p|p=="/held.css")&&paths.iter().any(|p|p=="/held.ppm")});
    assert!(app.is_loading());
    let target=(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some("target")).unwrap();
    let picture=(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some("picture")).unwrap();
    assert_eq!(app.page.client_boxes[target].unwrap().width,41);assert!(!app.images.contains_key(&picture));
    let nodes=app.document.nodes.as_ptr();let raw=app.raw.clone();let epoch=app.script_epoch;
    let request=app.pending.as_ref().map(|p|(p.id,p.committed));let worker=app.worker.as_ref().map(|w|w as *const _);
    let stylesheet_worker=app.dynamic_stylesheets.as_ref().map(|w|w as *const _);assert!(worker.is_some());assert!(stylesheet_worker.is_some());
    let history_index=app.history_index;let history_len=app.history.len();
    assert!(matches!(app.key(0x46,false,false,true),PlatformAction::None));
    assert!(matches!(app.key(0x23,false,false,false),PlatformAction::None));
    assert!(matches!(app.key(0x0d,false,false,false),PlatformAction::None));
    assert!(matches!(app.browser_panel.as_ref().unwrap().kind,PanelKind::About));
    assert!(app.is_loading());assert_eq!(app.pending_stylesheets,1);
    assert!(matches!(app.key(0x1b,false,false,false),PlatformAction::None));assert!(app.browser_panel.is_none());
    assert!(app.is_loading(),"Escape in About must not run the page Stop command");assert_eq!(app.pending_stylesheets,1);
    assert_eq!(app.pending.as_ref().map(|p|(p.id,p.committed)),request);
    assert_eq!(app.worker.as_ref().map(|w|w as *const _),worker);assert_eq!(app.dynamic_stylesheets.as_ref().map(|w|w as *const _),stylesheet_worker);
    assert_eq!(app.document.nodes.as_ptr(),nodes);assert_eq!(app.script_epoch,epoch);assert_eq!(app.raw,raw);
    site.release();site.wait(&mut app,|app|!app.is_loading());
    assert_eq!(app.document.title,"Pending About resources applied");assert_eq!(app.scripting.as_ref().unwrap().error_count(),0);assert_eq!(app.pending_stylesheets,0);
    assert_eq!(app.document.nodes.as_ptr(),nodes);assert_eq!(app.script_epoch,epoch);assert_eq!(app.raw,raw);assert_eq!(app.history_index,history_index);assert_eq!(app.history.len(),history_len);
    assert_eq!(app.page.client_boxes[target].unwrap().width,137);
    let rect=app.page.border_rects[target].unwrap();let canvas=app.draw();let x=(rect.x+rect.width-2) as usize;let y=(app.content_top()+rect.y+rect.height-2-app.scroll) as usize;
    assert_eq!(canvas.pixels[y*canvas.width+x],0x714dce);
    let image=app.images.get(&picture).expect("The same pending image must still decode and publish");assert_eq!((image.width,image.height),(2,2));assert_eq!(image.pixels,[0xff1ab2c3;4]);
    let requests=site.seen.lock().unwrap();assert_eq!(requests.len(),3);for path in ["/page","/held.css","/held.ppm"] {assert_eq!(requests.iter().filter(|p|p.as_str()==path).count(),1,"{path}");}
}
