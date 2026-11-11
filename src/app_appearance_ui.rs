//! Appearance and download-directory preferences use the existing staged
//! Settings editor. Apply commits the complete profile atomically.
use super::*;

impl App {
    pub fn show_download_settings(&mut self) {
        if !self
            .browser_panel
            .as_ref()
            .is_some_and(|panel| matches!(panel.kind, PanelKind::Settings))
        {
            self.show_settings();
        }
        if let Some(panel) = &mut self.browser_panel {
            panel.focus = Action::DownloadFolder;
            panel.scroll = 942;
        }
        self.sync_browser_panel();
        self.ensure_browser_focus_visible();
    }

    pub fn show_appearance_settings(&mut self) {
        if !self
            .browser_panel
            .as_ref()
            .is_some_and(|p| matches!(p.kind, PanelKind::Settings))
        {
            self.show_settings();
        }
        if let Some(panel) = &mut self.browser_panel {
            panel.focus = Action::DarkMode;
            panel.scroll = 503;
        }
        self.sync_browser_panel();
        self.ensure_browser_focus_visible();
    }

    pub(super) fn appearance_controls(&self, body: Rect, panel: &BrowserPanel) -> Vec<Control> {
        let mut controls = Vec::new();
        for (action, offset, label) in [
            (Action::DarkMode, 540, "Dark browser theme"),
            (
                Action::DarkPages,
                588,
                "Darken webpage backgrounds and text",
            ),
            (
                Action::BookmarkBar,
                636,
                "Show bookmarks bar below the address bar",
            ),
            (Action::LinkColor, 720, ""),
            (Action::VisitedColor, 800, ""),
            (Action::ResetColors, 848, "Use webpage link colors"),
            (
                Action::BookmarksOptions,
                896,
                "Manage bookmarks and folders   Ctrl+B",
            ),
            (Action::DownloadFolder, 1010, ""),
            (
                Action::DefaultDownloads,
                1054,
                "Use the default Downloads folder",
            ),
        ] {
            controls.push(Control {
                action,
                rect: Rect {
                    x: body.x,
                    y: body.y + offset - panel.scroll,
                    w: body.w,
                    h: 34,
                },
                label,
                enabled: true,
            });
        }
        if self.width >= 620 {
            for (index, (action, label)) in [
                (Action::AppearanceOptions, "Appearance"),
                (Action::DownloadOptions, "Downloads"),
            ]
            .into_iter()
            .enumerate()
            {
                controls.push(Control {
                    action,
                    rect: Rect {
                        x: self.width - 285 + index as i32 * 108,
                        y: self.chrome_bottom() + 13,
                        w: 100,
                        h: 30,
                    },
                    label,
                    enabled: true,
                });
            }
        }
        controls
    }

    pub(super) fn appearance_activate(&mut self, action: Action) -> bool {
        match action {
            Action::DarkMode => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.dark_mode = !panel.dark_mode;
                }
            }
            Action::DarkPages => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.dark_pages = !panel.dark_pages;
                }
            }
            Action::BookmarkBar => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.bookmark_bar = !panel.bookmark_bar;
                }
            }
            Action::ResetColors => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.link_color.set("");
                    panel.visited_color.set("");
                }
            }
            Action::DefaultDownloads => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.download_folder.set("");
                }
            }
            Action::BookmarksOptions => {
                let previous = self.browser_panel.take();
                self.show_bookmarks();
                if let Some(panel) = &mut self.browser_panel {
                    panel.return_panel = previous.map(Box::new);
                }
            }
            Action::AppearanceOptions => self.show_appearance_settings(),
            Action::DownloadOptions => {
                if let Some(panel) = &mut self.browser_panel {
                    panel.focus = Action::DownloadFolder;
                    panel.scroll = 942;
                }
                self.sync_browser_panel();
                self.ensure_browser_focus_visible();
            }
            _ => return false,
        }
        true
    }

    pub(super) fn paint_appearance_settings(
        &self,
        canvas: &mut Canvas,
        body: Rect,
        panel: &BrowserPanel,
    ) {
        let palette = self.browser_palette();
        let y = body.y - panel.scroll;
        canvas.fill_rect(body.x, y + 503, body.w, 1, palette.border);
        canvas.text(body.x, y + 514, "Appearance", 2, palette.foreground);
        canvas.text(
            body.x,
            y + 697,
            "Unvisited link color (#RRGGBB, blank uses page color)",
            1,
            palette.muted,
        );
        canvas.text(
            body.x,
            y + 777,
            "Visited link color (#RRGGBB, blank uses page color)",
            1,
            palette.muted,
        );
        for (value, offset) in [(&panel.link_color, 760), (&panel.visited_color, 840)] {
            if let Ok(Some(color)) = crate::profile::parse_color(&value.text()) {
                canvas.fill_rect(body.x, y + offset, 64, 3, color);
            }
        }
        canvas.fill_rect(body.x, y + 942, body.w, 1, palette.border);
        canvas.text(body.x, y + 958, "Downloads", 2, palette.foreground);
        canvas.text(
            body.x,
            y + 986,
            "Folder (absolute path; blank uses your Downloads folder)",
            1,
            palette.muted,
        );
        canvas.text(
            body.x,
            y + 1102,
            "Dark page colors are optional; image pixels keep their colors.",
            1,
            palette.muted,
        );
        canvas.text(
            body.x,
            y + 1118,
            "Link color changes are visual only. Apply saves all settings.",
            1,
            palette.muted,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_settings_shortcut_preserves_the_current_settings_draft() {
        let mut app = App::new();
        app.show_settings();
        let committed = app.profile.data().clone();
        let panel = app.browser_panel.as_mut().unwrap();
        panel.dark_mode = true;
        panel.link_color.set("#010203");
        panel.zoom.set("125");
        app.show_downloads();
        // About:home has no downloadable file. Focus starts at Folder; Tab
        // selects the popup's Settings control, and Enter follows that route.
        app.key(0x09, false, false, false);
        app.key(0x0d, false, false, false);
        assert!(!app.downloads_open());
        let panel = app.browser_panel.as_ref().unwrap();
        assert_eq!(panel.focus, Action::DownloadFolder);
        assert!(panel.dark_mode);
        assert_eq!(panel.link_color.text(), "#010203");
        assert_eq!(panel.zoom.text(), "125");
        assert_eq!(*app.profile.data(), committed);
        app.key(0x1b, false, false, false);
        assert!(app.browser_panel.is_none());
        assert_eq!(*app.profile.data(), committed);
    }
}
