//! Original author root wrapper keeps logical views separate from native splice.
use super::*;
#[test]
fn fragment_author_prototype_actual_app_logical_wrapper_native_splice_jobs_pixels_editor_and_link()
{
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-fragment-prototype-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,r##"<html><head><style>html,body{margin:0;background:#fff}#neighbor{position:absolute;left:10px;top:10px;width:160px;height:30px}#run{position:absolute;left:10px;top:70px;width:160px;height:30px}#bound{position:absolute;left:10px;top:120px;width:220px;height:30px}#go{position:absolute;display:block;left:260px;top:10px;width:100px;height:30px;background:#2040e0}</style></head><body><input id=neighbor value=seed><button id=run type=button>Update binding</button><div id=mount></div><a id=go href=#destination>Continue</a><div id=destination></div><script>
var template=document.createElement('template');template.innerHTML='<div id="bound">Wrapped ready</div>';var staged=document.importNode(template.content,true),nativeFirst=Object.getOwnPropertyDescriptor(Node.prototype,'firstChild').get,nativeConnected=Object.getOwnPropertyDescriptor(Node.prototype,'isConnected').get,nativeAppend=Node.prototype.appendChild,host=document.getElementById('mount'),bound=nativeFirst.call(staged),P=Object.create(DocumentFragment.prototype,{firstChild:{get:function(){return this.host.firstChild;}},ownerDocument:{get:function(){return this.host.ownerDocument;}},isConnected:{get:function(){return this.host.isConnected;}}});staged.host=host;staged.__proto__=P;if(staged.firstChild!==null||nativeFirst.call(staged)!==bound||!staged.isConnected||nativeConnected.call(staged))throw 'logical versus native fragment before splice';nativeAppend.call(host,staged);if(staged.firstChild!==bound||nativeFirst.call(staged)!==null||Object.getPrototypeOf(staged)!==P||staged.ownerDocument!==document||!staged.isConnected||nativeConnected.call(staged)||staged.getRootNode()!==staged||bound.parentNode!==host)throw 'native splice and saved owner';var observer=new MutationObserver(function(records){if(records.length!==1||records[0].target!==bound||bound.textContent!=='Wrapped active'||staged.firstChild!==bound||nativeConnected.call(staged))throw 'wrapped component mutation';document.title='Fragment wrapper active';});observer.observe(bound,{childList:true});document.getElementById('run').addEventListener('click',function(event){if(!event.isTrusted)throw 'native action';Promise.resolve().then(function(){staged.firstChild.textContent='Wrapped active';staged.firstChild.style.backgroundColor='#123456';});});document.title='Fragment wrapper ready';
</script></body></html>"##).unwrap();
    let mut app = App::new();
    app.resize(480, 360);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    assert_eq!(
        app.document.title,
        "Fragment wrapper ready",
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
        "Fragment wrapper active",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let c = app.draw();
    assert_eq!(
        c.pixels[(app.content_top() + 140 - app.scroll) as usize * c.width + 200] & 0xffffff,
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
