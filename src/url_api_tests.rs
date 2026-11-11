//! Original regression tests for the bounded local URL/query API.
use super::*;
struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        Err("URL values must not read a host".into())
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        Err("URL values must not write a host".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        Err("URL values must not call a host".into())
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
fn error(source: &str, fragment: &str) {
    let e = Runtime::new().execute(source, &mut NoHost).unwrap_err();
    assert!(e.contains(fragment), "{e}");
}

#[test]
fn urls_parse_and_resolve_without_network_capabilities() {
    succeeds(
        r#"
var u=new URL('../c?q=one two#part','HTTPS://User:pass@Example.org:443/a/b');
if(u.href!=='https://User:pass@example.org/c?q=one%20two#part'||u.origin!=='https://example.org')throw 'serialize';
if(u.searchParams.get('q')!=='one two'||u.hostname!=='example.org'||u.port!=='')throw 'components';
if(new URL('https:next',u).href!=='https://User:pass@example.org/next')throw 'same scheme';
if(new URL('//other.test/path',u).href!=='https://other.test/path')throw 'authority';
'ok';"#,
    );
}
#[test]
fn urls_dot_segments_and_special_slashes_are_canonical() {
    for (text, expected) in [
        ("https://example.org/a/%2e%2e/b", "https://example.org/b"),
        ("http:\\example.org\\a\\..\\b", "http://example.org/b"),
        ("https://example.org//a/../", "https://example.org//"),
        ("https://./", "https://./"),
    ] {
        assert_eq!(parse_url(text, None).unwrap().href(), expected);
    }
}
#[test]
fn urls_ipv4_and_ipv6_are_normalized_without_dns() {
    for (text, expected) in [
        ("http://0x7f.1/", "http://127.0.0.1/"),
        (
            "https://[0:0:0:0:0:FFFF:192.0.2.1]/",
            "https://[::ffff:c000:201]/",
        ),
        ("http://0177.1:80/", "http://127.0.0.1/"),
        ("http://0xffffffff/", "http://255.255.255.255/"),
    ] {
        assert_eq!(parse_url(text, None).unwrap().href(), expected);
    }
    for text in [
        "http://256.0.0.1/",
        "http://example.1/",
        "http://0x100000000000000000000/",
        "http://[2001:db8::1%25zone]/",
        "http://1.2.3.4:65536/",
    ] {
        assert!(
            matches!(parse_url(text, None), Err(ParseError::Invalid)),
            "{text}"
        );
    }
}
#[test]
fn urls_component_setters_preserve_query_identity() {
    succeeds(
        r#"
var u=new URL('https://example.org:8080/a?q=one'),p=u.searchParams;
u.host='next.test';if(u.port!=='8080')throw 'preserve port';
u.host='third.test:bad';if(u.hostname!=='third.test'||u.port!=='8080')throw 'host progress';
u.port='123rest';u.username='a\tb';u.password='c\nd';
if(u.username!=='a%09b'||u.password!=='c%0Ad'||u.port!=='123')throw 'userinfo';
u.search='?q=two';u.hash='a b';if(p!==u.searchParams||p.get('q')!=='two')throw 'live';
u.href='http://final.test/?z=3';if(p!==u.searchParams||p.get('z')!=='3')throw 'href';
'ok';"#,
    );
}
#[test]
fn urls_empty_query_and_fragment_preserve_delimiters() {
    succeeds(
        r#"
var u=new URL('https://example.org/?#');
if(u.search!==''||u.hash!==''||u.href!=='https://example.org/?#')throw 'empty';
u.searchParams.sort();if(u.href!=='https://example.org/#')throw 'query update';
u.hash='';if(u.href!=='https://example.org/')throw 'clear';'ok';"#,
    );
}
#[test]
fn urls_static_parse_distinguishes_malformed_from_unsupported() {
    succeeds(
        r#"if(URL.parse('/relative')!==null||URL.canParse('/relative'))throw 'invalid';if(!URL.canParse('/x','https://example.org')||URL.parse('/x','https://example.org').pathname!=='/x')throw 'base';try{new URL('bad');throw 'missing'}catch(e){if(e.name!=='TypeError')throw e;}'ok';"#,
    );
    error("URL.canParse('file:///a');", "only HTTP(S)");
    error("new URL('https://é.test');", "IDNA");
    error("new URL('https://xn--bcher-kva.test');", "IDNA");
}
#[test]
fn query_string_parsing_is_forgiving_utf8_form_encoding() {
    succeeds(
        r#"
var p=new URLSearchParams('?a=1&&a=2&space=a+b&bad=%E2%28%A1&bom=%EF%BB%BF');
if(p.getAll('a').join('|')!=='1|2'||p.get('space')!=='a b'||p.get('bad')!=='�(�'||p.get('bom')!=='﻿')throw 'parse';
p.append('a b+','é😀~!*()');if(p.toString().indexOf('a+b%2B=%C3%A9%F0%9F%98%80%7E%21*%28%29')<0)throw 'encode';
if(new URLSearchParams(null).toString()!=='null='||new URLSearchParams(undefined).size!==0)throw 'nullish';'ok';"#,
    );
}
#[test]
fn query_duplicates_optional_values_and_stable_utf16_sort() {
    succeeds(
        r#"
var p=new URLSearchParams('b=0&a=1&a=2&a=1');p.delete('a','1');if(p.toString()!=='b=0&a=2')throw 'delete';
p.append('a','3');p.set('b','x');p.sort();if(p.toString()!=='a=2&a=3&b=x')throw 'stable';
if(!p.has('a',undefined)||p.has('a','none'))throw 'has';p.delete('a',undefined);if(p.toString()!=='b=x')throw 'undefined';
var u=new URLSearchParams([['\uE000','a'],['😀','b']]);u.sort();if(Array.from(u.keys())[0]!=='😀')throw 'utf16';'ok';"#,
    );
}
#[test]
fn query_iterators_and_callbacks_observe_live_replacement() {
    succeeds(
        r#"
var u=new URL('https://example.org/?a=1&b=2'),p=u.searchParams,i=p.entries();
if(i.next().value.join(':')!=='a:1')throw 'first';u.search='x=3&y=4';
if(i.next().value.join(':')!=='y:4'||!i.next().done)throw 'replace';p.append('z','5');
if(i.next().value.join(':')!=='z:5')throw 'resume';
var log='',receiver={};p.forEach(function(v,k,s){if(s!==p||this!==receiver)throw 'callback';log+=k+v;if(k==='x')p.append('end','6')},receiver);
if(log!=='x3y4z5end6')throw log;'ok';"#,
    );
}
#[test]
fn query_conversions_read_latest_slots_and_record_keys() {
    succeeds(
        r#"
var p=new URLSearchParams('a=1'),trace='';
p.set({toString(){p.append('b','2');trace+='k';return 'a'}},{toString(){p.append('c','3');trace+='v';return 'x'}});
if(p.toString()!=='a=x&b=2&c=3'||trace!=='kv')throw 'conversion';
var o={get a(){delete this.b;this.c='late';return 'one'},b:'two'};
if(new URLSearchParams(o).toString()!=='a=one')throw 'key snapshot';'ok';"#,
    );
}
#[test]
fn query_callbacks_are_bounded_and_cannot_reset_a_job() {
    error(
        "var p=new URLSearchParams('x=1');p.forEach(function(){while(true){}});",
        "limit",
    );
    error(
        "var p=new URLSearchParams,o={toString(){return p.get(o)}};p.append(o,'v');",
        "limit",
    );
}
#[test]
fn query_iterator_next_has_web_idl_operation_attributes() {
    succeeds(
        r#"
var iterator=new URLSearchParams('q=one').entries();
var descriptor=Object.getOwnPropertyDescriptor(Object.getPrototypeOf(iterator),'next');
if(!descriptor.enumerable||!descriptor.writable||!descriptor.configurable)throw 'Web IDL attributes';
if(descriptor.value.name!=='next'||descriptor.value.length!==0)throw 'operation metadata';
if(descriptor.value.call(iterator).value.join(':')!=='q:one')throw 'iterator operation';
try{descriptor.value.call({});throw 'accepted forged iterator';}catch(e){if(e.name!=='TypeError')throw e;}
'ok';
"#,
    );
}

#[test]
fn query_internal_slots_survive_freeze_but_cannot_be_forged() {
    succeeds(
        r#"
var p=new URLSearchParams('a=1'),u=new URL('https://example.org/');Object.freeze(p);Object.freeze(u);p.append('b','2');u.search='q=ok';
if(p.size!==2||u.searchParams.get('q')!=='ok')throw 'freeze';
try{URLSearchParams.prototype.append.call(Object.create(URLSearchParams.prototype),'a','b');throw 'missing'}catch(e){if(e.name!=='TypeError')throw e;}
try{Object.getOwnPropertyDescriptor(URL.prototype,'href').get.call({});throw 'missing'}catch(e){if(e.name!=='TypeError')throw e;}'ok';"#,
    );
}
#[test]
fn urls_and_queries_charge_all_retained_strings_and_pair_metadata() {
    let mut runtime = Runtime::new();
    let initial = runtime.value_bytes;
    let Value::Object(id) = runtime
        .url_construct(
            Kind::Url,
            &[Value::String("https://example.org/?a=1&b=2".into())],
            &mut NoHost,
        )
        .unwrap()
    else {
        panic!()
    };
    let params = runtime.url_record(id).unwrap().1;
    assert_eq!(
        runtime.value_bytes - initial,
        runtime.url_data(id).unwrap().bytes() + runtime.url_data(params).unwrap().bytes()
    );
    runtime
        .params_replace(params, vec![("large".into(), "abc xyz".into())])
        .unwrap();
    assert_eq!(
        runtime.value_bytes - initial,
        runtime.url_data(id).unwrap().bytes() + runtime.url_data(params).unwrap().bytes()
    );
    runtime.params_replace(params, Vec::new()).unwrap();
    assert_eq!(
        runtime.value_bytes - initial,
        runtime.url_data(id).unwrap().bytes() + runtime.url_data(params).unwrap().bytes()
    );
    let empty_pairs = vec![(String::new(), String::new()); MAX_PAIRS];
    assert_eq!(
        pair_storage(&empty_pairs),
        MAX_PAIRS * std::mem::size_of::<Pair>()
    );
    let mut count = 0;
    loop {
        runtime.begin_job();
        match runtime.new_params_value(empty_pairs.clone()) {
            Ok(_) => count += 1,
            Err(error) => {
                assert!(error.display().contains("Retained value limit"));
                break;
            }
        }
    }
    assert!(
        count < 64,
        "Empty query metadata escaped the retained budget"
    );
}
#[test]
fn query_failed_attached_mutations_leave_both_slots_unchanged() {
    let mut runtime = Runtime::new();
    let Value::Object(url) = runtime
        .url_construct(
            Kind::Url,
            &[Value::String("https://example.org/?x=old".into())],
            &mut NoHost,
        )
        .unwrap()
    else {
        panic!()
    };
    let params = runtime.url_record(url).unwrap().1;
    let before = runtime.value_bytes;
    let old = runtime.url_record(url).unwrap().0.href();
    let result = runtime.params_replace(params, vec![("x".into(), "x".repeat(MAX_URL_BYTES))]);
    assert!(result.err().unwrap().display().contains("URL length limit"));
    assert_eq!(runtime.value_bytes, before);
    assert_eq!(runtime.url_record(url).unwrap().0.href(), old);
    assert_eq!(runtime.params_data(params).unwrap().0[0].1, "old");
}
#[test]
fn urls_lengths_and_query_entries_have_independent_limits() {
    assert!(matches!(
        parse_url(
            &format!("https://example.org/{}", "x".repeat(MAX_URL_BYTES)),
            None
        ),
        Err(ParseError::Limit(_))
    ));
    assert!(parse_pairs(&"x".repeat(MAX_PARAMS_BYTES + 1))
        .err()
        .unwrap()
        .display()
        .contains("byte limit"));
    assert!(parse_pairs(&"x=&".repeat(MAX_PAIRS + 1))
        .err()
        .unwrap()
        .display()
        .contains("entry limit"));
    let mut runtime = Runtime::new();
    let count = runtime.objects.len();
    runtime.value_bytes = MAX_RETAINED_VALUES - 1;
    assert!(runtime
        .url_construct(
            Kind::Url,
            &[Value::String("https://example.org/".into())],
            &mut NoHost
        )
        .err()
        .unwrap()
        .display()
        .contains("Retained value limit"));
    assert_eq!(runtime.objects.len(), count);
}
#[test]
fn query_limits_remain_fatal_and_do_not_run_catch_blocks() {
    let mut runtime = Runtime::new();
    runtime.execute("var caught=false;", &mut NoHost).unwrap();
    let source = format!(
        "try{{new URLSearchParams('{}');}}catch(e){{caught=true;}}",
        "a=&".repeat(MAX_PAIRS + 1)
    );
    assert!(runtime
        .execute(&source, &mut NoHost)
        .unwrap_err()
        .contains("entry limit"));
    runtime.begin_job();
    assert_eq!(
        runtime.execute("caught;", &mut NoHost).unwrap().to_text(),
        "false"
    );
}
