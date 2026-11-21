//! Whole CSSOM replacement changes original App layout, pixels and native hits.
use super::*;
#[test]
fn css_text_actual_app_replaces_layout_and_paint_without_losing_native_editor_or_link() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let fixture = Fixture(
        std::env::temp_dir().join(format!("chezcat-css-text-app-{}.html", std::process::id())),
    );
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Variable ready</title><style>
html,body{margin:0;background:#ffffff}#run{position:absolute;left:20px;top:20px;width:180px;height:32px}#neighbor{position:absolute;left:240px;top:20px;width:180px;height:32px}#row{position:absolute;left:20px;top:110px;--width:120px;--gap:10px;--display:flex;display:var(--display);width:var(--width);height:50px;gap:var(--gap)}#first{flex:0 0 40px;height:40px;background:#e03040}#link{display:block;flex:0 0 40px;height:40px;background:#20a040}
</style></head><body><button id=run type=button>Resize variables</button><input id=neighbor value=seed><div id=row><div id=first></div><a id=link href=#destination>Link</a></div><div id=destination></div><script>
document.getElementById('run').onclick=function(e){if(!e.isTrusted)throw 'native variable trigger';var row=document.getElementById('row');if(row.getBoundingClientRect().width!==120)throw 'real initial substituted width';row.style.cssText='position:absolute;left:20px;top:110px;display:flex;width:180px;height:40px;gap:30px;';if(row.getBoundingClientRect().width!==180)throw 'real changed substituted width';document.title='Variable delivered';};
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
    assert_eq!(app.page.link_node_at(75, 125), Some(link));
    {
        let canvas = app.draw();
        assert_eq!(
            canvas.pixels[(app.content_top() + 125 - app.scroll) as usize * canvas.width + 75]
                & 0xffffff,
            0x20a040
        );
    }
    let control = app.page.controls.iter().find(|c| c.node == run).unwrap();
    let (x, y) = (
        control.x + 12,
        app.content_top() + control.y + 12 - app.scroll,
    );
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    assert_eq!(
        app.document.title,
        "Variable delivered",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
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
    assert_eq!(app.page.link_node_at(95, 125), Some(link));
    assert_eq!(app.page.link_node_at(75, 125), None);
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[(app.content_top() + 125 - app.scroll) as usize * canvas.width + 95]
            & 0xffffff,
        0x20a040
    );
    assert_eq!(
        canvas.pixels[(app.content_top() + 125 - app.scroll) as usize * canvas.width + 75]
            & 0xffffff,
        0xffffff
    );
    let y = app.content_top() + 125 - app.scroll;
    app.pointer_down(95, y, false);
    app.pointer_up(95, y);
    assert_eq!(app.location.fragment, "destination");
}
