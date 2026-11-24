//! Public inert ID producers use the shared exact selector backend.
use super::*;
fn fixture()->(Document,Forms,Page){let mut doc=crate::html::parse("<!doctype html><html><body><p id=target></p></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn css_id_exact_public_inert_queries_skip_raw_alias_and_keep_literal_identity(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var d=document.implementation.createHTMLDocument('owner'),raw=d.createElement('p'),literal=d.createElement('p');d.body.appendChild(raw);d.body.appendChild(literal);raw.setAttribute('id','A\ud800z');literal.setAttribute('id','A�z');
if(raw.matches('#A�z')||!raw.matches('p:not(#A�z)')||!literal.matches('#A�z'))throw 'exact ID identity';
if(d.querySelector('#A�z')!==literal||d.body.querySelector('#A�z')!==literal||d.querySelectorAll('#A�z').length!==1)throw 'native order skips alias';
if(d.getElementById('A\ud800z')!==raw||d.getElementById('A�z')!==literal)throw 'lookup agreement';
var a=raw.getAttributeNode('id');a.value='plain';if(!raw.matches('#plain')||d.querySelector('#plain')!==raw)throw 'Attr updated identity';
raw.setAttributeNS(null,'id','A\udfffz');if(raw.matches('#A�z')||a.value!=='A\udfffz')throw 'namespace exact identity';
raw.setAttribute('id','A😺');if(!raw.matches('#A😺')||d.querySelector('#A😺')!==raw)throw 'pair identity';
"#);}
#[test]
fn css_id_exact_public_inert_query_refusal_is_read_only_and_fresh_job_recovers(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var d=document.implementation.createHTMLDocument('owner'),p=d.createElement('p');d.body.appendChild(p);p.setAttribute('id','A\\ud800z');");
    let nodes=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;page.begin_job();page.state.host_work=0;
    let error=page.runtime.execute("p.matches('#A�z')",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("work")||error.contains("allowance"),"{error}");assert_eq!(format!("{:?}",doc.nodes),nodes);assert_eq!(page.state.dom_commits,commits);
    page.begin_job();run(&mut doc,&mut forms,&mut page,"if(p.getAttribute('id')!=='A\\ud800z'||p.matches('#A�z'))throw 'fresh exact record';p.setAttribute('id','A�z');if(!p.matches('#A�z'))throw 'fresh literal recovery';");
}
