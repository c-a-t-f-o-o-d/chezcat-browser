//! Canonical readonly dataset accessors retain their actual element and owner.
use super::*;
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str)->Value{p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap()}
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><p id=target data-first-name='old'></p><svg id=shape data-note=svg></svg><form id=owner><input name=dataset></form></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();let location=crate::navigation::address("https://dataset-accessor.test/source",&crate::navigation::Location::home()).unwrap();p.set_location(&location);p.set_resource_base(&location);assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}

#[test]
fn dataset_native_accessors_canonical_readonly_descriptors_and_same_object(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var html=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'dataset'),svg=Object.getOwnPropertyDescriptor(SVGElement.prototype,'dataset'),p=document.getElementById('target'),shape=document.getElementById('shape');for(var d of [html,svg]){if(!d||typeof d.get!=='function'||d.get.name!=='get dataset'||d.get.length!==0||d.set!==undefined||!d.enumerable||!d.configurable)throw 'canonical readonly descriptor';}if(html.get===svg.get||Object.hasOwn(Element.prototype,'dataset')||Object.hasOwn(p,'dataset')||Object.hasOwn(shape,'dataset')||html.get.call(p)!==p.dataset||svg.get.call(shape)!==shape.dataset||html.get.call(p)===svg.get.call(shape)||!(p.dataset instanceof DOMStringMap)||shape.dataset.note!=='svg')throw 'same actual element-backed objects';
"#);}

#[test]
fn dataset_native_accessors_genuine_declaring_interface_brands(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var html=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'dataset').get,svg=Object.getOwnPropertyDescriptor(SVGElement.prototype,'dataset').get,p=document.getElementById('target'),shape=document.getElementById('shape');for(var pair of [[html,shape],[svg,p],[html,{}],[svg,{}],[html,document],[svg,document],[html,HTMLElement.prototype],[svg,SVGElement.prototype],[html,Object.create(HTMLElement.prototype)],[svg,p.dataset],[html,null],[html,undefined]]){var error;try{pair[0].call(pair[1]);}catch(e){error=e;}if(!(error instanceof TypeError))throw 'genuine per-interface native brand';}if(html.call(p)!==p.dataset||svg.call(shape)!==shape.dataset)throw 'brand refusal recovery';
"#);}

#[test]
fn dataset_native_accessors_readonly_assignment_does_not_convert_or_replace(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var p=document.getElementById('target'),saved=p.dataset,calls=0,value={toString:function(){calls++;throw 'no conversion';}};p.dataset=value;if(calls||p.dataset!==saved||Object.hasOwn(p,'dataset'))throw 'sloppy readonly assignment';var error;try{(function(){'use strict';p.dataset=value;})();}catch(e){error=e;}if(!(error instanceof TypeError)||calls||p.dataset!==saved)throw 'strict readonly native accessor';
"#);}

#[test]
fn dataset_native_accessors_author_own_and_prototype_shadows_preserve_saved_getter(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var p=document.getElementById('target'),desc=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'dataset'),get=desc.get,saved=get.call(p);Object.defineProperty(p,'dataset',{value:'own',writable:true,configurable:true});if(p.dataset!=='own'||get.call(p)!==saved)throw 'author own shadow';delete p.dataset;if(p.dataset!==saved)throw 'own shadow deletion';Object.defineProperty(HTMLElement.prototype,'dataset',{get:function(){return 'prototype';},configurable:true});if(p.dataset!=='prototype'||get.call(p)!==saved)throw 'author prototype shadow';delete HTMLElement.prototype.dataset;if(p.dataset!==undefined||get.call(p)!==saved)throw 'prototype deletion';saved.firstName='live';if(p.getAttribute('data-first-name')!=='live')throw 'saved actual backing after author shadows';Object.defineProperty(HTMLElement.prototype,'dataset',desc);if(p.dataset!==saved)throw 'canonical descriptor restoration';
"#);}

#[test]
fn dataset_native_accessors_named_form_precedence_and_saved_native_getter(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var form=document.getElementById('owner'),control=form.getElementsByTagName('input')[0],get=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'dataset').get;if(form.dataset!==control)throw 'form named-control precedence';var native=get.call(form);if(!(native instanceof DOMStringMap)||native===control.dataset)throw 'saved getter real form owner';native.actual='form';if(form.getAttribute('data-actual')!=='form'||control.hasAttribute('data-actual'))throw 'actual native form dataset backing';
"#);}

#[test]
fn dataset_native_accessors_saved_child_getter_uses_main_backing_and_origin_guards(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var get=Object.getOwnPropertyDescriptor(frame.contentWindow.HTMLElement.prototype,'dataset').get,p=document.getElementById('target');if(get.call(p)!==p.dataset||!(get.call(p) instanceof DOMStringMap)||get.call(p) instanceof frame.contentWindow.DOMStringMap)throw 'captured getter owner does not replace actual backing owner';var error;try{get.call(document.getElementById('shape'));}catch(e){error=e;}if(!(error instanceof frame.contentWindow.TypeError))throw 'captured child getter TypeError realm';
"#);}

#[test]
fn dataset_native_accessors_no_attribute_copy_admission_and_work_refusal_recover(){
 let(mut d,mut f,mut p)=fixture();let node=(0..d.nodes.len()).find(|&n|d.attr(n,"id")==Some("target")).unwrap();let before=(p.state.retained,p.state.dom_commits);let limit=p.state.retained_limit;p.state.retained_limit=p.state.retained;p.begin_job();p.state.copy_work=0;
 let value=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.call(node_handle(node),"\0HTMLElement.get.dataset",&[]).unwrap();assert!(matches!(value,Value::Host(handle)if handle==DATASET_BASE+node as u32));assert_eq!((p.state.retained,p.state.dom_commits),before);
 p.begin_job();p.state.host_work=0;let error=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.call(node_handle(node),"\0HTMLElement.get.dataset",&[]).unwrap_err();assert!(error.contains("work"),"{error}");assert_eq!((p.state.retained,p.state.dom_commits),before);p.state.retained_limit=limit;
 run(&mut d,&mut f,&mut p,"var get=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'dataset').get,node=document.getElementById('target');get.call(node).firstName='recovered';if(node.dataset.firstName!=='recovered')throw 'native getter recovery';");
}

#[test]
fn dataset_native_accessors_saved_child_origin_refuses_without_native_change_then_recovers(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var get=Object.getOwnPropertyDescriptor(frame.contentWindow.HTMLElement.prototype,'dataset').get,node=document.getElementById('target'),data=get.call(node);if(data!==node.dataset)throw 'same-origin saved getter';");let before=format!("{:?}",d.nodes);let native=(p.state.retained,p.state.dom_commits);
 p.set_location(&crate::navigation::address("https://foreign-dataset-accessor.test/",&crate::navigation::Location::home()).unwrap());p.begin_job();let error=p.runtime.execute("get.call(node);",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(error.contains("origin")||error.contains("Security"),"{error}");assert_eq!(format!("{:?}",d.nodes),before);assert_eq!((p.state.retained,p.state.dom_commits),native);
 p.set_location(&crate::navigation::address("https://dataset-accessor.test/source",&crate::navigation::Location::home()).unwrap());run(&mut d,&mut f,&mut p,"if(get.call(node)!==data||node.dataset!==data)throw 'captured getter origin recovery';");
}

#[test]
fn dataset_native_accessors_other_owner_backing_refusal_preserves_real_attributes(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
 var html=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'dataset').get,svg=Object.getOwnPropertyDescriptor(SVGElement.prototype,'dataset').get,owner=document.implementation.createHTMLDocument('inert'),element=owner.createElement('p'),shape=owner.createElementNS('http://www.w3.org/2000/svg','svg');element.setAttribute('data-value','inert');shape.setAttribute('data-value','inert-svg');for(var pair of [[html,element],[svg,shape]]){var error;try{pair[0].call(pair[1]);}catch(e){error=e;}if(!error||error.name!=='NotSupportedError'||!(error instanceof DOMException))throw 'explicit native backing integration guard '+(pair[0]===html?'HTML ':'SVG ')+(error?error.name+':'+error.message:'missing error');}var xml=new Document().createElement('p'),wrong;try{html.call(xml);}catch(e){wrong=e;}if(!(wrong instanceof TypeError))throw 'generic XML Element is not HTMLElement';if(element.getAttribute('data-value')!=='inert'||shape.getAttribute('data-value')!=='inert-svg')throw 'guard preserves original native attributes';var node=document.getElementById('target');if(html.call(node)!==node.dataset)throw 'main getter after other owner refusal';
 "#);
}
