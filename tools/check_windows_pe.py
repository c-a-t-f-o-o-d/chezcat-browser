#!/usr/bin/env python3
"""Check ChezCat's Windows executable with Python's standard library.

This is static inspection: no Windows code or external program is executed.
The expected version comes from Cargo.toml and icons from assets/chezcat.ico.
An optional previous executable adds exact icon and import-set comparisons.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import struct
import sys


PROJECT_ROOT = Path(__file__).resolve().parents[1]
# The 13 system DLLs recorded by the original 0.40 release inspector and
# preserved in 0.41. A build may omit an unused DLL; it may not add another
# dependency silently. --baseline-exe additionally requires the same import set.
SYSTEM_DLLS = {
    "api-ms-win-core-synch-l1-2-0.dll", "bcryptprimitives.dll", "bcrypt.dll",
    "comdlg32.dll", "dwmapi.dll", "gdi32.dll", "kernel32.dll", "msvcrt.dll",
    "ntdll.dll", "ole32.dll", "shell32.dll", "user32.dll", "ws2_32.dll",
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


class PE:
    def __init__(self, path):
        require(path.stat().st_size <= 64 * 1024 * 1024, "PE exceeds 64 MiB inspection limit")
        self.data = path.read_bytes()
        require(self.data[:2] == b"MZ", "Missing DOS header")
        pe = self.unpack("<I", 0x3c)[0]
        require(self.data[pe:pe + 4] == b"PE\0\0", "Missing PE signature")
        self.machine, count, _, _, _, size, _ = self.unpack("<HHIIIHH", pe + 4)
        optional = pe + 24
        require(size >= 112, "Truncated PE32+ optional header")
        self.magic = self.unpack("<H", optional)[0]
        require(self.magic == 0x20b, "Expected PE32+")
        self.subsystem = self.unpack("<H", optional + 68)[0]
        self.headers = self.unpack("<I", optional + 60)[0]
        self.image_base = self.unpack("<Q", optional + 24)[0]
        directories = self.unpack("<I", optional + 108)[0]
        require(directories >= 14 and 112 + directories * 8 <= size,
                "Invalid data directory size")
        self.directories = [self.unpack("<II", optional + 112 + i * 8)
                            for i in range(directories)]
        self.sections = []
        for i in range(count):
            at = optional + size + i * 40
            name = self.data[at:at + 8].split(b"\0", 1)[0].decode("ascii")
            virtual_size, address, raw_size, offset = self.unpack("<IIII", at + 8)
            self.sections.append((name, address, virtual_size, offset, raw_size))
        self.resources = self.read_resources()

    def unpack(self, fmt, offset):
        require(0 <= offset <= len(self.data) - struct.calcsize(fmt), "PE field outside file")
        return struct.unpack_from(fmt, self.data, offset)

    def at(self, address, size=1):
        require(size >= 0, "Negative RVA size")
        if address < self.headers:
            require(address + size <= self.headers, "RVA crosses header boundary")
            offset = address
        else:
            matches = [section for section in self.sections
                       if section[1] <= address < section[1] + max(section[2], section[4])]
            require(len(matches) == 1, "RVA outside a unique section")
            _, base, _, raw, length = matches[0]
            require(address - base + size <= length,
                    "RVA points outside file-backed section data")
            offset = raw + address - base
        require(0 <= offset <= len(self.data) - size, "RVA data outside file")
        return offset

    def cstring(self, address):
        start = self.at(address)
        end = self.data.find(b"\0", start, min(start + 1024, len(self.data)))
        require(end >= start, "Unterminated DLL name")
        self.at(address, end - start + 1)
        return self.data[start:end].decode("ascii")

    def imports(self):
        address, size = self.directories[1]
        if not address:
            require(size == 0, "Unexpected empty import directory size")
            return []
        offset = self.at(address, size)
        result = []
        for delta in range(0, size - 19, 20):
            descriptor = self.unpack("<IIIII", offset + delta)
            if not any(descriptor):
                return result
            result.append(self.cstring(descriptor[3]))
        raise ValueError("Import directory has no terminating descriptor")

    def delay_imports(self):
        address, size = self.directories[13]
        if not address:
            require(size == 0, "Unexpected delay import directory size")
            return []
        offset = self.at(address, size)
        result = []
        for delta in range(0, size - 31, 32):
            descriptor = self.unpack("<IIIIIIII", offset + delta)
            if not any(descriptor):
                return result
            rva = descriptor[1] if descriptor[0] & 1 else descriptor[1] - self.image_base
            result.append(self.cstring(rva))
        raise ValueError("Delay import directory has no terminating descriptor")

    def read_resources(self):
        address, size = self.directories[2]
        require(address and size, "Missing resources")
        base = self.at(address, size)
        visited = set()
        result = {}

        def relative(offset, needed):
            require(0 <= offset <= size - needed, "Resource offset outside directory")
            return base + offset

        def walk(offset, path):
            require(len(path) < 4 and offset not in visited, "Invalid/cyclic resource tree")
            visited.add(offset)
            current = relative(offset, 16)
            named, numbered = self.unpack("<HH", current + 12)
            require(named + numbered < 1024, "Excessive resource entries")
            relative(offset + 16, 8 * (named + numbered))
            for i in range(named + numbered):
                name, target = self.unpack("<II", current + 16 + 8 * i)
                if name & 0x80000000:
                    text_offset = name & 0x7fffffff
                    length = self.unpack("<H", relative(text_offset, 2))[0]
                    start = relative(text_offset + 2, length * 2)
                    key = self.data[start:start + length * 2].decode("utf-16le")
                else:
                    key = name
                entry_path = path + (key,)
                if target & 0x80000000:
                    walk(target & 0x7fffffff, entry_path)
                else:
                    require(len(entry_path) == 3, "Expected type/name/language resource path")
                    rva, length, _, _ = self.unpack("<IIII", relative(target, 16))
                    require(entry_path not in result, "Duplicate resource path")
                    require(len(result) < 4096, "Excessive resource payloads")
                    start = self.at(rva, length)
                    result[entry_path] = self.data[start:start + length]

        walk(0, ())
        return result


def version_info(data):
    def node(offset, bound, depth=0):
        require(depth < 8, "Excessive version resource nesting")
        require(offset + 6 <= bound, "Truncated version header")
        length, value_length, kind = struct.unpack_from("<HHH", data, offset)
        require(length >= 6 and offset + length <= bound, "Version block outside parent")
        end = offset + length
        current = offset + 6
        while current + 2 <= end and data[current:current + 2] != b"\0\0":
            current += 2
        require(current + 2 <= end, "Unterminated version key")
        key = data[offset + 6:current].decode("utf-16le")
        value_start = (current + 5) & ~3
        value_end = value_start + value_length * (2 if kind == 1 else 1)
        require(value_end <= end, "Version value outside block")
        value = data[value_start:value_end]
        children = []
        child = (value_end + 3) & ~3
        while child + 6 <= end:
            if struct.unpack_from("<H", data, child)[0] == 0:
                break
            item, child_end = node(child, end, depth + 1)
            children.append(item)
            child = (child_end + 3) & ~3
        return {"key": key, "kind": kind, "value": value, "children": children}, end

    root, _ = node(0, len(data))
    require(root["key"] == "VS_VERSION_INFO", "Unexpected version root")
    require(len(root["value"]) == 52, "Missing fixed version data")
    fixed = struct.unpack("<13I", root["value"])
    require(fixed[0] == 0xfeef04bd, "Wrong version signature")

    def split(high, low):
        return high >> 16, high & 65535, low >> 16, low & 65535

    strings = {}
    stack = [root]
    while stack:
        item = stack.pop()
        stack.extend(item["children"])
        if item["kind"] == 1 and item["value"]:
            strings[item["key"]] = item["value"].decode("utf-16le").rstrip("\0")
    return {"file_version": split(fixed[2], fixed[3]),
            "product_version": split(fixed[4], fixed[5]), "strings": strings}


def package_version():
    text = (PROJECT_ROOT / "Cargo.toml").read_text(encoding="utf-8")
    package = re.search(r"(?ms)^\[package\][ \t]*\n(.*?)(?=^\[|\Z)", text)
    require(package is not None, "Cargo.toml has no [package] table")
    version = re.search(r'^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"\s*(?:#.*)?$',
                        package.group(1), re.MULTILINE)
    require(version is not None, "Expected a numeric major.minor.patch package version")
    text = version.group(1)
    parts = tuple(int(part) for part in text.split(".")) + (0,)
    require(all(part <= 65535 for part in parts), "Version components exceed Windows limits")
    # Cargo requires three components; ChezCat releases are presented as 0.42,
    # 0.43, etc. Preserve older nonzero patch versions when inspecting history.
    display = ".".join(str(part) for part in parts[:2]) if parts[2] == 0 else text
    return display, parts


def check(exe, baseline_exe, report):
    string_version, expected = package_version()
    report["expected_package_version"] = string_version
    report["expected_windows_version"] = expected
    current = PE(exe)
    report.update({"sha256": digest(current.data), "bytes": len(current.data)})
    require(current.machine == 0x8664, "PE machine is not x64")
    require(current.subsystem == 2, "PE subsystem is not Windows GUI")
    report.update({"machine": "x86_64", "format": "PE32+", "subsystem": "Windows GUI"})
    versions = []
    for path, data in current.resources.items():
        if path[0] == 16:
            info = version_info(data)
            require(info["file_version"] == info["product_version"] == expected,
                    "Fixed file/product version does not match Cargo.toml")
            require(info["strings"].get("FileVersion") == string_version,
                    "String FileVersion does not match Cargo.toml")
            require(info["strings"].get("ProductVersion") == string_version,
                    "String ProductVersion does not match Cargo.toml")
            versions.append({"resource": path, **info})
    require(len(versions) == 1, "Expected exactly one version resource")
    report["versions"] = versions

    ico = (PROJECT_ROOT / "assets/chezcat.ico").read_bytes()
    require(len(ico) >= 6, "Truncated ICO header")
    reserved, kind, count = struct.unpack_from("<HHH", ico, 0)
    require((reserved, kind) == (0, 1) and 1 <= count <= 256, "Invalid ICO header")
    require(6 + 16 * count <= len(ico), "Truncated ICO directory")
    icon_entries = []
    for i in range(count):
        at = 6 + 16 * i
        size, start = struct.unpack_from("<II", ico, at + 8)
        require(size and 6 + 16 * count <= start <= len(ico) - size,
                "ICO payload outside file")
        icon_entries.append((ico[at:at + 12], ico[start:start + size]))
    icons = {path: data for path, data in current.resources.items() if path[0] == 3}
    require(len(icons) == count, "Embedded icon count differs from assets/chezcat.ico")
    groups = []
    for path, data in current.resources.items():
        if path[0] != 14:
            continue
        require(len(data) >= 6, "Truncated icon group")
        require(struct.unpack_from("<HHH", data, 0) == (0, 1, count),
                "Icon group header differs from ICO")
        require(len(data) == 6 + 14 * count, "Unexpected group icon payload length")
        ids = []
        for i, (entry, payload) in enumerate(icon_entries):
            at = 6 + 14 * i
            require(data[at:at + 12] == entry, "Group metadata differs from ICO entry")
            icon_id = struct.unpack_from("<H", data, at + 12)[0]
            require(icons.get((3, icon_id, path[2])) == payload,
                    "PE icon differs from assets/chezcat.ico payload")
            ids.append(icon_id)
        require(len(set(ids)) == count, "Duplicate icon ID in group")
        groups.append({"resource": path, "icon_ids": ids, "directory_matches_ico": True})
    require(len(groups) == 1, "Expected one icon group")
    report.update({
        "icon_sha256": digest(ico), "icon_count": count,
        "icons": [{"resource": path, "id": path[1], "bytes": len(data),
                   "sha256": digest(data)} for path, data in icons.items()],
        "icon_groups": groups, "icon_payloads_match_original_ico": True,
    })

    imports = current.imports()
    imported = {name.lower() for name in imports}
    report["imports"] = imports
    require(imports and len(imported) == len(imports), "Empty or duplicate DLL imports")
    require(imported <= SYSTEM_DLLS,
            "Unapproved DLL imports: " + ", ".join(sorted(imported - SYSTEM_DLLS)))
    # An allocated-but-empty delay directory is also unexpected for this project.
    require(current.directories[13] == (0, 0) and not current.delay_imports(),
            "Unexpected delay import directory")
    report.update({"imports_system_only": True, "delay_imports": []})

    if baseline_exe is not None:
        baseline = PE(baseline_exe)
        require(baseline.machine == 0x8664 and baseline.subsystem == 2,
                "Baseline must be an x64 Windows GUI executable")
        report["baseline_sha256"] = digest(baseline.data)
        baseline_icons = {path: data for path, data in baseline.resources.items()
                          if path[0] in (3, 14)}
        current_icons = {path: data for path, data in current.resources.items()
                         if path[0] in (3, 14)}
        require(current_icons == baseline_icons, "Icon payloads/groups differ from baseline")
        require(imported == {name.lower() for name in baseline.imports()},
                "Import set differs from baseline")
        require(baseline.directories[13] == (0, 0), "Baseline has a delay import directory")
        report.update({"icon_payloads_match_baseline": True, "imports_match_baseline": True})
    report["passed"] = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", type=Path, default=PROJECT_ROOT / "ChezCat.exe",
                        help="executable to inspect (default: project ChezCat.exe)")
    parser.add_argument("--output", type=Path,
                        default=PROJECT_ROOT / "results/verification/windows-pe.json",
                        help="JSON report path (written on success or failure)")
    parser.add_argument("--baseline-exe", type=Path,
                        help="optional earlier executable for icon/import preservation checks")
    args = parser.parse_args()
    exe = args.exe.resolve()
    baseline = args.baseline_exe.resolve() if args.baseline_exe is not None else None
    output = args.output.resolve()
    report = {
        "passed": False, "checked_utc": datetime.now(timezone.utc).isoformat(),
        "checker_sha256": digest(Path(__file__).read_bytes()),
        "inputs": {"exe": str(exe), "baseline_exe": str(baseline) if baseline else None,
                   "cargo": str(PROJECT_ROOT / "Cargo.toml"),
                   "icon": str(PROJECT_ROOT / "assets/chezcat.ico")},
        "allowlisted_system_dlls": sorted(SYSTEM_DLLS),
        "import_allowlist_basis": "Original 13 Windows system DLL imports verified in 0.40 and retained in 0.41.",
        "baseline_comparison_requested": baseline is not None,
        "runtime_executed_on_windows": False,
        "scope": "Static PE headers, imports, VERSIONINFO and all icon/group resource bytes; executable not run.",
    }
    try:
        check(exe, baseline, report)
    except (OSError, ValueError, struct.error) as error:
        report["error"] = str(error)
    try:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    except OSError as error:
        print("Could not write PE report: " + str(error), file=sys.stderr)
        return 2
    if report["passed"]:
        print("PASS: x64 Windows GUI, version " + report["expected_package_version"]
              + ", " + str(report["icon_count"]) + " matching icons, "
              + str(len(report["imports"])) + " allowed system DLLs; " + str(output))
        return 0
    print("FAIL: " + report["error"] + "; " + str(output), file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
