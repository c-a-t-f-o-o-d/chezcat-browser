//! Actual App software input/paint proof for a newly admitted native BUTTON.
//! All text is literal test data; no OS clipboard, desktop UI or network use.
use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};

fn id(app:&App,name:&str)->usize {
    (0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap()
}
fn clean(app:&App) {
    assert!(!app.failed,"{}",app.status);
    assert!(!app.is_loading());
    let page=app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(),0,"{}",page.report());
    assert!(!page.has_navigation_request());
}
fn click(app:&mut App,name:&str) {
    let node=id(app,name);let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();
    let (x,page_y)=(rect.x+rect.width/2,rect.y+rect.height/2);
    assert_eq!(app.page.control_at(x,page_y),Some(node));
    let y=app.content_top()+page_y-app.scroll;
    assert!(matches!(app.pointer_down(x,y,false),PlatformAction::None));
    assert!(matches!(app.pointer_up(x,y),PlatformAction::None));
    clean(app);
}
fn key(app:&mut App,code:u32,shift:bool) {
    assert!(matches!(app.key(code,false,shift,false),PlatformAction::None));clean(app);
}
fn replace_literal(app:&mut App,name:&str,text:&str) {
    click(app,name);key(app,0x24,false);key(app,0x23,true);app.paste(text);clean(app);
}
fn neighbor(app:&App)->(String,usize,usize,usize,bool,Option<usize>) {
    let c=&app.forms.controls[&id(app,"neighbor")];
    (c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.owner)
}
fn output_pixels(app:&App,expected:&str)->Vec<u32> {
    let rect=app.page.border_rects[id(app,"out")].unwrap();
    assert_eq!((rect.x,rect.y,rect.width,rect.height),(20,180,400,40));
    let runs:Vec<_>=app.page.runs.iter().filter(|run|run.x>=20&&run.x<420&&run.y>=180&&run.y<220).collect();
    assert_eq!(runs.iter().map(|run|run.text.as_str()).collect::<Vec<_>>(),expected.split(' ').collect::<Vec<_>>());
    for pair in runs.windows(2) {
        assert_eq!(pair[1].separator_before," ","The original counter keeps its single logical space");
        assert_eq!(pair[1].y,pair[0].y,"The counter remains on one rendered line");
        assert!(pair[1].x>pair[0].x+pair[0].width(),"The rendered words retain a visible spacing gap");
    }
    assert!(runs.iter().all(|run|run.color==0x123456));
    let canvas=app.draw();let top=(app.content_top()+180-app.scroll)as usize;
    let pixels:Vec<_>=(top..top+40).flat_map(|y|canvas.pixels[y*canvas.width+20..y*canvas.width+420].iter().copied()).collect();
    assert!(pixels.iter().any(|&pixel|pixel!=0xffffff));assert_eq!(pixels.last(),Some(&0xffffff));pixels
}

#[test]
fn created_button_actual_app_source_order_paints_clicks_and_uses_native_modes() {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let fixture=Fixture(std::env::temp_dir().join(format!("chezcat-created-button-app-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))));
    std::fs::write(&fixture.0,r#"<!doctype html><title>Button ready</title><style>
html,body{margin:0;padding:0;background:#ffffff}
input,button{display:block;width:220px;height:32px;margin:0}
#neighbor{position:absolute;left:20px;top:20px}#field{position:absolute;left:20px;top:70px}
#host{position:absolute;left:20px;top:120px;width:220px;height:32px}
#create,#to-reset,#to-submit{position:absolute;left:280px;width:180px;height:32px}
#create{top:20px}#to-reset{top:70px}#to-submit{top:120px}
#out{position:absolute;left:20px;top:180px;width:400px;height:40px;margin:0;padding:0;border:0;background:#ffffff;color:#123456;font-size:24px;line-height:32px}
</style><body><form id=g><input id=neighbor name=neighbor value="neighbor seed"></form>
<form id=f action="/unrequested"><input id=field name=field value="field seed"><div id=host></div></form>
<button id=create type=button>Create original button</button><button id=to-reset type=button>Use reset mode</button><button id=to-submit type=button>Use submit mode</button><p id=out>Before</p><script>
var dynamic=null,count=0;
document.getElementById('create').onclick=function(event){
 if(!event.isTrusted||dynamic!==null)throw 'one trusted creation required';
 dynamic=document.createElement('button');dynamic.setAttribute('name','increment');dynamic.setAttribute('type','button');dynamic.value='payload';
 dynamic.appendChild(document.createTextNode('Increment original counter'));dynamic.id='dynamic';
 dynamic.onclick=function(event){
  if(!event.isTrusted||this!==dynamic||this.form!==document.getElementById('f'))throw 'trusted native button required';
  count++;document.title='Clicked '+count;document.getElementById('out').textContent='Counter '+count;
 };
 document.getElementById('host').appendChild(dynamic);document.title='Button created';document.getElementById('out').textContent='Created';
};
document.getElementById('to-reset').onclick=function(event){if(!event.isTrusted)throw 'trusted mode change';dynamic.type='reset';};
document.getElementById('to-submit').onclick=function(event){if(!event.isTrusted)throw 'trusted mode change';dynamic.name='send';dynamic.value='send payload';dynamic.type='submit';};
document.getElementById('f').onsubmit=function(event){
 if(!event.isTrusted||dynamic.type!=='submit'||dynamic.value!=='send payload')throw 'genuine native submission';
 event.preventDefault();document.title='Submission kept';document.getElementById('out').textContent='Submit kept';
};
</script>"#).unwrap();
    let mut app=App::new();app.resize(640,480);app.set_javascript_enabled(true);app.open(&fixture.0);clean(&app);
    assert_eq!(app.document.title,"Button ready");
    let raw=app.raw.clone();let location=app.location.clone();let history=(app.history_index,app.history.len());
    replace_literal(&mut app,"neighbor","neighbor kept");
    key(&mut app,0x24,false);for _ in 0..2{key(&mut app,0x27,false);}for _ in 0..3{key(&mut app,0x27,true);}
    let preserved=neighbor(&app);assert_eq!((preserved.1,preserved.2),(5,2));assert!(preserved.4);
    replace_literal(&mut app,"field","field edited");
    let before=output_pixels(&app,"Before");
    click(&mut app,"create");assert_eq!(app.document.title,"Button created");
    let dynamic=id(&app,"dynamic");let control=&app.forms.controls[&dynamic];
    assert_eq!(control.kind,Kind::Button);assert_eq!(control.name,"increment");assert_eq!(control.value.text(),"payload");
    assert_eq!(control.caption,"Increment original counter");assert_eq!(control.owner,Some(id(&app,"f")));
    let rect=app.page.controls.iter().find(|rect|rect.node==dynamic).unwrap();
    assert_eq!((rect.x,rect.y,rect.width,rect.height),(20,120,220,32));
    let border=app.page.border_rects[dynamic].unwrap();assert_eq!((border.x,border.y,border.width,border.height),(20,120,220,32));
    let canvas=app.draw();let top=(app.content_top()+120-app.scroll)as usize;
    assert!((top..top+32).any(|y|canvas.pixels[y*canvas.width+20..y*canvas.width+240].iter().any(|&pixel|pixel!=0xffffff)));
    assert!((top+5..top+27).any(|y|canvas.pixels[y*canvas.width+25..y*canvas.width+235].iter().any(|&pixel|pixel!=0xe3edf2)),
        "Caption glyphs must paint inside the unfocused native background, away from its border");
    assert_ne!(output_pixels(&app,"Created"),before);
    assert_eq!(neighbor(&app),preserved);
    click(&mut app,"dynamic");assert_eq!(app.document.title,"Clicked 1");output_pixels(&app,"Counter 1");
    assert_eq!(app.forms.controls[&id(&app,"field")].value.text(),"field edited","type=button must not reset or submit");
    click(&mut app,"to-reset");assert_eq!(app.forms.controls[&dynamic].kind,Kind::Reset);
    click(&mut app,"dynamic");assert_eq!(app.document.title,"Clicked 2");assert_eq!(app.forms.controls[&id(&app,"field")].value.text(),"field seed");
    assert_eq!(app.forms.controls[&dynamic].value.text(),"payload");assert_eq!(neighbor(&app),preserved);
    click(&mut app,"to-submit");assert_eq!(app.forms.controls[&dynamic].kind,Kind::Submit);
    assert_eq!(app.forms.controls[&dynamic].name,"send");
    click(&mut app,"dynamic");assert_eq!(app.document.title,"Submission kept");output_pixels(&app,"Submit kept");
    assert_eq!(neighbor(&app),preserved);assert_eq!(app.forms.controls[&dynamic].caption,"Increment original counter");
    assert_eq!(app.raw,raw);assert_eq!(app.location,location);assert_eq!((app.history_index,app.history.len()),history);clean(&app);
}
