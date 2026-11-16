//! Original shared-history App/UI lifecycle and stale identity controls.
use super::*;
use crate::history::{Policy, Shared};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!("chezcat-shared-history-app-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap(); Self(root)
    }
    fn history(&self) -> PathBuf { self.0.join("history.txt") }
    fn page(&self, name: &str, title: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, format!("<!doctype html><title>{title}</title><p>Original local history page</p>")).unwrap(); path
    }
    fn shared(&self, policy: Policy) -> Shared {
        let mut store = crate::history::Store::open(self.history()).unwrap();
        store.configure(policy, crate::history::now()).unwrap(); Shared::new(store)
    }
}
impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
fn enabled() -> Policy { Policy {enabled:true,retention_days:None,clear_on_exit:false} }
fn pair(shared: &Shared) -> (App, App) {
    let mut first = App::new(); let mut peer = App::new();
    first.attach_shared_browsing_history(shared).unwrap(); peer.attach_shared_browsing_history(shared).unwrap();
    (first, peer)
}
fn point(app: &mut App, action: Action) -> (i32,i32) {
    app.browser_panel.as_mut().unwrap().focus = action;
    app.ensure_browser_focus_visible();
    let control = app.browser_controls().into_iter().find(|control| control.action == action).unwrap();
    assert!(control.enabled);
    (control.rect.x + control.rect.w/2, control.rect.y + control.rect.h/2)
}
fn click(app: &mut App, action: Action) {
    let (x,y) = point(app, action); app.pointer_down(x,y,false); app.pointer_up(x,y);
}
fn options(app: &mut App) { app.show_history(); click(app, Action::HistoryOptions); }

#[test]
fn shared_history_apps_close_and_drop_do_not_clear_before_final_browser_authority() {
    let fixture = Fixture::new(); let now = crate::history::now();
    let policy = Policy {clear_on_exit:true,..enabled()}; let shared = fixture.shared(policy);
    let one = fixture.page("one.html", "Original one"); let two = fixture.page("two.html", "Original two");
    let (mut first, mut peer) = pair(&shared);
    assert!(first.owned_history.is_none() && peer.owned_history.is_none());
    first.open(&one); peer.open(&two);
    assert!(!first.failed && !peer.failed);
    first.sync_shared_browsing_history().unwrap();
    assert_eq!(first.browsing_history.data().entries.len(), 2);
    let bytes = std::fs::read(fixture.history()).unwrap();
    drop(first);
    assert_eq!(std::fs::read(fixture.history()).unwrap(), bytes);
    peer.finish_browsing_session().unwrap();
    assert!(peer.history_closed);
    assert_eq!(std::fs::read(fixture.history()).unwrap(), bytes);
    assert_eq!(crate::history::Store::open(fixture.history()).unwrap().data().entries.len(), 2);
    assert_eq!(shared.shutdown(now).unwrap(), 2);
    assert!(crate::history::Store::open(fixture.history()).unwrap().data().entries.is_empty());
    // Idempotent final shutdown and ordinary handle destruction cannot clear
    // an independent browser's subsequent real visit.
    let mut other = crate::history::Store::open(fixture.history()).unwrap();
    other.record(now, "Independent later visit", "https://example.com/later").unwrap().unwrap();
    let later = std::fs::read(fixture.history()).unwrap();
    assert_eq!(shared.shutdown(now).unwrap(), 0);
    drop(peer); drop(shared);
    assert_eq!(std::fs::read(fixture.history()).unwrap(), later);
    assert_eq!(crate::history::Store::open(fixture.history()).unwrap().data().entries.len(), 1);
}

#[test]
fn shared_history_apps_record_latest_policy_without_merging_page_session_history() {
    let fixture = Fixture::new(); let shared = fixture.shared(enabled());
    let one = fixture.page("one.html", "Original one"); let two = fixture.page("two.html", "Original two");
    let three = fixture.page("three.html", "Original three"); let (mut first, mut peer) = pair(&shared);
    first.open(&one); peer.open(&two);
    options(&mut peer); click(&mut peer, Action::RecordHistory); click(&mut peer, Action::ApplyHistory);
    assert!(!peer.browsing_history.data().policy.enabled);
    assert!(first.browsing_history.data().policy.enabled, "This control requires a genuinely old App observation");
    let before = first.history.len(); first.open(&three);
    assert!(!first.failed); assert_eq!(first.history.len(), before+1);
    assert_eq!(first.browsing_history.data().entries.len(), 2);
    assert!(!first.browsing_history.data().policy.enabled);
    let peer_page = (peer.location.display(), peer.raw.clone(), peer.history.len(), peer.history_index);
    first.history_move(-1);
    assert_eq!(first.document.title, "Original one");
    assert_eq!((peer.location.display(), peer.raw.clone(), peer.history.len(), peer.history_index), peer_page);
    assert_eq!(crate::history::Store::open(fixture.history()).unwrap().data().entries.len(), 2);
}

#[test]
fn shared_history_app_same_id_replacement_cannot_be_blessed_by_refresh_or_open_press() {
    let fixture = Fixture::new(); let one = fixture.page("original-one.html", "Original one");
    let two = fixture.page("replacement-two.html", "Replacement two");
    let mut store = crate::history::Store::open(fixture.history()).unwrap();
    store.configure(enabled(), crate::history::now()).unwrap();
    let id = store.record(crate::history::now(), "Original selected visit", &navigation::explicit(&one).unwrap().display()).unwrap().unwrap();
    let shared = Shared::new(store); let (mut first, mut peer) = pair(&shared);
    first.show_history(); let expected = first.browser_panel.as_ref().unwrap().history_selected_entry.clone().unwrap();
    assert_eq!(expected.id, id);
    let original = std::fs::read_to_string(fixture.history()).unwrap();
    let replacement = original.replace("Original selected visit", "Replacement selected visit").replace("original-one.html", "replacement-two.html");
    assert_ne!(replacement, original); std::fs::write(fixture.history(), replacement.as_bytes()).unwrap();
    let replaced = crate::history::Store::open(fixture.history()).unwrap();
    assert_eq!(replaced.data().entries[0].id, id);
    assert_eq!(replaced.data().entries[0].url, navigation::explicit(&two).unwrap().display());
    let page = (first.location.display(), first.raw.clone(), first.script_epoch);
    click(&mut first, Action::Open);
    assert_eq!((first.location.display(), first.raw.clone(), first.script_epoch), page);
    assert!(first.browser_panel.as_ref().unwrap().error.contains("changed"));
    assert_eq!(std::fs::read(fixture.history()).unwrap(), replacement.as_bytes());
    // Restore the original fixture, arm Open, then let a peer refresh the same-id
    // replacement before the physical release. The stale gesture is canceled.
    std::fs::write(fixture.history(), original.as_bytes()).unwrap(); shared.refresh(crate::history::now()).unwrap(); first.show_history();
    let (x,y) = point(&mut first, Action::Open); first.pointer_down(x,y,false);
    std::fs::write(fixture.history(), replacement.as_bytes()).unwrap(); peer.refresh_browsing_history().unwrap();
    first.sync_shared_browsing_history().unwrap(); first.pointer_up(x,y);
    let panel = first.browser_panel.as_ref().unwrap();
    assert!(panel.history_selected.is_none() && panel.history_selected_entry.is_none());
    assert_eq!((first.location.display(), first.raw.clone(), first.script_epoch), page);
    first.browser_activate(Action::Delete);
    assert_eq!(std::fs::read(fixture.history()).unwrap(), replacement.as_bytes());
    // A deliberate reopen obtains the replacement's actual identity and can
    // remove it through the unchanged guarded Store transaction.
    first.show_history(); click(&mut first, Action::Delete);
    assert!(crate::history::Store::open(fixture.history()).unwrap().data().entries.is_empty());
}

#[test]
fn shared_history_app_stale_policy_apply_keeps_original_and_edited_fields_after_peer_sync() {
    let fixture = Fixture::new(); let shared = fixture.shared(enabled()); let (mut first, mut peer) = pair(&shared);
    options(&mut first); click(&mut first, Action::ClearOnExit);
    let original = first.browser_panel.as_ref().unwrap().history_original_policy.clone().unwrap();
    let edited = first.browser_panel.as_ref().unwrap().history_policy;
    options(&mut peer); click(&mut peer, Action::Retention(Some(7))); click(&mut peer, Action::RecordHistory); click(&mut peer, Action::ApplyHistory);
    let bytes = std::fs::read(fixture.history()).unwrap(); let peer_policy = peer.browsing_history.data().policy;
    first.sync_shared_browsing_history().unwrap();
    assert_eq!(first.browser_panel.as_ref().unwrap().history_original_policy.as_ref(), Some(&original));
    assert_eq!(original.policy(), enabled()); assert_ne!(first.browsing_history.policy_token(), original);
    click(&mut first, Action::ApplyHistory);
    let draft = first.browser_panel.as_ref().expect("Stale Apply must retain its editor");
    assert_eq!(draft.history_policy, edited); assert_eq!(draft.history_original_policy.as_ref(), Some(&original));
    assert!(draft.error.contains("changed"));
    assert_eq!(std::fs::read(fixture.history()).unwrap(), bytes);
    assert_eq!(crate::history::Store::open(fixture.history()).unwrap().data().policy, peer_policy);
    click(&mut first, Action::Cancel); click(&mut first, Action::HistoryOptions);
    click(&mut first, Action::Retention(Some(90))); click(&mut first, Action::ApplyHistory);
    assert!(!matches!(first.browser_panel.as_ref().map(|panel| panel.kind), Some(PanelKind::HistoryOptions)));
    assert_eq!(crate::history::Store::open(fixture.history()).unwrap().data().policy.retention_days, Some(90));
}

#[test]
fn shared_history_app_external_policy_change_is_refused_inside_real_store_transaction() {
    let fixture = Fixture::new(); let shared = fixture.shared(enabled()); let (mut first, _peer) = pair(&shared);
    options(&mut first); click(&mut first, Action::ClearOnExit);
    let original = first.browser_panel.as_ref().unwrap().history_original_policy.clone().unwrap();
    let draft_policy = first.browser_panel.as_ref().unwrap().history_policy;
    let external = Policy {enabled:false,retention_days:Some(7),clear_on_exit:false};
    crate::history::Store::open(fixture.history()).unwrap().configure(external, crate::history::now()).unwrap();
    let bytes = std::fs::read(fixture.history()).unwrap();
    // Deliberately skip Shared/View refresh: the in-process token still matches.
    assert_eq!(first.browsing_history.policy_token(), original);
    click(&mut first, Action::ApplyHistory);
    let draft = first.browser_panel.as_ref().unwrap();
    assert_eq!(draft.history_original_policy.as_ref(), Some(&original)); assert_eq!(draft.history_policy, draft_policy);
    assert!(draft.error.contains("changed"));
    assert_eq!(std::fs::read(fixture.history()).unwrap(), bytes);
    assert_eq!(crate::history::Store::open(fixture.history()).unwrap().data().policy, external);
}

#[test]
fn shared_history_app_policy_token_ignores_visits_but_rejects_observed_policy_aba() {
    let fixture = Fixture::new(); let shared = fixture.shared(enabled()); let (mut first, mut peer) = pair(&shared);
    options(&mut first); let original = first.browser_panel.as_ref().unwrap().history_original_policy.clone().unwrap();
    peer.browsing_history.record(crate::history::now(), "Original peer visit", "https://example.com/peer").unwrap().unwrap();
    first.sync_shared_browsing_history().unwrap(); assert_eq!(first.browsing_history.policy_token(), original);
    click(&mut first, Action::Retention(Some(7))); click(&mut first, Action::ApplyHistory);
    assert_eq!(first.browsing_history.data().entries.len(), 1);
    first.show_history_options(); let token = first.browser_panel.as_ref().unwrap().history_original_policy.clone().unwrap();
    click(&mut first, Action::ClearOnExit); let edited = first.browser_panel.as_ref().unwrap().history_policy;
    let policy = token.policy(); shared.configure(Policy {enabled:!policy.enabled,..policy}, crate::history::now()).unwrap();
    shared.configure(policy, crate::history::now()).unwrap(); first.sync_shared_browsing_history().unwrap();
    assert_eq!(first.browsing_history.data().policy, token.policy()); assert_ne!(first.browsing_history.policy_token(), token);
    let bytes = std::fs::read(fixture.history()).unwrap(); click(&mut first, Action::ApplyHistory);
    assert_eq!(first.browser_panel.as_ref().unwrap().history_policy, edited);
    assert!(first.browser_panel.as_ref().unwrap().error.contains("changed"));
    assert_eq!(std::fs::read(fixture.history()).unwrap(), bytes);
}

#[test]
fn shared_history_app_weak_policy_command_does_not_keep_a_closed_session_alive() {
    let fixture = Fixture::new(); let shared = fixture.shared(enabled()); let (mut first, peer) = pair(&shared);
    options(&mut first); click(&mut first, Action::ClearOnExit);
    let original = first.browser_panel.as_ref().unwrap().history_original_policy.clone().unwrap();
    let commands = first.history_commands.clone(); let bytes = std::fs::read(fixture.history()).unwrap();
    drop(shared);
    assert!(commands.configure_checked(&original, original.policy(), crate::history::now()).unwrap_err().contains("closed"));
    click(&mut first, Action::ApplyHistory);
    assert!(first.browser_panel.as_ref().unwrap().error.contains("closed"));
    assert_eq!(std::fs::read(fixture.history()).unwrap(), bytes);
    first.key(0x1b,false,false,false);
    assert!(!matches!(first.browser_panel.as_ref().map(|panel| panel.kind), Some(PanelKind::HistoryOptions)));
    drop(first); drop(peer); assert_eq!(std::fs::read(fixture.history()).unwrap(), bytes);
}

#[test]
fn shared_history_unchanged_snapshot_sync_preserves_a_valid_open_press() {
    let fixture = Fixture::new();
    let page = fixture.page("selected.html", "Selected history destination");
    let shared = fixture.shared(enabled());
    let url = navigation::explicit(&page).unwrap().display();
    let mut writer = shared.view().unwrap();
    writer.record(crate::history::now(), "Selected history destination", &url).unwrap().unwrap();
    for maintenance in [false, true] {
        let (mut app, _peer) = pair(&shared);
        app.show_history();
        let (x, y) = point(&mut app, Action::Open);
        app.pointer_down(x, y, false);
        let changed = if maintenance {
            app.refresh_browsing_history_at(crate::history::now()).unwrap()
        } else {
            app.sync_shared_browsing_history().unwrap()
        };
        assert!(!changed);
        app.pointer_up(x, y);
        assert_eq!(app.location.display(), url);
        assert_eq!(app.document.title, "Selected history destination");
        assert!(!app.failed);
    }
}
