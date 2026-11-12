use super::*;
use super::super::{Host, Runtime, Value, MAX_COPY_BYTES, MAX_REGEXP_WORK, MAX_STEPS};
use std::time::{Duration, Instant};

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        Err("No native objects.".into())
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        Err("No native objects.".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        Err("No native objects.".into())
    }
}

fn constrain(runtime: &mut Runtime, copies: usize) {
    runtime.constrain_job(
        MAX_STEPS,
        copies,
        MAX_REGEXP_WORK,
        Instant::now() + Duration::from_secs(1),
    );
}

#[test]
fn attribution_partitions_success_and_keeps_rejected_work_separate() {
    let mut runtime = Runtime::for_test();
    runtime.begin_job();
    constrain(&mut runtime, 64);
    for (kind, bytes) in Kind::ALL.into_iter().zip([3, 5, 7, 11, 13, 17]) {
        runtime.work_as(bytes, kind).unwrap();
    }
    runtime.work(0).unwrap();
    let before = runtime.job_usage();
    assert_eq!(before.1, 56);
    let fault = runtime.work_as(9, Kind::LengthScan).unwrap_err();
    assert_eq!(fault.message, "String/copy work limit reached (8 MiB per job).");
    assert!(fault.exception.is_none());
    assert_eq!(runtime.copy_bytes, 0);
    assert_eq!(runtime.job_usage().1, 64);
    let snapshot = runtime.work_attribution.last_failure.unwrap();
    assert_eq!(snapshot.job.totals(), (Counter { bytes: 56, calls: 7 }, false));
    assert_eq!(snapshot.job.categories[Kind::Other as usize], Counter { bytes: 17, calls: 2 });
    assert_eq!(snapshot.job.last_rejected, Some(Rejected {
        kind: Kind::LengthScan, requested: 9, remaining_before: 8,
    }));
    runtime.work_as(2, Kind::ValueCopy).unwrap_err();
    let latest = runtime.work_attribution.last_failure.unwrap();
    assert_eq!(latest.job.first_rejected, snapshot.job.first_rejected);
    assert_eq!(latest.job.last_rejected, Some(Rejected {
        kind: Kind::ValueCopy, requested: 2, remaining_before: 0,
    }));
    assert_eq!(latest.job.rejected_calls, 2);
    assert_eq!(latest.job.totals().0.bytes, 56);
    assert_eq!(runtime.job_usage().0, before.0);
}

#[test]
fn attribution_constraint_rebasing_does_not_reset_successful_counters() {
    let mut runtime = Runtime::for_test();
    runtime.begin_job();
    constrain(&mut runtime, 100);
    runtime.work_as(20, Kind::ValueCopy).unwrap();
    constrain(&mut runtime, 30);
    runtime.work_as(7, Kind::LengthScan).unwrap();
    assert_eq!(runtime.job_usage().1, 7); // Existing rebased usage semantics.
    runtime.work(24).unwrap_err();
    let snapshot = runtime.work_attribution.last_failure.unwrap();
    assert_eq!(snapshot.job.totals().0.bytes, 27);
    assert_eq!(snapshot.job.initial_allowance, MAX_COPY_BYTES);
    assert_eq!(snapshot.job.constrained_away, MAX_COPY_BYTES - 50);
    assert_eq!(snapshot.job.constraint_calls, 2);
    assert_eq!(snapshot.usage_allowance, 30);
    assert_eq!(snapshot.job.last_rejected.unwrap().remaining_before, 23);
    assert_eq!(runtime.job_usage().1, 30);
    let report = runtime.string_work_failure_report().unwrap();
    assert!(report.contains("effective=50;"));
    assert!(report.contains("Successful charges: bytes=27;"));
    assert!(report.contains("job_usage copy charge=30;"));
}

#[test]
fn attribution_exact_allowance_zero_charges_and_overflow_requests() {
    let mut runtime = Runtime::for_test();
    runtime.begin_job();
    constrain(&mut runtime, 8);
    runtime.work_as(8, Kind::BindingCopy).unwrap();
    runtime.work(0).unwrap();
    assert!(runtime.string_work_failure_report().is_none());
    runtime.work_as(usize::MAX, Kind::Other).unwrap_err();
    let snapshot = runtime.work_attribution.last_failure.unwrap();
    assert_eq!(snapshot.job.totals().0, Counter { bytes: 8, calls: 2 });
    assert_eq!(snapshot.job.last_rejected.unwrap().requested, usize::MAX);
    assert_eq!(snapshot.job.last_rejected.unwrap().remaining_before, 0);
    assert_eq!(runtime.job_usage().1, 8);
    assert!(!snapshot.job.saturated);
}

#[test]
fn attribution_preserves_snapshot_and_reporting_does_not_charge() {
    let mut runtime = Runtime::for_test();
    runtime.begin_job();
    runtime.work_as(MAX_COPY_BYTES + 1, Kind::Other).unwrap_err();
    let snapshot = runtime.work_attribution.last_failure;
    let report = runtime.string_work_failure_report().unwrap();
    runtime.begin_job();
    assert_eq!(runtime.work_attribution.current.totals().0, Counter::default());
    runtime.execute("21+21;", &mut NoHost).unwrap();
    let usage = runtime.job_usage();
    for _ in 0..3 {
        assert_eq!(runtime.string_work_failure_report().as_deref(), Some(report.as_str()));
        assert_eq!(runtime.job_usage(), usage);
        assert_eq!(runtime.work_attribution.last_failure, snapshot);
    }
    runtime.work_as(MAX_COPY_BYTES + 1, Kind::ValueCopy).unwrap_err();
    assert_ne!(runtime.work_attribution.last_failure, snapshot);
    assert_eq!(runtime.work_attribution.last_failure.unwrap().job.number, 2);
}

#[test]
fn attribution_fixed_storage_saturates_and_report_is_bounded() {
    assert!(std::mem::size_of::<Tracker>() <= 1024);
    let mut tracker = Tracker::new(usize::MAX);
    tracker.current.number = usize::MAX;
    for kind in Kind::ALL {
        tracker.current.categories[kind as usize] = Counter {
            bytes: usize::MAX, calls: usize::MAX,
        };
        tracker.accept(kind, 1);
    }
    tracker.current.constraint_calls = usize::MAX;
    tracker.constrain(usize::MAX, 0);
    tracker.reject(Kind::ConversionOutput, usize::MAX, usize::MAX, usize::MAX);
    let snapshot = tracker.last_failure.unwrap();
    assert!(snapshot.job.saturated);
    let report = tracker.failure_report().unwrap();
    assert!(report.is_ascii());
    assert!(report.len() < MAX_REPORT_BYTES);
    assert!(report.contains("Numeric saturation=yes (counters are lower bounds)"));
    tracker.begin_job(8);
    assert_eq!(tracker.current.number, usize::MAX);
    assert!(tracker.current.saturated);
    assert_eq!(tracker.current.totals().0, Counter::default());
    assert_eq!(tracker.last_failure, Some(snapshot));
}

#[test]
fn attribution_keeps_original_probe_totals_and_disjoint_scan_categories() {
    for (source, expected, binding, length, character) in [
        ("payload;\n", 8238, 8192, 0, 0),
        ("payload.length;\n", 8269, 0, 8192, 0),
        ("payload['length'];\n", 8272, 0, 8192, 0),
        ("payload?.length;\n", 16462, 8192, 8192, 0),
        ("payload.charCodeAt(0);\n", 8293, 8192, 0, 1),
        ("payload[0];\n", 8245, 8192, 0, 1),
    ] {
        let mut runtime = Runtime::for_test();
        runtime.define_global("payload", Value::String("A".repeat(8192))).unwrap();
        runtime.begin_job();
        runtime.execute(source, &mut NoHost).unwrap();
        let job = &runtime.work_attribution.current;
        assert_eq!(runtime.job_usage().1, expected, "{source}");
        assert_eq!(job.totals().0.bytes, expected, "{source}");
        assert_eq!(job.categories[Kind::BindingCopy as usize].bytes, binding, "{source}");
        assert_eq!(job.categories[Kind::LengthScan as usize].bytes, length, "{source}");
        assert_eq!(job.categories[Kind::CharacterScan as usize].bytes, character, "{source}");
        assert!(runtime.string_work_failure_report().is_none());
    }
}

#[test]
fn attribution_high_character_reads_and_conversion_use_existing_charges() {
    for source in ["payload.charCodeAt(8191);", "payload[8191];"] {
        let mut runtime = Runtime::for_test();
        runtime.define_global("payload", Value::String("A".repeat(8192))).unwrap();
        runtime.begin_job();
        runtime.execute(source, &mut NoHost).unwrap();
        let job = &runtime.work_attribution.current;
        assert_eq!(job.categories[Kind::BindingCopy as usize].bytes, 8192);
        assert_eq!(job.categories[Kind::CharacterScan as usize], Counter { bytes: 8192, calls: 8192 });
        assert_eq!(job.totals().0.bytes, runtime.job_usage().1);
    }
    let mut runtime = Runtime::for_test();
    runtime.define_global("payload", Value::String("private-payload".into())).unwrap();
    runtime.begin_job();
    assert_eq!(runtime.execute("String(payload);", &mut NoHost).unwrap().to_text(), "private-payload");
    let job = &runtime.work_attribution.current;
    assert_eq!(job.categories[Kind::ConversionOutput as usize].bytes, "private-payload".len());
    assert_eq!(job.totals().0.bytes, runtime.job_usage().1);
}

#[test]
fn attribution_records_scan_rejection_before_any_scan_success() {
    let mut runtime = Runtime::for_test();
    runtime.begin_job();
    constrain(&mut runtime, 2);
    runtime.string_scalar_at("\u{1f600}", 0).unwrap_err();
    let snapshot = runtime.work_attribution.last_failure.unwrap();
    assert_eq!(snapshot.job.totals().0, Counter::default());
    assert_eq!(snapshot.job.last_rejected, Some(Rejected {
        kind: Kind::CharacterScan, requested: 4, remaining_before: 2,
    }));
    assert_eq!(runtime.job_usage().1, 2);
}

#[test]
fn attribution_original_order_getters_and_unicode_fixture() {
    assert_eq!(Runtime::for_test().execute(
        include_str!("../tools/fixtures/string-work-attribution.js"), &mut NoHost,
    ).unwrap().to_text(), "ok");
}

#[test]
fn attribution_shared_microtask_failure_cancels_and_recovers_on_one_mib_stack() {
    std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
        for microtask in [false, true] {
            let mut runtime = Runtime::for_test();
            runtime.define_global("payload", Value::String("PRIVATE_WORK_PAYLOAD".repeat(256))).unwrap();
            runtime.execute("var tail=0,sentinel=0;function burn(){var i=0;while(i<64){payload.length;i++;}tail=1;}", &mut NoHost).unwrap();
            runtime.begin_job();
            constrain(&mut runtime, 16384);
            let job_number = runtime.work_attribution.current.number;
            let error = if microtask {
                runtime.execute("Promise.resolve().then(burn);Promise.resolve().then(function(){sentinel=1;});", &mut NoHost).unwrap();
                let bytes = runtime.work_attribution.current.totals().0.bytes;
                assert!(bytes > 0);
                let error = runtime.microtask_checkpoint(&mut NoHost).unwrap_err();
                assert_eq!(runtime.work_attribution.current.number, job_number);
                assert!(runtime.work_attribution.current.totals().0.bytes >= bytes);
                error
            } else {
                runtime.execute("Promise.resolve().then(function(){sentinel=1;});burn();", &mut NoHost).unwrap_err()
            };
            assert!(error.contains("String/copy work limit"), "{error}");
            assert!(!runtime.has_microtasks());
            assert_eq!((runtime.eval_depth, runtime.call_depth), (0, 0));
            let snapshot = runtime.work_attribution.last_failure.unwrap();
            assert_eq!(snapshot.job.number, job_number);
            assert!(snapshot.job.categories[Kind::LengthScan as usize].bytes > 0);
            assert_eq!(runtime.job_usage().1, 16384);
            let report = runtime.string_work_failure_report().unwrap();
            assert!(!report.contains("PRIVATE_WORK_PAYLOAD"));
            assert!(!report.contains("Promise.resolve"));
            runtime.begin_job();
            assert_eq!(runtime.execute("tail+sentinel;", &mut NoHost).unwrap().to_number(), 0.0);
            assert_eq!(runtime.execute("21+21;", &mut NoHost).unwrap().to_number(), 42.0);
            assert_eq!(runtime.string_work_failure_report().as_deref(), Some(report.as_str()));
        }
    }).unwrap().join().unwrap();
}
