//! Exact raw HTML IDs retain native records across owner transfer.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut doc=crate::html::parse("<!doctype html><html><body><div id=mount></div><p id='A�z'></p><form id='A�z'><input id=field form='A�z' name=q value=ready></form></body></html>");doc.scripting_enabled=true;
 let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn html_id_materialization_import_preserves_exact_copies_and_source_owner(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var owner=document.implementation.createHTMLDocument('source'),p=owner.createElement('p');owner.body.appendChild(p);p.id='A\ud800z';p.textContent='copied';var a=p.getAttributeNode('id'),copy=document.importNode(p,true),b=copy.getAttributeNode('id');
if(copy===p||copy.ownerDocument!==document||copy.id!==p.id||copy.textContent!=='copied'||a===b||a.ownerElement!==p||a.ownerDocument!==owner||b.ownerElement!==copy||b.ownerDocument!==document||p.parentNode!==owner.body)throw 'exact independent import';
document.getElementById('mount').appendChild(copy);if(document.getElementById(p.id)!==copy||document.getElementById('A�z')===copy||owner.getElementById(p.id)!==p)throw 'owner lookup';copy.id='B\udfffz';if(p.id!=='A\ud800z'||a.value!==p.id||b.value!==copy.id||document.getElementById(copy.id)!==copy)throw 'independent aliases';
"#);}
#[test]
fn html_id_materialization_parked_promotion_uses_exact_explicit_form_guard(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var t=document.createElement('template');t.innerHTML='<span id=seed>promoted</span>';var p=t.content.firstChild,owner=p.ownerDocument,a=p.getAttributeNode('id'),mount=document.getElementById('mount'),watch=new MutationObserver(function(){});p.id='A\ud800z';watch.observe(mount,{childList:true});
mount.appendChild(t.content);if(mount.firstChild!==p||p.ownerDocument!==document||p.getAttributeNode('id')!==a||a.ownerDocument!==document||a.ownerElement!==p||p.id!=='A\ud800z'||document.getElementById(p.id)!==p||document.getElementById('A�z')===p)throw 'exact promoted identity';var r=watch.takeRecords();if(r.length!==1||r[0].addedNodes[0]!==p||r[0].target!==mount||t.content.childNodes.length)throw 'actual promotion record';
var field=document.getElementById('field');if(field.form!==null)throw 'first nonform blocker preserved';
"#);}
#[test]
fn html_id_materialization_template_clone_keeps_raw_contents_inert_then_promotes(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var t=document.createElement('template');t.innerHTML='<span id=seed>copy</span>';var original=t.content.firstChild;original.id='B\udfffz';var c=document.importNode(t,true),p=c.content.firstChild,owner=p.ownerDocument;if(p===original||p.id!==original.id||p.ownerDocument===document||p.isConnected||document.getElementById(p.id)!==null)throw 'inert clone';
document.getElementById('mount').appendChild(c.content);if(p.ownerDocument!==document||document.getElementById(p.id)!==p||original.ownerDocument!==owner||t.content.firstChild!==original||original.id!=='B\udfffz')throw 'independent promoted clone';
"#);}
#[test]
fn html_id_materialization_import_resource_refusal_publishes_nothing_and_recovers(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"var owner=document.implementation.createHTMLDocument('source'),p=owner.createElement('p');p.id='long A\ud800z';p.textContent='held';owner.body.appendChild(p);var a=p.getAttributeNode('id'),watch=new MutationObserver(function(){});watch.observe(owner.body,{childList:true});"#);
 let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;let retained=page.state.retained;let limit=page.state.retained_limit;
 for mode in 0..3{page.begin_job();match mode{0=>page.state.host_work=0,1=>page.state.copy_work=0,_=>page.state.retained_limit=retained};let error=page.runtime.execute("document.importNode(p,true);",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("work")||error.contains("allowance")||error.contains("limit"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);assert_eq!(page.state.retained,retained);page.state.retained_limit=limit;}
 page.begin_job();run(&mut doc,&mut forms,&mut page,r#"if(p.parentNode!==owner.body||a.value!=='long A\ud800z'||watch.takeRecords().length)throw 'refused source';var c=document.importNode(p,true);document.getElementById('mount').appendChild(c);if(c.id!==p.id||document.getElementById(c.id)!==c)throw 'fresh import';"#);
}
#[test]
fn html_id_materialization_literal_explicit_association_still_refuses_promotion(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"var t=document.createElement('template');t.innerHTML='<span id=seed>held</span>';var p=t.content.firstChild,owner=p.ownerDocument,a=p.getAttributeNode('id'),mount=document.getElementById('mount'),watch=new MutationObserver(function(){});p.id='A�z';watch.observe(mount,{childList:true});"#);let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
 assert!(page.runtime.execute("mount.appendChild(t.content);",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).is_err());assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);page.begin_job();run(&mut doc,&mut forms,&mut page,r#"if(p.ownerDocument!==owner||p.parentNode!==t.content||p.getAttributeNode('id')!==a||watch.takeRecords().length||mount.childNodes.length)throw 'atomic association refusal';p.id='A\ud800z';mount.appendChild(t.content);if(document.getElementById(p.id)!==p||p.ownerDocument!==document)throw 'fresh exact promotion';"#);
}
#[test]
fn html_id_materialization_native_input_import_uses_actual_editor_and_exact_id(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"var owner=document.implementation.createHTMLDocument('source'),p=owner.createElement('input');p.id='I\ud800z';p.setAttribute('value','ready');var c=document.importNode(p,true);document.getElementById('mount').appendChild(c);if(c.id!==p.id||document.getElementById(c.id)!==c||c.value!=='ready'||c.form!==null)throw 'native imported input';c.value='changed';if(c.value!=='changed'||p.getAttribute('value')!=='ready')throw 'independent native editor';"#);
}
#[test]
fn html_id_materialization_raw_id_policy_keeps_svg_foreign_and_other_values_guarded(){
 let id=crate::html::Attribute{name:crate::html::AttributeName::unqualified("id".into()),value:DomString::from_units(&[65,0xd800,122],4096).unwrap()};
 assert!(script_attr::materializable_value(&id,crate::html::Namespace::Html));assert!(!script_attr::materializable_value(&id,crate::html::Namespace::Svg));assert!(!script_attr::materializable_value(&id,crate::html::Namespace::Other));
 let other=crate::html::Attribute{name:crate::html::AttributeName::unqualified("form".into()),value:id.value};assert!(!script_attr::materializable_value(&other,crate::html::Namespace::Html));
}
