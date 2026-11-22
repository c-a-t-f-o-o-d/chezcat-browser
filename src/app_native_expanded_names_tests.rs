//! Actual namespace-created, cloned SVG local-reference paint and mutation.
use super::*;
#[test]
fn native_expanded_names_actual_app_prefixed_html_svg_clone_paint_editor_and_link() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let fixture = Fixture(
        std::env::temp_dir().join(format!("chezcat-expanded-names-svg-{}.html", std::process::id())),
    );
    std::fs::write(&fixture.0,r##"<!doctype html><html><head><title>Vector ready</title><style>
html,body{margin:0;background:#ffffff}#run{position:absolute;left:20px;top:20px;width:180px;height:32px}#neighbor{position:absolute;left:240px;top:20px;width:180px;height:32px}#change{position:absolute;left:20px;top:65px;width:180px;height:32px}#toggle{position:absolute;left:240px;top:65px;width:180px;height:32px}#mount{position:absolute;left:20px;top:120px;width:80px;height:80px}#safe{display:block;position:absolute;left:200px;top:120px;width:180px;height:50px;background:#204060}
</style></head><body><button id=run type=button>Build vector</button><input id=neighbor value=seed><button id=change type=button>Change vector</button><button id=toggle type=button>Toggle vector</button><div id=mount></div><a id=safe href=#destination>Real link</a><div id=destination></div><script>
var painted,host;document.getElementById('run').onclick=function(e){if(!e.isTrusted)throw 'native trigger required';var ns='http://www.w3.org/2000/svg',xl='http://www.w3.org/1999/xlink',xn='http://www.w3.org/2000/xmlns/',svg=document.createElementNS(ns,'s:svg'),defs=document.createElementNS(ns,'s:defs'),path=document.createElementNS(ns,'s:path'),use=document.createElementNS(ns,'s:use');svg.setAttributeNS(xn,'xmlns',ns);svg.setAttributeNS(xn,'xmlns:xlink',xl);svg.setAttribute('viewBox','0 0 20 20');svg.setAttribute('width','80');svg.setAttribute('height','80');path.setAttribute('id','shape');path.setAttribute('d','M0 0 H20 V20 H0 Z');path.setAttribute('fill','#e03040');defs.appendChild(path);use.setAttributeNS(xl,'xlink:href','#shape');svg.appendChild(defs);svg.appendChild(use);painted=svg.cloneNode(true);if(painted.namespaceURI!==ns||painted.querySelector('path')===path)throw 'real vector copy';host=document.createElementNS('http://www.w3.org/1999/xhtml','h:div');host.style.width='80px';host.style.height='80px';host.appendChild(painted);document.getElementById('mount').appendChild(host);if(host.prefix!=='h'||host.localName!=='div'||host.tagName!=='H:DIV'||host.namespaceURI!=='http://www.w3.org/1999/xhtml'||painted.prefix!=='s'||painted.localName!=='svg'||painted.tagName!=='s:svg'||painted.querySelector('path').prefix!=='s'||painted.ownerDocument!==document)throw 'actual connected expanded names';document.title='Vector stamped';};document.getElementById('change').onclick=function(e){if(!e.isTrusted)throw 'native mutation required';painted.querySelector('path').setAttribute('fill','#40a020');document.title='Vector changed';};document.getElementById('toggle').onclick=function(e){if(!e.isTrusted)throw 'native toggle required';if(painted.isConnected){painted.parentNode.removeChild(painted);document.title='Vector removed';}else{host.appendChild(painted);if(painted.parentNode!==host||host.prefix!=='h'||painted.prefix!=='s')throw 'same native parent and names';document.title='Vector restored';}};
</script></body></html>"##).unwrap();
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    assert!(!app.failed && !app.is_loading());
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some(name))
            .unwrap()
    };
    let click = |app: &mut App, node: usize| {
        let c = app.page.controls.iter().find(|c| c.node == node).unwrap();
        let (x, y) = (c.x + 12, app.content_top() + c.y + 12 - app.scroll);
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
    };
    let neighbor = id(&app, "neighbor");
    click(&mut app, neighbor);
    app.paste("native neighbor");
    let value = &app.forms.controls[&neighbor].value;
    let saved = (value.text(), value.cursor, value.anchor, value.offset);
    let run = id(&app, "run");
    click(&mut app, run);
    assert_eq!(
        app.document.title,
        "Vector stamped",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let color = |app: &mut App| {
        let canvas = app.draw();
        canvas.pixels[(app.content_top() + 140 - app.scroll) as usize * canvas.width + 40]
            & 0xffffff
    };
    assert_eq!(color(&mut app), 0xe03040);
    let change = id(&app, "change");
    click(&mut app, change);
    assert_eq!(
        app.document.title,
        "Vector changed",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    assert_eq!(color(&mut app), 0x40a020);
    let toggle = id(&app, "toggle");
    click(&mut app, toggle);
    assert_eq!(app.document.title, "Vector removed", "{}", app.scripting.as_ref().unwrap().report());
    assert_eq!(color(&mut app), 0xffffff);
    assert!(app.inline_svg_sources.is_empty());
    click(&mut app, toggle);
    assert_eq!(app.document.title, "Vector restored");
    assert_eq!(color(&mut app), 0x40a020);
    let value = &app.forms.controls[&neighbor].value;
    assert_eq!(
        (value.text(), value.cursor, value.anchor, value.offset),
        saved
    );
    let link = id(&app, "safe");
    assert_eq!(app.page.link_node_at(220, 140), Some(link));
    let y = app.content_top() + 140 - app.scroll;
    app.pointer_down(220, y, false);
    app.pointer_up(220, y);
    assert_eq!(app.location.fragment, "destination");
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0);
}
