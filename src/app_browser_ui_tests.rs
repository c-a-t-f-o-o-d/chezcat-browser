//! Actual App input, persistence and request-lifetime checks for native panels.
use super::*;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-native-ui-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str, value: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, value).unwrap();
        path
    }
    fn profile(&self) -> PathBuf {
        self.0.join("profile.txt")
    }
    fn app(&self, markup: &str) -> App {
        let mut app = App::new();
        app.attach_profile(Store::open(self.profile()).unwrap());
        app.open(&self.file("page.html", markup));
        assert!(!app.failed, "{}", app.status);
        app
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn key(app: &mut App, key: u32) {
    app.key(key, false, false, false);
}
fn shortcut(app: &mut App, key: u32) {
    app.key(key, true, false, false);
}
fn replace(app: &mut App, value: &str) {
    shortcut(app, 0x41);
    assert!(matches!(
        app.key(0x56, true, false, false),
        PlatformAction::Paste
    ));
    app.paste(value);
}
fn click(app: &mut App, action: Action) {
    let control = app
        .browser_controls()
        .into_iter()
        .find(|c| c.action == action)
        .unwrap();
    let point = (
        control.rect.x + control.rect.w / 2,
        control.rect.y + control.rect.h / 2,
    );
    assert!(control.enabled);
    app.pointer_down(point.0, point.1, false);
    app.pointer_up(point.0, point.1);
}
fn focus(app: &App) -> Action {
    app.browser_panel.as_ref().unwrap().focus
}
fn shot(app: &App, name: &str) {
    let Some(folder) = std::env::var_os("CHEZCAT_NATIVE_SHOTS") else {
        return;
    };
    let folder = PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let mut output = std::io::BufWriter::new(std::fs::File::create(folder.join(name)).unwrap());
    let canvas = app.draw();
    write!(output, "P6\n{} {}\n255\n", canvas.width, canvas.height).unwrap();
    for pixel in canvas.pixels {
        output
            .write_all(&[(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8])
            .unwrap();
    }
}

#[test]
fn bookmark_keyboard_edit_clipboard_roundtrip_and_missing_file() {
    let fixture = Fixture::new();
    let mut app = fixture.app("<title>Original page</title><h1>Page</h1>");
    let original = app.location.display();
    shortcut(&mut app, 0x44);
    assert!(focus(&app) == Action::Title);
    app.paste("Café bookmark ");
    app.utf16_input(0xd83e);
    app.utf16_input(0xdd80);
    let expected = "Café bookmark 🦀";
    shortcut(&mut app, 0x41);
    assert!(
        matches!(app.key(0x43, true, false, false), PlatformAction::Copy(text) if text == expected)
    );
    let PlatformAction::Cut(text) = app.key(0x58, true, false, false) else {
        panic!("cut action");
    };
    // Clipboard failure must not delete the selection. Deletion follows the
    // platform's successful copy acknowledgment.
    assert_eq!(app.browser_panel.as_ref().unwrap().title.text(), expected);
    app.cut_copied(&text);
    assert!(app.browser_panel.as_ref().unwrap().title.text().is_empty());
    app.paste(&text);
    key(&mut app, 0x09);
    assert!(focus(&app) == Action::Address);
    assert_eq!(app.browser_panel.as_ref().unwrap().address.text(), original);
    key(&mut app, 0x0d);
    assert_eq!(
        app.profile.data().bookmarks,
        vec![Bookmark {
            title: expected.into(),
            url: original.clone(),
            ..Bookmark::default()
        }]
    );
    assert_eq!(
        Store::open(fixture.profile()).unwrap().data(),
        app.profile.data()
    );

    // Same document adds edit, not a duplicate.
    shortcut(&mut app, 0x44);
    assert!(matches!(
        app.browser_panel.as_ref().unwrap().kind,
        PanelKind::EditBookmark { index: Some(0) }
    ));
    app.paste("Updated name");
    key(&mut app, 0x0d);
    assert_eq!(app.profile.data().bookmarks.len(), 1);

    // Editing the title still works when that saved local file has moved.
    std::fs::remove_file(fixture.0.join("page.html")).unwrap();
    for _ in 0..3 {
        key(&mut app, 0x09);
    }
    assert!(focus(&app) == Action::Edit);
    key(&mut app, 0x0d);
    app.paste("Moved file, retained bookmark");
    key(&mut app, 0x0d);
    assert_eq!(
        app.profile.data().bookmarks[0].title,
        "Moved file, retained bookmark"
    );
    assert_eq!(app.profile.data().bookmarks[0].url, original);
    assert!(app.browser_panel.as_ref().unwrap().error.is_empty());

    let mut reopened = App::new();
    reopened.attach_profile(Store::open(fixture.profile()).unwrap());
    reopened.key(0x42, true, true, false);
    assert_eq!(reopened.profile.data(), app.profile.data());
    shot(&reopened, "bookmarks-list.ppm");
    key(&mut reopened, 0x0d);
    assert!(
        reopened.failed,
        "Missing bookmark target should report a navigation failure"
    );
    assert_eq!(reopened.profile.data().bookmarks.len(), 1);
    click(&mut reopened, Action::Edit);
    key(&mut reopened, 0x09);
    replace(&mut reopened, "javascript:alert(1)");
    key(&mut reopened, 0x0d);
    assert_eq!(reopened.profile.data().bookmarks[0].url, original);
    assert!(!reopened.browser_panel.as_ref().unwrap().error.is_empty());
    replace(&mut reopened, "example.COM/new#part");
    key(&mut reopened, 0x0d);
    assert_eq!(
        reopened.profile.data().bookmarks[0].url,
        "https://example.com/new#part"
    );
    assert_eq!(
        Store::open(fixture.profile()).unwrap().data(),
        reopened.profile.data()
    );
}

#[test]
fn bookmark_mouse_scrolling_keyboard_delete_and_small_window_menu() {
    let mut app = App::new();
    let mut data = app.profile.data().clone();
    for i in 0..20 {
        data.bookmarks.push(Bookmark {
            title: format!("Saved page {i:02}"),
            url: format!("about:home#page{i}"),
            order: i,
            ..Bookmark::default()
        });
    }
    app.profile.save(data).unwrap();
    app.resize(420, 260);
    // The native MENU remains usable at the minimum desktop client size.
    app.pointer_down(380, 30, false);
    app.pointer_up(380, 30);
    assert!(app.menu);
    // Readable rows scroll instead of shrinking into tiny pointer targets.
    app.wheel(-360, false);
    let (x, y, w, h) = app
        .chrome_rect(super::super::chrome::ChromeAction::MenuItem(10))
        .unwrap();
    app.pointer_down(x + w / 2, y + h / 2, false);
    app.pointer_up(x + w / 2, y + h / 2);
    assert!(matches!(
        app.browser_panel.as_ref().unwrap().kind,
        PanelKind::Bookmarks
    ));
    app.wheel(-120, false);
    assert_eq!(app.browser_panel.as_ref().unwrap().first, 3);
    let body = app.browser_body();
    app.pointer_down(body.x + 25, body.y + 20, false);
    app.pointer_up(body.x + 25, body.y + 20);
    assert_eq!(app.browser_panel.as_ref().unwrap().selected, Some(3));
    key(&mut app, 0x23);
    assert_eq!(app.browser_panel.as_ref().unwrap().selected, Some(19));
    shot(&app, "bookmarks-small.ppm");
    key(&mut app, 0x2e);
    assert_eq!(app.profile.data().bookmarks.len(), 19);
    assert_eq!(app.browser_panel.as_ref().unwrap().selected, Some(18));
    click(&mut app, Action::Open);
    assert!(app.browser_panel.is_none());
    assert_eq!(app.location.fragment, "page18");
    app.show_bookmarks();
    while !app.profile.data().bookmarks.is_empty() {
        key(&mut app, 0x2e);
    }
    assert_eq!(app.browser_panel.as_ref().unwrap().selected, None);
    assert!(app
        .browser_controls()
        .iter()
        .filter(|c| matches!(c.action, Action::Open | Action::Edit | Action::Delete))
        .all(|c| !c.enabled));
    shot(&app, "bookmarks-empty-small.ppm");
}

#[test]
fn settings_apply_controls_scripts_images_zoom_home_and_persists() {
    let fixture = Fixture::new();
    fixture.file("pixel.ppm", "P3\n1 1\n255\n200 30 40\n");
    let mut app = fixture.app("<title>Unscripted title</title><img src=pixel.ppm><script>document.title='Script ran'; setTimeout(function(){document.title='Late callback';},600000);</script>");
    assert_eq!(app.document.title, "Script ran");
    assert!(!app.images.is_empty());
    assert!(app.next_script_timer().is_some());
    let home = app.location.display();
    app.resize(420, 260);
    shortcut(&mut app, 0xbc);
    assert!(focus(&app) == Action::JavaScript);
    key(&mut app, 0x20);
    key(&mut app, 0x09);
    key(&mut app, 0x20);
    key(&mut app, 0x09);
    assert!(focus(&app) == Action::BlockingEnabled);
    key(&mut app, 0x09);
    assert!(focus(&app) == Action::Zoom);
    replace(&mut app, "150");
    key(&mut app, 0x09);
    key(&mut app, 0x09);
    assert!(focus(&app) == Action::CurrentHome);
    let body = app.browser_body();
    let control = app
        .browser_controls()
        .into_iter()
        .find(|c| c.action == Action::CurrentHome)
        .unwrap();
    assert!(control.rect.y >= body.y && control.rect.y + control.rect.h <= body.y + body.h);
    key(&mut app, 0x0d);
    assert_eq!(app.browser_panel.as_ref().unwrap().address.text(), home);
    shot(&app, "settings-small-scrolled.ppm");
    key(&mut app, 0x09);
    key(&mut app, 0x09);
    assert!(focus(&app) == Action::HistoryOptions);
    key(&mut app, 0x09);
    assert!(focus(&app) == Action::BlockingOptions);
    key(&mut app, 0x09);
    assert_eq!(focus(&app), Action::DarkMode);
    for _ in 0..app.browser_focus_order().len() {
        if focus(&app) == Action::Apply {
            break;
        }
        key(&mut app, 0x09);
    }
    assert!(focus(&app) == Action::Apply);
    key(&mut app, 0x0d);
    assert!(app.browser_panel.is_none());
    assert!(!app.scripts_enabled && !app.images_enabled);
    assert!(app.scripting.is_none());
    assert!(app.next_script_timer().is_none());
    assert!(app.images.is_empty());
    assert_eq!(app.document.title, "Unscripted title");
    assert_eq!(app.zoom, 1.5);
    assert_eq!(
        app.profile.data().settings.home.as_deref(),
        Some(home.as_str())
    );
    let saved = Store::open(fixture.profile()).unwrap();
    assert_eq!(saved.data(), app.profile.data());
    let mut reopened = App::new();
    reopened.attach_profile(saved);
    assert!(!reopened.scripts_enabled && !reopened.images_enabled);
    assert_eq!(reopened.zoom, 1.5);
    assert!(reopened.location.path.is_none());
    reopened.open_startup_home();
    assert_eq!(reopened.location.display(), home);
    assert_eq!(reopened.document.title, "Unscripted title");
    reopened.set_zoom(2.0);
    shortcut(&mut reopened, 0x30);
    assert_eq!(reopened.zoom, 1.5);
    // Ctrl+J is the same persisted switch, including from a native panel.
    reopened.show_settings();
    shortcut(&mut reopened, 0x4a);
    assert!(reopened.scripts_enabled);
    assert!(reopened.browser_panel.as_ref().unwrap().javascript);
    assert!(
        Store::open(fixture.profile())
            .unwrap()
            .data()
            .settings
            .javascript
    );
    assert_eq!(reopened.document.title, "Script ran");
    assert!(reopened.images.is_empty());
    reopened.resize(1080, 760);
    shot(&reopened, "settings.ppm");
}

#[test]
fn blocking_rules_are_staged_saved_cancelled_and_applied_to_load_options() {
    let fixture = Fixture::new();
    let mut app = fixture.app("<title>Request rules</title><p>Existing page</p>");
    app.show_settings();
    click(&mut app, Action::BlockingEnabled);
    click(&mut app, Action::BlockingOptions);
    assert_eq!(focus(&app), Action::RuleEntry);
    app.paste("Ads.Example.com");
    key(&mut app, 0x0d);
    assert_eq!(
        app.browser_panel.as_ref().unwrap().blocking.rules,
        ["host:ads.example.com"]
    );
    assert!(!fixture.profile().exists());
    app.paste("path:/bad*");
    key(&mut app, 0x0d);
    assert!(!app.browser_panel.as_ref().unwrap().error.is_empty());
    replace(&mut app, "@@path:/ads/allowed/");
    key(&mut app, 0x0d);
    shot(&app, "blocking-rules.ppm");
    click(&mut app, Action::ApplyBlocking);
    assert!(matches!(
        app.browser_panel.as_ref().unwrap().kind,
        PanelKind::Settings
    ));
    assert!(app.browser_panel.as_ref().unwrap().blocking.enabled);
    // Returning from the child editor does not silently save the parent draft.
    assert!(!app.profile.data().settings.blocking.enabled);
    click(&mut app, Action::Apply);
    assert!(app.browser_panel.is_none());
    assert!(app.profile.data().settings.blocking.enabled);
    assert_eq!(
        Store::open(fixture.profile()).unwrap().data(),
        app.profile.data()
    );
    let url = crate::url::parse("https://ads.example.com/pixel")
        .unwrap()
        .0;
    assert!(app.load_options().blocking.check(&url).is_err());
    let allowed = crate::url::parse("https://ads.example.com/ads/allowed/banner")
        .unwrap()
        .0;
    assert!(app.load_options().blocking.check(&allowed).is_ok());
    let saved = app.profile.data().clone();
    app.show_settings();
    click(&mut app, Action::BlockingOptions);
    app.paste("path:/new-rule/");
    key(&mut app, 0x0d);
    key(&mut app, 0x1b);
    assert_eq!(
        app.browser_panel.as_ref().unwrap().blocking,
        saved.settings.blocking
    );
    click(&mut app, Action::BlockingEnabled);
    key(&mut app, 0x1b);
    assert_eq!(app.profile.data(), &saved);
    assert!(app.load_options().blocking.check(&url).is_err());
}

#[test]
fn blocking_rule_list_keeps_its_own_selection_and_keyboard_scroll() {
    let mut app = App::new();
    app.show_settings();
    app.show_blocking_options();
    for index in 0..24 {
        app.browser_panel
            .as_mut()
            .unwrap()
            .address
            .set(&format!("path:/ads/{index}/"));
        key(&mut app, 0x0d);
    }
    app.browser_panel.as_mut().unwrap().focus = Action::RuleList;
    key(&mut app, 0x24);
    assert_eq!(app.browser_panel.as_ref().unwrap().first, 0);
    key(&mut app, 0x23);
    assert_eq!(app.browser_panel.as_ref().unwrap().selected, Some(23));
    assert!(app.browser_panel.as_ref().unwrap().first > 0);
    let list = app.blocking_list_rect();
    let first = app.browser_panel.as_ref().unwrap().first;
    app.pointer_down(list.x + 25, list.y + 15, false);
    app.pointer_up(list.x + 25, list.y + 15);
    assert_eq!(app.browser_panel.as_ref().unwrap().selected, Some(first));
    key(&mut app, 0x2e);
    assert_eq!(app.browser_panel.as_ref().unwrap().blocking.rules.len(), 23);
    assert!(app.profile.data().bookmarks.is_empty());
    key(&mut app, 0x0d);
    assert_eq!(focus(&app), Action::RuleEntry);
    assert!(!app
        .browser_panel
        .as_ref()
        .unwrap()
        .address
        .selected()
        .is_empty());
    // At minimum size the editor and footer remain accessible by Tab.
    app.resize(420, 260);
    app.ensure_browser_focus_visible();
    assert!(app.browser_panel.as_ref().unwrap().scroll > 0);
    shot(&app, "blocking-small.ppm");
}

#[test]
fn settings_cancel_validation_and_missing_home_are_independent() {
    let fixture = Fixture::new();
    let mut app = fixture.app("<title>Visible document</title>");
    let previous = app.profile.data().clone();
    shortcut(&mut app, 0xbc);
    key(&mut app, 0x20);
    key(&mut app, 0x1b);
    assert!(app.scripts_enabled);
    assert_eq!(app.profile.data(), &previous);
    assert!(!fixture.profile().exists());
    shortcut(&mut app, 0xbc);
    key(&mut app, 0x20);
    key(&mut app, 0x09);
    key(&mut app, 0x09);
    key(&mut app, 0x09);
    replace(&mut app, "499");
    key(&mut app, 0x0d);
    assert!(app
        .browser_panel
        .as_ref()
        .unwrap()
        .error
        .contains("50 to 300"));
    assert!(app.scripts_enabled);
    assert_eq!(app.profile.data(), &previous);
    replace(&mut app, "125");
    key(&mut app, 0x09);
    replace(&mut app, "javascript:document.title='unsafe'");
    key(&mut app, 0x0d);
    assert!(app.browser_panel.is_some());
    assert_eq!(app.profile.data(), &previous);
    let missing = navigation::explicit(&fixture.0.join("page.html"))
        .unwrap()
        .display();
    std::fs::remove_file(fixture.0.join("page.html")).unwrap();
    replace(&mut app, &missing);
    key(&mut app, 0x0d);
    assert!(app.browser_panel.is_none());
    assert_eq!(
        app.profile.data().settings.home.as_deref(),
        Some(missing.as_str())
    );
    // Unrelated preference changes must not attempt to open the stored home.
    app.show_settings();
    key(&mut app, 0x09);
    key(&mut app, 0x09);
    key(&mut app, 0x09);
    replace(&mut app, "175");
    key(&mut app, 0x0d);
    assert!(app.browser_panel.is_none());
    assert_eq!(
        Store::open(fixture.profile())
            .unwrap()
            .data()
            .settings
            .default_zoom,
        175
    );
    app.show_settings();
    click(&mut app, Action::BundledHome);
    click(&mut app, Action::Apply);
    assert!(app.profile.data().settings.home.is_none());
    app.home();
    assert!(!app.failed);
    assert!(app.location.path.is_none());
}

#[test]
fn failed_profile_save_preserves_bookmarks_draft_and_running_settings() {
    let fixture = Fixture::new();
    let mut app = fixture.app("<title>Saved page</title>");
    shortcut(&mut app, 0x44);
    key(&mut app, 0x0d);
    let original = app.profile.data().clone();
    let mut other = Store::open(fixture.profile()).unwrap();
    let mut next = other.data().clone();
    next.bookmarks[0].title = "Changed by another browser window".into();
    other.save(next).unwrap();
    let on_disk = std::fs::read(fixture.profile()).unwrap();
    shortcut(&mut app, 0x44);
    app.paste("Keep my unsaved edit");
    key(&mut app, 0x0d);
    assert!(matches!(
        app.browser_panel.as_ref().unwrap().kind,
        PanelKind::EditBookmark { .. }
    ));
    assert_eq!(
        app.browser_panel.as_ref().unwrap().title.text(),
        "Keep my unsaved edit"
    );
    assert!(!app.browser_panel.as_ref().unwrap().error.is_empty());
    assert_eq!(app.profile.data(), &original);
    assert_eq!(std::fs::read(fixture.profile()).unwrap(), on_disk);
    shortcut(&mut app, 0x4a);
    assert!(app.scripts_enabled);
    assert!(app.profile.data().settings.javascript);
    assert_eq!(std::fs::read(fixture.profile()).unwrap(), on_disk);
    shot(&app, "bookmark-save-error.ppm");
    shortcut(&mut app, 0xbc);
    key(&mut app, 0x20);
    click(&mut app, Action::Apply);
    assert!(app.scripts_enabled);
    assert!(!app.browser_panel.as_ref().unwrap().javascript);
    assert!(!app.browser_panel.as_ref().unwrap().error.is_empty());
    assert_eq!(app.profile.data(), &original);
}

#[test]
fn native_panels_isolate_document_events_and_bound_title_suggestions() {
    let fixture = Fixture::new();
    let mut app = fixture.app("<title>Page untouched</title><input id=q value=original><script>document.getElementById('q').oninput=function(){document.title='Page input fired';};document.addEventListener('keydown',function(){document.title='Page key fired';});</script>");
    let node = app
        .document
        .nodes
        .iter()
        .enumerate()
        .find_map(|(node, _)| (app.document.attr(node, "id") == Some("q")).then_some(node))
        .unwrap();
    app.focus_control(node);
    shortcut(&mut app, 0x44);
    replace(&mut app, "Native edit");
    key(&mut app, 0x25);
    app.text_input('!');
    assert_eq!(app.document.title, "Page untouched");
    assert_eq!(app.forms.controls[&node].value.text(), "original");
    assert!(app.selection.is_none());
    app.document.title = format!("\0\n\t{}", "🦀  \t".repeat(5000));
    shortcut(&mut app, 0x44);
    let title = app.browser_panel.as_ref().unwrap().title.text();
    assert!(title.len() <= MAX_TITLE_BYTES);
    assert!(!title.chars().any(char::is_control));
    assert!(!title.contains("  "));
    app.document.title = "\0\u{7f}\n\t".into();
    shortcut(&mut app, 0x44);
    assert_eq!(
        app.browser_panel.as_ref().unwrap().title.text(),
        app.location.display()
    );
}

#[test]
fn unavailable_profile_warning_survives_startup_and_does_not_mask_navigation_errors() {
    let fixture = Fixture::new();
    let path = fixture.file("profile.txt", "not a valid profile");
    let error = Store::open(path.clone()).unwrap_err();
    let mut app = App::new();
    app.attach_profile(Store::unavailable(path.clone(), error.clone()));
    app.use_home_file(fixture.file("start.html", "<title>Startup home</title>"));
    assert_eq!(app.document.title, "Startup home");
    let canvas = app.draw();
    assert_eq!(canvas.pixels[(canvas.height - 2) * canvas.width], ui::CREAM);
    shortcut(&mut app, 0xbc);
    assert_eq!(app.browser_panel.as_ref().unwrap().error, error);
    key(&mut app, 0x20);
    click(&mut app, Action::Apply);
    assert!(app.scripts_enabled);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "not a valid profile"
    );
    key(&mut app, 0x1b);
    app.open(&fixture.0.join("missing.html"));
    assert!(app.failed);
    let canvas = app.draw();
    assert_eq!(canvas.pixels[(canvas.height - 2) * canvas.width], ui::ERROR);
}

#[derive(Clone, Debug)]
struct Request {
    method: String,
    path: String,
    body: String,
}
struct Site {
    base: String,
    seen: Arc<Mutex<Vec<Request>>>,
    release: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Site {
    fn new() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let release = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let recorded = seen.clone();
        let ready = release.clone();
        let ending = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut clients = Vec::new();
            while !ending.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // Accepted Winsock sockets inherit the polling listener mode.
                        stream.set_nonblocking(false).unwrap();
                        let recorded = recorded.clone();
                        let ready = ready.clone();
                        clients.push(std::thread::spawn(move || {
                            stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                            let mut headers = Vec::new();
                            let mut byte = [0];
                            while !headers.ends_with(b"\r\n\r\n") && headers.len() < 32 * 1024 {
                                if stream.read(&mut byte).unwrap_or(0) == 0 { return; }
                                headers.push(byte[0]);
                            }
                            let headers = String::from_utf8(headers).unwrap();
                            let method = headers.split_whitespace().next().unwrap().to_owned();
                            let path = headers.split_whitespace().nth(1).unwrap().to_owned();
                            let count: usize = headers.lines().find_map(|line| line.strip_prefix("Content-Length: ")).map(|value| value.parse().unwrap()).unwrap_or(0);
                            assert!(count < 1024);
                            let mut body = vec![0; count];
                            if stream.read_exact(&mut body).is_err() { return; }
                            recorded.lock().unwrap().push(Request { method: method.clone(), path: path.clone(), body: String::from_utf8(body).unwrap() });
                            let response = match (method.as_str(), path.as_str()) {
                                ("GET", "/slow") => {
                                    let started = Instant::now();
                                    while !ready.load(Ordering::Relaxed) && started.elapsed() < Duration::from_secs(5) { std::thread::sleep(Duration::from_millis(2)); }
                                    "<title>Delayed document</title>"
                                }
                                ("GET", "/form") => "<title>Form page</title><form action=/result method=post><input name=q value=private-body><button id=send>Send</button></form>",
                                ("POST", "/result") => "<title>POST result</title><script>setTimeout(function(){document.title='Callback ran';},600000);</script>",
                                ("GET", "/result") => "<title>GET result</title>",
                                _ => "<title>Other page</title>",
                            };
                            let reply = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len());
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
            base,
            seen,
            release,
            stop,
            thread: Some(thread),
        }
    }
    fn open(&self, app: &mut App, path: &str) {
        app.open_address(&format!("{}{path}", self.base));
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
    }
    fn wait_request(&self, path: &str) {
        let started = Instant::now();
        while !self
            .seen
            .lock()
            .unwrap()
            .iter()
            .any(|request| request.path == path)
        {
            assert!(started.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
impl Drop for Site {
    fn drop(&mut self) {
        self.release.store(true, Ordering::Relaxed);
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
fn bookmark_captures_displayed_page_during_pending_and_failed_navigation() {
    let fixture = Fixture::new();
    let mut app = fixture.app("<title>Committed document</title>");
    let committed = app.location.display();
    shortcut(&mut app, 0x4c);
    app.paste("https://unsent.invalid/edited-address");
    shortcut(&mut app, 0x44);
    assert_eq!(
        app.browser_panel.as_ref().unwrap().address.text(),
        committed
    );
    key(&mut app, 0x1b);
    let site = Site::new();
    app.open_address(&format!("{}/slow", site.base));
    site.wait_request("/slow");
    assert!(app.is_loading());
    shortcut(&mut app, 0x44);
    assert_eq!(
        app.browser_panel.as_ref().unwrap().title.text(),
        "Committed document"
    );
    assert_eq!(
        app.browser_panel.as_ref().unwrap().address.text(),
        committed
    );
    // A later document commit must not rewrite an in-progress bookmark draft.
    app.paste("My captured title");
    site.release.store(true, Ordering::Relaxed);
    app.wait_for_load();
    assert_eq!(app.document.title, "Delayed document");
    assert_eq!(
        app.browser_panel.as_ref().unwrap().title.text(),
        "My captured title"
    );
    assert_eq!(
        app.browser_panel.as_ref().unwrap().address.text(),
        committed
    );
    key(&mut app, 0x0d);
    assert_eq!(app.profile.data().bookmarks[0].url, committed);
    key(&mut app, 0x1b);
    app.open(&fixture.0.join("does-not-exist.html"));
    assert!(app.failed);
    shortcut(&mut app, 0x44);
    assert_eq!(
        app.browser_panel.as_ref().unwrap().title.text(),
        "Delayed document"
    );
    assert_eq!(
        app.browser_panel.as_ref().unwrap().address.text(),
        format!("{}/slow", site.base)
    );
}

#[test]
fn post_bookmarks_open_with_get_and_setting_reload_keeps_resubmit_prompt() {
    let fixture = Fixture::new();
    let mut app = App::new();
    app.attach_profile(Store::open(fixture.profile()).unwrap());
    let site = Site::new();
    site.open(&mut app, "/form");
    let send = app
        .document
        .nodes
        .iter()
        .enumerate()
        .find_map(|(node, _)| (app.document.attr(node, "id") == Some("send")).then_some(node))
        .unwrap();
    app.activate_control(send);
    app.wait_for_load();
    assert!(app.current_request.is_post());
    assert_eq!(app.document.title, "POST result");
    assert!(app.next_script_timer().is_some());
    shortcut(&mut app, 0x44);
    key(&mut app, 0x0d);
    assert!(!std::fs::read_to_string(fixture.profile())
        .unwrap()
        .contains("private-body"));
    shortcut(&mut app, 0xbc);
    key(&mut app, 0x20);
    click(&mut app, Action::Apply);
    assert!(!app.scripts_enabled);
    assert!(app.scripting.is_none());
    assert!(app.next_script_timer().is_none());
    assert!(!app.is_loading());
    let (token, url) = app
        .take_resubmission_prompt()
        .expect("Settings reload must ask before POST replay");
    assert_eq!(url, format!("{}/result", site.base));
    app.confirm_resubmission(token, false);
    assert_eq!(
        site.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.method == "POST")
            .count(),
        1
    );
    app.key(0x42, true, true, false);
    key(&mut app, 0x0d);
    app.wait_for_load();
    assert_eq!(app.document.title, "GET result");
    assert!(!app.current_request.is_post());
    assert!(app.take_resubmission_prompt().is_none());
    let requests = site.seen.lock().unwrap();
    let results = requests
        .iter()
        .filter(|r| r.path == "/result")
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].method, "POST");
    assert_eq!(results[0].body, "q=private-body");
    assert_eq!(results[1].method, "GET");
    assert!(results[1].body.is_empty());
}

fn click_visible(app: &mut App, action: Action) {
    app.browser_panel.as_mut().unwrap().focus = action;
    app.ensure_browser_focus_visible();
    click(app, action);
}

#[test]
fn bookmark_folders_edit_move_and_preserve_the_settings_draft() {
    let fixture = Fixture::new();
    let mut app = fixture.app("<title>Local reference</title><p>Reference</p>");
    app.show_settings();
    click(&mut app, Action::JavaScript);
    assert!(!app.browser_panel.as_ref().unwrap().javascript);
    click_visible(&mut app, Action::BookmarksOptions);
    click(&mut app, Action::NewFolder);
    app.paste("Research");
    key(&mut app, 0x0d);
    let research = app.profile.data().folders[0].id;
    assert_eq!(
        app.browser_panel.as_ref().unwrap().selected_folder,
        Some(research)
    );
    click(&mut app, Action::Open);
    click(&mut app, Action::NewFolder);
    app.paste("Standards");
    key(&mut app, 0x0d);
    let standards = app
        .profile
        .data()
        .folders
        .iter()
        .find(|f| f.parent == research)
        .unwrap()
        .id;
    click(&mut app, Action::Edit);
    app.paste("Specs");
    key(&mut app, 0x0d);
    assert_eq!(app.profile.data().folder(standards).unwrap().title, "Specs");
    click(&mut app, Action::Move);
    click_visible(&mut app, Action::DestinationRow(standards));
    assert!(!app.browser_panel.as_ref().unwrap().error.is_empty());
    assert_eq!(
        app.profile.data().folder(standards).unwrap().parent,
        research
    );
    click(&mut app, Action::Cancel);
    click(&mut app, Action::Move);
    click_visible(&mut app, Action::DestinationRow(0));
    assert_eq!(app.profile.data().folder(standards).unwrap().parent, 0);
    app.browser_activate(Action::FolderRow(research));
    click(&mut app, Action::Open);
    click(&mut app, Action::Add);
    key(&mut app, 0x0d);
    assert_eq!(app.profile.data().bookmarks[0].folder, research);
    click(&mut app, Action::Edit);
    click_visible(&mut app, Action::Destination);
    click_visible(&mut app, Action::DestinationRow(standards));
    click(&mut app, Action::SaveBookmark);
    assert_eq!(app.profile.data().bookmarks[0].folder, standards);
    assert_eq!(app.browser_panel.as_ref().unwrap().folder, standards);
    click(&mut app, Action::SettingsBack);
    assert!(matches!(
        app.browser_panel.as_ref().unwrap().kind,
        PanelKind::Settings
    ));
    assert!(!app.browser_panel.as_ref().unwrap().javascript);
    assert!(app.scripts_enabled && app.profile.data().settings.javascript);
    click(&mut app, Action::Cancel);
    assert_eq!(
        Store::open(fixture.profile()).unwrap().data(),
        app.profile.data()
    );
}

#[test]
fn bookmark_folder_delete_confirms_subtree_and_retains_other_bookmarks() {
    let mut app = App::new();
    let mut data = app.profile.data().clone();
    let parent = data.add_folder(0, "Delete me".into()).unwrap();
    let child = data.add_folder(parent, "Child".into()).unwrap();
    for (folder, url) in [
        (parent, "about:home#one"),
        (child, "about:home#two"),
        (0, "about:home#keep"),
    ] {
        data.bookmarks.push(Bookmark {
            title: url.into(),
            url: url.into(),
            folder,
            order: data.next_order(folder),
        });
    }
    app.profile.save(data).unwrap();
    app.show_bookmarks();
    app.browser_activate(Action::FolderRow(parent));
    click(&mut app, Action::Delete);
    assert!(
        matches!(app.browser_panel.as_ref().unwrap().kind,PanelKind::DeleteFolder{id} if id==parent)
    );
    assert_eq!(app.profile.data().bookmarks.len(), 3);
    key(&mut app, 0x1b);
    assert_eq!(app.profile.data().folders.len(), 2);
    click(&mut app, Action::Delete);
    click(&mut app, Action::ConfirmDeleteFolder);
    assert!(app.profile.data().folders.is_empty());
    assert_eq!(app.profile.data().bookmarks.len(), 1);
    assert_eq!(app.profile.data().bookmarks[0].url, "about:home#keep");
}

#[test]
fn bookmark_tree_reorders_mixed_rows_and_keyboard_enters_parent_folders() {
    use crate::profile::BookmarkItem;
    let mut app = App::new();
    let mut data = app.profile.data().clone();
    let folder = data.add_folder(0, "Folder".into()).unwrap();
    for n in 0..2 {
        data.bookmarks.push(Bookmark {
            title: format!("Link {n}"),
            url: format!("about:home#{n}"),
            order: n + 1,
            ..Bookmark::default()
        });
    }
    app.profile.save(data).unwrap();
    shortcut(&mut app, 0x42);
    app.browser_activate(Action::Row(0));
    click(&mut app, Action::MoveDown);
    assert_eq!(
        app.profile.data().items(0),
        vec![
            BookmarkItem::Folder(folder),
            BookmarkItem::Link(1),
            BookmarkItem::Link(0)
        ]
    );
    click(&mut app, Action::MoveUp);
    assert_eq!(app.profile.data().items(0)[1], BookmarkItem::Link(0));
    app.browser_activate(Action::FolderRow(folder));
    key(&mut app, 0x0d);
    assert_eq!(app.browser_panel.as_ref().unwrap().folder, folder);
    key(&mut app, 0x08);
    assert_eq!(app.browser_panel.as_ref().unwrap().folder, 0);
    assert_eq!(
        app.browser_panel.as_ref().unwrap().selected_folder,
        Some(folder)
    );
}

#[test]
fn dark_native_panels_share_palette_and_small_controls_remain_reachable() {
    let mut app = App::new();
    let mut data = app.profile.data().clone();
    data.settings.dark_mode = true;
    let folder = data.add_folder(0, "Research".into()).unwrap();
    data.add_folder(folder, "Specifications".into()).unwrap();
    data.bookmarks.push(Bookmark {
        title: "Browser testing".into(),
        url: "https://example.com/".into(),
        order: 1,
        ..Bookmark::default()
    });
    app.profile.save(data).unwrap();
    app.show_bookmarks();
    shot(&app, "bookmarks-tree-dark.ppm");
    let colors = app.browser_palette();
    for show in [App::show_bookmarks, App::show_settings, App::show_history] {
        show(&mut app);
        let canvas = app.draw();
        let body = app.browser_body();
        assert_eq!(
            canvas.pixels[(body.y + 4) as usize * canvas.width + 1],
            colors.background
        );
    }
    app.show_settings();
    shot(&app, "settings-dark.ppm");
    app.resize(420, 260);
    for action in [
        Action::JavaScript,
        Action::LinkColor,
        Action::VisitedColor,
        Action::DownloadFolder,
    ] {
        app.browser_panel.as_mut().unwrap().focus = action;
        app.ensure_browser_focus_visible();
        let body = app.browser_body();
        let control = app
            .browser_controls()
            .into_iter()
            .find(|c| c.action == action)
            .unwrap();
        assert!(
            control.rect.y >= body.y && control.rect.y + control.rect.h <= body.y + body.h,
            "field {action:?} is clipped"
        );
    }
    shot(&app, "settings-dark-small.ppm");
}

#[test]
fn deleting_history_rebuilds_visited_colors_after_the_last_retained_visit() {
    let mut app = App::new();
    app.document = html::parse("<a href='https://example.com/read#section'>Read</a>");
    app.forms = Forms::new(&app.document);
    let mut data = app.profile.data().clone();
    data.settings.link_color = Some(0x123456);
    data.settings.visited_color = Some(0x789abc);
    app.profile.save(data).unwrap();
    let now = crate::history::now();
    app.browsing_history
        .configure(
            crate::history::Policy {
                enabled: true,
                retention_days: None,
                clear_on_exit: false,
            },
            now,
        )
        .unwrap();
    for fragment in ["first", "second"] {
        app.browsing_history
            .record(now, "Read", &format!("https://example.com/read#{fragment}"))
            .unwrap();
    }
    app.visited.insert("https://example.com/read".into());
    app.reflow();
    assert_eq!(app.link_paint.get(&0), Some(&0x789abc));
    shortcut(&mut app, 0x48);
    key(&mut app, 0x2e);
    assert_eq!(app.browsing_history.data().entries.len(), 1);
    assert!(!app.visited.contains("https://example.com/read"));
    assert_eq!(app.link_paint.get(&0), Some(&0x789abc));
    key(&mut app, 0x28);
    key(&mut app, 0x2e);
    assert!(app.browsing_history.data().entries.is_empty());
    assert_eq!(app.link_paint.get(&0), Some(&0x123456));
}


#[test]
fn address_search_settings_hits_cancel_apply_restart_and_stale_save_are_atomic() {
    use crate::profile::SearchEngine;
    let fixture=Fixture::new();
    let mut app=fixture.app("<title>Preserved page</title><script>window.marker={value:17};</script>");
    let location=app.location.clone();let epoch=app.script_epoch;app.resize(420,260);
    app.input_settings_action("Open").unwrap();app.input_settings_action("Bing").unwrap();
    assert_eq!(app.profile.data().settings.search_engine,SearchEngine::Google);
    app.input_settings_action("Cancel").unwrap();
    assert_eq!(app.profile.data().settings.search_engine,SearchEngine::Google);assert!(!fixture.profile().exists());
    for engine in [SearchEngine::Bing,SearchEngine::DuckDuckGo,SearchEngine::None,SearchEngine::Google] {
        app.input_settings_action("Open").unwrap();app.input_settings_action(engine.name()).unwrap();
        assert_eq!(app.browser_panel.as_ref().unwrap().search_engine,engine);
        app.input_settings_action("Apply").unwrap();assert!(app.browser_panel.is_none(),"{}",app.status);
        assert_eq!(app.profile.data().settings.search_engine,engine);
        let mut reopened=App::new();reopened.attach_profile(Store::open(fixture.profile()).unwrap());
        assert_eq!(reopened.profile.data().settings.search_engine,engine);
        assert_eq!(app.location,location);assert_eq!(app.script_epoch,epoch,"Changing only search engine must not reload");
    }
    let before=std::fs::read(fixture.profile()).unwrap();
    let mut other=Store::open(fixture.profile()).unwrap();let mut changed=other.data().clone();changed.settings.bookmark_bar=false;other.save(changed).unwrap();
    let external=std::fs::read(fixture.profile()).unwrap();assert_ne!(external,before);
    app.input_settings_action("Open").unwrap();app.input_settings_action("None").unwrap();app.input_settings_action("Apply").unwrap();
    assert!(app.browser_panel.is_some());assert!(!app.browser_panel.as_ref().unwrap().error.is_empty());
    assert_eq!(app.profile.data().settings.search_engine,SearchEngine::Google);assert_eq!(std::fs::read(fixture.profile()).unwrap(),external);
    app.input_settings_action("Cancel").unwrap();
}

#[test]
fn address_search_none_errors_and_non_chrome_navigation_never_contact_provider() {
    use crate::profile::SearchEngine;
    let fixture=Fixture::new();let mut app=fixture.app("<title>Local</title>");
    app.input_search_provider=Some("http://127.0.0.1:1/search".into());
    for input in ["http://[bad","example.test:99999","C:\\missing\\not-there.html","javascript:alert(1)"] {
        app.submit_address_bar(input);assert!(app.failed,"{input}");assert!(app.pending.is_none());
        assert!(app.network.snapshot().rows.is_empty(),"Explicit invalid input must never search: {input}");
    }
    let mut data=app.profile.data().clone();data.settings.search_engine=SearchEngine::None;app.profile.save(data).unwrap();
    app.submit_address_bar("private ordinary text");assert!(app.failed&&app.pending.is_none());assert!(app.network.snapshot().rows.is_empty());
    app.open_address("cupcake recipes");assert!(app.failed&&app.pending.is_none());assert!(app.network.snapshot().rows.is_empty(),"Programmatic opens do not search");
}
