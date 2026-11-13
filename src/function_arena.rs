//! Stable function IDs with explicit occupancy and pre-reserved retirement.
use super::{Fault, Function, Runtime, MAX_FUNCTIONS};

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

#[derive(Clone, Copy)]
pub(super) struct Allocation { pub(super) id: usize, appended: bool }

impl Runtime {
    pub(super) fn function_is_occupied(&self, id: usize) -> bool {
        matches!(self.functions.get(id), Some(function) if !matches!(function, Function::Vacant))
    }

    pub(super) fn next_function_slot(&self) -> Result<usize, Fault> {
        if let Some(&id) = self.function_arena.free.last() {
            if matches!(self.functions.get(id), Some(Function::Vacant)) { return Ok(id); }
        } else if self.functions.len() < MAX_FUNCTIONS { return Ok(self.functions.len()); }
        Err(Fault::new("Invalid bounded function-slot reservation."))
    }

    pub(super) fn insert_function_slot(&mut self, function: Function) -> Result<Allocation, Fault> {
        if matches!(function, Function::Vacant) { return Err(Fault::new("Cannot allocate a vacant function.")); }
        if self.function_arena.occupied >= MAX_FUNCTIONS { return Err(self.function_limit_fault("function-slot")); }
        let id = self.next_function_slot()?;
        let appended = id == self.functions.len();
        let realm = match &function { Function::Bound(bound) => self.function_realms[bound.target], _ => self.active_realm };
        if id == self.functions.len() {
            fn reserve<T>(values: &mut Vec<T>) -> Result<(), Fault> {
                if values.len() == values.capacity() {
                    let count = values.capacity().max(8).min(MAX_FUNCTIONS - values.len());
                    values.try_reserve_exact(count).map_err(|_| Fault::new("Unable to allocate bounded function-slot storage."))?;
                }
                Ok(())
            }
            reserve(&mut self.functions)?;
            reserve(&mut self.function_properties)?;
            reserve(&mut self.function_realms)?;
            reserve(&mut self.function_initial_names)?;
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
        self.vacate_function_slot(allocation.id, allocation.appended, false)
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
        self.function_arena.free.try_reserve(count).map_err(|_| Fault::new("Unable to reserve bounded free-function storage."))
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
        assert!(runtime.functions.len() <= MAX_FUNCTIONS);
    }
    fn vacant(higher: bool) -> (Runtime, usize, Option<usize>) {
        let mut runtime = Runtime::new();
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
    fn first_backing_failure_preserves_append_middle_and_trailing_topology_and_legacy_payload_charge() {
        for shape in 0..3 {
            let (mut runtime, expected) = match shape {
                0 => { let runtime = Runtime::new(); let id = runtime.functions.len(); (runtime, id) },
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
            assert_eq!(runtime.value_bytes, before.3 + "retain".len());
            assert_eq!(runtime.next_function_slot().unwrap(), expected);
            if shape != 0 { assert!(matches!(runtime.functions[expected], Function::Vacant)); }
            aligned(&runtime);
            runtime.limits.heap.objects = object_cap;
            assert!(matches!(runtime.native_function(Builtin::IsNaN, "isNaN").unwrap(), Value::Function(id) if id == expected));
            aligned(&runtime);
        }
    }
    #[test]
    fn first_backing_failure_keeps_legacy_bound_capture_charge() {
        let mut runtime = Runtime::new();
        let target = runtime.function_prototype.unwrap();
        let before = (runtime.functions.len(), runtime.function_arena.occupied, runtime.value_bytes);
        runtime.limits.heap.objects = runtime.objects.len();
        assert!(runtime.bind_function(target, &[Value::String("captured".into())], &mut Native).is_err());
        assert_eq!((runtime.functions.len(), runtime.function_arena.occupied), (before.0, before.1));
        assert_eq!(runtime.value_bytes, before.2 + std::mem::size_of::<Value>() + "captured".len());
        aligned(&runtime);
    }
    #[test]
    fn later_property_failure_retains_occupied_script_and_commits_one_monotonic_creation() {
        let mut runtime = Runtime::new();
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
        let mut runtime = Runtime::new();
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
        let mut runtime = Runtime::new();
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

        let mut runtime = Runtime::new();
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
