//! Stable function IDs with explicit occupancy and pre-reserved retirement.
use super::{Fault, Function, Runtime};

#[derive(Default)]
pub(super) struct Arena {
    pub(super) occupied: usize,
    pub(super) script_created: usize,
    pub(super) allocations: usize,
    pub(super) collections: usize,
    pub(super) demand_collections: usize,
    pub(super) retired: usize,
    pub(super) environments_retired: usize,
    pub(super) scope_retired: usize,
    pub(super) next_collection_at: usize,
    #[cfg(test)]
    pub(super) disable_collection: bool,
    #[cfg(test)]
    pub(super) disable_scope_collection: bool,
    free: Vec<usize>,
}

impl Arena {
    pub(super) fn free_is_empty_and_full(&self, length: usize, capacity: usize) -> bool {
        self.free.is_empty() && length == capacity
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Allocation { pub(super) id: usize, appended: bool }

impl Runtime {
    #[cfg(test)]
    pub(super) fn retained_arena_bytes(&self) -> usize {
        fn bytes<T>(values: &Vec<T>) -> usize { values.capacity() * std::mem::size_of::<T>() }
        bytes(&self.functions) + bytes(&self.function_properties) + bytes(&self.function_realms)
            + bytes(&self.function_initial_names) + bytes(&self.function_arena.free)
            + bytes(&self.environments) + bytes(&self.free_environments) + bytes(&self.deferred_native_methods)
    }

    pub(super) fn function_count_refused(&self, additional: usize) -> bool {
        #[cfg(test)]
        if self.limits.test_entry_arenas {
            return additional > self.limits.capacity.functions.saturating_sub(self.function_arena.occupied);
        }
        self.function_arena.occupied.checked_add(additional).is_none()
    }

    pub(super) fn prepare_function_slot_storage(&mut self) -> Result<(), Fault> {
        if !self.function_arena.free.is_empty() { return Ok(()); }
        let available = self.limits.heap.retained_values.saturating_sub(self.value_bytes);
        let before = self.functions.capacity();
        let slot = std::mem::size_of::<Function>() + 2 * std::mem::size_of::<usize>()
            + std::mem::size_of::<Option<usize>>() + std::mem::size_of::<Option<String>>();
        let capacity = if self.functions.len() < before { before } else {
            let growth = before.max(4).min(available / slot);
            if growth == 0 { return Err(Fault::new("Retained value arena storage exhausted.")); }
            before.checked_add(growth).ok_or_else(|| Fault::new("Function arena capacity overflow."))?
        };
        let functions = super::limits::prepare_retained_capacity(&self.functions, capacity, available)?;
        let properties = super::limits::prepare_retained_capacity(&self.function_properties, capacity, available - functions.added)?;
        let realms = super::limits::prepare_retained_capacity(&self.function_realms, capacity, available - functions.added - properties.added)?;
        let names = super::limits::prepare_retained_capacity(&self.function_initial_names, capacity, available - functions.added - properties.added - realms.added)?;
        let used = functions.added + properties.added + realms.added + names.added;
        let free = super::limits::prepare_retained_capacity(&self.function_arena.free, functions.capacity, available - used)?;
        let used = used + free.added;
        if used != 0 { self.work(self.functions.len().saturating_mul(5))?; }
        functions.commit(&mut self.functions);
        properties.commit(&mut self.function_properties);
        realms.commit(&mut self.function_realms);
        names.commit(&mut self.function_initial_names);
        free.commit(&mut self.function_arena.free);
        self.value_bytes += used;
        Ok(())
    }

    pub(super) fn prepare_environment_slot_storage(&mut self) -> Result<(), Fault> {
        if !self.free_environments.is_empty() { return Ok(()); }
        if self.environment_count_refused() { return Err(self.environment_limit_fault("Environment", self.limits.capacity.environments)); }
        let available = self.limits.heap.retained_values.saturating_sub(self.value_bytes);
        let before = self.environments.capacity();
        let slot = std::mem::size_of::<Option<super::Environment>>() + std::mem::size_of::<usize>();
        let capacity = if self.environments.len() < before { before } else {
            let growth = before.max(4).min(available / slot);
            if growth == 0 { return Err(Fault::new("Retained value arena storage exhausted.")); }
            before.checked_add(growth).ok_or_else(|| Fault::new("Environment arena capacity overflow."))?
        };
        let environments = super::limits::prepare_retained_capacity(&self.environments, capacity, available)?;
        // The permanent main global (ID zero) is never a reusable slot.
        let free = super::limits::prepare_retained_capacity(&self.free_environments, environments.capacity.saturating_sub(1), available - environments.added)?;
        let used = environments.added + free.added;
        if used != 0 { self.work(self.environments.len().saturating_mul(2))?; }
        environments.commit(&mut self.environments);
        free.commit(&mut self.free_environments);
        self.value_bytes += used;
        Ok(())
    }

    pub(super) fn reserve_environment_retirements(&mut self, count: usize) -> Result<(), Fault> {
        let growth = super::limits::prepare_retained_growth(&self.free_environments, count,
            self.limits.heap.retained_values.saturating_sub(self.value_bytes))?;
        self.value_bytes += growth.added;
        growth.commit(&mut self.free_environments);
        Ok(())
    }

    pub(super) fn environment_count_refused(&self) -> bool {
        #[cfg(test)]
        if self.limits.test_entry_arenas { return self.environments.len() >= self.limits.capacity.environments; }
        self.environments.len() == usize::MAX
    }

    pub(super) fn function_is_occupied(&self, id: usize) -> bool {
        matches!(self.functions.get(id), Some(function) if !matches!(function, Function::Vacant))
    }

    pub(super) fn next_function_slot(&self) -> Result<usize, Fault> {
        if let Some(&id) = self.function_arena.free.last() {
            if matches!(self.functions.get(id), Some(Function::Vacant)) { return Ok(id); }
        } else if !self.function_count_refused(1) && self.functions.len() < usize::MAX { return Ok(self.functions.len()); }
        Err(Fault::new("Invalid bounded function-slot reservation."))
    }

    pub(super) fn insert_function_slot(&mut self, function: Function) -> Result<Allocation, Fault> {
        if matches!(function, Function::Vacant) { return Err(Fault::new("Cannot allocate a vacant function.")); }
        if self.function_count_refused(1) { return Err(self.function_limit_fault("function-slot")); }
        let id = self.next_function_slot()?;
        let appended = id == self.functions.len();
        let realm = match &function { Function::Bound(bound) => self.function_realms[bound.target], _ => self.active_realm };
        if id == self.functions.len() {
            self.prepare_function_slot_storage()?;
            self.functions.push(function);
            self.function_properties.push(usize::MAX);
            self.function_realms.push(realm);
            self.function_initial_names.push(None);
        } else {
            self.function_arena.free.pop();
            self.functions[id] = function;
            self.function_properties[id] = usize::MAX;
            self.function_realms[id] = realm;
            self.function_initial_names[id] = None;
        }
        self.function_arena.occupied += 1;
        self.function_arena.allocations = self.function_arena.allocations.saturating_add(1);
        Ok(Allocation { id, appended })
    }

    pub(super) fn rollback_unpublished_function(&mut self, allocation: Allocation) -> Result<(), Fault> {
        let payload = match &self.functions[allocation.id] {
            Function::Script { lexical_this, .. } => lexical_this.as_ref().map_or(0, super::Value::bytes),
            Function::Bound(bound) => bound.retained_bytes(),
            Function::Promise(closure) => closure.retained_bytes(),
            _ => 0,
        };
        self.vacate_function_slot(allocation.id, allocation.appended, false)?;
        // Backing vector capacity remains owned; these dropped captures do not.
        self.value_bytes -= payload;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn retire_unreachable_script(&mut self, id: usize) -> Result<(), Fault> {
        if !matches!(self.functions.get(id), Some(Function::Script { .. }))
            || !self.function_is_retirement_candidate(id) || self.exported_function_roots.contains(&id) {
            return Err(Fault::new("Function is outside the unexported closure candidate class."));
        }
        // Tests supply a completed inactive graph proving unreachability.
        self.vacate_function_slot(id, false, true)
    }

    pub(super) fn reserve_function_retirements(&mut self, count: usize) -> Result<(), Fault> {
        let growth = super::limits::prepare_retained_growth(&self.function_arena.free, count,
            self.limits.heap.retained_values.saturating_sub(self.value_bytes))?;
        self.value_bytes += growth.added;
        growth.commit(&mut self.function_arena.free);
        Ok(())
    }

    fn vacate_function_slot(&mut self, id: usize, appended_rollback: bool, refund_script: bool) -> Result<(), Fault> {
        if !self.function_is_occupied(id) || self.exported_function_roots.contains(&id) {
            return Err(Fault::new("Cannot retire an invalid or exported function slot."));
        }
        let truncate = appended_rollback && id + 1 == self.functions.len();
        if !truncate { self.reserve_function_retirements(1)?; }
        self.vacate_function_slot_prepared(id, truncate, refund_script);
        Ok(())
    }

    // Caller has validated the complete transaction, refunds and export pins,
    // and reserved every free ID. This commit path cannot allocate or fail.
    pub(super) fn vacate_function_slot_prepared(&mut self, id: usize, truncate: bool, refund_script: bool) {
        if refund_script {
            if let Function::Script { lexical_this: Some(value), .. } = &self.functions[id] { self.value_bytes -= value.bytes(); }
        }
        if let Some(name) = self.function_initial_names[id].take() { self.value_bytes -= name.len(); }
        self.functions[id] = Function::Vacant;
        self.function_properties[id] = usize::MAX;
        self.function_realms[id] = None;
        self.object_assigned_closures.remove(&id);
        self.scope_assigned_closures.remove(&id);
        self.function_arena.occupied -= 1;
        if truncate {
            self.functions.pop(); self.function_properties.pop(); self.function_realms.pop(); self.function_initial_names.pop();
        } else { self.function_arena.free.push(id); }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    struct Native;
    impl Host for Native {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
        fn call(&mut self, object: u32, method: &str, _: &[Value]) -> Result<Value, String> {
            assert_eq!(object, 7);
            match method { "checked" => Ok(Value::Number(7.0)), "construct" => Ok(Value::Host(7)), _ => panic!("unexpected call") }
        }
    }
    fn aligned(runtime: &Runtime) {
        assert_eq!(runtime.functions.len(), runtime.function_properties.len());
        assert_eq!(runtime.functions.len(), runtime.function_realms.len());
        assert_eq!(runtime.functions.len(), runtime.function_initial_names.len());
        assert_eq!(runtime.function_arena.occupied, runtime.functions.iter().filter(|function| !matches!(function, Function::Vacant)).count());
        if runtime.limits.test_entry_arenas { assert!(runtime.functions.len() <= runtime.limits.capacity.functions); }
    }
    fn vacant(higher: bool) -> (Runtime, usize, Option<usize>) {
        let mut runtime = Runtime::for_capacity_test();
        runtime.execute("let blocked;var holder={};holder.fn=function gone(){return 3;};holder.fn=null;0;", &mut Native).unwrap();
        let id = *runtime.object_assigned_closures.iter().next().unwrap();
        let higher = if higher {
            runtime.execute("var top=function kept(){return 17;};0;", &mut Native).unwrap();
            let Value::Function(higher) = runtime.binding_value(0, "top").unwrap() else { unreachable!() };
            assert!(higher > id);
            Some(higher)
        } else { None };
        runtime.retire_proven_inactive_candidate(id).unwrap();
        assert!(!runtime.function_is_occupied(id));
        assert!(!runtime.object_assigned_closures.contains(&id));
        aligned(&runtime);
        (runtime, id, higher)
    }
    #[test]
    fn parallel_function_storage_refusal_preserves_capacities_and_reuses_retired_ids() {
        let mut runtime = Runtime::new();
        while runtime.functions.len() < runtime.functions.capacity() {
            runtime.insert_function_slot(Function::Native(Builtin::IsNaN)).unwrap();
        }
        let snapshot = |runtime: &Runtime| (runtime.functions.len(), runtime.functions.capacity(),
            runtime.function_properties.capacity(), runtime.function_realms.capacity(),
            runtime.function_initial_names.capacity(), runtime.function_arena.free.capacity(), runtime.value_bytes);
        let before = snapshot(&runtime);
        runtime.limits.heap.retained_values = runtime.value_bytes + std::mem::size_of::<Function>();
        assert!(runtime.insert_function_slot(Function::Native(Builtin::IsNaN)).unwrap_err().display().contains("arena storage exhausted"));
        assert_eq!(snapshot(&runtime), before);
        runtime.limits.heap.retained_values = runtime.value_bytes + std::mem::size_of::<Function>() + 2 * std::mem::size_of::<usize>()
            + std::mem::size_of::<Option<usize>>() + std::mem::size_of::<Option<String>>();
        let slot = runtime.insert_function_slot(Function::Native(Builtin::IsNaN)).unwrap();
        let retained = runtime.value_bytes;
        runtime.vacate_function_slot(slot.id, false, false).unwrap();
        runtime.limits.heap.retained_values = retained;
        let reused = runtime.insert_function_slot(Function::Native(Builtin::IsNaN)).unwrap();
        assert_eq!(reused.id, slot.id);
        assert_eq!(runtime.value_bytes, retained);
        aligned(&runtime);
    }

    #[test]
    fn parallel_environment_storage_refusal_preserves_capacities_and_reuses_completed_ids() {
        let mut runtime = Runtime::new();
        while runtime.environments.len() < runtime.environments.capacity() || !runtime.free_environments.is_empty() {
            runtime.new_environment(0, false).unwrap();
        }
        let before = (runtime.environments.len(), runtime.environments.capacity(), runtime.free_environments.capacity(), runtime.value_bytes);
        runtime.limits.heap.retained_values = runtime.value_bytes + std::mem::size_of::<Option<Environment>>();
        assert!(runtime.new_environment(0, false).unwrap_err().display().contains("arena storage exhausted"));
        assert_eq!((runtime.environments.len(), runtime.environments.capacity(), runtime.free_environments.capacity(), runtime.value_bytes), before);
        runtime.limits.heap.retained_values = runtime.value_bytes + std::mem::size_of::<Option<Environment>>() + std::mem::size_of::<usize>();
        let id = runtime.new_environment(0, false).unwrap();
        let retained = runtime.value_bytes;
        runtime.release_environment(id);
        runtime.limits.heap.retained_values = retained;
        assert_eq!(runtime.new_environment(0, false).unwrap(), id);
        assert_eq!(runtime.value_bytes, retained);
    }

    #[test]
    fn browser_retained_closures_exceed_former_function_and_environment_counts() {
        let mut runtime = Runtime::new();
        let result = runtime.execute(r#"
            var saved=[];
            function factory(value){return function(){return value;};}
            for(var index=0;index<70000;index++){saved.push(factory(index));}
            if(saved.length!==70000||saved[0]()!==0||saved[65536]()!==65536||saved[69999]()!==69999)throw 'retained closure';
            'ok';
        "#, &mut Native).unwrap();
        assert!(matches!(result, Value::String(ref value) if value=="ok"));
        assert!(runtime.function_arena.occupied > 65536);
        assert!(runtime.environments.iter().flatten().count() > 65536);
        let host = runtime.host_function(7, "checked").unwrap();
        assert_eq!(runtime.invoke(host, &[], Value::Undefined, &mut Native).unwrap().to_number(), 7.0);
        aligned(&runtime);
    }

    #[test]
    fn reused_middle_host_method_and_constructor_keep_exact_id_and_higher_live_function() {
        for constructor in [false, true] {
            let (mut runtime, id, higher) = vacant(true);
            let higher = higher.unwrap();
            let before = (runtime.functions.len(), runtime.function_properties[higher], runtime.function_realms[higher]);
            let function = if constructor { runtime.define_host_constructor("Ctor", 7, "construct").unwrap() }
                else { runtime.host_method(7, "checked").unwrap() };
            assert!(matches!(function, Value::Function(found) if found == id));
            assert_eq!((runtime.functions.len(), runtime.function_properties[higher], runtime.function_realms[higher]), before);
            if constructor {
                assert!(matches!(runtime.execute("new Ctor();", &mut Native).unwrap(), Value::Host(7)));
                let properties = runtime.function_properties[id];
                let Value::Object(prototype) = runtime.own_property(properties, "prototype").unwrap() else { unreachable!() };
                assert!(matches!(runtime.own_property(prototype, "constructor").unwrap(), Value::Function(found) if found == id));
            } else {
                assert_eq!(runtime.invoke(function.clone(), &[], Value::Host(7), &mut Native).unwrap().to_number(), 7.0);
                assert!(runtime.invoke(function, &[], Value::Undefined, &mut Native).unwrap_err().contains("incompatible receiver"));
            }
            runtime.begin_job();
            assert_eq!(runtime.execute("top();", &mut Native).unwrap().to_number(), 17.0);
            aligned(&runtime);
        }
    }
    #[test]
    fn reused_bound_slot_targets_the_correct_higher_function_and_keeps_source_metadata() {
        let (mut runtime, id, higher) = vacant(true);
        let higher = higher.unwrap();
        let before = runtime.functions.len();
        let bound = runtime.bind_function(higher, &[Value::Null], &mut Native).unwrap();
        assert!(matches!(bound, Value::Function(found) if found == id));
        assert_eq!(runtime.functions.len(), before);
        assert_eq!(runtime.function_realms[id], runtime.function_realms[higher]);
        assert_eq!(runtime.invoke(bound, &[], Value::Undefined, &mut Native).unwrap().to_number(), 17.0);
        assert_eq!(runtime.function_source(id).unwrap().to_text(), "function () { [native code] }");
        aligned(&runtime);
    }
    #[test]
    fn reused_host_definition_failure_restores_vacancy_pins_and_metadata_without_popping_higher_id() {
        let (mut runtime, id, higher) = vacant(true);
        let higher = higher.unwrap();
        let before = (runtime.functions.len(), runtime.function_arena.occupied, runtime.function_properties[higher]);
        assert!(runtime.define_host_function("bad-name", 7, "checked").is_err());
        assert_eq!((runtime.functions.len(), runtime.function_arena.occupied, runtime.function_properties[higher]), before);
        assert!(!runtime.function_is_occupied(id));
        assert!(!runtime.exported_function_roots.contains(&id));
        assert!(runtime.function_initial_names[id].is_none());
        assert_eq!(runtime.function_properties[id], usize::MAX);
        assert_eq!(runtime.next_function_slot().unwrap(), id);
        assert!(matches!(runtime.native_function(Builtin::IsNaN, "isNaN").unwrap(), Value::Function(found) if found == id));
        assert_eq!(runtime.execute("top();", &mut Native).unwrap().to_number(), 17.0);
        aligned(&runtime);
    }
    #[test]
    fn first_backing_failure_preserves_append_middle_and_trailing_topology_and_refunds_payload() {
        for shape in 0..3 {
            let (mut runtime, expected) = match shape {
                0 => { let runtime = Runtime::for_capacity_test(); let id = runtime.functions.len(); (runtime, id) },
                1 => { let (runtime, id, _) = vacant(true); (runtime, id) },
                _ => { let (runtime, id, _) = vacant(false); (runtime, id) },
            };
            let program = runtime.register_program("(()=>this);", false).unwrap();
            let StmtKind::Expr(expression) = program.stmts[program.body[0]].kind else { unreachable!() };
            let ExprKind::Function(template) = program.exprs[expression].kind else { unreachable!() };
            let before = (runtime.functions.len(), runtime.function_arena.occupied, runtime.function_arena.script_created, runtime.value_bytes);
            let object_cap = runtime.limits.heap.objects;
            runtime.limits.heap.objects = runtime.objects.len();
            assert!(runtime.make_function(&program, template, 0, &Value::String("retain".into())).is_err());
            assert_eq!((runtime.functions.len(), runtime.function_arena.occupied, runtime.function_arena.script_created), (before.0, before.1, before.2));
            assert_eq!(runtime.value_bytes, before.3);
            assert_eq!(runtime.next_function_slot().unwrap(), expected);
            if shape != 0 { assert!(matches!(runtime.functions[expected], Function::Vacant)); }
            aligned(&runtime);
            runtime.limits.heap.objects = object_cap;
            assert!(matches!(runtime.native_function(Builtin::IsNaN, "isNaN").unwrap(), Value::Function(id) if id == expected));
            aligned(&runtime);
        }
    }
    #[test]
    fn first_backing_failure_refunds_unpublished_bound_capture() {
        let mut runtime = Runtime::for_capacity_test();
        let target = runtime.function_prototype.unwrap();
        let before = (runtime.functions.len(), runtime.function_arena.occupied, runtime.value_bytes);
        runtime.limits.heap.objects = runtime.objects.len();
        assert!(runtime.bind_function(target, &[Value::String("captured".into())], &mut Native).is_err());
        assert_eq!((runtime.functions.len(), runtime.function_arena.occupied), (before.0, before.1));
        assert_eq!(runtime.value_bytes, before.2);
        aligned(&runtime);
    }
    #[test]
    fn later_property_failure_retains_occupied_script_and_commits_one_monotonic_creation() {
        let mut runtime = Runtime::for_capacity_test();
        let program = runtime.register_program("(()=>0);", false).unwrap();
        let StmtKind::Expr(expression) = program.stmts[program.body[0]].kind else { unreachable!() };
        let ExprKind::Function(template) = program.exprs[expression].kind else { unreachable!() };
        let before = (runtime.functions.len(), runtime.function_arena.occupied, runtime.function_arena.script_created);
        runtime.properties = runtime.limits.heap.properties;
        assert!(runtime.make_function(&program, template, 0, &Value::Undefined).is_err());
        assert_eq!((runtime.functions.len(), runtime.function_arena.occupied, runtime.function_arena.script_created), (before.0 + 1, before.1 + 1, before.2 + 1));
        assert!(runtime.function_is_occupied(before.0));
        assert!(runtime.function_properties[before.0] < runtime.objects.len());
        aligned(&runtime);
    }
    #[test]
    fn proven_retirement_refunds_only_the_captured_arrow_string_without_resetting_work_or_time() {
        let mut runtime = Runtime::for_capacity_test();
        runtime.execute("var holder={};function factory(){'use strict';holder.fn=()=>this;}factory.call('é𐀀');holder.fn=null;0;", &mut Native).unwrap();
        let id = *runtime.object_assigned_closures.iter().next().unwrap();
        let before = (runtime.value_bytes, runtime.job_usage(), runtime.job_deadline, runtime.function_arena.script_created, runtime.objects.len(), runtime.bindings);
        runtime.retire_proven_inactive_candidate(id).unwrap();
        assert_eq!(runtime.value_bytes, before.0 - "é𐀀".len());
        assert_eq!((runtime.job_usage(), runtime.job_deadline, runtime.function_arena.script_created, runtime.objects.len(), runtime.bindings), (before.1, before.2, before.3, before.4, before.5));
        aligned(&runtime);
    }
    #[test]
    fn occupied_cap_and_script_creation_counter_remain_correct_after_reusing_high_water_slots() {
        let mut runtime = Runtime::for_capacity_test();
        runtime.execute("var holder={};holder.fn=function gone(){};holder.fn=null;0;", &mut Native).unwrap();
        let id = *runtime.object_assigned_closures.iter().next().unwrap();
        while runtime.function_arena.occupied < MAX_FUNCTIONS { runtime.native_function(Builtin::IsNaN, "fixture").unwrap(); }
        assert_eq!(runtime.functions.len(), MAX_FUNCTIONS);
        let created = runtime.function_arena.script_created;
        runtime.retire_proven_inactive_candidate(id).unwrap();
        assert_eq!(runtime.function_arena.occupied, MAX_FUNCTIONS - 1);
        assert_eq!(runtime.functions.len(), MAX_FUNCTIONS);
        assert!(matches!(runtime.native_function(Builtin::IsNaN, "replacement").unwrap(), Value::Function(found) if found == id));
        assert_eq!(runtime.function_arena.script_created, created);
        let before = (runtime.objects.len(), runtime.job_usage(), runtime.job_deadline);
        assert!(runtime.native_function(Builtin::IsNaN, "overflow").unwrap_err().message.contains("Function limit reached (1024)"));
        assert_eq!((runtime.objects.len(), runtime.job_usage(), runtime.job_deadline), before);
        aligned(&runtime);

        let mut runtime = Runtime::for_capacity_test();
        let before = runtime.function_arena.script_created;
        let mut high_water = None;
        for _ in 0..8 {
            runtime.execute("var holder={};holder.fn=function gone(n){return n?gone(n-1):17;};if(holder.fn(3)!==17)throw 'private reused identity';holder.fn=null;0;", &mut Native).unwrap();
            let id = *runtime.object_assigned_closures.iter().next().unwrap();
            if let Some(count) = high_water { assert_eq!(runtime.functions.len(), count); } else { high_water = Some(runtime.functions.len()); }
            runtime.retire_proven_inactive_candidate(id).unwrap();
            aligned(&runtime);
        }
        assert_eq!(runtime.function_arena.script_created, before + 8);
    }
    #[test]
    fn failed_realm_bootstrap_repairs_owner_of_reused_id_below_old_append_boundary() {
        let (mut runtime, id, higher) = vacant(true);
        let higher = higher.unwrap();
        let before = (runtime.functions.len(), runtime.function_properties[higher], runtime.function_realms[higher]);
        let cap = runtime.limits.heap.objects;
        runtime.limits.heap.objects = runtime.objects.len() + 2;
        assert!(runtime.allocate_intrinsic_realm().is_err());
        assert!(runtime.intrinsic_realms.is_empty());
        assert!(matches!(runtime.functions[id], Function::Native(_)));
        assert_eq!(runtime.function_realms[id], None);
        assert_eq!((runtime.functions.len(), runtime.function_properties[higher], runtime.function_realms[higher]), before);
        assert!(runtime.function_realms.iter().all(Option::is_none));
        aligned(&runtime);
        runtime.limits.heap.objects = cap;
        runtime.begin_job();
        assert_eq!(runtime.execute("top();", &mut Native).unwrap().to_number(), 17.0);
    }
}
