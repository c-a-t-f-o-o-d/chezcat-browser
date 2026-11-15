// Original actual-App HTTP type/admission/seek/source-lifetime controls.
// Full metadata and paused seek only: no WinMM open or advancing audio claim.
mod network_av_app_tests {
    use super::*;
    use std::io::{Read,Write};
    use std::net::{SocketAddr,TcpListener};
    use std::sync::Mutex;
    use std::thread::{self,JoinHandle};
    const MKV:&[u8]=include_bytes!("../tools/fixtures/scene-inter-av.mkv");
    const IVF:&[u8]=include_bytes!("../tools/fixtures/scene-inter.ivf");
    struct Site {address:SocketAddr,stop:Arc<AtomicBool>,requests:Arc<Mutex<Vec<String>>>,thread:Option<JoinHandle<()>>}
    impl Site {
        fn new()->Self {
            let listener=TcpListener::bind("127.0.0.1:0").unwrap();listener.set_nonblocking(true).unwrap();
            let address=listener.local_addr().unwrap();let stop=Arc::new(AtomicBool::new(false));let requests=Arc::new(Mutex::new(Vec::new()));
            let(done,seen)=(stop.clone(),requests.clone());
            let thread=thread::spawn(move||while !done.load(Ordering::Acquire) {
                let(mut stream,_)=match listener.accept(){Ok(value)=>value,Err(error)if error.kind()==std::io::ErrorKind::WouldBlock=>{thread::sleep(Duration::from_millis(1));continue;},Err(error)=>panic!("owned AV accept: {error}")};
                stream.set_nonblocking(false).unwrap();stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut head=Vec::new();let mut byte=[0u8];
                while !head.ends_with(b"\r\n\r\n")&&head.len()<16384 {if stream.read_exact(&mut byte).is_err(){break;}head.push(byte[0]);}
                if !head.ends_with(b"\r\n\r\n"){continue;}
                let request=String::from_utf8_lossy(&head);let path=request.split_ascii_whitespace().nth(1).unwrap_or("").to_owned();
                seen.lock().unwrap().push(path.clone());
                let mut status="200 OK";let mut headers=String::new();let mut mime="text/html";
                let body=match path.as_str(){
                    "/redirect"=>{status="302 Found";headers="Location: /looks.ivf\r\n".into();Vec::new()},
                    "/looks.ivf"=>{mime="video/x-matroska";MKV.to_vec()},
                    "/ivf.mkv"=>{mime="video/x-ivf";IVF.to_vec()},
                    "/wrong"=>{mime="video/x-ivf";MKV.to_vec()},
                    _=>{
                        let source=if path=="/wrong-page"{"/wrong"}else if path=="/page"{"/redirect"}else{"/looks.ivf"};
                        let policy=if path=="/denied-page"{"media-src 'none'"}else{"media-src 'self'"};
                        let muted=if path=="/muted-page"{"muted"}else{""};
                        format!("<!doctype html><head><meta http-equiv=\"Content-Security-Policy\" content=\"{policy}\"><title>Original network AV</title><style>body{{margin:0}}video{{display:block;object-fit:none;object-position:left top}}</style></head><body><video id=v controls width=480 height=360 {muted} src='{source}'></video><script>var video=document.getElementById('v'),errors=0;video.addEventListener('error',function(){{errors++;}});</script></body>").into_bytes()
                    }
                };
                let head=format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n",body.len());
                let _=stream.write_all(head.as_bytes()).and_then(|_|stream.write_all(&body));
            });
            Self{address,stop,requests,thread:Some(thread)}
        }
        fn url(&self,path:&str)->String{format!("http://{}{path}",self.address)}
        fn count(&self,path:&str)->usize{self.requests.lock().unwrap().iter().filter(|value|value.as_str()==path).count()}
        fn app(&self,path:&str,small_audio:bool)->App{
            let mut app=App::new();app.resize(800,700);app.scripts_enabled=true;app.images_enabled=false;
            if small_audio {app.audio_limits.input_bytes=1;}
            app.open_address(&self.url(path));app.wait_for_load();assert!(!app.failed,"{}",app.status);app
        }
    }
    impl Drop for Site{fn drop(&mut self){self.stop.store(true,Ordering::Release);if let Some(thread)=self.thread.take(){let result=thread.join();if !thread::panicking(){assert!(result.is_ok());}}}}
    fn settled(app:&mut App){
        let until=Instant::now()+Duration::from_secs(5);
        while app.audio_sync_state().is_some_and(|audio|audio.pending)||app.media.as_ref().is_some_and(|p|p.pending.is_some()||p.seeking){
            assert!(Instant::now()<until,"{}{}",app.media_report(),app.audio_report());app.poll_load();app.poll_page_timer();thread::sleep(Duration::from_millis(1));
        }
        app.flush_embedded_video();
    }
    fn pixels(app:&App,index:usize){
        let player=app.media.as_ref().unwrap();assert_eq!(player.displayed,index);
        let rect=app.page.border_rects[player.node].unwrap();let canvas=app.draw();
        let x=rect.x;let y=rect.y+app.content_top()-app.scroll;
        assert!(x>=0&&y>=0&&(x+64)as usize<=canvas.width&&(y+48)as usize<=canvas.height);
        let reference=&include_bytes!("../tools/fixtures/scene-inter-centered.rgba")[index*64*48*4..(index+1)*64*48*4];
        let(mut sum,mut maximum)=(0u64,0u32);
        for row in 0..48 {for column in 0..64 {
            let actual=canvas.pixels[(y as usize+row)*canvas.width+x as usize+column];let at=(row*64+column)*4;
            for(shift,channel)in[(16,0),(8,1),(0,2)]{let error=((actual>>shift)&255).abs_diff(u32::from(reference[at+channel]));sum+=u64::from(error);maximum=maximum.max(error);}
        }}
        assert!((sum as f64)/(64.0*48.0*3.0)<0.5&&maximum<=2,"Independent centered display differs: sum={sum} max={maximum}");
    }

    #[test]
    fn authored_network_mime_installs_real_pcm_and_paused_seek_then_explicit_source_reload_retires_it(){
        let site=Site::new();let mut app=site.app("/page",false);
        assert!(app.audio.is_some()&&app.media.as_ref().unwrap().sync.is_some());
        assert!(app.audio_worker.is_none());assert_eq!(app.embedded_video_snapshot().unwrap().clock,crate::embedded_video::ClockKind::AcknowledgedPcm);
        pixels(&app,0);assert_eq!(site.count("/redirect"),1);assert_eq!(site.count("/looks.ivf"),1);
        eval(&mut app,"video.currentTime=1.25;");app.flush_embedded_video();settled(&mut app);
        assert_eq!(app.audio_sync_state().unwrap().position,Duration::from_millis(1250));pixels(&app,10);
        assert!(!app.audio_sync_state().unwrap().playing);assert!(!app.media.as_ref().unwrap().clock.playing());
        assert_eq!(app.audio_worker.as_ref().unwrap().ownership(),(false,0,false));
        let old=app.embedded_video.as_ref().unwrap().retired.clone();
        eval(&mut app,"video.removeAttribute('src');");app.flush_embedded_video();
        assert!(!old.load(Ordering::Acquire));assert!(app.audio.is_some()&&app.media.is_some(),"Raw src removal must retain the selected resource until load()");
        eval(&mut app,"video.load();");app.flush_embedded_video();
        assert!(old.load(Ordering::Acquire));assert!(app.audio.is_none()&&app.media.is_none());
        eval(&mut app,"video.src='/ivf.mkv';video.load();");app.wait_for_load();
        assert!(app.audio.is_none()&&app.media.as_ref().unwrap().sync.is_none());pixels(&app,0);
        assert_eq!(site.count("/ivf.mkv"),1);assert_eq!(app.scripting.as_ref().unwrap().error_count(),0);
    }

    #[test]
    fn authored_network_muxed_muting_mismatch_csp_and_audio_limits_never_install_or_retry(){
        for(page,target,contacts,small)in[("/muted-page","/looks.ivf",1,false),("/wrong-page","/wrong",1,false),("/denied-page","/looks.ivf",0,false),("/tiny-page","/looks.ivf",1,true)]{
            let site=Site::new();let mut app=site.app(page,small);
            assert!(app.audio.is_none()&&app.media.is_none(),"{page}");
            assert!(app.embedded_video.as_ref().unwrap().error.is_some());assert_eq!(site.count(target),contacts,"{page}");
            assert!(app.audio_worker.is_none());assert_eq!(app.scripting.as_ref().unwrap().error_count(),0);
            app.reflow();run(&mut app,Duration::from_millis(60));assert_eq!(site.count(target),contacts,"Unchanged failure retried: {page}");
        }
    }
}
