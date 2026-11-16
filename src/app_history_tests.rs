//! Saved-history lifecycle checks through the real App navigation paths.
use super::*;
use crate::history::{ClearRange, Policy, Store};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-history-app-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }
    fn history(&self) -> PathBuf {
        self.0.join("history.txt")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn enabled_store() -> Store {
    let mut store = Store::memory();
    store
        .configure(
            Policy {
                enabled: true,
                retention_days: None,
                clear_on_exit: false,
            },
            crate::history::now(),
        )
        .unwrap();
    store
}

#[test]
fn local_visits_record_documents_but_not_startup_fragments_or_failed_targets() {
    let fixture = Fixture::new();
    let home = fixture.file("start.html", "<title>Start</title><p>Home</p>");
    let one = fixture.file(
        "one.html",
        "<title>One</title><p id='anchor'>One</p><script>document.title='Script title';</script>",
    );
    let two = fixture.file("two.html", "<title>Two</title><p>Two</p>");
    let mut app = App::new();
    app.attach_browsing_history(enabled_store());
    app.use_home_file(home);
    assert!(app.browsing_history.data().entries.is_empty());
    app.open(&one);
    assert_eq!(app.browsing_history.data().entries.len(), 1);
    assert_eq!(app.browsing_history.data().entries[0].title, "Script title");
    assert_eq!(
        app.browsing_history.data().entries[0].url,
        app.location.display()
    );
    app.follow("#anchor");
    app.show_source();
    app.show_source();
    app.show_resources();
    app.show_resources();
    assert_eq!(app.browsing_history.data().entries.len(), 1);
    app.reload();
    assert_eq!(app.browsing_history.data().entries.len(), 2);
    app.open(&fixture.0.join("does-not-exist.html"));
    assert!(app.failed);
    assert_eq!(app.browsing_history.data().entries.len(), 2);
    app.open(&two);
    assert_eq!(app.browsing_history.data().entries.len(), 3);
    app.history_move(-1);
    assert_eq!(app.browsing_history.data().entries.len(), 4);
    app.history_move(-1); // Same document, earlier fragment position.
    assert_eq!(app.browsing_history.data().entries.len(), 4);
    let mut policy = app.browsing_history.data().policy;
    policy.enabled = false;
    app.configure_browsing_history(&app.browsing_history.policy_token(), policy, crate::history::now())
        .unwrap();
    app.open(&two);
    assert_eq!(app.browsing_history.data().entries.len(), 4);
    let session_entries = app.history.len();
    app.browsing_history
        .clear(ClearRange::All, crate::history::now())
        .unwrap();
    assert!(app.browsing_history.data().entries.is_empty());
    assert_eq!(app.history.len(), session_entries);
    app.history_move(-1);
    assert_eq!(app.document.title, "Script title");
    assert!(app.browsing_history.data().entries.is_empty());
}

#[test]
fn age_cleanup_runs_with_recording_and_javascript_disabled_and_no_page_activity() {
    let now = crate::history::now();
    let old = now.saturating_sub(86_401);
    let mut app = App::new();
    app.set_javascript_enabled(false);
    let mut policy = Policy {
        enabled: true,
        retention_days: Some(1),
        clear_on_exit: false,
    };
    app.configure_browsing_history(&app.browsing_history.policy_token(), policy, old).unwrap();
    app.browsing_history
        .record(old, "Expired", "https://example.com/old")
        .unwrap();
    policy.enabled = false;
    app.configure_browsing_history(&app.browsing_history.policy_token(), policy, old).unwrap();
    app.browsing_history_changed();
    assert!(app.next_history_maintenance().is_some());
    app.history_maintenance = Some(std::time::Instant::now());
    assert!(app.poll_history_maintenance());
    assert!(app.browsing_history.data().entries.is_empty());
    assert!(app.next_history_maintenance().is_none());
    assert!(app.next_script_timer().is_none());
    assert!(!app.poll_history_maintenance());
}

#[test]
fn startup_expires_old_entries_and_shutdown_uses_latest_policy_without_bookmark_changes() {
    let fixture = Fixture::new();
    let now = crate::history::now();
    let old = now.saturating_sub(86_401);
    let mut store = Store::open(fixture.history()).unwrap();
    let policy = Policy {
        enabled: true,
        retention_days: Some(1),
        clear_on_exit: false,
    };
    store.configure(policy, old).unwrap();
    store
        .record(old, "Expired", "https://example.com/expired")
        .unwrap();
    let mut app = App::new();
    app.attach_browsing_history(Store::open(fixture.history()).unwrap());
    assert!(app.browsing_history.data().entries.is_empty());
    let page = fixture.file("page.html", "<title>Current</title><script>setTimeout(function(){document.title='late';},1000);</script>");
    app.open(&page);
    assert_eq!(app.browsing_history.data().entries.len(), 1);
    assert!(app.next_script_timer().is_some());
    let profile = fixture.file(
        "profile.txt",
        "leave unrelated bookmarks and settings unchanged",
    );
    let original = std::fs::read(&profile).unwrap();
    let mut other = Store::open(fixture.history()).unwrap();
    other
        .configure(
            Policy {
                clear_on_exit: true,
                ..policy
            },
            now,
        )
        .unwrap();
    app.finish_browsing_session().unwrap();
    assert!(app.scripting.is_none());
    assert!(app.next_history_maintenance().is_none());
    assert!(Store::open(fixture.history())
        .unwrap()
        .data()
        .entries
        .is_empty());
    assert_eq!(std::fs::read(&profile).unwrap(), original);
    let saved = std::fs::read(fixture.history()).unwrap();
    app.finish_browsing_session().unwrap();
    assert_eq!(std::fs::read(fixture.history()).unwrap(), saved);
}

#[test]
fn shutdown_write_failure_is_reported_and_a_retry_can_clear_history() {
    let fixture = Fixture::new();
    let now = crate::history::now();
    let mut store = Store::open(fixture.history()).unwrap();
    store
        .configure(
            Policy {
                enabled: true,
                retention_days: None,
                clear_on_exit: true,
            },
            now,
        )
        .unwrap();
    store.record(now, "Saved", "https://example.com/").unwrap();
    let mut app = App::new();
    app.attach_browsing_history(store);
    let before = std::fs::read(fixture.history()).unwrap();
    let lock = fixture.file("history.txt.lock", "busy");
    assert!(app.finish_browsing_session().is_err());
    assert!(!app.history_closed);
    assert!(app.browsing_history_error.is_some());
    assert_eq!(std::fs::read(fixture.history()).unwrap(), before);
    std::fs::remove_file(lock).unwrap();
    app.finish_browsing_session().unwrap();
    assert!(Store::open(fixture.history())
        .unwrap()
        .data()
        .entries
        .is_empty());
}
