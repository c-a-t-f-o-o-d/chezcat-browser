//! Bounded String.prototype.replace/replaceAll for literal and native RegExp patterns.
//! Callable replacements receive the native captures, UTF-16 position and
//! original string, followed by a null-prototype named-groups object when needed.
//! Global matching finishes before the first callback. Custom Symbol.replace/
//! RegExp execution protocols remain outside the existing subset.

#[cfg(test)]
use super::MAX_ARRAY_LENGTH;
use super::{regexp, Fault, Host, Runtime, Value};

fn scalar_units(s: &str) -> Result<Vec<u16>, Fault> {
    let mut units = Vec::new();
    units
        .try_reserve_exact(s.len())
        .map_err(|_| Fault::new("Cannot reserve replacement input."))?;
    units.extend(s.encode_utf16());
    Ok(units)
}

impl Runtime {
    pub(super) fn string_replace_value(
        &mut self,
        receiver: Value,
        args: &[Value],
        all: bool,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        Self::require_property_receiver(&receiver)?;
        let pattern = args.first().unwrap_or(&Value::Undefined);
        let regex = match pattern {
            Value::Object(id) if self.object(*id)?.regexp.is_some() => Some(*id),
            _ => None,
        };
        if let Some(id) = regex.filter(|_| all) {
            let flags = self.get_property_with_host(id, "flags", host)?;
            Self::require_property_receiver(&flags)?;
            let value = self.replacement_value(flags, host)?;
            let global = match value {
                Value::String(s) => s.contains('g'),
                Value::StringUnits(u) => u.contains(&103),
                _ => unreachable!(),
            };
            if !global {
                return Err(Fault::exception(
                    "TypeError",
                    "replaceAll requires a global RegExp.",
                ));
            }
        }
        let text = self.replacement_value(receiver, host)?;
        self.work_as(
            text.bytes().saturating_mul(3),
            super::WorkKind::StringMethodInput,
        )?;
        let replacement = args.get(1).unwrap_or(&Value::Undefined);
        let callable = matches!(replacement, Value::Function(_));
        let literal = if regex.is_none() {
            let value = self.copy_value(pattern)?;
            Some(self.replacement_units(value, host)?)
        } else {
            None
        };
        let template = if callable {
            None
        } else {
            let value = self.copy_value(replacement)?;
            Some(self.replacement_units(value, host)?)
        };
        let encoded;
        let input: &[u16] = match &text {
            Value::String(s) => {
                encoded = scalar_units(s)?;
                &encoded
            }
            Value::StringUnits(u) => u,
            _ => unreachable!(),
        };
        let (global, unicode) = if let Some(id) = regex {
            let flags = self.object(id)?.regexp.as_ref().unwrap().flags;
            (flags.global, flags.unicode)
        } else {
            (all, false)
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
                    self.next_replacement_match(regex, literal.as_deref(), input, search_from)?
                else {
                    break;
                };
                if matches.len() >= self.limits.heap.array_length {
                    return Err(Fault::new(format!(
                        "Replacement match limit reached ({}).",
                        self.limits.heap.array_length
                    )));
                }
                let bytes = found.retained_bytes();
                snapshot_bytes = snapshot_bytes.saturating_add(bytes);
                if snapshot_bytes > self.limits.heap.retained_values {
                    return Err(Fault::new(format!(
                        "Replacement snapshot limit reached ({} bytes).",
                        self.limits.heap.retained_values
                    )));
                }
                // Range snapshots retain no duplicate source strings and also
                // spend the shared job allowance, including across recursion.
                self.work(bytes)?;
                let empty_end = (found.start() == found.end()).then_some(found.end());
                search_from = empty_end.map_or(found.end(), |index| {
                    regexp::advance_string_index(input, index, unicode)
                });
                matches
                    .try_reserve(1)
                    .map_err(|_| Fault::new("Cannot reserve replacement matches."))?;
                matches.push(found);
                if !global {
                    break;
                }
                if let (Some(_), Some(id)) = (empty_end, regex) {
                    self.set_property_throw(
                        id,
                        "lastIndex".into(),
                        Value::Number(search_from as f64),
                    )?;
                }
            }
            let mut output = Vec::new();
            let mut cursor = 0;
            if matches.is_empty() {
                return Ok(text);
            }
            for found in matches {
                self.step()?;
                let mut arguments = Vec::new();
                arguments
                    .try_reserve_exact(
                        found
                            .captures
                            .len()
                            .checked_add(3)
                            .ok_or_else(|| Fault::new("Replacement argument count overflow."))?,
                    )
                    .map_err(|_| Fault::new("Cannot reserve replacement arguments."))?;
                for capture in &found.captures {
                    arguments.push(if let Some((start, end)) = capture {
                        self.string_from_units(&input[*start..*end])?
                    } else {
                        Value::Undefined
                    });
                }
                arguments.push(Value::Number(found.start() as f64));
                arguments.push(self.copy_value(&text)?);
                if !found.named_captures.is_empty() {
                    arguments.push(self.regexp_named_groups(&found, input)?);
                }
                let value =
                    self.call_value(replacement.clone(), &arguments, Value::Undefined, host)?;
                let replace = self.replacement_value(value, host)?;
                // Callback execution and return-value conversion precede
                // concatenation. Its result is literal, not a $ template.
                self.append_replace_units(&mut output, &input[cursor..found.start()])?;
                self.append_replace_value(&mut output, &replace)?;
                cursor = found.end();
            }
            self.append_replace_units(&mut output, &input[cursor..])?;
            return self.finish_replacement(output);
        }

        // Preserve the original streaming path and $ substitutions for string
        // replacements; it does not need a callback match snapshot.
        let template = template.unwrap();
        let mut matched = false;
        let mut output = Vec::new();
        let mut cursor = 0;
        let mut search_from = 0;
        loop {
            self.step()?;
            let Some(found) =
                self.next_replacement_match(regex, literal.as_deref(), input, search_from)?
            else {
                break;
            };
            matched = true;
            self.append_replace_units(&mut output, &input[cursor..found.start()])?;
            self.append_substitution(&mut output, &template, &found, input)?;
            cursor = found.end();
            let empty = found.start() == found.end();
            search_from = if empty {
                regexp::advance_string_index(input, found.end(), unicode)
            } else {
                found.end()
            };
            if !global {
                break;
            }
            if empty && regex.is_some() {
                self.set_property_throw(
                    regex.unwrap(),
                    "lastIndex".into(),
                    Value::Number(search_from as f64),
                )?;
            }
        }
        if !matched {
            return Ok(text);
        }
        self.append_replace_units(&mut output, &input[cursor..])?;
        self.finish_replacement(output)
    }

    fn replacement_value(&mut self, value: Value, host: &mut dyn Host) -> Result<Value, Fault> {
        let value = self.primitive_with_hint(value, true, host)?;
        self.work(value.bytes())?;
        match value {
            Value::String(_) | Value::StringUnits(_) => self.checked_value(value),
            _ => self.checked_value(Value::String(value.script_text()?)),
        }
    }

    fn replacement_units(&mut self, value: Value, host: &mut dyn Host) -> Result<Vec<u16>, Fault> {
        match self.replacement_value(value, host)? {
            Value::StringUnits(u) => Ok(u),
            Value::String(s) => {
                self.work_as(s.len().saturating_mul(3), super::WorkKind::ConversionOutput)?;
                scalar_units(&s)
            }
            _ => unreachable!(),
        }
    }

    fn prepare_replace_output(&mut self, out: &mut Vec<u16>, count: usize) -> Result<(), Fault> {
        let limit = self.limits.capacity.string_bytes;
        let needed = out
            .len()
            .checked_add(count)
            .filter(|&n| n <= limit)
            .ok_or_else(|| self.string_limit_fault())?;
        self.work_as(count.saturating_mul(2), super::WorkKind::ConversionOutput)?;
        if needed > out.capacity() {
            self.work_as(out.len().saturating_mul(2), super::WorkKind::ValueCopy)?;
            let capacity = needed.max(out.capacity().saturating_mul(2).max(8).min(limit));
            out.try_reserve_exact(capacity - out.len())
                .map_err(|_| Fault::new("Cannot reserve string replacement output."))?;
        }
        Ok(())
    }

    fn append_replace_units(&mut self, out: &mut Vec<u16>, units: &[u16]) -> Result<(), Fault> {
        self.prepare_replace_output(out, units.len())?;
        out.extend_from_slice(units);
        Ok(())
    }

    fn append_replace_value(&mut self, out: &mut Vec<u16>, value: &Value) -> Result<(), Fault> {
        match value {
            Value::StringUnits(u) => self.append_replace_units(out, u),
            Value::String(s) => {
                self.work(s.len())?;
                self.prepare_replace_output(out, s.encode_utf16().count())?;
                out.extend(s.encode_utf16());
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn finish_replacement(&mut self, out: Vec<u16>) -> Result<Value, Fault> {
        // Joining two portions may repair a surrogate pair. Apply the final
        // scalar/raw byte allowance after composing the actual code units.
        self.work_as(
            out.len().saturating_mul(2),
            super::WorkKind::ConversionOutput,
        )?;
        let value = super::string_units::canonical(out, self.limits.capacity.string_bytes)?;
        self.checked_value(value)
    }

    fn append_substitution(
        &mut self,
        out: &mut Vec<u16>,
        template: &[u16],
        found: &regexp::Match,
        input: &[u16],
    ) -> Result<(), Fault> {
        // Literal scanning and the search for named-capture closers visit
        // each unit at most twice, including a suffix with no closing '>'.
        self.work(template.len().saturating_mul(4))?;
        let mut start = 0;
        let mut at = 0;
        let mut no_closer = false;
        let ascii = |at: usize| template.get(at).and_then(|&u| u8::try_from(u).ok());
        while at + 1 < template.len() {
            if template[at] != u16::from(b'$') {
                at += 1;
                continue;
            }
            let next = ascii(at + 1);
            let mut end = at + 2;
            let range = match next {
                Some(b'$') => {
                    self.append_replace_units(out, &template[start..=at])?;
                    at = end;
                    start = end;
                    continue;
                }
                Some(b'&') => Some((found.start(), found.end())),
                Some(b'`') => Some((0, found.start())),
                Some(b'\'') => Some((found.end(), input.len())),
                Some(d @ b'0'..=b'9') => {
                    let first = usize::from(d - b'0');
                    let second = ascii(end)
                        .filter(u8::is_ascii_digit)
                        .map(|d| first * 10 + usize::from(d - b'0'));
                    let capture =
                        if let Some(n) = second.filter(|&n| n != 0 && n < found.captures.len()) {
                            end += 1;
                            n
                        } else if first != 0 && first < found.captures.len() {
                            first
                        } else {
                            at += 1;
                            continue;
                        };
                    found.captures[capture]
                }
                Some(b'<') if !found.named_captures.is_empty() && !no_closer => {
                    let Some(length) = template[end..]
                        .iter()
                        .position(|&unit| unit == u16::from(b'>'))
                    else {
                        no_closer = true;
                        at += 1;
                        continue;
                    };
                    let name = &template[end..end + length];
                    end += length + 1;
                    let mut matched = None;
                    for (key, capture) in &found.named_captures {
                        self.work(key.len().saturating_add(name.len().saturating_mul(2)))?;
                        if key.encode_utf16().eq(name.iter().copied()) {
                            matched = *capture;
                            break;
                        }
                    }
                    matched
                }
                _ => {
                    at += 1;
                    continue;
                }
            };
            self.append_replace_units(out, &template[start..at])?;
            if let Some((a, b)) = range {
                self.append_replace_units(out, &input[a..b])?;
            }
            at = end;
            start = end;
        }
        self.append_replace_units(out, &template[start..])
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
    fn utf16_replacement_literal_and_template_values_preserve_and_join_units() {
        succeeds(
            r#"
var hi='\ud800',lo='\udfff',raw=hi+'X'+lo;
if(raw.replace('X','')!=='\u{103ff}'||raw.replaceAll('missing','noop')!==raw)throw 'join or no match';
if(raw.replaceAll(hi,'$&$&')!==hi+hi+'X'+lo||'abc'.replace('b',lo)!=='a'+lo+'c')throw 'raw search or substitution';
var expr=new RegExp('(?<unit>'+hi+')(z)?');
if(raw.replace(expr,'$<unit>:$2:$12:$&:$$')!==hi+'::'+hi+'2:'+hi+':$X'+lo)throw 'capture substitution';
if(raw.replace('X',String.fromCharCode(36,96)+'-'+String.fromCharCode(36,39))!==hi+hi+'-'+lo+lo)throw 'prefix and suffix';
if(String.prototype.replace.call({toString:function(){return raw;}},hi,lo)!==lo+'X'+lo)throw 'generic receiver';
if(('\u{1f431}').replace(/./,'x')!=='x\udc31')throw 'non-unicode scalar split';
'ok';
"#,
        );
    }

    #[test]
    fn utf16_replacement_callbacks_observe_raw_matches_groups_positions_and_reentry() {
        succeeds(
            r#"
var hi='\ud800',lo='\udfff',raw=hi+'b'+lo,re=new RegExp('(?<unit>['+hi+lo+'])','g'),trace=[];
var result=raw.replaceAll(re,function(m,u,pos,original,groups){
 if(m!==u||groups.unit!==u||Object.getPrototypeOf(groups)!==null||original!==raw)throw 'native callback arguments';
 trace.push(pos);re.lastIndex=8;return {toString:function(){return u===hi?lo:hi;}};
});
if(result!==lo+'b'+hi||trace.join(',')!=='0,2'||re.lastIndex!==8)throw 'snapshots and raw callback result';
var marker={},order='';
var receiver={toString:function(){order+='o';return raw;}},search={toString:function(){order+='s';return hi;}};
var replace={toString:function(){order+='r';return lo;}};
if(String.prototype.replaceAll.call(receiver,search,replace)!==lo+'b'+lo||order!=='osr')throw 'ordered conversion';
try{raw.replaceAll(re,function(){throw marker;});throw 'missing abrupt';}catch(e){if(e!==marker)throw e;}
'ok';
"#,
        );
    }

    #[test]
    fn utf16_replacement_final_storage_cap_and_recovery() {
        let mut runtime = Runtime::for_test();
        let mut host = NoHost;
        let high = Value::StringUnits(vec![0xd800]);
        let low = Value::StringUnits(vec![0xdfff]);
        let args = [Value::String(String::new()), high.clone()];
        runtime.limits.capacity.string_bytes = 3;
        let error = runtime
            .string_replace_value(low.clone(), &args, false, &mut host)
            .unwrap_err();
        assert!(error.exception.is_none() && error.message.contains("String limit"));
        runtime.begin_job();
        runtime.limits.capacity.string_bytes = 4;
        assert!(
            matches!(runtime.string_replace_value(low,&args,false,&mut host),Ok(Value::String(s)) if s=="\u{103ff}")
        );
        runtime.limits.capacity.string_bytes = 2;
        let args = [Value::String(String::new()), Value::String("x".into())];
        assert!(runtime
            .string_replace_value(high.clone(), &args, false, &mut host)
            .is_err());
        runtime.begin_job();
        // The absent replacement converts to "undefined" before matching.
        // Its nine-byte value itself exceeds this two-byte profile.
        let error=runtime.string_replace_value(high.clone(), &[Value::String("z".into())], false, &mut host).unwrap_err();
        assert!(error.exception.is_none() && error.message.contains("String limit"));
        runtime.begin_job();
        let small=[Value::String("z".into()),Value::String(String::new())];
        assert!(matches!(runtime.string_replace_value(high,&small,false,&mut host),Ok(Value::StringUnits(u)) if u==[0xd800]));
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
