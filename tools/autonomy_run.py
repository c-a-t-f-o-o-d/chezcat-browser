"""Bounded, logged local commands for the assistant-owned development loop."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shutil
import sys
import uuid

import capture_tests

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name", required=True)
    parser.add_argument("--timeout", type=int, default=600)
    parser.add_argument("--preserve-executable", action="store_true")
    parser.add_argument("--command-file", type=Path)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if not args.name.replace("-", "").replace("_", "").isalnum():
        parser.error("name must be alphanumeric with hyphens/underscores")
    if not 1 <= args.timeout <= 1800:
        parser.error("timeout must be 1..1800 seconds")
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if args.command_file:
        if command:
            parser.error("choose command-file or command arguments")
        command = json.loads(args.command_file.read_text(encoding="utf-8"))
        if not isinstance(command, list) or not all(isinstance(arg, str) for arg in command):
            parser.error("command-file must contain a JSON string array")
    if not command:
        parser.error("a command is required")
    folder = ROOT / "results/verification/autonomy" / (
        datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
        + "-" + args.name + "-" + uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    os.environ["PATH"] = str(Path(sys.executable).parent) + os.pathsep + os.environ["PATH"]
    executable = ROOT / "ChezCat.exe"
    manifest = {"name": args.name, "source_before": capture_tests.source_identity(ROOT),
                "runner_sha256": capture_tests.sha256(Path(__file__)),
                "executable_before_sha256": capture_tests.sha256(executable)
                if executable.is_file() else None}
    if args.preserve_executable and executable.is_file():
        shutil.copy2(executable, folder / "incoming-ChezCat.exe")
    capture_tests.write_json(folder / "manifest.json", manifest)
    print("Evidence: " + str(folder), flush=True)
    manifest["command"] = capture_tests.execute(command, folder / "command.log", ROOT, args.timeout)
    manifest["source_after"] = capture_tests.source_identity(ROOT)
    manifest["source_unchanged"] = manifest["source_before"] == manifest["source_after"]
    manifest["executable_after_sha256"] = capture_tests.sha256(executable) if executable.is_file() else None
    capture_tests.write_json(folder / "manifest.json", manifest)
    print((folder / "command.log").read_text(encoding="utf-8", errors="replace")[-12000:], flush=True)
    print("Command state: " + manifest["command"]["state"], flush=True)
    return 0 if manifest["command"]["state"] == "completed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
