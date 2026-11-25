//! Native required-field validation through actual App activation.
use super::*;
struct Fixture(std::path::PathBuf);
impl Drop for Fixture{fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
fn node(app:&App,id:&str)->usize{(0..app.document.nodes.len()).find(|&n|app.document.attr(n,"id")==Some(id)).unwrap()}
fn click(app:&mut App,n:usize){let r=app.page.controls.iter().find(|r|r.node==n).unwrap();let(x,y)=(r.x+r.width/2,app.content_top()+r.y+r.height/2-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);}
#[test]
fn native_invalid_actual_app_events_cancel_reporting_and_block_submit(){
 let file=Fixture(std::env::temp_dir().join(format!("chezcat-app-invalid-{}.html",std::process::id())));
 std::fs::write(&file.0,r#"<!doctype html><style>body{margin:0;background:white}input,button{display:block}#paint{position:absolute;left:420px;top:160px;width:40px;height:30px;background:#114455}#paint.done{background:#cc2244}#stable{position:absolute;left:420px;top:100px}</style><form id=owner action=https://invalid.test/submit><input id=first name=a required><input id=second name=b required><button id=send name=go>Send</button></form><input id=neighbor value=neighbor><a id=stable href=#destination>Go</a><div id=paint></div><div id=destination></div><script>
 var calls='',submits=0,owner=document.getElementById('owner');owner.addEventListener('invalid',function(e){if(e.bubbles||!e.cancelable||e.composed||!e.isTrusted||!(e instanceof Event)||e.eventPhase!==1)throw 'invalid metadata';calls+=e.target.id;document.getElementById('paint').className='done';e.preventDefault();},true);owner.addEventListener('invalid',function(){throw 'invalid bubbled';});owner.onsubmit=function(e){submits++;e.preventDefault();};
 </script>"#).unwrap();
 let mut app=App::new();app.resize(640,480);app.set_javascript_enabled(true);app.open(&file.0);assert!(!app.failed&&!app.is_loading(),"{}",app.status);
 let send=node(&app,"send");let neighbor=node(&app,"neighbor");let stable=node(&app,"stable");let raw=app.raw.clone();let location=app.location.clone();let epoch=app.script_epoch;app.forms.script_set_value(neighbor,"edited neighbor").unwrap();{let e=&mut app.forms.controls.get_mut(&neighbor).unwrap().value;e.cursor=5;e.anchor=2;e.offset=1;}
 let before=app.draw();click(&mut app,send);assert!(!app.is_loading());assert!(!app.failed,"canceled invalid reporting: {}",app.status);
 let outcome=app.scripting.as_mut().unwrap().console_eval(&mut app.document,&mut app.forms,"if(calls!=='firstsecond'||submits!==0)throw 'ordered invalid '+calls;document.title='Invalid events';");assert_eq!(outcome.errors,0,"{}",app.scripting.as_ref().unwrap().report());assert_eq!(app.document.title,"Invalid events");
 let e=&app.forms.controls[&neighbor].value;assert_eq!((e.text(),e.cursor,e.anchor,e.offset),("edited neighbor".into(),5,2,1));let painted=app.draw();let pixel=(app.content_top()+170)as usize*painted.width+430;assert_eq!(painted.pixels[pixel],0xcc2244);assert_ne!(before.pixels[pixel],painted.pixels[pixel]);assert_eq!(app.page.link_node_at(430,110),Some(stable));
 assert_eq!((app.raw.clone(),app.location.clone(),app.script_epoch),(raw,location,epoch));
}
#[test]
fn native_invalid_actual_app_reports_focus_and_fresh_editing_recovery(){
 for scripting in [false,true]{
  let file=Fixture(std::env::temp_dir().join(format!("chezcat-app-invalid-focus-{}-{scripting}.html",std::process::id())));
  std::fs::write(&file.0,"<!doctype html><style>input,button{display:block}</style><form id=owner><input id=first name=a required><input id=second name=b required><button id=send name=go>Send</button></form><script>var calls='';document.getElementById('first').oninvalid=function(){calls+='a';};document.getElementById('second').oninvalid=function(e){calls+='b';e.preventDefault();};document.getElementById('owner').onsubmit=function(e){document.title='Ready';e.preventDefault();};</script>").unwrap();
  let mut app=App::new();app.resize(640,480);app.set_javascript_enabled(scripting);app.open(&file.0);let first=node(&app,"first");let second=node(&app,"second");let send=node(&app,"send");let r=app.page.controls.iter().find(|r|r.node==first).unwrap();let rect=(r.x,r.y,r.width,r.height);
  click(&mut app,send);assert!(app.failed&&!app.is_loading());assert_eq!(app.status,"Complete the required field: a.");assert!(matches!(app.focus,Focus::Control(n)if n==first));let r=app.page.controls.iter().find(|r|r.node==first).unwrap();assert_eq!((r.x,r.y,r.width,r.height),rect);app.text_input('A');assert_eq!(app.forms.controls[&first].value.text(),"A");click(&mut app,second);app.text_input('B');assert_eq!(app.forms.controls[&second].value.text(),"B");
  if scripting{click(&mut app,send);assert_eq!(app.document.title,"Ready");assert!(!app.is_loading());let out=app.scripting.as_mut().unwrap().console_eval(&mut app.document,&mut app.forms,"if(calls!=='ab')throw 'single validation pass '+calls;");assert_eq!(out.errors,0,"{}",app.scripting.as_ref().unwrap().report());}
 }
}
