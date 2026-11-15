#!/usr/bin/env python3
"""Independent, offline AES-128/AES-256 GCM checks and a small timing sample.

Run from any directory with Python cryptography and a host Rust compiler:
    python tools/check_crypto_aes_gcm.py [--rustc /path/to/rustc]

The browser does not import, invoke, or link this checker or its Python oracle.
The probe and executable are temporary. No network is used. Seeded fixtures
compare AES blocks and GCM records with cryptography, and GHASH multiplication
with a separate Python polynomial-product/remainder calculation. The Rust unit
tests additionally contain FIPS 197 and all ten applicable NIST AES_GCM.pdf
examples for 128/256-bit keys and 128-bit tags.
"""

import argparse
from collections import Counter
import json
import os
from pathlib import Path
import random
import subprocess
import tempfile
import time

from cryptography.exceptions import InvalidTag
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

ROOT = Path(__file__).resolve().parents[1]
MAX_INPUT = 1024 * 1024

PROBE = r'''
#![allow(dead_code)]
#[path=MODULE] mod crypto_aes_gcm;
use std::io::{self,BufRead,Write};
fn unhex(s:&str)->Vec<u8>{s.as_bytes().as_chunks::<2>().0.iter().map(|b|u8::from_str_radix(std::str::from_utf8(b).unwrap(),16).unwrap()).collect()}
fn hex(v:&[u8])->String{let mut s=String::new();for b in v{use std::fmt::Write;write!(&mut s,"{b:02x}").unwrap();}s}
fn answer(result:Result<Vec<u8>,String>)->String{result.map(|v|hex(&v)).unwrap_or_else(|_|"ERR".into())}
fn bench(key_length:usize)->String{
 let key=vec![0x73;key_length];let plaintext=vec![0x61;16384];let aad=b"\x17\x03\x03\x40\x10";
 let rounds=32u64;let start=std::time::Instant::now();let mut check=0u8;
 for counter in 0..rounds{let mut nonce=[0u8;12];nonce[4..].copy_from_slice(&counter.to_be_bytes());
  let sealed=crypto_aes_gcm::seal(&key,&nonce,aad,std::hint::black_box(&plaintext)).unwrap();
  let opened=crypto_aes_gcm::open(&key,&nonce,aad,std::hint::black_box(&sealed)).unwrap();
  assert_eq!(opened,plaintext);check^=std::hint::black_box(sealed[0]);}
 std::hint::black_box(check);let seconds=start.elapsed().as_secs_f64();
 format!("{{\"key_bits\":{},\"record_bytes\":16384,\"round_trips\":{},\"seconds\":{},\"round_trip_ms\":{},\"payload_mib_per_second\":{}}}",key_length*8,rounds,seconds,seconds*1000.0/(rounds as f64),0.5/seconds)
}
fn main(){let mut out=io::BufWriter::new(io::stdout().lock());for line in io::stdin().lock().lines(){let line=line.unwrap();let f:Vec<_>=line.split('\t').collect();let result=match f[0]{
 "seal"=>answer(crypto_aes_gcm::seal(&unhex(f[1]),&unhex(f[2]).try_into().unwrap(),&unhex(f[3]),&unhex(f[4]))),
 "open"=>answer(crypto_aes_gcm::open(&unhex(f[1]),&unhex(f[2]).try_into().unwrap(),&unhex(f[3]),&unhex(f[4]))),
 "block"=>crypto_aes_gcm::oracle_block(&unhex(f[1]),&unhex(f[2]).try_into().unwrap()).map(|v|hex(&v)).unwrap_or_else(|_|"ERR".into()),
 "ghash"=>hex(&crypto_aes_gcm::oracle_product(&unhex(f[1]).try_into().unwrap(),&unhex(f[2]).try_into().unwrap())),
 "bench"=>bench(f[1].parse().unwrap()),_=>panic!("unknown operation")};writeln!(out,"{result}").unwrap();}}
'''


def polynomial_product(left, right):
    # GHASH's encoded most-significant bit denotes coefficient x^0. Reverse
    # that representation, multiply whole polynomials, then take a remainder.
    a = int(f"{int.from_bytes(left, 'big'):0128b}"[::-1], 2)
    b = int(f"{int.from_bytes(right, 'big'):0128b}"[::-1], 2)
    product = 0
    for bit in range(128):
        if b & (1 << bit):
            product ^= a << bit
    modulus = (1 << 128) | 0x87
    for bit in range(254, 127, -1):
        if product & (1 << bit):
            product ^= modulus << (bit - 128)
    return int(f"{product:0128b}"[::-1], 2).to_bytes(16, "big")


def run(rustc):
    started = time.monotonic()
    rng = random.Random(0x41455347434D)
    cases = []

    def add(label, operation, fields, expected):
        fields = [value.hex() if isinstance(value, bytes) else str(value) for value in fields]
        expected = expected.hex() if isinstance(expected, bytes) else expected
        cases.append((label, "\t".join([operation] + fields), expected))

    def reject(label, key, nonce, aad, sealed):
        try:
            AESGCM(key).decrypt(nonce, sealed, aad)
        except InvalidTag:
            pass
        else:
            raise AssertionError("Invalid fixture authenticated: " + label)
        add(label, "open", [key, nonce, aad, sealed], "ERR")

    def example(label, key, nonce, aad, plain, mutations=True):
        sealed = AESGCM(key).encrypt(nonce, plain, aad)
        add(label, "seal", [key, nonce, aad, plain], sealed)
        add(label, "open", [key, nonce, aad, sealed], plain)
        if not mutations:
            return
        changed = sealed[:-1] + bytes([sealed[-1] ^ 1])
        reject(label + " tag", key, nonce, aad, changed)
        changed_nonce = nonce[:-1] + bytes([nonce[-1] ^ 1])
        reject(label + " nonce", key, changed_nonce, aad, sealed)
        changed_aad = bytes([aad[0] ^ 1]) + aad[1:] if aad else b"\0"
        reject(label + " AAD", key, nonce, changed_aad, sealed)
        if plain:
            changed = bytes([sealed[0] ^ 1]) + sealed[1:]
            reject(label + " ciphertext", key, nonce, aad, changed)

    lengths = [0, 1, 2, 7, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257, 16383, 16384, 16385]
    for key_length in [16, 32]:
        for length in lengths:
            for aad_length in [0, 1, 15, 16, 17, 31, 32, 33]:
                example("length/padding boundary", rng.randbytes(key_length), rng.randbytes(12),
                        rng.randbytes(aad_length), rng.randbytes(length))
        for _ in range(48):
            example("generated", rng.randbytes(key_length), rng.randbytes(12),
                    rng.randbytes(rng.randrange(300)), rng.randbytes(rng.randrange(3000)))
        for fill in [0, 0xff]:
            example("uniform bytes", bytes([fill]) * key_length, rng.randbytes(12),
                    bytes([fill]) * 33, bytes([fill]) * 65)
        for plain_length, aad_length in [(MAX_INPUT, 0), (0, MAX_INPUT), (MAX_INPUT, MAX_INPUT)]:
            example("exact maximum input", rng.randbytes(key_length), rng.randbytes(12),
                    rng.randbytes(aad_length), rng.randbytes(plain_length), mutations=False)
        key, nonce, aad, plain = rng.randbytes(key_length), rng.randbytes(12), rng.randbytes(29), rng.randbytes(33)
        sealed = AESGCM(key).encrypt(nonce, plain, aad)
        for index in range(key_length):
            changed = bytearray(key)
            changed[index] ^= 1
            reject("every key byte", bytes(changed), nonce, aad, sealed)
        for index in range(16):
            changed = bytearray(sealed)
            changed[len(plain) + index] ^= 1
            reject("every tag byte", key, nonce, aad, bytes(changed))
        for candidate in [sealed[:-1], sealed + b"\0", sealed[:12], sealed[1:]]:
            reject("truncated/extended sealed input", key, nonce, aad, candidate)
        for _ in range(96):
            block, key = rng.randbytes(16), rng.randbytes(key_length)
            cipher = Cipher(algorithms.AES(key), modes.ECB()).encryptor()
            add("AES block vs cryptography", "block", [key, block], cipher.update(block) + cipher.finalize())

    special = [bytes(16), b"\x80" + bytes(15), bytes(15) + b"\x01", b"\xff" * 16]
    for a in special:
        for b in special:
            add("GHASH polynomial boundary", "ghash", [a, b], polynomial_product(a, b))
    for _ in range(128):
        a, b = rng.randbytes(16), rng.randbytes(16)
        add("GHASH vs polynomial remainder", "ghash", [a, b], polynomial_product(a, b))

    nonce = bytes(12)
    for length in list(range(0, 40)) + [64, 1024]:
        if length not in [16, 32]:
            add("unsupported key size", "seal", [bytes(length), nonce, b"", b""], "ERR")
            add("unsupported key size", "open", [bytes(length), nonce, b"", bytes(16)], "ERR")
    for size in range(16):
        add("missing complete tag", "open", [bytes(16), nonce, b"", bytes(size)], "ERR")
    oversized = bytes(MAX_INPUT + 1)
    for key_length in [16, 32]:
        key = bytes(key_length)
        add("plaintext cap", "seal", [key, nonce, b"", oversized], "ERR")
        add("AAD cap on seal", "seal", [key, nonce, oversized, b""], "ERR")
        add("AAD cap on open", "open", [key, nonce, oversized, bytes(16)], "ERR")
        add("ciphertext cap", "open", [key, nonce, b"", oversized + bytes(16)], "ERR")

    with tempfile.TemporaryDirectory(prefix="aes-gcm-check-") as temporary:
        temp = Path(temporary)
        # Hooks are appended only to a temporary copy. The browser exposes no
        # raw AES or standalone GHASH API and gains no test-only dependency.
        module = (ROOT / "src/crypto_aes_gcm.rs").read_text(encoding="utf-8")
        module += "\npub fn oracle_block(key:&[u8],block:&[u8;16])->Result<[u8;16],String>{Ok(Aes::new(key)?.encrypt(block))}\n"
        module += "pub fn oracle_product(a:&[u8;16],b:&[u8;16])->[u8;16]{ghash_multiply(u128::from_be_bytes(*a),u128::from_be_bytes(*b)).to_be_bytes()}\n"
        module_path = temp / "aes.rs"
        module_path.write_text(module, encoding="utf-8")
        source = temp / "probe.rs"
        source.write_text(PROBE.replace("MODULE", json.dumps(str(module_path), ensure_ascii=False)), encoding="utf-8")
        executable = temp / ("probe.exe" if os.name == "nt" else "probe")
        subprocess.run([rustc, "--edition=2021", "-O", "-C", "overflow-checks=yes", str(source), "-o", str(executable)], check=True, timeout=60)
        result = subprocess.run([str(executable)], input="\n".join(case[1] for case in cases) + "\n",
                                text=True, capture_output=True, check=True, timeout=120)
        actual = result.stdout.splitlines()
        assert len(actual) == len(cases), (len(actual), len(cases), result.stderr)
        failures = [{"case": name, "expected": expected[:160], "actual": answer[:160]}
                    for (name, _, expected), answer in zip(cases, actual) if answer != expected]
        assert not failures, failures[:10]
        timed = subprocess.run([str(executable)], input="bench\t16\nbench\t32\n", text=True,
                               capture_output=True, check=True, timeout=30)
        timings = [json.loads(line) for line in timed.stdout.splitlines()]
        report = {"cases": len(cases), "operations": dict(Counter(case[1].split("\t", 1)[0] for case in cases)),
                  "failures": failures, "seconds": round(time.monotonic() - started, 2),
                  "timing_sample": timings}
        print(json.dumps(report, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustc", default=os.environ.get("RUSTC", "rustc"), help="Host compiler (default: RUSTC or rustc from PATH)")
    args = parser.parse_args()
    run(args.rustc)
    print("Independent AES-GCM checks passed; no browser runtime dependency added.")


if __name__ == "__main__":
    main()
