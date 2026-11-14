//! Original local IVF demuxing and an independent video playback clock.
//! Initial slice: visible VP8 keyframes only; no audio, interframes, WebM,
//! network streaming or HTMLMediaElement capability is advertised.
//! Header/timebase format reference: WebM project's ivfenc.c. Codec: RFC6386.
use crate::image::Bitmap;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

pub fn is_local_media(location: &crate::navigation::Location) -> bool {
    location.url.is_none()
        && location
            .path
            .as_ref()
            .and_then(|path| path.extension())
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ivf")||ext.eq_ignore_ascii_case("wav")||ext.eq_ignore_ascii_case("mkv"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub input_bytes: usize,
    pub frame_bytes: usize,
    pub frames: usize,
    pub pixels: usize,
    pub duration: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            input_bytes: 512 * 1024 * 1024,
            frame_bytes: 32 * 1024 * 1024,
            frames: 1_000_000,
            pixels: 16 * 1024 * 1024,
            duration: Duration::from_secs(7 * 24 * 3600),
        }
    }
}
#[derive(Debug)]
struct Frame {
    bytes: Range<usize>,
    time: Duration,
}
pub struct Clip {
    data: Vec<u8>,
    frames: Vec<Frame>,
    pub width: usize,
    pub height: usize,
    pub duration: Duration,
    limits: Limits,
}
impl std::fmt::Debug for Clip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Clip")
            .field("bytes", &self.data.len())
            .field("frames", &self.frames.len())
            .field("width", &self.width)
            .field("height", &self.height)
            .field("duration", &self.duration)
            .finish()
    }
}
pub struct Asset {
    pub clip: Arc<Clip>,
    pub first: Arc<Bitmap>,
    pub initial_index: usize,
}

fn u16_at(data: &[u8], at: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        data.get(at..at + 2)
            .ok_or("Truncated IVF header.")?
            .try_into()
            .unwrap(),
    ))
}
fn u32_at(data: &[u8], at: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        data.get(at..at + 4)
            .ok_or("Truncated IVF record.")?
            .try_into()
            .unwrap(),
    ))
}
fn u64_at(data: &[u8], at: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        data.get(at..at + 8)
            .ok_or("Truncated IVF timestamp.")?
            .try_into()
            .unwrap(),
    ))
}
fn time(ticks: u64, scale: u32, rate: u32) -> Result<Duration, String> {
    let nanos = u128::from(ticks)
        .checked_mul(u128::from(scale))
        .and_then(|n| n.checked_mul(1_000_000_000))
        .ok_or("IVF timestamp multiplication overflow.")?
        / u128::from(rate);
    Ok(Duration::from_nanos(u64::try_from(nanos).map_err(
        |_| "IVF duration is outside the supported clock range.",
    )?))
}
impl Clip {
    /// Validated packet input from our local container demuxer, not a wrapper
    /// file manufactured for the IVF parser. Timestamps share the container origin.
    pub(crate) fn from_packets(data:Vec<u8>, packets:Vec<(Range<usize>,Duration)>,width:usize,height:usize,duration:Duration,limits:Limits,check:&dyn Fn()->Result<(),String>)->Result<Self,String> {
        if data.len()>limits.input_bytes||packets.is_empty()||packets.len()>limits.frames
            ||width==0||height==0||width>16383||height>16383||width.checked_mul(height).is_none_or(|n|n>limits.pixels)
            ||duration.is_zero()||duration>limits.duration {return Err("Muxed video dimensions/index/duration allowance exceeded.".into());}
        let mut frames=Vec::new();let mut previous=None;
        for (bytes,time) in packets {
            check()?;
            let payload=data.get(bytes.clone()).filter(|p|!p.is_empty()&&p.len()<=limits.frame_bytes).ok_or("Invalid muxed video packet range.")?;
            if time>=duration||previous.is_none()&&!time.is_zero()||previous.is_some_and(|old|time<=old) {return Err("Muxed video timestamps must start at zero and increase.".into());}
            if payload.len()<10||payload[0]&1!=0||payload[0]&16==0||payload[3..6]!=[0x9d,1,0x2a]
                ||usize::from(u16_at(payload,6)?)!=width||usize::from(u16_at(payload,8)?)!=height {return Err("Muxed video requires matching visible VP8 intra frames without scaling.".into());}
            frames.try_reserve(1).map_err(|_|"Could not grow muxed video index.")?;
            previous=Some(time);frames.push(Frame {bytes,time});
        }
        Ok(Self {data,frames,width,height,duration,limits})
    }
    pub fn parse(
        data: Vec<u8>,
        limits: Limits,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<Self, String> {
        check()?;
        if data.len() > limits.input_bytes {
            return Err("Video input byte allowance exceeded.".into());
        }
        if data.get(..4) != Some(b"DKIF") || u16_at(&data, 4)? != 0 || u16_at(&data, 6)? != 32 {
            return Err("Expected a version-zero 32-byte IVF header.".into());
        }
        if data.get(8..12) != Some(b"VP80") {
            return Err("This local video slice requires IVF VP8 keyframes.".into());
        }
        let (width, height) = (
            usize::from(u16_at(&data, 12)?),
            usize::from(u16_at(&data, 14)?),
        );
        if width == 0
            || height == 0
            || width > 16383
            || height > 16383
            || width.checked_mul(height).is_none_or(|n| n > limits.pixels)
        {
            return Err("Video dimensions/pixel allowance exceeded.".into());
        }
        let (rate, scale, count) = (u32_at(&data, 16)?, u32_at(&data, 20)?, u32_at(&data, 24)?);
        if rate == 0 || scale == 0 {
            return Err("IVF timebase must be nonzero.".into());
        }
        let mut frames = Vec::new();
        let mut position = 32usize;
        let mut first = None;
        let mut previous = 0u64;
        let mut last_gap = 1u64;
        while position < data.len() {
            check()?;
            if frames.len() >= limits.frames {
                return Err("Video frame-index allowance exceeded.".into());
            }
            let header_end = position
                .checked_add(12)
                .filter(|&end| end <= data.len())
                .ok_or("Truncated IVF frame record.")?;
            let size = usize::try_from(u32_at(&data, position)?)
                .map_err(|_| "IVF frame length exceeds this machine.")?;
            let stamp = u64_at(&data, position + 4)?;
            if size == 0 || size > limits.frame_bytes {
                return Err("Video encoded-frame allowance exceeded.".into());
            }
            let end = header_end
                .checked_add(size)
                .filter(|&end| end <= data.len())
                .ok_or("Truncated IVF frame payload.")?;
            let payload = &data[header_end..end];
            if payload.len() < 10
                || payload[0] & 1 != 0
                || payload[0] & 16 == 0
                || payload[3..6] != [0x9d, 1, 0x2a]
            {
                return Err("Only visible, complete intra VP8 frames are supported; interframes are not implemented.".into());
            }
            if usize::from(u16_at(payload, 6)?) != width
                || usize::from(u16_at(payload, 8)?) != height
            {
                return Err("VP8 frame dimensions/scaling differ from the IVF canvas.".into());
            }
            let start = *first.get_or_insert(stamp);
            if !frames.is_empty() && stamp < previous {
                return Err("IVF presentation timestamps run backwards.".into());
            }
            if !frames.is_empty() && stamp > previous {
                last_gap = stamp - previous;
            }
            let instant = time(stamp - start, scale, rate)?;
            if instant > limits.duration {
                return Err("Video duration allowance exceeded.".into());
            }
            frames
                .try_reserve(1)
                .map_err(|_| "Could not grow the video frame index.")?;
            frames.push(Frame {
                bytes: header_end..end,
                time: instant,
            });
            previous = stamp;
            position = end;
        }
        if frames.is_empty() || count != 0 && usize::try_from(count).ok() != Some(frames.len()) {
            return Err("IVF declared/actual frame count mismatch or empty clip.".into());
        }
        // IVF has timestamps but no packet duration. Hold the final frame for
        // the last positive timestamp interval, or one timebase tick for one
        // frame/duplicate timestamps. This is the explicit local-viewer policy.
        let end = previous
            .checked_sub(first.unwrap())
            .and_then(|n| n.checked_add(last_gap))
            .ok_or("IVF final timestamp overflow.")?;
        let duration = time(end, scale, rate)?;
        if duration.is_zero() || duration > limits.duration {
            return Err("Video duration allowance exceeded or below clock precision.".into());
        }
        Ok(Self {
            data,
            frames,
            width,
            height,
            duration,
            limits,
        })
    }
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }
    pub fn index_at(&self, position: Duration) -> usize {
        self.frames
            .partition_point(|frame| frame.time <= position)
            .saturating_sub(1)
    }
    pub fn decode(
        &self,
        index: usize,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<Arc<Bitmap>, String> {
        check()?;
        let frame = self
            .frames
            .get(index)
            .ok_or("Video frame index is unavailable.")?;
        let bitmap = crate::image::decode_vp8_frame_checked(
            &self.data[frame.bytes.clone()],
            self.limits.pixels,
            check,
        )
        .map_err(|error| format!("Video frame {index}: {error}"))?;
        if bitmap.width != self.width || bitmap.height != self.height {
            return Err("Decoded video frame has a different canvas.".into());
        }
        Ok(Arc::new(bitmap))
    }
    pub fn load(
        data: Vec<u8>,
        limits: Limits,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<Asset, String> {
        let clip = Arc::new(Self::parse(data, limits, check)?);
        let initial_index = clip.index_at(Duration::ZERO);
        let first = clip.decode(initial_index, check)?;
        Ok(Asset {
            clip,
            first,
            initial_index,
        })
    }
}

#[derive(Debug)]
pub struct Clock {
    pub duration: Duration,
    position: Duration,
    anchor: Option<Instant>,
    pub ended: bool,
}
impl Clock {
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            position: Duration::ZERO,
            anchor: None,
            ended: false,
        }
    }
    pub fn playing(&self) -> bool {
        self.anchor.is_some()
    }
    pub fn position(&self, now: Instant) -> Duration {
        self.position
            .saturating_add(self.anchor.map_or(Duration::ZERO, |anchor| {
                now.saturating_duration_since(anchor)
            }))
            .min(self.duration)
    }
    pub fn play(&mut self, now: Instant) {
        if self.playing() {
            return;
        }
        if self.position >= self.duration {
            self.position = Duration::ZERO;
        }
        self.ended = false;
        self.anchor = Some(now);
    }
    pub fn pause(&mut self, now: Instant) {
        self.position = self.position(now);
        self.anchor = None;
        self.ended = self.position >= self.duration;
    }
    pub fn seek(&mut self, position: Duration, now: Instant) {
        let playing = self.playing();
        self.position = position.min(self.duration);
        self.ended = self.position >= self.duration;
        self.anchor = (playing && !self.ended).then_some(now);
    }
    pub fn tick(&mut self, now: Instant) -> bool {
        if self.playing() && self.position(now) >= self.duration {
            self.position = self.duration;
            self.anchor = None;
            self.ended = true;
            true
        } else {
            false
        }
    }
}

struct Job {
    clip: Arc<Clip>,
    pub generation: u64,
    index: usize,
    cancel: Arc<AtomicBool>,
    retired:Option<Arc<AtomicBool>>,
}
pub struct Completion {
    pub clip: Arc<Clip>,
    pub generation: u64,
    pub index: usize,
    pub result: Result<Arc<Bitmap>, String>,
}
struct Shared {
    queued: Option<Job>,
    finished: Option<Completion>,
    active: Option<Arc<AtomicBool>>,
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
                finished: None,
                active: None,
                stopping: false,
                wake,
            }),
            Condvar::new(),
        ));
        let background = shared.clone();
        std::thread::Builder::new()
            .name("video-frame-decoder".into())
            .spawn(move || loop {
                let job = {
                    let mut state = background.0.lock().unwrap();
                    while state.queued.is_none() && !state.stopping {
                        state = background.1.wait(state).unwrap();
                    }
                    if state.stopping {
                        return;
                    }
                    let job = state.queued.take().unwrap();
                    state.active = Some(job.cancel.clone());
                    job
                };
                let result = job.clip.decode(job.index, &|| {
                    if job.cancel.load(Ordering::Relaxed) || job.retired.as_ref().is_some_and(|flag|flag.load(Ordering::Relaxed)) {
                        Err("Video frame decode cancelled.".into())
                    } else {
                        Ok(())
                    }
                });
                let wake = {
                    let mut state = background.0.lock().unwrap();
                    state.active = None;
                    if state.stopping {
                        return;
                    }
                    if job.cancel.load(Ordering::Relaxed) || job.retired.as_ref().is_some_and(|flag|flag.load(Ordering::Relaxed)) {
                        continue;
                    }
                    state.finished = Some(Completion {
                        clip: job.clip,
                        generation: job.generation,
                        index: job.index,
                        result,
                    });
                    state.wake.clone()
                };
                wake();
            })
            .map_err(|error| format!("Could not start video decoder: {error}"))?;
        Ok(Self { shared })
    }
    pub fn set_wake(&self, wake: crate::loader::Wake) {
        self.shared.0.lock().unwrap().wake = wake;
    }
    #[cfg(test)]
    pub fn submit(&self, clip: Arc<Clip>, generation: u64, index: usize) {
        self.submit_for_source(clip,generation,index,None);
    }
    pub fn submit_for_source(&self,clip:Arc<Clip>,generation:u64,index:usize,retired:Option<Arc<AtomicBool>>) {
        let mut state = self.shared.0.lock().unwrap();
        if let Some(cancel) = &state.active {
            cancel.store(true, Ordering::Relaxed);
        }
        state.finished = None;
        state.queued = Some(Job {
            clip,
            generation,
            index,
            cancel: Arc::new(AtomicBool::new(false)),
            retired,
        });
        self.shared.1.notify_one();
    }
    pub fn take(&self) -> Option<Completion> {
        self.shared.0.lock().unwrap().finished.take()
    }
    pub fn cancel(&self) {
        let mut state = self.shared.0.lock().unwrap();
        state.queued = None;
        state.finished = None;
        if let Some(cancel) = &state.active {
            cancel.store(true, Ordering::Relaxed);
        }
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
    const SCENE: &[u8] = include_bytes!("../tools/fixtures/scene-intra.ivf");
    fn clip() -> Clip {
        Clip::parse(SCENE.to_vec(), Limits::default(), &|| Ok(())).unwrap()
    }
    #[test]
    fn local_video_real_container_and_keyframes_have_distinct_decoded_pixels() {
        let clip = clip();
        assert_eq!((clip.width, clip.height, clip.frame_count()), (64, 48, 6));
        assert_eq!(clip.duration, Duration::from_secs(3));
        let mut signatures = std::collections::HashSet::new();
        for index in 0..clip.frame_count() {
            let bitmap = clip.decode(index, &|| Ok(())).unwrap();
            assert_eq!(bitmap.pixels.len(), 64 * 48);
            signatures.insert(crate::compression::crc32(
                &bitmap
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_be_bytes())
                    .collect::<Vec<_>>(),
            ));
        }
        assert_eq!(signatures.len(), 6);
        assert_eq!(clip.index_at(Duration::from_millis(1200)), 2);
    }
    #[test]
    fn local_video_pixels_match_independent_reference_with_bounded_rgb_error() {
        let reference = include_bytes!("../tools/fixtures/scene-intra.rgba");
        let clip = clip();
        for index in 0..6 {
            let bitmap = clip.decode(index, &|| Ok(())).unwrap();
            let mut error = 0u64;
            let mut maximum = 0u32;
            for (pixel, rgba) in bitmap
                .pixels
                .iter()
                .zip(reference[index * 64 * 48 * 4..(index + 1) * 64 * 48 * 4].chunks_exact(4))
            {
                for (channel, reference) in [
                    ((pixel >> 16) & 255, rgba[0]),
                    ((pixel >> 8) & 255, rgba[1]),
                    (pixel & 255, rgba[2]),
                ] {
                    let delta = channel.abs_diff(u32::from(reference));
                    error += u64::from(delta);
                    maximum = maximum.max(delta);
                }
                assert_eq!(pixel >> 24, 255);
            }
            println!(
                "Reference frame {index}: RGB mean error {} max {maximum}",
                error as f64 / (64.0 * 48.0 * 3.0)
            );
            // Existing original decoder uses different chroma upsampling from
            // the independent reference. Bound both the mean and edge error;
            // this is not a bit-exact color-conformance claim.
            assert!(
                error < 64 * 48 * 3 * 4 && maximum <= 80,
                "Independent reference RGB error: {index}: {error}, max {maximum}"
            );
        }
    }
    #[test]
    fn local_video_duplicate_times_initial_pixels_and_clock_eof_are_consistent() {
        let parsed = clip();
        let mut data = SCENE.to_vec();
        let frame = &parsed.frames[1];
        data[frame.bytes.start - 8..frame.bytes.start].copy_from_slice(&0u64.to_le_bytes());
        let asset = Clip::load(data, Limits::default(), &|| Ok(())).unwrap();
        assert_eq!(asset.initial_index, 1);
        assert_eq!(
            asset.first.pixels,
            asset
                .clip
                .decode(asset.clip.index_at(Duration::ZERO), &|| Ok(()))
                .unwrap()
                .pixels
        );
        let now = Instant::now();
        let mut clock = Clock::new(Duration::from_secs(1));
        clock.play(now);
        clock.pause(now + Duration::from_secs(2));
        assert!(clock.ended && !clock.playing());
        // Different ticks rounding to one nanosecond select a consistent poster too.
        let mut data = SCENE.to_vec();
        data[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        let asset = Clip::load(data, Limits::default(), &|| Ok(())).unwrap();
        assert_eq!(asset.initial_index, 4);
    }
    #[test]
    fn local_video_backward_interframe_scaling_overflow_and_worker_cancel_recover() {
        let parsed = clip();
        for variant in 0..4 {
            let mut data = SCENE.to_vec();
            match variant {
                0 => {
                    let frame = &parsed.frames[3];
                    data[frame.bytes.start - 8..frame.bytes.start]
                        .copy_from_slice(&0u64.to_le_bytes());
                }
                1 => data[parsed.frames[2].bytes.start] |= 1,
                2 => data[parsed.frames[2].bytes.start + 7] |= 0x40,
                _ => {
                    let frame = &parsed.frames[5];
                    data[frame.bytes.start - 8..frame.bytes.start]
                        .copy_from_slice(&u64::MAX.to_le_bytes());
                }
            }
            assert!(Clip::parse(data, Limits::default(), &|| Ok(())).is_err());
        }
        assert!(Limits::default().pixels > crate::image::MAX_PIXELS);
        let clip = Arc::new(clip());
        let worker = Worker::new(Arc::new(|| {})).unwrap();
        worker.submit(clip.clone(), 1, 4);
        worker.cancel();
        std::thread::sleep(Duration::from_millis(10));
        assert!(worker.take().is_none());
        worker.submit(clip.clone(), 2, 5);
        let deadline = Instant::now() + Duration::from_secs(3);
        let result = loop {
            if let Some(result) = worker.take() {
                break result;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        };
        assert!(Arc::ptr_eq(&clip, &result.clip));
        assert_eq!((result.generation, result.index), (2, 5));
        assert!(result.result.is_ok());
    }
    #[test]
    fn local_video_malformed_headers_counts_payloads_timebases_and_small_profiles_refuse_whole_clip(
    ) {
        for length in [0, 4, 31, 35, SCENE.len() - 1] {
            assert!(Clip::parse(SCENE[..length].to_vec(), Limits::default(), &|| Ok(())).is_err());
        }
        for (at, value) in [(0, b'X'), (4, 1), (8, b'X'), (16, 0), (20, 0)] {
            let mut data = SCENE.to_vec();
            if matches!(at, 16 | 20) {
                data[at..at + 4].fill(0);
            } else {
                data[at] = value;
            }
            assert!(Clip::parse(data, Limits::default(), &|| Ok(())).is_err());
        }
        let mut data = SCENE.to_vec();
        data[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Clip::parse(data, Limits::default(), &|| Ok(())).is_err());
        for limits in [
            Limits {
                input_bytes: 64,
                ..Limits::default()
            },
            Limits {
                frame_bytes: 1,
                ..Limits::default()
            },
            Limits {
                frames: 2,
                ..Limits::default()
            },
            Limits {
                pixels: 1,
                ..Limits::default()
            },
            Limits {
                duration: Duration::from_millis(100),
                ..Limits::default()
            },
        ] {
            assert!(Clip::parse(SCENE.to_vec(), limits, &|| Ok(())).is_err());
        }
        assert!(Clip::parse(SCENE.to_vec(), Limits::default(), &|| Err(
            "cancelled".into()
        ))
        .unwrap_err()
        .contains("cancelled"));
        assert!(clip()
            .decode(1, &|| Err("cancelled".into()))
            .unwrap_err()
            .contains("cancelled"));
    }
    #[test]
    fn local_video_clock_pause_seek_end_replay_and_nonuniform_timestamps_are_real() {
        let start = Instant::now();
        let mut clock = Clock::new(Duration::from_secs(3));
        clock.play(start);
        assert_eq!(
            clock.position(start + Duration::from_millis(600)),
            Duration::from_millis(600)
        );
        clock.pause(start + Duration::from_millis(600));
        assert_eq!(
            clock.position(start + Duration::from_secs(10)),
            Duration::from_millis(600)
        );
        clock.seek(Duration::from_secs(2), start);
        assert!(!clock.playing());
        clock.play(start);
        assert!(clock.tick(start + Duration::from_secs(2)));
        assert!(clock.ended && !clock.playing());
        assert_eq!(clock.position(start), Duration::from_secs(3));
        clock.play(start);
        assert_eq!(clock.position(start), Duration::ZERO);
        let parsed = clip();
        let mut data = SCENE.to_vec();
        let stamps = [10u64, 11, 13, 14, 18, 20];
        for (frame, stamp) in parsed.frames.iter().zip(stamps) {
            data[frame.bytes.start - 8..frame.bytes.start].copy_from_slice(&stamp.to_le_bytes());
        }
        let clip = Clip::parse(data, Limits::default(), &|| Ok(())).unwrap();
        assert_eq!(clip.duration, Duration::from_secs(6));
        assert_eq!(clip.index_at(Duration::from_millis(1400)), 1);
        assert_eq!(clip.index_at(Duration::from_millis(1600)), 2);
    }
}
