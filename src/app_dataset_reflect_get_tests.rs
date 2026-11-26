//! Dataset native mutation observations feed actual App paint and preserve adjacent controls.
use super::*;
#[test]
fn dataset_reflect_get_actual_app_trusted_task_observer_paint_editor_and_link_targets() {
    use std::io::{Read,Write};
    let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();listener.set_nonblocking(true).unwrap();
    let url=format!("http://127.0.0.1:{}/",listener.local_addr().unwrap().port());
    let worker=std::thread::spawn(move || {
      let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);
      loop {match listener.accept(){
       Ok((mut stream,_))=>{stream.set_nonblocking(false).unwrap();stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();let mut request=[0;8192];let n=stream.read(&mut request).unwrap();assert!(n>0);let body=r#"<!doctype html><html><head><title>Dataset own keys ready</title><style>
html,body{margin:0;background:#ffffff}#run{position:absolute;left:20px;top:20px;width:180px;height:32px}#neighbor{position:absolute;left:240px;top:20px;width:180px;height:32px}
#out{position:absolute;left:20px;top:100px;width:400px;height:40px;color:#123456;font-size:24px}#link{display:block;position:absolute;left:20px;top:180px;width:180px;height:36px;background:#204060}
</style></head><body><button id=run type=button>Dispatch event</button><input id=neighbor value=seed><div id=out>Original</div><a id=link href=#destination>Real link</a><div id=destination></div><script>
var out=document.getElementById('out');
document.getElementById('run').onclick=function(e){
 if(!e.isTrusted)throw 'native trigger required';
 var getter=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'dataset').get,data=getter.call(out);var symbolWatch=new MutationObserver(function(){});symbolWatch.observe(out,{attributes:true});var symbolKey=Symbol('native'),symbolValue={retained:true};data[symbolKey]=symbolValue;if(data[symbolKey]!==symbolValue||Object.getOwnPropertySymbols(data)[0]!==symbolKey||Object.keys(data).length||symbolWatch.takeRecords().length)throw 'actual native Symbol separation';delete data[symbolKey];symbolWatch.disconnect();if(!Object.isExtensible(data)||Object.isSealed(data)||Object.isFrozen(data))throw 'actual native integrity queries';for(var integrityMethod of ['preventExtensions','seal','freeze']){var integrityError;try{Object[integrityMethod](data);}catch(e){integrityError=e;}if(!(integrityError instanceof TypeError))throw 'actual native integrity refusal';}if(!Reflect.set(data,'raw\ud800Name','native\udfff'))throw 'actual native definition result';var receiverObject={label:'actual receiver'};if(Reflect.get(data,'raw\ud800Name',receiverObject)!=='native\udfff')throw 'actual native data receiver';Object.defineProperty(DOMStringMap.prototype,'receiverProbe',{get:function(){if(this!==receiverObject)throw 'actual inherited receiver';return this.label;},configurable:true});if(Reflect.get(data,'receiverProbe',receiverObject)!=='actual receiver')throw 'actual prototype read';delete DOMStringMap.prototype.receiverProbe;var loopNames=[];for(var loopKey in data)loopNames.push(loopKey);if(loopNames.length!==1||loopNames[0]!=='raw\ud800Name')throw 'native for-in actual task';if(Object.keys(data)[0]!=='raw\ud800Name'||Object.getOwnPropertyNames(data)[0]!=='raw\ud800Name'||Reflect.ownKeys(data)[0]!=='raw\ud800Name'||Object.values(data)[0]!=='native\udfff'||Object.entries(data)[0][0]!=='raw\ud800Name'||Object.assign({},data)['raw\ud800Name']!=='native\udfff')throw 'actual own snapshot consumers';if(data['raw\ud800Name']!=='native\udfff'||data['raw\ufffdName']!==undefined)throw 'actual raw dataset read';if(!('raw\ud800Name' in data)||!Object.hasOwn(data,'raw\ud800Name')||!Object.prototype.propertyIsEnumerable.call(data,'raw\ud800Name'))throw 'actual raw presence';var desc=Object.getOwnPropertyDescriptor(data,'raw\ud800Name');if(!desc||desc.value!=='native\udfff'||!desc.writable||!desc.enumerable||!desc.configurable)throw 'actual raw descriptor';var rawWatch=new MutationObserver(function(){});rawWatch.observe(out,{attributes:true,attributeOldValue:true});if(!delete data['raw\ud800Name']||Object.hasOwn(data,'raw\ud800Name'))throw 'actual raw removal';var rawRecords=rawWatch.takeRecords();if(rawRecords.length!==1||rawRecords[0].target!==out||rawRecords[0].attributeName!=='data-raw\ud800-name'||rawRecords[0].oldValue!=='native\udfff')throw 'actual raw native record';rawWatch.disconnect();if(data!==out.dataset||Object.hasOwn(out,'dataset'))throw 'actual canonical getter';if(!(data instanceof DOMStringMap)||Object.getPrototypeOf(data)!==DOMStringMap.prototype||data.constructor!==DOMStringMap)throw 'actual native interface';DOMStringMap.prototype.authorLabel='inherited';if(data.authorLabel!=='inherited'||out.hasAttribute('data-author-label'))throw 'author prototype is not backing';
 var watch=new MutationObserver(function(records){if(records.length!==2||records[0].target!==out||records[0].attributeName!=='data-status'||records[0].oldValue!==null||records[1].oldValue!=='Native dataset'||out.hasAttribute('data-status'))throw 'actual dataset native records';out.textContent='Native dataset';document.title='Dataset own keys delivered';});watch.observe(out,{attributes:true,attributeOldValue:true});out.dataset.status='Native dataset';delete out.dataset.status;

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
    let control = app.page.controls.iter().find(|c| c.node == run).unwrap();
    let (x, y) = (
        control.x + 12,
        app.content_top() + control.y + 12 - app.scroll,
    );
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    assert_eq!(app.document.title, "Dataset own keys delivered","{}",app.scripting.as_ref().unwrap().report());
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
