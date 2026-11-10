//! Original P-256 ephemeral Diffie-Hellman arithmetic, separate from the
//! variable-time ECDSA public-verification code.
//!
//! Domain parameters: NIST SP 800-186, section 3.2.1.3.
//! Group law and ECDH: Standards for Efficient Cryptography 1 (SEC 1),
//! version 2.0, sections 2.2.1, 3.2, and 3.3.1.
//! Peer validation: NIST SP 800-56A revision 3, section 5.6.2.3.4.
//! <https://doi.org/10.6028/NIST.SP.800-186>
//! <https://www.secg.org/sec1-v2.pdf>
//! <https://doi.org/10.6028/NIST.SP.800-56Ar3>
//!
//! Accepted private scalars run the same 256 double-and-add-always rounds.
//! Field operations use fixed limb/carry loops and masked corrections; point
//! exceptions are calculated and selected with masks. There are no secret
//! lookup tables or secret-dependent arithmetic loop bounds. This source-level
//! discipline is not a claim of compiler/hardware constant-time execution or
//! independent side-channel review. Stack/register copies are not guaranteed
//! to be erased. These routines are for fresh, single-handshake TLS keys.

// Four little-endian limbs; external scalars/coordinates are big-endian.
type Words = [u64; 4];

const P: Words = [
    0xffffffffffffffff,
    0x00000000ffffffff,
    0x0000000000000000,
    0xffffffff00000001,
];
const N: Words = [
    0xf3b9cac2fc632551,
    0xbce6faada7179e84,
    0xffffffffffffffff,
    0xffffffff00000000,
];
const B: Words = [
    0x3bce3c3e27d2604b,
    0x651d06b0cc53b0f6,
    0xb3ebbd55769886bc,
    0x5ac635d8aa3a93e7,
];
const GX: Words = [
    0xf4a13945d898c296,
    0x77037d812deb33a0,
    0xf8bce6e563a440f2,
    0x6b17d1f2e12c4247,
];
const GY: Words = [
    0xcbb6406837bf51f5,
    0x2bce33576b315ece,
    0x8ee7eb4a7c0f9e16,
    0x4fe342e2fe1a7f9b,
];

// Montgomery radix R=2^256. These constants are R mod p and R^2 mod p.
const R: Words = [
    0x0000000000000001,
    0xffffffff00000000,
    0xffffffffffffffff,
    0x00000000fffffffe,
];
const R2: Words = [
    0x0000000000000003,
    0xfffffffbffffffff,
    0xfffffffffffffffe,
    0x00000004fffffffd,
];
const P_MINUS_TWO: Words = [
    0xfffffffffffffffd,
    0x00000000ffffffff,
    0x0000000000000000,
    0xffffffff00000001,
];

/// Produce an uncompressed SEC1 P-256 public key. The input is a big-endian
/// scalar, strictly 1 <= d < n. The caller supplies independent uniform random
/// bytes and retries rejected candidates; no clamping or modulo bias occurs.
pub(super) fn public_key(secret: &[u8; 32]) -> Result<[u8; 65], String> {
    let scalar = scalar(secret)?;
    let base = Affine {
        x: Field::encode(GX),
        y: Field::encode(GY),
    };
    let point = multiply(&scalar, base);
    let (x, y) = point.affine()?;
    let mut encoded = [0u8; 65];
    encoded[0] = 4;
    encoded[1..33].copy_from_slice(&to_be(x.decode()));
    encoded[33..].copy_from_slice(&to_be(y.decode()));
    Ok(encoded)
}

/// Validate the finite uncompressed peer point, multiply it by a canonical
/// private scalar, and return the affine x coordinate padded to 32 bytes.
/// P-256 has prime order and cofactor one: every accepted finite curve point
/// belongs to the required subgroup. This is the ECDH field element, not a
/// directly usable encryption key; TLS supplies the subsequent key derivation.
pub(super) fn shared_secret(secret: &[u8; 32], peer: &[u8]) -> Result<[u8; 32], String> {
    let scalar = scalar(secret)?;
    let public = peer_point(peer)?;
    let point = multiply(&scalar, public);
    // This guard is unreachable for a valid scalar and validated P-256 point,
    // but keeps any invalid arithmetic result from becoming a TLS secret.
    if point.z.zero_mask() != 0 {
        return Err("P-256 ECDH produced the point at infinity.".into());
    }
    let reciprocal = point.z.inverse();
    let x = point.x.mul(reciprocal.square());
    Ok(to_be(x.decode()))
}

fn scalar(bytes: &[u8; 32]) -> Result<Words, String> {
    let value = from_be(bytes);
    let (_, below_n) = subtract_words(value, N);
    let valid = below_n & (!zero_mask(value) & 1);
    // Only the invalid/valid distinction is exposed. All valid scalars take
    // the same arithmetic schedule, including scalars with leading zeroes.
    if valid == 0 {
        return Err("A P-256 private scalar must be in the range 1..n-1.".into());
    }
    Ok(value)
}

fn peer_point(bytes: &[u8]) -> Result<Affine, String> {
    if bytes.len() != 65 || bytes[0] != 4 {
        return Err("P-256 ECDH requires a 65-byte uncompressed SEC1 peer point.".into());
    }
    let mut x_bytes = [0u8; 32];
    let mut y_bytes = [0u8; 32];
    x_bytes.copy_from_slice(&bytes[1..33]);
    y_bytes.copy_from_slice(&bytes[33..]);
    let x = from_be(&x_bytes);
    let y = from_be(&y_bytes);
    let (_, x_below_p) = subtract_words(x, P);
    let (_, y_below_p) = subtract_words(y, P);
    if (x_below_p & y_below_p) == 0 {
        return Err("P-256 peer coordinates must be below the field modulus.".into());
    }
    let x = Field::encode(x);
    let y = Field::encode(y);
    let rhs = x.square().mul(x).sub(x.add(x).add(x)).add(Field::encode(B));
    if y.square().sub(rhs).zero_mask() == 0 {
        return Err("The P-256 peer point is not on the curve.".into());
    }
    Ok(Affine { x, y })
}

#[derive(Clone, Copy)]
struct Field(Words);

impl Field {
    const ZERO: Self = Self([0; 4]);
    const ONE: Self = Self(R);

    fn encode(canonical: Words) -> Self {
        Self(montgomery(canonical, R2))
    }

    fn decode(self) -> Words {
        montgomery(self.0, [1, 0, 0, 0])
    }

    fn add(self, other: Self) -> Self {
        let mut sum = [0; 4];
        let mut carry = 0u128;
        for (i, slot) in sum.iter_mut().enumerate() {
            let value = (self.0[i] as u128)
                .wrapping_add(other.0[i] as u128)
                .wrapping_add(carry);
            *slot = value as u64;
            carry = value >> 64;
        }
        Self(reduce_once(sum, carry as u64))
    }

    fn sub(self, other: Self) -> Self {
        let (mut difference, borrow) = subtract_words(self.0, other.0);
        let mask = 0u64.wrapping_sub(borrow);
        let mut carry = 0u128;
        for i in 0..4 {
            let value = (difference[i] as u128)
                .wrapping_add((P[i] & mask) as u128)
                .wrapping_add(carry);
            difference[i] = value as u64;
            carry = value >> 64;
        }
        Self(difference)
    }

    fn mul(self, other: Self) -> Self {
        Self(montgomery(self.0, other.0))
    }

    fn square(self) -> Self {
        self.mul(self)
    }

    fn inverse(self) -> Self {
        // Fermat inversion with the public exponent p-2. Both candidates are
        // calculated in every iteration; zero maps to zero, never a panic.
        let mut value = Self::ONE;
        for bit in (0..256).rev() {
            let squared = value.square();
            let multiplied = squared.mul(self);
            let mask = 0u64.wrapping_sub((P_MINUS_TWO[bit / 64] >> (bit % 64)) & 1);
            value = squared.select(multiplied, mask);
        }
        value
    }

    fn zero_mask(self) -> u64 {
        zero_mask(self.0)
    }

    fn select(self, other: Self, mask: u64) -> Self {
        Self(select_words(self.0, other.0, mask))
    }
}

#[derive(Clone, Copy)]
struct Affine {
    x: Field,
    y: Field,
}

#[derive(Clone, Copy)]
struct Jacobian {
    // x_affine=X/Z^2, y_affine=Y/Z^3; Z=0 represents infinity.
    x: Field,
    y: Field,
    z: Field,
}

impl Jacobian {
    const INFINITY: Self = Self {
        x: Field::ZERO,
        y: Field::ZERO,
        z: Field::ZERO,
    };

    fn from_affine(point: Affine) -> Self {
        Self {
            x: point.x,
            y: point.y,
            z: Field::ONE,
        }
    }

    fn select(self, other: Self, mask: u64) -> Self {
        Self {
            x: self.x.select(other.x, mask),
            y: self.y.select(other.y, mask),
            z: self.z.select(other.z, mask),
        }
    }

    fn double(self) -> Self {
        // The affine tangent (3*x^2-3)/(2*y), with denominators carried in Z.
        let z2 = self.z.square();
        let y2 = self.y.square();
        let xy2 = self.x.mul(y2);
        let four_xy2 = xy2.add(xy2).add(xy2.add(xy2));
        let slope = self.x.sub(z2).mul(self.x.add(z2));
        let slope = slope.add(slope).add(slope);
        let x = slope.square().sub(four_xy2.add(four_xy2));
        let y4 = y2.square();
        let twice_y4 = y4.add(y4);
        let four_y4 = twice_y4.add(twice_y4);
        let y = slope.mul(four_xy2.sub(x)).sub(four_y4.add(four_y4));
        let yz = self.y.mul(self.z);
        let result = Self {
            x,
            y,
            z: yz.add(yz),
        };
        // Infinity and vertical tangents are selected without branching.
        result.select(Self::INFINITY, self.z.zero_mask() | self.y.zero_mask())
    }

    fn add_affine(self, other: Affine) -> Self {
        // Align the other point to our denominator, then apply the chord law.
        let z2 = self.z.square();
        let h = other.x.mul(z2).sub(self.x);
        let r = other.y.mul(z2.mul(self.z)).sub(self.y);
        let h2 = h.square();
        let h3 = h2.mul(h);
        let xh2 = self.x.mul(h2);
        let x = r.square().sub(h3).sub(xh2.add(xh2));
        let y = r.mul(xh2.sub(x)).sub(self.y.mul(h3));
        let ordinary = Self {
            x,
            y,
            z: self.z.mul(h),
        };
        let same_x = h.zero_mask();
        let same_y = r.zero_mask();
        // All candidates are calculated regardless of the intermediate point:
        // equal points double, opposites give infinity, infinity plus Q is Q.
        let doubled = self.double();
        ordinary
            .select(Self::INFINITY, same_x & !same_y)
            .select(doubled, same_x & same_y)
            .select(Self::from_affine(other), self.z.zero_mask())
    }

    fn affine(self) -> Result<(Field, Field), String> {
        // Accepted scalars and curve points cannot produce infinity. Retain a
        // failure guard rather than serializing an invalid arithmetic result.
        if self.z.zero_mask() != 0 {
            return Err("P-256 key generation produced the point at infinity.".into());
        }
        let reciprocal = self.z.inverse();
        let reciprocal2 = reciprocal.square();
        Ok((
            self.x.mul(reciprocal2),
            self.y.mul(reciprocal2.mul(reciprocal)),
        ))
    }
}

fn multiply(scalar: &Words, base: Affine) -> Jacobian {
    let mut result = Jacobian::INFINITY;
    for bit in (0..256).rev() {
        let doubled = result.double();
        let added = doubled.add_affine(base);
        let mask = 0u64.wrapping_sub((scalar[bit / 64] >> (bit % 64)) & 1);
        result = doubled.select(added, mask);
    }
    result
}

fn from_be(bytes: &[u8; 32]) -> Words {
    let mut words = [0; 4];
    for (i, word) in words.iter_mut().enumerate() {
        let mut part = [0; 8];
        part.copy_from_slice(&bytes[24 - 8 * i..32 - 8 * i]);
        *word = u64::from_be_bytes(part);
    }
    words
}

fn to_be(words: Words) -> [u8; 32] {
    let mut bytes = [0; 32];
    for (i, word) in words.iter().enumerate() {
        bytes[24 - 8 * i..32 - 8 * i].copy_from_slice(&word.to_be_bytes());
    }
    bytes
}

fn zero_mask(words: Words) -> u64 {
    let combined = words[0] | words[1] | words[2] | words[3];
    let nonzero_bit = (combined | combined.wrapping_neg()) >> 63;
    0u64.wrapping_sub(nonzero_bit ^ 1)
}

fn select_words(first: Words, second: Words, mask: u64) -> Words {
    let mut result = [0; 4];
    for i in 0..4 {
        result[i] = (first[i] & !mask) | (second[i] & mask);
    }
    result
}

fn subtract_word(a: u64, b: u64, borrow: u64) -> (u64, u64) {
    // The mathematical difference is in [-2^64, 2^64-1], so bit 127 of its
    // wrapping u128 representation exactly identifies a negative difference.
    let value = (a as u128).wrapping_sub((b as u128).wrapping_add(borrow as u128));
    (value as u64, (value >> 127) as u64)
}

fn subtract_words(a: Words, b: Words) -> (Words, u64) {
    let mut difference = [0; 4];
    let mut borrow = 0;
    for i in 0..4 {
        (difference[i], borrow) = subtract_word(a[i], b[i], borrow);
    }
    (difference, borrow)
}

fn reduce_once(low: Words, high: u64) -> Words {
    // Input is strictly below 2p, including its possible 257th bit. Treating
    // that bit as a fifth limb avoids losing the carry near the modulus.
    let (difference, borrow) = subtract_words(low, P);
    let (_, underflow) = subtract_word(high, 0, borrow);
    let use_difference = 0u64.wrapping_sub(underflow ^ 1);
    select_words(low, difference, use_difference)
}

fn montgomery(a: Words, b: Words) -> Words {
    // A ninth limb holds the final carry during reduction. Every carry loop
    // walks to its fixed public endpoint, even after the carry becomes zero.
    let mut product = [0u64; 9];
    for i in 0..4 {
        let mut carry = 0u128;
        for (j, word) in b.iter().enumerate() {
            let value = (a[i] as u128)
                .wrapping_mul(*word as u128)
                .wrapping_add(product[i + j] as u128)
                .wrapping_add(carry);
            product[i + j] = value as u64;
            carry = value >> 64;
        }
        for slot in product.iter_mut().skip(i + 4) {
            let value = (*slot as u128).wrapping_add(carry);
            *slot = value as u64;
            carry = value >> 64;
        }
    }
    // p[0]=-1 (mod 2^64), so -p[0]^-1=1. Adding product[i]*p cancels
    // the low limb. After four cancellations the result is a*b/R, below 2p.
    for i in 0..4 {
        let factor = product[i];
        let mut carry = 0u128;
        for j in 0..4 {
            let value = (factor as u128)
                .wrapping_mul(P[j] as u128)
                .wrapping_add(product[i + j] as u128)
                .wrapping_add(carry);
            product[i + j] = value as u64;
            carry = value >> 64;
        }
        for slot in product.iter_mut().skip(i + 4) {
            let value = (*slot as u128).wrapping_add(carry);
            *slot = value as u64;
            carry = value >> 64;
        }
    }
    reduce_once([product[4], product[5], product[6], product[7]], product[8])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generator() -> Affine {
        Affine {
            x: Field::encode(GX),
            y: Field::encode(GY),
        }
    }

    #[test]
    fn scalar_boundaries_and_generator() {
        let one = to_be([1, 0, 0, 0]);
        let public = public_key(&one).unwrap();
        assert_eq!(&public[1..33], &to_be(GX));
        assert_eq!(&public[33..], &to_be(GY));
        let last = to_be(subtract_words(N, [1, 0, 0, 0]).0);
        let negative = public_key(&last).unwrap();
        assert_eq!(&negative[1..33], &to_be(GX));
        assert_eq!(&negative[33..], &to_be(subtract_words(P, GY).0));
        for invalid in [[0u8; 32], to_be(N), [255u8; 32]] {
            assert!(public_key(&invalid).is_err());
            assert!(shared_secret(&invalid, &public).is_err());
        }
        assert_eq!(shared_secret(&one, &public).unwrap(), to_be(GX));
        assert_eq!(shared_secret(&last, &public).unwrap(), to_be(GX));
    }

    #[test]
    fn field_carry_borrow_and_inverse_boundaries() {
        let minus_one = Field::encode(subtract_words(P, [1, 0, 0, 0]).0);
        assert_eq!(Field::ONE.decode(), [1, 0, 0, 0]);
        assert_eq!(minus_one.add(Field::ONE).decode(), [0; 4]);
        assert_eq!(minus_one.square().decode(), [1, 0, 0, 0]);
        assert_eq!(Field::ZERO.sub(Field::ONE).decode(), minus_one.decode());
        assert_eq!(minus_one.add(minus_one).decode(), P_MINUS_TWO);
        assert_eq!(minus_one.inverse().decode(), minus_one.decode());
        assert_eq!(Field::ZERO.inverse().decode(), [0; 4]);
        for value in [1, 2, 3, 17, u64::MAX] {
            let element = Field::encode([value, 0, 0, 0]);
            assert_eq!(element.mul(element.inverse()).decode(), [1, 0, 0, 0]);
        }
    }

    #[test]
    fn point_exception_selection() {
        let g = generator();
        let inf = Jacobian::INFINITY;
        assert_ne!(inf.double().z.zero_mask(), 0);
        let from_inf = inf.add_affine(g).affine().unwrap();
        assert_eq!(from_inf.0.decode(), GX);
        assert_eq!(from_inf.1.decode(), GY);
        let point = Jacobian::from_affine(g);
        let doubled = point.double().affine().unwrap();
        let added = point.add_affine(g).affine().unwrap();
        assert_eq!(doubled.0.decode(), added.0.decode());
        assert_eq!(doubled.1.decode(), added.1.decode());
        let opposite = Affine {
            x: g.x,
            y: Field::ZERO.sub(g.y),
        };
        assert_ne!(point.add_affine(opposite).z.zero_mask(), 0);
    }

    #[test]
    fn rejects_malformed_peer_points() {
        let secret = to_be([2, 0, 0, 0]);
        let valid = public_key(&secret).unwrap();
        for length in [0, 1, 32, 33, 64] {
            assert!(shared_secret(&secret, &valid[..length]).is_err());
        }
        for prefix in [0, 2, 3, 5, 6, 7, 255] {
            let mut bad = valid;
            bad[0] = prefix;
            assert!(shared_secret(&secret, &bad).is_err());
        }
        for coordinate in [1..33, 33..65] {
            let mut bad = valid;
            bad[coordinate].copy_from_slice(&to_be(P));
            assert!(shared_secret(&secret, &bad).is_err());
        }
        let mut off_curve = valid;
        off_curve[64] ^= 1;
        assert!(shared_secret(&secret, &off_curve).is_err());
        assert!(shared_secret(&secret, &[0; 65]).is_err());
    }
}
