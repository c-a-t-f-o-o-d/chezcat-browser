#!/usr/bin/env python3
"""Offline signed-certificate policy and algorithm checks.

Run: python tools/check_x509.py [--rustc /path/to/rustc]

Python cryptography and a host Rust compiler are development-only requirements.
All private keys are ephemeral test data; the browser does not import, invoke,
or link this script. Its own X.509, RSA, and ECDSA code evaluate each generated path.
This checks the documented bounded subset, not full RFC 5280 conformance.
"""

import argparse
from datetime import datetime, timezone
import ipaddress
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec, padding, rsa, utils
from cryptography.x509.oid import ExtendedKeyUsageOID, ExtensionOID, NameOID, ObjectIdentifier

ROOT = Path(__file__).resolve().parents[1]
NOW = 1_794_182_400  # 2026-11-09 UTC, independent of the development clock
BEFORE = datetime(2025, 1, 1, tzinfo=timezone.utc)
AFTER = datetime(2035, 1, 1, tzinfo=timezone.utc)

PROBE = r'''
#![allow(dead_code)]
#[path=HASH] mod crypto_hash;
#[path=SHA512] mod crypto_sha512;
#[path=RSA] mod crypto_rsa;
#[path=EC] mod crypto_ec;
#[path=X509] mod x509;
use std::io::{self, BufRead};
fn unhex(s:&str)->Vec<u8> {
    s.as_bytes().as_chunks::<2>().0.iter().map(|b|
        u8::from_str_radix(std::str::from_utf8(b).unwrap(),16).unwrap()).collect()
}
fn main() {
    for line in io::stdin().lock().lines() {
        let line=line.unwrap(); let f:Vec<_>=line.split('\t').collect();
        let certs=|s:&str|->Vec<Vec<u8>> {
            if s.is_empty() {Vec::new()} else {s.split(',').map(unhex).collect()}
        };
        let now=std::time::UNIX_EPOCH+std::time::Duration::from_secs(f[1].parse().unwrap());
        match x509::verify_server(&certs(f[3]),&certs(f[2]),f[0],now) {
            Ok(server)=>println!("OK {}",match server.key {
                x509::PublicKey::Rsa{..}=>"RSA",
                x509::PublicKey::Ec{curve:crypto_ec::Curve::P256,..}=>"P256",
                x509::PublicKey::Ec{curve:crypto_ec::Curve::P384,..}=>"P384",
            }),
            Err(error)=>println!("ERR {error}"),
        }
    }
}
'''


def usage(*, digital=False, signing=False):
    return x509.KeyUsage(
        digital_signature=digital, content_commitment=False,
        key_encipherment=not digital and not signing, data_encipherment=False,
        key_agreement=False, key_cert_sign=signing, crl_sign=signing,
        encipher_only=False, decipher_only=False,
    )


def issue(subject, public, issuer, signer, extensions, *,
          algorithm=None, rsa_padding=None, before=BEFORE, after=AFTER):
    builder = (x509.CertificateBuilder().subject_name(subject).issuer_name(issuer)
               .public_key(public).serial_number(x509.random_serial_number())
               .not_valid_before(before).not_valid_after(after))
    for value, critical in extensions:
        builder = builder.add_extension(value, critical)
    return builder.sign(signer, algorithm or hashes.SHA256(), rsa_padding=rsa_padding)


def replace(extensions, oid, value, critical=False):
    result = [(v, c) for v, c in extensions if v.oid != oid]
    if value is not None:
        result.append((value, critical))
    return result


def der(certificate):
    return certificate if isinstance(certificate, bytes) else certificate.public_bytes(serialization.Encoding.DER)


def tlv(tag, value):
    size = len(value)
    length = bytes([size]) if size < 128 else size.to_bytes((size.bit_length() + 7) // 8, "big")
    if size >= 128:
        length = bytes([0x80 | len(length)]) + length
    return bytes([tag]) + length + value


def children(sequence):
    """Small development-only DER splitter for intentional mutation tests."""
    count = sequence[1] & 0x7f if sequence[1] & 0x80 else 0
    offset = 2 + count
    result = []
    while offset < len(sequence):
        start = offset
        first = sequence[offset + 1]
        count = first & 0x7f if first & 0x80 else 0
        size = int.from_bytes(sequence[offset + 2:offset + 2 + count], "big") if count else first
        offset += 2 + count + size
        result.append(sequence[start:offset])
    assert offset == len(sequence)
    return result


def resign_tbs(certificate, signer, change, *, algorithm=None, signature_algorithm=None):
    outer = children(der(certificate))
    body = children(outer[0])
    body = change(body)
    if signature_algorithm is not None:
        body[2] = signature_algorithm
        outer[1] = signature_algorithm
    tbs = tlv(0x30, b"".join(body))
    algorithm = algorithm or hashes.SHA256()
    if isinstance(signer, ec.EllipticCurvePrivateKey):
        signature = signer.sign(tbs, ec.ECDSA(algorithm))
    else:
        signature = signer.sign(tbs, padding.PKCS1v15(), algorithm)
    return tlv(0x30, tbs + outer[1] + tlv(0x03, b"\x00" + signature))


def run(rustc):
    started = time.monotonic()
    key = lambda: rsa.generate_private_key(public_exponent=65537, key_size=2048)
    root_key, issuer_key, leaf_key = key(), key(), key()
    name = lambda text: x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, text)])
    root_name, issuer_name, leaf_name = name("Generated root"), name("Generated issuer"), name("localhost")
    root_extensions = [
        (x509.BasicConstraints(ca=True, path_length=1), True),
        (usage(signing=True), True),
        (x509.SubjectKeyIdentifier.from_public_key(root_key.public_key()), False),
        (x509.AuthorityKeyIdentifier.from_issuer_public_key(root_key.public_key()), False),
    ]
    issuer_extensions = [
        (x509.BasicConstraints(ca=True, path_length=0), True),
        (usage(signing=True), True),
        (x509.SubjectKeyIdentifier.from_public_key(issuer_key.public_key()), False),
        (x509.AuthorityKeyIdentifier.from_issuer_public_key(root_key.public_key()), False),
    ]
    leaf_extensions = [
        (x509.BasicConstraints(ca=False, path_length=None), True),
        (usage(digital=True), True),
        (x509.AuthorityKeyIdentifier.from_issuer_public_key(issuer_key.public_key()), False),
        (x509.SubjectAlternativeName([
            x509.DNSName("localhost"), x509.DNSName("*.example.test"),
            x509.IPAddress(ipaddress.ip_address("127.0.0.1")),
            x509.IPAddress(ipaddress.ip_address("::1")),
        ]), False),
        (x509.ExtendedKeyUsage([ExtendedKeyUsageOID.SERVER_AUTH]), False),
    ]

    def root_cert(ext=None, **kwargs):
        return issue(root_name, root_key.public_key(), root_name, root_key,
                     root_extensions if ext is None else ext, **kwargs)

    def issuer_cert(ext=None, **kwargs):
        return issue(issuer_name, issuer_key.public_key(), root_name, root_key,
                     issuer_extensions if ext is None else ext, **kwargs)

    def leaf_cert(ext=None, **kwargs):
        return issue(leaf_name, leaf_key.public_key(), issuer_name, issuer_key,
                     leaf_extensions if ext is None else ext, **kwargs)

    anchor, issuer, leaf = root_cert(), issuer_cert(), leaf_cert()
    cases = []

    def add(label, expected, *, cert=leaf, ca=issuer, roots=None, host="localhost", now=NOW, chain=None,
            key_type="RSA", error_contains=None):
        chain = [cert, ca] if chain is None else chain
        roots = [anchor] if roots is None else roots
        encoded = "\t".join([host, str(now), ",".join(der(c).hex() for c in roots),
                             ",".join(der(c).hex() for c in chain)])
        cases.append((label, expected, encoded, key_type, error_contains))

    add("ordered valid chain", True)
    add("explicit root in server chain", True, chain=[leaf, issuer, anchor])
    for host in ["LOCALHOST.", "127.0.0.1", "::1", "www.example.test"]:
        add("SAN accepted " + host, True, host=host)
    for host in ["wrong.test", "example.test", "deep.www.example.test", "127.0.0.2", "::2"]:
        add("SAN rejected " + host, False, host=host)
    add("SAN absent does not use matching CN", False,
        cert=leaf_cert(replace(leaf_extensions, ExtensionOID.SUBJECT_ALTERNATIVE_NAME, None)))
    dns_only = replace(leaf_extensions, ExtensionOID.SUBJECT_ALTERNATIVE_NAME,
                       x509.SubjectAlternativeName([x509.DNSName("127.0.0.1")]))
    add("DNS SAN cannot assert IP identity", False, cert=leaf_cert(dns_only), host="127.0.0.1.")
    add("root absent", False, roots=[])
    add("wrong trust anchor", False, roots=[issuer])
    add("server chain empty", False, chain=[])
    add("server chain reversed", False, chain=[issuer, leaf])
    add("duplicate issuer", False, chain=[leaf, issuer, issuer])
    add("root is not leaf", False, chain=[anchor])
    add("before validity", False, now=0)
    add("after validity", False, now=2_524_608_000)
    old = datetime(2026, 1, 1, tzinfo=timezone.utc)
    add("expired leaf", False, cert=leaf_cert(after=old))
    add("expired issuer", False, ca=issuer_cert(after=old))
    add("expired anchor", False, roots=[root_cert(after=old)])
    future = datetime(2027, 1, 1, tzinfo=timezone.utc)
    add("future leaf", False, cert=leaf_cert(before=future))

    # A server can send a cross-sign of a root the client already trusts.
    # The selected path ends at the stored root, before that optional suffix;
    # the suffix must not force trust in its older issuing CA. These are all
    # independently signed certificates, not names-only synthetic structures.
    legacy_key = key()
    legacy_name = name("Legacy root absent from the default store")
    legacy_extensions = [
        (x509.BasicConstraints(ca=True, path_length=2), True),
        (usage(signing=True), True),
        (x509.SubjectKeyIdentifier.from_public_key(legacy_key.public_key()), False),
        (x509.AuthorityKeyIdentifier.from_issuer_public_key(legacy_key.public_key()), False),
    ]
    legacy_root = issue(legacy_name, legacy_key.public_key(), legacy_name,
                        legacy_key, legacy_extensions)
    cross_extensions = replace(root_extensions, ExtensionOID.AUTHORITY_KEY_IDENTIFIER,
                               x509.AuthorityKeyIdentifier.from_issuer_public_key(legacy_key.public_key()))

    def cross_cert(ext=None, **kwargs):
        return issue(root_name, root_key.public_key(), legacy_name, legacy_key,
                     cross_extensions if ext is None else ext, **kwargs)

    cross = cross_cert()
    add("trusted prefix before cross-signed root", True, chain=[leaf, issuer, cross])
    add("trusted prefix before cross-sign and legacy root", True,
        chain=[leaf, issuer, cross, legacy_root])
    add("cross-sign can instead lead to the explicitly trusted legacy root", True,
        roots=[legacy_root], chain=[leaf, issuer, cross])
    add("expired unused cross-sign does not invalidate trusted prefix", True,
        chain=[leaf, issuer, cross_cert(after=old)])
    add("unrelated unused suffix does not extend trusted prefix", True,
        chain=[leaf, issuer, legacy_root])
    add("unused cross-sign constraints do not replace stored root constraints", True,
        chain=[leaf, issuer, cross_cert(replace(cross_extensions, ExtensionOID.BASIC_CONSTRAINTS,
                                             x509.BasicConstraints(ca=True, path_length=0), True))])
    add("untrusted cross-sign still needs its actual issuer", False,
        roots=[issuer], chain=[leaf, issuer, cross])
    add("expired required cross-sign remains invalid", False,
        roots=[legacy_root], chain=[leaf, issuer, cross_cert(after=old)])
    add("cross-sign cannot evade stored root expiry", False,
        roots=[root_cert(after=old)], chain=[leaf, issuer, cross])
    add("cross-sign cannot evade stored root path length", False,
        roots=[root_cert(replace(root_extensions, ExtensionOID.BASIC_CONSTRAINTS,
                                x509.BasicConstraints(ca=True, path_length=0), True))],
        chain=[leaf, issuer, cross])
    add("cross-sign cannot evade intermediate expiry", False,
        chain=[leaf, issuer_cert(after=old), cross])
    add("cross-sign cannot evade hostname verification", False,
        chain=[leaf, issuer, cross], host="wrong.test")
    forged_prefix = bytearray(der(issuer)); forged_prefix[-1] ^= 1
    add("matching root name before suffix still requires valid signature", False,
        chain=[leaf, bytes(forged_prefix), cross])
    disguised_anchor = issue(root_name, legacy_key.public_key(), root_name,
                             legacy_key, root_extensions)
    add("matching root name and identifier before suffix cannot confer trust", False,
        roots=[disguised_anchor], chain=[leaf, issuer, cross])
    add("multiple root candidates may end a prefix", True,
        roots=[root_cert(after=old) for _ in range(7)] + [anchor], chain=[leaf, issuer, cross])
    add("root candidate limit also bounds prefix search", False,
        roots=[root_cert(after=old) for _ in range(8)] + [anchor], chain=[leaf, issuer, cross],
        error_contains="Too many matching root candidates")

    no_ca = replace(issuer_extensions, ExtensionOID.BASIC_CONSTRAINTS,
                    x509.BasicConstraints(ca=False, path_length=None), True)
    no_ca = replace(no_ca, ExtensionOID.KEY_USAGE, usage(digital=True), True)
    add("issuer CA false", False, ca=issuer_cert(no_ca))
    add("issuer missing basic constraints", False,
        ca=issuer_cert(replace(issuer_extensions, ExtensionOID.BASIC_CONSTRAINTS, None)))
    add("issuer basic constraints noncritical", False,
        ca=issuer_cert(replace(issuer_extensions, ExtensionOID.BASIC_CONSTRAINTS,
                               x509.BasicConstraints(ca=True, path_length=0))))
    add("issuer KU lacks keyCertSign", False,
        ca=issuer_cert(replace(issuer_extensions, ExtensionOID.KEY_USAGE, usage(digital=True), True)))
    add("leaf KU lacks digitalSignature", False,
        cert=leaf_cert(replace(leaf_extensions, ExtensionOID.KEY_USAGE, usage(), True)))
    add("leaf keyCertSign without CA", False,
        cert=leaf_cert(replace(leaf_extensions, ExtensionOID.KEY_USAGE, usage(digital=True, signing=True), True)))
    client_only = x509.ExtendedKeyUsage([ExtendedKeyUsageOID.CLIENT_AUTH])
    add("leaf EKU client-only", False,
        cert=leaf_cert(replace(leaf_extensions, ExtensionOID.EXTENDED_KEY_USAGE, client_only)))
    add("issuer EKU client-only", False,
        ca=issuer_cert(replace(issuer_extensions, ExtensionOID.EXTENDED_KEY_USAGE, client_only)))
    add("root pathLen zero excludes intermediate", False,
        roots=[root_cert(replace(root_extensions, ExtensionOID.BASIC_CONSTRAINTS,
                                x509.BasicConstraints(ca=True, path_length=0), True))])
    add("leaf marked CA", False,
        cert=leaf_cert(replace(leaf_extensions, ExtensionOID.BASIC_CONSTRAINTS,
                               x509.BasicConstraints(ca=True, path_length=None), True)))
    add("issuer SKI mismatch", False,
        ca=issuer_cert(replace(issuer_extensions, ExtensionOID.SUBJECT_KEY_IDENTIFIER,
                               x509.SubjectKeyIdentifier(bytes(20)))))

    for extension in [
        x509.NameConstraints(permitted_subtrees=[x509.DNSName("localhost")], excluded_subtrees=None),
        x509.PolicyConstraints(require_explicit_policy=0, inhibit_policy_mapping=None),
        x509.InhibitAnyPolicy(0),
        x509.UnrecognizedExtension(ObjectIdentifier("2.5.29.33"), b"\x30\x00"),
        x509.TLSFeature([x509.TLSFeatureType.status_request]),
    ]:
        add("unsupported noncritical " + extension.oid.dotted_string, False,
            ca=issuer_cert(issuer_extensions + [(extension, False)]))
    unknown = x509.UnrecognizedExtension(ObjectIdentifier("1.2.3.4.5"), b"\x05\x00")
    add("unknown critical extension", False, cert=leaf_cert(leaf_extensions + [(unknown, True)]))
    add("unknown noncritical metadata", True, cert=leaf_cert(leaf_extensions + [(unknown, False)]))

    for algorithm in [hashes.SHA256(), hashes.SHA384(), hashes.SHA512()]:
        add("PKCS1 " + algorithm.name, True, cert=leaf_cert(algorithm=algorithm),
            ca=issuer_cert(algorithm=algorithm), roots=[root_cert(algorithm=algorithm)])
        for salt in [0, 20, algorithm.digest_size]:
            pad = padding.PSS(mgf=padding.MGF1(algorithm), salt_length=salt)
            add(f"PSS {algorithm.name} salt={salt}", True,
                cert=leaf_cert(algorithm=algorithm, rsa_padding=pad),
                ca=issuer_cert(algorithm=algorithm, rsa_padding=pad),
                roots=[root_cert(algorithm=algorithm, rsa_padding=pad)])
    pad = padding.PSS(mgf=padding.MGF1(hashes.SHA384()), salt_length=32)
    add("PSS unsupported distinct MGF hash", False,
        cert=leaf_cert(algorithm=hashes.SHA256(), rsa_padding=pad))

    forged = bytearray(der(leaf)); forged[-1] ^= 1
    add("altered certificate signature", False, cert=bytes(forged))
    outer = children(der(leaf))
    bad_algorithm = outer[1].replace(bytes.fromhex("2a864886f70d01010b"),
                                   bytes.fromhex("2a864886f70d01010c"))
    add("inner outer algorithm disagreement", False,
        cert=tlv(0x30, outer[0] + bad_algorithm + outer[2]))

    def duplicate_extension(body):
        ext = children(body[-1])[0]
        values = children(ext)
        body[-1] = tlv(0xa3, tlv(0x30, b"".join(values + [values[0]])))
        return body

    add("signed duplicate extension", False, cert=resign_tbs(leaf, issuer_key, duplicate_extension))
    add("signed unsupported v1 certificate", False,
        cert=resign_tbs(leaf, issuer_key, lambda body: body[1:]))
    bad_root_key = key()
    same_name_root = issue(root_name, bad_root_key.public_key(), root_name,
                           bad_root_key, root_extensions)
    add("same root name cannot confer trust", False, roots=[same_name_root])

    # Every root/issuer/leaf key-type combination is signed independently by
    # cryptography. A leaf's key type is independent of its certificate's
    # signature algorithm; that algorithm follows the issuer's key instead.
    keys = {
        "RSA": (root_key, issuer_key, leaf_key),
        "P256": tuple(ec.generate_private_key(ec.SECP256R1()) for _ in range(3)),
        "P384": tuple(ec.generate_private_key(ec.SECP384R1()) for _ in range(3)),
    }

    def mixed_path(root_type, issuer_type, leaf_type, *, algorithm=None):
        rk, ik, lk = keys[root_type][0], keys[issuer_type][1], keys[leaf_type][2]
        rext = replace(root_extensions, ExtensionOID.SUBJECT_KEY_IDENTIFIER,
                       x509.SubjectKeyIdentifier.from_public_key(rk.public_key()))
        rext = replace(rext, ExtensionOID.AUTHORITY_KEY_IDENTIFIER,
                       x509.AuthorityKeyIdentifier.from_issuer_public_key(rk.public_key()))
        iext = replace(issuer_extensions, ExtensionOID.SUBJECT_KEY_IDENTIFIER,
                       x509.SubjectKeyIdentifier.from_public_key(ik.public_key()))
        iext = replace(iext, ExtensionOID.AUTHORITY_KEY_IDENTIFIER,
                       x509.AuthorityKeyIdentifier.from_issuer_public_key(rk.public_key()))
        lext = replace(leaf_extensions, ExtensionOID.AUTHORITY_KEY_IDENTIFIER,
                       x509.AuthorityKeyIdentifier.from_issuer_public_key(ik.public_key()))
        root_hash = algorithm or (hashes.SHA384() if root_type == "P384" else hashes.SHA256())
        leaf_hash = algorithm or (hashes.SHA384() if issuer_type == "P384" else hashes.SHA256())
        rcert = issue(root_name, rk.public_key(), root_name, rk, rext, algorithm=root_hash)
        icert = issue(issuer_name, ik.public_key(), root_name, rk, iext, algorithm=root_hash)
        lcert = issue(leaf_name, lk.public_key(), issuer_name, ik, lext, algorithm=leaf_hash)
        return lcert, icert, rcert

    mixed = {}
    for root_type in keys:
        for issuer_type in keys:
            for leaf_type in keys:
                path = mixed_path(root_type, issuer_type, leaf_type)
                mixed[root_type, issuer_type, leaf_type] = path
                label = f"mixed path root={root_type} issuer={issuer_type} leaf={leaf_type}"
                add(label, True, cert=path[0], ca=path[1], roots=[path[2]], key_type=leaf_type)

    def add_path(label, expected, path, *, key_type, **kwargs):
        add(label, expected, cert=path[0], ca=path[1], roots=[path[2]], key_type=key_type, **kwargs)

    for kind in ["P256", "P384"]:
        for algorithm in [hashes.SHA256(), hashes.SHA384(), hashes.SHA512()]:
            path = mixed_path(kind, kind, kind, algorithm=algorithm)
            add_path(f"ECDSA {kind} all certificates {algorithm.name}", True, path, key_type=kind)
        path = mixed[kind, kind, kind]
        add_path(f"ECDSA {kind} exact root included", True, path, key_type=kind,
                 chain=list(path))
        add_path(f"ECDSA {kind} wrong hostname", False, path, key_type=kind, host="other.test")
        add_path(f"ECDSA {kind} after validity", False, path, key_type=kind, now=2_524_608_000)
        corrupted = bytearray(der(path[0])); corrupted[-1] ^= 1
        add_path(f"ECDSA {kind} forged signature", False, (bytes(corrupted), *path[1:]), key_type=kind)

        # Sign malformed SPKI encodings with the real RSA issuer, so a parser
        # rejection cannot be mistaken for the effect of an invalid signature.
        ec_leaf = issue(leaf_name, keys[kind][2].public_key(), issuer_name, issuer_key, leaf_extensions)
        original_spki = children(children(der(ec_leaf))[0])[6]
        spki_fields = children(original_spki)
        key_algorithm = children(spki_fields[0])
        point = keys[kind][2].public_key().public_bytes(
            serialization.Encoding.X962, serialization.PublicFormat.UncompressedPoint)
        compressed = keys[kind][2].public_key().public_bytes(
            serialization.Encoding.X962, serialization.PublicFormat.CompressedPoint)

        def spki_mutation(label, algorithm_bytes=None, bits=None, extra=b""):
            fields = [algorithm_bytes or spki_fields[0],
                      tlv(0x03, b"\x00" + point) if bits is None else bits]
            changed = tlv(0x30, b"".join(fields) + extra)
            def change(body):
                body[6] = changed
                return body
            add(f"signed {kind} SPKI {label}", False,
                cert=resign_tbs(ec_leaf, issuer_key, change), key_type=kind)

        parameters = [
            ("missing curve", b""), ("NULL implicit curve", tlv(0x05, b"")),
            ("explicit curve parameters", tlv(0x30, b"")),
            ("unsupported P521", tlv(0x06, bytes.fromhex("2b81040023"))),
            ("wrong parameter type", tlv(0x02, b"\x01")),
            ("extra NULL", key_algorithm[1] + tlv(0x05, b"")),
            ("nonminimal curve OID", tlv(0x06, b"\x80\x2b\x81\x04\x00\x22")),
            ("wrong supported curve", tlv(0x06, bytes.fromhex(
                "2b81040022" if kind == "P256" else "2a8648ce3d030107"))),
        ]
        for label, parameter in parameters:
            spki_mutation(label, tlv(0x30, key_algorithm[0] + parameter))
        spki_mutation("restricted id-ecDH", tlv(0x30,
                      tlv(0x06, bytes.fromhex("2b8104010c")) + key_algorithm[1]))
        spki_mutation("extra public-key field", extra=tlv(0x05, b""))
        coordinate_bytes = (len(point) - 1) // 2
        off_curve = bytearray(point); off_curve[-1] ^= 1
        bad_points = [
            ("compressed point", compressed), ("hybrid point", b"\x06" + point[1:]),
            ("point at infinity", b"\x00"), ("zero point", b"\x04" + bytes(len(point) - 1)),
            ("out-of-field coordinate", b"\x04" + b"\xff" * coordinate_bytes + point[1 + coordinate_bytes:]),
            ("off-curve point", bytes(off_curve)), ("short point", point[:-1]),
            ("trailing point byte", point + b"\x00"), ("nested OCTET STRING", tlv(0x04, point)),
        ]
        for label, bad_point in bad_points:
            spki_mutation(label, bits=tlv(0x03, b"\x00" + bad_point))
        spki_mutation("nonzero unused bits", bits=tlv(0x03, b"\x01" + point[:-1] + bytes([point[-1] & 0xfe])))
        spki_mutation("wrong BIT STRING tag", bits=tlv(0x04, b"\x00" + point))

        # Both signature AlgorithmIdentifiers agree, and ECDSA signs the new
        # TBS bytes correctly. Invalid parameters still have to be rejected.
        signer = keys[kind][1]
        for algorithm, suffix in [(hashes.SHA256(), 2), (hashes.SHA384(), 3), (hashes.SHA512(), 4)]:
            path = mixed_path(kind, kind, kind, algorithm=algorithm)
            oid = tlv(0x06, bytes.fromhex("2a8648ce3d0403") + bytes([suffix]))
            for label, parameter in [("NULL", tlv(0x05, b"")), ("curve OID", key_algorithm[1]),
                                     ("sequence", tlv(0x30, b""))]:
                modified = resign_tbs(path[0], signer, lambda body: body,
                                      algorithm=algorithm, signature_algorithm=tlv(0x30, oid + parameter))
                add_path(f"signed {kind} {algorithm.name} ECDSA parameters {label}", False,
                         (modified, *path[1:]), key_type=kind)

        path = mixed[kind, kind, kind]
        outer = children(der(path[0]))
        signature = path[0].signature
        r, s = utils.decode_dss_signature(signature)
        order = int("ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551" if kind == "P256"
                    else "ffffffffffffffffffffffffffffffffffffffffffffffffc7634d81f4372ddf581a0db248b0a77aecec196accc52973", 16)
        # ECDSA accepts both s representatives; TLS/X.509 does not impose low-S.
        high_s = utils.encode_dss_signature(r, max(s, order - s))
        add_path(f"ECDSA {kind} valid high-S", True,
                 (tlv(0x30, outer[0] + outer[1] + tlv(0x03, b"\x00" + high_s)), *path[1:]), key_type=kind)
        components = children(signature)
        raw_values = [
            ("zero r", utils.encode_dss_signature(0, s)),
            ("zero s", utils.encode_dss_signature(r, 0)),
            ("r equals order", utils.encode_dss_signature(order, s)),
            ("s equals order", utils.encode_dss_signature(r, order)),
            ("negative r", tlv(0x30, tlv(0x02, b"\xff") + components[1])),
            ("redundant r zero", tlv(0x30, tlv(0x02, b"\x00\x01") + components[1])),
            ("extra integer", tlv(0x30, b"".join(components) + tlv(0x02, b"\x01"))),
            ("trailing data", signature + b"\x00"),
            ("truncated DER", signature[:-1]), ("empty DER", b""),
            ("oversized DER", tlv(0x30, tlv(0x02, b"\x01" * 145) + components[1])),
        ]
        for label, malformed in raw_values:
            add_path(f"ECDSA {kind} malformed signature {label}", False,
                     (tlv(0x30, outer[0] + outer[1] + tlv(0x03, b"\x00" + malformed)), *path[1:]), key_type=kind)

        # Preserve the issuer name and identifier while giving it a public key
        # of the wrong algorithm. Parsing succeeds; signature dispatch fails.
        ec_issuer_path = mixed["RSA", kind, "RSA"]
        disguised_rsa = issue(issuer_name, issuer_key.public_key(), root_name, root_key,
                               [(extension.value, extension.critical)
                                for extension in ec_issuer_path[1].extensions])
        add_path(f"ECDSA {kind} cannot use RSA issuer key", False,
                 (ec_issuer_path[0], disguised_rsa, anchor), key_type="RSA",
                 error_contains="does not match its issuer's key type")
        rsa_issuer_path = mixed["RSA", "RSA", kind]
        disguised_ec = issue(issuer_name, keys[kind][1].public_key(), root_name, root_key,
                             issuer_extensions)
        add_path(f"RSA signature cannot use {kind} issuer key", False,
                 (rsa_issuer_path[0], disguised_ec, anchor), key_type=kind,
                 error_contains="does not match its issuer's key type")

        bad_usage = replace(leaf_extensions, ExtensionOID.KEY_USAGE,
                            x509.KeyUsage(True, False, True, False, False, False, False, False, False), True)
        add(f"EC {kind} keyEncipherment usage rejected", False,
            cert=issue(leaf_name, keys[kind][2].public_key(), issuer_name, issuer_key, bad_usage), key_type=kind)

    with tempfile.TemporaryDirectory(prefix="own-browser-x509-") as directory:
        folder = Path(directory)
        probe = PROBE
        for token, filename in [("HASH", "crypto_hash.rs"), ("SHA512", "crypto_sha512.rs"),
                                ("RSA", "crypto_rsa.rs"), ("EC", "crypto_ec.rs"), ("X509", "x509.rs")]:
            probe = probe.replace("path=" + token, "path=" + json.dumps(str(ROOT / "src" / filename)))
        source = folder / "probe.rs"
        source.write_text(probe, encoding="utf-8")
        binary = folder / ("probe.exe" if os.name == "nt" else "probe")
        subprocess.run([rustc, "--edition", "2021", "-O", str(source), "-o", str(binary)], check=True)
        process = subprocess.run([str(binary)], input="\n".join(c[2] for c in cases) + "\n",
                                 text=True, capture_output=True, check=True, timeout=180)
        answers = process.stdout.splitlines()
        if len(answers) != len(cases):
            raise AssertionError(f"Expected {len(cases)} results, got {len(answers)}: {process.stderr}")
        failures = []
        for (label, expected, _, key_type, error_contains), answer in zip(cases, answers):
            if (answer.startswith("OK ") != expected or expected and answer != "OK " + key_type
                    or error_contains is not None and error_contains not in answer):
                failures.append({"case": label, "expected": "OK " + key_type if expected else "ERR", "answer": answer})
        print(json.dumps({"passed": len(cases) - len(failures), "total": len(cases),
                          "elapsed_seconds": round(time.monotonic() - started, 2),
                          "failures": failures}, indent=2))
        if failures:
            raise SystemExit(1)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustc", default=os.environ.get("RUSTC", "rustc"))
    run(parser.parse_args().rustc)
