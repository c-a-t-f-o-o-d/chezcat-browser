//! Original fresh text-family admission, clone/import and atomicity witnesses.
use super::*;
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse("<body><form id=form method=post action='https://text-family.test/submit'><div id=host></div></form><input id=neighbor value=neighbor></body>");d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();let l=crate::navigation::address("https://text-family.test/",&crate::navigation::Location::home()).unwrap();p.set_location(&l);p.set_resource_base(&l);assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}
fn node(d:&Document,id:&str)->usize{(0..d.nodes.len()).find(|&n|d.attr(n,"id")==Some(id)).unwrap()}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
#[test]
fn native_text_family_admission_fresh_type_first_writers_defaults_and_post(){
 for kind in ["password","search","tel","unknown"]{for writer in ["x.type=kind;","x.setAttribute('type',kind);","x.setAttributeNS(null,'type',kind);","var a=document.createAttribute('type');a.value=kind;x.setAttributeNodeNS(a);"]{
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,&format!("var kind='{kind}',x=document.createElement('input');{writer}x.id='fresh';x.name='entry';x.defaultValue='A\\r\\nB';x.value='Live😀';document.getElementById('host').appendChild(x);if(x.type!==(kind==='unknown'?'text':kind)||x.value!=='Live😀'||x.defaultValue!=='A\\r\\nB'||x.form!==document.getElementById('form'))throw 'fresh family state';"));
  let fresh=node(&d,"fresh");assert_eq!(f.controls[&fresh].kind,Kind::Text);assert_eq!(f.controls[&fresh].password,kind=="password");assert!(f.controls[&fresh].dirty_value);
  let owner=node(&d,"form");let l=crate::navigation::address("https://text-family.test/",&crate::navigation::Location::home()).unwrap();let sent=f.submit_request(&d,owner,None,&l,&l).unwrap();let crate::http::Request::FormPost{body,..}=sent.request else{panic!("POST")};assert_eq!(String::from_utf8(body).unwrap(),"entry=Live%F0%9F%98%80");
  run(&mut d,&mut f,&mut p,"document.getElementById('form').reset();if(x.value!=='AB'||x.defaultValue!=='A\\r\\nB')throw 'reset family default';");assert!(!f.controls[&fresh].dirty_value);assert_eq!(f.controls[&fresh].password,kind=="password");
 }}
}
#[test]
fn native_text_family_admission_fragment_clone_move_and_reset(){
 for kind in ["password","search","tel","unknown"]{
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,&format!(r#"
var host=document.getElementById('host'),form=document.getElementById('form');host.innerHTML='<input id=fresh name=q type={kind} value=Seed>';
var x=host.firstChild;x.value='Live😀';var copy=x.cloneNode(true);copy.id='copy';copy.name='copy';host.appendChild(copy);
if(copy.value!=='Live😀'||copy.defaultValue!=='Seed'||copy.type!==x.type||copy.form!==form)throw 'family clone';
x.defaultValue='First';copy.defaultValue='Second';if(x.value!=='Live😀'||copy.value!=='Live😀')throw 'independent dirty clones';
host.removeChild(x);x.value='Detached';host.appendChild(x);if(x.value!=='Detached'||x.form!==form)throw 'family move';
form.reset();if(x.value!=='First'||copy.value!=='Second'||x.type!==copy.type)throw 'family reset';
"#));for id in ["fresh","copy"]{assert_eq!(f.controls[&node(&d,id)].password,kind=="password");}
 }
}
#[test]
fn native_text_family_admission_inert_import_owns_native_value_and_mask(){
 for kind in ["password","search","tel","unknown"]{
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,&format!(r#"
var t=document.createElement('template');t.innerHTML='<input id=fresh name=q type={kind} value=Seed>';
var source=t.content,imported=document.importNode(source,true),x=imported.firstChild;
if(x.ownerDocument!==document||x.parentNode!==imported||x.isConnected||x.value!=='Seed'||x.defaultValue!=='Seed'||x.form!==null)throw 'detached native import';
x.value='Imported😀';document.getElementById('host').appendChild(imported);if(x.form!==document.getElementById('form')||x.value!=='Imported😀')throw 'connected native import';
document.getElementById('form').reset();if(x.value!=='Seed'||x.defaultValue!=='Seed')throw 'import reset';
"#));let c=&f.controls[&node(&d,"fresh")];assert_eq!(c.password,kind=="password");assert!(!c.dirty_value);
 }
}
#[test]
fn native_text_family_admission_refusal_publishes_no_control_and_recovers(){
 for mode in 0..4{
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var x=document.createElement('input');x.id='fresh';var initialType=x.type;");let x=node(&d,"fresh");assert!(!f.controls.contains_key(&x));let before=(format!("{:?}",d.nodes),p.state.retained,p.state.dom_commits,f.controls.len());let limit=p.state.retained_limit;p.begin_job();match mode{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=p.state.retained,_=>p.state.execution_deadline=Some(Instant::now())};
  let result=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.public_attribute(x,crate::html::AttributeName::unqualified("type".into()),Some("password".into()));assert!(result.is_err(),"mode={mode}");assert_eq!((format!("{:?}",d.nodes),p.state.retained,p.state.dom_commits,f.controls.len()),before);
  p.state.retained_limit=limit;run(&mut d,&mut f,&mut p,"x.type='password';x.value='Recovered';if(x.type!=='password'||x.value!=='Recovered')throw 'fresh admission recovery';");assert!(f.controls[&x].password);
 }
}
#[test]
fn native_text_family_admission_other_input_states_remain_refused(){
 for kind in ["file","email","url","number","range","color","date","submit","button"]{
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var x=document.createElement('input');x.id='fresh';var initialType=x.type;");let x=node(&d,"fresh");let before=(format!("{:?}",d.nodes),p.state.retained,p.state.dom_commits,f.controls.len());p.begin_job();assert!(p.runtime.execute(&format!("x.type='{kind}';"),&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).is_err());assert_eq!((format!("{:?}",d.nodes),p.state.retained,p.state.dom_commits,f.controls.len()),before);assert!(!f.controls.contains_key(&x));
  run(&mut d,&mut f,&mut p,"x.type='password';if(x.type!=='password')throw 'fresh valid admission';");assert!(f.controls[&x].password);
 }
}
