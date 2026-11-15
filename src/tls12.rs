//! Original TLS 1.2 full-handshake client, restricted to authenticated ECDHE.
//! AES-GCM is preferred, with SHA-256/SHA-384 AES-CBC interoperability. EMS
//! and Encrypt-then-MAC are offered; their negotiated state is reported.
//! No resumption, tickets, renegotiation, static RSA, client authentication,
//! exporters, or unauthenticated application data.
//!
//! Protocol references: RFC 5246, RFC 5289, RFC 7627 and RFC 8422. The parent
//! sends the common ClientHello and supplies any bytes following ServerHello.

use super::tls12_crypto::{self, Opener, Sealer, Suite};
use super::{
    alert_error, append_u16, dns_name, extensions, handshake_body, handshake_message, Bytes,
    HandshakeBuffer, Transcript, Transport, MAX_CERTIFICATES, MAX_CERTIFICATE_BYTES,
    MAX_CONTROL_RECORDS, MAX_HANDSHAKE_RECORDS,
};
use crate::crypto_rsa::{self, Hash};
use crate::tls_crypto::ContentType;
use crate::{crypto_ec, crypto_hash, crypto_sha512, crypto_x25519, x509};
use std::io::{self, Read, Write};
use std::time::SystemTime;

const X25519: u16 = 0x001d;
const P256: u16 = 0x0017;
const SIGNATURES: [u16; 8] = [
    0x0403, 0x0503, 0x0804, 0x0805, 0x0806, 0x0401, 0x0501, 0x0601,
];

struct ServerHello {
    suite: Suite,
    random: [u8; 32],
    extended_master_secret: bool,
    encrypt_then_mac: bool,
}

fn server_hello(message: &[u8], sent_sni: bool) -> Result<ServerHello, String> {
    let mut input = Bytes::new(handshake_body(message, 2)?);
    if input.u16()? != 0x0303 {
        return Err("TLS 1.2 requires ServerHello version 0x0303.".into());
    }
    let mut random = [0; 32];
    random.copy_from_slice(input.take(32)?);
    if random[24..] == *b"DOWNGRD\x01" || random[24..] == *b"DOWNGRD\x00" {
        return Err("TLS downgrade sentinel rejected: this client offered TLS 1.3.".into());
    }
    if random == super::HELLO_RETRY_RANDOM {
        return Err("TLS HelloRetryRequest is not a TLS 1.2 ServerHello.".into());
    }
    // A server may assign a new session ID even though our ID was empty. It is
    // discarded; an abbreviated handshake is rejected by the message order.
    if input.vector8()?.len() > 32 {
        return Err("TLS 1.2 session ID exceeds 32 bytes.".into());
    }
    let suite = Suite::from_id(input.u16()?)?;
    if input.byte()? != 0 {
        return Err("TLS compression is not permitted.".into());
    }
    let selected = if input.empty() {
        Vec::new()
    } else {
        extensions(input.vector16()?)?
    };
    input.finish()?;
    let mut extended_master_secret = false;
    let mut encrypt_then_mac = false;
    let mut secure_renegotiation = false;
    for (kind, value) in selected {
        match kind {
            0 if sent_sni && value.is_empty() => {}
            11 => {
                let mut formats = Bytes::new(value);
                let list = formats.vector8()?;
                if !list.contains(&0) {
                    return Err("TLS server does not accept uncompressed EC points.".into());
                }
                formats.finish()?;
            }
            16 => {
                let mut alpn = Bytes::new(value);
                let mut names = Bytes::new(alpn.vector16()?);
                if names.vector8()? != b"http/1.1" {
                    return Err("TLS server selected an unsupported ALPN protocol.".into());
                }
                names.finish()?;
                alpn.finish()?;
            }
            23 if value.is_empty() => extended_master_secret = true,
            22 if value.is_empty() && suite.uses_cbc() => encrypt_then_mac = true,
            0xff01 if value == [0] => secure_renegotiation = true,
            _ => {
                return Err(format!(
                    "Unsupported, malformed or unsolicited TLS 1.2 ServerHello extension {kind}."
                ));
            }
        }
    }
    // Even a client that never renegotiates must not let its initial flight
    // be spliced into a server's renegotiated connection (RFC 5746/9325).
    if !secure_renegotiation {
        return Err(
            "TLS 1.2 server did not acknowledge secure renegotiation (renegotiation_info).".into(),
        );
    }
    Ok(ServerHello {
        suite,
        random,
        extended_master_secret,
        encrypt_then_mac,
    })
}

fn certificates(message: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut input = Bytes::new(handshake_body(message, 11)?);
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

struct SignedParameters<'a> {
    group: u16,
    public: &'a [u8],
    encoded: &'a [u8],
    scheme: u16,
    signature: &'a [u8],
}

fn signed_parameters(message: &[u8]) -> Result<SignedParameters<'_>, String> {
    let body = handshake_body(message, 12)?;
    let mut input = Bytes::new(body);
    if input.byte()? != 3 {
        return Err("TLS 1.2 ECDHE requires a named curve.".into());
    }
    let group = input.u16()?;
    let public = input.vector8()?;
    match group {
        X25519 if public.len() == 32 => {}
        P256 if public.len() == 65 && public[0] == 4 => {
            crypto_ec::validate_key(crypto_ec::Curve::P256, public)?;
        }
        X25519 | P256 => return Err("Invalid TLS 1.2 ECDHE public key encoding.".into()),
        _ => return Err("The TLS 1.2 server selected an unsupported ECDHE group.".into()),
    }
    let encoded = &body[..input.offset];
    let scheme = input.u16()?;
    if !SIGNATURES.contains(&scheme) {
        return Err(
            "Unsupported or unoffered TLS 1.2 ServerKeyExchange signature algorithm.".into(),
        );
    }
    let signature = input.vector16()?;
    if signature.is_empty() || signature.len() > 512 {
        return Err("Invalid TLS 1.2 ServerKeyExchange signature size.".into());
    }
    input.finish()?;
    Ok(SignedParameters {
        group,
        public,
        encoded,
        scheme,
        signature,
    })
}

fn verify_parameters(
    parameters: &SignedParameters<'_>,
    verified: &x509::VerifiedServer,
    suite: Suite,
    client_random: &[u8; 32],
    server_random: &[u8; 32],
) -> Result<(), String> {
    let mut signed = Vec::with_capacity(64 + parameters.encoded.len());
    signed.extend_from_slice(client_random);
    signed.extend_from_slice(server_random);
    signed.extend_from_slice(parameters.encoded);
    let signature = parameters.signature;
    let result = match (suite.uses_rsa(), &verified.key, parameters.scheme) {
        (true, x509::PublicKey::Rsa { modulus, exponent }, 0x0804..=0x0806) => {
            let (hash, length) = match parameters.scheme {
                0x0804 => (Hash::Sha256, 32),
                0x0805 => (Hash::Sha384, 48),
                _ => (Hash::Sha512, 64),
            };
            crypto_rsa::verify_pss(modulus, exponent, hash, length, &signed, signature)
        }
        (true, x509::PublicKey::Rsa { modulus, exponent }, 0x0401 | 0x0501 | 0x0601) => {
            let hash = match parameters.scheme {
                0x0401 => Hash::Sha256,
                0x0501 => Hash::Sha384,
                _ => Hash::Sha512,
            };
            crypto_rsa::verify_pkcs1v15(modulus, exponent, hash, &signed, signature)
        }
        (false, x509::PublicKey::Ec { curve, point }, 0x0403 | 0x0503) => {
            // In TLS 1.2 these are hash/signature pairs, not TLS 1.3's
            // curve-bound SignatureSchemes. Either supported signing curve
            // can use either advertised SHA-256/SHA-384 hash.
            let digest = if parameters.scheme == 0x0403 {
                crypto_hash::sha256(&signed).to_vec()
            } else {
                crypto_sha512::sha384(&signed).to_vec()
            };
            crypto_ec::verify(*curve, point, &digest, signature)
        }
        _ => Err("TLS 1.2 cipher suite, signature and certificate key do not match.".into()),
    };
    result.map_err(|error| format!("TLS 1.2 ServerKeyExchange verification failed: {error}"))
}

fn shared_key(
    parameters: &SignedParameters<'_>,
    private_key: &[u8; 32],
) -> Result<(Vec<u8>, [u8; 32], &'static str), String> {
    match parameters.group {
        X25519 => {
            let mut peer = [0; 32];
            peer.copy_from_slice(parameters.public);
            let shared = crypto_x25519::shared_secret(private_key, &peer)?;
            Ok((
                crypto_x25519::public_key(private_key).to_vec(),
                shared,
                "X25519",
            ))
        }
        P256 => {
            let private = super::tls12_p256_secret(private_key)?;
            let shared = super::p256::shared_secret(&private, parameters.public)?;
            let public = super::p256::public_key(&private)?;
            Ok((public.to_vec(), shared, "P-256"))
        }
        _ => Err("Unsupported TLS 1.2 ECDHE group.".into()),
    }
}

fn count_record(records: &mut usize) -> Result<(), String> {
    *records = records
        .checked_add(1)
        .ok_or("TLS handshake record counter overflow.")?;
    if *records > MAX_HANDSHAKE_RECORDS {
        return Err("Too many TLS handshake records.".into());
    }
    Ok(())
}

fn plaintext_record(record: &[u8]) -> Result<(u8, &[u8]), String> {
    if record.len() < 5 || record[1..3] != [3, 3] {
        return Err("Invalid TLS 1.2 plaintext record version or header.".into());
    }
    let length = usize::from(u16::from_be_bytes([record[3], record[4]]));
    if length > tls12_crypto::MAX_CONTENT_BYTES || record.len() != 5 + length {
        return Err("Invalid or oversized TLS 1.2 plaintext record.".into());
    }
    Ok((record[0], &record[5..]))
}

fn receive_plain<S: Read + Write, C: Fn() -> Result<(), String>>(
    transport: &mut Transport<S, C>,
    buffer: &mut HandshakeBuffer,
    records: &mut usize,
) -> Result<Vec<u8>, String> {
    loop {
        (transport.check)()?;
        if let Some(message) = buffer.next()? {
            return Ok(message);
        }
        count_record(records)?;
        let record = transport.read_record()?;
        match plaintext_record(&record)? {
            (22, content) if !content.is_empty() => buffer.push(content)?,
            (21, content) => return Err(alert_error(content)),
            (20, _) => return Err("Premature TLS 1.2 ChangeCipherSpec.".into()),
            _ => return Err("Expected a nonempty TLS 1.2 handshake record.".into()),
        }
    }
}

fn receive_ccs<S: Read + Write, C: Fn() -> Result<(), String>>(
    transport: &mut Transport<S, C>,
    records: &mut usize,
) -> Result<(), String> {
    count_record(records)?;
    let record = transport.read_record()?;
    match plaintext_record(&record)? {
        (20, [1]) => Ok(()),
        (21, content) => Err(alert_error(content)),
        _ => Err("TLS 1.2 requires exactly one ChangeCipherSpec before server Finished.".into()),
    }
}

fn receive_finished<S: Read + Write, C: Fn() -> Result<(), String>>(
    transport: &mut Transport<S, C>,
    buffer: &mut HandshakeBuffer,
    opener: &mut Opener,
    records: &mut usize,
) -> Result<Vec<u8>, String> {
    loop {
        (transport.check)()?;
        if let Some(message) = buffer.next()? {
            return Ok(message);
        }
        count_record(records)?;
        let record = transport.read_record()?;
        let opened = opener.open(&record)?;
        match opened.content_type {
            ContentType::Handshake => buffer.push(&opened.content)?,
            ContentType::Alert => return Err(alert_error(&opened.content)),
            ContentType::ApplicationData => {
                return Err(
                    "TLS application data arrived before server Finished verification.".into(),
                );
            }
        }
    }
}

fn send_plain<S: Read + Write, C: Fn() -> Result<(), String>>(
    transport: &mut Transport<S, C>,
    kind: u8,
    content: &[u8],
) -> Result<(), String> {
    if content.len() > tls12_crypto::MAX_CONTENT_BYTES {
        return Err("TLS 1.2 plaintext exceeds the record limit.".into());
    }
    let mut record = vec![kind, 3, 3];
    append_u16(&mut record, content.len());
    record.extend_from_slice(content);
    transport.write_all(&record)
}

/// Authenticated, single-epoch TLS 1.2 connection. No secret state is cloneable
/// or printable; a failed record/transport operation poisons both directions.
pub(super) struct Connection<S, C> {
    transport: Transport<S, C>,
    opener: Opener,
    sealer: Sealer,
    buffered: Vec<u8>,
    buffered_at: usize,
    read_closed: bool,
    write_closed: bool,
    failed: bool,
    certificate_summary: String,
}

impl<S: Read + Write, C: Fn() -> Result<(), String>> Connection<S, C> {
    pub(super) fn reusable(&self) -> bool {
        !self.failed
            && !self.read_closed
            && !self.write_closed
            && self.buffered_at == self.buffered.len()
    }

    pub(super) fn peer_closed(&self) -> bool {
        self.read_closed && !self.failed
    }

    pub(super) fn get_ref(&self) -> &S {
        &self.transport.stream
    }

    #[allow(clippy::too_many_arguments)] // Parent transfers one partially read handshake.
    pub(super) fn connect(
        mut transport: Transport<S, C>,
        hello: Vec<u8>,
        server: Vec<u8>,
        mut buffer: HandshakeBuffer,
        mut records: usize,
        hostname: &str,
        roots: &[Vec<u8>],
        now: SystemTime,
        client_random: [u8; 32],
        private_key: [u8; 32],
    ) -> Result<Self, String> {
        (transport.check)()?;
        let name = dns_name(hostname)?;
        let chosen = server_hello(&server, name.is_some())
            .map_err(|error| format!("TLS 1.2 ServerHello: {error}"))?;
        let suite = chosen.suite;
        let ems = if chosen.extended_master_secret {
            "Extended Master Secret"
        } else {
            "EMS not negotiated; full handshake"
        };
        let protection = if !suite.uses_cbc() {
            "AEAD"
        } else if chosen.encrypt_then_mac {
            "Encrypt-then-MAC"
        } else {
            "legacy MAC-then-encrypt"
        };
        let negotiation = format!("{}, {ems}, {protection}", suite.name());
        let result = (|| {
            let mut transcript = Transcript::new();
            transcript.add(&hello)?;
            transcript.add(&server)?;

            let certificate = receive_plain(&mut transport, &mut buffer, &mut records)?;
            let chain = certificates(&certificate)?;
            (transport.check)()?;
            let verified = x509::verify_server(&chain, roots, name.unwrap_or(hostname), now)
                .map_err(|error| format!("TLS certificate verification failed: {error}"))?;
            (transport.check)()?;
            transcript.add(&certificate)?;

            let exchange = receive_plain(&mut transport, &mut buffer, &mut records)?;
            let parameters = signed_parameters(&exchange)?;
            verify_parameters(
                &parameters,
                &verified,
                suite,
                &client_random,
                &chosen.random,
            )?;
            (transport.check)()?;
            transcript.add(&exchange)?;

            let done = receive_plain(&mut transport, &mut buffer, &mut records)?;
            if !handshake_body(&done, 14)?.is_empty() {
                return Err("TLS 1.2 ServerHelloDone must be empty.".into());
            }
            if !buffer.empty() {
                return Err("Unexpected handshake bytes after TLS 1.2 ServerHelloDone.".into());
            }
            transcript.add(&done)?;

            let (public, shared, group) = shared_key(&parameters, &private_key)?;
            (transport.check)()?;
            let mut body = Vec::with_capacity(1 + public.len());
            body.push(public.len() as u8);
            body.extend_from_slice(&public);
            let client_exchange = handshake_message(16, &body);
            transcript.add(&client_exchange)?;
            // RFC 7627: the session hash includes ClientKeyExchange, including its
            // handshake header, and contains neither CCS nor record framing.
            let session_hash = suite.hash(&transcript.bytes);
            let master = if chosen.extended_master_secret {
                tls12_crypto::master_secret(suite, &shared, &session_hash)?
            } else {
                // RFC 7627 section 5.2's explicit legacy-server interoperability
                // path. No resumed/renegotiated session or master-secret binding
                // API exists in this implementation; see this module's header.
                tls12_crypto::legacy_master_secret(suite, &shared, &client_random, &chosen.random)?
            };
            let (mut sealer, mut opener) = tls12_crypto::key_pair(
                suite,
                &master,
                &client_random,
                &chosen.random,
                chosen.encrypt_then_mac,
            )?;
            let client_finished = handshake_message(
                20,
                &tls12_crypto::finished(suite, &master, "client finished", &session_hash)?,
            );
            send_plain(&mut transport, 22, &client_exchange)?;
            send_plain(&mut transport, 20, &[1])?;
            transport.write_all(&sealer.seal(ContentType::Handshake, &client_finished)?)?;
            transport.flush()?;
            transcript.add(&client_finished)?;

            receive_ccs(&mut transport, &mut records)?;
            let server_finished =
                receive_finished(&mut transport, &mut buffer, &mut opener, &mut records)?;
            tls12_crypto::verify_finished(
                suite,
                &master,
                "server finished",
                &suite.hash(&transcript.bytes),
                handshake_body(&server_finished, 20)?,
            )?;
            if !buffer.empty() {
                return Err("Unexpected handshake bytes after TLS 1.2 Finished.".into());
            }
            (transport.check)()?;

            Ok(Self {
                transport,
                opener,
                sealer,
                buffered: Vec::new(),
                buffered_at: 0,
                read_closed: false,
                write_closed: false,
                failed: false,
                certificate_summary: format!(
                    "TLS 1.2, {group}, {negotiation}; {}",
                    verified.summary
                ),
            })
        })();
        result.map_err(|error: String| format!("TLS 1.2 handshake; server selected {negotiation}; authentication incomplete: {error}"))
    }

    pub(super) fn certificate_summary(&self) -> &str {
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

    fn fill_buffer(&mut self) -> Result<(), String> {
        for _ in 0..MAX_CONTROL_RECORDS {
            self.ensure_live()?;
            let record = self.transport.read_record()?;
            let opened = self.opener.open(&record)?;
            match opened.content_type {
                ContentType::ApplicationData => {
                    if !opened.content.is_empty() {
                        self.buffered = opened.content;
                        self.buffered_at = 0;
                        return Ok(());
                    }
                }
                ContentType::Handshake => {
                    return Err(
                        "TLS 1.2 renegotiation and post-handshake messages are not supported."
                            .into(),
                    );
                }
                ContentType::Alert => {
                    if opened.content == [1, 0] {
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
        self.ensure_live()?;
        if output.is_empty() {
            return Ok(0);
        }
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
        self.ensure_live()?;
        if input.is_empty() {
            return Ok(0);
        }
        if self.write_closed || self.read_closed {
            return Err("TLS 1.2 application writes are closed.".into());
        }
        let count = input.len().min(tls12_crypto::MAX_CONTENT_BYTES);
        let record = self
            .sealer
            .seal(ContentType::ApplicationData, &input[..count])?;
        self.transport.write_all(&record)?;
        Ok(count)
    }

    /// Send an authenticated close_notify when HTTP retires this connection,
    /// also acknowledging a received close_notify.
    /// Dropping a connection never performs blocking network I/O.
    pub(super) fn close(&mut self) -> io::Result<()> {
        let result = (|| {
            self.ensure_live()?;
            if self.write_closed {
                return Ok(());
            }
            let record = self.sealer.seal(ContentType::Alert, &[1, 0])?;
            self.transport.write_all(&record)?;
            self.transport.flush()?;
            self.write_closed = true;
            Ok(())
        })();
        self.io_result(result)
    }
}

impl<S: Read + Write, C: Fn() -> Result<(), String>> Read for Connection<S, C> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let result = self.read_inner(output);
        self.io_result(result)
    }
}

impl<S: Read + Write, C: Fn() -> Result<(), String>> Write for Connection<S, C> {
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
    use super::super::{append_u24, extension, MAX_HANDSHAKE_MESSAGE};
    use super::*;
    use std::cell::Cell;
    use std::io::Cursor;

    fn server_with(suite: u16, selected: &[u8], random: [u8; 32], session: &[u8]) -> Vec<u8> {
        let mut body = vec![3, 3];
        body.extend_from_slice(&random);
        body.push(session.len() as u8);
        body.extend_from_slice(session);
        body.extend_from_slice(&suite.to_be_bytes());
        body.push(0);
        append_u16(&mut body, selected.len());
        body.extend_from_slice(selected);
        handshake_message(2, &body)
    }

    fn normal_extensions() -> Vec<u8> {
        let mut selected = Vec::new();
        extension(&mut selected, 0, &[]);
        extension(&mut selected, 11, &[3, 0, 1, 2]);
        extension(&mut selected, 16, b"\0\x09\x08http/1.1");
        extension(&mut selected, 23, &[]);
        extension(&mut selected, 0xff01, &[0]);
        selected
    }

    #[test]
    fn hello_checks_offers_secure_renegotiation_and_downgrade_sentinel() {
        let selected = normal_extensions();
        for suite in [0xc02f, 0xc030, 0xc02b, 0xc02c, 0xc027, 0xc028] {
            let message = server_with(suite, &selected, [0x31; 32], &[0x55; 32]);
            let parsed = server_hello(&message, true).unwrap();
            assert_eq!(parsed.random, [0x31; 32]);
            assert_eq!(
                parsed.suite.uses_rsa(),
                matches!(suite, 0xc02f | 0xc030 | 0xc027 | 0xc028)
            );
            assert!(parsed.extended_master_secret);
            assert!(!parsed.encrypt_then_mac);
            for cut in 0..message.len() {
                assert!(server_hello(&message[..cut], true).is_err(), "cut {cut}");
            }
        }
        for suffix in [b"DOWNGRD\x01", b"DOWNGRD\x00"] {
            let mut random = [0x31; 32];
            random[24..].copy_from_slice(suffix);
            assert!(
                server_hello(&server_with(0xc02f, &selected, random, &[]), true)
                    .err()
                    .unwrap()
                    .contains("downgrade")
            );
        }
        assert!(server_hello(&server_with(0xc02f, &[], [1; 32], &[]), true)
            .err()
            .unwrap()
            .contains("secure renegotiation"));
        assert!(server_hello(&server_with(0xc02f, &selected, [1; 32], &[0; 33]), true).is_err());
        assert!(server_hello(&server_with(0xc02f, &selected, [1; 32], &[]), false).is_err());
        for suite in [0x1301, 0x009c, 0xc013, 0, 0xffff] {
            assert!(server_hello(&server_with(suite, &selected, [1; 32], &[]), true).is_err());
        }
        for (kind, value) in [
            (23, vec![0]),
            (0xff01, vec![1, 0]),
            (35, vec![]),
            (43, vec![3, 3]),
            (11, vec![1, 1]),
            (16, b"\0\x03\x02h2".to_vec()),
            (22, vec![]), // RFC 7366 forbids EtM with an AEAD suite.
        ] {
            let mut changed = Vec::new();
            if kind != 23 {
                extension(&mut changed, 23, &[]);
            }
            extension(&mut changed, kind, &value);
            assert!(
                server_hello(&server_with(0xc02f, &changed, [1; 32], &[]), true).is_err(),
                "extension {kind}"
            );
        }
        let mut repeated = selected.clone();
        extension(&mut repeated, 23, &[]);
        assert!(server_hello(&server_with(0xc02f, &repeated, [1; 32], &[]), true).is_err());
        let mut bad = server_with(0xc02f, &selected, [1; 32], &[]);
        bad[5] = 2;
        assert!(server_hello(&bad, true).is_err());
        bad[5] = 3;
        bad[41] = 1;
        assert!(server_hello(&bad, true).is_err());
    }

    #[test]
    fn hello_distinguishes_ems_and_cbc_record_negotiation() {
        for suite in [0xc02f, 0xc030, 0xc027, 0xc028] {
            for ems in [false, true] {
                for etm in [false, true] {
                    let mut selected = Vec::new();
                    extension(&mut selected, 0xff01, &[0]);
                    if ems {
                        extension(&mut selected, 23, &[]);
                    }
                    if etm {
                        extension(&mut selected, 22, &[]);
                    }
                    let result = server_hello(&server_with(suite, &selected, [2; 32], &[]), false);
                    if etm && !matches!(suite, 0xc027 | 0xc028) {
                        assert!(result.is_err());
                    } else {
                        let parsed = result.unwrap();
                        assert_eq!(parsed.extended_master_secret, ems);
                        assert_eq!(parsed.encrypt_then_mac, etm);
                    }
                }
            }
        }
        let mut bad = normal_extensions();
        extension(&mut bad, 22, &[0]);
        assert!(server_hello(&server_with(0xc028, &bad, [2; 32], &[]), true).is_err());
        let mut duplicate = normal_extensions();
        extension(&mut duplicate, 22, &[]);
        extension(&mut duplicate, 22, &[]);
        assert!(server_hello(&server_with(0xc028, &duplicate, [2; 32], &[]), true).is_err());
    }

    #[test]
    fn certificate_message_has_tls12_lengths_and_hard_limits() {
        let mut list = Vec::new();
        for certificate in [b"first".as_slice(), b"second".as_slice()] {
            append_u24(&mut list, certificate.len());
            list.extend_from_slice(certificate);
        }
        let mut body = Vec::new();
        append_u24(&mut body, list.len());
        body.extend_from_slice(&list);
        let message = handshake_message(11, &body);
        assert_eq!(
            certificates(&message).unwrap(),
            [b"first".to_vec(), b"second".to_vec()]
        );
        for cut in 0..message.len() {
            assert!(certificates(&message[..cut]).is_err());
        }
        assert!(certificates(&handshake_message(11, &[0, 0, 0])).is_err());
        assert!(certificates(&handshake_message(11, &[0, 0, 3, 0, 0, 0])).is_err());
        assert!(certificates(&handshake_message(13, &[]))
            .err()
            .unwrap()
            .contains("client-certificate"));
        let mut list = Vec::new();
        for _ in 0..=MAX_CERTIFICATES {
            list.extend_from_slice(&[0, 0, 1, 0]);
        }
        let mut body = Vec::new();
        append_u24(&mut body, list.len());
        body.extend_from_slice(&list);
        assert!(certificates(&handshake_message(11, &body))
            .err()
            .unwrap()
            .contains("too long"));
        let length = MAX_CERTIFICATE_BYTES + 1;
        let mut body = Vec::new();
        append_u24(&mut body, length + 3);
        append_u24(&mut body, length);
        body.resize(body.len() + length, 0);
        assert!(certificates(&handshake_message(11, &body))
            .err()
            .unwrap()
            .contains("64 KiB"));
    }

    fn exchange(group: u16, public: &[u8], scheme: u16, signature: &[u8]) -> Vec<u8> {
        let mut body = vec![3];
        body.extend_from_slice(&group.to_be_bytes());
        body.push(public.len() as u8);
        body.extend_from_slice(public);
        body.extend_from_slice(&scheme.to_be_bytes());
        append_u16(&mut body, signature.len());
        body.extend_from_slice(signature);
        handshake_message(12, &body)
    }

    #[test]
    fn key_exchange_rejects_unknown_groups_malformed_points_and_weak_signatures() {
        let message = exchange(X25519, &[7; 32], 0x0804, &[1; 256]);
        let parsed = signed_parameters(&message).unwrap();
        assert_eq!(parsed.encoded, &message[4..40]);
        assert_eq!(parsed.scheme, 0x0804);
        for cut in 0..message.len() {
            assert!(signed_parameters(&message[..cut]).is_err());
        }
        for scheme in [0x0201, 0x0203, 0x0603, 0x0809, 0x0807, 0] {
            assert!(signed_parameters(&exchange(X25519, &[7; 32], scheme, &[1])).is_err());
        }
        for (group, public) in [
            (0x0018, vec![4; 97]),
            (X25519, vec![7; 31]),
            (P256, vec![4; 65]),
            (P256, vec![2; 33]),
        ] {
            assert!(signed_parameters(&exchange(group, &public, 0x0403, &[1])).is_err());
        }
        assert!(signed_parameters(&exchange(X25519, &[7; 32], 0x0804, &[])).is_err());
        let mut explicit = message.clone();
        explicit[4] = 1;
        assert!(signed_parameters(&explicit).is_err());
        let rsa = x509::VerifiedServer {
            key: x509::PublicKey::Rsa {
                modulus: vec![1],
                exponent: vec![3],
            },
            summary: String::new(),
        };
        assert!(verify_parameters(
            &parsed,
            &rsa,
            Suite::from_id(0xc02b).unwrap(),
            &[1; 32],
            &[2; 32]
        )
        .err()
        .unwrap()
        .contains("do not match"));
        let zero = exchange(X25519, &[0; 32], 0x0804, &[1]);
        assert!(shared_key(&signed_parameters(&zero).unwrap(), &[5; 32]).is_err());
    }

    struct MemoryIo {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }
    impl Read for MemoryIo {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            let length = output.len().min(3);
            self.input.read(&mut output[..length])
        }
    }
    impl Write for MemoryIo {
        fn write(&mut self, input: &[u8]) -> io::Result<usize> {
            let length = input.len().min(5);
            self.output.extend_from_slice(&input[..length]);
            Ok(length)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    fn memory(input: Vec<u8>) -> MemoryIo {
        MemoryIo {
            input: Cursor::new(input),
            output: Vec::new(),
        }
    }
    fn record(kind: u8, content: &[u8]) -> Vec<u8> {
        let mut wire = vec![kind, 3, 3];
        append_u16(&mut wire, content.len());
        wire.extend_from_slice(content);
        wire
    }

    #[test]
    fn plaintext_reassembly_preserves_coalesced_messages_and_ccs_is_strict() {
        let first = handshake_message(11, &[0, 0, 0]);
        let second = handshake_message(14, &[]);
        let mut wire = record(22, &first[..2]);
        let mut rest = first[2..].to_vec();
        rest.extend_from_slice(&second);
        wire.extend(record(22, &rest));
        wire.extend(record(20, &[1]));
        let mut transport = Transport {
            stream: memory(wire),
            check: || Ok(()),
        };
        let mut buffer = HandshakeBuffer::new(MAX_HANDSHAKE_MESSAGE);
        let mut records = 0;
        assert_eq!(
            receive_plain(&mut transport, &mut buffer, &mut records).unwrap(),
            first
        );
        assert_eq!(
            receive_plain(&mut transport, &mut buffer, &mut records).unwrap(),
            second
        );
        assert_eq!(records, 2);
        receive_ccs(&mut transport, &mut records).unwrap();
        assert_eq!(records, 3);
        for mut wire in [
            record(20, &[0]),
            record(20, &[1, 1]),
            record(22, &[1]),
            record(23, &[]),
            record(20, &[1]),
        ] {
            if wire == record(20, &[1]) {
                wire[2] = 1;
            }
            let mut transport = Transport {
                stream: memory(wire),
                check: || Ok(()),
            };
            assert!(receive_ccs(&mut transport, &mut 0).is_err());
        }
        let mut transport = Transport {
            stream: memory(record(20, &[1])),
            check: || Ok(()),
        };
        assert!(receive_plain(
            &mut transport,
            &mut HandshakeBuffer::new(MAX_HANDSHAKE_MESSAGE),
            &mut 0
        )
        .err()
        .unwrap()
        .contains("Premature"));
        let mut count = MAX_HANDSHAKE_RECORDS;
        assert!(count_record(&mut count).is_err());
        let mut transport = Transport {
            stream: memory(vec![]),
            check: || Err("cancelled".into()),
        };
        let mut buffered = HandshakeBuffer::new(MAX_HANDSHAKE_MESSAGE);
        buffered.push(&second).unwrap();
        assert_eq!(
            receive_plain(&mut transport, &mut buffered, &mut 0).unwrap_err(),
            "cancelled"
        );
    }

    #[test]
    fn authentication_failure_poisoning_does_not_release_plaintext_or_send_again() {
        let suite = Suite::from_id(0xc02f).unwrap();
        let master = [0x63; 48];
        let client = [1; 32];
        let server = [2; 32];
        // Opposite-direction ciphertext, with a corrupted tag, must not be
        // accepted or release bytes. Independent peers cover positive records.
        let (mut writer, _) =
            tls12_crypto::key_pair(suite, &master, &client, &server, false).unwrap();
        let (_, opener) = tls12_crypto::key_pair(suite, &master, &client, &server, false).unwrap();
        let mut bad = writer
            .seal(ContentType::ApplicationData, b"must not escape")
            .unwrap();
        *bad.last_mut().unwrap() ^= 1;
        let (sealer, _) = tls12_crypto::key_pair(suite, &master, &client, &server, false).unwrap();
        let cancelled = Cell::new(false);
        let mut connection = Connection {
            transport: Transport {
                stream: memory(bad),
                check: || {
                    if cancelled.get() {
                        Err("cancelled".into())
                    } else {
                        Ok(())
                    }
                },
            },
            opener,
            sealer,
            buffered: Vec::new(),
            buffered_at: 0,
            read_closed: false,
            write_closed: false,
            failed: false,
            certificate_summary: String::new(),
        };
        let mut output = [0x5a; 32];
        assert!(connection.read(&mut output).is_err());
        assert_eq!(output, [0x5a; 32]);
        assert!(connection.write(b"no replay").is_err());
        assert!(connection.close().is_err());
        assert!(connection.transport.stream.output.is_empty());
        cancelled.set(true);
        assert!(connection.flush().is_err());
    }
}
