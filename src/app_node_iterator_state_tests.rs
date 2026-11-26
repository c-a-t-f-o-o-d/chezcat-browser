//! Actual App iterator state selects genuine native roots for real paint.
use super::*;
#[test]
fn node_iterator_state_actual_app_root_identity_drives_native_text_paint_editor_and_link_targets() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "chezcat-node-iterator-state-app-{}.html",
        std::process::id()
    )));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Iterator state ready</title><style>
html,body{margin:0;background:#ffffff}#run{position:absolute;left:20px;top:20px;width:180px;height:32px}#neighbor{position:absolute;left:240px;top:20px;width:180px;height:32px}
#out{position:absolute;left:20px;top:100px;width:400px;height:40px;color:#123456;font-size:24px}#link{display:block;position:absolute;left:20px;top:180px;width:180px;height:36px;background:#204060}
</style></head><body><button id=run type=button>Dispatch event</button><input id=neighbor value=seed><div id=out>Original</div><a id=link href=#destination>Real link</a><div id=destination></div><script>
var out=document.getElementById('out');
document.getElementById('run').onclick=function(e){
 if(!e.isTrusted)throw 'native trigger required';
 var template=document.createElement('template');template.innerHTML='<span>Seed</span>';var root=template.content,filter={acceptNode:function(){throw 'state must not call filter';}},iterator=document.createNodeIterator(root,4,filter);
 if(iterator.root!==root||iterator.referenceNode!==root||!iterator.pointerBeforeReferenceNode||iterator.whatToShow!==4||iterator.filter!==filter||!(iterator instanceof NodeIterator))throw 'native iterator initial state';
 root.firstChild.firstChild.insertData(0,'X');if(iterator.detach()!==undefined||iterator.root!==root||iterator.referenceNode!==root||!iterator.pointerBeforeReferenceNode)throw 'detach retains native state';
 out.textContent='Iterator '+iterator.root.firstChild.textContent;document.title='Iterator state delivered';
};
</script></body></html>"#).unwrap();
    let mut app = App::new();
    app.resize(640, 360);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some(name))
            .unwrap()
    };
    let original = (app.raw.clone(),app.location.clone(),app.script_epoch);
    let neighbor = id(&app, "neighbor");
    let run = id(&app, "run");
    let link = id(&app, "link");
    let control = app
        .page
        .controls
        .iter()
        .find(|c| c.node == neighbor)
        .unwrap();
    let (x, y) = (
        control.x + 12,
        app.content_top() + control.y + 12 - app.scroll,
    );
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    app.paste("edited native");
    let value = &app.forms.controls[&neighbor].value;
    let editor = (value.text(), value.cursor, value.anchor, value.offset);
    let control = app.page.controls.iter().find(|c| c.node == run).unwrap();
    let (x, y) = (
        control.x + 12,
        app.content_top() + control.y + 12 - app.scroll,
    );
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    assert_eq!(app.document.title, "Iterator state delivered","{}",app.scripting.as_ref().unwrap().report());
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let value = &app.forms.controls[&neighbor].value;
    assert_eq!(
        (value.text(), value.cursor, value.anchor, value.offset),
        editor
    );
    assert_eq!(app.page.link_node_at(40, 195), Some(link));
    assert_eq!(app.page.link_at(40, 195), Some("#destination"));
    assert_eq!((app.raw.clone(),app.location.clone(),app.script_epoch),original);
    let canvas = app.draw();
    assert!((100..140).any(|y| (20..420).any(|x| canvas.pixels
        [(app.content_top() + y - app.scroll) as usize * canvas.width + x as usize]
        == 0x123456)));
}
