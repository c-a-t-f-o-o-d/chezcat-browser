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
PROMISE_HELPER_FILE = ROOT / "tools/check_promises.py"
COHORT_FILE = ROOT / "tools/compatibility-progress-cohort.json"
UPSTREAM = ROOT / "tools/reference/test262"
TEST262_HOST_PROFILE = {
    "adapter": "Existing browser-global fixture on both actual and reference sides",
    "native": "Runtime.bind_browser_global(2) with the existing Recorder host",
    "reference": "Node vm.createContext with window/self/globalThis bound to its sandbox",
    "scope": "Language fixtures only; no DOM, network, module, agent or extra realm support implied",
}


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


def load_helpers(helper_file):
    spec = importlib.util.spec_from_file_location("chezcat_existing_checks", helper_file)
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


def run_originals(cases, helper, probe, node, promise_helper=None, promise_probe=None):
    results = []
    for case in cases:
        try:
            selected = promise_helper if case.get("microtasks") else helper
            executable = promise_probe if case.get("microtasks") else probe
            if case.get("microtasks"):
                expected = selected.run_node(node, case["source"])
                actual = selected.run_own(executable, case["source"])
            else:
                browser = bool(case.get("browser_global", False))
                expected = selected.run_node(node, case["source"], browser=browser)
                actual = selected.run_own(executable, case["source"], browser=browser)
        except (OSError, subprocess.SubprocessError, ValueError) as error:
            expected = {"ok": False, "tool_error": str(error)}
            actual = {"ok": False, "tool_error": str(error)}
        matches = bool(expected.get("ok") and helper.finished_normally(actual)
                       and actual.get("ok") and actual["records"] == expected["records"])
        if case.get("microtasks"):
            matches = promise_helper.compare(actual, expected, case.get("expected_diagnostics", 0))
        results.append({"id": case["id"], "area": case["area"], "label": case["label"],
                        "fixture_sha256": hashlib.sha256(case["source"].encode()).hexdigest(),
                        "status": probe_status(case, actual, expected, matches),
                        "matches_node": matches, "note": case["note"],
                        "actual": actual, "reference": expected})
    return results


def selected_metadata(source, available):
    """Conservative adapter for the explicitly pinned synchronous selection.

    This is not a YAML parser. Valid but unsupported metadata is blocked before
    execution, and source text and assertions are never rewritten.
    """
    match = re.search(r"/\*---\s*\n(.*?)\n---\*/", source, re.DOTALL)
    if not match:
        return {"blocked": "Missing or unsupported Test262 frontmatter.", "modes": []}
    metadata = match.group(1)
    fields = {}
    for name in ("flags", "includes"):
        values = re.findall(r"^" + name + r"\s*:(.*)$", metadata, re.MULTILINE)
        if len(values) > 1:
            return {"blocked": "Duplicate " + name + " metadata.", "modes": []}
        if not values:
            fields[name] = []
            continue
        value = values[0].strip()
        if not re.fullmatch(r"\[\s*(?:[A-Za-z0-9_.-]+\s*(?:,\s*[A-Za-z0-9_.-]+\s*)*)?\]", value):
            return {"blocked": "Unsupported " + name + " metadata form.", "modes": []}
        fields[name] = [item.strip() for item in value[1:-1].split(",") if item.strip()]
    flags = set(fields["flags"])
    if flags - {"onlyStrict", "noStrict"}:
        return {"blocked": "Unsupported flags: " + ", ".join(sorted(flags - {"onlyStrict", "noStrict"})), "modes": []}
    if {"onlyStrict", "noStrict"} <= flags:
        return {"blocked": "Contradictory strictness flags.", "modes": []}
    modes = ["strict"] if "onlyStrict" in flags else ["default"] if "noStrict" in flags else ["default", "strict"]
    if re.search(r"^negative\s*:", metadata, re.MULTILINE):
        return {"blocked": "Negative phase/type adapter is unavailable.", "modes": modes}
    if "$262" in source or "$DONE" in source:
        return {"blocked": "Realm/agent/async host harness is unavailable.", "modes": modes}
    harness = ["harness/sta.js", "harness/assert.js"] + ["harness/" + name for name in fields["includes"]]
    missing = [name for name in harness if name not in available]
    if missing:
        return {"blocked": "Unavailable includes: " + ", ".join(missing), "modes": modes}
    return {"modes": modes, "harness": harness, "flags": sorted(flags)}


def native_status(value):
    if value.get("timeout"):
        return "timeout"
    if value.get("crash"):
        return "crash"
    if value.get("protocol_error"):
        return "protocol_error"
    if value.get("tool_error"):
        return "harness_blocked"
    return "passed" if value.get("ok") else "failed"


def run_native(helper, probe, source):
    try:
        return helper.run_own(probe, source, browser=True)
    except (OSError, subprocess.SubprocessError, ValueError) as error:
        return {"ok": False, "tool_error": str(error), "records": []}


def run_reference(helper, node, source):
    try:
        value = helper.run_node(node, source, browser=True)
        if not isinstance(value, dict) or not isinstance(value.get("ok"), bool) or not isinstance(value.get("records"), list):
            raise ValueError("Invalid reference protocol.")
        return value
    except (OSError, subprocess.SubprocessError, ValueError) as error:
        return {"ok": False, "tool_error": str(error), "records": []}


def test262_exit_code(upstream):
    if not upstream.get("controls", {}).get("passed"):
        return 2
    counts = upstream.get("counts", {})
    if upstream.get("unresolved_files"):
        return 2
    if any(counts.get(name, 0) for name in ("harness_blocked", "timeout", "crash", "protocol_error")):
        return 2
    return 1 if counts.get("failed", 0) else 0


def run_test262_controls(helper, probe, node, harness, available):
    """Actual native observations plus separate original adapter/process controls."""
    results = []
    def observe(name, source, ok, records, error_contains=None, strict=False):
        program = ('"use strict";\n' if strict else "") + harness + "\n" + source
        reference = run_reference(helper, node, program)
        actual = run_native(helper, probe, program)
        passed = (not reference.get("tool_error") and reference.get("ok") is ok
                  and reference.get("records") == records
                  and native_status(actual) == ("passed" if ok else "failed")
                  and actual.get("records") == records
                  and (error_contains is None or (error_contains in (actual.get("error") or "") and error_contains in (reference.get("error") or ""))))
        results.append({"id": name, "passed": passed, "kind": "actual_native_observation",
                        "actual": actual, "reference": reference})
    def record(text):
        return "string:" + text.encode().hex()
    observe("known-pass-body", "assert.sameValue(2+3,5);test.record('body');", True, [record("body")])
    observe("known-assertion-failure",
            "test.record('before-assert');try{assert.sameValue(1,2,'original-control-assertion');}"
            "catch(error){test.record(error instanceof Test262Error);test.record(error.message);throw error;}"
            "test.record('after');", False,
            [record("before-assert"), "boolean:true",
             record("original-control-assertion Expected SameValue(\u00ab1\u00bb, \u00ab2\u00bb) to be true")])
    observe("invalid-syntax-no-body", "test.record('must-not-run');var = ;", False, [])
    observe("runtime-throw-after-body", "test.record('before');throw new Error('original-control-throw');test.record('after');", False, [record("before")], error_contains="original-control-throw")
    observe("sloppy-this", "assert.sameValue((function(){return this===undefined;})(),false);test.record('sloppy');", True, [record("sloppy")])
    observe("strict-this", "assert.sameValue((function(){return this===undefined;})(),true);test.record('strict');", True, [record("strict")], strict=True)
    def meta(text):
        return "/*---\n" + text + "\n---*/\ntest.record('not-executed');"
    unsupported = {
        "missing-include": "includes: [unavailable-control.js]",
        "block-includes": "includes:\n  - compareArray.js",
        "unknown-flags": "flags: [inventedControlFlag]",
        "module": "flags: [module]", "async": "flags: [async]", "raw": "flags: [raw]",
        "contradictory-strict": "flags: [onlyStrict, noStrict]",
        "parse-negative": "negative:\n  phase: parse\n  type: SyntaxError",
        "runtime-negative": "negative:\n  phase: runtime\n  type: TypeError",
    }
    for name, text in unsupported.items():
        value = selected_metadata(meta(text), available)
        results.append({"id": name, "kind": "metadata_only_not_executed", "passed": bool(value.get("blocked")), "metadata": value})
    for name, body in (("agent", "$262.agent.start('');"), ("realm", "$262.createRealm();"), ("done", "$DONE();")):
        value = selected_metadata(meta("description: original control") + body, available)
        results.append({"id": name, "kind": "metadata_only_not_executed", "passed": bool(value.get("blocked")), "metadata": value})
    for flags, expected in (("", ["default", "strict"]), ("flags: [onlyStrict]", ["strict"]), ("flags: [noStrict]", ["default"])):
        value = selected_metadata(meta(flags or "description: original control"), available)
        results.append({"id": "modes-" + (flags or "default"), "kind": "metadata_only_not_executed", "passed": not value.get("blocked") and value.get("modes") == expected, "metadata": value})
    # These tiny owned children test process/outcome classification, not JS support.
    for name, body, timeout, expected in (("timeout", "import time;time.sleep(1)", .05, "timeout"),
                                          ("crash", "raise SystemExit(7)", 2, "crash"),
                                          ("protocol", "print('invalid-control-output')", 2, "protocol_error")):
        try:
            child = subprocess.run([sys.executable, "-c", body], capture_output=True, text=True, timeout=timeout)
            lines = child.stdout.splitlines()
            valid = bool(lines and (lines[0].startswith("OK\t") or lines[0].startswith("ERROR\t")))
            value = {"ok": False, "crash": child.returncode} if child.returncode else {"ok": False, "protocol_error": not valid}
        except subprocess.TimeoutExpired:
            value = {"ok": False, "timeout": True}
        results.append({"id": "owned-process-" + name, "kind": "process_control_not_js", "passed": native_status(value) == expected, "actual": value})
    missing_node = Path(probe).parent / "unavailable-reference-control.exe"
    if missing_node.exists():
        raise ValueError("Unexpected reference control executable exists.")
    missing_reference = run_reference(helper, str(missing_node), "1;")
    results.append({"id": "missing-reference-executable", "kind": "actual_runner_error_not_js", "passed": bool(missing_reference.get("tool_error")) and native_status(missing_reference) == "harness_blocked", "actual": missing_reference})
    blocked_reference = {"ok": False, "tool_error": "original reference control", "records": []}
    results.append({"id": "reference-error-is-blocked", "kind": "classification_only", "passed": native_status(blocked_reference) == "harness_blocked"})
    for name, counts, expected in (("pass", {"passed": 1}, 0), ("failed", {"failed": 1}, 1), ("blocked", {"harness_blocked": 1}, 2)):
        results.append({"id": "exit-gate-" + name, "kind": "classification_only", "passed": test262_exit_code({"controls": {"passed": True}, "counts": counts}) == expected})
    results.append({"id": "unresolved-file-exit-gate", "kind": "classification_only", "passed": test262_exit_code({"controls": {"passed": True}, "counts": {}, "unresolved_files": [{"path": "original-control"}]}) == 2})
    return {"host_profile": TEST262_HOST_PROFILE, "passed": all(row["passed"] for row in results), "total": len(results),
            "passed_count": sum(row["passed"] for row in results), "results": results,
            "negative_phase_support": "Unavailable: invalid-syntax and body-throw observations do not expose a typed suite-negative phase/type API."}


def run_test262(helper, probe, node, prefixes=(), controls_only=False):
    manifest_path = UPSTREAM / "manifest.json"
    if not manifest_path.is_file():
        return {"status": "not_run", "reason": "No pinned upstream selection is available."}
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    revision = manifest["revision"]
    if not 1 <= len(manifest["files"]) <= 64:
        raise ValueError("Pinned input selection must contain 1-64 files.")
    if not re.fullmatch(r"[0-9a-f]{40}", revision) or manifest["repository"] != "https://github.com/tc39/test262":
        raise ValueError("Invalid pinned upstream identity.")
    good = {}
    for entry in manifest["files"]:
        name = entry["path"]
        if name.startswith("/") or "\\" in name or any(part in ("", ".", "..") for part in name.split("/")) or name in good:
            raise ValueError("Unsafe or duplicate pinned upstream path: " + name)
        path = UPSTREAM / name
        if not isinstance(entry.get("bytes"), int) or not 0 < entry["bytes"] <= 1024 * 1024:
            raise ValueError("Pinned input file size is invalid or above 1 MiB: " + name)
        if not path.resolve().is_relative_to(UPSTREAM.resolve()):
            raise ValueError("Pinned upstream path escapes input tree.")
        if not entry.get("ok") or digest(path) != entry["sha256"] or path.stat().st_size != entry["bytes"]:
            raise ValueError("Pinned Test262 bytes changed or incomplete: " + name)
        if entry["url"] != "https://raw.githubusercontent.com/tc39/test262/" + revision + "/" + name:
            raise ValueError("Pinned upstream URL does not match revision/path: " + name)
        good[name] = entry
    required = ["LICENSE", "harness/sta.js", "harness/assert.js"]
    if any(name not in good for name in required):
        raise ValueError("Required pinned license/harness is unavailable.")
    harness = "\n".join((UPSTREAM / name).read_text(encoding="utf-8") for name in required[1:])
    controls = run_test262_controls(helper, probe, node, harness, good)
    selected = [name for name in good if name.startswith("test/") and (not prefixes or any(name.startswith(prefix) for prefix in prefixes))]
    if not selected:
        raise ValueError("No pinned upstream files match the selection.")
    results = []
    for name in selected:
        source = (UPSTREAM / name).read_text(encoding="utf-8")
        metadata = selected_metadata(source, good)
        if controls_only:
            results.append({"path": name, "status": "unrun", "modes": metadata["modes"], "reason": "Controls-only invocation."})
            continue
        if metadata.get("blocked") or not controls["passed"]:
            results.append({"path": name, "status": "harness_blocked", "modes": metadata["modes"], "reason": metadata.get("blocked", "Mandatory runner controls failed.")})
            continue
        for mode in metadata["modes"]:
            program = ('"use strict";\n' if mode == "strict" else "")
            program += "\n".join((UPSTREAM / item).read_text(encoding="utf-8") for item in metadata["harness"])
            program += "\n" + source
            reference = run_reference(helper, node, program)
            actual = None
            if not reference.get("ok"):
                status = "harness_blocked"
                reason = "Reference did not complete successfully; no engine verdict."
            else:
                actual = run_native(helper, probe, program)
                status = native_status(actual)
                reason = None
            results.append({"path": name, "mode": mode, "status": status, "reason": reason,
                            "file_sha256": good[name]["sha256"], "actual": actual, "reference": reference})
    for name, entry in good.items():
        if digest(UPSTREAM / name) != entry["sha256"] or (UPSTREAM / name).stat().st_size != entry["bytes"]:
            raise ValueError("Pinned input bytes changed during the run: " + name)
    counts = Counter()
    for row in results:
        counts[row["status"]] += len(row["modes"]) if "modes" in row else 1
    unresolved = [row for row in results if "modes" in row and not row["modes"]]
    return {"status": "controls_only" if controls_only else "selected_tests_only", "repository": manifest["repository"],
            "revision": revision, "host_profile": TEST262_HOST_PROFILE, "manifest_sha256": digest(manifest_path), "controls": controls,
            "selected_files": len(selected), "selected_modes": sum(len(row.get("modes", [row.get("mode")])) for row in results),
            "executed_modes": sum(row.get("actual") is not None for row in results),
            "counts": dict(counts), "counts_scope": "Known selected execution modes; unresolved metadata files are separate, never passes.",
            "unresolved_files": unresolved, "harness_blocked_files": len({row["path"] for row in results if row["status"] == "harness_blocked"}),
            "results": results,
            "limits": "Selected unchanged upstream files and pinned harness only; blocked files/modes are never passes. No suite-wide conformance claim."}


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


def progress_cohort(results):
    cohort = json.loads(COHORT_FILE.read_text(encoding="utf-8"))
    by_id = {row["id"]: row for row in results}
    incomparable = [row["id"] for row in cohort["probes"]
                    if row["id"] not in by_id or
                    by_id[row["id"]]["fixture_sha256"] != row["fixture_sha256"]]
    total = len(cohort["probes"])
    matches = sum(by_id[row["id"]]["matches_node"] for row in cohort["probes"]
                  if row["id"] not in incomparable)
    return {"label": cohort["label"], "total": total, "matches_node": matches,
            "percentage": None if incomparable else round(100 * matches / total, 2),
            "incomparable": incomparable, "cohort_sha256": digest(COHORT_FILE),
            "scope": cohort["scope"]}


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
    progress = report.get("progress_reference_cohort")
    if progress:
        percentage = (f"{progress['percentage']}%" if progress["percentage"] is not None
                      else "percentage unavailable: changed or missing fixtures")
        lines += ["", "## Stable progress measure", "",
                  f"The fixed {progress['total']}-probe {progress['label']} has "
                  f"**{progress['matches_node']}/{progress['total']} matches ({percentage})**. "
                  "Its IDs and source hashes stay fixed as new probes are added to the expanded inventory.", "",
                  "This percentage measures the selected probes, not overall browser completion, "
                  "website coverage or remaining engineering effort. Partial source-review areas are not counted as complete."]
        if "baseline" in progress:
            baseline = progress["baseline"]
            lines += ["", f"The comparison baseline matches **{baseline['matches_node']}/{baseline['total']} "
                      f"({baseline['percentage']}%)** of this same cohort."]
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
                     "Files, license, harness and download hashes are retained under `tools/reference/test262/`. "
                     "Default and strict executions are separate results. This is a small selection, not a full Test262 run.")
    lines += ["", "## Reproduce", "", "```sh",
              "python tools/check_compatibility.py --source-root PATH_TO_BASELINE --label baseline --output results/verification/compatibility-baseline.json",
              "python tools/check_compatibility.py --label current --compare results/verification/compatibility-baseline.json --output results/verification/compatibility.json --matrix docs/COMPATIBILITY_MATRIX.md",
              "```", "", f"Current source-manifest SHA256: `{report['source_manifest']['sha256']}`. "
              f"Original-fixture file SHA256: `{report['fixture_file_sha256']}`. "
              f"Reference: `{report['node']}`; compiler: `{report['rustc']}`.", "",
              "No captured Google access-check code is executed, no tokens are decoded or submitted, and no live-site "
              "requests are made by this checker. Pure URL construction and original Base64 test data do not perform networking.", ""]
    return "\n".join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, default=ROOT)
    parser.add_argument("--label", default="current")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--compare", type=Path)
    parser.add_argument("--matrix", type=Path)
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--node", default="node")
    parser.add_argument("--helper-file", type=Path, default=HELPER_FILE,
                        help="Existing JavaScript probe helper, optionally a pinned snapshot")
    parser.add_argument("--promise-helper-file", type=Path, default=PROMISE_HELPER_FILE,
                        help="Separate explicit-checkpoint helper for probes marked microtasks")
    parser.add_argument("--test262-only", action="store_true", help="Run the bounded upstream selection without the original inventory")
    parser.add_argument("--test262-controls-only", action="store_true", help="Run mandatory actual-native/adapter controls and leave upstream tests unrun")
    parser.add_argument("--test262-prefix", action="append", default=[], help="Select pinned test paths by literal prefix (repeatable)")
    options = parser.parse_args(argv)
    selected_only = options.test262_only or options.test262_controls_only
    if any(not prefix or not prefix.startswith("test/") for prefix in options.test262_prefix):
        parser.error("Test262 prefixes must be nonempty test/ paths.")
    if selected_only and (options.compare or options.matrix):
        parser.error("Comparison/matrix is for the original inventory; omit it for a selected upstream invocation.")
    source_root = options.source_root.resolve()
    data = {} if selected_only else json.loads(CASE_FILE.read_text(encoding="utf-8"))
    cases = [] if selected_only else data["probes"]
    if not selected_only and (not 1 <= len(cases) <= 256 or len({case['id'] for case in cases}) != len(cases)):
        parser.error("Expected between 1 and 256 distinct independent probes.")
    helper_file = options.helper_file.resolve()
    helper = load_helpers(helper_file)
    promise_helper_file = options.promise_helper_file.resolve()
    promise_helper = load_helpers(promise_helper_file) if any(case.get("microtasks") for case in cases) else None
    review_file = source_root / "tools/compatibility_cases.json"
    browser_inventory = [] if selected_only else json.loads(review_file.read_text(encoding="utf-8"))["browser_inventory"]
    before = source_manifest(source_root)
    input_paths = [Path(__file__), helper_file]
    if not selected_only:
        input_paths.extend([CASE_FILE, COHORT_FILE, review_file])
    if promise_helper:
        input_paths.append(promise_helper_file)
    if (UPSTREAM / "manifest.json").is_file():
        input_paths.append(UPSTREAM / "manifest.json")
    input_hashes = {path: digest(path) for path in input_paths}
    for item in browser_inventory:
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
            source.write_text(helper.PROBE.replace("JAVASCRIPT_SOURCE", json.dumps(str(source_root / "src/javascript.rs"))), encoding="utf-8")
            subprocess.run([options.rustc, "--edition=2021", "-O", str(source), "-o", str(probe)],
                           capture_output=True, text=True, timeout=120, check=True)
            promise_probe, checkpoint_available = (promise_helper.compile_probe(source_root, directory, options.rustc)
                                                   if promise_helper else (None, False))
            results = run_originals(cases, helper, probe, options.node, promise_helper, promise_probe)
            upstream = run_test262(helper, probe, options.node, options.test262_prefix, options.test262_controls_only)
        after = source_manifest(source_root)
        if before != after:
            raise ValueError("Source files changed during the check; results are not a frozen-source report.")
        if any(digest(path) != sha for path, sha in input_hashes.items()):
            raise ValueError("Checker, fixture or reference inputs changed during the check.")
    except (OSError, subprocess.SubprocessError, ValueError) as error:
        print("Compatibility checker failed: " + str(error), file=sys.stderr)
        if getattr(error, "stderr", None):
            print(error.stderr, file=sys.stderr)
        return 2
    report = {"schema_version": 1, "label": options.label,
              "scope": "Original independent samples plus a bounded pinned Test262 selection; browser inventory is source review only.",
              "rustc": rustc, "node": node, "source_manifest": before,
              "source_manifest_after": after,
              "checker_and_fixture_inputs_unchanged_during_run": True,
              "input_manifest": {str(path): sha for path, sha in input_hashes.items()},
              "fixture_file_sha256": None if selected_only else digest(CASE_FILE), "helper_sha256": digest(helper_file),
              "checker_sha256": digest(Path(__file__)),
              "counts": {"total": len(results), "matches_node": sum(row["matches_node"] for row in results),
                         "statuses": dict(Counter(row["status"] for row in results))},
              "probes": results, "browser_inventory": browser_inventory, "test262": upstream,
              "browser_inventory_file_sha256": None if selected_only else digest(review_file),
              "promise_helper_sha256": digest(promise_helper_file) if promise_helper else None,
              "microtask_probes": sum(bool(case.get("microtasks")) for case in cases),
              "microtask_checkpoint_available": checkpoint_available}
    if not selected_only:
        report["progress_reference_cohort"] = progress_cohort(results)
    if options.compare:
        previous = json.loads(options.compare.read_text(encoding="utf-8"))
        report["comparison"] = comparison(previous, results)
        report["progress_reference_cohort"]["baseline"] = progress_cohort(previous["probes"])
    options.output.parent.mkdir(parents=True, exist_ok=True)
    options.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if options.matrix:
        options.matrix.write_text(render_matrix(report), encoding="utf-8")
    summary = {"label": report["label"], "counts": report["counts"],
               "test262": upstream.get("counts", upstream["status"]),
               "source_manifest_sha256": before["sha256"]}
    if "comparison" in report:
        summary["comparison"] = report["comparison"]
    print(json.dumps(summary, indent=2))
    upstream_exit = test262_exit_code(upstream)
    if upstream_exit:
        return upstream_exit
    return 1 if (any(row["status"] == "unassessed" for row in results)
                 or report.get("comparison", {}).get("regressions")) else 0


if __name__ == "__main__":
    raise SystemExit(main())
