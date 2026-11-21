//! Connected original App package check; this is not an actual YouTube result.
use super::*;
#[test]
fn p2_shell_package_actual_app_composes_values_inline_flex_focus_hover_visibility_stacking_and_native_input(
) {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-p2-shell-package-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,r#"<html><head><title>shell ready</title><style>
html,body{margin:0;background:#fff;--bar-width:calc(100vw - 12px);--guide:40px;--gap:8px}#masthead{position:fixed;left:0;top:0;display:flex;align-items:center;box-sizing:border-box;padding:8px;gap:var(--gap);width:var(--bar-width);height:80px;background:#ffffff;z-index:100}#masthead:focus-within{background:#20a040}#logo{display:inline-flex;flex:0 0 40px;height:40px;background:#2040e0}#logo:hover{background:#e03040}#markWrap{display:block}#mark{display:inline-flex;gap:4px}#mark>span{display:block;flex:none;width:12px;height:12px}#query{flex:1 1 0%;min-width:0;height:32px}#run{flex:0 0 80px;height:32px}#guide{position:fixed;left:0;top:80px;width:var(--guide);height:300px;background:#dddddd}#content{position:absolute;left:var(--guide);top:80px;width:calc(100% - var(--guide));height:500px;background:#f0f0f0}#menu{position:fixed;left:180px;top:88px;width:100px;height:40px;visibility:hidden;pointer-events:none;z-index:200}#go{display:block;width:100px;height:40px;background:#2040e0}#decoration{position:fixed;left:0;top:0;width:100%;height:80px;pointer-events:none;z-index:300}#literal{position:absolute;visibility:hidden;pointer-events:none;width:calc(100vw - 12px);height:80px}
</style></head><body><div id=masthead><a id=logo href=#logo-destination><span id=markWrap><span id=mark><span></span><span></span></span></span></a><input id=query value=seed><button id=run type=button>Menu</button></div><div id=guide></div><div id=content></div><div id=menu><a id=go href=#destination></a></div><div id=decoration></div><div id=literal></div><div id=destination></div><div id=logo-destination></div><script>
var header=document.getElementById('masthead'),query=document.getElementById('query'),menu=document.getElementById('menu');
query.addEventListener('focusin',function(e){if(!e.isTrusted||!header.matches(':focus-within')||!query.matches(':focus'))throw 'real shell focus';});
document.getElementById('run').onclick=function(e){if(!e.isTrusted)throw 'native shell menu';if(header.getBoundingClientRect().width!==document.getElementById('literal').getBoundingClientRect().width||getComputedStyle(document.getElementById('mark')).display!=='inline-flex')throw 'shared shell values and inner inline flex';menu.style.cssText='position:fixed;left:180px;top:88px;width:100px;height:40px;visibility:visible;pointer-events:auto;z-index:200;';if(getComputedStyle(menu).visibility!=='visible'||getComputedStyle(menu).zIndex!=='200')throw 'real shell replacement';document.title='menu open';};
</script></body></html>"#).unwrap();
    let mut widths = Vec::new();
    for width in [640, 320] {
        let mut app = App::new();
        app.resize(width, 440);
        app.set_javascript_enabled(true);
        app.open(&file.0);
        assert!(!app.failed && !app.is_loading(), "{}", app.status);
        let id = |app: &App, name: &str| {
            (0..app.document.nodes.len())
                .find(|&n| app.document.attr(n, "id") == Some(name))
                .unwrap()
        };
        let query = id(&app, "query");
        let run = id(&app, "run");
        let logo = id(&app, "logo");
        let go = id(&app, "go");
        assert_eq!(
            app.page.border_rects[id(&app, "masthead")].unwrap().width,
            width - 12
        );
        assert_eq!(app.page.border_rects[id(&app, "mark")].unwrap().width, 28);
        let r = app.page.border_rects[logo].unwrap();
        app.pointer_move(r.x + 25, app.content_top() + r.y + 30 - app.scroll, false);
        let canvas = app.draw();
        assert_eq!(
            canvas.pixels[(app.content_top() + r.y + 30 - app.scroll) as usize * canvas.width
                + (r.x + 25) as usize]
                & 0xffffff,
            0xe03040
        );
        let r = app.page.controls.iter().find(|r| r.node == query).unwrap();
        widths.push(r.width);
        let (x, y) = (r.x + 12, app.content_top() + r.y + 12 - app.scroll);
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
        app.paste("rust");
        let v = &app.forms.controls[&query].value;
        let editor = (v.text(), v.cursor, v.anchor, v.offset);
        let canvas = app.draw();
        assert_eq!(
            canvas.pixels[(app.content_top() + 70 - app.scroll) as usize * canvas.width + 12]
                & 0xffffff,
            0x20a040
        );
        assert_ne!(app.page.link_node_at(210, 105), Some(go));
        let r = app.page.controls.iter().find(|r| r.node == run).unwrap();
        let (x, y) = (r.x + 12, app.content_top() + r.y + 12 - app.scroll);
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
        assert_eq!(
            app.document.title,
            "menu open",
            "{}",
            app.scripting.as_ref().unwrap().report()
        );
        assert_eq!(app.page.link_node_at(210, 105), Some(go));
        let v = &app.forms.controls[&query].value;
        assert_eq!((v.text(), v.cursor, v.anchor, v.offset), editor);
        let y = app.content_top() + 105 - app.scroll;
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
    assert!(widths[0] > widths[1] && widths[1] > 40);
}
