#!/usr/bin/env python3
"""Independent checks for our shared DEFLATE, zlib and gzip decoder.

Development only: Python's standard-library zlib/gzip implementations produce
the inputs and expected bytes. The browser has no zlib or Python dependency.
Run after installing Rust: python tools/check_compression.py [--rustc PATH]
"""

from __future__ import annotations

import argparse
from collections import Counter
import gzip
import json
import os
from pathlib import Path
import random
import struct
import subprocess
import tempfile
import time
import zlib


ROOT = Path(__file__).resolve().parents[1]
PROBE = r'''
#![allow(dead_code)]
#[path=SOURCE] mod compression;
use std::cell::Cell;
use std::io::{self, BufRead, Write};
fn unhex(value: &str) -> Vec<u8> {
    value.as_bytes().as_chunks::<2>().0.iter().map(|pair|
        u8::from_str_radix(std::str::from_utf8(pair).unwrap(),16).unwrap()).collect()
}
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len()*2);
    const DIGITS: &[u8;16] = b"0123456789abcdef";
    for &byte in bytes { out.push(DIGITS[(byte>>4) as usize] as char); out.push(DIGITS[(byte&15) as usize] as char); }
    out
}
fn main() {
    let mut out = io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let line = line.unwrap(); let fields: Vec<_> = line.split('\t').collect();
        let limit: usize = fields[1].parse().unwrap();
        let stop: usize = fields[2].parse().unwrap(); let count = Cell::new(0);
        let check = || { let current=count.get()+1; count.set(current);
            if stop!=0 && current>=stop { Err("cancelled".to_owned()) } else { Ok(()) } };
        let data = unhex(fields[3]);
        let result = if fields[0]=="zlib" { compression::zlib(&data,limit,&check) }
            else { compression::gzip(&data,limit,&check) };
        match result {
            Ok(bytes) => writeln!(out,"OK:{}",hex(&bytes)).unwrap(),
            Err(error) => writeln!(out,"ERR:{error}").unwrap(),
        }
    }
}
'''


def wrapped(raw, level, strategy, wbits):
    compressor = zlib.compressobj(level, zlib.DEFLATED, wbits, 8, strategy)
    return compressor.compress(raw) + compressor.flush()


def options(raw, flags):
    body = gzip.compress(raw, mtime=0)[10:]
    header = bytearray([31, 139, 8, flags, 0, 0, 0, 0, 0, 255])
    if flags & 4:
        extra = b"OB\x04\x00test"
        header += struct.pack("<H", len(extra)) + extra
    if flags & 8:
        header += b"page.html\0"
    if flags & 16:
        header += b"Original test comment\0"
    if flags & 2:
        header += struct.pack("<H", zlib.crc32(header) & 0xffff)
    return bytes(header) + body


def tests():
    rng = random.Random(0x195019511952)
    cases = []

    def add(label, kind, encoded, expected, limit=None, stop=0, error=None):
        if limit is None:
            limit = len(expected) if expected is not None else 2 * 1024 * 1024
        cases.append((label, kind, encoded, expected, limit, stop, error))

    for size in [0, 1, 2, 31, 257, 4096, 8191, 8192, 8193, 32768, 65535, 65536]:
        for pattern in ("random", "repeated", "mixed"):
            raw = (rng.randbytes(size) if pattern == "random" else
                   (b"abc123 " * ((size + 6) // 7))[:size] if pattern == "repeated" else
                   (bytes(range(256)) * ((size + 255) // 256))[:size])
            for level, strategy in [(0, zlib.Z_DEFAULT_STRATEGY), (6, zlib.Z_FIXED),
                                    (6, zlib.Z_DEFAULT_STRATEGY), (9, zlib.Z_HUFFMAN_ONLY)]:
                for kind, wbits in [("zlib", 15), ("gzip", 31)]:
                    encoded = wrapped(raw, level, strategy, wbits)
                    assert zlib.decompress(encoded, wbits) == raw
                    add(f"{pattern}/{size}/level{level}/strategy{strategy}", kind, encoded, raw)

    # All optional flag combinations, including the low CRC32 header bits.
    raw = b"Header fields do not alter body bytes.\x00\xff"
    for flags in range(32):
        encoded = options(raw, flags)
        assert zlib.decompress(encoded, 31) == raw
        add(f"optional-flags/{flags}", "gzip", encoded, raw)
        if flags & 2:
            changed = bytearray(encoded)
            changed[4] ^= 1  # MTIME is otherwise ignorable, but FHCRC authenticates it.
            add(f"bad-header-crc/{flags}", "gzip", changed, None, error="header CRC")

    for count in (1, 2, 8, 128):
        parts = [rng.randbytes(index % 79) for index in range(count)]
        encoded = b"".join(gzip.compress(part, mtime=0) for part in parts)
        assert gzip.decompress(encoded) == b"".join(parts)
        add(f"members/{count}", "gzip", encoded, b"".join(parts))
    add("too-many-members", "gzip", gzip.compress(b"", mtime=0) * 129, None, error="member")

    for kind, encoded in [("zlib", zlib.compress(raw)), ("gzip", options(raw, 31))]:
        for length in range(len(encoded)):
            add(f"truncated/{length}", kind, encoded[:length], None)
        for suffix in (b"\0", b"garbage", b"\x1f\x8b"):
            add("trailing/" + suffix.hex(), kind, encoded + suffix, None)
        for offset in range(1, 5 if kind == "zlib" else 9):
            changed = bytearray(encoded)
            changed[-offset] ^= 1
            add(f"bad-trailer/{offset}", kind, changed, None)
        add("limit/exact", kind, encoded, raw)
        add("limit/extra-space", kind, encoded, raw, limit=len(raw) + 1)
        add("limit/one-short", kind, encoded, None, limit=len(raw) - 1, error="limit")
        add("limit/zero", kind, encoded, None, limit=0, error="limit")
        add("limit/global-cap", kind, encoded, None, limit=32 * 1024 * 1024 + 1, error="limit")
        add("cancel/start", kind, encoded, None, stop=1, error="cancelled")
    for kind, wbits in [("zlib", 15), ("gzip", 31)]:
        large = rng.randbytes(100_000)
        encoded = wrapped(large, 6, zlib.Z_FIXED, wbits)
        add("cancel/decode", kind, encoded, None, stop=6, error="cancelled")
        add("large-fixed", kind, encoded, large)
        encoded = wrapped(large, 0, zlib.Z_DEFAULT_STRATEGY, wbits)
        add("large-stored", kind, encoded, large)

    # Neither RFC wrapper permits these misleading headers or raw fallback.
    raw_stream = wrapped(raw, 6, zlib.Z_DEFAULT_STRATEGY, -15)
    add("raw-deflate-is-not-zlib", "zlib", raw_stream, None)
    dictionary = b"a shared dictionary"
    compressor = zlib.compressobj(zdict=dictionary)
    encoded = compressor.compress(dictionary * 8) + compressor.flush()
    decoder = zlib.decompressobj(zdict=dictionary)
    assert decoder.decompress(encoded) == dictionary * 8
    add("unsupported-dictionary", "zlib", encoded, None, error="dictionar")
    for offset, value in [(0, 0), (1, 0), (2, 7), (3, 0x20), (3, 0x40), (3, 0x80)]:
        encoded = bytearray(gzip.compress(raw, mtime=0))
        encoded[offset] = value
        add(f"invalid-gzip-header/{offset}/{value}", "gzip", encoded, None)

    # A second member must not borrow the first member's output as its history.
    previous = b"a" * 4096
    compressor = zlib.compressobj(wbits=-15, zdict=previous)
    payload = b"a" * 256
    deflate = compressor.compress(payload) + compressor.flush()
    bad_member = b"\x1f\x8b\x08\x00\x00\x00\x00\x00\x00\xff" + deflate
    bad_member += struct.pack("<II", zlib.crc32(payload), len(payload))
    encoded = gzip.compress(previous, mtime=0) + bad_member
    try:
        gzip.decompress(encoded)
    except zlib.error:
        pass
    else:
        raise AssertionError("Cross-member fixture unexpectedly needs no previous history")
    add("member-history-reset", "gzip", encoded, None, error="history")

    empty = gzip.compress(b"", mtime=0)
    header = bytearray(empty[:10])
    header[3] = 8
    exact = bytes(header) + b"a" * (65536 - 11) + b"\0" + empty[10:]
    assert gzip.decompress(exact) == b""
    add("header-limit/exact", "gzip", exact, b"")
    add("header-limit/excess", "gzip", exact[:10] + b"a" + exact[10:], None, error="header")
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustc", default=os.environ.get("RUSTC", "rustc"))
    parser.add_argument("--output", type=Path, help="Optional JSON report")
    args = parser.parse_args()
    started = time.monotonic()
    cases = tests()
    with tempfile.TemporaryDirectory(prefix="own-browser-compression-") as temporary:
        directory = Path(temporary)
        source = directory / "probe.rs"
        source.write_text(PROBE.replace("SOURCE", json.dumps(str(ROOT / "src/compression.rs"))), encoding="utf-8")
        binary = directory / ("probe.exe" if os.name == "nt" else "probe")
        subprocess.run([args.rustc, "--edition=2021", "-O", "-C", "overflow-checks=yes",
                        str(source), "-o", str(binary)], check=True, timeout=60)
        request = "".join(f"{kind}\t{limit}\t{stop}\t{bytes(data).hex()}\n"
                          for _, kind, data, _, limit, stop, _ in cases)
        result = subprocess.run([str(binary)], input=request, text=True, capture_output=True, timeout=60)
        assert result.returncode == 0, result.stderr
        lines = result.stdout.splitlines()
        assert len(lines) == len(cases), (len(lines), len(cases), result.stderr)
        counts = Counter()
        for case, line in zip(cases, lines):
            label, kind, _, expected, _, _, error = case
            if expected is None:
                assert line.startswith("ERR:"), (kind, label, "Unexpected success")
                if error:
                    assert error.lower() in line.lower(), (kind, label, line)
            else:
                assert line == "OK:" + expected.hex(), (kind, label, line[:200])
            counts[kind] += 1
    report = {"passed": len(cases), "cases_by_wrapper": dict(counts),
              "oracle": "Python standard-library zlib/gzip", "zlib_version": zlib.ZLIB_RUNTIME_VERSION,
              "seconds": round(time.monotonic() - started, 3)}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
