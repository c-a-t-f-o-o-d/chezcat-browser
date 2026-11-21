//! Source-bounded framing records through the production HTTP parser.
//! These authored byte streams use no sockets, DNS, TLS or browser fixtures.
use super::*;
use std::cell::Cell;
use std::io::Cursor;

const MANY_CHUNKS: usize = 65_537;
const MANY_INFORMATIONAL: usize = 17;

fn chunk_prefix(count: usize) -> Vec<u8> {
    let mut wire = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nX-Final: retained\r\n\r\n".to_vec();
    wire.extend_from_slice(&b"1\r\nx\r\n".repeat(count));
    wire
}

fn informational_prefix(count: usize) -> Vec<u8> {
    b"HTTP/1.1 103 Early Hints\r\nX-Interim: ignored\r\n\r\n".repeat(count)
}

fn parse(wire: &[u8], limit: usize) -> Result<Message, String> {
    read_response(
        Cursor::new(wire),
        limit,
        &AtomicBool::new(false),
        Instant::now() + Duration::from_secs(10),
        false,
    )
}

#[test]
fn chunk_records_past_old_cap_keep_the_last_payload_and_trailer_isolation() {
    let mut wire = chunk_prefix(MANY_CHUNKS - 1);
    wire.extend_from_slice(b"1\r\nZ\r\n0\r\nX-Trailer: ignored\r\n\r\n");
    let message = parse(&wire, MANY_CHUNKS).unwrap();
    assert_eq!(message.status, 200);
    assert_eq!(message.body.len(), MANY_CHUNKS);
    assert!(message.body[..MANY_CHUNKS - 1].iter().all(|&byte| byte == b'x'));
    assert_eq!(message.body[MANY_CHUNKS - 1], b'Z');
    assert_eq!(message.encoded_bytes, MANY_CHUNKS);
    assert_eq!(header(&message.headers, "x-final"), Some("retained"));
    assert!(header(&message.headers, "x-trailer").is_none());
    assert!(message.reusable);

    // Buffered bytes after the completed message still prevent connection reuse.
    wire.extend_from_slice(b"trailing");
    let message = parse(&wire, MANY_CHUNKS).unwrap();
    assert_eq!(message.body[MANY_CHUNKS - 1], b'Z');
    assert!(!message.reusable);
}

#[test]
fn chunk_records_past_old_cap_reject_late_malformed_or_truncated_framing() {
    let prefix = chunk_prefix(MANY_CHUNKS);
    for (tail, expected) in [
        (b"g\r\n".as_slice(), "Invalid HTTP chunk size"),
        (b"ffffffffffffffffffffffffffffffff\r\n", "chunk size is too large"),
        (b"1\r\nz\rX", "terminating CRLF"),
        (b"3\r\nxy", "body is truncated"),
        (b"1", "before the message was complete"),
        (b"0\r\n", "before the message was complete"),
        (b"0\r\nContent-Length: 0\r\n\r\n", "prohibited field"),
    ] {
        let mut wire = prefix.clone();
        wire.extend_from_slice(tail);
        let error = parse(&wire, MANY_CHUNKS + 3).unwrap_err();
        assert!(error.contains(expected), "{expected}: {error}");
    }
}

#[test]
fn chunk_records_keep_aggregate_metadata_and_body_admission() {
    let mut wire = chunk_prefix(MANY_CHUNKS);
    wire.extend_from_slice(b"0\r\n\r\n");
    assert!(parse(&wire, MANY_CHUNKS - 1)
        .unwrap_err()
        .contains("body exceeds"));

    // A one-byte chunk spends five metadata bytes, independently of its body.
    // The failure must be this aggregate budget, not the old record count.
    let count = MAX_CHUNK_METADATA / 5 + 1;
    let mut wire = chunk_prefix(count);
    wire.extend_from_slice(b"0\r\n\r\n");
    assert!(parse(&wire, count)
        .unwrap_err()
        .contains("chunk metadata exceeds"));
}

#[test]
fn informational_records_past_old_cap_preserve_final_headers_and_persistence() {
    for intermediate in [
        b"HTTP/1.1 100 Continue\r\n\r\n".as_slice(),
        b"HTTP/1.1 103 Early Hints\r\nConnection: close\r\n\r\n",
        b"HTTP/1.0 100 Continue\r\n\r\n",
    ] {
        let mut wire = informational_prefix(MANY_INFORMATIONAL);
        wire.extend_from_slice(intermediate);
        wire.extend_from_slice(b"HTTP/1.1 200 Complete\r\nContent-Length: 4\r\nX-Final: retained\r\n\r\ntail");
        let message = parse(&wire, 4).unwrap();
        assert_eq!(message.status, 200);
        assert_eq!(message.status_text, "Complete");
        assert_eq!(message.body, b"tail");
        assert_eq!(header(&message.headers, "x-final"), Some("retained"));
        assert!(header(&message.headers, "x-interim").is_none());
        assert_eq!(message.reusable, intermediate == b"HTTP/1.1 100 Continue\r\n\r\n");
    }
}

#[test]
fn informational_records_past_old_cap_reject_late_protocol_and_framing_errors() {
    let prefix = informational_prefix(MANY_INFORMATIONAL);
    for (tail, expected) in [
        (b"HTTP/1.1 101 Switching Protocols\r\n\r\n".as_slice(), "upgrades are not supported"),
        (b"HTTP/1.1 103 Hints\r\nContent-Length: 0\r\n\r\n", "cannot declare body framing"),
        (b"HTTP/1.1 100 Continue\r\nTransfer-Encoding: chunked\r\n\r\n", "cannot declare body framing"),
        (b"HTTP/1.1 200\r\n\r\n", "Malformed or unsupported HTTP status"),
        (b"HTTP/1.1 200 OK\r\nContent-Length: 1", "before the message was complete"),
        (b"".as_slice(), "before the message was complete"),
    ] {
        let mut wire = prefix.clone();
        wire.extend_from_slice(tail);
        let error = parse(&wire, 4).unwrap_err();
        assert!(error.contains(expected), "{expected}: {error}");
    }
}

#[test]
fn informational_records_share_header_and_final_body_admission() {
    let mut wire = informational_prefix(MANY_INFORMATIONAL);
    wire.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ntail");
    assert!(parse(&wire, 3).unwrap_err().contains("body exceeds"));

    let one = b"HTTP/1.1 103 Hints\r\n\r\n";
    let mut wire = one.repeat(MAX_HEADER_BYTES / one.len() + 1);
    wire.extend_from_slice(b"HTTP/1.1 204 No Content\r\n\r\n");
    assert!(parse(&wire, 0).unwrap_err().contains("headers exceed 32 KiB"));
}

enum Interruption<'a> {
    Cancel(&'a AtomicBool),
    Deadline(Instant),
}

struct InterruptAt<'a> {
    source: Cursor<&'a [u8]>,
    boundary: usize,
    interruption: Interruption<'a>,
    triggered: &'a Cell<bool>,
}

impl Read for InterruptAt<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let position = self.source.position() as usize;
        if position == self.boundary && !self.triggered.replace(true) {
            match self.interruption {
                Interruption::Cancel(cancel) => cancel.store(true, Ordering::Release),
                Interruption::Deadline(deadline) => {
                    // The production parser receives its real Instant deadline.
                    // Delay only at the admitted boundary, with no clock shim.
                    std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
                }
            }
        }
        let count = if position < self.boundary {
            output.len().min(self.boundary - position)
        } else {
            output.len()
        };
        self.source.read(&mut output[..count])
    }
}

fn interruption_streams() -> Vec<(Vec<u8>, usize, usize)> {
    let mut chunks = chunk_prefix(MANY_CHUNKS);
    let chunk_boundary = chunks.len();
    chunks.extend_from_slice(b"0\r\n\r\n");
    let mut information = informational_prefix(MANY_INFORMATIONAL);
    let information_boundary = information.len();
    information.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ntail");
    vec![(chunks, chunk_boundary, MANY_CHUNKS), (information, information_boundary, 4)]
}

#[test]
fn framing_records_remain_cancelable_after_the_former_cutoffs() {
    for (wire, boundary, limit) in interruption_streams() {
        let cancel = AtomicBool::new(false);
        let triggered = Cell::new(false);
        let source = InterruptAt {
            source: Cursor::new(wire.as_slice()),
            boundary,
            interruption: Interruption::Cancel(&cancel),
            triggered: &triggered,
        };
        let error = read_response(source, limit, &cancel,
            Instant::now() + Duration::from_secs(10), false).unwrap_err();
        assert!(triggered.get(), "the parser must reach the boundary beyond its old cap");
        assert!(error.contains("request cancelled"), "{error}");
    }
}

#[test]
fn framing_records_keep_real_deadlines_after_the_former_cutoffs() {
    for (wire, boundary, limit) in interruption_streams() {
        let cancel = AtomicBool::new(false);
        let triggered = Cell::new(false);
        let deadline = Instant::now() + Duration::from_secs(2);
        let source = InterruptAt {
            source: Cursor::new(wire.as_slice()),
            boundary,
            interruption: Interruption::Deadline(deadline),
            triggered: &triggered,
        };
        let error = read_response(source, limit, &cancel, deadline, false).unwrap_err();
        assert!(triggered.get(), "the parser must reach the boundary before expiring");
        assert!(error.contains("request timed out"), "{error}");
    }
}
