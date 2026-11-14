//! Standalone local video viewer. These are native viewer controls and lifecycle
//! diagnostics, not a claim of HTMLMediaElement, WebM, audio or YouTube support.
use super::{App, Canvas, Focus, View};
use crate::image::Bitmap;
use crate::media::{self, Clock};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Control {
    Toggle,
    Stop,
    Seek(i32),
}
pub(super) struct ControlState {
    pub playing: bool,
    pub ended: bool,
    pub position: Duration,
    pub duration: Duration,
    pub error: bool,
    pub pending: bool,
}
pub(super) struct Player {
    clip: Arc<media::Clip>,
    node: usize,
    poster: Arc<Bitmap>,
    initial: usize,
    clock: Clock,
    generation: u64,
    pending: Option<usize>,
    displayed: usize,
    decoder_enabled: bool,
    error: Option<String>,
    ended_reported: bool,
    seeking: bool,
    pressed: Option<Control>,
    events: VecDeque<&'static str>,
    dropped: u64,
    deadline: Option<Instant>,
    suspended: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut app = App::new();
        app.scripts_enabled = false;
        app.images_enabled = false;
        app.open(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tools/fixtures/scene-intra.ivf"),
        );
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
        app
    }
    fn settle(app: &mut App) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while app
            .media
            .as_ref()
            .is_some_and(|player| player.pending.is_some())
            && Instant::now() < deadline
        {
            app.poll_media();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(app.media.as_ref().unwrap().pending.is_none());
    }
    #[test]
    fn local_video_native_controls_pause_seek_real_frames_end_replay_and_stop() {
        let mut app = app();
        let layout = app.page.items.as_ptr();
        let player = app.media.as_ref().unwrap();
        let first = app.images[&player.node].clone();
        assert!(!player.clock.playing());
        app.input_media_action("Seek:2200").unwrap();
        settle(&mut app);
        let player = app.media.as_ref().unwrap();
        assert_eq!(player.displayed, 4);
        assert!(!player.clock.playing());
        assert_ne!(first.pixels, app.images[&player.node].pixels);
        assert_eq!(layout, app.page.items.as_ptr());
        app.input_media_action("Play").unwrap();
        let later = Instant::now() + Duration::from_secs(2);
        app.poll_media_timer_at(later);
        settle(&mut app);
        let player = app.media.as_ref().unwrap();
        assert!(player.clock.ended);
        assert_eq!(player.displayed, 5);
        assert_eq!(
            player
                .events
                .iter()
                .filter(|event| **event == "ended")
                .count(),
            1
        );
        app.poll_media_timer_at(later);
        assert_eq!(
            app.media
                .as_ref()
                .unwrap()
                .events
                .iter()
                .filter(|event| **event == "ended")
                .count(),
            1
        );
        app.input_media_action("Play").unwrap();
        assert_eq!(app.media.as_ref().unwrap().displayed, 0);
        app.input_media_action("Pause").unwrap();
        let paused = app.media.as_ref().unwrap().clock.position(Instant::now());
        app.poll_media_timer_at(later);
        assert_eq!(app.media.as_ref().unwrap().clock.position(later), paused);
        app.input_media_action("Stop").unwrap();
        let player = app.media.as_ref().unwrap();
        assert_eq!(player.clock.position(later), Duration::ZERO);
        assert!(Arc::ptr_eq(&app.images[&player.node], &first));
        app.pointer_down(40, app.content_bottom() - 28, false);
        app.pointer_cancel();
        app.pointer_up(40, app.content_bottom() - 28);
        assert!(!app.media.as_ref().unwrap().clock.playing());
    }
    #[test]
    fn local_video_cancel_navigation_timer_failure_and_small_profile_recover() {
        let mut app = app();
        app.input_media_action("Seek:2200").unwrap();
        app.stop_media();
        std::thread::sleep(Duration::from_millis(15));
        app.poll_media();
        assert_eq!(app.media.as_ref().unwrap().displayed, 0);
        app.input_media_action("Play").unwrap();
        app.script_timer_unavailable();
        let player = app.media.as_ref().unwrap();
        assert!(!player.clock.playing() && player.pending.is_none());
        app.input_media_action("Play").unwrap();
        app.open(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pages/second.html"));
        app.poll_media();
        assert!(app.media.is_none());
        assert!(app.next_media_timer().is_none());
        app.media_limits.pixels = 1;
        app.open(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tools/fixtures/scene-intra.ivf"),
        );
        app.wait_for_load();
        assert!(app.failed && app.media.is_none());
        app.media_limits = media::Limits::default();
        app.open(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tools/fixtures/scene-intra.ivf"),
        );
        app.wait_for_load();
        assert!(!app.failed && app.media.is_some());
        app.input_media_action("Seek:2200").unwrap();
        app.open(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pages/second.html"));
        std::thread::sleep(Duration::from_millis(15));
        app.poll_media();
        assert!(app.media.is_none());
    }
    #[test]
    fn local_video_timer_deadline_is_stable_due_aware_and_suspend_cannot_requeue() {
        let mut app = app();
        app.input_media_action("Play").unwrap();
        let due = app.next_media_timer().unwrap();
        std::thread::sleep(Duration::from_millis(1));
        assert_eq!(app.next_media_timer(), Some(due));
        assert!(!app.poll_media_timer_at(due - Duration::from_millis(1)));
        assert_eq!(app.next_media_timer(), Some(due));
        assert!(app.poll_media_timer_at(due));
        assert!(app.next_media_timer().unwrap() > due);
        app.media_seek(Duration::from_millis(2200), Instant::now());
        app.suspend_media();
        app.poll_media_timer_at(due + Duration::from_secs(5));
        app.poll_media();
        assert!(app.media.as_ref().unwrap().pending.is_none());
        assert!(app.next_media_timer().is_none());
        app.media.as_mut().unwrap().generation = u64::MAX;
        app.stop_media();
        app.input_media_action("Play").unwrap();
        assert!(!app.media.as_ref().unwrap().clock.playing());
        assert!(app
            .media
            .as_ref()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("exhausted"));
    }
    #[test]
    fn local_video_initial_load_is_pending_cancellable_and_can_reopen() {
        let mut app = App::new();
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/fixtures/scene-intra.ivf");
        let previous = app.document.title.clone();
        app.open(&path);
        assert!(app.pending.is_some());
        assert!(app.media.is_none());
        app.stop_loading();
        app.wait_for_load();
        assert!(app.pending.is_none() && app.media.is_none());
        assert_eq!(app.document.title, previous);
        app.open(&path);
        app.wait_for_load();
        assert!(!app.failed && app.media.is_some());
        app.input_media_action("Play").unwrap();
        app.media_seek(Duration::from_millis(2200), Instant::now());
        app.reload();
        assert!(app.pending.is_some());
        assert!(!app.media.as_ref().unwrap().clock.playing());
        assert!(app.media.as_ref().unwrap().pending.is_none());
        assert!(app.next_media_timer().is_none());
        app.wait_for_load();
        assert!(app.media.is_some() && !app.failed);
        app.open(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pages/second.html"));
        app.history_move(-1);
        assert!(app.pending.is_some());
        app.wait_for_load();
        assert!(!app.failed && app.media.is_some());
    }
    #[test]
    fn local_video_late_decode_error_is_real_and_seek_to_valid_frame_recovers() {
        let mut app = app();
        let source = include_bytes!("../tools/fixtures/scene-intra.ivf");
        let mut bytes = source[..32].to_vec();
        let mut offset = 32;
        let mut index = 0;
        while offset < source.len() {
            let size = u32::from_le_bytes(source[offset..offset + 4].try_into().unwrap()) as usize;
            let count = if index == 2 { 10 } else { size };
            bytes.extend_from_slice(&(count as u32).to_le_bytes());
            bytes.extend_from_slice(&source[offset + 4..offset + 12]);
            bytes.extend_from_slice(&source[offset + 12..offset + 12 + count]);
            offset += 12 + size;
            index += 1;
        }
        let asset = media::Clip::load(bytes, media::Limits::default(), &|| Ok(())).unwrap();
        app.install_media(Some(asset));
        app.media_seek(Duration::from_millis(1200), Instant::now());
        settle(&mut app);
        let player = app.media.as_ref().unwrap();
        assert!(player.error.is_some() && !player.clock.playing());
        assert!(!player.events.contains(&"ended"));
        assert_eq!(player.displayed, 0);
        app.media_seek(Duration::ZERO, Instant::now());
        assert!(app.media.as_ref().unwrap().error.is_none());
        app.input_media_action("Play").unwrap();
        app.input_media_action("Stop").unwrap();
        assert_eq!(app.media.as_ref().unwrap().displayed, 0);
    }
    #[test]
    fn local_video_eof_pending_is_not_reported_as_displayed_end_and_restart_cancels_it() {
        let mut app = app();
        let poster = app.media.as_ref().unwrap().poster.clone();
        app.media_seek(Duration::from_secs(3), Instant::now());
        assert_eq!(app.media.as_ref().unwrap().pending, Some(5));
        assert!(app.media_report().contains("ended=false"));
        assert!(app.media_report().contains("timeline_eof=true"));
        app.input_media_action("Play").unwrap();
        std::thread::sleep(Duration::from_millis(15));
        app.poll_media();
        let player = app.media.as_ref().unwrap();
        assert!(player.clock.playing());
        assert_eq!(player.displayed, 0);
        assert!(Arc::ptr_eq(&app.images[&player.node], &poster));
        assert!(!player.ended_reported);
    }
}
impl Player {
    fn event(&mut self, event: &'static str) {
        if self.events.len() == 128 {
            self.events.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.events.push_back(event);
    }
    fn invalidate(&mut self) {
        self.pending = None;
        self.deadline = None;
        if let Some(next) = self.generation.checked_add(1) {
            self.generation = next;
        } else {
            self.decoder_enabled = false;
            self.clock.pause(Instant::now());
            self.error = Some("Video request generation exhausted.".into());
            self.event("error");
        }
    }
}
impl App {
    pub(super) fn install_media(&mut self, asset: Option<media::Asset>) {
        if let Some(worker) = &self.media_worker {
            worker.cancel();
        }
        self.media = None;
        let Some(asset) = asset else {
            return;
        };
        let Some(node) = self
            .document
            .nodes
            .iter()
            .enumerate()
            .find_map(|(node, _)| {
                (self.document.attr(node, "id") == Some("media-frame")).then_some(node)
            })
        else {
            return;
        };
        self.images.insert(node, asset.first.clone());
        let mut player = Player {
            clock: Clock::new(asset.clip.duration),
            clip: asset.clip,
            node,
            poster: asset.first,
            initial: asset.initial_index,
            generation: 0,
            pending: None,
            displayed: asset.initial_index,
            decoder_enabled: true,
            error: None,
            ended_reported: false,
            seeking: false,
            pressed: None,
            events: VecDeque::new(),
            dropped: 0,
            deadline: None,
            suspended: false,
        };
        player.event("loadedmetadata");
        player.event("loadeddata");
        self.media = Some(player);
    }
    pub(super) fn suspend_media(&mut self) {
        self.suspend_audio();
        if let Some(worker) = &self.media_worker {
            worker.cancel();
        }
        if let Some(player) = &mut self.media {
            player.clock.pause(Instant::now());
            player.invalidate();
            player.suspended = true;
            player.seeking = false;
            player.pressed = None;
            player.event("cancel");
        }
    }
    fn media_request_frame(&mut self, now: Instant) {
        let Some(player) = &mut self.media else {
            return;
        };
        if player.suspended
            || !player.decoder_enabled
            || player.error.is_some()
            || player.pending.is_some()
        {
            return;
        }
        if player.clock.playing() && player.deadline.is_none() {
            player.deadline = Some(now + Duration::from_millis(20));
        }
        let index = player.clip.index_at(player.clock.position(now));
        if index == player.displayed {
            if player.seeking {
                player.seeking = false;
                player.event("seeked");
            }
            return;
        }
        if index == player.initial {
            self.images.insert(player.node, player.poster.clone());
            player.displayed = index;
            if player.seeking {
                player.seeking = false;
                player.event("seeked");
            }
            return;
        }
        if self.media_worker.is_none() {
            match media::Worker::new(self.wake.clone()) {
                Ok(worker) => self.media_worker = Some(worker),
                Err(error) => {
                    player.clock.pause(now);
                    player.error = Some(error);
                    player.event("error");
                    return;
                }
            }
        }
        player.pending = Some(index);
        if player.deadline.is_none() {
            player.deadline = Some(now + Duration::from_millis(20));
        }
        self.media_worker
            .as_ref()
            .unwrap()
            .submit(player.clip.clone(), player.generation, index);
    }
    pub(super) fn poll_media(&mut self) -> bool {
        let Some(completion) = self.media_worker.as_ref().and_then(|worker| worker.take()) else {
            return false;
        };
        let Some(player) = &mut self.media else {
            return false;
        };
        if !Arc::ptr_eq(&completion.clip, &player.clip)
            || completion.generation != player.generation
            || player.pending != Some(completion.index)
        {
            return false;
        }
        player.pending = None;
        match completion.result {
            Ok(bitmap) => {
                self.images.insert(player.node, bitmap);
                player.displayed = completion.index;
                if player.seeking {
                    player.seeking = false;
                    player.event("seeked");
                }
            }
            Err(error) => {
                player.clock.pause(Instant::now());
                player.error = Some(error);
                player.seeking = false;
                player.event("error");
            }
        }
        self.media_request_frame(Instant::now());
        self.finish_media_tick(Instant::now());
        true
    }
    pub(super) fn next_media_timer(&self) -> Option<Instant> {
        self.media.as_ref().and_then(|player| player.deadline)
    }
    pub(super) fn poll_media_timer_at(&mut self, now: Instant) -> bool {
        if !self.next_media_timer().is_some_and(|due| due <= now) {
            return false;
        }
        if let Some(player) = &mut self.media {
            player.clock.tick(now);
        }
        self.media_request_frame(now);
        self.finish_media_tick(now);
        true
    }
    fn finish_media_tick(&mut self, now: Instant) {
        if let Some(player) = &mut self.media {
            if !player.suspended
                && player.clock.ended
                && player.error.is_none()
                && player.pending.is_none()
                && player.displayed == player.clip.frame_count() - 1
                && !player.ended_reported
            {
                player.ended_reported = true;
                player.event("ended");
            }
            player.deadline = (!player.suspended
                && (player.clock.playing() || player.pending.is_some()))
            .then_some(now + Duration::from_millis(20));
        }
    }
    fn media_seek(&mut self, position: Duration, now: Instant) {
        if let Some(worker) = &self.media_worker {
            worker.cancel();
        }
        if let Some(player) = &mut self.media {
            player.invalidate();
            if !player.decoder_enabled {
                return;
            }
            player.suspended = false;
            player.error = None;
            player.ended_reported = false;
            player.clock.seek(position, now);
            player.seeking = true;
            player.event("seeking");
        }
        self.media_request_frame(now);
        self.finish_media_tick(now);
    }
    pub(super) fn stop_media(&mut self) {
        self.stop_audio();
        if let Some(worker) = &self.media_worker {
            worker.cancel();
        }
        if let Some(player) = &mut self.media {
            player.clock.pause(Instant::now());
            player.clock.seek(Duration::ZERO, Instant::now());
            player.invalidate();
            player.suspended = true;
            if player.decoder_enabled {
                player.error = None;
            }
            player.ended_reported = false;
            player.seeking = false;
            player.displayed = player.initial;
            self.images.insert(player.node, player.poster.clone());
            player.event("stop");
        }
    }
    fn media_activate(&mut self, control: Control) {
        if self.audio.is_some() {
            self.activate_audio(control);
            return;
        }
        let now = Instant::now();
        match control {
            Control::Stop => self.stop_media(),
            Control::Seek(x) => {
                let (start, width) = self.media_track();
                if let Some(player) = &self.media {
                    let fraction = f64::from((x - start).clamp(0, width)) / f64::from(width);
                    let target = player.clip.duration.mul_f64(fraction);
                    self.media_seek(target, now);
                }
            }
            Control::Toggle => {
                if self
                    .media
                    .as_ref()
                    .is_some_and(|player| player.clock.playing())
                {
                    if let Some(worker) = &self.media_worker {
                        worker.cancel();
                    }
                    if let Some(player) = &mut self.media {
                        player.clock.pause(now);
                        player.invalidate();
                        player.event("pause");
                    }
                } else if let Some(player) = &mut self.media {
                    if !player.decoder_enabled {
                        return;
                    }
                    if player.clock.ended {
                        if let Some(worker) = &self.media_worker {
                            worker.cancel();
                        }
                        player.invalidate();
                        if !player.decoder_enabled {
                            return;
                        }
                        player.seeking = false;
                        player.displayed = player.initial;
                        self.images.insert(player.node, player.poster.clone());
                    }
                    player.suspended = false;
                    player.error = None;
                    player.ended_reported = false;
                    player.clock.play(now);
                    player.event("play");
                }
                self.media_request_frame(now);
                self.finish_media_tick(now);
            }
        }
    }
    fn media_visible(&self) -> bool {
        (self.media.is_some() || self.audio.is_some())
            && self.view == View::Page
            && !self.menu
            && self.browser_panel.is_none()
            && !self.downloads_open()
    }
    fn media_control_state(&self) -> Option<ControlState> {
        if let Some(state) = self.audio_control_state() {
            return Some(state);
        }
        self.media.as_ref().map(|player| ControlState {
            playing: player.clock.playing(),
            ended: player.ended_reported,
            position: player.clock.position(Instant::now()),
            duration: player.clip.duration,
            error: player.error.is_some(),
            pending: player.pending.is_some(),
        })
    }
    pub(super) fn media_track(&self) -> (i32, i32) {
        (170, (self.width - 194).max(1))
    }
    pub(super) fn media_hit(&self, x: i32, y: i32) -> Option<Control> {
        if !self.media_visible()
            || self.content_bottom() - 44 < self.content_top()
            || y < self.content_bottom() - 44
            || y >= self.content_bottom()
        {
            return None;
        }
        if (8..84).contains(&x) {
            Some(Control::Toggle)
        } else if (90..160).contains(&x) {
            Some(Control::Stop)
        } else {
            let (start, width) = self.media_track();
            (x >= start && x <= start + width).then_some(Control::Seek(x))
        }
    }
    pub(super) fn media_pointer_down(&mut self, x: i32, y: i32) -> bool {
        let Some(control) = self.media_hit(x, y) else {
            return false;
        };
        if let Some(player) = &mut self.media {
            player.pressed = Some(control);
        }
        self.press_audio(control);
        self.focus = Focus::Page;
        self.inspector.focused = false;
        self.drag = super::Drag::None;
        true
    }
    pub(super) fn media_pointer_up(&mut self, x: i32, y: i32) -> bool {
        let pressed = self
            .take_audio_press()
            .or_else(|| self.media.as_mut().and_then(|player| player.pressed.take()));
        let Some(pressed) = pressed else {
            return false;
        };
        if let Some(hit) = self.media_hit(x, y) {
            if hit == pressed || matches!((hit, pressed), (Control::Seek(_), Control::Seek(_))) {
                self.media_activate(hit);
            }
        }
        true
    }
    pub(super) fn media_pointer_cancel(&mut self) {
        self.cancel_audio_press();
        if let Some(player) = &mut self.media {
            player.pressed = None;
        }
    }
    pub(super) fn media_key(&mut self, key: u32) -> bool {
        if self.focus != Focus::Page || !self.media_visible() {
            return false;
        }
        match key {
            0x20 => self.media_activate(Control::Toggle),
            0x25 | 0x27 => {
                if self.audio.is_some() {
                    self.audio_seek_relative(key == 0x27);
                    return true;
                }
                let player = self.media.as_ref().unwrap();
                let current = player.clock.position(Instant::now());
                let target = if key == 0x25 {
                    current.saturating_sub(Duration::from_secs(5))
                } else {
                    current.saturating_add(Duration::from_secs(5))
                };
                self.media_seek(target, Instant::now());
            }
            _ => return false,
        }
        true
    }
    pub(super) fn draw_media_controls(&self, canvas: &mut Canvas) {
        if !self.media_visible() {
            return;
        }
        let state = self.media_control_state().unwrap();
        let y = self.content_bottom() - 44;
        if y < self.content_top() {
            return;
        }
        canvas.fill_rect(0, y, self.width, 44, 0xff162d3d);
        canvas.fill_rect(8, y + 5, 76, 30, 0xff346277);
        canvas.fill_rect(90, y + 5, 70, 30, 0xff346277);
        canvas.text(
            16,
            y + 15,
            if state.playing {
                "Pause"
            } else if state.ended {
                "Replay"
            } else {
                "Play"
            },
            1,
            0xffffffff,
        );
        canvas.text(100, y + 15, "Stop", 1, 0xffffffff);
        let (start, width) = self.media_track();
        canvas.fill_rect(start, y + 8, width, 8, 0xff6d8794);
        let fraction = state.position.as_secs_f64() / state.duration.as_secs_f64();
        canvas.fill_rect(
            start,
            y + 8,
            (f64::from(width) * fraction) as i32,
            8,
            0xff62cfba,
        );
        canvas.text(
            start,
            y + 24,
            &format!(
                "{:.2} / {:.2} s{}",
                state.position.as_secs_f64(),
                state.duration.as_secs_f64(),
                if state.error {
                    "  media error"
                } else if state.pending {
                    "  working"
                } else {
                    ""
                }
            ),
            1,
            0xffffffff,
        );
    }
    pub(super) fn media_report(&self) -> String {
        let Some(player) = &self.media else {
            return "Local video: none\n".into();
        };
        let rect = self.page.items.iter().find_map(|item| match item {
            crate::layout::PaintItem::Image {
                node,
                x,
                y,
                width,
                height,
                ..
            } if *node == player.node => {
                Some((*x, *y + self.content_top() - self.scroll, *width, *height))
            }
            _ => None,
        });
        format!("Local video: standalone IVF/VP8 intra, video only; native viewer events (no HTMLMediaElement)\nMedia playing={} ended={} time_ms={} duration_ms={} frame={} frames={} pending={:?} canvas={}x{} generation={} error={:?}\nMedia timeline_eof={}\nMedia lifecycle dropped={} {}\nMedia rendered rect={rect:?}\n",player.clock.playing(),player.ended_reported,player.clock.position(Instant::now()).as_millis(),player.clip.duration.as_millis(),player.displayed,player.clip.frame_count(),player.pending,player.clip.width,player.clip.height,player.generation,player.error,player.clock.ended,player.dropped,player.events.iter().copied().collect::<Vec<_>>().join(","))
    }
    pub(super) fn input_media_action(&mut self, action: &str) -> Result<(i32, i32), String> {
        if !self.media_visible() {
            return Err("Local media controls are not visible.".into());
        }
        let x = match action {
            "Play" | "Pause" => {
                let playing = self.media_control_state().unwrap().playing;
                if playing != (action == "Pause") {
                    return Err("Media Play/Pause state differs from the requested control.".into());
                }
                40
            }
            "Stop" => 120,
            _ => {
                let millis: u64 = action
                    .strip_prefix("Seek:")
                    .ok_or("Unknown media input control.")?
                    .parse()
                    .map_err(|_| "Invalid media seek milliseconds.")?;
                let duration = self.media_control_state().unwrap().duration;
                let (start, width) = self.media_track();
                let fraction = (Duration::from_millis(millis).min(duration)).as_secs_f64()
                    / duration.as_secs_f64();
                start + (f64::from(width) * fraction).round() as i32
            }
        };
        let y = self.content_bottom() - 28;
        if self.media_hit(x, y).is_none() {
            return Err("Media control is not a visible hit target.".into());
        }
        self.pointer_down(x, y, false);
        self.pointer_up(x, y);
        Ok((x, y))
    }
}
