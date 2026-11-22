#!/usr/bin/env python3
"""Generate pinned GBK/GB18030/EUC-KR mapping data for original decoders.

This offline development tool reads only the retained normative label and index
data. No browser engine implementation, platform codec or runtime dependency is
an input. The decoder algorithms are separate original Rust source.
"""

import argparse
import hashlib
import json
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "tools/reference/multibyte-encodings"
OUTPUT = ROOT / "src/text_encoding_multibyte_data.rs"
REVISION = "7d2e3e08e11f57ba1b080390b490d53648c6f69c"
REPOSITORY = "https://github.com/whatwg/encoding"
FILES = {"encodings.json", "LICENSE", "index-gb18030.txt",
         "index-gb18030-ranges.txt", "index-euc-kr.txt"}
NAMES = ("gbk", "gb18030", "euc-kr")
INDEX_SIZE = 126 * 190
UNDEFINED = 0xFFFF


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("Duplicate JSON field")
        result[key] = value
    return result


def read_inputs(folder=DATA):
    manifest = json.loads((folder / "manifest.json").read_bytes(),
                          object_pairs_hook=unique_object)
    if (type(manifest.get("schema_version")) is not int
            or manifest["schema_version"] != 1
            or manifest.get("repository") != REPOSITORY
            or manifest.get("revision") != REVISION):
        raise ValueError("Unexpected multibyte input manifest or revision")
    inputs = {}
    for entry in manifest["files"]:
        name = entry["path"]
        if name not in FILES:
            raise ValueError("Unexpected multibyte input path")
        if name in inputs:
            raise ValueError("Duplicate multibyte input path")
        path = folder / name
        if path.is_symlink() or not path.is_file():
            raise ValueError("Missing or linked multibyte input: " + name)
        data = path.read_bytes()
        blob = hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()
        if (type(entry["bytes"]) is not int or len(data) != entry["bytes"]
                or blob != entry["git_blob"]
                or hashlib.sha256(data).hexdigest() != entry["sha256"]):
            raise ValueError("Pinned multibyte input mismatch: " + name)
        inputs[name] = data
    if set(inputs) != FILES:
        raise ValueError("Incomplete multibyte input selection")
    return inputs


def rows(data, columns):
    # Split only at LF, because an index description may contain a literal
    # control character which Python's broader splitlines() treats as a boundary.
    for line in data.decode("utf-8").split("\n"):
        if not line.strip() or line.startswith("#"):
            continue
        fields = line.split("\t", columns - 1)
        if (len(fields) != columns or not re.fullmatch(r" *[0-9]+", fields[0])
                or not re.fullmatch(r"0x[0-9A-Fa-f]+", fields[1])):
            raise ValueError("Malformed normative index row")
        yield int(fields[0]), int(fields[1], 16)


def parse_index(data):
    values = [UNDEFINED] * INDEX_SIZE
    seen = set()
    for pointer, scalar in rows(data, 3):
        if pointer not in range(INDEX_SIZE) or pointer in seen:
            raise ValueError("Invalid or duplicate two-byte index pointer")
        if not 0 <= scalar < UNDEFINED or 0xD800 <= scalar <= 0xDFFF:
            raise ValueError("Two-byte index contains an unsupported scalar")
        seen.add(pointer)
        values[pointer] = scalar
    if not seen:
        raise ValueError("Empty two-byte index")
    return values


def parse_ranges(data):
    result = []
    for pointer, scalar in rows(data, 2):
        if (not 0 <= pointer <= 1237575 or not 0 <= scalar <= 0x10FFFF
                or 0xD800 <= scalar <= 0xDFFF):
            raise ValueError("Invalid GB18030 range value")
        if result and (pointer <= result[-1][0] or scalar <= result[-1][1]):
            raise ValueError("GB18030 ranges must be strictly ordered")
        result.append((pointer, scalar))
    if not result:
        raise ValueError("Empty GB18030 ranges")
    return result


def model(inputs):
    if set(inputs) != FILES:
        raise ValueError("Unexpected multibyte input selection")
    groups = json.loads(inputs["encodings.json"], object_pairs_hook=unique_object)
    selected = [encoding for group in groups for encoding in group["encodings"]
                if encoding["name"].lower() in NAMES]
    if tuple(encoding["name"].lower() for encoding in selected) != NAMES:
        raise ValueError("Pinned multibyte encoding inventory changed")
    labels = set()
    entries = []
    for encoding in selected:
        aliases = encoding["labels"]
        if not aliases or any(not re.fullmatch(r"[a-z0-9_.:-]+", alias) for alias in aliases):
            raise ValueError("Invalid multibyte encoding label")
        if len(set(aliases)) != len(aliases) or labels.intersection(aliases):
            raise ValueError("Duplicate multibyte encoding label")
        labels.update(aliases)
        entries.append((encoding["name"].lower(), aliases))
    gb = parse_index(inputs["index-gb18030.txt"])
    kr = parse_index(inputs["index-euc-kr.txt"])
    ranges = parse_ranges(inputs["index-gb18030-ranges.txt"])
    if (len(labels) != 20 or sum(value != UNDEFINED for value in gb) != 23940
            or sum(value != UNDEFINED for value in kr) != 17048
            or len(ranges) != 207 or ranges[0] != (0, 0x80)
            or ranges[-1] != (189000, 0x10000)):
        raise ValueError("Pinned multibyte mapping inventory changed")
    return entries, gb, kr, ranges


def render(inputs):
    entries, gb, kr, ranges = model(inputs)
    lines = [
        "//! Generated WHATWG GBK/GB18030/EUC-KR index and label data.",
        "//! Regenerate offline: python tools/generate_multibyte_encodings.py",
        "//! Source: https://github.com/whatwg/encoding/tree/" + REVISION,
        "//! Copyright (c) WHATWG (Apple, Google, Mozilla, Microsoft).",
        "//! Incorporated standard data is BSD-3-Clause; retain",
        "//! notices/WHATWG-Encoding-LICENSE.txt and tools/reference/multibyte-encodings/.",
        "//! Lookup/generator logic is original ChezCat code, not an imported decoder.",
        "", "// The caller normalizes ASCII case and surrounding ASCII whitespace.",
        "pub(super) fn canonical_name(label: &str) -> Option<&'static str> {",
        "    match label {",
    ]
    for name, aliases in entries:
        row = "        "
        for index, alias in enumerate(aliases):
            item = (' | ' if index else '') + '"' + alias + '"'
            if len(row) + len(item) > 92:
                lines.append(row)
                row = "        " + item.lstrip()
            else:
                row += item
        lines.append(row + ' => Some("' + name + '"),')
    lines.extend([
        "        _ => None,", "    }", "}", "",
        "pub(super) fn gb18030_index(pointer: usize) -> Option<char> {",
        "    indexed(&INDEX_GB18030, pointer)", "}", "",
        "pub(super) fn euc_kr_index(pointer: usize) -> Option<char> {",
        "    indexed(&INDEX_EUC_KR, pointer)", "}", "",
        "fn indexed(index: &[u16], pointer: usize) -> Option<char> {",
        "    let scalar = *index.get(pointer)?;",
        "    if scalar == 0xffff {", "        None", "    } else {",
        "        char::from_u32(u32::from(scalar))", "    }", "}", "",
        "// 0xffff marks an absent pointer and is never emitted as a code point.",
    ])
    for name, values in [("INDEX_GB18030", gb), ("INDEX_EUC_KR", kr)]:
        lines.append(f"const {name}: [u16; {INDEX_SIZE}] = [")
        for start in range(0, INDEX_SIZE, 8):
            lines.append("    " + ", ".join(f"0x{value:04x}" for value in values[start:start + 8]) + ",")
        lines.extend(["];", ""])
    lines.append("pub(super) const GB18030_RANGES: &[(u32, u32)] = &[")
    lines.extend(f"    ({pointer}, 0x{scalar:04x})," for pointer, scalar in ranges)
    lines.extend(["];", ""])
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Verify generated bytes without writing")
    args = parser.parse_args()
    try:
        result = render(read_inputs()).encode("utf-8")
        if args.check:
            if not OUTPUT.is_file() or OUTPUT.read_bytes() != result:
                raise ValueError("Generated multibyte encoding module is stale")
        else:
            OUTPUT.write_bytes(result)
    except (OSError, ValueError, KeyError, TypeError) as error:
        print("Multibyte data check failed: " + str(error), file=sys.stderr)
        return 1
    print("Pinned multibyte data verified: 3 names, 20 labels, 2 indexes, 207 range starts.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
