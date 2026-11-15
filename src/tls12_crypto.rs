//! Original TLS 1.2 key schedules and bounded AES-GCM/CBC record states.
//! RFC 5246 sections 5, 6.2.3.3 and 6.3; RFC 5288; RFC 5289; RFC 7627.
//! Only ephemeral ECDH suites are supported. Existing original AES-GCM and
//! SHA-256/SHA-384 primitives are shared with our TLS 1.3 implementation.

use super::tls12_cbc;
use crate::crypto_aes_gcm;
use crate::tls_crypto::{self, ContentType, PlaintextRecord};

pub(super) const MAX_CONTENT_BYTES: usize = 16_384;
const MAX_GCM_CIPHERTEXT_BYTES: usize = MAX_CONTENT_BYTES + 8 + 16;
pub(super) const MAX_CIPHERTEXT_BYTES: usize = tls12_cbc::MAX_CIPHERTEXT_BYTES;
const MAX_RECORDS_PER_KEY: u64 = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Suite {
    RsaAes128GcmSha256,
    RsaAes256GcmSha384,
    EcdsaAes128GcmSha256,
    EcdsaAes256GcmSha384,
    RsaAes128CbcSha256,
    RsaAes256CbcSha384,
}

impl Suite {
    pub(super) fn from_id(id: u16) -> Result<Self, String> {
        match id {
            0xc02f => Ok(Self::RsaAes128GcmSha256),
            0xc030 => Ok(Self::RsaAes256GcmSha384),
            0xc02b => Ok(Self::EcdsaAes128GcmSha256),
            0xc02c => Ok(Self::EcdsaAes256GcmSha384),
            0xc027 => Ok(Self::RsaAes128CbcSha256),
            0xc028 => Ok(Self::RsaAes256CbcSha384),
            _ => Err("The server selected an unsupported TLS 1.2 cipher; authenticated ECDHE with a supported AES suite is required.".into()),
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::RsaAes128GcmSha256 => "ECDHE-RSA / AES-128-GCM / SHA-256",
            Self::RsaAes256GcmSha384 => "ECDHE-RSA / AES-256-GCM / SHA-384",
            Self::EcdsaAes128GcmSha256 => "ECDHE-ECDSA / AES-128-GCM / SHA-256",
            Self::EcdsaAes256GcmSha384 => "ECDHE-ECDSA / AES-256-GCM / SHA-384",
            Self::RsaAes128CbcSha256 => "ECDHE-RSA / AES-128-CBC / SHA-256",
            Self::RsaAes256CbcSha384 => "ECDHE-RSA / AES-256-CBC / SHA-384",
        }
    }

    pub(super) fn uses_rsa(self) -> bool {
        matches!(
            self,
            Self::RsaAes128GcmSha256
                | Self::RsaAes256GcmSha384
                | Self::RsaAes128CbcSha256
                | Self::RsaAes256CbcSha384
        )
    }

    pub(super) fn uses_cbc(self) -> bool {
        matches!(self, Self::RsaAes128CbcSha256 | Self::RsaAes256CbcSha384)
    }

    pub(super) fn key_len(self) -> usize {
        match self {
            Self::RsaAes128GcmSha256 | Self::EcdsaAes128GcmSha256 | Self::RsaAes128CbcSha256 => 16,
            _ => 32,
        }
    }

    fn primitive(self) -> tls_crypto::Suite {
        if self.key_len() == 16 {
            tls_crypto::Suite::Aes128GcmSha256
        } else {
            tls_crypto::Suite::Aes256GcmSha384
        }
    }

    pub(super) fn hash(self, bytes: &[u8]) -> Vec<u8> {
        self.primitive().hash(bytes)
    }

    fn check_hash(self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() == self.primitive().hash_len() {
            Ok(())
        } else {
            Err("TLS 1.2 transcript hash has the wrong length for the cipher suite.".into())
        }
    }
}

fn prf(
    suite: Suite,
    secret: &[u8],
    label: &str,
    seed: &[u8],
    length: usize,
) -> Result<Vec<u8>, String> {
    if secret.is_empty()
        || secret.len() > 512
        || seed.len() > 512
        || length > 512
        || label.is_empty()
        || label.len() > 128
        || !label.is_ascii()
        || label.as_bytes().contains(&0)
    {
        return Err("TLS 1.2 PRF input is invalid or exceeds its bounds.".into());
    }
    let primitive = suite.primitive();
    let mut combined = Vec::with_capacity(label.len() + seed.len());
    combined.extend_from_slice(label.as_bytes());
    combined.extend_from_slice(seed);
    let mut a = primitive.hmac(secret, &combined);
    let mut output = Vec::with_capacity(length);
    while output.len() < length {
        let mut input = Vec::with_capacity(a.len() + combined.len());
        input.extend_from_slice(&a);
        input.extend_from_slice(&combined);
        let block = primitive.hmac(secret, &input);
        output.extend_from_slice(&block[..block.len().min(length - output.len())]);
        a = primitive.hmac(secret, &a);
    }
    Ok(output)
}

pub(super) fn master_secret(
    suite: Suite,
    premaster: &[u8],
    session_hash: &[u8],
) -> Result<[u8; 48], String> {
    suite.check_hash(session_hash)?;
    if premaster.len() != 32 {
        return Err("TLS 1.2 ECDH premaster secret must contain 32 bytes.".into());
    }
    prf(suite, premaster, "extended master secret", session_hash, 48)?
        .try_into()
        .map_err(|_| "Invalid TLS 1.2 master secret length.".into())
}

/// RFC 5246's original full-handshake derivation, used only when the peer
/// declines RFC 7627. This connection cannot resume, renegotiate, authenticate
/// a client certificate, or export channel-binding/key material.
pub(super) fn legacy_master_secret(
    suite: Suite,
    premaster: &[u8],
    client_random: &[u8; 32],
    server_random: &[u8; 32],
) -> Result<[u8; 48], String> {
    if premaster.len() != 32 {
        return Err("TLS 1.2 ECDH premaster secret must contain 32 bytes.".into());
    }
    let mut seed = [0; 64];
    seed[..32].copy_from_slice(client_random);
    seed[32..].copy_from_slice(server_random);
    prf(suite, premaster, "master secret", &seed, 48)?
        .try_into()
        .map_err(|_| "Invalid TLS 1.2 master secret length.".into())
}

pub(super) fn finished(
    suite: Suite,
    master: &[u8; 48],
    label: &str,
    handshake_hash: &[u8],
) -> Result<[u8; 12], String> {
    suite.check_hash(handshake_hash)?;
    if !matches!(label, "client finished" | "server finished") {
        return Err("Invalid TLS 1.2 Finished label.".into());
    }
    prf(suite, master, label, handshake_hash, 12)?
        .try_into()
        .map_err(|_| "Invalid TLS 1.2 Finished length.".into())
}

pub(super) fn verify_finished(
    suite: Suite,
    master: &[u8; 48],
    label: &str,
    handshake_hash: &[u8],
    received: &[u8],
) -> Result<(), String> {
    if received.len() != 12 {
        return Err("TLS 1.2 Finished must contain exactly 12 bytes.".into());
    }
    let expected = finished(suite, master, label, handshake_hash)?;
    if expected
        .iter()
        .zip(received)
        .fold(0u8, |diff, (a, b)| diff | (a ^ b))
        != 0
    {
        return Err("TLS 1.2 Finished verification failed.".into());
    }
    Ok(())
}

// No Clone/Copy/Debug, no public constructor, and no sequence reset. The full
// handshake creates fresh directional keys. Errors permanently poison a state.
struct RecordKey {
    key: Vec<u8>,
    salt: [u8; 4],
    cbc: Option<tls12_cbc::Keys>,
    sequence: u64,
    failed: bool,
}

impl RecordKey {
    fn new(key: &[u8], salt: &[u8]) -> Result<Self, String> {
        if !matches!(key.len(), 16 | 32) || salt.len() != 4 {
            return Err("Invalid TLS 1.2 AES-GCM key material.".into());
        }
        let mut fixed = [0; 4];
        fixed.copy_from_slice(salt);
        Ok(Self {
            key: key.to_vec(),
            salt: fixed,
            cbc: None,
            sequence: 0,
            failed: false,
        })
    }

    fn cbc(keys: tls12_cbc::Keys) -> Self {
        Self {
            key: Vec::new(),
            salt: [0; 4],
            cbc: Some(keys),
            sequence: 0,
            failed: false,
        }
    }

    fn ready(&self) -> Result<(), String> {
        if self.failed {
            return Err("TLS 1.2 record state is unusable after an earlier failure.".into());
        }
        if self.sequence >= MAX_RECORDS_PER_KEY {
            return Err("TLS 1.2 connection reached its authenticated-record limit; a fresh connection is required.".into());
        }
        Ok(())
    }

    fn nonce(&self, explicit: &[u8]) -> [u8; 12] {
        let mut nonce = [0; 12];
        nonce[..4].copy_from_slice(&self.salt);
        nonce[4..].copy_from_slice(explicit);
        nonce
    }

    fn aad(&self, kind: ContentType, length: usize) -> [u8; 13] {
        let mut aad = [0; 13];
        aad[..8].copy_from_slice(&self.sequence.to_be_bytes());
        aad[8] = kind as u8;
        aad[9..11].copy_from_slice(&[3, 3]);
        aad[11..].copy_from_slice(&(length as u16).to_be_bytes());
        aad
    }
}

pub(super) struct Sealer(RecordKey);
pub(super) struct Opener(RecordKey);

pub(super) fn key_pair(
    suite: Suite,
    master: &[u8; 48],
    client_random: &[u8; 32],
    server_random: &[u8; 32],
    encrypt_then_mac: bool,
) -> Result<(Sealer, Opener), String> {
    let mut seed = [0; 64];
    seed[..32].copy_from_slice(server_random);
    seed[32..].copy_from_slice(client_random);
    let k = suite.key_len();
    if suite.uses_cbc() {
        let m = suite.primitive().hash_len();
        // RFC 5246: MAC keys, then cipher keys. TLS 1.2 CBC uses explicit
        // per-record IVs, so no implicit IV bytes belong to this key block.
        let mut block = prf(suite, master, "key expansion", &seed, 2 * m + 2 * k)?;
        // Locally generated IVs need not be derived by the peer. A separate
        // PRF label supplies independent unpredictable directional IV keys;
        // no TLS encryption/MAC key is reused for IV generation.
        let mut iv_keys = prf(suite, master, "OwnBrowser TLS 1.2 CBC IV keys", &seed, 64)?;
        let writer = tls12_cbc::Keys::new(
            &block[2 * m..2 * m + k],
            &block[..m],
            iv_keys[..32]
                .try_into()
                .map_err(|_| "Invalid CBC IV key.")?,
            k == 32,
            encrypt_then_mac,
        )?;
        let reader = tls12_cbc::Keys::new(
            &block[2 * m + k..],
            &block[m..2 * m],
            iv_keys[32..]
                .try_into()
                .map_err(|_| "Invalid CBC IV key.")?,
            k == 32,
            encrypt_then_mac,
        )?;
        block.fill(0);
        iv_keys.fill(0);
        return Ok((
            Sealer(RecordKey::cbc(writer)),
            Opener(RecordKey::cbc(reader)),
        ));
    }
    if encrypt_then_mac {
        return Err("Encrypt-then-MAC cannot be selected with an AEAD cipher.".into());
    }
    let mut block = prf(suite, master, "key expansion", &seed, 2 * k + 8)?;
    let writer = RecordKey::new(&block[..k], &block[2 * k..2 * k + 4])?;
    let reader = RecordKey::new(&block[k..2 * k], &block[2 * k + 4..])?;
    block.fill(0);
    Ok((Sealer(writer), Opener(reader)))
}

impl Sealer {
    pub(super) fn seal(&mut self, kind: ContentType, content: &[u8]) -> Result<Vec<u8>, String> {
        let result = (|| {
            self.0.ready()?;
            if let Some(cbc) = &self.0.cbc {
                let wire = cbc.seal(self.0.sequence, kind, content)?;
                self.0.sequence += 1;
                return Ok(wire);
            }
            if content.len() > MAX_CONTENT_BYTES
                || (kind == ContentType::Alert && content.len() != 2)
                || (kind == ContentType::Handshake && content.is_empty())
            {
                return Err("Invalid TLS 1.2 plaintext record length.".into());
            }
            let explicit = self.0.sequence.to_be_bytes();
            let encrypted = crypto_aes_gcm::seal(
                &self.0.key,
                &self.0.nonce(&explicit),
                &self.0.aad(kind, content.len()),
                content,
            )?;
            let length = 8 + encrypted.len();
            let mut record = Vec::with_capacity(5 + length);
            record.extend_from_slice(&[kind as u8, 3, 3]);
            record.extend_from_slice(&(length as u16).to_be_bytes());
            record.extend_from_slice(&explicit);
            record.extend_from_slice(&encrypted);
            self.0.sequence += 1;
            Ok(record)
        })();
        if result.is_err() {
            self.0.failed = true;
        }
        result
    }
}

impl Opener {
    pub(super) fn open(&mut self, record: &[u8]) -> Result<PlaintextRecord, String> {
        let result = (|| {
            self.0.ready()?;
            if let Some(cbc) = &self.0.cbc {
                let plain = cbc.open(self.0.sequence, record)?;
                self.0.sequence += 1;
                return Ok(plain);
            }
            if record.len() < 5 + 8 + 16 || record.len() > 5 + MAX_GCM_CIPHERTEXT_BYTES {
                return Err("Invalid TLS 1.2 encrypted record length.".into());
            }
            if record[1..3] != [3, 3]
                || usize::from(u16::from_be_bytes([record[3], record[4]])) != record.len() - 5
            {
                return Err("Invalid TLS 1.2 record version or length field.".into());
            }
            let kind = match record[0] {
                21 => ContentType::Alert,
                22 => ContentType::Handshake,
                23 => ContentType::ApplicationData,
                _ => return Err("Unexpected encrypted TLS 1.2 record type.".into()),
            };
            let length = record.len() - 5 - 8 - 16;
            let content = crypto_aes_gcm::open(
                &self.0.key,
                &self.0.nonce(&record[5..13]),
                &self.0.aad(kind, length),
                &record[13..],
            )
            .map_err(|error| format!("TLS 1.2 record authentication failed: {error}"))?;
            if kind == ContentType::Handshake && content.is_empty() {
                return Err("An authenticated TLS 1.2 Handshake record cannot be empty.".into());
            }
            if kind == ContentType::Alert && (content.len() != 2 || !matches!(content[0], 1 | 2)) {
                return Err("Malformed authenticated TLS 1.2 alert.".into());
            }
            self.0.sequence += 1;
            Ok(PlaintextRecord {
                content_type: kind,
                content,
            })
        })();
        if result.is_err() {
            self.0.failed = true;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn prf_matches_independent_python_hmac_vectors() {
        let secret: Vec<u8> = (0..32).collect();
        let seed: Vec<u8> = (0..64).collect();
        let vectors = [
            (Suite::RsaAes128GcmSha256, "58caff8e32d9556f6565a2763fbbcdb7a933c13e82ac8b5bad4ad985d4dd2a5af9f82be7b5c2560bf7b30b12b2b01d06c2387dd40d2e1f70789bae8c29aa4f407ccdc0a595c2c85f807cbc77359798ae7225fa6ecdcbab287a0a1846c8f9a2e5"),
            (Suite::EcdsaAes256GcmSha384, "29a13387c7ecd842fa24152805a5363204ddf37f39655be9528e2862e5c2b5a4ef48328d824ebd2aea2576113274f62f17d89a13c2c97a88f92c8b940d7f0940ccaf5c0d11713f04f7e9ad4b82c66fff83ad394554e185f1176e9d3c30e76d3c"),
        ];
        for (suite, expected) in vectors {
            assert_eq!(
                hex(&prf(suite, &secret, "original comparison vector", &seed, 96).unwrap()),
                expected
            );
            assert!(prf(suite, &secret, "x", &seed, 513).is_err());
            assert!(master_secret(suite, &secret, &[0; 31]).is_err());
        }
    }

    #[test]
    fn transcript_bound_master_and_finished_reject_changes() {
        for id in [0xc02f, 0xc030, 0xc02b, 0xc02c, 0xc027, 0xc028] {
            let suite = Suite::from_id(id).unwrap();
            let hash = suite.hash(b"hello through ClientKeyExchange");
            let master = master_secret(suite, &[9; 32], &hash).unwrap();
            assert_ne!(
                master,
                master_secret(suite, &[9; 32], &suite.hash(b"changed")).unwrap()
            );
            let proof = finished(suite, &master, "server finished", &hash).unwrap();
            verify_finished(suite, &master, "server finished", &hash, &proof).unwrap();
            assert!(verify_finished(suite, &master, "client finished", &hash, &proof).is_err());
            assert!(
                verify_finished(suite, &master, "server finished", &hash, &proof[..11]).is_err()
            );
            let mut bad = proof;
            bad[11] ^= 1;
            assert!(verify_finished(suite, &master, "server finished", &hash, &bad).is_err());
        }
        for id in [0x1301, 0x009c, 0xc013, 0] {
            assert!(Suite::from_id(id).is_err());
        }
    }

    #[test]
    fn legacy_master_matches_independent_hmac_and_binds_both_randoms() {
        let secret: Vec<u8> = (0..32).collect();
        for (suite, expected) in [
            (Suite::RsaAes128GcmSha256, "2e413aedf058ebcd3881af6a4485e816a3f23f79fd451ef2fc8fb60708a9ffd4396698b67905bc6caf5f10bdf96413b3"),
            (Suite::RsaAes256CbcSha384, "f456edb31256b7d31ce4e7273a24b9d6e29e3f5dd41d9ab9eca013121bc5bb9da6044cbab314beb462e4cfd61e9478a5"),
        ] {
            let master = legacy_master_secret(suite, &secret, &[0x31; 32], &[0x52; 32]).unwrap();
            assert_eq!(hex(&master), expected);
            assert_ne!(master, legacy_master_secret(suite, &secret, &[0x32; 32], &[0x52; 32]).unwrap());
            assert_ne!(master, legacy_master_secret(suite, &secret, &[0x31; 32], &[0x53; 32]).unwrap());
            assert_ne!(master, master_secret(suite, &secret, &suite.hash(b"transcript")).unwrap());
            assert!(legacy_master_secret(suite, &secret[..31], &[0; 32], &[0; 32]).is_err());
        }
    }

    #[test]
    fn cbc_wrapper_poisoning_replay_limits_and_directional_keys() {
        for etm in [false, true] {
            let create = || {
                RecordKey::cbc(
                    tls12_cbc::Keys::new(&[1; 16], &[2; 32], [3; 32], false, etm).unwrap(),
                )
            };
            let mut writer = Sealer(create());
            let mut reader = Opener(create());
            let wire = writer
                .seal(ContentType::ApplicationData, b"authenticated")
                .unwrap();
            assert_eq!(reader.open(&wire).unwrap().content, b"authenticated");
            assert!(reader.open(&wire).is_err());
            assert!(reader
                .open(
                    &writer
                        .seal(ContentType::ApplicationData, b"after replay")
                        .unwrap()
                )
                .is_err());
            let mut writer = Sealer(create());
            writer.0.sequence = MAX_RECORDS_PER_KEY - 1;
            assert!(writer.seal(ContentType::ApplicationData, b"last").is_ok());
            assert!(writer
                .seal(ContentType::ApplicationData, b"exhausted")
                .is_err());
            let (mut client_writer, mut server_reader) =
                key_pair(Suite::RsaAes128CbcSha256, &[4; 48], &[5; 32], &[6; 32], etm).unwrap();
            // Opposite directions never share encryption/MAC keys.
            let reflected = client_writer
                .seal(ContentType::ApplicationData, b"reflection")
                .unwrap();
            assert!(server_reader.open(&reflected).is_err());
        }
        assert!(key_pair(
            Suite::RsaAes128GcmSha256,
            &[0; 48],
            &[1; 32],
            &[2; 32],
            true
        )
        .is_err());
    }

    #[test]
    fn record_authentication_binds_sequence_type_version_and_length() {
        for size in [0, 1, 16_384] {
            let mut writer = Sealer(RecordKey::new(&[0x21; 16], &[0x32; 4]).unwrap());
            let mut reader = Opener(RecordKey::new(&[0x21; 16], &[0x32; 4]).unwrap());
            let plain = vec![0x45; size];
            let wire = writer.seal(ContentType::ApplicationData, &plain).unwrap();
            assert_eq!(reader.open(&wire).unwrap().content, plain);
            assert!(reader.open(&wire).is_err());
            assert!(reader
                .open(&writer.seal(ContentType::ApplicationData, b"later").unwrap())
                .is_err());
            for index in [0, 1, 3, 5, wire.len() - 1] {
                let mut changed = wire.clone();
                changed[index] ^= 1;
                let mut reader = Opener(RecordKey::new(&[0x21; 16], &[0x32; 4]).unwrap());
                assert!(reader.open(&changed).is_err());
                assert!(reader.open(&wire).is_err());
            }
        }
    }

    #[test]
    fn record_bounds_and_sequence_exhaustion_fail_closed() {
        let mut writer = Sealer(RecordKey::new(&[1; 32], &[2; 4]).unwrap());
        assert!(writer
            .seal(ContentType::ApplicationData, &[0; MAX_CONTENT_BYTES + 1])
            .is_err());
        assert!(writer.seal(ContentType::ApplicationData, b"retry").is_err());
        let mut writer = Sealer(RecordKey::new(&[1; 32], &[2; 4]).unwrap());
        writer.0.sequence = MAX_RECORDS_PER_KEY - 1;
        assert!(writer.seal(ContentType::ApplicationData, b"last").is_ok());
        assert!(writer
            .seal(ContentType::ApplicationData, b"exhausted")
            .is_err());
        let mut writer = Sealer(RecordKey::new(&[1; 32], &[2; 4]).unwrap());
        assert!(writer.seal(ContentType::Alert, &[1]).is_err());
    }
}
