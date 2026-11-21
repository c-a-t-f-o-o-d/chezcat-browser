//! Genuine inert namespaced SVG import and Attr callback update actual pixels.
use super::*;
#[test]
fn attribute_notifications_actual_app_inert_namespace_import_alias_callback_and_real_paint() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "chezcat-attribute-notifications-{}.html",
        std::process::id()
    )));
    std::fs::write(&fixture.0,r##"<!doctype html><html><head><title>Vector ready</title><style>
html,body{margin:0;background:#ffffff}#run{position:absolute;left:20px;top:20px;width:180px;height:32px}#neighbor{position:absolute;left:240px;top:20px;width:180px;height:32px}#change{position:absolute;left:20px;top:65px;width:180px;height:32px}#toggle{position:absolute;left:240px;top:65px;width:180px;height:32px}#mount{position:absolute;left:20px;top:120px;width:80px;height:80px}#safe{display:block;position:absolute;left:200px;top:120px;width:180px;height:50px;background:#204060}
</style></head><body><button id=run type=button>Build vector</button><input id=neighbor value=seed><button id=change type=button>Change vector</button><button id=toggle type=button>Toggle vector</button><div id=mount></div><a id=safe href=#destination>Real link</a><div id=destination></div><script>
var painted;var observed;document.getElementById('run').onclick=function(e){if(!e.isTrusted)throw 'native trigger required';var ns='http://www.w3.org/2000/svg',xl='http://www.w3.org/1999/xlink',xn='http://www.w3.org/2000/xmlns/',owner=document.implementation.createHTMLDocument('icons'),svg=owner.createElementNS(ns,'svg'),defs=owner.createElementNS(ns,'defs'),path=owner.createElementNS(ns,'path'),use=owner.createElementNS(ns,'use');svg.setAttributeNS(xn,'xmlns',ns);svg.setAttributeNS(xn,'xmlns:xlink',xl);svg.setAttribute('viewBox','0 0 20 20');svg.setAttribute('width','80');svg.setAttribute('height','80');path.setAttribute('id','shape');path.setAttribute('d','M0 0 H20 V20 H0 Z');path.setAttribute('fill','#e03040');defs.appendChild(path);use.setAttributeNS(xl,'xlink:href','#shape');svg.appendChild(defs);svg.appendChild(use);painted=document.importNode(svg,true);if(painted.namespaceURI!==ns||painted.querySelector('path')===path)throw 'real vector copy';document.getElementById('mount').appendChild(painted);document.title='Vector stamped';var actual=painted.querySelector('path');observed=new MutationObserver(function(records){if(records.length!==1||records[0].target!==actual||records[0].attributeName!=='fill'||records[0].oldValue!=='#e03040')throw 'actual alias callback';document.title='Vector changed';});observed.observe(actual,{attributes:true,attributeOldValue:true,attributeFilter:['fill']});};document.getElementById('change').onclick=function(e){if(!e.isTrusted)throw 'native mutation required';painted.querySelector('path').getAttributeNode('fill').value='#40a020';if(document.title!=='Vector stamped')throw 'synchronous alias observer';};document.getElementById('toggle').onclick=function(e){if(!e.isTrusted)throw 'native toggle required';if(painted.isConnected){painted.parentNode.removeChild(painted);document.title='Vector removed';}else{document.getElementById('mount').appendChild(painted);document.title='Vector restored';}};
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
    assert_eq!(
        app.document.title,
        "Vector removed",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
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
