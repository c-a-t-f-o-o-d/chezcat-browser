#!/usr/bin/env python3
"""Independent, offline checks of ChezCat's original hash/X25519 modules.

Run from any directory with Python's `cryptography` package and a host Rust
toolchain installed:
    python tools/check_crypto_hash_x25519.py [--rustc /path/to/rustc]

Python hashlib/hmac and cryptography are TEST ORACLES ONLY. They are never
linked, copied, invoked, or needed by the browser. No network is used. Temporary
Rust probes are compiled outside the source tree and removed after the checks.
The Rust module tests contain published NIST/RFC vectors; this script also
compares deterministic generated inputs with independent implementations.
"""

from __future__ import annotations

import argparse
import hashlib
import hmac
import json
import os
from pathlib import Path
import random
import subprocess
import tempfile

from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey, X25519PublicKey
from cryptography.hazmat.primitives.kdf.hkdf import HKDFExpand


PROBE = r'''
#![allow(dead_code)]
#[path = HASH_PATH] mod crypto_hash;
#[path = CURVE_PATH] mod crypto_x25519;
use std::io::{self, BufRead, Write};

fn unhex(text: &str) -> Vec<u8> {
    assert!(text.len().is_multiple_of(2));
    text.as_bytes().as_chunks::<2>().0.iter().map(|pair|
        u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect()
}
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes { use std::fmt::Write; write!(&mut out, "{byte:02x}").unwrap(); }
    out
}
fn key(text: &str) -> [u8; 32] { unhex(text).try_into().unwrap() }
fn main() {
    let input = io::stdin();
    let mut output = io::BufWriter::new(io::stdout().lock());
    for line in input.lock().lines() {
        let line = line.unwrap();
        let fields: Vec<_> = line.split('\t').collect();
        let answer = match fields[0] {
            "sha256" => hex(&crypto_hash::sha256(&unhex(fields[1]))),
            "stream" => {
                let data = unhex(fields[1]);
                let size: usize = fields[2].parse().unwrap();
                let mut hash = crypto_hash::Sha256::new();
                hash.update(&[]).unwrap();
                for chunk in data.chunks(size) { hash.update(chunk).unwrap(); }
                hash.update(&[]).unwrap();
                hex(&hash.finalize())
            },
            "clone" => {
                let mut prefix = crypto_hash::Sha256::new();
                prefix.update(&unhex(fields[1])).unwrap();
                let digest = prefix.clone().finalize();
                prefix.update(&unhex(fields[2])).unwrap();
                format!("{}:{}", hex(&digest), hex(&prefix.finalize()))
            },
            "hmac" => hex(&crypto_hash::hmac_sha256(&unhex(fields[1]), &unhex(fields[2]))),
            "extract" => hex(&crypto_hash::hkdf_extract(&unhex(fields[1]), &unhex(fields[2]))),
            "expand" => crypto_hash::hkdf_expand(&key(fields[1]), &unhex(fields[2]), fields[3].parse().unwrap())
                .map(|result| hex(&result)).unwrap_or_else(|_| "ERR".into()),
            "public" => hex(&crypto_x25519::public_key(&key(fields[1]))),
            "shared" => crypto_x25519::shared_secret(&key(fields[1]), &key(fields[2]))
                .map(|result| hex(&result)).unwrap_or_else(|_| "ERR".into()),
            _ => panic!("Unknown test operation"),
        };
        writeln!(output, "{answer}").unwrap();
    }
}
'''


def public_bytes(private: X25519PrivateKey) -> bytes:
    return private.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)


def cases(random_count: int) -> list[tuple[str, str, str]]:
    rng = random.Random(0x42524F57534552)
    checks: list[tuple[str, str, str]] = []

    def add(name: str, operation: str, fields: list[bytes | int], expected: bytes | str) -> None:
        line = "\t".join([operation] + [value.hex() if isinstance(value, bytes) else str(value) for value in fields])
        checks.append((name, line, expected.hex() if isinstance(expected, bytes) else expected))

    sizes = [0, 1, 2, 3, 31, 32, 33, 54, 55, 56, 57, 63, 64, 65, 66,
             111, 112, 119, 120, 121, 127, 128, 129, 255, 256, 257,
             511, 512, 513, 1023, 1024, 1025, 4095, 4096, 4097, 65535, 65536, 65537]
    for size in sizes:
        data = rng.randbytes(size)
        expected = hashlib.sha256(data).digest()
        add(f"SHA length {size}", "sha256", [data], expected)
        for chunk in [1, 7, 55, 63, 64, 65, 127]:
            add(f"SHA length {size} in {chunk}-byte pieces", "stream", [data, chunk], expected)
        prefix = size // 3
        add(f"SHA clone length {size}", "clone", [data[:prefix], data[prefix:]],
            hashlib.sha256(data[:prefix]).hexdigest() + ":" + expected.hex())
    large = rng.randbytes(1024 * 1024)
    expected = hashlib.sha256(large).digest()
    add("SHA one MiB", "sha256", [large], expected)
    add("SHA one MiB streamed", "stream", [large, 4093], expected)

    for key_size in [0, 1, 31, 32, 63, 64, 65, 127, 128, 129, 131, 255, 256, 257, 1024]:
        key = rng.randbytes(key_size)
        for data_size in [0, 1, 55, 56, 63, 64, 65, 127, 128, 129, 256, 4096]:
            data = rng.randbytes(data_size)
            add(f"HMAC key {key_size}, data {data_size}", "hmac", [key, data], hmac.digest(key, data, "sha256"))

    for length in [0, 1, 31, 32, 33, 63, 64, 65, 255, 256, 257, 8159, 8160, 8161]:
        prk, info = rng.randbytes(32), rng.randbytes(81)
        expected = "ERR" if length > 8160 else HKDFExpand(hashes.SHA256(), length, info).derive(prk)
        add(f"HKDF expansion length {length}", "expand", [prk, info, length], expected)

    for number in range(random_count):
        data = rng.randbytes(rng.randrange(8193))
        add(f"SHA generated {number}", "stream", [data, rng.randrange(1, 151)], hashlib.sha256(data).digest())
        salt, ikm, info = rng.randbytes(rng.randrange(151)), rng.randbytes(rng.randrange(201)), rng.randbytes(rng.randrange(257))
        prk = hmac.digest(salt if salt else bytes(32), ikm, "sha256")
        add(f"HKDF extraction generated {number}", "extract", [salt, ikm], prk)
        length = rng.randrange(1025)
        expected = HKDFExpand(hashes.SHA256(), length, info).derive(prk)
        add(f"HKDF expansion generated {number}", "expand", [prk, info, length], expected)

        left, right, peer = rng.randbytes(32), rng.randbytes(32), rng.randbytes(32)
        left_private, right_private = X25519PrivateKey.from_private_bytes(left), X25519PrivateKey.from_private_bytes(right)
        left_public, right_public = public_bytes(left_private), public_bytes(right_private)
        add(f"X25519 public generated {number}", "public", [left], left_public)
        agreed = left_private.exchange(right_private.public_key())
        add(f"X25519 left exchange {number}", "shared", [left, right_public], agreed)
        add(f"X25519 right exchange {number}", "shared", [right, left_public], agreed)
        try:
            arbitrary = left_private.exchange(X25519PublicKey.from_public_bytes(peer))
        except ValueError:
            arbitrary = "ERR"
        add(f"X25519 arbitrary peer {number}", "shared", [left, peer], arbitrary)

    # Exercise ignored coordinate bits, all noncanonical encodings, and the
    # special zero/one/-one peers against an independent X25519 implementation.
    prime = (1 << 255) - 19
    secret = rng.randbytes(32)
    private = X25519PrivateKey.from_private_bytes(secret)
    coordinates = [0, 1, 2, 9, prime - 2, prime - 1] + list(range(prime, 1 << 255))
    for coordinate in coordinates:
        for high_bit in [0, 1]:
            peer = (coordinate | (high_bit << 255)).to_bytes(32, "little")
            try:
                expected = private.exchange(X25519PublicKey.from_public_bytes(peer))
            except ValueError:
                expected = "ERR"
            add(f"X25519 peer boundary {coordinate}, high bit {high_bit}", "shared", [secret, peer], expected)
    secrets = [bytes(32), bytes([255]) * 32, bytes(range(32)), bytes([0x55]) * 32]
    secrets += [(1 << bit).to_bytes(32, "little") for bit in range(256)]
    for number, secret in enumerate(secrets):
        expected = public_bytes(X25519PrivateKey.from_private_bytes(secret))
        add(f"X25519 scalar boundary {number}", "public", [secret], expected)
    return checks


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustc", default=os.environ.get("RUSTC", "rustc"), help="Host Rust compiler")
    parser.add_argument("--cases", type=int, default=256, help="Generated case count, 1 through 4096")
    args = parser.parse_args()
    if not 1 <= args.cases <= 4096:
        parser.error("--cases must be between 1 and 4096")
    root = Path(__file__).resolve().parents[1]
    source = PROBE.replace("HASH_PATH", json.dumps(str(root / "src/crypto_hash.rs"), ensure_ascii=False))
    source = source.replace("CURVE_PATH", json.dumps(str(root / "src/crypto_x25519.rs"), ensure_ascii=False))
    checks = cases(args.cases)
    suffix = ".exe" if os.name == "nt" else ""
    with tempfile.TemporaryDirectory(prefix="own-browser-crypto-") as directory:
        temp = Path(directory)
        harness, vectors, probe = temp / "probe.rs", temp / ("vectors" + suffix), temp / ("probe" + suffix)
        harness.write_text(source, encoding="utf-8")
        flags = [args.rustc, "--edition=2021", "-O", "-C", "overflow-checks=yes", str(harness)]
        subprocess.run(flags + ["--test", "-o", str(vectors)], check=True, timeout=60)
        subprocess.run([str(vectors)], check=True, timeout=60)
        subprocess.run(flags + ["-o", str(probe)], check=True, timeout=60)
        result = subprocess.run([str(probe)], input="\n".join(line for _, line, _ in checks) + "\n",
                                text=True, capture_output=True, check=True, timeout=120)
        output = result.stdout.splitlines()
        if len(output) != len(checks):
            raise AssertionError(f"Probe returned {len(output)} lines for {len(checks)} checks: {result.stderr}")
        for (name, _, expected), actual in zip(checks, output):
            if actual != expected:
                raise AssertionError(f"{name}: expected {expected}, got {actual}")
    print(f"Independent SHA-256/HMAC/HKDF/X25519 checks passed: {len(checks)} cases.")
    print("Oracles: Python hashlib/hmac and cryptography; no browser runtime dependency added.")


if __name__ == "__main__":
    main()
