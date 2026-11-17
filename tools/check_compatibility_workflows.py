"""Development-only original JS-enabled compatibility workflows; plan-only by default.

Uses exact external-cache distributions, caller-supplied build provenance, real
App/input and existing owned-process watchdogs. No install, eval grant, polyfill,
consent, public-service request or reference-interaction substitution.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import html
import http.server
import importlib
import json
from pathlib import Path
import re
import sys
import tempfile
import threading
import urllib.parse
import uuid

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tools/fixtures/compatibility'
FIRST, SECOND = 'amber 雪', 'violet λ'
CSP = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; connect-src 'self'; form-action 'self'; base-uri 'none'; object-src 'none'; font-src 'none'"
WORKFLOWS = ('W1', 'W2', 'W3')
STAGES = ('load', 'initialization', 'render', 'input', 'async', 'history')
PINNED_ASSETS = {
    'jquery-1.12.4.js': '430f36f9b5f21aae8cc9dca6a81c4d3d84da5175eaedcf2fdc2c226302cb3575',
    'bootstrap-3.4.1.js': 'dbd2a35e72edc7d6bde483481a912f1c38aa57fab2747d9b071d317339ee03a2',
    'bootstrap-3.4.1.css': 'd170052c16caec3810f2dee6456539045d8e326f6d8ed7c7f78e59ed34de348a',
    'foundation.min.css': 'c500decbe33468e54180f1b6238850d810e85dcdfbea7f87f56bcb17051a51e4',
    'vue.runtime.min.js': 'c403020f02111ef6dde6088c72e4baf584adb5b408a562bef1413e1f53e0b899',
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def safe_relative(name):
    path = Path(name)
    require(not path.is_absolute() and '..' not in path.parts and path.parts and ':' not in name and '\\' not in name, 'Expected a safe relative fixture/cache path.')
    return path


def fixture_identity():
    manifest = json.loads((FIXTURES / 'fixtures.json').read_bytes())
    for name, item in manifest['files'].items():
        path = FIXTURES / safe_relative(name)
        require(path.stat().st_size == item['bytes'] and sha(path.read_bytes()) == item['sha256'], 'Original fixture identity changed: ' + name)
    return {name: item['sha256'] for name, item in manifest['files'].items()}


def inputs(root):
    """Read exact development inputs from an external cache; never download/install."""
    manifest = json.loads((FIXTURES / 'inputs.json').read_bytes())
    require(manifest.get('complete') is True and manifest.get('schema_version') == 1,
        'A complete pinned development-input manifest is required.')
    require(len(manifest['assets']) == len(PINNED_ASSETS)
        and {Path(row['cache_path']).name for row in manifest['assets']} == set(PINNED_ASSETS),
        'The exact five browser distributions are required.')
    payloads = {}
    for row in manifest['assets']:
        path = root / safe_relative(row['cache_path'])
        data = path.read_bytes()
        require(len(data) == row['bytes'] and sha(data) == row['sha256'], 'Pinned development asset changed: ' + row['id'])
        require(sha(data) == PINNED_ASSETS[path.name], 'Unmodified distribution mismatch: ' + row['id'])
        license = row['license']
        legal = (root / safe_relative(license['cache_path'])).read_bytes()
        require(len(legal) == license['bytes'] and sha(legal) == license['sha256'], 'Retain the exact upstream MIT license for ' + row['id'])
        require(license['spdx'] == 'MIT' and (FIXTURES / safe_relative(license['notice'])).read_bytes() == legal,
            'Cached and retained upstream notices must agree.')
        require(row['mime'] in ('text/javascript', 'text/css'), 'Only declared browser JS/CSS inputs are served.')
        payloads['/' + path.name] = (row['mime'], data)
    for path in sorted(FIXTURES.iterdir()):
        if path.suffix in ('.js', '.css'):
            payloads['/' + path.name] = ('text/javascript' if path.suffix == '.js' else 'text/css', path.read_bytes())
    pixels = bytes([255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0]) * 4
    payloads['/control.ppm'] = ('image/x-portable-pixmap', b'P6\n4 4\n255\n' + pixels)
    payloads['/favicon.ico'] = ('image/x-icon', b'')
    return payloads, manifest


class Fixture:
    """One owned IPv4 loopback server; no proxy, redirect, remote or upload routes."""
    def __init__(self, folder, payloads):
        self.folder, self.payloads, self.rows = folder, payloads, []
        self.lock = threading.Lock()
        folder.mkdir()
        fixture = self

        class Handler(http.server.BaseHTTPRequestHandler):
            protocol_version = 'HTTP/1.1'

            def setup(self):
                super().setup()
                self.connection.settimeout(3)

            def do_GET(self):
                with fixture.lock:
                    number = len(fixture.rows) + 1
                    row = {'id': number, 'target': self.path, 'method': 'GET', 'completed': False}
                    fixture.rows.append(row)
                parsed = urllib.parse.urlsplit(self.path)
                query = urllib.parse.parse_qs(parsed.query, keep_blank_values=True).get('q', [''])[0]
                selected = fixture.payloads.get(parsed.path) if not parsed.query else None
                status = 200
                if parsed.path in ('/w1/', '/w1/search', '/w1/detail', '/w2/', '/w2/result', '/w3/'):
                    workflow = parsed.path[1:3]
                    page = (FIXTURES / (workflow + '.html')).read_text(encoding='utf-8')
                    kind = 'results' if parsed.path == '/w1/search' else 'detail' if parsed.path == '/w1/detail' else 'home'
                    content = ('Original result ' + html.escape(query) + ' <a href="/w1/detail?q=' + urllib.parse.quote(query, safe='') + '">Open original result</a>') if kind == 'results' else 'Original ' + kind + ' page'
                    page = page.replace('@@KIND@@', kind).replace('@@QUERY@@', html.escape(query, quote=True)).replace('@@RESPONSE@@', str(number)).replace('@@CONTENT@@', content)
                    selected = ('text/html; charset=utf-8', page.encode('utf-8'))
                if number > 64 or self.headers.get('Host') != '127.0.0.1:' + str(fixture.server.server_port):
                    selected, status = ('text/plain', b'Owned fixture allowance or Host rejected.'), 403
                if selected is None:
                    selected, status = ('text/plain', b'Unknown original fixture route.'), 404
                mime, body = selected
                row.update(status=status, mime=mime, body_bytes=len(body), body_sha256=sha(body), csp=CSP)
                (folder / (str(number).zfill(3) + '.response')).write_bytes(body)
                try:
                    self.send_response(status)
                    for name, value in [('Content-Type', mime), ('Content-Length', str(len(body))), ('Cache-Control', 'no-store'), ('Connection', 'close'), ('Content-Security-Policy', CSP), ('X-Content-Type-Options', 'nosniff')]:
                        self.send_header(name, value)
                    self.end_headers()
                    row['bytes_written'] = self.wfile.write(body)
                    self.wfile.flush()
                    row['completed'] = row['bytes_written'] == len(body)
                except OSError as error:
                    row['error_type'] = type(error).__name__
                self.close_connection = True

            def log_message(self, *_):
                pass

        self.server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.server.daemon_threads = False
        self.server.block_on_close = True
        self.thread = threading.Thread(target=self.server.serve_forever, kwargs={'poll_interval': .1}, daemon=False)
        self.stopped = False

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)
        self.stopped = not self.thread.is_alive()


def script_failures(activity):
    """Exclude only the explicit absence sentinel, retaining every actual fault."""
    return [line for line in activity.splitlines()
        if line.strip() != 'Last escaped engine fault: stage=unknown kind=unknown'
        and re.search(r'(?:JavaScript error at|Uncaught exception:|Last escaped engine fault:)', line)]


def read_stage(folder, label):
    matches = list(folder.glob('*-' + label + '.activity.txt'))
    if len(matches) != 1:
        return {'present': False, 'label': label, 'first_root': None}
    path = matches[0]
    prefix = path.name.removesuffix('.activity.txt')
    activity = path.read_text(encoding='utf-8')
    layout = path.with_name(prefix + '.layout.txt').read_text(encoding='utf-8')
    source = path.with_name(prefix + '.source.html').read_text(encoding='utf-8')
    ppm = path.with_name(prefix + '.ppm').read_bytes()
    def stamp(prefix):
        line = next((line for line in activity.splitlines() if line.startswith(prefix)), '')
        return dict(re.findall(r'(\w+)=([^\s]+)', line))
    document, geometry, canvas = [stamp(value) for value in ('Stage document=', 'Stage layout=', 'Stage canvas=')]
    parts = ppm.split(b'\n', 3)
    valid_pixels = len(parts) == 4 and parts[0] == b'P6' and parts[2] == b'255'
    if valid_pixels:
        dimensions = list(map(int, parts[1].split()))
        valid_pixels = dimensions == [int(canvas.get('width', 0)), int(canvas.get('height', 0))] and len(parts[3]) == dimensions[0] * dimensions[1] * 3
    control = None
    if valid_pixels:
        row = bytes([255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0])
        offset = parts[3].find(row)
        while offset >= 0:
            if offset % 3 == 0:
                x, y = (offset // 3) % dimensions[0], (offset // 3) // dimensions[0]
                if x + 4 <= dimensions[0] and y + 4 <= dimensions[1] and all(parts[3][offset + dy * dimensions[0] * 3:offset + dy * dimensions[0] * 3 + 12] == row for dy in range(4)):
                    control = [x, y, 4, 4]
                    break
            offset = parts[3].find(row, offset + 1)
    paired = (valid_pixels and 'Stage paired_ppm_written=1' in activity
        and document.get('document') not in (None, 'unknown')
        and canvas.get('document') == geometry.get('layout_document') == document.get('document')
        and canvas.get('dom_commit') == geometry.get('layout_dom') == document.get('dom_commit')
        and canvas.get('layout') == geometry.get('layout') not in (None, 'unknown')
        and canvas.get('page_view') == '1')
    summaries = re.findall(r'^(\d+) script\(s\), (\d+) active listener\(s\), (\d+) error\(s\)\.$', activity, re.M)
    errors = script_failures(activity)
    runs = [{'x': int(x), 'y': int(y), 'w': int(w), 'h': int(h), 'text': text}
        for x, y, w, h, text in re.findall(r'^x=(-?\d+) y=(-?\d+) w=(\d+) h=(\d+) text="([^"\n]*)"', layout, re.M)]
    # Saved source is the original response, not live DOM state. The activity
    # header records the actual current title; later diagnostics cannot replace it.
    title = re.fullmatch(r'[^\r\n]+ activity report\nStatus: [^\r\n]*\nPage title: ([^\r\n]*)\nJavaScript enabled: (?:true|false)\nImages enabled: (?:true|false)', activity.split('\n\n', 1)[0])
    return {'present': True, 'label': label, 'paired': paired, 'document': document, 'layout': geometry, 'canvas': canvas,
        'title': title[1] if title else None, 'visible': ' '.join(run['text'] for run in runs), 'runs': runs,
        'zero_errors': len(summaries) == 1 and int(summaries[0][0]) >= 1 and int(summaries[0][2]) == 0,
        'first_root': errors[0] if errors else None, 'later_script_errors': errors[1:],
        'fonts': [line for line in activity.splitlines() if line.startswith('FONTS:')],
        'javascript_enabled': 'JavaScript enabled: true' in activity,
        'original_control_exact_pixels': control,
        'source_sha256': sha(path.with_name(prefix + '.source.html').read_bytes()), 'ppm_sha256': sha(ppm), 'activity_sha256': sha(path.read_bytes())}


def actions(workflow):
    if workflow == 'W1':
        return ['wait\t150', 'snapshot\tloaded', 'fill\tq\t' + FIRST, 'key\tEnter', 'wait\t150', 'snapshot\tfirst-results', 'link-target\t/w1/detail', 'wait\t150', 'snapshot\tdetail', 'back', 'wait\t150', 'snapshot\tback', 'fill\tq\t' + SECOND, 'key\tEnter', 'wait\t150', 'snapshot\tsecond-results']
    if workflow == 'W2':
        return ['wait\t250', 'snapshot\tloaded', 'click\tmenu-toggle', 'snapshot\tmenu-open', 'click\tchoose-amber', 'wait\t150', 'snapshot\tchosen', 'click\tsubmit-legacy', 'wait\t250', 'snapshot\tform-result']
    return ['wait\t250', 'snapshot\tloaded', 'fill\treactive-name\tAda 雪', 'click\tincrement', 'click\tincrement', 'wait\t150', 'snapshot\treactive', 'click\tasync-update', 'wait\t250', 'snapshot\tasync-complete']


def score(workflow, stages, requests, action_lines, width):
    visible = lambda label: stages.get(label, {}).get('visible', '')
    loaded = stages.get('loaded', {})
    all_present = bool(stages) and all(row.get('present') and row.get('paired') and row.get('javascript_enabled') for row in stages.values())
    exact_pages = bool(requests) and all(row['completed'] and row['status'] == 200 for row in requests)
    required = {'/workflow.css', '/control.ppm', '/' + workflow.lower() + '/'} | {
        'W1': {'/w1.js'},
        'W2': {'/jquery-1.12.4.js', '/bootstrap-3.4.1.js', '/bootstrap-3.4.1.css', '/w2.js'},
        'W3': {'/vue.runtime.min.js', '/foundation.min.css', '/w3.js'},
    }[workflow]
    paths = {urllib.parse.urlsplit(row['target']).path for row in requests}
    dependencies = required.issubset(paths)
    result = {name: {'state': 'Not applicable'} for name in STAGES}
    result['load'] = {'state': 'Passed' if exact_pages and dependencies else 'Failed', 'checks': {'all_http_responses_completed': exact_pages, 'all_exact_required_assets_requested': dependencies}}
    checks = {'paired_actual_snapshots': all_present, 'independent_original_control_rendered': loaded.get('original_control_exact_pixels') is not None, 'all_current_scripts_zero_errors': all(row.get('zero_errors') for row in stages.values())}
    if workflow == 'W1':
        checks.update(initial_async_ready='W1 async ready' in visible('loaded'), first_query_visible=FIRST in visible('first-results'), detail_visible='Original detail page' in visible('detail'), exactly_one_back=action_lines.count('back') == 1, back_returns_first_query=stages.get('back', {}).get('title') == 'W1 results ' + FIRST, different_query_visible=SECOND in visible('second-results'))
        queries = [urllib.parse.parse_qs(urllib.parse.urlsplit(row['target']).query).get('q') for row in requests if urllib.parse.urlsplit(row['target']).path == '/w1/search']
        checks['both_actual_unicode_form_requests'] = [FIRST] in queries and [SECOND] in queries
        for label in ('initialization', 'input', 'async', 'history'):
            keys = {'initialization': ['initial_async_ready'], 'input': ['first_query_visible', 'different_query_visible', 'both_actual_unicode_form_requests'], 'async': ['initial_async_ready'], 'history': ['detail_visible', 'exactly_one_back', 'back_returns_first_query']}[label]
            result[label] = {'state': 'Passed' if all(checks[key] for key in keys) else 'Failed', 'checks': {key: checks[key] for key in keys}}
    elif workflow == 'W2':
        checks.update(libraries_initialized='W2 initialized jquery=1.12.4 bootstrap=3.4.1' in visible('loaded'), dropdown_opened='W2 dropdown open' in visible('menu-open'), trusted_choice='W2 trusted choice ' + FIRST in visible('chosen'), async_library_update='W2 asynchronous library text update complete' in visible('chosen'), actual_form_result='W2 server query=' + FIRST in visible('form-result'))
        checks['real_form_get'] = any(urllib.parse.urlsplit(row['target']).path == '/w2/result' and urllib.parse.parse_qs(urllib.parse.urlsplit(row['target']).query).get('q') == [FIRST] for row in requests)
        for name, keys in [('initialization', ['libraries_initialized']), ('input', ['dropdown_opened', 'trusted_choice', 'actual_form_result', 'real_form_get']), ('async', ['async_library_update'])]:
            result[name] = {'state': 'Passed' if all(checks[key] for key in keys) else 'Failed', 'checks': {key: checks[key] for key in keys}}
    else:
        checks.update(runtime_mounted='W3 runtime mounted Vue 2.7.16' in visible('loaded'), real_reactive_input_and_two_clicks='W3 reactive count=2 name=Ada 雪 phase=ready' in visible('reactive'), async_next_tick_completed='W3 reactive count=2 name=Ada 雪 phase=complete' in visible('async-complete') and stages.get('async-complete', {}).get('title') == 'W3 asynchronous Vue update complete')
        positions = {text: next((run for run in loaded.get('runs', []) if run['text'] == text), None) for text in ('FoundationA', 'FoundationB')}
        a, b = positions.values()
        responsive = bool(a and b and (a['y'] == b['y'] and b['x'] > a['x'] if width == 1280 else b['y'] > a['y']))
        result['generated_css'] = {'state': 'Passed' if responsive else 'Failed', 'checks': {'large_two_columns_or_small_stacked': responsive}, 'positions': positions, 'scope': 'Foundation 6.9 CSS only; independent of Vue mount/interaction.'}
        for name, key in [('initialization', 'runtime_mounted'), ('input', 'real_reactive_input_and_two_clicks'), ('async', 'async_next_tick_completed')]:
            result[name] = {'state': 'Passed' if checks[key] else 'Failed', 'checks': {key: checks[key]}}
    initial_render = loaded.get('paired') is True and loaded.get('javascript_enabled') is True
    result['render'] = {'state': 'Passed' if initial_render and checks['independent_original_control_rendered'] else 'Failed', 'checks': {'initial_document_layout_generated_pixels': initial_render, 'independent_original_control_rendered': checks['independent_original_control_rendered']}, 'scope': 'Initial paired generated pixels, visible text and exact original control pixels; later absent input snapshots do not erase this initial observation. No whole-page reference equality claim.'}
    return result, checks


def reference_initial(source, width, folder, capture, binding):
    """Optional reference initial output only; never an interaction substitute."""
    folder.mkdir()
    exe = Path(binding['path'])
    require(sha(exe.read_bytes()) == binding['executable_sha256'], 'Reference browser changed; refresh the explicit local binding first.')
    runs = {}
    for mode in ('dom', 'pixels'):
        profile = folder / ('isolated-profile-' + mode)
        require(not profile.exists(), 'Reference profile must be fresh.')
        args = [str(exe), '--headless=new', '--no-first-run', '--no-default-browser-check', '--disable-background-networking', '--disable-component-update', '--disable-sync', '--metrics-recording-only', '--user-data-dir=' + str(profile.resolve()), '--window-size=' + str(width) + ',800', '--virtual-time-budget=1000']
        args.append('--dump-dom' if mode == 'dom' else '--screenshot=' + str((folder / 'initial.png').resolve()))
        args.append(source)
        runs[mode] = capture.execute(args, folder / (mode + '.log'), folder, 45)
    screenshot = folder / 'initial.png'
    png = screenshot.read_bytes() if screenshot.is_file() else b''
    dom = (folder / 'dom.log').read_text(encoding='utf-8', errors='replace')
    result = {'binding': binding, 'runs': runs, 'initial_dom_observed': '<html' in dom.lower(), 'initial_png_observed': png.startswith(b'\x89PNG\r\n\x1a\n'), 'initial_png_dimensions': [int.from_bytes(png[16:20], 'big'), int.from_bytes(png[20:24], 'big')] if len(png) >= 24 else None, 'reference_interaction': 'Not run', 'scope': 'Two isolated initial-only captures; no claim of a single identical DOM/screenshot instant, full workflow, font equivalence or ChezCat output.', 'browser_after_sha256': sha(exe.read_bytes())}
    result['passed'] = all(row['state'] == 'completed' and row.get('exit_code') == 0 for row in runs.values()) and result['initial_dom_observed'] and result['initial_png_observed'] and result['initial_png_dimensions'] == [width, 800] and result['browser_after_sha256'] == binding['executable_sha256']
    write_json(folder / 'manifest.json', result)
    return result


def run_case(workflow, width, exe, folder, payloads, capture, reference_binding=None):
    folder.mkdir()
    lines = actions(workflow)
    script = folder / 'input.tsv'
    script.write_text('\n'.join(lines) + '\n', encoding='utf-8', newline='\n')
    labels = [line.split('\t')[1] for line in lines if line.startswith('snapshot\t')]
    with Fixture(folder / 'responses', payloads) as fixture:
        source = f'http://127.0.0.1:{fixture.server.server_port}/{workflow.lower()}/'
        with tempfile.TemporaryDirectory(prefix='owned-cwd-', dir=folder) as cwd:
            run = capture.execute([str(exe), source, '--dev-input-script', str(script), '--dev-input-output', str(folder / 'app'), '--javascript', '--images', '--no-profile', '--width', str(width), '--height', '800', '--zoom', '100'], folder / 'command.log', Path(cwd), 120)
        app_end = len(fixture.rows)
        reference = {'state': 'Not run', 'reference_interaction': 'Not run'}
        if reference_binding:
            try:
                reference = reference_initial(source, width, folder / 'reference-initial', capture, reference_binding)
            except (OSError, ValueError, KeyError, TypeError) as error:
                reference = {'state': 'failed', 'passed': False, 'error_type': type(error).__name__,
                    'error': str(error), 'reference_interaction': 'Not run'}
    app_rows = fixture.rows[:app_end]
    stages = {label: read_stage(folder / 'app', label) for label in labels}
    scores, checks = score(workflow, stages, app_rows, lines, width)
    checks.update(process_completed=run['state'] == 'completed' and run.get('exit_code') == 0, server_stopped=fixture.stopped)
    for name, value in scores.items():
        if value['state'] == 'Failed':
            checks[name + '_stage_passed'] = False
    root = next((stage['first_root'] for stage in stages.values() if stage.get('first_root')), None)
    result = {'workflow': workflow, 'viewport': [width, 800], 'zoom': 100, 'javascript_enabled': True, 'profile': 'fresh --no-profile', 'source': source, 'passed': all(checks.values()), 'checks': checks, 'stage_scores': scores, 'first_failed_stage': next((name for name, stage in scores.items() if stage['state'] == 'Failed'), None), 'first_ordered_script_failure': root, 'observed_script_first_root_count': 1 if root else 0, 'later_script_messages_are_not_independent_root_causes': True, 'run': run, 'snapshots': stages, 'requests': app_rows, 'reference_initial': reference, 'reference_requests': fixture.rows[app_end:], 'reference_interaction': 'Not run'}
    write_json(folder / 'manifest.json', result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', action='store_true', help='Explicitly execute the selected finite workflows; otherwise print the plan.')
    parser.add_argument('--inputs', type=Path, help='External development cache matching the public inputs.json; no automatic downloads.')
    parser.add_argument('--exe', type=Path, default=ROOT / 'ChezCat.exe')
    parser.add_argument('--build-record', type=Path, help='Existing build JSON with exact source_commit and executable_sha256; not current-tree inference.')
    parser.add_argument('--workflows', nargs='+', choices=WORKFLOWS, default=list(WORKFLOWS))
    parser.add_argument('--reference-exe', type=Path, help='Optional explicit isolated headless Edge/Chromium initial-only reference.')
    parser.add_argument('--reference-version')
    parser.add_argument('--reference-sha256')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if not args.run:
        print(json.dumps({'workflows': args.workflows, 'execution': 'Not run', 'reference_interaction': 'Not run', 'scope': __doc__}, indent=2))
        return 0
    require(args.inputs is not None and args.build_record is not None, '--run requires an external --inputs directory and actual --build-record.')
    exe, cache = args.exe.resolve(), args.inputs.resolve()
    require(cache.is_dir() and not cache.is_relative_to(ROOT.resolve()) and not cache.is_relative_to(exe.parent), 'Development cache must be outside source and runtime directories.')
    raw_build = args.build_record.read_bytes()
    require(len(raw_build) <= 4 * 1024 * 1024, 'Build record is too large.')
    build = json.loads(raw_build)
    require(re.fullmatch('[0-9a-f]{40}', build['source_commit']) and re.fullmatch('[0-9a-f]{64}', build['executable_sha256']), 'Full source revision and executable SHA-256 are required.')
    require(exe.is_file() and sha(exe.read_bytes()) == build['executable_sha256'], 'Build record does not identify this executable.')
    require(build.get('dirty') is None or type(build['dirty']) is bool, 'Build dirty state must be Boolean or unknown.')
    projected = {name: build[name] for name in ('source_commit', 'executable_sha256', 'dirty', 'cargo_version', 'source_map_sha256') if name in build}
    reference = None
    if any(value is not None for value in (args.reference_exe, args.reference_version, args.reference_sha256)):
        require(args.reference_exe is not None and args.reference_version and args.reference_sha256 is not None and re.fullmatch('[0-9a-f]{64}', args.reference_sha256), 'Reference requires explicit executable, version and SHA-256 together.')
        reference = {'path': str(args.reference_exe.resolve()), 'product_version': args.reference_version, 'executable_sha256': args.reference_sha256}
    sys.path.insert(0, str(ROOT / 'tools'))
    capture, acceptance = importlib.import_module('capture_tests'), importlib.import_module('accept_license')
    acceptance.terms()
    require(acceptance.check(acceptance.directory()), 'Existing caller-provided exact-term receipt required; this driver never accepts.')
    before = fixture_identity()
    payloads, provenance = inputs(cache)
    folder = args.output or (ROOT / 'results/verification/autonomy' / (datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ') + '-compatibility-workflows-' + uuid.uuid4().hex[:8]))
    folder = folder.resolve()
    require(folder.is_relative_to((ROOT / 'results/verification').resolve()) and not folder.exists(), 'Choose a fresh output below this repository results/verification directory.')
    folder.mkdir(parents=True)
    runner_sha = sha(Path(__file__).read_bytes())
    capture_sha = sha(Path(capture.__file__).read_bytes())
    record = {'schema_version': 1, 'build_provenance': projected, 'build_record_sha256': sha(raw_build), 'source_binding_scope': 'Caller-retained build provenance plus exact checked executable digest. Current worktree is not asserted as compiled source.', 'original_fixture_identity_before': before, 'runner_sha256': runner_sha, 'capture_helper_sha256': capture_sha, 'license_accepted_by_driver': False, 'library_inputs': provenance, 'workflow_denominator': 3, 'selected_workflows': args.workflows, 'cases': [], 'reference_initial_dom_and_render': 'Requested, initial-only' if reference else 'Not run', 'reference_interaction': 'Not run', 'driver_targeted_public_service_urls': 0}
    for workflow in args.workflows:
        for width in ((1280, 640) if workflow == 'W3' else (1280,)):
            try:
                case = run_case(workflow, width, exe, folder / (workflow + '-' + str(width)), payloads, capture, reference)
            except (OSError, ValueError, KeyError, TypeError) as error:
                case = {'workflow': workflow, 'viewport': [width, 800], 'passed': False, 'checks': {'case_completed': False}, 'error_type': type(error).__name__, 'error': str(error), 'reference_interaction': 'Not run'}
            record['cases'].append(case)
            write_json(folder / 'manifest.json', record)
    after = fixture_identity()
    _, provenance_after = inputs(cache)
    record['original_fixture_identity_after'] = after
    record['executable_after_sha256'] = sha(exe.read_bytes())
    record['workflow_scores'] = {name: {'state': ('Passed' if all(case['passed'] for case in record['cases'] if case['workflow'] == name) else 'Failed') if name in args.workflows else 'Not run', 'configurations': sum(case['workflow'] == name for case in record['cases'])} for name in WORKFLOWS}
    record['checks'] = {'all_selected_workflows_passed': bool(record['cases']) and all(case['passed'] for case in record['cases']), 'original_fixtures_unchanged': before == after, 'immutable_executable_unchanged': record['executable_after_sha256'] == build['executable_sha256'], 'build_record_unchanged': sha(args.build_record.read_bytes()) == sha(raw_build), 'unmodified_inputs_unchanged': provenance == provenance_after, 'runner_unchanged': sha(Path(__file__).read_bytes()) == runner_sha, 'capture_helper_unchanged': sha(Path(capture.__file__).read_bytes()) == capture_sha}
    record['passed'] = all(record['checks'].values())
    write_json(folder / 'manifest.json', record)
    print(json.dumps({'passed': record['passed'], 'checks': record['checks']}, indent=2))
    print('Evidence: ' + str(folder))
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
