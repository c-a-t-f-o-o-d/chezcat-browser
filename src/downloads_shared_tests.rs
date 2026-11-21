//! Original adapter controls. All fixtures are local owned files or loopback HTTP.
use super::*;
use super::super::{Status, MAX_ACTIVE, MAX_ENTRIES};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let mut bytes=[0u8;8]; crate::random::fill(&mut bytes).unwrap();
        let token:String=bytes.iter().map(|b|format!("{b:02x}")).collect();
        let path=std::env::temp_dir().join(format!("chezcat-shared-download-{token}"));
        fs::create_dir(&path).unwrap(); Self(fs::canonicalize(path).unwrap())
    }
    fn folder(&self)->PathBuf { self.0.join("Downloads") }
    fn files(&self)->Vec<PathBuf> {
        if !self.folder().exists() {return Vec::new();}
        let mut files:Vec<_>=fs::read_dir(self.folder()).unwrap().map(|e|e.unwrap().path()).collect();
        files.sort(); files
    }
}
impl Drop for Temp {fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
fn trust()->Arc<trust::Store> {Arc::new(trust::Store::unconfigured())}
fn source(path:&Path)->Location {
    Location {path:Some(path.to_owned()),url:None,fragment:String::new(),root:Some(path.parent().unwrap().to_owned())}
}
fn complete(shared:&Shared) {
    let deadline=Instant::now()+Duration::from_secs(5);
    loop {
        shared.poll().unwrap();
        if shared.active_count().unwrap()==0 {break;}
        assert!(Instant::now()<deadline,"Shared download did not complete.");
        thread::sleep(Duration::from_millis(2));
    }
}
struct Held {
    location:Location,
    observed:mpsc::Receiver<()>,
    release:Option<mpsc::Sender<()>>,
    thread:Option<thread::JoinHandle<()>>,
}
impl Held {
    fn new(expected:usize)->Self {Self::with_accepted_notice(expected,None)}
    fn with_accepted_notice(expected:usize,accepted:Option<mpsc::Sender<()>>)->Self {
        let listener=TcpListener::bind("127.0.0.1:0").unwrap();
        let location=super::super::web_location(crate::url::parse(&format!("http://{}/payload.bin",listener.local_addr().unwrap())).unwrap().0);
        listener.set_nonblocking(true).unwrap();
        let (send,observed)=mpsc::channel(); let (release,gate)=mpsc::channel();
        let thread=thread::spawn(move|| {
            let deadline=Instant::now()+Duration::from_secs(5);let mut streams=Vec::new();
            while streams.len()<expected && Instant::now()<deadline {
                let (mut stream,_)=match listener.accept() {
                    Ok(peer)=>peer,
                    Err(error) if error.kind()==std::io::ErrorKind::WouldBlock=>{thread::sleep(Duration::from_millis(1));continue;},
                    Err(error)=>panic!("Owned download peer failed: {error}"),
                };
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                // The regression peer withholds request bytes until this notice.
                if let Some(notice)=&accepted {notice.send(()).unwrap();}
                let mut request=Vec::new();let mut byte=[0u8;1];
                while !request.ends_with(b"\r\n\r\n") {
                    assert!(request.len()<4096);stream.read_exact(&mut byte).unwrap();request.push(byte[0]);
                }
                assert!(request.starts_with(b"GET /payload.bin HTTP/1.1\r\n"));
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 7\r\nConnection: close\r\n\r\n").unwrap();
                streams.push(stream);send.send(()).unwrap();
            }
            assert_eq!(streams.len(),expected,"Missing owned HTTP download requests.");
            let _=gate.recv_timeout(Duration::from_secs(5));
            for mut stream in streams {let _=stream.write_all(b"payload");}
        });
        Self {location,observed,release:Some(release),thread:Some(thread)}
    }
    fn wait(&self,count:usize){for _ in 0..count {self.observed.recv_timeout(Duration::from_secs(3)).unwrap();}}
    fn release(&mut self){if let Some(send)=self.release.take(){let _=send.send(());}if let Some(thread)=self.thread.take(){thread.join().unwrap();}}
}
impl Drop for Held {
    fn drop(&mut self){
        // Explicit release still checks the worker result. Cleanup must not
        // panic again when a failed observed wait is already unwinding.
        if let Some(send)=self.release.take(){let _=send.send(());}
        if let Some(thread)=self.thread.take(){let _=thread.join();}
    }
}


#[test]
fn held_download_peer_waits_for_complete_split_request_and_releases_exact_response() {
    let (notice,accepted)=mpsc::channel();
    let mut held=Held::with_accepted_notice(1,Some(notice));
    let url=held.location.url.as_ref().unwrap();
    let mut peer=std::net::TcpStream::connect((url.host.as_str(),url.port)).unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    peer.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
    accepted.recv_timeout(Duration::from_secs(3)).expect("Owned fixture did not accept the deliberately silent peer.");
    // Synchronize acceptance before either phase, rather than assuming that
    // connect completion means the server has started reading request bytes.
    assert!(matches!(held.observed.recv_timeout(Duration::from_millis(100)),Err(mpsc::RecvTimeoutError::Timeout)),
        "A live accepted peer with no request must not be announced or disconnect.");
    peer.write_all(b"GET /payload.bin HTTP/1.1\r\nHost: local").unwrap();
    assert!(matches!(held.observed.recv_timeout(Duration::from_millis(100)),Err(mpsc::RecvTimeoutError::Timeout)),
        "A partial header without its final empty line must not be announced or disconnect.");
    peer.write_all(b"host\r\n\r\n").unwrap();
    held.wait(1);
    let mut headers=Vec::new();let mut byte=[0u8;1];
    while !headers.ends_with(b"\r\n\r\n") {
        assert!(headers.len()<512,"Owned response headers exceeded their finite bound.");
        peer.read_exact(&mut byte).unwrap();headers.push(byte[0]);
    }
    assert_eq!(headers,b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 7\r\nConnection: close\r\n\r\n");
    // The same explicit release used by the product controls checks worker
    // completion. Destructor cleanup alone cannot turn a worker panic into PASS.
    held.release();
    let mut body=Vec::new();
    peer.take(8).read_to_end(&mut body).unwrap();
    assert_eq!(body,b"payload");
}

#[test]
fn shared_download_view_drop_keeps_held_transfer_and_worker_uses_retargeted_wake() {
    let temp=Temp::new();let mut held=Held::new(1);let shared=Shared::new();
    let old_count=Arc::new(AtomicUsize::new(0));let new_count=Arc::new(AtomicUsize::new(0));
    let marker=Arc::new(());let weak_marker=Arc::downgrade(&marker);
    let captured=marker.clone();let count=old_count.clone();
    shared.set_wake(Some(Arc::new(move||{let _=Arc::strong_count(&captured);count.fetch_add(1,Ordering::SeqCst);}))).unwrap();
    drop(marker);
    let mut originating_view=shared.view().unwrap();
    let id=originating_view.start(held.location.clone(),None,&temp.folder(),trust()).unwrap();
    held.wait(1);assert_eq!(shared.active_count().unwrap(),1);drop(originating_view);
    let count=new_count.clone();shared.set_wake(Some(Arc::new(move||{count.fetch_add(1,Ordering::SeqCst);}))).unwrap();
    assert!(weak_marker.upgrade().is_none(),"A running transfer retained the old tab/window wake closure.");
    assert_eq!(shared.active_count().unwrap(),1,"Dropping a View cancelled its browser-owned transfer.");
    held.release();complete(&shared);
    let fresh=shared.view().unwrap();assert_eq!(fresh.entries()[0].id,id);assert_eq!(fresh.entries()[0].status,Status::Complete);
    assert_eq!(fs::read(fresh.entries()[0].path.as_ref().unwrap()).unwrap(),b"payload");
    assert_eq!(old_count.load(Ordering::SeqCst),0);assert!(new_count.load(Ordering::SeqCst)>0);
}

#[test]
fn shared_download_dispatcher_reentry_retargets_without_holding_callback_lock() {
    let dispatcher=Dispatcher::default();let old=Arc::new(AtomicUsize::new(0));let new=Arc::new(AtomicUsize::new(0));
    let route=dispatcher.clone();let old_calls=old.clone();let new_calls=new.clone();
    dispatcher.set(Some(Arc::new(move|| {
        old_calls.fetch_add(1,Ordering::SeqCst);let calls=new_calls.clone();
        route.set(Some(Arc::new(move||{calls.fetch_add(1,Ordering::SeqCst);}))).unwrap();
    }))).unwrap();
    let wake=dispatcher.wake();let (sent,received)=mpsc::channel();
    let runner=thread::spawn(move||{wake();wake();sent.send(()).unwrap();});
    received.recv_timeout(Duration::from_secs(2)).expect("Wake callback was invoked under the routing lock.");runner.join().unwrap();
    assert_eq!(old.load(Ordering::SeqCst),1);assert_eq!(new.load(Ordering::SeqCst),1);
    dispatcher.set(None).unwrap();dispatcher.notify();assert_eq!(new.load(Ordering::SeqCst),1);
}

#[test]
fn shared_download_snapshot_ownership_busy_refusal_and_fresh_recovery_are_real() {
    let temp=Temp::new();let shared=Shared::new();let mut first=shared.view().unwrap();let mut peer=shared.view().unwrap();
    let missing=temp.0.join("arrives.bin");let guard=shared.manager.borrow_mut();
    assert_eq!(first.start(source(&missing),None,&temp.folder(),trust()).unwrap_err(),BUSY);
    assert!(!temp.folder().exists(),"Busy authority performed filesystem admission.");
    assert!(first.refresh().is_err());drop(guard);
    let failed=first.start(source(&missing),None,&temp.folder(),trust()).unwrap();complete(&shared);first.refresh().unwrap();
    let saved_observation=first.entries()[0].clone();assert_eq!(saved_observation.id,failed);assert!(matches!(saved_observation.status,Status::Failed(_)));
    peer.refresh().unwrap();assert_eq!(peer.entries()[0].id,failed);
    assert_eq!(peer.clear_finished().unwrap(),1);
    assert_eq!(first.entries()[0].id,failed,"Owned snapshot changed through a peer's write.");
    assert!(first.refresh().unwrap());assert!(first.entries().is_empty());assert!(first.problem().is_none());
}

#[test]
fn shared_download_retry_uses_original_manager_context_and_never_overwrites_files() {
    let temp=Temp::new();let shared=Shared::new();let mut first=shared.view().unwrap();let mut peer=shared.view().unwrap();
    let path=temp.0.join("arrives.bin");let failed=first.start(source(&path),None,&temp.folder(),trust()).unwrap();
    complete(&shared);peer.refresh().unwrap();assert!(peer.entries()[0].can_retry());
    fs::write(&path,b"new payload").unwrap();fs::write(temp.folder().join("arrives.bin"),b"already saved").unwrap();
    let retried=peer.retry(failed,&temp.folder(),trust()).unwrap();assert_ne!(retried,failed);complete(&shared);first.refresh().unwrap();
    assert_eq!(first.entries()[0].status,Status::Complete);assert!(matches!(first.entries()[1].status,Status::Failed(_)));
    let first_saved=first.entries()[0].path.clone().unwrap();assert_eq!(fs::read(&first_saved).unwrap(),b"new payload");
    let other=first.start(source(&path),None,&temp.folder(),trust()).unwrap();assert_ne!(other,retried);complete(&shared);peer.refresh().unwrap();
    let second_saved=peer.entries()[0].path.clone().unwrap();assert_ne!(first_saved,second_saved);
    assert_eq!(fs::read(temp.folder().join("arrives.bin")).unwrap(),b"already saved");
    assert_eq!(fs::read(second_saved).unwrap(),b"new payload");
    assert_eq!(peer.clear_finished().unwrap(),3);assert_eq!(temp.files().len(),3,"Clearing history removed real saved files.");
}

#[test]
fn shared_download_admission_is_one_global_cap_and_cancel_cleans_every_held_transfer() {
    let temp=Temp::new();let mut held=Held::new(MAX_ACTIVE);let shared=Shared::new();
    let mut first=shared.view().unwrap();let mut second=shared.view().unwrap();let mut ids=Vec::new();
    for index in 0..MAX_ACTIVE {
        let view=if index%2==0 {&mut first}else{&mut second};
        ids.push(view.start(held.location.clone(),None,&temp.folder(),trust()).unwrap());
    }
    held.wait(MAX_ACTIVE);assert_eq!(shared.active_count().unwrap(),MAX_ACTIVE);
    let refused=first.start(held.location.clone(),None,&temp.folder(),trust()).unwrap_err();assert!(refused.contains("At most 3"));
    assert_eq!(first.entries().len(),MAX_ACTIVE);assert!(first.entries().len()<=MAX_ENTRIES);
    for id in ids {assert!(second.cancel(id).unwrap());}
    assert_eq!(second.active_count(),MAX_ACTIVE);assert_eq!(second.clear_finished().unwrap(),0);
    held.release();complete(&shared);first.refresh().unwrap();assert_eq!(first.active_count(),0);
    assert!(first.entries().iter().all(|entry|entry.status==Status::Cancelled));assert!(temp.files().is_empty());
    assert!(first.entries().iter().all(Entry::can_retry));
}

fn received(body:&[u8])->http::Response {
    http::Response {document_timing:None,redirected:false,url:crate::url::parse("https://download.invalid/received.bin").unwrap().0,
        fragment:None,status:200,status_text:"OK".into(),encoded_body_bytes:body.len(),protocol:Some("TLS 1.3"),request_is_post:true,cookie_same_site:false,
        headers:vec![("content-disposition".into(),"attachment; filename=received.bin".into())],body:body.to_vec(),events:Vec::new()}
}

#[test]
fn shared_download_received_post_keeps_no_replay_retry_and_invalid_response_refusal() {
    let temp=Temp::new();let shared=Shared::new();let mut first=shared.view().unwrap();let mut peer=shared.view().unwrap();
    let id=first.start_received(received(b"already received"),&temp.folder(),trust()).unwrap();complete(&shared);peer.refresh().unwrap();
    assert_eq!(peer.entries()[0].status,Status::Complete);assert!(peer.entries()[0].retry.is_none());
    assert_eq!(fs::read(peer.entries()[0].path.as_ref().unwrap()).unwrap(),b"already received");
    assert!(peer.retry(id,&temp.folder(),trust()).is_err());assert!(peer.retry(u64::MAX,&temp.folder(),trust()).is_err());
    let mut partial=received(b"partial");partial.status=206;assert!(first.start_received(partial,&temp.folder(),trust()).is_err());
    assert_eq!(first.entries().len(),1);assert_eq!(first.active_count(),0);assert_eq!(temp.files().len(),1);
}

#[test]
fn shared_download_authority_drop_cancels_owned_workers_and_stale_view_cannot_restart() {
    let temp=Temp::new();let mut held=Held::new(1);let shared=Shared::new();let mut view=shared.view().unwrap();
    view.start(held.location.clone(),None,&temp.folder(),trust()).unwrap();held.wait(1);
    drop(shared);assert!(view.refresh().is_err());assert_eq!(view.active_count(),0);assert_eq!(view.problem(),Some(CLOSED));
    let untouched=temp.0.join("must-not-create");assert_eq!(view.start(held.location.clone(),None,&untouched,trust()).unwrap_err(),CLOSED);
    assert!(!untouched.exists());assert!(temp.files().is_empty());held.release();
}
