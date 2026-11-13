//! Explicit, local connection reports using the production HTTP/TLS client.
//! A diagnostic fetches one document with bounded redirects. It executes no
//! page code, loads no subresources and never submits a scanner's POST forms.

use crate::{http, trust, url};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub fn write_report(path: &Path, targets: &[String], trust: &trust::Store) -> Result<(), String> {
    if targets.is_empty() || targets.len() > 8 {
        return Err("Connection reports require between 1 and 8 --diagnose URLs.".into());
    }
    // Validate every target and open the report before any network activity.
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
    writeln!(report, "ChezCat 0.54 connection report\nUTC time: {timestamp} seconds since 1970-01-01\nExecutable: {}\nTrust file: {}\n\nEach check is an ordinary document GET using our HTTP/TLS code.\nNo page scripts, subresources or form submissions run.\nA received HTTP page does not mean a fingerprint or port test passed.\n", std::env::current_exe().map_err(|error| error.to_string())?.display(), trust.description())
        .map_err(|error| error.to_string())?;
    for target in targets {
        writeln!(report, "URL: {}", target.display()).map_err(|error| error.to_string())?;
        report.flush().map_err(|error| error.to_string())?;
        let started = Instant::now();
        match http::fetch(
            &target,
            2 * 1024 * 1024,
            None,
            &AtomicBool::new(false),
            trust,
            http::FetchMode::FullReload,
        ) {
            Ok(response) => {
                writeln!(report, "RESULT: document received\nFinal URL: {}\nHTTP status: {}\nProtocol: {}\nDecoded body: {} bytes\nContent-Type: {}", response.url.display(), response.status, response.protocol.unwrap_or("HTTP / unencrypted"), response.body.len(), response.header("content-type").unwrap_or("not supplied"))
                    .map_err(|error| error.to_string())?;
                for event in response.events {
                    writeln!(report, "  {event}").map_err(|error| error.to_string())?;
                }
            }
            Err(error) => {
                writeln!(report, "RESULT: failed\nERROR: {error}")
                    .map_err(|error| error.to_string())?;
            }
        }
        writeln!(report, "Elapsed: {} ms\n", started.elapsed().as_millis())
            .map_err(|error| error.to_string())?;
        report.flush().map_err(|error| error.to_string())?;
    }
    Ok(())
}
