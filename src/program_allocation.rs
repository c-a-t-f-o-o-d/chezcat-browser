//! Registry-only program retirement and bounded numeric guard attribution.
use super::*;

#[derive(Default)]
pub(super) struct State {
    pub(super) passes: usize,
    pub(super) retired: usize,
    #[cfg(test)]
    pub(super) disabled: bool,
}

#[cfg(test)]
mod retirement_tests {
    use super::*;
    #[derive(Default)]
    struct NoHost {
        denied: bool,
    }
    impl Host for NoHost {
        fn allow_eval(&mut self) -> Result<bool, String> {
            Ok(!self.denied)
        }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            panic!("host get")
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            panic!("host set")
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            panic!("host call")
        }
    }
    fn totals(runtime: &Runtime) -> (usize, usize, usize) {
        (
            runtime.programs.len(),
            runtime.retained_source,
            runtime.retained_ast,
        )
    }
    fn consistent(runtime: &Runtime) {
        assert_eq!(
            runtime.retained_source,
            runtime
                .programs
                .iter()
                .map(|p| p.source.len())
                .sum::<usize>()
        );
        assert_eq!(
            runtime.retained_ast,
            runtime
                .programs
                .iter()
                .map(|p| p.node_count())
                .sum::<usize>()
        );
        assert!(
            runtime.programs.len() <= MAX_PROGRAMS
                && runtime.retained_source <= MAX_RETAINED_SOURCE
                && runtime.retained_ast <= MAX_RETAINED_AST
        );
    }
    fn churn(runtime: &mut Runtime) {
        for i in 0..300 {
            runtime.begin_job();
            runtime.register_program(&format!("{i};"), false).unwrap();
        }
        consistent(runtime);
        assert!(runtime.program_allocation.retired >= MAX_PROGRAMS);
    }

    #[test]
    fn program_retirement_distinct_live_count_and_source_caps_preserve_callbacks() {
        for source_cap in [false, true] {
            let mut runtime = Runtime::for_test();
            // This witness isolates the legacy count/source policies.
            runtime.limits.heap.retained_values=64*1024*1024;
            let mut host = NoHost::default();
            let count = if source_cap {
                MAX_RETAINED_SOURCE / MAX_SOURCE_BYTES
            } else {
                MAX_PROGRAMS
            };
            let mut saved = Vec::new();
            for i in 0..count {
                let mut source = format!("/*{i}*/(function(){{return {i};}});");
                if source_cap {
                    source.push_str(&" ".repeat(MAX_SOURCE_BYTES - source.len()));
                }
                runtime.begin_job();
                saved.push(runtime.execute(&source, &mut host).unwrap());
            }
            let before = totals(&runtime);
            runtime.begin_job();
            let error = runtime
                .execute("var overflowEffect=1;", &mut host)
                .unwrap_err();
            assert!(error.contains("Retained script limit"), "{error}");
            assert_eq!(totals(&runtime), before);
            assert_eq!(runtime.program_allocation.retired, 0);
            for i in [0, count - 1] {
                runtime.begin_job();
                assert_eq!(
                    runtime
                        .invoke(saved[i].clone(), &[], Value::Undefined, &mut host)
                        .unwrap()
                        .to_number(),
                    i as f64
                );
            }
            consistent(&runtime);
        }
    }

    #[test]
    fn program_retirement_distinct_live_ast_cap_is_atomic_on_one_mib_stack() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                let mut runtime = Runtime::for_capacity_test();
                let mut saved = Vec::new();
                for i in 0..2 {
                    let prefix = format!("var kept=(function(){{return {i};}});");
                    let base =
                        Parser::parse(&format!("{prefix}kept;"), &mut MAX_REGEXP_WORK.clone())
                            .unwrap()
                            .node_count();
                    let remaining = MAX_AST - base;
                    let source = format!(
                        "{prefix}{}{}kept;",
                        if remaining % 2 == 1 { ";" } else { "" },
                        "0\n".repeat(remaining / 2),
                    );
                    assert_eq!(
                        Parser::parse(&source, &mut MAX_REGEXP_WORK.clone())
                            .unwrap()
                            .node_count(),
                        MAX_AST
                    );
                    runtime.begin_job();
                    saved.push(runtime.execute(&source, &mut NoHost::default()).unwrap());
                }
                assert_eq!(runtime.retained_ast, MAX_RETAINED_AST);
                let before = totals(&runtime);
                runtime.begin_job();
                let error = runtime
                    .execute("var overflowEffect=1;", &mut NoHost::default())
                    .unwrap_err();
                assert!(error.contains("Retained AST limit"), "{error}");
                assert_eq!(totals(&runtime), before);
                assert_eq!(runtime.program_allocation.retired, 0);
                for (i, callback) in saved.into_iter().enumerate() {
                    runtime.begin_job();
                    assert_eq!(
                        runtime
                            .invoke(callback, &[], Value::Undefined, &mut NoHost::default())
                            .unwrap()
                            .to_number(),
                        i as f64
                    );
                }
                consistent(&runtime);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn program_retirement_protects_active_eval_and_preserves_fresh_effects() {
        let mut runtime = Runtime::for_test();
        let source =
            "var n=0;for(var i=0;i<300;i++)eval('n++;');if(n!==300)throw 'state';'outer-complete';";
        assert_eq!(
            runtime
                .execute(source, &mut NoHost::default())
                .unwrap()
                .to_text(),
            "outer-complete"
        );
        assert!(runtime.programs.iter().any(|p| p.source == source));
        assert!(runtime.program_allocation.retired >= MAX_PROGRAMS);
        consistent(&runtime);
        assert_eq!((runtime.eval_depth, runtime.call_depth), (0, 0));
    }

    #[test]
    fn program_retirement_keeps_unicode_function_source_identity_and_escaped_regexp() {
        let mut runtime = Runtime::for_test();
        let text = "function original(){/*雪*/return 19;}";
        let callback = runtime
            .execute(
                &format!("var keep={text};var r=/a+/g;keep;"),
                &mut NoHost::default(),
            )
            .unwrap();
        let live = Rc::downgrade(runtime.programs.last().unwrap());
        runtime
            .execute("var escaped=/a+/g;escaped;", &mut NoHost::default())
            .unwrap();
        let dead = Rc::downgrade(runtime.programs.last().unwrap());
        let regexp_bytes = runtime.retained_regexp;
        churn(&mut runtime);
        assert!(live.upgrade().is_some());
        assert!(dead.upgrade().is_none());
        assert_eq!(runtime.retained_regexp, regexp_bytes);
        runtime.begin_job();
        assert_eq!(
            runtime
                .invoke(callback, &[], Value::Undefined, &mut NoHost::default())
                .unwrap()
                .to_number(),
            19.0
        );
        assert_eq!(
            runtime
                .execute("keep.toString();", &mut NoHost::default())
                .unwrap()
                .to_text(),
            text
        );
        assert_eq!(runtime.execute("escaped.exec('aa aa')[0]+':'+escaped.lastIndex+':'+escaped.exec('aa aa')[0]+':'+escaped.lastIndex+':'+escaped.exec('aa aa')+':'+escaped.lastIndex;", &mut NoHost::default()).unwrap().to_text(), "aa:2:aa:5:null:0");
        assert_eq!(runtime.execute("var a=Function('return 21;'),b=Function('return 21;');a!==b&&a()===21&&b()===21&&a.toString()===b.toString();", &mut NoHost::default()).unwrap().to_text(), "true");
        consistent(&runtime);
    }

    #[test]
    fn program_retirement_step_deadline_and_accounting_failures_are_atomic() {
        for mode in 0..4 {
            let mut runtime = Runtime::for_test();
            for _ in 0..MAX_PROGRAMS {
                runtime.register_program("1;", false).unwrap();
            }
            let before = totals(&runtime);
            let pointers: Vec<_> = runtime.programs.iter().map(Rc::as_ptr).collect();
            let copy = runtime.copy_bytes;
            match mode {
                0 => runtime.steps = 1,
                1 => runtime.steps = MAX_PROGRAMS + 2,
                2 => {
                    runtime.steps = 99999;
                    runtime.job_deadline = Instant::now() - Duration::from_millis(1);
                }
                _ => runtime.retained_source = 0,
            }
            let expected = totals(&runtime);
            let error = runtime.prepare_program_capacity(1, 1, true).unwrap_err();
            assert!(
                error.message.contains(if mode == 2 {
                    "time limit"
                } else if mode == 3 {
                    "accounting mismatch"
                } else {
                    "step limit"
                }),
                "{}",
                error.message
            );
            assert_eq!(totals(&runtime), expected);
            assert_eq!(
                runtime.programs.iter().map(Rc::as_ptr).collect::<Vec<_>>(),
                pointers
            );
            assert_eq!(runtime.copy_bytes, copy);
            assert_eq!(runtime.program_allocation.retired, 0);
            if mode == 3 {
                runtime.retained_source = before.1;
            }
            runtime.begin_job();
            runtime.register_program("2;", false).unwrap();
            assert_eq!(runtime.program_allocation.retired, MAX_PROGRAMS);
            consistent(&runtime);
        }
    }

    #[test]
    fn program_retirement_rechecks_parsed_and_dynamic_validation_guards() {
        let mut runtime = Runtime::for_capacity_test();
        let dense = "0\n".repeat(MAX_AST / 2);
        runtime.register_program(&dense, false).unwrap();
        runtime.register_program(&dense, false).unwrap();
        assert_eq!(runtime.retained_ast, MAX_RETAINED_AST);
        runtime.begin_job();
        let function = runtime
            .function_constructor(&[Value::String("return 9;".into())], &mut NoHost::default())
            .unwrap();
        assert_eq!(runtime.program_allocation.retired, 2);
        assert_eq!(
            runtime
                .invoke(function, &[], Value::Undefined, &mut NoHost::default())
                .unwrap()
                .to_number(),
            9.0
        );
        for _ in runtime.programs.len()..MAX_PROGRAMS {
            runtime.register_program("0;", false).unwrap();
        }
        let parsed = Rc::new(Parser::parse("41;", &mut MAX_REGEXP_WORK.clone()).unwrap());
        runtime.retain_parsed_program(parsed.clone()).unwrap();
        assert!(runtime.programs.iter().any(|p| Rc::ptr_eq(p, &parsed)));
        assert!(runtime.program_allocation.retired >= MAX_PROGRAMS);
        consistent(&runtime);
    }

    #[test]
    fn program_retirement_source_pressure_spends_work_without_refunding_or_resetting() {
        let mut runtime = Runtime::for_capacity_test();
        let source = format!("/*{}*/", "x".repeat(MAX_SOURCE_BYTES - 4));
        for _ in 0..MAX_RETAINED_SOURCE / MAX_SOURCE_BYTES {
            runtime.register_program(&source, false).unwrap();
        }
        let before = (runtime.steps, runtime.copy_bytes, runtime.job_deadline);
        runtime.prepare_program_capacity(1, 0, true).unwrap();
        assert_eq!(totals(&runtime), (0, 0, 0));
        assert!(runtime.steps < before.0 - MAX_RETAINED_SOURCE);
        assert_eq!(
            (runtime.copy_bytes, runtime.job_deadline),
            (before.1, before.2)
        );
        assert_eq!(runtime.program_allocation.retired, 4);
        // A later compile is parsed and charged again, with fresh Program identity.
        let first = runtime.register_program("41;", false).unwrap();
        let copy = runtime.copy_bytes;
        let second = runtime.register_program("41;", false).unwrap();
        assert!(!Rc::ptr_eq(&first, &second));
        assert_eq!(runtime.copy_bytes, copy - 3);
        consistent(&runtime);
    }

    #[test]
    fn program_retirement_keeps_fresh_strict_handler_fragment_and_csp_checks() {
        let mut runtime = Runtime::for_test();
        churn(&mut runtime);
        assert!(runtime
            .register_program_context("with({}){}", false, true)
            .is_err());
        assert!(runtime
            .register_program("(function(wrong){return 1;})", true)
            .is_err());
        runtime
            .register_program("(function(event){return 1;})", true)
            .unwrap();
        assert!(runtime
            .function_constructor(
                &[
                    Value::String("a/*".into()),
                    Value::String("*/return 1;".into())
                ],
                &mut NoHost::default()
            )
            .is_err());
        // Denial occurs before cleanup or parsing, even with registry pressure.
        for _ in runtime.programs.len()..MAX_PROGRAMS {
            runtime.register_program("0;", false).unwrap();
        }
        let before = (
            totals(&runtime),
            runtime.program_allocation.passes,
            runtime.functions.len(),
        );
        let error = runtime
            .function_constructor(
                &[Value::String("return 9;".into())],
                &mut NoHost { denied: true },
            )
            .unwrap_err();
        assert!(
            matches!(error.exception.as_deref(), Some(Exception::Error("EvalError")))
                && error.message.contains("Content Security Policy"),
            "{error:?}"
        );
        assert_eq!(
            (
                totals(&runtime),
                runtime.program_allocation.passes,
                runtime.functions.len()
            ),
            before
        );
        consistent(&runtime);
    }
}

impl Runtime {
    pub(super) fn program_count_limit(&self)->Option<usize> {
        #[cfg(test)]
        if self.limits.test_entry_arenas{return Some(self.limits.capacity.programs);}
        None
    }
    pub(super) fn program_count_refused(&self)->bool {
        self.program_count_limit().is_some_and(|limit|self.programs.len()>=limit)
    }
    pub(super) fn ast_count_refused(&self,nodes:usize)->bool {
        #[cfg(test)]
        if self.limits.test_entry_arenas{return self.retained_ast.saturating_add(nodes)>self.limits.capacity.retained_ast;}
        #[cfg(not(test))]
        let _=nodes;
        false
    }
    fn program_registry_pressure(&self)->bool {
        if self.program_count_limit().is_some(){return self.program_count_refused();}
        self.programs.len()==self.programs.capacity()
    }
    // A Program has no Rc<Program> edges. Functions and active evaluators own
    // independent Rc handles; the registry is the sole owner of dead entries.
    // No parser result, function identity, environment or regexp charge is reused.
    #[inline(never)]
    pub(super) fn prepare_program_capacity(
        &mut self,
        source_bytes: usize,
        nodes: usize,
        needs_slot: bool,
    ) -> Result<(), Fault> {
        self.prepare_program_storage(source_bytes,nodes,needs_slot,0)
    }
    pub(super) fn prepare_program_storage(&mut self,source_bytes:usize,nodes:usize,needs_slot:bool,values:usize)->Result<(),Fault> {
        #[cfg(test)]
        if self.program_allocation.disabled {
            return Ok(());
        }
        if !(needs_slot && self.program_registry_pressure()
            || self.retained_source.saturating_add(source_bytes) > self.limits.capacity.retained_source
            || self.ast_count_refused(nodes)
            || values>self.limits.heap.retained_values.saturating_sub(self.value_bytes))
        {
            return Ok(());
        }

        let mut source = 0usize;
        let mut ast = 0usize;
        let mut retired = 0usize;
        let mut payload=0usize;
        for index in 0..self.programs.len() {
            self.step()?;
            let program = &self.programs[index];
            if Rc::strong_count(program) == 1 {
                source = source
                    .checked_add(program.source.len())
                    .ok_or_else(|| Fault::new("Program retirement accounting overflow."))?;
                ast = ast
                    .checked_add(program.node_count())
                    .ok_or_else(|| Fault::new("Program retirement accounting overflow."))?;
                payload=payload.checked_add(program.retained_bytes.get()).ok_or_else(||Fault::new("Program payload retirement accounting overflow."))?;
                retired += 1;
            }
        }
        if source > self.retained_source || ast > self.retained_ast || payload>self.value_bytes {
            return Err(Fault::new("Program retirement accounting mismatch."));
        }
        // Prepay the second registry pass and conservative byte/node destruction
        // work before any mutation. No allocation, callback or fallible operation
        // remains after the final deadline check. Spent work is never refunded.
        let cleanup = ast
            .checked_mul(8)
            .and_then(|work| work.checked_add(source))
            .and_then(|work| work.checked_add(self.programs.len()))
            .ok_or_else(|| Fault::new("Program retirement accounting overflow."))?;
        for _ in 0..cleanup {
            self.step()?;
        }
        if Instant::now() >= self.job_deadline {
            self.steps = 0;
            return Err(Fault::new(format!(
                "Execution time limit reached ({} second{} per job).",
                self.limits.job_seconds,
                if self.limits.job_seconds == 1 {
                    ""
                } else {
                    "s"
                },
            )));
        }
        self.programs
            .retain(|program| Rc::strong_count(program) != 1);
        self.retained_source -= source;
        self.retained_ast -= ast;
        self.value_bytes-=payload;
        // Escaped RegExp objects own their code independently: no regexp refund.
        self.program_allocation.passes = self.program_allocation.passes.saturating_add(1);
        self.program_allocation.retired = self.program_allocation.retired.saturating_add(retired);
        Ok(())
    }

    pub(super) fn program_limit_fault(
        &self,
        source: &str,
        nodes: Option<usize>,
        kind: &'static str,
        input_strict: Option<bool>,
        ast: bool,
    ) -> Fault {
        let mut groups = BTreeMap::<&str, (usize, usize)>::new();
        let mut registry_only = 0usize;
        let mut requested_matches = 0usize;
        let mut dynamic = 0usize;
        for (index, program) in self.programs.iter().enumerate().take(self.program_count_limit().unwrap_or(4096)) {
            let group = groups.entry(program.source.as_str()).or_insert((index, 0));
            group.1 += 1;
            if program.source == source {
                requested_matches += 1;
            }
            if Rc::strong_count(program) == 1 {
                registry_only += 1;
            }
            if program
                .templates
                .iter()
                .any(|template| template.dynamic_source)
            {
                dynamic += 1;
            }
        }
        let unique = groups.len();
        let mut repeated: Vec<_> = groups
            .into_iter()
            .filter(|(_, (_, count))| *count > 1)
            .map(|(source, (first, count))| (first, count, source.len()))
            .collect();
        repeated.sort_unstable_by(|left, right| {
            right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0))
        });
        repeated.truncate(3);
        let count_cap = self.program_count_refused();
        let source_cap = self.retained_source.saturating_add(source.len()) > self.limits.capacity.retained_source;
        let ast_cap =
            nodes.is_some_and(|nodes|self.ast_count_refused(nodes));
        let prefix = if ast {
            format!("Retained AST limit reached ({} nodes).", self.limits.capacity.retained_ast)
        } else if let Some(limit)=self.program_count_limit() {
            format!("Retained script limit reached ({} scripts / {} MiB source).",limit,self.limits.capacity.retained_source/(1024*1024))
        } else {
            format!("Retained script source limit reached ({} MiB source).",self.limits.capacity.retained_source/(1024*1024))
        };
        Fault::new(format!("{prefix} Count/source/AST-cap={count_cap}/{source_cap}/{ast_cap}; programs={}; source-bytes={}; AST-nodes={}; regexp-accounting-bytes={}; requested-kind={kind}; requested-input-strict={input_strict:?}; requested-bytes={}; requested-nodes={nodes:?}; active-native={}; exact-source-groups={unique}; requested-exact-source-occurrences={requested_matches}; registry-only={registry_only}; dynamic-template-programs={dynamic}; top-duplicate-first-index/count/bytes={repeated:?}. Exact source equality is attribution, not proof of a reusable compilation context.",
            self.programs.len(), self.retained_source, self.retained_ast, self.retained_regexp, source.len(), self.eval_depth))
    }
}

#[test]
fn retained_program_attribution_is_numeric_and_preserves_existing_guards_and_work() {
    let source = "var PrivateSourcePayload = 1;";
    let mut runtime = Runtime::for_capacity_test();
    // Preserve the exact pre-retirement cumulative guard witness.
    runtime.program_allocation.disabled = true;
    for _ in 0..MAX_PROGRAMS {
        runtime.register_program(source, false).unwrap();
    }
    let before = (
        runtime.programs.len(),
        runtime.retained_source,
        runtime.retained_ast,
        runtime.retained_regexp,
        runtime.copy_bytes,
        runtime.steps,
        runtime.job_deadline,
    );
    let fault = runtime.register_program(source, false).err().unwrap();
    assert!(
        fault
            .message
            .contains("Count/source/AST-cap=true/false/false"),
        "{}",
        fault.message
    );
    assert!(
        fault.message.contains("exact-source-groups=1")
            && fault
                .message
                .contains("requested-exact-source-occurrences=128")
    );
    assert!(
        fault.message.contains("registry-only=128")
            && fault.message.contains("requested-input-strict=Some(false)")
    );
    assert!(!fault.message.contains("PrivateSourcePayload"));
    assert_eq!(
        (
            runtime.programs.len(),
            runtime.retained_source,
            runtime.retained_ast,
            runtime.retained_regexp,
            runtime.copy_bytes,
            runtime.steps,
            runtime.job_deadline
        ),
        (
            before.0,
            before.1,
            before.2,
            before.3,
            before.4,
            before.5 - 1,
            before.6
        )
    );
    for ast in [false, true] {
        let mut runtime = Runtime::for_capacity_test();
        if ast {
            runtime.retained_ast = MAX_RETAINED_AST;
        } else {
            runtime.retained_source = MAX_RETAINED_SOURCE;
        }
        let fault = runtime.register_program("42;", false).err().unwrap();
        assert!(
            fault.message.contains(if ast {
                "Count/source/AST-cap=false/false/true"
            } else {
                "Count/source/AST-cap=false/true/false"
            }),
            "{}",
            fault.message
        );
    }
}

#[cfg(test)]
#[path = "program_registry_storage_tests.rs"]
mod storage_tests;
