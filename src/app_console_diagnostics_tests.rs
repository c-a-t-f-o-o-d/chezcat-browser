//! Connected original App: trusted native event, Promise job and genuine Error logging.
use super::*;
#[test]
fn console_native_error_actual_app_queued_component_reports_origin_and_keeps_editor_pixels_link() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-console-native-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,r##"<html><head><style>html,body{margin:0;background:#fff}#neighbor{position:absolute;left:10px;top:10px;width:160px;height:30px}#run{position:absolute;left:10px;top:65px;width:180px;height:30px}#out{position:absolute;left:10px;top:120px;width:260px;height:30px}#go{position:absolute;display:block;left:10px;top:180px;width:180px;height:30px;background:#2040e0}</style></head><body><input id=neighbor value=seed><button id=run type=button>Diagnose component</button><div id=out></div><a id=go href=#destination>Continue</a><div id=destination></div><script>
var held,hits=0;document.getElementById('run').onclick=function(event){if(!event.isTrusted)throw 'trusted native event required';Promise.resolve().then(function(){try{null.missing;}catch(e){held=e;Object.defineProperty(e,'message',{get:function(){hits++;throw 'private getter';}});console.error('component initialization',e);}if(hits!==0||!(held instanceof TypeError))throw 'error identity or getter';var out=document.getElementById('out');out.textContent='Native error observed';out.style.backgroundColor='#123456';document.title='Original error diagnosed';});};
</script></body></html>"##).unwrap();
    let mut app = App::new();
    app.resize(480, 420);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
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
        let (x, y) = (r.x + 12, app.content_top() + r.y + 12 - app.scroll);
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
    };
    click(&mut app, neighbor);
    app.paste("edited");
    let v = &app.forms.controls[&neighbor].value;
    let editor = (v.text(), v.cursor, v.anchor, v.offset);
    click(&mut app, run);
    assert_eq!(app.document.title, "Original error diagnosed");
    let page = app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(), 0, "{}", page.report());
    let report = page.report();
    assert!(report.contains("console.error: component initialization TypeError: Cannot access a property of null or undefined."),"{report}");
    assert!(report.contains("Original native error at line"));
    let v = &app.forms.controls[&neighbor].value;
    assert_eq!((v.text(), v.cursor, v.anchor, v.offset), editor);
    let y = (app.content_top() + 140 - app.scroll) as usize;
    let canvas = app.draw();
    assert_eq!(canvas.pixels[y * canvas.width + 240] & 0xffffff, 0x123456);
    assert_eq!(app.page.link_node_at(40, 195), Some(go));
    assert!(195-app.scroll<app.viewport_height(),"native link point must be visible");
    let y = app.content_top() + 195 - app.scroll;
    app.pointer_down(40, y, false);
    app.pointer_up(40, y);
    assert_eq!(app.location.fragment, "destination");
}
