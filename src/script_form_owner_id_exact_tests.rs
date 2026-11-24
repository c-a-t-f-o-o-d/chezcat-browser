//! Exact ID guards preserve existing explicit-form ownership boundaries.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut doc=crate::html::parse("<!doctype html><html><body><p id='A�z'></p><form id='A�z'></form><input id=field form='A�z' name=q value=ready><div id=mount></div></body></html>");doc.scripting_enabled=true;let n=(0..doc.nodes.len()).find(|&n|tag(&doc,n)=="p").unwrap();let NodeKind::Element{attrs,..}=&mut doc.nodes[n].kind else{unreachable!()};attrs.iter_mut().find(|a|a.name.html_is("id")).unwrap().value=DomString::from_units(&[65,0xd800,122],4096).unwrap();
 let mut forms=Forms::new(&doc);let mut page=Page::new();let location=crate::navigation::address("https://form-owner-native.test/source",&crate::navigation::Location::home()).unwrap();page.set_location(&location);page.set_resource_base(&location);assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn form_owner_id_exact_raw_alias_can_change_without_changing_an_explicit_owner(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"var p=document.getElementsByTagName('p')[0],f=document.getElementsByTagName('form')[0],field=document.getElementById('field');p.id='unrelated';if(p.id!=='unrelated'||field.form!==f)throw 'exact unrelated id';"#);
}
#[test]
fn form_owner_id_exact_projection_equal_identity_change_refuses_atomically(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var p=document.getElementsByTagName('p')[0],a=p.getAttributeNode('id'),observer=new MutationObserver(function(){});observer.observe(p,{attributes:true,attributeOldValue:true});");
 let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;let retained=page.state.retained;
 let error=page.runtime.execute("p.id='A�z';",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("explicit form association"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);assert_eq!(page.state.retained,retained);
 page.begin_job();run(&mut doc,&mut forms,&mut page,r#"if(p.id!=='A\ud800z'||a.value!==p.id||observer.takeRecords().length!==0)throw 'atomic id guard';p.id='fresh';if(p.id!=='fresh')throw 'fresh recovery';"#);
}
#[test]
fn form_owner_id_exact_raw_alias_tree_move_preserves_literal_owner(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"var p=document.getElementsByTagName('p')[0],mount=document.getElementById('mount'),f=document.getElementsByTagName('form')[0],field=document.getElementById('field');mount.appendChild(p);if(p.parentNode!==mount||field.form!==f)throw 'exact move';"#);
}

#[test]
fn form_owner_id_exact_native_guard_borrows_keys_and_work_refusal_recovers_without_mutation(){
 let(doc,forms,mut page)=fixture();let p=(0..doc.nodes.len()).find(|&n|tag(&doc,n)=="p").unwrap();let f=(0..doc.nodes.len()).find(|&n|tag(&doc,n)=="form").unwrap();let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;let retained=page.state.retained;
 page.state.host_work=0;assert!(form_id_guards::used(&doc,&forms,&mut page.state,[doc.dom_attr(p,"id"),None]).unwrap_err().contains("work"));page.begin_job();page.state.copy_work=0;
 assert!(!form_id_guards::used(&doc,&forms,&mut page.state,[doc.dom_attr(p,"id"),None]).unwrap());assert!(form_id_guards::used(&doc,&forms,&mut page.state,[doc.dom_attr(f,"id"),None]).unwrap());assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);assert_eq!(page.state.retained,retained);
}
