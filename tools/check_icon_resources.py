#!/usr/bin/env python3
"""Independently inspect ChezCat's generated Windows resource formats.

This development check reads numeric .res records and AMD64 COFF resource
directories without using the generator's parser. Optional resource-only PE
probes must be linked separately; they are never treated as browser builds.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import struct

import check_windows_pe
import portable_resources


ROOT = Path(__file__).resolve().parents[1]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def read_res(data):
    offset = 0
    result = {}
    while offset < len(data):
        require(offset % 4 == 0 and offset + 32 <= len(data), "Invalid .res header position")
        size, header, kind_tag, kind, name_tag, name, revision, flags, lang, version, characteristics = (
            struct.unpack_from("<IIHHHHIHHII", data, offset))
        require(header == 32 and kind_tag == name_tag == 65535, "Expected numeric .res header")
        require(revision == version == characteristics == 0, "Unexpected .res header fields")
        if offset == 0:
            require(size == kind == name == flags == lang == 0, "Missing null .res header")
        else:
            key = kind, name, lang
            require(key not in result, "Duplicate .res resource")
            require(flags == (0 if kind == 16 else 0x1010), "Unexpected .res memory flags")
            end = offset + header + size
            require(end <= len(data), ".res payload outside input")
            result[key] = data[offset + header:end]
            require(not any(data[end:(end + 3) & ~3]), "Nonzero .res alignment padding")
        offset = (offset + header + size + 3) & ~3
    require(offset == len(data), "Trailing partial .res record")
    return result


def read_coff(data):
    require(len(data) >= 60, "Truncated COFF header")
    machine, sections, timestamp, symbols, count, optional, characteristics = struct.unpack_from("<HHIIIHH", data)
    require((machine, sections, timestamp, count, optional, characteristics) == (0x8664, 1, 0, 1, 0, 4),
            "Unexpected COFF file header")
    name, virtual_size, address, size, raw, relocations, lines, reloc_count, line_count, flags = (
        struct.unpack_from("<8sIIIIIIHHI", data, 20))
    require(name == b".rsrc\0\0\0" and virtual_size == address == lines == line_count == 0,
            "Unexpected COFF resource section")
    require(raw == 60 and relocations == raw + size and symbols == relocations + reloc_count * 10,
            "Unexpected COFF layout")
    require(flags == 0x40300040 and symbols + 22 == len(data), "Invalid COFF flags/extent")
    symbol = struct.unpack_from("<8sIhHBB", data, symbols)
    require(symbol == (b".rsrc\0\0\0", 0, 1, 0, 3, 0), "Invalid resource section symbol")
    require(struct.unpack_from("<I", data, symbols + 18)[0] == 4, "Unexpected COFF strings")
    section = data[raw:raw + size]
    result = {}
    leaves = []
    visited = set()

    def field(fmt, at):
        require(0 <= at <= len(section) - struct.calcsize(fmt), "COFF resource field outside section")
        return struct.unpack_from(fmt, section, at)

    def directory(at, path):
        require(at % 4 == 0 and at not in visited and len(path) < 3, "Invalid COFF resource directory")
        visited.add(at)
        characteristics, timestamp, major, minor, named, count = field("<IIHHHH", at)
        require(characteristics == timestamp == major == minor == named == 0 and 0 < count < 1024,
                "Unsupported COFF resource directory header")
        identifiers = []
        for index in range(count):
            name, target = field("<II", at + 16 + index * 8)
            require(name < 65536, "Expected numeric resource ID")
            identifiers.append(name)
            key = path + (name,)
            if target & 0x80000000:
                directory(target & 0x7fffffff, key)
            else:
                require(len(key) == 3 and key not in result and target % 4 == 0, "Invalid COFF resource leaf")
                offset, length, codepage, reserved = field("<IIII", target)
                require(codepage == reserved == 0 and offset % 4 == 0 and length > 0
                        and offset <= len(section) - length, "Invalid COFF resource payload")
                result[key] = section[offset:offset + length]
                leaves.append(target)
        require(identifiers == sorted(set(identifiers)), "Unsorted or duplicate resource IDs")

    directory(0, ())
    actual_relocations = [struct.unpack_from("<IIH", data, relocations + index * 10)
                          for index in range(reloc_count)]
    require(actual_relocations == [(at, 0, 3) for at in leaves],
            "COFF RVA relocations do not target each resource data entry/section symbol")
    return result, actual_relocations


def inspect_resources(entries, icon):
    reserved, kind, count = struct.unpack_from("<HHH", icon)
    require((reserved, kind) == (0, 1) and 1 <= count <= 256, "Invalid source ICO")
    expected_keys = {(3, index + 1, 0x409) for index in range(count)} | {(14, 101, 0x409), (16, 1, 0x409)}
    require(set(entries) == expected_keys, "Unexpected resource IDs/languages")
    group = entries[(14, 101, 0x409)]
    require(struct.unpack_from("<HHH", group) == (0, 1, count) and len(group) == 6 + count * 14,
            "Invalid icon group")
    for index in range(count):
        source = 6 + index * 16
        target = 6 + index * 14
        size, offset = struct.unpack_from("<II", icon, source + 8)
        require(6 + count * 16 <= offset <= len(icon) - size and size > 0, "Invalid ICO payload extent")
        require(group[target:target + 12] == icon[source:source + 12], "Icon group metadata changed")
        require(struct.unpack_from("<H", group, target + 12)[0] == index + 1, "Icon group ID changed")
        require(entries[(3, index + 1, 0x409)] == icon[offset:offset + size], "Icon payload changed")
    version = check_windows_pe.version_info(entries[(16, 1, 0x409)])
    display, fixed = check_windows_pe.package_version()
    require(version["file_version"] == version["product_version"] == fixed, "Fixed resource version mismatch")
    require(version["strings"] == {
        "FileDescription": "ChezCat web browser", "FileVersion": display,
        "InternalName": "ChezCat", "OriginalFilename": "ChezCat.exe",
        "ProductName": "ChezCat", "ProductVersion": display,
    }, "Resource branding/string version mismatch")
    return count, version


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe-res", type=Path, help="Separately linked resource-only PE from .res")
    parser.add_argument("--probe-coff", type=Path, help="Separately linked resource-only PE from .o")
    parser.add_argument("--baseline-ico", type=Path, help="Optional earlier unchanged icon")
    parser.add_argument("--output", type=Path, default=ROOT / "results/verification/icon-resources.json")
    args = parser.parse_args()
    source = ROOT / "resources/chezcat.rc"
    icon = (ROOT / "assets/chezcat.ico").read_bytes()
    res = (ROOT / "resources/chezcat.res").read_bytes()
    coff = (ROOT / "resources/chezcat-x64.o").read_bytes()
    res_entries = read_res(res)
    coff_entries, relocations = read_coff(coff)
    require(res_entries == coff_entries, "Resource formats contain different payloads")
    count, version = inspect_resources(res_entries, icon)
    require(portable_resources.generate(source.read_text(encoding="utf-8"), icon) == (res, coff),
            "Portable regeneration is not byte-identical")
    if args.baseline_ico:
        require(icon == args.baseline_ico.read_bytes(), "ICO file differs from baseline")
    manifest = (ROOT / "resources/SHA256SUMS.txt").read_text(encoding="utf-8").splitlines()
    for line in manifest:
        expected, relative = line.split("  ", 1)
        require(sha256((ROOT / relative).read_bytes()) == expected, "Resource manifest mismatch: " + relative)
    probes = []
    for kind, path in (("res", args.probe_res), ("coff", args.probe_coff)):
        if path is None:
            continue
        pe = check_windows_pe.PE(path)
        require(pe.machine == 0x8664 and pe.subsystem == 2 and pe.resources == res_entries,
                "Linked resource-only PE changed resource data")
        require(not pe.imports() and not pe.delay_imports(), "Resource-only probe unexpectedly imports code")
        probes.append({"input_format": kind, "path": str(path), "bytes": len(pe.data),
                       "sha256": sha256(pe.data), "resources_match": True,
                       "scope": "Development resource-only PE; not ChezCat and never executed."})
    report = {
        "checked_utc": datetime.now(timezone.utc).isoformat(), "passed": True,
        "scope": "Static resource metadata, exact icon payloads, COFF relocation structure and deterministic regeneration only; no browser build or runtime claim.",
        "resource_count": len(res_entries), "icon_count": count, "version": version,
        "ico_sha256": sha256(icon), "ico_matches_baseline": bool(args.baseline_ico),
        "formats_match": True, "deterministic_regeneration": True,
        "coff_relocations": relocations, "linked_probes": probes,
        "inputs": {relative: sha256((ROOT / relative).read_bytes()) for relative in (
            "tools/portable_resources.py", "tools/build_icon_resources.py", "tools/check_icon_resources.py",
            "tools/check_windows_pe.py", "Cargo.toml", "resources/chezcat.rc", "assets/chezcat.ico",
            "resources/chezcat.res", "resources/chezcat-x64.o", "resources/SHA256SUMS.txt")},
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"PASS: {count} unchanged icon payloads, version {version['strings']['FileVersion']}, "
          f"matching .res/COFF resources and {len(probes)} linked resource-only probes; {args.output}")


if __name__ == "__main__":
    main()
