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
