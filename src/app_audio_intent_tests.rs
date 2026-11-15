//! Desired-state App controls with the existing genuine PCM mock owner. No
//! native WinMM device, authored container, JS operation or audibility claim.
use super::*;

fn clip() -> Arc<Clip> {
    Arc::new(Clip::parse(include_bytes!("../tools/fixtures/original-tone.wav").to_vec(),crate::audio::Limits::default(),&||Ok(())).unwrap())
}
fn app() -> (App,Arc<AtomicBool>) {
    let mut app=App::new();let source=Arc::new(AtomicBool::new(false));
    app.audio_worker=Some(Worker::mock_for_app(app.wake.clone()).unwrap());
    app.install_audio_for_source(Some(clip()),Some(source.clone()));(app,source)
}
fn settle(app:&mut App) {
    let deadline=Instant::now()+Duration::from_secs(3);
    while app.audio.as_ref().is_some_and(|player|player.pending) {
        assert!(Instant::now()<deadline,"Real mock PCM acknowledgement timed out: {}",app.audio_report());
        app.poll_audio();std::thread::sleep(Duration::from_millis(1));
    }
    assert!(app.audio.as_ref().is_some_and(|player|player.error.is_none()),"{}",app.audio_report());
}

#[test]
fn local_audio_sync_desired_pause_metadata_noop_preserves_error_and_guards_admission() {
    let mut app=App::new();assert!(!app.audio_sync_play()&&!app.audio_sync_pause());assert!(app.audio_worker.is_none());
    let source=Arc::new(AtomicBool::new(false));app.install_audio_for_source(Some(clip()),Some(source.clone()));
    assert!(app.audio_sync_pause());assert!(app.audio_worker.is_none());assert_eq!(app.audio.as_ref().unwrap().generation,0);
    app.audio.as_mut().unwrap().error=Some("Original retained diagnostic".into());
    assert!(app.audio_sync_pause());assert_eq!(app.audio.as_ref().unwrap().error.as_deref(),Some("Original retained diagnostic"));
    assert_eq!(app.audio.as_ref().unwrap().generation,0);assert!(app.audio_worker.is_none());
    app.audio.as_mut().unwrap().enabled=false;
    assert!(!app.audio_sync_play()&&!app.audio_sync_pause());assert!(app.audio_worker.is_none());
    app.audio.as_mut().unwrap().enabled=true;source.store(true,Ordering::Relaxed);
    assert!(!app.audio_sync_play());assert!(app.audio.is_none());assert!(app.audio_worker.is_none());
    let (mut app,_)=self::app();app.audio.as_mut().unwrap().generation=u64::MAX;
    assert!(!app.audio_sync_play());let player=app.audio.as_ref().unwrap();
    assert!(!player.enabled&&!player.pending&&player.deadline.is_none());assert!(player.error.as_deref().unwrap().contains("exhausted"));
    assert!(!app.audio_sync_pause());
}

#[test]
fn local_audio_sync_desired_play_pause_keep_exact_sample_and_do_not_duplicate_pending_or_active_intent() {
    let (mut app,source)=app();
    app.submit_audio(Action::Seek {frame:600,resume:false});settle(&mut app);
    assert_eq!(app.audio.as_ref().unwrap().frame,600);assert!(!app.audio.as_ref().unwrap().opened);
    assert!(app.audio_sync_play());let playing_generation=app.audio.as_ref().unwrap().generation;
    assert!(app.audio.as_ref().unwrap().pending);assert!(app.audio_sync_play());assert_eq!(app.audio.as_ref().unwrap().generation,playing_generation);
    settle(&mut app);let player=app.audio.as_ref().unwrap();
    assert!(player.playing&&player.opened);assert_eq!(player.frame,600);
    assert!(Arc::ptr_eq(player.source_retired.as_ref().unwrap(),&source));
    assert!(app.audio_sync_play());assert_eq!(app.audio.as_ref().unwrap().generation,playing_generation);
    assert!(app.audio_sync_pause());let paused_generation=app.audio.as_ref().unwrap().generation;
    assert!(app.audio_sync_pause());assert_eq!(app.audio.as_ref().unwrap().generation,paused_generation);
    settle(&mut app);let player=app.audio.as_ref().unwrap();
    assert!(!player.playing&&player.opened);assert_eq!(player.frame,600);
    assert!(player.queued>0&&player.submitted>0&&player.retired<player.submitted,
        "A genuine pause keeps native queued ownership; an invented seek would reset it.");
    assert!(app.audio_sync_pause());assert_eq!(app.audio.as_ref().unwrap().generation,paused_generation);
}

#[test]
fn local_audio_sync_desired_pause_preserves_real_pending_seek_target_and_paused_seek_opens_no_device() {
    let (mut app,_)=app();
    app.submit_audio(Action::Seek {frame:3000,resume:false});let paused_seek_generation=app.audio.as_ref().unwrap().generation;
    assert_eq!(app.audio.as_ref().unwrap().desired_frame,Some(3000));assert!(app.audio_sync_pause());
    assert_eq!(app.audio.as_ref().unwrap().generation,paused_seek_generation);settle(&mut app);
    assert_eq!(app.audio.as_ref().unwrap().frame,3000);assert!(!app.audio.as_ref().unwrap().opened);
    // An explicit resume intent may reach the mock device; Pause must preserve
    // the actual requested target, regardless of whether that command ran first.
    app.submit_audio(Action::Seek {frame:5000,resume:true});
    assert_eq!(app.audio.as_ref().unwrap().frame,3000);assert_eq!(app.audio.as_ref().unwrap().desired_frame,Some(5000));
    assert!(app.audio_sync_pause());let generation=app.audio.as_ref().unwrap().generation;
    assert!(!app.audio.as_ref().unwrap().desired_playing);assert_eq!(app.audio.as_ref().unwrap().desired_frame,Some(5000));
    assert!(app.audio_sync_pause());assert_eq!(app.audio.as_ref().unwrap().generation,generation);
    settle(&mut app);assert_eq!(app.audio.as_ref().unwrap().frame,5000);assert!(!app.audio.as_ref().unwrap().playing);
}

#[test]
fn local_audio_sync_desired_play_uses_pending_seek_target_and_retirement_refuses_later_intent() {
    let (mut app,source)=app();
    app.submit_audio(Action::Seek {frame:7000,resume:false});
    assert!(app.audio_sync_play());assert_eq!(app.audio.as_ref().unwrap().desired_frame,Some(7000));
    assert!(app.audio.as_ref().unwrap().desired_playing);settle(&mut app);
    assert_eq!(app.audio.as_ref().unwrap().frame,7000);assert!(app.audio.as_ref().unwrap().playing);
    source.store(true,Ordering::Relaxed);assert!(!app.audio_sync_pause());assert!(app.audio.is_none());
    let deadline=Instant::now()+Duration::from_secs(3);
    while app.audio_worker.as_ref().unwrap().ownership()!=(false,0,false) {assert!(Instant::now()<deadline);app.poll_audio();std::thread::sleep(Duration::from_millis(1));}
    assert!(!app.audio_sync_play());assert!(!app.audio_sync_pause());
}
