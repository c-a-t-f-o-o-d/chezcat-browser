//! Original connected behavior preparation, not an actual captured-site result.
use super::*;
#[test]
fn c06_flat_workspace_actual_app_behavior_preparation_paints_and_preserves_native_editor_link() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(
        std::env::temp_dir().join(format!("chezcat-flat-behavior-{}.html", std::process::id())),
    );
    std::fs::write(&file.0,r##"<html><head><style>html,body{margin:0;background:#fff}#neighbor{position:absolute;left:10px;top:10px;width:160px;height:30px}#run{position:absolute;left:10px;top:70px;width:160px;height:30px}#out{position:absolute;left:10px;top:120px;width:220px;height:30px}#go{position:absolute;display:block;left:260px;top:10px;width:100px;height:30px;background:#2040e0}</style></head><body><input id=neighbor value=seed><div id=mount></div><a id=go href=#destination>Continue</a><div id=destination></div><script>
var behaviors=[{name:'text',apply:function(node){node.textContent='Behaviors active';}},{name:'paint',apply:function(node){node.style.backgroundColor='#123456';}}];for(var i=0;i<128;i++)behaviors=[behaviors];var prepared=behaviors.flat(Infinity).flatMap(function(behavior){return [behavior];});if(prepared.length!==2)throw 'actual behavior preparation';var template=document.createElement('template');template.innerHTML='<button id="run" type="button">Activate</button><div id="out">ready</div>';document.getElementById('mount').appendChild(document.importNode(template.content,true));document.getElementById('run').addEventListener('click',function(event){if(!event.isTrusted)throw 'native action';var node=document.getElementById('out');for(var i=0;i<prepared.length;i++)prepared[i].apply(node);document.title='Behavior preparation active';});document.title='Behavior preparation ready';
</script></body></html>"##).unwrap();
    let mut app = App::new();
    app.resize(480, 360);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    assert_eq!(
        app.document.title,
        "Behavior preparation ready",
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
        "Behavior preparation active",
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
}
