//! Remaining Param metadata uses actual DOMString attributes.
use super::*;
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse("<html><body><param id=param value=original type=original valuetype=original><input id=neighbor value=seed></body></html>");d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
fn native(property:&str,attribute:&str){
 let(mut d,mut f,mut p)=fixture();let source=r#"
var x=document.getElementById('param'),property='PROPERTY',attribute='ATTRIBUTE',descriptor=Object.getOwnPropertyDescriptor(HTMLParamElement.prototype,property),attr=x.getAttributeNode(attribute);
if(!(x instanceof HTMLParamElement)||!descriptor||descriptor.get.name!=='get '+property||descriptor.set.name!=='set '+property||!descriptor.enumerable||!descriptor.configurable||descriptor.get.length!==0||descriptor.set.length!==1||x[property]!=='original'||Object.hasOwn(x,property))throw 'canonical Param metadata';
var watch=new MutationObserver(function(){});watch.observe(x,{attributes:true,attributeOldValue:true});var calls=0;x[property]={toString:function(){calls++;return 'A\udfffZ';}};var records=watch.takeRecords();
if(calls!==1||x[property]!=='A\udfffZ'||attr.value!==x[property]||x.getAttributeNode(attribute)!==attr||records.length!==1||records[0].attributeName!==attribute||records[0].oldValue!=='original')throw 'actual Param metadata transaction';
attr.nodeValue='alias';if(x[property]!=='alias'||x.getAttributeNode(attribute)!==attr)throw 'attached Param metadata Attr';x.removeAttribute(attribute);if(x[property]!==''||attr.ownerElement!==null||document.getElementById('neighbor').value!=='seed')throw 'Param metadata absence';
"#.replace("PROPERTY",property).replace("ATTRIBUTE",attribute);run(&mut d,&mut f,&mut p,&source);
}
#[test]fn param_metadata_value_actual_attribute(){native("value","value");}
#[test]fn param_metadata_type_actual_attribute(){native("type","type");}
#[test]fn param_metadata_value_type_actual_attribute(){native("valueType","valuetype");}
#[test]fn param_metadata_inert_exact_namespace_and_attribute_case(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var owner=new Document(),x=owner.createElementNS('http://www.w3.org/1999/xhtml','param');
for(var pair of [['value','value'],['type','type'],['valueType','valuetype']]){var property=pair[0],attribute=pair[1];x[property]='A\ud800Z';var attr=x.getAttributeNode(attribute);if(x[property]!=='A\ud800Z'||!attr||attr.value!==x[property]||attr.ownerDocument!==owner||attr.ownerElement!==x||x.ownerDocument!==owner||x.parentNode!==null)throw 'actual inert Param metadata';x.setAttributeNS('urn:separate',attribute,'foreign');if(x[property]!=='A\ud800Z')throw 'exact attribute namespace';x.removeAttribute(attribute);if(x[property]!=='')throw 'inert absence';}
"#);
}

#[test]fn param_metadata_brands_abrupt_conversion_and_unrestricted_domstrings(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var x=document.getElementById('param'),owner=new Document();
for(var pair of [['value','value'],['type','type'],['valueType','valuetype']]){var property=pair[0],attribute=pair[1],descriptor=Object.getOwnPropertyDescriptor(HTMLParamElement.prototype,property),attr=x.getAttributeNode(attribute);
 for(var bad of [{},Object.create(HTMLParamElement.prototype),document.createElement('div'),document.createElementNS('urn:foreign','param'),owner.createElementNS('http://www.w3.org/1999/xhtml','PARAM')]){var caught=false;try{descriptor.get.call(bad);}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'Param metadata getter brand';caught=false;try{descriptor.set.call(bad,{toString:function(){throw 'unrequested conversion';}});}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'Param metadata brand precedes conversion';}
 var caught=false;try{x[property]=Symbol();}catch(e){caught=e instanceof TypeError;}if(!caught||x[property]!=='original'||attr.value!=='original'||x.getAttributeNode(attribute)!==attr)throw 'Symbol preserves metadata';
 var marker={},calls=0;caught=false;try{x[property]={toString:function(){calls++;x.setAttribute('data-effect','kept');throw marker;}};}catch(e){caught=e===marker;}if(!caught||calls!==1||x[property]!=='original'||x.getAttribute('data-effect')!=='kept')throw 'abrupt Param metadata';
 descriptor.set.call(x);if(x[property]!=='undefined')throw 'missing setter argument';descriptor.set.call(x,null,Symbol());if(x[property]!=='null')throw 'null DOMString and ignored extra argument';x[property]='MiXeD/Unrecognized';if(descriptor.get.call(x,Symbol())!=='MiXeD/Unrecognized')throw 'unrestricted reflected DOMString';
}
"#);
}

#[test]fn param_metadata_resource_refusals_preserve_records_and_recover(){
 for(property,attribute)in[("value","value"),("type","type"),("valueType","valuetype")]{for inert in[false,true]{for mode in 0..4{
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,&format!("var property='{property}',attribute='{attribute}';"));
  run(&mut d,&mut f,&mut p,if inert{"var owner=new Document(),x=owner.createElementNS('http://www.w3.org/1999/xhtml','param');x[property]='seed';"}else{"var x=document.getElementById('param');x[property]='seed';"});
  run(&mut d,&mut f,&mut p,"var attr=x.getAttributeNode(attribute),watch=new MutationObserver(function(){});watch.observe(x,{attributes:true,attributeOldValue:true});");p.state.rebuild(&d).unwrap();let nodes=format!("{:?}",d.nodes);let state=(p.state.retained,p.state.dom_commits,p.state.attributes.len());let limit=p.state.retained_limit;
  p.begin_job();match mode{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=p.state.retained,_=>p.state.execution_deadline=Some(Instant::now())};
  let e=p.runtime.execute("x[property]='long A\\ud800z B\\udfffz metadata';",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(e.contains("work")||e.contains("allowance")||e.contains("limit")||e.contains("bound")||e.contains("time"),"{property}/{inert}/{mode}: {e}");assert_eq!(format!("{:?}",d.nodes),nodes);assert_eq!((p.state.retained,p.state.dom_commits,p.state.attributes.len()),state);
  p.state.retained_limit=limit;p.state.execution_deadline=None;p.begin_job();run(&mut d,&mut f,&mut p,"if(x[property]!=='seed'||x.getAttributeNode(attribute)!==attr||attr.value!=='seed'||watch.takeRecords().length)throw 'atomic metadata refusal';x[property]='fresh';var records=watch.takeRecords();if(x[property]!=='fresh'||x.getAttributeNode(attribute)!==attr||attr.value!=='fresh'||records.length!==1||records[0].attributeName!==attribute||records[0].oldValue!=='seed')throw 'fresh metadata recovery';");assert!(!p.has_navigation_request());assert!(p.take_dynamic_scripts().is_empty()&&p.take_dynamic_stylesheets().is_empty());assert!(!p.state.images.has_changes());
 }}}
}

#[test]fn param_metadata_saved_child_origin_precedes_conversion(){
 for property in["value","type","valueType"]{
  let(mut d,mut f,mut p)=fixture();let location=crate::navigation::address("https://param-metadata.test/source",&crate::navigation::Location::home()).unwrap();p.set_location(&location);
  run(&mut d,&mut f,&mut p,&format!("var property='{property}';"));run(&mut d,&mut f,&mut p,"var x=document.getElementById('param'),frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var saved=Object.getOwnPropertyDescriptor(frame.contentWindow.HTMLParamElement.prototype,property),C=HTMLParamElement,calls=0;saved.set.call(x,'same');delete window.HTMLParamElement;if(!(x instanceof C)||saved.get.call(x)!=='same')throw 'saved metadata';");
  p.set_location(&crate::navigation::address("https://other-param-metadata.test/",&crate::navigation::Location::home()).unwrap());let nodes=format!("{:?}",d.nodes);let state=(p.state.retained,p.state.dom_commits);p.begin_job();let e=p.runtime.execute("saved.set.call(x,{toString:function(){calls++;return 'bad';}});",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(e.contains("origin")||e.contains("Security"),"{e}");assert_eq!(format!("{:?}",d.nodes),nodes);assert_eq!((p.state.retained,p.state.dom_commits),state);
  p.set_location(&location);p.begin_job();run(&mut d,&mut f,&mut p,"if(calls||saved.get.call(x)!=='same')throw 'origin precedes coercion';saved.set.call(x,'fresh');if(x[property]!=='fresh'||!(x instanceof C))throw 'captured metadata recovery';");
 }
}
