//! Actual App acceptance of nested template clone/import in a trusted callback.
use super::*;
#[test]
fn domparser_template_publication_actual_app_callback_paints_fields_and_keeps_editor_and_link_targets() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "chezcat-domparser-template-publication-app-{}.html",
        std::process::id()
    )));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Template import ready</title><style>
html,body{margin:0;background:#ffffff}#run{position:absolute;left:20px;top:20px;width:180px;height:32px}#neighbor{position:absolute;left:240px;top:20px;width:180px;height:32px}
#out{position:absolute;left:20px;top:100px;width:400px;height:40px;color:#123456;font-size:24px}#link{display:block;position:absolute;left:20px;top:180px;width:180px;height:36px;background:#204060}
</style></head><body><button id=run type=button>Dispatch event</button><input id=neighbor value=seed><div id=out>Original</div><a id=link href=#destination>Real link</a><div id=destination></div><script>
var out=document.getElementById('out');
document.getElementById('run').onclick=function(e){if(!e.isTrusted)throw 'native trigger required';var parsed=new DOMParser().parseFromString('<body><template id="source"><original-shell><style>#out{color:#123456}</style><span>Actual template stamp</span><template id="deferred"><b>Deferred</b></template></original-shell></template></body>','text/html'),t=parsed.getElementById('source');var host=document.importNode(t,true),sourceOwner=t.content.ownerDocument,content=host.content,owner=content.ownerDocument;if(host.ownerDocument!==document||owner===sourceOwner||owner===document||content===t.content||t.ownerDocument!==parsed)throw 'real parsed template destination owner';out.appendChild(host);if(!host.isConnected||content.isConnected||out.querySelector('span')!==null)throw 'host activated inert content';var stamp=document.importNode(content,true);out.textContent='';out.appendChild(stamp);var nested=out.querySelector('#deferred');if(nested.ownerDocument!==document||nested.content.ownerDocument!==owner||nested.content.textContent!=='Deferred'||nested.content.isConnected)throw 'nested appropriate owner';t.innerHTML='<span>Changed source</span>';if(out.querySelector('span').textContent!=='Actual template stamp')throw 'source alias';document.title='Template import delivered';};
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
    let original = (app.raw.clone(),app.location.clone(),app.script_epoch);
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
    assert_eq!(app.document.title, "Template import delivered","{}",app.scripting.as_ref().unwrap().report());
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
    assert_eq!(app.page.link_node_at(40, 195), Some(link));
    assert_eq!(app.page.link_at(40, 195), Some("#destination"));
    assert_eq!((app.raw.clone(),app.location.clone(),app.script_epoch),original);
    let canvas = app.draw();
    assert!((100..140).any(|y| (20..420).any(|x| canvas.pixels
        [(app.content_top() + y - app.scroll) as usize * canvas.width + x as usize]
        == 0x123456)));
}
