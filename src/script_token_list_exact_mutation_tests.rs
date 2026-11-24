//! Exact main-document HTML token add/remove mutations.
use super::*;
fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse("<!doctype html><html><body><p id=target class='one two one'></p><iframe id=frame sandbox='allow-scripts'></iframe></body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();let location=crate::navigation::address("https://token-native.test/source",&crate::navigation::Location::home()).unwrap();page.set_location(&location);page.set_resource_base(&location);assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn token_list_exact_mutation_ordered_set_preserves_raw_scalar_and_supplementary_identity(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target');p.className=' one A\ud800z one A�z 😺 ';
p.classList.add('last');if(p.className!=='one A\ud800z A�z 😺 last')throw 'existing raw token normalization';
p.classList.add('A\ud800z','A\udfffz','A�z','x\u0000y','x\u000by','x\u00a0y');
if(p.className!=='one A\ud800z A�z 😺 last A\udfffz x\u0000y x\u000by x\u00a0y'||p.classList.length!==9)throw 'exact ordered add';
p.classList.remove('A�z','A\udfffz');if(!p.classList.contains('A\ud800z')||p.classList.contains('A�z')||p.matches('.A�z')||!p.matches('.😺'))throw 'exact removal';
p.classList.remove('A\ud800z');if(p.classList.contains('A\ud800z'))throw 'raw token removal';
"#);}
#[test]
fn token_list_exact_mutation_converts_all_arguments_before_validation_and_keeps_abrupt_effects(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),calls='',marker={};
try{p.classList.add({toString:function(){calls+='a';p.className='late';return 'A\ud800z';}},{toString:function(){calls+='b';throw marker;}});throw 'missing abrupt';}catch(e){if(e!==marker)throw e;}
if(calls!=='ab'||p.className!=='late')throw 'abrupt retained effects';
try{p.classList.add('A\ud800z','',{toString:function(){calls+='c';p.className='later';return 'last';}});throw 'missing syntax';}catch(e){if(e.name!=='SyntaxError')throw e;}
if(calls!=='abc'||p.className!=='later')throw 'all argument conversion before validation';
try{p.classList.remove('A\ud800z','bad token');throw 'missing invalid token';}catch(e){if(e.name!=='InvalidCharacterError')throw e;}if(p.className!=='later')throw 'atomic token validation';
var add=DOMTokenList.prototype.add;try{add.call({}, {toString:function(){calls+='wrong';return 'A\ud800z';}});throw 'missing brand';}catch(e){if(!(e instanceof TypeError))throw e;}if(calls!=='abc')throw 'brand before conversion';
"#);}
#[test]
fn token_list_exact_mutation_empty_arguments_attribute_presence_and_old_values(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var e=document.createElement('p');e.classList.remove('A\ud800z');e.classList.add();if(e.hasAttribute('class'))throw 'absent empty set';
e.className=' A\ud800z A\ud800z ';var a=e.getAttributeNode('class'),o=new MutationObserver(function(){});o.observe(e,{attributes:true,attributeOldValue:true});e.classList.add();var rows=o.takeRecords();
if(e.className!=='A\ud800z'||a.value!==e.className||rows.length!==1||rows[0].oldValue!==' A\ud800z A\ud800z ')throw 'zero argument update';
e.classList.remove('A\ud800z');rows=o.takeRecords();if(!e.hasAttribute('class')||e.className!==''||e.getAttributeNode('class')!==a||rows.length!==1||rows[0].oldValue!=='A\ud800z')throw 'empty present attribute';
"#);}
#[test]
fn token_list_exact_mutation_live_clone_collection_and_observer_uses_actual_record(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),list=p.classList;p.className='plain A\ud800z';var clone=p.cloneNode(true),o=new MutationObserver(function(){});o.observe(p,{attributes:true,attributeOldValue:true});
list.add('A�z');var rows=o.takeRecords();if(rows.length!==1||rows[0].oldValue!=='plain A\ud800z'||p.className!=='plain A\ud800z A�z'||!p.matches('.A�z')||clone.className!=='plain A\ud800z')throw 'exact observer and clone';
list.remove('A\ud800z');if(document.getElementsByClassName('A\ud800z').length!==0||document.getElementsByClassName('A�z')[0]!==p||list.value!=='plain A�z'||Array.from(list).join('|')!=='plain|A�z')throw 'actual live collections';
"#);}

#[test]
fn token_list_exact_mutation_snapshot_scratch_output_overlap_is_admitted_before_commit(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var p=document.getElementById('target');p.className='A\\ud800z';");
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("target")).unwrap();let list=script_class_list::handle(node_handle(node));let before=doc.dom_attr(node,"class").unwrap().try_clone().unwrap();let retained=page.state.retained;let limit=page.state.retained_limit;let commits=page.state.dom_commits;
    let metadata=2*size_of::<crate::dom_string::DomStringRef<'_>>();let scratch=16;let output=26;
    for headroom in [metadata-1,metadata+scratch+output-1] {
        page.begin_job();page.state.retained_limit=retained+headroom;
        let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.class_list_call(list,"add",&[Value::String("tail".into())]).unwrap_err();assert!(error.contains("Token mutation")&&error.contains("allowance"),"{error}");
        assert_eq!(doc.dom_attr(node,"class"),Some(&before));assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);
    }
    page.begin_job();page.state.retained_limit=retained+metadata+scratch+output;
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.class_list_call(list,"add",&[Value::String("tail".into())]).unwrap();
    assert_eq!(doc.dom_attr(node,"class").unwrap().code_units().collect::<Vec<_>>(),"A".encode_utf16().chain([0xd800]).chain("z tail".encode_utf16()).collect::<Vec<_>>());assert_eq!(page.state.dom_commits,Some(commits.unwrap()+1));page.state.retained_limit=limit;
}
#[test]
fn token_list_exact_mutation_copy_work_and_final_string_refusal_keep_record_and_recover(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var p=document.getElementById('target');p.className='A\\ud800z';");
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("target")).unwrap();let list=script_class_list::handle(node_handle(node));let before=doc.dom_attr(node,"class").unwrap().try_clone().unwrap();let retained=page.state.retained;let commits=page.state.dom_commits;let string=page.state.limits.host.string_bytes;
    for mode in 0..3 {
        page.begin_job();if mode==0{page.state.copy_work=0;}else if mode==1{page.state.host_work=0;}else{page.state.limits.host.string_bytes=12;}
        let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.class_list_call(list,"add",&[Value::String("tail".into())]).unwrap_err();assert!(error.contains("work limit")||error.contains("string bound"),"{error}");
        assert_eq!(doc.dom_attr(node,"class"),Some(&before));assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);page.state.limits.host.string_bytes=string;
    }
    page.begin_job();run(&mut doc,&mut forms,&mut page,"p.classList.add('tail');if(p.className!=='A\\ud800z tail')throw 'fresh native mutation recovery';");
}
#[test]
fn token_list_exact_mutation_saved_child_origin_brand_and_other_token_policies_remain(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),list=p.classList,frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var add=frame.contentWindow.DOMTokenList.prototype.add,calls=0,probe={toString:function(){calls++;return 'B\udfffz';}};add.call(list,'A\ud800z');
var svg=document.createElementNS('http://www.w3.org/2000/svg','g');svg.setAttribute('class','before');var sandbox=document.getElementById('frame').sandbox;
"#);
    let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    for source in ["svg.classList.add('A\\ud800z')","sandbox.add('allow-scripts\\ud800')"] {page.begin_job();let error=page.runtime.execute(source,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("UTF-16"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);}
    page.set_location(&crate::navigation::address("https://foreign-mutation.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();let error=page.runtime.execute("add.call(list,probe)",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("origin")||error.contains("Security"),"{error}");assert_eq!(page.state.dom_commits,commits);
    page.set_location(&crate::navigation::address("https://token-native.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();run(&mut doc,&mut forms,&mut page,"if(calls!==0||!list.contains('A\\ud800z')||sandbox.supports('allow-scripts'))throw 'authority before conversion';");
}
