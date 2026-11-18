//! Original native-control consumer proof for supported literal CSS padding.
//! Real App layout/paint/hits and software input; no framework or OS clipboard.
use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};

fn id(app:&App,name:&str)->usize {
    (0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap()
}
fn clean(app:&App) {
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    let page=app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(),0,"{}",page.report());
    assert!(!page.has_navigation_request());
}
fn pixel(app:&App,canvas:&Canvas,x:i32,y:i32)->u32 {
    let y=app.content_top()+y-app.scroll;
    assert!(x>=0&&y>=0&&(x as usize)<canvas.width&&(y as usize)<canvas.height);
    canvas.pixels[y as usize*canvas.width+x as usize]
}
fn key(app:&mut App,code:u32,shift:bool) {
    assert!(matches!(app.key(code,false,shift,false),PlatformAction::None));
    clean(app);
}
fn neighbor(app:&App)->(String,usize,usize,usize,bool,Option<usize>) {
    let control=&app.forms.controls[&id(app,"neighbor")];
    (control.value.text(),control.value.cursor,control.value.anchor,control.value.offset,control.dirty_value,control.owner)
}

#[test]
fn native_padding_actual_app_literal_content_box_geometry_pixels_hits_and_neighbor_state() {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let fixture=Fixture(std::env::temp_dir().join(format!("chezcat-native-padding-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Native padding pending</title><style>
html,body{margin:0;padding:0;background:#ffffff}
.twin{position:absolute;display:block;top:20px;width:80px;height:24px;box-sizing:content-box;padding:6px 10px;border:0;margin:0;background:#0d6efd;color:#ffffff}
#literal{left:20px}#native{left:180px}
#neighbor{position:absolute;display:block;left:20px;top:100px;width:220px;height:32px;margin:0}
</style></head><body><div id=literal class=twin>X</div><button id=native name=go type=button class=twin>X</button><form id=f><input id=neighbor name=keep value="neighbor seed"></form><script>
document.getElementById('native').onclick=function(event){if(!event.isTrusted||this!==document.getElementById('native'))throw 'trusted original padded button';document.title='Native padding clicked';};
document.title='Native padding ready';
</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(480,300);app.set_javascript_enabled(true);app.open(&fixture.0);
    clean(&app);assert_eq!(app.document.title,"Native padding ready");
    let raw=app.raw.clone();let location=app.location.clone();let history=(app.history_index,app.history.len());
    let controls=app.forms.controls.keys().copied().collect::<Vec<_>>();

    // Edit the real neighboring native editor, then retain a nonempty selection.
    let neighbor_node=id(&app,"neighbor");
    let field=app.page.controls.iter().find(|rect|rect.node==neighbor_node).unwrap().clone();
    let (x,y)=(field.x+field.width/2,app.content_top()+field.y+field.height/2-app.scroll);
    assert_eq!(app.page.control_at(x,field.y+field.height/2),Some(neighbor_node));
    assert!(matches!(app.pointer_down(x,y,false),PlatformAction::None));
    assert!(matches!(app.pointer_up(x,y),PlatformAction::None));
    assert!(matches!(app.focus,Focus::Control(node) if node==neighbor_node));
    key(&mut app,0x24,false);key(&mut app,0x23,true);app.paste("neighbor edited");clean(&app);
    key(&mut app,0x24,false);for _ in 0..2 {key(&mut app,0x27,false);}for _ in 0..3 {key(&mut app,0x27,true);}
    let preserved=neighbor(&app);
    assert_eq!(preserved.0,"neighbor edited");assert_eq!((preserved.1,preserved.2),(5,2));
    assert!(preserved.4);assert_eq!(preserved.5,Some(id(&app,"f")));
    assert_eq!(app.forms.text_default_value(neighbor_node).unwrap(),"neighbor seed");

    // Establish the already-supported literal box before testing the consumer.
    let literal=app.page.border_rects[id(&app,"literal")].unwrap();
    assert_eq!((literal.x,literal.y,literal.width,literal.height),(20,20,100,36));
    let native_node=id(&app,"native");
    let native=app.page.controls.iter().find(|rect|rect.node==native_node).unwrap().clone();
    assert_eq!((native.x,native.y),(180,20));
    assert_eq!(app.forms.controls[&native_node].kind,Kind::Button);
    assert_eq!(app.forms.controls[&native_node].caption,"X");
    assert_eq!(app.page.control_at(native.x+20,native.y+10),Some(native_node));
    let canvas=app.draw();
    for(dx,dy)in[(5,5),(90,12),(50,30)] {assert_eq!(pixel(&app,&canvas,literal.x+dx,literal.y+dy),0x0d6efd);}
    for(dx,dy)in[(-1,12),(100,12),(50,-1),(50,36)] {assert_eq!(pixel(&app,&canvas,literal.x+dx,literal.y+dy),0xffffff);}
    assert_eq!(pixel(&app,&canvas,native.x+5,native.y+5),0x0d6efd,"Existing native interior is genuinely painted");
    assert_eq!(neighbor(&app),preserved);assert!(matches!(app.focus,Focus::Control(node) if node==neighbor_node));
    assert_eq!(app.forms.controls.keys().copied().collect::<Vec<_>>(),controls);
    assert_eq!(app.raw,raw);assert_eq!(app.location,location);assert_eq!((app.history_index,app.history.len()),history);
    clean(&app);

    // First new consumer assertion: 80x24 content + literal 6px/10px padding.
    assert_eq!((native.width,native.height),(100,36),"The real native control must consume the same supported literal padding as the ordinary DIV");
    let border=app.page.border_rects[native_node].unwrap();
    assert_eq!((border.x,border.y,border.width,border.height),(native.x,native.y,100,36));
    for(dx,dy)in[(90,12),(50,30)] {
        assert_eq!(pixel(&app,&canvas,native.x+dx,native.y+dy),0x0d6efd);
        assert_eq!(app.page.control_at(native.x+dx,native.y+dy),Some(native_node));
    }
    for(dx,dy)in[(-1,12),(100,12),(50,-1),(50,36)] {
        assert_eq!(pixel(&app,&canvas,native.x+dx,native.y+dy),0xffffff);
        assert_eq!(app.page.control_at(native.x+dx,native.y+dy),None);
    }
    // Activate inside padding added by CSS, through actual native pointer input.
    let (x,y)=(native.x+90,app.content_top()+native.y+12-app.scroll);
    assert!(matches!(app.pointer_down(x,y,false),PlatformAction::None));
    assert!(matches!(app.pointer_up(x,y),PlatformAction::None));
    clean(&app);assert_eq!(app.document.title,"Native padding clicked");
    assert_eq!(neighbor(&app),preserved);assert_eq!(app.forms.text_default_value(neighbor_node).unwrap(),"neighbor seed");
    assert_eq!(app.forms.controls.keys().copied().collect::<Vec<_>>(),controls);
    assert_eq!(app.raw,raw);assert_eq!(app.location,location);assert_eq!((app.history_index,app.history.len()),history);
}
