//! Connected original App proof for native ARIA reflections and author accessors.
use super::*;
#[test]
fn aria_reflection_actual_app_reactive_attribute_jobs_paint_keep_editor_and_activate_native_link() {
    verify_native_document(false);
}

#[test]
fn html_document_structure_actual_app_implied_head_and_body_preserve_original_input_and_paint() {
    verify_native_document(true);
}

fn verify_native_document(omit_structure:bool) {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-aria-reflection-{}-{omit_structure}.html",
        std::process::id()
    )));
    let source=r#"<html><head><meta name="button-label" content="ready"><style>html,body{margin:0}#query{position:absolute;left:10px;top:10px;width:180px;height:30px}#go{position:absolute;left:10px;top:60px;width:120px;height:32px;background:#2040e0}#go[aria-label=ready]{background:#20a040}#out{position:absolute;left:10px;top:110px;width:200px;height:30px;color:#123456}#stable{position:absolute;display:block;left:10px;top:160px;width:100px;height:24px;background:#2040e0}</style></head><body><input id=query value=seed><button id=go type=button>Update</button><p id=out>waiting</p><a id=stable href=#destination></a><div id=destination></div><script>
if(document.documentElement.localName!=='html'||document.head.parentNode!==document.documentElement||document.body.parentNode!==document.documentElement||document.getElementsByTagName('meta')[0].parentNode!==document.head)throw 'actual document structure';
var button=document.getElementById('go'),d=Object.getOwnPropertyDescriptor(Element.prototype,'ariaLabel');if(button.hasOwnProperty('ariaLabel'))throw 'inherited native label';Object.defineProperty(button,'ariaLabel',{get:function(){return d.get.call(this);},set:function(value){d.set.call(this,value);},configurable:true});
var observer=new MutationObserver(function(records){if(records.length!==2||records[0].target!==button||records[0].attributeName!=='aria-label'||records[1].attributeName!=='aria-pressed'||records[0].oldValue!==null||records[1].oldValue!==null||button.ariaLabel!=='ready'||button.ariaPressed!=='true'||button.disabled)throw 'actual native attribute jobs';var output=document.getElementById('out');output.textContent='';output.insertAdjacentText('beforeend','ARIA ready');document.title='ARIA ready';});observer.observe(button,{attributes:true,attributeOldValue:true,attributeFilter:['aria-label','aria-pressed']});button.onclick=function(e){if(!e.isTrusted)throw 'native click';var meta=document.getElementsByTagName('meta')[0];if(meta.name!=='button-label'||Object.getPrototypeOf(meta)!==HTMLMetaElement.prototype)throw 'loaded metadata';button.ariaLabel=meta.content;button.ariaPressed=true;};
</script></body></html>"#;
    let source=if omit_structure {source.replace("<html><head>","").replace("</head><body>","").replace("</body></html>","")}else{source.into()};
    std::fs::write(&file.0,source).unwrap();
    let mut app = App::new();
    app.resize(480, 360);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some(name))
            .unwrap()
    };
    let query = id(&app, "query");
    let button = id(&app, "go");
    let stable = id(&app, "stable");
    let r = app.page.controls.iter().find(|r| r.node == query).unwrap();
    let (x, y) = (r.x + 10, app.content_top() + r.y + 10 - app.scroll);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    app.paste("edited");
    let v = &app.forms.controls[&query].value;
    let editor = (v.text(), v.cursor, v.anchor, v.offset);
    let r = app.page.controls.iter().find(|r| r.node == button).unwrap();
    let (x, y) = (r.x + 10, app.content_top() + r.y + 10 - app.scroll);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    assert_eq!(
        app.document.title,
        "ARIA ready",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    assert_eq!(app.document.attr(button, "aria-label"), Some("ready"));
    assert_eq!(app.document.attr(button, "aria-pressed"), Some("true"));
    let v = &app.forms.controls[&query].value;
    assert_eq!((v.text(), v.cursor, v.anchor, v.offset), editor);
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[(app.content_top() + 80 - app.scroll) as usize * canvas.width + 110]
            & 0xffffff,
        0x20a040
    );
    let top = (app.content_top() + 110 - app.scroll) as usize;
    assert!((top..top + 40)
        .any(|y| canvas.pixels[y * canvas.width + 10..y * canvas.width + 210].contains(&0x123456)));
    assert_eq!(app.page.link_node_at(25, 170), Some(stable));
    let y = app.content_top() + 170 - app.scroll;
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
