//! The one permitted TLS 1.3 HelloRetryRequest flight.
//!
//! Original implementation from RFC 8446 sections 4.1.2, 4.1.4, 4.2.2,
//! 4.2.8 and 4.4.1. The initial offer has an X25519 share and advertises
//! X25519/P-256 groups. Only a P-256 request and/or a bounded cookie can
//! change that offer. No cookie or key is retained across connections.

use super::{
    append_u16, crypto_x25519, extension, extensions, handshake_body, handshake_message, p256,
    p256_secret, tls_crypto, Bytes, Suite, Transcript, HELLO_RETRY_RANDOM, P256_GROUP,
    X25519_GROUP,
};

const MAX_COOKIE_BYTES: usize = 4096;

pub(super) enum ClientKey {
    X25519([u8; 32]),
    P256([u8; 32]),
}

impl ClientKey {
    pub(super) fn group(&self) -> u16 {
        match self {
            Self::X25519(_) => X25519_GROUP,
            Self::P256(_) => P256_GROUP,
        }
    }

    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::X25519(_) => "X25519",
            Self::P256(_) => "P-256",
        }
    }

    pub(super) fn shared_secret(&self, peer: &[u8]) -> Result<[u8; 32], String> {
        match self {
            Self::X25519(private) => {
                let peer = peer
                    .try_into()
                    .map_err(|_| "Invalid server X25519 key length.")?;
                crypto_x25519::shared_secret(private, peer)
            }
            Self::P256(private) => p256::shared_secret(private, peer),
        }
    }
}

pub(super) struct Request<'a> {
    pub(super) suite: Suite,
    p256: bool,
    // The complete validated extension value, including its vector length,
    // is copied verbatim. It is never interpreted or stored in a profile.
    cookie: Option<&'a [u8]>,
}

impl<'a> Request<'a> {
    pub(super) fn parse(message: &'a [u8]) -> Result<Option<Self>, String> {
        let mut input = Bytes::new(handshake_body(message, 2)?);
        let legacy_version = input.u16()?;
        if input.take(32)? != HELLO_RETRY_RANDOM {
            return Ok(None);
        }
        if legacy_version != 0x0303 {
            return Err("Invalid HelloRetryRequest legacy version.".into());
        }
        if !input.vector8()?.is_empty() {
            return Err("HelloRetryRequest did not echo the empty session ID.".into());
        }
        let suite = Suite::from_wire(input.u16()?)?;
        if input.byte()? != 0 {
            return Err("TLS compression is not permitted in HelloRetryRequest.".into());
        }
        let selected = extensions(input.vector16()?)?;
        input.finish()?;
        let mut version = false;
        let mut p256 = false;
        let mut cookie = None;
        for (kind, bytes) in selected {
            match kind {
                43 if bytes == [3, 4] => version = true,
                43 => return Err("HelloRetryRequest did not select TLS 1.3.".into()),
                51 if bytes == P256_GROUP.to_be_bytes() => p256 = true,
                51 => {
                    return Err(
                        "HelloRetryRequest selected an unoffered group or an already offered key share."
                            .into(),
                    )
                }
                44 => {
                    let mut value = Bytes::new(bytes);
                    let contents = value.vector16()?;
                    value.finish()?;
                    if contents.is_empty() || contents.len() > MAX_COOKIE_BYTES {
                        return Err("HelloRetryRequest cookie must contain 1..4096 bytes.".into());
                    }
                    cookie = Some(bytes);
                }
                _ => return Err(format!("Unexpected HelloRetryRequest extension {kind}.")),
            }
        }
        if !version {
            return Err("HelloRetryRequest has no TLS 1.3 supported_versions extension.".into());
        }
        if !p256 && cookie.is_none() {
            return Err("HelloRetryRequest would not change the ClientHello.".into());
        }
        Ok(Some(Self {
            suite,
            p256,
            cookie,
        }))
    }

    pub(super) fn client_hello(
        &self,
        first: &[u8],
        entropy: &[u8; 32],
    ) -> Result<(Vec<u8>, ClientKey), String> {
        let key = if self.p256 {
            ClientKey::P256(p256_secret(
                entropy,
                b"OwnBrowser TLS 1.3 P-256 ephemeral key",
            )?)
        } else {
            ClientKey::X25519(*entropy)
        };
        let replacement = if let ClientKey::P256(private) = &key {
            let public = p256::public_key(private)?;
            let mut share = Vec::with_capacity(71);
            append_u16(&mut share, public.len() + 4);
            share.extend_from_slice(&P256_GROUP.to_be_bytes());
            append_u16(&mut share, public.len());
            share.extend_from_slice(&public);
            Some(share)
        } else {
            None
        };

        // Copy the original encoding, not a freshly generated ClientHello:
        // random, session ID, cipher suites, extension order and all unaffected
        // extension values must remain identical (RFC 8446 section 4.1.2).
        let body = handshake_body(first, 1)?;
        let mut input = Bytes::new(body);
        input.take(34)?;
        input.vector8()?;
        input.vector16()?;
        input.vector8()?;
        let prefix = input.offset;
        let offered = extensions(input.vector16()?)?;
        input.finish()?;
        let mut updated = Vec::new();
        let mut found_share = false;
        for (kind, bytes) in offered {
            if kind == 51 {
                found_share = true;
                extension(&mut updated, kind, replacement.as_deref().unwrap_or(bytes));
            } else {
                extension(&mut updated, kind, bytes);
            }
        }
        if !found_share {
            return Err("Initial ClientHello has no key share to retry.".into());
        }
        if let Some(cookie) = self.cookie {
            extension(&mut updated, 44, cookie);
        }
        let second_length = 4 + prefix + 2 + updated.len();
        if second_length > tls_crypto::MAX_CONTENT_BYTES {
            return Err("Retried ClientHello exceeds one 16 KiB TLS plaintext record.".into());
        }
        let mut second = body[..prefix].to_vec();
        append_u16(&mut second, updated.len());
        second.extend_from_slice(&updated);
        Ok((handshake_message(1, &second), key))
    }

    pub(super) fn transcript(&self, first: &[u8], retry: &[u8]) -> Result<Transcript, String> {
        let mut transcript = Transcript::new();
        // RFC 8446 section 4.4.1: the synthetic message includes the normal
        // handshake type/length header and hashes all of ClientHello1.
        transcript.add(&handshake_message(254, &self.suite.hash(first)))?;
        transcript.add(retry)?;
        Ok(transcript)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(extra: &[(u16, &[u8])], suite: u16) -> Vec<u8> {
        let mut selected = Vec::new();
        for (kind, value) in extra {
            extension(&mut selected, *kind, value);
        }
        let mut body = vec![3, 3];
        body.extend_from_slice(&HELLO_RETRY_RANDOM);
        body.push(0);
        body.extend_from_slice(&suite.to_be_bytes());
        body.push(0);
        append_u16(&mut body, selected.len());
        body.extend_from_slice(&selected);
        handshake_message(2, &body)
    }

    fn client_parts(message: &[u8]) -> (&[u8], Vec<(u16, &[u8])>) {
        let body = handshake_body(message, 1).unwrap();
        let mut input = Bytes::new(body);
        input.take(34).unwrap();
        input.vector8().unwrap();
        input.vector16().unwrap();
        input.vector8().unwrap();
        let prefix = input.offset;
        let offered = extensions(input.vector16().unwrap()).unwrap();
        input.finish().unwrap();
        (&body[..prefix], offered)
    }

    #[test]
    fn hello_retry_accepts_only_a_changed_supported_offer() {
        for extra in [
            vec![(43, &[3, 4][..]), (51, &[0, 0x17][..])],
            vec![(43, &[3, 4][..]), (44, &[0, 1, 0xaa][..])],
            vec![
                (43, &[3, 4][..]),
                (51, &[0, 0x17][..]),
                (44, &[0, 1, 0xaa][..]),
            ],
        ] {
            for suite in [0x1301, 0x1302, 0x1303] {
                let message = request(&extra, suite);
                assert!(Request::parse(&message).unwrap().is_some());
            }
        }
        let cases: &[&[(u16, &[u8])]] = &[
            &[],
            &[(43, &[3, 4])],
            &[(51, &[0, 0x17])],
            &[(43, &[3, 3]), (51, &[0, 0x17])],
            &[(43, &[3, 4]), (51, &[0, 0x1d])],
            &[(43, &[3, 4]), (51, &[0, 0x18])],
            &[(43, &[3, 4]), (51, &[0, 0x17, 0])],
            &[(43, &[3, 4]), (51, &[0])],
            &[(43, &[3, 4]), (44, &[0, 0])],
            &[(43, &[3, 4]), (44, &[0, 1])],
            &[(43, &[3, 4]), (44, &[0, 1, 0xaa, 0])],
            &[(43, &[3, 4]), (51, &[0, 0x17]), (43, &[3, 4])],
            &[(43, &[3, 4]), (51, &[0, 0x17]), (51, &[0, 0x17])],
            &[(43, &[3, 4]), (44, &[0, 1, 0xaa]), (44, &[0, 1, 0xbb])],
            &[(43, &[3, 4]), (51, &[0, 0x17]), (0, &[])],
            &[(43, &[3, 4]), (51, &[0, 0x17]), (16, &[])],
            &[(43, &[3, 4]), (51, &[0, 0x17]), (0xface, &[])],
        ];
        for extra in cases {
            assert!(
                Request::parse(&request(extra, 0x1303)).is_err(),
                "{extra:?}"
            );
        }
        let good = request(&[(43, &[3, 4]), (51, &[0, 0x17])], 0x1303);
        for offset in [4, 5, 38, 39, 41] {
            let mut changed = good.clone();
            changed[offset] ^= 1;
            assert!(Request::parse(&changed).is_err(), "offset {offset}");
        }
        for length in 0..good.len() {
            assert!(Request::parse(&good[..length]).is_err());
        }
        assert!(Request::parse(&request(&[(43, &[3, 4]), (51, &[0, 0x17])], 0xc02f)).is_err());
        assert!(super::super::selected_version(&request(&[(51, &[0, 0x17])], 0x1303)).is_err());
    }

    #[test]
    fn hello_retry_copies_opaque_cookie_and_preserves_every_other_field() {
        let entropy = [0x53; 32];
        let first = super::super::client_hello(
            "example.test",
            &[0x31; 32],
            &crypto_x25519::public_key(&entropy),
        )
        .unwrap();
        let mut cookie = Vec::new();
        append_u16(&mut cookie, MAX_COOKIE_BYTES);
        cookie.extend((0..MAX_COOKIE_BYTES).map(|index| index as u8));
        let message = request(&[(43, &[3, 4]), (44, &cookie)], 0x1303);
        let retry = Request::parse(&message).unwrap().unwrap();
        let (second, key) = retry.client_hello(&first, &entropy).unwrap();
        assert_eq!(key.group(), X25519_GROUP);
        let (first_prefix, first_extra) = client_parts(&first);
        let (second_prefix, second_extra) = client_parts(&second);
        assert_eq!(first_prefix, second_prefix);
        assert_eq!(&second_extra[..first_extra.len()], &first_extra);
        assert_eq!(second_extra.last().unwrap(), &(44, cookie.as_slice()));
        assert!(second.len() < tls_crypto::MAX_CONTENT_BYTES);
        assert!(!first_extra.iter().any(|(kind, _)| *kind == 44));
        cookie[0..2].copy_from_slice(&((MAX_COOKIE_BYTES + 1) as u16).to_be_bytes());
        cookie.push(0);
        assert!(Request::parse(&request(&[(43, &[3, 4]), (44, &cookie)], 0x1303)).is_err());
    }

    #[test]
    fn hello_retry_p256_share_is_independent_and_validates_peer_points() {
        let entropy = [0x63; 32];
        let first = super::super::client_hello(
            "example.test",
            &[0x32; 32],
            &crypto_x25519::public_key(&entropy),
        )
        .unwrap();
        let message = request(
            &[(43, &[3, 4]), (51, &[0, 0x17]), (44, &[0, 1, 0x52])],
            0x1302,
        );
        let retry = Request::parse(&message).unwrap().unwrap();
        let (second, key) = retry.client_hello(&first, &entropy).unwrap();
        let ClientKey::P256(private) = &key else {
            panic!("P-256 required");
        };
        assert_ne!(private, &entropy);
        assert_ne!(*private, super::super::tls12_p256_secret(&entropy).unwrap());
        let (first_prefix, first_extra) = client_parts(&first);
        let (second_prefix, second_extra) = client_parts(&second);
        assert_eq!(first_prefix, second_prefix);
        for (index, (kind, bytes)) in first_extra.iter().enumerate() {
            assert_eq!(second_extra[index].0, *kind);
            if *kind != 51 {
                assert_eq!(second_extra[index].1, *bytes);
            }
        }
        let mut shares = Bytes::new(second_extra.iter().find(|(kind, _)| *kind == 51).unwrap().1);
        let mut share = Bytes::new(shares.vector16().unwrap());
        shares.finish().unwrap();
        assert_eq!(share.u16().unwrap(), P256_GROUP);
        let public = share.vector16().unwrap();
        share.finish().unwrap();
        assert_eq!(public.len(), 65);
        let mut peer_secret = [0; 32];
        peer_secret[31] = 3;
        let peer = p256::public_key(&peer_secret).unwrap();
        assert_eq!(
            key.shared_secret(&peer).unwrap(),
            p256::shared_secret(&peer_secret, public).unwrap()
        );
        assert!(key.shared_secret(&[0; 65]).is_err());
        let mut invalid = [0; 65];
        invalid[0] = 4;
        assert!(key.shared_secret(&invalid).is_err());
        assert!(ClientKey::X25519(entropy).shared_secret(&[0; 32]).is_err());
        assert!(super::super::server_hello(&message, P256_GROUP).is_err());
    }

    #[test]
    fn hello_retry_transcript_uses_the_selected_hash_and_handshake_header() {
        let first = super::super::client_hello("example.test", &[0x31; 32], &[0x42; 32]).unwrap();
        for (wire, suite) in [
            (0x1301, Suite::Aes128GcmSha256),
            (0x1302, Suite::Aes256GcmSha384),
            (0x1303, Suite::ChaCha20Poly1305Sha256),
        ] {
            let message = request(&[(43, &[3, 4]), (44, &[0, 1, 0x53])], wire);
            let retry = Request::parse(&message).unwrap().unwrap();
            let (second, _) = retry.client_hello(&first, &[0x42; 32]).unwrap();
            let mut transcript = retry.transcript(&first, &message).unwrap();
            transcript.add(&second).unwrap();
            let mut expected = vec![254, 0, 0, suite.hash_len() as u8];
            expected.extend_from_slice(&suite.hash(&first));
            expected.extend_from_slice(&message);
            expected.extend_from_slice(&second);
            assert_eq!(transcript.bytes, expected);
            assert_eq!(transcript.digest(suite), suite.hash(&expected));
            assert_ne!(
                transcript.digest(suite),
                suite.hash(&[first.clone(), message, second].concat())
            );
        }
    }
}
