#!/usr/bin/env python3
"""Generate the original single-byte decoder's data from pinned WHATWG inputs.

This development tool is offline. It reads only the checked-in, hash-verified
input files beside the manifest; no engine implementation is an input.
"""

import argparse
import hashlib
import json
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "tools/reference/text-encodings"
OUTPUT = ROOT / "src/text_encoding_single_byte.rs"
REVISION = "7d2e3e08e11f57ba1b080390b490d53648c6f69c"
REPOSITORY = "https://github.com/whatwg/encoding"
UNDEFINED = 0xFFFF


def read_inputs(folder=DATA):
    manifest = json.loads((folder / "manifest.json").read_bytes())
    if (manifest.get("schema_version") != 1
            or manifest.get("repository") != REPOSITORY
            or manifest.get("revision") != REVISION):
        raise ValueError("Unexpected encoding input manifest or revision")
    inputs = {}
    for entry in manifest["files"]:
        name = entry["path"]
        if name not in {"encodings.json", "LICENSE"} and not re.fullmatch(
                r"index-[a-z0-9-]+\.txt", name):
            raise ValueError("Unexpected encoding input path")
        if name in inputs:
            raise ValueError("Duplicate encoding input path")
        path = folder / name
        if path.is_symlink() or not path.is_file():
            raise ValueError("Missing or linked encoding input: " + name)
        data = path.read_bytes()
        blob = hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()
        if (len(data) != entry["bytes"] or blob != entry["git_blob"]
                or hashlib.sha256(data).hexdigest() != entry["sha256"]):
            raise ValueError("Pinned encoding input mismatch: " + name)
        inputs[name] = data
    return inputs


def parse_index(data):
    values = [UNDEFINED] * 128
    seen = set()
    # Index descriptions contain literal C1 controls, including U+0085. Only
    # actual LF bytes delimit rows; str.splitlines() would split inside a row.
    for line in data.decode("utf-8").split("\n"):
        if not line.strip() or line.startswith("#"):
            continue
        fields = line.split("\t", 2)
        if len(fields) != 3 or not re.fullmatch(r"0x[0-9A-Fa-f]+", fields[1]):
            raise ValueError("Malformed single-byte index row")
        pointer, scalar = int(fields[0]), int(fields[1], 16)
        if pointer not in range(128) or pointer in seen:
            raise ValueError("Invalid or duplicate single-byte index pointer")
        if not 0 <= scalar < UNDEFINED or 0xD800 <= scalar <= 0xDFFF:
            raise ValueError("Single-byte index contains an unsupported scalar")
        seen.add(pointer)
        values[pointer] = scalar
    if not seen:
        raise ValueError("Empty single-byte index")
    return values


def model(inputs):
    groups = json.loads(inputs["encodings.json"])
    selected = [group for group in groups if group["heading"] == "Legacy single-byte encodings"]
    if len(selected) != 1:
        raise ValueError("Single-byte encoding group missing or duplicated")
    encodings = selected[0]["encodings"]
    if len(encodings) != 28:
        raise ValueError("Pinned single-byte encoding inventory changed")
    labels = set()
    names = set()
    tables = {}
    entries = []
    for encoding in encodings:
        name = encoding["name"].lower()
        if name in names or not re.fullmatch(r"[a-z0-9-]+", name):
            raise ValueError("Invalid or duplicate encoding name")
        names.add(name)
        variant = "".join(part.capitalize() for part in name.split("-"))
        index_name = "iso-8859-8" if name == "iso-8859-8-i" else name
        path = "index-" + index_name + ".txt"
        table = "INDEX_" + index_name.upper().replace("-", "_")
        if table not in tables:
            tables[table] = parse_index(inputs[path])
        aliases = encoding["labels"]
        if not aliases or any(not re.fullmatch(r"[a-z0-9_.:-]+", alias) for alias in aliases):
            raise ValueError("Invalid encoding labels")
        if len(set(aliases)) != len(aliases) or labels.intersection(aliases):
            raise ValueError("Duplicate encoding label")
        labels.update(aliases)
        entries.append((variant, name, aliases, table))
    if len(tables) != 27 or len(labels) != 168:
        raise ValueError("Pinned single-byte table/label inventory changed")
    expected = {"encodings.json", "LICENSE"} | {
        "index-" + ("iso-8859-8" if name == "iso-8859-8-i" else name) + ".txt"
        for name in names
    }
    if set(inputs) != expected:
        raise ValueError("Unselected or missing single-byte input")
    return entries, tables


def render(inputs):
    entries, tables = model(inputs)
    lines = [
        "//! Generated WHATWG single-byte index and label data.",
        "//! Regenerate offline: python tools/generate_single_byte_encodings.py",
        "//! Source: https://github.com/whatwg/encoding/tree/" + REVISION,
        "//! Copyright (c) WHATWG (Apple, Google, Mozilla, Microsoft).",
        "//! The incorporated standard data is under BSD-3-Clause; retain",
        "//! notices/WHATWG-Encoding-LICENSE.txt and tools/reference/text-encodings/.",
        "//! Lookup code is original ChezCat code; no engine implementation was imported.",
        "", "#[derive(Clone, Copy, Debug, Eq, PartialEq)]", "pub(super) enum SingleByte {",
    ]
    lines.extend("    " + variant + "," for variant, _, _, _ in entries)
    lines.extend(["}", "", "impl SingleByte {", "    pub(super) fn name(self) -> &'static str {", "        match self {"])
    lines.extend('            Self::' + variant + ' => "' + name + '",' for variant, name, _, _ in entries)
    lines.extend(["        }", "    }", "", "    // The caller applies Encoding's ASCII whitespace/case normalization.",
                  "    pub(super) fn from_label(label: &str) -> Option<Self> {", "        match label {"])
    for variant, _, aliases, _ in entries:
        row = "            "
        for index, alias in enumerate(aliases):
            item = (' | ' if index else '') + '"' + alias + '"'
            if len(row) + len(item) > 95:
                lines.append(row)
                row = "            " + item.lstrip()
            else:
                row += item
        row += " => Some(Self::" + variant + "),"
        lines.append(row)
    lines.extend(["            _ => None,", "        }", "    }", "",
                  "    pub(super) fn decode(self, byte: u8) -> Option<char> {",
                  "        if byte < 0x80 {", "            return Some(char::from(byte));", "        }",
                  "        let index = match self {"])
    for table in tables:
        variants = ["Self::" + variant for variant, _, _, item in entries if item == table]
        lines.append("            " + " | ".join(variants) + " => &" + table + ",")
    lines.extend(["        };", "        let scalar = index[usize::from(byte - 0x80)];",
                  "        if scalar == 0xffff {", "            None", "        } else {",
                  "            char::from_u32(u32::from(scalar))", "        }", "    }", "}", "",
                  "// 0xffff is an undefined index pointer, not a decoded code point."])
    for table, values in tables.items():
        lines.append("const " + table + ": [u16; 128] = [")
        for start in range(0, 128, 8):
            lines.append("    " + ", ".join(f"0x{value:04x}" for value in values[start:start + 8]) + ",")
        lines.extend(["];", ""])
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Verify exact generated bytes without writing")
    args = parser.parse_args()
    try:
        result = render(read_inputs()).encode("utf-8")
        if args.check:
            if not OUTPUT.is_file() or OUTPUT.read_bytes() != result:
                raise ValueError("Generated single-byte encoding module is stale")
        else:
            OUTPUT.write_bytes(result)
    except (OSError, ValueError, KeyError, TypeError) as error:
        print("Single-byte data check failed: " + str(error), file=sys.stderr)
        return 1
    print("Pinned single-byte data verified: 28 names, 168 labels, 27 tables.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
