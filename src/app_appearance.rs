//! User appearance preferences affect painting, not script-visible styles.
use super::*;

impl App {
    pub(super) fn browser_palette(&self) -> ui::Palette {
        ui::Palette::new(self.profile.data().settings.dark_mode)
    }

    pub fn title_bar_colors(&self) -> (u32, u32) {
        let colors = self.browser_palette();
        (colors.chrome, colors.chrome_text)
    }

    pub(super) fn remember_visit(&mut self) {
        if self.visited.len() >= 8192 {
            self.visited.clear();
        }
        self.visited
            .insert(visit_key(&self.location.display()).to_owned());
    }

    pub(super) fn rebuild_link_colors(&mut self) {
        self.link_paint.clear();
        if self.view != View::Page {
            return;
        }
        let settings = &self.profile.data().settings;
        if settings.link_color.is_none() && settings.visited_color.is_none() {
            return;
        }
        // Saved history is consulted only while recording is enabled. This
        // cache is browser-owned and is never reflected into the document,
        // selectors, getComputedStyle, or element geometry.
        let history: std::collections::HashSet<&str> =
            if self.browsing_history.data().policy.enabled {
                self.browsing_history
                    .data()
                    .entries
                    .iter()
                    .map(|entry| visit_key(&entry.url))
                    .collect()
            } else {
                std::collections::HashSet::new()
            };
        for (index, href) in self.page.link_targets().enumerate() {
            let location = navigation::resolve(href, &self.base_location).ok();
            let visited = location.as_ref().is_some_and(|location| {
                let display = location.display();
                let key = visit_key(&display);
                self.visited.contains(key) || history.contains(key)
            });
            if let Some(color) = if visited {
                settings.visited_color
            } else {
                settings.link_color
            } {
                self.link_paint.insert(index, color);
            }
        }
    }

    pub(super) fn page_background_color(&self, color: u32) -> u32 {
        if !self.profile.data().settings.dark_pages {
            return color;
        }
        if luminance(color) > 150 {
            // Lower light surfaces while retaining a small amount of their
            // original hue. Images and media pixels never pass through here.
            let r = 12 + ((color >> 16) & 255) / 20;
            let g = 18 + ((color >> 8) & 255) / 20;
            let b = 29 + (color & 255) / 20;
            (r << 16) | (g << 8) | b
        } else {
            color
        }
    }

    pub(super) fn page_text_color(&self, run: &layout::TextRun) -> u32 {
        if let Some(color) = run.link.and_then(|id| self.link_paint.get(&id)).copied() {
            return color;
        }
        let color = run.color;
        if !self.profile.data().settings.dark_pages || luminance(color) >= 165 {
            return color;
        }
        let r = 174 + ((color >> 16) & 255) / 4;
        let g = 180 + ((color >> 8) & 255) / 4;
        let b = 190 + (color & 255) / 4;
        (r << 16) | (g << 8) | b
    }
}

fn visit_key(address: &str) -> &str {
    address.split('#').next().unwrap_or(address)
}
fn luminance(color: u32) -> u32 {
    (((color >> 16) & 255) * 299 + ((color >> 8) & 255) * 587 + (color & 255) * 114) / 1000
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visited_and_unvisited_link_overrides_are_independent() {
        let mut app = App::new();
        app.document = html::parse("<style>a{color:#225588}</style><body><a href='https://example.com/one'>One</a> <a href='https://example.com/two'>Two</a></body>");
        app.forms = Forms::new(&app.document);
        app.visited.insert("https://example.com/one".into());
        for (unvisited, visited) in [
            (Some(0x00ddaa), None),
            (None, Some(0xbb88ff)),
            (Some(0x00ddaa), Some(0xbb88ff)),
            (None, None),
        ] {
            let mut data = app.profile.data().clone();
            data.settings.link_color = unvisited;
            data.settings.visited_color = visited;
            app.profile.save(data).unwrap();
            app.reflow();
            let one = app.page.runs.iter().find(|run| run.text == "One").unwrap();
            let two = app.page.runs.iter().find(|run| run.text == "Two").unwrap();
            assert_eq!(app.page_text_color(one), visited.unwrap_or(0x225588));
            assert_eq!(app.page_text_color(two), unvisited.unwrap_or(0x225588));
            assert_eq!(one.color, 0x225588);
            assert_eq!(two.color, 0x225588);
        }
    }

    #[test]
    fn link_preferences_change_paint_only_and_dark_surfaces_keep_image_pixels() {
        let mut app = App::new();
        app.document=html::parse("<style>a{color:#225588}</style><body><a href='https://example.com/one'>One</a> <a href='https://example.com/two'>Two</a></body>");
        app.forms = Forms::new(&app.document);
        let mut data = app.profile.data().clone();
        data.settings.link_color = Some(0x00ddaa);
        data.settings.visited_color = Some(0xbb88ff);
        data.settings.dark_mode = true;
        data.settings.dark_pages = true;
        app.profile.save(data).unwrap();
        app.visited.insert("https://example.com/one".into());
        app.reflow();
        let one = app.page.runs.iter().find(|run| run.text == "One").unwrap();
        let two = app.page.runs.iter().find(|run| run.text == "Two").unwrap();
        assert_eq!(one.color, 0x225588);
        assert_eq!(two.color, 0x225588);
        assert_eq!(app.page_text_color(one), 0xbb88ff);
        assert_eq!(app.page_text_color(two), 0x00ddaa);
        assert!(luminance(app.page_background_color(0xffffff)) < 50);
        assert_eq!(app.page_background_color(0x0b153c), 0x0b153c);
        app.visited.clear();
        app.rebuild_link_colors();
        assert!(app.link_paint.values().all(|color| *color == 0x00ddaa));
        assert!(!app.browsing_history.data().policy.enabled);
        assert!(app.browsing_history.data().entries.is_empty());
    }
}
