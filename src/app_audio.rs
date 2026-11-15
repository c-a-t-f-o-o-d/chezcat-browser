//! Local PCM viewer controls. Playback state/time come from the device worker,
//! never elapsed UI time. These are native diagnostics, not HTMLMediaElement.
use super::{
    media_ui::{Control, ControlState},
    App,
};
use crate::audio::Clip;
use crate::audio_output::{Action, Worker};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub(super) struct Player {
    clip: Arc<Clip>,
    source_retired: Option<Arc<AtomicBool>>,
    frame: u64,
    playing: bool,
    ended: bool,
    opened: bool,
    pending: bool,
    desired_playing: bool,
    desired_frame: Option<u64>,
    generation: u64,
    enabled: bool,
    suspended: bool,
    error: Option<String>,
    deadline: Option<Instant>,
    pressed: Option<Control>,
    queued: usize,
    submitted: u64,
    retired: u64,
    unit: Option<u32>,
    retained: usize,
    seeking: bool,
    stopping: bool,
    data_seen: bool,
    ended_reported: bool,
    events: VecDeque<&'static str>,
    dropped: u64,
}

#[cfg(test)]
#[path = "app_audio_retirement_tests.rs"]
mod audio_retirement_tests;

pub(super) struct SyncState {pub position:Duration,pub desired:Duration,pub pending:bool,pub playing:bool,pub wants_play:bool,pub drained:bool,pub error:Option<String>}

#[cfg(test)]
mod tests {
    use super::*;
    fn path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/fixtures/original-tone.wav")
    }
    fn app() -> App {
        let mut app = App::new();
        app.scripts_enabled = false;
        app.images_enabled = false;
        app.open(&path());
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
        assert!(app.audio.is_some());
        app
    }
    fn settle(app: &mut App) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while app.audio.as_ref().is_some_and(|player| player.pending) && Instant::now() < deadline {
            app.poll_audio();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!app.audio.as_ref().unwrap().pending);
    }
    #[test]
    fn local_audio_app_metadata_controls_seek_stop_and_pointer_cancel_open_no_device() {
        let mut app = app();
        assert_eq!(app.document.content_type, "audio/wav");
        assert!(app.audio_worker.is_none());
        assert!(app.media.is_none());
        app.input_media_action("Seek:750").unwrap();
        let due = app.next_audio_timer().unwrap();
        assert_eq!(app.next_audio_timer(), Some(due));
        assert!(!app.poll_audio_timer_at(due - Duration::from_millis(1)));
        settle(&mut app);
        let player = app.audio.as_ref().unwrap();
        assert!((35000..37000).contains(&player.frame));
        assert!(!player.playing && !player.opened && player.error.is_none());
        assert!(!player.events.contains(&"loadeddata"));
        app.input_media_action("Stop").unwrap();
        settle(&mut app);
        assert_eq!(app.audio.as_ref().unwrap().frame, 0);
        app.pointer_down(40, app.content_bottom() - 28, false);
        app.pointer_cancel();
        app.pointer_up(40, app.content_bottom() - 28);
        assert!(!app.audio.as_ref().unwrap().pending);
        assert!(!app.audio.as_ref().unwrap().opened);
        app.open(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pages/second.html"));
        app.poll_audio();
        assert!(app.audio.is_none());
        assert!(app.next_audio_timer().is_none());
    }
    #[test]
    fn local_audio_app_initial_cancellation_small_profile_and_generation_exhaustion_recover() {
        let mut app = App::new();
        let previous = app.document.title.clone();
        app.open(&path());
        assert!(app.pending.is_some() && app.audio.is_none());
        app.stop_loading();
        app.wait_for_load();
        assert!(app.audio.is_none());
        assert_eq!(app.document.title, previous);
        app.audio_limits.channels = 1;
        app.open(&path());
        app.wait_for_load();
        assert!(app.failed && app.audio.is_none());
        app.audio_limits = crate::audio::Limits::default();
        app.open(&path());
        app.wait_for_load();
        assert!(!app.failed && app.audio.is_some());
        app.input_media_action("Seek:600").unwrap();
        app.script_timer_unavailable();
        settle(&mut app);
        assert!(app.next_audio_timer().is_none());
        assert!(!app.audio.as_ref().unwrap().opened);
        app.audio.as_mut().unwrap().generation = u64::MAX;
        app.stop_audio();
        assert!(!app.audio.as_ref().unwrap().enabled);
        assert!(app
            .audio
            .as_ref()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("exhausted"));
        assert!(app.next_audio_timer().is_none());
    }
    #[test]
    fn local_audio_app_mock_rapid_seek_stop_play_and_relative_intents_are_preserved() {
        let mut app = app();
        app.audio_worker = Some(Worker::mock_for_app(app.wake.clone()).unwrap());
        app.input_media_action("Seek:750").unwrap();
        let target = app.audio.as_ref().unwrap().desired_frame.unwrap();
        app.activate_audio(Control::Toggle);
        settle(&mut app);
        assert_eq!(app.audio.as_ref().unwrap().frame, target);
        assert!(app.audio.as_ref().unwrap().playing);
        app.stop_audio();
        app.activate_audio(Control::Toggle);
        settle(&mut app);
        assert_eq!(app.audio.as_ref().unwrap().frame, 0);
        assert!(app.audio.as_ref().unwrap().playing);
        let mut bytes = include_bytes!("../tools/fixtures/original-tone.wav").to_vec();
        bytes[24..28].copy_from_slice(&9600u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&38400u32.to_le_bytes());
        let clip =
            Arc::new(Clip::parse(bytes, crate::audio::Limits::default(), &|| Ok(())).unwrap());
        app.install_audio(Some(clip));
        app.audio_seek_relative(true);
        app.audio_seek_relative(true);
        settle(&mut app);
        assert_eq!(app.audio.as_ref().unwrap().frame, 96000);
        assert!(!app.audio.as_ref().unwrap().opened);
        app.activate_audio(Control::Toggle);
        settle(&mut app);
        assert!(app.audio.as_ref().unwrap().playing);
        assert_eq!(app.audio.as_ref().unwrap().frame, 0);
        app.submit_audio(Action::Seek {
            frame: 96000,
            resume: false,
        });
        settle(&mut app);
        assert!(app.audio.as_ref().unwrap().opened);
        assert_eq!(app.audio.as_ref().unwrap().frame, 96000);
        app.activate_audio(Control::Toggle);
        settle(&mut app);
        assert!(app.audio.as_ref().unwrap().playing);
        assert_eq!(app.audio.as_ref().unwrap().frame, 0);
        app.stop_audio();
        settle(&mut app);
    }
}
#[cfg(test)]
mod sync_fault_tests {
    use super::*;
    #[test]
    fn matroska_shared_audio_fault_latches_once_without_timer_spin_or_resume_intent() {
        let mut app=App::new();app.open(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/fixtures/scene-av.mkv"));app.wait_for_load();assert!(!app.failed);
        // Controller-level backend report, separate from native ownership
        // tests in audio_output. This control never opens a device.
        let p=app.audio.as_mut().unwrap();p.error=Some("original injected device failure".into());p.playing=false;p.pending=false;
        assert!(app.poll_media_sync());assert!(!app.poll_media_sync());assert!(!app.poll_media_sync());
        assert!(app.media_report().contains("Shared PCM output: original injected device failure"));
        assert!(app.input_media_action("Pause").is_err());assert!(app.next_media_timer().is_none());assert!(app.audio_worker.is_none());
    }
}
impl Player {
    fn source_alive(&self) -> bool {self.source_retired.as_ref().is_none_or(|source|!source.load(Ordering::Relaxed))}
    fn event(&mut self, name: &'static str) {
        if self.events.len() == 128 {
            self.events.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.events.push_back(name);
    }
}
impl App {
    pub(super) fn audio_sync_state(&self)->Option<SyncState> {
        self.audio.as_ref().filter(|p|p.source_alive()).map(|p|SyncState {position:p.clip.time_at(p.frame),desired:p.clip.time_at(p.desired_frame.unwrap_or(p.frame)),pending:p.pending,playing:p.playing,wants_play:if p.pending {p.desired_playing}else{p.playing},drained:p.ended&&p.frame==p.clip.frames&&p.submitted>0&&p.retired==p.submitted&&p.queued==0,error:p.error.clone()})
    }
    pub(super) fn audio_sync_seek(&mut self,position:Duration) {
        if let Some(p)=&mut self.audio {p.suspended=false;let frame=p.clip.frame_at(position);self.submit_audio(Action::Seek {frame,resume:false});}
    }
    pub(super) fn install_audio(&mut self, clip: Option<Arc<Clip>>) {
        self.install_audio_for_source(clip, None);
    }
    pub(super) fn install_audio_for_source(&mut self, clip: Option<Arc<Clip>>, source_retired: Option<Arc<AtomicBool>>) {
        // A late retired candidate cannot tear down the current live owner.
        // None is an explicit teardown command and remains authoritative.
        if clip.is_some() && source_retired.as_ref().is_some_and(|source|source.load(Ordering::Relaxed)) {return;}
        if let Some(worker) = &self.audio_worker {
            worker.clear();
        }
        self.audio = None;
        let Some(clip) = clip else {
            return;
        };
        let mut player = Player {
            clip,
            source_retired,
            frame: 0,
            playing: false,
            ended: false,
            opened: false,
            pending: false,
            desired_playing: false,
            desired_frame: None,
            generation: 0,
            enabled: true,
            suspended: false,
            error: None,
            deadline: None,
            pressed: None,
            queued: 0,
            submitted: 0,
            retired: 0,
            unit: None,
            retained: 0,
            seeking: false,
            stopping: false,
            data_seen: false,
            ended_reported: false,
            events: VecDeque::new(),
            dropped: 0,
        };
        player.event("loadedmetadata");
        self.audio = Some(player);
    }
    fn retire_audio_source(&mut self) -> bool {
        if self.audio.as_ref().is_none_or(Player::source_alive) {return false;}
        // The binding is no longer observable, but the real native owner keeps
        // all headers/PCM until its existing safe reset/unprepare path completes.
        self.audio = None;
        if let Some(worker) = &self.audio_worker {worker.clear();}
        true
    }
    fn submit_audio(&mut self, action: Action) {
        if self.retire_audio_source() {return;}
        let Some(player) = &mut self.audio else {
            return;
        };
        if !player.enabled {
            return;
        }
        let action = match (action, player.desired_frame) {
            (Action::Play { .. }, Some(frame)) => Action::Seek {
                frame,
                resume: true,
            },
            (Action::Pause { .. }, Some(frame)) => Action::Seek {
                frame,
                resume: false,
            },
            (action, _) => action,
        };
        match action {
            Action::Seek { frame, .. } => {
                player.desired_frame = Some(frame.min(player.clip.frames))
            }
            Action::Stop => player.desired_frame = Some(0),
            _ => {}
        }
        let Some(generation) = player.generation.checked_add(1) else {
            player.enabled = false;
            player.error = Some("Audio request generation exhausted.".into());
            player.playing = false;
            player.pending = false;
            player.deadline = None;
            player.event("error");
            if let Some(worker) = &self.audio_worker {
                worker.clear();
            }
            return;
        };
        if self.audio_worker.is_none() {
            match Worker::new(self.wake.clone()) {
                Ok(worker) => self.audio_worker = Some(worker),
                Err(error) => {
                    player.error = Some(error);
                    player.event("error");
                    return;
                }
            }
        }
        player.generation = generation;
        player.pending = true;
        player.error = None;
        player.ended_reported = false;
        player.desired_playing = matches!(
            action,
            Action::Play { .. } | Action::Seek { resume: true, .. }
        );
        player.seeking = matches!(action, Action::Seek { .. });
        player.stopping = matches!(action, Action::Stop);
        if player.seeking {
            player.event("seeking");
        }
        player.deadline = (!player.suspended).then_some(Instant::now() + Duration::from_millis(20));
        let worker = self.audio_worker.as_ref().unwrap();
        if let Some(source) = &player.source_retired {
            worker.submit_for_source(player.clip.clone(), generation, action, Some(source.clone()));
        } else {
            worker.submit(player.clip.clone(), generation, action);
        }
    }
    pub(super) fn suspend_audio(&mut self) {
        if let Some(player) = &mut self.audio {
            player.suspended = true;
            player.pressed = None;
            let frame = player.frame;
            self.submit_audio(Action::Pause { from: frame });
        }
    }
    pub(super) fn stop_audio(&mut self) {
        if let Some(player) = &mut self.audio {
            player.suspended = false;
            self.submit_audio(Action::Stop);
        }
    }
    pub(super) fn activate_audio(&mut self, control: Control) {
        let Some(player) = &mut self.audio else {
            return;
        };
        player.suspended = false;
        match control {
            Control::Stop => self.submit_audio(Action::Stop),
            Control::Toggle => {
                let playing = if player.pending {
                    player.desired_playing
                } else {
                    player.playing
                };
                let from = player.desired_frame.unwrap_or(player.frame);
                if !playing && from >= player.clip.frames {
                    self.submit_audio(Action::Seek {
                        frame: 0,
                        resume: true,
                    });
                    return;
                }
                self.submit_audio(if playing {
                    Action::Pause { from }
                } else {
                    Action::Play { from }
                });
            }
            Control::Seek(x) => {
                let (start, width) = self.media_track();
                let player = self.audio.as_ref().unwrap();
                let fraction = f64::from((x - start).clamp(0, width)) / f64::from(width);
                let frame = player.clip.frame_at(player.clip.duration.mul_f64(fraction));
                let resume = if player.pending {
                    player.desired_playing
                } else {
                    player.playing
                };
                self.submit_audio(Action::Seek { frame, resume });
            }
        }
    }
    pub(super) fn audio_seek_relative(&mut self, forward: bool) {
        let Some(player) = &self.audio else {
            return;
        };
        let current = player
            .clip
            .time_at(player.desired_frame.unwrap_or(player.frame));
        let target = if forward {
            current.saturating_add(Duration::from_secs(5))
        } else {
            current.saturating_sub(Duration::from_secs(5))
        };
        let frame = player.clip.frame_at(target);
        let resume = if player.pending {
            player.desired_playing
        } else {
            player.playing
        };
        self.submit_audio(Action::Seek { frame, resume });
    }
    pub(super) fn audio_control_state(&self) -> Option<ControlState> {
        self.audio.as_ref().filter(|player|player.source_alive()).map(|player| ControlState {
            playing: if player.pending {
                player.desired_playing
            } else {
                player.playing
            },
            ended: player.ended
                || player.desired_frame.unwrap_or(player.frame) >= player.clip.frames,
            position: player.clip.time_at(player.frame),
            duration: player.clip.duration,
            error: player.error.is_some(),
            pending: player.pending,
        })
    }
    pub(super) fn press_audio(&mut self, control: Control) {
        if let Some(player) = &mut self.audio {
            player.pressed = Some(control);
        }
    }
    pub(super) fn take_audio_press(&mut self) -> Option<Control> {
        self.audio.as_mut().and_then(|player| player.pressed.take())
    }
    pub(super) fn cancel_audio_press(&mut self) {
        if let Some(player) = &mut self.audio {
            player.pressed = None;
        }
    }
    pub(super) fn next_audio_timer(&self) -> Option<Instant> {
        self.audio.as_ref().filter(|player|player.source_alive()).and_then(|player| player.deadline)
    }
    pub(super) fn poll_audio_timer_at(&mut self, now: Instant) -> bool {
        if self.retire_audio_source() {return true;}
        let Some(player) = &mut self.audio else {
            return false;
        };
        if !player.deadline.is_some_and(|due| due <= now) {
            return false;
        }
        player.deadline = (!player.suspended && (player.pending || player.playing))
            .then_some(now + Duration::from_millis(20));
        true
    }
    pub(super) fn poll_audio(&mut self) -> bool {
        if self.retire_audio_source() {return true;}
        let Some(worker) = &self.audio_worker else {
            return false;
        };
        let (snapshot, notice) = worker.poll();
        let mut changed = false;
        if let Some(notice) = notice {
            self.status = format!("Audio output error: {notice}");
            self.audio_notice = Some(notice);
            changed = true;
        }
        let Some(snapshot) = snapshot else {
            return changed;
        };
        self.accept_audio_snapshot(snapshot) || changed
    }
    fn accept_audio_snapshot(&mut self, snapshot: crate::audio_output::Snapshot) -> bool {
        let Some(player) = &mut self.audio else {
            return false;
        };
        if snapshot.generation != player.generation || !Arc::ptr_eq(&snapshot.clip, &player.clip)
            || !snapshot.matches_source(player.source_retired.as_ref()) {
            return false;
        }
        let (was_playing, was_opened) = (player.playing, player.opened);
        player.pending = false;
        player.frame = snapshot.frame;
        player.playing = snapshot.playing;
        player.ended = snapshot.ended;
        player.opened = snapshot.opened;
        player.queued = snapshot.queued;
        player.submitted = snapshot.submitted;
        player.retired = snapshot.retired;
        player.unit = snapshot.position_unit;
        player.retained = snapshot.retained_buffers;
        if player.error != snapshot.error && snapshot.error.is_some() {
            player.event("error");
        }
        player.error = snapshot.error;
        if player.error.is_none() {
            player.desired_frame = None;
            if !was_opened && player.opened {
                player.event("deviceopened");
            }
            if !player.data_seen && player.submitted > 0 {
                player.data_seen = true;
                player.event("loadeddata");
            }
            if !was_playing && player.playing {
                player.event("play");
            }
            if was_playing && !player.playing && !player.ended {
                player.event("pause");
            }
            if player.seeking {
                player.event("seeked");
            }
            if player.stopping {
                player.event("stop");
            }
            if player.ended && !player.ended_reported {
                player.ended_reported = true;
                player.event(if player.submitted > 0 {
                    "ended"
                } else {
                    "atend"
                });
            }
        }
        player.seeking = false;
        player.stopping = false;
        player.desired_playing = player.playing;
        player.deadline = (!player.suspended && (player.pending || player.playing))
            .then_some(Instant::now() + Duration::from_millis(20));
        true
    }
    pub(super) fn audio_report(&self) -> String {
        let (opened, retained, busy) = self
            .audio_worker
            .as_ref()
            .map_or((false, 0, false), Worker::ownership);
        let owner =
            format!("Audio owner opened={opened} retained_buffers={retained} busy={busy}\n");
        let notice = self.audio_notice.as_deref().unwrap_or("none");
        let Some(player) = self.audio.as_ref().filter(|player|player.source_alive()) else {
            return format!("Local audio: none\n{owner}Last audio output notice: {notice}\n");
        };
        let scope=if self.media.is_some(){"Matroska PCM16 track with shared video clock"}else{"standalone integer PCM WAVE"};
        format!("Local audio: {scope}, native viewer lifecycle (no HTMLMediaElement)\nAudio playing={} ended={} pending={} time_ms={} duration_ms={} frame={} frames={} generation={} error={:?}\nAudio device opened={} queued={} submitted={} retired={} position_unit={:?} retained_buffers={}\n{owner}Audio lifecycle dropped={} {}\nAudio clock: {}; device acceptance/completion does not prove physical audibility; A/V evidence is recorded separately.\nLast audio output notice: {notice}\n",player.playing,player.ended,player.pending,player.clip.time_at(player.frame).as_millis(),player.clip.duration.as_millis(),player.frame,player.clip.frames,player.generation,player.error,player.opened,player.queued,player.submitted,player.retired,player.unit,player.retained,player.dropped,player.events.iter().copied().collect::<Vec<_>>().join(","),if player.opened{"reported device position and returned PCM buffers"}else{"selected metadata sample offset; device not opened"})
    }
}
