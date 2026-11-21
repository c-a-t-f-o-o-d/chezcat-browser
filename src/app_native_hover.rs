//! Native pointer state from the displayed renderer, never synthetic DOM events.
use super::*;
impl App {
    fn native_hover_target(&self) -> Option<usize> {
        let (x, y) = self.hover_point?;
        if self.view != View::Page
            || self.alternate.is_some()
            || self.menu
            || self.chrome.pressed.is_some()
            || self.browser_panel.is_some()
            || self.select_popup.is_some()
            || self.downloads_open()
            || self.bookmark_popup_open()
            || self.bookmark_drag_active()
            || x < 0
            || x >= self.width - 12
            || y < self.content_top()
            || y >= self.content_bottom()
        {
            return None;
        }
        self.page
            .element_node_at(x, y - self.content_top() + self.scroll)
            .or(self.page.root_hit_node)
    }
    fn set_native_hover(&mut self, next: Option<usize>) -> bool {
        if self.document.native_hover.get() == next {
            return false;
        }
        self.document.native_hover.set(next);
        self.invalidate_script_geometry();
        if !self.page.has_hover_styles {
            return false;
        }
        let previous = self.hover_reflow_active;
        self.hover_reflow_active = true;
        self.reflow_preserving_drag();
        self.hover_reflow_active = previous;
        true
    }
    pub(super) fn refresh_native_hover(&mut self) -> bool {
        if self.hover_reflow_active {
            return false;
        }
        let next = self.native_hover_target();
        self.set_native_hover(next)
    }
    pub(super) fn clear_native_hover(&mut self) -> bool {
        self.hover_point = None;
        self.set_native_hover(None)
    }
    pub fn pointer_move(&mut self, x: i32, y: i32, dragging: bool) -> bool {
        self.hover_point = Some((x, y));
        let changed = self.refresh_native_hover();
        self.pointer_move_impl(x, y, dragging) || changed
    }
    pub fn pointer_down(&mut self, x: i32, y: i32, shift: bool) -> PlatformAction {
        self.hover_point = Some((x, y));
        self.refresh_native_hover();
        self.pointer_down_impl(x, y, shift)
    }
    pub fn pointer_up(&mut self, x: i32, y: i32) -> PlatformAction {
        self.hover_point = Some((x, y));
        self.refresh_native_hover();
        self.pointer_up_impl(x, y)
    }
}
