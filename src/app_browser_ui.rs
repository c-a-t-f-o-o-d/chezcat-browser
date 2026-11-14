//! Native browser-owned panels. These controls are not document nodes and are
//! never reachable through the page's JavaScript or form event paths.

use super::*;
use crate::profile::{Bookmark, Store, MAX_ADDRESS_BYTES, MAX_BOOKMARKS, MAX_TITLE_BYTES};
use crate::ui::{self, Icon};

#[path = "app_appearance_ui.rs"]
mod appearance_ui;
#[path = "app_blocking_ui.rs"]
mod blocking_ui;
#[path = "app_bookmark_tree_ui.rs"]
mod bookmark_tree_ui;

const ROW: i32 = 52;
const PANEL_HEADER: i32 = 60;
const PANEL_FOOTER: i32 = 54;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Close,
    Add,
    List,
    Row(usize),
    Open,
    Edit,
    Delete,
    Title,
    Address,
    SaveBookmark,
    CancelBookmark,
    JavaScript,
    Images,
    SearchEngine(crate::profile::SearchEngine),
    BlockingEnabled,
    BlockingOptions,
    BuiltinBlocking,
    RuleEntry,
    RuleAdd,
    RuleDelete,
    RuleList,
    RuleRow(usize),
    ApplyBlocking,
    Zoom,
    Home,
    CurrentHome,
    BundledHome,
    Apply,
    Cancel,
    HistoryOptions,
    HistorySearch,
    SearchField,
    ApplySearch,
    HistoryRow(u64),
    HistoryClear,
    RecordHistory,
    Retention(Option<u16>),
    ClearOnExit,
    ApplyHistory,
    ClearRange(crate::history::ClearRange),
    ConfirmClear,
    DarkMode,
    DarkPages,
    BookmarkBar,
    LinkColor,
    VisitedColor,
    ResetColors,
    DownloadFolder,
    DefaultDownloads,
    BookmarksOptions,
    AppearanceOptions,
    DownloadOptions,
    FolderRow(u32),
    NewFolder,
    FolderUp,
    SettingsBack,
    Destination,
    DestinationRow(u32),
    SaveFolder,
    ConfirmDeleteFolder,
    Move,
    MoveUp,
    MoveDown,
}

#[derive(Clone, Copy)]
enum PanelKind {
    Bookmarks,
    EditBookmark { index: Option<usize> },
    Settings,
    BlockingRules,
    History,
    HistoryOptions,
    HistorySearch,
    HistoryClear,
    EditFolder { id: Option<u32> },
    FolderDestination,
    DeleteFolder { id: u32 },
}

pub(super) struct BrowserPanel {
    kind: PanelKind,
    focus: Action,
    selected: Option<usize>,
    first: usize,
    scroll: i32,
    title: Editor,
    address: Editor,
    zoom: Editor,
    javascript: bool,
    dark_mode: bool,
    dark_pages: bool,
    bookmark_bar: bool,
    link_color: Editor,
    visited_color: Editor,
    download_folder: Editor,
    folder: u32,
    selected_folder: Option<u32>,
    destination: u32,

    images: bool,
    search_engine: crate::profile::SearchEngine,
    blocking: crate::blocking::Config,
    error: String,
    pressed: Option<Action>,
    hover: Option<Action>,
    dragging_field: bool,
    wheel_remainder: i32,
    history_selected: Option<u64>,
    search: Editor,
    history_policy: crate::history::Policy,
    clear_range: crate::history::ClearRange,
    return_panel: Option<Box<BrowserPanel>>,
}

impl BrowserPanel {
    fn new(kind: PanelKind, focus: Action) -> Self {
        Self {
            kind,
            focus,
            selected: None,
            first: 0,
            scroll: 0,
            title: Editor::default(),
            address: Editor::default(),
            zoom: Editor::default(),
            javascript: true,
            dark_mode: false,
            dark_pages: false,
            bookmark_bar: true,
            link_color: Editor::default(),
            visited_color: Editor::default(),
            download_folder: Editor::default(),
            folder: 0,
            selected_folder: None,
            destination: 0,

            images: true,
            search_engine: crate::profile::SearchEngine::Google,
            blocking: crate::blocking::Config::default(),
            error: String::new(),
            pressed: None,
            hover: None,
            dragging_field: false,
            wheel_remainder: 0,
            history_selected: None,
            search: Editor::default(),
            history_policy: crate::history::Policy::default(),
            clear_range: crate::history::ClearRange::Hour,
            return_panel: None,
        }
    }
    fn editor(&mut self) -> Option<&mut Editor> {
        match self.focus {
            Action::Title => Some(&mut self.title),
            Action::Address | Action::Home | Action::RuleEntry => Some(&mut self.address),
            Action::Zoom => Some(&mut self.zoom),
            Action::LinkColor => Some(&mut self.link_color),
            Action::VisitedColor => Some(&mut self.visited_color),
            Action::DownloadFolder => Some(&mut self.download_folder),
            Action::SearchField => Some(&mut self.search),
            _ => None,
        }
    }
    pub(super) fn cancel_pointer(&mut self) {
        self.pressed = None;
        self.hover = None;
        self.dragging_field = false;
    }
}

#[derive(Clone, Copy)]
struct Rect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}
impl Rect {
    fn tuple(self) -> (i32, i32, i32, i32) {
        (self.x, self.y, self.w, self.h)
    }
    fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

struct Control {
    action: Action,
    rect: Rect,
    label: &'static str,
    enabled: bool,
}

impl App {
    /// Development TSV only. Selection and Apply/Cancel use native panel hits;
    /// making a control visible changes scroll/focus, never its stored value.
    pub(super) fn input_settings_action(&mut self, name: &str) -> Result<(),String> {
        if name=="Open" {self.show_settings();return Ok(());}
        let action=match name {"Apply"=>Action::Apply,"Cancel"=>Action::Cancel,
            value=>Action::SearchEngine(crate::profile::SearchEngine::parse(value)?)};
        let panel=self.browser_panel.as_mut().ok_or("Settings panel is not open.")?;
        if !matches!(panel.kind,PanelKind::Settings) {return Err("Expected the settings panel.".into());}
        panel.focus=action;
        self.ensure_browser_focus_visible();
        let control=self.browser_controls().into_iter().find(|control|control.action==action&&control.enabled).ok_or("Settings control is unavailable.")?;
        let (x,y)=(control.rect.x+control.rect.w/2,control.rect.y+control.rect.h/2);
        let body=self.browser_body();
        if self.browser_body_action(action) && !body.contains(x,y) {return Err("Settings control is not visible.".into());}
        self.pointer_down(x,y,false);self.pointer_up(x,y);
        Ok(())
    }
    /// Startup only: applying a profile does not execute a page or navigate.
    /// App::new remains deterministic and memory-only for tests/headless use.
    pub fn attach_profile(&mut self, store: Store) {
        self.cancel_pending();
        self.scripting = None;
        self.scripts_enabled = store.data().settings.javascript;
        self.images_enabled = store.data().settings.images;
        self.zoom = store.data().settings.default_zoom as f32 / 100.0;
        self.profile = store;
        self.reflow();
        if let Some(problem) = self.profile.problem() {
            self.status = format!("Preferences unavailable: {problem}");
            self.failed = false;
        }
    }

    pub fn open_startup_home(&mut self) {
        if self.profile.data().settings.home.is_some() {
            self.home();
        }
    }

    pub(super) fn profile_error(&mut self, error: String) {
        self.status = format!("Preferences were not saved: {error}");
        self.failed = false;
        if let Some(panel) = &mut self.browser_panel {
            panel.error = error;
        }
    }

    fn begin_browser_panel(&mut self, panel: BrowserPanel) {
        self.reset_chrome();
        self.close_downloads();
        self.close_bookmark_popup();
        self.inspector.focused = false;
        self.browser_panel = Some(panel);
        self.select_popup = None;
        self.menu = false;
        self.focus = Focus::Page;
        self.selection = None;
        self.drag = Drag::None;
        self.hover = None;
        self.pending_surrogate = None;
        self.sync_browser_panel();
    }

    pub(super) fn close_browser_panel(&mut self) {
        self.browser_panel = None;
        self.drag = Drag::None;
        self.pending_surrogate = None;
        self.focus = Focus::Page;
    }

    fn leave_browser_panel(&mut self) {
        let previous = self
            .browser_panel
            .as_mut()
            .and_then(|panel| panel.return_panel.take());
        if let Some(previous) = previous {
            self.begin_browser_panel(*previous);
        } else {
            self.close_browser_panel();
        }
    }

    pub fn show_history(&mut self) {
        let error = self.refresh_browsing_history().err();
        let mut panel = BrowserPanel::new(PanelKind::History, Action::List);
        panel.history_selected = self
            .browsing_history
            .data()
            .entries
            .first()
            .map(|entry| entry.id);
        panel.error = error
            .or_else(|| self.browsing_history_error.clone())
            .or_else(|| self.browsing_history.problem().map(str::to_owned))
            .unwrap_or_default();
        self.begin_browser_panel(panel);
    }

    fn show_history_options(&mut self) {
        let error = self.refresh_browsing_history().err();
        let mut panel = BrowserPanel::new(PanelKind::HistoryOptions, Action::RecordHistory);
        panel.history_policy = self.browsing_history.data().policy;
        panel.error = error
            .or_else(|| self.browsing_history_error.clone())
            .or_else(|| self.browsing_history.problem().map(str::to_owned))
            .unwrap_or_default();
        // Only Settings and the history list offer this control, so the return
        // state is bounded to one panel and preserves an unsaved settings draft.
        panel.return_panel = self.browser_panel.take().map(Box::new);
        self.begin_browser_panel(panel);
    }

    fn show_history_search(&mut self) {
        let Some(previous) = self.browser_panel.take() else {
            return;
        };
        if !matches!(previous.kind, PanelKind::History) {
            self.browser_panel = Some(previous);
            return;
        }
        let mut panel = BrowserPanel::new(PanelKind::HistorySearch, Action::SearchField);
        panel.search.set(&previous.search.text());
        panel.search.select_all();
        panel.return_panel = Some(Box::new(previous));
        self.begin_browser_panel(panel);
        self.ensure_browser_focus_visible();
    }

    fn apply_history_search(&mut self) {
        let Some(panel) = self.browser_panel.as_mut() else {
            return;
        };
        if !matches!(panel.kind, PanelKind::HistorySearch) {
            return;
        }
        if let Some(previous) = &mut panel.return_panel {
            previous.search.set(&panel.search.text());
            previous.history_selected = None;
            previous.first = 0;
            previous.focus = Action::List;
        }
        self.leave_browser_panel();
        let first = self.history_matches().first().copied();
        if let Some(panel) = &mut self.browser_panel {
            panel.history_selected = first;
        }
    }

    fn show_history_clear(&mut self) {
        let error = self.refresh_browsing_history().err();
        let mut panel = BrowserPanel::new(
            PanelKind::HistoryClear,
            Action::ClearRange(crate::history::ClearRange::Hour),
        );
        panel.error = error.unwrap_or_default();
        panel.return_panel = self.browser_panel.take().map(Box::new);
        self.begin_browser_panel(panel);
        self.ensure_browser_focus_visible();
    }

    fn history_panel_error(&mut self, error: String) {
        self.status = format!("History: {error}");
        self.failed = false;
        if let Some(panel) = &mut self.browser_panel {
            panel.error = error;
        }
    }

    fn apply_history_options(&mut self) {
        let Some(panel) = self.browser_panel.as_ref() else {
            return;
        };
        if !matches!(panel.kind, PanelKind::HistoryOptions) {
            return;
        }
        let policy = panel.history_policy;
        match self
            .browsing_history
            .configure(policy, crate::history::now())
        {
            Ok(_) => {
                self.browsing_history_changed();
                self.leave_browser_panel();
                self.status = "History settings saved.".into();
                self.failed = false;
            }
            Err(error) => self.history_panel_error(error),
        }
    }

    fn confirm_history_clear(&mut self) {
        let Some(panel) = self.browser_panel.as_ref() else {
            return;
        };
        if !matches!(panel.kind, PanelKind::HistoryClear) {
            return;
        }
        let range = panel.clear_range;
        match self.browsing_history.clear(range, crate::history::now()) {
            Ok(count) => {
                self.browsing_history_changed();
                self.visited.clear();
                self.rebuild_link_colors();
                self.leave_browser_panel();
                if let Some(panel) = &mut self.browser_panel {
                    panel.error.clear();
                }
                self.sync_browser_panel();
                self.status = format!("Cleared {count} history visit(s). Bookmarks are unchanged.");
                self.failed = false;
            }
            Err(error) => self.history_panel_error(error),
        }
    }

    fn history_matches(&self) -> Vec<u64> {
        let query = self
            .browser_panel
            .as_ref()
            .map(|panel| panel.search.text().to_lowercase())
            .unwrap_or_default();
        self.browsing_history
            .data()
            .entries
            .iter()
            .filter(|entry| {
                query.is_empty()
                    || entry.title.to_lowercase().contains(&query)
                    || entry.url.to_lowercase().contains(&query)
            })
            .map(|entry| entry.id)
            .collect()
    }

    fn sync_history_panel(&mut self) {
        if !self
            .browser_panel
            .as_ref()
            .is_some_and(|panel| matches!(panel.kind, PanelKind::History))
        {
            return;
        }
        let matches = self.history_matches();
        let rows = self.bookmark_rows();
        if let Some(panel) = &mut self.browser_panel {
            if panel
                .history_selected
                .is_some_and(|id| !matches.contains(&id))
            {
                panel.history_selected = None;
            }
            panel.first = panel.first.min(matches.len().saturating_sub(rows));
        }
    }

    fn ensure_history_visible(&mut self) {
        let matches = self.history_matches();
        let rows = self.bookmark_rows();
        if let Some(panel) = &mut self.browser_panel {
            if let Some(index) = panel
                .history_selected
                .and_then(|id| matches.iter().position(|entry| *entry == id))
            {
                if index < panel.first {
                    panel.first = index;
                }
                if index >= panel.first + rows {
                    panel.first = index + 1 - rows;
                }
            }
            panel.first = panel.first.min(matches.len().saturating_sub(rows));
        }
    }

    fn open_selected_history(&mut self) {
        let Some(id) = self
            .browser_panel
            .as_ref()
            .and_then(|panel| panel.history_selected)
        else {
            return;
        };
        if let Err(error) = self.refresh_browsing_history() {
            self.history_panel_error(error);
            return;
        }
        let Some(entry) = self
            .browsing_history
            .data()
            .entries
            .iter()
            .find(|entry| entry.id == id)
        else {
            self.history_panel_error(
                "That visit was removed or expired. Choose another entry.".into(),
            );
            self.sync_browser_panel();
            return;
        };
        let location = match navigation::address(&entry.url, &Location::home()) {
            Ok(location) => location,
            Err(error) => {
                self.history_panel_error(format!("Cannot open visit: {error}"));
                return;
            }
        };
        self.close_browser_panel();
        // A saved visit contains an address, never a captured POST payload.
        self.navigate(location);
    }

    fn remove_selected_history(&mut self) {
        let Some(id) = self
            .browser_panel
            .as_ref()
            .and_then(|panel| panel.history_selected)
        else {
            return;
        };
        let removed_url = self
            .browsing_history
            .data()
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.url.split('#').next().unwrap_or(&entry.url).to_owned());
        match self.browsing_history.remove(id, crate::history::now()) {
            Ok(removed) => {
                if removed {
                    if let Some(url) = removed_url {
                        self.visited.remove(&url);
                    }
                }
                self.browsing_history_changed();
                self.sync_browser_panel();
                if let Some(panel) = &mut self.browser_panel {
                    panel.error.clear();
                }
                self.status = if removed {
                    "History visit removed."
                } else {
                    "That visit was already removed or expired."
                }
                .into();
                self.failed = false;
            }
            Err(error) => self.history_panel_error(error),
        }
    }

    pub fn show_bookmarks(&mut self) {
        self.show_bookmark_folder(0);
    }

    pub fn show_settings(&mut self) {
        let settings = &self.profile.data().settings;
        let mut panel = BrowserPanel::new(PanelKind::Settings, Action::JavaScript);
        // Session flags may override disk settings; Apply deliberately commits
        // the effective choices shown here, while Cancel leaves both untouched.
        panel.javascript = self.scripts_enabled;
        panel.images = self.images_enabled;
        panel.search_engine = settings.search_engine;
        panel.blocking = settings.blocking.clone();
        panel.dark_mode = settings.dark_mode;
        panel.dark_pages = settings.dark_pages;
        panel.bookmark_bar = settings.bookmark_bar;
        panel
            .link_color
            .set(&crate::profile::format_color(settings.link_color));
        panel
            .visited_color
            .set(&crate::profile::format_color(settings.visited_color));
        panel
            .download_folder
            .set(settings.download_folder.as_deref().unwrap_or(""));
        panel.zoom.set(&settings.default_zoom.to_string());
        panel
            .address
            .set(settings.home.as_deref().unwrap_or("about:home"));
        if let Some(problem) = self.profile.problem() {
            panel.error = problem.to_owned();
        }
        self.begin_browser_panel(panel);
    }

    pub fn add_current_bookmark(&mut self) {
        // Capture the document on screen, never an edited address or an
        // uncommitted navigation. No request payload is retained in a bookmark.
        let url = self.location.display();
        let existing = self
            .profile
            .data()
            .bookmarks
            .iter()
            .position(|entry| entry.url == url);
        if let Some(index) = existing {
            self.edit_bookmark(index);
            return;
        }
        let folder = self.browser_panel.as_ref().map_or(0, |p| p.folder);
        let title = bookmark_title(&self.document.title, &url);
        let mut panel = BrowserPanel::new(PanelKind::EditBookmark { index: None }, Action::Title);
        panel.folder = folder;
        panel.title.set(&title);
        panel.title.select_all();
        panel.address.set(&url);
        panel.return_panel = self.bookmark_editor_return();
        self.begin_browser_panel(panel);
    }

    fn edit_bookmark(&mut self, index: usize) {
        let Some(bookmark) = self.profile.data().bookmarks.get(index) else {
            return;
        };
        let mut panel = BrowserPanel::new(
            PanelKind::EditBookmark { index: Some(index) },
            Action::Title,
        );
        panel.folder = bookmark.folder;
        panel.title.set(&bookmark.title);
        panel.title.select_all();
        panel.address.set(&bookmark.url);
        panel.return_panel = self.bookmark_editor_return();
        self.begin_browser_panel(panel);
    }

    fn normalized_browser_address(value: &str) -> Result<String, String> {
        let value = value.trim();
        if value.len() > MAX_ADDRESS_BYTES {
            return Err("The saved address is too long (maximum 16 KiB).".into());
        }
        // Already-absolute addresses are stored without probing disk. A moved
        // local file must not prevent changing its title or another preference.
        navigation::stored_address(value).or_else(|_| {
            navigation::address(value, &Location::home())
                .and_then(|location| navigation::stored_address(&location.display()))
        })
    }

    fn save_bookmark(&mut self) {
        let Some(panel) = &self.browser_panel else {
            return;
        };
        let PanelKind::EditBookmark { index } = panel.kind else {
            return;
        };
        let title = panel.title.text().trim().to_owned();
        let url = match Self::normalized_browser_address(&panel.address.text()) {
            Ok(url) => url,
            Err(error) => {
                self.profile_error(error);
                return;
            }
        };
        let mut data = self.profile.data().clone();
        if data
            .bookmarks
            .iter()
            .enumerate()
            .any(|(other, entry)| Some(other) != index && entry.url == url)
        {
            self.profile_error(
                "That address is already bookmarked. Edit its existing entry.".into(),
            );
            return;
        }
        let folder = panel.folder;
        let order = index
            .and_then(|i| data.bookmarks.get(i))
            .filter(|b| b.folder == folder)
            .map_or_else(|| data.next_order(folder), |b| b.order);
        let value = Bookmark {
            title,
            url,
            folder,
            order,
        };
        let selected = if let Some(index) = index {
            let Some(entry) = data.bookmarks.get_mut(index) else {
                return;
            };
            *entry = value;
            index
        } else {
            if data.bookmarks.len() >= MAX_BOOKMARKS {
                self.profile_error(format!(
                    "The bookmark limit is {MAX_BOOKMARKS}. Remove one before adding another."
                ));
                return;
            }
            data.bookmarks.push(value);
            data.bookmarks.len() - 1
        };
        if let Err(error) = self.profile.save(data) {
            self.profile_error(error);
            return;
        }
        self.show_bookmark_folder(folder);
        if let Some(panel) = &mut self.browser_panel {
            panel.selected = Some(selected);
            panel.selected_folder = None;
        }
        self.ensure_bookmark_visible();
        self.status = "Bookmark saved.".into();
        self.failed = false;
    }

    fn remove_selected_bookmark(&mut self) {
        let Some(index) = self.browser_panel.as_ref().and_then(|panel| panel.selected) else {
            return;
        };
        let folder = self.browser_panel.as_ref().map_or(0, |panel| panel.folder);
        let position = self
            .bookmark_tree_items()
            .iter()
            .position(|item| *item == crate::profile::BookmarkItem::Link(index))
            .unwrap_or(0);
        let mut data = self.profile.data().clone();
        if index >= data.bookmarks.len() {
            return;
        }
        data.bookmarks.remove(index);
        if let Err(error) = self.profile.save(data) {
            self.profile_error(error);
            return;
        }
        self.show_bookmark_folder(folder);
        let rows = self.bookmark_tree_items();
        if let Some(item) = rows.get(position.min(rows.len().saturating_sub(1))) {
            let action = match item {
                crate::profile::BookmarkItem::Link(i) => Action::Row(*i),
                crate::profile::BookmarkItem::Folder(id) => Action::FolderRow(*id),
            };
            self.bookmark_tree_activate(action);
        }
        self.ensure_bookmark_visible();
        self.status = "Bookmark removed.".into();
        self.failed = false;
    }

    fn open_selected_bookmark(&mut self) {
        let Some(index) = self.browser_panel.as_ref().and_then(|panel| panel.selected) else {
            return;
        };
        let Some(entry) = self.profile.data().bookmarks.get(index) else {
            return;
        };
        let location = match navigation::address(&entry.url, &Location::home()) {
            Ok(location) => location,
            Err(error) => {
                self.cancel_pending();
                if let Some(panel) = &mut self.browser_panel {
                    panel.error = format!("Cannot open bookmark: {error}");
                }
                self.error(error);
                return;
            }
        };
        self.close_browser_panel();
        // Explicit navigation never restores a history entry's POST request.
        self.navigate(location);
    }

    fn apply_browser_settings(&mut self) {
        let Some(panel) = &self.browser_panel else {
            return;
        };
        if !matches!(panel.kind, PanelKind::Settings) {
            return;
        }
        let zoom = match panel.zoom.text().parse::<u16>() {
            Ok(value) if (50..=300).contains(&value) => value,
            _ => {
                self.profile_error("Default zoom must be a whole number from 50 to 300.".into());
                return;
            }
        };
        let home = match Self::normalized_browser_address(&panel.address.text()) {
            Ok(value) => (value != "about:home").then_some(value),
            Err(error) => {
                self.profile_error(error);
                return;
            }
        };
        let javascript = panel.javascript;
        let images = panel.images;
        let blocking = panel.blocking.clone();
        let reload = self.scripts_enabled != javascript
            || self.images_enabled != images
            || self.profile.data().settings.blocking != blocking;
        let link_color = match crate::profile::parse_color(&panel.link_color.text()) {
            Ok(v) => v,
            Err(e) => {
                self.profile_error(e);
                return;
            }
        };
        let visited_color = match crate::profile::parse_color(&panel.visited_color.text()) {
            Ok(v) => v,
            Err(e) => {
                self.profile_error(e);
                return;
            }
        };
        let folder = panel.download_folder.text().trim().to_owned();
        if !folder.is_empty() {
            if let Err(e) = crate::downloads::validate_folder(Path::new(&folder)) {
                self.profile_error(e);
                return;
            }
        }
        let mut data = self.profile.data().clone();
        data.settings.dark_mode = panel.dark_mode;
        data.settings.dark_pages = panel.dark_pages;
        data.settings.bookmark_bar = panel.bookmark_bar;
        data.settings.link_color = link_color;
        data.settings.visited_color = visited_color;
        data.settings.download_folder = (!folder.is_empty()).then_some(folder);
        data.settings.javascript = javascript;
        data.settings.images = images;
        data.settings.search_engine = panel.search_engine;
        data.settings.blocking = blocking;
        data.settings.default_zoom = zoom;
        data.settings.home = home;
        if let Err(error) = self.profile.save(data) {
            self.profile_error(error);
            return;
        }
        self.close_browser_panel();
        self.scripts_enabled = javascript;
        self.images_enabled = images;
        if !javascript {
            self.scripting = None;
            self.document.scripting_enabled = false;
        }
        if !images {
            self.images.clear();
        }
        self.set_zoom(zoom as f32 / 100.0);
        self.status = "Settings saved.".into();
        self.failed = false;
        if reload {
            self.reload();
        }
    }

    fn browser_header_height(&self) -> i32 {
        if self.content_bottom() - self.chrome_bottom() < 170 {
            44
        } else {
            PANEL_HEADER
        }
    }

    fn browser_footer_height(&self) -> i32 {
        if self.browser_header_height() == 44 {
            44
        } else {
            PANEL_FOOTER
        }
    }

    fn browser_footer_y(&self) -> i32 {
        self.content_bottom()
            - if self.browser_header_height() == 44 {
                38
            } else {
                44
            }
    }

    fn browser_body(&self) -> Rect {
        let margin = 20.min((self.width / 8).max(0));
        Rect {
            x: margin,
            y: self.chrome_bottom() + self.browser_header_height(),
            w: (self.width - margin * 2).max(1),
            h: (self.content_bottom()
                - self.browser_footer_height()
                - self.chrome_bottom()
                - self.browser_header_height())
            .max(1),
        }
    }

    fn bookmark_rows(&self) -> usize {
        (self.browser_body().h / ROW).max(1) as usize
    }

    fn browser_controls(&self) -> Vec<Control> {
        let Some(panel) = &self.browser_panel else {
            return Vec::new();
        };
        let body = self.browser_body();
        if matches!(
            panel.kind,
            PanelKind::Bookmarks
                | PanelKind::EditFolder { .. }
                | PanelKind::FolderDestination
                | PanelKind::DeleteFolder { .. }
        ) {
            return self.bookmark_tree_controls(body, panel);
        }
        let mut controls = vec![Control {
            action: Action::Close,
            rect: Rect {
                x: (self.width - 52).max(0),
                y: self.chrome_bottom() + 12,
                w: 32,
                h: 32,
            },
            label: "Close",
            enabled: true,
        }];
        let footer = self.browser_footer_y();
        let mut add = |action, rect, label, enabled| {
            controls.push(Control {
                action,
                rect,
                label,
                enabled,
            })
        };
        match panel.kind {
            PanelKind::Bookmarks | PanelKind::History => {
                let history = matches!(panel.kind, PanelKind::History);
                add(
                    if history {
                        Action::HistorySearch
                    } else {
                        Action::Add
                    },
                    Rect {
                        x: (self.width - 186).max(body.x),
                        y: self.chrome_bottom() + 12,
                        w: 124,
                        h: 32,
                    },
                    if history {
                        "Search  Ctrl+F"
                    } else {
                        "Add page"
                    },
                    true,
                );
                add(Action::List, body, "", true);
                let actions: &[(Action, &str)] = if history {
                    &[
                        (Action::Open, "Open"),
                        (Action::Delete, "Delete"),
                        (Action::HistoryClear, "Clear..."),
                        (Action::HistoryOptions, "Settings"),
                    ]
                } else {
                    &[
                        (Action::Open, "Open"),
                        (Action::Edit, "Edit"),
                        (Action::Delete, "Delete"),
                    ]
                };
                let width =
                    ((body.w - 8 * (actions.len() as i32 - 1)) / actions.len() as i32).max(1);
                let active = if history {
                    panel.history_selected.is_some()
                } else {
                    panel.selected.is_some()
                };
                for (i, &(action, label)) in actions.iter().enumerate() {
                    add(
                        action,
                        Rect {
                            x: body.x + i as i32 * (width + 8),
                            y: footer,
                            w: width,
                            h: 32,
                        },
                        label,
                        active || matches!(action, Action::HistoryClear | Action::HistoryOptions),
                    );
                }
            }
            PanelKind::EditBookmark { .. } => {
                add(
                    Action::Title,
                    Rect {
                        x: body.x,
                        y: body.y + 28 - panel.scroll,
                        w: body.w,
                        h: 34,
                    },
                    "",
                    true,
                );
                add(
                    Action::Address,
                    Rect {
                        x: body.x,
                        y: body.y + 108 - panel.scroll,
                        w: body.w,
                        h: 34,
                    },
                    "",
                    true,
                );
                add(
                    Action::SaveBookmark,
                    Rect {
                        x: body.x,
                        y: footer,
                        w: (body.w / 2 - 4).max(1),
                        h: 32,
                    },
                    "Save",
                    true,
                );
                add(
                    Action::CancelBookmark,
                    Rect {
                        x: body.x + body.w / 2 + 4,
                        y: footer,
                        w: (body.w / 2 - 4).max(1),
                        h: 32,
                    },
                    "Cancel",
                    true,
                );
            }
            PanelKind::Settings => {
                add(
                    Action::JavaScript,
                    Rect {
                        x: body.x,
                        y: body.y - panel.scroll,
                        w: body.w,
                        h: 36,
                    },
                    "Enable JavaScript",
                    true,
                );
                add(
                    Action::Images,
                    Rect {
                        x: body.x,
                        y: body.y + 48 - panel.scroll,
                        w: body.w,
                        h: 36,
                    },
                    "Load images",
                    true,
                );
                add(
                    Action::BlockingEnabled,
                    Rect {
                        x: body.x,
                        y: body.y + 96 - panel.scroll,
                        w: body.w,
                        h: 36,
                    },
                    "Block ads / trackers with local request rules",
                    true,
                );
                add(
                    Action::Zoom,
                    Rect {
                        x: body.x,
                        y: body.y + 176 - panel.scroll,
                        w: 100.min(body.w),
                        h: 34,
                    },
                    "",
                    true,
                );
                add(
                    Action::Home,
                    Rect {
                        x: body.x,
                        y: body.y + 256 - panel.scroll,
                        w: body.w,
                        h: 34,
                    },
                    "",
                    true,
                );
                let width = ((body.w - 8) / 2).max(1);
                add(
                    Action::CurrentHome,
                    Rect {
                        x: body.x,
                        y: body.y + 302 - panel.scroll,
                        w: width,
                        h: 32,
                    },
                    "Use current page",
                    true,
                );
                add(
                    Action::BundledHome,
                    Rect {
                        x: body.x + width + 8,
                        y: body.y + 302 - panel.scroll,
                        w: width,
                        h: 32,
                    },
                    "Use start page",
                    true,
                );
                add(
                    Action::HistoryOptions,
                    Rect {
                        x: body.x,
                        y: body.y + 402 - panel.scroll,
                        w: body.w,
                        h: 34,
                    },
                    "History settings...",
                    true,
                );
                add(
                    Action::BlockingOptions,
                    Rect {
                        x: body.x,
                        y: body.y + 450 - panel.scroll,
                        w: body.w,
                        h: 34,
                    },
                    "Request-blocking rules...",
                    true,
                );
                add(
                    Action::Apply,
                    Rect {
                        x: body.x,
                        y: footer,
                        w: width,
                        h: 32,
                    },
                    "Apply",
                    true,
                );
                add(
                    Action::Cancel,
                    Rect {
                        x: body.x + width + 8,
                        y: footer,
                        w: width,
                        h: 32,
                    },
                    "Cancel",
                    true,
                );
            }
            PanelKind::EditFolder { .. }
            | PanelKind::FolderDestination
            | PanelKind::DeleteFolder { .. } => {}
            PanelKind::BlockingRules => {
                controls.extend(self.blocking_controls(body, footer, panel));
            }
            PanelKind::HistorySearch => {
                add(
                    Action::SearchField,
                    Rect {
                        x: body.x,
                        y: body.y + 6 - panel.scroll,
                        w: body.w,
                        h: 34,
                    },
                    "",
                    true,
                );
                let width = ((body.w - 8) / 2).max(1);
                add(
                    Action::ApplySearch,
                    Rect {
                        x: body.x,
                        y: footer,
                        w: width,
                        h: 32,
                    },
                    "Search",
                    true,
                );
                add(
                    Action::Cancel,
                    Rect {
                        x: body.x + width + 8,
                        y: footer,
                        w: width,
                        h: 32,
                    },
                    "Cancel",
                    true,
                );
            }
            PanelKind::HistoryOptions => {
                add(
                    Action::RecordHistory,
                    Rect {
                        x: body.x,
                        y: body.y - panel.scroll,
                        w: body.w,
                        h: 36,
                    },
                    "Record browsing history",
                    true,
                );
                for (i, (days, label)) in [
                    (Some(1), "1 day"),
                    (Some(7), "7 days"),
                    (Some(30), "30 days"),
                    (Some(90), "90 days"),
                    (None, "No age limit"),
                ]
                .into_iter()
                .enumerate()
                {
                    add(
                        Action::Retention(days),
                        Rect {
                            x: body.x,
                            y: body.y + 84 + i as i32 * 38 - panel.scroll,
                            w: body.w,
                            h: 36,
                        },
                        label,
                        true,
                    );
                }
                add(
                    Action::ClearOnExit,
                    Rect {
                        x: body.x,
                        y: body.y + 286 - panel.scroll,
                        w: body.w,
                        h: 36,
                    },
                    "Clear on window close",
                    true,
                );
                let width = ((body.w - 8) / 2).max(1);
                add(
                    Action::ApplyHistory,
                    Rect {
                        x: body.x,
                        y: footer,
                        w: width,
                        h: 32,
                    },
                    "Apply",
                    true,
                );
                add(
                    Action::Cancel,
                    Rect {
                        x: body.x + width + 8,
                        y: footer,
                        w: width,
                        h: 32,
                    },
                    "Cancel",
                    true,
                );
            }
            PanelKind::HistoryClear => {
                for (i, (range, label)) in [
                    (crate::history::ClearRange::Hour, "Last hour"),
                    (crate::history::ClearRange::Day, "Last 24 hours"),
                    (crate::history::ClearRange::Week, "Last 7 days"),
                    (crate::history::ClearRange::All, "All time"),
                ]
                .into_iter()
                .enumerate()
                {
                    add(
                        Action::ClearRange(range),
                        Rect {
                            x: body.x,
                            y: body.y + 58 + i as i32 * 40 - panel.scroll,
                            w: body.w,
                            h: 36,
                        },
                        label,
                        true,
                    );
                }
                let width = ((body.w - 8) / 2).max(1);
                add(
                    Action::ConfirmClear,
                    Rect {
                        x: body.x,
                        y: footer,
                        w: width,
                        h: 32,
                    },
                    "Clear history",
                    true,
                );
                add(
                    Action::Cancel,
                    Rect {
                        x: body.x + width + 8,
                        y: footer,
                        w: width,
                        h: 32,
                    },
                    "Cancel",
                    true,
                );
            }
        }
        if matches!(panel.kind, PanelKind::Settings) {
            for (index,engine) in crate::profile::SearchEngine::ALL.into_iter().enumerate() {
                controls.push(Control {action:Action::SearchEngine(engine),rect:Rect{x:body.x,y:body.y+1192+index as i32*44-panel.scroll,w:body.w,h:36},label:engine.name(),enabled:true});
            }
            controls.extend(self.appearance_controls(body, panel));
        }
        if matches!(panel.kind, PanelKind::EditBookmark { .. }) {
            controls.extend(self.bookmark_editor_controls(body, panel));
        }
        controls
    }

    fn browser_body_action(&self, action: Action) -> bool {
        if action == Action::HistoryOptions {
            return self
                .browser_panel
                .as_ref()
                .is_some_and(|panel| matches!(panel.kind, PanelKind::Settings));
        }
        matches!(
            action,
            Action::List
                | Action::Title
                | Action::Address
                | Action::JavaScript
                | Action::Images
                | Action::SearchEngine(_)
                | Action::BlockingEnabled
                | Action::BlockingOptions
                | Action::BuiltinBlocking
                | Action::RuleEntry
                | Action::RuleAdd
                | Action::RuleDelete
                | Action::RuleList
                | Action::Zoom
                | Action::Home
                | Action::DarkMode
                | Action::DarkPages
                | Action::BookmarkBar
                | Action::LinkColor
                | Action::VisitedColor
                | Action::ResetColors
                | Action::DownloadFolder
                | Action::DefaultDownloads
                | Action::Destination
                | Action::DestinationRow(_)
                | Action::BookmarksOptions
                | Action::CurrentHome
                | Action::BundledHome
                | Action::SearchField
                | Action::RecordHistory
                | Action::Retention(_)
                | Action::ClearOnExit
                | Action::ClearRange(_)
        )
    }

    fn browser_action_at(&self, x: i32, y: i32) -> Option<Action> {
        let body = self.browser_body();
        let panel = self.browser_panel.as_ref()?;
        for control in self.browser_controls() {
            if control.enabled
                && control.rect.contains(x, y)
                && (!self.browser_body_action(control.action) || body.contains(x, y))
            {
                if control.action == Action::RuleList {
                    let index = panel.first + ((y - control.rect.y) / ROW) as usize;
                    return (index < panel.blocking.rules.len()).then_some(Action::RuleRow(index));
                }
                if control.action == Action::List {
                    let index = panel.first + ((y - body.y) / ROW) as usize;
                    if matches!(panel.kind, PanelKind::History) {
                        return self
                            .history_matches()
                            .get(index)
                            .copied()
                            .map(Action::HistoryRow);
                    }
                    return self.bookmark_tree_row(index);
                }
                return Some(control.action);
            }
        }
        None
    }

    fn browser_focus_order(&self) -> Vec<Action> {
        let Some(panel) = &self.browser_panel else {
            return Vec::new();
        };
        if matches!(
            panel.kind,
            PanelKind::Bookmarks
                | PanelKind::EditFolder { .. }
                | PanelKind::FolderDestination
                | PanelKind::DeleteFolder { .. }
        ) {
            let controls = self.bookmark_tree_controls(self.browser_body(), panel);
            if matches!(panel.kind, PanelKind::Bookmarks) {
                return [
                    Action::List,
                    Action::Add,
                    Action::Open,
                    Action::Edit,
                    Action::Move,
                    Action::Delete,
                    Action::NewFolder,
                    Action::FolderUp,
                    Action::MoveUp,
                    Action::MoveDown,
                    Action::SettingsBack,
                    Action::Close,
                ]
                .into_iter()
                .filter(|action| {
                    controls
                        .iter()
                        .any(|control| control.action == *action && control.enabled)
                })
                .collect();
            }
            return controls
                .into_iter()
                .filter(|c| c.enabled)
                .map(|c| c.action)
                .collect();
        }
        let order: &[Action] = match panel.kind {
            PanelKind::EditFolder { .. }
            | PanelKind::FolderDestination
            | PanelKind::DeleteFolder { .. } => &[],
            PanelKind::Bookmarks => &[
                Action::List,
                Action::Add,
                Action::Open,
                Action::Edit,
                Action::Delete,
                Action::Close,
            ],
            PanelKind::EditBookmark { .. } => &[
                Action::Title,
                Action::Address,
                Action::Destination,
                Action::SaveBookmark,
                Action::CancelBookmark,
                Action::Close,
            ],
            PanelKind::Settings => &[
                Action::JavaScript,
                Action::Images,
                Action::BlockingEnabled,
                Action::Zoom,
                Action::Home,
                Action::CurrentHome,
                Action::BundledHome,
                Action::HistoryOptions,
                Action::BlockingOptions,
                Action::DarkMode,
                Action::DarkPages,
                Action::BookmarkBar,
                Action::LinkColor,
                Action::VisitedColor,
                Action::ResetColors,
                Action::BookmarksOptions,
                Action::DownloadFolder,
                Action::DefaultDownloads,
                Action::SearchEngine(crate::profile::SearchEngine::Google),
                Action::SearchEngine(crate::profile::SearchEngine::Bing),
                Action::SearchEngine(crate::profile::SearchEngine::DuckDuckGo),
                Action::SearchEngine(crate::profile::SearchEngine::None),
                Action::Apply,
                Action::Cancel,
                Action::Close,
            ],
            PanelKind::BlockingRules => &[
                Action::BuiltinBlocking,
                Action::RuleEntry,
                Action::RuleAdd,
                Action::RuleList,
                Action::RuleDelete,
                Action::ApplyBlocking,
                Action::Cancel,
                Action::Close,
            ],
            PanelKind::History => &[
                Action::List,
                Action::HistorySearch,
                Action::Open,
                Action::Delete,
                Action::HistoryClear,
                Action::HistoryOptions,
                Action::Close,
            ],
            PanelKind::HistorySearch => &[
                Action::SearchField,
                Action::ApplySearch,
                Action::Cancel,
                Action::Close,
            ],
            PanelKind::HistoryOptions => &[
                Action::RecordHistory,
                Action::Retention(Some(1)),
                Action::Retention(Some(7)),
                Action::Retention(Some(30)),
                Action::Retention(Some(90)),
                Action::Retention(None),
                Action::ClearOnExit,
                Action::ApplyHistory,
                Action::Cancel,
                Action::Close,
            ],
            PanelKind::HistoryClear => &[
                Action::ClearRange(crate::history::ClearRange::Hour),
                Action::ClearRange(crate::history::ClearRange::Day),
                Action::ClearRange(crate::history::ClearRange::Week),
                Action::ClearRange(crate::history::ClearRange::All),
                Action::ConfirmClear,
                Action::Cancel,
                Action::Close,
            ],
        };
        let controls = self.browser_controls();
        order
            .iter()
            .copied()
            .filter(|action| {
                controls
                    .iter()
                    .any(|control| control.action == *action && control.enabled)
            })
            .collect()
    }

    fn browser_content_height(&self) -> i32 {
        match self.browser_panel.as_ref().map(|panel| panel.kind) {
            Some(PanelKind::Settings) => 1412,
            Some(PanelKind::BlockingRules) => self.browser_body().h.max(272),
            Some(PanelKind::HistoryOptions) => 390,
            Some(PanelKind::HistoryClear) => 300,
            Some(PanelKind::HistorySearch) => 102,
            Some(PanelKind::EditBookmark { .. }) => 280,
            Some(PanelKind::EditFolder { .. }) => 180,
            Some(PanelKind::FolderDestination) => self.folder_destination_height(),
            Some(PanelKind::DeleteFolder { .. }) => 160,
            _ => 0,
        }
    }

    fn ensure_browser_focus_visible(&mut self) {
        let body = self.browser_body();
        let Some(focus) = self.browser_panel.as_ref().map(|panel| panel.focus) else {
            return;
        };
        if self.browser_body_action(focus) && focus != Action::List {
            if let Some(control) = self
                .browser_controls()
                .into_iter()
                .find(|control| control.action == focus)
            {
                let amount = if control.rect.y < body.y {
                    control.rect.y - body.y
                } else if control.rect.y + control.rect.h > body.y + body.h {
                    control.rect.y + control.rect.h - body.y - body.h
                } else {
                    0
                };
                let max = (self.browser_content_height() - body.h).max(0);
                if let Some(panel) = &mut self.browser_panel {
                    panel.scroll = (panel.scroll + amount).clamp(0, max);
                }
            }
        }
        self.sync_browser_panel();
    }

    fn ensure_bookmark_visible(&mut self) {
        let items = self.bookmark_tree_items();
        let selected = self.bookmark_selected_item();
        let selected_index = selected.and_then(|item| items.iter().position(|i| *i == item));
        let count = items.len();
        let rows = self.bookmark_rows();
        if let Some(panel) = &mut self.browser_panel {
            if let Some(index) = selected_index {
                if index < panel.first {
                    panel.first = index;
                }
                if index >= panel.first + rows {
                    panel.first = index + 1 - rows;
                }
            }
            panel.first = panel.first.min(count.saturating_sub(rows));
        }
    }

    pub(super) fn sync_browser_panel(&mut self) {
        self.sync_history_panel();
        let body = self.browser_body();
        let max = (self.browser_content_height() - body.h).max(0);
        if let Some(panel) = &mut self.browser_panel {
            panel.scroll = panel.scroll.clamp(0, max);
            panel
                .title
                .ensure_visible(((body.w - 16) / 12).max(1) as usize);
            panel
                .address
                .ensure_visible(((body.w - 16) / 12).max(1) as usize);
            panel
                .zoom
                .ensure_visible(((100.min(body.w) - 16) / 12).max(1) as usize);
            panel
                .search
                .ensure_visible(((body.w - 16) / 12).max(1) as usize);
            panel
                .link_color
                .ensure_visible(((body.w - 16) / 12).max(1) as usize);
            panel
                .visited_color
                .ensure_visible(((body.w - 16) / 12).max(1) as usize);
            panel
                .download_folder
                .ensure_visible(((body.w - 16) / 12).max(1) as usize);
        }
    }

    fn browser_activate(&mut self, action: Action) {
        if self.bookmark_tree_activate(action) || self.appearance_activate(action) {
            self.sync_browser_panel();
            return;
        }
        match action {
            Action::Close | Action::Cancel => self.leave_browser_panel(),
            Action::Add => self.add_current_bookmark(),
            Action::Open => {
                if self
                    .browser_panel
                    .as_ref()
                    .is_some_and(|panel| matches!(panel.kind, PanelKind::History))
                {
                    self.open_selected_history();
                } else {
                    self.open_selected_bookmark();
                }
            }
            Action::Edit => {
                if let Some(index) = self.browser_panel.as_ref().and_then(|panel| panel.selected) {
                    self.edit_bookmark(index);
                }
            }
            Action::Delete => {
                if self
                    .browser_panel
                    .as_ref()
                    .is_some_and(|panel| matches!(panel.kind, PanelKind::History))
                {
                    self.remove_selected_history();
                } else {
                    self.remove_selected_bookmark();
                }
            }
            Action::SaveBookmark => self.save_bookmark(),
            Action::CancelBookmark => {
                if self
                    .browser_panel
                    .as_ref()
                    .is_some_and(|panel| panel.return_panel.is_some())
                {
                    self.leave_browser_panel();
                } else {
                    let folder = self.browser_panel.as_ref().map_or(0, |panel| panel.folder);
                    self.show_bookmark_folder(folder);
                }
            }
            Action::JavaScript => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.javascript = !panel.javascript;
                }
            }
            Action::SearchEngine(engine) => {if let Some(panel)=&mut self.browser_panel { panel.search_engine=engine; }}
            Action::Images => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.images = !panel.images;
                }
            }
            Action::BlockingEnabled => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.blocking.enabled = !panel.blocking.enabled;
                }
            }
            Action::BlockingOptions => self.show_blocking_options(),
            Action::BuiltinBlocking => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.blocking.builtins = !panel.blocking.builtins;
                }
            }
            Action::RuleAdd => self.add_blocking_rule(),
            Action::RuleDelete => self.delete_blocking_rule(),
            Action::ApplyBlocking => self.apply_blocking_options(),
            Action::RuleRow(index) => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.selected = Some(index);
                    panel.focus = Action::RuleList;
                }
            }
            Action::CurrentHome => {
                let address = self.location.display();
                if let Some(panel) = &mut self.browser_panel {
                    panel.address.set(&address);
                }
            }
            Action::BundledHome => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.address.set("about:home");
                }
            }
            Action::Apply => self.apply_browser_settings(),
            Action::HistoryOptions => self.show_history_options(),
            Action::HistorySearch => self.show_history_search(),
            Action::ApplySearch => self.apply_history_search(),
            Action::HistoryClear => self.show_history_clear(),
            Action::ApplyHistory => self.apply_history_options(),
            Action::ConfirmClear => self.confirm_history_clear(),
            Action::HistoryRow(id) => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.history_selected = Some(id);
                    panel.focus = Action::List;
                }
            }
            Action::RecordHistory => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.history_policy.enabled = !panel.history_policy.enabled;
                }
            }
            Action::Retention(days) => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.history_policy.retention_days = days;
                }
            }
            Action::ClearOnExit => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.history_policy.clear_on_exit = !panel.history_policy.clear_on_exit;
                }
            }
            Action::ClearRange(range) => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.clear_range = range;
                }
            }
            Action::Row(index) => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.selected = Some(index);
                    panel.focus = Action::List;
                }
            }
            _ => {}
        }
        self.sync_browser_panel();
    }

    pub(super) fn browser_key(
        &mut self,
        key: u32,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> PlatformAction {
        if ctrl && alt {
            return PlatformAction::None;
        }
        if (ctrl && key == 0x4c) || (alt && key == 0x44) {
            self.close_browser_panel();
            self.focus_address();
            return PlatformAction::None;
        }
        if ctrl && key == 0x48 {
            self.show_history();
            return PlatformAction::None;
        }
        if ctrl
            && key == 0x46
            && self
                .browser_panel
                .as_ref()
                .is_some_and(|panel| matches!(panel.kind, PanelKind::History))
        {
            self.show_history_search();
            return PlatformAction::None;
        }
        if ctrl && key == 0x44 {
            self.add_current_bookmark();
            return PlatformAction::None;
        }
        if ctrl && key == 0x42 {
            self.show_bookmarks();
            return PlatformAction::None;
        }
        if ctrl && key == 0x4a {
            self.toggle_javascript();
            if let Some(panel) = &mut self.browser_panel {
                if matches!(panel.kind, PanelKind::Settings) {
                    panel.javascript = self.scripts_enabled;
                }
            }
            return PlatformAction::None;
        }
        if ctrl && key == 0xbc {
            self.show_settings();
            return PlatformAction::None;
        }
        let Some(panel) = &mut self.browser_panel else {
            return PlatformAction::None;
        };
        if ctrl {
            if let Some(editor) = panel.editor() {
                match key {
                    0x41 => editor.select_all(),
                    0x43 | 0x58 => {
                        let text = editor.selected();
                        return if text.is_empty() {
                            PlatformAction::None
                        } else if key == 0x58 {
                            PlatformAction::Cut(text)
                        } else {
                            PlatformAction::Copy(text)
                        };
                    }
                    0x56 => return PlatformAction::Paste,
                    0x24 => editor.move_to(0, shift),
                    0x23 => editor.move_to(editor.chars.len(), shift),
                    _ => {}
                }
                self.sync_browser_panel();
            }
            return PlatformAction::None;
        }
        if alt {
            return PlatformAction::None;
        }
        if key == 0x1b {
            self.leave_browser_panel();
            return PlatformAction::None;
        }
        if key == 0x09 {
            let focus = panel.focus;
            let order = self.browser_focus_order();
            if !order.is_empty() {
                let current = order
                    .iter()
                    .position(|action| *action == focus)
                    .unwrap_or(0);
                let next = if shift {
                    (current + order.len() - 1) % order.len()
                } else {
                    (current + 1) % order.len()
                };
                self.browser_panel.as_mut().unwrap().focus = order[next];
                self.pending_surrogate = None;
                self.ensure_browser_focus_visible();
            }
            return PlatformAction::None;
        }
        let panel = self.browser_panel.as_mut().unwrap();
        if panel.focus == Action::RuleList {
            self.blocking_key(key);
            return PlatformAction::None;
        }
        if panel.focus == Action::List && matches!(panel.kind, PanelKind::History) {
            let selected = panel.history_selected;
            let matches = self.history_matches();
            let rows = self.bookmark_rows();
            if !matches.is_empty() {
                let index = selected.and_then(|id| matches.iter().position(|entry| *entry == id));
                if key == 0x0d && index.is_some() {
                    self.open_selected_history();
                } else if key == 0x2e && index.is_some() {
                    self.remove_selected_history();
                } else {
                    let next = match key {
                        0x26 => Some(index.map_or(matches.len() - 1, |i| i.saturating_sub(1))),
                        0x28 => Some(index.map_or(0, |i| (i + 1).min(matches.len() - 1))),
                        0x24 => Some(0),
                        0x23 => Some(matches.len() - 1),
                        0x21 => Some(index.map_or(0, |i| i.saturating_sub(rows))),
                        0x22 => Some(index.map_or(0, |i| (i + rows).min(matches.len() - 1))),
                        _ => None,
                    };
                    if let Some(next) = next {
                        self.browser_panel.as_mut().unwrap().history_selected = Some(matches[next]);
                        self.ensure_history_visible();
                    }
                }
            }
            return PlatformAction::None;
        }
        if self
            .browser_panel
            .as_ref()
            .is_some_and(|p| matches!(p.kind, PanelKind::Bookmarks) && p.focus == Action::List)
        {
            self.bookmark_tree_key(key);
            return PlatformAction::None;
        }
        let rows = self.bookmark_rows();
        let panel = self.browser_panel.as_mut().unwrap();
        if panel.focus == Action::List {
            let length = self.profile.data().bookmarks.len();
            if length > 0 {
                let index = panel.selected.unwrap_or(0);
                panel.selected = Some(match key {
                    0x26 => index.saturating_sub(1),
                    0x28 => (index + 1).min(length - 1),
                    0x24 => 0,
                    0x23 => length - 1,
                    0x21 => index.saturating_sub(rows),
                    0x22 => (index + rows).min(length - 1),
                    _ => index,
                });
                if key == 0x0d {
                    self.open_selected_bookmark();
                } else if key == 0x2e {
                    self.remove_selected_bookmark();
                } else {
                    self.ensure_bookmark_visible();
                }
            }
            return PlatformAction::None;
        }
        let kind = panel.kind;
        if let Some(editor) = panel.editor() {
            match key {
                0x08 => editor.backspace(),
                0x2e => editor.delete(),
                0x25 => editor.left(shift),
                0x27 => editor.right(shift),
                0x24 => editor.move_to(0, shift),
                0x23 => editor.move_to(editor.chars.len(), shift),
                0x0d => match kind {
                    PanelKind::EditBookmark { .. } => self.save_bookmark(),
                    PanelKind::EditFolder { .. } => self.save_folder(),
                    PanelKind::Settings => self.apply_browser_settings(),
                    PanelKind::BlockingRules => self.add_blocking_rule(),
                    PanelKind::HistorySearch => self.apply_history_search(),
                    _ => {}
                },
                _ => {}
            }
            self.sync_browser_panel();
        } else if matches!(key, 0x0d | 0x20) {
            let action = panel.focus;
            self.browser_activate(action);
        }
        PlatformAction::None
    }

    pub(super) fn browser_paste(&mut self, text: &str) {
        let Some(panel) = &mut self.browser_panel else {
            return;
        };
        let limit = match panel.focus {
            Action::Title => MAX_TITLE_BYTES,
            Action::Zoom => 3,
            Action::SearchField => 256,
            Action::RuleEntry => crate::blocking::MAX_RULE_BYTES,
            _ => MAX_ADDRESS_BYTES,
        };
        if let Some(editor) = panel.editor() {
            editor.insert(text, limit);
        }
        self.sync_browser_panel();
    }

    pub(super) fn browser_cut_copied(&mut self, text: &str) {
        if let Some(editor) = self.browser_panel.as_mut().and_then(BrowserPanel::editor) {
            if editor.selected() == text {
                editor.delete_selection();
            }
        }
        self.sync_browser_panel();
    }

    pub(super) fn browser_wheel(&mut self, delta: i32) {
        let count = if self
            .browser_panel
            .as_ref()
            .is_some_and(|panel| matches!(panel.kind, PanelKind::History))
        {
            self.history_matches().len()
        } else {
            self.bookmark_tree_items().len()
        };
        let rules = self
            .browser_panel
            .as_ref()
            .filter(|p| matches!(p.kind, PanelKind::BlockingRules));
        let count = rules.map_or(count, |p| p.blocking.rules.len());
        let rows = if rules.is_some() {
            self.blocking_rows()
        } else {
            self.bookmark_rows()
        };
        let max = (self.browser_content_height() - self.browser_body().h).max(0);
        if let Some(panel) = &mut self.browser_panel {
            panel.hover = None;
            panel.wheel_remainder = panel.wheel_remainder.saturating_add(delta);
            let steps = panel.wheel_remainder / 120;
            panel.wheel_remainder %= 120;
            if matches!(panel.kind, PanelKind::Bookmarks | PanelKind::History)
                || (matches!(panel.kind, PanelKind::BlockingRules)
                    && panel.focus == Action::RuleList)
            {
                panel.first = (panel.first as i32)
                    .saturating_sub(steps.saturating_mul(3))
                    .clamp(0, count.saturating_sub(rows) as i32)
                    as usize;
            } else {
                panel.scroll = panel
                    .scroll
                    .saturating_sub(steps.saturating_mul(48))
                    .clamp(0, max);
            }
        }
    }

    pub(super) fn browser_pointer_down(&mut self, x: i32, y: i32, shift: bool) -> bool {
        if y < self.chrome_bottom() {
            self.close_browser_panel();
            return false;
        }
        self.pending_surrogate = None;
        let action = self.browser_action_at(x, y);
        if let Some(
            action @ (Action::Row(_)
            | Action::FolderRow(_)
            | Action::HistoryRow(_)
            | Action::RuleRow(_)),
        ) = action
        {
            if let Some(panel) = &mut self.browser_panel {
                panel.cancel_pointer();
                panel.hover = Some(action);
            }
            self.browser_activate(action);
            return true;
        }
        let rect = action.and_then(|action| {
            self.browser_controls()
                .into_iter()
                .find(|control| control.action == action)
                .map(|control| control.rect)
        });
        if let Some(panel) = &mut self.browser_panel {
            panel.cancel_pointer();
            panel.hover = action;
            if let Some(action) = action {
                panel.focus = action;
                if let Some(editor) = panel.editor() {
                    let left = rect.unwrap().x;
                    let position = editor.offset + ((x - left - 8).max(0) / 12) as usize;
                    editor.move_to(position, shift);
                    panel.dragging_field = true;
                } else {
                    panel.pressed = Some(action);
                }
            }
        }
        self.sync_browser_panel();
        true
    }

    pub(super) fn browser_pointer_move(&mut self, x: i32, y: i32, dragging: bool) -> bool {
        let hover = self.browser_action_at(x, y);
        let Some(panel) = &mut self.browser_panel else {
            return false;
        };
        let changed = panel.hover != hover;
        panel.hover = hover;
        if !dragging || !panel.dragging_field {
            return changed;
        }
        let focus = panel.focus;
        let Some(rect) = self
            .browser_controls()
            .into_iter()
            .find(|control| control.action == focus)
            .map(|control| control.rect)
        else {
            return changed;
        };
        if let Some(editor) = self.browser_panel.as_mut().and_then(BrowserPanel::editor) {
            editor.move_to(
                editor.offset + ((x - rect.x - 8).max(0) / 12) as usize,
                true,
            );
        }
        self.sync_browser_panel();
        true
    }

    pub(super) fn browser_pointer_up(&mut self, x: i32, y: i32) {
        let action = self.browser_action_at(x, y);
        let pressed = self.browser_panel.as_mut().and_then(|panel| {
            let pressed = panel.pressed;
            panel.cancel_pointer();
            panel.hover = action;
            pressed
        });
        if let Some(pressed) = pressed.filter(|pressed| Some(*pressed) == action) {
            self.browser_activate(pressed);
        }
    }

    pub(super) fn browser_wants_hand(&self, x: i32, y: i32) -> bool {
        self.browser_action_at(x, y).is_some_and(|action| {
            !matches!(
                action,
                Action::Title
                    | Action::Address
                    | Action::Zoom
                    | Action::Home
                    | Action::SearchField
                    | Action::RuleEntry
                    | Action::LinkColor
                    | Action::VisitedColor
                    | Action::DownloadFolder
                    | Action::RuleList
                    | Action::List
            )
        })
    }

    pub(super) fn draw_browser_panel(&self, canvas: &mut Canvas) {
        let Some(panel) = &self.browser_panel else {
            return;
        };
        let palette = self.browser_palette();
        let bottom = self.content_bottom();
        canvas.set_clip(Some(crate::paint::ClipRect::new(
            0,
            self.chrome_bottom(),
            self.width,
            (bottom - self.chrome_bottom()).max(0),
        )));
        canvas.fill_rect(
            0,
            self.chrome_bottom(),
            self.width,
            bottom - self.chrome_bottom(),
            palette.background,
        );
        let body = self.browser_body();
        canvas.fill_rect(
            0,
            self.chrome_bottom(),
            self.width,
            self.browser_header_height() - 5,
            palette.panel,
        );
        canvas.fill_rect(
            0,
            bottom - self.browser_footer_height() + 2,
            self.width,
            self.browser_footer_height(),
            palette.panel,
        );
        canvas.fill_rect(
            body.x,
            bottom - self.browser_footer_height() + 2,
            body.w,
            1,
            palette.border,
        );
        let title = match panel.kind {
            PanelKind::Bookmarks => "Settings / Bookmarks",
            PanelKind::EditFolder { id: None } => "New bookmark folder",
            PanelKind::EditFolder { .. } => "Edit bookmark folder",
            PanelKind::FolderDestination => "Choose destination folder",
            PanelKind::DeleteFolder { .. } => "Delete bookmark folder",
            PanelKind::Settings => "Settings",
            PanelKind::BlockingRules => "Request blocking",
            PanelKind::History => "History",
            PanelKind::HistoryOptions => "History settings",
            PanelKind::HistorySearch => "Search history",
            PanelKind::HistoryClear => "Clear history",
            PanelKind::EditBookmark { index: None } => "Add bookmark",
            PanelKind::EditBookmark { .. } => "Edit bookmark",
        };
        let title_icon = match panel.kind {
            PanelKind::Bookmarks => Icon::Bookmarks,
            PanelKind::EditFolder { .. }
            | PanelKind::FolderDestination
            | PanelKind::DeleteFolder { .. } => Icon::Folder,
            PanelKind::Settings | PanelKind::HistoryOptions | PanelKind::BlockingRules => {
                Icon::Settings
            }
            PanelKind::History | PanelKind::HistoryClear => Icon::History,
            PanelKind::HistorySearch => Icon::Search,
            PanelKind::EditBookmark { .. } => Icon::Bookmark,
        };
        ui::rounded_rect(
            canvas,
            (body.x, self.chrome_bottom() + 13, 30, 30),
            9.0,
            palette.chrome,
        );
        ui::icon(
            canvas,
            title_icon,
            body.x + 5,
            self.chrome_bottom() + 18,
            20,
            palette.chrome_text,
            palette.accent,
        );
        let title_right = if matches!(panel.kind, PanelKind::Bookmarks) && self.width >= 760 {
            (self.width - 316).max(body.x)
        } else if matches!(panel.kind, PanelKind::Bookmarks | PanelKind::History) {
            (self.width - 198).max(body.x)
        } else {
            (self.width - 64).max(body.x)
        };
        canvas.text(
            body.x + 40,
            self.chrome_bottom() + 20,
            &fit_text(title, ((title_right - body.x - 40) / 12).max(0) as usize),
            2,
            palette.foreground,
        );
        let history_panel = matches!(
            panel.kind,
            PanelKind::History
                | PanelKind::HistoryOptions
                | PanelKind::HistorySearch
                | PanelKind::HistoryClear
        );
        let message = if !panel.error.is_empty() {
            panel.error.clone()
        } else if matches!(panel.kind, PanelKind::BlockingRules) {
            "Done returns to Settings. Apply saves your rules.".into()
        } else if matches!(panel.kind, PanelKind::HistorySearch) {
            "Title or address. Blank shows all visits.".into()
        } else if matches!(panel.kind, PanelKind::Bookmarks) {
            format!(
                "{} / {} item(s)",
                self.profile.data().folder_path(panel.folder),
                self.bookmark_tree_items().len()
            )
        } else if matches!(panel.kind, PanelKind::History) {
            let count = self.history_matches().len();
            let enabled = if self.browsing_history.data().policy.enabled {
                "on"
            } else {
                "off"
            };
            if panel.search.chars.is_empty() {
                format!("{count} visit(s). Recording {enabled}. Times are UTC.")
            } else {
                format!(
                    "{count} match(es). Recording {enabled}. Filter: {}",
                    panel.search.text()
                )
            }
        } else if (history_panel && self.browsing_history.path().is_none())
            || (!history_panel && self.profile.path().is_none())
        {
            "Changes last for this session.".into()
        } else {
            "Saved locally on this computer.".into()
        };
        if self.browser_header_height() >= PANEL_HEADER {
            canvas.text(
                body.x,
                self.chrome_bottom() + 46,
                &fit_text(&message, (body.w / 6).max(0) as usize),
                1,
                if panel.error.is_empty() {
                    palette.muted
                } else {
                    palette.error
                },
            );
        }
        canvas.set_clip(Some(crate::paint::ClipRect::new(
            body.x, body.y, body.w, body.h,
        )));
        match panel.kind {
            PanelKind::Bookmarks
            | PanelKind::EditFolder { .. }
            | PanelKind::FolderDestination
            | PanelKind::DeleteFolder { .. } => self.paint_bookmark_tree(canvas, body, panel),
            PanelKind::History => {
                ui::rounded_rect(canvas, body.tuple(), 8.0, palette.panel);
                let matches = self.history_matches();
                if matches.is_empty() {
                    ui::icon(
                        canvas,
                        Icon::History,
                        body.x + 14,
                        body.y + 10,
                        20,
                        palette.accent,
                        palette.accent,
                    );
                    canvas.text(
                        body.x + 46,
                        body.y + 10,
                        if panel.search.chars.is_empty() {
                            "No saved visits."
                        } else {
                            "No matching visits."
                        },
                        2,
                        palette.foreground,
                    );
                    let help = if !panel.search.chars.is_empty() {
                        "Search again with no text to show every visit."
                    } else if self.browsing_history.data().policy.enabled {
                        "Pages you open will appear here."
                    } else {
                        "Open Settings to enable recording."
                    };
                    canvas.text(
                        body.x + 10,
                        body.y + 33,
                        &fit_text(help, ((body.w - 22) / 6).max(0) as usize),
                        1,
                        palette.muted,
                    );
                }
                for (offset, id) in matches
                    .iter()
                    .skip(panel.first)
                    .take(self.bookmark_rows() + 1)
                    .enumerate()
                {
                    let Some(entry) = self
                        .browsing_history
                        .data()
                        .entries
                        .iter()
                        .find(|entry| entry.id == *id)
                    else {
                        continue;
                    };
                    let y = body.y + offset as i32 * ROW;
                    let selected = panel.history_selected == Some(*id);
                    let hover = panel.hover == Some(Action::HistoryRow(*id));
                    panel_list_row(canvas, body, y, selected, hover, palette);
                    ui::icon(
                        canvas,
                        Icon::History,
                        body.x + 14,
                        y + 14,
                        20,
                        if selected {
                            palette.accent
                        } else {
                            palette.muted
                        },
                        palette.accent,
                    );
                    canvas.text(
                        body.x + 46,
                        y + 8,
                        &fit_text(&entry.title, ((body.w - 60) / 12).max(0) as usize),
                        2,
                        palette.foreground,
                    );
                    let detail = format!("{}  {}", history_date(entry.timestamp), entry.url);
                    canvas.text(
                        body.x + 46,
                        y + 31,
                        &fit_text(&detail, ((body.w - 60) / 6).max(0) as usize),
                        1,
                        palette.muted,
                    );
                }
                let total = matches.len();
                if total > self.bookmark_rows() {
                    let thumb = (body.h * self.bookmark_rows() as i32 / total as i32)
                        .max(8)
                        .min(body.h);
                    let y = body.y
                        + (body.h - thumb) * panel.first as i32
                            / total.saturating_sub(self.bookmark_rows()).max(1) as i32;
                    ui::rounded_rect(
                        canvas,
                        (body.x + body.w - 5, body.y, 5, body.h),
                        2.5,
                        palette.background,
                    );
                    ui::rounded_rect(
                        canvas,
                        (body.x + body.w - 5, y, 5, thumb),
                        2.5,
                        palette.accent,
                    );
                }
            }
            PanelKind::HistorySearch => {
                canvas.text(
                    body.x,
                    body.y + 62 - panel.scroll,
                    "Leave blank to show all visits.",
                    1,
                    palette.muted,
                );
                canvas.text(
                    body.x,
                    body.y + 78 - panel.scroll,
                    "Search stays in this window.",
                    1,
                    palette.muted,
                );
            }
            PanelKind::HistoryOptions => {
                canvas.text(
                    body.x,
                    body.y + 58 - panel.scroll,
                    "Automatically remove after",
                    2,
                    palette.foreground,
                );
                canvas.text(
                    body.x,
                    body.y + 342 - panel.scroll,
                    "Turning recording off keeps earlier visits.",
                    1,
                    palette.muted,
                );
                canvas.text(
                    body.x,
                    body.y + 358 - panel.scroll,
                    "Apply removes visits older than your age limit.",
                    1,
                    palette.muted,
                );
                canvas.text(
                    body.x,
                    body.y + 374 - panel.scroll,
                    "Exit cleanup runs on a normal window close.",
                    1,
                    palette.muted,
                );
            }
            PanelKind::HistoryClear => {
                canvas.text(
                    body.x,
                    body.y + 5 - panel.scroll,
                    "Choose the visits to remove.",
                    2,
                    palette.foreground,
                );
                canvas.text(
                    body.x,
                    body.y + 32 - panel.scroll,
                    "Bookmarks and Back/Forward stay available.",
                    1,
                    palette.muted,
                );
                let now = crate::history::now();
                let cutoff = history_clear_cutoff(panel.clear_range, now);
                let count = self
                    .browsing_history
                    .data()
                    .entries
                    .iter()
                    .filter(|entry| {
                        entry.timestamp >= cutoff
                            && (panel.clear_range == crate::history::ClearRange::All
                                || entry.timestamp <= now)
                    })
                    .count();
                let note = format!("Clear history removes {count} matching visit(s).");
                canvas.text(body.x, body.y + 236 - panel.scroll, &note, 1, palette.muted);
                if self.browsing_history.problem().is_some() {
                    canvas.text(
                        body.x,
                        body.y + 256 - panel.scroll,
                        "All time also resets corrupt history settings.",
                        1,
                        palette.error,
                    );
                    canvas.text(
                        body.x,
                        body.y + 272 - panel.scroll,
                        "Recording will be off after that reset.",
                        1,
                        palette.error,
                    );
                } else {
                    canvas.text(
                        body.x,
                        body.y + 256 - panel.scroll,
                        "Cancel leaves these visits in history.",
                        1,
                        palette.muted,
                    );
                }
            }
            PanelKind::EditBookmark { .. } => {
                canvas.text(
                    body.x,
                    body.y + 7 - panel.scroll,
                    "Name",
                    2,
                    palette.foreground,
                );
                canvas.text(
                    body.x,
                    body.y + 87 - panel.scroll,
                    "Address",
                    2,
                    palette.foreground,
                );
                canvas.text(
                    body.x,
                    body.y + 184 - panel.scroll,
                    &fit_text(
                        &self.profile.data().folder_path(panel.folder),
                        (body.w / 6).max(0) as usize,
                    ),
                    1,
                    palette.muted,
                );
                canvas.text(
                    body.x,
                    body.y + 160 - panel.scroll,
                    "Only the address is saved; form data is not stored.",
                    1,
                    palette.muted,
                );
            }
            PanelKind::Settings => {
                self.paint_appearance_settings(canvas, body, panel);
                canvas.text(body.x,body.y+1158-panel.scroll,"Address-bar search engine",2,palette.foreground);
                canvas.text(
                    body.x,
                    body.y + 153 - panel.scroll,
                    "Default zoom (%)",
                    2,
                    palette.foreground,
                );
                canvas.text(
                    body.x,
                    body.y + 233 - panel.scroll,
                    "Home page",
                    2,
                    palette.foreground,
                );
                canvas.text(
                    body.x,
                    body.y + 355 - panel.scroll,
                    "JavaScript, images and blocking changes reload.",
                    1,
                    palette.muted,
                );
                canvas.text(
                    body.x,
                    body.y + 369 - panel.scroll,
                    "Home changes apply when you use Home or start again.",
                    1,
                    palette.muted,
                );
            }
            PanelKind::BlockingRules => self.paint_blocking_rules(canvas, body, panel),
        }
        for control in self.browser_controls() {
            if matches!(control.action, Action::List | Action::RuleList) {
                continue;
            }
            let content = self.browser_body_action(control.action);
            canvas.set_clip(Some(if content {
                crate::paint::ClipRect::new(body.x, body.y, body.w, body.h)
            } else {
                crate::paint::ClipRect::new(
                    0,
                    self.chrome_bottom(),
                    self.width,
                    (bottom - self.chrome_bottom()).max(0),
                )
            }));
            let focused = panel.focus == control.action;
            let hovered = panel.hover == Some(control.action);
            let pressed = panel.pressed == Some(control.action) && hovered;
            let editor = match control.action {
                Action::Title => Some(&panel.title),
                Action::Address | Action::Home | Action::RuleEntry => Some(&panel.address),
                Action::Zoom => Some(&panel.zoom),
                Action::LinkColor => Some(&panel.link_color),
                Action::VisitedColor => Some(&panel.visited_color),
                Action::DownloadFolder => Some(&panel.download_folder),
                Action::SearchField => Some(&panel.search),
                _ => None,
            };
            if let Some(editor) = editor {
                field_themed(canvas, editor, control.rect.tuple(), focused, palette);
                if hovered && !focused {
                    ui::rounded_border(canvas, control.rect.tuple(), 7.0, 1.0, palette.accent);
                }
            } else if matches!(
                control.action,
                Action::JavaScript
                    | Action::Images
                    | Action::DarkMode
                    | Action::DarkPages
                    | Action::BookmarkBar
                    | Action::BlockingEnabled
                    | Action::BuiltinBlocking
                    | Action::RecordHistory
                    | Action::ClearOnExit
                    | Action::Retention(_)
                    | Action::SearchEngine(_)
                    | Action::ClearRange(_)
            ) {
                let checked = match control.action {
                    Action::JavaScript => panel.javascript,
                    Action::Images => panel.images,
                    Action::SearchEngine(engine) => panel.search_engine==engine,
                    Action::DarkMode => panel.dark_mode,
                    Action::DarkPages => panel.dark_pages,
                    Action::BookmarkBar => panel.bookmark_bar,
                    Action::BlockingEnabled => panel.blocking.enabled,
                    Action::BuiltinBlocking => panel.blocking.builtins,
                    Action::RecordHistory => panel.history_policy.enabled,
                    Action::ClearOnExit => panel.history_policy.clear_on_exit,
                    Action::Retention(days) => panel.history_policy.retention_days == days,
                    Action::ClearRange(range) => panel.clear_range == range,
                    _ => false,
                };
                ui::rounded_rect(
                    canvas,
                    control.rect.tuple(),
                    7.0,
                    if hovered || pressed {
                        palette.selected
                    } else {
                        palette.panel
                    },
                );
                ui::rounded_border(
                    canvas,
                    control.rect.tuple(),
                    7.0,
                    if focused { 2.0 } else { 1.0 },
                    if focused {
                        palette.accent
                    } else {
                        palette.border
                    },
                );
                let radio = matches!(control.action, Action::Retention(_) | Action::ClearRange(_) | Action::SearchEngine(_));
                let marker = (control.rect.x + 9, control.rect.y + 8, 20, 20);
                let radius = if radio { 10.0 } else { 5.0 };
                ui::rounded_rect(
                    canvas,
                    marker,
                    radius,
                    if checked && !radio {
                        palette.chrome
                    } else {
                        palette.panel
                    },
                );
                ui::rounded_border(
                    canvas,
                    marker,
                    radius,
                    1.0,
                    if checked {
                        palette.accent
                    } else {
                        palette.muted
                    },
                );
                if checked && radio {
                    ui::rounded_rect(
                        canvas,
                        (control.rect.x + 15, control.rect.y + 14, 8, 8),
                        4.0,
                        palette.accent,
                    );
                } else if checked {
                    ui::icon(
                        canvas,
                        Icon::Check,
                        control.rect.x + 11,
                        control.rect.y + 10,
                        16,
                        palette.chrome_text,
                        palette.accent,
                    );
                }
                let scale = if control.label.chars().count() as i32 * 12 <= control.rect.w - 50 {
                    2
                } else {
                    1
                };
                canvas.text(
                    control.rect.x + 40,
                    control.rect.y + (control.rect.h - 7 * scale) / 2,
                    &fit_text(
                        control.label,
                        ((control.rect.w - 50) / (6 * scale)).max(0) as usize,
                    ),
                    scale,
                    palette.foreground,
                );
            } else {
                panel_button(canvas, &control, focused, hovered, pressed, palette);
                if let Action::DestinationRow(id) = control.action {
                    canvas.text(
                        control.rect.x + 12,
                        control.rect.y + 10,
                        &fit_text(
                            &self.profile.data().folder_path(id),
                            ((control.rect.w - 24) / 6).max(0) as usize,
                        ),
                        1,
                        palette.foreground,
                    );
                }
            }
        }
        if panel.focus == Action::List {
            canvas.set_clip(Some(crate::paint::ClipRect::new(
                body.x, body.y, body.w, body.h,
            )));
            ui::rounded_border(canvas, body.tuple(), 8.0, 1.5, palette.accent);
        }
        let content_height = self.browser_content_height();
        if content_height > body.h {
            canvas.set_clip(Some(crate::paint::ClipRect::new(
                0,
                self.chrome_bottom(),
                self.width,
                (bottom - self.chrome_bottom()).max(0),
            )));
            let thumb = (body.h * body.h / content_height).max(12).min(body.h);
            let y = body.y + (body.h - thumb) * panel.scroll / (content_height - body.h);
            ui::rounded_rect(
                canvas,
                (body.x + body.w + 8, body.y, 4, body.h),
                2.0,
                palette.border,
            );
            ui::rounded_rect(
                canvas,
                (body.x + body.w + 8, y, 4, thumb),
                2.0,
                palette.accent,
            );
        }
        canvas.set_clip(None);
    }
}

fn panel_list_row(
    canvas: &mut Canvas,
    body: Rect,
    y: i32,
    selected: bool,
    hovered: bool,
    palette: ui::Palette,
) {
    if selected || hovered {
        ui::rounded_rect(
            canvas,
            (body.x + 3, y + 2, body.w - 11, ROW - 4),
            7.0,
            if selected {
                palette.selected
            } else {
                palette.background
            },
        );
    }
    if selected {
        ui::rounded_rect(
            canvas,
            (body.x + 4, y + 10, 3, ROW - 20),
            1.5,
            palette.accent,
        );
    }
    canvas.fill_rect(
        body.x + 46,
        y + ROW - 1,
        (body.w - 60).max(0),
        1,
        palette.border,
    );
}

fn panel_button(
    canvas: &mut Canvas,
    control: &Control,
    focused: bool,
    hovered: bool,
    pressed: bool,
    palette: ui::Palette,
) {
    let primary = matches!(
        control.action,
        Action::Add
            | Action::Open
            | Action::SaveBookmark
            | Action::SaveFolder
            | Action::Apply
            | Action::ApplySearch
            | Action::ApplyHistory
    );
    let destructive = matches!(
        control.action,
        Action::Delete | Action::ConfirmClear | Action::ConfirmDeleteFolder
    );
    let (background, foreground) = if !control.enabled {
        (palette.background, palette.muted)
    } else if pressed {
        (
            if destructive {
                palette.error
            } else {
                palette.accent
            },
            palette.chrome_text,
        )
    } else if primary {
        (
            if hovered {
                ui::NAVY_LIGHT
            } else {
                palette.chrome
            },
            palette.chrome_text,
        )
    } else if destructive {
        (palette.panel_alt, palette.error)
    } else {
        (
            if hovered {
                palette.selected
            } else {
                palette.panel
            },
            palette.foreground,
        )
    };
    let rect = control.rect;
    ui::rounded_rect(canvas, rect.tuple(), 7.0, background);
    ui::rounded_border(
        canvas,
        rect.tuple(),
        7.0,
        1.0,
        if destructive && hovered {
            palette.error
        } else if primary && control.enabled {
            palette.chrome
        } else {
            palette.border
        },
    );
    let icon = match control.action {
        Action::Close | Action::Cancel | Action::CancelBookmark => Some(Icon::Close),
        Action::Add => Some(Icon::Plus),
        Action::NewFolder | Action::Destination => Some(Icon::Folder),
        Action::FolderUp | Action::MoveUp => Some(Icon::Up),
        Action::MoveDown => Some(Icon::Down),
        Action::Delete | Action::ConfirmDeleteFolder => Some(Icon::Trash),
        Action::Move => Some(Icon::Forward),
        Action::SettingsBack => Some(Icon::Settings),
        Action::Open => Some(Icon::Forward),
        Action::SaveBookmark | Action::SaveFolder | Action::Apply | Action::ApplyHistory => {
            Some(Icon::Check)
        }
        Action::HistorySearch | Action::ApplySearch => Some(Icon::Search),
        Action::HistoryOptions => Some(Icon::Settings),
        Action::CurrentHome | Action::BundledHome => Some(Icon::Home),
        _ => None,
    };
    if control.action == Action::Close {
        ui::icon(
            canvas,
            Icon::Close,
            rect.x + (rect.w - 18) / 2,
            rect.y + (rect.h - 18) / 2,
            18,
            foreground,
            palette.accent,
        );
    } else {
        let scale = if rect.w >= 160 && control.label.chars().count() as i32 * 12 + 40 <= rect.w {
            2
        } else {
            1
        };
        let icon = icon.filter(|_| control.label.chars().count() as i32 * 6 * scale + 40 <= rect.w);
        let icon_space = if icon.is_some() { 24 } else { 0 };
        let text = fit_text(
            control.label,
            ((rect.w - 16 - icon_space) / (6 * scale)).max(0) as usize,
        );
        let width = text.chars().count() as i32 * 6 * scale + icon_space;
        let x = rect.x + (rect.w - width) / 2;
        if let Some(icon) = icon {
            ui::icon(
                canvas,
                icon,
                x,
                rect.y + (rect.h - 16) / 2,
                16,
                foreground,
                if control.enabled {
                    palette.accent
                } else {
                    palette.muted
                },
            );
        }
        canvas.text(
            x + icon_space,
            rect.y + (rect.h - 7 * scale) / 2,
            &text,
            scale,
            foreground,
        );
    }
    if focused && control.enabled {
        ui::rounded_border(
            canvas,
            (rect.x + 2, rect.y + 2, rect.w - 4, rect.h - 4),
            5.0,
            2.0,
            palette.accent,
        );
    }
}

fn history_clear_cutoff(range: crate::history::ClearRange, now: u64) -> u64 {
    match range {
        crate::history::ClearRange::Hour => now.saturating_sub(3600),
        crate::history::ClearRange::Day => now.saturating_sub(86400),
        crate::history::ClearRange::Week => now.saturating_sub(7 * 86400),
        crate::history::ClearRange::All => 0,
    }
}

fn history_date(timestamp: u64) -> String {
    // Gregorian civil dates use 400-year eras. UTC is explicit in the UI;
    // displaying a timestamp does not require a system timezone lookup.
    if timestamp > 253_402_300_799 {
        return "Date outside display range".into();
    }
    let days = (timestamp / 86400) as i64 + 719468;
    let era = days / 146097;
    let day_of_era = days - era * 146097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let march_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * march_month + 2) / 5 + 1;
    let month = march_month + if march_month < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    let hour = timestamp % 86400 / 3600;
    let minute = timestamp % 3600 / 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}

// Page titles are untrusted suggestions. Normalize their whitespace, discard
// controls, and stop before growing an editor beyond the persisted byte limit.
fn bookmark_title(value: &str, fallback: &str) -> String {
    let mut title = String::with_capacity(value.len().min(MAX_TITLE_BYTES));
    let mut space = false;
    for ch in value.chars() {
        if ch.is_whitespace() {
            space = !title.is_empty();
        } else if !ch.is_control() {
            if title.len() + usize::from(space) + ch.len_utf8() > MAX_TITLE_BYTES {
                break;
            }
            if space {
                title.push(' ');
            }
            title.push(ch);
            space = false;
        }
    }
    if title.is_empty() {
        for ch in fallback.chars() {
            if title.len() + ch.len_utf8() > MAX_TITLE_BYTES {
                break;
            }
            title.push(ch);
        }
    }
    title
}

#[cfg(test)]
#[path = "app_browser_ui_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "app_history_ui_tests.rs"]
mod history_tests;
