//! Original RSA public-signature verification from RFC 8017 sections 5, 8, 9.
//!
//! This module performs public operations only: no signing, private exponents,
//! encryption, or key generation. Its arithmetic need not hide public values
//! and must not be reused for private RSA operations. Fixed-size storage bounds
//! the arithmetic to odd 2048–4096-bit moduli and odd 32-bit public exponents.
//! Key validation here checks representation and policy, not factorization.
//! <https://www.rfc-editor.org/rfc/rfc8017.html>

use std::cmp::Ordering;

const MAX_BYTES: usize = 512;
const MAX_WORDS: usize = MAX_BYTES / 4;
const INVALID_SIGNATURE: &str = "RSA signature verification failed.";
type Integer = [u32; MAX_WORDS];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hash {
    Sha256,
    Sha384,
    Sha512,
}

impl Hash {
    fn length(self) -> usize {
        match self {
            Self::Sha256 => 32,
            Self::Sha384 => 48,
            Self::Sha512 => 64,
        }
    }

    fn digest(self, message: &[u8]) -> Result<[u8; 64], String> {
        let mut output = [0u8; 64];
        match self {
            Self::Sha256 => {
                let mut state = crate::crypto_hash::Sha256::new();
                state.update(message)?;
                output[..32].copy_from_slice(&state.finalize());
            }
            Self::Sha384 => {
                output[..48].copy_from_slice(&crate::crypto_sha512::sha384(message));
            }
            Self::Sha512 => output = crate::crypto_sha512::sha512(message),
        }
        Ok(output)
    }

    // RFC 8017 section 9.2: the exact DER DigestInfo prefix, including NULL.
    fn digest_info(self) -> [u8; 19] {
        let (outer_length, oid_last, digest_length) = match self {
            Self::Sha256 => (0x31, 1, 0x20),
            Self::Sha384 => (0x41, 2, 0x30),
            Self::Sha512 => (0x51, 3, 0x40),
        };
        [
            0x30,
            outer_length,
            0x30,
            0x0d,
            0x06,
            0x09,
            0x60,
            0x86,
            0x48,
            0x01,
            0x65,
            0x03,
            0x04,
            0x02,
            oid_last,
            0x05,
            0x00,
            0x04,
            digest_length,
        ]
    }
}

/// Check canonical unsigned big-endian magnitudes and the supported key policy.
/// A DER reader must remove its one required INTEGER sign-padding byte first.
/// Leading-zero magnitudes, even moduli/exponents, and exponents over u32 fail.
pub fn validate_key(n: &[u8], e: &[u8]) -> Result<(), String> {
    parameters(n, e).map(|_| ())
}

fn parameters(n: &[u8], e: &[u8]) -> Result<(usize, u32), String> {
    if n.is_empty() || n.len() > MAX_BYTES || n[0] == 0 {
        return Err("RSA modulus must be a canonical 2048–4096-bit magnitude.".into());
    }
    let bits = (n.len() - 1) * 8 + (8 - n[0].leading_zeros() as usize);
    if !(2048..=4096).contains(&bits) || n[n.len() - 1] & 1 == 0 {
        return Err("RSA modulus must be odd and contain 2048–4096 bits.".into());
    }
    if e.is_empty() || e.len() > 4 || e[0] == 0 {
        return Err("RSA exponent must be a canonical odd 32-bit magnitude.".into());
    }
    let exponent = e
        .iter()
        .fold(0u32, |value, byte| (value << 8) | u32::from(*byte));
    if exponent < 3 || exponent & 1 == 0 {
        return Err("RSA exponent must be odd and at least three.".into());
    }
    Ok((bits, exponent))
}

/// Verify the complete message using strict DER EMSA-PKCS1-v1_5 encoding.
/// The signature must contain exactly ceil(modulus_bits / 8) bytes.
pub fn verify_pkcs1v15(
    n: &[u8],
    e: &[u8],
    hash: Hash,
    message: &[u8],
    signature: &[u8],
) -> Result<(), String> {
    let encoded = public_operation(n, e, signature)?;
    let prefix = hash.digest_info();
    let digest = hash.digest(message)?;
    let digest_length = hash.length();
    // All supported keys have room for the digest and at least eight FF bytes.
    let separator = encoded.length - prefix.len() - digest_length - 1;
    let mut expected = [0u8; MAX_BYTES];
    expected[1] = 1;
    expected[2..separator].fill(0xff);
    expected[separator + 1..separator + 1 + prefix.len()].copy_from_slice(&prefix);
    expected[encoded.length - digest_length..encoded.length]
        .copy_from_slice(&digest[..digest_length]);
    if !equal(
        &encoded.bytes[..encoded.length],
        &expected[..encoded.length],
    ) {
        return Err(INVALID_SIGNATURE.into());
    }
    Ok(())
}

/// Verify EMSA-PSS with the specified, exact salt length and MGF1(hash).
/// No automatic salt-length detection, mixed MGF hash, or alternate trailer.
pub fn verify_pss(
    n: &[u8],
    e: &[u8],
    hash: Hash,
    salt_len: usize,
    message: &[u8],
    signature: &[u8],
) -> Result<(), String> {
    let (modulus_bits, _) = parameters(n, e)?;
    let encoded_bits = modulus_bits - 1;
    let encoded_len = encoded_bits.div_ceil(8);
    let hash_len = hash.length();
    if salt_len > encoded_len - hash_len - 2 {
        return Err("RSA-PSS salt length exceeds the encoded message capacity.".into());
    }
    let recovered = public_operation(n, e, signature)?;
    // For a modulus with bit length congruent to 1 modulo 8, emLen = k - 1.
    let start = recovered.length - encoded_len;
    if recovered.bytes[..start].iter().any(|&byte| byte != 0) {
        return Err(INVALID_SIGNATURE.into());
    }
    let encoded = &recovered.bytes[start..recovered.length];
    let data_len = encoded_len - hash_len - 1;
    let high_mask = 0xffu8 >> (8 * encoded_len - encoded_bits);
    if encoded[encoded_len - 1] != 0xbc || encoded[0] & !high_mask != 0 {
        return Err(INVALID_SIGNATURE.into());
    }
    let expected_hash = &encoded[data_len..data_len + hash_len];
    let mut data = [0u8; MAX_BYTES];
    data[..data_len].copy_from_slice(&encoded[..data_len]);
    xor_mgf1(hash, expected_hash, &mut data[..data_len])?;
    data[0] &= high_mask;
    let separator = data_len - salt_len - 1;
    if data[..separator].iter().any(|&byte| byte != 0) || data[separator] != 1 {
        return Err(INVALID_SIGNATURE.into());
    }
    let message_hash = hash.digest(message)?;
    let mut preimage = [0u8; 8 + 64 + MAX_BYTES];
    preimage[8..8 + hash_len].copy_from_slice(&message_hash[..hash_len]);
    preimage[8 + hash_len..8 + hash_len + salt_len].copy_from_slice(&data[separator + 1..data_len]);
    let actual_hash = hash.digest(&preimage[..8 + hash_len + salt_len])?;
    if !equal(expected_hash, &actual_hash[..hash_len]) {
        return Err(INVALID_SIGNATURE.into());
    }
    Ok(())
}

fn equal(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .fold(0u8, |difference, (&a, &b)| difference | (a ^ b))
            == 0
}

// The only caller uses a hash-sized seed and a <=512-byte destination, so the
// four-byte counter cannot wrap and no input-sized temporary allocation occurs.
fn xor_mgf1(hash: Hash, seed: &[u8], output: &mut [u8]) -> Result<(), String> {
    let mut input = [0u8; 68];
    input[..seed.len()].copy_from_slice(seed);
    for (counter, chunk) in output.chunks_mut(hash.length()).enumerate() {
        input[seed.len()..seed.len() + 4].copy_from_slice(&(counter as u32).to_be_bytes());
        let block = hash.digest(&input[..seed.len() + 4])?;
        for (byte, mask) in chunk.iter_mut().zip(block) {
            *byte ^= mask;
        }
    }
    Ok(())
}

struct Encoded {
    bytes: [u8; MAX_BYTES],
    length: usize,
}

fn public_operation(n: &[u8], e: &[u8], signature: &[u8]) -> Result<Encoded, String> {
    let (_, exponent) = parameters(n, e)?;
    if signature.len() != n.len() || signature >= n {
        return Err(INVALID_SIGNATURE.into());
    }
    let arithmetic = Montgomery::new(n);
    let result = arithmetic.power(&from_bytes(signature), exponent);
    let mut encoded = Encoded {
        bytes: [0; MAX_BYTES],
        length: n.len(),
    };
    for (index, byte) in encoded.bytes[..n.len()].iter_mut().rev().enumerate() {
        *byte = (result[index / 4] >> (8 * (index % 4))) as u8;
    }
    Ok(encoded)
}

fn from_bytes(bytes: &[u8]) -> Integer {
    let mut result = [0; MAX_WORDS];
    for (index, byte) in bytes.iter().rev().enumerate() {
        result[index / 4] |= u32::from(*byte) << (8 * (index % 4));
    }
    result
}

fn compare(left: &Integer, right: &Integer, words: usize) -> Ordering {
    for index in (0..words).rev() {
        match left[index].cmp(&right[index]) {
            Ordering::Equal => {}
            unequal => return unequal,
        }
    }
    Ordering::Equal
}

fn subtract(value: &mut Integer, modulus: &Integer, words: usize) {
    let mut borrow = 0u64;
    for (word, &n) in value[..words].iter_mut().zip(&modulus[..words]) {
        let subtrahend = u64::from(n) + borrow;
        let original = u64::from(*word);
        *word = original.wrapping_sub(subtrahend) as u32;
        borrow = u64::from(original < subtrahend);
    }
}

struct Montgomery {
    modulus: Integer,
    words: usize,
    negative_inverse: u32,
    r_squared: Integer,
}

impl Montgomery {
    // Requires a nonzero canonical odd modulus, at most MAX_BYTES bytes.
    fn new(n: &[u8]) -> Self {
        let modulus = from_bytes(n);
        let words = n.len().div_ceil(4);
        // Newton iteration doubles the number of correct inverse bits each
        // round, beginning with the inverse of the odd low word modulo two.
        let mut inverse = 1u32;
        for _ in 0..5 {
            inverse = inverse.wrapping_mul(2u32.wrapping_sub(modulus[0].wrapping_mul(inverse)));
        }
        let mut result = Self {
            modulus,
            words,
            negative_inverse: inverse.wrapping_neg(),
            r_squared: [0; MAX_WORDS],
        };
        result.r_squared[0] = 1;
        // R = 2^(32*words). Repeated modular doubling obtains R^2 without
        // division, with <2*n before each conditional subtraction.
        for _ in 0..64 * words {
            let mut carry = 0u64;
            for word in &mut result.r_squared[..words] {
                let doubled = (u64::from(*word) << 1) | carry;
                *word = doubled as u32;
                carry = doubled >> 32;
            }
            if carry != 0 || compare(&result.r_squared, &modulus, words) != Ordering::Less {
                subtract(&mut result.r_squared, &modulus, words);
            }
        }
        result
    }

    // Canonical a,b<n -> a*b*R^-1 mod n. Each inner sum is at most 2^64-1:
    // (2^32-1)^2 + (2^32-1) + (2^32-1). The extra word stores REDC carry.
    fn multiply(&self, left: &Integer, right: &Integer) -> Integer {
        let words = self.words;
        let mut product = [0u32; 2 * MAX_WORDS + 1];
        for (i, &a) in left[..words].iter().enumerate() {
            let mut carry = 0u64;
            for (j, &b) in right[..words].iter().enumerate() {
                let sum = u64::from(a) * u64::from(b) + u64::from(product[i + j]) + carry;
                product[i + j] = sum as u32;
                carry = sum >> 32;
            }
            product[i + words] = carry as u32;
        }
        for i in 0..words {
            let factor = product[i].wrapping_mul(self.negative_inverse);
            let mut carry = 0u64;
            for (j, &n) in self.modulus[..words].iter().enumerate() {
                let sum = u64::from(factor) * u64::from(n) + u64::from(product[i + j]) + carry;
                product[i + j] = sum as u32;
                carry = sum >> 32;
            }
            for word in &mut product[i + words..=2 * words] {
                let sum = u64::from(*word) + carry;
                *word = sum as u32;
                carry = sum >> 32;
            }
            debug_assert_eq!(carry, 0);
            debug_assert_eq!(product[i], 0);
        }
        let mut reduced = [0u32; MAX_WORDS];
        reduced[..words].copy_from_slice(&product[words..2 * words]);
        // REDC is less than 2*n because the input product is less than n*R.
        // If the extra word is set, subtraction's borrow consumes that word.
        if product[2 * words] != 0 || compare(&reduced, &self.modulus, words) != Ordering::Less {
            subtract(&mut reduced, &self.modulus, words);
        }
        debug_assert_eq!(compare(&reduced, &self.modulus, words), Ordering::Less);
        reduced
    }

    fn power(&self, base: &Integer, exponent: u32) -> Integer {
        let mut one = [0u32; MAX_WORDS];
        one[0] = 1;
        let base = self.multiply(base, &self.r_squared);
        let mut accumulator = self.multiply(&one, &self.r_squared);
        for bit in (0..32 - exponent.leading_zeros()).rev() {
            accumulator = self.multiply(&accumulator, &accumulator);
            if exponent & (1u32 << bit) != 0 {
                accumulator = self.multiply(&accumulator, &base);
            }
        }
        self.multiply(&accumulator, &one)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small(value: u64) -> Integer {
        from_bytes(&value.to_be_bytes())
    }
    fn to_small(value: &Integer) -> u64 {
        assert!(value[2..].iter().all(|&word| word == 0));
        u64::from(value[0]) | (u64::from(value[1]) << 32)
    }
    fn reference_power(base: u64, exponent: u32, modulus: u64) -> u64 {
        let mut result = 1u128;
        for bit in (0..32 - exponent.leading_zeros()).rev() {
            result = result * result % u128::from(modulus);
            if exponent & (1 << bit) != 0 {
                result = result * u128::from(base) % u128::from(modulus);
            }
        }
        result as u64
    }

    #[test]
    fn montgomery_carries_match_independent_small_integer_arithmetic() {
        for n in [
            3u64,
            17,
            0x7fff_ffff,
            0xffff_fffb,
            0x1_0000_0001,
            0x8000_0000_0000_0001,
            u64::MAX,
        ] {
            let bytes = n.to_be_bytes();
            let first = bytes.iter().position(|&byte| byte != 0).unwrap();
            let arithmetic = Montgomery::new(&bytes[first..]);
            for a in [0, 1, n / 2, n - 2, n - 1] {
                for b in [0, 1, n / 2, n - 2, n - 1] {
                    let am = arithmetic.multiply(&small(a), &arithmetic.r_squared);
                    let bm = arithmetic.multiply(&small(b), &arithmetic.r_squared);
                    let product = arithmetic.multiply(&am, &bm);
                    let normal = arithmetic.multiply(&product, &small(1));
                    assert_eq!(
                        to_small(&normal),
                        (u128::from(a) * u128::from(b) % u128::from(n)) as u64
                    );
                }
                for e in [3, 65537, u32::MAX] {
                    assert_eq!(
                        to_small(&arithmetic.power(&small(a), e)),
                        reference_power(a, e, n)
                    );
                }
            }
        }
    }

    #[test]
    fn key_policy_and_signature_representations_are_bounded() {
        let exponent = [1, 0, 1];
        for bits in [2047, 2048, 2049, 4096, 4097] {
            let mut n = vec![0u8; (bits as usize).div_ceil(8)];
            n[0] = 1 << ((bits - 1) % 8);
            *n.last_mut().unwrap() |= 1;
            assert_eq!(
                validate_key(&n, &exponent).is_ok(),
                (2048..=4096).contains(&bits)
            );
        }
        let n = [0xff; 256];
        for e in [
            &[][..],
            &[0],
            &[1],
            &[2],
            &[0, 3],
            &[1, 0, 0, 0, 1],
            &[0xff, 0xff, 0xff, 0xfe],
        ] {
            assert!(validate_key(&n, e).is_err());
        }
        for e in [&[3][..], &[1, 0, 1], &[0xff; 4]] {
            assert!(validate_key(&n, e).is_ok());
        }
        let mut padded = vec![0];
        padded.extend_from_slice(&n);
        assert!(validate_key(&padded, &exponent).is_err());
        let mut even = n;
        even[255] = 0xfe;
        assert!(validate_key(&even, &exponent).is_err());
        for signature in [&[][..], &[0; 255], &[0; 257], &n] {
            assert!(verify_pkcs1v15(&n, &exponent, Hash::Sha256, b"", signature).is_err());
            assert!(verify_pss(&n, &exponent, Hash::Sha256, 32, b"", signature).is_err());
        }
        assert!(verify_pss(&n, &exponent, Hash::Sha256, usize::MAX, b"", &[0; 256]).is_err());
    }

    fn hex(text: &str) -> Vec<u8> {
        text.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    // Published public test data: https://www.rfc-editor.org/rfc/rfc7515.html#appendix-A.2
    #[test]
    fn published_rfc7515_pkcs1_sha256() {
        let n = hex(concat!(
            "a1f8160ae2e3c9b465ce8d2d656263362b927dbe29e1f02477fc1625cc90a136e38bd93497c5b6ea63dd7711e67c7429",
            "f956b0fb8a8f089adc4b69893cc1333f53edd019b87784252fec914fe4857769594bea4280d32c0f55bf62944f130396",
            "bc6e9bdf6ebdd2bda3678eeca0c668f701b38dbffb38c8342ce2fe6d27fade4a5a4874979dd4b9cf9adec4c75b05852c",
            "2c0f5ef8a5c1750392f944e8ed64c110c6b647609aa4783aeb9c6c9ad755313050638b83665c6f6f7a82a396702a1f64",
            "1b82d3ebf2392219491fb686872c5716f50af8358d9a8b9d17c340728f7f87d89a18d8fcab67ad84590c2ecf75933936",
            "3c07034d6f606f9e21e05456cae5e9a1",
        ));
        let signature = hex(concat!(
            "702e218943e88fd11eb5d82dbf7845f34106ae1b81fff7731116add1717d83656d420afd3c96eedd73a2663e5166687b",
            "000b87226e0187ed1073f945e582adfcef16d85a798ee8c66ddb3db8975b17d09402beedd5d9d97007108db28160d5f8",
            "040ca7445762b81fbe7ff9d92e0ae76f24f25b33bbe6f44ae61eb1040acb20044d3ef9128ed40130795bd4bd3b41eeca",
            "d066ab651981fde48df77f372dc38b9fafdd3befb18b5da3cc3c2eb02f9e3a41d612caad15911273a05f23b9e838faaf",
            "849d698429ef5a1e88798236c3d40e604522a544c8f27a7a2db80663d16cf7caea56de405cb2215a45b2c25566b55ac1",
            "a748a070dfc8a32a469543d019eefb47",
        ));
        let e = [1, 0, 1];
        let message = br#"eyJhbGciOiJSUzI1NiJ9.eyJpc3MiOiJqb2UiLA0KICJleHAiOjEzMDA4MTkzODAsDQogImh0dHA6Ly9leGFtcGxlLmNvbS9pc19yb290Ijp0cnVlfQ"#;
        assert!(verify_pkcs1v15(&n, &e, Hash::Sha256, message, &signature).is_ok());
        assert!(verify_pkcs1v15(&n, &e, Hash::Sha256, b"changed", &signature).is_err());
        assert!(verify_pkcs1v15(&n, &e, Hash::Sha384, message, &signature).is_err());
    }

    // Published public test data: https://www.rfc-editor.org/rfc/rfc9421.html#appendix-B.2.1
    #[test]
    fn published_rfc9421_pss_sha512() {
        let n = hex(concat!(
            "af8b669b7af6d1677f3dbaaf3f5b36f9012dbe9b91695f18ab8d208d447ccb6463c5ae9da46d865c76cf7ef32cf1cb7e",
            "2e1d461f8e71dbc470dd1cb9de69bea005e3c90f3a3a70e467937c9586e0803e0edf0e8cea902f2e4864f79027753ae2",
            "7db2053cd53c3cf30eececab1401ea803b339e33c59933ad08470dd99d45a5681c870b982cf2fe5a892a96d775d67aaa",
            "ce2f9b27d72f48a00361d50000de5652dcdda62cba2db4e04b13fba1c894e139f483923a683649ec0f0bce8d0a4b2658",
            "a00e3ce66a9c3b419501d570f65ab868e4fdbfa77e9dbe1b9cd91056494b4377d502f266fb17433a9f4b08d08de3c576",
            "a670ce90557af94f67579a3273a5c8db",
        ));
        let signature = hex(concat!(
            "776a664ef99b9dc0f7c509bc13d655dbcdbc0634161a08b001ac396c092089b528a5e98b25c583cbf9646db1c0bdee1c",
            "440b71df522aefce94ee2b7efb0806c5bb51c5ff14771ef3159b1c9335da24c900ec2846e76792905c7290978daacad6",
            "1f94bea31345943e1dcd5bb07bc0e14d28daf31c5b47f67670e19d09bcd1ef6ae0156873c7656306a273b0f2cc2102a1",
            "3b80c67b35de8615b0139e98084f8ee9cd26299b1fc55ae8814bc0e0710b8d5296980bed97a5270a1f23633b951b9592",
            "6ff40454f9cfe5399c0272c7d60facfbebfa778b3c9b4802c357d5e7f4884cbabd9a1868f0adfeec43d84d4f085356cb",
            "85dc4ee4dcadf02f2cb229d0f7c5f0f5",
        ));
        let e = [1, 0, 1];
        let message = br#""@signature-params": ();created=1618884473;keyid="test-key-rsa-pss";nonce="b3k2pp5k7z-50gnwp.yemd""#;
        assert!(verify_pss(&n, &e, Hash::Sha512, 64, message, &signature).is_ok());
        assert!(verify_pss(&n, &e, Hash::Sha512, 63, message, &signature).is_err());
        assert!(verify_pss(&n, &e, Hash::Sha512, 64, b"changed", &signature).is_err());
    }
}
