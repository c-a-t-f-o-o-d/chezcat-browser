//! Original real-App strip/input controls. These are draft source, not native acceptance.
use super::*;
use super::super::Limits;
use crate::app::{App,Focus,PlatformCompletion};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration,Instant};

struct Temp(PathBuf);
impl Temp {
    fn new()->Self{
        let mut bytes=[0u8;8];crate::random::fill(&mut bytes).unwrap();
        let token:String=bytes.iter().map(|b|format!("{b:02x}")).collect();
        // Keep the ordinary absolute input path. Windows canonicalize returns
        // a device-prefixed spelling intentionally refused at file admission.
        let path=std::env::temp_dir().join(format!("chezcat-session-ui-{token}"));assert!(path.is_absolute());fs::create_dir(&path).unwrap();Self(path)
    }
    fn page(&self,name:&str,title:&str)->PathBuf{
        let path=self.0.join(name);
        fs::write(&path,format!("<!doctype html><html><head><title>{title}</title><style>body{{margin:0}}input,button{{display:block}}#spacer{{height:1800px}}</style></head><body><input id=q value=seed><button id=change type=button onclick=\"count++;document.title='{title} '+count;\">Change</button><div id=spacer>Original UI fixture</div><script>var count=0;document.addEventListener('keyup',function(e){{if(e.keyCode==9)document.title='UNEXPECTED TAB UP';}});</script></body></html>")).unwrap();path
    }
}
impl Drop for Temp{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
fn key(code:u32,ctrl:bool,shift:bool,down:bool)->KeyboardInput{
    KeyboardInput{key_code:code,ctrl,shift,down,key:if code==9{"Tab".into()}else{char::from_u32(code).unwrap_or('?').to_string()},..KeyboardInput::default()}
}
fn adopt(app:App,limits:Limits)->Session{match Session::adopt(Box::new(app),limits){Ok(session)=>session,Err((_app,error))=>panic!("{error}")}}
fn setup(app:App)->(Session,SessionUi,WindowId,TabId){
    let mut session=adopt(app,Limits::default());let window=session.window_ids().next().unwrap();let tab=session.active(window).unwrap().unwrap();let mut ui=SessionUi::new();ui.resize(&mut session,window,900,700).unwrap();(session,ui,window,tab)
}
fn open(app:&mut App,path:&PathBuf,title:&str){app.open(path);assert!(!app.failed,"Fixture admission failed: {}",app.status);assert_eq!(app.document.title,title);}
fn node(app:&App,id:&str)->usize{(0..app.document.nodes.len()).find(|&n|app.document.attr(n,"id")==Some(id)).expect("Fixture id missing after successful admission")}
fn point(session:&Session,tab:TabId,id:&str)->(i32,i32){session.observe_tab(tab,|app|{let node=node(app,id);let r=app.page.controls.iter().find(|r|r.node==node).unwrap();(r.x+r.width/2,STRIP_HEIGHT+app.content_top()+r.y-app.scroll+r.height/2)}).unwrap()}
fn tap(ui:&mut SessionUi,session:&mut Session,window:WindowId,point:(i32,i32))->Outcome{ui.pointer_down(session,window,point.0,point.1,false).unwrap();ui.pointer_up(session,window,point.0,point.1).unwrap()}
fn edit(ui:&mut SessionUi,session:&mut Session,window:WindowId,tab:TabId,text:&str){
    let p=point(session,tab,"q");tap(ui,session,window,p);ui.key_event(session,window,key(0x41,true,false,true)).unwrap();for unit in text.encode_utf16(){ui.utf16_input(session,window,unit).unwrap();}
    session.observe_tab(tab,|app|assert_eq!(app.forms.controls[&node(app,"q")].value.text(),text)).unwrap();
}
fn control(ui:&SessionUi,session:&Session,window:WindowId,hit:Hit)->(i32,i32){let geometry=ui.geometry(session,window).unwrap();let(_,r)=geometry.controls.iter().find(|(candidate,_)|*candidate==hit).expect("Control is not visible");((r.left+r.right)/2,(r.top+r.bottom)/2)}
fn idle(session:&mut Session){for _ in 0..64{if !session.poll(4).unwrap().more{return;}}panic!("Owned wake queue did not converge");}

#[test]
fn session_ui_composites_the_real_app_once_and_readonly_observation_enqueues_no_wake(){
    let temp=Temp::new();let page=temp.page("compose.html","Composite");let mut app=App::new();open(&mut app,&page,"Composite");let(mut session,mut ui,window,tab)=setup(app);
    ui.resize(&mut session,window,900,700).unwrap();edit(&mut ui,&mut session,window,tab,"real rendered input");
    let before=session.observe_tab(tab,|app|{assert_eq!((app.width,app.height),(900,700-STRIP_HEIGHT));(app.raw.clone(),app.script_epoch,app.document.nodes.as_ptr(),app.draw())}).unwrap();idle(&mut session);assert!(!session.mailbox.pending());
    let canvas=ui.frame(&session,window).unwrap();assert_eq!((canvas.width,canvas.height),(900,700));
    assert_eq!(&canvas.pixels[STRIP_HEIGHT as usize*900..],before.3.pixels.as_slice());
    assert!(canvas.pixels[..STRIP_HEIGHT as usize*900].iter().any(|&pixel|pixel==ui::Palette::new(false).chrome));
    assert!(ui.title(&session,window).unwrap().contains("Composite"));let _=ui.wants_hand(&session,window,30,16).unwrap();let _=ui.resize_cursor(&session,window,30,16).unwrap();
    assert!(!session.mailbox.pending(),"Read-only draw/title/cursor paths scheduled owner work");
    session.observe_active(window,|app|{assert_eq!(app.raw,before.0);assert_eq!(app.script_epoch,before.1);assert_eq!(app.document.nodes.as_ptr(),before.2);assert_eq!(app.forms.controls[&node(app,"q")].value.text(),"real rendered input");}).unwrap();
}

#[test]
fn session_ui_controls_preserve_original_page_forms_vm_scroll_and_back_history(){
    let temp=Temp::new();let a=temp.page("one.html","One");let b=temp.page("two.html","Two");let later=temp.page("later.html","Later");let mut app=App::new();open(&mut app,&a,"One");let(mut session,mut ui,window,one)=setup(app);
    edit(&mut ui,&mut session,window,one,"edited one");let p=point(&session,one,"change");tap(&mut ui,&mut session,window,p);
    let before=session.with_tab(one,|app|{app.scroll_by(170);assert_eq!(app.document.title,"One 1");(app.raw.clone(),app.script_epoch,app.document.nodes.as_ptr(),app.scroll,app.history.len())}).unwrap();
    let outcome=ui.key_event(&mut session,window,key(0x54,true,false,true)).unwrap();assert!(outcome.suppress_text);let two=session.active(window).unwrap().unwrap();assert_ne!(one,two);
    session.with_tab(two,|app|{assert!(app.scripting.is_none());assert_eq!(app.history.len(),1);open(app,&b,"Two");}).unwrap();edit(&mut ui,&mut session,window,two,"edited two");let p=point(&session,two,"change");tap(&mut ui,&mut session,window,p);
    ui.key_event(&mut session,window,key(9,true,true,true)).unwrap();assert_eq!(session.active(window).unwrap(),Some(one));
    session.observe_tab(one,|app|{assert_eq!(app.document.title,"One 1");assert_eq!(app.raw,before.0);assert_eq!(app.script_epoch,before.1);assert_eq!(app.document.nodes.as_ptr(),before.2);assert_eq!(app.scroll,before.3);assert_eq!(app.history.len(),before.4);assert_eq!(app.forms.controls[&node(app,"q")].value.text(),"edited one");}).unwrap();
    let p=control(&ui,&session,window,Hit::Select(two));tap(&mut ui,&mut session,window,p);assert_eq!(session.active(window).unwrap(),Some(two));
    session.with_tab(two,|app|{assert_eq!(app.document.title,"Two 1");assert_eq!(app.forms.controls[&node(app,"q")].value.text(),"edited two");let accepted=app.location.clone();open(app,&later,"Later");app.mouse_navigation(false);assert_eq!(app.location.path,accepted.path);assert_eq!(app.history.len(),3);}).unwrap();
    let p=control(&ui,&session,window,Hit::Close(two));tap(&mut ui,&mut session,window,p);assert_eq!(session.active(window).unwrap(),Some(one));assert!(session.observe_tab(two,|_|()).is_err());assert_eq!(session.tab_count(),1);
    session.observe_active(window,|app|{assert_eq!(app.document.title,"One 1");assert_eq!((app.width,app.height),(900,668));assert_eq!(app.raw,before.0);}).unwrap();
}

#[test]
fn session_ui_translates_page_pointer_and_wheel_once_and_never_sends_strip_to_page(){
    let temp=Temp::new();let page=temp.page("input.html","Input");let mut app=App::new();open(&mut app,&page,"Input");let(mut session,mut ui,window,tab)=setup(app);
    let p=point(&session,tab,"change");assert!(ui.wants_hand(&session,window,p.0,p.1).unwrap());tap(&mut ui,&mut session,window,p);session.observe_tab(tab,|app|assert_eq!(app.document.title,"Input 1")).unwrap();
    let before=session.observe_tab(tab,|app|(app.scroll,app.downloads.entries().len(),app.forms.controls[&node(app,"q")].value.text().to_owned())).unwrap();
    ui.wheel_at(&mut session,window,100,16,-120,false,false).unwrap();assert!(!ui.download_at(&mut session,window,100,16).unwrap());assert!(!ui.resize_cursor(&session,window,100,16).unwrap());
    session.observe_tab(tab,|app|{assert_eq!(app.scroll,before.0);assert_eq!(app.downloads.entries().len(),before.1);assert_eq!(app.forms.controls[&node(app,"q")].value.text(),before.2);}).unwrap();
    ui.wheel_at(&mut session,window,300,STRIP_HEIGHT+160,-120,false,false).unwrap();session.observe_tab(tab,|app|assert!(app.scroll>before.0)).unwrap();
    ui.pointer_cancel(&mut session,window).unwrap();ui.pointer_leave(&mut session,window).unwrap();session.observe_tab(tab,|app|assert!(matches!(app.drag,crate::app::Drag::None))).unwrap();
}

#[test]
fn session_ui_press_release_refuses_changed_membership_geometry_and_replaced_owner(){
    let(mut session,mut ui,window,one)=setup(App::new());let two=session.new_tab(window).unwrap();ui.refresh(&session,window).unwrap();
    let p=control(&ui,&session,window,Hit::Close(one));ui.pointer_down(&mut session,window,p.0,p.1,false).unwrap();let three=session.new_tab(window).unwrap();ui.pointer_up(&mut session,window,p.0,p.1).unwrap();assert_eq!(session.tab_count(),3);assert!(session.observe_tab(one,|_|()).is_ok());assert_eq!(session.active(window).unwrap(),Some(three));
    ui.refresh(&session,window).unwrap();let p=control(&ui,&session,window,Hit::Close(two));ui.pointer_down(&mut session,window,p.0,p.1,false).unwrap();ui.resize(&mut session,window,980,700).unwrap();ui.resize(&mut session,window,900,700).unwrap();ui.pointer_up(&mut session,window,p.0,p.1).unwrap();assert_eq!(session.tab_count(),3);
    let p=control(&ui,&session,window,Hit::Close(one));ui.pointer_down(&mut session,window,p.0,p.1,false).unwrap();session.close_tab(one).unwrap();let replacement=session.new_tab(window).unwrap();ui.pointer_up(&mut session,window,p.0,p.1).unwrap();assert_ne!(replacement,one);assert_eq!(session.tab_count(),3);assert!(session.observe_tab(replacement,|_|()).is_ok());
    ui.refresh(&session,window).unwrap();let p=control(&ui,&session,window,Hit::Close(two));ui.pointer_down(&mut session,window,p.0,p.1,false).unwrap();ui.pointer_move(&mut session,window,p.0+8,p.1,true).unwrap();ui.pointer_up(&mut session,window,p.0,p.1).unwrap();assert!(session.observe_tab(two,|_|()).is_ok());
}

#[test]
fn session_ui_overflow_tiny_viewports_and_close_forget_keep_full_typed_windows_bounded(){
    let(mut session,mut ui,window,_)=setup(App::new());for _ in 0..5{ui.new_tab(&mut session,window).unwrap();}ui.resize(&mut session,window,210,130).unwrap();
    let active=session.active(window).unwrap().unwrap();let geometry=ui.geometry(&session,window).unwrap();assert!(geometry.controls.iter().any(|(hit,_)|*hit==Hit::Select(active)));for(_,r)in &geometry.controls{assert!(r.left>=0&&r.right<=210&&r.top>=0&&r.bottom<=130);}
    ui.wheel_at(&mut session,window,10,16,120,false,false).unwrap();let scrolled=ui.geometry(&session,window).unwrap().token.first;assert!(scrolled<geometry.token.first);
    ui.key_event(&mut session,window,key(9,true,true,true)).unwrap();let next=session.active(window).unwrap().unwrap();assert_ne!(next,active);assert!(ui.geometry(&session,window).unwrap().controls.iter().any(|(hit,_)|*hit==Hit::Select(next)));
    ui.resize(&mut session,window,210,16).unwrap();
    assert!(ui.geometry(&session,window).unwrap().controls.iter().all(|(_,r)|r.bottom<=16));
    ui.pointer_down(&mut session,window,100,20,false).unwrap();ui.pointer_up(&mut session,window,100,20).unwrap();assert_eq!(session.active(window).unwrap(),Some(next));
    ui.resize(&mut session,window,210,1).unwrap();assert!(ui.geometry(&session,window).unwrap().controls.is_empty());let canvas=ui.frame(&session,window).unwrap();assert_eq!((canvas.width,canvas.height),(210,1));
    ui.resize(&mut session,window,1,24).unwrap();let canvas=ui.frame(&session,window).unwrap();assert_eq!((canvas.width,canvas.height),(1,24));session.observe_active(window,|app|assert_eq!((app.width,app.height),(1,1))).unwrap();
    ui.resize(&mut session,window,210,130).unwrap();let outcome=ui.key_event(&mut session,window,key(0x4e,true,false,true)).unwrap();let(other,tab)=outcome.opened.unwrap();assert_ne!(window,other);assert_eq!(session.active(other).unwrap(),Some(tab));assert_eq!(ui.windows.len(),2);
    let outcome=ui.close_window(&mut session,other).unwrap();assert_eq!(outcome.closed,Some(other));assert_eq!(ui.windows.len(),1);assert!(ui.frame(&session,other).is_err());
    let outcome=ui.key_event(&mut session,window,key(0x4e,true,false,true)).unwrap();let(reopened,replacement)=outcome.opened.unwrap();assert_ne!(reopened,other);assert_ne!(replacement,tab);assert!(ui.pointer_down(&mut session,other,5,16,false).is_err());assert_eq!(ui.windows.len(),2);
}

#[test]
fn session_ui_shortcut_keyup_is_not_delivered_to_the_new_page_owner(){
    let temp=Temp::new();let a=temp.page("key-one.html","Key one");let b=temp.page("key-two.html","Key two");let mut app=App::new();open(&mut app,&a,"Key one");let(mut session,mut ui,window,one)=setup(app);ui.new_tab(&mut session,window).unwrap();let two=session.active(window).unwrap().unwrap();session.with_tab(two,|app|open(app,&b,"Key two")).unwrap();
    ui.key_event(&mut session,window,key(9,true,false,true)).unwrap();assert_eq!(session.active(window).unwrap(),Some(one));ui.pointer_cancel(&mut session,window).unwrap();let outcome=ui.key_event(&mut session,window,key(9,false,false,false)).unwrap();assert!(outcome.suppress_text);session.observe_tab(one,|app|assert_eq!(app.document.title,"Key one")).unwrap();
    ui.key_event(&mut session,window,key(9,true,false,true)).unwrap();ui.key_event(&mut session,window,key(9,false,false,false)).unwrap();session.observe_tab(two,|app|assert_eq!(app.document.title,"Key two")).unwrap();
}

#[test]
fn session_ui_picker_input_cancel_allows_unchanged_request_but_logical_window_change_refuses(){
    let(mut session,mut ui,window,tab)=setup(App::new());ui.key_event(&mut session,window,key(0x4c,true,false,true)).unwrap();let request=ui.key_event(&mut session,window,key(0x56,true,false,true)).unwrap().request.unwrap();assert_eq!((request.window,request.tab),(window,tab));
    // WM_KILLFOCUS from an owned picker cancels input capture, not logical editor authority.
    ui.pointer_cancel(&mut session,window).unwrap();assert!(session.with_tab(tab,|app|app.complete_platform_request(request.request,PlatformCompletion::Paste("unchanged picker completion".into()))).unwrap());
    session.observe_tab(tab,|app|assert_eq!(app.address.text(),"unchanged picker completion")).unwrap();
    let request=ui.key_event(&mut session,window,key(0x56,true,false,true)).unwrap().request.unwrap();let(other,_)=session.new_window(500,400).unwrap();ui.resize(&mut session,other,500,432).unwrap();ui.activate_window(&mut session,other).unwrap();ui.activate_window(&mut session,window).unwrap();
    assert!(!session.with_tab(tab,|app|app.complete_platform_request(request.request,PlatformCompletion::Paste("refused".into()))).unwrap());session.observe_tab(tab,|app|assert_eq!(app.address.text(),"unchanged picker completion")).unwrap();
}

#[test]
fn session_core_direct_switch_invalidates_platform_request_even_after_switching_back(){
    let mut session=adopt(App::new(),Limits::default());let window=session.window_ids().next().unwrap();let one=session.active(window).unwrap().unwrap();
    let request=session.with_tab(one,|app|{app.key(0x4c,true,false,false);let action=app.key(0x56,true,false,false);app.platform_request(action)}).unwrap();let before=session.observe_tab(one,|app|app.address.text().to_owned()).unwrap();
    let two=session.new_tab(window).unwrap();session.switch_tab(window,one).unwrap();assert!(!session.with_tab(one,|app|app.complete_platform_request(request,PlatformCompletion::Paste("refused".into()))).unwrap());session.observe_tab(one,|app|assert_eq!(app.address.text(),before)).unwrap();
    let request=session.with_tab(one,|app|{let action=app.key(0x56,true,false,false);app.platform_request(action)}).unwrap();session.switch_tab(window,two).unwrap();session.switch_tab(window,one).unwrap();assert!(!session.with_tab(one,|app|app.complete_platform_request(request,PlatformCompletion::Paste("also refused".into()))).unwrap());
    let request=session.with_tab(one,|app|{let action=app.key(0x56,true,false,false);app.platform_request(action)}).unwrap();session.close_tab(one).unwrap();let replacement=session.new_tab(window).unwrap();assert_ne!(one,replacement);assert!(session.with_tab(one,|app|app.complete_platform_request(request,PlatformCompletion::Paste("wrong owner".into()))).is_err());
}

#[test]
fn session_ui_last_close_reports_typed_window_and_defers_history_to_final_shutdown(){
    let temp=Temp::new();let page=temp.page("history.html","History UI");let file=temp.0.join("history.txt");let mut store=crate::history::Store::open(file.clone()).unwrap();store.configure(crate::history::Policy{enabled:true,retention_days:None,clear_on_exit:true},crate::history::now()).unwrap();let mut app=App::new();app.attach_browsing_history(store);open(&mut app,&page,"History UI");let(mut session,mut ui,window,one)=setup(app);
    ui.new_tab(&mut session,window).unwrap();let two=session.active(window).unwrap().unwrap();session.with_tab(two,|app|open(app,&page,"History UI")).unwrap();let before=fs::read(&file).unwrap();let outcome=ui.close_tab(&mut session,window,one).unwrap();assert_eq!(outcome.closed,None);assert_eq!(fs::read(&file).unwrap(),before);
    let outcome=ui.key_event(&mut session,window,key(0x57,true,false,true)).unwrap();assert_eq!(outcome.closed,Some(window));assert_eq!(session.window_count(),0);assert_eq!(session.tab_count(),0);assert!(ui.windows.is_empty());assert_eq!(fs::read(&file).unwrap(),before);assert!(session.observe_tab(two,|_|()).is_err());
    let done=session.shutdown(crate::history::now()).unwrap();assert_eq!(done.history_removed,2);assert!(crate::history::Store::open(file).unwrap().data().entries.is_empty());assert_eq!(session.shutdown(crate::history::now()).unwrap().history_removed,0);
}


struct HeldState {requests:Vec<String>,released:bool,finished:bool}
struct HeldSite {
    address:std::net::SocketAddr,state:std::sync::Arc<(std::sync::Mutex<HeldState>,std::sync::Condvar)>,
    stop:std::sync::Arc<std::sync::atomic::AtomicBool>,thread:Option<std::thread::JoinHandle<()>>,
}
impl HeldSite {
    fn new()->Self{
        use std::io::{Read,Write};use std::sync::{Arc,Mutex,Condvar};use std::sync::atomic::{AtomicBool,Ordering};
        let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();listener.set_nonblocking(true).unwrap();let address=listener.local_addr().unwrap();
        let state=Arc::new((Mutex::new(HeldState{requests:Vec::new(),released:false,finished:false}),Condvar::new()));let stop=Arc::new(AtomicBool::new(false));let(shared,stopped)=(state.clone(),stop.clone());
        let thread=std::thread::spawn(move||{let mut peers=Vec::new();while !stopped.load(Ordering::Relaxed){match listener.accept(){Ok((mut stream,_))=>{let shared=shared.clone();peers.push(std::thread::spawn(move||{
            stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();stream.set_write_timeout(Some(Duration::from_secs(10))).unwrap();let mut request=Vec::new();let mut byte=[0u8;1];while !request.ends_with(b"\r\n\r\n"){if request.len()>8192||stream.read_exact(&mut byte).is_err(){return;}request.push(byte[0]);}
            let text=String::from_utf8_lossy(&request);let path=text.lines().next().and_then(|line|line.split_whitespace().nth(1)).unwrap_or("/").to_owned();shared.0.lock().unwrap().requests.push(path.clone());shared.1.notify_all();
            let(mime,body)=match path.as_str(){"/owner"=>("text/html","<!doctype html><title>Held UI owner</title><body>Owner<script>var s=document.createElement('script');s.src='/late.js';document.body.appendChild(s);</script>"),"/late.js"=>("text/javascript","document.title='LATE';location.href='/escaped';"),_=>("text/plain","Unexpected route")};
            let header=format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'\r\n\r\n",body.len());let _=stream.write_all(header.as_bytes());
            if path=="/late.js"{let guard=shared.0.lock().unwrap();let _guard=shared.1.wait_while(guard,|state|!state.released).unwrap();}
            let _=stream.write_all(body.as_bytes());if path=="/late.js"{shared.0.lock().unwrap().finished=true;shared.1.notify_all();}
        }));},Err(error)if error.kind()==std::io::ErrorKind::WouldBlock=>std::thread::sleep(Duration::from_millis(1)),Err(error)=>panic!("{error}")}}for peer in peers{peer.join().unwrap();}});
        Self{address,state,stop,thread:Some(thread)}
    }
    fn seen(&self,path:&str)->bool{self.state.0.lock().unwrap().requests.iter().any(|value|value==path)}
    fn release(&self){self.state.0.lock().unwrap().released=true;self.state.1.notify_all();}
}
impl Drop for HeldSite{fn drop(&mut self){self.release();self.stop.store(true,std::sync::atomic::Ordering::Relaxed);if let Some(thread)=self.thread.take(){thread.join().unwrap();}}}

#[test]
fn session_ui_close_control_consumes_held_page_and_stale_wake_without_changing_survivor(){
    let temp=Temp::new();let page=temp.page("survivor.html","UI survivor");let mut app=App::new();open(&mut app,&page,"UI survivor");let(mut session,mut ui,window,alive)=setup(app);edit(&mut ui,&mut session,window,alive,"surviving editor");let p=point(&session,alive,"change");tap(&mut ui,&mut session,window,p);
    ui.new_tab(&mut session,window).unwrap();let closing=session.active(window).unwrap().unwrap();let site=HeldSite::new();session.with_tab(closing,|app|app.open_address(&format!("http://{}/owner",site.address))).unwrap();
    let deadline=Instant::now()+Duration::from_secs(8);while !site.seen("/late.js"){assert!(Instant::now()<deadline,"Held UI fixture never requested its external script");session.poll(4).unwrap();std::thread::sleep(Duration::from_millis(2));}
    session.observe_tab(closing,|app|assert_eq!(app.document.title,"Held UI owner")).unwrap();assert!(!site.state.0.lock().unwrap().finished,"Fixture was released before close");
    let before=session.observe_tab(alive,|app|(app.raw.clone(),app.script_epoch,app.document.nodes.as_ptr(),app.history.len())).unwrap();let wake=session.tab_wake(closing).unwrap();let p=control(&ui,&session,window,Hit::Close(closing));tap(&mut ui,&mut session,window,p);assert!(session.observe_tab(closing,|_|()).is_err());assert_eq!(session.active(window).unwrap(),Some(alive));site.release();wake();
    let deadline=Instant::now()+Duration::from_secs(8);while !site.state.0.lock().unwrap().finished{assert!(Instant::now()<deadline,"Released response did not reach terminal write attempt");session.poll(4).unwrap();std::thread::sleep(Duration::from_millis(2));}idle(&mut session);assert!(!site.seen("/escaped"),"Consumed UI page performed late navigation");
    session.observe_tab(alive,|app|{assert_eq!(app.document.title,"UI survivor 1");assert_eq!(app.forms.controls[&node(app,"q")].value.text(),"surviving editor");assert_eq!(app.raw,before.0);assert_eq!(app.script_epoch,before.1);assert_eq!(app.document.nodes.as_ptr(),before.2);assert_eq!(app.history.len(),before.3);}).unwrap();
    ui.new_tab(&mut session,window).unwrap();let replacement=session.active(window).unwrap().unwrap();assert_ne!(replacement,closing);assert_ne!(session.observe_tab(replacement,|app|app.document.title.clone()).unwrap(),"LATE");
}
