//! Public main-document HTML class DOMString producers and consumers.
use super::*;
fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse("<!doctype html><html><body><p id=target class='one two one'></p><iframe id=frame sandbox='allow-scripts'></iframe></body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();let location=crate::navigation::address("https://token-native.test/source",&crate::navigation::Location::home()).unwrap();page.set_location(&location);page.set_resource_base(&location);assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn html_class_domstring_public_attribute_reflection_and_attr_identity(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),raw=' A\ud800z plain A😺 ',a=p.getAttributeNode('class');
p.setAttribute('class',raw);if(p.className!==raw||p.getAttribute('class')!==raw||a.value!==raw||p.getAttributeNode('class')!==a||p.classList.value!==raw)throw 'setAttribute exact record';
p.className='B\udfffz';if(a.value!=='B\udfffz'||p.classList[0]!=='B\udfffz'||p.classList.contains('B�z'))throw 'className exact record';
a.value=raw;if(p.className!==raw||p.classList.item(0)!=='A\ud800z'||!p.classList.contains('A😺')||p.classList.length!==3)throw 'attached Attr producer';
p.setAttributeNS(null,'class','C\ud800z');if(p.className!=='C\ud800z'||p.getAttributeNode('class')!==a)throw 'null namespace producer';
var fresh=document.createElement('p');fresh.className=raw;if(fresh.className!==raw||fresh.classList.value!==raw)throw 'created element producer';
"#);}
#[test]
fn html_class_domstring_token_value_saved_setter_and_put_forwards_keep_once_conversion(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),list=p.classList,d=Object.getOwnPropertyDescriptor(DOMTokenList.prototype,'value'),calls=0;
list.value={toString:function(){calls++;return 'A\ud800z plain';}};if(calls!==1||p.className!=='A\ud800z plain'||list[0]!=='A\ud800z')throw 'list value conversion';
d.set.call(list,'B\udfffz');if(d.get.call(list)!=='B\udfffz'||p.className!=='B\udfffz')throw 'saved setter';
p.classList={toString:function(){calls++;return 'C\ud800z';}};if(calls!==2||p.classList!==list||p.className!=='C\ud800z')throw 'PutForwards exact conversion';
var marker={};try{p.className={toString:function(){throw marker;}};throw 'missing abrupt';}catch(e){if(e!==marker)throw e;}if(p.className!=='C\ud800z')throw 'abrupt commit';
try{d.set.call({}, {toString:function(){calls++;return 'bad';}});throw 'missing brand';}catch(e){if(!(e instanceof TypeError))throw e;}if(calls!==2)throw 'brand before conversion';
"#);}
#[test]
fn html_class_domstring_observer_clone_serialization_and_exact_selectors(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),raw='plain A\ud800z',a=p.getAttributeNode('class'),observer=new MutationObserver(function(){});observer.observe(p,{attributes:true,attributeOldValue:true});
p.className=raw;var rows=observer.takeRecords();if(rows.length!==1||rows[0].oldValue!=='one two one'||rows[0].attributeName!=='class')throw 'first old value';
if(!p.matches('.plain')||p.matches('.A�z')||!p.matches(':not(.A�z)')||document.querySelectorAll('.A�z').length!==0||document.getElementsByClassName('A\ud800z')[0]!==p)throw 'exact selector consumers';
var clone=p.cloneNode(true);if(clone.className!==raw||clone.getAttributeNode('class')===a||clone.outerHTML.indexOf('A\ud800z')<0)throw 'clone serialization';
p.className='A�z';rows=observer.takeRecords();if(rows.length!==1||rows[0].oldValue!==raw||!p.matches('.A�z')||clone.className!==raw||a.value!=='A�z')throw 'live scalar replacement';
"#);}
#[test]
fn html_class_domstring_raw_token_mutation_and_unapproved_attributes_preserve_exact_record(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"var p=document.getElementById('target'),frame=document.getElementById('frame');p.className='A\ud800z plain';"#);
    run(&mut doc,&mut forms,&mut page,"p.classList.add('next');if(p.className!=='A\\ud800z plain next')throw 'exact supported token mutation';");
    let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    for source in ["frame.setAttribute('sandbox','allow-scripts\\ud800')","p.setAttribute('style','color:red\\ud800')"] {
        page.begin_job();let error=page.runtime.execute(source,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();
        assert!(error.contains("UTF-16")||error.contains("Unpaired"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);
    }
    page.begin_job();run(&mut doc,&mut forms,&mut page,"p.className='plain';p.classList.add('next');if(p.className!=='plain next'||frame.sandbox.supports('allow-scripts'))throw 'fresh recovery and sandbox authority';");
}

#[test]
fn html_class_domstring_native_copy_retained_work_and_output_refusal_recover(){
    let(mut doc,mut forms,mut page)=fixture();let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("target")).unwrap();
    let original=doc.dom_attr(node,"class").unwrap().try_clone().unwrap();let retained=page.state.retained;let limit=page.state.retained_limit;let commits=page.state.dom_commits;
    let raw=Value::StringUnits("longer class ".encode_utf16().chain([0xd800]).chain(" payload".encode_utf16()).collect());
    for mode in 0..3 {
        page.begin_job();if mode==0{page.state.copy_work=0;}else if mode==1{page.state.retained_limit=retained;}else{page.state.host_work=0;}
        let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.html_class_set(node,&raw).unwrap_err();
        assert!(error.contains("work limit")||error.contains("retained")||error.contains("allowance"),"{error}");
        assert_eq!(doc.dom_attr(node,"class"),Some(&original));assert_eq!(page.state.dom_commits,commits);assert_eq!(page.state.retained,retained);page.state.retained_limit=limit;
    }
    page.begin_job();Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.html_class_set(node,&raw).unwrap();
    assert!(doc.dom_attr(node,"class").unwrap().raw_units().is_some());let committed=page.state.dom_commits;let held=page.state.retained;let string_limit=page.state.limits.host.string_bytes;
    page.begin_job();page.state.limits.host.string_bytes=1;
    let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.class_value(node,"class").unwrap_err();assert!(error.contains("string allowance"),"{error}");assert_eq!(page.state.dom_commits,committed);assert_eq!(page.state.retained,held);
    page.state.limits.host.string_bytes=string_limit;page.begin_job();
    let value=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.class_value(node,"class").unwrap();assert!(matches!((value,raw),(Value::StringUnits(a),Value::StringUnits(b)) if a==b));
}
#[test]
fn html_class_domstring_saved_child_value_accessors_recheck_origin_before_conversion(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"var p=document.getElementById('target'),calls=0,probe={toString:function(){calls++;return 'other\ud800';}};p.className='A\ud800z';var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var childValue=Object.getOwnPropertyDescriptor(frame.contentWindow.DOMTokenList.prototype,'value'),get=childValue.get,set=childValue.set,list=p.classList;if(get.call(list)!==p.className)throw 'same origin child getter';"#);
    let commits=page.state.dom_commits;page.set_location(&crate::navigation::address("https://foreign-class.test/source",&crate::navigation::Location::home()).unwrap());
    for source in ["get.call(list)","set.call(list,probe)"] {page.begin_job();let error=page.runtime.execute(source,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("origin")||error.contains("Security"),"{error}");assert_eq!(page.state.dom_commits,commits);}
    page.set_location(&crate::navigation::address("https://token-native.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();run(&mut doc,&mut forms,&mut page,"if(calls!==0||p.className!=='A\\ud800z')throw 'foreign call committed';");
}
#[test]
fn html_class_domstring_attribute_node_replacement_removal_and_svg_boundary(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),old=p.getAttributeNode('class'),next=document.createAttribute('class');next.value='A\ud800z';
if(p.setAttributeNode(next)!==old||old.ownerElement!==null||next.ownerElement!==p||p.className!=='A\ud800z')throw 'replacement identity';
if(p.removeAttributeNode(next)!==next||next.ownerElement!==null||next.value!=='A\ud800z'||p.className!==''||p.classList.length!==0)throw 'removal exact detached value';
var svg=document.createElementNS('http://www.w3.org/2000/svg','g');svg.setAttribute('class','before');
"#);
    let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    for source in ["svg.setAttribute('class','A\\ud800z')","svg.className='A\\ud800z'"] {
        page.begin_job();let error=page.runtime.execute(source,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();
        assert!(error.contains("Unpaired")||error.contains("UTF-16"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);
    }
}
