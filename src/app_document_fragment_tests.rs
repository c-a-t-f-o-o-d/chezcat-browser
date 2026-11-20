//! Original real-App fragment batch, isolated pixels, link hits and editor proof.
use super::*;
use crate::html::NodeKind;
use std::sync::atomic::{AtomicUsize,Ordering};

fn id(app:&App,name:&str)->usize {(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap()}
fn clean(app:&App) {
    assert!(!app.failed,"{}",app.status);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
    assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
}
fn click(app:&mut App,name:&str) {
    let node=id(app,name);let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();
    let(x,page_y)=(rect.x+rect.width/2,rect.y+rect.height/2);
    assert_eq!(app.page.control_at(x,page_y),Some(node));
    let y=app.content_top()+page_y-app.scroll;
    assert!(matches!(app.pointer_down(x,y,false),PlatformAction::None));
    assert!(matches!(app.pointer_up(x,y),PlatformAction::None));clean(app);
}
fn editor(app:&App,node:usize)->(String,usize,usize,usize,bool,Option<usize>) {
    let c=&app.forms.controls[&node];(c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.owner)
}
fn output_pixels(app:&App,expected:&str)->Vec<u32> {
    let rect=app.page.border_rects[id(app,"out")].unwrap();
    assert_eq!((rect.x,rect.y,rect.width,rect.height),(20,100,300,40));
    let runs:Vec<_>=app.page.runs.iter().filter(|run|run.x>=rect.x&&run.x<rect.x+rect.width&&run.y>=rect.y&&run.y<rect.y+rect.height).collect();
    assert_eq!(runs.iter().map(|run|run.text.as_str()).collect::<Vec<_>>(),[expected]);assert!(runs.iter().all(|run|run.color==0x123456));
    let canvas=app.draw();let top=(app.content_top()+rect.y-app.scroll)as usize;let left=rect.x as usize;
    let pixels:Vec<_>=(top..top+40).flat_map(|y|canvas.pixels[y*canvas.width+left..y*canvas.width+left+300].iter().copied()).collect();
    assert!(pixels.iter().any(|&pixel|pixel!=0xffffff));assert_eq!(pixels.last(),Some(&0xffffff));pixels
}

#[test]
fn document_fragment_actual_app_batch_paints_real_children_and_preserves_neighbor_editor() {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    struct Fixture(PathBuf);impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let fixture=Fixture(std::env::temp_dir().join(format!("chezcat-fragment-app-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Fragment pending</title><style>
html,body{margin:0;padding:0;background:#ffffff}
#neighbor{position:absolute;left:20px;top:20px;width:220px;height:32px;margin:0}
#build{position:absolute;left:280px;top:20px;width:180px;height:32px}
#out{position:absolute;left:20px;top:100px;width:300px;height:40px;margin:0;padding:0;border:0;background:#ffffff;color:#123456;font-size:24px;line-height:32px}
#mount{position:absolute;left:20px;top:180px;width:240px;height:80px;background:#f0e0c0}
#fragment-link{display:block;position:absolute;left:80px;top:0;width:80px;height:24px;background:#336699;color:white}
#fragment-box{display:block;position:absolute;left:0;top:32px;width:100px;height:20px;background:#cc3300;color:white}
</style></head><body><form id=f><input id=neighbor name=neighbor value=seed><button id=build type=button>Insert original batch</button><div id=mount></div></form><p id=out>Before</p><div id=destination></div><script>
var fragment=null,donorList=null,originalText=null,originalComment=null,originalLink=null,originalBox=null;
document.getElementById('build').onclick=function(event){
 if(!event.isTrusted||fragment!==null)throw 'one real trusted build';
 fragment=document.createDocumentFragment();donorList=fragment.childNodes;fragment.id='phantom-fragment';
 originalText=document.createTextNode('Mixed ');originalComment=document.createComment('Original no-box comment');
 originalLink=document.createElement('a');originalLink.id='fragment-link';originalLink.setAttribute('href','#destination');originalLink.textContent='Go';
 originalBox=document.createElement('span');originalBox.id='fragment-box';originalBox.textContent='Box';
 fragment.appendChild(originalText);fragment.appendChild(originalComment);fragment.appendChild(originalLink);fragment.appendChild(originalBox);
 if(donorList.length!==4||originalLink.getRootNode()!==fragment)throw 'actual mixed donor';
 var mount=document.getElementById('mount');if(mount.appendChild(fragment)!==fragment)throw 'real fragment return';
 if(donorList!==fragment.childNodes||donorList.length!==0||fragment.parentNode!==null||fragment.isConnected||fragment.getRootNode()!==fragment||document.getElementById('phantom-fragment')!==null)throw 'empty non-Element donor';
 if(mount.childNodes.length!==4||mount.childNodes[0]!==originalText||mount.childNodes[1]!==originalComment||mount.childNodes[2]!==originalLink||mount.childNodes[3]!==originalBox||originalLink.parentNode!==mount||originalComment.getRootNode()!==document||mount.textContent!=='Mixed GoBox')throw 'real inserted identities';
 document.getElementById('out').textContent='BatchReady';document.title='Fragment complete';
};
</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(640,480);app.set_javascript_enabled(true);app.open(&fixture.0);clean(&app);
    let raw=app.raw.clone();let location=app.location.clone();let history=(app.history_index,app.history.len());
    click(&mut app,"neighbor");
    assert!(matches!(app.key(0x41,true,false,false),PlatformAction::None));app.paste("neighbor kept");
    assert!(matches!(app.key(0x24,false,false,false),PlatformAction::None));
    for _ in 0..2 {app.key(0x27,false,false,false);}
    for _ in 0..3 {app.key(0x27,false,true,false);}
    let neighbor=id(&app,"neighbor");let preserved=editor(&app,neighbor);
    assert_eq!((preserved.0.as_str(),preserved.1,preserved.2),("neighbor kept",5,2));assert!(preserved.4);
    let before=output_pixels(&app,"Before");
    click(&mut app,"build");assert_eq!(app.document.title,"Fragment complete");
    assert_eq!(editor(&app,neighbor),preserved,"Batch insertion must not reconstruct an unrelated live editor");
    assert_ne!(output_pixels(&app,"BatchReady"),before,"The isolated original result must actually change pixels");
    let fragment=app.document.nodes.iter().position(|node|matches!(node.kind,NodeKind::DocumentFragment)).unwrap();
    let comment=app.document.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Comment(text)if text=="Original no-box comment")).unwrap();
    for node in [fragment,comment] {
        assert!(app.page.border_rects[node].is_none(),"Non-box Node {node} acquired a layout box");
        assert!(!app.page.controls.iter().any(|control|control.node==node));
        assert!(app.focus!=Focus::Control(node));
    }
    assert!(app.document.nodes[fragment].children.is_empty());
    assert!(!app.page.runs.iter().any(|run|run.text.contains("no-box")||run.text.contains("comment")),"Comment data must not paint");
    let link=id(&app,"fragment-link");let rect=app.page.border_rects[link].unwrap();
    assert_eq!((rect.x,rect.y,rect.width,rect.height),(100,180,80,24));
    for(x,y)in[(rect.x,rect.y),(rect.x+40,rect.y+12),(rect.x+79,rect.y+23)] {
        assert_eq!(app.page.link_node_at(x,y),Some(link));assert_eq!(app.page.link_at(x,y),Some("#destination"));
    }
    assert_eq!(app.page.link_node_at(rect.x-1,rect.y+12),None);assert_eq!(app.page.link_node_at(rect.x+80,rect.y+12),None);
    let canvas=app.draw();let pixel=|x:i32,y:i32|canvas.pixels[(app.content_top()+y-app.scroll)as usize*canvas.width+x as usize];
    assert_eq!(pixel(rect.x+76,rect.y+20),0x336699);
    let block=app.page.border_rects[id(&app,"fragment-box")].unwrap();assert_eq!((block.x,block.y,block.width,block.height),(20,212,100,20));
    assert_eq!(pixel(block.x+96,block.y+10),0xcc3300);
    assert!(app.page.runs.iter().any(|run|run.text=="Mixed"));assert!(app.page.runs.iter().any(|run|run.text=="Go"));
    assert_eq!(app.forms.controls.len(),2,"Only the original text field and button are native controls");
    assert_eq!(app.raw,raw);assert_eq!(app.location,location);assert_eq!((app.history_index,app.history.len()),history);clean(&app);
}

#[test]
fn document_fragment_imported_template_renders_real_pixels_and_link_hits() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "chezcat-template-import-{}-{}.html", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    std::fs::write(&fixture.0, r#"<!doctype html><html><head><title>Template pending</title><style>
html,body{margin:0;padding:0;background:#ffffff}
#copy{position:absolute;left:40px;top:30px;width:160px;height:30px}
#mount{position:absolute;left:40px;top:100px;width:240px;height:80px;background:#ffffff}
#copy-link{display:block;position:absolute;left:20px;top:12px;width:100px;height:28px;background:#236144;color:white}
</style></head><body><template id=seed><a id=copy-link href=#destination on-click=ordinary onclick="throw 'inert handler ran'">Imported link</a></template>
<button id=copy type=button>Import template</button><div id=mount></div><div id=destination></div>
<script>
var input=document.getElementById('seed').content, sourceView=input.children, source=Array.from(sourceView)[0], mount=document.getElementById('mount'), mountView=mount.children;
if(input.childElementCount!==1||input.lastElementChild!==source||!source||source.nodeName!=='A')throw 'real inert ParentNode view';
var sourceNames=source.getAttributeNames();if(sourceNames.join(',')!=='id,href,on-click,onclick'||!source.hasAttribute('onclick'))throw 'ordered inert attribute preparation';
source.removeAttribute('onclick');if(source.hasAttribute('onclick')||source.getAttribute('on-click')!=='ordinary')throw 'actual handler removal and ordinary data';
document.getElementById('copy').onclick=function(event){
    if(!event.isTrusted)throw 'actual pointer activation';
    var fragment=document.importNode(input,true), importedView=fragment.children, imported=Array.from(importedView)[0], savedChildren=imported.children;
    if(fragment.childElementCount!==1||fragment.lastElementChild!==imported)throw 'real imported ParentNode view';
    if(imported.getAttribute('on-click')!=='ordinary'||imported.getAttribute('onclick')!==null)throw 'real prepared attribute transfer';
    if(imported===source||source.ownerDocument===document||imported.ownerDocument!==document||imported.isConnected)
        throw 'real import ownership';
    mount.appendChild(fragment);
    if(mount.firstChild!==imported||!imported.isConnected||source.isConnected||input.firstChild!==source)
        throw 'real insertion kept source inert';
    if(fragment.childElementCount!==0||fragment.firstElementChild!==null||fragment.lastElementChild!==null||mount.childElementCount!==1||mount.firstElementChild!==imported||mount.lastElementChild!==imported||input.firstElementChild!==source)throw 'live ParentNode insertion';
    if(fragment.children!==importedView||importedView.length!==0||mount.children!==mountView||Array.from(mountView)[0]!==imported||input.children!==sourceView||sourceView[0]!==source||imported.children!==savedChildren)throw 'real live HTMLCollection import and insertion';
    document.title='Template imported';
};
</script></body></html>"#).unwrap();
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    clean(&app);
    let original = id(&app, "copy-link");
    let location = app.location.clone();
    assert!(app.page.border_rects[original].is_none());
    assert_eq!(app.page.link_node_at(120, 125), None);
    let before = app.draw();
    let sample = (app.content_top() + 134 - app.scroll) as usize * before.width + 153;
    assert_eq!(before.pixels[sample], 0xffffff);
    click(&mut app, "copy");
    assert_eq!(app.document.title, "Template imported");
    let imported = app.document.nodes[id(&app, "mount")].children[0];
    assert_ne!(imported, original);
    let rect = app.page.border_rects[imported].unwrap();
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (60, 112, 100, 28));
    assert_eq!(app.page.link_node_at(120, 125), Some(imported));
    assert_eq!(app.page.link_at(120, 125), Some("#destination"));
    assert_eq!(app.page.link_node_at(160, 125), None);
    let after = app.draw();
    assert_eq!(after.pixels[sample], 0x236144);
    assert!(app.page.border_rects[original].is_none());
    assert_eq!(app.location, location);
    clean(&app);
}
