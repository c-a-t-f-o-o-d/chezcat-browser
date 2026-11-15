//! Offline acceptance of the exact shipped terms. A local receipt is evidence,
//! not DRM, a signature, a server account or permission to redistribute.
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const LICENSE_TEXT: &str = include_str!("../LICENSE");
pub const LICENSE_VERSION: &str = "1.1";
pub const LICENSE_SHA256: &str = "34003a38eeb1db77472c307e23b5a5540c7625c50d9f0ea0dc7bacf6e5b9b749";
const MAX_RECEIPT: u64 = 2048;
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope { Personal, Organization }
impl Scope {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text { "personal" => Ok(Self::Personal), "organization" => Ok(Self::Organization),
            _ => Err("License scope must be personal or organization.".into()) }
    }
    pub fn text(self) -> &'static str { match self { Self::Personal => "personal", Self::Organization => "organization" } }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State { Missing, NeedsAcceptance, Accepted(Scope) }

pub fn verify_terms() -> Result<(), String> {
    let actual = crate::crypto_hash::sha256(LICENSE_TEXT.as_bytes());
    let mut hex = String::with_capacity(64);
    use std::fmt::Write as _;
    for byte in actual { write!(hex, "{byte:02x}").unwrap(); }
    if hex != LICENSE_SHA256 { return Err("Embedded license text and approved digest differ.".into()); }
    Ok(())
}

fn reject_link(metadata: &fs::Metadata) -> Result<(), String> {
    let mut linked = metadata.file_type().is_symlink();
    #[cfg(windows)] { use std::os::windows::fs::MetadataExt; linked |= metadata.file_attributes() & 0x400 != 0; }
    if linked { Err("License receipt storage must not use links or reparse points.".into()) } else { Ok(()) }
}

// Canonicalize the existing prefix while rejecting '..', links and network/ADS
// paths. The remaining suffix need not exist before an affirmative acceptance.
fn projected(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() { return Err("License receipt directory must be absolute.".into()); }
    let mut prefix = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir | Component::CurDir => return Err("License receipt directory must be normalized.".into()),
            Component::Normal(name) if name.to_string_lossy().contains(':') => return Err("Invalid license receipt storage component.".into()),
            #[cfg(windows)]
            Component::Prefix(value) if matches!(value.kind(), std::path::Prefix::UNC(..) | std::path::Prefix::VerbatimUNC(..) | std::path::Prefix::DeviceNS(..)) => return Err("License receipt storage must be local.".into()),
            _ => prefix.push(component.as_os_str()),
        }
        if matches!(component, Component::Prefix(_)) { continue; }
        match fs::symlink_metadata(&prefix) {
            Ok(metadata) => reject_link(&metadata)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(_) => return Err("Cannot inspect license receipt storage.".into()),
        }
    }
    let mut existing = prefix.as_path();
    let mut suffix = Vec::new();
    while !existing.exists() {
        suffix.push(existing.file_name().ok_or("Invalid license receipt storage.")?.to_os_string());
        existing = existing.parent().ok_or("Invalid license receipt storage.")?;
    }
    let mut result = existing.canonicalize().map_err(|_| "Cannot resolve license receipt storage.")?;
    for name in suffix.into_iter().rev() { result.push(name); }
    Ok(result)
}
fn within(path: &Path, root: &Path) -> bool {
    #[cfg(windows)] { PathBuf::from(path.to_string_lossy().to_uppercase()).starts_with(PathBuf::from(root.to_string_lossy().to_uppercase())) }
    #[cfg(not(windows))] { path.starts_with(root) }
}
pub fn directory(explicit: Option<PathBuf>, profile: Option<&Path>) -> Result<PathBuf, String> {
    let requested = explicit.or_else(|| std::env::var_os("CHEZCAT_LICENSE_RECEIPT_DIR").map(PathBuf::from));
    let requested = match requested {
        Some(path) => path,
        None => {
            #[cfg(windows)] { PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("Local license storage is unavailable; choose an external --license-receipt-dir.")?).join("ChezCat/License") }
            #[cfg(not(windows))] {
                if let Some(base) = std::env::var_os("XDG_STATE_HOME") { PathBuf::from(base).join("chezcat/license") }
                else { PathBuf::from(std::env::var_os("HOME").ok_or("Local license storage is unavailable.")?).join(".local/state/chezcat/license") }
            }
        }
    };
    let selected = projected(&requested)?;
    let cwd = std::env::current_dir().map_err(|_| "Cannot establish receipt trust boundaries.")?.canonicalize().map_err(|_| "Cannot establish receipt trust boundaries.")?;
    let exe = std::env::current_exe().map_err(|_| "Cannot establish receipt trust boundaries.")?;
    let exe_dir = exe.parent().ok_or("Cannot establish receipt trust boundaries.")?.canonicalize().map_err(|_| "Cannot establish receipt trust boundaries.")?;
    if within(&selected, &exe_dir) {
        return Err("License receipts must be outside the application folder.".into());
    }
    // A normal shell may start in the user's home: its descendants include the
    // trusted per-user store. Reject source roots, not every arbitrary cwd.
    // Walk both origins so a target/debug executable launched elsewhere cannot
    // trust a copied receipt anywhere inside its source checkout.
    for origin in [&cwd, &exe_dir] {
        for ancestor in origin.ancestors() {
            if (ancestor.join(".git").exists()
                || (ancestor.join("Cargo.toml").is_file() && ancestor.join("src/main.rs").is_file()))
                && within(&selected, ancestor) {
                return Err("License receipts must be outside source checkouts.".into());
            }
        }
    }
    #[cfg(windows)]
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        let default_profile = PathBuf::from(base).join("OwnBrowser");
        if default_profile.is_absolute() && within(&selected, &projected(&default_profile)?) {
            return Err("License receipts must be outside the default application profile.".into());
        }
    }
    if let Some(profile) = profile {
        let profile = if profile.is_absolute() { profile.to_path_buf() } else { cwd.join(profile) };
        if within(&selected, &projected(&profile)?) { return Err("License receipts must be outside the application profile.".into()); }
    }
    Ok(selected)
}

struct Parser<'a> { bytes: &'a [u8], at: usize }
impl<'a> Parser<'a> {
    fn space(&mut self) { while self.bytes.get(self.at).is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n')) { self.at += 1; } }
    fn take(&mut self, byte: u8) -> Result<(), ()> { self.space(); if self.bytes.get(self.at) != Some(&byte) { return Err(()); } self.at += 1; Ok(()) }
    fn string(&mut self) -> Result<&'a str, ()> {
        self.take(b'"')?; let start = self.at;
        while let Some(&byte) = self.bytes.get(self.at) {
            if byte == b'"' { let text = std::str::from_utf8(&self.bytes[start..self.at]).map_err(|_| ())?; self.at += 1; return Ok(text); }
            if !(0x20..=0x7e).contains(&byte) || byte == b'\\' { return Err(()); }
            self.at += 1;
        }
        Err(())
    }
    fn number(&mut self) -> Result<u64, ()> {
        self.space(); let start = self.at;
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) { self.at += 1; }
        let bytes = &self.bytes[start..self.at];
        if bytes.is_empty() || (bytes.len() > 1 && bytes[0] == b'0') { return Err(()); }
        std::str::from_utf8(bytes).map_err(|_| ())?.parse().map_err(|_| ())
    }
}
fn decode(bytes: &[u8]) -> Result<Scope, ()> {
    let mut parser = Parser { bytes, at: 0 }; parser.take(b'{')?;
    let mut fields = 0u8; let mut scope = None;
    for index in 0..5 {
        if index != 0 { parser.take(b',')?; }
        let name = parser.string()?; parser.take(b':')?;
        let bit = match name {
            "schema_version" => { if parser.number()? != 1 { return Err(()); } 1 },
            "license_version" => { if parser.string()? != LICENSE_VERSION { return Err(()); } 2 },
            "license_sha256" => { if parser.string()? != LICENSE_SHA256 { return Err(()); } 4 },
            "accepted_scope" => { scope = Some(Scope::parse(parser.string()?).map_err(|_| ())?); 8 },
            "accepted_at_utc" => { if parser.number()? > 253402300799 { return Err(()); } 16 },
            _ => return Err(()),
        };
        if fields & bit != 0 { return Err(()); } fields |= bit;
    }
    parser.take(b'}')?; parser.space();
    if fields != 31 || parser.at != bytes.len() { return Err(()); }
    scope.ok_or(())
}
pub fn check(directory: &Path) -> Result<State, String> {
    verify_terms()?;
    let path = directory.join("receipt.json");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(State::Missing),
        Err(_) => return Err("Cannot read license acceptance receipt.".into()),
    };
    reject_link(&metadata)?;
    if !metadata.is_file() { return Err("License receipt must be a regular local file.".into()); }
    if metadata.len() > MAX_RECEIPT { return Ok(State::NeedsAcceptance); }
    let mut bytes = Vec::new();
    File::open(path).and_then(|f| f.take(MAX_RECEIPT + 1).read_to_end(&mut bytes)).map_err(|_| "Cannot read license acceptance receipt.")?;
    if bytes.len() as u64 > MAX_RECEIPT { return Ok(State::NeedsAcceptance); }
    Ok(decode(&bytes).map(State::Accepted).unwrap_or(State::NeedsAcceptance))
}
pub fn accept(directory: &Path, digest: &str, scope: Scope) -> Result<(), String> {
    verify_terms()?;
    if digest != LICENSE_SHA256 { return Err("Acceptance requires the exact full shipped LICENSE SHA-256.".into()); }
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| "UTC clock is unavailable for acceptance.")?.as_secs();
    if seconds > 253402300799 { return Err("UTC clock is outside the receipt format.".into()); }
    let bytes = format!("{{\"schema_version\":1,\"license_version\":\"{LICENSE_VERSION}\",\"license_sha256\":\"{LICENSE_SHA256}\",\"accepted_scope\":\"{}\",\"accepted_at_utc\":{seconds}}}\n", scope.text());
    let mut builder = fs::DirBuilder::new(); builder.recursive(true);
    #[cfg(unix)] { use std::os::unix::fs::DirBuilderExt; builder.mode(0o700); }
    builder.create(directory).map_err(|_| "Cannot create local license receipt storage.")?;
    projected(directory)?;
    let target = directory.join("receipt.json");
    if let Ok(metadata) = fs::symlink_metadata(&target) {
        reject_link(&metadata)?;
        if !metadata.is_file() || metadata.permissions().readonly() { return Err("License receipt is not writable; browsing has not started.".into()); }
    }
    let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
    let temporary = directory.join(format!(".receipt-{}-{sequence}.tmp", std::process::id()));
    struct Pending(PathBuf); impl Drop for Pending { fn drop(&mut self) { let _ = fs::remove_file(&self.0); } }

    let mut options = OpenOptions::new(); options.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let mut file = options.open(&temporary).map_err(|_| "Cannot stage local license receipt.")?;
    let pending = Pending(temporary);
    file.write_all(bytes.as_bytes()).and_then(|()| file.sync_all()).map_err(|_| "Cannot finish local license receipt.")?;
    drop(file);
    fs::rename(&pending.0, &target).map_err(|_| "Cannot atomically replace local license receipt.")?;
    Ok(())
}
pub fn require(directory: &Path) -> Result<(), String> {
    match check(directory)? { State::Accepted(_) => Ok(()), _ => Err(format!("Explicit CSAPUL {LICENSE_VERSION} acceptance is required before builds or browsing. View LICENSE, then use --accept-license {LICENSE_SHA256} --license-scope personal|organization.")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn receipt(scope: &str) -> Vec<u8> { format!("{{\"schema_version\":1,\"license_version\":\"1.1\",\"license_sha256\":\"{LICENSE_SHA256}\",\"accepted_scope\":\"{scope}\",\"accepted_at_utc\":100}}\n").into_bytes() }
    #[test] fn license_acceptance_embedded_terms_match_approved_digest() { verify_terms().unwrap(); }
    #[test] fn license_acceptance_strict_receipt_rejects_stale_corrupt_duplicate_and_types() {
        let valid = receipt("personal"); assert_eq!(decode(&valid), Ok(Scope::Personal));
        for bad in [String::from_utf8(valid.clone()).unwrap().replace("1.1", "1.0"), String::from_utf8(valid.clone()).unwrap().replace("\"schema_version\":1", "\"schema_version\":\"1\""), String::from_utf8(valid.clone()).unwrap().replace("\"accepted_scope\":\"personal\"", "\"license_version\":\"1.1\""), String::from_utf8(valid.clone()).unwrap().replace("100}", "-1}"), String::from_utf8(valid.clone()).unwrap().replace("100}", "01}"), String::from_utf8(valid.clone()).unwrap().replace("100}", "1e2}"), String::from_utf8(valid.clone()).unwrap().replace("100}", "100,\"extra\":0}")] { assert!(decode(bad.as_bytes()).is_err()); }
        assert!(decode(&receipt("unknown")).is_err());
    }
    #[test] fn license_acceptance_checkout_and_relative_receipts_are_untrusted() {
        assert!(directory(Some(PathBuf::from("relative")),None).is_err());
        assert!(directory(Some(std::env::current_dir().unwrap().join("copied-receipt")),None).is_err());
    }
    #[test] fn license_acceptance_atomic_receipts_reuse_and_preserve_wrong_digest() {
        let dir = std::env::temp_dir().join(format!("chezcat-consent-{}-{}",std::process::id(),NEXT_TEMPORARY.fetch_add(1,Ordering::Relaxed)));
        fs::create_dir(&dir).unwrap();
        let dir = directory(Some(dir),None).unwrap();
        assert_eq!(check(&dir).unwrap(),State::Missing);
        accept(&dir,LICENSE_SHA256,Scope::Organization).unwrap();
        assert_eq!(check(&dir).unwrap(),State::Accepted(Scope::Organization));
        let before=fs::read(dir.join("receipt.json")).unwrap();
        assert!(accept(&dir,"incorrect",Scope::Personal).is_err());
        assert_eq!(fs::read(dir.join("receipt.json")).unwrap(),before);
        accept(&dir,LICENSE_SHA256,Scope::Personal).unwrap();
        assert_eq!(check(&dir).unwrap(),State::Accepted(Scope::Personal));
        fs::write(dir.join("receipt.json"),b"{}").unwrap(); assert_eq!(check(&dir).unwrap(),State::NeedsAcceptance);
        fs::remove_dir_all(dir).unwrap();
    }
}
