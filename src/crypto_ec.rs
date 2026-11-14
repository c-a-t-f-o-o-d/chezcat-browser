//! Original, bounded ECDSA public verification for NIST P-256 and P-384.
//!
//! Domain constants: NIST SP 800-186, sections 3.2.1.3 and 3.2.1.4.
//! Verification: FIPS 186-5, section 6.4.2. Point encoding: RFC 5480.
//! <https://doi.org/10.6028/NIST.SP.800-186>
//! <https://doi.org/10.6028/NIST.FIPS.186-5>
//! <https://www.rfc-editor.org/rfc/rfc5480.html>
//!
//! Only public data is processed. The variable-time arithmetic must not be
//! reused for private scalars, signing, key generation, or ECDH. There are no
//! arbitrary curve parameters, compressed points, or external crypto services.

use std::cmp::Ordering;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    P256,
    P384,
}

// Little-endian limbs. The upper two limbs stay zero for P-256. Every field
// operation returns the unique representative below its modulus.
type Number = [u64; 6];
const ZERO: Number = [0; 6];
const ONE: Number = [1, 0, 0, 0, 0, 0];
const TWO: Number = [2, 0, 0, 0, 0, 0];
const INVALID: &str = "ECDSA signature verification failed.";

struct Modular {
    modulus: Number,
    words: usize,
    negative_inverse: u64,
    r_squared: Number,
    one: Number,
}

#[derive(Clone, Copy)]
struct Affine {
    x: Number,
    y: Number,
}

#[derive(Clone, Copy)]
struct Point {
    // Jacobian coordinates: affine x=X/Z^2, y=Y/Z^3. Z=0 is infinity.
    x: Number,
    y: Number,
    z: Number,
}

impl Point {
    const INFINITY: Self = Self {
        x: ZERO,
        y: ZERO,
        z: ZERO,
    };
}

struct Parameters {
    field: Modular,
    order: Modular,
    b: Number,
    generator: Affine,
}

/// Accept only a canonical, finite, uncompressed SEC1 point on the named curve.
/// Both supported curves have prime group order and cofactor one, so a finite
/// point on the curve is automatically in the required prime-order subgroup.
pub fn validate_key(curve: Curve, uncompressed: &[u8]) -> Result<(), String> {
    parameters(curve).point(uncompressed).map(|_| ())
}

/// Verify a DER SEQUENCE of positive INTEGER r,s against an already computed
/// SHA-256/384/512 digest. The caller chooses the certificate/TLS hash algorithm.
/// Digest truncation keeps the leftmost min(order_bits, digest_bits) bits.
/// High-S signatures are valid ECDSA and are accepted.
pub fn verify(
    curve: Curve,
    uncompressed: &[u8],
    digest: &[u8],
    signature_der: &[u8],
) -> Result<(), String> {
    if !matches!(digest.len(), 32 | 48 | 64) {
        return Err("ECDSA requires a SHA-256, SHA-384, or SHA-512 digest.".into());
    }
    let parameters = parameters(curve);
    let public = parameters.point(uncompressed)?;
    let order = &parameters.order;
    let (r, s) = signature(signature_der, order)?;
    // Both orders have exactly 256/384 bits, so truncation ends on a byte.
    let mut e = from_be(&digest[..digest.len().min(order.words * 8)]);
    if compare(&e, &order.modulus, order.words) != Ordering::Less {
        e = difference(&e, &order.modulus, order.words).0;
    }
    let inverse = order.inverse(&order.encode(&s));
    let u = order.decode(&order.multiply(&order.encode(&e), &inverse));
    let v = order.decode(&order.multiply(&order.encode(&r), &inverse));
    let result = parameters.combination(&u, &v, &public);
    if result.z == ZERO {
        return Err(INVALID.into());
    }
    // x mod n == r iff x is r or r+n: n < p < 2n on both fixed curves.
    // Compare in Jacobian form to avoid an unnecessary field inversion.
    let field = &parameters.field;
    let z_squared = field.square(&result.z);
    if result.x == field.multiply(&field.encode(&r), &z_squared) {
        return Ok(());
    }
    let (second, carry) = sum(&r, &order.modulus, order.words);
    if carry == 0
        && compare(&second, &field.modulus, field.words) == Ordering::Less
        && result.x == field.multiply(&field.encode(&second), &z_squared)
    {
        return Ok(());
    }
    Err(INVALID.into())
}

impl Parameters {
    fn new(curve: Curve) -> Self {
        // These are mathematical domain parameters, not implementation data
        // from another library. a=-3 and cofactor=1 on both curves.
        let (words, p, n, b, gx, gy) = match curve {
            Curve::P256 => (
                4,
                hex_number("FFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF"),
                hex_number("FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551"),
                hex_number("5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B"),
                hex_number("6B17D1F2E12C4247F8BCE6E563A440F277037D812DEB33A0F4A13945D898C296"),
                hex_number("4FE342E2FE1A7F9B8EE7EB4A7C0F9E162BCE33576B315ECECBB6406837BF51F5"),
            ),
            Curve::P384 => (
                6,
                hex_number("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFFFF0000000000000000FFFFFFFF"),
                hex_number("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC7634D81F4372DDF581A0DB248B0A77AECEC196ACCC52973"),
                hex_number("B3312FA7E23EE7E4988E056BE3F82D19181D9C6EFE8141120314088F5013875AC656398D8A2ED19D2A85C8EDD3EC2AEF"),
                hex_number("AA87CA22BE8B05378EB1C71EF320AD746E1D3B628BA79B9859F741E082542A385502F25DBF55296C3A545E3872760AB7"),
                hex_number("3617DE4A96262C6F5D9E98BF9292DC29F8F41DBD289A147CE9DA3113B5F0B8C00A60B1CE1D7E819D7A431D7C90EA0E5F"),
            ),
        };
        let field = Modular::new(p, words);
        let b = field.encode(&b);
        let generator = Affine {
            x: field.encode(&gx),
            y: field.encode(&gy),
        };
        Self {
            field,
            order: Modular::new(n, words),
            b,
            generator,
        }
    }

    fn point(&self, input: &[u8]) -> Result<Affine, String> {
        let width = self.field.words * 8;
        if input.len() != 1 + 2 * width || input[0] != 4 {
            return Err("An EC key must be an uncompressed P-256/P-384 SEC1 point.".into());
        }
        let x = from_be(&input[1..1 + width]);
        let y = from_be(&input[1 + width..]);
        let field = &self.field;
        if compare(&x, &field.modulus, field.words) != Ordering::Less
            || compare(&y, &field.modulus, field.words) != Ordering::Less
        {
            return Err("EC point coordinates must be below the field modulus.".into());
        }
        let x = field.encode(&x);
        let y = field.encode(&y);
        let x_cubed = field.multiply(&field.square(&x), &x);
        let three_x = field.add(&field.add(&x, &x), &x);
        if field.square(&y) != field.add(&field.subtract(&x_cubed, &three_x), &self.b) {
            return Err("The EC public point is not on the selected curve.".into());
        }
        Ok(Affine { x, y })
    }

    fn double(&self, point: &Point) -> Point {
        if point.z == ZERO || point.y == ZERO {
            return Point::INFINITY;
        }
        let f = &self.field;
        let z2 = f.square(&point.z);
        let y2 = f.square(&point.y);
        // With a=-3, the tangent numerator is 3(X-Z^2)(X+Z^2).
        let tangent = f.multiply(&f.subtract(&point.x, &z2), &f.add(&point.x, &z2));
        let tangent = f.add(&f.add(&tangent, &tangent), &tangent);
        let support = f.multiply(&point.x, &y2);
        let support = f.add(&support, &support);
        let support = f.add(&support, &support); // 4 X Y^2
        let x = f.subtract(&f.square(&tangent), &f.add(&support, &support));
        let eight_y4 = f.square(&y2);
        let eight_y4 = f.add(&eight_y4, &eight_y4);
        let eight_y4 = f.add(&eight_y4, &eight_y4);
        let eight_y4 = f.add(&eight_y4, &eight_y4);
        let y = f.subtract(&f.multiply(&tangent, &f.subtract(&support, &x)), &eight_y4);
        let z = f.multiply(&point.y, &point.z);
        Point {
            x,
            y,
            z: f.add(&z, &z),
        }
    }

    fn add_affine(&self, point: &Point, other: &Affine) -> Point {
        let f = &self.field;
        if point.z == ZERO {
            return Point {
                x: other.x,
                y: other.y,
                z: f.one,
            };
        }
        // Put both points over the same Z before applying the chord slope.
        let z2 = f.square(&point.z);
        let aligned_x = f.multiply(&other.x, &z2);
        let aligned_y = f.multiply(&other.y, &f.multiply(&z2, &point.z));
        let dx = f.subtract(&aligned_x, &point.x);
        let dy = f.subtract(&aligned_y, &point.y);
        if dx == ZERO {
            return if dy == ZERO {
                self.double(point)
            } else {
                Point::INFINITY
            };
        }
        let dx2 = f.square(&dx);
        let dx3 = f.multiply(&dx2, &dx);
        let support = f.multiply(&point.x, &dx2);
        let x = f.subtract(
            &f.subtract(&f.square(&dy), &dx3),
            &f.add(&support, &support),
        );
        let y = f.subtract(
            &f.multiply(&dy, &f.subtract(&support, &x)),
            &f.multiply(&point.y, &dx3),
        );
        let z = f.multiply(&point.z, &dx);
        Point { x, y, z }
    }

    fn combination(&self, u: &Number, v: &Number, public: &Affine) -> Point {
        let mut result = Point::INFINITY;
        // A single bounded double-and-add pass computes uG+vQ. Scalar bits and
        // all points here are public; exceptional additions are handled above.
        for bit in (0..self.order.words * 64).rev() {
            result = self.double(&result);
            if (u[bit / 64] >> (bit % 64)) & 1 != 0 {
                result = self.add_affine(&result, &self.generator);
            }
            if (v[bit / 64] >> (bit % 64)) & 1 != 0 {
                result = self.add_affine(&result, public);
            }
        }
        result
    }
}

fn parameters(curve: Curve) -> &'static Parameters {
    static P256: OnceLock<Parameters> = OnceLock::new();
    static P384: OnceLock<Parameters> = OnceLock::new();
    match curve {
        Curve::P256 => P256.get_or_init(|| Parameters::new(curve)),
        Curve::P384 => P384.get_or_init(|| Parameters::new(curve)),
    }
}

impl Modular {
    fn new(modulus: Number, words: usize) -> Self {
        // Newton lifting in Z/(2^64): each step doubles the correct bits.
        let mut inverse = 1u64;
        for _ in 0..6 {
            inverse = inverse.wrapping_mul(2u64.wrapping_sub(modulus[0].wrapping_mul(inverse)));
        }
        let mut result = Self {
            modulus,
            words,
            negative_inverse: inverse.wrapping_neg(),
            r_squared: ZERO,
            one: ZERO,
        };
        let mut value = ONE;
        for _ in 0..words * 64 {
            value = result.add(&value, &value);
        }
        result.one = value; // R mod modulus
        for _ in 0..words * 64 {
            value = result.add(&value, &value);
        }
        result.r_squared = value;
        result
    }

    fn add(&self, left: &Number, right: &Number) -> Number {
        let (value, carry) = sum(left, right, self.words);
        if carry != 0 || compare(&value, &self.modulus, self.words) != Ordering::Less {
            difference(&value, &self.modulus, self.words).0
        } else {
            value
        }
    }

    fn subtract(&self, left: &Number, right: &Number) -> Number {
        let (value, borrow) = difference(left, right, self.words);
        if borrow != 0 {
            sum(&value, &self.modulus, self.words).0
        } else {
            value
        }
    }

    fn multiply(&self, left: &Number, right: &Number) -> Number {
        let mut wide = [0u64; 14];
        let n = self.words;
        // Full fixed-width product. A product plus one limb and carry is at
        // most 2^128-1, so each u128 accumulator fits without wrapping.
        for (i, &left_word) in left[..n].iter().enumerate() {
            let mut carry = 0u128;
            for (j, &right_word) in right[..n].iter().enumerate() {
                let total = u128::from(left_word) * u128::from(right_word)
                    + u128::from(wide[i + j])
                    + carry;
                wide[i + j] = total as u64;
                carry = total >> 64;
            }
            for word in &mut wide[i + n..=2 * n] {
                let total = u128::from(*word) + carry;
                *word = total as u64;
                carry = total >> 64;
            }
            debug_assert_eq!(carry, 0);
        }
        // Cancel each low limb by adding a multiple of the odd modulus.
        // Division by R then consists of taking the upper n limbs.
        for i in 0..n {
            let factor = wide[i].wrapping_mul(self.negative_inverse);
            let mut carry = 0u128;
            for (j, &modulus_word) in self.modulus[..n].iter().enumerate() {
                let total =
                    u128::from(factor) * u128::from(modulus_word) + u128::from(wide[i + j]) + carry;
                wide[i + j] = total as u64;
                carry = total >> 64;
            }
            for word in &mut wide[i + n..=2 * n] {
                let total = u128::from(*word) + carry;
                *word = total as u64;
                carry = total >> 64;
            }
            debug_assert_eq!(carry, 0);
            debug_assert_eq!(wide[i], 0);
        }
        let mut value = ZERO;
        value[..n].copy_from_slice(&wide[n..2 * n]);
        // Reduced inputs imply the REDC result is below 2*modulus.
        if wide[2 * n] != 0 || compare(&value, &self.modulus, n) != Ordering::Less {
            value = difference(&value, &self.modulus, n).0;
        }
        debug_assert_eq!(compare(&value, &self.modulus, n), Ordering::Less);
        value
    }

    fn square(&self, value: &Number) -> Number {
        self.multiply(value, value)
    }

    fn encode(&self, value: &Number) -> Number {
        self.multiply(value, &self.r_squared)
    }

    fn decode(&self, value: &Number) -> Number {
        self.multiply(value, &ONE)
    }

    fn inverse(&self, value: &Number) -> Number {
        // Both fixed moduli are prime; Fermat inversion has a fixed maximum
        // of 384 squarings and 384 multiplies. The caller excludes zero.
        debug_assert_ne!(*value, ZERO);
        let exponent = difference(&self.modulus, &TWO, self.words).0;
        let mut result = self.one;
        for bit in (0..self.words * 64).rev() {
            result = self.square(&result);
            if (exponent[bit / 64] >> (bit % 64)) & 1 != 0 {
                result = self.multiply(&result, value);
            }
        }
        result
    }
}

fn compare(left: &Number, right: &Number, words: usize) -> Ordering {
    for (&left, &right) in left[..words].iter().zip(&right[..words]).rev() {
        let order = left.cmp(&right);
        if order != Ordering::Equal {
            return order;
        }
    }
    Ordering::Equal
}

fn sum(left: &Number, right: &Number, words: usize) -> (Number, u64) {
    let mut result = ZERO;
    let mut carry = 0u128;
    for index in 0..words {
        let total = u128::from(left[index]) + u128::from(right[index]) + carry;
        result[index] = total as u64;
        carry = total >> 64;
    }
    (result, carry as u64)
}

fn difference(left: &Number, right: &Number, words: usize) -> (Number, u64) {
    let mut result = ZERO;
    let mut borrow = false;
    for index in 0..words {
        let (value, first) = left[index].overflowing_sub(right[index]);
        let (value, second) = value.overflowing_sub(u64::from(borrow));
        result[index] = value;
        borrow = first || second;
    }
    (result, u64::from(borrow))
}

fn from_be(bytes: &[u8]) -> Number {
    debug_assert!(bytes.len() <= 48);
    let mut value = ZERO;
    for (index, &byte) in bytes.iter().rev().enumerate() {
        value[index / 8] |= u64::from(byte) << (8 * (index % 8));
    }
    value
}

const fn hex_number(text: &str) -> Number {
    let bytes = text.as_bytes();
    assert!(bytes.len() <= 96);
    let mut value = ZERO;
    let mut index = 0;
    while index < bytes.len() {
        let digit = match bytes[bytes.len() - index - 1] {
            b'0'..=b'9' => bytes[bytes.len() - index - 1] - b'0',
            b'A'..=b'F' => bytes[bytes.len() - index - 1] - b'A' + 10,
            _ => panic!("Invalid static curve constant"),
        };
        value[index / 16] |= (digit as u64) << (4 * (index % 16));
        index += 1;
    }
    value
}

fn signature(input: &[u8], order: &Modular) -> Result<(Number, Number), String> {
    // Maximum DER length is 72/104 bytes, so canonical DER always uses
    // one-octet short lengths. Indefinite/long lengths cannot be canonical.
    if input.len() < 8
        || input.len() > 2 * order.words * 8 + 8
        || input[0] != 0x30
        || usize::from(input[1]) != input.len() - 2
    {
        return Err("ECDSA signature must be a strict DER SEQUENCE of r and s.".into());
    }
    let mut position = 2;
    let r = signature_integer(input, &mut position, order)?;
    let s = signature_integer(input, &mut position, order)?;
    if position != input.len() {
        return Err("ECDSA signature has trailing DER fields.".into());
    }
    Ok((r, s))
}

fn signature_integer(
    input: &[u8],
    position: &mut usize,
    order: &Modular,
) -> Result<Number, String> {
    let header = input
        .get(*position..*position + 2)
        .ok_or("Truncated ECDSA INTEGER.")?;
    let length = usize::from(header[1]);
    if header[0] != 2 || length == 0 || length > order.words * 8 + 1 {
        return Err("Invalid ECDSA INTEGER tag or length.".into());
    }
    *position += 2;
    let mut bytes = input
        .get(*position..*position + length)
        .ok_or("Truncated ECDSA INTEGER.")?;
    *position += length;
    if bytes[0] & 0x80 != 0 {
        return Err("ECDSA r and s must be positive.".into());
    }
    if bytes[0] == 0 {
        if bytes.len() == 1 || bytes[1] & 0x80 == 0 {
            return Err("ECDSA INTEGER is zero or has unnecessary sign padding.".into());
        }
        bytes = &bytes[1..];
    }
    if bytes.len() > order.words * 8 {
        return Err("ECDSA INTEGER exceeds the curve order size.".into());
    }
    let value = from_be(bytes);
    if value == ZERO || compare(&value, &order.modulus, order.words) != Ordering::Less {
        return Err("ECDSA r and s must be in the interval 1..n-1.".into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(text: &str) -> Vec<u8> {
        text.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    fn encoded(point: &Affine, parameters: &Parameters) -> Vec<u8> {
        let mut output = vec![4];
        for coordinate in [point.x, point.y] {
            let value = parameters.field.decode(&coordinate);
            for index in (0..parameters.field.words * 8).rev() {
                output.push((value[index / 8] >> (8 * (index % 8))) as u8);
            }
        }
        output
    }

    fn der_pair(r: &[u8], s: &[u8]) -> Vec<u8> {
        let mut result = vec![0x30, 0];
        for value in [r, s] {
            let pad = usize::from(value[0] & 0x80 != 0);
            result.extend_from_slice(&[2, (value.len() + pad) as u8]);
            if pad != 0 {
                result.push(0);
            }
            result.extend_from_slice(value);
        }
        result[1] = (result.len() - 2) as u8;
        result
    }

    #[test]
    fn published_rfc6979_p256_and_p384_signatures() {
        // Public vector data from RFC 6979 A.2.5 / A.2.6, message "sample".
        let vectors = [
            (Curve::P256,
             "0460FED4BA255A9D31C961EB74C6356D68C049B8923B61FA6CE669622E60F29FB67903FE1008B8BC99A41AE9E95628BC64F2F1B20C2D7E9F5177A3C294D4462299",
             "AF2BDBE1AA9B6EC1E2ADE1D694F41FC71A831D0268E9891562113D8A62ADD1BF",
             "EFD48B2AACB6A8FD1140DD9CD45E81D69D2C877B56AAF991C34D0EA84EAF3716",
             "F7CB1C942D657C41D436C7A1B6E29F65F3E900DBB9AFF4064DC4AB2F843ACDA8"),
            (Curve::P384,
             "04EC3A4E415B4E19A4568618029F427FA5DA9A8BC4AE92E02E06AAE5286B300C64DEF8F0EA9055866064A254515480BC138015D9B72D7D57244EA8EF9AC0C621896708A59367F9DFB9F54CA84B3F1C9DB1288B231C3AE0D4FE7344FD2533264720",
             "9A9083505BC92276AEC4BE312696EF7BF3BF603F4BBD381196A029F340585312313BCA4A9B5B890EFEE42C77B1EE25FE",
             "94EDBB92A5ECB8AAD4736E56C691916B3F88140666CE9FA73D64C4EA95AD133C81A648152E44ACF96E36DD1E80FABE46",
             "99EF4AEB15F178CEA1FE40DB2603138F130E740A19624526203B6351D0A3A94FA329C145786E679E7B82C71A38628AC8"),
        ];
        for (curve, key, digest, r, s) in vectors {
            let key = bytes(key);
            let mut digest = bytes(digest);
            let signature = der_pair(&bytes(r), &bytes(s));
            assert!(validate_key(curve, &key).is_ok());
            verify(curve, &key, &digest, &signature).unwrap();
            digest[0] ^= 1;
            assert!(verify(curve, &key, &digest, &signature).is_err());
            for length in 0..signature.len() {
                assert!(verify(curve, &key, &digest, &signature[..length]).is_err());
            }
        }
    }

    #[test]
    fn finite_on_curve_keys_and_group_boundaries() {
        for curve in [Curve::P256, Curve::P384] {
            let p = parameters(curve);
            let generator = encoded(&p.generator, p);
            validate_key(curve, &generator).unwrap();
            let mut off_curve = generator.clone();
            *off_curve.last_mut().unwrap() ^= 1;
            assert!(validate_key(curve, &off_curve).is_err());
            let mut zero = vec![0; generator.len()];
            zero[0] = 4;
            assert!(validate_key(curve, &zero).is_err());
            for prefix in [0, 2, 3, 6, 7] {
                let mut invalid = generator.clone();
                invalid[0] = prefix;
                assert!(validate_key(curve, &invalid).is_err());
            }
            let original = p.combination(&ONE, &ZERO, &p.generator);
            assert_eq!(original.x, p.generator.x);
            assert_eq!(original.y, p.generator.y);
            assert_eq!(original.z, p.field.one);
            assert_eq!(p.combination(&p.order.modulus, &ZERO, &p.generator).z, ZERO);
            let negative = Affine {
                x: p.generator.x,
                y: p.field.subtract(&ZERO, &p.generator.y),
            };
            assert_eq!(p.combination(&ONE, &ONE, &negative).z, ZERO);
            let doubled = p.combination(&TWO, &ZERO, &p.generator);
            let added = p.combination(&ONE, &ONE, &p.generator);
            assert_eq!(doubled.x, added.x);
            assert_eq!(doubled.y, added.y);
            assert_eq!(doubled.z, added.z);
        }
    }

    #[test]
    fn strict_der_scalar_ranges_and_digest_lengths() {
        let p = parameters(Curve::P256);
        for invalid in [
            &[0x30, 6, 2, 1, 0, 2, 1, 1][..],
            &[0x30, 6, 2, 1, 1, 2, 1, 0][..],
            &[0x30, 6, 2, 1, 0x80, 2, 1, 1][..],
            &[0x30, 7, 2, 2, 0, 1, 2, 1, 1][..],
            &[0x30, 0x81, 6, 2, 1, 1, 2, 1, 1][..],
            &[0x30, 6, 2, 0x81, 1, 2, 1, 1][..],
            &[0x30, 6, 2, 1, 1, 2, 1, 1, 0][..],
        ] {
            assert!(signature(invalid, &p.order).is_err());
        }
        let order = bytes("FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551");
        assert!(signature(&der_pair(&order, &[1]), &p.order).is_err());
        assert!(signature(&der_pair(&[1], &order), &p.order).is_err());
        let key = encoded(&p.generator, p);
        for length in [0, 1, 31, 33, 47, 49, 63, 65] {
            assert!(verify(Curve::P256, &key, &vec![0; length], &der_pair(&[1], &[1])).is_err());
        }
    }
}
