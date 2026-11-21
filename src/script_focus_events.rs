//! Trusted focus changes from the displayed native form controls.
use super::*;
impl Page {
    pub(crate) fn take_native_focus_loss(&mut self)->bool{std::mem::take(&mut self.state.dom.native_focus_removed)}

    pub(crate) fn focus_transition(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        previous: Option<usize>,
        next: Option<usize>,
    ) -> Outcome {
        let before = self.state.errors;
        self.state.dom.native_focus_removed=false;
        if doc.native_focus.get()!=next{doc.native_focus.set(next);self.state.layout_changed();}
        if previous == next || !self.state.enabled || !doc.scripting_enabled {
            return Outcome::default();
        }
        // Only the native host enters here; synthetic FocusEvent dispatch never
        // requests native focus, changes an editor or grants activation.
        for (node, related, name, bubbles) in [
            (previous, next, "blur", false),
            (previous, next, "focusout", true),
            (next, previous, "focus", false),
            (next, previous, "focusin", true),
        ] {
            let Some(node) = node else { continue };
            if let Err(error) = self.state.rebuild(doc) {
                self.state.error("Focus transition", &error);
                break;
            }
            if !self.state.tree.active.get(node).copied().unwrap_or(false) {
                continue;
            }
            let related = related
                .filter(|&node| self.state.tree.active.get(node).copied().unwrap_or(false))
                .map(node_handle);
            let (_, complete) = self.dispatch_composed_status(
                doc,
                forms,
                node_handle(node),
                name,
                false,
                bubbles,
                true,
                None,
                Some(related),
            );
            if !complete {
                break;
            }
        }
        Outcome {
            layout_changed: std::mem::take(&mut self.state.changed),
            errors: self.state.errors.saturating_sub(before),
            ..Outcome::default()
        }
    }
}
