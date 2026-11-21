//! Original real App ownership controls; no native window/tab-strip acceptance.
use super::*;
use std::fs;
use std::io::{Read,Write};
use std::net::{TcpListener,SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool,Ordering};
use std::sync::{mpsc,Condvar};
use std::thread;

struct Temp(PathBuf);
impl Temp {
    fn new()->Self{
        let mut bytes=[0u8;8];crate::random::fill(&mut bytes).unwrap();
        let token:String=bytes.iter().map(|b|format!("{b:02x}")).collect();
        let path=std::env::temp_dir().join(format!("chezcat-session-core-{token}"));fs::create_dir(&path).unwrap();
        // Exercise a genuine filesystem alias when short names are available.
        // Keep input free of device prefixes; the browser canonicalizes it.
        #[cfg(windows)]
        let path={
            use std::os::windows::ffi::{OsStrExt,OsStringExt};
            #[link(name="kernel32")]
            unsafe extern "system" {fn GetShortPathNameW(long:*const u16,short:*mut u16,size:u32)->u32;}
            let units:Vec<u16>=path.as_os_str().encode_wide().chain(Some(0)).collect();
            let mut short=vec![0u16;32768];
            let size=unsafe{GetShortPathNameW(units.as_ptr(),short.as_mut_ptr(),short.len() as u32)} as usize;
            if size>0 && size<short.len(){PathBuf::from(std::ffi::OsString::from_wide(&short[..size]))}else{path}
        };
        Self(path)
    }
    fn page(&self,name:&str,title:&str)->PathBuf{
        let path=self.0.join(name);fs::write(&path,format!("<!doctype html><html><head><title>{title}</title><style>body{{margin:0}}input,button{{display:block}}#spacer{{height:1800px}}</style></head><body><input id=q value=seed><button id=change type=button onclick=\"count++;document.title='{title} '+count;\">Change</button><p id=out>Actual original page</p><div id=spacer>Scroll</div><script>var count=0;</script></body></html>")).unwrap();path
    }
}
impl Drop for Temp {
    fn drop(&mut self){if let Ok(metadata)=fs::metadata(self.0.join("history.txt")){if metadata.permissions().readonly(){let mut permissions=metadata.permissions();permissions.set_readonly(false);let _=fs::set_permissions(self.0.join("history.txt"),permissions);}}let _=fs::remove_dir_all(&self.0);}
}
fn adopt(app:App,limits:Limits)->Session{match Session::adopt(Box::new(app),limits){Ok(session)=>session,Err((_app,error))=>panic!("{error}")}}
fn first(session:&Session)->(WindowId,TabId){let window=session.window_ids().next().unwrap();(window,session.active(window).unwrap().unwrap())}
fn node(app:&App,name:&str)->usize{(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap()}
fn point(app:&App,name:&str)->(i32,i32){let node=node(app,name);let rect=app.page.controls.iter().find(|rect|rect.node==node).unwrap();(rect.x+rect.width/2,app.content_top()+rect.y-app.scroll+rect.height/2)}
fn edit(app:&mut App,text:&str){assert!(!app.failed,"Fixture page failed: {}",app.status);let(x,y)=point(app,"q");app.pointer_down(x,y,false);app.pointer_up(x,y);app.key(0x41,true,false,false);app.paste(text);assert_eq!(app.forms.controls[&node(app,"q")].value.text(),text);}
fn tap(app:&mut App,name:&str){let(x,y)=point(app,name);app.pointer_down(x,y,false);app.pointer_up(x,y);}
fn until(session:&mut Session,mut ready:impl FnMut(&mut Session)->bool){let deadline=Instant::now()+Duration::from_secs(15);while !ready(session){session.poll(4).unwrap();assert!(Instant::now()<deadline,"Real session ownership fixture timed out.");thread::sleep(Duration::from_millis(2));}}
fn received()->crate::http::Response{crate::http::Response{document_timing:None,redirected:false,url:crate::url::parse("https://download.invalid/session.bin").unwrap().0,fragment:None,status:200,status_text:"OK".into(),encoded_body_bytes:b"browser download".len(),protocol:Some("TLS 1.3"),request_is_post:true,cookie_same_site:false,headers:vec![("content-disposition".into(),"attachment; filename=session.bin".into())],body:b"browser download".to_vec(),events:Vec::new()}}

#[test]
fn session_core_real_tabs_keep_independent_forms_vm_source_scroll_and_back_history(){
    let temp=Temp::new();let a=temp.page("a.html","A");let b=temp.page("b.html","B");let later=temp.page("later.html","Later");
    let mut initial=App::new();initial.open(&a);edit(&mut initial,"edited A");tap(&mut initial,"change");initial.scroll_by(170);
    let a_location=initial.location.clone();let raw=initial.raw.clone();let epoch=initial.script_epoch;let nodes=initial.document.nodes.as_ptr();let scroll=initial.scroll;
    let mut session=adopt(initial,Limits::default());let(window,one)=first(&session);let two=session.new_tab(window).unwrap();
    let b_location=session.with_tab(two,|app|{assert!(app.scripting.is_none());assert_eq!(app.history.len(),1);app.open(&b);edit(app,"edited B");tap(app,"change");app.location.clone()}).unwrap();
    session.switch_tab(window,one).unwrap();session.with_active(window,|app|{assert_eq!(app.raw,raw);assert_eq!(app.script_epoch,epoch);assert_eq!(app.document.nodes.as_ptr(),nodes);assert_eq!(app.scroll,scroll);assert_eq!(app.document.title,"A 1");assert_eq!(app.forms.controls[&node(app,"q")].value.text(),"edited A");}).unwrap();
    session.switch_tab(window,two).unwrap();session.with_active(window,|app|{assert_eq!(app.document.title,"B 1");assert_eq!(app.forms.controls[&node(app,"q")].value.text(),"edited B");app.open(&later);app.mouse_navigation(false);assert_eq!(app.location,b_location);assert_eq!(app.history.len(),3);}).unwrap();
    session.switch_tab(window,one).unwrap();session.with_active(window,|app|{assert_eq!(app.document.title,"A 1");assert_eq!(app.history.len(),2);assert_eq!(app.raw,raw);}).unwrap();
    // Compare actual shared jars/trust without replacing each App's own origin.
    let jar=session.cookies.clone();let trust=session.trust.clone();session.with_tab(one,|app|{assert!(Arc::ptr_eq(&app.cookies,&jar));assert!(Arc::ptr_eq(&app.trust,&trust));assert_eq!(app.location,a_location);}).unwrap();
    session.with_tab(two,|app|{assert!(Arc::ptr_eq(&app.cookies,&jar));assert!(Arc::ptr_eq(&app.trust,&trust));assert_eq!(app.location,b_location);}).unwrap();
}

#[test]
fn session_core_switch_cancels_old_input_capture_without_resetting_page_state(){
    let temp=Temp::new();let page=temp.page("capture.html","Capture");let mut initial=App::new();initial.open(&page);
    let mut session=adopt(initial,Limits::default());let(window,one)=first(&session);
    let (x,y)=session.with_tab(one,|app|{let p=point(app,"change");app.pointer_down(p.0,p.1,false);p}).unwrap();
    let two=session.new_tab(window).unwrap();session.switch_tab(window,one).unwrap();session.with_active(window,|app|{app.pointer_up(x,y);assert_eq!(app.document.title,"Capture");assert!(matches!(app.drag,Drag::None));}).unwrap();
    assert_eq!(session.active(window).unwrap(),Some(one));assert_ne!(one,two);
}

#[test]
fn session_core_close_active_refreshes_real_neighbor_viewport_and_shared_observations(){
    let temp=Temp::new();let page=temp.page("neighbor.html","Neighbor");let other=temp.page("other.html","Other");
    let mut initial=App::new();initial.open(&page);edit(&mut initial,"kept");let nodes=initial.document.nodes.as_ptr();let epoch=initial.script_epoch;
    let mut session=adopt(initial,Limits::default());let(window,one)=first(&session);let two=session.new_tab(window).unwrap();session.resize_window(window,1400,900).unwrap();
    let mut profile=session.profile.view().unwrap();let mut data=profile.data().clone();data.settings.dark_mode=true;profile.save(data).unwrap();
    session.history.configure(crate::history::Policy{enabled:true,retention_days:None,clear_on_exit:false},crate::history::now()).unwrap();
    session.with_tab(two,|app|app.open(&other)).unwrap();
    let mut downloads=session.downloads.as_ref().unwrap().view().unwrap();downloads.start_received(received(),&temp.0.join("downloads"),session.trust.clone()).unwrap();
    let deadline=Instant::now()+Duration::from_secs(5);while session.downloads.as_ref().unwrap().active_count().unwrap()!=0{session.downloads.as_ref().unwrap().poll().unwrap();assert!(Instant::now()<deadline);thread::sleep(Duration::from_millis(2));}
    session.with_tab(one,|app|{assert_ne!(app.width,1400);assert!(!app.profile.data().settings.dark_mode);assert!(app.downloads.entries().is_empty());assert!(!app.browsing_history.data().policy.enabled);}).unwrap();
    session.close_tab(two).unwrap();assert_eq!(session.active(window).unwrap(),Some(one));
    session.with_active(window,|app|{assert_eq!((app.width,app.height),(1400,900));assert!(app.profile.data().settings.dark_mode);assert_eq!(app.downloads.entries().len(),1);assert!(app.browsing_history.data().policy.enabled);assert_eq!(app.browsing_history.data().entries.len(),1);assert_eq!(app.document.nodes.as_ptr(),nodes);assert_eq!(app.script_epoch,epoch);assert_eq!(app.forms.controls[&node(app,"q")].value.text(),"kept");assert_eq!(app.document.title,"Neighbor");}).unwrap();
}

#[test]
fn session_core_checked_nonreused_ids_caps_foreign_sessions_and_stale_wakes(){
    let mut session=adopt(App::new(),Limits{tabs:2,windows:2,wakes:8,poll_tabs:1});let(window,one)=first(&session);let old_tab_wake=session.tab_wake(one).unwrap();
    let two=session.new_tab(window).unwrap();assert!(session.new_tab(window).is_err());let before=session.next_tab;
    session.close_tab(one).unwrap();let three=session.new_tab(window).unwrap();assert!(three.value()>two.value());assert!(session.with_tab(one,|_|()).is_err());assert!(session.new_window(500,400).is_err());assert_eq!(session.next_tab,before+1);
    session.close_tab(two).unwrap();let(other,tab)=session.new_window(500,400).unwrap();assert!(session.switch_tab(window,tab).is_err());
    let old_window_wake=session.window_wake(window).unwrap();session.close_window(window).unwrap();assert!(session.with_tab(three,|_|()).is_err());assert_eq!(session.active(other).unwrap(),Some(tab));
    while session.poll(1).unwrap().more{}old_tab_wake();old_window_wake();let report=session.poll(1).unwrap();assert_eq!(report.stale_wakes,2);
    session.next_tab=u64::MAX;assert!(session.new_tab(other).is_err());assert_eq!(session.tab_count(),1);session.next_window=u64::MAX;assert!(session.new_window(1,1).is_err());
    let mut foreign=adopt(App::new(),Limits::default());let(foreign_window,foreign_tab)=first(&foreign);
    assert_eq!(tab.value(),4);assert_eq!(foreign_tab.value(),1);assert_ne!(window,foreign_window);
    assert!(foreign.with_tab(one,|_|panic!("Foreign tab callback ran.")).is_err());assert!(foreign.close_tab(one).is_err());assert!(foreign.switch_tab(foreign_window,one).is_err());assert!(foreign.resize_window(window,1,1).is_err());assert!(foreign.close_window(window).is_err());assert_eq!(foreign.active(foreign_window).unwrap(),Some(foreign_tab));
}

#[test]
fn session_core_bounded_overflow_poll_serves_real_inactive_timers_without_hot_loop(){
    let temp=Temp::new();let mut session=adopt(App::new(),Limits{tabs:3,windows:1,wakes:1,poll_tabs:1});let(window,one)=first(&session);let two=session.new_tab(window).unwrap();let three=session.new_tab(window).unwrap();
    for (id,name) in [(one,"One"),(two,"Two"),(three,"Three")] {
        let path=temp.0.join(format!("{name}.html"));fs::write(&path,format!("<title>{name} waiting</title><p>{name}</p><script>setTimeout(function(){{document.title='{name} done';}},0);</script>")).unwrap();session.with_tab(id,|app|app.open(&path)).unwrap();
    }
    thread::sleep(Duration::from_millis(2));let mut visits=0;loop{let report=session.poll(1).unwrap();assert!(report.changed.len()<=1);visits+=1;if !report.more{break;}assert!(visits<=12,"Mailbox continuation did not converge.");}
    for (id,name) in [(one,"One"),(two,"Two"),(three,"Three")] {session.with_tab(id,|app|assert_eq!(app.document.title,format!("{name} done"))).unwrap();}
}

#[derive(Debug,PartialEq)]
enum PeerRead { Accepted, PrefixConsumed, Failed, Held }

struct SplitHeader {
    prefix_len:usize,
    prefix_written:mpsc::Receiver<()>,
    notice:mpsc::Sender<PeerRead>,
}

struct Site {address:SocketAddr,stopped:Arc<AtomicBool>,requests:Arc<Mutex<Vec<String>>>,held:Arc<Mutex<Vec<String>>>,finished:Arc<Mutex<Vec<String>>>,read_errors:Arc<Mutex<Vec<String>>>,gate:Arc<(Mutex<bool>,Condvar)>,worker:Option<thread::JoinHandle<()>>}
impl Site {
    fn new(kind:&'static str)->Self{Self::with_split_header(kind,None)}
    fn with_split_header(kind:&'static str,mut probe:Option<SplitHeader>)->Self{
        let listener=TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();listener.set_nonblocking(true).unwrap();
        let stopped=Arc::new(AtomicBool::new(false));let requests=Arc::new(Mutex::new(Vec::new()));let held=Arc::new(Mutex::new(Vec::new()));let finished=Arc::new(Mutex::new(Vec::new()));let gate=Arc::new((Mutex::new(false),Condvar::new()));
        let read_errors=Arc::new(Mutex::new(Vec::new()));let errors=read_errors.clone();
        let(stop,seen,holding,done,release)=(stopped.clone(),requests.clone(),held.clone(),finished.clone(),gate.clone());let worker=thread::spawn(move||{
            let mut peers=Vec::new();while !stop.load(Ordering::Relaxed){match listener.accept(){Ok((mut stream,_))=>{assert!(peers.len()<32,"Fixture connection bound exceeded.");let seen=seen.clone();let holding=holding.clone();let done=done.clone();let release=release.clone();let errors=errors.clone();let probe=probe.take();peers.push(thread::spawn(move||{
                // Only the listener polls. An accepted Winsock peer inherits its nonblocking mode.
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(15))).unwrap();stream.set_write_timeout(Some(Duration::from_secs(15))).unwrap();
                if let Some(probe)=&probe {
                    if probe.notice.send(PeerRead::Accepted).is_err() || probe.prefix_written.recv_timeout(Duration::from_secs(15)).is_err(){return;}
                }
                let mut request=Vec::new();let mut byte=[0u8;1];
                while !request.ends_with(b"\r\n\r\n") {
                    let read=if request.len()==8192 {Err(std::io::ErrorKind::InvalidData)} else {stream.read_exact(&mut byte).map_err(|e|e.kind())};
                    if let Err(kind)=read {
                        let prefix=String::from_utf8_lossy(&request[..request.len().min(128)]);
                        let path=prefix.lines().next().and_then(|line|line.split_whitespace().nth(1));
                        errors.lock().unwrap().push(format!("header bytes={}, path={path:?}, error={kind:?}",request.len()));
                        if let Some(probe)=&probe {let _=probe.notice.send(PeerRead::Failed);}
                        return;
                    }
                    request.push(byte[0]);
                    if let Some(probe)=&probe {
                        if request.len()==probe.prefix_len {let _=probe.notice.send(PeerRead::PrefixConsumed);}
                    }
                }
                let text=String::from_utf8_lossy(&request);let path=text.lines().next().and_then(|line|line.split_whitespace().nth(1)).unwrap_or("/").to_owned();seen.lock().unwrap().push(path.clone());
                let (mime,body)=if path=="/" {
                    let content=match kind {"sheet"=>"<script>var sheet=document.createElement('link');sheet.rel='stylesheet';sheet.href='/held.css';sheet.onload=function(){document.title='LATE';};document.head.appendChild(sheet);</script>","script"=>"<script>var s=document.createElement('script');s.src='/held.js';document.body.appendChild(s);</script>","image"=>"<img src='/held.ppm' onload=\"document.title='LATE'\">","background"=>"<style>#box{background-image:url('/held.ppm')}</style>",_=>""};
                    ("text/html",format!("<!doctype html><html><head><title>Held owner</title><style>#box{{width:32px;height:32px;background:#008844}}</style></head><body><div id=box>Owner</div>{content}</body></html>").into_bytes())
                }else if path=="/held.css"{("text/css",b"#box{width:900px;background:#ff0000}".to_vec())}
                else if path=="/held.js"{("text/javascript",b"document.title='LATE';location.href='/escaped';".to_vec())}
                else if path=="/held.bin"{("application/octet-stream",b"browser held download".to_vec())}
                else if path=="/held.ppm"{let mut data=b"P6\n2 2\n255\n".to_vec();data.extend([0,0,255].repeat(4));("image/x-portable-pixmap",data)}else{("text/plain",b"Not a fixture route".to_vec())};
                let header=format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self'; object-src 'none'\r\n\r\n",body.len());let _=stream.write_all(header.as_bytes());
                if path.starts_with("/held."){
                    holding.lock().unwrap().push(path.clone());
                    if let Some(probe)=&probe {let _=probe.notice.send(PeerRead::Held);}
                    let(open,_)=release.1.wait_timeout_while(release.0.lock().unwrap(),Duration::from_secs(30),|open|!*open).unwrap();
                    // Timeout is fixture failure, never implicit publication.
                    if !*open{return;}drop(open);
                }
                let _=stream.write_all(&body);
                done.lock().unwrap().push(path);
            }));},Err(error) if error.kind()==std::io::ErrorKind::WouldBlock=>thread::sleep(Duration::from_millis(1)),Err(error)=>panic!("{error}")}}
            for peer in peers{peer.join().unwrap();}
        });Self{address,stopped,requests,held,finished,read_errors,gate,worker:Some(worker)}
    }
    fn url(&self,path:&str)->String{format!("http://{}{path}",self.address)}
    fn seen(&self,path:&str)->bool{self.requests.lock().unwrap().iter().any(|p|p==path)}
    fn held(&self,path:&str)->bool{self.held.lock().unwrap().iter().any(|p|p==path)}
    fn finished(&self,path:&str)->bool{self.finished.lock().unwrap().iter().any(|p|p==path)}
    fn observations(&self)->String {
        format!("requests={:?}, held={:?}, finished={:?}, read errors={:?}",
            self.requests.lock().unwrap(),self.held.lock().unwrap(),self.finished.lock().unwrap(),self.read_errors.lock().unwrap())
    }
    fn release(&self){*self.gate.0.lock().unwrap()=true;self.gate.1.notify_all();}
}
impl Drop for Site{fn drop(&mut self){self.release();self.stopped.store(true,Ordering::Relaxed);if let Some(worker)=self.worker.take(){worker.join().unwrap();}}}

#[test]
fn session_held_peer_reads_split_image_request_before_holding_and_releasing_body() {
    let path="/held.ppm";
    let prefix=b"GET /held.ppm HTTP/1.1\r\nHost: localhost\r\n";
    let (prefix_written,admit_read)=mpsc::channel();
    let (notice,events)=mpsc::channel();
    let site=Site::with_split_header("image",Some(SplitHeader {prefix_len:prefix.len(),prefix_written:admit_read,notice}));
    let mut client=std::net::TcpStream::connect(site.address).unwrap();
    let timeout=Duration::from_secs(5);
    client.set_read_timeout(Some(timeout)).unwrap();
    client.set_write_timeout(Some(timeout)).unwrap();
    assert_eq!(events.recv_timeout(timeout),Ok(PeerRead::Accepted),"{}",site.observations());
    client.write_all(prefix).unwrap();
    prefix_written.send(()).unwrap();
    assert_eq!(events.recv_timeout(timeout),Ok(PeerRead::PrefixConsumed),"{}",site.observations());
    // The peer has consumed exactly the first fragment. Withhold the final
    // blank line and observe its next real socket read before completing it.
    assert_eq!(events.recv_timeout(Duration::from_millis(200)),Err(mpsc::RecvTimeoutError::Timeout),"{}",site.observations());
    assert!(!site.seen(path)&&!site.held(path)&&!site.finished(path),"{}",site.observations());
    client.write_all(b"\r\n").unwrap();
    assert_eq!(events.recv_timeout(timeout),Ok(PeerRead::Held),"{}",site.observations());
    let expected=b"HTTP/1.1 200 OK\r\nContent-Type: image/x-portable-pixmap\r\nContent-Length: 23\r\nConnection: close\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self'; object-src 'none'\r\n\r\n";
    let mut header=vec![0;expected.len()];
    client.read_exact(&mut header).unwrap();
    assert_eq!(header.as_slice(),expected);
    assert!(site.seen(path)&&site.held(path)&&!site.finished(path),"{}",site.observations());
    site.release();
    let mut body=Vec::new();
    client.take(24).read_to_end(&mut body).unwrap();
    assert_eq!(body,b"P6\n2 2\n255\n\0\0\xff\0\0\xff\0\0\xff\0\0\xff");
    assert!(site.finished(path),"{}",site.observations());
}

#[test]
fn session_core_close_consumes_held_script_stylesheet_image_and_background_owners(){
    for(kind,resource)in[("sheet","/held.css"),("script","/held.js"),("image","/held.ppm"),("background","/held.ppm")]{
        let temp=Temp::new();let survivor=temp.page("survivor.html","Survivor");let site=Site::new(kind);let mut session=adopt(App::new(),Limits::default());let(window,closing)=first(&session);let alive=session.new_tab(window).unwrap();
        session.with_tab(alive,|app|{app.open(&survivor);edit(app,"survives");let x=app.width-20;let y=app.content_top()+app.viewport_height()-24;app.pointer_down(x,y,false);app.pointer_up(x,y);assert!(matches!(app.focus,Focus::Page));}).unwrap();
        session.with_tab(closing,|app|app.open_address(&site.url("/"))).unwrap();
        if kind=="image" {
            // Deliberately let the parser resource arrive before the UI polls
            // its queued first paint. Request arrival is not document commit.
            let deadline=Instant::now()+Duration::from_secs(15);
            while !site.held(resource){assert!(Instant::now()<deadline,"{kind}: parser resource did not arrive. {}",site.observations());thread::sleep(Duration::from_millis(2));}
        }
        until(&mut session,|session|site.held(resource)&&session.with_tab(closing,|app|!app.failed&&app.document.title=="Held owner").unwrap());
        session.with_tab(closing,|app|assert_eq!(app.document.title,"Held owner")).unwrap();
        assert!(!site.finished(resource),"{kind}: held response completed before close.");
        let retired:Vec<Box<dyn Fn()->bool>>=session.with_tab(closing,|app|{
            let mut probes:Vec<Box<dyn Fn()->bool>>=Vec::new();
            if let Some(worker)=&app.worker{probes.push(Box::new(worker.retirement_probe()));}
            if let Some(worker)=&app.dynamic_scripts{probes.push(Box::new(worker.retirement_probe()));}
            if let Some(worker)=&app.dynamic_stylesheets{probes.push(Box::new(worker.retirement_probe()));}
            if let Some(worker)=&app.dynamic_images{probes.push(Box::new(worker.retirement_probe()));}
            assert!(!probes.is_empty()&&probes.iter().all(|probe|!probe()));probes
        }).unwrap();
        let before=session.with_tab(alive,|app|(app.raw.clone(),app.script_epoch,app.document.nodes.as_ptr(),app.draw().pixels.clone(),app.forms.controls[&node(app,"q")].value.text().to_owned())).unwrap();let stale=session.tab_wake(closing).unwrap();
        session.close_tab(closing).unwrap();assert!(session.with_tab(closing,|_|()).is_err());let replacement=session.new_tab(window).unwrap();assert_ne!(replacement,closing);site.release();stale();
        until(&mut session,|_|site.finished(resource)&&retired.iter().all(|probe|probe()));
        let deadline=Instant::now()+Duration::from_secs(15);
        while session.poll(4).unwrap().more{assert!(Instant::now()<deadline,"{kind}: retired wakes did not drain.");}
        assert!(!site.seen("/escaped"),"A retired script performed navigation.");session.with_tab(replacement,|app|{assert_ne!(app.document.title,"LATE");assert!(app.styles.is_empty());assert!(app.backgrounds.is_empty());}).unwrap();
        session.with_tab(alive,|app|{assert_eq!(app.raw,before.0);assert_eq!(app.script_epoch,before.1);assert_eq!(app.document.nodes.as_ptr(),before.2);assert_eq!(app.draw().pixels,before.3);assert_eq!(app.forms.controls[&node(app,"q")].value.text(),before.4);assert_eq!(app.document.title,"Survivor");}).unwrap();
    }
}

#[test]
fn session_core_adopts_live_download_authority_without_cancelling_on_tab_close(){
    let temp=Temp::new();let site=Site::new("download");let mut app=App::new();let mut data=app.profile.data().clone();data.settings.download_folder=Some(temp.0.join("downloads").display().to_string());app.profile.save(data).unwrap();
    let location=navigation::address(&site.url("/held.bin"),&Location::home()).unwrap();app.download_location(location,None);
    let deadline=Instant::now()+Duration::from_secs(5);while !site.seen("/held.bin"){assert!(Instant::now()<deadline);thread::sleep(Duration::from_millis(2));}
    let mut session=adopt(app,Limits::default());let(window,tab)=first(&session);session.close_tab(tab).unwrap();assert_eq!(session.downloads.as_ref().unwrap().active_count().unwrap(),1);site.release();until(&mut session,|session|session.downloads.as_ref().unwrap().active_count().unwrap()==0);
    let replacement=session.new_tab(window).unwrap();session.with_tab(replacement,|app|{assert_eq!(app.downloads.entries().len(),1);let entry=&app.downloads.entries()[0];assert_eq!(entry.status,crate::downloads::Status::Complete);assert_eq!(fs::read(entry.path.as_ref().unwrap()).unwrap(),b"browser held download");}).unwrap();
}

#[test]
fn session_core_consumed_media_owner_retires_source_tokens_without_notifications(){
    let mut app=App::new();app.open(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools/fixtures/authored-video.html"));app.wait_for_load();let snapshot=app.embedded_video_snapshot().unwrap();let retired=snapshot.retired.clone();let tasks=snapshot.tasks.clone();
    assert!(!retired.load(Ordering::Relaxed));let mut session=adopt(app,Limits::default());let(_,tab)=first(&session);session.close_tab(tab).unwrap();assert!(retired.load(Ordering::Relaxed));assert!(!tasks.lock().unwrap().has_pending());assert_eq!(session.tab_count(),0);
}

#[test]
fn session_core_shared_history_clears_only_explicit_final_shutdown_and_retries_failure(){
    let temp=Temp::new();let page=temp.page("history.html","History");let file=temp.0.join("history.txt");let mut store=crate::history::Store::open(file.clone()).unwrap();store.configure(crate::history::Policy{enabled:true,retention_days:None,clear_on_exit:true},crate::history::now()).unwrap();
    let mut app=App::new();app.attach_browsing_history(store);app.open(&page);let mut session=adopt(app,Limits::default());let(window,one)=first(&session);let two=session.new_tab(window).unwrap();session.with_tab(two,|app|app.open(&page)).unwrap();let before=fs::read(&file).unwrap();
    session.close_tab(one).unwrap();session.close_tab(two).unwrap();assert_eq!(fs::read(&file).unwrap(),before);assert_eq!(crate::history::Store::open(file.clone()).unwrap().data().entries.len(),2);
    let original=fs::metadata(&file).unwrap().permissions();let mut readonly=original.clone();readonly.set_readonly(true);fs::set_permissions(&file,readonly).unwrap();assert!(session.shutdown(crate::history::now()).is_err());assert_eq!(session.tab_count(),0);assert!(session.new_window(1,1).is_err());assert_eq!(fs::read(&file).unwrap(),before);
    fs::set_permissions(&file,original).unwrap();let done=session.shutdown(crate::history::now()).unwrap();assert_eq!(done.history_removed,2);assert!(crate::history::Store::open(file.clone()).unwrap().data().entries.is_empty());let after=fs::read(&file).unwrap();assert_eq!(session.shutdown(crate::history::now()).unwrap().history_removed,0);assert_eq!(fs::read(&file).unwrap(),after);
}
