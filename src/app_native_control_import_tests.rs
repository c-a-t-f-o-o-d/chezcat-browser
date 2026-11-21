//! Actual native controls imported from inert template data and operated by input.
use super::*;
#[test]
fn native_control_import_actual_app_edits_and_activates_real_controls_with_stable_neighbors() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "chezcat-native-control-import-{}.html",
        std::process::id()
    )));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Import ready</title><style>
html,body{margin:0;background:#ffffff}#run{position:absolute;left:20px;top:20px;width:180px;height:32px}#neighbor{position:absolute;left:240px;top:20px;width:180px;height:32px}
#mount{position:absolute;left:20px;top:100px;width:520px;height:80px}#mount original-controls{display:flex;gap:12px}#mount input{width:180px;height:32px}#mount button{width:100px;height:32px}
#report{position:absolute;left:20px;top:210px;width:400px;height:40px;color:#123456;font-size:24px}#link{display:block;position:absolute;left:20px;top:280px;width:180px;height:36px;background:#204060}
</style></head><body><button id=run type=button>Import controls</button><input id=neighbor value=seed><form id=owner><div id=mount></div></form><div id=report>Original</div><a id=link href=#destination>Real link</a><div id=destination></div><script>
document.getElementById('run').onclick=function(e){if(!e.isTrusted)throw 'native trigger required';var t=document.createElement('template');t.innerHTML='<original-controls><input id="stamped" name="q" value="seed"><button id="commit" type="button">Commit</button><button id="clear" type="reset">Reset</button></original-controls>';var copy=document.importNode(t.content,true);document.getElementById('mount').appendChild(copy);var input=document.getElementById('stamped'),button=document.getElementById('commit'),owner=document.getElementById('owner');if(input.form!==owner||button.form!==owner||input.value!=='seed')throw 'actual imported native owners';button.onclick=function(e){if(!e.isTrusted||input.value.indexOf('typed')<0)throw 'real native edit/activation';document.getElementById('report').textContent='Native import edited';document.title='Native import edited';};document.title='Native controls imported';};
</script></body></html>"#).unwrap();
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some(name))
            .unwrap()
    };
    let click = |app: &mut App, node: usize| {
        let c = app.page.controls.iter().find(|c| c.node == node).unwrap();
        let (x, y) = (c.x + 12, app.content_top() + c.y + 12 - app.scroll);
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
    };
    let neighbor = id(&app, "neighbor");
    click(&mut app, neighbor);
    app.paste("neighbor edit");
    let value = &app.forms.controls[&neighbor].value;
    let saved = (value.text(), value.cursor, value.anchor, value.offset);
    let run = id(&app, "run");
    click(&mut app, run);
    assert_eq!(
        app.document.title,
        "Native controls imported",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let input = id(&app, "stamped");
    click(&mut app, input);
    app.paste("typed");
    assert!(app.forms.controls[&input].value.text().contains("typed"));
    let commit = id(&app, "commit");
    click(&mut app, commit);
    assert_eq!(
        app.document.title,
        "Native import edited",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let clear = id(&app, "clear");
    click(&mut app, clear);
    assert_eq!(app.forms.controls[&input].value.text(), "seed");
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0);
    let value = &app.forms.controls[&neighbor].value;
    assert_eq!(
        (value.text(), value.cursor, value.anchor, value.offset),
        saved
    );
    let link = id(&app, "link");
    assert_eq!(app.page.link_node_at(40, 295), Some(link));
    assert_eq!(app.page.link_at(40, 295), Some("#destination"));
    let canvas = app.draw();
    assert!((210..250).any(|y| (20..420).any(|x| canvas.pixels
        [(app.content_top() + y - app.scroll) as usize * canvas.width + x as usize]
        == 0x123456)));
    let y = app.content_top() + 295 - app.scroll;
    app.pointer_down(40, y, false);
    app.pointer_up(40, y);
    assert_eq!(app.location.fragment, "destination");
}
