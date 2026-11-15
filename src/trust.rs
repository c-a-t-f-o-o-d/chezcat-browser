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
        if certificates.is_empty() || certificates.len() > 128 {
            return Err("A trust file must contain between 1 and 128 certificates.".into());
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
