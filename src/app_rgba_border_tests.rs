//! Original real-App RGBA border reduction; no framework, reference browser or UI.
use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};

fn id(app:&App,name:&str)->usize {(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap()}
fn rect(app:&App,name:&str)->crate::layout::BorderRect {app.page.border_rects[id(app,name)].expect("Original box must have real layout")}
fn pixel(app:&App,canvas:&Canvas,x:i32,y:i32)->u32 {
    let y=app.content_top()+y-app.scroll;
    assert!(x>=0&&y>=0&&(x as usize)<canvas.width&&(y as usize)<canvas.height);
    canvas.pixels[y as usize*canvas.width+x as usize]
}

#[test]
fn rgba_border_actual_app_literal_variable_and_transparent_geometry_match_original_pixels() {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    struct Fixture(PathBuf);impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let fixture=Fixture(std::env::temp_dir().join(format!("chezcat-rgba-border-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>RGBA pending</title><style>
html,body{margin:0;padding:0;background:#c86432;--Edge:rgba(0, 0, 0, 0.2)}
.box{display:block;position:absolute;top:20px;width:40px;height:20px;box-sizing:content-box;background:#64c832}
#opaque{left:20px;border:4px solid #50a028}
#literal{left:120px;border:4px solid rgba(0, 0, 0, 0.2)}
#variable{left:220px;border:4px solid var(--Edge)}
#zero{left:320px;border:4px solid rgba(255, 0, 255, 0)}
</style></head><body><div id=opaque class=box></div><div id=literal class=box></div><div id=variable class=box></div><a id=zero class=box href=#target></a><div id=target></div><script>document.title='RGBA ready';</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(640,300);app.set_javascript_enabled(true);app.open(&fixture.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());assert_eq!(app.document.title,"RGBA ready");
    let canvas=app.draw();let opaque=rect(&app,"opaque");let literal=rect(&app,"literal");
    assert_eq!((opaque.x,opaque.y,opaque.width,opaque.height),(20,20,48,28));
    assert_eq!(pixel(&app,&canvas,opaque.x+2,opaque.y+2),0x50a028,"Opaque reference must already paint its exact original color");
    // Black at alpha .2 over the box's own border-box background (100,200,50)
    // is (80,160,40). Parent backdrop (200,100,50) differs deliberately. The
    // corner also must be composited once, never once per overlapping side.
    assert_eq!(pixel(&app,&canvas,literal.x+2,literal.y+2),0x50a028,
        "Literal RGBA border must paint source-over its actual backdrop; observed literal geometry {:?}",
        (literal.x,literal.y,literal.width,literal.height));
    for name in ["literal","variable"] {
        let border=rect(&app,name);assert_eq!((border.width,border.height),(48,28),"{name}");
        for y in 0..28 {for x in 0..48 {
            assert_eq!(pixel(&app,&canvas,border.x+x,border.y+y),pixel(&app,&canvas,opaque.x+x,opaque.y+y),"{name} relative pixel ({x},{y})");
        }}
        assert_eq!(pixel(&app,&canvas,border.x-1,border.y+2),0xc86432,"The independent nonwhite parent backdrop stays visible outside {name}");
        assert_eq!(pixel(&app,&canvas,border.x+8,border.y+8),0x64c832,"Interior remains the unmodified opaque box background");
    }
    let zero=id(&app,"zero");let border=rect(&app,"zero");assert_eq!((border.x,border.y,border.width,border.height),(320,20,48,28));
    for(x,y)in[(border.x,border.y),(border.x+47,border.y+27),(border.x+24,border.y+14)] {
        assert_eq!(app.page.link_node_at(x,y),Some(zero));assert_eq!(app.page.link_at(x,y),Some("#target"));
        assert_eq!(pixel(&app,&canvas,x,y),0x64c832,"Alpha zero preserves border geometry/hits while leaving the underlying box pixels unchanged");
    }
    assert_eq!(app.page.link_node_at(border.x-1,border.y+8),None);assert_eq!(app.page.link_node_at(border.x+48,border.y+8),None);
    assert!(app.forms.controls.is_empty());assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
}

#[test]
fn rgba_border_flex_stretch_keeps_single_corner_composite() {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    struct Fixture(PathBuf);impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let fixture=Fixture(std::env::temp_dir().join(format!("chezcat-rgba-flex-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><style>
html,body{margin:0;padding:0;background:#c86432}
#row{display:flex;align-items:stretch;width:100px;height:60px}
#stretch{width:40px;background:#64c832;border:4px solid rgba(0,0,0,.2)}
</style></head><body><div id=row><a id=stretch href=#target></a></div><div id=target></div></body></html>"#).unwrap();
    let mut app=App::new();app.resize(300,200);app.open(&fixture.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    let border=rect(&app,"stretch");assert_eq!((border.width,border.height),(48,60));
    let canvas=app.draw();
    for (dx,dy) in [(1,1),(46,1),(1,58),(46,58),(24,1),(24,58)] {
        assert_eq!(pixel(&app,&canvas,border.x+dx,border.y+dy),0x50a028,"Stretched corner ({dx},{dy}) must blend once");
    }
    assert_eq!(pixel(&app,&canvas,border.x+8,border.y+8),0x64c832);
    assert_eq!(app.page.link_node_at(border.x+1,border.y+58),Some(id(&app,"stretch")));
}
