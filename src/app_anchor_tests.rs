//! Original App anchor/source-authority controls; no native/tab acceptance claim.
use super::*;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn anchor_url_app_idl_mutation_updates_real_hit_without_granting_navigation() {
    let files=Files::new();let mut app=files.page("");
    let source=app.location.clone();let root=source.root.clone();
    let (x,y)=point(&app,"a");
    eval(&mut app,"a.href='other.html#original';if(a.getAttribute('href')!=='other.html#original')throw 'raw IDL';");
    assert_eq!(app.location,source);
    assert_eq!(app.page.link_at(x,y-app.content_top()+app.scroll),Some("other.html#original"));
    assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
    let target=take_link(activate(&mut app,"a",input(0)));
    assert_eq!(target.location,navigation::resolve("other.html#original",&app.base_location).unwrap());
    assert_eq!(target.location.root,root);
    eval(&mut app,"a.href='../outside.html';");
    assert_eq!(app.location,source);
    let hit=app.anchor_hit(x,y,0).unwrap();
    assert!(app.activate_anchor(hit,x,y,false,input(0)).is_err(),"The trusted activation boundary must reject a file-root escape after IDL assignment");
    assert_eq!(app.location.root,root);
}

#[test]
fn anchor_url_app_local_file_projection_preserves_fragment_colons_and_absolute_href() {
    let files=Files::new();let mut app=files.page("");let source=app.location.clone();
    for raw in ["#part:one","other.html#part:one"] {
        eval(&mut app,&format!("a.href='{raw}';document.title=a.href;"));
        assert_eq!(app.document.title,navigation::selection_url(raw,&app.base_location).unwrap());
        assert_eq!(app.document.attr(node(&app,"a"),"href"),Some(raw));
    }
    let absolute=app.document.title.clone();
    eval(&mut app,"a.href=a.href;document.title=a.href;");
    assert_eq!(app.document.attr(node(&app,"a"),"href"),Some(absolute.as_str()));
    assert_eq!(app.document.title,absolute);
    assert_eq!(app.location,source);
    assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
}

#[test]
fn anchor_app_native_scrollbar_strip_masks_pointer_and_context_but_plain_page_edge_remains_live() {
    let files=Files::new();
    files.write("site/target.html","<title>Strip target</title>");
    for tall in [false,true] {
        let spacer=if tall {"<div style='height:1800px'>Scroll extent</div>"} else {""};
        let path=files.write(if tall {"site/strip-scroll.html"} else {"site/strip-flat.html"},
            &format!("<!doctype html><style>body{{margin:0}}a{{position:fixed;left:790px;top:0;width:10px;height:32px;background:red}}</style><body><a id=a href=target.html>Go</a>{spacer}</body>"));
        let mut app=App::new();app.resize(800,600);app.open(&path);app.wait_for_load();
        assert!(!app.failed,"{}",app.status);
        assert_eq!(app.max_scroll()>0,tall,"Fixture scrollbar admission");
        let (x,y)=(795,app.content_top()+16);let anchor=node(&app,"a");
        assert!(x>=app.width-12&&x<app.width);
        assert_eq!(app.page.link_node_at(x,y-app.content_top()+app.scroll),Some(anchor),
            "Fixture must contain the genuine underlying anchor hit");
        if tall {
            assert!(app.anchor_hit(x,y,0).is_none(),"Native scrollbar must own the strip before pointer anchor capture");
            assert!(app.anchor_context(x,y).is_none(),"Native scrollbar must own the strip before context capture");
        } else {
            assert!(app.anchor_hit(x,y,0).is_some(),"No scrollbar: visible page edge remains a pointer target");
            assert!(app.anchor_context(x,y).is_some(),"No scrollbar: visible page edge remains a context target");
        }
    }
}

#[test]
fn anchor_app_live_href_mutation_rebuilds_rendered_destination_and_removal_revokes_hit() {
    let files=Files::new();let mut app=files.page("");
    let (x,y)=point(&app,"a");let original=app.location.clone();
    eval(&mut app,"a.setAttribute('href','other.html');");
    assert_eq!(app.page.link_at(x,y-app.content_top()+app.scroll),Some("other.html"),
        "The painted destination must follow the real live attribute");
    let target=take_link(activate(&mut app,"a",input(0)));
    assert_eq!(target.location,navigation::resolve("other.html",&app.base_location).unwrap());
    assert_eq!(app.location,original,"Changing the attribute must not navigate");
    eval(&mut app,"a.removeAttribute('href');");
    assert!(app.page.link_at(x,y-app.content_top()+app.scroll).is_none());
    assert!(app.anchor_hit(x,y,0).is_none());
    assert!(app.anchor_context(x,y).is_none());
    eval(&mut app,"a.setAttribute('href','');");
    assert_eq!(app.page.link_at(x,y-app.content_top()+app.scroll),Some(""));
    assert!(app.anchor_hit(x,y,0).is_some());
    assert!(app.anchor_context(x,y).is_some());
    assert_eq!(app.location,original);
}

#[test]
fn anchor_app_live_href_presence_controls_user_agent_link_style_and_inert_metadata() {
    let files=Files::new();let mut app=files.page("");let anchor=node(&app,"a");
    let linked=crate::css::compute(&app.document,&app.styles)[anchor].clone();
    assert!(linked.underline,"The fixture must start with genuine link styling");
    eval(&mut app,"a.removeAttribute('href');");
    let plain=crate::css::compute(&app.document,&app.styles)[anchor].clone();
    assert!(!plain.underline,"An anchor without href must lose UA link decoration");
    assert_ne!(plain.color,linked.color,"Link color must follow actual attribute presence");
    eval(&mut app,"a.setAttributeNS('urn:anchor-metadata','m:href','other.html');a.setAttributeNS(null,'HREF','other.html');");
    let metadata=crate::css::compute(&app.document,&app.styles)[anchor].clone();
    assert_eq!((metadata.color,metadata.underline),(plain.color,plain.underline),
        "Namespaced and uppercase metadata must not create a live HTML link");
    eval(&mut app,"a.setAttribute('href','');");
    let restored=crate::css::compute(&app.document,&app.styles)[anchor].clone();
    assert_eq!((restored.color,restored.underline),(linked.color,linked.underline));
}
struct Files(PathBuf);
impl Files{
    fn new()->Self{let mut bytes=[0u8;8];crate::random::fill(&mut bytes).unwrap();let token:String=bytes.iter().map(|b|format!("{b:02x}")).collect();let root=std::env::temp_dir().join(format!("chezcat-anchor-{token}"));assert!(root.is_absolute());fs::create_dir(&root).unwrap();Self(root)}
    fn write(&self,path:&str,text:&str)->PathBuf{let path=self.0.join(path);fs::create_dir_all(path.parent().unwrap()).unwrap();fs::write(&path,text).unwrap();path}
    fn page(&self,handler:&str)->App{self.write("site/target.html","<title>Original target</title><p>Target");self.write("site/other.html","<title>Original other</title><p>Other");let path=self.write("site/source.html",&format!("<!doctype html><title>Original source</title><style>body{{margin:0}}a{{display:block;width:160px;height:32px;position:relative}}#spacer{{height:1800px}}</style><body><a id=a href=target.html>Original anchor</a><a id=b href=target.html>Same href second</a><div id=spacer>Scroll</div><script>var calls=0;var a=document.getElementById('a');{handler}</script></body>"));let mut app=App::new();app.resize(800,600);app.open(&path);app.wait_for_load();assert!(!app.failed,"Fixture admission: {}",app.status);assert_eq!(app.document.title,"Original source");app}
}
impl Drop for Files{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
fn node(app:&App,id:&str)->usize{(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(id)).unwrap()}
fn point(app:&App,id:&str)->(i32,i32){let n=node(app,id);let r=app.page.border_rects[n].unwrap();let p=(r.x+r.width/2,app.content_top()+r.y-app.scroll+r.height/2);assert_eq!(app.page.link_node_at(p.0,p.1-app.content_top()+app.scroll),Some(n),"Fixture anchor is not actually hit");p}
fn input(button:u8)->AnchorMouseInput{AnchorMouseInput{button,screen_x:73.0,screen_y:91.0,..AnchorMouseInput::default()}}
fn activate(app:&mut App,id:&str,input:AnchorMouseInput)->AnchorDefault{let(x,y)=point(app,id);let hit=app.anchor_hit(x,y,input.button).unwrap_or_else(||panic!("Missing anchor capture: surface={}, pending={}, document href={:?}, painted href={:?}, status={}",app.anchor_surface(x,y),app.scripting.as_ref().is_some_and(|page|page.has_navigation_request()),app.document.attr(node(app,id),"href"),app.page.link_at(x,y-app.content_top()+app.scroll),app.status));app.activate_anchor(hit,x,y,false,input).unwrap()}
fn take_link(value:AnchorDefault)->OwnedLinkTarget{match value{AnchorDefault::Link(target,_)=>target,_=>panic!("Actual anchor did not export its accepted target")}}
fn eval(app:&mut App,source:&str){let page=app.scripting.as_mut().unwrap();let result=page.console_eval(&mut app.document,&mut app.forms,source);assert_eq!(result.errors,0,"{}",page.report());if result.layout_changed{app.reflow();}}

#[test]
fn anchor_app_real_trusted_click_runs_capture_target_bubble_before_default_and_preserves_local_root(){
    let files=Files::new();let mut app=files.page("var order='';window.addEventListener('click',function(e){order+='W';},true);a.addEventListener('click',function(e){if(!(e instanceof MouseEvent)||!e.isTrusted||e.button!==0||e.buttons!==0||!e.ctrlKey||e.shiftKey||e.screenX!==73||e.screenY!==91||e.target!==a)throw 'actual native mouse';calls++;order+='A';});document.addEventListener('click',function(e){order+='D';document.title=order;});");
    let source=app.location.clone();let mut mouse=input(0);mouse.ctrl=true;let target=match activate(&mut app,"a",mouse){AnchorDefault::Link(target,AnchorTrigger::Pointer(actual))=>{assert!(actual.ctrl);target},_=>panic!("Uncancelled genuine click exported no target")};assert_eq!(app.document.title,"WAD");assert_eq!(app.location,source);assert_eq!(target.location.root,source.root);let mut destination=App::new();destination.open_link_target(target);destination.wait_for_load();assert!(!destination.failed,"{}",destination.status);assert_eq!(destination.document.title,"Original target");assert_eq!(destination.location.root,source.root);eval(&mut app,"if(calls!==1)throw 'callback count';");
}

#[test]
fn anchor_app_cancelled_primary_and_middle_keep_effects_and_suppress_download_or_link(){
    let files=Files::new();for button in [0,1]{let mut app=files.page("a.setAttribute('download','original.bin');a.onclick=function(e){calls++;document.title='cancelled primary';return false;};a.addEventListener('auxclick',function(e){if(!(e instanceof MouseEvent)||e.button!==1)throw 'middle';calls++;document.title='cancelled middle';e.preventDefault();});");let before=app.location.clone();assert!(matches!(activate(&mut app,"a",input(button)),AnchorDefault::Cancelled));assert_eq!(app.location,before);assert!(app.downloads.entries().is_empty());assert!(app.document.title.starts_with("cancelled"));eval(&mut app,"if(calls!==1)throw 'callback lost or duplicated';");}
}

#[test]
fn anchor_app_handler_movement_and_href_change_follow_same_live_anchor_after_microtasks(){
    let files=Files::new();let mut app=files.page("a.onclick=function(e){calls++;this.style.left='250px';Promise.resolve().then(function(){a.setAttribute('href','other.html');document.title='handler moved';});};");let old=point(&app,"a");let original=node(&app,"a");let target=take_link(activate(&mut app,"a",input(0)));assert_eq!(app.document.title,"handler moved");assert_ne!(app.page.link_node_at(old.0,old.1-app.content_top()+app.scroll),Some(original));assert_eq!(target.location,navigation::resolve("other.html",&app.base_location).unwrap());let mut destination=App::new();destination.open_link_target(target);assert_eq!(destination.document.title,"Original other");eval(&mut app,"if(calls!==1)throw 'callback count';");
}

#[test]
fn anchor_app_authored_download_wins_over_modified_open_and_stays_source_owned(){
    let files=Files::new();let mut app=files.page("a.onclick=function(){calls++;a.setAttribute('download','changed.bin');document.title='download callback';};");let mut data=app.profile.data().clone();data.settings.download_folder=Some(files.0.join("downloads").display().to_string());app.profile.save(data).unwrap();let source=app.location.clone();let mut mouse=input(0);mouse.ctrl=true;mouse.shift=true;assert!(matches!(activate(&mut app,"a",mouse),AnchorDefault::Download));assert_eq!(app.location,source);assert_eq!(app.document.title,"download callback");assert_eq!(app.downloads.entries().len(),1);assert!(app.downloads_open());eval(&mut app,"if(calls!==1)throw 'download callback count';");
}

#[test]
fn anchor_app_stale_geometry_aba_source_and_foreign_app_tokens_run_no_author_callback(){
    let files=Files::new();let mut app=files.page("a.onclick=function(){calls++;document.title='BAD';};");let(x,y)=point(&app,"a");let hit=app.anchor_hit(x,y,0).unwrap();app.resize(840,600);app.resize(800,600);assert!(app.activate_anchor(hit,x,y,false,input(0)).is_err());let hit=app.anchor_hit(x,y,0).unwrap();app.scroll_by(100);app.scroll_by(-100);assert!(app.activate_anchor(hit,x,y,false,input(0)).is_err());let hit=app.anchor_hit(x,y,0).unwrap();let mut foreign=files.page("");assert!(foreign.activate_anchor(hit,x,y,false,input(0)).is_err());let hit=app.anchor_hit(x,y,0).unwrap();app.open(&files.0.join("site/other.html"));assert!(app.activate_anchor(hit,x,y,false,input(0)).is_err());assert_eq!(app.document.title,"Original other");
}

#[test]
fn anchor_app_identical_href_different_anchor_and_button_or_drag_mismatch_refuse_before_dispatch(){
    let files=Files::new();let path=files.write("tiny/source.html","<!doctype html><style>body{margin:0}a{display:inline-block;width:3px;height:24px;background:red}</style><body><a id=a href=target.html></a><a id=b href=target.html></a><script>var calls=0;document.addEventListener('click',function(){calls++;document.title='BAD';});</script>");files.write("tiny/target.html","<title>Target</title>");let mut app=App::new();app.open(&path);assert!(!app.failed,"{}",app.status);let a=point(&app,"a");let b=point(&app,"b");assert!((a.0-b.0).abs()<=4&&(a.1-b.1).abs()<=4);let hit=app.anchor_hit(a.0,a.1,0).unwrap();assert!(app.activate_anchor(hit,b.0,b.1,false,input(0)).is_err());let hit=app.anchor_hit(a.0,a.1,0).unwrap();assert!(app.activate_anchor(hit,a.0,a.1,false,input(1)).is_err());let hit=app.anchor_hit(a.0,a.1,0).unwrap();assert!(app.activate_anchor(hit,a.0,a.1,true,input(0)).is_err());eval(&mut app,"if(calls!==0)throw 'refused gesture dispatched';");
}

#[test]
fn anchor_app_context_open_uses_original_snapshot_without_forging_click_and_refuses_mutation(){
    let files=Files::new();let mut app=files.page("a.onclick=function(){calls++;document.title='BAD';};");let(x,y)=point(&app,"a");let context=app.anchor_context(x,y).unwrap();let target=match app.open_anchor_context(context).unwrap(){AnchorDefault::Link(target,AnchorTrigger::Context)=>target,_=>panic!("Context Open did not preserve its source")};assert_eq!(target.location,navigation::resolve("target.html",&app.base_location).unwrap());assert_eq!(app.document.title,"Original source");eval(&mut app,"if(calls!==0)throw 'context forged click';");let context=app.anchor_context(x,y).unwrap();eval(&mut app,"a.setAttribute('href','other.html');a.setAttribute('href','target.html');");assert!(app.open_anchor_context(context).is_err());let context=app.anchor_context(x,y).unwrap();app.invalidate_platform_requests();assert!(app.open_anchor_context(context).is_err());
}

#[test]
fn anchor_app_local_base_resolution_retains_root_and_denies_escape_after_actual_callback(){
    let files=Files::new();files.write("site/sub/index.html","<title>Base</title>");files.write("site/sub/target.html","<title>Sub target</title>");files.write("outside.html","<title>Outside private</title>");let path=files.write("site/base.html","<!doctype html><base href='sub/index.html'><title>Root source</title><a id=a href=target.html style='display:block;width:150px;height:30px'>Go</a><script>var a=document.getElementById('a');a.onclick=function(){document.title='callback retained';};</script>");let mut app=App::new();app.open(&path);assert!(!app.failed,"{}",app.status);let root=app.location.root.clone();let target=take_link(activate(&mut app,"a",input(0)));assert_eq!(target.location.root,root);let mut destination=App::new();destination.open_link_target(target);assert_eq!(destination.document.title,"Sub target");assert_eq!(destination.location.root,root);eval(&mut app,"a.setAttribute('href','../../outside.html');");let(x,y)=point(&app,"a");let hit=app.anchor_hit(x,y,0).unwrap();assert!(app.activate_anchor(hit,x,y,false,input(0)).is_err());assert_eq!(app.document.title,"callback retained");assert_eq!(app.location.root,root);
}


#[derive(Clone)]
struct Request {method:String,path:String,cookie:String,body:Vec<u8>}
struct Site {address:std::net::SocketAddr,requests:std::sync::Arc<std::sync::Mutex<Vec<Request>>>,stop:std::sync::Arc<std::sync::atomic::AtomicBool>,worker:Option<std::thread::JoinHandle<()>>}
impl Site {
    fn new(handler:&str)->Self{
        use std::io::{Read,Write};use std::sync::{Arc,Mutex};use std::sync::atomic::{AtomicBool,Ordering};
        let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();listener.set_nonblocking(true).unwrap();let address=listener.local_addr().unwrap();let requests=Arc::new(Mutex::new(Vec::new()));let stop=Arc::new(AtomicBool::new(false));let(received,stopped)=(requests.clone(),stop.clone());
        let source=format!("<!doctype html><base href='http://127.0.0.1:{}/base/'><title>HTTP source</title><style>body{{margin:0}}a{{display:block;width:160px;height:32px}}</style><body><a id=a href=target>Original HTTP anchor</a><script>var a=document.getElementById('a');{handler}</script></body>",address.port());
        let worker=std::thread::spawn(move||{let mut peers=Vec::new();while !stopped.load(Ordering::Relaxed){match listener.accept(){Ok((mut stream,_))=>{let received=received.clone();let source=source.clone();peers.push(std::thread::spawn(move||{
            // The listener polls for shutdown. Each accepted peer performs timed blocking I/O.
            stream.set_nonblocking(false).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(8))).unwrap();stream.set_write_timeout(Some(Duration::from_secs(8))).unwrap();let mut bytes=Vec::new();let mut byte=[0u8;1];while !bytes.ends_with(b"\r\n\r\n"){if bytes.len()>16384||stream.read_exact(&mut byte).is_err(){return;}bytes.push(byte[0]);}let header=String::from_utf8_lossy(&bytes);let mut first=header.lines().next().unwrap().split_whitespace();let method=first.next().unwrap().to_owned();let path=first.next().unwrap().to_owned();let cookie=header.lines().find_map(|line|line.split_once(':').filter(|(name,_)|name.eq_ignore_ascii_case("cookie")).map(|(_,value)|value.trim().to_owned())).unwrap_or_default();let length=header.lines().find_map(|line|line.split_once(':').filter(|(name,_)|name.eq_ignore_ascii_case("content-length")).map(|(_,value)|value.trim().parse::<usize>().unwrap())).unwrap_or(0);assert!(length<=32768);let mut body=vec![0;length];if stream.read_exact(&mut body).is_err(){return;}received.lock().unwrap().push(Request{method,path:path.clone(),cookie,body});
            let(status,extra,body)=match path.as_str(){"/source"=>("200 OK","",source),"/base/target"=>("302 Found","Location: /final\r\n",String::new()),"/script-target"=>("200 OK","","<title>Author navigation</title><p>Script target</p>".into()),_=>("200 OK","","<title>Captured target</title><p>Target</p>".into())};let header=format!("HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n",body.len());let _=stream.write_all(header.as_bytes());let _=stream.write_all(body.as_bytes());
        }));},Err(error)if error.kind()==std::io::ErrorKind::WouldBlock=>std::thread::sleep(Duration::from_millis(1)),Err(error)=>panic!("{error}")}}for peer in peers{peer.join().unwrap();}});
        Self{address,requests,stop,worker:Some(worker)}
    }
    fn source(&self)->String{format!("http://localhost:{}/source",self.address.port())}
    fn logs(&self)->Vec<Request>{self.requests.lock().unwrap().clone()}
}
impl Drop for Site{fn drop(&mut self){self.stop.store(true,std::sync::atomic::Ordering::Relaxed);if let Some(worker)=self.worker.take(){worker.join().unwrap();}}}

#[test]
fn anchor_app_cross_site_effective_base_keeps_top_url_cookie_initiator_new_chain_and_get_after_real_post(){
    let site=Site::new("");let mut source=App::new();let location=navigation::address(&site.source(),&Location::home()).unwrap();source.start_network_request(location.clone(),NavigationKind::New,crate::http::FetchMode::Normal,crate::http::Request::FormPost{body:b"source=only".to_vec(),origin:location.url.clone()});source.wait_for_load();assert!(!source.failed,"{}",source.status);assert!(source.current_request.is_post());assert_eq!(source.document.title,"HTTP source");assert_eq!(source.location.url.as_ref().unwrap().host,"localhost");assert_eq!(source.base_location.url.as_ref().unwrap().host,"127.0.0.1");let target_url=crate::url::parse(&format!("http://{}/base/target",site.address)).unwrap().0;source.cookies.set_http(&target_url,"target_strict=retained; SameSite=Strict; Path=/").unwrap();assert!(source.cookies.request_header(&target_url,true,true).contains("target_strict="));source.cookie_same_site=true;
    let target=take_link(activate(&mut source,"a",input(0)));assert_eq!(target.initiator,source.location.url);let mut destination=App::new();destination.cookies=source.cookies.clone();destination.open_link_target(target);destination.wait_for_load();assert!(!destination.failed,"{}",destination.status);assert_eq!(destination.document.title,"Captured target");assert!(!destination.cookie_same_site);assert!(!destination.current_request.is_post());assert!(destination.resubmission.is_none());
    let logs=site.logs();let posted=logs.iter().find(|request|request.path=="/source").unwrap();assert_eq!(posted.method,"POST");assert_eq!(posted.body,b"source=only");for path in ["/base/target","/final"]{let request=logs.iter().find(|request|request.path==path).unwrap();assert_eq!(request.method,"GET");assert!(request.body.is_empty());assert!(!request.cookie.contains("target_strict="),"Effective base or destination was substituted for original initiator on {path}");}
}

#[test]
fn anchor_app_callback_navigation_remains_queued_and_suppresses_link_without_losing_cookie_effect(){
    let site=Site::new("a.onclick=function(){document.cookie='accepted=effect; Path=/';location.href='/script-target';document.title='callback queued';};");let mut app=App::new();app.open_address(&site.source());app.wait_for_load();assert!(!app.failed,"{}",app.status);assert_eq!(app.document.title,"HTTP source");assert!(matches!(activate(&mut app,"a",input(0)),AnchorDefault::Cancelled));assert_eq!(app.document.title,"callback queued");assert!(app.scripting.as_ref().unwrap().has_navigation_request());assert!(app.cookies.document(app.location.url.as_ref().unwrap()).contains("accepted=effect"));app.poll_load();app.wait_for_load();assert_eq!(app.document.title,"Author navigation");assert!(!site.logs().iter().any(|request|request.path=="/base/target"));
}


#[test]
fn anchor_app_detached_callback_target_cancels_but_ordinary_author_throw_preserves_default(){
    let files=Files::new();let mut removed=files.page("a.onclick=function(){this.parentNode.removeChild(this);document.title='removed original';};");assert!(matches!(activate(&mut removed,"a",input(0)),AnchorDefault::Cancelled));assert_eq!(removed.document.title,"removed original");let original=node(&removed,"a");assert!(!removed.anchor_connected(original),"Detached node remained active because its arena index survived");
    let mut thrown=files.page("a.onclick=function(){document.title='effect before throw';throw 'ordinary original listener error';};");let target=take_link(activate(&mut thrown,"a",input(0)));assert_eq!(thrown.document.title,"effect before throw");assert!(!thrown.failed);let mut destination=App::new();destination.open_link_target(target);assert_eq!(destination.document.title,"Original target");
}


#[test]
fn anchor_app_same_site_source_sends_strict_on_new_chain_despite_prior_cross_site_response_bit(){
    let site=Site::new("");let mut source=App::new();source.open_address(&site.source());source.wait_for_load();assert_eq!(source.document.title,"HTTP source");assert!(!source.cookie_same_site,"Typed home-to-web source should begin cross-site");let url=crate::url::parse(&format!("http://localhost:{}/same-site-target",site.address.port())).unwrap().0;source.cookies.set_http(&url,"same_strict=accepted; SameSite=Strict; Path=/").unwrap();eval(&mut source,&format!("a.setAttribute('href','{}');",url.display()));let target=take_link(activate(&mut source,"a",input(0)));let mut destination=App::new();destination.cookies=source.cookies.clone();destination.open_link_target(target);destination.wait_for_load();assert_eq!(destination.document.title,"Captured target");assert!(destination.cookie_same_site);let request=site.logs().into_iter().find(|request|request.path=="/same-site-target").unwrap();assert_eq!(request.method,"GET");assert!(request.cookie.contains("same_strict=accepted"),"Lost source initiator or reused prior response's cross-site reload bit");
}

#[test]
fn anchor_app_local_opaque_initiator_overrides_destination_prior_web_origin(){
    let site = Site::new("");
    let files = Files::new();
    let mut source = files.page("");
    let url = crate::url::parse(&format!("http://{}/local-target", site.address)).unwrap().0;
    eval(&mut source, &format!("a.setAttribute('href','{}');", url.display()));
    let target = take_link(activate(&mut source, "a", input(0)));
    assert!(target.initiator.is_none());

    let mut destination = App::new();
    destination.open_address(&format!("http://{}/previous", site.address));
    destination.wait_for_load();
    assert!(!destination.failed, "Prior page failed: {}; received paths: {:?}", destination.status,
        site.logs().iter().map(|request| request.path.as_str()).collect::<Vec<_>>());
    assert_eq!(destination.document.title, "Captured target");
    destination.cookies.set_http(&url, "destination_strict=must_not_leak; SameSite=Strict; Path=/").unwrap();
    destination.open_link_target(target);
    destination.wait_for_load();
    assert!(!destination.failed, "{}", destination.status);
    assert_eq!(destination.location.url.as_ref(), Some(&url));
    assert_eq!(destination.document.title, "Captured target");
    assert!(!destination.cookie_same_site);
    let request = site.logs().into_iter().find(|request| request.path == "/local-target").unwrap();
    assert_eq!(request.method, "GET");
    assert!(!request.cookie.contains("destination_strict="), "Opaque source origin fell back to destination's prior web origin");
}
