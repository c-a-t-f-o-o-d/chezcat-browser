//! Explicit browser-owned trust data. No OS root-store lookup or downloads.
use std::collections::HashSet;
use std::io::Read;
use std::path::Path;

use crate::{crypto_hash, x509};

pub struct Store {
    certificates: Vec<Vec<u8>>,
    description: String,
    failure: Option<String>,
}

impl Store {
    pub fn unavailable(message: String) -> Self {
        Self {
            certificates: Vec::new(),
            description: "No usable trust file".into(),
            failure: Some(message),
        }
    }

    pub fn unconfigured() -> Self {
        Self::unavailable("HTTPS requires an explicit local PEM trust file.".into())
    }

    pub fn from_file(path: &Path) -> Result<Self, String> {
        const MAX_FILE: u64 = 2 * 1024 * 1024;
        let file = std::fs::File::open(path)
            .map_err(|e| format!("Cannot open trust file {}: {e}", path.display()))?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("The trust-file path must name a regular PEM file.".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_FILE + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Cannot read trust file: {e}"))?;
        if bytes.len() as u64 > MAX_FILE {
            return Err("Trust files are limited to 2 MiB.".into());
        }
        let certificates = x509::parse_pem_certificates(&bytes)?;
        if certificates.is_empty() || certificates.len() > x509::MAX_ROOTS {
            return Err(format!("A trust file must contain between 1 and {} certificates.", x509::MAX_ROOTS));
        }
        let mut seen = HashSet::new();
        let certificates: Vec<_> = certificates
            .into_iter()
            .filter(|der| seen.insert(crypto_hash::sha256(der)))
            .collect();
        Ok(Self {
            description: format!(
                "{} ({} certificate(s), PEM SHA-256 {})",
                path.display(),
                certificates.len(),
                hex(&crypto_hash::sha256(&bytes))
            ),
            certificates,
            failure: None,
        })
    }

    pub fn roots(&self) -> Result<&[Vec<u8>], String> {
        if let Some(failure) = &self.failure {
            return Err(format!("HTTPS trust configuration: {failure}"));
        }
        Ok(&self.certificates)
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        result.push(DIGITS[(byte >> 4) as usize] as char);
        result.push(DIGITS[(byte & 15) as usize] as char);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
    struct OwnedFile(PathBuf);
    impl OwnedFile {
        fn new(bytes: &[u8]) -> Self {
            let path = std::env::temp_dir().join(format!("chezcat-custom-trust-{}-{}-{}.pem",
                std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed)));
            let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
            file.write_all(bytes).unwrap();
            Self(path)
        }
    }
    impl Drop for OwnedFile { fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); } }

    #[test]
    fn custom_trust_accepts_more_than_128_raw_pem_roots_and_verifies_real_path() {
        // Real original signed certificates, not synthetic DER or signed-root claims.
        // This proves raw PEM capacity and deduplication, not 129 unique authorities.
        let original = include_bytes!("../tools/https-demo/ca.pem");
        let bytes = original.repeat(129);
        assert!(bytes.len() < 2 * 1024 * 1024);
        assert_eq!(x509::parse_pem_certificates(&bytes).unwrap().len(), 129);
        let file = OwnedFile::new(&bytes);
        let store = Store::from_file(&file.0).unwrap();
        let roots = store.roots().unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots, x509::parse_pem_certificates(original).unwrap());
        assert!(store.description().contains("1 certificate(s)"));
        assert!(store.description().contains(&hex(&crypto_hash::sha256(&bytes))));
        let chain = x509::parse_pem_certificates(include_bytes!("../tools/https-demo/server-chain.pem")).unwrap();
        let now = UNIX_EPOCH + Duration::from_secs(1_794_182_400);
        x509::verify_server(&chain, roots, "localhost", now).unwrap();
        assert!(x509::verify_server(&chain, roots, "wrong.invalid", now).is_err());

        // Exercise the verifier's increased raw root allowance as well. It still
        // selects and cryptographically validates the actual demo authority.
        let repeated = vec![roots[0].clone(); 129];
        x509::verify_server(&chain, &repeated, "localhost", now).unwrap();

        // All custom roots are parsed before any path succeeds: an unsupported
        // extra DER object is not silently ignored behind the valid first root.
        let mut unsupported = roots.to_vec();
        unsupported.push(vec![0x30, 0]);
        let error = x509::verify_server(&chain, &unsupported, "localhost", now).unwrap_err();
        assert!(error.contains("Trusted root 2:"), "{error}");

        let malformed = OwnedFile::new(b"-----BEGIN CERTIFICATE-----\nnot base64%\n-----END CERTIFICATE-----\n");
        assert!(Store::from_file(&malformed.0).is_err());
    }
}
