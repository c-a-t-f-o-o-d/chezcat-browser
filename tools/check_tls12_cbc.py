#!/usr/bin/env python3
"""Independent offline checks for original AES decryption and TLS 1.2 CBC.

    python tools/check_tls12_cbc.py --output /tmp/tls12-cbc-report.json

Requires a host Rust compiler and Python cryptography for the development
oracle. Neither is a browser runtime dependency. The temporary Rust probe
imports production modules directly. Python's AES-CBC, HMAC-SHA256/SHA384 and
independently assembled wire records exercise both Encrypt-then-MAC and
MAC-then-encrypt, every legal TLS padding value, boundaries, tampering and
authenticated malformed content. No network or installed trust store is used.

The optional microbenchmark reports throughput, not a timing-attack proof.
Connection sequence advancement/poisoning is covered by check_tls12.py because
the CBC primitive deliberately leaves that state with the connection wrapper.
"""
from __future__ import annotations

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

from cryptography.hazmat.backends.openssl.backend import backend
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

ROOT = Path(__file__).resolve().parents[1]
MODULES = ["crypto_hash", "crypto_sha512", "crypto_aead", "crypto_aes_gcm", "tls_crypto", "tls12_cbc"]
BAD_RECORD = "error TLS 1.2 CBC record authentication failed."
PROBE = r'''
use std::io::{self,BufRead,Write};
fn bytes(value:&str)->Vec<u8>{
 if value=="-" {return vec![];}
 (0..value.len()).step_by(2).map(|i|u8::from_str_radix(&value[i..i+2],16).unwrap()).collect()
}
fn kind(value:&str)->tls_crypto::ContentType {
 match value {"21"=>tls_crypto::ContentType::Alert,"22"=>tls_crypto::ContentType::Handshake,
 "23"=>tls_crypto::ContentType::ApplicationData,_=>panic!("probe type")}
}
fn run(values:&[&str])->Result<Vec<u8>,String>{
 if values[0].starts_with("aes-") {
  let key=crypto_aes_gcm::Aes::new(&bytes(values[1]))?;
  let block:[u8;16]=bytes(values[2]).try_into().map_err(|_|"probe block length")?;
  return Ok(if values[0]=="aes-encrypt" {key.encrypt(&block)} else {key.decrypt(&block)}.to_vec());
 }
 let keys=tls12_cbc::Keys::new(&bytes(values[1]),&bytes(values[2]),
   bytes(values[3]).try_into().map_err(|_|"probe IV key length")?,values[4]=="384",values[5]=="1")?;
 let sequence=values[6].parse::<u64>().map_err(|e|e.to_string())?;
 match values[0] {
 "seal"=>keys.seal(sequence,kind(values[7]),&bytes(values[8])),
 "open"=>{let record=keys.open(sequence,&bytes(values[7]))?;
   let mut result=vec![record.content_type as u8];result.extend_from_slice(&record.content);Ok(result)},
 "bench"=>{
   let size=values[7].parse::<usize>().unwrap();let count=values[8].parse::<u32>().unwrap();
   let wire=keys.seal(sequence,tls_crypto::ContentType::ApplicationData,&vec![42;size])?;
   let start=std::time::Instant::now();let mut check=0usize;
   for _ in 0..count {check^=std::hint::black_box(keys.open(sequence,&wire)?).content.len();}
   std::hint::black_box(check);
   Ok(format!("{}",start.elapsed().as_nanos()/u128::from(count)).into_bytes())
 },
 _=>Err("probe operation".into())
 }
}
fn main(){
 let mut output=io::BufWriter::new(io::stdout().lock());
 for line in io::stdin().lock().lines(){
  let line=line.unwrap();let values:Vec<_>=line.split_whitespace().collect();
  match run(&values){Ok(result)=>{write!(output,"ok").unwrap();if !result.is_empty(){write!(output," ").unwrap();}
   for byte in result {write!(output,"{byte:02x}").unwrap();}writeln!(output).unwrap();},
   Err(error)=>writeln!(output,"error {error}").unwrap()}
 }
}
'''


def encoded(value):
    return value.hex() or "-"


def aes(key, iv, content, encrypt=True):
    cipher = Cipher(algorithms.AES(key), modes.CBC(iv))
    operation = cipher.encryptor() if encrypt else cipher.decryptor()
    return operation.update(content) + operation.finalize()


def mac(key, digest, sequence, kind, fragment):
    prefix = sequence.to_bytes(8, "big") + bytes([kind, 3, 3]) + len(fragment).to_bytes(2, "big")
    return hmac.new(key, prefix + fragment, digest).digest()


def frame(kind, payload):
    return bytes([kind, 3, 3]) + len(payload).to_bytes(2, "big") + payload


def protect(key, mac_key, digest, etm, sequence, kind, content, iv, padding=None, mutate=None):
    inner = bytearray(content)
    if not etm:
        inner.extend(mac(mac_key, digest, sequence, kind, content))
    if padding is None:
        padding = (-len(inner) - 1) % 16
    inner.extend(bytes([padding]) * (padding + 1))
    assert len(inner) % 16 == 0
    if mutate is not None:
        mutate(inner)
    payload = iv + aes(key, iv, bytes(inner))
    if etm:
        payload += mac(mac_key, digest, sequence, kind, payload)
    return frame(kind, payload)


def run(rustc, benchmark):
    started = time.monotonic()
    rng = random.Random(0x130cbc)
    cases = []

    def add(category, command, wanted):
        if isinstance(wanted, bytes):
            wanted = "ok" + (" " + wanted.hex() if wanted else "")
        cases.append((category, command, wanted))

    # Full block outputs come from an independent AES-ECB implementation.
    for length in (16, 32):
        for _ in range(256):
            key, plain = rng.randbytes(length), rng.randbytes(16)
            worker = Cipher(algorithms.AES(key), modes.ECB()).encryptor()
            cipher = worker.update(plain) + worker.finalize()
            add("aes-encryption-reference", f"aes-encrypt {key.hex()} {plain.hex()}", cipher)
            add("aes-decryption-reference", f"aes-decrypt {key.hex()} {cipher.hex()}", plain)

    for bits in (256, 384):
        digest = "sha256" if bits == 256 else "sha384"
        key_length, mac_length = (16, 32) if bits == 256 else (32, 48)
        key, mac_key, iv_key = rng.randbytes(key_length), rng.randbytes(mac_length), rng.randbytes(32)
        for etm in (False, True):
            prefix = f"{key.hex()} {mac_key.hex()} {iv_key.hex()} {bits} {int(etm)}"

            def opening(category, wire, wanted, sequence=7, keys=prefix):
                add(category, f"open {keys} {sequence} {encoded(wire)}", wanted)

            for length in list(range(81)) + [95, 96, 111, 112, 127, 128, 129, 255, 256, 257,
                                            511, 512, 513, 1024, 16383, 16384]:
                content = rng.randbytes(length)
                iv = hmac.new(iv_key, b"OwnBrowser TLS 1.2 CBC IV\0" + (7).to_bytes(8, "big"), "sha256").digest()[:16]
                wire = protect(key, mac_key, digest, etm, 7, 23, content, iv)
                add("seal-reference", f"seal {prefix} 7 23 {encoded(content)}", wire)
                opening("open-reference", wire, bytes([23]) + content)
                # A server's explicit IV is independent of the client's generator.
                random_iv_wire = protect(key, mac_key, digest, etm, 7, 23, content, rng.randbytes(16))
                opening("arbitrary-peer-iv", random_iv_wire, bytes([23]) + content)

            for padding in range(256):
                content_length = 64 + (-(padding + 1 + (0 if etm else mac_length))) % 16
                content = rng.randbytes(content_length)
                iv = rng.randbytes(16)
                wire = protect(key, mac_key, digest, etm, 7, 23, content, iv, padding)
                opening("all-padding-values", wire, bytes([23]) + content)

                def break_padding(inner):
                    if padding == 0:
                        inner[-1], inner[-2] = 1, 0x42
                    else:
                        inner[-padding - 1] ^= 1

                bad = protect(key, mac_key, digest, etm, 7, 23, content, iv, padding, break_padding)
                opening("invalid-padding-with-valid-mac", bad, BAD_RECORD)

            # Maximum plaintext plus the largest legal non-minimal TLS padding.
            content = rng.randbytes(16384)
            wire = protect(key, mac_key, digest, etm, 7, 23, content, rng.randbytes(16), 255)
            opening("maximum-plaintext-and-padding", wire, bytes([23]) + content)

            content = b"independent authenticated record"
            valid = protect(key, mac_key, digest, etm, 7, 23, content, rng.randbytes(16))
            for index in range(len(valid)):
                damaged = bytearray(valid)
                damaged[index] ^= 1
                opening("tamper-every-wire-byte", damaged, BAD_RECORD)
            for length in range(len(valid)):
                opening("truncation-every-length", valid[:length], BAD_RECORD)
            for sequence in (0, 1, 6, 8, 65535, 1 << 20, (1 << 64) - 1):
                opening("replay-wrong-sequence", valid, BAD_RECORD, sequence)
            for extra in (b"\x00", b"\x00" * 15, b"\x00" * 16):
                opening("framed-noncanonical-ciphertext", frame(23, valid[5:] + extra), BAD_RECORD)
            opening("wire-length-overflow", bytes([23, 3, 3, 0x41, 0x41]) + bytes(16705), BAD_RECORD)
            opening("wrong-encryption-key", valid, BAD_RECORD,
                    keys=f"{bytes(key_length).hex()} {mac_key.hex()} {iv_key.hex()} {bits} {int(etm)}")
            opening("wrong-mac-key", valid, BAD_RECORD,
                    keys=f"{key.hex()} {bytes(mac_length).hex()} {iv_key.hex()} {bits} {int(etm)}")
            other = f"{key.hex()} {mac_key.hex()} {iv_key.hex()} {bits} {int(not etm)}"
            opening("wrong-record-mode", valid, BAD_RECORD, keys=other)

            for sequence in (0, 1, 255, 256, 65535, (1 << 20) - 1):
                iv = hmac.new(iv_key, b"OwnBrowser TLS 1.2 CBC IV\0" + sequence.to_bytes(8, "big"), "sha256").digest()[:16]
                wire = protect(key, mac_key, digest, etm, sequence, 23, b"seq", iv)
                add("iv-sequence-reference", f"seal {prefix} {sequence} 23 736571", wire)
                opening("sequence-boundary", wire, b"\x17seq", sequence)

            for kind, data, valid_content in [(21, b"\x01\x00", True), (21, b"\x02\x14", True),
                                              (22, b"\x14\x00\x00\x0c" + bytes(12), True),
                                              (21, b"", False), (21, b"\x01", False),
                                              (21, b"\x03\x00", False), (21, b"\x01\x00\x00", False),
                                              (22, b"", False), (20, b"\x01", False), (24, b"abc", False)]:
                wire = protect(key, mac_key, digest, etm, 7, kind, data, rng.randbytes(16))
                opening("authenticated-content-policy", wire, bytes([kind]) + data if valid_content else BAD_RECORD)
            oversized = protect(key, mac_key, digest, etm, 7, 23, bytes(16385), rng.randbytes(16))
            opening("authenticated-oversized-content", oversized, BAD_RECORD)
            for content_length in (0, 1, 16384):
                add("seal-exhausted-sequence", f"seal {prefix} {1<<20} 23 {encoded(bytes(content_length))}", "error")
            add("seal-oversized-content", f"seal {prefix} 0 23 {encoded(bytes(16385))}", "error")

            if not etm:
                for offset in range(mac_length):
                    def break_mac(inner):
                        inner[len(content) + offset] ^= 0x80
                    broken = protect(key, mac_key, digest, etm, 7, 23, content, rng.randbytes(16), mutate=break_mac)
                    opening("invalid-inner-mac", broken, BAD_RECORD)

        for wrong_length in (0, 1, 15, 17, 31, 33, 47, 49):
            add("invalid-mac-key-length", f"seal {key.hex()} {encoded(bytes(wrong_length))} {iv_key.hex()} {bits} 1 0 23 -", "error")
        for wrong_length in (0, 1, 15, 17, 31, 33):
            add("invalid-aes-key-length", f"seal {encoded(bytes(wrong_length))} {mac_key.hex()} {iv_key.hex()} {bits} 1 0 23 -", "error")

    source_hashes = {name: hashlib.sha256((ROOT / "src" / (name + ".rs")).read_bytes()).hexdigest() for name in MODULES}
    with tempfile.TemporaryDirectory(prefix="chezcat-cbc-check-") as temporary:
        path = Path(temporary)
        source = path / "probe.rs"
        # JSON string escaping also works for these absolute Rust path literals.
        source.write_text("#![allow(dead_code)]\n" + "\n".join(
            f"#[path={json.dumps(str(ROOT / 'src' / (name + '.rs')))}] mod {name};" for name in MODULES
        ) + PROBE, encoding="utf-8")
        executable = path / ("probe.exe" if os.name == "nt" else "probe")
        subprocess.run([rustc, "--edition=2021", "-O", str(source), "-o", str(executable)], check=True,
                       text=True, capture_output=True)
        result = subprocess.run([str(executable)], input="\n".join(command for _, command, _ in cases) + "\n",
                                capture_output=True, text=True, check=True, timeout=240)
        lines = result.stdout.splitlines()
        if len(lines) != len(cases):
            raise AssertionError(f"Probe returned {len(lines)} lines for {len(cases)} cases: {result.stderr}")
        failures = []
        for index, ((category, _, wanted), actual) in enumerate(zip(cases, lines)):
            passed = actual.startswith("error ") if wanted == "error" else actual == wanted
            if not passed:
                failures.append({"index": index, "category": category,
                                 "expected": wanted[:200], "actual": actual[:200]})
        measurements = []
        if benchmark:
            commands = []
            profiles = []
            for bits in (256, 384):
                key_length, mac_length = (16, 32) if bits == 256 else (32, 48)
                for etm in (False, True):
                    for length in (256, 4096, 16384):
                        profiles.append((bits, etm, length))
                        prefix = f"{bytes(key_length).hex()} {bytes(mac_length).hex()} {bytes(32).hex()} {bits} {int(etm)}"
                        commands.append(f"bench {prefix} 3 {length} 12")
            observed = subprocess.run([str(executable)], input="\n".join(commands) + "\n", capture_output=True,
                                      text=True, check=True, timeout=120).stdout.splitlines()
            for (bits, etm, length), line in zip(profiles, observed):
                nanos = int(bytes.fromhex(line.removeprefix("ok ")).decode("ascii"))
                measurements.append({"hash": bits, "encrypt_then_mac": etm, "content_bytes": length,
                                     "open_microseconds": round(nanos / 1000, 2),
                                     "open_mebibytes_per_second": round(length * 1e9 / nanos / 1048576, 3)})
    return {"passed": not failures, "cases": len(cases), "categories": dict(Counter(category for category, _, _ in cases)),
            "failures": failures, "source_sha256": source_hashes, "oracle": backend.openssl_version_text(),
            "elapsed_seconds": round(time.monotonic() - started, 3), "benchmarks": measurements,
            "limits": "Correctness/interoperability checks; not a compiler/CPU timing proof or cryptographic audit."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustc", default=os.environ.get("RUSTC", "rustc"))
    parser.add_argument("--output", type=Path)
    parser.add_argument("--benchmark", action="store_true")
    args = parser.parse_args()
    report = run(args.rustc, args.benchmark)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    raise SystemExit(0 if report["passed"] else 1)


if __name__ == "__main__":
    main()
