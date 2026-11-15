//! Original actual-App input/render regression, distinct from upstream suite results.
use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new()->Self {
        static NEXT:AtomicUsize=AtomicUsize::new(0);
        let path=std::env::temp_dir().join(format!("chezcat-child-node-app-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("page.html"),r#"<!doctype html><html><head><title>Child nodes waiting</title>
<style>html,body{margin:0}#owner{width:81px;height:27px;background:#111111}</style></head><body>
<p id=output>Waiting for input</p><div id=owner>first<span id=middle>second</span></div><button id=change>Change list</button>
<script>
var docList=document.childNodes;
var documentSnapshot=Array.prototype.slice.call(docList);
if(docList!==document.childNodes||!Array.isArray(documentSnapshot)||documentSnapshot.length!==docList.length||documentSnapshot.indexOf(document.documentElement)<0)throw 'real Document child-node snapshot';
var owner=document.getElementById('owner');var children=owner.childNodes;
var snapshot=Array.prototype.slice.call(children);
if(children!==owner.childNodes||children.length!==2||children.item(0)!==snapshot[0]||snapshot[0].nodeType!==3||snapshot[1]!==document.getElementById('middle')||Object.getPrototypeOf(snapshot)!==Array.prototype)throw 'real Element child-node snapshot';
document.getElementById('change').onclick=function(){
 owner.appendChild(document.createTextNode('last'));
 if(children!==owner.childNodes||children.length!==3||children[2].nodeType!==3||snapshot.length!==2||snapshot[2]!==undefined||children.item(3)!==null)throw 'live source and static copied Array';
 var next=Array.prototype.slice.call(children,-2);if(next.length!==2||next[0]!==snapshot[1]||next[1]!==children[2])throw 'post-mutation borrowed slice';
 owner.style.width='141px';owner.style.backgroundColor='#3c6e91';
 document.getElementById('output').textContent='Live list updated';document.title='Child nodes changed by input';
};document.title='Child nodes ready';
</script></body></html>"#).unwrap();Self(path)
    }
}
impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
fn id(app:&App,name:&str)->usize {(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap()}

#[test]
fn child_nodes_actual_app_input_retains_live_lists_and_static_array_snapshots(){
    let fixture=Fixture::new();let mut app=App::new();app.resize(640,480);app.set_javascript_enabled(true);app.open(&fixture.0.join("page.html"));
    assert!(!app.failed,"{}",app.status);assert!(!app.is_loading());
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{:?}",app.events);
    assert_eq!(app.document.title,"Child nodes ready","{:?}",app.events);
    let raw=app.raw.clone();let location=app.location.display();let history_index=app.history_index;let history_len=app.history.len();let epoch=app.script_epoch;
    let owner=id(&app,"owner");let button=id(&app,"change");let output=id(&app,"output");
    assert_eq!(app.page.client_boxes[owner].unwrap().width,81);
    let rect=app.page.controls.iter().find(|rect|rect.node==button).expect("The original button must be a real rendered hit");
    let x=rect.x+rect.width/2;let y=app.content_top()+rect.y+rect.height/2-app.scroll;
    assert!(x>=0&&x<app.width&&y>=app.content_top()&&y<app.content_bottom());assert_eq!(app.page.control_at(x,y-app.content_top()+app.scroll),Some(button));
    assert!(matches!(app.pointer_down(x,y,false),PlatformAction::None));assert!(matches!(app.pointer_up(x,y),PlatformAction::None));
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{:?}",app.events);assert_eq!(app.document.title,"Child nodes changed by input","{:?}",app.events);
    assert_eq!(app.page.client_boxes[owner].unwrap().width,141);assert_eq!(app.document.nodes[owner].children.len(),3);
    let output_source:String=app.document.nodes[output].children.iter().filter_map(|&node|match &app.document.nodes[node].kind {crate::html::NodeKind::Text(text)=>Some(text.as_str()),_=>None}).collect();
    assert_eq!(output_source,"Live list updated");
    let output_rect=app.page.border_rects[output].unwrap();
    let output_words:Vec<_>=app.page.runs.iter().filter(|run|run.x>=output_rect.x&&run.x<output_rect.x+output_rect.width&&run.y>=output_rect.y&&run.y<output_rect.y+output_rect.height).map(|run|run.text.as_str()).collect();
    assert_eq!(output_words,["Live","list","updated"]);
    let rect=app.page.border_rects[owner].unwrap();let canvas=app.draw();let x=(rect.x+rect.width-2) as usize;let y=(app.content_top()+rect.y+rect.height-2-app.scroll) as usize;
    assert_eq!(canvas.pixels[y*canvas.width+x],0x3c6e91);
    assert_eq!(app.raw,raw);assert_eq!(app.location.display(),location);assert_eq!(app.history_index,history_index);assert_eq!(app.history.len(),history_len);assert_eq!(app.script_epoch,epoch);
}
