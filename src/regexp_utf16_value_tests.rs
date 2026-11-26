use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("Unexpected native read".into()) }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("Unexpected native write".into()) }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("Unexpected native call".into()) }
}
fn succeeds(source: &str) {
    assert_eq!(Runtime::for_test().execute(source, &mut NoHost).unwrap().to_text(), "ok");
}

#[test]
fn utf16_classic_regexp_call_accepts_a_quoted_range_and_returns_an_exact_half() {
    succeeds(r#"
var mark=RegExp("^[^\ud918\ud919]*[\ud918-\ud919]");
var result=mark.exec("ab\ud919\udc21");
if(!result || result.index!==0 || result[0].length!==3 ||
   result[0].charCodeAt(2)!==0xd919 || mark.exec("abc")!==null)throw 'classic range';
'ok';
"#);
}

#[test]
fn utf16_regexp_constructor_independent_class_units_preserve_ranges_and_last_index() {
    succeeds(r#"
var part='\uda80\uda83-\udab8\udabb-\udbff';
var actual=new RegExp('['+part+']','g');
var escaped=new RegExp('[\\uda80\\uda83-\\udab8\\udabb-\\udbff]','g');
for(var r of [actual,escaped]){
 if(!r.test('\uda83\udc00')||r.lastIndex!==1)throw 'positive high unit';
 r.lastIndex=0;
 if(r.test('\udab9\udc00')||r.lastIndex!==0)throw 'excluded high unit';
 r.lastIndex=0;
 if(!r.test('\uda80')||r.lastIndex!==1)throw 'adjacent singleton';
 r.lastIndex=0;
 if(!r.test('\udbff')||r.lastIndex!==1)throw 'upper range';
}
'ok';
"#);
}

#[test]
fn utf16_regexp_exec_real_capture_input_named_groups_and_sticky_positions() {
    succeeds(r#"
var input='a\uda83\udc00b',r=new RegExp('(?<unit>[\uda83-\udab8])','gy');
r.lastIndex=1;var m=r.exec(input);
if(m.index!==1||m[0].length!==1||m[0].charCodeAt(0)!==55939||m[1]!==m[0]||m.groups.unit!==m[0])throw 'capture unit';
if(m.input!==input||m.input.length!==4||r.lastIndex!==2)throw 'real input and end';
if(r.exec(input)!==null||r.lastIndex!==0)throw 'sticky failure reset';
var low=new RegExp('(\udc00)','g'),n=low.exec(input);
if(n.index!==2||n[1].charCodeAt(0)!==56320||low.lastIndex!==3)throw 'low capture';
var pair=/../.exec('\uda83\udc00');
if(pair[0]!=='\uda83\udc00'||pair[0].length!==2)throw 'paired canonical capture';
var empty=new RegExp('','g');empty.lastIndex=1;var e=empty.exec('\uda83');
if(e.index!==1||e[0]!==''||empty.lastIndex!==1)throw 'empty exec does not advance';
'ok';
"#);
}

#[test]
fn utf16_regexp_source_clone_and_to_string_preserve_actual_units() {
    succeeds(r#"
var source='[\uda80\uda83-\udab8\udabb-\udbff]',r=new RegExp(source,'g');
if(r.source!==source||r.source.charCodeAt(1)!==55936)throw 'raw source';
var clone=new RegExp(r);
if(clone===r||clone.source!==source||clone.flags!=='g'||!clone.test('\uda83'))throw 'clone';
if(RegExp(r)!==r||r.toString()!=='/'+source+'/g')throw 'identity or serialization';
var escaped=new RegExp('/\uda80\n');
if(escaped.source!=='\\/\uda80\\n'||escaped.toString()!=='/\\/\uda80\\n/')throw 'source escaping';
'ok';
"#);
}

#[test]
fn utf16_regexp_match_search_return_actual_units_and_convert_before_state() {
    succeeds(r#"
var text='x\uda80\uda83',r=new RegExp('[\uda80\uda83]','g');
var matches=text.match(r);
if(matches.length!==2||matches[0].charCodeAt(0)!==55936||matches[1].charCodeAt(0)!==55939||r.lastIndex!==0)throw 'global units';
if(text.search(new RegExp('\uda83'))!==2)throw 'unit search';
var scalar='\uda83\udc00'.match(/./g);
if(scalar.length!==2||scalar[0].charCodeAt(0)!==55939||scalar[1].charCodeAt(0)!==56320)throw 'scalar halves';
var count=0,live=new RegExp('\uda83','g'),value={toString:function(){count++;live.lastIndex=1;return 'x\uda83';}};
var found=live.exec(value);
if(count!==1||found.index!==1||found[0].charCodeAt(0)!==55939||live.lastIndex!==2)throw 'conversion before state';
'ok';
"#);
}

#[test]
fn legacy_regexp_class_escape_range_uses_both_atoms_and_literal_hyphen() {
    succeeds(r#"
var patterns = [/^[\d-\s]$/, new RegExp('^[\\d-\\s]$'),
                /^[\d-q]$/, new RegExp('^[q-\\d]$')];
for (var i = 0; i < patterns.length; i++) {
    var re = patterns[i];
    if (!re.test('-') || !re.test('5') || re.test('!') ||
        re.test(' ') !== (i < 2) || re.test('q') !== (i >= 2))
        throw 'legacy character class ' + i;
}
if (/^[^\d-\s]$/.test('-') || !/^[a-c]$/.test('b'))
    throw 'class controls';
var error;
try { new RegExp('[c-a]'); } catch (e) { error = e; }
if (!(error instanceof SyntaxError)) throw 'descending range';
'ok';
"#);
}

#[test]
fn unicode_regexp_bmp_classes_keep_utf16_positions_and_surrogate_boundaries() {
    succeeds(r#"
var re = /(?:[\u0400-\u040f]\u00b7)*[\u0410-\u042f][\u0430-\u044f]*/gu;
var text = '😀\u0400\u00b7\u0410\u0430😀\u0411';
re.lastIndex = 1;
var found = re.exec(text);
if (!re.unicode || re.flags !== 'gu' || found.index !== 2 ||
    found[0] !== '\u0400\u00b7\u0410\u0430' || re.lastIndex !== 6 ||
    re.exec(text).index !== 8 || re.exec(text) !== null || re.lastIndex !== 0)
    throw 'Unicode search positions';
var sticky = new RegExp('(?:)', 'uy');
sticky.lastIndex = 1;
if (sticky.exec('😀').index !== 0 || sticky.lastIndex !== 0)
    throw 'sticky inside a surrogate pair';
if ('😀x'.match(/(?:)/gu).length !== 3 ||
    '😀x'.replace(/(?:)/gu, '-') !== '-😀-x-' ||
    '😀x'.split(/(?:)/u).join('|') !== '😀|x')
    throw 'zero-width Unicode progress';
'ok';
"#);
}

#[test]
fn unicode_regexp_supplementary_ranges_consume_whole_code_points() {
    succeeds(r#"
var re = /([A\u{1f680}-\u{1f683}]+)A$/u;
var found = re.exec('!🚀A🚂A');
if (found.index !== 1 || found[1] !== '🚀A🚂' || found[0].length !== 6)
    throw 'mixed-width repetition or backtracking';
var sticky = new RegExp('[\\u{1f680}-\\u{1f683}]', 'uy');
sticky.lastIndex = 1;
if (sticky.exec('🚀').index !== 0 || sticky.lastIndex !== 2 || sticky.test('x'))
    throw 'supplementary sticky match';
if (!/\u{000041}/u.test('A') || !/🚀/u.test('🚀') || /[\u{1f680}-\u{1f683}]/u.test('😀'))
    throw 'supplementary literals or endpoints';
for (var invalid of ['\\u{}', '\\u{110000}', '\\u{1z}', '\\u{12']) {
    var error = null;
    try { new RegExp(invalid, 'u'); } catch (e) { error = e; }
    if (!(error instanceof SyntaxError)) throw 'invalid braced escape';
}
'ok';
"#);
}

#[test]
fn utf16_regexp_genuine_class_escape_and_flag_errors_remain_syntax_errors() {
    succeeds(r#"
var errors=0;
for(var pattern of ['[\udab8-\uda83]','[\\u12xz]','[\\x0z]','[\\c1]']){
 try{new RegExp(pattern);}catch(e){if(e instanceof SyntaxError)errors++;else throw e;}
}
for(var flags of ['gg','q','\uda80']){
 try{new RegExp('\uda80',flags);}catch(e){if(e instanceof SyntaxError)errors++;else throw e;}
}
if(errors!==7||!new RegExp('[\uda83-\udab8]').test('\uda83'))throw 'syntax controls';
'ok';
"#);
}

#[test]
fn utf16_regexp_raw_compilation_limits_remain_fatal_and_do_not_publish_regex() {
    let mut runtime = Runtime::for_test();
    runtime.execute("var pattern='[\\uda80]',finished=false;", &mut NoHost).unwrap();
    let retained = runtime.retained_regexp;
    runtime.regexp_work = 0;
    let error = runtime.execute("try{new RegExp(pattern);}catch(e){finished=true;}finally{finished=true;}", &mut NoHost).unwrap_err();
    assert!(error.contains("Regular-expression work limit"), "{error}");
    assert_eq!(runtime.retained_regexp, retained);
    assert_eq!(runtime.regexp_work, 0);
    runtime.begin_job();
    assert_eq!(runtime.execute("!finished&&new RegExp(pattern).test('\\uda80');", &mut NoHost).unwrap().to_text(), "true");
}

#[test]
fn utf16_regexp_scalar_result_input_keeps_the_original_copy_budget() {
    let text = "x".repeat(4096);
    for (method, pattern, input_charges) in [("exec", "/x/g", 5), ("match", "/x/", 4)] {
        let mut runtime = Runtime::for_test();
        let expression = runtime.execute(pattern, &mut NoHost).unwrap();
        let Value::Object(regex) = expression else { panic!("expected RegExp") };
        runtime.begin_job();
        // Three lengths for matching input and one for the retained property.
        // RegExp.exec additionally snapshots its argument before conversion.
        runtime.copy_bytes = text.len() * input_charges + 128;
        let value = if method == "exec" {
            runtime.regexp_method(regex, method, &[Value::String(text.clone())], &mut NoHost)
        } else {
            runtime.regexp_string_method(&text, method, &[expression], &mut NoHost)
        }.unwrap();
        let Value::Object(result) = value else { panic!("expected a match result") };
        runtime.begin_job();
        assert!(matches!(runtime.own_property(result, "input").unwrap(), Value::String(input) if input == text));
        assert!(matches!(runtime.own_property(result, "0").unwrap(), Value::String(first) if first == "x"));
        assert!(matches!(runtime.own_property(regex, "lastIndex").unwrap(), Value::Number(last)
            if last == (if method == "exec" { 1.0 } else { 0.0 })));
    }
}

#[test]
fn unicode_character_sets_native_exec_keeps_exact_units_and_sticky_offsets() {
    succeeds(r#"
var text='😀A\ud800\n🚀', re=/./gu, found=re.exec(text);
if(found[0]!=='😀'||found.index!==0||re.lastIndex!==2)throw 'paired dot';
found=re.exec(text);if(found[0]!=='A'||found.index!==2||re.lastIndex!==3)throw 'BMP dot';
found=re.exec(text);if(found[0].length!==1||found[0].charCodeAt(0)!==0xd800||re.lastIndex!==4)throw 'unpaired dot';
found=re.exec(text);if(found[0]!=='🚀'||found.index!==5||re.lastIndex!==7)throw 'skip newline';
if(re.exec(text)!==null||re.lastIndex!==0)throw 'end reset';
var sticky=new RegExp('[^!]','uy');sticky.lastIndex=1;
if(sticky.exec('😀')[0]!=='😀'||sticky.lastIndex!==2)throw 'interior sticky';
sticky.lastIndex=2;if(sticky.exec('😀')!==null||sticky.lastIndex!==0)throw 'sticky end';
if(!/^.$/su.test('\n')||/^.$/u.test('\u2029')||!/^.$/su.test('\u2029'))throw 'dotAll';
'ok';
"#);
}

#[test]
fn unicode_character_sets_native_string_consumers_keep_captures_and_progress() {
    succeeds(r#"
var text='😀A🚀', list=text.match(/\D/gu);
if(list.length!==3||list[0]!=='😀'||list[2]!=='🚀')throw 'global match';
if(text.search(/[^A]/u)!==0||'A😀'.search(/\W/u)!==1)throw 'search offsets';
if(text.replace(/([^!]+?)A/u,'<$1>')!=='<😀>🚀')throw 'capture replacement';
var seen='';var result=text.replace(/./gu,function(value,index,input){
 seen=seen+index+':'+value.length+';';if(input!==text)throw 'real input';return '['+value+']';
});
if(seen!=='0:2;2:1;3:2;'||result!=='[😀][A][🚀]')throw 'callback units and offsets';
if('a😀b🚀c'.split(/\W/u).join('|')!=='a|b|c')throw 'split code points';
if('😀A'.match(/[^!]*?/gu).length!==3||'😀A'.replace(/[^!]*?/gu,'-')!=='-😀-A-')throw 'empty progress';
if(!/^[^\D]+$/u.test('59')||/^[^\D]+$/u.test('😀')||!/^\W+$/u.test('😀🚀'))throw 'complement union';
'ok';
"#);
}

#[test]
fn unicode_surrogate_pattern_native_source_captures_and_consumers_are_exact() {
    succeeds(r#"
var raw=new RegExp('\ud800','gu'), escaped=new RegExp('\\uD800','gu');
if(raw.source!=='\ud800'||raw.toString()!=='/\ud800/gu')throw 'raw source';
for(var re of [raw,escaped]){
 var m=re.exec('😀\ud800x');if(m.index!==2||m[0].charCodeAt(0)!==0xd800||re.lastIndex!==3)throw 'lone match';
 re.lastIndex=0;if(re.test('😀'))throw 'paired input excluded';
}
var pair=/\uD83D\uDE00+/u, found=pair.exec('x😀😀');
if(found.index!==1||found[0].length!==4)throw 'pair quantifier';
if('😀\ud800😀'.replace(/\ud800/u,'-')!=='😀-😀')throw 'exact replacement';

if('😀\ud800'.search(/\ud800/u)!==2)throw 'search offset';
'ok';
"#);
}

#[test]
fn unicode_surrogate_pattern_native_classes_and_mixed_escapes_keep_grammar() {
    succeeds(r#"
if(!/^[\uD83D\uDE00-\uD83D\uDE03]+$/u.test('😀😃')||/^[\uD83D\uDE00-\uD83D\uDE03]+$/u.test('🚀'))throw 'paired range';
if(!/^[^\uD800-\uDFFF]+$/u.test('😀')||/^[^\uD800-\uDFFF]+$/u.test('\ud800'))throw 'surrogate range complement';
if(/\uD83D+/u.test('😀')||/\u{d83d}\u{de00}/u.test('😀')||/\uD83D\u{de00}/u.test('😀'))throw 'separate atoms';
var rawHigh='\ud83d',rawLow='\ude00';
if(new RegExp(rawHigh+'\\uDE00','u').test('😀')||new RegExp('\\uD83D'+rawLow,'u').test('😀'))throw 'raw escaped distinction';
if(!new RegExp(rawHigh+rawLow,'u').test('😀'))throw 'raw pair';
var error=null;try{new RegExp('[\\uDFFF-\\uD800]','u');}catch(e){error=e;}
if(!(error instanceof SyntaxError)||!/^\ud800$/u.test('\ud800'))throw 'syntax then recovery';
'ok';
"#);
}

#[test]
fn unicode_surrogate_pattern_split_backend_preserves_exact_units_and_native_recovery() {
    let mut runtime=Runtime::for_test();
    assert_eq!(runtime.execute(r"'a\ud800b'.split(/\ud800/u).join('|')",&mut NoHost).unwrap().to_text(),"a|b");
    assert_eq!(runtime.execute(r"/^\ud800$/u.test('\ud800') ? 'ok' : 'bad'",&mut NoHost).unwrap().to_text(),"ok");
}

#[test]
fn unicode_group_reference_native_results_named_captures_and_consumers_keep_offsets() {
    succeeds(r#"
var re=/(?<x>.)\k<x>/gu, found=re.exec('!😀😀');
if(found.index!==1||found[0]!=='😀😀'||found[1]!=='😀'||found.groups.x!=='😀'||found.input!=='!😀😀'||re.lastIndex!==5)throw 'exact named result';
if(re.exec('!😀😀')!==null||re.lastIndex!==0)throw 'global reset';
if('x😀A'.replace(/(?<=(.))A/u,'<$1>')!=='x😀<😀>')throw 'lookbehind replacement';
var pieces='😀😀!'.split(/(?<x>.)\k<x>/u);if(pieces.length!==3||pieces[0]!==''||pieces[1]!=='😀'||pieces[2]!=='!')throw 'captured split';
if('😀A'.search(/(?<=😀)A/u)!==2||'😀A'.match(/(?!😀)./u)[0]!=='A')throw 'assertion consumers';
var clone=new RegExp(re);if(clone.source!==re.source||clone.flags!=='gu'||!clone.unicode)throw 'clone metadata';
'ok';
"#);
}

#[test]
fn unicode_group_reference_native_errors_boundaries_and_fresh_state_are_preserved() {
    succeeds(r#"
for(var pattern of ['(?=a)*','(a)\\2','\\01','[\\1]','(?<x>a)\\k<y>']){
 var error=null;try{new RegExp(pattern,'u');}catch(e){error=e;}
 if(!(error instanceof SyntaxError))throw 'Unicode grammar syntax';
}
if(/(\ud800)\1/u.test('\ud800\ud800\udc00')||/(?<=\1(\udc00))A/u.test('\ud800\udc00\udc00A'))throw 'reference pair boundaries';
if(!/(\ud800)\1/u.test('\ud800\ud800')||!/(?<=\1(\udc00))A/u.test('\udc00\udc00A'))throw 'isolated reference recovery';
if(!/^\1(.)$/u.test('😀')||!/^(a|(b))\2c$/u.test('ac'))throw 'forward unmatched';
'ok';
"#);
}

#[test]
fn unicode_set_algebra_native_metadata_and_consumers_keep_code_points() {
    succeeds(r#"
var re=/[[a-z]--[aeiou]]+/gv;
if(!re.unicodeSets||re.unicode||re.flags!=='gv'||re.toString()!=='/[[a-z]--[aeiou]]+/gv')throw 'v metadata';
var m=re.exec('aBcdef');if(m.index!==2||m[0]!=='cd'||re.lastIndex!==4)throw 'v match';
var copy=new RegExp(re);if(!copy.unicodeSets||copy.unicode||copy.source!==re.source)throw 'v clone';
if('😀A'.split(/(?:)/v).join('|')!=='😀|A'||'😀'.replace(/(?:)/gv,'!')!=='!😀!')throw 'v empty progress';
if('😀A'.match(/(?:)/gv).length!==3)throw 'v global progress';
if('😀A'.search(/[[😀A]&&[^😀]]/v)!==2)throw 'v search';
var lone=new RegExp('[[\\ud800]--[a]]','v');if(!lone.test('\ud800')||lone.test('😀'))throw 'v lone units';
if(!/(?<x>[[😀A]--[A]])\k<x>/v.test('😀😀'))throw 'v shared groups';
'ok';
"#);
}

#[test]
fn unicode_set_algebra_native_invalid_syntax_and_fresh_recovery_are_exact() {
    succeeds(r#"
for(var pair of [['a','uv'],['a','vu'],['a','vv'],['[a&&b--c]','v'],['[a-z&&b]','v'],['[a-]','v'],['[!!]','v']]){
 var error=null;try{new RegExp(pair[0],pair[1]);}catch(e){error=e;}if(!(error instanceof SyntaxError))throw 'v syntax';
}
if(!/[a&&b]/u.test('&')||!/^[[a-z]&&[^aeiou]]$/v.test('b')||/^[[a-z]&&[^aeiou]]$/v.test('a'))throw 'fresh v grammar';
'ok';
"#);
}

#[test]
fn unicode_string_set_native_captures_consumers_and_progress_are_exact() {
    succeeds(r#"
var re=/(?<x>[\q{😀|😀A}])/gv, m=re.exec('!😀A');
if(m.index!==1||m[0]!=='😀A'||m.groups.x!=='😀A'||re.lastIndex!==4||!re.unicodeSets||re.unicode)throw 'q native result';
if('😀A!'.replace(/([\q{😀|😀A}])/v,'<$1>')!=='<😀A>!')throw 'q replace';
var pieces='x😀Ay'.split(/([\q{😀A}])/v);if(pieces.length!==3||pieces[0]!=='x'||pieces[1]!=='😀A'||pieces[2]!=='y')throw 'q split';
if('!😀A'.search(/[\q{😀A}]/v)!==1||'😀'.match(/[\q{}]/gv).length!==2)throw 'q search empty progress';
if('😀A'.split(/[\q{}]/v).join('|')!=='😀|A')throw 'q empty split';
if(!/(?<=([\q{😀|😀A}]))!/v.test('😀A!')||!/^([\q{😀A}])\1$/v.test('😀A😀A'))throw 'q shared assertion reference';
'ok';
"#);
}

#[test]
fn unicode_string_set_native_syntax_and_algebra_preserve_fresh_execution() {
    succeeds(r#"
for(var pattern of ['[^\\q{ab}]','[^\\q{}]','[^\\q{ab}--\\q{ab}]','[\\q{a}-z]','[\\q{a-b}]','[\\q{\\d}]']){
 var error=null;try{new RegExp(pattern,'v');}catch(e){error=e;}if(!(error instanceof SyntaxError))throw 'q syntax';
}
if(!/^[[\q{ab|a|}]--[\q{ab}]]$/v.test('a')||!/^[[\q{ab|a|}]--[\q{ab}]]$/v.test('')||/^[[\q{ab|a|}]--[\q{ab}]]$/v.test('ab'))throw 'q subtraction';
if(!/^[^[\q{ab}]&&[a]]$/v.test('😀')||!/^([\q{a|ab|abc}])bc$/v.test('abc'))throw 'q recovery';
'ok';
"#);
}
