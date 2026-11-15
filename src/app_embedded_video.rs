//! Document-owned source/element lifecycle. All callbacks execute on the UI thread.
use super::*;
use crate::embedded_video::{Completion, Plan, Snapshot};
impl App {
    /// Capture this document's ordinary resource authority, independently of
    /// classic-script credential exceptions or mutable UI/worker state.
    fn attach_embedded_network_context(&self, plan: Plan, policy_scope: usize) -> Result<Plan, String> {
        if plan.source.url.is_none() {
            return Ok(plan);
        }
        let origin = self.location.url.clone()
            .ok_or("A local document cannot grant network video authority.")?;
        let retired = plan.retired.clone();
        let credentials = plan.cors_mode;
        plan.with_network_context(crate::network_media::Context {
            origin,
            credentials,
            retired,
            trust: self.trust.clone(),
            cookies: self.cookies.clone(),
            blocking: Arc::new(crate::blocking::Policy::new(&self.profile.data().settings.blocking)),
            policy: self.document_policy.clone(),
            policy_scope,
            network: self.network.clone(),
        })
    }
    pub(in crate::app) fn embedded_video_retired(&self) -> bool {
        self.embedded_video
            .as_ref()
            .is_some_and(|v| v.retired.load(Ordering::Relaxed))
    }
    pub(in crate::app) fn process_media_selection(&mut self) -> bool {
        let Some(selection) = self
            .scripting
            .as_mut()
            .and_then(|p| p.take_media_selection())
        else {
            return false;
        };
        if let Some(worker) = &self.embedded_video_worker {
            worker.cancel();
        }
        if let Some(worker) = &self.media_worker {
            worker.cancel();
        }
        if let Some(player) = self.media.take() {
            self.images.remove(&player.node);
        }
        let previous = self.embedded_video.take();
        let tasks=previous.as_ref().map(|v|v.tasks.clone())
            .or_else(||self.scripting.as_ref().and_then(|page|page.media_tasks()))
            .unwrap_or_else(||media_tasks::shared(crate::javascript::limits::BROWSER.host.media_tasks));
        let mut binding = Embedded {
            node: selection.node,
            raw: selection.raw.clone().unwrap_or_default(),
            source: selection.uri.clone().unwrap_or_else(|| {
                previous
                    .as_ref()
                    .map_or_else(String::new, |v| v.source.clone())
            }),
            retired: selection.retired.clone(),
            plan: None,
            network: 0,
            error: None,
            aborted: false,
            tasks,
            last_update: Duration::ZERO,
            script_play_intent:None,
        };
        let selecting=selection.raw.is_some() || selection.error.is_some();
        if !binding.enqueue_events([
            selection.abort.then_some("abort"),selection.emptied.then_some("emptied"),
            selection.timeupdate.then_some("timeupdate"),selecting.then_some("loadstart")
        ].into_iter().flatten()) {
            self.events.push(format!("VIDEO node {}: {}",binding.node,binding.error.as_deref().unwrap_or("Media task queue failed.")));
            self.embedded_video=Some(binding);self.reflow();return true;
        }
        if selecting {
            binding.network = 2;
            let result = selection
                .error
                .map_or_else(
                    || {
                        Plan::selected(
                            selection.node,
                            &binding.raw,
                            selection.retired,
                            &self.document,
                            &self.location,
                            &self.base_location,
                            &self.document_policy,
                            self.media_limits,
                            usize::MAX,
                        )
                    },
                    Err,
                )
                .and_then(|plan| {
                    let plan = self.attach_embedded_network_context(plan, usize::MAX)?;
                    if self.embedded_video_worker.is_none() {
                        self.embedded_video_worker =
                            Some(crate::embedded_video::Worker::new(self.wake.clone())?);
                    }
                    let plan = Arc::new(plan);
                    self.embedded_video_worker
                        .as_ref()
                        .unwrap()
                        .submit(plan.clone());
                    binding.plan = Some(plan);
                    Ok(())
                });
            if let Err(error) = result {
                self.events
                    .push(format!("VIDEO node {}: {error}", binding.node));
                binding.script_play_intent=None;
                binding.error = Some(error);
                binding.network = 3;
                binding.enqueue_events(["error"]);
            }
        }
        self.embedded_video = Some(binding);
        self.reflow();
        true
    }
    pub(in crate::app) fn media_control_rect(&self) -> Option<crate::paint::ClipRect> {
        if self.embedded_video_retired() {
            return None;
        }
        if let Some(p) = self.media.as_ref().filter(|p| p.embedded) {
            if self.document.attr(p.node, "controls").is_none() {
                return None;
            }
            self.page.items.iter().find_map(|item| {
                if let crate::layout::PaintItem::Image {
                    node,
                    x,
                    y,
                    width,
                    height,
                    ..
                } = item
                {
                    (*node == p.node && *width >= 200 && *height >= 44).then(|| {
                        crate::paint::ClipRect::new(
                            *x,
                            y + self.content_top() - self.scroll + height - 44,
                            *width,
                            44,
                        )
                    })
                } else {
                    None
                }
            })
        } else {
            (self.content_bottom() - 44 >= self.content_top())
                .then(|| crate::paint::ClipRect::new(0, self.content_bottom() - 44, self.width, 44))
        }
    }
    pub(in crate::app) fn draw_embedded_video_controls(
        &self,
        canvas: &mut Canvas,
        node: usize,
        rect: crate::paint::ClipRect,
    ) {
        if !self
            .media
            .as_ref()
            .is_some_and(|p| p.embedded && p.node == node)
            || self.embedded_video_retired()
            || self.document.attr(node, "controls").is_none()
            || !self.media_visible()
            || rect.right - rect.left < 200
            || rect.bottom - rect.top < 44
        {
            return;
        }
        let clip = canvas.clip();
        canvas.set_clip(Some(clip.intersect(rect)));
        self.draw_media_bar(
            canvas,
            crate::paint::ClipRect::new(rect.left, rect.bottom - 44, rect.right - rect.left, 44),
        );
        canvas.set_clip(Some(clip));
    }
    pub(in crate::app) fn embedded_video_loading(&self) -> bool {
        self.embedded_video.as_ref().is_some_and(|v| {
            (v.plan.is_some() || !v.aborted && v.tasks.lock().map_or(false,|tasks|tasks.has_pending()))
                && !v.retired.load(Ordering::Relaxed)
        })
    }
    pub(in crate::app) fn cancel_embedded_video(&mut self) {
        if let Some(page) = &mut self.scripting {
            page.cancel_media_selection();
            page.cancel_media_pause();
        }
        if let Some(worker) = &self.embedded_video_worker {
            worker.cancel();
        }
        if let Some(v) = &mut self.embedded_video {
            v.script_play_intent=None;
            if v.plan.take().is_some() {
                v.aborted = true;
                v.network = 1;
                v.enqueue_events(["abort"]);
            }
        }
    }
    pub(in crate::app) fn prepare_embedded_video(&mut self) {
        if self.media.is_some() || self.audio.is_some() {
            return;
        }
        let Some(node) = crate::embedded_video::authored_node(&self.document) else {
            return;
        };
        let raw = self.document.attr(node, "src").unwrap_or("").to_owned();
        let mut binding = Embedded {
            retired: Arc::new(AtomicBool::new(false)),
            node,
            raw,
            source: String::new(),
            network: 2,
            plan: None,
            error: None,
            aborted: false,
            tasks:self.scripting.as_ref().and_then(|page|page.media_tasks())
                .unwrap_or_else(||media_tasks::shared(crate::javascript::limits::BROWSER.host.media_tasks)),
            last_update: Duration::ZERO,
            script_play_intent:None,
        };
        if !binding.enqueue_events(["loadstart"]) {
            self.events.push(format!("VIDEO node {node}: {}",binding.error.as_deref().unwrap_or("Media task queue failed.")));
            self.embedded_video=Some(binding);return;
        }
        let result = Plan::new(
            node,
            &self.document,
            &self.location,
            &self.base_location,
            &self.document_policy,
            self.media_limits,
        )
        .and_then(|mut plan| {
            plan.retired = binding.retired.clone();
            let plan = self.attach_embedded_network_context(plan, node)?;
            if self.embedded_video_worker.is_none() {
                self.embedded_video_worker =
                    Some(crate::embedded_video::Worker::new(self.wake.clone())?);
            }
            let plan=Arc::new(plan);
            binding.source = plan.source.display();
            self.embedded_video_worker
                .as_ref()
                .unwrap()
                .submit(plan.clone());
            binding.plan = Some(plan);
            Ok(())
        });
        if let Err(error) = result {
            self.events.push(format!("VIDEO node {node}: {error}"));
            binding.script_play_intent=None;
            binding.error = Some(error);
            binding.network = 3;
            binding.enqueue_events(["error"]);
        }
        self.embedded_video = Some(binding);
    }
    pub(in crate::app) fn embedded_video_snapshot(&self) -> Option<Snapshot> {
        let v = self.embedded_video.as_ref()?;
        if v.retired.load(Ordering::Relaxed) {
            return None;
        }
        let now = Instant::now();
        let mut s = Snapshot {
            tasks:v.tasks.clone(),
            retired: v.retired.clone(),
            node: v.node,
            current_src: v.source.clone(),
            width: 0,
            height: 0,
            duration: None,
            position: Duration::ZERO,
            observed: now,
            clock: crate::embedded_video::ClockKind::WallTime,
            playing: false,
            paused: v.script_play_intent.is_none(),
            load_pending: v.plan.is_some()&&!v.aborted,
            ended: false,
            seeking: false,
            ready: 0,
            network: v.network,
            error_code:v.error.as_ref().map(|_|if v.aborted {1}else{4}),
            frame_ready:false,
        };
        if let Some(p) = self
            .media
            .as_ref()
            .filter(|p| p.embedded && p.node == v.node)
        {
            s.width = p.clip.width as u32;
            s.height = p.clip.height as u32;
            s.duration = Some(p.clip.duration);
            s.position = p.position(now);
            s.clock = if p.sync.is_some() {
                crate::embedded_video::ClockKind::AcknowledgedPcm
            } else {
                crate::embedded_video::ClockKind::WallTime
            };
            s.frame_ready=p.error.is_none()&&p.displayed==p.clip.index_at(p.position(now));
            if p.error.is_some() {s.error_code=Some(if p.decoder_enabled {3}else{1});}
            s.playing = p.playing();
            s.paused = p.paused;
            s.ended = p.ended_reported;
            s.seeking = p.seeking;
            s.load_pending|=p.initial_data_pending&&p.error.is_none();
            s.ready = if p.error.is_some() || p.initial_data_pending || p.seeking && p.pending.is_some() {
                1
            } else {
                4
            };
        }
        Some(s)
    }
    pub(in crate::app) fn poll_embedded_video(&mut self) -> bool {
        let Some(completion) = self.embedded_video_worker.as_ref().and_then(|w| w.take()) else {
            return false;
        };
        self.accept_embedded_video(completion)
    }
    fn accept_embedded_video(&mut self, completion: Completion) -> bool {
        let Some(v) = &mut self.embedded_video else {
            return false;
        };
        if v.retired.load(Ordering::Relaxed)
            || !v.plan.as_ref().is_some_and(|p| {
                Arc::ptr_eq(p, &completion.plan) && p.node == v.node && p.raw == v.raw
            })
        {
            return false;
        }
        v.plan = None;
        if crate::embedded_video::authored_node(&self.document) != Some(v.node) {
            v.aborted = true;
            v.enqueue_events(["abort"]);
            return true;
        }
        let node = v.node;
        v.network = 1;
        match completion.result {
            Ok(asset) => {
                let source=self.embedded_video.as_ref().unwrap().retired.clone();
                // Metadata processing always clears the signed default. A
                // positive start enters the same real seek/decode path, and
                // initial data-ready events wait for its requested bitmap.
                let start=self.scripting.as_mut().and_then(|page|page.take_media_default_start(&source));
                self.install_media_with_readiness(Some(asset),Some(node),true,start.is_some());
                if let Some(seconds)=start {
                    let duration=self.media.as_ref().unwrap().clip.duration;
                    let target=Duration::try_from_secs_f64(seconds.min(duration.as_secs_f64()))
                        .unwrap_or(duration).min(duration);
                    let now=Instant::now();
                    self.media_seek_physical(target,now);
                    self.media_request_frame_physical(now);
                    self.finish_media_tick(now);
                }
                let intent=self.embedded_video.as_mut().and_then(|v|v.script_play_intent.take());
                if let Some(issued)=intent {
                    // Metadata/initial seek became real only now: never replay
                    // elapsed wall time from the loading interval.
                    self.script_media_play_physical(issued,false);
                }
                self.reflow();
            }
            Err(error) => {
                self.events.push(format!("VIDEO node {node}: {error}"));
                let v = self.embedded_video.as_mut().unwrap();
                v.script_play_intent=None;
                v.error = Some(error);
                v.network = 3;
                v.enqueue_events(["error"]);
            }
        }
        true
    }
    pub(in crate::app) fn fail_media_queue(&mut self, problem:String) {
        self.events.push(format!("VIDEO: {problem}"));
        // Preserve owned batch and pending rejection effects before any FIFO
        // entry is removed. This path never dispatches JS or checkpoints.
        if let Some(page)=&mut self.scripting {page.cancel_media_pause();}
        if let Some(worker)=&self.media_worker {worker.cancel();}
        if let Some(worker)=&self.embedded_video_worker {worker.cancel();}
        if let Some(v)=&mut self.embedded_video {
            if let Ok(mut tasks)=v.tasks.lock() {tasks.cancel_source(&v.retired);}
            v.fail_tasks(problem.clone());
        }
        if let Some(player)=self.media.as_mut().filter(|player|player.embedded) {player.fail_task_queue(problem);}
        let snapshot=self.embedded_video_snapshot();
        if let Some(page)=&mut self.scripting {page.set_embedded_video(snapshot);}
    }
    // Legacy call sites keep their issue-time guard, now draining all admitted
    // controls in the shared source FIFO before any tick or JS notification.
    pub(in crate::app) fn apply_media_pause(&mut self) -> bool {
        // A source selector may already be ready after load()/play(). Install
        // its binding before destructive control pops can discard the new Scope.
        let selected=if self.embedded_video_retired() {self.process_media_selection()}else{false};
        if self.embedded_video_retired() {return selected;}
        let Some(tasks)=self.embedded_video.as_ref().map(|v|v.tasks.clone())
            .or_else(||self.scripting.as_ref().and_then(|page|page.media_tasks())) else {return false;};
        let mut applied=selected;
        loop {
            let next=tasks.lock().map(|mut queue|queue.take_control())
                .map_err(|_|"Media task queue is unavailable.".to_owned());
            let request=match next {Ok(Some(request))=>request,Ok(None)=>break,Err(problem)=>{self.fail_media_queue(problem);break;}};
            let (_,scope,control)=request;
            let valid=!scope.retired.load(Ordering::Relaxed) && self.embedded_video.as_ref().is_some_and(|v|
                v.node==scope.node&&Arc::ptr_eq(&v.retired,&scope.retired)&&Arc::ptr_eq(&v.tasks,&tasks));
            if !valid {continue;}
            match control {
                crate::media_tasks::Control::Pause {issued,position}=>{
                    if let Some(v)=&mut self.embedded_video {v.script_play_intent=None;}
                    if let Some(player)=self.media.as_mut().filter(|p|p.embedded&&p.node==scope.node&&p.sync.is_none()) {
                        player.clock.pause(issued);player.paused=true;player.dom_playback_started=false;
                        player.script_play_waiting=false;player.record_only("pause");
                        if player.seek_resume.is_some() {player.seek_resume=Some(false);}
                        debug_assert_eq!(player.clock.position(issued),position);
                        if player.pending.is_none() {player.deadline=None;}
                    }
                }
                crate::media_tasks::Control::Play {issued,notify_queued}=>{
                    if self.media.as_ref().is_some_and(|p|p.embedded&&p.node==scope.node) {
                        self.script_media_play_physical(issued,notify_queued);
                    } else if let Some(v)=&mut self.embedded_video {
                        v.script_play_intent=if v.error.is_none()&&!v.aborted {Some(issued)}else{None};
                    }
                }
                crate::media_tasks::Control::Seek {seconds}=>{
                    let Some(duration)=self.media.as_ref().filter(|p|p.embedded&&p.node==scope.node&&p.sync.is_none()).map(|p|p.clip.duration) else {
                        self.fail_media_queue("Media seek has no bound decoded player.".into());break;
                    };
                    if !seconds.is_finite() {self.fail_media_queue("Media seek position must be finite.".into());break;}
                    let target=Duration::from_secs_f64(seconds.max(0.0).min(duration.as_secs_f64())).min(duration);
                    self.media_seek_physical(target,Instant::now());
                }
            }
            applied=true;
        }
        // Completion work is requested only after the full physical-control
        // pass. Neither helper re-enters the control drain or dispatches JS.
        if applied {
            let now=Instant::now();self.media_request_frame_physical(now);self.finish_media_tick(now);
        }
        applied
    }
    pub(in crate::app) fn poll_media_promise_cleanup(&mut self)->bool {
        if self.script_jobs_paused() {return false;}
        let Some(page)=self.scripting.as_mut().filter(|page|page.has_media_promise_effects()) else {return false;};
        let outcome=page.run_media_promise_cleanup(&mut self.document,&mut self.forms);
        if outcome.layout_changed {self.reflow();}
        self.validate_control_focus();self.sync_editors();
        if outcome.errors!=0 {self.show_script_errors();}
        true
    }
    pub(in crate::app) fn flush_embedded_video(&mut self) -> bool {
        let controls=self.apply_media_pause();
        let processed=self.process_media_selection() | controls;
        let controls=self.apply_media_pause() | controls;
        if self.embedded_video_retired() {return processed | controls;}
        if let Some(problem)=self.embedded_video.as_ref().filter(|v|v.aborted).and_then(|v|v.error.clone()) {
            let stopped=self.media.as_ref().is_none_or(|p|p.suspended&&!p.decoder_enabled);
            let queue_empty=self.embedded_video.as_ref().is_none_or(|v|v.tasks.lock().map_or(false,|queue|!queue.has_pending()));
            if !stopped||!queue_empty {self.fail_media_queue(problem);return true;}
        }
        if let Some(problem)=self.media.as_ref().filter(|player|player.embedded&&!player.decoder_enabled)
            .and_then(|player|player.error.clone()) {
            if self.embedded_video.as_ref().is_some_and(|v|v.error.as_ref()!=Some(&problem)||!v.aborted) {
                self.fail_media_queue(problem);return true;
            }
        }
        let Some(v)=&mut self.embedded_video else {return processed | controls;};
        let node=v.node;let resource=v.retired.clone();let tasks=v.tasks.clone();
        if let Some(player)=self.media.as_ref().filter(|player|player.embedded&&player.node==node) {
            let position=player.position(Instant::now());
            if position.abs_diff(v.last_update)>=Duration::from_millis(250) {
                if !v.enqueue_events(["timeupdate"]) {
                    let problem=v.error.clone().unwrap_or_else(||"Media task queue failed.".into());
                    self.fail_media_queue(problem);return true;
                }
                v.last_update=position;
            }
        }
        let finish=v.plan.is_none()&&!v.aborted;
        let snapshot=self.embedded_video_snapshot();
        let mut captured_or_aborted=false;
        let fault=if let Some(page)=&mut self.scripting {
            page.set_embedded_video(snapshot);
            captured_or_aborted=page.pending_media_play_count()==0;
            page.take_media_play_fault()
        } else {None};
        if captured_or_aborted {if let Some(player)=&mut self.media {player.script_play_waiting=false;}}
        if let Some(problem)=fault {self.fail_media_queue(problem);return true;}
        let budget=match tasks.lock().map(|queue|queue.count_tasks())
            .map_err(|_|"Media task queue is unavailable.".to_owned()) {
            Ok(count)=>count,Err(problem)=>{self.fail_media_queue(problem);return true;}
        };
        let mut changed=processed | controls;
        for _ in 0..budget {
            if self.script_jobs_paused() {break;}
            let next=tasks.lock().map(|mut queue|queue.take_task())
                .map_err(|_|"Media task queue is unavailable.".to_owned());
            let (_,scope,task)=match next {Ok(Some(task))=>task,Ok(None)=>break,Err(problem)=>{self.fail_media_queue(problem);break;}};
            // The queue lock is gone before authority checks and JS. Check the
            // source once before a task, never between members of a Pause pair.
            if scope.retired.load(Ordering::Relaxed) || !self.embedded_video.as_ref().is_some_and(|v|
                v.node==scope.node&&Arc::ptr_eq(&v.retired,&scope.retired)&&Arc::ptr_eq(&v.tasks,&tasks)) {
                continue;
            }
            if let Some(page)=&mut self.scripting {
                let outcome=page.dispatch_media_task(&mut self.document,&mut self.forms,scope.node,task);
                if outcome.layout_changed {self.reflow();}
                changed=true;
            }
        }
        if finish&&!resource.load(Ordering::Relaxed) {
            if let Some(page)=&mut self.scripting {
                let outcome=page.finish_loading(&mut self.document,&mut self.forms);
                if outcome.layout_changed {self.reflow();}
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_video_javascript_load_restarts_same_source_and_rejects_old_completion() {
        let mut app=open("authored-video.html");app.wait_for_load();let node=app.media.as_ref().unwrap().node;let first=app.images[&node].clone();let retired=app.embedded_video.as_ref().unwrap().retired.clone();
        app.input_media_action("Play").unwrap();run(&mut app,Duration::from_millis(650));
        eval(&mut app,"var reloadAttr=video.getAttributeNode('src'),reloadRaw=reloadAttr.value;mediaEvents=[];if(video.load()!==undefined||video.currentTime!==0||!video.paused||video.readyState!==0||video.seeking||!isNaN(video.duration)||video.getAttributeNode('src')!==reloadAttr||reloadAttr.value!==reloadRaw)throw 'load reset/identity';");
        assert!(retired.load(Ordering::Relaxed));app.wait_for_load();assert_eq!(app.images[&node].pixels,first.pixels);assert!(!app.media.as_ref().unwrap().playing());assert_eq!(app.media.as_ref().unwrap().position(Instant::now()),Duration::ZERO);
        eval(&mut app,"if(mediaEvents.slice(0,4).join(',')!=='abort,emptied,timeupdate,loadstart'||video.readyState!==4||video.duration!==3||video.getAttributeNode('src')!==reloadAttr||reloadAttr.value!==reloadRaw)throw 'real reload events '+mediaEvents.join(',')+' ready='+video.readyState;video.load();");app.process_media_selection();let stale=app.embedded_video.as_ref().unwrap().plan.clone().unwrap();
        eval(&mut app,"video.load();");app.process_media_selection();let asset=media::Clip::load(include_bytes!("../tools/fixtures/scene-intra.ivf").to_vec(),media::Limits::default(),&||Ok(())).unwrap();assert!(!app.accept_embedded_video(Completion {plan:stale,result:Ok(asset)}));assert!(app.media.is_none());app.wait_for_load();assert_eq!(app.images[&node].pixels,first.pixels);
    }
    fn open(name: &str) -> App {
        let mut app = App::new();
        app.scripts_enabled = true;
        app.images_enabled = false;
        app.open(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tools/fixtures")
                .join(name),
        );
        app
    }
    fn eval(app: &mut App, source: &str) {
        let p = app.scripting.as_mut().unwrap();
        let outcome = p.console_eval(&mut app.document, &mut app.forms, source);
        assert_eq!(outcome.errors, 0, "{}", p.report());
        if outcome.layout_changed {
            app.reflow();
        }
    }
    fn run(app: &mut App, duration: Duration) {
        let end = Instant::now() + duration;
        while Instant::now() < end {
            app.poll_load();
            app.poll_page_timer();
            std::thread::sleep(Duration::from_millis(1));
        }
        app.poll_load();
    }
    fn settle(app: &mut App) {
        let end = Instant::now() + Duration::from_secs(5);
        while app.media.as_ref().is_some_and(|p| p.pending.is_some()) {
            assert!(Instant::now() < end);
            app.poll_load();
            std::thread::sleep(Duration::from_millis(1));
        }
        app.flush_embedded_video();
    }
    #[test]
    fn authored_video_real_node_metadata_events_controls_pause_seek_end_replay_and_navigation() {
        let mut app = open("authored-video.html");
        assert!(app.is_loading());
        assert!(app.media.is_none());
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
        let p = app.media.as_ref().expect("real element player");
        assert!(p.embedded);
        let node = p.node;
        assert!(
            matches!(&app.document.nodes[node].kind,crate::html::NodeKind::Element {tag,..} if tag=="video")
        );
        assert!(!app
            .document
            .nodes
            .iter()
            .any(|n| matches!(&n.kind,crate::html::NodeKind::Element {tag,..} if tag=="img")));
        assert_eq!(
            app.scripting.as_ref().unwrap().error_count(),
            0,
            "{}",
            app.scripting.as_ref().unwrap().report()
        );
        eval(&mut app,"if(initialMediaState!==0||!mediaWindowLoaded||video.readyState!==4||video.networkState!==1||video.currentTime!==0||mediaEvents.slice(0,6).join(',')!=='loadstart,durationchange,loadedmetadata,loadeddata,canplay,canplaythrough')throw 'actual initial sequence';video.duration=0;if(video.duration!==3)throw 'readonly duration';var deniedPlay=video.play(),deniedSeen=false;if(!(deniedPlay instanceof Promise)||typeof onplay!=='undefined'||window.onloadedmetadata!==undefined)throw 'genuine promise/window scope';deniedPlay.catch(function(e){if(!(e instanceof DOMException)||e.name!=='NotAllowedError')throw 'play policy reason';deniedSeen=true;});queueMicrotask(function(){if(!deniedSeen)throw 'policy rejection missing';});var propertyEvents=0;video.onpause=function(e){if(e.target!==video||!e.isTrusted)throw 'native handler';propertyEvents++;};if(!('onpause' in video)||!('onerror' in video))throw 'native handler presence';");
        assert!(
            !app.media_key(0x20),
            "Page Space must not activate an unfocused element"
        );
        let first = app.images[&node].clone();
        app.input_media_action("Play").unwrap();
        run(&mut app, Duration::from_millis(650));
        app.input_media_action("Pause").unwrap();
        settle(&mut app);
        let paused = app.embedded_video_snapshot().unwrap().position;
        assert!(paused >= Duration::from_millis(500));
        let frame = app.media.as_ref().unwrap().displayed;
        assert!(frame >= 1);
        let paused_pixels = app.images[&node].clone();
        assert_ne!(first.pixels, paused_pixels.pixels);
        run(&mut app, Duration::from_millis(100));
        assert_eq!(app.embedded_video_snapshot().unwrap().position, paused);
        assert_eq!(app.media.as_ref().unwrap().displayed, frame);
        assert_eq!(app.images[&node].pixels, paused_pixels.pixels);
        eval(&mut app,"if(!video.paused||video.currentTime<0.5||mediaEvents.indexOf('playing')<0||mediaEvents.indexOf('pause')<0||propertyEvents!==1)throw 'real play and pause';");
        app.input_media_action("Seek:2200").unwrap();
        settle(&mut app);
        assert_eq!(app.media.as_ref().unwrap().displayed, 4);
        assert!(!app.media.as_ref().unwrap().seeking);
        eval(&mut app,"if(video.currentTime<2.1||video.currentTime>2.3||!video.paused||video.seeking||mediaEvents.indexOf('seeking')<0||mediaEvents.indexOf('seeked')<0)throw 'actual settled seek';");
        app.input_media_action("Play").unwrap();
        run(&mut app, Duration::from_millis(1000));
        settle(&mut app);
        assert_eq!(app.media.as_ref().unwrap().displayed, 5);
        assert!(app.media.as_ref().unwrap().ended_reported);
        eval(&mut app,"if(!video.ended||!video.paused||video.currentTime!==3||mediaEvents.indexOf('ended')<0||document.getElementById('media-state').textContent!=='Video ended')throw 'confirmed actual end';");
        app.input_media_action("Play").unwrap();
        eval(
            &mut app,
            "if(video.ended||video.paused||video.currentTime>0.2)throw 'real replay';",
        );
        app.input_media_action("Stop").unwrap();
        eval(
            &mut app,
            "if(!video.paused||video.currentTime!==0||video.ended)throw 'real stop';",
        );
        app.open(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tools/fixtures/authored-video-next.html"),
        );
        app.wait_for_load();
        assert!(app.media.is_none() && app.embedded_video.is_none());
        assert!(app.next_media_timer().is_none());
        assert!(app.audio.is_none() && app.audio_worker.is_none());
    }
    #[test]
    fn authored_video_controls_respect_real_paint_order_and_reappear_after_overlay_removal() {
        let mut app = open("authored-video-covered.html");
        app.wait_for_load();
        let p = app.media.as_ref().unwrap();
        let node = p.node;
        let r = app.media_control_rect().unwrap();
        let (x, y) = (r.left + 40, r.top + 16);
        assert!(!app
            .page
            .image_topmost_at(node, x, y - app.content_top() + app.scroll));
        assert!(app.media_hit(x, y).is_none());
        assert!(app.input_media_action("Play").is_err());
        assert!(!app.media.as_ref().unwrap().playing());
        eval(
            &mut app,
            "var cover=document.getElementById('cover');cover.parentNode.removeChild(cover);",
        );
        assert!(app.media_hit(x, y).is_some());
        app.input_media_action("Play").unwrap();
        assert!(app.media.as_ref().unwrap().playing());
        app.stop_media();
        let p = app.scripting.as_mut().unwrap();
        let out = p.console_eval(
            &mut app.document,
            &mut app.forms,
            "document.body.innerHTML='<p>replacement</p>';",
        );
        assert_eq!(out.errors, 1, "{}", p.report());
        assert!(app.media.as_ref().unwrap().embedded);
        app.scroll_by(r.top - app.content_top() + 10);
        let clipped = app.media_control_rect().unwrap();
        assert_eq!(clipped.top, app.content_top() - 10);
        let (x, y) = (clipped.left + 12, app.content_top() + 6);
        assert!(app.media_hit(x, y).is_some());
        let canvas = app.draw();
        assert_eq!(
            canvas.pixels[y as usize * app.width as usize + x as usize],
            0x346277,
            "a hit must have actual visible button RGB pixels"
        );
        app.input_media_action("Play").unwrap();
        eval(
            &mut app,
            "document.getElementById('authored-video').style.display='none';",
        );
        assert!(app.media_control_rect().is_none());
        assert!(!app.media_key(0x20));
        app.stop_media();
    }
    #[test]
    fn authored_video_source_attribute_and_structure_guards_preserve_loaded_resource_and_foreign_owners(
    ) {
        let mut app = open("authored-video.html");
        app.wait_for_load();
        let node = app.media.as_ref().unwrap().node;
        let pixels = app.images[&node].clone();
        let src = app.document.attr(node, "src").unwrap().to_owned();
        for source in [
            "video.setAttribute('autoplay','');",
            "video.setAttribute('poster','authored-video-next.html');",
            "video.parentNode.removeChild(video);",
            "document.body.textContent='replacement';",
            "document.body.innerHTML='<p>replacement</p>';",
        ] {
            let p = app.scripting.as_mut().unwrap();
            let out = p.console_eval(&mut app.document, &mut app.forms, source);
            assert_eq!(out.errors, 1, "{source}: {}", p.report());
            assert_eq!(app.document.attr(node, "src"), Some(src.as_str()));
            assert!(Arc::ptr_eq(&pixels, &app.images[&node]));
            assert!(!app.embedded_video_loading());
        }
        eval(&mut app,"video.setAttributeNS('urn:original','src','metadata-only');if(video.getAttribute('src')!=='scene-intra.ivf'||video.readyState!==4)throw 'namespace metadata';");
        // Trusted host-context isolation control: local-file blank frames are
        // deliberately unavailable. This changes no App source/root or media IO.
        app.scripting
            .as_mut()
            .unwrap()
            .set_location(&crate::navigation::Location {
                url: Some(crate::url::parse("https://source.test/").unwrap().0),
                path: None,
                fragment: String::new(),
                root: None,
            });
        eval(&mut app,"var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);var w=f.contentWindow,read=w.Function('v','return v.duration;'),present=w.Function('v','return \"duration\" in v;'),writeOwned=w.Function('v','x','v.src=x;'),reloadOwned=w.Function('v','v.load();'),savedLoad=video.load,borrowLoad=w.Function('fn','v','fn.call(v);'),sourceConversions=0;if(read(video)!==3||!present(video))throw 'same origin native owner';");
        let p = app.scripting.as_mut().unwrap();
        p.set_location(&crate::navigation::Location {
            url: Some(crate::url::parse("https://other.test/").unwrap().0),
            path: None,
            fragment: String::new(),
            root: None,
        });
        let count = app.document.nodes.len();
        for source in ["read(video);", "present(video);", "writeOwned(video,{toString:function(){sourceConversions++;return 'scene-replacement.ivf';}});","reloadOwned(video);","borrowLoad(savedLoad,video);"] {
            let out = p.console_eval(&mut app.document, &mut app.forms, source);
            assert_eq!(out.errors, 1, "{}", p.report());
            assert!(p.report().contains("same-origin"));
            assert_eq!(app.document.nodes.len(), count);
        }
        let out=p.console_eval(&mut app.document,&mut app.forms,"if(sourceConversions!==0)throw 'foreign source coerced';");assert_eq!(out.errors,0,"{}",p.report());
    }
    #[test]
    fn authored_video_csp_refusal_stop_and_old_document_source_completion_never_attach() {
        let mut blocked = open("authored-video-blocked.html");
        blocked.wait_for_load();
        assert!(blocked.media.is_none());
        assert!(blocked.embedded_video_worker.is_none());
        assert!(blocked
            .events
            .iter()
            .any(|s| s.contains("media-src/default-src")));
        assert_eq!(blocked.embedded_video_snapshot().unwrap().network, 3);
        let mut app = open("authored-video.html");
        let plan = app.embedded_video.as_ref().unwrap().plan.clone().unwrap();
        app.stop_loading();
        assert!(!app.embedded_video_loading());
        assert!(app.media.is_none());
        app.poll_load();
        assert!(app.media.is_none());
        app.open(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tools/fixtures/authored-video-next.html"),
        );
        let asset = media::Clip::load(
            include_bytes!("../tools/fixtures/scene-intra.ivf").to_vec(),
            media::Limits::default(),
            &|| Ok(()),
        )
        .unwrap();
        assert!(!app.accept_embedded_video(Completion {
            plan,
            result: Ok(asset)
        }));
        assert!(app.media.is_none() && app.embedded_video.is_none());
        let mut cancelled = open("authored-video.html");
        cancelled.script_timer_unavailable();
        assert!(!cancelled.is_loading());
        assert!(cancelled.media.is_none());
        eval(&mut cancelled,"if(video.networkState===2||mediaEvents.indexOf('abort')<0)throw 'cancelled source snapshot';");
    }
    #[test]
    fn source_replacement_stable_fifo_reset_removal_and_same_value_load() {
        let mut app = open("authored-video.html");
        app.wait_for_load();
        let old = app.embedded_video.as_ref().unwrap().retired.clone();
        eval(&mut app,"var oldCurrent=video.currentSrc,selectionOrder='';queueMicrotask(function(){if(video.networkState!==3||video.readyState!==0||video.currentSrc!==oldCurrent||!video.paused||!isNaN(video.duration))throw 'before stable';selectionOrder+='before;';});video.src='scene-replacement.ivf';if(video.readyState!==0||video.currentTime!==0||video.currentSrc!==oldCurrent)throw 'sync reset';Promise.resolve().then(function(){if(video.networkState!==2||video.currentSrc.indexOf('scene-replacement.ivf')<0)throw 'after stable';selectionOrder+='after;';});");
        assert!(old.load(Ordering::Relaxed));
        assert!(app.embedded_video_retired());
        assert!(app.media_control_rect().is_none());
        // Removal after stable selection retains the selected resource.
        eval(&mut app,"video.removeAttribute('src');if(selectionOrder!=='before;after;'||video.networkState!==2)throw 'selected removal';");
        app.wait_for_load();
        assert!(app.media.is_some());
        assert_eq!(
            app.embedded_video.as_ref().unwrap().raw,
            "scene-replacement.ivf"
        );
        eval(&mut app,"if(video.readyState!==4||video.currentSrc.indexOf('scene-replacement.ivf')<0||video.getAttribute('src')!==null)throw 'retained selected resource';video.src='scene-intra.ivf';video.removeAttribute('src');");
        app.wait_for_load();
        assert!(app.media.is_none());
        eval(&mut app,"if(video.networkState!==0||video.readyState!==0||video.currentSrc.indexOf('scene-replacement.ivf')<0)throw 'removed before stable';video.src='scene-intra.ivf';");
        app.wait_for_load();
        let token = app.embedded_video.as_ref().unwrap().retired.clone();
        eval(&mut app, "video.src=video.getAttribute('src');");
        assert!(token.load(Ordering::Relaxed));
        app.wait_for_load();
        assert!(app.media.is_some());
    }
    #[test]
    fn source_replacement_attr_routes_failures_and_stale_completion() {
        let mut app = open("authored-video.html");
        app.wait_for_load();
        eval(
            &mut app,
            "video.setAttribute('src','scene-replacement.ivf');",
        );
        app.process_media_selection();
        let stale = app.embedded_video.as_ref().unwrap().plan.clone().unwrap();
        eval(
            &mut app,
            "video.getAttributeNode('src').value='missing-selected.ivf';",
        );
        eval(
            &mut app,
            "if(video.currentSrc.indexOf('missing-selected.ivf')<0)throw 'selected missing URI';",
        );
        app.process_media_selection();
        let asset = media::Clip::load(
            include_bytes!("../tools/fixtures/scene-intra.ivf").to_vec(),
            media::Limits::default(),
            &|| Ok(()),
        )
        .unwrap();
        assert!(!app.accept_embedded_video(Completion {
            plan: stale,
            result: Ok(asset)
        }));
        app.wait_for_load();
        assert!(app.media.is_none());
        eval(&mut app,"if(video.networkState!==3||video.readyState!==0||video.currentSrc.indexOf('missing-selected.ivf')<0)throw 'honest source failure';video.src='';if(video.currentSrc.indexOf('missing-selected.ivf')<0)throw 'empty preserves currentSrc';");
        app.wait_for_load();
        assert!(app.media.is_none());
        eval(
            &mut app,
            "video.src='scene-replacement.ivf';video.src='scene-intra.ivf';",
        );
        app.wait_for_load();
        assert_eq!(app.embedded_video.as_ref().unwrap().raw, "scene-intra.ivf");
        assert!(app.media.is_some());
        for source in ["video.setAttributeNS(null,'src','scene-replacement.ivf');","var a=document.createAttribute('src');a.value='scene-intra.ivf';video.setAttributeNode(a);","var b=document.createAttribute('src');b.value='scene-replacement.ivf';video.setAttributeNodeNS(b);"]{
            eval(&mut app,source);app.wait_for_load();assert!(app.media.is_some());
        }
    }
    #[test]
    fn source_replacement_callback_discards_old_resource_events_and_stop_pending() {
        let mut app = open("authored-video.html");
        app.wait_for_load();
        eval(&mut app,"var oldLoaded=0;video.onloadedmetadata=function(){video.src='missing-selected.ivf';};video.onloadeddata=function(){oldLoaded++;};video.src='scene-replacement.ivf';");
        app.wait_for_load();
        assert!(app.media.is_none());
        eval(&mut app,"if(oldLoaded!==0||video.networkState!==3)throw 'old event escaped replacement';video.src='scene-intra.ivf';");
        app.stop_loading();
        app.poll_load();
        assert!(!app.is_loading());
        assert!(app.media.is_none());
    }
    #[test]
    fn authored_video_without_controls_keeps_real_frame_and_never_paints_or_hits_native_bar() {
        let mut app = open("authored-video-no-controls.html");
        app.wait_for_load();
        assert!(app.media.as_ref().unwrap().embedded);
        assert!(app.media_control_rect().is_none());
        assert!(app.input_media_action("Play").is_err());
        assert!(!app.media_key(0x20));
        let mut canvas = Canvas::new(520, 400);
        canvas.fill_rect(0, 0, 520, 400, 0xff123456);
        let before = canvas.pixels.clone();
        let node = app.media.as_ref().unwrap().node;
        app.draw_embedded_video_controls(
            &mut canvas,
            node,
            crate::paint::ClipRect::new(0, 0, 480, 360),
        );
        assert_eq!(canvas.pixels, before);
        assert!(app.images.contains_key(&node));
    }

    #[test]
    fn script_pause_freezes_issue_time_before_a_later_native_timer_tick() {
        let mut app = open("authored-video.html");
        app.wait_for_load();
        app.input_media_action("Play").unwrap();
        app.media_seek(Duration::from_millis(700), Instant::now());
        app.flush_embedded_video();
        let player = app.media.as_ref().unwrap();
        let clip = player.clip.clone();
        let generation = player.generation;
        let pending = player.pending;
        let source = app.embedded_video.as_ref().unwrap().retired.clone();
        eval(&mut app, "video.pause();var pausedIssue=video.currentTime;if(!video.paused||video.currentTime!==pausedIssue)throw 'sync pause';");
        let request = app.scripting.as_ref().unwrap().media_pause().unwrap().clone();
        // A native media timer can be arbitrarily delayed. The issue-time
        // command must win BEFORE tick can clear the old clock anchor at EOF.
        app.poll_media_timer_at(request.issued + Duration::from_secs(4));
        let player = app.media.as_ref().unwrap();
        assert_eq!(player.clock.position(Instant::now()), request.position);
        assert!(!player.playing() && player.paused);
        assert!(Arc::ptr_eq(&player.clip, &clip));
        assert_eq!(player.generation, generation);
        assert_eq!(player.pending, pending);
        assert!(Arc::ptr_eq(&app.embedded_video.as_ref().unwrap().retired, &source));
        settle(&mut app);
        assert_eq!(app.media.as_ref().unwrap().displayed, 1);
        assert_eq!(app.embedded_video_snapshot().unwrap().position, request.position);
        eval(&mut app, "if(!video.paused||video.currentTime!==pausedIssue||video.seeking)throw 'settled pause';");
    }

    #[test]
    fn script_pause_survives_producer_fatal_error_before_snapshot_boundary() {
        let mut app = open("authored-video.html");
        app.wait_for_load();
        app.input_media_action("Play").unwrap();
        let result = app.scripting.as_mut().unwrap().console_eval(&mut app.document,
            &mut app.forms, "var stalePauseJob=0;video.pause();queueMicrotask(function(){stalePauseJob++;});document.body.scrollWidth;");
        assert_eq!(result.errors, 1);
        let request = app.scripting.as_ref().unwrap().media_pause().unwrap().clone();
        app.show_script_errors();
        assert_eq!(app.media.as_ref().unwrap().clock.position(Instant::now()), request.position);
        assert!(!app.media.as_ref().unwrap().playing());
        app.flush_embedded_video();
        eval(&mut app, "if(stalePauseJob!==0||!video.paused)throw 'fatal producer cleanup';");
    }

    fn assert_reference_frame(app: &App, index: usize) {
        let node = app.media.as_ref().unwrap().node;
        let bitmap = &app.images[&node];
        assert_eq!((bitmap.width, bitmap.height), (64, 48));
        let reference = include_bytes!("../tools/fixtures/scene-intra.rgba");
        let expected = &reference[index*64*48*4..(index+1)*64*48*4];
        assert_eq!(bitmap.pixels.len(), 64*48);
        let (mut error, mut maximum) = (0u64, 0u32);
        for (pixel, rgba) in bitmap.pixels.iter().zip(expected.chunks_exact(4)) {
            for (actual, expected) in [((pixel>>16)&255,rgba[0]),((pixel>>8)&255,rgba[1]),(pixel&255,rgba[2])] {
                let delta=actual.abs_diff(u32::from(expected));
                error+=u64::from(delta);maximum=maximum.max(delta);
            }
            assert_eq!(pixel>>24,255);
        }
        // The established original decoder/reference use different chroma
        // upsampling. Retain both existing mean and edge-error bounds.
        assert!(error < 64*48*3*4 && maximum <= 80, "frame {index}: error={error}, max={maximum}");
    }

    #[test]
    fn script_pause_preserves_real_pending_seek_at_eof_and_non_eof() {
        for (was_playing, target, index) in [(true,3000,5),(true,2200,4),(false,2200,4)] {
            let mut app = open("authored-video.html");
            app.wait_for_load();
            if was_playing {app.input_media_action("Play").unwrap();}
            app.media_seek(Duration::from_millis(target), Instant::now());
            app.flush_embedded_video();
            let player = app.media.as_ref().unwrap();
            assert!(player.seeking);
            assert_eq!(player.pending, Some(index));
            assert_eq!(player.paused, !was_playing);
            if target==3000 {assert!(!player.clock.playing());}
            let generation = player.generation;
            let clip = player.clip.clone();
            let source = app.embedded_video.as_ref().unwrap().retired.clone();
            // Hold a genuine original worker result. Decoding may complete at
            // any speed; App has not accepted/published this requested frame.
            let deadline=Instant::now()+Duration::from_secs(5);
            let completion=loop {
                if let Some(done)=app.media_worker.as_ref().unwrap().take() {break done;}
                assert!(Instant::now()<deadline,"real decode did not finish");
                std::thread::sleep(Duration::from_millis(1));
            };
            eval(&mut app,"video.pause();if(!video.paused||!video.seeking)throw 'pause preserves seek';");
            app.apply_media_pause();
            let player=app.media.as_ref().unwrap();
            assert!(player.seeking && player.paused && !player.playing());
            assert_eq!(player.pending,Some(index));
            assert_eq!(player.generation,generation);
            assert!(Arc::ptr_eq(&player.clip,&clip));
            assert!(Arc::ptr_eq(&app.embedded_video.as_ref().unwrap().retired,&source));
            assert!(!source.load(Ordering::Relaxed));
            assert!(app.accept_media_frame(completion));
            // The requested bitmap is installed before seeking is cleared and
            // any queued seeked handler can run.
            assert_eq!(app.media.as_ref().unwrap().displayed,index);
            assert!(!app.media.as_ref().unwrap().seeking);
            assert_reference_frame(&app,index);
            app.flush_embedded_video();
            eval(&mut app,"if(!video.paused||video.seeking||mediaEvents.indexOf('seeked')<0)throw 'real seek completion';");
            assert!(!app.media.as_ref().unwrap().playing());
        }
    }

    #[test]
    fn script_pause_in_keydown_precedes_native_arrow_seek_default() {
        let mut app=open("authored-video.html");
        app.wait_for_load();
        app.input_media_action("Play").unwrap();
        app.media_seek(Duration::from_millis(700),Instant::now());
        app.flush_embedded_video();
        eval(&mut app,"document.addEventListener('keydown',function(){video.pause();});");
        app.focus=Focus::Page;
        app.media.as_mut().unwrap().controls_focused=true;
        app.key(0x27,false,false,false);
        settle(&mut app);
        assert_eq!(app.media.as_ref().unwrap().clock.position(Instant::now()),Duration::from_secs(3));
        assert!(app.media.as_ref().unwrap().paused);
        assert!(!app.media.as_ref().unwrap().playing());
        assert_eq!(app.media.as_ref().unwrap().displayed,5);
        assert_reference_frame(&app,5);
    }

    fn take_decoded_frame(app:&App)->media::Completion {
        let deadline=Instant::now()+Duration::from_secs(5);
        loop {
            if let Some(frame)=app.media_worker.as_ref().unwrap().take() {return frame;}
            assert!(Instant::now()<deadline,"real decoder completion timed out");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn script_current_time_paused_seek_publishes_frame_before_completion_events() {
        let mut app=open("authored-video.html");app.wait_for_load();
        eval(&mut app,"mediaEvents=[];video.currentTime=1.25;if(video.currentTime!==1.25||!video.seeking||!video.paused)throw 'immediate seek';");
        app.apply_media_pause();
        assert_eq!(app.media.as_ref().unwrap().pending,Some(2));
        let frame=take_decoded_frame(&app);
        eval(&mut app,"if(mediaEvents.indexOf('seeked')>=0||!video.seeking)throw 'early completion';");
        assert!(app.accept_media_frame(frame));
        assert_eq!(app.media.as_ref().unwrap().displayed,2);assert_reference_frame(&app,2);
        assert!(!app.media.as_ref().unwrap().seeking);
        app.flush_embedded_video();
        eval(&mut app,"if(video.currentTime!==1.25||!video.paused||video.seeking||mediaEvents.indexOf('seeking')<0||mediaEvents.indexOf('timeupdate')>mediaEvents.indexOf('seeked')||mediaEvents.indexOf('seeked')<0)throw 'settled seek state/events';");
    }

    #[test]
    fn script_current_time_playing_seek_waits_for_real_frame_and_pending_pause_wins() {
        for pause in [false,true] {
            let mut app=open("authored-video.html");app.wait_for_load();app.input_media_action("Play").unwrap();
            eval(&mut app,"video.currentTime=1.25;");app.apply_media_pause();
            assert!(!app.media.as_ref().unwrap().paused);
            assert!(!app.media.as_ref().unwrap().clock.playing());
            assert_eq!(app.media.as_ref().unwrap().seek_resume,Some(true));
            let frame=take_decoded_frame(&app);
            app.poll_media_timer_at(Instant::now()+Duration::from_secs(4));
            assert_eq!(app.media.as_ref().unwrap().clock.position(Instant::now()),Duration::from_millis(1250));
            assert_eq!(app.media.as_ref().unwrap().pending,Some(2));
            if pause {eval(&mut app,"video.pause();");app.apply_media_pause();}
            assert!(app.accept_media_frame(frame));assert_reference_frame(&app,2);
            let player=app.media.as_ref().unwrap();assert_eq!(player.paused,pause);
            assert_eq!(player.clock.playing(),!pause);assert!(!player.seeking);
            assert!(player.seek_resume.is_none());
        }
    }

    #[test]
    fn script_current_time_metadata_defaults_clamp_and_use_actual_decoded_frames() {
        for (seconds,position,index) in [(-2.5,0,0),(1.75,1750,3),(f64::MAX,3000,5)] {
            let mut app=open("authored-video-default.html");
            eval(&mut app,&format!("video.currentTime={seconds};if(video.currentTime!=={seconds}||video.readyState!==0||video.seeking)throw 'premetadata default';"));
            app.wait_for_load();settle(&mut app);
            assert_eq!(app.media.as_ref().unwrap().displayed,index);assert_reference_frame(&app,index);
            assert_eq!(app.media.as_ref().unwrap().clock.position(Instant::now()),Duration::from_millis(position));
            eval(&mut app,&format!("if(video.currentTime!=={}||video.seeking||!video.paused)throw 'default actual playback position';",position as f64/1000.0));
        }
    }

    #[test]
    fn script_current_time_repeated_same_frame_and_ordered_pause_seek_controls() {
        let mut app=open("authored-video.html");app.wait_for_load();
        eval(&mut app,"video.currentTime=0;");app.flush_embedded_video();
        assert!(app.media.as_ref().unwrap().pending.is_none());assert!(!app.media.as_ref().unwrap().seeking);
        app.input_media_action("Play").unwrap();
        eval(&mut app,"video.currentTime=2.2;video.pause();video.currentTime=0.7;if(!video.paused||!video.seeking||video.currentTime!==0.7)throw 'ordered projection';");
        app.apply_media_pause();settle(&mut app);assert_reference_frame(&app,1);
        assert_eq!(app.media.as_ref().unwrap().clock.position(Instant::now()),Duration::from_millis(700));
        assert!(app.media.as_ref().unwrap().paused);assert!(!app.media.as_ref().unwrap().playing());
        eval(&mut app,"video.currentTime=-100;if(video.currentTime!==-100)throw 'finite negative official';");
        app.flush_embedded_video();assert_reference_frame(&app,0);
        eval(&mut app,"if(video.currentTime!==0||video.seeking)throw 'negative real clamp';");
    }

    #[test]
    fn script_current_time_supersedes_real_worker_completion_and_survives_producer_failure() {
        let mut app=open("authored-video.html");app.wait_for_load();
        eval(&mut app,"video.currentTime=1.25;");app.apply_media_pause();let old=take_decoded_frame(&app);
        eval(&mut app,"video.currentTime=2.2;");app.apply_media_pause();
        assert!(!app.accept_media_frame(old));assert_eq!(app.media.as_ref().unwrap().pending,Some(4));
        let current=take_decoded_frame(&app);assert!(app.accept_media_frame(current));assert_reference_frame(&app,4);
        let result=app.scripting.as_mut().unwrap().console_eval(&mut app.document,&mut app.forms,
            "video.currentTime=1.25;queueMicrotask(function(){throw 'stale seek job';});document.body.scrollWidth;");
        assert_eq!(result.errors,1);app.show_script_errors();
        let current=take_decoded_frame(&app);assert!(app.accept_media_frame(current));assert_reference_frame(&app,2);
        app.flush_embedded_video();
        assert_eq!(app.media.as_ref().unwrap().clock.position(Instant::now()),Duration::from_millis(1250));
    }

    #[test]
    fn script_current_time_native_play_waits_for_requested_bitmap() {
        let mut app=open("authored-video.html");app.wait_for_load();
        eval(&mut app,"mediaEvents=[];video.currentTime=1.25;");app.apply_media_pause();
        let frame=take_decoded_frame(&app);
        app.input_media_action("Play").unwrap();
        assert!(!app.media.as_ref().unwrap().clock.playing(),"native Play must not advance on the old bitmap");
        assert_eq!(app.media.as_ref().unwrap().seek_resume,Some(true));
        assert!(!app.media.as_ref().unwrap().paused);
        eval(&mut app,"if(mediaEvents.indexOf('playing')>=0)throw 'playing before requested data';");
        assert!(app.accept_media_frame(frame));assert_reference_frame(&app,2);
        assert!(app.media.as_ref().unwrap().clock.playing());
        app.flush_embedded_video();
        eval(&mut app,"if(video.paused||video.seeking||mediaEvents.indexOf('playing')<0)throw 'ready native play';");
    }

    #[test]
    fn script_current_time_default_data_events_wait_for_requested_frame() {
        let mut app=open("authored-video-default.html");
        eval(&mut app,"var firstData=0;video.onloadeddata=function(){if(video.readyState<2||video.seeking)throw 'loadeddata before requested frame';firstData++;};video.currentTime=1.75;");
        let deadline=Instant::now()+Duration::from_secs(5);
        let source=loop {
            if let Some(done)=app.embedded_video_worker.as_ref().unwrap().take(){break done;}
            assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(1));
        };
        assert!(app.accept_embedded_video(source));
        let frame=take_decoded_frame(&app);
        app.flush_embedded_video();
        assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
        eval(&mut app,"if(firstData!==0||mediaWindowLoaded)throw 'early data/window load';");
        assert!(app.accept_media_frame(frame));assert_reference_frame(&app,3);
        app.flush_embedded_video();
        eval(&mut app,"if(firstData!==1||!mediaWindowLoaded||video.currentTime!==1.75||video.seeking)throw 'ready default data';");
    }

    #[test]
    fn script_current_time_async_notification_capacity_fails_closed_and_reload_recovers() {
        let mut app=open("authored-video.html");app.wait_for_load();
        app.scripting.as_mut().unwrap().set_media_task_capacity_for_test(2);
        // One genuine older task occupies the front. The seek control itself
        // can be admitted, but later native notification storage may fail.
        app.media.as_mut().unwrap().event("timeupdate");
        eval(&mut app,"video.currentTime=1.25;if(!video.seeking)throw 'accepted control';");
        app.apply_media_pause();app.flush_embedded_video();
        let player=app.media.as_ref().unwrap();
        assert!(player.error.as_ref().is_some_and(|error|error.contains("allowance")));
        assert!(!player.decoder_enabled&&player.paused&&!player.clock.playing());
        assert!(player.pending.is_none()&&!player.seeking);
        assert!(!app.scripting.as_ref().unwrap().media_tasks().unwrap().lock().unwrap().has_pending());
        assert!(app.embedded_video.as_ref().unwrap().error.is_some());
        app.scripting.as_mut().unwrap().set_media_task_capacity_for_test(crate::javascript::limits::BROWSER.host.media_tasks);
        eval(&mut app,"video.load();");app.wait_for_load();
        assert!(app.media.as_ref().unwrap().decoder_enabled);
        assert!(app.media.as_ref().unwrap().error.is_none());
        eval(&mut app,"video.currentTime=0.7;");app.apply_media_pause();settle(&mut app);assert_reference_frame(&app,1);
        assert!(app.media.as_ref().unwrap().error.is_none());
    }

    fn activate_media_document(app:&mut App) {
        // Real App content press/release, outside browser chrome/native media
        // controls. No OS mouse/keyboard injection or synthetic DOM activation.
        let y=app.content_top()+5;app.pointer_down(40,y,false);app.pointer_up(40,y);
    }
    fn take_decoded_source(app:&App)->Completion {
        let deadline=Instant::now()+Duration::from_secs(5);
        loop {
            if let Some(source)=app.embedded_video_worker.as_ref().unwrap().take(){return source;}
            assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn script_play_promise_fulfills_after_real_playing_and_decoded_frame_time_advances() {
        let mut app=open("authored-video.html");app.wait_for_load();activate_media_document(&mut app);
        eval(&mut app,"var playState='pending',playOrder='',playSettles=0;video.addEventListener('playing',function(){playOrder+='playing;';});var playPromise=video.play({toString:function(){throw 'ignored argument';}});if(!(playPromise instanceof Promise)||video.paused)throw 'genuine play promise';playPromise.then(function(){playState='fulfilled';playSettles++;playOrder+='then;';},function(e){throw e;});if(playState!=='pending')throw 'synchronous fulfillment';");
        assert!(!app.media.as_ref().unwrap().clock.playing());
        app.flush_embedded_video();assert!(app.media.as_ref().unwrap().clock.playing());assert_reference_frame(&app,0);
        eval(&mut app,"if(playState!=='fulfilled'||playSettles!==1||playOrder!=='playing;then;'||video.paused)throw 'actual playing settlement';");
        run(&mut app,Duration::from_millis(650));
        let index=app.media.as_ref().unwrap().displayed;assert!(index>=1);assert_reference_frame(&app,index);
        assert!(app.media.as_ref().unwrap().position(Instant::now())>=Duration::from_millis(500));
        eval(&mut app,"video.pause();");app.flush_embedded_video();settle(&mut app);
        let frozen=app.media.as_ref().unwrap().clock.position(Instant::now());run(&mut app,Duration::from_millis(30));
        assert_eq!(app.media.as_ref().unwrap().clock.position(Instant::now()),frozen);
        assert!(!app.scripting.as_ref().unwrap().has_media_play_promises_for_test());
        eval(&mut app,"if(playSettles!==1||!video.paused)throw 'exact settlement/pause';");
    }

    #[test]
    fn script_play_loading_waits_for_real_default_frame_and_keeps_admission_after_expiry() {
        let mut app=open("authored-video-default.html");activate_media_document(&mut app);
        eval(&mut app,"var loadedPlay='pending',loadedSettles=0;video.currentTime=1.75;video.play().then(function(){loadedPlay='fulfilled';loadedSettles++;},function(e){throw e;});if(video.paused||video.readyState!==0)throw 'loading play projection';");
        app.flush_embedded_video();assert!(app.media.is_none());
        eval(&mut app,"if(loadedPlay!=='pending'||video.paused)throw 'loading play settled early';");
        app.scripting.as_mut().unwrap().expire_media_activation_for_test();
        assert!(app.accept_embedded_video(take_decoded_source(&app)));
        assert!(!app.media.as_ref().unwrap().clock.playing());
        let frame=take_decoded_frame(&app);app.flush_embedded_video();
        eval(&mut app,"if(loadedPlay!=='pending'||video.paused||!video.seeking)throw 'waiting decoded target';");
        assert!(app.accept_media_frame(frame));assert_reference_frame(&app,3);
        app.flush_embedded_video();assert!(app.media.as_ref().unwrap().clock.playing());
        eval(&mut app,"if(loadedPlay!=='fulfilled'||loadedSettles!==1||video.paused||video.seeking||video.currentTime<1.75)throw 'actual loading play';");
        assert!(!app.scripting.as_ref().unwrap().has_media_play_promises_for_test());
    }

    #[test]
    fn script_play_ready_pause_preserves_captured_fulfillment_but_loading_pause_rejects() {
        for ready in [false,true] {
            let mut app=open("authored-video-default.html");if ready {app.wait_for_load();}
            activate_media_document(&mut app);
            eval(&mut app,"var race='pending',raceOrder='';video.addEventListener('timeupdate',function(){raceOrder+='time;';});video.addEventListener('pause',function(){raceOrder+='pause;';});video.play().then(function(){race='fulfilled';raceOrder+='then;';},function(e){if(!(e instanceof DOMException)||e.name!=='AbortError')throw 'wrong interruption';race='rejected';raceOrder+='catch;';});video.pause();if(race!=='pending'||!video.paused)throw 'pause race immediate';");
            app.flush_embedded_video();
            if ready {eval(&mut app,"if(race!=='fulfilled'||raceOrder.indexOf('then;')<0||raceOrder.indexOf('catch;')>=0)throw 'captured ready outcome';");}
            else {eval(&mut app,"if(race!=='rejected'||raceOrder.indexOf('time;pause;catch;')<0)throw 'pending pause outcome '+raceOrder;");}
            assert!(!app.scripting.as_ref().unwrap().has_media_play_promises_for_test());
            assert!(app.media.as_ref().is_none_or(|player|!player.clock.playing()));
        }
    }

    #[test]
    fn script_play_load_settles_queued_outcomes_before_selection_and_does_not_replay() {
        let mut app=open("authored-video.html");app.wait_for_load();activate_media_document(&mut app);
        eval(&mut app,"var resetOrder='',resetSettles=0;video.play().then(function(){if(video.networkState!==3)throw 'load settlement ran after selection';resetOrder+='then;';resetSettles++;});video.load();queueMicrotask(function(){if(video.networkState!==2)throw 'stable selection missing';resetOrder+='after;';});");
        eval(&mut app,"if(resetOrder!=='then;after;'||resetSettles!==1||!video.paused)throw 'load queued outcome '+resetOrder;");
        app.wait_for_load();assert!(!app.media.as_ref().unwrap().clock.playing());
        assert!(!app.scripting.as_ref().unwrap().has_media_play_promises_for_test());
        eval(&mut app,"if(resetSettles!==1)throw 'duplicate reset settlement';");
    }

    #[test]
    fn script_play_fatal_started_listener_defers_native_settlement_without_stale_callbacks() {
        let mut app=open("authored-video.html");app.wait_for_load();activate_media_document(&mut app);
        eval(&mut app,"var fatalPlaySettles=0,fatalStale=0,freshPlaySettles=0;video.addEventListener('playing',function(){queueMicrotask(function(){fatalStale++;});document.body.scrollWidth;});var fatalPromise=video.play();fatalPromise.then(function(){fatalPlaySettles++;},function(e){throw e;});");
        app.flush_embedded_video();assert!(app.media.as_ref().unwrap().clock.playing());
        assert!(app.scripting.as_ref().unwrap().has_media_promise_effects());
        assert_eq!(app.scripting.as_ref().unwrap().error_count(),1);
        // Existing fatal-job policy discards old reactions as well as queued
        // callbacks. A fresh observer proves the genuine deferred outcome.
        eval(&mut app,"if(fatalPlaySettles!==0||fatalStale!==0)throw 'stale fatal observers';fatalPromise.then(function(){freshPlaySettles++;},function(e){throw e;});");
        app.process_script_requests();
        assert!(!app.scripting.as_ref().unwrap().has_media_play_promises_for_test());
        eval(&mut app,"if(freshPlaySettles!==1||fatalPlaySettles!==0||fatalStale!==0)throw 'fatal task settlement cleanup';");
    }

    #[test]
    fn script_play_rewinds_eof_and_real_cached_frame_precedes_replay_fulfillment() {
        let mut app=open("authored-video.html");app.wait_for_load();
        eval(&mut app,"video.currentTime=3;");app.apply_media_pause();settle(&mut app);assert_reference_frame(&app,5);
        activate_media_document(&mut app);
        eval(&mut app,"var replayPlay='pending';video.play().then(function(){replayPlay='fulfilled';},function(e){throw e;});if(video.paused||video.ended||video.currentTime!==0)throw 'replay synchronous intent';");
        app.flush_embedded_video();assert_reference_frame(&app,0);
        assert!(app.media.as_ref().unwrap().clock.playing());
        eval(&mut app,"if(replayPlay!=='fulfilled'||video.paused||video.ended||video.seeking)throw 'actual replay promise';");
        assert!(!app.scripting.as_ref().unwrap().has_media_play_promises_for_test());
    }

    #[test]
    fn script_play_started_playing_load_preserves_queued_and_started_outcomes_in_order() {
        let mut app=open("authored-video.html");app.wait_for_load();activate_media_document(&mut app);
        eval(&mut app,"var startedOrder='',startedLoads=0;video.addEventListener('playing',function(){if(startedLoads++===0)video.load();});video.play().then(function(){startedOrder+='first;';},function(e){throw e;});video.play().then(function(){startedOrder+='second;';},function(e){throw e;});");
        app.flush_embedded_video();
        eval(&mut app,"if(startedOrder!=='second;first;'||startedLoads!==1||!video.paused)throw 'started versus queued ownership '+startedOrder;");
        app.wait_for_load();assert!(!app.media.as_ref().unwrap().clock.playing());
        assert!(!app.scripting.as_ref().unwrap().has_media_play_promises_for_test());
    }

    #[test]
    fn script_play_unsupported_source_rejects_real_promise_after_actual_file_failure() {
        let mut app=open("authored-video.html");app.wait_for_load();activate_media_document(&mut app);
        eval(&mut app,"var unsupportedPlay='pending',unsupportedSettles=0;video.src='missing-play-source.ivf';video.play().then(function(){throw 'unsupported fulfilled';},function(e){if(!(e instanceof DOMException)||e.name!=='NotSupportedError')throw 'source rejection reason';unsupportedPlay='rejected';unsupportedSettles++;});");
        app.wait_for_load();app.process_script_requests();
        assert!(app.media.is_none());assert!(app.embedded_video.as_ref().unwrap().error.is_some());
        eval(&mut app,"if(unsupportedPlay!=='rejected'||unsupportedSettles!==1||mediaEvents.indexOf('error')<0)throw 'actual unsupported source settlement';");
        assert!(!app.scripting.as_ref().unwrap().has_media_play_promises_for_test());
    }

    #[test]
    fn script_play_stop_while_loading_rejects_once_and_discards_real_late_source() {
        let mut app=open("authored-video-default.html");activate_media_document(&mut app);
        eval(&mut app,"var stoppedPlay='pending',stoppedSettles=0;video.play().then(function(){throw 'stopped loading fulfilled';},function(e){if(!(e instanceof DOMException)||e.name!=='AbortError')throw 'stop reason';stoppedPlay='rejected';stoppedSettles++;});");
        app.flush_embedded_video();let late=take_decoded_source(&app);
        app.stop_loading();app.process_script_requests();
        assert!(!app.accept_embedded_video(late));assert!(app.media.is_none());
        eval(&mut app,"if(stoppedPlay!=='rejected'||stoppedSettles!==1)throw 'stop cleanup';");
        assert!(!app.scripting.as_ref().unwrap().has_media_play_promises_for_test());
    }

    #[test]
    fn script_play_real_decode_error_uses_explicit_pause_and_load_recovers() {
        let mut app=open("authored-video.html");app.wait_for_load();
        // Keep the independent first frame, but truncate a later compressed
        // packet. The actual worker must discover the real decoder failure.
        let source=include_bytes!("../tools/fixtures/scene-intra.ivf");
        let mut bytes=source[..32].to_vec();let mut offset=32;let mut index=0;
        while offset<source.len() {
            let size=u32::from_le_bytes(source[offset..offset+4].try_into().unwrap()) as usize;
            let count=if index==2 {10}else{size};
            bytes.extend_from_slice(&(count as u32).to_le_bytes());
            bytes.extend_from_slice(&source[offset+4..offset+12]);
            bytes.extend_from_slice(&source[offset+12..offset+12+count]);
            offset+=12+size;index+=1;
        }
        let asset=media::Clip::load(bytes,media::Limits::default(),&||Ok(())).unwrap();
        let node=app.media.as_ref().unwrap().node;app.install_media(Some(asset),Some(node),true);
        app.flush_embedded_video();
        app.media_seek(Duration::from_millis(1200),Instant::now());settle(&mut app);
        assert!(app.media.as_ref().unwrap().error.is_some());assert_reference_frame(&app,0);
        assert_eq!(app.embedded_video_snapshot().unwrap().error_code,Some(3));
        activate_media_document(&mut app);
        eval(&mut app,"var decodePlay='pending',decodeOrder='',decodeSettles=0;video.addEventListener('timeupdate',function(){decodeOrder+='time;';});video.addEventListener('pause',function(){decodeOrder+='pause;';});var decodePromise=video.play();if(!(decodePromise instanceof Promise)||video.paused)throw 'decoder admission';decodePromise.then(function(){throw 'decoder fulfilled';},function(e){if(!(e instanceof DOMException)||e.name!=='AbortError')throw 'explicit decoder pause reason';decodePlay='rejected';decodeSettles++;decodeOrder+='catch;';});if(decodePlay!=='pending')throw 'numeric error rejected synchronously';");
        app.flush_embedded_video();
        // Observation queues the explicit internal Pause; the next normal
        // control drain applies it before its grouped task can dispatch.
        app.flush_embedded_video();
        eval(&mut app,"if(decodePlay!=='rejected'||decodeSettles!==1||decodeOrder!=='time;pause;catch;'||!video.paused)throw 'decoder pause sequence '+decodeOrder;");
        assert_reference_frame(&app,0);assert!(!app.media.as_ref().unwrap().clock.playing());
        assert!(!app.scripting.as_ref().unwrap().has_media_play_promises_for_test());
        eval(&mut app,"video.load();");app.wait_for_load();activate_media_document(&mut app);
        eval(&mut app,"var recoveredPlay=0;video.play().then(function(){recoveredPlay++;},function(e){throw e;});");
        app.flush_embedded_video();
        eval(&mut app,"if(recoveredPlay!==1||decodeSettles!==1||video.paused)throw 'decoder reload recovery';");
        assert!(app.media.as_ref().unwrap().clock.playing());assert!(app.media.as_ref().unwrap().error.is_none());
    }

    #[test]
    fn script_play_rewinds_measured_eof_before_timer_reports_ended() {
        let mut app=open("authored-video.html");app.wait_for_load();
        eval(&mut app,"video.currentTime=2.75;");app.apply_media_pause();settle(&mut app);assert_reference_frame(&app,5);
        let now=Instant::now();let past=now-Duration::from_secs(1);
        {
            let p=app.media.as_mut().unwrap();
            p.clock.seek(Duration::from_millis(2750),past);p.clock.play(past);
            p.paused=false;p.ended_reported=false;p.dom_playback_started=true;
        }
        let s=app.embedded_video_snapshot().unwrap();
        assert_eq!(s.position,Duration::from_secs(3));assert!(s.playing&&!s.paused&&!s.ended&&s.frame_ready);
        app.scripting.as_mut().unwrap().set_embedded_video(Some(s));activate_media_document(&mut app);
        eval(&mut app,"var edgePlay='pending',edgePlayEvents=0;video.addEventListener('play',function(){edgePlayEvents++;});video.play().then(function(){edgePlay='fulfilled';},function(e){throw e;});if(edgePlay!=='pending'||video.paused||video.currentTime!==0||!video.seeking)throw 'pre-tick rewind admission';");
        app.flush_embedded_video();assert_reference_frame(&app,0);
        assert!(app.media.as_ref().unwrap().clock.playing());
        eval(&mut app,"if(edgePlay!=='fulfilled'||video.paused||video.seeking||video.ended||edgePlayEvents!==0||video.currentTime>0.2)throw 'pre-tick real rewind';");
        assert!(!app.scripting.as_ref().unwrap().has_media_play_promises_for_test());
    }
    include!("app_network_media_tests.rs");
    include!("app_cors_media_tests.rs");
    include!("app_interframe_tests.rs");
}
