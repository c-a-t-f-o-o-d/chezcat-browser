//! Readonly state and real native events for the one bound authored video.
use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
fn selection_copy_cost(
    raw: &str,
    base: Option<&crate::navigation::Location>,
) -> Result<usize, String> {
    let mut bytes = raw.len();
    if let Some(base) = base {
        for count in [
            base.fragment.len(),
            base.path.as_ref().map_or(0, |p| p.as_os_str().len()),
            base.url.as_ref().map_or(0, |u| u.host.len()),
            base.url.as_ref().map_or(0, |u| u.path.len()),
            base.url
                .as_ref()
                .and_then(|u| u.query.as_ref())
                .map_or(0, String::len),
        ] {
            bytes = bytes
                .checked_add(count)
                .ok_or("Media URL copy accounting overflow.")?;
        }
    }
    // Covers resolution temporaries, file percent encoding and both retained
    // URI copies; no allocation depends on a claimed resource size.
    bytes
        .checked_mul(20)
        .and_then(|n| n.checked_add(1024))
        .ok_or_else(|| "Media URL copy accounting overflow.".into())
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
    next: u64,
    prepared: Option<Prepared>,
    pub(super) pending: Option<Pending>,
    ready: Option<crate::embedded_video::Selection>,
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
            node,
            retired: Arc::new(AtomicBool::new(false)),
            current_src: "previous".into(),
            width: 64,
            height: 48,
            duration: Some(Duration::from_secs(3)),
            position: Duration::ZERO,
            observed: Instant::now(),
            playing: false,
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
    fn native_media_queue_quota_and_identity_overflow_reject_before_mutation() {
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
            let out = p.console_eval(&mut doc, &mut forms, "video.src='scene-replacement.ivf';");
            assert_eq!(out.errors, 1, "{}", p.report());
            assert_eq!(doc.attr(node, "src"), Some("scene-intra.ivf"));
            assert!(!token.load(Ordering::Relaxed));
            assert!(!p.has_media_selection());
            assert!(!p.runtime.has_microtasks());
            assert!(p.state.media_changes.prepared.is_none());
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
    pub fn dispatch_media(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        node: usize,
        event: &str,
    ) -> Outcome {
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
        snapshot.retired.store(true, Ordering::Relaxed);
        snapshot.retired = prepared.retired;
        snapshot.width = 0;
        snapshot.height = 0;
        snapshot.duration = None;
        snapshot.position = Duration::ZERO;
        snapshot.observed = Instant::now();
        snapshot.playing = false;
        snapshot.ended = false;
        snapshot.seeking = false;
        snapshot.ready = 0;
        snapshot.network = 3;
        self.state.media_changes.ready = None;
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
        let mut error = selection_copy_cost(raw.unwrap_or(""), self.state.dynamic.base.as_ref())
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
            "play" | "pause" | "load" | "canPlayType" => Some(false),
            _ => None,
        })
    }
    pub(super) fn media_element_get(
        &mut self,
        object: u32,
        key: &str,
    ) -> Result<Option<Value>, String> {
        let node = self.node(object)?;
        if key == "src" {
            let Some(raw)=self.doc.attr(node,"src") else{return self.string("").map(Some);};
            self.state
                .copies(selection_copy_cost(raw, self.state.dynamic.base.as_ref())?)?;
            let selected = self
                .state
                .dynamic
                .base
                .as_ref()
                .and_then(|base| crate::navigation::selection_url(raw, base).ok())
                .unwrap_or_else(|| raw.to_owned());
            return self.string(&selected).map(Some);
        }
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
            "currentTime" => Value::Number(s.position().as_secs_f64()),
            "paused" => Value::Bool(!s.playing),
            "ended" => Value::Bool(s.ended),
            "seeking" => Value::Bool(s.seeking),
            "readyState" => Value::Number(f64::from(s.ready)),
            "networkState" => Value::Number(f64::from(s.network)),
            "play" | "pause" | "load" | "canPlayType" => Value::Undefined,
            _ => return Ok(None),
        }))
    }
}
