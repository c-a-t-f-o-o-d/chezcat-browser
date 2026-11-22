//! Original SVG bindings retain owner metadata across staging and real paint.
use super::*;
#[test]
fn inert_foreign_attribute_actual_app_staging_refusal_recovery_jobs_paint_editor_and_link() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-inert-foreign-attributes-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,r##"<html><head><style>html,body{margin:0;background:#fff}#neighbor{position:absolute;left:10px;top:10px;width:160px;height:30px}#run{position:absolute;left:10px;top:70px;width:160px;height:30px}#bound{position:absolute;left:10px;top:120px;width:48px;height:48px}#go{position:absolute;display:block;left:260px;top:10px;width:100px;height:30px;background:#2040e0}</style></head><body><input id=neighbor value=seed><button id=run type=button>Update binding</button><div id=mount></div><a id=go href=#destination>Continue</a><div id=destination></div><script>
var template=document.createElement('template');template.innerHTML='<svg id="bound" width="24" height="24" viewBox="0 0 24 24"><rect width="24" height="24" fill="red"></rect></svg>';var staged=template.content,root=staged.firstChild,node=root.firstChild,fill=node.getAttributeNode('fill'),owner=node.ownerDocument;node.bindingState={held:node};var observer=new MutationObserver(function(records){if(records.length!==3||records[0].attributeName!=='onclick'||records[1].oldValue!=='unrequested()'||records[2].attributeName!=='fill'||records[2].oldValue!=='red'||node.getAttributeNode('fill')!==fill||fill.value!=='#123456'||fill.ownerDocument!==document||node.ownerDocument!==document||node.bindingState.held!==node||staged.firstChild!==null)throw 'actual foreign data promotion and observation';document.title='Inert SVG data active';});observer.observe(node,{attributes:true,attributeOldValue:true});document.getElementById('run').addEventListener('click',function(event){if(!event.isTrusted)throw 'native action';Promise.resolve().then(function(){node.setAttribute('onclick','unrequested()');var caught=false;try{document.getElementById('mount').appendChild(staged);}catch(e){caught=e instanceof DOMException&&e.name==='NotSupportedError';}if(!caught||node.ownerDocument!==owner||staged.firstChild!==root)throw 'inert data grants no activation';node.removeAttribute('onclick');node.setAttribute('fill','#123456');document.getElementById('mount').appendChild(staged);});});document.title='Inert SVG data ready';
</script></body></html>"##).unwrap();
    let mut app = App::new();
    app.resize(480, 360);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    assert_eq!(
        app.document.title,
        "Inert SVG data ready",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some(name))
            .unwrap()
    };
    let neighbor = id(&app, "neighbor");
    let run = id(&app, "run");
    let go = id(&app, "go");
    let click = |app: &mut App, node| {
        let r = app.page.controls.iter().find(|r| r.node == node).unwrap();
        let (x, y) = (r.x + 10, app.content_top() + r.y + 10 - app.scroll);
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
    };
    click(&mut app, neighbor);
    app.paste("edited");
    let v = &app.forms.controls[&neighbor].value;
    let editor = (v.text(), v.cursor, v.anchor, v.offset);
    click(&mut app, run);
    assert_eq!(
        app.document.title,
        "Inert SVG data active",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let c = app.draw();
    assert_eq!(
        c.pixels[(app.content_top() + 140 - app.scroll) as usize * c.width + 30] & 0xffffff,
        0x123456
    );
    let v = &app.forms.controls[&neighbor].value;
    assert_eq!((v.text(), v.cursor, v.anchor, v.offset), editor);
    assert_eq!(app.page.link_node_at(280, 20), Some(go));
    let y = app.content_top() + 20 - app.scroll;
    app.pointer_down(280, y, false);
    app.pointer_up(280, y);
    assert_eq!(app.location.fragment, "destination");
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
}
