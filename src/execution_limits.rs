//! Host-selected execution and retained-heap budgets, separate from native-stack guards.
//! The browser default is deliberately generous. Smaller test budgets exercise
//! the same accounting/cancellation paths without expensive hostile loops.
//! No page API can select a profile or reset these counters.

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
    pub functions: usize,
    pub programs: usize,
    pub environments: usize,
    pub bindings: usize,
    pub arguments: usize,
    pub source_bytes: usize,
    pub string_bytes: usize,
    pub tokens: usize,
    pub ast_nodes: usize,
    pub compilation_bytes: usize,
    pub retained_source: usize,
    pub retained_ast: usize,
    pub retained_regexp: usize,
    pub property_bytes: usize,
    pub promises: usize,
    pub promise_jobs: usize,
}

#[cfg(test)]
pub const REGRESSION_CAPACITIES: Capacities = Capacities {
    functions: 1024, programs: 128, environments: 4096, bindings: 4096,
    arguments: 256, source_bytes: 512 * 1024, string_bytes: 128 * 1024,
    tokens: 131_072, ast_nodes: 131_072, retained_source: 2 * 1024 * 1024,
    compilation_bytes: 64 * 1024 * 1024,
    retained_ast: 262_144, retained_regexp: 2 * 1024 * 1024,
    property_bytes: 256, promises: 1024, promise_jobs: 4096,
};

const BROWSER_TOKEN_RECORD_BYTES: usize = 256 * 1024 * 1024;

pub const BROWSER_CAPACITIES: Capacities = Capacities {
    functions: 65_536, programs: 4096, environments: 65_536, bindings: 1_048_576,
    arguments: 65_536, source_bytes: 32 * 1024 * 1024, string_bytes: 8 * 1024 * 1024,
    tokens: BROWSER_TOKEN_RECORD_BYTES / std::mem::size_of::<super::Token>() - 1,
    ast_nodes: 6_291_456, compilation_bytes: 768 * 1024 * 1024,
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
    timer_bytes: 32 * 1024 * 1024, timer_callbacks: 65_536, created_events: 65_536, event_targets: 65_536, animation_frames: 65_536, media_tasks: 65_536, attribute_identities: 65_536, node_lists: 65_536, node_list_references: 1_048_576,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
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

/// Default for every browser script, callback, timer and microtask checkpoint.
/// Copy bytes are cumulative charged work, not reserved or retained memory.
pub const BROWSER: Limits = Limits {
    capacity: BROWSER_CAPACITIES,
    host: BROWSER_HOST,
    heap: BROWSER_HEAP,
    steps: 1_000_000_000,
    copy_bytes: 16 * 1024 * 1024 * 1024,
    regexp_work: 256_000_000,
    job_seconds: 30,
    host_work: 64_000_000,
    startup_steps: 4_000_000_000,
    startup_copy: 64 * 1024 * 1024 * 1024,
    startup_regexp: 1_024_000_000,
    startup_host_work: 256_000_000,
    startup_seconds: 120,
};

/// Historical quota witnesses retain their original finite workload/clock
/// policy. This is explicit test selection, never the normal browser default.
#[cfg(test)]
pub(crate) const CAPACITY_REGRESSION: Limits = Limits {
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
