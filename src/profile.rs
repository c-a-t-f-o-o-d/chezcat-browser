//! Local bookmarks and preferences. The browser engine and webpages never own
//! this store; only explicit browser-UI actions may save it.
//!
//! The small UTF-8 format has a version header and tab-separated records.
//! Text escapes are \t, \n, \r and \\. Reads never create a directory or file.
//! A save stages a complete bounded file, synchronizes it, then replaces the
//! previous file under an exclusive browser-writer lock. Exact previous bytes
//! detect edits made since this window loaded its profile.

use crate::navigation;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "profile_bookmarks.rs"]
mod bookmark_tree;
pub use bookmark_tree::BookmarkItem;

pub fn parse_color(value: &str) -> Result<Option<u32>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(
            "Use a six-digit color such as #67D8C6, or leave it empty to use the page's colors."
                .into(),
        );
    }
    u32::from_str_radix(hex, 16)
        .map(Some)
        .map_err(|_| "Invalid RGB color.".into())
}

pub fn format_color(value: Option<u32>) -> String {
    value.map(|v| format!("#{v:06X}")).unwrap_or_default()
}

pub use crate::search::SearchEngine;

pub const MAX_BOOKMARKS: usize = 512;
pub const MAX_TITLE_BYTES: usize = 1024;
pub const MAX_ADDRESS_BYTES: usize = 16 * 1024;
pub const MAX_PROFILE_BYTES: usize = 1024 * 1024;
const HEADER: &str = "OwnBrowser profile 2";
const LEGACY_HEADER: &str = "OwnBrowser profile 1";
pub const MAX_FOLDERS: usize = 128;
pub const MAX_FOLDER_DEPTH: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub search_engine: SearchEngine,
    pub javascript: bool,
    pub images: bool,
    pub blocking: crate::blocking::Config,
    pub default_zoom: u16,
    pub home: Option<String>,
    pub dark_mode: bool,
    pub dark_pages: bool,
    pub bookmark_bar: bool,
    pub link_color: Option<u32>,
    pub visited_color: Option<u32>,
    pub download_folder: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            search_engine: SearchEngine::Google,
            javascript: true,
            images: true,
            blocking: crate::blocking::Config::default(),
            default_zoom: 100,
            home: None,
            dark_mode: false,
            dark_pages: false,
            bookmark_bar: true,
            link_color: None,
            visited_color: None,
            download_folder: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
    pub folder: u32,
    pub order: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookmarkFolder {
    pub id: u32,
    pub parent: u32,
    pub title: String,
    pub order: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Data {
    pub settings: Settings,
    pub bookmarks: Vec<Bookmark>,
    pub folders: Vec<BookmarkFolder>,
}

#[derive(Debug)]
pub struct Store {
    path: Option<PathBuf>,
    data: Data,
    // None means the path did not exist when loaded. Keep bytes rather than a
    // timestamp/hash so coarse timestamps and hash collisions cannot hide edits.
    original: Option<Vec<u8>>,
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
            original: None,
            problem: None,
        }
    }

    /// Open an explicit file without creating it or its parent directories.
    /// Relative paths become absolute once, independent of future navigation.
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let path = absolute_path(path)?;
        let original = read_snapshot(&path)?;
        let data = match &original {
            Some(bytes) => decode(bytes)?,
            None => Data::default(),
        };
        Ok(Self {
            path: Some(path),
            data,
            original,
            problem: None,
        })
    }

    /// Retain a failed profile's identity and error without making its contents
    /// writable. A startup failure must never turn into an empty overwrite.
    pub fn unavailable(path: PathBuf, reason: String) -> Self {
        let mut fallback=Self::memory();
        // A corrupt/unknown preference must never silently send typed text.
        fallback.data.settings.search_engine=SearchEngine::None;
        Self {
            path: Some(absolute_path(path.clone()).unwrap_or(path)),
            problem: Some(reason),
            ..fallback
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

    /// Save first, then publish the new data to the caller. Failure preserves
    /// both the last committed UI model and the previous destination contents.
    /// The caller owns its staged clone and can keep an editor open for retry.
    pub fn save(&mut self, mut next: Data) -> Result<(), String> {
        if let Some(problem) = &self.problem {
            return Err(format!(
                "This profile is read-only because it could not be loaded: {problem}"
            ));
        }
        normalize(&mut next)?;
        let encoded = encode(&next)?;
        if next == self.data {
            return Ok(());
        }
        let Some(path) = &self.path else {
            self.data = next;
            return Ok(());
        };
        let parent = path.parent().ok_or("The profile has no parent folder.")?;
        let mut directories = fs::DirBuilder::new();
        directories.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            directories.mode(0o700);
        }
        directories
            .create(parent)
            .map_err(|error| format!("Cannot create the profile folder: {error}"))?;
        let _lock = WriterLock::acquire(path)?;
        self.check_original(path)?;

        let mut temporary = Temporary::create(path)?;
        let output = temporary.file.as_mut().unwrap();
        output
            .write_all(&encoded)
            .and_then(|()| output.sync_all())
            .map_err(|error| format!("Cannot finish saving the profile: {error}"))?;
        // Close before rename, including on Windows. The previous file is
        // untouched if writing, synchronization, or replacement fails.
        temporary.file.take();
        self.check_original(path)?;
        fs::rename(&temporary.path, path)
            .map_err(|error| format!("Cannot replace the profile file: {error}"))?;
        temporary.installed = true;
        self.original = Some(encoded);
        self.data = next;
        Ok(())
    }

    fn check_original(&self, path: &Path) -> Result<(), String> {
        if read_snapshot(path)? != self.original {
            return Err("Settings or bookmarks changed on disk after this window loaded them. Restart ChezCat to load the newer profile before saving.".into());
        }
        if fs::metadata(path).is_ok_and(|metadata| metadata.permissions().readonly()) {
            return Err(
                "The profile file is marked read-only. Its contents have not been changed.".into(),
            );
        }
        Ok(())
    }
}

/// The Windows desktop defaults to a per-user path that survives replacing the
/// extracted program folder. Other hosts may still use an explicit profile.
pub fn default_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let directory = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or("LOCALAPPDATA is unavailable; choose a folder with --profile-dir.")?;
        Ok(directory.join("OwnBrowser").join("profile.txt"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Automatic profiles are available in the Windows desktop build; choose a folder with --profile-dir.".into())
    }
}

fn absolute_path(path: PathBuf) -> Result<PathBuf, String> {
    if path.file_name().is_none() {
        return Err("A profile path must name a file.".into());
    }
    if path.is_absolute() {
        Ok(path)
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(path))
            .map_err(|error| format!("Cannot locate the profile folder: {error}"))
    }
}

fn read_snapshot(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Cannot inspect the profile: {error}")),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("The profile must be a regular file, not a directory or symbolic link.".into());
    }
    if metadata.len() > MAX_PROFILE_BYTES as u64 {
        return Err("The profile exceeds the 1 MiB file limit.".into());
    }
    let file = File::open(path).map_err(|error| format!("Cannot open the profile: {error}"))?;
    if !file
        .metadata()
        .map_err(|error| format!("Cannot inspect the opened profile: {error}"))?
        .is_file()
    {
        return Err("The opened profile is not a regular file.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_PROFILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read the profile: {error}"))?;
    if bytes.len() > MAX_PROFILE_BYTES {
        return Err("The profile exceeds the 1 MiB file limit.".into());
    }
    Ok(Some(bytes))
}

fn normalize(data: &mut Data) -> Result<(), String> {
    data.settings.blocking.normalize()?;
    data.validate_folders()?;
    if data.settings.link_color.is_some_and(|c| c > 0xffffff)
        || data.settings.visited_color.is_some_and(|c| c > 0xffffff)
    {
        return Err("Link colors must be six-digit RGB values.".into());
    }
    if let Some(folder) = &data.settings.download_folder {
        if folder.len() > MAX_ADDRESS_BYTES || folder.chars().any(char::is_control) {
            return Err("The downloads folder is invalid or too long.".into());
        }
        crate::downloads::validate_folder(Path::new(folder))?;
    }
    if !(50..=300).contains(&data.settings.default_zoom) {
        return Err("Default zoom must be between 50 and 300 percent.".into());
    }
    if data.bookmarks.len() > MAX_BOOKMARKS {
        return Err(format!("The bookmark limit is {MAX_BOOKMARKS}."));
    }
    if let Some(home) = &mut data.settings.home {
        *home = checked_address(home)?;
    }
    let mut addresses = std::collections::HashSet::new();
    for bookmark in &mut data.bookmarks {
        if bookmark.title.trim().is_empty() {
            return Err("A bookmark needs a title.".into());
        }
        if bookmark.title.len() > MAX_TITLE_BYTES {
            return Err(format!(
                "Bookmark titles are limited to {MAX_TITLE_BYTES} UTF-8 bytes."
            ));
        }
        if bookmark
            .title
            .chars()
            .any(|ch| ch.is_control() && !matches!(ch, '\t' | '\r' | '\n'))
        {
            return Err(
                "Bookmark titles cannot contain control characters other than tab or line breaks."
                    .into(),
            );
        }
        bookmark.url = checked_address(&bookmark.url)?;
        if !addresses.insert(bookmark.url.clone()) {
            return Err("A bookmark already exists for that address.".into());
        }
    }
    Ok(())
}

fn checked_address(value: &str) -> Result<String, String> {
    if value.len() > MAX_ADDRESS_BYTES {
        return Err("Saved addresses are limited to 16 KiB.".into());
    }
    navigation::stored_address(value)
}

fn decode(bytes: &[u8]) -> Result<Data, String> {
    if bytes.len() > MAX_PROFILE_BYTES {
        return Err("The profile exceeds the 1 MiB file limit.".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "The profile is not valid UTF-8.")?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines();
    let header = lines.next();
    if header != Some(HEADER) && header != Some(LEGACY_HEADER) {
        return Err(
            "The profile header or version is not supported. Its file has not been changed.".into(),
        );
    }
    let mut data = Data::default();
    let mut settings = 0u8;
    let mut extension_settings = std::collections::HashSet::new();
    for (index, line) in lines.enumerate() {
        let fields: Vec<_> = line.split('\t').collect();
        let bad = || format!("Invalid profile record on line {}.", index + 2);
        match fields.as_slice() {
            ["javascript", value] | ["images", value] => {
                let bit = if fields[0] == "javascript" { 1 } else { 2 };
                if settings & bit != 0 {
                    return Err(bad());
                }
                settings |= bit;
                let enabled = match *value {
                    "true" => true,
                    "false" => false,
                    _ => return Err(bad()),
                };
                if bit == 1 {
                    data.settings.javascript = enabled;
                } else {
                    data.settings.images = enabled;
                }
            }
            ["default_zoom", value] => {
                if settings & 4 != 0 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(bad());
                }
                settings |= 4;
                data.settings.default_zoom = value.parse().map_err(|_| bad())?;
            }
            ["blocking", value] | ["blocking_builtins", value] => {
                let bit = if fields[0] == "blocking" { 16 } else { 32 };
                if settings & bit != 0 {
                    return Err(bad());
                }
                settings |= bit;
                let value = match *value {
                    "true" => true,
                    "false" => false,
                    _ => return Err(bad()),
                };
                if bit == 16 {
                    data.settings.blocking.enabled = value;
                } else {
                    data.settings.blocking.builtins = value;
                }
            }
            ["block_rule", value] => {
                if data.settings.blocking.rules.len() >= crate::blocking::MAX_RULES {
                    return Err("The profile contains too many request-blocking rules.".into());
                }
                data.settings
                    .blocking
                    .rules
                    .push(unescape(value, crate::blocking::MAX_RULE_BYTES)?);
            }
            ["home", value] => {
                if settings & 8 != 0 {
                    return Err(bad());
                }
                settings |= 8;
                if !value.is_empty() {
                    data.settings.home = Some(unescape(value, MAX_ADDRESS_BYTES)?);
                }
            }
            [name @ ("dark_mode" | "dark_pages" | "bookmark_bar"), value] => {
                if !extension_settings.insert(*name) {
                    return Err(bad());
                }
                let enabled = match *value {
                    "true" => true,
                    "false" => false,
                    _ => return Err(bad()),
                };
                match *name {
                    "dark_mode" => data.settings.dark_mode = enabled,
                    "dark_pages" => data.settings.dark_pages = enabled,
                    _ => data.settings.bookmark_bar = enabled,
                }
            }
            [name @ ("link_color" | "visited_color"), value] => {
                if !extension_settings.insert(*name) {
                    return Err(bad());
                }
                let color = parse_color(value)?;
                if *name == "link_color" {
                    data.settings.link_color = color;
                } else {
                    data.settings.visited_color = color;
                }
            }
            ["search_engine", value] => {
                if !extension_settings.insert("search_engine") { return Err(bad()); }
                data.settings.search_engine=SearchEngine::parse(value)?;
            }
            ["download_folder", value] => {
                if !extension_settings.insert("download_folder") {
                    return Err(bad());
                }
                if !value.is_empty() {
                    data.settings.download_folder = Some(unescape(value, MAX_ADDRESS_BYTES)?);
                }
            }
            ["folder", id, parent, order, title] => {
                if data.folders.len() >= MAX_FOLDERS {
                    return Err("Too many bookmark folders.".into());
                }
                data.folders.push(BookmarkFolder {
                    id: id.parse().map_err(|_| bad())?,
                    parent: parent.parse().map_err(|_| bad())?,
                    order: order.parse().map_err(|_| bad())?,
                    title: unescape(title, MAX_TITLE_BYTES)?,
                });
            }
            ["bookmark2", folder, order, title, url] => {
                if data.bookmarks.len() >= MAX_BOOKMARKS {
                    return Err("Too many bookmarks.".into());
                }
                data.bookmarks.push(Bookmark {
                    title: unescape(title, MAX_TITLE_BYTES)?,
                    url: unescape(url, MAX_ADDRESS_BYTES)?,
                    folder: folder.parse().map_err(|_| bad())?,
                    order: order.parse().map_err(|_| bad())?,
                });
            }
            ["bookmark", title, url] => {
                if data.bookmarks.len() == MAX_BOOKMARKS {
                    return Err(format!("The profile exceeds {MAX_BOOKMARKS} bookmarks."));
                }
                data.bookmarks.push(Bookmark {
                    title: unescape(title, MAX_TITLE_BYTES)?,
                    url: unescape(url, MAX_ADDRESS_BYTES)?,
                    folder: 0,
                    order: data.bookmarks.len() as u32,
                });
            }
            _ => return Err(bad()),
        }
    }
    if settings & 15 != 15 {
        return Err(
            "The profile is missing a settings record. Its file has not been changed.".into(),
        );
    }
    normalize(&mut data)?;
    Ok(data)
}

fn unescape(text: &str, limit: usize) -> Result<String, String> {
    if text.len() > limit * 2 {
        return Err("An escaped profile field exceeds its size limit.".into());
    }
    let mut output = String::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        let ch = if ch == '\\' {
            match chars.next() {
                Some('\\') => '\\',
                Some('t') => '\t',
                Some('r') => '\r',
                Some('n') => '\n',
                _ => return Err("The profile contains an invalid text escape.".into()),
            }
        } else {
            if ch.is_control() {
                return Err("The profile contains an unescaped control character.".into());
            }
            ch
        };
        if output.len() + ch.len_utf8() > limit {
            return Err("A profile field exceeds its size limit.".into());
        }
        output.push(ch);
    }
    Ok(output)
}

fn encode(data: &Data) -> Result<Vec<u8>, String> {
    let mut text = format!(
        "{HEADER}\njavascript\t{}\nimages\t{}\ndefault_zoom\t{}\nhome\t",
        data.settings.javascript, data.settings.images, data.settings.default_zoom
    );
    escaped(&mut text, data.settings.home.as_deref().unwrap_or(""))?;
    text.push('\n');
    // Omit inactive blocking records. Reading a legacy file does not rewrite it;
    // an actual save uses the current header and preserves its existing choices.
    if data.settings.blocking != crate::blocking::Config::default() {
        text.push_str(&format!(
            "blocking\t{}\nblocking_builtins\t{}\n",
            data.settings.blocking.enabled, data.settings.blocking.builtins
        ));
        for rule in &data.settings.blocking.rules {
            text.push_str("block_rule\t");
            escaped(&mut text, rule)?;
            text.push('\n');
        }
    }
    text.push_str(&format!("dark_mode\t{}\ndark_pages\t{}\nbookmark_bar\t{}\nlink_color\t{}\nvisited_color\t{}\ndownload_folder\t",
        data.settings.dark_mode, data.settings.dark_pages, data.settings.bookmark_bar,
        format_color(data.settings.link_color), format_color(data.settings.visited_color)));
    escaped(
        &mut text,
        data.settings.download_folder.as_deref().unwrap_or(""),
    )?;
    text.push('\n');
    text.push_str(&format!("search_engine\t{}\n",data.settings.search_engine.name()));
    for folder in &data.folders {
        text.push_str(&format!(
            "folder\t{}\t{}\t{}\t",
            folder.id, folder.parent, folder.order
        ));
        escaped(&mut text, &folder.title)?;
        text.push('\n');
    }
    for bookmark in &data.bookmarks {
        if text.len() + 11 > MAX_PROFILE_BYTES {
            return Err("The profile exceeds the 1 MiB file limit.".into());
        }
        text.push_str(&format!(
            "bookmark2\t{}\t{}\t",
            bookmark.folder, bookmark.order
        ));
        escaped(&mut text, &bookmark.title)?;
        text.push('\t');
        escaped(&mut text, &bookmark.url)?;
        text.push('\n');
    }
    if text.len() > MAX_PROFILE_BYTES {
        return Err("The profile exceeds the 1 MiB file limit.".into());
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
        let bytes = escape.map_or(ch.len_utf8(), str::len);
        if output.len() + bytes > MAX_PROFILE_BYTES {
            return Err("The profile exceeds the 1 MiB file limit.".into());
        }
        if let Some(escape) = escape {
            output.push_str(escape);
        } else {
            output.push(ch);
        }
    }
    Ok(())
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

fn create_private_file(path: &Path) -> std::io::Result<File> {
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
    fn acquire(profile: &Path) -> Result<Self, String> {
        let path = sibling(profile, ".lock");
        let file = create_private_file(&path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    format!("The profile is busy. Close other browser windows and retry. If no browser is running, remove the leftover lock file {}.", path.display())
                } else {
                    format!("Cannot lock the profile for saving: {error}")
                }
            })?;
        let mut lock = Self {
            path,
            file: Some(file),
        };
        writeln!(lock.file.as_mut().unwrap(), "{}", std::process::id())
            .map_err(|error| format!("Cannot write the profile lock: {error}"))?;
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
    fn create(profile: &Path) -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let suffix = format!(
            ".tmp-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = sibling(profile, &suffix);
        let file = create_private_file(&path)
            .map_err(|error| format!("Cannot create the staged profile file: {error}"))?;
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

    #[test]
    fn address_search_profiles_migrate_without_writes_and_keep_explicit_none() {
        let fixture=Fixture::new();
        let legacy=b"OwnBrowser profile 1\njavascript\ttrue\nimages\ttrue\ndefault_zoom\t100\nhome\t\n";
        fs::write(fixture.file(),legacy).unwrap();
        let mut store=Store::open(fixture.file()).unwrap();
        assert_eq!(store.data().settings.search_engine,SearchEngine::Google);
        assert_eq!(fs::read(fixture.file()).unwrap(),legacy);
        let mut data=sample();data.settings.search_engine=SearchEngine::None;
        store.save(data.clone()).unwrap();assert_eq!(Store::open(fixture.file()).unwrap().data(),&data);
        for engine in SearchEngine::ALL {data.settings.search_engine=engine;assert_eq!(decode(&encode(&data).unwrap()).unwrap(),data);}
        for tail in ["search_engine\tUnknown\n","search_engine\t\n","search_engine\tGoogle\nsearch_engine\tNone\n"] {
            let bytes=[legacy.as_slice(),tail.as_bytes()].concat();fs::write(fixture.file(),&bytes).unwrap();
            let error=Store::open(fixture.file()).unwrap_err();let mut store=Store::unavailable(fixture.file(),error);
            assert!(store.problem().is_some());assert_eq!(store.data().settings.search_engine,SearchEngine::None);
            assert!(store.save(Data::default()).is_err());assert_eq!(fs::read(fixture.file()).unwrap(),bytes);
        }
    }

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "chezcat-profile-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn file(&self) -> PathBuf {
            self.0.join("profile.txt")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sample() -> Data {
        Data {
            settings: Settings {
                javascript: false,
                images: false,
                default_zoom: 125,
                home: Some("https://example.com/home#top".into()),
                blocking: crate::blocking::Config::default(),
                ..Settings::default()
            },
            folders: Vec::new(),
            bookmarks: vec![
                Bookmark {
                    title: "Café 😀 \\ tab\tline\nreturn\r".into(),
                    url: "https://example.com/path?q=one%26two#part".into(),
                    ..Bookmark::default()
                },
                Bookmark {
                    title: "Home".into(),
                    url: "about:home".into(),
                    ..Bookmark::default()
                },
            ],
        }
    }

    #[test]
    fn blocking_extensions_preserve_old_profiles_and_validate_before_save() {
        let fixture = Fixture::new();
        let mut store = Store::open(fixture.file()).unwrap();
        store.save(sample()).unwrap();
        let original = fs::read(fixture.file()).unwrap();
        assert!(!String::from_utf8_lossy(&original).contains("blocking\t"));
        assert_eq!(
            Store::open(fixture.file())
                .unwrap()
                .data()
                .settings
                .blocking,
            crate::blocking::Config::default()
        );
        let mut next = store.data().clone();
        next.settings.blocking.enabled = true;
        next.settings.blocking.rules =
            vec!["ADS.example.com".into(), "@@path:/ads/allowed/".into()];
        store.save(next).unwrap();
        assert_eq!(
            store.data().settings.blocking.rules[0],
            "host:ads.example.com"
        );
        let reopened = Store::open(fixture.file()).unwrap();
        assert_eq!(reopened.data(), store.data());
        let saved = fs::read(fixture.file()).unwrap();
        let mut invalid = store.data().clone();
        invalid.settings.blocking.rules.push("path:/bad*".into());
        assert!(store.save(invalid).is_err());
        assert_eq!(fs::read(fixture.file()).unwrap(), saved);
        assert_eq!(store.data(), reopened.data());
        let duplicate = [saved.as_slice(), b"blocking\ttrue\n"].concat();
        assert!(decode(&duplicate).is_err());
    }

    #[test]
    fn missing_and_memory_profiles_do_not_create_files_on_read() {
        let fixture = Fixture::new();
        let directory = fixture.0.join("not-created");
        let file = directory.join("profile.txt");
        let store = Store::open(file.clone()).unwrap();
        assert_eq!(store.path(), Some(file.as_path()));
        assert_eq!(store.data(), &Data::default());
        assert!(!directory.exists());
        let mut memory = Store::memory();
        memory.save(sample()).unwrap();
        assert_eq!(memory.data(), &sample());
        assert!(memory.path().is_none());
        assert!(!directory.exists());
    }

    #[test]
    fn profile_round_trip_preserves_unicode_text_and_replaces_atomically() {
        let fixture = Fixture::new();
        let file = fixture.file();
        let mut store = Store::open(file.clone()).unwrap();
        store.save(sample()).unwrap();
        let bytes = fs::read(&file).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.contains("Café 😀 \\\\ tab\\tline\\nreturn\\r\t"));
        assert_eq!(Store::open(file.clone()).unwrap().data(), &sample());
        let mut next = sample();
        next.settings.javascript = true;
        next.bookmarks.remove(0);
        store.save(next.clone()).unwrap();
        assert_eq!(Store::open(file.clone()).unwrap().data(), &next);
        assert_ne!(fs::read(&file).unwrap(), bytes);
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let windows_text = format!("\u{feff}{}", text.replace('\n', "\r\n"));
        assert_eq!(decode(windows_text.as_bytes()).unwrap(), sample());
    }

    #[test]
    fn stale_windows_and_external_edits_never_replace_the_loaded_snapshot() {
        let fixture = Fixture::new();
        let file = fixture.file();
        let mut first = Store::open(file.clone()).unwrap();
        let mut second = Store::open(file.clone()).unwrap();
        first.save(sample()).unwrap();
        let committed = fs::read(&file).unwrap();
        let mut different = sample();
        different.settings.default_zoom = 150;
        assert!(second
            .save(different.clone())
            .unwrap_err()
            .contains("changed on disk"));
        assert_eq!(second.data(), &Data::default());
        assert_eq!(fs::read(&file).unwrap(), committed);

        fs::write(&file, b"An external editor changed this file.").unwrap();
        assert!(first
            .save(different)
            .unwrap_err()
            .contains("changed on disk"));
        assert_eq!(first.data(), &sample());
        assert_eq!(
            fs::read(&file).unwrap(),
            b"An external editor changed this file."
        );
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }

    #[test]
    fn lock_and_folder_write_failures_keep_the_old_ui_and_file() {
        let fixture = Fixture::new();
        let file = fixture.file();
        let mut store = Store::open(file.clone()).unwrap();
        store.save(sample()).unwrap();
        let committed = fs::read(&file).unwrap();
        let lock = WriterLock::acquire(&file).unwrap();
        let mut next = sample();
        next.settings.images = true;
        assert!(store.save(next.clone()).unwrap_err().contains("busy"));
        assert_eq!(store.data(), &sample());
        assert_eq!(fs::read(&file).unwrap(), committed);
        drop(lock);
        store.save(next.clone()).unwrap();
        assert_eq!(Store::open(file.clone()).unwrap().data(), &next);
        let permissions = fs::metadata(&file).unwrap().permissions();
        let mut readonly = permissions.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&file, readonly).unwrap();
        assert!(store.save(sample()).unwrap_err().contains("read-only"));
        assert_eq!(store.data(), &next);
        assert_eq!(decode(&fs::read(&file).unwrap()).unwrap(), next);
        fs::set_permissions(&file, permissions).unwrap();

        let missing = fixture.0.join("blocked").join("profile.txt");
        let mut blocked = Store::open(missing.clone()).unwrap();
        fs::write(
            fixture.0.join("blocked"),
            b"A file occupies the parent path.",
        )
        .unwrap();
        assert!(blocked.save(sample()).is_err());
        assert_eq!(blocked.data(), &Data::default());
        assert!(!missing.exists());
    }

    #[test]
    fn malformed_and_unknown_profiles_are_read_only_until_explicit_repair() {
        let fixture = Fixture::new();
        let valid = String::from_utf8(encode(&sample()).unwrap()).unwrap();
        for text in [
            "".to_owned(),
            valid.replace(HEADER, "OwnBrowser profile 999"),
            valid.replace("images\tfalse", "images\tfalse\nimages\ttrue"),
            valid.replace("images\tfalse\n", ""),
            valid.replace("default_zoom\t125", "default_zoom\t499"),
            valid.replace("javascript\tfalse", "javascript\tmaybe"),
            valid.replace("\\tline", "\\qline"),
            valid.replace("images\tfalse", "unexpected\tfalse"),
        ] {
            fs::write(fixture.file(), text.as_bytes()).unwrap();
            let error = Store::open(fixture.file()).unwrap_err();
            let mut store = Store::unavailable(fixture.file(), error);
            assert!(store.problem().is_some());
            assert!(store.save(sample()).unwrap_err().contains("read-only"));
            let mut safe=Data::default();safe.settings.search_engine=SearchEngine::None;
            assert_eq!(store.data(), &safe);
            assert_eq!(fs::read(fixture.file()).unwrap(), text.as_bytes());
        }
    }

    #[test]
    fn stored_addresses_are_canonical_without_requiring_local_files_to_exist() {
        let mut data = sample();
        data.bookmarks[0].url = "HTTPS://EXAMPLE.COM:443/a/../path#space here".into();
        let mut store = Store::memory();
        store.save(data).unwrap();
        assert_eq!(
            store.data().bookmarks[0].url,
            "https://example.com/path#space%20here"
        );

        let nonexistent =
            std::env::temp_dir().join("chezcat-intentionally-absent-bookmark-20.html");
        let location = navigation::Location {
            path: Some(nonexistent),
            url: None,
            fragment: "old section".into(),
            root: None,
        };
        let mut next = store.data().clone();
        next.bookmarks.push(Bookmark {
            title: "Missing local page".into(),
            url: location.display(),
            ..Bookmark::default()
        });
        store.save(next.clone()).unwrap();
        assert_eq!(decode(&encode(&next).unwrap()).unwrap(), next);
        for url in [
            "relative.html",
            "google.com",
            "#part",
            "javascript:run()",
            "file://server/share/page",
            "https://name:password@example.com/",
        ] {
            let mut invalid = sample();
            invalid.bookmarks[0].url = url.into();
            assert!(store.save(invalid).is_err(), "{url}");
        }
        assert_eq!(store.data(), &next);
    }

    #[test]
    fn profile_limits_reject_whole_changes_without_partial_data() {
        let mut store = Store::memory();
        let mut oversized_title = sample();
        oversized_title.bookmarks[0].title = "x".repeat(MAX_TITLE_BYTES + 1);
        assert!(store.save(oversized_title).is_err());
        let mut many = Data::default();
        for index in 0..=MAX_BOOKMARKS {
            many.bookmarks.push(Bookmark {
                title: format!("Bookmark {index}"),
                url: format!("https://example.com/{index}"),
                ..Bookmark::default()
            });
        }
        assert!(store.save(many).is_err());
        let mut too_large = Data::default();
        for index in 0..MAX_BOOKMARKS {
            too_large.bookmarks.push(Bookmark {
                title: format!("Bookmark {index}"),
                url: format!("https://example.com/{index}/{}", "x".repeat(4096)),
                ..Bookmark::default()
            });
        }
        assert!(store.save(too_large).unwrap_err().contains("1 MiB"));
        assert!(decode(&vec![b'x'; MAX_PROFILE_BYTES + 1]).is_err());
        let mut duplicate = sample();
        duplicate.bookmarks.push(duplicate.bookmarks[0].clone());
        assert!(store
            .save(duplicate)
            .unwrap_err()
            .contains("already exists"));
        assert_eq!(store.data(), &Data::default());
    }

    #[test]
    fn nonregular_profile_destinations_are_rejected() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.file()).unwrap();
        assert!(Store::open(fixture.file())
            .unwrap_err()
            .contains("regular file"));
        #[cfg(unix)]
        {
            let target = fixture.0.join("actual-profile");
            fs::write(&target, encode(&sample()).unwrap()).unwrap();
            let link = fixture.0.join("linked-profile");
            std::os::unix::fs::symlink(&target, &link).unwrap();
            assert!(Store::open(link).unwrap_err().contains("symbolic link"));
            assert_eq!(fs::read(&target).unwrap(), encode(&sample()).unwrap());
        }
    }
}
