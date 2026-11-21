//! ChezCat's own window controls. The same rectangles drive paint and input.
use super::*;
use crate::ui::{self, Icon};

const MENU_ITEMS: usize = 15;
const MENU_ROW: i32 = 34;
type Rect = (i32, i32, i32, i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ChromeAction {
    Back,
    Forward,
    Reload,
    Home,
    Bookmark,
    Downloads,
    Menu,
    FindPrevious,
    FindNext,
    FindClose,
    MenuItem(usize),
}

#[derive(Default)]
pub(super) struct Chrome {
    pub hover: Option<ChromeAction>,
    pub pressed: Option<ChromeAction>,
    pub focus: Option<ChromeAction>,
    pub menu_selected: Option<usize>,
    pub menu_first: usize,
    menu_wheel: i32,
    pressed_stop: bool,
}

fn contains((x, y, width, height): Rect, px: i32, py: i32) -> bool {
    (x..x + width).contains(&px) && (y..y + height).contains(&py)
}

impl App {
    pub(super) fn reset_chrome(&mut self) {
        self.chrome = Chrome::default();
    }

    pub(super) fn menu_visible_rows(&self) -> usize {
        ((self.height - FOOT_HEIGHT - 70) / MENU_ROW).clamp(1, MENU_ITEMS as i32) as usize
    }

    pub(super) fn menu_geometry(&self) -> Rect {
        (
            (self.width - 372).max(4),
            58,
            360,
            self.menu_visible_rows() as i32 * MENU_ROW + 12,
        )
    }

    pub(super) fn keep_menu_selection_visible(&mut self) {
        let rows = self.menu_visible_rows();
        self.chrome.menu_first = self.chrome.menu_first.min(MENU_ITEMS - rows);
        if let Some(selected) = self.chrome.menu_selected {
            if selected < self.chrome.menu_first {
                self.chrome.menu_first = selected;
            } else if selected >= self.chrome.menu_first + rows {
                self.chrome.menu_first = selected + 1 - rows;
            }
        }
    }

    pub(super) fn chrome_rect(&self, action: ChromeAction) -> Option<Rect> {
        use ChromeAction::*;
        let compact = self.width < 680;
        Some(match action {
            Back if compact => (8, 14, 32, 40),
            Forward if compact => (44, 14, 32, 40),
            Reload if compact => (80, 14, 32, 40),
            Home if compact => (116, 14, 32, 40),
            Bookmark if compact => (self.width - 116, 14, 30, 40),
            Downloads if compact => (self.width - 80, 14, 34, 40),
            Menu if compact => (self.width - 42, 14, 34, 40),
            Back => (10, 14, 40, 40),
            Forward => (56, 14, 40, 40),
            Reload => (102, 14, 40, 40),
            Home => (148, 14, 40, 40),
            Bookmark => (self.width - 148, 14, 38, 40),
            Downloads => (self.width - 100, 14, 40, 40),
            Menu => (self.width - 52, 14, 40, 40),
            FindPrevious if self.find_open && self.browser_panel.is_none() => {
                (self.width - 128, self.chrome_bottom() + 6, 36, 30)
            }
            FindNext if self.find_open && self.browser_panel.is_none() => {
                (self.width - 84, self.chrome_bottom() + 6, 36, 30)
            }
            FindClose if self.find_open && self.browser_panel.is_none() => {
                (self.width - 40, self.chrome_bottom() + 6, 28, 30)
            }
            MenuItem(index)
                if self.menu
                    && (self.chrome.menu_first
                        ..self.chrome.menu_first + self.menu_visible_rows())
                        .contains(&index) =>
            {
                let (left, top, width, _) = self.menu_geometry();
                (
                    left + 6,
                    top + 6 + (index - self.chrome.menu_first) as i32 * MENU_ROW,
                    width - 12,
                    MENU_ROW,
                )
            }
            _ => return None,
        })
    }

    pub(super) fn chrome_enabled(&self, action: ChromeAction) -> bool {
        match action {
            ChromeAction::Back => self.navigation_history_index() > 0,
            ChromeAction::Forward => self.navigation_history_index() + 1 < self.history.len(),
            ChromeAction::FindPrevious | ChromeAction::FindNext => !self.matches.is_empty(),
            ChromeAction::MenuItem(index) => index < MENU_ITEMS,
            _ => true,
        }
    }

    pub(super) fn chrome_hit(&self, x: i32, y: i32) -> Option<ChromeAction> {
        use ChromeAction::*;
        if self.menu && contains(self.menu_geometry(), x, y) {
            return (self.chrome.menu_first..self.chrome.menu_first + self.menu_visible_rows())
                .map(MenuItem)
                .find(|action| {
                    self.chrome_rect(*action)
                        .is_some_and(|rect| contains(rect, x, y))
                });
        }
        [
            Back,
            Forward,
            Reload,
            Home,
            Bookmark,
            Downloads,
            Menu,
            FindPrevious,
            FindNext,
            FindClose,
        ]
        .into_iter()
        .find(|action| {
            self.chrome_rect(*action)
                .is_some_and(|rect| contains(rect, x, y))
        })
    }

    pub(super) fn open_menu(&mut self, keyboard: bool) {
        self.close_downloads();
        self.close_bookmark_popup();
        self.inspector.focused = false;
        if self.browser_panel.is_some() {
            self.close_browser_panel();
        }
        self.select_popup = None;
        self.pending_surrogate = None;
        self.drag = Drag::None;
        self.focus = Focus::Page;
        self.chrome.focus = keyboard.then_some(ChromeAction::Menu);
        self.chrome.menu_selected = keyboard.then_some(0);
        self.chrome.menu_first = 0;
        self.chrome.menu_wheel = 0;
        self.chrome.hover = None;
        self.hover = None;
        self.menu = true;
    }

    pub(super) fn activate_chrome(&mut self, action: ChromeAction) -> PlatformAction {
        use ChromeAction::*;
        if !self.chrome_enabled(action) {
            return PlatformAction::None;
        }
        match action {
            Back => self.history_move(-1),
            Forward => self.history_move(1),
            Reload if self.is_loading() => self.stop_loading(),
            Reload => self.reload(),
            Home => self.home(),
            Bookmark => self.add_current_bookmark(),
            Downloads => self.toggle_downloads(),
            Menu => {
                if self.menu {
                    self.menu = false;
                    self.chrome.menu_selected = None;
                } else {
                    self.open_menu(self.chrome.focus == Some(Menu));
                }
            }
            FindPrevious => self.find_next(true),
            FindNext => self.find_next(false),
            FindClose => {
                self.find_open = false;
                self.matches.clear();
                self.chrome.focus = None;
                self.chrome.hover = None;
                self.focus = Focus::Page;
                self.reflow();
                self.set_scroll(self.scroll.clamp(0, self.max_scroll()));
            }
            MenuItem(index) => {
                self.menu = false;
                self.reset_chrome();
                match index {
                    0 => return PlatformAction::OpenFile,
                    1 => self.open_find(),
                    2 => self.set_zoom(self.zoom + 0.25),
                    3 => self.set_zoom(self.zoom - 0.25),
                    4 => self.set_zoom(self.profile.data().settings.default_zoom as f32 / 100.0),
                    5 => self.show_source(),
                    6 => self.show_resources(),
                    7 => self.home(),
                    8 => self.toggle_javascript(),
                    9 => self.add_current_bookmark(),
                    10 => self.show_bookmarks(),
                    11 => self.show_settings(),
                    12 => self.show_history(),
                    13 => self.toggle_inspector(),
                    14 => self.show_about(),
                    _ => {}
                }
            }
        }
        PlatformAction::None
    }

    /// Pressing does not execute an action: release must land on the same control.
    pub(super) fn chrome_pointer_down(&mut self, x: i32, y: i32, shift: bool) -> bool {
        if self.menu && !contains(self.menu_geometry(), x, y) && y >= BAR_HEIGHT {
            self.menu = false;
            self.reset_chrome();
            self.drag = Drag::None;
            return true;
        }
        if let Some(action) = self.chrome_hit(x, y) {
            self.drag = Drag::None;
            self.select_popup = None;
            self.chrome.focus = None;
            self.chrome.hover = Some(action);
            self.chrome.pressed = self.chrome_enabled(action).then_some(action);
            self.chrome.pressed_stop = action == ChromeAction::Reload && self.is_loading();
            if action == ChromeAction::Bookmark {
                self.arm_current_bookmark_drag(x, y);
            }
            if let ChromeAction::MenuItem(index) = action {
                self.chrome.menu_selected = Some(index);
            } else if action != ChromeAction::Menu {
                self.menu = false;
            }
            // A click on browser controls never leaves the page's editor active.
            self.focus = Focus::Page;
            return true;
        }
        if self.menu && contains(self.menu_geometry(), x, y) {
            return true;
        }
        self.chrome.pressed = None;
        self.chrome.focus = None;
        if (0..BAR_HEIGHT).contains(&y) {
            self.menu = false;
            let (left, width) = self.address_rect();
            if contains((left, 14, width, 40), x, y) {
                if self.focus != Focus::Address {
                    self.focus_address();
                } else {
                    let pos = self.address.offset + ((x - left - 8).max(0) / 12) as usize;
                    self.address.move_to(pos, shift);
                    self.drag = Drag::Field;
                }
            }
            self.sync_editors();
            return true;
        }
        if self.find_open && (self.chrome_bottom()..self.content_top()).contains(&y) {
            if contains(
                (72, self.chrome_bottom() + 5, (self.width - 304).max(40), 32),
                x,
                y,
            ) {
                self.focus = Focus::Find;
                let pos = self.find.offset + ((x - 80).max(0) / 12) as usize;
                self.find.move_to(pos, shift);
                self.drag = Drag::Field;
            }
            self.sync_editors();
            return true;
        }
        false
    }

    pub(super) fn chrome_pointer_up(&mut self, x: i32, y: i32) -> Option<PlatformAction> {
        let pressed = self.chrome.pressed.take()?;
        if self.chrome_hit(x, y) != Some(pressed) || !self.chrome_enabled(pressed) {
            return Some(PlatformAction::None);
        }
        if pressed == ChromeAction::Reload && self.chrome.pressed_stop {
            // A load finishing between down/up must not turn Stop into a new reload.
            self.stop_loading();
            return Some(PlatformAction::None);
        }
        Some(self.activate_chrome(pressed))
    }

    pub(super) fn chrome_wheel(&mut self, delta: i32) {
        self.chrome.menu_wheel = self.chrome.menu_wheel.saturating_add(delta);
        let steps = self.chrome.menu_wheel / 120;
        self.chrome.menu_wheel %= 120;
        self.chrome.menu_first = (self.chrome.menu_first as i32)
            .saturating_sub(steps.saturating_mul(3))
            .clamp(0, (MENU_ITEMS - self.menu_visible_rows()) as i32)
            as usize;
        self.chrome.hover = None;
        self.chrome.pressed = None;
        self.chrome.menu_selected = None;
    }

    pub(super) fn focus_chrome(&mut self, action: ChromeAction) {
        self.focus = Focus::Page;
        self.select_popup = None;
        self.drag = Drag::None;
        self.pending_surrogate = None;
        self.chrome.focus = Some(action);
        self.chrome.hover = None;
    }

    pub(super) fn step_chrome_focus(&mut self, previous: bool) {
        use ChromeAction::*;
        let current = self.chrome.focus.take();
        if matches!(current, Some(FindPrevious | FindNext | FindClose)) || self.focus == Focus::Find
        {
            let targets = [FindPrevious, FindNext, FindClose];
            let index = current.and_then(|action| targets.iter().position(|item| *item == action));
            let next = if previous {
                index.map_or(Some(targets.len() - 1), |i| i.checked_sub(1))
            } else {
                Some(index.map_or(0, |i| i + 1))
            };
            if let Some(action) = next.and_then(|i| targets.get(i)).copied() {
                if self.chrome_enabled(action) {
                    self.focus_chrome(action);
                } else {
                    self.chrome.focus = Some(action);
                    self.step_chrome_focus(previous);
                }
            } else {
                self.focus = Focus::Find;
            }
            return;
        }
        // Address is the editor between Home and Menu in the toolbar's tab order.
        let targets = [
            Some(Back),
            Some(Forward),
            Some(Reload),
            Some(Home),
            None,
            Some(Bookmark),
            Some(Downloads),
            Some(Menu),
        ];
        let index = if self.focus == Focus::Address {
            4
        } else {
            targets
                .iter()
                .position(|item| *item == current)
                .unwrap_or(7)
        };
        let mut next = if previous {
            index as i32 - 1
        } else {
            index as i32 + 1
        };
        while (0..targets.len() as i32).contains(&next) {
            match targets[next as usize] {
                Some(action) if self.chrome_enabled(action) => {
                    self.focus_chrome(action);
                    return;
                }
                None => {
                    self.focus_address();
                    return;
                }
                _ => next += if previous { -1 } else { 1 },
            }
        }
        self.focus = Focus::Page;
        self.tab_focus(previous);
    }

    pub(super) fn chrome_key(
        &mut self,
        key: u32,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> Option<PlatformAction> {
        if alt && !ctrl && key == 0x46 {
            if self.menu {
                self.menu = false;
                self.focus_chrome(ChromeAction::Menu);
            } else {
                self.open_menu(true);
            }
            return Some(PlatformAction::None);
        }
        if self.menu {
            if ctrl || alt || key == 0x75 {
                self.menu = false;
                self.reset_chrome();
                return None;
            }
            let rows = self.menu_visible_rows();
            let selected = self.chrome.menu_selected;
            match key {
                0x1b | 0x09 => {
                    self.menu = false;
                    self.chrome.menu_selected = None;
                    self.focus_chrome(ChromeAction::Menu);
                }
                0x26 => {
                    self.chrome.menu_selected =
                        Some(selected.map_or(MENU_ITEMS - 1, |i| (i + MENU_ITEMS - 1) % MENU_ITEMS))
                }
                0x28 => {
                    self.chrome.menu_selected = Some(selected.map_or(0, |i| (i + 1) % MENU_ITEMS))
                }
                0x24 => self.chrome.menu_selected = Some(0),
                0x23 => self.chrome.menu_selected = Some(MENU_ITEMS - 1),
                0x21 => {
                    self.chrome.menu_selected = Some(
                        selected
                            .unwrap_or(self.chrome.menu_first)
                            .saturating_sub(rows),
                    )
                }
                0x22 => {
                    self.chrome.menu_selected = Some(
                        (selected.unwrap_or(self.chrome.menu_first) + rows).min(MENU_ITEMS - 1),
                    )
                }
                0x0d | 0x20 => {
                    if let Some(index) = selected {
                        return Some(self.activate_chrome(ChromeAction::MenuItem(index)));
                    }
                }
                _ => {}
            }
            self.chrome.hover = None;
            self.keep_menu_selection_visible();
            return Some(PlatformAction::None);
        }
        if ctrl || alt || matches!(key, 0x72 | 0x74 | 0x75) {
            return None;
        }
        if let Some(action) = self.chrome.focus {
            match key {
                0x09 => self.step_chrome_focus(shift),
                0x25 => self.step_chrome_focus(true),
                0x27 => self.step_chrome_focus(false),
                0x1b => {
                    self.chrome.focus = None;
                    self.focus = Focus::Page;
                }
                0x28 if action == ChromeAction::Menu => self.open_menu(true),
                0x0d | 0x20 => return Some(self.activate_chrome(action)),
                _ => {}
            }
            return Some(PlatformAction::None);
        }
        if key == 0x09 && matches!(self.focus, Focus::Address | Focus::Find) {
            self.step_chrome_focus(shift);
            return Some(PlatformAction::None);
        }
        None
    }

    pub(super) fn draw_icon_button(
        &self,
        canvas: &mut Canvas,
        action: ChromeAction,
        icon: Icon,
        light: bool,
    ) {
        let Some(rect @ (x, y, width, height)) = self.chrome_rect(action) else {
            return;
        };
        let enabled = self.chrome_enabled(action);
        let hovered = self.chrome.hover == Some(action) && enabled;
        let focused = self.chrome.focus == Some(action);
        let pressed = self.chrome.pressed == Some(action) && hovered;
        let palette = self.browser_palette();
        let active = action == ChromeAction::Menu && self.menu
            || action == ChromeAction::Downloads && self.downloads_open();
        let background = if pressed || active {
            ui::TEAL_DARK
        } else if hovered {
            if light {
                palette.selected
            } else {
                ui::NAVY_LIGHT
            }
        } else if light {
            palette.background
        } else {
            ui::NAVY
        };
        ui::rounded_rect(canvas, rect, 8.0, background);
        if focused {
            ui::rounded_border(canvas, rect, 8.0, 2.0, ui::TEAL);
        } else if hovered || active {
            ui::rounded_border(
                canvas,
                rect,
                8.0,
                1.0,
                if light { palette.border } else { ui::TEAL_DARK },
            );
        }
        let foreground = if !enabled {
            ui::DISABLED
        } else if light && !pressed {
            palette.foreground
        } else {
            ui::CREAM
        };
        let size = if height >= 40 { 22 } else { 18 };
        ui::icon(
            canvas,
            icon,
            x + (width - size) / 2,
            y + (height - size) / 2,
            size,
            foreground,
            if enabled { ui::TEAL } else { ui::DISABLED },
        );
    }

    pub(super) fn draw_toolbar(&self, canvas: &mut Canvas) {
        use ChromeAction::*;
        canvas.fill_rect(0, 0, self.width, BAR_HEIGHT, ui::NAVY);
        canvas.fill_rect(0, BAR_HEIGHT - 1, self.width, 1, ui::TEAL_DARK);
        for (action, icon) in [
            (Back, Icon::Back),
            (Forward, Icon::Forward),
            (
                Reload,
                if self.is_loading() {
                    Icon::Stop
                } else {
                    Icon::Refresh
                },
            ),
            (Home, Icon::Home),
            (
                Bookmark,
                if self.current_bookmarked() {
                    Icon::Bookmarked
                } else {
                    Icon::Bookmark
                },
            ),
            (Downloads, Icon::Download),
            (Menu, Icon::Menu),
        ] {
            self.draw_icon_button(canvas, action, icon, false);
        }
        let (x, width) = self.address_rect();
        field_at_offset_themed(
            canvas,
            &self.address,
            (x, 14, width, 40),
            self.focus == Focus::Address,
            if self.focus == Focus::Address {
                self.address.offset
            } else {
                0
            },
            self.browser_palette(),
        );
        if self.address.text().is_empty() {
            canvas.set_clip(Some(crate::paint::ClipRect::new(x+8,16,(width-16).max(0),36)));
            canvas.text(x+12,27,self.profile.data().settings.search_engine.hint(),1,self.browser_palette().muted);
            canvas.set_clip(None);
        }
        if self.current_bookmarked() {
            if let Some((x, y, w, h)) = self.chrome_rect(Bookmark) {
                ui::icon(
                    canvas,
                    Icon::Bookmarked,
                    x + (w - 22) / 2,
                    y + (h - 22) / 2,
                    22,
                    ui::TEAL,
                    ui::TEAL,
                );
            }
        }
        if self.downloads_active() {
            if let Some((x, y, _, _)) = self.chrome_rect(Downloads) {
                ui::rounded_rect(canvas, (x + 28, y + 4, 7, 7), 3.5, ui::TEAL);
            }
        }
    }

    pub(super) fn draw_find_bar(&self, canvas: &mut Canvas) {
        if !self.find_open {
            return;
        }
        let palette = self.browser_palette();
        canvas.fill_rect(
            0,
            self.chrome_bottom(),
            self.width,
            FIND_HEIGHT,
            palette.background,
        );
        canvas.fill_rect(
            0,
            self.chrome_bottom() + FIND_HEIGHT - 1,
            self.width,
            1,
            palette.border,
        );
        ui::icon(
            canvas,
            Icon::Search,
            18,
            self.chrome_bottom() + 12,
            18,
            palette.foreground,
            ui::TEAL_DARK,
        );
        field_themed(
            canvas,
            &self.find,
            (72, self.chrome_bottom() + 5, (self.width - 304).max(40), 32),
            self.focus == Focus::Find,
            palette,
        );
        let count = if self.find.chars.is_empty() {
            String::new()
        } else if self.matches.is_empty() {
            "No match".to_owned()
        } else {
            format!(
                "{}/{}{}",
                self.match_index + 1,
                self.matches.len(),
                if self.matches.len() == 2000 { "+" } else { "" }
            )
        };
        canvas.text(
            self.width - 218,
            self.chrome_bottom() + 17,
            &count,
            1,
            if self.matches.is_empty() {
                ui::ERROR
            } else {
                palette.muted
            },
        );
        for (action, icon) in [
            (ChromeAction::FindPrevious, Icon::Up),
            (ChromeAction::FindNext, Icon::Down),
            (ChromeAction::FindClose, Icon::Close),
        ] {
            self.draw_icon_button(canvas, action, icon, true);
        }
    }

    pub(super) fn draw_menu(&self, canvas: &mut Canvas) {
        if !self.menu {
            return;
        }
        let palette = self.browser_palette();
        let (left, top, width, height) = self.menu_geometry();
        // A small two-step shadow, drawn by us rather than an OS menu theme.
        ui::rounded_rect(
            canvas,
            (left - 2, top + 3, width + 4, height + 4),
            11.0,
            0x9aa4ac,
        );
        ui::rounded_rect(canvas, (left, top, width, height), 10.0, palette.chrome);
        ui::rounded_border(
            canvas,
            (left, top, width, height),
            10.0,
            1.0,
            ui::NAVY_LIGHT,
        );
        let rows = [
            (Icon::File, "Open file", "Ctrl+O"),
            (Icon::Search, "Find in page", "Ctrl+F"),
            (Icon::Plus, "Zoom in", "Ctrl++"),
            (Icon::Minus, "Zoom out", "Ctrl+-"),
            (Icon::Reset, "Reset zoom", "Ctrl+0"),
            (Icon::Code, "View source", "Ctrl+U"),
            (Icon::Activity, "Resource activity", "Ctrl+I"),
            (Icon::Home, "Home", "Alt+Home"),
            (
                Icon::Code,
                if self.scripts_enabled {
                    "JavaScript: ON"
                } else {
                    "JavaScript: OFF"
                },
                "Ctrl+J",
            ),
            (Icon::Bookmark, "Bookmark this page", "Ctrl+D"),
            (Icon::Bookmarks, "Bookmarks", "Ctrl+B"),
            (Icon::Settings, "Settings", "Ctrl+,"),
            (Icon::History, "History", "Ctrl+H"),
            (Icon::Code, "Developer tools", "F12"),
            (Icon::Code, "About ChezCat", ""),
        ];
        for (index, (icon, label, shortcut)) in rows
            .iter()
            .enumerate()
            .skip(self.chrome.menu_first)
            .take(self.menu_visible_rows())
        {
            let rect @ (x, y, w, h) = self.chrome_rect(ChromeAction::MenuItem(index)).unwrap();
            let selected = self.chrome.menu_selected == Some(index);
            let hovered = self.chrome.hover == Some(ChromeAction::MenuItem(index));
            if selected || hovered {
                ui::rounded_rect(
                    canvas,
                    rect,
                    5.0,
                    if hovered && self.chrome.pressed == self.chrome.hover {
                        ui::TEAL_DARK
                    } else {
                        ui::NAVY_LIGHT
                    },
                );
                if selected && self.chrome.focus.is_some() {
                    ui::rounded_border(canvas, rect, 5.0, 1.0, ui::TEAL);
                }
            }
            ui::icon(
                canvas,
                *icon,
                x + 8,
                y + (h - 17) / 2,
                17,
                palette.chrome_text,
                ui::TEAL,
            );
            canvas.text(x + 34, y + (h - 14) / 2, label, 2, palette.chrome_text);
            canvas.text(
                x + w - 12 - shortcut.len() as i32 * 6,
                y + (h - 7) / 2,
                shortcut,
                1,
                0xa9b8c9,
            );
        }
        let visible = self.menu_visible_rows();
        if visible < MENU_ITEMS {
            let track = height - 16;
            let thumb = (track * visible as i32 / MENU_ITEMS as i32).max(8);
            let offset =
                (track - thumb) * self.chrome.menu_first as i32 / (MENU_ITEMS - visible) as i32;
            ui::rounded_rect(
                canvas,
                (left + width - 5, top + 8 + offset, 2, thumb),
                1.0,
                ui::TEAL,
            );
        }
    }

    pub(super) fn draw_chrome_tooltip(&self, canvas: &mut Canvas) {
        if self.menu || self.chrome.pressed.is_some() {
            return;
        }
        let Some(action) = self.chrome.hover.or(self.chrome.focus) else {
            return;
        };
        if !self.chrome_enabled(action) {
            return;
        }
        use ChromeAction::*;
        let text = match action {
            Back => "Back  Alt+Left",
            Forward => "Forward  Alt+Right",
            Reload if self.is_loading() => "Stop loading  Esc",
            Reload => "Reload  F5 | Full reload  Ctrl+F5",
            Home => "Home  Alt+Home",
            Bookmark if self.current_bookmarked() => {
                "Edit bookmark  Ctrl+D | Drag to bookmarks bar"
            }
            Bookmark => "Bookmark page  Ctrl+D | Drag to bookmarks bar",
            Downloads => "Downloads | Save current page  Ctrl+S",
            Menu => "Menu  Alt+F",
            FindPrevious => "Previous match  Shift+F3",
            FindNext => "Next match  F3",
            FindClose => "Close find  Esc",
            MenuItem(_) => return,
        };
        let Some((x, y, width, height)) = self.chrome_rect(action) else {
            return;
        };
        let w = text.len() as i32 * 6 + 20;
        let left = (x + width / 2 - w / 2).clamp(8, (self.width - w - 8).max(8));
        ui::rounded_rect(canvas, (left, y + height + 7, w, 27), 6.0, ui::NAVY);
        ui::rounded_border(
            canvas,
            (left, y + height + 7, w, 27),
            6.0,
            1.0,
            ui::TEAL_DARK,
        );
        canvas.text(left + 10, y + height + 17, text, 1, ui::CREAM);
    }

    /// Pointer leave must not terminate a captured text selection or scrollbar drag.
    pub fn pointer_leave(&mut self) -> bool {
        let css_changed=self.clear_native_hover();
        let changed = self.chrome.hover.take().is_some() || self.hover.is_some();
        self.hover = None;
        let panel_changed =
            self.browser_panel.is_some() && self.browser_pointer_move(-1, -1, false);
        changed || panel_changed || css_changed
    }
}
