#!/usr/bin/env python3
"""Generate HTML named-reference data from exact pinned WHATWG inputs, offline.

Only normative data is imported. The generator and runtime matching algorithms
are original ChezCat code; no browser engine implementation is an input.
"""

import argparse
import hashlib
import json
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "tools/reference/html-entities"
OUTPUT = ROOT / "src/html_named_references_data.rs"
NOTICE = ROOT / "notices/WHATWG-HTML-Entities-LICENSE.txt"
REPOSITORY = "https://github.com/whatwg/html-build"
REVISION = "283a3531a61106d07d9a7d9fb3e6f3b9bfd33d70"
HTML_REVISION = "0e292f402d0c3dd17fe00c1bb2ef3a3c94cd8bce"
FOUNDATION = {
    "coverage_pin": "html_build_entities",
    "input_manifest": "tools/reference/html-foundation/inputs.json",
    "html_revision": HTML_REVISION,
}
INVENTORY = {
    "name_count": 2231,
    "multi_codepoint_count": 93,
    "semicolonless_legacy_count": 106,
    "max_name_bytes": 32,
}
FILES = {
    "entities.json": {
        "upstream_path": "entities/out/entities.json",
        "bytes": 145897,
        "git_blob": "557170b41f47a13a46ec695561eb5fe76da73bdb",
        "sha256": "d741d877ac77c4194c4ad526b5b4a19aef8dfe411ab840a466891cdbb9f362e6",
    },
    "LICENSE": {
        "upstream_path": "LICENSE",
        "bytes": 16315,
        "git_blob": "f2dcda46deccefd245749202a88a7837e35c6daa",
        "sha256": "85dc6f5ccb57a6fe8c33d158f9fc8fc7ee5655a5d3db2cdd131c6a3d0f48a864",
    },
}


def unique_object(pairs):
    result = {}
    for name, value in pairs:
        if name in result:
            raise ValueError("Duplicate JSON member: " + name)
        result[name] = value
    return result


def invalid_constant(value):
    raise ValueError("Invalid JSON constant: " + value)


def parse_json(data):
    return json.loads(data.decode("utf-8"), object_pairs_hook=unique_object,
                      parse_constant=invalid_constant)


def read_regular(path):
    if path.is_symlink() or not path.is_file():
        raise ValueError("Missing or linked regular file: " + str(path))
    return path.read_bytes()


def read_inputs(folder=DATA):
    if folder.is_symlink() or not folder.is_dir():
        raise ValueError("Missing or linked entity input directory")
    manifest = parse_json(read_regular(folder / "manifest.json"))
    fields = {"schema_version", "repository", "revision", "foundation", "inventory", "files"}
    if (not isinstance(manifest, dict) or set(manifest) != fields
            or type(manifest["schema_version"]) is not int or manifest["schema_version"] != 1
            or manifest["repository"] != REPOSITORY or manifest["revision"] != REVISION
            or manifest["foundation"] != FOUNDATION or manifest["inventory"] != INVENTORY
            or any(type(value) is not int for value in manifest["inventory"].values())
            or not isinstance(manifest["files"], list)):
        raise ValueError("Unexpected entity input manifest or revision")
    inputs = {}
    for entry in manifest["files"]:
        if not isinstance(entry, dict) or set(entry) != {"path", "upstream_path", "bytes", "git_blob", "sha256"}:
            raise ValueError("Invalid entity input file schema")
        name = entry["path"]
        if not isinstance(name, str) or name not in FILES:
            raise ValueError("Unexpected entity input path")
        if name in inputs:
            raise ValueError("Duplicate entity input path")
        expected = FILES[name]
        if type(entry["bytes"]) is not int or entry != {"path": name, **expected}:
            raise ValueError("Pinned entity input identity mismatch: " + name)
        data = read_regular(folder / name)
        blob = hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()
        if (len(data) != expected["bytes"] or blob != expected["git_blob"]
                or hashlib.sha256(data).hexdigest() != expected["sha256"]):
            raise ValueError("Pinned entity input mismatch: " + name)
        inputs[name] = data
    if set(inputs) != set(FILES):
        raise ValueError("Unselected or missing entity inputs")
    return inputs


def parse_entities(data):
    source = parse_json(data)
    if not isinstance(source, dict) or not source:
        raise ValueError("Entity data must be a nonempty object")
    entries = []
    for name, value in source.items():
        if not re.fullmatch(r"&[A-Za-z][A-Za-z0-9]*;?", name):
            raise ValueError("Invalid ASCII entity name: " + repr(name))
        if not isinstance(value, dict) or set(value) != {"codepoints", "characters"}:
            raise ValueError("Invalid entity mapping schema: " + name)
        scalars = value["codepoints"]
        if (not isinstance(scalars, list) or len(scalars) not in (1, 2)
                or any(type(value) is not int or not 0 <= value <= 0x10ffff
                       or 0xd800 <= value <= 0xdfff for value in scalars)):
            raise ValueError("Invalid entity scalar sequence: " + name)
        characters = value["characters"]
        if not isinstance(characters, str) or [ord(value) for value in characters] != scalars:
            raise ValueError("Entity characters/codepoints mismatch: " + name)
        entries.append((name[1:], characters))
    return sorted(entries)


def model(inputs):
    if set(inputs) != set(FILES):
        raise ValueError("Unselected or missing entity inputs")
    entries = parse_entities(inputs["entities.json"])
    inventory = {
        "name_count": len(entries),
        "multi_codepoint_count": sum(len(value) == 2 for _, value in entries),
        "semicolonless_legacy_count": sum(not name.endswith(";") for name, _ in entries),
        "max_name_bytes": max(len(name.encode("ascii")) for name, _ in entries),
    }
    if inventory != INVENTORY:
        raise ValueError("Pinned entity inventory changed")
    names = dict(entries)
    if any(names.get(name + ";") != value for name, value in entries if not name.endswith(";")):
        raise ValueError("Legacy entity differs from its semicolon spelling")
    return entries, inventory["max_name_bytes"]


def render(inputs):
    entries, maximum = model(inputs)
    lines = [
        "//! Generated WHATWG HTML named-character-reference data.",
        "//! Regenerate offline: python tools/generate_html_entities.py",
        "//! Source: " + REPOSITORY + "/blob/" + REVISION + "/entities/out/entities.json",
        "//! Source Git blob: " + FILES["entities.json"]["git_blob"] + ".",
        "//! Copyright (c) WHATWG (Apple, Google, Mozilla, Microsoft).",
        "//! Incorporated standard data is under BSD-3-Clause; retain",
        "//! notices/WHATWG-HTML-Entities-LICENSE.txt and tools/reference/html-entities/.",
        "//! Generator and matching logic are original; no engine implementation was imported.",
        "", "// Names omit the initial ampersand and retain any normative semicolon.",
        "pub(super) const MAX_NAME_BYTES: usize = " + str(maximum) + ";",
        "", "pub(super) static ENTITIES: &[(&str, &str)] = &[",
    ]
    for name, value in entries:
        escaped = "".join("\\u{" + format(ord(character), "x") + "}" for character in value)
        lines.append('    ("' + name + '", "' + escaped + '"),')
    lines.extend(["];", ""])
    return "\n".join(lines)


def verify_notice(inputs, path=NOTICE):
    if read_regular(path) != inputs["LICENSE"]:
        raise ValueError("Distribution notice differs from the pinned upstream license")


def verify_generated(expected, path=OUTPUT):
    if read_regular(path) != expected:
        raise ValueError("Generated HTML entity module is stale")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Verify exact generated bytes without writing")
    args = parser.parse_args()
    try:
        inputs = read_inputs()
        verify_notice(inputs)
        result = render(inputs).encode("ascii")
        if args.check:
            verify_generated(result)
        else:
            if OUTPUT.is_symlink() or (OUTPUT.exists() and not OUTPUT.is_file()):
                raise ValueError("Generated output must be a regular file")
            OUTPUT.write_bytes(result)
    except (OSError, ValueError, KeyError, TypeError) as error:
        print("HTML entity data check failed: " + str(error), file=sys.stderr)
        return 1
    print("Pinned HTML entity data verified: 2231 names, 93 two-scalar mappings, 106 legacy names.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
