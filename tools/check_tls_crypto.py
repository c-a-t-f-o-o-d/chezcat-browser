#!/usr/bin/env python3
"""Independent offline TLS 1.3 derivation and record interoperability checks.

Run: python tools/check_tls_crypto.py [--rustc /path/to/rustc]

Development requirements: Python cryptography and a host Rust compiler. The
browser does not import, invoke, or link this script or its oracles. No network
is used. SHA-256/SHA-384 HKDF, HMAC and Finished values are compared with Python
hashlib/hmac/cryptography; records are checked with independent ChaCha20Poly1305
and AESGCM implementations. Temporary probes are removed on completion.
"""

import argparse
from collections import Counter
import hashlib
import hmac
import json
import os
from pathlib import Path
import random
import subprocess
import tempfile
import time

from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.ciphers.aead import AESGCM, ChaCha20Poly1305
from cryptography.hazmat.primitives.kdf.hkdf import HKDFExpand

ROOT = Path(__file__).resolve().parents[1]
SUITES = {
    "chacha": ("sha256", hashes.SHA256, 32, ChaCha20Poly1305),
    "aes128": ("sha256", hashes.SHA256, 16, AESGCM),
    "aes256": ("sha384", hashes.SHA384, 32, AESGCM),
}

PROBE = r'''
#![allow(dead_code)]
#[path=HASH] mod crypto_hash;
#[path=SHA512] mod crypto_sha512;
#[path=AEAD] mod crypto_aead;
#[path=AES] mod crypto_aes_gcm;
#[path=TLS] mod tls_crypto;
use std::io::{self,BufRead,Write};
fn unhex(s:&str)->Vec<u8>{s.as_bytes().as_chunks::<2>().0.iter().map(|b|u8::from_str_radix(std::str::from_utf8(b).unwrap(),16).unwrap()).collect()}
fn hex(v:&[u8])->String{let mut s=String::new();for b in v{use std::fmt::Write;write!(&mut s,"{b:02x}").unwrap();}s}
fn output(result:Result<Vec<u8>,String>)->String{result.map(|v|hex(&v)).unwrap_or_else(|_|"ERR".into())}
fn shared(s:&str)->[u8;32]{unhex(s).try_into().unwrap()}
fn text(s:&str)->String{String::from_utf8(unhex(s)).unwrap()}
fn suite(s:&str)->tls_crypto::Suite{match s{
    "chacha"=>tls_crypto::Suite::ChaCha20Poly1305Sha256,
    "aes128"=>tls_crypto::Suite::Aes128GcmSha256,
    "aes256"=>tls_crypto::Suite::Aes256GcmSha384,_=>panic!()}}
fn kind(n:&str)->tls_crypto::ContentType{match n{"21"=>tls_crypto::ContentType::Alert,"22"=>tls_crypto::ContentType::Handshake,"23"=>tls_crypto::ContentType::ApplicationData,_=>panic!()}}
fn name(t:tls_crypto::ContentType)->u8{match t{tls_crypto::ContentType::Alert=>21,tls_crypto::ContentType::Handshake=>22,tls_crypto::ContentType::ApplicationData=>23}}
fn main(){let mut out=io::BufWriter::new(io::stdout().lock());for line in io::stdin().lock().lines(){let line=line.unwrap();let f:Vec<_>=line.split('\t').collect();let suite=suite(f[1]);let answer=match f[0]{
"early"=>hex(&tls_crypto::early_secret(suite)),
"hash"=>hex(&suite.hash(&unhex(f[2]))),
"label"=>output(tls_crypto::hkdf_expand_label(suite,&unhex(f[2]),&text(f[3]),&unhex(f[4]),f[5].parse().unwrap())),
"derive"=>output(tls_crypto::derive_secret(suite,&unhex(f[2]),&text(f[3]),&unhex(f[4]))),
"handshake"=>output(tls_crypto::handshake_secret(suite,&unhex(f[2]),&shared(f[3]))),
"main"=>output(tls_crypto::main_secret(suite,&unhex(f[2]))),
"finished"=>output(tls_crypto::finished_verify_data(suite,&unhex(f[2]),&unhex(f[3]))),
"verify"=>tls_crypto::verify_finished(suite,&unhex(f[2]),&unhex(f[3]),&unhex(f[4])).map(|_|"OK".into()).unwrap_or_else(|_|"ERR".into()),
"next"=>output(tls_crypto::next_traffic_secret(suite,&unhex(f[2]))),
"seal"=>match tls_crypto::Sealer::new(suite,&unhex(f[2])){
    Err(_)=>"ERR".into(),Ok(mut sealer)=>f[3..].iter().map(|s|{let r:Vec<_>=s.split(':').collect();
        output(sealer.seal(kind(r[0]),&unhex(r[1]),if r[2]=="max"{usize::MAX}else{r[2].parse().unwrap()}))
    }).collect::<Vec<_>>().join("|")},
"open"=>match tls_crypto::Opener::new(suite,&unhex(f[2])){
    Err(_)=>"ERR".into(),Ok(mut opener)=>f[3..].iter().map(|s|opener.open(&unhex(s))
        .map(|v|format!("{}:{}",name(v.content_type),hex(&v.content)))
        .unwrap_or_else(|_|"ERR".into())).collect::<Vec<_>>().join("|")},
_=>panic!("unknown")};writeln!(out,"{answer}").unwrap();}}
'''


def expand(suite, secret, label, context, length):
    label = b"tls13 " + label
    info = length.to_bytes(2, "big") + bytes([len(label)]) + label + bytes([len(context)]) + context
    return HKDFExpand(SUITES[suite][1](), length, info).derive(secret)


def record(suite, secret, sequence, inner):
    key = expand(suite, secret, b"key", b"", SUITES[suite][2])
    iv = expand(suite, secret, b"iv", b"", 12)
    nonce = bytes(a ^ b for a, b in zip(iv, sequence.to_bytes(12, "big")))
    header = b"\x17\x03\x03" + (len(inner) + 16).to_bytes(2, "big")
    return header + SUITES[suite][3](key).encrypt(nonce, inner, header)


def run(rustc):
    started = time.monotonic()
    rng = random.Random(0x5245434F5244)
    cases = []
    comparisons = Counter()

    def add(suite, name, operation, fields, expected):
        line = "\t".join([operation, suite] + [v.hex() if isinstance(v, bytes) else str(v) for v in fields])
        expected = expected.hex() if isinstance(expected, bytes) else expected
        cases.append((suite, name, line, expected))
        comparisons[suite] += expected.count("|") + 1

    for suite, (hash_name, _, _, _) in SUITES.items():
        hash_size = hashlib.new(hash_name).digest_size
        empty_hash = hashlib.new(hash_name, b"").digest()
        early = hmac.digest(bytes(hash_size), bytes(hash_size), hash_name)
        add(suite, "no-PSK early secret", "early", [], early)
        for length in [0, 1, 55, 56, 63, 64, 111, 112, 127, 128, 129, 1024]:
            data = rng.randbytes(length)
            add(suite, "suite transcript hash", "hash", [data], hashlib.new(hash_name, data).digest())
        for index in range(24):
            secret = rng.randbytes(hash_size)
            shared = rng.randbytes(32)  # X25519 remains 32 bytes for every suite.
            transcript = hashlib.new(hash_name, rng.randbytes(200)).digest()
            for label in [b"derived", b"c hs traffic", b"s hs traffic", b"c ap traffic",
                          b"s ap traffic", b"exp master", b"res master"]:
                add(suite, "derive " + label.decode(), "derive", [secret, label, transcript],
                    expand(suite, secret, label, transcript, hash_size))
            handshake = hmac.digest(expand(suite, early, b"derived", empty_hash, hash_size), shared, hash_name)
            add(suite, "handshake secret", "handshake", [early, shared], handshake)
            main = hmac.digest(expand(suite, handshake, b"derived", empty_hash, hash_size), bytes(hash_size), hash_name)
            add(suite, "main secret", "main", [handshake], main)
            finished = hmac.digest(expand(suite, secret, b"finished", b"", hash_size), transcript, hash_name)
            add(suite, "Finished HMAC", "finished", [secret, transcript], finished)
            add(suite, "valid Finished", "verify", [secret, transcript, finished], "OK")
            bad = bytearray(finished)
            bad[index % hash_size] ^= 1
            add(suite, "changed Finished", "verify", [secret, transcript, bytes(bad)], "ERR")
            add(suite, "KeyUpdate traffic secret", "next", [secret], expand(suite, secret, b"traffic upd", b"", hash_size))
            records, plaintexts, instructions = [], [], []
            for sequence, content_type in enumerate([21, 22, 23]):
                data = rng.randbytes(2 if content_type == 21 else rng.randrange(1, 500))
                padding = rng.randrange(300)
                records.append(record(suite, secret, sequence, data + bytes([content_type]) + bytes(padding)))
                plaintexts.append(f"{content_type}:{data.hex()}")
                instructions.append(f"{content_type}:{data.hex()}:{padding}")
            add(suite, "generated record seal", "seal", [secret] + instructions, "|".join(v.hex() for v in records))
            add(suite, "generated record open", "open", [secret] + records, "|".join(plaintexts))

        secret = rng.randbytes(hash_size)
        maximum_hkdf = 255 * hash_size
        for label, context, length in [
            (b"a", b"", 0), (b"a" * 249, bytes(255), hash_size),
            (b"key", b"", SUITES[suite][2]), (b"iv", b"", 12),
            (b"x", b"context", maximum_hkdf),
        ]:
            add(suite, "HKDF serialization boundary", "label", [secret, label, context, length],
                expand(suite, secret, label, context, length))
        for label, context, length in [
            (b"", b"", hash_size), (b"a" * 250, b"", hash_size),
            (b"x", bytes(256), hash_size), (b"x", b"", maximum_hkdf + 1),
            (b"x", b"", 65536), (b"x\0", b"", hash_size), ("é".encode(), b"", hash_size),
        ]:
            add(suite, "invalid HKDF parameters", "label", [secret, label, context, length], "ERR")

        valid_finished = hmac.digest(expand(suite, secret, b"finished", b"", hash_size), empty_hash, hash_name)
        for length in sorted({0, 1, hash_size - 1, hash_size + 1, 32 if hash_size == 48 else 48, 128}):
            wrong = bytes(length)
            add(suite, "wrong secret length label", "label", [wrong, b"key", b"", 16], "ERR")
            add(suite, "wrong secret length derive", "derive", [wrong, b"derived", empty_hash], "ERR")
            add(suite, "wrong secret length handshake", "handshake", [wrong, bytes(32)], "ERR")
            add(suite, "wrong secret length main", "main", [wrong], "ERR")
            add(suite, "wrong secret length Finished", "finished", [wrong, empty_hash], "ERR")
            add(suite, "wrong secret length verify", "verify", [wrong, empty_hash, valid_finished], "ERR")
            add(suite, "wrong secret length KeyUpdate", "next", [wrong], "ERR")
            add(suite, "wrong secret length sealer", "seal", [wrong, "23:6f6b:0"], "ERR")
            add(suite, "wrong secret length opener", "open", [wrong, b""], "ERR")
            add(suite, "wrong transcript length derive", "derive", [secret, b"derived", wrong], "ERR")
            add(suite, "wrong transcript length Finished", "finished", [secret, wrong], "ERR")
            add(suite, "wrong transcript length verify", "verify", [secret, wrong, valid_finished], "ERR")
            add(suite, "wrong Finished length", "verify", [secret, empty_hash, wrong], "ERR")

        # One state crosses the 255 -> 256 sequence-byte carry for each cipher.
        records, plaintexts, instructions = [], [], []
        for sequence in range(300):
            data = sequence.to_bytes(4, "big") + b"\0"
            padding = sequence % 17
            records.append(record(suite, secret, sequence, data + b"\x17" + bytes(padding)))
            plaintexts.append("23:" + data.hex())
            instructions.append(f"23:{data.hex()}:{padding}")
        add(suite, "300 consecutive seal records", "seal", [secret] + instructions, "|".join(v.hex() for v in records))
        add(suite, "300 consecutive open records", "open", [secret] + records, "|".join(plaintexts))
        valid0 = record(suite, secret, 0, b"ok\x17")
        valid1 = record(suite, secret, 1, b"next\x17")
        for data, padding in [(b"", 0), (b"", 16384), (bytes(16384), 0), (b"x", 16383)]:
            wire = record(suite, secret, 0, data + b"\x17" + bytes(padding))
            add(suite, "record boundary seal", "seal", [secret, f"23:{data.hex()}:{padding}"], wire)
            add(suite, "record boundary open", "open", [secret, wire], "23:" + data.hex())
        for instruction in ["23::16385", "23:00:max", "22::0", "21::0", "21:00:0",
                            "21:000000:0", "23:" + bytes(16385).hex() + ":0"]:
            add(suite, "sealer error permanently poisons", "seal", [secret, instruction, "23:6f6b:0"], "ERR|ERR")
        invalid = []
        for position in [0, 1, 2, 3, 4, 5, len(valid0) - 1]:
            wire = bytearray(valid0)
            wire[position] ^= 1
            invalid.append(bytes(wire))
        invalid.extend([b"", valid0[:4], valid0[:-1], valid0 + b"\0", valid0 + valid1])
        for inner in [b"", b"\0", bytes(64), b"\x14", b"\x16", b"\x15", b"x\x15", b"xyz\x15",
                      b"content\xff", bytes(16385) + b"\x17", b"abc\x17\x01"]:
            invalid.append(record(suite, secret, 0, inner))
        for wire in invalid:
            add(suite, "opener error permanently poisons", "open", [secret, wire, valid0], "ERR|ERR")
        add(suite, "replay fails and poisons", "open", [secret, valid0, valid0, valid1], "23:6f6b|ERR|ERR")
        add(suite, "out-of-order fails and poisons", "open", [secret, valid1, valid0], "ERR|ERR")
        updated = expand(suite, secret, b"traffic upd", b"", hash_size)
        next_record = record(suite, updated, 0, b"new\x17")
        add(suite, "KeyUpdate opens at sequence zero", "open", [updated, next_record], "23:6e6577")
        add(suite, "KeyUpdate seals at sequence zero", "seal", [updated, "23:6e6577:0"], next_record)
        add(suite, "old key rejects new epoch", "open", [secret, next_record], "ERR")

    # Same-sized SHA-256 secrets cannot make records interchangeable between
    # AES-128-GCM and ChaCha20-Poly1305 even when their IV derivation is equal.
    shared_secret = rng.randbytes(32)
    for sender, receiver in [("chacha", "aes128"), ("aes128", "chacha")]:
        add(receiver, "different cipher suite rejects record", "open",
            [shared_secret, record(sender, shared_secret, 0, b"bound\x17")], "ERR")

    source = PROBE
    for token, filename in [("HASH", "crypto_hash.rs"), ("SHA512", "crypto_sha512.rs"),
                            ("AEAD", "crypto_aead.rs"), ("AES", "crypto_aes_gcm.rs"), ("TLS", "tls_crypto.rs")]:
        source = source.replace("path=" + token, "path=" + json.dumps(str(ROOT / "src" / filename), ensure_ascii=False))
    with tempfile.TemporaryDirectory(prefix="tls-review-") as directory:
        folder = Path(directory)
        source_path = folder / "probe.rs"
        probe_path = folder / ("probe.exe" if os.name == "nt" else "probe")
        source_path.write_text(source, encoding="utf-8")
        subprocess.run([rustc, "--edition=2021", "-O", "-C", "overflow-checks=yes",
                        str(source_path), "-o", str(probe_path)], check=True, timeout=60)
        result = subprocess.run([str(probe_path)], input="\n".join(c[2] for c in cases) + "\n",
                                text=True, capture_output=True, check=True, timeout=60)
        lines = result.stdout.splitlines()
        assert len(lines) == len(cases), (len(lines), len(cases), result.stderr)
        failures = [{"suite": suite, "case": name, "expected": expected[:160], "actual": actual[:160]}
                    for (suite, name, _, expected), actual in zip(cases, lines) if actual != expected]
        print(json.dumps({"cases": len(cases), "comparisons": sum(comparisons.values()),
                          "comparisons_by_suite": dict(comparisons),
                          "elapsed_seconds": round(time.monotonic() - started, 2),
                          "failures": failures}, indent=2))
        if failures:
            raise SystemExit(1)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustc", default=os.environ.get("RUSTC", "rustc"), help="Host Rust compiler")
    run(parser.parse_args().rustc)
