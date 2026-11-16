#!/usr/bin/env python3
"""Retain uncaptured native test output; retry failures with one worker.

Development-only. Run from the project root: python tools/capture_tests.py
Uses the existing offline/locked Cargo test profile through rustup stable. It
never changes source, raises a runtime limit, installs tools, or runs a site.
A successful diagnostic retry NEVER changes a failed normal run into a pass.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time
import uuid


ROOT = Path(__file__).resolve().parents[1]
MAX_LOG_BYTES = 32 * 1024 * 1024
SUMMARY = re.compile(
    r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; "
    r"(\d+) ignored; (\d+) measured; (\d+) filtered out;", re.MULTILINE)
START = re.compile(r"^running (\d+) tests?$", re.MULTILINE)
STATUS = re.compile(r"exit code: (0x[0-9a-fA-F]+)")
MAX_PANICKING_NAMES = 8
PANICKING_THREAD = re.compile(
    r"^thread '([A-Za-z_][A-Za-z_0-9:]{0,511})'(?: \([0-9]{1,20}\))? "
    r"(?:panicked at(?:[ :]|$)|has overflowed its stack(?:[.\t ]|$))", re.MULTILINE)
PANIC_IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z_0-9]*(?:::[A-Za-z_][A-Za-z_0-9]*)*\Z")


def now():
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n",
                         encoding="utf-8")
    # Windows readers can briefly deny replacement of a just-written manifest.
    # Preserve atomic publication and the prior manifest; never swallow a
    # persistent permission error or report a test pass when publication failed.
    for attempt in range(7):
        try:
            temporary.replace(path)
            break
        except PermissionError:
            if attempt == 6:
                raise
            time.sleep(0.01 * (2 ** attempt))


def source_identity(root):
    """Hash compilation inputs, not personal settings or the entire environment."""
    paths = [root / name for name in ("Cargo.toml", "Cargo.lock", "build.rs", "LICENSE",
                                       "LICENSE_SCOPE.md", "THIRD_PARTY_NOTICES.md", "SOURCE_REVISION")]
    for directory in ("src", "data", "resources", "assets", "pages", "trust",
                      "tools/fixtures", "tools/reference"):
        paths += sorted((root / directory).rglob("*"))
    for name in ("rust-toolchain", "rust-toolchain.toml", ".cargo/config", ".cargo/config.toml"):
        path = root / name
        if path.is_file():
            paths.append(path)
    return {path.relative_to(root).as_posix(): sha256(path)
            for path in paths if path.is_file()}


def stop_tree(process):
    if process.poll() is not None:
        return
    if os.name == "nt":
        try:
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                           timeout=15, check=False)
        except (OSError, subprocess.TimeoutExpired):
            pass
    else:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    if process.poll() is None:
        process.kill()
    process.wait(timeout=15)


def execute(command, log, root, timeout_seconds):
    """Write directly to disk so native aborts cannot lose a Python buffer.

    The output limit is polled: output can exceed it between polls. Do not call
    this a hard disk quota. The complete partial log remains available on abort,
    timeout, cancellation, or a launch error.
    """
    row = {"command": command, "log": log.name, "started_utc": now()}
    started = time.monotonic()
    process = None
    try:
        with log.open("wb") as stream:
            options = ({"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP}
                       if os.name == "nt" else {"start_new_session": True})
            environment = dict(os.environ)
            environment["RUST_BACKTRACE"] = "1"
            process = subprocess.Popen(command, cwd=str(root), env=environment,
                                       stdin=subprocess.DEVNULL, stdout=stream,
                                       stderr=subprocess.STDOUT, **options)
            row["pid"] = process.pid
            while process.poll() is None:
                if log.stat().st_size > MAX_LOG_BYTES:
                    row["state"] = "output_limit"
                    stop_tree(process)
                    break
                if time.monotonic() - started >= timeout_seconds:
                    row["state"] = "timeout"
                    stop_tree(process)
                    break
                time.sleep(0.05)
            row["exit_code"] = process.wait(timeout=15)
            if "state" not in row:
                row["state"] = ("output_limit" if log.stat().st_size > MAX_LOG_BYTES
                                else "completed" if row["exit_code"] == 0 else "failed")
    except KeyboardInterrupt:
        row["state"] = "interrupted"
        if process is not None:
            stop_tree(process)
    except (OSError, subprocess.SubprocessError) as error:
        row.update({"state": "launch_error", "error": str(error)})
        if process is not None:
            stop_tree(process)
    finally:
        row["finished_utc"] = now()
        row["elapsed_seconds"] = round(time.monotonic() - started, 3)
        if log.exists():
            row["bytes"] = log.stat().st_size
            row["sha256"] = sha256(log)
    return row


def panicking_names(text):
    # Reported Rust panic/stack-overflow thread labels are diagnostic only.
    # Expected panics or a named spawned thread do not establish test failure.
    # Never retain panic messages, locations, unnamed threads or arbitrary text.
    names = []
    seen = set()
    for match in PANICKING_THREAD.finditer(text):
        name = match.group(1)
        if not PANIC_IDENTIFIER.fullmatch(name) or name in seen:
            continue
        if len(names) == MAX_PANICKING_NAMES:
            return names, True
        seen.add(name)
        names.append(name)
    return names, False


def analyze(log):
    with log.open("rb") as stream:
        data = stream.read(MAX_LOG_BYTES + 1)
    truncated = len(data) > MAX_LOG_BYTES
    # Raw logs/hashes stay byte-for-byte intact. Python/native producers on
    # Windows may use CRLF; line-oriented harness analysis accepts both forms.
    text = data[:MAX_LOG_BYTES].decode("utf-8", errors="replace").replace("\r\n", "\n")
    starts = START.findall(text)
    summaries = SUMMARY.findall(text)
    result = {"analysis_truncated": truncated,
              "harness_started": bool(starts),
              "native_exit_statuses": STATUS.findall(text),
              "completed_test_lines": len(re.findall(r"^test \S+ \.\.\. ok$", text, re.MULTILINE)),
              "failed_test_names": re.findall(r"^test (\S+) \.\.\. FAILED$", text, re.MULTILINE),
              "complete_success_summary": False}
    result["panicking_test_names"], result["panicking_names_limited"] = panicking_names(text)
    # A single explicitly selected binary must produce one complete test run.
    # Exit code alone, an empty filter, or a serial success is not a gate.
    if len(starts) == 1 and len(summaries) == 1:
        state, passed, failed, ignored, measured, filtered = summaries[0]
        counts = dict(zip(("passed", "failed", "ignored", "measured", "filtered"),
                          map(int, (passed, failed, ignored, measured, filtered))))
        result["summary"] = {"state": state, **counts}
        result["announced_tests"] = int(starts[0])
        result["complete_success_summary"] = bool(
            not truncated and state == "ok" and counts["passed"] > 0
            and counts["failed"] == counts["ignored"] == counts["measured"] == counts["filtered"] == 0
            and counts["passed"] == int(starts[0]))
    artifacts = []
    for line in text.splitlines():
        if not line.startswith('{"reason":'):
            continue
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if (item.get("reason") == "compiler-artifact"
                and item.get("target", {}).get("name") == "chezcat"
                and item.get("profile", {}).get("test") is True
                and isinstance(item.get("executable"), str)):
            path = Path(item["executable"])
            # Hash metadata only. Never execute a path extracted from a log.
            artifacts.append({"path": str(path), "sha256": sha256(path) if path.is_file() else None})
    result["test_executables"] = artifacts
    return result


def passed(row):
    return (row.get("state") == "completed" and row.get("exit_code") == 0
            and row.get("analysis", {}).get("complete_success_summary") is True)


def commands(toolchain):
    prefix = ["rustup", "run", toolchain]
    cargo = prefix + ["cargo", "test", "--offline", "--locked", "--bin", "chezcat",
                      "--message-format=json", "--", "--no-capture", "--color=never"]
    return [("compiler", prefix + ["rustc", "-Vv"]),
            ("normal", cargo), ("serial", cargo + ["--test-threads=1"])]


def capture(root, run, plan, timeout_seconds):
    run.mkdir(parents=True, exist_ok=False)
    before = source_identity(root)
    manifest = {"schema_version": 1, "started_utc": now(), "state": "running",
                "scope": "Native test diagnostics only; no browser release build or site capture.",
                "source_before": before, "steps": [], "normal_test_gate_passed": False,
                "runner_sha256": sha256(Path(__file__)), "effective_RUST_BACKTRACE": "1",
                "timeout_seconds_per_stage": timeout_seconds,
                "polled_log_limit_bytes": MAX_LOG_BYTES,
                "environment": {key: os.environ.get(key) for key in (
                    "RUST_TEST_THREADS", "RUST_MIN_STACK", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS",
                    "CARGO_BUILD_TARGET", "CARGO_TARGET_DIR", "RUSTUP_TOOLCHAIN")}}
    path = run / "manifest.json"
    write_json(path, manifest)
    try:
        for label, command in plan:
            if label == "serial":
                normal = next(row for row in manifest["steps"] if row["name"] == "normal")
                if passed(normal) or not normal["analysis"]["harness_started"]:
                    break
            print("Running " + label + " stage; raw output: " + str(run / (label + ".log")), flush=True)
            row = {"name": label, "command": command, "state": "running"}
            manifest["steps"].append(row)
            write_json(path, manifest)
            row.update(execute(command, run / (label + ".log"), root, timeout_seconds))
            if label != "compiler":
                row["analysis"] = analyze(run / (label + ".log"))
            write_json(path, manifest)
            if row["state"] in ("interrupted", "launch_error", "timeout", "output_limit"):
                break
            if label == "compiler" and row["exit_code"] != 0:
                break
        after = source_identity(root)
        manifest["source_after"] = after
        manifest["source_unchanged"] = before == after
        normal = next((row for row in manifest["steps"] if row["name"] == "normal"), {})
        manifest["normal_test_gate_passed"] = passed(normal) and before == after
        manifest["state"] = "passed" if manifest["normal_test_gate_passed"] else "not_passed"
    except KeyboardInterrupt:
        manifest["state"] = "interrupted"
    except (OSError, ValueError) as error:
        manifest.update({"state": "error", "error": str(error)})
    finally:
        manifest["finished_utc"] = now()
        write_json(path, manifest)
        lines = ["ChezCat native test diagnostics", "Normal test gate: " +
                 ("PASSED" if manifest["normal_test_gate_passed"] else "NOT PASSED")]
        for row in manifest["steps"]:
            lines.append(row["name"] + ": " + row["state"] + "; exit=" + str(row.get("exit_code")))
            analysis = row.get("analysis", {})
            if "summary" in analysis:
                lines.append("  " + json.dumps(analysis["summary"], sort_keys=True))
            if analysis.get("native_exit_statuses"):
                lines.append("  Native status: " + ", ".join(analysis["native_exit_statuses"]))
        lines += ["A serial retry is diagnostic; it cannot erase a normal-run failure.",
                  "Keep this entire folder, including both logs and manifest.json."]
        (run / "summary.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return manifest


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain", default="stable")
    parser.add_argument("--timeout-seconds", type=int, default=600)
    args = parser.parse_args(argv)
    if not re.fullmatch(r"[A-Za-z0-9_.-]+", args.toolchain) or args.toolchain.startswith("-"):
        parser.error("Use a rustup toolchain name, not a shell command or option")
    if not 15 <= args.timeout_seconds <= 1800:
        parser.error("--timeout-seconds must be 15..1800")
    run = ROOT / "results/verification/windows-tests" / (
        datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ") + "-" + uuid.uuid4().hex[:8])
    result = capture(ROOT, run, commands(args.toolchain), args.timeout_seconds)
    print("\n" + (run / "summary.txt").read_text(encoding="utf-8"))
    print("Results: " + str(run))
    return 0 if result["normal_test_gate_passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
