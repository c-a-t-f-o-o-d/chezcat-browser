//! Original bounded template literals.
//! https://tc39.es/ecma262/#sec-template-literal-lexical-components
//! https://tc39.es/ecma262/#sec-template-literals-runtime-semantics-evaluation
//! https://tc39.es/ecma262/#sec-gettemplateobject

#[cfg(test)]
use super::MAX_STRING_BYTES;
use super::{
    primitive_conversion::Hint, Argument, ExprId, ExprKind, Fault, Host, Parser, Program, Runtime,
    TokenKind, Value, MAX_EXPRESSION_FRAMES,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

#[derive(Clone, Copy, Debug)]
pub(super) enum Part {
    NoSubstitution,
    Head,
    Middle,
    Tail,
}

struct TaggedChunk {
    cooked: String,
    raw: Range<usize>,
}

pub(super) struct TaggedTemplateSite {
    chunks: Vec<TaggedChunk>,
    // Each parsed site has one array per realm. Both arrays contain only
    // strings and the raw link, and their runtime object IDs are append-only.
    objects: RefCell<HashMap<Option<usize>, usize>>,
}

// Cooked text is represented in the runtime's existing UTF-8 string model.
// Pair adjacent escaped UTF-16 units within a chunk; lone units remain an
// explicit capability boundary, including at an interpolation boundary.
fn push_cooked(text: &mut String, high: &mut Option<u32>, value: u32, string_limit: usize) -> Result<(), Fault> {
    let scalar = if let Some(first) = high.take() {
        if !(0xdc00..=0xdfff).contains(&value) {
            return Err(Fault::new(
                "Lone UTF-16 surrogate escapes are not supported.",
            ));
        }
        0x10000 + ((first - 0xd800) << 10) + value - 0xdc00
    } else if (0xd800..=0xdbff).contains(&value) {
        *high = Some(value);
        return Ok(());
    } else {
        value
    };
    let ch = char::from_u32(scalar)
        .ok_or_else(|| Fault::new("Lone UTF-16 surrogate escapes are not supported."))?;
    if text.len().saturating_add(ch.len_utf8()) > string_limit {
        return Err(Fault::new(format!("String limit reached ({} KiB).", string_limit / 1024)));
    }
    text.push(ch);
    Ok(())
}

fn braced_unicode(source: &str, at: &mut usize) -> Result<u32, Fault> {
    *at += 1; // Opening brace after the already-consumed backslash-u.
    let start = *at;
    let mut value = 0u32;
    while let Some(digit) = source
        .as_bytes()
        .get(*at)
        .and_then(|b| (*b as char).to_digit(16))
    {
        value = value.saturating_mul(16).saturating_add(digit);
        *at += 1;
        if value > 0x10ffff {
            return Err(
                Fault::exception("SyntaxError", "Unicode code point is out of range.")
                    .at(source, start),
            );
        }
    }
    if *at == start || source.as_bytes().get(*at) != Some(&b'}') {
        return Err(
            Fault::exception("SyntaxError", "Invalid braced Unicode escape.").at(source, start),
        );
    }
    *at += 1;
    Ok(value)
}

/// Scan exactly one chunk, after the opening backtick or closing substitution
/// brace. The boolean says whether a backtick (rather than ${) ended it.
pub(super) fn chunk(source: &str, at: &mut usize, start: usize, string_limit: usize) -> Result<(String, bool), Fault> {
    let bytes = source.as_bytes();
    let mut text = String::new();
    let mut high = None;
    while let Some(ch) = source[*at..].chars().next() {
        if ch == '`' || ch == '$' && bytes.get(*at + 1) == Some(&b'{') {
            if high.is_some() {
                return Err(
                    Fault::new("Lone UTF-16 surrogate escapes are not supported.").at(source, *at),
                );
            }
            *at += if ch == '`' { 1 } else { 2 };
            return Ok((text, ch == '`'));
        }
        let position = *at;
        *at += ch.len_utf8();
        let value = if ch == '\\' {
            let escaped = source[*at..].chars().next().ok_or_else(|| {
                Fault::exception("SyntaxError", "Unterminated template escape.").at(source, *at)
            })?;
            *at += escaped.len_utf8();
            match escaped {
                'n' => '\n' as u32,
                'r' => '\r' as u32,
                't' => '\t' as u32,
                'b' => 0x08,
                'f' => 0x0c,
                'v' => 0x0b,
                '\n' | '\u{2028}' | '\u{2029}' => continue,
                '\r' => {
                    if bytes.get(*at) == Some(&b'\n') {
                        *at += 1;
                    }
                    continue;
                }
                '0' if !bytes.get(*at).is_some_and(u8::is_ascii_digit) => 0,
                '0'..='9' => {
                    return Err(Fault::exception(
                        "SyntaxError",
                        "Numeric escapes are invalid in template literals.",
                    )
                    .at(source, position))
                }
                'x' => super::hex_escape(source, at, 2)?,
                'u' if bytes.get(*at) == Some(&b'{') => braced_unicode(source, at)?,
                'u' => super::hex_escape(source, at, 4)?,
                other => other as u32,
            }
        } else if ch == '\r' {
            if bytes.get(*at) == Some(&b'\n') {
                *at += 1;
            }
            '\n' as u32
        } else {
            ch as u32
        };
        push_cooked(&mut text, &mut high, value, string_limit).map_err(|error| error.at(source, position))?;
    }
    Err(Fault::exception("SyntaxError", "Unterminated template literal.").at(source, start))
}

impl Parser {
    pub(super) fn template_literal(
        &mut self,
        head: String,
        substitutions: bool,
    ) -> Result<ExprKind, Fault> {
        self.current += 1;
        let mut spans = Vec::new();
        if substitutions {
            loop {
                if spans.len() >= MAX_EXPRESSION_FRAMES {
                    return Err(self.unsupported("Template substitution limit reached (4096)."));
                }
                let expression = self.expression_with_in()?;
                let TokenKind::Template(text, part @ (Part::Middle | Part::Tail)) =
                    &self.token().kind
                else {
                    return Err(self.error("Expected the closing template substitution brace."));
                };
                let tail = matches!(part, Part::Tail);
                spans.push((expression, text.clone()));
                self.current += 1;
                if tail {
                    break;
                }
            }
        }
        Ok(ExprKind::TemplateLiteral(head, spans))
    }

    pub(super) fn tagged_template_arguments(&mut self) -> Result<Vec<Argument>, Fault> {
        let start = self.at();
        // Reserve the leading argument's budget before consuming substitutions.
        let mut arguments = Vec::new();
        let site = self.expr(ExprKind::Literal(Value::Undefined), start)?;
        self.append_argument(&mut arguments, Argument { expression: site, spread: false })?;
        let mut chunks = Vec::new();
        loop {
            let token = self.token();
            let TokenKind::Template(cooked, part) = &token.kind else {
                return Err(self.error("Expected the closing template substitution brace."));
            };
            let tail = matches!(part, Part::NoSubstitution | Part::Tail);
            // A token begins at the backtick or substitution's closing brace,
            // and includes the trailing backtick or opening ${ delimiter.
            let raw = token.at() + 1..token.end() - if tail { 1 } else { 2 };
            chunks.try_reserve(1).map_err(|_| Fault::new("Template chunk allocation failed."))?;
            chunks.push(TaggedChunk { cooked: cooked.clone(), raw });
            self.current += 1;
            if tail {
                break;
            }
            if arguments.len() > MAX_EXPRESSION_FRAMES {
                return Err(self.unsupported("Template substitution limit reached (4096)."));
            }
            let expression = self.expression_with_in()?;
            self.append_argument(&mut arguments, Argument { expression, spread: false })?;
        }
        self.program.exprs[site].kind = ExprKind::TemplateObject(Rc::new(TaggedTemplateSite {
            chunks, objects: RefCell::default(),
        }));
        Ok(arguments)
    }
}

impl Runtime {
    pub(super) fn eval_template_object(
        &mut self, program: &Program, site: &TaggedTemplateSite,
    ) -> Result<Value, Fault> {
        let realm = self.active_realm;
        let mut objects = site.objects.borrow_mut();
        if let Some(&id) = objects.get(&realm) {
            return Ok(Value::Object(id));
        }
        objects.try_reserve(1).map_err(|_| Fault::new("Template site allocation failed."))?;
        let cooked = self.new_object(Some(site.chunks.len()))?;
        let raw = self.new_object(Some(site.chunks.len()))?;
        for (index, chunk) in site.chunks.iter().enumerate() {
            let key = index.to_string();
            let mut text = String::new();
            self.append_string(&mut text, &chunk.cooked)?;
            self.set_property(cooked, key.clone(), Value::String(text))?;
            let source = &program.source[chunk.raw.clone()];
            self.work(source.len())?;
            let mut text = String::new();
            let mut after_cr = false;
            for part in source.split_inclusive('\r') {
                let part = if after_cr { part.strip_prefix('\n').unwrap_or(part) } else { part };
                after_cr = part.ends_with('\r');
                if let Some(line) = part.strip_suffix('\r') {
                    self.append_string(&mut text, line)?;
                    self.append_string(&mut text, "\n")?;
                } else {
                    self.append_string(&mut text, part)?;
                }
            }
            self.set_property(raw, key, Value::String(text))?;
        }
        self.object_integrity("freeze", &Value::Object(raw))?;
        self.store_property(cooked, "raw".into(), Value::Object(raw), Some((false, false, false)))?;
        self.object_integrity("freeze", &Value::Object(cooked))?;
        objects.insert(realm, cooked);
        Ok(Value::Object(cooked))
    }

    pub(super) fn eval_template(
        &mut self,
        program: &Rc<Program>,
        head: &str,
        spans: &[(ExprId, String)],
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let mut text = String::new();
        self.append_string(&mut text, head)?;
        for (expression, literal) in spans {
            let value = self.eval(program, *expression, env, this, host)?;
            let primitive = self.primitive_with_preference(value, Hint::String, host)?;
            self.append_string(&mut text, &primitive.script_text()?)?;
            self.append_string(&mut text, literal)?;
        }
        Ok(Value::String(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct NoHost {
        reads: usize,
    }
    impl Host for NoHost {
        fn allow_eval(&mut self) -> Result<bool, String> {
            Ok(true)
        }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            self.reads += 1;
            Err("No native template reads.".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("No native template writes.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("No native template calls.".into())
        }
    }
    fn succeeds(source: &str) {
        let mut runtime = Runtime::for_test();
        let window = runtime.new_object(None).unwrap();
        runtime
            .define_global("window", Value::Object(window))
            .unwrap();
        assert_eq!(
            runtime
                .execute(source, &mut NoHost::default())
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn template_values_and_substitutions_use_string_hint_in_order() {
        succeeds(
            r#"
var log='',item={};Object.defineProperty(item,Symbol.toPrimitive,{get:function(){log+='g';return function(h){log+=h;if(this!==item)throw 'receiver';return 'value';};}});
function next(){log+='n';return 5;}
if(`head${item}mid${next()}tail`!=='headvaluemid5tail'||log!=='gstringn')throw 'conversion order';
var fallback={toString(){log+='s';return 'text';},valueOf(){throw 'number hint';}};
if(`${fallback}`!=='text'||log!=='gstringns')throw 'ordinary conversion';
if(`${undefined}|${null}|${true}|${-0}|${3.5}`!=='undefined|null|true|0|3.5')throw 'values';
var n=0;if(`x${n++,n+4}y${n++}z`!=='x5y1z'||n!==2)throw 'commas';
if(``!==''||`plain`!=='plain')throw 'no substitutions';'ok';
"#,
        );
    }

    #[test]
    fn template_abrupt_conversions_preserve_errors_and_stop_later_expressions() {
        succeeds(
            r#"
var marker={},after=0,caught=0,bad={toString(){throw marker;}};
try{`${bad}${after++}`;}catch(e){if(e===marker)caught++;}
try{`${Symbol('x')}${after++}`;}catch(e){if(e.name==='TypeError')caught++;}
var object={};object[Symbol.toPrimitive]=function(){return {};};
try{`${object}`;}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==3||after!==0)throw 'abrupt conversion';'ok';
"#,
        );
    }

    #[test]
    fn template_lexical_goals_preserve_nested_functions_regexps_and_division() {
        succeeds(
            r#"
var suffix='!';function make(x){return function(y){return `outer:${`${x+y}:${({n:3}).n}`}${suffix}`;};}
if(make(2)(4)!=='outer:6:3!')throw 'nested closure';
if(`${function(){return {text:'}'};}().text}`!=='}'||`${(()=>({x:8}))().x}`!=='8')throw 'function braces';
if(`${/[}]/.test('}')}|${/[{}]/.test('{')}|${/\$\{/.test('${')}`!=='true|true|true')throw 'regexp braces';
if(`${({n:8}).n / 2}::${(function(){return 9;})()/3}`!=='4::3')throw 'division';
if(`${1/* } ` ${ */+2}`!=='3'||`${// } ` ${
4}`!=='4')throw 'comment braces';
if(`${`6` / 2}`!=='3'||`12` / 3!==4)throw 'template division';'ok';
"#,
        );
    }

    #[test]
    fn template_cooking_preserves_escapes_unicode_and_line_terminators() {
        succeeds(
            r#"
if(`a\n\r\t\b\f\v\0z`!=='a\n\r\t\b\f\v\0z')throw 'escapes';
if(`\x41\u0042\u{43}\u{1F600}`!=='ABC😀')throw 'code points';
if(`\uD83D\uDE00`!=='😀'||`\u{D83D}\u{DE00}`!=='😀')throw 'surrogate pairs';
if(`back\`tick \\ \${literal} \q`!=='back`tick \\ ${literal} q')throw 'template escapes';
if(`a\
b`!=='ab'||`\uD83D\
\uDE00`!=='😀')throw 'line continuations';'ok';
"#,
        );
        succeeds("if(`a\rb\r\nc\nd\u{2028}e\u{2029}f`!=='a\\nb\\nc\\nd\\u2028e\\u2029f')throw 'normalization';if(`a\\\r\nb\\\rc`!=='abc')throw 'CR continuations';'ok';");
    }

    #[test]
    fn template_expressions_keep_existing_grammar_and_dynamic_compilation() {
        succeeds(
            r#"
var target={x:1},seen='';for(var key=`${'x' in target}`;key==='true';key='done'){seen=key;}
if(seen!=='true'||`${'x' in target}`!=='true')throw '+In expression';
if(`hello ${2}`.toUpperCase()!=='HELLO 2'||`abcd`[2]!=='c')throw 'member access';
var object={[`key${1}`]:7};if(object.key1!==7)throw 'computed key';
function C(v){this.value=v;}if(new C(`arg${9}`).value!=='arg9')throw 'construction';
var x=7;if(eval('`value ${x+1}`')!=='value 8'||eval('(function(x){return `dynamic ${x*2}`;})')(6)!=='dynamic 12')throw 'dynamic';
switch(`a${1}`){case `a${1}`:seen='match';break;default:throw 'switch';}
if(seen!=='match')throw 'case';'ok';
"#,
        );
    }

    #[test]
    fn template_plain_text_is_not_a_use_strict_directive() {
        succeeds(
            r#"
function f(){`use strict`;return this===undefined;}
function g(...args){`use strict`;return args.length;}
function h(){'other';`use strict`;return this===undefined;}
function actual(){'use strict';return `${this}`;}
if(f()||h()||g(1,2)!==2||actual()!=='undefined')throw 'directive distinction';'ok';
"#,
        );
    }

    #[test]
    fn template_invalid_grammar_and_escapes_are_atomic_syntax_errors() {
        succeeds(
            r#"
var ran=false,caught=0;for(var text of ['`unfinished','`${}`','`${1','`${(1}`','`${1 2}`','`\\1`','`\\8`','`\\01`','`\\x0G`','`\\u{110000}`','`\\u{}`']){
try{eval('ran=true;'+text);}catch(e){if(e.name==='SyntaxError')caught++;}if(ran)throw 'partial execution';}
if(caught!==11)throw 'syntax exceptions';'ok';
"#,
        );
    }

    #[test]
    fn template_tagged_calls_preserve_site_identity_receiver_and_values() {
        succeeds(
            r#"
var log='', token=Symbol('substitution'), value={toString(){throw 'coercion';}};
var owner={get tag(){log+='g';return function(site,a,b){
    if(this!==owner||a!==token||b!==value)throw 'receiver or value';
    if(site.length!==3||site[0]!=='a\n'||site[1]!=='b'||site[2]!=='c')throw 'cooked';
    if(site.raw.length!==3||site.raw[0]!=='a\\n'||site.raw[1]!=='b'||site.raw[2]!=='c')throw 'raw';
    var descriptor=Object.getOwnPropertyDescriptor(site,'raw');
    if(descriptor.value!==site.raw||descriptor.writable||descriptor.configurable||descriptor.enumerable)throw 'descriptor';
    if(!Object.isFrozen(site)||!Object.isFrozen(site.raw))throw 'mutable site';
    log+='t';return site;
};}};
function first(){return owner.tag`a\n${(log+='a',token)}b${(log+='b',value)}c`;}
function second(){return owner.tag`a\n${(log+='a',token)}b${(log+='b',value)}c`;}
if(first()!==first()||first()===second()||log!=='gabtgabtgabtgabt')throw 'order or site identity';
function empty(site){return site;}
if(empty``[0]!==''||empty``.raw[0]!=='')throw 'empty';
function factory(site){return function Box(){this.value=site[0];};}
if((new factory`box`).value!=='box')throw 'new target';
var caught=0;try{eval('owner?.tag`x`');}catch(error){if(error.name==='SyntaxError')caught++;}
if(caught!==1)throw 'optional chain';'ok';
"#,
        );
        succeeds("function tag(s){return s[0]==='a\\nb'&&s.raw[0]==='a'+String.fromCharCode(92,10,10)+'b';}if(!tag`a\\\r\r\nb`)throw 'CR normalization';'ok';");
    }

    #[test]
    fn template_optional_chain_and_lone_surrogate_boundaries_stay_explicit_and_atomic() {
        for text in [
            "var owner={};owner?.tag`text`;",
            r"tag`\uD800`;",
            r"`\uD800`;",
            r"`\uDC00`;",
            r"`\uD800${1}\uDC00`;",
        ] {
            let source = format!("var ran=true;{text}");
            let mut runtime = Runtime::for_test();
            let error = runtime
                .execute(&source, &mut NoHost::default())
                .unwrap_err();
            assert!(
                error.contains("template") || error.contains("surrogate"),
                "{error}"
            );
            assert_eq!(
                runtime
                    .execute("typeof ran", &mut NoHost::default())
                    .unwrap()
                    .to_text(),
                "undefined"
            );
        }
    }

    #[test]
    fn template_substitution_nesting_string_and_work_limits_remain_fatal() {
        let accepted = format!("`{}`", "${1}".repeat(MAX_EXPRESSION_FRAMES));
        assert_eq!(
            Runtime::for_test()
                .execute(&accepted, &mut NoHost::default())
                .unwrap()
                .to_text()
                .len(),
            MAX_EXPRESSION_FRAMES
        );
        for source in [
            format!("`{}`", "${1}".repeat(MAX_EXPRESSION_FRAMES + 1)),
            format!("{}1{}", "`${".repeat(65), "}`".repeat(65)),
            format!("`{}`", "x".repeat(MAX_STRING_BYTES + 1)),
        ] {
            let error = Runtime::for_test()
                .execute(&source, &mut NoHost::default())
                .unwrap_err();
            assert!(error.contains("limit"), "{error}");
        }
        let mut runtime = Runtime::for_test();
        runtime
            .define_global("x", Value::String("x".repeat(MAX_STRING_BYTES)))
            .unwrap();
        let error = runtime.execute("var continued=false;try{`${x}a`;}catch(e){continued=true;}finally{continued=true;}", &mut NoHost::default()).unwrap_err();
        assert!(error.contains("String limit"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("continued", &mut NoHost::default())
                .unwrap()
                .to_text(),
            "false"
        );
        let mut runtime = Runtime::for_test();
        runtime
            .define_global("x", Value::String("x".repeat(8192)))
            .unwrap();
        let error = runtime.execute("var continued=false;try{for(var n=0;n<2000;n++)`${x}`;}catch(e){continued=true;}finally{continued=true;}", &mut NoHost::default()).unwrap_err();
        assert!(
            error.contains("work limit") || error.contains("copy limit"),
            "{error}"
        );
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("continued", &mut NoHost::default())
                .unwrap()
                .to_text(),
            "false"
        );
    }

    #[test]
    fn template_native_conversion_keeps_the_host_capability_gate() {
        let mut runtime = Runtime::for_test();
        runtime.define_global("opaque", Value::Host(9)).unwrap();
        let mut host = NoHost::default();
        let error = runtime.execute("var continued=false;try{`${opaque}`;}catch(e){continued=true;}finally{continued=true;}", &mut host).unwrap_err();
        assert!(error.contains("Native host"), "{error}");
        assert_eq!(host.reads, 0);
        assert_eq!(
            runtime.execute("continued", &mut host).unwrap().to_text(),
            "false"
        );
    }
}
