//! Borrow a string only across a proven callback-free character call.
//! All other member calls retain their owned receiver/callee snapshots.

use super::{Argument, Builtin, ExprId, ExprKind, Fault, Function, Host, Program,
    Runtime, Value, WorkKind, MAX_CALL_DEPTH, MAX_EVALUATION_FRAMES, MAX_STRING_BYTES};
use std::rc::Rc;

impl Runtime {
    // An uncharged, read-only eligibility probe. The admitted path repeats the
    // normal charged lookups in receiver/key/argument order. No rejected probe
    // reads host properties, invokes getters, materializes arguments or throws.
    fn character_local_scope(&self, mut env: usize, name: &str) -> Option<usize> {
        loop {
            let current = self.environment(env);
            if current.bindings.contains_key(name) {
                if name == "arguments" && current.arguments.is_some()
                    || env == 0 && self.browser_global.is_some() && self.global_binding(name) {
                    return None;
                }
                return Some(env);
            }
            env = current.parent?;
        }
    }

    pub(super) fn try_binding_character(
        &mut self, program: &Rc<Program>, base: ExprId, key: ExprId,
        arguments: &[Argument], env: usize, host: &mut dyn Host,
    ) -> Option<Result<Value, Fault>> {
        let ExprKind::Name(name) = &program.exprs[base].kind else { return None };
        let ExprKind::Literal(Value::String(method)) = &program.exprs[key].kind else { return None };
        if method != "charCodeAt" || arguments.len() != 1 || arguments[0].spread
            || self.evaluation_frames >= MAX_EVALUATION_FRAMES { return None; }
        let argument = arguments[0].expression;
        let ExprKind::Name(index_name) = &program.exprs[argument].kind else { return None };
        let scope = self.character_local_scope(env, name)?;
        let index_scope = self.character_local_scope(env, index_name)?;
        let binding = &self.environment(scope).bindings[name];
        let Some(Value::String(text)) = binding.value.as_ref() else { return None };
        if !matches!(self.environment(index_scope).bindings[index_name].value, Some(Value::Number(_))) {
            return None;
        }
        let prototype = self.string_prototype?;
        let property = self.objects.get(prototype)?.properties.get(method)?;
        if property.getter.is_some() || property.accessor.is_some() { return None; }
        let Value::Function(function) = property.value else { return None };
        if !matches!(self.functions.get(function), Some(Function::Native(Builtin::StringMethod("charCodeAt")))) {
            return None;
        }
        let bytes = text.len();
        Some((|| {
            // These are heap-expression child evaluations, with the same step
            // and location rules, without adding native recursive eval frames.
            self.step()?;
            (|| {
                let resolved = self.lookup(env, name)?;
                debug_assert_eq!(resolved, Some(scope));
                Ok::<_, Fault>(())
            })().map_err(|error| error.at(&program.source, program.exprs[base].at))?;
            self.step()?;
            let key_value = (|| {
                let ExprKind::Literal(value) = &program.exprs[key].kind else { unreachable!() };
                self.copy_value(value)
            })().map_err(|error| error.at(&program.source, program.exprs[key].at))?;
            // The descriptor above is an own native data method. Its lookup
            // cannot observe this empty receiver: accessors/overrides fall back
            // before entering this path. Keep the actual property-read charges.
            let _ = self.resolve_member_call(Value::String(String::new()), key_value, host)?;
            self.step()?;
            let index = (|| {
                let scope = self.lookup(env, index_name)?.expect("eligible index binding");
                self.binding_value(scope, index_name)
            })().map_err(|error| error.at(&program.source, program.exprs[argument].at))?;
            self.step()?; // call_value
            if bytes > MAX_STRING_BYTES { return Err(Fault::new("String limit reached (128 KiB).")); }
            if self.call_depth >= MAX_CALL_DEPTH { return Err(Fault::new("Function call limit reached (32).")); }
            self.call_depth += 1;
            let result = (|| {
                self.step()?; // call_member
                let Value::Number(position) = index else { unreachable!() };
                self.work_as(0, WorkKind::ConversionOutput)?;
                let position = if position.is_nan() { 0.0 } else { position.trunc() };
                if position < 0.0 || position >= bytes as f64 { return Ok(Value::Number(f64::NAN)); }
                Ok(Value::Number(self.binding_character_unit(scope, name, position as usize)?
                    .map_or(f64::NAN, f64::from)))
            })();
            self.call_depth -= 1;
            result
        })())
    }

    fn binding_character_unit(&mut self, scope: usize, name: &str, target: usize) -> Result<Option<u16>, Fault> {
        let binding = &self.environment(scope).bindings[name];
        let Some(Value::String(text)) = binding.value.as_ref() else { unreachable!() };
        let bytes = text.len();
        if target >= bytes { return Ok(None); }
        // Equality is an ASCII proof: every non-ASCII scalar has strictly more
        // UTF-8 bytes than UTF-16 units. No fresh uncharged whole-string scan.
        if binding.utf16_length == Some(bytes) {
            self.work_as(1, WorkKind::CharacterScan)?;
            let Some(Value::String(text)) = self.environment(scope).bindings[name].value.as_ref()
                else { unreachable!() };
            return Ok(Some(u16::from(text.as_bytes()[target])));
        }
        let (mut byte, mut unit) = binding.utf16_cursor.filter(|(_, unit)| *unit <= target).unwrap_or((0, 0));
        while byte < bytes {
            let Some(Value::String(text)) = self.environment(scope).bindings[name].value.as_ref()
                else { unreachable!() };
            let first = text.as_bytes()[byte];
            let width = if first < 0x80 { 1 } else if first < 0xe0 { 2 } else if first < 0xf0 { 3 } else { 4 };
            // Charge every visited scalar before decoding. No borrow spans the
            // mutable work charge and no cursor is published on failed work.
            self.work_as(width, WorkKind::CharacterScan)?;
            let Some(Value::String(text)) = self.environment(scope).bindings[name].value.as_ref()
                else { unreachable!() };
            let character = text[byte..].chars().next().expect("UTF-8 scalar boundary");
            let units = character.len_utf16();
            if target < unit + units {
                let mut buffer = [0; 2];
                let result = character.encode_utf16(&mut buffer)[target - unit];
                self.environment_mut(scope).bindings.get_mut(name).unwrap().utf16_cursor = Some((byte, unit));
                return Ok(Some(result));
            }
            byte += width;
            unit += units;
        }
        self.environment_mut(scope).bindings.get_mut(name).unwrap().utf16_cursor = Some((byte, unit));
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Host, Runtime, Value};

    struct NoHost;
    impl Host for NoHost {
        fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("unexpected host get".into()) }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("unexpected host set".into()) }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("unexpected host call".into()) }
    }

    #[test]
    fn binding_character_original_ascii_scan_is_linear_and_bounded() {
        let mut runtime = Runtime::new();
        runtime.define_global("payload", Value::String("A".repeat(32768))).unwrap();
        runtime.begin_job();
        let result = runtime.execute(
            "let total=0;for(let index=0;index<16384;index++)total+=payload.charCodeAt(index);total;",
            &mut NoHost,
        );
        assert_eq!(result.unwrap().to_number(), 65.0 * 16384.0);
        assert!(runtime.job_usage().1 < 16 * 1024 * 1024, "{:?}", runtime.job_usage());
    }

    #[test]
    fn binding_character_unicode_and_replacements_preserve_units() {
        let source = "let payload='ABCD',index=1;payload.length;if(payload.charCodeAt(index)!==66)throw 'ascii';payload='😀';if(payload.charCodeAt(index)!==56832)throw 'replacement';payload='Aé😀Z';let expected=[65,233,55357,56832,90];for(index=0;index<5;index++)if(payload.charCodeAt(index)!==expected[index])throw 'unicode';'ok';";
        assert_eq!(Runtime::new().execute(source, &mut NoHost).unwrap().to_text(), "ok");
    }

    #[test]
    fn binding_character_effectful_arguments_getters_and_overrides_keep_snapshots() {
        let source = "let payload='AB',index=1,native=String.prototype.charCodeAt,log='';if(payload.charCodeAt((payload='CD',index))!==66)throw 'argument';payload='AB';Object.defineProperty(String.prototype,'charCodeAt',{configurable:true,get:function(){log+='G';payload='CD';return native;}});if(payload.charCodeAt(index)!==66||log!=='G')throw 'getter';Object.defineProperty(String.prototype,'charCodeAt',{configurable:true,writable:true,value:function(i){'use strict';return this==='CD'&&i===1?7:8;}});if(payload.charCodeAt(index)!==7)throw 'override';'ok';";
        assert_eq!(Runtime::new().execute(source, &mut NoHost).unwrap().to_text(), "ok");
    }

    #[test]
    fn binding_character_unicode_cursor_is_linear_and_resets_on_all_writes() {
        let mut runtime = Runtime::new();
        runtime.define_global("payload", Value::String("Aé😀Z".repeat(4096))).unwrap();
        runtime.begin_job();
        let source = "let total=0;for(let index=0;index<20480;index++)total+=payload.charCodeAt(index);total;";
        assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_number(),
            f64::from(65 + 233 + 55357 + 56832 + 90) * 4096.0);
        assert!(runtime.job_usage().1 < 16 * 1024 * 1024);
        let source = "function mapped(s){let i=2;s.charCodeAt(i);arguments[0]='é😀';return s.charCodeAt(i);}if(mapped('ABCD')!==56832)throw 'mapped';function evaluated(s){let i=2;s.charCodeAt(i);eval(\"s='é😀'\");return s.charCodeAt(i);}if(evaluated('ABCD')!==56832)throw 'eval';'ok';";
        runtime.begin_job();
        assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_text(), "ok");
    }

    #[test]
    fn binding_character_scan_charges_before_decode_and_failure_restores_depth() {
        let mut runtime = Runtime::for_test();
        runtime.define_global("payload", Value::String("Aé😀Z".into())).unwrap();
        for budget in 0..=7 {
            runtime.environment_mut(0).bindings.get_mut("payload").unwrap().utf16_cursor = None;
            runtime.begin_job();
            runtime.copy_bytes = budget;
            let result = runtime.binding_character_unit(0, "payload", 2);
            if budget == 7 {
                assert_eq!(result.unwrap(), Some(55357));
                assert_eq!(runtime.environment(0).bindings["payload"].utf16_cursor, Some((3, 2)));
            } else {
                assert!(result.unwrap_err().exception.is_none());
                assert_eq!(runtime.environment(0).bindings["payload"].utf16_cursor, None);
            }
            assert_eq!(runtime.copy_bytes, 0);
        }
        runtime.begin_job();
        runtime.define_global("index", Value::Number(1.0)).unwrap();
        runtime.call_depth = super::MAX_CALL_DEPTH;
        assert!(runtime.execute("payload.charCodeAt(index);", &mut NoHost).unwrap_err().contains("Function call limit"));
        assert_eq!(runtime.call_depth, super::MAX_CALL_DEPTH);
        runtime.call_depth = 0;
        runtime.begin_job();
        assert_eq!(runtime.execute("payload.charCodeAt(index);", &mut NoHost).unwrap().to_number(), 233.0);
        assert_eq!((runtime.call_depth, runtime.eval_depth, runtime.evaluation_frames), (0, 0, 0));
    }

    #[test]
    fn binding_character_steps_match_original_member_route_at_every_boundary() {
        use super::super::{Fault, Parser, StmtKind, MAX_REGEXP_WORK};
        use std::rc::Rc;
        let program = Rc::new(Parser::parse("payload.charCodeAt(index);", &mut MAX_REGEXP_WORK.clone()).unwrap());
        let StmtKind::Expr(id) = program.stmts[program.body[0]].kind else { unreachable!() };
        let super::super::ExprKind::Call(callee, ref arguments) = program.exprs[id].kind else { unreachable!() };
        let super::super::ExprKind::Member(base, key) = program.exprs[callee].kind else { unreachable!() };
        for budget in 0..32 {
            let mut observations = Vec::new();
            for original in [true, false] {
                let mut runtime = Runtime::for_test();
                runtime.define_global("payload", Value::String("Aé😀Z".into())).unwrap();
                runtime.define_global("index", Value::Number(3.0)).unwrap();
                runtime.begin_job();
                runtime.steps = budget;
                let result = if original {
                    runtime.step().and_then(|_| (|| {
                        let (receiver, property, function) = runtime.prepare_member_call(&program, base, key, 0, &Value::Undefined, &mut NoHost)?;
                        let args = runtime.eval_argument_list(&program, arguments, 0, &Value::Undefined, &mut NoHost)?;
                        runtime.invoke_member_call(receiver, property, function, &args, &mut NoHost)
                    })().map_err(|error: Fault| error.at(&program.source, program.exprs[id].at)))
                } else {
                    runtime.eval(&program, id, 0, &Value::Undefined, &mut NoHost)
                }.map(|value| value.to_text()).map_err(Fault::display);
                assert_eq!((runtime.call_depth, runtime.eval_depth, runtime.evaluation_frames), (0, 0, 0));
                observations.push((result, runtime.steps));
            }
            assert_eq!(observations[0], observations[1], "budget={budget}");
        }
    }
}
