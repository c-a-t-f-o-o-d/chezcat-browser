//! Original DOM-layer composition for the selected noPatch configuration.
//! This is not execution of the deployed polyfill or native Custom Elements proof.
use super::*;
#[test]
fn selected_component_actual_app_nested_owner_wrappers_deferred_jobs_style_icon_focus_input_and_link(
) {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-selected-component-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,r##"<html><head><style>html,body{margin:0;background:#fff}#neighbor{position:absolute;left:400px;top:20px;width:160px;height:28px}</style></head><body><input id=neighbor value=seed><div id=mount></div><div id=destination></div><template id=source><demo-component id=widget><style>
#widget{display:block;position:absolute;left:10px;top:10px;width:360px;height:200px;background:#eee;--tone:#20a040}#widget:focus-within{background:#ddeeff}#query{position:absolute;left:10px;top:40px;width:150px;height:28px}#run{position:absolute;left:180px;top:40px;width:100px;height:28px}#out{position:absolute;left:10px;top:90px;width:220px;height:30px;background:var(--tone)}#icon{position:absolute;left:300px;top:90px;width:24px;height:24px}#widget .shape{fill:var(--tone)}#go{position:absolute;display:block;left:10px;top:150px;width:120px;height:24px;background:#2040e0}
</style><!--actual comment--><template><input id=query value=query-seed on-keydown=type><button id=run type=button on-click=activate>Activate</button><div id=out>ready</div><svg id=icon viewBox='0 0 24 24'><rect class=shape width=24 height=24></rect></svg><a id=go href=#destination>Continue</a></template></demo-component></template><script>
var configuration={force:true,noPatch:true,disableRuntime:true},cache=new WeakMap(),bindings=[],inertRecords=0,order=[],count=0,root,query,out;
var append=Node.prototype.appendChild,remove=Node.prototype.removeChild,replace=Node.prototype.replaceChild;
function wrap(node){var value=cache.get(node);if(!value){value={node:node,append:function(child){return append.call(this.node,child);},remove:function(child){return remove.call(this.node,child);},replace:function(next,old){return replace.call(this.node,next,old);}};cache.set(node,value);}return value;}
function prepare(fragment){var all=fragment.querySelectorAll('*');for(var i=0;i<all.length;i++){var node=all[i],attrs=node.attributes;for(var a=attrs.length-1;a>=0;a--){var attr=attrs[a],name=attr.name;if(name==='on-click'||name==='on-keydown'){bindings.push({id:node.getAttribute('id'),event:name.slice(3),handler:attr.value});node.removeAttribute(name);}}}var nested=fragment.querySelectorAll('template');for(var i=0;i<nested.length;i++){var node=nested[i];prepare(node.content);var part=node.ownerDocument.createDocumentFragment();while(node.content.firstChild)wrap(part).append(node.content.firstChild);if(wrap(node.parentNode).replace(part,node)!==node||node.parentNode!==null)throw 'real inert owner replacement';}}
var template=document.getElementById('source'),section=template.content.firstElementChild;
var inertObserver=new MutationObserver(function(records){for(var i=0;i<records.length;i++){if(records[i].target!==section||records[i].removedNodes.length!==1||records[i].removedNodes[0].localName!=='template')throw 'real inert preparation record';inertRecords++;}});inertObserver.observe(section,{childList:true});prepare(template.content);if(inertRecords!==0)throw 'inert callbacks ran synchronously';
Promise.resolve().then(function(){if(inertRecords!==1)throw 'deferred inert preparation';var imported=document.importNode(template.content,true),style=imported.querySelector('style'),copy=style.cloneNode(true);wrap(style.parentNode).remove(style);copy.textContent=copy.textContent+'#widget[data-ready]{border:0}';document.head.appendChild(copy);wrap(document.getElementById('mount')).append(imported);root=document.getElementById('widget');query=document.getElementById('query');out=document.getElementById('out');if(root.ownerDocument!==document||root.querySelector('template')!==null||root.querySelector('[on-click]')!==null||root.querySelector('[on-keydown]')!==null)throw 'real imported prepared nodes';
var aria=Object.getOwnPropertyDescriptor(Element.prototype,'ariaLabel');Object.defineProperty(root,'ariaLabel',{configurable:true,get:function(){return aria.get.call(this);},set:function(value){aria.set.call(this,value);}});root.ariaLabel='Original component';if(root.getAttribute('aria-label')!=='Original component')throw 'real author native forwarding';
var marker=document.createTextNode('0'),pending=[],serial=0;
var observer=new MutationObserver(function(records){if(records.length!==1||records[0].target!==marker||records[0].type!=='characterData')throw 'text scheduler record';order.push('observer');var jobs=pending;pending=[];for(var i=0;i<jobs.length;i++)jobs[i]();Promise.resolve().then(function(){order.push('promise');root.style.setProperty('--tone',count%2?'#e03040':'#2040d0');root.setAttribute('data-ready','yes');document.title='Lifecycle '+count;});});observer.observe(marker,{characterData:true});
function activate(event){if(!event.isTrusted)throw 'trusted native action';order.push('event');pending.push(function(){count++;var next=document.createTextNode('Action '+count);wrap(out).replace(next,out.firstChild);query.focus();});marker.data=String(++serial);if(order[order.length-1]!=='event')throw 'scheduler ran synchronously';}
var handlers={activate:activate,type:function(event){if(event.key==='Enter'){event.preventDefault();activate(event);}}};for(var i=0;i<bindings.length;i++){(function(binding){var target=document.getElementById(binding.id);target.addEventListener(binding.event,function(event){handlers[binding.handler](event);});})(bindings[i]);}
root.addEventListener('component-connected',function(event){if(event.isTrusted||event.target!==root)throw 'actual synthetic domain event';root.setAttribute('data-ready','yes');document.title='Lifecycle ready';});Promise.resolve().then(function(){root.dispatchEvent(new Event('component-connected'));});
});
</script></body></html>"##).unwrap();
    let mut app = App::new();
    app.resize(640, 420);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let report = || app.scripting.as_ref().unwrap().report();
    assert_eq!(app.document.title, "Lifecycle ready", "{}", report());
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        report()
    );
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| {
                app.document.attr(n, "id") == Some(name) && app.page.border_rects[n].is_some()
            })
            .unwrap()
    };
    let neighbor = id(&app, "neighbor");
    let query = id(&app, "query");
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
        "Lifecycle 1",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let pixel = |app: &mut App, x: usize, y: i32| {
        let y = (app.content_top() + y - app.scroll) as usize;
        let c = app.draw();
        c.pixels[y * c.width + x] & 0xffffff
    };
    assert_eq!(pixel(&mut app, 320, 110), 0xe03040);
    assert_eq!(pixel(&mut app, 20, 20), 0xddeeff);
    app.paste("typed");
    app.key(0x0d,false,false,false);
    assert_eq!(
        app.document.title,
        "Lifecycle 2",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    assert_eq!(pixel(&mut app, 320, 110), 0x2040d0);
    let result=app.scripting.as_mut().unwrap().console_eval(&mut app.document,&mut app.forms,"if(order.join(',')!=='event,observer,promise,event,observer,promise')throw 'order '+order.join(',');if(count!==2)throw 'count '+count;if(document.activeElement!==query)throw 'active '+document.activeElement.id;if(root.ariaLabel!=='Original component')throw 'author aria '+root.ariaLabel;");
    assert_eq!(
        result.errors,
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let v = &app.forms.controls[&neighbor].value;
    assert_eq!((v.text(), v.cursor, v.anchor, v.offset), editor);
    assert!(app.forms.controls[&query].value.text().contains("typed"));
    assert_eq!(app.page.link_node_at(40, 170), Some(go));
    let y = app.content_top() + 170 - app.scroll;
    app.pointer_down(40, y, false);
    app.pointer_up(40, y);
    assert_eq!(app.location.fragment, "destination");
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
}
