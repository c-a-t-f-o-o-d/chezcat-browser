//! Original exact property-key consumer through native App input and paint.
use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};

const MARKUP:&str=r#"<!doctype html><html><head><title>Forward ready</title>
<style>html,body{margin:0;background:white}#go{position:absolute;left:20px;top:20px;width:160px;height:32px}#neighbor{position:absolute;left:220px;top:20px;width:200px;height:32px}#out{position:absolute;left:20px;top:100px;width:300px;height:40px;margin:0;color:#123456;font-size:24px}#stable{position:absolute;left:20px;top:180px;width:100px;height:24px}</style>
</head><body><button id=go>Search key</button><input id=neighbor value=seed><p id=out>Waiting</p><a id=stable href=#destination>Go</a><div id=destination></div><script>
var round=0;
document.getElementById('go').onclick=function(e){
if(!e.isTrusted)throw 'native input';
var key=round===0?'A\ud800B\ud800':'A\udfffB\ud800',record={};record[key]=7;
var parsed=JSON.parse(JSON.stringify(record)),actual=Object.keys(parsed)[0];
if(actual!==key||parsed[actual]!==7)throw 'exact property key';
var script=document.createElement('script');script.charset=actual;
var order='',f=String.prototype.indexOf;
var index=f.call({toString:function(){order+='R';return script.charset;}},{toString:function(){order+='S';return '\ud800';}},{valueOf:function(){order+='P';return NaN;}});
if(order!=='RSP'||index!==(round===0?1:3)||f.call('😀','\ude00')!==1)throw 'exact forward key consumer';
round++;document.getElementById('out').textContent='Key '+index;document.title='Forward key '+index;
};
</script></body></html>"#;

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new()->Self {
        static NEXT:AtomicUsize=AtomicUsize::new(0);
        let path=std::env::temp_dir().join(format!("chezcat-string-forward-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        std::fs::write(&path,MARKUP).unwrap();Self(path)
    }
}
impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
fn node(app:&App,name:&str)->usize {(0..app.document.nodes.len()).find(|&n|app.document.attr(n,"id")==Some(name)).unwrap()}

#[test]
fn string_index_utf16_actual_app_reflected_key_drives_native_paint_and_retains_interaction() {
    let fixture=Fixture::new();let mut app=App::new();app.resize(640,480);app.set_javascript_enabled(true);app.open(&fixture.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0);
    let neighbor=node(&app,"neighbor");app.forms.script_set_value(neighbor,"edited neighbor").unwrap();
    {let editor=&mut app.forms.controls.get_mut(&neighbor).unwrap().value;editor.cursor=5;editor.anchor=2;editor.offset=1;}
    let raw=app.raw.clone();let location=app.location.clone();let epoch=app.script_epoch;
    let go=node(&app,"go");let rect=app.page.controls.iter().find(|rect|rect.node==go).unwrap();
    let (x,y)=(rect.x+rect.width/2,app.content_top()+rect.y+rect.height/2-app.scroll);
    let mut before=app.draw().pixels;
    for index in [1,3] {
        app.pointer_down(x,y,false);app.pointer_up(x,y);
        assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
        assert_eq!(app.document.title,format!("Forward key {index}"));
        let output=node(&app,"out");let rect=app.page.border_rects[output].unwrap();
        assert_eq!((rect.x,rect.y,rect.width,rect.height),(20,100,300,40));
        let text:String=app.page.runs.iter().filter(|run|run.x>=20&&run.x<320&&run.y>=100&&run.y<140).map(|run|run.text.as_str()).collect();
        assert_eq!(text,format!("Key{index}"));
        let after=app.draw();assert_ne!(before,after.pixels);
        let top=(app.content_top()+100-app.scroll) as usize;
        assert!((top..top+40).any(|y|after.pixels[y*after.width+20..y*after.width+320].contains(&0x123456)));
        before=after.pixels;
        let editor=&app.forms.controls[&neighbor].value;
        assert_eq!((editor.text(),editor.cursor,editor.anchor,editor.offset),("edited neighbor".into(),5,2,1));
        assert_eq!(app.page.link_node_at(70,192),Some(node(&app,"stable")));
        assert_eq!(app.page.link_at(70,192),Some("#destination"));
        assert_eq!(app.raw,raw);assert_eq!(app.location,location);assert_eq!(app.script_epoch,epoch);
    }
}
