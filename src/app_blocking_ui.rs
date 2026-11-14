//! Local request-rule editor, staged inside the existing Settings draft.
use super::*;

impl App {
    pub(super) fn show_blocking_options(&mut self) {
        let Some(previous) = self.browser_panel.take() else {
            return;
        };
        if !matches!(previous.kind, PanelKind::Settings) {
            self.browser_panel = Some(previous);
            return;
        }
        let mut panel = BrowserPanel::new(PanelKind::BlockingRules, Action::RuleEntry);
        panel.blocking = previous.blocking.clone();
        panel.selected = (!panel.blocking.rules.is_empty()).then_some(0);
        panel.return_panel = Some(Box::new(previous));
        self.begin_browser_panel(panel);
    }

    pub(super) fn apply_blocking_options(&mut self) {
        let Some(panel) = &mut self.browser_panel else {
            return;
        };
        if let Err(error) = panel.blocking.normalize() {
            panel.error = error;
            return;
        }
        let config = panel.blocking.clone();
        let previous = panel.return_panel.take();
        if let Some(mut previous) = previous {
            previous.blocking = config;
            self.begin_browser_panel(*previous);
        } else {
            self.close_browser_panel();
        }
    }

    pub(super) fn add_blocking_rule(&mut self) {
        let Some(panel) = &mut self.browser_panel else {
            return;
        };
        if !matches!(panel.kind, PanelKind::BlockingRules) {
            return;
        }
        let rule = match crate::blocking::normalize_rule(&panel.address.text()) {
            Ok(rule) => rule,
            Err(error) => {
                panel.error = error;
                return;
            }
        };
        let mut next = panel.blocking.clone();
        next.rules.push(rule);
        if let Err(error) = next.normalize() {
            panel.error = error;
            return;
        }
        panel.blocking = next;
        panel.selected = Some(panel.blocking.rules.len() - 1);
        panel.address.set("");
        panel.error.clear();
        panel.focus = Action::RuleEntry;
        self.sync_blocking_panel();
    }

    pub(super) fn delete_blocking_rule(&mut self) {
        let Some(panel) = &mut self.browser_panel else {
            return;
        };
        if !matches!(panel.kind, PanelKind::BlockingRules) {
            return;
        }
        if let Some(index) = panel
            .selected
            .filter(|index| *index < panel.blocking.rules.len())
        {
            panel.blocking.rules.remove(index);
            panel.selected = (!panel.blocking.rules.is_empty())
                .then(|| index.min(panel.blocking.rules.len() - 1));
            panel.error.clear();
        }
        self.sync_blocking_panel();
    }

    pub(super) fn blocking_list_rect(&self) -> Rect {
        let body = self.browser_body();
        Rect {
            x: body.x,
            y: body.y + 220 - self.browser_panel.as_ref().map_or(0, |p| p.scroll),
            w: body.w,
            h: (body.h - 220).max(ROW),
        }
    }

    pub(super) fn blocking_rows(&self) -> usize {
        (self.blocking_list_rect().h.min(self.browser_body().h) / ROW).max(1) as usize
    }

    pub(super) fn sync_blocking_panel(&mut self) {
        let rows = self.blocking_rows();
        if let Some(panel) = &mut self.browser_panel {
            if !matches!(panel.kind, PanelKind::BlockingRules) {
                return;
            }
            let count = panel.blocking.rules.len();
            panel.selected = panel.selected.filter(|index| *index < count);
            if let Some(index) = panel.selected {
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

    pub(super) fn blocking_key(&mut self, key: u32) {
        let rows = self.blocking_rows();
        let Some(panel) = &mut self.browser_panel else {
            return;
        };
        let count = panel.blocking.rules.len();
        if count == 0 {
            return;
        }
        let index = panel.selected.unwrap_or(0);
        panel.selected = Some(match key {
            0x26 => index.saturating_sub(1),
            0x28 => (index + 1).min(count - 1),
            0x24 => 0,
            0x23 => count - 1,
            0x21 => index.saturating_sub(rows),
            0x22 => (index + rows).min(count - 1),
            _ => index,
        });
        if key == 0x2e {
            self.delete_blocking_rule();
        } else if key == 0x0d {
            let selected = panel.blocking.rules[panel.selected.unwrap()].clone();
            panel.address.set(&selected);
            panel.address.select_all();
            panel.focus = Action::RuleEntry;
            self.ensure_browser_focus_visible();
        } else {
            self.sync_blocking_panel();
        }
    }

    pub(super) fn blocking_controls(
        &self,
        body: Rect,
        footer: i32,
        panel: &BrowserPanel,
    ) -> Vec<Control> {
        let y = body.y - panel.scroll;
        let half = ((body.w - 8) / 2).max(1);
        vec![
            Control {
                action: Action::BuiltinBlocking,
                rect: Rect {
                    x: body.x,
                    y,
                    w: body.w,
                    h: 36,
                },
                label: "Use starter ad / analytics host rules",
                enabled: true,
            },
            Control {
                action: Action::RuleEntry,
                rect: Rect {
                    x: body.x,
                    y: y + 92,
                    w: body.w,
                    h: 34,
                },
                label: "",
                enabled: true,
            },
            Control {
                action: Action::RuleAdd,
                rect: Rect {
                    x: body.x,
                    y: y + 138,
                    w: half,
                    h: 32,
                },
                label: "Add rule",
                enabled: true,
            },
            Control {
                action: Action::RuleDelete,
                rect: Rect {
                    x: body.x + half + 8,
                    y: y + 138,
                    w: half,
                    h: 32,
                },
                label: "Remove selected",
                enabled: panel.selected.is_some(),
            },
            Control {
                action: Action::RuleList,
                rect: self.blocking_list_rect(),
                label: "",
                enabled: true,
            },
            Control {
                action: Action::ApplyBlocking,
                rect: Rect {
                    x: body.x,
                    y: footer,
                    w: half,
                    h: 32,
                },
                label: "Done",
                enabled: true,
            },
            Control {
                action: Action::Cancel,
                rect: Rect {
                    x: body.x + half + 8,
                    y: footer,
                    w: half,
                    h: 32,
                },
                label: "Cancel",
                enabled: true,
            },
        ]
    }

    pub(super) fn paint_blocking_rules(
        &self,
        canvas: &mut Canvas,
        body: Rect,
        panel: &BrowserPanel,
    ) {
        let palette = self.browser_palette();
        let y = body.y - panel.scroll;
        for (offset, text) in [
            (44, "doubleclick.net, googleadservices.com, googlesyndication.com, google-analytics.com"),
            (70, "Add a host, path:/prefix or url:https://host/prefix"),
            (182, "Prefix @@ to allow a matching request. Allow rules win over block rules."),
            (198, "Done returns to Settings. Apply there saves your changes."),
        ] {
            canvas.text(body.x, y + offset, &fit_text(text, (body.w / 6).max(0) as usize), 1, palette.muted);
        }
        let list = self.blocking_list_rect();
        ui::rounded_rect(canvas, list.tuple(), 8.0, palette.panel);
        if panel.focus == Action::RuleList {
            ui::rounded_border(canvas, list.tuple(), 8.0, 2.0, palette.accent);
        }
        if panel.blocking.rules.is_empty() {
            canvas.text(
                list.x + 12,
                list.y + 14,
                "No custom rules.",
                2,
                palette.foreground,
            );
            return;
        }
        for (index, rule) in panel
            .blocking
            .rules
            .iter()
            .enumerate()
            .skip(panel.first)
            .take(self.blocking_rows() + 1)
        {
            let top = list.y + (index - panel.first) as i32 * ROW;
            if top >= list.y + list.h {
                break;
            }
            let selected = panel.selected == Some(index);
            let hovered = panel.hover == Some(Action::RuleRow(index));
            panel_list_row(canvas, list, top, selected, hovered, palette);
            canvas.text(
                list.x + 12,
                top + 7,
                if rule.starts_with("@@") {
                    "Allow"
                } else {
                    "Block"
                },
                1,
                palette.accent,
            );
            canvas.text(
                list.x + 12,
                top + 25,
                &fit_text(rule, ((list.w - 24) / 12).max(0) as usize),
                2,
                palette.foreground,
            );
        }
    }
}
