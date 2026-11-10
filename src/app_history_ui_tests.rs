//! Native history controls exercise the real App, persistence and input paths.
use super::*;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "own-browser-history-ui-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn history(&self) -> PathBuf {
        self.0.join("history.txt")
    }
    fn app(&self) -> App {
        let mut app = App::new();
        app.attach_profile(Store::open(self.0.join("profile.txt")).unwrap());
        app.attach_browsing_history(crate::history::Store::open(self.history()).unwrap());
        app
    }
    fn page(&self) -> PathBuf {
        let path = self.0.join("page.html");
        std::fs::write(
            &path,
            "<title>Opened saved visit</title><h1>History opened this page</h1>",
        )
        .unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn key(app: &mut App, code: u32) {
    app.key(code, false, false, false);
}
fn shortcut(app: &mut App, code: u32) {
    app.key(code, true, false, false);
}
fn focus(app: &App) -> Action {
    app.browser_panel.as_ref().unwrap().focus
}
fn seek(app: &mut App, action: Action) {
    for _ in 0..20 {
        if focus(app) == action {
            return;
        }
        key(app, 0x09);
    }
    panic!("Native control not in tab order");
}
fn click(app: &mut App, action: Action) {
    seek(app, action);
    let control = app
        .browser_controls()
        .into_iter()
        .find(|control| control.action == action)
        .unwrap();
    assert!(control.enabled);
    let x = control.rect.x + control.rect.w / 2;
    let y = control.rect.y + control.rect.h / 2;
    assert_eq!(app.browser_action_at(x, y), Some(action));
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
}
fn replace(app: &mut App, value: &str) {
    shortcut(app, 0x41);
    assert!(matches!(
        app.key(0x56, true, false, false),
        PlatformAction::Paste
    ));
    app.paste(value);
}
fn enable(app: &mut App) {
    let policy = crate::history::Policy {
        enabled: true,
        retention_days: None,
        clear_on_exit: false,
    };
    app.browsing_history
        .configure(policy, crate::history::now())
        .unwrap();
    app.browsing_history_changed();
}
fn record(app: &mut App, when: u64, title: &str, url: &str) -> u64 {
    app.browsing_history
        .record(when, title, url)
        .unwrap()
        .unwrap()
}
fn shot(app: &App, name: &str) {
    let Some(path) = std::env::var_os("OWN_BROWSER_HISTORY_SHOTS") else {
        return;
    };
    let folder = PathBuf::from(path);
    std::fs::create_dir_all(&folder).unwrap();
    let canvas = app.draw();
    let mut output = std::io::BufWriter::new(std::fs::File::create(folder.join(name)).unwrap());
    write!(output, "P6\n{} {}\n255\n", canvas.width, canvas.height).unwrap();
    for pixel in canvas.pixels {
        output
            .write_all(&[(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8])
            .unwrap();
    }
}

#[test]
fn history_list_search_clipboard_open_and_delete_use_saved_ids() {
    let fixture = Fixture::new();
    let mut app = fixture.app();
    enable(&mut app);
    let now = crate::history::now();
    let target = navigation::explicit(&fixture.page()).unwrap().display();
    let first = record(&mut app, now - 60, "Café reading", &target);
    let second = record(&mut app, now, "Search example", "https://example.com/read");
    shortcut(&mut app, 0x48);
    assert!(matches!(
        app.browser_panel.as_ref().unwrap().kind,
        PanelKind::History
    ));
    assert_eq!(
        app.browser_panel.as_ref().unwrap().history_selected,
        Some(second)
    );
    shot(&app, "history-list.ppm");

    shortcut(&mut app, 0x46);
    replace(&mut app, "CAFÉ");
    shortcut(&mut app, 0x41);
    assert!(matches!(app.key(0x43,true,false,false),PlatformAction::Copy(value) if value=="CAFÉ"));
    key(&mut app, 0x0d);
    assert_eq!(app.history_matches(), vec![first]);
    // Search does not persist a search string in the history file.
    assert!(!std::fs::read_to_string(fixture.history())
        .unwrap()
        .contains("CAFÉ"));
    key(&mut app, 0x28);
    assert_eq!(
        app.browser_panel.as_ref().unwrap().history_selected,
        Some(first)
    );
    key(&mut app, 0x0d);
    assert!(app.browser_panel.is_none());
    assert_eq!(app.document.title, "Opened saved visit");
    assert!(matches!(app.current_request, crate::http::Request::Get));
    assert_eq!(app.location.display(), target);

    shortcut(&mut app, 0x48);
    let before = app.browsing_history.data().entries.len();
    let selected = app
        .browser_panel
        .as_ref()
        .unwrap()
        .history_selected
        .unwrap();
    key(&mut app, 0x2e);
    assert_eq!(app.browsing_history.data().entries.len(), before - 1);
    assert!(!app
        .browsing_history
        .data()
        .entries
        .iter()
        .any(|entry| entry.id == selected));
    assert_eq!(
        crate::history::Store::open(fixture.history())
            .unwrap()
            .data(),
        app.browsing_history.data()
    );
}

#[test]
fn history_options_persist_independently_and_preserve_settings_draft() {
    let fixture = Fixture::new();
    let mut app = fixture.app();
    shortcut(&mut app, 0xbc);
    key(&mut app, 0x20); // Draft JavaScript off; not applied yet.
    assert!(!app.browser_panel.as_ref().unwrap().javascript);
    click(&mut app, Action::HistoryOptions);
    assert!(!app.browsing_history.data().policy.enabled);
    click(&mut app, Action::RecordHistory);
    click(&mut app, Action::Retention(Some(7)));
    click(&mut app, Action::ClearOnExit);
    shot(&app, "history-settings.ppm");
    click(&mut app, Action::ApplyHistory);
    assert!(matches!(
        app.browser_panel.as_ref().unwrap().kind,
        PanelKind::Settings
    ));
    assert!(!app.browser_panel.as_ref().unwrap().javascript);
    assert!(app.scripts_enabled); // The general Settings draft is still unapplied.
    assert!(!fixture.0.join("profile.txt").exists());
    let expected = crate::history::Policy {
        enabled: true,
        retention_days: Some(7),
        clear_on_exit: true,
    };
    assert_eq!(app.browsing_history.data().policy, expected);
    assert_eq!(
        crate::history::Store::open(fixture.history())
            .unwrap()
            .data()
            .policy,
        expected
    );

    let bytes = std::fs::read(fixture.history()).unwrap();
    click(&mut app, Action::HistoryOptions);
    click(&mut app, Action::RecordHistory);
    key(&mut app, 0x1b);
    assert_eq!(app.browsing_history.data().policy, expected);
    assert_eq!(std::fs::read(fixture.history()).unwrap(), bytes);
    assert!(!app.browser_panel.as_ref().unwrap().javascript);
    key(&mut app, 0x1b);
    assert!(app.browser_panel.is_none());

    // Disabling stops new records and leaves older visits viewable.
    let now = crate::history::now();
    record(&mut app, now, "Earlier visit", "https://example.com/");
    app.show_history();
    click(&mut app, Action::HistoryOptions);
    click(&mut app, Action::RecordHistory);
    click(&mut app, Action::ApplyHistory);
    assert!(!app.browsing_history.data().policy.enabled);
    assert_eq!(app.browsing_history.data().entries.len(), 1);
    assert!(app
        .browsing_history
        .record(now, "Not saved", "https://example.com/private")
        .unwrap()
        .is_none());
    assert_eq!(app.browsing_history.data().entries.len(), 1);
}

#[test]
fn clear_confirmation_applies_only_the_selected_time_range() {
    let fixture = Fixture::new();
    let mut app = fixture.app();
    enable(&mut app);
    let now = crate::history::now();
    let old = record(
        &mut app,
        now - 8 * 86400,
        "Older",
        "https://example.com/old",
    );
    let day = record(
        &mut app,
        now - 7200,
        "Earlier today",
        "https://example.com/day",
    );
    let recent = record(&mut app, now - 300, "Recent", "https://example.com/recent");
    let mut data = app.profile.data().clone();
    data.bookmarks.push(Bookmark {
        title: "Keep bookmark".into(),
        url: "https://example.com/".into(),
    });
    app.profile.save(data.clone()).unwrap();
    let profile_bytes = std::fs::read(fixture.0.join("profile.txt")).unwrap();
    let back_forward = app.history.len();
    app.show_history();
    click(&mut app, Action::HistoryClear);
    shot(&app, "history-clear.ppm");
    click(&mut app, Action::Cancel);
    assert_eq!(app.browsing_history.data().entries.len(), 3);
    click(&mut app, Action::HistoryClear);
    click(&mut app, Action::ConfirmClear); // Default: last hour.
    assert_eq!(
        app.browsing_history
            .data()
            .entries
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        vec![day, old]
    );
    assert!(!app
        .browsing_history
        .data()
        .entries
        .iter()
        .any(|entry| entry.id == recent));
    click(&mut app, Action::HistoryClear);
    click(
        &mut app,
        Action::ClearRange(crate::history::ClearRange::All),
    );
    click(&mut app, Action::ConfirmClear);
    assert!(app.browsing_history.data().entries.is_empty());
    assert_eq!(app.history.len(), back_forward);
    assert_eq!(app.profile.data(), &data);
    assert_eq!(
        std::fs::read(fixture.0.join("profile.txt")).unwrap(),
        profile_bytes
    );
    assert!(app.browsing_history.data().policy.enabled);
}

#[test]
fn automatic_refresh_never_retargets_a_delete_to_a_different_visit() {
    let fixture = Fixture::new();
    let mut app = fixture.app();
    enable(&mut app);
    let now = crate::history::now();
    let oldest = record(&mut app, now, "Oldest visit", "https://example.com/oldest");
    let survivor = record(&mut app, now, "Other visit", "https://example.com/other");
    let selected = record(
        &mut app,
        now,
        "Selected visit",
        "https://example.com/selected",
    );
    app.show_history();
    assert_eq!(
        app.browser_panel.as_ref().unwrap().history_selected,
        Some(selected)
    );
    let mut other = crate::history::Store::open(fixture.history()).unwrap();
    other.remove(selected, now).unwrap();
    app.refresh_browsing_history().unwrap();
    assert_eq!(app.browser_panel.as_ref().unwrap().history_selected, None);
    key(&mut app, 0x41); // Unrelated keys must not silently select another visit.
    key(&mut app, 0x0d);
    assert_eq!(app.browser_panel.as_ref().unwrap().history_selected, None);
    key(&mut app, 0x2e);
    assert_eq!(
        app.browsing_history
            .data()
            .entries
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        vec![survivor, oldest]
    );
    key(&mut app, 0x28);
    assert_eq!(
        app.browser_panel.as_ref().unwrap().history_selected,
        Some(survivor)
    );
    key(&mut app, 0x2e);
    assert_eq!(app.browser_panel.as_ref().unwrap().history_selected, None);
    key(&mut app, 0x26);
    assert_eq!(
        app.browser_panel.as_ref().unwrap().history_selected,
        Some(oldest)
    );
    key(&mut app, 0x2e);
    assert!(app.browsing_history.data().entries.is_empty());
}

#[test]
fn compact_history_controls_scroll_and_corrupt_file_requires_confirmed_all() {
    let fixture = Fixture::new();
    let mut app = fixture.app();
    enable(&mut app);
    let now = crate::history::now();
    for index in 0..20 {
        record(
            &mut app,
            now,
            "Saved page",
            &format!("https://example.com/page{index}"),
        );
    }
    app.resize(420, 260);
    app.show_history();
    key(&mut app, 0x23);
    assert!(app.browser_panel.as_ref().unwrap().first > 0);
    shot(&app, "history-small.ppm");
    app.wheel(120, false);
    click(&mut app, Action::HistorySearch);
    replace(&mut app, "page2");
    shot(&app, "history-search-small.ppm");
    key(&mut app, 0x0d);
    assert_eq!(app.history_matches().len(), 1);
    click(&mut app, Action::HistoryOptions);
    click(&mut app, Action::Retention(Some(90)));
    click(&mut app, Action::ClearOnExit);
    shot(&app, "history-settings-small.ppm");
    click(&mut app, Action::ApplyHistory);
    assert_eq!(app.browsing_history.data().policy.retention_days, Some(90));

    std::fs::write(fixture.history(), b"corrupt file\n").unwrap();
    app.show_history();
    assert!(!app.browser_panel.as_ref().unwrap().error.is_empty());
    click(&mut app, Action::HistoryClear);
    click(
        &mut app,
        Action::ClearRange(crate::history::ClearRange::All),
    );
    click(&mut app, Action::Cancel);
    assert_eq!(std::fs::read(fixture.history()).unwrap(), b"corrupt file\n");
    click(&mut app, Action::HistoryClear);
    click(
        &mut app,
        Action::ClearRange(crate::history::ClearRange::All),
    );
    click(&mut app, Action::ConfirmClear);
    assert!(app.browsing_history.data().entries.is_empty());
    assert!(!app.browsing_history.data().policy.enabled);
    assert!(app.browser_panel.as_ref().unwrap().error.is_empty());
    assert!(crate::history::Store::open(fixture.history()).is_ok());
}

#[test]
fn utc_history_date_covers_epoch_leap_day_and_range() {
    assert_eq!(history_date(0), "1970-01-01 00:00 UTC");
    assert_eq!(history_date(951_827_696), "2000-02-29 12:34 UTC");
    assert_eq!(history_date(253_402_300_799), "9999-12-31 23:59 UTC");
    assert_eq!(history_date(u64::MAX), "Date outside display range");
}
