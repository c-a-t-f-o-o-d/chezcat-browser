//! Original actual-App background-variable reduction; no framework or desktop UI.
use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};

fn id(app:&App,name:&str)->usize {
    (0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap()
}
fn pixel(app:&App,canvas:&Canvas,x:i32,y:i32)->u32 {
    let y=app.content_top()+y-app.scroll;
    assert!(x>=0&&y>=0&&(x as usize)<canvas.width&&(y as usize)<canvas.height);
    canvas.pixels[y as usize*canvas.width+x as usize]
}

#[test]
fn background_variable_actual_app_button_twins_paint_blue_with_real_control_hits() {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let fixture=Fixture(std::env::temp_dir().join(format!("chezcat-background-twins-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Background pending</title><style>
html,body{margin:0;padding:0;background:#ffffff;--ButtonFill:#0d6efd}
.twin{display:block;position:absolute;top:20px;width:180px;height:36px;color:#ffffff;font-size:18px}
#literal{left:20px;background-color:#0d6efd}
#variable{left:240px;background-color:var(--ButtonFill)}
</style></head><body><button id=literal name=literal type=button class=twin>Original twin</button><button id=variable name=variable type=button class=twin>Original twin</button><script>document.title='Background ready';</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(640,240);app.set_javascript_enabled(true);app.open(&fixture.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    assert_eq!(app.document.title,"Background ready");
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
    let literal=id(&app,"literal");let variable=id(&app,"variable");
    let a=app.page.controls.iter().find(|control|control.node==literal).unwrap();
    let b=app.page.controls.iter().find(|control|control.node==variable).unwrap();
    assert_eq!((a.x,a.y,a.width,a.height),(20,20,180,36));
    assert_eq!((b.x,b.y,b.width,b.height),(240,20,180,36));
    for (node,control) in [(literal,a),(variable,b)] {
        let border=app.page.border_rects[node].unwrap();
        assert_eq!((border.x,border.y,border.width,border.height),(control.x,control.y,control.width,control.height));
        for (dx,dy) in [(0,0),(90,18),(179,35)] {
            assert_eq!(app.page.control_at(control.x+dx,control.y+dy),Some(node));
        }
        assert_eq!(app.page.control_at(control.x-1,control.y+18),None);
        assert_eq!(app.page.control_at(control.x+180,control.y+18),None);
    }
    let canvas=app.draw();
    assert_eq!(pixel(&app,&canvas,a.x+2,a.y+2),0x0d6efd,"Literal background must already paint the exact original blue");
    assert_eq!(pixel(&app,&canvas,b.x+2,b.y+2),0x0d6efd,"A variable background must reach the real native button painter");
    for y in 0..36 {for x in 0..180 {
        assert_eq!(pixel(&app,&canvas,b.x+x,b.y+y),pixel(&app,&canvas,a.x+x,a.y+y),"Twin relative pixel ({x},{y})");
    }}
    assert_eq!(pixel(&app,&canvas,b.x-1,b.y+2),0xffffff);
    assert_eq!(app.forms.controls.len(),2);
    assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
}

fn click(app:&mut App,name:&str) {
    let node=id(app,name);let control=app.page.controls.iter().find(|control|control.node==node).unwrap();
    let(x,page_y)=(control.x+control.width/2,control.y+control.height/2);
    assert_eq!(app.page.control_at(x,page_y),Some(node));
    let y=app.content_top()+page_y-app.scroll;
    assert!(matches!(app.pointer_down(x,y,false),PlatformAction::None));
    assert!(matches!(app.pointer_up(x,y),PlatformAction::None));
}
fn neighbor(app:&App)->(String,usize,usize,usize,bool,Option<usize>) {
    let control=&app.forms.controls[&id(app,"neighbor")];
    (control.value.text(),control.value.cursor,control.value.anchor,control.value.offset,control.dirty_value,control.owner)
}

#[test]
fn background_variable_actual_app_trusted_class_change_repaints_and_preserves_neighbor_editor() {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    struct Fixture(PathBuf);impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let fixture=Fixture(std::env::temp_dir().join(format!("chezcat-background-live-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Live background ready</title><style>
html,body{margin:0;padding:0;background:#ffffff}
#theme{--ButtonFill:#0d6efd}#theme.changed{--ButtonFill:#b02070}
.twin{display:block;position:absolute;top:20px;width:180px;height:36px;color:#ffffff;font-size:18px}
#literal{left:20px;background-color:#b02070}#variable{left:240px;background-color:var(--ButtonFill)}
#change{position:absolute;left:20px;top:80px;width:180px;height:32px}
#neighbor{position:absolute;left:240px;top:80px;width:180px;height:32px}
</style></head><body><button id=literal type=button class=twin>Live original twin</button><div id=theme><button id=variable type=button class=twin>Live original twin</button></div><button id=change type=button>Change inherited fill</button><form id=g><input id=neighbor name=neighbor value="neighbor seed"></form><script>
document.getElementById('change').onclick=function(event){
 if(!event.isTrusted)throw 'trusted original activation required';
 document.getElementById('theme').className='changed';document.title='Live background changed';
};
</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(640,280);app.set_javascript_enabled(true);app.open(&fixture.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    let canvas=app.draw();assert_eq!(pixel(&app,&canvas,22,22),0xb02070);assert_eq!(pixel(&app,&canvas,242,22),0x0d6efd);
    let raw=app.raw.clone();let location=app.location.clone();let history=(app.history_index,app.history.len());let nodes=app.document.nodes.as_ptr();
    click(&mut app,"neighbor");
    assert!(matches!(app.key(0x24,false,false,false),PlatformAction::None));
    assert!(matches!(app.key(0x23,false,true,false),PlatformAction::None));app.paste("neighbor edited");
    assert!(matches!(app.key(0x24,false,false,false),PlatformAction::None));
    for _ in 0..2 {assert!(matches!(app.key(0x27,false,false,false),PlatformAction::None));}
    for _ in 0..3 {assert!(matches!(app.key(0x27,false,true,false),PlatformAction::None));}
    let preserved=neighbor(&app);assert_eq!(preserved.0,"neighbor edited");assert_eq!((preserved.1,preserved.2),(5,2));assert!(preserved.4);assert_eq!(preserved.5,Some(id(&app,"g")));
    click(&mut app,"change");
    assert_eq!(app.document.title,"Live background changed");assert_eq!(app.document.attr(id(&app,"theme"),"class"),Some("changed"));
    assert_eq!(neighbor(&app),preserved);assert_eq!(app.forms.text_default_value(id(&app,"neighbor")).unwrap(),"neighbor seed");
    let canvas=app.draw();
    for(name,left)in[("literal",20),("variable",240)] {
        let node=id(&app,name);let control=app.page.controls.iter().find(|control|control.node==node).unwrap();
        assert_eq!((control.x,control.y,control.width,control.height),(left,20,180,36));
        assert_eq!(pixel(&app,&canvas,left+2,22),0xb02070,"{name}");
        for(dx,dy)in[(0,0),(90,18),(179,35)] {assert_eq!(app.page.control_at(left+dx,20+dy),Some(node));}
        assert_eq!(app.page.control_at(left-1,38),None);assert_eq!(app.page.control_at(left+180,38),None);
    }
    for y in 0..36 {for x in 0..180 {assert_eq!(pixel(&app,&canvas,240+x,20+y),pixel(&app,&canvas,20+x,20+y),"Live twin relative pixel ({x},{y})");}}
    assert_eq!(app.document.nodes.as_ptr(),nodes);assert_eq!(app.raw,raw);assert_eq!(app.location,location);assert_eq!((app.history_index,app.history.len()),history);assert_eq!(app.forms.controls.len(),4);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
    assert!(!app.failed&&!app.is_loading());assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
}
