#!/usr/bin/env python3
"""Independent TLS 1.3 handshake authentication gates over local TCP.

Run from any directory with Python cryptography and a host Rust compiler:
    python tools/check_tls_handshake.py [--rustc /path/to/rustc]

Uses the first-party RSA development fixture plus ephemeral mixed RSA/EC paths.
No public network or production credentials are used. Python cryptography,
hashlib, and hmac independently construct TLS 1.3 messages and protected records;
the browser never imports, invokes, or links these test dependencies. The Rust
probe and executable exist only in a temporary directory and are cleaned up.

All three offered cipher suites are crossed with RSA-PSS SHA-256/384/512 and
ECDSA P-256/SHA-256 or P-384/SHA-384. Each combination tests a valid handshake,
a valid signature over the wrong transcript, and a wrong Finished MAC, with
ordinary and fragmented records and transport I/O. Further valid-AEAD cases
bind the ECDSA signature scheme to the certificate's exact curve. Negative
cases must fail before an authenticated stream, client Finished, or HTTP request.
Valid controls independently verify client Finished, both directions of a
KeyUpdate exchange, HTTP application data, and reciprocal close_notify.
This is a focused regression check, not a general TLS conformance suite.
Protocol reference: https://www.rfc-editor.org/rfc/rfc9846
"""
from pathlib import Path
import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import os
import tempfile
import hmac
import json
import socket
import subprocess
import threading
import traceback
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec, padding, rsa, x25519
from cryptography.hazmat.primitives.ciphers.aead import AESGCM, ChaCha20Poly1305
from cryptography.hazmat.primitives.kdf.hkdf import HKDFExpand
from cryptography.x509.oid import ExtendedKeyUsageOID, NameOID

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT/'tools/https-demo'

PROBE = r'''
#![allow(dead_code)]
#[path=__CRYPTO_HASH__] mod crypto_hash;
#[path=__CRYPTO_SHA512__] mod crypto_sha512;
#[path=__CRYPTO_RSA__] mod crypto_rsa;
#[path=__CRYPTO_EC__] mod crypto_ec;
#[path=__CRYPTO_AEAD__] mod crypto_aead;
#[path=__CRYPTO_AES_GCM__] mod crypto_aes_gcm;
#[path=__CRYPTO_X25519__] mod crypto_x25519;
#[path=__TLS_CRYPTO__] mod tls_crypto;
#[path=__X509__] mod x509;
#[path=__TLS__] mod tls;

use std::io::{Read,Write};
struct Fragmented(std::net::TcpStream,usize,usize);
impl Read for Fragmented {fn read(&mut self,b:&mut[u8])->std::io::Result<usize>{let n=b.len().min(self.1);self.0.read(&mut b[..n])}}
impl Write for Fragmented {fn write(&mut self,b:&[u8])->std::io::Result<usize>{self.0.write(&b[..b.len().min(self.2)])}fn flush(&mut self)->std::io::Result<()>{self.0.flush()}}
fn run()->Result<(),String>{
 let a:Vec<String>=std::env::args().collect();
 let port=a[1].parse::<u16>().map_err(|e|e.to_string())?;
 let roots=x509::parse_pem_certificates(&std::fs::read(&a[2]).map_err(|e|e.to_string())?)?;
 let socket=std::net::TcpStream::connect(("127.0.0.1",port)).map_err(|e|e.to_string())?;
 socket.set_read_timeout(Some(std::time::Duration::from_millis(200))).unwrap();
 socket.set_write_timeout(Some(std::time::Duration::from_millis(200))).unwrap();socket.set_nodelay(true).unwrap();
 let fragmented=a.get(4).is_some_and(|s|s=="fragment");
 let socket=Fragmented(socket,if fragmented{7}else{65536},if fragmented{11}else{65536});
 // The Python checker supplies fresh OS entropy on every invocation.
 let random: [u8;64] = a[3].as_bytes().as_chunks::<2>().0.iter().map(|pair|u8::from_str_radix(std::str::from_utf8(pair).unwrap(),16).unwrap()).collect::<Vec<_>>().try_into().unwrap();
 let deadline=std::time::Instant::now()+std::time::Duration::from_secs(15);
 let mut stream=tls::TlsStream::connect(socket,"localhost",&roots,std::time::SystemTime::now(),random,move||if std::time::Instant::now()<deadline{Ok(())}else{Err("TLS test deadline".into())})?;
 println!("AUTH {}",stream.certificate_summary());
 stream.write_all(b"GET /secure HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").map_err(|e|e.to_string())?;
 let mut body=Vec::new();stream.read_to_end(&mut body).map_err(|e|e.to_string())?;
 stream.close().map_err(|e|e.to_string())?;
 let text=String::from_utf8_lossy(&body);
 if !text.contains("HTTPS_FIXTURE_OK"){return Err(format!("Missing HTTP marker: {}",text.chars().take(100).collect::<String>()));}
 println!("PASS HTTP marker; clean authenticated EOF; {} bytes; fragmented={fragmented}",body.len());Ok(())
}
fn main(){if let Err(e)=run(){eprintln!("ERROR {e}");std::process::exit(1);}}
'''


def u16(number):
    return number.to_bytes(2, "big")


def u24(number):
    return number.to_bytes(3, "big")


def vector(data):
    return u16(len(data)) + data


def extension(kind, data):
    return u16(kind) + vector(data)


def handshake(kind, data):
    return bytes([kind]) + u24(len(data)) + data


@dataclass(frozen=True)
class Suite:
    wire: int
    name: str
    algorithm: type
    key_length: int
    cipher: type

    @property
    def hash_length(self):
        return self.algorithm().digest_size

    def digest(self, data):
        return hashlib.new(self.algorithm().name, data).digest()

    def extract(self, salt, material):
        return hmac.digest(salt, material, self.algorithm().name)

    def expand(self, secret, label, context=b"", length=None):
        length = self.hash_length if length is None else length
        label = b"tls13 " + label
        info = u16(length) + bytes([len(label)]) + label + bytes([len(context)]) + context
        return HKDFExpand(self.algorithm(), length, info).derive(secret)

    def finished(self, secret, transcript):
        return self.extract(self.expand(secret, b"finished"), self.digest(transcript))


SUITES = [
    Suite(0x1303, "CHACHA20_POLY1305_SHA256", hashes.SHA256, 32, ChaCha20Poly1305),
    Suite(0x1301, "AES_128_GCM_SHA256", hashes.SHA256, 16, AESGCM),
    Suite(0x1302, "AES_256_GCM_SHA384", hashes.SHA384, 32, AESGCM),
]


@dataclass(frozen=True)
class SignatureScheme:
    wire: int
    name: str
    key_type: str
    algorithm: type


SIGNATURES = [
    SignatureScheme(0x0804, "RSA_PSS_RSAE_SHA256", "RSA", hashes.SHA256),
    SignatureScheme(0x0805, "RSA_PSS_RSAE_SHA384", "RSA", hashes.SHA384),
    SignatureScheme(0x0806, "RSA_PSS_RSAE_SHA512", "RSA", hashes.SHA512),
    SignatureScheme(0x0403, "ECDSA_SECP256R1_SHA256", "P256", hashes.SHA256),
    SignatureScheme(0x0503, "ECDSA_SECP384R1_SHA384", "P384", hashes.SHA384),
]


class Traffic:
    def __init__(self, suite, secret):
        self.suite = suite
        self.secret = secret
        self.key = suite.expand(secret, b"key", length=suite.key_length)
        self.iv = suite.expand(secret, b"iv", length=12)
        self.sequence = 0

    def nonce(self):
        result = bytes(a ^ b for a, b in zip(self.iv, self.sequence.to_bytes(12, "big")))
        self.sequence += 1
        return result

    def seal(self, kind, content):
        plain = content + bytes([kind])
        header = b"\x17\x03\x03" + u16(len(plain) + 16)
        return header + self.suite.cipher(self.key).encrypt(self.nonce(), plain, header)

    def open(self, wire):
        assert wire[:3] == b"\x17\x03\x03"
        plain = self.suite.cipher(self.key).decrypt(self.nonce(), wire[5:], wire[:5])
        index = len(plain) - 1
        while index >= 0 and plain[index] == 0:
            index -= 1
        assert index >= 0, "TLSInnerPlaintext has no content type"
        return plain[index], plain[:index]

    def updated(self):
        return Traffic(self.suite, self.suite.expand(self.secret, b"traffic upd"))


def exact(stream, length):
    result = b""
    while len(result) < length:
        part = stream.recv(length - len(result))
        if not part:
            raise EOFError()
        result += part
    return result


def receive(stream):
    header = exact(stream, 5)
    length = int.from_bytes(header[3:5], "big")
    assert length <= 16640, "oversized test-peer TLS record"
    return header + exact(stream, length)


def client_parameters(hello):
    assert hello[0] == 1 and len(hello) == int.from_bytes(hello[1:4], "big") + 4
    assert hello[4:6] == b"\x03\x03"
    offset = 4 + 2 + 32
    session_length = hello[offset]
    assert session_length == 0
    offset += 1 + session_length
    suite_length = int.from_bytes(hello[offset:offset + 2], "big")
    offset += 2
    assert suite_length % 2 == 0
    suites = {int.from_bytes(hello[i:i + 2], "big") for i in range(offset, offset + suite_length, 2)}
    offset += suite_length
    assert hello[offset:offset + 2] == b"\x01\x00"
    offset += 2
    length = int.from_bytes(hello[offset:offset + 2], "big")
    offset += 2
    end = offset + length
    assert end == len(hello)
    values = {}
    while offset < end:
        assert offset + 4 <= end
        kind = int.from_bytes(hello[offset:offset + 2], "big")
        size = int.from_bytes(hello[offset + 2:offset + 4], "big")
        value = hello[offset + 4:offset + 4 + size]
        offset += 4 + size
        assert kind not in values and offset <= end
        values[kind] = value
    key_share = values[51]
    assert key_share[:6] == b"\x00\x24\x00\x1d\x00\x20" and len(key_share) == 38
    signature_bytes = values[13]
    assert int.from_bytes(signature_bytes[:2], "big") == len(signature_bytes) - 2
    assert len(signature_bytes) % 2 == 0
    signatures = {int.from_bytes(signature_bytes[i:i + 2], "big")
                  for i in range(2, len(signature_bytes), 2)}
    return key_share[6:], suites, signatures


def ephemeral_identity(directory, key_type):
    # Mixed paths exercise certificate issuer dispatch as part of real TLS:
    # P-384 CA -> RSA issuer -> P-256 leaf, and RSA CA -> P-256 issuer -> P-384 leaf.
    rsa_key = lambda: rsa.generate_private_key(public_exponent=65537, key_size=2048)
    if key_type == "P256":
        root_key, issuer_key, leaf_key = ec.generate_private_key(ec.SECP384R1()), rsa_key(), ec.generate_private_key(ec.SECP256R1())
    else:
        root_key, issuer_key, leaf_key = rsa_key(), ec.generate_private_key(ec.SECP256R1()), ec.generate_private_key(ec.SECP384R1())
    name = lambda label: x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, label)])
    root_name, issuer_name, leaf_name = name("Ephemeral " + key_type + " test root"), name("Ephemeral issuer"), name("localhost")

    def issue(subject, public, issuer, signer, ca, path_length=None):
        builder = (x509.CertificateBuilder().subject_name(subject).issuer_name(issuer)
                   .public_key(public).serial_number(x509.random_serial_number())
                   .not_valid_before(datetime(2025, 1, 1, tzinfo=timezone.utc))
                   .not_valid_after(datetime(2035, 1, 1, tzinfo=timezone.utc))
                   .add_extension(x509.BasicConstraints(ca=ca, path_length=path_length), critical=True)
                   .add_extension(x509.KeyUsage(not ca, False, False, False, False, ca, ca, False, False), critical=True)
                   .add_extension(x509.SubjectKeyIdentifier.from_public_key(public), critical=False)
                   .add_extension(x509.AuthorityKeyIdentifier.from_issuer_public_key(signer.public_key()), critical=False))
        if not ca:
            builder = (builder.add_extension(x509.SubjectAlternativeName([x509.DNSName("localhost")]), critical=False)
                       .add_extension(x509.ExtendedKeyUsage([ExtendedKeyUsageOID.SERVER_AUTH]), critical=False))
        algorithm = hashes.SHA384() if isinstance(signer, ec.EllipticCurvePrivateKey) and signer.curve.key_size == 384 else hashes.SHA256()
        return builder.sign(signer, algorithm)

    root = issue(root_name, root_key.public_key(), root_name, root_key, True, 1)
    issuer = issue(issuer_name, issuer_key.public_key(), root_name, root_key, True, 0)
    leaf = issue(leaf_name, leaf_key.public_key(), issuer_name, issuer_key, False)
    trust_file = directory / (key_type.lower() + "-ca.pem")
    trust_file.write_bytes(root.public_bytes(serialization.Encoding.PEM))
    return {"key": leaf_key, "chain": [leaf, issuer], "trust_file": trust_file}


def sign_and_verify(key, algorithm, message):
    if isinstance(key, rsa.RSAPrivateKey):
        scheme = padding.PSS(padding.MGF1(algorithm), algorithm.digest_size)
        signature = key.sign(message, scheme, algorithm)
        key.public_key().verify(signature, message, scheme, algorithm)
    else:
        signature = key.sign(message, ec.ECDSA(algorithm))
        key.public_key().verify(signature, message, ec.ECDSA(algorithm))
    return signature


def server(listener, suite, scheme, identity, mode, fragmented, result):
    try:
        stream, _ = listener.accept()
        with stream:
            stream.settimeout(5)
            wire = receive(stream)
            assert wire[0] == 22
            hello = wire[5:]
            peer_key, offered_suites, offered_signatures = client_parameters(hello)
            assert suite.wire in offered_suites and scheme.wire in offered_signatures
            assert {item.wire for item in SUITES} <= offered_suites
            assert {item.wire for item in SIGNATURES} <= offered_signatures
            peer = x25519.X25519PublicKey.from_public_bytes(peer_key)
            private = x25519.X25519PrivateKey.generate()
            key = private.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
            extensions = extension(43, b"\x03\x04") + extension(51, b"\x00\x1d" + vector(key))
            sh = handshake(2, b"\x03\x03" + os.urandom(32) + b"\x00" + u16(suite.wire) + b"\x00" + vector(extensions))
            transcript = hello + sh
            zero = bytes(suite.hash_length)
            early = suite.extract(zero, zero)
            secret = suite.extract(suite.expand(early, b"derived", suite.digest(b"")), private.exchange(peer))
            server_hs = suite.expand(secret, b"s hs traffic", suite.digest(transcript))
            client_hs = suite.expand(secret, b"c hs traffic", suite.digest(transcript))
            read_hs, write_hs = Traffic(suite, client_hs), Traffic(suite, server_hs)
            ee = handshake(8, vector(extension(0, b"") + extension(16, b"\x00\x09\x08http/1.1")))
            entries = b""
            for certificate in identity["chain"]:
                der = certificate.public_bytes(serialization.Encoding.DER)
                entries += u24(len(der)) + der + b"\x00\x00"
            certificate = handshake(11, b"\x00" + u24(len(entries)) + entries)
            transcript += ee + certificate
            proof_hash = suite.digest(transcript)
            if mode == "bad_cv":
                proof_hash = bytes([proof_hash[0] ^ 1]) + proof_hash[1:]
            signed = b" " * 64 + b"TLS 1.3, server CertificateVerify\0" + proof_hash
            selected_scheme = scheme
            if mode == "bad_curve":
                selected_scheme = next(item for item in SIGNATURES
                                       if item.key_type == ("P384" if scheme.key_type == "P256" else "P256"))
            # This signature is mathematically valid for the certified key and
            # the selected hash. bad_cv changes only its signed transcript;
            # bad_curve changes the TLS scheme's required curve, not the key.
            signature = sign_and_verify(identity["key"], selected_scheme.algorithm(), signed)
            cv = handshake(15, u16(selected_scheme.wire) + vector(signature))
            transcript += cv
            verify = suite.finished(server_hs, transcript)
            if mode == "bad_finished":
                verify = verify[:-1] + bytes([verify[-1] ^ 1])
            sf = handshake(20, verify)
            flight = ee + certificate + cv + sf
            record = b"\x16\x03\x03" + u16(len(sh)) + sh
            # Fragments split a handshake header and certificate body. Finished
            # ends at the final old-key record boundary, as TLS requires.
            cuts = [1, 4, 37, 503, len(flight)] if fragmented else [len(flight)]
            start = 0
            for end in cuts:
                assert end > start
                record += write_hs.seal(22, flight[start:end])
                start = end
            if fragmented:
                for offset in range(0, len(record), 11):
                    stream.sendall(record[offset:offset + 11])
            else:
                stream.sendall(record)
            result["protected_client_types"] = []
            if mode != "valid":
                while True:
                    try:
                        wire = receive(stream)
                    except (EOFError, ConnectionResetError):
                        break
                    kind, _ = read_hs.open(wire)
                    result["protected_client_types"].append(kind)
                    assert kind == 21, "client sent Finished or application data after failed authentication"
                result["server_pass"] = True
                return
            transcript += sf
            kind, client_finished = read_hs.open(receive(stream))
            assert kind == 22 and client_finished == handshake(20, suite.finished(client_hs, transcript))
            main = suite.extract(suite.expand(secret, b"derived", suite.digest(b"")), zero)
            read_app = Traffic(suite, suite.expand(main, b"c ap traffic", suite.digest(transcript)))
            write_app = Traffic(suite, suite.expand(main, b"s ap traffic", suite.digest(transcript)))
            kind, request = read_app.open(receive(stream))
            assert kind == 23 and request.startswith(b"GET /secure HTTP/1.1\r\n")
            result["http_request_after_auth"] = True
            # The request and reply use each direction's old application keys;
            # following records must use fresh secrets with sequence zero.
            stream.sendall(write_app.seal(22, handshake(24, b"\x01")))
            kind, reply = read_app.open(receive(stream))
            assert kind == 22 and reply == handshake(24, b"\x00")
            read_app, write_app = read_app.updated(), write_app.updated()
            result["key_update"] = True
            body = b"HTTPS_FIXTURE_OK"
            response = (b"HTTP/1.1 200 OK\r\nContent-Length: " + str(len(body)).encode()
                        + b"\r\nConnection: close\r\n\r\n" + body)
            stream.sendall(write_app.seal(23, response) + write_app.seal(21, b"\x01\x00"))
            kind, closed = read_app.open(receive(stream))
            assert kind == 21 and closed == b"\x01\x00"
            result["server_pass"] = True
    except Exception:
        result["server_error"] = traceback.format_exc()


def check(client, identities):
    cases = [(suite, scheme, mode, fragmented)
             for suite in SUITES for scheme in SIGNATURES
             for mode in ["valid", "bad_cv", "bad_finished"] for fragmented in [False, True]]
    cases.extend((suite, scheme, "bad_curve", True)
                 for suite in SUITES for scheme in SIGNATURES if scheme.key_type != "RSA")
    failures = []
    passed = 0
    authenticated = 0
    rejected = 0
    for suite, scheme, mode, fragmented in cases:
        identity = identities[scheme.key_type]
        result = {"suite": suite.name, "signature": scheme.name, "mode": mode, "fragmented": fragmented}
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen()
            listener.settimeout(5)
            worker = threading.Thread(target=server,
                                      args=(listener, suite, scheme, identity, mode, fragmented, result), daemon=True)
            worker.start()
            args = [str(client), str(listener.getsockname()[1]), str(identity["trust_file"]), os.urandom(64).hex()]
            if fragmented:
                args.append("fragment")
            try:
                process = subprocess.run(args, text=True, capture_output=True, timeout=20)
                worker.join(6)
                result.update(returncode=process.returncode, stdout=process.stdout.strip(), stderr=process.stderr.strip())
                assert not worker.is_alive(), "test server did not finish"
                assert result.get("server_pass"), "independent test server failed"
                if mode == "valid":
                    assert process.returncode == 0 and "AUTH " in process.stdout and "PASS HTTP" in process.stdout
                    assert result.get("http_request_after_auth") and result.get("key_update")
                    authenticated += 1
                else:
                    assert process.returncode == 1 and "AUTH " not in process.stdout
                    expected = "Finished" if mode == "bad_finished" else "CertificateVerify failed"
                    assert expected in process.stderr
                    if mode == "bad_curve":
                        assert "does not match" in process.stderr
                    assert not result.get("http_request_after_auth")
                    assert not result["protected_client_types"], "authentication failure emitted protected data"
                    rejected += 1
                passed += 1
            except Exception as error:
                result["check_error"] = str(error)
                failures.append(result)
    print(json.dumps({"passed": passed, "total": len(cases), "cipher_suites": len(SUITES),
                      "signature_schemes": len(SIGNATURES), "authenticated_controls": authenticated,
                      "authentication_rejections": rejected, "failures": failures}, indent=2))
    if failures:
        raise SystemExit(1)


def run(rustc):
    for filename in ["ca.pem", "server-chain.pem", "server-key.pem"]:
        if not (FIXTURE / filename).is_file():
            raise FileNotFoundError(f"Missing first-party HTTPS development fixture: {FIXTURE / filename}")
    source_text = PROBE
    for name in ["crypto_hash", "crypto_sha512", "crypto_rsa", "crypto_ec", "crypto_aead",
                 "crypto_aes_gcm", "crypto_x25519", "tls_crypto", "x509", "tls"]:
        source_text = source_text.replace("__" + name.upper() + "__", json.dumps(str(ROOT / "src" / (name + ".rs")), ensure_ascii=False))
    with tempfile.TemporaryDirectory(prefix="tls-handshake-check-") as temporary:
        temp = Path(temporary)
        source = temp / "probe.rs"
        client = temp / ("probe.exe" if os.name == "nt" else "probe")
        source.write_text(source_text, encoding="utf-8")
        subprocess.run([rustc, "--edition=2021", "-O", "-C", "overflow-checks=yes", str(source), "-o", str(client)],
                       check=True, timeout=60)
        identities = {
            "RSA": {"key": serialization.load_pem_private_key((FIXTURE / "server-key.pem").read_bytes(), password=None),
                    "chain": x509.load_pem_x509_certificates((FIXTURE / "server-chain.pem").read_bytes()),
                    "trust_file": FIXTURE / "ca.pem"},
            "P256": ephemeral_identity(temp, "P256"),
            "P384": ephemeral_identity(temp, "P384"),
        }
        check(client, identities)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustc", default=os.environ.get("RUSTC", "rustc"),
                        help="Host Rust compiler (default: RUSTC or rustc from PATH)")
    run(parser.parse_args().rustc)
    print("Independent TLS handshake authentication checks passed; no browser runtime dependency added.")


if __name__ == "__main__":
    main()
