//! Measured state, real reload control and native events for the bound video.
use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
pub(crate) use crate::media_tasks::Task as MediaTask;
#[derive(Clone)]
pub(crate) struct PauseRequest {
    pub node: usize,
    pub retired: Arc<AtomicBool>,
    pub issued: Instant,
    pub position: Duration,
    pub applied: bool,
}
struct Prepared {
    ticket: u64,
    retired: Arc<AtomicBool>,
}
pub(super) struct Pending {
    ticket: u64,
    abort: bool,
    emptied: bool,
    timeupdate: bool,
}
#[derive(Default)]
pub(super) struct Changes {
    // Official position is sampled once per execution job. Physical time may
    // continue to advance, but repeated script reads cannot observe wall time
    // passing within the same script/listener microtask checkpoint.
    official_position: f64,
    default_start: f64,
    next: u64,
    prepared: Option<Prepared>,
    pub(super) pending: Option<Pending>,
    ready: Option<crate::embedded_video::Selection>,
    // Accepted controls/tasks share the bound player's source-scoped FIFO.
    // Request admission lives in the actual Snapshot task handle.
}
pub(crate) fn event_name(name: &str) -> bool {
    matches!(
        name,
        "loadstart"
            | "abort"
            | "error"
            | "loadedmetadata"
            | "durationchange"
            | "loadeddata"
            | "canplay"
            | "canplaythrough"
            | "play"
            | "playing"
            | "pause"
            | "seeking"
            | "seeked"
            | "timeupdate"
            | "ended"
            | "emptied"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page(mut limits: crate::javascript::limits::Limits) -> (Document, Forms, Page, usize) {
        limits.startup_steps = 100000;
        let mut doc = crate::html::parse("<video id='v' src='scene-intra.ivf'></video>");
        doc.scripting_enabled = true;
        let node = crate::embedded_video::authored_node(&doc).unwrap();
        let mut forms = Forms::new(&doc);
        let mut p = Page::with_limits(limits);
        let base = crate::navigation::explicit(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tools/fixtures/authored-video.html"),
        )
        .unwrap();
        p.set_location(&base);
        p.set_resource_base(&base);
        p.set_embedded_video(Some(crate::embedded_video::Snapshot {
            tasks: crate::media_tasks::shared(limits.host.media_tasks),
            node,
            retired: Arc::new(AtomicBool::new(false)),
            current_src: "previous".into(),
            width: 64,
            height: 48,
            duration: Some(Duration::from_secs(3)),
            position: Duration::ZERO,
            observed: Instant::now(),
            playing: false,
            paused: true,
            load_pending: false,
            ended: false,
            seeking: false,
            ready: 4,
            network: 1,
        }));
        let out = p.start(&mut doc, &mut forms, &[]);
        assert_eq!(out.errors, 0, "{}", p.report());
        let out = p.console_eval(
            &mut doc,
            &mut forms,
            "var video=document.getElementById('v');",
        );
        assert_eq!(out.errors, 0, "{}", p.report());
        (doc, forms, p, node)
    }
    #[test]
    fn native_media_load_preserves_raw_attr_and_uses_real_fifo_selection() {
        let(mut doc,mut forms,mut p,node)=page(crate::javascript::limits::BROWSER);
        let s=p.state.embedded_video.as_mut().unwrap();s.position=Duration::from_secs(1);s.playing=true;
        let retired=s.retired.clone();let commits=p.state.dom_commits;
        let out=p.console_eval(&mut doc,&mut forms,r#"
var attr=video.getAttributeNode('src'),raw=attr.value,loadMethod=video.load,order='';
if(typeof loadMethod!=='function'||loadMethod!==video.load||loadMethod.length!==0||!('load' in video))throw 'method identity';
queueMicrotask(function(){if(video.networkState!==3||video.currentSrc!=='previous'||video.readyState!==0)throw 'before selection';order+='before;';});
if(video.load({toString:function(){throw 'ignored';}})!==undefined||video.getAttributeNode('src')!==attr||attr.value!==raw||!video.paused||video.currentTime!==0||!isNaN(video.duration)||video.readyState!==0||video.seeking||video.ended)throw 'synchronous reset';
queueMicrotask(function(){if(video.networkState!==2||video.currentSrc.indexOf('scene-intra.ivf')<0||video.getAttributeNode('src')!==attr)throw 'after selection';order+='after;';});
"#);assert_eq!(out.errors,0,"{}",p.report());assert!(retired.load(Ordering::Relaxed));assert_eq!(doc.attr(node,"src"),Some("scene-intra.ivf"));assert_eq!(p.state.dom_commits,commits);
        let selection=p.take_media_selection().unwrap();assert!(selection.abort&&selection.emptied&&selection.timeupdate);assert_eq!(selection.raw.as_deref(),Some("scene-intra.ivf"));
        let out=p.console_eval(&mut doc,&mut forms,"if(order!=='before;after;')throw 'FIFO';var wrong=0;try{loadMethod.call({});}catch(e){if(e instanceof TypeError)wrong++;}try{new loadMethod();}catch(e){if(e instanceof TypeError)wrong++;}if(wrong!==2)throw 'brand/new';video.load();video.load();");assert_eq!(out.errors,0,"{}",p.report());assert!(p.take_media_selection().is_some());assert_eq!(p.state.dom_commits,commits);
    }
    #[test]
    fn native_media_queue_quota_and_identity_overflow_reject_before_mutation() {
        for operation in ["video.src='scene-replacement.ivf';","video.load();"] {
        for overflow in [false, true] {
            let mut limits = crate::javascript::limits::CAPACITY_REGRESSION;
            if !overflow {
                limits.capacity.promises = 0;
            }
            let (mut doc, mut forms, mut p, node) = page(limits);
            if overflow {
                p.state.media_changes.next = u64::MAX;
            }
            let token = p.state.embedded_video.as_ref().unwrap().retired.clone();
            let out = p.console_eval(&mut doc, &mut forms, operation);
            assert_eq!(out.errors, 1, "{}", p.report());
            assert_eq!(doc.attr(node, "src"), Some("scene-intra.ivf"));
            assert!(!token.load(Ordering::Relaxed));
            assert!(!p.has_media_selection());
            assert!(!p.runtime.has_microtasks());
            assert!(p.state.media_changes.prepared.is_none());
        }
        }
    }
    #[test]
    fn native_media_fatal_execute_and_callback_cancel_pending_and_allow_recovery() {
        for callback in [false, true] {
            let mut limits = crate::javascript::limits::CAPACITY_REGRESSION;
            limits.steps = 4000;
            let (mut doc, mut forms, mut p, _) = page(limits);
            let script = if callback {
                "queueMicrotask(function(){video.src='scene-replacement.ivf';while(true){}});"
            } else {
                "video.src='scene-replacement.ivf';while(true){}"
            };
            let out = p.console_eval(&mut doc, &mut forms, script);
            assert_eq!(out.errors, 1, "{}", p.report());
            assert!(!p.has_media_selection());
            assert!(!p.runtime.has_microtasks());
            let out = p.console_eval(&mut doc, &mut forms, "video.src='scene-intra.ivf';");
            assert_eq!(out.errors, 0, "{}", p.report());
            assert_eq!(
                p.take_media_selection().unwrap().raw.as_deref(),
                Some("scene-intra.ivf")
            );
        }
    }
    #[test]
    fn native_media_same_owned_attr_needs_no_job_even_when_queue_full() {
        let mut limits = crate::javascript::limits::CAPACITY_REGRESSION;
        limits.capacity.promises = 1;
        let (mut doc, mut forms, mut p, node) = page(limits);
        let out=p.console_eval(&mut doc,&mut forms,"var ran=0,a=video.getAttributeNode('src');queueMicrotask(function(){ran++;});if(video.setAttributeNode(a)!==a)throw 'same owned identity';");
        assert_eq!(out.errors, 0, "{}", p.report());
        assert_eq!(doc.attr(node, "src"), Some("scene-intra.ivf"));
        assert!(!p.has_media_selection());
        let out = p.console_eval(
            &mut doc,
            &mut forms,
            "if(ran!==1)throw 'earlier callback lost';",
        );
        assert_eq!(out.errors, 0, "{}", p.report());
    }
    #[test]
    fn native_media_copy_budget_accounts_for_long_base_before_selection() {
        let (mut doc, mut forms, mut p, _) = page(crate::javascript::limits::CAPACITY_REGRESSION);
        let base = crate::navigation::address(
            &format!("https://example.test/{}/page", "long".repeat(1000)),
            &crate::navigation::Location::home(),
        )
        .unwrap();
        p.set_resource_base(&base);
        p.state.limits.copy_bytes = 10000;
        let out = p.console_eval(&mut doc, &mut forms, "video.src='x';");
        assert_eq!(out.errors, 0, "{}", p.report());
        let selection = p.take_media_selection().unwrap();
        assert!(selection.uri.is_none());
        assert!(selection.error.unwrap().contains("string work"));
    }
}
impl Page {
    pub(super) fn refresh_media_official_position(&mut self) {
        self.state.media_changes.official_position = self.state.embedded_video
            .as_ref().map_or(0.0, |snapshot| snapshot.position().as_secs_f64());
    }

    /// Metadata processing resets the default even when it is negative. Only a
    /// positive default initiates actual seeking on the matching accepted source.
    pub(crate) fn take_media_default_start(&mut self, retired: &Arc<AtomicBool>) -> Option<f64> {
        if !self.state.embedded_video.as_ref().is_some_and(|snapshot|Arc::ptr_eq(&snapshot.retired,retired)) {
            return None;
        }
        let value=std::mem::replace(&mut self.state.media_changes.default_start,0.0);
        (value>0.0).then_some(value)
    }

    #[cfg(test)]
    pub(crate) fn set_media_task_capacity_for_test(&mut self, limit:usize) {
        self.state.limits.host.media_tasks=limit;
        if let Some(tasks)=self.media_tasks() {tasks.lock().unwrap().set_limit(limit);}
    }

    pub(crate) fn media_tasks(&self) -> Option<crate::media_tasks::Shared> {
        self.state.embedded_video.as_ref().map(|snapshot|snapshot.tasks.clone())
    }
    pub(crate) fn has_media_tasks(&self) -> bool {
        self.media_tasks().is_some_and(|tasks|tasks.lock().map_or(true,|queue|queue.has_pending()))
    }
    pub(crate) fn media_pause(&self) -> Option<PauseRequest> {
        let tasks=self.media_tasks()?;
        let (_,scope,issued,position,applied)=tasks.lock().ok()?.peek_pause()?;
        Some(PauseRequest {node:scope.node,retired:scope.retired,issued,position,applied})
    }
    #[cfg(test)]
    pub(crate) fn mark_media_pause_applied(&mut self) {
        if let Some(tasks)=self.media_tasks() {tasks.lock().unwrap().take_control();}
    }
    #[cfg(test)]
    pub(crate) fn take_media_pause_task(&mut self) -> Option<PauseRequest> {
        let request=self.media_pause().filter(|request|request.applied)?;
        let tasks=self.media_tasks()?;
        let (_,scope,task)=tasks.lock().ok()?.take_task()?;
        assert!(matches!(task,MediaTask::Pause));
        assert!(Arc::ptr_eq(&scope.retired,&request.retired));
        Some(request)
    }
    pub(crate) fn cancel_media_pause(&mut self) {
        if let Some(snapshot)=self.state.embedded_video.as_ref() {
            if let Ok(mut tasks)=snapshot.tasks.lock() {tasks.cancel_source(&snapshot.retired);}
        }
    }
    pub fn take_media_selection(&mut self) -> Option<crate::embedded_video::Selection> {
        self.state.media_changes.ready.take()
    }
    pub fn has_media_selection(&self) -> bool {
        self.state.media_changes.pending.is_some() || self.state.media_changes.ready.is_some()
    }
    pub fn cancel_media_selection(&mut self) {
        if self.has_media_selection() {
            if let Some(s) = &mut self.state.embedded_video {
                s.retired.store(true, Ordering::Relaxed);
                s.network = 1;
            }
        }
        self.state.media_changes.pending = None;
        self.state.media_changes.ready = None;
        self.state.media_changes.prepared = None;
    }
    pub(crate) fn dispatch_media_task(&mut self, doc:&mut Document, forms:&mut Forms, node:usize, task:MediaTask) -> Outcome {
        let event=match task {MediaTask::Single(event)=>event,MediaTask::Pause=>"timeupdate"};
        if !self.state.enabled
            || !doc.scripting_enabled
            || !event_name(event)
            || !self
                .state
                .embedded_video
                .as_ref()
                .is_some_and(|s| s.node == node)
            || !doc.nodes.get(node).is_some_and(|n| {
                n.namespace == crate::html::Namespace::Html
                    && matches!(&n.kind,NodeKind::Element {tag,..} if tag=="video")
            })
        {
            return Outcome::default();
        }
        self.begin_job();
        let before = self.state.errors;
        self.dispatch_to(doc, forms, node_handle(node), event, false, false);
        // Source selection may reset this same element during timeupdate or its
        // checkpoint. A started task owns its tail; no App work runs in between.
        if matches!(task,MediaTask::Pause) && self.state.enabled && doc.scripting_enabled
            && !self.runtime.invocation_failed_fatally() {
            self.dispatch_to(doc, forms, node_handle(node), "pause", false, false);
        }
        Outcome {
            prevented: false,
            layout_changed: std::mem::take(&mut self.state.changed),
            errors: self.state.errors.saturating_sub(before),
        }
    }
}
impl Bridge<'_> {
    pub(super) fn prepares_media_selection(
        &self,
        object: u32,
        member: &str,
        args: &[Value],
        setter: bool,
    ) -> bool {
        let source_attr = |value: &Value| {
            script_attr::index(match value {
                Value::Host(id) => *id,
                _ => return false,
            })
            .and_then(|i| self.state.attributes.get(i))
            .is_some_and(|attr| {
                attr.name.html_is("src")
                    && attr.owner != node_index(script_named_map::element(object).unwrap_or(object))
            })
        };
        if setter {
            if self.media_element_receiver(object) && member == "src" {
                return true;
            }
            return matches!(member, "value" | "nodeValue" | "textContent")
                && script_attr::index(object)
                    .and_then(|i| self.state.attributes.get(i))
                    .is_some_and(|attr| {
                        attr.name.html_is("src")
                            && attr
                                .owner
                                .is_some_and(|node| self.media_element_receiver(node_handle(node)))
                    });
        }
        let target = script_named_map::element(object).unwrap_or(object);
        if !self.media_element_receiver(target) {
            return false;
        }
        (match member {
            "load" => true,
            "pause" => self.state.embedded_video.as_ref().is_some_and(|s|s.network==0),
            "setAttribute" => {
                matches!(args.first(),Some(Value::String(name)) if name.eq_ignore_ascii_case("src"))
            }
            "setAttributeNS" => {
                matches!(args.first(), Some(Value::Null | Value::Undefined))
                    || matches!(args.first(),Some(Value::String(name)) if name.is_empty())
            }
            "setAttributeNode" | "setAttributeNodeNS" => {
                args.first().is_some_and(source_attr)
            }
            "toggleAttribute" => {
                matches!(args.first(),Some(Value::String(name)) if name.eq_ignore_ascii_case("src"))
                    && node_index(target).is_some_and(|node| self.doc.attr(node, "src").is_none())
                    && args.get(1).is_none_or(Value::truthy)
            }
            _ => false,
        }) && (member != "setAttributeNS"
            || matches!(args.get(1),Some(Value::String(name)) if name=="src"))
    }
    pub(super) fn prepare_media_microtask(
        &mut self,
        object: u32,
        member: &str,
        args: &[Value],
        setter: bool,
    ) -> Result<Option<u64>, String> {
        if !self.prepares_media_selection(object, member, args, setter) {
            return Ok(None);
        }
        self.state.work(1)?;
        let ticket = self
            .state
            .media_changes
            .next
            .checked_add(1)
            .ok_or("Media selection identity exhausted.")?;
        self.state.media_changes.next = ticket;
        self.state.media_changes.prepared = Some(Prepared {
            ticket,
            retired: Arc::new(AtomicBool::new(false)),
        });
        Ok(Some(ticket))
    }
    pub(super) fn finish_media_microtask(&mut self, ticket: u64) -> bool {
        if self
            .state
            .media_changes
            .prepared
            .as_ref()
            .is_some_and(|p| p.ticket == ticket)
        {
            self.state.media_changes.prepared = None;
        }
        self.state
            .media_changes
            .pending
            .as_ref()
            .is_some_and(|p| p.ticket == ticket)
    }
    pub(super) fn media_source_prepared(&self) -> bool {
        self.state.media_changes.prepared.is_some()
    }
    pub(super) fn commit_media_source_change(&mut self, node: usize) -> Result<(), String> {
        let prepared = self
            .state
            .media_changes
            .prepared
            .take()
            .ok_or("Media source mutation lacks its reserved stable-state job.")?;
        let snapshot = self
            .state
            .embedded_video
            .as_mut()
            .filter(|s| s.node == node)
            .ok_or("Media source lost its element binding.")?;
        let pending = Pending {
            ticket: prepared.ticket,
            abort: matches!(snapshot.network, 1 | 2),
            emptied: snapshot.network != 0,
            timeupdate: snapshot.position() != Duration::ZERO,
        };
        if let Ok(mut tasks)=snapshot.tasks.lock() {tasks.cancel_source(&snapshot.retired);}
        snapshot.retired.store(true, Ordering::Relaxed);
        snapshot.retired = prepared.retired;
        snapshot.width = 0;
        snapshot.height = 0;
        snapshot.duration = None;
        snapshot.position = Duration::ZERO;
        snapshot.observed = Instant::now();
        snapshot.playing = false;
        snapshot.paused = true;
        snapshot.ended = false;
        snapshot.seeking = false;
        snapshot.ready = 0;
        snapshot.network = 3;
        self.state.media_changes.official_position = 0.0;
        self.state.media_changes.default_start = 0.0;
        self.state.media_changes.ready = None;
        // The new source keeps this Document's same queue/order domain;
        // old queued entries have lost their source authority.
        self.state.media_changes.pending = Some(pending);
        Ok(())
    }
    pub(super) fn run_media_microtask(&mut self, ticket: u64) -> Result<(), String> {
        if !self
            .state
            .media_changes
            .pending
            .as_ref()
            .is_some_and(|p| p.ticket == ticket)
        {
            return Ok(());
        }
        let pending = self.state.media_changes.pending.take().unwrap();
        let Some(snapshot) = self.state.embedded_video.as_ref() else {
            return Ok(());
        };
        if snapshot.retired.load(Ordering::Relaxed) {
            return Ok(());
        }
        let node = snapshot.node;
        let retired = snapshot.retired.clone();
        let raw = self.doc.attr(node, "src");
        let mut error = dom_compat::url_copy_cost(raw.unwrap_or(""), self.state.dynamic.base.as_ref())
            .and_then(|cost| self.state.copies(cost))
            .err();
        let raw = if error.is_none() {
            self.doc.attr(node, "src").map(str::to_owned)
        } else {
            None
        };
        if raw.is_none()
            && self.doc.nodes[node].children.iter().any(|n| {
                self.doc.nodes.get(*n).is_some_and(|n| {
                    n.namespace == crate::html::Namespace::Html
                        && matches!(&n.kind,NodeKind::Element {tag,..} if tag=="source")
                })
            })
        {
            error = Some("Media source children are not implemented in this element slice.".into());
        }
        let mut uri = None;
        if error.is_none() {
            if let Some(source) = raw.as_ref().filter(|s| !s.is_empty()) {
                let base = self.state.dynamic.base.as_ref();
                match base
                    .ok_or_else(|| "Media URL selection has no accepted document base.".to_owned())
                    .and_then(|base| crate::navigation::selection_url(source, base))
                {
                    Ok(value) => {
                        if value.len() > self.state.limits.host.string_bytes {
                            error = Some(
                                "Selected media URL exceeds the configured string allowance."
                                    .into(),
                            );
                        } else {
                            uri = Some(value);
                        }
                    }
                    Err(reason) => error = Some(reason),
                }
            } else if raw.as_ref().is_some_and(String::is_empty) {
                error = Some("The selected media src is empty.".into());
            }
        }
        let snapshot = self.state.embedded_video.as_mut().unwrap();
        snapshot.network = if raw.is_none() && error.is_none() {
            0
        } else {
            2
        };
        if let Some(uri) = &uri {
            snapshot.current_src = uri.clone();
        }
        self.state.media_changes.ready = Some(crate::embedded_video::Selection {
            node,
            retired,
            raw,
            uri,
            error,
            abort: pending.abort,
            emptied: pending.emptied,
            timeupdate: pending.timeupdate,
        });
        Ok(())
    }
    pub(super) fn set_media_time(&mut self, node:usize, seconds:f64) -> Result<(),String> {
        self.state.work(1)?;
        if !self.state.enabled || !self.doc.scripting_enabled {return Err("Scripting is disabled.".into());}
        if !seconds.is_finite() {return Err("Media time must be finite.".into());}
        let snapshot=self.state.embedded_video.as_ref().filter(|snapshot|snapshot.node==node)
            .ok_or("Media binding is unavailable.")?;
        if snapshot.retired.load(Ordering::Relaxed) {return Err("Media source is retired.".into());}
        if snapshot.ready==0 {
            // No Duration conversion or decoder request at HAVE_NOTHING.
            self.state.media_changes.default_start=seconds;
            return Ok(());
        }
        let duration=snapshot.duration.ok_or("Media metadata is unavailable.")?;
        let target=Duration::try_from_secs_f64(seconds.clamp(0.0,duration.as_secs_f64()))
            .map_err(|_|"Media time cannot be represented safely.")?;
        let ticket=self.state.media_changes.next.checked_add(1).ok_or("Media control identity exhausted.")?;
        let mut tasks=snapshot.tasks.lock().map_err(|_|"Media task queue is unavailable.")?;
        tasks.set_limit(self.state.limits.host.media_tasks);
        tasks.enqueue(crate::media_tasks::Scope {node,retired:snapshot.retired.clone()},
            Some(crate::media_tasks::Control::Seek {seconds}),None)?;
        drop(tasks);
        self.state.media_changes.next=ticket;
        self.state.media_changes.default_start=0.0;
        self.state.media_changes.official_position=seconds;
        let snapshot=self.state.embedded_video.as_mut().unwrap();
        snapshot.position=target;snapshot.observed=Instant::now();snapshot.playing=false;
        snapshot.seeking=true;snapshot.ended=false;
        Ok(())
    }

    pub(super) fn pause_media(&mut self, node:usize) -> Result<Value,String> {
        self.state.work(1)?;
        if !self.state.enabled || !self.doc.scripting_enabled {return Err("Scripting is disabled.".into());}
        if self.state.embedded_video.as_ref().is_some_and(|s|s.network==0) {
            self.commit_media_source_change(node)?;
        }
        let snapshot=self.state.embedded_video.as_ref().ok_or("Media binding is unavailable.")?;
        if snapshot.paused {return Ok(Value::Undefined);}
        if self.state.limits.host.media_tasks==0 {
            return Err("Media task allowance reached.".into());
        }
        let ticket=self.state.media_changes.next.checked_add(1).ok_or("Media control identity exhausted.")?;
        if snapshot.retired.load(Ordering::Relaxed) {return Err("Media source is retired.".into());}
        let issued=Instant::now();
        let position=snapshot.position_at(issued);
        let mut tasks=snapshot.tasks.lock().map_err(|_|"Media task queue is unavailable.")?;
        tasks.set_limit(self.state.limits.host.media_tasks);
        tasks.enqueue(crate::media_tasks::Scope {node,retired:snapshot.retired.clone()},
            Some(crate::media_tasks::Control::Pause {issued,position}),Some(MediaTask::Pause))?;
        drop(tasks);
        // Queue storage/control identity are admitted before projection. The
        // source-bound physical effect survives a producer-job failure.
        self.state.media_changes.next=ticket;
        let snapshot=self.state.embedded_video.as_mut().unwrap();
        snapshot.position=position;snapshot.observed=issued;snapshot.playing=false;snapshot.paused=true;
        self.state.media_changes.official_position=position.as_secs_f64();
        Ok(Value::Undefined)
    }
    pub(super) fn media_handler_event<'a>(
        &self,
        object: u32,
        property: &'a str,
    ) -> Option<&'a str> {
        handler_event(property).or_else(|| {
            self.media_element_receiver(object)
                .then(|| property.strip_prefix("on").filter(|name| event_name(name)))
                .flatten()
        })
    }
    pub(super) fn media_element_receiver(&self, object: u32) -> bool {
        node_index(object).is_some_and(|node| {
            self.state
                .embedded_video
                .as_ref()
                .is_some_and(|s| s.node == node)
                && self.doc.nodes.get(node).is_some_and(|n| {
                    n.namespace == crate::html::Namespace::Html
                        && matches!(&n.kind,NodeKind::Element {tag,..} if tag=="video")
                })
        })
    }
    pub(super) fn media_element_has(
        &mut self,
        object: u32,
        key: &str,
    ) -> Result<Option<bool>, String> {
        self.node(object)?;
        Ok(match key {
            "src" | "currentSrc" | "controls" | "videoWidth" | "videoHeight" | "duration"
            | "currentTime" | "paused" | "ended" | "seeking" | "readyState" | "networkState" => {
                Some(true)
            }
            "load" | "pause" => Some(true),
            "play" | "canPlayType" => Some(false),
            _ => None,
        })
    }
    pub(super) fn media_element_get(
        &mut self,
        object: u32,
        key: &str,
    ) -> Result<Option<Value>, String> {
        let node = self.node(object)?;
        if key == "src" {return self.reflected_url(node,"src").map(Some);}
        if key == "controls" {
            return Ok(Some(Value::Bool(self.doc.attr(node, "controls").is_some())));
        }
        if key == "currentSrc" {
            let source = self
                .state
                .embedded_video
                .as_ref()
                .unwrap()
                .current_src
                .clone();
            return self.string(&source).map(Some);
        }
        let s = self.state.embedded_video.as_ref().unwrap();
        Ok(Some(match key {
            "videoWidth" => Value::Number(f64::from(s.width)),
            "videoHeight" => Value::Number(f64::from(s.height)),
            "duration" => Value::Number(s.duration.map_or(f64::NAN, |d| d.as_secs_f64())),
            "currentTime" => Value::Number(if self.state.media_changes.default_start!=0.0 {
                self.state.media_changes.default_start
            } else {self.state.media_changes.official_position}),
            "paused" => Value::Bool(s.paused),
            "ended" => Value::Bool(s.ended),
            "seeking" => Value::Bool(s.seeking),
            "readyState" => Value::Number(f64::from(s.ready)),
            "networkState" => Value::Number(f64::from(s.network)),
            "play" | "pause" | "load" | "canPlayType" => Value::Undefined,
            _ => return Ok(None),
        }))
    }
}

#[cfg(test)]
#[path = "script_media_pause_tests.rs"]
mod pause_tests;
