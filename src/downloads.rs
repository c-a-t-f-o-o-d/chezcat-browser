//! Browser-owned, bounded downloads. Explicit user actions are the only callers.
//!
//! HTTP uses the same original transport and certificate checks as navigation.
//! That transport buffers at most 32 MiB, so network progress is deliberately
//! indeterminate until it returns. Saving uses a private `.part` file followed by
//! a no-replace publication: an existing file is never overwritten.
//! Download history is session-only; clearing it never removes saved files.

use crate::{http, navigation::Location, random, trust};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const MAX_DOWNLOAD_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 50;
pub const MAX_ACTIVE: usize = 3;
const MAX_NAME_BYTES: usize = 160;
const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_COLLISIONS: usize = 1000;
pub type Wake = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Receiving,
    Saving,
    Cancelling,
    Complete,
    Cancelled,
    Failed(String),
}

impl Status {
    pub fn active(&self) -> bool {
        matches!(self, Self::Receiving | Self::Saving | Self::Cancelling)
    }
}

#[derive(Clone)]
struct Retry {
    location: Location,
    suggested: Option<String>,
}

pub struct Entry {
    pub id: u64,
    pub name: String,
    pub source: String,
    pub folder: PathBuf,
    pub status: Status,
    /// Bytes actually written to the staging file, not a guessed transfer size.
    pub bytes: u64,
    pub total: Option<u64>,
    pub path: Option<PathBuf>,
    started: Instant,
    elapsed: Option<Duration>,
    retry: Option<Retry>,
}

impl Entry {
    pub fn elapsed(&self) -> Duration {
        self.elapsed.unwrap_or_else(|| self.started.elapsed())
    }

    pub fn can_retry(&self) -> bool {
        self.retry.is_some() && matches!(self.status, Status::Cancelled | Status::Failed(_))
    }
}

enum Task {
    Request(Location, Option<String>),
    Received(http::Response),
}

enum Update {
    Ready {
        name: String,
        source: String,
        total: u64,
    },
    Written(u64),
    Done(Result<Saved, String>),
}

struct Saved {
    path: PathBuf,
    bytes: u64,
}

struct Worker {
    id: u64,
    cancel: Arc<AtomicBool>,
    receive: mpsc::Receiver<Update>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Default)]
pub struct Manager {
    entries: Vec<Entry>,
    workers: Vec<Worker>,
    next_id: u64,
}

impl Manager {
    /// Most recent first. The list holds at most MAX_ENTRIES entries.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn active_count(&self) -> usize {
        self.workers.len()
    }

    pub fn start(
        &mut self,
        location: Location,
        suggested: Option<String>,
        folder: &Path,
        trust: Arc<trust::Store>,
        wake: Wake,
    ) -> Result<u64, String> {
        validate_source(&location)?;
        let name = proposed_filename(&location, suggested.as_deref(), None, None);
        let source = location.display();
        let retry = Some(Retry {
            location: location.clone(),
            suggested: suggested.clone(),
        });
        self.launch(
            Task::Request(location, suggested),
            name,
            source,
            retry,
            folder,
            trust,
            wake,
        )
    }

    /// Saves the response already fetched by a user navigation. In particular,
    /// this does not repeat a POST request to obtain its attachment a second time.
    pub fn start_received(
        &mut self,
        response: http::Response,
        folder: &Path,
        trust: Arc<trust::Store>,
        wake: Wake,
    ) -> Result<u64, String> {
        validate_response(&response)?;
        let location = web_location(response.url.clone());
        let name = proposed_filename(
            &location,
            None,
            response.header("content-disposition"),
            response.header("content-type"),
        );
        let retry = (!response.request_is_post).then_some(Retry {
            location,
            suggested: None,
        });
        let source = response.url.display();
        self.launch(
            Task::Received(response),
            name,
            source,
            retry,
            folder,
            trust,
            wake,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn launch(
        &mut self,
        task: Task,
        name: String,
        source: String,
        retry: Option<Retry>,
        folder: &Path,
        trust: Arc<trust::Store>,
        wake: Wake,
    ) -> Result<u64, String> {
        self.poll();
        if self.workers.len() >= MAX_ACTIVE {
            return Err(format!("At most {MAX_ACTIVE} downloads can run at once. Cancel one or wait for it to finish."));
        }
        let folder = prepare_folder(folder)?;
        let id = self
            .next_id
            .checked_add(1)
            .ok_or("The download identifier limit was reached.")?;
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let destination = folder.clone();
        let (send, receive) = mpsc::sync_channel(4);
        let thread = thread::Builder::new()
            .name(format!("chezcat-download-{id}"))
            .spawn(move || {
                let result = perform(task, &destination, &stop, &trust, &send, &wake);
                // A full progress mailbox cannot block cancellation or shutdown.
                // Dropping Manager disconnects the receiver before joining workers.
                let _ = send.send(Update::Done(result));
                wake();
            })
            .map_err(|error| format!("Cannot start a download worker: {error}"))?;
        self.next_id = id;
        while self.entries.len() >= MAX_ENTRIES {
            if let Some(index) = self
                .entries
                .iter()
                .rposition(|entry| !entry.status.active())
            {
                self.entries.remove(index);
            } else {
                // MAX_ACTIVE is lower than MAX_ENTRIES; this is unreachable.
                break;
            }
        }
        self.entries.insert(
            0,
            Entry {
                id,
                name,
                source,
                folder,
                status: Status::Receiving,
                bytes: 0,
                total: None,
                path: None,
                started: Instant::now(),
                elapsed: None,
                retry,
            },
        );
        self.workers.push(Worker {
            id,
            cancel,
            receive,
            thread: Some(thread),
        });
        Ok(id)
    }

    pub fn cancel(&mut self, id: u64) -> bool {
        let Some(worker) = self.workers.iter().find(|worker| worker.id == id) else {
            return false;
        };
        worker.cancel.store(true, Ordering::Relaxed);
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            entry.status = Status::Cancelling;
        }
        true
    }

    /// Retrying is another explicit user action. Already-received POST responses
    /// deliberately have no retry action; the originating form owns resubmission.
    pub fn retry(
        &mut self,
        id: u64,
        folder: &Path,
        trust: Arc<trust::Store>,
        wake: Wake,
    ) -> Result<u64, String> {
        let retry = self
            .entries
            .iter()
            .find(|entry| entry.id == id && entry.can_retry())
            .and_then(|entry| entry.retry.clone())
            .ok_or("This download cannot be retried.")?;
        self.start(retry.location, retry.suggested, folder, trust, wake)
    }

    pub fn clear_finished(&mut self) -> usize {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.status.active());
        before - self.entries.len()
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        let mut completed = Vec::new();
        for worker in &self.workers {
            loop {
                let message = match worker.receive.try_recv() {
                    Ok(message) => message,
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        if let Some(entry) =
                            self.entries.iter_mut().find(|entry| entry.id == worker.id)
                        {
                            if entry.status.active() {
                                entry.status = Status::Failed(
                                    "The download worker ended unexpectedly.".into(),
                                );
                                entry.elapsed = Some(entry.started.elapsed());
                                changed = true;
                            }
                        }
                        completed.push(worker.id);
                        break;
                    }
                };
                let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == worker.id)
                else {
                    break;
                };
                changed = true;
                match message {
                    Update::Ready {
                        name,
                        source,
                        total,
                    } => {
                        entry.name = name;
                        entry.source = source;
                        entry.total = Some(total);
                        if !matches!(entry.status, Status::Cancelling) {
                            entry.status = Status::Saving;
                        }
                    }
                    Update::Written(bytes) => entry.bytes = bytes,
                    Update::Done(result) => {
                        entry.elapsed = Some(entry.started.elapsed());
                        match result {
                            Ok(saved) => {
                                // A cancel arriving after atomic publication cannot
                                // honestly turn a saved file into a cancelled one.
                                entry.name = saved
                                    .path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned();
                                entry.path = Some(saved.path);
                                entry.bytes = saved.bytes;
                                entry.total = Some(saved.bytes);
                                entry.status = Status::Complete;
                            }
                            Err(_) if worker.cancel.load(Ordering::Relaxed) => {
                                entry.status = Status::Cancelled
                            }
                            Err(error) => entry.status = Status::Failed(error),
                        }
                        completed.push(worker.id);
                        break;
                    }
                }
            }
        }
        for id in completed {
            if let Some(index) = self.workers.iter().position(|worker| worker.id == id) {
                let mut worker = self.workers.swap_remove(index);
                if let Some(thread) = worker.thread.take() {
                    let _ = thread.join();
                }
            }
        }
        changed
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        for worker in &self.workers {
            worker.cancel.store(true, Ordering::Relaxed);
        }
        let handles: Vec<_> = self
            .workers
            .drain(..)
            .filter_map(|mut worker| worker.thread.take())
            .collect();
        // Receivers are now disconnected. Wait for worker guards to remove their
        // partial files before the application process exits.
        for handle in handles {
            let _ = handle.join();
        }
    }
}

fn validate_source(location: &Location) -> Result<(), String> {
    match (&location.url, &location.path) {
        (Some(_), None) => Ok(()),
        (None, Some(path)) if path.is_absolute() => Ok(()),
        _ => Err("Downloads require an HTTP(S) address or an absolute local file.".into()),
    }
}

fn validate_response(response: &http::Response) -> Result<(), String> {
    if !(200..300).contains(&response.status) {
        return Err(format!(
            "The server returned HTTP {}; no file was saved.",
            response.status
        ));
    }
    if response.status == 206 {
        return Err(
            "An unsolicited partial HTTP response cannot be saved as a complete download.".into(),
        );
    }
    if matches!(response.status, 204 | 205) {
        return Err(format!(
            "HTTP {} supplied no download representation; no file was saved.",
            response.status
        ));
    }
    if response.body.len() > MAX_DOWNLOAD_BYTES {
        return Err("Downloads are limited to 32 MiB in this build.".into());
    }
    Ok(())
}

fn web_location(url: crate::url::HttpUrl) -> Location {
    Location {
        path: None,
        url: Some(url),
        fragment: String::new(),
        root: None,
    }
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Download cancelled.".into())
    } else {
        Ok(())
    }
}

fn perform(
    task: Task,
    folder: &Path,
    cancel: &AtomicBool,
    trust: &trust::Store,
    send: &mpsc::SyncSender<Update>,
    wake: &Wake,
) -> Result<Saved, String> {
    check_cancel(cancel)?;
    match task {
        Task::Request(location, suggested) if location.url.is_none() => {
            let path = location
                .path
                .as_ref()
                .ok_or("The download source has no file path.")?;
            let metadata = fs::symlink_metadata(path)
                .map_err(|e| format!("Cannot read the source file: {e}"))?;
            if is_link(&metadata) || !metadata.is_file() {
                return Err(
                    "A local download must be a regular file, not a link or device.".into(),
                );
            }
            let canonical = fs::canonicalize(path)
                .map_err(|e| format!("Cannot resolve the source file: {e}"))?;
            if let Some(root) = &location.root {
                let root = fs::canonicalize(root)
                    .map_err(|e| format!("Cannot resolve the source folder: {e}"))?;
                if !canonical.starts_with(root) {
                    return Err(
                        "The local download is outside the document's content folder.".into(),
                    );
                }
            }
            if metadata.len() > MAX_DOWNLOAD_BYTES as u64 {
                return Err("Downloads are limited to 32 MiB in this build.".into());
            }
            let file =
                File::open(&canonical).map_err(|e| format!("Cannot open the source file: {e}"))?;
            if !file.metadata().map_err(|e| e.to_string())?.is_file() {
                return Err("The source changed to a non-regular file.".into());
            }
            let name = proposed_filename(&location, suggested.as_deref(), None, None);
            let _ = send.try_send(Update::Ready {
                name: name.clone(),
                source: location.display(),
                total: metadata.len(),
            });
            wake();
            save_reader(file, folder, &name, cancel, send, wake)
        }
        Task::Request(location, suggested) => {
            let url = location
                .url
                .as_ref()
                .ok_or("The download has no web address.")?;
            let response = http::fetch_request(
                url,
                &http::Request::Get,
                MAX_DOWNLOAD_BYTES,
                None,
                cancel,
                trust,
                http::FetchMode::Normal,
            )?;
            save_response(response, suggested.as_deref(), folder, cancel, send, wake)
        }
        Task::Received(response) => save_response(response, None, folder, cancel, send, wake),
    }
}

fn save_response(
    response: http::Response,
    suggested: Option<&str>,
    folder: &Path,
    cancel: &AtomicBool,
    send: &mpsc::SyncSender<Update>,
    wake: &Wake,
) -> Result<Saved, String> {
    check_cancel(cancel)?;
    validate_response(&response)?;
    let location = web_location(response.url.clone());
    let name = proposed_filename(
        &location,
        suggested,
        response.header("content-disposition"),
        response.header("content-type"),
    );
    let _ = send.try_send(Update::Ready {
        name: name.clone(),
        source: response.url.display(),
        total: response.body.len() as u64,
    });
    wake();
    save_reader(response.body.as_slice(), folder, &name, cancel, send, wake)
}

struct Partial {
    path: PathBuf,
    file: Option<File>,
}
impl Drop for Partial {
    fn drop(&mut self) {
        // Windows must not retain our writable handle during cleanup.
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
}

fn save_reader(
    mut reader: impl Read,
    folder: &Path,
    name: &str,
    cancel: &AtomicBool,
    send: &mpsc::SyncSender<Update>,
    wake: &Wake,
) -> Result<Saved, String> {
    check_cancel(cancel)?;
    // Recheck the folder before both staging and publication. Server-supplied
    // names are always reduced to one safe component; they cannot choose paths.
    check_folder(folder, false)?;
    let mut nonce = [0u8; 16];
    random::fill(&mut nonce)?;
    let token: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
    let partial_path = folder.join(format!(".chezcat-{token}.part"));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&partial_path).map_err(|e| {
        format!(
            "Cannot create a partial download in {}: {e}",
            folder.display()
        )
    })?;
    let mut partial = Partial {
        path: partial_path,
        file: Some(file),
    };
    let mut buffer = [0u8; 64 * 1024];
    let mut count = 0u64;
    let mut reported = 0u64;
    loop {
        check_cancel(cancel)?;
        let size = reader
            .read(&mut buffer)
            .map_err(|e| format!("Cannot read download bytes: {e}"))?;
        if size == 0 {
            break;
        }
        count = count
            .checked_add(size as u64)
            .ok_or("The download size overflowed.")?;
        if count > MAX_DOWNLOAD_BYTES as u64 {
            return Err("Downloads are limited to 32 MiB in this build.".into());
        }
        partial
            .file
            .as_mut()
            .unwrap()
            .write_all(&buffer[..size])
            .map_err(|e| format!("Cannot save download bytes: {e}"))?;
        if count.saturating_sub(reported) >= 256 * 1024 {
            let _ = send.try_send(Update::Written(count));
            reported = count;
            wake();
        }
    }
    partial
        .file
        .as_ref()
        .unwrap()
        .sync_all()
        .map_err(|e| format!("Cannot finish the download file: {e}"))?;
    drop(partial.file.take());
    check_cancel(cancel)?;
    check_folder(folder, false)?;
    let metadata = fs::symlink_metadata(&partial.path)
        .map_err(|e| format!("The partial download disappeared: {e}"))?;
    if is_link(&metadata) || !metadata.is_file() || metadata.len() != count {
        return Err("The partial download changed before it could be published.".into());
    }
    let name = sanitize_filename(name);
    for index in 0..MAX_COLLISIONS {
        check_cancel(cancel)?;
        let path = folder.join(collision_name(&name, index));
        // Publication never opts into overwriting an existing path, including
        // symlinks. The usual std::fs::rename would replace existing files.
        match publish_no_replace(&partial.path, &path) {
            Ok(()) => { drop(partial); return Ok(Saved { path, bytes: count }); }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Cannot publish the download without overwriting another file: {error}. Choose another local download folder.")),
        }
    }
    Err("Too many downloads already use that filename. Choose another download folder.".into())
}

#[cfg(not(windows))]
fn publish_no_replace(partial: &Path, final_path: &Path) -> std::io::Result<()> {
    // Hard-link insertion is atomic and fails for every existing directory
    // entry. The guard removes the staging link after successful publication.
    fs::hard_link(partial, final_path)
}

#[cfg(windows)]
fn publish_no_replace(partial: &Path, final_path: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(source: *const u16, destination: *const u16, flags: u32) -> i32;
    }
    let source: Vec<u16> = partial.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = final_path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // Same-folder native rename also works on FAT/exFAT, which have no hard
    // links. WRITE_THROUGH only: REPLACE_EXISTING and COPY_ALLOWED are absent.
    let moved = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 8) };
    if moved != 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if matches!(error.raw_os_error(), Some(80 | 183)) || fs::symlink_metadata(final_path).is_ok() {
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "A path with this name already exists.",
        ))
    } else {
        Err(error)
    }
}

/// A configured folder is an explicit local absolute directory. This check is
/// non-mutating so Settings can validate a draft without creating directories.
pub fn validate_folder(path: &Path) -> Result<(), String> {
    if !path.is_absolute() || path.as_os_str().is_empty() {
        return Err("Choose an absolute download folder path.".into());
    }
    if path.as_os_str().to_string_lossy().len() > 4096
        || path
            .as_os_str()
            .to_string_lossy()
            .chars()
            .any(char::is_control)
    {
        return Err("The download folder path is too long or contains control characters.".into());
    }
    for part in path.components() {
        match part {
            Component::ParentDir => {
                return Err("The download folder cannot contain '..' path components.".into())
            }
            #[cfg(windows)]
            Component::Prefix(prefix)
                if !matches!(
                    prefix.kind(),
                    std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)
                ) =>
            {
                return Err(
                    "Choose a local drive folder, not a network or device namespace.".into(),
                );
            }
            Component::Normal(name) => {
                let value = name.to_string_lossy();
                if value.contains(':') || value.ends_with([' ', '.']) || reserved_name(&value) {
                    return Err("The download folder contains an unsafe path component.".into());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Creates missing directories only in response to an explicit download/folder
/// action. Existing links and Windows reparse points are never followed.
pub fn prepare_folder(path: &Path) -> Result<PathBuf, String> {
    check_folder(path, true)?;
    fs::canonicalize(path).map_err(|e| format!("Cannot resolve the download folder: {e}"))
}

fn check_folder(path: &Path, create: bool) -> Result<(), String> {
    validate_folder(path)?;
    let mut current = PathBuf::new();
    for part in path.components() {
        current.push(part.as_os_str());
        if matches!(part, Component::Prefix(_) | Component::CurDir) {
            continue;
        }
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => {
                        return Err(format!(
                            "Cannot create download folder {}: {error}",
                            current.display()
                        ))
                    }
                }
                fs::symlink_metadata(&current).map_err(|e| {
                    format!("Cannot inspect download folder {}: {e}", current.display())
                })?
            }
            Err(error) => {
                return Err(format!(
                    "Cannot inspect download folder {}: {error}",
                    current.display()
                ))
            }
        };
        if is_link(&metadata) || !metadata.is_dir() {
            return Err(format!(
                "The download folder must use real directories, not links, junctions, or files: {}",
                current.display()
            ));
        }
    }
    Ok(())
}

fn is_link(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

pub fn default_folder() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let known = windows_downloads_folder();
    #[cfg(not(windows))]
    let known = None;
    #[cfg(windows)]
    let user = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let user = std::env::var_os("HOME");
    choose_default_folder(known, user)
}

fn choose_default_folder(
    known: Option<PathBuf>,
    user: Option<std::ffi::OsString>,
) -> Result<PathBuf, String> {
    // Use the current Windows location, including a user relocation. An unsafe
    // configured system path is an error, not permission to save somewhere else.
    let path = known
        .or_else(|| user.map(|path| PathBuf::from(path).join("Downloads")))
        .ok_or("No user folder is available. Set an absolute download folder in Settings.")?;
    validate_folder(&path)?;
    Ok(path)
}

#[cfg(windows)]
fn windows_downloads_folder() -> Option<PathBuf> {
    use std::ffi::{c_void, OsString};
    use std::os::windows::ffi::OsStringExt;

    #[repr(C)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }
    // Microsoft KNOWNFOLDERID: FOLDERID_Downloads.
    // https://learn.microsoft.com/windows/win32/shell/knownfolderid
    const DOWNLOADS: Guid = Guid {
        data1: 0x374de290,
        data2: 0x123f,
        data3: 0x4565,
        data4: [0x91, 0x64, 0x39, 0xc4, 0x92, 0x5e, 0x46, 0x7b],
    };
    const COINIT_APARTMENTTHREADED: u32 = 2;
    const RPC_E_CHANGED_MODE: i32 = 0x80010106u32 as i32;
    const KF_FLAG_DONT_VERIFY: u32 = 0x4000;

    #[link(name = "shell32")]
    extern "system" {
        fn SHGetKnownFolderPath(
            folder: *const Guid,
            flags: u32,
            token: *mut c_void,
            path: *mut *mut u16,
        ) -> i32;
    }
    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, mode: u32) -> i32;
        fn CoUninitialize();
        fn CoTaskMemFree(memory: *mut c_void);
    }
    struct ComInitialization(bool);
    impl Drop for ComInitialization {
        fn drop(&mut self) {
            if self.0 {
                // Balance S_OK and S_FALSE, never a failed initialization.
                unsafe { CoUninitialize() };
            }
        }
    }
    struct ShellPath(*mut u16);
    impl Drop for ShellPath {
        fn drop(&mut self) {
            // The Shell owns this allocation's format. Free it even when
            // SHGetKnownFolderPath fails; CoTaskMemFree also accepts null.
            unsafe { CoTaskMemFree(self.0.cast()) };
        }
    }

    let initialized = unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED) };
    if initialized < 0 && initialized != RPC_E_CHANGED_MODE {
        return None;
    }
    // RPC_E_CHANGED_MODE means the caller already initialized another apartment.
    let _com = ComInitialization(initialized >= 0);
    let mut path = ShellPath(std::ptr::null_mut());
    // No CREATE flag: resolving a default must not create folders. DONT_VERIFY
    // also avoids probing a redirected network destination before our own
    // local-directory validation rejects it.
    let result = unsafe {
        SHGetKnownFolderPath(
            &DOWNLOADS,
            KF_FLAG_DONT_VERIFY,
            std::ptr::null_mut(),
            &mut path.0,
        )
    };
    if result < 0 || path.0.is_null() {
        return None;
    }
    // A successful Shell result is a null-terminated UTF-16 string. Bound the
    // scan to the Windows path ceiling, then apply our stricter folder limits.
    let length = (0..32768).find(|&index| unsafe { *path.0.add(index) == 0 })?;
    let units = unsafe { std::slice::from_raw_parts(path.0, length) };
    Some(PathBuf::from(OsString::from_wide(units)))
}

/// Attachment detection does not treat an inline filename as a download order.
pub fn is_attachment(value: &str) -> bool {
    value
        .split(';')
        .next()
        .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("attachment"))
}

fn proposed_filename(
    location: &Location,
    suggested: Option<&str>,
    disposition: Option<&str>,
    mime: Option<&str>,
) -> String {
    if let Some(name) = disposition.and_then(disposition_filename) {
        return sanitize_filename(&name);
    }
    if let Some(name) = suggested.filter(|name| !name.trim().is_empty()) {
        return sanitize_filename(name);
    }
    if let Some(path) = &location.path {
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            return sanitize_filename(name);
        }
    }
    if let Some(url) = &location.url {
        if let Some(name) = url.path.rsplit('/').next().filter(|name| !name.is_empty()) {
            if let Some(decoded) =
                percent_bytes(name).and_then(|value| String::from_utf8(value).ok())
            {
                return sanitize_filename(&decoded);
            }
            return sanitize_filename(name);
        }
    }
    let extension = match mime
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "text/html" => "html",
        "text/plain" => "txt",
        "text/css" => "css",
        "application/json" => "json",
        "application/pdf" => "pdf",
        "application/zip" => "zip",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "image/avif" => "avif",
        "video/mp4" => "mp4",
        "audio/mpeg" => "mp3",
        _ => "bin",
    };
    format!("download.{extension}")
}

/// Original small RFC 6266/8187 parser. Duplicate filename parameters invalidate
/// the advisory name; valid filename* takes precedence over filename. Extended
/// values support UTF-8 and the older ISO-8859-1 form from RFC 5987.
pub fn disposition_filename(value: &str) -> Option<String> {
    if value.len() > MAX_HEADER_BYTES || value.bytes().any(|b| b < 0x20 && b != b'\t') {
        return None;
    }
    let bytes = value.as_bytes();
    let mut i = bytes.iter().position(|b| *b == b';')?;
    let mut plain = None;
    let mut extended = None;
    let mut seen_plain = false;
    let mut seen_extended = false;
    while i < bytes.len() {
        if bytes[i] != b';' {
            return None;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && token(bytes[i]) {
            i += 1;
        }
        if start == i {
            return None;
        }
        let key = &value[start..i];
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b'=') {
            return None;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut quoted = false;
        let parameter = if bytes.get(i) == Some(&b'"') {
            quoted = true;
            i += 1;
            let mut result = Vec::new();
            let mut closed = false;
            while i < bytes.len() {
                match bytes[i] {
                    b'"' => {
                        i += 1;
                        closed = true;
                        break;
                    }
                    b'\\' => {
                        i += 1;
                        let escaped = *bytes.get(i)?;
                        // HTTP quoted-pair escapes only one following byte.
                        result.push(escaped);
                        i += 1;
                    }
                    byte => {
                        result.push(byte);
                        i += 1;
                    }
                }
            }
            if !closed {
                return None;
            }
            String::from_utf8(result).ok()?
        } else {
            let start = i;
            while i < bytes.len() && token(bytes[i]) {
                i += 1;
            }
            if start == i {
                return None;
            }
            value[start..i].to_owned()
        };
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] != b';' {
            return None;
        }
        if key.eq_ignore_ascii_case("filename") {
            if seen_plain {
                return None;
            }
            seen_plain = true;
            plain = Some(parameter);
        } else if key.eq_ignore_ascii_case("filename*") {
            if seen_extended {
                return None;
            }
            seen_extended = true;
            if !quoted {
                extended = extended_filename(&parameter);
            }
        }
    }
    extended.or(plain).filter(|name| !name.trim().is_empty())
}

fn token(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn extended_filename(value: &str) -> Option<String> {
    let mut pieces = value.splitn(3, '\'');
    let charset = pieces.next()?;
    let language = pieces.next()?;
    if !language
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return None;
    }
    let encoded = pieces.next()?;
    if !encoded
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"!#$&+-.^_`|~%".contains(&b))
    {
        return None;
    }
    let bytes = percent_bytes(encoded)?;
    if charset.eq_ignore_ascii_case("utf-8") {
        String::from_utf8(bytes).ok()
    } else if charset.eq_ignore_ascii_case("iso-8859-1") {
        Some(bytes.into_iter().map(char::from).collect())
    } else {
        None
    }
}

fn percent_bytes(value: &str) -> Option<Vec<u8>> {
    let mut result = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let high = char::from(*bytes.get(i + 1)?).to_digit(16)?;
            let low = char::from(*bytes.get(i + 2)?).to_digit(16)?;
            result.push(((high << 4) | low) as u8);
            i += 3;
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }
    Some(result)
}

/// Names are advisory only: directory segments are discarded, unsafe characters
/// replaced, control/bidi formatting removed, and Windows device aliases avoided
/// on every host so a download stays safe when copied between platforms.
pub fn sanitize_filename(value: &str) -> String {
    let basename = value.rsplit(['/', '\\']).next().unwrap_or("");
    let mut name = String::new();
    for character in basename.chars() {
        if character.is_control()
            || matches!(character, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')
        {
            continue;
        }
        let character = if "<>:\"|?*".contains(character) {
            '_'
        } else {
            character
        };
        if name.len() + character.len_utf8() > MAX_NAME_BYTES {
            break;
        }
        name.push(character);
    }
    name = name.trim_matches([' ', '.']).to_owned();
    if name.is_empty() || name == "~" {
        return "download.bin".into();
    }
    if reserved_name(&name) {
        name.insert(0, '_');
    }
    name
}

fn reserved_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    })
}

fn collision_name(name: &str, index: usize) -> String {
    if index == 0 {
        return name.to_owned();
    }
    match name.rsplit_once('.') {
        Some((base, extension)) if !base.is_empty() && extension.len() <= 20 => {
            format!("{base} ({index}).{extension}")
        }
        _ => format!("{name} ({index})"),
    }
}

#[cfg(test)]
#[path = "downloads_tests.rs"]
mod tests;
