//! Direct, document-owned CharacterData observers.
//! https://dom.spec.whatwg.org/#mutation-observers
use super::function_reachability::Edge;
use super::*;
use std::collections::HashMap;

const MAX_OBSERVERS: usize = 4_096;
const MAX_REGISTRATIONS: usize = 65_536;
const MAX_PENDING_RECORDS: usize = 65_536;

#[derive(Clone, Copy)]
pub(super) enum Op {
    Constructor,
    RecordConstructor,
    Observe,
    Disconnect,
    TakeRecords,
    Record(&'static str),
}

impl Op {
    pub(super) fn arity(self) -> usize {
        match self {
            Self::Constructor => 1,
            Self::Observe => 2,
            _ => 0,
        }
    }
}

#[derive(Default)]
pub(super) struct Arena {
    observer_prototype: Option<usize>,
    record_prototype: Option<usize>,
    observers: BTreeMap<usize, Observer>,
    targets: HashMap<u32, Vec<Registration>>,
    records: HashMap<usize, Record>,
    registrations: usize,
    pending_records: usize,
    queued: bool,
}

struct Observer {
    callback: Value,
    targets: Vec<u32>,
    pending: Vec<Record>,
    scheduled: bool,
}

#[derive(Clone, Copy)]
struct Registration {
    observer: usize,
    old_value: bool,
}

struct Record {
    target: u32,
    old_value: Value,
    added_nodes: Option<u32>,
    removed_nodes: Option<u32>,
}

impl Record {
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.old_value.bytes())
    }
}

pub(super) struct PreparedChange {
    target: u32,
    generation: u64,
    entries: Vec<(usize, Record)>,
    retained: usize,
    new_job: bool,
}

impl Arena {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(Edge)) {
        for callback in self.observers.values().map(|observer| &observer.callback) {
            Edge::value(callback, add);
        }
    }

    pub(super) fn reachability_cost(&self) -> usize {
        self.observers.len()
    }
}

impl Runtime {
    pub(crate) fn define_mutation_observers(&mut self) -> Result<(), String> {
        let marker = self.inhibit_transient_collection();
        let result = self.define_mutation_observer_interfaces();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }

    fn define_mutation_observer_interfaces(&mut self) -> Result<(), Fault> {
        for (name, op) in [
            ("MutationObserver", Op::Constructor),
            ("MutationRecord", Op::RecordConstructor),
        ] {
            let prototype = self.new_object(None)?;
            let constructor = self.native_function(Builtin::MutationObserver(op), name)?;
            self.put_binding(self.global_environment, name.into(), Some(constructor), Declaration::Var, false)?;
            self.global_attributes.insert(name.into(), (false, true, true));
            self.constructor_prototype(name, Value::Object(prototype))?;
            self.store_symbol_property(prototype, self.symbol_to_string_tag.unwrap(),
                Value::String(name.into()), Some((false, false, true)))?;
            if matches!(op, Op::Constructor) {
                self.mutations.observer_prototype = Some(prototype);
                for (method, op) in [
                    ("observe", Op::Observe),
                    ("disconnect", Op::Disconnect),
                    ("takeRecords", Op::TakeRecords),
                ] {
                    self.prototype_method(prototype, method, Builtin::MutationObserver(op))?;
                }
            } else {
                self.mutations.record_prototype = Some(prototype);
                for field in ["type", "target", "addedNodes", "removedNodes", "previousSibling",
                    "nextSibling", "attributeName", "attributeNamespace", "oldValue"]
                {
                    let Value::Function(get) = self.native_function(
                        Builtin::MutationObserver(Op::Record(field)), &format!("get {field}"))?
                    else { unreachable!() };
                    self.store_accessor_property(prototype, field.into(),
                        Accessors { get: Some(get), set: None }, (true, true))?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn construct_mutation_observer(
        &mut self, args: &[Value], new_target: Option<usize>, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let Some(callback @ Value::Function(_)) = args.first() else {
            return Err(Fault::exception("TypeError", "MutationObserver requires a callable callback."));
        };
        let prototype = match new_target {
            Some(target) => Ordinary::from_value(&self.read_property(&Value::Function(target), "prototype", host)?),
            None => None,
        }.or(self.mutations.observer_prototype.map(Ordinary::Object));
        if self.mutations.observers.len() >= MAX_OBSERVERS {
            return Err(Fault::new("MutationObserver identity limit reached."));
        }
        let id = self.new_object(None)?;
        self.objects[id].prototype = prototype;
        self.mutations.observers.insert(id, Observer {
            callback: callback.clone(), targets: Vec::new(), pending: Vec::new(), scheduled: false,
        });
        Ok(Value::Object(id))
    }

    pub(super) fn mutation_observer_builtin(
        &mut self, op: Op, args: &[Value], this: Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match op {
            Op::Constructor => return Err(Fault::exception("TypeError", "MutationObserver requires new.")),
            Op::RecordConstructor => return Err(Fault::exception("TypeError", "Illegal MutationRecord constructor.")),
            _ => {}
        }
        let Value::Object(id) = this else {
            return Err(Fault::exception("TypeError", "Incompatible mutation observer receiver."));
        };
        if let Op::Record(field) = op {
            return self.mutation_record_field(id, field, host);
        }
        if !self.mutations.observers.contains_key(&id) {
            return Err(Fault::exception("TypeError", "Incompatible MutationObserver receiver."));
        }
        match op {
            Op::Observe => {
                self.observe_character_data(id, args, host)?;
                Ok(Value::Undefined)
            }
            Op::Disconnect => {
                for target in std::mem::take(&mut self.mutations.observers.get_mut(&id).unwrap().targets) {
                    let registrations = self.mutations.targets.get_mut(&target).unwrap();
                    registrations.retain(|registration| registration.observer != id);
                    self.mutations.registrations -= 1;
                    if registrations.is_empty() {
                        self.mutations.targets.remove(&target);
                    }
                }
                self.take_mutation_queue(id);
                Ok(Value::Undefined)
            }
            Op::TakeRecords => self.build_mutation_records(id),
            _ => unreachable!(),
        }
    }

    fn observe_character_data(&mut self, id: usize, args: &[Value], host: &mut dyn Host) -> Result<(), Fault> {
        let (Some(Value::Host(target)), Some(options)) = (args.first(), args.get(1)) else {
            return Err(Fault::exception("TypeError", "MutationObserver.observe requires a Node and options."));
        };
        self.validate_native_access(*target, host)?;
        let supported_target = host.mutation_observation_target(*target)
            .ok_or_else(|| Fault::exception("TypeError", "MutationObserver.observe requires a Node."))?;
        if !matches!(options, Value::Undefined | Value::Null | Value::Host(_) | Value::Object(_) | Value::Function(_)) {
            return Err(Fault::exception("TypeError", "MutationObserver options must be a dictionary."));
        }
        let mut option = |name| {
            if matches!(options, Value::Undefined | Value::Null) { Ok(Value::Undefined) }
            else { self.read_property(options, name, host) }
        };
        let filter = option("attributeFilter")?;
        let attribute_old = option("attributeOldValue")?;
        let attributes = option("attributes")?;
        let character = option("characterData")?;
        let old = option("characterDataOldValue")?;
        let children = option("childList")?;
        option("subtree")?;
        let has_filter = !matches!(filter, Value::Undefined);
        let attributes = if matches!(attributes, Value::Undefined) {
            has_filter || !matches!(attribute_old, Value::Undefined)
        } else { attributes.truthy() };
        let character = if matches!(character, Value::Undefined) { !matches!(old, Value::Undefined) }
            else { character.truthy() };
        if (!attributes && (has_filter || attribute_old.truthy())) || (!character && old.truthy())
            || !(attributes || character || children.truthy())
        {
            return Err(Fault::exception("TypeError", "MutationObserver options select inconsistent or empty mutation types."));
        }
        if attributes || children.truthy() {
            let error = self.native_dom_exception("NotSupportedError", "Attribute and child-list observation are unavailable.")?;
            return Err(Fault::thrown(error));
        }
        if !supported_target {
            let error = self.native_dom_exception("NotSupportedError", "Only direct CharacterData observation is available.")?;
            return Err(Fault::thrown(error));
        }
        if let Some(registration) = self.mutations.targets.get_mut(target)
            .and_then(|registrations| registrations.iter_mut().find(|registration| registration.observer == id))
        {
            registration.old_value = old.truthy();
            return Ok(());
        }
        if self.mutations.registrations >= MAX_REGISTRATIONS {
            return Err(Fault::new("MutationObserver registration limit reached."));
        }
        self.mutations.targets.try_reserve(1).map_err(|_| Fault::new("Cannot reserve mutation targets."))?;
        let observer = self.mutations.observers.get_mut(&id).unwrap();
        observer.targets.try_reserve(1).map_err(|_| Fault::new("Cannot reserve observer targets."))?;
        use std::collections::hash_map::Entry;
        let registrations = match self.mutations.targets.entry(*target) {
            Entry::Occupied(entry) => {
                let registrations = entry.into_mut();
                registrations.try_reserve(1).map_err(|_| Fault::new("Cannot reserve mutation registrations."))?;
                registrations
            }
            Entry::Vacant(entry) => {
                let mut registrations = Vec::new();
                registrations.try_reserve(1).map_err(|_| Fault::new("Cannot reserve mutation registrations."))?;
                entry.insert(registrations)
            }
        };
        registrations.push(Registration { observer: id, old_value: old.truthy() });
        observer.targets.push(*target);
        self.mutations.registrations += 1;
        Ok(())
    }

    pub(super) fn prepare_character_data_write(
        &mut self, target: u32, property: &str, host: &mut dyn Host,
    ) -> Result<Option<PreparedChange>, Fault> {
        let Some(generation) = host.character_data_write_generation(target, property) else { return Ok(None) };
        let Some(registrations) = self.mutations.targets.get(&target) else { return Ok(None) };
        if self.mutations.pending_records.saturating_add(registrations.len()) > MAX_PENDING_RECORDS {
            return Err(Fault::new("MutationObserver pending-record limit reached."));
        }
        let count = registrations.len();
        if count == 0 { return Ok(None); }
        self.work(count)?;
        let registrations = self.mutations.targets[&target].clone();
        let old = if registrations.iter().any(|registration| registration.old_value) {
            self.host_get_checked(target, "data", host)?
        } else { Value::Null };
        let mut entries = Vec::new();
        entries.try_reserve_exact(registrations.len()).map_err(|_| Fault::new("Cannot reserve mutation records."))?;
        let copies = registrations.iter().filter(|registration| registration.old_value).count();
        let retained = count.saturating_mul(std::mem::size_of::<Record>())
            .saturating_add(copies.saturating_mul(old.bytes()));
        if self.value_bytes.saturating_add(retained) > self.limits.heap.retained_values {
            return Err(Fault::new("MutationObserver retained-value limit reached."));
        }
        for registration in registrations {
            self.mutations.observers.get_mut(&registration.observer).unwrap().pending.try_reserve(1)
                .map_err(|_| Fault::new("Cannot reserve observer records."))?;
            let old_value = if registration.old_value { self.copy_value(&old)? } else { Value::Null };
            let record = Record { target, old_value, added_nodes: None, removed_nodes: None };
            entries.push((registration.observer, record));
        }
        self.value_bytes += retained;
        let new_job = !self.mutations.queued;
        if new_job {
            if let Err(error) = self.enqueue_mutation_observers() {
                self.value_bytes -= retained;
                return Err(error);
            }
        }
        Ok(Some(PreparedChange { target, generation, entries, retained, new_job }))
    }

    pub(super) fn finish_character_data_write(
        &mut self, pending: Option<PreparedChange>, property: &str, host: &dyn Host,
    ) -> Result<(), Fault> {
        let Some(pending) = pending else { return Ok(()) };
        if host.character_data_write_generation(pending.target, property)
            .is_some_and(|generation| generation != pending.generation)
        {
            self.mutations.queued = true;
            self.mutations.pending_records += pending.entries.len();
            for (id, record) in pending.entries {
                let observer = self.mutations.observers.get_mut(&id).unwrap();
                observer.pending.push(record);
                observer.scheduled = true;
            }
        } else {
            self.value_bytes -= pending.retained;
            if pending.new_job { self.remove_mutation_observer_reservation()?; }
        }
        Ok(())
    }

    fn take_mutation_queue(&mut self, id: usize) -> Vec<Record> {
        let records = std::mem::take(&mut self.mutations.observers.get_mut(&id).unwrap().pending);
        self.mutations.pending_records -= records.len();
        self.value_bytes -= records.iter().map(Record::retained_bytes).sum::<usize>();
        records
    }

    pub(super) fn cancel_mutation_observers(&mut self) {
        self.mutations.queued = false;
        self.mutations.pending_records = 0;
        for observer in self.mutations.observers.values_mut() {
            self.value_bytes -= observer.pending.iter().map(Record::retained_bytes).sum::<usize>();
            observer.pending = Vec::new();
            observer.scheduled = false;
        }
    }

    fn build_mutation_records(&mut self, observer: usize) -> Result<Value, Fault> {
        let queue = self.take_mutation_queue(observer);
        self.mutations.records.try_reserve(queue.len()).map_err(|_| Fault::new("Cannot reserve MutationRecord identities."))?;
        let array = self.new_object(Some(queue.len()))?;
        for (index, record) in queue.into_iter().enumerate() {
            let id = self.new_object(None)?;
            self.objects[id].prototype = self.mutations.record_prototype.map(Ordinary::Object);
            let retained = self.value_bytes.saturating_add(record.retained_bytes());
            if retained > self.limits.heap.retained_values {
                return Err(Fault::new("MutationRecord retained-value limit reached."));
            }
            self.mutations.records.insert(id, record);
            self.value_bytes = retained;
            self.store_property(array, index.to_string(), Value::Object(id), Some((true, true, true)))?;
        }
        Ok(Value::Object(array))
    }

    fn mutation_record_field(&mut self, id: usize, field: &str, host: &mut dyn Host) -> Result<Value, Fault> {
        let record = self.mutations.records.get(&id)
            .ok_or_else(|| Fault::exception("TypeError", "Incompatible MutationRecord receiver."))?;
        self.validate_native_access(record.target, host)?;
        match field {
            "type" => Ok(Value::String("characterData".into())),
            "target" => Ok(Value::Host(record.target)),
            "oldValue" => {
                self.work(record.old_value.bytes())?;
                Ok(self.mutations.records[&id].old_value.clone())
            }
            "addedNodes" | "removedNodes" => {
                let list = if field == "addedNodes" { record.added_nodes } else { record.removed_nodes };
                let handle = if let Some(handle) = list { handle } else {
                    let handle = host.empty_mutation_node_list().map_err(Fault::new)?;
                    let record = self.mutations.records.get_mut(&id).unwrap();
                    if field == "addedNodes" { record.added_nodes = Some(handle); }
                    else { record.removed_nodes = Some(handle); }
                    handle
                };
                Ok(Value::Host(handle))
            }
            _ => Ok(Value::Null),
        }
    }

    pub(super) fn notify_mutation_observers(&mut self, host: &mut dyn Host) -> Result<(), Fault> {
        self.mutations.queued = false;
        self.work(self.mutations.observers.len())?;
        let batch: Vec<_> = self.mutations.observers.iter_mut()
            .filter_map(|(&id, observer)| std::mem::take(&mut observer.scheduled).then_some(id)).collect();
        for id in batch {
            let observer = &self.mutations.observers[&id];
            if observer.pending.is_empty() { continue; }
            let callback = observer.callback.clone();
            let records = self.build_mutation_records(id)?;
            let observer = Value::Object(id);
            if let Err(error) = self.call_value(callback, &[records, observer.clone()], observer, host) {
                if error.exception.is_none() { return Err(error); }
                host.mutation_callback_error(&error.display());
            }
        }
        Ok(())
    }
}
