//! Bounded String.prototype.replace/replaceAll for literal and native RegExp patterns.
//! Callable replacements receive the native captures, UTF-16 position and
//! original string, followed by a null-prototype named-groups object when needed.
//! Global matching finishes before the first callback. Custom Symbol.replace/
//! RegExp execution protocols remain outside the existing subset.

use super::{regexp, Fault, Host, Runtime, Value};
#[cfg(test)]
use super::MAX_ARRAY_LENGTH;

impl Runtime {
    pub(super) fn string_replace_all(
        &mut self,
        receiver: Value,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        Self::require_property_receiver(&receiver)?;
        let pattern = args.first().unwrap_or(&Value::Undefined);
        let regex = match pattern {
            Value::Object(id) if self.object(*id)?.regexp.is_some() => Some(*id),
            _ => None,
        };
        // RegExp validation precedes observable receiver/replacement conversion.
        // Custom Symbol.replace protocols remain outside the RegExp subset.
        if let Some(id) = regex {
            let flags = self.get_property_with_host(id, "flags", host)?;
            Self::require_property_receiver(&flags)?;
            if !self.replacement_string_value(flags, host)?.contains('g') {
                return Err(Fault::exception(
                    "TypeError",
                    "replaceAll requires a global RegExp.",
                ));
            }
        }
        let text = self.replacement_string_value(receiver, host)?;
        self.work(text.len().saturating_mul(3))?;
        self.string_replace_mode(
            &text,
            pattern,
            regex,
            args.get(1).unwrap_or(&Value::Undefined),
            true,
            host,
        )
    }

    pub(super) fn string_replace(
        &mut self,
        text: &str,
        pattern: &Value,
        regex: Option<usize>,
        replacement: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.string_replace_mode(text, pattern, regex, replacement, false, host)
    }

    fn string_replace_mode(
        &mut self,
        text: &str,
        pattern: &Value,
        regex: Option<usize>,
        replacement: &Value,
        all: bool,
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
            all
        };
        if let Some(id) = regex.filter(|_| global) {
            self.set_property_throw(id, "lastIndex".into(), Value::Number(0.0))?;
        }

        if callable {
            let mut matches = Vec::new();
            let mut snapshot_bytes = 0usize;
            let mut search_from = 0usize;
            loop {
                self.step()?;
                let Some(found) =
                    self.next_replacement_match(regex, literal.as_deref(), &input, search_from)?
                else {
                    break;
                };
                if matches.len() >= self.limits.heap.array_length {
                    return Err(Fault::new(format!("Replacement match limit reached ({}).", self.limits.heap.array_length)));
                }
                let bytes = found.retained_bytes();
                snapshot_bytes = snapshot_bytes.saturating_add(bytes);
                if snapshot_bytes > self.limits.heap.retained_values {
                    return Err(Fault::new(format!("Replacement snapshot limit reached ({} bytes).", self.limits.heap.retained_values)));
                }
                // Range snapshots retain no duplicate source strings and also
                // spend the shared job allowance, including across recursion.
                self.work(bytes)?;
                let empty_end = (found.start() == found.end()).then_some(found.end());
                search_from = found.end().max(found.start() + 1);
                matches.push(found);
                if !global {
                    break;
                }
                if let (Some(end), Some(id)) = (empty_end, regex) {
                    self.set_property_throw(
                        id,
                        "lastIndex".into(),
                        Value::Number((end + 1) as f64),
                    )?;
                }
            }
            let mut output = String::new();
            let mut cursor = 0;
            for found in matches {
                self.step()?;
                let mut arguments = Vec::with_capacity(found.captures.len() + 3);
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
                if !found.named_captures.is_empty() {
                    arguments.push(self.regexp_named_groups(&found, &input)?);
                }
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
        let mut search_from = 0;
        loop {
            self.step()?;
            let Some(found) =
                self.next_replacement_match(regex, literal.as_deref(), &input, search_from)?
            else {
                break;
            };
            let prefix = self.units_string(&input[cursor..found.start()])?;
            self.append_string(&mut output, &prefix)?;
            let replace = self.replacement_text(&template, &found, &input)?;
            self.append_string(&mut output, &replace)?;
            cursor = found.end();
            search_from = found.end().max(found.start() + 1);
            if !global {
                break;
            }
            if found.start() == found.end() && regex.is_some() {
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
        from: usize,
    ) -> Result<Option<regexp::Match>, Fault> {
        if let Some(id) = regex {
            return self.regexp_run(id, input);
        }
        let needle = literal.unwrap();
        if from > input.len() {
            return Ok(None);
        }
        let mut position = None;
        if needle.is_empty() {
            position = Some(from);
        } else if needle.len() <= input.len() - from {
            for (at, part) in input[from..].windows(needle.len()).enumerate() {
                self.work(needle.len().saturating_mul(2))?;
                if part == needle {
                    position = Some(from + at);
                    break;
                }
            }
        }
        Ok(position.map(|start| regexp::Match {
            captures: vec![Some((start, start + needle.len()))],
            named_captures: Vec::new(),
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
            Runtime::for_test()
                .execute(source, &mut NoHost)
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn replace_all_literal_callbacks_empty_matches_and_nonoverlap() {
        succeeds(
            r#"
var calls=[];
var result='😀aba'.replaceAll('a',function(match,index,text){'use strict';
if(this!==undefined||arguments.length!==3||match!=='a'||text!=='😀aba')throw 'arguments';calls.push(index);return '$&';});
if(result!=='😀$&b$&'||calls.join()!=='2,4')throw 'positions';
if('aaaaa'.replaceAll('aa','x')!=='xxa')throw 'nonoverlap';
if('ab'.replaceAll('',function(match,index){return index;})!=='0a1b2'||''.replaceAll('',()=>'-')!=='-')throw 'empty';
if('abc'.replaceAll('missing',function(){throw 'unexpected';})!=='abc')throw 'missing';
if(String.prototype.replaceAll.length!==2||String.prototype.replaceAll.name!=='replaceAll')throw 'metadata';
'ok';
"#,
        );
    }

    #[test]
    fn replace_all_templates_generic_receiver_and_conversion_order() {
        succeeds(
            r#"
var order='',receiver={toString(){order+='o';return 'aba';}},search={toString(){order+='s';return 'a';}},replacement={toString(){order+='r';return '<$&>';}};
if(String.prototype.replaceAll.call(receiver,search,replacement)!=='<a>b<a>'||order!=='osr')throw 'conversion';
if(String.prototype.replaceAll.call(121,1,()=>9)!=='929')throw 'generic receiver';
if('aba'.replaceAll('a','[$$][$&][$`][$\']')!=='[$][a][][ba]b[$][a][ab][]')throw 'template';
if('undefined undefined'.replaceAll(undefined,'x')!=='x x')throw 'undefined';
var marker={},calls=0;try{'aaa'.replaceAll('a',function(){calls++;throw marker;});}catch(e){if(e!==marker)throw e;}
if(calls!==1)throw 'callback throw';
var caught=false;try{'a'.replaceAll('a',()=>Symbol());}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'symbol return';
'ok';
"#,
        );
    }

    #[test]
    fn replace_all_native_regexp_snapshots_and_early_global_validation() {
        succeeds(
            r#"
var r=/(a)|(b)/g,seen=[];r.lastIndex=19;
var result='ab'.replaceAll(r,function(match,a,b,index,input){'use strict';
if(this!==undefined||arguments.length!==5||input!=='ab')throw 'captures';seen.push(index+':'+r.lastIndex);r.lastIndex=7;return index;});
if(result!=='01'||seen.join('|')!=='0:0|1:7'||r.lastIndex!==7)throw 'snapshot';
var order='',receiver={toString(){order+='receiver';return 'aaa';}},replacement={toString(){order+='replacement';return 'x';}},caught=false;
try{String.prototype.replaceAll.call(receiver,/a/,replacement);}catch(e){caught=e.name==='TypeError';}
if(!caught||order!=='')throw 'nonglobal order';
'ok';
"#,
        );
        // Native RegExp configuration still uses the pre-existing virtual
        // properties. Keep the unsupported descriptor boundary explicit;
        // replaceAll does not claim custom RegExp accessors or protocols.
        let error = Runtime::for_test().execute(r#"
var order='',receiver={toString(){order+='receiver';return 'aaa';}},replacement={toString(){order+='replacement';return 'x';}};
var marker={},reg=/a/g;Object.defineProperty(reg,'flags',{get:function(){throw marker;}});
try{String.prototype.replaceAll.call(receiver,reg,replacement);}catch(e){if(e!==marker)throw e;}
if(order!=='')throw 'flags read before receiver';
"#, &mut NoHost).unwrap_err();
        assert!(
            error.contains("RegExp accessor descriptor changes are not implemented"),
            "{error}"
        );
    }

    #[test]
    fn replace_all_match_budget_is_fatal_before_callbacks() {
        let mut runtime = Runtime::for_test();
        runtime.execute("var caught=false,finished=false,called=false,text='a';for(var i=0;i<14;i++)text+=text;",&mut NoHost).unwrap();
        let error=runtime.execute("try{text.replaceAll('a',function(){called=true;return '';});}catch(e){caught=true;}finally{finished=true;}",&mut NoHost).unwrap_err();
        assert!(error.contains("limit"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("!called&&!caught&&!finished;", &mut NoHost)
                .unwrap()
                .to_text(),
            "true"
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
        let mut runtime = Runtime::for_test();
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

        let mut runtime = Runtime::for_test();
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

        let mut runtime = Runtime::for_test();
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

        let mut runtime = Runtime::for_test();
        let error = runtime
            .execute("'a'.replace('a',function(){while(true);});", &mut NoHost)
            .unwrap_err();
        assert!(
            error.contains("Execution step limit") || error.contains("Execution time limit"),
            "{error}"
        );
        let mut runtime = Runtime::for_test();
        let error = runtime
            .execute(
                "function again(){return 'a'.replace(/a/g,again);}again();",
                &mut NoHost,
            )
            .unwrap_err();
        assert!(error.contains("call limit"), "{error}");
    }
}
