//! Actual App software-pointer proof for one existing native checkbox move.
//! Literal fixture input only; no OS clipboard, desktop window or network use.
use super::*;
use crate::html::NodeKind;
use std::sync::atomic::{AtomicUsize,Ordering};

fn id(app:&App,name:&str)->usize {(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap()}
fn clean(app:&App) {
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    let page=app.scripting.as_ref().unwrap();assert_eq!(page.error_count(),0,"{}",page.report());assert!(!page.has_navigation_request());
}
fn click(app:&mut App,name:&str) {
    let node=id(app,name);let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();
    let(x,page_y)=(rect.x+rect.width/2,rect.y+rect.height/2);assert_eq!(app.page.control_at(x,page_y),Some(node));
    let y=app.content_top()+page_y-app.scroll;
    assert!(matches!(app.pointer_down(x,y,false),PlatformAction::None));assert!(matches!(app.pointer_up(x,y),PlatformAction::None));clean(app);
}
fn key(app:&mut App,code:u32,shift:bool) {assert!(matches!(app.key(code,false,shift,false),PlatformAction::None));clean(app);}
fn neighbor(app:&App)->(String,usize,usize,usize,bool,Option<usize>) {
    let control=&app.forms.controls[&id(app,"neighbor")];
    (control.value.text(),control.value.cursor,control.value.anchor,control.value.offset,control.dirty_value,control.owner)
}
fn pixel(app:&App,canvas:&Canvas,x:i32,y:i32)->u32 {
    let y=app.content_top()+y-app.scroll;assert!(x>=0&&y>=0&&(x as usize)<canvas.width&&(y as usize)<canvas.height);
    canvas.pixels[y as usize*canvas.width+x as usize]
}
fn checkbox_pixels(app:&App,left:i32,checked:bool)->Vec<u32> {
    let node=id(app,"check");let controls:Vec<_>=app.page.controls.iter().filter(|rect|rect.node==node).collect();assert_eq!(controls.len(),1);
    let rect=controls[0];assert_eq!((rect.x,rect.y,rect.width,rect.height),(left,80,24,24));
    let border=app.page.border_rects[node].unwrap();assert_eq!((border.x,border.y,border.width,border.height),(left,80,24,24));
    for (dx, dy) in [(0, 0), (12, 12), (23, 23)] {
        let (x, y) = (left + dx, 80 + dy);
        let top = app.page.element_node_at(x, y);
        assert_eq!(app.page.control_at(x, y), Some(node),
            "checkbox checked={checked} at ({x},{y}), topmost node {top:?}, id {:?}",
            top.and_then(|item| app.document.attr(item, "id")));
    }
    assert_eq!(app.page.control_at(left-1,92),None);assert_eq!(app.page.control_at(left+24,92),None);
    let control=&app.forms.controls[&node];assert_eq!(control.kind,Kind::Checkbox);assert_eq!(control.checked,checked);assert_eq!(control.value.text(),"yes");assert_eq!(app.document.attr(node,"checked"),Some(""));assert_eq!(app.document.attr(node,"value"),Some("yes"));
    let canvas=app.draw();
    // Exclude the native focus border; compare the actual check mark and its
    // background independently of which software-pointer target owns focus.
    let pixels:Vec<_>=(3..21).flat_map(|y|(3..21).map(move|x|(x,y))).map(|(x,y)|pixel(app,&canvas,left+x,80+y)).collect();
    assert!(pixels.iter().any(|&color|color==0xffffff));
    assert_eq!(pixels.iter().any(|&color|color==0x123456),checked,"The real native checkbox mark must follow checkedness");pixels
}
fn link(app:&App) {
    let node=id(app,"proof-link");let rect=app.page.border_rects[node].unwrap();assert_eq!((rect.x,rect.y,rect.width,rect.height),(20,230,160,24));
    for(x,y)in[(20,230),(100,242),(179,253)] {assert_eq!(app.page.link_node_at(x,y),Some(node));assert_eq!(app.page.link_at(x,y),Some("#destination"));}
    assert_eq!(app.page.link_node_at(19,242),None);assert_eq!(app.page.link_node_at(180,242),None);
    let canvas=app.draw();assert_eq!(pixel(app,&canvas,24,234),0x336699);
}
fn query(app:&App,owner:&str)->String {
    // Preview the existing serializer against a reserved test origin. Actual
    // trusted submit events below are canceled before App navigation/requests.
    let current=crate::navigation::address("https://checkbox.test/start",&Location::home()).unwrap();
    app.forms.submit_request(&app.document,id(app,owner),None,&current,&current).unwrap().location.url.unwrap().query.unwrap_or_default()
}

#[test]
fn checkbox_move_actual_app_trusted_fragment_click_paint_submit_and_reset_preserve_neighbor() {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    struct Fixture(PathBuf);impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let fixture=Fixture(std::env::temp_dir().join(format!("chezcat-checkbox-move-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Checkbox ready</title><style>
html,body{margin:0;padding:0;background:#ffffff}
#neighbor{position:absolute;left:20px;top:20px;width:220px;height:32px;margin:0}
#source,#dest{position:absolute;top:80px;width:220px;height:32px;margin:0;padding:0}
#source{left:20px}#dest{left:280px}
#check{display:block;position:absolute;left:0;top:0;width:24px;height:24px;margin:0;color:#123456;background:#ffffff}
.action{position:absolute;width:180px;height:32px}#move{left:280px;top:20px}#submit-a{left:20px;top:130px}#submit-b{left:280px;top:130px}#reset-b{left:280px;top:180px}
#proof-link{display:block;position:absolute;left:20px;top:230px;width:160px;height:24px;background:#336699}
</style></head><body><form id=a action='/must-not-navigate-a'><input id=neighbor name=keep value='neighbor seed'><div id=source><input id=check type=checkbox name=choice value=yes checked></div><button id=submit-a type=submit class=action>Submit original A</button></form><form id=b action='/must-not-navigate-b'><div id=dest></div><button id=submit-b type=submit class=action>Submit original B</button><button id=reset-b type=reset class=action>Reset original B</button></form><button id=move type=button class=action>Move existing checkbox</button><a id=proof-link href=#destination></a><div id=destination></div><script>
var check=document.getElementById('check'),a=document.getElementById('a'),b=document.getElementById('b'),moved=false,clicks=0,fragment=null,donorList=null;
check.onclick=function(event){
 if(!event.isTrusted||this!==check||this.ownerDocument!==document||this.form!==(moved?b:a))throw 'trusted native checkbox activation';
 clicks++;document.title='Checkbox click '+clicks;
};
document.getElementById('move').onclick=function(event){
 if(!event.isTrusted||moved||check.form!==a||check.checked)throw 'one trusted existing-checkbox move';
 fragment=document.createDocumentFragment();donorList=fragment.childNodes;fragment.appendChild(check);
 if(check.parentNode!==fragment||check.getRootNode()!==fragment||check.form!==null||check.isConnected||check.ownerDocument!==document||donorList.length!==1||donorList[0]!==check)throw 'genuine detached native checkbox';
 var dest=document.getElementById('dest');if(dest.appendChild(fragment)!==fragment)throw 'real fragment splice';
 if(donorList!==fragment.childNodes||donorList.length!==0||fragment.parentNode!==null||fragment.isConnected||check.parentNode!==dest||check.form!==b||!check.isConnected||check.ownerDocument!==document||document.getElementById('check')!==check||check.checked)throw 'same checkbox at new form';
 moved=true;document.title='Checkbox moved';
};
a.onsubmit=function(event){if(!event.isTrusted||this!==a||check.form!==(moved?b:a))throw 'trusted A ownership';event.preventDefault();document.title='Submit A kept';};
b.onsubmit=function(event){if(!event.isTrusted||this!==b||check.form!==b)throw 'trusted B ownership';event.preventDefault();document.title='Submit B kept';};
b.onreset=function(event){if(!event.isTrusted||this!==b||check.form!==b)throw 'trusted B reset';document.title='Reset B applied';};
</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(640,480);app.set_javascript_enabled(true);app.open(&fixture.0);clean(&app);assert_eq!(app.document.title,"Checkbox ready");
    let check=id(&app,"check");let controls:Vec<_>=app.forms.controls.keys().copied().collect();let raw=app.raw.clone();let location=app.location.clone();let history=(app.history_index,app.history.len());
    let checked=checkbox_pixels(&app,20,true);link(&app);assert_eq!(app.forms.controls[&check].owner,Some(id(&app,"a")));
    click(&mut app,"neighbor");key(&mut app,0x24,false);key(&mut app,0x23,true);app.paste("neighbor edited");clean(&app);key(&mut app,0x24,false);
    for _ in 0..2 {key(&mut app,0x27,false);}for _ in 0..3 {key(&mut app,0x27,true);}
    let preserved=neighbor(&app);assert_eq!(preserved.0,"neighbor edited");assert_eq!((preserved.1,preserved.2),(5,2));assert!(preserved.4);assert_eq!(preserved.5,Some(id(&app,"a")));assert_eq!(app.forms.text_default_value(id(&app,"neighbor")).unwrap(),"neighbor seed");
    click(&mut app,"check");assert_eq!(app.document.title,"Checkbox click 1");let unchecked=checkbox_pixels(&app,20,false);assert_ne!(unchecked,checked);
    assert_eq!(query(&app,"a"),"keep=neighbor+edited");click(&mut app,"submit-a");assert_eq!(app.document.title,"Submit A kept");
    click(&mut app,"move");assert_eq!(app.document.title,"Checkbox moved");assert_eq!(id(&app,"check"),check);assert_eq!(app.forms.controls[&check].owner,Some(id(&app,"b")));assert_eq!(checkbox_pixels(&app,280,false),unchecked);
    assert_eq!(app.page.control_at(32,92),None,"Old checkbox hit region must disappear");let canvas=app.draw();assert_eq!(pixel(&app,&canvas,32,92),0xffffff);link(&app);
    let fragment=app.document.nodes.iter().position(|node|matches!(node.kind,NodeKind::DocumentFragment)).unwrap();assert!(app.document.nodes[fragment].children.is_empty());assert!(app.page.border_rects[fragment].is_none());assert!(!app.page.controls.iter().any(|control|control.node==fragment));
    assert_eq!(neighbor(&app),preserved);assert_eq!(query(&app,"a"),"keep=neighbor+edited");assert_eq!(query(&app,"b"),"");
    click(&mut app,"check");assert_eq!(app.document.title,"Checkbox click 2");assert_eq!(checkbox_pixels(&app,280,true),checked);assert_eq!(query(&app,"a"),"keep=neighbor+edited");assert_eq!(query(&app,"b"),"choice=yes");
    click(&mut app,"submit-a");assert_eq!(app.document.title,"Submit A kept");click(&mut app,"submit-b");assert_eq!(app.document.title,"Submit B kept");
    click(&mut app,"check");assert_eq!(app.document.title,"Checkbox click 3");assert_eq!(checkbox_pixels(&app,280,false),unchecked);assert_eq!(query(&app,"b"),"");
    click(&mut app,"reset-b");assert_eq!(app.document.title,"Reset B applied");assert_eq!(checkbox_pixels(&app,280,true),checked);assert_eq!(query(&app,"b"),"choice=yes");
    assert_eq!(neighbor(&app),preserved);assert_eq!(app.forms.text_default_value(id(&app,"neighbor")).unwrap(),"neighbor seed");assert_eq!(app.forms.controls.keys().copied().collect::<Vec<_>>(),controls);link(&app);
    assert_eq!(app.raw,raw);assert_eq!(app.location,location);assert_eq!((app.history_index,app.history.len()),history);clean(&app);
}
