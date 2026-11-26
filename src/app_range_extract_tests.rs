//! Actual App selected native extraction feeds paint and preserves adjacent controls.
use super::*;
#[test]
fn range_extract_actual_app_native_identity_moves_drive_paint_editor_and_link_targets() {
    use std::io::{Read,Write};
    let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();listener.set_nonblocking(true).unwrap();
    let url=format!("http://127.0.0.1:{}/",listener.local_addr().unwrap().port());
    let worker=std::thread::spawn(move || {
      let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);
      loop {match listener.accept(){
       Ok((mut stream,_))=>{stream.set_nonblocking(false).unwrap();stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();let mut request=[0;8192];let n=stream.read(&mut request).unwrap();assert!(n>0);let body=r#"<!doctype html><html><head><title>Range extraction ready</title><style>
html,body{margin:0;background:#ffffff}#run{position:absolute;left:20px;top:20px;width:180px;height:32px}#neighbor{position:absolute;left:240px;top:20px;width:180px;height:32px}
#out{position:absolute;left:20px;top:100px;width:400px;height:40px;color:#123456;font-size:24px}#link{display:block;position:absolute;left:20px;top:180px;width:180px;height:36px;background:#204060}
</style></head><body><button id=run type=button>Dispatch event</button><input id=neighbor value=seed><div id=out>Original</div><a id=link href=#destination>Real link</a><div id=destination></div><script>
var out=document.getElementById('out');
document.getElementById('run').onclick=function(e){
 if(!e.isTrusted)throw 'native trigger required';
 var root=document.createElement('section');root.innerHTML='<b>Before</b><span>Moved</span><em>After</em>';out.appendChild(root);var first=root.firstChild.firstChild,last=root.lastChild.firstChild,moved=root.childNodes[1],range=document.createRange();range.setStart(first,2);range.setEnd(last,2);var fragment=range.extractContents();if(first.data!=='Be'||last.data!=='ter'||fragment.childNodes[1]!==moved||moved.parentNode!==fragment||fragment.ownerDocument!==document||!range.collapsed||range.startContainer!==root||range.startOffset!==1)throw 'native selected extraction';out.textContent=fragment.textContent;document.title='Range extraction delivered';
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
    assert_eq!(app.document.title, "Range extraction delivered","{}",app.scripting.as_ref().unwrap().report());
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
