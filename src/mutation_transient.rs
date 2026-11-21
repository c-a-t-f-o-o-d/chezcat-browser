//! Source-aware transient registrations over real detached native nodes.
//! https://dom.spec.whatwg.org/#concept-node-add-transient-registered-observers
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Transient {
    target: u32,
    source: u32,
    observer: usize,
}
impl Transient {
    fn key(self) -> (u32, u32, usize) {
        (self.target, self.source, self.observer)
    }
}
pub(super) struct TransientPlan {
    entries: Vec<Transient>,
    replacement: Option<Vec<Transient>>,
    ordered: bool,
}
impl TransientPlan {
    pub(super) fn retained_bytes(&self) -> usize {
        self.entries.capacity() * std::mem::size_of::<Transient>()
            + self
                .replacement
                .as_ref()
                .map_or(0, |v| v.capacity() * std::mem::size_of::<Transient>())
    }
}
impl Arena {
    fn transient_at(&self, node: u32) -> &[Transient] {
        let start = self.transients.partition_point(|entry| entry.target < node);
        let length = self.transients[start..].partition_point(|entry| entry.target == node);
        &self.transients[start..start + length]
    }
    fn source_registration(&self, entry: Transient) -> Option<&Registration> {
        self.targets
            .get(&entry.source)?
            .iter()
            .find(|reg| reg.observer == entry.observer)
    }
    pub(super) fn registrations_at(&self, node: u32) -> impl Iterator<Item = &Registration> {
        self.targets.get(&node).into_iter().flatten().chain(
            self.transient_at(node)
                .iter()
                .filter_map(|entry| self.source_registration(*entry)),
        )
    }
    pub(super) fn transient_lookup_work(&self, node: u32) -> usize {
        let search = usize::BITS - self.transients.len().leading_zeros();
        self.transient_at(node)
            .iter()
            .fold(search as usize * 2, |n, entry| {
                n.saturating_add(self.targets.get(&entry.source).map_or(0, Vec::len))
            })
    }
    pub(super) fn observes_tree_mutations(&self) -> bool {
        self.child_list_registrations != 0
            || self.subtree_registrations != 0
            || !self.transients.is_empty()
    }
}

fn reserve_entries<T>(
    entries: &mut Vec<T>,
    needed: usize,
    available: usize,
) -> Result<usize, Fault> {
    let maximum = available / std::mem::size_of::<T>();
    if needed > maximum {
        return Err(Fault::new(
            "Transient observer metadata allowance exhausted.",
        ));
    }
    if needed <= entries.capacity() {
        return Ok(0);
    }
    let capacity = entries
        .capacity()
        .saturating_mul(2)
        .max(4)
        .max(needed)
        .min(maximum);
    let moved = entries.len();
    entries
        .try_reserve_exact(capacity - entries.len())
        .map_err(|_| Fault::new("Cannot reserve transient observer metadata."))?;
    if entries.capacity() > maximum {
        return Err(Fault::new(
            "Allocated transient metadata exceeds its allowance.",
        ));
    }
    Ok(moved)
}

impl Runtime {
    pub(super) fn has_subtree_sources(
        &mut self,
        target: u32,
        host: &dyn Host,
    ) -> Result<bool, Fault> {
        if self.mutations.subtree_registrations == 0 && self.mutations.transients.is_empty() {
            return Ok(false);
        }
        let mut current = Some(target);
        while let Some(node) = current {
            let work = self
                .mutations
                .transient_lookup_work(node)
                .saturating_add(self.mutations.targets.get(&node).map_or(0, Vec::len))
                .saturating_add(1);
            self.work(work)?;
            if self.mutations.registrations_at(node).any(|reg| reg.subtree) {
                return Ok(true);
            }
            current = host.mutation_observation_parent(node);
        }
        Ok(false)
    }
    pub(super) fn prepare_transient_changes(
        &mut self,
        changes: &[NativeChildListChange],
        host: &dyn Host,
    ) -> Result<Option<TransientPlan>, Fault> {
        if self.mutations.subtree_registrations == 0 && self.mutations.transients.is_empty() {
            return Ok(None);
        }
        let available = self
            .limits
            .heap
            .retained_values
            .checked_sub(self.value_bytes)
            .ok_or_else(|| Fault::new("Transient observer retained allowance exhausted."))?;
        let mut entries = Vec::new();
        for change in changes {
            if change.removed.is_empty() {
                continue;
            }
            let mut current = Some(change.target);
            while let Some(node) = current {
                let count = self.mutations.targets.get(&node).map_or(0, Vec::len);
                let transient_count = self.mutations.transient_at(node).len();
                self.work(
                    self.mutations
                        .transient_lookup_work(node)
                        .saturating_add(count)
                        .saturating_add(1),
                )?;
                for index in 0..count + transient_count {
                    let source = if index < count {
                        let reg = &self.mutations.targets[&node][index];
                        reg.subtree.then_some((node, reg.observer))
                    } else {
                        let entry = self.mutations.transient_at(node)[index - count];
                        self.mutations
                            .source_registration(entry)
                            .filter(|reg| reg.subtree)
                            .map(|_| (entry.source, entry.observer))
                    };
                    let Some((source, observer)) = source else {
                        continue;
                    };
                    self.work(change.removed.len())?;
                    for &target in &change.removed {
                        let entry = Transient {
                            target,
                            source,
                            observer,
                        };
                        self.work(
                            (usize::BITS - self.mutations.transients.len().leading_zeros())
                                as usize
                                + 1,
                        )?;
                        if self
                            .mutations
                            .transients
                            .binary_search_by_key(&entry.key(), |entry| entry.key())
                            .is_ok()
                        {
                            continue;
                        }
                        let needed = entries
                            .len()
                            .checked_add(1)
                            .ok_or_else(|| Fault::new("Transient observer count overflow."))?;
                        let moved = if needed > entries.capacity() {
                            entries.len()
                        } else {
                            0
                        };
                        self.work(
                            moved
                                .saturating_add(1)
                                .saturating_mul(std::mem::size_of::<Transient>()),
                        )?;
                        reserve_entries(&mut entries, needed, available)?;
                        entries.push(entry);
                    }
                }
                current = host.mutation_observation_parent(node);
            }
        }
        if entries.is_empty() {
            return Ok(None);
        }
        self.work(
            entries
                .len()
                .saturating_mul((usize::BITS - entries.len().leading_zeros()) as usize + 1),
        )?;
        entries.sort_unstable_by_key(|entry| entry.key());
        entries.dedup();
        let required = self
            .mutations
            .transients
            .len()
            .checked_add(entries.len())
            .ok_or_else(|| Fault::new("Transient observer backing overflow."))?;
        let mut replacement = None;
        if required > self.mutations.transients.capacity() {
            let scratch = entries.capacity() * std::mem::size_of::<Transient>();
            let available = available
                .checked_sub(scratch)
                .ok_or_else(|| Fault::new("Transient observer staging allowance exhausted."))?;
            let maximum = available / std::mem::size_of::<Transient>();
            let capacity = self
                .mutations
                .transients
                .capacity()
                .saturating_mul(2)
                .max(required)
                .min(maximum);
            if capacity < required {
                return Err(Fault::new(
                    "Transient observer replacement allowance exhausted.",
                ));
            }
            let mut next = Vec::new();
            reserve_entries(&mut next, capacity, available)?;
            replacement = Some(next);
        }
        let ordered = self
            .mutations
            .transients
            .last()
            .zip(entries.first())
            .is_none_or(|(old, new)| old.key() < new.key());
        if ordered {
            self.work(
                if replacement.is_some() {
                    required
                } else {
                    entries.len()
                }
                .saturating_add(1),
            )?;
        } else {
            self.work(
                required.saturating_mul((usize::BITS - required.leading_zeros()) as usize + 1),
            )?;
        }
        Ok(Some(TransientPlan {
            entries,
            replacement,
            ordered,
        }))
    }
    pub(super) fn commit_transient_changes(&mut self, mut plan: TransientPlan) {
        if let Some(mut next) = plan.replacement.take() {
            next.append(&mut self.mutations.transients);
            let old = std::mem::replace(&mut self.mutations.transients, next);
            self.value_bytes -= old.capacity() * std::mem::size_of::<Transient>();
            drop(old);
        }
        for entry in &plan.entries {
            self.mutations
                .observers
                .get_mut(&entry.observer)
                .expect("preflight observer")
                .scheduled = true;
        }
        self.mutations.transients.append(&mut plan.entries);
        if !plan.ordered {
            self.mutations
                .transients
                .sort_unstable_by_key(|entry| entry.key());
        }
        self.value_bytes -= plan.entries.capacity() * std::mem::size_of::<Transient>();
    }
    pub(super) fn clear_transient_observer(&mut self, observer: usize) -> Result<(), Fault> {
        self.work(self.mutations.transients.len())?;
        self.mutations
            .transients
            .retain(|entry| entry.observer != observer);
        Ok(())
    }
    pub(super) fn clear_transient_source(&mut self, observer: usize, source: u32) {
        self.mutations
            .transients
            .retain(|entry| entry.observer != observer || entry.source != source);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Tree;
    impl Host for Tree {
        fn get(&mut self, _object: u32, _property: &str) -> Result<Value, String> {
            Err("unused tree getter".into())
        }
        fn set(&mut self, _object: u32, _property: &str, _value: Value) -> Result<(), String> {
            Err("unused tree setter".into())
        }
        fn call(&mut self, _object: u32, _method: &str, _args: &[Value]) -> Result<Value, String> {
            Err("unused tree call".into())
        }
    }
    #[test]
    fn transient_observation_native_backing_and_notification_admission_refuse_and_recover() {
        let mut runtime = Runtime::with_limits(limits::BROWSER);
        runtime.mutations.targets.insert(
            1,
            vec![Registration {
                observer: 0,
                character_data: false,
                child_list: false,
                attributes: true,
                subtree: true,
                old_value: false,
                attribute_old_value: false,
                attribute_filter: None,
            }],
        );
        runtime.mutations.observers.insert(
            0,
            Observer {
                callback: Value::Undefined,
                targets: vec![1],
                pending: Vec::new(),
                scheduled: false,
            },
        );
        runtime.mutations.registrations = 1;
        runtime.mutations.subtree_registrations = 1;
        runtime.mutations.attribute_registrations = 1;
        let changes = vec![NativeChildListChange {
            target: 1,
            added: Vec::new(),
            removed: vec![2],
            previous: None,
            next: None,
        }];
        let base = runtime.value_bytes;
        runtime.limits.heap.retained_values = base + std::mem::size_of::<Transient>();
        assert!(runtime.prepare_transient_changes(&changes, &Tree).is_err());
        assert_eq!(runtime.value_bytes, base);
        assert_eq!(runtime.mutations.transients.capacity(), 0);
        runtime.limits.heap.retained_values = base + 4096;
        let plan = runtime.prepare_transient_changes(&changes, &Tree).unwrap();
        let pending = runtime
            .prepare_mutation_effects(1, 0, Vec::new(), plan)
            .unwrap();
        runtime
            .finish_mutation_records(pending.unwrap(), false)
            .unwrap();
        assert_eq!(runtime.mutations.transients.capacity(), 0);
        assert!(!runtime.mutations.queued);
        assert!(!runtime.mutations.observers[&0].scheduled);
        let plan = runtime.prepare_transient_changes(&changes, &Tree).unwrap();
        let pending = runtime
            .prepare_mutation_effects(1, 0, Vec::new(), plan)
            .unwrap();
        runtime
            .finish_mutation_records(pending.unwrap(), true)
            .unwrap();
        assert_eq!(runtime.mutations.transients.len(), 1);
        assert!(runtime.mutations.observers[&0].scheduled);
        assert_eq!(runtime.mutations.pending_records, 0);
        let backing = runtime.mutations.transients.capacity();
        runtime.notify_mutation_observers(&mut Tree).unwrap();
        assert!(runtime.mutations.transients.is_empty());
        assert_eq!(runtime.mutations.transients.capacity(), backing);
    }
}
