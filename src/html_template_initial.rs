//! Initial template state used only by native P/BR end-tag recovery.
//! Later template insertion modes retain the existing bounded parser handling.
use super::*;

struct Frame {
    node: usize,
    initial: bool,
}

#[derive(Default)]
pub(super) struct InitialTemplates {
    frames: Vec<Frame>,
}

impl InitialTemplates {
    pub(super) fn new(context_template: bool) -> Result<Self, String> {
        let mut state = Self::default();
        if context_template { state.opened(0, &[0])?; }
        Ok(state)
    }

    fn retain_open(&mut self, stack: &[usize]) {
        // Both lists are bounded by MAX_DEPTH. Identity survives ordinary
        // stack movement; removed templates cannot leak state into siblings.
        self.frames.retain(|frame| stack.contains(&frame.node));
    }

    pub(super) fn observe_original_start(&mut self, document: &Document, stack: &[usize], tag: &Tag) {
        self.retain_open(stack);
        let current = *stack.last().unwrap();
        if tag.closing || current != 0 && document.nodes[current].namespace != Namespace::Html {
            return;
        }
        // Observe before structure/table dispatch can consume an original
        // start without inserting a node. Raw metadata closing tokens never
        // enter this path. A nested template retains its parent's state.
        if !matches!(tag.name.as_str(), "base" | "basefont" | "bgsound" | "link" | "meta"
            | "noframes" | "script" | "style" | "template" | "title") {
            if let Some(frame) = self.frames.last_mut() { frame.initial = false; }
        }
    }

    pub(super) fn is_initial(&mut self, stack: &[usize]) -> bool {
        self.retain_open(stack);
        self.frames.last().is_some_and(|frame| frame.initial)
    }

    pub(super) fn opened(&mut self, node: usize, stack: &[usize]) -> Result<(), String> {
        self.retain_open(stack);
        if self.frames.len() >= MAX_DEPTH {
            return Err("HTML template recovery state limit reached.".into());
        }
        self.frames.try_reserve(1).map_err(|_| "Cannot reserve HTML template recovery state.")?;
        self.frames.push(Frame { node, initial: true });
        Ok(())
    }
}
