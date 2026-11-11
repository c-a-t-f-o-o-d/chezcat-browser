#!/usr/bin/env python3
"""Replay frozen independent finite String integration review programs.

Node is a development-only reference, never a ChezCat runtime dependency.
Source programs and raw reference records are preserved across implementation
fixes. This checker makes no network requests.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / 'tools/verification-0.40/check_javascript-inventory-helper.py'
FIXTURES = ROOT / 'tools/string_integration_review_cases_041.json'
REFERENCES = ROOT / 'tools/verification-0.41/string-integration-original-node.json'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def manifest(root):
    return {p.relative_to(root).as_posix(): digest(p) for p in sorted(root.glob('src/**/*.rs'))}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root', type=Path, default=ROOT)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--label', default='current')
    parser.add_argument('--allow-gaps', action='store_true')
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location('string_integration_helper', HELPER)
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    cases = json.loads(FIXTURES.read_text())['cases']
    originals = json.loads(REFERENCES.read_text())
    assert originals['fixture_sha256'] == digest(FIXTURES)
    originals = {c['id']: c for c in originals['cases']}
    root = args.source_root.resolve()
    before = manifest(root)
    paths = [Path(__file__), HELPER, FIXTURES, REFERENCES]
    inputs = {p.relative_to(ROOT).as_posix(): digest(p) for p in paths}
    rows = []
    with tempfile.TemporaryDirectory(prefix='chezcat-string-integration-041-') as tmp:
        tmp = Path(tmp)
        rust = tmp / 'probe.rs'
        probe = tmp / 'probe'
        rust.write_text(helper.PROBE.replace('JAVASCRIPT_SOURCE', json.dumps(str(root / 'src/javascript.rs'))))
        subprocess.run(['rustc', '--edition=2021', '-O', str(rust), '-o', str(probe)], capture_output=True, text=True, check=True, timeout=180)
        for case in cases:
            source = case['source']
            assert hashlib.sha256(source.encode()).hexdigest() == case['source_sha256']
            assert originals[case['id']]['source'] == source
            reference = helper.run_node('node', source, browser=case['browser_global'])
            assert reference == originals[case['id']]['reference'], ('Reference changed', case['id'])
            assert reference['ok'] and reference['records'] and reference['value'] == 'string:6f6b'
            actual = helper.run_own(probe, source, browser=case['browser_global'])
            passed = helper.finished_normally(actual) and actual['ok'] and actual['records'] == reference['records'] and actual['value'] == reference['value']
            rows.append({**case, 'reference': reference, 'actual': actual, 'matches_node': bool(passed)})
    assert manifest(root) == before, 'Source changed during review'
    assert inputs == {p.relative_to(ROOT).as_posix(): digest(p) for p in paths}, 'Frozen input changed during review'
    report = {'label': args.label, 'node': subprocess.check_output(['node', '--version'], text=True).strip(),
        'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(), 'source_manifest': before,
        'inputs': inputs, 'source_and_inputs_unchanged_during_run': True,
        'cases': len(rows), 'matches': sum(x['matches_node'] for x in rows), 'results': rows}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: v for k, v in report.items() if k not in ['source_manifest', 'inputs', 'results']}, indent=2))
    for row in rows:
        if not row['matches_node']:
            print(row['id'] + ': ' + json.dumps(row['actual']))
    return 0 if args.allow_gaps or all(x['matches_node'] for x in rows) else 1


if __name__ == '__main__':
    raise SystemExit(main())
