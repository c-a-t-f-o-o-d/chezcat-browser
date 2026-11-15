// Include inside app_media::tests. No playback/device/mock-clock fabrication.
mod interframe_av_app_tests {
    use super::*;
    const CENTERED: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tools/fixtures/scene-inter-centered.rgba"
    ));
    fn path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/fixtures/scene-inter-av.mkv")
    }
    fn inter_av_with_limits(limits: media::Limits) -> App {
        let mut app = App::new();
        app.scripts_enabled = false;
        app.images_enabled = false;
        app.media_limits = limits;
        app.open(&path());
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
        assert!(app.synchronized_media() && app.media.is_some() && app.audio.is_some());
        assert_eq!(app.media.as_ref().unwrap().clip.frame_count(), 24);
        assert!(app.audio_worker.is_none());
        app
    }
    fn assert_frame(app: &App, index: usize) {
        let player = app.media.as_ref().unwrap();
        assert_eq!(player.displayed, index);
        let bitmap = &app.images[&player.node];
        assert_eq!((bitmap.width, bitmap.height), (64, 48));
        assert_eq!(bitmap.pixels.len(), 64 * 48);
        let expected = &CENTERED[index * 64 * 48 * 4..(index + 1) * 64 * 48 * 4];
        let (mut sum, mut maximum) = (0u64, 0u32);
        for (pixel, rgba) in bitmap.pixels.iter().zip(expected.chunks_exact(4)) {
            for (actual, reference) in [
                ((pixel >> 16) & 255, rgba[0]),
                ((pixel >> 8) & 255, rgba[1]),
                (pixel & 255, rgba[2]),
            ] {
                let delta = actual.abs_diff(u32::from(reference));
                sum += u64::from(delta);
                maximum = maximum.max(delta);
            }
            assert_eq!(pixel >> 24, 255);
        }
        assert!(
            sum * 2 < 64 * 48 * 3 && maximum <= 2,
            "A/V dependent frame {index}: RGB sum={sum}, max={maximum}"
        );
    }
    fn assert_selected(app: &App, millis: u64, frame: usize, pcm_frame: u64) {
        assert_frame(app, frame);
        let sync = app.audio_sync_state().unwrap();
        assert_eq!(sync.position, Duration::from_millis(millis));
        assert_eq!(sync.desired, sync.position);
        assert!(!sync.pending && !sync.playing && !sync.wants_play && !sync.drained);
        assert!(sync.error.is_none());
        assert_eq!(app.media_control_state().unwrap().position, sync.position);
        assert!(!app.media.as_ref().unwrap().clock.playing());
        assert!(!app.media_control_state().unwrap().playing);
        // This report comes from the App's accepted genuine PCM worker snapshot.
        // It is a selected sample offset, never a claimed hardware clock/drain.
        let report = app.audio_report();
        assert!(
            report.contains(&format!(" frame={pcm_frame} frames=144000 ")),
            "{report}"
        );
        assert!(
            report.contains("selected metadata sample offset; device not opened"),
            "{report}"
        );
        assert_eq!(
            app.audio_worker.as_ref().unwrap().ownership(),
            (false, 0, false)
        );
    }
    fn poll_for(app: &mut App, duration: Duration) {
        let end = Instant::now() + duration;
        while Instant::now() < end {
            app.poll_load();
            app.poll_page_timer();
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn matroska_interframe_paused_seeks_follow_real_selected_pcm_without_opening_device() {
        let mut app = inter_av_with_limits(media::Limits::default());
        assert_frame(&app, 0);
        for (millis, index, pcm_frame) in [(2200, 17, 105600), (250, 2, 12000)] {
            app.synchronized_seek(Duration::from_millis(millis), false);
            settle_av(&mut app);
            assert_selected(&app, millis, index, pcm_frame);
            poll_for(&mut app, Duration::from_millis(80));
            assert_selected(&app, millis, index, pcm_frame);
        }
        app.open(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pages/second.html"));
        app.wait_for_load();
        let deadline = Instant::now() + Duration::from_secs(3);
        while app
            .audio_worker
            .as_ref()
            .is_some_and(|worker| worker.ownership().2)
        {
            assert!(Instant::now() < deadline, "owned PCM clear did not finish");
            app.poll_load();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(app.media.is_none() && app.audio.is_none());
        assert!(app.next_media_timer().is_none());
        assert!(app
            .audio_worker
            .as_ref()
            .is_none_or(|worker| worker.ownership() == (false, 0, false)));
    }
    #[test]
    fn matroska_interframe_real_dependency_failure_keeps_poster_and_explicit_reload_recovers() {
        let normal = media::Limits::default();
        let mut app = inter_av_with_limits(media::Limits {
            decode_frames: 1,
            ..normal
        });
        assert_frame(&app, 0);
        let clip = app.media.as_ref().unwrap().clip.clone();
        let poster = app.images[&app.media.as_ref().unwrap().node].clone();
        app.synchronized_seek(Duration::from_millis(2200), false);
        settle_av(&mut app);
        let player = app.media.as_ref().unwrap();
        assert!(
            player
                .error
                .as_ref()
                .is_some_and(|error| error.contains("dependency decode-frame allowance")),
            "{}",
            app.media_report()
        );
        assert!(!player.seeking && player.pending.is_none() && !player.clock.playing());
        assert!(Arc::ptr_eq(&app.images[&player.node], &poster));
        assert_frame(&app, 0);
        assert_eq!(
            app.audio_worker.as_ref().unwrap().ownership(),
            (false, 0, false)
        );
        poll_for(&mut app, Duration::from_millis(100));
        assert!(Arc::ptr_eq(&app.media.as_ref().unwrap().clip, &clip));
        assert!(app.media.as_ref().unwrap().error.is_some());
        assert_frame(&app, 0);
        app.media_limits = normal;
        app.reload();
        app.wait_for_load();
        assert!(!app.failed && app.synchronized_media());
        assert!(!Arc::ptr_eq(&app.media.as_ref().unwrap().clip, &clip));
        assert!(app.media.as_ref().unwrap().error.is_none());
        assert_frame(&app, 0);
        app.synchronized_seek(Duration::from_millis(2200), false);
        settle_av(&mut app);
        assert_selected(&app, 2200, 17, 105600);
        app.synchronized_seek(Duration::from_millis(250), false);
        settle_av(&mut app);
        assert_selected(&app, 250, 2, 12000);
    }
}
