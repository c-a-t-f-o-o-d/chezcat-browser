//! Actual native App document records, nonrendering doctypes and live controls.
use super::*;
#[test]
fn native_document_records_actual_app_three_modes_no_doctype_box_jobs_pixels_editor_and_link(){
    struct Fixture(PathBuf);impl Drop for Fixture{fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    for (index,(declaration,mode)) in [
        ("",crate::html::DocumentMode::Quirks),
        ("<!doctype html>",crate::html::DocumentMode::NoQuirks),
        ("<!doctype html PUBLIC '-//W3C//DTD XHTML 1.0 Transitional//EN' 'native-system'>",crate::html::DocumentMode::LimitedQuirks),
    ].into_iter().enumerate(){
        let file=Fixture(std::env::temp_dir().join(format!("chezcat-native-document-records-{}-{index}.html",std::process::id())));
        let source=r#"<html><head><style>body{margin:0;background:white}#field{position:absolute;left:10px;top:10px;width:140px;height:30px}#run{position:absolute;left:10px;top:65px;width:140px;height:30px}#bound{position:absolute;left:10px;top:120px;display:block;width:220px;height:30px;background:#eeeeee}#go{position:absolute;left:260px;top:10px;display:block;width:100px;height:30px;background:#2040e0}</style></head><body><input id=field value=seed><button id=run type=button>Use records</button><span id=bound>seed</span><a id=go href=#end>Continue</a><div id=end></div><script>
var root=document.getElementById('bound'),initial=document.doctype,expected='_EXPECTED_MODE_';if(document.compatMode!==expected)throw 'actual document mode';
var matches=document.getElementsByClassName('nativeMark');
root.setAttribute('class','NATIVEMARK');
if(matches.length!==(expected==='BackCompat'?1:0))throw 'live class mode';
root.setAttribute('class','nativeMark');
if(matches.length!==1||matches[0]!==root)throw 'same live class member';
if(initial!==null&&(initial.nodeType!==10||initial.name!=='html'||initial.ownerDocument!==document||initial.parentNode!==document))throw 'actual loaded doctype';
var observer=new MutationObserver(function(records){if(records.length!==1||records[0].target!==root||root.textContent!=='Native records active'||document.doctype!==initial||document.compatMode!==expected)throw 'actual record binding';root.style.backgroundColor='#123456';document.title='Native records active';});observer.observe(root,{childList:true});
document.getElementById('run').onclick=function(event){if(!event.isTrusted)throw 'native trusted callback';Promise.resolve().then(function(){var owner=document.implementation.createHTMLDocument('owner'),made=owner.implementation.createDocumentType('native','\ud800','\udfff'),copy=document.importNode(made,true);if(made.ownerDocument!==owner||copy.ownerDocument!==document||copy.publicId.charCodeAt(0)!==0xd800||copy.systemId.charCodeAt(0)!==0xdfff||copy.parentNode!==null||copy.isConnected)throw 'native factory/import records';root.textContent='Native records active';});};document.title='Native records ready';
</script></body></html>"#.replace("_EXPECTED_MODE_",mode.compat_mode());
        std::fs::write(&file.0,format!("{declaration}{source}")).unwrap();
        let mut app=App::new();app.resize(480,320);app.set_javascript_enabled(true);app.open(&file.0);
        assert!(!app.failed&&!app.is_loading(),"{}",app.status);
        assert_eq!(app.document.mode,mode);assert_eq!(app.document.title,"Native records ready","{}",app.scripting.as_ref().unwrap().report());
        let doctypes:Vec<_>=app.document.nodes.iter().enumerate().filter_map(|(node,item)|matches!(item.kind,crate::html::NodeKind::DocumentType(_)).then_some(node)).collect();
        assert_eq!(doctypes.len(),usize::from(index!=0));
        for &node in &doctypes{assert!(app.page.client_boxes[node].is_none()&&app.page.border_rects[node].is_none());}
        assert!(!app.page.runs.iter().any(|run|run.text.contains("native-system")||run.text.contains("-//W3C//DTD")));
        let id=|app:&App,name:&str|(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap();
        let(field,run,go)=(id(&app,"field"),id(&app,"run"),id(&app,"go"));
        let click=|app:&mut App,node|{let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();let(x,y)=(rect.x+10,app.content_top()+rect.y+10-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);};
        click(&mut app,field);app.paste("edited");let value=&app.forms.controls[&field].value;let editing=(value.text(),value.cursor,value.anchor,value.offset);
        click(&mut app,run);assert_eq!(app.document.title,"Native records active","{}",app.scripting.as_ref().unwrap().report());
        let canvas=app.draw();assert_eq!(canvas.pixels[(app.content_top()+140-app.scroll) as usize*canvas.width+200]&0xffffff,0x123456);
        let value=&app.forms.controls[&field].value;assert_eq!((value.text(),value.cursor,value.anchor,value.offset),editing);
        for &node in &doctypes{assert!(app.page.client_boxes[node].is_none()&&app.page.border_rects[node].is_none());}
        assert_eq!(app.page.link_node_at(280,20),Some(go));let y=app.content_top()+20-app.scroll;let(_,hit)=app.primary_anchor_down(280,y,false);
        let(_,default)=app.release_anchor_pointer(hit,280,y,false,crate::scripting::AnchorMouseInput::default());
        let Some(crate::app::anchor::AnchorDefault::Link(target,_))=default else{panic!("real link")};app.open_link_target(target);assert_eq!(app.location.fragment,"end");
        assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
    }
}
