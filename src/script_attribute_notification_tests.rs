//! Original attribute aliases, token mutations and inert namespace contracts.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut d = crate::html::parse(
        "<html><body><div id=mount data-note=old class=old></div></body></html>",
    );
    d.scripting_enabled = true;
    let mut f = Forms::new(&d);
    let mut p = Page::with_limits(crate::javascript::limits::BROWSER);
    assert_eq!(p.start(&mut d, &mut f, &[]).errors, 0);
    (d, f, p)
}
fn run(d: &mut Document, f: &mut Forms, p: &mut Page, s: &str) {
    let out = p.console_eval(d, f, s);
    assert_eq!(out.errors, 0, "{}", p.report());
}
#[test]
fn attribute_notifications_real_alias_class_and_reflection_writes_have_old_values() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('mount'),attr=node.getAttributeNode('data-note'),obs=new MutationObserver(function(){throw 'drained attribute aliases';});obs.observe(node,{attributes:true,attributeOldValue:true});attr.value='changed';node.classList.add('built');node.classList.value='replaced';node.className='native';node.id='renamed';var records=obs.takeRecords();if(records.length!==5||records[0].target!==node||records[0].attributeName!=='data-note'||records[0].oldValue!=='old'||records[1].attributeName!=='class'||records[1].oldValue!=='old'||records[2].oldValue!=='old built'||records[3].oldValue!=='replaced'||records[4].attributeName!=='id'||records[4].oldValue!=='mount'||attr.ownerElement!==node||node.getAttribute('class')!=='native'||document.getElementById('renamed')!==node)throw 'actual alias token and reflected records';
var incoming=document.createAttribute('data-note');incoming.value='incoming';if(obs.takeRecords().length)throw 'detached Attr notified Element';if(node.setAttributeNode(incoming)!==attr||attr.ownerElement!==null)throw 'real Attr replacement';node.removeAttributeNode(incoming);incoming.value='detached';records=obs.takeRecords();if(records.length!==2||records[0].oldValue!=='changed'||records[1].oldValue!=='incoming'||records[1].target!==node||incoming.ownerElement!==null)throw 'actual Attr adoption removal records';
"#,
    );
}
#[test]
fn attribute_notifications_inert_svg_namespace_creation_alias_removal_and_import_are_real() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var owner=document.implementation.createHTMLDocument('icons'),ns='http://www.w3.org/2000/svg',xl='http://www.w3.org/1999/xlink',use=owner.createElementNS(ns,'use'),obs=new MutationObserver(function(){throw 'drained namespace records';});obs.observe(use,{attributes:true,attributeOldValue:true});use.setAttributeNS(xl,'xlink:href','#first');var attr=use.getAttributeNodeNS(xl,'href');attr.value='#second';use.removeAttributeNS(xl,'href');var records=obs.takeRecords();if(records.length!==3||records[0].target!==use||records[0].attributeName!=='href'||records[0].attributeNamespace!==xl||records[0].oldValue!==null||records[1].oldValue!=='#first'||records[2].oldValue!=='#second'||attr.ownerElement!==null||attr.ownerDocument!==owner||attr.namespaceURI!==xl||attr.prefix!=='xlink'||attr.localName!=='href'||attr.value!=='#second'||use.getAttributeNS(xl,'href')!==null)throw 'real inert expanded attributes';
use.setAttributeNS(null,'data-role','icon');var copy=document.importNode(use,true);if(copy.ownerDocument!==document||copy.namespaceURI!==ns||copy.getAttributeNS(null,'data-role')!=='icon'||use.ownerDocument!==owner)throw 'real namespace data import';obs.takeRecords();
"#,
    );
}
#[test]
fn attribute_notifications_namespace_canonical_conversion_brand_and_errors_are_ordered() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var owner=document.implementation.createHTMLDocument('metadata'),node=owner.createElement('div'),native=Element.prototype.setAttributeNS,order='',obs=new MutationObserver(function(){throw 'drained namespace metadata';});if(native.length!==3||native.name!=='setAttributeNS'||Element.prototype.getAttributeNS.length!==2)throw 'canonical namespace method metadata';obs.observe(node,{attributes:true,attributeOldValue:true});native.call(node,{toString:function(){order+='ns';return 'urn:original';}},{toString:function(){order+='name';return 'm:key';}},{toString:function(){order+='value';return 'one';}});if(order!=='nsnamevalue'||node.getAttributeNS('urn:original','key')!=='one')throw 'ordered namespace conversion';var records=obs.takeRecords();if(records.length!==1||records[0].attributeName!=='key'||records[0].attributeNamespace!=='urn:original'||records[0].oldValue!==null)throw 'namespace metadata record';
var failed=false;try{native.call({}, {toString:function(){order+='bad';return null;}},'key','value');}catch(e){failed=e instanceof TypeError;}if(!failed||order!=='nsnamevalue')throw 'brand before namespace conversion';failed=false;try{native.call(node,null,'m:bad','value');}catch(e){failed=e instanceof DOMException&&e.name==='NamespaceError';}if(!failed||node.getAttribute('m:bad')!==null||obs.takeRecords().length)throw 'typed namespace refusal or partial publication';
"#,
    );
}

#[test]
fn attribute_notifications_alias_conversion_reentry_filter_and_refusal_are_atomic() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('mount'),attr=node.getAttributeNode('data-note'),obs=new MutationObserver(function(){throw 'drained alias conversion';});obs.observe(node,{attributes:true,attributeOldValue:true,attributeFilter:['data-note']});var calls=0;attr.value={toString:function(){calls++;node.setAttribute('data-note','during');return 'after';}};var records=obs.takeRecords();if(calls!==1||records.length!==2||records[0].oldValue!=='old'||records[1].oldValue!=='during'||attr.value!=='after')throw 'alias conversion before native preflight';node.setAttributeNS('urn:metadata','m:data-note','filtered');if(obs.takeRecords().length)throw 'namespace filter ignored real URI';
"#,
    );
    let retained = p.state.retained;
    let limit = p.state.retained_limit;
    p.state.retained_limit = retained;
    let generation = p.state.dom_commits;
    let out = p.console_eval(&mut d, &mut f, "attr.value='blocked longer payload';");
    assert_eq!(out.errors, 1);
    assert_eq!(p.state.dom_commits, generation);
    p.state.retained_limit = limit;
    run(&mut d,&mut f,&mut p,"if(attr.value!=='after'||obs.takeRecords().length)throw 'refusal leaked attribute record';attr.value={toString:function(){node.removeAttribute('data-note');return 'detached';}};var records=obs.takeRecords();if(records.length!==1||records[0].oldValue!=='after'||attr.ownerElement!==null||attr.value!=='detached'||node.getAttribute('data-note')!==null)throw 'coercion detached Attr retained fake owner';");
}
