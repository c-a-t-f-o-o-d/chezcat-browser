use super::*;
use std::net::TcpListener;

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let mut bytes = [0u8; 8];
        random::fill(&mut bytes).unwrap();
        let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let path = std::env::temp_dir().join(format!("chezcat-download-test-{token}"));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
    fn folder(&self) -> PathBuf {
        self.0.join("Downloads")
    }
    fn files(&self) -> Vec<PathBuf> {
        let mut files: Vec<_> = fs::read_dir(self.folder())
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        files.sort();
        files
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn trust() -> Arc<trust::Store> {
    Arc::new(trust::Store::unconfigured())
}
fn wake() -> Wake {
    Arc::new(|| {})
}
fn source(path: &Path) -> Location {
    Location {
        path: Some(path.to_owned()),
        url: None,
        fragment: String::new(),
        root: Some(path.parent().unwrap().to_owned()),
    }
}
fn complete(manager: &mut Manager) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while manager.active_count() > 0 {
        manager.poll();
        assert!(Instant::now() < deadline, "download did not finish");
        thread::sleep(Duration::from_millis(2));
    }
}
fn response(body: &[u8], disposition: &str) -> http::Response {
    http::Response {
        document_timing:None,
        redirected: false,
        url: crate::url::parse("https://download.invalid/payload")
            .unwrap()
            .0,
        fragment: None,
        status: 200,
        status_text:"OK".into(),
        encoded_body_bytes:body.len(),
        protocol: Some("TLS 1.3"),
        request_is_post: false,
        cookie_same_site: false,
        headers: vec![("content-disposition".into(), disposition.into())],
        body: body.to_vec(),
        events: Vec::new(),
    }
}

#[test]
fn default_folder_prefers_relocation_and_keeps_path_validation() {
    let root = std::env::temp_dir();
    let user = root.join("chezcat-user");
    let relocated = root.join("chezcat-relocated-downloads");
    assert_eq!(
        choose_default_folder(Some(relocated.clone()), Some(user.clone().into_os_string()))
            .unwrap(),
        relocated
    );
    assert_eq!(
        choose_default_folder(None, Some(user.clone().into_os_string())).unwrap(),
        user.join("Downloads")
    );
    assert!(choose_default_folder(None, None).is_err());
    assert!(choose_default_folder(
        Some(PathBuf::from("relative-downloads")),
        Some(user.clone().into_os_string())
    )
    .is_err());
    assert!(choose_default_folder(
        Some(root.join("bad/../folder")),
        Some(user.into_os_string())
    )
    .is_err());
}

#[test]
fn extended_names_prefer_utf8_and_preserve_literal_percent_in_plain_names() {
    assert_eq!(
        disposition_filename(
            "attachment; filename=plain.txt; filename*=UTF-8''caf%C3%A9%20report.txt"
        ),
        Some("café report.txt".into())
    );
    assert_eq!(
        disposition_filename("attachment; filename*=iso-8859-1'en'%A3%20rates.txt"),
        Some("£ rates.txt".into())
    );
    assert_eq!(
        disposition_filename("attachment; filename=\"100%20literal.txt\""),
        Some("100%20literal.txt".into())
    );
    assert_eq!(
        disposition_filename("attachment; filename=\"a; b.txt\"; size=40"),
        Some("a; b.txt".into())
    );
    assert_eq!(
        disposition_filename("attachment; filename=fallback.txt; filename*=UTF-8''%FF"),
        Some("fallback.txt".into())
    );
    assert_eq!(
        disposition_filename("attachment; filename=a.txt; FILENAME=b.txt"),
        None
    );
    assert_eq!(
        disposition_filename("attachment; filename*=UTF-8''one; filename*=UTF-8''two"),
        None
    );
    assert_eq!(
        disposition_filename("attachment; filename=\"unfinished"),
        None
    );
    assert_eq!(
        disposition_filename("attachment; filename=a\r\nX-Evil: yes"),
        None
    );
    assert!(is_attachment(" Attachment ; filename=a.txt"));
    assert!(!is_attachment("inline; filename=attachment"));
}

#[test]
fn advisory_names_never_become_paths_devices_or_alternate_streams() {
    assert_eq!(sanitize_filename("../../outside.txt"), "outside.txt");
    assert_eq!(sanitize_filename("C:\\Windows\\evil.exe"), "evil.exe");
    assert_eq!(sanitize_filename("secret.txt:stream"), "secret.txt_stream");
    assert_eq!(sanitize_filename("\u{202e}x\0y. "), "xy");
    assert_eq!(sanitize_filename(".."), "download.bin");
    assert_eq!(sanitize_filename(". . "), "download.bin");
    for name in [
        "CON",
        "con.txt",
        "AUX.png",
        "COM1",
        "com¹.jpg",
        "LPT².txt",
        "NUL.tar.gz",
        "CONOUT$",
    ] {
        assert!(
            sanitize_filename(name).starts_with('_'),
            "device name {name}"
        );
    }
    let long = sanitize_filename(&"é".repeat(1000));
    assert!(long.len() <= MAX_NAME_BYTES);
    assert!(!long.is_empty());
}

#[test]
fn local_downloads_publish_complete_unique_files_and_clear_only_the_list() {
    let temp = Temp::new();
    let original = temp.0.join("report.txt");
    fs::write(&original, b"download body\0exact").unwrap();
    let folder = prepare_folder(&temp.folder()).unwrap();
    fs::write(folder.join("report.txt"), b"keep existing").unwrap();
    let mut manager = Manager::default();
    for _ in 0..2 {
        manager
            .start(source(&original), None, &folder, trust(), wake())
            .unwrap();
        complete(&mut manager);
    }
    assert_eq!(
        fs::read(folder.join("report.txt")).unwrap(),
        b"keep existing"
    );
    assert_eq!(
        fs::read(folder.join("report (1).txt")).unwrap(),
        b"download body\0exact"
    );
    assert_eq!(
        fs::read(folder.join("report (2).txt")).unwrap(),
        b"download body\0exact"
    );
    assert!(manager
        .entries()
        .iter()
        .all(|entry| entry.status == Status::Complete && entry.bytes == 19));
    assert_eq!(manager.clear_finished(), 2);
    assert!(manager.entries().is_empty());
    assert_eq!(temp.files().len(), 3);
    assert!(temp
        .files()
        .iter()
        .all(|path| !path.to_string_lossy().ends_with(".part")));
}

#[test]
fn received_post_response_is_saved_without_replaying_it_or_offering_retry() {
    let temp = Temp::new();
    let mut manager = Manager::default();
    let mut received = response(
        b"already received",
        "attachment; filename*=UTF-8''résumé.bin",
    );
    // Invalid non-ASCII ext-value falls back safely to the URL basename.
    received.headers[0].1 = "attachment; filename*=UTF-8''r%C3%A9sum%C3%A9.bin".into();
    received.request_is_post = true;
    manager
        .start_received(received, &temp.folder(), trust(), wake())
        .unwrap();
    complete(&mut manager);
    let entry = &manager.entries()[0];
    assert_eq!(entry.status, Status::Complete);
    assert_eq!(entry.name, "résumé.bin");
    assert!(entry.retry.is_none());
    assert_eq!(
        fs::read(entry.path.as_ref().unwrap()).unwrap(),
        b"already received"
    );
}

#[test]
fn http_status_and_partial_responses_do_not_become_downloaded_files() {
    let temp = Temp::new();
    let mut manager = Manager::default();
    for status in [204, 205, 206, 301, 403, 404, 500] {
        let mut received = response(b"error page", "attachment; filename=error.html");
        received.status = status;
        assert!(manager
            .start_received(received, &temp.folder(), trust(), wake())
            .is_err());
    }
    assert!(manager.entries().is_empty());
    assert!(!temp.folder().exists());
}

#[test]
fn folder_validation_rejects_relative_parent_and_device_components() {
    assert!(validate_folder(Path::new("relative/Downloads")).is_err());
    let temp = Temp::new();
    assert!(validate_folder(&temp.folder()).is_ok());
    for suffix in ["../outside", "nested/../outside", "CON", "a:stream", "trailing."] {
        // PathBuf::join removes '..' from Windows verbatim paths. Append native
        // text instead so validation receives the unsafe component itself.
        let mut raw = temp.0.as_os_str().to_os_string();
        raw.push(std::path::MAIN_SEPARATOR_STR);
        raw.push(suffix.replace('/', std::path::MAIN_SEPARATOR_STR));
        let path = PathBuf::from(raw);
        if suffix.split('/').any(|part| part == "..") {
            assert!(
                path.components().any(|part| part == Component::ParentDir),
                "fixture must retain its parent component: {path:?}"
            );
        }
        assert!(validate_folder(&path).is_err(), "unsafe folder: {suffix}");
    }
    let file = temp.0.join("not-a-directory");
    fs::write(&file, b"keep").unwrap();
    assert!(prepare_folder(&file.join("Downloads")).is_err());
    assert_eq!(fs::read(file).unwrap(), b"keep");
}

#[cfg(unix)]
#[test]
fn symbolic_destinations_are_not_followed_and_symlink_folders_are_rejected() {
    use std::os::unix::fs::symlink;
    let temp = Temp::new();
    let folder = prepare_folder(&temp.folder()).unwrap();
    let outside = temp.0.join("outside");
    fs::write(&outside, b"original").unwrap();
    symlink(&outside, folder.join("result.txt")).unwrap();
    let mut manager = Manager::default();
    manager
        .start_received(
            response(b"new bytes", "attachment; filename=result.txt"),
            &folder,
            trust(),
            wake(),
        )
        .unwrap();
    complete(&mut manager);
    assert_eq!(manager.entries()[0].status, Status::Complete);
    assert_eq!(manager.entries()[0].name, "result (1).txt");
    assert_eq!(fs::read(&outside).unwrap(), b"original");
    let linked = temp.0.join("linked");
    symlink(&folder, &linked).unwrap();
    assert!(prepare_folder(&linked).is_err());
    assert!(prepare_folder(&linked.join("child")).is_err());
    assert!(!folder.join("child").exists());
}

#[test]
fn oversized_local_files_fail_without_staging_or_publishing() {
    let temp = Temp::new();
    let path = temp.0.join("large.bin");
    File::create(&path)
        .unwrap()
        .set_len(MAX_DOWNLOAD_BYTES as u64 + 1)
        .unwrap();
    let mut manager = Manager::default();
    manager
        .start(source(&path), None, &temp.folder(), trust(), wake())
        .unwrap();
    complete(&mut manager);
    assert!(
        matches!(&manager.entries()[0].status, Status::Failed(error) if error.contains("32 MiB"))
    );
    assert!(temp.files().is_empty());
}

#[test]
fn local_sources_cannot_escape_the_established_document_folder() {
    let temp = Temp::new();
    let path = temp.0.join("outside.txt");
    fs::write(&path, b"private").unwrap();
    let root = temp.0.join("document");
    fs::create_dir(&root).unwrap();
    let mut location = source(&path);
    location.root = Some(root);
    let mut manager = Manager::default();
    manager
        .start(location, None, &temp.folder(), trust(), wake())
        .unwrap();
    complete(&mut manager);
    assert!(
        matches!(&manager.entries()[0].status, Status::Failed(error) if error.contains("outside"))
    );
    assert!(temp.files().is_empty());
}

#[test]
fn cancellation_during_file_write_removes_partial_files() {
    struct CancellingReader<'a> {
        cancelled: &'a AtomicBool,
        first: bool,
    }
    impl Read for CancellingReader<'_> {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if !self.first {
                return Ok(0);
            }
            self.first = false;
            out[..4].copy_from_slice(b"data");
            self.cancelled.store(true, Ordering::Relaxed);
            Ok(4)
        }
    }
    let temp = Temp::new();
    let folder = prepare_folder(&temp.folder()).unwrap();
    let cancel = AtomicBool::new(false);
    let (send, _receive) = mpsc::sync_channel(4);
    let result = save_reader(
        CancellingReader {
            cancelled: &cancel,
            first: true,
        },
        &folder,
        "result.bin",
        &cancel,
        &send,
        &wake(),
    );
    assert!(result.is_err());
    assert!(temp.files().is_empty());
}

fn server(reply: &'static [u8]) -> (Location, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let thread = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        let mut byte = [0u8; 1];
        while !request.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        assert!(request.starts_with(b"GET /file HTTP/1.1\r\n"));
        stream.write_all(reply).unwrap();
    });
    (
        web_location(
            crate::url::parse(&format!("http://{address}/file"))
                .unwrap()
                .0,
        ),
        thread,
    )
}

#[test]
fn actual_http_download_honors_disposition_and_saves_exact_body() {
    let temp = Temp::new();
    let (location, server) = server(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nContent-Disposition: attachment; filename=\"../../safe.bin\"\r\nConnection: close\r\n\r\na\0b\xffxyz");
    let mut manager = Manager::default();
    manager
        .start(
            location,
            Some("hint.txt".into()),
            &temp.folder(),
            trust(),
            wake(),
        )
        .unwrap();
    complete(&mut manager);
    server.join().unwrap();
    let entry = &manager.entries()[0];
    assert_eq!(entry.status, Status::Complete);
    assert_eq!(entry.name, "safe.bin");
    assert_eq!(entry.bytes, 7);
    assert_eq!(
        fs::read(entry.path.as_ref().unwrap()).unwrap(),
        b"a\0b\xffxyz"
    );
    assert_eq!(temp.files().len(), 1);
}

#[test]
fn truncated_http_body_is_not_published() {
    let temp = Temp::new();
    let (location, server) =
        server(b"HTTP/1.1 200 OK\r\nContent-Length: 40\r\nConnection: close\r\n\r\nshort");
    let mut manager = Manager::default();
    manager
        .start(location, None, &temp.folder(), trust(), wake())
        .unwrap();
    complete(&mut manager);
    server.join().unwrap();
    assert!(matches!(manager.entries()[0].status, Status::Failed(_)));
    assert!(temp.files().is_empty());
}

#[test]
fn cancel_http_transfer_keeps_it_active_until_the_worker_cleans_up() {
    let temp = Temp::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let location = web_location(
        crate::url::parse(&format!("http://{}/file", listener.local_addr().unwrap()))
            .unwrap()
            .0,
    );
    let (connected, observed) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut buffer = [0u8; 2048];
        let _ = stream.read(&mut buffer);
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\nConnection: close\r\n\r\npartial",
            )
            .unwrap();
        connected.send(()).unwrap();
        // The client cancels while waiting for the remainder, then closes.
        let _ = stream.read(&mut buffer);
    });
    let mut manager = Manager::default();
    let id = manager
        .start(location, None, &temp.folder(), trust(), wake())
        .unwrap();
    observed.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(manager.cancel(id));
    assert_eq!(manager.entries()[0].status, Status::Cancelling);
    assert_eq!(manager.clear_finished(), 0);
    complete(&mut manager);
    assert_eq!(manager.entries()[0].status, Status::Cancelled);
    assert!(manager.entries()[0].can_retry());
    assert!(temp.files().is_empty());
    server.join().unwrap();
}

#[test]
fn recent_download_list_is_bounded() {
    let temp = Temp::new();
    let mut manager = Manager::default();
    for _ in 0..MAX_ENTRIES + 4 {
        manager
            .start_received(
                response(b"", "attachment; filename=empty.txt"),
                &temp.folder(),
                trust(),
                wake(),
            )
            .unwrap();
        complete(&mut manager);
    }
    assert_eq!(manager.entries().len(), MAX_ENTRIES);
    assert_eq!(temp.files().len(), MAX_ENTRIES + 4);
    assert!(manager
        .entries()
        .windows(2)
        .all(|pair| pair[0].id > pair[1].id));
}

#[test]
fn retry_is_an_explicit_new_attempt_that_preserves_the_previous_failure() {
    let temp = Temp::new();
    let path = temp.0.join("not-yet-created.bin");
    let mut manager = Manager::default();
    let first = manager
        .start(source(&path), None, &temp.folder(), trust(), wake())
        .unwrap();
    complete(&mut manager);
    assert!(matches!(manager.entries()[0].status, Status::Failed(_)));
    fs::write(&path, b"available now").unwrap();
    let second = manager
        .retry(first, &temp.folder(), trust(), wake())
        .unwrap();
    complete(&mut manager);
    assert_ne!(first, second);
    assert_eq!(manager.entries()[0].status, Status::Complete);
    assert!(matches!(manager.entries()[1].status, Status::Failed(_)));
    assert_eq!(
        fs::read(manager.entries()[0].path.as_ref().unwrap()).unwrap(),
        b"available now"
    );
}

#[test]
fn read_failure_after_partial_write_never_publishes_a_file() {
    struct FailedReader(bool);
    impl Read for FailedReader {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if self.0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "fixture read failed",
                ));
            }
            self.0 = true;
            out[..4].copy_from_slice(b"part");
            Ok(4)
        }
    }
    let temp = Temp::new();
    let folder = prepare_folder(&temp.folder()).unwrap();
    let (send, _receive) = mpsc::sync_channel(4);
    let result = save_reader(
        FailedReader(false),
        &folder,
        "result.bin",
        &AtomicBool::new(false),
        &send,
        &wake(),
    );
    assert!(result.is_err());
    assert!(temp.files().is_empty());
}

#[test]
fn concurrent_downloads_are_bounded_and_shutdown_cancels_all_workers() {
    let temp = Temp::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let location = web_location(
        crate::url::parse(&format!("http://{}/file", listener.local_addr().unwrap()))
            .unwrap()
            .0,
    );
    let (ready, observed) = mpsc::channel();
    let (release, finish) = mpsc::channel();
    let server = thread::spawn(move || {
        let mut peers = Vec::new();
        for _ in 0..MAX_ACTIVE {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut byte = [0u8; 1];
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\nConnection: close\r\n\r\npartial").unwrap();
            peers.push(stream);
        }
        ready.send(()).unwrap();
        let _ = finish.recv_timeout(Duration::from_secs(3));
    });
    let mut manager = Manager::default();
    for _ in 0..MAX_ACTIVE {
        manager
            .start(location.clone(), None, &temp.folder(), trust(), wake())
            .unwrap();
    }
    observed.recv_timeout(Duration::from_secs(3)).unwrap();
    let error = manager
        .start(location, None, &temp.folder(), trust(), wake())
        .unwrap_err();
    assert!(error.contains("At most 3"));
    assert_eq!(manager.active_count(), MAX_ACTIVE);
    assert_eq!(manager.clear_finished(), 0);
    let started = Instant::now();
    drop(manager);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(temp.files().is_empty());
    release.send(()).unwrap();
    server.join().unwrap();
}
