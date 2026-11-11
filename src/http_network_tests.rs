use super::*;
use crate::network_trace::{Outcome, PhaseKind, Trace};
use std::net::TcpListener;
use std::sync::Barrier;

fn request_head(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut byte = [0];
    while !bytes.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
        assert!(bytes.len() < 32768);
    }
    String::from_utf8(bytes).unwrap()
}

#[test]
fn network_trace_measures_real_wait_receive_and_connection_reuse() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let peer = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        request_head(&mut stream);
        std::thread::sleep(Duration::from_millis(35));
        let head = b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n";
        stream.write_all(head).unwrap();
        stream.write_all(b"a").unwrap();
        std::thread::sleep(Duration::from_millis(40));
        stream.write_all(b"bc").unwrap();
        request_head(&mut stream);
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
            .unwrap();
        head.len()
    });
    let url = crate::url::parse(&format!("http://127.0.0.1:{port}/delayed"))
        .unwrap()
        .0;
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let trace = Trace::default();
    let pool = Pool::new(&cancel, &trust).with_network_trace(trace.clone());
    assert_eq!(
        pool.fetch_kind(&url, 100, None, FetchMode::Normal, "image")
            .unwrap()
            .body,
        b"abc"
    );
    assert_eq!(
        pool.fetch_kind(&url, 100, None, FetchMode::Normal, "image")
            .unwrap()
            .status,
        204
    );
    let header_bytes = peer.join().unwrap();
    let snapshot = trace.snapshot();
    assert_eq!(snapshot.rows.len(), 2);
    let first = &snapshot.rows[0];
    assert_eq!(first.outcome, Outcome::Complete);
    assert_eq!(first.status, Some(200));
    assert_eq!(first.http_bytes, header_bytes + 3);
    assert_eq!(first.encoded_bytes, Some(3));
    assert!(!first.reused_connection);
    assert!(!first
        .phases
        .iter()
        .any(|p| matches!(p.kind, PhaseKind::Dns | PhaseKind::Tls)));
    for kind in [PhaseKind::Wait, PhaseKind::Receive] {
        let phase = first.phases.iter().find(|p| p.kind == kind).unwrap();
        assert!(
            phase.end.unwrap() - phase.start >= Duration::from_millis(20),
            "{kind:?}: {phase:?}"
        );
    }
    assert!(snapshot.rows[1].reused_connection);
    assert!(!snapshot.rows[1]
        .phases
        .iter()
        .any(|p| matches!(p.kind, PhaseKind::Connect | PhaseKind::Dns | PhaseKind::Tls)));
    assert!(first.end.unwrap() <= snapshot.rows[1].start);
    for row in &snapshot.rows {
        for pair in row.phases.windows(2) {
            assert!(pair[0].end.unwrap() <= pair[1].start);
        }
    }
}

#[test]
fn network_trace_records_redirect_each_attempt_and_partial_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let peer = std::thread::spawn(move || {
        for index in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let head = request_head(&mut stream);
            if index == 0 {
                assert!(head.starts_with("GET /redirect "));
                stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: /short\r\nContent-Length: 500\r\nConnection: close\r\n\r\n").unwrap();
            } else {
                assert!(head.starts_with("GET /short "));
                stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nxy",
                    )
                    .unwrap();
            }
        }
    });
    let url = crate::url::parse(&format!("http://127.0.0.1:{port}/redirect"))
        .unwrap()
        .0;
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let trace = Trace::default();
    let pool = Pool::new(&cancel, &trust).with_network_trace(trace.clone());
    assert!(pool
        .fetch_kind(&url, 100, None, FetchMode::Normal, "document")
        .is_err());
    peer.join().unwrap();
    let rows = trace.snapshot().rows;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].status, Some(302));
    assert!(rows[0].note.contains("intentionally skipped"));
    assert_eq!(rows[0].encoded_bytes, None); // Redirect body intentionally not processed.
    assert!(rows[0].end.unwrap() <= rows[1].start);
    assert_eq!(rows[1].outcome, Outcome::Failed);
    assert_eq!(rows[1].status, Some(200));
    assert!(rows[1].http_bytes > 2);
    assert_eq!(rows[1].encoded_bytes, None);
    assert!(rows[1].note.contains("truncated"));
}

#[test]
fn network_trace_measures_overlapping_actual_parallel_requests() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let barrier = Arc::new(Barrier::new(3));
    let peer = std::thread::spawn(move || {
        let mut peers = Vec::new();
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            let barrier = barrier.clone();
            peers.push(std::thread::spawn(move || {
                request_head(&mut stream);
                barrier.wait();
                std::thread::sleep(Duration::from_millis(25));
                stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\nx",
                    )
                    .unwrap();
            }));
        }
        for peer in peers {
            peer.join().unwrap();
        }
    });
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let trace = Trace::default();
    let pool = Pool::new(&cancel, &trust).with_network_trace(trace.clone());
    std::thread::scope(|scope| {
        for index in 0..3 {
            let pool = &pool;
            scope.spawn(move || {
                let url = crate::url::parse(&format!("http://127.0.0.1:{port}/image-{index}"))
                    .unwrap()
                    .0;
                assert_eq!(
                    pool.fetch_kind(&url, 100, None, FetchMode::Normal, "image")
                        .unwrap()
                        .body,
                    b"x"
                );
            });
        }
    });
    peer.join().unwrap();
    let rows = trace.snapshot().rows;
    assert_eq!(rows.len(), 3);
    let last_start = rows.iter().map(|row| row.start).max().unwrap();
    let first_end = rows.iter().map(|row| row.end.unwrap()).min().unwrap();
    assert!(last_start < first_end, "parallel intervals must overlap");
    assert!(rows.iter().all(|row| row.outcome == Outcome::Complete));
}

#[test]
fn document_download_limit_is_selected_from_real_headers_without_replay() {
    let cancel = AtomicBool::new(false);
    let content = vec![b'a'; 2 * 1024 * 1024 + 1];
    for (disposition, accepted) in [
        ("attachment; filename=test.bin", true),
        ("inline; filename=test.html", false),
        ("attachevil", false),
    ] {
        let mut wire = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Disposition: {disposition}\r\n\r\n",
            content.len()
        )
        .into_bytes();
        wire.extend_from_slice(&content);
        let response = read_response_traced(
            wire.as_slice(),
            MAX_BODY_BYTES,
            &cancel,
            Instant::now() + Duration::from_secs(3),
            true,
            None,
            true,
        );
        assert_eq!(response.is_ok(), accepted, "{disposition}");
        if accepted {
            assert_eq!(response.unwrap().body, content);
        } else {
            assert!(response.err().unwrap().text.contains("2097152-byte limit"));
        }
    }
}

#[test]
fn network_trace_records_block_before_dns_without_a_connection() {
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let trace = Trace::default();
    let filter = |_url: &HttpUrl| Err("Test resource policy".to_owned());
    let pool = Pool::new(&cancel, &trust)
        .with_resource_filter(&filter)
        .with_network_trace(trace.clone());
    let url = crate::url::parse("http://must-not-resolve.invalid/blocked")
        .unwrap()
        .0;
    assert!(pool
        .fetch_kind(&url, 100, Some(&url), FetchMode::Normal, "script")
        .is_err());
    let rows = trace.snapshot().rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].outcome, Outcome::Blocked);
    assert!(rows[0].phases.is_empty());
    assert_eq!(rows[0].http_bytes, 0);
}
