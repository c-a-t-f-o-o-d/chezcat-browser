#!/usr/bin/env python3
"""Independent, local TLS 1.2 interoperability and authentication checks.

    python tools/check_tls12.py --output /tmp/tls12-report.json

Requires a host Rust compiler and Python's ssl/cryptography packages. These
development oracles are never linked into or invoked by ChezCat. A temporary
Rust probe imports the production HTTP/TLS implementation, uses its OS-random
source, and explicitly trusts only freshly generated test roots. No public
server, scan, user certificate, browser profile, or installed trust store is used.

OpenSSL forces four ECDHE/AES-GCM suites and two ECDHE/RSA/AES-CBC suites with
X25519 and P-256. Actual
HTTP requests exercise GET, a 128 KiB form POST, full reload, and 303/307 redirects.
An independent scripted TLS 1.2 peer verifies EMS and classic master secrets,
CBC with encrypt-then-MAC and MAC-then-encrypt, PRF/key derivation, signatures,
both Finished messages, protected records, and authenticated closure. Negative
cases distinguish rejection before ClientKeyExchange, rejection before HTTP,
and rejection of bad application records after an authenticated handshake.

References: RFC 5246 (TLS 1.2), RFC 5288/5289 (GCM/ECDHE suites), RFC 7627 (EMS),
RFC 5746 (secure renegotiation acknowledgement), RFC 7366 (encrypt-then-MAC),
RFC 8422 (ECC), RFC 8446/9846 (downgrade sentinel and signature scheme codes).
This is a focused independent regression harness, not a TLS conformance audit.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
import hashlib
import hmac
import ipaddress
import json
import os
from pathlib import Path
import socket
import ssl
import subprocess
import tempfile
import threading
import traceback

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec, padding, rsa, x25519
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from cryptography.hazmat.primitives.ciphers.aead import AESGCM
from cryptography.x509.oid import ExtendedKeyUsageOID, NameOID

ROOT = Path(__file__).resolve().parents[1]
MARKER = b"TLS12_FIXTURE_OK"
FORM = b"payload=" + b"x" * (128 * 1024 - len(b"payload="))
MODULES = [
    "http", "url", "compression", "random", "tls", "trust", "x509", "tls_crypto",
    "crypto_hash", "crypto_sha512", "crypto_aead", "crypto_aes_gcm",
    "crypto_x25519", "crypto_rsa", "crypto_ec",
]
PROBE = r'''
use std::io::{Read,Write};
struct Fragmented(std::net::TcpStream,usize,usize);
impl Read for Fragmented {
 fn read(&mut self,b:&mut[u8])->std::io::Result<usize>{let n=b.len().min(self.1);self.0.read(&mut b[..n])}
}
impl Write for Fragmented {
 fn write(&mut self,b:&[u8])->std::io::Result<usize>{self.0.write(&b[..b.len().min(self.2)])}
 fn flush(&mut self)->std::io::Result<()>{self.0.flush()}
}
fn run()->Result<(),String>{
 let a:Vec<String>=std::env::args().collect();
 if a[1]=="http" {
  let target=url::parse(&a[2])?.0;
  let trust=trust::Store::from_file(std::path::Path::new(&a[3]))?;
  let request=if a[4]=="-" { http::Request::Get } else {
   http::Request::FormPost{body:std::fs::read(&a[4]).map_err(|e|e.to_string())?,origin:Some(target.clone())}
  };
  let mode=if a[5]=="full" {http::FetchMode::FullReload} else {http::FetchMode::Normal};
  let response=http::fetch_request(&target,&request,1024*1024,None,
   &std::sync::atomic::AtomicBool::new(false),&trust,mode)?;
  if response.status!=200 || !response.body.starts_with(b"TLS12_FIXTURE_OK") {
   return Err(format!("Unexpected HTTP response {} / {} bytes",response.status,response.body.len()));
  }
  if !response.events.iter().any(|event|event.contains("TLS 1.2")) {
   return Err(format!("TLS1.2 was not reported: {:?}",response.events));
  }
  for event in &response.events {if event.contains("TLS 1.2"){println!("AUTH {event}");}}
  println!("PASS HTTP status={} post={} bytes={}",response.status,response.request_is_post,response.body.len());
 } else {
  let port=a[2].parse::<u16>().map_err(|e|e.to_string())?;
  let roots=x509::parse_pem_certificates(&std::fs::read(&a[3]).map_err(|e|e.to_string())?)?;
  let socket=std::net::TcpStream::connect(("127.0.0.1",port)).map_err(|e|e.to_string())?;
  socket.set_read_timeout(Some(std::time::Duration::from_millis(100))).map_err(|e|e.to_string())?;
  socket.set_write_timeout(Some(std::time::Duration::from_millis(100))).map_err(|e|e.to_string())?;
  socket.set_nodelay(true).map_err(|e|e.to_string())?;
  let fragment=a[4]=="fragment";
  let socket=Fragmented(socket,if fragment{7}else{65536},if fragment{11}else{65536});
  let mut entropy=[0u8;64];random::fill(&mut entropy)?;
  let deadline=std::time::Instant::now()+std::time::Duration::from_secs(8);
  let mut stream=tls::TlsStream::connect(socket,"localhost",&roots,std::time::SystemTime::now(),entropy,
   move||if std::time::Instant::now()<deadline{Ok(())}else{Err("TLS12 test deadline".into())})?;
  println!("AUTH {}",stream.certificate_summary());
  stream.write_all(b"GET /scripted HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").map_err(|e|e.to_string())?;
  let mut bytes=Vec::new();stream.read_to_end(&mut bytes).map_err(|e|e.to_string())?;
  stream.close().map_err(|e|e.to_string())?;
  if !bytes.windows(b"TLS12_FIXTURE_OK".len()).any(|b|b==b"TLS12_FIXTURE_OK") {
   return Err("Missing HTTP fixture marker".into());
  }
  println!("PASS STREAM bytes={} fragmented={fragment}",bytes.len());
 }
 Ok(())
}
fn main(){if let Err(error)=run(){eprintln!("ERROR {error}");std::process::exit(1);}}
'''


def u16(value):
    return value.to_bytes(2, "big")


def u24(value):
    return value.to_bytes(3, "big")


def vector(value):
    return u16(len(value)) + value


def extension(kind, value):
    return u16(kind) + vector(value)


def handshake(kind, value):
    return bytes([kind]) + u24(len(value)) + value


def record(kind, value, version=b"\x03\x03"):
    return bytes([kind]) + version + u16(len(value)) + value


def exact(stream, length):
    result = bytearray()
    while len(result) < length:
        part = stream.recv(length - len(result))
        if not part:
            raise EOFError("test peer closed the TCP stream")
        result.extend(part)
    return bytes(result)


def receive(stream):
    header = exact(stream, 5)
    length = int.from_bytes(header[3:5], "big")
    assert length <= 18432, "oversized peer TLS record"
    return header + exact(stream, length)


@dataclass(frozen=True)
class Suite:
    wire: int
    name: str
    key_type: str
    hash_name: str
    key_length: int
    cbc: bool = False

    def digest(self, data):
        return hashlib.new(self.hash_name, data).digest()

    def prf(self, secret, label, seed, length):
        seed = label + seed
        output = b""
        a = seed
        while len(output) < length:
            a = hmac.digest(secret, a, self.hash_name)
            output += hmac.digest(secret, a + seed, self.hash_name)
        return output[:length]

    def finished(self, master, side, transcript):
        return self.prf(master, side + b" finished", self.digest(transcript), 12)


SUITES = [
    Suite(0xC02F, "ECDHE-RSA-AES128-GCM-SHA256", "RSA", "sha256", 16),
    Suite(0xC030, "ECDHE-RSA-AES256-GCM-SHA384", "RSA", "sha384", 32),
    Suite(0xC02B, "ECDHE-ECDSA-AES128-GCM-SHA256", "P256", "sha256", 16),
    Suite(0xC02C, "ECDHE-ECDSA-AES256-GCM-SHA384", "P256", "sha384", 32),
    Suite(0xC027, "ECDHE-RSA-AES128-SHA256", "RSA", "sha256", 16, True),
    Suite(0xC028, "ECDHE-RSA-AES256-SHA384", "RSA", "sha384", 32, True),
]
GCM_SUITES = SUITES[:4]
CBC_SUITES = SUITES[4:]
GROUPS = [(29, "X25519"), (23, "prime256v1")]
SIGNATURES = {
    0x0401: ("RSA_PKCS1_SHA256", "RSA", hashes.SHA256),
    0x0501: ("RSA_PKCS1_SHA384", "RSA", hashes.SHA384),
    0x0601: ("RSA_PKCS1_SHA512", "RSA", hashes.SHA512),
    0x0804: ("RSA_PSS_RSAE_SHA256", "RSA", hashes.SHA256),
    0x0805: ("RSA_PSS_RSAE_SHA384", "RSA", hashes.SHA384),
    0x0806: ("RSA_PSS_RSAE_SHA512", "RSA", hashes.SHA512),
    0x0403: ("ECDSA_P256_SHA256", "P256", hashes.SHA256),
    0x0503: ("ECDSA_P384_SHA384", "P384", hashes.SHA384),
}


class Traffic:
    """RFC 5288 AEAD, implemented independently with Python cryptography."""
    def __init__(self, key, fixed_iv):
        self.aead = AESGCM(key)
        self.fixed_iv = fixed_iv
        self.sequence = 0

    def seal(self, kind, plain, version=b"\x03\x03"):
        sequence = self.sequence.to_bytes(8, "big")
        self.sequence += 1
        aad = sequence + bytes([kind]) + version + u16(len(plain))
        cipher = self.aead.encrypt(self.fixed_iv + sequence, plain, aad)
        return record(kind, sequence + cipher, version)

    def open(self, wire):
        assert len(wire) >= 29, "short GCM record"
        sequence = self.sequence.to_bytes(8, "big")
        self.sequence += 1
        length = len(wire) - 5 - 8 - 16
        aad = sequence + wire[:3] + u16(length)
        plain = self.aead.decrypt(self.fixed_iv + wire[5:13], wire[13:], aad)
        return wire[0], plain


class CbcTraffic:
    """Independent RFC 5246/7366 peer; Python cryptography does the AES work.

    This development oracle does not handle production secrets or claim
    constant-time padding validation. Production code has its own checks.
    """
    def __init__(self, key, mac_key, hash_name, etm):
        self.key, self.mac_key, self.hash_name, self.etm = key, mac_key, hash_name, etm
        self.mac_length = hashlib.new(hash_name).digest_size
        self.sequence = 0

    def mac(self, sequence, kind_version, data):
        return hmac.digest(self.mac_key, sequence + kind_version + u16(len(data)) + data, self.hash_name)

    def seal(self, kind, plain, version=b"\x03\x03"):
        sequence = self.sequence.to_bytes(8, "big")
        self.sequence += 1
        kind_version = bytes([kind]) + version
        content = plain if self.etm else plain + self.mac(sequence, kind_version, plain)
        pad = (-len(content) - 1) % 16
        content += bytes([pad]) * (pad + 1)
        iv = os.urandom(16)
        encryptor = Cipher(algorithms.AES(self.key), modes.CBC(iv)).encryptor()
        cipher = iv + encryptor.update(content) + encryptor.finalize()
        if self.etm:
            cipher += self.mac(sequence, kind_version, cipher)
        return record(kind, cipher, version)

    def open(self, wire):
        assert len(wire) == int.from_bytes(wire[3:5], "big") + 5, "bad CBC record length"
        sequence = self.sequence.to_bytes(8, "big")
        self.sequence += 1
        cipher = wire[5:]
        if self.etm:
            cipher, mac = cipher[:-self.mac_length], cipher[-self.mac_length:]
            assert hmac.compare_digest(self.mac(sequence, wire[:3], cipher), mac), "bad CBC EtM MAC"
        assert len(cipher) >= 32 and len(cipher) % 16 == 0, "bad CBC block length"
        decryptor = Cipher(algorithms.AES(self.key), modes.CBC(cipher[:16])).decryptor()
        padded = decryptor.update(cipher[16:]) + decryptor.finalize()
        padding_length = padded[-1] + 1
        assert padding_length <= len(padded), "bad CBC padding length"
        assert padded[-padding_length:] == bytes([padding_length - 1]) * padding_length, "bad CBC padding bytes"
        plain = padded[:-padding_length]
        if not self.etm:
            assert len(plain) >= self.mac_length, "short CBC MAC"
            plain, mac = plain[:-self.mac_length], plain[-self.mac_length:]
            assert hmac.compare_digest(self.mac(sequence, wire[:3], plain), mac), "bad CBC MtE MAC"
        return wire[0], plain


def traffic_pair(suite, master, client_random, server_random, etm):
    if suite.cbc:
        mac_length = hashlib.new(suite.hash_name).digest_size
        block = suite.prf(master, b"key expansion", server_random + client_random,
                          2 * (mac_length + suite.key_length))
        client_mac, server_mac = block[:mac_length], block[mac_length:2 * mac_length]
        keys = block[2 * mac_length:]
        return (CbcTraffic(keys[:suite.key_length], client_mac, suite.hash_name, etm),
                CbcTraffic(keys[suite.key_length:], server_mac, suite.hash_name, etm))
    block = suite.prf(master, b"key expansion", server_random + client_random, suite.key_length * 2 + 8)
    return (Traffic(block[:suite.key_length], block[suite.key_length * 2:suite.key_length * 2 + 4]),
            Traffic(block[suite.key_length:suite.key_length * 2], block[-4:]))


def identities(directory):
    now = datetime.now(timezone.utc)
    rsa_key = lambda: rsa.generate_private_key(public_exponent=65537, key_size=2048)
    name = lambda text: x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, text)])
    root_key = rsa_key()
    root_name = name("ChezCat ephemeral TLS12 test root")

    def issue(subject, key, issuer, signer, ca=False, variant="valid"):
        before, after = now - timedelta(days=2), now + timedelta(days=2)
        if variant == "expired":
            after = now - timedelta(days=1)
        if variant == "future":
            before = now + timedelta(days=1)
        builder = (x509.CertificateBuilder().subject_name(subject).issuer_name(issuer)
                   .public_key(key.public_key()).serial_number(x509.random_serial_number())
                   .not_valid_before(before).not_valid_after(after)
                   .add_extension(x509.BasicConstraints(ca=ca, path_length=0 if ca else None), True)
                   .add_extension(x509.KeyUsage(not ca, False, False, False, False, ca, ca, False, False), True)
                   .add_extension(x509.SubjectKeyIdentifier.from_public_key(key.public_key()), False)
                   .add_extension(x509.AuthorityKeyIdentifier.from_issuer_public_key(signer.public_key()), False))
        if not ca:
            san = [x509.DNSName("wrong.invalid")] if variant == "wrong_host" else [
                x509.DNSName("localhost"), x509.IPAddress(ipaddress.ip_address("127.0.0.1"))]
            eku = ExtendedKeyUsageOID.CLIENT_AUTH if variant == "wrong_eku" else ExtendedKeyUsageOID.SERVER_AUTH
            builder = builder.add_extension(x509.SubjectAlternativeName(san), False)
            builder = builder.add_extension(x509.ExtendedKeyUsage([eku]), False)
        return builder.sign(signer, hashes.SHA256())

    root = issue(root_name, root_key, root_name, root_key, ca=True)
    trust_file = directory / "ca.pem"
    trust_file.write_bytes(root.public_bytes(serialization.Encoding.PEM))
    other_key = rsa_key()
    other_name = name("Untrusted ephemeral root")
    other = issue(other_name, other_key, other_name, other_key, ca=True)
    other_file = directory / "other-ca.pem"
    other_file.write_bytes(other.public_bytes(serialization.Encoding.PEM))
    result = {}
    for kind in ["RSA", "P256", "P384", "wrong_host", "expired", "future", "wrong_eku"]:
        key = (ec.generate_private_key(ec.SECP256R1()) if kind == "P256" else
               ec.generate_private_key(ec.SECP384R1()) if kind == "P384" else rsa_key())
        cert = issue(name("localhost"), key, root_name, root_key, variant=kind)
        cert_file, key_file = directory / (kind + ".pem"), directory / (kind + "-key.pem")
        cert_file.write_bytes(cert.public_bytes(serialization.Encoding.PEM))
        key_file.write_bytes(key.private_bytes(serialization.Encoding.PEM,
                                             serialization.PrivateFormat.PKCS8,
                                             serialization.NoEncryption()))
        result[kind] = {"key": key, "chain": [cert], "cert_file": cert_file,
                        "key_file": key_file, "trust_file": trust_file, "other_file": other_file}
    return result


def compile_probe(directory, rustc):
    source = "#![allow(dead_code)]\n" + "\n".join(
        f"#[path={json.dumps(str(ROOT / 'src' / (name + '.rs')))}] mod {name};" for name in MODULES)
    path = directory / "probe.rs"
    path.write_text(source + "\n" + PROBE)
    binary = directory / ("probe.exe" if os.name == "nt" else "probe")
    subprocess.run([rustc, "--edition=2021", "-O", str(path), "-o", str(binary)], check=True)
    return binary


def read_http(stream):
    data = b""
    while b"\r\n\r\n" not in data:
        part = stream.recv(16384)
        if not part:
            raise EOFError("TLS finished without an HTTP header")
        data += part
        assert len(data) < 256 * 1024, "oversized HTTP fixture request"
    head, body = data.split(b"\r\n\r\n", 1)
    lines = head.decode("ascii").split("\r\n")
    method, target, version = lines[0].split(" ")
    assert version == "HTTP/1.1"
    headers = {}
    for line in lines[1:]:
        key, value = line.split(":", 1)
        key = key.lower()
        assert key not in headers, "duplicate outgoing header"
        headers[key] = value.strip()
    length = int(headers.get("content-length", "0"))
    assert length <= 128 * 1024
    if len(body) < length:
        body += exact(stream, length - len(body))
    assert len(body) == length
    return {"method": method, "target": target, "headers": headers,
            "body_length": len(body), "body_sha256": hashlib.sha256(body).hexdigest()}


def ssl_server(listener, context, count, result, redirect):
    result["requests"] = []
    result["connections"] = []
    try:
        for index in range(count):
            sock, _ = listener.accept()
            with sock:
                sock.settimeout(5)
                try:
                    with context.wrap_socket(sock, server_side=True) as secure:
                        result["connections"].append({"version": secure.version(), "cipher": secure.cipher()[0],
                                                      "alpn": secure.selected_alpn_protocol()})
                        request = read_http(secure)
                        result["requests"].append(request)
                        if redirect and index == 0:
                            response = (f"HTTP/1.1 {redirect} Redirect\r\nLocation: /done?preserved=1\r\n"
                                        "Content-Length: 0\r\nConnection: close\r\n\r\n").encode()
                        else:
                            body = MARKER + (b"\n" + b"r" * 32768 if request["method"] == "GET" else b"")
                            response = (b"HTTP/1.1 200 OK\r\nContent-Length: " + str(len(body)).encode()
                                        + b"\r\nConnection: close\r\n\r\n" + body)
                        secure.sendall(response)
                        try:
                            secure.unwrap().close()
                        except (OSError, ssl.SSLError):
                            # HTTP content-length does not require waiting for peer closure.
                            pass
                except (ssl.SSLError, EOFError, ConnectionResetError) as error:
                    result["peer_error"] = str(error)
                    if count != 1 or result["requests"]:
                        raise
        result["server_pass"] = True
    except Exception:
        result["server_error"] = traceback.format_exc()


def client_parameters(hello):
    assert hello[0] == 1 and len(hello) == int.from_bytes(hello[1:4], "big") + 4
    assert hello[4:6] == b"\x03\x03"
    random = hello[6:38]
    offset = 38
    size = hello[offset]
    assert size == 0, "session resumption must not be offered"
    offset += 1 + size
    size = int.from_bytes(hello[offset:offset + 2], "big")
    offset += 2
    assert size % 2 == 0
    suites = {int.from_bytes(hello[i:i + 2], "big") for i in range(offset, offset + size, 2)}
    offset += size
    assert hello[offset:offset + 2] == b"\x01\x00", "only null compression is allowed"
    offset += 2
    size = int.from_bytes(hello[offset:offset + 2], "big")
    offset += 2
    assert offset + size == len(hello)
    extensions = {}
    while offset < len(hello):
        kind = int.from_bytes(hello[offset:offset + 2], "big")
        size = int.from_bytes(hello[offset + 2:offset + 4], "big")
        offset += 4
        assert kind not in extensions and offset + size <= len(hello)
        extensions[kind] = hello[offset:offset + size]
        offset += size
    assert {suite.wire for suite in SUITES} <= suites
    assert {0x1301, 0x1302, 0x1303} <= suites, "one ClientHello must offer both TLS versions"
    assert extensions[23] == b"", "EMS must be offered"
    assert extensions[22] == b"", "encrypt-then-MAC must be offered for CBC"
    assert extensions[0xFF01] == b"\x00", "initial secure-renegotiation acknowledgement must be offered"
    assert extensions[43] == b"\x04\x03\x04\x03\x03", "offer TLS1.3 and TLS1.2 without a retry"
    assert extensions[16] == b"\x00\x09\x08http/1.1"
    assert not ({35, 41, 42} & extensions.keys()), "tickets, PSK, and early data must be absent"
    groups = extensions[10]
    assert len(groups) == int.from_bytes(groups[:2], "big") + 2
    offered_groups = {int.from_bytes(groups[i:i + 2], "big") for i in range(2, len(groups), 2)}
    assert {23, 29} <= offered_groups
    signatures = extensions[13]
    assert len(signatures) == int.from_bytes(signatures[:2], "big") + 2
    offered_signatures = {int.from_bytes(signatures[i:i + 2], "big") for i in range(2, len(signatures), 2)}
    return random, suites, offered_groups, offered_signatures, extensions


def sign(key, scheme, message, mode):
    algorithm = SIGNATURES[scheme][2]()
    if isinstance(key, rsa.RSAPrivateKey):
        method = (padding.PSS(padding.MGF1(algorithm), 0 if mode == "bad_pss_salt" else algorithm.digest_size)
                  if scheme >= 0x0804 else padding.PKCS1v15())
        signature = key.sign(message, method, algorithm)
        key.public_key().verify(signature, message, method, algorithm)
    else:
        signature = key.sign(message, ec.ECDSA(algorithm))
        key.public_key().verify(signature, message, ec.ECDSA(algorithm))
    return signature


EARLY_REJECT = {
    "nonempty_ems", "duplicate_ems", "bad_alpn", "duplicate_alpn",
    "unoffered_extension", "downgrade", "old_version", "unsupported_suite", "tls13_suite",
    "compression", "empty_certificate", "bad_certificate_length", "bad_signature",
    "wrong_random_signature", "bad_pss_salt", "bad_curve", "invalid_point", "ske_trailing",
    "signature_auth_mismatch", "certificate_auth_mismatch", "hello_done_nonempty",
    "early_ccs", "early_application", "handshake_order", "server_hello_record_version",
    "missing_renegotiation", "bad_renegotiation", "duplicate_renegotiation",
    "nonempty_etm", "etm_with_aead", "cert_wrong_host", "cert_untrusted", "cert_expired",
}
LATE_REJECT = {
    "missing_ccs", "bad_ccs", "duplicate_ccs", "clear_finished", "bad_finished",
    "bad_finished_tag", "short_finished_record", "bad_finished_length", "wrong_finished_type",
    "trailing_finished", "application_before_finished", "close_before_finished", "record_version",
    "empty_handshake_before_finished",
    "wrong_master_finished", "wrong_master_keys",
}
RECORD_REJECT = {"application_bad_tag", "application_replay", "application_wrong_aad",
                 "truncated_eof", "plaintext_application", "renegotiation"}


def drain(stream, result, traffic=None, early=False):
    while True:
        try:
            wire = receive(stream)
        except (EOFError, ConnectionResetError):
            break
        if traffic is not None:
            kind, plain = traffic.open(wire)
        else:
            kind, plain = wire[0], wire[5:]
        result["client_record_types"].append(kind)
        assert kind != 23, "HTTP bytes escaped a failed server authentication gate"
        if early:
            assert kind == 21, "client sent key-exchange/CCS/Finished after a rejected server flight"
    result["server_pass"] = True


def scripted_server(listener, suite, group, scheme, identity, mode, fragmented, result, ems=True, etm=False):
    result["client_record_types"] = []
    result["extended_master_secret"] = ems
    result["record_protection"] = ("CBC EtM" if etm else "CBC MtE") if suite.cbc else "GCM"
    try:
        sock, _ = listener.accept()
        with sock:
            sock.settimeout(4)
            wire = receive(sock)
            assert wire[0] == 22
            hello = wire[5:]
            client_random, _, groups, signatures, offered_extensions = client_parameters(hello)
            assert group in groups and scheme in signatures
            result["dual_version_client_hello"] = True
            server_random = os.urandom(32)
            if mode == "downgrade":
                server_random = server_random[:-8] + b"DOWNGRD\x01"
            private = (x25519.X25519PrivateKey.generate() if group == 29 else
                       ec.generate_private_key(ec.SECP256R1()))
            public = (private.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
                      if group == 29 else private.public_key().public_bytes(
                          serialization.Encoding.X962, serialization.PublicFormat.UncompressedPoint))
            if mode == "invalid_point":
                public = bytes(32) if group == 29 else b"\x04" + bytes(64)
            params = b"\x03" + u16(24 if mode == "bad_curve" else group) + bytes([len(public)]) + public
            signed_random = client_random
            if mode == "wrong_random_signature":
                signed_random = bytes([client_random[0] ^ 1]) + client_random[1:]
            signature = sign(identity["key"], scheme, signed_random + server_random + params, mode)
            if mode == "bad_signature":
                signature = signature[:-1] + bytes([signature[-1] ^ 1])
            wire_scheme = 0x0403 if mode == "signature_auth_mismatch" else scheme
            ske = handshake(12, params + u16(wire_scheme) + vector(signature) + (b"\x00" if mode == "ske_trailing" else b""))
            extensions = extension(23, b"x" if mode == "nonempty_ems" else b"") if ems else b""
            if mode == "duplicate_ems":
                extensions += extension(23, b"")
            extensions += extension(16, b"\x00\x03\x02h2" if mode == "bad_alpn" else b"\x00\x09\x08http/1.1")
            if mode == "duplicate_alpn":
                extensions += extension(16, b"\x00\x09\x08http/1.1")
            if mode == "unoffered_extension":
                extensions += extension(65000, b"")
            if (suite.cbc and etm) or mode in {"nonempty_etm", "etm_with_aead"}:
                extensions += extension(22, b"x" if mode == "nonempty_etm" else b"")
            if 0xFF01 in offered_extensions and mode != "missing_renegotiation":
                extensions += extension(0xFF01, b"\x01\x00" if mode == "bad_renegotiation" else b"\x00")
            if mode == "duplicate_renegotiation":
                extensions += extension(0xFF01, b"\x00")
            wire_suite = (0x009C if mode == "unsupported_suite" else 0x1301 if mode == "tls13_suite" else
                          0xC02B if mode == "certificate_auth_mismatch" else suite.wire)
            sh = handshake(2, (b"\x03\x02" if mode == "old_version" else b"\x03\x03")
                           + server_random + b"\x00" + u16(wire_suite)
                           + (b"\x01" if mode == "compression" else b"\x00") + vector(extensions))
            certs = b"".join(u24(len(der)) + der for der in
                             (cert.public_bytes(serialization.Encoding.DER) for cert in identity["chain"]))
            if mode == "empty_certificate":
                certs = b""
            certificate = handshake(11, u24(len(certs) + (1 if mode == "bad_certificate_length" else 0)) + certs)
            done = handshake(14, b"\x00" if mode == "hello_done_nonempty" else b"")
            flight = sh + (ske + certificate if mode == "handshake_order" else certificate + ske) + done
            transcript = hello + flight
            if mode in {"early_ccs", "early_application"}:
                flight_wire = record(22, sh) + record(20 if mode == "early_ccs" else 23, b"\x01") + record(22, certificate + ske + done)
            else:
                cuts = [1, 4, 37, 503, len(flight)] if fragmented else [len(flight)]
                start, flight_wire = 0, b""
                for end in cuts:
                    assert end > start
                    version = b"\x03\x01" if mode == "server_hello_record_version" else b"\x03\x03"
                    flight_wire += record(22, flight[start:end], version)
                    start = end
            if fragmented:
                for offset in range(0, len(flight_wire), 71):
                    sock.sendall(flight_wire[offset:offset + 71])
            else:
                sock.sendall(flight_wire)
            if mode in EARLY_REJECT:
                drain(sock, result, early=True)
                return
            cke = b""
            while len(cke) < 4 or len(cke) < int.from_bytes(cke[1:4], "big") + 4:
                wire = receive(sock)
                assert wire[0] == 22, f"expected ClientKeyExchange, got record {wire[0]}"
                cke += wire[5:]
            assert cke[0] == 16 and len(cke) == int.from_bytes(cke[1:4], "big") + 4
            assert cke[4] + 5 == len(cke)
            peer_key = cke[5:]
            shared = (private.exchange(x25519.X25519PublicKey.from_public_bytes(peer_key)) if group == 29 else
                      private.exchange(ec.ECDH(), ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), peer_key)))
            transcript += cke
            ems_master = suite.prf(shared, b"extended master secret", suite.digest(transcript), 48)
            classic_master = suite.prf(shared, b"master secret", client_random + server_random, 48)
            master = ems_master if ems else classic_master
            other_master = classic_master if ems else ems_master
            read, write = traffic_pair(suite, master, client_random, server_random, etm)
            assert receive(sock) == record(20, b"\x01"), "invalid client ChangeCipherSpec"
            kind, cf = read.open(receive(sock))
            assert kind == 22 and cf == handshake(20, suite.finished(master, b"client", transcript)), "bad client Finished/master-secret/PRF"
            result["client_finished_verified"] = True
            transcript += cf
            verify = suite.finished(master, b"server", transcript)
            if mode == "wrong_master_finished":
                verify = suite.finished(other_master, b"server", transcript)
            if mode == "wrong_master_keys":
                _, write = traffic_pair(suite, other_master, client_random, server_random, etm)
            if mode == "bad_finished":
                verify = verify[:-1] + bytes([verify[-1] ^ 1])
            if mode == "bad_finished_length":
                verify = verify[:-1]
            sf = handshake(15 if mode == "wrong_finished_type" else 20, verify)
            if mode == "trailing_finished":
                sf += handshake(0, b"")
            if mode == "valid" and fragmented:
                # Split the handshake header and verify_data over separately
                # authenticated records, not just fragmented TCP reads.
                finish_wire = b"".join(write.seal(22, part) for part in [sf[:1], sf[1:4], sf[4:]])
            else:
                finish_wire = write.seal(22, sf, b"\x03\x01" if mode == "record_version" else b"\x03\x03")
            if mode == "bad_finished_tag":
                finish_wire = finish_wire[:-1] + bytes([finish_wire[-1] ^ 1])
            if mode == "clear_finished":
                finish_wire = record(22, sf)
            if mode == "short_finished_record":
                finish_wire = record(22, bytes(23))
            if mode == "application_before_finished":
                write.sequence = 0
                finish_wire = write.seal(23, b"not authenticated")
            if mode == "empty_handshake_before_finished":
                write.sequence = 0
                # A valid Finished follows, so ignoring an authenticated empty
                # handshake record would incorrectly release the HTTP request.
                finish_wire = write.seal(22, b"") + write.seal(22, sf)
            if mode == "close_before_finished":
                write.sequence = 0
                finish_wire = write.seal(21, b"\x01\x00")
            ccs = b"" if mode == "missing_ccs" else record(20, b"\x02" if mode == "bad_ccs" else b"\x01")
            if mode == "duplicate_ccs":
                ccs += record(20, b"\x01")
            sock.sendall(ccs + finish_wire)
            if mode in LATE_REJECT:
                drain(sock, result, read)
                return
            request = b""
            while b"\r\n\r\n" not in request:
                kind, plain = read.open(receive(sock))
                assert kind == 23
                request += plain
                assert len(request) < 4096
            assert request.startswith(b"GET /scripted HTTP/1.1\r\n")
            result["http_after_finished"] = True
            response = b"HTTP/1.1 200 OK\r\nContent-Length: " + str(len(MARKER)).encode() + b"\r\n\r\n" + MARKER
            wire = write.seal(23, response)
            if mode == "application_bad_tag":
                wire = wire[:-1] + bytes([wire[-1] ^ 1])
            elif mode == "application_replay":
                wire = wire + wire
            elif mode == "application_wrong_aad":
                # Authenticated as a handshake record, then relabeled application data.
                write.sequence -= 1
                wire = write.seal(22, response)
                wire = b"\x17" + wire[1:]
            elif mode == "plaintext_application":
                wire = record(23, response)
            elif mode == "renegotiation":
                write.sequence -= 1
                wire = write.seal(22, handshake(0, b""))
            sock.sendall(wire)
            if mode == "truncated_eof":
                sock.shutdown(socket.SHUT_WR)
                drain(sock, result, read)
                return
            if mode in RECORD_REJECT:
                drain(sock, result, read)
                return
            sock.sendall(write.seal(21, b"\x01\x00"))
            kind, closed = read.open(receive(sock))
            assert kind == 21 and closed == b"\x01\x00", "missing reciprocal close_notify"
            result["clean_close"] = True
            result["server_pass"] = True
    except Exception:
        result["server_error"] = traceback.format_exc()


class Checks:
    def __init__(self, probe, directory, match):
        self.probe, self.directory, self.match = probe, directory, match
        self.results = []
        self.form = directory / "form.bin"
        self.form.write_bytes(FORM)

    def run(self, name, server, args, expect, details=None, executable=None):
        if self.match and self.match not in name:
            return
        result = {"name": name, **(details or {})}
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen(4)
            listener.settimeout(5)
            port = listener.getsockname()[1]
            worker = threading.Thread(target=server, args=(listener, result), daemon=True)
            worker.start()
            try:
                process = subprocess.run([str(executable or self.probe), *args(port)], text=True,
                                         capture_output=True, timeout=16)
                worker.join(5)
                result.update(returncode=process.returncode, stdout=process.stdout.strip(), stderr=process.stderr.strip())
                assert not worker.is_alive(), "test server did not finish"
                assert result.get("server_pass"), "independent server failed; see server_error"
                expect(result)
                result["pass"] = True
            except Exception as error:
                result["pass"] = False
                result["check_error"] = str(error)
        self.results.append(result)
        print(("PASS " if result["pass"] else "FAIL ") + name, flush=True)
        if not result["pass"]:
            print(json.dumps(result, indent=2), flush=True)


def openssl_checks(checks, identity):
    for suite in SUITES:
        for group, curve in GROUPS:
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.minimum_version = context.maximum_version = ssl.TLSVersion.TLSv1_2
            context.set_ciphers(suite.name)
            context.set_ecdh_curve(curve)
            context.set_alpn_protocols(["http/1.1"])
            chosen = identity[suite.key_type]
            context.load_cert_chain(chosen["cert_file"], chosen["key_file"])
            for operation in ["get", "post128k", "post303", "post307"]:
                redirect = 303 if operation == "post303" else 307 if operation == "post307" else 0
                count = 2 if redirect else 1
                post = operation != "get"
                name = f"openssl/{suite.name}/{curve}/{operation}"

                def args(port):
                    return ["http", f"https://localhost:{port}/start?keep=1", str(chosen["trust_file"]),
                            str(checks.form) if post else "-", "full" if post else "normal"]

                def expect(result):
                    assert result["returncode"] == 0 and "PASS HTTP" in result["stdout"]
                    if suite.cbc:
                        assert "Encrypt-then-MAC" in result["stdout"], "OpenSSL CBC did not negotiate EtM"
                    requests = result["requests"]
                    assert len(requests) == count
                    assert all(connection == {"version": "TLSv1.2", "cipher": suite.name, "alpn": "http/1.1"}
                               for connection in result["connections"])
                    assert requests[0]["target"] == "/start?keep=1"
                    assert requests[0]["headers"]["user-agent"].startswith("ChezCat/")
                    for index, request in enumerate(requests):
                        actual_post = post and not (redirect == 303 and index == 1)
                        assert request["method"] == ("POST" if actual_post else "GET")
                        assert request["body_length"] == (len(FORM) if actual_post else 0)
                        assert request["body_sha256"] == hashlib.sha256(FORM if actual_post else b"").hexdigest()
                        headers = request["headers"]
                        if post:
                            assert headers["cache-control"] == "no-cache" and headers["pragma"] == "no-cache"
                        if actual_post:
                            assert headers["content-type"] == "application/x-www-form-urlencoded"
                            assert headers["content-length"] == str(len(FORM))
                            assert headers["origin"] == "https://" + headers["host"]
                        else:
                            assert not ({"origin", "content-type", "content-length"} & headers.keys())
                    if redirect:
                        assert requests[1]["target"] == "/done?preserved=1"
                    assert ("post=true" in result["stdout"]) == (post and redirect != 303)

                checks.run(name, lambda listener, result: ssl_server(listener, context, count, result, redirect),
                           args, expect, {"oracle": ssl.OPENSSL_VERSION, "group_forced": group})
    for variant in ["wrong_host", "untrusted", "expired", "future", "wrong_eku"]:
        chosen = identity["RSA" if variant == "untrusted" else variant]
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.minimum_version = context.maximum_version = ssl.TLSVersion.TLSv1_2
        context.set_ciphers(SUITES[0].name)
        context.set_ecdh_curve("X25519")
        context.set_alpn_protocols(["http/1.1"])
        context.load_cert_chain(chosen["cert_file"], chosen["key_file"])
        trust = chosen["other_file" if variant == "untrusted" else "trust_file"]

        def expect(result):
            assert result["returncode"] == 1 and "PASS" not in result["stdout"]
            assert not result["requests"], "HTTP/form bytes sent after a certificate failure"
            assert "certificate" in result["stderr"].lower(), "expected an explicit certificate failure"

        checks.run(f"openssl/auth/{variant}/post-withheld",
                   lambda listener, result: ssl_server(listener, context, 1, result, 0),
                   lambda port: ["http", f"https://localhost:{port}/private", str(trust), str(checks.form), "full"], expect)


def scripted_checks(checks, identity):
    cases = []
    for suite in SUITES:
        for group, _ in GROUPS:
            scheme = 0x0401 if suite.key_type == "RSA" else 0x0403
            for fragment in [False, True]:
                for etm in ([True, False] if suite.cbc else [False]):
                    cases.append((suite, group, scheme, "valid", fragment, True, etm))
        # A server omitting EMS must use the original TLS 1.2 master-secret
        # formula. The independent peer verifies the client's Finished, too.
        for etm in ([True, False] if suite.cbc else [False]):
            cases.append((suite, 23, scheme, "valid", True, False, etm))
    for scheme, (_, kind, _) in SIGNATURES.items():
        for suite in (GCM_SUITES[:2] if kind == "RSA" else GCM_SUITES[2:]):
            cases.append((suite, 23, scheme, "valid", True, True, False))
    for mode in sorted(EARLY_REJECT | LATE_REJECT | RECORD_REJECT):
        cases.append((GCM_SUITES[0], 23, 0x0804, mode, False, True, False))
    for suite, etm in [(GCM_SUITES[0], False), (CBC_SUITES[1], True), (CBC_SUITES[1], False)]:
        for mode in ["cert_wrong_host", "cert_untrusted", "cert_expired", "bad_signature",
                     "missing_renegotiation", "bad_finished", "wrong_master_finished", "wrong_master_keys"]:
            cases.append((suite, 23, 0x0804, mode, False, False, etm))
    for etm in [True, False]:
        for mode in sorted(RECORD_REJECT | {"bad_finished", "bad_finished_tag", "application_before_finished"}):
            cases.append((CBC_SUITES[1], 23, 0x0804, mode, False, True, etm))
    cases.append((CBC_SUITES[0], 23, 0x0804, "nonempty_etm", False, True, True))
    for suite, group, scheme, mode, fragment, ems, etm in dict.fromkeys(cases):
        identity_name = {"cert_wrong_host": "wrong_host", "cert_expired": "expired"}.get(mode, SIGNATURES[scheme][1])
        chosen = identity[identity_name]
        trust = chosen["other_file" if mode == "cert_untrusted" else "trust_file"]
        name = f"scripted/{suite.name}/{group}/{SIGNATURES[scheme][0]}/{mode}/{'fragment' if fragment else 'normal'}"
        if not ems:
            name += "/no-ems"
        if suite.cbc:
            name += "/etm" if etm else "/mte"

        def expect(result):
            if mode == "valid":
                assert result["returncode"] == 0 and "AUTH " in result["stdout"] and "PASS STREAM" in result["stdout"]
                assert "TLS 1.2" in result["stdout"]
                assert ("Extended Master Secret" if ems else "EMS not negotiated") in result["stdout"]
                if suite.cbc:
                    assert ("Encrypt-then-MAC" if etm else "legacy MAC-then-encrypt") in result["stdout"]
                assert result.get("client_finished_verified") and result.get("http_after_finished") and result.get("clean_close")
            else:
                assert result["returncode"] == 1 and "PASS" not in result["stdout"]
                assert "deadline" not in result["stderr"].lower(), "timeout is not a protocol rejection"
                if mode in RECORD_REJECT:
                    assert "AUTH " in result["stdout"] and result.get("http_after_finished")
                else:
                    assert "AUTH " not in result["stdout"] and not result.get("http_after_finished")
                    assert 23 not in result["client_record_types"]

        checks.run(name, lambda listener, result: scripted_server(
            listener, suite, group, scheme, chosen, mode, fragment, result, ems, etm),
            lambda port: ["stream", str(port), str(trust), "fragment" if fragment else "normal"], expect)


def browser_checks(checks, identity, browser):
    """The final executable, including App and loader, against non-EMS peers."""
    cases = [(suite, etm, "valid") for suite in SUITES
             for etm in ([True, False] if suite.cbc else [False])]
    for suite, etm in [(GCM_SUITES[0], False), (CBC_SUITES[1], True), (CBC_SUITES[1], False)]:
        for mode in ["cert_wrong_host", "cert_untrusted", "bad_finished"]:
            cases.append((suite, etm, mode))
    for suite, etm, mode in cases:
        scheme = 0x0401 if suite.key_type == "RSA" else 0x0403
        chosen = identity["wrong_host" if mode == "cert_wrong_host" else suite.key_type]
        trust = chosen["other_file" if mode == "cert_untrusted" else "trust_file"]
        name = f"browser/{suite.name}/no-ems/{mode}"
        if suite.cbc:
            name += "/etm" if etm else "/mte"
        image = checks.directory / (hashlib.sha256(name.encode()).hexdigest()[:16] + ".ppm")

        def expect(result):
            if mode == "valid":
                assert result["returncode"] == 0, result["stderr"]
                assert image.is_file() and image.read_bytes()[:3] == b"P6\n", "browser produced no render"
                assert result.get("client_finished_verified") and result.get("http_after_finished") and result.get("clean_close")
            else:
                assert result["returncode"] == 1 and not image.exists(), "failed TLS unexpectedly rendered"
                assert not result.get("http_after_finished") and 23 not in result["client_record_types"]
                assert "deadline" not in result["stderr"].lower(), "timeout is not a protocol rejection"

        checks.run(name, lambda listener, result: scripted_server(
            listener, suite, 23, scheme, chosen, mode, False, result, False, etm),
            lambda port: ["--trust-file", str(trust), f"https://localhost:{port}/scripted",
                          "--render", str(image), "--width", "640", "--height", "480"],
            expect, {"final_browser": str(browser)}, executable=browser)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustc", default="rustc", help="host Rust compiler")
    parser.add_argument("--output", type=Path, help="save complete JSON results")
    parser.add_argument("--only", choices=["openssl", "scripted", "browser"], help="run one independent oracle")
    parser.add_argument("--browser", type=Path, help="also check the built browser executable against non-EMS peers")
    parser.add_argument("--match", default="", help="run case names containing this text")
    arguments = parser.parse_args()
    if arguments.only == "browser" and not arguments.browser:
        parser.error("--only browser requires --browser")
    browser = arguments.browser.resolve() if arguments.browser else None
    if browser and not browser.is_file():
        parser.error("--browser must name the built executable")
    with tempfile.TemporaryDirectory(prefix="own-browser-tls12-") as temporary:
        directory = Path(temporary)
        probe = browser if arguments.only == "browser" else compile_probe(directory, arguments.rustc)
        identity = identities(directory)
        checks = Checks(probe, directory, arguments.match)
        if arguments.only in (None, "openssl"):
            openssl_checks(checks, identity)
        if arguments.only in (None, "scripted"):
            scripted_checks(checks, identity)
        if browser and arguments.only in (None, "browser"):
            browser_checks(checks, identity, browser)
        passed = sum(result["pass"] for result in checks.results)
        report = {"passed": passed, "total": len(checks.results), "openssl": ssl.OPENSSL_VERSION,
                  "scope": "Loopback only; production Rust transport against independent development oracles; no live sites or scans.",
                  "cases": checks.results}
        if arguments.output:
            arguments.output.parent.mkdir(parents=True, exist_ok=True)
            arguments.output.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps({key: value for key, value in report.items() if key != "cases"}, indent=2))
        return 0 if passed == len(checks.results) and passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
