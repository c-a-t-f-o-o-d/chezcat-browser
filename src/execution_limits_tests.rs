//! Production-budget tests are separate from the small adversarial profile.
use super::*;

#[derive(Default)]
struct Recorder {
    records: Vec<String>,
}

impl Host for Recorder {
    fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        Err("No native reads in budget fixture.".into())
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        Err("No native writes in budget fixture.".into())
    }
    fn call(&mut self, object: u32, method: &str, args: &[Value]) -> Result<Value, String> {
        if object == 1 && method == "record" {
            self.records.push(args.first().unwrap_or(&Value::Undefined).to_text());
            Ok(Value::Undefined)
        } else {
            Err("No native call capability in budget fixture.".into())
        }
    }
}

#[test]
fn browser_budget_is_the_default_even_when_compiling_tests() {
    let mut runtime = Runtime::new();
    assert_eq!(runtime.limits, limits::BROWSER);
    runtime.begin_job();
    assert_eq!(runtime.steps, 50_000_000);
    assert_eq!(runtime.copy_bytes, 256 * 1024 * 1024);
    assert_eq!(runtime.regexp_work, 32_000_000);
    assert_eq!(runtime.job_usage(), (0, 0, 0));
    assert!(runtime.job_deadline > Instant::now());
    assert_eq!(runtime.limits.job_seconds, 10);
    assert_eq!(Runtime::default().limits, limits::BROWSER);
    assert_eq!(Runtime::for_test().limits, limits::REGRESSION);
}

#[test]
fn browser_budget_accepts_the_users_previous_rejected_charge() {
    // Byte counts supplied by the user's attribution block. This replays charge
    // totals, not Google code, original invocation counts, or page execution.
    let categories = [
        (WorkKind::BindingCopy, 3_419_427),
        (WorkKind::ValueCopy, 171_161),
        (WorkKind::LengthScan, 3_279_448),
        (WorkKind::CharacterScan, 23_771),
        (WorkKind::ConversionOutput, 76_388),
        (WorkKind::Other, 1_410_794),
    ];
    for profile in [limits::REGRESSION, limits::BROWSER] {
        let mut runtime = Runtime::with_limits(profile);
        runtime.begin_job();
        for (kind, bytes) in categories {
            runtime.work_as(bytes, kind).unwrap();
        }
        assert_eq!(runtime.job_usage().1, 8_380_989);
        let result = runtime.work_as(17_738, WorkKind::BindingCopy);
        if profile == limits::REGRESSION {
            assert_eq!(profile.copy_bytes - 8_380_989, 7_619);
            assert!(result.unwrap_err().exception.is_none());
            assert!(runtime.string_work_failure_report().unwrap().contains("requested=17738"));
        } else {
            result.unwrap();
            assert_eq!(runtime.job_usage().1, 8_398_727);
            assert_eq!(runtime.copy_bytes, 260_036_729);
            assert!(runtime.string_work_failure_report().is_none());
        }
    }
}

#[test]
fn browser_budget_keeps_a_finite_exact_copy_boundary_without_allocating_it() {
    let mut runtime = Runtime::new();
    runtime.begin_job();
    runtime.work_as(limits::BROWSER.copy_bytes, WorkKind::Other).unwrap();
    runtime.work(0).unwrap();
    let error = runtime.work(1).unwrap_err();
    assert_eq!(error.message, "String/copy work limit reached (256 MiB per job).");
    assert!(error.exception.is_none());
    assert_eq!(runtime.copy_bytes, 0);
    assert_eq!(runtime.job_usage().1, limits::BROWSER.copy_bytes);
    assert!(runtime.string_work_failure_report().unwrap().contains("initial=268435456"));
    runtime.begin_job();
    assert_eq!(runtime.copy_bytes, limits::BROWSER.copy_bytes);
}

#[test]
fn browser_budget_constraining_a_job_never_changes_the_next_jobs_profile() {
    let mut runtime = Runtime::new();
    runtime.begin_job();
    let deadline = Instant::now() + Duration::from_millis(500);
    runtime.constrain_job(123, 456, 789, deadline);
    assert_eq!(runtime.job_allowance, (123, 456, 789));
    assert!(runtime.job_deadline <= deadline);
    // A subsequent constrain cannot replenish an exhausted/smaller allowance.
    runtime.constrain_job(5000, 5000, 5000, Instant::now() + Duration::from_secs(30));
    assert_eq!(runtime.job_allowance, (123, 456, 789));
    runtime.begin_job();
    assert_eq!(runtime.job_allowance, (50_000_000, 256 * 1024 * 1024, 32_000_000));
    assert_eq!(runtime.limits, limits::BROWSER);
}

#[test]
fn browser_budget_step_and_clock_guards_remain_independent() {
    let mut runtime = Runtime::new();
    runtime.begin_job();
    runtime.steps = 1;
    runtime.step().unwrap();
    let error = runtime.step().unwrap_err();
    assert_eq!(error.message, "Execution step limit reached (50000000 per job).");
    assert!(error.exception.is_none());
    runtime.begin_job();
    runtime.job_deadline = Instant::now() - Duration::from_secs(1);
    runtime.steps &= !255;
    let error = runtime.step().unwrap_err();
    assert_eq!(error.message, "Execution time limit reached (10 seconds per job).");
    assert!(error.exception.is_none());
    runtime.begin_job();
    runtime.step().unwrap();
}

#[test]
fn browser_budget_runs_original_length_work_beyond_eight_mib() {
    // Preserve the original literal-key workload as an optimized success.
    // The dynamic-key control still demonstrates the larger actual budget.
    for cached in [true, false] {
        let source = if cached {
            "var total=0;for(var i=0;i<1024;i++)total+=payload.length;total;"
        } else {
            "var total=0,key='length';for(var i=0;i<1024;i++)total+=payload[key];total;"
        };
        for profile in [limits::REGRESSION, limits::BROWSER] {
            let mut runtime = Runtime::with_limits(profile);
            let mut host = Recorder::default();
            runtime.define_global("payload", Value::String("x".repeat(16 * 1024))).unwrap();
            runtime.begin_job();
            let result = runtime.execute(source, &mut host);
            if profile == limits::REGRESSION && !cached {
                assert!(result.unwrap_err().contains("String/copy work limit"));
            } else {
                assert_eq!(result.unwrap().to_text(), "16777216");
                assert_eq!(runtime.job_usage().1 < limits::REGRESSION.copy_bytes, cached);
                assert!(runtime.job_usage().1 < limits::BROWSER.copy_bytes);
            }
        }
    }
}

#[test]
fn browser_budget_microtasks_share_the_enlarged_allowance_and_still_cancel() {
    let mut runtime = Runtime::new();
    let mut host = Recorder::default();
    runtime.define_global("probe", Value::Host(1)).unwrap();
    runtime.define_global("payload", Value::String("x".repeat(16 * 1024))).unwrap();
    runtime.begin_job();
    runtime.execute(concat!(
        "var total=0;Promise.resolve().then(function(){",
        "for(var i=0;i<1024;i++)total+=payload.length;",
        "probe.record(total);});"
    ), &mut host).unwrap();
    let before = runtime.job_usage().1;
    runtime.microtask_checkpoint(&mut host).unwrap();
    assert_eq!(host.records, vec!["16777216".to_owned()]);
    assert!(runtime.job_usage().1 > before);
    assert!(runtime.job_usage().1 < limits::REGRESSION.copy_bytes);
    // The original loop remains; it now reuses the successful first scan.
    assert!(runtime.copy_bytes < limits::BROWSER.copy_bytes);
    runtime.begin_job();
    host.records.clear();
    runtime.execute(concat!(
        "Promise.resolve().then(function(){try{payload.length;}catch(e){probe.record('caught');}",
        "finally{probe.record('finally');}});",
        "Promise.resolve().then(function(){probe.record('late');});"
    ), &mut host).unwrap();
    runtime.constrain_job(runtime.steps, 0, runtime.regexp_work, runtime.job_deadline);
    let error = runtime.microtask_checkpoint(&mut host).unwrap_err();
    assert!(error.contains("String/copy work limit"), "{error}");
    assert!(host.records.is_empty());
    assert!(!runtime.has_microtasks());
    runtime.begin_job();
    assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
    assert_eq!(runtime.limits, limits::BROWSER);
}

#[test]
fn browser_budget_preserves_native_and_single_string_limits() {
    assert_eq!(MAX_PARSE_DEPTH, 64);
    assert_eq!(MAX_EVAL_DEPTH, 128);
    assert_eq!(MAX_CALL_DEPTH, 32);
    assert_eq!(MAX_EVALUATION_FRAMES, 4096);
    assert_eq!(MAX_STRING_BYTES, 128 * 1024);
    assert_eq!(limits::REGRESSION_HEAP.retained_values, 2 * 1024 * 1024);
    assert_eq!(limits::BROWSER.heap, limits::BROWSER_HEAP);
    assert!(limits::BROWSER.startup_copy >= 2 * limits::BROWSER.copy_bytes);
    assert!(limits::BROWSER.startup_steps >= limits::BROWSER.steps);
    assert!(limits::BROWSER.startup_regexp >= limits::BROWSER.regexp_work);
    assert!(limits::BROWSER.startup_host_work >= limits::BROWSER.host_work);
    assert!(limits::BROWSER.startup_seconds >= limits::BROWSER.job_seconds);
}

#[test]
fn browser_step_allowance_crosses_the_previous_ceiling_without_resetting_the_job() {
    let mut runtime = Runtime::new();
    runtime.begin_job();
    let deadline = runtime.job_deadline;
    for _ in 0..5_000_001 { runtime.step().unwrap(); }
    assert_eq!(runtime.job_usage().0, 5_000_001);
    assert_eq!(runtime.steps, 44_999_999);
    assert_eq!(runtime.job_deadline, deadline);
    assert_eq!(runtime.job_allowance.0, 50_000_000);
    assert_eq!(runtime.limits.startup_steps, 200_000_000);
    // A larger work allowance does not disable or extend the clock guard.
    runtime.job_deadline = Instant::now() - Duration::from_secs(1);
    runtime.steps &= !255;
    assert!(runtime.step().unwrap_err().message.contains("time limit"));
    runtime.begin_job();
    assert_eq!(runtime.steps, 50_000_000);
}
