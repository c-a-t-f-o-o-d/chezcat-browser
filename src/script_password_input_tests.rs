//! Original native HTML pattern witnesses, without a script RegExp substitute.
use super::*;
#[test]
fn native_password_input_user_length_required_and_refused_write_recovery(){
 let(mut d,mut f,mut p)=fixture();let n=node(&d,"input");
 run(&mut d,&mut f,&mut p,"var x=document.getElementById('input'),v=x.validity;x.removeAttribute('pattern');x.minLength=4;x.maxLength=5;x.required=true;if(!v.valueMissing||v.tooShort)throw 'required empty';");
 f.controls.get_mut(&n).unwrap().insert("😀");run(&mut d,&mut f,&mut p,"if(v.valueMissing||!v.tooShort||v.valid)throw 'user UTF16 length';");
 let before=(format!("{:?}",d.nodes),p.state.retained,p.state.dom_commits,f.controls[&n].value.text());p.begin_job();p.state.host_work=0;p.state.copy_work=0;
 assert!(p.runtime.execute("x.value='ready';",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).is_err());assert_eq!((format!("{:?}",d.nodes),p.state.retained,p.state.dom_commits,f.controls[&n].value.text()),before);
 run(&mut d,&mut f,&mut p,"if(!v.tooShort)throw 'refusal retains user flag';x.value='a';if(v.tooShort||v.valueMissing||!v.valid)throw 'script value excludes length';");
 {let c=f.controls.get_mut(&n).unwrap();c.value.select_all();c.insert("😀😀😀");assert_eq!(c.value.text(),"😀😀");}
 run(&mut d,&mut f,&mut p,"if(v.tooShort||v.tooLong||!v.valid||x.value!=='😀😀')throw 'native maximum UTF16';");
}
fn fixture()->(Document,Forms,Page){
 let mut d=crate::html::parse("<body><form id=form><input id=input type=password name=q pattern='[A-Z]+'><input id=other name=other pattern='[0-9]+'><button id=button>Submit</button></form><textarea id=area pattern='x'></textarea></body>");d.scripting_enabled=true;
 let mut f=Forms::new(&d);let mut p=Page::new();let l=crate::navigation::address("https://pattern-validation.test/",&crate::navigation::Location::home()).unwrap();p.set_location(&l);p.set_resource_base(&l);assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)
}
fn node(d:&Document,id:&str)->usize{(0..d.nodes.len()).find(|&n|d.attr(n,"id")==Some(id)).unwrap()}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,source:&str){p.begin_job();p.runtime.execute(source,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}

#[test]
fn native_password_input_live_flags_empty_invalid_and_barred_states(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var x=document.getElementById('input'),v=x.validity;
if(v.patternMismatch||!v.valid||!x.checkValidity()||x.validationMessage)throw 'empty optional';
x.value='abc';if(!v.patternMismatch||v.valid||x.checkValidity()||x.validationMessage!=='Match the required pattern: q.')throw 'whole value mismatch';
x.value='ABC';if(v.patternMismatch||!v.valid||!x.checkValidity())throw 'matching value';
x.pattern='';if(!v.patternMismatch||v.valid)throw 'empty pattern matches only empty';
for(var pattern of ['[','a)|(?:b','(?-:a)','[a&&b--c]']){x.pattern=pattern;x.value='z';if(v.patternMismatch||!v.valid||!x.checkValidity())throw 'invalid standalone ignored';}
x.pattern='[A-Z]+';x.value='a';x.readOnly=true;if(!v.patternMismatch||v.valid||!x.checkValidity()||x.validationMessage)throw 'readonly independent flag';
x.readOnly=false;x.disabled=true;if(!v.patternMismatch||v.valid||!x.checkValidity()||x.validationMessage)throw 'disabled independent flag';
x.disabled=false;x.setCustomValidity('Custom');if(!v.patternMismatch||!v.customError||v.valid||x.validationMessage!=='Custom')throw 'custom priority';x.setCustomValidity('');
x.removeAttribute('pattern');if(v.patternMismatch||!v.valid||!x.checkValidity())throw 'removed pattern';
var a=document.getElementById('area');a.value='y';if(a.validity.patternMismatch||!a.validity.valid)throw 'textarea inapplicable';
"#);
}

#[test]
fn native_password_input_unicode_v_sets_modifiers_and_raw_attribute_units(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var x=document.getElementById('input'),v=x.validity;
for(var pair of [['(?i:[a-z]+)','Kſ'],['[\\q{ab|cd}&&\\q{ab}]','ab'],['\\p{Script=Greek}+','Σς'],['(?i:𐐀)','𐐨'],['(?<猫>a)\\k<猫>','aa']]){x.pattern=pair[0];x.value=pair[1];if(v.patternMismatch||!v.valid)throw 'Unicode pattern';x.value='!'+pair[1];if(!v.patternMismatch||v.valid)throw 'anchored Unicode';}
x.pattern='(?i:a)b';x.value='AB';if(!v.patternMismatch)throw 'local i restoration';x.value='Ab';if(v.patternMismatch)throw 'local i matching';
x.pattern='\ud800';x.value='\ufffd';if(!v.patternMismatch)throw 'exact raw pattern units';
"#);
 for state in ["text","search","tel","password","unknown"] {
  let mut d=crate::html::parse(&format!("<input id=input type={state} pattern=a value=b>"));d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);
  run(&mut d,&mut f,&mut p,"var x=document.getElementById('input');if(!x.validity.patternMismatch||x.checkValidity())throw 'parsed admitted text state';");
 }
}

#[test]
fn native_password_input_fatal_work_skips_catch_finally_and_recovers(){
 for operation in ["x.checkValidity()","form.checkValidity()","form.requestSubmit(b)","x.validationMessage","x.validity.patternMismatch"] {
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r"var x=document.getElementById('input'),form=document.getElementById('form'),b=document.getElementById('button'),finished=false;x.pattern='(?i:[\\p{Lu}])';x.value='a';");
  // Establish the lazy native view/method/tree caches before measuring the
  // attempted validation's own publication and temporary-storage cleanup.
  run(&mut d,&mut f,&mut p,"var check=x.checkValidity,formCheck=form.checkValidity,request=form.requestSubmit,view=x.validity;if(view.patternMismatch)throw 'warm matching pattern';");
  // requestSubmit retains its admitted frame-vector backing for reuse. Warm
  // that backing with a canceled valid submission before the refusal baseline.
  run(&mut d,&mut f,&mut p,"form.addEventListener('submit',function(e){e.preventDefault();});form.requestSubmit(b);");
  assert!(p.state.form_requests.is_empty());
  assert_eq!(p.take_validation_report(),Some(None));
  let before=(format!("{:?}",d.nodes),p.state.retained,p.state.dom_commits);p.begin_job();p.state.host_work=3000;
  let error=p.runtime.execute(&format!("try{{{operation};}}catch(e){{finished=true;}}finally{{finished=true;}}"),&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();
  assert!(error.contains("Regular-expression work limit"),"{operation}: {error}");
  assert_eq!((format!("{:?}",d.nodes),p.state.retained,p.state.dom_commits),before,"{operation}");assert!(p.state.form_validations.is_empty()&&p.state.synthetic_dispatches.is_empty());assert!(p.take_validation_report().is_none());
  assert!(p.state.form_requests.is_empty());
  run(&mut d,&mut f,&mut p,"if(finished||x.validity.patternMismatch||!x.checkValidity())throw 'fresh fatal recovery';");
 }
}

#[test]
fn native_password_input_events_submission_snapshot_reset_and_recovery(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var x=document.getElementById('input'),other=document.getElementById('other'),form=document.getElementById('form'),b=document.getElementById('button'),invalid=0,submitted=0;
x.value='a';other.value='b';
x.addEventListener('invalid',function(e){if(!e.isTrusted||e.cancelable!==true||e.bubbles!==false)throw 'trusted invalid';invalid++;other.value='1';e.preventDefault();});
other.addEventListener('invalid',function(e){invalid++;e.preventDefault();});
form.addEventListener('submit',function(e){if(!e.isTrusted||e.submitter!==b)throw 'trusted submit';submitted++;e.preventDefault();});
if(form.checkValidity()!==false||invalid!==2)throw 'frozen snapshot';
form.requestSubmit(b);if(submitted!==0||invalid!==3)throw 'pattern blocks request';
form.noValidate=true;form.requestSubmit(b);if(submitted!==1||invalid!==3||form.reportValidity()!==false||invalid!==4)throw 'bypass versus explicit validation';
form.noValidate=false;x.value='A';form.requestSubmit(b);if(submitted!==2||invalid!==4)throw 'repaired request';
x.defaultValue='bad';form.reset();if(x.value!=='bad'||!x.validity.patternMismatch||x.validity.valid)throw 'reset applies pattern to all values';
x.pattern='(?i:bad)';if(!x.checkValidity()||x.validity.patternMismatch)throw 'live pattern recovery';
"#);
 let owner=node(&d,"form");let input=node(&d,"input");f.script_set_value(input,"!").unwrap();assert!(f.validate_submission(&d,owner,None).is_err());f.script_set_value(input,"BAD").unwrap();assert!(f.validate_submission(&d,owner,None).is_ok());
}

#[test]
fn native_password_input_refusal_does_not_dispatch_publish_or_mutate(){
 for mode in 0..4 {
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var x=document.getElementById('input'),events=0;x.value='a';x.addEventListener('invalid',function(){events++;});");
  let object=node_handle(node(&d,"input"));let before=(format!("{:?}",d.nodes),p.state.retained,p.state.dom_commits,f.controls[&node(&d,"input")].value.text());let limit=p.state.retained_limit;p.begin_job();
  match mode {0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=p.state.retained,_=>p.state.execution_deadline=Some(Instant::now())};
  assert!(Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.prepare_native_control_validation(object,Some(true)).is_err(),"mode {mode}");
  assert_eq!((format!("{:?}",d.nodes),p.state.retained,p.state.dom_commits,f.controls[&node(&d,"input")].value.text()),before);assert!(p.state.form_validations.is_empty());assert!(p.take_validation_report().is_none());
  p.state.retained_limit=limit;run(&mut d,&mut f,&mut p,"if(events!==0||!x.validity.patternMismatch||x.checkValidity()!==false||events!==1)throw 'fresh pattern recovery';");
 }
}

#[test]
fn native_password_input_defaults_dirty_newlines_and_native_submission(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var x=document.getElementById('input'),form=document.getElementById('form');
if(x.type!=='password'||x.value!==''||x.defaultValue!=='')throw 'parsed password';
x.defaultValue='A\r\nB';if(x.value!=='AB'||x.defaultValue!=='A\r\nB')throw 'pristine newline sanitization';
x.value='C\rD\n';x.defaultValue='EF';if(x.value!=='CD'||x.defaultValue!=='EF')throw 'dirty default';
form.reset();if(x.value!=='EF'||x.validity.patternMismatch)throw 'password reset';
x.value='G😀';x.removeAttribute('pattern');if(!x.checkValidity())throw 'raw script value';
"#);
 run(&mut d,&mut f,&mut p,"x.type='text';if(x.type!=='text'||x.value!=='G😀')throw 'text transition retains value';x.type='password';if(x.type!=='password'||x.value!=='G😀')throw 'password transition retains value';");
 let input=node(&d,"input");assert!(f.controls[&input].editable());let owner=node(&d,"form");assert!(f.validate_submission(&d,owner,None).is_ok());run(&mut d,&mut f,&mut p,"form.method='post';form.action='https://pattern-validation.test/submit';");let location=crate::navigation::address("https://pattern-validation.test/",&crate::navigation::Location::home()).unwrap();let sent=f.submit_request(&d,owner,None,&location,&location).unwrap();let crate::http::Request::FormPost{body,..}=sent.request else{panic!("native password POST")};assert_eq!(String::from_utf8(body).unwrap(),"q=G%F0%9F%98%80&other=");
}
