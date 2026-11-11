//! Bounded String.prototype.replace for literal and native RegExp patterns.
//! Callable replacements receive the native captures, UTF-16 position and
//! original string. Global matching finishes before the first callback.
//! Custom Symbol.replace/RegExp execution protocols and named groups remain
//! outside the existing subset; template substitutions use the original helper.

use super::{regexp, Fault, Host, Runtime, Value, MAX_ARRAY_LENGTH, MAX_RETAINED_VALUES};

impl Runtime {
    pub(super) fn string_replace(
        &mut self,
        text: &str,
        pattern: &Value,
        regex: Option<usize>,
        replacement: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let callable = matches!(replacement, Value::Function(_));
        // Literal search conversion precedes replacement conversion. A native
        // RegExp keeps its identity and does not stringify its pattern.
        let literal = if regex.is_none() {
            let pattern = self.copy_value(pattern)?;
            let pattern = self.replacement_string_value(pattern, host)?;
            Some(pattern.encode_utf16().collect::<Vec<_>>())
        } else {
            None
        };
        let template = if callable {
            None
        } else {
            let replacement = self.copy_value(replacement)?;
            Some(self.replacement_string_value(replacement, host)?)
        };
        let input: Vec<_> = text.encode_utf16().collect();
        let global = if let Some(id) = regex {
            self.object(id)?.regexp.as_ref().unwrap().flags.global
        } else {
            false
        };
        if global {
            self.set_property_throw(regex.unwrap(), "lastIndex".into(), Value::Number(0.0))?;
        }

        if callable {
            let mut matches = Vec::new();
            let mut snapshot_bytes = 0usize;
            loop {
                self.step()?;
                let Some(found) = self.next_replacement_match(regex, literal.as_deref(), &input)?
                else {
                    break;
                };
                if matches.len() >= MAX_ARRAY_LENGTH {
                    return Err(Fault::new("Replacement match limit reached (8192)."));
                }
                let bytes = std::mem::size_of::<regexp::Match>()
                    + found.captures.len() * std::mem::size_of::<Option<(usize, usize)>>();
                snapshot_bytes = snapshot_bytes.saturating_add(bytes);
                if snapshot_bytes > MAX_RETAINED_VALUES {
                    return Err(Fault::new("Replacement snapshot limit reached (2 MiB)."));
                }
                // Range snapshots retain no duplicate source strings and also
                // spend the shared job allowance, including across recursion.
                self.work(bytes)?;
                let empty_end = (found.start() == found.end()).then_some(found.end());
                matches.push(found);
                if !global {
                    break;
                }
                if let Some(end) = empty_end {
                    self.set_property_throw(
                        regex.unwrap(),
                        "lastIndex".into(),
                        Value::Number((end + 1) as f64),
                    )?;
                }
            }
            let mut output = String::new();
            let mut cursor = 0;
            for found in matches {
                self.step()?;
                let mut arguments = Vec::with_capacity(found.captures.len() + 2);
                for capture in &found.captures {
                    arguments.push(if let Some((start, end)) = capture {
                        Value::String(self.units_string(&input[*start..*end])?)
                    } else {
                        Value::Undefined
                    });
                }
                arguments.push(Value::Number(found.start() as f64));
                self.work(text.len())?;
                arguments.push(Value::String(text.into()));
                let value =
                    self.call_value(replacement.clone(), &arguments, Value::Undefined, host)?;
                let replace = self.replacement_string_value(value, host)?;
                // Callback execution and return-value conversion precede
                // concatenation. Its result is literal, not a $ template.
                let prefix = self.units_string(&input[cursor..found.start()])?;
                self.append_string(&mut output, &prefix)?;
                self.append_string(&mut output, &replace)?;
                cursor = found.end();
            }
            let suffix = self.units_string(&input[cursor..])?;
            self.append_string(&mut output, &suffix)?;
            return self.checked_value(Value::String(output));
        }

        // Preserve the original streaming path and $ substitutions for string
        // replacements; it does not need a callback match snapshot.
        let template = template.unwrap();
        let mut output = String::new();
        let mut cursor = 0;
        loop {
            self.step()?;
            let Some(found) = self.next_replacement_match(regex, literal.as_deref(), &input)?
            else {
                break;
            };
            let prefix = self.units_string(&input[cursor..found.start()])?;
            self.append_string(&mut output, &prefix)?;
            let replace = self.replacement_text(&template, &found, &input)?;
            self.append_string(&mut output, &replace)?;
            cursor = found.end();
            if !global {
                break;
            }
            if found.start() == found.end() {
                self.set_property_throw(
                    regex.unwrap(),
                    "lastIndex".into(),
                    Value::Number((found.end() + 1) as f64),
                )?;
            }
        }
        let suffix = self.units_string(&input[cursor..])?;
        self.append_string(&mut output, &suffix)?;
        self.checked_value(Value::String(output))
    }

    fn replacement_string_value(
        &mut self,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<String, Fault> {
        let value = self.primitive_with_hint(value, true, host)?;
        self.work(value.bytes())?;
        value.script_text()
    }

    fn next_replacement_match(
        &mut self,
        regex: Option<usize>,
        literal: Option<&[u16]>,
        input: &[u16],
    ) -> Result<Option<regexp::Match>, Fault> {
        if let Some(id) = regex {
            return self.regexp_run(id, input);
        }
        let needle = literal.unwrap();
        let mut position = None;
        if needle.is_empty() {
            position = Some(0);
        } else if needle.len() <= input.len() {
            for (at, part) in input.windows(needle.len()).enumerate() {
                self.work(needle.len().saturating_mul(2))?;
                if part == needle {
                    position = Some(at);
                    break;
                }
            }
        }
        Ok(position.map(|start| regexp::Match {
            captures: vec![Some((start, start + needle.len()))],
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            Err("Unexpected native property read.".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("Unexpected native property write.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("Unexpected native method call.".into())
        }
    }

    fn succeeds(source: &str) {
        assert_eq!(
            Runtime::new()
                .execute(source, &mut NoHost)
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn callable_replace_literal_arguments_utf16_offset_and_receiver() {
        succeeds(
            r#"
            let calls=0, original='😀cat-cat';
            let result=original.replace('cat',function(match,offset,text){
                'use strict';calls++;
                if(this!==undefined||arguments.length!==3||match!=='cat'||offset!==2||text!==original)throw 'literal arguments';
                return '$&';
            });
            if(result!=='😀$&-cat'||calls!==1)throw 'literal replacement';
            if('abc'.replace('missing',function(){throw 'unexpected';})!=='abc')throw 'no match';
            if('abc'.replace('',function(match,index,text){return match+index+text;})!=='0abcabc')throw 'empty literal';
            let receiver=this;
            if('a'.replace('a',function(){return this===receiver?'yes':'no';})!=='yes')throw 'sloppy receiver';
            if(String.prototype.replace.call(123,2,function(match,index,text){return match+index+text;})!=='1211233')throw 'borrowed receiver';
            'ok';
            "#,
        );
    }

    #[test]
    fn callable_replace_native_captures_and_global_match_snapshot() {
        succeeds(
            r#"
            let regex=/(a)|(b)/g, seen='', count=0;
            regex.lastIndex=99;
            let result='ab'.replace(regex,function(match,a,b,index,text){
                'use strict';
                if(this!==undefined||arguments.length!==5||text!=='ab'||index!==count)throw 'capture arguments';
                if(count===0){if(match!=='a'||a!=='a'||b!==undefined||regex.lastIndex!==0)throw 'first capture';regex.lastIndex=77;Object.freeze(regex);}
                else if(match!=='b'||a!==undefined||b!=='b'||regex.lastIndex!==77)throw 'second capture';
                seen+=match;count++;return index;
            });
            if(result!=='01'||seen!=='ab'||count!==2||regex.lastIndex!==77)throw 'global snapshot';
            let untouched=/z/g;untouched.lastIndex=8;
            if('ab'.replace(untouched,function(){throw 'no match callback';})!=='ab'||untouched.lastIndex!==0)throw 'no global match';
            'ok';
            "#,
        );
    }

    #[test]
    fn callable_replace_empty_matches_and_sticky_last_index() {
        succeeds(
            r#"
            let positions='', empty=/(?:)/g;
            let result='ab'.replace(empty,function(match,index,text){positions+=index;return '-';});
            if(result!=='-a-b-'||positions!=='012'||empty.lastIndex!==0)throw 'empty progress';
            let sticky=/a/y;sticky.lastIndex=1;
            result='ba'.replace(sticky,function(match,index,text){if(index!==1||sticky.lastIndex!==2)throw 'sticky index';sticky.lastIndex=9;return 'x';});
            if(result!=='bx'||sticky.lastIndex!==9)throw 'sticky callback mutation';
            let plain=/a/;plain.lastIndex=6;
            if('ba'.replace(plain,function(){return 'x';})!=='bx'||plain.lastIndex!==6)throw 'nonstateful index';
            let locked=/a/g;Object.freeze(locked);let caught=false;
            try{'a'.replace(locked,function(){throw 'unexpected callback';});}catch(e){caught=e.name==='TypeError';}
            if(!caught)throw 'readonly lastIndex';
            'ok';
            "#,
        );
    }

    #[test]
    fn callable_replace_result_conversion_and_exception_order() {
        succeeds(
            r#"
            let log='', regex=/a/g;
            let result='aa'.replace(regex,function(match,index){log+='c'+index;return {toString:function(){log+='s'+index;return index;},valueOf:function(){throw 'wrong hint';}};});
            if(result!=='01'||log!=='c0s0c1s1')throw 'result conversion order';
            if('a'.replace('a',function(){return undefined;})!=='undefined'||'a'.replace('a',function(){return null;})!=='null')throw 'primitive result';
            let calls=0, caught=false, marker=Symbol('failure');
            try{'aa'.replace(regex,function(){calls++;throw marker;});}catch(e){caught=e===marker;}
            if(!caught||calls!==1||regex.lastIndex!==0)throw 'callback exception';
            calls=0;caught=false;
            try{'aa'.replace(regex,function(){calls++;return Symbol();});}catch(e){caught=e.name==='TypeError';}
            if(!caught||calls!==1||regex.lastIndex!==0)throw 'result Symbol conversion';
            calls=0;caught=false;
            try{'aa'.replace(regex,function(){calls++;return {toString:function(){throw marker;}};});}catch(e){caught=e===marker;}
            if(!caught||calls!==1||regex.lastIndex!==0)throw 'result exception';
            'ok';
            "#,
        );
    }

    #[test]
    fn replace_preserves_templates_and_orders_literal_conversions() {
        succeeds(
            r#"
            if('abc'.replace(/(b)/,'$$:$&:$`:$\':$1:$2')!=='a$:b:a:c:b:$2c')throw 'template substitutions';
            if('ab'.replace(/(a)|(b)/g,'<$1$2>')!=='<a><b>')throw 'unmatched template captures';
            let log='', search={toString:function(){log+='s';return 'a';}}, replacement={toString:function(){log+='r';return '$&!';}};
            if('a'.replace(search,replacement)!=='a!'||log!=='sr')throw 'literal conversion order';
            log='';let caught=false;
            try{'a'.replace(Symbol(),replacement);}catch(e){caught=e.name==='TypeError';}
            if(!caught||log!=='')throw 'search Symbol before replacement';
            if('undefined'.replace(undefined,function(match,index,text){return 'x';})!=='x')throw 'undefined search';
            'ok';
            "#,
        );
    }

    #[test]
    fn callable_replace_snapshot_and_callback_work_remain_bounded() {
        let mut runtime = Runtime::new();
        let source = format!(
            "var callbacks=0;var text='{}';text.replace(/a/g,function(){{callbacks++;return '';}});",
            "a".repeat(MAX_ARRAY_LENGTH + 1)
        );
        let error = runtime.execute(&source, &mut NoHost).unwrap_err();
        assert!(error.contains("Replacement match limit"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("callbacks;", &mut NoHost)
                .unwrap()
                .to_text(),
            "0"
        );

        let mut runtime = Runtime::new();
        let source = format!(
            "var callbacks=0;var text='{}';text.replace(/(a){}/g,function(){{callbacks++;return '';}});",
            "a".repeat(5000),
            "()".repeat(15)
        );
        let error = runtime.execute(&source, &mut NoHost).unwrap_err();
        assert!(error.contains("Replacement snapshot limit"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("callbacks;", &mut NoHost)
                .unwrap()
                .to_text(),
            "0"
        );

        let mut runtime = Runtime::new();
        runtime
            .define_global(
                "wide",
                Value::String("x".repeat(super::super::MAX_STRING_BYTES)),
            )
            .unwrap();
        let error = runtime
            .execute("var callbacks=0;try{'ab'.replace('a',function(){callbacks++;return wide;});}catch(e){callbacks+=10;}", &mut NoHost)
            .unwrap_err();
        assert!(error.contains("String limit"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("callbacks;", &mut NoHost)
                .unwrap()
                .to_text(),
            "1"
        );

        let mut runtime = Runtime::new();
        let error = runtime
            .execute("'a'.replace('a',function(){while(true);});", &mut NoHost)
            .unwrap_err();
        assert!(
            error.contains("Execution step limit") || error.contains("Execution time limit"),
            "{error}"
        );
        let mut runtime = Runtime::new();
        let error = runtime
            .execute(
                "function again(){return 'a'.replace(/a/g,again);}again();",
                &mut NoHost,
            )
            .unwrap_err();
        assert!(error.contains("call limit"), "{error}");
    }
}
