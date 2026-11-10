//! Bounded single-line editing for the address and find fields.
#[derive(Default)]
pub struct Editor {
    pub chars: Vec<char>,
    pub cursor: usize,
    pub anchor: usize,
    pub offset: usize,
}

impl Editor {
    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }
    pub fn set(&mut self, text: &str) {
        self.chars = text
            .chars()
            .filter(|c| !c.is_control())
            .take(32_768)
            .collect();
        self.cursor = self.chars.len();
        self.anchor = self.cursor;
        self.offset = 0;
    }
    pub fn range(&self) -> std::ops::Range<usize> {
        self.cursor.min(self.anchor)..self.cursor.max(self.anchor)
    }
    pub fn selected(&self) -> String {
        self.chars[self.range()].iter().collect()
    }
    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.chars.len();
    }
    pub fn delete_selection(&mut self) -> bool {
        let range = self.range();
        if range.is_empty() {
            return false;
        }
        self.cursor = range.start;
        self.anchor = self.cursor;
        self.chars.drain(range);
        true
    }
    pub fn insert(&mut self, text: &str, limit: usize) {
        self.delete_selection();
        let incoming: Vec<char> = text
            .chars()
            .filter(|c| !c.is_control())
            .take(limit.saturating_sub(self.chars.len()))
            .collect();
        let count = incoming.len();
        self.chars.splice(self.cursor..self.cursor, incoming);
        self.cursor += count;
        self.anchor = self.cursor;
    }
    pub fn backspace(&mut self) {
        if !self.delete_selection() && self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
            self.anchor = self.cursor;
        }
    }
    pub fn delete(&mut self) {
        if !self.delete_selection() && self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        }
    }
    pub fn move_to(&mut self, position: usize, extend: bool) {
        self.cursor = position.min(self.chars.len());
        if !extend {
            self.anchor = self.cursor;
        }
    }
    pub fn left(&mut self, extend: bool) {
        let n = if !extend && !self.range().is_empty() {
            self.range().start
        } else {
            self.cursor.saturating_sub(1)
        };
        self.move_to(n, extend);
    }
    pub fn right(&mut self, extend: bool) {
        let n = if !extend && !self.range().is_empty() {
            self.range().end
        } else {
            self.cursor + 1
        };
        self.move_to(n, extend);
    }
    pub fn ensure_visible(&mut self, cells: usize) {
        let cells = cells.max(1);
        // Replacing a selected, scrolled URL can leave the old offset beyond
        // the new text. Clamp to a full viewport before following the caret,
        // otherwise the first typed character remains hidden at offset 1.
        self.offset = self.offset.min(self.chars.len().saturating_sub(cells));
        if self.cursor < self.offset {
            self.offset = self.cursor;
        }
        // The caret after the last visible character fits at the right edge.
        if self.cursor > self.offset + cells {
            self.offset = self.cursor - cells;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_selection_and_limits() {
        let mut e = Editor::default();
        e.set("a🦀z");
        e.move_to(2, false);
        e.left(true);
        assert_eq!(e.selected(), "🦀");
        e.insert("é\nxyz", 5);
        assert_eq!(e.text(), "aéxyz");
        e.select_all();
        e.backspace();
        assert_eq!(e.text(), "");
    }

    #[test]
    fn replacing_scrolled_text_keeps_every_typed_character_visible() {
        let mut e = Editor::default();
        e.set("https://example.test/a/long/address?search=previous");
        e.select_all();
        e.ensure_visible(16);
        assert!(e.offset > 0);
        let mut expected = String::new();
        for ch in "google.com".chars() {
            expected.push(ch);
            e.insert(&ch.to_string(), 32_768);
            e.ensure_visible(16);
            assert_eq!(e.text(), expected);
            assert_eq!(
                e.offset, 0,
                "short replacement must start with its first character"
            );
        }
    }

    #[test]
    fn viewport_fits_the_end_caret_and_clamps_after_deletion_or_widening() {
        let mut e = Editor::default();
        e.set("abcdé🦀");
        e.ensure_visible(6);
        assert_eq!(e.offset, 0, "an exact fit keeps its first character");
        e.ensure_visible(4);
        assert_eq!(e.offset, 2);
        e.backspace();
        e.ensure_visible(4);
        assert_eq!(e.offset, 1);
        e.backspace();
        e.ensure_visible(4);
        assert_eq!(e.offset, 0);
        e.set("0123456789");
        e.ensure_visible(4);
        assert_eq!(e.offset, 6);
        e.move_to(3, false);
        e.ensure_visible(4);
        assert_eq!(e.offset, 3);
        e.move_to(10, false);
        e.ensure_visible(4);
        assert_eq!(e.offset, 6);
        e.ensure_visible(10);
        assert_eq!(e.offset, 0, "a wider field reveals the whole value again");
        e.select_all();
        e.delete();
        e.ensure_visible(0);
        assert_eq!(e.offset, 0);
    }
}
