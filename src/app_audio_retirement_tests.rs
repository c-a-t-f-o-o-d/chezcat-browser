//! Actual App admission with genuine mock-worker snapshots. No native device,
//! authored container enablement, JavaScript media or audibility claim.
use super::*;

fn owned_clip() -> Arc<Clip> {
    Arc::new(
        Clip::parse(
            include_bytes!("../tools/fixtures/original-tone.wav").to_vec(),
            crate::audio::Limits::default(),
            &|| Ok(()),
        )
        .unwrap(),
    )
}
fn source_snapshot(
    worker: &Worker,
    source: &Arc<AtomicBool>,
    generation: u64,
) -> crate::audio_output::Snapshot {
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        let (snapshot, notice) = worker.poll();
        assert!(
            notice.is_none(),
            "Unexpected mock ownership notice: {notice:?}"
        );
        if let Some(snapshot) = snapshot {
            if snapshot.generation == generation && snapshot.matches_source(Some(source)) {
                return snapshot;
            }
        }
        assert!(
            Instant::now() < until,
            "Actual mock source snapshot timed out"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn local_audio_app_source_token_rejects_old_and_retired_actual_snapshots_even_with_same_clip_generation(
) {
    let clip = owned_clip();
    let older = Arc::new(AtomicBool::new(false));
    let current = Arc::new(AtomicBool::new(false));
    let worker = Worker::mock_for_app(Arc::new(|| {})).unwrap();
    worker.submit_for_source(
        clip.clone(),
        7,
        Action::Seek {
            frame: 100,
            resume: false,
        },
        Some(older.clone()),
    );
    let old = source_snapshot(&worker, &older, 7);
    let mut app = App::new();
    app.install_audio_for_source(Some(clip.clone()), Some(current.clone()));
    app.audio.as_mut().unwrap().generation = 7;
    assert!(Arc::ptr_eq(&old.clip, &app.audio.as_ref().unwrap().clip));
    assert!(!app.accept_audio_snapshot(old));
    assert_eq!(app.audio.as_ref().unwrap().frame, 0);
    worker.submit_for_source(
        clip.clone(),
        7,
        Action::Seek {
            frame: 200,
            resume: false,
        },
        Some(current.clone()),
    );
    assert!(app.accept_audio_snapshot(source_snapshot(&worker, &current, 7)));
    assert_eq!(app.audio.as_ref().unwrap().frame, 200);
    worker.submit_for_source(
        clip,
        7,
        Action::Seek {
            frame: 300,
            resume: false,
        },
        Some(current.clone()),
    );
    let late = source_snapshot(&worker, &current, 7);
    current.store(true, Ordering::Relaxed);
    assert!(!app.accept_audio_snapshot(late));
    assert_eq!(app.audio.as_ref().unwrap().frame, 200);
    assert!(
        app.audio_control_state().is_none()
            && app.audio_sync_state().is_none()
            && app.next_audio_timer().is_none()
    );
    assert!(app.poll_audio());
    assert!(app.audio.is_none());
    assert_eq!(worker.ownership(), (false, 0, false));
}

#[test]
fn local_audio_app_source_retirement_drops_binding_and_requests_one_owner_clear_without_device_open(
) {
    let mut app = App::new();
    let token = Arc::new(AtomicBool::new(false));
    app.audio_worker = Some(Worker::mock_for_app(app.wake.clone()).unwrap());
    app.install_audio_for_source(Some(owned_clip()), Some(token.clone()));
    app.submit_audio(Action::Seek {
        frame: 500,
        resume: false,
    });
    let until = Instant::now() + Duration::from_secs(3);
    while app.audio.as_ref().unwrap().pending {
        assert!(Instant::now() < until);
        app.poll_audio();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(app.audio.as_ref().unwrap().frame, 500);
    assert!(!app.audio.as_ref().unwrap().opened);
    token.store(true, Ordering::Relaxed);
    assert!(app.poll_audio());
    assert!(app.audio.is_none());
    let until = Instant::now() + Duration::from_secs(3);
    while app.audio_worker.as_ref().unwrap().ownership().2 {
        assert!(Instant::now() < until);
        app.poll_audio();
        std::thread::sleep(Duration::from_millis(1));
    }
    for _ in 0..3 {
        assert!(!app.poll_audio());
        assert_eq!(
            app.audio_worker.as_ref().unwrap().ownership(),
            (false, 0, false)
        );
    }
    assert!(app.audio_report().contains("Local audio: none"));
    assert!(app
        .audio_report()
        .contains("Audio owner opened=false retained_buffers=0 busy=false"));
    // The original standalone API still selects an unfenced metadata-only clip.
    app.install_audio(Some(owned_clip()));
    assert!(app.audio.as_ref().unwrap().source_retired.is_none());
}

#[test]
fn local_audio_app_late_retired_install_preserves_current_source() {
    let clip = owned_clip();
    let current = Arc::new(AtomicBool::new(false));
    let retired = Arc::new(AtomicBool::new(true));
    let mut app = App::new();
    app.audio_worker = Some(Worker::mock_for_app(app.wake.clone()).unwrap());
    app.install_audio_for_source(Some(clip.clone()), Some(current.clone()));
    app.submit_audio(Action::Seek {
        frame: 600,
        resume: false,
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.audio.as_ref().unwrap().pending {
        assert!(Instant::now() < deadline);
        app.poll_audio();
        std::thread::sleep(Duration::from_millis(1));
    }
    let generation = app.audio.as_ref().unwrap().generation;
    app.install_audio_for_source(Some(clip.clone()), Some(retired.clone()));
    let player = app
        .audio
        .as_ref()
        .expect("late retired installation discarded the current source");
    assert!(Arc::ptr_eq(&player.clip, &clip));
    assert!(Arc::ptr_eq(
        player.source_retired.as_ref().unwrap(),
        &current
    ));
    assert_eq!((player.frame, player.generation), (600, generation));
    assert!(!current.load(Ordering::Relaxed));
    app.submit_audio(Action::Seek {
        frame: 800,
        resume: false,
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.audio.as_ref().unwrap().pending {
        assert!(Instant::now() < deadline);
        app.poll_audio();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(app.audio.as_ref().unwrap().frame, 800);
    // Explicit teardown remains authoritative even with a retired optional tag.
    app.install_audio_for_source(None, Some(retired));
    assert!(app.audio.is_none());
}
