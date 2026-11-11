//! Original TLS 1.3 / TLS 1.2 client over an existing byte stream.
//!
//! One ClientHello offers both versions. TLS 1.3 uses X25519 (or P-256 after
//! one validated HelloRetryRequest) and our existing
//! ChaCha20-Poly1305/AES-GCM state; TLS 1.2 prefers ECDHE AES-GCM and also
//! offers authenticated SHA-256/SHA-384 AES-CBC compatibility suites.
//! Certificate validation uses our X.509 module and explicit trust anchors.
//! There is no reconnect fallback, resumption, early data,
//! renegotiation, or client authentication.
//!
//! The caller supplies fresh OS-generated entropy and a deadline/cancellation
//! check. Blocking transports must have short read AND write timeouts so that
//! the check can run while a peer stalls. This code never opens a socket.
//!
//! Protocol reference: <https://www.rfc-editor.org/rfc/rfc8446>

use crate::crypto_rsa::{self, Hash};
use crate::crypto_x25519;
use crate::tls_crypto::{self, ContentType, Opener, Sealer, Suite};
use crate::x509;
use crate::{crypto_ec, crypto_hash, crypto_sha512};
use std::io::{self, ErrorKind, Read, Write};
use std::net::IpAddr;
use std::time::SystemTime;

#[path = "crypto_p256_ecdh.rs"]
mod p256;
#[path = "tls_retry.rs"]
mod retry;
#[path = "tls12.rs"]
mod tls12;
#[path = "tls12_cbc.rs"]
mod tls12_cbc;
#[path = "tls12_crypto.rs"]
mod tls12_crypto;

const MAX_HANDSHAKE_MESSAGE: usize = 1024 * 1024;
const MAX_TRANSCRIPT_BYTES: usize = 2 * 1024 * 1024;
const MAX_HANDSHAKE_RECORDS: usize = 1024;
const MAX_CERTIFICATES: usize = 8;
const MAX_CERTIFICATE_BYTES: usize = 64 * 1024;
const MAX_EXTENSIONS: usize = 64;
const MAX_DUMMY_CCS: usize = 8;
const MAX_POST_MESSAGE: usize = 64 * 1024;
const MAX_TICKETS: usize = 8;
const MAX_KEY_UPDATES: usize = 32;
const MAX_CONTROL_RECORDS: usize = 128;
const X25519_GROUP: u16 = 0x001d;
const P256_GROUP: u16 = 0x0017;
const SIGNATURE_SCHEMES: [u16; 5] = [0x0403, 0x0503, 0x0804, 0x0805, 0x0806];
const OFFERED_SIGNATURE_SCHEMES: [u16; 8] = [
    0x0403, 0x0503, 0x0804, 0x0805, 0x0806, 0x0401, 0x0501, 0x0601,
];
const HELLO_RETRY_RANDOM: [u8; 32] = [
    0xcf, 0x21, 0xad, 0x74, 0xe5, 0x9a, 0x61, 0x11, 0xbe, 0x1d, 0x8c, 0x02, 0x1e, 0x65, 0xb8, 0x91,
    0xc2, 0xa2, 0x11, 0x16, 0x7a, 0xbb, 0x8c, 0x5e, 0x07, 0x9e, 0x09, 0xe2, 0xc8, 0xa8, 0x33, 0x9c,
];

struct Transport<S, C> {
    stream: S,
    check: C,
}

fn transient(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::Interrupted | ErrorKind::WouldBlock | ErrorKind::TimedOut
    )
}

impl<S: Read + Write, C: Fn() -> Result<(), String>> Transport<S, C> {
    fn read_exact(&mut self, mut output: &mut [u8]) -> Result<(), String> {
        while !output.is_empty() {
            (self.check)()?;
            match self.stream.read(output) {
                Ok(0) => {
                    return Err(
                        "TLS transport ended without close_notify (truncated connection).".into(),
                    )
                }
                Ok(count) => output = &mut output[count..],
                Err(error) if transient(&error) => continue,
                Err(error) => return Err(format!("TLS transport read failed: {error}")),
            }
        }
        (self.check)()
    }

    fn read_record(&mut self) -> Result<Vec<u8>, String> {
        let mut header = [0u8; 5];
        self.read_exact(&mut header)?;
        if !matches!(header[0], 20..=23) {
            return Err("Unknown TLS record type.".into());
        }
        let length = usize::from(u16::from_be_bytes([header[3], header[4]]));
        if length > tls_crypto::MAX_CIPHERTEXT_BYTES.max(tls12_crypto::MAX_CIPHERTEXT_BYTES) {
            return Err("TLS record exceeds the supported cipher's size limit.".into());
        }
        let mut record = Vec::new();
        record
            .try_reserve_exact(5 + length)
            .map_err(|_| "Cannot allocate TLS record.")?;
        record.extend_from_slice(&header);
        record.resize(5 + length, 0);
        self.read_exact(&mut record[5..])?;
        Ok(record)
    }

    fn write_all(&mut self, mut bytes: &[u8]) -> Result<(), String> {
        while !bytes.is_empty() {
            (self.check)()?;
            match self.stream.write(bytes) {
                Ok(0) => return Err("TLS transport made no progress while writing.".into()),
                Ok(count) => bytes = &bytes[count..],
                Err(error) if transient(&error) => continue,
                Err(error) => return Err(format!("TLS transport write failed: {error}")),
            }
        }
        (self.check)()
    }

    fn flush(&mut self) -> Result<(), String> {
        loop {
            (self.check)()?;
            match self.stream.flush() {
                Ok(()) => return (self.check)(),
                Err(error) if transient(&error) => continue,
                Err(error) => return Err(format!("TLS transport flush failed: {error}")),
            }
        }
    }
}

struct Bytes<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Bytes<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or("TLS field length overflow.")?;
        let value = self
            .input
            .get(self.offset..end)
            .ok_or("Truncated TLS field.")?;
        self.offset = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        let bytes = self.take(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }
    fn u24(&mut self) -> Result<usize, String> {
        let bytes = self.take(3)?;
        Ok((usize::from(bytes[0]) << 16) | (usize::from(bytes[1]) << 8) | usize::from(bytes[2]))
    }
    fn vector8(&mut self) -> Result<&'a [u8], String> {
        let length = usize::from(self.byte()?);
        self.take(length)
    }
    fn vector16(&mut self) -> Result<&'a [u8], String> {
        let length = usize::from(self.u16()?);
        self.take(length)
    }
    fn vector24(&mut self) -> Result<&'a [u8], String> {
        let length = self.u24()?;
        self.take(length)
    }
    fn empty(&self) -> bool {
        self.offset == self.input.len()
    }
    fn finish(&self) -> Result<(), String> {
        if self.empty() {
            Ok(())
        } else {
            Err("Trailing bytes in TLS structure.".into())
        }
    }
}

fn extensions(bytes: &[u8]) -> Result<Vec<(u16, &[u8])>, String> {
    let mut input = Bytes::new(bytes);
    let mut result = Vec::new();
    while !input.empty() {
        if result.len() >= MAX_EXTENSIONS {
            return Err("Too many TLS extensions.".into());
        }
        let kind = input.u16()?;
        let value = input.vector16()?;
        if result.iter().any(|(existing, _)| *existing == kind) {
            return Err("Duplicate TLS extension.".into());
        }
        result.push((kind, value));
    }
    Ok(result)
}

fn append_u16(output: &mut Vec<u8>, value: usize) {
    output.extend_from_slice(&(value as u16).to_be_bytes());
}

fn append_u24(output: &mut Vec<u8>, value: usize) {
    output.extend_from_slice(&[(value >> 16) as u8, (value >> 8) as u8, value as u8]);
}

fn extension(output: &mut Vec<u8>, kind: u16, bytes: &[u8]) {
    output.extend_from_slice(&kind.to_be_bytes());
    append_u16(output, bytes.len());
    output.extend_from_slice(bytes);
}

fn handshake_message(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut message = Vec::with_capacity(body.len() + 4);
    message.push(kind);
    append_u24(&mut message, body.len());
    message.extend_from_slice(body);
    message
}

fn handshake_body(message: &[u8], expected: u8) -> Result<&[u8], String> {
    let mut input = Bytes::new(message);
    let kind = input.byte()?;
    let body = input.vector24()?;
    input.finish()?;
    if kind == 13 {
        return Err("TLS client-certificate authentication is not implemented.".into());
    }
    if kind != expected {
        return Err(format!(
            "Unexpected TLS handshake message {kind}; expected {expected}."
        ));
    }
    Ok(body)
}

fn dns_name(hostname: &str) -> Result<Option<&str>, String> {
    if hostname.parse::<IpAddr>().is_ok() {
        return Ok(None);
    }
    let name = hostname.strip_suffix('.').unwrap_or(hostname);
    if name.is_empty()
        || name.len() > 253
        || !name.is_ascii()
        || name.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err("TLS requires a valid ASCII DNS hostname or an IP address.".into());
    }
    Ok(Some(name))
}

fn client_hello(
    hostname: &str,
    random: &[u8; 32],
    public_key: &[u8; 32],
) -> Result<Vec<u8>, String> {
    let mut body = Vec::with_capacity(512);
    body.extend_from_slice(&[3, 3]);
    body.extend_from_slice(random);
    body.push(0); // Empty legacy session ID; no resumption or compatibility mode.
    body.extend_from_slice(&[
        0, 18, 0x13, 3, 0x13, 1, 0x13, 2, 0xc0, 0x2b, 0xc0, 0x2f, 0xc0, 0x2c, 0xc0, 0x30, 0xc0,
        0x27, 0xc0, 0x28, 1, 0,
    ]); // TLS 1.3, then TLS 1.2 GCM, then ECDHE-RSA CBC; null compression
    let mut offered = Vec::new();
    if let Some(name) = dns_name(hostname)? {
        let mut server_name = Vec::new();
        append_u16(&mut server_name, name.len() + 3);
        server_name.push(0);
        append_u16(&mut server_name, name.len());
        server_name.extend_from_slice(name.as_bytes());
        extension(&mut offered, 0, &server_name);
    }
    extension(&mut offered, 10, &[0, 4, 0, 0x1d, 0, 0x17]); // X25519, P-256
    extension(&mut offered, 11, &[1, 0]); // uncompressed EC points
    extension(&mut offered, 23, &[]); // TLS 1.2 extended master secret
    extension(&mut offered, 22, &[]); // authenticate CBC ciphertext before decryption
    extension(&mut offered, 0xff01, &[0]); // no previous renegotiation
    let mut signatures = Vec::with_capacity(12);
    append_u16(&mut signatures, OFFERED_SIGNATURE_SCHEMES.len() * 2);
    for scheme in OFFERED_SIGNATURE_SCHEMES {
        signatures.extend_from_slice(&scheme.to_be_bytes());
    }
    extension(&mut offered, 13, &signatures);
    extension(
        &mut offered,
        50,
        &[0, 16, 4, 3, 5, 3, 4, 1, 5, 1, 6, 1, 8, 4, 8, 5, 8, 6],
    );
    extension(&mut offered, 16, b"\x00\x09\x08http/1.1");
    extension(&mut offered, 43, &[4, 3, 4, 3, 3]); // TLS 1.3 and TLS 1.2
    let mut share = Vec::with_capacity(38);
    share.extend_from_slice(&[0, 36, 0, 0x1d, 0, 32]);
    share.extend_from_slice(public_key);
    extension(&mut offered, 51, &share);
    append_u16(&mut body, offered.len());
    body.extend_from_slice(&offered);
    Ok(handshake_message(1, &body))
}

fn server_hello(message: &[u8], expected_group: u16) -> Result<(Suite, Vec<u8>), String> {
    let mut input = Bytes::new(handshake_body(message, 2)?);
    if input.u16()? != 0x0303 {
        return Err("Invalid TLS ServerHello legacy version.".into());
    }
    if input.take(32)? == HELLO_RETRY_RANDOM {
        return Err("A second TLS HelloRetryRequest is not permitted.".into());
    }
    if !input.vector8()?.is_empty() {
        return Err("ServerHello did not echo the empty session ID.".into());
    }
    let suite = Suite::from_wire(input.u16()?)?;
    if input.byte()? != 0 {
        return Err("TLS compression is not permitted.".into());
    }
    let selected = extensions(input.vector16()?)?;
    input.finish()?;
    let mut version = false;
    let mut peer_key = None;
    for (kind, bytes) in selected {
        match kind {
            43 => {
                if bytes != [3, 4] {
                    return Err("Server did not select TLS 1.3.".into());
                }
                version = true;
            }
            51 => {
                let mut share = Bytes::new(bytes);
                if share.u16()? != expected_group {
                    return Err(
                        "ServerHello did not select the offered TLS key share group.".into(),
                    );
                }
                let key = share.vector16()?;
                match expected_group {
                    X25519_GROUP if key.len() == 32 => {}
                    P256_GROUP if key.len() == 65 && key[0] == 4 => {}
                    _ => return Err("Invalid server TLS key share encoding.".into()),
                }
                share.finish()?;
                peer_key = Some(key.to_vec());
            }
            _ => return Err(format!("Unexpected ServerHello extension {kind}.")),
        }
    }
    if !version {
        return Err("ServerHello has no TLS 1.3 supported_versions extension.".into());
    }
    peer_key
        .map(|key| (suite, key))
        .ok_or_else(|| "ServerHello has no TLS key share.".into())
}

fn encrypted_extensions(message: &[u8], sent_sni: bool) -> Result<(), String> {
    let mut input = Bytes::new(handshake_body(message, 8)?);
    let selected = extensions(input.vector16()?)?;
    input.finish()?;
    for (kind, bytes) in selected {
        match kind {
            0 if sent_sni && bytes.is_empty() => {}
            10 => {
                let mut groups = Bytes::new(bytes);
                let names = groups.vector16()?;
                if names.is_empty() || names.len() % 2 != 0 {
                    return Err("Invalid server supported_groups list.".into());
                }
                groups.finish()?;
            }
            16 => {
                let mut alpn = Bytes::new(bytes);
                let mut names = Bytes::new(alpn.vector16()?);
                if names.vector8()? != b"http/1.1" {
                    return Err("TLS server selected an unsupported ALPN protocol.".into());
                }
                names.finish()?;
                alpn.finish()?;
            }
            _ => {
                return Err(format!(
                    "Unsupported or unsolicited EncryptedExtensions entry {kind}."
                ))
            }
        }
    }
    // No ALPN response means HTTP/1.1's ordinary HTTPS default.
    Ok(())
}

fn certificates(message: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut input = Bytes::new(handshake_body(message, 11)?);
    if !input.vector8()?.is_empty() {
        return Err("Unexpected server certificate request context.".into());
    }
    let mut list = Bytes::new(input.vector24()?);
    input.finish()?;
    let mut chain = Vec::new();
    while !list.empty() {
        if chain.len() >= MAX_CERTIFICATES {
            return Err("TLS certificate chain is too long.".into());
        }
        let certificate = list.vector24()?;
        if certificate.is_empty() || certificate.len() > MAX_CERTIFICATE_BYTES {
            return Err("TLS certificate is empty or exceeds 64 KiB.".into());
        }
        if !extensions(list.vector16()?)?.is_empty() {
            return Err("Unsolicited TLS certificate-entry extension.".into());
        }
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(certificate.len())
            .map_err(|_| "Cannot allocate certificate.")?;
        owned.extend_from_slice(certificate);
        chain.push(owned);
    }
    if chain.is_empty() {
        return Err("TLS server supplied no certificate.".into());
    }
    Ok(chain)
}

fn certificate_verify(
    message: &[u8],
    verified: &x509::VerifiedServer,
    transcript_hash: &[u8],
) -> Result<(), String> {
    let mut input = Bytes::new(handshake_body(message, 15)?);
    let scheme = input.u16()?;
    if !SIGNATURE_SCHEMES.contains(&scheme) {
        return Err("Unsupported or unoffered TLS CertificateVerify signature algorithm.".into());
    }
    let signature = input.vector16()?;
    input.finish()?;
    let mut signed = Vec::with_capacity(146);
    signed.extend_from_slice(&[0x20; 64]);
    signed.extend_from_slice(b"TLS 1.3, server CertificateVerify");
    signed.push(0);
    signed.extend_from_slice(transcript_hash);
    // The CertificateVerify signature hash is independent of the selected
    // cipher suite's transcript hash. Curve-specific TLS schemes are exact.
    let result = match (&verified.key, scheme) {
        (x509::PublicKey::Rsa { modulus, exponent }, 0x0804..=0x0806) => {
            let (hash, salt_len) = match scheme {
                0x0804 => (Hash::Sha256, 32),
                0x0805 => (Hash::Sha384, 48),
                _ => (Hash::Sha512, 64),
            };
            crypto_rsa::verify_pss(modulus, exponent, hash, salt_len, &signed, signature)
        }
        (
            x509::PublicKey::Ec {
                curve: crypto_ec::Curve::P256,
                point,
            },
            0x0403,
        ) => crypto_ec::verify(
            crypto_ec::Curve::P256,
            point,
            &crypto_hash::sha256(&signed),
            signature,
        ),
        (
            x509::PublicKey::Ec {
                curve: crypto_ec::Curve::P384,
                point,
            },
            0x0503,
        ) => crypto_ec::verify(
            crypto_ec::Curve::P384,
            point,
            &crypto_sha512::sha384(&signed),
            signature,
        ),
        _ => Err("TLS signature scheme does not match the verified certificate key.".into()),
    };
    result.map_err(|error| format!("TLS server CertificateVerify failed: {error}"))
}

struct Transcript {
    bytes: Vec<u8>,
}

impl Transcript {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }
    fn add(&mut self, message: &[u8]) -> Result<(), String> {
        self.bytes
            .len()
            .checked_add(message.len())
            .filter(|&n| n <= MAX_TRANSCRIPT_BYTES)
            .ok_or("TLS handshake transcript exceeds 2 MiB.")?;
        self.bytes
            .try_reserve(message.len())
            .map_err(|_| "Cannot allocate TLS transcript.")?;
        self.bytes.extend_from_slice(message);
        Ok(())
    }
    fn digest(&self, suite: Suite) -> Vec<u8> {
        suite.hash(&self.bytes)
    }
}

struct HandshakeBuffer {
    bytes: Vec<u8>,
    start: usize,
    maximum: usize,
}

impl HandshakeBuffer {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            start: 0,
            maximum,
        }
    }
    fn empty(&self) -> bool {
        self.start == self.bytes.len()
    }
    fn push(&mut self, content: &[u8]) -> Result<(), String> {
        if self.start != 0 {
            self.bytes.drain(..self.start);
            self.start = 0;
        }
        if self
            .bytes
            .len()
            .checked_add(content.len())
            .is_none_or(|size| size > self.maximum + 4 + tls_crypto::MAX_CONTENT_BYTES)
        {
            return Err("TLS handshake reassembly buffer limit reached.".into());
        }
        self.bytes
            .try_reserve(content.len())
            .map_err(|_| "Cannot allocate TLS handshake buffer.")?;
        self.bytes.extend_from_slice(content);
        Ok(())
    }
    fn next(&mut self) -> Result<Option<Vec<u8>>, String> {
        let available = &self.bytes[self.start..];
        if available.len() < 4 {
            return Ok(None);
        }
        let size = (usize::from(available[1]) << 16)
            | (usize::from(available[2]) << 8)
            | usize::from(available[3]);
        if size > self.maximum {
            return Err("TLS handshake message exceeds its size limit.".into());
        }
        if available.len() < 4 + size {
            return Ok(None);
        }
        let mut message = Vec::new();
        message
            .try_reserve_exact(4 + size)
            .map_err(|_| "Cannot allocate TLS handshake message.")?;
        message.extend_from_slice(&available[..4 + size]);
        self.start += 4 + size;
        if self.empty() {
            self.bytes.clear();
            self.start = 0;
        }
        Ok(Some(message))
    }
}

fn alert_error(bytes: &[u8]) -> String {
    if bytes.len() != 2 {
        return "Malformed TLS alert.".into();
    }
    let name = match bytes[1] {
        0 => "close_notify",
        10 => "unexpected_message",
        20 => "bad_record_mac",
        22 => "record_overflow",
        40 => "handshake_failure",
        42 => "bad_certificate",
        43 => "unsupported_certificate",
        45 => "certificate_expired",
        46 => "certificate_unknown",
        47 => "illegal_parameter",
        48 => "unknown_ca",
        50 => "decode_error",
        51 => "decrypt_error",
        70 => "protocol_version",
        71 => "insufficient_security",
        80 => "internal_error",
        109 => "missing_extension",
        110 => "unsupported_extension",
        116 => "certificate_required",
        120 => "no_application_protocol",
        _ => "unrecognized alert",
    };
    format!("TLS peer sent {name} ({}).", bytes[1])
}

fn receive_handshake<S: Read + Write, C: Fn() -> Result<(), String>>(
    transport: &mut Transport<S, C>,
    buffer: &mut HandshakeBuffer,
    mut opener: Option<&mut Opener>,
    records: &mut usize,
    dummy_ccs: &mut usize,
) -> Result<Vec<u8>, String> {
    loop {
        if let Some(message) = buffer.next()? {
            return Ok(message);
        }
        *records += 1;
        if *records > MAX_HANDSHAKE_RECORDS {
            return Err("Too many TLS handshake records.".into());
        }
        let record = transport.read_record()?;
        if record[0] == 20 {
            *dummy_ccs += 1;
            if *dummy_ccs > MAX_DUMMY_CCS || record[1..3] != [3, 3] || record[5..] != [1] {
                return Err(
                    "Invalid or excessive TLS compatibility ChangeCipherSpec records.".into(),
                );
            }
            continue;
        }
        if let Some(reader) = opener.as_deref_mut() {
            let opened = reader.open(&record)?;
            match opened.content_type {
                ContentType::Handshake => buffer.push(&opened.content)?,
                ContentType::Alert => return Err(alert_error(&opened.content)),
                ContentType::ApplicationData => {
                    return Err(
                        "TLS application data arrived before server authentication finished."
                            .into(),
                    )
                }
            }
        } else {
            match record[0] {
                22 if record[1..3] == [3, 3]
                    && record.len() > 5
                    && record.len() <= 5 + tls_crypto::MAX_CONTENT_BYTES =>
                {
                    buffer.push(&record[5..])?
                }
                21 => return Err(alert_error(&record[5..])),
                _ => return Err("Expected a plaintext TLS ServerHello record.".into()),
            }
        }
    }
}

fn ignore_ticket(message: &[u8]) -> Result<(), String> {
    let mut input = Bytes::new(handshake_body(message, 4)?);
    input.take(8)?; // lifetime and age_add; neither is retained or acted upon
    input.vector8()?; // ticket nonce
    if input.vector16()?.is_empty() {
        return Err("Empty TLS session ticket.".into());
    }
    extensions(input.vector16()?)?;
    input.finish()
}

fn selected_version(message: &[u8]) -> Result<u16, String> {
    let mut input = Bytes::new(handshake_body(message, 2)?);
    if input.u16()? != 0x0303 {
        return Err(
            "The server selected a TLS version below 1.2 or an invalid legacy version.".into(),
        );
    }
    let retry = input.take(32)? == HELLO_RETRY_RANDOM;
    if input.vector8()?.len() > 32 {
        return Err("TLS ServerHello session ID exceeds 32 bytes.".into());
    }
    input.u16()?;
    if input.byte()? != 0 {
        return Err("TLS compression is not permitted.".into());
    }
    let entries = if input.empty() {
        Vec::new()
    } else {
        extensions(input.vector16()?)?
    };
    input.finish()?;
    match entries.iter().find(|(kind, _)| *kind == 43) {
        Some((_, [3, 4])) => Ok(0x0304),
        Some(_) => {
            Err("ServerHello selected an invalid or unoffered supported_versions value.".into())
        }
        None if retry => {
            Err("HelloRetryRequest has no TLS 1.3 supported_versions extension.".into())
        }
        None => Ok(0x0303),
    }
}

fn tls12_p256_secret(entropy: &[u8; 32]) -> Result<[u8; 32], String> {
    p256_secret(entropy, b"OwnBrowser TLS 1.2 P-256 ephemeral key")
}

fn p256_secret(entropy: &[u8; 32], context: &[u8]) -> Result<[u8; 32], String> {
    // Domain separation avoids exposing one private scalar through two curve
    // systems. Rejection avoids modulo bias in the HMAC-derived candidate.
    const ORDER: [u8; 32] = [
        0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84, 0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63,
        0x25, 0x51,
    ];
    for counter in 0..16u8 {
        let mut label = context.to_vec();
        label.push(counter);
        let candidate = crypto_hash::hmac_sha256(entropy, &label);
        let mut borrow = 0u16;
        let mut nonzero = 0u8;
        for index in (0..32).rev() {
            let difference = 256 + u16::from(candidate[index]) - u16::from(ORDER[index]) - borrow;
            borrow = 1 ^ (difference >> 8);
            nonzero |= candidate[index];
        }
        if borrow == 1 && nonzero != 0 {
            return Ok(candidate);
        }
    }
    Err("Could not derive a valid P-256 ephemeral scalar from fresh entropy.".into())
}

enum ProtocolStream<S, C> {
    Tls13(Box<Tls13Stream<S, C>>),
    Tls12(Box<tls12::Connection<S, C>>),
}

/// An authenticated connection selected by one TLS 1.3/1.2 ClientHello.
/// No HTTP bytes can be written until certificate, signed handshake and
/// server Finished verification all succeed. Failed handshakes never reconnect.
pub struct TlsStream<S, C> {
    inner: ProtocolStream<S, C>,
}

impl<S: Read + Write, C: Fn() -> Result<(), String>> TlsStream<S, C> {
    /// The caller supplies 64 fresh OS-random bytes: ClientHello random plus
    /// X25519 private entropy. TLS 1.2/1.3 P-256 derive separate scalars from it.
    pub fn connect(
        stream: S,
        hostname: &str,
        roots: &[Vec<u8>],
        now: SystemTime,
        random: [u8; 64],
        check: C,
    ) -> Result<Self, String> {
        check()?;
        if roots.is_empty() || roots.len() > 128 {
            return Err(
                "TLS requires between 1 and 128 explicit trusted root certificates.".into(),
            );
        }
        let verified_name = dns_name(hostname)?.unwrap_or(hostname);
        let mut client_random = [0; 32];
        client_random.copy_from_slice(&random[..32]);
        let mut private_key = [0; 32];
        private_key.copy_from_slice(&random[32..]);
        let public_key = crypto_x25519::public_key(&private_key);
        let hello = client_hello(verified_name, &client_random, &public_key)?;
        let mut transport = Transport { stream, check };
        let mut first_record = vec![22, 3, 1];
        append_u16(&mut first_record, hello.len());
        first_record.extend_from_slice(&hello);
        transport.write_all(&first_record)?;
        transport.flush()?;
        let mut buffer = HandshakeBuffer::new(MAX_HANDSHAKE_MESSAGE);
        let mut records = 0;
        let mut dummy_ccs = 0;
        let server = receive_handshake(
            &mut transport,
            &mut buffer,
            None,
            &mut records,
            &mut dummy_ccs,
        )
        .map_err(|error| format!("TLS handshake before ServerHello: {error}"))?;
        let inner = match selected_version(&server)? {
            0x0304 => ProtocolStream::Tls13(Box::new(Tls13Stream::finish_handshake(
                transport,
                hello,
                buffer,
                records,
                dummy_ccs,
                server,
                private_key,
                verified_name,
                roots,
                now,
            )?)),
            _ => {
                if dummy_ccs != 0 {
                    return Err("TLS 1.2 received ChangeCipherSpec before ServerHello.".into());
                }
                ProtocolStream::Tls12(Box::new(tls12::Connection::connect(
                    transport,
                    hello,
                    server,
                    buffer,
                    records,
                    verified_name,
                    roots,
                    now,
                    client_random,
                    private_key,
                )?))
            }
        };
        Ok(Self { inner })
    }

    pub fn protocol(&self) -> &'static str {
        match self.inner {
            ProtocolStream::Tls13(_) => "TLS 1.3",
            ProtocolStream::Tls12(_) => "TLS 1.2",
        }
    }

    pub fn certificate_summary(&self) -> &str {
        match &self.inner {
            ProtocolStream::Tls13(stream) => stream.certificate_summary(),
            ProtocolStream::Tls12(stream) => stream.certificate_summary(),
        }
    }

    /// HTTP may retain this exact authenticated record stream only when no
    /// application bytes remain buffered and neither direction has failed or
    /// closed. Keys, sequence counters and post-handshake state stay intact.
    pub fn reusable(&self) -> bool {
        match &self.inner {
            ProtocolStream::Tls13(stream) => {
                !stream.failed
                    && !stream.read_closed
                    && !stream.write_closed
                    && stream.buffered_at == stream.buffered.len()
                    && stream.post_handshake.empty()
            }
            ProtocolStream::Tls12(stream) => stream.reusable(),
        }
    }

    /// Only an authenticated close_notify counts as a clean peer shutdown.
    pub fn peer_closed(&self) -> bool {
        match &self.inner {
            ProtocolStream::Tls13(stream) => stream.read_closed && !stream.failed,
            ProtocolStream::Tls12(stream) => stream.peer_closed(),
        }
    }

    pub fn get_ref(&self) -> &S {
        match &self.inner {
            ProtocolStream::Tls13(stream) => &stream.transport.stream,
            ProtocolStream::Tls12(stream) => stream.get_ref(),
        }
    }

    pub fn close(&mut self) -> io::Result<()> {
        match &mut self.inner {
            ProtocolStream::Tls13(stream) => stream.close(),
            ProtocolStream::Tls12(stream) => stream.close(),
        }
    }
}

impl<S: Read + Write, C: Fn() -> Result<(), String>> Read for TlsStream<S, C> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        match &mut self.inner {
            ProtocolStream::Tls13(stream) => stream.read(output),
            ProtocolStream::Tls12(stream) => stream.read(output),
        }
    }
}

impl<S: Read + Write, C: Fn() -> Result<(), String>> Write for TlsStream<S, C> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        match &mut self.inner {
            ProtocolStream::Tls13(stream) => stream.write(input),
            ProtocolStream::Tls12(stream) => stream.write(input),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match &mut self.inner {
            ProtocolStream::Tls13(stream) => stream.flush(),
            ProtocolStream::Tls12(stream) => stream.flush(),
        }
    }
}

/// An authenticated TLS 1.3 client connection. Construction only succeeds after
/// the trust chain, hostname, CertificateVerify, and server Finished all pass,
/// and after the client Finished is written. No Clone/Debug or insecure bypass.
struct Tls13Stream<S, C> {
    transport: Transport<S, C>,
    opener: Opener,
    sealer: Sealer,
    suite: Suite,
    read_secret: Vec<u8>,
    write_secret: Vec<u8>,
    write_records: u64,
    post_handshake: HandshakeBuffer,
    tickets: usize,
    read_updates: usize,
    write_updates: usize,
    buffered: Vec<u8>,
    buffered_at: usize,
    read_closed: bool,
    write_closed: bool,
    failed: bool,
    certificate_summary: String,
}

impl<S: Read + Write, C: Fn() -> Result<(), String>> Tls13Stream<S, C> {
    #[allow(clippy::too_many_arguments)]
    fn finish_handshake(
        mut transport: Transport<S, C>,
        hello: Vec<u8>,
        mut buffer: HandshakeBuffer,
        mut records: usize,
        mut dummy_ccs: usize,
        mut server: Vec<u8>,
        private_key: [u8; 32],
        hostname: &str,
        roots: &[Vec<u8>],
        now: SystemTime,
    ) -> Result<Self, String> {
        let name = dns_name(hostname)?;
        let verified_name = name.unwrap_or(hostname);
        let mut transcript = Transcript::new();
        let mut client_key = retry::ClientKey::X25519(private_key);
        let mut retry_suite = None;
        if let Some(request) = retry::Request::parse(&server)? {
            if !buffer.empty() {
                return Err("HelloRetryRequest must finish at a TLS record boundary.".into());
            }
            (transport.check)()?;
            let (second, key) = request.client_hello(&hello, &private_key)?;
            (transport.check)()?;
            transcript = request.transcript(&hello, &server)?;
            transcript.add(&second)?;
            retry_suite = Some(request.suite);
            client_key = key;
            let mut record = vec![22, 3, 3];
            append_u16(&mut record, second.len());
            record.extend_from_slice(&second);
            transport.write_all(&record)?;
            transport.flush()?;
            // Continue this connection under the same deadline and counters.
            // There is no retry loop or TLS 1.2 branch after this point.
            server = receive_handshake(
                &mut transport,
                &mut buffer,
                None,
                &mut records,
                &mut dummy_ccs,
            )
            .map_err(|error| format!("TLS handshake after HelloRetryRequest: {error}"))?;
        } else {
            transcript.add(&hello)?;
        }
        let (suite, peer_key) = server_hello(&server, client_key.group())?;
        if retry_suite.is_some_and(|selected| selected != suite) {
            return Err("ServerHello changed the HelloRetryRequest cipher suite.".into());
        }
        if !buffer.empty() {
            return Err("ServerHello must finish at a TLS record boundary.".into());
        }
        transcript.add(&server)?;
        (transport.check)()?;
        let shared = client_key.shared_secret(&peer_key)?;
        (transport.check)()?;
        let handshake =
            tls_crypto::handshake_secret(suite, &tls_crypto::early_secret(suite), &shared)?;
        let hello_hash = transcript.digest(suite);
        let server_handshake =
            tls_crypto::derive_secret(suite, &handshake, "s hs traffic", &hello_hash)?;
        let client_handshake =
            tls_crypto::derive_secret(suite, &handshake, "c hs traffic", &hello_hash)?;
        let mut handshake_reader = Opener::new(suite, &server_handshake)?;
        let mut handshake_writer = Sealer::new(suite, &client_handshake)?;

        let parameters = receive_handshake(
            &mut transport,
            &mut buffer,
            Some(&mut handshake_reader),
            &mut records,
            &mut dummy_ccs,
        )?;
        encrypted_extensions(&parameters, name.is_some())?;
        transcript.add(&parameters)?;

        let certificate = receive_handshake(
            &mut transport,
            &mut buffer,
            Some(&mut handshake_reader),
            &mut records,
            &mut dummy_ccs,
        )?;
        let chain = certificates(&certificate)?;
        (transport.check)()?;
        let verified = x509::verify_server(&chain, roots, verified_name, now)
            .map_err(|error| format!("TLS certificate verification failed: {error}"))?;
        (transport.check)()?;
        transcript.add(&certificate)?;

        let proof = receive_handshake(
            &mut transport,
            &mut buffer,
            Some(&mut handshake_reader),
            &mut records,
            &mut dummy_ccs,
        )?;
        certificate_verify(&proof, &verified, &transcript.digest(suite))?;
        (transport.check)()?;
        transcript.add(&proof)?;

        let finished = receive_handshake(
            &mut transport,
            &mut buffer,
            Some(&mut handshake_reader),
            &mut records,
            &mut dummy_ccs,
        )?;
        let verify_data = handshake_body(&finished, 20)?;
        tls_crypto::verify_finished(
            suite,
            &server_handshake,
            &transcript.digest(suite),
            verify_data,
        )?;
        if !buffer.empty() {
            return Err("Server Finished must finish at a TLS record boundary.".into());
        }
        transcript.add(&finished)?;

        // Application secrets use the transcript THROUGH server Finished,
        // before the client's Finished is appended. No application state is
        // returned until the server's three independent authentication checks
        // above and the client Finished write below have succeeded.
        let main = tls_crypto::main_secret(suite, &handshake)?;
        let application_hash = transcript.digest(suite);
        let read_secret =
            tls_crypto::derive_secret(suite, &main, "s ap traffic", &application_hash)?;
        let write_secret =
            tls_crypto::derive_secret(suite, &main, "c ap traffic", &application_hash)?;
        let our_finished = handshake_message(
            20,
            &tls_crypto::finished_verify_data(suite, &client_handshake, &application_hash)?,
        );
        let encrypted = handshake_writer.seal(ContentType::Handshake, &our_finished, 0)?;
        transport.write_all(&encrypted)?;
        transport.flush()?;
        transcript.add(&our_finished)?;
        (transport.check)()?;

        Ok(Self {
            transport,
            opener: Opener::new(suite, &read_secret)?,
            sealer: Sealer::new(suite, &write_secret)?,
            suite,
            read_secret,
            write_secret,
            write_records: 0,
            post_handshake: HandshakeBuffer::new(MAX_POST_MESSAGE),
            tickets: 0,
            read_updates: 0,
            write_updates: 0,
            buffered: Vec::new(),
            buffered_at: 0,
            read_closed: false,
            write_closed: false,
            failed: false,
            certificate_summary: format!(
                "TLS 1.3, {}, {}; {}",
                client_key.name(),
                suite.name(),
                verified.summary
            ),
        })
    }

    pub fn certificate_summary(&self) -> &str {
        &self.certificate_summary
    }

    fn ensure_live(&self) -> Result<(), String> {
        if self.failed {
            return Err("TLS connection is unusable after an earlier failure.".into());
        }
        (self.transport.check)()
    }

    fn io_result<T>(&mut self, result: Result<T, String>) -> io::Result<T> {
        result.map_err(|error| {
            self.failed = true;
            io::Error::other(error)
        })
    }

    fn send_protected(&mut self, kind: ContentType, bytes: &[u8]) -> Result<(), String> {
        if self.write_closed {
            return Err("TLS sending direction is already closed.".into());
        }
        let record = self.sealer.seal(kind, bytes, 0)?;
        self.write_records += 1;
        self.transport.write_all(&record)
    }

    fn update_write_key(&mut self) -> Result<(), String> {
        self.write_updates += 1;
        if self.write_updates > MAX_KEY_UPDATES {
            return Err("TLS write key-update limit reached.".into());
        }
        // KeyUpdate itself uses the OLD key. Subsequent records start with
        // sequence zero under the NEXT secret. Never reset an existing key.
        let next = tls_crypto::next_traffic_secret(self.suite, &self.write_secret)?;
        let replacement = Sealer::new(self.suite, &next)?;
        self.send_protected(ContentType::Handshake, &[24, 0, 0, 1, 0])?;
        self.transport.flush()?;
        self.write_secret = next;
        self.sealer = replacement;
        self.write_records = 0;
        Ok(())
    }

    fn post_messages(&mut self) -> Result<(), String> {
        while let Some(message) = self.post_handshake.next()? {
            match message[0] {
                4 => {
                    self.tickets += 1;
                    if self.tickets > MAX_TICKETS {
                        return Err("Too many TLS session tickets.".into());
                    }
                    ignore_ticket(&message)?; // Parsed and discarded, never cached.
                }
                24 => {
                    let body = handshake_body(&message, 24)?;
                    if body.len() != 1 || body[0] > 1 {
                        return Err("Invalid TLS KeyUpdate request.".into());
                    }
                    if !self.post_handshake.empty() {
                        return Err("TLS KeyUpdate must end its record.".into());
                    }
                    self.read_updates += 1;
                    if self.read_updates > MAX_KEY_UPDATES {
                        return Err("TLS read key-update limit reached.".into());
                    }
                    let next = tls_crypto::next_traffic_secret(self.suite, &self.read_secret)?;
                    self.opener = Opener::new(self.suite, &next)?;
                    self.read_secret = next;
                    if body[0] == 1 {
                        self.update_write_key()?;
                    }
                }
                _ => {
                    return Err(format!(
                        "Unsupported TLS post-handshake message {}.",
                        message[0]
                    ))
                }
            }
        }
        Ok(())
    }

    fn fill_buffer(&mut self) -> Result<(), String> {
        for _ in 0..MAX_CONTROL_RECORDS {
            self.ensure_live()?;
            let record = self.transport.read_record()?;
            let opened = self.opener.open(&record)?;
            match opened.content_type {
                ContentType::Handshake => {
                    self.post_handshake.push(&opened.content)?;
                    self.post_messages()?;
                }
                ContentType::ApplicationData => {
                    if !self.post_handshake.empty() {
                        return Err(
                            "TLS application data interrupted a fragmented handshake message."
                                .into(),
                        );
                    }
                    if !opened.content.is_empty() {
                        self.buffered = opened.content;
                        self.buffered_at = 0;
                        return Ok(());
                    }
                }
                ContentType::Alert => {
                    if !self.post_handshake.empty() {
                        return Err("TLS alert interrupted a fragmented handshake message.".into());
                    }
                    if opened.content[1] == 0 {
                        self.read_closed = true;
                        return Ok(());
                    }
                    return Err(alert_error(&opened.content));
                }
            }
        }
        Err("Too many TLS control or empty records without application data.".into())
    }

    fn read_inner(&mut self, output: &mut [u8]) -> Result<usize, String> {
        if output.is_empty() {
            return Ok(0);
        }
        self.ensure_live()?;
        if self.buffered_at == self.buffered.len() {
            self.buffered.clear();
            self.buffered_at = 0;
            if self.read_closed {
                return Ok(0);
            }
            self.fill_buffer()?;
        }
        self.ensure_live()?;
        let count = output.len().min(self.buffered.len() - self.buffered_at);
        output[..count].copy_from_slice(&self.buffered[self.buffered_at..self.buffered_at + count]);
        self.buffered_at += count;
        Ok(count)
    }

    fn write_inner(&mut self, input: &[u8]) -> Result<usize, String> {
        if input.is_empty() {
            return Ok(0);
        }
        self.ensure_live()?;
        if self.write_records >= tls_crypto::MAX_RECORDS_PER_KEY - 1 {
            self.update_write_key()?;
        }
        let count = input.len().min(tls_crypto::MAX_CONTENT_BYTES);
        self.send_protected(ContentType::ApplicationData, &input[..count])?;
        Ok(count)
    }

    /// Close this sending direction cleanly. Dropping does not do blocking I/O.
    /// Reads may continue until the peer's own authenticated close_notify.
    pub fn close(&mut self) -> io::Result<()> {
        let result = (|| {
            self.ensure_live()?;
            if self.write_closed {
                return Ok(());
            }
            if self.write_records >= tls_crypto::MAX_RECORDS_PER_KEY - 1 {
                self.update_write_key()?;
            }
            self.send_protected(ContentType::Alert, &[1, 0])?;
            self.transport.flush()?;
            self.write_closed = true;
            Ok(())
        })();
        self.io_result(result)
    }
}

impl<S: Read + Write, C: Fn() -> Result<(), String>> Read for Tls13Stream<S, C> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let result = self.read_inner(output);
        self.io_result(result)
    }
}

impl<S: Read + Write, C: Fn() -> Result<(), String>> Write for Tls13Stream<S, C> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let result = self.write_inner(input);
        self.io_result(result)
    }
    fn flush(&mut self) -> io::Result<()> {
        let result = self.ensure_live().and_then(|()| self.transport.flush());
        self.io_result(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::io::Cursor;

    fn permitted() -> Result<(), String> {
        Ok(())
    }

    struct MemoryIo {
        input: Cursor<Vec<u8>>,
        written: Vec<u8>,
        chunk: usize,
        read_timeout: bool,
        write_timeout: bool,
    }

    impl MemoryIo {
        fn new(input: Vec<u8>) -> Self {
            Self {
                input: Cursor::new(input),
                written: Vec::new(),
                chunk: 3,
                read_timeout: false,
                write_timeout: false,
            }
        }
    }

    impl Read for MemoryIo {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            if self.read_timeout {
                self.read_timeout = false;
                return Err(ErrorKind::TimedOut.into());
            }
            let length = bytes.len().min(self.chunk);
            self.input.read(&mut bytes[..length])
        }
    }
    impl Write for MemoryIo {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.write_timeout {
                self.write_timeout = false;
                return Err(ErrorKind::WouldBlock.into());
            }
            let length = bytes.len().min(self.chunk);
            self.written.extend_from_slice(&bytes[..length]);
            Ok(length)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn plain_record(kind: u8, payload: &[u8]) -> Vec<u8> {
        let mut record = vec![kind, 3, 3];
        append_u16(&mut record, payload.len());
        record.extend_from_slice(payload);
        record
    }

    fn hello_with_extensions(extra: &[u8], random: [u8; 32]) -> Vec<u8> {
        let mut body = vec![3, 3];
        body.extend_from_slice(&random);
        body.extend_from_slice(&[0, 0x13, 3, 0]);
        append_u16(&mut body, extra.len());
        body.extend_from_slice(extra);
        handshake_message(2, &body)
    }

    fn normal_server_extensions() -> Vec<u8> {
        let mut selected = Vec::new();
        extension(&mut selected, 43, &[3, 4]);
        let mut key = vec![0, 0x1d, 0, 32];
        key.extend_from_slice(&[0x51; 32]);
        extension(&mut selected, 51, &key);
        selected
    }

    #[test]
    fn client_offer_and_server_selection_are_narrow_and_exact() {
        for (host, sni) in [("example.test", true), ("127.0.0.1", false), ("::1", false)] {
            let message = client_hello(host, &[0x31; 32], &[0x42; 32]).unwrap();
            let mut input = Bytes::new(handshake_body(&message, 1).unwrap());
            assert_eq!(input.u16().unwrap(), 0x0303);
            assert_eq!(input.take(32).unwrap(), [0x31; 32]);
            assert!(input.vector8().unwrap().is_empty());
            assert_eq!(
                input.vector16().unwrap(),
                [
                    0x13, 3, 0x13, 1, 0x13, 2, 0xc0, 0x2b, 0xc0, 0x2f, 0xc0, 0x2c, 0xc0, 0x30,
                    0xc0, 0x27, 0xc0, 0x28
                ]
            );
            assert_eq!(input.vector8().unwrap(), [0]);
            let offered = extensions(input.vector16().unwrap()).unwrap();
            input.finish().unwrap();
            assert_eq!(offered.iter().any(|(kind, _)| *kind == 0), sni);
            assert_eq!(
                offered.iter().find(|(kind, _)| *kind == 43).unwrap().1,
                [4, 3, 4, 3, 3]
            );
            assert_eq!(
                offered.iter().find(|(kind, _)| *kind == 13).unwrap().1,
                [0, 16, 4, 3, 5, 3, 8, 4, 8, 5, 8, 6, 4, 1, 5, 1, 6, 1]
            );
            assert_eq!(
                offered.iter().find(|(kind, _)| *kind == 16).unwrap().1,
                b"\0\x09\x08http/1.1"
            );
            assert!(!offered.iter().any(|(kind, _)| matches!(kind, 41 | 42 | 45)));
            assert_eq!(offered.iter().find(|(kind, _)| *kind == 22).unwrap().1, []);
            assert_eq!(offered.iter().find(|(kind, _)| *kind == 23).unwrap().1, []);
            assert_eq!(
                offered.iter().find(|(kind, _)| *kind == 0xff01).unwrap().1,
                [0]
            );
        }
        assert_eq!(dns_name("example.test.").unwrap(), Some("example.test"));
        for bad in [
            "",
            "bad\r\nhost",
            "-example.test",
            "bad..test",
            "\u{00e9}.test",
        ] {
            assert!(client_hello(bad, &[0; 32], &[0; 32]).is_err());
        }
        let extra = normal_server_extensions();
        let hello = hello_with_extensions(&extra, [0x52; 32]);
        assert_eq!(
            server_hello(&hello, X25519_GROUP).unwrap(),
            (Suite::ChaCha20Poly1305Sha256, vec![0x51; 32])
        );
        assert!(server_hello(
            &hello_with_extensions(&extra, HELLO_RETRY_RANDOM),
            X25519_GROUP
        )
        .unwrap_err()
        .contains("HelloRetryRequest"));
        let mut duplicate = extra.clone();
        extension(&mut duplicate, 43, &[3, 4]);
        assert!(server_hello(&hello_with_extensions(&duplicate, [0; 32]), X25519_GROUP).is_err());
        let mut unsolicited = extra.clone();
        extension(&mut unsolicited, 41, &[0, 0]);
        assert!(server_hello(&hello_with_extensions(&unsolicited, [0; 32]), X25519_GROUP).is_err());
        for offset in [4, 5, 38, 39, 41] {
            let mut changed = hello.clone();
            changed[offset] ^= 1;
            assert!(
                server_hello(&changed, X25519_GROUP).is_err(),
                "offset {offset}"
            );
        }
        for length in 0..hello.len() {
            assert!(server_hello(&hello[..length], X25519_GROUP).is_err());
        }
    }

    #[test]
    fn hello_retry_never_repeats_or_changes_negotiated_parameters() {
        let mut retry_extra = Vec::new();
        extension(&mut retry_extra, 43, &[3, 4]);
        extension(&mut retry_extra, 44, &[0, 1, 0x51]);
        let retry = hello_with_extensions(&retry_extra, HELLO_RETRY_RANDOM);
        let normal = hello_with_extensions(&normal_server_extensions(), [0x52; 32]);
        let mut changed_suite = normal.clone();
        changed_suite[40] = 1;
        let mut changed_version_extra = Vec::new();
        extension(&mut changed_version_extra, 43, &[3, 3]);
        let changed_version = hello_with_extensions(&changed_version_extra, [0x52; 32]);
        let no_version = hello_with_extensions(&[], [0x52; 32]);
        let mut changed_group_extra = Vec::new();
        extension(&mut changed_group_extra, 43, &[3, 4]);
        extension(&mut changed_group_extra, 51, &[0, 0x17, 0, 1, 4]);
        let changed_group = hello_with_extensions(&changed_group_extra, [0x52; 32]);
        for (second, expected) in [
            (retry.clone(), "second TLS HelloRetryRequest"),
            (changed_suite, "changed the HelloRetryRequest cipher suite"),
            (changed_version, "did not select TLS 1.3"),
            (no_version, "no TLS 1.3"),
            (changed_group, "offered TLS key share group"),
        ] {
            let mut io =
                MemoryIo::new([plain_record(22, &retry), plain_record(22, &second)].concat());
            let error = TlsStream::connect(
                &mut io,
                "example.test",
                &[vec![0]],
                SystemTime::now(),
                [0x42; 64],
                permitted,
            )
            .err()
            .expect("retry must fail before authentication");
            assert!(error.contains(expected), "{error}; expected {expected}");
            let first_length = usize::from(u16::from_be_bytes([io.written[3], io.written[4]]));
            let second_start = 5 + first_length;
            assert_eq!(&io.written[second_start..second_start + 3], &[22, 3, 3]);
            let second_length = usize::from(u16::from_be_bytes([
                io.written[second_start + 3],
                io.written[second_start + 4],
            ]));
            assert_eq!(io.written.len(), second_start + 5 + second_length);
            assert_eq!(io.written[5], 1);
            assert_eq!(io.written[second_start + 5], 1); // Two ClientHellos, no Finished/HTTP.
        }
    }

    #[test]
    fn hello_retry_preserves_record_ccs_and_cancellation_limits() {
        let mut extra = Vec::new();
        extension(&mut extra, 43, &[3, 4]);
        extension(&mut extra, 44, &[0, 1, 0x52]);
        let retry = hello_with_extensions(&extra, HELLO_RETRY_RANDOM);
        let first = client_hello("example.test", &[0x31; 32], &[0x42; 32]).unwrap();
        for (records, ccs, incoming, expected) in [
            (
                MAX_HANDSHAKE_RECORDS,
                0,
                Vec::new(),
                "Too many TLS handshake records",
            ),
            (
                1,
                MAX_DUMMY_CCS,
                plain_record(20, &[1]),
                "excessive TLS compatibility",
            ),
        ] {
            let mut io = MemoryIo::new(incoming);
            let error = Tls13Stream::finish_handshake(
                Transport {
                    stream: &mut io,
                    check: permitted,
                },
                first.clone(),
                HandshakeBuffer::new(MAX_HANDSHAKE_MESSAGE),
                records,
                ccs,
                retry.clone(),
                [0x42; 32],
                "example.test",
                &[vec![0]],
                SystemTime::now(),
            )
            .err()
            .expect("counter must not reset for retry");
            assert!(error.contains(expected), "{error}");
        }
        let mut io = MemoryIo::new(Vec::new());
        let mut buffered = HandshakeBuffer::new(MAX_HANDSHAKE_MESSAGE);
        buffered.push(&[2]).unwrap();
        let error = Tls13Stream::finish_handshake(
            Transport {
                stream: &mut io,
                check: permitted,
            },
            first.clone(),
            buffered,
            1,
            0,
            retry.clone(),
            [0x42; 32],
            "example.test",
            &[vec![0]],
            SystemTime::now(),
        )
        .err()
        .unwrap();
        assert!(error.contains("record boundary"));
        assert!(io.written.is_empty());
        let error = Tls13Stream::finish_handshake(
            Transport {
                stream: &mut io,
                check: || Err("cancelled retry".into()),
            },
            first,
            HandshakeBuffer::new(MAX_HANDSHAKE_MESSAGE),
            1,
            0,
            retry,
            [0x42; 32],
            "example.test",
            &[vec![0]],
            SystemTime::now(),
        )
        .err()
        .unwrap();
        assert_eq!(error, "cancelled retry");
        assert!(io.written.is_empty());
    }

    #[test]
    fn encrypted_extensions_validate_alpn_and_unsolicited_values() {
        let message = |extra: &[u8]| {
            let mut body = Vec::new();
            append_u16(&mut body, extra.len());
            body.extend_from_slice(extra);
            handshake_message(8, &body)
        };
        encrypted_extensions(&message(&[]), false).unwrap(); // ALPN absence is accepted.
        let mut offered = Vec::new();
        extension(&mut offered, 16, b"\0\x09\x08http/1.1");
        extension(&mut offered, 0, &[]);
        encrypted_extensions(&message(&offered), true).unwrap();
        assert!(encrypted_extensions(&message(&offered), false).is_err());
        let mut h2 = Vec::new();
        extension(&mut h2, 16, b"\0\x03\x02h2");
        assert!(encrypted_extensions(&message(&h2), false).is_err());
        let mut unknown = Vec::new();
        extension(&mut unknown, 42, &[]);
        assert!(encrypted_extensions(&message(&unknown), false).is_err());
        let mut duplicate = offered.clone();
        extension(&mut duplicate, 16, b"\0\x09\x08http/1.1");
        assert!(encrypted_extensions(&message(&duplicate), true).is_err());
    }

    #[test]
    fn certificate_message_framing_is_bounded_before_x509() {
        let message = |count: usize, certificate: &[u8], context: &[u8], extra: &[u8]| {
            let mut list = Vec::new();
            for _ in 0..count {
                append_u24(&mut list, certificate.len());
                list.extend_from_slice(certificate);
                append_u16(&mut list, extra.len());
                list.extend_from_slice(extra);
            }
            let mut body = vec![context.len() as u8];
            body.extend_from_slice(context);
            append_u24(&mut body, list.len());
            body.extend_from_slice(&list);
            handshake_message(11, &body)
        };
        // Framing alone does not validate these deliberately non-DER bytes.
        assert_eq!(
            certificates(&message(1, b"DER", &[], &[])).unwrap(),
            vec![b"DER".to_vec()]
        );
        assert!(certificates(&message(0, b"", &[], &[])).is_err());
        assert!(certificates(&message(1, b"", &[], &[])).is_err());
        assert!(certificates(&message(9, b"DER", &[], &[])).is_err());
        assert!(certificates(&message(1, &vec![0; MAX_CERTIFICATE_BYTES + 1], &[], &[])).is_err());
        assert!(certificates(&message(1, b"DER", b"x", &[])).is_err());
        assert!(certificates(&message(1, b"DER", &[], &[0, 5, 0, 0])).is_err());
    }

    #[test]
    fn handshake_reassembly_preserves_message_bytes_and_limits() {
        let first = handshake_message(8, &[0, 0]);
        let second = handshake_message(11, b"certificate payload");
        let joined = [first.clone(), second.clone()].concat();
        for chunk in [1, 2, 3, 4, 7, joined.len()] {
            let mut buffer = HandshakeBuffer::new(64);
            let mut messages = Vec::new();
            for bytes in joined.chunks(chunk) {
                buffer.push(bytes).unwrap();
                while let Some(message) = buffer.next().unwrap() {
                    messages.push(message);
                }
            }
            assert_eq!(messages, [first.clone(), second.clone()]);
            assert!(buffer.empty());
        }
        let mut overlong = HandshakeBuffer::new(32);
        overlong.push(&[11, 0, 0, 33]).unwrap();
        assert!(overlong.next().is_err());
        let mut transcript = Transcript::new();
        transcript.add(&[1, 0, 0, 0]).unwrap();
        let saved = transcript.digest(Suite::ChaCha20Poly1305Sha256);
        assert!(transcript.add(&vec![0; MAX_TRANSCRIPT_BYTES]).is_err());
        assert_eq!(transcript.digest(Suite::ChaCha20Poly1305Sha256), saved);
    }

    #[test]
    fn transport_handles_short_io_timeout_retry_cancellation_and_eof() {
        let record = plain_record(22, &[1, 2, 3, 4, 5]);
        let mut memory = MemoryIo::new(record.clone());
        memory.read_timeout = true;
        memory.write_timeout = true;
        let calls = Cell::new(0);
        let mut transport = Transport {
            stream: memory,
            check: || {
                calls.set(calls.get() + 1);
                Ok(())
            },
        };
        assert_eq!(transport.read_record().unwrap(), record);
        transport.write_all(b"abcdefghij").unwrap();
        assert_eq!(transport.stream.written, b"abcdefghij");
        assert!(calls.get() > 5);
        assert!(transport
            .read_record()
            .unwrap_err()
            .contains("close_notify"));
        for length in 0..record.len() {
            let mut truncated = Transport {
                stream: MemoryIo::new(record[..length].to_vec()),
                check: permitted,
            };
            assert!(truncated.read_record().is_err());
        }
        let mut cancelled = Transport {
            stream: MemoryIo::new(record),
            check: || Err("cancelled".into()),
        };
        assert_eq!(cancelled.read_record().unwrap_err(), "cancelled");
        assert_eq!(cancelled.stream.input.position(), 0);
        assert!(cancelled.write_all(b"request").is_err());
        assert!(cancelled.stream.written.is_empty());
        let attempts = Cell::new(0);
        let mut midway = Transport {
            stream: MemoryIo::new(plain_record(22, b"data")),
            check: || {
                attempts.set(attempts.get() + 1);
                if attempts.get() == 2 {
                    Err("cancelled midway".into())
                } else {
                    Ok(())
                }
            },
        };
        assert_eq!(midway.read_record().unwrap_err(), "cancelled midway");
        assert_eq!(midway.stream.input.position(), 3);
    }

    #[test]
    fn application_data_before_finished_never_becomes_a_handshake_message() {
        let secret = [0x33; 32];
        let mut writer = Sealer::new(Suite::ChaCha20Poly1305Sha256, &secret).unwrap();
        let record = writer
            .seal(ContentType::ApplicationData, b"untrusted HTTP bytes", 0)
            .unwrap();
        let mut transport = Transport {
            stream: MemoryIo::new(record),
            check: permitted,
        };
        let mut buffer = HandshakeBuffer::new(MAX_HANDSHAKE_MESSAGE);
        let mut reader = Opener::new(Suite::ChaCha20Poly1305Sha256, &secret).unwrap();
        let error = receive_handshake(
            &mut transport,
            &mut buffer,
            Some(&mut reader),
            &mut 0,
            &mut 0,
        )
        .unwrap_err();
        assert!(error.contains("before server authentication"));
        assert!(buffer.empty());
    }

    fn established_for_record_tests(
        input: Vec<u8>,
    ) -> Tls13Stream<MemoryIo, fn() -> Result<(), String>> {
        // Unit-test-only construction for record lifecycle tests. The public
        // constructor always performs the complete authenticated handshake.
        Tls13Stream {
            transport: Transport {
                stream: MemoryIo::new(input),
                check: permitted,
            },
            opener: Opener::new(Suite::ChaCha20Poly1305Sha256, &[0x61; 32]).unwrap(),
            sealer: Sealer::new(Suite::ChaCha20Poly1305Sha256, &[0x62; 32]).unwrap(),
            suite: Suite::ChaCha20Poly1305Sha256,
            read_secret: vec![0x61; 32],
            write_secret: vec![0x62; 32],
            write_records: 0,
            post_handshake: HandshakeBuffer::new(MAX_POST_MESSAGE),
            tickets: 0,
            read_updates: 0,
            write_updates: 0,
            buffered: Vec::new(),
            buffered_at: 0,
            read_closed: false,
            write_closed: false,
            failed: false,
            certificate_summary: "unit-test record state".into(),
        }
    }

    #[test]
    fn authenticated_closure_differs_from_eof_and_tamper_poisons_stream() {
        let mut server = Sealer::new(Suite::ChaCha20Poly1305Sha256, &[0x61; 32]).unwrap();
        let first = server
            .seal(ContentType::ApplicationData, b"hello", 0)
            .unwrap();
        let close = server.seal(ContentType::Alert, &[1, 0], 0).unwrap();
        let mut clean = established_for_record_tests([first.clone(), close].concat());
        let mut bytes = Vec::new();
        clean.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"hello");
        assert_eq!(clean.read(&mut [0; 4]).unwrap(), 0);
        clean.close().unwrap();
        assert!(clean.write_all(b"late").is_err());
        let mut truncated = established_for_record_tests(first.clone());
        let mut buffer = [0; 8];
        assert_eq!(truncated.read(&mut buffer).unwrap(), 5);
        assert!(truncated
            .read(&mut buffer)
            .unwrap_err()
            .to_string()
            .contains("close_notify"));
        assert!(truncated.write_all(b"after failure").is_err());
        let mut damaged = first;
        damaged[10] ^= 1;
        let mut bad = established_for_record_tests(damaged);
        let mut untouched = [0xaa; 16];
        assert!(bad.read(&mut untouched).is_err());
        assert_eq!(untouched, [0xaa; 16]);
        assert!(bad.close().is_err());
    }

    #[test]
    fn key_update_uses_old_key_for_message_then_new_key_for_data() {
        let mut server_old = Sealer::new(Suite::ChaCha20Poly1305Sha256, &[0x61; 32]).unwrap();
        let update = server_old
            .seal(ContentType::Handshake, &[24, 0, 0, 1, 1], 0)
            .unwrap();
        let next_read =
            tls_crypto::next_traffic_secret(Suite::ChaCha20Poly1305Sha256, &[0x61; 32]).unwrap();
        let mut server_new = Sealer::new(Suite::ChaCha20Poly1305Sha256, &next_read).unwrap();
        let application = server_new
            .seal(ContentType::ApplicationData, b"new epoch", 0)
            .unwrap();
        let close = server_new.seal(ContentType::Alert, &[1, 0], 0).unwrap();
        let mut client = established_for_record_tests([update, application, close].concat());
        let mut received = [0; 32];
        assert_eq!(client.read(&mut received).unwrap(), 9);
        assert_eq!(&received[..9], b"new epoch");
        client.write_all(b"reply").unwrap();
        assert_eq!(client.read(&mut received).unwrap(), 0);
        client.close().unwrap();
        let mut sent = Transport {
            stream: MemoryIo::new(client.transport.stream.written),
            check: permitted,
        };
        let mut old = Opener::new(Suite::ChaCha20Poly1305Sha256, &[0x62; 32]).unwrap();
        let update_reply = old.open(&sent.read_record().unwrap()).unwrap();
        assert_eq!(update_reply.content_type, ContentType::Handshake);
        assert_eq!(update_reply.content, [24, 0, 0, 1, 0]);
        let mut new = Opener::new(
            Suite::ChaCha20Poly1305Sha256,
            &tls_crypto::next_traffic_secret(Suite::ChaCha20Poly1305Sha256, &[0x62; 32]).unwrap(),
        )
        .unwrap();
        assert_eq!(
            new.open(&sent.read_record().unwrap()).unwrap().content,
            b"reply"
        );
        assert_eq!(
            new.open(&sent.read_record().unwrap()).unwrap().content,
            [1, 0]
        );
    }

    #[test]
    fn fragmented_tickets_are_ignored_and_control_limits_are_enforced() {
        let ticket = handshake_message(4, &[0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0x55, 0, 0]);
        let mut server = Sealer::new(Suite::ChaCha20Poly1305Sha256, &[0x61; 32]).unwrap();
        let mut wire = Vec::new();
        for part in ticket.chunks(2) {
            wire.extend_from_slice(&server.seal(ContentType::Handshake, part, 0).unwrap());
        }
        wire.extend_from_slice(
            &server
                .seal(ContentType::ApplicationData, b"after ticket", 0)
                .unwrap(),
        );
        let mut client = established_for_record_tests(wire);
        let mut received = [0; 32];
        assert_eq!(client.read(&mut received).unwrap(), 12);
        assert_eq!(&received[..12], b"after ticket");
        assert_eq!(client.tickets, 1);
        assert!(client.post_handshake.empty());

        let mut server = Sealer::new(Suite::ChaCha20Poly1305Sha256, &[0x61; 32]).unwrap();
        let mut excessive = Vec::new();
        for _ in 0..=MAX_TICKETS {
            excessive.extend_from_slice(&server.seal(ContentType::Handshake, &ticket, 0).unwrap());
        }
        let mut client = established_for_record_tests(excessive);
        assert!(client
            .read(&mut received)
            .unwrap_err()
            .to_string()
            .contains("Too many TLS session tickets"));
        assert!(client.failed);

        let mut server = Sealer::new(Suite::ChaCha20Poly1305Sha256, &[0x61; 32]).unwrap();
        let mut wrong_boundary = vec![24, 0, 0, 1, 0];
        wrong_boundary.extend_from_slice(&ticket);
        let record = server
            .seal(ContentType::Handshake, &wrong_boundary, 0)
            .unwrap();
        let mut client = established_for_record_tests(record);
        assert!(client
            .read(&mut received)
            .unwrap_err()
            .to_string()
            .contains("end its record"));
        assert!(client.failed);
    }
}
