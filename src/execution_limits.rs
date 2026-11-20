//! Host-selected execution and retained-heap budgets, separate from native-stack guards.
//! The browser default is deliberately generous. Smaller test budgets exercise
//! the same accounting/cancellation paths without expensive hostile loops.
//! No page API can select a profile or reset these counters.

/// Grow heap-resident suspension bookkeeping geometrically. Admission uses the
/// allocator's actual capacity, and failure leaves both contents and capacity
/// unchanged. Only the retained replacement remains charged after the move.
pub(super) fn grow_suspension_stack<T>(stack: &mut Vec<T>, available: usize) -> Result<usize, super::Fault> {
    use std::mem::size_of;
    let slot = size_of::<T>();
    debug_assert!(slot != 0);
    let before = stack.capacity();
    let growth = before.max(4).min(available / slot);
    if growth == 0 {
        return Err(super::Fault::new("Retained value storage exhausted while growing suspended scopes."));
    }
    let capacity = before.checked_add(growth)
        .ok_or_else(|| super::Fault::new("Suspended scope capacity arithmetic overflow."))?;
    let mut replacement = Vec::new();
    replacement.try_reserve_exact(capacity)
        .map_err(|_| super::Fault::new("Unable to allocate suspended scope storage."))?;
    let added = replacement.capacity().checked_sub(before)
        .and_then(|slots| slots.checked_mul(slot))
        .filter(|bytes| *bytes <= available)
        .ok_or_else(|| super::Fault::new("Allocated suspended scope capacity exceeds retained value storage."))?;
    replacement.append(stack);
    *stack = replacement;
    Ok(added)
}

#[test]
fn suspended_storage_growth_uses_capacity_and_preserves_failed_reservations() {
    let mut stack: Vec<usize> = Vec::new();
    for i in 0..300 {
        if stack.len() == stack.capacity() {
            let before = stack.capacity();
            let added = grow_suspension_stack(&mut stack, 16 * 1024).unwrap();
            assert_eq!(added, (stack.capacity() - before) * std::mem::size_of::<usize>());
        }
        stack.push(i);
    }
    let before = (stack.clone(), stack.capacity());
    assert!(grow_suspension_stack(&mut stack, std::mem::size_of::<usize>() - 1).is_err());
    assert_eq!((&stack, stack.capacity()), (&before.0, before.1));
}

/// A fallible replacement reservation which does not mutate the live vector.
/// Several parallel stores can be prepared under one byte allowance and then
/// published together without allocation or failure.
pub(super) struct RetainedGrowth<T> {
    replacement: Option<Vec<T>>,
    pub added: usize,
    pub capacity: usize,
}
impl<T> RetainedGrowth<T> {
    pub fn commit(self, values: &mut Vec<T>) {
        if let Some(mut next) = self.replacement { next.append(values); *values = next; }
    }
}
pub(super) fn prepare_retained_growth<T>(values: &Vec<T>, additional: usize, available: usize) -> Result<RetainedGrowth<T>, super::Fault> {
    let before = values.capacity();
    let required = values.len().checked_add(additional).ok_or_else(|| super::Fault::new("Arena capacity overflow."))?;
    if required <= before { return Ok(RetainedGrowth { replacement: None, added: 0, capacity: before }); }
    let slot = std::mem::size_of::<T>();
    let growth = before.max(4).max(required - before).min(available / slot);
    let capacity = before.checked_add(growth).filter(|capacity| *capacity >= required)
        .ok_or_else(|| super::Fault::new("Retained value arena storage exhausted."))?;
    prepare_retained_capacity(values, capacity, available)
}
pub(super) fn prepare_retained_capacity<T>(values: &Vec<T>, capacity: usize, available: usize) -> Result<RetainedGrowth<T>, super::Fault> {
    let before = values.capacity();
    if capacity <= before { return Ok(RetainedGrowth { replacement: None, added: 0, capacity: before }); }
    let mut next = Vec::new();
    next.try_reserve_exact(capacity).map_err(|_| super::Fault::new("Cannot allocate retained arena storage."))?;
    let added = next.capacity().checked_sub(before).and_then(|slots| slots.checked_mul(std::mem::size_of::<T>()))
        .filter(|bytes| *bytes <= available).ok_or_else(|| super::Fault::new("Retained value arena allocation exceeds storage allowance."))?;
    let capacity = next.capacity();
    Ok(RetainedGrowth { replacement: Some(next), added, capacity })
}

/// Retained entries/payloads, not a process working-set or preallocation promise.
/// Object handles remain stable for the document lifetime; these are hard caps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeapLimits {
    pub objects: usize,
    pub properties: usize,
    pub object_properties: usize,
    pub array_length: usize,
    pub retained_values: usize,
}

pub const BROWSER_HEAP: HeapLimits = HeapLimits {
    objects: 1_048_576,
    properties: 16_777_216,
    object_properties: 1_048_576,
    array_length: 16_777_216,
    retained_values: 512 * 1024 * 1024,
};

/// Explicit small profile for unit tests and standalone development probes.
/// Compiling tests does not change Runtime::new() or the browser default.
#[cfg(test)]
pub const REGRESSION_HEAP: HeapLimits = HeapLimits {
    objects: 4096,
    properties: 16_384,
    object_properties: 4096,
    array_length: 8192,
    retained_values: 2 * 1024 * 1024,
};

/// Policy capacities; IDs use usize and storage grows with actual use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capacities {
    #[cfg(test)]
    pub(crate) test_expression_frames: Option<usize>,
    #[cfg(test)]
    pub(crate) test_function_syntax_depth: Option<usize>,
    #[cfg(test)]
    pub(crate) test_native_parser_depth: Option<usize>,
    pub functions: usize,
    #[cfg(test)]
    pub programs: usize,
    #[cfg(test)]
    pub(crate) test_loader_program_count:Option<usize>,
    pub environments: usize,
    pub bindings: usize,
    pub arguments: usize,
    pub source_bytes: usize,
    pub string_bytes: usize,
    pub tokens: usize,
    pub ast_nodes: usize,
    pub compilation_bytes: usize,
    pub retained_source: usize,
    /// Owned loader records, source/label capacity and transport storage.
    pub script_storage_bytes:usize,
    pub retained_ast: usize,
    pub retained_regexp: usize,
    pub property_bytes: usize,
    pub promises: usize,
    pub promise_jobs: usize,
}

#[cfg(test)]
pub const REGRESSION_CAPACITIES: Capacities = Capacities {
    test_expression_frames: Some(super::MAX_EXPRESSION_FRAMES),
    test_function_syntax_depth: Some(super::MAX_PARSE_DEPTH),
    test_native_parser_depth: Some(super::MAX_PARSE_DEPTH),
    functions: 1024, programs: 128, test_loader_program_count:Some(128), environments: 4096, bindings: 4096,
    arguments: 256, source_bytes: 512 * 1024, string_bytes: 128 * 1024,
    tokens: 131_072, ast_nodes: 131_072, retained_source: 2 * 1024 * 1024,
    compilation_bytes: 64 * 1024 * 1024,
    script_storage_bytes:REGRESSION_HEAP.retained_values,
    retained_ast: 262_144, retained_regexp: 2 * 1024 * 1024,
    property_bytes: 256, promises: 1024, promise_jobs: 4096,
};

const BROWSER_TOKEN_RECORD_BYTES: usize = 256 * 1024 * 1024;

pub const BROWSER_CAPACITIES: Capacities = Capacities {
    #[cfg(test)]
    test_expression_frames: None,
    #[cfg(test)]
    test_function_syntax_depth: None,
    #[cfg(test)]
    test_native_parser_depth: None,
    functions: 65_536,
    #[cfg(test)]
    programs:4096,
    #[cfg(test)]
    test_loader_program_count:None,
    environments: 65_536, bindings: 1_048_576,
    arguments: 65_536, source_bytes: 32 * 1024 * 1024, string_bytes: 8 * 1024 * 1024,
    tokens: BROWSER_TOKEN_RECORD_BYTES / std::mem::size_of::<super::Token>() - 1,
    ast_nodes: 6_291_456, compilation_bytes: 768 * 1024 * 1024,
    script_storage_bytes:BROWSER_HEAP.retained_values,
    retained_source: 256 * 1024 * 1024, retained_ast: 16_777_216, retained_regexp: 16 * 1024 * 1024,
    property_bytes: 8192, promises: 65_536, promise_jobs: 262_144,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostCapacities {
    pub string_bytes: usize,
    pub extra_nodes: usize,
    pub retained_bytes: usize,
    pub listeners: usize,
    pub event_type_bytes: usize,
    pub timers: usize,
    pub timer_arguments: usize,
    pub timer_bytes: usize,
    pub timer_callbacks: usize,
    pub created_events: usize,
    pub event_targets: usize,
    pub animation_frames: usize,
    pub media_tasks: usize,
    #[cfg(test)]
    pub attribute_identities: usize,
    pub node_lists: usize,
    pub node_list_references: usize,
}

#[cfg(test)]
pub const REGRESSION_HOST: HostCapacities = HostCapacities {
    string_bytes: 32 * 1024, extra_nodes: 256, retained_bytes: 1024 * 1024,
    listeners: 64, event_type_bytes: 256, timers: 64, timer_arguments: 30,
    timer_bytes: 256 * 1024, timer_callbacks: 4096, created_events: 256, event_targets: 256, animation_frames: 64, media_tasks: 64, attribute_identities: 256, node_lists: 128, node_list_references: 16_384,
};
pub const BROWSER_HOST: HostCapacities = HostCapacities {
    string_bytes: BROWSER_CAPACITIES.string_bytes, extra_nodes: 16_384,
    retained_bytes: 64 * 1024 * 1024, listeners: 4096, event_type_bytes: 16 * 1024,
    timers: 1024, timer_arguments: BROWSER_CAPACITIES.arguments,
    timer_bytes: 32 * 1024 * 1024, timer_callbacks: 65_536, created_events: 65_536, event_targets: 65_536, animation_frames: 65_536, media_tasks: 65_536,
    #[cfg(test)]
    attribute_identities: 65_536,
    node_lists: 65_536, node_list_references: 1_048_576,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    #[cfg(test)]
    pub(crate) test_entry_arenas: bool,
    #[cfg(test)]
    pub(crate) test_collection_entries: Option<(usize, usize)>,
    #[cfg(test)]
    pub(crate) test_call_depth: Option<usize>,
    #[cfg(test)]
    pub(crate) test_evaluation_frames: Option<usize>,
    #[cfg(test)]
    pub(crate) test_native_evaluation_depth: Option<usize>,
    pub capacity: Capacities,
    pub host: HostCapacities,
    pub heap: HeapLimits,
    pub steps: usize,
    pub copy_bytes: usize,
    pub regexp_work: usize,
    pub job_seconds: u64,
    pub host_work: usize,
    pub startup_steps: usize,
    pub startup_copy: usize,
    pub startup_regexp: usize,
    pub startup_host_work: usize,
    pub startup_seconds: u64,
}

// Admit the full CSS document's source charge with 256 Mi work units for
// selector matching, cascade, document structure and other native work.
const BROWSER_HOST_WORK: usize = crate::css::FULL_STYLESHEET_SOURCE_WORK + 256 * 1024 * 1024;

/// Default for every browser script, callback, timer and microtask checkpoint.
/// Copy bytes are cumulative charged work, not reserved or retained memory.
pub const BROWSER: Limits = Limits {
    #[cfg(test)]
    test_entry_arenas: false,
    #[cfg(test)]
    test_collection_entries: None,
    #[cfg(test)]
    test_call_depth: None,
    #[cfg(test)]
    test_evaluation_frames: None,
    #[cfg(test)]
    test_native_evaluation_depth: None,
    capacity: BROWSER_CAPACITIES,
    host: BROWSER_HOST,
    heap: BROWSER_HEAP,
    steps: 1_000_000_000,
    copy_bytes: 16 * 1024 * 1024 * 1024,
    regexp_work: 256_000_000,
    job_seconds: 30,
    host_work: BROWSER_HOST_WORK,
    startup_steps: 4_000_000_000,
    startup_copy: 64 * 1024 * 1024 * 1024,
    startup_regexp: 1_024_000_000,
    startup_host_work: BROWSER_HOST_WORK * 4,
    startup_seconds: 120,
};

/// Historical quota witnesses retain their original finite workload/clock
/// policy. This is explicit test selection, never the normal browser default.
#[cfg(test)]
pub(crate) const CAPACITY_REGRESSION: Limits = Limits {
    test_entry_arenas: true,
    test_collection_entries: Some((4096, 16384)),
    test_call_depth: Some(super::MAX_CALL_DEPTH),
    test_evaluation_frames: Some(super::MAX_EVALUATION_FRAMES),
    test_native_evaluation_depth: Some(super::MAX_EVAL_DEPTH),
    capacity: REGRESSION_CAPACITIES,
    host: REGRESSION_HOST,
    steps: 50_000_000,
    copy_bytes: 256 * 1024 * 1024,
    regexp_work: 32_000_000,
    job_seconds: 10,
    host_work: 2_000_000,
    startup_steps: 200_000_000,
    startup_copy: 1024 * 1024 * 1024,
    startup_regexp: 128_000_000,
    startup_host_work: 8_000_000,
    startup_seconds: 30,
    ..BROWSER
};

/// Explicit historical profile used by quota/adversarial regression fixtures.
/// Default-profile tests separately exercise BROWSER; production never selects
/// this constant, and compiling with cfg(test) does not change Runtime::new().
#[cfg(test)]
pub(crate) const REGRESSION: Limits = Limits {
    test_entry_arenas: true,
    test_collection_entries: Some((4096, 16384)),
    test_call_depth: Some(super::MAX_CALL_DEPTH),
    test_evaluation_frames: Some(super::MAX_EVALUATION_FRAMES),
    test_native_evaluation_depth: Some(super::MAX_EVAL_DEPTH),
    capacity: REGRESSION_CAPACITIES,
    host: REGRESSION_HOST,
    heap: REGRESSION_HEAP,
    steps: 250_000,
    copy_bytes: 8 * 1024 * 1024,
    regexp_work: 2_000_000,
    job_seconds: 1,
    host_work: 100_000,
    startup_steps: 1_000_000,
    startup_copy: 64 * 1024 * 1024,
    startup_regexp: 8_000_000,
    startup_host_work: 500_000,
    startup_seconds: 2,
};
