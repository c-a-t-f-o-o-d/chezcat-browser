//! Original bounded real-App admission controls. UNRUN private draft: no native
//! mouse/menu/OS activation acceptance is claimed by these Session tests.
use super::*;
use crate::app::{App, Drag, Focus, View};
use crate::app::anchor::{AnchorDefault, AnchorTrigger};
use crate::editor::Editor;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        let mut bytes = [0u8; 8];
        crate::random::fill(&mut bytes).unwrap();
        let suffix: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        let path = std::env::temp_dir().join(format!("chezcat-link-admission-{suffix}"));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }
    fn source(&self) -> App {
        self.write("site/nested/target.html", "<!doctype html><title>Destination 🦉</title><p>Exact local target</p>");
        self.write("site/nested/base.html", "<title>Resource base</title>");
        self.write("outside.html", "<title>Outside original root</title>");
        let path = self.write("site/source.html", "<!doctype html><base href='nested/base.html'><title>Preserved source</title><style>body{margin:0}input,button,a{display:block;width:180px;height:28px}#space{height:2000px}</style><body><input id=q value=seed><button id=b type=button onclick=\"count++;document.title='Source '+count;\">Count</button><a id=go href=target.html>Accepted destination</a><div id=space>Original source scroll</div><script>var count=0;</script></body>");
        let mut app = App::new();
        app.resize(900, 640);
        app.open(&path);
        assert!(!app.failed, "Actual file fixture failed: {}", app.status);
        assert_eq!(app.document.title, "Preserved source");
        app
    }
}
impl Drop for Files { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
fn adopt(app: App, limits: Limits) -> Session {
    match Session::adopt(Box::new(app), limits) {
        Ok(session) => session,
        Err((_app, error)) => panic!("{error}"),
    }
}
fn first(session: &Session) -> (WindowId, TabId) {
    let window = session.window_ids().next().unwrap();
    (window, session.active(window).unwrap().unwrap())
}
fn node(app: &App, id: &str) -> usize {
    (0..app.document.nodes.len()).find(|&n| app.document.attr(n, "id") == Some(id)).unwrap()
}
fn point(app: &App, id: &str, control: bool) -> (i32, i32) {
    let n = node(app, id);
    let (x, y, width, height) = if control {
        let rect = app.page.controls.iter().find(|rect| rect.node == n).unwrap();
        (rect.x, rect.y, rect.width, rect.height)
    } else {
        let rect = app.page.border_rects[n].unwrap();
        (rect.x, rect.y, rect.width, rect.height)
    };
    (x + width / 2, app.content_top() + y - app.scroll + height / 2)
}
fn target(session: &mut Session, tab: TabId) -> OwnedLinkTarget {
    session.with_tab(tab, |app| {
        let (x, y) = point(app, "go", false);
        assert_eq!(app.page.link_node_at(x, y - app.content_top() + app.scroll), Some(node(app, "go")));
        let context = app.anchor_context(x, y).expect("Genuine rendered anchor did not capture");
        match app.open_anchor_context(context).unwrap() {
            AnchorDefault::Link(target, AnchorTrigger::Context) => target,
            _ => panic!("Accepted real App context did not export its destination"),
        }
    }).unwrap()
}
fn tap(app: &mut App, id: &str) {
    let (x, y) = point(app, id, true);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
}
fn edit(app: &mut App) {
    tap(app, "q");
    app.key(0x41, true, false, false);
    app.paste("preserved café 🦉");
    assert_eq!(app.forms.controls[&node(app, "q")].value.text(), "preserved café 🦉");
    tap(app, "b");
    assert_eq!(app.document.title, "Source 1");
}
fn eval(app: &mut App, text: &str) {
    let page = app.scripting.as_mut().unwrap();
    let outcome = page.console_eval(&mut app.document, &mut app.forms, text);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    if outcome.layout_changed { app.reflow(); }
}
type EditorState = (Vec<char>, usize, usize, usize);
fn editor(value: &Editor) -> EditorState {
    (value.chars.clone(), value.cursor, value.anchor, value.offset)
}
#[derive(Debug, PartialEq)]
struct PageState {
    raw: String, title: String, location: Location, base: Location,
    document: u64, nodes: *const crate::html::Node, vm: Option<*const crate::scripting::Page>,
    scroll: i32, page_scroll: i32, wheel: i32, size: (i32, i32), zoom: u32,
    history: Vec<(Location, i32, crate::http::Request, u64, String)>, index: usize,
    field: EditorState, address: EditorState, find: EditorState,
    focus: (u8, usize), view: u8, surrogate: Option<u16>,
    press: (u8, usize, usize, i32, i32, bool, Option<String>),
    request: crate::http::Request, next_request: u64, committed_request: Option<u64>,
}
struct Saved { state: PageState, request_scope: Rc<()>, geometry_scope: Rc<()> }
fn save(session: &Session, tab: TabId) -> Saved {
    session.observe_tab(tab, |app| {
        let focus = match app.focus { Focus::Page => (0, 0), Focus::Address => (1, 0), Focus::Find => (2, 0), Focus::Control(n) => (3, n) };
        let press = match &app.drag {
            Drag::None => (0, 0, 0, 0, 0, false, None),
            Drag::Text { x, y, href, moved } => (1, 0, 0, *x, *y, *moved, href.clone()),
            Drag::Scrollbar { offset } => (2, 0, 0, *offset, 0, false, None),
            Drag::Field => (3, 0, 0, 0, 0, false, None),
            Drag::Control { node } => (4, *node, 0, 0, 0, false, None),
            Drag::SelectOption { node, index } => (5, *node, *index, 0, 0, false, None),
        };
        Saved {
            state: PageState {
                raw: app.raw.clone(), title: app.document.title.clone(), location: app.location.clone(), base: app.base_location.clone(),
                document: app.script_epoch, nodes: app.document.nodes.as_ptr(), vm: app.scripting.as_ref().map(|page| page as *const _),
                scroll: app.scroll, page_scroll: app.page_scroll, wheel: app.wheel_remainder, size: (app.width, app.height), zoom: app.zoom.to_bits(),
                history: app.history.iter().map(|entry| (entry.location.clone(), entry.scroll, entry.request.clone(), entry.document, format!("{:?}", entry.data))).collect(),
                index: app.history_index, field: editor(&app.forms.controls[&node(app, "q")].value), address: editor(&app.address), find: editor(&app.find),
                focus, view: match app.view { View::Page => 0, View::Source => 1, View::Resources => 2 }, surrogate: app.pending_surrogate, press,
                request: app.current_request.clone(), next_request: app.next_request, committed_request: app.committed_request,
            },
            request_scope: app.platform_request_scope.clone(), geometry_scope: app.anchor_geometry_scope.clone(),
        }
    }).unwrap()
}
fn same(session: &Session, tab: TabId, before: &Saved) {
    let after = save(session, tab);
    assert_eq!(after.state, before.state, "Admission touched the original source owner");
    assert!(Rc::ptr_eq(&after.request_scope, &before.request_scope), "Source request generation changed");
    assert!(Rc::ptr_eq(&after.geometry_scope, &before.geometry_scope), "Source geometry generation changed");
}
#[derive(Debug, PartialEq)]
struct Owners {
    windows: Vec<(WindowId, Vec<TabId>, Option<TabId>, i32, i32)>,
    tabs: Vec<(TabId, WindowId)>, next: (u64, u64), cursor: usize, sweep: usize,
    wakes: Vec<Event>, overflow: bool, closed: bool,
}
fn owners(session: &Session) -> Owners {
    let messages = session.mailbox.0.lock().unwrap();
    Owners {
        windows: session.windows.iter().map(|window| (window.id, window.tabs.clone(), window.active, window.width, window.height)).collect(),
        tabs: session.tabs.iter().map(|tab| (tab.id, tab.window)).collect(),
        next: (session.next_tab, session.next_window), cursor: session.cursor, sweep: session.sweep,
        wakes: messages.queue.iter().copied().collect(), overflow: messages.overflow, closed: messages.closed,
    }
}
fn press_scrollbar(session: &mut Session, tab: TabId) {
    session.with_tab(tab, |app| {
        app.scroll_by(170);
        let (top, thumb, _) = app.scrollbar();
        app.pointer_down(app.width - 5, top + thumb / 2, false);
        assert!(matches!(app.drag, Drag::Scrollbar { .. }), "Fixture must retain an actual live source press");
        app.utf16_input(0xd83e);
        assert_eq!(app.pending_surrogate, Some(0xd83e));
    }).unwrap();
}

#[test]
fn session_link_background_publishes_unselected_owner_without_touching_source_state_or_input_generations() {
    let files = Files::new();
    let mut session = adopt(files.source(), Limits::default());
    let (window, source) = first(&session);
    let accepted = target(&mut session, source);
    session.with_tab(source, edit).unwrap();
    press_scrollbar(&mut session, source);
    let before = save(&session, source);
    let admission = session.open_link(window, source, accepted, LinkDisposition::BackgroundTab).unwrap();
    assert_eq!(admission.disposition, LinkDisposition::BackgroundTab);
    assert_eq!(admission.window, window);
    assert_ne!(admission.tab, source);
    assert_eq!(session.active(window).unwrap(), Some(source));
    assert_eq!(session.window_tabs(window).unwrap(), &[source, admission.tab]);
    same(&session, source, &before);
    session.observe_tab(admission.tab, |app| {
        assert_eq!(app.document.title, "Destination 🦉");
        assert!(app.owned_downloads.is_none() && app.owned_history.is_none());
        assert!(Arc::ptr_eq(&app.cookies, &session.cookies));
        assert!(Arc::ptr_eq(&app.trust, &session.trust));
    }).unwrap();
    // A genuine retained scrollbar gesture still works after admission. The
    // original realm remains live, with its edited field and original counter.
    session.with_tab(source, |app| {
        let previous = app.scroll;
        let (top, thumb, _) = app.scrollbar();
        app.pointer_move(app.width - 5, top + thumb / 2 + 30, true);
        assert!(app.scroll > previous);
        app.pointer_up(app.width - 5, top + thumb / 2 + 30);
        assert!(matches!(app.drag, Drag::None));
        eval(app, "if(count!==1)throw 'lost realm';count++;document.title='resumed '+count;");
        assert_eq!(app.document.title, "resumed 2");
        assert_eq!(app.forms.controls[&node(app, "q")].value.text(), "preserved café 🦉");
    }).unwrap();
}

#[test]
fn session_link_foreground_selects_new_tab_and_new_window_selects_only_its_own_first_tab() {
    let files = Files::new();
    for disposition in [LinkDisposition::ForegroundTab, LinkDisposition::NewWindow] {
        let mut session = adopt(files.source(), Limits::default());
        let (window, source) = first(&session);
        let accepted = target(&mut session, source);
        session.with_tab(source, edit).unwrap();
        press_scrollbar(&mut session, source);
        let before = save(&session, source);
        let admission = session.open_link(window, source, accepted, disposition).unwrap();
        assert_eq!(admission.disposition, disposition);
        assert_eq!(session.active(admission.window).unwrap(), Some(admission.tab));
        session.observe_tab(admission.tab, |app| {
            assert_eq!(app.document.title, "Destination 🦉");
            assert_eq!((app.width, app.height), before.state.size);
        }).unwrap();
        if disposition == LinkDisposition::ForegroundTab {
            assert_eq!(admission.window, window);
            session.observe_tab(source, |app| {
                assert!(matches!(app.drag, Drag::None));
                assert!(app.pending_surrogate.is_none());
                assert!(!Rc::ptr_eq(&app.platform_request_scope, &before.request_scope));
                assert_eq!(app.document.nodes.as_ptr(), before.state.nodes);
                assert_eq!(app.script_epoch, before.state.document);
                assert_eq!(app.forms.controls[&node(app, "q")].value.text(), "preserved café 🦉");
                assert_eq!(app.scroll, before.state.scroll);
            }).unwrap();
        } else {
            assert_ne!(admission.window, window);
            assert_eq!(session.active(window).unwrap(), Some(source));
            assert_eq!(session.window_count(), 2);
            same(&session, source, &before);
            // Session has per-window selection only. Native/logical activation
            // is intentionally not implemented or claimed by this receipt.
        }
    }
}

#[test]
fn session_link_current_uses_existing_owner_even_when_creation_caps_and_ids_are_exhausted() {
    let files = Files::new();
    let mut session = adopt(files.source(), Limits { tabs: 1, windows: 1, wakes: 4, poll_tabs: 1 });
    let (window, source) = first(&session);
    let accepted = target(&mut session, source);
    session.next_tab = u64::MAX;
    session.next_window = u64::MAX;
    let admission = session.open_link(window, source, accepted, LinkDisposition::Current).unwrap();
    assert_eq!(admission, LinkAdmission { window, tab: source, disposition: LinkDisposition::Current });
    assert_eq!((session.tab_count(), session.window_count()), (1, 1));
    assert_eq!((session.next_tab, session.next_window), (u64::MAX, u64::MAX));
    session.observe_tab(source, |app| assert_eq!(app.document.title, "Destination 🦉")).unwrap();
}

#[test]
fn session_link_refuses_wrong_inactive_closed_and_foreign_full_source_ids_without_owner_changes() {
    let files = Files::new();
    let mut session = adopt(files.source(), Limits::default());
    let (window, source) = first(&session);
    let (other, _) = session.new_window(480, 320).unwrap();
    let accepted = target(&mut session, source);
    let before = owners(&session);
    let page = save(&session, source);
    assert!(session.open_link(other, source, accepted, LinkDisposition::BackgroundTab).is_err());
    assert_eq!(owners(&session), before);
    same(&session, source, &page);

    let accepted = target(&mut session, source);
    let replacement = session.new_tab(window).unwrap();
    let before = owners(&session);
    let page = save(&session, source);
    assert!(session.open_link(window, source, accepted, LinkDisposition::ForegroundTab).is_err());
    assert_eq!(owners(&session), before);
    same(&session, source, &page);

    session.switch_tab(window, source).unwrap();
    let accepted = target(&mut session, source);
    session.close_tab(source).unwrap();
    let before = owners(&session);
    assert!(session.open_link(window, source, accepted, LinkDisposition::NewWindow).is_err());
    assert_eq!(owners(&session), before);
    assert_eq!(session.active(window).unwrap(), Some(replacement));

    let mut local = adopt(files.source(), Limits::default());
    let mut foreign = adopt(files.source(), Limits::default());
    let (local_window, local_tab) = first(&local);
    let (foreign_window, foreign_tab) = first(&foreign);
    assert_eq!(local_window.value(), foreign_window.value());
    assert_eq!(local_tab.value(), foreign_tab.value());
    assert_ne!(local_window, foreign_window);
    assert_ne!(local_tab, foreign_tab);
    for (wrong_window, wrong_tab) in [(foreign_window, local_tab), (local_window, foreign_tab), (foreign_window, foreign_tab)] {
        let accepted = target(&mut local, local_tab);
        let before = owners(&local);
        let page = save(&local, local_tab);
        assert!(local.open_link(wrong_window, wrong_tab, accepted, LinkDisposition::Current).is_err());
        assert_eq!(owners(&local), before);
        same(&local, local_tab, &page);
    }
    let accepted = target(&mut foreign, foreign_tab);
    foreign.close_window(foreign_window).unwrap();
    let before = owners(&foreign);
    assert!(foreign.open_link(foreign_window, foreign_tab, accepted, LinkDisposition::NewWindow).is_err());
    assert_eq!(owners(&foreign), before);
}

#[test]
fn session_link_caps_checked_serials_real_capacity_refusal_and_closed_service_preparation_preserve_owners() {
    let files = Files::new();
    // Each case refuses before invalidating the actual outgoing source owner.
    for (tabs, windows, next_tab, next_window, disposition) in [
        (1, 2, 1, 1, LinkDisposition::BackgroundTab),
        (1, 2, 1, 1, LinkDisposition::ForegroundTab),
        (2, 1, 1, 1, LinkDisposition::NewWindow),
        (2, 2, u64::MAX, 1, LinkDisposition::ForegroundTab),
        (2, 2, u64::MAX, 1, LinkDisposition::BackgroundTab),
        (2, 2, u64::MAX, 1, LinkDisposition::NewWindow),
        (2, 2, 1, u64::MAX, LinkDisposition::NewWindow),
    ] {
        let mut session = adopt(files.source(), Limits { tabs, windows, wakes: 8, poll_tabs: 1 });
        let (window, source) = first(&session);
        let accepted = target(&mut session, source);
        press_scrollbar(&mut session, source);
        session.next_tab = next_tab;
        session.next_window = next_window;
        let before = owners(&session);
        let page = save(&session, source);
        assert!(session.open_link(window, source, accepted, disposition).is_err());
        assert_eq!(owners(&session), before);
        same(&session, source, &page);
    }

    let mut session = adopt(files.source(), Limits::default());
    let (window, source) = first(&session);
    let accepted = target(&mut session, source);
    press_scrollbar(&mut session, source);
    // Exercise the genuine std Vec capacity-overflow refusal deterministically,
    // without a fake allocator or reserving unbounded physical memory. Only this
    // private test lifts an already validated internal limit to the boundary.
    session.limits.tabs = usize::MAX;
    let before = owners(&session);
    let page = save(&session, source);
    let error = session.open_link(window, source, accepted, LinkDisposition::NewWindow).unwrap_err();
    assert!(error.contains("reserve link window tabs"), "{error}");
    assert_eq!(owners(&session), before);
    same(&session, source, &page);

    for disposition in [LinkDisposition::ForegroundTab, LinkDisposition::BackgroundTab, LinkDisposition::NewWindow] {
        let mut session = adopt(files.source(), Limits::default());
        let (window, source) = first(&session);
        let accepted = target(&mut session, source);
        press_scrollbar(&mut session, source);
        // Genuine shared-service preparation refusal through fresh_app: the
        // history owner is closed, while all original page owners stay routed.
        session.history.shutdown(crate::history::now()).unwrap();
        let before = owners(&session);
        let page = save(&session, source);
        assert!(session.open_link(window, source, accepted, disposition).is_err());
        assert_eq!(owners(&session), before);
        same(&session, source, &page);
    }
}

#[test]
fn session_link_file_target_retains_exact_resolved_base_and_original_content_root() {
    let files = Files::new();
    let mut session = adopt(files.source(), Limits::default());
    let (window, source) = first(&session);
    let (expected, root) = session.observe_tab(source, |app| {
        (navigation::resolve("target.html", &app.base_location).unwrap(), app.location.root.clone())
    }).unwrap();
    let accepted = target(&mut session, source);
    let admission = session.open_link(window, source, accepted, LinkDisposition::BackgroundTab).unwrap();
    session.observe_tab(admission.tab, |app| {
        assert_eq!(app.location, expected);
        assert_eq!(app.location.root, root);
        assert_eq!(app.document.title, "Destination 🦉");
        assert!(navigation::resolve("../../outside.html", &app.base_location).is_err(), "New owner silently granted a wider explicit file root");
    }).unwrap();
}

#[derive(Clone, Debug)]
struct Request { method: String, path: String, cookie: String, body: Vec<u8> }
struct Site {
    address: SocketAddr, stopped: Arc<AtomicBool>, requests: Arc<Mutex<Vec<Request>>>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Site {
    fn new(cross_site_base: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stopped = Arc::new(AtomicBool::new(false));
        let (seen, stop) = (requests.clone(), stopped.clone());
        let host = if cross_site_base { "127.0.0.1" } else { "localhost" };
        let source = format!("<!doctype html><base href='http://{host}:{}/base/'><title>POST source</title><style>body{{margin:0}}a{{display:block;width:180px;height:30px}}</style><body><input id=q value=seed><a id=go href=destination>Actual GET target</a>", address.port());
        let worker = thread::spawn(move || {
            let mut count = 0;
            while !stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        count += 1;
                        assert!(count <= 12, "Original HTTP fixture connection bound exceeded");
                        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                        stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
                        let mut bytes = Vec::new();
                        let mut one = [0u8; 1];
                        while !bytes.ends_with(b"\r\n\r\n") {
                            assert!(bytes.len() < 16384, "Original request header bound exceeded");
                            if stream.read_exact(&mut one).is_err() { return; }
                            bytes.push(one[0]);
                        }
                        let text = String::from_utf8(bytes).unwrap();
                        let mut words = text.lines().next().unwrap().split_whitespace();
                        let method = words.next().unwrap().to_owned();
                        let path = words.next().unwrap().to_owned();
                        let cookie = text.lines().filter_map(|line| line.split_once(':')).find(|(name, _)| name.eq_ignore_ascii_case("cookie")).map_or(String::new(), |(_, value)| value.trim().to_owned());
                        let length = text.lines().filter_map(|line| line.split_once(':')).find(|(name, _)| name.eq_ignore_ascii_case("content-length")).map_or(0, |(_, value)| value.trim().parse::<usize>().unwrap());
                        assert!(length <= 16384, "Original request body bound exceeded");
                        let mut body = vec![0; length];
                        stream.read_exact(&mut body).unwrap();
                        seen.lock().unwrap().push(Request { method, path: path.clone(), cookie, body });
                        let (status, extra, body) = match path.as_str() {
                            "/source" => ("200 OK", "", source.clone()),
                            "/base/destination" => ("302 Found", "Location: /landing\r\n", String::new()),
                            "/landing" => ("200 OK", "", "<!doctype html><title>GET destination</title><p>New chain</p>".into()),
                            _ => ("404 Not Found", "", "<title>Unexpected fixture route</title>".into()),
                        };
                        write!(stream, "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}", body.len()).unwrap();
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(1)),
                    Err(error) => panic!("{error}"),
                }
            }
        });
        Self { address, stopped, requests, worker: Some(worker) }
    }
    fn source(&self) -> String { format!("http://localhost:{}/source", self.address.port()) }
}
impl Drop for Site {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() { worker.join().unwrap(); }
    }
}
fn loaded(session: &mut Session, tab: TabId) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if session.observe_tab(tab, |app| !app.is_loading()).unwrap() { return; }
        session.poll(4).unwrap();
        assert!(Instant::now() < deadline, "Real admitted owner did not finish loading");
        thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn session_link_shared_factory_uses_original_top_cookie_origin_and_get_chain_after_source_post() {
    for cross_site_base in [true, false] {
        let site = Site::new(cross_site_base);
        let mut initial = App::new();
        let source = navigation::address(&site.source(), &Location::home()).unwrap();
        initial.start_network_request(source.clone(), NavigationKind::New, crate::http::FetchMode::Normal,
            crate::http::Request::FormPost { body: b"original=source-only".to_vec(), origin: source.url.clone() });
        initial.wait_for_load();
        assert!(!initial.failed, "{}", initial.status);
        assert_eq!(initial.document.title, "POST source");
        assert!(initial.current_request.is_post());
        let destination_url = navigation::resolve("destination", &initial.base_location).unwrap().url.unwrap();
        initial.cookies.set_http(&destination_url, "strict_target=shared; SameSite=Strict; Path=/").unwrap();
        assert!(initial.cookies.request_header(&destination_url, true, true).contains("strict_target="));
        initial.cookie_same_site = cross_site_base; // Deliberately wrong for the new target chain.
        let mut session = adopt(initial, Limits::default());
        let (window, source_tab) = first(&session);
        let accepted = target(&mut session, source_tab);
        let before = save(&session, source_tab);
        let admitted = session.open_link(window, source_tab, accepted, LinkDisposition::BackgroundTab).unwrap();
        loaded(&mut session, admitted.tab);
        same(&session, source_tab, &before);
        assert_eq!(session.active(window).unwrap(), Some(source_tab));
        session.observe_tab(admitted.tab, |app| {
            assert!(!app.failed, "{}", app.status);
            assert_eq!(app.document.title, "GET destination");
            assert!(!app.current_request.is_post());
            assert!(app.resubmission.is_none());
            assert_eq!(app.cookie_same_site, !cross_site_base);
            assert!(Arc::ptr_eq(&app.cookies, &session.cookies));
        }).unwrap();
        let requests = site.requests.lock().unwrap();
        let posted = requests.iter().find(|request| request.path == "/source").unwrap();
        assert_eq!(posted.method, "POST");
        assert_eq!(posted.body, b"original=source-only");
        for path in ["/base/destination", "/landing"] {
            let request = requests.iter().find(|request| request.path == path).unwrap();
            assert_eq!(request.method, "GET");
            assert!(request.body.is_empty(), "New owner replayed source POST");
            assert_eq!(request.cookie.contains("strict_target=shared"), !cross_site_base,
                "Cookie chain used the effective base, prior response bit, or destination in place of original top URL on {path}");
        }
    }
}
