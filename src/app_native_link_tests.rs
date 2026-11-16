//! Original real-App native-boundary controls. PRIVATE DRAFT; ALL UNRUN.
//! No actual native input, owner windows, clipboard, or live sites.
use super::*;
use std::fs;
use std::path::PathBuf;

struct Files(PathBuf);
impl Files {
    fn new()->Self{
        let mut random=[0u8;8];crate::random::fill(&mut random).unwrap();
        let suffix:String=random.iter().map(|byte|format!("{byte:02x}")).collect();
        let path=std::env::temp_dir().join(format!("chezcat-app-native-link-{suffix}"));fs::create_dir(&path).unwrap();Self(path)
    }
    fn app(&self)->App{
        fs::write(self.0.join("target.html"),"<!doctype html><title>Native boundary target</title><p>Destination</p>").unwrap();
        let path=self.0.join("source.html");
        fs::write(&path,"<!doctype html><title>Native boundary source</title><style>body{margin:0}a,select{display:block;width:180px;height:32px}#space{height:1800px}</style><body><a id=a href=target.html>Actual anchor</a><a href=target.html><input id=q type=checkbox></a><select id=s><option>First</option><option>Second</option></select><div id=space>Scroll</div><script>var calls=0,a=document.getElementById('a');a.onclick=function(){calls++;document.title='actual '+calls;};</script>").unwrap();
        let mut app=App::new();app.resize(800,600);app.open(&path);assert!(!app.failed,"{}",app.status);app
    }
}
impl Drop for Files{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
fn node(app:&App,id:&str)->usize{(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(id)).unwrap()}
fn point(app:&App,id:&str)->(i32,i32){let node=node(app,id);let rect=app.page.border_rects[node].unwrap();(rect.x+rect.width/2,app.content_top()+rect.y-app.scroll+rect.height/2)}
fn control(app:&App,id:&str)->(i32,i32){let node=node(app,id);let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();(rect.x+rect.width/2,app.content_top()+rect.y-app.scroll+rect.height/2)}
fn eval(app:&mut App,source:&str){let page=app.scripting.as_mut().unwrap();let outcome=page.console_eval(&mut app.document,&mut app.forms,source);assert_eq!(outcome.errors,0,"{}",page.report());if outcome.layout_changed{app.reflow();}}

#[test]
fn native_app_primary_capture_uses_real_down_and_consumes_one_guarded_callback(){
    let files=Files::new();let mut app=files.app();let(x,y)=point(&app,"a");let original=app.location.clone();
    let(action,hit)=app.primary_anchor_down(x,y,false);assert!(matches!(action,PlatformAction::None));let hit=hit.unwrap();
    assert!(app.anchor_hit_valid(&hit,x,y,false,0));app.pointer_cancel();
    let input=AnchorMouseInput{screen_x:-171.0,screen_y:212.0,..AnchorMouseInput::default()};
    assert!(matches!(app.activate_anchor(hit,x,y,false,input).unwrap(),AnchorDefault::Link(_,AnchorTrigger::Pointer(_))));
    assert_eq!(app.location,original);assert_eq!(app.document.title,"actual 1");eval(&mut app,"if(calls!==1)throw 'duplicate callback';");assert!(matches!(app.drag,Drag::None));
}

#[test]
fn native_app_suppressed_release_keeps_checkbox_and_blocks_legacy_anchor_default(){
    let files=Files::new();let mut app=files.app();let original=app.location.clone();let(x,y)=point(&app,"a");
    app.primary_anchor_down(x,y,false);app.pointer_up_without_link_default(x,y);
    assert_eq!(app.location,original);assert_eq!(app.document.title,"Native boundary source");assert!(matches!(app.drag,Drag::None));
    let(x,y)=control(&app,"q");let node=node(&app,"q");let(_,hit)=app.primary_anchor_down(x,y,false);assert!(hit.is_none(),"Nested control owns the native hit");
    app.pointer_up_without_link_default(x,y);assert!(app.forms.controls[&node].checked);assert_eq!(app.location,original);
}

#[test]
fn native_app_middle_never_enters_primary_control_selection_or_legacy_drag(){
    let files=Files::new();let mut app=files.app();let(x,y)=control(&app,"q");let node=node(&app,"q");let focus=app.focus;let original=app.location.clone();
    assert!(app.middle_anchor_down(x,y).is_none());assert!(!app.forms.controls[&node].checked);assert!(app.focus==focus);assert!(matches!(app.drag,Drag::None));assert_eq!(app.location,original);
    let(x,y)=point(&app,"a");let hit=app.middle_anchor_down(x,y).unwrap();assert!(app.anchor_hit_valid(&hit,x,y,false,1));assert!(!app.anchor_hit_valid(&hit,x,y,false,0));
    app.pointer_cancel();let input=AnchorMouseInput{button:1,screen_x:19.0,screen_y:-53.0,..AnchorMouseInput::default()};
    assert!(matches!(app.activate_anchor(hit,x,y,false,input).unwrap(),AnchorDefault::Link(_,AnchorTrigger::Pointer(actual))if actual.button==1));
    eval(&mut app,"if(calls!==0)throw 'middle fabricated primary click';");
}

#[test]
fn native_app_overlay_closing_down_cannot_rearm_an_old_or_underlying_anchor(){
    let files=Files::new();let mut app=files.app();let anchor=point(&app,"a");app.primary_anchor_down(anchor.0,anchor.1,false);
    let select=control(&app,"s");app.pointer_down(select.0,select.1,false);app.pointer_up(select.0,select.1);assert!(app.select_popup.is_some());
    let(_,hit)=app.primary_anchor_down(anchor.0,anchor.1,false);assert!(hit.is_none(),"Closing a select popup cannot create a fresh link gesture");
    app.pointer_up_without_link_default(anchor.0,anchor.1);assert_eq!(app.document.title,"Native boundary source");assert_eq!(app.location.path.as_ref().unwrap(),&files.0.join("source.html"));
}
