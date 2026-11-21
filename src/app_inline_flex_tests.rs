//! Actual shared variable sizing/flex/gap changes update pixels and hit geometry.
use super::*;
#[test]
fn inline_flex_actual_app_keeps_inline_geometry_computed_display_and_native_hits() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "chezcat-inline-flex-app-{}.html",
        std::process::id()
    )));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Variable ready</title><style>
html,body{margin:0;background:#ffffff}#run{position:absolute;left:20px;top:20px;width:180px;height:32px}#neighbor{position:absolute;left:240px;top:20px;width:180px;height:32px}#wrap{position:absolute;left:20px;top:110px;width:calc(100% - 40px);height:40px}#pre,#post{display:inline-block;width:20px;height:40px}#row{--gap:10px;--display:inline-flex;display:var(--display);height:40px;gap:var(--gap)}#first{flex:0 0 40px;height:40px;background:#e03040}#link{display:block;flex:0 0 40px;min-width:0;height:40px;background:#20a040}
</style></head><body><button id=run type=button>Resize variables</button><input id=neighbor value=seed><div id=wrap><div id=pre></div><div id=row><div id=first></div><a id=link href=#destination></a></div><div id=post></div></div><div id=destination></div><script>
document.getElementById('run').onclick=function(e){if(!e.isTrusted)throw 'native inline-flex trigger';var row=document.getElementById('row');if(row.getBoundingClientRect().width!==90||getComputedStyle(row).display!=='inline-flex')throw 'real inline flex layout '+row.getBoundingClientRect().width+' '+getComputedStyle(row).display;row.style.cssText='--gap:30px;display:inline-flex;';if(row.getBoundingClientRect().width!==110||getComputedStyle(row).display!=='inline-flex')throw 'whole CSSOM inner flex layout';document.title='Variable delivered';};
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
    let link_y = app.page.border_rects[link].unwrap().y + 30;
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
    assert_eq!(app.page.link_node_at(95, link_y), Some(link));
    {
        let canvas = app.draw();
        assert_eq!(
            canvas.pixels[(app.content_top() + link_y - app.scroll) as usize * canvas.width + 95]
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
    assert_eq!(app.page.link_node_at(115, link_y), Some(link));
    assert_eq!(app.page.link_node_at(95, link_y), None);
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[(app.content_top() + link_y - app.scroll) as usize * canvas.width + 115]
            & 0xffffff,
        0x20a040
    );
    assert_eq!(
        canvas.pixels[(app.content_top() + link_y - app.scroll) as usize * canvas.width + 95]
            & 0xffffff,
        0xffffff
    );
    app.resize(320, 360);
    assert_eq!(app.page.link_node_at(115, link_y), Some(link));
    let y = app.content_top() + link_y - app.scroll;
    app.pointer_down(115, y, false);
    app.pointer_up(115, y);
    assert_eq!(app.location.fragment, "destination");
}
