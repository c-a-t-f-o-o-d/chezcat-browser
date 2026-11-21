//! Original delayed integer conversion drives real text, paint and controls.
use super::*;
#[test]
fn bigint_width_actual_app_saved_intrinsics_timer_and_promise_update_native_text_editor_and_link() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(
        std::env::temp_dir().join(format!("chezcat-bigint-width-{}.html", std::process::id())),
    );
    std::fs::write(&file.0,r##"<html><head><style>html,body{margin:0;background:#fff}#neighbor{position:absolute;left:10px;top:10px;width:160px;height:30px}#run{position:absolute;left:10px;top:70px;width:160px;height:30px}#bound{position:absolute;left:10px;top:120px;width:220px;height:30px}#go{position:absolute;display:block;left:260px;top:10px;width:100px;height:30px;background:#2040e0}</style></head><body><input id=neighbor value=seed><button id=run type=button>Update binding</button><div id=mount></div><a id=go href=#destination>Continue</a><div id=destination></div><script>
var signed=BigInt.asIntN,unsigned=BigInt.asUintN;document.getElementById('mount').innerHTML='<div id="bound">Pending integer</div>';var root=document.getElementById('bound');document.getElementById('run').addEventListener('click',function(event){if(!event.isTrusted)throw 'native action';setTimeout(function(){var a=signed(64,'18446744073709551615'),b=unsigned(64,'-1');Promise.resolve().then(function(){root.textContent=String(a)+' / '+String(b);root.style.backgroundColor='#123456';document.title='Width '+String(a)+' '+String(b);});},0);});document.title='Integer ready';
</script></body></html>"##).unwrap();
    let mut app = App::new();
    app.resize(480, 360);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    assert_eq!(
        app.document.title,
        "Integer ready",
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
    app.wait_for_script_timers(std::time::Duration::from_millis(200));
    assert!(app.document.nodes.iter().any(
        |n| matches!(&n.kind,crate::html::NodeKind::Text(text) if text=="-1 / 18446744073709551615")
    ));
    assert_eq!(
        app.document.title,
        "Width -1 18446744073709551615",
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
