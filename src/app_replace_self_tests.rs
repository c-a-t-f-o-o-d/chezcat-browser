//! Native focus and editor effects of a real same-node DOM replacement.
use super::*;
use crate::scripting::KeyboardInput;
#[test]
fn replacement_self_actual_app_removal_clears_native_focus_keeps_editor_pixels_and_link() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(
        std::env::temp_dir().join(format!("chezcat-self-replace-{}.html", std::process::id())),
    );
    std::fs::write(&file.0,r##"<html><head><style>html,body{margin:0}#shell{position:absolute;left:10px;top:10px;width:240px;height:150px;background:#e03040}#shell:focus-within{background:#20a040}#query{position:absolute;left:10px;top:10px;width:160px;height:32px}#go{position:absolute;display:block;left:10px;top:80px;width:100px;height:24px;background:#2040e0}</style></head><body><div id=mount></div><div id=destination></div><script>
var t=document.createElement('template');t.innerHTML='<div id="shell"><input id="query" value="seed"><a id="go" href="#destination"></a></div>';document.getElementById('mount').appendChild(document.importNode(t.content,true));var shell=document.getElementById('shell'),query=document.getElementById('query');query.onkeydown=function(e){if(e.key==='Enter'){if(!e.isTrusted||!query.matches(':focus'))throw 'real native focus before removal';e.preventDefault();shell.parentNode.replaceChild(shell,shell);if(query.matches(':focus')||shell.matches(':focus-within'))throw 'native self replacement removal focus';document.title='Self replaced';}};
</script></body></html>"##).unwrap();
    let mut app = App::new();
    app.resize(480, 320);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some(name))
            .unwrap()
    };
    let query = id(&app, "query");
    let link = id(&app, "go");
    let r = app.page.controls.iter().find(|r| r.node == query).unwrap();
    let (x, y) = (r.x + 10, app.content_top() + r.y + 10 - app.scroll);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    app.paste("edited");
    let v = &app.forms.controls[&query].value;
    let editor = (v.text(), v.cursor, v.anchor, v.offset);
    assert_eq!(app.document.native_focus.get(), Some(query));
    let before = app.draw();
    assert_eq!(
        before.pixels[(app.content_top() + 140 - app.scroll) as usize * before.width + 230]
            & 0xffffff,
        0x20a040
    );
    let (_, canceled) = app.key_event(KeyboardInput {
        down: true,
        key: "Enter".into(),
        code: "Enter".into(),
        key_code: 13,
        ..KeyboardInput::default()
    });
    assert!(canceled);
    assert_eq!(
        app.document.title,
        "Self replaced",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    assert_eq!(app.document.native_focus.get(), None);
    assert!(!matches!(app.focus, Focus::Control(_)));
    let v = &app.forms.controls[&query].value;
    assert_eq!((v.text(), v.cursor, v.anchor, v.offset), editor);
    let after = app.draw();
    assert_eq!(
        after.pixels[(app.content_top() + 140 - app.scroll) as usize * after.width + 230]
            & 0xffffff,
        0xe03040
    );
    assert_eq!(app.page.link_node_at(30, 100), Some(link));
    let y = app.content_top() + 100 - app.scroll;
    app.pointer_down(30, y, false);
    app.pointer_up(30, y);
    assert_eq!(app.location.fragment, "destination");
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
}
