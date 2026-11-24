//! Canonical Element.id reflection through native App input and paint.
use super::*;
use crate::html::NodeKind;
use std::sync::atomic::{AtomicUsize,Ordering};

const MARKUP:&str=r#"<!doctype html><html><head><title>Code units ready</title>
<style>html,body{margin:0;background:white}#go{position:absolute;left:20px;top:20px;width:160px;height:32px}#neighbor{position:absolute;left:220px;top:20px;width:200px;height:32px}#out{position:absolute;left:20px;top:100px;width:300px;height:40px;margin:0;color:#123456;font-size:24px}#stable{position:absolute;left:20px;top:180px;width:100px;height:24px}#out.ready{color:#654321}</style>
</head><body><button id=go>Search key</button><input id=neighbor value=seed><p id=out>Waiting</p><a id=stable href=#destination>Go</a><div id=destination></div><form id=form style='position:absolute;left:20px;top:240px'><input id='A�z'><input id=literal name='A�z'></form><script>
var round=0;
document.getElementById('go').onclick=function(e){
if(!e.isTrusted)throw 'native input';
var f=document.getElementById('form'),literal=document.getElementById('literal'),out=document.getElementById('out');
var d=Object.getOwnPropertyDescriptor(Element.prototype,'id'),p=f.getElementsByTagName('input')[0];
if(round===0){if(p.id!=='A\ud800z'||d.get.call(p)!==p.id||document.getElementById(p.id)!==p)throw 'exact id read';literal.value='changed';out.className='';}
else{d.set.call(p,'changed');if(p.id!=='changed'||document.getElementById('changed')!==p)throw 'scalar canonical write';out.className='ready';}
out.textContent='Key '+round;document.title='Code units key '+round;round++;
};
</script></body></html>"#;

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new()->Self {
        static NEXT:AtomicUsize=AtomicUsize::new(0);
        let path=std::env::temp_dir().join(format!("chezcat-string-code-unit-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        std::fs::write(&path,MARKUP).unwrap();Self(path)
    }
}
impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
fn node(app:&App,name:&str)->usize {(0..app.document.nodes.len()).find(|&n|app.document.attr(n,"id")==Some(name)).unwrap()}

#[test]
fn element_id_accessor_actual_app_native_record_lookup_drives_paint_and_retains_interaction() {
    let fixture=Fixture::new();let mut app=App::new();app.resize(640,480);app.set_javascript_enabled(true);app.open(&fixture.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0);
    let first=node(&app,"A�z");
    let NodeKind::Element{attrs,..}=&mut app.document.nodes[first].kind else{unreachable!()};
    attrs.iter_mut().find(|a|a.name.html_is("id")).unwrap().value=crate::dom_string::DomString::from_units(&[65,0xd800,122],4096).unwrap();
    let neighbor=node(&app,"neighbor");app.forms.script_set_value(neighbor,"edited neighbor").unwrap();
    {let editor=&mut app.forms.controls.get_mut(&neighbor).unwrap().value;editor.cursor=5;editor.anchor=2;editor.offset=1;}
    let raw=app.raw.clone();let location=app.location.clone();let epoch=app.script_epoch;
    let go=node(&app,"go");let rect=app.page.controls.iter().find(|rect|rect.node==go).unwrap();
    let (x,y)=(rect.x+rect.width/2,app.content_top()+rect.y+rect.height/2-app.scroll);
    let mut before=app.draw().pixels;
    for index in [0,1] {
        app.pointer_down(x,y,false);app.pointer_up(x,y);
        assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
        assert_eq!(app.document.title,format!("Code units key {index}"));
        let output=node(&app,"out");let rect=app.page.border_rects[output].unwrap();
        assert_eq!((rect.x,rect.y,rect.width,rect.height),(20,100,300,40));
        let text:String=app.page.runs.iter().filter(|run|run.x>=20&&run.x<320&&run.y>=100&&run.y<140).map(|run|run.text.as_str()).collect();
        assert_eq!(text,format!("Key{index}"));
        let after=app.draw();assert_ne!(before,after.pixels);
        let top=(app.content_top()+100-app.scroll) as usize;
        assert!((top..top+40).any(|y|after.pixels[y*after.width+20..y*after.width+320].contains(&(if index==0{0x123456}else{0x654321}))));
        before=after.pixels;
        let editor=&app.forms.controls[&neighbor].value;
        assert_eq!((editor.text(),editor.cursor,editor.anchor,editor.offset),("edited neighbor".into(),5,2,1));
        assert_eq!(app.page.link_node_at(70,192),Some(node(&app,"stable")));
        assert_eq!(app.page.link_at(70,192),Some("#destination"));
        assert_eq!(app.raw,raw);assert_eq!(app.location,location);assert_eq!(app.script_epoch,epoch);
    }
}
