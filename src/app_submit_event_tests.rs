//! Actual App chosen-submitter delivery, cancellation, POST data and paint.
use super::*;
struct Fixture(std::path::PathBuf);
impl Drop for Fixture{fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
fn node(app:&App,id:&str)->usize{(0..app.document.nodes.len()).find(|&n|app.document.attr(n,"id")==Some(id)).unwrap()}
fn click(app:&mut App,n:usize){let r=app.page.controls.iter().find(|r|r.node==n).unwrap();let(x,y)=(r.x+r.width/2,app.content_top()+r.y+r.height/2-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);}
#[test]
fn submit_event_actual_app_chosen_buttons_cancel_post_paint_and_links(){
 let file=Fixture(std::env::temp_dir().join(format!("chezcat-app-submit-event-{}.html",std::process::id())));
 std::fs::write(&file.0,r#"<!doctype html><style>body{margin:0;background:white}#edit{position:absolute;left:20px;top:80px;width:180px;height:30px}#first{position:absolute;left:20px;top:20px;width:130px;height:32px}#second{position:absolute;left:170px;top:20px;width:130px;height:32px}#neighbor{position:absolute;left:220px;top:100px;width:160px;height:30px}#stable{position:absolute;left:420px;top:100px}#paint{position:absolute;left:420px;top:160px;width:40px;height:30px;background:#114455}#paint.done{background:#cc2244}</style><form id=form method=post action=https://submit-event.test/submit><input id=edit name=q autocomplete=off><input id=first type=submit name=choice value=first><button id=second type=submit name=choice value=second>Second</button></form><input id=neighbor value=neighbor><a id=stable href=#destination>Go</a><div id=paint></div><div id=destination></div><script>
 var form=document.getElementById('form'),first=document.getElementById('first'),second=document.getElementById('second'),calls=0;
 form.onsubmit=function(e){var chosen=calls===0?first:second;if(!(e instanceof SubmitEvent)||!(e instanceof Event)||e.submitter!==chosen||e.target!==form||!e.isTrusted||!e.bubbles||!e.cancelable||e.composed)throw 'actual chosen submitter';calls++;e.preventDefault();if(!e.defaultPrevented)throw 'cancel';document.getElementById('paint').className='done';document.title='Chosen submitter '+calls;};
 </script>"#).unwrap();
 let mut app=App::new();app.resize(640,480);app.set_javascript_enabled(true);app.open(&file.0);assert!(!app.failed&&!app.is_loading(),"{}",app.status);
 let edit=node(&app,"edit");let owner=node(&app,"form");let first=node(&app,"first");let second=node(&app,"second");let neighbor=node(&app,"neighbor");let stable=node(&app,"stable");
 let raw=app.raw.clone();let location=app.location.clone();let epoch=app.script_epoch;let r=app.page.controls.iter().find(|r|r.node==edit).unwrap();let rect=(r.x,r.y,r.width,r.height);
 click(&mut app,edit);for ch in ['A','😀','Z']{app.text_input(ch);}assert_eq!(app.forms.controls[&edit].value.text(),"A😀Z");app.forms.script_set_value(neighbor,"edited neighbor").unwrap();{let e=&mut app.forms.controls.get_mut(&neighbor).unwrap().value;e.cursor=5;e.anchor=2;e.offset=1;}
 let before=app.draw();click(&mut app,first);assert_eq!(app.document.title,"Chosen submitter 1");click(&mut app,second);assert_eq!(app.document.title,"Chosen submitter 2");assert!(!app.is_loading());assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
 for(chosen,name)in[(first,"first"),(second,"second")]{let submission=app.forms.submit_request(&app.document,owner,Some(chosen),&app.location,&app.base_location).unwrap();let crate::http::Request::FormPost{body,..}=submission.request else{panic!("POST")};assert_eq!(body,format!("q=A%F0%9F%98%80Z&choice={name}").as_bytes());}
 let e=&app.forms.controls[&neighbor].value;assert_eq!((e.text(),e.cursor,e.anchor,e.offset),("edited neighbor".into(),5,2,1));let painted=app.draw();let pixel=(app.content_top()+170)as usize*painted.width+430;assert_eq!(painted.pixels[pixel],0xcc2244);assert_ne!(before.pixels[pixel],painted.pixels[pixel]);let r=app.page.controls.iter().find(|r|r.node==edit).unwrap();assert_eq!((r.x,r.y,r.width,r.height),rect);assert_eq!(app.page.link_node_at(430,110),Some(stable));assert_eq!((app.raw.clone(),app.location.clone(),app.script_epoch),(raw,location,epoch));
}
