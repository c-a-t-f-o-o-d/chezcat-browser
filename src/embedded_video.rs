//! Parser-authored local IVF video source loading. No network/codec helper.
use crate::{
    html::{Document, Namespace, NodeKind},
    media,
    navigation::Location,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::{Duration, Instant};

pub struct Plan {
    pub retired: Arc<AtomicBool>,
    pub node: usize,
    pub raw: String,
    pub source: Location,
    limits: media::Limits,
}
impl Plan {
    pub fn new(
        node: usize,
        doc: &Document,
        location: &Location,
        base: &Location,
        policy: &crate::csp::PolicySet,
        limits: media::Limits,
    ) -> Result<Self, String> {
        let raw = doc
            .attr(node, "src")
            .ok_or("This video slice requires src; source children are not implemented.")?;
        Self::selected(
            node,
            raw,
            Arc::new(AtomicBool::new(false)),
            doc,
            location,
            base,
            policy,
            limits,
            node,
        )
    }
    pub fn selected(
        node: usize,
        raw: &str,
        retired: Arc<AtomicBool>,
        doc: &Document,
        location: &Location,
        base: &Location,
        policy: &crate::csp::PolicySet,
        limits: media::Limits,
        policy_scope: usize,
    ) -> Result<Self, String> {
        if !doc.nodes.get(node).is_some_and(|n| {
            n.namespace == Namespace::Html
                && matches!(&n.kind,NodeKind::Element {tag,..} if tag=="video")
        }) {
            return Err("Embedded media plan requires its actual HTML video node.".into());
        }
        if location.url.is_some() || location.path.is_none() {
            return Err("Embedded video currently accepts a local document and an in-root local IVF source only.".into());
        }
        if let Some(name) = ["loop", "poster", "autoplay", "crossorigin"]
            .into_iter()
            .find(|name| doc.attr(node, name).is_some())
        {
            return Err(format!("Authored video attribute {name:?} is not implemented in this element slice; source was not loaded."));
        }
        if raw.is_empty() {
            return Err("The selected media src is empty.".into());
        }
        let source = crate::navigation::resolve(raw, base)?;
        if source.url.is_some() || source.path.is_none() || source.root != location.root {
            return Err(
                "Embedded video source is outside the local document root or is not a local file."
                    .into(),
            );
        }
        policy.allows_media(policy_scope, &source)?;
        if !source
            .path
            .as_ref()
            .unwrap()
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("ivf"))
        {
            return Err("Embedded video currently decodes original local IVF/VP8-intra video only; other containers/audio/network are not enabled for this element slice.".into());
        }
        Ok(Self {
            retired,
            node,
            raw: raw.into(),
            source,
            limits,
        })
    }
    fn load(&self, cancel: &AtomicBool) -> Result<media::Asset, String> {
        let check = || {
            if cancel.load(Ordering::Relaxed) || self.retired.load(Ordering::Relaxed) {
                Err("Embedded video source cancelled.".into())
            } else {
                Ok(())
            }
        };
        check()?;
        let bytes = crate::loader::read_local_media_checked(
            self.source
                .path
                .as_ref()
                .ok_or("Missing local video path.")?,
            self.limits.input_bytes,
            &check,
        )?;
        media::Clip::load(bytes, self.limits, &check)
    }
}

/// Only connected parser nodes participate; inert/foreign subtrees supply no job.
pub fn authored_node(doc: &Document) -> Option<usize> {
    let mut seen = vec![false; doc.nodes.len()];
    let mut stack = vec![0];
    while let Some(node) = stack.pop() {
        if node >= doc.nodes.len() || seen[node] {
            continue;
        }
        seen[node] = true;
        if let NodeKind::Element { tag, .. } = &doc.nodes[node].kind {
            if matches!(tag.as_str(), "template" | "iframe" | "object")
                || tag == "noscript" && doc.scripting_enabled
            {
                continue;
            }
            if doc.nodes[node].namespace == Namespace::Html && tag == "video" {
                return Some(node);
            }
        }
        stack.extend(doc.nodes[node].children.iter().rev().copied());
    }
    None
}

#[derive(Clone)]
pub struct Snapshot {
    pub tasks: crate::media_tasks::Shared,
    pub retired: Arc<AtomicBool>,
    pub node: usize,
    pub current_src: String,
    pub width: u32,
    pub height: u32,
    pub duration: Option<Duration>,
    pub position: Duration,
    pub observed: Instant,
    pub playing: bool,
    pub paused: bool,
    pub load_pending: bool,
    pub ended: bool,
    pub seeking: bool,
    pub ready: u8,
    pub network: u8,
}
pub struct Selection {
    pub node: usize,
    pub retired: Arc<AtomicBool>,
    pub raw: Option<String>,
    pub uri: Option<String>,
    pub error: Option<String>,
    pub abort: bool,
    pub emptied: bool,
    pub timeupdate: bool,
}
impl Snapshot {
    pub fn position(&self) -> Duration {
        self.position_at(Instant::now())
    }
    pub fn position_at(&self, now: Instant) -> Duration {
        self.duration.map_or(self.position, |duration| {
            if self.playing {
                self.position
                    .saturating_add(now.saturating_duration_since(self.observed))
                    .min(duration)
            } else {
                self.position
            }
        })
    }
}

struct Job {
    plan: Arc<Plan>,
    cancel: Arc<AtomicBool>,
}
pub struct Completion {
    pub plan: Arc<Plan>,
    pub result: Result<media::Asset, String>,
}
struct Shared {
    queued: Option<Job>,
    active: Option<Arc<AtomicBool>>,
    finished: Option<Completion>,
    stopping: bool,
    wake: crate::loader::Wake,
}
pub struct Worker {
    shared: Arc<(Mutex<Shared>, Condvar)>,
}
impl Worker {
    pub fn new(wake: crate::loader::Wake) -> Result<Self, String> {
        let shared = Arc::new((
            Mutex::new(Shared {
                queued: None,
                active: None,
                finished: None,
                stopping: false,
                wake,
            }),
            Condvar::new(),
        ));
        let background = shared.clone();
        std::thread::Builder::new()
            .name("embedded-video-source".into())
            .spawn(move || loop {
                let job = {
                    let mut s = background.0.lock().unwrap();
                    while s.queued.is_none() && !s.stopping {
                        s = background.1.wait(s).unwrap();
                    }
                    if s.stopping {
                        return;
                    }
                    let job = s.queued.take().unwrap();
                    s.active = Some(job.cancel.clone());
                    job
                };
                let result = job.plan.load(&job.cancel);
                let wake = {
                    let mut s = background.0.lock().unwrap();
                    s.active = None;
                    if s.stopping {
                        return;
                    }
                    if job.cancel.load(Ordering::Relaxed)
                        || job.plan.retired.load(Ordering::Relaxed)
                    {
                        continue;
                    }
                    s.finished = Some(Completion {
                        plan: job.plan,
                        result,
                    });
                    s.wake.clone()
                };
                wake();
            })
            .map_err(|e| format!("Could not start embedded video source worker: {e}"))?;
        Ok(Self { shared })
    }
    pub fn set_wake(&self, wake: crate::loader::Wake) {
        self.shared.0.lock().unwrap().wake = wake;
    }
    pub fn submit(&self, plan: Arc<Plan>) {
        self.cancel();
        let mut s = self.shared.0.lock().unwrap();
        s.queued = Some(Job {
            plan,
            cancel: Arc::new(AtomicBool::new(false)),
        });
        self.shared.1.notify_one();
    }
    pub fn take(&self) -> Option<Completion> {
        self.shared.0.lock().unwrap().finished.take()
    }
    pub fn cancel(&self) {
        let mut s = self.shared.0.lock().unwrap();
        if let Some(cancel) = &s.active {
            cancel.store(true, Ordering::Relaxed);
        }
        s.queued = None;
        s.finished = None;
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
        self.shared.0.lock().unwrap().stopping = true;
        self.shared.1.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn location() -> Location {
        crate::navigation::explicit(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tools/fixtures/authored-video.html"),
        )
        .unwrap()
    }
    fn document() -> Document {
        crate::html::parse(include_str!("../tools/fixtures/authored-video.html"))
    }
    #[test]
    fn embedded_video_source_policy_root_and_inert_selection_precede_resource_loading() {
        let location = location();
        let doc = document();
        let node = authored_node(&doc).unwrap();
        for text in ["media-src 'none'", "default-src 'none'; img-src 'self'"] {
            let mut policy = crate::csp::PolicySet::new(&location);
            policy.add_header(text);
            assert!(Plan::new(
                node,
                &doc,
                &location,
                &location,
                &policy,
                media::Limits::default()
            )
            .err()
            .unwrap()
            .contains("media-src/default-src"));
        }
        let mut policy = crate::csp::PolicySet::new(&location);
        policy.add_header("default-src 'none'; media-src 'self'");
        assert!(Plan::new(
            node,
            &doc,
            &location,
            &location,
            &policy,
            media::Limits::default()
        )
        .is_ok());
        policy.add_header("media-src 'none'");
        assert!(Plan::new(
            node,
            &doc,
            &location,
            &location,
            &policy,
            media::Limits::default()
        )
        .is_err());
        let mut policy = crate::csp::PolicySet::new(&location);
        policy.add_meta("media-src 'none'", node);
        assert!(Plan::new(
            node,
            &doc,
            &location,
            &location,
            &policy,
            media::Limits::default()
        )
        .is_ok());
        assert!(policy.allows_media(node + 1, &location).is_err());
        assert!(
            Plan::selected(
                node,
                "scene-intra.ivf",
                Arc::new(AtomicBool::new(false)),
                &doc,
                &location,
                &location,
                &policy,
                media::Limits::default(),
                usize::MAX
            )
            .is_err(),
            "Dynamic selection observes all installed meta policies."
        );
        for source in [
            "../../Cargo.toml",
            "https://example.test/clip.ivf",
            "data:video/x-ivf,not-a-file",
            "scene-av.mkv",
        ] {
            let d = crate::html::parse(&format!("<video src='{source}' controls></video>"));
            let n = authored_node(&d).unwrap();
            assert!(
                Plan::new(
                    n,
                    &d,
                    &location,
                    &location,
                    &crate::csp::PolicySet::new(&location),
                    media::Limits::default()
                )
                .is_err(),
                "{source}"
            );
        }
        for name in ["loop", "poster", "autoplay", "crossorigin"] {
            let d = crate::html::parse(&format!("<video {name} src='scene-intra.ivf'></video>"));
            let n = authored_node(&d).unwrap();
            assert!(Plan::new(
                n,
                &d,
                &location,
                &location,
                &crate::csp::PolicySet::new(&location),
                media::Limits::default()
            )
            .err()
            .unwrap()
            .contains("source was not loaded"));
        }
        for html in [
            "<template><video src=scene-intra.ivf></video></template>",
            "<iframe><video src=scene-intra.ivf></video></iframe>",
            "<svg><video src=scene-intra.ivf></video></svg>",
        ] {
            assert!(authored_node(&crate::html::parse(html)).is_none(), "{html}");
        }
    }
    #[test]
    fn embedded_video_real_source_worker_cancellation_latest_plan_and_small_profile_recovery() {
        let location = location();
        let doc = document();
        let node = authored_node(&doc).unwrap();
        let policy = crate::csp::PolicySet::new(&location);
        let plan = Arc::new(
            Plan::new(
                node,
                &doc,
                &location,
                &location,
                &policy,
                media::Limits::default(),
            )
            .unwrap(),
        );
        assert!(plan
            .load(&AtomicBool::new(true))
            .err()
            .unwrap()
            .contains("cancelled"));
        let worker = Worker::new(Arc::new(|| {})).unwrap();
        worker.submit(plan.clone());
        worker.cancel();
        let next = Arc::new(
            Plan::new(
                node,
                &doc,
                &location,
                &location,
                &policy,
                media::Limits::default(),
            )
            .unwrap(),
        );
        worker.submit(next.clone());
        let until = Instant::now() + Duration::from_secs(5);
        let result = loop {
            if let Some(c) = worker.take() {
                break c;
            }
            assert!(Instant::now() < until, "real source worker timed out");
            std::thread::sleep(Duration::from_millis(1));
        };
        assert!(Arc::ptr_eq(&result.plan, &next));
        let asset = result.result.unwrap();
        assert_eq!(
            (
                asset.clip.width,
                asset.clip.height,
                asset.clip.frame_count(),
                asset.clip.duration
            ),
            (64, 48, 6, Duration::from_secs(3))
        );
        let small = Plan::new(
            node,
            &doc,
            &location,
            &location,
            &policy,
            media::Limits {
                frames: 1,
                ..media::Limits::default()
            },
        )
        .unwrap();
        assert!(small.load(&AtomicBool::new(false)).is_err());
        assert!(plan.load(&AtomicBool::new(false)).is_ok());
    }
}
