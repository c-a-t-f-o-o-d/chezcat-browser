//! Original App font-size twins use the existing authored Geometry test face.
//! No system/downloaded font, reference browser or external resource is needed.
use super::*;
use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};

fn id(app:&App,name:&str)->usize {(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap()}
fn pixel(app:&App,canvas:&Canvas,x:i32,y:i32)->u32 {
    let y=app.content_top()+y-app.scroll;assert!(x>=0&&y>=0&&(x as usize)<canvas.width&&(y as usize)<canvas.height);
    canvas.pixels[y as usize*canvas.width+x as usize]
}
fn crop(app:&App,canvas:&Canvas,left:i32,top:i32,width:i32,height:i32)->Vec<u32> {
    (0..height).flat_map(|y|(0..width).map(move|x|(x,y))).map(|(x,y)|pixel(app,canvas,left+x,top+y)).collect()
}
fn open(path:&Path)->App {
    let mut app=App::new();
    app.base_fonts.add("Geometry".into(),400,false,
        Arc::new(crate::font::Font::parse(&crate::font::tests::fixture_font()).unwrap())).unwrap();
    app.fonts=app.base_fonts.clone();app.resize(840,400);app.set_javascript_enabled(true);app.open(path);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());app
}

#[test]
fn font_size_variable_actual_app_native_and_link_twins_match_supported_literal_metrics_and_pixels() {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    struct Fixture(PathBuf);impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let fixture=Fixture(std::env::temp_dir().join(format!("chezcat-font-size-twins-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Font pending</title><style>
html{font-size:16px}body{margin:0;padding:0;background:#ffffff;font-family:Geometry;font-size:12px;--Size:24px}
.native,.text{display:block;position:absolute;margin:0;background:#0d6efd;color:#ffffff;font-weight:400}
.native{top:20px}.text{top:120px;width:180px;line-height:40px}
#small-button,#small-text{left:20px;font-size:12px}
#literal-button,#literal-text{left:280px;font-size:24px}
#variable-button,#variable-text{left:540px;font-size:var(--Size)}
</style></head><body><button id=small-button type=button class=native>AV</button><button id=literal-button type=button class=native>AV</button><button id=variable-button type=button class=native>AV</button><a id=small-text class=text href=#destination>AV<br>AV</a><a id=literal-text class=text href=#destination>AV<br>AV</a><a id=variable-text class=text href=#destination>AV<br>AV</a><div id=destination></div><script>document.title='Font ready';</script></body></html>"#).unwrap();
    let app=open(&fixture.0);assert_eq!(app.document.title,"Font ready");let canvas=app.draw();
    let small=app.page.controls.iter().find(|rect|rect.node==id(&app,"small-button")).unwrap();
    let literal=app.page.controls.iter().find(|rect|rect.node==id(&app,"literal-button")).unwrap();
    let variable=app.page.controls.iter().find(|rect|rect.node==id(&app,"variable-button")).unwrap();
    assert_eq!(small.text_font.as_ref().unwrap().size,12.0);assert_eq!(literal.text_font.as_ref().unwrap().size,24.0);
    assert!(literal.width>small.width&&literal.height>small.height,"Supported literal font-size must already change real native auto sizing");
    for(name,left,rect)in[("small-button",20,small),("literal-button",280,literal),("variable-button",540,variable)] {
        let node=id(&app,name);assert_eq!((rect.x,rect.y),(left,20));assert_eq!(app.forms.controls[&node].caption,"AV");
        let border=app.page.border_rects[node].unwrap();assert_eq!((border.x,border.y,border.width,border.height),(rect.x,rect.y,rect.width,rect.height));
        for(dx,dy)in[(0,0),(rect.width/2,rect.height/2),(rect.width-1,rect.height-1)] {assert_eq!(app.page.control_at(left+dx,20+dy),Some(node));}
        assert_eq!(app.page.control_at(left-1,20+rect.height/2),None);assert_eq!(app.page.control_at(left+rect.width,20+rect.height/2),None);
    }
    let small_pixels=crop(&app,&canvas,small.x,small.y,small.width,small.height);
    let literal_pixels=crop(&app,&canvas,literal.x,literal.y,literal.width,literal.height);
    for pixels in [&small_pixels,&literal_pixels] {assert!(pixels.iter().any(|&color|color==0xffffff));assert!(pixels.iter().any(|&color|color==0x0d6efd));}
    assert!(literal_pixels.iter().filter(|&&color|color==0xffffff).count()>small_pixels.iter().filter(|&&color|color==0xffffff).count(),"Larger authored glyphs must paint more actual opaque white pixels");
    for(name,left)in[("small-text",20),("literal-text",280),("variable-text",540)] {
        let node=id(&app,name);let border=app.page.border_rects[node].unwrap();assert_eq!((border.x,border.y,border.width,border.height),(left,120,180,80));
        for(dx,dy)in[(0,0),(90,40),(179,79)] {assert_eq!(app.page.link_node_at(left+dx,120+dy),Some(node));assert_eq!(app.page.link_at(left+dx,120+dy),Some("#destination"));}
        assert_eq!(app.page.link_node_at(left-1,160),None);assert_eq!(app.page.link_node_at(left+180,160),None);
    }
    let small_text=crop(&app,&canvas,20,120,180,80);let literal_text=crop(&app,&canvas,280,120,180,80);
    for pixels in [&small_text,&literal_text] {assert!(pixels.iter().any(|&color|color==0xffffff));assert!(pixels.iter().any(|&color|color==0x0d6efd));}
    assert!(literal_text.iter().filter(|&&color|color==0xffffff).count()>small_text.iter().filter(|&&color|color==0xffffff).count(),"Literal font-size must visibly scale ordinary linked text too");
    // Every literal sizing/paint/hit positive above runs before var equivalence.
    assert_eq!((variable.width,variable.height),(literal.width,literal.height),"Variable font-size must reach the actual native caption's auto metrics");
    assert_eq!(variable.text_font.as_ref().unwrap().size,24.0);
    assert_eq!(crop(&app,&canvas,variable.x,variable.y,variable.width,variable.height),literal_pixels,"Native caption twins must match their entire relative pixels");
    assert_eq!(crop(&app,&canvas,540,120,180,80),literal_text,"Ordinary linked text must match its entire literal twin crop");
    assert_eq!(app.forms.controls.len(),3);assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
}

fn click(app:&mut App,name:&str) {
    let node=id(app,name);let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();let(x,page_y)=(rect.x+rect.width/2,rect.y+rect.height/2);assert_eq!(app.page.control_at(x,page_y),Some(node));
    let y=app.content_top()+page_y-app.scroll;assert!(matches!(app.pointer_down(x,y,false),PlatformAction::None));assert!(matches!(app.pointer_up(x,y),PlatformAction::None));
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
}
fn key(app:&mut App,code:u32,shift:bool) {assert!(matches!(app.key(code,false,shift,false),PlatformAction::None));}
fn neighbor(app:&App)->(String,usize,usize,usize,bool,Option<usize>) {
    let control=&app.forms.controls[&id(app,"neighbor")];(control.value.text(),control.value.cursor,control.value.anchor,control.value.offset,control.dirty_value,control.owner)
}

#[test]
fn font_size_variable_actual_app_trusted_class_reflows_paints_hits_and_keeps_neighbor_editor() {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    struct Fixture(PathBuf);impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let fixture=Fixture(std::env::temp_dir().join(format!("chezcat-font-size-live-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Live font ready</title><style>
html{font-size:16px}body{margin:0;padding:0;background:#ffffff;font-family:Geometry;font-size:12px}
#theme{--Size:24px}#theme.changed{--Size:32px}
.native,.text{display:block;position:absolute;margin:0;background:#0d6efd;color:#ffffff;font-weight:400}
.native{top:20px}.text{top:120px;width:180px;line-height:40px}
#literal-button,#literal-text{left:20px;font-size:32px}#variable-button,#variable-text{left:280px;font-size:var(--Size)}
#neighbor{position:absolute;left:540px;top:20px;width:220px;height:32px;margin:0}#change{position:absolute;left:540px;top:80px;width:220px;height:32px}
</style></head><body><button id=literal-button type=button class=native>AV</button><a id=literal-text class=text href=#destination>AV<br>AV</a><div id=theme><button id=variable-button type=button class=native>AV</button><a id=variable-text class=text href=#destination>AV<br>AV</a></div><form id=f><input id=neighbor name=keep value='neighbor seed'></form><button id=change type=button>Grow original font</button><div id=destination></div><script>
var originalButton=document.getElementById('variable-button'),originalText=document.getElementById('variable-text');
document.getElementById('change').onclick=function(event){
 if(!event.isTrusted||this.ownerDocument!==document)throw 'trusted original font change';
 document.getElementById('theme').className='changed';
 if(document.getElementById('variable-button')!==originalButton||document.getElementById('variable-text')!==originalText||originalButton.ownerDocument!==document||originalText.ownerDocument!==document)throw 'font change replaced original nodes';
 document.title='Live font changed';
};
</script></body></html>"#).unwrap();
    let mut app=open(&fixture.0);assert_eq!(app.document.title,"Live font ready");
    let variable=id(&app,"variable-button");let literal=id(&app,"literal-button");let before=app.page.controls.iter().find(|rect|rect.node==variable).unwrap();let before_rect=(before.x,before.y,before.width,before.height);
    assert_eq!((before.x,before.y),(280,20));assert_eq!(before.text_font.as_ref().unwrap().size,24.0);assert_eq!(app.page.control_at(before.x+before.width,before.y+before.height/2),None);
    let reference=app.page.controls.iter().find(|rect|rect.node==literal).unwrap();assert_eq!(reference.text_font.as_ref().unwrap().size,32.0);assert!(reference.width>before.width&&reference.height>before.height);
    let canvas=app.draw();let old_text=crop(&app,&canvas,280,120,180,80);let reference_text=crop(&app,&canvas,20,120,180,80);
    assert!(old_text.iter().any(|&color|color==0xffffff));assert!(reference_text.iter().filter(|&&color|color==0xffffff).count()>old_text.iter().filter(|&&color|color==0xffffff).count());
    let raw=app.raw.clone();let location=app.location.clone();let history=(app.history_index,app.history.len());let controls:Vec<_>=app.forms.controls.keys().copied().collect();
    click(&mut app,"neighbor");key(&mut app,0x24,false);key(&mut app,0x23,true);app.paste("neighbor edited");key(&mut app,0x24,false);for _ in 0..2 {key(&mut app,0x27,false);}for _ in 0..3 {key(&mut app,0x27,true);}
    let preserved=neighbor(&app);assert_eq!(preserved.0,"neighbor edited");assert_eq!((preserved.1,preserved.2),(5,2));assert!(preserved.4);assert_eq!(preserved.5,Some(id(&app,"f")));assert_eq!(app.forms.text_default_value(id(&app,"neighbor")).unwrap(),"neighbor seed");
    click(&mut app,"change");assert_eq!(app.document.title,"Live font changed");assert_eq!(app.document.attr(id(&app,"theme"),"class"),Some("changed"));
    let reference=app.page.controls.iter().find(|rect|rect.node==literal).unwrap();let resized=app.page.controls.iter().find(|rect|rect.node==variable).unwrap();
    assert_eq!((resized.x,resized.y),(280,20));assert_eq!(resized.text_font.as_ref().unwrap().size,32.0);assert_eq!((resized.width,resized.height),(reference.width,reference.height));assert!(resized.width>before_rect.2&&resized.height>before_rect.3);
    assert_eq!(app.page.control_at(before_rect.0+before_rect.2,before_rect.1+before_rect.3/2),Some(variable),"The expanded native hit region must replace the old smaller region");
    for(dx,dy)in[(0,0),(resized.width/2,resized.height/2),(resized.width-1,resized.height-1)] {assert_eq!(app.page.control_at(resized.x+dx,resized.y+dy),Some(variable));}
    assert_eq!(app.page.control_at(resized.x-1,resized.y+resized.height/2),None);assert_eq!(app.page.control_at(resized.x+resized.width,resized.y+resized.height/2),None);
    let canvas=app.draw();assert_eq!(crop(&app,&canvas,resized.x,resized.y,resized.width,resized.height),crop(&app,&canvas,reference.x,reference.y,reference.width,reference.height));
    let changed_text=crop(&app,&canvas,280,120,180,80);assert_eq!(changed_text,reference_text);assert_ne!(changed_text,old_text,"Live font-size must visibly repaint ordinary linked glyphs");
    for(name,left)in[("literal-text",20),("variable-text",280)] {
        let node=id(&app,name);let rect=app.page.border_rects[node].unwrap();assert_eq!((rect.x,rect.y,rect.width,rect.height),(left,120,180,80));
        for(dx,dy)in[(0,0),(90,40),(179,79)] {assert_eq!(app.page.link_node_at(left+dx,120+dy),Some(node));assert_eq!(app.page.link_at(left+dx,120+dy),Some("#destination"));}
        assert_eq!(app.page.link_node_at(left-1,160),None);assert_eq!(app.page.link_node_at(left+180,160),None);
    }
    assert_eq!(neighbor(&app),preserved);assert_eq!(app.forms.text_default_value(id(&app,"neighbor")).unwrap(),"neighbor seed");assert_eq!(app.forms.controls.keys().copied().collect::<Vec<_>>(),controls);
    assert_eq!(app.raw,raw);assert_eq!(app.location,location);assert_eq!((app.history_index,app.history.len()),history);assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
}

#[test]
fn computed_font_size_actual_app_matches_native_glyph_twins_and_preserves_editor_and_hits() {
    struct Fixture(PathBuf);impl Drop for Fixture{fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let fixture=Fixture(std::env::temp_dir().join(format!("chezcat-computed-font-app-{}.html",std::process::id())));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Computed font ready</title><style>
html{font-size:16px}body{margin:0;background:#ffffff;font-family:Geometry;font-size:12px;--Size:12px}
a{display:block;position:absolute;top:100px;width:180px;height:60px;line-height:40px;color:#ffffff;background:#0d6efd}
#actual{left:20px;font-size:var(--Size)}#literal{left:280px;font-size:24px}
#change{position:absolute;left:20px;top:20px;width:180px;height:32px}#neighbor{position:absolute;left:280px;top:20px;width:180px;height:32px}
</style></head><body><button id=change type=button>Read computed font</button><input id=neighbor value=seed><a id=actual href=#destination>AV</a><a id=literal href=#destination>AV</a><div id=destination></div><script>
var actual=document.getElementById('actual'),readback=getComputedStyle(actual),literal=document.getElementById('literal');
if(readback.fontSize!=='12px'||getComputedStyle(literal).fontSize!=='24px')throw 'actual initial native metrics';
document.getElementById('change').onclick=function(e){if(!e.isTrusted)throw 'native click';actual.style.setProperty('--Size','2em');if(readback.fontSize!=='24px')throw 'actual changed cascade';document.title='Computed font '+readback.fontSize;};
</script></body></html>"#).unwrap();
    let mut app=open(&fixture.0);let before=app.draw();assert_ne!(crop(&app,&before,20,100,180,60),crop(&app,&before,280,100,180,60));
    let neighbor=id(&app,"neighbor");let control=app.page.controls.iter().find(|c|c.node==neighbor).unwrap();let(x,y)=(control.x+12,app.content_top()+control.y+12-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);app.paste("real editor");
    let value=&app.forms.controls[&neighbor].value;let editor=(value.text(),value.cursor,value.anchor,value.offset);
    let button=id(&app,"change");let control=app.page.controls.iter().find(|c|c.node==button).unwrap();let(x,y)=(control.x+12,app.content_top()+control.y+12-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);
    assert_eq!(app.document.title,"Computed font 24px");assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
    let after=app.draw();let actual=crop(&app,&after,20,100,180,60);assert!(actual.contains(&0xffffff));assert!(actual.contains(&0x0d6efd));assert_eq!(actual,crop(&app,&after,280,100,180,60));
    let value=&app.forms.controls[&neighbor].value;assert_eq!((value.text(),value.cursor,value.anchor,value.offset),editor);
    assert_eq!(app.page.link_node_at(40,120),Some(id(&app,"actual")));assert_eq!(app.page.link_at(40,120),Some("#destination"));
}
