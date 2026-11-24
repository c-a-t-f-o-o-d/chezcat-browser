//! Exact scalar named-property lookup over genuine main-document form records.
use super::*;
fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse("<!doctype html><html><body><form id=f><input id='A�z'><input id=literal name='A�z'></form><form id=raw-form name='B�z'></form><form id=literal-form name=other></form></body></html>");doc.scripting_enabled=true;
    let first=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("A�z")).unwrap();record(&mut doc,first,"id",&[65,0xd800,122]);
    let raw=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("raw-form")).unwrap();let literal=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("literal-form")).unwrap();record(&mut doc,raw,"name",&[66,0xd800,122]);record(&mut doc,literal,"name",&"B�z".encode_utf16().collect::<Vec<_>>());
    let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn record(doc:&mut Document,n:usize,name:&str,units:&[u16]){let NodeKind::Element{attrs,..}=&mut doc.nodes[n].kind else{unreachable!()};attrs.iter_mut().find(|a|a.name.html_is(name)).unwrap().value=DomString::from_units(units,4096).unwrap();}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn form_named_exact_native_id_alias_does_not_create_duplicate_or_mask_literal_field(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var f=document.getElementById('f'),literal=document.getElementById('literal'),calls=0,key={toString:function(){calls++;return 'A�z';}};
if(f[key]!==literal||calls!==1||!('A�z' in f))throw 'exact literal field';literal.setAttribute('name','changed');if(f['A�z']!==undefined||'A�z' in f)throw 'raw alias leaked';if(f.changed!==literal)throw 'live scalar name';
"#);}
#[test]
fn form_named_exact_raw_alias_does_not_override_author_prototype_and_real_duplicates_refuse(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var f=document.getElementById('f'),literal=document.getElementById('literal'),n=0,P=HTMLFormElement.prototype;literal.setAttribute('name','changed');Object.defineProperty(P,'A�z',{get:function(){n++;return this;},configurable:true});
if(f['A�z']!==f||n!==1||!('A�z' in f))throw 'ordinary prototype lookup';delete P['A�z'];if(f['A�z']!==undefined||'A�z' in f)throw 'deleted prototype lookup';
"#);
    let error=page.runtime.execute("var first=document.getElementsByTagName('input')[0];first.setAttribute('id','same');literal.setAttribute('name','same');f.same;",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();
    assert!(error.contains("RadioNodeList"),"{error}");
}
#[test]
fn form_named_exact_document_names_skip_raw_alias_and_keep_live_duplicate_refusal(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var literal=document.getElementById('literal-form');if(document['B�z']!==literal||!('B�z' in document))throw 'exact document form name';
"#);
    let raw=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("raw-form")).unwrap();
    record(&mut doc,raw,"name",&"B�z".encode_utf16().collect::<Vec<_>>());
    let error=page.runtime.execute("document['B�z'];",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();
    assert!(error.contains("live HTMLCollection"),"{error}");
}
#[test]
fn form_named_exact_native_work_refusal_and_borrowed_lookup_recover_without_mutation(){
    let(mut doc,mut forms,mut page)=fixture();let form=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("f")).unwrap();let literal=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("literal")).unwrap();let nodes=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;let retained=page.state.retained;let limit=page.state.retained_limit;
    {let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};bridge.state.rebuild(bridge.doc).unwrap();bridge.state.host_work=0;assert!(bridge.named_form_property(form,"A�z").unwrap_err().contains("work"));bridge.state.host_work=bridge.state.limits.host_work;bridge.state.copy_work=0;bridge.state.retained_limit=retained;assert!(matches!(bridge.named_form_property(form,"A�z").unwrap(),Value::Host(n) if n==node_handle(literal)));assert_eq!(bridge.state.retained,retained);}
    assert_eq!(format!("{:?}",doc.nodes),nodes);assert_eq!(page.state.dom_commits,commits);page.state.retained_limit=limit;page.begin_job();run(&mut doc,&mut forms,&mut page,"if(document.getElementById('f')['A�z']!==document.getElementById('literal')||document['B�z']!==document.getElementById('literal-form'))throw 'fresh exact recovery';");
}
