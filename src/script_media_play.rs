//! One authored player's genuine pending play Promises and captured task lists.
//! Runtime alone creates and settles tokens; this registry only preserves order.
use crate::javascript::NativePromise;
use crate::media_tasks::{Control, Scope, Shared, Task};
use std::collections::VecDeque;
use std::sync::{atomic::Ordering, Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Rejection { Abort, NotSupported }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BatchKind { Playing, Resolve, Reject(Rejection) }

impl BatchKind {
    fn rejection(self) -> Option<Rejection> {
        match self { Self::Playing | Self::Resolve => None, Self::Reject(reason) => Some(reason) }
    }
    fn task(self, id: u64, control: Option<Control>) -> Task {
        match self {
            Self::Playing => Task::Playing(id),
            Self::Resolve => Task::ResolvePlay(id),
            Self::Reject(Rejection::Abort) if matches!(control, Some(Control::Pause { .. })) =>
                Task::PauseReject(id),
            Self::Reject(_) => Task::RejectPlay(id),
        }
    }
}

#[derive(Clone)]
pub(super) struct Effect {
    pub token: NativePromise,
    pub rejection: Option<Rejection>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase { Queued, Started }

struct Batch {
    id: u64,
    scope: Scope,
    kind: BatchKind,
    tokens: Vec<NativePromise>,
    phase: Phase,
}

#[derive(Default)]
pub(super) struct State {
    pending_scope: Option<Scope>,
    pending: Vec<NativePromise>,
    batches: VecDeque<Batch>,
    effects: VecDeque<Effect>,
    outstanding: usize,
    next: u64,
}

fn same_scope(left: &Scope, right: &Scope) -> bool {
    left.node == right.node && Arc::ptr_eq(&left.retired, &right.retired)
}

impl State {
    /// Preflight storage before creating a native Promise or changing playback.
    /// The effects allocation covers every admitted token, wherever it resides,
    /// so later task completion and synchronous load-reset moves cannot allocate.
    pub(super) fn reserve_pending(&mut self, limit: usize) -> Result<(), String> {
        let total = self.outstanding.checked_add(1).ok_or("Pending play count exhausted.")?;
        if total > limit { return Err("Pending play Promise allowance reached.".into()); }
        self.next.checked_add(1).ok_or("Play Promise task identity exhausted.")?;
        self.pending.try_reserve(1).map_err(|_| "Could not retain the pending play Promise.")?;
        self.batches.try_reserve(1).map_err(|_|"Could not reserve play Promise task metadata.")?;
        let additional = total.checked_sub(self.effects.len())
            .ok_or("Invalid pending play effect accounting.")?;
        self.effects.try_reserve(additional).map_err(|_| "Could not retain play Promise settlement storage.")?;
        Ok(())
    }

    pub(super) fn push_pending(
        &mut self, scope: Scope, token: NativePromise, limit: usize,
    ) -> Result<(), String> {
        if scope.retired.load(Ordering::Relaxed) { return Err("Play source is retired.".into()); }
        if !self.same_source(&scope) { return Err("Pending play Promises belong to another source.".into()); }
        self.reserve_pending(limit)?;
        if self.pending_scope.is_none() { self.pending_scope = Some(scope); }
        self.pending.push(token);
        self.outstanding += 1; // checked by reserve_pending before any mutation
        Ok(())
    }

    /// Roll back only the most recently admitted, still-element-pending call.
    /// Captured task lists and already deferred outcomes are never affected.
    pub(super) fn pop_pending_last(&mut self) -> Option<NativePromise> {
        let token = self.pending.pop()?;
        self.outstanding -= 1;
        if self.pending.is_empty() { self.pending_scope = None; }
        Some(token)
    }

    pub(super) fn pending_count(&self) -> usize { self.pending.len() }
    pub(super) fn retained_count(&self)->usize {self.outstanding}

    /// Whether a new pending Promise may join this element-pending list.
    pub(super) fn same_source(&self, scope: &Scope) -> bool {
        self.pending_scope.as_ref().map_or(true, |owner| same_scope(owner, scope))
    }

    pub(super) fn pending_for_source(&self, scope: &Scope) -> usize {
        self.pending_scope.as_ref().filter(|owner| same_scope(owner, scope))
            .map_or(0, |_| self.pending.len())
    }

    /// Queue the real event/control task before taking its Promise list. Failure
    /// leaves all tokens pending; None means no task or control was enqueued.
    pub(super) fn capture_pending(
        &mut self, scope: &Scope, kind: BatchKind, control: Option<Control>,
        tasks: &Shared, limit: usize,
    ) -> Result<Option<u64>, String> {
        self.capture_pending_with_prefix(scope,kind,control,None,tasks,limit)
    }

    pub(super) fn capture_pending_with_prefix(
        &mut self,scope:&Scope,kind:BatchKind,control:Option<Control>,prefix:Option<Task>,
        tasks:&Shared,limit:usize,
    )->Result<Option<u64>,String> {
        if !self.same_source(scope) { return Err("Pending play Promises belong to another source.".into()); }
        if self.pending_for_source(scope) == 0 { return Ok(None); }
        if scope.retired.load(Ordering::Relaxed) { return Err("Play source is retired.".into()); }
        if self.batches.len() >= limit { return Err("Play Promise task allowance reached.".into()); }
        let id = self.next.checked_add(1).ok_or("Play Promise task identity exhausted.")?;
        self.batches.try_reserve(1).map_err(|_| "Could not retain the play Promise task.")?;
        let mut queue = tasks.lock().map_err(|_| "Media task queue is unavailable.")?;
        queue.set_limit(limit);
        if let Some(prefix)=prefix {
            queue.enqueue_many(scope.clone(),&[(control,Some(prefix)),(None,Some(kind.task(id,None)))])?;
        } else {queue.enqueue(scope.clone(), control, Some(kind.task(id, control)))?;}
        let tokens = std::mem::take(&mut self.pending);
        let owner = self.pending_scope.take().expect("nonempty pending play list has its source");
        self.batches.push_back(Batch { id, scope: owner, kind, tokens, phase: Phase::Queued });
        self.next = id;
        drop(queue); // no consumer can take the task before its batch is visible
        Ok(Some(id))
    }

    pub(super) fn start_batch(&mut self, id: u64, scope: &Scope) -> bool {
        if scope.retired.load(Ordering::Relaxed) { return false; }
        let Some(batch) = self.batches.iter_mut().find(|batch| batch.id == id) else { return false; };
        if batch.phase != Phase::Queued || !same_scope(&batch.scope, scope) { return false; }
        batch.phase = Phase::Started;
        true
    }

    /// A started task retains ownership through reentrant load/source changes.
    /// Even a fatal callback finishes into deferred native cleanup, not callbacks.
    pub(super) fn finish_batch(&mut self, id: u64) -> bool {
        let Some(index) = self.batches.iter().position(|batch| batch.id == id && batch.phase == Phase::Started)
            else { return false; };
        let batch = self.batches.remove(index).expect("located play Promise task");
        let rejection = batch.kind.rejection();
        debug_assert!(batch.tokens.len() <= self.effects.capacity() - self.effects.len());
        for token in batch.tokens { self.effects.push_back(Effect { token, rejection }); }
        true
    }

    /// No allocation follows this preflight. The bound is over all outstanding
    /// tokens, including old-source started tasks and already deferred effects.
    pub(super) fn prepare_source_reset(&self, scope: &Scope) -> Result<(), String> {
        let mut needed = self.pending_for_source(scope);
        for batch in &self.batches {
            if batch.phase == Phase::Queued && same_scope(&batch.scope, scope) {
                needed = needed.checked_add(batch.tokens.len()).ok_or("Play reset count exhausted.")?;
            }
        }
        let total = self.effects.len().checked_add(needed).ok_or("Play reset effect count exhausted.")?;
        if total > self.outstanding || self.effects.capacity() < self.outstanding {
            return Err("Play reset settlement storage is unavailable.".into());
        }
        Ok(())
    }

    /// Preserve queued batches' original outcomes in their queue order, then
    /// abort still-element-pending calls. A STARTED task's tail stays its own.
    /// The caller retires/cancels old media tasks only after recording outcomes.
    pub(super) fn reset_source(&mut self, scope: &Scope) {
        let effects = &mut self.effects;
        self.batches.retain_mut(|batch| {
            if batch.phase != Phase::Queued || !same_scope(&batch.scope, scope) { return true; }
            let rejection = batch.kind.rejection();
            debug_assert!(batch.tokens.len() <= effects.capacity() - effects.len());
            for token in batch.tokens.drain(..) { effects.push_back(Effect { token, rejection }); }
            false
        });
        if self.pending_for_source(scope) != 0 {
            debug_assert!(self.pending.len() <= self.effects.capacity() - self.effects.len());
            for token in self.pending.drain(..) {
                self.effects.push_back(Effect { token, rejection: Some(Rejection::Abort) });
            }
            self.pending_scope = None;
        }
    }

    pub(super) fn reject_pending_now(&mut self,scope:&Scope,reason:Rejection) {
        if self.pending_for_source(scope)!=0 {
            debug_assert!(self.pending.len()<=self.effects.capacity()-self.effects.len());
            for token in self.pending.drain(..) {self.effects.push_back(Effect {token,rejection:Some(reason)});}
            self.pending_scope=None;
        }
    }

    pub(super) fn peek_effect(&self) -> Option<Effect> { self.effects.front().cloned() }

    /// Call only after Runtime confirms settlement (including already settled).
    /// A failing settlement must stay at the front for pending-state inspection.
    pub(super) fn commit_effect(&mut self) -> bool {
        if self.effects.pop_front().is_none() { return false; }
        self.outstanding -= 1;
        true
    }

    pub(super) fn has_effects(&self) -> bool { !self.effects.is_empty() }
    pub(super) fn has_pending(&self) -> bool { self.outstanding != 0 }
}
