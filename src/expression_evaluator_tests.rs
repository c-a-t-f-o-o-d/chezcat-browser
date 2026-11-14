//! Original heap-evaluation regressions; no captured script execution.
use super::*;

#[derive(Default)]
struct Recorder { records: Vec<String> }
impl Host for Recorder {
    fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
    fn get(&mut self, object: u32, property: &str) -> Result<Value, String> {
        if object == 1 && property == "child" { Ok(Value::Host(1)) }
        else { Err("Native read denied in heap-evaluation fixture.".into()) }
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        Err("Native write denied in heap-evaluation fixture.".into())
    }
    fn has(&mut self, object: u32, property: &str) -> Result<bool, String> {
        Ok(object == 1 && matches!(property, "record" | "child"))
    }
    fn call(&mut self, object: u32, method: &str, args: &[Value]) -> Result<Value, String> {
        if object == 1 && method == "record" {
            self.records.push(args.first().unwrap_or(&Value::Undefined).to_text());
            Ok(Value::Undefined)
        } else { Err("Native call denied in heap-evaluation fixture.".into()) }
    }
}

fn realm() -> (Runtime, Recorder) {
    let mut runtime = Runtime::for_test();
    runtime.define_global("probe", Value::Host(1)).unwrap();
    (runtime, Recorder::default())
}

fn clean(runtime: &Runtime) {
    assert_eq!((runtime.eval_depth, runtime.call_depth), (0, 0));
    assert_eq!((runtime.evaluation_frames, runtime.evaluation_frame_slots,
                runtime.evaluation_argument_slots), (0, 0, 0));
}

fn small_stack(action: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name(std::thread::current().name().unwrap_or("heap-evaluator").to_owned())
        .stack_size(1024 * 1024).spawn(action).unwrap().join().unwrap();
}

fn observation(source: &str, recursive: bool) -> (Result<String, String>, Vec<String>, (usize, usize, usize)) {
    let (mut runtime, mut host) = realm();
    runtime.recursive_evaluation_oracle = recursive;
    runtime.begin_job();
    let result = runtime.execute(source, &mut host).map(|v| v.to_text());
    clean(&runtime);
    (result, host.records, runtime.job_usage())
}

#[test]
fn heap_evaluation_matches_recursive_values_effects_and_work() {
    for source in [
        include_str!("../tools/fixtures/heap-expression-evaluation.js"),
        include_str!("../tools/fixtures/call-evaluation-stack.js"),
        include_str!("../tools/fixtures/parser-continuation-semantics.js"),
        include_str!("../tools/fixtures/primitive-receiver-lookup.js"),
        include_str!("../tools/fixtures/binding-length-reads.js"),
        include_str!("../tools/fixtures/primitive-string-index-reads.js"),
        include_str!("../tools/fixtures/utf16-character-reads.js"),
    ] {
        let expected = observation(source, true);
        assert_eq!(expected.0.as_ref().unwrap(), "ok");
        assert_eq!(observation(source, false), expected, "{source}");
    }
}

#[test]
fn heap_evaluation_matches_recursive_exceptions_and_positions() {
    for source in [
        "var x=0;\n(1+(missingName));",
        "var x=0;\n(null.value)(probe.record('argument'));",
        "var x=0;\n(0)(probe.record('argument'));",
        "var x=0;\nprobe.noSuchMethod(probe.record('argument'));",
        "var x=0;\nvar o={get value(){throw 'marker';}};o.value(1);",
        "var x=0;\nnull[(probe.record('key'),'value')];",
        "var x=0;\nvar o={};o.value||=(probe.record('rhs'),missingName);",
        "var x=0;\n(new (0))(probe.record('argument'));",
        "var x=0;\n(missingName?.value);",
        "var x=0;\n(undefined?.value)(probe.record('argument'));",
        "var x=0;\n({value:0}).value?.(probe.record('argument'));",
        "var x=0;\n(function(a){return a;})(...(null));",
    ] {
        let expected = observation(source, true);
        assert!(expected.0.is_err());
        assert_eq!(observation(source, false), expected, "{source}");
    }
}

#[test]
fn heap_evaluation_thousands_of_calls_fit_one_mib() {
    small_stack(|| {
        for depth in [256, 1024, 2048] {
            let source = format!("function keep(v){{return v;}}{}7{};", "keep(".repeat(depth), ")".repeat(depth));
            let (mut runtime, mut host) = realm();
            assert_eq!(runtime.execute(&source, &mut host).unwrap().to_text(), "7");
            clean(&runtime);
            runtime.begin_job();
            assert_eq!(runtime.execute("keep(9);", &mut host).unwrap().to_text(), "9");
        }
    });
}

#[test]
fn heap_evaluation_thousands_of_members_optional_calls_and_keys_fit_one_mib() {
    small_stack(|| {
        for opening in ["o.keep(", "o['keep'](", "o[s](", "o.keep?.(", "(o?.keep)("] {
            let source = format!(
                "function keep(v){{return v;}}var o={{keep:keep}};var s=Symbol();o[s]=keep;{}7{};",
                opening.repeat(1024), ")".repeat(1024));
            let (mut runtime, mut host) = realm();
            assert_eq!(runtime.execute(&source, &mut host).unwrap().to_text(), "7", "{opening}");
            clean(&runtime);
        }
        let (mut runtime, mut host) = realm();
        assert!(matches!(runtime.execute(&format!("probe{};", ".child".repeat(2048)), &mut host).unwrap(), Value::Host(1)));
        clean(&runtime);
        runtime.begin_job();
        let source = format!("var o={{0:0}};{}0{};", "o[".repeat(2048), "]".repeat(2048));
        assert_eq!(runtime.execute(&source, &mut host).unwrap().to_text(), "0");
        clean(&runtime);
    });
}

// Build a flat arena directly to isolate the execution quota from parsing.
fn binary_program(depth: usize) -> (Rc<Program>, ExprId) {
    let mut program = Parser::parse("1;", &mut MAX_REGEXP_WORK.clone()).unwrap();
    let mut root = 0;
    for _ in 0..depth {
        let id = program.exprs.len();
        program.exprs.push(Expr { kind: ExprKind::Binary("+", root, 0), at: 0, parenthesized: false });
        root = id;
    }
    (Rc::new(program), root)
}

#[test]
fn heap_evaluation_exact_shared_frame_boundary_and_cleanup() {
    small_stack(|| {
        for depth in [MAX_EVALUATION_FRAMES - 1, MAX_EVALUATION_FRAMES] {
            let (program, root) = binary_program(depth);
            let (mut runtime, mut host) = realm();
            runtime.begin_job();
            let result = runtime.eval(&program, root, 0, &Value::Undefined, &mut host);
            if depth < MAX_EVALUATION_FRAMES {
                assert_eq!(result.unwrap().to_number(), MAX_EVALUATION_FRAMES as f64);
            } else {
                assert!(result.err().unwrap().message.contains("Pending evaluation limit reached (4096)"));
            }
            clean(&runtime);
            runtime.begin_job();
            assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
        }
    });
}

#[test]
fn heap_evaluation_reentrant_getter_shares_enclosing_frame_quota() {
    let (mut runtime, mut host) = realm();
    runtime.execute("var object={get value(){return 1+2;}};", &mut host).unwrap();
    let program = Rc::new(Parser::parse("object.value;", &mut MAX_REGEXP_WORK.clone()).unwrap());
    let StmtKind::Expr(id) = program.stmts[program.body[0]].kind else { unreachable!() };
    runtime.begin_job();
    runtime.evaluation_frames = MAX_EVALUATION_FRAMES - 2;
    let error = runtime.eval(&program, id, 0, &Value::Undefined, &mut host).err().unwrap();
    assert!(error.message.contains("Pending evaluation limit"), "{}", error.display());
    assert_eq!(runtime.evaluation_frames, MAX_EVALUATION_FRAMES - 2);
    assert_eq!((runtime.eval_depth, runtime.call_depth), (0, 0));
    assert_eq!((runtime.evaluation_frame_slots, runtime.evaluation_argument_slots), (0, 0));
    runtime.evaluation_frames = 0;
    runtime.begin_job();
    assert_eq!(runtime.execute("object.value;", &mut host).unwrap().to_text(), "3");
    clean(&runtime);
}

#[test]
fn heap_evaluation_scratch_capacity_is_shared_and_restored() {
    for (frames, arguments) in [(MAX_EVALUATION_FRAME_SLOTS, 0), (0, MAX_EVALUATION_ARGUMENT_SLOTS)] {
        let (mut runtime, mut host) = realm();
        runtime.execute("function keep(v){return v;}", &mut host).unwrap();
        runtime.begin_job();
        runtime.evaluation_frame_slots = frames;
        runtime.evaluation_argument_slots = arguments;
        // A root leaf needs a logical slot but no continuation/argument buffer.
        assert_eq!(runtime.execute("7;", &mut host).unwrap().to_text(), "7");
        let error = runtime.execute("try{keep(1);}catch(e){probe.record('caught');}finally{probe.record('finally');}", &mut host).unwrap_err();
        assert!(error.contains("Evaluation scratch storage limit"), "{error}");
        assert!(host.records.is_empty());
        assert_eq!((runtime.evaluation_frame_slots, runtime.evaluation_argument_slots), (frames, arguments));
        assert_eq!(runtime.evaluation_frames, 0);
        runtime.evaluation_frame_slots = 0;
        runtime.evaluation_argument_slots = 0;
        runtime.begin_job();
        assert_eq!(runtime.execute("keep(9);", &mut host).unwrap().to_text(), "9");
        clean(&runtime);
    }
}

#[test]
fn heap_evaluation_native_function_recursion_and_reentry_stay_bounded() {
    small_stack(|| {
        for (case, source) in [
            "function recurse(){return recurse();}try{recurse();}catch(e){probe.record('caught');}finally{probe.record('finally');}",
            "var object={get value(){return object.value;}};object.value;",
            "var code='eval(code)';try{eval(code);}catch(e){probe.record('caught');}finally{probe.record('finally');}",
        ].into_iter().enumerate() {
            eprintln!("native reentry case {case}: script recursion, getter recursion, dynamic eval respectively");
            let (mut runtime, mut host) = realm();
            let error = runtime.execute(source, &mut host).unwrap_err();
            assert!(error.contains("limit"), "{error}");
            assert!(host.records.is_empty());
            clean(&runtime);
            runtime.begin_job();
            assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
        }
    });
}

#[test]
fn heap_evaluation_native_entry_guard_is_not_the_heap_guard() {
    let (program, root) = binary_program(0);
    let (mut runtime, mut host) = realm();
    runtime.eval_depth = MAX_EVAL_DEPTH;
    let error = runtime.eval(&program, root, 0, &Value::Undefined, &mut host).err().unwrap();
    assert!(error.message.contains("Evaluation nesting limit reached (128)"));
    assert_eq!(runtime.eval_depth, MAX_EVAL_DEPTH);
    assert_eq!(runtime.evaluation_frames, 0);
    runtime.eval_depth = 0;
    clean(&runtime);
}

#[test]
fn heap_evaluation_fatal_work_and_frame_failures_cancel_microtasks() {
    small_stack(|| {
        for (expression, expected) in [
            (format!("1{}", "+1".repeat(MAX_EVALUATION_FRAMES * 2)), "Pending evaluation limit"),
            ("var total=0;for(var i=0;i<2000;i++)total+=payload.charCodeAt(0)".into(), "String/copy work limit"),
        ] {
            let (mut runtime, mut host) = realm();
            runtime.define_global("payload", Value::String("x".repeat(16 * 1024))).unwrap();
            let source = format!("Promise.resolve().then(function(){{try{{{expression};}}catch(e){{probe.record('caught');}}finally{{probe.record('finally');}}}});Promise.resolve().then(function(){{probe.record('late');}});");
            runtime.execute(&source, &mut host).unwrap();
            let error = runtime.microtask_checkpoint(&mut host).unwrap_err();
            assert!(error.contains(expected), "expected {expected}: {error}");
            assert!(host.records.is_empty());
            assert!(!runtime.has_microtasks());
            clean(&runtime);
            runtime.begin_job();
            assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
        }
    });
}

#[test]
fn heap_evaluation_argument_buffers_keep_outer_values_across_nested_calls() {
    let source = concat!(
        "function keep(a,b,c){return a+':'+b+':'+c;}",
        "var answer=keep('outer',keep('middle',...[keep('inner',2,3)],4),'last');",
        "if(answer!=='outer:middle:inner:2:3:4:last')throw answer;",
        "var trace='';var iterator={};iterator[Symbol.iterator]=function(){return {next:function(){trace+='n';throw 'marker';},return:function(){trace+='r';return {};}};};",
        "try{keep(1,...iterator,3);}catch(e){if(e!=='marker')throw e;}",
        "if(trace!=='n')throw trace;'ok';"
    );
    assert_eq!(observation(source, false), observation(source, true));
    assert_eq!(observation(source, false).0.unwrap(), "ok");
}

#[test]
fn heap_evaluation_step_exhaustion_preserves_original_charges() {
    for source in ["1+(2*3);", "null?.method(1);", "(1,2,3);", "true?7:missing;"] {
        let program = Rc::new(Parser::parse(source, &mut MAX_REGEXP_WORK.clone()).unwrap());
        let StmtKind::Expr(id) = program.stmts[program.body[0]].kind else { unreachable!() };
        for steps in 0..24 {
            let mut observations = Vec::new();
            for recursive in [true, false] {
                let (mut runtime, mut host) = realm();
                runtime.recursive_evaluation_oracle = recursive;
                runtime.steps = steps;
                let result = runtime.eval(&program, id, 0, &Value::Undefined, &mut host)
                    .map(|value| value.to_text()).map_err(Fault::display);
                observations.push((result, runtime.steps, host.records));
                clean(&runtime);
            }
            assert_eq!(observations[0], observations[1], "{source}, steps={steps}");
        }
    }
}


#[test]
fn heap_evaluation_original_missing_method_is_catchable_not_a_work_fault() {
    let (mut runtime, mut host) = realm();
    // Preserve the old failed test input with its actual expected semantics.
    // String.prototype.repeat remains unsupported in this pass.
    runtime.execute(concat!(
        "Promise.resolve().then(function(){try{'x'.repeat(100000).repeat(2);}",
        "catch(e){probe.record('caught');}finally{probe.record('finally');}});",
        "Promise.resolve().then(function(){probe.record('late');});"
    ), &mut host).unwrap();
    assert!(runtime.microtask_checkpoint(&mut host).unwrap().is_empty());
    assert_eq!(host.records, ["caught", "finally", "late"]);
    assert!(runtime.string_work_failure_report().is_none());
    clean(&runtime);
}

#[test]
fn heap_evaluation_recursive_calls_keep_finite_success_and_guard_on_one_mib() {
    small_stack(|| {
        let (mut runtime, mut host) = realm();
        runtime.execute("function dive(n){return n?dive(n-1):7;}", &mut host).unwrap();
        runtime.begin_job();
        assert_eq!(runtime.execute("dive(16);", &mut host).unwrap().to_text(), "7");
        clean(&runtime);
        runtime.begin_job();
        let error = runtime.execute("dive(64);", &mut host).unwrap_err();
        assert!(error.contains("limit"), "{error}");
        clean(&runtime);
        runtime.begin_job();
        assert_eq!(runtime.execute("dive(4);", &mut host).unwrap().to_text(), "7");
        clean(&runtime);
    });
}

#[test]
fn heap_evaluation_compound_helpers_match_recursive_semantics_and_charges() {
    for source in [
        "var log='';var item={get value(){log+='v';return 3;}};var a=[item.value,...[4,5]];var o={value:a[0],...{other:a[2]}};log+':'+o.value+':'+o.other;",
        "var input={};input[Symbol.iterator]=function(){var n=0;return {next:function(){return {done:n===2,value:n++};}};};var out=[...input];out.join(':');",
        "var o={get value(){probe.record('getter');return 7;}};`value:${o.value}`;",
        "var key={toString:function(){probe.record('key');return 'x';}};({[key]:(probe.record('value'),2)}).x;",
        "var input={};Object.defineProperty(input,Symbol.iterator,{get:function(){throw 'original';}});[...input];",
    ] {
        assert_eq!(observation(source, false), observation(source, true), "{source}");
    }
}

#[test]
fn heap_evaluation_finite_recursive_array_iterators_fit_one_mib() {
    small_stack(|| {
        for source in [
            "var depth=0,input={};Object.defineProperty(input,Symbol.iterator,{get:function(){if(depth++<12)[...input];depth--;return function(){return [].values();};}});[...input];depth;",
            "var depth=0,input={};input[Symbol.iterator]=function(){var used=false;return {next:function(){if(used)return {done:true};used=true;if(depth<12){depth++;[...input];depth--;}return {done:false,value:1};}};};[...input];depth;",
        ] {
            let (mut runtime, mut host) = realm();
            assert_eq!(runtime.execute(source, &mut host).unwrap().to_text(), "0");
            clean(&runtime);
            runtime.begin_job();
            assert_eq!(runtime.execute("1+2;", &mut host).unwrap().to_text(), "3");
        }
    });
}
