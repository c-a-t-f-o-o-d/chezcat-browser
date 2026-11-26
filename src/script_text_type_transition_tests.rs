//! Native same-value-mode type transitions use the shared attribute transaction.
use super::*;
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse("<body><form id=form><input id=x name=q type=text value=Seed pattern='[A-Z]+'><input id=neighbor value=neighbor></form></body>");d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}
fn node(d:&Document,id:&str)->usize{(0..d.nodes.len()).find(|&n|d.attr(n,"id")==Some(id)).unwrap()}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
#[test]
fn native_text_type_transition_value_dirty_user_selection_and_default_identity(){
 let(mut d,mut f,mut p)=fixture();let x=node(&d,"x");run(&mut d,&mut f,&mut p,"var x=document.getElementById('x'),v=x.validity;x.value='AB';");
 {let c=f.controls.get_mut(&x).unwrap();c.value.cursor=1;c.value.anchor=0;c.value.offset=1;c.last_value_change_user=true;}
 for(kind,canonical,masked)in[("password","password",true),("SEARCH","search",false),("tel","tel",false),("not-a-state","text",false),("PaSsWoRd","password",true),("text","text",false)]{
  run(&mut d,&mut f,&mut p,&format!("x.type='{kind}';if(x.type!=='{canonical}'||x.value!=='AB'||x.defaultValue!=='Seed'||x.validity!==v||v.patternMismatch)throw 'same-value type transition';"));
  let c=&f.controls[&x];assert_eq!((c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.last_value_change_user),("AB".into(),1,0,1,true,true));assert_eq!(c.password,masked);assert_eq!(c.kind,Kind::Text);
 }
 run(&mut d,&mut f,&mut p,"x.type='password';x.defaultValue='Default';if(x.value!=='AB')throw 'dirty value retained';document.getElementById('form').reset();if(x.value!=='Default'||x.type!=='password')throw 'reset after transition';");assert!(!f.controls[&x].dirty_value&&f.controls[&x].password);
}
#[test]
fn native_text_type_transition_all_attribute_producers_and_observation(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var x=document.getElementById('x'),a=x.getAttributeNode('type'),o=new MutationObserver(function(){});o.observe(x,{attributes:true,attributeOldValue:true,attributeFilter:['type']});
x.setAttribute('type','password');if(x.type!=='password'||a.value!=='password'||x.getAttributeNode('type')!==a)throw 'attribute type';var r=o.takeRecords();if(r.length!==1||r[0].oldValue!=='text')throw 'observation';
x.setAttributeNS(null,'type','search');if(x.type!=='search'||a.value!=='search')throw 'NS type';
a.value='password';if(x.type!=='password')throw 'Attr value';a.nodeValue='tel';if(x.type!=='tel')throw 'Attr nodeValue';a.textContent='password';if(x.type!=='password')throw 'Attr textContent';
var replacement=document.createAttribute('type');replacement.value='search';x.setAttributeNodeNS(replacement);if(x.type!=='search'||a.ownerElement!==null||replacement.ownerElement!==x)throw 'replacement';
x.removeAttributeNode(replacement);if(x.type!=='text'||replacement.ownerElement!==null)throw 'remove Attr';x.type='password';x.removeAttribute('type');if(x.type!=='text'||x.getAttribute('type')!==null)throw 'remove type';
"#);assert!(!f.controls[&node(&d,"x")].password);
}
#[test]
fn native_text_type_transition_refusal_is_atomic_and_fresh_job_recovers(){
 for mode in 0..4{
  let(mut d,mut f,mut p)=fixture();let x=node(&d,"x");run(&mut d,&mut f,&mut p,"var x=document.getElementById('x'),a=x.getAttributeNode('type');");let before=(format!("{:?}",d.nodes),p.state.retained,p.state.dom_commits,f.controls[&x].value.text());let limit=p.state.retained_limit;p.begin_job();match mode{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=p.state.retained,_=>p.state.execution_deadline=Some(Instant::now())};
  let result=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.public_attribute(x,crate::html::AttributeName::unqualified("type".into()),Some("password".into()));assert!(result.is_err(),"mode={mode}");assert_eq!((format!("{:?}",d.nodes),p.state.retained,p.state.dom_commits,f.controls[&x].value.text()),before);assert!(!f.controls[&x].password);
  p.state.retained_limit=limit;run(&mut d,&mut f,&mut p,"x.type='password';if(x.type!=='password'||x.value!=='Seed'||a.value!=='password')throw 'fresh type transition';");assert!(f.controls[&x].password);
 }
}
#[test]
fn native_text_type_transition_other_value_modes_remain_refused_without_change(){
 for kind in ["hidden","checkbox","radio","file","email","url","number","submit","button","reset","image","date","color"]{
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var x=document.getElementById('x');");let before=(format!("{:?}",d.nodes),p.state.retained,p.state.dom_commits);p.begin_job();assert!(p.runtime.execute(&format!("x.type='{kind}';"),&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).is_err());assert_eq!((format!("{:?}",d.nodes),p.state.retained,p.state.dom_commits),before);run(&mut d,&mut f,&mut p,"if(x.type!=='text'||x.value!=='Seed')throw 'unchanged refused mode';x.type='password';if(x.type!=='password')throw 'fresh admitted transition';");
 }
}
