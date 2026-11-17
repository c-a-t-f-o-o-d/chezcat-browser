//! Original real-App native UI/link controls.
//! Host payload values are supplied through the pure seam; physical OS input,
//! popup interaction and native activation are not claimed by these controls.
use super::*;
use crate::app::{App,Focus};
use std::fs;
use std::path::PathBuf;

struct Files(PathBuf);
impl Files{
    fn new()->Self{let mut random=[0u8;8];crate::random::fill(&mut random).unwrap();let suffix:String=random.iter().map(|byte|format!("{byte:02x}")).collect();let path=std::env::temp_dir().join(format!("chezcat-ui-native-link-{suffix}"));fs::create_dir(&path).unwrap();Self(path)}
    fn setup(&self)->(Session,SessionUi,WindowId,TabId){
        fs::write(self.0.join("target.html"),"<!doctype html><title>UI link target</title><p>Destination</p>").unwrap();
        fs::write(self.0.join("other.html"),"<!doctype html><title>Authored next</title><p>Source navigation</p>").unwrap();
        let path=self.0.join("source.html");
        fs::write(&path,"<!doctype html><title>UI link source</title><style>body{margin:0}a,input,button{display:block;width:180px;height:32px}#space{height:2000px}</style><body><a id=a href=target.html>Actual link café</a><input id=q value=seed><input id=c type=checkbox><button id=b type=button onclick=\"document.title='control callback';\">Control</button><div id=space>Scroll</div><script>var a=document.getElementById('a'),clicks=0,auxes=0,actual='',actualTrusted=false;a.addEventListener('click',function(e){clicks++;actual=[e.button,e.ctrlKey,e.shiftKey,e.altKey,e.metaKey,e.screenX,e.screenY,e.buttons,e.clientX,e.clientY].join(',');actualTrusted=e instanceof MouseEvent&&e.isTrusted;document.title='primary '+clicks;});a.addEventListener('auxclick',function(e){auxes++;actual=[e.button,e.ctrlKey,e.shiftKey,e.altKey,e.metaKey,e.screenX,e.screenY,e.buttons,e.clientX,e.clientY].join(',');actualTrusted=e instanceof MouseEvent&&e.isTrusted;document.title='middle '+auxes;});</script>").unwrap();
        let mut app=App::new();app.open(&path);assert!(!app.failed,"{}",app.status);
        let mut session=Session::adopt(Box::new(app),super::super::Limits::default()).map_err(|(_,error)|error).unwrap();
        let window=session.window_ids().next().unwrap();let tab=session.active(window).unwrap().unwrap();let mut ui=SessionUi::new();ui.resize(&mut session,window,900,700).unwrap();(session,ui,window,tab)
    }
}
impl Drop for Files{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
fn node(app:&App,id:&str)->usize{(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(id)).unwrap()}
fn point(session:&Session,tab:TabId,id:&str)->(i32,i32){session.observe_tab(tab,|app|{let node=node(app,id);let r=app.page.border_rects[node].unwrap();(r.x+r.width/2,STRIP_HEIGHT+app.content_top()+r.y-app.scroll+r.height/2)}).unwrap()}
fn control(session:&Session,tab:TabId,id:&str)->(i32,i32){session.observe_tab(tab,|app|{let node=node(app,id);let r=app.page.controls.iter().find(|r|r.node==node).unwrap();(r.x+r.width/2,STRIP_HEIGHT+app.content_top()+r.y-app.scroll+r.height/2)}).unwrap()}
fn input(button:u8,ctrl:bool,shift:bool)->AnchorMouseInput{AnchorMouseInput{button,ctrl,shift,screen_x:-407.0,screen_y:291.0,..AnchorMouseInput::default()}}
fn eval(session:&mut Session,tab:TabId,source:&str){session.with_tab(tab,|app|{let page=app.scripting.as_mut().unwrap();let outcome=page.console_eval(&mut app.document,&mut app.forms,source);assert_eq!(outcome.errors,0,"{}",page.report());if outcome.layout_changed{app.reflow();}}).unwrap();}
fn release(ui:&mut SessionUi,session:&mut Session,window:WindowId,point:(i32,i32),input:AnchorMouseInput)->Outcome{ui.link_pointer_down(session,window,point.0,point.1,input.button,input.shift).unwrap();ui.link_pointer_up(session,window,point.0,point.1,input).unwrap()}
fn realm(session:&mut Session,tab:TabId,primary:u8,middle:u8){eval(session,tab,&format!("if(clicks!=={primary}||auxes!=={middle})throw 'author count';"));}

#[test]
fn native_ui_disposition_matrix_retains_real_owner_and_exact_release_payload(){
    for(button,ctrl,shift,disposition)in [(0,false,false,LinkDisposition::Current),(0,true,false,LinkDisposition::BackgroundTab),(0,false,true,LinkDisposition::NewWindow),(0,true,true,LinkDisposition::ForegroundTab),(1,false,false,LinkDisposition::BackgroundTab),(1,false,true,LinkDisposition::ForegroundTab)]{
        let files=Files::new();let(mut session,mut ui,window,source)=files.setup();let point=point(&session,source,"a");
        let before=session.observe_tab(source,|app|(app.document.nodes.as_ptr(),app.raw.clone(),app.location.clone(),app.scroll,app.history.len())).unwrap();
        let css=session.observe_tab(source,|app|(f64::from(point.0)/f64::from(app.zoom),f64::from(point.1-STRIP_HEIGHT-app.content_top())/f64::from(app.zoom))).unwrap();
        let out=release(&mut ui,&mut session,window,point,input(button,ctrl,shift));
        if disposition==LinkDisposition::Current{assert_eq!(session.tab_count(),1);session.observe_tab(source,|app|assert_eq!(app.document.title,"UI link target")).unwrap();continue;}
        assert_eq!(session.tab_count(),2);let destination=if let Some((new_window,tab))=out.opened{assert_eq!(disposition,LinkDisposition::NewWindow);assert_ne!(new_window,window);assert_eq!(ui.logical_active,Some(new_window));tab}else{assert_ne!(disposition,LinkDisposition::NewWindow);*session.window_tabs(window).unwrap().last().unwrap()};
        session.observe_tab(destination,|app|assert_eq!(app.document.title,"UI link target")).unwrap();
        let selected=if disposition==LinkDisposition::ForegroundTab{destination}else{source};assert_eq!(session.active(window).unwrap(),Some(selected));
        session.observe_tab(source,|app|{assert_eq!(app.document.nodes.as_ptr(),before.0);assert_eq!(app.raw,before.1);assert_eq!(app.location,before.2);assert_eq!(app.scroll,before.3);assert_eq!(app.history.len(),before.4);}).unwrap();
        realm(&mut session,source,u8::from(button==0),u8::from(button==1));
        // Record fields inside the genuine dispatch slot; do not retain or
        // fabricate a later live MouseEvent payload.
        eval(&mut session,source,&format!("if(!actualTrusted||actual!=='{button},{ctrl},{shift},false,false,-407,291,0,{},{}')throw 'release payload '+actual;",css.0,css.1));
    }
}

#[test]
fn native_ui_jitter_survives_and_drag_return_never_reacquires_default(){
    for dragged in [false,true]{let files=Files::new();let(mut session,mut ui,window,source)=files.setup();let point=point(&session,source,"a");
        ui.link_pointer_down(&mut session,window,point.0,point.1,0,false).unwrap();let delta=if dragged{12}else{2};
        ui.pointer_move(&mut session,window,point.0+delta,point.1,true).unwrap();ui.pointer_move(&mut session,window,point.0,point.1,true).unwrap();
        ui.link_pointer_up(&mut session,window,point.0,point.1,input(0,true,false)).unwrap();
        assert_eq!(session.tab_count(),if dragged{1}else{2});assert_eq!(session.active(window).unwrap(),Some(source));realm(&mut session,source,u8::from(!dragged),0);
    }
}

#[test]
fn native_ui_stale_scope_geometry_and_source_switch_cannot_fall_into_legacy_follow(){
    for change in 0..4{let files=Files::new();let(mut session,mut ui,window,source)=files.setup();let point=point(&session,source,"a");
        ui.link_pointer_down(&mut session,window,point.0,point.1,0,false).unwrap();
        match change{
            0=>{session.with_tab(source,|app|app.invalidate_platform_requests()).unwrap();},
            1=>{ui.resize(&mut session,window,940,700).unwrap();ui.resize(&mut session,window,900,700).unwrap();},
            2=>{let other=session.new_tab(window).unwrap();session.switch_tab(window,source).unwrap();session.close_tab(other).unwrap();},
            _=>{eval(&mut session,source,"a.setAttribute('href','other.html');a.setAttribute('href','target.html');");},
        }
        ui.link_pointer_up(&mut session,window,point.0,point.1,input(0,false,false)).unwrap();assert_eq!(session.tab_count(),1);assert_eq!(session.active(window).unwrap(),Some(source));
        session.observe_tab(source,|app|assert_eq!(app.document.title,"UI link source")).unwrap();realm(&mut session,source,0,0);
    }
}

#[test]
fn native_ui_uncaptured_pending_author_navigation_stays_queued_and_controls_keep_defaults(){
    let files=Files::new();let(_http,mut session,mut ui,window,source)=http_setup(&files);let point=point(&session,source,"a");
    eval(&mut session,source,"location.href='other.html';");release(&mut ui,&mut session,window,point,input(0,true,false));
    assert_eq!(session.tab_count(),1);session.observe_tab(source,|app|{assert_eq!(app.document.title,"UI link source");assert!(app.scripting.as_ref().unwrap().has_navigation_request());}).unwrap();realm(&mut session,source,0,0);
    session.with_tab(source,|app|{app.poll_load();app.wait_for_load();assert_eq!(app.document.title,"Authored next");}).unwrap();

    let(mut session,mut ui,window,source)=files.setup();let checkbox=control(&session,source,"c");
    release(&mut ui,&mut session,window,checkbox,input(1,false,false));session.observe_tab(source,|app|assert!(!app.forms.controls[&node(app,"c")].checked)).unwrap();
    release(&mut ui,&mut session,window,checkbox,input(0,false,false));session.observe_tab(source,|app|assert!(app.forms.controls[&node(app,"c")].checked)).unwrap();
    let button=control(&session,source,"b");release(&mut ui,&mut session,window,button,input(0,false,false));session.observe_tab(source,|app|assert_eq!(app.document.title,"control callback")).unwrap();assert_eq!(session.tab_count(),1);
}

#[test]
fn native_ui_cancelled_author_effects_and_authored_download_precede_disposition(){
    for cancel in [true,false]{let files=Files::new();let(mut session,mut ui,window,source)=files.setup();let point=point(&session,source,"a");
        session.with_tab(source,|app|{let mut data=app.profile.data().clone();data.settings.download_folder=Some(files.0.join("downloads").display().to_string());app.profile.save(data).unwrap();}).unwrap();
        eval(&mut session,source,if cancel{"a.setAttribute('download','accepted.bin');a.onclick=function(e){document.title='cancel effect';e.preventDefault();};"}else{"a.onclick=function(){a.setAttribute('download','accepted.bin');document.title='download effect';};"});
        release(&mut ui,&mut session,window,point,input(0,true,true));assert_eq!(session.tab_count(),1);assert_eq!(session.window_count(),1);assert_eq!(session.active(window).unwrap(),Some(source));
        session.observe_tab(source,|app|{assert_eq!(app.document.title,if cancel{"cancel effect"}else{"download effect"});assert_eq!(app.downloads.entries().len(),usize::from(!cancel));}).unwrap();realm(&mut session,source,1,0);
    }
}

#[test]
fn native_ui_context_explicit_dispositions_do_not_dispatch_author_click(){
    for disposition in [LinkDisposition::ForegroundTab,LinkDisposition::BackgroundTab,LinkDisposition::NewWindow]{let files=Files::new();let(mut session,mut ui,window,source)=files.setup();let point=point(&session,source,"a");
        let context=ui.link_context(&mut session,window,point.0,point.1).unwrap().unwrap();let out=ui.open_link_context(&mut session,context,disposition).unwrap();
        assert_eq!(session.tab_count(),2);assert_eq!(out.opened.is_some(),disposition==LinkDisposition::NewWindow);realm(&mut session,source,0,0);
        session.observe_tab(source,|app|assert_eq!(app.document.title,"UI link source")).unwrap();
        if disposition==LinkDisposition::BackgroundTab{assert_eq!(session.active(window).unwrap(),Some(source));}
    }
}

#[test]
fn native_ui_menu_scope_refuses_keyup_input_membership_and_logical_window_aba(){
    for change in 0..6{let files=Files::new();let(mut session,mut ui,window,source)=files.setup();let point=point(&session,source,"a");let context=ui.link_context(&mut session,window,point.0,point.1).unwrap().unwrap();
        match change{
            0=>{let before=session.observe_tab(source,|app|app.platform_request_scope.clone()).unwrap();ui.key_event(&mut session,window,KeyboardInput{down:false,key_code:0x41,key:"a".into(),..KeyboardInput::default()}).unwrap();assert!(session.observe_tab(source,|app|Rc::ptr_eq(&before,&app.platform_request_scope)).unwrap(),"The keyup fixture must isolate UI menu scope from App request scope");},
            1=>{ui.invalidate_link_context(window);},
            2=>{ui.resize(&mut session,window,940,700).unwrap();ui.resize(&mut session,window,900,700).unwrap();},
            3=>{let tab=ui.new_tab(&mut session,window).unwrap();let replacement=session.active(window).unwrap().unwrap();assert!(tab.repaint);ui.close_tab(&mut session,window,replacement).unwrap();},
            4=>{let(other,_)=session.new_window(600,500).unwrap();ui.resize(&mut session,other,600,532).unwrap();ui.activate_window(&mut session,other).unwrap();ui.activate_window(&mut session,window).unwrap();ui.close_window(&mut session,other).unwrap();},
            _=>{ui.pointer_move(&mut session,window,point.0+1,point.1,true).unwrap();},
        }
        ui.open_link_context(&mut session,context,LinkDisposition::BackgroundTab).unwrap();assert_eq!(session.tab_count(),1);assert_eq!(session.window_count(),1);realm(&mut session,source,0,0);
    }
}

#[test]
fn native_ui_context_capture_cancel_is_safe_and_authored_download_stays_source_owned(){
    let files=Files::new();let(mut session,mut ui,window,source)=files.setup();let point=point(&session,source,"a");
    session.with_tab(source,|app|{let mut data=app.profile.data().clone();data.settings.download_folder=Some(files.0.join("downloads").display().to_string());app.profile.save(data).unwrap();}).unwrap();
    eval(&mut session,source,"a.setAttribute('download','menu.bin');");let context=ui.link_context(&mut session,window,point.0,point.1).unwrap().unwrap();
    // The popup's own temporary capture cancellation must not invalidate its menu.
    ui.pointer_cancel(&mut session,window).unwrap();ui.open_link_context(&mut session,context,LinkDisposition::NewWindow).unwrap();
    assert_eq!(session.tab_count(),1);assert_eq!(session.window_count(),1);session.observe_tab(source,|app|assert_eq!(app.downloads.entries().len(),1)).unwrap();realm(&mut session,source,0,0);
}

#[test]
fn native_ui_wrong_release_button_and_legacy_wrapper_cannot_fabricate_anchor_input(){
    let files=Files::new();let(mut session,mut ui,window,source)=files.setup();let point=point(&session,source,"a");
    ui.link_pointer_down(&mut session,window,point.0,point.1,0,false).unwrap();ui.link_pointer_up(&mut session,window,point.0,point.1,input(1,false,false)).unwrap();
    ui.link_pointer_down(&mut session,window,point.0,point.1,0,false).unwrap();ui.pointer_up(&mut session,window,point.0,point.1).unwrap();assert_eq!(session.tab_count(),1);realm(&mut session,source,0,0);
    // Browser strip controls keep their original down/up semantics through native APIs.
    let geometry=ui.geometry(&session,window).unwrap();let(_,r)=geometry.controls.iter().find(|(hit,_)|*hit==Hit::New).unwrap();let point=((r.left+r.right)/2,(r.top+r.bottom)/2);
    release(&mut ui,&mut session,window,point,input(0,false,false));assert_eq!(session.tab_count(),2);assert_ne!(session.active(window).unwrap(),Some(source));
    session.observe_tab(source,|app|assert!(matches!(app.focus,Focus::Page))).unwrap();
}


#[test]
fn native_ui_zoom_and_alt_meta_fields_remain_actual_without_new_disposition_meanings(){
    let files=Files::new();let(mut session,mut ui,window,source)=files.setup();session.with_tab(source,|app|app.set_zoom(1.5)).unwrap();
    let point=point(&session,source,"a");let css=session.observe_tab(source,|app|(f64::from(point.0)/f64::from(app.zoom),f64::from(point.1-STRIP_HEIGHT-app.content_top())/f64::from(app.zoom))).unwrap();
    let mouse=AnchorMouseInput{alt:true,meta:true,..input(0,true,false)};release(&mut ui,&mut session,window,point,mouse);
    assert_eq!(session.tab_count(),2);assert_eq!(session.active(window).unwrap(),Some(source));
    eval(&mut session,source,&format!("if(!actualTrusted||actual!=='0,true,false,true,true,-407,291,0,{},{}')throw 'actual zoom/modifiers '+actual;",css.0,css.1));realm(&mut session,source,1,0);
}

#[test]
fn native_ui_callback_navigation_retains_effects_and_suppresses_modified_default(){
    let files=Files::new();let(_http,mut session,mut ui,window,source)=http_setup(&files);let point=point(&session,source,"a");
    eval(&mut session,source,"a.onclick=function(){location.href='other.html';document.title='source queued';};");
    release(&mut ui,&mut session,window,point,input(0,true,true));assert_eq!(session.tab_count(),1);assert_eq!(session.window_count(),1);realm(&mut session,source,1,0);
    session.observe_tab(source,|app|{assert_eq!(app.document.title,"source queued");assert!(app.scripting.as_ref().unwrap().has_navigation_request());}).unwrap();
    session.with_tab(source,|app|{app.poll_load();app.wait_for_load();assert_eq!(app.document.title,"Authored next");}).unwrap();
}

#[test]
fn native_ui_pre_native_menu_origin_refuses_same_owner_replacement_and_input_geometry_aba(){
    for change in 0..4{let files=Files::new();let(mut session,mut ui,window,source)=files.setup();let point=point(&session,source,"a");
        let origin=ui.link_context_origin(&mut session,window).unwrap().unwrap();
        match change{
            0=>{ui.pointer_cancel(&mut session,window).unwrap();},
            1=>{session.with_tab(source,|app|app.open(&files.0.join("source.html"))).unwrap();},
            2=>{ui.invalidate_link_context(window);},
            _=>{session.with_tab(source,|app|{app.scroll_by(60);app.scroll_by(-60);}).unwrap();},
        }
        let context=ui.link_context_from_origin(&mut session,origin,point.0,point.1).unwrap();assert_eq!(context.is_some(),change==0);
        if let Some(context)=context{ui.open_link_context(&mut session,context,LinkDisposition::BackgroundTab).unwrap();}
        assert_eq!(session.tab_count(),if change==0{2}else{1});realm(&mut session,source,0,0);
    }
}

#[test]
fn native_ui_drag_selection_and_page_bookmark_drop_keep_original_nonlink_release(){
    for bookmark in [false,true]{let files=Files::new();let(mut session,mut ui,window,source)=files.setup();let mut point=point(&session,source,"a");point.0=14;
        ui.link_pointer_down(&mut session,window,point.0,point.1,0,false).unwrap();
        let end=if bookmark{session.observe_tab(source,|app|(90,STRIP_HEIGHT+app.chrome_bottom()-10)).unwrap()}else{(point.0+45,point.1)};
        ui.pointer_move(&mut session,window,end.0,end.1,true).unwrap();ui.link_pointer_up(&mut session,window,end.0,end.1,input(0,true,false)).unwrap();
        assert_eq!(session.tab_count(),1);realm(&mut session,source,0,0);
        session.observe_tab(source,|app|{
            assert!(matches!(app.drag,crate::app::Drag::None));
            if bookmark{assert!(app.profile.data().bookmarks.iter().any(|entry|entry.url.ends_with("target.html")),"Original page-to-bookmark drop was lost");}
            else{let selection=app.selection.expect("Original text selection was lost");assert!(selection.start!=selection.end,"Dragged selection did not extend");}
        }).unwrap();
    }
}


#[test]
fn native_ui_unpaired_cancellation_without_origin_never_edits_replacement_input(){
    let files=Files::new();let(mut session,mut ui,window,source)=files.setup();
    let replacement=session.new_tab(window).unwrap();ui.refresh(&session,window).unwrap();
    session.with_tab(replacement,|app|{app.utf16_input(0xd83e);assert_eq!(app.pending_surrogate,Some(0xd83e));}).unwrap();
    ui.cancel_link_pointer(&mut session,window,None).unwrap();session.observe_tab(replacement,|app|assert_eq!(app.pending_surrogate,Some(0xd83e))).unwrap();
    assert_eq!(session.active(window).unwrap(),Some(replacement));realm(&mut session,source,0,0);
}


#[test]
fn native_ui_old_envelope_cancellation_does_not_consume_replacement_gesture(){
    let files=Files::new();let(mut session,mut ui,window,source)=files.setup();let replacement=session.new_tab(window).unwrap();
    session.with_tab(replacement,|app|app.open(&files.0.join("source.html"))).unwrap();ui.refresh(&session,window).unwrap();
    let point=point(&session,replacement,"a");ui.link_pointer_down(&mut session,window,point.0,point.1,0,false).unwrap();
    ui.cancel_link_pointer(&mut session,window,Some(source)).unwrap();ui.link_pointer_up(&mut session,window,point.0,point.1,input(0,true,false)).unwrap();
    assert_eq!(session.tab_count(),3);assert_eq!(session.active(window).unwrap(),Some(replacement));realm(&mut session,replacement,1,0);realm(&mut session,source,0,0);
}


// The two authored-navigation controls require a genuine HTTP document. The
// browser deliberately refuses script navigation from local files; do not fake
// its App/Location origin or relax that production boundary for a fixture.
#[derive(Default)]
struct NavigationProgress{
    header_bytes:std::sync::atomic::AtomicUsize,
    incomplete_peers:std::sync::atomic::AtomicUsize,
    completed_responses:std::sync::atomic::AtomicUsize,
}
struct NavigationSite{
    address:std::net::SocketAddr,stop:std::sync::Arc<std::sync::atomic::AtomicBool>,worker:Option<std::thread::JoinHandle<()>>,
    progress:std::sync::Arc<NavigationProgress>,
}
impl NavigationSite{
    fn new(files:&Files)->Self{
        use std::io::{Read,Write};use std::sync::{Arc,atomic::{AtomicBool,Ordering}};use std::time::Duration;
        let source=fs::read(files.0.join("source.html")).unwrap();let target=fs::read(files.0.join("target.html")).unwrap();let other=fs::read(files.0.join("other.html")).unwrap();
        let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();listener.set_nonblocking(true).unwrap();let address=listener.local_addr().unwrap();
        let stop=Arc::new(AtomicBool::new(false));let stopped=stop.clone();let progress=Arc::new(NavigationProgress::default());let observed=progress.clone();
        let worker=std::thread::spawn(move||{let mut requests=0;'accept:while !stopped.load(Ordering::Relaxed){match listener.accept(){
            Ok((mut stream,_))=>{
                requests+=1;assert!(requests<=16,"Original navigation fixture request bound exceeded");
                // Accepted Winsock sockets inherit the polling listener mode.
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut header=Vec::new();let mut byte=[0u8;1];while !header.ends_with(b"\r\n\r\n"){
                    assert!(header.len()<16384,"Original navigation fixture header bound exceeded");if stream.read_exact(&mut byte).is_err(){observed.incomplete_peers.fetch_add(1,Ordering::SeqCst);continue 'accept;}header.push(byte[0]);observed.header_bytes.fetch_add(1,Ordering::SeqCst);
                }
                let header=String::from_utf8(header).unwrap();let path=header.lines().next().unwrap().split_whitespace().nth(1).unwrap();
                let(status,body)=match path{"/source"=>("200 OK",source.as_slice()),"/target.html"=>("200 OK",target.as_slice()),"/other.html"=>("200 OK",other.as_slice()),_=>("404 Not Found",b"<title>Unexpected navigation fixture route</title>".as_slice())};
                let response=format!("HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());stream.write_all(response.as_bytes()).unwrap();stream.write_all(body).unwrap();
                observed.completed_responses.fetch_add(1,Ordering::SeqCst);
            },
            Err(error)if error.kind()==std::io::ErrorKind::WouldBlock=>std::thread::sleep(Duration::from_millis(1)),
            Err(error)=>panic!("{error}"),
        }}});
        Self{address,stop,worker:Some(worker),progress}
    }
}
impl Drop for NavigationSite{fn drop(&mut self){self.stop.store(true,std::sync::atomic::Ordering::Relaxed);if let Some(worker)=self.worker.take(){worker.join().unwrap();}}}

// Observe the actual private fixture stages before completing each request.
// These controls do not infer which incomplete-peer condition occurred in a
// historical App failure, and do not replace any native navigation assertion.
fn navigation_fixture_wait(mut observed:impl FnMut()->bool){
    let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
    while !observed(){
        assert!(std::time::Instant::now()<deadline,"Original navigation fixture did not reach the required request stage");
        std::thread::yield_now();
    }
}
fn navigation_fixture_client(site:&NavigationSite)->std::net::TcpStream{
    let stream=std::net::TcpStream::connect(site.address).expect("Navigation fixture must keep listening after an incomplete peer");
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
    stream.set_write_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
    stream
}
fn navigation_fixture_response(mut client:std::net::TcpStream)->Vec<u8>{
    use std::io::Read;
    let mut response=Vec::new();
    client.read_to_end(&mut response).expect("Navigation fixture must return the complete framed response");
    let split=response.windows(4).position(|part|part==b"\r\n\r\n").expect("HTTP response header terminator");
    let header=std::str::from_utf8(&response[..split]).unwrap();
    assert!(header.starts_with("HTTP/1.1 200 OK\r\n"),"{header}");
    let length=header.lines().find_map(|line|line.strip_prefix("Content-Length: ")).unwrap().parse::<usize>().unwrap();
    let body=response[split+4..].to_vec();
    assert_eq!(body.len(),length,"Original navigation response must contain exactly its declared bytes");
    body
}
#[test]
fn native_ui_navigation_fixture_keeps_listening_after_cancelled_peer(){
    use std::io::Write;use std::sync::atomic::Ordering;
    let files=Files::new();let(local,local_ui,_,_)=files.setup();drop(local_ui);drop(local);
    let expected=fs::read(files.0.join("source.html")).unwrap();let site=NavigationSite::new(&files);
    drop(navigation_fixture_client(&site));
    navigation_fixture_wait(||site.progress.incomplete_peers.load(Ordering::SeqCst)==1);
    assert_eq!(site.progress.header_bytes.load(Ordering::SeqCst),0);
    assert_eq!(site.progress.completed_responses.load(Ordering::SeqCst),0);
    let mut client=navigation_fixture_client(&site);client.write_all(b"GET /source HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
    assert_eq!(navigation_fixture_response(client),expected);
    assert_eq!(site.progress.completed_responses.load(Ordering::SeqCst),1);
    assert_eq!(site.progress.incomplete_peers.load(Ordering::SeqCst),1);
}
#[test]
fn native_ui_navigation_fixture_waits_for_fragmented_header(){
    use std::io::Write;use std::sync::atomic::Ordering;
    let files=Files::new();let(local,local_ui,_,_)=files.setup();drop(local_ui);drop(local);
    let expected=fs::read(files.0.join("source.html")).unwrap();let site=NavigationSite::new(&files);
    let mut client=navigation_fixture_client(&site);let prefix=b"GET /source HTTP/1.1\r\nHost: localhost\r\n";client.write_all(prefix).unwrap();
    navigation_fixture_wait(||site.progress.header_bytes.load(Ordering::SeqCst)==prefix.len()||site.progress.incomplete_peers.load(Ordering::SeqCst)!=0);
    assert_eq!(site.progress.header_bytes.load(Ordering::SeqCst),prefix.len());
    assert_eq!(site.progress.incomplete_peers.load(Ordering::SeqCst),0,"Navigation fixture must wait for the remaining header bytes");
    assert_eq!(site.progress.completed_responses.load(Ordering::SeqCst),0);
    client.write_all(b"\r\n").unwrap();assert_eq!(navigation_fixture_response(client),expected);
    assert_eq!(site.progress.completed_responses.load(Ordering::SeqCst),1);
    assert_eq!(site.progress.incomplete_peers.load(Ordering::SeqCst),0);
}
fn http_setup(files:&Files)->(NavigationSite,Session,SessionUi,WindowId,TabId){
    // Reuse and validate the identical source/target/other fixture bytes. The
    // temporary original local session owns only its ordinary memory services.
    let(local,local_ui,_,_)=files.setup();drop(local_ui);drop(local);
    let http=NavigationSite::new(files);let mut app=App::new();app.open_address(&format!("http://{}/source",http.address));app.wait_for_load();assert!(!app.failed,"{}",app.status);assert_eq!(app.document.title,"UI link source");
    let mut session=Session::adopt(Box::new(app),super::super::Limits::default()).map_err(|(_,error)|error).unwrap();let window=session.window_ids().next().unwrap();let tab=session.active(window).unwrap().unwrap();
    let mut ui=SessionUi::new();ui.resize(&mut session,window,900,700).unwrap();(http,session,ui,window,tab)
}

// Passive hover is presentation only. A menu retains the original anchor even
// when hover leaves its rendered box; these calls do not manufacture a click.
#[test]
fn native_ui_context_survives_passive_hover_and_leave_without_author_callback(){
    for change in 0..3{let files=Files::new();let(mut session,mut ui,window,source)=files.setup();let point=point(&session,source,"a");
        let expected=session.observe_tab(source,|app|crate::navigation::resolve("target.html",&app.base_location).unwrap()).unwrap();
        let scope=session.observe_tab(source,|app|app.platform_request_scope.clone()).unwrap();
        let context=ui.link_context(&mut session,window,point.0,point.1).unwrap().unwrap();
        if change!=1{ui.pointer_move(&mut session,window,5,5,false).unwrap();}
        if change!=0{ui.pointer_leave(&mut session,window).unwrap();}
        assert!(session.observe_tab(source,|app|Rc::ptr_eq(&scope,&app.platform_request_scope)).unwrap(),"Passive presentation changed the App request scope");
        let out=ui.open_link_context(&mut session,context,LinkDisposition::BackgroundTab).unwrap();
        assert!(out.opened.is_none());assert_eq!(session.tab_count(),2);assert_eq!(session.window_count(),1);assert_eq!(session.active(window).unwrap(),Some(source));
        let destination=*session.window_tabs(window).unwrap().last().unwrap();assert_ne!(destination,source);
        session.observe_tab(destination,|app|{assert_eq!(app.location,expected);assert_eq!(app.document.title,"UI link target");}).unwrap();
        session.observe_tab(source,|app|assert_eq!(app.document.title,"UI link source")).unwrap();realm(&mut session,source,0,0);
        eval(&mut session,source,"if(document.getElementById('q').value!=='seed')throw 'passive hover changed editable value';");
    }
}

#[test]
fn native_ui_context_refuses_dragging_move_even_after_passive_leave(){
    let files=Files::new();let(mut session,mut ui,window,source)=files.setup();let point=point(&session,source,"a");
    let context=ui.link_context(&mut session,window,point.0,point.1).unwrap().unwrap();
    ui.pointer_move(&mut session,window,point.0+1,point.1,true).unwrap();ui.pointer_leave(&mut session,window).unwrap();
    ui.open_link_context(&mut session,context,LinkDisposition::BackgroundTab).unwrap();
    assert_eq!(session.tab_count(),1);assert_eq!(session.window_count(),1);assert_eq!(session.active(window).unwrap(),Some(source));
    session.observe_tab(source,|app|assert_eq!(app.document.title,"UI link source")).unwrap();realm(&mut session,source,0,0);
}

// Test-only App descendant adapters: preserve exact location/root comparison
// and real console evaluation without exposing production App internals.
impl App {
    pub(crate) fn context_hover_test_locations(&self)->(crate::navigation::Location,crate::navigation::Location){
        (self.location.clone(),self.base_location.clone())
    }
    pub(crate) fn context_hover_test_assert_author_counts_zero(&mut self){
        let page=self.scripting.as_mut().unwrap();
        let result=page.console_eval(&mut self.document,&mut self.forms,"if(calls!==0)throw 'context dispatched an author click or auxclick';");
        assert_eq!(result.errors,0,"{}",page.report());
    }
}
