//! Actual App raw-unit readback, display projection and native neighboring state.
use super::*;
#[test]
fn native_dom_string_actual_app_jobs_units_range_pixels_editor_and_link_are_distinct(){
    struct Fixture(PathBuf);
    impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let file=Fixture(std::env::temp_dir().join(format!("chezcat-native-units-{}.html",std::process::id())));
    std::fs::write(&file.0,r#"<html><head><style>body{margin:0;background:white}#field{position:absolute;left:10px;top:10px;width:140px;height:30px}#run{position:absolute;left:10px;top:65px;width:140px;height:30px}#bound{position:absolute;left:10px;top:120px;display:block;width:220px;height:30px;background:#eeeeee}#go{position:absolute;left:260px;top:10px;display:block;width:100px;height:30px;background:#2040e0}</style></head><body><input id=field value=seed><button id=run type=button>Update units</button><span id=bound>seed</span><a id=go href=#end>Continue</a><div id=end></div><script>
var root=document.getElementById('bound'),text=root.firstChild,range=new Range();range.selectNodeContents(text);
var observer=new MutationObserver(function(records){if(records.length!==2||records[0].target!==text||records[0].oldValue!=='seed'||records[1].oldValue.charCodeAt(0)!==0xd800||text.data.length!==3||text.data.charCodeAt(0)!==0xdfff||text.data.charCodeAt(2)!==0xd800||root.textContent!==text.data||root.innerHTML!==text.data||range.startContainer!==text||range.startOffset||range.endOffset)throw 'actual code units and old values';root.style.backgroundColor='#123456';document.title='Native units active';});observer.observe(text,{characterData:true,characterDataOldValue:true});
document.getElementById('run').onclick=function(e){if(!e.isTrusted)throw 'real native callback';Promise.resolve().then(function(){text.data='\ud800A\udfff';text.nodeValue='\udfffB\ud800';});};document.title='Native units ready';
</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(480,320);app.set_javascript_enabled(true);app.open(&file.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    assert_eq!(app.document.title,"Native units ready","{}",app.scripting.as_ref().unwrap().report());
    let id=|app:&App,name:&str|(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap();
    let field=id(&app,"field");let run=id(&app,"run");let go=id(&app,"go");
    let click=|app:&mut App,node|{let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();let (x,y)=(rect.x+10,app.content_top()+rect.y+10-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);};
    click(&mut app,field);app.paste("edited");
    let value=&app.forms.controls[&field].value;let editing=(value.text(),value.cursor,value.anchor,value.offset);
    click(&mut app,run);
    assert_eq!(app.document.title,"Native units active","{}",app.scripting.as_ref().unwrap().report());
    assert!(app.page.runs.iter().any(|run|run.text.contains('\u{fffd}')&&run.text.contains('B')),"actual scalar display projection");
    let canvas=app.draw();assert_eq!(canvas.pixels[(app.content_top()+140-app.scroll) as usize*canvas.width+200]&0xffffff,0x123456);
    let value=&app.forms.controls[&field].value;assert_eq!((value.text(),value.cursor,value.anchor,value.offset),editing);
    assert_eq!(app.page.link_node_at(280,20),Some(go));
    let y=app.content_top()+20-app.scroll;let (_,hit)=app.primary_anchor_down(280,y,false);
    let (_,default)=app.release_anchor_pointer(hit,280,y,false,crate::scripting::AnchorMouseInput::default());
    let Some(crate::app::anchor::AnchorDefault::Link(target,_))=default else {panic!("real link")};
    app.open_link_target(target);assert_eq!(app.location.fragment,"end");
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
}

#[test]
fn native_dom_string_attributes_actual_app_alias_jobs_pixels_editor_and_link_are_real(){
    struct Fixture(PathBuf);
    impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let file=Fixture(std::env::temp_dir().join(format!("chezcat-native-attr-units-{}.html",std::process::id())));
    std::fs::write(&file.0,r#"<html><head><style>body{margin:0;background:white}#field{position:absolute;left:10px;top:10px;width:140px;height:30px}#run{position:absolute;left:10px;top:65px;width:140px;height:30px}#bound{position:absolute;left:10px;top:120px;display:block;width:220px;height:30px;background:#eeeeee}#go{position:absolute;left:260px;top:10px;display:block;width:100px;height:30px;background:#2040e0}</style></head><body><input id=field value=seed><button id=run type=button>Update units</button><span id=bound>seed</span><a id=go href=#end>Continue</a><div id=end></div><script>
var root=document.getElementById('bound'),text=root.firstChild,range=new Range();range.selectNodeContents(text);
root.setAttribute('data-native','seed');var saved=root.getAttributeNode('data-native');var observer=new MutationObserver(function(records){if(records.length!==2||records[0].target!==root||records[0].oldValue!=='seed'||records[1].oldValue.charCodeAt(0)!==0xd800||saved.value.length!==3||saved.value.charCodeAt(0)!==0xdfff||saved.value.charCodeAt(2)!==0xd800||root.getAttribute('data-native')!==saved.value||root.matches('[data-native=\"�B�\"]'))throw 'actual Attr code units and old values';if(root.outerHTML!=='<span id="bound" data-native="'+saved.value+'">seed</span>')throw 'serialized Attr identity';text.data=saved.value;root.style.backgroundColor='#123456';document.title='Native Attr units active';});observer.observe(root,{attributes:true,attributeOldValue:true,attributeFilter:['data-native']});
document.getElementById('run').onclick=function(e){if(!e.isTrusted)throw 'real native callback';Promise.resolve().then(function(){root.setAttribute('data-native','\ud800A\udfff');saved.value='\udfffB\ud800';});};document.title='Native Attr units ready';
</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(480,320);app.set_javascript_enabled(true);app.open(&file.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    assert_eq!(app.document.title,"Native Attr units ready","{}",app.scripting.as_ref().unwrap().report());
    let id=|app:&App,name:&str|(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap();
    let field=id(&app,"field");let run=id(&app,"run");let go=id(&app,"go");
    let click=|app:&mut App,node|{let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();let (x,y)=(rect.x+10,app.content_top()+rect.y+10-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);};
    click(&mut app,field);app.paste("edited");
    let value=&app.forms.controls[&field].value;let editing=(value.text(),value.cursor,value.anchor,value.offset);
    click(&mut app,run);
    assert_eq!(app.document.title,"Native Attr units active","{}",app.scripting.as_ref().unwrap().report());
    assert!(app.page.runs.iter().any(|run|run.text.contains('\u{fffd}')&&run.text.contains('B')),"actual scalar display projection");
    let canvas=app.draw();assert_eq!(canvas.pixels[(app.content_top()+140-app.scroll) as usize*canvas.width+200]&0xffffff,0x123456);
    let value=&app.forms.controls[&field].value;assert_eq!((value.text(),value.cursor,value.anchor,value.offset),editing);
    assert_eq!(app.page.link_node_at(280,20),Some(go));
    let y=app.content_top()+20-app.scroll;let (_,hit)=app.primary_anchor_down(280,y,false);
    let (_,default)=app.release_anchor_pointer(hit,280,y,false,crate::scripting::AnchorMouseInput::default());
    let Some(crate::app::anchor::AnchorDefault::Link(target,_))=default else {panic!("real link")};
    app.open_link_target(target);assert_eq!(app.location.fragment,"end");
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
}
