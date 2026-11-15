//! Actual local source worker + App admission/lifetime controls; no device.
use super::*;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
struct Files { root: PathBuf, page: PathBuf, next: PathBuf }
impl Files {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("chezcat-authored-av-{}-{}",std::process::id(),NEXT_FIXTURE.fetch_add(1,Ordering::Relaxed)));
        std::fs::create_dir_all(&root).unwrap();
        let page = root.join("player.html"); let next = root.join("next.html");
        std::fs::write(&page, "<!doctype html><title>Original authored AV</title><body style='margin:0'><video id=av src=scene-inter-av.mkv controls width=480 height=360></video></body>").unwrap();
        std::fs::write(&next, "<!doctype html><title>Original next document</title><p>Next</p>").unwrap();
        std::fs::write(root.join("scene-inter-av.mkv"), include_bytes!("../tools/fixtures/scene-inter-av.mkv")).unwrap();
        std::fs::write(root.join("scene-inter.ivf"), include_bytes!("../tools/fixtures/scene-inter.ivf")).unwrap();
        Self { root, page, next }
    }
    fn open(&self) -> App {
        let mut app=App::new();app.resize(800,700);app.scripts_enabled=true;app.images_enabled=false;app.open(&self.page);app.wait_for_load();
        assert!(!app.failed,"{}",app.status);app
    }
}
impl Drop for Files { fn drop(&mut self) { let _=std::fs::remove_dir_all(&self.root); } }
fn eval(app:&mut App, source:&str) {
    let page=app.scripting.as_mut().unwrap();let outcome=page.console_eval(&mut app.document,&mut app.forms,source);
    assert_eq!(outcome.errors,0,"{}",page.report());if outcome.layout_changed {app.reflow();}
}
fn source_completion(app:&App) -> Completion {
    let until=Instant::now()+Duration::from_secs(5);
    loop {if let Some(value)=app.embedded_video_worker.as_ref().unwrap().take(){return value;}
        assert!(Instant::now()<until,"Actual authored source worker timed out");std::thread::sleep(Duration::from_millis(1));}
}
fn settle(app:&mut App) {
    let until=Instant::now()+Duration::from_secs(3);
    while app.audio_sync_state().is_some_and(|state|state.pending)
        || app.media.as_ref().is_some_and(|player|player.pending.is_some()||player.seeking) {
        assert!(Instant::now()<until,"{}{}",app.media_report(),app.audio_report());
        app.poll_load();app.poll_page_timer();std::thread::sleep(Duration::from_millis(1));
    }
    app.flush_embedded_video();
}

#[test]
fn authored_local_av_metadata_installs_both_tracks_before_sync_without_opening_a_device() {
    let files=Files::new();let mut app=files.open();
    let player=app.media.as_ref().unwrap();assert!(player.embedded&&player.sync.is_some());
    assert_eq!((player.clip.width,player.clip.height,player.clip.frame_count()),(64,48,24));
    assert!(!player.clock.playing());assert!(app.audio.is_some()&&app.audio_worker.is_none());
    let source=app.embedded_video.as_ref().unwrap().retired.clone();
    let snapshot=app.embedded_video_snapshot().unwrap();
    assert!(Arc::ptr_eq(&snapshot.retired,&source));assert_eq!(snapshot.clock,crate::embedded_video::ClockKind::AcknowledgedPcm);
    assert_eq!(app.audio_sync_state().unwrap().position,Duration::ZERO);
    assert_eq!(snapshot.position(),Duration::ZERO);assert!(snapshot.paused&&!snapshot.playing);
    assert_eq!(app.images[&player.node].pixels,player.poster.pixels);
    // Use the existing mock Api for actual native controller commands, never
    // a composed/fabricated A/V asset or a physical WinMM device.
    app.audio_worker=Some(crate::audio_output::Worker::mock_for_app(app.wake.clone()).unwrap());
    app.input_media_action("Play").unwrap();settle(&mut app);
    assert!(app.audio_sync_state().unwrap().playing);assert!(!app.media.as_ref().unwrap().clock.playing());
    app.input_media_action("Stop").unwrap();settle(&mut app);
    assert!(app.audio.is_some()&&app.media.as_ref().unwrap().sync.is_some());
    assert!(!source.load(Ordering::Relaxed));assert_eq!(app.audio_sync_state().unwrap().position,Duration::ZERO);
    assert!(!app.audio_sync_state().unwrap().playing);
    assert_eq!(app.audio_worker.as_ref().unwrap().ownership().1,0);
    app.input_media_action("Play").unwrap();settle(&mut app);
    assert!(app.audio_sync_state().unwrap().playing&&app.media.as_ref().unwrap().playing());
    assert!(Arc::ptr_eq(&source,&app.embedded_video.as_ref().unwrap().retired));
    assert!(!app.media.as_ref().unwrap().clock.playing(),"Replay started the unused video wall clock");
    app.stop_media();settle(&mut app);
}

#[test]
fn authored_av_reload_retires_pcm_and_old_source_before_ivf_replacement_or_failure() {
    let files=Files::new();let mut app=files.open();
    let old=app.embedded_video.as_ref().unwrap().retired.clone();
    eval(&mut app,"var v=document.getElementById('av');v.load();");
    assert!(old.load(Ordering::Relaxed));assert!(app.audio_sync_state().is_none(),"Retired PCM was still observable before UI selection");
    assert!(app.process_media_selection());assert!(app.audio.is_none()&&app.media.is_none());
    let late=source_completion(&app);
    eval(&mut app,"v.src='scene-inter.ivf';v.load();");app.process_media_selection();
    assert!(app.audio.is_none());assert!(!app.accept_embedded_video(late));assert!(app.audio.is_none());
    app.wait_for_load();assert!(app.audio.is_none()&&app.media.as_ref().unwrap().sync.is_none());
    eval(&mut app,"v.src='missing.mkv';v.load();");app.process_media_selection();app.wait_for_load();
    assert!(app.audio.is_none()&&app.media.is_none());assert!(app.embedded_video.as_ref().unwrap().error.is_some());
}

#[test]
fn authored_av_source_removal_then_load_and_navigation_clear_both_tracks() {
    let files=Files::new();
    for remove in [true,false] {
        let mut app=files.open();assert!(app.audio.is_some());
        if remove {
            // Existing structural guards still reject removal of a loaded
            // VIDEO. Exercise the supported actual source-removal operation.
            eval(&mut app,"var v=document.getElementById('av');v.removeAttribute('src');if(v.parentNode===null||v.getAttribute('src')!==null)throw 'source removal changed node';");
            // Removing src after selection retains the selected resource,
            // exactly as the existing IVF source contract requires. Explicit
            // load then runs real selection with no source and retires it.
            let source=app.embedded_video.as_ref().unwrap().retired.clone();
            app.flush_embedded_video();assert!(app.audio.is_some()&&app.media.is_some());
            assert!(!source.load(Ordering::Relaxed));
            eval(&mut app,"v.load();");assert!(source.load(Ordering::Relaxed));
            app.flush_embedded_video();app.process_media_selection();
        } else {app.open(&files.next);app.wait_for_load();}
        assert!(app.audio.is_none()&&app.media.is_none());
        assert!(app.audio_sync_state().is_none());
        assert!(app.audio_worker.as_ref().is_none_or(|worker|worker.ownership()==(false,0,false)));
    }
}
