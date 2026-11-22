//! Native expanded-name identity, exact URI data and namespace authority.
use super::*;
fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse_loaded_document("<!doctype html><html><head></head><body><span id=mount></span><input id=field value=seed></body></html>",&||Ok(())).unwrap();doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();page.set_location(&crate::navigation::address("https://expanded-names.test/source",&crate::navigation::Location::home()).unwrap());
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){assert_eq!(page.console_eval(doc,forms,source).errors,0,"{source}\n{}",page.report());}
fn no_effects(page:&mut Page){assert!(!page.has_navigation_request());assert!(page.take_dynamic_scripts().is_empty()&&page.take_dynamic_stylesheets().is_empty());let(a,b,c)=page.take_image_changes();assert!(a.is_empty()&&b.is_empty()&&c.is_empty());}
#[test]
fn native_expanded_names_arbitrary_null_prefix_case_and_clone_use_actual_records(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var n=document.createElementNS('urn:original','p:Mixed'),copy=n.cloneNode(true);
if(n.namespaceURI!=='urn:original'||n.prefix!=='p'||n.localName!=='Mixed'||n.tagName!=='p:Mixed'||n.nodeName!=='p:Mixed'||n.ownerDocument!==document||n.parentNode!==null||n.isConnected||!(n instanceof Element)||n instanceof HTMLElement||Object.getPrototypeOf(n)!==Element.prototype)throw 'actual foreign node';
if(copy===n||copy.namespaceURI!==n.namespaceURI||copy.prefix!=='p'||copy.localName!=='Mixed'||copy.nodeName!==n.nodeName||copy.ownerDocument!==document)throw 'actual cloned expanded name';
for(var ns of [null,'']){var empty=document.createElementNS(ns,'MiXeD');if(empty.namespaceURI!==null||empty.prefix!==null||empty.localName!=='MiXeD'||empty.nodeName!=='MiXeD'||!empty.isDefaultNamespace(null))throw 'actual null namespace';}
var html=document.createElementNS('http://www.w3.org/1999/xhtml','h:DiV'),svg=document.createElementNS('http://www.w3.org/2000/svg','s:circle');if(html.localName!=='DiV'||html.prefix!=='h'||html.tagName!=='H:DIV'||html.nodeName!=='H:DIV'||svg.tagName!=='s:circle'||svg.localName!=='circle'||svg.prefix!=='s'||!(svg instanceof SVGElement)||html.cloneNode().prefix!=='h'||svg.cloneNode().namespaceURI!==svg.namespaceURI)throw 'known namespace prefixes';
if(html.isDefaultNamespace('http://www.w3.org/1999/xhtml')||!html.isDefaultNamespace(null))throw 'prefix is not a default declaration';
html.setAttributeNS('http://www.w3.org/2000/xmlns/','xmlns','urn:default');if(!html.isDefaultNamespace('urn:default')||html.isDefaultNamespace(null)||html.namespaceURI!=='http://www.w3.org/1999/xhtml')throw 'declaration differs from actual node namespace';
"#);no_effects(&mut page);
}
#[test]
fn native_expanded_names_uri_domstring_units_and_ordered_validation_are_exact(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var count='',n=document.createElementNS({toString:function(){count+='n';return '\ud800';}},{toString:function(){count+='q';return 'p:Mixed';}}),copy=n.cloneNode();
if(count!=='nq'||n.namespaceURI.length!==1||n.namespaceURI.charCodeAt(0)!==0xd800||copy.namespaceURI!==n.namespaceURI||n.prefix!=='p'||!n.isDefaultNamespace(null)||n.isDefaultNamespace('\ud800'))throw 'actual raw URI and prefix';
var plain=document.createElementNS('\udfff','Mixed');if(plain.namespaceURI.charCodeAt(0)!==0xdfff||!plain.isDefaultNamespace('\udfff')||plain.isDefaultNamespace('�'))throw 'URI is not its display projection';
var withNull=document.createElementNS('urn:a\0b','x');if(withNull.namespaceURI.length!==7||withNull.namespaceURI.charCodeAt(5)!==0||withNull.cloneNode().namespaceURI!==withNull.namespaceURI)throw 'URI is not a URL or terminated string';
var caught=false;count='';try{document.createElementNS({toString:function(){count+='n';return 'urn:wrong';}},{toString:function(){count+='q';return 'xml:Mixed';}});}catch(e){caught=e.name==='NamespaceError';}if(!caught||count!=='nq')throw 'ordered namespace validation';
caught=false;try{document.createElementNS(null,'p:Mixed');}catch(e){caught=e.name==='NamespaceError';}if(!caught||n.namespaceURI.charCodeAt(0)!==0xd800)throw 'prefix needs actual namespace';
"#);no_effects(&mut page);
}
#[test]
fn native_expanded_names_inert_owner_children_copy_and_known_prefixes_stay_real(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var t=document.createElement('template'),owner=t.content.ownerDocument,n=owner.createElementNS('urn:original','p:MiXeD'),text=owner.createTextNode('kept');n.appendChild(text);t.content.appendChild(n);var copy=n.cloneNode(true);
if(n.ownerDocument!==owner||n.parentNode!==t.content||n.prefix!=='p'||n.localName!=='MiXeD'||n.namespaceURI!=='urn:original'||n.firstChild!==text||n.textContent!=='kept'||copy===n||copy.firstChild===text||copy.ownerDocument!==owner||copy.firstChild.ownerDocument!==owner||copy.namespaceURI!==n.namespaceURI||copy.textContent!=='kept')throw 'real inert expanded records';
var html=owner.createElementNS('http://www.w3.org/1999/xhtml','h:div'),svg=owner.createElementNS('http://www.w3.org/2000/svg','s:circle');if(html.prefix!=='h'||html.tagName!=='H:DIV'||svg.prefix!=='s'||svg.tagName!=='s:circle')throw 'inert known namespace prefixes';
"#);no_effects(&mut page);
}
#[test]
fn native_expanded_names_foreign_html_lookalikes_have_no_html_interface_or_requests(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
for(var name of ['script','img','input','iframe','video','a','style','link','canvas']){
 var node=document.createElementNS('urn:unrelated',name);if(node.namespaceURI!=='urn:unrelated'||node.localName!==name||node.tagName!==name||node.ownerDocument!==document||node instanceof HTMLElement||node instanceof SVGElement||Object.getPrototypeOf(node)!==Element.prototype||node.src!==undefined||node.href!==undefined||node.value!==undefined||node.contentWindow!==undefined)throw 'foreign names do not grant HTML interfaces';
 var caught=false;try{document.getElementById('mount').appendChild(node);}catch(e){caught=true;}if(!caught||node.parentNode!==null||document.getElementById('mount').childNodes.length)throw 'unsupported activation is atomic';
}
for(var name of ['SCRIPT','IMG','INPUT','IFRAME','VIDEO']){var unknown=document.createElementNS('http://www.w3.org/1999/xhtml',name),copy=unknown.cloneNode();if(copy.localName!==name||copy.src!==undefined||copy.value!==undefined||copy.contentWindow!==undefined||copy.parentNode!==null)throw 'case preserved unknown data has no built-in state';}

"#);no_effects(&mut page);
}
#[test]
fn native_expanded_names_admission_refusal_has_no_record_prefix_and_recovers(){
    let(mut doc,mut forms,mut page)=fixture();let before=(doc.nodes.len(),page.state.retained,page.state.dom_commits);let limit=page.state.retained_limit;page.state.retained_limit=page.state.retained;
    assert_eq!(page.console_eval(&mut doc,&mut forms,"document.createElementNS('urn:original','p:Mixed');").errors,1);assert_eq!((doc.nodes.len(),page.state.retained,page.state.dom_commits),before);page.state.retained_limit=limit;
    run(&mut doc,&mut forms,&mut page,"var recovered=document.createElementNS('urn:original','p:Mixed');if(recovered.namespaceURI!=='urn:original'||recovered.prefix!=='p'||recovered.localName!=='Mixed'||recovered.parentNode!==null)throw 'fresh job namespace recovery';");no_effects(&mut page);
}

#[test]
fn native_expanded_names_native_payload_and_backing_are_charged_for_prefixed_factories(){
    let(mut doc,mut forms,mut page)=fixture();
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let backing=|bridge:&Bridge<'_>|bridge.doc.nodes.capacity()*size_of::<Node>()+bridge.state.node_state_bytes()+bridge.state.node_sets_storage_bytes();
    for (namespace,name) in [("http://www.w3.org/1999/xhtml","h:div"),("http://www.w3.org/1999/xhtml","h:img"),("http://www.w3.org/2000/svg","s:circle")]{
        let before=bridge.state.retained;let previous=backing(&bridge);
        let Value::Host(handle)=bridge.create_expanded_element(DOCUMENT,&[Value::String(namespace.into()),Value::String(name.into())]).unwrap()else{panic!("real named node")};
        let node=&bridge.doc.nodes[node_index(handle).unwrap()];let NodeKind::Element{tag,href,expanded_name,..}=&node.kind else{panic!("actual Element")};
        assert_eq!(expanded_name.len(),1);assert_eq!(expanded_name[0].prefix.as_deref(),Some(&name[..1]));
        let payload=node_bytes(node)+tag.capacity()+href.as_ref().map_or(0,String::capacity);
        assert_eq!(bridge.state.retained-before,payload+backing(&bridge)-previous,"{name}");
    }
}

#[test]
fn native_expanded_names_canonical_namespace_method_brands_nulls_and_attributes_stay_native(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var query=Node.prototype.isDefaultNamespace,node=document.createElementNS('urn:original','Mixed'),calls=0;
if(typeof query!=='function'||query.length!==1||query.name!=='isDefaultNamespace'||!query.call(node,'urn:original')||query.call(node,null)||query.call(node,''))throw 'canonical namespace method';
for(var wrong of [{},window,node.style,undefined]){var caught=false;try{query.call(wrong,{toString:function(){calls++;return 'urn:original';}});}catch(e){caught=e instanceof TypeError;}if(!caught||calls)throw 'brand before conversion';}
var parent=document.createElementNS('http://www.w3.org/1999/xhtml','h:div');parent.setAttributeNS('http://www.w3.org/2000/xmlns/','xmlns','urn:default');var attr=parent.getAttributeNodeNS('http://www.w3.org/2000/xmlns/','xmlns');if(!query.call(attr,'urn:default')||attr.ownerElement!==parent||parent.namespaceURI!=='http://www.w3.org/1999/xhtml')throw 'actual attached attribute namespace context';
if(!query.call(document.createDocumentFragment(),null)||!query.call(document.doctype,null))throw 'non-element namespace context';
"#);no_effects(&mut page);
}
