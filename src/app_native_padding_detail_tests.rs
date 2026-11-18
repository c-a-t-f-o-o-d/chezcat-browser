//! Native padding preserves caption content coordinates and real button modes.
use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new(source:&str)->Self {
        static NEXT:AtomicUsize=AtomicUsize::new(0);
        let fixture=Self(std::env::temp_dir().join(format!("chezcat-native-padding-detail-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))));
        std::fs::write(&fixture.0,source).unwrap();fixture
    }
    fn open(&self)->App {
        let mut app=App::new();app.resize(640,360);app.set_javascript_enabled(true);app.open(&self.0);clean(&app);app
    }
}
impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
fn id(app:&App,name:&str)->usize {(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap()}
fn rect(app:&App,name:&str)->layout::ControlBox {app.page.controls.iter().find(|rect|rect.node==id(app,name)).unwrap().clone()}
fn clean(app:&App) {
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    let page=app.scripting.as_ref().unwrap();assert_eq!(page.error_count(),0,"{}",page.report());assert!(!page.has_navigation_request());
}
fn crop(app:&App,canvas:&Canvas,x:i32,y:i32,width:i32,height:i32)->Vec<u32> {
    let y=app.content_top()+y-app.scroll;
    assert!(x>=0&&y>=0&&x+width<=canvas.width as i32&&y+height<=canvas.height as i32);
    (0..height).flat_map(|dy|canvas.pixels[(y+dy)as usize*canvas.width+x as usize..(y+dy)as usize*canvas.width+(x+width)as usize].iter().copied()).collect()
}
fn click(app:&mut App,name:&str,dx:i32,dy:i32) {
    let rect=rect(app,name);let (x,y)=(rect.x+dx,app.content_top()+rect.y+dy-app.scroll);
    assert_eq!(app.page.control_at(x,rect.y+dy),Some(rect.node));
    assert!(matches!(app.pointer_down(x,y,false),PlatformAction::None));
    assert!(matches!(app.pointer_up(x,y),PlatformAction::None));clean(app);
}

#[test]
fn native_padding_actual_app_asymmetric_caption_keeps_the_original_content_crop() {
    let fixture=Fixture::new(r#"<!doctype html><title>Caption padding</title><style>
html,body{margin:0;padding:0;background:#ffffff}
button{position:absolute;display:block;top:20px;width:80px;height:24px;box-sizing:content-box;margin:0;border:0;background:#0d6efd;color:#ffffff}
#plain{left:20px;padding:0}#padded{left:180px;padding:3px 17px 9px 5px}
</style><button id=plain type=button>X</button><button id=padded type=button>X</button><script>document.title='Caption ready';</script>"#);
    let app=fixture.open();assert_eq!(app.document.title,"Caption ready");
    let plain=rect(&app,"plain");let padded=rect(&app,"padded");
    assert_eq!((plain.width,plain.height,plain.padding),(80,24,[0;4]));
    assert_eq!((padded.width,padded.height,padded.padding),(102,36,[3,17,9,5]));
    let canvas=app.draw();
    // Exclude only the unchanged one-pixel native theme and its focus gutter.
    let original=crop(&app,&canvas,plain.x+2,plain.y+2,76,20);
    assert!(original.iter().any(|&color|color==0xffffff),"The original caption must contain real glyph pixels");
    assert!(original.iter().any(|&color|color==0x0d6efd));
    assert_eq!(crop(&app,&canvas,padded.x+5+2,padded.y+3+2,76,20),original,
        "Asymmetric authored padding moves the caption content, not its internal geometry");
    for(dx,dy)in[(2,10),(90,12),(40,30)] {
        assert_eq!(crop(&app,&canvas,padded.x+dx,padded.y+dy,1,1),vec![0x0d6efd]);
        assert_eq!(app.page.control_at(padded.x+dx,padded.y+dy),Some(padded.node));
    }
    assert_eq!(app.page.control_at(padded.x+102,padded.y+12),None);
    assert_eq!(app.page.control_at(padded.x+40,padded.y+36),None);
}

#[test]
fn native_padding_actual_app_submit_reset_and_disabled_modes_keep_values_and_native_actions() {
    let fixture=Fixture::new(r#"<!doctype html><title>Native modes pending</title><style>
html,body{margin:0;padding:0;background:#ffffff}
input{position:absolute;left:20px;top:90px;width:220px;height:32px}
button{position:absolute;top:20px;width:80px;height:24px;box-sizing:content-box;padding:6px 10px;margin:0;border:0;background:#0d6efd}
#submit{left:20px}#reset{left:180px}#disabled{left:340px}
</style><form id=f action='/unrequested'><input id=field name=q value='field seed'><button id=submit type=submit name=send value=payload>Send</button><button id=reset type=reset value=restore>Reset</button><button id=disabled type=button disabled>Off</button></form><script>
document.getElementById('f').onsubmit=function(event){if(!event.isTrusted||document.getElementById('submit').value!=='payload')throw 'real padded submission';event.preventDefault();document.title='Padded submit kept';};
document.getElementById('disabled').onclick=function(){throw 'disabled padded button activated';};
document.title='Native modes ready';
</script>"#);
    let mut app=fixture.open();assert_eq!(app.document.title,"Native modes ready");
    let raw=app.raw.clone();let location=app.location.clone();let history=(app.history_index,app.history.len());
    let form=id(&app,"f");let field=id(&app,"field");
    for(name,kind)in[("submit",Kind::Submit),("reset",Kind::Reset),("disabled",Kind::Button)] {
        let node=id(&app,name);let control=&app.forms.controls[&node];
        assert_eq!(control.kind,kind);assert_eq!(control.owner,Some(form));
        let rect=rect(&app,name);assert_eq!((rect.width,rect.height),(100,36));
    }
    assert_eq!(app.forms.controls[&id(&app,"submit")].value.text(),"payload");
    assert_eq!(app.forms.controls[&id(&app,"reset")].value.text(),"restore");
    assert!(app.forms.controls[&id(&app,"disabled")].disabled);
    click(&mut app,"field",20,12);
    assert!(matches!(app.key(0x24,false,false,false),PlatformAction::None));
    assert!(matches!(app.key(0x23,false,true,false),PlatformAction::None));app.paste("field edited");clean(&app);
    assert_eq!(app.forms.controls[&field].value.text(),"field edited");
    assert_eq!(app.forms.text_default_value(field).unwrap(),"field seed");
    click(&mut app,"disabled",90,12);assert_eq!(app.document.title,"Native modes ready");
    assert_eq!(app.forms.controls[&field].value.text(),"field edited");
    click(&mut app,"submit",90,12);assert_eq!(app.document.title,"Padded submit kept");
    assert_eq!(app.forms.controls[&field].value.text(),"field edited");
    click(&mut app,"reset",90,12);assert_eq!(app.forms.controls[&field].value.text(),"field seed");
    assert_eq!(app.forms.text_default_value(field).unwrap(),"field seed");
    assert_eq!(app.forms.controls[&id(&app,"submit")].kind,Kind::Submit);
    assert_eq!(app.forms.controls[&id(&app,"submit")].value.text(),"payload");
    assert_eq!(app.raw,raw);assert_eq!(app.location,location);assert_eq!((app.history_index,app.history.len()),history);
}
