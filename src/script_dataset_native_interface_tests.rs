//! Native DOMStringMap roots retain the real dataset named-property backing.
use super::*;
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str)->Value{p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap()}
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><p data-first-name='old'></p></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();let location=crate::navigation::address("https://dataset-interface.test/source",&crate::navigation::Location::home()).unwrap();p.set_location(&location);p.set_resource_base(&location);assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}

#[test]
fn dataset_native_interface_canonical_roots_and_descriptors(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
if(typeof DOMStringMap!=='function'||window.DOMStringMap!==DOMStringMap)throw 'native interface object';var proto=DOMStringMap.prototype,node=document.querySelector('p'),data=node.dataset,other=document.createElement('div').dataset;if(Object.getPrototypeOf(proto)!==Object.prototype||Object.getPrototypeOf(data)!==proto||Object.getPrototypeOf(other)!==proto||data===other||data!==node.dataset||!(data instanceof DOMStringMap)||!(data instanceof Object))throw 'canonical actual dataset ancestry';var c=Object.getOwnPropertyDescriptor(proto,'constructor'),p=Object.getOwnPropertyDescriptor(DOMStringMap,'prototype'),tag=Object.getOwnPropertyDescriptor(proto,Symbol.toStringTag),global=Object.getOwnPropertyDescriptor(window,'DOMStringMap');if(c.value!==DOMStringMap||c.enumerable||!c.writable||!c.configurable||p.value!==proto||p.writable||p.enumerable||p.configurable||tag.value!=='DOMStringMap'||tag.writable||tag.enumerable||!tag.configurable||global.value!==DOMStringMap||global.enumerable||!global.writable||!global.configurable||DOMStringMap.name!=='DOMStringMap'||DOMStringMap.length!==0)throw 'WebIDL interface descriptors';if(Object.prototype.toString.call(data)!=='[object DOMStringMap]'||Object.keys(proto).length)throw 'native prototype tag';
"#);}

#[test]
fn dataset_native_interface_illegal_constructor_preserves_argument_identity(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var saved=DOMStringMap,calls=0,value={toString:function(){calls++;throw 'no conversion';}};for(var construct of [false,true]){var error;try{if(construct)new saved(value);else saved(value);}catch(e){error=e;}if(!(error instanceof TypeError)||calls)throw 'illegal native interface construction';}var data=document.querySelector('p').dataset;data.firstName='recovered';if(!(data instanceof saved)||data.firstName!=='recovered')throw 'native backing after illegal construction';
"#);}

#[test]
fn dataset_native_interface_supported_keys_override_author_prototype_accessors(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var proto=DOMStringMap.prototype,node=document.querySelector('p'),data=node.dataset,hits=0;Object.defineProperty(proto,'shadow',{get:function(){hits++;return 'inherited';},set:function(){throw 'author setter';},enumerable:true,configurable:true});if(data.shadow!=='inherited'||hits!==1||Object.hasOwn(data,'shadow')||node.hasAttribute('data-shadow'))throw 'author prototype is not a content attribute';data.shadow={toString:function(){return 'native';}};if(data.shadow!=='native'||hits!==1||node.getAttribute('data-shadow')!=='native'||Object.getOwnPropertyDescriptor(data,'shadow').value!=='native')throw 'LegacyOverrideBuiltIns actual named property';delete data.shadow;if(data.shadow!=='inherited'||hits!==2||Object.hasOwn(data,'shadow')||Object.keys(data).join(',')!=='firstName')throw 'native deletion restores author prototype';delete proto.shadow;
"#);}

#[test]
fn dataset_native_interface_constructor_and_object_names_are_real_attributes(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var node=document.querySelector('p'),data=node.dataset,saved=DOMStringMap,proto=saved.prototype;data.constructor='content';data.toString='text';data.hasOwnProperty='own';if(data.constructor!=='content'||data.toString!=='text'||data.hasOwnProperty!=='own'||!(data instanceof saved)||Object.getPrototypeOf(data)!==proto||Object.keys(data).join(',')!=='firstName,constructor,toString,hasOwnProperty')throw 'native named override retains ancestry';var watch=new MutationObserver(function(){});watch.observe(node,{attributes:true,attributeOldValue:true});delete data.constructor;delete data.toString;delete data.hasOwnProperty;if(data.constructor!==saved||data.toString!==Object.prototype.toString||data.hasOwnProperty!==Object.prototype.hasOwnProperty||watch.takeRecords().length!==3)throw 'real deletions restore canonical inherited names';
"#);}

#[test]
fn dataset_native_interface_saved_roots_survive_author_global_replacement(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var saved=DOMStringMap,proto=saved.prototype,node=document.querySelector('p'),data=node.dataset;DOMStringMap=function Author(){};if(Object.getPrototypeOf(data)!==proto||data.constructor!==saved||!(data instanceof saved))throw 'author global does not replace native roots';delete window.DOMStringMap;var other=document.createElement('div').dataset;if(Object.getPrototypeOf(other)!==proto||other.constructor!==saved||!(other instanceof saved))throw 'native roots after global deletion';other.fresh='attribute';if(Object.keys(other).join(',')!=='fresh')throw 'new native dataset remains live';
"#);}

#[test]
fn dataset_native_interface_child_window_roots_are_distinct_from_main_backing(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var child=frame.contentWindow.DOMStringMap,data=document.querySelector('p').dataset;if(typeof child!=='function'||child===DOMStringMap||child.prototype===DOMStringMap.prototype||!(data instanceof DOMStringMap)||data instanceof child||Object.getPrototypeOf(child.prototype)!==frame.contentWindow.Object.prototype)throw 'owner-specific native interface roots';var error;try{new child();}catch(e){error=e;}if(!(error instanceof frame.contentWindow.TypeError))throw 'child illegal constructor realm';
"#);}
