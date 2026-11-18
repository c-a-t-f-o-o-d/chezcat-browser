//! Exact retained source views; no eval, compilation, name getters or replay.
//! https://tc39.es/ecma262/#sec-function.prototype.tostring
use super::*;

impl Runtime {
    pub(super) fn function_source(&mut self, function: usize) -> Result<Value, Fault> {
        let (program, span, dynamic, asynchronous) = match self.functions.get(function) {
            Some(Function::Script { program, template, .. }) => {
                let template = &program.templates[*template];
                (Some(Rc::clone(program)), template.source_start..template.source_end, template.dynamic_source, template.asynchronous)
            }
            Some(Function::Vacant) | None => return Err(Fault::new("Invalid callable source identity.")),
            Some(_) => (None, 0..0, false, false),
        };
        if let Some(program) = program {
            let source = program.source.get(span).ok_or_else(|| Fault::new("Invalid retained function source range."))?;
            let extra = if dynamic { " anonymous".len() } else { 0 };
            let bytes = source.len().saturating_add(extra);
            self.function_source_output_work(bytes)?;
            let value = if dynamic {
                let prefix = if asynchronous { "async function" } else { "function" };
                let tail = source.strip_prefix(prefix).ok_or_else(|| Fault::new("Invalid dynamic function source range."))?;
                let mut value = String::with_capacity(bytes);
                value.push_str(prefix);
                value.push_str(" anonymous");
                value.push_str(tail);
                value
            } else { source.to_owned() };
            return Ok(Value::String(value));
        }
        let name = if matches!(self.functions[function], Function::Bound(_)) { "" }
            else { self.function_initial_names[function].as_deref().unwrap_or("") };
        let bytes = "function ".len() + name.len() + "() { [native code] }".len();
        // Capture only the index/length before the mutable budget check; do not
        // read or convert any public function property.
        self.function_source_output_work(bytes)?;
        let name = if matches!(self.functions[function], Function::Bound(_)) { "" }
            else { self.function_initial_names[function].as_deref().unwrap_or("") };
        Ok(Value::String(format!("function {name}() {{ [native code] }}")))
    }

    fn function_source_output_work(&mut self, bytes: usize) -> Result<(), Fault> {
        if bytes > self.limits.capacity.string_bytes { return Err(Fault::new(format!("String limit reached ({} KiB).", self.limits.capacity.string_bytes / 1024))); }
        self.work_as(bytes, WorkKind::ConversionOutput)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct NoHost { deny: bool, compilation_checks: usize }
    impl Host for NoHost {
        fn allow_eval(&mut self) -> Result<bool, String> { self.compilation_checks += 1; Ok(!self.deny) }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("unexpected host get".into()) }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("unexpected host set".into()) }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("unexpected host call".into()) }
    }

    #[test]
    fn script_source_preserves_exact_function_arrow_method_and_accessor_syntax() {
        for (source, expected) in [
            ("function /*a*/ f (x){/*b*/return x;} f.toString();", "function /*a*/ f (x){/*b*/return x;}"),
            ("((function /*é*/ named (x) { return x; })).toString();", "function /*é*/ named (x) { return x; }"),
            ("(function(x){return /a+/g.test(x);}).toString();", "function(x){return /a+/g.test(x);}"),
            ("(x=>x+1 /* trailing */).toString();", "x=>x+1"),
            ("((x /*c*/)=>({x})).toString();", "(x /*c*/)=>({x})"),
            ("(x => { /*é😀*/return x; }).toString();", "x => { /*é😀*/return x; }"),
            ("({m /*c*/ (x){return x;}}).m.toString();", "m /*c*/ (x){return x;}"),
            ("({'quoted key' (x){return x;}})['quoted key'].toString();", "'quoted key' (x){return x;}"),
            ("({23 (x){return x;}})[23].toString();", "23 (x){return x;}"),
            ("var key='computed';({[key] (x){return x;}}).computed.toString();", "[key] (x){return x;}"),
            ("var key='computed',o={get /*c*/ [key](){return 1;}};Object.getOwnPropertyDescriptor(o,key).get.toString();", "get /*c*/ [key](){return 1;}"),
            ("var o={set /*c*/ value(x){}};Object.getOwnPropertyDescriptor(o,'value').set.toString();", "set /*c*/ value(x){}"),
            ("/*prefix é😀*/\r\nfunction f (x) {\r\nreturn `${x}😀`;\r\n}\r\nf.toString();", "function f (x) {\r\nreturn `${x}😀`;\r\n}"),
            ("var outer=()=>x=>({x});outer().toString();", "x=>({x})"),
            ("(()=>0x0A).toString();", "()=>0x0A"),
            ("(()=> '\\x41').toString();", "()=> '\\x41'"),
            ("(()=> /a+/gi).toString();", "()=> /a+/gi"),
            ("(()=> `a${1}z`).toString();", "()=> `a${1}z`"),
        ] {
            let mut runtime = Runtime::for_capacity_test();
            let result = runtime.execute(source, &mut NoHost::default()).unwrap();
            assert_eq!(result.to_text(), expected, "{source}");
        }
    }

    #[test]
    fn dynamic_function_source_uses_anonymous_without_a_private_name_binding() {
        let mut runtime = Runtime::for_capacity_test();
        let mut host = NoHost::default();
        assert_eq!(runtime.execute("Function().toString();", &mut host).unwrap().to_text(), "function anonymous(\n) {\n\n}");
        assert_eq!(runtime.execute("Function('x /*p*/','y','return x+y;').toString();", &mut host).unwrap().to_text(), "function anonymous(x /*p*/,y\n) {\nreturn x+y;\n}");
        assert_eq!(runtime.execute("var f=Function('return [typeof anonymous,function inner(){return 1;}];');var values=f();if(values[0]!=='undefined')throw 'anonymous binding';values[1].toString();", &mut host).unwrap().to_text(), "function inner(){return 1;}");
    }

    #[test]
    fn public_name_changes_and_getters_do_not_affect_source_or_run_code() {
        let mut runtime = Runtime::for_capacity_test();
        let source = r#"
var count=0,f=function /*original*/ real(){return 1;},n=Array,b=f.bind(null);
for(var target of [f,n,b]){Object.defineProperty(target,'name',{get:function(){count++;throw 'name getter';},configurable:true});}
if(f.toString()!=='function /*original*/ real(){return 1;}')throw 'script source';
if(n.toString()!=='function Array() { [native code] }')throw 'native initial name';
if(b.toString()!=='function () { [native code] }'||count!==0)throw 'name effects';
var errors=0;try{Function.prototype.toString.call({});}catch(e){if(e.name==='TypeError')errors++;}
if(errors!==1)throw 'callable receiver';'ok';
"#;
        assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    }

    #[test]
    fn reading_existing_source_does_not_ask_for_string_compilation_permission() {
        let mut runtime = Runtime::for_capacity_test();
        let mut host = NoHost { deny: true, ..NoHost::default() };
        assert_eq!(runtime.execute("function f(){return 7;}f.toString();", &mut host).unwrap().to_text(), "function f(){return 7;}");
        assert_eq!(host.compilation_checks, 0);
        runtime.execute("var blocked=false;try{Function('return 1;');}catch(e){blocked=e.name==='EvalError';}if(!blocked)throw 'compilation gate';", &mut host).unwrap();
        assert_eq!(host.compilation_checks, 1);
    }

    #[test]
    fn source_output_limits_charge_before_copy_and_do_not_grow_callable_or_program_arenas() {
        for extra in [0usize, 1] {
            let mut runtime = Runtime::for_capacity_test();
            let prefix = "function f(){/*";
            let suffix = "*/}";
            let text = format!("{prefix}{}{suffix}", "x".repeat(MAX_STRING_BYTES + extra - prefix.len() - suffix.len()));
            let value = runtime.execute(&format!("{text};f;"), &mut NoHost::default()).unwrap();
            let Value::Function(function) = value else { panic!("script function") };
            runtime.begin_job();
            let before = (runtime.functions.len(), runtime.programs.len(), runtime.job_deadline);
            if extra == 0 {
                assert_eq!(runtime.function_source(function).unwrap().to_text(), text);
                assert_eq!(runtime.job_usage().1, MAX_STRING_BYTES);
                assert_eq!(runtime.function_source(function).unwrap().to_text(), text);
                assert_eq!(runtime.job_usage().1, 2 * MAX_STRING_BYTES);
                assert_eq!(runtime.job_deadline, before.2);
                runtime.begin_job();
                runtime.copy_bytes = MAX_STRING_BYTES - 1;
                let error = runtime.function_source(function).unwrap_err();
                assert!(error.message.contains("String/copy work limit"), "{}", error.message);
                assert_eq!(runtime.copy_bytes, 0);
                assert!(runtime.string_work_failure_report().unwrap().contains("Successful charges: bytes=0;"));
            } else {
                let error = runtime.function_source(function).unwrap_err();
                assert!(error.message.contains("String limit"), "{}", error.message);
                assert_eq!(runtime.job_usage().1, 0);
                assert_eq!(runtime.job_deadline, before.2);
            }
            assert_eq!((runtime.functions.len(), runtime.programs.len()), (before.0, before.1));
            assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
        }
    }

    #[test]
    fn implicit_conversion_uses_source_then_observes_public_conversion_overrides() {
        let mut runtime = Runtime::for_capacity_test();
        let source = r#"
var f=function /*source*/ demo(){return 1;},text='function /*source*/ demo(){return 1;}';
if(String(f)!==text||(''+f)!==text||`${f}`!==text)throw 'implicit source';
f.toString=function(){return 'override';};
if(String(f)!=='override'||(''+f)!=='override'||Function.prototype.toString.call(f)!==text)throw 'public toString';
f[Symbol.toPrimitive]=function(hint){return hint==='string'?'symbol-string':'symbol-default';};
if(String(f)!=='symbol-string'||(''+f)!=='symbol-default'||Function.prototype.toString.call(f)!==text)throw 'public conversion';
'ok';
"#;
        assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    }

    #[test]
    fn dynamic_anonymous_name_is_included_in_the_output_size_bound() {
        for extra in [0usize, 1] {
            let mut runtime = Runtime::for_capacity_test();
            let fixed = "function anonymous(\n) {\n/**/\n}".len();
            let body = format!("/*{}*/", "x".repeat(MAX_STRING_BYTES + extra - fixed));
            let Value::Function(function) = runtime.function_constructor(&[Value::String(body)], &mut NoHost::default()).unwrap() else { panic!("dynamic function") };
            runtime.begin_job();
            if extra == 0 {
                let value = runtime.function_source(function).unwrap().to_text();
                assert_eq!(value.len(), MAX_STRING_BYTES);
                assert!(value.starts_with("function anonymous(\n) {\n/*"));
                assert_eq!(runtime.job_usage().1, MAX_STRING_BYTES);
            } else {
                assert!(runtime.function_source(function).unwrap_err().message.contains("String limit"));
                assert_eq!(runtime.job_usage().1, 0);
            }
        }
    }

    #[test]
    fn late_initial_name_failures_leave_parallel_function_arenas_aligned() {
        for budget in 0..64 {
            let mut runtime = Runtime::for_capacity_test();
            runtime.begin_job();
            runtime.copy_bytes = budget;
            let result = runtime.native_function(Builtin::IsNaN, "originalName");
            assert_eq!(runtime.functions.len(), runtime.function_properties.len());
            assert_eq!(runtime.functions.len(), runtime.function_realms.len());
            assert_eq!(runtime.functions.len(), runtime.function_initial_names.len());
            if result.is_ok() { assert_eq!(runtime.function_initial_names.last().unwrap().as_deref(), Some("originalName")); }
            runtime.begin_job();
            assert_eq!(runtime.execute("6*7;", &mut NoHost::default()).unwrap().to_number(), 42.0);
        }
    }
}
