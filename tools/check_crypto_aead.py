#!/usr/bin/env python3
"""Development-only differential checks for the original AEAD module.

Requires rustc and Python's cryptography package as an independent test oracle.
Neither this script nor that package is part of the browser's runtime/build.
All generated keys/nonces/data are deterministic test fixtures, not real secrets.
"""

from __future__ import annotations

import os
from pathlib import Path
import random
import shutil
import subprocess
import tempfile

from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305
from cryptography.hazmat.primitives.poly1305 import Poly1305


DRIVER = r'''
fn oracle_bytes(value: &str) -> Result<Vec<u8>, String> {
    if value == "-" { return Ok(Vec::new()); }
    if !value.is_ascii() || value.len() % 2 != 0 { return Err("invalid hex".into()); }
    (0..value.len()).step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i+2], 16).map_err(|_| "invalid hex".into()))
        .collect()
}
fn oracle_run(fields: &[&str]) -> Result<Vec<u8>, String> {
    if fields.len() < 3 { return Err("missing arguments".into()); }
    let key: [u8; 32] = oracle_bytes(fields[1])?.try_into().map_err(|_| "key length")?;
    if fields[0] == "poly" && fields.len() == 3 {
        let input = oracle_bytes(fields[2])?;
        let mut mac = Poly1305::new(&key);
        for chunk in input.chunks(16) { mac.block(chunk); }
        return Ok(mac.finish().to_vec());
    }
    if fields.len() != 5 { return Err("argument count".into()); }
    let nonce: [u8; 12] = oracle_bytes(fields[2])?.try_into().map_err(|_| "nonce length")?;
    let aad = oracle_bytes(fields[3])?;
    let data = oracle_bytes(fields[4])?;
    match fields[0] {
        "seal" => seal(&key, &nonce, &aad, &data),
        "open" => open(&key, &nonce, &aad, &data),
        _ => Err("unknown command".into()),
    }
}
fn main() {
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let mut stdout = std::io::BufWriter::new(std::io::stdout().lock());
    for line in stdin.lock().lines() {
        let line = line.expect("read request");
        match oracle_run(&line.split_whitespace().collect::<Vec<_>>()) {
            Ok(bytes) => {
                write!(stdout, "OK ").unwrap();
                if bytes.is_empty() { write!(stdout, "-").unwrap(); }
                for byte in bytes { write!(stdout, "{byte:02x}").unwrap(); }
                writeln!(stdout).unwrap();
            }
            Err(error) => writeln!(stdout, "ERR {error}").unwrap(),
        }
        stdout.flush().unwrap();
    }
}
'''


def reference_poly(key: bytes, data: bytes) -> bytes:
    """Separate, direct integer interpretation of RFC 8439 section 2.5."""
    multiplier = int.from_bytes(key[:16], "little") & 0x0FFFFFFC0FFFFFFC0FFFFFFC0FFFFFFF
    accumulator = 0
    modulus = (1 << 130) - 5
    for offset in range(0, len(data), 16):
        block = int.from_bytes(data[offset : offset + 16] + b"\x01", "little")
        accumulator = ((accumulator + block) * multiplier) % modulus
    tag = (accumulator + int.from_bytes(key[16:], "little")) % (1 << 128)
    return tag.to_bytes(16, "little")


def main() -> None:
    root = Path(__file__).resolve().parent.parent
    rustc = shutil.which("rustc")
    if rustc is None:
        raise SystemExit("rustc must be available on PATH for this development check.")
    source = (root / "src" / "crypto_aead.rs").read_text(encoding="utf-8")
    random_data = random.Random(0x8439)
    exchanges = 0
    aead_cases = 0
    poly_cases = 0

    with tempfile.TemporaryDirectory(prefix="own-browser-aead-") as temporary:
        harness = Path(temporary) / "aead_oracle_driver.rs"
        executable = Path(temporary) / ("aead_oracle_driver.exe" if os.name == "nt" else "aead_oracle_driver")
        harness.write_text(source + "\n" + DRIVER, encoding="utf-8")
        subprocess.run(
            [rustc, "--edition=2021", "-O", "-C", "overflow-checks=yes", str(harness), "-o", str(executable)],
            check=True,
            timeout=60,
        )
        process = subprocess.Popen(
            [str(executable)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, text=True, encoding="ascii",
        )

        def query(operation: str, *arguments: bytes, reject: bool = False) -> bytes:
            nonlocal exchanges
            request = " ".join([operation, *(argument.hex() or "-" for argument in arguments)])
            assert process.stdin is not None and process.stdout is not None
            process.stdin.write(request + "\n")
            process.stdin.flush()
            reply = process.stdout.readline().strip()
            exchanges += 1
            if reject:
                assert reply.startswith("ERR "), f"expected rejection for {operation}: {reply[:100]}"
                return b""
            assert reply.startswith("OK "), f"{operation} failed: {reply[:200]}"
            result = reply[3:]
            return b"" if result == "-" else bytes.fromhex(result)

        def check_aead(aad_length: int, text_length: int) -> None:
            nonlocal aead_cases
            key = random_data.randbytes(32)
            nonce = random_data.randbytes(12)
            aad = random_data.randbytes(aad_length)
            text = random_data.randbytes(text_length)
            oracle = ChaCha20Poly1305(key)
            expected = oracle.encrypt(nonce, text, aad)
            actual = query("seal", key, nonce, aad, text)
            assert actual == expected, f"seal mismatch for AAD={aad_length}, text={text_length}"
            assert query("open", key, nonce, aad, expected) == text
            assert oracle.decrypt(nonce, actual, aad) == text
            damaged = bytearray(expected)
            damaged[random_data.randrange(len(damaged))] ^= 1 << random_data.randrange(8)
            query("open", key, nonce, aad, bytes(damaged), reject=True)
            aead_cases += 1

        def check_poly(key: bytes, data: bytes) -> None:
            nonlocal poly_cases
            expected = Poly1305.generate_tag(key, data)
            assert reference_poly(key, data) == expected
            assert query("poly", key, data) == expected, f"Poly1305 mismatch at {len(data)} bytes"
            poly_cases += 1

        try:
            boundaries = [0, 1, 2, 7, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129]
            for aad_length in boundaries:
                for text_length in boundaries:
                    check_aead(aad_length, text_length)
            for _ in range(400):
                check_aead(random_data.randrange(513), random_data.randrange(8193))
            for length in [255, 256, 257, 4096, 16383, 16384, 16385, 65536, 1024 * 1024]:
                check_aead(13, length)
                check_aead(length, 17)
            check_aead(1024 * 1024, 1024 * 1024)

            for length in boundaries + [255, 256, 257, 4095, 4096, 4097, 16384]:
                for value in [0, 1, 0x7F, 0x80, 0xFE, 0xFF]:
                    check_poly(bytes([value]) * 32, bytes([value]) * length)
            for _ in range(800):
                check_poly(random_data.randbytes(32), random_data.randbytes(random_data.randrange(4097)))

            key, nonce = bytes(32), bytes(12)
            aad, text = b"test-only AAD", b"test-only plaintext"
            sealed = ChaCha20Poly1305(key).encrypt(nonce, text, aad)
            for index in range(len(sealed) * 8):
                modified = bytearray(sealed)
                modified[index // 8] ^= 1 << (index % 8)
                query("open", key, nonce, aad, bytes(modified), reject=True)
            for length in range(len(sealed)):
                query("open", key, nonce, aad, sealed[:length], reject=True)
            query("open", key, nonce, aad, sealed + b"\0", reject=True)
            query("open", key, nonce, aad + b"\0", sealed, reject=True)
            query("open", bytes([1]) + key[1:], nonce, aad, sealed, reject=True)
            query("open", key, bytes([1]) + nonce[1:], aad, sealed, reject=True)
            over_limit = bytes(1024 * 1024 + 1)
            query("seal", key, nonce, over_limit, b"", reject=True)
            query("seal", key, nonce, b"", over_limit, reject=True)
            query("open", key, nonce, over_limit, bytes(16), reject=True)
            query("open", key, nonce, b"", over_limit + bytes(16), reject=True)
        finally:
            if process.stdin is not None:
                process.stdin.close()
            try:
                result = process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
                raise
            if result:
                assert process.stderr is not None
                raise RuntimeError(process.stderr.read())

    print(f"PASS: {aead_cases} AEAD cases matched cryptography, including reciprocal decryption.")
    print(f"PASS: {poly_cases} Poly1305 cases matched cryptography and a separate integer oracle.")
    print(f"PASS: {exchanges} total Rust requests including corruption, truncation, and size rejection.")
    print("This is correctness evidence for an experimental primitive, not a security audit.")


if __name__ == "__main__":
    main()
