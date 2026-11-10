//! Bounded RSA/ECDSA X.509 server verification implemented for this browser.
//!
//! The accepted path is an ordered leaf -> issuer prefix of at most eight
//! presented certificates. It ends as soon as an explicitly supplied root
//! validates the current certificate; an unnecessary cross-sign or root suffix
//! does not extend that path. Names are bound by exact DER, not display strings.
//! RFC 5280 supplies the certificate/constraint rules, RFC 9525 supplies SAN
//! identity matching, RFC 8017 supplies RSA signature identifiers, and RFCs
//! 5480 / 5758 specify the EC key and ECDSA signature encodings.
//!
//! This accepts rsaEncryption and named P-256/P-384 EC public keys. It rejects
//! RSA-PSS public keys, other curves, SHA-1, name/policy constraints, must-staple,
//! and unknown critical extensions. It performs no AIA fetching, revocation
//! checking, CT evaluation, OS trust evaluation, or CN fallback.

use crate::crypto_ec::{self, Curve};
use crate::crypto_hash::sha256;
use crate::crypto_rsa::{self, Hash};
use crate::crypto_sha512::{sha384, sha512};
use std::net::IpAddr;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_CERT_BYTES: usize = 64 * 1024;
pub const MAX_CHAIN: usize = 8;
pub const MAX_ROOTS: usize = 128;
const MAX_PEM_BYTES: usize = 12 * 1024 * 1024;
const MAX_EXTENSIONS: usize = 64;
const MAX_SAN_NAMES: usize = 256;
const MAX_ROOT_CANDIDATES: usize = 8;

#[derive(Debug)]
pub enum PublicKey {
    Rsa { modulus: Vec<u8>, exponent: Vec<u8> },
    Ec { curve: Curve, point: Vec<u8> },
}

#[derive(Debug)]
pub struct VerifiedServer {
    pub key: PublicKey,
    pub summary: String,
}

#[derive(Clone, Copy)]
enum PublicKeyRef<'a> {
    Rsa {
        modulus: &'a [u8],
        exponent: &'a [u8],
    },
    Ec {
        curve: Curve,
        point: &'a [u8],
    },
}

impl PublicKeyRef<'_> {
    fn owned(self) -> PublicKey {
        match self {
            Self::Rsa { modulus, exponent } => PublicKey::Rsa {
                modulus: modulus.to_vec(),
                exponent: exponent.to_vec(),
            },
            Self::Ec { curve, point } => PublicKey::Ec {
                curve,
                point: point.to_vec(),
            },
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Rsa { .. } => "RSA",
            Self::Ec {
                curve: Curve::P256, ..
            } => "ECDSA P-256",
            Self::Ec {
                curve: Curve::P384, ..
            } => "ECDSA P-384",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Signature {
    Pkcs1(Hash),
    Pss { hash: Hash, salt_len: usize },
    Ecdsa(Hash),
}

struct Date {
    seconds: i64,
    display: String,
}

#[derive(Clone, Copy)]
struct BasicConstraints {
    ca: bool,
    path_len: Option<usize>,
    critical: bool,
}

enum Identity<'a> {
    Dns(&'a str),
    Ip(&'a [u8]),
}

#[derive(Default)]
struct Extensions<'a> {
    basic: Option<BasicConstraints>,
    key_usage: Option<u16>,
    server_auth: Option<bool>,
    subject_key_id: Option<&'a [u8]>,
    authority_key_id: Option<&'a [u8]>,
    identities: Vec<Identity<'a>>,
    san_present: bool,
    san_critical: bool,
}

struct Certificate<'a> {
    der: &'a [u8],
    tbs: &'a [u8],
    signature: &'a [u8],
    algorithm: Signature,
    issuer: &'a [u8],
    subject: &'a [u8],
    before: Date,
    after: Date,
    key: PublicKeyRef<'a>,
    extensions: Extensions<'a>,
    fingerprint: [u8; 32],
}

/// Verify a bounded, ordered server path against explicitly supplied roots.
/// Roots must be self-issued supported CA certificates; their own signatures and
/// validity are checked too. KU/EKU restrictions apply whenever present.
pub fn verify_server(
    chain: &[Vec<u8>],
    roots: &[Vec<u8>],
    hostname: &str,
    now: SystemTime,
) -> Result<VerifiedServer, String> {
    if chain.is_empty() || chain.len() > MAX_CHAIN {
        return Err("A server chain must contain 1 to 8 certificates.".into());
    }
    if roots.is_empty() || roots.len() > MAX_ROOTS {
        return Err("Supply 1 to 128 explicit trusted root certificates.".into());
    }
    let now = system_seconds(now)?;
    let chain: Vec<_> = chain
        .iter()
        .enumerate()
        .map(|(index, der)| {
            parse_certificate(der).map_err(|error| format!("Certificate {}: {error}", index + 1))
        })
        .collect::<Result<_, _>>()?;
    let roots: Vec<_> = roots
        .iter()
        .enumerate()
        .map(|(index, der)| {
            parse_certificate(der).map_err(|error| format!("Trusted root {}: {error}", index + 1))
        })
        .collect::<Result<_, _>>()?;
    for (index, certificate) in chain.iter().enumerate() {
        if chain[..index].iter().any(|previous| {
            previous.fingerprint == certificate.fingerprint && previous.der == certificate.der
        }) {
            return Err("A certificate is repeated in the server chain.".into());
        }
    }
    let leaf = &chain[0];
    check_time(leaf, now)?;
    check_purpose(leaf)?;
    if leaf.extensions.basic.is_some_and(|basic| basic.ca) {
        return Err("The server certificate must be an end-entity certificate, not a CA.".into());
    }
    if leaf
        .extensions
        .key_usage
        .is_some_and(|usage| usage & 1 == 0)
    {
        return Err("The server certificate does not permit digital signatures.".into());
    }
    match_hostname(&leaf.extensions, hostname)?;

    // Try our own roots at every verified prefix. A server may append a
    // cross-signed version of a root we already trust, leading onward to an
    // older CA we do not trust. Continuing past our root unnecessarily would
    // reject an otherwise valid path (and could inherit an expired cross-sign).
    // Trust still comes exclusively from the supplied store: issuer names and
    // identifiers only select candidates; every chosen link is signed by the
    // actual stored root key, with that root's constraints applied.
    let mut candidates = 0;
    let mut failure = None;
    for (index, certificate) in chain.iter().enumerate() {
        if index != 0 {
            check_time(certificate, now)?;
            check_ca(certificate, intermediate_count(&chain[1..index]))?;
            if !issuer_matches(&chain[index - 1], certificate) {
                return Err(
                    "Server certificates are not an ordered issuer/subject/key-identifier chain."
                        .into(),
                );
            }
            verify_signature(&chain[index - 1], certificate)?;
        }
        for root in &roots {
            if !issuer_matches(certificate, root) {
                continue;
            }
            candidates += 1;
            if candidates > MAX_ROOT_CANDIDATES {
                return Err("Too many matching root candidates for this bounded verifier.".into());
            }
            let result = check_anchor(root, intermediate_count(&chain[1..index + 1]), now)
                .and_then(|()| verify_signature(certificate, root));
            match result {
                Ok(()) => {
                    return Ok(VerifiedServer {
                        key: leaf.key.owned(),
                        summary: format!(
                            "{} certificate path verified for {hostname}\nLeaf SHA-256: {}\nTrusted root SHA-256: {}\nLeaf validity: {} to {}\nVerified server path: {} certificate(s); unused supplied suffix: {} certificate(s).\nRevocation and certificate transparency were not checked.",
                            leaf.key.description(), fingerprint(&leaf.fingerprint), fingerprint(&root.fingerprint), leaf.before.display, leaf.after.display,
                            index + 1, chain.len() - index - 1
                        ),
                    });
                }
                Err(error) => failure = Some(error),
            }
        }
    }
    Err(match failure {
        Some(error) => format!("No supplied trusted root validates this chain: {error}"),
        None => "The server chain does not reach a supplied trusted root.".to_owned(),
    })
}

fn intermediate_count(certificates: &[Certificate<'_>]) -> usize {
    certificates
        .iter()
        .filter(|certificate| certificate.subject != certificate.issuer)
        .count()
}

fn check_time(certificate: &Certificate<'_>, now: i64) -> Result<(), String> {
    if now < certificate.before.seconds {
        Err("A certificate is not valid yet; check the system clock.".into())
    } else if now > certificate.after.seconds {
        Err("A certificate has expired.".into())
    } else {
        Ok(())
    }
}

fn check_purpose(certificate: &Certificate<'_>) -> Result<(), String> {
    if certificate.extensions.server_auth == Some(false) {
        return Err(
            "A certificate's extended key usage does not permit TLS server authentication.".into(),
        );
    }
    Ok(())
}

fn check_ca(certificate: &Certificate<'_>, below: usize) -> Result<(), String> {
    let basic = certificate
        .extensions
        .basic
        .ok_or("An issuer is missing CA basic constraints.")?;
    if !basic.ca || !basic.critical {
        return Err("An issuer needs critical basic constraints with CA=true.".into());
    }
    if certificate
        .extensions
        .key_usage
        .is_some_and(|usage| usage & (1 << 5) == 0)
    {
        return Err("An issuer's key usage does not permit certificate signing.".into());
    }
    if basic.path_len.is_some_and(|limit| below > limit) {
        return Err("The certificate chain exceeds an issuer's path-length constraint.".into());
    }
    check_purpose(certificate)
}

fn check_anchor(root: &Certificate<'_>, below: usize, now: i64) -> Result<(), String> {
    check_time(root, now)?;
    check_ca(root, below)?;
    if root.issuer != root.subject {
        return Err(
            "This build requires a self-issued certificate as its explicit trust anchor.".into(),
        );
    }
    verify_signature(root, root)
}

fn issuer_matches(child: &Certificate<'_>, issuer: &Certificate<'_>) -> bool {
    child.issuer == issuer.subject
        && match (
            child.extensions.authority_key_id,
            issuer.extensions.subject_key_id,
        ) {
            (Some(authority), Some(subject)) => authority == subject,
            _ => true,
        }
}

fn verify_signature(child: &Certificate<'_>, issuer: &Certificate<'_>) -> Result<(), String> {
    let result = match (child.algorithm, issuer.key) {
        (Signature::Pkcs1(hash), PublicKeyRef::Rsa { modulus, exponent }) => {
            crypto_rsa::verify_pkcs1v15(modulus, exponent, hash, child.tbs, child.signature)
        }
        (Signature::Pss { hash, salt_len }, PublicKeyRef::Rsa { modulus, exponent }) => {
            crypto_rsa::verify_pss(
                modulus,
                exponent,
                hash,
                salt_len,
                child.tbs,
                child.signature,
            )
        }
        (Signature::Ecdsa(hash), PublicKeyRef::Ec { curve, point }) => match hash {
            Hash::Sha256 => crypto_ec::verify(curve, point, &sha256(child.tbs), child.signature),
            Hash::Sha384 => crypto_ec::verify(curve, point, &sha384(child.tbs), child.signature),
            Hash::Sha512 => crypto_ec::verify(curve, point, &sha512(child.tbs), child.signature),
        },
        _ => {
            Err("The certificate signature algorithm does not match its issuer's key type.".into())
        }
    };
    result.map_err(|error| format!("Certificate signature verification failed: {error}"))
}

fn fingerprint(hash: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut result = String::with_capacity(95);
    for (index, byte) in hash.iter().copied().enumerate() {
        if index != 0 {
            result.push(':');
        }
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 15)]));
    }
    result
}

fn system_seconds(now: SystemTime) -> Result<i64, String> {
    match now.duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_secs())
            .map_err(|_| "System time is outside the supported range.".into()),
        Err(error) => {
            let duration = error.duration();
            let seconds = i64::try_from(duration.as_secs())
                .map_err(|_| "System time is outside the supported range.".to_owned())?;
            Ok(-seconds - i64::from(duration.subsec_nanos() != 0))
        }
    }
}

// OID content octets, authored from RFCs 5280 / 8017 / 5480 / 5758.
const RSA_ENCRYPTION: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 1, 1, 1];
const RSA_SHA256: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 1, 1, 11];
const RSA_SHA384: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 1, 1, 12];
const RSA_SHA512: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 1, 1, 13];
const RSA_PSS: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 1, 1, 10];
const EC_PUBLIC_KEY: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 2, 1];
const PRIME256V1: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 3, 1, 7];
const SECP384R1: &[u8] = &[0x2b, 0x81, 4, 0, 0x22];
const ECDSA_SHA256: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 4, 3, 2];
const ECDSA_SHA384: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 4, 3, 3];
const ECDSA_SHA512: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 4, 3, 4];
const MGF1: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 1, 1, 8];
const SHA256: &[u8] = &[0x60, 0x86, 0x48, 1, 0x65, 3, 4, 2, 1];
const SHA384: &[u8] = &[0x60, 0x86, 0x48, 1, 0x65, 3, 4, 2, 2];
const SHA512: &[u8] = &[0x60, 0x86, 0x48, 1, 0x65, 3, 4, 2, 3];

fn parse_certificate(der: &[u8]) -> Result<Certificate<'_>, String> {
    if der.is_empty() || der.len() > MAX_CERT_BYTES {
        return Err("Each DER certificate must contain 1 to 65536 bytes.".into());
    }
    validate_der(der)?;
    let certificate = single(der, 0x30)?;
    let mut fields = Reader::new(certificate.value);
    let tbs = fields.expect(0x30)?;
    let outer_algorithm = fields.expect(0x30)?;
    let (signature, unused) = bit_string(fields.expect(0x03)?.value)?;
    fields.finish()?;
    if unused != 0 {
        return Err("Certificate signatures must be byte aligned.".into());
    }
    let mut body = Reader::new(tbs.value);
    let version = body.expect(0xa0)?;
    if integer_u32(single(version.value, 0x02)?.value)? != 2 {
        return Err("Only X.509 version 3 certificates are supported.".into());
    }
    let serial = body.expect(0x02)?.value;
    let magnitude = unsigned_integer(serial)?;
    if serial.len() > 20 || magnitude.iter().all(|byte| *byte == 0) {
        return Err(
            "A certificate serial number must be positive and at most 20 DER octets.".into(),
        );
    }
    let inner_algorithm = body.expect(0x30)?;
    if inner_algorithm.full != outer_algorithm.full {
        return Err("Inner and outer certificate signature algorithms differ.".into());
    }
    let algorithm = signature_algorithm(outer_algorithm)?;
    match algorithm {
        Signature::Pkcs1(_) | Signature::Pss { .. } if !(256..=512).contains(&signature.len()) => {
            return Err("Certificate RSA signatures must be 2048 to 4096 bits.".into());
        }
        Signature::Ecdsa(_) => {
            if !(8..=144).contains(&signature.len()) {
                return Err("Certificate ECDSA signatures must contain 8 to 144 DER bytes.".into());
            }
            validate_der(signature)?;
        }
        _ => {}
    }
    let issuer = body.expect(0x30)?;
    validate_name(issuer)?;
    if issuer.value.is_empty() {
        return Err("A certificate issuer name cannot be empty.".into());
    }
    let mut validity = Reader::new(body.expect(0x30)?.value);
    let before = parse_date(validity.next()?)?;
    let after = parse_date(validity.next()?)?;
    validity.finish()?;
    if before.seconds > after.seconds {
        return Err("Certificate validity dates are reversed.".into());
    }
    let subject = body.expect(0x30)?;
    validate_name(subject)?;
    let key = public_key(body.expect(0x30)?)?;
    // Unique identifiers are obsolete but have no effect on this path policy.
    if body.peek() == Some(0x81) {
        bit_string(body.next()?.value)?;
    }
    if body.peek() == Some(0x82) {
        bit_string(body.next()?.value)?;
    }
    let extensions = if body.peek() == Some(0xa3) {
        parse_extensions(single(body.next()?.value, 0x30)?)?
    } else {
        Extensions::default()
    };
    body.finish()?;
    if subject.value.is_empty() && (!extensions.san_present || !extensions.san_critical) {
        return Err("An empty subject requires a critical subjectAltName extension.".into());
    }
    if extensions
        .basic
        .is_some_and(|basic| basic.path_len.is_some())
        && extensions
            .key_usage
            .is_some_and(|usage| usage & (1 << 5) == 0)
    {
        return Err("A path-length constraint requires certificate-signing key usage.".into());
    }
    if extensions
        .key_usage
        .is_some_and(|usage| usage & (1 << 5) != 0)
        && !extensions.basic.is_some_and(|basic| basic.ca)
    {
        return Err("Certificate-signing key usage requires CA basic constraints.".into());
    }
    if matches!(key, PublicKeyRef::Ec { .. })
        && extensions
            .key_usage
            .is_some_and(|usage| usage & ((1 << 2) | (1 << 3)) != 0)
    {
        return Err("EC key usage cannot permit key or data encipherment.".into());
    }
    Ok(Certificate {
        der,
        tbs: tbs.full,
        signature,
        algorithm,
        issuer: issuer.full,
        subject: subject.full,
        before,
        after,
        key,
        extensions,
        fingerprint: sha256(der),
    })
}

fn public_key(spki: Tlv<'_>) -> Result<PublicKeyRef<'_>, String> {
    let mut fields = Reader::new(spki.value);
    let mut algorithm = Reader::new(fields.expect(0x30)?.value);
    let oid = algorithm.expect(0x06)?.value;
    let (bits, unused) = bit_string(fields.expect(0x03)?.value)?;
    fields.finish()?;
    if unused != 0 {
        return Err("The public key bit string must be byte aligned.".into());
    }
    match oid {
        RSA_ENCRYPTION => {
            if !algorithm.expect(0x05)?.value.is_empty() {
                return Err("rsaEncryption needs NULL parameters.".into());
            }
            algorithm.finish()?;
            let mut key = Reader::new(single(bits, 0x30)?.value);
            let modulus = unsigned_integer(key.expect(0x02)?.value)?;
            let exponent = unsigned_integer(key.expect(0x02)?.value)?;
            key.finish()?;
            crypto_rsa::validate_key(modulus, exponent)?;
            Ok(PublicKeyRef::Rsa { modulus, exponent })
        }
        EC_PUBLIC_KEY => {
            let curve = match algorithm.expect(0x06)?.value {
                PRIME256V1 => Curve::P256,
                SECP384R1 => Curve::P384,
                _ => return Err("Only named P-256 and P-384 EC public keys are supported.".into()),
            };
            algorithm.finish()?;
            crypto_ec::validate_key(curve, bits)?;
            Ok(PublicKeyRef::Ec { curve, point: bits })
        }
        _ => Err("Only rsaEncryption and named P-256/P-384 EC public keys are supported.".into()),
    }
}

fn signature_algorithm(sequence: Tlv<'_>) -> Result<Signature, String> {
    let mut fields = Reader::new(sequence.value);
    let oid = fields.expect(0x06)?.value;
    let hash = match oid {
        RSA_SHA256 => Some(Hash::Sha256),
        RSA_SHA384 => Some(Hash::Sha384),
        RSA_SHA512 => Some(Hash::Sha512),
        _ => None,
    };
    if let Some(hash) = hash {
        optional_null(&mut fields)?;
        return Ok(Signature::Pkcs1(hash));
    }
    let hash = match oid {
        ECDSA_SHA256 => Some(Hash::Sha256),
        ECDSA_SHA384 => Some(Hash::Sha384),
        ECDSA_SHA512 => Some(Hash::Sha512),
        _ => None,
    };
    if let Some(hash) = hash {
        // RFC 5758 requires absent parameters, including no ASN.1 NULL.
        fields.finish()?;
        return Ok(Signature::Ecdsa(hash));
    }
    if oid != RSA_PSS {
        return Err("Only RSA/ECDSA SHA-256/384/512 certificate signatures are supported.".into());
    }
    let mut parameters = Reader::new(fields.expect(0x30)?.value);
    fields.finish()?;
    // Omitted hash/MGF fields default to SHA-1, which this verifier forbids.
    let hash = hash_algorithm(single(parameters.expect(0xa0)?.value, 0x30)?)?;
    let mut mgf = Reader::new(single(parameters.expect(0xa1)?.value, 0x30)?.value);
    if mgf.expect(0x06)?.value != MGF1 {
        return Err("RSA-PSS requires MGF1.".into());
    }
    let mgf_hash = hash_algorithm(mgf.expect(0x30)?)?;
    mgf.finish()?;
    if mgf_hash != hash {
        return Err("RSA-PSS with a different MGF1 hash is unsupported.".into());
    }
    let salt_len = if parameters.peek() == Some(0xa2) {
        let length = integer_u32(single(parameters.next()?.value, 0x02)?.value)? as usize;
        if length == 20 {
            return Err("DER must omit the default RSA-PSS salt length.".into());
        }
        length
    } else {
        20
    };
    if salt_len > 512 {
        return Err("RSA-PSS salt length exceeds the key-size bound.".into());
    }
    if parameters.peek() == Some(0xa3) {
        return Err("Only the omitted default RSA-PSS trailer field is supported.".into());
    }
    parameters.finish()?;
    Ok(Signature::Pss { hash, salt_len })
}

fn hash_algorithm(sequence: Tlv<'_>) -> Result<Hash, String> {
    let mut fields = Reader::new(sequence.value);
    let hash = match fields.expect(0x06)?.value {
        SHA256 => Hash::Sha256,
        SHA384 => Hash::Sha384,
        SHA512 => Hash::Sha512,
        _ => return Err("Only SHA-256, SHA-384 and SHA-512 are supported here.".into()),
    };
    optional_null(&mut fields)?;
    Ok(hash)
}

fn optional_null(fields: &mut Reader<'_>) -> Result<(), String> {
    if fields.peek().is_some() && !fields.expect(0x05)?.value.is_empty() {
        return Err("Algorithm NULL parameters must be empty.".into());
    }
    fields.finish()
}

fn parse_extensions(sequence: Tlv<'_>) -> Result<Extensions<'_>, String> {
    let mut fields = Reader::new(sequence.value);
    let mut extensions = Extensions::default();
    let mut seen = Vec::new();
    while fields.peek().is_some() {
        if seen.len() == MAX_EXTENSIONS {
            return Err("Certificate extension count exceeds 64.".into());
        }
        let mut extension = Reader::new(fields.expect(0x30)?.value);
        let oid = extension.expect(0x06)?.value;
        if seen.contains(&oid) {
            return Err("A certificate extension is duplicated.".into());
        }
        seen.push(oid);
        let critical = if extension.peek() == Some(0x01) {
            if extension.next()?.value != [0xff] {
                return Err("DER must omit a false extension critical flag.".into());
            }
            true
        } else {
            false
        };
        let value = extension.expect(0x04)?.value;
        extension.finish()?;
        validate_der(value)?;
        match oid {
            [0x55, 0x1d, 0x13] => {
                let mut basic = Reader::new(single(value, 0x30)?.value);
                let ca = if basic.peek() == Some(0x01) {
                    if basic.next()?.value != [0xff] {
                        return Err("DER must omit CA=false in basic constraints.".into());
                    }
                    true
                } else {
                    false
                };
                let path_len = if basic.peek() == Some(0x02) {
                    Some(integer_u32(basic.next()?.value)? as usize)
                } else {
                    None
                };
                basic.finish()?;
                if path_len.is_some() && !ca {
                    return Err("A non-CA certificate cannot have a path-length constraint.".into());
                }
                extensions.basic = Some(BasicConstraints {
                    ca,
                    path_len,
                    critical,
                });
            }
            [0x55, 0x1d, 0x0f] => {
                let (bits, unused) = bit_string(single(value, 0x03)?.value)?;
                if bits.is_empty()
                    || bits.len() > 2
                    || bits[bits.len() - 1] == 0
                    || bits[bits.len() - 1].trailing_zeros() != u32::from(unused)
                    || bits.len() == 2 && unused != 7
                {
                    return Err("Invalid or non-minimal keyUsage bit string.".into());
                }
                let mut usage = 0;
                for bit in 0..9 {
                    if bits
                        .get(bit / 8)
                        .is_some_and(|byte| byte & (0x80 >> (bit % 8)) != 0)
                    {
                        usage |= 1 << bit;
                    }
                }
                if usage & ((1 << 7) | (1 << 8)) != 0 && usage & (1 << 4) == 0 {
                    return Err("Encipher-only/decipher-only usage requires key agreement.".into());
                }
                extensions.key_usage = Some(usage);
            }
            [0x55, 0x1d, 0x25] => {
                let mut purposes = Reader::new(single(value, 0x30)?.value);
                let mut server = false;
                let mut purposes_seen = Vec::new();
                while purposes.peek().is_some() {
                    if purposes_seen.len() == 32 {
                        return Err("Too many extended key purposes.".into());
                    }
                    let purpose = purposes.expect(0x06)?.value;
                    if purposes_seen.contains(&purpose) {
                        return Err("An extended key purpose is duplicated.".into());
                    }
                    purposes_seen.push(purpose);
                    // serverAuth or anyExtendedKeyUsage; other purposes do not
                    // grant server authentication merely by being present.
                    server |=
                        purpose == [0x2b, 6, 1, 5, 5, 7, 3, 1] || purpose == [0x55, 0x1d, 0x25, 0];
                }
                if purposes_seen.is_empty() {
                    return Err("Extended key usage cannot be empty.".into());
                }
                extensions.server_auth = Some(server);
            }
            [0x55, 0x1d, 0x0e] => {
                if critical {
                    return Err("Subject key identifiers must be noncritical.".into());
                }
                let id = single(value, 0x04)?.value;
                check_key_id(id)?;
                extensions.subject_key_id = Some(id);
            }
            [0x55, 0x1d, 0x23] => {
                if critical {
                    return Err("Authority key identifiers must be noncritical.".into());
                }
                let mut authority = Reader::new(single(value, 0x30)?.value);
                if authority.peek() != Some(0x80) {
                    return Err(
                        "This build needs the keyIdentifier form of authorityKeyIdentifier.".into(),
                    );
                }
                let id = authority.next()?.value;
                check_key_id(id)?;
                extensions.authority_key_id = Some(id);
                if authority.peek().is_some() {
                    return Err(
                        "Authority issuer/serial key identifiers are not supported yet.".into(),
                    );
                }
            }
            [0x55, 0x1d, 0x11] => {
                extensions.san_present = true;
                extensions.san_critical = critical;
                let mut names = Reader::new(single(value, 0x30)?.value);
                let mut count = 0;
                while names.peek().is_some() {
                    if count == MAX_SAN_NAMES {
                        return Err("Too many subject alternative names.".into());
                    }
                    count += 1;
                    let name = names.next()?;
                    match name.tag {
                        0x82 => {
                            let dns = std::str::from_utf8(name.value)
                                .map_err(|_| "A DNS SAN must be ASCII.".to_owned())?;
                            validate_dns_name(dns, true)?;
                            extensions.identities.push(Identity::Dns(dns));
                        }
                        0x87 => {
                            if !matches!(name.value.len(), 4 | 16) {
                                return Err("An IP SAN needs exactly 4 or 16 octets.".into());
                            }
                            extensions.identities.push(Identity::Ip(name.value));
                        }
                        0x81 | 0x86 => {
                            if !name.value.is_ascii() || name.value.iter().any(u8::is_ascii_control)
                            {
                                return Err("An IA5 subject alternative name is malformed.".into());
                            }
                            if critical {
                                return Err(
                                    "Critical email/URI subject alternative names are unsupported."
                                        .into(),
                                );
                            }
                        }
                        0xa0 | 0xa3 | 0xa4 | 0xa5 | 0x88 => {
                            if critical {
                                return Err(
                                    "This critical subject alternative name type is unsupported."
                                        .into(),
                                );
                            }
                            if name.tag == 0xa4 {
                                validate_name(single(name.value, 0x30)?)?;
                            }
                            if name.tag == 0x88 {
                                validate_oid(name.value)?;
                            }
                        }
                        _ => return Err("Malformed subject alternative name tag.".into()),
                    }
                }
                if count == 0 {
                    return Err("Subject alternative names cannot be empty.".into());
                }
            }
            // These extensions constrain authorization. Never ignore them just
            // because a certificate incorrectly marked them noncritical.
            [0x55, 0x1d, 0x1e] | [0x55, 0x1d, 0x24] | [0x55, 0x1d, 0x21] | [0x55, 0x1d, 0x36] => {
                return Err(
                    "Name constraints and certificate-policy constraints are not implemented."
                        .into(),
                );
            }
            [0x2b, 6, 1, 5, 5, 7, 1, 24] => {
                return Err("TLS Feature/must-staple certificates are unsupported without revocation stapling.".into());
            }
            _ if critical => {
                return Err("A certificate has an unsupported critical extension.".into())
            }
            _ => { /* Noncritical informational metadata does not grant trust. */ }
        }
    }
    if seen.is_empty() {
        return Err("An extensions sequence cannot be empty.".into());
    }
    Ok(extensions)
}

fn check_key_id(id: &[u8]) -> Result<(), String> {
    if id.is_empty() || id.len() > 64 {
        Err("A key identifier must contain 1 to 64 bytes.".into())
    } else {
        Ok(())
    }
}

fn match_hostname(extensions: &Extensions<'_>, hostname: &str) -> Result<(), String> {
    if !extensions.san_present {
        return Err(
            "The server certificate needs a subjectAltName; CN fallback is disabled.".into(),
        );
    }
    let hostname = hostname.strip_suffix('.').unwrap_or(hostname);
    let matched = if let Ok(ip) = hostname.parse::<IpAddr>() {
        extensions
            .identities
            .iter()
            .any(|identity| match (identity, ip) {
                (Identity::Ip(bytes), IpAddr::V4(address)) => *bytes == address.octets(),
                (Identity::Ip(bytes), IpAddr::V6(address)) => *bytes == address.octets(),
                _ => false,
            })
    } else {
        validate_dns_name(hostname, false)?;
        extensions.identities.iter().any(|identity| match identity {
            Identity::Dns(pattern) => dns_matches(pattern, hostname),
            Identity::Ip(_) => false,
        })
    };
    if matched {
        Ok(())
    } else {
        Err("The certificate subjectAltName does not match the requested host.".into())
    }
}

fn validate_dns_name(name: &str, wildcard: bool) -> Result<(), String> {
    if !name.is_ascii() {
        return Err("DNS certificate identities must use ASCII A-labels.".into());
    }
    let name = name.strip_suffix('.').unwrap_or(name);
    if name.is_empty() || name.len() > 253 {
        return Err("Invalid DNS certificate identity length.".into());
    }
    let ordinary = if let Some(suffix) = name.strip_prefix("*.") {
        if !wildcard || !suffix.contains('.') {
            return Err(
                "Only a whole leftmost wildcard above a multi-label suffix is supported.".into(),
            );
        }
        suffix
    } else {
        name
    };
    for label in ordinary.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("Malformed DNS certificate identity.".into());
        }
    }
    Ok(())
}

fn dns_matches(pattern: &str, hostname: &str) -> bool {
    let pattern = pattern.strip_suffix('.').unwrap_or(pattern);
    if let Some(suffix) = pattern.strip_prefix("*.") {
        hostname
            .split_once('.')
            .is_some_and(|(first, rest)| !first.is_empty() && rest.eq_ignore_ascii_case(suffix))
    } else {
        pattern.eq_ignore_ascii_case(hostname)
    }
}

// A small strict DER reader. It borrows original bytes, including the signed
// TBSCertificate. Lengths are definite and minimal; high tag numbers are outside
// this certificate subset. No parser branch reads beyond its enclosing value.
#[derive(Clone, Copy)]
struct Tlv<'a> {
    tag: u8,
    value: &'a [u8],
    full: &'a [u8],
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, at: 0 }
    }
    fn peek(&self) -> Option<u8> {
        self.data.get(self.at).copied()
    }
    fn next(&mut self) -> Result<Tlv<'a>, String> {
        let start = self.at;
        let tag = *self.data.get(start).ok_or("Truncated DER tag.")?;
        if tag == 0 || tag & 0x1f == 0x1f {
            return Err("Unsupported or invalid DER tag.".into());
        }
        let first = *self.data.get(start + 1).ok_or("Truncated DER length.")?;
        let (length, header) = if first < 0x80 {
            (usize::from(first), 2)
        } else {
            let count = usize::from(first & 0x7f);
            if count == 0 || count > 3 {
                return Err("Indefinite or oversized DER length.".into());
            }
            let encoded = self
                .data
                .get(start + 2..start + 2 + count)
                .ok_or("Truncated DER length.")?;
            if encoded[0] == 0 {
                return Err("DER length has a redundant leading zero.".into());
            }
            let length = encoded
                .iter()
                .fold(0usize, |value, byte| (value << 8) | usize::from(*byte));
            if length < 128 {
                return Err("DER length uses a non-minimal long form.".into());
            }
            (length, 2 + count)
        };
        let value_start = start.checked_add(header).ok_or("DER length overflow.")?;
        let end = value_start
            .checked_add(length)
            .ok_or("DER length overflow.")?;
        let value = self
            .data
            .get(value_start..end)
            .ok_or("Truncated DER value.")?;
        self.at = end;
        Ok(Tlv {
            tag,
            value,
            full: &self.data[start..end],
        })
    }
    fn expect(&mut self, tag: u8) -> Result<Tlv<'a>, String> {
        let value = self.next()?;
        if value.tag != tag {
            return Err(format!(
                "Unexpected DER tag {:02X}; expected {tag:02X}.",
                value.tag
            ));
        }
        Ok(value)
    }
    fn finish(&self) -> Result<(), String> {
        if self.at == self.data.len() {
            Ok(())
        } else {
            Err("Unexpected trailing DER fields.".into())
        }
    }
}

fn single(input: &[u8], tag: u8) -> Result<Tlv<'_>, String> {
    let mut reader = Reader::new(input);
    let item = reader.expect(tag)?;
    reader.finish()?;
    Ok(item)
}

fn unsigned_integer(input: &[u8]) -> Result<&[u8], String> {
    validate_integer(input)?;
    if input[0] & 0x80 != 0 {
        return Err("A positive DER integer was expected.".into());
    }
    Ok(if input.len() > 1 && input[0] == 0 {
        &input[1..]
    } else {
        input
    })
}

fn integer_u32(input: &[u8]) -> Result<u32, String> {
    let input = unsigned_integer(input)?;
    if input.len() > 4 {
        return Err("DER integer exceeds this field's bound.".into());
    }
    Ok(input
        .iter()
        .fold(0, |value, byte| (value << 8) | u32::from(*byte)))
}

fn validate_integer(input: &[u8]) -> Result<(), String> {
    if input.is_empty()
        || input.len() > 1
            && (input[0] == 0 && input[1] & 0x80 == 0 || input[0] == 0xff && input[1] & 0x80 != 0)
    {
        Err("Empty or non-minimal DER integer.".into())
    } else {
        Ok(())
    }
}

fn bit_string(input: &[u8]) -> Result<(&[u8], u8), String> {
    let (&unused, bits) = input
        .split_first()
        .ok_or("Empty DER bit-string encoding.")?;
    if unused > 7
        || bits.is_empty() && unused != 0
        || bits
            .last()
            .is_some_and(|byte| byte & ((1 << unused) - 1) != 0)
    {
        return Err("Invalid unused bits in DER bit string.".into());
    }
    Ok((bits, unused))
}

fn validate_oid(input: &[u8]) -> Result<(), String> {
    if input.is_empty() || input.len() > 64 {
        return Err("Invalid DER object identifier length.".into());
    }
    let mut first = true;
    let mut arc_bytes = 0;
    for &byte in input {
        if first && byte == 0x80 {
            return Err("Non-minimal DER object identifier.".into());
        }
        arc_bytes += 1;
        if arc_bytes > 10 {
            return Err("Object identifier arc exceeds the parser bound.".into());
        }
        first = byte & 0x80 == 0;
        if first {
            arc_bytes = 0;
        }
    }
    if !first {
        return Err("Unterminated DER object identifier.".into());
    }
    Ok(())
}

fn validate_der(input: &[u8]) -> Result<(), String> {
    fn walk(input: &[u8], depth: usize, nodes: &mut usize, set: bool) -> Result<(), String> {
        if depth > 16 {
            return Err("DER nesting exceeds 16 levels.".into());
        }
        let mut reader = Reader::new(input);
        let mut previous: Option<&[u8]> = None;
        while reader.peek().is_some() {
            *nodes += 1;
            if *nodes > 8192 {
                return Err("DER node count exceeds the parser bound.".into());
            }
            let item = reader.next()?;
            if set && previous.is_some_and(|prior| prior > item.full) {
                return Err("DER SET members are not sorted.".into());
            }
            previous = Some(item.full);
            if item.tag & 0x20 != 0 {
                if item.tag & 0xc0 == 0 && !matches!(item.tag, 0x30 | 0x31) {
                    return Err(
                        "Constructed encoding is invalid for this DER universal tag.".into(),
                    );
                }
                walk(item.value, depth + 1, nodes, item.tag == 0x31)?;
            } else {
                match item.tag {
                    0x01 if item.value != [0] && item.value != [0xff] => {
                        return Err("Invalid DER boolean.".into())
                    }
                    0x02 | 0x0a => validate_integer(item.value)?,
                    0x03 => {
                        bit_string(item.value)?;
                    }
                    0x05 if !item.value.is_empty() => return Err("DER NULL must be empty.".into()),
                    0x06 => validate_oid(item.value)?,
                    0x0c => {
                        std::str::from_utf8(item.value)
                            .map_err(|_| "Invalid DER UTF8String.".to_owned())?;
                    }
                    0x10 | 0x11 => {
                        return Err("DER SEQUENCE/SET must use constructed encoding.".into())
                    }
                    0x13 | 0x16 if !item.value.is_ascii() => {
                        return Err("A DER ASCII string contains non-ASCII bytes.".into())
                    }
                    0x17 | 0x18 => {
                        parse_date(item)?;
                    }
                    _ => {}
                }
            }
        }
        reader.finish()
    }
    if input.is_empty() {
        return Err("An encoded DER value cannot be empty.".into());
    }
    let mut root = Reader::new(input);
    root.next()?;
    root.finish()?;
    walk(input, 0, &mut 0, false)
}

fn validate_name(name: Tlv<'_>) -> Result<(), String> {
    let mut rdns = Reader::new(name.value);
    let mut count = 0;
    while rdns.peek().is_some() {
        count += 1;
        if count > 64 {
            return Err("A distinguished name has too many RDNs.".into());
        }
        let mut rdn = Reader::new(rdns.expect(0x31)?.value);
        let mut attributes = 0;
        while rdn.peek().is_some() {
            attributes += 1;
            if attributes > 16 {
                return Err("An RDN has too many attributes.".into());
            }
            let mut attribute = Reader::new(rdn.expect(0x30)?.value);
            attribute.expect(0x06)?;
            let value = attribute.next()?;
            attribute.finish()?;
            if value.value.is_empty()
                || !matches!(value.tag, 0x0c | 0x13 | 0x14 | 0x16 | 0x1c | 0x1e)
            {
                return Err("Unsupported or empty distinguished-name attribute.".into());
            }
            match value.tag {
                0x13 if !value
                    .value
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || b" '()+,-./:=?".contains(byte)) =>
                {
                    return Err("Malformed PrintableString in a distinguished name.".into())
                }
                0x1c if !value.value.len().is_multiple_of(4)
                    || value
                        .value
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|bytes| char::from_u32(u32::from_be_bytes(*bytes)).is_none()) =>
                {
                    return Err("Malformed UniversalString in a distinguished name.".into());
                }
                0x1e if !value.value.len().is_multiple_of(2)
                    || value
                        .value
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .any(|bytes| (0xd800..=0xdfff).contains(&u16::from_be_bytes(*bytes))) =>
                {
                    return Err("Malformed BMPString in a distinguished name.".into());
                }
                _ => {}
            }
        }
        if attributes == 0 {
            return Err("A distinguished-name RDN cannot be empty.".into());
        }
    }
    Ok(())
}

fn parse_date(item: Tlv<'_>) -> Result<Date, String> {
    let input = item.value;
    let (year, offset) = match item.tag {
        0x17 if input.len() == 13 => {
            let year = decimal(&input[..2])?;
            (if year >= 50 { 1900 + year } else { 2000 + year }, 2)
        }
        0x18 if input.len() == 15 => (decimal(&input[..4])?, 4),
        _ => {
            return Err(
                "Certificate dates need canonical UTC/GeneralizedTime with seconds and Z.".into(),
            )
        }
    };
    if input.last() != Some(&b'Z') || !(1..=9999).contains(&year) {
        return Err("Certificate dates must use a supported UTC year and Z suffix.".into());
    }
    let month = decimal(&input[offset..offset + 2])?;
    let day = decimal(&input[offset + 2..offset + 4])?;
    let hour = decimal(&input[offset + 4..offset + 6])?;
    let minute = decimal(&input[offset + 6..offset + 8])?;
    let second = decimal(&input[offset + 8..offset + 10])?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=12).contains(&month)
        || day == 0
        || day > months[month.saturating_sub(1) as usize]
        || hour > 23
        || minute > 59
        || second > 59
    {
        return Err("Certificate date components are out of range.".into());
    }
    let previous = i64::from(year - 1);
    let days_before_year = 365 * previous + previous / 4 - previous / 100 + previous / 400;
    const DAYS_BEFORE_1970: i64 = 719_162;
    let days = days_before_year
        + i64::from(months[..month as usize - 1].iter().sum::<u32>())
        + i64::from(day - 1)
        - DAYS_BEFORE_1970;
    let seconds = days * 86_400 + i64::from(hour * 3600 + minute * 60 + second);
    Ok(Date {
        seconds,
        display: format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC"),
    })
}

fn decimal(input: &[u8]) -> Result<u32, String> {
    if input.is_empty() || !input.iter().all(u8::is_ascii_digit) {
        return Err("A date component is not decimal.".into());
    }
    Ok(input
        .iter()
        .fold(0, |number, byte| number * 10 + u32::from(byte - b'0')))
}

/// Decode CERTIFICATE PEM blocks with our own bounded base64 decoder. Text
/// outside blocks may contain explanatory comments; other PEM object types,
/// incomplete blocks, noncanonical padding, and trailing DER bytes are rejected.
/// Decoding alone grants no trust: verify_server applies certificate policy.
pub fn parse_pem_certificates(input: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if input.len() > MAX_PEM_BYTES {
        return Err("The PEM bundle exceeds 12 MiB.".into());
    }
    let mut certificates = Vec::new();
    let mut encoded: Option<Vec<u8>> = None;
    for line in input.split(|byte| *byte == b'\n') {
        let line = trim_ascii(line);
        if line == b"-----BEGIN CERTIFICATE-----" {
            if encoded.is_some() {
                return Err("Nested PEM certificate blocks are invalid.".into());
            }
            if certificates.len() == MAX_ROOTS {
                return Err("The PEM bundle has more than 128 certificates.".into());
            }
            encoded = Some(Vec::new());
        } else if line == b"-----END CERTIFICATE-----" {
            let body = encoded
                .take()
                .ok_or("A PEM certificate end marker has no start marker.")?;
            let der = decode_base64(&body)?;
            single(&der, 0x30)?;
            validate_der(&der)?;
            certificates.push(der);
        } else if line.starts_with(b"-----BEGIN ") || line.starts_with(b"-----END ") {
            return Err("Only CERTIFICATE PEM blocks are accepted in the trust file.".into());
        } else if let Some(body) = &mut encoded {
            for &byte in line {
                if matches!(byte, b' ' | b'\t' | b'\r') {
                    continue;
                }
                if !byte.is_ascii_alphanumeric() && !matches!(byte, b'+' | b'/' | b'=') {
                    return Err("Invalid character in PEM base64 data.".into());
                }
                body.push(byte);
                if body.len() > MAX_CERT_BYTES.div_ceil(3) * 4 {
                    return Err("A PEM certificate exceeds 64 KiB.".into());
                }
            }
        }
    }
    if encoded.is_some() {
        return Err("A PEM certificate block is unterminated.".into());
    }
    if certificates.is_empty() {
        return Err("No PEM certificates were found.".into());
    }
    Ok(certificates)
}

fn trim_ascii(mut input: &[u8]) -> &[u8] {
    while input.first().is_some_and(u8::is_ascii_whitespace) {
        input = &input[1..];
    }
    while input.last().is_some_and(u8::is_ascii_whitespace) {
        input = &input[..input.len() - 1];
    }
    input
}

fn decode_base64(input: &[u8]) -> Result<Vec<u8>, String> {
    fn digit(byte: u8) -> Result<u8, String> {
        match byte {
            b'A'..=b'Z' => Ok(byte - b'A'),
            b'a'..=b'z' => Ok(byte - b'a' + 26),
            b'0'..=b'9' => Ok(byte - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err("Invalid base64 digit or padding position.".into()),
        }
    }
    if input.is_empty()
        || !input.len().is_multiple_of(4)
        || input.len() > MAX_CERT_BYTES.div_ceil(3) * 4
    {
        return Err("Invalid PEM base64 length.".into());
    }
    let mut output = Vec::with_capacity((input.len() / 4 * 3).min(MAX_CERT_BYTES));
    let groups = input.len() / 4;
    for (index, group) in input.as_chunks::<4>().0.iter().enumerate() {
        let a = digit(group[0])?;
        let b = digit(group[1])?;
        output.push((a << 2) | (b >> 4));
        if group[2] == b'=' {
            if group[3] != b'=' || index + 1 != groups || b & 15 != 0 {
                return Err("Noncanonical PEM base64 padding.".into());
            }
        } else {
            let c = digit(group[2])?;
            output.push((b << 4) | (c >> 2));
            if group[3] == b'=' {
                if index + 1 != groups || c & 3 != 0 {
                    return Err("Noncanonical PEM base64 padding.".into());
                }
            } else {
                output.push((c << 6) | digit(group[3])?);
            }
        }
        if output.len() > MAX_CERT_BYTES {
            return Err("A PEM certificate exceeds 64 KiB.".into());
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_rejects_noncanonical_lengths_integers_bits_and_sets() {
        for invalid in [
            &[0x30, 0x80, 0, 0][..],
            &[0x30, 0x81, 0][..],
            &[0x30, 0x82, 0, 128][..],
            &[0x02, 0][..],
            &[0x02, 2, 0, 1][..],
            &[0x03, 2, 7, 1][..],
            &[0x01, 1, 1][..],
            &[0x06, 2, 0x80, 1][..],
            &[0x06, 1, 0x81][..],
            &[0x31, 6, 0x02, 1, 2, 0x02, 1, 1][..],
            &[0x30, 0, 0][..],
        ] {
            assert!(validate_der(invalid).is_err(), "{invalid:02x?}");
        }
        assert_eq!(unsigned_integer(&[0, 0x80]).unwrap(), [0x80]);
        assert!(unsigned_integer(&[0x80]).is_err());
        assert!(validate_der(&[0x31, 6, 0x02, 1, 1, 0x02, 1, 2]).is_ok());
    }

    #[test]
    fn certificate_times_handle_leap_days_year_pivot_and_epoch() {
        let date = |tag, value: &'static [u8]| {
            parse_date(Tlv {
                tag,
                value,
                full: &[],
            })
        };
        assert_eq!(date(0x17, b"700101000000Z").unwrap().seconds, 0);
        assert_eq!(
            date(0x17, b"500101000000Z").unwrap().display,
            "1950-01-01 00:00:00 UTC"
        );
        assert_eq!(
            date(0x17, b"491231235959Z").unwrap().display,
            "2049-12-31 23:59:59 UTC"
        );
        assert!(date(0x18, b"20000229000000Z").is_ok());
        assert!(date(0x18, b"21000229000000Z").is_err());
        assert!(date(0x17, b"260231000000Z").is_err());
        assert!(date(0x17, b"261101246000Z").is_err());
        assert!(date(0x17, b"261101000000+").is_err());
    }

    #[test]
    fn san_matching_never_uses_cn_or_dns_spelling_for_ip_addresses() {
        let mut extensions = Extensions {
            san_present: true,
            ..Extensions::default()
        };
        extensions.identities.push(Identity::Dns("*.example.test"));
        extensions
            .identities
            .push(Identity::Dns("EXACT.example.test"));
        extensions.identities.push(Identity::Dns("127.0.0.1"));
        assert!(match_hostname(&extensions, "www.example.test").is_ok());
        assert!(match_hostname(&extensions, "EXACT.example.test.").is_ok());
        assert!(match_hostname(&extensions, "example.test").is_err());
        assert!(match_hostname(&extensions, "deep.www.example.test").is_err());
        assert!(match_hostname(&extensions, "127.0.0.1").is_err());
        assert!(match_hostname(&extensions, "127.0.0.1.").is_err());
        extensions.identities.push(Identity::Ip(&[127, 0, 0, 1]));
        assert!(match_hostname(&extensions, "127.0.0.1").is_ok());
        assert!(match_hostname(&extensions, "127.0.0.1.").is_ok());
        assert!(match_hostname(&Extensions::default(), "exact.example.test").is_err());
        for invalid in [
            "*example.test",
            "x.*.test",
            "*.test",
            "bad..test",
            "good.test\0evil",
            "éxample.test",
        ] {
            assert!(validate_dns_name(invalid, true).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn pem_base64_padding_boundaries_and_object_types_are_strict() {
        assert_eq!(decode_base64(b"TQ==").unwrap(), b"M");
        assert_eq!(decode_base64(b"TWE=").unwrap(), b"Ma");
        assert_eq!(decode_base64(b"TWFu").unwrap(), b"Man");
        for invalid in [
            b"TR==".as_slice(),
            b"TWF=".as_slice(),
            b"=AAA".as_slice(),
            b"TQ==AAAA".as_slice(),
            b"TQ=".as_slice(),
        ] {
            assert!(decode_base64(invalid).is_err());
        }
        let bundle = b"# explicit test data\r\n-----BEGIN CERTIFICATE-----\r\nMAA=\r\n-----END CERTIFICATE-----\r\n";
        assert_eq!(parse_pem_certificates(bundle).unwrap(), vec![vec![0x30, 0]]);
        assert!(parse_pem_certificates(
            b"-----BEGIN PRIVATE KEY-----\nMAA=\n-----END PRIVATE KEY-----"
        )
        .is_err());
        assert!(parse_pem_certificates(b"-----BEGIN CERTIFICATE-----\nMAA=\n").is_err());
        assert!(parse_pem_certificates(&bundle.repeat(MAX_ROOTS + 1)).is_err());
    }

    #[test]
    fn shipped_public_anchors_have_valid_signatures_and_ca_constraints() {
        let roots = parse_pem_certificates(include_bytes!("../trust/roots.pem")).unwrap();
        let expected = [
            ("96:BC:EC:06:26:49:76:F3:74:60:77:9A:CF:28:C5:A7:CF:E8:A3:C0:AA:E1:1A:8F:FC:EE:05:C0:BD:DF:08:C6", 512, "RSA"),
            ("CB:3C:CB:B7:60:31:E5:E0:13:8F:8D:D3:9A:23:F9:DE:47:FF:C3:5E:43:C1:14:4C:EA:27:D4:6A:5A:B1:CB:5F", 256, "RSA"),
            ("8E:CD:E6:88:4F:3D:87:B1:12:5B:A3:1A:C3:FC:B1:3D:70:16:DE:7F:57:CC:90:4F:E1:CB:97:C6:AE:98:19:6E", 256, "RSA"),
            ("69:72:9B:8E:15:A8:6E:FC:17:7A:57:AF:B7:17:1D:FC:64:AD:D2:8C:2F:CA:8C:F1:50:7E:34:45:3C:CB:14:70", 97, "ECDSA P-384"),
            ("D9:47:43:2A:BD:E7:B7:FA:90:FC:2E:6B:59:10:1B:12:80:E0:E1:C7:E4:E4:0F:A3:C6:88:7F:FF:57:A7:F4:CF", 512, "RSA"),
            ("34:D8:A7:3E:E2:08:D9:BC:DB:0D:95:65:20:93:4B:4E:40:E6:94:82:59:6E:8B:6F:73:C8:42:6B:01:0A:6F:48", 97, "ECDSA P-384"),
            ("34:9D:FA:40:58:C5:E2:63:12:3B:39:8A:E7:95:57:3C:4E:13:13:C8:3F:E6:8F:93:55:6C:D5:E8:03:1B:3C:7D", 97, "ECDSA P-384"),
        ];
        assert_eq!(roots.len(), expected.len());
        for (der, (expected_fingerprint, key_bytes, key_kind)) in roots.iter().zip(expected) {
            let root = parse_certificate(der).unwrap();
            assert_eq!(fingerprint(&root.fingerprint), expected_fingerprint);
            assert_eq!(root.key.description(), key_kind);
            let actual_bytes = match root.key {
                PublicKeyRef::Rsa { modulus, .. } => modulus.len(),
                PublicKeyRef::Ec { point, .. } => point.len(),
            };
            assert_eq!(actual_bytes, key_bytes);
            check_anchor(&root, 0, 1_794_182_400).unwrap(); // 2026-11-09 UTC
            println!(
                "Trusted anchor: {}, {:?}, {} through {}, {}",
                key_kind,
                root.algorithm,
                root.before.display,
                root.after.display,
                expected_fingerprint
            );
        }
    }

    #[test]
    fn signed_demo_path_requires_identity_time_signature_order_and_exact_trust() {
        let roots = parse_pem_certificates(include_bytes!("../tools/https-demo/ca.pem")).unwrap();
        let chain =
            parse_pem_certificates(include_bytes!("../tools/https-demo/server-chain.pem")).unwrap();
        let now = UNIX_EPOCH + std::time::Duration::from_secs(1_794_182_400);
        assert_eq!(chain.len(), 2);
        for name in ["localhost", "LOCALHOST.", "127.0.0.1"] {
            let server = verify_server(&chain, &roots, name, now).unwrap();
            assert!(matches!(server.key, PublicKey::Rsa { modulus, .. } if modulus.len() == 256));
            assert!(server.summary.contains("Trusted root SHA-256:"));
        }
        let mut full = chain.clone();
        full.extend(roots.iter().cloned());
        assert!(verify_server(&full, &roots, "localhost", now).is_ok());
        assert!(verify_server(&chain, &roots, "wrong.test", now).is_err());
        assert!(verify_server(&chain, &roots, "localhost", UNIX_EPOCH).is_err());
        let late = UNIX_EPOCH + std::time::Duration::from_secs(2_524_608_000);
        assert!(verify_server(&chain, &roots, "localhost", late).is_err());
        let public_roots = parse_pem_certificates(include_bytes!("../trust/roots.pem")).unwrap();
        assert!(verify_server(&chain, &public_roots, "localhost", now).is_err());
        let mut altered = chain.clone();
        *altered[0].last_mut().unwrap() ^= 1;
        assert!(verify_server(&altered, &roots, "localhost", now).is_err());
        let mut duplicate = chain.clone();
        duplicate.push(chain[1].clone());
        assert!(verify_server(&duplicate, &roots, "localhost", now).is_err());
        let mut reversed = chain.clone();
        reversed.reverse();
        assert!(verify_server(&reversed, &roots, "localhost", now).is_err());
        let leaf = &chain[0];
        for length in 0..leaf.len() {
            assert!(parse_certificate(&leaf[..length]).is_err());
        }
        for index in 0..leaf.len() {
            let mut malformed = leaf.clone();
            malformed[index] ^= 0xff;
            // Content mutations can remain syntactically valid; the parser
            // must return a result without panicking for every byte position.
            let _ = parse_certificate(&malformed);
        }
    }

    fn encode(tag: u8, value: &[u8]) -> Vec<u8> {
        let mut result = vec![tag];
        if value.len() < 128 {
            result.push(value.len() as u8);
        } else if value.len() <= 255 {
            result.extend_from_slice(&[0x81, value.len() as u8]);
        } else {
            result.extend_from_slice(&[0x82, (value.len() >> 8) as u8, value.len() as u8]);
        }
        result.extend_from_slice(value);
        result
    }

    fn unhex(text: &str) -> Vec<u8> {
        text.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn ecdsa_signature_identifiers_require_absent_parameters() {
        for (oid, hash) in [
            (ECDSA_SHA256, Hash::Sha256),
            (ECDSA_SHA384, Hash::Sha384),
            (ECDSA_SHA512, Hash::Sha512),
        ] {
            let encoded_oid = encode(0x06, oid);
            let algorithm = encode(0x30, &encoded_oid);
            assert_eq!(
                signature_algorithm(single(&algorithm, 0x30).unwrap()).unwrap(),
                Signature::Ecdsa(hash)
            );
            for parameters in [
                encode(0x05, &[]),
                encode(0x30, &[]),
                encode(0x06, PRIME256V1),
            ] {
                let mut contents = encoded_oid.clone();
                contents.extend_from_slice(&parameters);
                let malformed = encode(0x30, &contents);
                assert!(signature_algorithm(single(&malformed, 0x30).unwrap()).is_err());
            }
        }
        // SHA-224, SHA-1, and a made-up suffix must not select a nearby hash.
        for suffix in [0, 1, 5, 0xff] {
            let mut oid = ECDSA_SHA256.to_vec();
            *oid.last_mut().unwrap() = suffix;
            let algorithm = encode(0x30, &encode(0x06, &oid));
            assert!(signature_algorithm(single(&algorithm, 0x30).unwrap()).is_err());
        }
    }

    #[test]
    fn ec_spki_requires_named_supported_curves_and_valid_uncompressed_points() {
        // Standard curve base points (private scalar 1), also checked by the
        // independent development oracle against its own EC implementation.
        let p256 = unhex(concat!(
            "04",
            "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
            "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
        ));
        let p384 = unhex(concat!(
            "04", "aa87ca22be8b05378eb1c71ef320ad746e1d3b628ba79b9859f741e082542a385502f25dbf55296c3a545e3872760ab7",
            "3617de4a96262c6f5d9e98bf9292dc29f8f41dbd289a147ce9da3113b5f0b8c00a60b1ce1d7e819d7a431d7c90ea0e5f"));
        let spki = |parameters: &[u8], point: &[u8], unused: u8| {
            let mut algorithm = encode(0x06, EC_PUBLIC_KEY);
            algorithm.extend_from_slice(parameters);
            let mut contents = encode(0x30, &algorithm);
            let mut bits = vec![unused];
            bits.extend_from_slice(point);
            contents.extend_from_slice(&encode(0x03, &bits));
            encode(0x30, &contents)
        };
        for (oid, curve, point) in [
            (PRIME256V1, Curve::P256, p256),
            (SECP384R1, Curve::P384, p384),
        ] {
            let parameters = encode(0x06, oid);
            let valid = spki(&parameters, &point, 0);
            assert!(matches!(public_key(single(&valid, 0x30).unwrap()).unwrap(),
                PublicKeyRef::Ec { curve: parsed, point: bytes } if parsed == curve && bytes == point));
            for bad_parameters in [
                Vec::new(),
                encode(0x05, &[]),
                encode(0x30, &[]),
                encode(0x06, &[0x2b, 0x81, 4, 0, 0x23]),
                [parameters.as_slice(), &encode(0x05, &[])].concat(),
            ] {
                let malformed = spki(&bad_parameters, &point, 0);
                assert!(public_key(single(&malformed, 0x30).unwrap()).is_err());
            }
            let coordinate_bytes = (point.len() - 1) / 2;
            let mut compressed = vec![2];
            compressed.extend_from_slice(&point[1..=coordinate_bytes]);
            let mut hybrid = point.clone();
            hybrid[0] = 6;
            let mut zero = vec![0; point.len()];
            zero[0] = 4;
            let mut out_of_field = point.clone();
            out_of_field[1..=coordinate_bytes].fill(0xff);
            for bad_point in [
                vec![0],
                point[..point.len() - 1].to_vec(),
                compressed,
                hybrid,
                zero,
                out_of_field,
            ] {
                let malformed = spki(&parameters, &bad_point, 0);
                assert!(public_key(single(&malformed, 0x30).unwrap()).is_err());
            }
            let unaligned = spki(&parameters, &point, 1);
            assert!(public_key(single(&unaligned, 0x30).unwrap()).is_err());
        }
    }
}
