//! Native pointer/keyboard focus updates actual CSS, geometry and painted UI.
use super::*;
#[test]
fn native_focus_selectors_actual_app_pointer_tab_chrome_and_editor_state_agree() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-native-focus-app-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,r#"<html><head><title>ready</title><style>html,body{margin:0;background:#fff}#wrap{position:absolute;left:20px;top:90px;width:240px;height:80px;background:#e03040;color:#e03040}#wrap:focus-within{background:#20a040;color:#20a040}#a{position:absolute;left:10px;top:10px;width:100px;height:32px}#a:focus{color:#2040e0}#b{position:absolute;left:300px;top:100px;width:100px;height:32px}#link{position:absolute;left:180px;top:40px;display:block;width:40px;height:20px;background:#2040e0}</style></head><body><div id=wrap><input id=a value=seed><a id=link href=#destination></a></div><input id=b><div id=destination></div><script>
var a=document.getElementById('a'),b=document.getElementById('b'),wrap=document.getElementById('wrap');
a.addEventListener('focusin',function(e){if(!e.isTrusted||!a.matches(':focus')||!wrap.matches(':focus-within')||getComputedStyle(wrap).color!=='rgb(32, 160, 64)')throw 'real focused CSS in native callback';document.title='focused';});
a.addEventListener('blur',function(e){if(a.matches(':focus')||wrap.matches(':focus-within'))throw 'native blur CSS';document.title='blurred';});
</script></body></html>"#).unwrap();
    let mut app = App::new();
    app.resize(640, 400);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some(name))
            .unwrap()
    };
    let a = id(&app, "a");
    let b = id(&app, "b");
    let link = id(&app, "link");
    let pixel = |app: &App| {
        let canvas = app.draw();
        canvas.pixels[(app.content_top() + 155 - app.scroll) as usize * canvas.width + 35]
            & 0xffffff
    };
    assert_eq!(pixel(&app), 0xe03040);
    let r = app.page.controls.iter().find(|r| r.node == a).unwrap();
    let (x, y) = (r.x + 12, app.content_top() + r.y + 12 - app.scroll);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    assert_eq!(
        app.document.title,
        "focused",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    assert_eq!(pixel(&app), 0x20a040);
    app.paste("edited native");
    let v = &app.forms.controls[&a].value;
    let editor = (v.text(), v.cursor, v.anchor, v.offset);
    app.tab_focus(false);
    assert!(app.focus == Focus::Control(b));
    assert_eq!(
        app.document.title,
        "blurred",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    assert_eq!(pixel(&app), 0xe03040);
    let v = &app.forms.controls[&a].value;
    assert_eq!((v.text(), v.cursor, v.anchor, v.offset), editor);
    app.focus_control(a);
    assert_eq!(pixel(&app), 0x20a040);
    app.focus_address();
    assert_eq!(pixel(&app), 0xe03040);
    assert_eq!(app.page.link_node_at(210, 140), Some(link));
    let y = app.content_top() + 140 - app.scroll;
    app.pointer_down(210, y, false);
    app.pointer_up(210, y);
    assert_eq!(app.location.fragment, "destination");
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
}

#[test]
fn native_focus_selectors_work_without_javascript_and_hidden_targets_do_not_reacquire_focus() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    for hidden in [false, true] {
        let file = Fixture(std::env::temp_dir().join(format!(
            "chezcat-native-focus-boundary-{}-{hidden}.html",
            std::process::id()
        )));
        std::fs::write(&file.0,format!("<style>html,body{{margin:0;background:#fff}}#wrap{{position:absolute;left:20px;top:90px;width:240px;height:80px;background:#e03040}}#wrap:focus-within{{background:#20a040}}#a{{position:absolute;left:10px;top:10px;width:100px;height:32px}}{}</style><div id=wrap><input id=a value=seed></div>",if hidden{"#a:focus{display:none}"}else{""})).unwrap();
        let mut app = App::new();
        app.resize(640, 400);
        app.set_javascript_enabled(false);
        app.open(&file.0);
        assert!(!app.document.scripting_enabled);
        let a = (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some("a"))
            .unwrap();
        let r = app.page.controls.iter().find(|r| r.node == a).unwrap();
        let (x, y) = (r.x + 12, app.content_top() + r.y + 12 - app.scroll);
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
        let canvas = app.draw();
        assert_eq!(
            canvas.pixels[(app.content_top() + 155 - app.scroll) as usize * canvas.width + 35]
                & 0xffffff,
            if hidden { 0xe03040 } else { 0x20a040 }
        );
        assert_eq!(
            app.document.native_focus.get(),
            if hidden { None } else { Some(a) }
        );
        if hidden {
            assert!(app.focus == Focus::Page);
            assert!(app.page.controls.iter().any(|r| r.node == a));
        } else {
            assert!(app.focus == Focus::Control(a));
            app.paste("native");
            assert!(app.forms.controls[&a].value.text().contains("native"));
        }
    }
}
