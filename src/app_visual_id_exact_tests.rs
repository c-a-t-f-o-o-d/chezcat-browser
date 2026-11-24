//! Native App ID record fixtures exercise real pointer and navigation consumers.
use super::*;
use crate::html::NodeKind;
struct Fixture(std::path::PathBuf);
impl Fixture{fn new(name:&str,markup:&str)->Self{let path=std::env::temp_dir().join(format!("chezcat-exact-native-id-{}-{name}.html",std::process::id()));std::fs::write(&path,markup).unwrap();Self(path)}}
impl Drop for Fixture{fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
fn node(app:&App,id:&str)->usize{(0..app.document.nodes.len()).find(|&n|app.document.attr(n,"id")==Some(id)).unwrap()}
fn record(app:&mut App,n:usize,name:&str,units:&[u16]){let NodeKind::Element{attrs,..}=&mut app.document.nodes[n].kind else{unreachable!()};attrs.iter_mut().find(|a|a.name.html_is(name)).unwrap().value=crate::dom_string::DomString::from_units(units,4096).unwrap();}
#[test]
fn visual_id_exact_actual_app_label_pointer_targets_literal_then_raw_record(){
    let file=Fixture::new("label",r#"<!doctype html><style>body{margin:0;background:white}label{position:absolute;left:20px;top:20px;width:120px;height:24px}.raw{position:absolute;left:20px;top:100px}.literal{position:absolute;left:20px;top:160px}#neighbor{position:absolute;left:220px;top:20px;width:180px;height:30px}#stable{position:absolute;left:220px;top:100px}</style><body><label id=label for='A�z'>Choice</label><input class=raw id='A�z' type=checkbox><input class=literal id=second type=checkbox><input id=neighbor value=seed><a id=stable href=#destination>Go</a><div id=destination></div></body>"#);
    let mut app=App::new();app.resize(640,480);app.open(&file.0);assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    let first=node(&app,"A�z");let second=node(&app,"second");let label=node(&app,"label");let neighbor=node(&app,"neighbor");let stable=node(&app,"stable");
    app.forms.script_set_value(neighbor,"edited neighbor").unwrap();{let e=&mut app.forms.controls.get_mut(&neighbor).unwrap().value;e.cursor=5;e.anchor=2;e.offset=1;}
    let raw=app.raw.clone();let location=app.location.clone();let epoch=app.script_epoch;record(&mut app,first,"id",&[65,0xd800,122]);record(&mut app,second,"id",&"A�z".encode_utf16().collect::<Vec<_>>());app.reflow();
    let before=app.draw().pixels;let y=app.content_top()+30;app.pointer_down(30,y,false);app.pointer_up(30,y);assert!(!app.forms.controls[&first].checked);assert!(app.forms.controls[&second].checked);assert_ne!(before,app.draw().pixels);
    record(&mut app,label,"for",&[65,0xd800,122]);app.reflow();let before=app.draw().pixels;app.pointer_down(30,y,false);app.pointer_up(30,y);assert!(app.forms.controls[&first].checked&&app.forms.controls[&second].checked);assert_ne!(before,app.draw().pixels);
    let e=&app.forms.controls[&neighbor].value;assert_eq!((e.text(),e.cursor,e.anchor,e.offset),("edited neighbor".into(),5,2,1));assert_eq!(app.page.link_node_at(230,110),Some(stable));assert_eq!(app.raw,raw);assert_eq!(app.location,location);assert_eq!(app.script_epoch,epoch);
}
#[test]
fn visual_id_exact_actual_app_fragment_link_skips_raw_fixed_alias_and_scrolls_to_literal(){
    let file=Fixture::new("fragment",r#"<!doctype html><style>body{margin:0;background:white}a{position:absolute;left:20px;top:20px;width:160px;height:24px}.raw{position:fixed;left:220px;top:100px}.literal{margin-top:800px;height:100px;background:#654321}.extent{height:1200px}</style><body><a id=go href='#A�z'>Go</a><div class=raw id='A�z'>raw</div><div class=literal id=second>literal</div><div class=extent></div></body>"#);
    let mut app=App::new();app.resize(640,480);app.open(&file.0);assert!(!app.failed&&!app.is_loading(),"{}",app.status);let first=node(&app,"A�z");let second=node(&app,"second");let go=node(&app,"go");record(&mut app,first,"id",&[65,0xd800,122]);record(&mut app,second,"id",&"A�z".encode_utf16().collect::<Vec<_>>());app.reflow();let target_y=app.page.border_rects[second].unwrap().y;let epoch=app.script_epoch;let raw=app.raw.clone();
    assert_eq!(app.page.link_node_at(40,30),Some(go));let y=app.content_top()+30;let (_,hit)=app.primary_anchor_down(40,y,false);let (_,default)=app.release_anchor_pointer(hit,40,y,false,crate::scripting::AnchorMouseInput::default());let Some(crate::app::anchor::AnchorDefault::Link(target,_))=default else{panic!("real link")};app.open_link_target(target);
    assert_eq!(app.location.fragment,"A�z");assert_eq!(app.scroll,(target_y-12).clamp(0,app.max_scroll()));assert!(app.scroll>500);assert_eq!(app.raw,raw);assert_eq!(app.script_epoch,epoch);let image=app.draw();assert!(image.pixels.contains(&0x654321));
}
