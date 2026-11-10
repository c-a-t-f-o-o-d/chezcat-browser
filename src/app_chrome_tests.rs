//! Browser-chrome input checks: real local navigation, cancellation and keyboard
//! ownership. These deliberately avoid network services and persistent profiles.
use super::chrome::ChromeAction;
use super::*;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-chrome-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str, markup: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, markup).unwrap();
        path
    }
    fn app(&self) -> App {
        let mut app = App::new();
        app.open(&self.file(
            "home.html",
            "<title>My home</title><h1>Home</h1><p>target target</p>",
        ));
        let mut data = app.profile.data().clone();
        data.settings.home = Some(app.location.display());
        app.profile.save(data).unwrap();
        app
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn center(app: &App, action: ChromeAction) -> (i32, i32) {
    let (x, y, w, h) = app.chrome_rect(action).expect("visible chrome action");
    (x + w / 2, y + h / 2)
}
fn click(app: &mut App, action: ChromeAction) -> PlatformAction {
    let (x, y) = center(app, action);
    assert!(matches!(
        app.pointer_down(x, y, false),
        PlatformAction::None
    ));
    // WM_PAINT refreshes the known client size between press and release.
    // Repainting at the same size must preserve the pressed button.
    app.resize(app.width, app.height);
    let _paint = app.draw();
    app.pointer_up(x, y)
}
fn shot(app: &App, name: &str) {
    let Some(folder) = std::env::var_os("OWN_BROWSER_CHROME_SHOTS") else {
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
fn toolbar_icons_navigate_and_refresh_the_displayed_local_document() {
    let fixture = Fixture::new();
    let mut app = fixture.app();
    let original = app.location.display();
    let next = fixture.file("next.html", "<title>Second page</title><h1>Second</h1>");
    app.open(&next);
    assert_eq!(app.document.title, "Second page");

    click(&mut app, ChromeAction::Back);
    assert_eq!(app.location.display(), original);
    assert_eq!(app.document.title, "My home");
    click(&mut app, ChromeAction::Forward);
    assert_eq!(app.document.title, "Second page");
    assert!(!app.chrome_enabled(ChromeAction::Forward));

    // Model a Back navigation that has not committed yet. Forward must let
    // the user return to the still-displayed page while that load is pending.
    let previous = app.history_index - 1;
    app.pending = Some(Pending {
        id: app.next_request,
        location: app.history[previous].location.clone(),
        kind: NavigationKind::History(previous, 0),
        mode: crate::http::FetchMode::Normal,
        request: app.history[previous].request.clone(),
        committed: false,
        scroll_target: None,
    });
    assert!(app.chrome_enabled(ChromeAction::Forward));
    click(&mut app, ChromeAction::Forward);
    assert!(app.pending.is_none());
    assert_eq!(app.document.title, "Second page");
    assert!(!app.chrome_enabled(ChromeAction::Forward));

    fixture.file(
        "next.html",
        "<title>Updated on disk</title><h1>Updated</h1>",
    );
    let visits = app.history.len();
    click(&mut app, ChromeAction::Reload);
    assert_eq!(app.document.title, "Updated on disk");
    assert_eq!(
        app.history.len(),
        visits,
        "reload does not add a navigation"
    );
    click(&mut app, ChromeAction::Home);
    assert_eq!(app.location.display(), original);
    assert_eq!(app.document.title, "My home");
    assert_eq!(
        app.profile.data().settings.home.as_deref(),
        Some(original.as_str())
    );
}

#[test]
fn chrome_clicks_require_matching_release_and_cancel_without_navigation() {
    let fixture = Fixture::new();
    let mut app = fixture.app();
    app.open(&fixture.file("next.html", "<title>Stay here</title>"));
    let location = app.location.display();
    let (x, y) = center(&app, ChromeAction::Home);
    app.pointer_down(x, y, false);
    assert_eq!(
        app.location.display(),
        location,
        "press alone must not navigate"
    );
    app.pointer_up(x, BAR_HEIGHT + 20);
    assert_eq!(
        app.location.display(),
        location,
        "release outside cancels the action"
    );

    app.pointer_down(x, y, false);
    app.pointer_cancel();
    app.pointer_up(x, y);
    assert_eq!(
        app.location.display(),
        location,
        "capture loss cancels the action"
    );

    app.pointer_down(x, y, false);
    let other = center(&app, ChromeAction::Reload);
    app.pointer_up(other.0, other.1);
    assert_eq!(app.document.title, "Stay here");
    assert_eq!(app.location.display(), location);

    click(&mut app, ChromeAction::Menu);
    let (x, y) = center(&app, ChromeAction::MenuItem(0));
    assert!(matches!(
        app.pointer_down(x, y, false),
        PlatformAction::None
    ));
    assert!(app.menu, "the menu remains open while a row is pressed");
    assert!(matches!(app.pointer_up(x, y), PlatformAction::OpenFile));
    assert!(!app.menu);
}

#[test]
fn compact_menu_scrolls_without_scrolling_the_page_and_reaches_history() {
    let fixture = Fixture::new();
    let mut app = fixture.app();
    app.resize(420, 260);
    click(&mut app, ChromeAction::Menu);
    assert!(app.menu);
    assert!(app.chrome_rect(ChromeAction::MenuItem(12)).is_none());
    let page_scroll = app.scroll;
    shot(&app, "menu-small-top.ppm");
    for _ in 0..13 {
        if app.chrome_rect(ChromeAction::MenuItem(12)).is_some() {
            break;
        }
        app.wheel(-120, false);
    }
    assert_eq!(
        app.scroll, page_scroll,
        "wheel input belongs to the open menu"
    );
    assert!(app.chrome_rect(ChromeAction::MenuItem(0)).is_none());
    let (_, y, _, h) = app.chrome_rect(ChromeAction::MenuItem(12)).unwrap();
    assert!(y >= BAR_HEIGHT && y + h <= app.height - FOOT_HEIGHT);
    shot(&app, "menu-small-bottom.ppm");
    click(&mut app, ChromeAction::MenuItem(12));
    assert!(!app.menu);
    assert!(app.browser_panel.is_some());
    shot(&app, "menu-small-history.ppm");

    // Pointer selection of the History menu row must reach the same History
    // screen as the existing Ctrl+H command, not merely open any native panel.
    let via_menu = app.draw().pixels;
    app.key(0x1b, false, false, false);
    app.key(0x48, true, false, false);
    assert!(
        app.draw().pixels == via_menu,
        "the History menu action must display the History panel"
    );
}

#[test]
fn toolbar_and_menu_keyboard_focus_own_their_keys() {
    let fixture = Fixture::new();
    let mut app = fixture.app();
    app.open(&fixture.file("keys.html", "<title>Keys stay in chrome</title><input id=q value=unchanged><script>document.addEventListener('keydown',function(){document.title='Page key fired';});</script>"));
    app.key(0x4c, true, false, false); // Ctrl+L
    app.key(0x09, false, true, false); // Shift+Tab, to Home
    assert!(app.chrome.focus == Some(ChromeAction::Home));
    app.key(0x41, false, false, false); // Letter A belongs to the focused icon.
    assert_eq!(app.document.title, "Keys stay in chrome");
    app.key(0x20, false, false, false);
    assert_eq!(app.document.title, "My home");

    app.key(0x46, false, false, true); // Alt+F
    assert!(app.menu);
    app.key(0x23, false, false, false); // End
    assert_eq!(app.chrome.menu_selected, Some(13));
    assert!(app.chrome_rect(ChromeAction::MenuItem(13)).is_some());
    app.key(0x24, false, false, false); // Home
    assert_eq!(app.chrome.menu_selected, Some(0));
    assert!(matches!(
        app.key(0x0d, false, false, false),
        PlatformAction::OpenFile
    ));
    assert!(!app.menu);

    app.key(0x46, false, false, true);
    app.key(0x1b, false, false, false);
    assert!(!app.menu);
    assert!(app.chrome.focus == Some(ChromeAction::Menu));
    app.key(0x0d, false, false, false);
    assert!(app.menu, "keyboard focus can reopen the menu");
    shot(&app, "menu-keyboard.ppm");
    assert_eq!(app.document.title, "My home");
}

#[test]
fn hover_leave_and_disabled_navigation_do_not_capture_page_input() {
    let fixture = Fixture::new();
    let mut app = fixture.app();
    let back = center(&app, ChromeAction::Back);
    // The initial about:home entry may precede the first opened local file;
    // reach its beginning before testing the disabled affordance.
    while app.chrome_enabled(ChromeAction::Back) {
        click(&mut app, ChromeAction::Back);
    }
    assert!(!app.wants_hand(back.0, back.1));
    let location = app.location.display();
    click(&mut app, ChromeAction::Back);
    assert_eq!(app.location.display(), location);
    assert!(!app.wants_hand(0, 0), "empty toolbar space is not a button");

    let home = center(&app, ChromeAction::Home);
    app.pointer_move(home.0, home.1, false);
    assert!(app.chrome.hover == Some(ChromeAction::Home));
    shot(&app, "toolbar-home-hover.ppm");
    assert!(app.pointer_leave());
    assert!(app.chrome.hover.is_none());
    assert!(app.hover.is_none());
    app.pointer_move(home.0, home.1, false);
    app.pointer_down(home.0, home.1, false);
    app.pointer_cancel();
    assert!(app.chrome.pressed.is_none());
    assert!(app.chrome.hover.is_none());

    // Native panels cover the find bar. Hidden find buttons must not expose
    // their hover/tooltip over the panel's own header controls.
    app.search("target");
    let hidden_find = center(&app, ChromeAction::FindClose);
    app.show_history();
    app.pointer_move(hidden_find.0, hidden_find.1, false);
    assert!(
        app.chrome.hover.is_none(),
        "covered find controls must not be hit"
    );
    app.key(0x1b, false, false, false);

    // Once focus returns to the address field, ordinary text editing keeps its
    // original meaning rather than triggering focused icon commands.
    app.key(0x4c, true, false, false);
    app.paste("https://example.test/a path");
    app.key(0x24, false, false, false);
    app.text_input('x');
    assert!(app.focus == Focus::Address);
    assert_eq!(app.address.text(), "xhttps://example.test/a path");
    assert_eq!(app.location.display(), location);
}

#[test]
fn replacing_a_long_address_by_clicking_shows_the_first_character_after_repaint() {
    let fixture = Fixture::new();
    let mut app = fixture.app();
    app.resize(480, 360);
    let location = app.location.display();
    let (left, width) = app.address_rect();
    assert!(location.chars().count() > ((width - 16) / 12) as usize);
    let (x, y) = (left + 30, 34);
    app.pointer_down(x, y, false);
    app.resize(app.width, app.height);
    let _paint = app.draw();
    app.pointer_up(x, y);
    assert!(app.focus == Focus::Address);
    assert_eq!(app.address.selected(), location);
    assert!(app.address.offset > 0, "selected long URL starts scrolled");

    let mut expected = String::new();
    for ch in "google.com".chars() {
        app.text_input(ch);
        expected.push(ch);
        // Match the normal Windows WM_CHAR -> repaint -> resize path.
        app.resize(app.width, app.height);
        let _paint = app.draw();
        if expected == "g" {
            shot(&app, "address-first-character.ppm");
        }
        assert_eq!(
            app.address.text(),
            expected,
            "typing preserves the actual address"
        );
        assert_eq!(
            app.address.offset, 0,
            "painting must not hide the first character"
        );
    }
    shot(&app, "address-replacement.ppm");
    assert_eq!(
        navigation::address(&app.address.text(), &app.location)
            .unwrap()
            .display(),
        "https://google.com/",
        "the complete visible hostname is the hostname navigation receives"
    );
    app.key(0x41, true, false, false);
    assert_eq!(app.address.selected(), "google.com");
    app.pointer_down(left + 8, y, false);
    app.resize(app.width, app.height);
    app.pointer_up(left + 8, y);
    assert_eq!(
        app.address.cursor, 0,
        "the visible start is also the editing start"
    );
    app.text_input('x');
    assert_eq!(app.address.text(), "xgoogle.com");
    assert_eq!(
        app.location.display(),
        location,
        "editing alone does not navigate"
    );
}
