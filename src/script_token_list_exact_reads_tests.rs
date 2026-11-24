//! Original token-list exact read boundaries over native attribute records.
use super::*;
fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse("<!doctype html><html><body><p id=target class='A�z'></p><iframe id=frame sandbox='allow-scripts'></iframe></body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
fn raw_record(doc:&mut Document,page:&mut Page)->usize{
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("target")).unwrap();
    let NodeKind::Element{attrs,..}=&mut doc.nodes[node].kind else{unreachable!()};
    let units:Vec<_>=" one A".encode_utf16().chain([0xd800]).chain("z one \u{a0}\u{b} ".encode_utf16()).collect();
    attrs.iter_mut().find(|a|a.name.html_is("class")).unwrap().value=DomString::from_units(&units,256).unwrap();
    page.state.rebuild(doc).unwrap();node
}

#[test]
fn token_list_exact_reads_membership_conversion_and_supported_tokens() {
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var list=document.getElementById('target').classList,calls=0,probe={toString:function(){calls++;return 'A\ud800z';}};
if(list.contains(probe)||calls!==1||!list.contains('A\ufffdz')||list.contains('\udfff'))throw 'exact membership conversion';
var sandbox=document.getElementById('frame').sandbox;if(sandbox.supports('\ud800')||sandbox.supports('allow-scripts'))throw 'supported token does not grant authority';
try{list.supports('\ud800');throw 'missing unsupported set';}catch(e){if(!(e instanceof TypeError))throw e;}
var marker={};try{list.contains({toString:function(){throw marker;}});throw 'missing abrupt';}catch(e){if(e!==marker)throw e;}
if(!list.contains('A\ufffdz'))throw 'fresh read recovery';
"#);
}

#[test]
fn token_list_exact_reads_native_raw_record_value_indices_keys_clone_and_live_changes() {
    let(mut doc,mut forms,mut page)=fixture();raw_record(&mut doc,&mut page);
    // Public raw class mutation remains gated by separate CSS consumers.
    // This source-bound native record exercises the existing shared storage.
    run(&mut doc,&mut forms,&mut page,r#"
var target=document.getElementById('target'),list=target.classList,raw=' one A\ud800z one \u00a0\u000b ';
if(list.value!==raw||String(list)!==raw||list.length!==3||list[0]!=='one'||list[1]!=='A\ud800z'||list.item(2)!=='\u00a0\u000b'||list[3]!==undefined||list.item(3)!==null)throw 'exact native token snapshot';
if(!list.contains('A\ud800z')||list.contains('A\ufffdz')||Object.keys(list).join(',')!=='0,1,2')throw 'indices and raw equality';
var copy=target.cloneNode(false);if(copy.classList[1]!=='A\ud800z'||copy.classList.value!==raw)throw 'clone exact reads';
target.setAttribute('class','changed changed Other');if(list.length!==2||list[0]!=='changed'||list.value!=='changed changed Other'||copy.classList[1]!=='A\ud800z')throw 'live reads and independent clone';
"#);
}

#[test]
fn token_list_exact_reads_do_not_scalarize_native_raw_records_during_unsupported_mutation() {
    let(mut doc,mut forms,mut page)=fixture();let node=raw_record(&mut doc,&mut page);let before=doc.dom_attr(node,"class").unwrap().try_clone().unwrap();
    let retained=page.state.retained;let commits=page.state.dom_commits;
    let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.class_list_call(script_class_list::handle(node_handle(node)),"add",&[Value::String("other".into())]).unwrap_err();
    assert!(error.contains("raw")||error.contains("UTF-16"),"{error}");
    assert_eq!(doc.dom_attr(node,"class").unwrap(),&before);assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);
}

#[test]
fn token_list_exact_reads_snapshot_and_selected_raw_output_share_headroom() {
    let(mut doc,mut forms,mut page)=fixture();let node=raw_record(&mut doc,&mut page);
    let retained=page.state.retained;let limit=page.state.retained_limit;let commits=page.state.dom_commits;
    let snapshot=4*size_of::<crate::dom_string::DomStringRef<'_>>();
    let output=DomString::from_units(&[65,0xd800,122],64).unwrap().capacity();
    for (headroom,needle) in [(snapshot-1,"snapshot"),(snapshot+output-1,"output")]{
        page.begin_job();page.state.retained_limit=retained+headroom;
        let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.read_class_tokens(node,"class",token_list_reads::Read::Index(1,false)).unwrap_err();
        assert!(error.contains(needle)&&error.contains("allowance"),"{error}");assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);
    }
    page.begin_job();page.state.retained_limit=retained+snapshot+output;
    let value=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.read_class_tokens(node,"class",token_list_reads::Read::Index(1,false)).unwrap();
    assert!(matches!(value,Value::StringUnits(units) if units==[65,0xd800,122]));assert_eq!(page.state.retained,retained);
    page.begin_job();page.state.retained_limit=retained;
    let value=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.read_class_tokens(node,"class",token_list_reads::Read::Contains(crate::dom_string::DomStringRef::Raw(&[65,0xd800,122]))).unwrap();
    assert!(matches!(value,Value::Bool(true)));assert_eq!(page.state.retained,retained);
    page.state.retained_limit=limit;page.begin_job();
    assert!(matches!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.class_list_get(script_class_list::handle(node_handle(node)),"length").unwrap(),Value::Number(3.0)));
}

#[test]
fn token_list_exact_reads_work_keys_and_empty_snapshot_refusal_recover() {
    let(mut doc,mut forms,mut page)=fixture();let node=raw_record(&mut doc,&mut page);let retained=page.state.retained;let limit=page.state.retained_limit;
    page.begin_job();page.state.copy_work=0;
    let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.read_class_tokens(node,"class",token_list_reads::Read::Length).unwrap_err();
    assert!(error.contains("string work limit"),"{error}");assert_eq!(page.state.retained,retained);
    page.begin_job();page.state.host_work=0;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.read_class_tokens(node,"class",token_list_reads::Read::Length).is_err());assert_eq!(page.state.retained,retained);
    page.begin_job();let snapshot=4*size_of::<crate::dom_string::DomStringRef<'_>>();
    page.state.retained_limit=retained+snapshot;
    let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.class_list_keys(script_class_list::handle(node_handle(node))).unwrap_err();
    assert!(error.contains("keys retained allowance"),"{error}");assert_eq!(page.state.retained,retained);
    page.state.retained_limit=limit;page.begin_job();
    assert_eq!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.class_list_keys(script_class_list::handle(node_handle(node))).unwrap(),["0","1","2"]);
    page.state.retained_limit=retained;page.begin_job();
    assert!(matches!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.read_class_tokens(node,"missing",token_list_reads::Read::Length).unwrap(),Value::Number(0.0)));
    assert_eq!(page.state.retained,retained);page.state.retained_limit=limit;
}
