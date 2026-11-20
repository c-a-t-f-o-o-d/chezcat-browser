//! Synchronize displayed App metadata after already committed same-document history changes.
use super::*;
impl App {
    pub(super) fn apply_history_replacement(&mut self) -> bool {
        let Some(page) = &mut self.scripting else {
            return false;
        };
        let history = self.history.borrow();
        let Some(entry) = history
            .current()
            .filter(|entry| entry.document == self.script_epoch)
        else {
            return false;
        };
        if !page.take_history_change() {
            return false;
        }
        let location = entry.location.clone();
        let index = history.index();
        drop(history);
        let old = self.location.display();
        self.location = location;
        self.history_index = index;
        if self.base_is_document_url {
            self.base_location = self.location.clone();
        }
        if let Some(pending) = self.pending.as_mut().filter(|pending| pending.committed) {
            pending.location = self.location.clone();
        }
        if self.focus != Focus::Address || self.address.text() == old {
            self.address.set(&self.location.display());
        }
        self.inspector.invalidate();
        true
    }
}
