//! Local browsing history and its recording/expiry policy. Only the browser UI
//! owns this store. Every disk mutation reloads under the history writer lock,
//! so an older window cannot restore visits deleted by a newer window.

use crate::navigation;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "history_shared.rs"]
mod shared;
pub use shared::{Shared, View};

pub const MAX_ENTRIES: usize = 5000;
pub const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_TITLE_BYTES: usize = 1024;
pub const MAX_ADDRESS_BYTES: usize = 16 * 1024;
const HEADER: &str = "OwnBrowser history 1";
const DAY: u64 = 24 * 60 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    pub enabled: bool,
    pub retention_days: Option<u16>,
    pub clear_on_exit: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            enabled: false,
            retention_days: Some(30),
            clear_on_exit: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: u64,
    pub timestamp: u64,
    pub title: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Data {
    pub policy: Policy,
    /// Newest recorded visit first. IDs, rather than wall-clock ordering, keep
    /// a newly visited page first when the system clock moves backward.
    pub entries: Vec<Entry>,
    next_id: u64,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            policy: Policy::default(),
            entries: Vec::new(),
            next_id: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClearRange {
    Hour,
    Day,
    Week,
    All,
}

#[derive(Debug)]
pub struct Store {
    path: Option<PathBuf>,
    data: Data,
    problem: Option<String>,
}

impl Default for Store {
    fn default() -> Self {
        Self::memory()
    }
}

impl Store {
    pub fn memory() -> Self {
        Self {
            path: None,
            data: Data::default(),
            problem: None,
        }
    }

    /// Reads only. The caller may run refresh(now) to perform startup expiry.
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let path = absolute_path(path)?;
        let data = match read_snapshot(&path)? {
            Some(bytes) => decode(&bytes)?,
            None => Data::default(),
        };
        Ok(Self {
            path: Some(path),
            data,
            problem: None,
        })
    }

    pub fn unavailable(path: PathBuf, reason: String) -> Self {
        Self {
            path: Some(absolute_path(path.clone()).unwrap_or(path)),
            problem: Some(reason),
            ..Self::memory()
        }
    }

    pub fn data(&self) -> &Data {
        &self.data
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    /// Refreshes another window's changes and removes expired records even
    /// while recording is disabled. True means the in-memory view changed.
    pub fn refresh(&mut self, now: u64) -> Result<bool, String> {
        self.transact(now, false, false, |_| Ok(()))
            .map(|(_, changed, _)| changed)
    }

    /// Records a committed top-level page only when the latest disk policy is
    /// enabled. Unsupported addresses are skipped. No request/body is accepted.
    pub fn record(&mut self, now: u64, title: &str, url: &str) -> Result<Option<u64>, String> {
        self.transact(now, false, false, |data| {
            if !data.policy.enabled {
                return Ok(None);
            }
            if url.len() > MAX_ADDRESS_BYTES {
                return Err("This page's address exceeds the 16 KiB history limit.".into());
            }
            let url = match navigation::stored_address(url) {
                Ok(url) if !url.starts_with("about:home") => url,
                _ => return Ok(None),
            };
            let title = title_for_history(title, &url);
            let id = data.next_id;
            data.next_id = id
                .checked_add(1)
                .ok_or("The history visit identifier limit has been reached.")?;
            data.entries.insert(
                0,
                Entry {
                    id,
                    timestamp: now,
                    title,
                    url,
                },
            );
            Ok(Some(id))
        })
        .map(|(id, _, _)| id)
    }

    /// Saving policy preserves visits read from the latest file. A shorter
    /// retention takes effect immediately; disabling recording keeps old rows.
    pub fn configure(&mut self, policy: Policy, now: u64) -> Result<bool, String> {
        validate_policy(policy)?;
        self.transact(now, policy != Policy::default(), false, |data| {
            data.policy = policy;
            prune(data, now);
            Ok(())
        })
        .map(|(_, changed, _)| changed)
    }

    /// The selected row must still be the row this caller observed. Normal
    /// clears retain next_id; this check also protects a stale selection after
    /// explicit recovery or external replacement of the entire history file.
    pub fn remove(&mut self, id: u64, now: u64) -> Result<bool, String> {
        let expected = self
            .data
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .cloned();
        self.transact(now, false, false, |data| {
            let Some(index) = data.entries.iter().position(|entry| entry.id == id) else {
                return Ok(false);
            };
            if expected.as_ref() != Some(&data.entries[index]) {
                return Err("This history entry changed in another window. Reopen History before removing it.".into());
            }
            data.entries.remove(index);
            Ok(true)
        })
        .map(|(removed, _, _)| removed)
    }

    /// Range boundaries include the cutoff and now. Future timestamps caused
    /// by a clock change are retained by a time range; All removes every row.
    /// Confirmed All is the only operation allowed to replace corrupt readable
    /// history; that recovery resets policy to its disabled defaults.
    pub fn clear(&mut self, range: ClearRange, now: u64) -> Result<usize, String> {
        self.transact(now, false, range == ClearRange::All, |data| {
            let before = data.entries.len();
            let span = match range {
                ClearRange::Hour => Some(60 * 60),
                ClearRange::Day => Some(DAY),
                ClearRange::Week => Some(7 * DAY),
                ClearRange::All => None,
            };
            data.entries.retain(|entry| {
                span.is_some_and(|span| {
                    entry.timestamp < now.saturating_sub(span) || entry.timestamp > now
                })
            });
            Ok(before - data.entries.len())
        })
        .map(|(removed, _, expired)| removed + expired)
    }

    /// Normal window shutdown only. It reads the latest policy under the lock;
    /// one window's shutdown cannot resurrect another window's cleared rows.
    pub fn shutdown(&mut self, now: u64) -> Result<usize, String> {
        self.transact(now, false, false, |data| {
            let removed = if data.policy.clear_on_exit {
                let count = data.entries.len();
                data.entries.clear();
                count
            } else {
                0
            };
            Ok(removed)
        })
        .map(|(removed, _, expired)| removed + expired)
    }

    /// Earliest known expiration, including when recording is disabled. The
    /// application periodically refreshes too, to observe other windows.
    pub fn next_expiration(&self) -> Option<u64> {
        let span = u64::from(self.data.policy.retention_days?) * DAY;
        self.data
            .entries
            .iter()
            .filter_map(|entry| entry.timestamp.checked_add(span))
            .min()
    }

    fn transact<T>(
        &mut self,
        now: u64,
        create: bool,
        recover: bool,
        action: impl FnOnce(&mut Data) -> Result<T, String>,
    ) -> Result<(T, bool, usize), String> {
        let result = self.transact_inner(now, create, recover, action);
        self.problem = result.as_ref().err().cloned();
        result
    }

    fn transact_inner<T>(
        &mut self,
        now: u64,
        create: bool,
        recover: bool,
        action: impl FnOnce(&mut Data) -> Result<T, String>,
    ) -> Result<(T, bool, usize), String> {
        let Some(path) = self.path.clone() else {
            let mut next = self.data.clone();
            let expired = prune(&mut next, now);
            let output = action(&mut next)?;
            trim(&mut next);
            validate(&mut next)?;
            let changed = self.data != next;
            self.data = next;
            return Ok((output, changed, expired));
        };

        // A disabled, missing store stays entirely absent: not even its folder
        // or a transient lock file is created by browsing/refresh/shutdown.
        if read_snapshot(&path)?.is_none() && !create {
            let mut next = Data::default();
            let output = action(&mut next)?;
            debug_assert_eq!(next, Data::default());
            let changed = self.data != next;
            self.data = next;
            return Ok((output, changed, 0));
        }
        let parent = path
            .parent()
            .ok_or("The history path has no parent folder.")?;
        if create {
            let mut directories = fs::DirBuilder::new();
            directories.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                directories.mode(0o700);
            }
            directories
                .create(parent)
                .map_err(|error| format!("Cannot create the history folder: {error}"))?;
        }
        let _lock = WriterLock::acquire(&path)?;
        let original = read_snapshot(&path)?;
        let (latest, recovered) = match &original {
            Some(bytes) => match decode(bytes) {
                Ok(data) => (data, false),
                Err(_) if recover => (Data::default(), true),
                Err(error) => return Err(error),
            },
            None => (Data::default(), false),
        };
        let mut next = latest.clone();
        if recovered || (original.is_none() && create) {
            // Keep IDs across ordinary clear-all. Only a new/recovered file
            // starts a fresh range, avoiding reuse of an older window's IDs.
            let mut bytes = [0u8; 8];
            crate::random::fill(&mut bytes)?;
            next.next_id = (u64::from_le_bytes(bytes) & (u64::MAX >> 1)).max(1);
        }
        let expired = prune(&mut next, now);
        let output = action(&mut next)?;
        trim(&mut next);
        validate(&mut next)?;
        if recovered || next != latest {
            let encoded = encode(&next)?;
            check_snapshot(&path, &original)?;
            let mut temporary = Temporary::create(&path)?;
            temporary
                .file
                .as_mut()
                .unwrap()
                .write_all(&encoded)
                .and_then(|()| temporary.file.as_ref().unwrap().sync_all())
                .map_err(|error| format!("Cannot finish saving history: {error}"))?;
            temporary.file.take();
            check_snapshot(&path, &original)?;
            fs::rename(&temporary.path, &path)
                .map_err(|error| format!("Cannot replace the history file: {error}"))?;
            temporary.installed = true;
        }
        let changed = self.data != next;
        self.data = next;
        Ok((output, changed, expired))
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn validate_policy(policy: Policy) -> Result<(), String> {
    if !matches!(policy.retention_days, None | Some(1 | 7 | 30 | 90)) {
        return Err("History retention must be 1, 7, 30 or 90 days, or until cleared.".into());
    }
    Ok(())
}

fn prune(data: &mut Data, now: u64) -> usize {
    let before = data.entries.len();
    if let Some(cutoff) = data
        .policy
        .retention_days
        .and_then(|days| now.checked_sub(u64::from(days) * DAY))
    {
        data.entries.retain(|entry| entry.timestamp > cutoff);
    }
    before - data.entries.len()
}

fn title_for_history(title: &str, url: &str) -> String {
    let mut output = String::new();
    let mut space = false;
    for ch in title.chars() {
        if ch.is_whitespace() || ch.is_control() {
            space = !output.is_empty();
            continue;
        }
        let extra = usize::from(space) + ch.len_utf8();
        if output.len() + extra > MAX_TITLE_BYTES {
            break;
        }
        if space {
            output.push(' ');
            space = false;
        }
        output.push(ch);
    }
    if output.is_empty() {
        let mut end = url.len().min(MAX_TITLE_BYTES);
        while !url.is_char_boundary(end) {
            end -= 1;
        }
        output.push_str(&url[..end]);
    }
    output
}

fn escaped_len(value: &str) -> usize {
    value.len()
        + value
            .bytes()
            .filter(|byte| matches!(byte, b'\\' | b'\t' | b'\r' | b'\n'))
            .count()
}

fn entry_bytes(entry: &Entry) -> usize {
    10 + entry.id.to_string().len()
        + entry.timestamp.to_string().len()
        + escaped_len(&entry.title)
        + escaped_len(&entry.url)
}

fn trim(data: &mut Data) {
    data.entries.truncate(MAX_ENTRIES);
    // Reserve more than the fixed header/policy/next-ID records can consume.
    // Remove oldest recorded entries, including if their URLs use the cap.
    let mut bytes: usize = 256 + data.entries.iter().map(entry_bytes).sum::<usize>();
    while bytes > MAX_FILE_BYTES {
        let Some(entry) = data.entries.pop() else {
            break;
        };
        bytes -= entry_bytes(&entry);
    }
}

fn validate(data: &mut Data) -> Result<(), String> {
    validate_policy(data.policy)?;
    if data.entries.len() > MAX_ENTRIES || data.next_id == 0 {
        return Err("The history count or next visit identifier is invalid.".into());
    }
    let mut previous = data.next_id;
    for entry in &mut data.entries {
        if entry.id == 0 || entry.id >= previous {
            return Err("History visit identifiers are duplicated or out of order.".into());
        }
        previous = entry.id;
        if entry.title.is_empty()
            || entry.title.len() > MAX_TITLE_BYTES
            || entry.title.chars().any(char::is_control)
        {
            return Err(
                "A history title is empty, too long, or contains control characters.".into(),
            );
        }
        if entry.url.len() > MAX_ADDRESS_BYTES {
            return Err("A history address exceeds 16 KiB.".into());
        }
        entry.url = navigation::stored_address(&entry.url)?;
        if entry.url.starts_with("about:home") {
            return Err("The bundled home page is not stored in browsing history.".into());
        }
    }
    Ok(())
}

fn decode(bytes: &[u8]) -> Result<Data, String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err("The history file exceeds 4 MiB.".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "The history file is not valid UTF-8.")?;
    let mut lines = text.strip_prefix('\u{feff}').unwrap_or(text).lines();
    if lines.next() != Some(HEADER) {
        return Err(
            "The history format or version is not supported. Use confirmed Clear all to reset it."
                .into(),
        );
    }
    let mut data = Data::default();
    let mut fields_seen = 0u8;
    for (number, line) in lines.enumerate() {
        let fields: Vec<_> = line.splitn(6, '\t').collect();
        let bad = || {
            format!(
                "Invalid history record on line {}. Use confirmed Clear all to reset it.",
                number + 2
            )
        };
        match fields.as_slice() {
            ["enabled", value] | ["clear_on_exit", value] => {
                let bit = if fields[0] == "enabled" { 1 } else { 2 };
                if fields_seen & bit != 0 {
                    return Err(bad());
                }
                fields_seen |= bit;
                let enabled = match *value {
                    "true" => true,
                    "false" => false,
                    _ => return Err(bad()),
                };
                if bit == 1 {
                    data.policy.enabled = enabled;
                } else {
                    data.policy.clear_on_exit = enabled;
                }
            }
            ["retention_days", value] => {
                if fields_seen & 4 != 0 {
                    return Err(bad());
                }
                fields_seen |= 4;
                data.policy.retention_days = if *value == "none" {
                    None
                } else {
                    Some(decimal(value).map_err(|_| bad())?)
                };
            }
            ["next_id", value] => {
                if fields_seen & 8 != 0 {
                    return Err(bad());
                }
                fields_seen |= 8;
                data.next_id = decimal(value).map_err(|_| bad())?;
            }
            ["visit", id, timestamp, title, url] => {
                if data.entries.len() == MAX_ENTRIES {
                    return Err("The history file exceeds 5,000 entries.".into());
                }
                data.entries.push(Entry {
                    id: decimal(id).map_err(|_| bad())?,
                    timestamp: decimal(timestamp).map_err(|_| bad())?,
                    title: unescape(title, MAX_TITLE_BYTES)?,
                    url: unescape(url, MAX_ADDRESS_BYTES)?,
                });
            }
            _ => return Err(bad()),
        }
    }
    if fields_seen != 15 {
        return Err("The history file is missing a policy or identifier record. Use confirmed Clear all to reset it.".into());
    }
    validate(&mut data)?;
    Ok(data)
}

fn decimal<T: std::str::FromStr>(value: &str) -> Result<T, ()> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(());
    }
    value.parse().map_err(|_| ())
}

fn unescape(value: &str, limit: usize) -> Result<String, String> {
    if value.len() > limit * 2 {
        return Err("An escaped history field exceeds its size limit.".into());
    }
    let mut output = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        let ch = if ch == '\\' {
            match chars.next() {
                Some('\\') => '\\',
                Some('t') => '\t',
                Some('r') => '\r',
                Some('n') => '\n',
                _ => return Err("The history file contains an invalid escape.".into()),
            }
        } else {
            if ch.is_control() {
                return Err("The history file contains an unescaped control character.".into());
            }
            ch
        };
        if output.len() + ch.len_utf8() > limit {
            return Err("A history field exceeds its size limit.".into());
        }
        output.push(ch);
    }
    Ok(output)
}

fn encode(data: &Data) -> Result<Vec<u8>, String> {
    let retention = data
        .policy
        .retention_days
        .map_or_else(|| "none".into(), |days| days.to_string());
    let mut text = format!(
        "{HEADER}\nenabled\t{}\nclear_on_exit\t{}\nretention_days\t{retention}\nnext_id\t{}\n",
        data.policy.enabled, data.policy.clear_on_exit, data.next_id
    );
    for entry in &data.entries {
        text.push_str(&format!("visit\t{}\t{}\t", entry.id, entry.timestamp));
        escaped(&mut text, &entry.title)?;
        text.push('\t');
        escaped(&mut text, &entry.url)?;
        text.push('\n');
        if text.len() > MAX_FILE_BYTES {
            return Err("The history file exceeds 4 MiB.".into());
        }
    }
    Ok(text.into_bytes())
}

fn escaped(output: &mut String, value: &str) -> Result<(), String> {
    for ch in value.chars() {
        let escape = match ch {
            '\\' => Some("\\\\"),
            '\t' => Some("\\t"),
            '\r' => Some("\\r"),
            '\n' => Some("\\n"),
            _ => None,
        };
        if output.len() + escape.map_or(ch.len_utf8(), str::len) > MAX_FILE_BYTES {
            return Err("The history file exceeds 4 MiB.".into());
        }
        if let Some(value) = escape {
            output.push_str(value);
        } else {
            output.push(ch);
        }
    }
    Ok(())
}

fn absolute_path(path: PathBuf) -> Result<PathBuf, String> {
    if path.file_name().is_none() {
        return Err("A history path must name a file.".into());
    }
    if path.is_absolute() {
        Ok(path)
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(path))
            .map_err(|error| format!("Cannot locate the history folder: {error}"))
    }
}

fn read_snapshot(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Cannot inspect history: {error}")),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("History must be a regular file, not a directory or symbolic link.".into());
    }
    if metadata.len() > MAX_FILE_BYTES as u64 {
        return Err(
            "The history file exceeds 4 MiB. Close the browser and remove that file to reset it."
                .into(),
        );
    }
    let file = File::open(path).map_err(|error| format!("Cannot open history: {error}"))?;
    if !file
        .metadata()
        .map_err(|error| format!("Cannot inspect opened history: {error}"))?
        .is_file()
    {
        return Err("The opened history is not a regular file.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read history: {error}"))?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err("The history file exceeds 4 MiB.".into());
    }
    Ok(Some(bytes))
}

fn check_snapshot(path: &Path, original: &Option<Vec<u8>>) -> Result<(), String> {
    if read_snapshot(path)? != *original {
        return Err("History changed outside this browser while saving. Retry the action.".into());
    }
    if fs::metadata(path).is_ok_and(|metadata| metadata.permissions().readonly()) {
        return Err(
            "The history file is marked read-only. Its contents have not been changed.".into(),
        );
    }
    Ok(())
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

fn private_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

struct WriterLock {
    path: PathBuf,
    file: Option<File>,
}

impl WriterLock {
    fn acquire(history: &Path) -> Result<Self, String> {
        let path = sibling(history, ".lock");
        let file = private_file(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                format!("History is busy in another window. Retry shortly. If no browser is running, remove the leftover lock file {}.", path.display())
            } else {
                format!("Cannot lock history for saving: {error}")
            }
        })?;
        let mut lock = Self {
            path,
            file: Some(file),
        };
        writeln!(lock.file.as_mut().unwrap(), "{}", std::process::id())
            .map_err(|error| format!("Cannot write the history lock: {error}"))?;
        Ok(lock)
    }
}

impl Drop for WriterLock {
    fn drop(&mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.path);
    }
}

struct Temporary {
    path: PathBuf,
    file: Option<File>,
    installed: bool,
}

impl Temporary {
    fn create(history: &Path) -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = sibling(
            history,
            &format!(
                ".tmp-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ),
        );
        let file = private_file(&path)
            .map_err(|error| format!("Cannot create the staged history file: {error}"))?;
        Ok(Self {
            path,
            file: Some(file),
            installed: false,
        })
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        self.file.take();
        if !self.installed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "chezcat-history-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn file(&self) -> PathBuf {
            self.0.join("history.txt")
        }
        fn open(&self) -> Store {
            Store::open(self.file()).unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn enabled() -> Policy {
        Policy {
            enabled: true,
            retention_days: None,
            clear_on_exit: false,
        }
    }

    fn record(store: &mut Store, time: u64, page: &str) -> u64 {
        store
            .record(time, page, &format!("https://example.com/{page}"))
            .unwrap()
            .unwrap()
    }

    #[test]
    fn absent_disabled_history_never_creates_a_folder_or_lock() {
        let fixture = Fixture::new();
        let path = fixture.0.join("absent/folder/history.txt");
        let mut store = Store::open(path.clone()).unwrap();
        assert_eq!(store.data().policy, Policy::default());
        assert!(!store.refresh(10 * DAY).unwrap());
        assert_eq!(
            store.record(10, "Page", "https://example.com/").unwrap(),
            None
        );
        assert_eq!(store.clear(ClearRange::All, 10).unwrap(), 0);
        assert_eq!(store.shutdown(10).unwrap(), 0);
        assert!(!store.configure(Policy::default(), 10).unwrap());
        assert!(!path.parent().unwrap().exists());
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
    }

    #[test]
    fn records_roundtrip_with_normalized_titles_and_addresses() {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        store.configure(enabled(), 100).unwrap();
        let first = store
            .record(
                100,
                "  Café\t\n😀\\ title\0 ",
                "HTTPS://Example.COM:443/a/../b#café",
            )
            .unwrap()
            .unwrap();
        let second = store
            .record(90, "", "https://example.com/older-clock")
            .unwrap()
            .unwrap();
        assert!(second > first);
        assert_eq!(store.data().entries[0].timestamp, 90);
        assert_eq!(store.data().entries[1].title, "Café 😀\\ title");
        assert_eq!(
            store.data().entries[1].url,
            "https://example.com/b#caf%C3%A9"
        );
        assert_eq!(
            store.data().entries[0].title,
            "https://example.com/older-clock"
        );
        assert_eq!(fixture.open().data(), store.data());
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
        assert!(String::from_utf8(fs::read(fixture.file()).unwrap())
            .unwrap()
            .contains("Café 😀\\\\ title"));
    }

    #[test]
    fn memory_history_has_the_same_policy_but_no_path() {
        let mut store = Store::memory();
        assert!(store.path().is_none());
        assert!(store
            .record(100, "Off", "https://example.com/")
            .unwrap()
            .is_none());
        store.configure(enabled(), 100).unwrap();
        assert_eq!(record(&mut store, 100, "one"), 1);
        assert_eq!(record(&mut store, 101, "two"), 2);
        assert!(store.remove(1, 102).unwrap());
        assert_eq!(store.clear(ClearRange::All, 103).unwrap(), 1);
        assert_eq!(record(&mut store, 104, "three"), 3);
    }

    #[test]
    fn stale_windows_reload_disabled_policy_and_never_restore_deleted_rows() {
        let fixture = Fixture::new();
        let mut first = fixture.open();
        first.configure(enabled(), 10).unwrap();
        let one = record(&mut first, 10, "one");
        let mut second = fixture.open();
        let two = record(&mut second, 20, "two");
        assert!(first.remove(one, 21).unwrap());
        record(&mut second, 22, "three");
        assert!(!second.data().entries.iter().any(|entry| entry.id == one));
        first.refresh(23).unwrap();
        assert_eq!(first.data().entries.len(), 2);
        let mut off = enabled();
        off.enabled = false;
        first.configure(off, 24).unwrap();
        assert_eq!(
            second
                .record(25, "Ignored", "https://example.com/ignored")
                .unwrap(),
            None
        );
        assert!(!second.data().policy.enabled);
        assert_eq!(second.data().entries.len(), 2);
        assert!(second.data().entries.iter().any(|entry| entry.id == two));
        assert_eq!(first.clear(ClearRange::All, 26).unwrap(), 2);
        second.configure(enabled(), 27).unwrap();
        let fresh = record(&mut second, 28, "fresh");
        assert!(fresh > two);
        assert_eq!(fixture.open().data().entries.len(), 1);
        assert!(!first.remove(one, 29).unwrap());
        assert_eq!(first.data().entries[0].id, fresh);
    }

    #[test]
    fn retention_boundary_and_clock_changes_are_explicit() {
        let mut store = Store::memory();
        store.configure(enabled(), 0).unwrap();
        record(&mut store, 0, "epoch");
        record(&mut store, 1, "one");
        record(&mut store, 3 * DAY, "future");
        let policy = Policy {
            enabled: false,
            retention_days: Some(1),
            clear_on_exit: false,
        };
        store.configure(policy, DAY - 1).unwrap();
        assert_eq!(store.data().entries.len(), 3);
        assert_eq!(store.next_expiration(), Some(DAY));
        assert!(store.refresh(DAY).unwrap());
        assert_eq!(store.data().entries.len(), 2);
        assert_eq!(store.next_expiration(), Some(DAY + 1));
        assert!(store.refresh(DAY + 1).unwrap());
        assert_eq!(store.data().entries.len(), 1);
        assert!(!store.refresh(0).unwrap());
        assert_eq!(store.data().entries[0].timestamp, 3 * DAY);
        assert_eq!(store.next_expiration(), Some(4 * DAY));
    }

    #[test]
    fn manual_ranges_include_cutoffs_and_keep_future_records() {
        let mut store = Store::memory();
        store.configure(enabled(), 0).unwrap();
        let now = 10 * DAY;
        for time in [
            now - 7 * DAY - 1,
            now - 7 * DAY,
            now - DAY,
            now - 3600 - 1,
            now - 3600,
            now,
            now + 1,
        ] {
            record(&mut store, time, &time.to_string());
        }
        assert_eq!(store.clear(ClearRange::Hour, now).unwrap(), 2);
        assert_eq!(store.clear(ClearRange::Day, now).unwrap(), 2);
        assert_eq!(store.clear(ClearRange::Week, now).unwrap(), 1);
        assert_eq!(store.data().entries.len(), 2);
        assert_eq!(store.clear(ClearRange::All, now).unwrap(), 2);
    }

    #[test]
    fn shutdown_obeys_fresh_policy_and_cannot_resurrect_history() {
        let fixture = Fixture::new();
        let mut first = fixture.open();
        first.configure(enabled(), 1).unwrap();
        record(&mut first, 2, "page");
        let mut second = fixture.open();
        let mut policy = enabled();
        policy.clear_on_exit = true;
        second.configure(policy, 3).unwrap();
        assert_eq!(first.shutdown(4).unwrap(), 1);
        assert_eq!(second.shutdown(5).unwrap(), 0);
        assert!(second.data().entries.is_empty());
        record(&mut second, 6, "new");
        policy.clear_on_exit = false;
        first.configure(policy, 7).unwrap();
        assert_eq!(second.shutdown(8).unwrap(), 0);
        assert_eq!(fixture.open().data().entries.len(), 1);
    }

    #[test]
    fn corrupt_data_blocks_automatic_writes_and_all_clear_can_recover() {
        let fixture = Fixture::new();
        let bad = b"OwnBrowser history 999\nnot our records\n";
        fs::write(fixture.file(), bad).unwrap();
        assert!(Store::open(fixture.file()).is_err());
        let mut store = Store::unavailable(fixture.file(), "bad file".into());
        assert!(store.refresh(10).is_err());
        assert!(store.record(10, "Page", "https://example.com/").is_err());
        assert!(store.configure(enabled(), 10).is_err());
        assert!(store.clear(ClearRange::Hour, 10).is_err());
        assert!(store.shutdown(10).is_err());
        assert_eq!(fs::read(fixture.file()).unwrap(), bad);
        assert!(store.problem().is_some());
        assert_eq!(store.clear(ClearRange::All, 10).unwrap(), 0);
        assert!(store.problem().is_none());
        assert_eq!(fixture.open().data().policy, Policy::default());
        assert!(fixture.open().data().entries.is_empty());
    }

    #[test]
    fn locked_or_readonly_saves_preserve_disk_and_memory() {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        store.configure(enabled(), 1).unwrap();
        record(&mut store, 2, "page");
        let before = fs::read(fixture.file()).unwrap();
        let data = store.data().clone();
        let lock = WriterLock::acquire(&fixture.file()).unwrap();
        assert!(store
            .clear(ClearRange::All, 3)
            .unwrap_err()
            .contains("busy"));
        assert_eq!(fs::read(fixture.file()).unwrap(), before);
        assert_eq!(store.data(), &data);
        drop(lock);
        let original = fs::metadata(fixture.file()).unwrap().permissions();
        let mut permissions = original.clone();
        permissions.set_readonly(true);
        fs::set_permissions(fixture.file(), permissions).unwrap();
        assert!(store
            .clear(ClearRange::All, 4)
            .unwrap_err()
            .contains("read-only"));
        assert_eq!(fs::read(fixture.file()).unwrap(), before);
        assert_eq!(store.data(), &data);
        fs::set_permissions(fixture.file(), original).unwrap();
        assert_eq!(store.clear(ClearRange::All, 5).unwrap(), 1);
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }

    #[test]
    fn bounded_file_and_entries_evict_oldest_and_reject_bad_records() {
        let mut data = Data {
            policy: enabled(),
            entries: Vec::new(),
            next_id: MAX_ENTRIES as u64 + 2,
        };
        for id in (1..data.next_id).rev() {
            data.entries.push(Entry {
                id,
                timestamp: id,
                title: "Title".into(),
                url: "https://example.com/".into(),
            });
        }
        trim(&mut data);
        assert_eq!(data.entries.len(), MAX_ENTRIES);
        assert_eq!(data.entries.last().unwrap().id, 2);
        for entry in &mut data.entries {
            entry.url = format!("https://example.com/{}", "a".repeat(16000));
        }
        trim(&mut data);
        let encoded = encode(&data).unwrap();
        assert!(encoded.len() <= MAX_FILE_BYTES);
        assert!(data.entries.len() < 300);
        assert_eq!(decode(&encoded).unwrap(), data);
        assert!(decode(&vec![b'a'; MAX_FILE_BYTES + 1]).is_err());
        let mut bad = data.clone();
        bad.entries[0].id = bad.next_id;
        assert!(validate(&mut bad).is_err());
        let long_title = title_for_history(&"😀".repeat(1024), "https://example.com/");
        assert_eq!(long_title.len(), MAX_TITLE_BYTES);
        assert!(long_title.ends_with('😀'));
        for retention in [0, 2, 31, 365] {
            assert!(validate_policy(Policy {
                retention_days: Some(retention),
                ..Policy::default()
            })
            .is_err());
        }
    }

    #[test]
    fn unsafe_addresses_are_not_recorded_and_profiles_are_untouched() {
        let fixture = Fixture::new();
        let profile = fixture.0.join("profile.txt");
        fs::write(&profile, "Unrelated bookmarks and settings").unwrap();
        let mut store = fixture.open();
        store.configure(enabled(), 1).unwrap();
        for url in [
            "javascript:run()",
            "data:text/html,test",
            "https://user:password@example.com/",
            "about:home",
            "about:home#test",
            "relative.html",
            "file://remote/share/page.html",
        ] {
            assert_eq!(store.record(2, "Unsafe", url).unwrap(), None, "{url}");
        }
        assert!(store.data().entries.is_empty());
        let missing = fixture.0.join("not-present.html");
        let file = navigation::Location {
            path: Some(missing),
            url: None,
            fragment: String::new(),
            root: None,
        }
        .display();
        assert!(store.record(3, "Missing later", &file).unwrap().is_some());
        store.clear(ClearRange::All, 4).unwrap();
        assert_eq!(
            fs::read_to_string(profile).unwrap(),
            "Unrelated bookmarks and settings"
        );
    }
}
