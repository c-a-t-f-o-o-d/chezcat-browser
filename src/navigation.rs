//! Local-file and HTTP(S) locations with separate trust boundaries.
//! Explicit file addresses may establish a new content root; file references
//! cannot escape it, including through symlinks. Web references never access
//! local files. Queries are supported for web URLs and rejected for local files.

use crate::url::{self, HttpUrl};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub path: Option<PathBuf>,
    pub url: Option<HttpUrl>,
    pub fragment: String,
    pub root: Option<PathBuf>,
}

impl Location {
    pub(crate) fn payload_bytes(&self) -> usize {
        self.path.as_ref().map_or(0, |path| path.capacity())
            .saturating_add(self.root.as_ref().map_or(0, |path| path.capacity()))
            .saturating_add(self.fragment.capacity())
            .saturating_add(self.url.as_ref().map_or(0, |url|
                url.host.capacity().saturating_add(url.path.capacity())
                    .saturating_add(url.query.as_ref().map_or(0, String::capacity))))
    }

    pub fn home() -> Self {
        Self {
            path: None,
            url: None,
            fragment: String::new(),
            root: None,
        }
    }

    /// A file URI keeps spaces, percent signs, and literal '#' filename characters
    /// distinct from navigation syntax when this value is entered again.
    pub fn display(&self) -> String {
        let mut value = match (&self.url, &self.path) {
            (Some(url), _) => url.display(),
            (None, None) => "about:home".to_owned(),
            (None, Some(path)) => {
                let value = path.to_string_lossy();
                #[cfg(windows)]
                let value = value.replace('\\', "/");
                if value.starts_with('/') {
                    format!("file://{}", percent_encode(&value, true))
                } else {
                    format!("file:///{}", percent_encode(&value, true))
                }
            }
        };
        if !self.fragment.is_empty() {
            value.push('#');
            value.push_str(&percent_encode(&self.fragment, false));
        }
        value
    }
}

/// Open a path explicitly selected by the user, with its parent as the new root.
pub fn explicit(path: &Path) -> Result<Location, String> {
    let path = canonical_file(path)?;
    let root = path
        .parent()
        .ok_or_else(|| "The file has no parent folder.".to_owned())?
        .to_path_buf();
    Ok(Location {
        path: Some(path),
        url: None,
        fragment: String::new(),
        root: Some(root),
    })
}

/// Validate a saved address without probing disk or making a request. Bookmarks
/// must remain readable when their local file is temporarily unavailable.
/// Stored addresses are absolute; opening one still uses normal navigation and
/// re-establishes/checks its file boundary at that time.
pub fn stored_address(input: &str) -> Result<String, String> {
    if input.len() > 16 * 1024 || input.chars().any(char::is_control) {
        return Err(
            "Saved addresses must be at most 16 KiB and contain no control characters.".into(),
        );
    }
    let input = input.trim();
    let location = if starts_web_url(input) {
        web_location(url::parse(input)?)
    } else {
        let (reference, fragment) = split_reference(input)?;
        if reference.eq_ignore_ascii_case("about:home") {
            Location {
                fragment,
                ..Location::home()
            }
        } else {
            if !starts_file_uri(reference) {
                return Err("Saved addresses require http://, https://, an absolute file:// address, or about:home.".into());
            }
            let path = file_uri_path(reference)?;
            Location {
                root: path.parent().map(Path::to_path_buf),
                path: Some(path),
                url: None,
                fragment,
            }
        }
    };
    let display = location.display();
    if display.len() > 16 * 1024 {
        return Err("Encoded saved addresses must be at most 16 KiB.".into());
    }
    Ok(display)
}

/// Address-bar input accepts about:home, local paths, file:// and HTTP(S) URLs.
/// A bare dotted host, localhost, IP address or host:port defaults to HTTPS.
/// Explicit paths and existing local files take precedence over that shorthand.
/// Raw paths preserve '%' literally; URI paths and fragments use UTF-8 percent
/// escapes. Matching quotes make a raw path literal, including '#' characters.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum AddressIntent { Navigate, Search }

/// Classify before navigation, never as a fallback after a failure. Only the
/// address bar uses this policy; links, forms and scripts keep address/resolve.
pub fn address_intent(input: &str, current: &Location) -> Result<AddressIntent,String> {
    if input.len()>16*1024 || input.chars().any(char::is_control) {
        return Err("Address input exceeds 16 KiB or contains control characters.".into());
    }
    let input=input.trim();
    if input.is_empty() { return Err("Enter a search or address.".into()); }
    let path_syntax=|value:&str|value.starts_with(['/', '\\']) || value=="." || value==".."
        || value.starts_with("./") || value.starts_with("../") || value.contains('\\')
        || value.as_bytes().get(1)==Some(&b':') && value.as_bytes()[0].is_ascii_alphabetic();
    if let Some(inner)=quoted_path(input) {
        let literal=Path::new(inner);
        let exists=validate_local_path(literal).is_ok() && address_path(literal,current).is_file();
        return Ok(if path_syntax(inner) || exists || starts_web_url(inner) || starts_file_uri(inner) {
            AddressIntent::Navigate
        } else {AddressIntent::Search});
    }
    if path_syntax(input) || input.starts_with(['#','[']) || input.parse::<std::net::IpAddr>().is_ok() || existing_address_file(input,current) {
        return Ok(AddressIntent::Navigate);
    }
    let first=input.split_whitespace().next().unwrap_or("").trim_start_matches('-');
    if first.split_once(':').is_some_and(|(name,_)|["site","filetype","ext","intitle","inurl","intext","allintitle","allinurl","allintext","related","cache","define","before","after"].iter().any(|op|name.eq_ignore_ascii_case(op))) {
        return Ok(AddressIntent::Search);
    }
    if input.contains("://") || input.split_once(':').is_some_and(|(scheme,_)|!scheme.is_empty()
        && scheme.as_bytes()[0].is_ascii_alphabetic() && scheme.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'+'|b'-'|b'.'))) {
        return Ok(AddressIntent::Navigate);
    }
    if !input.chars().any(char::is_whitespace) && (bare_web_address(input) || input.starts_with('[') || input.contains('/')) {
        return Ok(AddressIntent::Navigate);
    }
    Ok(AddressIntent::Search)
}

pub fn address(input: &str, current: &Location) -> Result<Location, String> {
    if input.chars().any(char::is_control) {
        return Err("Addresses cannot contain control characters.".to_owned());
    }
    if starts_web_url(input.trim()) {
        return Ok(web_location(url::parse(input)?));
    }
    if input.trim().starts_with('#') {
        if let Some(url) = &current.url {
            return Ok(web_location(url.resolve(input)?));
        }
    }
    let input = input.trim();
    if input.is_empty() {
        return Err("Enter a website address, a local file path, or about:home.".to_owned());
    }
    if let Some(path) = quoted_path(input) {
        let location = explicit(&address_path(Path::new(path), current))?;
        return Ok(preserve_current_root(location, current));
    }
    if bare_web_address(input) && !existing_address_file(input, current) {
        return Ok(web_location(url::parse(&format!("https://{input}"))?));
    }
    let (reference, fragment) = split_reference(input)?;
    if reference.eq_ignore_ascii_case("about:home") {
        return Ok(Location {
            fragment,
            ..Location::home()
        });
    }
    if reference.is_empty() {
        return Ok(Location {
            fragment,
            ..current.clone()
        });
    }
    let path = if starts_file_uri(reference) {
        file_uri_path(reference)?
    } else {
        validate_local_path(Path::new(reference))?;
        PathBuf::from(reference)
    };
    let mut location = explicit(&address_path(&path, current))?;
    location.fragment = fragment;
    Ok(preserve_current_root(location, current))
}

/// Only user-entered addresses use this heuristic. Document links still use
/// strict URL resolution, so a relative link such as `other.html` stays relative.
fn bare_web_address(input: &str) -> bool {
    let bytes = input.as_bytes();
    if input.starts_with(['/', '\\', '.'])
        || input.contains('\\')
        || input.contains("://")
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
    {
        return false;
    }
    let authority = input.split(['/', '?', '#']).next().unwrap_or("");
    if authority.starts_with('[') {
        // The URL parser validates brackets, the IPv6 address and any port.
        return true;
    }
    let (host, port) = authority
        .rsplit_once(':')
        .map_or((authority, None), |(host, port)| (host, Some(port)));
    host.contains('.')
        || host.eq_ignore_ascii_case("localhost")
        || port.is_some_and(|port| {
            !host.is_empty() && !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit())
        })
}

fn existing_address_file(input: &str, current: &Location) -> bool {
    let reference = input.split(['?', '#']).next().unwrap_or("");
    let path = Path::new(reference);
    validate_local_path(path).is_ok() && address_path(path, current).is_file()
}

fn preserve_current_root(mut location: Location, current: &Location) -> Location {
    // Re-entering the displayed address of a linked child page must not suddenly
    // exclude its parent stylesheets or sibling links by narrowing its root.
    if location.path == current.path {
        location.root = current.root.clone();
    }
    location
}

/// Resolve a document link or asset without widening its original content root.
/// Queries such as image.bmp?v=2 are rejected rather than silently discarded.
/// URL selection/reflection only: no existence check, new root, open or authority.
/// Callers MUST separately use resolve/local-file protection before resource IO.
pub(crate) fn selection_url(input:&str,current:&Location)->Result<String,String>{
    if let Some(url)=&current.url{return Ok(web_location(url.resolve(input)?).display());}
    if starts_web_url(input.trim()){return Ok(web_location(url::parse(input)?).display());}
    let (reference,fragment)=split_reference(input.trim())?;
    if reference.is_empty(){let mut selected=current.clone();selected.fragment=fragment;return Ok(selected.display());}
    if reference.eq_ignore_ascii_case("about:home"){return Ok(Location {fragment,..Location::home()}.display());}
    let path=if starts_file_uri(reference){file_uri_path(reference)?}else{let decoded=percent_decode(reference)?;validate_local_path(Path::new(&decoded))?;PathBuf::from(decoded)};
    let source=current.path.as_ref().ok_or("A local URL selection requires the document path.")?;
    let candidate=if path.is_absolute(){path}else{source.parent().ok_or("The local document has no parent folder.")?.join(path)};
    let mut lexical=PathBuf::new();
    for part in candidate.components(){match part{std::path::Component::CurDir=>{},std::path::Component::ParentDir=>{if lexical.file_name().is_some(){lexical.pop();}},part=>lexical.push(part.as_os_str())}}
    validate_local_path(&lexical)?;
    Ok(Location {path:Some(lexical),url:None,fragment,root:current.root.clone()}.display())
}

pub fn resolve(input: &str, current: &Location) -> Result<Location, String> {
    if let Some(url) = &current.url {
        return Ok(web_location(url.resolve(input)?));
    }
    // Local pages may offer explicit web links. Automatic asset policy belongs
    // to the caller, which can forbid file-to-network resource requests.
    if starts_web_url(input.trim()) {
        return Ok(web_location(url::parse(input)?));
    }
    let (reference, fragment) = split_reference(input.trim())?;
    if reference.is_empty() {
        return Ok(Location {
            fragment,
            ..current.clone()
        });
    }
    if reference.eq_ignore_ascii_case("about:home") {
        return Ok(Location {
            fragment,
            ..Location::home()
        });
    }
    let source = current
        .path
        .as_ref()
        .ok_or_else(|| "Open a local file before following file links.".to_owned())?;
    let root = current
        .root
        .as_ref()
        .ok_or_else(|| "The current document has no content root.".to_owned())?;
    let path = if starts_file_uri(reference) {
        file_uri_path(reference)?
    } else {
        let decoded = percent_decode(reference)?;
        validate_local_path(Path::new(&decoded))?;
        PathBuf::from(decoded)
    };
    let candidate = if path.is_absolute() {
        path
    } else {
        source
            .parent()
            .ok_or_else(|| "The current document has no parent folder.".to_owned())?
            .join(path)
    };
    let canonical = canonical_file(&candidate)?;
    if !canonical.starts_with(root) {
        return Err(
            "That file is outside this document's content folder. Open it explicitly to use it."
                .to_owned(),
        );
    }
    Ok(Location {
        path: Some(canonical),
        url: None,
        fragment,
        root: Some(root.clone()),
    })
}

fn web_location((url, fragment): (HttpUrl, String)) -> Location {
    Location {
        path: None,
        url: Some(url),
        fragment,
        root: None,
    }
}

fn starts_web_url(input: &str) -> bool {
    input
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http://"))
        || input
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
}

/// Read only a regular local file, and never allocate based on its reported size.
pub fn read_limited(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = open_local_file(path)?;
    let maximum = u64::try_from(limit)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| "The requested file size limit is too large.".to_owned())?;
    let mut bytes = Vec::new();
    file.take(maximum)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read file: {error}"))?;
    if bytes.len() > limit {
        return Err(format!("This file exceeds the {limit}-byte size limit."));
    }
    Ok(bytes)
}

/// Shared regular-file/path checks for bounded and cancellable readers.
pub(crate) fn open_local_file(path: &Path) -> Result<File, String> {
    let path = canonical_file(path)?;
    let file = File::open(&path).map_err(|error| format!("Cannot open file: {error}"))?;
    if !file
        .metadata()
        .map_err(|error| format!("Cannot inspect file: {error}"))?
        .is_file()
    {
        return Err("Choose a regular local file.".to_owned());
    }
    Ok(file)
}

fn address_path(path: &Path, current: &Location) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    current
        .path
        .as_ref()
        .and_then(|source| source.parent())
        .map(|parent| parent.join(path))
        .unwrap_or_else(|| path.to_path_buf())
}

fn quoted_path(input: &str) -> Option<&str> {
    if input.len() >= 2 {
        let first = input.as_bytes()[0];
        if matches!(first, b'\'' | b'"') && input.as_bytes()[input.len() - 1] == first {
            return Some(&input[1..input.len() - 1]);
        }
    }
    None
}

fn split_reference(input: &str) -> Result<(&str, String), String> {
    let (path, fragment) = input.split_once('#').unwrap_or((input, ""));
    if path.contains('?') {
        return Err("Queries are not supported for local files in this build.".to_owned());
    }
    Ok((path, percent_decode(fragment)?))
}

fn starts_file_uri(input: &str) -> bool {
    input
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file://"))
}

fn file_uri_path(input: &str) -> Result<PathBuf, String> {
    let rest = &input[7..];
    let encoded_path = if rest.starts_with('/') {
        rest
    } else {
        let (host, _) = rest
            .split_once('/')
            .ok_or_else(|| "Use a file URI with an absolute local path.".to_owned())?;
        if !host.eq_ignore_ascii_case("localhost") {
            return Err("Network file locations are not supported.".to_owned());
        }
        &rest[host.len()..]
    };
    let decoded = percent_decode(encoded_path)?;
    #[cfg(windows)]
    let decoded = {
        let bytes = decoded.as_bytes();
        if bytes.len() >= 4
            && bytes[0] == b'/'
            && bytes[1].is_ascii_alphabetic()
            && bytes[2] == b':'
            && matches!(bytes[3], b'/' | b'\\')
        {
            decoded[1..].to_owned()
        } else {
            decoded
        }
    };
    let path = PathBuf::from(decoded);
    validate_local_path(&path)?;
    if !path.is_absolute() {
        return Err("A file URI must identify an absolute local path.".to_owned());
    }
    Ok(path)
}

fn validate_local_path(path: &Path) -> Result<(), String> {
    let value = path.as_os_str().to_string_lossy();
    let bytes = value.as_bytes();
    if bytes.contains(&0) {
        return Err("File paths cannot contain NUL characters.".to_owned());
    }
    if bytes.len() >= 2 && is_separator(bytes[0]) && is_separator(bytes[1]) {
        return Err("Network and device paths are not supported.".to_owned());
    }
    if value.starts_with("\\??\\") || value.starts_with("/??/") {
        return Err("Device paths are not supported.".to_owned());
    }
    if let Some(colon) = bytes.iter().position(|byte| *byte == b':') {
        if colon != 1 || !bytes[0].is_ascii_alphabetic() {
            return Err(
                "Use a local path, file://, http://, https://, or about:home address.".to_owned(),
            );
        }
        if bytes.len() < 3 || !is_separator(bytes[2]) {
            return Err("Use an absolute drive path such as C:\\folder\\page.html.".to_owned());
        }
        #[cfg(not(windows))]
        return Err("Windows drive paths are unavailable on this platform.".to_owned());
    }
    #[cfg(windows)]
    validate_windows_components(path)?;
    Ok(())
}

fn is_separator(byte: u8) -> bool {
    matches!(byte, b'/' | b'\\')
}

fn canonical_file(path: &Path) -> Result<PathBuf, String> {
    validate_local_path(path)?;
    let canonical =
        fs::canonicalize(path).map_err(|error| format!("Cannot resolve file: {error}"))?;
    let canonical = normalize_canonical(canonical)?;
    validate_local_path(&canonical)?;
    if !fs::metadata(&canonical)
        .map_err(|error| format!("Cannot inspect file: {error}"))?
        .is_file()
    {
        return Err("Choose a regular local file.".to_owned());
    }
    Ok(canonical)
}

#[cfg(not(windows))]
fn normalize_canonical(path: PathBuf) -> Result<PathBuf, String> {
    Ok(path)
}

#[cfg(windows)]
fn normalize_canonical(path: PathBuf) -> Result<PathBuf, String> {
    use std::path::{Component, Prefix};
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err("The resolved file has no local drive.".to_owned());
    };
    let drive = match prefix.kind() {
        Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
        _ => return Err("The resolved file is a network or device path.".to_owned()),
    };
    // Rebuild from OsStr components, preserving Unicode and native UTF-16 data.
    // Reject names whose meaning changes when the internal \\?\ prefix is removed.
    validate_windows_components(&path)?;
    let mut normalized = PathBuf::from(format!("{}:\\", char::from(drive)));
    for component in components {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => normalized.push(name),
            _ => return Err("The resolved path contains unsupported components.".to_owned()),
        }
    }
    Ok(normalized)
}

#[cfg(windows)]
fn validate_windows_components(path: &Path) -> Result<(), String> {
    use std::path::{Component, Prefix};
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => match prefix.kind() {
                Prefix::Disk(_) | Prefix::VerbatimDisk(_) => {}
                _ => return Err("Network and device paths are not supported.".to_owned()),
            },
            Component::Normal(name) => {
                let name = name.to_string_lossy();
                if name.ends_with([' ', '.']) || name.contains(':') {
                    return Err(
                        "Device-style filenames and alternate data streams are not supported."
                            .to_owned(),
                    );
                }
                let base = name.split('.').next().unwrap_or("").to_ascii_uppercase();
                let reserved = matches!(
                    base.as_str(),
                    "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
                ) || ["COM", "LPT"].iter().any(|prefix| {
                    base.strip_prefix(*prefix).is_some_and(|suffix| {
                        matches!(
                            suffix,
                            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                        )
                    })
                });
                if reserved {
                    return Err("Windows device names cannot be opened as documents.".to_owned());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn percent_decode(input: &str) -> Result<String, String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = if bytes[index] == b'%' {
            let high = bytes.get(index + 1).and_then(|byte| hex_value(*byte));
            let low = bytes.get(index + 2).and_then(|byte| hex_value(*byte));
            let (Some(high), Some(low)) = (high, low) else {
                return Err("A percent escape must contain two hexadecimal digits.".to_owned());
            };
            index += 3;
            high * 16 + low
        } else {
            let byte = bytes[index];
            index += 1;
            byte
        };
        if byte == 0 {
            return Err("Locations cannot contain NUL characters.".to_owned());
        }
        decoded.push(byte);
    }
    String::from_utf8(decoded)
        .map_err(|_| "Percent escapes must encode valid UTF-8 text.".to_owned())
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn percent_encode(input: &str, path: bool) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(input.len());
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'.' | b'_' | b'~')
            || path && matches!(byte, b'/' | b':')
        {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Folder(PathBuf);
    #[test]
    fn selected_media_url_does_not_require_existence_or_grant_file_authority(){
        let folder=Folder::new();let page=folder.file("page.html",b"page");let base=explicit(&page).unwrap();
        let selected=selection_url("missing%20clip.ivf#frame",&base).unwrap();
        assert!(selected.ends_with("missing%20clip.ivf#frame"),"{selected}");
        assert!(resolve("missing%20clip.ivf#frame",&base).is_err());
        assert!(selection_url("../outside.ivf",&base).is_ok());assert!(resolve("../outside.ivf",&base).is_err());
        assert!(selection_url("clip.ivf?unsupported=query",&base).is_err());
        assert_eq!(selection_url("../b?q=1#x",&address("https://example.test/a/file",&Location::home()).unwrap()).unwrap(),"https://example.test/b?q=1#x");
    }

    #[test]
    fn address_search_intent_keeps_explicit_failures_local_and_preserves_files() {
        let folder=Folder::new();
        let page=folder.file("page.html",b"page");
        folder.file("existing note.html",b"local");folder.file("local.test",b"local");folder.file("report#2026.html",b"literal");
        let current=explicit(&page).unwrap();
        for input in ["cupcake recipes","youtube","cheetos","meow","crazy html","ponies","C++ & C#","café 🦀","100%","\"quoted ordinary phrase\"","hello.world phrase",".NET tutorial","site:rust-lang.org ownership","-filetype:pdf"] {
            assert_eq!(address_intent(input,&current).unwrap(),AddressIntent::Search,"{input}");
        }
        for input in ["https://example.test/a?q=x#f","HTTP://example.test/","example.test","localhost","127.0.0.1:8080/a","[::1]:80/","2001:db8::abcd","[::1] bad","machine:8080","about:home","#part","./missing.html","folder/missing.html","C:\\missing folder\\a.html","\\\\server\\share\\page.html","file:///C:/missing.html","existing note.html","\"existing note.html\"","local.test","https:/broken","http://[broken","ftp://example.test/","mailto:private@example.test","javascript:alert(1)","localhost:bad","example.test:99999","http://user@example.test/","\"C:\\missing folder\\a.html\""] {
            assert_eq!(address_intent(input,&current).unwrap(),AddressIntent::Navigate,"{input}");
        }
        for input in ["","   ","a\nb","\0"] {assert!(address_intent(input,&current).is_err(),"{input:?}");}
        assert!(address_intent(&"x".repeat(16385),&current).is_err());
        assert_eq!(address_intent("\"report#2026.html\"",&current).unwrap(),AddressIntent::Navigate);
        assert_eq!(address("\"report#2026.html\"",&current).unwrap().path.unwrap().file_name().unwrap(),"report#2026.html");
        assert!(resolve("file:///C:/page.html",&address("https://example.test/",&Location::home()).unwrap()).is_err());
    }

    impl Folder {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "chezcat-navigation-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn file(&self, relative: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, bytes).unwrap();
            path
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn percent_escapes_are_strict_utf8_and_plus_stays_literal() {
        assert_eq!(percent_decode("a+b%20c%E2%98%83").unwrap(), "a+b c☃");
        for invalid in ["%", "%1", "%x1", "%FF", "%C0%AF", "%00"] {
            assert!(percent_decode(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn displayed_locations_round_trip_unicode_and_reserved_filename_characters() {
        let folder = Folder::new();
        let path = folder.file("space # 100% café 😀.html", b"page");
        let mut location = explicit(&path).unwrap();
        location.fragment = "heading # 50% / α".to_owned();
        assert_eq!(
            address(&location.display(), &Location::home()).unwrap(),
            location
        );
        assert_eq!(
            address(&format!("\"{}\"", path.display()), &Location::home()).unwrap(),
            explicit(&path).unwrap()
        );
        let local_host = location
            .display()
            .replacen("file:///", "file://localhost/", 1);
        assert_eq!(address(&local_host, &Location::home()).unwrap(), location);
    }

    #[test]
    fn raw_addresses_keep_percent_signs_while_document_references_decode_them() {
        let folder = Folder::new();
        let start = explicit(&folder.file("index.html", b"index")).unwrap();
        let percent = explicit(&folder.file("100%20real.html", b"literal percent")).unwrap();
        let spaced = explicit(&folder.file("child page.html", b"space")).unwrap();
        assert_eq!(address("100%20real.html", &start).unwrap(), percent);
        assert_eq!(resolve("100%2520real.html", &start).unwrap(), percent);
        assert_eq!(resolve("child%20page.html", &start).unwrap(), spaced);
    }

    #[test]
    fn document_navigation_retains_its_root_and_rejects_sibling_prefixes() {
        let folder = Folder::new();
        let start = explicit(&folder.file("site/index.html", b"index")).unwrap();
        folder.file("site/nested/detail.html", b"detail");
        let outside = explicit(&folder.file("private.html", b"outside")).unwrap();
        let sibling = explicit(&folder.file("site-other/page.html", b"sibling")).unwrap();
        let nested = resolve("nested/detail.html", &start).unwrap();
        assert_eq!(nested.root, start.root);
        assert_eq!(address(&nested.display(), &nested).unwrap(), nested);
        assert_eq!(resolve("../index.html", &nested).unwrap(), start);
        assert!(resolve("%2e%2e/private.html", &start).is_err());
        assert!(resolve("../../private.html", &nested).is_err());
        assert!(resolve(&outside.display(), &start).is_err());
        assert!(resolve(&sibling.display(), &start).is_err());
        assert_eq!(address("../private.html", &start).unwrap(), outside);
    }

    #[test]
    fn local_fragments_work_but_queries_unsupported_schemes_and_network_files_do_not() {
        let folder = Folder::new();
        let start = explicit(&folder.file("index.html", b"index")).unwrap();
        let fragment = resolve("#chapter%20two", &start).unwrap();
        assert_eq!(fragment.path, start.path);
        assert_eq!(fragment.root, start.root);
        assert_eq!(fragment.fragment, "chapter two");
        assert_eq!(address("about:home#top", &start).unwrap().fragment, "top");
        for invalid in [
            "javascript:alert(1)",
            "data:text/html,hello",
            "file://server/share/page.html",
            "\\\\server\\share\\page.html",
            "/\\server/share/page.html",
            "\\\\?\\C:\\page.html",
            "index.html?x=1",
            "index.html#%00",
        ] {
            assert!(resolve(invalid, &start).is_err(), "{invalid}");
            assert!(address(invalid, &start).is_err(), "{invalid}");
        }
    }

    #[test]
    fn explicit_web_addresses_and_file_to_web_links_have_no_local_file_authority() {
        let folder = Folder::new();
        let start = explicit(&folder.file("index.html", b"index")).unwrap();
        let web = address("https://Example.COM/a?q=1#heading%20two", &start).unwrap();
        assert_eq!(web.display(), "https://example.com/a?q=1#heading%20two");
        assert!(web.path.is_none());
        assert!(web.root.is_none());
        assert_eq!(resolve(&web.display(), &start).unwrap(), web);
        assert_eq!(address(&web.display(), &Location::home()).unwrap(), web);
        assert_eq!(address(&start.display(), &web).unwrap(), start);
        assert_eq!(address("about:home", &web).unwrap(), Location::home());
        assert_eq!(
            address("example.com", &Location::home()).unwrap().display(),
            "https://example.com/"
        );
        assert!(address("\nhttp://example.com/", &start).is_err());
    }

    #[test]
    fn user_entered_hosts_default_to_https_but_document_references_do_not() {
        let home = Location::home();
        for (input, expected) in [
            ("google.com", "https://google.com/"),
            (" www.Google.com ", "https://www.google.com/"),
            (
                "google.com/search?q=own+browser#results",
                "https://google.com/search?q=own+browser#results",
            ),
            ("example.com:8443/a", "https://example.com:8443/a"),
            ("localhost", "https://localhost/"),
            ("localhost:8766", "https://localhost:8766/"),
            ("intranet:8443", "https://intranet:8443/"),
            ("127.0.0.1:8766/a", "https://127.0.0.1:8766/a"),
            ("[::1]:8766/a", "https://[::1]:8766/a"),
            ("http://google.com", "http://google.com/"),
        ] {
            let location = address(input, &home).unwrap();
            assert_eq!(location.display(), expected, "{input}");
            assert!(location.path.is_none() && location.root.is_none());
        }
        let current = address("http://example.test/folder/index.html", &home).unwrap();
        assert_eq!(
            resolve("google.com", &current).unwrap().display(),
            "http://example.test/folder/google.com"
        );
        assert_eq!(
            address("#section", &current).unwrap().display(),
            "http://example.test/folder/index.html#section"
        );
        for input in [
            "google.com\n",
            "\rgoogle.com",
            "google.com:0",
            "google.com:65536",
            "[::1",
            "user@google.com",
            "javascript:alert(1)",
            "data:text/html,hello",
            "https:google.com",
        ] {
            assert!(address(input, &home).is_err(), "{input}");
        }
    }

    #[test]
    fn existing_local_files_and_explicit_paths_keep_their_meaning() {
        let folder = Folder::new();
        let current = explicit(&folder.file("index.html", b"index")).unwrap();
        let domain_file = explicit(&folder.file("www.google.com", b"local file")).unwrap();
        let nested = explicit(&folder.file("my.folder/page.html", b"local page")).unwrap();
        assert_eq!(address("www.google.com", &current).unwrap(), domain_file);
        assert_eq!(address("my.folder/page.html", &current).unwrap(), nested);
        assert_eq!(address("./www.google.com", &current).unwrap(), domain_file);
        assert_eq!(
            address("\"www.google.com\"", &current).unwrap(),
            domain_file
        );
        assert_eq!(
            address("index.html#heading", &current).unwrap().fragment,
            "heading"
        );
        assert!(address("index.html?query=unsupported", &current).is_err());
        assert!(address("./missing.example", &current)
            .unwrap_err()
            .contains("file"));
        for local in [
            r"C:\\site\\page.html",
            "C:/site/page.html",
            r"\\\\server\\page.html",
            "//server/page.html",
            "../missing.example",
            "./missing.example",
        ] {
            assert!(!bare_web_address(local), "{local}");
        }
    }

    #[test]
    fn web_document_references_cannot_enter_local_or_internal_locations() {
        let web = address("http://example.com/a/page?old=1#old", &Location::home()).unwrap();
        let linked = resolve("../image.bmp?new=2#part", &web).unwrap();
        assert_eq!(linked.display(), "http://example.com/image.bmp?new=2#part");
        assert!(linked.path.is_none());
        assert!(linked.root.is_none());
        assert_eq!(resolve("#new", &web).unwrap().fragment, "new");
        assert_eq!(address("#next", &web).unwrap().fragment, "next");
        assert!(address("#next\r\n", &web).is_err());
        assert_eq!(
            resolve("/etc/passwd", &web).unwrap().url.unwrap().path,
            "/etc/passwd"
        );
        for invalid in [
            "file:///etc/passwd",
            "file://localhost/C:/secret",
            "about:home",
            "C:/secret",
            "C:\\secret",
            "\\\\server\\file",
            "javascript:alert(1)",
            "data:text/html,secret",
        ] {
            assert!(resolve(invalid, &web).is_err(), "{invalid}");
        }
    }

    #[test]
    fn bounded_reads_accept_the_exact_limit_and_reject_directories() {
        let folder = Folder::new();
        let path = folder.file("data.html", b"abc");
        assert_eq!(read_limited(&path, 3).unwrap(), b"abc");
        assert!(read_limited(&path, 2).is_err());
        assert!(read_limited(&folder.0, 100).is_err());
        assert!(explicit(&folder.0).is_err());
        assert!(read_limited(&folder.file("empty.html", b""), 0)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn stored_addresses_are_absolute_bounded_and_need_no_file_to_exist() {
        assert_eq!(
            stored_address("HTTPS://Example.COM:443/a/../b#c").unwrap(),
            "https://example.com/b#c"
        );
        assert_eq!(
            stored_address("ABOUT:HOME#café").unwrap(),
            "about:home#caf%C3%A9"
        );
        let folder = Folder::new();
        let missing = folder.0.join("missing # café.html");
        let location = Location {
            path: Some(missing.clone()),
            url: None,
            fragment: "part 2".into(),
            root: Some(folder.0.clone()),
        };
        let address = location.display();
        assert_eq!(stored_address(&address).unwrap(), address);
        assert!(!missing.exists());
        assert!(self::address(&address, &Location::home()).is_err());
        for invalid in [
            "google.com",
            "relative.html",
            "#part",
            "javascript:1",
            "data:text/html,x",
            "file://server/share/page",
            "file:///\\\\server/share/page",
            "https://example.com/\n",
            "https://name:password@example.com/",
        ] {
            assert!(stored_address(invalid).is_err(), "{invalid}");
        }
        assert!(stored_address(&format!("about:home#{}", "é".repeat(6000))).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn document_links_cannot_follow_a_symlink_outside_the_root() {
        let folder = Folder::new();
        let start = explicit(&folder.file("site/index.html", b"index")).unwrap();
        let outside = folder.file("private.html", b"private");
        std::os::unix::fs::symlink(outside, folder.0.join("site/escape.html")).unwrap();
        assert!(resolve("escape.html", &start).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn canonical_drive_prefixes_preserve_unicode_without_accepting_device_names() {
        assert_eq!(
            normalize_canonical(PathBuf::from(r"\\?\C:\測試\😀\page.html")).unwrap(),
            PathBuf::from(r"C:\測試\😀\page.html")
        );
        for invalid in [
            r"\\?\UNC\server\share\page.html",
            r"\\.\C:\page.html",
            r"\\?\C:\folder.\page.html",
            r"\\?\C:\page.html:stream",
            r"\\?\C:\CON.txt",
        ] {
            assert!(
                normalize_canonical(PathBuf::from(invalid)).is_err(),
                "{invalid}"
            );
        }
    }
}
