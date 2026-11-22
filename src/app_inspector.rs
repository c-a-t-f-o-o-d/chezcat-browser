//! ChezCat's original developer inspector. It reads the actual document,
//! renderer and activity log; the console enters the existing bounded runtime.
//! Both the dock and the auxiliary Windows window use these same controls.
use super::*;
use crate::paint::ClipRect;
use std::cell::{Cell, Ref, RefCell};
use std::collections::HashSet;
use std::fmt::Write;

const HEADER: i32 = 38;
const TOOLS: i32 = 34;
const FOOTER: i32 = 24;
const COMMAND: i32 = 36;
const MAX_LINES: usize = 20_000;
const MAX_VIEW_BYTES: usize = 2 * 1024 * 1024;
const MAX_COMMAND: usize = crate::scripting::MAX_CONSOLE_SOURCE;
const MAX_HISTORY: usize = 64;
const MAX_HISTORY_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Mode {
    #[default]
    Closed,
    Docked,
    Detached,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Tab {
    #[default]
    Elements,
    Sources,
    Console,
    Network,
}

impl Tab {
    const ALL: [Self; 4] = [Self::Elements, Self::Sources, Self::Console, Self::Network];
    fn title(self) -> &'static str {
        match self {
            Self::Elements => "Elements",
            Self::Sources => "Sources",
            Self::Console => "Console",
            Self::Network => "Network",
        }
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|tab| *tab == self).unwrap()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Pane {
    #[default]
    Content,
    Details,
    Filter,
    Command,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Tab(Tab),
    Dock,
    Close,
    Refresh,
    Copy,
    PreviousSource,
    NextSource,
    Collapse,
    Expand,
    ClearConsole,
    Run,
    Row(usize, bool),
    Request(u64),
}

#[derive(Clone, Copy)]
struct Rect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

struct NetworkGeometry {
    table_header: Rect,
    table_rows: Rect,
    divider: Rect,
    axis: Rect,
    lanes: Rect,
    details: Rect,
}

impl Rect {
    fn has(self, x: i32, y: i32) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x.saturating_add(self.w)
            && y < self.y.saturating_add(self.h)
    }
    fn tuple(self) -> (i32, i32, i32, i32) {
        (self.x, self.y, self.w, self.h)
    }
    fn clip(self) -> ClipRect {
        ClipRect::new(self.x, self.y, self.w, self.h)
    }
}

struct Row {
    text: String,
    node: Option<usize>,
    depth: usize,
    children: bool,
    number: usize,
    color: u32,
}

#[derive(Default)]
struct Snapshot {
    rows: Vec<Row>,
    details: Vec<String>,
    label: String,
    total: usize,
    truncated: bool,
    network: Option<crate::network_trace::Snapshot>,
}

#[derive(Clone)]
pub(super) struct PlatformInspectorState {
    identity: std::rc::Rc<()>,mode:Mode,focused:bool,tab:Tab,pane:Pane,
}
impl PartialEq for PlatformInspectorState {
    fn eq(&self,other:&Self)->bool {std::rc::Rc::ptr_eq(&self.identity,&other.identity)
        &&self.mode==other.mode&&self.focused==other.focused&&self.tab==other.tab&&self.pane==other.pane}
}
impl Eq for PlatformInspectorState {}

pub(super) struct Inspector {
    platform_identity: std::rc::Rc<()>,
    pub mode: Mode,
    pub focused: bool,
    pub height: i32,
    tab: Tab,
    pane: Pane,
    font_scale: i32,
    network_font_scale: i32,
    scroll: usize,
    detail_scroll: usize,
    horizontal: usize,
    wheel_remainder: i32,
    selected: Option<usize>,
    network_selected: Option<u64>,
    network_split: i32,
    network_resize: bool,
    network_revision: Cell<u64>,
    collapsed: HashSet<usize>,
    filter: Editor,
    command: Editor,
    history: Vec<String>,
    history_position: Option<usize>,
    history_draft: String,
    pressed: Option<Action>,
    resize_drag: Option<(i32, i32)>,
    scrollbar_drag: Option<(bool, i32)>,
    auxiliary_size: (i32, i32),
    source_index: usize,
    scripts: Vec<(String, String)>,
    notice: String,
    dirty: Cell<bool>,
    snapshot: RefCell<Snapshot>,
}

impl Default for Inspector {
    fn default() -> Self {
        Self {
            platform_identity: std::rc::Rc::new(()),
            mode: Mode::Closed,
            focused: false,
            height: 330,
            tab: Tab::Elements,
            pane: Pane::Content,
            font_scale: 2,
            network_font_scale: 1,
            scroll: 0,
            detail_scroll: 0,
            horizontal: 0,
            wheel_remainder: 0,
            selected: None,
            network_selected: None,
            network_split: 54,
            network_resize: false,
            network_revision: Cell::new(0),
            collapsed: HashSet::new(),
            filter: Editor::default(),
            command: Editor::default(),
            history: Vec::new(),
            history_position: None,
            history_draft: String::new(),
            pressed: None,
            resize_drag: None,
            scrollbar_drag: None,
            auxiliary_size: (1000, 620),
            source_index: 0,
            scripts: Vec::new(),
            notice: String::new(),
            dirty: Cell::new(true),
            snapshot: RefCell::new(Snapshot::default()),
        }
    }
}

impl Inspector {
    pub(super) fn network_visible(&self) -> bool {
        self.mode != Mode::Closed && self.tab == Tab::Network
    }
    pub(super) fn invalidate(&self) {
        self.dirty.set(true);
    }
    pub(super) fn navigation(&mut self, scripts: &[crate::loader::Script]) {
        self.cancel_pointer();
        self.scripts.clear();
        let mut bytes = 0usize;
        for script in scripts.iter().take(16) {
            if script.source.len() > 64 * 1024
                || bytes.saturating_add(script.source.len()) > 256 * 1024
            {
                break;
            }
            bytes += script.source.len();
            self.scripts
                .push((truncate(&script.label, 512), script.source.clone()));
        }
        self.selected = None;
        self.network_selected = None;
        self.collapsed.clear();
        self.source_index = 0;
        self.scroll = 0;
        self.detail_scroll = 0;
        self.horizontal = 0;
        self.notice.clear();
        self.invalidate();
    }
    pub(super) fn platform_state(&self)->PlatformInspectorState {
        PlatformInspectorState {identity:self.platform_identity.clone(),mode:self.mode,focused:self.focused,tab:self.tab,pane:self.pane}
    }
    pub(super) fn platform_editor(&self)->Option<&Editor> {
        if self.mode==Mode::Closed||!self.focused {return None;}
        match self.pane {Pane::Filter=>Some(&self.filter),Pane::Command=>Some(&self.command),_=>None}
    }
    pub(super) fn cancel_pointer(&mut self) {
        self.pressed = None;
        self.resize_drag = None;
        self.scrollbar_drag = None;
        self.network_resize = false;
    }
    fn line_height(&self) -> i32 {
        9 * self.font_scale + 4
    }
    fn active_scale(&mut self) -> &mut i32 {
        if self.tab == Tab::Network {
            &mut self.network_font_scale
        } else {
            &mut self.font_scale
        }
    }
}

impl App {
    /// The inspector does not navigate, reload, or replace the inspected page.
    pub fn open_inspector(&mut self) {
        self.invalidate_platform_requests();
        // A native Settings/bookmark/history draft can stay above the dock.
        // Opening developer tools must not silently discard that staged edit.
        if self.view != View::Page {
            self.toggle_view(self.view);
        }
        self.menu = false;
        self.reset_chrome();
        self.select_popup = None;
        self.drag = Drag::None;
        self.inspector.mode = Mode::Docked;
        self.inspector.focused = true;
        self.inspector.invalidate();
        self.reflow();
    }

    pub fn toggle_inspector(&mut self) {
        if self.inspector.mode == Mode::Closed {
            self.open_inspector();
        } else {
            self.close_inspector();
        }
    }

    pub fn close_inspector(&mut self) {
        self.invalidate_platform_requests();
        let was_docked = self.inspector.mode == Mode::Docked;
        self.inspector.mode = Mode::Closed;
        self.inspector.focused = false;
        self.inspector.cancel_pointer();
        *self.inspector.snapshot.borrow_mut() = Snapshot::default();
        self.inspector.invalidate();
        if was_docked {
            self.reflow();
        }
    }

    pub fn inspector_detached(&self) -> bool {
        self.inspector.mode == Mode::Detached
    }

    /// Select a view for the native UI or reproducible headless inspection.
    pub fn inspector_tab_by_name(&mut self, name: &str) -> bool {
        let Some(tab) = Tab::ALL
            .into_iter()
            .find(|tab| tab.title().eq_ignore_ascii_case(name))
        else {
            return false;
        };
        self.inspector_tab(tab);
        true
    }

    pub fn detach_inspector(&mut self) {
        if self.inspector.mode == Mode::Closed {
            self.open_inspector();
        }
        if self.inspector.mode != Mode::Detached {
            self.inspector_activate(Action::Dock);
        }
    }

    pub fn inspector_focus(&mut self) {
        if !self.inspector.focused { self.invalidate_platform_requests(); }
        self.pending_surrogate = None;
        self.inspector.focused = true;
    }

    pub fn inspector_utf16_input(&mut self, unit: u16) {
        self.invalidate_platform_requests();
        // An auxiliary-window character must never edit the address bar or a
        // page field, even after Escape moves focus out of its command editor.
        self.inspector.focused = true;
        self.utf16_input(unit);
    }

    pub fn main_window_focus(&mut self) {
        self.pending_surrogate = None;
        if self.inspector.mode == Mode::Detached {
            if self.inspector.focused { self.invalidate_platform_requests(); }
            self.inspector.focused = false;
        }
    }

    pub fn inspector_cancel_pointer(&mut self) {
        self.pending_surrogate = None;
        self.inspector.cancel_pointer();
    }

    pub fn inspector_detach_failed(&mut self, error: String) {
        self.inspector.mode = Mode::Docked;
        self.inspector.notice =
            format!("Could not open inspector window: {}", truncate(&error, 240));
        self.reflow();
    }

    pub fn inspector_window_size(&mut self, width: i32, height: i32) {
        self.inspector.auxiliary_size = (width.clamp(1, 8192), height.clamp(1, 8192));
        self.inspector_sync_editor();
    }

    pub(super) fn inspector_dock_height(&self) -> i32 {
        if self.inspector.mode != Mode::Docked {
            return 0;
        }
        let available = (self.height - self.content_top() - FOOT_HEIGHT - 80).max(0);
        self.inspector.height.clamp(140, 2000).min(available)
    }

    pub fn content_bottom(&self) -> i32 {
        self.height - FOOT_HEIGHT - self.inspector_dock_height()
    }

    fn inspector_rect(&self) -> Rect {
        if self.inspector.mode == Mode::Detached {
            Rect {
                x: 0,
                y: 0,
                w: self.inspector.auxiliary_size.0,
                h: self.inspector.auxiliary_size.1,
            }
        } else {
            Rect {
                x: 0,
                y: self.content_bottom(),
                w: self.width,
                h: self.inspector_dock_height(),
            }
        }
    }

    fn inspector_body(&self) -> Rect {
        let outer = self.inspector_rect();
        let command = if self.inspector.tab == Tab::Console {
            COMMAND
        } else {
            0
        };
        Rect {
            x: outer.x,
            y: outer.y + HEADER + TOOLS,
            w: outer.w,
            h: (outer.h - HEADER - TOOLS - FOOTER - command).max(0),
        }
    }

    fn inspector_panes(&self) -> (Rect, Option<Rect>) {
        let body = self.inspector_body();
        if self.inspector.tab == Tab::Network {
            return (self.network_geometry().table_rows, None);
        }
        if self.inspector.tab == Tab::Elements && body.w >= 360 {
            let left = (body.w * 45 / 100).clamp(160, body.w - 160);
            (
                Rect { w: left, ..body },
                Some(Rect {
                    x: left + 1,
                    w: body.w - left - 1,
                    ..body
                }),
            )
        } else {
            (body, None)
        }
    }

    fn inspector_filter_rect(&self) -> Rect {
        let outer = self.inspector_rect();
        Rect {
            x: outer.x + 8,
            y: outer.y + HEADER + 4,
            w: (outer.w - 226).max(70),
            h: TOOLS - 8,
        }
    }

    fn inspector_command_rect(&self) -> Rect {
        let outer = self.inspector_rect();
        Rect {
            x: outer.x + 8,
            y: outer.y + outer.h - FOOTER - COMMAND + 4,
            w: (outer.w - 78).max(30),
            h: COMMAND - 8,
        }
    }

    fn inspector_buttons(&self) -> Vec<(Action, Rect, &'static str)> {
        let outer = self.inspector_rect();
        let mut buttons = Vec::with_capacity(11);
        let tab_width = ((outer.w - 114) / 4).clamp(46, 94);
        for (i, tab) in Tab::ALL.iter().enumerate() {
            buttons.push((
                Action::Tab(*tab),
                Rect {
                    x: outer.x + 6 + i as i32 * tab_width,
                    y: outer.y + 7,
                    w: tab_width - 3,
                    h: HEADER - 12,
                },
                tab.title(),
            ));
        }
        buttons.push((
            Action::Dock,
            Rect {
                x: outer.x + outer.w - 102,
                y: outer.y + 7,
                w: 66,
                h: HEADER - 12,
            },
            if self.inspector.mode == Mode::Detached {
                "Dock"
            } else {
                "Detach"
            },
        ));
        buttons.push((
            Action::Close,
            Rect {
                x: outer.x + outer.w - 31,
                y: outer.y + 7,
                w: 25,
                h: HEADER - 12,
            },
            "X",
        ));
        let y = outer.y + HEADER + 4;
        buttons.push((
            Action::Copy,
            Rect {
                x: outer.w - 209,
                y,
                w: 66,
                h: TOOLS - 8,
            },
            "Copy view",
        ));
        buttons.push((
            Action::Refresh,
            Rect {
                x: outer.w - 139,
                y,
                w: 55,
                h: TOOLS - 8,
            },
            "Refresh",
        ));
        match self.inspector.tab {
            Tab::Elements => {
                buttons.push((
                    Action::Collapse,
                    Rect {
                        x: outer.w - 80,
                        y,
                        w: 35,
                        h: TOOLS - 8,
                    },
                    "- All",
                ));
                buttons.push((
                    Action::Expand,
                    Rect {
                        x: outer.w - 41,
                        y,
                        w: 35,
                        h: TOOLS - 8,
                    },
                    "+ All",
                ));
            }
            Tab::Sources => {
                buttons.push((
                    Action::PreviousSource,
                    Rect {
                        x: outer.w - 80,
                        y,
                        w: 35,
                        h: TOOLS - 8,
                    },
                    "Prev",
                ));
                buttons.push((
                    Action::NextSource,
                    Rect {
                        x: outer.w - 41,
                        y,
                        w: 35,
                        h: TOOLS - 8,
                    },
                    "Next",
                ));
            }
            Tab::Console => {
                buttons.push((
                    Action::ClearConsole,
                    Rect {
                        x: outer.w - 80,
                        y,
                        w: 74,
                        h: TOOLS - 8,
                    },
                    "Clear log",
                ));
                let command = self.inspector_command_rect();
                buttons.push((
                    Action::Run,
                    Rect {
                        x: outer.w - 62,
                        y: command.y,
                        w: 54,
                        h: command.h,
                    },
                    "Run",
                ));
            }
            Tab::Network => {}
        }
        buttons
    }

    fn inspector_snapshot(&self) -> Ref<'_, Snapshot> {
        if self.inspector.tab == Tab::Network {
            let revision = self.network.revision();
            if self.inspector.network_revision.get() != revision {
                self.inspector.invalidate();
            }
        }
        if self.inspector.dirty.get() {
            // Workers can add network measurements during a paint. Nested
            // geometry reads keep one coherent snapshot until the next paint.
            if let Ok(mut current) = self.inspector.snapshot.try_borrow_mut() {
                let snapshot = build_snapshot(self);
                if let Some(network) = &snapshot.network {
                    self.inspector.network_revision.set(network.revision);
                }
                *current = snapshot;
                self.inspector.dirty.set(false);
            }
        }
        self.inspector.snapshot.borrow()
    }

    fn inspector_visible_rows(&self) -> usize {
        if self.inspector.tab == Tab::Network {
            let area = self.network_geometry();
            return (area.table_rows.h / self.network_row_height())
                .min(area.lanes.h / self.network_lane_height())
                .max(1) as usize;
        }
        (self.inspector_body().h / self.inspector.line_height()).max(1) as usize
    }

    fn inspector_max_scroll(&self, details: bool) -> usize {
        let snapshot = self.inspector_snapshot();
        let count = if details {
            snapshot.details.len()
        } else {
            snapshot.rows.len()
        };
        count.saturating_sub(self.inspector_visible_rows())
    }

    fn inspector_scrollbar(&self, rect: Rect, details: bool) -> Option<Rect> {
        let max = self.inspector_max_scroll(details);
        if max == 0 || rect.h <= 0 {
            return None;
        }
        let count = max + self.inspector_visible_rows();
        let thumb = ((rect.h as usize * self.inspector_visible_rows()) / count)
            .max(18)
            .min(rect.h as usize) as i32;
        let offset = if details {
            self.inspector.detail_scroll
        } else {
            self.inspector.scroll
        }
        .min(max);
        let top = rect.y + ((rect.h - thumb) as i64 * offset as i64 / max as i64) as i32;
        Some(Rect {
            x: rect.x + rect.w - 10,
            y: top,
            w: 10,
            h: thumb,
        })
    }

    fn inspector_hit(&self, x: i32, y: i32) -> Option<Action> {
        if let Some((action, _, _)) = self
            .inspector_buttons()
            .iter()
            .find(|(_, rect, _)| rect.has(x, y))
        {
            return Some(*action);
        }
        let (body, _) = self.inspector_panes();
        if self.inspector.tab == Tab::Network {
            let area = self.network_geometry();
            let index =
                if area.table_rows.has(x, y) && x < area.table_rows.x + area.table_rows.w - 12 {
                    Some(((y - area.table_rows.y) / self.network_row_height()) as usize)
                } else if area.lanes.has(x, y) {
                    Some(((y - area.lanes.y) / self.network_lane_height()) as usize)
                } else {
                    None
                };
            if let Some(index) = index.filter(|index| *index < self.inspector_visible_rows()) {
                let snapshot = self.inspector_snapshot();
                let first = self.inspector.scroll.min(self.inspector_max_scroll(false));
                return snapshot
                    .rows
                    .get(first + index)
                    .map(|row| Action::Request((row.number - 1) as u64));
            }
        }
        if self.inspector.tab == Tab::Elements && body.has(x, y) && x < body.x + body.w - 12 {
            let row = self.inspector.scroll.min(self.inspector_max_scroll(false))
                + ((y - body.y) / self.inspector.line_height()) as usize;
            let snapshot = self.inspector_snapshot();
            if let Some(item) = snapshot.rows.get(row) {
                let expand = item.children && x < body.x + 22 + item.depth.min(16) as i32 * 10;
                return item.node.map(|node| Action::Row(node, expand));
            }
        }
        None
    }

    pub fn inspector_resize_cursor(&self, x: i32, y: i32) -> bool {
        !self.menu
            && ((self.inspector.mode != Mode::Closed
                && self.inspector.tab == Tab::Network
                && self.network_geometry().divider.has(x, y))
                || (self.inspector.mode == Mode::Docked
                    && self.inspector_rect().has(x, y)
                    && y < self.inspector_rect().y + 5))
    }

    pub fn inspector_wants_hand(&self, x: i32, y: i32) -> bool {
        self.inspector.mode != Mode::Closed && self.inspector_hit(x, y).is_some()
    }

    pub(super) fn inspector_main_pointer_down(&mut self, x: i32, y: i32) -> bool {
        if self.inspector.mode != Mode::Docked || !self.inspector_rect().has(x, y) {
            self.inspector.focused = false;
            return false;
        }
        self.inspector_pointer_down(x, y);
        true
    }

    pub fn inspector_pointer_down(&mut self, x: i32, y: i32) {
        self.invalidate_platform_requests();
        self.pending_surrogate = None;
        self.inspector.focused = true;
        self.focus = Focus::Page;
        self.select_popup = None;
        self.drag = Drag::None;
        self.inspector.cancel_pointer();
        if self.inspector.tab == Tab::Network && self.network_geometry().divider.has(x, y) {
            self.inspector.network_resize = true;
            return;
        }
        if self.inspector_resize_cursor(x, y) {
            self.inspector.resize_drag = Some((y, self.inspector_dock_height()));
            return;
        }
        if self.inspector_filter_rect().has(x, y) {
            self.inspector.pane = Pane::Filter;
            self.inspector.filter.select_all();
            return;
        }
        if self.inspector.tab == Tab::Console && self.inspector_command_rect().has(x, y) {
            self.inspector.pane = Pane::Command;
            let rect = self.inspector_command_rect();
            let cursor = self.inspector.command.offset
                + ((x - rect.x - 20).max(0) / (6 * self.inspector.font_scale)) as usize;
            self.inspector.command.move_to(cursor, false);
            return;
        }
        let (left, right) = self.inspector_panes();
        self.inspector.pane = if right.is_some_and(|rect| rect.has(x, y)) {
            Pane::Details
        } else {
            Pane::Content
        };
        for (rect, details) in [(Some(left), false), (right, true)] {
            let Some(rect) = rect else {
                continue;
            };
            if rect.has(x, y) && x >= rect.x + rect.w - 12 {
                if let Some(thumb) = self.inspector_scrollbar(rect, details) {
                    if thumb.has(x, y) {
                        self.inspector.scrollbar_drag = Some((details, y - thumb.y));
                    } else {
                        let amount =
                            self.inspector_visible_rows() as i32 * if y < thumb.y { -1 } else { 1 };
                        self.inspector_scroll(amount, details);
                    }
                }
                return;
            }
        }
        self.inspector.pressed = self.inspector_hit(x, y);
    }

    pub fn inspector_pointer_move(&mut self, _x: i32, y: i32, dragging: bool) -> bool {
        if !dragging {
            return false;
        }
        if self.inspector.network_resize {
            let body = self.inspector_body();
            let available = (self.network_geometry().details.y - body.y - 6).max(1);
            self.inspector.network_split = ((y - body.y) * 100 / available).clamp(28, 75);
            return true;
        }
        if let Some((start_y, height)) = self.inspector.resize_drag {
            let height = height
                .saturating_add(start_y.saturating_sub(y))
                .clamp(140, 2000);
            if self.inspector.height != height {
                self.inspector.height = height;
                self.reflow();
                return true;
            }
        }
        if let Some((details, offset)) = self.inspector.scrollbar_drag {
            let (left, right) = self.inspector_panes();
            let rect = if details { right.unwrap_or(left) } else { left };
            if let Some(thumb) = self.inspector_scrollbar(rect, details) {
                let max = self.inspector_max_scroll(details);
                let top = (y - offset - rect.y).clamp(0, (rect.h - thumb.h).max(0));
                let scroll = (top as i64 * max as i64 / (rect.h - thumb.h).max(1) as i64) as usize;
                if details {
                    self.inspector.detail_scroll = scroll;
                } else {
                    self.inspector.scroll = scroll;
                }
                return true;
            }
        }
        false
    }

    pub fn inspector_pointer_up(&mut self, x: i32, y: i32) -> PlatformAction {
        self.invalidate_platform_requests();
        let pressed = self.inspector.pressed.take();
        self.inspector.cancel_pointer();
        if let Some(action) = pressed.filter(|action| Some(*action) == self.inspector_hit(x, y)) {
            return self.inspector_activate(action);
        }
        PlatformAction::None
    }

    fn inspector_activate(&mut self, action: Action) -> PlatformAction {
        match action {
            Action::Tab(tab) => self.inspector_tab(tab),
            Action::Dock => {
                self.inspector.mode = if self.inspector.mode == Mode::Detached {
                    Mode::Docked
                } else {
                    Mode::Detached
                };
                self.inspector.cancel_pointer();
                self.reflow();
            }
            Action::Close => self.close_inspector(),
            Action::Refresh => {
                self.inspector.invalidate();
                self.inspector.notice.clear();
            }
            Action::Copy => return PlatformAction::Copy(self.inspector_copy()),
            Action::PreviousSource | Action::NextSource => {
                let count = 1 + self.inspector.scripts.len() + self.styles.len();
                self.inspector.source_index = if action == Action::NextSource {
                    (self.inspector.source_index + 1) % count
                } else {
                    (self.inspector.source_index + count - 1) % count
                };
                self.inspector.scroll = 0;
                self.inspector.horizontal = 0;
                self.inspector.invalidate();
            }
            Action::Collapse => {
                self.inspector.collapsed = self
                    .document
                    .nodes
                    .iter()
                    .enumerate()
                    .filter_map(|(node, item)| {
                        (node != 0 && !item.children.is_empty()).then_some(node)
                    })
                    .collect();
                self.inspector.scroll = 0;
                self.inspector.invalidate();
            }
            Action::Expand => {
                self.inspector.collapsed.clear();
                self.inspector.scroll = 0;
                self.inspector.invalidate();
            }
            Action::Run => self.inspector_run_command(),
            Action::ClearConsole => {
                if let Some(page) = &mut self.scripting {
                    page.clear_console();
                }
                self.inspector.scroll = 0;
                self.inspector.notice =
                    "Console log cleared; page state and error count are unchanged.".into();
                self.inspector.invalidate();
            }
            Action::Row(node, expand) => {
                if self.document.nodes.get(node).is_some() {
                    self.inspector.selected = Some(node);
                    self.inspector.detail_scroll = 0;
                    if expand && !self.inspector.collapsed.remove(&node) {
                        self.inspector.collapsed.insert(node);
                    }
                    self.inspector.invalidate();
                }
            }
            Action::Request(id) => {
                self.inspector.network_selected = Some(id);
                self.inspector.invalidate();
            }
        }
        PlatformAction::None
    }

    fn inspector_tab(&mut self, tab: Tab) {
        self.invalidate_platform_requests();
        self.inspector.tab = tab;
        self.inspector.scroll = 0;
        self.inspector.detail_scroll = 0;
        self.inspector.horizontal = 0;
        self.inspector.filter.set("");
        self.inspector.pane = if tab == Tab::Console {
            Pane::Command
        } else {
            Pane::Content
        };
        self.inspector.notice.clear();
        self.inspector.invalidate();
    }

    fn inspector_scroll(&mut self, rows: i32, details: bool) {
        let max = self.inspector_max_scroll(details);
        let previous = if details {
            self.inspector.detail_scroll
        } else {
            self.inspector.scroll
        };
        let next = (previous as i64 + rows as i64).clamp(0, max as i64) as usize;
        if details {
            self.inspector.detail_scroll = next;
        } else {
            self.inspector.scroll = next;
        }
    }

    pub fn wheel_at(&mut self, x: i32, y: i32, delta: i32, ctrl: bool, shift: bool) {
        if !self.menu && self.inspector.mode == Mode::Docked && self.inspector_rect().has(x, y) {
            self.inspector_wheel(x, y, delta, ctrl, shift);
        } else {
            self.wheel(delta, ctrl);
        }
    }

    pub fn inspector_wheel(&mut self, x: i32, y: i32, delta: i32, ctrl: bool, shift: bool) {
        if ctrl {
            let scale = self.inspector.active_scale();
            *scale = (*scale + delta.signum()).clamp(1, 3);
            self.inspector_sync_editor();
            return;
        }
        self.inspector.wheel_remainder = self.inspector.wheel_remainder.saturating_add(delta);
        let steps = self.inspector.wheel_remainder / 120;
        self.inspector.wheel_remainder %= 120;
        if shift {
            self.inspector.horizontal = (self.inspector.horizontal as i64 - i64::from(steps) * 8)
                .clamp(0, MAX_VIEW_BYTES as i64) as usize;
        } else {
            let details = self.inspector_panes().1.is_some_and(|rect| rect.has(x, y));
            self.inspector_scroll(steps.saturating_mul(-3), details);
        }
    }

    pub fn inspector_key(
        &mut self,
        key: u32,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> PlatformAction {
        self.invalidate_platform_requests();
        if alt {
            return PlatformAction::None;
        }
        if key == 0x7b || ctrl && shift && key == 0x49 {
            self.close_inspector();
            return PlatformAction::None;
        }
        if ctrl {
            match key {
                0x09 => {
                    let index = (self.inspector.tab.index() + if shift { 3 } else { 1 }) % 4;
                    self.inspector_tab(Tab::ALL[index]);
                }
                0x46 => {
                    self.inspector.pane = Pane::Filter;
                    self.inspector.filter.select_all();
                }
                0x43 | 0x58 => {
                    let selected = match self.inspector.pane {
                        Pane::Filter => self.inspector.filter.selected(),
                        Pane::Command => self.inspector.command.selected(),
                        _ => String::new(),
                    };
                    let text = if selected.is_empty() {
                        self.inspector_copy()
                    } else {
                        selected
                    };
                    return if key == 0x58
                        && matches!(self.inspector.pane, Pane::Filter | Pane::Command)
                    {
                        PlatformAction::Cut(text)
                    } else {
                        PlatformAction::Copy(text)
                    };
                }
                0x56 if matches!(self.inspector.pane, Pane::Filter | Pane::Command) => {
                    return PlatformAction::Paste
                }
                0x41 => match self.inspector.pane {
                    Pane::Filter => self.inspector.filter.select_all(),
                    Pane::Command => self.inspector.command.select_all(),
                    _ => {}
                },
                0xbb | 0x6b => {
                    let scale = self.inspector.active_scale();
                    *scale = (*scale + 1).min(3);
                }
                0xbd | 0x6d => {
                    let scale = self.inspector.active_scale();
                    *scale = (*scale - 1).max(1);
                }
                0x30 | 0x60 => {
                    let reset = if self.inspector.tab == Tab::Network {
                        1
                    } else {
                        2
                    };
                    *self.inspector.active_scale() = reset;
                }
                0x24 => self.inspector.scroll = 0,
                0x23 => self.inspector.scroll = self.inspector_max_scroll(false),
                _ => {}
            }
            self.inspector_sync_editor();
            return PlatformAction::None;
        }
        if key == 0x1b {
            if self.inspector.pane == Pane::Filter && !self.inspector.filter.chars.is_empty() {
                self.inspector.filter.set("");
                self.inspector.scroll = 0;
                self.inspector.invalidate();
            } else {
                self.inspector.focused = self.inspector.mode == Mode::Detached;
                self.inspector.pane = Pane::Content;
            }
            return PlatformAction::None;
        }
        if key == 0x09 {
            self.inspector.pane = match self.inspector.pane {
                Pane::Filter => {
                    if self.inspector.tab == Tab::Console {
                        Pane::Command
                    } else {
                        Pane::Content
                    }
                }
                Pane::Command => Pane::Content,
                _ => Pane::Filter,
            };
            return PlatformAction::None;
        }
        if self.inspector.pane == Pane::Command && matches!(key, 0x26 | 0x28) {
            self.inspector_history(key == 0x26);
            return PlatformAction::None;
        }
        if matches!(self.inspector.pane, Pane::Filter | Pane::Command) {
            if key == 0x0d {
                if self.inspector.pane == Pane::Command && shift {
                    self.inspector_paste("\n");
                } else if self.inspector.pane == Pane::Command {
                    self.inspector_run_command();
                } else {
                    self.inspector.pane = Pane::Content;
                }
                return PlatformAction::None;
            }
            let editor = if self.inspector.pane == Pane::Filter {
                &mut self.inspector.filter
            } else {
                &mut self.inspector.command
            };
            match key {
                0x08 => editor.backspace(),
                0x2e => editor.delete(),
                0x25 => editor.left(shift),
                0x27 => editor.right(shift),
                0x24 => editor.move_to(0, shift),
                0x23 => editor.move_to(editor.chars.len(), shift),
                _ => {}
            }
            if self.inspector.pane == Pane::Filter {
                self.inspector.scroll = 0;
                self.inspector.invalidate();
            }
            self.inspector_sync_editor();
            return PlatformAction::None;
        }
        match key {
            0x26 => self.inspector_scroll(-1, self.inspector.pane == Pane::Details),
            0x28 => self.inspector_scroll(1, self.inspector.pane == Pane::Details),
            0x21 => self.inspector_scroll(
                -(self.inspector_visible_rows() as i32),
                self.inspector.pane == Pane::Details,
            ),
            0x22 | 0x20 => self.inspector_scroll(
                self.inspector_visible_rows() as i32 * if shift { -1 } else { 1 },
                self.inspector.pane == Pane::Details,
            ),
            0x24 => {
                self.inspector.scroll = 0;
                self.inspector.detail_scroll = 0;
            }
            0x23 => {
                if self.inspector.pane == Pane::Details {
                    self.inspector.detail_scroll = self.inspector_max_scroll(true);
                } else {
                    self.inspector.scroll = self.inspector_max_scroll(false);
                }
            }
            0x25 => self.inspector.horizontal = self.inspector.horizontal.saturating_sub(8),
            0x27 => self.inspector.horizontal = (self.inspector.horizontal + 8).min(MAX_VIEW_BYTES),
            _ => {}
        }
        PlatformAction::None
    }

    pub fn inspector_paste(&mut self, text: &str) {
        self.invalidate_platform_requests();
        match self.inspector.pane {
            Pane::Filter => {
                self.inspector.filter.insert(text, 128);
                self.inspector.scroll = 0;
                self.inspector.invalidate();
            }
            Pane::Command => {
                insert_command(&mut self.inspector.command, text);
            }
            _ => {}
        }
        self.inspector_sync_editor();
    }

    pub fn inspector_cut_copied(&mut self, text: &str) {
        self.invalidate_platform_requests();
        match self.inspector.pane {
            Pane::Filter if self.inspector.filter.selected() == text => {
                self.inspector.filter.delete_selection();
                self.inspector.invalidate();
            }
            Pane::Command if self.inspector.command.selected() == text => {
                self.inspector.command.delete_selection();
            }
            _ => {}
        }
        self.inspector_sync_editor();
    }

    pub(super) fn inspector_sync_editor(&mut self) {
        let filter = ((self.inspector_filter_rect().w - 16) / 6).max(1) as usize;
        let command = ((self.inspector_command_rect().w - 30) / (6 * self.inspector.font_scale))
            .max(1) as usize;
        self.inspector.filter.ensure_visible(filter);
        self.inspector.command.ensure_visible(command);
    }

    fn inspector_history(&mut self, previous: bool) {
        if self.inspector.history.is_empty() {
            return;
        }
        let position = match (self.inspector.history_position, previous) {
            (None, true) => {
                self.inspector.history_draft = self.inspector.command.text();
                Some(self.inspector.history.len() - 1)
            }
            (Some(0), true) => Some(0),
            (Some(position), true) => Some(position - 1),
            (Some(position), false) if position + 1 < self.inspector.history.len() => {
                Some(position + 1)
            }
            _ => None,
        };
        self.inspector.history_position = position;
        let text = position
            .map(|index| self.inspector.history[index].as_str())
            .unwrap_or(&self.inspector.history_draft)
            .to_owned();
        self.inspector.command.set(&text);
        self.inspector_sync_editor();
    }

    fn inspector_run_command(&mut self) {
        let source = self.inspector.command.text();
        let source = source.trim();
        if source.is_empty() {
            return;
        }
        if source.len() > MAX_COMMAND {
            self.inspector.notice = "Console commands are limited to 8 KiB.".into();
            return;
        }
        if self
            .inspector
            .history
            .last()
            .is_none_or(|previous| previous != source)
        {
            self.inspector.history.push(source.to_owned());
            while self.inspector.history.len() > MAX_HISTORY
                || self
                    .inspector
                    .history
                    .iter()
                    .map(String::len)
                    .sum::<usize>()
                    > MAX_HISTORY_BYTES
            {
                self.inspector.history.remove(0);
            }
        }
        self.inspector.command.set("");
        self.inspector.history_position = None;
        self.inspector.history_draft.clear();
        self.inspector.notice.clear();
        self.update_script_geometry();
        if let Some(page) = &mut self.scripting {
            let outcome = page.console_eval(&mut self.document, &mut self.forms, source);
            if outcome.layout_changed {
                self.reflow();
            }
            self.validate_control_focus();
            self.sync_editors();
            self.show_script_errors();
        } else {
            self.inspector.notice = if self.scripts_enabled {
                "This document has no active JavaScript runtime."
            } else {
                "JavaScript is disabled. Enable it in Settings to run console commands."
            }
            .into();
        }
        self.inspector.pane = Pane::Command;
        self.inspector.invalidate();
        self.inspector.scroll = self.inspector_max_scroll(false);
    }

    fn inspector_copy(&self) -> String {
        if self.inspector.tab == Tab::Network {
            return self.network_copy();
        }
        let snapshot = self.inspector_snapshot();
        let mut output = String::new();
        for row in &snapshot.rows {
            if output
                .len()
                .saturating_add(row.text.len())
                .saturating_add(1)
                > MAX_VIEW_BYTES
            {
                break;
            }
            output.push_str(&row.text);
            output.push('\n');
        }
        if self.inspector.tab == Tab::Elements {
            output.push_str("\nSelected node\n");
            for line in &snapshot.details {
                if output.len().saturating_add(line.len()).saturating_add(1) > MAX_VIEW_BYTES {
                    break;
                }
                output.push_str(line);
                output.push('\n');
            }
        }
        output
    }

    /// Hardware navigation uses the same history path as the toolbar. Cancel
    /// any pending left-button action first so a later release cannot follow it.
    pub fn mouse_navigation(&mut self, forward: bool) {
        self.pointer_cancel();
        self.select_popup = None;
        self.history_move(if forward { 1 } else { -1 });
    }

    pub fn draw_inspector_window(&self) -> Canvas {
        let (width, height) = self.inspector.auxiliary_size;
        let mut canvas = Canvas::new(width as usize, height as usize);
        self.draw_inspector(&mut canvas);
        canvas
    }

    fn network_row_height(&self) -> i32 {
        (7 * self.inspector.network_font_scale + 8).max(18)
    }
    fn network_lane_height(&self) -> i32 {
        (self.network_row_height() - 5).max(12)
    }

    fn network_geometry(&self) -> NetworkGeometry {
        let body = self.inspector_body();
        let detail_height = if body.h >= 200 {
            64
        } else if body.h >= 130 {
            42
        } else {
            0
        };
        let available = (body.h - detail_height - 6).max(0);
        let upper = (available * self.inspector.network_split / 100).clamp(0, available);
        let header_height = 21.min(upper);
        let divider = Rect {
            y: body.y + upper,
            h: 6.min(body.h),
            ..body
        };
        let graph_y = divider.y + divider.h;
        let graph_h = (body.y + available + 6 - graph_y).max(0);
        let axis_h = 28.min(graph_h);
        NetworkGeometry {
            table_header: Rect {
                h: header_height,
                ..body
            },
            table_rows: Rect {
                y: body.y + header_height,
                h: upper - header_height,
                ..body
            },
            divider,
            axis: Rect {
                y: graph_y,
                h: axis_h,
                ..body
            },
            lanes: Rect {
                y: graph_y + axis_h,
                h: (graph_h - axis_h).max(0),
                ..body
            },
            details: Rect {
                y: body.y + body.h - detail_height,
                h: detail_height,
                ..body
            },
        }
    }

    fn network_copy(&self) -> String {
        let snapshot = self.inspector_snapshot();
        let mut text = format!(
            "DISPLAYED DOCUMENT: {}\nCONTENT TYPE: {}\nTRUST: {}\nLAST FAILED NAVIGATION: {}\n\nMeasured HTTP requests for the latest navigation (times from its monotonic start):\n",
            self.location.display(), self.document.content_type, self.trust.description(),
            self.last_failed_navigation.as_deref().unwrap_or("None"));
        if let Some(network) = &snapshot.network {
            let now = if network.rows.iter().any(|r| r.end.is_none()) {
                self.network.elapsed()
            } else {
                network.elapsed
            };
            for row in &snapshot.rows {
                let request = &network.rows[row.number - 1];
                let _ = writeln!(text, "#{} {} {}\n  {} | {} | {} | start {} | total {} | HTTP received {} bytes | encoded body {} | decoded body {}{}",
                    request.id + 1, request.method, request.url,
                    request.kind, network_status(request), network_protocol(request),
                    milliseconds(request.start), milliseconds(request.duration(now)),
                    request.http_bytes, optional_bytes(request.encoded_bytes), optional_bytes(request.decoded_bytes),
                    if request.reused_connection { " | connection reused" } else { "" });
                for phase in &request.phases {
                    let _ = writeln!(
                        text,
                        "  {}: start {}, duration {}{}",
                        phase.kind.label(),
                        milliseconds(phase.start),
                        milliseconds(phase.end.unwrap_or(now).saturating_sub(phase.start)),
                        if phase.end.is_none() {
                            " (in progress)"
                        } else {
                            ""
                        }
                    );
                }
                if !request.note.is_empty() {
                    let _ = writeln!(text, "  {}", request.note);
                }
            }
            let _ = writeln!(text, "{} recorded; {} omitted at the bounded trace limit.\nHTTP byte counts include received headers/framing, exclude TLS/TCP overhead. Missing phases were not measured; literal addresses skip DNS.\n", network.rows.len(), network.omitted);
        }
        text.push_str("Current displayed page activity (Ctrl+I):\n");
        let query = self.inspector.filter.text().to_lowercase();
        for event in &self.events {
            if !query.is_empty() && !event.to_lowercase().contains(&query) {
                continue;
            }
            if text.len().saturating_add(event.len() + 2) > MAX_VIEW_BYTES {
                break;
            }
            text.push_str(event);
            text.push_str("\n\n");
        }
        text
    }

    fn draw_network(&self, canvas: &mut Canvas) {
        use crate::network_trace::{Outcome, PhaseKind};
        let palette = self.browser_palette();
        let outer = self.inspector_rect();
        let clip = canvas.clip();
        let area = self.network_geometry();
        let snapshot = self.inspector_snapshot();
        let Some(network) = &snapshot.network else {
            return;
        };
        let now = if network.rows.iter().any(|r| r.end.is_none()) {
            self.network.elapsed()
        } else {
            network.elapsed
        };
        let first = self.inspector.scroll.min(
            snapshot
                .rows
                .len()
                .saturating_sub(self.inspector_visible_rows()),
        );
        let visible = snapshot
            .rows
            .iter()
            .skip(first)
            .take(self.inspector_visible_rows())
            .collect::<Vec<_>>();
        let scale = self.inspector.network_font_scale;
        let columns = network_columns(area.table_header.w, scale);
        canvas.fill_rect(
            area.table_header.x,
            area.table_header.y,
            area.table_header.w,
            area.table_header.h,
            palette.panel_alt,
        );
        canvas.set_clip(Some(area.table_header.clip().intersect(clip)));
        for (x, width, label) in &columns {
            canvas.text(
                area.table_header.x + x + 5,
                area.table_header.y + 7,
                &fit_text(label, ((width - 8) / 6).max(0) as usize),
                1,
                palette.muted,
            );
        }
        canvas.set_clip(Some(area.table_rows.clip().intersect(clip)));
        for (lane, row) in visible.iter().enumerate() {
            let request = &network.rows[row.number - 1];
            let selected = Some(request.id) == self.inspector.network_selected;
            let y = area.table_rows.y + lane as i32 * self.network_row_height();
            canvas.set_clip(Some(area.table_rows.clip().intersect(clip)));
            canvas.fill_rect(
                area.table_rows.x,
                y,
                area.table_rows.w - 11,
                self.network_row_height(),
                if selected {
                    palette.selected
                } else if (first + lane) % 2 == 1 {
                    palette.panel_alt
                } else {
                    palette.panel
                },
            );
            let color = match request.outcome {
                Outcome::Failed => palette.error,
                _ if request.status.is_some_and(|status| status >= 400) => palette.error,
                Outcome::Blocked => palette.muted,
                _ => palette.foreground,
            };
            for (x, width, label) in &columns {
                let value = match *label {
                    "#" => (request.id + 1).to_string(),
                    "Request" => network_name(&request.url).to_owned(),
                    "Method" => request.method.to_owned(),
                    "Status" => network_status(request),
                    "Type" => request.kind.to_lowercase(),
                    "Received" => {
                        if matches!(request.outcome, Outcome::Blocked | Outcome::Reused) {
                            "--".into()
                        } else {
                            compact_bytes(request.http_bytes)
                        }
                    }
                    "Time" => {
                        if matches!(request.outcome, Outcome::Blocked | Outcome::Reused) {
                            "--".into()
                        } else {
                            milliseconds(request.duration(now))
                        }
                    }
                    _ => String::new(),
                };
                let field = Rect {
                    x: area.table_rows.x + x + 5,
                    y,
                    w: width - 8,
                    h: self.network_row_height(),
                };
                canvas.set_clip(Some(
                    field
                        .clip()
                        .intersect(area.table_rows.clip())
                        .intersect(clip),
                ));
                let text = if *label == "Request" {
                    horizontal_slice(
                        &value,
                        self.inspector.horizontal,
                        ((width - 8) / (6 * scale)).max(0) as usize,
                    )
                } else {
                    fit_text(&value, ((width - 8) / (6 * scale)).max(0) as usize)
                };
                canvas.text(field.x, y + 4, &text, scale, color);
            }
        }
        canvas.set_clip(Some(area.table_rows.clip().intersect(clip)));
        if snapshot.rows.is_empty() {
            canvas.text(
                area.table_rows.x + 8,
                area.table_rows.y + 5,
                &fit_text(
                    if network.rows.is_empty() {
                        "No HTTP requests recorded. Open a website to measure its requests."
                    } else {
                        "No requests match the filter."
                    },
                    ((area.table_rows.w - 20) / 6).max(0) as usize,
                ),
                1,
                palette.muted,
            );
        }
        if let Some(thumb) = self.inspector_scrollbar(area.table_rows, false) {
            ui::rounded_rect(
                canvas,
                (thumb.x + 2, thumb.y, 6, thumb.h),
                3.0,
                palette.muted,
            );
        }
        canvas.set_clip(Some(outer.clip().intersect(clip)));
        canvas.fill_rect(
            area.divider.x,
            area.divider.y,
            area.divider.w,
            area.divider.h,
            palette.border,
        );
        canvas.fill_rect(
            area.divider.x + area.divider.w / 2 - 18,
            area.divider.y + 2,
            36,
            2,
            palette.muted,
        );
        canvas.fill_rect(
            area.axis.x,
            area.axis.y,
            area.axis.w,
            area.axis.h,
            palette.panel_alt,
        );
        canvas.fill_rect(
            area.lanes.x,
            area.lanes.y,
            area.lanes.w,
            area.lanes.h,
            palette.background,
        );
        let label_width = (area.lanes.w / 4).clamp(74, 160);
        let plot_x = area.lanes.x + label_width;
        let plot_w = (area.lanes.w - label_width - 16).max(1);
        let span = network
            .rows
            .iter()
            .map(|r| r.end.unwrap_or(now))
            .max()
            .unwrap_or_default()
            .as_secs_f64()
            .max(0.001);
        canvas.set_clip(Some(area.axis.clip().intersect(clip)));
        canvas.text(
            area.axis.x + 7,
            area.axis.y + 4,
            "TIMELINE",
            1,
            palette.foreground,
        );
        let legend = [
            (PhaseKind::Queue, "Queue"),
            (PhaseKind::Dns, "DNS"),
            (PhaseKind::Connect, "TCP"),
            (PhaseKind::Tls, "TLS"),
            (PhaseKind::Send, "Send"),
            (PhaseKind::Wait, "Wait"),
            (PhaseKind::Receive, "Receive"),
            (PhaseKind::Decode, "Decode"),
        ];
        let mut legend_x = plot_x;
        for (phase, label) in legend {
            if legend_x + label.len() as i32 * 6 + 15 > outer.w - 8 {
                break;
            }
            canvas.fill_rect(legend_x, area.axis.y + 3, 6, 7, network_phase_color(phase));
            canvas.text(legend_x + 9, area.axis.y + 3, label, 1, palette.muted);
            legend_x += label.len() as i32 * 6 + 22;
        }
        let ticks = if plot_w >= 550 { 5 } else { 3 };
        for tick in 0..=ticks {
            let x = plot_x + plot_w * tick / ticks;
            let label = format!(
                "{:.0}ms",
                span * 1000.0 * f64::from(tick) / f64::from(ticks)
            );
            canvas.set_clip(Some(area.axis.clip().intersect(clip)));
            canvas.text(
                (x - if tick == ticks {
                    label.len() as i32 * 6
                } else {
                    0
                })
                .max(plot_x),
                area.axis.y + 17,
                &label,
                1,
                palette.muted,
            );
            canvas.set_clip(Some(area.lanes.clip().intersect(clip)));
            canvas.fill_rect(x, area.lanes.y, 1, area.lanes.h, palette.border);
        }
        for (lane, row) in visible.iter().enumerate() {
            let request = &network.rows[row.number - 1];
            let selected = Some(request.id) == self.inspector.network_selected;
            let y = area.lanes.y + lane as i32 * self.network_lane_height();
            canvas.set_clip(Some(area.lanes.clip().intersect(clip)));
            if selected {
                canvas.fill_rect(
                    area.lanes.x,
                    y,
                    area.lanes.w,
                    self.network_lane_height(),
                    palette.selected,
                );
            }
            canvas.text(
                area.lanes.x + 7,
                y + 3,
                &fit_text(
                    &format!("{} {}", request.id + 1, network_name(&request.url)),
                    ((label_width - 12) / 6).max(0) as usize,
                ),
                1,
                if selected {
                    palette.foreground
                } else {
                    palette.muted
                },
            );
            let position = |time: std::time::Duration| {
                plot_x
                    + (time.as_secs_f64() / span * f64::from(plot_w)).clamp(0.0, f64::from(plot_w))
                        as i32
            };
            let start = position(request.start);
            let end = position(request.end.unwrap_or(now));
            let bar_height = (self.network_lane_height() - 5).max(5);
            if request.phases.is_empty() {
                canvas.fill_rect(
                    start,
                    y + 2,
                    3,
                    bar_height,
                    if request.outcome == Outcome::Failed {
                        palette.error
                    } else {
                        palette.muted
                    },
                );
            } else {
                canvas.fill_rect(
                    start,
                    y + 2,
                    (end - start).max(1),
                    bar_height,
                    palette.border,
                );
                for phase in &request.phases {
                    let x = position(phase.start);
                    let end = position(phase.end.unwrap_or(now));
                    canvas.fill_rect(
                        x,
                        y + 2,
                        (end - x).max(1),
                        bar_height,
                        network_phase_color(phase.kind),
                    );
                }
            }
            if selected {
                ui::rounded_border(
                    canvas,
                    (start - 1, y + 1, (end - start).max(3) + 2, bar_height + 2),
                    1.0,
                    1.0,
                    palette.foreground,
                );
            }
        }
        canvas.set_clip(Some(area.details.clip().intersect(clip)));
        canvas.fill_rect(
            area.details.x,
            area.details.y,
            area.details.w,
            area.details.h,
            palette.panel_alt,
        );
        canvas.fill_rect(
            area.details.x,
            area.details.y,
            area.details.w,
            1,
            palette.border,
        );
        let selected = self
            .inspector
            .network_selected
            .and_then(|id| network.rows.get(id as usize));
        let detail_lines = if let Some(request) = selected {
            network_details(request, now)
        } else {
            vec!["Select a request above or a timeline row for its measured phases. Drag the divider to resize.".into(),
            "Received = HTTP headers + framing + encoded body; TLS/TCP overhead is excluded. Ctrl+I keeps the full activity log.".into()]
        };
        for (index, line) in detail_lines.iter().enumerate() {
            canvas.text(
                area.details.x + 8,
                area.details.y + 6 + index as i32 * 14,
                &horizontal_slice(
                    line,
                    self.inspector.horizontal,
                    ((area.details.w - 16) / 6).max(0) as usize,
                ),
                1,
                if index == 0 {
                    palette.foreground
                } else {
                    palette.muted
                },
            );
        }
        canvas.set_clip(Some(outer.clip().intersect(clip)));
        let bottom = outer.y + outer.h - FOOTER;
        canvas.fill_rect(outer.x, bottom, outer.w, FOOTER, palette.chrome);
        let received: usize = network.rows.iter().map(|r| r.http_bytes).sum();
        let footer = format!("{} / {} requests | {} received | {} total{} | Filter: URL/type/status | Ctrl+C copies measurements + activity",
            snapshot.rows.len(), network.rows.len(), compact_bytes(received), milliseconds(std::time::Duration::from_secs_f64(span)),
            if network.omitted != 0 { format!(" | {} omitted", network.omitted) } else { String::new() });
        canvas.text(
            outer.x + 8,
            bottom + 8,
            &fit_text(&footer, ((outer.w - 16) / 6).max(0) as usize),
            1,
            palette.chrome_text,
        );
        canvas.set_clip(Some(clip));
    }

    pub(super) fn draw_inspector(&self, canvas: &mut Canvas) {
        if self.inspector.mode == Mode::Closed {
            return;
        }
        let outer = self.inspector_rect();
        let palette = self.browser_palette();
        if outer.h <= 0 || outer.w <= 0 {
            return;
        }
        let original_clip = canvas.clip();
        canvas.set_clip(Some(outer.clip()));
        canvas.fill_rect(outer.x, outer.y, outer.w, outer.h, palette.panel);
        canvas.fill_rect(outer.x, outer.y, outer.w, HEADER, ui::NAVY);
        canvas.fill_rect(outer.x, outer.y, outer.w, 3, ui::TEAL);
        canvas.fill_rect(
            outer.x,
            outer.y + HEADER,
            outer.w,
            TOOLS,
            palette.background,
        );
        for (action, rect, label) in self.inspector_buttons() {
            let active = matches!(action, Action::Tab(tab) if tab == self.inspector.tab);
            let header = rect.y < outer.y + HEADER;
            let pressed = self.inspector.pressed == Some(action);
            ui::rounded_rect(
                canvas,
                rect.tuple(),
                4.0,
                if pressed {
                    palette.accent
                } else if active {
                    ui::TEAL
                } else if header {
                    ui::NAVY_LIGHT
                } else {
                    palette.panel
                },
            );
            if !header {
                ui::rounded_border(canvas, rect.tuple(), 4.0, 1.0, palette.border);
            }
            let color = if active {
                ui::NAVY
            } else if header || pressed {
                ui::CREAM
            } else {
                palette.foreground
            };
            let text = fit_text(label, ((rect.w - 4) / 6).max(0) as usize);
            canvas.text(
                rect.x + (rect.w - text.chars().count() as i32 * 6) / 2,
                rect.y + (rect.h - 7) / 2,
                &text,
                1,
                color,
            );
        }
        draw_editor(
            canvas,
            self.inspector_filter_rect(),
            &self.inspector.filter,
            "Filter / Ctrl+F",
            self.inspector.focused && self.inspector.pane == Pane::Filter,
            false,
            1,
            palette,
        );
        if self.inspector.tab == Tab::Network {
            self.draw_network(canvas);
            canvas.set_clip(Some(original_clip));
            return;
        }
        let snapshot = self.inspector_snapshot();
        let (left, right) = self.inspector_panes();
        let scale = self.inspector.font_scale;
        let line_height = self.inspector.line_height();
        let first = self.inspector.scroll.min(
            snapshot
                .rows
                .len()
                .saturating_sub(self.inspector_visible_rows()),
        );
        canvas.set_clip(Some(left.clip().intersect(original_clip)));
        for (index, row) in snapshot
            .rows
            .iter()
            .enumerate()
            .skip(first)
            .take(self.inspector_visible_rows() + 1)
        {
            let y = left.y + (index - first) as i32 * line_height;
            let selected = row.node.is_some() && row.node == self.inspector.selected;
            if selected {
                canvas.fill_rect(left.x, y, left.w - 10, line_height, palette.selected);
            }
            let mut x = left.x + 8;
            if self.inspector.tab == Tab::Elements {
                x += row.depth.min(16) as i32 * 10;
                if row.children {
                    canvas.text(
                        x,
                        y + 4,
                        if row
                            .node
                            .is_some_and(|node| self.inspector.collapsed.contains(&node))
                        {
                            "+"
                        } else {
                            "-"
                        },
                        scale,
                        palette.accent,
                    );
                }
                x += 10;
            } else if row.number > 0 {
                canvas.text(
                    x,
                    y + 4,
                    &format!("{:>5}", row.number),
                    scale,
                    palette.muted,
                );
                x += 40 * scale;
            }
            let text = horizontal_slice(
                &row.text,
                self.inspector.horizontal,
                ((left.x + left.w - 15 - x) / (6 * scale)).max(0) as usize,
            );
            canvas.text(
                x,
                y + 4,
                &text,
                scale,
                match row.color {
                    ui::INK => palette.foreground,
                    ui::MUTED => palette.muted,
                    ui::ERROR => palette.error,
                    color => color,
                },
            );
        }
        if let Some(right) = right {
            canvas.set_clip(Some(right.clip().intersect(original_clip)));
            canvas.fill_rect(right.x, right.y, 1, right.h, palette.border);
            let first = self.inspector.detail_scroll.min(
                snapshot
                    .details
                    .len()
                    .saturating_sub(self.inspector_visible_rows()),
            );
            for (index, line) in snapshot
                .details
                .iter()
                .enumerate()
                .skip(first)
                .take(self.inspector_visible_rows() + 1)
            {
                let y = right.y + (index - first) as i32 * line_height;
                let text = horizontal_slice(
                    line,
                    self.inspector.horizontal,
                    ((right.w - 24) / (6 * scale)).max(0) as usize,
                );
                canvas.text(
                    right.x + 10,
                    y + 4,
                    &text,
                    scale,
                    if line.ends_with(':') {
                        palette.accent
                    } else {
                        palette.foreground
                    },
                );
            }
        }
        canvas.set_clip(Some(outer.clip().intersect(original_clip)));
        for (rect, details) in [(Some(left), false), (right, true)] {
            if let Some(thumb) = rect.and_then(|rect| self.inspector_scrollbar(rect, details)) {
                ui::rounded_rect(
                    canvas,
                    (thumb.x + 2, thumb.y, 6, thumb.h),
                    3.0,
                    palette.muted,
                );
            }
        }
        if self.inspector.tab == Tab::Console {
            let command = self.inspector_command_rect();
            draw_editor(
                canvas,
                command,
                &self.inspector.command,
                "Enter JavaScript; Enter runs, Up/Down recalls",
                self.inspector.focused && self.inspector.pane == Pane::Command,
                true,
                scale,
                palette,
            );
        }
        let bottom = outer.y + outer.h - FOOTER;
        canvas.fill_rect(outer.x, bottom, outer.w, FOOTER, ui::NAVY_LIGHT);
        let description = if self.inspector.notice.is_empty() {
            format!(
                "{} | {} / {} lines{} | Shift+wheel: sideways | Ctrl+wheel: text size",
                snapshot.label,
                snapshot.rows.len(),
                snapshot.total,
                if snapshot.truncated {
                    " (view capped)"
                } else {
                    ""
                }
            )
        } else {
            self.inspector.notice.clone()
        };
        canvas.text(
            outer.x + 8,
            bottom + 8,
            &fit_text(&description, ((outer.w - 16) / 6).max(0) as usize),
            1,
            ui::CREAM,
        );
        canvas.set_clip(Some(original_clip));
    }
}

fn truncate(text: &str, bytes: usize) -> String {
    if text.len() <= bytes {
        return text.to_owned();
    }
    if bytes < 3 {
        return ".".repeat(bytes);
    }
    let mut end = bytes.saturating_sub(3).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}

fn network_columns(width: i32, scale: i32) -> Vec<(i32, i32, &'static str)> {
    let available = (width - 12).max(0);
    let cell = 6 * scale;
    let compact = available < 82 * cell;
    let mut widths = vec![("#", 4 * cell + 8), ("Request", 0)];
    if !compact {
        widths.push(("Method", 7 * cell + 8));
    }
    widths.push(("Status", 8 * cell + 8));
    if !compact {
        widths.push(("Type", 7 * cell + 8));
    }
    if available >= 50 * cell {
        widths.push(("Received", 9 * cell + 8));
    }
    widths.push(("Time", 9 * cell + 8));
    let fixed: i32 = widths.iter().map(|(_, w)| w).sum();
    widths[1].1 = (available - fixed).max(48);
    let mut x = 0;
    widths
        .into_iter()
        .map(|(label, w)| {
            let column = (x, w, label);
            x += w;
            column
        })
        .collect()
}

fn network_name(url: &str) -> &str {
    url.split_once("://").map_or(url, |(_, rest)| {
        rest.find('/').map_or(rest, |index| {
            if index + 1 == rest.len() {
                rest
            } else {
                &rest[index..]
            }
        })
    })
}
fn network_status(request: &crate::network_trace::Exchange) -> String {
    match request.outcome {
        crate::network_trace::Outcome::Complete => request
            .status
            .map_or_else(|| "complete".into(), |status| status.to_string()),
        crate::network_trace::Outcome::Failed if request.status.is_some() => {
            format!("{} fail", request.status.unwrap())
        }
        crate::network_trace::Outcome::Pending if request.status.is_some() => {
            format!("{} ...", request.status.unwrap())
        }
        _ => request.outcome.label().into(),
    }
}
fn network_protocol(request: &crate::network_trace::Exchange) -> &'static str {
    if matches!(
        request.outcome,
        crate::network_trace::Outcome::Blocked | crate::network_trace::Outcome::Reused
    ) {
        "No HTTP exchange"
    } else if let Some(protocol) = request.protocol {
        protocol
    } else if request.url.starts_with("https://") {
        "TLS not established"
    } else {
        "HTTP/1.1 over TCP"
    }
}
fn milliseconds(value: std::time::Duration) -> String {
    let ms = value.as_secs_f64() * 1000.0;
    if ms < 10.0 {
        format!("{ms:.2}ms")
    } else if ms < 1000.0 {
        format!("{ms:.1}ms")
    } else {
        format!("{:.2}s", ms / 1000.0)
    }
}
fn compact_bytes(value: usize) -> String {
    if value < 1024 {
        format!("{value}B")
    } else if value < 1024 * 1024 {
        format!("{:.1}KiB", value as f64 / 1024.0)
    } else {
        format!("{:.1}MiB", value as f64 / (1024.0 * 1024.0))
    }
}
fn optional_bytes(value: Option<usize>) -> String {
    value.map_or_else(|| "not measured".into(), compact_bytes)
}
fn network_phase_color(phase: crate::network_trace::PhaseKind) -> u32 {
    use crate::network_trace::PhaseKind::*;
    match phase {
        Queue => 0x8996a8,
        Dns => 0xb5a1ed,
        Connect => 0xd89352,
        Tls => 0x9673d7,
        Send => 0xc2ac43,
        Wait => 0x439b80,
        Receive => 0x488acb,
        Decode => 0xc276a0,
    }
}
fn network_details(
    request: &crate::network_trace::Exchange,
    now: std::time::Duration,
) -> Vec<String> {
    use crate::network_trace::PhaseKind::*;
    let phases = request
        .phases
        .iter()
        .map(|phase| {
            let name = match phase.kind {
                Queue => "Queue",
                Dns => "DNS",
                Connect => "TCP",
                Tls => "TLS",
                Send => "Send",
                Wait => "Wait",
                Receive => "Receive",
                Decode => "Decode",
            };
            format!(
                "{name} {}{}",
                milliseconds(phase.end.unwrap_or(now).saturating_sub(phase.start)),
                if phase.end.is_none() { "+" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join(" | ");
    vec![
        format!("#{} {} {}", request.id + 1, request.method, request.url),
        format!(
            "{} | {} | start {} | total {} | received {} | encoded {} | decoded {}{}",
            network_status(request),
            network_protocol(request),
            milliseconds(request.start),
            milliseconds(request.duration(now)),
            compact_bytes(request.http_bytes),
            optional_bytes(request.encoded_bytes),
            optional_bytes(request.decoded_bytes),
            if request.reused_connection {
                " | connection reused"
            } else {
                ""
            }
        ),
        if phases.is_empty() {
            "No network phases: no HTTP exchange was performed.".into()
        } else {
            phases
        },
        request.note.clone(),
    ]
}

fn horizontal_slice(text: &str, offset: usize, count: usize) -> String {
    text.chars()
        .skip(offset)
        .take(count)
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn draw_editor(
    canvas: &mut Canvas,
    rect: Rect,
    editor: &Editor,
    placeholder: &str,
    focused: bool,
    command: bool,
    scale: i32,
    palette: ui::Palette,
) {
    ui::rounded_rect(canvas, rect.tuple(), 4.0, palette.panel);
    ui::rounded_border(
        canvas,
        rect.tuple(),
        4.0,
        1.0,
        if focused {
            palette.accent
        } else {
            palette.border
        },
    );
    let clip = canvas.clip();
    canvas.set_clip(Some(rect.clip().intersect(clip)));
    let prefix = if command { 20 } else { 8 };
    let advance = 6 * scale;
    let text_top = rect.y + (rect.h - 7 * scale) / 2;
    if command {
        canvas.text(rect.x + 5, text_top, ">", scale, palette.accent);
    }
    let cells = ((rect.w - prefix - 8) / advance).max(1) as usize;
    let offset = editor.offset.min(editor.chars.len().saturating_sub(cells));
    if !focused && editor.chars.is_empty() {
        canvas.text(
            rect.x + prefix,
            text_top,
            &fit_text(placeholder, cells),
            scale,
            palette.muted,
        );
    } else {
        let selected = editor.range();
        let from = selected.start.max(offset);
        let to = selected.end.min(offset + cells);
        if to > from {
            canvas.fill_rect(
                rect.x + prefix + (from - offset) as i32 * advance,
                text_top - 2,
                (to - from) as i32 * advance,
                7 * scale + 4,
                palette.selected,
            );
        }
        let text = editor
            .chars
            .iter()
            .skip(offset)
            .take(cells)
            .map(|ch| if ch.is_control() { ' ' } else { *ch })
            .collect::<String>();
        canvas.text(rect.x + prefix, text_top, &text, scale, palette.foreground);
        if focused {
            let cursor = editor.cursor.saturating_sub(offset).min(cells) as i32 * advance;
            canvas.fill_rect(
                (rect.x + prefix + cursor).min(rect.x + rect.w - 3),
                text_top - 2,
                1,
                7 * scale + 4,
                palette.accent,
            );
        }
    }
    canvas.set_clip(Some(clip));
}

fn build_snapshot(app: &App) -> Snapshot {
    let inspector = &app.inspector;
    let mut snapshot = Snapshot::default();
    match inspector.tab {
        Tab::Elements => {
            snapshot.label = "Parsed document / current computed styles".into();
            let mut stack = vec![(0usize, 0usize)];
            let mut seen = HashSet::new();
            let mut bytes = 0usize;
            while let Some((node, depth)) = stack.pop() {
                if !seen.insert(node) || seen.len() > MAX_LINES {
                    snapshot.truncated = true;
                    break;
                }
                let Some(item) = app.document.nodes.get(node) else {
                    continue;
                };
                let mut text = String::new();
                match &item.kind {
                    html::NodeKind::DocumentFragment => text.push_str("#document-fragment"),
                    html::NodeKind::DocumentType(data)=>{text=format!("<!DOCTYPE {}>",truncate(&data.name,128));},
                    html::NodeKind::Element { tag, attrs, .. } => {
                        let _ = write!(text, "<{tag}");
                        for attr in attrs.iter().take(8) {
                            let (key, value) = (&attr.name.qualified, &attr.value);
                            let _ =
                                write!(text, " {}=\"{}\"", truncate(key, 50), truncate(value, 100));
                            if text.len() > 300 {
                                break;
                            }
                        }
                        text.push('>');
                    }
                    html::NodeKind::Comment(value) => { text = format!("<!--{}-->", truncate(value, 240)); }
                    html::NodeKind::ProcessingInstruction { target, data } => {
                        text = format!("<?{} {}?>", truncate(target, 128), truncate(data, 240));
                    }
                    html::NodeKind::Text(value) => {
                        let value = value
                            .split_whitespace()
                            .take(60)
                            .collect::<Vec<_>>()
                            .join(" ");
                        if !value.is_empty() {
                            text = format!("\"{}\"", truncate(&value, 240));
                        }
                    }
                }
                if node != 0 && !text.is_empty() {
                    if bytes.saturating_add(text.len()) > MAX_VIEW_BYTES {
                        snapshot.truncated = true;
                        break;
                    }
                    bytes += text.len();
                    snapshot.rows.push(Row {
                        text,
                        node: Some(node),
                        depth: depth.saturating_sub(1),
                        children: !item.children.is_empty(),
                        number: 0,
                        color: if item.kind.character_data().is_some() {
                            ui::MUTED
                        } else {
                            ui::INK
                        },
                    });
                }
                if !inspector.collapsed.contains(&node) || !inspector.filter.chars.is_empty() {
                    stack.extend(item.children.iter().rev().map(|child| (*child, depth + 1)));
                }
            }
            snapshot.details = selected_details(app);
        }
        Tab::Sources => {
            let count = 1 + inspector.scripts.len() + app.styles.len();
            let index = inspector.source_index.min(count - 1);
            let (label, source) = if index == 0 {
                let kind = if app.document.content_type.starts_with("image/") {
                    "Generated image viewer"
                } else if app.document.content_type == "text/plain" {
                    "Original document text"
                } else {
                    "Document HTML"
                };
                (
                    format!("{kind} ({})", app.document.content_type),
                    app.raw.as_str(),
                )
            } else if let Some((label, source)) = inspector.scripts.get(index - 1) {
                (label.clone(), source.as_str())
            } else {
                let mut ids = app.styles.keys().copied().collect::<Vec<_>>();
                ids.sort_unstable();
                let node = ids[index - 1 - inspector.scripts.len()];
                (
                    app.document
                        .attr(node, "href")
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("Stylesheet node {node}")),
                    app.styles[&node].as_str(),
                )
            };
            snapshot.label = format!(
                "Source {} / {}: {}",
                index + 1,
                count,
                truncate(&label, 160)
            );
            add_lines(&mut snapshot, source, true);
        }
        Tab::Console => {
            snapshot.label = "Page JavaScript / command output".into();
            let report = if !app.scripts_enabled {
                "JavaScript is disabled in Settings.".into()
            } else if let Some(reason) = &app.scripting_blocked {
                format!("JavaScript is blocked: {reason}")
            } else {
                app.scripting
                    .as_ref()
                    .map(|page| page.report())
                    .unwrap_or_else(|| "No active page JavaScript runtime.".into())
            };
            add_lines(&mut snapshot, &report, false);
        }
        Tab::Network => {
            snapshot.label = "Measured HTTP requests / latest navigation".into();
            let network = app.network.snapshot();
            snapshot.truncated = network.omitted != 0;
            for request in &network.rows {
                snapshot.rows.push(Row {
                    text: format!(
                        "{} {} {} {} {} {}",
                        request.method,
                        request.url,
                        request
                            .status
                            .map_or_else(|| request.outcome.label().into(), |s| s.to_string()),
                        request.kind,
                        request.outcome.label(),
                        request.note
                    ),
                    node: None,
                    depth: 0,
                    children: false,
                    number: request.id as usize + 1,
                    color: ui::INK,
                });
            }
            snapshot.network = Some(network);
        }
    }
    snapshot.total = snapshot.rows.len();
    let query = inspector.filter.text().to_lowercase();
    if !query.is_empty() {
        snapshot
            .rows
            .retain(|row| row.text.to_lowercase().contains(&query));
    }
    snapshot
}

fn add_lines(snapshot: &mut Snapshot, source: &str, numbers: bool) {
    let mut bytes = 0usize;
    for (index, line) in source.lines().enumerate() {
        if snapshot.rows.len() >= MAX_LINES || bytes >= MAX_VIEW_BYTES {
            snapshot.truncated = true;
            break;
        }
        let available = MAX_VIEW_BYTES.saturating_sub(bytes).saturating_sub(1);
        let text = truncate(line, available);
        let partial = line.len() > available;
        bytes += text.len() + 1;
        snapshot.rows.push(Row {
            text,
            node: None,
            depth: 0,
            children: false,
            number: if numbers { index + 1 } else { 0 },
            color: if line.contains("JavaScript error")
                || line.starts_with("ERROR")
                || line.contains("FAILED")
            {
                ui::ERROR
            } else {
                ui::INK
            },
        });
        if partial {
            snapshot.truncated = true;
            break;
        }
    }
}

fn selected_details(app: &App) -> Vec<String> {
    let Some(node) = app
        .inspector
        .selected
        .and_then(|node| app.document.nodes.get(node).map(|item| (node, item)))
    else {
        return vec![
            "Select an element to inspect it.".into(),
            String::new(),
            "The tree is the current parsed document.".into(),
            "Styles include live supported mutations.".into(),
            "Source files retain their loaded text.".into(),
        ];
    };
    let (index, node) = node;
    let mut lines = vec![format!("Node {index}"), String::new()];
    match &node.kind {
        html::NodeKind::DocumentType(data)=>{lines.push(format!("DocumentType name: {}",truncate(&data.name,128)));lines.push(format!("Public ID: {}",truncate(&data.public_id,4096)));lines.push(format!("System ID: {}",truncate(&data.system_id,4096)));return lines;}
        html::NodeKind::DocumentFragment => {
            lines.push("DocumentFragment: no element attributes or layout box.".into());
            return lines;
        }
        html::NodeKind::ProcessingInstruction { target, data } => {
            lines.push(format!("Processing instruction {}:", truncate(target, 128)));
            lines.extend(data.lines().take(128).map(|line| truncate(line, 4096)));
            return lines;
        }
        html::NodeKind::Comment(data) => {
            lines.push("Comment data:".into());
            lines.extend(data.lines().take(128).map(|line| truncate(line, 4096)));
            return lines;
        }
        html::NodeKind::Text(text) => {
            lines.push("Text content:".into());
            lines.extend(text.lines().take(128).map(|line| truncate(line, 4096)));
            return lines;
        }
        html::NodeKind::Element { tag, attrs, .. } => {
            lines[0] = format!("<{tag}> / node {index}");
            lines.push("Attributes:".into());
            if attrs.is_empty() {
                lines.push("  (none)".into());
            }
            for attr in attrs.iter().take(128) {
                let (key, value) = (&attr.name.qualified, &attr.value);
                lines.push(format!(
                    "  {} = {}",
                    truncate(key, 128),
                    truncate(value, 2048)
                ));
            }
        }
    }
    lines.push(String::new());
    lines.push("Layout (zoomed pixels):".into());
    if app.view == View::Page {
        if let Some(Some(client)) = app.page.client_boxes.get(index) {
            lines.push(format!("  clientWidth: {}", client.width));
            lines.push(format!("  clientHeight: {}", client.height));
            lines.push(format!("  clientLeft: {}", client.left));
            lines.push(format!("  clientTop: {}", client.top));
        } else {
            lines.push("  No client box (inline, hidden or inert).".into());
        }
    } else {
        lines.push("  Return to the document for layout data.".into());
    }
    lines.push(String::new());
    lines.push("Computed styles (supported subset):".into());
    let checked = |node| {
        app.forms
            .controls
            .get(&node)
            .filter(|control| matches!(control.kind, Kind::Checkbox | Kind::Radio))
            .map(|control| control.checked)
    };
    let styles = crate::css::compute_with_state(&app.document, &app.styles, Some(&checked));
    if let Some(style) = styles.get(index) {
        lines.extend([
            format!("  display: {:?}", style.display),
            format!("  position: {:?}", style.position),
            format!("  object-fit: {:?}", style.object_fit),
            format!("  image-rendering: {:?}", style.image_rendering),
            format!(
                "  object-position: {}% + {}; {}% + {}",
                style.object_position[0].fraction * 100.0,
                length(style.object_position[0].offset),
                style.object_position[1].fraction * 100.0,
                length(style.object_position[1].offset)
            ),
            format!("  color: #{:06x}", style.color),
            format!(
                "  background: {}",
                style
                    .background
                    .map(|color| format!("#{color:06x}"))
                    .unwrap_or_else(|| "transparent".into())
            ),
            format!("  font-family: {}", style.font_families.join(", ")),
            format!("  font-size: {}px", style.font_size),
            format!("  font-weight: {}", style.font_weight),
            format!(
                "  font-style: {}",
                if style.italic { "italic" } else { "normal" }
            ),
            format!("  line-height: {:?}", style.line_height),
            format!("  text-align: {:?}", style.align),
            format!("  white-space: {:?}", style.white_space),
            format!("  width: {}", length(style.width)),
            format!("  height: {}", length(style.height)),
            format!("  min-width: {}", length(style.min_width)),
            format!("  min-height: {}", length(style.min_height)),
            format!(
                "  max-width: {}",
                style.max_width.map(length).unwrap_or_else(|| "none".into())
            ),
            format!(
                "  max-height: {}",
                style
                    .max_height
                    .map(length)
                    .unwrap_or_else(|| "none".into())
            ),
            format!(
                "  box-sizing: {}",
                if style.border_box {
                    "border-box"
                } else {
                    "content-box"
                }
            ),
            format!("  margin: {}", lengths(&style.margin)),
            format!("  padding: {}", lengths(&style.padding)),
            format!("  border-width: {}", lengths(&style.border_width)),
            format!("  overflow: {}", style.overflow.to_css()),
            format!("  flex-direction: {:?}", style.flex_direction),
            format!("  flex-wrap: {}", style.flex_wrap),
            format!("  flex-grow: {}", style.flex_grow),
            format!("  flex-shrink: {}", style.flex_shrink),
            format!("  flex-basis: {}", length(style.flex_basis)),
            format!("  justify-content: {:?}", style.justify_content),
            format!("  align-items: {:?}", style.align_items),
            format!("  gap: {} {}", length(style.gap[0]), length(style.gap[1])),
        ]);
    }
    lines
}

fn length(value: crate::css::Length) -> String {
    match value {
        crate::css::Length::Auto => "auto".into(),
        crate::css::Length::Px(value) => format!("{value}px"),
        crate::css::Length::Percent(value) => format!("{value}%"),
        crate::css::Length::Em(value) => format!("{value}em"),
        crate::css::Length::Rem(value) => format!("{value}rem"),
        crate::css::Length::Calc(value)=>value.text(),
    }
}

fn lengths(values: &[crate::css::Length; 4]) -> String {
    values
        .iter()
        .copied()
        .map(length)
        .collect::<Vec<_>>()
        .join(" ")
}

// Preserve pasted JavaScript line breaks (including // comments) while keeping
// the familiar one-line command editor. Other control characters are rejected.
fn insert_command(editor: &mut Editor, text: &str) {
    editor.delete_selection();
    let mut bytes = editor.chars.iter().map(|ch| ch.len_utf8()).sum::<usize>();
    let mut incoming = Vec::new();
    for ch in text
        .chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
    {
        if bytes.saturating_add(ch.len_utf8()) > MAX_COMMAND {
            break;
        }
        bytes += ch.len_utf8();
        incoming.push(ch);
    }
    let count = incoming.len();
    editor.chars.splice(editor.cursor..editor.cursor, incoming);
    editor.cursor += count;
    editor.anchor = editor.cursor;
}

#[cfg(test)]
#[path = "app_inspector_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "app_network_tests.rs"]
mod network_tests;
