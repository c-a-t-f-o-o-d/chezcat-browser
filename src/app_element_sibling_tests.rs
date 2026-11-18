//! Real sibling reads after trusted App moves, with literal software input.
use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};

fn node(app:&App,id:&str)->usize {
    (0..app.document.nodes.len()).find(|&n|app.document.attr(n,"id")==Some(id)).unwrap()
}
fn clean(app:&App) {
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    let page=app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(),0,"{}",page.report());
    assert!(!page.has_navigation_request());
}
fn click(app:&mut App,id:&str) {
    let target=node(app,id);
    let rect=app.page.controls.iter().find(|r|r.node==target).unwrap();
    let(x,page_y)=(rect.x+rect.width/2,rect.y+rect.height/2);
    assert_eq!(app.page.control_at(x,page_y),Some(target));
    let y=app.content_top()+page_y-app.scroll;
    assert!(matches!(app.pointer_down(x,y,false),PlatformAction::None));
    assert!(matches!(app.pointer_up(x,y),PlatformAction::None));
    clean(app);
}
fn editor(app:&App)->(String,usize,usize,usize,bool,Option<usize>) {
    let c=&app.forms.controls[&node(app,"neighbor")];
    (c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.owner)
}
fn pixels(app:&App,expected:&str)->Vec<u32> {
    let rect=app.page.border_rects[node(app,"out")].unwrap();
    assert_eq!((rect.x,rect.y,rect.width,rect.height),(20,100,300,40));
    let runs:Vec<_>=app.page.runs.iter().filter(|r|r.x>=20&&r.x<320&&r.y>=100&&r.y<140).collect();
    assert_eq!(runs.iter().map(|r|r.text.as_str()).collect::<Vec<_>>(),[expected]);
    assert!(runs.iter().all(|r|r.color==0x123456));
    let canvas=app.draw();let top=(app.content_top()+100-app.scroll) as usize;
    let result:Vec<_>=(top..top+40).flat_map(|y|canvas.pixels[y*canvas.width+20..y*canvas.width+320].iter().copied()).collect();
    assert!(result.iter().any(|&pixel|pixel==0x123456));
    assert_eq!(result.last(),Some(&0xffffff));result
}
fn stable_hits(app:&App) {
    let neighbor=node(app,"neighbor");let rect=app.page.controls.iter().find(|r|r.node==neighbor).unwrap();
    assert_eq!((rect.x,rect.y,rect.width,rect.height),(20,20,220,32));
    assert_eq!(app.page.control_at(130,36),Some(neighbor));
    assert_eq!(app.page.control_at(240,36),None);
    let link=node(app,"stable-link");let rect=app.page.border_rects[link].unwrap();
    assert_eq!((rect.x,rect.y,rect.width,rect.height),(20,180,100,24));
    for(x,y)in[(20,180),(70,192),(119,203)] {
        assert_eq!(app.page.link_node_at(x,y),Some(link));
        assert_eq!(app.page.link_at(x,y),Some("#destination"));
    }
    assert_eq!(app.page.link_node_at(19,192),None);assert_eq!(app.page.link_node_at(120,192),None);
}

#[test]
fn element_siblings_actual_app_trusted_moves_repaint_live_order_and_preserve_neighbor() {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    struct Fixture(PathBuf);impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let fixture=Fixture(std::env::temp_dir().join(format!("chezcat-element-siblings-app-{}-{}.html",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Sibling pending</title><style>
html,body{margin:0;padding:0;background:#ffffff}
#neighbor{display:block;position:absolute;left:20px;top:20px;width:220px;height:32px;margin:0}
#advance{display:block;position:absolute;left:280px;top:20px;width:180px;height:32px;margin:0}
#out{position:absolute;left:20px;top:100px;width:300px;height:40px;margin:0;padding:0;border:0;background:#ffffff;color:#123456;font-size:24px;line-height:32px}
#stable-link{display:block;position:absolute;left:20px;top:180px;width:100px;height:24px;background:#336699;color:white}
#box{position:absolute;left:20px;top:240px;width:400px;height:40px}
</style></head><body><form id=f><input id=neighbor name=neighbor value="neighbor seed"><button id=advance name=advance type=button value=payload>Move siblings</button></form>
<p id=out>Fallback</p><a id=stable-link href=#destination>Go</a><div id=destination></div><div id=box>lead<span id=a>Alpha</span>middle<span id=b>Bravo</span>tail</div><script>
var box=document.getElementById('box'),a=document.getElementById('a'),b=document.getElementById('b'),middle=a.nextSibling,marker=document.createComment('original sibling marker'),phase=0;
box.insertBefore(marker,b);
function show(first,second){
 if(first.nextElementSibling!==second||second.previousElementSibling!==first||first.previousElementSibling!==null||second.nextElementSibling!==null)throw 'live element identity';
 if(a.ownerDocument!==document||b.ownerDocument!==document||middle.ownerDocument!==document||marker.ownerDocument!==document||a.parentNode!==box||b.parentNode!==box||middle.parentNode!==box||marker.parentNode!==box)throw 'same physical document nodes';
 var label=first.textContent+'>'+second.textContent;document.getElementById('out').textContent=label;document.title='Sibling order '+label;
}
if(middle.nodeType!==3||marker.nodeType!==8||middle.nextSibling!==marker||marker.nextSibling!==b||middle.previousElementSibling!==a||marker.nextElementSibling!==b)throw 'physical CharacterData prefix';
show(a,b);
document.getElementById('advance').onclick=function(event){
 if(!event.isTrusted||event.currentTarget!==this||this!==document.getElementById('advance')||this.form!==document.getElementById('f'))throw 'trusted sibling move';
 if(phase===0){
  if(box.insertBefore(b,a)!==b||middle.previousElementSibling!==a||middle.nextElementSibling!==null||marker.previousElementSibling!==a||marker.nextElementSibling!==null||a.nextSibling!==middle||middle.nextSibling!==marker)throw 'move before keeps text and comment';
  phase=1;show(b,a);
 }else if(phase===1){
  if(box.appendChild(b)!==b||middle.previousElementSibling!==a||middle.nextElementSibling!==b||marker.previousElementSibling!==a||marker.nextElementSibling!==b||a.nextSibling!==middle||middle.nextSibling!==marker)throw 'append keeps text and comment';
  phase=2;show(a,b);
 }else throw 'unexpected third move';
};
</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(640,480);app.set_javascript_enabled(true);app.open(&fixture.0);clean(&app);
    assert_eq!(app.document.title,"Sibling order Alpha>Bravo");
    let raw=app.raw.clone();let location=app.location.clone();let history=(app.history_index,app.history.len());
    let identities=(node(&app,"a"),node(&app,"b"),node(&app,"box"),node(&app,"neighbor"));
    let children=app.document.nodes[identities.2].children.clone();assert_eq!(children.len(),6);
    let controls:Vec<_>=app.forms.controls.keys().copied().collect();stable_hits(&app);
    click(&mut app,"neighbor");assert!(matches!(app.key(0x41,true,false,false),PlatformAction::None));app.paste("neighbor kept");
    assert!(matches!(app.key(0x24,false,false,false),PlatformAction::None));
    for _ in 0..2 {assert!(matches!(app.key(0x27,false,false,false),PlatformAction::None));}
    for _ in 0..3 {assert!(matches!(app.key(0x27,false,true,false),PlatformAction::None));}
    let preserved=editor(&app);assert_eq!((preserved.0.as_str(),preserved.1,preserved.2),("neighbor kept",5,2));assert!(preserved.4);
    let initial=pixels(&app,"Alpha>Bravo");
    click(&mut app,"advance");assert_eq!(app.document.title,"Sibling order Bravo>Alpha");
    assert_ne!(pixels(&app,"Bravo>Alpha"),initial);assert_eq!(editor(&app),preserved);stable_hits(&app);
    assert_eq!(app.document.nodes[identities.2].children,[children[0],children[4],children[1],children[2],children[3],children[5]]);
    click(&mut app,"advance");assert_eq!(app.document.title,"Sibling order Alpha>Bravo");
    assert_eq!(pixels(&app,"Alpha>Bravo"),initial);
    assert_eq!(app.document.nodes[identities.2].children,[children[0],children[1],children[2],children[3],children[5],children[4]]);
    assert_eq!(editor(&app),preserved);assert_eq!(app.forms.text_default_value(node(&app,"neighbor")).unwrap(),"neighbor seed");
    assert_eq!(app.forms.controls.keys().copied().collect::<Vec<_>>(),controls);
    assert_eq!((node(&app,"a"),node(&app,"b"),node(&app,"box"),node(&app,"neighbor")),identities);
    assert_eq!(app.raw,raw);assert_eq!(app.location,location);assert_eq!((app.history_index,app.history.len()),history);stable_hits(&app);clean(&app);
}
