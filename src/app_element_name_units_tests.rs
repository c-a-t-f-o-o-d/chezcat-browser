//! Actual App XML record binding through imported HTML data.
use super::*;
#[test]
fn element_name_units_actual_app_same_node_names_queries_css_pixels_editor_and_link(){
    struct Fixture(PathBuf);
    impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let file=Fixture(std::env::temp_dir().join(format!("chezcat-element-name-units-{}.html",std::process::id())));
    std::fs::write(&file.0,r#"<html><head><style>body{margin:0;background:white}#field{position:absolute;left:10px;top:10px;width:140px;height:30px}#run{position:absolute;left:10px;top:65px;width:140px;height:30px}#bound{position:absolute;left:10px;top:120px;display:block;width:220px;height:30px;background:#eeeeee}#go{position:absolute;left:260px;top:10px;display:block;width:100px;height:30px;background:#2040e0}a�{background:#ff0000!important}</style></head><body><input id=field value=seed><button id=run type=button>Update units</button><span id=bound>seed</span><a id=go href=#end>Continue</a><div id=end></div><script>
var root=document.getElementById('bound'),owner=null,node=null,text=null,list=null,attr=null,binding=null,oldNames=null;root.textContent='';var observer=new MutationObserver(function(records){if(records.length!==1||records[0].target!==root||records[0].addedNodes[0]!==node||node.ownerDocument!==document||node.parentNode!==root||node.localName!=='a\ud800'||node.tagName!=='A\ud800'||node.nodeName!=='A\ud800'||node.prefix!==null||node.namespaceURI!=='http://www.w3.org/1999/xhtml'||node.firstChild!==text||node.childNodes!==list||node.getAttributeNode('id')!==attr||attr.ownerDocument!==document||node.bindingState!==binding||binding.held!==node||oldNames.length||root.getElementsByTagName('A\ud800')[0]!==node||root.getElementsByTagName('A\ufffd').length||root.querySelector('a\ufffd')!==null)throw 'actual exact Element name promotion and query identity';document.title='Native Element units active';});observer.observe(root,{childList:true});document.getElementById('run').onclick=function(event){if(!event.isTrusted)throw 'native trusted callback';Promise.resolve().then(function(){owner=document.implementation.createHTMLDocument('owner');node=owner.createElement('A\ud800');node.setAttribute('id','actual');node.setAttribute('style','display:block;width:220px;height:30px;background:#123456');text=owner.createTextNode('Native Element units active');node.appendChild(text);owner.body.appendChild(node);oldNames=owner.body.getElementsByTagName('A\ud800');if(oldNames.length!==1||oldNames[0]!==node)throw 'native source QName filter';list=node.childNodes;attr=node.getAttributeNode('id');binding={held:node};node.bindingState=binding;if(root.appendChild(node)!==node)throw 'same source identity';});};document.title='Native Element units ready';
</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(480,320);app.set_javascript_enabled(true);app.open(&file.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    assert_eq!(app.document.title,"Native Element units ready","{}",app.scripting.as_ref().unwrap().report());
    let id=|app:&App,name:&str|(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap();
    let field=id(&app,"field");let run=id(&app,"run");let go=id(&app,"go");
    let click=|app:&mut App,node|{let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();let (x,y)=(rect.x+10,app.content_top()+rect.y+10-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);};
    click(&mut app,field);app.paste("edited");
    let value=&app.forms.controls[&field].value;let editing=(value.text(),value.cursor,value.anchor,value.offset);
    click(&mut app,run);
    assert_eq!(app.document.title,"Native Element units active","{}",app.scripting.as_ref().unwrap().report());
    let bound=id(&app,"bound");let rect=app.page.border_rects[bound].unwrap();
    let label=app.page.runs.iter().filter(|run|run.x>=rect.x&&run.x<rect.x+rect.width&&run.y>=rect.y&&run.y<rect.y+rect.height).map(|run|run.text.as_str()).collect::<Vec<_>>().join(" ");
    assert_eq!(label.split_whitespace().collect::<Vec<_>>().join(" "),"Native Element units active","complete imported label in its real box");
    let canvas=app.draw();assert_eq!(canvas.pixels[(app.content_top()+140-app.scroll) as usize*canvas.width+200]&0xffffff,0x123456);
    let value=&app.forms.controls[&field].value;assert_eq!((value.text(),value.cursor,value.anchor,value.offset),editing);
    assert_eq!(app.page.link_node_at(280,20),Some(go));
    let y=app.content_top()+20-app.scroll;let (_,hit)=app.primary_anchor_down(280,y,false);
    let (_,default)=app.release_anchor_pointer(hit,280,y,false,crate::scripting::AnchorMouseInput::default());
    let Some(crate::app::anchor::AnchorDefault::Link(target,_))=default else {panic!("real link")};
    app.open_link_target(target);assert_eq!(app.location.fragment,"end");
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
}

