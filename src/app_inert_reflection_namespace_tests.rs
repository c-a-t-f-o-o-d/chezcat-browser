//! Native inert reflections preserve namespace-separated data and drive actual App paint.
use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};

const MARKUP:&str=r#"<!doctype html><html><head><title>Code units ready</title>
<style>html,body{margin:0;background:white}#go{position:absolute;left:20px;top:20px;width:160px;height:32px}#neighbor{position:absolute;left:220px;top:20px;width:200px;height:32px}#out{position:absolute;left:20px;top:100px;width:300px;height:40px;margin:0;color:#123456;font-size:24px}#stable{position:absolute;left:20px;top:180px;width:100px;height:24px}#out.ready{color:#654321}.painted{color:#357913}.A�z,.B�z{color:#ffff00}</style>
</head><body><button id=go>Search key</button><input id=neighbor value=seed><p id=out>Waiting</p><a id=stable href=#destination>Go</a><div id=destination></div><form id=form style='position:absolute;left:20px;top:240px'><input id=seed><input id=literal name='A�z'></form><script>
var round=0;
document.getElementById('go').onclick=function(e){
if(!e.isTrusted)throw 'native input';
var out=document.getElementById('out');
if(round===0){var owner=document.implementation.createHTMLDocument('source'),p=owner.createElement('p');p.id='materialized0';var source=owner.createElement('span');for(var name of ['id','title','lang'])source.setAttributeNS('urn:attribute',name,'foreign');source.id='A\ud800z';source.title='T\ud800z';source.lang='L\udfffz';if(source.getAttributeNS('urn:attribute','id')!=='foreign'||source.getAttributeNS('urn:attribute','title')!=='foreign'||source.getAttributeNS('urn:attribute','lang')!=='foreign')throw 'native namespaced source';p.classList.add('painted',source.id);p.title=source.title;p.lang=source.lang;p.dir='D\ud800z';p.textContent='imported';p.setAttribute('style','position:absolute;left:400px;top:300px;width:100px;height:24px;margin:0;font-size:20px');var c=document.importNode(p,true);document.body.appendChild(c);var tokens=c.classList;c.classList='painted A\ud800z';if(c.classList!==tokens)throw 'native stable tokens';if(c===p||c.className!==p.getAttribute('class')||c.title!==p.title||c.lang!==p.lang||c.dir!==''||c.getAttribute('dir')!==p.getAttribute('dir')||c.ownerDocument!==document||p.ownerDocument!==owner||!c.classList.contains('A\ud800z')||c.matches('.A�z')||document.getElementsByClassName('A\ud800z')[0]!==c)throw 'public metadata import';out.className='';}
else{var t=document.createElement('template');t.innerHTML='<span style="position:absolute;left:400px;top:340px;width:100px;height:24px;margin:0;font-size:20px" id=materialized1>promoted</span>';var p=t.content.firstChild;var descriptor=Object.getOwnPropertyDescriptor(Element.prototype,'className');var held=p.classList;var xml=document.implementation.createDocument(null,'Root'),source=xml.documentElement;source.setAttributeNS('urn:attribute','id','foreign');source.id='B\udfffz';if(source.getAttributeNS('urn:attribute','id')!=='foreign')throw 'native XML namespace';held.add('painted',source.id);p.title='T\udfffz';p.lang='L\ud800z';p.dir='D\udfffz';var a=p.getAttributeNode('title');document.body.appendChild(t.content);if(p.classList!==held)throw 'promoted token identity';var tokens=p.classList,forward=Object.getOwnPropertyDescriptor(Element.prototype,'classList'),calls=0;Object.defineProperty(p,'classList',{get:function(){calls++;return tokens;},configurable:true});forward.set.call(p,'painted B\udfffz');if(calls!==1||tokens!==forward.get.call(p))throw 'author getter forwarding';if(descriptor.get.call(p)!=='painted B\udfffz'||p.ownerDocument!==document||document.getElementsByClassName('B\udfffz')[0]!==p||p.getAttributeNode('title')!==a||a.ownerDocument!==document||a.value!=='T\udfffz'||p.lang!=='L\ud800z'||p.dir!==''||p.matches('.B�z'))throw 'public metadata promotion';out.className='ready';}
out.textContent='Key '+round;document.title='Code units key '+round;round++;
};
</script></body></html>"#;

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new()->Self {
        static NEXT:AtomicUsize=AtomicUsize::new(0);
        let path=std::env::temp_dir().join(format!("chezcat-app-inert-reflection-namespace-tests-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        std::fs::write(&path,MARKUP).unwrap();Self(path)
    }
}
impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
fn node(app:&App,name:&str)->usize {(0..app.document.nodes.len()).find(|&n|app.document.attr(n,"id")==Some(name)).unwrap()}

#[test]
fn inert_reflection_namespace_actual_app_native_values_retain_paint_and_interaction() {
    let fixture=Fixture::new();let mut app=App::new();app.resize(640,480);app.set_javascript_enabled(true);app.open(&fixture.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0);
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
        let subject=node(&app,&format!("materialized{index}"));let painted=app.page.border_rects[subject].unwrap();
        assert_eq!((painted.x,painted.y,painted.width,painted.height),(400,300+index*40,100,24));
        let subject_top=(app.content_top()+painted.y-app.scroll) as usize;
        assert!((subject_top..subject_top+24).any(|y|after.pixels[y*after.width+400..y*after.width+500].contains(&0x357913)));
        before=after.pixels;
        let editor=&app.forms.controls[&neighbor].value;
        assert_eq!((editor.text(),editor.cursor,editor.anchor,editor.offset),("edited neighbor".into(),5,2,1));
        assert_eq!(app.page.link_node_at(70,192),Some(node(&app,"stable")));
        assert_eq!(app.page.link_at(70,192),Some("#destination"));
        assert_eq!(app.raw,raw);assert_eq!(app.location,location);assert_eq!(app.script_epoch,epoch);
    }
}
