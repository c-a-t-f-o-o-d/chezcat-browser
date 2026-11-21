//! Original SVG bindings retain owner metadata across staging and real paint.
use super::*;
#[test]
fn svg_author_properties_actual_app_staging_metadata_mutation_jobs_paint_editor_and_link() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-svg-properties-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,r##"<html><head><style>html,body{margin:0;background:#fff}#neighbor{position:absolute;left:10px;top:10px;width:160px;height:30px}#run{position:absolute;left:10px;top:70px;width:160px;height:30px}#bound{position:absolute;left:10px;top:120px;width:48px;height:48px}#go{position:absolute;display:block;left:260px;top:10px;width:100px;height:30px;background:#2040e0}</style></head><body><input id=neighbor value=seed><button id=run type=button>Update binding</button><div id=mount></div><a id=go href=#destination>Continue</a><div id=destination></div><script>
var template=document.createElement('template');template.innerHTML='<svg id="bound" width="24" height="24" viewBox="0 0 24 24"><rect width="24" height="24" fill="red"></rect></svg>';var original=template.content.firstChild.firstChild;original.bindingState={held:original};var staged=document.importNode(template.content,true),root=staged.firstChild,node=root.firstChild;if(node.bindingState!==undefined)throw 'DOM import must not copy author metadata';node.bindingState={color:'red',held:node};Object.defineProperty(node,'bindingValue',{configurable:true,set:function(value){this.bindingState.color=value;this.setAttribute('fill',value);}});document.getElementById('mount').appendChild(staged);if(node.ownerDocument!==document||node.bindingState.held!==node)throw 'promoted native alias';var observer=new MutationObserver(function(records){if(records.length!==1||records[0].target!==node||records[0].attributeName!=='fill'||node.getAttribute('fill')!=='#123456'||node.bindingState.color!=='#123456')throw 'actual SVG binding mutation';document.title='SVG metadata active';});observer.observe(node,{attributes:true});document.getElementById('run').addEventListener('click',function(event){if(!event.isTrusted)throw 'native action';Promise.resolve().then(function(){node.bindingValue='#123456';});});document.title='SVG metadata ready';
</script></body></html>"##).unwrap();
    let mut app = App::new();
    app.resize(480, 360);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    assert_eq!(
        app.document.title,
        "SVG metadata ready",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
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
        let (x, y) = (r.x + 10, app.content_top() + r.y + 10 - app.scroll);
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
    };
    click(&mut app, neighbor);
    app.paste("edited");
    let v = &app.forms.controls[&neighbor].value;
    let editor = (v.text(), v.cursor, v.anchor, v.offset);
    click(&mut app, run);
    assert_eq!(
        app.document.title,
        "SVG metadata active",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let c = app.draw();
    assert_eq!(
        c.pixels[(app.content_top() + 140 - app.scroll) as usize * c.width + 30] & 0xffffff,
        0x123456
    );
    let v = &app.forms.controls[&neighbor].value;
    assert_eq!((v.text(), v.cursor, v.anchor, v.offset), editor);
    assert_eq!(app.page.link_node_at(280, 20), Some(go));
    let y = app.content_top() + 20 - app.scroll;
    app.pointer_down(280, y, false);
    app.pointer_up(280, y);
    assert_eq!(app.location.fragment, "destination");
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
}
