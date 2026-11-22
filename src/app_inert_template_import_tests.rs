//! Actual App XML record binding through imported HTML data.
use super::*;
#[test]
fn inert_template_import_actual_app_target_contents_same_node_pixels_editor_and_link(){
    struct Fixture(PathBuf);
    impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let file=Fixture(std::env::temp_dir().join(format!("chezcat-inert-template-import-{}.html",std::process::id())));
    std::fs::write(&file.0,r#"<html><head><style>body{margin:0;background:white}#field{position:absolute;left:10px;top:10px;width:140px;height:30px}#run{position:absolute;left:10px;top:65px;width:140px;height:30px}#bound{position:absolute;left:10px;top:120px;display:block;width:220px;height:30px;background:#eeeeee}#go{position:absolute;left:260px;top:10px;display:block;width:100px;height:30px;background:#2040e0}</style></head><body><input id=field value=seed><button id=run type=button>Update units</button><span id=bound>seed</span><a id=go href=#end>Continue</a><div id=end></div><template id=source><span data-note=held>Native template import active</span><template><b>nested</b></template></template><script>
var root=document.getElementById('bound'),source=document.getElementById('source'),target=null,copy=null,owner=null,node=null,text=null,list=null,attr=null,binding=null;root.textContent='';var observer=new MutationObserver(function(records){if(records.length!==1||records[0].target!==root||records[0].addedNodes[0]!==node||copy.ownerDocument!==target||copy.content.ownerDocument!==owner||owner===target||owner.contentType!=='application/xml'||owner.createAttribute('MiXeD').name!=='MiXeD'||copy.content.childNodes.length!==1||copy.content.firstChild.content.ownerDocument!==owner||copy.content.firstChild.content.textContent!=='nested'||node.ownerDocument!==document||node.parentNode!==root||node.firstChild!==text||text.ownerDocument!==document||node.childNodes!==list||node.getAttributeNode('data-note')!==attr||attr.ownerDocument!==document||node.bindingState!==binding||binding.held!==node||source.content.childNodes.length!==2||source.content.firstChild.textContent!=='Native template import active')throw 'actual destination template owner and same content node activation';root.style.backgroundColor='#123456';document.title='Native template import active';});observer.observe(root,{childList:true});document.getElementById('run').onclick=function(event){if(!event.isTrusted)throw 'native trusted callback';Promise.resolve().then(function(){target=document.implementation.createDocument(null,'');copy=target.importNode(source,true);owner=copy.content.ownerDocument;node=copy.content.firstChild;text=node.firstChild;list=node.childNodes;attr=node.getAttributeNode('data-note');binding={held:node};node.bindingState=binding;if(node.ownerDocument!==owner||node.tagName!=='span'||owner===source.content.ownerDocument)throw 'real XML-kind imported content owner';if(root.appendChild(node)!==node)throw 'same template data node activated';});};document.title='Native template import ready';
</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(480,320);app.set_javascript_enabled(true);app.open(&file.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    assert_eq!(app.document.title,"Native template import ready","{}",app.scripting.as_ref().unwrap().report());
    let id=|app:&App,name:&str|(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap();
    let field=id(&app,"field");let run=id(&app,"run");let go=id(&app,"go");
    let click=|app:&mut App,node|{let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();let (x,y)=(rect.x+10,app.content_top()+rect.y+10-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);};
    click(&mut app,field);app.paste("edited");
    let value=&app.forms.controls[&field].value;let editing=(value.text(),value.cursor,value.anchor,value.offset);
    click(&mut app,run);
    assert_eq!(app.document.title,"Native template import active","{}",app.scripting.as_ref().unwrap().report());
    let bound=id(&app,"bound");let rect=app.page.border_rects[bound].unwrap();
    let label=app.page.runs.iter().filter(|run|run.x>=rect.x&&run.x<rect.x+rect.width&&run.y>=rect.y&&run.y<rect.y+rect.height).map(|run|run.text.as_str()).collect::<Vec<_>>().join(" ");
    assert_eq!(label.split_whitespace().collect::<Vec<_>>().join(" "),"Native template import active","complete imported label in its real box");
    let canvas=app.draw();assert_eq!(canvas.pixels[(app.content_top()+140-app.scroll) as usize*canvas.width+200]&0xffffff,0x123456);
    let value=&app.forms.controls[&field].value;assert_eq!((value.text(),value.cursor,value.anchor,value.offset),editing);
    assert_eq!(app.page.link_node_at(280,20),Some(go));
    let y=app.content_top()+20-app.scroll;let (_,hit)=app.primary_anchor_down(280,y,false);
    let (_,default)=app.release_anchor_pointer(hit,280,y,false,crate::scripting::AnchorMouseInput::default());
    let Some(crate::app::anchor::AnchorDefault::Link(target,_))=default else {panic!("real link")};
    app.open_link_target(target);assert_eq!(app.location.fragment,"end");
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
}

