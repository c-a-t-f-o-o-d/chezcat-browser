//! Original two-App profile/editor identity regressions. No tabs or OS input.
use super::*;
use crate::profile::{Data, Shared};
use std::sync::atomic::{AtomicUsize, Ordering};

fn pair(shared: &Shared) -> (App, App) {
    let mut first = App::new();
    let mut second = App::new();
    first.attach_shared_profile(shared.clone()).unwrap();
    second.attach_shared_profile(shared.clone()).unwrap();
    (first, second)
}

fn click(app: &mut App, action: Action) {
    let control_action = if matches!(action, Action::RuleRow(_)) { Action::RuleList } else { action };
    if let Some(panel) = &mut app.browser_panel { panel.focus = control_action; }
    app.ensure_browser_focus_visible();
    let control = app.browser_controls().into_iter().find(|control| control.action == control_action).unwrap();
    assert!(control.enabled);
    let x = control.rect.x + control.rect.w / 2;
    let y = if let Action::RuleRow(index) = action {
        let first = app.browser_panel.as_ref().unwrap().first;
        control.rect.y + (index - first) as i32 * ROW + ROW / 2
    } else { control.rect.y + control.rect.h / 2 };
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
}

#[test]
fn shared_profile_app_stale_settings_after_refresh_keep_nested_draft_and_session() {
    let shared = Shared::default();
    let (mut first, mut second) = pair(&shared);
    first.show_settings();
    let original = first.browser_panel.as_ref().unwrap().profile_revision;
    click(&mut first, Action::JavaScript);
    click(&mut first, Action::BlockingOptions);
    assert_eq!(first.browser_panel.as_ref().unwrap().profile_revision, original);
    first.browser_panel.as_mut().unwrap().address.set("ads.example.com");
    click(&mut first, Action::RuleAdd);
    assert_eq!(first.browser_panel.as_ref().unwrap().blocking.rules.len(), 1);
    second.show_settings();
    click(&mut second, Action::DarkMode);
    click(&mut second, Action::Apply);
    assert!(second.browser_panel.is_none());
    let committed = shared.snapshot().unwrap();
    let before = (first.raw.clone(), first.script_epoch, first.history_index,
        first.history.len(), first.scripts_enabled, first.images_enabled, first.zoom);
    assert!(first.refresh_profile().unwrap());
    assert_eq!(first.browser_panel.as_ref().unwrap().profile_revision, original);
    // These mutate only the local draft and remain usable while its save token
    // is stale; eventual full Settings publication must still refuse.
    click(&mut first, Action::RuleRow(0));
    click(&mut first, Action::RuleDelete);
    assert!(first.browser_panel.as_ref().unwrap().blocking.rules.is_empty());
    first.browser_panel.as_mut().unwrap().address.set("ads.example.com");
    click(&mut first, Action::RuleAdd);
    click(&mut first, Action::ApplyBlocking);
    let draft = first.browser_panel.as_ref().unwrap();
    assert!(matches!(draft.kind, PanelKind::Settings));
    assert_eq!(draft.profile_revision, original);
    assert!(!draft.javascript);
    assert_eq!(draft.blocking.rules.len(), 1);
    click(&mut first, Action::Apply);
    let draft = first.browser_panel.as_ref().expect("A stale Apply must retain the editor");
    assert_eq!(draft.profile_revision, original);
    assert!(!draft.javascript);
    assert_eq!(draft.blocking.rules.len(), 1);
    assert!(draft.error.contains("changed"));
    assert_eq!(shared.snapshot().unwrap().data(), committed.data());
    assert_eq!((first.raw.clone(), first.script_epoch, first.history_index,
        first.history.len(), first.scripts_enabled, first.images_enabled, first.zoom), before);
    // An explicitly reopened editor owns a fresh token and can commit normally.
    first.show_settings();
    assert_eq!(first.browser_panel.as_ref().unwrap().profile_revision, Some(committed.revision()));
    click(&mut first, Action::JavaScript);
    click(&mut first, Action::Apply);
    assert!(first.browser_panel.is_none());
    assert!(!shared.snapshot().unwrap().data().settings.javascript);
}

fn bookmarks() -> Data {
    let mut data = Data::default();
    for (title, url) in [("First original", "about:home#one"), ("Second original", "about:home#two")] {
        data.bookmarks.push(Bookmark { title: title.into(), url: url.into(),
            order: data.bookmarks.len() as u32, ..Bookmark::default() });
    }
    data
}

#[test]
fn shared_profile_app_still_valid_bookmark_index_cannot_edit_or_open_peer_reorder() {
    let shared = Shared::default();
    shared.view().unwrap().save(bookmarks()).unwrap();
    let (mut first, mut second) = pair(&shared);
    first.show_bookmarks();
    first.browser_activate(Action::Row(1));
    click(&mut first, Action::Edit);
    first.browser_panel.as_mut().unwrap().title.set("Kept original edit draft");
    let original = first.browser_panel.as_ref().unwrap().profile_revision;
    let mut peer = second.profile.data().clone();
    peer.bookmarks.swap(0, 1);
    second.profile.save(peer.clone()).unwrap();
    assert!(first.refresh_profile().unwrap());
    click(&mut first, Action::SaveBookmark);
    let editor = first.browser_panel.as_ref().unwrap();
    assert_eq!(editor.title.text(), "Kept original edit draft");
    assert_eq!(editor.profile_revision, original);
    assert!(editor.error.contains("changed"));
    assert_eq!(shared.snapshot().unwrap().data(), &peer);
    first.show_bookmarks();
    first.browser_activate(Action::Row(1));
    let location = first.location.display();
    let raw = first.raw.clone();
    let mut newer = second.profile.data().clone();
    newer.bookmarks.swap(0, 1);
    second.profile.save(newer.clone()).unwrap();
    // No View refresh first: an old still-in-range selected index is not authority.
    click(&mut first, Action::Open);
    assert_eq!(first.location.display(), location);
    assert_eq!(first.raw, raw);
    assert!(first.browser_panel.as_ref().unwrap().error.contains("changed"));
    assert_eq!(shared.snapshot().unwrap().data(), &newer);
    first.show_bookmarks();
    first.browser_activate(Action::Row(1));
    click(&mut first, Action::Open);
    assert_eq!(first.location.fragment, "two");
}

#[test]
fn shared_profile_app_peer_refresh_preserves_actual_vm_form_history_and_source() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let folder = std::env::temp_dir().join(format!("chezcat-shared-app-{}-{}",
        std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir(&folder).unwrap();
    let source = folder.join("page.html");
    std::fs::write(&source, "<!doctype html><title>Original live page</title><input id=field><button id=probe type=button>Probe retained VM</button><a href=#paint>Original paint link</a><div style='height:1600px'>Original tall content</div><script>var retained=17;document.getElementById('probe').onclick=function(){if(retained!==17)throw 'retained state';document.title='Original VM retained';};</script>").unwrap();
    let shared = Shared::default();
    let (mut first, mut second) = pair(&shared);
    first.resize(640, 480);
    first.open(&source);
    assert!(!first.failed, "{}", first.status);
    assert!(!first.is_loading());
    assert_eq!(first.scripting.as_ref().unwrap().error_count(), 0);
    let id = |app: &App, name: &str| (0..app.document.nodes.len()).find(|&node| app.document.attr(node, "id") == Some(name)).unwrap();
    let field = id(&first, "field");
    let rect = first.page.controls.iter().find(|rect| rect.node == field).unwrap();
    let (x,y) = (rect.x + rect.width/2, first.content_top() + rect.y + rect.height/2);
    first.pointer_down(x,y,false); first.pointer_up(x,y); first.paste("Retained original text");
    assert_eq!(first.forms.controls[&field].value.text(), "Retained original text");
    first.scroll = 73;
    let nodes = first.document.nodes.as_ptr();
    let vm = first.scripting.as_ref().unwrap() as *const _;
    let control = &first.forms.controls[&field] as *const _;
    let runs = first.page.runs.as_ptr();
    let history = first.history.as_ptr();
    let focus = first.focus;
    let viewport = (first.content_top(), first.viewport_height(), first.document.media_environment);
    let before = (first.source.clone(), first.raw.clone(), first.location.display(), first.script_epoch,
        first.history_index, first.history.len(), first.scroll, first.scripts_enabled, first.images_enabled, first.zoom);
    let events = first.events.clone();
    let mut peer = second.profile.data().clone();
    peer.settings.javascript = false; peer.settings.images = false;
    peer.settings.default_zoom = 150; peer.settings.dark_mode = true;
    peer.settings.bookmark_bar = false;
    peer.settings.link_color = Some(0x112233); peer.settings.visited_color = Some(0x112233);
    second.profile.save(peer).unwrap();
    assert!(first.refresh_profile().unwrap());
    assert_eq!(first.document.nodes.as_ptr(), nodes);
    assert_eq!(first.scripting.as_ref().unwrap() as *const _, vm);
    assert_eq!(&first.forms.controls[&field] as *const _, control);
    assert_eq!(first.page.runs.as_ptr(), runs);
    assert_eq!(first.history.as_ptr(), history);
    assert!(first.focus == focus);
    assert!(first.bookmark_bar_visible);
    assert_eq!((first.content_top(), first.viewport_height(), first.document.media_environment), viewport);
    assert!(!first.link_paint.is_empty());
    assert!(first.link_paint.values().all(|&color| color == 0x112233));
    assert_eq!(first.forms.controls[&field].value.text(), "Retained original text");
    assert_eq!((first.source.clone(), first.raw.clone(), first.location.display(), first.script_epoch,
        first.history_index, first.history.len(), first.scroll, first.scripts_enabled, first.images_enabled, first.zoom), before);
    assert_eq!(first.events, events);
    first.scroll = 0;
    let probe = id(&first, "probe");
    let rect = first.page.controls.iter().find(|rect| rect.node == probe).unwrap();
    let (x,y) = (rect.x + rect.width/2, first.content_top() + rect.y + rect.height/2);
    first.pointer_down(x,y,false); first.pointer_up(x,y);
    assert_eq!(first.document.title, "Original VM retained");
    assert_eq!(first.scripting.as_ref().unwrap().error_count(), 0);
    drop(first); drop(second);
    std::fs::remove_file(source).unwrap(); std::fs::remove_dir(folder).unwrap();
}

#[test]
fn shared_profile_app_add_cannot_rebind_a_stale_manager_to_reused_folder_id() {
    let shared = Shared::default();
    let mut seed = shared.view().unwrap();
    let mut data = seed.data().clone();
    let old_id = data.add_folder(0, "Original destination".into()).unwrap();
    seed.save(data).unwrap();
    let (mut first, mut second) = pair(&shared);
    first.show_bookmark_folder(old_id);
    let original_revision = first.browser_panel.as_ref().unwrap().profile_revision;
    let mut peer = second.profile.data().clone();
    peer.remove_folder(old_id);
    let replacement_id = peer.add_folder(0, "Different peer destination".into()).unwrap();
    assert_eq!(replacement_id, old_id, "This control must exercise actual folder-ID reuse");
    second.profile.save(peer.clone()).unwrap();
    click(&mut first, Action::Add);
    let panel = first.browser_panel.as_ref().unwrap();
    assert!(matches!(panel.kind, PanelKind::Bookmarks));
    assert_eq!(panel.profile_revision, original_revision);
    assert!(panel.error.contains("changed"));
    assert_eq!(shared.snapshot().unwrap().data(), &peer);
    // The keyboard/star path enters the same Add function and must guard before
    // refreshing its View or copying the stale manager's folder destination.
    first.add_current_bookmark();
    assert!(matches!(first.browser_panel.as_ref().unwrap().kind, PanelKind::Bookmarks));
    assert!(shared.snapshot().unwrap().data().bookmarks.is_empty());
    first.key(0x1b, false, false, false);
    assert!(first.browser_panel.is_none(), "Escape must remain usable on a stale manager");
}
