//! Real template replacement notification drives a scheduled mixed App stamp.
use super::*;
#[test]
fn content_observation_actual_app_replacement_callback_paints_native_controls_svg_and_links() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "chezcat-content-observation-app-{}.html",
        std::process::id()
    )));
    std::fs::write(&fixture.0,r##"<!doctype html><html><head><title>Observer ready</title><style>
html,body{margin:0;background:#ffffff}#run{position:absolute;left:20px;top:20px;width:180px;height:32px}#neighbor{position:absolute;left:240px;top:20px;width:180px;height:32px}
#out{position:absolute;left:20px;top:100px;width:180px;height:40px;color:#123456;font-size:24px}#native{position:absolute;left:240px;top:100px;width:180px;height:32px}#vector{position:absolute;left:20px;top:170px;width:40px;height:40px}#link{display:block;position:absolute;left:200px;top:170px;width:180px;height:36px;background:#204060}
</style></head><body><button id=run type=button>Dispatch event</button><input id=neighbor value=seed><div id=mount></div><div id=destination></div><script>
var trace='';document.getElementById('run').onclick=function(e){if(!e.isTrusted)throw 'native trigger required';var t=document.createElement('template');t.innerHTML='<span>Old inert</span>';var replacement='<original-shell><style>#out{color:#123456}</style><span id="out">Observed shell</span><input id="native" value="stamped"><svg id="vector" width="40" height="40" viewBox="0 0 10 10"><rect width="10" height="10" fill="#20a040"/></svg><a id="link" href="#destination">Observed link</a><template id="deferred"><b>Deferred</b></template></original-shell>';var content=t.content,old=content.firstChild,owner=content.ownerDocument,inside;var observer=new MutationObserver(function(records,self){if(trace!=='before|'||records.length!==1||records[0].target!==content||records[0].removedNodes[0]!==old||records[0].addedNodes[0]!==inside||self!==observer)throw 'genuine observer callback';trace+='observer|';var stamp=document.importNode(content,true);document.getElementById('mount').appendChild(stamp);var nested=document.getElementById('deferred');if(nested.content.isConnected||nested.content.ownerDocument!==owner||content.isConnected||old.parentNode!==null||inside.isConnected)throw 'actual owner boundary';document.title='Observer stamp delivered';});observer.observe(content,{childList:true});queueMicrotask(function(){trace+='before|';});t.innerHTML=replacement;inside=content.firstChild;Promise.resolve().then(function(){if(trace!=='before|observer|'||document.title!=='Observer stamp delivered')throw 'native scheduling order';trace+='after|';document.title='Observer stamp complete';});if(trace!=='')throw 'synchronous observer';};
</script></body></html>"##).unwrap();
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
        "Observer stamp complete",
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
    let link = id(&app, "link");
    assert_eq!(
        app.forms.controls[&id(&app, "native")].value.text(),
        "stamped"
    );
    assert_eq!(app.page.link_node_at(220, 185), Some(link));
    assert_eq!(app.page.link_at(220, 185), Some("#destination"));
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[(app.content_top() + 190 - app.scroll) as usize * canvas.width + 40]
            & 0xffffff,
        0x20a040
    );
    assert!((100..140).any(|y| (20..200).any(|x| canvas.pixels
        [(app.content_top() + y - app.scroll) as usize * canvas.width + x as usize]
        == 0x123456)));
    let y = app.content_top() + 185 - app.scroll;
    app.pointer_down(220, y, false);
    app.pointer_up(220, y);
    assert_eq!(app.location.fragment, "destination");
}
