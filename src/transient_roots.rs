//! ID-only registrations for suspended interpreter state. No collection occurs.
use super::{Fault, Runtime, MAX_ARGUMENTS, MAX_CALL_DEPTH, MAX_EVAL_DEPTH,
    MAX_EVALUATION_ARGUMENT_SLOTS, MAX_EVALUATION_FRAME_SLOTS};
use super::function_reachability::Edge;

const MAX_ROOT_REFERENCES: usize = 4 * MAX_EVALUATION_FRAME_SLOTS
    + MAX_EVALUATION_ARGUMENT_SLOTS + MAX_CALL_DEPTH * (MAX_ARGUMENTS + 4)
    + 4 * MAX_EVAL_DEPTH;

#[derive(Default)]
pub(super) struct State {
    pub(super) roots: Vec<Edge>,
    pub(super) evaluation_frames: usize,
    pub(super) frame_slots: usize,
    pub(super) argument_slots: usize,
    pub(super) inhibited: usize,
    pub(super) snapshots: usize,
    pub(super) peak: usize,
}

#[derive(Clone, Copy)]
pub(super) struct Marker {
    length: usize,
    evaluation_frames: usize,
    frame_slots: usize,
    argument_slots: usize,
    inhibited: usize,
}

impl Runtime {
    pub(super) fn transient_marker(&self) -> Marker {
        let state = &self.transient_roots;
        Marker { length: state.roots.len(), evaluation_frames: state.evaluation_frames,
            frame_slots: state.frame_slots, argument_slots: state.argument_slots, inhibited: state.inhibited }
    }

    pub(super) fn inhibit_transient_collection(&mut self) -> Marker {
        let marker = self.transient_marker();
        self.transient_roots.inhibited += 1;
        marker
    }

    pub(super) fn restore_transient_marker(&mut self, marker: Marker) {
        let state = &mut self.transient_roots;
        state.roots.truncate(marker.length);
        state.evaluation_frames = marker.evaluation_frames;
        state.frame_slots = marker.frame_slots;
        state.argument_slots = marker.argument_slots;
        state.inhibited = marker.inhibited;
    }

    pub(super) fn push_transient_root(&mut self, edge: Edge) -> Result<(), Fault> {
        let roots = &mut self.transient_roots.roots;
        if roots.len() >= MAX_ROOT_REFERENCES { return Err(Fault::new("Transient root-reference limit reached.")); }
        if roots.len() == roots.capacity() {
            let additional = roots.capacity().max(8).min(MAX_ROOT_REFERENCES - roots.len());
            roots.try_reserve_exact(additional).map_err(|_| Fault::new("Unable to allocate bounded transient-root scratch."))?;
        }
        roots.push(edge);
        self.transient_roots.peak = self.transient_roots.peak.max(roots.len());
        Ok(())
    }

    pub(super) fn transient_registration_is_empty(&self) -> bool {
        let state = &self.transient_roots;
        state.roots.is_empty() && state.evaluation_frames == 0 && state.frame_slots == 0
            && state.argument_slots == 0 && state.inhibited == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_root_bound_and_nested_markers_restore_without_refunding_work() {
        let mut runtime = Runtime::new();
        let before = (runtime.job_usage(), runtime.job_deadline, runtime.value_bytes);
        let outer = runtime.transient_marker();
        runtime.push_transient_root(Edge::Environment(0)).unwrap();
        let inner = runtime.inhibit_transient_collection();
        runtime.push_transient_root(Edge::Function(0)).unwrap();
        runtime.restore_transient_marker(inner);
        assert_eq!(runtime.transient_roots.roots.len(), 1);
        assert_eq!(runtime.transient_roots.inhibited, 0);
        for _ in 1..MAX_ROOT_REFERENCES { runtime.push_transient_root(Edge::Environment(0)).unwrap(); }
        assert!(runtime.push_transient_root(Edge::Environment(0)).unwrap_err().message.contains("root-reference limit"));
        runtime.restore_transient_marker(outer);
        assert!(runtime.transient_registration_is_empty());
        assert_eq!((runtime.job_usage(), runtime.job_deadline, runtime.value_bytes), before);
    }
}
