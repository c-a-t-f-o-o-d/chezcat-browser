//! Document-owned mutation observers and microtask delivery.
//! https://dom.spec.whatwg.org/#mutation-observers
use super::function_reachability::Edge;
use super::*;
use std::collections::HashMap;

#[path="mutation_transient.rs"]
mod transient;
use transient::{Transient,TransientPlan};

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
    child_list_registrations: usize,
    attribute_registrations: usize,
    subtree_registrations: usize,
    transients: Vec<Transient>,
    pending_records: usize,
    queued: bool,
}

struct Observer {
    callback: Value,
    targets: Vec<u32>,
    pending: Vec<Record>,
    scheduled: bool,
}

struct Registration {
    observer: usize,
    character_data: bool,
    child_list: bool,
    attributes: bool,
    subtree: bool,
    old_value: bool,
    attribute_old_value: bool,
    attribute_filter: Option<Vec<String>>,
}

impl Registration {
    fn retained_bytes(&self) -> usize {
        self.attribute_filter.as_ref().map_or(0, |filter| {
            filter.capacity().saturating_mul(std::mem::size_of::<String>())
                .saturating_add(filter.iter().map(String::capacity).sum::<usize>())
        })
    }
}

struct Record {
    target: u32,
    old_value: Value,
    child_list: Option<Rc<NativeChildListChange>>,
    attribute: Option<Rc<NativeAttributeChange>>,
    added_nodes: Option<u32>,
    removed_nodes: Option<u32>,
}

impl Record {
    fn retained_bytes(&self) -> usize {
        let child_references = self.child_list.as_ref().map_or(0, |change|
            std::mem::size_of::<NativeChildListChange>().saturating_add(
                change.added.capacity().saturating_add(change.removed.capacity())
                    .saturating_mul(std::mem::size_of::<u32>())));
        let attribute_bytes = self.attribute.as_ref().map_or(0, |change|
            std::mem::size_of::<NativeAttributeChange>().saturating_add(change.local_name.capacity())
                .saturating_add(change.namespace.as_ref().map_or(0, crate::dom_string::DomString::capacity)));
        std::mem::size_of::<Self>().saturating_add(self.old_value.bytes())
            .saturating_add(child_references).saturating_add(attribute_bytes)
    }
}

pub(super) struct PreparedChange {
    target: u32,
    generation: u64,
    entries: Vec<(usize, Record)>,
    retained: usize,
    new_job: bool,
    transient: Option<TransientPlan>,
}

impl Arena {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(Edge)) {
        for prototype in [self.observer_prototype, self.record_prototype].into_iter().flatten() {
            add(Edge::Object(prototype));
        }
        for callback in self.observers.values().map(|observer| &observer.callback) {
            Edge::value(callback, add);
        }
    }

    pub(super) fn reachability_cost(&self) -> usize {
        self.observers.len() + usize::from(self.observer_prototype.is_some())
            + usize::from(self.record_prototype.is_some())
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
                self.observe_mutations(id, args, host)?;
                Ok(Value::Undefined)
            }
            Op::Disconnect => {
                self.clear_transient_observer(id)?;
                for target in std::mem::take(&mut self.mutations.observers.get_mut(&id).unwrap().targets) {
                    let registrations = self.mutations.targets.get_mut(&target).unwrap();
                    registrations.retain(|registration| {
                        if registration.observer != id { return true; }
                        self.mutations.child_list_registrations -= usize::from(registration.child_list);
                        self.mutations.attribute_registrations -= usize::from(registration.attributes);
                        self.mutations.subtree_registrations -= usize::from(registration.subtree);
                        self.value_bytes -= registration.retained_bytes();
                        false
                    });
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

    fn observe_mutations(&mut self, id: usize, args: &[Value], host: &mut dyn Host) -> Result<(), Fault> {
        let (Some(Value::Host(target)), Some(options)) = (args.first(), args.get(1)) else {
            return Err(Fault::exception("TypeError", "MutationObserver.observe requires a Node and options."));
        };
        self.validate_native_access(*target, host)?;
        let supported_target = host.mutation_observation_target(*target)
            .ok_or_else(|| Fault::exception("TypeError", "MutationObserver.observe requires a Node."))?;
        if !matches!(options, Value::Undefined | Value::Null | Value::Host(_) | Value::Object(_) | Value::Function(_)) {
            return Err(Fault::exception("TypeError", "MutationObserver options must be a dictionary."));
        }
        let filter = self.mutation_option(options, "attributeFilter", host)?;
        let attribute_filter = if matches!(filter, Value::Undefined) { None }
            else { Some(self.attribute_filter_strings(filter, host)?) };
        let attribute_old = self.mutation_option(options, "attributeOldValue", host)?;
        let attributes = self.mutation_option(options, "attributes", host)?;
        let character = self.mutation_option(options, "characterData", host)?;
        let old = self.mutation_option(options, "characterDataOldValue", host)?;
        let children = self.mutation_option(options, "childList", host)?.truthy();
        let subtree = self.mutation_option(options, "subtree", host)?.truthy();
        let has_filter = attribute_filter.is_some();
        let attributes = if matches!(attributes, Value::Undefined) {
            has_filter || !matches!(attribute_old, Value::Undefined)
        } else { attributes.truthy() };
        let character = if matches!(character, Value::Undefined) { !matches!(old, Value::Undefined) }
            else { character.truthy() };
        if (!attributes && (has_filter || attribute_old.truthy())) || (!character && old.truthy())
            || !(attributes || character || children)
        {
            return Err(Fault::exception("TypeError", "MutationObserver options select inconsistent or empty mutation types."));
        }
        let tree_target = host.mutation_child_list_target(*target);
        if (children || attributes || subtree) && !tree_target
            || character && !supported_target && !(subtree && tree_target) && !(children || attributes)
        {
            let error = self.native_dom_exception("NotSupportedError", "This Node does not support the requested observations.")?;
            return Err(Fault::thrown(error));
        }
        let requested = Registration {
            observer: id, character_data: character, child_list: children, attributes, subtree,
            old_value: old.truthy(), attribute_old_value: attribute_old.truthy(), attribute_filter,
        };
        let bytes = requested.retained_bytes();
        self.work(self.mutations.transients.len())?;
        if let Some(registration) = self.mutations.targets.get_mut(target)
            .and_then(|registrations| registrations.iter_mut().find(|registration| registration.observer == id))
        {
            let retained = self.value_bytes.saturating_sub(registration.retained_bytes()).saturating_add(bytes);
            if retained > self.limits.heap.retained_values {
                return Err(Fault::new("MutationObserver attribute-filter retained limit reached."));
            }
            self.mutations.child_list_registrations -= usize::from(registration.child_list);
            self.mutations.child_list_registrations += usize::from(children);
            self.mutations.attribute_registrations -= usize::from(registration.attributes);
            self.mutations.attribute_registrations += usize::from(attributes);
            self.mutations.subtree_registrations -= usize::from(registration.subtree);
            self.mutations.subtree_registrations += usize::from(subtree);
            *registration = requested;
            self.clear_transient_source(id,*target);
            self.value_bytes = retained;
            return Ok(());
        }
        if self.value_bytes.saturating_add(bytes) > self.limits.heap.retained_values {
            return Err(Fault::new("MutationObserver attribute-filter retained limit reached."));
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
        registrations.push(requested);
        observer.targets.push(*target);
        self.mutations.registrations += 1;
        self.mutations.child_list_registrations += usize::from(children);
        self.mutations.attribute_registrations += usize::from(attributes);
        self.mutations.subtree_registrations += usize::from(subtree);
        self.value_bytes += bytes;
        Ok(())
    }

    fn mutation_option(&mut self, options: &Value, name: &str, host: &mut dyn Host) -> Result<Value, Fault> {
        if matches!(options, Value::Undefined | Value::Null) { Ok(Value::Undefined) }
        else { self.read_property(options, name, host) }
    }

    fn attribute_filter_strings(&mut self, input: Value, host: &mut dyn Host) -> Result<Vec<String>, Fault> {
        if !matches!(input, Value::Object(_) | Value::Function(_) | Value::Host(_)) {
            return Err(Fault::exception("TypeError", "MutationObserver attributeFilter requires an iterable object."));
        }
        let Some(method) = self.iterator_method(&input, host)? else {
            return Err(Fault::exception("TypeError", "MutationObserver attributeFilter is not iterable."));
        };
        let mut iterator = self.iterator_from_method(input, method, host)?;
        let mut strings = Vec::new();
        let mut bytes = 0usize;
        while let Some(value) = self.iterator_next(&mut iterator, host)? {
            let converted = (|| {
                if strings.len() >= self.limits.capacity.arguments {
                    return Err(Fault::new("MutationObserver attribute-filter item limit reached."));
                }
                let value = self.primitive_with_hint(value, true, host)?;
                self.work(value.bytes())?;
                let text = value.script_text()?;
                bytes = bytes.saturating_add(text.capacity()).saturating_add(std::mem::size_of::<String>());
                if bytes > self.limits.heap.retained_values {
                    return Err(Fault::new("MutationObserver attribute-filter retained limit reached."));
                }
                strings.try_reserve(1).map_err(|_| Fault::new("Cannot reserve MutationObserver attribute filter."))?;
                Ok(text)
            })();
            match converted {
                Ok(text) => strings.push(text),
                Err(error) => return self.iterator_close(&mut iterator, Err(error), host),
            }
        }
        Ok(strings)
    }

    fn matching_observers(&mut self, target: u32, child_list: bool,
        attribute: Option<&NativeAttributeChange>, host: &dyn Host)
        -> Result<BTreeMap<usize, bool>, Fault>
    {
        let mut interested = BTreeMap::new();
        let mut current = Some(target);
        while let Some(node) = current {
            let count=self.mutations.registration_count_at(node);
            let passes=if attribute.is_some_and(|change|change.namespace.is_none()){2}else{1};
            self.work(count.saturating_add(self.mutations.transient_lookup_work(node)).saturating_mul(passes).saturating_add(1))?;
            let filter_work=if let Some(change)=attribute.filter(|change|change.namespace.is_none()){
                self.mutations.registrations_at(node).filter(|reg|node==target||reg.subtree).filter_map(|reg|reg.attribute_filter.as_ref()).map(|filter|filter.len().saturating_mul(change.local_name.len().saturating_add(1))).fold(0usize,usize::saturating_add)
            }else{0};
            self.work(filter_work)?;
            for registration in self.mutations.registrations_at(node){
                if node!=target&&!registration.subtree{continue;}
                let (matches,old)=if let Some(change)=attribute{
                    let passes=registration.attribute_filter.as_ref().is_none_or(|filter|change.namespace.is_none()&&filter.iter().any(|name|change.local_name==*name));
                    (registration.attributes&&passes,registration.attribute_old_value)
                }else if child_list{(registration.child_list,false)}else{(registration.character_data,registration.old_value)};
                if matches{*interested.entry(registration.observer).or_insert(false)|=old;}
            }
            current = host.mutation_observation_parent(node);
        }
        Ok(interested)
    }

    pub(super) fn prepare_character_data_write(
        &mut self, target: u32, property: &str, host: &mut dyn Host,
    ) -> Result<Option<PreparedChange>, Fault> {
        if self.mutations.targets.is_empty() { return Ok(None); }
        let Some(generation) = host.character_data_write_generation(target, property) else { return Ok(None) };
        let observers = self.matching_observers(target, false, None, host)?;
        if observers.is_empty() { return Ok(None); }
        let old = if observers.values().any(|&old_value| old_value) {
            self.host_get_checked(target, "data", host)?
        } else { Value::Null };
        let mut entries = Vec::new();
        entries.try_reserve_exact(observers.len()).map_err(|_| Fault::new("Cannot reserve mutation records."))?;
        for (observer, old_value) in observers {
            let record = Record { target, old_value: if old_value { self.copy_value(&old)? } else { Value::Null },
                child_list: None, attribute: None, added_nodes: None, removed_nodes: None };
            entries.push((observer, record));
        }
        self.prepare_mutation_records(target, generation, entries)
    }

    pub(super) fn prepare_child_list_write(&mut self, target: u32, method: &str, args: &[Value], host: &mut dyn Host)
        -> Result<Option<PreparedChange>, Fault>
    {
        if !self.mutations.observes_tree_mutations() { return Ok(None); }
        let Some(generation) = host.mutation_tree_generation() else { return Ok(None) };
        let changes = host.mutation_child_list_changes(target, method, args).map_err(Fault::new)?;
        self.prepare_child_list_changes(target,generation,changes,host)
    }

    fn prepare_child_list_changes(&mut self,target:u32,generation:u64,changes:Vec<NativeChildListChange>,host:&mut dyn Host)->Result<Option<PreparedChange>,Fault>{
        if changes.is_empty() { return Ok(None); }
        let transient=self.prepare_transient_changes(&changes,host)?;
        let mut entries = Vec::new();
        for change in changes {
            let observers = self.matching_observers(change.target, true, None, host)?;
            entries.try_reserve(observers.len()).map_err(|_| Fault::new("Cannot reserve child-list records."))?;
            let change = Rc::new(change);
            for (observer, _) in observers {
                entries.push((observer, Record {
                    target: change.target, old_value: Value::Null, child_list: Some(Rc::clone(&change)), attribute: None,
                    added_nodes: None, removed_nodes: None,
                }));
            }
        }
        self.prepare_mutation_effects(target,generation,entries,transient)
    }

    pub(super) fn prepare_setter_observation(&mut self,handle:u32,name:&str,value:&Value,reflected:Option<&str>,host:&mut dyn Host)->Result<(Option<PreparedChange>,bool),Fault>{
        if let Some(attribute)=reflected{return self.prepare_attribute_write(handle,"setAttributeNS",&[Value::Null,Value::String(attribute.into())],host).map(|pending|(pending,true));}
        if self.mutations.attribute_registrations!=0{
            if let Some(generation)=host.mutation_tree_generation(){
                if let Some((target,change))=host.mutation_attribute_setter_change(handle,name).map_err(Fault::new)?{return self.prepare_attribute_change(target,generation,change,host).map(|pending|(pending,true));}
            }
        }
        if self.mutations.observes_tree_mutations(){
            if let Some(target)=host.mutation_content_target(handle,name).map_err(Fault::new)?{
                if self.has_subtree_sources(target,host)?||!self.matching_observers(target,true,None,host)?.is_empty(){
                    if let Some(generation)=host.mutation_tree_generation(){
                        let changes=host.mutation_content_changes(handle,name,value).map_err(Fault::new)?;
                        return self.prepare_child_list_changes(target,generation,changes,host).map(|pending|(pending,true));
                    }
                }
                return Ok((None,true));
            }
        }
        self.prepare_character_data_write(handle,name,host).map(|pending|(pending,false))
    }

    pub(super) fn prepare_attribute_write(&mut self, target: u32, method: &str, args: &[Value], host: &mut dyn Host)
        -> Result<Option<PreparedChange>, Fault>
    {
        if self.mutations.attribute_registrations == 0 { return Ok(None); }
        let Some(generation) = host.mutation_tree_generation() else { return Ok(None); };
        let Some(change) = host.mutation_attribute_change(target, method, args).map_err(Fault::new)? else {
            return Ok(None);
        };
        self.prepare_attribute_change(target,generation,change,host)
    }

    fn prepare_attribute_change(&mut self,target:u32,generation:u64,mut change:NativeAttributeChange,host:&mut dyn Host)->Result<Option<PreparedChange>,Fault>{
        let observers = self.matching_observers(target, false, Some(&change), host)?;
        if observers.is_empty() { return Ok(None); }
        let old = change.old_value.take().unwrap_or(Value::Null);
        let change = Rc::new(change);
        let mut entries = Vec::new();
        entries.try_reserve_exact(observers.len()).map_err(|_| Fault::new("Cannot reserve attribute records."))?;
        for (observer, retain_old) in observers {
            entries.push((observer, Record {
                target, old_value: if retain_old { self.copy_value(&old)? } else { Value::Null },
                child_list: None, attribute: Some(Rc::clone(&change)), added_nodes: None, removed_nodes: None,
            }));
        }
        self.prepare_mutation_records(target, generation, entries)
    }

    pub(super) fn prepare_operation_observation(&mut self,target:u32,method:&str,args:&[Value],host:&mut dyn Host)->Result<Option<PreparedChange>,Fault>{
        if self.mutations.attribute_registrations!=0{
            if let Some(generation)=host.mutation_tree_generation(){
                if let Some((owner,change))=host.mutation_attribute_operation_change(target,method).map_err(Fault::new)?{return self.prepare_attribute_change(owner,generation,change,host);}
            }
        }
        if matches!(method,"setAttribute"|"removeAttribute"|"setAttributeNS"|"removeAttributeNS"|"setAttributeNode"|"setAttributeNodeNS"|"removeAttributeNode"){self.prepare_attribute_write(target,method,args,host)}else{self.prepare_child_list_write(target,method,args,host)}
    }

    fn prepare_mutation_records(&mut self, target: u32, generation: u64, entries: Vec<(usize, Record)>)
        -> Result<Option<PreparedChange>, Fault>
    {
        self.prepare_mutation_effects(target,generation,entries,None)
    }
    fn prepare_mutation_effects(&mut self,target:u32,generation:u64,entries:Vec<(usize,Record)>,transient:Option<TransientPlan>)->Result<Option<PreparedChange>,Fault>{
        if entries.is_empty()&&transient.is_none(){return Ok(None);}
        if self.mutations.pending_records.saturating_add(entries.len()) > MAX_PENDING_RECORDS {
            return Err(Fault::new("MutationObserver pending-record limit reached."));
        }
        let mut counts = BTreeMap::new();
        let mut retained = transient.as_ref().map_or(0,TransientPlan::retained_bytes);
        for (observer, record) in &entries {
            *counts.entry(*observer).or_insert(0) += 1;
            retained = retained.saturating_add(record.retained_bytes());
        }
        if self.value_bytes.saturating_add(retained) > self.limits.heap.retained_values {
            return Err(Fault::new("MutationObserver retained-value limit reached."));
        }
        for (observer, count) in counts {
            self.mutations.observers.get_mut(&observer).unwrap().pending.try_reserve(count)
                .map_err(|_| Fault::new("Cannot reserve observer records."))?;
        }
        self.value_bytes += retained;
        let new_job = !self.mutations.queued;
        if new_job {
            if let Err(error) = self.enqueue_mutation_observers() {
                self.value_bytes -= retained;
                return Err(error);
            }
        }
        Ok(Some(PreparedChange { target, generation, entries, retained, new_job, transient }))
    }

    pub(super) fn finish_character_data_write(
        &mut self, pending: Option<PreparedChange>, property: &str, host: &dyn Host,
    ) -> Result<(), Fault> {
        let Some(pending) = pending else { return Ok(()) };
        let committed = host.character_data_write_generation(pending.target, property)
            .is_some_and(|generation| generation != pending.generation);
        self.finish_mutation_records(pending, committed)
    }

    pub(super) fn finish_tree_observation(
        &mut self, pending: Option<PreparedChange>, host: &dyn Host,
    ) -> Result<(), Fault> {
        let Some(pending) = pending else { return Ok(()) };
        let committed = host.mutation_tree_generation().is_some_and(|generation| generation != pending.generation);
        self.finish_mutation_records(pending, committed)
    }

    fn finish_mutation_records(&mut self, pending: PreparedChange, committed: bool) -> Result<(), Fault> {
        if committed {
            if let Some(transient)=pending.transient{self.commit_transient_changes(transient);}
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
        self.mutations.transients.clear();
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
            "type" => Ok(Value::String(if record.child_list.is_some() { "childList" }
                else if record.attribute.is_some() { "attributes" } else { "characterData" }.into())),
            "attributeName" | "attributeNamespace" => {
                let value=if let Some(change)=record.attribute.as_ref(){if field=="attributeName"{match change.local_name.raw_units(){Some(u)=>Value::StringUnits(u.to_vec()),None=>Value::String(change.local_name.scalar().map_err(Fault::new)?.into())}}else{match change.namespace.as_ref(){None=>Value::Null,Some(uri)=>match uri.raw_units(){Some(u)=>Value::StringUnits(u.to_vec()),None=>Value::String(uri.scalar().map_err(Fault::new)?.into())}}}}else{Value::Null};
                self.work(value.bytes())?;Ok(value)
            }
            "previousSibling" => Ok(record.child_list.as_ref().and_then(|change| change.previous).map_or(Value::Null, Value::Host)),
            "nextSibling" => Ok(record.child_list.as_ref().and_then(|change| change.next).map_or(Value::Null, Value::Host)),
            "target" => Ok(Value::Host(record.target)),
            "oldValue" => {
                self.work(record.old_value.bytes())?;
                Ok(self.mutations.records[&id].old_value.clone())
            }
            "addedNodes" | "removedNodes" => {
                let list = if field == "addedNodes" { record.added_nodes } else { record.removed_nodes };
                let handle = if let Some(handle) = list { handle } else {
                    let nodes = record.child_list.as_ref().map_or(&[][..], |change|
                        if field == "addedNodes" { change.added.as_slice() } else { change.removed.as_slice() });
                    let handle = host.mutation_node_list(nodes).map_err(Fault::new)?;
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
            self.clear_transient_observer(id)?;
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
