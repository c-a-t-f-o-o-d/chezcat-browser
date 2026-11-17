//! Original App software-input/paint proof for String.prototype.concat.
//! This is an isolated local page, not framework or physical desktop acceptance.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

const MARKUP: &str = r#"<!doctype html><html><head><title>Concat ready</title>
<style>html,body{margin:0;background:#ffffff}#go{position:absolute;left:20px;top:20px;width:160px;height:32px}#out{position:absolute;left:20px;top:80px;width:240px;height:40px;margin:0;padding:0;border:0;background:#ffffff;color:#123456;font-size:24px;line-height:32px}</style>
</head><body><button id=go type=button>Join text</button><p id=out>I</p><script>
var button=document.getElementById('go'),round=0;
button.onclick=function(event){
    if(!event.isTrusted||this!==button)throw 'actual trusted button input';
    var order='',letter=round===0?'W':'I';
    var receiver={toString:function(){order+='R';return letter;},valueOf:function(){throw 'receiver used number hint';}};
    var first={toString:function(){order+='A';return letter;},valueOf:function(){throw 'argument used number hint';}};
    var second={toString:function(){order+='B';return letter;},valueOf:function(){throw 'argument used number hint';}};
    var text=String.prototype.concat.call(receiver,first,second);
    if(order!=='RAB'||text!==(round===0?'WWW':'III'))throw 'real ordered string conversion';
    round++;
    document.getElementById('out').textContent=text;
    document.title='Concat '+round;
};
</script></body></html>"#;

struct ConcatFixture(std::path::PathBuf);
impl ConcatFixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path=std::env::temp_dir().join(format!("chezcat-string-concat-app-{}-{}.html",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::write(&path, MARKUP).unwrap();
        Self(path)
    }
}
impl Drop for ConcatFixture {
    fn drop(&mut self) { let _=std::fs::remove_file(&self.0); }
}

fn node(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len()).find(|&node| app.document.attr(node,"id")==Some(name)).unwrap()
}
fn output_text(app: &App, output: usize) -> String {
    app.document.nodes[output].children.iter().filter_map(|&child| {
        if let html::NodeKind::Text(text)=&app.document.nodes[child].kind {Some(text.as_str())} else {None}
    }).collect()
}
fn text_pixels(app: &App, output: usize, expected: &str) -> Vec<u32> {
    assert_eq!(output_text(app, output), expected);
    let rect=app.page.border_rects[output].expect("Output owns an actual layout box");
    assert_eq!((rect.x,rect.y,rect.width,rect.height),(20,80,240,40));
    let runs:Vec<_>=app.page.runs.iter().filter(|run| run.x>=rect.x&&run.x<rect.x+rect.width
        &&run.y>=rect.y&&run.y<rect.y+rect.height).collect();
    assert_eq!(runs.iter().map(|run|run.text.as_str()).collect::<Vec<_>>(),[expected]);
    assert!(runs.iter().all(|run|run.color==0x123456));
    let left=rect.x as usize;
    let top=(app.content_top()+rect.y-app.scroll) as usize;
    let canvas=app.draw();
    assert!(left+rect.width as usize<=canvas.width&&top+rect.height as usize<=canvas.height);
    let pixels:Vec<_>=(top..top+rect.height as usize).flat_map(|y| {
        canvas.pixels[y*canvas.width+left..y*canvas.width+left+rect.width as usize].iter().copied()
    }).collect();
    assert!(pixels.iter().any(|&pixel|pixel!=0xffffff),"The isolated text must actually paint");
    assert_eq!(pixels.last(),Some(&0xffffff));
    pixels
}

#[test]
fn string_concat_actual_app_pointer_release_converts_objects_in_order_and_changes_text_pixels() {
    let fixture=ConcatFixture::new();
    let mut app=App::new();
    app.resize(640,480);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{:?}",app.events);
    assert_eq!(app.document.title,"Concat ready");
    let raw=app.raw.clone();let location=app.location.clone();
    let history=(app.history_index,app.history.len());let epoch=app.script_epoch;
    let output=node(&app,"out");let button=node(&app,"go");
    assert!(matches!(app.forms.controls[&button].kind,Kind::Button));
    let rect=app.page.controls.iter().find(|rect|rect.node==button).expect("Button must be a real App hit target");
    let x=rect.x+rect.width/2;let y=app.content_top()+rect.y+rect.height/2-app.scroll;
    assert!(x>=0&&x<app.width&&y>=app.content_top()&&y<app.content_bottom());
    assert_eq!(app.page.control_at(x,y-app.content_top()+app.scroll),Some(button));
    let mut previous=text_pixels(&app,output,"I");
    let mut previous_text="I";
    for (count,expected) in [(1,"WWW"),(2,"III")] {
        assert!(matches!(app.pointer_down(x,y,false),PlatformAction::None));
        assert_eq!(output_text(&app,output),previous_text,"Down alone must not execute the authored click");
        assert_eq!(app.document.title,if count==1 {"Concat ready".to_owned()} else {format!("Concat {}",count-1)});
        assert!(matches!(app.pointer_up(x,y),PlatformAction::None));
        assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{:?}",app.events);
        assert_eq!(app.document.title,format!("Concat {count}"));
        let pixels=text_pixels(&app,output,expected);
        assert_ne!(pixels,previous,"Only the text changes inside the fixed white crop");
        previous=pixels;previous_text=expected;
    }
    assert!(!app.failed&&!app.is_loading());
    assert_eq!(app.raw,raw);assert_eq!(app.location,location);
    assert_eq!((app.history_index,app.history.len()),history);
    assert_eq!(app.script_epoch,epoch);
}
