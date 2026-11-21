//! Response fields use admitted bytes, not a separate record ceiling.
use super::*;
use std::cell::Cell;
use std::io::Cursor;

const MANY_FIELDS: usize = 1_024;

fn prefix(status: &[u8], count: usize) -> Vec<u8> {
    let mut wire = status.to_vec();
    wire.extend_from_slice(&b"X: x\r\n".repeat(count));
    wire
}

fn parse(wire: &[u8], limit: usize) -> Result<Message, String> {
    read_response(Cursor::new(wire), limit, &AtomicBool::new(false),
        Instant::now() + Duration::from_secs(10), false)
}

#[test]
fn response_fields_past_old_count_preserve_order_values_and_body() {
    let mut wire = prefix(b"HTTP/1.1 200 Complete\r\n", MANY_FIELDS);
    wire.extend_from_slice(b"X-Opaque: \t\x80\xff \t\r\nSet-Cookie: first=one\r\nSet-Cookie: second=two\r\nContent-Length: 4\r\nContent-Length: 4\r\n\r\ntail");
    let message = parse(&wire, 4).unwrap();
    assert_eq!(message.status_text, "Complete");
    assert_eq!(message.body, b"tail");
    assert_eq!(message.headers.len(), MANY_FIELDS + 5);
    assert!(message.headers[..MANY_FIELDS].iter().all(|entry| entry == &("x".into(), "x".into())));
    assert_eq!(message.headers[MANY_FIELDS], ("x-opaque".into(), "\u{80}\u{ff}".into()));
    assert_eq!(message.headers[MANY_FIELDS + 1], ("set-cookie".into(), "first=one".into()));
    assert_eq!(message.headers[MANY_FIELDS + 2], ("set-cookie".into(), "second=two".into()));
    assert_eq!(message.encoded_bytes, 4);
    assert!(message.reusable);

    let mut closing = prefix(b"HTTP/1.1 200 OK\r\n", MANY_FIELDS);
    closing.extend_from_slice(b"Connection: close\r\nContent-Length: 1\r\n\r\nx");
    assert!(!parse(&closing, 1).unwrap().reusable);
}

#[test]
fn many_interim_final_and_trailer_fields_keep_separate_meaning() {
    let mut wire = b"HTTP/1.1 103 Hints\r\n".to_vec();
    wire.extend_from_slice(&b"X-Interim: v\r\n".repeat(256));
    wire.extend_from_slice(b"\r\nHTTP/1.1 200 OK\r\n");
    wire.extend_from_slice(&b"X-Final: v\r\n".repeat(MANY_FIELDS));
    wire.extend_from_slice(b"Transfer-Encoding: chunked\r\n\r\n4\r\ntail\r\n0\r\n");
    wire.extend_from_slice(&b"X-Trailer: v\r\n".repeat(512));
    wire.extend_from_slice(b"\r\n");
    let message = parse(&wire, 4).unwrap();
    assert_eq!(message.body, b"tail");
    assert_eq!(message.headers.len(), MANY_FIELDS + 1);
    assert_eq!(header(&message.headers, "x-final"), Some("v"));
    assert_eq!(header(&message.headers, "x-interim"), None);
    assert_eq!(header(&message.headers, "x-trailer"), None);
    assert!(message.reusable);
}

#[test]
fn field_syntax_and_framing_refuse_late_errors_after_old_count() {
    for (tail, expected) in [
        (b"Bad Name: value\r\n\r\n".as_slice(), "Invalid HTTP field name"),
        (b" folded\r\n\r\n", "missing its colon"),
        (b"X: bad\x7fvalue\r\n\r\n", "forbidden control"),
        (b"X: bad\0value\r\n\r\n", "forbidden control"),
        (b"X: truncated", "before the message was complete"),
        (b"X: bare-LF\n\r\n", "CRLF"),
        (b"Content-Length: 1\r\nTransfer-Encoding: chunked\r\n\r\n", "both Transfer-Encoding and Content-Length"),
        (b"Content-Length: 1\r\nContent-Length: 2\r\n\r\n", "Conflicting HTTP Content-Length"),
        (b"Transfer-Encoding: chunked\r\nTransfer-Encoding: chunked\r\n\r\n", "single chunked"),
        (b"Transfer-Encoding: ,chunked\r\n\r\n", "single chunked"),
    ] {
        let mut wire = prefix(b"HTTP/1.1 200 OK\r\n", MANY_FIELDS);
        wire.extend_from_slice(tail);
        let error = parse(&wire, 4).unwrap_err();
        assert!(error.contains(expected), "{expected}: {error}");
    }
    let mut redirect = prefix(b"HTTP/1.1 302 Found\r\n", MANY_FIELDS);
    redirect.extend_from_slice(b"Location: /first\r\nLocation: /second\r\nContent-Length: 0\r\n\r\n");
    assert!(parse(&redirect, 0).unwrap_err().contains("Duplicate HTTP location"));
}

#[test]
fn many_trailers_cannot_supply_framing_or_sensitive_fields() {
    let mut start = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\nx\r\n0\r\n".to_vec();
    start.extend_from_slice(&b"X: x\r\n".repeat(MANY_FIELDS));
    for name in ["Content-Length", "Transfer-Encoding", "Connection", "Location",
        "Content-Encoding", "Authorization", "Cookie", "Set-Cookie"] {
        let mut wire = start.clone();
        wire.extend_from_slice(format!("{name}: forbidden\r\n\r\n").as_bytes());
        assert!(parse(&wire, 1).unwrap_err().contains("prohibited field"), "{name}");
    }
}

#[test]
fn fields_keep_aggregate_header_line_and_body_admission() {
    let mut many = prefix(b"HTTP/1.1 200 OK\r\n", MAX_HEADER_BYTES / 6 + 1);
    many.extend_from_slice(b"Content-Length: 0\r\n\r\n");
    assert!(parse(&many, 0).unwrap_err().contains("headers exceed 32 KiB"));

    let mut aggregate = prefix(b"HTTP/1.1 103 Hints\r\n", 3_000);
    aggregate.extend_from_slice(b"\r\nHTTP/1.1 200 OK\r\n");
    aggregate.extend_from_slice(&b"X: x\r\n".repeat(3_000));
    aggregate.extend_from_slice(b"Content-Length: 0\r\n\r\n");
    assert!(parse(&aggregate, 0).unwrap_err().contains("headers exceed 32 KiB"));

    let mut line = prefix(b"HTTP/1.1 200 OK\r\n", MANY_FIELDS);
    line.extend_from_slice(b"X:");
    line.extend_from_slice(&vec![0xff; MAX_LINE_BYTES - 3]);
    line.extend_from_slice(b"\r\n\r\n");
    assert!(parse(&line, 0).unwrap_err().contains("line exceeds 8 KiB"));

    let mut body = prefix(b"HTTP/1.1 200 OK\r\n", MANY_FIELDS);
    body.extend_from_slice(b"Content-Length: 4\r\n\r\ntail");
    assert!(parse(&body, 3).unwrap_err().contains("body exceeds"));
}

#[test]
fn maximum_admitted_line_keeps_all_latin1_value_bytes() {
    let mut wire = b"HTTP/1.1 200 OK\r\nX:".to_vec();
    wire.extend_from_slice(&vec![0xff; MAX_LINE_BYTES - 4]);
    wire.extend_from_slice(b"\r\nContent-Length: 0\r\n\r\n");
    let message = parse(&wire, 0).unwrap();
    let value = header(&message.headers, "x").unwrap();
    assert_eq!(value.chars().count(), MAX_LINE_BYTES - 4);
    assert_eq!(value.len(), 2 * (MAX_LINE_BYTES - 4));
    assert!(value.chars().all(|ch| ch == '\u{ff}'));
}

struct InterruptAt<'a> {
    source: Cursor<&'a [u8]>,
    boundary: usize,
    cancel: Option<&'a AtomicBool>,
    deadline: Instant,
    triggered: &'a Cell<bool>,
}

impl Read for InterruptAt<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let position = self.source.position() as usize;
        if position == self.boundary && !self.triggered.replace(true) {
            if let Some(cancel) = self.cancel {
                cancel.store(true, Ordering::Release);
            } else {
                std::thread::sleep(self.deadline.saturating_duration_since(Instant::now()));
            }
        }
        let count = if position < self.boundary { output.len().min(self.boundary - position) }
            else { output.len() };
        self.source.read(&mut output[..count])
    }
}

#[test]
fn fields_remain_cancelable_and_deadlines_apply_after_old_boundary() {
    let mut headers = prefix(b"HTTP/1.1 200 OK\r\n", MANY_FIELDS);
    let header_boundary = headers.len();
    headers.extend_from_slice(b"Content-Length: 1\r\n\r\nx");
    let mut trailers = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\nx\r\n0\r\n".to_vec();
    trailers.extend_from_slice(&b"X: x\r\n".repeat(MANY_FIELDS));
    let trailer_boundary = trailers.len();
    trailers.extend_from_slice(b"X-Final: v\r\n\r\n");
    for (wire, boundary) in [(headers, header_boundary), (trailers, trailer_boundary)] {
        for cancel_now in [true, false] {
            let cancel = AtomicBool::new(false);
            let triggered = Cell::new(false);
            let deadline = Instant::now() + Duration::from_secs(1);
            let source = InterruptAt { source: Cursor::new(wire.as_slice()), boundary,
                cancel: cancel_now.then_some(&cancel), deadline, triggered: &triggered };
            let error = read_response(source, 1, &cancel, deadline, false).unwrap_err();
            assert!(triggered.get(), "must reach fields beyond the old count before refusal");
            assert!(error.contains(if cancel_now { "request cancelled" } else { "request timed out" }), "{error}");
        }
        assert_eq!(parse(&wire, 1).unwrap().body, b"x");
    }
}
