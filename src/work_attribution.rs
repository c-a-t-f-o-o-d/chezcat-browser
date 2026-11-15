//! Fixed-size attribution of the existing interpreter copy/work allowance.
//! These are charged work units, not measured allocations or elapsed time.
//! Successful charges are disjoint. Untagged charge sites remain in Other.
//! No string payload, property name, URL, cookie, or script source is retained.

use std::fmt::Write;

const CATEGORY_COUNT: usize = 22;
const MAX_REPORT_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub(super) enum Kind {
    BindingCopy,
    ValueCopy,
    LengthScan,
    CharacterScan,
    ConversionOutput,
    Other,
    BindingStore,
    LookupName,
    PropertyName,
    PropertyStore,
    BinaryInspection,
    ConcatenationScratch,
    ScopeNames,
    ConditionalTarget,
    PropertyEnumeration,
    StringMethodInput,
    NativeInput,
    NumberScratch,
    ArgumentCapture,
    ProgramCompilation,
    PatternPreparation,
    ArrayJoinPreflight,
}

impl Kind {
    const ALL: [Self; CATEGORY_COUNT] = [
        Self::BindingCopy,
        Self::ValueCopy,
        Self::LengthScan,
        Self::CharacterScan,
        Self::ConversionOutput,
        Self::Other,
        Self::BindingStore,
        Self::LookupName,
        Self::PropertyName,
        Self::PropertyStore,
        Self::BinaryInspection,
        Self::ConcatenationScratch,
        Self::ScopeNames,
        Self::ConditionalTarget,
        Self::PropertyEnumeration,
        Self::StringMethodInput,
        Self::NativeInput,
        Self::NumberScratch,
        Self::ArgumentCapture,
        Self::ProgramCompilation,
        Self::PatternPreparation,
        Self::ArrayJoinPreflight,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::BindingCopy => "binding-payload-copy",
            Self::ValueCopy => "value-payload-copy",
            Self::LengthScan => "utf16-length-scan",
            Self::CharacterScan => "character-index-scan",
            Self::ConversionOutput => "conversion-output",
            Self::Other => "other",
            Self::BindingStore => "binding-store",
            Self::LookupName => "lookup-name",
            Self::PropertyName => "property-name",
            Self::PropertyStore => "property-store",
            Self::BinaryInspection => "binary-inspection",
            Self::ConcatenationScratch => "concatenation-preflight",
            Self::ScopeNames => "scope-name-preparation",
            Self::ConditionalTarget => "conditional-target-copy",
            Self::PropertyEnumeration => "property-enumeration",
            Self::StringMethodInput => "string-method-input",
            Self::NativeInput => "native-input-preflight",
            Self::NumberScratch => "number-radix-scratch",
            Self::ArgumentCapture => "argument-capture",
            Self::ProgramCompilation => "program-compilation",
            Self::PatternPreparation => "pattern-name-preparation",
            Self::ArrayJoinPreflight => "array-join-preflight",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Counter {
    bytes: usize,
    calls: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Rejected {
    kind: Kind,
    requested: usize,
    remaining_before: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Job {
    number: usize,
    initial_allowance: usize,
    constrained_away: usize,
    constraint_calls: usize,
    categories: [Counter; CATEGORY_COUNT],
    rejected_calls: usize,
    first_rejected: Option<Rejected>,
    last_rejected: Option<Rejected>,
    saturated: bool,
}

impl Job {
    fn new(number: usize, allowance: usize, saturated: bool) -> Self {
        Self {
            number,
            initial_allowance: allowance,
            constrained_away: 0,
            constraint_calls: 0,
            categories: [Counter::default(); CATEGORY_COUNT],
            rejected_calls: 0,
            first_rejected: None,
            last_rejected: None,
            saturated,
        }
    }

    fn totals(&self) -> (Counter, bool) {
        let mut total = Counter::default();
        let mut saturated = self.saturated;
        for counter in self.categories {
            add(&mut total.bytes, counter.bytes, &mut saturated);
            add(&mut total.calls, counter.calls, &mut saturated);
        }
        (total, saturated)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Failure {
    job: Job,
    // constrain_job rebases this existing usage counter. Do not confuse it
    // with successful work since begin_job or invent successful filler bytes.
    usage_allowance: usize,
}

pub(super) struct Tracker {
    current: Job,
    last_failure: Option<Failure>,
}

impl Tracker {
    pub(super) fn new(allowance: usize) -> Self {
        Self {
            current: Job::new(0, allowance, false),
            last_failure: None,
        }
    }

    pub(super) fn begin_job(&mut self, allowance: usize) {
        let number = self.current.number.saturating_add(1);
        let saturated = self.current.number == usize::MAX;
        self.current = Job::new(number, allowance, saturated);
        // A later successful job must not overwrite the last failed job.
    }

    pub(super) fn constrain(&mut self, before: usize, after: usize) {
        add(&mut self.current.constraint_calls, 1, &mut self.current.saturated);
        add(
            &mut self.current.constrained_away,
            before.saturating_sub(after),
            &mut self.current.saturated,
        );
    }

    pub(super) fn accept(&mut self, kind: Kind, bytes: usize) {
        let counter = &mut self.current.categories[kind as usize];
        add(&mut counter.bytes, bytes, &mut self.current.saturated);
        add(&mut counter.calls, 1, &mut self.current.saturated);
    }

    #[inline(never)]
    pub(super) fn reject(
        &mut self,
        kind: Kind,
        requested: usize,
        remaining_before: usize,
        usage_allowance: usize,
    ) {
        let rejected = Rejected {
            kind,
            requested,
            remaining_before,
        };
        self.current.first_rejected.get_or_insert(rejected);
        self.current.last_rejected = Some(rejected);
        add(&mut self.current.rejected_calls, 1, &mut self.current.saturated);
        self.last_failure = Some(Failure {
            job: self.current,
            usage_allowance,
        });
    }

    pub(super) fn failure_report(&self) -> Option<String> {
        let snapshot = self.last_failure.as_ref()?;
        let job = &snapshot.job;
        let (total, saturated) = job.totals();
        let mut text = String::with_capacity(MAX_REPORT_BYTES);
        let _ = writeln!(text, "String/copy work attribution: last failed job {} (retained snapshot).", job.number);
        let _ = writeln!(text, "Work units, not memory/time. Job 0 includes initialization; checkpoints share the job.");
        let _ = writeln!(text, "Allowance: initial={}; effective={}; constrained-away={}; constraint-calls={}.", job.initial_allowance, job.initial_allowance.saturating_sub(job.constrained_away), job.constrained_away, job.constraint_calls);
        let _ = writeln!(text, "Successful charges: bytes={}; calls={}.", total.bytes, total.calls);
        for kind in Kind::ALL {
            let counter = job.categories[kind as usize];
            let _ = writeln!(text, "  {}: bytes={}; calls={}.", kind.label(), counter.bytes, counter.calls);
        }
        for (label, rejected) in [
            ("First rejected", job.first_rejected),
            ("Last rejected", job.last_rejected),
        ] {
            if let Some(rejected) = rejected {
                let _ = writeln!(text, "{label}: category={}; requested={}; remaining-before={}.", rejected.kind.label(), rejected.requested, rejected.remaining_before);
            }
        }
        let _ = writeln!(text, "Rejected calls={}; remaining-after=0; job_usage copy charge={}; usage-allowance={}.", job.rejected_calls, snapshot.usage_allowance, snapshot.usage_allowance);
        let _ = writeln!(text, "Rejected bytes are not successful. Exhaustion or constrain_job rebasing can make job_usage differ.");
        let _ = writeln!(text, "Calls are charge invocations (zero-byte and per-scalar included), not JS calls. Other is untagged; coverage is partial.");
        let _ = writeln!(text, "Numeric saturation={}. One retained snapshot; no payloads.", if saturated { "yes (counters are lower bounds)" } else { "no" });
        // All labels and numeric formatting above are ASCII. Bound even the
        // report allocation/output independently of page-controlled content.
        debug_assert!(text.len() < MAX_REPORT_BYTES);
        text.truncate(MAX_REPORT_BYTES);
        Some(text)
    }
}

fn add(target: &mut usize, amount: usize, saturated: &mut bool) {
    if let Some(sum) = target.checked_add(amount) {
        *target = sum;
    } else {
        *target = usize::MAX;
        *saturated = true;
    }
}

#[cfg(test)]
#[path = "work_attribution_tests.rs"]
mod tests;
