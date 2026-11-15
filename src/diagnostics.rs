//! Explicit local document checks through the production HTTP/TLS client.
//! No page code, subresources, scanner forms, or challenge solving runs here.

use crate::{http, network_trace, trust, url};
use std::io::Write;
use std::path::Path;
use std::sync::{atomic::AtomicBool, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const BODY_LIMIT: usize = 2 * 1024 * 1024;
const MAX_HOSTS: usize = 128;
const FIELD_LIMIT: usize = 4096;

/// Exact canonical hosts, never suffix patterns, URL prefixes, or DNS aliases.
/// An empty list preserves the existing unrestricted diagnostic redirect policy.
#[derive(Default)]
pub struct AllowedHosts(Vec<String>);

impl AllowedHosts {
    pub fn push(&mut self, value: &str) -> Result<(), String> {
        if self.0.len() >= MAX_HOSTS {
            return Err("At most 128 --diagnose-allow-host options are allowed.".into());
        }
        // Reuse the URL parser's DNS/IPv4/IPv6 grammar, without accepting a
        // scheme, port, path, credentials, whitespace, query, or fragment.
        let ipv6 = value.starts_with('[') && value.ends_with(']');
        if value.is_empty()
            || value.len() > 254
            || !value.is_ascii()
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(byte, b'-' | b'.')
                    || ipv6 && matches!(byte, b':' | b'[' | b']')
            })
        {
            return Err("--diagnose-allow-host requires an ASCII DNS host, IPv4 address, or bracketed IPv6 address; no URL or port.".into());
        }
        let mut address = String::new();
        address
            .try_reserve(value.len() + 8)
            .map_err(|_| "Cannot allocate a diagnostic host.")?;
        address.push_str("http://");
        address.push_str(value);
        address.push('/');
        let host = url::parse(&address)?.0.host;
        self.0
            .try_reserve(1)
            .map_err(|_| "Cannot allocate diagnostic hosts.")?;
        self.0.push(host);
        Ok(())
    }

    pub fn require_diagnose(&self, has_targets: bool) -> Result<(), String> {
        if !self.0.is_empty() && !has_targets {
            Err("--diagnose-allow-host is only available with --diagnose.".into())
        } else {
            Ok(())
        }
    }

    fn check(&self, target: &url::HttpUrl) -> Result<(), String> {
        if self.0.is_empty() || self.0.iter().any(|host| host == &target.host) {
            Ok(())
        } else {
            Err(format!(
                "Diagnostic host allowlist refused host: {}",
                target.host
            ))
        }
    }
}

pub fn write_report(
    path: &Path,
    targets: &[String],
    trust: &trust::Store,
    allowed: &AllowedHosts,
) -> Result<(), String> {
    if targets.is_empty() || targets.len() > 8 {
        return Err("Connection reports require between 1 and 8 --diagnose URLs.".into());
    }
    // Validate all input and open the report before any network activity.
    let targets = targets
        .iter()
        .map(|value| url::parse(value).map(|pair| pair.0))
        .collect::<Result<Vec<_>, _>>()?;
    let mut report = std::fs::File::create(path).map_err(|error| {
        format!(
            "Cannot create connection report {}: {error}",
            path.display()
        )
    })?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs();
    writeln!(
        report,
        "ChezCat {} connection report\nUTC time: {timestamp} seconds since 1970-01-01",
        env!("CARGO_PKG_VERSION")
    )
    .map_err(|error| error.to_string())?;
    field(
        &mut report,
        "Executable",
        &std::env::current_exe()
            .map_err(|error| error.to_string())?
            .display()
            .to_string(),
    )?;
    field(&mut report, "Trust file", trust.description())?;
    writeln!(report, "\nEach check is an ordinary document GET using our HTTP/TLS code.\nNo page scripts, subresources, form submissions, or challenge solving run.\nPage usability and fingerprint/port tests are not tested by receiving a document.\nMetadata fields are escaped and limited to {FIELD_LIMIT} UTF-8 bytes each.\nHost policy: {}", if allowed.0.is_empty() { "default unrestricted redirects" } else { "explicit exact-host allowlist; checked before DNS on every hop" })
        .map_err(|error| error.to_string())?;
    for host in &allowed.0 {
        field(&mut report, "Allowed host", host)?;
    }
    for target in targets {
        field(&mut report, "\nURL", &target.display())?;
        report.flush().map_err(|error| error.to_string())?;
        let started = Instant::now();
        let trace = network_trace::Trace::default();
        let attempted = Mutex::new(None);
        let guard = |next: &url::HttpUrl| {
            *attempted
                .lock()
                .map_err(|_| "Diagnostic request guard state failed.")? = Some(next.display());
            allowed.check(next)
        };
        let cancel = AtomicBool::new(false);
        let pool = http::Pool::new(&cancel, trust)
            .with_document_filter(&guard)
            .with_network_trace(trace.clone());
        match pool.fetch_document_limited(
            &target,
            &http::Request::Get,
            http::FetchMode::FullReload,
            BODY_LIMIT,
        ) {
            Ok(response) => {
                writeln!(report, "RESULT: document received\nHTTP status: {}\nDecoded body: {} bytes\nPage usability: not tested", response.status, response.body.len())
                    .map_err(|error| error.to_string())?;
                field(&mut report, "Final URL", &response.url.display())?;
                field(
                    &mut report,
                    "Protocol",
                    response.protocol.unwrap_or("HTTP / unencrypted"),
                )?;
                field(
                    &mut report,
                    "Content-Type",
                    response.header("content-type").unwrap_or("not supplied"),
                )?;
                field(
                    &mut report,
                    "Access-related HTTP status",
                    if matches!(response.status, 401 | 403 | 407 | 429 | 451) {
                        "received; this does not establish the page's reason or challenge state"
                    } else {
                        "none of 401, 403, 407, 429, 451"
                    },
                )?;
                let mut marker_found = false;
                for marker in ["captcha", "unusual traffic", "access denied"] {
                    if response
                        .body
                        .windows(marker.len())
                        .any(|part| part.eq_ignore_ascii_case(marker.as_bytes()))
                    {
                        field(&mut report, "Possible body challenge marker", marker)?;
                        marker_found = true;
                    }
                }
                if marker_found {
                    field(
                        &mut report,
                        "Body interpretation",
                        "text markers only; not proof of a challenge, denial, or successful access",
                    )?;
                } else {
                    field(&mut report, "Body interpretation", "no configured text marker found; this does not establish access or usability")?;
                }
                for event in response.events {
                    field(&mut report, "Activity", &event)?;
                }
            }
            Err(error) => {
                writeln!(report, "RESULT: failed").map_err(|error| error.to_string())?;
                let attempted = attempted
                    .lock()
                    .map_err(|_| "Diagnostic request guard state failed.")?;
                field(
                    &mut report,
                    "Failed URL",
                    attempted.as_deref().unwrap_or("not recorded"),
                )?;
                let snapshot = trace.snapshot();
                let row = snapshot.rows.last();
                field(
                    &mut report,
                    "Failure boundary",
                    if row.is_some_and(|row| row.outcome == network_trace::Outcome::Blocked) {
                        "document guard before DNS"
                    } else {
                        "not separately recorded; measured phase and error follow"
                    },
                )?;
                field(
                    &mut report,
                    "Last measured phase",
                    row.and_then(|row| row.phases.last())
                        .map_or("not recorded", |phase| phase.kind.label()),
                )?;
                field(&mut report, "ERROR", &error)?;
            }
        }
        write_trace(&mut report, &trace.snapshot())?;
        writeln!(report, "Elapsed: {} ms\n", started.elapsed().as_millis())
            .map_err(|error| error.to_string())?;
        report.flush().map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn write_trace(report: &mut impl Write, snapshot: &network_trace::Snapshot) -> Result<(), String> {
    for row in &snapshot.rows {
        writeln!(
            report,
            "Trace exchange: {}; outcome={}; status={}",
            row.id,
            row.outcome.label(),
            row.status
                .map_or_else(|| "not received".into(), |status| status.to_string())
        )
        .map_err(|error| error.to_string())?;
        field(report, "  URL", &row.url)?;
        field(report, "  Note", &row.note)?;
        for phase in &row.phases {
            writeln!(
                report,
                "  Measured phase: {}; start={} ms; end={}",
                phase.kind.label(),
                phase.start.as_millis(),
                phase.end.map_or_else(
                    || "not recorded".into(),
                    |end| format!("{} ms", end.as_millis())
                )
            )
            .map_err(|error| error.to_string())?;
        }
    }
    writeln!(report, "Trace omitted exchanges: {}", snapshot.omitted)
        .map_err(|error| error.to_string())
}

fn field(report: &mut impl Write, label: &str, value: &str) -> Result<(), String> {
    writeln!(report, "{label}: {}", escaped(value)?).map_err(|error| error.to_string())
}

fn escaped(value: &str) -> Result<String, String> {
    const TRUNCATED: &str = "...[truncated]";
    let mut result = String::new();
    for character in value.chars() {
        let mut bytes = [0u8; 12];
        let piece = if character.is_control()
            || character == '\\'
            || matches!(character, '\u{2028}' | '\u{2029}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            let mut length = 0;
            for escaped in character.escape_default() {
                bytes[length] = escaped as u8;
                length += 1;
            }
            std::str::from_utf8(&bytes[..length])
                .map_err(|_| "Cannot escape diagnostic metadata.")?
        } else {
            character.encode_utf8(&mut bytes)
        };
        if result.len() + piece.len() > FIELD_LIMIT - TRUNCATED.len() {
            result
                .try_reserve(TRUNCATED.len())
                .map_err(|_| "Cannot allocate diagnostic metadata.")?;
            result.push_str(TRUNCATED);
            break;
        }
        result
            .try_reserve(piece.len())
            .map_err(|_| "Cannot allocate diagnostic metadata.")?;
        result.push_str(piece);
    }
    Ok(result)
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
