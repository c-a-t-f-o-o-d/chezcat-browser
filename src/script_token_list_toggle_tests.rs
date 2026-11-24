//! Canonical DOMTokenList toggle behavior and native boundaries.
use super::*;
fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse("<!doctype html><html><body><p id=target class='one two one'></p><iframe id=frame sandbox='allow-scripts'></iframe></body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();let location=crate::navigation::address("https://token-native.test/source",&crate::navigation::Location::home()).unwrap();page.set_location(&location);page.set_resource_base(&location);assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn token_list_toggle_canonical_metadata_shadows_deletion_and_saved_brand(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var P=DOMTokenList.prototype,d=Object.getOwnPropertyDescriptor(P,'toggle'),list=document.getElementById('target').classList,calls=0;
if(!d||typeof d.value!=='function'||d.value.name!=='toggle'||d.value.length!==1||!d.writable||!d.enumerable||!d.configurable||Object.prototype.hasOwnProperty.call(list,'toggle'))throw 'canonical toggle';
var toggle=d.value;delete P.toggle;if(list.toggle!==undefined||'toggle' in list)throw 'deleted toggle resurrection';if(toggle.call(list,'one')!==false||list.contains('one'))throw 'saved toggle';
P.toggle=function(){calls++;return 'shadow';};if(list.toggle()!=='shadow'||calls!==1)throw 'prototype shadow';list.toggle=17;if(list.toggle!==17)throw 'own shadow';delete list.toggle;P.toggle=toggle;
try{toggle.call({}, {toString:function(){calls++;return 'bad';}});throw 'missing brand';}catch(e){if(!(e instanceof TypeError))throw e;}if(calls!==1)throw 'brand before conversion';
"#);}
#[test]
fn token_list_toggle_raw_identity_force_presence_and_boolean_conversion(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),list=p.classList;p.className='A\ud800z A�z';
if(list.toggle('A\ud800z')!==false||list.value!=='A�z'||list.toggle('A\ud800z')!==true||list.value!=='A�z A\ud800z')throw 'exact toggle';
if(list.toggle('A\ud800z',undefined)!==false||list.value!=='A�z'||list.toggle('next',false)!==false||list.contains('next'))throw 'present undefined force';
var force={toString:function(){throw 'force must not stringify';},valueOf:function(){throw 'force must not number';}};
if(list.toggle('next',force)!==true||list.toggle('next',Symbol('truthy'))!==true||list.toggle('next',0)!==false)throw 'Boolean force';
if(list.toggle('x\u000by')!==true||!list.contains('x\u000by')||list.toggle('😺')!==true||!p.matches('.😺'))throw 'native valid tokens';
"#);}
#[test]
fn token_list_toggle_forced_noops_preserve_record_and_observer_update_is_single(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),raw=' A\ud800z A\ud800z ';p.className=raw;var a=p.getAttributeNode('class'),o=new MutationObserver(function(){});o.observe(p,{attributes:true,attributeOldValue:true});
if(p.classList.toggle('A\ud800z',true)!==true||p.classList.toggle('missing',false)!==false||p.className!==raw||o.takeRecords().length!==0)throw 'forced no-op normalized or notified';
if(p.classList.toggle('A\ud800z',false)!==false)throw 'forced removal result';var rows=o.takeRecords();if(rows.length!==1||rows[0].oldValue!==raw||p.className!==''||a!==p.getAttributeNode('class')||!p.hasAttribute('class'))throw 'single native update';
var e=document.createElement('p');if(e.classList.toggle('absent',false)!==false||e.hasAttribute('class'))throw 'absent no-op';
"#);}
#[test]
fn token_list_toggle_once_conversion_reentry_abrupt_and_validation_order(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),calls=0,marker={};
if(p.classList.toggle({toString:function(){calls++;p.className='A\ud800z';return 'A\ud800z';}})!==false||calls!==1||p.className!=='')throw 'conversion before live membership';
try{p.classList.toggle({toString:function(){calls++;p.className='retained';throw marker;}});throw 'missing abrupt';}catch(e){if(e!==marker)throw e;}if(calls!==2||p.className!=='retained')throw 'abrupt effects';
for(var i=0;i<2;i++){try{p.classList.toggle(i===0?'':'bad token',false);throw 'missing token validation';}catch(e){if(e.name!==(i===0?'SyntaxError':'InvalidCharacterError'))throw e;}}
try{p.classList.toggle();throw 'missing required token';}catch(e){if(!(e instanceof TypeError))throw e;}
if(p.className!=='retained')throw 'invalid token committed';
"#);}

#[test]
fn token_list_toggle_noop_needs_no_retained_headroom_but_pays_read_work(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var p=document.getElementById('target');p.className=' A\\ud800z A\\ud800z ';");
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("target")).unwrap();let list=script_class_list::handle(node_handle(node));let before=doc.dom_attr(node,"class").unwrap().try_clone().unwrap();let retained=page.state.retained;let commits=page.state.dom_commits;
    page.begin_job();page.state.copy_work=0;
    let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.class_list_call(list,"toggle",&[Value::StringUnits(vec![0x41,0xd800,0x7a]),Value::Bool(true)]).unwrap_err();assert!(error.contains("work limit"),"{error}");
    assert_eq!(doc.dom_attr(node,"class"),Some(&before));assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);
    let token=Value::StringUnits(vec![0x41,0xd800,0x7a]);
    page.begin_job();page.state.retained_limit=retained;
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    assert!(matches!(bridge.class_list_call(list,"toggle",&[token,Value::Bool(true)]).unwrap(),Value::Bool(true)));
    assert!(matches!(bridge.class_list_call(list,"toggle",&[Value::String("absent".into()),Value::Bool(false)]).unwrap(),Value::Bool(false)));
    assert_eq!(doc.dom_attr(node,"class"),Some(&before));assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);
}
#[test]
fn token_list_toggle_mutation_refusal_keeps_exact_record_and_fresh_job_recovers(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var p=document.getElementById('target');p.className='A\\ud800z';");
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("target")).unwrap();let list=script_class_list::handle(node_handle(node));let before=doc.dom_attr(node,"class").unwrap().try_clone().unwrap();let retained=page.state.retained;let limit=page.state.retained_limit;let commits=page.state.dom_commits;
    for mode in 0..3 {
        page.begin_job();if mode==0{page.state.copy_work=0;}else if mode==1{page.state.host_work=0;}else{page.state.retained_limit=retained;}
        let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.class_list_call(list,"toggle",&[Value::String("tail".into()),Value::Bool(true)]).unwrap_err();assert!(error.contains("work limit")||error.contains("allowance"),"{error}");
        assert_eq!(doc.dom_attr(node,"class"),Some(&before));assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);page.state.retained_limit=limit;
    }
    page.begin_job();run(&mut doc,&mut forms,&mut page,"if(p.classList.toggle('tail',true)!==true||p.className!=='A\\ud800z tail')throw 'fresh toggle recovery';");
}
#[test]
fn token_list_toggle_saved_child_origin_and_other_token_policies_remain(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),list=p.classList,frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var toggle=frame.contentWindow.DOMTokenList.prototype.toggle,calls=0,probe={toString:function(){calls++;return 'B\udfffz';}};if(toggle.call(list,'A\ud800z',true)!==true)throw 'saved child toggle';
var svg=document.createElementNS('http://www.w3.org/2000/svg','g');svg.setAttribute('class','before');var sandbox=document.createElement('iframe').sandbox;
if(svg.classList.toggle('after')!==true||svg.classList.toggle('before',false)!==false||sandbox.toggle('allow-forms',true)!==true||sandbox.supports('allow-scripts'))throw 'scalar token policies';
"#);
    let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    for source in ["svg.classList.toggle('A\\ud800z')","sandbox.toggle('allow-scripts\\ud800')"] {page.begin_job();let error=page.runtime.execute(source,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("UTF-16"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);}
    page.set_location(&crate::navigation::address("https://foreign-toggle.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();let error=page.runtime.execute("toggle.call(list,probe)",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("origin")||error.contains("Security"),"{error}");assert_eq!(page.state.dom_commits,commits);
    page.set_location(&crate::navigation::address("https://token-native.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();run(&mut doc,&mut forms,&mut page,"if(calls!==0||!list.contains('A\\ud800z'))throw 'authority before toggle conversion';");
}
