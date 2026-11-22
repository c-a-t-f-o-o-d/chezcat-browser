//! Actual App XML record binding through imported HTML data.
use super::*;
#[test]
fn inert_document_import_actual_app_main_source_target_owner_same_copy_pixels_editor_and_link(){
    struct Fixture(PathBuf);
    impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let file=Fixture(std::env::temp_dir().join(format!("chezcat-inert-document-import-{}.html",std::process::id())));
    std::fs::write(&file.0,r#"<html><head><style>body{margin:0;background:white}#field{position:absolute;left:10px;top:10px;width:140px;height:30px}#run{position:absolute;left:10px;top:65px;width:140px;height:30px}#bound{position:absolute;left:10px;top:120px;display:block;width:220px;height:30px;background:#eeeeee}#go{position:absolute;left:260px;top:10px;display:block;width:100px;height:30px;background:#2040e0}</style></head><body><input id=field value=seed><button id=run type=button>Update units</button><span id=bound>seed</span><a id=go href=#end>Continue</a><div id=end></div><span id=source style="display:none" data-note=held>original</span><script>
var root=document.getElementById('bound'),source=document.getElementById('source'),target=null,copy=null,text=null,list=null,attr=null,binding=null,original=source.getAttributeNode('data-note'),sourceWatch=new MutationObserver(function(){});root.textContent='';sourceWatch.observe(source,{attributes:true,childList:true,characterData:true,subtree:true});var observer=new MutationObserver(function(records){if(records.length!==1||records[0].target!==root||records[0].addedNodes[0]!==copy||copy.ownerDocument!==document||copy.parentNode!==root||copy.firstChild!==text||text.ownerDocument!==document||copy.childNodes!==list||copy.getAttributeNode('data-note')!==attr||attr===original||attr.ownerDocument!==document||copy.bindingState!==binding||binding.held!==copy||copy.tagName!=='SPAN'||copy.textContent!=='Native inert import active'||target.documentElement!==null||source.getAttribute('style')!=='display:none'||source.getAttribute('data-note')!=='held'||source.textContent!=='original'||sourceWatch.takeRecords().length)throw 'actual target owner import and same copy promotion without source mutation';root.style.backgroundColor='#123456';document.title='Native inert import active';});observer.observe(root,{childList:true});document.getElementById('run').onclick=function(event){if(!event.isTrusted)throw 'native trusted callback';Promise.resolve().then(function(){target=document.implementation.createDocument(null,'');copy=target.importNode(source,true);if(copy===source||copy.ownerDocument!==target||copy.tagName!=='span'||copy.firstChild.ownerDocument!==target)throw 'actual XML target owner and case';copy.removeAttribute('style');copy.setAttribute('id','copied');copy.firstChild.data='Native inert import active';list=copy.childNodes;text=copy.firstChild;attr=copy.getAttributeNode('data-note');binding={held:copy};copy.bindingState=binding;if(root.appendChild(copy)!==copy)throw 'same imported identity activated';});};document.title='Native inert import ready';
</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(480,320);app.set_javascript_enabled(true);app.open(&file.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    assert_eq!(app.document.title,"Native inert import ready","{}",app.scripting.as_ref().unwrap().report());
    let id=|app:&App,name:&str|(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap();
    let field=id(&app,"field");let run=id(&app,"run");let go=id(&app,"go");
    let click=|app:&mut App,node|{let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();let (x,y)=(rect.x+10,app.content_top()+rect.y+10-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);};
    click(&mut app,field);app.paste("edited");
    let value=&app.forms.controls[&field].value;let editing=(value.text(),value.cursor,value.anchor,value.offset);
    click(&mut app,run);
    assert_eq!(app.document.title,"Native inert import active","{}",app.scripting.as_ref().unwrap().report());
    let bound=id(&app,"bound");let rect=app.page.border_rects[bound].unwrap();
    let label=app.page.runs.iter().filter(|run|run.x>=rect.x&&run.x<rect.x+rect.width&&run.y>=rect.y&&run.y<rect.y+rect.height).map(|run|run.text.as_str()).collect::<Vec<_>>().join(" ");
    assert_eq!(label.split_whitespace().collect::<Vec<_>>().join(" "),"Native inert import active","complete imported label in its real box");
    let canvas=app.draw();assert_eq!(canvas.pixels[(app.content_top()+140-app.scroll) as usize*canvas.width+200]&0xffffff,0x123456);
    let value=&app.forms.controls[&field].value;assert_eq!((value.text(),value.cursor,value.anchor,value.offset),editing);
    assert_eq!(app.page.link_node_at(280,20),Some(go));
    let y=app.content_top()+20-app.scroll;let (_,hit)=app.primary_anchor_down(280,y,false);
    let (_,default)=app.release_anchor_pointer(hit,280,y,false,crate::scripting::AnchorMouseInput::default());
    let Some(crate::app::anchor::AnchorDefault::Link(target,_))=default else {panic!("real link")};
    app.open_link_target(target);assert_eq!(app.location.fragment,"end");
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
}

