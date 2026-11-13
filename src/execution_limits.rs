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
    objects: 65_536,
    properties: 1_048_576,
    object_properties: 262_144,
    array_length: 1_048_576,
    retained_values: 64 * 1024 * 1024,
};

/// Explicit small profile for unit tests and standalone development probes.
/// Compiling tests does not change Runtime::new() or the browser default.
pub const REGRESSION_HEAP: HeapLimits = HeapLimits {
    objects: 4096,
    properties: 16_384,
    object_properties: 4096,
    array_length: 8192,
    retained_values: 2 * 1024 * 1024,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
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
    heap: BROWSER_HEAP,
    steps: 50_000_000,
    copy_bytes: 256 * 1024 * 1024,
    regexp_work: 32_000_000,
    job_seconds: 10,
    host_work: 2_000_000,
    startup_steps: 200_000_000,
    startup_copy: 1024 * 1024 * 1024,
    startup_regexp: 128_000_000,
    startup_host_work: 10_000_000,
    startup_seconds: 30,
};

/// Explicit historical profile used by quota/adversarial regression fixtures.
/// Default-profile tests separately exercise BROWSER; production never selects
/// this constant, and compiling with cfg(test) does not change Runtime::new().
#[cfg(test)]
pub(crate) const REGRESSION: Limits = Limits {
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
