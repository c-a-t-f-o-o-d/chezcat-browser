//! Original SHA-256, HMAC-SHA256, and HKDF-SHA256 implementations.
//!
//! Algorithm definitions: FIPS 180-4 sections 4–6, RFC 2104 section 2,
//! and RFC 5869 section 2. These experimental primitives do not implement TLS,
//! certificate authentication, random-number generation, or guaranteed erasure.
//! No external cryptographic library is used by this module.

const MAX_MESSAGE_BYTES: u64 = u64::MAX / 8;
const HASH_BYTES: usize = 32;
const BLOCK_BYTES: usize = 64;

// FIPS 180-4 section 4.2.2: fractional cube-root bits of the first 64 primes.
const ROUND_CONSTANTS: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// A cloneable SHA-256 state. Memory use is independent of the message size.
/// Cloning permits a transcript prefix to be finalized or extended separately.
#[derive(Clone)]
pub struct Sha256 {
    state: [u32; 8],
    block: [u8; BLOCK_BYTES],
    used: usize,
    bytes: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Self {
            // FIPS 180-4 section 5.3.3: fractional square-root bits.
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            block: [0; BLOCK_BYTES],
            used: 0,
            bytes: 0,
        }
    }

    /// Append bytes, rejecting a total message length of 2^64 bits or more.
    /// An error leaves the existing state unchanged.
    pub fn update(&mut self, mut input: &[u8]) -> Result<(), String> {
        let incoming = u64::try_from(input.len()).map_err(|_| "SHA-256 input is too long.")?;
        let total = self
            .bytes
            .checked_add(incoming)
            .filter(|&length| length <= MAX_MESSAGE_BYTES)
            .ok_or("SHA-256 message must be shorter than 2^64 bits.")?;
        self.bytes = total;
        if self.used != 0 {
            let take = (BLOCK_BYTES - self.used).min(input.len());
            self.block[self.used..self.used + take].copy_from_slice(&input[..take]);
            self.used += take;
            input = &input[take..];
            if self.used != BLOCK_BYTES {
                return Ok(());
            }
            compress(&mut self.state, &self.block);
            self.used = 0;
        }
        let (blocks, tail) = input.as_chunks::<BLOCK_BYTES>();
        for block in blocks {
            compress(&mut self.state, block);
        }
        self.block[..tail.len()].copy_from_slice(tail);
        self.used = tail.len();
        Ok(())
    }

    /// Consume this state and append SHA-256's bit marker and length padding.
    pub fn finalize(mut self) -> [u8; HASH_BYTES] {
        self.block[self.used] = 0x80;
        self.used += 1;
        self.block[self.used..].fill(0);
        if self.used > 56 {
            compress(&mut self.state, &self.block);
            self.block.fill(0);
        }
        self.block[56..].copy_from_slice(&(self.bytes * 8).to_be_bytes());
        compress(&mut self.state, &self.block);
        let mut digest = [0u8; HASH_BYTES];
        for (word, chunk) in self.state.iter().zip(digest.as_chunks_mut::<4>().0) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        digest
    }
}

fn compress(state: &mut [u32; 8], block: &[u8; BLOCK_BYTES]) {
    let mut words = [0u32; 64];
    for (word, bytes) in words[..16].iter_mut().zip(block.as_chunks::<4>().0) {
        *word = u32::from_be_bytes(*bytes);
    }
    for index in 16..64 {
        let early = words[index - 15];
        let late = words[index - 2];
        let sigma0 = early.rotate_right(7) ^ early.rotate_right(18) ^ (early >> 3);
        let sigma1 = late.rotate_right(17) ^ late.rotate_right(19) ^ (late >> 10);
        words[index] = words[index - 16]
            .wrapping_add(sigma0)
            .wrapping_add(words[index - 7])
            .wrapping_add(sigma1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for index in 0..64 {
        let upper = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let choice = (e & f) ^ (!e & g);
        let first = h
            .wrapping_add(upper)
            .wrapping_add(choice)
            .wrapping_add(ROUND_CONSTANTS[index])
            .wrapping_add(words[index]);
        let lower = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let second = lower.wrapping_add(majority);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(first);
        d = c;
        c = b;
        b = a;
        a = first.wrapping_add(second);
    }
    for (accumulator, word) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *accumulator = accumulator.wrapping_add(word);
    }
}

/// Hash one byte slice. Panics only if it exceeds SHA-256's 2^64-bit limit.
pub fn sha256(data: &[u8]) -> [u8; HASH_BYTES] {
    let mut hash = Sha256::new();
    hash.update(data)
        .expect("SHA-256 input exceeds the algorithm's length limit");
    hash.finalize()
}

#[derive(Clone)]
struct Hmac {
    inner: Sha256,
    outer: Sha256,
}

impl Hmac {
    fn new(key: &[u8]) -> Self {
        let mut padded = [0u8; BLOCK_BYTES];
        if key.len() > BLOCK_BYTES {
            padded[..HASH_BYTES].copy_from_slice(&sha256(key));
        } else {
            padded[..key.len()].copy_from_slice(key);
        }
        let mut inner = Sha256::new();
        let mut outer = Sha256::new();
        let mut pad = [0u8; BLOCK_BYTES];
        for (output, key_byte) in pad.iter_mut().zip(padded) {
            *output = key_byte ^ 0x36;
        }
        inner.update(&pad).expect("fixed-size HMAC inner pad");
        for (output, key_byte) in pad.iter_mut().zip(padded) {
            *output = key_byte ^ 0x5c;
        }
        outer.update(&pad).expect("fixed-size HMAC outer pad");
        Self { inner, outer }
    }

    fn update(&mut self, data: &[u8]) -> Result<(), String> {
        self.inner.update(data)
    }

    fn finalize(mut self) -> [u8; HASH_BYTES] {
        self.outer
            .update(&self.inner.finalize())
            .expect("fixed-size HMAC outer digest");
        self.outer.finalize()
    }
}

/// RFC 2104 HMAC with SHA-256. Returns the complete 32-byte authentication tag.
/// Data plus its 64-byte inner pad must fit SHA-256's message-length limit.
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; HASH_BYTES] {
    let mut mac = Hmac::new(key);
    mac.update(data)
        .expect("HMAC input exceeds SHA-256's length limit");
    mac.finalize()
}

/// RFC 5869 extraction; an empty salt denotes 32 zero bytes.
pub fn hkdf_extract(salt: &[u8], ikm: &[u8]) -> [u8; HASH_BYTES] {
    if salt.is_empty() {
        hmac_sha256(&[0; HASH_BYTES], ikm)
    } else {
        hmac_sha256(salt, ikm)
    }
}

/// RFC 5869 expansion. At most 255 SHA-256 blocks (8,160 bytes) may be requested.
/// The context is streamed into HMAC; no context-sized temporary is allocated.
pub fn hkdf_expand(prk: &[u8; HASH_BYTES], info: &[u8], length: usize) -> Result<Vec<u8>, String> {
    if length > 255 * HASH_BYTES {
        return Err("HKDF-SHA256 output cannot exceed 8160 bytes.".into());
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| "Not enough memory for HKDF output.")?;
    let prepared = Hmac::new(prk);
    let mut previous = [0u8; HASH_BYTES];
    let mut previous_length = 0;
    for counter in 1..=length.div_ceil(HASH_BYTES) {
        let mut block = prepared.clone();
        block.update(&previous[..previous_length])?;
        block.update(info)?;
        block.update(&[counter as u8])?;
        previous = block.finalize();
        previous_length = HASH_BYTES;
        let take = HASH_BYTES.min(length - output.len());
        output.extend_from_slice(&previous[..take]);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(input: &str) -> Vec<u8> {
        input
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn sha256_examples_and_streaming_boundaries() {
        // NIST's SHA256.pdf one-block and two-block message samples, plus
        // the empty-message and million-'a' known-answer cases.
        for (input, digest) in [
            (
                b"".as_slice(),
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                b"abc",
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
                "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
        ] {
            assert_eq!(sha256(input).as_slice(), hex(digest));
        }
        let mut million = Sha256::new();
        for _ in 0..1000 {
            million.update(&[b'a'; 1000]).unwrap();
        }
        assert_eq!(
            million.finalize().as_slice(),
            hex("cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0")
        );

        let bytes: Vec<u8> = (0..257).map(|n| (n * 37) as u8).collect();
        for size in [
            0, 1, 55, 56, 63, 64, 65, 119, 120, 127, 128, 129, 255, 256, 257,
        ] {
            let expected = sha256(&bytes[..size]);
            for split in 0..=size {
                let mut prefix = Sha256::new();
                prefix.update(&bytes[..split]).unwrap();
                assert_eq!(prefix.clone().finalize(), sha256(&bytes[..split]));
                prefix.update(&[]).unwrap();
                prefix.update(&bytes[split..size]).unwrap();
                assert_eq!(prefix.finalize(), expected, "size={size}, split={split}");
            }
        }
    }

    #[test]
    fn message_length_rejection_preserves_state() {
        let mut state = Sha256::new();
        state.bytes = MAX_MESSAGE_BYTES;
        state.used = 63;
        let before = state.clone();
        assert!(state.update(b"x").is_err());
        assert_eq!(state.bytes, before.bytes);
        assert_eq!(state.block, before.block);
        assert_eq!(state.state, before.state);
        assert_eq!(state.used, before.used);
        state.update(&[]).unwrap();
        let _ = state.finalize(); // Padding is not included in the message limit.
    }

    #[test]
    fn rfc4231_all_sha256_cases() {
        let cases = [
            (vec![0x0b; 20], b"Hi There".to_vec(), "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"),
            (b"Jefe".to_vec(), b"what do ya want for nothing?".to_vec(), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"),
            (vec![0xaa; 20], vec![0xdd; 50], "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"),
            ((1..=25).collect(), vec![0xcd; 50], "82558a389a443c0ea4cc819899f2083a85f0faa3e578f8077a2e3ff46729665b"),
            (vec![0x0c; 20], b"Test With Truncation".to_vec(), "a3b6167473100ee06e0c796c2955552b"),
            (vec![0xaa; 131], b"Test Using Larger Than Block-Size Key - Hash Key First".to_vec(), "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"),
            (vec![0xaa; 131], b"This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm.".to_vec(), "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2"),
        ];
        for (key, data, expected) in cases {
            let expected = hex(expected);
            assert_eq!(&hmac_sha256(&key, &data)[..expected.len()], expected);
        }
    }

    #[test]
    fn rfc5869_all_sha256_cases_and_output_bounds() {
        let cases = [
            (vec![0x0b; 22], (0..=0x0c).collect::<Vec<u8>>(), (0xf0..=0xf9).collect::<Vec<u8>>(),
                "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5",
                "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"),
            ((0..=0x4f).collect(), (0x60..=0xaf).collect(), (0xb0..=0xff).collect(),
                "06a6b88c5853361a06104c9ceb35b45cef760014904671014a193f40c15fc244",
                "b11e398dc80327a1c8e7f78c596a49344f012eda2d4efad8a050cc4c19afa97c59045a99cac7827271cb41c65e590e09da3275600c2f09b8367793a9aca3db71cc30c58179ec3e87c14c01d5c1f3434f1d87"),
            (vec![0x0b; 22], vec![], vec![],
                "19ef24a32c717b167f33a91d6f648bdf96596776afdb6377ac434c1c293ccb04",
                "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d9d201395faa4b61a96c8"),
        ];
        for (ikm, salt, info, expected_prk, expected_okm) in cases {
            let prk = hkdf_extract(&salt, &ikm);
            assert_eq!(prk.as_slice(), hex(expected_prk));
            let expected = hex(expected_okm);
            assert_eq!(hkdf_expand(&prk, &info, expected.len()).unwrap(), expected);
        }
        let prk = hkdf_extract(&[], b"key");
        assert!(hkdf_expand(&prk, b"context", 0).unwrap().is_empty());
        assert_eq!(hkdf_expand(&prk, b"context", 8160).unwrap().len(), 8160);
        assert!(hkdf_expand(&prk, b"context", 8161).is_err());
        assert!(hkdf_expand(&prk, b"context", usize::MAX).is_err());
    }
}
