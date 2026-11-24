//! Canonical DOMTokenList toggle behavior and native boundaries.
use super::*;
fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse("<!doctype html><html><body><p id=target class='one two one'></p><iframe id=frame sandbox='allow-scripts'></iframe></body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();let location=crate::navigation::address("https://token-native.test/source",&crate::navigation::Location::home()).unwrap();page.set_location(&location);page.set_resource_base(&location);assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn token_list_replace_canonical_metadata_deletion_shadows_and_brand(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var P=DOMTokenList.prototype,d=Object.getOwnPropertyDescriptor(P,'replace'),list=document.getElementById('target').classList,calls=0;
if(!d||typeof d.value!=='function'||d.value.name!=='replace'||d.value.length!==2||!d.writable||!d.enumerable||!d.configurable||Object.prototype.hasOwnProperty.call(list,'replace'))throw 'canonical replace';
var replace=d.value;delete P.replace;if(list.replace!==undefined||'replace' in list)throw 'deleted replace resurrection';if(replace.call(list,'one','next')!==true||list.value!=='next two')throw 'saved replace';
P.replace=function(){calls++;return 'shadow';};if(list.replace()!=='shadow'||calls!==1)throw 'prototype shadow';list.replace=17;if(list.replace!==17)throw 'own shadow';delete list.replace;P.replace=replace;
try{replace.call({}, {toString:function(){calls++;return 'old';}},'new');throw 'missing brand';}catch(e){if(!(e instanceof TypeError))throw e;}if(calls!==1)throw 'brand before conversion';
"#);}
#[test]
fn token_list_replace_exact_order_first_old_or_new_and_duplicate_normalization(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),l=p.classList;
p.className=' old middle new old ';if(l.replace('old','new')!==true||p.className!=='new middle')throw 'old first';
p.className=' new middle old new ';if(l.replace('old','new')!==true||p.className!=='new middle')throw 'new first';
p.className=' A\ud800z middle A�z A\ud800z ';if(l.replace('A\ud800z','A�z')!==true||p.className!=='A�z middle'||l.contains('A\ud800z'))throw 'exact old first';
p.className=' A�z middle A\ud800z ';if(l.replace('A\ud800z','A�z')!==true||p.className!=='A�z middle')throw 'exact new first';
p.className=' A\ud800z A\ud800z ';if(l.replace('A\ud800z','A\ud800z')!==true||p.className!=='A\ud800z')throw 'same token update';
if(l.replace('A\ud800z','😺')!==true||!p.matches('.😺')||l.replace('😺','x\u000by')!==true||l[0]!=='x\u000by')throw 'valid token identity';
"#);}
#[test]
fn token_list_replace_absent_preserves_spelling_and_success_is_one_observed_update(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target');p.className=' A\ud800z A\ud800z ';var a=p.getAttributeNode('class'),o=new MutationObserver(function(){});o.observe(p,{attributes:true,attributeOldValue:true});
if(p.classList.replace('absent','next')!==false||p.className!==' A\ud800z A\ud800z '||o.takeRecords().length!==0)throw 'missing token updated';
if(p.classList.replace('A\ud800z','A�z')!==true)throw 'replacement result';var rows=o.takeRecords();if(rows.length!==1||rows[0].oldValue!==' A\ud800z A\ud800z '||p.className!=='A�z'||p.getAttributeNode('class')!==a||a.value!==p.className)throw 'single exact native update';
if(p.classList.replace('A�z','A�z')!==true)throw 'equal replacement result';rows=o.takeRecords();if(rows.length!==1||rows[0].oldValue!=='A�z'||p.className!=='A�z')throw 'equal replacement must update';
var e=document.createElement('p');if(e.classList.replace('absent','next')!==false||e.hasAttribute('class'))throw 'absent attribute';
"#);}
#[test]
fn token_list_replace_both_conversions_precede_global_validation_and_live_membership(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),calls='',marker={};
if(p.classList.replace({toString:function(){calls+='a';return 'A\ud800z';}},{toString:function(){calls+='b';p.className='A\ud800z';return 'next';}})!==true||calls!=='ab'||p.className!=='next')throw 'ordered live conversion';
try{p.classList.replace('bad token',{toString:function(){calls+='c';p.className='retained';return '';}});throw 'missing empty';}catch(e){if(e.name!=='SyntaxError')throw e;}if(calls!=='abc'||p.className!=='retained')throw 'global empty priority';
try{p.classList.replace({toString:function(){calls+='d';p.className='effect';return '';}},{toString:function(){calls+='e';throw marker;}});throw 'missing abrupt';}catch(e){if(e!==marker)throw e;}if(calls!=='abcde'||p.className!=='effect')throw 'abrupt identity effects';
try{p.classList.replace('absent','bad token');throw 'missing invalid';}catch(e){if(e.name!=='InvalidCharacterError')throw e;}
try{p.classList.replace('only');throw 'missing required';}catch(e){if(!(e instanceof TypeError))throw e;}
if(p.classList.replace('effect','last',{toString:function(){throw 'excess argument converted';}})!==true||p.className!=='last')throw 'excess argument';
"#);}

#[test]
fn token_list_replace_workspace_copy_work_and_final_string_refusals_recover(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var p=document.getElementById('target');p.className='A\\ud800z';");
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("target")).unwrap();let list=script_class_list::handle(node_handle(node));let before=doc.dom_attr(node,"class").unwrap().try_clone().unwrap();let retained=page.state.retained;let limit=page.state.retained_limit;let commits=page.state.dom_commits;let string=page.state.limits.host.string_bytes;
    let metadata=size_of::<crate::dom_string::DomStringRef<'_>>();let scratch=22;let output=11;
    for mode in 0..5 {
        page.begin_job();if mode==0{page.state.copy_work=0;}else if mode==1{page.state.host_work=0;}else if mode==2{page.state.retained_limit=retained+metadata-1;}else if mode==3{page.state.retained_limit=retained+metadata+scratch+output-1;}else{page.state.limits.host.string_bytes=6;}
        let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.class_list_call(list,"replace",&[Value::StringUnits(vec![0x41,0xd800,0x7a]),Value::String("tail-beyond".into())]).unwrap_err();assert!(error.contains("work limit")||error.contains("allowance")||error.contains("string bound"),"{error}");
        assert_eq!(doc.dom_attr(node,"class"),Some(&before));assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);page.state.retained_limit=limit;page.state.limits.host.string_bytes=string;
    }
    page.begin_job();run(&mut doc,&mut forms,&mut page,"if(p.classList.replace('A\\ud800z','tail-beyond')!==true||p.className!=='tail-beyond')throw 'fresh replacement recovery';");
}
#[test]
fn token_list_replace_missing_old_needs_no_retained_headroom_but_pays_read_work(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var p=document.getElementById('target');p.className=' A\\ud800z A\\ud800z ';");
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("target")).unwrap();let list=script_class_list::handle(node_handle(node));let before=doc.dom_attr(node,"class").unwrap().try_clone().unwrap();let retained=page.state.retained;let commits=page.state.dom_commits;let args=[Value::String("absent".into()),Value::String("next".into())];
    page.begin_job();page.state.copy_work=0;
    let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.class_list_call(list,"replace",&args).unwrap_err();assert!(error.contains("work limit"),"{error}");
    page.begin_job();page.state.retained_limit=retained;
    assert!(matches!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.class_list_call(list,"replace",&args).unwrap(),Value::Bool(false)));
    assert_eq!(doc.dom_attr(node,"class"),Some(&before));assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);
}
#[test]
fn token_list_replace_saved_child_origin_and_svg_sandbox_guards_remain(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),list=p.classList,frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var replace=frame.contentWindow.DOMTokenList.prototype.replace,calls=0,probe={toString:function(){calls++;return 'B\udfffz';}};p.className='A\ud800z';if(replace.call(list,'A\ud800z','B\udfffz')!==true)throw 'saved child replace';
var svg=document.createElementNS('http://www.w3.org/2000/svg','g');svg.setAttribute('class','before');var sandbox=document.createElement('iframe').sandbox;sandbox.value='allow-scripts';
if(svg.classList.replace('before','after')!==true||svg.getAttribute('class')!=='after'||sandbox.replace('allow-scripts','allow-forms')!==true||sandbox.value!=='allow-forms'||sandbox.supports('allow-forms'))throw 'scalar attribute policies';
"#);
    let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    for (source,expected) in [("svg.classList.replace('after','A\\ud800z')","UTF-16"),("sandbox.replace('allow-forms','allow-scripts\\ud800')","UTF-16"),("document.getElementById('frame').sandbox.replace('allow-scripts','allow-forms')","Changing connected blank-frame")] {
        page.begin_job();let error=page.runtime.execute(source,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains(expected),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);
    }
    page.set_location(&crate::navigation::address("https://foreign-replace.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();let error=page.runtime.execute("replace.call(list,probe,'next')",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("origin")||error.contains("Security"),"{error}");assert_eq!(page.state.dom_commits,commits);
    page.set_location(&crate::navigation::address("https://token-native.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();run(&mut doc,&mut forms,&mut page,"if(calls!==0||list.value!=='B\\udfffz')throw 'authority before replacement conversion';");
}
