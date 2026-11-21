//! Original App proof for reactive author overrides over genuine HTML slots.
use super::*;
#[test]
fn html_reflector_actual_app_reactive_hidden_preserves_editor_and_native_link() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-html-reflector-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,r#"<html><head><style>html,body{margin:0}#query{position:absolute;left:10px;top:10px;width:120px;height:30px}#toggle{position:absolute;left:150px;top:10px;width:80px;height:30px}#menu{position:absolute;left:10px;top:70px;width:100px;height:40px;background:#2040e0}</style></head><body><input id=query value=seed><button id=toggle type=button>Toggle</button><a id=menu hidden href=#destination></a><div id=destination></div><script>
var menu=document.getElementById('menu'),d=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'hidden'),calls=0;
if(menu.hasOwnProperty('hidden'))throw 'inherited native property';
Object.defineProperty(menu,'hidden',{get:function(){return d.get.call(this);},set:function(v){calls++;d.set.call(this,v);},configurable:true});
document.getElementById('toggle').onclick=function(){menu.hidden=!menu.hidden;menu.title='reactive';document.title='changed '+calls;};
</script></body></html>"#).unwrap();
    let mut app = App::new();
    app.resize(480, 320);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some(name))
            .unwrap()
    };
    let query = id(&app, "query");
    let toggle = id(&app, "toggle");
    let menu = id(&app, "menu");
    assert_ne!(app.page.link_node_at(25, 85), Some(menu));
    let r = app.page.controls.iter().find(|r| r.node == query).unwrap();
    let (x, y) = (r.x + 10, app.content_top() + r.y + 10 - app.scroll);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    app.paste("rust");
    let v = &app.forms.controls[&query].value;
    let editor = (v.text(), v.cursor, v.anchor, v.offset);
    let r = app.page.controls.iter().find(|r| r.node == toggle).unwrap();
    let (x, y) = (r.x + 10, app.content_top() + r.y + 10 - app.scroll);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    assert_eq!(
        app.document.title,
        "changed 1",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    assert_eq!(app.document.attr(menu, "title"), Some("reactive"));
    assert_eq!(app.document.attr(menu, "hidden"), None);
    assert_eq!(app.page.link_node_at(25, 85), Some(menu));
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[(app.content_top() + 85 - app.scroll) as usize * canvas.width + 25]
            & 0xffffff,
        0x2040e0
    );
    let v = &app.forms.controls[&query].value;
    assert_eq!((v.text(), v.cursor, v.anchor, v.offset), editor);
    let y = app.content_top() + 85 - app.scroll;
    app.pointer_down(25, y, false);
    app.pointer_up(25, y);
    assert_eq!(app.location.fragment, "destination");
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
}
