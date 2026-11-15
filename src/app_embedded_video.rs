//! Document-owned source/element lifecycle. All callbacks execute on the UI thread.
use super::*;
use crate::embedded_video::{Completion, Plan, Snapshot};
impl App {
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
            events: VecDeque::new(),
            last_update: Duration::ZERO,
        };
        if selection.abort {
            binding.events.push_back("abort");
        }
        if selection.emptied {
            binding.events.push_back("emptied");
        }
        if selection.timeupdate {
            binding.events.push_back("timeupdate");
        }
        if selection.raw.is_some() || selection.error.is_some() {
            binding.network = 2;
            binding.events.push_back("loadstart");
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
                binding.error = Some(error);
                binding.network = 3;
                binding.events.push_back("error");
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
            (v.plan.is_some() || !v.aborted && !v.events.is_empty())
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
            if v.plan.take().is_some() {
                v.aborted = true;
                v.network = 1;
                v.events.push_back("abort");
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
            events: VecDeque::from(["loadstart"]),
            last_update: Duration::ZERO,
        };
        let result = Plan::new(
            node,
            &self.document,
            &self.location,
            &self.base_location,
            &self.document_policy,
            self.media_limits,
        )
        .and_then(|plan| {
            if self.embedded_video_worker.is_none() {
                self.embedded_video_worker =
                    Some(crate::embedded_video::Worker::new(self.wake.clone())?);
            }
            let plan = Arc::new(plan);
            binding.retired = plan.retired.clone();
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
            binding.error = Some(error);
            binding.network = 3;
            binding.events.push_back("error");
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
            retired: v.retired.clone(),
            node: v.node,
            current_src: v.source.clone(),
            width: 0,
            height: 0,
            duration: None,
            position: Duration::ZERO,
            observed: now,
            playing: false,
            paused: true,
            ended: false,
            seeking: false,
            ready: 0,
            network: v.network,
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
            s.playing = p.playing();
            s.paused = p.paused;
            s.ended = p.ended_reported;
            s.seeking = p.seeking;
            s.ready = if p.error.is_some() || p.seeking && p.pending.is_some() {
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
            v.events.push_back("abort");
            return true;
        }
        let node = v.node;
        v.network = 1;
        match completion.result {
            Ok(asset) => {
                self.install_media(Some(asset), Some(node), true);
                self.reflow();
            }
            Err(error) => {
                self.events.push(format!("VIDEO node {node}: {error}"));
                let v = self.embedded_video.as_mut().unwrap();
                v.error = Some(error);
                v.network = 3;
                v.events.push_back("error");
            }
        }
        true
    }
    pub(in crate::app) fn apply_media_pause(&mut self) -> bool {
        let Some(request)=self.scripting.as_ref().and_then(|p|p.media_pause()).filter(|p|!p.applied).cloned() else {return false;};
        let valid=!request.retired.load(Ordering::Relaxed) && self.embedded_video.as_ref().is_some_and(|v|v.node==request.node&&Arc::ptr_eq(&v.retired,&request.retired));
        if valid {
            if let Some(player)=self.media.as_mut().filter(|p|p.embedded&&p.node==request.node&&p.sync.is_none()) {
                player.clock.pause(request.issued);
                player.paused=true;player.dom_playback_started=false;
                // Keep pending decode/seek/generation/source intact. The snapshot
                // issue time was sampled from this clock before any later tick.
                debug_assert_eq!(player.clock.position(request.issued),request.position);
                if player.pending.is_none() {player.deadline=None;}
            }
            self.scripting.as_mut().unwrap().mark_media_pause_applied();
        } else {self.scripting.as_mut().unwrap().cancel_media_pause();}
        valid
    }
    pub(in crate::app) fn flush_embedded_video(&mut self) -> bool {
        let paused=self.apply_media_pause();
        let processed = self.process_media_selection() | paused;
        if self.embedded_video_retired() {
            return false;
        }
        let Some(v) = &mut self.embedded_video else {
            return false;
        };
        let node = v.node;
        let resource = v.retired.clone();
        let mut events = std::mem::take(&mut v.events);
        if let Some(p) = self.media.as_mut().filter(|p| p.embedded && p.node == node) {
            events.append(&mut p.dom_events);
            let position = p.position(Instant::now());
            if position.abs_diff(v.last_update) >= Duration::from_millis(250) {
                events.push_back("timeupdate");
                v.last_update = position;
            }
        }
        let finish = v.plan.is_none() && !v.aborted;
        let snapshot = self.embedded_video_snapshot();
        let mut changed = processed;
        if let Some(page) = &mut self.scripting {
            page.set_embedded_video(snapshot);
        }
        for event in events {
            if resource.load(Ordering::Relaxed) {
                break;
            }
            if let Some(page) = &mut self.scripting {
                let outcome = page.dispatch_media(&mut self.document, &mut self.forms, node, event);
                if outcome.layout_changed {
                    self.reflow();
                }
                changed = true;
            }
        }
        if let Some(request)=self.scripting.as_mut().and_then(|p|p.take_media_pause_task()) {
            if !request.retired.load(Ordering::Relaxed) && !self.script_jobs_paused() {
                if let Some(page)=&mut self.scripting {
                    let outcome=page.dispatch_media_task(&mut self.document,&mut self.forms,node,crate::scripting::MediaTask::Pause);
                    if outcome.layout_changed {self.reflow();}
                    changed=true;
                }
            }
        }
        if finish && !resource.load(Ordering::Relaxed) {
            if let Some(page) = &mut self.scripting {
                let outcome = page.finish_loading(&mut self.document, &mut self.forms);
                if outcome.layout_changed {
                    self.reflow();
                }
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
        eval(&mut app,"if(initialMediaState!==0||!mediaWindowLoaded||video.readyState!==4||video.networkState!==1||video.currentTime!==0||mediaEvents.slice(0,6).join(',')!=='loadstart,durationchange,loadedmetadata,loadeddata,canplay,canplaythrough')throw 'actual initial sequence';video.duration=0;if(video.duration!==3)throw 'readonly duration';var absentCall=false;try{video.play();}catch(e){absentCall=e instanceof TypeError;}if(!absentCall||typeof onplay!=='undefined'||window.onloadedmetadata!==undefined)throw 'absent calls/window scope';var propertyEvents=0;video.onpause=function(e){if(e.target!==video||!e.isTrusted)throw 'native handler';propertyEvents++;};if(!('onpause' in video)||!('onerror' in video))throw 'native handler presence';");
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
}
