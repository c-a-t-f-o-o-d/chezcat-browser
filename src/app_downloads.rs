//! Browser-owned download popup. It never creates document nodes, executes a
//! downloaded file, or exposes download paths to page JavaScript.

use super::*;
use crate::downloads::{self, Status};
use crate::ui::Icon;

const HEADER: i32 = 54;
const INTRO: i32 = 66;
const ROW: i32 = 82;
const FOOTER: i32 = 66;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Folder,
    Settings,
    Close,
    Current,
    Cancel(u64),
    Retry(u64),
    EntryFolder(u64),
    Details(u64),
    Clear,
}

#[derive(Default)]
pub(super) struct DownloadsUi {
    open: bool,
    first: usize,
    hover: Option<Action>,
    pressed: Option<Action>,
    focus: Option<Action>,
    pointer: bool,
    wheel: i32,
    error: String,
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
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

struct Popup {
    rect: Rect,
    first: usize,
    visible: usize,
    rows_y: i32,
    footer_y: i32,
}
struct Control {
    action: Action,
    rect: Rect,
    icon: Option<Icon>,
    label: &'static str,
    enabled: bool,
}

impl App {
    pub fn downloads_open(&self) -> bool {
        self.downloads_ui.open
    }

    pub fn downloads_active(&self) -> bool {
        self.downloads.active_count() > 0
    }

    pub fn show_downloads(&mut self) {
        self.reset_chrome();
        self.close_bookmark_popup();
        self.menu = false;
        self.select_popup = None;
        self.drag = Drag::None;
        self.focus = Focus::Page;
        self.inspector.focused = false;
        self.pending_surrogate = None;
        self.downloads_ui.open = true;
        self.downloads_ui.first = 0;
        self.downloads_ui.focus = Some(
            if self.location.url.is_some() || self.location.path.is_some() {
                Action::Current
            } else {
                Action::Folder
            },
        );
        self.downloads_ui.hover = None;
        self.downloads_ui.pressed = None;
        self.downloads_ui.pointer = false;
        self.downloads_ui.error.clear();
    }

    pub fn toggle_downloads(&mut self) {
        if self.downloads_open() {
            self.close_downloads();
        } else {
            self.show_downloads();
        }
    }

    pub fn close_downloads(&mut self) {
        self.downloads_ui.open = false;
        self.downloads_ui.hover = None;
        self.downloads_ui.pressed = None;
        self.downloads_ui.focus = None;
        self.downloads_ui.wheel = 0;
    }

    pub fn download_folder(&self) -> Result<PathBuf, String> {
        if let Some(folder) = &self.profile.data().settings.download_folder {
            let path = PathBuf::from(folder);
            downloads::validate_folder(&path)?;
            Ok(path)
        } else {
            downloads::default_folder()
        }
    }

    fn download_error(&mut self, error: String) {
        self.status = format!("Download: {error}");
        self.failed = false;
        self.downloads_ui.error = error;
    }

    pub fn download_location(&mut self, location: Location, suggested: Option<String>) {
        self.show_downloads();
        let result = self.download_folder().and_then(|folder| {
            self.downloads.start(
                location,
                suggested,
                &folder,
                self.trust.clone(),
                self.wake.clone(),
            )
        });
        match result {
            Ok(_) => {
                self.status = "Download started. The network transfer is limited to 32 MiB.".into();
                self.failed = false;
            }
            Err(error) => self.download_error(error),
        }
    }

    pub fn download_received(&mut self, response: crate::http::Response) {
        self.show_downloads();
        let result = self.download_folder().and_then(|folder| {
            self.downloads
                .start_received(response, &folder, self.trust.clone(), self.wake.clone())
        });
        match result {
            Ok(_) => {
                self.status = "Saving the received attachment.".into();
                self.failed = false;
            }
            Err(error) => self.download_error(error),
        }
    }

    pub fn download_link(&mut self, href: &str, suggested: Option<String>) {
        match navigation::resolve(href, &self.base_location) {
            Ok(location) => self.download_location(location, suggested),
            Err(error) => {
                self.show_downloads();
                self.download_error(error);
            }
        }
    }

    /// The native shell calls this only for a physical Alt+click. Ordinary
    /// links can therefore be saved even when their server has no attachment
    /// header or their HTML has no download attribute.
    pub fn download_at(&mut self, x: i32, y: i32) -> bool {
        if self.downloads_open()
            || self.browser_panel.is_some()
            || self.menu
            || self.bookmark_popup_open()
            || self.view != View::Page
            || y < self.content_top()
            || y >= self.content_bottom()
        {
            return false;
        }
        let page_y = y - self.content_top() + self.scroll;
        let Some(href) = self.page.link_at(x, page_y).map(str::to_owned) else {
            return false;
        };
        let suggested = self
            .page
            .link_node_at(x, page_y)
            .and_then(|node| self.document.attr(node, "download"))
            .filter(|name| !name.is_empty())
            .map(str::to_owned);
        self.download_link(&href, suggested);
        true
    }

    /// Fetches the page URL as a file; linked images/styles are separate files.
    /// A POST result must not silently become a different GET or replay a form.
    pub fn download_current(&mut self) {
        if self.current_request.is_post() {
            self.show_downloads();
            self.download_error("This page is a POST result. Use a download link on the page; its form will not be resubmitted automatically.".into());
        } else if self.location.path.is_none() && self.location.url.is_none() {
            self.show_downloads();
            self.download_error(
                "Open a website or local file before downloading the page file.".into(),
            );
        } else {
            self.download_location(self.location.clone(), None);
        }
    }

    pub fn poll_downloads(&mut self) -> bool {
        let active: Vec<_> = self
            .downloads
            .entries()
            .iter()
            .filter(|entry| entry.status.active())
            .map(|entry| entry.id)
            .collect();
        let changed = self.downloads.poll();
        if changed && self.downloads_open() {
            if let Some(entry) = self
                .downloads
                .entries()
                .iter()
                .find(|entry| active.contains(&entry.id) && !entry.status.active())
            {
                self.status = match &entry.status {
                    Status::Complete => {
                        format!("Downloaded {} ({}).", entry.name, size(entry.bytes))
                    }
                    Status::Cancelled => format!("Download {} cancelled.", entry.name),
                    Status::Failed(error) => format!("Download {} failed: {error}", entry.name),
                    _ => self.status.clone(),
                };
                self.failed = false;
            }
            let popup = self.downloads_popup();
            self.downloads_ui.first = popup.first;
            if self.downloads_ui.focus.is_some_and(|focus| {
                !self
                    .downloads_controls(&popup)
                    .iter()
                    .any(|control| control.enabled && control.action == focus)
            }) {
                self.downloads_ui.focus = Some(Action::Current);
            }
        }
        changed
    }

    fn downloads_popup(&self) -> Popup {
        let width = (self.width - 24).clamp(1, 604);
        let top = BAR_HEIGHT + 4;
        let available = (self.height - FOOT_HEIGHT - top - 8).max(1);
        let count = self.downloads.entries().len();
        let capacity = ((available - HEADER - INTRO - FOOTER).max(0) / ROW).min(6) as usize;
        let visible = count.min(capacity);
        let first = self
            .downloads_ui
            .first
            .min(count.saturating_sub(capacity.max(1)));
        let rows_height =
            ((visible.max(1) as i32) * ROW).min((available - HEADER - INTRO - FOOTER).max(0));
        let height = (HEADER + INTRO + rows_height + FOOTER).min(available);
        let rect = Rect {
            x: (self.width - width - 12).max(0),
            y: top,
            w: width,
            h: height,
        };
        Popup {
            rect,
            first,
            visible,
            rows_y: top + HEADER + INTRO,
            footer_y: top + height - FOOTER,
        }
    }

    fn downloads_controls(&self, popup: &Popup) -> Vec<Control> {
        let r = popup.rect;
        let mut controls = vec![
            Control {
                action: Action::Folder,
                rect: Rect {
                    x: r.x + r.w - 122,
                    y: r.y + 10,
                    w: 32,
                    h: 32,
                },
                icon: Some(Icon::Folder),
                label: "Open download folder",
                enabled: true,
            },
            Control {
                action: Action::Settings,
                rect: Rect {
                    x: r.x + r.w - 84,
                    y: r.y + 10,
                    w: 32,
                    h: 32,
                },
                icon: Some(Icon::Settings),
                label: "Download settings",
                enabled: true,
            },
            Control {
                action: Action::Close,
                rect: Rect {
                    x: r.x + r.w - 46,
                    y: r.y + 10,
                    w: 32,
                    h: 32,
                },
                icon: Some(Icon::Close),
                label: "Close downloads",
                enabled: true,
            },
            Control {
                action: Action::Current,
                rect: Rect {
                    x: r.x + 16,
                    y: r.y + HEADER + 7,
                    w: (r.w - 32).min(268),
                    h: 30,
                },
                icon: Some(Icon::Download),
                label: "Download page file",
                enabled: self.location.url.is_some() || self.location.path.is_some(),
            },
        ];
        for (offset, entry) in self
            .downloads
            .entries()
            .iter()
            .skip(popup.first)
            .take(popup.visible)
            .enumerate()
        {
            let y = popup.rows_y + offset as i32 * ROW;
            if y + ROW > popup.footer_y {
                break;
            }
            let row = Rect {
                x: r.x + 12,
                y: y + 2,
                w: r.w - 24,
                h: ROW - 4,
            };
            controls.push(Control {
                action: Action::Details(entry.id),
                rect: Rect {
                    w: row.w - 50,
                    ..row
                },
                icon: None,
                label: "Download details",
                enabled: true,
            });
            let action = if entry.status.active() && entry.status != Status::Cancelling {
                Some((Action::Cancel(entry.id), Icon::Close, "Cancel download"))
            } else if entry.can_retry() {
                Some((Action::Retry(entry.id), Icon::Refresh, "Retry download"))
            } else if entry.status == Status::Complete {
                Some((
                    Action::EntryFolder(entry.id),
                    Icon::Folder,
                    "Open containing folder",
                ))
            } else {
                None
            };
            if let Some((action, icon, label)) = action {
                controls.push(Control {
                    action,
                    rect: Rect {
                        x: r.x + r.w - 50,
                        y: y + 12,
                        w: 32,
                        h: 32,
                    },
                    icon: Some(icon),
                    label,
                    enabled: true,
                });
            }
        }
        controls.push(Control {
            action: Action::Clear,
            rect: Rect {
                x: r.x + 16,
                y: popup.footer_y + 7,
                w: (r.w - 32).min(182),
                h: 30,
            },
            icon: Some(Icon::Trash),
            label: "Clear list",
            enabled: self
                .downloads
                .entries()
                .iter()
                .any(|entry| !entry.status.active()),
        });
        controls
    }

    fn download_hit(&self, x: i32, y: i32) -> Option<Action> {
        let popup = self.downloads_popup();
        if !popup.rect.contains(x, y) {
            return None;
        }
        self.downloads_controls(&popup)
            .into_iter()
            .find(|control| control.enabled && control.rect.contains(x, y))
            .map(|control| control.action)
    }

    pub(super) fn downloads_wants_hand(&self, x: i32, y: i32) -> bool {
        self.downloads_open() && self.download_hit(x, y).is_some()
    }

    pub(super) fn downloads_pointer_down(&mut self, x: i32, y: i32) -> bool {
        if !self.downloads_open() {
            return false;
        }
        self.downloads_ui.pointer = true;
        self.downloads_ui.pressed = self.download_hit(x, y);
        if !self.downloads_popup().rect.contains(x, y) {
            self.close_downloads();
        } else if let Some(action) = self.downloads_ui.pressed {
            self.downloads_ui.focus = Some(action);
        }
        true
    }

    pub(super) fn downloads_pointer_up(&mut self, x: i32, y: i32) -> Option<PlatformAction> {
        if !self.downloads_open() && !self.downloads_ui.pointer {
            return None;
        }
        self.downloads_ui.pointer = false;
        let pressed = self.downloads_ui.pressed.take();
        if self.downloads_open() {
            if let Some(action) =
                pressed.filter(|pressed| self.download_hit(x, y) == Some(*pressed))
            {
                return Some(self.activate_download_action(action));
            }
        }
        Some(PlatformAction::None)
    }

    pub(super) fn downloads_pointer_move(&mut self, x: i32, y: i32) -> bool {
        if !self.downloads_open() {
            return false;
        }
        let hover = self.download_hit(x, y);
        let changed = hover != self.downloads_ui.hover;
        self.downloads_ui.hover = hover;
        changed
    }

    pub(super) fn downloads_pointer_cancel(&mut self) {
        self.downloads_ui.pointer = false;
        self.downloads_ui.pressed = None;
        self.downloads_ui.hover = None;
    }

    pub(super) fn downloads_wheel(&mut self, delta: i32) -> bool {
        if !self.downloads_open() {
            return false;
        }
        self.downloads_ui.wheel = self.downloads_ui.wheel.saturating_add(delta);
        let steps = self.downloads_ui.wheel / 120;
        self.downloads_ui.wheel %= 120;
        if steps != 0 {
            let popup = self.downloads_popup();
            self.downloads_ui.first = (popup.first as i64 - i64::from(steps)).clamp(
                0,
                self.downloads
                    .entries()
                    .len()
                    .saturating_sub(popup.visible.max(1)) as i64,
            ) as usize;
            self.downloads_ui.hover = None;
            self.downloads_ui.pressed = None;
        }
        true
    }

    pub(super) fn downloads_key(
        &mut self,
        key: u32,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> Option<PlatformAction> {
        if !self.downloads_open() {
            return None;
        }
        if matches!(key, 0x74 | 0x75 | 0x7b | 0xa6 | 0xa7) {
            self.close_downloads();
            return None;
        }
        if key == 0x1b && !ctrl && !alt {
            self.close_downloads();
            return Some(PlatformAction::None);
        }
        if ctrl || alt {
            if (ctrl && key == 0x4c) || (alt && key == 0x44) {
                self.close_downloads();
                self.focus_address();
                return Some(PlatformAction::None);
            }
            // Other browser shortcuts retain their usual meaning after closing.
            self.close_downloads();
            return None;
        }
        match key {
            0x09 | 0x26 | 0x28 => {
                let popup = self.downloads_popup();
                let controls: Vec<_> = self
                    .downloads_controls(&popup)
                    .into_iter()
                    .filter(|control| control.enabled)
                    .collect();
                let reverse = key == 0x26 || key == 0x09 && shift;
                let current = controls
                    .iter()
                    .position(|control| Some(control.action) == self.downloads_ui.focus);
                let next = match current {
                    Some(index) if reverse => (index + controls.len() - 1) % controls.len(),
                    Some(index) => (index + 1) % controls.len(),
                    None => 0,
                };
                self.downloads_ui.focus = controls.get(next).map(|control| control.action);
            }
            0x0d | 0x20 => {
                if let Some(action) = self.downloads_ui.focus {
                    return Some(self.activate_download_action(action));
                }
            }
            0x21..=0x24 => {
                let popup = self.downloads_popup();
                let max = self
                    .downloads
                    .entries()
                    .len()
                    .saturating_sub(popup.visible.max(1));
                self.downloads_ui.first = match key {
                    0x21 => popup.first.saturating_sub(popup.visible),
                    0x22 => popup.first.saturating_add(popup.visible).min(max),
                    0x24 => 0,
                    _ => max,
                };
                self.downloads_ui.focus = Some(Action::Current);
            }
            _ => {}
        }
        Some(PlatformAction::None)
    }

    fn activate_download_action(&mut self, action: Action) -> PlatformAction {
        self.downloads_ui.error.clear();
        match action {
            Action::Close => self.close_downloads(),
            Action::Settings => {
                self.close_downloads();
                self.show_download_settings();
            }
            Action::Folder => match self
                .download_folder()
                .and_then(|folder| downloads::prepare_folder(&folder))
            {
                Ok(folder) => return PlatformAction::OpenFolder(folder),
                Err(error) => self.download_error(error),
            },
            Action::Current => self.download_current(),
            Action::Cancel(id) => {
                self.downloads.cancel(id);
            }
            Action::Retry(id) => {
                let result = self.download_folder().and_then(|folder| {
                    self.downloads
                        .retry(id, &folder, self.trust.clone(), self.wake.clone())
                });
                match result {
                    Ok(_) => {
                        self.downloads_ui.first = 0;
                        self.status = "Download restarted.".into();
                        self.failed = false;
                    }
                    Err(error) => self.download_error(error),
                }
            }
            Action::EntryFolder(id) => {
                let folder = self
                    .downloads
                    .entries()
                    .iter()
                    .find(|entry| entry.id == id)
                    .map(|entry| entry.folder.clone());
                if let Some(folder) = folder {
                    match downloads::prepare_folder(&folder) {
                        Ok(folder) => return PlatformAction::OpenFolder(folder),
                        Err(error) => self.download_error(error),
                    }
                }
            }
            Action::Details(id) => {
                if let Some(entry) = self.downloads.entries().iter().find(|entry| entry.id == id) {
                    self.status = match &entry.status {
                        Status::Failed(error) => format!("Download {}: {error}", entry.name),
                        _ => format!(
                            "{} | {} | {}",
                            entry.name,
                            status_text(entry),
                            entry.path.as_deref().unwrap_or(&entry.folder).display()
                        ),
                    };
                    self.failed = false;
                }
            }
            Action::Clear => {
                let count = self.downloads.clear_finished();
                self.downloads_ui.first = 0;
                self.status = format!("Cleared {count} finished download(s) from the list. Saved files remain in the download folder.");
                self.failed = false;
            }
        }
        PlatformAction::None
    }

    pub(super) fn draw_downloads(&self, canvas: &mut Canvas) {
        if !self.downloads_open() {
            return;
        }
        let popup = self.downloads_popup();
        let r = popup.rect;
        let palette = self.browser_palette();
        let old_clip = canvas.clip();
        canvas.set_clip(None);
        ui::rounded_rect(canvas, (r.x + 4, r.y + 5, r.w, r.h), 12.0, palette.border);
        ui::rounded_rect(canvas, r.tuple(), 12.0, palette.panel);
        ui::rounded_border(canvas, r.tuple(), 12.0, 1.0, palette.border);
        canvas.set_clip(Some(crate::paint::ClipRect::new(
            r.x + 1,
            r.y + 1,
            r.w - 2,
            r.h - 2,
        )));
        canvas.text(r.x + 18, r.y + 19, "Downloads", 2, palette.foreground);
        canvas.fill_rect(r.x + 12, r.y + HEADER - 1, r.w - 24, 1, palette.border);
        let limit = format!(
            "32 MiB per file | {} of {} active",
            self.downloads.active_count(),
            downloads::MAX_ACTIVE
        );
        canvas.text(r.x + 18, r.y + HEADER + 45, &limit, 1, palette.muted);
        if self.downloads.entries().is_empty() && popup.footer_y - popup.rows_y >= 64 {
            canvas.text(
                r.x + 20,
                popup.rows_y + 16,
                "No downloads this session.",
                2,
                palette.muted,
            );
            canvas.text(
                r.x + 20,
                popup.rows_y + 43,
                "Alt+click a page link to download its file.",
                1,
                palette.muted,
            );
        } else if popup.visible == 0 && popup.footer_y - popup.rows_y >= 14 {
            canvas.text(
                r.x + 20,
                popup.rows_y + 4,
                if self.downloads.entries().is_empty() {
                    "No downloads this session."
                } else {
                    "Enlarge the window to view download details."
                },
                1,
                palette.muted,
            );
        }
        for (offset, entry) in self
            .downloads
            .entries()
            .iter()
            .skip(popup.first)
            .take(popup.visible)
            .enumerate()
        {
            let y = popup.rows_y + offset as i32 * ROW;
            if y + ROW > popup.footer_y {
                break;
            }
            let row = Rect {
                x: r.x + 12,
                y: y + 2,
                w: r.w - 24,
                h: ROW - 4,
            };
            if self.downloads_ui.focus == Some(Action::Details(entry.id))
                || self.downloads_ui.hover == Some(Action::Details(entry.id))
            {
                ui::rounded_rect(canvas, row.tuple(), 6.0, palette.panel_alt);
            }
            let icon = match entry.status {
                Status::Complete => Icon::Check,
                Status::Failed(_) | Status::Cancelled => Icon::Stop,
                _ => Icon::Download,
            };
            let color = if matches!(entry.status, Status::Failed(_)) {
                palette.error
            } else {
                palette.foreground
            };
            ui::icon(canvas, icon, r.x + 20, y + 12, 20, color, palette.accent);
            let max = ((r.w - 122) / 12).max(1) as usize;
            canvas.text(r.x + 50, y + 12, &fit(&entry.name, max), 2, color);
            let detail = status_text(entry);
            canvas.text(
                r.x + 50,
                y + 36,
                &fit(&detail, ((r.w - 116) / 6).max(1) as usize),
                1,
                if matches!(entry.status, Status::Failed(_)) {
                    palette.error
                } else {
                    palette.muted
                },
            );
            canvas.text(
                r.x + 50,
                y + 53,
                &fit(&entry.source, ((r.w - 116) / 6).max(1) as usize),
                1,
                palette.muted,
            );
            if entry.status == Status::Saving {
                if let Some(total) = entry.total.filter(|total| *total > 0) {
                    let width = (r.w - 116).max(0);
                    canvas.fill_rect(r.x + 50, y + 69, width, 3, palette.border);
                    canvas.fill_rect(
                        r.x + 50,
                        y + 69,
                        (entry.bytes.min(total) * width as u64 / total) as i32,
                        3,
                        palette.accent,
                    );
                }
            }
            canvas.fill_rect(r.x + 20, y + ROW - 1, r.w - 40, 1, palette.border);
        }
        if self.downloads.entries().len() > popup.visible && popup.visible > 0 {
            let count = format!(
                "{}-{} of {}",
                popup.first + 1,
                popup.first + popup.visible,
                self.downloads.entries().len()
            );
            canvas.text(
                r.x + r.w - 136,
                popup.footer_y + 18,
                &count,
                1,
                palette.muted,
            );
        }
        for control in self.downloads_controls(&popup) {
            if !matches!(control.action, Action::Details(_)) {
                draw_control(canvas, &control, &self.downloads_ui, palette);
            }
        }
        let footer = if !self.downloads_ui.error.is_empty() {
            self.downloads_ui.error.as_str()
        } else if let Some(action) = self.downloads_ui.hover.or(self.downloads_ui.focus) {
            match action {
                Action::Folder => "Open the configured download folder",
                Action::Settings => "Change the download folder in Settings",
                Action::Current => "Downloads this file only; linked resources are separate",
                Action::Cancel(_) => "Cancel the transfer and remove its partial file",
                Action::Retry(_) => "Retry this download using the current download folder",
                Action::EntryFolder(_) => "Open the folder containing this file",
                Action::Clear => "Clear finished entries; saved files are kept",
                _ => "Session list only. Files are never opened automatically.",
            }
        } else {
            "Session list only. Files are never opened automatically."
        };
        canvas.text(
            r.x + 18,
            popup.footer_y + 46,
            &fit(footer, ((r.w - 36) / 6).max(1) as usize),
            1,
            if self.downloads_ui.error.is_empty() {
                palette.muted
            } else {
                palette.error
            },
        );
        canvas.set_clip(Some(old_clip));
    }
}

fn draw_control(canvas: &mut Canvas, control: &Control, state: &DownloadsUi, palette: ui::Palette) {
    let rect = control.rect;
    let active = control.enabled
        && (state.hover == Some(control.action) || state.pressed == Some(control.action));
    let color = if control.enabled {
        palette.foreground
    } else {
        palette.muted
    };
    ui::rounded_rect(
        canvas,
        rect.tuple(),
        6.0,
        if active {
            palette.selected
        } else {
            palette.panel_alt
        },
    );
    let icon_only = !matches!(control.action, Action::Current | Action::Clear);
    if let Some(icon) = control.icon {
        ui::icon(
            canvas,
            icon,
            rect.x + if icon_only { (rect.w - 20) / 2 } else { 9 },
            rect.y + (rect.h - 20) / 2,
            20,
            color,
            if control.enabled {
                palette.accent
            } else {
                palette.muted
            },
        );
    }
    if !icon_only {
        canvas.text(
            rect.x + 36,
            rect.y + (rect.h - 7) / 2,
            control.label,
            1,
            color,
        );
    }
    if control.enabled && state.focus == Some(control.action) {
        ui::rounded_border(
            canvas,
            (rect.x + 1, rect.y + 1, rect.w - 2, rect.h - 2),
            5.0,
            1.0,
            palette.accent,
        );
    }
}

fn fit(text: &str, count: usize) -> String {
    if text.chars().count() <= count {
        text.to_owned()
    } else if count <= 3 {
        ".".repeat(count)
    } else {
        text.chars().take(count - 3).collect::<String>() + "..."
    }
}

fn size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn status_text(entry: &downloads::Entry) -> String {
    let duration = format!("{:.1}s", entry.elapsed().as_secs_f64());
    match &entry.status {
        Status::Receiving => format!("Receiving - size pending | {duration}"),
        Status::Saving => format!(
            "Saving {} / {} | {duration}",
            size(entry.bytes),
            entry
                .total
                .map(size)
                .unwrap_or_else(|| "size pending".into())
        ),
        Status::Cancelling => "Cancelling...".into(),
        Status::Complete => format!("Saved {} | {duration}", size(entry.bytes)),
        Status::Cancelled => format!("Cancelled | {duration}"),
        Status::Failed(error) => format!("Failed: {}", error.lines().next().unwrap_or(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preview(app: &App, name: &str) {
        let Some(folder) = std::env::var_os("CHEZCAT_DOWNLOAD_PREVIEW_DIR") else {
            return;
        };
        use std::io::Write;
        let folder = PathBuf::from(folder);
        std::fs::create_dir_all(&folder).unwrap();
        let canvas = app.draw();
        let mut file = std::io::BufWriter::new(std::fs::File::create(folder.join(name)).unwrap());
        write!(file, "P6\n{} {}\n255\n", canvas.width, canvas.height).unwrap();
        for pixel in &canvas.pixels {
            file.write_all(&[(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8])
                .unwrap();
        }
        file.flush().unwrap();
    }

    #[test]
    fn download_popup_requires_release_inside_the_pressed_control() {
        let mut app = App::new();
        app.show_downloads();
        let popup = app.downloads_popup();
        let close = app
            .downloads_controls(&popup)
            .into_iter()
            .find(|control| control.action == Action::Close)
            .unwrap()
            .rect;
        app.pointer_down(close.x + 5, close.y + 5, false);
        app.pointer_up(close.x - 30, close.y + 5);
        assert!(app.downloads_open());
        app.pointer_down(close.x + 5, close.y + 5, false);
        app.pointer_up(close.x + 5, close.y + 5);
        assert!(!app.downloads_open());
    }

    #[test]
    fn dismissing_download_popup_consumes_both_pointer_edges() {
        let mut app = App::new();
        app.show_downloads();
        assert!(app.downloads_pointer_down(1, 1));
        assert!(!app.downloads_open());
        assert!(app.downloads_pointer_up(1, 1).is_some());
        assert!(app.downloads_pointer_up(1, 1).is_none());
    }

    #[test]
    fn download_settings_and_escape_are_keyboard_accessible() {
        let mut app = App::new();
        app.show_downloads();
        app.downloads_ui.focus = Some(Action::Settings);
        app.key(0x0d, false, false, false);
        assert!(!app.downloads_open());
        assert!(app.browser_panel.is_some());
        app.show_downloads();
        app.key(0x1b, false, false, false);
        assert!(!app.downloads_open());
    }

    #[test]
    fn popup_draws_in_both_themes_and_small_windows() {
        let mut app = App::new();
        app.show_downloads();
        for (width, height, dark) in [(1080, 760, false), (1080, 760, true), (480, 320, true)] {
            let mut data = app.profile.data().clone();
            data.settings.dark_mode = dark;
            app.profile.save(data).unwrap();
            app.resize(width, height);
            let mut canvas = Canvas::new(width as usize, height as usize);
            app.draw_downloads(&mut canvas);
            assert_eq!(canvas.pixels.len(), (width * height) as usize);
            let popup = app.downloads_popup();
            assert!(popup.rect.x >= 0 && popup.rect.x + popup.rect.w <= width);
            preview(
                &app,
                if width < 600 {
                    "downloads-compact.ppm"
                } else if dark {
                    "downloads-dark.ppm"
                } else {
                    "downloads-light.ppm"
                },
            );
        }
    }

    #[test]
    fn explicit_link_download_and_clear_button_keep_the_page_and_saved_file() {
        let mut nonce = [0u8; 8];
        crate::random::fill(&mut nonce).unwrap();
        let token: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        let root = std::env::temp_dir().join(format!("chezcat-download-ui-{token}"));
        std::fs::create_dir(&root).unwrap();
        // Keep the user-selected input in ordinary path syntax. On Windows,
        // canonicalize adds a device prefix that explicit navigation rejects.
        let source = root.join("page.html");
        let original = root.join("payload.bin");
        std::fs::write(
            &source,
            b"<!doctype html><a href=payload.bin>Save the file</a>",
        )
        .unwrap();
        std::fs::write(&original, b"original binary\0bytes").unwrap();
        let expected_location = navigation::explicit(&source).unwrap();
        {
            let mut app = App::new();
            let mut data = app.profile.data().clone();
            data.settings.download_folder =
                Some(root.join("Downloads").to_string_lossy().into_owned());
            app.profile.save(data).unwrap();
            app.open(&source);
            app.wait_for_load();
            assert!(!app.failed, "{}", app.status);
            assert_eq!(app.location, expected_location);
            let run = app.page.runs.iter().find(|run| run.link.is_some()).unwrap();
            let (x, y) = (run.x + 2, run.y + app.content_top() + 2);
            assert!(app.download_at(x, y));
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while app.downloads_active() {
                app.poll_load();
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            assert_eq!(app.location, expected_location);
            assert_eq!(app.downloads.entries()[0].status, Status::Complete);
            assert!(app.status.starts_with("Downloaded payload.bin"));
            let saved = app.downloads.entries()[0].path.clone().unwrap();
            assert_eq!(std::fs::read(&saved).unwrap(), b"original binary\0bytes");
            preview(&app, "downloads-completed.ppm");
            let popup = app.downloads_popup();
            let clear = app
                .downloads_controls(&popup)
                .into_iter()
                .find(|control| control.action == Action::Clear)
                .unwrap()
                .rect;
            app.pointer_down(clear.x + 4, clear.y + 4, false);
            app.pointer_up(clear.x + 4, clear.y + 4);
            assert!(app.downloads.entries().is_empty());
            assert!(saved.is_file());
            assert_eq!(app.location, expected_location);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
#[path = "app_attachment_tests.rs"]
mod attachment_tests;
