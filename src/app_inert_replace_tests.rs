//! Connected original component-style inert replacement/import and native App.
use super::*;
#[test]
fn inert_replace_actual_app_component_preparation_imports_real_nodes_paints_and_keeps_native_input_link(
) {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(
        std::env::temp_dir().join(format!("chezcat-inert-replace-{}.html", std::process::id())),
    );
    std::fs::write(&file.0,r##"<html><head><style>html,body{margin:0}#new{position:absolute;left:10px;top:10px;width:180px;height:30px;color:#123456}#query{position:absolute;left:10px;top:60px;width:180px;height:30px}#go{position:absolute;display:block;left:10px;top:110px;width:100px;height:24px;background:#2040e0}</style></head><body><div id=mount></div><div id=destination></div><script>
var template=document.createElement('template');template.innerHTML='<div><span id="old">old</span><input id="query" value="seed"><a id="go" href="#destination"></a></div>';var fragment=template.content,parent=fragment.firstElementChild,old=parent.firstChild,owner=parent.ownerDocument,next=owner.createElement('span');next.setAttribute('id','new');next.textContent='Replacement ready';var records=0,observer=new MutationObserver(function(changes){if(changes.length!==1||changes[0].target!==parent||changes[0].addedNodes[0]!==next||changes[0].removedNodes[0]!==old)throw 'real inert replacement record';records++;});observer.observe(parent,{childList:true});var component={node:parent,replace:function(n,o){return Node.prototype.replaceChild.call(this.node,n,o);}};if(component.replace(next,old)!==old||parent.firstChild!==next||next.ownerDocument!==owner||old.parentNode!==null)throw 'actual owner replacement';Promise.resolve().then(function(){if(records!==1)throw 'deferred actual observation';var imported=document.importNode(fragment,true);document.getElementById('mount').appendChild(imported);if(document.getElementById('new').textContent!=='Replacement ready')throw 'real imported replacement';document.title='Replacement ready';});
</script></body></html>"##).unwrap();
    let mut app = App::new();
    app.resize(480, 320);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    assert_eq!(
        app.document.title,
        "Replacement ready",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some(name))
            .unwrap()
    };
    let query = id(&app, "query");
    let go = id(&app, "go");
    let canvas = app.draw();
    let top = (app.content_top() + 10 - app.scroll) as usize;
    assert!((top..top + 30)
        .any(|y| canvas.pixels[y * canvas.width + 10..y * canvas.width + 210].contains(&0x123456)));
    assert_eq!(
        canvas.pixels[(app.content_top() + 120 - app.scroll) as usize * canvas.width + 25]
            & 0xffffff,
        0x2040e0
    );
    let r = app.page.controls.iter().find(|r| r.node == query).unwrap();
    let (x, y) = (r.x + 10, app.content_top() + r.y + 10 - app.scroll);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    app.paste("edited");
    assert!(app.forms.controls[&query].value.text().contains("edited"));
    assert_eq!(app.page.link_node_at(25, 120), Some(go));
    let y = app.content_top() + 120 - app.scroll;
    app.pointer_down(25, y, false);
    app.pointer_up(25, y);
    assert_eq!(app.location.fragment, "destination");
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
}
