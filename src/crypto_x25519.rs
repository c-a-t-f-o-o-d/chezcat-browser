//! Original X25519 key agreement from RFC 7748 sections 5 and 6.1.
//!
//! Field arithmetic uses five 51-bit limbs and u128 multiplication, with
//! fixed-count carry passes and masked conditional swaps. The scalar ladder
//! always takes 255 steps; inversion uses a fixed public exponent. These source
//! properties are not a guarantee of compiler- or hardware-level constant time.
//! This experimental module supplies no randomness, peer authentication, TLS,
//! or guaranteed secret erasure. Callers must obtain fresh secret bytes from a
//! cryptographically secure random source before any future protocol use.

const LIMB_BITS: usize = 51;
const RADIX: u64 = 1u64 << LIMB_BITS;
const MASK: u64 = RADIX - 1;
const PRIME_LIMBS: [u64; 5] = [RADIX - 19, MASK, MASK, MASK, MASK];

// Every constructed Field has canonical limbs and denotes an integer below
// p = 2^255 - 19. This keeps all multiplication coefficients below 2^109.
#[derive(Clone, Copy)]
struct Field([u64; 5]);

impl Field {
    const ZERO: Self = Self([0; 5]);
    const ONE: Self = Self([1, 0, 0, 0, 0]);

    fn decode(bytes: &[u8; 32]) -> Self {
        let mut digits = [0u128; 5];
        // Deliberately consume only 255 bits: RFC 7748 ignores input bit 255.
        // A simple fixed loop also avoids overlapping/end-of-buffer loads.
        for bit in 0..255 {
            let value = (bytes[bit / 8] >> (bit % 8)) & 1;
            digits[bit / LIMB_BITS] |= u128::from(value) << (bit % LIMB_BITS);
        }
        // The 19 noncanonical input values p..2^255-1 are reduced, not rejected.
        Self::reduce(digits)
    }

    fn encode(self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for bit in 0..255 {
            let value = (self.0[bit / LIMB_BITS] >> (bit % LIMB_BITS)) & 1;
            bytes[bit / 8] |= (value as u8) << (bit % 8);
        }
        bytes
    }

    fn reduce(mut digits: [u128; 5]) -> Self {
        // 2^255 == 19 (mod p). Multiplication starts with coefficients at
        // most 77*(2^51-1)^2. After the first pass only digit 0 can exceed a
        // limb, by less than 97 radix units; after the second it is < radix+19.
        // A third fixed pass settles the remaining possible carry chain.
        for _ in 0..3 {
            for index in 0..4 {
                let carry = digits[index] >> LIMB_BITS;
                digits[index] &= u128::from(MASK);
                digits[index + 1] += carry;
            }
            let carry = digits[4] >> LIMB_BITS;
            digits[4] &= u128::from(MASK);
            digits[0] += 19 * carry;
        }
        let limbs = digits.map(|digit| digit as u64);
        // x is now below 2^255. Adding 19 overflows that range exactly when
        // x >= p; the low 255 bits then equal x-p. Select without a secret
        // comparison branch or a data-dependent carry loop.
        let mut candidate = [0u64; 5];
        let mut carry = 19u64;
        for index in 0..5 {
            let sum = limbs[index] + carry;
            candidate[index] = sum & MASK;
            carry = sum >> LIMB_BITS;
        }
        let choose_reduced = 0u64.wrapping_sub(carry);
        Self(std::array::from_fn(|index| {
            (candidate[index] & choose_reduced) | (limbs[index] & !choose_reduced)
        }))
    }

    fn add(self, rhs: Self) -> Self {
        Self::reduce(std::array::from_fn(|index| {
            u128::from(self.0[index]) + u128::from(rhs.0[index])
        }))
    }

    fn sub(self, rhs: Self) -> Self {
        // Adding 2p leaves every limb positive before subtraction, including
        // the slightly smaller low limb, without altering the field value.
        Self::reduce(std::array::from_fn(|index| {
            u128::from(self.0[index]) + 2 * u128::from(PRIME_LIMBS[index])
                - u128::from(rhs.0[index])
        }))
    }

    fn mul(self, rhs: Self) -> Self {
        // Polynomial multiplication in radix 2^51. Fold degrees 5..8 back
        // into degrees 0..3 using radix^5 == 19 (mod p).
        let mut polynomial = [0u128; 9];
        for (left_index, &left) in self.0.iter().enumerate() {
            for (right_index, &right) in rhs.0.iter().enumerate() {
                polynomial[left_index + right_index] += u128::from(left) * u128::from(right);
            }
        }
        let mut digits = std::array::from_fn(|index| polynomial[index]);
        for index in 0..4 {
            digits[index] += 19 * polynomial[index + 5];
        }
        Self::reduce(digits)
    }

    fn square(self) -> Self {
        self.mul(self)
    }

    fn times_a24(self) -> Self {
        // (486662 - 2) / 4 from the RFC 7748 Montgomery formulas.
        Self::reduce(self.0.map(|digit| u128::from(digit) * 121_665))
    }

    fn inverse(self) -> Self {
        // Fermat inversion: p-2 = 2^255-21. Among its 255 bits, only bits
        // 2 and 4 are zero. The branching below depends solely on this fixed
        // public exponent; its operation schedule is identical for all input.
        // Zero maps to zero and is rejected by shared_secret after the ladder.
        let mut result = Self::ONE;
        for bit in (0..255).rev() {
            result = result.square();
            if bit != 2 && bit != 4 {
                result = result.mul(self);
            }
        }
        result
    }

    fn swap(left: &mut Self, right: &mut Self, bit: u64) {
        let mask = 0u64.wrapping_sub(bit);
        for index in 0..5 {
            let changed = (left.0[index] ^ right.0[index]) & mask;
            left.0[index] ^= changed;
            right.0[index] ^= changed;
        }
    }
}

fn ladder(secret: &[u8; 32], coordinate: &[u8; 32]) -> [u8; 32] {
    let mut scalar = *secret;
    scalar[0] &= 0xf8;
    scalar[31] = (scalar[31] & 0x7f) | 0x40;
    let base = Field::decode(coordinate);
    let mut low_x = Field::ONE;
    let mut low_z = Field::ZERO;
    let mut high_x = base;
    let mut high_z = Field::ONE;
    let mut previous = 0u64;

    for position in (0..255).rev() {
        let bit = u64::from((scalar[position / 8] >> (position % 8)) & 1);
        Field::swap(&mut low_x, &mut high_x, bit ^ previous);
        Field::swap(&mut low_z, &mut high_z, bit ^ previous);
        previous = bit;

        let low_sum = low_x.add(low_z);
        let low_difference = low_x.sub(low_z);
        let sum_squared = low_sum.square();
        let difference_squared = low_difference.square();
        let separation = sum_squared.sub(difference_squared);
        let cross_left = high_x.sub(high_z).mul(low_sum);
        let cross_right = high_x.add(high_z).mul(low_difference);

        high_x = cross_left.add(cross_right).square();
        high_z = base.mul(cross_left.sub(cross_right).square());
        low_x = sum_squared.mul(difference_squared);
        low_z = separation.mul(sum_squared.add(separation.times_a24()));
    }
    Field::swap(&mut low_x, &mut high_x, previous);
    Field::swap(&mut low_z, &mut high_z, previous);
    low_x.mul(low_z.inverse()).encode()
}

/// Compute the X25519 public key for caller-supplied 32-byte secret material.
/// RFC 7748 scalar clamping is applied internally; this function generates no RNG.
pub fn public_key(secret: &[u8; 32]) -> [u8; 32] {
    let mut base = [0u8; 32];
    base[0] = 9;
    ladder(secret, &base)
}

/// Compute X25519, rejecting an all-zero result from a low-order peer input.
/// A successful result is key-agreement material, not proof of peer identity.
pub fn shared_secret(secret: &[u8; 32], peer: &[u8; 32]) -> Result<[u8; 32], String> {
    let shared = ladder(secret, peer);
    let nonzero = shared.iter().fold(0u8, |combined, &byte| combined | byte);
    if nonzero == 0 {
        return Err("X25519 peer produced an all-zero shared secret.".into());
    }
    Ok(shared)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(input: &str) -> [u8; 32] {
        assert_eq!(input.len(), 64);
        let mut bytes = [0; 32];
        for (byte, pair) in bytes.iter_mut().zip(input.as_bytes().as_chunks::<2>().0) {
            *byte = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
        }
        bytes
    }

    #[test]
    fn rfc7748_scalar_vectors() {
        for (secret, peer, expected) in [
            (
                "a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4",
                "e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c",
                "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552",
            ),
            (
                "4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d",
                "e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493",
                "95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957",
            ),
        ] {
            assert_eq!(
                shared_secret(&hex(secret), &hex(peer)).unwrap(),
                hex(expected)
            );
        }
    }

    #[test]
    fn rfc7748_alice_and_bob_agree() {
        let alice = hex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        let bob = hex("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb");
        let alice_public = hex("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a");
        let bob_public = hex("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f");
        let expected = hex("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");
        assert_eq!(public_key(&alice), alice_public);
        assert_eq!(public_key(&bob), bob_public);
        assert_eq!(shared_secret(&alice, &bob_public).unwrap(), expected);
        assert_eq!(shared_secret(&bob, &alice_public).unwrap(), expected);
    }

    #[test]
    fn rfc7748_iteration_vectors() {
        let mut scalar = [0u8; 32];
        scalar[0] = 9;
        let mut coordinate = scalar;
        for iteration in 1..=1000 {
            let old_scalar = scalar;
            scalar = ladder(&scalar, &coordinate);
            coordinate = old_scalar;
            if iteration == 1 {
                assert_eq!(
                    scalar,
                    hex("422c8e7a6227d7bca1350b3e2bb7279f7897b87bb6854b783c60e80311ae3079")
                );
            }
        }
        assert_eq!(
            scalar,
            hex("684cf59ba83309552800ef566f2f4d3c1c3887c49360e3875f2eb94d99532c51")
        );
    }

    #[test]
    fn peer_encoding_clamping_and_zero_rejection() {
        let secret = [0x66; 32];
        let expected = public_key(&secret);
        let mut base = [0u8; 32];
        base[0] = 9;
        let mut top_bit = base;
        top_bit[31] = 0x80;
        assert_eq!(shared_secret(&secret, &top_bit).unwrap(), expected);
        let mut noncanonical = [0xff; 32];
        noncanonical[0] = 0xf6;
        noncanonical[31] = 0x7f;
        assert_eq!(shared_secret(&secret, &noncanonical).unwrap(), expected); // p+9
        noncanonical[31] |= 0x80;
        assert_eq!(shared_secret(&secret, &noncanonical).unwrap(), expected);
        let mut same_scalar = secret;
        same_scalar[0] ^= 7;
        same_scalar[31] ^= 0xc0;
        assert_eq!(public_key(&same_scalar), expected);
        let mut one = [0u8; 32];
        one[0] = 1;
        assert!(shared_secret(&secret, &[0; 32]).is_err());
        assert!(shared_secret(&secret, &one).is_err());
        for low in [0xec, 0xed, 0xee] {
            // p-1, p, p+1
            let mut peer = [0xff; 32];
            peer[0] = low;
            peer[31] = 0x7f;
            assert!(shared_secret(&secret, &peer).is_err());
            peer[31] |= 0x80;
            assert!(shared_secret(&secret, &peer).is_err());
        }
    }

    #[test]
    fn field_carries_and_canonical_reduction() {
        let mut p_minus_one = [0xff; 32];
        p_minus_one[0] = 0xec;
        p_minus_one[31] = 0x7f;
        let negative_one = Field::decode(&p_minus_one);
        assert_eq!(negative_one.square().encode(), Field::ONE.encode());
        assert_eq!(negative_one.add(Field::ONE).encode(), [0; 32]);
        assert_eq!(Field::ZERO.sub(Field::ONE).encode(), p_minus_one);
        assert_eq!(negative_one.inverse().encode(), p_minus_one);
        let maximum = Field::decode(&[0xff; 32]); // (2^255-1) mod p = 18
        let mut eighteen = [0u8; 32];
        eighteen[0] = 18;
        assert_eq!(maximum.encode(), eighteen);
        for number in [0, 1, 2, 9, 18, 121_665, MASK] {
            let value = Field([number, 0, 0, 0, 0]);
            assert_eq!(value.sub(value).encode(), [0; 32]);
            assert_eq!(
                value.add(negative_one).add(Field::ONE).encode(),
                value.encode()
            );
            if number != 0 {
                assert_eq!(value.mul(value.inverse()).encode(), Field::ONE.encode());
            }
        }
    }
}
