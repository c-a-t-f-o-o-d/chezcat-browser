use super::*;

#[test]
fn html_border_reflects_real_strings_and_legacy_null_only_on_its_brands() {
    let (doc, _, page, outcome) = page("<img id=i border=' 05px '><table id=t border=' 05px '></table><p id=p>ordinary</p>", &[r#"
if(document.getElementById('i').border!==' 05px '||document.getElementById('t').border!==' 05px ')throw 'raw reflection';
for(var tag of ['img','object','table']){
 var e=document.createElement(tag);if(e.border!==''||!('border' in e)||e.hasAttribute('border'))throw 'empty default';
 e.border=null;if(e.getAttribute('border')!==(tag==='table'?'null':''))throw 'original null';
 var attr=e.getAttributeNode('border');e.border=undefined;if(e.border!=='undefined'||attr.value!=='undefined')throw 'undefined';
 e.border={toString:function(){return null;}};if(e.border!=='null')throw 'object-produced null';
 e.setAttribute('border',null);if(e.border!=='null')throw 'raw setter conversion';
 var log='';e.border={toString:function(){log+='s';return 'raw';},valueOf:function(){log+='v';return 1;}};if(log!=='s'||e.border!=='raw'||attr.value!=='raw'||e.getAttributeNode('border')!==attr)throw 'coercion/identity';
 var caught=false;try{e.border=Symbol();}catch(x){caught=x.name==='TypeError';}if(!caught||e.border!=='raw')throw 'Symbol atomic';
 var marker={},caught=false;try{e.border={toString:function(){throw marker;}};}catch(x){caught=x===marker;}if(!caught||e.border!=='raw')throw 'throw atomic';
 e.removeAttribute('border');if(e.border!==''||attr.ownerElement!==null||attr.value!=='raw')throw 'detach';
}
for(var e of [document.createElement('input'),document.createElement('embed'),document.createElement('iframe'),document.createElement('div'),document.createTextNode('text')]){if('border' in e||e.border!==undefined)throw 'wrong brand';}
if('border' in document)throw 'Document brand';document.title='border-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "border-ok");
}

#[test]
fn html_border_budget_and_unknown_namespace_failures_preserve_existing_attributes() {
    let (mut doc, mut forms, mut page, outcome) = page("<p>ordinary</p>", &["var image=document.createElement('img');image.border='old';"]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let node = doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "img")).unwrap();
    page.begin_job();
    page.state.retained_limit = page.state.retained;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    assert!(bridge.set(node_handle(node), "border", Value::String("longer".into())).is_err());
    assert_eq!(bridge.doc.attr(node, "border"), Some("old"));
    bridge.state.copy_work = 0;
    assert!(bridge.get(node_handle(node), "border").is_err());
    assert_eq!(bridge.doc.attr(node, "border"), Some("old"));
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    bridge.doc.nodes[node].namespace = crate::html::Namespace::UnknownForeign;
    assert!(bridge.get(node_handle(node), "border").unwrap_err().contains("namespace"));
}

#[test]
fn html_spacing_reflects_unsigned_attributes_on_real_brands_and_keeps_attr_identity() {
    let (doc, _, page, outcome) = page("<p id=p>ordinary</p>", &[r#"
for(var tag of ['img','input','object','embed','marquee']){
 var e=document.createElement(tag);if(e.vspace!==0||e.hspace!==0||!('vspace' in e)||e.hasAttribute('vspace'))throw 'default/brand';
 e.setAttribute('vspace',' +12px');var a=e.getAttributeNode('vspace');if(e.vspace!==12||a.value!==' +12px')throw 'attribute parser';
 for(var pair of [[1.9,'1'],[-1,'0'],[-4294967295,'1'],[4294967303,'7'],[NaN,'0'],[Infinity,'0']]){
  e.vspace=pair[0];if(e.getAttribute('vspace')!==pair[1]||a.value!==pair[1]||e.getAttributeNode('vspace')!==a)throw 'unsigned reflection';
 }
 e.vspace=2147483647;if(e.vspace!==2147483647)throw 'max';
 for(var text of ['-1','2147483648','garbage']){e.setAttribute('vspace',text);if(e.vspace!==0)throw 'invalid attribute';}
 e.vspace='12px';if(e.getAttribute('vspace')!=='0')throw 'number conversion';
 var calls=0;e.hspace={valueOf:function(){calls++;return 17;}};if(calls!==1||e.hspace!==17||e.getAttribute('hspace')!=='17')throw 'ordered numeric conversion';
 var caught=false;try{e.hspace=Symbol();}catch(x){caught=x.name==='TypeError';}if(!caught||e.hspace!==17)throw 'Symbol atomic';
 e.removeAttribute('vspace');if(e.vspace!==0||a.ownerElement!==null||a.value!=='0')throw 'removal';
}
for(var e of [document.createElement('div'),document.createElement('iframe'),document.createTextNode('text')]){
 if('vspace' in e||e.vspace!==undefined)throw 'wrong brand';
}
if('vspace' in document)throw 'Document brand';document.title='spacing-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "spacing-ok");
}

#[test]
fn html_spacing_work_and_retention_failures_do_not_mutate_the_attribute() {
    let (mut doc, mut forms, mut page, outcome) = page("<p>ordinary</p>", &["var image=document.createElement('img');image.vspace=7;"]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let node = doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "img")).unwrap();
    page.begin_job();
    page.state.retained_limit = page.state.retained;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    assert!(bridge.set(node_handle(node), "vspace", Value::Number(1000.0)).is_err());
    assert_eq!(bridge.doc.attr(node, "vspace"), Some("7"));
    bridge.state.host_work = 0;
    assert!(bridge.get(node_handle(node), "vspace").is_err());
    assert_eq!(bridge.doc.attr(node, "vspace"), Some("7"));
}

#[test]
fn attr_namespace_lookup_keeps_exact_local_names_raw_colons_and_stable_identity() {
    let (doc, _, page, outcome) = page("<p id=p TITLE=old x:y=colon xmlns='urn:raw' xmlns:x='urn:x' null=n undefined=u></p>", &[r#"
var p=document.getElementById('p'),a=p.getAttributeNode('TITLE');
if(p.getAttributeNodeNS(null,'title')!==a||p.getAttributeNodeNS(undefined,'title')!==a||p.getAttributeNodeNS('','title')!==a||p.getAttributeNodeNS(null,'TITLE')!==null)throw 'namespace/case/identity';
if(p.getAttributeNodeNS('urn:x','title')!==null||p.getAttributeNodeNS('http://www.w3.org/1999/xhtml','title')!==null)throw 'unqualified attrs';
var colon=p.getAttributeNodeNS(null,'x:y');if(colon!==p.getAttributeNode('x:y')||colon.localName!=='x:y'||colon.prefix!==null||colon.namespaceURI!==null||p.getAttributeNodeNS(null,'y')!==null||p.getAttributeNodeNS('urn:x','y')!==null)throw 'raw colon';
if(p.getAttributeNodeNS(null,'xmlns').namespaceURI!==null||p.getAttributeNodeNS('http://www.w3.org/2000/xmlns/','x')!==null||p.getAttributeNodeNS(null,'xmlns:x').localName!=='xmlns:x')throw 'raw xmlns';
if(p.getAttributeNodeNS(null,null).value!=='n'||p.getAttributeNodeNS(undefined,undefined).value!=='u'||p.getAttributeNodeNS(null,'bad name')!==null||p.getAttributeNodeNS(null,'')!==null)throw 'local DOMString/query syntax';
p.setAttribute('title','live');if(p.getAttributeNodeNS(null,'title')!==a||a.value!=='live')throw 'live attr';
p.removeAttributeNode(a);p.setAttribute('title','fresh');if(p.getAttributeNodeNS(null,'title')===a||a.value!=='live'||a.ownerElement!==null)throw 'detach/readd';
var get=p.getAttributeNodeNS;if(get!==p.getAttributeNodeNS||get.call(p,null,'title').value!=='fresh')throw 'method borrowing';
document.title='attr-namespace-lookup-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "attr-namespace-lookup-ok");
}

#[test]
fn attr_namespace_conversion_order_nullability_failures_and_reentrancy_are_real() {
    let (doc, _, page, outcome) = page("<p id=p title=old></p>", &[r#"
var p=document.getElementById('p'),log='',marker={},old=p.getAttributeNode('title');
var ns={toString:function(){log+='N';return '';},valueOf:function(){throw 'wrong hint';}},name={toString:function(){log+='L';return 'title';},valueOf:function(){throw 'wrong hint';}};
if(p.getAttributeNodeNS(ns,name)!==old||log!=='NL')throw 'ordered once conversion';
log='';if(p.getAttributeNodeNS({toString:function(){log+='N';return null;}},name)!==null||log!=='NL')throw 'object-produced null';
log='';if(p.getAttributeNodeNS({toString:function(){log+='N';return undefined;}},name)!==null||log!=='NL')throw 'object-produced undefined';
log='';var rejected=0;try{p.getAttributeNodeNS(ns);}catch(e){if(e.name==='TypeError')rejected++;}if(log!=='')throw 'arity before conversion';
try{p.getAttributeNodeNS(Symbol(),name);}catch(e){if(e.name==='TypeError')rejected++;}if(log!=='')throw 'namespace Symbol first';
try{p.getAttributeNodeNS('urn:x',Symbol());}catch(e){if(e.name==='TypeError')rejected++;}
var caught=false;try{p.getAttributeNodeNS({toString:function(){throw marker;}},name);}catch(e){caught=e===marker;}if(!caught||log!=='')throw 'namespace throw identity';
caught=false;try{p.getAttributeNodeNS(null,{toString:function(){throw marker;}});}catch(e){caught=e===marker;}if(!caught||old.value!=='old')throw 'name throw identity';
var fresh=p.getAttributeNodeNS(null,{toString:function(){p.removeAttribute('title');p.setAttribute('title','after');return 'title';}});
if(fresh===old||fresh.value!=='after'||old.ownerElement!==null||old.value!=='old'||rejected!==3)throw 'post-coercion lookup';
var calls=0,arg={toString:function(){calls++;return 'title';}};
for(var bad of [{},window,document,p.style,p.dataset,document.createTextNode('x')]){try{p.getAttributeNodeNS.call(bad,arg,arg);}catch(e){if(e.name==='TypeError')rejected++;}}
if(calls!==0||rejected!==9)throw 'brand before arguments';
document.title='attr-namespace-conversion-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "attr-namespace-conversion-ok");
}

#[test]
fn attr_namespace_misses_do_not_consume_identity_capacity_or_retained_bytes() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p title=old></p>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let p = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("p")).unwrap();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for i in 0..256 {
        let name = format!("data-{i}");
        bridge.call(node_handle(p), "setAttribute", &[Value::String(name.clone()), Value::String("v".into())]).unwrap();
        bridge.call(node_handle(p), "getAttributeNode", &[Value::String(name)]).unwrap();
    }
    let before = bridge.state.retained;
    for (namespace, name) in [(Value::Null, "missing"), (Value::Null, "TITLE"), (Value::String("urn:x".into()), "title")] {
        assert!(matches!(bridge.call(node_handle(p), "getAttributeNodeNS", &[namespace, Value::String(name.into())]).unwrap(), Value::Null));
    }
    assert_eq!(bridge.state.attributes.len(), 256);
    assert_eq!(bridge.state.retained, before);
    let existing = bridge.call(node_handle(p), "getAttributeNodeNS", &[Value::Null, Value::String("data-0".into())]).unwrap();
    assert!(matches!(existing, Value::Host(handle) if script_attr::index(handle) == Some(0)));
    assert!(bridge.call(node_handle(p), "getAttributeNodeNS", &[Value::Null, Value::String("title".into())]).unwrap_err().contains("256"));
    assert_eq!(bridge.state.retained, before);
}

#[test]
fn attr_namespace_foreign_context_and_failed_new_record_keep_existing_boundaries() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p title=old></p><svg><g id=f title=unknown></g></svg><template><i id=inert></i></template>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let find = |id: &str| (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(id)).unwrap();
    let p = find("p"); let foreign = find("f"); let inert = find("inert");
    let invalid = [node_handle(0), node_handle(doc.nodes.len()+99), node_handle(inert)];
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for handle in invalid {
        assert!(!bridge.accepts_receiver(node_handle(p), "getAttributeNodeNS", handle));
        assert!(bridge.method(handle, "getAttributeNodeNS").is_none());
    }
    for namespace in [Value::Null, Value::String("urn:x".into())] {
        assert!(bridge.call(node_handle(foreign), "getAttributeNodeNS", &[namespace, Value::String("title".into())]).unwrap_err().contains("foreign"));
    }
    let before = bridge.state.retained;
    bridge.state.retained_limit = before;
    assert!(bridge.call(node_handle(p), "getAttributeNodeNS", &[Value::Null, Value::String("title".into())]).is_err());
    assert!(bridge.state.attributes.is_empty());
    assert_eq!(bridge.state.retained, before);
}

#[test]
fn tab_index_getter_uses_tag_defaults_and_physical_summary_parent() {
    let (doc, _, page, outcome) = page("<html><body><input id=hidden type=hidden><input id=disabled disabled><a id=a>No href</a><div id=d></div><details><i>lead</i><summary id=first>First</summary><summary id=second>Second</summary></details><summary id=outside>Outside</summary></body></html>", &[r#"
for(var id of ['hidden','disabled','a']){if(document.getElementById(id).tabIndex!==0)throw 'default historical zero';}
for(var id of ['d','second','outside']){if(document.getElementById(id).tabIndex!==-1)throw 'default minus one';}
if(document.getElementById('first').tabIndex!==0||'tabIndex' in document||'tabIndex' in document.createTextNode('x'))throw 'interface/default';
for(var name of ['a','area','button','frame','iframe','input','object','select','textarea']){var n=document.createElement(name);if(!('tabIndex' in n)||n.tabIndex!==0)throw 'detached tag default';}
var details=document.createElement('details'),first=document.createElement('summary'),second=document.createElement('summary');
details.appendChild(first);details.appendChild(second);if(first.tabIndex!==0||second.tabIndex!==-1)throw 'detached summary order';details.removeChild(first);if(first.tabIndex!==-1||second.tabIndex!==0)throw 'summary mutation';
document.title='tab-index-defaults-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "tab-index-defaults-ok");
}

#[test]
fn tab_index_getter_and_native_control_order_share_the_authored_integer_parser() {
    for (text, expected) in [(" +12junk", Some(12)), ("1e3", Some(1)), ("0x10", Some(0)),
        ("-0", Some(0)), ("2147483647", Some(i32::MAX)), ("-2147483648", Some(i32::MIN)),
        ("2147483648", None), ("-2147483649", None), ("+", None), ("\u{a0}2", None), ("\x0b2", None)] {
        let markup = format!("<input id=c tabindex='{text}'><div id=d tabindex='{text}'></div>");
        let source = format!("if(document.getElementById('c').tabIndex!=={}||document.getElementById('d').tabIndex!=={})throw 'integer prefix';", expected.unwrap_or(0), expected.unwrap_or(-1));
        let (doc, forms, page, outcome) = page(&markup, &[&source]);
        assert_eq!(outcome.errors, 0, "{text:?}: {}", page.report());
        let control = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("c")).unwrap();
        assert_eq!(forms.controls[&control].tab_index, expected.unwrap_or(0));
    }
}

#[test]
fn tab_index_summary_context_keeps_foreign_provenance_after_moves() {
    let (mut doc, mut forms, mut page, outcome) = page("<html><body><svg><details id=fd></details><summary id=fs></summary></svg></body></html>", &[r#"
var foreign=document.getElementById('fd'),s=document.createElement('summary');s.id='known-parent';foreign.appendChild(s);
var d=document.createElement('details'),bad=document.getElementById('fs'),x=document.createElement('summary');x.id='known-sibling';d.appendChild(bad);d.appendChild(x);
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let nodes: Vec<_> = ["known-parent", "known-sibling"].iter().map(|id| (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(*id)).unwrap()).collect();
    {
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        for node in nodes {
            assert!(bridge.get(node_handle(node), "tabIndex").unwrap_err().contains("unknown"));
        }
    }
    let outcome = page.console_eval(&mut doc, &mut forms, "d.removeChild(bad);if(x.tabIndex!==0)throw 'known summary after removal';");
    assert_eq!(outcome.errors, 0, "{}", page.report());
}

#[test]
fn tab_index_and_zero_offset_reads_keep_invalid_foreign_write_and_work_boundaries() {
    let (mut doc, mut forms, mut page, outcome) = page("<html><body><span id=connected>Text</span><svg><g id=foreign></g></svg><template><i id=inert></i></template></body></html>", &[r#"
var detached=document.createElement('input');if(detached.offsetLeft!==0||detached.offsetTop!==0||document.body.offsetLeft!==0||document.body.offsetTop!==0)throw 'defined zero offsets';
var effects=0,rhs={valueOf:function(){effects++;return 2;},toString:function(){effects++;return '2';}};
if((detached.offsetLeft=rhs)!==rhs||detached.offsetLeft!==0||effects!==0)throw 'readonly offset';
var caught=false;try{(function(){'use strict';detached.offsetTop=7;})();}catch(e){caught=e.name==='TypeError';}if(!caught||detached.offsetTop!==0)throw 'strict offset';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let find = |id: &str| (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(id)).unwrap();
    let connected = find("connected"); let foreign = find("foreign"); let inert = find("inert");
    let invalid = [node_handle(0), node_handle(doc.nodes.len()+99), node_handle(inert), node_handle(foreign)];
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for handle in invalid {
        for property in ["tabIndex", "offsetLeft", "offsetTop"] {
            assert!(bridge.get(handle, property).is_err());
            assert!(bridge.has(handle, property).is_err());
            assert!(!bridge.readonly_property(handle, property));
        }
    }
    let diagnostic = bridge.get(node_handle(connected), "offsetLeft").unwrap_err();
    assert!(diagnostic.contains("class=span; connected=true; created=false; geometry-attached=false"), "{diagnostic}");
    assert!(diagnostic.len() < 300);
    assert!(bridge.set(node_handle(connected), "tabIndex", Value::Number(3.0)).is_err());
    assert_eq!(bridge.doc.attr(connected, "tabindex"), None);
    bridge.state.copy_work = 0;
    assert!(bridge.get(node_handle(connected), "tabIndex").is_err());
    bridge.state.host_work = 0;
    assert!(bridge.get(node_handle(connected), "offsetTop").is_err());
}

#[test]
fn attr_identity_ownership_detach_and_live_values_follow_existing_mutations() {
    let (doc, _, page, outcome) = page("<div id=p title=old data-x=one style='color:red'></div><div id=q></div>", &[r#"
var p=document.getElementById('p'),q=document.getElementById('q'),a=p.getAttributeNode('TITLE'),x=p.getAttributeNode('data-x'),s=p.getAttributeNode('style');
if(a!==p.getAttributeNode('title')||p.getAttributeNode('missing')!==null)throw 'identity/absence';
if(a.name!=='title'||a.localName!=='title'||a.nodeName!=='title'||a.value!=='old'||a.nodeType!==2||a.ATTRIBUTE_NODE!==2||a.ownerElement!==p||a.ownerDocument!==document||a.namespaceURI!==null||a.prefix!==null||!a.specified||a.isConnected)throw 'metadata';
if(a.parentNode!==null||a.firstChild!==null||a.lastChild!==null||a.nextSibling!==null||a.previousSibling!==null||a.parentElement!==null)throw 'Attr topology';
if(!a.isDefaultNamespace('http://www.w3.org/1999/xhtml')||a.isDefaultNamespace(null))throw 'attached namespace';
p.setAttribute('title','new');p.dataset.x='two';p.style.color='blue';
if(a.value!=='new'||a.nodeValue!=='new'||a.textContent!=='new'||x.value!=='two'||s.value.indexOf('blue')<0||a!==p.getAttributeNode('title'))throw 'live mutations';
var wrong=false;try{q.removeAttributeNode(a);}catch(e){wrong=e.name==='NotFoundError';}if(!wrong||a.ownerElement!==p)throw 'wrong owner';
var remove=q.removeAttributeNode;if(remove.call(p,a)!==a||a.ownerElement!==null||a.value!=='new'||p.hasAttribute('title')||p.getAttributeNode('title')!==null)throw 'detach';
if(!a.isDefaultNamespace(null)||a.isDefaultNamespace('http://www.w3.org/1999/xhtml'))throw 'detached namespace';
p.setAttribute('title','fresh');if(p.getAttributeNode('title')===a||a.value!=='new')throw 'readd identity';
var missing=false;try{p.removeAttributeNode(a);}catch(e){missing=e.name==='NotFoundError';}if(!missing)throw 'detached removal';
p.removeAttribute('data-x');p.setAttribute('data-x','three');if(x.ownerElement!==null||x.value!=='two'||p.getAttributeNode('data-x')===x)throw 'string removal lifecycle';
document.title='attr-lifecycle-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "attr-lifecycle-ok");
}

#[test]
fn attr_value_aliases_coerce_once_preserve_nullability_and_do_not_forge_brands() {
    let (doc, _, page, outcome) = page("<p id=p title=old></p>", &[r#"
var p=document.getElementById('p'),a=p.getAttributeNode('title'),calls=0;
var value={toString:function(){calls++;return 'once';},valueOf:function(){throw 'wrong hint';}};
a.value=value;if(calls!==1||a.value!=='once'||p.getAttribute('title')!=='once')throw 'attached value';
a.nodeValue=null;if(a.value!=='')throw 'nullable nodeValue';a.textContent=undefined;if(a.value!=='')throw 'nullable textContent';
a.value=null;if(a.value!=='null')throw 'ordinary value';a.nodeValue={toString:function(){return null;}};if(a.value!=='null')throw 'object-produced null';
var marker={},caught=false;try{a.value={toString:function(){throw marker;}};}catch(e){caught=e===marker;}if(!caught||a.value!=='null')throw 'failed value';
var rejected=0;try{a.value=Symbol();}catch(e){if(e.name==='TypeError')rejected++;}
for(var v of [null,{},p,p.style,document,document.createTextNode('x')]){try{p.removeAttributeNode(v);}catch(e){if(e.name==='TypeError')rejected++;}}
try{p.getAttributeNode();}catch(e){if(e.name==='TypeError')rejected++;}
try{p.removeAttributeNode();}catch(e){if(e.name==='TypeError')rejected++;}
var effect=0;for(var v of [{},window,p.style]){try{p.getAttributeNode.call(v,{toString:function(){effect++;return 'title';}});}catch(e){if(e.name==='TypeError')rejected++;}}
if(rejected!==12||effect!==0||a.ownerElement!==p)throw 'brand/arity/order';
var rhs={toString:function(){throw 'readonly coercion';}},result=(a.name=rhs);if(result!==rhs||a.name!=='title')throw 'readonly result';
caught=false;try{(function(){'use strict';a.ownerElement=p;})();}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'strict readonly';
p.removeAttributeNode(a);a.value=value;if(a.value!=='once'||calls!==2||p.hasAttribute('title'))throw 'detached write';
document.title='attr-conversion-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "attr-conversion-ok");
}

#[test]
fn attr_removal_and_setters_keep_resource_control_and_inert_policy() {
    let (mut doc, mut forms, mut page, outcome) = page("<body><input id=p value=initial onclick='return false'><img id=image src='https://example.test/x.png'><a id=link href='https://example.test/'>link</a><template><i id=inert title=x></i></template></body>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let nodes: Vec<_> = ["p", "image", "link", "inert"].iter().map(|id| (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(*id)).unwrap()).collect();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for (node, name) in [(nodes[0], "value"), (nodes[0], "onclick"), (nodes[1], "src"), (nodes[2], "href")] {
        let value = bridge.call(node_handle(node), "getAttributeNode", &[Value::String(name.into())]).unwrap();
        let Value::Host(handle) = value else { panic!("expected real Attr") };
        let original = bridge.doc.attr(node, name).unwrap().to_owned();
        let before = bridge.state.retained;
        assert!(bridge.call(node_handle(node), "removeAttributeNode", &[Value::Host(handle)]).is_err());
        assert!(bridge.set(handle, "value", Value::String("changed".into())).is_err());
        assert_eq!(bridge.doc.attr(node, name), Some(original.as_str()));
        assert_eq!(bridge.state.retained, before);
        assert_eq!(bridge.state.attributes[script_attr::index(handle).unwrap()].owner, Some(node));
    }
    assert!(bridge.call(node_handle(nodes[3]), "getAttributeNode", &[Value::String("title".into())]).is_err());
}

#[test]
fn attr_identity_limit_and_failed_materialization_are_atomic_and_keep_live_identity() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p title=old></p>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let p = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("p")).unwrap();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let before = bridge.state.retained;
    bridge.state.retained_limit = before;
    assert!(bridge.call(node_handle(p), "getAttributeNode", &[Value::String("title".into())]).is_err());
    assert_eq!(bridge.state.retained, before);
    assert!(bridge.state.attributes.is_empty());
    bridge.state.retained_limit = before + MAX_EXTRA_BYTES;
    for i in 0..256 {
        let name = format!("data-{i}");
        bridge.call(node_handle(p), "setAttribute", &[Value::String(name.clone()), Value::String("x".into())]).unwrap();
        let value = bridge.call(node_handle(p), "getAttributeNode", &[Value::String(name)]).unwrap();
        assert!(matches!(value, Value::Host(handle) if script_attr::index(handle) == Some(i)));
    }
    let retained = bridge.state.retained;
    assert!(bridge.call(node_handle(p), "getAttributeNode", &[Value::String("title".into())]).unwrap_err().contains("256"));
    assert_eq!(bridge.state.attributes.len(), 256);
    assert_eq!(bridge.state.retained, retained);
    let existing = bridge.call(node_handle(p), "getAttributeNode", &[Value::String("data-0".into())]).unwrap();
    assert!(matches!(existing, Value::Host(handle) if script_attr::index(handle) == Some(0)));
}

#[test]
fn attr_detach_retains_exact_value_bytes_and_budget_failures_publish_no_partial_change() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p title=original></p>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let p = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("p")).unwrap();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let value = bridge.call(node_handle(p), "getAttributeNode", &[Value::String("title".into())]).unwrap();
    let Value::Host(attr) = value else { panic!("expected Attr") };
    let before = bridge.state.retained;
    bridge.state.host_work = 0;
    assert!(bridge.call(node_handle(p), "removeAttributeNode", &[Value::Host(attr)]).is_err());
    assert_eq!(bridge.doc.attr(p, "title"), Some("original"));
    assert_eq!(bridge.state.attributes[script_attr::index(attr).unwrap()].owner, Some(p));
    assert_eq!(bridge.state.retained, before);
    bridge.state.host_work = bridge.state.limits.host_work;
    bridge.state.copy_work = 0;
    assert!(bridge.call(node_handle(p), "removeAttributeNode", &[Value::Host(attr)]).is_err());
    assert_eq!(bridge.doc.attr(p, "title"), Some("original"));
    assert_eq!(bridge.state.retained, before);
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    bridge.call(node_handle(p), "removeAttributeNode", &[Value::Host(attr)]).unwrap();
    assert_eq!(bridge.state.retained, before - "title".len());
    assert_eq!(bridge.state.attributes[script_attr::index(attr).unwrap()].value, "original");
    bridge.set(attr, "value", Value::String("longer-value".into())).unwrap();
    assert_eq!(bridge.state.retained, before - "title".len() - "original".len() + "longer-value".len());
    for invalid in [768 + 1, NODE_BASE - 1] {
        assert!(bridge.get(invalid, "value").is_err());
        assert!(bridge.has(invalid, "value").is_err());
        assert!(!bridge.readonly_property(invalid, "name"));
        assert!(bridge.set(invalid, "value", Value::String("x".into())).is_err());
    }
}

#[test]
fn attr_borrowed_methods_reject_invalid_inert_and_non_element_receivers_before_coercion() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p title=old></p><template><i id=inert title=x></i></template>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let inert = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("inert")).unwrap();
    for handle in [node_handle(0), node_handle(inert), node_handle(doc.nodes.len()+99)] {
        page.runtime.define_global("invalidAttrReceiver", Value::Host(handle)).unwrap();
        let outcome = page.console_eval(&mut doc, &mut forms, "var calls=0,p=document.getElementById('p'),arg={toString:function(){calls++;return 'title';}},rejected=0;try{p.getAttributeNode.call(invalidAttrReceiver,arg);}catch(e){if(e.name==='TypeError')rejected++;}try{p.removeAttributeNode.call(invalidAttrReceiver,arg);}catch(e){if(e.name==='TypeError')rejected++;}if(calls!==0||rejected!==2)throw 'invalid native brand';");
        assert_eq!(outcome.errors, 0, "{}", page.report());
    }
    assert!(page.state.attributes.is_empty());
}

#[test]
fn attr_coercion_reentrancy_and_promise_resolution_preserve_original_identity() {
    let (doc, _, page, outcome) = page("<p id=p title=old></p>", &[r#"
var p=document.getElementById('p'),a=p.getAttributeNode('title'),effects=0;
a.value={toString:function(){effects++;p.removeAttributeNode(a);p.setAttribute('title','replacement');return 'detached-value';}};
var fresh=p.getAttributeNode('title');
if(effects!==1||a.ownerElement!==null||a.value!=='detached-value'||fresh===a||fresh.value!=='replacement'||fresh.ownerElement!==p)throw 'reentrant owner snapshot';
var name={toString:function(){effects++;p.removeAttribute('title');p.setAttribute('title','after');return 'TITLE';}};
var after=p.getAttributeNode(name);if(effects!==2||after===fresh||fresh.ownerElement!==null||fresh.value!=='replacement'||after.value!=='after')throw 'lookup conversion order';
Promise.resolve(a).then(function(value){if(value!==a||value.value!=='detached-value'||value.ownerElement!==null)throw 'Promise Attr identity';document.title='attr-reentrancy-ok';});
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "attr-reentrancy-ok");
}

#[test]
fn default_namespace_tracks_real_html_nodes_and_physical_text_parents() {
    let (doc, _, page, outcome) = page("<html><body><div id=p xmlns='urn:other'>text</div><b id=empty xmlns=''></b></body></html>", &[r#"
var uri='http://www.w3.org/1999/xhtml',p=document.getElementById('p'),t=document.createTextNode('x');
var e=document.createElement('svg'),image=new Image(),nodes=[document,p,p.firstChild,e,image];
for(var n of nodes){if(!('isDefaultNamespace' in n)||!n.isDefaultNamespace(uri)||n.isDefaultNamespace(null)||n.isDefaultNamespace(undefined)||n.isDefaultNamespace('')||n.isDefaultNamespace('urn:other'))throw 'HTML namespace';}
if(!t.isDefaultNamespace(null)||!t.isDefaultNamespace(undefined)||!t.isDefaultNamespace('')||t.isDefaultNamespace(uri))throw 'unparented text';
e.appendChild(t);if(!t.isDefaultNamespace(uri)||t.isDefaultNamespace(null))throw 'detached parent';
e.removeChild(t);if(!t.isDefaultNamespace(null)||t.isDefaultNamespace(uri))throw 'removed text';
if(!document.getElementById('empty').isDefaultNamespace(uri))throw 'xmlns attribute';
var check=p.isDefaultNamespace;
if(!check.call(document,uri)||!check.call(t,null)||!document.isDefaultNamespace.call(p,uri))throw 'Node borrowing';
document.title='namespace-topology-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "namespace-topology-ok");
}

#[test]
fn default_namespace_nullable_conversion_and_receiver_failures_preserve_effect_order() {
    let (doc, _, page, outcome) = page("<p id=p>coercion</p>", &[r#"
var p=document.getElementById('p'),t=document.createTextNode('x'),calls='',marker={};
var uri={toString:function(){calls+='s';return 'http://www.w3.org/1999/xhtml';},valueOf:function(){throw 'wrong hint';}};
if(!p.isDefaultNamespace(uri)||calls!=='s')throw 'once/string hint';
if(t.isDefaultNamespace({toString:function(){return null;}})||t.isDefaultNamespace({toString:function(){return undefined;}}))throw 'post-coercion nullability';
if(t.isDefaultNamespace('null')||t.isDefaultNamespace('undefined'))throw 'literal nullability';
try{p.isDefaultNamespace({toString:function(){throw marker;}});throw 'missing throw';}catch(e){if(e!==marker)throw e;}
var failures=0;try{p.isDefaultNamespace();}catch(e){if(e.name==='TypeError')failures++;}
for(var v of [Symbol('x'),Object(Symbol('y'))]){try{p.isDefaultNamespace(v);}catch(e){if(e.name==='TypeError')failures++;}}
var arg={toString:function(){calls+='BAD';return '';}};
for(var bad of [{},window,p.style,p.dataset,document.querySelectorAll('p')]){try{p.isDefaultNamespace.call(bad,arg);}catch(e){if(e.name==='TypeError')failures++;}}
if(failures!==8||calls!=='s')throw 'brand/arity/Symbol';
if(document.createTextNode(null).data!=='null')throw 'ordinary string conversion changed';
document.title='namespace-coercion-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "namespace-coercion-ok");
}

#[test]
fn default_namespace_foreign_provenance_survives_moves_and_rejects_unknown_context() {
    let (mut doc, mut forms, mut page, outcome) = page("<html><body><div id=out></div><svg><g id=foreign><i id=inside>f</i></g></svg><p id=html>h</p></body></html>", &[r#"
var g=document.getElementById('foreign'),i=document.getElementById('inside'),out=document.getElementById('out'),uri='http://www.w3.org/1999/xhtml';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let foreign = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("foreign")).unwrap();
    let inside = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("inside")).unwrap();
    let text = doc.nodes[inside].children[0];
    for action in ["", "out.appendChild(g);", "out.removeChild(g);"] {
        if !action.is_empty() {
            let outcome = page.console_eval(&mut doc, &mut forms, action);
            assert_eq!(outcome.errors, 0, "{}", page.report());
        }
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        for node in [foreign, inside, text] {
            let error = bridge.call(node_handle(node), "isDefaultNamespace", &[Value::Null]).unwrap_err();
            assert!(error.contains("unsupported foreign parser context"), "{error}");
        }
    }
    let outcome = page.console_eval(&mut doc, &mut forms, "if(!document.getElementById('html').isDefaultNamespace(uri))throw 'ordinary HTML after foreign';var created=document.createElement('svg');g.appendChild(created);if(!created.isDefaultNamespace(uri))throw 'creation provenance';document.title='namespace-provenance-ok';");
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "namespace-provenance-ok");
}

#[test]
fn default_namespace_uses_actual_document_root_and_retains_invalid_handle_and_work_bounds() {
    for (markup, expected) in [("", true), ("text", true), ("<object>inert</object>", false)] {
        let source = format!("if(document.isDefaultNamespace(null)!=={expected})throw 'document root';");
        let (_, _, page, outcome) = page(markup, &[&source]);
        assert_eq!(outcome.errors, 0, "{}", page.report());
    }
    let (mut doc, mut forms, mut page, outcome) = page("<p>ok</p><template><i id=inert>x</i></template>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let inert = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("inert")).unwrap();
    let invalid = [node_handle(0), node_handle(doc.nodes.len() + 99), node_handle(inert), WINDOW, STYLE_BASE];
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for handle in invalid {
        assert!(bridge.method(handle, "isDefaultNamespace").is_none());
        assert!(!bridge.accepts_receiver(DOCUMENT, "isDefaultNamespace", handle));
        assert!(bridge.call(handle, "isDefaultNamespace", &[Value::Null]).is_err());
    }
    bridge.state.host_work = 0;
    assert!(bridge.call(DOCUMENT, "isDefaultNamespace", &[Value::Null]).is_err());
}

fn page(markup: &str, scripts: &[&str]) -> (Document, Forms, Page, Outcome) {
    let mut doc = crate::html::parse(markup);
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let scripts: Vec<_> = scripts
        .iter()
        .enumerate()
        .map(|(node, source)| Script {
            node,
            source: (*source).into(),
            label: format!("compat script {node}"),
        })
        .collect();
    let mut page = Page::for_test();
    page.set_location(
        &crate::navigation::address(
            "https://www.google.com/search?q=test",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    let outcome = page.start(&mut doc, &mut forms, &scripts);
    (doc, forms, page, outcome)
}

#[test]
fn page_date_clock_is_integral_utc_and_preserves_monotonic_performance_origin() {
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    let (doc, _, page, outcome) = page(
        "<p>UTC clock</p>",
        &[r#"
var now=Date.now(),current=new Date(),mono=performance.now();
if(!Number.isInteger(now)||!Number.isInteger(current.getTime())||!Number.isInteger(mono))throw 'clock units';
if(Date!==window.Date||Date!==globalThis.Date)throw 'global Date';
if(!Number.isFinite(current.getTime())||mono<0||mono>=86400000)throw 'clock origins';
if(Date.parse(current.toISOString())!==current.getTime())throw 'UTC roundtrip';
if(JSON.parse(JSON.stringify({when:new Date(0)})).when!=='1970-01-01T00:00:00.000Z')throw 'Date JSON';
document.title=String(now);
"#],
    );
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let observed: f64 = doc.title.parse().unwrap();
    assert!(observed >= before.min(after) && observed <= before.max(after));
}

#[test]
fn page_date_clock_requires_active_scripting_and_charges_host_work() {
    let (mut doc, mut forms, mut page, outcome) = page("<p>Clock capability</p>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    page.state.host_work = 1;
    let mut bridge = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    };
    let timestamp = bridge.date_now().unwrap();
    assert!(timestamp.is_finite() && timestamp == timestamp.trunc());
    assert_eq!(bridge.state.host_work, 0);
    assert!(bridge.date_now().unwrap_err().contains("DOM work limit"));
    bridge.state.host_work = 10;
    bridge.state.enabled = false;
    assert!(bridge
        .date_now()
        .unwrap_err()
        .contains("scripting is disabled"));
    bridge.state.enabled = true;
    bridge.doc.scripting_enabled = false;
    assert!(bridge
        .date_now()
        .unwrap_err()
        .contains("scripting is disabled"));
}

#[test]
fn dataset_is_live_reflected_and_enumerable_with_stable_identity() {
    let (doc, _, page, outcome) = page(
        "<div id=p data-first-name='Ada' data-x--y='double' data-α='unicode'></div>",
        &[r#"
var p=document.getElementById('p'),d=p.dataset;
if(d!==p.dataset || d.firstName!=='Ada' || d['x-Y']!=='double' || d['α']!=='unicode')throw 'reflection';
if(Object.keys(d).join(',')!=='firstName,x-Y,α')throw 'attribute order';
d.eqid=17;d.empty='';d.nil=null;d.undef=undefined;
if(p.getAttribute('data-eqid')!=='17'|| d.nil!=='null'||d.undef!=='undefined'||!('empty' in d))throw 'conversion';
d.object={toString(){return 'converted';}};
if(d.object!=='converted')throw 'object ToString';
p.setAttribute('data-first-name','Grace');if(d.firstName!=='Grace')throw 'live read';
if(!delete d.firstName||p.hasAttribute('data-first-name')||'firstName' in d)throw 'delete';
if(d['x-y']!==undefined||!delete d['x-y']||d['x-Y']!=='double')throw 'invalid spelling alias';
var desc=Object.getOwnPropertyDescriptor(d,'eqid');
if(desc.value!=='17'||!desc.enumerable||!desc.configurable||!desc.writable)throw 'descriptor';
if(!Object.prototype.hasOwnProperty.call(d,'eqid')||!d.propertyIsEnumerable('eqid')||!('toString' in d))throw 'own or prototype';
Object.assign(d,{lastName:'Hopper'});var clone=Object.assign({},d);
if(clone.lastName!=='Hopper')throw 'assign';
Object.defineProperty(d,'defined',{value:'yes'});if(p.getAttribute('data-defined')!=='yes')throw 'define';
var keys='';for(var key in d){if(key==='eqid')delete d.empty;keys+=key+',';}
if(keys.indexOf('empty,')!==-1)throw 'for in deletion';
document.title='dataset passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "dataset passed");
}

#[test]
fn empty_element_and_style_reads_are_absent_and_keep_dataset_routing() {
    let (doc, _, page, outcome) = page(
        "<p id=p style='color:blue' data-='dataset-empty'>Text</p>",
        &[r#"
var p=document.getElementById('p'),s=p.style,n=0,key={toString(){n++;return '';}};
if(p[key]!==undefined||s[key]!==undefined||n!==2)throw 'empty read/key conversion';
if('' in p||'' in s)throw 'empty presence';
if(p.dataset['']!=='dataset-empty'||p.getAttribute('data-')!=='dataset-empty')throw 'dataset routing';
var marker={},caught=false;try{p[{toString(){throw marker;}}];}catch(e){caught=e===marker;}
if(!caught)throw 'thrown key identity';
if(p.getAttribute('style')!=='color:blue'||p.textContent!=='Text')throw 'native mutation';
document.title='empty-read-ok';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "empty-read-ok");
    let node = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("p")).unwrap();
    assert_eq!(doc.attr(node, "style"), Some("color:blue"));
    assert_eq!(doc.attr(node, "data-"), Some("dataset-empty"));
    assert!(page.state.timers.is_empty());
}

#[test]
fn empty_element_and_style_writes_remain_rejected_without_partial_effects() {
    for source in ["p['']=1;", "p.style['']='red';"] {
        let (doc, _, page, outcome) = page(
            "<p id=p style='color:blue'>Text</p>",
            &["var p=document.getElementById('p');", source,
              "if(p['']!==undefined||p.style['']!==undefined||p.textContent!=='Text'||p.getAttribute('style')!=='color:blue')throw 'partial effect';document.title='empty-write-rejected';"],
        );
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert_eq!(doc.title, "empty-write-rejected");
        assert!(page.report().contains("not writable") || page.report().contains("not supported"));
    }
}

#[test]
fn empty_host_property_reads_keep_handle_validation_and_dom_work_bounds() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p>Text</p>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let missing = doc.nodes.len() + 99;
    let node = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("p")).unwrap();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for handle in [node_handle(missing), STYLE_BASE + missing as u32] {
        assert!(bridge.get(handle, "").is_err());
    }
    bridge.state.host_work = 1;
    assert!(matches!(bridge.get(STYLE_BASE + node as u32, "").unwrap(), Value::Undefined));
    assert_eq!(bridge.state.host_work, 0);
    assert!(bridge.get(STYLE_BASE + node as u32, "").unwrap_err().contains("DOM work limit"));
}

#[test]
fn node_constants_are_readonly_on_real_document_element_text_and_detached_nodes() {
    let (doc, _, page, outcome) = page("<p id=p>Text</p>", &[r#"
var p=document.getElementById('p'),text=document.createTextNode('x'),detached=document.createElement('i');
var nodes=[document,p,text,detached],pairs=[
['ELEMENT_NODE',1],['ATTRIBUTE_NODE',2],['TEXT_NODE',3],['CDATA_SECTION_NODE',4],
['ENTITY_REFERENCE_NODE',5],['ENTITY_NODE',6],['PROCESSING_INSTRUCTION_NODE',7],
['COMMENT_NODE',8],['DOCUMENT_NODE',9],['DOCUMENT_TYPE_NODE',10],
['DOCUMENT_FRAGMENT_NODE',11],['NOTATION_NODE',12],['DOCUMENT_POSITION_DISCONNECTED',1],
['DOCUMENT_POSITION_PRECEDING',2],['DOCUMENT_POSITION_FOLLOWING',4],
['DOCUMENT_POSITION_CONTAINS',8],['DOCUMENT_POSITION_CONTAINED_BY',16],
['DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC',32]];
var effects=0,coercions=0,rhs={valueOf:function(){coercions++;return 0;},toString:function(){coercions++;return '0';}};
function strictSet(node,key){'use strict';node[key]=0;}
for(var node of nodes){for(var pair of pairs){var key=pair[0],value=pair[1];
if(!(key in node)||node[key]!==value)throw 'constant value/presence';
var result=(node[key]=(effects++,rhs));if(result!==rhs||node[key]!==value)throw 'readonly value';
var caught=false;try{strictSet(node,key);}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'strict readonly';}}
if(effects!==72||coercions!==0||p.nodeType!==p.ELEMENT_NODE||text.nodeType!==text.TEXT_NODE)throw 'conversion/type';
if('DOCUMENT_TYPE_NODE' in window||'DOCUMENT_TYPE_NODE' in p.style||'DOCUMENT_TYPE_NODE' in document.querySelectorAll('p'))throw 'wrong receiver';
p.id='changed';p.textContent='changed';text.data='new';if(p.id!=='changed'||p.textContent!=='changed'||text.data!=='new')throw 'ordinary writes';
document.title='node-constants-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "node-constants-ok");
}

#[test]
fn node_constants_reject_root_out_of_bounds_and_inert_node_handles() {
    let (mut doc, mut forms, mut page, outcome) = page("<p>Visible</p><template><p id=inert>Hidden</p></template>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let inert = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("inert")).unwrap();
    let invalid = [node_handle(0), node_handle(doc.nodes.len() + 99), node_handle(inert)];
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for handle in invalid {
        assert!(bridge.get(handle, "DOCUMENT_TYPE_NODE").is_err());
        assert!(bridge.has(handle, "DOCUMENT_TYPE_NODE").is_err());
        assert!(!bridge.readonly_property(handle, "DOCUMENT_TYPE_NODE"));
        assert!(bridge.set(handle, "DOCUMENT_TYPE_NODE", Value::Number(0.0)).is_err());
    }
}

#[test]
fn event_interface_diagnostic_is_bounded_and_does_not_echo_payload_or_claim_support() {
    for (input, class) in [("eVeNt", "base-event"), ("MouseEvents", "mouse-event"), ("KeyboardEvent", "keyboard-event"), ("PRIVATE_INTERFACE_PAYLOAD", "other")] {
        let report = unsupported_event_interface(Some(&Value::String(input.into())));
        assert!(report.contains(&format!("class={class}; UTF-8 bytes=Some({})", input.len())), "{report}");
        assert!(report.contains("not supported") && report.contains("no event was created"));
        assert!(!report.contains("PRIVATE_INTERFACE_PAYLOAD"));
        assert!(report.len() < 256);
    }
    assert!(unsupported_event_interface(None).contains("class=missing"));
    assert!(unsupported_event_interface(Some(&Value::Number(1.0))).contains("class=non-string"));
}

#[test]
fn event_interface_diagnostic_invokes_no_coercion_or_event_lifecycle() {
    let (mut doc, mut forms, mut page, outcome) = page("<p>Events</p>", &["var conversions=0,request={toString:function(){conversions++;return 'Event';}};"]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    // Preserve the original unsupported-factory diagnostic baseline explicitly.
    page.state.disable_event_factory = true;
    let before = (page.state.next_event, page.state.event.is_some(), page.state.listeners.len());
    let outcome = page.console_eval(&mut doc, &mut forms, "document.createEvent(request);");
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert!(page.report().contains("Requested interface class=non-string"));
    assert_eq!((page.state.next_event, page.state.event.is_some(), page.state.listeners.len()), before);
    let outcome = page.console_eval(&mut doc, &mut forms, "if(conversions!==0||'createEvent' in document||'dispatchEvent' in document||'dispatchEvent' in window)throw 'false capability';");
    assert_eq!(outcome.errors, 0, "{}", page.report());
}

#[test]
fn dataset_validates_keys_and_keeps_native_attribute_readonly() {
    let (doc, _, page, outcome) = page(
        "<p id=p></p>",
        &[r#"
var p=document.getElementById('p'),d=p.dataset,caught='';
try{d['bad-key']='x';}catch(e){caught+=e.name;}
try{d['bad key']='x';}catch(e){caught+=','+e.name;}
try{Object.defineProperty(d,'bad',{value:1,enumerable:false});}catch(e){caught+=','+e.name;}
if(caught!=='SyntaxError,InvalidCharacterError,TypeError')throw caught;
d['']='blank';if(p.getAttribute('data-')!=='blank')throw 'empty key';
d.toString='shadow';if(d.toString!=='shadow'||Object.keys(d).join(',')!==',toString')throw 'override builtins';
delete d.toString;if(typeof d.toString!=='function')throw 'prototype restored';
p.dataset={fake:1};if(p.dataset!==d)throw 'read only dataset';
var strictFailed=false;try{(function(){'use strict';p.dataset={};})();}catch(e){strictFailed=e.name==='TypeError';}
if(!strictFailed)throw 'strict readonly';document.title='dataset validation passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "dataset validation passed");
}

#[test]
fn navigator_expandos_methods_and_descriptors_work_without_replacing_native_facts() {
    let (doc, _, page, outcome) = page(
        "<p>navigator</p>",
        &[r#"
var agent=navigator.userAgent,original=navigator.sendBeacon;
navigator.feature={count:3};navigator.polyfill=function(a){return this===navigator&&a+this.feature.count;};
if(navigator.polyfill(4)!==7||clientInformation.feature!==navigator.feature)throw 'expando or receiver';
navigator.sendBeacon=function(a){return this===navigator&&a==='queued';};
if(!navigator.sendBeacon('queued'))throw 'method override';
if(!delete navigator.sendBeacon||navigator.sendBeacon!==original)throw 'prototype method restored';
Object.defineProperty(navigator,'custom',{value:5,writable:true,enumerable:true,configurable:true});
if(navigator.custom!==5||Object.keys(navigator).join(',')!=='feature,polyfill,custom')throw 'descriptor or enumeration';
if(!delete navigator.custom||'custom' in navigator)throw 'delete';
navigator.userAgent='fake';navigator.cookieEnabled=true;
if(navigator.userAgent!==agent||navigator.cookieEnabled!==true)throw 'native information changed';
if(Object.getOwnPropertyDescriptor(navigator,'userAgent')!==undefined)throw 'inherited native getter';
var failed=false;try{(function(){'use strict';navigator.userAgent='fake';})();}catch(e){failed=e.name==='TypeError';}
if(!failed||!delete navigator.userAgent||navigator.userAgent!==agent)throw 'protected native getter';
document.title='navigator passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "navigator passed");
}

#[test]
fn image_constructor_creates_detached_nodes_without_claiming_a_load() {
    let (doc, _, page, outcome) = page(
        "<p>images</p>",
        &[r#"
var image=new Image(13,17),other=document.createElement('img'),loaded=false;
if(image===other||!(image instanceof Image)||!(other instanceof Image)||({} instanceof Image))throw 'identity';
if(image.tagName!=='IMG'||image.nodeType!==1||image.parentNode!==null||image.parentElement!==null||image.ownerDocument!==document||image.isConnected)throw 'detached';
if(image.width!==13||image.height!==17||image.getAttribute('width')!=='13'||!image.complete||image.src!=='')throw 'initial dimensions';
image.onload=function(){loaded=true;};image.onerror=function(){loaded=true;};
image.id='detached';image.dataset.source='test';image.src='/pixel.png';
if(image.src!=='https://www.google.com/pixel.png'||image.getAttribute('src')!=='/pixel.png'||image.complete||image.currentSrc!==''||image.naturalWidth!==0||image.naturalHeight!==0)throw 'truthful source';
image.setAttribute('src','next.png');if(image.src!=='https://www.google.com/next.png')throw 'src reflection';
image.width=-1;image.height=5.9;if(image.width!==4294967295||image.height!==5)throw 'unsigned long';
image.width={valueOf(){return 18;}};if(image.width!==18)throw 'numeric conversion';
if(document.images.length!==0||document.getElementById('detached')!==null||loaded)throw 'connected or loaded';
document.title='image passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "image passed");
    assert_eq!(page.state.detached_images.len(), 2);
    assert_eq!(page.state.declined_images, 2);
    assert!(page
        .report()
        .contains("no request or load event was fabricated"));
}

#[test]
fn google_adapted_image_and_beacon_snippets_finish_and_decline_network() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<div id=css></div>",
        &[r#"
var src='/gen_204?image=1',iesg=true,cssId='css',eid='local';
window.addEventListener('load',function(){window.n&&window.n();if(document.images){new Image().src=src;}});
typeof navigator.sendBeacon!=='function'&&(navigator.sendBeacon=function(a){(new Image).src=a});
var queued=navigator.sendBeacon('/gen_204?startup=1','');if(queued!==false)throw 'fabricated queue success';
setTimeout(function(){document.getElementById(cssId).setAttribute('style','');if(navigator.sendBeacon('/gen_204?cad=sg_trbl&ei='+eid,'')!==false)throw 'queued';document.title='beacon passed';},0);
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    page.poll_timer_at(
        &mut doc,
        &mut forms,
        Instant::now() + Duration::from_secs(1),
    );
    assert_eq!(page.state.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "beacon passed");
    assert_eq!(page.state.declined_beacons, 2);
    assert_eq!(page.state.declined_images, 1);
    assert_eq!(page.state.timer_callbacks, 1);
}

#[test]
fn element_method_values_are_stable_callable_and_check_their_receiver() {
    let (doc, _, page, outcome) = page(
        "<button id=b>Button</button>",
        &[r#"
var b=document.getElementById('b'),handler=function(){},method=b.addEventListener;
if(typeof method!=='function'||method!==b.addEventListener)throw 'unstable method';
b.dataset.eqid='g1';b&&b.addEventListener?b.addEventListener('click',handler):0;
method.call(b,'click',handler);b.removeEventListener('click',handler);
if(b.getAttribute.call(b,'data-eqid')!=='g1')throw 'extracted native getter';
var failed=false;try{method.call({},'click',handler);}catch(e){failed=e.name==='TypeError';}
if(!failed)throw 'receiver';document.title='native methods passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "native methods passed");
    assert!(page.state.listeners.is_empty());
}

#[test]
fn beacon_rejects_invalid_urls_and_does_not_claim_cross_origin_transmission() {
    let (doc, _, page, outcome) = page(
        "<p>beacons</p>",
        &[r#"
var caught=0;try{navigator.sendBeacon();}catch(e){if(e.name==='TypeError')caught++;}
try{navigator.sendBeacon('file:///private');}catch(e){if(e.name==='TypeError')caught++;}
try{navigator.sendBeacon('javascript:alert(1)');}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==3)throw 'URL validation';
var source={toString(){return 'https://other.example/report';}};
if(navigator.sendBeacon(source,'body')!==false)throw 'cross origin sent';
document.title='beacon validation passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "beacon validation passed");
    assert_eq!(page.state.declined_beacons, 1);
}

#[test]
fn image_growth_is_bounded_and_following_classic_script_recovers() {
    let (doc, _, page, outcome) = page(
        "<p>node limit</p>",
        &[
            "for(var i=0;i<1000;i++){new Image();}",
            "document.title='node limit recovered';",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "node limit recovered");
    assert_eq!(page.state.detached_images.len(), MAX_EXTRA_NODES);
    assert!(page.report().contains("DOM appended-node limit reached"));
}

#[test]
fn classic_script_failure_does_not_poison_later_scripts_or_events() {
    let (doc, _, page, outcome) = page(
        "<p>budgets</p>",
        &[
            "window.addEventListener('load',function(){document.title+=' loaded';});while(true);",
            "document.title='following script';",
            "document.addEventListener('DOMContentLoaded',function(){document.title+=' ready';});",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "following script ready loaded");
    assert!(page.report().contains("step limit"));
    assert_eq!(page.state.executed, 3);
}

#[test]
fn aggregate_startup_allowance_stops_a_chain_of_hostile_classic_scripts() {
    let (doc, _, page, outcome) = page(
        "<p>budgets</p>",
        &[
            "while(true);",
            "while(true);",
            "while(true);",
            "while(true);",
            "while(true);",
            "document.title='should not run';",
        ],
    );
    assert_ne!(doc.title, "should not run");
    assert!(outcome.errors >= 2);
    assert!(page.state.executed <= 4, "{}", page.report());
    assert!(page
        .report()
        .contains("Aggregate startup work/time limit reached"));
}

#[test]
fn many_parameters_work_and_parameter_bound_remains_atomic() {
    let parameters = (0..80)
        .map(|index| format!("p{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let arguments = (0..80)
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let source=format!("function large({parameters}){{return p0+p32+p79+arguments.length;}}var arrow=({parameters})=>p79;if(large({arguments})!==191||large.apply(null,[{arguments}])!==191||arrow({arguments})!==79)throw 'large args';document.title='parameters passed';");
    let (doc, _, page, outcome) = page("<p>arguments</p>", &[&source]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "parameters passed");
    let parameters = (0..257)
        .map(|index| format!("p{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!("document.title='bad';function excessive({parameters}){{}};");
    let (doc, _, page, outcome) = self::page(
        "<p>arguments</p>",
        &[&source, "document.title='recovered';"],
    );
    assert_eq!(outcome.errors, 1);
    assert_eq!(doc.title, "recovered");
    assert!(page
        .report()
        .contains("Function parameter limit reached (256)"));
}

#[test]
fn element_methods_can_be_borrowed_and_inspection_does_not_grant_mutation() {
    let (doc, _, page, outcome) = page(
        "<a id=a href='/original'>A</a><a id=b href='/other' data-α='greek'>B</a><input id=c name=choice type=checkbox>",
        &[r#"
var a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c');
var get=a.getAttribute,set=a.setAttribute;
if(get.call(b,'href')!=='/other'||get.call(c,'type')!=='checkbox'||!c.hasAttribute('name')||b.getAttribute('data-α')!=='greek')throw 'attribute reads';
var conversions='';
set.call(b,{toString(){conversions+='n';return 'data-result';}},{toString(){conversions+='v';return 'converted';}});
if(conversions!=='nv'||b.dataset.result!=='converted'||a.hasAttribute('data-result'))throw 'conversion or receiver';
if(document.getElementById({toString(){return 'b';}})!==b||get.call(b,'missing')!==null)throw 'argument conversion';
var event=function(){};document.addEventListener.call(b,'click',event);a.removeEventListener.call(b,'click',event);
var failures=0;try{get.call({},'id');}catch(e){if(e.name==='TypeError')failures++;}
try{get.call(document,'id');}catch(e){if(e.name==='TypeError')failures++;}
try{get.call(b);}catch(e){if(e.name==='TypeError')failures++;}
try{set.call(b,'data-empty');}catch(e){if(e.name==='TypeError')failures++;}
if(failures!==4)throw 'receiver or required argument';
var premature=false;try{set.call(b,{toString(){premature=true;return 'data-empty';}});}catch(e){}
if(premature)throw 'required arguments must be checked before conversion';
document.title='read methods passed';
"#,
          "document.getElementById('b').setAttribute('href','https://other.example/');",
          "if(document.getElementById('b').getAttribute('href')!=='/other')throw 'unsafe mutation';document.title+=' safely';"],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "read methods passed safely");
    assert!(page.state.listeners.is_empty());
    assert!(page.report().contains("Supported attributes"));
}

#[test]
fn selector_queries_return_static_ordered_lists_and_match_ancestors() {
    let (doc, _, page, outcome) = page(
        "<body><section id=root class=group><p id=a class=card><span id=inner>A</span></p><p id=b class=card>B</p><input id=check type=checkbox checked></section><p id=c class=card>C</p></body>",
        &[r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),inner=document.getElementById('inner');
var list=document.querySelectorAll('.card,#b,.card'),local=root.querySelectorAll('body .card');
if(list.length!==3||local.length!==2||list[0]!==a||list[1]!==b||list[2].id!=='c'||list[3]!==undefined||list['01']!==undefined)throw 'ordered list';
if(list.item(0)!==a||list.item(-1)!==null||list.item(100)!==null||list.item({valueOf(){return 1;}})!==b)throw 'item';
if(Object.keys(list).join(',')!=='0,1,2'||!('0' in list)||'3' in list||list===document.querySelectorAll('.card'))throw 'identity or keys';
if(Object.getOwnPropertyDescriptor(list,'0').writable||Object.getOwnPropertyDescriptor(list,'length')!==undefined)throw 'descriptors';
if(!a.matches('section > p.card')||a.matches('#b')||inner.closest('.group')!==root||a.closest('p')!==a||a.closest('.absent')!==null)throw 'match or closest';
if(a.matches.call(b,'#b')!==true||a.querySelectorAll.call(root,'.card').length!==2||list.item.call(local,1)!==b)throw 'borrowed query';
var check=document.getElementById('check');check.checked=false;
if(check.matches(':checked')||document.querySelectorAll(':checked').length!==0)throw 'checked state';
var image=new Image();if(!image.matches('img')||image.closest('img')!==image||image.closest('body')!==null)throw 'detached match';
var order='',receiver={mark:7};
if(list.forEach(function(node,index,collection){if(this!==receiver||collection!==list)throw 'callback';order+=node.id+index;},receiver)!==undefined||order!=='a0b1c2')throw 'forEach';
var caught=0;try{list.forEach(null);}catch(e){if(e.name==='TypeError')caught++;}
try{list.forEach(function(){throw Error('stop');});}catch(e){if(e.message==='stop')caught++;}
try{list.item();}catch(e){if(e.name==='TypeError')caught++;}
try{root.querySelectorAll('p:unsupported');}catch(e){if(e.name==='SyntaxError')caught++;}
try{a.closest('[');}catch(e){if(e.name==='SyntaxError')caught++;}
try{a.matches();}catch(e){if(e.name==='TypeError')caught++;}
try{(function(){'use strict';list[0]=b;})();}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==7||delete list[0]||!delete list[99])throw 'errors or deletion';
for(var invalid of ['.123','#123','.-1','-1']){var syntax=false;try{a.matches(invalid);}catch(e){syntax=e.name==='SyntaxError';}if(!syntax)throw 'invalid identifier';}
if(a.matches('.--valid'))throw 'valid identifier';
list.length=0;if(list.length!==3)throw 'readonly length';
if(document.querySelector({toString(){return '#b';}},'ignored')!==b)throw 'selector conversion';
document.title='queries passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "queries passed");
}

#[test]
fn selector_snapshots_preserve_detached_nodes_and_keep_live_lookup_separate() {
    let (doc, _, page, outcome) = page(
        "<div id=root><span id=a class=card><b id=inner>old</b></span><span id=b class=card>second</span></div><p id=counter>0</p>",
        &[r#"
var root=document.getElementById('root'),list=root.querySelectorAll('.card'),a=list[0],inner=document.getElementById('inner');
a.className='changed';if(list.length!==2||root.querySelectorAll('.card').length!==1)throw 'static membership';
root.textContent='new';
if(list[0]!==a||a.isConnected||a.id!=='a'||a.textContent!=='old'||a.parentNode!==null||inner.parentNode!==a||inner.closest('span')!==a)throw 'detached identity';
if(document.getElementById('a')!==null||document.getElementById('inner')!==null||document.querySelectorAll('span').length!==0||root.querySelectorAll('*').length!==0)throw 'connected lookup';
if(a.querySelector('b')!==inner||a.querySelectorAll('b')[0]!==inner)throw 'detached subtree';
var visited='';list.forEach(function(node){visited+=node.textContent;});if(visited!=='oldsecond')throw 'snapshot contents';
inner.textContent='retained';if(a.textContent!=='retained'||root.textContent!=='new')throw 'detached mutation';
a.textContent='changed';if(a.textContent!=='changed'||inner.parentNode!==null||inner.textContent!=='retained'||root.textContent!=='new')throw 'detached replacement';
var counter=document.getElementById('counter');for(var i=0;i<1000;i++)counter.textContent=String(i);
if(counter.textContent!=='999')throw 'counter';document.title='snapshot passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "snapshot passed");
    assert_eq!(page.state.detached_nodes.len(), 6);
    assert!(page.state.retained <= page.state.retained_limit);
    let before = crate::html::parse("<div id=root><span id=a class=card><b id=inner>old</b></span><span id=b class=card>second</span></div><p id=counter>0</p>").nodes.len();
    assert_eq!(
        doc.nodes.len(),
        before + 2,
        "counter replacement must reuse its text node"
    );
}

#[test]
fn selector_snapshot_counts_and_retained_references_are_bounded() {
    let (doc, _, page, outcome) = page(
        "<p id=one>one</p>",
        &[
            "for(var i=0;i<129;i++)document.querySelectorAll('p');",
            "document.title='snapshot cap recovered';",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "snapshot cap recovered");
    assert_eq!(page.state.node_lists.len(), MAX_NODE_LISTS);
    assert!(page.report().contains("NodeList snapshot limit"));

    let markup = "<div></div>".repeat(512);
    let (mut doc, mut forms, mut page, outcome) = self::page(&markup, &[
        "var listCount=0;function collect(){document.querySelectorAll('div');listCount++;if(listCount<40)setTimeout(collect,0);}collect();",
    ]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    for _ in 0..40 {
        page.poll_timer_at(
            &mut doc,
            &mut forms,
            Instant::now() + Duration::from_secs(1),
        );
    }
    assert_eq!(page.state.node_list_references, MAX_NODE_LIST_REFERENCES);
    assert_eq!(page.state.node_lists.len(), 32);
    assert!(
        page.report().contains("NodeList retained-reference limit"),
        "{}",
        page.report()
    );
}

#[test]
fn selector_result_and_callback_work_limits_do_not_publish_partial_success() {
    let markup = "<div></div>".repeat(4097);
    let (doc, _, page, outcome) = page(
        &markup,
        &[
            "document.querySelectorAll('div');document.title='incorrect';",
            "document.title='result bound recovered';",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "result bound recovered");
    assert!(page.state.node_lists.is_empty());
    assert_eq!(page.state.node_list_references, 0);
    assert!(page.report().contains("selector result limit"));
    let (doc, _, page, outcome) = self::page("<p>one</p>", &[
        "var caught=false;try{document.querySelectorAll('p').forEach(function(){while(true);});}catch(e){caught=true;}finally{caught=true;}",
        "if(caught)throw 'budget was catchable';document.title='callback bound recovered';",
    ]);
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "callback bound recovered");
    assert!(page.report().contains("step limit"));
}

#[test]
fn detached_snapshot_ids_do_not_shadow_connected_label_targets() {
    let markup = "<div id=host><span id=field>old</span></div><input id=field><label id=label for=field>Click</label><template><input id=field></template>";
    let before = crate::html::parse(markup);
    assert!(
        crate::forms::label_targets(&before).is_empty(),
        "first connected duplicate is not labelable"
    );
    let (doc, forms, page, outcome) = page(
        markup,
        &[r#"
var host=document.getElementById('host'),saved=host.querySelectorAll('span');host.textContent='';
if(saved[0].id!=='field'||saved[0].isConnected||document.getElementById('field').tagName!=='INPUT')throw 'connected IDs';
document.title='label lookup passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "label lookup passed");
    let input = *forms.controls.keys().next().unwrap();
    let label = doc
        .nodes
        .iter()
        .enumerate()
        .find(|(node, _)| doc.attr(*node, "id") == Some("label"))
        .unwrap()
        .0;
    let labels = crate::forms::label_targets(&doc);
    assert_eq!(labels.get(&label), Some(&input));
    assert_eq!(labels.len(), 1);
}

#[test]
fn element_only_selector_methods_are_not_exposed_on_document() {
    let (doc, _, page, outcome) = page("<p>Element selectors</p>", &[
        "document.matches('p');",
        "document.closest('p');",
        "if('matches' in document||'closest' in document)throw 'wrong interface';document.title='interfaces passed';",
    ]);
    assert_eq!(outcome.errors, 2, "{}", page.report());
    assert_eq!(doc.title, "interfaces passed");
    assert!(page.report().contains("Document method"));
}

#[test]
fn dom_creation_insertion_text_and_saved_nodes_keep_their_identity() {
    let (doc, _, page, outcome) = page(
        "<html><head></head><body><div id=root><span id=a>A</span><span id=b>B</span></div><template><p id=hidden>inert</p></template></body></html>",
        &[r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b');
var box=document.createElement('DIV'),text=document.createTextNode({toString(){return 'A😀';}}),last=document.createTextNode('Z');
box.id='new';if(box.tagName!=='DIV'||box.isConnected||box.parentNode!==null)throw 'created element';
if(box.appendChild(text)!==text||box.appendChild(last)!==last||text.nodeType!==3||text.nodeName!=='#text'||text.length!==3||text.data!=='A😀')throw 'text node';
if(box.firstChild!==text||box.lastChild!==last||text.nextSibling!==last||last.previousSibling!==text||text.parentElement!==box||text.ownerDocument!==document)throw 'links';
text.data='new';if(box.textContent!=='newZ')throw 'live text';
if(root.insertBefore(box,b)!==box||document.getElementById('new')!==box||!text.isConnected||box.previousSibling!==a||box.nextSibling!==b)throw 'insert';
var saved=root.querySelectorAll('div');
root.insertBefore(box,box);root.appendChild(a);if(root.firstChild!==box||root.lastChild!==a)throw 'reorder';
if(root.removeChild(box)!==box||box.isConnected||text.isConnected||box.parentNode!==null||text.parentNode!==box||document.getElementById('new')!==null||saved[0]!==box)throw 'remove identity';
root.insertBefore(box,null);if(!box.isConnected||root.lastChild!==box||root.querySelector('#new')!==box)throw 'reinsert';
box.removeChild(text);if(text.parentNode!==null||text.data!=='new'||text.isConnected)throw 'text removal';
text.nodeValue=null;if(text.data!==''||box.textContent!=='Z')throw 'text null';
if(document.getElementById('hidden')!==null||document.querySelectorAll('template,p').length!==0)throw 'inert lookup';
document.title='DOM tree passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "DOM tree passed");
}

#[test]
fn named_form_controls_request_real_focus_and_keep_builtin_priorities() {
    let (doc, forms, mut page, outcome) = page(
        "<html><head></head><body><form id=search name=f><input id=query name=q><input name=textContent><input name=querySelector></form><input id=external name=outside form=search><form name=body></form><form name=getElementById></form><form name=other><input name=q></form><template><form name=inert><input name=q></form></template></body></html>",
        &[r#"
if(document.f!==document.getElementById('search')||!('f' in document)||!('q' in document.f))throw 'named form';
if(document.f.q!==document.getElementById('query')||document.f.query!==document.f.q||document.f.outside!==document.getElementById('external'))throw 'owned controls';
if(document.body.tagName!=='BODY'||typeof document.getElementById!=='function'||typeof document.f.querySelector!=='function'||typeof document.f.textContent!=='string')throw 'native priority';
if(document.missing!==undefined||document.inert!==undefined||document.f.missing!==undefined||'missing' in document.f)throw 'missing or inert name';
document.f&&document.f.q.focus();document.title='named focus passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "named focus passed");
    let query = forms
        .controls
        .keys()
        .copied()
        .find(|&node| doc.attr(node, "id") == Some("query"))
        .unwrap();
    assert_eq!(page.take_focus_request(), Some(query));
    assert_eq!(page.take_focus_request(), None);
}

#[test]
fn ambiguous_named_form_collections_are_explicitly_unavailable() {
    let (doc, _, page, outcome) = page(
        "<form name=duplicate></form><form name=duplicate></form><form name=f><input name=q><input name=q></form>",
        &[
            "var wrong=document.duplicate;document.title='incorrect';",
            "document.f.q.focus();document.title='incorrect';",
            "document.title='named ambiguity recovered';",
        ],
    );
    assert_eq!(outcome.errors, 2, "{}", page.report());
    assert_eq!(doc.title, "named ambiguity recovered");
    assert!(page.report().contains("live HTMLCollection"));
    assert!(page.report().contains("RadioNodeList"));
    assert_eq!(page.state.dom.pending_focus, None);
}

#[test]
fn created_script_properties_and_text_are_real_without_fabricated_loads() {
    let (doc, _, page, outcome) = page(
        "<body><div id=root></div></body>",
        &[r#"
var script=document.createElement('script'),fired=0;
if(!script.async||script.defer||script.noModule||script.src!==''||script.type!=='')throw 'script defaults';
script.src={toString(){return '/payload.js';}};script.type='text/javascript';script.nonce='nonce-1';script.charset='utf-8';
script.async=false;script.defer=true;script.noModule=true;script.crossOrigin='anonymous';script.referrerPolicy='no-referrer';
script.integrity='sha256-original';
if(script.src!=='https://www.google.com/payload.js'||script.getAttribute('src')!=='/payload.js'||script.async||!script.defer||!script.noModule||script.getAttribute('nonce')!=='nonce-1'||script.getAttribute('crossorigin')!=='anonymous')throw 'reflection';
script.setAttribute('src','next.js');script.removeAttribute('defer');if(script.src!=='https://www.google.com/next.js'||script.defer)throw 'attribute mutation';
script.onload=function(){fired++;};script.onerror=function(){fired++;};
var old=document.createTextNode('old');script.appendChild(old);if(script.text!=='old'||script.textContent!=='old')throw 'script text';
script.text='new';if(old.data!=='old'||old.parentNode!==null||script.firstChild.data!=='new')throw 'retained text';
script.textContent=null;if(script.firstChild!==null||script.text!=='')throw 'empty text';
if(script.isConnected||script.parentNode!==null||fired||document.querySelector('script')!==null)throw 'no invented load';
document.title='script objects passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "script objects passed");
}

#[test]
fn connected_created_scripts_queue_once_and_completion_keeps_native_receivers() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<html><head></head><body><div id=root></div></body></html>",
        &[r#"
var first=document.createElement('script'),second=document.createElement('script'),phases='';
window.addEventListener('load',function(e){if(e.target===first){if(e.eventPhase!==1||e.bubbles)throw 'capture phase';phases+='capture,';}},true);
first.src='/one.js';first.async=false;
first.onload=function(e){if(this!==first||e.target!==first||e.currentTarget!==first||e.eventPhase!==2||e.bubbles||window.loadedValue!==17)throw 'load receiver';phases+='target';document.title=phases;};
document.head.appendChild(first);document.head.appendChild(first);
document.head.appendChild(second);second.src='/two.js';
second.onerror=function(e){if(this!==second||e.target!==second||e.bubbles||this.isConnected||this.parentNode!==null)throw 'detached error receiver';document.title+=' + error';};
document.head.removeChild(second);
var data=document.createElement('script');data.type='application/json';data.id='data';data.text='never executed';document.getElementById('root').appendChild(data);
if(!first.isConnected||first.parentNode!==document.head||second.isConnected||!data.isConnected||document.querySelector('script')!==first||document.getElementById('data')!==data||document.getElementById('root').textContent!=='')throw 'physical or inert tree';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let mut requests = page.take_dynamic_scripts();
    assert_eq!(
        requests.len(),
        2,
        "same script must prepare once and data block never prepares"
    );
    let first = requests.pop_front().unwrap();
    let second = requests.pop_front().unwrap();
    assert!(first.error.is_none(), "{:?}", first.error);
    assert!(second.error.is_none(), "{:?}", second.error);
    assert_eq!(first.target.display(), "https://www.google.com/one.js");
    assert_eq!(second.target.display(), "https://www.google.com/two.js");
    let loaded = page.complete_dynamic_script(
        &mut doc,
        &mut forms,
        &first,
        Ok(Script {
            node: first.node,
            source: "window.loadedValue=17;".into(),
            label: "original local fixture".into(),
        }),
    );
    assert_eq!(loaded.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "capture,target");
    let failed = page.complete_dynamic_script(
        &mut doc,
        &mut forms,
        &second,
        Err("fixture fetch failure".into()),
    );
    assert_eq!(failed.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "capture,target + error");
    assert!(page.take_dynamic_scripts().is_empty());
}

#[test]
fn dom_tree_rejections_preserve_existing_controls_resources_and_parent_links() {
    let (doc, forms, page, outcome) = page(
        "<html><head></head><body><div id=root><span id=child>old</span></div><span id=outside>outside</span><form id=form><input id=query value=original></form><img id=image src=pixel.png></body></html>",
        &[
            "var root=document.getElementById('root'),child=document.getElementById('child'),outside=document.getElementById('outside');child.appendChild(root);",
            "root.insertBefore(document.createElement('p'),outside);",
            "document.body.removeChild(document.getElementById('form'));",
            "document.body.appendChild(document.createElement('input'));",
            "document.body.appendChild(document.getElementById('image'));",
            "var text=document.createTextNode('safe');text.appendChild(document.createElement('span'));",
            "if(root.parentNode!==document.body||child.parentNode!==root||root.textContent!=='old'||outside.parentNode!==document.body||document.getElementById('query').value!=='original'||document.images.length!==1||text.data!=='safe')throw 'partial mutation';document.title='mutation limits recovered';",
        ],
    );
    assert_eq!(outcome.errors, 6, "{}", page.report());
    assert_eq!(doc.title, "mutation limits recovered");
    assert_eq!(forms.controls.len(), 1);
    assert!(page.report().contains("HierarchyRequestError"));
    assert!(page.report().contains("NotFoundError"));
    assert!(page.report().contains("loaded resources"));
}

#[test]
fn dom_creation_validates_arguments_and_borrowed_native_receivers() {
    let (doc, _, page, outcome) = page(
        "<body><div id=p></div></body>",
        &[r#"
var p=document.getElementById('p'),text=document.createTextNode('x'),bad=0;
try{document.createElement('bad name');}catch(e){if(e.name==='InvalidCharacterError')bad++;}
try{document.createElement();}catch(e){if(e.name==='TypeError')bad++;}
try{document.createTextNode();}catch(e){if(e.name==='TypeError')bad++;}
try{p.appendChild({});}catch(e){if(e.name==='TypeError')bad++;}
try{p.insertBefore(text);}catch(e){if(e.name==='TypeError')bad++;}
try{p.appendChild.call({},text);}catch(e){if(e.name==='TypeError')bad++;}
var other=document.createElement('span');p.appendChild.call(other,text);
if(bad!==6||text.parentNode!==other||other.firstChild!==text||p.firstChild!==null)throw 'receiver or validation';
text.nodeType=1;if(text.nodeType!==3)throw 'readonly';document.title='DOM arguments passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "DOM arguments passed");
}

#[test]
fn general_dom_creation_shares_the_existing_node_growth_bound() {
    let (doc, _, page, outcome) = page("<p>bounds</p>", &[
        "for(var i=0;i<1000;i++){if(i%3===0)new Image();else if(i%3===1)document.createElement('div');else document.createTextNode('');}",
        "document.title='DOM node cap recovered';",
    ]);
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "DOM node cap recovered");
    assert_eq!(page.state.dom.created.len(), MAX_EXTRA_NODES);
    assert!(page.report().contains("appended-node limit"));
}

#[test]
fn dynamic_dom_depth_is_bounded_and_next_script_can_run() {
    let (doc, _, page, outcome) = page("<p>depth</p>", &[
        "var tree=document.createElement('div'),tail=tree;for(var i=0;i<150;i++){var child=document.createElement('div');tail.appendChild(child);tail=child;}",
        "if(tree.isConnected||tail.parentNode===null)throw 'invalid detached tree';document.title='DOM depth recovered';",
    ]);
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "DOM depth recovered");
    assert!(page.report().contains("tree depth limit"));
}

#[test]
fn tag_collections_are_live_scoped_ordered_and_keep_native_member_priorities() {
    let (doc, _, page, outcome) = page(
        "<body><div id=root><p id=one name=alpha>A</p><p id=two>B</p></div><p id=outside>C</p><img id=image><template><p id=inert>hidden</p></template></body>",
        &[r#"
var root=document.getElementById('root'),one=document.getElementById('one'),two=document.getElementById('two');
var all=document.getElementsByTagName('P'),local=root.getElementsByTagName('p'),saved=root.querySelectorAll('p');
if(all.length!==3||local.length!==2||all[2].id!=='outside'||local.item(0)!==one||local[2]!==undefined||local.item(-1)!==null||local.item(100)!==null)throw 'initial collection';
if(local.namedItem('alpha')!==one||local.one!==one||local.namedItem('')!==null||local.namedItem('inert')!==null||local.namedItem({toString(){return 'two';}})!==two)throw 'named collection';
if(local.item({valueOf(){return 1;}})!==two||Object.keys(local).join(',')!=='0,1'||!('one' in local)||'2' in local)throw 'index reflection';
if(Object.getOwnPropertyDescriptor(local,'0').writable||Object.getOwnPropertyDescriptor(local,'length')!==undefined)throw 'index descriptor';
var child=document.createElement('p');child.id='new';root.insertBefore(child,one);
if(local.length!==3||all.length!==4||local[0]!==child||saved.length!==2||saved[0]!==one)throw 'live insert versus snapshot';
root.appendChild(one);if(local[2]!==one||local.namedItem('new')!==child)throw 'live order';
child.id='renamed';if(local.namedItem('new')!==null||local.namedItem('renamed')!==child)throw 'live rename';
child.id='length';if(local.length!==3||local.namedItem('length')!==child)throw 'length priority';
child.id='item';if(typeof local.item!=='function'||local.namedItem('item')!==child)throw 'method priority';
if(local.item.call(document.images,0)!==document.getElementById('image')||document.images.item.call(local,0)!==child)throw 'borrowed collection method';
local.length=0;local[0]=null;if(local.length!==3||local[0]!==child)throw 'readonly collection';
var bad=0;try{local.item();}catch(e){if(e.name==='TypeError')bad++;}
try{local.namedItem();}catch(e){if(e.name==='TypeError')bad++;}
try{root.getElementsByTagName();}catch(e){if(e.name==='TypeError')bad++;}
try{local.item.call({},0);}catch(e){if(e.name==='TypeError')bad++;}
if(bad!==4)throw 'collection receivers';
root.textContent='replacement';if(local.length!==0||all.length!==1||saved[0]!==one||one.parentNode!==null)throw 'live removal';
root.appendChild(child);if(local.length!==1||local[0]!==child||all.length!==2||root.getElementsByTagName('*').length!==1)throw 'retained live root';
document.title='live tag collections passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "live tag collections passed");
}

#[test]
fn parsed_script_nonce_lookup_is_real_without_activating_inert_contents() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<html><head><script id=seed nonce=YWJj>original_source_only()</script><template><script id=inert nonce=bad>inert_source()</script></template></head><body><p>visible</p></body></html>",
        &[r#"
var scripts=document.getElementsByTagName('SCRIPT'),seed=document.querySelector('script[nonce]');
if(scripts.length!==1||scripts[0]!==seed||seed!==document.getElementById('seed')||seed.nonce!=='YWJj'||seed.getAttribute('nonce')!=='YWJj'||!seed.isConnected)throw 'actual nonce';
if(document.getElementsByTagName('HEAD')[0]!==document.head||document.head.getElementsByTagName('script')[0]!==seed||document.body.getElementsByTagName('script').length!==0)throw 'root scoping';
if(seed.textContent!=='original_source_only()'||seed.text!=='original_source_only()'||document.head.textContent!==''||document.querySelectorAll('script').length!==1||document.getElementById('inert')!==null)throw 'inert source';
var added=document.createElement('script');added.nonce=seed.nonce;added.src='/nonce-script.js';document.head.appendChild(added);
if(scripts.length!==2||scripts[1]!==added||added.nonce!==seed.nonce)throw 'live script nonce';
document.title='nonce lookup passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "nonce lookup passed");
    let requests = page.take_dynamic_scripts();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].nonce.as_deref(), Some("YWJj"));
    let seed = (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some("seed"))
        .unwrap();
    page.begin_job();
    Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    }
    .queue_dynamic_script(seed)
    .unwrap();
    assert!(
        page.take_dynamic_scripts().is_empty(),
        "parsed script must never enter the dynamic queue"
    );
    let styles = crate::css::compute(&doc, &HashMap::new());
    assert_eq!(styles[seed].display, crate::css::Display::None);
}

#[test]
fn parsed_script_tree_mutation_is_rejected_without_partial_source_changes() {
    let (doc, _, page, outcome) = page(
        "<head><script id=seed nonce=YWJj>original()</script></head><body></body>",
        &[
            "var seed=document.querySelector('script[nonce]');seed.appendChild(document.createTextNode('changed()'));",
            "document.head.removeChild(seed);",
            "if(seed.textContent!=='original()'||seed.parentNode!==document.head||!seed.isConnected)throw 'source changed';document.title='parsed script protected';",
        ],
    );
    assert_eq!(outcome.errors, 2, "{}", page.report());
    assert_eq!(doc.title, "parsed script protected");
}

#[test]
fn live_tag_collections_keep_retention_and_repeated_walks_bounded() {
    let (doc, _, page, outcome) = page(
        "<p>collection caps</p>",
        &[
            "for(var i=0;i<1000;i++)document.getElementsByTagName('tag'+i);",
            "document.title='collection cap recovered';",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "collection cap recovered");
    assert_eq!(page.state.dom.collections.len(), 128);
    assert!(page.report().contains("tag-collection limit"));

    let markup = format!("<body>{}</body>", "<div>value</div>".repeat(1000));
    let (doc, _, page, outcome) = self::page(
        &markup,
        &[
            "var live=document.getElementsByTagName('div');for(var i=0;i<1000;i++)live.length;",
            "document.title='live work recovered';",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "live work recovered");
    assert!(page.report().contains("DOM work limit"));
}

#[test]
fn dynamic_function_callbacks_keep_document_string_compilation_policy() {
    for (header, allowed) in [
        ("script-src 'nonce-original-function-fixture'", false),
        ("script-src 'nonce-original-function-fixture' 'unsafe-eval'", true),
        ("default-src 'none'; script-src-elem 'nonce-original-function-fixture' 'unsafe-eval'", false),
        ("script-src 'nonce-original-function-fixture' 'unsafe-eval', script-src 'nonce-original-function-fixture'", false),
    ] {
        let mut policy = crate::csp::PolicySet::default();
        policy.add_header(header);
        let mut doc = crate::html::parse("<p id=out></p>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let location = crate::navigation::address(
            "http://127.0.0.1:1/function-policy",
            &crate::navigation::Location::home(),
        ).unwrap();
        page.set_location(&location);
        let source = r#"
var log='';
function attempt(label){
try{
var body="document.getElementById('out').textContent+=label;";
var compiled=label==='timer'?new Function('label',body):Function('label',body);
compiled(label);log+=label+':allowed;';
}catch(error){if(error.name!=='EvalError')throw error;log+=label+':blocked;';}
document.title=log;
}
attempt('script');
Promise.resolve().then(function(){attempt('promise');});
setTimeout(function(){attempt('timer');},0);
"#;
        assert!(policy.allows_inline(0, Some("original-function-fixture"), source).is_ok());
        let scripts = [Script { node: 0, label: "original Function CSP fixture".into(), source: source.into() }];
        let outcome = page.start_with_policy(&mut doc, &mut forms, &scripts, &policy);
        assert_eq!(outcome.errors, 0, "{header}: {}", page.report());
        let result = if allowed { "allowed" } else { "blocked" };
        assert_eq!(doc.title, format!("script:{result};promise:{result};"));
        let outcome = page.poll_timer_at(&mut doc, &mut forms, Instant::now() + Duration::from_secs(1)).unwrap();
        assert_eq!(outcome.errors, 0, "{header}: {}", page.report());
        assert_eq!(doc.title, format!("script:{result};promise:{result};timer:{result};"));
        let out = doc.nodes.iter().enumerate().find_map(|(id, _)| (doc.attr(id, "id") == Some("out")).then_some(id)).unwrap();
        let text = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }.text_content(out).unwrap();
        assert_eq!(text, if allowed { "scriptpromisetimer" } else { "" });
    }
}


#[test]
fn dynamic_crossorigin_mode_is_snapshotted_and_cross_origin_is_still_denied() {
    let (_, _, mut page, outcome) = page("<html><head></head><body></body></html>", &[r#"
var first=document.createElement('script');first.async=false;first.crossOrigin='anonymous';first.src='/one.js';
document.head.appendChild(first);first.removeAttribute('crossorigin');
var second=document.createElement('script');second.async=false;second.crossOrigin='use-credentials';second.src='/two.js';document.head.appendChild(second);
var third=document.createElement('script');third.async=false;third.crossOrigin='anonymous';third.src='https://other.test/blocked.js';document.head.appendChild(third);
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let requests = page.take_dynamic_scripts();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].same_origin_only && requests[0].error.is_none());
    assert!(requests[1].same_origin_only && requests[1].error.is_none());
    assert!(requests[2].same_origin_only);
    assert!(requests[2].error.as_deref().unwrap().contains("CORS-mode"));
}
