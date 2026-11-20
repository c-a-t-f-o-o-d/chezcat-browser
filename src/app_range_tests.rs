//! Native Range boundaries drive real App pixels after trusted input.
use super::*;
#[test]
fn dom_range_actual_app_trusted_mutation_paints_live_text_and_keeps_editor_and_link() {
    let path = std::env::temp_dir().join(format!(
        "chezcat-live-range-app-{}.html",
        std::process::id()
    ));
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let fixture = Fixture(path);
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><style>
html,body{margin:0;background:#ffffff}#neighbor{position:absolute;left:20px;top:20px;width:200px;height:32px}#advance{position:absolute;left:260px;top:20px;width:120px;height:32px}
#out{position:absolute;left:20px;top:100px;width:300px;height:40px;margin:0;color:#123456;background:#ffffff;font-size:24px}#stable{display:block;position:absolute;left:20px;top:180px;width:100px;height:24px;background:#336699;color:white}
</style></head><body><input id=neighbor value=seed><button id=advance>Advance</button><p id=out>Fallback</p><a id=stable href=#destination>Go</a><div id=destination></div><div id=source>AB</div><script>
var source=document.getElementById('source'),out=document.getElementById('out'),r=new Range();r.selectNodeContents(source.firstChild);out.textContent=r.toString();
document.getElementById('advance').onclick=function(e){if(!e.isTrusted)throw 'trusted click required';source.appendChild(document.createTextNode('CD'));r.setEnd(source.lastChild,2);source.normalize();if(r.startContainer!==source.firstChild||r.endOffset!==4)throw 'live merge';out.textContent=r.toString();document.title='Range live';};
</script></body></html>"#).unwrap();
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&node| app.document.attr(node, "id") == Some(name))
            .unwrap()
    };
    assert!(!app.failed, "{}", app.status);
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let neighbor = id(&app, "neighbor");
    app.forms
        .script_set_value(neighbor, "edited neighbor")
        .unwrap();
    {
        let editor = &mut app.forms.controls.get_mut(&neighbor).unwrap().value;
        editor.cursor = 5;
        editor.anchor = 2;
        editor.offset = 1;
    }
    let out = id(&app, "out");
    let before = app.draw().pixels;
    let button = id(&app, "advance");
    let rect = app
        .page
        .controls
        .iter()
        .find(|rect| rect.node == button)
        .unwrap();
    let (x, y) = (
        rect.x + rect.width / 2,
        app.content_top() + rect.y + rect.height / 2 - app.scroll,
    );
    assert!(matches!(
        app.pointer_down(x, y, false),
        PlatformAction::None
    ));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    assert_eq!(app.document.title, "Range live");
    let rect = app.page.border_rects[out].unwrap();
    assert_eq!(
        (rect.x, rect.y, rect.width, rect.height),
        (20, 100, 300, 40)
    );
    let text: String = app
        .page
        .runs
        .iter()
        .filter(|run| run.x >= 20 && run.x < 320 && run.y >= 100 && run.y < 140)
        .map(|run| run.text.as_str())
        .collect();
    assert_eq!(text, "ABCD");
    let after = app.draw();
    assert_ne!(before, after.pixels);
    let top = (app.content_top() + 100 - app.scroll) as usize;
    assert!((top..top + 40)
        .any(|y| after.pixels[y * after.width + 20..y * after.width + 320].contains(&0x123456)));
    let editor = &app.forms.controls[&neighbor].value;
    assert_eq!(
        (editor.text(), editor.cursor, editor.anchor, editor.offset),
        ("edited neighbor".into(), 5, 2, 1)
    );
    assert_eq!(app.page.link_node_at(70, 192), Some(id(&app, "stable")));
    assert_eq!(app.page.link_at(70, 192), Some("#destination"));
}
