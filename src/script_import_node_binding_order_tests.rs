//! Binding conversion precedes importNode algorithm rejection and source snapshot.
use super::*;
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str)->Value{p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap()}
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div>Main</div></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();p.set_location(&crate::navigation::address("https://import-order.test/source",&crate::navigation::Location::home()).unwrap());assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}

#[test]
fn import_node_binding_order_document_rejection_follows_all_dictionary_gets(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
for(var target of [document,new Document(),document.createElement('template').content.ownerDocument]){for(var source of [document,new Document()]){var order='',options={get customElementRegistry(){order+='r';return undefined;},get selfOnly(){order+='s';return false;}},error;try{target.importNode(source,options);}catch(e){error=e;}if(order!=='rs'||!(error instanceof DOMException)||error.name!=='NotSupportedError')throw 'dictionary conversion before prohibited Document algorithm';}}
"#);}

#[test]
fn import_node_binding_order_abrupt_getter_identity_precedes_prohibited_document(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
for(var target of [document,new Document()]){for(var member of ['customElementRegistry','selfOnly']){var marker={},hits=0,options={};Object.defineProperty(options,member,{get:function(){hits++;throw marker;}});var error;try{target.importNode(document,options);}catch(e){error=e;}if(error!==marker||hits!==1)throw 'same abrupt dictionary value precedes Document refusal';}}
"#);}

#[test]
fn import_node_binding_order_invalid_registry_stops_before_later_member_and_document_algorithm(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
for(var target of [document,new Document()]){for(var registry of [null,{},false,document,Element.prototype]){var hits=0,options={get customElementRegistry(){hits++;return registry;},get selfOnly(){hits+=100;return false;}},error;try{target.importNode(document,options);}catch(e){error=e;}if(!(error instanceof TypeError)||hits!==1)throw 'registry interface conversion before later member and Document algorithm';}}
"#);}

#[test]
fn import_node_binding_order_genuine_receiver_and_first_node_guard_precede_dictionary(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var method=Document.prototype.importNode,hits=0,options={get customElementRegistry(){hits++;return undefined;},get selfOnly(){hits++;return false;}};for(var source of [{},null,undefined,window,Node.prototype]){var error;try{method.call(document,source,options);}catch(e){error=e;}if(!(error instanceof TypeError)||hits)throw 'genuine first Node before dictionary';}for(var target of [{},null,window,document.body]){var error;try{method.call(target,document,options);}catch(e){error=e;}if(!(error instanceof TypeError)||hits)throw 'genuine Document receiver before dictionary';}
"#);}

#[test]
fn import_node_binding_order_boolean_null_defaults_and_object_self_only_use_native_clones(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var source=document.createElement('div'),text=document.createTextNode('raw'+String.fromCharCode(55296));source.appendChild(text);source.authorState={};for(var target of [document,new Document(),document.createElement('template').content.ownerDocument]){for(var options of [true,null,{}, {selfOnly:false},1,'deep']){var copy=target.importNode(source,options);if(copy===source||copy.ownerDocument!==target||copy.parentNode!==null||copy.firstChild===text||copy.firstChild.ownerDocument!==target||copy.firstChild.data.charCodeAt(3)!==55296||Object.hasOwn(copy,'authorState'))throw 'actual deep union copy';}for(var options of [false,undefined,{selfOnly:true},0,'']){var copy=target.importNode(source,options);if(copy.ownerDocument!==target||copy.childNodes.length)throw 'actual shallow union copy';}var shallow=target.importNode(source);if(shallow.childNodes.length)throw 'omitted default false';var hits=0,truth={toString:function(){hits++;throw 'no coercion';},valueOf:function(){hits++;throw 'no coercion';}},copy=target.importNode(source,{selfOnly:truth});if(hits||copy.childNodes.length)throw 'selfOnly boolean ToBoolean';}
"#);}

#[test]
fn import_node_binding_order_author_native_effects_survive_later_document_refusal(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var host=document.createElement('div');document.body.appendChild(host);var watch=new MutationObserver(function(){});watch.observe(host,{childList:true});var error;try{document.importNode(document,{get selfOnly(){host.appendChild(document.createTextNode('author effect'));return true;}});}catch(e){error=e;}var records=watch.takeRecords();if(!(error instanceof DOMException)||error.name!=='NotSupportedError'||host.textContent!=='author effect'||records.length!==1||records[0].target!==host||records[0].addedNodes[0]!==host.firstChild)throw 'actual author mutation retained before forbidden import';
"#);}

#[test]
fn import_node_binding_order_forbidden_document_constructs_no_native_clone_under_exhaustion(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var target=new Document(),calls=0,options={get selfOnly(){calls++;return false;}};");let limit=p.state.retained_limit;let native=(d.nodes.len(),p.state.dom_commits);p.state.retained_limit=p.state.retained;
 run(&mut d,&mut f,&mut p,"var error;try{target.importNode(document,options);}catch(e){error=e;}if(!(error instanceof DOMException)||error.name!=='NotSupportedError'||calls!==1)throw 'binding conversion before native storage admission';");assert_eq!((d.nodes.len(),p.state.dom_commits),native);p.state.retained_limit=limit;
 run(&mut d,&mut f,&mut p,"var source=document.createTextNode('recovered'),copy=target.importNode(source);if(copy===source||copy.ownerDocument!==target||copy.data!=='recovered')throw 'native clone recovery';");
}
