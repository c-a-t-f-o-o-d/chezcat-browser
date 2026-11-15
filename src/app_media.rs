//! Standalone local video viewer. These are native viewer controls and lifecycle
//! diagnostics, not a claim of HTMLMediaElement, WebM, audio or YouTube support.
use super::{App, Canvas, Focus, View};
use crate::image::Bitmap;
use crate::media::{self, Clock};
use crate::media_tasks::{self, Shared as MediaTasks, Scope as MediaScope, Task as MediaTask};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool,Ordering};
use std::time::{Duration, Instant};
#[path="app_embedded_video.rs"]
mod embedded;

pub(super) struct Embedded {
    retired:Arc<AtomicBool>,
    node:usize,raw:String,source:String,plan:Option<Arc<crate::embedded_video::Plan>>,network:u8,
    error:Option<String>,aborted:bool,tasks:MediaTasks,last_update:Duration,
    script_play_intent:Option<Instant>,
}

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
    embedded:bool,
    controls_focused:bool,
    task_binding:Option<(MediaTasks,MediaScope)>,
    dom_playback_started:bool,
    paused:bool,
    // Authored seek readiness freezes the physical position while retaining
    // logical play intent; the requested bitmap must be published first.
    seek_resume:Option<bool>,
    initial_data_pending:bool,
    script_play_waiting:bool,
    sync:Option<Synchronized>,
    clip: Arc<media::Clip>,
    pub(super) node: usize,
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
struct Synchronized {position:Duration,playing:bool,drained:bool,resume_after_seek:bool}

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
        let node=app.media.as_ref().map(|p|p.node);app.install_media(Some(asset),node,false);
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

    fn av_app()->App {
        let mut app=App::new();app.scripts_enabled=false;app.open(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/fixtures/scene-av.mkv"));app.wait_for_load();
        assert!(!app.failed,"{}",app.status);assert!(app.synchronized_media());app
    }
    fn settle_av(app:&mut App) {
        let deadline=Instant::now()+Duration::from_secs(3);
        while app.media_control_state().unwrap().pending&&Instant::now()<deadline {app.poll_load();app.poll_page_timer();std::thread::sleep(Duration::from_millis(1));}
        assert!(!app.media_control_state().unwrap().pending,"{}{}",app.media_report(),app.audio_report());
    }
    #[test]
    fn matroska_app_paused_seek_decodes_without_device_and_metadata_eof_is_not_drain() {
        let mut app=av_app();assert!(app.audio_worker.is_none());
        app.synchronized_seek(Duration::from_millis(2200),false);settle_av(&mut app);
        assert_eq!(app.media.as_ref().unwrap().displayed,4);assert_eq!(app.media_control_state().unwrap().position,Duration::from_millis(2200));
        assert_eq!(app.audio_worker.as_ref().unwrap().ownership(),(false,0,false));
        assert!(!app.media.as_ref().unwrap().clock.playing());
        app.synchronized_seek(Duration::from_secs(3),false);settle_av(&mut app);
        assert_eq!(app.media.as_ref().unwrap().displayed,5);assert!(!app.media.as_ref().unwrap().ended_reported);
        // Independently acknowledge a hypothetical completed device drain to
        // isolate the controller's final-frame publication gate (no device).
        let p=app.media.as_mut().unwrap();p.sync.as_mut().unwrap().drained=true;p.displayed=4;
        app.finish_media_tick(Instant::now());assert!(!app.media.as_ref().unwrap().ended_reported);
        app.media.as_mut().unwrap().displayed=5;app.finish_media_tick(Instant::now());assert!(app.media.as_ref().unwrap().ended_reported);
        app.stop_media();settle_av(&mut app);assert_eq!(app.media.as_ref().unwrap().displayed,0);assert!(!app.media.as_ref().unwrap().ended_reported);
    }
    #[test]
    fn matroska_app_mock_device_stall_never_uses_wall_time_and_rapid_intents_preserve_frames() {
        let mut app=av_app();app.audio_worker=Some(crate::audio_output::Worker::mock_for_app(app.wake.clone()).unwrap());
        app.input_media_action("Play").unwrap();settle_av(&mut app);
        let p=app.media.as_ref().unwrap();assert!(p.playing());assert!(!p.clock.playing());
        let future=Instant::now()+Duration::from_secs(30);app.poll_media_timer_at(future);
        assert_eq!(app.media_control_state().unwrap().position,Duration::ZERO);assert_eq!(app.media.as_ref().unwrap().displayed,0);
        app.synchronized_seek(Duration::from_millis(2200),true);
        assert!(!app.audio_sync_state().unwrap().wants_play);
        let deadline=Instant::now()+Duration::from_secs(3);
        while app.media_control_state().unwrap().pending&&Instant::now()<deadline {
            app.poll_audio();let state=app.audio_sync_state().unwrap();
            if app.media.as_ref().unwrap().seeking {assert!(!state.wants_play);}
            app.poll_media();app.poll_media_sync();std::thread::sleep(Duration::from_millis(1));
        }
        settle_av(&mut app);assert!(app.media_control_state().unwrap().playing);assert_eq!(app.media.as_ref().unwrap().displayed,4);
        app.synchronized_seek(Duration::from_millis(700),true);app.synchronized_seek(Duration::from_millis(1200),true);app.stop_media();app.synchronized_activate(Control::Toggle);settle_av(&mut app);
        assert_eq!(app.media_control_state().unwrap().position,Duration::ZERO);assert_eq!(app.media.as_ref().unwrap().displayed,0);
        assert!(app.media_control_state().unwrap().playing);
        app.suspend_media();app.poll_media_timer_at(future);assert!(app.next_media_timer().is_none());assert!(app.media.as_ref().unwrap().pending.is_none());
    }
    #[test]
    fn matroska_app_navigation_replacement_and_small_profile_recover_without_device() {
        let mut app=av_app();app.synchronized_seek(Duration::from_millis(2200),false);
        app.open(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pages/second.html"));app.wait_for_load();
        std::thread::sleep(Duration::from_millis(15));app.poll_load();assert!(app.media.is_none()&&app.audio.is_none());assert!(app.next_media_timer().is_none());
        app.media_limits.frames=1;let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/fixtures/scene-av.mkv");app.open(&path);app.wait_for_load();assert!(app.failed&&!app.synchronized_media());
        app.media_limits=media::Limits::default();app.open(&path);app.wait_for_load();assert!(!app.failed&&app.synchronized_media());
        app.synchronized_seek(Duration::from_millis(1200),false);app.reload();app.wait_for_load();app.poll_load();assert_eq!(app.media.as_ref().unwrap().displayed,0);assert!(!app.media_control_state().unwrap().playing);
    }
    #[test]
    fn matroska_app_pending_pcm_gates_every_frame_route_and_generation_fault_pauses_master() {
        let mut app=av_app();app.audio_worker=Some(crate::audio_output::Worker::mock_for_app(app.wake.clone()).unwrap());
        app.synchronized_seek(Duration::from_millis(2200),false);settle_av(&mut app);
        // A renderer lagged the previous acknowledged sample position when
        // the user requested another seek. No old frame may enter its epoch.
        app.media.as_mut().unwrap().displayed=0;
        app.synchronized_seek(Duration::from_millis(700),false);
        assert!(app.audio_sync_state().unwrap().pending);
        app.media_request_frame(Instant::now());assert!(app.media.as_ref().unwrap().pending.is_none());
        let due=app.next_media_timer().unwrap();app.poll_media_timer_at(due);assert!(app.media.as_ref().unwrap().pending.is_none());
        settle_av(&mut app);assert_eq!(app.media.as_ref().unwrap().displayed,1);
        app.synchronized_activate(Control::Toggle);settle_av(&mut app);assert!(app.audio_sync_state().unwrap().playing);
        app.synchronized_seek(Duration::from_millis(900),true);assert!(app.media.as_ref().unwrap().seeking);
        app.media.as_mut().unwrap().generation=u64::MAX;
        app.synchronized_seek(Duration::from_millis(1200),true);
        assert!(app.media.as_ref().unwrap().error.as_deref().is_some_and(|e|e.contains("exhausted")));
        assert!(!app.media.as_ref().unwrap().decoder_enabled);
        assert!(!app.media.as_ref().unwrap().seeking);assert!(app.next_media_timer().is_none());
        let deadline=Instant::now()+Duration::from_secs(3);while app.audio_sync_state().unwrap().pending&&Instant::now()<deadline {app.poll_audio();std::thread::sleep(Duration::from_millis(1));}
        assert!(!app.audio_sync_state().unwrap().playing);assert!(!app.media.as_ref().unwrap().clock.playing());
    }
    #[test]
    fn matroska_app_real_video_decode_fault_clears_shared_resume_intent() {
        let mut app=av_app();app.audio_worker=Some(crate::audio_output::Worker::mock_for_app(app.wake.clone()).unwrap());
        let source=include_bytes!("../tools/fixtures/scene-intra.ivf");let mut bytes=source[..32].to_vec();let mut offset=32;let mut index=0;
        while offset<source.len(){let size=u32::from_le_bytes(source[offset..offset+4].try_into().unwrap()) as usize;let n=if index==2 {10}else{size};bytes.extend_from_slice(&(n as u32).to_le_bytes());bytes.extend_from_slice(&source[offset+4..offset+12]);bytes.extend_from_slice(&source[offset+12..offset+12+n]);offset+=12+size;index+=1;}
        let node=app.media.as_ref().map(|p|p.node);app.install_media(Some(media::Clip::load(bytes,media::Limits::default(),&||Ok(())).unwrap()),node,false);app.install_media_sync();
        app.synchronized_seek(Duration::from_millis(1200),true);
        let deadline=Instant::now()+Duration::from_secs(3);while app.media.as_ref().unwrap().error.is_none()&&Instant::now()<deadline {app.poll_load();std::thread::sleep(Duration::from_millis(1));}
        assert!(app.media.as_ref().unwrap().error.is_some());assert!(!app.media_control_state().unwrap().playing);assert!(!app.media.as_ref().unwrap().sync.as_ref().unwrap().resume_after_seek);
        assert!(!app.audio_sync_state().unwrap().wants_play);assert!(!app.poll_media_sync());
    }
    include!("app_interframe_av_tests.rs");
}
impl Embedded {
    fn task_scope(&self) -> MediaScope { MediaScope {node:self.node,retired:self.retired.clone()} }
    fn fail_tasks(&mut self, problem:String) {
        // App captures owned Promise outcomes before erasing this source FIFO.
        self.plan=None;self.network=3;self.aborted=true;self.script_play_intent=None;self.error=Some(problem);
    }
    fn enqueue_events(&mut self, events:impl IntoIterator<Item=&'static str>) -> bool {
        let scope=self.task_scope();
        let result=self.tasks.lock().map_err(|_|"Media task queue is unavailable.".to_owned()).and_then(|mut tasks| {
            for event in events {tasks.enqueue(scope.clone(),None,Some(MediaTask::Single(event)))?;}
            Ok(())
        });
        if let Err(problem)=result {self.fail_tasks(problem);false}else{true}
    }
}
impl Player {
    fn position(&self,now:Instant)->Duration {self.sync.as_ref().map_or_else(||self.clock.position(now),|s|s.position)}
    fn playing(&self)->bool {self.sync.as_ref().map_or_else(||self.clock.playing(),|s|s.playing)}
    fn drained(&self)->bool {self.sync.as_ref().map_or(self.clock.ended,|s|s.drained)}
    fn event(&mut self, event: &'static str) {
        if event=="play" {self.paused=false;}
        if matches!(event,"pause"|"stop"|"cancel"|"error"|"ended") {self.paused=true;}
        if matches!(event,"stop"|"cancel") {self.dom_playback_started=false;}
        if matches!(event,"pause"|"stop"|"cancel"|"error") {self.script_play_waiting=false;}
        if matches!(event,"stop"|"cancel"|"error") {self.seek_resume=None;}
        if event=="pause" && self.seek_resume.is_some() {self.seek_resume=Some(false);}

        if self.embedded && crate::scripting::media_event_name(event) && (self.decoder_enabled||event=="error") {
            if event=="ended" {
                if self.dom_playback_started {self.queue_dom_event("timeupdate");self.queue_dom_event("pause");self.queue_dom_event("ended");self.dom_playback_started=false;}
            }else{
                self.queue_dom_event(event);
                if event=="play"{self.dom_playback_started=self.decoder_enabled&&self.clock.playing();}
                if matches!(event,"pause"|"error"){self.dom_playback_started=false;}
            }
        }
        self.record_only(event);
    }
    fn record_only(&mut self,event:&'static str) {
        if self.events.len() == 128 {
            self.events.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.events.push_back(event);
    }
    fn fail_task_queue(&mut self, problem:String) {
        // Queue erasure is deferred to App's failure path, after Page has
        // captured pending and already-batched Promise outcomes.
        self.script_play_waiting=false;
        self.clock.pause(Instant::now());self.paused=true;self.dom_playback_started=false;
        self.decoder_enabled=false;self.pending=None;self.deadline=None;self.seeking=false;self.seek_resume=None;
        self.suspended=true;self.error=Some(problem);
    }
    fn queue_dom_event(&mut self,event:&'static str){
        let Some((tasks,scope))=&self.task_binding else {return;};
        // Commit at the actual native event time. No separate Player event lane.
        let result=tasks.lock().map_err(|_|"Media task queue is unavailable.".to_owned())
            .and_then(|mut tasks|tasks.enqueue(scope.clone(),None,Some(MediaTask::Single(event))).map(|_|()));
        if let Err(problem)=result {self.fail_task_queue(problem);}
    }
    fn publish_initial_data(&mut self) {
        if self.initial_data_pending {
            self.initial_data_pending=false;
            self.event("loadeddata");self.event("canplay");self.event("canplaythrough");
        }
    }
    fn complete_seek(&mut self, now:Instant) {
        self.seeking=false;
        if self.embedded {
            let resume=self.seek_resume.take().unwrap_or(false);
            if resume && !self.paused && self.clock.position(now)<self.clip.duration {
                self.clock.play(now);
                if !self.dom_playback_started {
                    self.dom_playback_started=true;
                    if self.script_play_waiting {self.record_only("playing");}
                    else {self.event("playing");}
                }
            }
            self.event("timeupdate");
        }
        self.event("seeked");
    }

    fn invalidate(&mut self) {
        self.seek_resume=None;
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
    pub(super) fn install_media_sync(&mut self) {
        if self.audio.is_some() {if let Some(p)=&mut self.media {p.sync=Some(Synchronized {position:Duration::ZERO,playing:false,drained:false,resume_after_seek:false});}}
    }
    fn synchronized_media(&self)->bool {self.media.as_ref().is_some_and(|p|p.sync.is_some())}
    /// Only an acknowledged PCM sample position chooses video presentation.
    /// The video Instant clock is never started in this mode.
    pub(super) fn poll_media_sync(&mut self)->bool {
        let controls=self.apply_media_pause();
        if !self.synchronized_media()||self.embedded_video_retired(){return controls;}
        let Some(audio)=self.audio_sync_state() else{return controls;};
        let player=self.media.as_mut().unwrap();
        if player.suspended{return controls;}
        if let Some(error)=audio.error {
            let error=format!("Shared PCM output: {error}");
            let changed=player.error.as_ref()!=Some(&error);
            if changed {player.error=Some(error);player.invalidate();player.event("error");if let Some(w)=&self.media_worker{w.cancel();}}
            player.seeking=false;let sync=player.sync.as_mut().unwrap();sync.playing=false;sync.resume_after_seek=false;return changed|controls;
        }
        if player.error.is_some() {
            let sync=player.sync.as_mut().unwrap();sync.playing=false;sync.resume_after_seek=false;
            if audio.wants_play {self.audio_sync_pause();return true;}
            return controls;
        }
        if audio.pending{return controls;}
        let sync=player.sync.as_mut().unwrap();
        let mut changed=controls||sync.position!=audio.position||sync.playing!=audio.playing||sync.drained!=audio.drained;
        sync.position=audio.position;sync.playing=audio.playing;sync.drained=audio.drained;
        // The control pass above owns this boundary. No nested drain may pair
        // this acknowledged sample with a later control's seek generation.
        self.media_request_frame_physical(Instant::now());
        let player=self.media.as_mut().unwrap();
        let mut resume=false;
        if player.seeking&&player.pending.is_none()&&player.displayed==player.clip.index_at(audio.position)&&player.error.is_none() {
            player.seeking=false;
            if player.embedded {player.event("timeupdate");}
            player.event("seeked");changed=true;
            let intent=std::mem::take(&mut player.sync.as_mut().unwrap().resume_after_seek);
            // EOF selection is not a request to replay from zero. Notification
            // failure also cannot start the PCM sink after the player stopped.
            resume=intent&&(!player.embedded||!player.paused)&&player.error.is_none()
                &&player.decoder_enabled&&audio.position<player.clip.duration;
        }
        if player.embedded&&!player.paused&&!player.seeking&&player.error.is_none()
            &&audio.playing&&player.displayed==player.clip.index_at(audio.position)&&!player.dom_playback_started {
            player.dom_playback_started=true;
            if player.script_play_waiting {player.record_only("playing");}else{player.event("playing");}
            changed=true;
        }
        if resume&&!self.audio_sync_play() {self.fail_media_sync_command("Play after seek");changed=true;}
        self.finish_media_tick(Instant::now());changed
    }
    fn fail_media_sync_command(&mut self,operation:&str) {
        let problem=self.audio_sync_state().and_then(|s|s.error)
            .unwrap_or_else(||format!("Shared PCM {operation} command was not admitted."));
        if let Some(worker)=&self.media_worker {worker.cancel();}
        if let Some(player)=self.media.as_mut().filter(|p|p.sync.is_some()) {
            player.invalidate();player.error=Some(problem);player.seeking=false;
            let sync=player.sync.as_mut().unwrap();sync.playing=false;sync.resume_after_seek=false;
            player.event("error");
        }
        // The existing native cleanup retains owned device buffers until a
        // genuine Pause/reset succeeds; no output-close or progress is guessed.
        self.suspend_audio();
    }
    fn synchronized_seek(&mut self,position:Duration,resume:bool) {
        if let Some(w)=&self.media_worker{w.cancel();}
        let Some(p)=&mut self.media else{return;};
        p.invalidate();
        if !p.decoder_enabled {p.seeking=false;if let Some(s)=&mut p.sync{s.playing=false;s.resume_after_seek=false;}p.event("error");self.suspend_audio();return;}
        p.suspended=false;p.error=None;p.ended_reported=false;
        p.seeking=true;p.event("seeking");
        if !p.decoder_enabled||p.error.is_some() {p.seeking=false;self.suspend_audio();return;}
        let sync=p.sync.as_mut().unwrap();sync.resume_after_seek=resume;sync.drained=false;sync.playing=false;
        self.audio_sync_seek(position);
        self.finish_media_tick(Instant::now());
    }
    fn synchronized_activate(&mut self,control:Control) {
        let Some(audio)=self.audio_sync_state()else{return;};
        match control {
            Control::Stop=>self.stop_media(),
            Control::Seek(x)=>{
                let(start,width)=self.media_track();let p=self.media.as_ref().unwrap();
                let resume=if p.embedded {!p.paused}else{audio.wants_play||p.sync.as_ref().unwrap().resume_after_seek};
                let position=p.clip.duration.mul_f64(f64::from((x-start).clamp(0,width))/f64::from(width));
                self.synchronized_seek(position,resume);
            }
            Control::Toggle=>{
                if self.media.as_ref().is_some_and(|p|p.embedded) {
                    let p=self.media.as_ref().unwrap();let pause=!p.paused;let position=p.position(Instant::now());
                    if pause {
                        self.script_media_pause_physical(Instant::now(),position);
                        if let Some(p)=&mut self.media {p.event("pause");}
                    } else {
                        if let Some(p)=&mut self.media {p.event("play");}
                        self.script_media_play_physical(Instant::now(),false);
                    }
                    return;
                }
                let p=self.media.as_mut().unwrap();p.suspended=false;
                if p.seeking {let s=p.sync.as_mut().unwrap();s.resume_after_seek=!s.resume_after_seek;return;}
                if !audio.wants_play&&(audio.desired>=p.clip.duration||p.error.is_some()) {let position=if audio.desired>=p.clip.duration{Duration::ZERO}else{audio.desired};self.synchronized_seek(position,true);return;}
                if audio.wants_play {if let Some(w)=&self.media_worker{w.cancel();}p.invalidate();}
                let accepted=if audio.wants_play {self.audio_sync_pause()}else{self.audio_sync_play()};
                if !accepted {self.fail_media_sync_command("native playback");}
            }
        }
    }
    pub(super) fn install_media(&mut self, asset: Option<media::Asset>,node:Option<usize>,embedded:bool) {
        self.install_media_with_readiness(asset,node,embedded,false);
    }
    pub(super) fn install_media_with_readiness(&mut self, asset: Option<media::Asset>,node:Option<usize>,embedded:bool,defer_data:bool) {
        if let Some(worker) = &self.media_worker {
            worker.cancel();
        }
        self.media = None;
        let Some(asset) = asset else {
            return;
        };
        let Some(node)=node.filter(|&node|self.document.nodes.get(node).is_some_and(|item|item.namespace==crate::html::Namespace::Html&&matches!(&item.kind,crate::html::NodeKind::Element {tag,..} if tag==if embedded{"video"}else{"img"}))) else{return;};
        self.images.insert(node, asset.first.clone());
        let mut player = Player {
            embedded,controls_focused:false,
            task_binding:self.embedded_video.as_ref().filter(|v|embedded&&v.node==node)
                .map(|v|(v.tasks.clone(),v.task_scope())),
            dom_playback_started:false,paused:true,seek_resume:None,initial_data_pending:embedded&&defer_data,script_play_waiting:false,
            sync:None,
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
        if embedded {player.event("durationchange");}
        player.event("loadedmetadata");
        if !defer_data {
            player.event("loadeddata");
            if embedded {player.event("canplay");player.event("canplaythrough");}
        }
        self.media = Some(player);
    }
    // Physical execution of the source-scoped JS Play command. Page owns the
    // play/waiting/PlayingBatch notifications and their Promise identities.
    pub(in crate::app) fn script_media_play_physical(&mut self,issued:Instant,notify_queued:bool)->bool {
        let now=Instant::now();
        if self.media.as_ref().is_some_and(|p|p.embedded&&p.sync.is_some()) {
            let target={
                let p=self.media.as_mut().unwrap();
                if !p.decoder_enabled||p.error.is_some() {p.script_play_waiting=false;return false;}
                p.script_play_waiting=true;p.paused=false;p.suspended=false;p.record_only("play");
                if p.seeking {p.sync.as_mut().unwrap().resume_after_seek=true;return true;}
                if p.position(now)>=p.clip.duration||p.ended_reported {Some(Duration::ZERO)}
                else if p.initial_data_pending {Some(p.position(now))}else{None}
            };
            if let Some(target)=target {self.synchronized_seek(target,true);return true;}
            if !self.audio_sync_play() {self.fail_media_sync_command("Play");return false;}
            return true;
        }
        let rewind={
            let Some(player)=self.media.as_mut().filter(|p|p.embedded&&p.sync.is_none()) else {return false;};
            if !player.decoder_enabled||player.error.is_some() {player.script_play_waiting=false;return false;}
            player.script_play_waiting=true;player.paused=false;player.suspended=false;
            player.record_only("play");
            // Preserve an already-requested/default bitmap. No elapsed time is
            // accrued during its decode; complete_seek resumes at actual ready.
            if player.seeking||player.initial_data_pending {
                player.clock.pause(now);player.seek_resume=Some(true);return true;
            }
            player.position(now)>=player.clip.duration||player.ended_reported
        };
        if rewind {
            // Rewind through the existing real seek/cached-poster publication,
            // before either starting a clock or advertising ready playback.
            self.media_seek_physical(Duration::ZERO,now);
            self.media_request_frame_physical(now);
        }
        let Some(player)=self.media.as_mut().filter(|p|p.embedded&&p.sync.is_none()) else {return false;};
        if !player.decoder_enabled||player.error.is_some() {player.script_play_waiting=false;return false;}
        if player.seeking||player.initial_data_pending {
            player.seek_resume=Some(true);return true;
        }
        let ready=Instant::now();
        if rewind&&notify_queued {
            player.clock.pause(ready);player.clock.seek(Duration::ZERO,ready);
        }
        player.ended_reported=false;
        player.clock.play(if notify_queued {issued}else{ready});
        if !player.dom_playback_started {player.dom_playback_started=true;player.record_only("playing");}
        true
    }
    // Page already committed the official-position projection and owns its
    // grouped notifications. PCM Pause reports its real later acknowledgement;
    // it never seeks the device back to the earlier JS-observed sample.
    pub(in crate::app) fn script_media_pause_physical(&mut self,issued:Instant,position:Duration)->bool {
        let Some(player)=self.media.as_mut().filter(|p|p.embedded) else {return false;};
        player.paused=true;player.dom_playback_started=false;
        player.script_play_waiting=false;player.record_only("pause");
        if let Some(sync)=&mut player.sync {
            sync.resume_after_seek=false;
            if !self.audio_sync_pause() {self.fail_media_sync_command("Pause");return false;}
            return true;
        }
        player.clock.pause(issued);
        if player.seek_resume.is_some() {player.seek_resume=Some(false);}
        debug_assert_eq!(player.clock.position(issued),position);
        if player.pending.is_none() {player.deadline=None;}
        true
    }
    pub(super) fn suspend_media(&mut self) {
        self.cancel_embedded_video();
        self.suspend_audio();
        if let Some(worker) = &self.media_worker {
            worker.cancel();
        }
        if let Some(player) = &mut self.media {
            let was_playing=player.playing();
            player.clock.pause(Instant::now());
            if was_playing && player.embedded {player.event("pause");}
            player.invalidate();
            player.suspended = true;
            if let Some(s)=&mut player.sync{s.playing=false;s.resume_after_seek=false;}
            player.seeking = false;
            player.pressed = None;
            player.event("cancel");
        }
        self.flush_embedded_video();
    }
    fn media_request_frame(&mut self, now: Instant) {
        self.apply_media_pause();
        self.media_request_frame_physical(now);
    }
    // Controller draining calls this only after every admitted control is set.
    // It never recursively drains a later control during an earlier seek.
    pub(in crate::app) fn media_request_frame_physical(&mut self, now: Instant) {
        if self.embedded_video_retired(){return;}
        // Every request route (timer, completion and controller) observes the
        // PCM acknowledgement gate. Old cached time cannot enter a new seek.
        if self.synchronized_media() {
            let Some(audio)=self.audio_sync_state() else{return;};
            if audio.pending||audio.error.is_some(){return;}
            let s=self.media.as_mut().unwrap().sync.as_mut().unwrap();
            s.position=audio.position;s.playing=audio.playing;s.drained=audio.drained;
        }
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
        if player.playing() && player.deadline.is_none() {
            player.deadline = Some(now + Duration::from_millis(20));
        }
        let index = player.clip.index_at(player.position(now));
        if index == player.displayed {
            player.publish_initial_data();
            if player.seeking&&player.sync.is_none() {
                player.complete_seek(now);
            }
            return;
        }
        if index == player.initial {
            self.images.insert(player.node, player.poster.clone());
            player.displayed = index;
            player.publish_initial_data();
            if player.seeking&&player.sync.is_none() {
                player.complete_seek(now);
            }
            return;
        }
        if self.media_worker.is_none() {
            match media::Worker::new(self.wake.clone()) {
                Ok(worker) => self.media_worker = Some(worker),
                Err(error) => {
                    player.clock.pause(now);
                    if let Some(s)=&mut player.sync{s.playing=false;s.resume_after_seek=false;}
                    player.seeking=false;
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
            .submit_for_source(player.clip.clone(), player.generation, index,self.embedded_video.as_ref().filter(|_|player.embedded).map(|v|v.retired.clone()));
    }
    pub(super) fn poll_media(&mut self) -> bool {
        let paused = self.apply_media_pause();
        if self.embedded_video_retired(){return paused;}
        let Some(completion) = self.media_worker.as_ref().and_then(|worker| worker.take()) else {
            return paused;
        };
        self.accept_media_frame(completion) | paused
    }
    // Real worker completions share one acceptance path. Source/generation/index
    // checks precede any publication; tests can retain a genuine decoded result
    // while exercising a committed control without a timing race.
    fn accept_media_frame(&mut self, completion: media::Completion) -> bool {
        self.apply_media_pause();
        if self.embedded_video_retired(){return false;}
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
                player.publish_initial_data();
                if player.seeking&&player.sync.is_none() {
                    player.complete_seek(Instant::now());
                }
            }
            Err(error) => {
                player.clock.pause(Instant::now());
                if let Some(s)=&mut player.sync{s.playing=false;s.resume_after_seek=false;}
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
        if self.embedded_video_retired(){return None;}
        self.media.as_ref().and_then(|player| player.deadline)
    }
    pub(super) fn poll_media_timer_at(&mut self, now: Instant) -> bool {
        let paused = self.apply_media_pause();
        if !self.next_media_timer().is_some_and(|due| due <= now) {
            return paused;
        }
        self.poll_media_sync();
        if let Some(player) = &mut self.media {
            if player.sync.is_none(){player.clock.tick(now);}
        }
        self.media_request_frame(now);
        self.finish_media_tick(now);
        self.flush_embedded_video();
        true
    }
    pub(in crate::app) fn finish_media_tick(&mut self, now: Instant) {
        if let Some(player) = &mut self.media {
            if !player.suspended
                && player.drained()
                && !player.seeking
                && player.error.is_none()
                && player.pending.is_none()
                && player.displayed == player.clip.frame_count() - 1
                && !player.ended_reported
            {
                player.ended_reported = true;
                player.event("ended");
            }
            player.deadline = (!player.suspended
                && (player.playing() || player.pending.is_some()||player.sync.is_some()&&player.seeking))
            .then_some(now + Duration::from_millis(20));
        }
    }
    fn media_seek(&mut self, position: Duration, now: Instant) {
        self.apply_media_pause();
        self.media_seek_physical(position,now);
        self.media_request_frame_physical(now);
        self.finish_media_tick(now);
    }
    pub(in crate::app) fn media_seek_physical(&mut self, position: Duration, now: Instant) {
        if self.synchronized_media() {
            let resume=self.media.as_ref().is_some_and(|p|!p.paused);
            self.synchronized_seek(position,resume);
            return;
        }
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
            if player.embedded {
                let resume=!player.paused;
                player.clock.pause(now);
                player.clock.seek(position,now);
                player.seek_resume=Some(resume);
            } else {player.clock.seek(position, now);}
            player.seeking = true;
            player.event("seeking");
        }
    }
    pub(super) fn stop_media(&mut self) {
        self.cancel_embedded_video();
        self.stop_audio();
        if let Some(worker) = &self.media_worker {
            worker.cancel();
        }
        if let Some(player) = &mut self.media {
            let was_playing=player.playing();let moved=player.position(Instant::now())!=Duration::ZERO;
            player.clock.pause(Instant::now());
            player.clock.seek(Duration::ZERO, Instant::now());
            if let Some(s)=&mut player.sync {s.position=Duration::ZERO;s.playing=false;s.drained=false;s.resume_after_seek=false;}
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
            if was_playing && player.embedded {player.event("pause");}
            if moved && player.embedded {player.event("seeking");player.event("timeupdate");player.event("seeked");}
        }
        self.flush_embedded_video();
    }
    // Both clock kinds use Page's original source-bound Pause task/Promise
    // capture. true means this native action was handled, including a refusal.
    fn capture_authored_native_pause(&mut self)->bool {
        if !self.media.as_ref().is_some_and(|p|p.embedded)||!self.scripts_enabled
            ||!self.document.scripting_enabled
            ||!self.scripting.as_ref().is_some_and(|page|page.native_media_pause_enabled()) {return false;}
        let snapshot=self.embedded_video_snapshot();
        let mut captured_or_aborted=false;
        let fault=if let Some(page)=&mut self.scripting {
            page.set_embedded_video(snapshot);
            captured_or_aborted=page.pending_media_play_count()==0;
            page.take_media_play_fault()
        } else {None};
        if captured_or_aborted {if let Some(player)=&mut self.media {player.script_play_waiting=false;}}
        if let Some(problem)=fault {self.fail_media_queue(problem);return true;}
        let result=self.scripting.as_mut().unwrap()
            .request_native_media_pause(&mut self.document,&mut self.forms);
        if let Err(problem)=result {self.fail_media_queue(problem);return true;}
        self.apply_media_pause();self.flush_embedded_video();true
    }
    fn media_activate(&mut self, control: Control) {
        // Earlier script controls take effect before the native default. Their
        // notifications remain in the same FIFO ahead of this native action.
        self.apply_media_pause();
        if self.synchronized_media(){
            if matches!(control,Control::Toggle)&&self.media.as_ref().is_some_and(|p|p.embedded&&!p.paused)
                &&self.capture_authored_native_pause() {return;}
            self.synchronized_activate(control);return;
        }
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
                    .is_some_and(|player| if player.embedded {!player.paused} else {player.clock.playing()})
                {
                    if self.capture_authored_native_pause() {return;}
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
                    if player.embedded && player.seeking && player.seek_resume.is_some() {
                        // Native Play changes intent while the requested frame
                        // remains pending; actual playing waits for publication.
                        player.seek_resume=Some(true);
                        player.event("play");
                    } else {
                        player.clock.play(now);
                        player.event("play");
                        if player.embedded {player.event("playing");}
                    }
                }
                self.media_request_frame(now);
                self.finish_media_tick(now);
            }
        }
    }
    fn media_visible(&self) -> bool {
        (self.media.is_some() || self.audio.is_some())
            && self.media_control_rect().is_some()
            && self.view == View::Page
            && !self.menu
            && self.browser_panel.is_none()
            && !self.downloads_open()
    }
    fn media_control_state(&self) -> Option<ControlState> {
        if self.synchronized_media() {
            let p=self.media.as_ref().unwrap();let s=p.sync.as_ref().unwrap();let audio=self.audio_sync_state()?;
            return Some(ControlState {playing:audio.wants_play||s.resume_after_seek,ended:p.ended_reported,position:s.position,duration:p.clip.duration,error:p.error.is_some()||audio.error.is_some(),pending:audio.pending||p.pending.is_some()||p.seeking});
        }
        if let Some(state) = self.audio_control_state() {
            return Some(state);
        }
        self.media.as_ref().map(|player| ControlState {
            playing: if player.embedded {!player.paused} else {player.clock.playing()},
            ended: player.ended_reported,
            position: player.clock.position(Instant::now()),
            duration: player.clip.duration,
            error: player.error.is_some(),
            pending: player.pending.is_some(),
        })
    }
    pub(super) fn media_track(&self) -> (i32, i32) {
        self.media_control_rect().map_or((170,(self.width-194).max(1)),|r|(r.left+170,(r.right-r.left-194).max(1)))
    }
    pub(super) fn media_hit(&self, x: i32, y: i32) -> Option<Control> {
        if !self.media_visible(){return None;}
        let rect=self.media_control_rect()?;
        if !rect.contains(x,y) || y<self.content_top() || y>=self.content_bottom() || x<0 || x>=self.width {return None;}
        if self.media.as_ref().is_some_and(|p|p.embedded&&!self.page.image_topmost_at(p.node,x,y-self.content_top()+self.scroll)){return None;}
        if (rect.left+8..rect.left+84).contains(&x) {
            Some(Control::Toggle)
        } else if (rect.left+90..rect.left+160).contains(&x) {
            Some(Control::Stop)
        } else {
            let (start, width) = self.media_track();
            (x >= start && x <= start + width).then_some(Control::Seek(x))
        }
    }
    pub(super) fn media_pointer_down(&mut self, x: i32, y: i32) -> bool {
        let Some(control) = self.media_hit(x, y) else {
            if let Some(p)=&mut self.media {p.controls_focused=false;}
            return false;
        };
        if let Some(player) = &mut self.media {
            player.pressed = Some(control);
            player.controls_focused=true;
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
                self.flush_embedded_video();
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
        self.apply_media_pause();
        if self.media.as_ref().is_some_and(|p|p.embedded&&!p.controls_focused){return false;}
        if self.focus != Focus::Page || !self.media_visible() {
            return false;
        }
        match key {
            0x20 => self.media_activate(Control::Toggle),
            0x25 | 0x27 => {
                if self.synchronized_media(){let audio=self.audio_sync_state().unwrap();let p=self.media.as_ref().unwrap();let resume=audio.wants_play||p.sync.as_ref().unwrap().resume_after_seek;let target=if key==0x25 {audio.desired.saturating_sub(Duration::from_secs(5))}else{audio.desired.saturating_add(Duration::from_secs(5))};self.synchronized_seek(target,resume);return true;}
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
        self.flush_embedded_video();true
    }
    pub(super) fn draw_media_controls(&self,canvas:&mut Canvas){
        if !self.media_visible() || self.media.as_ref().is_some_and(|p|p.embedded){return;}
        if let Some(rect)=self.media_control_rect(){self.draw_media_bar(canvas,rect);}
    }
    fn draw_media_bar(&self,canvas:&mut Canvas,rect:crate::paint::ClipRect) {
        let state = self.media_control_state().unwrap();
        let y=rect.top;let left=rect.left;let bar_width=rect.right-rect.left;
        canvas.fill_rect(left, y, bar_width, 44, 0xff162d3d);
        canvas.fill_rect(left+8, y + 5, 76, 30, 0xff346277);
        canvas.fill_rect(left+90, y + 5, 70, 30, 0xff346277);
        canvas.text(
            left+16,
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
        canvas.text(left+100, y + 15, "Stop", 1, 0xffffffff);
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
        let element=self.embedded_video_snapshot().map_or_else(String::new,|s|format!("HTML video binding node={} ready_state={} network_state={} width={} height={} paused={} ended={} seeking={} current_ms={} duration_ms={:?} source_pending={} error={:?}\n",s.node,s.ready,s.network,s.width,s.height,s.paused,s.ended,s.seeking,s.position().as_millis(),s.duration.map(|d|d.as_millis()),self.embedded_video_loading(),self.embedded_video.as_ref().and_then(|v|v.error.as_deref())));
        let Some(player) = &self.media else {
            return element+"Local video: none\n";
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
        let scope=if player.embedded&&player.sync.is_some(){"authored local Matroska VP8/PCM16, acknowledged sample clock, script load/play/pause/currentTime and native controls; no streaming/MSE"}else if player.embedded{"authored HTML video, IVF/VP8, script load/play/pause/currentTime and native controls; video only, no MSE"}else if player.sync.is_some(){"local Matroska VP8/PCM16, device-sample clock; native viewer events (no HTMLMediaElement)"}else{"standalone IVF/VP8, video only; native viewer events (no HTMLMediaElement)"};
        element+&format!("Local video: {scope}\nMedia playing={} ended={} time_ms={} duration_ms={} frame={} frames={} pending={:?} canvas={}x{} generation={} error={:?}\nMedia timeline_eof={}\nMedia lifecycle dropped={} {}\nMedia rendered rect={rect:?}\n",player.playing(),player.ended_reported,player.position(Instant::now()).as_millis(),player.clip.duration.as_millis(),player.displayed,player.clip.frame_count(),player.pending,player.clip.width,player.clip.height,player.generation,player.error,player.position(Instant::now())>=player.clip.duration,player.dropped,player.events.iter().copied().collect::<Vec<_>>().join(","))
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
                self.media_control_rect().ok_or("Media controls have no element box.")?.left+40
            }
            "Stop" => self.media_control_rect().ok_or("Media controls have no element box.")?.left+120,
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
        let y = self.media_control_rect().ok_or("Media controls have no element box.")?.top+16;
        if self.media_hit(x, y).is_none() {
            return Err("Media control is not a visible hit target.".into());
        }
        self.pointer_down(x, y, false);
        self.pointer_up(x, y);
        Ok((x, y))
    }
}
