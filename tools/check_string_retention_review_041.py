#!/usr/bin/env python3
"""Run independent String accounting assertions in a temporary source copy."""
from pathlib import Path
import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TESTS = ROOT / 'tools/string_retention_review_041.rs'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root', type=Path, default=ROOT)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    root = args.source_root.resolve()
    source_manifest = {p.relative_to(root).as_posix(): digest(p) for p in sorted(root.glob('src/**/*.rs'))}
    test_hash = digest(TESTS)
    with tempfile.TemporaryDirectory(prefix='chezcat-string-retention-041-') as tmp:
        tmp = Path(tmp)
        shutil.copytree(root / 'src', tmp / 'src')
        shutil.copyfile(TESTS, tmp / 'src/independent_string_retention_review.rs')
        source = tmp / 'src/javascript.rs'
        with source.open('a') as out:
            out.write('\n#[cfg(test)]\n#[path="independent_string_retention_review.rs"]\nmod independent_string_retention_review;\n')
        harness = tmp / 'harness.rs'
        harness.write_text('#![allow(dead_code)]\n#[path="src/javascript.rs"]\nmod javascript;\n', encoding="utf-8")
        probe = tmp / 'tests'
        compiled = subprocess.run(['rustc', '--edition=2021', '--test', str(harness), '-o', str(probe)], capture_output=True, text=True, timeout=180)
        if compiled.returncode:
            raise RuntimeError(compiled.stderr)
        run = subprocess.run([str(probe), 'independent_string_retention_review::', '--nocapture'], capture_output=True, text=True, timeout=30)
    assert source_manifest == {p.relative_to(root).as_posix(): digest(p) for p in sorted(root.glob('src/**/*.rs'))}
    assert test_hash == digest(TESTS)
    report = {'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(),
        'source_manifest': source_manifest, 'test_sha256': test_hash, 'checker_sha256': digest(Path(__file__)),
        'source_and_tests_unchanged_during_run': True, 'exit_code': run.returncode,
        'stdout': run.stdout, 'stderr': run.stderr}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n', encoding="utf-8")
    print(run.stdout)
    print(run.stderr, end='')
    return run.returncode


if __name__ == '__main__':
    raise SystemExit(main())
