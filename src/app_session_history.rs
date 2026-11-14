//! Apply validated same-document History commands to actual App session entries.
use super::*;

impl App {
    pub(super) fn apply_history_replacement(&mut self) -> bool {
        // Startup scripts run before finish_navigation publishes the new entry.
        // Leave their command queued until that entry belongs to this document.
        if self.history.get(self.history_index).is_none_or(|entry| entry.document != self.script_epoch) { return false; }
        let Some(replacement) = self.scripting.as_mut().and_then(|page| page.take_history_replacement()) else { return false; };
        let old = self.location.display();
        self.location = replacement.location;
        let entry = &mut self.history[self.history_index];
        entry.location = self.location.clone();
        entry.data = replacement.data;
        if self.base_is_document_url { self.base_location = self.location.clone(); }
        if let Some(pending) = self.pending.as_mut().filter(|pending| pending.committed) { pending.location = self.location.clone(); }
        if self.focus != Focus::Address || self.address.text() == old { self.address.set(&self.location.display()); }
        self.inspector.invalidate();
        true
    }
}
