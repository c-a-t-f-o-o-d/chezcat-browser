"""Finite development-only current jQuery/Bootstrap apps; plan-only by default.

Reuse the original loopback fixture, real App/input snapshots and owned-process
watchdog. Never download, install, accept terms, change libraries or drive a desktop.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import sys
import tempfile
import uuid

sys.dont_write_bytecode = True
import accept_license
import capture_tests as capture
import check_compatibility_workflows as existing

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tools/fixtures/compatibility-modern'
WORKFLOWS = ('jquery', 'bootstrap')
NAME = 'amber 雪'
PINS = {
    'jquery': 'f5fb077959ca06faa1dc50761d8bbb836c6c78067932537a2b3fea9e401257c5',
    'bootstrap_js': '56d73efb18652e2682e6daa95196442fe3b50200e283e188f8904e061d823752',
    'bootstrap_css': 'ec743fa098b0dc109177873c05296547234dcff6777c5a7d7324974a1aa4c172',
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_json(path, limit=4 * 1024 * 1024):
    require(path.is_file() and path.stat().st_size <= limit, 'Expected bounded JSON file is unavailable.')
    return json.loads(path.read_bytes())


def fixture_identity():
    manifest = read_json(FIXTURES / 'fixtures.json')
    files = manifest['files']
    require({p.relative_to(FIXTURES).as_posix() for p in FIXTURES.rglob('*') if p.is_file()}
            == set(files) | {'fixtures.json'}, 'Unexpected or missing original fixture file.')
    for name, item in files.items():
        path = capture.checked_output_path(FIXTURES / existing.safe_relative(name))
        require(path.is_relative_to(FIXTURES), 'Fixture path escaped its directory.')
        require(path.is_file() and path.stat().st_size == item['bytes'] <= 2 * 1024 * 1024, 'Fixture size mismatch: ' + name)
        data = path.read_bytes()
        require(len(data) == item['bytes'] and sha(data) == item['sha256'], 'Original fixture changed: ' + name)
    return {name: item['sha256'] for name, item in files.items()}


def inputs(cache):
    manifest = read_json(FIXTURES / 'inputs.json')
    require(manifest['schema_version'] == 1 and manifest['versions'] == {
        'jquery': '4.0.0', 'bootstrap': '5.3.9', 'popper': '2.11.8'}, 'Expected exact current cohort.')
    require(set(manifest['assets']) == set(PINS) and set(manifest['licenses']) == {'jquery', 'bootstrap', 'popper'},
            'Expected three assets and their three MIT notices.')
    payloads = {}
    for name, item in manifest['licenses'].items():
        path = capture.checked_output_path(cache / existing.safe_relative(item['cache_path']))
        require(path.is_relative_to(cache), 'License cache path escaped its directory.')
        require(path.is_file() and path.stat().st_size == item['bytes'] <= 16 * 1024, 'License size mismatch: ' + name)
        data = path.read_bytes()
        require(item['spdx'] == 'MIT' and len(data) == item['bytes'] and sha(data) == item['sha256']
                and data == (FIXTURES / existing.safe_relative(item['notice'])).read_bytes(),
                'Retain the exact upstream MIT notice: ' + name)
    for name, item in manifest['assets'].items():
        path = capture.checked_output_path(cache / existing.safe_relative(item['cache_path']))
        require(path.is_relative_to(cache), 'Asset cache path escaped its directory.')
        require(path.is_file() and path.stat().st_size == item['bytes'] <= 2 * 1024 * 1024, 'Distribution size mismatch: ' + name)
        data = path.read_bytes()
        require(len(data) == item['bytes'] and sha(data) == item['sha256'] == PINS[name],
                'Unmodified exact distribution mismatch: ' + name)
        require(item['mime'] in ('text/javascript', 'text/css') and item['licenses']
                and all(value in manifest['licenses'] for value in item['licenses']), 'Asset MIME/license mismatch.')
        require(item['route'] == '/' + path.name, 'Expected one exact local distribution route.')
        payloads[item['route']] = (item['mime'], data)
    for name in ('jquery', 'bootstrap'):
        payloads['/' + name + '/'] = ('text/html; charset=utf-8', (FIXTURES / (name + '.html')).read_bytes())
        payloads['/' + name + '-app.js'] = ('text/javascript', (FIXTURES / (name + '-app.js')).read_bytes())
    payloads['/modern.css'] = ('text/css', (FIXTURES / 'modern.css').read_bytes())
    payloads['/control.ppm'] = ('image/x-portable-pixmap', b'P6\n4 4\n255\n' + bytes(
        [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0]) * 4)
    payloads['/favicon.ico'] = ('image/x-icon', b'')
    return payloads, manifest


def actions(workflow):
    if workflow == 'jquery':
        return ['wait\t250', 'snapshot\tloaded', 'fill\tmodern-name\t' + NAME,
                'click\tjquery-apply', 'wait\t150', 'snapshot\tupdated']
    return ['wait\t250', 'snapshot\tloaded', 'click\tmodern-menu-toggle',
            'wait\t150', 'snapshot\topened', 'click\tmodern-menu-choice',
            'wait\t150', 'snapshot\tchosen']


def observed(passed, scope):
    return {'state': 'Passed' if passed else 'Failed', 'scope': scope}


def control_pixels_observed(stage):
    """Accept only the exact rectangle contract returned by the PPM inspector."""
    rectangle = stage.get('original_control_exact_pixels')
    if (stage.get('present') is not True or 'snapshot_error' in stage
            or type(rectangle) is not list or len(rectangle) != 4
            or any(type(value) is not int for value in rectangle)):
        return False
    try:
        dimensions = [stage['canvas'][name] for name in ('width', 'height')]
        if any(type(value) not in (int, str) or not re.fullmatch(r'[0-9]{1,9}', str(value)) for value in dimensions):
            return False
        width, height = map(int, dimensions)
    except (KeyError, TypeError, ValueError):
        return False
    x, y, w, h = rectangle
    return x >= 0 and y >= 0 and w == h == 4 and x + w <= width and y + h <= height


def score(workflow, width, stages, requests):
    loaded = stages.get('loaded', {})
    visible = lambda label: stages.get(label, {}).get('visible', '')
    required = {'/' + workflow + '/', '/' + workflow + '-app.js', '/modern.css', '/control.ppm'}
    required |= {'/jquery-4.0.0.js'} if workflow == 'jquery' else {'/bootstrap.bundle.js', '/bootstrap.css'}
    paths = {row['target'] for row in requests}
    loaded_ok = bool(requests) and required <= paths and all(row['completed'] and row['status'] == 200 for row in requests)
    version = '4.0.0' if workflow == 'jquery' else '5.3.9'
    marker = ('jQuery' if workflow == 'jquery' else 'Bootstrap') + ' initialized version=' + version
    initialized = marker in visible('loaded') and loaded.get('zero_errors') is True
    syntax_errors = [line for row in stages.values() for line in [row.get('first_root'), *row.get('later_script_errors', [])]
                     if line and re.search(r'\bstage=(?:parse|parser|lex|lexer|tokenize)\b', line)]
    parse = {'state': 'Failed' if syntax_errors else 'Passed' if initialized else 'Not established',
             'scope': 'Explicit parser fault, or library/app reached the exact initialization marker; dormant paths are unassessed.',
             'explicit_parse_failures': syntax_errors}
    paired = loaded.get('paired') is True and loaded.get('javascript_enabled') is True
    rendered = paired and control_pixels_observed(loaded)
    if workflow == 'jquery':
        interacted = initialized and 'jQuery trusted update ' + NAME in visible('updated')
        asynchronous = 'jQuery asynchronous update complete' in visible('updated')
    else:
        interacted = (initialized and 'Bootstrap dropdown open' in visible('opened')
                      and 'Bootstrap trusted choice amber' in visible('chosen')
                      and 'Bootstrap dropdown closed' in visible('chosen'))
        asynchronous = None
    stages_ok = all(row.get('present') and row.get('paired') and row.get('javascript_enabled')
                    and row.get('zero_errors') for row in stages.values())
    results = {
        'load': observed(loaded_ok, 'All exact required local routes requested and every observed response completed HTTP 200.'),
        'parse': parse,
        'initialization': observed(initialized, 'Exact library version marker reached with zero startup script errors.'),
        'render': observed(rendered, 'JS-enabled initial paired document/layout/pixels plus independent original control; no reference equality claim.'),
        'input': observed(interacted and stages_ok, 'Actual App trusted input changes the original visible state through the unmodified library.'),
        'async': observed(asynchronous and stages_ok, 'Original timer callback updates text through jQuery; no AJAX/Deferred claim.')
            if asynchronous is not None else {'state': 'Not applicable', 'scope': 'No asynchronous workflow selected.'},
    }
    if workflow == 'bootstrap':
        positions = {text: next((run for run in loaded.get('runs', []) if run['text'] == text), None)
                     for text in ('BootstrapA', 'BootstrapB')}
        a, b = positions.values()
        responsive = bool(a and b and (a['y'] == b['y'] and b['x'] > a['x'] if width == 1280 else b['y'] > a['y']))
        results['framework_layout'] = dict(observed(responsive, 'Unmodified Bootstrap CSS: two original cards beside each other at1280 and stacked at640.'), positions=positions)
    return results


def run_case(workflow, width, exe, folder, payloads):
    folder.mkdir()
    lines = actions(workflow)
    script = folder / 'input.tsv'
    script.write_text('\n'.join(lines) + '\n', encoding='utf-8', newline='\n')
    labels = [line.split('\t')[1] for line in lines if line.startswith('snapshot\t')]
    with existing.Fixture(folder / 'responses', payloads) as fixture:
        source = f'http://127.0.0.1:{fixture.server.server_port}/{workflow}/'
        with tempfile.TemporaryDirectory(prefix='owned-cwd-', dir=folder) as cwd:
            run = capture.execute([str(exe), source, '--dev-input-script', str(script), '--dev-input-output', str(folder / 'app'),
                                   '--javascript', '--images', '--no-profile', '--width', str(width), '--height', '800', '--zoom', '100'],
                                  folder / 'command.log', Path(cwd), 120)
    stages = {}
    for label in labels:
        try:
            stages[label] = existing.read_stage(folder / 'app', label)
        except (OSError, ValueError, KeyError) as error:
            stages[label] = {'present': False, 'label': label, 'first_root': None, 'snapshot_error': str(error)}
    results = score(workflow, width, stages, fixture.rows)
    root = next((row['first_root'] for row in stages.values() if row.get('first_root')), None)
    checks = {'process_completed': run['state'] == 'completed' and run.get('exit_code') == 0,
              'fixture_stopped': fixture.stopped,
              'selected_stages_passed': all(row['state'] in ('Passed', 'Not applicable') for row in results.values())}
    record = {'workflow': workflow, 'viewport': [width, 800], 'zoom': 100, 'javascript_enabled': True,
              'profile': '--no-profile', 'passed': all(checks.values()), 'checks': checks, 'stage_scores': results,
              'first_failed_stage': next((name for name, row in results.items() if row['state'] == 'Failed'), None),
              'first_ordered_script_failure': root, 'later_messages_are_not_independent_causes': True,
              'run': run, 'snapshots': stages, 'requests': fixture.rows, 'reference_browser': 'Not run'}
    existing.write_json(folder / 'manifest.json', record)
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--run', action='store_true')
    mode.add_argument('--check-inputs', action='store_true', help='Hash checks only; no executable or server.')
    parser.add_argument('--inputs', type=Path, help='Existing pinned external modern cache; never downloaded here.')
    parser.add_argument('--exe', type=Path, default=ROOT / 'ChezCat.exe')
    parser.add_argument('--build-record', type=Path, help='Existing JSON proving actual source_commit and executable_sha256.')
    parser.add_argument('--workflows', nargs='+', choices=WORKFLOWS, default=list(WORKFLOWS))
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if not args.run and not args.check_inputs:
        print(json.dumps({'execution': 'Not run', 'workflows': args.workflows, 'versions': read_json(FIXTURES / 'inputs.json')['versions']}, indent=2))
        return 0
    require(args.inputs is not None, 'Select the existing external --inputs cache.')
    cache = capture.checked_output_path(args.inputs)
    exe = args.exe.resolve()
    require(cache.is_dir() and not cache.is_relative_to(ROOT) and not ROOT.is_relative_to(cache)
            and not cache.is_relative_to(exe.parent), 'Development inputs must remain outside source/runtime directories.')
    before = fixture_identity()
    payloads, provenance = inputs(cache)
    if args.check_inputs:
        print(json.dumps({'passed': True, 'scope': 'Exact asset/notice/original-fixture hashes only; no browser execution',
                          'assets': len(provenance['assets']), 'licenses': len(provenance['licenses']), 'fixtures': len(before)}, indent=2))
        return 0
    require(args.build_record is not None, '--run requires the actual --build-record.')
    require(args.build_record.is_file() and args.build_record.stat().st_size <= 4 * 1024 * 1024, 'Build record unavailable or too large.')
    raw_build = args.build_record.read_bytes()
    build = json.loads(raw_build)
    require(re.fullmatch('[0-9a-f]{40}', build['source_commit']) and re.fullmatch('[0-9a-f]{64}', build['executable_sha256']), 'Exact source/executable identity is required.')
    require(exe.is_file() and sha(exe.read_bytes()) == build['executable_sha256'], 'Actual executable does not match build record.')
    accept_license.terms()
    require(accept_license.check(accept_license.directory()), 'Existing caller-selected exact-term acceptance is required; never created here.')
    storage = capture.verification_root(ROOT)
    folder = capture.checked_output_path(args.output if args.output else storage / (
        datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ') + '-modern-compatibility-' + uuid.uuid4().hex[:8]))
    require(folder != storage and folder.is_relative_to(storage) and not folder.exists(), 'Choose fresh output below selected verification storage.')
    folder.mkdir(parents=True)
    helpers = [Path(__file__), Path(existing.__file__), Path(capture.__file__)]
    helper_before = {path.name: sha(path.read_bytes()) for path in helpers}
    record = {'schema_version': 1, 'build_provenance': {key: build[key] for key in (
        'source_commit', 'executable_sha256', 'dirty', 'cargo_version', 'source_map_sha256') if key in build},
        'source_binding_scope': 'Caller-retained actual build record plus exact executable digest; current dirty worktree is not relabeled compiled source.',
        'build_record_sha256': sha(raw_build), 'fixture_identity_before': before, 'helpers_before': helper_before,
        'library_inputs': provenance, 'workflow_denominator': 2, 'selected_workflows': args.workflows,
        'output_configuration': capture.output_configuration(ROOT), 'cases': [], 'reference_browser': 'Not run', 'license_accepted_by_driver': False}
    for workflow in args.workflows:
        for width in ((1280, 640) if workflow == 'bootstrap' else (1280,)):
            try:
                case = run_case(workflow, width, exe, folder / (workflow + '-' + str(width)), payloads)
            except (OSError, ValueError, KeyError, TypeError) as error:
                case = {'workflow': workflow, 'viewport': [width, 800], 'passed': False, 'error_type': type(error).__name__, 'error': str(error)}
            record['cases'].append(case)
            existing.write_json(folder / 'manifest.json', record)
    record['checks'] = {'selected_workflows_passed': bool(record['cases']) and all(case['passed'] for case in record['cases']),
        'original_fixtures_unchanged': before == fixture_identity(), 'inputs_unchanged': provenance == inputs(cache)[1],
        'executable_unchanged': sha(exe.read_bytes()) == build['executable_sha256'], 'build_record_unchanged': sha(args.build_record.read_bytes()) == sha(raw_build),
        'helpers_unchanged': helper_before == {path.name: sha(path.read_bytes()) for path in helpers}}
    record['passed'] = all(record['checks'].values())
    existing.write_json(folder / 'manifest.json', record)
    print(json.dumps({'passed': record['passed'], 'checks': record['checks'], 'evidence': str(folder)}, indent=2))
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
