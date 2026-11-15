// Include inside app_embedded_video::tests. Original real-worker App controls.
mod interframe_app_tests {
    use super::*;
    const CENTERED: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tools/fixtures/scene-inter-centered.rgba"
    ));
    fn assert_inter_bitmap(bitmap: &crate::image::Bitmap, index: usize) {
        assert!(index < 24);
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
        // Independent centered-bilinear oracle; mean RGB error strictly <0.5.
        assert!(
            sum * 2 < 64 * 48 * 3 && maximum <= 2,
            "dependent frame {index}: RGB sum={sum}, max={maximum}"
        );
    }
    fn assert_inter_frame(app: &App, index: usize) {
        let player = app.media.as_ref().unwrap();
        assert_eq!(player.displayed, index);
        assert_inter_bitmap(&app.images[&player.node], index);
    }
    fn hold_seventeen(app: &mut App) -> media::Completion {
        eval(app,"mediaEvents=[];video.currentTime=2.2;if(video.currentTime!==2.2||!video.seeking||!video.paused)throw 'dependent seek admission';");
        app.apply_media_pause();
        assert_eq!(app.media.as_ref().unwrap().pending, Some(17));
        let completion = take_decoded_frame(app);
        assert_eq!(completion.index, 17);
        assert_inter_bitmap(completion.result.as_ref().unwrap(), 17);
        assert_eq!(
            app.media.as_ref().unwrap().displayed,
            0,
            "worker result was not yet accepted"
        );
        completion
    }
    #[test]
    fn authored_interframe_superseded_real_seek_then_genuine_play_pause_uses_independent_pixels() {
        let mut app = open("authored-inter-video.html");
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
        assert_inter_frame(&app, 0);
        assert_eq!(app.media.as_ref().unwrap().clip.frame_count(), 24);
        let poster = app.images[&app.media.as_ref().unwrap().node].clone();
        let late = hold_seventeen(&mut app);
        let old_generation = late.generation;
        eval(&mut app,"video.currentTime=0.25;if(video.currentTime!==0.25||!video.seeking||!video.paused)throw 'dependent backward seek';");
        app.apply_media_pause();
        assert_ne!(app.media.as_ref().unwrap().generation, old_generation);
        assert_eq!(app.media.as_ref().unwrap().pending, Some(2));
        assert!(
            !app.accept_media_frame(late),
            "superseded real target17 was accepted"
        );
        assert!(Arc::ptr_eq(
            &app.images[&app.media.as_ref().unwrap().node],
            &poster
        ));
        let backward = take_decoded_frame(&app);
        assert_eq!(backward.index, 2);
        assert_inter_bitmap(backward.result.as_ref().unwrap(), 2);
        assert!(app.accept_media_frame(backward));
        assert_inter_frame(&app, 2);
        app.flush_embedded_video();
        eval(&mut app,"if(video.currentTime!==0.25||video.seeking||!video.paused||mediaEvents.indexOf('seeked')<0)throw 'dependent backward publication';var interPlay='pending',interSettles=0;");
        activate_media_document(&mut app);
        eval(&mut app,"var interPromise=video.play();if(!(interPromise instanceof Promise)||video.paused)throw 'dependent genuine Promise';interPromise.then(function(){interSettles++;interPlay='fulfilled';},function(e){throw e;});");
        app.flush_embedded_video();
        run(&mut app, Duration::from_millis(650));
        eval(&mut app,"if(interPlay!=='fulfilled'||interSettles!==1||video.currentTime<0.75)throw 'dependent actual playback';video.pause();if(!video.paused)throw 'dependent pause';");
        app.flush_embedded_video();
        settle(&mut app);
        let position = app.embedded_video_snapshot().unwrap().position;
        let frame = app.media.as_ref().unwrap().displayed;
        assert!(frame >= 6);
        assert_inter_frame(&app, frame);
        assert!(!app.media.as_ref().unwrap().clock.playing());
        run(&mut app, Duration::from_millis(100));
        assert_eq!(app.embedded_video_snapshot().unwrap().position, position);
        assert_inter_frame(&app, frame);
        assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0);
    }
    #[test]
    fn authored_interframe_real_completion_cannot_cross_source_native_stop_or_document() {
        for action in ["replace", "stop", "navigate"] {
            let mut app = open("authored-inter-video.html");
            app.wait_for_load();
            assert_inter_frame(&app, 0);
            let clip = app.media.as_ref().unwrap().clip.clone();
            let retired = app.embedded_video.as_ref().unwrap().retired.clone();
            let poster = app.images[&app.media.as_ref().unwrap().node].clone();
            let late = hold_seventeen(&mut app);
            let generation = late.generation;
            match action {
                "replace" => {
                    eval(&mut app, "video.src='scene-intra.ivf';");
                    assert!(app.process_media_selection());
                    app.wait_for_load();
                }
                "stop" => {
                    app.input_media_action("Stop").unwrap();
                }
                _ => {
                    app.open(
                        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("tools/fixtures/authored-video-next.html"),
                    );
                    app.wait_for_load();
                }
            }
            if action == "stop" {
                // Native Stop retains the source for replay and invalidates
                // pending frame work through the existing player generation.
                assert!(
                    !retired.load(Ordering::Relaxed),
                    "native Stop retired its retained source"
                );
                assert!(Arc::ptr_eq(
                    &app.embedded_video.as_ref().unwrap().retired,
                    &retired
                ));
                assert_ne!(
                    app.media.as_ref().unwrap().generation,
                    generation,
                    "native Stop kept pending generation"
                );
            } else if action == "replace" {
                assert!(
                    retired.load(Ordering::Relaxed),
                    "{action}: source token still active"
                );
            } else {
                // A committed document drops the old source/Page/Player;
                // worker cancellation is independent of a retained token Arc.
                assert!(app
                    .location
                    .path
                    .as_ref()
                    .unwrap()
                    .ends_with("authored-video-next.html"));
                assert!(
                    app.media.is_none() && app.embedded_video.is_none() && app.images.is_empty()
                );
            }
            assert!(
                !app.accept_media_frame(late),
                "{action}: late dependent result accepted"
            );
            run(&mut app, Duration::from_millis(80));
            match action {
                "replace" => {
                    let player = app.media.as_ref().unwrap();
                    assert!(!Arc::ptr_eq(&player.clip, &clip));
                    assert_eq!(player.clip.frame_count(), 6);
                    assert_reference_frame(&app, 0);
                    assert!(!player.seeking && player.pending.is_none());
                }
                "stop" => {
                    let player = app.media.as_ref().unwrap();
                    assert!(Arc::ptr_eq(&player.clip, &clip));
                    assert_eq!(player.displayed, 0);
                    assert!(player.pending.is_none() && !player.seeking && !player.clock.playing());
                    assert!(Arc::ptr_eq(&app.images[&player.node], &poster));
                    assert_inter_frame(&app, 0);
                }
                _ => {
                    assert!(app
                        .location
                        .path
                        .as_ref()
                        .unwrap()
                        .ends_with("authored-video-next.html"));
                    assert!(
                        app.media.is_none()
                            && app.embedded_video.is_none()
                            && app.images.is_empty()
                    );
                }
            }
            assert!(
                app.scripting
                    .as_ref()
                    .is_none_or(|page| page.error_count() == 0),
                "{action}"
            );
        }
    }
}
