//! Original bounded TLS 1.2 AES-CBC record protection, RFC 5246 / RFC 7366.
//!
//! Encrypt-then-MAC authenticates the complete explicit IV and ciphertext
//! before decrypting. Legacy MAC-then-encrypt evaluates every feasible public
//! padding-length candidate, including its full HMAC, before selecting a
//! result. No decrypted byte controls a pre-authentication index, branch,
//! allocation size, or hash length. This deliberately costs more than AEAD.
//! Source-level fixed work is not a compiler/CPU constant-time guarantee.
//!
//! This module owns directional key material, not counters or connection
//! policy. Its caller MUST reserve unique increasing sequence numbers, poison
//! the connection on any error, and never reuse these keys for another epoch.

use crate::crypto_aes_gcm::Aes;
use crate::crypto_hash::hmac_sha256;
use crate::tls_crypto::{ContentType, PlaintextRecord, Suite};

const BLOCK: usize = 16;
const MAX_CONTENT_BYTES: usize = 16_384;
pub(super) const MAX_CIPHERTEXT_BYTES: usize = MAX_CONTENT_BYTES + BLOCK + 256 + 48;
const MAX_RECORDS_PER_KEY: u64 = 1 << 20;
const BAD_RECORD: &str = "TLS 1.2 CBC record authentication failed.";

// No Clone/Copy/Debug: directional secrets stay in one connection state.
pub(super) struct Keys {
    aes: Aes,
    mac_key: Vec<u8>,
    iv_key: [u8; 32],
    hash: Suite,
    encrypt_then_mac: bool,
}

impl Keys {
    pub(super) fn new(
        aes_key: &[u8],
        mac_key: &[u8],
        iv_key: [u8; 32],
        sha384: bool,
        encrypt_then_mac: bool,
    ) -> Result<Self, String> {
        let (key_length, mac_length, hash) = if sha384 {
            (32, 48, Suite::Aes256GcmSha384)
        } else {
            (16, 32, Suite::Aes128GcmSha256)
        };
        if aes_key.len() != key_length || mac_key.len() != mac_length {
            return Err("Invalid TLS 1.2 CBC encryption or MAC key length.".into());
        }
        Ok(Self {
            aes: Aes::new(aes_key)?,
            mac_key: mac_key.to_vec(),
            iv_key,
            hash,
            encrypt_then_mac,
        })
    }

    fn mac(&self, sequence: u64, kind: ContentType, fragment: &[u8]) -> Vec<u8> {
        let mut input = Vec::with_capacity(13 + fragment.len());
        input.extend_from_slice(&sequence.to_be_bytes());
        input.extend_from_slice(&[kind as u8, 3, 3]);
        input.extend_from_slice(&(fragment.len() as u16).to_be_bytes());
        input.extend_from_slice(fragment);
        self.hash.hmac(&self.mac_key, &input)
    }

    fn iv(&self, sequence: u64) -> [u8; BLOCK] {
        // The handshake derives independent directional IV keys with a
        // dedicated PRF label. HMAC turns the public sequence into an
        // unpredictable explicit IV without borrowing the encryption key or
        // coupling the record layer to an OS random-number API.
        let mut input = Vec::with_capacity(36);
        input.extend_from_slice(b"OwnBrowser TLS 1.2 CBC IV\0");
        input.extend_from_slice(&sequence.to_be_bytes());
        let digest = hmac_sha256(&self.iv_key, &input);
        let mut iv = [0; BLOCK];
        iv.copy_from_slice(&digest[..BLOCK]);
        iv
    }

    pub(super) fn seal(
        &self,
        sequence: u64,
        kind: ContentType,
        content: &[u8],
    ) -> Result<Vec<u8>, String> {
        if sequence >= MAX_RECORDS_PER_KEY || !valid_plaintext(kind, content) {
            return Err("Invalid TLS 1.2 CBC plaintext or exhausted record sequence.".into());
        }
        let mut plaintext = Vec::with_capacity(content.len() + self.hash.hash_len() + BLOCK);
        plaintext.extend_from_slice(content);
        if !self.encrypt_then_mac {
            plaintext.extend_from_slice(&self.mac(sequence, kind, content));
        }
        let padding = BLOCK - 1 - plaintext.len() % BLOCK;
        plaintext.resize(plaintext.len() + padding + 1, padding as u8);
        let iv = self.iv(sequence);
        let mut previous = iv;
        for block in plaintext.as_chunks_mut::<BLOCK>().0 {
            for (byte, chained) in block.iter_mut().zip(previous) {
                *byte ^= chained;
            }
            *block = self.aes.encrypt(block);
            previous = *block;
        }
        let mut fragment = Vec::with_capacity(BLOCK + plaintext.len() + self.hash.hash_len());
        fragment.extend_from_slice(&iv);
        fragment.extend_from_slice(&plaintext);
        if self.encrypt_then_mac {
            let tag = self.mac(sequence, kind, &fragment);
            fragment.extend_from_slice(&tag);
        }
        let mut wire = Vec::with_capacity(5 + fragment.len());
        wire.extend_from_slice(&[kind as u8, 3, 3]);
        wire.extend_from_slice(&(fragment.len() as u16).to_be_bytes());
        wire.extend_from_slice(&fragment);
        Ok(wire)
    }

    pub(super) fn open(&self, sequence: u64, wire: &[u8]) -> Result<PlaintextRecord, String> {
        if sequence >= MAX_RECORDS_PER_KEY
            || wire.len() < 5 + 2 * BLOCK
            || wire.len() > 5 + MAX_CIPHERTEXT_BYTES
            || wire[1..3] != [3, 3]
            || usize::from(u16::from_be_bytes([wire[3], wire[4]])) != wire.len() - 5
        {
            return Err(BAD_RECORD.into());
        }
        let kind = match wire[0] {
            21 => ContentType::Alert,
            22 => ContentType::Handshake,
            23 => ContentType::ApplicationData,
            _ => return Err(BAD_RECORD.into()),
        };
        let mut fragment = &wire[5..];
        if self.encrypt_then_mac {
            let length = fragment
                .len()
                .checked_sub(self.hash.hash_len())
                .filter(|length| *length >= 2 * BLOCK)
                .ok_or(BAD_RECORD)?;
            let (encrypted, tag) = fragment.split_at(length);
            // RFC 7366: verify the outer MAC before any decryption, padding
            // examination, or plaintext allocation.
            let expected = self.mac(sequence, kind, encrypted);
            if difference(&expected, tag) != 0 {
                return Err(BAD_RECORD.into());
            }
            fragment = encrypted;
        }
        if fragment.len() < 2 * BLOCK || !fragment.len().is_multiple_of(BLOCK) {
            return Err(BAD_RECORD.into());
        }
        let mut previous = [0; BLOCK];
        previous.copy_from_slice(&fragment[..BLOCK]);
        let mut plaintext = Vec::new();
        plaintext
            .try_reserve_exact(fragment.len() - BLOCK)
            .map_err(|_| BAD_RECORD)?;
        for block in fragment[BLOCK..].as_chunks::<BLOCK>().0 {
            let mut decoded = self.aes.decrypt(block);
            for (byte, chained) in decoded.iter_mut().zip(previous) {
                *byte ^= chained;
            }
            plaintext.extend_from_slice(&decoded);
            previous = *block;
        }
        let authenticated_length = if self.encrypt_then_mac {
            authenticated_padding_length(&plaintext)
        } else {
            self.verify_legacy(sequence, kind, &plaintext)
        };
        let length = match authenticated_length {
            Some(length) => length,
            None => {
                plaintext.fill(0);
                return Err(BAD_RECORD.into());
            }
        };
        if !valid_plaintext(kind, &plaintext[..length]) {
            plaintext.fill(0);
            return Err(BAD_RECORD.into());
        }
        plaintext[length..].fill(0);
        plaintext.truncate(length);
        Ok(PlaintextRecord {
            content_type: kind,
            content: plaintext,
        })
    }

    #[inline(never)]
    fn verify_legacy(&self, sequence: u64, kind: ContentType, plaintext: &[u8]) -> Option<usize> {
        let mac_length = self.hash.hash_len();
        let mut selected = 0usize;
        let mut found = 0u8;
        // The entire 0..=255 search is evaluated. Feasibility, slice positions,
        // HMAC input lengths, and loop sizes depend only on public wire length
        // and the public loop counter, never on the decrypted padding byte.
        // Computing only one HMAC at a padding-dependent length would expose
        // a timing oracle even with a constant-work padding-byte scan.
        for padding in 0..=255usize {
            let Some(content_length) = plaintext.len().checked_sub(mac_length + padding + 1) else {
                continue;
            };
            if content_length > MAX_CONTENT_BYTES {
                continue;
            }
            let expected = self.mac(sequence, kind, &plaintext[..content_length]);
            let received = &plaintext[content_length..content_length + mac_length];
            let mut mismatch = difference(&expected, received);
            for &byte in &plaintext[content_length + mac_length..] {
                mismatch |= byte ^ padding as u8;
            }
            // 0 -> 0xff; every nonzero u8 -> 0. No equality branch or index
            // derived from padding/MAC is used until all candidates finish.
            let mask = (u16::from(mismatch).wrapping_sub(1) >> 8) as u8;
            found |= mask;
            selected |= content_length & 0usize.wrapping_sub(usize::from(mask & 1));
        }
        if found == 0xff {
            Some(selected)
        } else {
            None
        }
    }
}

#[inline(never)]
fn difference(expected: &[u8], received: &[u8]) -> u8 {
    debug_assert_eq!(expected.len(), received.len());
    expected
        .iter()
        .zip(received)
        .fold(0, |different, (left, right)| different | (left ^ right))
}

fn authenticated_padding_length(plaintext: &[u8]) -> Option<usize> {
    // This path runs only after a successful outer MAC. Decrypted padding
    // cannot influence processing of a peer's unauthenticated ciphertext.
    let &value = plaintext.last()?;
    let content_length = plaintext.len().checked_sub(usize::from(value) + 1)?;
    let mismatch = plaintext[content_length..]
        .iter()
        .fold(0, |different, byte| different | (byte ^ value));
    if mismatch == 0 && content_length <= MAX_CONTENT_BYTES {
        Some(content_length)
    } else {
        None
    }
}

fn valid_plaintext(kind: ContentType, content: &[u8]) -> bool {
    content.len() <= MAX_CONTENT_BYTES
        && (kind != ContentType::Handshake || !content.is_empty())
        && (kind != ContentType::Alert || (content.len() == 2 && matches!(content[0], 1 | 2)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cbc_records_bind_sequence_type_version_and_iv() {
        for sha384 in [false, true] {
            for etm in [false, true] {
                let keys = Keys::new(
                    &vec![7; if sha384 { 32 } else { 16 }],
                    &vec![9; if sha384 { 48 } else { 32 }],
                    [11; 32],
                    sha384,
                    etm,
                )
                .unwrap();
                let wire = keys
                    .seal(3, ContentType::ApplicationData, b"example")
                    .unwrap();
                assert_eq!(keys.open(3, &wire).unwrap().content, b"example");
                assert!(keys.open(2, &wire).is_err());
                assert!(keys.open(4, &wire).is_err());
                for position in [0, 1, 3, 5, 20, 21, wire.len() - 1] {
                    let mut damaged = wire.clone();
                    damaged[position] ^= 1;
                    assert!(keys.open(3, &damaged).is_err());
                }
                assert_ne!(
                    &wire[5..21],
                    &keys
                        .seal(4, ContentType::ApplicationData, b"example")
                        .unwrap()[5..21]
                );
                assert!(keys.open(MAX_RECORDS_PER_KEY, &wire).is_err());
                assert!(keys
                    .seal(MAX_RECORDS_PER_KEY, ContentType::ApplicationData, b"")
                    .is_err());
                assert!(keys.seal(0, ContentType::Handshake, b"").is_err());
                assert!(keys.seal(0, ContentType::Alert, &[3, 0]).is_err());
                assert!(keys
                    .seal(0, ContentType::ApplicationData, &vec![0; 16385])
                    .is_err());
            }
        }
    }
}
