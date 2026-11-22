//! Actual App XML record binding through imported HTML data.
use super::*;
#[test]
fn shared_normalize_actual_app_kept_binding_removed_data_ranges_records_pixels_editor_and_link(){
    struct Fixture(PathBuf);
    impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let file=Fixture(std::env::temp_dir().join(format!("chezcat-shared-normalize-{}.html",std::process::id())));
    std::fs::write(&file.0,r#"<html><head><style>body{margin:0;background:white}#field{position:absolute;left:10px;top:10px;width:140px;height:30px}#run{position:absolute;left:10px;top:65px;width:140px;height:30px}#bound{position:absolute;left:10px;top:120px;display:block;width:220px;height:30px;background:#eeeeee;color:#000000}#go{position:absolute;left:260px;top:10px;display:block;width:100px;height:30px;background:#2040e0}</style></head><body><input id=field value=seed><button id=run type=button>Update units</button><span id=bound>Native normalize active</span><a id=go href=#end>Continue</a><div id=end></div><script>
var root=document.getElementById('bound');root.textContent='';var empty=document.createTextNode(''),first=document.createTextNode('Native '),second=document.createTextNode('normalize active'),list=root.childNodes,binding={held:first},range=document.createRange(),gap=document.createRange();first.bindingState=binding;root.appendChild(empty);root.appendChild(first);root.appendChild(second);range.setStart(second,3);range.setEnd(second,8);gap.setStart(root,2);gap.collapse(true);var observer=new MutationObserver(function(records){if(records.length!==3||records[0].type!=='childList'||records[0].removedNodes[0]!==empty||records[0].previousSibling!==null||records[0].nextSibling!==first||records[1].type!=='characterData'||records[1].target!==first||records[1].oldValue!=='Native '||records[2].type!=='childList'||records[2].removedNodes[0]!==second||records[2].previousSibling!==first||records[2].nextSibling!==null||root.firstChild!==first||root.childNodes!==list||list.length!==1||empty.parentNode!==null||second.parentNode!==null||second.data!=='normalize active'||first.data!=='Native normalize active'||first.bindingState!==binding||binding.held!==first||range.startContainer!==first||range.endContainer!==first||range.startOffset!==10||range.endOffset!==15||gap.startContainer!==first||gap.endContainer!==first||gap.startOffset!==7||gap.endOffset!==7)throw 'actual shared normalization identities data Range and sequential records';root.style.backgroundColor='#123456';document.title=first.wholeText;});observer.observe(root,{subtree:true,childList:true,characterData:true,characterDataOldValue:true});document.getElementById('run').onclick=function(event){if(!event.isTrusted)throw 'native trusted callback';Promise.resolve().then(function(){root.normalize();});};document.title='Native normalize ready';
</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(480,320);app.set_javascript_enabled(true);app.open(&file.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    assert_eq!(app.document.title,"Native normalize ready","{}",app.scripting.as_ref().unwrap().report());
    let id=|app:&App,name:&str|(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap();
    let field=id(&app,"field");let run=id(&app,"run");let go=id(&app,"go");
    let click=|app:&mut App,node|{let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();let (x,y)=(rect.x+10,app.content_top()+rect.y+10-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);};
    click(&mut app,field);app.paste("edited");
    let value=&app.forms.controls[&field].value;let editing=(value.text(),value.cursor,value.anchor,value.offset);
    let initial_bound=id(&app,"bound");let initial_rect=app.page.border_rects[initial_bound].unwrap();
    let black_mask=|app:&mut App,rect:crate::layout::BorderRect|{let canvas=app.draw();let mut points=Vec::new();for dy in 0..rect.height{for dx in 0..rect.width{let x=rect.x+dx;let y=app.content_top()+rect.y+dy-app.scroll;if x>=0&&y>=0&&(x as usize)<canvas.width&&(y as usize)<canvas.height&&canvas.pixels[y as usize*canvas.width+x as usize]&0xffffff==0{points.push((dx,dy));}}}points};
    let before_glyphs=black_mask(&mut app,initial_rect);assert!(!before_glyphs.is_empty(),"real initial glyph mask");
    click(&mut app,run);
    assert_eq!(app.document.title,"Native normalize active","{}",app.scripting.as_ref().unwrap().report());
    let bound=id(&app,"bound");let rect=app.page.border_rects[bound].unwrap();
    let label=app.page.runs.iter().filter(|run|run.x>=rect.x&&run.x<rect.x+rect.width&&run.y>=rect.y&&run.y<rect.y+rect.height).map(|run|run.text.as_str()).collect::<Vec<_>>().join(" ");
    assert_eq!(label.split_whitespace().collect::<Vec<_>>().join(" "),"Native normalize active","complete imported label in its real box");
    assert_eq!(black_mask(&mut app,rect),before_glyphs,"split preserves actual glyph positions across the new Text boundary");
    let canvas=app.draw();assert_eq!(canvas.pixels[(app.content_top()+140-app.scroll) as usize*canvas.width+200]&0xffffff,0x123456);
    let value=&app.forms.controls[&field].value;assert_eq!((value.text(),value.cursor,value.anchor,value.offset),editing);
    assert_eq!(app.page.link_node_at(280,20),Some(go));
    let y=app.content_top()+20-app.scroll;let (_,hit)=app.primary_anchor_down(280,y,false);
    let (_,default)=app.release_anchor_pointer(hit,280,y,false,crate::scripting::AnchorMouseInput::default());
    let Some(crate::app::anchor::AnchorDefault::Link(target,_))=default else {panic!("real link")};
    app.open_link_target(target);assert_eq!(app.location.fragment,"end");
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
}

