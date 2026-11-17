//! Original continuation regression tests. No captured page code is evaluated.
use super::*;

#[derive(Default)]
struct Recorder {
    records: Vec<String>,
    eval_allowed: bool,
}
impl Host for Recorder {
    // Only the explicit record call is available. Reject unexpected property
    // access so a parser regression cannot gain an unobserved host capability.
    fn get(&mut self, _object: u32, _property: &str) -> Result<Value, String> {
        Err("Property reads are not available in the continuation-test host.".into())
    }
    fn set(&mut self, _object: u32, _property: &str, _value: Value) -> Result<(), String> {
        Err("Property writes are not available in the continuation-test host.".into())
    }
    fn allow_eval(&mut self) -> Result<bool, String> {
        Ok(self.eval_allowed)
    }
    fn has(&mut self, object: u32, name: &str) -> Result<bool, String> {
        Ok(object == 1 && name == "record")
    }
    fn call(&mut self, object: u32, name: &str, args: &[Value]) -> Result<Value, String> {
        if object == 1 && name == "record" {
            self.records.push(args.first().unwrap_or(&Value::Undefined).to_text());
            Ok(Value::Undefined)
        } else {
            Err("Unknown continuation-test capability.".into())
        }
    }
}
fn runtime() -> (Runtime, Recorder) {
    let mut runtime = Runtime::for_test();
    runtime.define_global("probe", Value::Host(1)).unwrap();
    (runtime, Recorder { eval_allowed: true, ..Recorder::default() })
}
fn parse(source: &str) -> Result<Program, Fault> {
    Parser::parse(source, &mut MAX_REGEXP_WORK.clone())
}
fn small_stack(action: impl FnOnce() + Send + 'static) {
    // Preserve the harness name in a fatal native diagnostic, without changing
    // the deliberately small stack or turning an abort into a passing test.
    let name = std::thread::current()
        .name()
        .unwrap_or("continuation-test")
        .to_owned();
    std::thread::Builder::new()
        .name(name)
        .stack_size(1024 * 1024)
        .spawn(action)
        .unwrap()
        .join()
        .unwrap();
}
fn execute_ok(source: &str) {
    let (mut runtime, mut host) = runtime();
    assert_eq!(runtime.execute(source, &mut host).unwrap().to_text(), "ok");
    assert_eq!(runtime.eval_depth, 0);
    assert_eq!(runtime.call_depth, 0);
}

#[test]
fn continuation_reference_and_operator_semantics() {
    execute_ok(include_str!("../tools/fixtures/parser-continuation-semantics.js"));
}

#[test]
fn continuation_early_errors_remain_atomic() {
    execute_ok(include_str!("../tools/fixtures/parser-continuation-errors.js"));
}

#[test]
fn continuation_original_dynamic_call_group_witnesses_now_parse() {
    small_stack(|| {
        for pairs in [28, 29, 30, 128, 512] {
            let source = format!(
                "probe.record('parsed');function unused(){{return {}function(value){{return value;}}{};}}'ok';",
                "keep((".repeat(pairs), "))".repeat(pairs)
            );
            parse(&source).unwrap();
            let wrapper = format!("(function(){{return (0,eval)({source:?});}}).call(this);");
            let (mut runtime, mut host) = runtime();
            assert_eq!(runtime.execute(&wrapper, &mut host).unwrap().to_text(), "ok");
            assert_eq!(host.records, ["parsed"]);
            assert_eq!(runtime.eval_depth, 0);
            assert_eq!(runtime.call_depth, 0);
        }
    });
}

#[test]
fn continuation_group_limit_and_active_native_seed_are_independent() {
    small_stack(|| {
        for active in [0, 4, 16, 32, 60] {
            let source = format!("{}1{}", "(".repeat(MAX_EXPRESSION_FRAMES), ")".repeat(MAX_EXPRESSION_FRAMES));
            Parser::parse_context(&source, &mut MAX_REGEXP_WORK.clone(), false, active).unwrap();
            let excess = format!("({source})");
            let error = Parser::parse_context(&excess, &mut MAX_REGEXP_WORK.clone(), false, active).err().unwrap();
            assert!(error.message.contains("Pending expression limit"), "{}", error.display());
        }
        let error = Parser::parse_context("1;", &mut MAX_REGEXP_WORK.clone(), false, MAX_PARSE_DEPTH).err().unwrap();
        assert!(error.message.contains("Parser nesting limit"));
    });
}

#[test]
fn continuation_deep_calls_keys_and_cleanup_fit_small_native_stack() {
    small_stack(|| {
        for expression in [
            format!("{}0{}", "keep(".repeat(MAX_EXPRESSION_FRAMES), ")".repeat(MAX_EXPRESSION_FRAMES)),
            format!("{}0{}", "holder[".repeat(MAX_EXPRESSION_FRAMES), "]".repeat(MAX_EXPRESSION_FRAMES)),
            format!("{}function(x){{return x;}}{}", "(".repeat(MAX_EXPRESSION_FRAMES), ")".repeat(MAX_EXPRESSION_FRAMES)),
        ] {
            let source = format!("function unused(){{return {expression};}}probe.record('parsed');'ok';");
            let (mut runtime, mut host) = runtime();
            assert_eq!(runtime.execute(&source, &mut host).unwrap().to_text(), "ok");
            assert_eq!(host.records, ["parsed"]);
            drop(runtime); // Flat AST ownership must not recursively destroy calls.
        }
    });
}

#[test]
fn continuation_shared_frames_cover_operators_groups_and_helper_reentry() {
    small_stack(|| {
        for expression in [
            format!("{}({}1)", "x=".repeat(2048), "x=".repeat(2048)),
            format!("{}({}1)", "1**".repeat(2048), "1**".repeat(2048)),
            format!("{}({{value:(1)}}){}", "(".repeat(MAX_EXPRESSION_FRAMES - 1), ")".repeat(MAX_EXPRESSION_FRAMES - 1)),
            format!("{}1{}", "f(".repeat(MAX_EXPRESSION_FRAMES + 1), ")".repeat(MAX_EXPRESSION_FRAMES + 1)),
        ] {
            let source = format!("probe.record('early');function unused(){{return {expression};}}");
            let (mut runtime, mut host) = runtime();
            let error = runtime.execute(&source, &mut host).unwrap_err();
            assert!(error.contains("Pending expression limit"), "{error}");
            assert!(host.records.is_empty());
            assert!(runtime.programs.is_empty());
            runtime.begin_job();
            assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
        }
    });
}

#[test]
fn continuation_recursive_boundaries_remain_native_and_atomic() {
    small_stack(|| {
        for (open, close) in [("[", "]"), ("!", ""), ("new ", ""), ("{x:", "}")] {
            let leaf = if open == "new " { "Array" } else { "1" };
            let source = format!(
                "probe.record('early');function unused(){{return {}{}{leaf}{}{};}}",
                "keep((".repeat(128), open.repeat(80), close.repeat(80), "))".repeat(128)
            );
            let (mut runtime, mut host) = runtime();
            let error = runtime.execute(&source, &mut host).unwrap_err();
            assert!(error.contains("Parser nesting limit"), "{error}");
            assert!(host.records.is_empty());
            assert!(runtime.programs.is_empty());
        }
    });
}

#[test]
fn continuation_invalid_deferred_syntax_is_not_hidden_by_deep_groups() {
    small_stack(|| {
        for bad in ["1+", "f(,)", "x?.", "a??b||c", "-2**2", "a?.b=1"] {
            let source = format!(
                "probe.record('early');function unused(){{return {}{bad}{};}}",
                "keep((".repeat(128), "))".repeat(128)
            );
            let fault = parse(&source).err().expect("malformed deferred body");
            assert!(matches!(fault.exception.as_deref(), Some(Exception::Error("SyntaxError"))), "{}", fault.message);
            let (mut runtime, mut host) = runtime();
            let error = runtime.execute(&source, &mut host).unwrap_err();
            assert!(!error.contains("Parser nesting limit"), "{error}");
            assert!(host.records.is_empty());
            assert!(runtime.programs.is_empty());
        }
    });
}

#[test]
fn continuation_eval_policy_denial_still_precedes_compilation() {
    let (mut runtime, mut host) = runtime();
    host.eval_allowed = false;
    let source = format!("{}1{};", "(".repeat(MAX_EXPRESSION_FRAMES + 1), ")".repeat(MAX_EXPRESSION_FRAMES + 1));
    runtime.define_global("source", Value::String(source)).unwrap();
    assert_eq!(runtime.execute("var reason='';try{eval(source);}catch(e){reason=e.name;}reason;", &mut host).unwrap().to_text(), "EvalError");
    assert_eq!(runtime.programs.len(), 1);
}

#[test]
fn continuation_deep_executable_ast_uses_heap_and_keeps_recovery() {
    small_stack(|| {
        let source = format!(
            "function keep(x){{return x;}}try{{{}1{};}}catch(e){{probe.record('caught');}}finally{{probe.record('finally');}}",
            "keep(".repeat(256), ")".repeat(256)
        );
        let (mut runtime, mut host) = runtime();
        assert_eq!(runtime.execute(&source, &mut host).unwrap().to_text(), "1");
        assert_eq!(host.records, ["finally"]);
        assert_eq!(runtime.evaluation_frames, 0);
        assert_eq!(runtime.eval_depth, 0);
        assert_eq!(runtime.call_depth, 0);
        runtime.begin_job();
        assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
    });
}

#[test]
fn continuation_call_arguments_keep_existing_capacity_and_trailing_comma() {
    let source = format!("function unused(){{return f({},);}}", vec!["1"; MAX_ARGUMENTS].join(","));
    parse(&source).unwrap();
    let excess = format!("probe.record('early');f({});", vec!["1"; MAX_ARGUMENTS + 1].join(","));
    let (mut runtime, mut host) = runtime();
    let error = runtime.execute(&excess, &mut host).unwrap_err();
    assert!(error.contains("Call argument limit"));
    assert!(host.records.is_empty());
}

#[test]
fn continuation_grouped_errors_keep_inner_source_position() {
    let source = format!("var value = (\n{}absentName{}\n);", "(".repeat(512), ")".repeat(512));
    let (mut runtime, mut host) = runtime();
    let error = runtime.execute(&source, &mut host).unwrap_err();
    assert!(error.contains("line 2, column 513"), "{error}");
    assert!(error.contains("absentName"), "{error}");
}

// A parser with no function/arrow lookahead is sufficient to test partial-error
// cleanup directly. Its sentinel quota belongs to a simulated enclosing helper.
fn scratch_parser(source: &str, enclosing: usize) -> Parser {
    let tokens = tokenize(source, &mut MAX_REGEXP_WORK.clone()).unwrap();
    Parser {
        capacity: limits::REGRESSION_CAPACITIES,
        heap: limits::REGRESSION_HEAP,
        program: Program {
            source: source.into(), strict: false, exprs: Vec::new(), stmts: Vec::new(),
            templates: Vec::new(), classes: Vec::new(), class_members: 0, patterns: Vec::new(), body: Vec::new(), chain_steps: 0,
        },
        delimiter_ends: vec![usize::MAX; tokens.len()], tokens, deferred_bodies: Vec::new(),
        current: 0, depth: 0, expression_frames: enclosing, argument_entries: 0,
        if_branches: 0, strict: false, loops: Vec::new(), breakables: Vec::new(),
        labels: Vec::new(), allow_in: false, functions: 0, new_target_allowed: false,
    }
}

#[test]
fn continuation_cleanup_releases_only_owned_slots_and_restores_in_context() {
    for source in ["f((1+),2)", "f(1,g(2,)) +", "a[(b,c)", "(1+2)"] {
        let seed = if source == "(1+2)" { MAX_EXPRESSION_FRAMES - 1 } else { 7 };
        let mut parser = scratch_parser(source, seed);
        assert!(parser.assignment_expression().is_err(), "{source}");
        assert_eq!(parser.expression_frames, seed, "{source}");
        assert!(!parser.allow_in, "{source}");
        assert_eq!(parser.depth, 0, "{source}");
    }
    let mut parser = scratch_parser("f(1,g(2,3))", 7);
    parser.assignment_expression().unwrap();
    assert_eq!(parser.expression_frames, 7);
    assert_eq!(parser.argument_entries, 4);
    let mut parser = scratch_parser("f(1)", 7);
    parser.argument_entries = MAX_AST;
    assert!(parser.assignment_expression().err().unwrap().message.contains("argument storage limit"));
    assert_eq!(parser.expression_frames, 7);
    assert_eq!(parser.argument_entries, MAX_AST);
}

#[test]
fn continuation_grouped_arrow_lookahead_preserves_balanced_defaults() {
    execute_ok("var f=((x=(function(v){return v;})(3), y=((a)=>a+1)(4))=>x+y);if(f()!==8)throw 'defaults';'ok';");
    for source in ["((x) => x)", "((x = f((1))) => x)", "(x => ((y => y)(x)))"] {
        parse(source).unwrap();
    }
    assert!(parse("(x\n=>x)").is_err());
}

#[test]
fn continuation_same_ast_shapes_as_frozen_independent_parser() {
    for line in include_str!("../tools/reference/parser-047/ast-witnesses.tsv").lines() {
        if line.is_empty() || line.starts_with('#') { continue; }
        let (source, expected) = line.split_once('\t').unwrap();
        let program = parse(source).unwrap_or_else(|error| panic!("{source}: {}", error.display()));
        let Some(&statement) = program.body.first() else { panic!("missing expression"); };
        let StmtKind::Expr(root) = program.stmts[statement].kind else { panic!("not an expression"); };
        assert_eq!(ast_signature(&program, root), expected, "{source}");
    }
}

fn ast_signature(program: &Program, root: ExprId) -> String {
    // Only shallow generated oracle cases use this table. The implementation is
    // iterative even here; no recursive Rust walk is introduced by deeper ASTs.
    let mut values: Vec<String> = Vec::new();
    for expression in &program.exprs {
        let args = |arguments: &[Argument]| arguments.iter().map(|a| format!("{}{}", if a.spread { "..." } else { "" }, values[a.expression])).collect::<Vec<_>>().join(",");
        let text = match &expression.kind {
            ExprKind::Literal(Value::String(value)) => format!("s({value})"),
            ExprKind::Literal(value) => format!("v({})", value.to_text()),
            ExprKind::Name(name) => format!("n({name})"),
            ExprKind::This => "this".into(),
            ExprKind::Unary(op, child) => format!("u({op},{})", values[*child]),
            ExprKind::Binary(op, left, right) => format!("b({op},{},{})", values[*left], values[*right]),
            ExprKind::Assign(op, left, right) => format!("a({op},{},{})", values[*left], values[*right]),
            ExprKind::Conditional(c, y, n) => format!("q({},{},{})", values[*c], values[*y], values[*n]),
            ExprKind::Sequence(left, right) => format!("sqr({},{})", values[*left], values[*right]),
            ExprKind::Member(object, key) => format!("m({},{})", values[*object], values[*key]),
            ExprKind::Call(callee, arguments) => format!("c({},[{}])", values[*callee], args(arguments)),
            ExprKind::Construct(callee, arguments) => format!("new({},[{}])", values[*callee], args(arguments)),
            ExprKind::Update(child, increment, prefix) => format!("update({},{},{})", if *increment { "++" } else { "--" }, prefix, values[*child]),
            ExprKind::OptionalChain(base, steps) => {
                let steps = steps.iter().map(|step| match step {
                    optional_chaining::Step::Member { key, optional } => format!("m({optional},{})", values[*key]),
                    optional_chaining::Step::Call { args: arguments, optional } => format!("c({optional},[{}])", args(arguments)),
                }).collect::<Vec<_>>().join(",");
                format!("chain({},[{steps}])", values[*base])
            }
            _ => panic!("unexpected oracle expression"),
        };
        values.push(text);
    }
    values[root].clone()
}

#[test]
fn continuation_call_variants_execute_deeply_on_small_stack_and_recover() {
    small_stack(|| {
        let setup = concat!(
            "function keep(x){return x;}var holder={method:keep};",
            "var key=Symbol('method');holder[key]=keep;"
        );
        for opening in [
            "keep(",
            "holder.method(",
            "holder['method'](",
            "holder[key](",
            "keep?.(",
            "holder.method?.(",
            "(holder?.method)(",
        ] {
            let (mut runtime, mut host) = runtime();
            let shallow = format!("{setup}{}7{};", opening.repeat(24), ")".repeat(24));
            assert_eq!(runtime.execute(&shallow, &mut host).unwrap().to_text(), "7");
            assert_eq!((runtime.eval_depth, runtime.call_depth), (0, 0));
            runtime.begin_job();
            let deep = format!(
                "try{{{}1{};}}catch(e){{probe.record('caught');}}finally{{probe.record('finally');}}",
                opening.repeat(256), ")".repeat(256)
            );
            assert_eq!(runtime.execute(&deep, &mut host).unwrap().to_text(), "1", "{opening}");
            assert_eq!(host.records, ["finally"], "{opening}");
            assert_eq!(runtime.evaluation_frames, 0);
            assert_eq!((runtime.eval_depth, runtime.call_depth), (0, 0), "{opening}");
            runtime.begin_job();
            assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
        }
    });
}

#[test]
fn continuation_call_frame_repair_preserves_reference_and_spread_semantics() {
    small_stack(|| {
        execute_ok(include_str!("../tools/fixtures/call-evaluation-stack.js"));
        let setup = "function wrap(value){return [value];}";
        let (mut runtime, mut host) = runtime();
        let shallow = format!(
            "{setup}({}[7]{})[0];", "wrap(...".repeat(24), ")".repeat(24)
        );
        assert_eq!(runtime.execute(&shallow, &mut host).unwrap().to_text(), "7");
        runtime.begin_job();
        let deep = format!("{}[7]{};", "wrap(...".repeat(256), ")".repeat(256));
        let Value::Object(object) = runtime.execute(&deep, &mut host).unwrap() else { panic!("expected wrapped array") };
        assert_eq!(runtime.get_property(object, "0").unwrap().to_text(), "7");
        assert_eq!(runtime.evaluation_frames, 0);
        assert_eq!((runtime.eval_depth, runtime.call_depth), (0, 0));
        runtime.begin_job();
        assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
    });
}
