//! Original real-App Comment lifecycle under native pointer/text input.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new()->Self {
        static NEXT:AtomicUsize=AtomicUsize::new(0);
        let directory=std::env::temp_dir().join(format!("chezcat-comment-app-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("page.html"),r#"<!doctype html><html><head><title>Comment waiting</title>
<style>html,body{margin:0}#owner{display:flex;flex-flow:row nowrap;gap:10px;width:160px;height:36px;background:#21344c}.tile{display:block;flex:none;width:50px;height:36px;background:#385c84}#payload{width:180px}</style></head><body>
<div id=owner><a id=left class=tile href=destination.html#left>Left</a><a id=right class=tile href=destination.html#right>Right</a></div>
<input id=payload type=text value=""><button id=change type=button>Next comment step</button><p id=phase>Ready</p>
<script>
var owner=document.getElementById('owner'),right=document.getElementById('right'),list=owner.childNodes,initial=Array.prototype.slice.call(list),created=null,insertedSnapshot=null,step=0;
var resources=' <img src=https://unrequested.test/image><script src=https://unrequested.test/script><link rel=stylesheet href=https://unrequested.test/style>';
if(list.length!==2||initial[0]!==document.getElementById('left')||initial[1]!==right)throw 'original real source children';
document.getElementById('change').onclick=function(event){
 if(!event.isTrusted)throw 'expected native trusted button input';
 var value=document.getElementById('payload').value;
 if(step===0){
  if(value!=='Typed 雪')throw 'expected actual edited input value';
  created=document.createComment(value+resources);owner.insertBefore(created,right);insertedSnapshot=Array.prototype.slice.call(list);
  if(created.nodeType!==8||created.nodeName!=='#comment'||created.data!==value+resources||list.length!==3||list[1]!==created||created.parentNode!==owner||!created.isConnected||insertedSnapshot[1]!==created)throw 'genuine created and inserted Comment';
  step=1;document.title='Comment inserted';document.getElementById('phase').textContent='Inserted';
 }else if(step===1){
  created.textContent='Edited '+value+resources+' url(https://unrequested.test/background)';
  if(created.nodeType!==8||created.nodeValue!=='Edited '+value+resources+' url(https://unrequested.test/background)'||insertedSnapshot[1]!==created||list[1]!==created)throw 'same retained Comment edited';
  step=2;document.title='Comment edited';document.getElementById('phase').textContent='Edited';
 }else if(step===2){
  owner.removeChild(created);
  if(created.parentNode!==null||created.isConnected||created.nodeType!==8||list.length!==2||list[0]!==initial[0]||list[1]!==right||insertedSnapshot.length!==3||insertedSnapshot[1]!==created)throw 'removed Comment retained by ordinary snapshot';
  step=3;document.title='Comment removed';document.getElementById('phase').textContent='Removed';
 }else throw 'unexpected extra input';
 if(owner.childNodes!==list||owner.textContent!=='LeftRight'||owner.getElementsByTagName('*').length!==2||owner.querySelectorAll('*').length!==2||initial.length!==2||initial[1]!==right||created.childNodes.length)throw 'live childNodes differs from Element queries and Text';
};document.title='Comment ready';
</script></body></html>"#).unwrap();
        Self(directory)
    }
}
impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
fn id(app:&App,name:&str)->usize {(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap()}
fn click_control(app:&mut App,node:usize) {
    let rect=app.page.controls.iter().find(|rect|rect.node==node).expect("real rendered native control");
    let x=rect.x+rect.width/2;let y=app.content_top()+rect.y+rect.height/2-app.scroll;
    assert!(x>=0&&x<app.width&&y>=app.content_top()&&y<app.content_bottom());
    assert_eq!(app.page.control_at(x,y-app.content_top()+app.scroll),Some(node));
    assert!(matches!(app.pointer_down(x,y,false),PlatformAction::None));
    assert!(matches!(app.pointer_up(x,y),PlatformAction::None));
}
fn owner_pixels(app:&App,owner:usize)->Vec<u32> {
    let rect=app.page.border_rects[owner].expect("real owner border rectangle");
    let canvas=app.draw();let x=rect.x;let y=app.content_top()+rect.y-app.scroll;
    assert!(x>=0&&y>=app.content_top()&&rect.width>0&&rect.height>0);
    assert!((x+rect.width) as usize<=canvas.width&&(y+rect.height) as usize<=canvas.height);
    let mut pixels=Vec::with_capacity((rect.width*rect.height) as usize);
    for row in y..y+rect.height {
        let start=row as usize*canvas.width+x as usize;
        pixels.extend_from_slice(&canvas.pixels[start..start+rect.width as usize]);
    }
    assert_eq!(canvas.pixels[(y+rect.height-2) as usize*canvas.width+(x+rect.width-2) as usize],0x21344c,"Original unused owner background remains visible");
    pixels
}
fn no_resource_tasks(app:&App) {
    assert!(!app.failed,"{}",app.status);assert!(!app.is_loading());
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{:?}",app.events);
    assert!(!app.scripting.as_ref().unwrap().has_host_tasks());
    assert_eq!(app.dynamic_pending,0);assert_eq!(app.pending_stylesheets,0);
    assert!(app.pending_images.is_empty()&&app.pending_backgrounds.is_empty());
}

#[test]
fn comment_real_app_pointer_and_text_input_keep_pixels_layout_links_and_resources_stable() {
    let fixture=Fixture::new();let mut app=App::new();app.resize(640,480);app.set_javascript_enabled(true);app.open(&fixture.0.join("page.html"));
    no_resource_tasks(&app);assert_eq!(app.document.title,"Comment ready","{:?}",app.events);
    let owner=id(&app,"owner");let entry=id(&app,"payload");let button=id(&app,"change");let right=id(&app,"right");
    click_control(&mut app,entry);assert!(app.focus==Focus::Control(entry));
    for ch in "Typed 雪".chars(){app.text_input(ch);}
    assert_eq!(app.forms.controls[&entry].value.text(),"Typed 雪");no_resource_tasks(&app);
    let raw=app.raw.clone();let location=app.location.display();let history_index=app.history_index;let history_len=app.history.len();let epoch=app.script_epoch;
    let rect=app.page.border_rects[owner].unwrap();let bounds=(rect.x,rect.y,rect.width,rect.height);assert_eq!((rect.width,rect.height),(160,36));
    let pixels=owner_pixels(&app,owner);
    // A genuine link may occupy multiple word runs in the existing narrow tile.
    // Require its complete readable label and real node/href ownership.
    let right_runs:Vec<_>=app.page.runs.iter().filter(|run|app.page.link_node_at(run.x+2,run.y+2)==Some(right)).collect();
    let label:String=right_runs.iter().map(|run|run.text.as_str()).collect();
    assert_eq!(label,"Right","Actual Right-owned link fragments must retain their full label");
    assert_eq!(app.document.attr(right,"href"),Some("destination.html#right"));
    let link_run=right_runs.first().expect("original real Right link hit");
    let hit=(link_run.x+2,link_run.y+2);assert_eq!(app.page.link_node_at(hit.0,hit.1),Some(right));
    let target=app.page.link_at(hit.0,hit.1).unwrap().to_owned();
    let original_script_elements=app.document.nodes.iter().filter(|node|matches!(&node.kind,crate::html::NodeKind::Element{tag,..} if tag=="script")).count();
    let mut comment_id=None;
    for (stage,title,children) in [(1,"Comment inserted",3),(2,"Comment edited",3),(3,"Comment removed",2)] {
        click_control(&mut app,button);no_resource_tasks(&app);assert_eq!(app.document.title,title,"{:?}",app.events);
        assert_eq!(app.forms.controls[&entry].value.text(),"Typed 雪");assert_eq!(app.document.nodes[owner].children.len(),children);
        let comments:Vec<_>=app.document.nodes.iter().enumerate().filter_map(|(node,item)|matches!(item.kind,crate::html::NodeKind::Comment(_)).then_some(node)).collect();
        assert_eq!(comments.len(),1,"Only a genuine Comment arena node was created");
        let node=comments[0];if let Some(saved)=comment_id {assert_eq!(node,saved);}else{comment_id=Some(node);}
        let crate::html::NodeKind::Comment(data)=&app.document.nodes[node].kind else{unreachable!()};
        assert!(data.contains("https://unrequested.test/")&&data.contains("Typed 雪"));assert_eq!(data.starts_with("Edited "),stage>=2);
        assert!(app.page.client_boxes[node].is_none()&&app.page.border_rects[node].is_none(),"Comment has no rendered box");
        let rect=app.page.border_rects[owner].unwrap();assert_eq!((rect.x,rect.y,rect.width,rect.height),bounds);
        assert_eq!(owner_pixels(&app,owner),pixels,"Comment insertion/data/removal has no owner pixel footprint: {stage}");
        assert_eq!(app.page.link_node_at(hit.0,hit.1),Some(right));assert_eq!(app.page.link_at(hit.0,hit.1),Some(target.as_str()));
        assert!(!app.page.runs.iter().any(|run|run.x>=rect.x&&run.x<rect.x+rect.width&&run.y>=rect.y&&run.y<rect.y+rect.height&&(run.text.contains("Typed")||run.text.contains("unrequested"))),"Comment data never becomes owner Text");
        assert_eq!(app.document.nodes.iter().filter(|node|matches!(&node.kind,crate::html::NodeKind::Element{tag,..} if tag=="script")).count(),original_script_elements);
        assert_eq!(app.raw,raw);assert_eq!(app.location.display(),location);assert_eq!(app.history_index,history_index);assert_eq!(app.history.len(),history_len);assert_eq!(app.script_epoch,epoch);
    }
}
