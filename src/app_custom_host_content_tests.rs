//! Original custom HTML host replacement drives native Range and observers.
use super::*;
#[test]
fn custom_host_content_actual_app_replacement_range_jobs_pixels_editor_and_link() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-custom-content-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,r##"<html><head><style>html,body{margin:0;background:#fff}#neighbor{position:absolute;left:10px;top:10px;width:160px;height:30px}#run{position:absolute;left:10px;top:70px;width:160px;height:30px}#bound{position:absolute;left:10px;top:120px;width:220px;height:30px}#go{position:absolute;display:block;left:260px;top:10px;width:100px;height:30px;background:#2040e0}</style></head><body><input id=neighbor value=seed><button id=run type=button>Update binding</button><div id=mount></div><a id=go href=#destination>Continue</a><div id=destination></div><script>
function wrapNativeEvent(e){var P=Object.create(Object.getPrototypeOf(e),{bindingType:{get:function(){return this.type;}}});Object.setPrototypeOf(e,P);if(!e.isTrusted||e.bindingType!==e.type||!(e instanceof Event))throw 'real trusted subclass';return e;}document.getElementById('neighbor').addEventListener('focus',function(e){wrapNativeEvent(e);if(!(e instanceof FocusEvent))throw 'real focus interface';document.title='Custom focus wrapped';});document.getElementById('go').addEventListener('click',function(e){wrapNativeEvent(e);if(!(e instanceof MouseEvent))throw 'real mouse interface';document.title='Custom link wrapped';});var template=document.createElement('template');template.innerHTML='<original-card id="bound">staged</original-card>';var staged=template.content,owner=staged.ownerDocument,root=owner.createElement('original-card');root.setAttribute('id','bound');staged.textContent='';staged.appendChild(root);root.textContent='Original ready';var icon=owner.createElementNS('http://www.w3.org/2000/svg','svg'),shape=owner.createElementNS('http://www.w3.org/2000/svg','rect'),sheet=owner.createElement('style');icon.setAttribute('id','made-icon');icon.setAttribute('width','20');icon.setAttribute('height','20');shape.setAttribute('width','20');shape.setAttribute('height','20');shape.setAttribute('fill','#a02050');icon.appendChild(shape);sheet.textContent='#made-icon{position:absolute;left:380px;top:120px;width:20px;height:20px}';staged.appendChild(sheet);staged.appendChild(icon);var originalRoot=root,importedTemplate=document.importNode(template,true);staged=importedTemplate.content;root=staged.firstChild;if(root===originalRoot||originalRoot.ownerDocument!==owner)throw 'actual clone source';var savedLeaf=root.firstChild;savedLeaf.bindingState={node:savedLeaf};document.getElementById('mount').appendChild(importedTemplate);document.getElementById('mount').appendChild(staged);if(importedTemplate.content!==staged||root.firstChild!==savedLeaf||savedLeaf.ownerDocument!==document||savedLeaf.bindingState.node!==savedLeaf||!savedLeaf.isConnected)throw 'actual inert leaf activation';var old=root.firstChild,range=new Range();range.selectNodeContents(old);var observer=new MutationObserver(function(records){if(records.length!==1||records[0].target!==root||records[0].removedNodes[0]!==old||records[0].addedNodes[0]!==root.firstChild||root.textContent!=='Original active'||old.parentNode!==null||range.startContainer!==root||range.startOffset||!range.collapsed)throw 'actual custom host mutation';document.title='Custom content active';});observer.observe(root,{childList:true});document.getElementById('run').addEventListener('click',function(event){if(!event.isTrusted)throw 'native action';Promise.resolve().then(function(){root.textContent='Original active';root.style.backgroundColor='#123456';});});document.title='Custom content ready';
</script></body></html>"##).unwrap();
    let mut app = App::new();
    app.resize(480, 360);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    assert_eq!(
        app.document.title,
        "Custom content ready",
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
    assert_eq!(app.document.title,"Custom focus wrapped","{}",app.scripting.as_ref().unwrap().report());
    app.paste("edited");
    let v = &app.forms.controls[&neighbor].value;
    let editor = (v.text(), v.cursor, v.anchor, v.offset);
    click(&mut app, run);
    assert_eq!(
        app.document.title,
        "Custom content active",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let c = app.draw();
    assert_eq!(
        c.pixels[(app.content_top() + 140 - app.scroll) as usize * c.width + 200] & 0xffffff,
        0x123456
    );
    assert_eq!(
        c.pixels[(app.content_top() + 130 - app.scroll) as usize * c.width + 390] & 0xffffff,
        0xa02050
    );
    let v = &app.forms.controls[&neighbor].value;
    assert_eq!((v.text(), v.cursor, v.anchor, v.offset), editor);
    assert_eq!(app.page.link_node_at(280, 20), Some(go));
    let y = app.content_top() + 20 - app.scroll;
    let (_,hit)=app.primary_anchor_down(280,y,false);
    let (_,default)=app.release_anchor_pointer(hit,280,y,false,crate::scripting::AnchorMouseInput::default());
    let Some(crate::app::anchor::AnchorDefault::Link(target,_))=default else {panic!("actual native link default")};
    app.open_link_target(target);
    assert_eq!(app.location.fragment, "destination");
    assert_eq!(app.document.title,"Custom link wrapped","{}",app.scripting.as_ref().unwrap().report());
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
}
