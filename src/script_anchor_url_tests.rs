//! Original HTML hyperlink API reductions, independent of framework source.
use super::*;

fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<!doctype html><title>Anchor projection</title><body><a id=parsed href=initial>Original</a><div id=other></div><svg><a id=foreign href=old></a></svg>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let base = crate::navigation::address("https://example.test/one/page", &crate::navigation::Location::home()).unwrap();
    page.set_location(&base);
    page.set_resource_base(&base);
    let outcome = page.start(&mut doc, &mut forms, &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    (doc, forms, page)
}

fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let outcome = page.console_eval(doc, forms, source);
    assert_eq!(outcome.errors, 0, "{source}\n{}", page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    assert!(page.take_image_changes().2.is_empty());
}

#[test]
fn anchor_url_detached_raw_attribute_and_live_effective_base() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var a=document.createElement('a');
a.href='../next?q=one two#part';
if(a.getAttribute('href')!=='../next?q=one two#part')throw 'raw href';
if(a.href!=='https://example.test/next?q=one%20two#part'||a.protocol!=='https:'||a.host!=='example.test')throw 'resolved href';
var h=a.getAttributeNode('href');
"#);
    let base = crate::navigation::address("https://other.test/two/deeper/page", &crate::navigation::Location::home()).unwrap();
    page.set_resource_base(&base);
    run(&mut doc, &mut forms, &mut page, r#"
if(a.href!=='https://other.test/two/next?q=one%20two#part'||h.value!=='../next?q=one two#part'||a.getAttributeNode('href')!==h)throw 'live base or Attr identity';
a.href=a.href;
if(h.value!=='https://other.test/two/next?q=one%20two#part')throw 'normalizing assignment';
"#);
    let base = crate::navigation::address("https://third.test/elsewhere", &crate::navigation::Location::home()).unwrap();
    page.set_resource_base(&base);
    run(&mut doc, &mut forms, &mut page, "if(a.href!=='https://other.test/two/next?q=one%20two#part')throw 'absolute href moved';");
}

#[test]
fn anchor_url_absent_empty_invalid_and_live_attribute_mutation() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var a=document.createElement('a');
if(a.href!==''||a.protocol!==':'||a.host!==''||a.origin!=='')throw 'absent';
a.href='';
if(a.href!=='https://example.test/one/page'||a.getAttribute('href')!=='')throw 'present empty';
a.href='http://[invalid';
if(a.href!=='http://[invalid'||a.protocol!==':'||a.hostname!==''||a.pathname!=='')throw 'invalid URL fallback';
a.setAttribute('href','//EXAMPLE.test:443/ok?#');
if(a.href!=='https://example.test/ok?#'||a.search!==''||a.hash!==''||a.port!=='')throw 'live attribute projection';
a.getAttributeNode('href').value='#changed';
if(a.href!=='https://example.test/one/page#changed'||a.hash!=='#changed')throw 'Attr update';
a.removeAttribute('href');
if(a.href!==''||a.protocol!==':'||a.hasAttribute('href'))throw 'removal';
"#);
}

#[test]
fn anchor_url_components_share_canonical_record_and_mutate_raw_href() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var a=document.getElementById('parsed');
a.href=' HTTPS://user:pass@EXAMPLE.test:443/a/%2e%2e/b?q=one two#snow ';
if(a.href!=='https://user:pass@example.test/b?q=one%20two#snow'||a.origin!=='https://example.test'||a.username!=='user'||a.password!=='pass'||a.hostname!=='example.test'||a.port!==''||a.pathname!=='/b'||a.search!=='?q=one%20two'||a.hash!=='#snow')throw 'canonical components';
var h=a.getAttributeNode('href');
a.host='[2001:db8::1]:8443';a.username='first last';a.password='p@ss';a.pathname='/next path';a.search='?q=a b';a.hash='#part two';
if(a.href!=='https://first%20last:p%40ss@[2001:db8::1]:8443/next%20path?q=a%20b#part%20two'||a.host!=='[2001:db8::1]:8443'||a.hostname!=='[2001:db8::1]'||a.port!=='8443')throw 'component mutations';
if(h!==a.getAttributeNode('href')||h.value!==a.href)throw 'mutation lost Attr';
a.protocol='http:';a.port='80';a.hostname='Other.test';a.search='';a.hash='';
if(a.href!=='http://first%20last:p%40ss@other.test/next%20path'||a.origin!=='http://other.test')throw 'default port and empty tails';
a.href='../raw';a.hostname='invalid:123';
if(a.getAttribute('href')!=='../raw')throw 'invalid component canonicalized raw';
a.removeAttribute('href');a.hash='#ignored';
if(a.hasAttribute('href'))throw 'component setter created missing href';
"#);
}

#[test]
fn anchor_url_coercion_is_ordered_once_and_abrupt_assignment_is_atomic() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var a=document.getElementById('parsed'), conversions=0;
a.href={toString:function(){conversions++;a.setAttribute('href','side-effect');return '../final';}};
if(conversions!==1||a.getAttribute('href')!=='../final'||a.href!=='https://example.test/final')throw 'href conversion';
a.hash={toString:function(){conversions++;a.setAttribute('href','https://changed.test/fresh');return 'final part';}};
if(conversions!==2||a.href!=='https://changed.test/fresh#final%20part')throw 'component conversion saw stale URL';
var old=a.getAttribute('href'), failures=0;
try{a.href={toString:function(){failures++;throw 'original';}};}catch(e){}
try{a.href=Symbol('url');}catch(e){failures++;}
if(failures!==2||a.getAttribute('href')!==old)throw 'abrupt coercion changed href';
a.href=null;if(a.getAttribute('href')!=='null')throw 'null conversion';
a.href=undefined;if(a.getAttribute('href')!=='undefined')throw 'undefined conversion';
"#);
}

#[test]
fn anchor_url_failed_retained_and_copy_budget_preserve_attribute_and_requests() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, "var a=document.getElementById('parsed'), h=a.getAttributeNode('href');");
    let node = (0..doc.nodes.len()).find(|&node| doc.attr(node,"id")==Some("parsed")).unwrap();
    let retained = page.state.retained;
    page.state.retained_limit = retained;
    for source in ["a.href='a-much-longer-original-target';", "a.hash='a-much-longer-original-fragment';"] {
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert_eq!(doc.attr(node,"href"), Some("initial"));
        assert_eq!(page.state.retained, retained);
        assert!(!page.has_navigation_request());
    }
    let copy = page.state.limits.copy_bytes;
    page.state.limits.copy_bytes = 256;
    let outcome = page.console_eval(&mut doc, &mut forms, "a.href;");
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.attr(node,"href"), Some("initial"));
    page.state.limits.copy_bytes = copy;
    run(&mut doc, &mut forms, &mut page, "if(h!==a.getAttributeNode('href')||h.value!=='initial')throw 'failed quota changed Attr';");
}

#[test]
fn anchor_url_namespace_and_unsupported_capabilities_leave_raw_data_intact() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, "var a=document.getElementById('parsed');a.setAttributeNS('urn:metadata','m:href','metadata');a.href='mailto:original@example.test';");
    for source in ["a.href;", "a.protocol;", "document.getElementById('foreign').href='changed';"] {
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
        assert!(!page.has_navigation_request());
    }
    run(&mut doc, &mut forms, &mut page, "var other=document.getElementById('other');other.href='ordinary expando';if(other.getAttribute('href')!==null||other.href!=='ordinary expando')throw 'expando became link data';");
    run(&mut doc, &mut forms, &mut page, "if(a.getAttribute('href')!=='mailto:original@example.test'||a.getAttributeNS('urn:metadata','href')!=='metadata')throw 'unsupported projection changed raw data';");
}
