//! Browser-owned ordered bookmark folders. Every mutation stages and validates
//! the profile before committing; documents cannot access this tree or editor.
use super::*;
use crate::profile::BookmarkItem;

impl App {
    pub(super) fn bookmark_editor_return(&mut self) -> Option<Box<BrowserPanel>> {
        self.browser_panel.take().and_then(|mut panel| {
            if matches!(panel.kind, PanelKind::Bookmarks | PanelKind::Settings) {
                Some(Box::new(panel))
            } else {
                panel.return_panel.take()
            }
        })
    }

    pub(super) fn bookmark_tree_items(&self) -> Vec<BookmarkItem> {
        self.profile
            .data()
            .items(self.browser_panel.as_ref().map_or(0, |panel| panel.folder))
    }

    pub(super) fn bookmark_selected_item(&self) -> Option<BookmarkItem> {
        let panel = self.browser_panel.as_ref()?;
        panel
            .selected_folder
            .map(BookmarkItem::Folder)
            .or_else(|| panel.selected.map(BookmarkItem::Link))
    }

    fn select_bookmark_item(&mut self, item: Option<BookmarkItem>) {
        if let Some(panel) = &mut self.browser_panel {
            panel.selected = match item {
                Some(BookmarkItem::Link(index)) => Some(index),
                _ => None,
            };
            panel.selected_folder = match item {
                Some(BookmarkItem::Folder(id)) => Some(id),
                _ => None,
            };
            panel.focus = Action::List;
        }
    }

    pub(super) fn bookmark_tree_row(&self, index: usize) -> Option<Action> {
        self.bookmark_tree_items()
            .get(index)
            .map(|item| match item {
                BookmarkItem::Link(index) => Action::Row(*index),
                BookmarkItem::Folder(id) => Action::FolderRow(*id),
            })
    }

    pub(in crate::app) fn show_bookmark_folder(&mut self, id: u32) {
        if let Err(error) = self.refresh_profile() { self.profile_error(error); return; }
        let id = if id == 0 || self.profile.data().folder(id).is_some() {
            id
        } else {
            0
        };
        let previous = self.browser_panel.take();
        let mut panel = BrowserPanel::new(PanelKind::Bookmarks, Action::List);
        panel.profile_revision = Some(self.profile.revision());
        panel.folder = id;
        panel.return_panel = previous.and_then(|mut previous| match previous.kind {
            PanelKind::Settings => Some(Box::new(previous)),
            PanelKind::Bookmarks => previous.return_panel.take(),
            PanelKind::EditBookmark { .. }
            | PanelKind::EditFolder { .. }
            | PanelKind::DeleteFolder { .. } => {
                previous.return_panel.take().and_then(|mut manager| {
                    if matches!(manager.kind, PanelKind::Bookmarks) {
                        manager.return_panel.take()
                    } else {
                        Some(manager)
                    }
                })
            }
            _ => None,
        });
        panel.error = self.profile.problem().unwrap_or("").to_owned();
        self.begin_browser_panel(panel);
        self.select_bookmark_item(self.profile.data().items(id).first().copied());
    }

    pub(super) fn bookmark_editor_controls(
        &self,
        body: Rect,
        panel: &BrowserPanel,
    ) -> Vec<Control> {
        vec![Control {
            action: Action::Destination,
            rect: Rect {
                x: body.x,
                y: body.y + 208 - panel.scroll,
                w: body.w,
                h: 34,
            },
            label: "Change folder",
            enabled: true,
        }]
    }

    pub(super) fn bookmark_tree_controls(&self, body: Rect, panel: &BrowserPanel) -> Vec<Control> {
        let footer = self.browser_footer_y();
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
        match panel.kind {
            PanelKind::Bookmarks => {
                controls.push(Control {
                    action: Action::List,
                    rect: body,
                    label: "",
                    enabled: true,
                });
                controls.push(Control {
                    action: Action::Add,
                    rect: Rect {
                        x: (self.width - 186).max(body.x),
                        y: self.chrome_bottom() + 12,
                        w: 124,
                        h: 32,
                    },
                    label: "Add page",
                    enabled: true,
                });
                if self.width >= 760 {
                    controls.push(Control {
                        action: Action::SettingsBack,
                        rect: Rect {
                            x: self.width - 304,
                            y: self.chrome_bottom() + 12,
                            w: 108,
                            h: 32,
                        },
                        label: "Settings",
                        enabled: true,
                    });
                }
                let items = self.bookmark_tree_items();
                let selected = self.bookmark_selected_item();
                let position = selected.and_then(|item| items.iter().position(|row| *row == item));
                let actions = [
                    (Action::Open, "Open", selected.is_some()),
                    (Action::Edit, "Edit", selected.is_some()),
                    (Action::Move, "Move", selected.is_some()),
                    (Action::Delete, "Delete", selected.is_some()),
                    (Action::NewFolder, "New folder", true),
                    (Action::FolderUp, "Parent", panel.folder != 0),
                    (Action::MoveUp, "Move up", position.is_some_and(|i| i > 0)),
                    (
                        Action::MoveDown,
                        "Move down",
                        position.is_some_and(|i| i + 1 < items.len()),
                    ),
                ];
                let gap = if body.w < 700 { 4 } else { 8 };
                let width =
                    ((body.w - gap * (actions.len() as i32 - 1)) / actions.len() as i32).max(1);
                for (index, (action, label, enabled)) in actions.into_iter().enumerate() {
                    controls.push(Control {
                        action,
                        rect: Rect {
                            x: body.x + index as i32 * (width + gap),
                            y: footer,
                            w: width,
                            h: 32,
                        },
                        label,
                        enabled,
                    });
                }
            }
            PanelKind::EditFolder { .. } => {
                controls.push(Control {
                    action: Action::Title,
                    rect: Rect {
                        x: body.x,
                        y: body.y + 28 - panel.scroll,
                        w: body.w,
                        h: 34,
                    },
                    label: "",
                    enabled: true,
                });
                controls.push(Control {
                    action: Action::Destination,
                    rect: Rect {
                        x: body.x,
                        y: body.y + 112 - panel.scroll,
                        w: body.w,
                        h: 34,
                    },
                    label: "Change parent folder",
                    enabled: true,
                });
                controls.extend(self.bookmark_dialog_buttons(
                    body,
                    footer,
                    Action::SaveFolder,
                    "Save folder",
                ));
            }
            PanelKind::FolderDestination => {
                for (index, (id, _)) in self.bookmark_destinations().into_iter().enumerate() {
                    controls.push(Control {
                        action: Action::DestinationRow(id),
                        rect: Rect {
                            x: body.x,
                            y: body.y + 34 + index as i32 * 40 - panel.scroll,
                            w: body.w,
                            h: 34,
                        },
                        label: "",
                        enabled: true,
                    });
                }
                controls.push(Control {
                    action: Action::Cancel,
                    rect: Rect {
                        x: body.x,
                        y: footer,
                        w: body.w,
                        h: 32,
                    },
                    label: "Cancel",
                    enabled: true,
                });
            }
            PanelKind::DeleteFolder { .. } => controls.extend(self.bookmark_dialog_buttons(
                body,
                footer,
                Action::ConfirmDeleteFolder,
                "Delete folder and contents",
            )),
            _ => {}
        }
        controls
    }

    fn bookmark_dialog_buttons(
        &self,
        body: Rect,
        footer: i32,
        action: Action,
        label: &'static str,
    ) -> Vec<Control> {
        let width = ((body.w - 8) / 2).max(1);
        vec![
            Control {
                action,
                rect: Rect {
                    x: body.x,
                    y: footer,
                    w: width,
                    h: 32,
                },
                label,
                enabled: true,
            },
            Control {
                action: Action::Cancel,
                rect: Rect {
                    x: body.x + width + 8,
                    y: footer,
                    w: width,
                    h: 32,
                },
                label: "Cancel",
                enabled: true,
            },
        ]
    }

    pub(super) fn bookmark_destinations(&self) -> Vec<(u32, String)> {
        let mut rows = vec![(0, self.profile.data().folder_path(0))];
        rows.extend(
            self.profile
                .data()
                .folders
                .iter()
                .map(|folder| (folder.id, self.profile.data().folder_path(folder.id))),
        );
        rows[1..].sort_by_key(|row| row.1.to_lowercase());
        rows
    }

    pub(super) fn folder_destination_height(&self) -> i32 {
        40 + self.bookmark_destinations().len() as i32 * 40
    }

    fn edit_folder(&mut self, id: Option<u32>) {
        let mut panel = BrowserPanel::new(PanelKind::EditFolder { id }, Action::Title);
        panel.folder = self.browser_panel.as_ref().map_or(0, |panel| panel.folder);
        if let Some(folder) = id.and_then(|id| self.profile.data().folder(id)) {
            panel.folder = folder.parent;
            panel.title.set(&folder.title);
        } else {
            panel.title.set("New folder");
        }
        panel.title.select_all();
        panel.return_panel = self.browser_panel.take().map(Box::new);
        self.begin_browser_panel(panel);
    }

    pub(super) fn save_folder(&mut self) {
        let Some(panel) = &self.browser_panel else {
            return;
        };
        let PanelKind::EditFolder { id } = panel.kind else {
            return;
        };
        let parent = panel.folder;
        let title = panel.title.text().trim().to_owned();
        let mut data = self.profile.data().clone();
        let result = if let Some(id) = id {
            let Some(folder) = data.folders.iter_mut().find(|folder| folder.id == id) else {
                return;
            };
            folder.title = title;
            let position = if folder.parent == parent {
                data.items(parent)
                    .iter()
                    .position(|item| *item == BookmarkItem::Folder(id))
                    .unwrap_or(0)
            } else {
                data.items(parent).len()
            };
            data.move_item(BookmarkItem::Folder(id), parent, position)
                .map(|()| id)
        } else {
            data.add_folder(parent, title)
        };
        let id = match result {
            Ok(id) => id,
            Err(error) => {
                self.profile_error(error);
                return;
            }
        };
        if let Err(error) = self.save_profile_editor(data) {
            self.profile_error(error);
            return;
        }
        self.show_bookmark_folder(parent);
        self.select_bookmark_item(Some(BookmarkItem::Folder(id)));
        self.ensure_bookmark_visible();
        self.status = "Bookmark folder saved.".into();
        self.failed = false;
    }

    fn choose_bookmark_destination(&mut self) {
        let Some(previous) = self.browser_panel.take() else {
            return;
        };
        let mut panel = BrowserPanel::new(
            PanelKind::FolderDestination,
            Action::DestinationRow(previous.folder),
        );
        panel.destination = previous.folder;
        panel.folder = previous.folder;
        panel.return_panel = Some(Box::new(previous));
        self.begin_browser_panel(panel);
        self.ensure_browser_focus_visible();
    }

    fn commit_bookmark_destination(&mut self, destination: u32) {
        let Some(previous) = self
            .browser_panel
            .as_ref()
            .and_then(|panel| panel.return_panel.as_ref())
        else {
            return;
        };
        if matches!(
            previous.kind,
            PanelKind::EditBookmark { .. } | PanelKind::EditFolder { .. }
        ) {
            if let Some(previous) = self
                .browser_panel
                .as_mut()
                .and_then(|panel| panel.return_panel.as_mut())
            {
                previous.folder = destination;
                previous.focus = Action::Destination;
            }
            self.leave_browser_panel();
            return;
        }
        let item = previous
            .selected_folder
            .map(BookmarkItem::Folder)
            .or_else(|| previous.selected.map(BookmarkItem::Link));
        let Some(item) = item else {
            return;
        };
        let mut data = self.profile.data().clone();
        let position = data.items(destination).len();
        if let Err(error) = data
            .move_item(item, destination, position)
            .and_then(|()| self.save_profile_editor(data))
        {
            self.profile_error(error);
            return;
        }
        self.leave_browser_panel();
        self.show_bookmark_folder(destination);
        self.select_bookmark_item(Some(item));
        self.ensure_bookmark_visible();
        self.status = "Bookmark moved.".into();
        self.failed = false;
    }

    fn reorder_bookmark(&mut self, down: bool) {
        let Some(item) = self.bookmark_selected_item() else {
            return;
        };
        let Some(parent) = self.profile.data().item_parent(item) else {
            return;
        };
        let rows = self.profile.data().items(parent);
        let Some(index) = rows.iter().position(|row| *row == item) else {
            return;
        };
        let target = if down {
            (index + 1).min(rows.len().saturating_sub(1))
        } else {
            index.saturating_sub(1)
        };
        if index == target {
            return;
        }
        let mut data = self.profile.data().clone();
        if let Err(error) = data
            .move_item(item, parent, target)
            .and_then(|()| self.save_profile_editor(data))
        {
            self.profile_error(error);
            return;
        }
        self.ensure_bookmark_visible();
        self.status = "Bookmark order saved.".into();
        self.failed = false;
    }

    fn confirm_folder_delete(&mut self, id: u32) {
        let mut panel = BrowserPanel::new(PanelKind::DeleteFolder { id }, Action::Cancel);
        panel.folder = self
            .profile
            .data()
            .folder(id)
            .map_or(0, |folder| folder.parent);
        panel.return_panel = self.browser_panel.take().map(Box::new);
        self.begin_browser_panel(panel);
    }

    fn delete_folder(&mut self) {
        let Some(panel) = &self.browser_panel else {
            return;
        };
        let PanelKind::DeleteFolder { id } = panel.kind else {
            return;
        };
        let parent = panel.folder;
        let mut data = self.profile.data().clone();
        let (links, folders) = data.remove_folder(id);
        if let Err(error) = self.save_profile_editor(data) {
            self.profile_error(error);
            return;
        }
        self.show_bookmark_folder(parent);
        self.status = format!("Removed {folders} folder(s) and {links} bookmark(s).");
        self.failed = false;
    }

    pub(super) fn bookmark_tree_activate(&mut self, action: Action) -> bool {
        let persisted = self.browser_panel.as_ref().is_some_and(|panel| matches!(panel.kind,
            PanelKind::Bookmarks | PanelKind::EditBookmark { .. } | PanelKind::EditFolder { .. }
            | PanelKind::FolderDestination | PanelKind::DeleteFolder { .. }));
        if persisted && matches!(action, Action::Open | Action::Edit | Action::Delete | Action::Row(_)
            | Action::FolderRow(_) | Action::NewFolder | Action::FolderUp | Action::Destination
            | Action::Move | Action::DestinationRow(_) | Action::SaveFolder
            | Action::ConfirmDeleteFolder | Action::MoveUp | Action::MoveDown)
            && !self.guard_profile_panel() { return true; }
        let manager = self
            .browser_panel
            .as_ref()
            .is_some_and(|panel| matches!(panel.kind, PanelKind::Bookmarks));
        match action {
            Action::FolderRow(id) if manager => {
                self.select_bookmark_item(Some(BookmarkItem::Folder(id)))
            }
            Action::Row(index) if manager => {
                self.select_bookmark_item(Some(BookmarkItem::Link(index)))
            }
            Action::Open
                if manager
                    && self
                        .browser_panel
                        .as_ref()
                        .and_then(|panel| panel.selected_folder)
                        .is_some() =>
            {
                self.show_bookmark_folder(
                    self.browser_panel
                        .as_ref()
                        .unwrap()
                        .selected_folder
                        .unwrap(),
                )
            }
            Action::Edit
                if manager
                    && self
                        .browser_panel
                        .as_ref()
                        .and_then(|panel| panel.selected_folder)
                        .is_some() =>
            {
                self.edit_folder(self.browser_panel.as_ref().unwrap().selected_folder)
            }
            Action::Delete
                if manager
                    && self
                        .browser_panel
                        .as_ref()
                        .and_then(|panel| panel.selected_folder)
                        .is_some() =>
            {
                self.confirm_folder_delete(
                    self.browser_panel
                        .as_ref()
                        .unwrap()
                        .selected_folder
                        .unwrap(),
                )
            }
            Action::NewFolder if manager => self.edit_folder(None),
            Action::FolderUp if manager => {
                let parent = self
                    .profile
                    .data()
                    .folder(self.browser_panel.as_ref().unwrap().folder)
                    .map_or(0, |folder| folder.parent);
                self.show_bookmark_folder(parent);
            }
            Action::SettingsBack if manager => {
                if self
                    .browser_panel
                    .as_ref()
                    .is_some_and(|panel| panel.return_panel.is_some())
                {
                    self.leave_browser_panel();
                } else {
                    self.show_settings();
                }
            }
            Action::Destination | Action::Move => self.choose_bookmark_destination(),
            Action::DestinationRow(id) => self.commit_bookmark_destination(id),
            Action::SaveFolder => self.save_folder(),
            Action::ConfirmDeleteFolder => self.delete_folder(),
            Action::MoveUp if manager => self.reorder_bookmark(false),
            Action::MoveDown if manager => self.reorder_bookmark(true),
            _ => return false,
        }
        true
    }

    pub(super) fn bookmark_tree_key(&mut self, key: u32) {
        if !self.browser_panel.as_ref().is_some_and(|panel| matches!(panel.kind, PanelKind::Bookmarks)) { return; }
        if !self.guard_profile_panel() { return; }
        if key == 0x08 || key == 0x25 {
            self.bookmark_tree_activate(Action::FolderUp);
            return;
        }
        if matches!(key, 0x0d | 0x27) {
            self.browser_activate(Action::Open);
            return;
        }
        if key == 0x2e {
            self.browser_activate(Action::Delete);
            return;
        }
        if key == 0x71 {
            self.browser_activate(Action::Edit);
            return;
        }
        let items = self.bookmark_tree_items();
        if items.is_empty() {
            return;
        }
        let index = self
            .bookmark_selected_item()
            .and_then(|item| items.iter().position(|row| *row == item))
            .unwrap_or(0);
        let rows = self.bookmark_rows();
        let next = match key {
            0x26 => index.saturating_sub(1),
            0x28 => (index + 1).min(items.len() - 1),
            0x24 => 0,
            0x23 => items.len() - 1,
            0x21 => index.saturating_sub(rows),
            0x22 => (index + rows).min(items.len() - 1),
            _ => return,
        };
        self.select_bookmark_item(Some(items[next]));
        self.ensure_bookmark_visible();
    }

    pub(super) fn paint_bookmark_tree(
        &self,
        canvas: &mut Canvas,
        body: Rect,
        panel: &BrowserPanel,
    ) {
        let colors = self.browser_palette();
        let y = body.y - panel.scroll;
        match panel.kind {
            PanelKind::Bookmarks => {
                ui::rounded_rect(canvas, body.tuple(), 8.0, colors.panel);
                let rows = self.bookmark_tree_items();
                if rows.is_empty() {
                    canvas.text(
                        body.x + 14,
                        body.y + 14,
                        "This folder is empty.",
                        2,
                        colors.foreground,
                    );
                    canvas.text(
                        body.x + 14,
                        body.y + 40,
                        "Add this page or create a folder below.",
                        1,
                        colors.muted,
                    );
                }
                for (index, item) in rows
                    .iter()
                    .enumerate()
                    .skip(panel.first)
                    .take(self.bookmark_rows() + 1)
                {
                    let top = body.y + (index - panel.first) as i32 * ROW;
                    let action = match item {
                        BookmarkItem::Link(index) => Action::Row(*index),
                        BookmarkItem::Folder(id) => Action::FolderRow(*id),
                    };
                    panel_list_row(
                        canvas,
                        body,
                        top,
                        self.bookmark_selected_item() == Some(*item),
                        panel.hover == Some(action),
                        colors,
                    );
                    ui::icon(
                        canvas,
                        if matches!(item, BookmarkItem::Folder(_)) {
                            Icon::Folder
                        } else {
                            Icon::Bookmark
                        },
                        body.x + 12,
                        top + 15,
                        24,
                        colors.foreground,
                        colors.accent,
                    );
                    canvas.text(
                        body.x + 48,
                        top + 8,
                        &fit_text(
                            self.profile.data().item_title(*item),
                            ((body.w - 72) / 12).max(0) as usize,
                        ),
                        2,
                        colors.foreground,
                    );
                    let detail = match item {
                        BookmarkItem::Link(index) => {
                            self.profile.data().bookmarks[*index].url.clone()
                        }
                        BookmarkItem::Folder(id) => {
                            format!("Folder / {} item(s)", self.profile.data().items(*id).len())
                        }
                    };
                    canvas.text(
                        body.x + 48,
                        top + 31,
                        &fit_text(&detail, ((body.w - 72) / 6).max(0) as usize),
                        1,
                        colors.muted,
                    );
                }
                if rows.len() > self.bookmark_rows() {
                    let thumb = (body.h * self.bookmark_rows() as i32 / rows.len() as i32)
                        .max(12)
                        .min(body.h);
                    let top = body.y
                        + (body.h - thumb) * panel.first as i32
                            / rows.len().saturating_sub(self.bookmark_rows()).max(1) as i32;
                    ui::rounded_rect(
                        canvas,
                        (body.x + body.w - 7, top, 4, thumb),
                        2.0,
                        colors.accent,
                    );
                }
            }
            PanelKind::EditFolder { .. } => {
                canvas.text(body.x, y + 7, "Folder name", 2, colors.foreground);
                canvas.text(
                    body.x,
                    y + 86,
                    &fit_text(
                        &self.profile.data().folder_path(panel.folder),
                        (body.w / 6).max(0) as usize,
                    ),
                    1,
                    colors.muted,
                );
            }
            PanelKind::FolderDestination => canvas.text(
                body.x,
                y + 8,
                "Choose the destination folder",
                2,
                colors.foreground,
            ),
            PanelKind::DeleteFolder { id } => {
                let name = self
                    .profile
                    .data()
                    .folder(id)
                    .map_or("Folder", |folder| folder.title.as_str());
                let mut copy = self.profile.data().clone();
                let (links, folders) = copy.remove_folder(id);
                canvas.text(
                    body.x,
                    y + 10,
                    &fit_text(&format!("Delete {name}?"), (body.w / 12).max(0) as usize),
                    2,
                    colors.foreground,
                );
                canvas.text(body.x,y+48,&fit_text(&format!("This removes {links} bookmark(s) and {folders} folder(s), including subfolders."),(body.w/6).max(0) as usize),1,colors.muted);
                canvas.text(
                    body.x,
                    y + 76,
                    "Choose Cancel to keep them.",
                    1,
                    colors.muted,
                );
            }
            _ => {}
        }
    }
}
