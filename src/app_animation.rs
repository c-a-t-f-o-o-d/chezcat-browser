//! Image playback is independent of scripting. Immutable composited frames are
//! shared across references; advancing a frame never parses or reflows a page.
use super::*;
use crate::image::Animation;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(super) struct Playback {
    animation: Arc<Animation>,
    started: Instant,
    ends_ms: Vec<u64>,
    cycle_ms: u64,
    frame: usize,
    deadline: Option<Instant>,
}

impl Playback {
    fn new(animation: Arc<Animation>, started: Instant, now: Instant) -> Self {
        let mut cycle_ms = 0;
        let ends_ms = animation
            .frames
            .iter()
            .map(|frame| {
                cycle_ms += u64::from(frame.delay_ms.max(20));
                cycle_ms
            })
            .collect();
        let mut player = Self {
            animation,
            started,
            ends_ms,
            cycle_ms,
            frame: 0,
            deadline: None,
        };
        player.advance(now);
        player
    }

    fn advance(&mut self, now: Instant) -> bool {
        if self.animation.frames.len() <= 1 || self.cycle_ms == 0 {
            self.deadline = None;
            return false;
        }
        let elapsed = now.saturating_duration_since(self.started).as_millis();
        let cycle = u128::from(self.cycle_ms);
        let finished = self
            .animation
            .plays
            .is_some_and(|plays| elapsed >= cycle * u128::from(plays));
        let frame = if finished {
            self.animation.frames.len() - 1
        } else {
            let phase = (elapsed % cycle) as u64;
            self.ends_ms.partition_point(|end| *end <= phase)
        };
        self.deadline = if finished {
            None
        } else {
            let boundary = (elapsed / cycle) * cycle + u128::from(self.ends_ms[frame]);
            u64::try_from(boundary)
                .ok()
                .and_then(|ms| self.started.checked_add(Duration::from_millis(ms)))
        };
        let changed = frame != self.frame;
        self.frame = frame;
        changed
    }
}

impl App {
    fn image_union_bytes(&self, excluded: usize, asset: &crate::image::DecodedImage) -> Result<usize, String> {
        let mut seen = HashSet::new();
        let mut bytes = 0usize;
        let mut count = |bitmap: &Arc<Bitmap>| -> Result<(), String> {
            if seen.insert(Arc::as_ptr(bitmap)) {
                bytes = bytes.checked_add(bitmap.pixels.len().checked_mul(4).ok_or("Image byte count overflow.")?).ok_or("Image byte count overflow.")?;
            }
            Ok(())
        };
        for (&node, bitmap) in &self.images { if node != excluded { count(bitmap)?; } }
        for (&node, player) in &self.animations {
            if node != excluded { for frame in &player.animation.frames { count(&frame.bitmap)?; } }
        }
        count(&asset.bitmap)?;
        if let Some(animation) = &asset.animation { for frame in &animation.frames { count(&frame.bitmap)?; } }
        Ok(bytes)
    }

    pub(super) fn fit_document_image(&self, node: usize, asset: &mut crate::image::DecodedImage) -> Result<(), String> {
        asset.retained_bytes()?;
        if self.image_union_bytes(node, asset)? > loader::MAX_DECODED_IMAGE_BYTES && asset.animation.is_some() {
            asset.animation = None;
            asset.notice = Some("Animation kept static to stay within the document's 64 MiB decoded image budget.".into());
        }
        if self.image_union_bytes(node, asset)? > loader::MAX_DECODED_IMAGE_BYTES { return Err("Decoded image budget exceeded (64 MiB).".into()); }
        Ok(())
    }

    pub(super) fn install_animations(&mut self, animations: HashMap<usize, Arc<Animation>>) {
        let now = Instant::now();
        for (node, animation) in animations {
            if animation.frames.is_empty() {
                continue;
            }
            // A reused resource has one time origin, even if references arrive
            // in separate background-loader deliveries.
            let started = self
                .animations
                .values()
                .find(|other| Arc::ptr_eq(&animation, &other.animation))
                .map_or(now, |other| other.started);
            let player = Playback::new(animation, started, now);
            self.images
                .insert(node, player.animation.frames[player.frame].bitmap.clone());
            self.animations.insert(node, player);
        }
    }

    fn visible_animated_nodes(&self) -> HashSet<usize> {
        let mut nodes = HashSet::new();
        if self.animations.is_empty()
            || !self.images_enabled
            || self.view != View::Page
            || self.browser_panel.is_some()
            || self.resubmission.is_some()
            || self.pending.as_ref().is_some_and(|load| !load.committed)
        {
            return nodes;
        }
        let viewport =
            crate::paint::ClipRect::new(0, self.scroll, self.width, self.viewport_height());
        for (index, item) in self.page.items.iter().enumerate() {
            let (node, rectangle) = match *item {
                PaintItem::Image {
                    node,
                    x,
                    y,
                    width,
                    height,
                    ..
                } => (node, crate::paint::ClipRect::new(x, y, width, height)),
                PaintItem::Control(control) => {
                    let rect = &self.page.controls[control];
                    (
                        rect.node,
                        crate::paint::ClipRect::new(rect.x, rect.y, rect.width, rect.height),
                    )
                }
                _ => continue,
            };
            if !self.animations.contains_key(&node) {
                continue;
            }
            let rectangle = rectangle.intersect(viewport);
            let rectangle = self
                .page
                .paint_clip(index)
                .map_or(rectangle, |clip| rectangle.intersect(clip));
            if rectangle.right > rectangle.left && rectangle.bottom > rectangle.top {
                nodes.insert(node);
            }
        }
        nodes
    }

    pub(super) fn next_image_frame(&self) -> Option<Instant> {
        self.visible_animated_nodes()
            .iter()
            .filter_map(|node| self.animations[node].deadline)
            .min()
    }

    fn poll_image_frames_at(&mut self, now: Instant) -> bool {
        let visible = self.visible_animated_nodes();
        let mut changed = false;
        for node in visible {
            let player = self.animations.get_mut(&node).unwrap();
            if player.deadline.is_some_and(|due| due <= now) && player.advance(now) {
                self.images
                    .insert(node, player.animation.frames[player.frame].bitmap.clone());
                changed = true;
            }
        }
        changed
    }

    pub fn next_page_timer(&self) -> Option<Instant> {
        self.next_script_timer()
            .into_iter()
            .chain(self.next_image_frame())
            .chain(self.next_media_timer())
            .chain(self.next_audio_timer())
            .min()
    }

    pub fn poll_page_timer(&mut self) -> bool {
        let scripts = self
            .next_script_timer()
            .is_some_and(|due| due <= Instant::now())
            && self.poll_script_timer();
        let images = self.poll_image_frames_at(Instant::now());
        let media=self.poll_media_timer_at(Instant::now());
        let audio=self.poll_audio_timer_at(Instant::now());
        scripts || images || media || audio
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::ImageFrame;

    fn sequence(plays: Option<u32>) -> Arc<Animation> {
        Arc::new(Animation {
            frames: [0xffff0000, 0xff00ff00, 0xff0000ff]
                .into_iter()
                .zip([10, 30, 40])
                .map(|(pixel, delay_ms)| ImageFrame {
                    bitmap: Arc::new(Bitmap {
                        width: 1,
                        height: 1,
                        pixels: vec![pixel],
                    }),
                    delay_ms,
                })
                .collect(),
            plays,
        })
    }

    #[test]
    fn absolute_clock_obeys_delays_loops_and_final_frame_without_catchup_work() {
        let start = Instant::now();
        let mut player = Playback::new(sequence(Some(2)), start, start);
        assert_eq!(player.frame, 0);
        assert_eq!(player.deadline, Some(start + Duration::from_millis(20)));
        assert!(!player.advance(start + Duration::from_millis(19)));
        assert!(player.advance(start + Duration::from_millis(20)));
        assert_eq!(player.frame, 1);
        assert!(player.advance(start + Duration::from_millis(90)));
        assert_eq!(player.frame, 0);
        assert!(player.advance(start + Duration::from_secs(86400)));
        assert_eq!(player.frame, 2);
        assert!(player.deadline.is_none());

        let mut repeating = Playback::new(sequence(None), start, start);
        let far = start + Duration::from_millis(90_000_050);
        repeating.advance(far);
        assert_eq!(repeating.frame, 2);
        assert_eq!(repeating.deadline, Some(far + Duration::from_millis(40)));
    }

    #[test]
    fn real_gif_apng_and_webp_documents_play_without_javascript_and_clear_on_navigation() {
        for name in [
            "format-animated.gif",
            "format-animated.apng",
            "format-animated.webp",
        ] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("pages/images")
                .join(name);
            let mut app = App::new();
            app.scripts_enabled = false;
            app.open(&path);
            assert_eq!(app.animations.len(), 1, "{name}: {:?}", app.events);
            let node = *app.animations.keys().next().unwrap();
            let player = &app.animations[&node];
            let first = app.images[&node].clone();
            let second = player.animation.frames[1].bitmap.clone();
            let due = player.started + Duration::from_millis(player.ends_ms[0]);
            let layout = app.page.items.as_ptr();
            assert!(app.next_script_timer().is_none());
            assert!(app.poll_image_frames_at(due));
            assert!(Arc::ptr_eq(&app.images[&node], &second));
            assert!(!Arc::ptr_eq(&first, &second));
            assert_eq!(layout, app.page.items.as_ptr());
            app.open(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pages/second.html"));
            assert!(app.animations.is_empty());
            assert!(app.next_image_frame().is_none());
        }
    }

    #[test]
    fn real_reused_animation_waits_offscreen_and_restarts_when_resize_reveals_it() {
        let path =
            std::env::temp_dir().join(format!("chezcat-animation-page-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(path.clone());
        std::fs::write(
            path.join("motion.gif"),
            include_bytes!("../pages/images/format-animated.gif"),
        )
        .unwrap();
        std::fs::write(path.join("index.html"),"<body style='margin:0'><div style='height:600px'>Scroll down</div><img src=motion.gif width=160 height=96><img src=motion.gif width=160 height=96></body>").unwrap();
        let mut app = App::new();
        app.scripts_enabled = false;
        app.resize(800, 400);
        app.open(&path.join("index.html"));
        assert_eq!(app.animations.len(), 2);
        let players: Vec<_> = app.animations.values().collect();
        assert!(Arc::ptr_eq(&players[0].animation, &players[1].animation));
        assert_eq!(players[0].started, players[1].started);
        let start = players[0].started;
        assert!(app.next_page_timer().is_none());
        app.resize(800, 1000);
        assert!(app.next_page_timer().is_some());
        assert!(app.poll_image_frames_at(start + Duration::from_millis(150)));
        let frames: Vec<_> = app.images.values().collect();
        assert!(Arc::ptr_eq(frames[0], frames[1]));
        app.set_images_enabled(false);
        assert!(app.animations.is_empty() && app.next_image_frame().is_none());
    }

    #[test]
    fn image_animation_runs_with_scripts_off_and_never_reflows_or_edits_forms() {
        let mut app = App::new();
        app.scripts_enabled = false;
        app.document = html::parse("<body><img id=a width=20 height=20><img id=b width=20 height=20><input id=entry value=kept></body>");
        app.forms = Forms::new(&app.document);
        let ids: Vec<_> = app
            .document
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(n, _)| {
                app.document
                    .attr(n, "id")
                    .filter(|id| *id == "a" || *id == "b")
                    .map(|_| n)
            })
            .collect();
        let frames = sequence(Some(2));
        app.install_animations(HashMap::from([(ids[0], frames.clone())]));
        app.install_animations(HashMap::from([(ids[1], frames)]));
        app.reflow();
        let start = app.animations[&ids[0]].started;
        assert_eq!(app.animations[&ids[1]].started, start);
        let layout = app.page.items.as_ptr();
        assert!(app.next_script_timer().is_none());
        assert!(app.next_page_timer().is_some());
        assert!(app.poll_image_frames_at(start + Duration::from_millis(25)));
        assert_eq!(app.images[&ids[0]].pixels, [0xff00ff00]);
        assert!(Arc::ptr_eq(&app.images[&ids[0]], &app.images[&ids[1]]));
        assert_eq!(app.page.items.as_ptr(), layout);
        assert!(app
            .forms
            .controls
            .values()
            .any(|c| c.value.text() == "kept"));
        app.view = View::Resources;
        assert!(app.next_page_timer().is_none());
        assert!(!app.poll_image_frames_at(start + Duration::from_millis(80)));
        app.view = View::Page;
        assert!(app.poll_image_frames_at(start + Duration::from_millis(180)));
        assert_eq!(app.images[&ids[0]].pixels, [0xff0000ff]);
        assert!(app.next_image_frame().is_none());
    }
}
