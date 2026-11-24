//! Public HTML ID producers retain exact UTF-16 through native consumers.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut doc=crate::html::parse("<!doctype html><html><body><p id=seed></p><p id='A�z'></p><div id=mount></div><form id=owner><input id=field name=q form=owner value=ready></form><svg id=shape></svg></body></html>");doc.scripting_enabled=true;
 let mut forms=Forms::new(&doc);let mut page=Page::new();let location=crate::navigation::address("https://html-id.test/source",&crate::navigation::Location::home()).unwrap();page.set_location(&location);page.set_resource_base(&location);assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn html_id_domstring_public_setters_attr_aliases_and_exact_lookup(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('seed'),literal=document.getElementById('A�z'),a=p.getAttributeNode('id'),o=new MutationObserver(function(){});o.observe(p,{attributes:true,attributeOldValue:true});
p.setAttribute('id','A\ud800z');if(p.id!=='A\ud800z'||a.value!==p.id||document.getElementById(p.id)!==p||document.getElementById('A�z')!==literal)throw 'setAttribute exact';
p.setAttributeNS(null,'id','B\udfffz');if(p.id!=='B\udfffz'||document.getElementById('A\ud800z')!==null||a.nodeValue!==p.id)throw 'namespace exact';
p.id='C\ud800z';a.value='D\udfffz';a.nodeValue='E\ud800z';a.textContent='F\udfffz';if(document.getElementById(p.id)!==p||p.id!=='F\udfffz')throw 'aliases exact';
var records=o.takeRecords();if(records.length!==6||records[0].oldValue!=='seed'||records[1].oldValue!=='A\ud800z'||records[5].oldValue!=='E\ud800z')throw 'exact observer';
var detached=document.createAttribute('id');detached.value='G\ud800z';var old=p.setAttributeNode(detached);if(old!==a||old.ownerElement!==null||old.value!=='F\udfffz'||detached.ownerElement!==p||p.id!==detached.value||document.getElementById(p.id)!==p)throw 'attachment exact';
p.removeAttribute('id');if(detached.ownerElement!==null||detached.value!=='G\ud800z'||p.id!==''||document.getElementById('G\ud800z')!==null)throw 'detachment exact';p.id='fresh';if(document.getElementById('fresh')!==p)throw 'recovery';
var next=document.createAttributeNS(null,'id');next.value='H\udfffz';p.setAttributeNodeNS(next);if(next.ownerElement!==p||p.id!==next.value||document.getElementById(p.id)!==p)throw 'namespace attachment';
"#);}
#[test]
fn html_id_domstring_clone_move_collection_selector_and_form_consumers(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('seed'),literal=document.getElementById('A�z'),mount=document.getElementById('mount'),field=document.getElementById('field'),f=document.getElementById('owner');p.id='A\ud800z';
if(document.querySelector('#A�z')!==literal||p.matches('#A�z')||document.getElementsByTagName('p').namedItem(p.id)!==p||document.getElementsByTagName('p').namedItem('A�z')!==literal)throw 'exact consumers';
var c=p.cloneNode(true);if(c.id!==p.id||c.getAttributeNode('id').value!==p.id||c.parentNode!==null)throw 'clone';mount.appendChild(c);if(document.getElementById(p.id)!==p)throw 'first tree order';mount.appendChild(p);if(document.getElementById(p.id)!==c)throw 'moved tree order';c.remove();if(document.getElementById(p.id)!==p||field.form!==f||f.field!==field)throw 'remove and form';
"#);}
#[test]
fn html_id_domstring_raw_write_resource_refusal_is_atomic_and_recovers(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var p=document.getElementById('seed'),a=p.getAttributeNode('id'),o=new MutationObserver(function(){});o.observe(p,{attributes:true,attributeOldValue:true});");
 let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;let retained=page.state.retained;let limit=page.state.retained_limit;let strings=page.state.limits.host.string_bytes;
 for mode in 0..4{page.begin_job();match mode{0=>page.state.host_work=0,1=>page.state.copy_work=0,2=>page.state.retained_limit=retained,_=>page.state.limits.host.string_bytes=4};let error=page.runtime.execute(r#"p.id='long A\ud800z';"#,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("work")||error.contains("allowance")||error.contains("bound"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);assert_eq!(page.state.retained,retained);page.state.retained_limit=limit;page.state.limits.host.string_bytes=strings;}
 page.begin_job();run(&mut doc,&mut forms,&mut page,r#"if(a.value!=='seed'||o.takeRecords().length!==0)throw 'refusal publication';p.id='fresh A\ud800z';var r=o.takeRecords();if(a.value!==p.id||r.length!==1||r[0].oldValue!=='seed'||document.getElementById(p.id)!==p)throw 'fresh raw recovery';"#);
}
#[test]
fn html_id_domstring_explicit_form_owner_guard_retains_conversion_effects(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var f=document.getElementById('owner'),field=document.getElementById('field'),calls=0;");let error=page.runtime.execute(r#"f.id={toString:function(){calls++;f.setAttribute('data-effect','yes');return 'A\ud800z';}};"#,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("explicit form association"),"{error}");page.begin_job();run(&mut doc,&mut forms,&mut page,"if(calls!==1||f.id!=='owner'||field.form!==f||f.getAttribute('data-effect')!=='yes')throw 'guard effects';");
}
#[test]
fn html_id_domstring_retained_storage_tracks_raw_backing_and_scalar_mirror(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var p=document.getElementById('seed');");let n=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("seed")).unwrap();
 fn payload(doc:&Document,n:usize)->usize{let NodeKind::Element{attrs,id,..}=&doc.nodes[n].kind else{unreachable!()};attrs.iter().find(|a|a.name.html_is("id")).unwrap().value.capacity()+id.as_ref().map_or(0,String::capacity)}
 for source in [r#"p.id='long A\ud800z';"#,"p.id='short';",r#"p.setAttribute('id','B\udfffz');"#,"p.id='';"] {let before=page.state.retained;let old=payload(&doc,n);run(&mut doc,&mut forms,&mut page,source);assert_eq!(page.state.retained,before-old+payload(&doc,n));}
}
