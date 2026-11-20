//! Ordinary Promise handlers and suspended async invocations share the same
//! pending reaction list. Settling a Promise moves each observer onto the FIFO.
//! https://tc39.es/ecma262/#await
use super::*;
use super::super::{async_execution::Activation, function_reachability::Edge};

pub(super) enum Reaction {
    Then {
        capability: Capability,
        fulfill: Option<usize>,
        reject: Option<usize>,
    },
    Await(Box<Activation>),
}

impl Reaction {
    pub(super) fn into_job(self, value: Value, rejected: bool) -> Job {
        match self {
            Self::Then { capability, fulfill, reject } => Job::Reaction {
                capability,
                handler: if rejected { reject } else { fulfill },
                value,
                reject: rejected,
            },
            Self::Await(frame) => Job::Await { frame, value, reject: rejected },
        }
    }

    pub(super) fn payload_bytes(&self) -> usize {
        match self {
            Self::Then { .. } => 0,
            Self::Await(frame) => frame.retained_bytes(),
        }
    }

    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(Edge)) {
        match self {
            Self::Then { capability, fulfill, reject } => {
                capability.reachability_edges(add);
                for id in [*fulfill, *reject].into_iter().flatten() {
                    add(Edge::Function(id));
                }
            }
            Self::Await(frame) => frame.reachability_edges(add),
        }
    }

    pub(super) fn reachability_cost(&self) -> usize {
        match self {
            Self::Then { .. } => 5,
            Self::Await(frame) => frame.reachability_cost(),
        }
    }
}

impl Runtime {
    /// Use the function realm's intrinsic constructor. An observable
    /// constructor getter or thenable still goes through PromiseResolve.
    pub(in super::super) fn promise_for_await(
        &mut self, value: Value, host: &mut dyn Host,
    ) -> Result<usize, Fault> {
        let constructor = self.promises.constructor.ok_or_else(|| Fault::new("Missing intrinsic Promise constructor."))?;
        match self.promise_resolve(Value::Function(constructor), value, host)? {
            Value::Object(promise) => Ok(promise),
            _ => Err(Fault::new("Intrinsic Promise resolution returned an invalid object.")),
        }
    }

    pub(in super::super) fn suspend_await(&mut self, promise: usize, frame: Box<Activation>) -> Result<(), Fault> {
        self.observe_promise(promise, Reaction::Await(frame))
    }

    /// Perform the internal observation step directly. Await does not fetch
    /// the public then method or create a species-selected result Promise.
    pub(super) fn observe_promise(&mut self, promise: usize, reaction: Reaction) -> Result<(), Fault> {
        let data = self.objects[promise].promise.as_ref().unwrap();
        match &data.state {
            State::Pending => {
                #[cfg(test)]
                if self.limits.test_entry_arenas && self.promises.reactions >= self.limits.capacity.promises {
                    return Err(Fault::new(format!("Promise reaction limit reached ({}).", self.limits.capacity.promises)));
                }
                let count = self.promises.reactions.checked_add(1).ok_or_else(|| Fault::new("Promise reaction counter overflow."))?;
                let payload = reaction.payload_bytes();
                let available = self.limits.heap.retained_values.checked_sub(self.value_bytes)
                    .and_then(|bytes| bytes.checked_sub(payload)).ok_or_else(|| Fault::new("Retained value reaction storage exhausted."))?;
                let growth = super::super::limits::prepare_retained_growth(&data.reactions, 1, available)?;
                self.work(payload.saturating_add(std::mem::size_of::<Reaction>().saturating_mul(
                    if growth.added == 0 { 1 } else { data.reactions.len().saturating_add(1) })))?;
                self.value_bytes += payload + growth.added;
                let reactions = &mut self.objects[promise].promise.as_mut().unwrap().reactions;
                growth.commit(reactions);
                reactions.push(reaction);
                self.promises.reactions = count;
            }
            State::Rejected(value) | State::Fulfilled(value) => {
                let reject = matches!(data.state, State::Rejected(_));
                let job = reaction.into_job(value.clone(), reject);
                self.promise_enqueue(job)?;
            }
        }
        self.objects[promise].promise.as_mut().unwrap().handled = true;
        Ok(())
    }
}
