//! Original anchor content-attribute controls. Mutation alone never opens a link.
use super::*;

fn fixture(source: &str) -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<!doctype html><title>Attribute source</title><body><a id=a href=initial.html download=original.bin>Original anchor</a><div id=other></div><script id=resource src=original.js></script><link id=sheet rel=stylesheet href=original.css><svg><a id=foreign href=foreign.html download=foreign.bin>Foreign anchor</a></svg></body>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let location = crate::navigation::address("https://anchor-attributes.test/source", &crate::navigation::Location::home()).unwrap();
    page.set_location(&location);
    page.set_resource_base(&location);
    let outcome = page.start(&mut doc, &mut forms, &[Script {
        node: 0, label: "Original anchor attribute fixture".into(), source: source.into(),
    }]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    (doc, forms, page)
}

fn node(doc: &Document, id: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(id)).unwrap()
}

#[test]
fn anchor_attributes_dom_conversion_raw_values_and_removal_never_navigate() {
    let (doc, _, mut page) = fixture(r#"
var a=document.getElementById('a'), order='';
a.setAttribute({toString:function(){order+='N';return 'HREF';}},
 {toString:function(){order+='V';return '../missing.html?q=raw#part';}});
if(order!=='NV'||a.getAttribute('href')!=='../missing.html?q=raw#part')throw 'DOM conversion order or raw href';
a.setAttribute('DOWNLOAD',{toString:function(){order+='D';return 'report 1.txt';}});
if(order!=='NVD'||a.getAttribute('download')!=='report 1.txt')throw 'download conversion';
a.removeAttribute('DOWNLOAD');
if(a.hasAttribute('download'))throw 'download removal';
a.setAttribute('href','');
if(!a.hasAttribute('href')||a.getAttribute('href')!=='')throw 'empty href is present';
a.removeAttribute('HREF');
if(a.hasAttribute('href'))throw 'href removal';
a.setAttribute('href','https://other-attributes.test/unvisited#target');
"#);
    assert_eq!(doc.attr(node(&doc, "a"), "href"), Some("https://other-attributes.test/unvisited#target"));
    assert_eq!(doc.title, "Attribute source");
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (_, _, images) = page.take_image_changes();
    assert!(images.is_empty());
}

#[test]
fn anchor_attributes_attached_attr_adoption_removal_and_live_map_keep_real_identity() {
    let (doc, _, page) = fixture(r#"
var a=document.getElementById('a'), h=a.getAttributeNode('href'), d=a.getAttributeNode('download'), conversions=0;
h.value={toString:function(){conversions++;return '#attribute-value';}};
d.nodeValue='renamed.bin';
if(conversions!==1||a.getAttribute('href')!=='#attribute-value'||h.value!=='#attribute-value'||h.ownerElement!==a)throw 'attached href value';
if(a.attributes.getNamedItem('href')!==h||a.attributes.getNamedItem('download')!==d||d.value!=='renamed.bin')throw 'live map identity';
var incoming=document.createAttribute('download');incoming.value='adopted.bin';
var removed=a.setAttributeNode(incoming);
if(removed!==d||d.ownerElement!==null||d.value!=='renamed.bin'||incoming.ownerElement!==a||a.getAttributeNode('download')!==incoming)throw 'real Attr adoption';
if(a.removeAttributeNode(incoming)!==incoming||incoming.ownerElement!==null||incoming.value!=='adopted.bin'||a.hasAttribute('download'))throw 'real Attr removal';
a.setAttributeNS(null,'href','#null-namespace');
if(h!==a.getAttributeNodeNS(null,'href')||h.value!=='#null-namespace')throw 'null namespace shares the attached href';
"#);
    assert_eq!(doc.attr(node(&doc, "a"), "href"), Some("#null-namespace"));
    assert_eq!(doc.attr(node(&doc, "a"), "download"), None);
    assert!(!page.has_navigation_request());
}

#[test]
fn anchor_attributes_failed_conversion_and_retained_budget_leave_dom_and_attr_atomic() {
    let (mut doc, mut forms, mut page) = fixture(r#"
var a=document.getElementById('a'), h=a.getAttributeNode('href'), failures=0;
try{a.setAttribute('href',{toString:function(){failures++;throw 'original DOM conversion';}});}catch(e){}
try{h.value={toString:function(){failures++;throw 'original Attr conversion';}};}catch(e){}
if(failures!==2||a.getAttribute('href')!=='initial.html'||h.value!=='initial.html'||h.ownerElement!==a||a.getAttributeNode('href')!==h)throw 'conversion failure mutated source';
"#);
    let anchor = node(&doc, "a");
    let retained = page.state.retained;
    page.state.retained_limit = retained;
    for source in [
        "a.setAttribute('href','a-much-longer-original-anchor-target.html');",
        "h.value='a-second-longer-original-attribute-target.html';",
    ] {
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert_eq!(doc.attr(anchor, "href"), Some("initial.html"));
        assert_eq!(page.state.retained, retained);
        assert!(!page.has_navigation_request());
    }
    let outcome = page.console_eval(&mut doc, &mut forms,
        "if(h.value!=='initial.html'||h.ownerElement!==a||a.getAttributeNode('href')!==h)throw 'failed growth changed attached Attr';");
    assert_eq!(outcome.errors, 0, "{}", page.report());
}

#[test]
fn anchor_attributes_namespace_metadata_and_foreign_or_resource_guards_stay_separate() {
    let (mut doc, mut forms, mut page) = fixture(r#"
var a=document.getElementById('a'), other=document.getElementById('other'), resource=document.getElementById('resource'), sheet=document.getElementById('sheet'), foreign=document.getElementById('foreign');
var originalHref=a.getAttributeNode('href'), unsupportedIncoming=document.createAttribute('href');
a.setAttributeNS('urn:anchor-metadata','m:href','metadata-href');
a.setAttributeNS('urn:anchor-metadata','m:download','metadata-download');
a.setAttributeNS(null,'HREF','uppercase-metadata');
if(a.getAttribute('href')!=='initial.html'||a.getAttribute('download')!=='original.bin'||a.getAttributeNS('urn:anchor-metadata','href')!=='metadata-href'||a.getAttributeNS(null,'HREF')!=='uppercase-metadata')throw 'namespace metadata became real link data';
"#);
    // Unsupported native API guards report a job failure, not a catchable
    // author exception. Use independent real jobs and retain every refusal.
    let mut refused = 0;
    for source in [
        "other.setAttribute('href','other.html');",
        "other.setAttribute('download','other.bin');",
        "resource.setAttribute('src','replacement.js');",
        "foreign.setAttribute('href','foreign-replacement.html');",
        "foreign.setAttribute('download','foreign-replacement.bin');",
        "a.attributes.setNamedItem(unsupportedIncoming);",
    ] {
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "Rejected job: {source}\n{}", page.report());
        refused += 1;
        assert_eq!(doc.attr(node(&doc, "other"), "href"), None);
        assert_eq!(doc.attr(node(&doc, "other"), "download"), None);
        assert_eq!(doc.attr(node(&doc, "a"), "href"), Some("initial.html"));
        assert_eq!(doc.attr(node(&doc, "a"), "download"), Some("original.bin"));
        assert_eq!(doc.attr(node(&doc, "foreign"), "href"), Some("foreign.html"));
        assert_eq!(doc.attr(node(&doc, "foreign"), "download"), Some("foreign.bin"));
        assert_eq!(doc.attr(node(&doc, "resource"), "src"), Some("original.js"));
        assert_eq!(doc.attr(node(&doc, "sheet"), "href"), Some("original.css"));
        assert!(!page.has_navigation_request());
        assert!(page.take_dynamic_scripts().is_empty());
        assert!(page.take_dynamic_stylesheets().is_empty());
        let outcome = page.console_eval(&mut doc, &mut forms,
            "if(a.getAttributeNode('href')!==originalHref||originalHref.value!=='initial.html'||originalHref.ownerElement!==a||unsupportedIncoming.ownerElement!==null)throw 'rejected job changed Attr identity';");
        assert_eq!(outcome.errors, 0, "{}", page.report());
    }
    assert_eq!(refused, 6);
    assert_eq!(doc.attr(node(&doc, "a"), "href"), Some("initial.html"));
    assert_eq!(doc.attr(node(&doc, "foreign"), "href"), Some("foreign.html"));
    assert_eq!(doc.attr(node(&doc, "foreign"), "download"), Some("foreign.bin"));
    assert_eq!(doc.attr(node(&doc, "resource"), "src"), Some("original.js"));
    assert_eq!(doc.attr(node(&doc, "sheet"), "href"), Some("original.css"));
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let outcome=page.console_eval(&mut doc,&mut forms,"var sheetHref=sheet.getAttributeNode('href');sheet.setAttribute('href','replacement.css');if(sheet.getAttributeNode('href')!==sheetHref||sheetHref.value!=='replacement.css'||sheetHref.ownerElement!==sheet)throw 'stylesheet Attr identity';");
    assert_eq!(outcome.errors,0,"{}",page.report());
    let request=page.state.stylesheets.pending.front().unwrap();
    assert_eq!(request.target.display(),"https://anchor-attributes.test/replacement.css");
    assert!(request.error.is_none());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());

}
