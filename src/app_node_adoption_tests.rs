//! Actual App XML record binding through imported HTML data.
use super::*;
#[test]
fn node_adoption_actual_app_same_native_subtree_owner_roundtrip_jobs_pixels_editor_and_link(){
    struct Fixture(PathBuf);
    impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let file=Fixture(std::env::temp_dir().join(format!("chezcat-node-adoption-{}.html",std::process::id())));
    std::fs::write(&file.0,r#"<html><head><style>body{margin:0;background:white}#field{position:absolute;left:10px;top:10px;width:140px;height:30px}#run{position:absolute;left:10px;top:65px;width:140px;height:30px}#bound{position:absolute;left:10px;top:120px;display:block;width:220px;height:30px;background:#eeeeee}#go{position:absolute;left:260px;top:10px;display:block;width:100px;height:30px;background:#2040e0}</style></head><body><input id=field value=seed><button id=run type=button>Update units</button><span id=bound><span id=payload data-note=held>seed</span></span><a id=go href=#end>Continue</a><div id=end></div><script>
var root=document.getElementById('bound'),copied=root.firstChild,xml=null,list=copied.childNodes,attr=copied.getAttributeNode('data-note'),binding={held:copied};copied.bindingState=binding;
var observer=new MutationObserver(function(records){if(records.length!==2||records[0].target!==root||records[0].removedNodes[0]!==copied||records[1].target!==root||records[1].addedNodes[0]!==copied||copied.ownerDocument!==document||copied.parentNode!==root||copied.childNodes!==list||copied.getAttributeNode('data-note')!==attr||attr.ownerDocument!==document||copied.bindingState!==binding||binding.held!==copied||copied.textContent!=='Native adoption active'||!(xml instanceof XMLDocument)||xml.defaultView!==null||xml.documentElement.ownerDocument!==xml)throw 'actual adoption binding and native identities';root.style.backgroundColor='#123456';document.title='Native node adoption active';});observer.observe(root,{childList:true});
document.getElementById('run').onclick=function(event){if(!event.isTrusted)throw 'native trusted callback';Promise.resolve().then(function(){xml=document.implementation.createDocument('urn:original','p:Mixed');if(xml.adoptNode(copied)!==copied||copied.ownerDocument!==xml||attr.ownerDocument!==xml||copied.parentNode!==null||copied.tagName!=='span'||copied.childNodes!==list||document.getElementById('payload')!==null||root.firstChild!==null)throw 'actual native export';copied.firstChild.data='Native adoption active';if(list[0].data!=='Native adoption active')throw 'saved native list follows adopted data';if(document.adoptNode(copied)!==copied||attr.ownerDocument!==document||copied.tagName!=='SPAN')throw 'real owner return';var fragment=xml.createDocumentFragment();document.adoptNode(fragment);fragment.appendChild(copied);root.appendChild(fragment);if(fragment.childNodes.length||root.firstChild!==copied||copied.childNodes!==list)throw 'same identity native promotion';});};document.title='Native node adoption ready';
</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(480,320);app.set_javascript_enabled(true);app.open(&file.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    assert_eq!(app.document.title,"Native node adoption ready","{}",app.scripting.as_ref().unwrap().report());
    let id=|app:&App,name:&str|(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap();
    let field=id(&app,"field");let run=id(&app,"run");let go=id(&app,"go");
    let click=|app:&mut App,node|{let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();let (x,y)=(rect.x+10,app.content_top()+rect.y+10-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);};
    click(&mut app,field);app.paste("edited");
    let value=&app.forms.controls[&field].value;let editing=(value.text(),value.cursor,value.anchor,value.offset);
    click(&mut app,run);
    assert_eq!(app.document.title,"Native node adoption active","{}",app.scripting.as_ref().unwrap().report());
    let bound=id(&app,"bound");let rect=app.page.border_rects[bound].unwrap();
    let label=app.page.runs.iter().filter(|run|run.x>=rect.x&&run.x<rect.x+rect.width&&run.y>=rect.y&&run.y<rect.y+rect.height).map(|run|run.text.as_str()).collect::<Vec<_>>().join(" ");
    assert_eq!(label.split_whitespace().collect::<Vec<_>>().join(" "),"Native adoption active","complete imported label in its real box");
    let canvas=app.draw();assert_eq!(canvas.pixels[(app.content_top()+140-app.scroll) as usize*canvas.width+200]&0xffffff,0x123456);
    let value=&app.forms.controls[&field].value;assert_eq!((value.text(),value.cursor,value.anchor,value.offset),editing);
    assert_eq!(app.page.link_node_at(280,20),Some(go));
    let y=app.content_top()+20-app.scroll;let (_,hit)=app.primary_anchor_down(280,y,false);
    let (_,default)=app.release_anchor_pointer(hit,280,y,false,crate::scripting::AnchorMouseInput::default());
    let Some(crate::app::anchor::AnchorDefault::Link(target,_))=default else {panic!("real link")};
    app.open_link_target(target);assert_eq!(app.location.fragment,"end");
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
}

