//! Actual App synchronous OUTPUT reset shares real child transactions and paint.
use super::*;
struct Fixture(std::path::PathBuf);
impl Drop for Fixture{fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
fn node(app:&App,id:&str)->usize{(0..app.document.nodes.len()).find(|&n|app.document.attr(n,"id")==Some(id)).unwrap()}
fn click(app:&mut App,n:usize){let r=app.page.controls.iter().find(|r|r.node==n).unwrap();let(x,y)=(r.x+r.width/2,app.content_top()+r.y+r.height/2-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);}
#[test]
fn form_reset_output_actual_app_trusted_cancel_reset_post_paint_and_links(){
 let file=Fixture(std::env::temp_dir().join(format!("chezcat-app-form-reset-output-{}.html",std::process::id())));
 std::fs::write(&file.0,r#"<!doctype html><style>body{margin:0;background:white}#edit{position:absolute;left:20px;top:80px;width:180px;height:30px}#cancel{position:absolute;left:20px;top:20px;width:130px;height:32px}#reset{position:absolute;left:170px;top:20px;width:130px;height:32px}#neighbor{position:absolute;left:220px;top:100px;width:160px;height:30px}#stable{position:absolute;left:420px;top:100px}#output{position:absolute;left:20px;top:150px;width:180px;height:30px}#paint{position:absolute;left:420px;top:160px;width:40px;height:30px;background:#114455}#paint.done{background:#cc2244}</style><form id=form method=post action=https://form-reset.test/submit><input id=edit name=q autocomplete=off><output id=output>Default output</output></form><input id=neighbor value=neighbor><button id=cancel type=button>Cancel reset</button><button id=reset type=button>Reset</button><a id=stable href=#destination>Go</a><div id=paint></div><div id=destination></div><script>
 var form=document.getElementById('form'),edit=document.getElementById('edit'),events=0,resets=0,cancel=true,received,output=document.getElementById('output');output.value='Changed output';
 edit.oninput=function(e){if(!e.isTrusted)throw 'untrusted input';events++;};
 form.onreset=function(e){if(!e.isTrusted||!e.bubbles||!e.cancelable||e.target!==form||edit.value!=='A😀ZQ')throw 'reset ordering';resets++;received=e;form.reset();if(cancel)e.preventDefault();};
 document.getElementById('cancel').onclick=function(e){if(!e.isTrusted||events!==4)throw 'trusted trigger';form.reset();if(resets!==1||edit.value!=='A😀ZQ'||output.value!=='Changed output'||!received.defaultPrevented)throw 'cancel reset';cancel=false;};
 document.getElementById('reset').onclick=function(e){if(!e.isTrusted)throw 'trusted reset trigger';form.reset();if(resets!==2||output.value!=='Default output'||edit.value!==''||edit.defaultValue!==''||received.currentTarget!==null||received.eventPhase!==0)throw 'synchronous reset';document.getElementById('paint').className='done';document.title='Form output reset checked';};
 </script>"#).unwrap();
 let mut app=App::new();app.resize(640,480);app.set_javascript_enabled(true);app.open(&file.0);assert!(!app.failed&&!app.is_loading(),"{}",app.status);
 let edit=node(&app,"edit");let owner=node(&app,"form");let neighbor=node(&app,"neighbor");let cancel=node(&app,"cancel");let reset=node(&app,"reset");let stable=node(&app,"stable");
 let raw=app.raw.clone();let location=app.location.clone();let epoch=app.script_epoch;
 let r=app.page.controls.iter().find(|r|r.node==edit).unwrap();let rect=(r.x,r.y,r.width,r.height);
 click(&mut app,edit);for ch in ['A','😀','Z','Q']{app.text_input(ch);}assert_eq!(app.forms.controls[&edit].value.text(),"A😀ZQ");
 app.forms.script_set_value(neighbor,"edited neighbor").unwrap();{let e=&mut app.forms.controls.get_mut(&neighbor).unwrap().value;e.cursor=5;e.anchor=2;e.offset=1;}
 let before=app.draw();click(&mut app,cancel);assert_eq!(app.forms.controls[&edit].value.text(),"A😀ZQ");assert!(app.forms.controls[&edit].dirty_value);click(&mut app,reset);
 assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());assert_eq!(app.document.title,"Form output reset checked");assert_eq!(app.forms.controls[&edit].value.text(),"");assert!(!app.forms.controls[&edit].dirty_value);
 let e=&app.forms.controls[&neighbor].value;assert_eq!((e.text(),e.cursor,e.anchor,e.offset),("edited neighbor".into(),5,2,1));
 let submission=app.forms.submit_request(&app.document,owner,None,&app.location,&app.base_location).unwrap();let crate::http::Request::FormPost{body,..}=submission.request else{panic!("POST")};assert_eq!(body,b"q=");
 let painted=app.draw();assert_eq!(painted.pixels[(app.content_top()+170)as usize*painted.width+430],0xcc2244);
 let r=app.page.controls.iter().find(|r|r.node==edit).unwrap();assert_eq!((r.x,r.y,r.width,r.height),rect);
 let(left,top,right,bottom)=(r.x.max(0)as usize,(app.content_top()+r.y-app.scroll).max(0)as usize,(r.x+r.width).min(painted.width as i32)as usize,(app.content_top()+r.y-app.scroll+r.height).min(painted.height as i32)as usize);
 assert!(left<right&&top<bottom);assert!((top..bottom).any(|y|before.pixels[y*before.width+left..y*before.width+right]!=painted.pixels[y*painted.width+left..y*painted.width+right]),"actual editor reset paint");
 let top=(app.content_top()+150)as usize;assert!((top..top+30).any(|y|before.pixels[y*before.width+20..y*before.width+200]!=painted.pixels[y*painted.width+20..y*painted.width+200]),"actual OUTPUT text paint");
 assert_eq!(app.page.link_node_at(430,110),Some(stable));assert_eq!((app.raw.clone(),app.location.clone(),app.script_epoch),(raw,location,epoch));
}
