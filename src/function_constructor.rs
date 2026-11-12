//! Bounded dynamic Function compilation using the original parser and realm.
//! Parameter and body fragments are validated independently before the final
//! unnamed template is retained. No wrapper is executed. Source conversion,
//! compilation and later calls share existing budgets and the host's eval policy.
//! https://tc39.es/ecma262/#sec-createdynamicfunction

use super::{
    ExprKind, Fault, Host, Parser, Program, Runtime, StmtKind, Value, MAX_ARGUMENTS,
    MAX_CALL_DEPTH, MAX_PROGRAMS, MAX_RETAINED_AST, MAX_RETAINED_SOURCE, MAX_SOURCE_BYTES,
    MAX_STRING_BYTES,
};
use std::rc::Rc;

impl Runtime {
    pub(super) fn function_constructor(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Function call limit reached (32)."));
        }
        if args.len() > MAX_ARGUMENTS {
            return Err(Fault::new("Call argument limit reached (256)."));
        }
        self.call_depth += 1;
        let result = (|| {
            let mut parameters = String::new();
            for (index, value) in args.iter().take(args.len().saturating_sub(1)).enumerate() {
                let text = self.function_source_argument(value, host)?;
                if index != 0 {
                    self.function_source_append(&mut parameters, ",")?;
                }
                self.function_source_append(&mut parameters, &text)?;
            }
            let body = match args.last() {
                Some(value) => self.function_source_argument(value, host)?,
                None => String::new(),
            };
            let source = self.function_source_wrapper(&parameters, &body)?;
            // CreateDynamicFunction converts every argument before asking the
            // host to permit string compilation. No fragment is parsed first.
            if !host.allow_eval().map_err(Fault::new)? {
                return Err(Fault::exception(
                    "EvalError",
                    "Content Security Policy blocks dynamic Function compilation.",
                ));
            }
            if self.programs.len() >= MAX_PROGRAMS
                || self.retained_source.saturating_add(source.len()) > MAX_RETAINED_SOURCE
            {
                return Err(Fault::new(
                    "Retained script limit reached (128 scripts / 2 MiB source).",
                ));
            }
            // Newlines terminate trailing line comments. Separate parses and
            // exact wrapper shapes prevent either fragment from supplying the
            // other's syntax, closing the wrapper, or adding top-level code.
            let parameter_source = self.function_source_wrapper(&parameters, "")?;
            let parameter_program = self.parse_function_source(&parameter_source)?;
            Self::function_source_template(&parameter_program, false, true)?;
            drop(parameter_program);
            drop(parameter_source);

            let body_source = self.function_source_wrapper("", &body)?;
            let body_program = self.parse_function_source(&body_source)?;
            Self::function_source_template(&body_program, true, false)?;
            drop(body_program);
            drop(body_source);

            // The final parse enforces parameter/body early errors together,
            // including duplicate bindings and strict directives with rest.
            let program = self.parse_function_source(&source)?;
            let template = Self::function_source_template(&program, false, false)?;
            let program = self.retain_parsed_program(Rc::new(program))?;
            let function = self.make_function(&program, template, 0, &Value::Undefined)?;
            let properties = self.ordinary_id(self.ordinary_required(&function)?)?;
            // The name is metadata only. An unnamed parsed template creates no
            // private lexical `anonymous` binding and captures the global realm.
            self.store_property(
                properties,
                "name".into(),
                Value::String("anonymous".into()),
                Some((false, false, true)),
            )?;
            Ok(function)
        })();
        self.call_depth -= 1;
        result
    }

    fn function_source_argument(
        &mut self,
        value: &Value,
        host: &mut dyn Host,
    ) -> Result<String, Fault> {
        if value.bytes() > MAX_STRING_BYTES {
            return Err(Fault::new("String limit reached (128 KiB)."));
        }
        let value = self.copy_value(value)?;
        let primitive = self.primitive_with_hint(value, true, host)?;
        self.checked_value(primitive)?.script_text()
    }

    fn function_source_append(&mut self, output: &mut String, text: &str) -> Result<(), Fault> {
        if output.len().saturating_add(text.len()) > MAX_SOURCE_BYTES {
            return Err(Fault::new(
                "Dynamic Function source limit reached (512 KiB).",
            ));
        }
        self.work(text.len())?;
        output.push_str(text);
        Ok(())
    }

    fn function_source_wrapper(&mut self, parameters: &str, body: &str) -> Result<String, Fault> {
        let mut source = String::new();
        for part in ["(function(", parameters, "\n) {\n", body, "\n})"] {
            self.function_source_append(&mut source, part)?;
        }
        Ok(source)
    }

    fn parse_function_source(&mut self, source: &str) -> Result<Program, Fault> {
        self.step()?;
        self.work(source.len())?;
        // Dynamic functions do not inherit caller strictness. Active evaluator
        // depth still counts because parsing happens on the same native stack.
        let program = Parser::parse_context(source, &mut self.regexp_work, false, self.eval_depth)?;
        let nodes = program.node_count();
        if self.retained_ast.saturating_add(nodes) > MAX_RETAINED_AST {
            return Err(Fault::new("Retained AST limit reached (262144 nodes)."));
        }
        self.work(nodes.saturating_mul(std::mem::size_of::<usize>()))?;
        Ok(program)
    }

    fn function_source_template(
        program: &Program,
        empty_parameters: bool,
        empty_body: bool,
    ) -> Result<usize, Fault> {
        let template = match program.body.as_slice() {
            [statement] => match program.stmts[*statement].kind {
                StmtKind::Expr(expression) => match program.exprs[expression].kind {
                    ExprKind::Function(template) => Some(template),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };
        if let Some(template) = template {
            let function = &program.templates[template];
            if !function.arrow
                && function.constructible
                && function.name.is_none()
                && (!empty_parameters || function.params.is_empty() && !function.rest)
                && (!empty_body || function.body.is_empty())
            {
                return Ok(template);
            }
        }
        Err(
            Fault::exception("SyntaxError", "Invalid dynamic Function fragment boundary.")
                .at(&program.source, 0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestHost {
        denied: bool,
        unavailable: bool,
        policy_calls: usize,
    }
    impl Host for TestHost {
        fn allow_eval(&mut self) -> Result<bool, String> {
            self.policy_calls += 1;
            if self.unavailable {
                Err("String compilation capability is unavailable.".into())
            } else {
                Ok(!self.denied)
            }
        }
        fn has(&mut self, _: u32, _: &str) -> Result<bool, String> {
            Ok(false)
        }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            Ok(Value::Undefined)
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("Unexpected native write.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("Unexpected native call.".into())
        }
    }

    fn realm() -> Runtime {
        let mut runtime = Runtime::for_test();
        for name in ["window", "self", "globalThis"] {
            runtime.define_global(name, Value::Host(2)).unwrap();
        }
        runtime.bind_browser_global(2).unwrap();
        runtime
    }

    fn succeeds(source: &str) {
        assert_eq!(
            realm()
                .execute(source, &mut TestHost::default())
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn dynamic_function_call_new_bound_constructor_and_metadata() {
        succeeds(
            r#"
var a=Function('x','y','return x+y;'),b=new Function('x','return x*2;');
if(a(11,12)!==23||b(13)!==26||!(a instanceof Function)||!(b instanceof Function))throw 'call and new';
var Bound=Function.bind(null,'value'),C=new Bound('this.value=value;'),instance=new C(9);
if(instance.value!==9||!(instance instanceof C)||C.prototype.constructor!==C)throw 'bound constructor';
if(({}).constructor.constructor('return 7;')()!==7||Function.call(null,'return 8;')()!==8)throw 'indirect aliases';
if(a.name!=='anonymous'||a.length!==2||Object.getPrototypeOf(a)!==Function.prototype)throw 'metadata';
var d=Object.getOwnPropertyDescriptor(a,'name'),l=Object.getOwnPropertyDescriptor(a,'length'),p=Object.getOwnPropertyDescriptor(a,'prototype');
if(d.writable||d.enumerable||!d.configurable||l.writable||l.enumerable||!l.configurable||!p.writable||p.enumerable||p.configurable)throw 'descriptors';
'ok';
"#,
        );
    }

    #[test]
    fn dynamic_function_uses_global_scope_without_anonymous_binding() {
        succeeds(
            r#"
var outer=14;let lexical=15;var anonymous='global name';
function build(){var outer=16;let lexical=17;var anonymous='local name';return Function('return outer+lexical+":"+anonymous;');}
if(build()()!=='29:global name')throw 'global scope';
function factory(){var hidden='caller';return Function('return typeof hidden;');}
if(factory()()!=='undefined')throw 'caller binding captured';
var closure=Function('start','return function(delta){start+=delta;return start;};')(25);
if(closure(2)!==27||closure(3)!==30)throw 'returned closure';
'ok';
"#,
        );
    }

    #[test]
    fn dynamic_function_strictness_arguments_and_rest_are_local_to_new_body() {
        succeeds(
            r#"
function strictCaller(){'use strict';return Function('return this===globalThis;');}
if(!strictCaller()()||!Function('"use strict";return this===undefined;')())throw 'strict receiver';
if(Function('a','arguments[0]=9;return a;')(1)!==9||Function('a','"use strict";arguments[0]=9;return a;')(1)!==1)throw 'arguments aliasing';
if(Function('a','a','return a;')(2,3)!==3)throw 'sloppy duplicate';
var caught=0;for(var pair of [['a,a','"use strict";return a;'],['...rest','"use strict";return rest.length;'],['a','let a;']]){
try{Function(pair[0],pair[1]);}catch(e){if(e.name==='SyntaxError')caught++;}}
if(caught!==3)throw 'cross-fragment early errors';
var f=Function('first','...rest','return first+rest.length;');if(f.length!==1||f(4,5,6)!==6)throw 'rest';
'ok';
"#,
        );
    }

    #[test]
    fn dynamic_function_comments_fragments_templates_and_empty_body() {
        succeeds(
            r#"
if(Function('a//parameter comment','return a;//body comment')(22)!==22)throw 'trailing comments';
if(Function('a/*comment*/,b','c','return a+b+c;')(1,2,3)!==6)throw 'comma fragments';
if(Function('a//comment','ignored','return arguments.length+":"+a;')(4,5)!=='2:4')throw 'joined parameter comment';
if(Function('value','return `next ${value+1}`;')(6)!=='next 7')throw 'template body';
if(Function()()!==undefined||Function(undefined)()!==undefined||Function(null)()!==undefined)throw 'empty and primitive bodies';
'ok';
"#,
        );
    }

    #[test]
    fn dynamic_function_converts_arguments_before_syntax_and_propagates_identity() {
        succeeds(
            r#"
var log='',marker={};function text(mark,value){return {toString:function(){log+=mark;return value;}};}
var f=Function(text('p','a'),text('q','b'),text('r','return a+b;'));
if(log!=='pqr'||f(2,3)!==5)throw 'conversion order';log='';
try{Function(text('p','a)'),text('b','return 1;'));}catch(e){if(e.name!=='SyntaxError')throw e;}
if(log!=='pb')throw 'parse before conversion';log='';
try{Function({toString:function(){log+='x';throw marker;}},text('y','return 1;'));}catch(e){if(e!==marker)throw e;}
if(log!=='x')throw 'conversion abrupt order';var caught=false;
try{Function(Symbol());}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'symbol source';
'ok';
"#,
        );
    }

    #[test]
    fn dynamic_function_separate_parses_reject_wrapper_and_comment_injection() {
        succeeds(
            r#"
var effects=0,caught=0;
for(var pair of [
['a/*','*/return 1;'],['a) { effects=1; } //','return 2;'],['a','} effects=2; {'],
['a) { } ); effects=3; (function(','return 4;'],['a','}); effects=5; (function(){'],
['a/* */) { effects=6; } /*','*/'],['','effects=7;return )']]){
try{Function(pair[0],pair[1]);}catch(e){if(e.name==='SyntaxError')caught++;else throw e;}}
if(caught!==7||effects!==0)throw 'fragment injection or execution';
'ok';
"#,
        );
    }

    #[test]
    fn dynamic_function_policy_denial_happens_after_conversion_before_parsing() {
        let mut runtime = realm();
        let mut host = TestHost {
            denied: true,
            ..TestHost::default()
        };
        runtime.execute("var log='',p={toString:function(){log+='p';return 'a)';}},b={toString:function(){log+='b';return 'return )';}};",&mut host).unwrap();
        let functions = runtime.functions.len();
        let programs = runtime.programs.len();
        assert_eq!(
            runtime
                .execute(
                    "var result='';try{Function(p,b);}catch(e){result=e.name;}log+':'+result;",
                    &mut host
                )
                .unwrap()
                .to_text(),
            "pb:EvalError"
        );
        assert_eq!(host.policy_calls, 1);
        assert_eq!(runtime.functions.len(), functions);
        assert_eq!(runtime.programs.len(), programs + 1);
        assert_eq!(runtime.call_depth, 0);
    }

    #[test]
    fn dynamic_function_unavailable_capability_remains_fatal_and_cancels_jobs() {
        let mut runtime = realm();
        let mut host = TestHost {
            unavailable: true,
            ..TestHost::default()
        };
        let error=runtime.execute("var converted=false,caught=false,finished=false,queued=false;queueMicrotask(function(){queued=true;});try{Function({toString:function(){converted=true;return 'return 1;';}});}catch(e){caught=true;}finally{finished=true;}",&mut host).unwrap_err();
        assert!(error.contains("capability is unavailable"), "{error}");
        assert_eq!(host.policy_calls, 1);
        assert!(!runtime.has_microtasks());
        assert_eq!(runtime.call_depth, 0);
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("converted&&!caught&&!finished&&!queued;", &mut host)
                .unwrap()
                .to_text(),
            "true"
        );
    }

    #[test]
    fn dynamic_function_validation_asts_are_not_retained_or_instantiated() {
        let mut runtime = realm();
        let mut host = TestHost::default();
        let functions = runtime.functions.len();
        let source="var caught=0;for(var i=0;i<140;i++){try{Function('a)','return 1;');}catch(e){if(e.name==='SyntaxError')caught++;}}if(caught!==140)throw 'validation';";
        runtime.execute(source, &mut host).unwrap();
        assert_eq!(runtime.programs.len(), 1);
        assert_eq!(runtime.retained_source, source.len());
        assert_eq!(runtime.functions.len(), functions);
        runtime.begin_job();
        runtime
            .execute("var f=Function('a','return a+1;');", &mut host)
            .unwrap();
        assert_eq!(runtime.programs.len(), 3);
        assert_eq!(runtime.functions.len(), functions + 1);
        assert_eq!(
            runtime.retained_ast,
            runtime
                .programs
                .iter()
                .map(|p| p.node_count())
                .sum::<usize>()
        );
    }

    #[test]
    fn dynamic_function_retained_program_source_and_ast_caps_remain_fatal() {
        let mut runtime = realm();
        let mut host = TestHost::default();
        let mut saved = None;
        for _ in 0..MAX_PROGRAMS {
            runtime.begin_job();
            let function = runtime
                .function_constructor(&[Value::String("return 1;".into())], &mut host)
                .unwrap();
            if saved.is_none() {
                saved = Some(function);
            }
        }
        runtime.begin_job();
        let error = runtime.function_constructor(&[], &mut host).unwrap_err();
        assert!(error.exception.is_none() && error.message.contains("Retained script limit"));
        assert_eq!(runtime.programs.len(), MAX_PROGRAMS);
        runtime.begin_job();
        assert_eq!(
            runtime
                .invoke(saved.unwrap(), &[], Value::Undefined, &mut host)
                .unwrap()
                .to_text(),
            "1"
        );
        for (body, needle) in [
            (";".repeat(90_000), "Retained AST"),
            (
                format!("/*{}*/ return 1;", "x".repeat(120_000)),
                "Retained script",
            ),
        ] {
            let mut runtime = realm();
            let mut reached = false;
            for _ in 0..24 {
                runtime.begin_job();
                match runtime.function_constructor(&[Value::String(body.clone())], &mut host) {
                    Ok(_) => {}
                    Err(error) => {
                        assert!(
                            error.exception.is_none() && error.message.contains(needle),
                            "{}",
                            error.message
                        );
                        reached = true;
                        break;
                    }
                }
            }
            assert!(reached);
            assert!(
                runtime.retained_source <= MAX_RETAINED_SOURCE
                    && runtime.retained_ast <= MAX_RETAINED_AST
            );
            assert_eq!(runtime.call_depth, 0);
        }
    }

    #[test]
    fn dynamic_function_assembled_source_and_parameter_caps_are_bounded() {
        let mut runtime = realm();
        let mut host = TestHost::default();
        let args = vec![Value::String(" ".repeat(MAX_STRING_BYTES)); 5];
        let error = runtime.function_constructor(&args, &mut host).unwrap_err();
        assert!(
            error.exception.is_none() && error.message.contains("source limit"),
            "{}",
            error.message
        );
        assert_eq!(host.policy_calls, 0);
        assert_eq!(runtime.call_depth, 0);
        runtime.begin_job();
        let parameters = (0..257)
            .map(|index| format!("p{index}"))
            .collect::<Vec<_>>()
            .join(",");
        let error = runtime
            .function_constructor(
                &[Value::String(parameters), Value::String("return 1;".into())],
                &mut host,
            )
            .unwrap_err();
        assert!(
            error.exception.is_none() && error.message.contains("parameter limit"),
            "{}",
            error.message
        );
        assert!(runtime.programs.is_empty());
    }

    #[test]
    fn dynamic_function_recursion_parse_and_body_work_share_fatal_job_limits() {
        std::thread::Builder::new().stack_size(1024*1024).spawn(|| {
            let grouped=format!("try{{Function({:?})();}}catch(e){{caught=true;}}finally{{finished=true;}}",format!("return {}0{};","(".repeat(180),")".repeat(180)));
            let mut accepted = realm();
            let mut host = TestHost::default();
            accepted.execute("var caught=false,finished=false;", &mut host).unwrap();
            assert_eq!(accepted.execute(&grouped, &mut host).unwrap().to_text(), "0");
            assert_eq!(accepted.execute("!caught&&finished;", &mut host).unwrap().to_text(), "true");
            let count = super::super::MAX_EXPRESSION_FRAMES + 1;
            let deep=format!("try{{Function({:?})();}}catch(e){{caught=true;}}finally{{finished=true;}}",format!("return {}0{};","(".repeat(count),")".repeat(count)));
            for source in [
                "var o={toString:function(){return Function(o);}};try{Function(o);}catch(e){caught=true;}finally{finished=true;}".to_owned(),
                "try{Function('while(true){}')();}catch(e){caught=true;}finally{finished=true;}".to_owned(),
                deep,
            ] {
                let mut runtime=realm();
                let mut host=TestHost::default();
                runtime.execute("var caught=false,finished=false;",&mut host).unwrap();
                let error=runtime.execute(&source,&mut host).unwrap_err();
                assert!(error.contains("limit"),"{error}");
                assert_eq!(runtime.call_depth,0);
                assert_eq!(runtime.eval_depth,0);
                runtime.begin_job();
                assert_eq!(runtime.execute("!caught&&!finished&&Function('return 7;')()===7;",&mut host).unwrap().to_text(),"true");
            }
        }).unwrap().join().unwrap();
    }
}
