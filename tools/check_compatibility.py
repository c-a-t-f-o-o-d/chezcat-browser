#!/usr/bin/env python3
"""Bounded, independent ChezCat capability inventory (development only).

Uses the existing check_javascript.py Rust/Node probe helpers. Each original
program gets a fresh interpreter and Node process; no websites or captured page
scripts are used. Missing capabilities are findings, not checker failures.
The optional pinned Test262 selection is deliberately not a full suite runner.
"""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
CASE_FILE = ROOT / "tools/compatibility_cases.json"
HELPER_FILE = ROOT / "tools/check_javascript.py"
UPSTREAM = ROOT / "tools/verification-0.34/test262"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_manifest(root):
    paths = sorted(root.glob("src/**/*.rs"))
    paths += [root / name for name in ("Cargo.toml", "Cargo.lock", "build.rs")
              if (root / name).is_file()]
    files = {path.relative_to(root).as_posix(): digest(path)
             for path in sorted(paths, key=lambda item: item.relative_to(root).as_posix())}
    joined = "".join(f"{name}\0{value}\n" for name, value in files.items())
    return {"sha256": hashlib.sha256(joined.encode()).hexdigest(), "files": files}


def load_helpers():
    spec = importlib.util.spec_from_file_location("chezcat_existing_checks", HELPER_FILE)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def probe_status(case, actual, expected, matches):
    if matches:
        return "supported"
    if not expected.get("ok") or any(actual.get(k) for k in
                                      ("crash", "timeout", "protocol_error", "tool_error")):
        return "unassessed"
    if case["baseline_status"] == "intentionally_limited":
        return "intentionally_limited"
    if actual.get("ok") or case["baseline_status"] == "partial":
        return "partial"
    return "missing"


def run_originals(cases, helper, probe, node):
    results = []
    for case in cases:
        try:
            expected = helper.run_node(node, case["source"])
            actual = helper.run_own(probe, case["source"])
        except (OSError, subprocess.SubprocessError, ValueError) as error:
            expected = {"ok": False, "tool_error": str(error)}
            actual = {"ok": False, "tool_error": str(error)}
        matches = bool(expected.get("ok") and helper.finished_normally(actual)
                       and actual.get("ok") and actual["records"] == expected["records"])
        results.append({"id": case["id"], "area": case["area"], "label": case["label"],
                        "fixture_sha256": hashlib.sha256(case["source"].encode()).hexdigest(),
                        "status": probe_status(case, actual, expected, matches),
                        "matches_node": matches, "note": case["note"],
                        "actual": actual, "reference": expected})
    return results


def run_test262(helper, probe, node):
    manifest_path = UPSTREAM / "manifest.json"
    if not manifest_path.is_file():
        return {"status": "not_run", "reason": "No pinned upstream selection is available."}
    manifest = json.loads(manifest_path.read_text())
    good = {entry["path"]: entry for entry in manifest["files"] if entry["ok"]}
    for path, entry in good.items():
        if digest(UPSTREAM / path) != entry["sha256"]:
            raise ValueError(f"Pinned Test262 file hash changed: {path}")
    results = []
    for path in good:
        if not path.startswith("test/"):
            continue
        source = (UPSTREAM / path).read_text()
        includes_match = re.search(r"^includes:\s*\[([^\]]*)\]", source, re.MULTILINE)
        includes = [item.strip().strip("\"'") for item in
                    includes_match.group(1).split(",")] if includes_match else []
        flags_match = re.search(r"^flags:\s*\[([^\]]*)\]", source, re.MULTILINE)
        flags = flags_match.group(1).split(",") if flags_match else []
        flags = {flag.strip() for flag in flags}
        harness = ["harness/sta.js", "harness/assert.js"]
        harness += ["harness/" + item for item in includes if item]
        missing = [name for name in harness if name not in good]
        if missing or flags.intersection({"async", "module", "raw"}) or re.search(r"^negative:", source, re.MULTILINE):
            results.append({"path": path, "status": "not_run",
                            "reason": "Unsupported selected-test metadata or unavailable harness.",
                            "missing_harness": missing, "flags": sorted(flags)})
            continue
        modes = ["strict"] if "onlyStrict" in flags else ["default"] if "noStrict" in flags else ["default", "strict"]
        for mode in modes:
            program = ("\"use strict\";\n" if mode == "strict" else "")
            program += "\n".join((UPSTREAM / item).read_text() for item in harness)
            program += "\n" + source
            expected = helper.run_node(node, program)
            actual = helper.run_own(probe, program)
            passed = bool(expected.get("ok") and helper.finished_normally(actual) and actual.get("ok"))
            results.append({"path": path, "mode": mode, "status": "passed" if passed else "failed",
                            "file_sha256": good[path]["sha256"], "actual": actual,
                            "reference": expected})
    return {"status": "selected_tests_only", "repository": manifest["repository"],
            "revision": manifest["revision"], "manifest_sha256": digest(manifest_path),
            "counts": dict(Counter(row["status"] for row in results)), "results": results,
            "limits": "Selected unchanged upstream files and pinned harness only; no Test262 suite-wide conformance claim."}


def comparison(previous, results):
    before = {row["id"]: row for row in previous["probes"]}
    changed, regressions, incomparable = [], [], []
    for row in results:
        old = before.get(row["id"])
        if old is None or old["fixture_sha256"] != row["fixture_sha256"]:
            incomparable.append(row["id"])
        elif old["status"] != row["status"] or old["matches_node"] != row["matches_node"]:
            entry = {"id": row["id"], "before": old["status"], "after": row["status"],
                     "matched_before": old["matches_node"], "matches_now": row["matches_node"]}
            changed.append(entry)
            if old["matches_node"] and not row["matches_node"]:
                regressions.append(row["id"])
    return {"baseline_label": previous["label"],
            "baseline_source_sha256": previous["source_manifest"]["sha256"],
            "changes": changed, "regressions": regressions, "incomparable": incomparable}


def render_matrix(report):
    counts = report["counts"]
    lines = ["# ChezCat compatibility matrix — " + report["label"], "",
             "This is a bounded capability inventory, not a standards-conformance score or a claim that a website works. "
             "Each original JavaScript probe starts a fresh interpreter and Node process. The browser inventory below "
             "is a source review; this tool does not rerun App, rendering, networking or security integration tests.", "",
             "**Status:** supported means the stated sample agrees with Node; partial means a limited implementation "
             "or a behavioral difference; missing means the tested capability cannot run; intentionally limited means "
             "an explicit resource/privacy/subset boundary; unassessed means no suitable execution evidence.", "",
             "## Independent probe results", "",
             f"{counts['matches_node']} of {counts['total']} original samples match Node. "
             f"The remaining samples are recorded as capability gaps or explicit limits, not hidden as skipped passes.", "",
             "| Area | Samples matching Node | Total |", "| --- | ---: | ---: |"]
    for area in sorted({row["area"] for row in report["probes"]}):
        rows = [row for row in report["probes"] if row["area"] == area]
        lines.append(f"| {area} | {sum(row['matches_node'] for row in rows)} | {len(rows)} |")
    if "comparison" in report:
        lines += ["", "## Delta from " + report["comparison"]["baseline_label"], "",
                  "| Probe | Baseline | Current |", "| --- | --- | --- |"]
        for row in report["comparison"]["changes"]:
            lines.append(f"| `{row['id']}` | {row['before']} | {row['after']} |")
        if not report["comparison"]["changes"]:
            lines.append("| No sample status changed | — | — |")
        lines += ["", f"Regressions among previously matching comparable samples: **{len(report['comparison']['regressions'])}**. "
                  f"Incomparable/new fixtures: **{len(report['comparison']['incomparable'])}**."]
    lines += ["", "## Remaining sampled differences and limits", "",
              "| Probe | Status | Scope |", "| --- | --- | --- |"]
    for row in report["probes"]:
        if not row["matches_node"]:
            note = (row["note"] or row["label"]).replace("|", "/")
            lines.append(f"| `{row['id']}` | {row['status']} | {note} |")
    lines += ["", "## Browser inventory — source review", "",
              "The named files are evidence locations. Their exact hashes are included in the JSON report. "
              "A source-backed status does not imply that this tool exercised the behavior.", "",
              "| Area | Status | Implemented scope and principal limits | Source evidence |",
              "| --- | --- | --- | --- |"]
    for item in report["browser_inventory"]:
        refs = ", ".join(f"`{name}`" for name in item["evidence_files"])
        lines.append(f"| {item['area']} | {item['status']} | {item['scope']} {item['limits']} | {refs} |")
    upstream = report["test262"]
    lines += ["", "## Selected Test262 evidence", ""]
    if upstream["status"] == "not_run":
        lines.append("Not run: " + upstream["reason"])
    else:
        lines.append(f"Pinned official repository revision: `{upstream['revision']}`. "
                     f"Selected execution results: `{json.dumps(upstream['counts'], sort_keys=True)}`. "
                     "Files, license, harness and download hashes are retained under `tools/verification-0.34/test262/`. "
                     "Default and strict executions are separate results. This is a small selection, not a full Test262 run.")
    lines += ["", "## Reproduce", "", "```sh",
              "python tools/check_compatibility.py --source-root PATH_TO_BASELINE --label 0.33.5 --output tools/verification-0.34/compatibility-baseline.json",
              "python tools/check_compatibility.py --label 0.34 --compare tools/verification-0.34/compatibility-baseline.json --output tools/verification-0.34/compatibility-final.json --matrix COMPATIBILITY_MATRIX.md",
              "```", "", f"Current source-manifest SHA256: `{report['source_manifest']['sha256']}`. "
              f"Original-fixture file SHA256: `{report['fixture_file_sha256']}`. "
              f"Reference: `{report['node']}`; compiler: `{report['rustc']}`.", "",
              "No captured Google access-check code is executed, no tokens are decoded or submitted, and no live-site "
              "requests are made by this checker. Pure URL construction and original Base64 test data do not perform networking.", ""]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, default=ROOT)
    parser.add_argument("--label", default="current")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--compare", type=Path)
    parser.add_argument("--matrix", type=Path)
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--node", default="node")
    options = parser.parse_args()
    source_root = options.source_root.resolve()
    data = json.loads(CASE_FILE.read_text())
    cases = data["probes"]
    if not 1 <= len(cases) <= 100 or len({case['id'] for case in cases}) != len(cases):
        parser.error("Expected between1 and100 distinct independent probes.")
    helper = load_helpers()
    before = source_manifest(source_root)
    for item in data["browser_inventory"]:
        for name in item["evidence_files"]:
            if not (source_root / name).is_file():
                parser.error("Missing inventory source evidence: " + name)
    try:
        rustc = subprocess.check_output([options.rustc, "--version"], text=True, timeout=10).strip()
        node = subprocess.check_output([options.node, "--version"], text=True, timeout=10).strip()
        with tempfile.TemporaryDirectory(prefix="chezcat-capability-") as directory:
            directory = Path(directory)
            source = directory / "probe.rs"
            probe = directory / ("probe.exe" if os.name == "nt" else "probe")
            source.write_text(helper.PROBE.replace("JAVASCRIPT_SOURCE", json.dumps(str(source_root / "src/javascript.rs"))))
            subprocess.run([options.rustc, "--edition=2021", "-O", str(source), "-o", str(probe)],
                           capture_output=True, text=True, timeout=120, check=True)
            results = run_originals(cases, helper, probe, options.node)
            upstream = run_test262(helper, probe, options.node)
        if before != source_manifest(source_root):
            raise ValueError("Source files changed during the check; results are not a frozen-source report.")
    except (OSError, subprocess.SubprocessError, ValueError) as error:
        print("Compatibility checker failed: " + str(error), file=sys.stderr)
        if getattr(error, "stderr", None):
            print(error.stderr, file=sys.stderr)
        return 2
    report = {"schema_version": 1, "label": options.label,
              "scope": "Original independent samples plus a bounded pinned Test262 selection; browser inventory is source review only.",
              "rustc": rustc, "node": node, "source_manifest": before,
              "fixture_file_sha256": digest(CASE_FILE), "helper_sha256": digest(HELPER_FILE),
              "checker_sha256": digest(Path(__file__)),
              "counts": {"total": len(results), "matches_node": sum(row["matches_node"] for row in results),
                         "statuses": dict(Counter(row["status"] for row in results))},
              "probes": results, "browser_inventory": data["browser_inventory"], "test262": upstream}
    if options.compare:
        report["comparison"] = comparison(json.loads(options.compare.read_text()), results)
    options.output.parent.mkdir(parents=True, exist_ok=True)
    options.output.write_text(json.dumps(report, indent=2) + "\n")
    if options.matrix:
        options.matrix.write_text(render_matrix(report))
    summary = {"label": report["label"], "counts": report["counts"],
               "test262": upstream.get("counts", upstream["status"]),
               "source_manifest_sha256": before["sha256"]}
    if "comparison" in report:
        summary["comparison"] = report["comparison"]
    print(json.dumps(summary, indent=2))
    return 1 if (any(row["status"] == "unassessed" for row in results)
                 or report.get("comparison", {}).get("regressions")) else 0


if __name__ == "__main__":
    raise SystemExit(main())
