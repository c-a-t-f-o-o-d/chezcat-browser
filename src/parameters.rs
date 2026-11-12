//! Original bounded formal-parameter initialization. Defaults execute only after
//! every formal binding exists, with a separate body scope so initializer
//! closures cannot see body declarations. Direct eval keeps its existing gates.
//! https://tc39.es/ecma262/#sec-functiondeclarationinstantiation

use super::{
    collect_var_names_with_work, Control, Declaration, ExprId, ExprKind, Fault, Host, Ordinary,
    Program, Runtime, StmtKind, Template, Value,
};
use std::rc::Rc;

impl Template {
    pub(super) fn has_parameter_expressions(&self, program: &Program) -> bool {
        self.params
            .iter()
            .any(|p| p.initial.is_some() || program.patterns[p.pattern].expressions)
    }

    pub(super) fn non_simple(&self, program: &Program) -> bool {
        self.rest
            || self.params.iter().any(|p| {
                p.initial.is_some()
                    || !matches!(
                        program.patterns[p.pattern].kind,
                        super::binding_patterns::Kind::Name(_)
                    )
            })
    }

    pub(super) fn parameter_names(&self, program: &Program) -> Vec<String> {
        self.params
            .iter()
            .flat_map(|p| program.pattern_names(p.pattern))
            .collect()
    }

    pub(super) fn parameter_length(&self) -> usize {
        self.params
            .iter()
            .position(|p| p.initial.is_some())
            .unwrap_or(self.params.len() - usize::from(self.rest))
    }
}

impl Runtime {
    pub(super) fn parameter_initializer(
        &mut self,
        program: &Rc<Program>,
        expression: ExprId,
        name: Option<&str>,
        environment: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let value = self.eval(program, expression, environment, this, host)?;
        if let (Some(name), ExprKind::Function(template), Value::Function(function)) =
            (name, &program.exprs[expression].kind, &value)
        {
            if program.templates[*template].name.is_none() {
                let object = self.ordinary_id(Ordinary::Function(*function))?;
                self.store_property(
                    object,
                    "name".into(),
                    Value::String(name.into()),
                    Some((false, false, true)),
                )?;
            }
        }
        Ok(value)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn script_parameters_and_body(
        &mut self,
        program: &Rc<Program>,
        template: usize,
        callee: usize,
        parent: usize,
        args: &[Value],
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let body = self.prepare_script_call(program, template, callee, parent, args, this, host)?;
        match self.statements(program, &program.templates[template].body, body, this, host)? {
            Control::Return(value) => Ok(value),
            Control::Normal(_) => Ok(Value::Undefined),
            _ => Err(Fault::new("Unexpected loop control outside a loop.")),
        }
    }

    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn prepare_script_call(
        &mut self,
        program: &Rc<Program>,
        template: usize,
        callee: usize,
        parent: usize,
        args: &[Value],
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<usize, Fault> {
        let function = &program.templates[template];
        let has_expressions = function.has_parameter_expressions(program);
        let call_scope = self.new_environment(parent, true)?;
        // Sloppy default eval may introduce vars, but cannot replace formal
        // bindings. Keeping these records distinct also preserves its closures.
        let parameters = if has_expressions && !function.strict {
            self.new_environment(call_scope, false)?
        } else {
            call_scope
        };
        for parameter in &function.params {
            self.step()?;
            for name in program.pattern_names_with_work(parameter.pattern, |n| self.work(n))? {
                if !self.environment(parameters).bindings.contains_key(&name) {
                    self.put_binding(parameters, name, None, Declaration::Var, false)?;
                }
            }
        }
        self.prepare_arguments(parameters, callee, program, template, args)?;
        for (position, parameter) in function.params.iter().enumerate() {
            self.step()?;
            let mut value = if function.rest && position + 1 == function.params.len() {
                self.array_from_values(args.get(position..).unwrap_or(&[]))?
            } else {
                args.get(position)
                    .map_or(Ok(Value::Undefined), |v| self.copy_value(v))?
            };
            if let (Value::Undefined, Some(initial)) = (&value, parameter.initial) {
                value = self.parameter_initializer(
                    program,
                    initial,
                    program.pattern_name(parameter.pattern),
                    parameters,
                    this,
                    host,
                )?;
            }
            self.initialize_pattern(
                program,
                parameter.pattern,
                value,
                super::binding_patterns::Mode::Initialize(parameters),
                parameters,
                this,
                host,
            )?;
        }
        let variables = if has_expressions {
            let scope = self.new_environment(parameters, true)?;
            for name in collect_var_names_with_work(program, &function.body, |n| self.work(n))? {
                self.step()?;
                if self.environment(scope).bindings.contains_key(&name) {
                    continue;
                }
                let replaced_by_function = function.body.iter().any(
                    |id| matches!(&program.stmts[*id].kind, StmtKind::Function(n, _) if n == &name),
                );
                let value = if !replaced_by_function
                    && self.environment(parameters).bindings.contains_key(&name)
                {
                    self.binding_value(parameters, &name)?
                } else {
                    Value::Undefined
                };
                self.put_binding(scope, name, Some(value), Declaration::Var, false)?;
            }
            scope
        } else {
            parameters
        };
        let body = if has_expressions && !function.strict {
            self.new_environment(variables, false)?
        } else {
            variables
        };
        self.prepare_scopes(program, &function.body, body, variables, true, host)?;
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            Err("No native reads.".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("No native writes.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("No native calls.".into())
        }
        fn allow_eval(&mut self) -> Result<bool, String> {
            Ok(true)
        }
    }
    fn succeeds(source: &str) {
        assert_eq!(
            Runtime::for_test()
                .execute(source, &mut NoHost)
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn default_parameters_order_undefined_only_and_metadata() {
        succeeds(
            r#"
var order='';function f(a=(order+='a',2),b=(order+='b',a+3),c){return [a,b,c].join(':');}
if(f()!=='2:5:'||order!=='ab')throw 'default ordering';
if(f(null,0,false)!==':0:false'||order!=='ab')throw 'undefined only';
function g(a,b=2,c,...rest){return rest;}if(f.length!==0||g.length!==1||g(1,2,3,4)[0]!==4)throw 'length/rest';
function h(a=function(){},b=()=>1,c=function named(){}){return [a.name,b.name,c.name].join();}
if(h()!=='a,b,named')throw 'initializer names';'ok';
"#,
        );
    }

    #[test]
    fn default_parameters_tdz_and_body_scope_are_separate() {
        succeeds(
            r#"
var out='';function a(x=x){return x;}function b(x=y,y=2){return x;}
try{a();}catch(e){out+=e.name;}try{b();}catch(e){out+=':'+e.name;}
if(out!=='ReferenceError:ReferenceError')throw 'TDZ';
var x=8;function f(a=x,read=()=>a){var x=99;var a=4;return [a,read(),x].join();}
if(f()!=='4,8,99')throw 'body scopes';
function hidden(a=()=>secret){var secret=3;return a;}
try{hidden()();throw 'missing ReferenceError';}catch(e){if(e.name!=='ReferenceError')throw e;}
function replaced(a=2,read=()=>a){function a(){return 5;}return [a(),read()].join();}
if(replaced()!=='5,2')throw 'function-body overwrite';'ok';
"#,
        );
    }

    #[test]
    fn default_parameters_arguments_before_defaults_and_unmapped() {
        succeeds(
            r#"
function f(a=3,b=arguments.length){a=7;arguments[0]=9;return [a,b,arguments.length,arguments[0]].join();}
if(f(1)!=='7,1,1,9'||f()!=='7,0,0,9')throw 'arguments';
function g(a=arguments[0]){let arguments=5;return a+arguments;}
if(g(3)!==8)throw 'body lexical arguments';
function h(a=arguments.length){function arguments(){return 5;}return a+arguments();}
if(h()!==5)throw 'body function arguments';
function restricted(a=1){try{return arguments.callee;}catch(e){return e.name;}}
if(restricted()!=='TypeError')throw 'unmapped callee';'ok';
"#,
        );
    }

    #[test]
    fn default_parameters_eval_uses_outer_parameter_variable_scope() {
        succeeds(
            r#"
function f(a=eval('var hidden=4; hidden'),b=()=>hidden){var hidden=9;return [a,b(),hidden].join();}
if(f()!=='4,4,9')throw 'eval scope';
function g(a=eval('var a=4')){}var result='';try{g();}catch(e){result=e.name;}
if(result!=='SyntaxError')throw 'eval conflict';
function outer(){'use strict';return function(a=eval('var local=4;local'),b=()=>typeof local){return a+':'+b();};}
if(outer()()!=='4:undefined')throw 'strict eval';'ok';
"#,
        );
    }

    #[test]
    fn default_parameters_methods_arrows_constructors_and_dynamic_function() {
        succeeds(
            r#"
function Box(x=4){this.x=x;}var obj={v:6,m(x=this.v){return x;}};
var f=(a=2,...r)=>a+r.length;
if(new Box().x!==4||obj.m()!==6||f(undefined,7,8)!==4||f.length!==0)throw 'forms';
var g=Function('a=3','b=a+2','return a*b;');if(g()!==15||g(4)!==24||g.length!==0)throw 'dynamic';
var set={set x(a=4){this.value=a;}};set.x=undefined;if(set.value!==4)throw 'setter default';
function nested(){return (a=()=>this.x+arguments[0])=>a();}
if(nested.call({x:3},4)()!==7)throw 'lexical arrow';'ok';
"#,
        );
    }

    #[test]
    fn default_parameters_early_errors_happen_before_effects() {
        for source in [
            "var effect=1;function f(a=1,a){}",
            "var effect=1;function f(a=1){'use strict';}",
            "var effect=1;function f(...a=[]){}",
            "var effect=1;var f=(a,a=2)=>a;",
            "var effect=1;var o={set x(a=1,){}};",
            "var effect=1;function f(a=1){let a;}",
        ] {
            let mut runtime = Runtime::for_test();
            let error = runtime.execute(source, &mut NoHost).unwrap_err();
            assert!(
                error.contains("SyntaxError")
                    || error.contains("parameter")
                    || error.contains("declaration"),
                "{error}"
            );
            runtime.begin_job();
            assert_eq!(
                runtime
                    .execute("typeof effect;", &mut NoHost)
                    .unwrap()
                    .to_text(),
                "undefined"
            );
        }
    }

    #[test]
    fn default_parameters_callback_limits_remain_fatal() {
        let mut runtime = Runtime::for_test();
        runtime
            .execute("var caught=false,finalized=false;", &mut NoHost)
            .unwrap();
        let error = runtime
            .execute(
                "function f(a=f()){}try{f();}catch(e){caught=true;}finally{finalized=true;}",
                &mut NoHost,
            )
            .unwrap_err();
        assert!(error.contains("limit"), "{error}");
        assert_eq!(runtime.call_depth, 0);
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("!caught&&!finalized;", &mut NoHost)
                .unwrap()
                .to_text(),
            "true"
        );
    }
}
