//! Generic Document identity is independent from the HTML/XML document flag.
use super::*;
fn fixture()->(Document,Forms,Page){let mut doc=crate::html::parse("<!doctype html><html><body><span id=mount></span><input id=field value=seed></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();page.set_location(&crate::navigation::address("https://attribute-factories.test/source",&crate::navigation::Location::home()).unwrap());assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){assert_eq!(page.console_eval(doc,forms,source).errors,0,"{source}\n{}",page.report());}


#[test]
fn generic_document_constructor_real_interface_defaults_xml_factories_and_clones(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var calls=0,C=Document,first=new C({toString:function(){calls++;return 'ignored';}}),second=new C();if(calls||first===second||!(first instanceof Document)||first instanceof XMLDocument||!(first instanceof Node)||Object.getPrototypeOf(first)!==Document.prototype||first.nodeType!==9||first.nodeName!=='#document'||first.ownerDocument!==null||first.parentNode!==null||first.defaultView!==null||first.documentElement!==null||first.doctype!==null||first.head!==null||first.body!==null||first.title!==''||first.URL!=='about:blank'||first.documentURI!=='about:blank'||first.baseURI!=='about:blank'||first.contentType!=='application/xml'||first.characterSet!=='UTF-8'||first.compatMode!=='CSS1Compat'||first.implementation===second.implementation)throw 'genuine generic Document defaults and independent implementation';var root=first.createElement('MiXeD'),attr=first.createAttribute('MiXeD');attr.value='case';root.setAttributeNode(attr);root.appendChild(first.createCDATASection('\ud800'));first.appendChild(root);if(root.namespaceURI!==null||root.tagName!=='MiXeD'||attr.name!=='MiXeD'||root.ownerDocument!==first||root.firstChild.ownerDocument!==first)throw 'generic XML kind is not XMLDocument interface';var clone=first.cloneNode(true),shallow=first.cloneNode(false);if(clone===first||clone instanceof XMLDocument||Object.getPrototypeOf(clone)!==Document.prototype||clone.documentElement===root||clone.documentElement.ownerDocument!==clone||clone.documentElement.tagName!=='MiXeD'||clone.documentElement.firstChild.data.charCodeAt(0)!==0xd800||clone.documentElement.getAttributeNode('MiXeD').ownerDocument!==clone||shallow.documentElement!==null||shallow instanceof XMLDocument)throw 'generic document clones retain interface kind metadata and owner topology';var xml=document.implementation.createDocument(null,'Root');if(!(xml instanceof XMLDocument)||!(xml.cloneNode() instanceof XMLDocument))throw 'XMLDocument factory interface retained';
"#);
}
#[test]
fn generic_document_clone_of_template_owner_is_not_marked_algorithm_created(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var ns='http://www.w3.org/1999/xhtml',xml=document.implementation.createDocument(null,''),xmlOwner=xml.createElementNS(ns,'template').content.ownerDocument,htmlOwner=document.createElement('template').content.ownerDocument;for(var owner of [xmlOwner,htmlOwner]){var copy=owner.cloneNode(true);if(copy===owner||copy instanceof XMLDocument||copy.contentType!=='application/xml'||copy.URL!=='about:blank'||copy.compatMode!=='CSS1Compat'||copy.defaultView!==null||copy.documentElement!==null||copy.implementation===owner.implementation)throw 'genuine generic template owner clone';var template=copy.createElementNS(ns,'template'),associated=template.content.ownerDocument;if(associated===copy||associated===owner||associated.createAttribute('MiXeD').name!==(owner===xmlOwner?'MiXeD':'mixed'))throw 'cloned document is not marked created by appropriate-owner algorithm';if(copy.createAttribute('MiXeD').name!==(owner===xmlOwner?'MiXeD':'mixed'))throw 'clone preserves actual HTML/XML flag independent of MIME';}
"#);
}
#[test]
fn generic_document_constructor_requires_new_and_preserves_unimplemented_realm_guard(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var calls=0,caught=false;try{Document({toString:function(){calls++;return 'ignored';}});}catch(e){caught=e instanceof TypeError;}if(!caught||calls)throw 'constructor requires new before author work';caught=false;try{new XMLDocument();}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'XMLDocument remains illegal constructor';var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);caught=false;try{new frame.contentWindow.Document();}catch(e){caught=e.name==='NotSupportedError';}if(!caught)throw 'child realm backend is not faked with parent prototype';
"#);
}

#[test]
fn generic_document_saved_child_factory_origin_precedes_author_conversion(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var owner=new Document(),frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var saved=frame.contentWindow.Document.prototype.createAttribute,calls=0,name={toString:function(){calls++;return 'MiXeD';}},attr=saved.call(owner,name);if(attr.name!=='MiXeD'||attr.ownerDocument!==owner||calls!==1)throw 'real generic document factory';");page.set_location(&crate::navigation::address("https://other-generic-document.test/",&crate::navigation::Location::home()).unwrap());let before=(doc.nodes.len(),page.state.attributes.len(),page.state.dom_commits);assert_eq!(page.console_eval(&mut doc,&mut forms,"saved.call(owner,name);").errors,1);assert!(page.report().contains("same-origin")||page.report().contains("Security"),"{}",page.report());assert_eq!((doc.nodes.len(),page.state.attributes.len(),page.state.dom_commits),before);run(&mut doc,&mut forms,&mut page,"if(calls!==1||attr.name!=='MiXeD'||owner.contentType!=='application/xml')throw 'origin before generic document factory conversion';");
}

#[test]
fn generic_document_native_subclass_remains_explicitly_unsupported(){
 let(mut doc,mut forms,mut page)=fixture();let before=(doc.nodes.len(),page.state.dom_commits);assert_eq!(page.console_eval(&mut doc,&mut forms,"class Derived extends Document{};new Derived();").errors,1);assert!(page.report().contains("class base is not implemented"),"{}",page.report());assert_eq!((doc.nodes.len(),page.state.dom_commits),before);run(&mut doc,&mut forms,&mut page,"var ordinary=new Document();if(ordinary instanceof XMLDocument||ordinary.nodeType!==9)throw 'ordinary constructor remains usable';");
}
