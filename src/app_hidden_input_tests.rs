//! Genuine App software input, hidden form state and no painted hidden control.
use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};

fn text_pixels(app:&App,output:usize,expected:&str)->Vec<u32>{
    let rect=app.page.border_rects[output].expect("Visible result owns a real layout box");
    assert_eq!((rect.x,rect.y,rect.width,rect.height),(20,180,300,40));
    let runs:Vec<_>=app.page.runs.iter().filter(|run|run.x>=rect.x&&run.x<rect.x+rect.width
        &&run.y>=rect.y&&run.y<rect.y+rect.height).collect();
    assert_eq!(runs.iter().map(|run|run.text.as_str()).collect::<Vec<_>>(),[expected]);
    assert!(runs.iter().all(|run|run.color==0x123456));
    let canvas=app.draw();let left=rect.x as usize;let top=(app.content_top()+rect.y-app.scroll)as usize;
    assert!(left+rect.width as usize<=canvas.width&&top+rect.height as usize<=canvas.height);
    let pixels:Vec<_>=(top..top+rect.height as usize).flat_map(|y|
        canvas.pixels[y*canvas.width+left..y*canvas.width+left+rect.width as usize].iter().copied()).collect();
    assert!(pixels.iter().any(|&pixel|pixel!=0xffffff));assert_eq!(pixels.last(),Some(&0xffffff));pixels
}

#[test]
fn hidden_input_actual_app_click_preserves_visible_controls_and_has_no_box_or_focus() {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    let path=std::env::temp_dir().join(format!("chezcat-hidden-input-app-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture{fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let fixture=Fixture(path);
    std::fs::write(&fixture.0,r#"<!doctype html><title>Hidden ready</title><style>body{margin:0;background:#ffffff}button{display:block;width:180px;height:32px}input[type=hidden]{display:block;width:900px;height:300px}#out{position:absolute;left:20px;top:180px;width:300px;height:40px;margin:0;padding:0;border:0;background:#ffffff;color:#123456;font-size:24px;line-height:32px}</style><body><form id=f action='/result'><div id=host></div><input id=keep name=q value=original><button id=go type=button>Make hidden input</button></form><p id=out>Before</p><script>
document.getElementById('go').onclick=function(e){if(!e.isTrusted)throw 'real click required';var input=document.createElement('input');input.type='hidden';input.id='new';input.name='D';input.value='kept';document.getElementById('host').appendChild(input);input.focus();document.getElementById('out').textContent='HiddenReady';document.title='Hidden complete';};
</script>"#).unwrap();
    let mut app=App::new();app.resize(640,480);app.set_javascript_enabled(true);app.open(&fixture.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    let node=|app:&App,name:&str|(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap();
    let keep=node(&app,"keep");app.forms.script_set_value(keep,"edited").unwrap();
    app.forms.controls.get_mut(&keep).unwrap().value.cursor=2;
    app.forms.controls.get_mut(&keep).unwrap().value.anchor=4;
    let output=node(&app,"out");let before=text_pixels(&app,output,"Before");
    let raw=app.raw.clone();let location=app.location.clone();let history=(app.history_index,app.history.len());
    let button=node(&app,"go");let rect=app.page.controls.iter().find(|rect|rect.node==button).unwrap();
    let (x,y)=(rect.x+rect.width/2,app.content_top()+rect.y+rect.height/2-app.scroll);
    assert!(matches!(app.pointer_down(x,y,false),PlatformAction::None));
    assert_eq!(app.document.title,"Hidden ready","Down alone does not run the authored action");
    assert!(matches!(app.pointer_up(x,y),PlatformAction::None));
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{:?}",app.events);
    assert_eq!(app.document.title,"Hidden complete");let input=node(&app,"new");
    assert_eq!(app.forms.controls[&input].kind,Kind::Hidden);
    assert_eq!(app.forms.controls[&input].value.text(),"kept");
    assert!(app.page.border_rects[input].is_none(),"Hidden input cannot gain a box from author display rules");
    assert!(!app.page.controls.iter().any(|control|control.node==input));
    assert!(!matches!(app.focus,Focus::Control(control)if control==input));
    assert_eq!(app.forms.controls[&keep].value.text(),"edited");
    assert_eq!((app.forms.controls[&keep].value.cursor,app.forms.controls[&keep].value.anchor),(2,4));
    assert_ne!(text_pixels(&app,output,"HiddenReady"),before,"Only the authored result changes in the fixed white text crop");
    assert_eq!(app.raw,raw);assert_eq!(app.location,location);assert_eq!((app.history_index,app.history.len()),history);
    let current=crate::navigation::address("http://fixture.test/start",&crate::navigation::Location::home()).unwrap();
    assert_eq!(app.forms.submit_request(&app.document,node(&app,"f"),None,&current,&current).unwrap().location.url.unwrap().query.as_deref(),Some("D=kept&q=edited"));
}
