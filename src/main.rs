#![cfg_attr(all(target_os = "windows", not(test)), windows_subsystem = "windows")]

// The Windows message pump owns the interactive methods; headless builds don't.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod app;
mod audio;
mod audio_output;
mod blocking;
mod build_identity;
mod canvas_2d_bitmap;
mod canvas_color;
mod compression;
mod cookies;
mod crypto_aead;
mod crypto_aes_gcm;
mod crypto_ec;
mod crypto_hash;
mod crypto_rsa;
mod crypto_sha512;
mod crypto_x25519;
mod csp;
mod css;
mod diagnostics;
mod downloads;
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod editor;
mod embedded_video;
mod font;
mod fonts;
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod forms;
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod history;
mod html;
mod html_canvas;
mod http;
mod image;
mod image_data;
mod image_selection;
mod javascript;
mod jpeg;
mod layout;
mod license_acceptance;
mod loader;
mod media;
mod media_tasks;
mod matroska;
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod navigation;
mod network_trace;
mod network_media;
mod paint;
#[cfg(target_os = "windows")]
mod platform;
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod profile;
mod public_suffix;
mod random;
mod script_geometry;
mod scripting;
mod search;
mod text;
mod tls;
mod tls_crypto;
mod trust;
mod ui;
mod url;
mod x509;

use std::io::Write;
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        #[cfg(target_os = "windows")]
        {
            // Automated headless checks must get an exit code, not a modal box.
            if std::env::args_os().any(|arg| {
                arg == "--license" || arg == "--accept-license" || arg == "--license-ui-script" || arg == "--help" || arg == "-h" || arg == "--render"
                    || arg == "--diagnose"
                    || arg == "--diagnose-allow-host"
                    || arg == "--activity-report"
                    || arg == "--save-source"
                    || arg == "--dev-input-script"
            }) {
                eprintln!("{}", error);
            } else {
                platform::show_error(&error);
            }
        }
        #[cfg(not(target_os = "windows"))]
        eprintln!("{}", error);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut license_digest: Option<String> = None;
    let mut license_scope: Option<license_acceptance::Scope> = None;
    let mut license_directory: Option<PathBuf> = None;
    let mut license_ui_script: Option<PathBuf> = None;
    let mut license_ui_report: Option<PathBuf> = None;
    let mut render: Option<PathBuf> = None;
    let mut address: Option<std::ffi::OsString> = None;
    let mut width = 1080;
    let mut height = 760;
    let mut scroll = 0;
    let mut zoom: Option<i32> = None;
    let mut source_view = false;
    let mut resource_view = false;
    let mut inspector_view: Option<String> = None;
    let mut inspector_window = false;
    let mut browser_view: Option<String> = None;
    let mut find = None;
    let mut trust_file: Option<PathBuf> = None;
    let mut diagnostic_urls = Vec::new();
    let mut diagnostic_hosts = diagnostics::AllowedHosts::default();
    let mut diagnostic_report: Option<PathBuf> = None;
    let mut activity_report: Option<PathBuf> = None;
    let mut save_source: Option<PathBuf> = None;
    let mut scripts_enabled: Option<bool> = None;
    let mut images_enabled: Option<bool> = None;
    let mut profile_directory: Option<PathBuf> = None;
    let mut no_profile = false;
    let mut wait_ms = 0;
    let mut input_script: Option<PathBuf> = None;
    let mut input_output: Option<PathBuf> = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--license") => { print!("{}", license_acceptance::LICENSE_TEXT); return Ok(()); }
            Some("--accept-license") => {
                if license_digest.is_some() { return Err("Pass --accept-license once.".into()); }
                license_digest = Some(args.next().and_then(|v| v.into_string().ok()).ok_or("--accept-license requires the full shipped SHA-256")?);
            }
            Some("--license-scope") => {
                if license_scope.is_some() { return Err("Pass --license-scope once.".into()); }
                let value = args.next().and_then(|v| v.into_string().ok()).ok_or("--license-scope requires personal or organization")?;
                license_scope = Some(license_acceptance::Scope::parse(&value)?);
            }
            Some("--license-receipt-dir") => {
                if license_directory.is_some() { return Err("Pass --license-receipt-dir once.".into()); }
                license_directory = Some(args.next().ok_or("--license-receipt-dir requires an absolute external folder")?.into());
            }
            Some("--license-ui-script") => {
                if license_ui_script.is_some() { return Err("Pass --license-ui-script once.".into()); }
                license_ui_script = Some(args.next().ok_or("--license-ui-script requires an explicit local native-input script")?.into());
            }
            Some("--license-ui-report") => {
                if license_ui_report.is_some() { return Err("Pass --license-ui-report once.".into()); }
                license_ui_report = Some(args.next().ok_or("--license-ui-report requires a new local report")?.into());
            }
            Some("--dev-input-script") => {
                if input_script.is_some() { return Err("Pass --dev-input-script once.".into()); }
                input_script = Some(args.next().ok_or("--dev-input-script requires a local script path")?.into());
            }
            Some("--dev-input-output") => {
                if input_output.is_some() { return Err("Pass --dev-input-output once.".into()); }
                input_output = Some(args.next().ok_or("--dev-input-output requires a new output directory")?.into());
            }
            Some("--render") => {
                render = Some(
                    args.next()
                        .ok_or("--render requires an output .ppm path")?
                        .into(),
                )
            }
            Some("--width") => width = number(args.next(), "--width")?,
            Some("--height") => height = number(args.next(), "--height")?,
            Some("--scroll") => scroll = number(args.next(), "--scroll")?,
            Some("--zoom") => zoom = Some(number(args.next(), "--zoom")?),
            Some("--source") => source_view = true,
            Some("--resources") => resource_view = true,
            Some("--inspector") => {
                let name = args
                    .next()
                    .ok_or("--inspector requires elements, sources, console or network")?
                    .into_string()
                    .map_err(|_| "Inspector tab must be UTF-8")?;
                if !matches!(
                    name.as_str(),
                    "elements" | "sources" | "console" | "network"
                ) {
                    return Err("--inspector requires elements, sources, console or network".into());
                }
                inspector_view = Some(name);
            }
            Some("--inspector-window") => inspector_window = true,
            Some("--browser-ui") => {
                let name = args
                    .next()
                    .and_then(|value| value.into_string().ok())
                    .ok_or("--browser-ui requires settings, appearance, bookmarks, downloads or download-settings")?;
                if !matches!(
                    name.as_str(),
                    "settings" | "appearance" | "bookmarks" | "downloads" | "download-settings"
                ) {
                    return Err("--browser-ui requires settings, appearance, bookmarks, downloads or download-settings".into());
                }
                browser_view = Some(name);
            }
            Some("--no-javascript") => scripts_enabled = Some(false),
            Some("--javascript") => scripts_enabled = Some(true),
            Some("--no-images") => images_enabled = Some(false),
            Some("--images") => images_enabled = Some(true),
            Some("--no-profile") => no_profile = true,
            Some("--profile-dir") => {
                if profile_directory.is_some() {
                    return Err("Pass --profile-dir at most once.".into());
                }
                profile_directory = Some(
                    args.next()
                        .ok_or("--profile-dir requires a folder path")?
                        .into(),
                );
            }
            Some("--wait-ms") => {
                wait_ms = args
                    .next()
                    .and_then(|value| value.to_str().and_then(|text| text.parse::<u32>().ok()))
                    .filter(|value| *value <= 10000)
                    .ok_or("--wait-ms requires an integer between 0 and 10000 milliseconds.")?;
            }
            Some("--diagnose") => {
                if diagnostic_urls.len() >= 8 {
                    return Err("At most 8 --diagnose URLs are allowed.".into());
                }
                diagnostic_urls.push(
                    args.next()
                        .ok_or("--diagnose requires an explicit HTTP(S) URL")?
                        .into_string()
                        .map_err(|_| "Diagnostic URLs must be UTF-8.")?,
                );
            }
            Some("--diagnose-allow-host") => {
                let host = args.next()
                    .ok_or("--diagnose-allow-host requires an ASCII host, not a URL")?
                    .into_string().map_err(|_| "Diagnostic hosts must be ASCII.")?;
                diagnostic_hosts.push(&host)?;
            }
            Some("--report") => {
                if diagnostic_report.is_some() {
                    return Err("Pass --report at most once.".into());
                }
                diagnostic_report = Some(
                    args.next()
                        .ok_or("--report requires a text file path")?
                        .into(),
                );
            }
            Some("--activity-report") => {
                if activity_report.is_some() {
                    return Err("Pass --activity-report at most once.".into());
                }
                activity_report = Some(
                    args.next()
                        .ok_or("--activity-report requires a text file path")?
                        .into(),
                );
            }
            Some("--save-source") => {
                if save_source.is_some() {
                    return Err("Pass --save-source at most once.".into());
                }
                save_source = Some(
                    args.next()
                        .ok_or("--save-source requires an HTML file path")?
                        .into(),
                );
            }
            Some("--trust-file") => {
                if trust_file.is_some() {
                    return Err("Pass --trust-file at most once.".into());
                }
                trust_file = Some(
                    args.next()
                        .ok_or("--trust-file requires a PEM file path")?
                        .into(),
                );
            }
            Some("--find") => {
                find = Some(
                    args.next()
                        .ok_or("--find requires text")?
                        .to_string_lossy()
                        .to_string(),
                )
            }
            Some("--help") | Some("-h") => {
                let help = concat!("ChezCat ", env!("CARGO_PKG_VERSION"), r#"

Open local HTML, UTF-8 text, supported images or a website address. Bare hosts such as google.com default to HTTPS; explicit http:// remains supported.
Ctrl+O: file picker; Ctrl+L: address; Esc: stop loading.
F5 / Ctrl+R: reload; Ctrl+F5 / Ctrl+Shift+R: full reload.
Ctrl+D: add bookmark; Ctrl+B: bookmarks; Ctrl+H: history; Ctrl+comma: settings.
Ctrl+S: download page file; Alt+click a link: download its target. The download icon opens recent downloads.
Ctrl+F: find; Ctrl+U: source; Ctrl+I: resources and script errors; Ctrl+J: save JavaScript preference and reload.
Alt+Left/Right or mouse side buttons: history; Ctrl +/-: zoom; Ctrl+0: saved default zoom.

F12 / Ctrl+Shift+I: live inspector; Elements, Sources, Console and Network; dock or detach.
Settings: JavaScript, images, request blocking, zoom, home, dark theme/page colors, bookmarks bar, custom link colors and Downloads folder. Use Menu > Settings, then Apply.
History: Ctrl+H > Settings enables recording, age cleanup (1/7/30/90 days or no age limit), and clear on normal window exit.
History starts disabled; clear time ranges and individual removal are available in the History panel.
Bookmarks and settings are saved in %LOCALAPPDATA%/OwnBrowser/profile.txt on Windows.
--profile-dir FOLDER selects a separate profile. --no-profile keeps changes in memory only.
--javascript / --no-javascript and --images / --no-images override saved values for this session.
--zoom 125 overrides the current page zoom. Apply in Settings deliberately saves the displayed choices.

License: --license displays the exact offline terms without consent or network access.
Before source/headless use: --accept-license FULL_SHA256 --license-scope personal|organization.
--license-receipt-dir ABSOLUTE_EXTERNAL_FOLDER selects local receipt storage, never implies consent.
Changed text requires renewed acceptance. Ordinary first-run startup offers offline Accept/Decline.

Headless rendering:
ChezCat.exe [file.html or https://host/path] --render output.ppm --width 1080 --height 760
Optional: --scroll 200 --zoom 125 --source --resources --find text --no-javascript --no-images --wait-ms 500
Inspector snapshots: --inspector elements|sources|console|network; add --inspector-window for its detached surface.
Browser panels: --browser-ui settings|appearance|bookmarks|downloads|download-settings.
Headless rendering ignores the personal profile unless --profile-dir is explicit. It never reads or changes saved browsing history.
Page activity: URL --activity-report activity.txt (runs the page and exits; add --render page.ppm for an image).
Activity reports use the same isolated-profile/history rules as headless rendering and include resource events and script errors.
Page source: URL --save-source page.html (saves the displayed document's decoded source as UTF-8, then exits).
Combine --save-source with --activity-report and --wait-ms to capture the final document after script navigation.
Source export uses the same profile isolation as rendering. It saves the original loaded HTML, before DOM changes; failed navigation does not create a source file.
--wait-ms adds timer settling time after page loading (0..10000; default 0).
Trust: --trust-file roots.pem (replaces trust/roots.pem beside the executable)
Connection report: --diagnose https://host/path --report connection.txt
Repeat --diagnose up to eight times, or run tools\demos\Check-Connections.cmd. Diagnostics do not load a profile.
Optional --diagnose-allow-host HOST (repeat up to 128 times, only with --diagnose) restricts every hop before DNS to exact ASCII DNS/IPv4 or bracketed IPv6 hosts; no URL, port, or wildcard. Without this option, the existing unrestricted redirect policy remains.
Reports record measured phases, attempted URLs and possible challenge text; receiving HTTP 200 does not establish page usability.

HTTP, the TLS 1.3/1.2 HTTPS subset, gzip/deflate decoding, images, layout, controls and JavaScript use our own code. Page fonts use our TrueType/WOFF1 reader and renderer; try pages/fonts.html. GET and URL-encoded POST forms work; refresh/history asks before resending POST. The bounded JavaScript subset includes typed arrays, DataView, Map/Set, text encoding, animation frames and native keyboard events. Session cookies stay in memory; host-only and validated Domain cookies use bundled public-suffix boundaries. Same-origin script navigation and external classic script insertion are supported within work, CSP and request-blocking limits. Fetch, XHR, persistent storage and HTML injection remain unavailable. Try the workshops and pages/browser-tests.html for current limits."#);
                println!("{}", help);
                return Ok(());
            }
            Some(flag) if flag.starts_with('-') => return Err(format!("Unknown option: {}", flag)),
            _ => {
                if address.is_some() {
                    return Err("Pass at most one HTML file or HTTP/HTTPS URL.".to_owned());
                }
                address = Some(arg);
            }
        }
    }
    if input_script.is_some() != input_output.is_some() {
        return Err("Use --dev-input-script with --dev-input-output.".into());
    }
    if input_script.is_some() && (render.is_some() || activity_report.is_some() || save_source.is_some()
        || source_view || resource_view || inspector_view.is_some() || browser_view.is_some() || find.is_some()) {
        return Err("Use development input mode separately from other output/view options.".into());
    }
    if wait_ms > 0 && render.is_none() && activity_report.is_none() && save_source.is_none() && input_script.is_none() {
        return Err(
            "--wait-ms requires --render, --activity-report or --save-source output.".into(),
        );
    }
    if inspector_window && (render.is_none() || inspector_view.is_none()) {
        return Err("--inspector-window requires --render and --inspector TAB.".into());
    }
    diagnostic_hosts.require_diagnose(!diagnostic_urls.is_empty())?;
    if no_profile && profile_directory.is_some() {
        return Err("Choose either --no-profile or --profile-dir, not both.".into());
    }
    if license_ui_script.is_some() != license_ui_report.is_some() {
        return Err("Use --license-ui-script with --license-ui-report.".into());
    }
    let receipt_directory = license_acceptance::directory(license_directory, profile_directory.as_deref())?;
    if let Some(digest) = license_digest {
        let scope = license_scope.ok_or("--accept-license requires --license-scope personal or organization.")?;
        if address.is_some() || render.is_some() || activity_report.is_some() || save_source.is_some()
            || input_script.is_some() || !diagnostic_urls.is_empty() || diagnostic_report.is_some() || license_ui_script.is_some() {
            return Err("Use explicit acceptance separately from browsing, diagnosis or input/output modes.".into());
        }
        license_acceptance::accept(&receipt_directory, &digest, scope)?;
        println!("Explicit local acceptance recorded: CSAPUL {}; SHA-256 {}; scope {}.", license_acceptance::LICENSE_VERSION, license_acceptance::LICENSE_SHA256, scope.text());
        return Ok(());
    }
    if license_scope.is_some() { return Err("--license-scope requires an explicit --accept-license operation.".into()); }
    let license_required = !matches!(license_acceptance::check(&receipt_directory)?, license_acceptance::State::Accepted(_));
    if license_required || license_ui_script.is_some() {
        let unattended = render.is_some() || activity_report.is_some() || save_source.is_some() || input_script.is_some()
            || !diagnostic_urls.is_empty() || diagnostic_report.is_some();
        if unattended && license_ui_script.is_none() { return license_acceptance::require(&receipt_directory); }
        #[cfg(target_os = "windows")]
        match platform::license_prompt(license_ui_script.as_deref(), license_ui_report.as_deref())? {
            Some(scope) => license_acceptance::accept(&receipt_directory, license_acceptance::LICENSE_SHA256, scope)?,
            None => return Ok(()),
        }
        #[cfg(not(target_os = "windows"))]
        return license_acceptance::require(&receipt_directory);
    }
    let outputs = [
        (&render, "--render"),
        (&activity_report, "--activity-report"),
        (&save_source, "--save-source"),
    ];
    for (index, (first, first_flag)) in outputs.iter().enumerate() {
        for (second, second_flag) in outputs.iter().skip(index + 1) {
            if let (Some(first), Some(second)) = (first, second) {
                let first = output_destination(first)?;
                let second = output_destination(second)?;
                // Windows normally treats case variants as one destination.
                #[cfg(target_os = "windows")]
                let same = first.to_string_lossy().to_uppercase()
                    == second.to_string_lossy().to_uppercase();
                #[cfg(not(target_os = "windows"))]
                let same = first == second;
                if same {
                    return Err(format!(
                        "Choose different files for {first_flag} and {second_flag}."
                    ));
                }
            }
        }
    }
    if zoom.is_some_and(|value| !(50..=300).contains(&value)) {
        return Err("--zoom must be between 50 and 300 percent".into());
    }
    let trust = if let Some(path) = trust_file {
        trust::Store::from_file(&path)?
    } else {
        let default_path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|dir| dir.join("trust/roots.pem")));
        match default_path {
            Some(path) => trust::Store::from_file(&path).unwrap_or_else(trust::Store::unavailable),
            None => trust::Store::unavailable(
                "Cannot locate trust/roots.pem beside the executable.".into(),
            ),
        }
    };
    if !diagnostic_urls.is_empty() || diagnostic_report.is_some() {
        if address.is_some()
            || render.is_some()
            || source_view
            || resource_view
            || inspector_view.is_some()
            || browser_view.is_some()
            || activity_report.is_some()
            || save_source.is_some()
            || input_script.is_some()
            || find.is_some()
            || profile_directory.is_some()
        {
            return Err(
                "Use --diagnose/--report separately from browsing and rendering options.".into(),
            );
        }
        let path = diagnostic_report.ok_or("--diagnose requires --report output.txt")?;
        return diagnostics::write_report(&path, &diagnostic_urls, &trust, &diagnostic_hosts);
    }
    let headless = render.is_some() || activity_report.is_some() || save_source.is_some() || input_script.is_some();
    let mut browser = app::App::new();
    browser.use_system_fonts();
    // Headless output is independent of personal settings unless a profile
    // folder is explicitly requested. Diagnostics returned above never read it.
    let profile_path = if no_profile {
        None
    } else if let Some(directory) = profile_directory {
        let directory = if directory.is_absolute() {
            directory
        } else {
            std::env::current_dir()
                .map_err(|error| format!("Cannot resolve profile folder: {error}"))?
                .join(directory)
        };
        Some(directory.join("profile.txt"))
    } else if cfg!(target_os = "windows") && !headless {
        Some(profile::default_path()?)
    } else {
        None
    };
    if let Some(path) = profile_path {
        let history_path = path.with_file_name("history.txt");
        let store = profile::Store::open(path.clone())
            .unwrap_or_else(|error| profile::Store::unavailable(path, error));
        browser.attach_profile(store);
        // Renders may read explicit preferences, but never inspect, append,
        // expire or clear personal browsing history.
        if cfg!(target_os = "windows") && !headless {
            let history = history::Store::open(history_path.clone())
                .unwrap_or_else(|error| history::Store::unavailable(history_path, error));
            browser.attach_browsing_history(history);
        }
    }
    browser.use_trust_store(std::sync::Arc::new(trust));
    if let Some(enabled) = scripts_enabled {
        browser.set_javascript_enabled(enabled);
    }
    if let Some(enabled) = images_enabled {
        browser.set_images_enabled(enabled);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            browser.use_home_file(dir.join("pages/start.html"));
        }
    }
    // Responsive image selection captures viewport and page zoom at load start.
    // Apply command-line dimensions before the first navigation, including renders.
    browser.resize(width, height);
    if let Some(zoom) = zoom {
        browser.set_zoom(zoom as f32 / 100.0);
    }
    if let Some(address) = address {
        browser.open_argument(&address);
    } else {
        browser.open_startup_home();
    }
    if headless || source_view || resource_view || find.is_some() || inspector_view.is_some() {
        browser.wait_for_load();
    }
    browser.wait_for_script_timers(std::time::Duration::from_millis(wait_ms as u64));
    if let (Some(script), Some(output)) = (input_script, input_output) {
        return browser.run_input_script(&script, &output);
    }
    if let Some(path) = activity_report {
        std::fs::write(&path, browser.activity_report())
            .map_err(|error| format!("Cannot write activity report {}: {error}", path.display()))?;
    }
    if !browser.failed {
        if let Some(path) = save_source {
            std::fs::write(&path, browser.document_source())
                .map_err(|error| format!("Cannot write page source {}: {error}", path.display()))?;
        }
    }
    if headless && render.is_none() {
        return if browser.failed {
            Err(browser.status.clone())
        } else {
            Ok(())
        };
    }
    if source_view {
        browser.show_source();
    }
    if resource_view {
        browser.show_resources();
    }
    if let Some(query) = find {
        browser.search(&query);
    }
    browser.scroll_by(scroll);
    if let Some(tab) = inspector_view {
        browser.open_inspector();
        browser.inspector_tab_by_name(&tab);
        if inspector_window {
            browser.detach_inspector();
            browser.inspector_window_size(width, height);
        }
    }
    if let Some(view) = browser_view {
        match view.as_str() {
            "settings" => browser.show_settings(),
            "appearance" => browser.show_appearance_settings(),
            "bookmarks" => browser.show_bookmarks(),
            "downloads" => browser.show_downloads(),
            "download-settings" => browser.show_download_settings(),
            _ => {}
        }
    }
    if let Some(path) = render {
        if browser.failed {
            return Err(browser.status.clone());
        }
        let canvas = if inspector_window {
            browser.draw_inspector_window()
        } else {
            browser.draw()
        };
        if canvas.pixels.is_empty() {
            return Err("Could not allocate the drawing surface.".to_owned());
        }
        let output = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        let mut out = std::io::BufWriter::new(output);
        write!(out, "P6\n{} {}\n255\n", canvas.width, canvas.height).map_err(|e| e.to_string())?;
        for pixel in &canvas.pixels {
            out.write_all(&[(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8])
                .map_err(|e| e.to_string())?;
        }
        out.flush().map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        platform::run(browser)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("The desktop window currently targets Windows. Use --render output.ppm to exercise the same engine here.".to_owned())
    }
}

fn number(value: Option<std::ffi::OsString>, name: &str) -> Result<i32, String> {
    value
        .and_then(|v| v.to_str().and_then(|s| s.parse::<i32>().ok()))
        .filter(|v| *v >= 0 && *v <= 8192)
        .ok_or_else(|| format!("{} requires an integer between 0 and 8192", name))
}

fn output_destination(path: &std::path::Path) -> Result<PathBuf, String> {
    // Resolve an existing file or its parent so relative spelling and symlink
    // aliases cannot silently overwrite the text report with rendered pixels.
    if let Ok(path) = path.canonicalize() {
        return Ok(path);
    }
    let absolute = std::path::absolute(path).map_err(|error| error.to_string())?;
    if let (Some(parent), Some(name)) = (absolute.parent(), absolute.file_name()) {
        if let Ok(parent) = parent.canonicalize() {
            return Ok(parent.join(name));
        }
    }
    Ok(absolute)
}
