//! Actual App paint/hit transitions with stable native editor state.
use super::*;
#[test]
fn stacking_actual_app_changes_context_levels_and_native_link_order_without_losing_editor() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(
        std::env::temp_dir().join(format!("chezcat-stacking-app-{}.html", std::process::id())),
    );
    std::fs::write(&file.0,r#"<html><head><title>ready</title><style>html,body{margin:0;background:#fff}#run{position:absolute;left:20px;top:20px;width:160px;height:32px}#bar{position:fixed;left:20px;top:110px;width:180px;height:80px;z-index:10}#barlink{display:block;width:180px;height:80px;background:#2040e0}#front{position:absolute;left:20px;top:110px;width:180px;height:80px;background:#20a040;z-index:20}#field{position:absolute;left:10px;top:10px;width:100px;height:30px}</style></head><body><button id=run type=button>Lower search panel</button><div id=bar><a id=barlink href=#destination></a></div><div id=front><input id=field value=seed></div><div id=destination></div><script>document.getElementById('run').onclick=function(e){if(!e.isTrusted)throw 'native stacking trigger';var front=document.getElementById('front');if(getComputedStyle(front).zIndex!=='20')throw 'real initial level';front.style.zIndex='0';if(getComputedStyle(front).getPropertyValue('z-index')!=='0')throw 'real changed level';document.title='lowered';};</script></body></html>"#).unwrap();
    let mut app = App::new();
    app.resize(640, 440);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some(name))
            .unwrap()
    };
    let field = id(&app, "field");
    let run = id(&app, "run");
    let link = id(&app, "barlink");
    let rect = app.page.controls.iter().find(|r| r.node == field).unwrap();
    let (x, y) = (rect.x + 12, app.content_top() + rect.y + 12 - app.scroll);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    app.paste("edited native");
    let v = &app.forms.controls[&field].value;
    let editor = (v.text(), v.cursor, v.anchor, v.offset);
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[(app.content_top() + 165 - app.scroll) as usize * canvas.width + 160]
            & 0xffffff,
        0x20a040
    );
    assert_eq!(app.page.link_node_at(160, 165), None);
    let rect = app.page.controls.iter().find(|r| r.node == run).unwrap();
    let (x, y) = (rect.x + 12, app.content_top() + rect.y + 12 - app.scroll);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    assert_eq!(
        app.document.title,
        "lowered",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[(app.content_top() + 165 - app.scroll) as usize * canvas.width + 160]
            & 0xffffff,
        0x2040e0
    );
    assert_eq!(app.page.link_node_at(160, 165), Some(link));
    let v = &app.forms.controls[&field].value;
    assert_eq!((v.text(), v.cursor, v.anchor, v.offset), editor);
    let y = app.content_top() + 165 - app.scroll;
    app.pointer_down(160, y, false);
    app.pointer_up(160, y);
    assert_eq!(app.location.fragment, "destination");
}

#[test]
fn stacking_propagated_body_canvas_color_does_not_repaint_over_negative_content() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-stacking-canvas-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,"<html><head><style>html,body{margin:0}body{background:#ffffff}#flow{height:180px}#negative{position:absolute;left:20px;top:20px;width:80px;height:40px;background:#e03040;z-index:-1}</style></head><body><div id=flow></div><div id=negative></div></body></html>").unwrap();
    let mut app = App::new();
    app.resize(640, 400);
    app.open(&file.0);
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[(app.content_top() + 35 - app.scroll) as usize * canvas.width + 35]
            & 0xffffff,
        0xe03040
    );
    assert_eq!(
        canvas.pixels[(app.content_top() + 100 - app.scroll) as usize * canvas.width + 135]
            & 0xffffff,
        0xffffff
    );
}
