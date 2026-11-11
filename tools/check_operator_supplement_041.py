#!/usr/bin/env python3
"""Frozen original 0.41 operator comparisons, current-spec assertions and bounds.

Node is a development reference only. Programs and reference records are frozen
before production changes; current-spec key-caching assertions preserve the
Node discrepancy explicitly. No networking or remote script execution occurs.
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
FIXTURES = ROOT / 'tools/operator_supplement_cases_041.json'
REFERENCES = ROOT / 'tools/verification-0.41/operator-supplement-original-node.json'
ADJUDICATION = ROOT / 'tools/verification-0.41/operator-supplement-bound-adjudication.json'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root', type=Path, default=ROOT)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--label', default='current')
    parser.add_argument('--allow-gaps', action='store_true')
    options = parser.parse_args()
    spec = importlib.util.spec_from_file_location('operator_helper_041', HELPER)
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    fixture_data = json.loads(FIXTURES.read_text())
    reference_data = json.loads(REFERENCES.read_text())
    assert reference_data['fixture_sha256'] == digest(FIXTURES)
    originals = {c['id']: c for c in reference_data['cases']}
    adjudication = json.loads(ADJUDICATION.read_text())
    assert adjudication['fixture_sha256'] == digest(FIXTURES)
    bound_assertions = {c['id']: c for c in adjudication['cases']}
    root = options.source_root.resolve()
    files = sorted(root.glob('src/**/*.rs'))
    source_manifest = {p.relative_to(root).as_posix(): digest(p) for p in files}
    input_paths = [Path(__file__), HELPER, FIXTURES, REFERENCES, ADJUDICATION]
    inputs = {str(p.relative_to(ROOT)): digest(p) for p in input_paths}
    results = []
    early = []
    bounds = []
    with tempfile.TemporaryDirectory(prefix='chezcat-operators-041-') as tmp:
        tmp = Path(tmp)
        rust = tmp / 'probe.rs'
        probe = tmp / 'probe'
        rust.write_text(helper.PROBE.replace('JAVASCRIPT_SOURCE', json.dumps(str(root / 'src/javascript.rs'))))
        subprocess.run(['rustc', '--edition=2021', '-O', str(rust), '-o', str(probe)], capture_output=True, text=True, check=True, timeout=180)
        for case in fixture_data['cases']:
            source = case['source']
            assert hashlib.sha256(source.encode()).hexdigest() == case['source_sha256']
            assert originals[case['id']]['source'] == source
            reference = helper.run_node('node', source, browser=case['browser_global'])
            assert reference == originals[case['id']]['reference'], ('Reference changed', case['id'])
            actual = helper.run_own(probe, source, browser=case['browser_global'])
            if case['kind'] == 'early-error':
                assert not reference['ok'] and not reference['records'] and 'SyntaxError' in reference['error']
                passed = helper.finished_normally(actual) and not actual['ok'] and not actual['records']
                early.append({**case, 'reference': reference, 'actual': actual, 'passed': bool(passed)})
            else:
                assert reference['ok'] and reference['records'] and reference['value'] == 'string:6f6b'
                expected = case.get('current_spec_records', reference['records'])
                passed = helper.finished_normally(actual) and actual['ok'] and actual['value'] == reference['value'] and actual['records'] == expected
                matches_node = helper.finished_normally(actual) and actual['ok'] and actual['value'] == reference['value'] and actual['records'] == reference['records']
                if 'current_spec_records' in case:
                    assert expected != reference['records']
                results.append({**case, 'reference': reference, 'actual': actual, 'passed': bool(passed), 'matches_node': bool(matches_node)})
        for case in fixture_data['bounds']:
            assert hashlib.sha256(case['source'].encode()).hexdigest() == case['source_sha256']
            actual = helper.run_own(probe, case['source'], browser=case['browser_global'])
            safe_failure = helper.finished_normally(actual) and not actual['ok'] and not actual['records']
            original_passed = safe_failure and case['error_fragment'] in (actual.get('error') or '').lower()
            assertion = bound_assertions.get(case['id'])
            if assertion:
                assert assertion['source_sha256'] == case['source_sha256']
                assert assertion['original_error_fragment'] == case['error_fragment']
            fragment = assertion['adjudicated_error_fragment'] if assertion else case['error_fragment']
            passed = safe_failure and fragment in (actual.get('error') or '').lower()
            bounds.append({**case, 'actual': actual, 'passed': bool(passed), 'original_expected_category_matched': bool(original_passed), 'explicit_assertion': assertion, 'node_run': False})
    stable = source_manifest == {p.relative_to(root).as_posix(): digest(p) for p in files}
    assert inputs == {str(p.relative_to(ROOT)): digest(p) for p in input_paths}, 'Frozen inputs changed'
    report = {'label': options.label, 'node': subprocess.check_output(['node', '--version'], text=True).strip(),
        'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(), 'source_manifest': source_manifest,
        'source_unchanged_during_run': stable, 'inputs': inputs,
        'ordinary_cases': sum('current_spec_records' not in x for x in results),
        'ordinary_matches': sum(x['matches_node'] for x in results if 'current_spec_records' not in x),
        'current_spec_cases': sum('current_spec_records' in x for x in results),
        'current_spec_passed': sum(x['passed'] for x in results if 'current_spec_records' in x),
        'early_cases': len(early), 'early_passed': sum(x['passed'] for x in early),
        'bound_cases': len(bounds), 'bounds_passed': sum(x['passed'] for x in bounds),
        'original_bound_categories_matched': sum(x['original_expected_category_matched'] for x in bounds),
        'results': results, 'early_errors': early, 'bounds': bounds}
    options.output.parent.mkdir(parents=True, exist_ok=True)
    options.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: v for k, v in report.items() if k not in ['source_manifest', 'inputs', 'results', 'early_errors', 'bounds']}, indent=2))
    return 0 if options.allow_gaps or all(x['passed'] for x in results + early + bounds) else 1


if __name__ == '__main__':
    raise SystemExit(main())
