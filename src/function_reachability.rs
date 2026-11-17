//! Checked persistent/transient roots and conservative Script-entry reclamation.
//! Objects remain append-only; only proven dead candidate functions/environments retire.
use super::*;

#[cfg(test)]
#[path = "function_array_tests.rs"]
mod array_seed_tests;

#[cfg(test)]
#[path = "scope_candidate_tests.rs"]
mod scope_candidate_tests;

#[cfg(test)]
#[path = "scope_collection_tests.rs"]
mod scope_collection_tests;
#[cfg(test)]
#[path = "new_target_lifetime_tests.rs"]
mod new_target_lifetime_tests;
#[cfg(test)]
#[path = "base_class_lifetime_tests.rs"]
mod base_class_lifetime_tests;

#[derive(Clone, Copy, Debug)]
pub(super) enum Edge { Object(usize), Function(usize), Environment(usize) }

impl Edge {
    pub(super) fn value(value: &Value, add: &mut impl FnMut(Edge)) {
        match value { Value::Object(id) => add(Self::Object(*id)), Value::Function(id) => add(Self::Function(*id)), _ => {} }
    }

    pub(super) fn reference(reference: &Reference, add: &mut impl FnMut(Edge)) {
        match reference {
            Reference::Binding(id, _) => add(Self::Environment(*id)),
            Reference::Property(receiver, _) | Reference::SymbolProperty(receiver, _) => Self::value(receiver, add),
            Reference::ComputedProperty(receiver, key) => { Self::value(receiver, add); Self::value(key, add); }
            Reference::Missing(_) | Reference::Global(_) => {}
        }
    }
}

struct Graph {
    objects: Vec<bool>,
    functions: Vec<bool>,
    occupied_functions: Vec<bool>,
    environments: Vec<bool>,
    pending: Vec<Edge>,
    invalid: bool,
}

// Charge ID/slot traversal to the existing step/deadline allowance, without
// copying script payloads or spending/resetting the string-copy allowance.
struct Meter { remaining: Option<usize>, deadline: Instant }
impl Meter {
    fn charge(&mut self, count: usize) -> Result<(), &'static str> {
        let Some(remaining) = &mut self.remaining else { return Ok(()); };
        for _ in 0..count {
            if *remaining == 0 { return Err("Execution step limit reached during reachability."); }
            if *remaining & 255 == 0 && Instant::now() >= self.deadline {
                *remaining = 0;
                return Err("Execution time limit reached during reachability.");
            }
            *remaining -= 1;
        }
        Ok(())
    }
}

impl Graph {
    fn new(runtime: &Runtime) -> Result<Self, &'static str> {
        fn bitmap(count: usize) -> Result<Vec<bool>, &'static str> {
            let mut bits = Vec::new();
            bits.try_reserve_exact(count).map_err(|_| "Unable to allocate bounded reachability scratch.")?;
            bits.resize(count, false);
            Ok(bits)
        }
        let mut pending = Vec::new();
        pending.try_reserve_exact(runtime.objects.len().saturating_add(runtime.functions.len()).saturating_add(runtime.environments.len()))
            .map_err(|_| "Unable to allocate bounded reachability scratch.")?;
        let mut occupied_functions = bitmap(runtime.functions.len())?;
        for (id, function) in runtime.functions.iter().enumerate() { occupied_functions[id] = !matches!(function, Function::Vacant); }
        Ok(Self { objects: bitmap(runtime.objects.len())?, functions: bitmap(runtime.functions.len())?, occupied_functions,
            environments: bitmap(runtime.environments.len())?, pending, invalid: false })
    }

    fn add(&mut self, edge: Edge) {
        if let Edge::Function(id) = edge {
            if !self.occupied_functions.get(id).copied().unwrap_or(false) { self.invalid = true; return; }
        }
        let slot = match edge {
            Edge::Object(id) => self.objects.get_mut(id),
            Edge::Function(id) => self.functions.get_mut(id),
            Edge::Environment(id) => self.environments.get_mut(id),
        };
        let Some(slot) = slot else { self.invalid = true; return; };
        if !*slot { *slot = true; self.pending.push(edge); }
    }
}

impl Runtime {
    pub(super) fn scope_retirement_enabled(&self) -> bool {
        #[cfg(test)]
        { !self.function_arena.disable_scope_collection }
        #[cfg(not(test))]
        { true }
    }

    pub(super) fn function_is_retirement_candidate(&self, id: usize) -> bool {
        self.object_assigned_closures.contains(&id)
            || self.scope_retirement_enabled() && self.scope_assigned_closures.contains(&id)
    }

    fn active_roots_are_audited(&self) -> bool {
        let state = &self.transient_roots;
        #[cfg(test)]
        if self.recursive_evaluation_oracle { return false; }
        state.inhibited == 0 && self.call_depth == state.owned_call_depth && self.eval_depth == state.owned_eval_depth
            && self.evaluation_frames == state.evaluation_frames && self.evaluation_frame_slots == state.frame_slots
            && self.evaluation_argument_slots == state.argument_slots && self.active_array_joins.is_empty()
            && self.promises.reachability_job_is_registered(state.active_job_registrations)
    }

    #[inline(never)]
    pub(super) fn collect_function_candidates_at_entry(&mut self) -> Result<(), Fault> {
        self.collect_function_candidates(false)
    }

    #[inline(never)]
    pub(super) fn collect_before_function_literal(&mut self) -> Result<(), Fault> {
        #[cfg(test)]
        if self.transient_roots.disable_function_literal_collection { return Ok(()); }
        self.collect_function_candidates(true)
    }

    #[inline(never)]
    fn collect_function_candidates(&mut self, allocation_demand: bool) -> Result<(), Fault> {
        #[cfg(test)]
        if self.function_arena.disable_collection { return Ok(()); }
        let include_scope = self.scope_retirement_enabled();
        if self.object_assigned_closures.is_empty() && (!include_scope || self.scope_assigned_closures.is_empty())
            || self.function_arena.occupied < self.limits.capacity.functions * 3 / 4
            || self.function_arena.allocations < self.function_arena.next_collection_at
                && !(allocation_demand && self.function_arena.occupied == self.limits.capacity.functions)
            || !self.active_roots_are_audited() {
            return Ok(());
        }
        let mut meter = Meter { remaining: Some(self.steps), deadline: self.job_deadline };
        // Production MUST include permanent API/Host export pins. The test-only
        // pin-disabled audit below exists to prove transient edges independently.
        let traced = self.trace_root_graph_candidate_policy(&[], true, true, true, false, include_scope, &mut meter);
        self.steps = meter.remaining.unwrap();
        let graph = traced.map_err(Fault::new)?;
        let prepared = (|| {
          // Prepay each preflight/commit scan before inspecting its contents.
          // Failure paths retain this spent work, even if a later reserve fails.
          meter.charge((self.functions.len() + self.environments.len()) * 2).map_err(Fault::new)?;
          let mut functions = 0usize;
          let mut environments = 0usize;
          let mut scope_functions = 0usize;
          let mut bindings = 0usize;
          let mut bytes = 0usize;
          for (id, function) in self.functions.iter().enumerate() {
            if matches!(function, Function::Vacant) { continue; }
            if !graph.functions[id] {
                let Function::Script { lexical_this, .. } = function else { return Err(Fault::new("Unmarked non-Script function in retirement transaction.")); };
                if !self.function_is_retirement_candidate(id) || self.exported_function_roots.contains(&id) {
                    return Err(Fault::new("Unmarked function outside retirement candidate class."));
                }
                functions += 1;
                scope_functions += usize::from(self.scope_assigned_closures.contains(&id));
                bytes = bytes.checked_add(lexical_this.as_ref().map_or(0, Value::bytes))
                    .and_then(|bytes| bytes.checked_add(self.function_initial_names[id].as_ref().map_or(0, String::len)))
                    .ok_or_else(|| Fault::new("Invalid function retirement accounting."))?;
            } else if let Function::Script { environment, .. } = function {
                if graph.environments.get(*environment).copied() != Some(true) {
                    return Err(Fault::new("Retained function has an unmarked capture environment."));
                }
            }
          }
          for (id, environment) in self.environments.iter().enumerate() {
            if graph.environments[id] { continue; }
            if let Some(environment) = environment {
                meter.charge((environment.bindings.len() + environment.arguments.as_ref().map_or(0, arguments::Pending::reachability_cost)) * 2).map_err(Fault::new)?;
                environments += 1;
                bindings = bindings.checked_add(environment.bindings.len()).ok_or_else(|| Fault::new("Invalid environment retirement accounting."))?;
                let retained = environment.bindings.values().filter_map(|binding| binding.value.as_ref()).map(Value::bytes)
                    .chain(environment.arguments.as_ref().map(arguments::Pending::bytes));
                for retained in retained { bytes = bytes.checked_add(retained).ok_or_else(|| Fault::new("Invalid environment retirement accounting."))?; }
            }
          }
          if bindings > self.bindings || bytes > self.value_bytes { return Err(Fault::new("Retirement refunds exceed retained payload accounting.")); }
          Ok((functions, environments, scope_functions))
        })();
        self.steps = meter.remaining.unwrap();
        let (functions, environments, scope_functions) = prepared?;
        self.reserve_function_retirements(functions)?;
        self.free_environments.try_reserve(environments).map_err(|_| Fault::new("Unable to reserve bounded free-environment storage."))?;
        if Instant::now() >= self.job_deadline {
            self.steps = 0;
            return Err(Fault::new("Execution time limit reached during reachability."));
        }
        for id in 0..self.functions.len() {
            if self.function_is_occupied(id) && !graph.functions[id] { self.vacate_function_slot_prepared(id, false, true); }
        }
        for id in 0..self.environments.len() {
            if !graph.environments[id] { self.discard_environment(id); }
        }
        self.function_arena.collections = self.function_arena.collections.saturating_add(1);
        if allocation_demand { self.function_arena.demand_collections = self.function_arena.demand_collections.saturating_add(1); }
        self.function_arena.retired = self.function_arena.retired.saturating_add(functions);
        self.function_arena.environments_retired = self.function_arena.environments_retired.saturating_add(environments);
        self.function_arena.scope_retired = self.function_arena.scope_retired.saturating_add(scope_functions);
        self.function_arena.next_collection_at = self.function_arena.allocations.saturating_add(32);
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn retire_proven_inactive_candidate(&mut self, id: usize) -> Result<(), Fault> {
        let graph = if self.scope_retirement_enabled() { self.mark_scope_candidate_comparison(true) }
            else { self.mark_root_graph(&[], true, true) }.map_err(Fault::new)?;
        if graph.functions.get(id).copied() != Some(false) { return Err(Fault::new("Candidate remains reachable.")); }
        self.retire_unreachable_script(id)
    }

    pub(super) fn direct_object_closure_assignment(&self, program: &Rc<Program>, right: ExprId, reference: &Reference, value: &Value) -> Option<usize> {
        if !matches!(reference, Reference::Property(Value::Object(_), _) | Reference::SymbolProperty(Value::Object(_), _)
            | Reference::ComputedProperty(Value::Object(_), _)) { return None; }
        let ExprKind::Function(template) = &program.exprs[right].kind else { return None; };
        let Value::Function(id) = value else { return None; };
        matches!(self.functions.get(*id), Some(Function::Script { program: actual, template: actual_template, .. })
            if Rc::ptr_eq(program, actual) && template == actual_template).then_some(*id)
    }

    // Only the syntactic expression's freshly created anonymous identity may be
    // tagged. All root/audit/inhibitor gates still apply to actual retirement.
    pub(super) fn fresh_anonymous_scope_closure(&self, program: &Rc<Program>, expression: ExprId, value: &Value) -> Option<usize> {
        let ExprKind::Function(template) = &program.exprs[expression].kind else { return None; };
        if program.templates[*template].name.is_some() { return None; }
        let Value::Function(id) = value else { return None; };
        matches!(self.functions.get(*id), Some(Function::Script { program: actual, template: actual_template, .. })
            if Rc::ptr_eq(program, actual) && template == actual_template).then_some(*id)
    }

    // Raw public Value IDs can be retained by an embedder after a call returns.
    // Pins are permanent until document/runtime teardown. ID-only sets are bounded
    // by the existing object/function arenas and never copy script payloads.
    pub(super) fn pin_exported_value(&mut self, value: &Value) {
        match value {
            Value::Function(id) if self.function_is_occupied(*id) => { self.exported_function_roots.insert(*id); }
            Value::Object(id) if *id < self.objects.len() => { self.exported_object_roots.insert(*id); }
            _ => {}
        }
    }

    pub(crate) fn persistent_allocation_report(&self) -> String {
        match self.mark_persistent_functions(&[], true) {
            Ok(graph) => {
                let allocated = self.functions.iter().filter(|function| matches!(function, Function::Script { .. })).count();
                let reachable = self.functions.iter().enumerate().filter(|(id, function)| graph.functions[*id] && matches!(function, Function::Script { .. })).count();
                drop(graph);
                let graph = match self.mark_root_graph(&[], true, true) {
                    Ok(graph) => graph,
                    Err(message) => return format!("Callable reachability unavailable: {message}\n"),
                };
                let candidates = self.object_assigned_closures.len();
                let unmarked = self.object_assigned_closures.iter().filter(|&&id| !graph.functions[id]).count();
                drop(graph);
                let scope_comparison = match self.mark_scope_candidate_comparison(true) {
                    Ok(graph) => {
                        let scope_only = self.scope_assigned_closures.iter().filter(|&&id| !graph.functions[id]).count();
                        let total = self.object_assigned_closures.union(&self.scope_assigned_closures)
                            .filter(|&&id| !graph.functions[id]).count();
                        let policy = if self.scope_retirement_enabled() { "enabled at audited entries" } else { "disabled" };
                        format!("Scope-binding mark-only comparison (inactive; permanent API pins retained): tagged={}; eligible unmarked={total}; scope unmarked={scope_only}. Production scope retirement remains {policy}; scope functions retired={}.", self.scope_assigned_closures.len(), self.function_arena.scope_retired)
                    }
                    Err(message) => format!("Scope-binding mark-only comparison unavailable: {message}"),
                };
                // Ordinary arrays retain managed values through their traced
                // properties/prototype and explicit persistent/transient roots.
                // Never re-seed discarded arrays: their unreachable slots may
                // now contain retired IDs, like unreachable plain objects.
                let arrays = self.objects.iter().filter(|object| object.length.is_some() && Self::plain_array_storage(object)).count();
                let array_comparison = format!("Ordinary arrays follow traced references: arrays={arrays}; other exotic seed roots retained. Array/object slots remain append-only.\n{scope_comparison}");
                let program_retention = format!("Program retention: programs={}; source-bytes={}; AST-nodes={}; regexp-accounting-bytes={}; pressure passes={}; programs retired={}. Regexp charges retained.",
                    self.programs.len(), self.retained_source, self.retained_ast, self.retained_regexp,
                    self.program_allocation.passes, self.program_allocation.retired);
                let history = if self.function_arena.collections == 0 { "No collection or slot reuse has occurred." } else { "Candidate function slots may be reused." };
                format!("Callable reachability (inactive stack; conservative native/Promise/API roots): Script allocated={allocated}; reachable={reachable}; exported function/object pins={}/{}. Object-assigned closure candidates={candidates}; unmarked with other functions/non-array exotic objects rooted={unmarked}. Conservative Script-entry collection: passes={}; functions retired={}; environments retired={}; occupied/high-water={}/{}; Script creations={}; slot allocations={}; Function-literal passes={}. {history} Objects remain append-only.\n{array_comparison}\n{program_retention}\nEvaluator root snapshots: {}; call-root scopes={}; completion-root scopes={}; peak ID references={}; active registrations=0. Script entries={}; uninhibited={}.\n",
                    self.exported_function_roots.len(), self.exported_object_roots.len(), self.function_arena.collections, self.function_arena.retired,
                    self.function_arena.environments_retired, self.function_arena.occupied, self.functions.len(), self.function_arena.script_created,
                    self.function_arena.allocations, self.function_arena.demand_collections,
                    self.transient_roots.snapshots, self.transient_roots.call_scopes, self.transient_roots.completion_scopes, self.transient_roots.peak, self.transient_roots.script_entries, self.transient_roots.uninhibited_script_entries)
            }
            Err(message) => format!("Callable reachability unavailable: {message}\n"),
        }
    }

    fn mark_persistent_functions(&self, exported: &[Value], include_registered_exports: bool) -> Result<Graph, &'static str> {
        self.mark_root_graph(exported, include_registered_exports, false)
    }

    fn mark_root_graph(&self, exported: &[Value], include_registered_exports: bool, conservative_candidates: bool) -> Result<Graph, &'static str> {
        if self.call_depth != 0 || self.eval_depth != 0 || self.evaluation_frames != 0
            || self.evaluation_frame_slots != 0 || self.evaluation_argument_slots != 0
            || !self.active_array_joins.is_empty() || !self.promises.reachability_is_inactive()
            || !self.transient_registration_is_empty() {
            return Err("Mark-only snapshot requires inactive JavaScript; transient roots are not covered.");
        }
        self.trace_root_graph(exported, include_registered_exports, conservative_candidates, false)
    }

    #[cfg(test)]
    fn mark_audited_active_roots(&self, conservative_candidates: bool) -> Result<Graph, &'static str> {
        if !self.active_roots_are_audited() {
            return Err("Active mark-only audit requires complete registered guard ownership and uninhibited helpers.");
        }
        self.trace_root_graph(&[], false, conservative_candidates, true)
    }

    fn trace_root_graph(&self, exported: &[Value], include_registered_exports: bool,
        conservative_candidates: bool, transient: bool) -> Result<Graph, &'static str> {
        self.trace_root_graph_metered(exported, include_registered_exports, conservative_candidates, transient,
            &mut Meter { remaining: None, deadline: self.job_deadline })
    }

    fn trace_root_graph_metered(&self, exported: &[Value], include_registered_exports: bool,
        conservative_candidates: bool, transient: bool, meter: &mut Meter) -> Result<Graph, &'static str> {
        self.trace_root_graph_array_policy(exported, include_registered_exports, conservative_candidates, transient, false, meter)
    }

    fn plain_array_storage(object: &Object) -> bool {
        object.date.is_none() && object.url.is_none() && object.rect.is_none() && object.dom_exception.is_none() && object.primitive.is_none()
            && object.promise.is_none() && object.arguments.is_none() && object.binary.is_none() && object.collection.is_none()
            && object.iterator.is_none() && object.text.is_none() && object.regexp.is_none() && object.error_name.is_none()
    }

    fn trace_root_graph_array_policy(&self, exported: &[Value], include_registered_exports: bool,
        conservative_candidates: bool, transient: bool, keep_array_seeds: bool, meter: &mut Meter) -> Result<Graph, &'static str> {
        self.trace_root_graph_candidate_policy(exported, include_registered_exports, conservative_candidates,
            transient, keep_array_seeds, false, meter)
    }

    fn mark_scope_candidate_comparison(&self, include_registered_exports: bool) -> Result<Graph, &'static str> {
        if self.call_depth != 0 || self.eval_depth != 0 || self.evaluation_frames != 0
            || self.evaluation_frame_slots != 0 || self.evaluation_argument_slots != 0
            || !self.active_array_joins.is_empty() || !self.promises.reachability_is_inactive()
            || !self.transient_registration_is_empty() {
            return Err("Scope comparison requires inactive JavaScript.");
        }
        self.trace_root_graph_candidate_policy(&[], include_registered_exports, true, false, false, true,
            &mut Meter { remaining: None, deadline: self.job_deadline })
    }

    fn trace_root_graph_candidate_policy(&self, exported: &[Value], include_registered_exports: bool,
        conservative_candidates: bool, transient: bool, keep_array_seeds: bool,
        include_scope_candidates: bool, meter: &mut Meter) -> Result<Graph, &'static str> {
        meter.charge(self.objects.len() + self.functions.len() * 4 + self.environments.len())?;
        let mut graph = Graph::new(self)?;
        // Fixed-size intrinsic families include current and parked main blocks,
        // even when no child realm exists. Error-family maps have bounded keys.
        meter.charge(self.transient_roots.roots.len() + 2
            + (1 + usize::from(self.parked_main_roots.is_some()) + self.intrinsic_realms.len()) * 256
            + self.host_objects.len() + self.host_method_values.len() + exported.len()
            + self.exported_function_roots.len() + self.exported_object_roots.len())?;
        if transient { for &edge in &self.transient_roots.roots { graph.add(edge); } }
        if let Some(target) = self.invocation.and_then(|context| context.new_target) {
            graph.add(Edge::Function(target));
        }
        graph.add(Edge::Environment(0));
        graph.add(Edge::Environment(self.global_environment));
        for realm in &self.intrinsic_realms { graph.add(Edge::Environment(realm.global_environment)); }
        for (id, function) in self.functions.iter().enumerate() {
            meter.charge(1)?;
            if !matches!(function, Function::Vacant) && (!matches!(function, Function::Script { .. }) || conservative_candidates && !(self.object_assigned_closures.contains(&id)
                || include_scope_candidates && self.scope_assigned_closures.contains(&id))) {
                graph.add(Edge::Function(id));
            }
        }
        for &id in self.host_objects.values() { graph.add(Edge::Object(id)); }
        for value in self.host_method_values.values().chain(exported) { Edge::value(value, &mut |edge| graph.add(edge)); }
        if include_registered_exports {
            for &id in &self.exported_function_roots { graph.add(Edge::Function(id)); }
            for &id in &self.exported_object_roots { graph.add(Edge::Object(id)); }
        }
        // Conservatively preserve all Promise metadata and pending rejection
        // reporting. This is intentionally broader than eventual precise roots.
        for (id, object) in self.objects.iter().enumerate() {
            meter.charge(1)?;
            let plain = (object.length.is_none() || !keep_array_seeds) && Self::plain_array_storage(object);
            if object.promise.is_some() || conservative_candidates && !plain { graph.add(Edge::Object(id)); }
        }
        meter.charge(self.promises.reachability_cost())?;
        self.promises.reachability_edges(&mut |edge| graph.add(edge));
        self.reachability_intrinsic_edges(&mut |edge| graph.add(edge));
        while let Some(edge) = graph.pending.pop() {
            meter.charge(1)?;
            let mut add = |edge| graph.add(edge);
            match edge {
                Edge::Environment(id) => {
                    let Some(environment) = self.environments[id].as_ref() else { add(Edge::Environment(usize::MAX)); continue; };
                    meter.charge(2 + environment.bindings.len() * 2 + environment.arguments.as_ref().map_or(0, arguments::Pending::reachability_cost))?;
                    if let Some(parent) = environment.parent { add(Edge::Environment(parent)); }
                    for binding in environment.bindings.values() {
                        if let Some(value) = &binding.value { Edge::value(value, &mut add); }
                    }
                    if let Some(pending) = &environment.arguments { pending.reachability_edges(&mut add); }
                }
                Edge::Function(id) => {
                    meter.charge(16 + match &self.functions[id] { Function::Bound(bound) => bound.reachability_cost(), _ => 0 })?;
                    let Some(&properties) = self.function_properties.get(id) else { add(Edge::Object(usize::MAX)); continue; };
                    add(Edge::Object(properties));
                    match &self.functions[id] {
                        Function::Script { environment, lexical_this, lexical_invocation, .. } => {
                            add(Edge::Environment(*environment));
                            if let Some(value) = lexical_this { Edge::value(value, &mut add); }
                            if let Some(target) = lexical_invocation.and_then(|context| context.new_target) {
                                add(Edge::Function(target));
                            }
                        }
                        Function::Bound(bound) => bound.reachability_edges(&mut add),
                        Function::Promise(closure) => closure.reachability_edges(&mut add),
                        _ => {}
                    }
                }
                Edge::Object(id) => {
                    let object = &self.objects[id];
                    meter.charge(32 + (object.properties.len() + object.symbol_properties.len()) * 4
                        + object.collection.as_ref().map_or(0, collections::Data::reachability_cost)
                        + object.promise.as_ref().map_or(0, promises::Data::reachability_cost))?;
                    if let Some(prototype) = object.prototype {
                        add(match prototype { Ordinary::Object(id) => Edge::Object(id), Ordinary::Function(id) => Edge::Function(id) });
                    }
                    for property in object.properties.values().chain(object.symbol_properties.values()) {
                        if let StoredValue::Value(value) = &property.value { Edge::value(value, &mut add); }
                        if let Some(accessors) = property.accessor {
                            for id in [accessors.get, accessors.set].into_iter().flatten() { add(Edge::Function(id)); }
                        }
                    }
                    if let Some(arguments) = &object.arguments {
                        if !arguments.mapped.is_empty() { add(Edge::Environment(arguments.environment)); }
                    }
                    if let Some(binary) = &object.binary { binary.reachability_edges(&mut add); }
                    if let Some(collection) = &object.collection { collection.reachability_edges(&mut add); }
                    if let Some(iterator) = &object.iterator { iterator.reachability_edges(&mut add); }
                    if let Some(url) = &object.url { url.reachability_edges(&mut add); }
                    if let Some(promise) = &object.promise { promise.reachability_edges(&mut add); }
                }
            }
        }
        if graph.invalid { Err("A retained root has no valid arena entry; snapshot rejected.") } else { Ok(graph) }
    }
}

#[test]
fn vacant_function_handles_are_rejected_by_dispatch_reflection_and_graph_roots() {
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    let mut runtime = Runtime::new();
    runtime.execute("var holder={};holder.fn=function gone(){};holder.fn=null;0;", &mut NoHost).unwrap();
    let id = *runtime.object_assigned_closures.iter().next().unwrap();
    runtime.retire_proven_inactive_candidate(id).unwrap();
    runtime.pin_exported_value(&Value::Function(id));
    assert!(!runtime.exported_function_roots.contains(&id));
    assert!(runtime.ordinary_id(Ordinary::Function(id)).is_err());
    assert!(runtime.function_source(id).is_err());
    assert!(runtime.from_constructor(&Value::Function(id)).is_err());
    assert!(runtime.bind_function(id, &[], &mut NoHost).is_err());
    assert!(runtime.call_value(Value::Function(id), &[], Value::Undefined, &mut NoHost).is_err());
    assert!(runtime.construct_value(Value::Function(id), &[], &mut NoHost).is_err());
    assert!(runtime.mark_root_graph(&[Value::Function(id)], true, true).is_err());
    assert!(runtime.mark_root_graph(&[], true, true).is_ok());
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn array_seed_comparison_distinguishes_discarded_cycles_without_mutation_or_callbacks() {
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    let mut runtime = Runtime::new();
    runtime.execute("function build(n){var state=n,c={},a=[c];c.write=function(v){state=v;};c.read=function(){return state;};c.back=a;return a;}var kept=build(71);build(72);0;", &mut NoHost).unwrap();
    let before = (runtime.function_arena.occupied, runtime.objects.len(), runtime.bindings, runtime.value_bytes,
        runtime.job_usage(), runtime.job_deadline, runtime.function_arena.collections);
    // Retain the original pre-collection root-policy comparison explicitly.
    let current = runtime.trace_root_graph_array_policy(&[], true, true, false, true,
        &mut Meter { remaining: None, deadline: runtime.job_deadline }).unwrap();
    assert_eq!(runtime.object_assigned_closures.iter().filter(|&&id| current.functions[id]).count(), 4);
    let compared = runtime.trace_root_graph_array_policy(&[], true, true, false, false,
        &mut Meter { remaining: None, deadline: runtime.job_deadline }).unwrap();
    assert_eq!(runtime.object_assigned_closures.iter().filter(|&&id| compared.functions[id]).count(), 2);
    let report = runtime.persistent_allocation_report();
    assert!(report.contains("unmarked with other functions/non-array exotic objects rooted=2"), "{report}");
    assert!(report.contains("Ordinary arrays follow traced references"), "{report}");
    assert_eq!((runtime.function_arena.occupied, runtime.objects.len(), runtime.bindings, runtime.value_bytes,
        runtime.job_usage(), runtime.job_deadline, runtime.function_arena.collections), before);
    assert_eq!(runtime.execute("kept[0].write(83);kept[0].read();", &mut NoHost).unwrap().to_number(), 83.0);
}

#[test]
fn audited_script_constructor_bodies_keep_receiver_prototype_arguments_and_result_handoffs() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static BODIES: AtomicUsize = AtomicUsize::new(0);
    static NATIVE_EXECUTORS: AtomicUsize = AtomicUsize::new(0);
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    fn observe(runtime: &Runtime, function: usize, args: &[Value], this: &Value) {
        let StoredValue::Value(Value::String(name)) = &runtime.objects[runtime.function_properties[function]].properties["name"].value else { panic!("fixture data name") };
        if name == "nativeExecutor" {
            assert!(runtime.transient_roots.inhibited > 0);
            assert!(runtime.mark_audited_active_roots(false).is_err());
            NATIVE_EXECUTORS.fetch_add(1, Ordering::Relaxed);
        }
        if !matches!(name.as_str(), "Item" | "Returns" | "Thrower") { return; }
        assert_eq!(runtime.transient_roots.inhibited, 0);
        let graph = runtime.mark_audited_active_roots(false).unwrap();
        let Value::Object(receiver) = this else { panic!("constructed receiver") };
        assert!(graph.objects[*receiver]);
        assert!(graph.functions[function]);
        if let Some(prototype) = runtime.objects[*receiver].prototype {
            assert!(match prototype { Ordinary::Object(id) => graph.objects[id], Ordinary::Function(id) => graph.functions[id] });
        }
        for value in args {
            match value { Value::Object(id) => assert!(graph.objects[*id]), Value::Function(id) => assert!(graph.functions[*id]), _ => {} }
        }
        BODIES.fetch_add(1, Ordering::Relaxed);
    }
    let mut runtime = Runtime::new();
    runtime.transient_roots.script_input_observer = Some(observe);
    runtime.execute(r#"
var seen;
function Item(fn,arg){seen=this;this.fn=fn;this.arg=arg;Item.prototype={later:1};return 17;}
Item.prototype={original:1};
var bound=Item.bind({wrong:1},function(){return 19;},{value:23}),chained=bound.bind({wrong:2});
var first=new chained();
if(first!==seen||first.original!==1||first.fn()!==19||first.arg.value!==23||first.wrong!==undefined)throw 'bound construction';
var second=new Item(function(){return 29;},{value:31});
if(second===first||second!==seen||second.later!==1||second.original!==undefined||first.original!==1)throw 'prototype snapshot';
function Returns(){this.discarded=true;return {override:37};}
if(new Returns().override!==37)throw 'object return';
function Thrower(){throw {reason:41};}
try{new Thrower();throw 'accepted throw';}catch(e){if(e.reason!==41)throw e;}
new Promise(function nativeExecutor(resolve){resolve(43);});
"#, &mut NoHost).unwrap();
    assert_eq!(BODIES.load(Ordering::Relaxed), 4);
    assert_eq!(NATIVE_EXECUTORS.load(Ordering::Relaxed), 1);
    assert!(runtime.transient_registration_is_empty());
    runtime.define_global("native", Value::Host(7)).unwrap();
    let error = runtime.execute("var ran=false;function Broken(){ran=true;}Broken.prototype=native;new Broken();", &mut NoHost).unwrap_err();
    assert!(error.contains("Native host object prototypes are not implemented"));
    assert!(runtime.transient_registration_is_empty());
    runtime.begin_job();
    assert!(matches!(runtime.execute("ran;", &mut NoHost).unwrap(), Value::Bool(false)));
}

#[test]
fn active_mark_audit_covers_forwarders_completions_and_popped_jobs_without_export_pins() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static AUDITS: AtomicUsize = AtomicUsize::new(0);
    static JOBS: AtomicUsize = AtomicUsize::new(0);
    static COMPLETIONS: AtomicUsize = AtomicUsize::new(0);
    struct EvalHost;
    impl Host for EvalHost {
        fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    fn observe(runtime: &Runtime, function: usize, args: &[Value], this: &Value) {
        if runtime.transient_roots.inhibited != 0 {
            assert!(runtime.mark_audited_active_roots(false).is_err());
            return;
        }
        let before = (runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage());
        let graph = runtime.mark_audited_active_roots(false).unwrap();
        assert!(graph.functions[function]);
        for value in args.iter().chain(std::iter::once(this)) {
            match value {
                Value::Object(id) => assert!(graph.objects[*id]),
                Value::Function(id) => assert!(graph.functions[*id]),
                _ => {}
            }
        }
        // Declarations have no private named-expression binding. Inspect the
        // fixture's unchanged own data name without invoking a public getter.
        let name = |id: usize| match &runtime.objects[runtime.function_properties[id]].properties["name"].value {
            StoredValue::Value(Value::String(name)) => Some(name.as_str()), _ => None,
        };
        if name(function) == Some("checkpoint") {
            let phase = runtime.environment(runtime.global_environment).bindings["phase"].value.as_ref().unwrap().to_number() as usize;
            let expected = ["hiddenReturn", "hiddenThrow", "hiddenLoop", "hiddenList", "hiddenSwitch", "hiddenFor"][phase - 1];
            let id = (0..runtime.functions.len()).rfind(|&id| name(id) == Some(expected)).unwrap();
            assert!(graph.functions[id], "lost pending {expected}");
            COMPLETIONS.fetch_or(1 << (phase - 1), Ordering::Relaxed);
            let discarded = (0..runtime.functions.len()).find(|&id| name(id) == Some("discarded")).unwrap();
            assert!(!graph.functions[discarded], "dead constructor/capture cycle became a root");
        }
        for (label, bit) in [("callback", 1), ("reaction", 2), ("thenableMethod", 4)] {
            if name(function) == Some(label) { JOBS.fetch_or(bit, Ordering::Relaxed); }
        }
        AUDITS.fetch_add(1, Ordering::Relaxed);
        assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage()), before);
    }
    let mut runtime = Runtime::new();
    runtime.execute("(function discarded(){});0;", &mut EvalHost).unwrap();
    runtime.transient_roots.script_input_observer = Some(observe);
    runtime.execute(r#"
function checkpoint(){return undefined;}var phase=1;
function returning(){try{return function hiddenReturn(){return 71;};}finally{checkpoint();}}
if(returning()()!==71)throw 'return';
phase=2;try{try{throw function hiddenThrow(){return 72;};}finally{checkpoint();}}catch(e){if(e()!==72)throw 'throw';}
phase=3;eval('var n=0;while(n++<2){if(n===1)(function hiddenLoop(){});else{checkpoint();break;}}');
phase=4;eval('(function hiddenList(){});var ignored=checkpoint();');
phase=5;switch((function hiddenSwitch(){})){case checkpoint():throw 'matched';default:break;}
phase=6;for(var i=0;i<2;checkpoint()){i++;(function hiddenFor(){});}
function f(v){return v;}f.call(null,{});f.apply(null,[{}]);f.bind(null,{})();
var getter={get value(){return f({});}};getter.value;
eval('f({});');(0,eval)('f({});');
var scoped=eval("'use strict';var retained=79;(function heldEval(){return retained;});");
if(scoped()!==79)throw 'eval scope';
function nestedEval(){return eval('f({});');}nestedEval();
function throwingEval(){try{eval('throw 17;');}catch(e){return e;}}
if(throwingEval()!==17)throw 'eval recovery';
var privateBound=(function receiverFactory(){var privateReceiver={value:83};function readPrivate(){return this.value;}return readPrivate.bind(privateReceiver).bind({value:0});})();
if(privateBound()!==83)throw 'private bound receiver';
queueMicrotask(function callback(){f({});});Promise.resolve(1).then(function reaction(){f({});});
Promise.resolve({then:function thenableMethod(resolve){f({});resolve(2);}});
[1].map(function guarded(){f({});});
"#, &mut EvalHost).unwrap();
    assert!(runtime.microtask_checkpoint(&mut EvalHost).unwrap().is_empty());
    assert_eq!(JOBS.load(Ordering::Relaxed), 7);
    assert_eq!(COMPLETIONS.load(Ordering::Relaxed), 63);
    assert!(AUDITS.load(Ordering::Relaxed) > 25);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn active_mark_audit_rejects_unowned_guards_inhibitors_and_invalid_transient_handles() {
    let mut runtime = Runtime::new();
    assert!(runtime.mark_audited_active_roots(true).is_ok());
    runtime.call_depth = 1;
    assert!(runtime.mark_audited_active_roots(true).is_err());
    runtime.call_depth = 0;
    runtime.eval_depth = 1;
    assert!(runtime.mark_audited_active_roots(true).is_err());
    runtime.eval_depth = 0;
    runtime.evaluation_frames = 1;
    assert!(runtime.mark_audited_active_roots(true).is_err());
    runtime.evaluation_frames = 0;
    let marker = runtime.inhibit_transient_collection();
    assert!(runtime.mark_audited_active_roots(true).is_err());
    runtime.restore_transient_marker(marker);
    runtime.transient_roots.active_job_registrations = 1;
    assert!(runtime.mark_audited_active_roots(true).is_err());
    runtime.transient_roots.active_job_registrations = 0;
    runtime.recursive_evaluation_oracle = true;
    assert!(runtime.mark_audited_active_roots(true).is_err());
    runtime.recursive_evaluation_oracle = false;
    let marker = runtime.root_marker();
    runtime.push_transient_root(Edge::Function(usize::MAX)).unwrap();
    assert!(runtime.mark_audited_active_roots(true).is_err());
    runtime.restore_root_marker(marker);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn replaced_cell_cycles_are_unreachable_without_losing_exported_or_live_state() {
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    let mut runtime = Runtime::new();
    runtime.execute("var slots=[],first;var model=Object.create(Object.prototype,{payload:{get:function(){return this.read();}}});function replace(value){var state=value;var cell=Object.create(model);cell.write=function(next){state=next;};cell.read=function(){return state;};slots[0]=cell;return cell;}for(var i=0;i<64;i++){var current=replace(i);if(i===0)first=current;current.write(i*2);if(current.payload!==i*2)throw 'state';}", &mut NoHost).unwrap();
    let exported = runtime.execute("var saved=replace(99);var outward=saved.read;saved=null;outward=null;slots[0]=current;", &mut NoHost).unwrap();
    // Capture an actual handle separately, then remove its script-visible name.
    let exported_function = runtime.execute("var escaped=replace(100).read;escaped;", &mut NoHost).unwrap();
    runtime.execute("escaped=null;slots[0]=current;", &mut NoHost).unwrap();
    let before = (runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage());
    let graph = runtime.mark_persistent_functions(&[exported, exported_function.clone()], false).unwrap();
    let scripts = runtime.functions.iter().enumerate().filter(|(id, function)| graph.functions[*id] && matches!(function, Function::Script { .. })).count();
    let total = runtime.functions.iter().filter(|function| matches!(function, Function::Script { .. })).count();
    assert!(total >= 134 && scripts < 16, "total={total}; marked={scripts}");
    let Value::Function(id) = exported_function else { panic!("function export") };
    assert!(graph.functions[id]);
    assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage()), before);
    assert_eq!(runtime.invoke(Value::Function(id), &[], Value::Undefined, &mut NoHost).unwrap().to_number(), 100.0);
    println!("Original replaced-cell graph: total Script={total}, persistent/export-root reachable={scripts}; no slots collected or reused.");
    runtime.call_depth = 1;
    assert!(runtime.mark_persistent_functions(&[], false).is_err());
}

#[test]
fn persistent_graph_keeps_opaque_storage_accessors_arguments_and_pending_jobs() {
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    for (label, setup) in [
        ("symbol", "var holder={};holder[Symbol('slot')]=candidate;"),
        ("accessor", "var holder={};Object.defineProperty(holder,'x',{get:candidate});"),
        ("map key", "var holder=new Map([[candidate,17]]);"),
        ("set value", "var holder=new Set([candidate]);"),
        ("iterator source", "var holder=[candidate].values();"),
        ("bound receiver", "var holder=(function(){return this.value;}).bind({value:candidate});"),
        ("mapped arguments", "var holder=(function(f){var a=arguments;f=candidate;return a;})(0);"),
        ("pending strict arguments", "var holder=(function(f){'use strict';f=null;return ()=>arguments[0];})(candidate);"),
        ("view buffer", "var holder=(function(f){var b=new ArrayBuffer(2);b.saved=f;return new DataView(b);})(candidate);"),
        ("query owner", "var holder=(function(f){var u=new URL('https://example.com/?a=b');u.saved=f;return u.searchParams;})(candidate);"),
        ("fulfilled promise", "var holder=Promise.resolve(candidate);"),
        ("unreported rejection", "Promise.reject(candidate);"),
        ("finally closure", "Promise.resolve(0).finally(candidate);"),
        ("queued callback", "queueMicrotask(candidate);"),
    ] {
        let mut runtime = Runtime::new();
        let candidate = runtime.execute(&format!("var candidate=function(){{return 42;}};{setup}candidate;"), &mut NoHost).unwrap();
        let Value::Function(id) = candidate else { panic!("candidate") };
        runtime.execute("candidate=null;", &mut NoHost).unwrap();
        let before = (runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage());
        let graph = runtime.mark_persistent_functions(&[], false).unwrap();
        assert!(graph.functions[id], "lost {label}");
        assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage()), before, "{label}");
    }
}

#[test]
fn api_exports_are_permanent_deduplicated_roots_without_payload_or_work_copies() {
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    let mut runtime = Runtime::new();
    let function = runtime.execute("(function(){return 17;});", &mut NoHost).unwrap();
    let Value::Function(id) = function else { panic!("export") };
    assert!(runtime.exported_function_roots.contains(&id));
    assert!(runtime.mark_persistent_functions(&[], true).unwrap().functions[id]);
    assert!(!runtime.mark_persistent_functions(&[], false).unwrap().functions[id]);
    let before = (runtime.functions.len(), runtime.objects.len(), runtime.value_bytes, runtime.job_usage(), runtime.exported_function_roots.len());
    runtime.pin_exported_value(&function);
    runtime.pin_exported_value(&function.clone());
    let report = runtime.persistent_allocation_report();
    assert!(report.contains("No collection or slot reuse"), "{report}");
    assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.value_bytes, runtime.job_usage(), runtime.exported_function_roots.len()), before);
    let handler = runtime.compile_handler("return function(){return 23;};").unwrap();
    let returned = runtime.invoke(handler, &[], Value::Undefined, &mut NoHost).unwrap();
    let Value::Function(returned_id) = returned else { panic!("invoke export") };
    assert!(runtime.exported_function_roots.contains(&returned_id));
    assert_eq!(runtime.invoke(returned, &[], Value::Undefined, &mut NoHost).unwrap().to_number(), 23.0);
    let object = runtime.execute("(function(){var state=29;return {read:function(){return state;}};})();", &mut NoHost).unwrap();
    let Value::Object(object_id) = object else { panic!("object export") };
    let Value::Function(method_id) = *runtime.objects[object_id].properties["read"].value.as_value().unwrap() else { panic!("method") };
    assert!(runtime.exported_object_roots.contains(&object_id));
    assert!(runtime.mark_persistent_functions(&[], true).unwrap().functions[method_id]);
    assert!(!runtime.mark_persistent_functions(&[], false).unwrap().functions[method_id]);
    runtime.define_global("retained", object).unwrap();
    assert_eq!(runtime.execute("retained.read();", &mut NoHost).unwrap().to_number(), 29.0);
    let before = (runtime.functions.len(), runtime.exported_function_roots.clone());
    assert!(runtime.define_host_function("invalid name", 1, "unused").is_err());
    assert_eq!((runtime.functions.len(), runtime.exported_function_roots.clone()), before);
    assert!(runtime.mark_persistent_functions(&[], true).is_ok());
}

#[test]
fn reachability_snapshot_rejects_unregistered_transient_state_and_invalid_roots() {
    let mut runtime = Runtime::new();
    runtime.call_depth = 1;
    assert!(runtime.persistent_allocation_report().contains("unavailable"));
    runtime.call_depth = 0;
    runtime.eval_depth = 1;
    assert!(runtime.persistent_allocation_report().contains("unavailable"));
    runtime.eval_depth = 0;
    runtime.evaluation_frames = 1;
    assert!(runtime.persistent_allocation_report().contains("unavailable"));
    runtime.evaluation_frames = 0;
    runtime.evaluation_frame_slots = 1;
    assert!(runtime.persistent_allocation_report().contains("unavailable"));
    runtime.evaluation_frame_slots = 0;
    runtime.evaluation_argument_slots = 1;
    assert!(runtime.persistent_allocation_report().contains("unavailable"));
    runtime.evaluation_argument_slots = 0;
    runtime.active_array_joins.push(0);
    assert!(runtime.persistent_allocation_report().contains("unavailable"));
    runtime.active_array_joins.clear();
    runtime.exported_function_roots.insert(usize::MAX);
    let before = (runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage());
    assert!(runtime.persistent_allocation_report().contains("snapshot rejected"));
    assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.value_bytes, runtime.job_usage()), before);
}

#[test]
fn host_validation_dictionary_and_failed_hoist_exports_remain_roots() {
    use std::cell::RefCell;
    struct Retain { mode: &'static str, held: RefCell<Vec<Value>> }
    impl Host for Retain {
        fn owns_window_property(&self, object: Option<u32>, name: &str) -> bool { object == Some(77) && name == "captured" }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn validate_set(&self, _: u32, _: &str, value: &Value) -> Result<(), (&'static str, String)> {
            self.held.borrow_mut().push(value.clone()); Err(("TypeError", "retained rejected value".into()))
        }
        fn set(&mut self, _: u32, _: &str, value: Value) -> Result<(), String> { self.held.borrow_mut().push(value); Err("retained rejected declaration".into()) }
        fn validate_call(&self, _: u32, _: &str, args: &[Value]) -> Result<(), (&'static str, String)> {
            if matches!(self.mode, "call" | "object") { self.held.borrow_mut().push(args[0].clone()); Err(("TypeError", "retained rejected argument".into())) } else { Ok(()) }
        }
        fn dictionary_argument(&self, _: u32, _: &str) -> Option<(usize, &'static [&'static str])> { (self.mode == "dictionary").then_some((0, &["callback"])) }
        fn call_with_dictionary(&mut self, _: u32, _: &str, _: &[Value], fields: Option<&[(&str, Value)]>) -> Result<Value, String> {
            self.held.borrow_mut().push(fields.unwrap()[0].1.clone()); Ok(Value::Undefined)
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    for (mode, source) in [
        ("call", "try{native(function(){return 17;});}catch(e){}0;"),
        ("object", "try{native({read:function(){return 17;}});}catch(e){}0;"),
        ("set", "try{window.captured=function(){return 17;};}catch(e){}0;"),
        ("dictionary", "native({get callback(){return function(){return 17;};}});0;"),
        ("hoist", "function captured(){return 17;}"),
    ] {
        let mut runtime = Runtime::new();
        runtime.bind_browser_global(77).unwrap();
        runtime.define_global("window", Value::Host(77)).unwrap();
        runtime.define_host_function("native", 1, "retain").unwrap();
        let mut host = Retain { mode, held: RefCell::new(Vec::new()) };
        let result = runtime.execute(source, &mut host);
        if mode == "hoist" { assert!(result.unwrap_err().contains("retained rejected declaration")); } else { result.unwrap(); }
        let held = host.held.borrow();
        let id = match held[0] {
            Value::Function(id) => { assert!(runtime.exported_function_roots.contains(&id), "{mode}"); id }
            Value::Object(id) => {
                assert!(runtime.exported_object_roots.contains(&id));
                let Value::Function(method) = *runtime.objects[id].properties["read"].value.as_value().unwrap() else { panic!("method") };
                method
            }
            _ => panic!("export: {mode}"),
        };
        assert!(runtime.mark_persistent_functions(&[], true).unwrap().functions[id], "{mode}");
        assert!(!runtime.mark_persistent_functions(&[], false).unwrap().functions[id], "{mode}");
    }
}

#[test]
fn object_assignment_candidates_preserve_aliases_exports_and_constructor_cycles() {
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    let mut runtime = Runtime::new();
    runtime.execute("var cell={};cell.read=function(){return 1;};var alias=cell.read;cell.read=function(){return 2;};var ordinary=function(){return 3;};(function(){return 4;})();var literal={read(){return 5;}};0;", &mut NoHost).unwrap();
    assert_eq!(runtime.object_assigned_closures.len(), 2);
    let candidates: Vec<_> = runtime.object_assigned_closures.iter().copied().collect();
    let graph = runtime.mark_root_graph(&[], true, true).unwrap();
    assert!(candidates.iter().all(|&id| graph.functions[id]));
    runtime.execute("alias=null;cell.read=null;0;", &mut NoHost).unwrap();
    let graph = runtime.mark_root_graph(&[], true, true).unwrap();
    assert!(candidates.iter().all(|&id| !graph.functions[id]));
    // Automatic prototype.constructor backlinks did not tag the other functions.
    for (id, function) in runtime.functions.iter().enumerate() {
        if matches!(function, Function::Script { .. }) && !runtime.object_assigned_closures.contains(&id) { assert!(graph.functions[id]); }
    }

    runtime.execute("cell.read=function(){return 17;};0;", &mut NoHost).unwrap();
    let exported = runtime.execute("cell.read;", &mut NoHost).unwrap();
    let Value::Function(id) = exported else { panic!("export") };
    runtime.execute("cell.read=null;0;", &mut NoHost).unwrap();
    assert!(!runtime.mark_root_graph(&[], false, true).unwrap().functions[id]);
    assert!(runtime.mark_root_graph(&[], true, true).unwrap().functions[id]);
    assert_eq!(runtime.invoke(exported, &[], Value::Undefined, &mut NoHost).unwrap().to_number(), 17.0);
}

#[test]
fn candidate_tagging_observes_successful_writes_without_reconverting_keys_or_replaying_setters() {
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
    }
    let mut runtime = Runtime::new();
    runtime.execute("var conversions=0,sets=0,escaped=null;var target={set value(f){sets++;escaped=f;}};var key={toString(){conversions++;return 'value';}};target[key]=function(){return 17;};if(conversions!==1||sets!==1||escaped()!==17)throw 'key/set order';0;", &mut NoHost).unwrap();
    assert_eq!(runtime.object_assigned_closures.len(), 1);
    let id = *runtime.object_assigned_closures.iter().next().unwrap();
    assert!(runtime.mark_root_graph(&[], true, true).unwrap().functions[id]);
    runtime.execute("escaped=null;0;", &mut NoHost).unwrap();
    assert!(!runtime.mark_root_graph(&[], true, true).unwrap().functions[id]);
    runtime.execute("var rejected={set value(f){throw 23;}};try{rejected.value=function(){return 99;};throw 'missing';}catch(e){if(e!==23)throw e;}0;", &mut NoHost).unwrap();
    assert_eq!(runtime.object_assigned_closures.len(), 1);
    runtime.execute("var symbol=Symbol('slot'),holder={};holder[symbol]=function(){return 31;};if(holder[symbol]()!==31)throw 'symbol';0;", &mut NoHost).unwrap();
    assert_eq!(runtime.object_assigned_closures.len(), 2);
}
