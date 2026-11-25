//! Param name reflection preserves actual metadata.
use super::*;
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse("<html><body><param id=param name=original><input id=neighbor value=seed></body></html>");d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}

#[test]
fn param_name_canonical_interface_descriptor_and_parsed_attribute(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"var x=document.getElementById('param'),C=HTMLParamElement,P=C.prototype,a=Object.getOwnPropertyDescriptor(P,'name');if(!(x instanceof C)||Object.getPrototypeOf(x)!==P||Object.getPrototypeOf(P)!==HTMLElement.prototype||!a||!a.enumerable||!a.configurable||a.get.name!=='get name'||a.set.name!=='set name'||a.get.length!==0||a.set.length!==1||Object.hasOwn(x,'name')||x.name!=='original'||x.firstChild!==null)throw 'canonical Param name';"#);
}
#[test]
fn param_name_native_exact_domstring_attr_and_observer_share_one_record(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"var x=document.getElementById('param'),attr=x.getAttributeNode('name'),watch=new MutationObserver(function(){});watch.observe(x,{attributes:true,attributeOldValue:true});x.name='A\udfffZ';var records=watch.takeRecords();if(x.name!=='A\udfffZ'||x.getAttribute('name')!=='A\udfffZ'||x.getAttributeNode('name')!==attr||attr.value!==x.name||records.length!==1||records[0].attributeName!=='name'||records[0].oldValue!=='original')throw 'actual Param name backing';attr.nodeValue='alias';if(x.name!=='alias'||x.getAttributeNode('name')!==attr)throw 'attached Param Attr';x.removeAttribute('name');if(x.name!==''||attr.ownerElement!==null||document.getElementById('neighbor').value!=='seed')throw 'missing Param metadata';"#);
}
#[test]
fn param_name_inert_exact_namespace_and_once_conversion_keep_source_owner(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"var owner=new Document(),x=owner.createElementNS('http://www.w3.org/1999/xhtml','param'),calls=0;x.name={toString:function(){calls++;return 'A\ud800Z';}};var attr=x.getAttributeNode('name');if(calls!==1||!(x instanceof HTMLParamElement)||x.name!=='A\ud800Z'||!attr||attr.value!==x.name||attr.ownerElement!==x||attr.ownerDocument!==owner||x.ownerDocument!==owner||x.parentNode!==null)throw 'actual inert Param metadata';"#);
}

#[test]
fn param_name_genuine_brand_exact_namespace_and_abrupt_conversion_guards(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var x=document.getElementById('param'),descriptor=Object.getOwnPropertyDescriptor(HTMLParamElement.prototype,'name'),attr=x.getAttributeNode('name'),owner=new Document();
for(var bad of [{},Object.create(HTMLParamElement.prototype),document.createElement('div'),document.createElementNS('urn:foreign','param'),owner.createElementNS('http://www.w3.org/1999/xhtml','PARAM')]){
 var caught=false;try{descriptor.get.call(bad);}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'genuine Param getter brand';
 caught=false;try{descriptor.set.call(bad,{toString:function(){throw 'unrequested conversion';}});}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'Param brand precedes conversion';
}
var caught=false;try{x.name=Symbol();}catch(e){caught=e instanceof TypeError;}if(!caught||x.name!=='original'||x.getAttributeNode('name')!==attr)throw 'Symbol preserves Param record';
var marker={},calls=0;caught=false;try{x.name={toString:function(){calls++;x.setAttribute('data-effect','kept');throw marker;}};}catch(e){caught=e===marker;}if(!caught||calls!==1||x.name!=='original'||attr.value!=='original'||x.getAttribute('data-effect')!=='kept')throw 'abrupt effects and original Param name';
var caught=false;try{new HTMLParamElement();}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'unsupported native HTML constructor';
"#);
}

#[test]
fn param_name_native_and_inert_resource_refusals_preserve_real_records_and_recover(){
 for inert in [false,true]{for mode in 0..4{
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,if inert{"var owner=new Document(),x=owner.createElementNS('http://www.w3.org/1999/xhtml','param');x.name='seed';"}else{"var x=document.getElementById('param');x.name='seed';"});
  run(&mut d,&mut f,&mut p,"var attr=x.getAttributeNode('name'),watch=new MutationObserver(function(){});watch.observe(x,{attributes:true,attributeOldValue:true});");
  p.state.rebuild(&d).unwrap();let before=format!("{:?}",d.nodes);let state=(p.state.retained,p.state.dom_commits,p.state.attributes.len());let limit=p.state.retained_limit;
  p.begin_job();match mode{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=p.state.retained,_=>p.state.execution_deadline=Some(Instant::now())};
  let e=p.runtime.execute("x.name='long A\\ud800z B\\udfffz metadata';",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(e.contains("work")||e.contains("allowance")||e.contains("limit")||e.contains("bound")||e.contains("time"),"{inert}/{mode}: {e}");
  assert_eq!(format!("{:?}",d.nodes),before);assert_eq!((p.state.retained,p.state.dom_commits,p.state.attributes.len()),state);
  p.state.retained_limit=limit;p.state.execution_deadline=None;p.begin_job();
  run(&mut d,&mut f,&mut p,"if(x.name!=='seed'||x.getAttributeNode('name')!==attr||attr.value!=='seed'||watch.takeRecords().length)throw 'atomic Param refusal';x.name='fresh';var records=watch.takeRecords();if(x.name!=='fresh'||x.getAttributeNode('name')!==attr||attr.value!=='fresh'||records.length!==1||records[0].oldValue!=='seed')throw 'Param fresh recovery';");
  assert!(!p.has_navigation_request());assert!(p.take_dynamic_scripts().is_empty()&&p.take_dynamic_stylesheets().is_empty());assert!(!p.state.images.has_changes());
 }}
}

#[test]
fn param_name_saved_child_origin_precedes_conversion_and_keeps_canonical_identity(){
 let(mut d,mut f,mut p)=fixture();let location=crate::navigation::address("https://param-name.test/source",&crate::navigation::Location::home()).unwrap();p.set_location(&location);
 run(&mut d,&mut f,&mut p,"var x=document.getElementById('param'),frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var saved=Object.getOwnPropertyDescriptor(frame.contentWindow.HTMLParamElement.prototype,'name'),C=HTMLParamElement,calls=0;saved.set.call(x,'same');delete window.HTMLParamElement;if(!(x instanceof C)||saved.get.call(x)!=='same')throw 'saved Param canonical identity';");
 p.set_location(&crate::navigation::address("https://other-param.test/",&crate::navigation::Location::home()).unwrap());let before=format!("{:?}",d.nodes);let state=(p.state.retained,p.state.dom_commits);p.begin_job();
 let e=p.runtime.execute("saved.set.call(x,{toString:function(){calls++;return 'bad';}});",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(e.contains("origin")||e.contains("Security"),"{e}");
 assert_eq!(format!("{:?}",d.nodes),before);assert_eq!((p.state.retained,p.state.dom_commits),state);p.set_location(&location);p.begin_job();
 run(&mut d,&mut f,&mut p,"if(calls||saved.get.call(x)!=='same')throw 'Param origin precedes coercion';saved.set.call(x,'fresh');if(x.name!=='fresh'||!(x instanceof C))throw 'captured Param recovery';");
}

#[test]
fn param_name_constructor_owner_realms_and_global_key_order(){
 let(mut d,mut f,mut p)=fixture();p.set_location(&crate::navigation::address("https://param-realm.test/",&crate::navigation::Location::home()).unwrap());run(&mut d,&mut f,&mut p,r#"
var x=document.getElementById('param'),P=Object.getPrototypeOf(x),C=P.constructor;
var names=Object.getOwnPropertyNames(window),position=names.indexOf('HTMLParamElement');
if(position<0||position<names.indexOf('HTMLMapElement')||position>names.indexOf('HTMLModElement')||Object.keys(window).indexOf('HTMLParamElement')>=0||HTMLParamElement!==C)throw 'reserved global order';
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var child=frame.contentWindow,D=child.HTMLParamElement,Q=D.prototype;
if(D===C||Q.constructor!==D||Object.getPrototypeOf(D)!==child.HTMLElement||Object.getPrototypeOf(Q)!==child.HTMLElement.prototype)throw 'owner realm constructor';
var saved=Object.getOwnPropertyDescriptor(Q,'name');saved.set.call(x,'cross realm');if(x.name!=='cross realm'||!(x instanceof C)||Object.getPrototypeOf(x)!==P)throw 'native anchor stable';
if(!child.Function("var C=HTMLParamElement;window.HTMLParamElement=17;delete window.HTMLParamElement;return C.prototype.constructor===C&&typeof HTMLParamElement==='undefined';")()||Q.constructor!==D||child.HTMLParamElement!==undefined)throw 'child mutation preserves canonical prototype';
"#);
}
