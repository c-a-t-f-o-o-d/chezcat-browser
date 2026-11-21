//! Actual native hints, multiline pixels, editing and real anchor activation.
use super::*;
#[test]
fn native_placeholder_actual_app_updates_multiline_hints_without_values_and_keeps_editor_links() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "chezcat-native-placeholder-app-{}.html",
        std::process::id()
    )));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Hint ready</title><style>
html,body{margin:0;background:#ffffff}#run{position:absolute;left:20px;top:20px;width:180px;height:32px}#neighbor{position:absolute;left:240px;top:20px;width:180px;height:32px}#area{position:absolute;left:20px;top:100px;width:180px;height:80px}#input{position:absolute;left:240px;top:100px;width:180px;height:32px}#link{display:block;position:absolute;left:20px;top:220px;width:180px;height:36px;background:#204060}
</style></head><body><button id=run type=button>Set hints</button><input id=neighbor value=seed><textarea id=area name=note></textarea><input id=input name=q><a id=link href=#destination>Real link</a><div id=destination></div><script>
document.getElementById('run').onclick=function(e){if(!e.isTrusted)throw 'native hint trigger';var area=document.getElementById('area'),input=document.getElementById('input');area.placeholder='First hint\r\nSecond hint';input.placeholder='Input\r\nhint';if(area.value!==''||area.textContent!==''||input.value!==''||area.getAttribute('placeholder')!=='First hint\r\nSecond hint')throw 'hint became real value';document.title='Hint delivered';};
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
    assert_eq!(
        app.document.title,
        "Hint delivered",
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
    assert_eq!(app.page.link_node_at(40, 235), Some(link));
    assert_eq!(app.page.link_at(40, 235), Some("#destination"));
    let area = id(&app, "area");
    let input = id(&app, "input");
    assert!(app.forms.controls[&area].value.text().is_empty());
    assert!(app.forms.controls[&input].value.text().is_empty());
    let rect = app.page.controls.iter().find(|c| c.node == area).unwrap();
    let line = rect
        .text_font
        .as_ref()
        .map_or((GLYPH_HEIGHT + 2) * rect.scale, |font| font.height() + 2)
        .max(1);
    let (left, top, width) = (rect.x, rect.y, rect.width);
    let canvas = app.draw();
    let hint_in = |from: i32, to: i32| {
        (from..to).any(|y| {
            (left..left + width).any(|x| {
                canvas.pixels
                    [(app.content_top() + y - app.scroll) as usize * canvas.width + x as usize]
                    & 0xffffff
                    == 0x75818b
            })
        })
    };
    assert!(hint_in(top + 4, top + 4 + line), "first real hint line");
    assert!(
        hint_in(top + 4 + line, top + 4 + 2 * line),
        "second real hint line"
    );
    let c = app.page.controls.iter().find(|c| c.node == area).unwrap();
    let (x, y) = (c.x + 12, app.content_top() + c.y + 12 - app.scroll);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    app.paste("typed value");
    assert_eq!(app.forms.controls[&area].value.text(), "typed value");
    assert_eq!(
        app.document.attr(area, "placeholder"),
        Some("First hint\r\nSecond hint")
    );
    let c = app
        .page
        .controls
        .iter()
        .find(|c| c.node == neighbor)
        .unwrap();
    let (x, y) = (c.x + 12, app.content_top() + c.y + 12 - app.scroll);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    let canvas = app.draw();
    assert!(
        !(top + 4..top + 4 + 2 * line).any(|y| (left..left + width).any(|x| canvas.pixels
            [(app.content_top() + y - app.scroll) as usize * canvas.width + x as usize]
            & 0xffffff
            == 0x75818b)),
        "hint painted over typed native value"
    );
    let y = app.content_top() + 235 - app.scroll;
    app.pointer_down(40, y, false);
    app.pointer_up(40, y);
    assert_eq!(app.location.fragment, "destination");
}
