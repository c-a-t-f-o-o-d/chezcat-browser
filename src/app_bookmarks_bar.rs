//! Browser-owned bookmark strip, folder popups and same-window drag/drop.
use super::*;
use crate::profile::{Bookmark, BookmarkItem};
use crate::ui::Icon;

pub(super) const HEIGHT: i32 = 34;
const ROW: i32 = 34;
type Rect = (i32, i32, i32, i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Item(BookmarkItem),
    Overflow,
    Manage,
    Up,
}
#[derive(Clone)]
enum Source {
    Item(BookmarkItem),
    Link { title: String, url: String },
}
struct PendingDrag {
    source: Source,
    x: i32,
    y: i32,
    active: bool,
    from_page: bool,
    point: (i32, i32),
}
struct Popup {
    folder: u32,
    x: i32,
    first: usize,
    selected: usize,
}
#[derive(Default)]
pub(super) struct BookmarkBar {
    popup: Option<Popup>,
    hover: Option<Target>,
    pressed: Option<Target>,
    drag: Option<PendingDrag>,
    wheel: i32,
}

impl App {
    pub(super) fn chrome_bottom(&self) -> i32 {
        BAR_HEIGHT
            + if self.profile.data().settings.bookmark_bar {
                HEIGHT
            } else {
                0
            }
    }
    pub(super) fn current_bookmarked(&self) -> bool {
        let address = self.location.display();
        self.profile
            .data()
            .bookmarks
            .iter()
            .any(|entry| entry.url == address)
    }
    pub(super) fn close_bookmark_popup(&mut self) {
        self.bookmarks_bar.popup = None;
        self.bookmarks_bar.pressed = None;
        self.bookmarks_bar.hover = None;
        self.bookmarks_bar.drag = None;
        self.bookmarks_bar.wheel = 0;
    }
    pub(super) fn cancel_bookmark_pointer(&mut self) {
        // A normal native ReleaseCapture also calls pointer_cancel. It must
        // not dismiss a folder popup just opened by the completed click.
        self.bookmarks_bar.pressed = None;
        self.bookmarks_bar.hover = None;
        self.bookmarks_bar.drag = None;
    }
    pub(super) fn bookmark_drag_active(&self) -> bool {
        self.bookmarks_bar
            .drag
            .as_ref()
            .is_some_and(|drag| drag.active)
    }
    pub(super) fn bookmark_popup_open(&self) -> bool {
        self.bookmarks_bar.popup.is_some()
    }

    fn bar_items(&self) -> Vec<(Target, Rect)> {
        if !self.profile.data().settings.bookmark_bar {
            return Vec::new();
        }
        let mut output = Vec::new();
        let mut x = 10;
        let rows = self.profile.data().items(0);
        for item in rows {
            let title = self.profile.data().item_title(item);
            let width = (title.chars().count() as i32 * 6 + 38).clamp(54, 170);
            if x + width > self.width - 86 {
                output.push((
                    Target::Overflow,
                    (
                        x.min((self.width - 78).max(0)),
                        BAR_HEIGHT + 3,
                        32,
                        HEIGHT - 6,
                    ),
                ));
                break;
            }
            output.push((Target::Item(item), (x, BAR_HEIGHT + 3, width, HEIGHT - 6)));
            x += width + 4;
        }
        output.push((
            Target::Manage,
            ((self.width - 38).max(0), BAR_HEIGHT + 3, 30, HEIGHT - 6),
        ));
        output
    }
    fn popup_rect(&self) -> Option<Rect> {
        let popup = self.bookmarks_bar.popup.as_ref()?;
        let count = self.profile.data().items(popup.folder).len().max(1);
        let width = 342.min((self.width - 12).max(1));
        let height = (70 + count.min(12) as i32 * ROW)
            .min((self.height - self.chrome_bottom() - FOOT_HEIGHT - 8).max(72));
        Some((
            popup.x.min((self.width - width - 6).max(0)).max(0),
            self.chrome_bottom(),
            width,
            height,
        ))
    }
    fn popup_rows(&self) -> usize {
        self.popup_rect()
            .map_or(1, |r| ((r.3 - 70) / ROW).max(1) as usize)
    }
    fn popup_items(&self) -> Vec<(Target, Rect)> {
        let Some(popup) = &self.bookmarks_bar.popup else {
            return Vec::new();
        };
        let Some((x, y, w, h)) = self.popup_rect() else {
            return Vec::new();
        };
        let mut controls = vec![
            (Target::Up, (x + 4, y + 4, w - 8, 30)),
            (Target::Manage, (x + 4, y + h - 32, w - 8, 28)),
        ];
        for (offset, item) in self
            .profile
            .data()
            .items(popup.folder)
            .into_iter()
            .skip(popup.first)
            .take(self.popup_rows())
            .enumerate()
        {
            controls.push((
                Target::Item(item),
                (x + 4, y + 36 + offset as i32 * ROW, w - 8, ROW),
            ));
        }
        controls
    }
    fn bookmark_hit(&self, x: i32, y: i32) -> Option<Target> {
        self.popup_items()
            .into_iter()
            .chain(self.bar_items())
            .find(|(_, r)| contains(*r, x, y))
            .map(|(target, _)| target)
    }
    pub(super) fn bookmark_bar_wants_hand(&self, x: i32, y: i32) -> bool {
        self.bookmark_hit(x, y).is_some()
    }
    pub(super) fn bookmark_bar_pointer_down(&mut self, x: i32, y: i32) -> bool {
        let hit = self.bookmark_hit(x, y);
        if self.bookmark_popup_open() && hit.is_none() {
            self.close_bookmark_popup();
            return true;
        }
        if let Some(target) = hit {
            self.close_downloads();
            self.menu = false;
            self.chrome.pressed = None;
            self.focus = Focus::Page;
            self.drag = Drag::None;
            self.bookmarks_bar.pressed = Some(target);
            self.bookmarks_bar.hover = Some(target);
            if let Target::Item(item) = target {
                self.bookmarks_bar.drag = Some(PendingDrag {
                    source: Source::Item(item),
                    x,
                    y,
                    active: false,
                    from_page: false,
                    point: (x, y),
                });
            }
            return true;
        }
        if (BAR_HEIGHT..self.chrome_bottom()).contains(&y) {
            self.bookmarks_bar.pressed = None;
            return true;
        }
        false
    }
    pub(super) fn arm_current_bookmark_drag(&mut self, x: i32, y: i32) {
        self.bookmarks_bar.drag = Some(PendingDrag {
            source: Source::Link {
                title: self.document.title.clone(),
                url: self.location.display(),
            },
            x,
            y,
            active: false,
            from_page: false,
            point: (x, y),
        });
    }
    pub(super) fn arm_page_bookmark_drag(&mut self, href: &str, x: i32, y: i32) {
        let Ok(location) = navigation::resolve(href, &self.base_location) else {
            return;
        };
        let title = self
            .page
            .runs
            .iter()
            .filter(|run| {
                run.link
                    .and_then(|index| self.page.link_targets().nth(index))
                    == Some(href)
            })
            .map(|run| run.text.as_str())
            .take(24)
            .collect::<Vec<_>>()
            .join(" ");
        self.bookmarks_bar.drag = Some(PendingDrag {
            source: Source::Link {
                title,
                url: location.display(),
            },
            x,
            y,
            active: false,
            from_page: true,
            point: (x, y),
        });
    }
    pub(super) fn bookmark_bar_pointer_move(&mut self, x: i32, y: i32, dragging: bool) -> bool {
        let hit = self.bookmark_hit(x, y);
        let changed = self.bookmarks_bar.hover != hit;
        self.bookmarks_bar.hover = hit;
        let bar_bottom = self.chrome_bottom();
        if dragging {
            if let Some(drag) = &mut self.bookmarks_bar.drag {
                drag.point = (x, y);
                let moved = (x - drag.x).abs() > 6 || (y - drag.y).abs() > 6;
                if moved && (!drag.from_page || (BAR_HEIGHT..bar_bottom).contains(&y)) {
                    drag.active = true;
                }
                if drag.active {
                    self.drag = Drag::None;
                    self.selection = None;
                    self.chrome.pressed = None;
                    return true;
                }
            }
        }
        changed
    }
    fn bookmark_drop_target(&self, x: i32, y: i32) -> Option<(u32, usize, Option<Rect>)> {
        let parent = if let Some(popup) = &self.bookmarks_bar.popup {
            if self.popup_rect().is_some_and(|rect| contains(rect, x, y)) {
                popup.folder
            } else if (BAR_HEIGHT..self.chrome_bottom()).contains(&y) {
                0
            } else {
                return None;
            }
        } else if (BAR_HEIGHT..self.chrome_bottom()).contains(&y) {
            0
        } else {
            return None;
        };
        let rows = self.profile.data().items(parent);
        let controls = if parent != 0 || y >= self.chrome_bottom() {
            self.popup_items()
        } else {
            self.bar_items()
        };
        for (target, rect) in controls {
            let Target::Item(item) = target else {
                continue;
            };
            if !contains(rect, x, y) {
                continue;
            }
            let position = rows.iter().position(|candidate| *candidate == item)?;
            let popup = y >= self.chrome_bottom();
            let edge = if popup {
                (y - rect.1, rect.3)
            } else {
                (x - rect.0, rect.2)
            };
            if let BookmarkItem::Folder(id) = item {
                if edge.0 > edge.1 / 4 && edge.0 < edge.1 * 3 / 4 {
                    return Some((id, self.profile.data().items(id).len(), Some(rect)));
                }
            }
            let after = edge.0 > edge.1 / 2;
            let marker = if popup {
                (rect.0, rect.1 + if after { rect.3 } else { 0 }, rect.2, 2)
            } else {
                (rect.0 + if after { rect.2 } else { 0 }, rect.1, 2, rect.3)
            };
            return Some((parent, position + usize::from(after), Some(marker)));
        }
        Some((parent, rows.len(), None))
    }
    pub(super) fn bookmark_bar_pointer_up(&mut self, x: i32, y: i32) -> bool {
        if let Some(drag) = self.bookmarks_bar.drag.take() {
            if drag.active {
                self.bookmarks_bar.pressed = None;
                if let Some((parent, mut position, _)) = self.bookmark_drop_target(x, y) {
                    let mut data = self.profile.data().clone();
                    let item = match drag.source {
                        Source::Item(item) => item,
                        Source::Link { title, url } => {
                            if let Some(index) = data.bookmarks.iter().position(|b| b.url == url) {
                                BookmarkItem::Link(index)
                            } else {
                                if data.bookmarks.len() >= crate::profile::MAX_BOOKMARKS {
                                    self.profile_error(
                                        "The bookmark limit has been reached.".into(),
                                    );
                                    return true;
                                }
                                let title = if title.trim().is_empty() {
                                    url.clone()
                                } else {
                                    title.chars().take(220).collect()
                                };
                                data.bookmarks.push(Bookmark {
                                    title,
                                    url,
                                    folder: parent,
                                    order: data.next_order(parent),
                                });
                                BookmarkItem::Link(data.bookmarks.len() - 1)
                            }
                        }
                    };
                    if data.item_parent(item) == Some(parent) {
                        if let Some(old) = data.items(parent).iter().position(|v| *v == item) {
                            if old < position {
                                position = position.saturating_sub(1);
                            }
                        }
                    }
                    match data
                        .move_item(item, parent, position)
                        .and_then(|()| self.profile.save(data))
                    {
                        Ok(()) => {
                            self.status = "Bookmark saved in its new position.".into();
                            self.failed = false;
                        }
                        Err(error) => self.profile_error(error),
                    }
                }
                self.drag = Drag::None;
                return true;
            }
        }
        let Some(pressed) = self.bookmarks_bar.pressed.take() else {
            return false;
        };
        if self.bookmark_hit(x, y) == Some(pressed) {
            self.activate_bookmark_target(pressed, x);
        }
        true
    }
    fn activate_bookmark_target(&mut self, target: Target, x: i32) {
        match target {
            Target::Item(BookmarkItem::Link(index)) => {
                let Some(url) = self
                    .profile
                    .data()
                    .bookmarks
                    .get(index)
                    .map(|b| b.url.clone())
                else {
                    return;
                };
                self.close_bookmark_popup();
                self.close_browser_panel();
                self.open_address(&url);
            }
            Target::Item(BookmarkItem::Folder(folder)) => {
                let x = self.bookmarks_bar.popup.as_ref().map_or(x, |popup| popup.x);
                self.bookmarks_bar.popup = Some(Popup {
                    folder,
                    x,
                    first: 0,
                    selected: 0,
                });
            }
            Target::Overflow => {
                self.bookmarks_bar.popup = Some(Popup {
                    folder: 0,
                    x,
                    first: 0,
                    selected: 0,
                })
            }
            Target::Manage => {
                self.close_bookmark_popup();
                self.show_bookmarks();
            }
            Target::Up => {
                let parent = self
                    .bookmarks_bar
                    .popup
                    .as_ref()
                    .and_then(|p| self.profile.data().folder(p.folder))
                    .map(|f| f.parent);
                if let Some(parent) = parent {
                    if let Some(p) = &mut self.bookmarks_bar.popup {
                        p.folder = parent;
                        p.first = 0;
                        p.selected = 0;
                    }
                } else {
                    self.close_bookmark_popup();
                }
            }
        }
    }
    pub(super) fn bookmark_bar_wheel(&mut self, delta: i32) -> bool {
        if !self.bookmark_popup_open() {
            return false;
        }
        let rows = self.popup_rows();
        let count = self
            .profile
            .data()
            .items(self.bookmarks_bar.popup.as_ref().unwrap().folder)
            .len();
        self.bookmarks_bar.wheel = self.bookmarks_bar.wheel.saturating_add(delta);
        let step = self.bookmarks_bar.wheel / 120;
        self.bookmarks_bar.wheel %= 120;
        if let Some(p) = &mut self.bookmarks_bar.popup {
            p.first =
                (p.first as i32 - step * 3).clamp(0, count.saturating_sub(rows) as i32) as usize;
        }
        true
    }
    pub(super) fn bookmark_bar_key(
        &mut self,
        key: u32,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> bool {
        if !self.bookmark_popup_open() {
            return false;
        }
        if key == 0x1b {
            self.close_bookmark_popup();
            return true;
        }
        if ctrl || alt {
            self.close_bookmark_popup();
            return false;
        }
        let popup = self.bookmarks_bar.popup.as_ref().unwrap();
        let items = self.profile.data().items(popup.folder);
        if key == 0x08 || key == 0x25 {
            self.activate_bookmark_target(Target::Up, 0);
            return true;
        }
        if key == 0x09 {
            self.close_bookmark_popup();
            self.show_bookmarks();
            return true;
        }
        if let Some(item) = items.get(popup.selected).copied() {
            if key == 0x0d || key == 0x27 {
                self.activate_bookmark_target(Target::Item(item), popup.x);
                return true;
            }
        }
        let rows = self.popup_rows();
        if let Some(p) = &mut self.bookmarks_bar.popup {
            match key {
                0x26 => p.selected = p.selected.saturating_sub(1),
                0x28 => p.selected = (p.selected + 1).min(items.len().saturating_sub(1)),
                0x24 => p.selected = 0,
                0x23 => p.selected = items.len().saturating_sub(1),
                _ => {
                    let _ = shift;
                }
            }
            if p.selected < p.first {
                p.first = p.selected;
            }
            if p.selected >= p.first + rows {
                p.first = p.selected + 1 - rows;
            }
        }
        true
    }
    pub(super) fn draw_bookmarks_bar(&self, canvas: &mut Canvas) {
        if !self.profile.data().settings.bookmark_bar {
            return;
        }
        let p = self.browser_palette();
        canvas.fill_rect(0, BAR_HEIGHT, self.width, HEIGHT, p.panel_alt);
        canvas.fill_rect(0, BAR_HEIGHT + HEIGHT - 1, self.width, 1, p.border);
        if self.profile.data().items(0).is_empty() {
            canvas.text(
                12,
                BAR_HEIGHT + 12,
                &fit_text(
                    "Drag a page link or the address bookmark icon here. Ctrl+B manages bookmarks.",
                    ((self.width - 60) / 6).max(0) as usize,
                ),
                1,
                p.muted,
            );
        }
        for (target, rect) in self.bar_items() {
            self.draw_bookmark_item(canvas, target, rect, false);
        }
    }
    fn draw_bookmark_item(&self, canvas: &mut Canvas, target: Target, rect: Rect, selected: bool) {
        let p = self.browser_palette();
        let (x, y, w, h) = rect;
        if self.bookmarks_bar.hover == Some(target) || selected {
            ui::rounded_rect(canvas, rect, 6.0, p.selected);
        }
        let (icon, title) = match target {
            Target::Item(item) => (
                if matches!(item, BookmarkItem::Folder(_)) {
                    Icon::Folder
                } else {
                    Icon::Bookmark
                },
                self.profile.data().item_title(item),
            ),
            Target::Overflow => (Icon::Forward, ""),
            Target::Manage => (
                Icon::Settings,
                if y >= self.chrome_bottom() {
                    "Manage bookmarks   Ctrl+B"
                } else {
                    ""
                },
            ),
            Target::Up => (Icon::Back, ""),
        };
        ui::icon(
            canvas,
            icon,
            x + 6,
            y + (h - 18) / 2,
            18,
            p.foreground,
            p.accent,
        );
        if !title.is_empty() {
            canvas.text(
                x + 30,
                y + (h - 7) / 2,
                &fit_text(title, ((w - 38) / 6).max(0) as usize),
                1,
                p.foreground,
            );
        }
        if matches!(target, Target::Item(BookmarkItem::Folder(_))) && y >= self.chrome_bottom() {
            ui::icon(
                canvas,
                Icon::Forward,
                x + w - 22,
                y + 10,
                14,
                p.muted,
                p.muted,
            );
        }
    }
    pub(super) fn draw_bookmark_popup(&self, canvas: &mut Canvas) {
        let p = self.browser_palette();
        if let (Some(popup), Some(rect)) = (&self.bookmarks_bar.popup, self.popup_rect()) {
            let (x, y, w, h) = rect;
            ui::rounded_rect(canvas, (x + 3, y + 4, w, h), 9.0, 0x07101c);
            ui::rounded_rect(canvas, rect, 9.0, p.panel);
            ui::rounded_border(canvas, rect, 9.0, 1.0, p.border);
            let items = self.profile.data().items(popup.folder);
            for (target, r) in self.popup_items() {
                self.draw_bookmark_item(
                    canvas,
                    target,
                    r,
                    items
                        .get(popup.selected)
                        .is_some_and(|v| target == Target::Item(*v)),
                );
            }
            let title = self
                .profile
                .data()
                .folder(popup.folder)
                .map_or("Bookmarks", |f| f.title.as_str());
            canvas.text(
                x + 34,
                y + 15,
                &fit_text(title, ((w - 50) / 6).max(0) as usize),
                1,
                p.foreground,
            );
            canvas.fill_rect(x + 8, y + 34, w - 16, 1, p.border);
            canvas.fill_rect(x + 8, y + h - 35, w - 16, 1, p.border);
            if items.is_empty() {
                canvas.text(x + 16, y + 50, "This folder is empty.", 1, p.muted);
            }
            if items.len() > self.popup_rows() {
                let text = format!(
                    "{}-{} / {}",
                    popup.first + 1,
                    (popup.first + self.popup_rows()).min(items.len()),
                    items.len()
                );
                canvas.text(
                    x + w - (text.len() as i32 * 6) - 12,
                    y + h - 18,
                    &text,
                    1,
                    p.muted,
                );
            }
        }
        if let Some(drag) = self.bookmarks_bar.drag.as_ref().filter(|drag| drag.active) {
            if let Some((_, _, Some(marker))) =
                self.bookmark_drop_target(drag.point.0, drag.point.1)
            {
                ui::rounded_border(canvas, marker, 2.0, 2.0, p.accent);
                if marker.2 <= 2 || marker.3 <= 2 {
                    canvas.fill_rect(marker.0, marker.1, marker.2, marker.3, p.accent);
                }
            }
            let title = match &drag.source {
                Source::Item(item) => self.profile.data().item_title(*item),
                Source::Link { title, .. } => title.as_str(),
            };
            let text = fit_text(title, 28);
            let width = text.chars().count() as i32 * 6 + 34;
            let x = drag.point.0.min((self.width - width - 8).max(0));
            let y = (drag.point.1 + 18).min(self.height - FOOT_HEIGHT - 30);
            ui::rounded_rect(canvas, (x, y, width, 26), 6.0, p.chrome);
            ui::icon(
                canvas,
                Icon::Bookmark,
                x + 6,
                y + 5,
                16,
                p.chrome_text,
                p.accent,
            );
            canvas.text(x + 28, y + 10, &text, 1, p.chrome_text);
        }
    }
}

fn contains((left, top, width, height): Rect, x: i32, y: i32) -> bool {
    x >= left && y >= top && x < left + width && y < top + height
}

#[cfg(test)]
mod tests {
    use super::*;
    fn click(app: &mut App, rect: Rect) {
        let (x, y, w, h) = rect;
        app.pointer_down(x + w / 2, y + h / 2, false);
        app.pointer_up(x + w / 2, y + h / 2);
        // Windows sends this after the ordinary release as capture ends.
        app.pointer_cancel();
    }
    fn bar_rect(app: &App, item: BookmarkItem) -> Rect {
        app.bar_items()
            .into_iter()
            .find(|(target, _)| *target == Target::Item(item))
            .unwrap()
            .1
    }
    #[test]
    fn folder_popup_survives_capture_release_and_opens_nested_folder() {
        let mut app = App::new();
        let mut data = app.profile.data().clone();
        let first = data.add_folder(0, "Research".into()).unwrap();
        let second = data.add_folder(first, "Specifications".into()).unwrap();
        app.profile.save(data).unwrap();
        let rect = bar_rect(&app, BookmarkItem::Folder(first));
        click(&mut app, rect);
        assert!(app.bookmark_popup_open());
        let rect = app
            .popup_items()
            .into_iter()
            .find(|(target, _)| *target == Target::Item(BookmarkItem::Folder(second)))
            .unwrap()
            .1;
        click(&mut app, rect);
        assert_eq!(app.bookmarks_bar.popup.as_ref().unwrap().folder, second);
        app.key(0x1b, false, false, false);
        assert!(!app.bookmark_popup_open());
    }
    #[test]
    fn page_and_address_drags_save_urls_without_navigating_or_deleting_address_text() {
        let mut app = App::new();
        app.document=html::parse("<title>Reference page</title><body><a href='https://example.com/reference'>Reference</a></body>");
        app.forms = Forms::new(&app.document);
        app.reflow();
        let before = app.location.display();
        let run = app.page.runs.iter().find(|run| run.link.is_some()).unwrap();
        let (x, y) = (run.x + 2, run.y + app.content_top() + 3);
        app.pointer_down(x, y, false);
        app.pointer_move(320, BAR_HEIGHT + 17, true);
        app.pointer_up(320, BAR_HEIGHT + 17);
        app.pointer_cancel();
        assert_eq!(app.location.display(), before);
        assert_eq!(
            app.profile.data().bookmarks[0].url,
            "https://example.com/reference"
        );
        let rect = app.chrome_rect(chrome::ChromeAction::Bookmark).unwrap();
        let (x, y) = (rect.0 + rect.2 / 2, rect.1 + rect.3 / 2);
        app.pointer_down(x, y, false);
        app.pointer_move(500, BAR_HEIGHT + 17, true);
        app.pointer_up(500, BAR_HEIGHT + 17);
        app.pointer_cancel();
        assert_eq!(app.address.text(), before);
        assert!(app.current_bookmarked());
        assert_eq!(app.profile.data().bookmarks.len(), 2);
    }
    #[test]
    fn toolbar_reordering_and_folder_drop_persist_without_cycles() {
        let mut app = App::new();
        let mut data = app.profile.data().clone();
        let folder = data.add_folder(0, "Tools".into()).unwrap();
        let child = data.add_folder(folder, "Nested".into()).unwrap();
        for (i, title) in ["One", "Two"].into_iter().enumerate() {
            data.bookmarks.push(Bookmark {
                title: title.into(),
                url: format!("https://example.com/{i}"),
                folder: 0,
                order: i as u32 + 1,
            });
        }
        app.profile.save(data).unwrap();
        let from = bar_rect(&app, BookmarkItem::Link(1));
        let to = bar_rect(&app, BookmarkItem::Link(0));
        app.pointer_down(from.0 + from.2 / 2, from.1 + 12, false);
        app.pointer_move(to.0 + 2, to.1 + 12, true);
        app.pointer_up(to.0 + 2, to.1 + 12);
        app.pointer_cancel();
        assert_eq!(
            app.profile.data().items(0),
            vec![
                BookmarkItem::Folder(folder),
                BookmarkItem::Link(1),
                BookmarkItem::Link(0)
            ]
        );
        let from = bar_rect(&app, BookmarkItem::Link(0));
        let to = bar_rect(&app, BookmarkItem::Folder(folder));
        app.pointer_down(from.0 + 15, from.1 + 12, false);
        app.pointer_move(to.0 + to.2 / 2, to.1 + 12, true);
        app.pointer_up(to.0 + to.2 / 2, to.1 + 12);
        app.pointer_cancel();
        assert_eq!(app.profile.data().bookmarks[0].folder, folder);
        click(&mut app, to);
        let nested = app
            .popup_items()
            .into_iter()
            .find(|(target, _)| *target == Target::Item(BookmarkItem::Folder(child)))
            .unwrap()
            .1;
        let before = app.profile.data().clone();
        app.pointer_down(to.0 + 15, to.1 + 12, false);
        app.pointer_move(nested.0 + nested.2 / 2, nested.1 + 17, true);
        app.pointer_up(nested.0 + nested.2 / 2, nested.1 + 17);
        assert_eq!(*app.profile.data(), before);
        assert!(app.status.contains("subfolders"));
    }
}
