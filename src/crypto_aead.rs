//! Experimental IETF ChaCha20-Poly1305, implemented from RFC 8439.
//!
//! This module supplies a primitive, not a TLS implementation. The caller must
//! never reuse a nonce with the same key. Keys and nonces are supplied by the
//! caller; this module does not generate randomness or manage key lifetimes.
//! There is no claim of a completed cryptographic audit or guaranteed erasure
//! of temporary secrets. No external implementation or crate is used here.
//!
//! Specification and published test vectors:
//! <https://www.rfc-editor.org/rfc/rfc8439.html>

/// A local resource bound for each plaintext/ciphertext and AAD input. A future
/// TLS record layer must impose its own, smaller protocol record-size limits.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
pub const TAG_BYTES: usize = 16;

/// Encrypt and authenticate one message, returning ciphertext followed by its
/// 16-byte authentication tag. AAD is authenticated but is not encrypted.
pub fn seal(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, String> {
    check_lengths(aad.len(), plaintext.len())?;
    let capacity = plaintext
        .len()
        .checked_add(TAG_BYTES)
        .ok_or("ChaCha20-Poly1305 output length overflow.")?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| "Cannot allocate ChaCha20-Poly1305 output.")?;
    output.extend_from_slice(plaintext);
    chacha_xor(key, nonce, 1, &mut output)?;
    let tag = authenticate(key, nonce, aad, &output);
    output.extend_from_slice(&tag);
    Ok(output)
}

/// Authenticate ciphertext and its appended tag before allocating or producing
/// any plaintext. Authentication failures never return partially decrypted data.
pub fn open(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    sealed: &[u8],
) -> Result<Vec<u8>, String> {
    let text_len = sealed
        .len()
        .checked_sub(TAG_BYTES)
        .ok_or("ChaCha20-Poly1305 input is missing its authentication tag.")?;
    check_lengths(aad.len(), text_len)?;
    let (ciphertext, received_bytes) = sealed.split_at(text_len);
    let expected = authenticate(key, nonce, aad, ciphertext);
    let mut received = [0u8; TAG_BYTES];
    received.copy_from_slice(received_bytes);
    if !tags_equal(&expected, &received) {
        return Err("ChaCha20-Poly1305 authentication failed.".into());
    }

    let mut plaintext = Vec::new();
    plaintext
        .try_reserve_exact(text_len)
        .map_err(|_| "Cannot allocate ChaCha20-Poly1305 plaintext.")?;
    plaintext.extend_from_slice(ciphertext);
    chacha_xor(key, nonce, 1, &mut plaintext)?;
    Ok(plaintext)
}

fn check_lengths(aad_len: usize, text_len: usize) -> Result<(), String> {
    if aad_len > MAX_INPUT_BYTES || text_len > MAX_INPUT_BYTES {
        return Err("ChaCha20-Poly1305 inputs are limited to 1 MiB each.".into());
    }
    check_counter(1, text_len)
}

// Fixed work over all 16 bytes: there is no mismatch-dependent early return.
// This source-level property is not a guarantee about every compiler/CPU.
#[inline(never)]
fn tags_equal(left: &[u8; TAG_BYTES], right: &[u8; TAG_BYTES]) -> bool {
    let mut difference = 0u8;
    for index in 0..TAG_BYTES {
        difference |= left[index] ^ right[index];
    }
    difference == 0
}

fn read_word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn quarter_round(state: &mut [u32; 16], indexes: [usize; 4]) {
    let [ai, bi, ci, di] = indexes;
    let [mut a, mut b, mut c, mut d] = [state[ai], state[bi], state[ci], state[di]];
    a = a.wrapping_add(b);
    d = (d ^ a).rotate_left(16);
    c = c.wrapping_add(d);
    b = (b ^ c).rotate_left(12);
    a = a.wrapping_add(b);
    d = (d ^ a).rotate_left(8);
    c = c.wrapping_add(d);
    b = (b ^ c).rotate_left(7);
    state[ai] = a;
    state[bi] = b;
    state[ci] = c;
    state[di] = d;
}

fn chacha_block(key: &[u8; 32], nonce: &[u8; 12], counter: u32) -> [u8; 64] {
    let mut initial = [0u32; 16];
    initial[..4].copy_from_slice(&[0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574]);
    for index in 0..8 {
        initial[4 + index] = read_word(key, index * 4);
    }
    initial[12] = counter;
    for index in 0..3 {
        initial[13 + index] = read_word(nonce, index * 4);
    }
    let mut working = initial;
    for _ in 0..10 {
        for column in 0..4 {
            quarter_round(&mut working, [column, column + 4, column + 8, column + 12]);
        }
        for diagonal in 0..4 {
            quarter_round(
                &mut working,
                [
                    diagonal,
                    4 + (diagonal + 1) % 4,
                    8 + (diagonal + 2) % 4,
                    12 + (diagonal + 3) % 4,
                ],
            );
        }
    }
    let mut output = [0u8; 64];
    for index in 0..16 {
        output[index * 4..index * 4 + 4]
            .copy_from_slice(&working[index].wrapping_add(initial[index]).to_le_bytes());
    }
    output
}

fn check_counter(counter: u32, byte_len: usize) -> Result<(), String> {
    let full_blocks =
        u64::try_from(byte_len / 64).map_err(|_| "ChaCha20 block counter would overflow.")?;
    let blocks = full_blocks + u64::from(!byte_len.is_multiple_of(64));
    let remaining = u64::from(u32::MAX) - u64::from(counter) + 1;
    if blocks > remaining {
        return Err("ChaCha20 block counter would overflow.".into());
    }
    Ok(())
}

fn chacha_xor(
    key: &[u8; 32],
    nonce: &[u8; 12],
    counter: u32,
    bytes: &mut [u8],
) -> Result<(), String> {
    // Reject exhaustion before changing the input. The wider sum below never
    // wraps, including when the final allowed block has counter 0xffff_ffff.
    check_counter(counter, bytes.len())?;
    for (number, chunk) in bytes.chunks_mut(64).enumerate() {
        let block_counter = (u64::from(counter) + number as u64) as u32;
        let stream = chacha_block(key, nonce, block_counter);
        for (byte, mask) in chunk.iter_mut().zip(stream) {
            *byte ^= mask;
        }
    }
    Ok(())
}

const LIMB_BITS: u32 = 26;
const LIMB_MASK: u64 = (1 << LIMB_BITS) - 1;

// Little-endian base-2^26 representation of up to 130 bits. The seventeenth
// input byte is either zero or the appended Poly1305 block bit.
fn limbs(bytes: &[u8; 17]) -> [u64; 5] {
    [
        u64::from(read_word(bytes, 0)) & LIMB_MASK,
        (u64::from(read_word(bytes, 3)) >> 2) & LIMB_MASK,
        (u64::from(read_word(bytes, 6)) >> 4) & LIMB_MASK,
        (u64::from(read_word(bytes, 9)) >> 6) & LIMB_MASK,
        (u64::from(read_word(bytes, 12)) >> 8) | (u64::from(bytes[16]) << 24),
    ]
}

struct Poly1305 {
    r: [u64; 5],
    accumulator: [u64; 5],
    pad: [u32; 4],
}

impl Poly1305 {
    fn new(key: &[u8; 32]) -> Self {
        let mut clamped = [0u8; 17];
        clamped[..16].copy_from_slice(&key[..16]);
        for (index, byte) in clamped[..16].iter_mut().enumerate() {
            *byte &= match index {
                3 | 7 | 11 | 15 => 0x0f,
                4 | 8 | 12 => 0xfc,
                _ => 0xff,
            };
        }
        Self {
            r: limbs(&clamped),
            accumulator: [0; 5],
            pad: [
                read_word(key, 16),
                read_word(key, 20),
                read_word(key, 24),
                read_word(key, 28),
            ],
        }
    }

    fn block(&mut self, input: &[u8]) {
        debug_assert!(!input.is_empty() && input.len() <= 16);
        let mut encoded = [0u8; 17];
        encoded[..input.len()].copy_from_slice(input);
        encoded[input.len()] = 1;
        let message = limbs(&encoded);
        let mut sum = self.accumulator;
        for index in 0..5 {
            sum[index] += message[index];
        }

        // With B = 2^26 and p = B^5 - 5, a term at position i+j >= 5
        // folds into position i+j-5 multiplied by 5. Each sum limb is
        // less than 2B, each r limb is less than B; even the largest
        // accumulated product is below 42*B^2, safely below 2^64.
        let mut product = [0u64; 5];
        for (left, value) in sum.iter().enumerate() {
            for (right, factor) in self.r.iter().enumerate() {
                let position = left + right;
                if position < 5 {
                    product[position] += value * factor;
                } else {
                    product[position - 5] += value * factor * 5;
                }
            }
        }

        // Normalize all limbs, folding a carry above bit 129 back as 5.
        // Pass one leaves less than 52B in limb zero. Pass two can leave
        // only a final +5 there. A third fixed pass settles that carry.
        // Loop counts and branches depend on indexes, never limb values.
        for _ in 0..3 {
            for index in 0..4 {
                let carry = product[index] >> LIMB_BITS;
                product[index] &= LIMB_MASK;
                product[index + 1] += carry;
            }
            let top = product[4] >> LIMB_BITS;
            product[4] &= LIMB_MASK;
            product[0] += top * 5;
        }
        debug_assert!(product.iter().all(|limb| *limb <= LIMB_MASK));
        self.accumulator = product;
    }

    fn padded(&mut self, bytes: &[u8]) {
        // RFC 8439 AEAD padding belongs to the authenticated message itself.
        // Even a short final chunk is therefore a full Poly1305 block here.
        // An empty or aligned input gets no additional all-zero block.
        for chunk in bytes.chunks(16) {
            let mut padded = [0u8; 16];
            padded[..chunk.len()].copy_from_slice(chunk);
            self.block(&padded);
        }
    }

    fn finish(self) -> [u8; TAG_BYTES] {
        // The accumulator is in [0, 2^130). h+5 has a carry beyond bit
        // 129 exactly when h >= p. Select h-p in that case without a
        // branch on the secret accumulator; otherwise select h.
        let mut reduced = self.accumulator;
        let mut carry = 5u64;
        for limb in &mut reduced {
            let value = *limb + carry;
            *limb = value & LIMB_MASK;
            carry = value >> LIMB_BITS;
        }
        let use_reduced = 0u64.wrapping_sub(carry);
        let mut canonical = [0u64; 5];
        for index in 0..5 {
            canonical[index] =
                (reduced[index] & use_reduced) | (self.accumulator[index] & !use_reduced);
        }

        // Keep only the low 128 bits, then add s modulo 2^128 using four
        // little-endian words. Casting each word before adding avoids
        // counting its high bits twice in the next word's carry.
        let words = [
            (canonical[0] | (canonical[1] << 26)) as u32,
            ((canonical[1] >> 6) | (canonical[2] << 20)) as u32,
            ((canonical[2] >> 12) | (canonical[3] << 14)) as u32,
            ((canonical[3] >> 18) | (canonical[4] << 8)) as u32,
        ];
        let mut tag = [0u8; TAG_BYTES];
        let mut word_carry = 0u64;
        for index in 0..4 {
            let word = u64::from(words[index]) + u64::from(self.pad[index]) + word_carry;
            tag[index * 4..index * 4 + 4].copy_from_slice(&(word as u32).to_le_bytes());
            word_carry = word >> 32;
        }
        tag
    }
}

fn authenticate(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    ciphertext: &[u8],
) -> [u8; TAG_BYTES] {
    let first_block = chacha_block(key, nonce, 0);
    let mut one_time_key = [0u8; 32];
    one_time_key.copy_from_slice(&first_block[..32]);
    let mut mac = Poly1305::new(&one_time_key);
    mac.padded(aad);
    mac.padded(ciphertext);
    let mut lengths = [0u8; 16];
    lengths[..8].copy_from_slice(&(aad.len() as u64).to_le_bytes());
    lengths[8..].copy_from_slice(&(ciphertext.len() as u64).to_le_bytes());
    mac.block(&lengths);
    mac.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        let compact: String = text.chars().filter(|c| !c.is_ascii_whitespace()).collect();
        assert_eq!(compact.len() % 2, 0);
        (0..compact.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&compact[index..index + 2], 16).unwrap())
            .collect()
    }

    fn array<const N: usize>(text: &str) -> [u8; N] {
        hex(text).try_into().unwrap()
    }

    fn poly(key: &[u8; 32], message: &[u8]) -> [u8; 16] {
        let mut mac = Poly1305::new(key);
        for chunk in message.chunks(16) {
            mac.block(chunk);
        }
        mac.finish()
    }

    const SUNSCREEN: &[u8] = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";

    #[test]
    fn rfc_8439_quarter_round_and_block() {
        let mut state = [0; 16];
        state[..4].copy_from_slice(&[0x1111_1111, 0x0102_0304, 0x9b8d_6f43, 0x0123_4567]);
        quarter_round(&mut state, [0, 1, 2, 3]);
        assert_eq!(
            &state[..4],
            &[0xea2a_92f4, 0xcb1c_f8ce, 0x4581_472e, 0x5881_c4bb]
        );

        let key = std::array::from_fn(|index| index as u8);
        let nonce = array("00000009 0000004a 00000000");
        assert_eq!(
            chacha_block(&key, &nonce, 1).as_slice(),
            hex("10f1e7e4 d13b5915 500fdd1f a32071c4
                 c7d1f4c7 33c06803 0422aa9a c3d46c4e
                 d2826446 079faa09 14c2d705 d98b02a2
                 b5129cd1 de164eb9 cbd083e8 a2503c4e")
        );
    }

    #[test]
    fn rfc_8439_chacha_encryption() {
        let key = std::array::from_fn(|index| index as u8);
        let nonce = array("00000000 0000004a 00000000");
        let ciphertext = hex("6e2e359a2568f98041ba0728dd0d6981
             e97e7aec1d4360c20a27afccfd9fae0b
             f91b65c5524733ab8f593dabcd62b357
             1639d624e65152ab8f530c359f0861d8
             07ca0dbf500d6a6156a38e088a22b65e
             52bc514d16ccf806818ce91ab7793736
             5af90bbf74a35be6b40b8eedf2785e42874d");
        let mut bytes = SUNSCREEN.to_vec();
        chacha_xor(&key, &nonce, 1, &mut bytes).unwrap();
        assert_eq!(bytes, ciphertext);
        chacha_xor(&key, &nonce, 1, &mut bytes).unwrap();
        assert_eq!(bytes, SUNSCREEN);
    }

    #[test]
    fn rfc_8439_poly1305_example() {
        let key = array("85d6be7857556d337f4452fe42d506a8 0103808afb0db2fd4abff6af4149f51b");
        assert_eq!(
            poly(&key, b"Cryptographic Forum Research Group"),
            array("a8061dc1305136c6c22b8baf0c0127a9")
        );
    }

    #[test]
    fn rfc_8439_poly1305_appendix_vectors_1_to_4() {
        assert_eq!(poly(&[0; 32], &[0; 64]), [0; 16]);
        let submission = concat!(
            "Any submission to the IETF intended by the Contributor for publication as all ",
            "or part of an IETF Internet-Draft or RFC and any statement made within the ",
            "context of an IETF activity is considered an \"IETF Contribution\". Such ",
            "statements include oral statements in IETF sessions, as well as written and ",
            "electronic communications made at any time or place, which are addressed to"
        )
        .as_bytes();
        assert_eq!(submission.len(), 375);
        let half = array::<16>("36e5f6b5c5e06070f0efca96227a863e");
        let mut key = [0u8; 32];
        key[16..].copy_from_slice(&half);
        assert_eq!(poly(&key, submission), half);
        key[..16].copy_from_slice(&half);
        key[16..].fill(0);
        assert_eq!(
            poly(&key, submission),
            array("f3477e7cd95417af89a6b8794c310cf0")
        );
        let poem = b"'Twas brillig, and the slithy toves\nDid gyre and gimble in the wabe:\nAll mimsy were the borogoves,\nAnd the mome raths outgrabe.";
        let key = array("1c9240a5eb55d38af333888604f6b5f0 473917c1402b80099dca5cbc207075c0");
        assert_eq!(poly(&key, poem), array("4541669a7eaaee61e708dc7cbcc5eb62"));
    }

    #[test]
    fn rfc_8439_poly1305_appendix_carry_vectors_5_to_11() {
        // Inputs and expected tags are published algorithm test data, not an
        // implementation. These cover exact p, p-1, carries, and 131-bit folds.
        let cases = [
            ("02000000000000000000000000000000", "00000000000000000000000000000000",
             "ffffffffffffffffffffffffffffffff", "03000000000000000000000000000000"),
            ("02000000000000000000000000000000", "ffffffffffffffffffffffffffffffff",
             "02000000000000000000000000000000", "03000000000000000000000000000000"),
            ("01000000000000000000000000000000", "00000000000000000000000000000000",
             "ffffffffffffffffffffffffffffffff f0ffffffffffffffffffffffffffffff 11000000000000000000000000000000",
             "05000000000000000000000000000000"),
            ("01000000000000000000000000000000", "00000000000000000000000000000000",
             "ffffffffffffffffffffffffffffffff fbfefefefefefefefefefefefefefefe 01010101010101010101010101010101",
             "00000000000000000000000000000000"),
            ("02000000000000000000000000000000", "00000000000000000000000000000000",
             "fdffffffffffffffffffffffffffffff", "faffffffffffffffffffffffffffffff"),
            ("01000000000000000400000000000000", "00000000000000000000000000000000",
             "e33594d7505e43b90000000000000000 3394d7505e4379cd0100000000000000
              00000000000000000000000000000000 01000000000000000000000000000000",
             "14000000000000005500000000000000"),
            ("01000000000000000400000000000000", "00000000000000000000000000000000",
             "e33594d7505e43b90000000000000000 3394d7505e4379cd0100000000000000
              00000000000000000000000000000000",
             "13000000000000000000000000000000"),
        ];
        for (index, (r, s, data, tag)) in cases.iter().enumerate() {
            let key = array::<32>(&format!("{r}{s}"));
            assert_eq!(
                poly(&key, &hex(data)),
                array::<16>(tag),
                "RFC vector {}",
                index + 5
            );
        }
    }

    #[test]
    fn rfc_8439_poly1305_key_generation_appendix() {
        let cases = [
            (
                [0; 32],
                [0; 12],
                "76b8e0ada0f13d90405d6ae55386bd28 bdd219b8a08ded1aa836efcc8b770dc7",
            ),
            (
                array("00000000000000000000000000000000 00000000000000000000000000000001"),
                array("000000000000000000000002"),
                "ecfa254f845f647473d3cb140da9e876 06cb33066c447b87bc2666dde3fbb739",
            ),
            (
                array("1c9240a5eb55d38af333888604f6b5f0 473917c1402b80099dca5cbc207075c0"),
                array("000000000000000000000002"),
                "965e3bc6f9ec7ed9560808f4d229f94b 137ff275ca9b3fcbdd59deaad23310ae",
            ),
        ];
        for (key, nonce, expected) in cases {
            assert_eq!(&chacha_block(&key, &nonce, 0)[..32], hex(expected));
        }
    }

    #[test]
    fn rfc_8439_aead_encryption_and_decryption() {
        let key = std::array::from_fn(|index| 0x80 + index as u8);
        let nonce = array("07000000 4041424344454647");
        let aad = hex("50515253c0c1c2c3c4c5c6c7");
        let expected = hex("d31a8d34648e60db7b86afbc53ef7ec2
             a4aded51296e08fea9e2b5a736ee62d6
             3dbea45e8ca9671282fafb69da92728b
             1a71de0a9e060b2905d6a5b67ecd3b36
             92ddbd7f2d778b8c9803aee328091b58
             fab324e4fad675945585808b4831d7bc
             3ff4def08e4b7a9de576d26586cec64b6116
             1ae10b594f09e26a7e902ecbd0600691");
        assert_eq!(seal(&key, &nonce, &aad, SUNSCREEN).unwrap(), expected);
        assert_eq!(open(&key, &nonce, &aad, &expected).unwrap(), SUNSCREEN);
    }

    #[test]
    fn rfc_8439_appendix_aead_decryption() {
        let key = array("1c9240a5eb55d38af333888604f6b5f0 473917c1402b80099dca5cbc207075c0");
        let nonce = array("00000000 0102030405060708");
        let aad = hex("f33388860000000000004e91");
        let sealed = hex(
            "64a0861575861af460f062c79be643bd 5e805cfd345cf389f108670ac76c8cb2
             4c6cfc18755d43eea09ee94e382d26b0 bdb7b73c321b0100d4f03b7f355894cf
             332f830e710b97ce98c8a84abd0b9481 14ad176e008d33bd60f982b1ff37c855
             9797a06ef4f0ef61c186324e2b350638 3606907b6a7c02b0f9f6157b53c867e4
             b9166c767b804d46a59b5216cde7a4e9 9040c5a40433225ee282a1b0a06c523e
             af4534d7f83fa1155b0047718cbc546a 0d072b04b3564eea1b422273f548271a
             0bb2316053fa76991955ebd63159434e cebb4e466dae5a1073a6727627097a10
             49e617d91d361094fa68f0ff77987130 305beaba2eda04df997b714d6c6f2c29
             a6ad5cb4022b02709b eead9d67890cbb22392336fea1851f38",
        );
        let expected = concat!(
            "Internet-Drafts are draft documents valid for a maximum of six months ",
            "and may be updated, replaced, or obsoleted by other documents at any ",
            "time. It is inappropriate to use Internet-Drafts as reference ",
            "material or to cite them other than as /\u{201c}work in progress./\u{201d}"
        )
        .as_bytes();
        assert_eq!(expected.len(), 265);
        assert_eq!(open(&key, &nonce, &aad, &sealed).unwrap(), expected);
        assert_eq!(seal(&key, &nonce, &aad, expected).unwrap(), sealed);
    }

    #[test]
    fn poly1305_final_reduction_and_empty_message() {
        for excess in 0..5 {
            let mut mac = Poly1305::new(&[0; 32]);
            mac.accumulator = [LIMB_MASK; 5];
            mac.accumulator[0] = LIMB_MASK - 4 + excess;
            let mut expected = [0; 16];
            expected[0] = excess as u8;
            assert_eq!(mac.finish(), expected);
        }
        let key = std::array::from_fn(|index| index as u8);
        assert_eq!(poly(&key, &[]).as_slice(), &key[16..]);
    }

    #[test]
    fn tampering_with_every_authenticated_input_is_rejected() {
        let key = [0x31; 32];
        let nonce = [0x45; 12];
        let aad = b"authenticated header";
        let sealed = seal(&key, &nonce, aad, b"secret message across two blocks").unwrap();
        for byte in 0..sealed.len() {
            for bit in 0..8 {
                let mut changed = sealed.clone();
                changed[byte] ^= 1 << bit;
                assert!(open(&key, &nonce, aad, &changed).is_err());
            }
        }
        for byte in 0..aad.len() {
            let mut changed = aad.to_vec();
            changed[byte] ^= 1;
            assert!(open(&key, &nonce, &changed, &sealed).is_err());
        }
        for byte in 0..key.len() {
            let mut changed = key;
            changed[byte] ^= 1;
            assert!(open(&changed, &nonce, aad, &sealed).is_err());
        }
        for byte in 0..nonce.len() {
            let mut changed = nonce;
            changed[byte] ^= 1;
            assert!(open(&key, &changed, aad, &sealed).is_err());
        }
        for length in 0..sealed.len() {
            assert!(open(&key, &nonce, aad, &sealed[..length]).is_err());
        }
        let mut extended = sealed;
        extended.push(0);
        assert!(open(&key, &nonce, aad, &extended).is_err());
    }

    #[test]
    fn empty_and_partial_aead_blocks_round_trip() {
        let key = [0; 32];
        let nonce = [0; 12];
        for aad_len in [0, 1, 15, 16, 17, 31, 32, 33] {
            let aad = vec![0x51; aad_len];
            for text_len in [0, 1, 15, 16, 17, 31, 32, 33, 63, 64, 65] {
                let text = vec![0x9a; text_len];
                let sealed = seal(&key, &nonce, &aad, &text).unwrap();
                assert_eq!(sealed.len(), text.len() + TAG_BYTES);
                assert_eq!(open(&key, &nonce, &aad, &sealed).unwrap(), text);
            }
        }
    }

    #[test]
    fn aead_input_limits_are_checked() {
        let key = [0; 32];
        let nonce = [0; 12];
        let max = vec![0x7a; MAX_INPUT_BYTES];
        let sealed = seal(&key, &nonce, &max, &max).unwrap();
        assert_eq!(open(&key, &nonce, &max, &sealed).unwrap(), max);
        let too_long = vec![0; MAX_INPUT_BYTES + 1];
        assert!(seal(&key, &nonce, &too_long, &[]).is_err());
        assert!(seal(&key, &nonce, &[], &too_long).is_err());
        assert!(open(&key, &nonce, &too_long, &[0; TAG_BYTES]).is_err());
        assert!(open(&key, &nonce, &[], &vec![0; MAX_INPUT_BYTES + TAG_BYTES + 1]).is_err());
    }

    #[test]
    fn chacha_counter_exhaustion_rejected_before_mutation() {
        let key = [0; 32];
        let nonce = [0; 12];
        let mut one = [0; 64];
        chacha_xor(&key, &nonce, u32::MAX, &mut one).unwrap();
        assert_eq!(one, chacha_block(&key, &nonce, u32::MAX));
        let mut too_long = [0x7a; 65];
        assert!(chacha_xor(&key, &nonce, u32::MAX, &mut too_long).is_err());
        assert_eq!(too_long, [0x7a; 65]);
        assert!(check_counter(u32::MAX - 1, 128).is_ok());
        assert!(check_counter(u32::MAX - 1, 129).is_err());
        assert!(check_counter(u32::MAX, 0).is_ok());
        if usize::BITS >= 64 {
            let maximum = u64::from(u32::MAX) * 64;
            assert!(check_counter(1, maximum as usize).is_ok());
            assert!(check_counter(1, (maximum + 1) as usize).is_err());
            assert!(check_counter(0, (maximum + 64) as usize).is_ok());
            assert!(check_counter(0, (maximum + 65) as usize).is_err());
        }
    }
}
