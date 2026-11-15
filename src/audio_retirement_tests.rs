// Original native-buffer ownership protocol tests using the existing Mock Api.
// No real WinMM device is opened and no physical audio/playback is claimed.

#[derive(Clone, Copy, PartialEq, Eq)]
enum RetireAt {
    Never,
    Open,
    Prepare,
    Write,
    Refill,
}
struct FencedMock {
    inner: Mock,
    source: Arc<AtomicBool>,
    phase: RetireAt,
    prepared: usize,
    calls: Arc<std::sync::Mutex<Vec<&'static str>>>,
    fail_reset: Arc<AtomicBool>,
    fail_unprepare: Arc<AtomicBool>,
}
impl FencedMock {
    fn log(&self, name: &'static str) {
        self.calls.lock().unwrap().push(name);
    }
}
impl Api for FencedMock {
    fn open(&mut self, format: &Format) -> Result<Handle, String> {
        self.log("open");
        let handle = self.inner.open(format)?;
        if self.phase == RetireAt::Open {
            self.source.store(true, Ordering::Relaxed);
        }
        Ok(handle)
    }
    unsafe fn prepare(&mut self, h: Handle, p: *mut Header) -> u32 {
        self.log("prepare");
        let result = self.inner.prepare(h, p);
        self.prepared += 1;
        if self.phase == RetireAt::Prepare
            || self.phase == RetireAt::Refill && self.prepared > HEADERS
        {
            self.source.store(true, Ordering::Relaxed);
        }
        result
    }
    unsafe fn write(&mut self, h: Handle, p: *mut Header) -> u32 {
        self.log("write");
        let result = self.inner.write(h, p);
        if self.phase == RetireAt::Write {
            self.source.store(true, Ordering::Relaxed);
        }
        result
    }
    unsafe fn unprepare(&mut self, h: Handle, p: *mut Header) -> u32 {
        self.log("unprepare");
        if self.fail_unprepare.load(Ordering::Relaxed) {
            return STILL_PLAYING;
        }
        self.inner.unprepare(h, p)
    }
    fn pause(&mut self, h: Handle) -> u32 {
        self.log("pause");
        self.inner.pause(h)
    }
    fn restart(&mut self, h: Handle) -> u32 {
        self.log("restart");
        self.inner.restart(h)
    }
    fn reset(&mut self, h: Handle) -> u32 {
        self.log("reset");
        if self.fail_reset.load(Ordering::Relaxed) {
            return 1;
        }
        self.inner.reset(h)
    }
    fn position(&mut self, h: Handle) -> Result<(u32, u32), String> {
        self.log("position");
        if self.phase == RetireAt::Refill && self.prepared >= HEADERS {
            self.inner.return_all = true;
            self.inner.position = (HEADERS * BLOCK) as u32;
        }
        self.inner.position(h)
    }
    fn close(&mut self, h: Handle) -> u32 {
        self.log("close");
        self.inner.close(h)
    }
}
struct FenceDriver {
    worker: Worker,
    source: Arc<AtomicBool>,
    calls: Arc<std::sync::Mutex<Vec<&'static str>>>,
    fail_reset: Arc<AtomicBool>,
    fail_unprepare: Arc<AtomicBool>,
}
impl FenceDriver {
    fn new(phase: RetireAt, reset: bool, unprepare: bool) -> Self {
        let source = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let fail_reset = Arc::new(AtomicBool::new(reset));
        let fail_unprepare = Arc::new(AtomicBool::new(unprepare));
        let (token, record, resets, unprepares) = (
            source.clone(),
            calls.clone(),
            fail_reset.clone(),
            fail_unprepare.clone(),
        );
        let worker = Worker::with_factory(Arc::new(|| {}), move || FencedMock {
            inner: Mock::default(),
            source: token.clone(),
            phase,
            prepared: 0,
            calls: record.clone(),
            fail_reset: resets.clone(),
            fail_unprepare: unprepares.clone(),
        })
        .unwrap();
        Self {
            worker,
            source,
            calls,
            fail_reset,
            fail_unprepare,
        }
    }
    fn count(&self, name: &str) -> usize {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|&&call| call == name)
            .count()
    }
}
fn fence_wait(condition: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !condition() {
        assert!(
            std::time::Instant::now() < deadline,
            "PCM source-fence owner timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn local_audio_source_retired_before_submission_opens_no_device() {
    let driver = FenceDriver::new(RetireAt::Never, false, false);
    driver.source.store(true, Ordering::Relaxed);
    driver.worker.submit_for_source(
        clip(),
        1,
        Action::Play { from: 0 },
        Some(driver.source.clone()),
    );
    std::thread::sleep(std::time::Duration::from_millis(30));
    assert_eq!(driver.count("open"), 0);
    assert_eq!(driver.worker.ownership(), (false, 0, false));
    let (snapshot, notice) = driver.worker.poll();
    assert!(snapshot.is_none() && notice.is_none());
}

#[test]
fn local_audio_source_retirement_during_open_closes_returned_handle_before_preparing_buffers() {
    let driver = FenceDriver::new(RetireAt::Open, false, false);
    driver.worker.submit_for_source(
        clip(),
        1,
        Action::Play { from: 0 },
        Some(driver.source.clone()),
    );
    fence_wait(|| driver.count("close") == 1 && driver.worker.ownership() == (false, 0, false));
    assert_eq!(driver.count("open"), 1);
    assert_eq!(driver.count("prepare"), 0);
    assert_eq!(driver.count("write"), 0);
    assert_eq!(driver.count("restart"), 0);
    assert!(driver.worker.poll().0.is_none());
}

#[test]
fn local_audio_source_retirement_during_prepare_or_write_closes_only_after_safe_unprepare() {
    for phase in [RetireAt::Prepare, RetireAt::Write] {
        let driver = FenceDriver::new(phase, false, false);
        driver.worker.submit_for_source(
            clip(),
            1,
            Action::Play { from: 0 },
            Some(driver.source.clone()),
        );
        fence_wait(|| driver.count("close") == 1 && driver.worker.ownership() == (false, 0, false));
        assert_eq!(driver.count("prepare"), 1);
        assert_eq!(driver.count("write"), usize::from(phase == RetireAt::Write));
        assert_eq!(driver.count("restart"), 0);
        let calls = driver.calls.lock().unwrap();
        assert!(
            calls.iter().position(|&call| call == "reset").unwrap()
                < calls.iter().position(|&call| call == "unprepare").unwrap()
        );
        assert!(
            calls.iter().position(|&call| call == "unprepare").unwrap()
                < calls.iter().position(|&call| call == "close").unwrap()
        );
        drop(calls);
        assert!(driver.worker.poll().0.is_none());
    }
}

#[test]
fn local_audio_source_retirement_during_real_refill_prepares_no_extra_written_buffer() {
    let driver = FenceDriver::new(RetireAt::Refill, false, false);
    driver.worker.submit_for_source(
        clip(),
        1,
        Action::Play { from: 0 },
        Some(driver.source.clone()),
    );
    fence_wait(|| driver.count("close") == 1 && driver.worker.ownership() == (false, 0, false));
    assert_eq!(driver.count("prepare"), HEADERS + 1);
    assert_eq!(driver.count("write"), HEADERS);
    assert_eq!(driver.count("unprepare"), HEADERS + 1);
    assert_eq!(driver.count("restart"), 1);
    assert!(driver.worker.poll().0.is_none());
}

#[test]
fn local_audio_source_retirement_is_observed_while_paused_without_refill_or_auto_retry() {
    let driver = FenceDriver::new(RetireAt::Never, false, false);
    let clip = clip();
    driver.worker.submit_for_source(
        clip.clone(),
        1,
        Action::Play { from: 0 },
        Some(driver.source.clone()),
    );
    assert!(wait_snapshot(&driver.worker, 1).opened);
    driver.worker.submit_for_source(
        clip,
        2,
        Action::Pause { from: 0 },
        Some(driver.source.clone()),
    );
    let paused = wait_snapshot(&driver.worker, 2);
    assert!(!paused.playing && paused.opened);
    let written = driver.count("write");
    driver.source.store(true, Ordering::Relaxed);
    fence_wait(|| driver.worker.ownership() == (false, 0, false));
    assert_eq!(driver.count("write"), written);
    assert_eq!(driver.count("close"), 1);
    std::thread::sleep(std::time::Duration::from_millis(35));
    assert_eq!(driver.count("close"), 1);
    assert!(driver.worker.poll().0.is_none());
}

#[test]
fn local_audio_source_pointer_identity_fences_late_requests_and_snapshots_with_reused_clip_generation(
) {
    let driver = FenceDriver::new(RetireAt::Never, false, false);
    let clip = clip();
    let newer = Arc::new(AtomicBool::new(false));
    driver.worker.submit_for_source(
        clip.clone(),
        7,
        Action::Seek {
            frame: 100,
            resume: false,
        },
        Some(driver.source.clone()),
    );
    let old = wait_snapshot(&driver.worker, 7);
    assert!(old.matches_source(Some(&driver.source)));
    driver.worker.submit_for_source(
        clip.clone(),
        7,
        Action::Seek {
            frame: 200,
            resume: false,
        },
        Some(newer.clone()),
    );
    let current = wait_snapshot(&driver.worker, 7);
    assert_eq!((old.generation, current.generation), (7, 7));
    assert!(Arc::ptr_eq(&old.clip, &current.clip));
    assert!(!old.matches_source(Some(&newer)) && !current.matches_source(Some(&driver.source)));
    assert!(current.matches_source(Some(&newer)));
    assert_eq!(current.frame, 200);
    driver.source.store(true, Ordering::Relaxed);
    driver.worker.submit_for_source(
        clip,
        7,
        Action::Play { from: 0 },
        Some(driver.source.clone()),
    );
    std::thread::sleep(std::time::Duration::from_millis(35));
    assert_eq!(driver.count("open"), 0);
    assert!(driver.worker.poll().0.is_none());
    assert!(!old.matches_source(Some(&driver.source)));
}

#[test]
fn local_audio_source_replacement_resets_same_clip_owner_before_reuse_and_old_retirement_cannot_close_new_owner(
) {
    let driver = FenceDriver::new(RetireAt::Never, false, false);
    let clip = clip();
    let newer = Arc::new(AtomicBool::new(false));
    driver.worker.submit_for_source(
        clip.clone(),
        9,
        Action::Play { from: 0 },
        Some(driver.source.clone()),
    );
    assert!(wait_snapshot(&driver.worker, 9).opened);
    driver.worker.submit_for_source(
        clip.clone(),
        9,
        Action::Play { from: 500 },
        Some(newer.clone()),
    );
    let current = wait_snapshot(&driver.worker, 9);
    assert!(current.matches_source(Some(&newer)) && current.playing);
    assert_eq!(current.frame, 500);
    assert_eq!(driver.count("open"), 2);
    assert_eq!(driver.count("close"), 1);
    let calls = driver.calls.lock().unwrap();
    let second_open = calls
        .iter()
        .enumerate()
        .filter(|(_, call)| **call == "open")
        .nth(1)
        .unwrap()
        .0;
    assert!(calls.iter().position(|&call| call == "close").unwrap() < second_open);
    drop(calls);
    driver.source.store(true, Ordering::Relaxed);
    driver
        .worker
        .submit_for_source(clip, 9, Action::Stop, Some(driver.source.clone()));
    std::thread::sleep(std::time::Duration::from_millis(35));
    assert!(driver.worker.ownership().0);
    assert_eq!(driver.count("close"), 1);
    newer.store(true, Ordering::Relaxed);
    fence_wait(|| driver.worker.ownership() == (false, 0, false));
    assert_eq!(driver.count("close"), 2);
}

#[test]
fn local_audio_source_retirement_reset_or_unprepare_failure_retains_native_storage_until_explicit_recovery(
) {
    for reset_failure in [true, false] {
        let driver = FenceDriver::new(RetireAt::Prepare, reset_failure, !reset_failure);
        driver.worker.submit_for_source(
            clip(),
            1,
            Action::Play { from: 0 },
            Some(driver.source.clone()),
        );
        fence_wait(|| driver.worker.ownership() == (true, 1, false));
        let (snapshot, notice) = driver.worker.poll();
        assert!(snapshot.is_none());
        assert!(notice.is_some());
        let resets = driver.count("reset");
        let unprepares = driver.count("unprepare");
        assert_eq!(resets, 1);
        assert_eq!(driver.count("write"), 0);
        assert_eq!(driver.count("close"), 0);
        std::thread::sleep(std::time::Duration::from_millis(40));
        assert_eq!(
            (driver.count("reset"), driver.count("unprepare")),
            (resets, unprepares),
            "Failed native owner must not retry itself on idle ticks."
        );
        assert_eq!(driver.worker.ownership(), (true, 1, false));
        driver.fail_reset.store(false, Ordering::Relaxed);
        driver.fail_unprepare.store(false, Ordering::Relaxed);
        driver.worker.clear();
        fence_wait(|| driver.worker.ownership() == (false, 0, false));
        assert_eq!(driver.count("close"), 1);
    }
}
