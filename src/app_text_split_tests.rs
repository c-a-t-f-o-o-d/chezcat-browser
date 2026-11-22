//! Actual App XML record binding through imported HTML data.
use super::*;
#[test]
fn text_split_actual_app_original_text_binding_tail_ranges_records_pixels_editor_and_link(){
    struct Fixture(PathBuf);
    impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let file=Fixture(std::env::temp_dir().join(format!("chezcat-text-split-{}.html",std::process::id())));
    std::fs::write(&file.0,r#"<html><head><style>body{margin:0;background:white}#field{position:absolute;left:10px;top:10px;width:140px;height:30px}#run{position:absolute;left:10px;top:65px;width:140px;height:30px}#bound{position:absolute;left:10px;top:120px;display:block;width:220px;height:30px;background:#eeeeee;color:#000000}#go{position:absolute;left:260px;top:10px;display:block;width:100px;height:30px;background:#2040e0}</style></head><body><input id=field value=seed><button id=run type=button>Update units</button><span id=bound>Native split active</span><a id=go href=#end>Continue</a><div id=end></div><script>
var root=document.getElementById('bound'),text=root.firstChild,list=root.childNodes,binding={held:text},tail=null,range=document.createRange(),gap=document.createRange();text.bindingState=binding;range.setStart(text,2);range.setEnd(text,15);gap.setStart(root,1);gap.collapse(true);var observer=new MutationObserver(function(records){if(records.length!==2||records[0].type!=='childList'||records[0].target!==root||records[0].addedNodes[0]!==tail||records[0].previousSibling!==text||records[0].nextSibling!==null||records[1].type!=='characterData'||records[1].target!==text||records[1].oldValue!=='Native split active'||root.firstChild!==text||root.childNodes!==list||list.length!==2||list[1]!==tail||text.nextSibling!==tail||tail.previousSibling!==text||tail.ownerDocument!==document||text.bindingState!==binding||binding.held!==text||tail.bindingState!==undefined||text.data!=='Native '||tail.data!=='split active'||text.wholeText!=='Native split active'||tail.wholeText!=='Native split active'||range.startContainer!==text||range.startOffset!==2||range.endContainer!==tail||range.endOffset!==8||gap.startOffset!==2||gap.endOffset!==2)throw 'actual split identities bindings live Range and ordered records';root.style.backgroundColor='#123456';document.title=text.wholeText;});observer.observe(root,{subtree:true,childList:true,characterData:true,characterDataOldValue:true});document.getElementById('run').onclick=function(event){if(!event.isTrusted)throw 'native trusted callback';Promise.resolve().then(function(){tail=text.splitText(7);});};document.title='Native split ready';
</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(480,320);app.set_javascript_enabled(true);app.open(&file.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    assert_eq!(app.document.title,"Native split ready","{}",app.scripting.as_ref().unwrap().report());
    let id=|app:&App,name:&str|(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap();
    let field=id(&app,"field");let run=id(&app,"run");let go=id(&app,"go");
    let click=|app:&mut App,node|{let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();let (x,y)=(rect.x+10,app.content_top()+rect.y+10-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);};
    click(&mut app,field);app.paste("edited");
    let value=&app.forms.controls[&field].value;let editing=(value.text(),value.cursor,value.anchor,value.offset);
    let initial_bound=id(&app,"bound");let initial_rect=app.page.border_rects[initial_bound].unwrap();
    let black_mask=|app:&mut App,rect:crate::layout::BorderRect|{let canvas=app.draw();let mut points=Vec::new();for dy in 0..rect.height{for dx in 0..rect.width{let x=rect.x+dx;let y=app.content_top()+rect.y+dy-app.scroll;if x>=0&&y>=0&&(x as usize)<canvas.width&&(y as usize)<canvas.height&&canvas.pixels[y as usize*canvas.width+x as usize]&0xffffff==0{points.push((dx,dy));}}}points};
    let before_glyphs=black_mask(&mut app,initial_rect);assert!(!before_glyphs.is_empty(),"real initial glyph mask");
    click(&mut app,run);
    assert_eq!(app.document.title,"Native split active","{}",app.scripting.as_ref().unwrap().report());
    let bound=id(&app,"bound");let rect=app.page.border_rects[bound].unwrap();
    let label=app.page.runs.iter().filter(|run|run.x>=rect.x&&run.x<rect.x+rect.width&&run.y>=rect.y&&run.y<rect.y+rect.height).map(|run|run.text.as_str()).collect::<Vec<_>>().join(" ");
    assert_eq!(label.split_whitespace().collect::<Vec<_>>().join(" "),"Native split active","complete imported label in its real box");
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

