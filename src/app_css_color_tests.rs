//! Original real-App opaque foreground reduction; no framework or desktop UI.
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
fn crop(app:&App,canvas:&Canvas,left:i32,top:i32)->Vec<u32> {
    (0..36).flat_map(|y|(0..180).map(move|x|pixel(app,canvas,left+x,top+y))).collect()
}

#[test]
fn color_variable_actual_app_white_caption_and_link_twins_match_literal_pixels_and_hits() {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    struct Fixture(PathBuf);impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let fixture=Fixture(std::env::temp_dir().join(format!("chezcat-color-twins-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Foreground pending</title><style>
html,body{margin:0;padding:0;background:#ffffff;--Foreground:#ffffff}
.twin{display:block;position:absolute;width:180px;height:36px;background:#0d6efd;font-size:18px;line-height:24px;text-align:center}
#literal-button,#variable-button{top:20px}#literal-text,#variable-text{top:80px}
#literal-button,#literal-text{left:20px;color:#ffffff}
#variable-button,#variable-text{left:240px;color:var(--Foreground)}
</style></head><body><button id=literal-button name=literal type=button class=twin>Original caption</button><button id=variable-button name=variable type=button class=twin>Original caption</button><a id=literal-text href=#destination class=twin>Original caption</a><a id=variable-text href=#destination class=twin>Original caption</a><div id=destination></div><script>document.title='Foreground ready';</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(640,300);app.set_javascript_enabled(true);app.open(&fixture.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);assert_eq!(app.document.title,"Foreground ready");
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
    let canvas=app.draw();
    for(literal,variable,top,control)in[("literal-button","variable-button",20,true),("literal-text","variable-text",80,false)] {
        for(name,left)in[(literal,20),(variable,240)] {
            let node=id(&app,name);let border=app.page.border_rects[node].unwrap();
            assert_eq!((border.x,border.y,border.width,border.height),(left,top,180,36),"{name}");
            assert_eq!(pixel(&app,&canvas,left+2,top+2),0x0d6efd,"Literal blue backgrounds already paint for {name}");
            if control {
                let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();
                assert_eq!((rect.x,rect.y,rect.width,rect.height),(left,top,180,36));
                for(dx,dy)in[(0,0),(90,18),(179,35)] {assert_eq!(app.page.control_at(left+dx,top+dy),Some(node));}
                assert_eq!(app.page.control_at(left-1,top+18),None);assert_eq!(app.page.control_at(left+180,top+18),None);
            } else {
                for(dx,dy)in[(0,0),(90,18),(179,35)] {assert_eq!(app.page.link_node_at(left+dx,top+dy),Some(node));assert_eq!(app.page.link_at(left+dx,top+dy),Some("#destination"));}
                assert_eq!(app.page.link_node_at(left-1,top+18),None);assert_eq!(app.page.link_node_at(left+180,top+18),None);
            }
        }
        let literal_pixels=crop(&app,&canvas,20,top);
        assert!(literal_pixels.iter().any(|&color|color==0xffffff),"Independent literal {literal} must contain real opaque white glyph pixels");
        assert!(literal_pixels.iter().filter(|&&color|color==0x0d6efd).count()>3000,"Independent literal {literal} must retain its blue background");
    }
    for(name,top)in[("native caption",20),("ordinary link text",80)] {
        let literal=crop(&app,&canvas,20,top);let variable=crop(&app,&canvas,240,top);
        assert!(variable.iter().any(|&color|color==0xffffff),"Variable {name} must paint real white glyph pixels on the blue background");
        assert_eq!(variable,literal,"Whole original {name} crop must match the independent literal twin");
    }
    assert_eq!(app.forms.controls.len(),2);assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
}

fn click(app:&mut App,name:&str) {
    let node=id(app,name);let control=app.page.controls.iter().find(|control|control.node==node).unwrap();
    let(x,page_y)=(control.x+control.width/2,control.y+control.height/2);assert_eq!(app.page.control_at(x,page_y),Some(node));
    let y=app.content_top()+page_y-app.scroll;
    assert!(matches!(app.pointer_down(x,y,false),PlatformAction::None));assert!(matches!(app.pointer_up(x,y),PlatformAction::None));
}
fn neighbor(app:&App)->(String,usize,usize,usize,bool,Option<usize>) {
    let control=&app.forms.controls[&id(app,"neighbor")];
    (control.value.text(),control.value.cursor,control.value.anchor,control.value.offset,control.dirty_value,control.owner)
}

#[test]
fn color_variable_actual_app_trusted_class_change_repaints_text_and_preserves_neighbor_editor() {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    struct Fixture(PathBuf);impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let fixture=Fixture(std::env::temp_dir().join(format!("chezcat-color-live-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Live foreground ready</title><style>
html,body{margin:0;padding:0;background:#ffffff}
#theme{--Foreground:#ffffff}#theme.changed{--Foreground:#ffd28a}
.twin{display:block;position:absolute;width:180px;height:36px;background:#0d6efd;font-size:18px;line-height:24px;text-align:center}
#literal-button,#variable-button{top:20px}#literal-text,#variable-text{top:80px}
#literal-button,#literal-text{left:20px;color:#ffd28a}#variable-button,#variable-text{left:240px;color:var(--Foreground)}
#change{position:absolute;left:20px;top:140px;width:180px;height:32px}#neighbor{position:absolute;left:240px;top:140px;width:180px;height:32px}
</style></head><body><button id=literal-button type=button class=twin>Original caption</button><a id=literal-text href=#destination class=twin>Original caption</a><div id=theme><button id=variable-button type=button class=twin>Original caption</button><a id=variable-text href=#destination class=twin>Original caption</a></div><button id=change type=button>Change inherited color</button><form id=g><input id=neighbor name=neighbor value="neighbor seed"></form><div id=destination></div><script>
document.getElementById('change').onclick=function(event){
 if(!event.isTrusted||this.ownerDocument!==document)throw 'trusted original activation required';
 document.getElementById('theme').className='changed';document.title='Live foreground changed';
};
</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(640,360);app.set_javascript_enabled(true);app.open(&fixture.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
    let canvas=app.draw();
    for top in [20,80] {assert!(crop(&app,&canvas,20,top).iter().any(|&color|color==0xffd28a));assert!(crop(&app,&canvas,240,top).iter().any(|&color|color==0xffffff));}
    let raw=app.raw.clone();let location=app.location.clone();let history=(app.history_index,app.history.len());let nodes=app.document.nodes.as_ptr();
    click(&mut app,"neighbor");assert!(matches!(app.key(0x24,false,false,false),PlatformAction::None));assert!(matches!(app.key(0x23,false,true,false),PlatformAction::None));app.paste("neighbor edited");
    assert!(matches!(app.key(0x24,false,false,false),PlatformAction::None));
    for _ in 0..2 {assert!(matches!(app.key(0x27,false,false,false),PlatformAction::None));}
    for _ in 0..3 {assert!(matches!(app.key(0x27,false,true,false),PlatformAction::None));}
    let preserved=neighbor(&app);assert_eq!(preserved.0,"neighbor edited");assert_eq!((preserved.1,preserved.2),(5,2));assert!(preserved.4);assert_eq!(preserved.5,Some(id(&app,"g")));
    click(&mut app,"change");assert_eq!(app.document.title,"Live foreground changed");assert_eq!(app.document.attr(id(&app,"theme"),"class"),Some("changed"));
    assert_eq!(neighbor(&app),preserved);assert_eq!(app.forms.text_default_value(id(&app,"neighbor")).unwrap(),"neighbor seed");
    let canvas=app.draw();
    for(literal,variable,top,control)in[("literal-button","variable-button",20,true),("literal-text","variable-text",80,false)] {
        for(name,left)in[(literal,20),(variable,240)] {
            let node=id(&app,name);let rect=app.page.border_rects[node].unwrap();assert_eq!((rect.x,rect.y,rect.width,rect.height),(left,top,180,36));
            assert_eq!(pixel(&app,&canvas,left+2,top+2),0x0d6efd);
            if control {
                for(dx,dy)in[(0,0),(90,18),(179,35)] {assert_eq!(app.page.control_at(left+dx,top+dy),Some(node));}
                assert_eq!(app.page.control_at(left-1,top+18),None);assert_eq!(app.page.control_at(left+180,top+18),None);
            } else {
                for(dx,dy)in[(0,0),(90,18),(179,35)] {assert_eq!(app.page.link_node_at(left+dx,top+dy),Some(node));assert_eq!(app.page.link_at(left+dx,top+dy),Some("#destination"));}
                assert_eq!(app.page.link_node_at(left-1,top+18),None);assert_eq!(app.page.link_node_at(left+180,top+18),None);
            }
        }
        let literal=crop(&app,&canvas,20,top);let variable=crop(&app,&canvas,240,top);
        assert!(variable.iter().any(|&color|color==0xffd28a));assert!(!variable.iter().any(|&color|color==0xffffff),"The old opaque white foreground must disappear");assert_eq!(variable,literal,"Live foreground must match its literal twin");
    }
    assert_eq!(app.document.nodes.as_ptr(),nodes);assert_eq!(app.raw,raw);assert_eq!(app.location,location);assert_eq!((app.history_index,app.history.len()),history);assert_eq!(app.forms.controls.len(),4);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());assert!(!app.failed&&!app.is_loading());assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
}
