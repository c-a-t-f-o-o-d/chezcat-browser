//! Low-level PCM device presentation. Original decoding lives in audio.rs.
//! Headers and PCM storage stay at stable addresses until unprepared by the OS.
use crate::audio::Clip;
use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn source_is_retired(source: Option<&Arc<AtomicBool>>) -> bool {
    source.is_some_and(|source| source.load(Ordering::Relaxed))
}
pub(crate) fn same_source(left: Option<&Arc<AtomicBool>>, right: Option<&Arc<AtomicBool>>) -> bool {
    match (left, right) { (None, None) => true, (Some(left), Some(right)) => Arc::ptr_eq(left, right), _ => false }
}

const HEADERS: usize = 3;
const BLOCK: usize = 4096;
const STILL_PLAYING: u32 = 33;
#[derive(Debug)]
enum OutputFault {
    Cancelled,
    Failure(String),
}
impl From<String> for OutputFault {
    fn from(value: String) -> Self {
        Self::Failure(value)
    }
}
impl From<&str> for OutputFault {
    fn from(value: &str) -> Self {
        Self::Failure(value.into())
    }
}
type Handle = *mut c_void;

#[repr(C, packed(1))]
struct Format {
    kind: u16,
    channels: u16,
    rate: u32,
    byte_rate: u32,
    align: u16,
    bits: u16,
    extra: u16,
}
#[repr(C, packed(1))]
struct Header {
    data: *mut u8,
    bytes: u32,
    recorded: u32,
    user: usize,
    flags: u32,
    loops: u32,
    next: *mut Header,
    reserved: usize,
}
#[repr(C, packed(1))]
struct Time {
    kind: u32,
    data: [u8; 8],
}

// This fixed interface also permits offline ownership/failure controls. The
// production implementation below calls only the native PCM presentation APIs.
trait Api {
    fn open(&mut self, format: &Format) -> Result<Handle, String>;
    unsafe fn prepare(&mut self, handle: Handle, header: *mut Header) -> u32;
    unsafe fn write(&mut self, handle: Handle, header: *mut Header) -> u32;
    unsafe fn unprepare(&mut self, handle: Handle, header: *mut Header) -> u32;
    fn pause(&mut self, handle: Handle) -> u32;
    fn restart(&mut self, handle: Handle) -> u32;
    fn reset(&mut self, handle: Handle) -> u32;
    fn position(&mut self, handle: Handle) -> Result<(u32, u32), String>;
    fn close(&mut self, handle: Handle) -> u32;
}

#[cfg(windows)]
struct Native;
#[cfg(windows)]
#[link(name = "winmm")]
extern "system" {
    fn waveOutOpen(
        handle: *mut Handle,
        device: u32,
        format: *const Format,
        callback: usize,
        instance: usize,
        flags: u32,
    ) -> u32;
    fn waveOutPrepareHeader(handle: Handle, header: *mut Header, size: u32) -> u32;
    fn waveOutWrite(handle: Handle, header: *mut Header, size: u32) -> u32;
    fn waveOutUnprepareHeader(handle: Handle, header: *mut Header, size: u32) -> u32;
    fn waveOutPause(handle: Handle) -> u32;
    fn waveOutRestart(handle: Handle) -> u32;
    fn waveOutReset(handle: Handle) -> u32;
    fn waveOutGetPosition(handle: Handle, time: *mut Time, size: u32) -> u32;
    fn waveOutClose(handle: Handle) -> u32;
}
fn result(code: u32, operation: &str) -> Result<(), String> {
    if code == 0 {
        Ok(())
    } else {
        Err(format!("PCM device {operation} failed (MMRESULT {code})."))
    }
}
#[cfg(windows)]
impl Api for Native {
    fn open(&mut self, format: &Format) -> Result<Handle, String> {
        let mut handle = std::ptr::null_mut();
        unsafe {
            result(waveOutOpen(&mut handle, u32::MAX, format, 0, 0, 8), "open")?;
        }
        if handle.is_null() {
            return Err("PCM device returned a null handle.".into());
        }
        Ok(handle)
    }
    unsafe fn prepare(&mut self, h: Handle, p: *mut Header) -> u32 {
        waveOutPrepareHeader(h, p, std::mem::size_of::<Header>() as u32)
    }
    unsafe fn write(&mut self, h: Handle, p: *mut Header) -> u32 {
        waveOutWrite(h, p, std::mem::size_of::<Header>() as u32)
    }
    unsafe fn unprepare(&mut self, h: Handle, p: *mut Header) -> u32 {
        waveOutUnprepareHeader(h, p, std::mem::size_of::<Header>() as u32)
    }
    fn pause(&mut self, h: Handle) -> u32 {
        unsafe { waveOutPause(h) }
    }
    fn restart(&mut self, h: Handle) -> u32 {
        unsafe { waveOutRestart(h) }
    }
    fn reset(&mut self, h: Handle) -> u32 {
        unsafe { waveOutReset(h) }
    }
    fn close(&mut self, h: Handle) -> u32 {
        unsafe { waveOutClose(h) }
    }
    fn position(&mut self, h: Handle) -> Result<(u32, u32), String> {
        let mut time = Time {
            kind: 2,
            data: [0; 8],
        };
        unsafe {
            result(
                waveOutGetPosition(h, &mut time, std::mem::size_of::<Time>() as u32),
                "position",
            )?;
        }
        Ok((
            time.kind,
            u32::from_le_bytes(time.data[..4].try_into().unwrap()),
        ))
    }
}

struct Buffer {
    header: Vec<UnsafeCell<Header>>,
    _samples: Vec<i16>,
    frames: u64,
    written: bool,
}
impl Buffer {
    fn new(mut samples: Vec<i16>, channels: u16) -> Result<Self, String> {
        let frames = u64::try_from(samples.len() / usize::from(channels))
            .map_err(|_| "PCM buffer frame count overflow.")?;
        let bytes = samples
            .len()
            .checked_mul(2)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("PCM output buffer byte count overflow.")?;
        let mut header = Vec::new();
        header
            .try_reserve_exact(1)
            .map_err(|_| "Could not reserve a stable PCM header.")?;
        header.push(UnsafeCell::new(Header {
            data: samples.as_mut_ptr().cast(),
            bytes,
            recorded: 0,
            user: 0,
            flags: 0,
            loops: 0,
            next: std::ptr::null_mut(),
            reserved: 0,
        }));
        Ok(Self {
            header,
            _samples: samples,
            frames,
            written: false,
        })
    }
    fn pointer(&self) -> *mut Header {
        self.header.as_ptr().cast::<Header>().cast_mut()
    }
}

#[derive(Default)]
struct Counter {
    unit: Option<u32>,
    last: u32,
    total: u64,
}
impl Counter {
    fn update(
        &mut self,
        unit: u32,
        value: u32,
        channels: u16,
        rate: u32,
        submitted: u64,
    ) -> Result<u64, String> {
        if !matches!(unit, 1 | 2 | 4) {
            return Err("PCM device returned an unsupported position unit.".into());
        }
        if let Some(old) = self.unit {
            if old != unit {
                return Err("PCM device changed position units without a reset.".into());
            }
        }
        let delta = if self.unit.is_some() {
            u64::from(value.wrapping_sub(self.last))
        } else {
            u64::from(value)
        };
        let total = self
            .total
            .checked_add(delta)
            .ok_or("PCM device position overflow.")?;
        let frames = match unit {
            2 => total,
            4 => total / u64::from(channels) / 2,
            1 => u64::try_from(u128::from(total) * u128::from(rate) / 1000)
                .map_err(|_| "PCM position conversion overflow.")?,
            _ => unreachable!(),
        };
        let precision = if unit == 1 {
            u64::from(rate).div_ceil(1000)
        } else {
            0
        };
        if frames > submitted.saturating_add(precision) {
            return Err(
                "PCM device position exceeds submitted audio; no playback time was fabricated."
                    .into(),
            );
        }
        self.unit = Some(unit);
        self.last = value;
        self.total = total;
        Ok(frames.min(submitted))
    }
}

struct Output<A: Api> {
    api: A,
    handle: Handle,
    queue: VecDeque<Buffer>,
    pub clip: Arc<Clip>,
    origin: u64,
    next: u64,
    submitted: u64,
    consumed: u64,
    retired: u64,
    counter: Counter,
    pub playing: bool,
    pub ended: bool,
    failed: bool,
    source_retired: Option<Arc<AtomicBool>>,
    retirement_handled: bool,
}
impl<A: Api> Output<A> {
    #[cfg(test)]
    fn open(api: A, clip: Arc<Clip>, frame: u64) -> Result<Self, String> {
        Self::open_for_source(api, clip, frame, None).map_err(|fault| match fault {
            OutputFault::Cancelled => "PCM source retired before device open.".into(),
            OutputFault::Failure(error) => error,
        })
    }
    fn open_for_source(mut api: A, clip: Arc<Clip>, frame: u64, source_retired: Option<Arc<AtomicBool>>) -> Result<Self, OutputFault> {
        if source_is_retired(source_retired.as_ref()) {return Err(OutputFault::Cancelled);}
        let align = clip
            .channels
            .checked_mul(2)
            .ok_or("PCM output alignment overflow.")?;
        let byte_rate = clip
            .sample_rate
            .checked_mul(u32::from(align))
            .ok_or("PCM output byte-rate overflow.")?;
        let format = Format {
            kind: 1,
            channels: clip.channels,
            rate: clip.sample_rate,
            byte_rate,
            align,
            bits: 16,
            extra: 0,
        };
        let handle = api.open(&format)?;
        let output = Self {
            api,
            handle,
            queue: VecDeque::new(),
            origin: frame.min(clip.frames),
            next: frame.min(clip.frames),
            submitted: 0,
            consumed: 0,
            retired: 0,
            counter: Counter::default(),
            playing: false,
            ended: false,
            failed: false,
            source_retired,
            retirement_handled: false,
            clip,
        };
        Ok(output)
    }
    fn update_position(&mut self) -> Result<(), String> {
        let (unit, value) = self.api.position(self.handle)?;
        self.consumed = self.counter.update(
            unit,
            value,
            self.clip.channels,
            self.clip.sample_rate,
            self.submitted,
        )?;
        Ok(())
    }
    fn retire(&mut self) -> Result<(), String> {
        while let Some(buffer) = self.queue.front() {
            let code = unsafe { self.api.unprepare(self.handle, buffer.pointer()) };
            if code == STILL_PLAYING {
                break;
            }
            result(code, "unprepare")?;
            let buffer = self.queue.pop_front().unwrap();
            if buffer.written {
                self.retired = self
                    .retired
                    .checked_add(buffer.frames)
                    .ok_or("PCM retired-frame count overflow.")?;
            }
        }
        Ok(())
    }
    fn fill(&mut self, check: &dyn Fn() -> Result<(), String>) -> Result<(), OutputFault> {
        while self.queue.len() < HEADERS && self.next < self.clip.frames {
            check().map_err(|_| OutputFault::Cancelled)?;
            let samples = self
                .clip
                .decode(self.next, BLOCK.min(self.clip.decode_block_frames()), check)
                .map_err(|error| {
                    if check().is_err() {
                        OutputFault::Cancelled
                    } else {
                        OutputFault::Failure(error)
                    }
                })?;
            if samples.is_empty() {
                return Err("PCM decoder made no progress before end of audio.".into());
            }
            let buffer = Buffer::new(samples, self.clip.channels)?;
            self.queue
                .try_reserve(1)
                .map_err(|_| "Could not reserve a PCM queue slot.")?;
            check().map_err(|_| OutputFault::Cancelled)?;
            result(
                unsafe { self.api.prepare(self.handle, buffer.pointer()) },
                "prepare",
            )?;
            // Publish ownership before Write, including the Write failure path.
            self.queue.push_back(buffer);
            // A prepared native header must stay owned even if retirement was
            // observed during Prepare. Cleanup resets/unprepares it safely.
            check().map_err(|_| OutputFault::Cancelled)?;
            let buffer = self.queue.back_mut().unwrap();
            result(
                unsafe { self.api.write(self.handle, buffer.pointer()) },
                "write",
            )?;
            buffer.written = true;
            self.next = self
                .next
                .checked_add(buffer.frames)
                .ok_or("PCM queue frame position overflow.")?;
            self.submitted = self
                .submitted
                .checked_add(buffer.frames)
                .ok_or("PCM submitted count overflow.")?;
            check().map_err(|_| OutputFault::Cancelled)?;
        }
        Ok(())
    }
    fn play(&mut self, check: &dyn Fn() -> Result<(), String>) -> Result<(), OutputFault> {
        if self.failed {
            return Err(
                "PCM output is retaining failed device ownership; reset must succeed before reuse."
                    .into(),
            );
        }
        if self.ended {
            self.reset(0)?;
        }
        if self.queue.is_empty() {
            result(self.api.pause(self.handle), "prepare paused playback")?;
        }
        self.fill(check)?;
        check().map_err(|_| OutputFault::Cancelled)?;
        result(self.api.restart(self.handle), "restart")?;
        self.playing = true;
        self.ended = false;
        Ok(())
    }
    fn pause(&mut self) -> Result<(), String> {
        result(self.api.pause(self.handle), "pause")?;
        self.playing = false;
        self.update_position()
    }
    fn tick(&mut self, check: &dyn Fn() -> Result<(), String>) -> Result<(), OutputFault> {
        check().map_err(|_| OutputFault::Cancelled)?;
        if self.failed {
            return Err("PCM device remains in failed ownership state.".into());
        }
        if self.playing {
            self.update_position()?;
            self.retire()?;
            self.fill(check)?;
            if self.next == self.clip.frames && self.queue.is_empty() {
                if self.retired != self.submitted {
                    return Err("PCM retirement count differs from submitted audio.".into());
                }
                // Returning every written header proves drain, including when
                // a device's MMTIME fallback has only millisecond precision.
                self.consumed = self.submitted;
                self.playing = false;
                self.ended = true;
            }
        }
        Ok(())
    }
    fn reset(&mut self, frame: u64) -> Result<(), String> {
        self.playing = false;
        self.ended = false;
        self.failed = true;
        result(self.api.reset(self.handle), "reset")?;
        self.retire()?;
        if !self.queue.is_empty() {
            self.failed = true;
            return Err(
                "PCM reset has not returned all prepared buffers; ownership is retained.".into(),
            );
        }
        self.origin = frame.min(self.clip.frames);
        self.next = self.origin;
        self.submitted = 0;
        self.consumed = 0;
        self.retired = 0;
        self.counter = Counter::default();
        self.failed = false;
        Ok(())
    }
    fn frame(&self) -> u64 {
        self.origin
            .saturating_add(self.consumed)
            .min(self.clip.frames)
    }
    fn fail(&mut self) {
        self.failed = true;
        self.playing = false;
        let _ = self.api.pause(self.handle);
    }
    fn close(&mut self) -> Result<(), String> {
        if source_is_retired(self.source_retired.as_ref()) {self.retirement_handled = true;}
        if self.handle.is_null() {
            return Ok(());
        }
        self.reset(0)?;
        result(self.api.close(self.handle), "close")?;
        self.handle = std::ptr::null_mut();
        Ok(())
    }
}
impl<A: Api> Drop for Output<A> {
    fn drop(&mut self) {
        if self.close().is_err() {
            // A failed driver must never receive dangling prepared-header/PCM pointers.
            // Production keeps one failed Output alive and refuses replacement. At
            // process shutdown only, retain this finite queue rather than freeing it.
            let retained = std::mem::take(&mut self.queue);
            std::mem::forget(retained);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Play { from: u64 },
    Pause { from: u64 },
    Seek { frame: u64, resume: bool },
    Stop,
}
impl Action {
    fn frame(self) -> u64 {
        match self {
            Self::Play { from } | Self::Pause { from } => from,
            Self::Seek { frame, .. } => frame,
            Self::Stop => 0,
        }
    }
}
pub struct Snapshot {
    pub clip: Arc<Clip>,
    pub generation: u64,
    pub source_retired: Option<Arc<AtomicBool>>,
    pub frame: u64,
    pub playing: bool,
    pub ended: bool,
    pub opened: bool,
    pub queued: usize,
    pub submitted: u64,
    pub retired: u64,
    pub position_unit: Option<u32>,
    pub error: Option<String>,
    pub retained_buffers: usize,
}
impl Snapshot {
    pub(crate) fn matches_source(&self, source: Option<&Arc<AtomicBool>>) -> bool {
        !source_is_retired(self.source_retired.as_ref()) && !source_is_retired(source)
            && same_source(self.source_retired.as_ref(), source)
    }
}
struct Request {
    clip: Option<Arc<Clip>>,
    generation: u64,
    action: Action,
    cancel: Arc<std::sync::atomic::AtomicBool>,
    source_retired: Option<Arc<AtomicBool>>,
}
struct Shared {
    request: Option<Request>,
    snapshot: Option<Snapshot>,
    notice: Option<String>,
    active: Option<Arc<std::sync::atomic::AtomicBool>>,
    stopping: bool,
    wake: crate::loader::Wake,
    opened: bool,
    retained: usize,
    busy: bool,
}
pub struct Worker {
    shared: Arc<(std::sync::Mutex<Shared>, std::sync::Condvar)>,
}
impl Worker {
    pub fn new(wake: crate::loader::Wake) -> Result<Self, String> {
        #[cfg(windows)]
        {
            Self::with_factory(wake, || Native)
        }
        #[cfg(not(windows))]
        {
            let _ = wake;
            Err("PCM device presentation is implemented only on Windows.".into())
        }
    }
    fn with_factory<A: Api + 'static, F: FnMut() -> A + Send + 'static>(
        wake: crate::loader::Wake,
        mut factory: F,
    ) -> Result<Self, String> {
        use std::sync::atomic::{AtomicBool, Ordering};
        let shared = Arc::new((
            std::sync::Mutex::new(Shared {
                request: None,
                snapshot: None,
                notice: None,
                active: None,
                stopping: false,
                wake,
                opened: false,
                retained: 0,
                busy: false,
            }),
            std::sync::Condvar::new(),
        ));
        let background = shared.clone();
        std::thread::Builder::new()
            .name("pcm-device-owner".into())
            .spawn(move || {
                let mut output: Option<Output<A>> = None;
                let mut context: Option<(Arc<Clip>, u64, u64, Option<Arc<AtomicBool>>)> = None;
                let mut cancel = Arc::new(AtomicBool::new(false));
                loop {
                    let request = {
                        let mut state = background.0.lock().unwrap();
                        while state.request.is_none() && !state.stopping {
                            if output.as_ref().is_some_and(|output| output.playing)
                                || context.as_ref().is_some_and(|(_,_,_,source)| source.is_some())
                                || output.as_ref().is_some_and(|output| output.source_retired.is_some() && !output.retirement_handled) {
                                let (next, timeout) = background
                                    .1
                                    .wait_timeout(state, std::time::Duration::from_millis(10))
                                    .unwrap();
                                state = next;
                                if timeout.timed_out() {
                                    break;
                                }
                            } else {
                                state = background.1.wait(state).unwrap();
                            }
                        }
                        if state.stopping {
                            break;
                        }
                        let request = state.request.take();
                        if let Some(request) = &request {
                            state.active = Some(request.cancel.clone());
                        }
                        request
                    };
                    let retiring = context.as_ref().is_some_and(|(_,_,_,source)| source_is_retired(source.as_ref()))
                        || output.as_ref().is_some_and(|output| !output.retirement_handled && source_is_retired(output.source_retired.as_ref()));
                    if request.is_none() && !retiring && !output.as_ref().is_some_and(|output| output.playing) {
                        // Fenced paused/metadata owners poll only their token.
                        // Do not republish snapshots or retry a failed device.
                        continue;
                    }
                    let mut error = None;
                    if let Some(request) = request {
                        cancel = request.cancel;
                        let source = request.source_retired;
                        let check = || {
                            if cancel.load(Ordering::Relaxed) || source_is_retired(source.as_ref()) {
                                Err("PCM command cancelled.".into())
                            } else {
                                Ok(())
                            }
                        };
                        if let Some(clip) = request.clip {
                            let frame = request.action.frame().min(clip.frames);
                            context = Some((clip.clone(), request.generation, frame, source.clone()));
                            let operation = (|| {
                                check().map_err(|_|OutputFault::Cancelled)?;
                                if output
                                    .as_ref()
                                    .is_some_and(|output| !Arc::ptr_eq(&output.clip, &clip)
                                        || !same_source(output.source_retired.as_ref(), source.as_ref()))
                                {
                                    output.as_mut().unwrap().close()?;
                                    output = None;
                                }
                                let needs_device = matches!(
                                    request.action,
                                    Action::Play { .. } | Action::Seek { resume: true, .. }
                                );
                                if output.is_none() && needs_device {
                                    check().map_err(|_|OutputFault::Cancelled)?;
                                    output = Some(Output::open_for_source(factory(), clip.clone(), frame, source.clone())?);
                                }
                                check().map_err(|_|OutputFault::Cancelled)?;
                                if let Some(output) = &mut output {
                                    match request.action {
                                        Action::Play { from } => {
                                            if output.failed {
                                                output.reset(if from>=output.clip.frames{0}else{from})?;
                                            } else if output.frame()>=output.clip.frames {
                                                output.reset(0)?;
                                            }
                                            output.play(&check)?;
                                        }
                                        Action::Pause { .. } => output.pause()?,
                                        Action::Seek { frame, resume } => {
                                            output.reset(frame)?;
                                            if resume {
                                                output.play(&check)?;
                                            }
                                        }
                                        Action::Stop => output.reset(0)?,
                                    }
                                }
                                Ok::<(), OutputFault>(())
                            })();
                            if let Err(OutputFault::Failure(failure)) = operation {
                                if let Some(active)=&mut output{active.fail();}
                                if !cancel.load(Ordering::Relaxed) {
                                    error = Some(failure);
                                }
                            }
                        } else {
                            context = None;
                            if let Some(active) = &mut output {
                                match active.close() {
                                    Ok(()) => output = None,
                                    Err(failure) => error = Some(failure),
                                }
                            }
                        }
                    } else if !cancel.load(Ordering::Relaxed) {
                        if let Some(active) = output.as_mut().filter(|output|output.playing) {
                            let source = active.source_retired.clone();
                            if let Err(OutputFault::Failure(failure)) = active.tick(&|| {
                                if cancel.load(Ordering::Relaxed) || source_is_retired(source.as_ref()) {
                                    Err("PCM command cancelled.".into())
                                } else {
                                    Ok(())
                                }
                            }) {
                                active.fail();
                                if !cancel.load(Ordering::Relaxed) {
                                    error = Some(failure);
                                }
                            }
                        }
                    }
                    if context.as_ref().is_some_and(|(_,_,_,source)| source_is_retired(source.as_ref())) {
                        context = None;
                    }
                    let mut retirement_notice = None;
                    if output.as_ref().is_some_and(|output| !output.retirement_handled && source_is_retired(output.source_retired.as_ref())) {
                        match output.as_mut().unwrap().close() {
                            Ok(()) => output = None,
                            Err(failure) => {
                                // close marks retirement handled even on reset/
                                // unprepare failure. Retain the native owner and
                                // buffers; only a later explicit command retries.
                                retirement_notice = Some(failure.clone());
                                error = Some(failure);
                            }
                        }
                    }
                    if error.is_some() {
                        if let Some(active) = &mut output {
                            active.fail();
                        }
                    }
                    let snapshot = context.as_ref().filter(|(_,_,_,source)| !source_is_retired(source.as_ref())).map(|(clip, generation, frame, source)| {
                        let active = output
                            .as_ref()
                            .filter(|output| Arc::ptr_eq(&output.clip, clip)
                                && same_source(output.source_retired.as_ref(), source.as_ref()));
                        Snapshot {
                            clip: clip.clone(),
                            generation: *generation,
                            source_retired: source.clone(),
                            frame: active.map_or(*frame, Output::frame),
                            playing: active.is_some_and(|output| output.playing),
                            ended: active.is_some_and(|output| output.ended),
                            opened: active.is_some(),
                            queued: active.map_or(0, |output| output.queue.len()),
                            submitted: active.map_or(0, |output| output.submitted),
                            retired: active.map_or(0, |output| output.retired),
                            position_unit: active.and_then(|output| output.counter.unit),
                            error: error.clone().or_else(||active.filter(|output|output.failed).map(|_|"PCM device ownership remains failed; Stop/reset must succeed before playback.".into())),
                            retained_buffers: output
                                .as_ref()
                                .map_or(0, |output| output.queue.len()),
                        }
                    });
                    let wake = {
                        let mut state = background.0.lock().unwrap();
                        state.opened=output.as_ref().is_some_and(|output|!output.handle.is_null());
                        state.retained=output.as_ref().map_or(0,|output|output.queue.len());
                        state.busy=state.request.is_some();
                        if state.stopping {
                            break;
                        }
                        if let Some(notice) = retirement_notice {state.notice = Some(notice);}
                        if state.snapshot.as_ref().is_some_and(|snapshot|source_is_retired(snapshot.source_retired.as_ref())) {state.snapshot = None;}
                        if cancel.load(Ordering::Relaxed) {
                            continue;
                        }
                        if let Some(error) = error {
                            state.notice = Some(error);
                        }
                        state.snapshot = snapshot.filter(|snapshot|!source_is_retired(snapshot.source_retired.as_ref()));
                        state.wake.clone()
                    };
                    wake();
                }
                // Output and all native device ownership are destroyed on this same
                // thread. Its Drop retains any buffers a failed driver still owns.
            })
            .map_err(|error| format!("Could not start the PCM device worker: {error}"))?;
        Ok(Self { shared })
    }
    fn submit_request(&self, clip: Option<Arc<Clip>>, generation: u64, action: Action, source_retired: Option<Arc<AtomicBool>>) {
        use std::sync::atomic::{AtomicBool, Ordering};
        let mut state = self.shared.0.lock().unwrap();
        // An already-retired late request cannot replace/cancel a newer source,
        // even if a caller reused the same clip and numeric generation.
        if source_is_retired(source_retired.as_ref()) {return;}
        if let Some(active) = &state.active {
            active.store(true, Ordering::Relaxed);
        }
        state.snapshot = None;
        state.busy = true;
        state.request = Some(Request {
            clip,
            generation,
            action,
            cancel: Arc::new(AtomicBool::new(false)),
            source_retired,
        });
        self.shared.1.notify_one();
    }
    pub fn submit(&self, clip: Arc<Clip>, generation: u64, action: Action) {
        self.submit_for_source(clip, generation, action, None);
    }
    pub fn submit_for_source(&self, clip: Arc<Clip>, generation: u64, action: Action, source_retired: Option<Arc<AtomicBool>>) {
        self.submit_request(Some(clip), generation, action, source_retired);
    }
    pub fn clear(&self) {
        self.submit_request(None, 0, Action::Stop, None);
    }
    pub fn poll(&self) -> (Option<Snapshot>, Option<String>) {
        let mut state = self.shared.0.lock().unwrap();
        (state.snapshot.take().filter(|snapshot|!source_is_retired(snapshot.source_retired.as_ref())), state.notice.take())
    }
    pub fn set_wake(&self, wake: crate::loader::Wake) {
        self.shared.0.lock().unwrap().wake = wake;
    }
    pub fn ownership(&self) -> (bool, usize, bool) {
        let state = self.shared.0.lock().unwrap();
        (state.opened, state.retained, state.busy)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering;
        let mut state = self.shared.0.lock().unwrap();
        state.stopping = true;
        if let Some(active) = &state.active {
            active.store(true, Ordering::Relaxed);
        }
        self.shared.1.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    impl Worker {
        pub(crate) fn mock_for_app(wake: crate::loader::Wake) -> Result<Self, String> {
            Self::with_factory(wake, Mock::default)
        }
        pub(crate) fn mock_for_app_write_failure(wake: crate::loader::Wake) -> Result<Self, String> {
            Self::with_factory(wake, || Mock { write_error: true, ..Mock::default() })
        }
    }
    #[derive(Default)]
    struct Mock {
        prepared: std::collections::HashMap<usize, (usize, u32, bool)>,
        position: u32,
        return_all: bool,
        write_error: bool,
        reset_error: bool,
        running: bool,
        write_gate: Option<Arc<(std::sync::Mutex<(bool, bool)>, std::sync::Condvar)>>,
    }
    impl Api for Mock {
        fn open(&mut self, format: &Format) -> Result<Handle, String> {
            let bits = format.bits;
            assert_eq!(bits, 16);
            Ok(1usize as Handle)
        }
        unsafe fn prepare(&mut self, _: Handle, header: *mut Header) -> u32 {
            let header_value = std::ptr::read_unaligned(header);
            assert!(self
                .prepared
                .insert(
                    header as usize,
                    (header_value.data as usize, header_value.bytes, false)
                )
                .is_none());
            0
        }
        unsafe fn write(&mut self, _: Handle, header: *mut Header) -> u32 {
            if let Some(gate) = &self.write_gate {
                let mut state = gate.0.lock().unwrap();
                state.0 = true;
                gate.1.notify_all();
                while !state.1 {
                    state = gate.1.wait(state).unwrap();
                }
            }
            let entry = self.prepared.get_mut(&(header as usize)).unwrap();
            let value = std::ptr::read_unaligned(header);
            assert_eq!((value.data as usize, value.bytes), (entry.0, entry.1));
            if self.write_error {
                return 1;
            }
            entry.2 = true;
            0
        }
        unsafe fn unprepare(&mut self, _: Handle, header: *mut Header) -> u32 {
            let entry = self.prepared.get(&(header as usize)).unwrap();
            let value = std::ptr::read_unaligned(header);
            assert_eq!((value.data as usize, value.bytes), (entry.0, entry.1));
            if entry.2 && !self.return_all {
                return STILL_PLAYING;
            }
            self.prepared.remove(&(header as usize));
            0
        }
        fn pause(&mut self, _: Handle) -> u32 {
            self.running = false;
            0
        }
        fn restart(&mut self, _: Handle) -> u32 {
            self.running = true;
            0
        }
        fn reset(&mut self, _: Handle) -> u32 {
            if self.reset_error {
                return 1;
            }
            self.running = false;
            self.position = 0;
            self.return_all = true;
            0
        }
        fn position(&mut self, _: Handle) -> Result<(u32, u32), String> {
            Ok((2, self.position))
        }
        fn close(&mut self, _: Handle) -> u32 {
            assert!(self.prepared.is_empty());
            0
        }
    }
    fn clip() -> Arc<Clip> {
        Arc::new(
            Clip::parse(
                include_bytes!("../tools/fixtures/original-tone.wav").to_vec(),
                crate::audio::Limits::default(),
                &|| Ok(()),
            )
            .unwrap(),
        )
    }
    fn wait_snapshot(worker: &Worker, generation: u64) -> Snapshot {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if let (Some(snapshot), _) = worker.poll() {
                if snapshot.generation == generation {
                    return snapshot;
                }
            }
            assert!(
                std::time::Instant::now() < deadline,
                "PCM mock worker did not complete"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    #[test]
    fn local_audio_mock_worker_latest_clip_pause_and_seek_use_device_frames_not_wall_time() {
        let worker = Worker::with_factory(Arc::new(|| {}), Mock::default).unwrap();
        let first = clip();
        let current = clip();
        worker.submit(
            first,
            1,
            Action::Seek {
                frame: 100,
                resume: false,
            },
        );
        worker.submit(
            current.clone(),
            2,
            Action::Seek {
                frame: 36000,
                resume: false,
            },
        );
        let snapshot = wait_snapshot(&worker, 2);
        assert!(Arc::ptr_eq(&snapshot.clip, &current));
        assert_eq!(snapshot.frame, 36000);
        assert!(!snapshot.opened && !snapshot.playing);
        worker.submit(current.clone(), 3, Action::Play { from: 36000 });
        let snapshot = wait_snapshot(&worker, 3);
        assert!(snapshot.opened && snapshot.playing && snapshot.queued == 3);
        assert_eq!(snapshot.frame, 36000);
        std::thread::sleep(std::time::Duration::from_millis(35));
        let snapshot = wait_snapshot(&worker, 3);
        assert_eq!(
            snapshot.frame, 36000,
            "wall time must not fabricate native sample progress"
        );
        worker.submit(current.clone(), 4, Action::Pause { from: 0 });
        assert!(!wait_snapshot(&worker, 4).playing);
        worker.submit(
            current.clone(),
            5,
            Action::Seek {
                frame: 48000,
                resume: false,
            },
        );
        let snapshot = wait_snapshot(&worker, 5);
        assert_eq!(snapshot.frame, 48000);
        assert_eq!(snapshot.queued, 0);
        worker.submit(current, 6, Action::Stop);
        assert_eq!(wait_snapshot(&worker, 6).frame, 0);
        worker.clear();
    }
    #[test]
    fn local_audio_mock_worker_latches_write_failure_and_does_not_retry_on_ticks() {
        let worker = Worker::with_factory(Arc::new(|| {}), || Mock {
            write_error: true,
            ..Mock::default()
        })
        .unwrap();
        let clip = clip();
        worker.submit(clip.clone(), 1, Action::Play { from: 0 });
        let snapshot = wait_snapshot(&worker, 1);
        assert!(snapshot.error.is_some() && !snapshot.playing && snapshot.retained_buffers == 1);
        std::thread::sleep(std::time::Duration::from_millis(35));
        assert!(
            worker.poll().0.is_none(),
            "failed output must not automatically resume/retry"
        );
        worker.submit(clip.clone(), 2, Action::Pause { from: 0 });
        assert!(wait_snapshot(&worker, 2).error.is_some());
        worker.submit(clip, 3, Action::Stop);
        let snapshot = wait_snapshot(&worker, 3);
        assert!(snapshot.error.is_none() && snapshot.retained_buffers == 0 && !snapshot.playing);
        worker.clear();
    }
    #[test]
    fn local_audio_mock_native_write_failure_during_cancellation_still_latches_ownership() {
        let gate = Arc::new((
            std::sync::Mutex::new((false, false)),
            std::sync::Condvar::new(),
        ));
        let driver_gate = gate.clone();
        let worker = Worker::with_factory(Arc::new(|| {}), move || Mock {
            write_error: true,
            write_gate: Some(driver_gate.clone()),
            ..Mock::default()
        })
        .unwrap();
        let clip = clip();
        worker.submit(clip.clone(), 1, Action::Play { from: 0 });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut entered = gate.0.lock().unwrap();
        while !entered.0 {
            let (next, _) = gate
                .1
                .wait_timeout(entered, std::time::Duration::from_millis(10))
                .unwrap();
            entered = next;
            assert!(std::time::Instant::now() < deadline);
        }
        drop(entered);
        worker.submit(clip.clone(), 2, Action::Pause { from: 0 });
        {
            let mut state = gate.0.lock().unwrap();
            state.1 = true;
            gate.1.notify_all();
        }
        let snapshot = wait_snapshot(&worker, 2);
        assert!(snapshot.error.is_some() && !snapshot.playing && snapshot.retained_buffers == 1);
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(worker.poll().0.is_none());
        worker.submit(clip, 3, Action::Stop);
        assert!(wait_snapshot(&worker, 3).error.is_none());
        worker.clear();
    }
    #[test]
    fn local_audio_mock_device_stable_ownership_pause_seek_drain_and_replay() {
        let mut output = Output::open(Mock::default(), clip(), 0).unwrap();
        output.play(&|| Ok(())).unwrap();
        assert_eq!(output.queue.len(), HEADERS);
        assert_eq!(output.api.prepared.len(), HEADERS);
        assert!(output.playing && output.api.running);
        output.api.position = 1100;
        output.pause().unwrap();
        assert_eq!(output.frame(), 1100);
        output.api.position = 2200;
        output.tick(&|| Ok(())).unwrap();
        assert_eq!(output.frame(), 1100);
        output.reset(48000).unwrap();
        assert_eq!(output.frame(), 48000);
        assert!(output.queue.is_empty());
        output.api.return_all = false;
        output.play(&|| Ok(())).unwrap();
        for _ in 0..20 {
            output.api.position = output.submitted as u32;
            output.api.return_all = true;
            output.tick(&|| Ok(())).unwrap();
            if output.ended {
                break;
            }
        }
        assert!(output.ended && !output.playing && output.queue.is_empty());
        assert_eq!(output.frame(), 96000);
        output.play(&|| Ok(())).unwrap();
        assert_eq!(output.frame(), 0);
        assert!(output.playing);
        output.close().unwrap();
        assert!(output.handle.is_null());
    }
    #[test]
    fn local_audio_mock_write_and_reset_failures_retain_driver_buffers_until_safe_recovery() {
        let mut output = Output::open(
            Mock {
                write_error: true,
                ..Mock::default()
            },
            clip(),
            0,
        )
        .unwrap();
        assert!(output.play(&|| Ok(())).is_err());
        assert_eq!(output.queue.len(), 1);
        assert_eq!(output.api.prepared.len(), 1);
        output.fail();
        output.api.reset_error = true;
        assert!(output.reset(0).is_err());
        assert_eq!(output.queue.len(), 1);
        assert!(output.failed);
        output.api.reset_error = false;
        output.api.write_error = false;
        output.reset(0).unwrap();
        assert!(output.queue.is_empty() && output.api.prepared.is_empty());
        output.play(&|| Ok(())).unwrap();
        assert_eq!(output.queue.len(), HEADERS);
        output.close().unwrap();
    }
    #[test]
    fn local_audio_device_abi_and_counter_units_wrap_and_bad_progress_are_checked() {
        assert_eq!(std::mem::size_of::<Format>(), 18);
        assert_eq!(std::mem::size_of::<Header>(), 48);
        assert_eq!(std::mem::size_of::<Time>(), 12);
        let mut counter = Counter::default();
        assert_eq!(counter.update(2, 1024, 2, 48000, 2048).unwrap(), 1024);
        assert_eq!(counter.update(2, 2048, 2, 48000, 2048).unwrap(), 2048);
        assert!(counter.update(2, 2049, 2, 48000, 2048).is_err());
        assert!(counter.update(4, 8192, 2, 48000, 4096).is_err());
        let mut bytes = Counter::default();
        assert_eq!(bytes.update(4, 4096, 2, 48000, 1024).unwrap(), 1024);
        let mut millis = Counter::default();
        assert_eq!(millis.update(1, 100, 2, 48000, 4800).unwrap(), 4800);
        let mut wrap = Counter {
            unit: Some(2),
            last: u32::MAX - 7,
            total: u64::from(u32::MAX) - 7,
        };
        assert_eq!(
            wrap.update(2, 8, 2, 48000, u64::from(u32::MAX) + 9)
                .unwrap(),
            u64::from(u32::MAX) + 9
        );
    }
    include!("audio_retirement_tests.rs");
}
