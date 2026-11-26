//! Passive noscript markup replacement retains actual records and App state.
use super::*;
#[test]
fn noscript_inner_html_actual_app_trusted_markup_records_paint_editor_and_link_targets() {
    use std::io::{Read,Write};
    let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();listener.set_nonblocking(true).unwrap();
    let url=format!("http://127.0.0.1:{}/",listener.local_addr().unwrap().port());
    let worker=std::thread::spawn(move || {
      let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);
      loop {match listener.accept(){
       Ok((mut stream,_))=>{stream.set_nonblocking(false).unwrap();stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();let mut request=[0;8192];let n=stream.read(&mut request).unwrap();assert!(n>0);let body=r#"<!doctype html><html><head><title>Noscript context ready</title><noscript id=head-fallback><script>window.fallbackExecuted=1;</script><meta http-equiv=Content-Security-Policy content="default-src none"><base href="http://127.0.0.1:1/poison/"></noscript><style>
html,body{margin:0;background:#ffffff}#run{position:absolute;left:20px;top:20px;width:180px;height:32px}#neighbor{position:absolute;left:240px;top:20px;width:180px;height:32px}
#out{position:absolute;left:20px;top:100px;width:400px;height:40px;color:#123456;font-size:24px}#link{display:block;position:absolute;left:20px;top:180px;width:180px;height:36px;background:#204060}
</style></head><body><noscript id=body-fallback><button id=fake>Fallback control</button><style>#out{color:#ff00ff;left:500px}</style><script>window.fallbackExecuted=2;</script></noscript><div id=fragment-root></div><button id=run type=button>Dispatch event</button><input id=neighbor value=seed><div id=out>Original</div><a id=link href=#destination>Real link</a><div id=destination></div><script>
var out=document.getElementById('out');window.allowed=7;
document.getElementById('run').onclick=function(e){
 if(!e.isTrusted)throw 'native trigger required';
 if(window.allowed!==7||window.fallbackExecuted!==undefined||document.getElementById('fake')!==null||document.querySelectorAll('script').length!==1||document.getElementById('body-fallback').firstChild.nodeType!==3||!document.getElementById('head-fallback').textContent.includes('Content-Security-Policy')||out.textContent!=='Original')throw 'captured native noscript context';var mount=document.getElementById('fragment-root'),watch=new MutationObserver(function(){});watch.observe(mount,{childList:true});var raw='<button id=fragment-fake>Fake</button><style>#out{color:#ff00ff;left:500px}</style><img src=missing><scr'+'ipt>window.fragmentExecuted=1;</scr'+'ipt>&amp;';mount.innerHTML='<noscript id=fragment-fallback>'+raw+'</noscript>';var fallback=document.getElementById('fragment-fallback'),records=watch.takeRecords();if(!fallback||fallback.firstChild.nodeType!==3||fallback.textContent!==raw||fallback.innerHTML!==raw||fallback.outerHTML!=='<noscript id="fragment-fallback">'+raw+'</noscript>'||document.getElementById('fragment-fake')!==null||document.querySelectorAll('noscript').length!==3||document.querySelectorAll('script').length!==1||window.fragmentExecuted!==undefined||records.length!==1||records[0].target!==mount||records[0].removedNodes.length||records[0].addedNodes.length!==1||records[0].addedNodes[0]!==fallback)throw 'actual fragment raw data and mutation records';var text=fallback.firstChild;mount.innerHTML='<noscript id=second-fallback>'+raw+'</noscript>';var second=document.getElementById('second-fallback');records=watch.takeRecords();if(!second||fallback.parentNode!==null||fallback.firstChild!==text||text.parentNode!==fallback||fallback.textContent!==raw||fallback.innerHTML!==raw||fallback.ownerDocument!==document||document.getElementById('fragment-fallback')!==null||records.length!==1||records[0].removedNodes[0]!==fallback||records[0].addedNodes[0]!==second)throw 'retired actual raw host';mount.appendChild(fallback);records=watch.takeRecords();if(document.getElementById('fragment-fallback')!==fallback||fallback.firstChild!==text||records.length!==1||records[0].addedNodes[0]!==fallback)throw 'reconnected actual raw host';mount.removeChild(fallback);records=watch.takeRecords();if(fallback.parentNode!==null||fallback.firstChild!==text||records.length!==1||records[0].removedNodes[0]!==fallback||document.querySelectorAll('noscript').length!==3||document.querySelectorAll('script').length!==1||window.fragmentExecuted!==undefined||out.textContent!=='Original')throw 'removed raw host remained passive';var shallow=fallback.cloneNode(false),clone=fallback.cloneNode(true);if(shallow===fallback||clone===fallback||shallow.childNodes.length||clone.firstChild===text||clone.firstChild.parentNode!==clone||clone.firstChild.data!==raw||clone.textContent!==raw||clone.innerHTML!==raw||clone.ownerDocument!==document||shallow.ownerDocument!==document||watch.takeRecords().length)throw 'actual independent passive clones';clone.id='clone-fallback';mount.appendChild(clone);records=watch.takeRecords();if(document.getElementById('clone-fallback')!==clone||records.length!==1||records[0].addedNodes[0]!==clone||document.querySelectorAll('noscript').length!==4||document.querySelectorAll('script').length!==1||window.fragmentExecuted!==undefined)throw 'passive clone connection';mount.removeChild(clone);records=watch.takeRecords();if(clone.parentNode!==null||clone.firstChild.data!==raw||records.length!==1||records[0].removedNodes[0]!==clone||document.querySelectorAll('noscript').length!==3)throw 'actual clone retirement';var inert=document.implementation.createHTMLDocument('donor'),donor=inert.createElement('noscript');donor.appendChild(inert.createTextNode(raw));donor.id='donor';var imported=document.importNode(donor,true),bare=inert.createElement('noscript');bare.appendChild(inert.createTextNode(raw));var bareCopy=document.importNode(bare,true);if(imported===donor||imported.ownerDocument!==document||imported.firstChild.ownerDocument!==document||imported.firstChild===donor.firstChild||imported.firstChild.data!==raw||bareCopy.firstChild.data!==raw||bareCopy.ownerDocument!==document||donor.ownerDocument!==inert||watch.takeRecords().length)throw 'actual inert import owner and raw data';imported.id='imported-fallback';mount.appendChild(imported);records=watch.takeRecords();donor.firstChild.data='Changed';if(imported.firstChild.data!==raw||document.getElementById('imported-fallback')!==imported||records.length!==1||records[0].addedNodes[0]!==imported||document.querySelectorAll('noscript').length!==4||document.querySelectorAll('script').length!==1||window.fragmentExecuted!==undefined)throw 'passive imported connection and independence';mount.removeChild(imported);records=watch.takeRecords();if(imported.parentNode!==null||imported.firstChild.data!==raw||records.length!==1||records[0].removedNodes[0]!==imported||document.querySelectorAll('noscript').length!==3)throw 'actual imported retirement';var original=second.firstChild,raw2=raw+' literal replacement';watch.observe(second,{childList:true});second.textContent=raw2;var replacement=second.firstChild;records=watch.takeRecords();if(replacement===original||replacement.nodeType!==3||replacement.data!==raw2||second.innerHTML!==raw2||original.parentNode!==null||original.data!==raw||records.length!==1||records[0].target!==second||records[0].removedNodes[0]!==original||records[0].addedNodes[0]!==replacement)throw 'actual passive text replacement';second.textContent=null;records=watch.takeRecords();if(second.firstChild!==null||replacement.parentNode!==null||replacement.data!==raw2||records.length!==1||records[0].removedNodes[0]!==replacement||records[0].addedNodes.length)throw 'actual nullable clear';second.textContent=raw;var restored=second.firstChild;records=watch.takeRecords();if(records.length!==1||records[0].addedNodes[0]!==restored||restored.data!==raw)throw 'new actual Text after clear';var prior=restored;second.innerHTML=raw+' markup';restored=second.firstChild;records=watch.takeRecords();if(restored===prior||restored.nodeType!==3||restored.data!==raw+' markup'||second.innerHTML!==raw+' markup'||prior.parentNode!==null||prior.data!==raw||records.length!==1||records[0].target!==second||records[0].removedNodes[0]!==prior||records[0].addedNodes[0]!==restored)throw 'actual connected raw markup replacement';prior=restored;second.innerHTML=raw;restored=second.firstChild;records=watch.takeRecords();if(restored===prior||restored.data!==raw||prior.parentNode!==null||prior.data!==raw+' markup'||records.length!==1||records[0].removedNodes[0]!==prior||records[0].addedNodes[0]!==restored)throw 'actual connected markup restore';watch.observe(second,{childList:true,characterData:true,characterDataOldValue:true,subtree:true});restored.data=raw+' changed';records=watch.takeRecords();if(second.firstChild!==restored||restored.parentNode!==second||second.innerHTML!==raw+' changed'||records.length!==1||records[0].type!=='characterData'||records[0].target!==restored||records[0].oldValue!==raw||records[0].addedNodes.length||records[0].removedNodes.length)throw 'actual connected character edit';restored.replaceData(0,restored.length,raw);records=watch.takeRecords();if(restored.data!==raw||records.length!==1||records[0].target!==restored||records[0].oldValue!==raw+' changed')throw 'actual connected character splice';mount.textContent='';records=watch.takeRecords();if(mount.childNodes.length||second.parentNode!==null||second.firstChild!==restored||restored.parentNode!==second||restored.data!==raw||records.length!==1||records[0].target!==mount||records[0].removedNodes[0]!==second||records[0].addedNodes.length||document.querySelectorAll('noscript').length!==2||document.querySelectorAll('script').length!==1||document.getElementById('fragment-fake')!==null||window.fragmentExecuted!==undefined||out.textContent!=='Original')throw 'actual parent text retirement remains passive';document.title='Noscript markup preserved';

};
</script></body></html>"#;write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();return;},
       Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{assert!(std::time::Instant::now()<deadline,"loopback request timeout");std::thread::sleep(std::time::Duration::from_millis(2));},
       Err(e)=>panic!("loopback: {e}")
      }}
    });
    let mut app = App::new();
    app.resize(640, 360);
    app.set_javascript_enabled(true);
    app.open_address(&url);app.wait_for_load();worker.join().unwrap();
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
    let before_data=app.draw();
    let control = app.page.controls.iter().find(|c| c.node == run).unwrap();
    let (x, y) = (
        control.x + 12,
        app.content_top() + control.y + 12 - app.scroll,
    );
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    assert_eq!(app.document.title, "Noscript markup preserved","{}",app.scripting.as_ref().unwrap().report());
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
    for y in 100..140 { for x in 20..420 {
        let at=(app.content_top()+y-app.scroll) as usize*canvas.width+x as usize;
        assert_eq!(canvas.pixels[at],before_data.pixels[at],"noscript raw markup must not alter rendered content");
    }}
    assert!((100..140).any(|y| (20..420).any(|x| canvas.pixels
        [(app.content_top() + y - app.scroll) as usize * canvas.width + x as usize]
        == 0x123456)));
}
