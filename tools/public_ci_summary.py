"""Project only bounded, public-safe CI states/counts and public test basenames."""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
MAX_JSON = 4 * 1024 * 1024
MAX_ROWS = 4096
MAX_COUNT = 10_000_000
SCOPE = 'Normal offline verification only; no live/device/security certification claim.'
NAME_SCOPE = ('Only test function basenames declared beside #[test] in committed public Rust source; '
              'module prefixes are never published, and duplicate basenames may be ambiguous.')
STATES = {'running', 'completed', 'failed', 'timeout', 'launch_error', 'interrupted', 'output_limit'}
NATIVE_STATES = {'running', 'passed', 'not_passed', 'interrupted', 'error'}
STAGES = {'license-acceptance', 'publication-boundary', 'publication-controls', 'artifact-guards',
          'version-parser-controls', 'capture-controls', 'normal', 'release', 'pe'}
RUN_NAME = r'\d{8}T\d{6}\.\d{6}Z-[0-9a-f]{8}'
IDENTIFIER = re.compile(r'[A-Za-z_][A-Za-z_0-9]*(?:::[A-Za-z_][A-Za-z_0-9]*)*\Z')


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('Duplicate JSON key')
        result[key] = value
    return result


def read_json(path, root):
    path = path.resolve()
    if not path.is_relative_to(root.resolve()) or not path.is_file() or path.stat().st_size > MAX_JSON:
        return None
    try:
        with path.open('rb') as stream:
            data = stream.read(MAX_JSON + 1)
        if len(data) > MAX_JSON:
            return None
        value = json.loads(data, object_pairs_hook=unique_object)
        return value if isinstance(value, dict) else None
    except (OSError, ValueError, TypeError, UnicodeError, RecursionError):
        return None


def integer(value):
    return type(value) is int and 0 <= value <= MAX_COUNT


def utc(value):
    if not isinstance(value, str) or len(value) > 40:
        return None
    try:
        result = datetime.fromisoformat(value.replace('Z', '+00:00'))
        return result if result.tzinfo is not None and result.utcoffset().total_seconds() == 0 else None
    except (ValueError, OverflowError):
        return None


def public_basenames(root):
    # Read committed public source, never run a path obtained from diagnostics.
    # Conservative adjacency omits unrecognized declarations rather than widening
    # the output namespace. Context separators prevent matches across files.
    result = subprocess.run(['git', 'grep', '--no-color', '-h', '-A', '3', '-F', '#[test]',
                             'HEAD', '--', 'src'], cwd=root, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, timeout=30, check=False)
    if result.returncode not in (0, 1) or len(result.stdout) > 16 * 1024 * 1024:
        return set()
    pattern = r'(?m)^[ \t]*#\[test\](?:[ \t]*\r?\n)?(?:[ \t]*#\[(?:ignore|should_panic)(?:[^\r\n]*)\][ \t]*\r?\n)*[ \t]*(?:async[ \t]+)?fn[ \t]+([A-Za-z_][A-Za-z_0-9]*)[ \t]*\('
    return set(re.findall(pattern, result.stdout.decode('utf-8', errors='replace')))


def project_step(row, known):
    state = row.get('state')
    projected = {'state': state if isinstance(state, str) and state in STATES else 'unavailable',
                 'failed_test_basenames': [], 'failed_identifiers_complete': False}
    analysis = row.get('analysis')
    if not isinstance(analysis, dict):
        return projected
    if integer(analysis.get('announced_tests')):
        projected['announced_tests'] = analysis['announced_tests']
    summary = analysis.get('summary')
    keys = ['passed', 'failed', 'ignored', 'measured', 'filtered']
    if isinstance(summary, dict) and summary.get('state') in ('ok', 'FAILED') and all(integer(summary.get(key)) for key in keys):
        projected['summary'] = {'state': summary['state'], **{key: summary[key] for key in keys}}
    failed = analysis.get('failed_test_names')
    if isinstance(failed, list) and len(failed) <= MAX_ROWS:
        safe = set()
        rejected = 0
        for name in failed:
            if isinstance(name, str) and len(name) <= 512 and IDENTIFIER.fullmatch(name) and name.rsplit('::', 1)[-1] in known:
                safe.add(name.rsplit('::', 1)[-1])
            else:
                rejected += 1
        projected['failed_test_basenames'] = sorted(safe)
        expected = projected.get('summary', {}).get('failed')
        projected['failed_identifiers_complete'] = rejected == 0 and integer(expected) and len(failed) == expected
        projected['excluded_identifier_count'] = rejected
    return projected


def native_projection(manifest, known):
    result = {'state': 'unavailable', 'normal_test_gate_passed': False, 'identifier_scope': NAME_SCOPE}
    if not isinstance(manifest, dict) or type(manifest.get('schema_version')) is not int or manifest['schema_version'] != 1:
        return result
    if isinstance(manifest.get('state'), str) and manifest['state'] in NATIVE_STATES:
        result['state'] = manifest['state']
    steps = manifest.get('steps')
    if not isinstance(steps, list) or len(steps) > 8:
        return result
    for label in ['normal', 'serial']:
        matching = [row for row in steps if isinstance(row, dict) and row.get('name') == label]
        if len(matching) == 1:
            result[label] = project_step(matching[0], known)
    normal = result.get('normal', {})
    counts = normal.get('summary', {})
    result['normal_test_gate_passed'] = (manifest.get('normal_test_gate_passed') is True
        and manifest.get('source_unchanged') is True and normal.get('state') == 'completed'
        and counts.get('state') == 'ok' and counts.get('passed', 0) > 0
        and all(counts.get(key) == 0 for key in ['failed', 'ignored', 'measured', 'filtered']))
    return result


def find_native(root, report):
    runs = report.get('runs')
    if not isinstance(runs, list) or len(runs) > MAX_ROWS:
        return None
    matching = [row for row in runs if isinstance(row, dict) and row.get('name') == 'normal']
    if len(matching) != 1:
        return None
    start, finish = utc(matching[0].get('started_utc')), utc(matching[0].get('finished_utc'))
    source = report.get('source_before')
    if start is None or finish is None or start > finish or not isinstance(source, dict):
        return None
    folder = root / 'results/verification/windows-tests'
    try:
        candidates = [p for p in folder.iterdir() if re.fullmatch(RUN_NAME, p.name)]
    except OSError:
        return None
    if len(candidates) > 256:
        return None
    matches = []
    for candidate in candidates:
        manifest = read_json(candidate / 'manifest.json', root)
        if manifest is None:
            continue
        began, ended = utc(manifest.get('started_utc')), utc(manifest.get('finished_utc'))
        if began is not None and ended is not None and start <= began <= ended <= finish and manifest.get('source_before') == source:
            matches.append(manifest)
    return matches[0] if len(matches) == 1 else None


def summary(root=ROOT):
    result = {'schema_version': 1, 'passed': False, 'scope': SCOPE, 'source_commit': None,
              'executable_sha256': None, 'passing_journeys': 0, 'stages': [],
              'native_tests': native_projection(None, set())}
    try:
        commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root,
                                         stderr=subprocess.DEVNULL, timeout=15).decode('ascii').strip()
        if re.fullmatch('[0-9a-f]{40}', commit):
            result['source_commit'] = commit
        pointer = read_json(root / 'results/verification/latest-offline.json', root)
        location = pointer.get('report') if isinstance(pointer, dict) else None
        if not isinstance(location, str) or not re.fullmatch('results/verification/offline/' + RUN_NAME + '/verification.json', location.replace('\\', '/')):
            return result
        report = read_json(root.joinpath(*location.replace('\\', '/').split('/')), root)
        if report is None or result['source_commit'] is None or report.get('source_commit') != result['source_commit']:
            return result
        result['passed'] = report.get('passed') is True and report.get('source_commit') == result['source_commit'] and result['source_commit'] is not None
        sha = report.get('executable_sha256')
        if isinstance(sha, str) and re.fullmatch('[0-9a-f]{64}', sha):
            result['executable_sha256'] = sha
        journeys = report.get('journeys')
        if isinstance(journeys, list) and len(journeys) <= MAX_ROWS:
            result['passing_journeys'] = sum(isinstance(row, dict) and row.get('valid') is True for row in journeys)
        runs = report.get('runs')
        if isinstance(runs, list) and len(runs) <= MAX_ROWS:
            for row in runs:
                if not isinstance(row, dict):
                    continue
                name, state = row.get('name'), row.get('state')
                if isinstance(name, str) and (name in STAGES or re.fullmatch(r'journey-[0-9]{1,3}', name)) and isinstance(state, str) and state in STATES:
                    result['stages'].append({'name': name, 'state': state})
        result['native_tests'] = native_projection(find_native(root, report), public_basenames(root))
        # A serial success or a malformed/unlinked native record cannot turn
        # the original normal gate into a public passing result.
        result['passed'] = result['passed'] and result['native_tests']['normal_test_gate_passed']
    except (OSError, ValueError, TypeError, subprocess.SubprocessError, UnicodeError, RecursionError):
        pass
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'results/verification/public-summary.json')
    parser.add_argument('--print', action='store_true', dest='print_summary')
    args = parser.parse_args(argv)
    output = args.output.resolve()
    if not output.is_relative_to((ROOT / 'results/verification').resolve()) or output.suffix != '.json':
        parser.error('Output must be verification JSON inside this checkout.')
    safe = summary()
    encoded = (json.dumps(safe, indent=2) + '\n').encode('utf-8')
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded)
    if args.print_summary:
        print(encoded.decode('utf-8'), end='')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
