//! Original bounded optional chains and nullish-coalescing grammar checks.
//! Chains retain ordinary references for method receivers and delete, while
//! every lookup/call keeps the existing host capability and execution budgets.
//! https://tc39.es/ecma262/#sec-optional-chains
//! https://tc39.es/ecma262/#sec-binary-logical-operators

use super::{
    Argument, ExprId, ExprKind, Fault, Host, Ordinary, Parser, Reference, Runtime,
    TokenKind, Value,
};
#[cfg(test)]
use std::rc::Rc;
#[cfg(test)]
use super::{Program, MAX_EVAL_DEPTH, MAX_EXPRESSION_FRAMES};

#[derive(Clone)]
pub(super) enum Step {
    Member { key: ExprId, optional: bool },
    Call { args: Vec<Argument>, optional: bool },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct NoHost {
        calls: usize,
        reads: usize,
    }
    impl Host for NoHost {
        fn allow_eval(&mut self) -> Result<bool, String> {
            Ok(true)
        }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            self.reads += 1;
            Err("Optional review native property is denied.".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("Optional review native write is denied.".into())
        }
        fn method(&self, object: u32, property: &str) -> Option<&'static str> {
            (object == 1 && property == "run").then_some("run")
        }
        fn resolve_member_before_call(&self, object: u32) -> bool {
            object == 1
        }
        fn call(&mut self, object: u32, method: &str, args: &[Value]) -> Result<Value, String> {
            if object == 1 && method == "run" {
                self.calls += 1;
                Ok(args.first().cloned().unwrap_or(Value::Undefined))
            } else {
                Err("Optional review native call is denied.".into())
            }
        }
    }
    fn succeeds(source: &str) {
        assert_eq!(
            Runtime::for_test()
                .execute(source, &mut NoHost::default())
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn optional_chains_skip_only_nullish_contiguous_parts_and_effects() {
        succeeds(
            r#"
var count=0,a=null,b;
if(a?.[count++]!==undefined||a?.method(count++)!==undefined||b?.(count++)!==undefined||count!==0)throw 'skipped effects';
if(a?.x.y.z!==undefined||a?.()[count++]!==undefined||count!==0)throw 'whole chain';
if(({a:{b:21}})?.a.b!==21)throw 'present';
if((0)?.toString()!=='0'||false?.valueOf()!==false||''?.length!==0)throw 'falsy receivers';
var errors=0;try{({a:null})?.a.b;}catch(e){if(e.name==='TypeError')errors++;}
try{(a?.x).y;}catch(e){if(e.name==='TypeError')errors++;}
try{(a?.x)(count++);}catch(e){if(e.name==='TypeError')errors++;}
if(errors!==3||count!==1)throw 'grouping boundary';'ok';
"#,
        );
    }

    #[test]
    fn optional_chains_preserve_references_receivers_and_lookup_order() {
        succeeds(
            r#"
var log='',o={value:4,get method(){log+='g';return function(x){'use strict';log+=this===o?'r':'x';return this===undefined?'none':this.value+x;};}};
function arg(){log+='a';return 2;}
if(o?.method(arg())!==6||log!=='gar')throw 'member order';log='';
if(o.method?.(arg())!==6||log!=='gar')throw 'optional call order';log='';
if((o?.method)(arg())!==6||log!=='gar')throw 'grouped reference';log='';
if((o.method)?.(arg())!==6||log!=='gar')throw 'ordinary grouped reference';log='';
if((0,o?.method)()!=='none'||log!=='gx')throw 'sequence strips reference';
var key={toString(){log+='k';return 'method';}};log='';
if(o?.[key]?.(arg())!==6||log!=='kgar')throw 'key conversion';
var s=Symbol('m');o[s]=function(){return this===o;};if(!o?.[s]?.())throw 'symbol receiver';'ok';
"#,
        );
    }

    #[test]
    fn optional_calls_get_callee_before_arguments_and_check_callability_after_them() {
        succeeds(
            r#"
var log='',a={get method(){log+='g';return undefined;}};
function arg(){log+='a';return 1;}
if(a.method?.(arg())!==undefined||log!=='g')throw 'nullish call';
var errors=0;try{({method:0}).method?.(arg());}catch(e){if(e.name==='TypeError')errors++;}
if(errors!==1||log!=='ga')throw 'callability order';
var marker={},after=0,o={get method(){throw marker;}};
try{o.method?.(after++);}catch(e){if(e===marker)errors++;}
if(after!==0||errors!==2)throw 'getter exception';
var live={n:3,get m(){return function(v){return this.n+v;};}};
if(live?.m((live.m=0,4))!==7)throw 'resolved callee';
function args(a,b){return a+b;}if(args?.(...[2,3])!==5)throw 'spread';'ok';
"#,
        );
    }

    #[test]
    fn optional_delete_uses_final_reference_without_invoking_getters() {
        succeeds(
            r#"
var reads=0,keys=0,o={get x(){reads++;return 1;}},p=Object.create({inherited:2});
if(!delete null?.[keys++]||keys!==0)throw 'null delete';
if(!delete o?.x||reads!==0||'x' in o)throw 'getter-free delete';
Object.defineProperty(o,'y',{get:function(){reads++;return 2;},configurable:true});
if(!delete (o?.y)||reads!==0||'y' in o)throw 'grouped delete';
if(!delete p?.inherited||p.inherited!==2)throw 'inherited delete';
Object.defineProperty(o,'fixed',{value:3,configurable:false});if(delete o?.fixed)throw 'nonconfigurable';
var caught=false;function strict(){'use strict';delete o?.fixed;}try{strict();}catch(e){caught=e.name==='TypeError';}
if(!caught)throw 'strict delete';
var key={toString(){keys++;return 'z';}};o.z=4;if(!delete o?.[key]||keys!==1||'z' in o)throw 'delete key';'ok';
"#,
        );
    }

    #[test]
    fn optional_chains_leave_unresolvable_names_and_indirect_eval_observable() {
        succeeds(
            r#"
var errors=0;try{missingOptionalName?.x;}catch(e){if(e.name==='ReferenceError')errors++;}
try{missingOptionalName?.();}catch(e){if(e.name==='ReferenceError')errors++;}
try{typeof missingOptionalName?.x;}catch(e){if(e.name==='ReferenceError')errors++;}
var marker=34;function read(){var marker=35;return eval?.('marker');}
if(read()!==34||marker!==34||errors!==3)throw 'name or eval';
var o={eval:eval};function other(){var marker=36;return o?.eval('marker');}
if(other()!==34)throw 'member indirect eval';'ok';
"#,
        );
    }

    #[test]
    fn nullish_operators_preserve_falsy_values_order_and_parenthesized_precedence() {
        succeeds(
            r#"
if((null??19)!==19||(undefined??20)!==20||(false??1)!==false||(0??1)!==0||(''??1)!==''||!Number.isNaN(NaN??1))throw 'nullish values';
var log='';function read(k,v){log+=k;return v;}
if((read('a',0)??read('b',2))!==0||(read('c',null)??read('d',undefined)??read('e',27))!==27||log!=='acde')throw 'order';
if(((false||null)??28)!==28||((1&&0)??29)!==0||(null??(false||30))!==30||(0??true?31:32)!==32)throw 'precedence';
if(((null??3)||4)!==3||(false&&(null??4))!==false)throw 'other grouping';
var obj={value:0};if((obj?.value??7)!==0||(null?.value??7)!==7)throw 'chain combination';'ok';
"#,
        );
    }

    #[test]
    fn optional_lexical_goals_keep_decimal_conditionals_reserved_names_and_regexps() {
        succeeds(
            r#"
if((true?.3:0)!==0.3||(false?.3:0)!==0)throw 'decimal lookahead';
var o={return:6,function:8,if:function(v){return v;}};
if(o?.return / 2!==3||o?.function / 2!==4||!o?.if(/}/).test('}'))throw 'lexical goals';
if(`${o?.return??0}`!=='6'||`${null?.return??0}`!=='0')throw 'template combination';
function C(v){this.value=v;}var holder={C:C};
if(new C(4)?.value!==4||new (holder?.C)(5).value!==5)throw 'valid construction';'ok';
"#,
        );
    }

    #[test]
    fn optional_and_nullish_early_errors_are_atomic_and_catchable() {
        succeeds(
            r#"
var ran=false,count=0;for(var text of ['a??b||c','a||b??c','a??b&&c','a&&b??c','a?.x=1','a?.x++','++a?.x','new a?.()','a?.`x`','a?.x`x`']){
try{eval('ran=true;'+text);}catch(e){if(e.name==='SyntaxError')count++;}if(ran)throw 'partial execution';}
if(count!==10)throw 'early errors';'ok';
"#,
        );
    }

    #[test]
    fn optional_native_calls_preserve_registered_receivers_and_denied_capabilities() {
        let mut runtime = Runtime::for_test();
        runtime.define_global("native", Value::Host(1)).unwrap();
        runtime.define_global("opaque", Value::Host(2)).unwrap();
        let mut host = NoHost::default();
        assert_eq!(runtime.execute("if(native?.run(3)!==3||native.run?.(4)!==4||(native?.run)(5)!==5)throw 'native receiver';'ok';", &mut host).unwrap().to_text(), "ok");
        assert_eq!(host.calls, 3);
        assert_eq!(host.reads, 0);
        let error = runtime.execute("var continued=false;try{opaque?.run?.();}catch(e){continued=true;}finally{continued=true;}", &mut host).unwrap_err();
        assert!(error.contains("native property is denied"), "{error}");
        assert_eq!(
            runtime.execute("continued", &mut host).unwrap().to_text(),
            "false"
        );
        assert_eq!(host.calls, 3);
    }

    #[test]
    fn optional_chain_steps_are_bounded_counted_and_do_not_reset_execution() {
        let source = format!("null{}", "?.x".repeat(MAX_EXPRESSION_FRAMES));
        let mut runtime = Runtime::for_test();
        assert!(matches!(
            runtime.execute(&source, &mut NoHost::default()).unwrap(),
            Value::Undefined
        ));
        let program = runtime.programs.last().unwrap();
        assert_eq!(program.chain_steps, MAX_EXPRESSION_FRAMES);
        assert_eq!(runtime.retained_ast, program.node_count());
        assert!(runtime.retained_ast > 2 * MAX_EXPRESSION_FRAMES);
        let source = format!(
            "var ran=true;null{}",
            "?.x".repeat(MAX_EXPRESSION_FRAMES + 1)
        );
        let mut runtime = Runtime::for_test();
        let error = runtime
            .execute(&source, &mut NoHost::default())
            .unwrap_err();
        assert!(error.contains("Optional-chain step limit"), "{error}");
        assert_eq!(
            runtime
                .execute("typeof ran", &mut NoHost::default())
                .unwrap()
                .to_text(),
            "undefined"
        );
        let mut runtime = Runtime::for_test();
        let error = runtime.execute("var continued=false,o={f(){while(true){}}};try{o?.f();}catch(e){continued=true;}finally{continued=true;}", &mut NoHost::default()).unwrap_err();
        assert!(error.contains("limit"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("continued", &mut NoHost::default())
                .unwrap()
                .to_text(),
            "false"
        );
    }
}

impl Parser {
    pub(super) fn chain_property(&mut self) -> Result<ExprId, Fault> {
        let TokenKind::Word(name) = &self.token().kind else {
            return Err(self.error("Expected a property name after the optional-chain operator."));
        };
        let name = name.clone();
        let at = self.at();
        self.current += 1;
        self.expr(ExprKind::Literal(Value::String(name)), at)
    }

    pub(super) fn check_coalescing(
        &self,
        op: &str,
        left: ExprId,
        right: ExprId,
    ) -> Result<(), Fault> {
        for id in [left, right] {
            let expression = &self.program.exprs[id];
            if expression.parenthesized {
                continue;
            }
            if let ExprKind::Binary(child, _, _) = expression.kind {
                if op == "??" && matches!(child, "&&" | "||")
                    || matches!(op, "&&" | "||") && child == "??"
                {
                    return Err(self.error(
                        "Nullish coalescing cannot mix with && or || without parentheses.",
                    ));
                }
            }
        }
        Ok(())
    }
}

pub(super) enum Completion {
    Value(Value),
    Reference(Reference),
}

// A missing value here is only the existing legacy native-member call route.
// An optional call must read its callee before it can decide to skip a call.
pub(super) struct CallTarget {
    pub(super) function: Option<Value>,
    pub(super) receiver: Value,
    pub(super) name: Option<String>,
}

impl Runtime {
    pub(super) fn chain_value(&mut self, completion: Completion, host: &mut dyn Host) -> Result<Value, Fault> {
        match completion {
            Completion::Value(value) => Ok(value),
            Completion::Reference(reference) => self.read_reference(&reference, host),
        }
    }

    #[cfg(test)]
    fn chain_operand(
        &mut self,
        program: &Rc<Program>,
        id: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Completion, Fault> {
        match program.exprs[id].kind {
            ExprKind::Name(_) | ExprKind::Member(_, _) => self
                .reference(program, id, env, this, host)
                .map(Completion::Reference),
            ExprKind::OptionalChain(_, _) => self.chain_reference(program, id, env, this, host),
            _ => self
                .eval(program, id, env, this, host)
                .map(Completion::Value),
        }
    }

    #[cfg(test)]
    fn chain_reference(
        &mut self,
        program: &Rc<Program>,
        id: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Completion, Fault> {
        self.step()?;
        if self.eval_depth >= MAX_EVAL_DEPTH {
            return Err(Fault::new(
                "Expression evaluation depth limit reached (128).",
            ));
        }
        self.eval_depth += 1;
        let result = (|| {
            let ExprKind::OptionalChain(base, steps) = &program.exprs[id].kind else {
                unreachable!()
            };
            let mut current = self.chain_operand(program, *base, env, this, host)?;
            for step in steps {
                self.step()?;
                current = match step {
                    Step::Member { key, optional } => {
                        let value = self.chain_value(current, host)?;
                        if *optional && matches!(value, Value::Undefined | Value::Null) {
                            return Ok(Completion::Value(Value::Undefined));
                        }
                        let key = self.eval(program, *key, env, this, host)?;
                        Completion::Reference(Reference::ComputedProperty(value, key))
                    }
                    Step::Call { args, optional } => {
                        let target = self.prepare_chain_call(current, *optional, host)?;
                        if *optional
                            && matches!(
                                target.function.as_ref(),
                                Some(Value::Undefined | Value::Null)
                            )
                        {
                            return Ok(Completion::Value(Value::Undefined));
                        }
                        let args = self.eval_argument_list(program, args, env, this, host)?;
                        Completion::Value(self.invoke_chain_call(target, &args, host)?)
                    }
                };
            }
            Ok(current)
        })();
        self.eval_depth -= 1;
        result.map_err(|error: Fault| error.at(&program.source, program.exprs[id].at))
    }

    pub(super) fn prepare_chain_call(
        &mut self,
        current: Completion,
        optional: bool,
        host: &mut dyn Host,
    ) -> Result<CallTarget, Fault> {
        let reference = match current {
            Completion::Value(function) => {
                return Ok(CallTarget {
                    function: Some(function),
                    receiver: Value::Undefined,
                    name: None,
                })
            }
            Completion::Reference(Reference::ComputedProperty(object, key)) => {
                Self::require_property_receiver(&object)?;
                self.property_key(key, host)?.reference(object)
            }
            Completion::Reference(reference) => reference,
        };
        match reference {
            Reference::Property(receiver, name) => {
                Self::require_property_receiver(&receiver)?;
                // Same lookup-before-arguments routing as an ordinary member
                // call. Expandos and explicit host methods keep their receiver.
                let function = if let Some(ordinary) = Ordinary::from_value(&receiver) {
                    Some(self.get_ordinary_property_with_host(ordinary, &name, host)?)
                } else if let Value::Host(handle) = receiver {
                    if self.foreign_window(handle, host)
                        || self.host_ordinary(handle, &name, host)?.is_some()
                        || host.ordinary_prototype(handle)
                        || host.ordinary_lookup_tail_property(handle,&name)
                        || host.resolve_member_before_call(handle)
                    {
                        Some(self.host_read(handle, &name, host)?)
                    } else if self.is_browser_global(handle) && !self.host_owns_global(&name, host) {
                        Some(self.global_value(&name, host)?)
                    } else if optional {
                        Some(self.host_read(handle, &name, host)?)
                    } else {
                        None
                    }
                } else {
                    Some(self.read_property(&receiver, &name, host)?)
                };
                Ok(CallTarget {
                    function,
                    receiver,
                    name: Some(name),
                })
            }
            Reference::SymbolProperty(receiver, key) => {
                let object = self.copy_value(&receiver)?;
                let function = self.get_symbol_property(object, key, host)?;
                Ok(CallTarget {
                    function: Some(function),
                    receiver,
                    name: None,
                })
            }
            reference => {
                let function = self.read_reference(&reference, host)?;
                Ok(CallTarget {
                    function: Some(function),
                    receiver: Value::Undefined,
                    name: None,
                })
            }
        }
    }

    pub(super) fn invoke_chain_call(
        &mut self,
        target: CallTarget,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match (target.function, target.name) {
            (Some(function), Some(name)) => {
                self.call_named_value(function, args, target.receiver, &name, host)
            }
            (Some(function), None) => self.call_value(function, args, target.receiver, host),
            (None, Some(name)) => self.call_member(target.receiver, &name, args, host),
            (None, None) => unreachable!(),
        }
    }

    #[cfg(test)]
    pub(super) fn eval_optional_chain(
        &mut self,
        program: &Rc<Program>,
        id: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let completion = self.chain_reference(program, id, env, this, host)?;
        self.chain_value(completion, host)
    }

    #[cfg(test)]
    pub(super) fn call_parenthesized_chain(
        &mut self,
        program: &Rc<Program>,
        callee: ExprId,
        args: &[Argument],
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let completion = self.chain_reference(program, callee, env, this, host)?;
        let target = self.prepare_chain_call(completion, false, host)?;
        let args = self.eval_argument_list(program, args, env, this, host)?;
        self.invoke_chain_call(target, &args, host)
    }

    #[cfg(test)]
    pub(super) fn delete_optional_chain(
        &mut self,
        program: &Rc<Program>,
        id: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match self.chain_reference(program, id, env, this, host)? {
            Completion::Reference(reference) => self.delete_reference(reference, host),
            Completion::Value(_) => Ok(Value::Bool(true)),
        }
    }
}
