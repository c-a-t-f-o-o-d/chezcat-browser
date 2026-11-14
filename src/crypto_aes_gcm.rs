//! Original AES-128/AES-256 GCM with 96-bit nonces and 128-bit tags.
//!
//! AES follows FIPS 197 (2023 update), sections 4–5. GCM follows NIST
//! SP 800-38D, sections 6–7. No external implementation or crate is used.
//! S-box evaluation uses finite-field arithmetic on sixteen byte lanes;
//! GHASH uses fixed masked multiplication. Neither uses key-indexed tables.
//! These source-level properties do not guarantee compiler/CPU constant time
//! or guaranteed erasure, and this experimental code has not been audited.
//!
//! The caller must never repeat a nonce with the same key and must manage key
//! lifetimes. This module does not generate entropy or manage record counters.
//! <https://doi.org/10.6028/NIST.FIPS.197-upd1>
//! <https://doi.org/10.6028/NIST.SP.800-38D>

pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
pub const TAG_BYTES: usize = 16;

/// Encrypt a message and append its complete 16-byte authentication tag.
/// Only 16-byte and 32-byte keys are supported. AAD is authenticated unchanged.
pub fn seal(key: &[u8], nonce: &[u8; 12], aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    check_lengths(aad.len(), plaintext.len())?;
    let aes = Aes::new(key)?;
    let capacity = plaintext
        .len()
        .checked_add(TAG_BYTES)
        .ok_or("AES-GCM output length overflow.")?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| "Cannot allocate AES-GCM output.")?;
    output.extend_from_slice(plaintext);
    counter_xor(&aes, nonce, &mut output);
    let tag = authenticate(&aes, nonce, aad, &output);
    output.extend_from_slice(&tag);
    Ok(output)
}

/// Authenticate ciphertext and its appended tag before allocating or computing
/// any plaintext. Truncated tags and all authentication failures return errors.
pub fn open(
    key: &[u8],
    nonce: &[u8; 12],
    aad: &[u8],
    ciphertext_with_tag: &[u8],
) -> Result<Vec<u8>, String> {
    let length = ciphertext_with_tag
        .len()
        .checked_sub(TAG_BYTES)
        .ok_or("AES-GCM input is missing its 16-byte authentication tag.")?;
    check_lengths(aad.len(), length)?;
    let aes = Aes::new(key)?;
    let (ciphertext, received) = ciphertext_with_tag.split_at(length);
    let expected = authenticate(&aes, nonce, aad, ciphertext);
    if !tags_equal(&expected, received) {
        return Err("AES-GCM authentication failed.".into());
    }
    let mut plaintext = Vec::new();
    plaintext
        .try_reserve_exact(length)
        .map_err(|_| "Cannot allocate AES-GCM plaintext.")?;
    plaintext.extend_from_slice(ciphertext);
    counter_xor(&aes, nonce, &mut plaintext);
    Ok(plaintext)
}

fn check_lengths(aad_length: usize, text_length: usize) -> Result<(), String> {
    if aad_length > MAX_INPUT_BYTES || text_length > MAX_INPUT_BYTES {
        return Err("AES-GCM inputs are limited to 1 MiB each.".into());
    }
    // At most 65536 payload blocks use counters 2..=65537. Thus no counter
    // wraps or collides with J0, and both 64-bit bit-length fields fit exactly.
    Ok(())
}

#[inline(never)]
fn tags_equal(expected: &[u8; TAG_BYTES], received: &[u8]) -> bool {
    debug_assert_eq!(received.len(), TAG_BYTES);
    let mut difference = 0u8;
    for index in 0..TAG_BYTES {
        difference |= expected[index] ^ received[index];
    }
    difference == 0
}

// The low bit of each of sixteen independent byte lanes in a u128.
const BYTE_ONES: u128 = 0x0101_0101_0101_0101_0101_0101_0101_0101;
const LOW_SEVEN: u128 = BYTE_ONES * 0x7f;

// Multiply each byte by x modulo x^8+x^4+x^3+x+1 (the AES polynomial).
#[inline]
fn times_x(bytes: u128) -> u128 {
    ((bytes & LOW_SEVEN) << 1) ^ (((bytes >> 7) & BYTE_ONES).wrapping_mul(0x1b))
}

// Each selected low bit expands to an entire byte mask. The factors occupy
// disjoint byte lanes, so integer multiplication by 0xff creates no carries
// between lanes. Exactly eight iterations handle every possible input byte.
#[inline]
fn field_multiply(mut left: u128, mut right: u128) -> u128 {
    let mut product = 0u128;
    for _ in 0..8 {
        product ^= left & (right & BYTE_ONES).wrapping_mul(0xff);
        left = times_x(left);
        right = (right >> 1) & LOW_SEVEN;
    }
    product
}

// Squaring is linear in characteristic two. Squaring the eight polynomial
// basis values and reducing modulo 0x11b gives 01,04,10,40,1b,6c,ab,9a.
// Apply those fixed coefficients to every byte in parallel, with no lookup.
#[inline]
fn field_square(bytes: u128) -> u128 {
    (bytes & BYTE_ONES)
        ^ (((bytes >> 1) & BYTE_ONES) << 2)
        ^ (((bytes >> 2) & BYTE_ONES) << 4)
        ^ (((bytes >> 3) & BYTE_ONES) << 6)
        ^ ((bytes >> 4) & BYTE_ONES).wrapping_mul(0x1b)
        ^ ((bytes >> 5) & BYTE_ONES).wrapping_mul(0x6c)
        ^ ((bytes >> 6) & BYTE_ONES).wrapping_mul(0xab)
        ^ ((bytes >> 7) & BYTE_ONES).wrapping_mul(0x9a)
}

#[inline]
fn rotate_each_byte(bytes: u128, count: u32) -> u128 {
    let high_mask = BYTE_ONES * u128::from((0xffu8) << count);
    let low_mask = BYTE_ONES * ((1u128 << count) - 1);
    ((bytes << count) & high_mask) | ((bytes >> (8 - count)) & low_mask)
}

fn invert_bytes(bytes: u128) -> u128 {
    // A fixed addition chain for a^254 = a^-1 in GF(2^8); it also maps zero
    // to zero. Four multiplications and seven squarings work on all 16 bytes.
    let a2 = field_square(bytes);
    let a3 = field_multiply(a2, bytes);
    let a6 = field_square(a3);
    let a7 = field_multiply(a6, bytes);
    let a14 = field_square(a7);
    let a15 = field_multiply(a14, bytes);
    let a30 = field_square(a15);
    let a60 = field_square(a30);
    let a120 = field_square(a60);
    let a127 = field_multiply(a120, a7);
    field_square(a127)
}

fn sub_bytes(bytes: u128) -> u128 {
    let inverse = invert_bytes(bytes);
    // FIPS 197 equation 5.3, expressed as byte-local rotations and XOR.
    inverse
        ^ rotate_each_byte(inverse, 1)
        ^ rotate_each_byte(inverse, 2)
        ^ rotate_each_byte(inverse, 3)
        ^ rotate_each_byte(inverse, 4)
        ^ (BYTE_ONES * 0x63)
}

fn inverse_sub_bytes(bytes: u128) -> u128 {
    // Inverse of FIPS 197's affine map, followed by the same multiplicative
    // inverse as encryption. Every byte lane follows the same arithmetic.
    invert_bytes(
        rotate_each_byte(bytes, 1)
            ^ rotate_each_byte(bytes, 3)
            ^ rotate_each_byte(bytes, 6)
            ^ (BYTE_ONES * 0x05),
    )
}

// State bytes are packed little-endian in the FIPS column-major order:
// byte 4*c+r is row r, column c. These masks select fixed, public positions.
fn shift_rows(state: u128) -> u128 {
    const ROW_ZERO: u128 = 0x0000_00ff_0000_00ff_0000_00ff_0000_00ff;
    (state & ROW_ZERO)
        | (state.rotate_right(32) & (ROW_ZERO << 8))
        | (state.rotate_right(64) & (ROW_ZERO << 16))
        | (state.rotate_right(96) & (ROW_ZERO << 24))
}

fn inverse_shift_rows(state: u128) -> u128 {
    const ROW_ZERO: u128 = 0x0000_00ff_0000_00ff_0000_00ff_0000_00ff;
    (state & ROW_ZERO)
        | (state.rotate_left(32) & (ROW_ZERO << 8))
        | (state.rotate_left(64) & (ROW_ZERO << 16))
        | (state.rotate_left(96) & (ROW_ZERO << 24))
}

fn mix_columns(state: u128) -> u128 {
    // Rotate one byte within each 32-bit column, without crossing columns.
    let next = ((state >> 8) & 0x00ff_ffff_00ff_ffff_00ff_ffff_00ff_ffff)
        | ((state << 24) & 0xff00_0000_ff00_0000_ff00_0000_ff00_0000);
    let adjacent = state ^ next;
    let opposite = ((adjacent >> 16) & 0x0000_ffff_0000_ffff_0000_ffff_0000_ffff)
        | ((adjacent << 16) & 0xffff_0000_ffff_0000_ffff_0000_ffff_0000);
    // Each output byte is a_i XOR (a_0 XOR ... XOR a_3)
    // XOR 2*(a_i XOR a_(i+1)), equivalent to the specified AES matrix.
    state ^ adjacent ^ opposite ^ times_x(adjacent)
}

fn inverse_mix_columns(state: u128) -> u128 {
    // Multiplying by the inverse AES matrix is equivalent to adding
    // 4*(a_i XOR a_(i+2)) in each column before the forward matrix.
    let opposite = ((state >> 16) & 0x0000_ffff_0000_ffff_0000_ffff_0000_ffff)
        | ((state << 16) & 0xffff_0000_ffff_0000_ffff_0000_ffff_0000);
    mix_columns(state ^ times_x(times_x(state ^ opposite)))
}

/// Shared original AES block primitive for GCM and TLS 1.2 CBC. Callers must
/// supply an authenticated mode; a raw AES block is not a secure protocol.
pub(crate) struct Aes {
    round_keys: [u128; 15],
    rounds: usize,
}

impl Aes {
    pub(crate) fn new(key: &[u8]) -> Result<Self, String> {
        if !matches!(key.len(), 16 | 32) {
            return Err("AES requires a 16-byte or 32-byte key.".into());
        }
        let key_words = key.len() / 4;
        let rounds = key_words + 6;
        let mut words = [0u32; 60];
        for (word, bytes) in words.iter_mut().zip(key.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*bytes);
        }
        let mut round_constant = 1u8;
        for index in key_words..4 * (rounds + 1) {
            let mut previous = words[index - 1];
            if index % key_words == 0 {
                previous = sub_bytes(u128::from(previous.rotate_left(8))) as u32
                    ^ (u32::from(round_constant) << 24);
                round_constant = times_x(u128::from(round_constant)) as u8;
            } else if key_words == 8 && index % key_words == 4 {
                previous = sub_bytes(u128::from(previous)) as u32;
            }
            words[index] = words[index - key_words] ^ previous;
        }
        let mut round_keys = [0u128; 15];
        for (key, chunk) in round_keys.iter_mut().zip(words.as_chunks::<4>().0) {
            let mut block = [0u8; 16];
            for (destination, word) in block.as_chunks_mut::<4>().0.iter_mut().zip(chunk) {
                *destination = word.to_be_bytes();
            }
            *key = u128::from_le_bytes(block);
        }
        Ok(Self { round_keys, rounds })
    }

    pub(crate) fn encrypt(&self, block: &[u8; 16]) -> [u8; 16] {
        let mut state = u128::from_le_bytes(*block) ^ self.round_keys[0];
        for round in 1..self.rounds {
            state = mix_columns(shift_rows(sub_bytes(state))) ^ self.round_keys[round];
        }
        state = shift_rows(sub_bytes(state)) ^ self.round_keys[self.rounds];
        state.to_le_bytes()
    }

    pub(crate) fn decrypt(&self, block: &[u8; 16]) -> [u8; 16] {
        let mut state = u128::from_le_bytes(*block) ^ self.round_keys[self.rounds];
        for round in (1..self.rounds).rev() {
            state = inverse_mix_columns(
                inverse_sub_bytes(inverse_shift_rows(state)) ^ self.round_keys[round],
            );
        }
        state = inverse_sub_bytes(inverse_shift_rows(state)) ^ self.round_keys[0];
        state.to_le_bytes()
    }
}

fn counter_xor(aes: &Aes, nonce: &[u8; 12], bytes: &mut [u8]) {
    debug_assert!(bytes.len() <= MAX_INPUT_BYTES);
    let mut counter = [0u8; 16];
    counter[..12].copy_from_slice(nonce);
    for (index, chunk) in bytes.chunks_mut(16).enumerate() {
        // The checked 1 MiB bound guarantees index + 2 <= 65537.
        counter[12..].copy_from_slice(&((index as u32) + 2).to_be_bytes());
        let mask = aes.encrypt(&counter);
        for (byte, stream) in chunk.iter_mut().zip(mask) {
            *byte ^= stream;
        }
    }
}

// NIST SP 800-38D Algorithm 1. The most significant encoded bit represents
// the constant polynomial coefficient; reduction therefore shifts right.
fn ghash_multiply(mut left: u128, mut right: u128) -> u128 {
    const REDUCTION: u128 = 0xe100_0000_0000_0000_0000_0000_0000_0000;
    let mut product = 0u128;
    for _ in 0..128 {
        product ^= right & 0u128.wrapping_sub(left >> 127);
        right = (right >> 1) ^ (REDUCTION & 0u128.wrapping_sub(right & 1));
        left <<= 1;
    }
    product
}

fn ghash_part(accumulator: &mut u128, hash_key: u128, bytes: &[u8]) {
    let (blocks, tail) = bytes.as_chunks::<16>();
    for block in blocks {
        *accumulator = ghash_multiply(*accumulator ^ u128::from_be_bytes(*block), hash_key);
    }
    if !tail.is_empty() {
        let mut padded = [0u8; 16];
        padded[..tail.len()].copy_from_slice(tail);
        *accumulator = ghash_multiply(*accumulator ^ u128::from_be_bytes(padded), hash_key);
    }
}

fn authenticate(aes: &Aes, nonce: &[u8; 12], aad: &[u8], ciphertext: &[u8]) -> [u8; 16] {
    let hash_key = u128::from_be_bytes(aes.encrypt(&[0u8; 16]));
    let mut accumulator = 0u128;
    // Padding is applied to each input separately, not to their concatenation.
    ghash_part(&mut accumulator, hash_key, aad);
    ghash_part(&mut accumulator, hash_key, ciphertext);
    let length_block = ((aad.len() as u128 * 8) << 64) | (ciphertext.len() as u128 * 8);
    accumulator = ghash_multiply(accumulator ^ length_block, hash_key);
    let mut initial = [0u8; 16];
    initial[..12].copy_from_slice(nonce);
    initial[15] = 1;
    (accumulator ^ u128::from_be_bytes(aes.encrypt(&initial))).to_be_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        text.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    // An independent polynomial-product/remainder calculation for testing
    // packed lane boundaries, rather than repeating the runtime xtime method.
    fn polynomial_product(a: u8, b: u8) -> u8 {
        let mut product = 0u16;
        for bit in 0..8 {
            if b & (1 << bit) != 0 {
                product ^= u16::from(a) << bit;
            }
        }
        for bit in (8..=14).rev() {
            if product & (1 << bit) != 0 {
                product ^= 0x11b << (bit - 8);
            }
        }
        product as u8
    }

    #[test]
    fn packed_field_and_sbox_match_the_polynomial_definition() {
        for value in 0..=255u8 {
            let lanes: [u8; 16] = std::array::from_fn(|i| value.wrapping_add(i as u8));
            let packed = u128::from_le_bytes(lanes);
            let squared = field_square(packed).to_le_bytes();
            let substituted = sub_bytes(packed).to_le_bytes();
            for i in 0..16 {
                assert_eq!(squared[i], polynomial_product(lanes[i], lanes[i]));
                let inverse = if lanes[i] == 0 {
                    0
                } else {
                    (1..=255u8)
                        .find(|&candidate| polynomial_product(lanes[i], candidate) == 1)
                        .unwrap()
                };
                let mut expected = 0x63u8;
                for bit in 0..8 {
                    let output = ((inverse >> bit)
                        ^ (inverse >> ((bit + 4) % 8))
                        ^ (inverse >> ((bit + 5) % 8))
                        ^ (inverse >> ((bit + 6) % 8))
                        ^ (inverse >> ((bit + 7) % 8)))
                        & 1;
                    expected ^= output << bit;
                }
                assert_eq!(substituted[i], expected);
            }
            assert_eq!(inverse_sub_bytes(sub_bytes(packed)), packed);
            assert_eq!(sub_bytes(inverse_sub_bytes(packed)), packed);
            assert_eq!(inverse_shift_rows(shift_rows(packed)), packed);
            assert_eq!(inverse_mix_columns(mix_columns(packed)), packed);
        }
    }

    #[test]
    fn fips197_block_examples() {
        let input: [u8; 16] = hex("00112233445566778899aabbccddeeff").try_into().unwrap();
        for (key, output) in [
            (
                "000102030405060708090a0b0c0d0e0f",
                "69c4e0d86a7b0430d8cdb78070b4c55a",
            ),
            (
                "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
                "8ea2b7ca516745bfeafc49904b496089",
            ),
        ] {
            assert_eq!(
                Aes::new(&hex(key)).unwrap().encrypt(&input).as_slice(),
                hex(output)
            );
            let ciphertext: [u8; 16] = hex(output).try_into().unwrap();
            assert_eq!(Aes::new(&hex(key)).unwrap().decrypt(&ciphertext), input);
        }
    }

    // NIST's AES_GCM.pdf, AES-128 and AES-256 examples 1–5 (128-bit tags).
    // https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Standards-and-Guidelines/documents/examples/AES_GCM.pdf
    #[test]
    fn nist_gcm_published_examples() {
        let key = hex("feffe9928665731c6d6a8f9467308308");
        let nonce: [u8; 12] = hex("cafebabefacedbaddecaf888").try_into().unwrap();
        let plaintext = hex(concat!(
            "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72",
            "1c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b391aafd255"
        ));
        let aad = hex(concat!(
            "3ad77bb40d7a3660a89ecaf32466ef97f5d3d58503b9699de785895a96fdbaaf",
            "43b1cd7f598ece23881b00e3ed0306887b0c785e27e8ad3f8223207104725dd4"
        ));
        for (key, ciphertext, tags) in [
            (
                key.clone(),
                concat!(
                    "42831ec2217774244b7221b784d0d49ce3aa212f2c02a4e035c17e2329aca12e",
                    "21d514b25466931c7d8f6a5aac84aa051ba30b396a0aac973d58e091473f5985"
                ),
                [
                    "3247184b3c4f69a44dbcd22887bbb418",
                    "4d5c2af327cd64a62cf35abd2ba6fab4",
                    "5f91d77123ef5eb9997913849b8dc1e9",
                    "64c0232904af398a5b67c10b53a5024d",
                    "f07c2528eea2fca1211f905e1b6a881b",
                ],
            ),
            (
                [key.as_slice(), key.as_slice()].concat(),
                concat!(
                    "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa",
                    "8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662898015ad"
                ),
                [
                    "fd2caa16a5832e76aa132c1453eeda7e",
                    "b094dac5d93471bdec1a502270e3cc6c",
                    "de34b6dcd4cee2fdbec3cea01af1ee44",
                    "c06d76f31930fef37acae23ed465ae62",
                    "e097195f4532da895fb917a5a55c6aa0",
                ],
            ),
        ] {
            let ciphertext = hex(ciphertext);
            for ((a, p), tag) in [(0, 0), (0, 64), (64, 0), (64, 64), (20, 60)]
                .into_iter()
                .zip(tags)
            {
                let mut expected = ciphertext[..p].to_vec();
                expected.extend_from_slice(&hex(tag));
                assert_eq!(
                    seal(&key, &nonce, &aad[..a], &plaintext[..p]).unwrap(),
                    expected
                );
                assert_eq!(
                    open(&key, &nonce, &aad[..a], &expected).unwrap(),
                    plaintext[..p]
                );
            }
        }
    }

    #[test]
    fn tampering_each_input_never_returns_plaintext() {
        let nonce = [0x42; 12];
        let aad = b"authenticated header";
        let plaintext = b"secret message with a partial last block";
        for key_length in [16, 32] {
            let key = vec![0x91; key_length];
            let sealed = seal(&key, &nonce, aad, plaintext).unwrap();
            for index in 0..sealed.len() {
                let mut changed = sealed.clone();
                changed[index] ^= 1;
                assert!(open(&key, &nonce, aad, &changed).is_err());
            }
            for index in 0..key.len() {
                let mut changed = key.clone();
                changed[index] ^= 1;
                assert!(open(&changed, &nonce, aad, &sealed).is_err());
            }
            for index in 0..nonce.len() {
                let mut changed = nonce;
                changed[index] ^= 1;
                assert!(open(&key, &changed, aad, &sealed).is_err());
            }
            for index in 0..aad.len() {
                let mut changed = aad.to_vec();
                changed[index] ^= 1;
                assert!(open(&key, &nonce, &changed, &sealed).is_err());
            }
            assert!(open(&key, &nonce, aad, &sealed[..sealed.len() - 1]).is_err());
            let mut extended = sealed;
            extended.push(0);
            assert!(open(&key, &nonce, aad, &extended).is_err());
        }
    }

    #[test]
    fn exact_key_tag_and_resource_limits() {
        let nonce = [0u8; 12];
        for length in [0, 1, 15, 17, 24, 31, 33, 64] {
            let key = vec![0u8; length];
            assert!(seal(&key, &nonce, b"", b"").is_err());
            assert!(open(&key, &nonce, b"", &[0; 16]).is_err());
        }
        for length in 0..16 {
            assert!(open(&[0; 16], &nonce, b"", &vec![0; length]).is_err());
        }
        assert!(check_lengths(MAX_INPUT_BYTES, MAX_INPUT_BYTES).is_ok());
        for (aad, text) in [
            (MAX_INPUT_BYTES + 1, 0),
            (0, MAX_INPUT_BYTES + 1),
            (usize::MAX, 0),
            (0, usize::MAX),
        ] {
            assert!(check_lengths(aad, text).is_err());
        }
        let too_large = vec![0; MAX_INPUT_BYTES + 1];
        assert!(seal(&[0; 16], &nonce, &too_large, b"").is_err());
        assert!(seal(&[0; 16], &nonce, b"", &too_large).is_err());
        assert!(open(&[0; 16], &nonce, &too_large, &[0; 16]).is_err());
        assert!(open(
            &[0; 16],
            &nonce,
            b"",
            &vec![0; MAX_INPUT_BYTES + TAG_BYTES + 1]
        )
        .is_err());
    }
}
