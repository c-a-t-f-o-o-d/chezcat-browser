//! Original TLS 1.3 key schedule and authenticated record states.
//! The selected cipher suite determines its hash, key length and record cipher.
//! The connection layer supplies transcripts and owns key-epoch transitions.
//! References: RFC 9846 sections 4.5.3, 5 and 7; RFC 8448 test transcripts.

use crate::crypto_hash::{hkdf_expand, hkdf_extract, hmac_sha256, sha256};
use crate::{crypto_aead, crypto_aes_gcm, crypto_sha512};

pub const MAX_CONTENT_BYTES: usize = 1 << 14;
pub const MAX_INNER_BYTES: usize = MAX_CONTENT_BYTES + 1;
const TAG_BYTES: usize = 16;
pub const MAX_CIPHERTEXT_BYTES: usize = MAX_INNER_BYTES + TAG_BYTES;
pub const RECORD_HEADER_BYTES: usize = 5;
/// Conservative per-key cap for all supported suites. The connection layer
/// sends KeyUpdate under the old key before constructing the next key epoch.
pub const MAX_RECORDS_PER_KEY: u64 = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suite {
    ChaCha20Poly1305Sha256,
    Aes128GcmSha256,
    Aes256GcmSha384,
}

impl Suite {
    pub fn from_wire(value: u16) -> Result<Self, String> {
        match value {
            0x1301 => Ok(Self::Aes128GcmSha256),
            0x1302 => Ok(Self::Aes256GcmSha384),
            0x1303 => Ok(Self::ChaCha20Poly1305Sha256),
            _ => Err("The server selected an unsupported or unoffered TLS cipher suite.".into()),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::ChaCha20Poly1305Sha256 => "ChaCha20-Poly1305 / SHA-256",
            Self::Aes128GcmSha256 => "AES-128-GCM / SHA-256",
            Self::Aes256GcmSha384 => "AES-256-GCM / SHA-384",
        }
    }
    pub fn hash_len(self) -> usize {
        match self {
            Self::Aes256GcmSha384 => 48,
            _ => 32,
        }
    }
    fn key_len(self) -> usize {
        match self {
            Self::Aes128GcmSha256 => 16,
            _ => 32,
        }
    }
    pub fn hash(self, input: &[u8]) -> Vec<u8> {
        match self {
            Self::Aes256GcmSha384 => crypto_sha512::sha384(input).to_vec(),
            _ => sha256(input).to_vec(),
        }
    }
    pub(crate) fn hmac(self, key: &[u8], input: &[u8]) -> Vec<u8> {
        if self.hash_len() == 32 {
            return hmac_sha256(key, input).to_vec();
        }
        // SHA-384 uses SHA-512's 128-byte compression block. All callers have
        // bounded TLS key-schedule inputs and CBC record MAC inputs (<= 16,717 bytes).
        let mut block = [0u8; 128];
        if key.len() > block.len() {
            block[..48].copy_from_slice(&crypto_sha512::sha384(key));
        } else {
            block[..key.len()].copy_from_slice(key);
        }
        let mut inner = Vec::with_capacity(128 + input.len());
        inner.extend(block.iter().map(|byte| byte ^ 0x36));
        inner.extend_from_slice(input);
        let digest = crypto_sha512::sha384(&inner);
        let mut outer = [0u8; 176];
        for (target, byte) in outer[..128].iter_mut().zip(block) {
            *target = byte ^ 0x5c;
        }
        outer[128..].copy_from_slice(&digest);
        crypto_sha512::sha384(&outer).to_vec()
    }
    fn extract(self, salt: &[u8], input: &[u8]) -> Vec<u8> {
        match self {
            Self::Aes256GcmSha384 => self.hmac(salt, input),
            _ => hkdf_extract(salt, input).to_vec(),
        }
    }
    fn check_secret(self, secret: &[u8]) -> Result<(), String> {
        if secret.len() != self.hash_len() {
            Err("TLS secret or transcript hash length does not match its selected suite.".into())
        } else {
            Ok(())
        }
    }
}

/// RFC 9846 HKDF-Expand-Label. Secret, output bounds and hash are suite-specific.
pub fn hkdf_expand_label(
    suite: Suite,
    secret: &[u8],
    label: &str,
    context: &[u8],
    length: usize,
) -> Result<Vec<u8>, String> {
    suite.check_secret(secret)?;
    if label.is_empty() || label.len() > 249 || !label.is_ascii() || label.as_bytes().contains(&0) {
        return Err("TLS HKDF labels need 1 to 249 ASCII bytes without NUL.".into());
    }
    if context.len() > 255 {
        return Err("TLS HKDF context cannot exceed 255 bytes.".into());
    }
    if length > 255 * suite.hash_len() {
        return Err("TLS HKDF output exceeds 255 hash-sized blocks.".into());
    }
    let mut info = [0u8; 514];
    info[..2].copy_from_slice(&(length as u16).to_be_bytes());
    info[2] = (6 + label.len()) as u8;
    info[3..9].copy_from_slice(b"tls13 ");
    info[9..9 + label.len()].copy_from_slice(label.as_bytes());
    let context_at = 9 + label.len();
    info[context_at] = context.len() as u8;
    let info_len = context_at + 1 + context.len();
    info[context_at + 1..info_len].copy_from_slice(context);
    if suite.hash_len() == 32 {
        let fixed: &[u8; 32] = secret
            .try_into()
            .map_err(|_| "TLS SHA-256 secret must contain 32 bytes.")?;
        return hkdf_expand(fixed, &info[..info_len], length);
    }
    let mut result = Vec::with_capacity(length);
    let mut previous = Vec::new();
    for counter in 1..=length.div_ceil(suite.hash_len()) {
        let mut input = Vec::with_capacity(previous.len() + info_len + 1);
        input.extend_from_slice(&previous);
        input.extend_from_slice(&info[..info_len]);
        input.push(counter as u8);
        previous = suite.hmac(secret, &input);
        result.extend_from_slice(&previous[..previous.len().min(length - result.len())]);
    }
    Ok(result)
}

fn expand_fixed<const N: usize>(
    suite: Suite,
    secret: &[u8],
    label: &str,
    context: &[u8],
) -> Result<[u8; N], String> {
    hkdf_expand_label(suite, secret, label, context, N)?
        .try_into()
        .map_err(|_| "TLS HKDF returned an unexpected output length.".into())
}

/// The caller passes the already-hashed transcript, including handshake headers.
pub fn derive_secret(
    suite: Suite,
    secret: &[u8],
    label: &str,
    transcript_hash: &[u8],
) -> Result<Vec<u8>, String> {
    suite.check_secret(transcript_hash)?;
    hkdf_expand_label(suite, secret, label, transcript_hash, suite.hash_len())
}

pub fn early_secret(suite: Suite) -> Vec<u8> {
    let zero = vec![0u8; suite.hash_len()];
    suite.extract(&zero, &zero)
}

pub fn handshake_secret(suite: Suite, early: &[u8], shared: &[u8; 32]) -> Result<Vec<u8>, String> {
    let salt = derive_secret(suite, early, "derived", &suite.hash(b""))?;
    Ok(suite.extract(&salt, shared))
}

pub fn main_secret(suite: Suite, handshake: &[u8]) -> Result<Vec<u8>, String> {
    let salt = derive_secret(suite, handshake, "derived", &suite.hash(b""))?;
    Ok(suite.extract(&salt, &vec![0u8; suite.hash_len()]))
}

pub fn finished_verify_data(
    suite: Suite,
    traffic: &[u8],
    transcript_hash: &[u8],
) -> Result<Vec<u8>, String> {
    suite.check_secret(transcript_hash)?;
    let key = hkdf_expand_label(suite, traffic, "finished", b"", suite.hash_len())?;
    Ok(suite.hmac(&key, transcript_hash))
}

pub fn verify_finished(
    suite: Suite,
    traffic: &[u8],
    transcript_hash: &[u8],
    received: &[u8],
) -> Result<(), String> {
    if received.len() != suite.hash_len() {
        return Err("TLS Finished.verify_data has the wrong length for the selected suite.".into());
    }
    let expected = finished_verify_data(suite, traffic, transcript_hash)?;
    if expected
        .iter()
        .zip(received)
        .fold(0u8, |diff, (a, b)| diff | (a ^ b))
        != 0
    {
        return Err("TLS Finished verification failed.".into());
    }
    Ok(())
}

pub fn next_traffic_secret(suite: Suite, secret: &[u8]) -> Result<Vec<u8>, String> {
    hkdf_expand_label(suite, secret, "traffic upd", b"", suite.hash_len())
}

/// Only content types permitted inside protected records in this subset.
/// ChangeCipherSpec is a separate plaintext compatibility record, never an
/// encrypted inner type, and is intentionally absent here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ContentType {
    Alert = 21,
    Handshake = 22,
    ApplicationData = 23,
}

impl ContentType {
    fn decode(byte: u8) -> Result<Self, String> {
        match byte {
            21 => Ok(Self::Alert),
            22 => Ok(Self::Handshake),
            23 => Ok(Self::ApplicationData),
            _ => Err("Unexpected TLS inner content type.".into()),
        }
    }
}

pub struct PlaintextRecord {
    pub content_type: ContentType,
    pub content: Vec<u8>,
}

// Intentionally no Clone/Copy/Debug and no sequence setter or reset method.
struct RecordKey {
    suite: Suite,
    key: [u8; 32],
    iv: [u8; 12],
    sequence: u64,
    failed: bool,
}

impl RecordKey {
    fn new(suite: Suite, traffic_secret: &[u8]) -> Result<Self, String> {
        let material = hkdf_expand_label(suite, traffic_secret, "key", b"", suite.key_len())?;
        let mut key = [0u8; 32];
        key[..material.len()].copy_from_slice(&material);
        Ok(Self {
            suite,
            key,
            iv: expand_fixed(suite, traffic_secret, "iv", b"")?,
            sequence: 0,
            failed: false,
        })
    }

    fn begin(&mut self) -> Result<[u8; 12], String> {
        if self.failed {
            return Err("TLS record state is unusable after an earlier error.".into());
        }
        // Any subsequent early return leaves the state permanently poisoned,
        // even if malformed framing prevented use of the reserved nonce.
        self.failed = true;
        if self.sequence >= MAX_RECORDS_PER_KEY {
            return Err(
                "TLS record key usage limit reached; fresh traffic keys are required.".into(),
            );
        }
        let nonce = record_nonce(&self.iv, self.sequence);
        self.sequence += 1; // Strict cap above makes overflow impossible.
        Ok(nonce)
    }
}

fn record_nonce(iv: &[u8; 12], sequence: u64) -> [u8; 12] {
    let mut nonce = *iv;
    for (target, byte) in nonce[4..].iter_mut().zip(sequence.to_be_bytes()) {
        *target ^= byte;
    }
    nonce
}

fn validate_content(content_type: ContentType, length: usize) -> Result<(), String> {
    if length > MAX_CONTENT_BYTES {
        return Err("TLS content exceeds the 16384-byte record limit.".into());
    }
    match content_type {
        ContentType::Alert if length != 2 => {
            Err("A TLS Alert record must contain exactly one two-byte alert.".into())
        }
        ContentType::Handshake if length == 0 => {
            Err("A TLS Handshake record cannot contain an empty fragment.".into())
        }
        _ => Ok(()),
    }
}

/// One sending direction in one key epoch. Construct only once for a particular
/// traffic secret; reconstructing it with a previously used secret would reuse
/// nonces. Different directions must use distinct traffic secrets.
pub struct Sealer {
    state: RecordKey,
}

impl Sealer {
    pub fn new(suite: Suite, traffic_secret: &[u8]) -> Result<Self, String> {
        Ok(Self {
            state: RecordKey::new(suite, traffic_secret)?,
        })
    }

    #[allow(dead_code)] // Also exposed to the independent developer probe.
    pub fn is_failed(&self) -> bool {
        self.state.failed
    }

    /// Return exactly one wire record, including its five-byte header.
    /// Padding is an explicit count of zero bytes after the inner type.
    /// Every error permanently disables this instance; no retry/reset exists.
    pub fn seal(
        &mut self,
        content_type: ContentType,
        content: &[u8],
        padding: usize,
    ) -> Result<Vec<u8>, String> {
        let nonce = self.state.begin()?;
        validate_content(content_type, content.len())?;
        let inner_length = content
            .len()
            .checked_add(1)
            .and_then(|length| length.checked_add(padding))
            .filter(|&length| length <= MAX_INNER_BYTES)
            .ok_or("TLS content, type, and padding exceed the 16385-byte inner limit.")?;
        let encrypted_length = inner_length + TAG_BYTES;
        let [high, low] = (encrypted_length as u16).to_be_bytes();
        let header = [ContentType::ApplicationData as u8, 3, 3, high, low];

        let mut inner = Vec::new();
        inner
            .try_reserve_exact(inner_length)
            .map_err(|_| "Cannot allocate TLS record plaintext.")?;
        inner.extend_from_slice(content);
        inner.push(content_type as u8);
        inner.resize(inner_length, 0);
        let encrypted = match self.state.suite {
            Suite::ChaCha20Poly1305Sha256 => {
                crypto_aead::seal(&self.state.key, &nonce, &header, &inner)
            }
            suite => {
                crypto_aes_gcm::seal(&self.state.key[..suite.key_len()], &nonce, &header, &inner)
            }
        }?;
        let mut record = Vec::new();
        record
            .try_reserve_exact(RECORD_HEADER_BYTES + encrypted_length)
            .map_err(|_| "Cannot allocate TLS wire record.")?;
        record.extend_from_slice(&header);
        record.extend_from_slice(&encrypted);
        self.state.failed = false;
        Ok(record)
    }
}

/// One receiving direction in one key epoch. This is an exact-record API:
/// the future network framer must supply one complete record, without extra
/// bytes. Authentication/framing/content failures permanently disable it.
pub struct Opener {
    state: RecordKey,
}

impl Opener {
    pub fn new(suite: Suite, traffic_secret: &[u8]) -> Result<Self, String> {
        Ok(Self {
            state: RecordKey::new(suite, traffic_secret)?,
        })
    }

    #[allow(dead_code)] // Also exposed to the independent developer probe.
    pub fn is_failed(&self) -> bool {
        self.state.failed
    }

    pub fn open(&mut self, record: &[u8]) -> Result<PlaintextRecord, String> {
        let nonce = self.state.begin()?;
        if record.len() < RECORD_HEADER_BYTES {
            return Err("Incomplete TLS ciphertext header.".into());
        }
        let header = &record[..RECORD_HEADER_BYTES];
        if header[..3] != [ContentType::ApplicationData as u8, 3, 3] {
            return Err("Expected a protected TLS 1.3 record header (17 03 03).".into());
        }
        let encrypted_length = usize::from(u16::from_be_bytes([header[3], header[4]]));
        if !(TAG_BYTES + 1..=MAX_CIPHERTEXT_BYTES).contains(&encrypted_length) {
            return Err("Invalid TLS AEAD ciphertext length.".into());
        }
        if record.len() != RECORD_HEADER_BYTES + encrypted_length {
            return Err("TLS record length does not match its header.".into());
        }

        // AEAD checks its tag before allocating/decrypting plaintext. Only
        // authenticated bytes reach the inner type/padding parser below.
        let encrypted = &record[RECORD_HEADER_BYTES..];
        let mut inner = match self.state.suite {
            Suite::ChaCha20Poly1305Sha256 => {
                crypto_aead::open(&self.state.key, &nonce, header, encrypted)
            }
            suite => crypto_aes_gcm::open(
                &self.state.key[..suite.key_len()],
                &nonce,
                header,
                encrypted,
            ),
        }?;
        let type_position = inner
            .iter()
            .rposition(|&byte| byte != 0)
            .ok_or("TLS inner plaintext has no nonzero content type.")?;
        let content_type = ContentType::decode(inner[type_position])?;
        validate_content(content_type, type_position)?;
        inner.truncate(type_position);
        self.state.failed = false;
        Ok(PlaintextRecord {
            content_type,
            content: inner,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        let compact: String = text.chars().filter(|c| !c.is_ascii_whitespace()).collect();
        (0..compact.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&compact[index..index + 2], 16).unwrap())
            .collect()
    }

    fn array<const N: usize>(text: &str) -> [u8; N] {
        hex(text).try_into().unwrap()
    }

    #[test]
    fn rfc8448_no_psk_sha256_key_schedule() {
        let early = early_secret(Suite::ChaCha20Poly1305Sha256);
        assert_eq!(
            early,
            array::<32>("33ad0a1c607ec03b09e6cd9893680ce2 10adf300aa1f2660e1b22e10f170f92a")
        );
        assert_eq!(
            derive_secret(
                Suite::ChaCha20Poly1305Sha256,
                &early,
                "derived",
                &sha256(b"")
            )
            .unwrap(),
            array::<32>("6f2615a108c702c5678f54fc9dbab697 16c076189c48250cebeac3576c3611ba")
        );
        let shared =
            array::<32>("8bd4054fb55b9d63fdfbacf9f04b9f0d 35e6d63f537563efd46272900f89492d");
        let handshake = handshake_secret(Suite::ChaCha20Poly1305Sha256, &early, &shared).unwrap();
        assert_eq!(
            handshake,
            array::<32>("1dc826e93606aa6fdc0aadc12f741b01 046aa6b99f691ed221a9f0ca043fbeac")
        );
        let hello_hash =
            array::<32>("860c06edc07858ee8e78f0e7428c58ed d6b43f2ca3e6e95f02ed063cf0e1cad8");
        let client_hs = derive_secret(
            Suite::ChaCha20Poly1305Sha256,
            &handshake,
            "c hs traffic",
            &hello_hash,
        )
        .unwrap();
        let server_hs = derive_secret(
            Suite::ChaCha20Poly1305Sha256,
            &handshake,
            "s hs traffic",
            &hello_hash,
        )
        .unwrap();
        assert_eq!(
            client_hs,
            array::<32>("b3eddb126e067f35a780b3abf45e2d8f 3b1a950738f52e9600746a0e27a55a21")
        );
        assert_eq!(
            server_hs,
            array::<32>("b67b7d690cc16c4e75e54213cb2d37b4 e9c912bcded9105d42befd59d391ad38")
        );
        assert_eq!(
            derive_secret(
                Suite::ChaCha20Poly1305Sha256,
                &handshake,
                "derived",
                &sha256(b"")
            )
            .unwrap(),
            array::<32>("43de77e0c77713859a944db9db2590b5 3190a65b3ee2e4f12dd7a0bb7ce254b4")
        );
        let main = main_secret(Suite::ChaCha20Poly1305Sha256, &handshake).unwrap();
        assert_eq!(
            main,
            array::<32>("18df06843d13a08bf2a449844c5f8a47 8001bc4d4c627984d5a41da8d0402919")
        );
        let server_finished_hash =
            array::<32>("9608102a0f1ccc6db6250b7b7e417b1a 000eaada3daae4777a7686c9ff83df13");
        assert_eq!(
            derive_secret(
                Suite::ChaCha20Poly1305Sha256,
                &main,
                "c ap traffic",
                &server_finished_hash
            )
            .unwrap(),
            array::<32>("9e40646ce79a7f9dc05af8889bce6552 875afa0b06df0087f792ebb7c17504a5")
        );
        assert_eq!(
            derive_secret(
                Suite::ChaCha20Poly1305Sha256,
                &main,
                "s ap traffic",
                &server_finished_hash
            )
            .unwrap(),
            array::<32>("a11af9f05531f856ad47116b45a95032 8204b4f44bfb6b3a4b4f1f3fcb631643")
        );
        // RFC 8448's cipher is AES-128-GCM: these are only HKDF label
        // vectors. Our record layer always asks for a 32-byte ChaCha key.
        assert_eq!(
            hkdf_expand_label(Suite::ChaCha20Poly1305Sha256, &server_hs, "key", b"", 16).unwrap(),
            hex("3fce516009c21727d0f2e4e86ee403bc")
        );
        assert_eq!(
            hkdf_expand_label(Suite::ChaCha20Poly1305Sha256, &server_hs, "iv", b"", 12).unwrap(),
            hex("5d313eb2671276ee13000b30")
        );
        let client_finished = finished_verify_data(
            Suite::ChaCha20Poly1305Sha256,
            &client_hs,
            &server_finished_hash,
        )
        .unwrap();
        assert_eq!(
            client_finished,
            array::<32>("a8ec436d677634ae525ac1fcebe11a03 9ec17694fac6e98527b642f2edd5ce61")
        );
        verify_finished(
            Suite::ChaCha20Poly1305Sha256,
            &client_hs,
            &server_finished_hash,
            &client_finished,
        )
        .unwrap();
        for byte in 0..32 {
            let mut changed = client_finished.clone();
            changed[byte] ^= 1;
            assert!(verify_finished(
                Suite::ChaCha20Poly1305Sha256,
                &client_hs,
                &server_finished_hash,
                &changed
            )
            .is_err());
        }
        assert!(verify_finished(
            Suite::ChaCha20Poly1305Sha256,
            &client_hs,
            &server_finished_hash,
            &[0; 31]
        )
        .is_err());
        assert!(verify_finished(
            Suite::ChaCha20Poly1305Sha256,
            &client_hs,
            &server_finished_hash,
            &[0; 33]
        )
        .is_err());
    }

    #[test]
    fn label_encoding_bounds_and_context_distinction() {
        let secret = [0x5a; 32];
        let label = "x".repeat(249);
        assert_eq!(
            hkdf_expand_label(
                Suite::ChaCha20Poly1305Sha256,
                &secret,
                &label,
                &[0; 255],
                8160
            )
            .unwrap()
            .len(),
            8160
        );
        assert!(hkdf_expand_label(
            Suite::ChaCha20Poly1305Sha256,
            &secret,
            &"x".repeat(250),
            b"",
            32
        )
        .is_err());
        assert!(
            hkdf_expand_label(Suite::ChaCha20Poly1305Sha256, &secret, "key", &[0; 256], 32)
                .is_err()
        );
        assert!(
            hkdf_expand_label(Suite::ChaCha20Poly1305Sha256, &secret, "key", b"", 8161).is_err()
        );
        for bad in ["", "key\0", "\u{00e9}"] {
            assert!(
                hkdf_expand_label(Suite::ChaCha20Poly1305Sha256, &secret, bad, b"", 32).is_err()
            );
        }
        assert!(
            hkdf_expand_label(Suite::ChaCha20Poly1305Sha256, &secret, "key", b"", 0)
                .unwrap()
                .is_empty()
        );
        assert_ne!(
            hkdf_expand_label(Suite::ChaCha20Poly1305Sha256, &secret, "derived", b"", 32).unwrap(),
            derive_secret(
                Suite::ChaCha20Poly1305Sha256,
                &secret,
                "derived",
                &sha256(b"")
            )
            .unwrap()
        );
        assert_ne!(
            next_traffic_secret(Suite::ChaCha20Poly1305Sha256, &secret).unwrap(),
            secret
        );
        let next = next_traffic_secret(Suite::ChaCha20Poly1305Sha256, &secret).unwrap();
        assert_ne!(
            next_traffic_secret(Suite::ChaCha20Poly1305Sha256, &next).unwrap(),
            next
        );
    }

    #[test]
    fn independent_chacha_record_fixtures() {
        // Independently generated using Python's hashlib/hmac and
        // cryptography ChaCha20Poly1305; no Python is used at runtime.
        let secret: [u8; 32] = std::array::from_fn(|index| index as u8);
        let mut sealer = Sealer::new(Suite::ChaCha20Poly1305Sha256, &secret).unwrap();
        assert_eq!(
            sealer.state.key,
            array::<32>("2ffbc449e87844051c7768f61ffd8ad0 70830f01ec2c520ef65f08e8296dffa0")
        );
        assert_eq!(sealer.state.iv, array::<12>("2f41c846a431a163814bcd71"));
        let first = hex("170303001a dacb07926875ade1ef6d896851c4ed2af1923bcc882d8e6da775");
        let second = hex("1703030011 1cce774f6d49cfc8855164656d230b65fb");
        assert_eq!(
            sealer
                .seal(ContentType::ApplicationData, b"hello\0", 3)
                .unwrap(),
            first
        );
        assert_eq!(
            sealer.seal(ContentType::ApplicationData, b"", 0).unwrap(),
            second
        );
        let mut opener = Opener::new(Suite::ChaCha20Poly1305Sha256, &secret).unwrap();
        let plaintext = opener.open(&first).unwrap();
        assert_eq!(plaintext.content_type, ContentType::ApplicationData);
        assert_eq!(plaintext.content, b"hello\0");
        assert!(opener.open(&second).unwrap().content.is_empty());
        assert!(!sealer.is_failed());
        assert!(!opener.is_failed());
        assert_eq!(sealer.state.sequence, 2);
        assert_eq!(opener.state.sequence, 2);
    }

    #[test]
    fn nonce_uses_big_endian_sequence_in_low_eight_iv_bytes() {
        let iv = [0x5a; 12];
        assert_eq!(record_nonce(&iv, 0), iv);
        assert_eq!(
            record_nonce(&iv, 0x0102_0304_0506_0708),
            [0x5a, 0x5a, 0x5a, 0x5a, 0x5b, 0x58, 0x59, 0x5e, 0x5f, 0x5c, 0x5d, 0x52]
        );
        assert_ne!(record_nonce(&iv, 255), record_nonce(&iv, 256));
    }

    #[test]
    fn record_types_padding_and_size_boundaries() {
        let mut sealer = Sealer::new(Suite::ChaCha20Poly1305Sha256, &[0x73; 32]).unwrap();
        let mut opener = Opener::new(Suite::ChaCha20Poly1305Sha256, &[0x73; 32]).unwrap();
        for (content_type, content) in [
            (ContentType::Alert, b"\x01\x00".as_slice()),
            (ContentType::Handshake, b"handshake fragment\0\0".as_slice()),
            (ContentType::ApplicationData, b"payload\0\0".as_slice()),
            (ContentType::ApplicationData, b"".as_slice()),
        ] {
            for padding in [0, 1, 15, 16, 63, MAX_INNER_BYTES - content.len() - 1] {
                let record = sealer.seal(content_type, content, padding).unwrap();
                assert!(record.len() <= RECORD_HEADER_BYTES + MAX_CIPHERTEXT_BYTES);
                let opened = opener.open(&record).unwrap();
                assert_eq!(opened.content_type, content_type);
                assert_eq!(opened.content, content);
            }
        }
        let maximum = vec![0xaa; MAX_CONTENT_BYTES];
        let record = sealer
            .seal(ContentType::ApplicationData, &maximum, 0)
            .unwrap();
        assert_eq!(record.len(), RECORD_HEADER_BYTES + MAX_CIPHERTEXT_BYTES);
        assert_eq!(opener.open(&record).unwrap().content, maximum);
        for (content_type, content, padding) in [
            (
                ContentType::ApplicationData,
                vec![0; MAX_CONTENT_BYTES + 1],
                0,
            ),
            (ContentType::ApplicationData, maximum, 1),
            (ContentType::ApplicationData, Vec::new(), usize::MAX),
            (ContentType::Handshake, Vec::new(), 0),
            (ContentType::Alert, Vec::new(), 0),
            (ContentType::Alert, vec![2], 0),
            (ContentType::Alert, vec![2, 0, 2, 0], 0),
        ] {
            let mut invalid = Sealer::new(Suite::ChaCha20Poly1305Sha256, &[0x73; 32]).unwrap();
            assert!(invalid.seal(content_type, &content, padding).is_err());
            assert!(invalid.is_failed());
            assert!(invalid
                .seal(ContentType::ApplicationData, b"retry", 0)
                .is_err());
        }
    }

    #[test]
    fn corruption_truncation_replay_and_reordering_poison_opener() {
        let secret = [0x41; 32];
        let mut sealer = Sealer::new(Suite::ChaCha20Poly1305Sha256, &secret).unwrap();
        let first = sealer
            .seal(ContentType::ApplicationData, b"first", 5)
            .unwrap();
        let second = sealer
            .seal(ContentType::ApplicationData, b"second", 0)
            .unwrap();
        for position in 0..first.len() {
            let mut opener = Opener::new(Suite::ChaCha20Poly1305Sha256, &secret).unwrap();
            let mut damaged = first.clone();
            damaged[position] ^= 1;
            assert!(opener.open(&damaged).is_err());
            assert!(opener.is_failed());
            assert!(opener.open(&first).is_err());
        }
        for length in 0..first.len() {
            let mut opener = Opener::new(Suite::ChaCha20Poly1305Sha256, &secret).unwrap();
            assert!(opener.open(&first[..length]).is_err());
            assert!(opener.open(&first).is_err());
        }
        let mut appended = first.clone();
        appended.push(0);
        let mut opener = Opener::new(Suite::ChaCha20Poly1305Sha256, &secret).unwrap();
        assert!(opener.open(&appended).is_err());
        assert!(opener.is_failed());
        let mut opener = Opener::new(Suite::ChaCha20Poly1305Sha256, &secret).unwrap();
        assert_eq!(opener.open(&first).unwrap().content, b"first");
        assert!(opener.open(&first).is_err()); // replay at sequence 1
        assert!(opener.open(&second).is_err()); // no recovery after failure
        let mut opener = Opener::new(Suite::ChaCha20Poly1305Sha256, &secret).unwrap();
        assert!(opener.open(&second).is_err()); // out of order at sequence 0
        assert!(opener.open(&first).is_err());
        let mut wrong_direction = Opener::new(Suite::ChaCha20Poly1305Sha256, &[0x42; 32]).unwrap();
        assert!(wrong_direction.open(&first).is_err());
        assert!(wrong_direction.is_failed());
    }

    fn authenticated_invalid_inner(secret: &[u8; 32], inner: &[u8]) -> Vec<u8> {
        let key = RecordKey::new(Suite::ChaCha20Poly1305Sha256, secret).unwrap();
        let length = inner.len() + TAG_BYTES;
        let [high, low] = (length as u16).to_be_bytes();
        let header = [23, 3, 3, high, low];
        let encrypted =
            crypto_aead::seal(&key.key, &record_nonce(&key.iv, 0), &header, inner).unwrap();
        let mut record = header.to_vec();
        record.extend_from_slice(&encrypted);
        record
    }

    #[test]
    fn authenticated_bad_inner_framing_is_rejected_and_poisoned() {
        let secret = [0x15; 32];
        for inner in [
            vec![],
            vec![0; 128],
            vec![22],                      // empty Handshake
            vec![21],                      // empty Alert
            vec![2, 21],                   // incomplete Alert
            vec![2, 0, 2, 0, 21],          // two alerts in one record
            vec![1, 20],                   // encrypted ChangeCipherSpec
            vec![1, 0xff, 0, 0],           // unknown inner type
            vec![23; MAX_INNER_BYTES + 1], // oversized despite valid AEAD
        ] {
            let record = authenticated_invalid_inner(&secret, &inner);
            let mut opener = Opener::new(Suite::ChaCha20Poly1305Sha256, &secret).unwrap();
            assert!(opener.open(&record).is_err());
            assert!(opener.is_failed());
        }
    }

    #[test]
    fn lifetime_cap_has_no_counter_wrap_or_recovery() {
        let secret = [0x22; 32];
        let mut sealer = Sealer::new(Suite::ChaCha20Poly1305Sha256, &secret).unwrap();
        let mut opener = Opener::new(Suite::ChaCha20Poly1305Sha256, &secret).unwrap();
        sealer.state.sequence = MAX_RECORDS_PER_KEY - 1;
        opener.state.sequence = MAX_RECORDS_PER_KEY - 1;
        let last = sealer
            .seal(ContentType::ApplicationData, b"last", 0)
            .unwrap();
        assert_eq!(opener.open(&last).unwrap().content, b"last");
        assert_eq!(sealer.state.sequence, MAX_RECORDS_PER_KEY);
        assert_eq!(opener.state.sequence, MAX_RECORDS_PER_KEY);
        assert!(sealer
            .seal(ContentType::ApplicationData, b"over limit", 0)
            .is_err());
        assert!(opener.open(&last).is_err());
        assert!(sealer.is_failed());
        assert!(opener.is_failed());
        let mut maximum = Sealer::new(Suite::ChaCha20Poly1305Sha256, &secret).unwrap();
        maximum.state.sequence = u64::MAX;
        assert!(maximum.seal(ContentType::ApplicationData, b"", 0).is_err());
        assert_eq!(maximum.state.sequence, u64::MAX);
    }
}
