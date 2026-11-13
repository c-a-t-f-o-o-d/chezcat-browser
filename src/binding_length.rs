//! Borrow a primitive binding only for an ordinary, literal length read.
//! The key cannot run user code. Other reads, calls and writes keep their
//! owned receiver snapshots and existing evaluation paths.

use super::{
    ExprId, ExprKind, Fault, Host, Program, PropertyKey, Reference, Runtime, Value,
    WorkKind, MAX_EVAL_DEPTH,
};
use std::rc::Rc;

impl Runtime {
    pub(super) fn eval_binding_length(
        &mut self,
        program: &Rc<Program>,
        object: ExprId,
        property: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let ExprKind::Name(name) = &program.exprs[object].kind else {
            unreachable!("length read requires a name receiver");
        };
        debug_assert!(matches!(
            &program.exprs[property].kind,
            ExprKind::Literal(Value::String(key)) if key == "length"
        ));

        // This descriptor cannot escape the read or enter a general Reference.
        // Resolve once: an ineligible binding continues with its owned value.
        enum Receiver {
            Binding { env: usize, bytes: usize },
            Owned(Value),
        }

        self.step()?; // reference construction
        self.step()?; // base-name eval, including its normal depth boundary
        if self.eval_depth >= MAX_EVAL_DEPTH {
            return Err(Fault::new("Evaluation nesting limit reached (128)."));
        }
        self.eval_depth += 1;
        let receiver = (|| {
            match self.lookup(env, name)? {
                Some(scope) => {
                    // Do not probe host ownership here: read_binding performs
                    // the original host queries exactly once on fallback.
                    let global_route = scope == 0
                        && self.browser_global.is_some()
                        && self.global_binding(name);
                    let current = self.environment(scope);
                    let pending_arguments = name == "arguments" && current.arguments.is_some();
                    if !global_route && !pending_arguments {
                        if let Some(Value::String(text)) = current.bindings[name].value.as_ref() {
                            return Ok(Receiver::Binding {
                                env: scope,
                                bytes: text.len(),
                            });
                        }
                    }
                    self.read_binding(scope, name, host).map(Receiver::Owned)
                }
                None if self.global_exists(name, host)? => {
                    self.global_value(name, host).map(Receiver::Owned)
                }
                None => Err(Fault::exception(
                    "ReferenceError",
                    format!("{name} is not defined."),
                )),
            }
        })();
        self.eval_depth -= 1;
        let receiver =
            receiver.map_err(|error| error.at(&program.source, program.exprs[object].at))?;
        let property = self.eval(program, property, env, this, host)?;
        match receiver {
            Receiver::Owned(value) => {
                self.read_reference(&Reference::ComputedProperty(value, property), host)
            }
            Receiver::Binding { env, bytes } => {
                self.step()?; // computed-reference read
                let property = self.copy_value(&property)?;
                let key = self.property_key(property, host)?;
                debug_assert!(matches!(key, PropertyKey::String(key) if key == "length"));
                self.step()?; // property read
                // Literal evaluation/key conversion cannot call out or mutate
                // this binding. A successful earlier read can reuse its count.
                // Hits still pay normal lookup/key/step costs, but do not scan.
                if let Some(length) = self.environment(env).bindings[name].utf16_length {
                    return Ok(Value::Number(length as f64));
                }
                self.work_as(bytes, WorkKind::LengthScan)?;

                // Charge the full cold scan before doing it or publishing the
                // memo. A failed charge leaves no cached successful result.
                let Some(Value::String(text)) =
                    self.environment(env).bindings[name].value.as_ref()
                else {
                    unreachable!("literal length read changed its binding");
                };
                debug_assert_eq!(text.len(), bytes);
                let length = text.encode_utf16().count();
                self.environment_mut(env).bindings.get_mut(name).unwrap().utf16_length = Some(length);
                Ok(Value::Number(length as f64))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Declaration, Parser, StmtKind, MAX_COPY_BYTES, MAX_REGEXP_WORK};
    use super::*;
    use std::cell::RefCell;

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            Err("Unexpected native read.".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("Unexpected native write.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("Unexpected native call.".into())
        }
    }

    fn expression(source: &str) -> (Rc<Program>, ExprId) {
        let mut regexp = MAX_REGEXP_WORK;
        let program = Rc::new(Parser::parse(source, &mut regexp).unwrap());
        let StmtKind::Expr(id) = &program.stmts[program.body[0]].kind else {
            panic!("expected one expression statement");
        };
        let id = *id;
        (program, id)
    }

    // Keep the original member/reference route as a test oracle. This is not
    // another runtime mode; it lets fallback tests compare steps and positions.
    fn original_member(
        runtime: &mut Runtime,
        program: &Rc<Program>,
        id: ExprId,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        runtime.step()?;
        if runtime.eval_depth >= MAX_EVAL_DEPTH {
            return Err(Fault::new("Evaluation nesting limit reached (128)."));
        }
        runtime.eval_depth += 1;
        let result = (|| {
            let reference = runtime.reference(program, id, 0, &Value::Undefined, host)?;
            runtime.read_reference(&reference, host)
        })();
        runtime.eval_depth -= 1;
        result.map_err(|error| error.at(&program.source, program.exprs[id].at))
    }

    fn member(
        runtime: &mut Runtime,
        program: &Rc<Program>,
        id: ExprId,
        original: bool,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if original {
            original_member(runtime, program, id, host)
        } else {
            runtime.eval(program, id, 0, &Value::Undefined, host)
        }
    }

    #[test]
    fn binding_length_original_semantic_fixtures() {
        for source in [
            include_str!("../tools/fixtures/string-work-design.js"),
            include_str!("../tools/fixtures/binding-length-reads.js"),
        ] {
            assert_eq!(
                Runtime::for_test().execute(source, &mut NoHost).unwrap().to_text(),
                "ok"
            );
        }
    }

    #[test]
    fn binding_length_removes_one_copy_and_keeps_all_expression_steps() {
        for source in [
            "payload.length;",
            "payload['length'];",
            "(payload).length;",
            "(payload.length);",
        ] {
            let (program, id) = expression(source);
            for text in ["", "abcd", "éAB", "éé", "😀", "A😀ΩZ"] {
                let mut observations = Vec::new();
                for original in [true, false] {
                    let mut runtime = Runtime::for_test();
                    runtime.define_global("payload", Value::String(text.into())).unwrap();
                    runtime.begin_job();
                    let value = member(&mut runtime, &program, id, original, &mut NoHost).unwrap();
                    assert_eq!(value.to_number(), text.encode_utf16().count() as f64);
                    assert_eq!(runtime.eval_depth, 0);
                    observations.push(runtime.job_usage());
                }
                assert_eq!(observations[0].0, observations[1].0, "{source}");
                assert_eq!(observations[0].1 - observations[1].1, text.len(), "{source}");
                assert_eq!(observations[0].2, observations[1].2, "{source}");
            }
        }
    }

    #[test]
    fn binding_length_exact_retained_probe_counts() {
        for (source, expected_work) in [
            ("payload.length;\n", 8269),
            ("payload['length'];\n", 8272),
            ("payload?.length;\n", 16462),
            ("payload;\n", 8238),
            ("payload.charCodeAt(0);\n", 8293),
        ] {
            let mut runtime = Runtime::for_test();
            runtime.define_global("payload", Value::String("A".repeat(8192))).unwrap();
            runtime.begin_job();
            runtime.execute(source, &mut NoHost).unwrap();
            assert_eq!(runtime.job_usage().1, expected_work, "{source}");
        }
    }

    #[test]
    fn binding_length_dynamic_keys_keep_the_original_payload_slope() {
        for source in ["payload[key];", "payload['len'+'gth'];", "payload?.length;"] {
            let mut usage = Vec::new();
            for size in [1024, 8192] {
                let mut runtime = Runtime::for_test();
                runtime.define_global("payload", Value::String("A".repeat(size))).unwrap();
                runtime.define_global("key", Value::String("length".into())).unwrap();
                runtime.begin_job();
                assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_number(), size as f64);
                usage.push(runtime.job_usage());
            }
            assert_eq!(usage[1].1 - usage[0].1, 2 * (8192 - 1024), "{source}");
            assert_eq!(usage[0].0, usage[1].0, "{source}");
        }
    }

    #[test]
    fn binding_length_fallback_errors_keep_lookup_steps_and_fault_positions() {
        for kind in ["missing", "tdz", "null", "undefined", "number"] {
            let (program, id) = expression("\n payload.length;");
            let mut observations = Vec::new();
            for original in [true, false] {
                let mut runtime = Runtime::for_test();
                match kind {
                    "missing" => (),
                    "tdz" => runtime
                        .put_binding(0, "payload".into(), None, Declaration::Let, false)
                        .unwrap(),
                    "null" => runtime.define_global("payload", Value::Null).unwrap(),
                    "undefined" => runtime.define_global("payload", Value::Undefined).unwrap(),
                    _ => runtime.define_global("payload", Value::Number(4.0)).unwrap(),
                }
                runtime.begin_job();
                let result = member(&mut runtime, &program, id, original, &mut NoHost)
                    .map(|value| value.to_text()).map_err(Fault::display);
                assert_eq!(runtime.eval_depth, 0);
                observations.push((result, runtime.job_usage()));
            }
            assert_eq!(observations[0], observations[1], "{kind}");
            if kind == "number" {
                assert_eq!(observations[1].0.as_ref().unwrap(), "undefined");
            } else {
                let message = observations[1].0.as_ref().unwrap_err();
                let expected = match kind {
                    "null" | "undefined" => "null or undefined",
                    "missing" => "payload is not defined",
                    _ => "before its initialization",
                };
                assert!(message.contains(expected), "{message}");
            }
        }
    }

    #[derive(Default)]
    struct CountingHost {
        owns: bool,
        fail: bool,
        events: RefCell<Vec<String>>,
    }
    impl Host for CountingHost {
        fn owns_global_property(&self, name: &str) -> bool {
            self.events.borrow_mut().push(format!("owns:{name}"));
            self.owns && name == "payload"
        }
        fn get(&mut self, _: u32, name: &str) -> Result<Value, String> {
            self.events.borrow_mut().push(format!("get:{name}"));
            if self.fail {
                Err("Host read rejected.".into())
            } else {
                Ok(Value::String("é😀".into()))
            }
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("Unexpected native write.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("Unexpected native call.".into())
        }
    }

    #[test]
    fn binding_length_browser_global_fallback_queries_host_only_as_before() {
        let (program, id) = expression("payload.length;");
        for (binding, owns, fail) in [
            (true, false, false),
            (true, true, false),
            (false, true, false),
            (true, true, true),
        ] {
            let mut observations = Vec::new();
            for original in [true, false] {
                let mut runtime = Runtime::for_test();
                runtime.bind_browser_global(7).unwrap();
                if binding {
                    runtime.define_global("payload", Value::String("A".repeat(8192))).unwrap();
                }
                let mut host = CountingHost { owns, fail, ..CountingHost::default() };
                runtime.begin_job();
                let result = member(&mut runtime, &program, id, original, &mut host)
                    .map(|value| value.to_text()).map_err(Fault::display);
                observations.push((result, runtime.job_usage(), host.events.into_inner()));
            }
            assert_eq!(observations[0], observations[1]);
            let expected = if owns {
                vec!["owns:payload", "owns:payload", "get:payload"]
            } else {
                vec!["owns:payload"]
            };
            assert_eq!(observations[1].2, expected);
        }
    }

    #[test]
    fn binding_length_browser_global_lexical_binding_never_queries_host() {
        let (program, id) = expression("payload.length;");
        let mut observations = Vec::new();
        for original in [true, false] {
            let mut runtime = Runtime::for_test();
            runtime.bind_browser_global(7).unwrap();
            runtime
                .put_binding(
                    0,
                    "payload".into(),
                    Some(Value::String("A".repeat(8192))),
                    Declaration::Let,
                    false,
                )
                .unwrap();
            let mut host = CountingHost { owns: true, ..CountingHost::default() };
            runtime.begin_job();
            assert_eq!(
                member(&mut runtime, &program, id, original, &mut host)
                    .unwrap()
                    .to_number(),
                8192.0
            );
            assert!(host.events.borrow().is_empty());
            observations.push(runtime.job_usage());
        }
        assert_eq!(observations[0].0, observations[1].0);
        assert_eq!(observations[0].1 - observations[1].1, 8192);
    }

    #[test]
    fn binding_length_base_name_depth_and_step_boundaries_match_reference_route() {
        let (program, id) = expression("\n payload['length'];");
        for seed in [0, MAX_EVAL_DEPTH - 2, MAX_EVAL_DEPTH - 1, MAX_EVAL_DEPTH] {
            for steps in [0, 1, 2, 3, 4, 5, 6, 7, 8, 64] {
                let mut observations = Vec::new();
                for original in [true, false] {
                    let mut runtime = Runtime::for_test();
                    runtime.define_global("payload", Value::String("é😀".into())).unwrap();
                    runtime.begin_job();
                    runtime.eval_depth = seed;
                    runtime.steps = steps;
                    let result = member(&mut runtime, &program, id, original, &mut NoHost)
                        .map(|value| value.to_text()).map_err(Fault::display);
                    assert_eq!(runtime.eval_depth, seed);
                    observations.push((result, runtime.steps));
                }
                assert_eq!(observations[0], observations[1], "depth={seed}, steps={steps}");
            }
        }
    }

    #[test]
    fn binding_length_scan_is_charged_before_a_result_can_be_returned() {
        let (program, id) = expression("payload.length;");
        // This evaluates an already parsed expression: lookup costs 7 bytes,
        // literal/key copies and conversion cost four lots of 6 bytes.
        let fixed = 7 + 4 * "length".len();
        for enough in [false, true] {
            let mut runtime = Runtime::for_test();
            runtime.define_global("payload", Value::String("A".repeat(8192))).unwrap();
            runtime.begin_job();
            runtime.copy_bytes = fixed + 8192 - usize::from(!enough);
            let result = runtime.eval(&program, id, 0, &Value::Undefined, &mut NoHost);
            if enough {
                assert_eq!(result.unwrap().to_number(), 8192.0);
            } else {
                let error = result.unwrap_err();
                assert!(error.exception.is_none());
                assert!(error.display().contains("String/copy work limit"));
            }
            assert_eq!(runtime.copy_bytes, 0);
            assert_eq!(runtime.eval_depth, 0);
            runtime.begin_job();
            assert_eq!(runtime.execute("6*7;", &mut NoHost).unwrap().to_text(), "42");
        }
    }

    #[test]
    fn binding_length_shared_limit_cancels_queued_work_and_recovers_on_one_mib_stack() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                for microtask in [false, true] {
                    // Both original literal-key witnesses now reuse one scan.
                    // Dynamic keys retain snapshots and must still exhaust the
                    // same budget; this does not weaken the cancellation check.
                    for (iterations, cached) in [(512, true), (1536, true), (1536, false)] {
                        let mut runtime = Runtime::for_test();
                        runtime.define_global("payload", Value::String("A".repeat(8192))).unwrap();
                        let setup = "var completed=0,total=0,tail=0,sentinel=0,key='length';";
                        let read = if cached { "payload.length" } else { "payload[key]" };
                        let scan = format!(
                            "while(completed<{iterations}){{total+={read};completed++;}}tail=1;total;"
                        );
                        let queued = "Promise.resolve().then(function(){sentinel++;});";
                        let source = if microtask {
                            format!("{setup}Promise.resolve().then(function(){{{scan}}});{queued}")
                        } else {
                            format!("{setup}{queued}{scan}")
                        };
                        runtime.begin_job();
                        let execution = runtime.execute(&source, &mut NoHost).map(|_| ());
                        let completion = if microtask || execution.is_ok() {
                            execution.unwrap();
                            runtime
                                .microtask_checkpoint(&mut NoHost)
                                .map(|messages| assert!(messages.is_empty()))
                        } else {
                            execution
                        };
                        if cached {
                            completion.unwrap();
                            assert!(runtime.job_usage().1 < MAX_COPY_BYTES);
                        } else {
                            let error = completion.unwrap_err();
                            assert!(error.contains("String/copy work limit"), "{error}");
                            assert_eq!(runtime.job_usage().1, MAX_COPY_BYTES);
                        }
                        assert!(!runtime.has_microtasks());
                        assert_eq!(runtime.eval_depth, 0);
                        assert_eq!(runtime.call_depth, 0);
                        runtime.begin_job();
                        let completed = runtime
                            .execute("completed;", &mut NoHost)
                            .unwrap()
                            .to_number();
                        let total = runtime.execute("total;", &mut NoHost).unwrap().to_number();
                        let tail = runtime.execute("tail;", &mut NoHost).unwrap().to_number();
                        let sentinel = runtime
                            .execute("sentinel;", &mut NoHost)
                            .unwrap()
                            .to_number();
                        if cached {
                            assert_eq!(total, completed * 8192.0);
                            assert_eq!(completed, iterations as f64);
                            assert_eq!((tail, sentinel), (1.0, 1.0));
                        } else {
                            assert!(completed > 0.0 && completed < 1536.0);
                            // A fatal charge can occur between the total and
                            // progress writes; earlier effects are not rolled back.
                            assert!(total == completed * 8192.0
                                || total == (completed + 1.0) * 8192.0);
                            assert_eq!((tail, sentinel), (0.0, 0.0));
                        }
                        assert_eq!(runtime.execute("21+21;", &mut NoHost).unwrap().to_text(), "42");
                    }
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn binding_length_memo_reuses_only_successful_scan_and_keeps_steps() {
        for text in ["", "ABCD", "éΩ", "A😀Z"] {
            for source in ["payload.length;\n", "payload['length'];\n"] {
                let mut runtime = Runtime::for_test();
                runtime.define_global("payload", Value::String(text.into())).unwrap();
                runtime.begin_job();
                let cold = runtime.execute(source, &mut NoHost).unwrap();
                let cold_usage = runtime.job_usage();
                runtime.begin_job(); // the immutable value, not the job, owns the memo
                let warm = runtime.execute(source, &mut NoHost).unwrap();
                let warm_usage = runtime.job_usage();
                assert_eq!(cold.to_number(), text.encode_utf16().count() as f64);
                assert_eq!(cold.to_number(), warm.to_number());
                assert_eq!(cold_usage.0, warm_usage.0);
                assert_eq!(cold_usage.1 - warm_usage.1, text.len());
                assert_eq!(runtime.environment(0).bindings["payload"].utf16_length,
                    Some(text.encode_utf16().count()));
            }
        }
    }

    #[test]
    fn binding_length_memo_resets_on_equal_byte_replacement_and_survives_failed_write() {
        let mut runtime = Runtime::for_test();
        runtime.define_global("payload", Value::String("ABCD".into())).unwrap();
        runtime.execute("payload.length;", &mut NoHost).unwrap();
        assert_eq!(runtime.environment(0).bindings["payload"].utf16_length, Some(4));
        runtime.begin_job();
        runtime.copy_bytes = 0;
        assert!(runtime.store_binding(0, "payload", Value::String("😀".into()), false).is_err());
        assert_eq!(runtime.environment(0).bindings["payload"].utf16_length, Some(4));
        runtime.begin_job();
        runtime.store_binding(0, "payload", Value::String("😀".into()), false).unwrap();
        assert_eq!(runtime.environment(0).bindings["payload"].utf16_length, None);
        assert_eq!(runtime.execute("payload.length;", &mut NoHost).unwrap().to_number(), 2.0);
        runtime.store_binding(0, "payload", Value::Number(7.0), false).unwrap();
        assert_eq!(runtime.environment(0).bindings["payload"].utf16_length, None);
        assert_eq!(runtime.execute("payload.length;", &mut NoHost).unwrap().to_text(), "undefined");
    }

    #[test]
    fn binding_length_failed_scan_does_not_publish_memo() {
        let (program, id) = expression("payload.length;");
        let mut runtime = Runtime::for_test();
        runtime.define_global("payload", Value::String("x".repeat(8192))).unwrap();
        runtime.begin_job();
        runtime.copy_bytes = 7 + 4 * "length".len() + 8191;
        assert!(runtime.eval(&program, id, 0, &Value::Undefined, &mut NoHost).is_err());
        assert_eq!(runtime.environment(0).bindings["payload"].utf16_length, None);
        runtime.begin_job();
        assert_eq!(runtime.execute("payload.length;", &mut NoHost).unwrap().to_number(), 8192.0);
        assert_eq!(runtime.environment(0).bindings["payload"].utf16_length, Some(8192));
    }

    #[test]
    fn binding_length_memo_keeps_alias_closure_argument_and_key_order_semantics() {
        assert_eq!(Runtime::for_test().execute(
            include_str!("../tools/fixtures/binding-length-memo.js"), &mut NoHost,
        ).unwrap().to_text(), "ok");
    }

    #[test]
    fn binding_length_memo_user_scale_original_workload_scans_once() {
        let mut runtime = Runtime::new();
        runtime.define_global("payload", Value::String("x".repeat(17_592))).unwrap();
        runtime.begin_job();
        let result = runtime.execute(
            "var total=0;for(var i=0;i<6530;i++)total+=payload.length;total;", &mut NoHost,
        ).unwrap();
        assert_eq!(result.to_number(), (17_592 * 6530) as f64);
        assert!(runtime.job_usage().1 < 4 * 1024 * 1024);
        assert_eq!(runtime.environment(0).bindings["payload"].utf16_length, Some(17_592));
        // One cached count is stored, not a growing table of payloads or keys.
        assert!(runtime.environment(0).bindings.len() < 256);
    }

    #[test]
    fn binding_length_original_attribution_loop_is_now_a_positive_witness() {
        let mut runtime = Runtime::for_test();
        runtime.define_global("payload", Value::String("PRIVATE_WORK_PAYLOAD".repeat(256))).unwrap();
        runtime.execute("var tail=0,sentinel=0;function burn(){var i=0;while(i<64){payload.length;i++;}tail=1;}", &mut NoHost).unwrap();
        runtime.begin_job();
        runtime.copy_bytes = 16384;
        runtime.execute("burn();tail;", &mut NoHost).unwrap();
        assert_eq!(runtime.execute("tail;", &mut NoHost).unwrap().to_number(), 1.0);
        assert!(runtime.string_work_failure_report().is_none());
    }

}
