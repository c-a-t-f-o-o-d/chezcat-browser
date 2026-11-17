"""Original offline App/input navigation and bridge-history journey.

Six original loopback cases check startup assign, explicit replace, HTTP302,
completed-load button activation, load-handler assign and its microtask. Each
uses one Back action followed by a different Unicode form query. Evidence uses
real titles, served response IDs, history.length and paired document/layout/pixel
stamps; raw session-entry vectors/indices are not exposed by the activity report.
No public-service, native-desktop or full-conformance claim. This driver neither
builds the executable nor records license acceptance.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import html
import http.server
import json
from pathlib import Path
import re
import tempfile
import threading
import time
import urllib.parse
import uuid

import accept_license
import capture_tests

ROOT = Path(__file__).resolve().parents[1]
CASES = ('startup', 'replace', 'http', 'postload', 'onload', 'onload-microtask')
FIRST = 'alpha'
SECOND = 'beta + café /?&#'


def require(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def stage_fields(activity, prefix):
    line = next((row for row in activity.splitlines() if row.startswith(prefix)), '')
    return dict(re.findall(r'(\w+)=([^\s]+)', line))


def visible_text(layout):
    return ' '.join(re.findall(r'text="([^"\n]*)"', layout))


def read_stage(evidence, label, capture):
    matches = sorted(evidence.glob('*-' + label + '.activity.txt'))
    if len(matches) != 1:
        return {'present': False, 'label': label, 'matching_snapshots': len(matches)}
    path = matches[0]
    activity = path.read_text(encoding='utf-8')
    prefix = path.name.removesuffix('.activity.txt')
    layout_path = path.with_name(prefix + '.layout.txt')
    source_path = path.with_name(prefix + '.source.html')
    ppm = path.with_name(prefix + '.ppm')
    layout = layout_path.read_text(encoding='utf-8') if layout_path.is_file() else ''
    source = source_path.read_text(encoding='utf-8') if source_path.is_file() else ''
    match = re.search(r'<title>(.*?)</title>', source, re.S | re.I)
    rendered = visible_text(layout)
    fields = dict(re.findall(r'\b(response_id|history_length|phase|ready)=([A-Za-z0-9_-]+)', rendered))
    document = stage_fields(activity, 'Stage document=')
    geometry = stage_fields(activity, 'Stage layout=')
    canvas = stage_fields(activity, 'Stage canvas=')
    hit = stage_fields(activity, 'Stage last_link_hit ')
    paired = ('Stage paired_ppm_written=1' in activity and layout_path.is_file() and source_path.is_file()
        and ppm.is_file() and ppm.stat().st_size > 32
        and document.get('document', 'unknown') != 'unknown'
        and canvas.get('document') == document.get('document') == geometry.get('layout_document')
        and canvas.get('dom_commit') == document.get('dom_commit')
        and canvas.get('layout') == geometry.get('layout') != 'unknown'
        and canvas.get('layout_dom') == geometry.get('layout_dom') and canvas.get('page_view') == '1')
    return {'present': True, 'label': label, 'activity_file': path.name,
        'activity_sha256': capture.sha256(path), 'layout_sha256': capture.sha256(layout_path) if layout_path.is_file() else None,
        'source_sha256': capture.sha256(source_path) if source_path.is_file() else None,
        'ppm_sha256': capture.sha256(ppm) if ppm.is_file() else None, 'title': html.unescape(match[1]) if match else None,
        'visible_text': rendered, 'fixture_state': fields, 'document': document,
        'layout': geometry, 'canvas': canvas, 'last_link_hit': hit,
        'paired_generated_pixels': paired, 'zero_script_errors': '0 error(s).' in activity}


def page(case, kind, response_id, query=''):
    title = 'F2 ' + kind + (' ' + query if query else '')
    body = ('<!doctype html><html><head><meta charset="utf-8"><title>' + html.escape(title)
        + '</title><style>body{margin:24px;font-size:18px}button,input{font-size:18px}</style></head><body>'
        + '<h1>' + html.escape(title) + '</h1><p id="probe-state">Probe pending</p>'
        + '<form action="/search"><input name="q" value="' + html.escape(query, quote=True)
        + '"><input type="hidden" name="case" value="' + case
        + '"><button name="send">Search</button></form>')
    if kind == 'results':
        target = '/hop' if case == 'http' else '/bridge'
        body += '<p><a href="' + target + '?case=' + case + '">Original F2 bridge result</a></p>'
    if kind == 'bridge' and case == 'postload':
        body += '<p><button type="button" id="continue" name="continue">Continue after load</button></p>'
    body += '<p>Original local fixture. Response number ' + str(response_id) + '.</p><script>'
    body += ("function stamp(phase){document.getElementById('probe-state').textContent="
        + json.dumps('response_id=' + str(response_id) + ' history_length=')
        + "+history.length+' phase='+phase+' ready='+document.readyState;}"
        + "stamp('startup');window.addEventListener('load',function(){stamp('load');});")
    destination = '/destination?case=' + case + '&issuer='
    if kind == 'bridge':
        if case == 'startup':
            body += 'location.assign(' + json.dumps(destination) + '+document.readyState);'
        elif case == 'replace':
            body += 'location.replace(' + json.dumps(destination) + '+document.readyState);'
        elif case == 'onload':
            body += "window.addEventListener('load',function(){stamp('onload-assign');location.assign(" + json.dumps(destination) + '+document.readyState);});'
        elif case == 'onload-microtask':
            body += ("window.addEventListener('load',function(){stamp('onload-queued');queueMicrotask(function(){stamp('load-microtask');location.assign("
                + json.dumps(destination) + '+document.readyState);});});')
        elif case == 'postload':
            body += ("document.getElementById('continue').onclick=function(e){stamp('postload-click');location.assign("
                + json.dumps(destination) + "+document.readyState+'&trusted='+(e.isTrusted?'1':'0'));};")
    return (body + '</script></body></html>').encode('utf-8')


class Fixture:
    def __init__(self, case, folder):
        self.case = case
        self.folder = folder
        self.lock = threading.Lock()
        self.events = []
        self.requests = []
        self.responses = []
        self.folder.mkdir()
        fixture = self

        class Handler(http.server.BaseHTTPRequestHandler):
            protocol_version = 'HTTP/1.1'

            def do_GET(self):
                with fixture.lock:
                    response_id = len(fixture.requests) + 1
                    request = {'id': response_id, 'method': 'GET', 'target': self.path,
                        'monotonic_ns': time.monotonic_ns(), 'referer': self.headers.get('Referer'),
                        'origin': self.headers.get('Origin')}
                    fixture.requests.append(request)
                    fixture.events.append({'order': len(fixture.events) + 1, 'kind': 'request', **request})
                parsed = urllib.parse.urlsplit(self.path)
                params = urllib.parse.parse_qs(parsed.query, keep_blank_values=True)
                status, location, kind = 200, None, 'unknown'
                if response_id > 64:
                    status, body = 503, b'<title>Fixture allowance reached</title><p>Local fixture stopped.</p>'
                elif parsed.path == '/':
                    kind = 'home'
                    body = page(fixture.case, kind, response_id)
                elif parsed.path == '/search' and params.get('case') == [fixture.case]:
                    kind = 'results'
                    body = page(fixture.case, kind, response_id, params.get('q', [''])[0])
                elif parsed.path == '/bridge' and params.get('case') == [fixture.case] and fixture.case != 'http':
                    kind = 'bridge'
                    body = page(fixture.case, kind, response_id)
                elif parsed.path == '/hop' and fixture.case == 'http' and params.get('case') == ['http']:
                    status, location, kind, body = 302, '/destination?case=http&issuer=http302', 'http-hop', b''
                elif parsed.path == '/destination' and params.get('case') == [fixture.case]:
                    kind = 'destination'
                    body = page(fixture.case, kind, response_id)
                else:
                    status, body = 404, b'<title>Unknown original fixture request</title>'
                relative = 'response-' + str(response_id).zfill(3) + '-' + kind + '.html'
                (fixture.folder / relative).write_bytes(body)
                response = {'id': response_id, 'target': self.path, 'status': status, 'location': location,
                    'fixture_kind': kind, 'body_bytes': len(body), 'body_sha256': digest(body), 'body_file': relative}
                self.send_response(status)
                self.send_header('Content-Type', 'text/html; charset=utf-8')
                self.send_header('Content-Length', str(len(body)))
                self.send_header('Cache-Control', 'no-store')
                self.send_header('Connection', 'close')
                if location:
                    self.send_header('Location', location)
                self.end_headers()
                try:
                    self.wfile.write(body)
                    self.wfile.flush()
                    response['body_written'] = True
                except (BrokenPipeError, ConnectionResetError):
                    response['body_written'] = False
                response['monotonic_ns'] = time.monotonic_ns()
                with fixture.lock:
                    fixture.responses.append(response)
                    fixture.events.append({'order': len(fixture.events) + 1, 'kind': 'response', **response})
                self.close_connection = True

            def log_message(self, *_):
                pass

        self.server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.server.daemon_threads = True
        self.server.block_on_close = False
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=3)


def run_case(case, exe, folder, capture):
    folder.mkdir()
    action_lines = ['fill\tq\t' + FIRST, 'key\tEnter', 'snapshot\tfirst-results',
        'link-target\t' + ('/hop' if case == 'http' else '/bridge')]
    if case == 'postload':
        action_lines += ['snapshot\tbridge-ready', 'click\tcontinue']
    action_lines += ['snapshot\tdestination', 'back', 'snapshot\tback',
        'fill\tq\t' + SECOND, 'key\tEnter', 'snapshot\tsecond-results']
    script = folder / 'input.tsv'
    script.write_text('\n'.join(action_lines) + '\n', encoding='utf-8', newline='\n')
    evidence = folder / 'journey'
    before = capture.source_identity(ROOT)
    exe_before = capture.sha256(exe)
    with Fixture(case, folder / 'served-originals') as fixture:
        with tempfile.TemporaryDirectory(prefix='isolated-cwd-', dir=folder) as cwd:
            command = [str(exe), 'http://127.0.0.1:' + str(fixture.server.server_port) + '/?case=' + case,
                '--dev-input-script', str(script), '--dev-input-output', str(evidence), '--javascript', '--no-profile']
            run = capture.execute(command, folder / 'command.log', Path(cwd), 60)
    stages = {label: read_stage(evidence, label, capture)
              for label in ('first-results', 'destination', 'back', 'second-results')}
    if case == 'postload':
        stages['bridge-ready'] = read_stage(evidence, 'bridge-ready', capture)
    requests = fixture.requests
    responses = fixture.responses
    paths = [urllib.parse.urlsplit(row['target']).path for row in requests]
    queries = [urllib.parse.parse_qs(urllib.parse.urlsplit(row['target']).query, keep_blank_values=True)
               for row in requests if urllib.parse.urlsplit(row['target']).path == '/search']
    first, arrived, back, second = (stages[label] for label in ('first-results', 'destination', 'back', 'second-results'))
    expected_back = 'F2 bridge' if case == 'postload' else 'F2 results ' + FIRST
    checks = {'process_completed': run['state'] == 'completed' and run.get('exit_code') == 0,
        'both_real_form_queries_requested': {'q': [FIRST], 'case': [case], 'send': ['']} in queries
            and {'q': [SECOND], 'case': [case], 'send': ['']} in queries,
        'first_results_readable_and_linked': first.get('title') == 'F2 results ' + FIRST
            and all(word in first.get('visible_text', '') for word in ('Original', 'bridge', 'result'))
            and int(first.get('layout', {}).get('viewport_link_regions', '0')) > 0,
        'actual_destination_document_rendered': arrived.get('title') == 'F2 destination'
            and 'destination' in arrived.get('visible_text', ''),
        'one_back_expected_predecessor': back.get('title') == expected_back,
        'different_results_document_rendered': second.get('title') == 'F2 results ' + SECOND,
        'all_stage_pixels_match_actual_document_layout': all(stage.get('paired_generated_pixels', False) for stage in stages.values()),
        'all_stage_scripts_without_errors': all(stage.get('zero_script_errors', False) for stage in stages.values()),
        'immutable_executable': capture.sha256(exe) == exe_before,
        'source_unchanged': capture.source_identity(ROOT) == before,
        'fixture_responses_successful': bool(responses) and all(row['status'] in (200, 302) and row['body_written'] for row in responses),
        'one_browser_back_action_only': action_lines.count('back') == 1}
    hit = arrived.get('last_link_hit', {})
    checks['native_result_activation_uses_original_document'] = (
        hit.get('matched_press') == '1' and hit.get('document') == first.get('document', {}).get('document')
        and hit.get('layout') == first.get('layout', {}).get('layout')
        and arrived.get('document', {}).get('document') != first.get('document', {}).get('document'))
    try:
        first_length = int(first['fixture_state']['history_length'])
        destination_length = int(arrived['fixture_state']['history_length'])
        checks['measured_history_uses_expected_bridge_handling'] = destination_length == first_length + (2 if case == 'postload' else 1)
        checks['traversal_preserves_forward_history_length'] = int(back['fixture_state']['history_length']) == destination_length
    except (KeyError, ValueError):
        checks['measured_history_uses_expected_bridge_handling'] = False
        checks['traversal_preserves_forward_history_length'] = False
    destination_requests = [urllib.parse.parse_qs(urllib.parse.urlsplit(row['target']).query)
        for row in requests if urllib.parse.urlsplit(row['target']).path == '/destination']
    # ChezCat's current supported classic-script phase is post-parse interactive.
    # This is still before complete loading; the Back/history expectations stay unchanged.
    expected_issuer = 'complete' if case in ('postload', 'onload', 'onload-microtask') else 'http302' if case == 'http' else 'interactive'
    checks['destination_reports_actual_issuing_phase'] = bool(destination_requests) and destination_requests[0].get('issuer') == [expected_issuer]
    if case == 'postload':
        checks['completed_bridge_displayed_before_real_button'] = stages['bridge-ready'].get('title') == 'F2 bridge' and stages['bridge-ready'].get('fixture_state', {}).get('ready') == 'complete'
        checks['real_button_event_was_trusted'] = bool(destination_requests) and destination_requests[0].get('trusted') == ['1']
    if case == 'http':
        checks['actual_http302_hop_used'] = any(row['status'] == 302 and row['fixture_kind'] == 'http-hop' for row in responses)
    result = {'case': case, 'passed': all(checks.values()), 'checks': checks, 'run': run,
        'executable_sha256': exe_before, 'runner_sha256': capture.sha256(Path(__file__)),
        'source_before': before, 'source_after': capture.source_identity(ROOT), 'stages': stages,
        'requests': requests, 'responses': responses, 'ordered_http_events': fixture.events,
        'observations': {'expected_back_title': expected_back, 'actual_back_title': back.get('title'),
            'bridge_requests': paths.count('/bridge'), 'destination_requests': paths.count('/destination'),
            'observed_reentry_to_auto_assign_bridge': case in ('startup', 'onload', 'onload-microtask')
                and back.get('title') == 'F2 destination' and paths.count('/bridge') >= 2 and paths.count('/destination') >= 2,
            'history_observation': 'Real JavaScript history.length, HTTP response IDs and native document/navigation/layout stamps; raw session-entry vector/index is unavailable in this executable report.'},
        'scope': __doc__, 'live_service_cause_inferred': False, 'license_accepted_by_driver': False}
    result['evidence_files'] = {str(path.relative_to(folder)): {'bytes': path.stat().st_size, 'sha256': capture.sha256(path)}
        for path in folder.rglob('*') if path.is_file()}
    capture.write_json(folder / 'manifest.json', result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--exe', type=Path, default=ROOT / 'ChezCat.exe',
                        help='existing product executable; this harness never builds it')
    parser.add_argument('--case', choices=(*CASES, 'all'), default='all',
                        help='run all six cases by default, or one named focused case')
    args = parser.parse_args()
    exe = args.exe.resolve()
    selected = CASES if args.case == 'all' else (args.case,)
    folder = capture_tests.verification_root(ROOT) / 'autonomy' / (
        datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
        + '-navigation-journey-' + uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    manifest = folder / 'manifest.json'
    pointer = ROOT / 'results/verification/latest-navigation.json'
    pointer.parent.mkdir(parents=True, exist_ok=True)
    before = capture_tests.source_identity(ROOT)
    exe_before = capture_tests.sha256(exe) if exe.is_file() else None
    checks = {'existing_executable': exe_before is not None, 'existing_exact_license_acceptance': False}
    record = {'schema_version': 1, 'passed': False, 'state': 'running',
        'scope': __doc__, 'source_before': before, 'source_after': before,
        'executable_sha256': exe_before, 'runner_sha256': capture_tests.sha256(Path(__file__)),
        'case_selection': list(selected), 'complete_six_case_suite': selected == CASES,
        'checks': checks, 'cases': [], 'public_service_requests_made_by_driver': 0,
        'license_accepted_by_driver': False}
    capture_tests.write_json(manifest, record)
    capture_tests.write_json(pointer, {'report': capture_tests.evidence_reference(manifest, ROOT),
        'passed': False, 'state': 'running', 'case_selection': list(selected)})
    results = []
    try:
        require(checks['existing_executable'], 'Select an existing product executable; this harness never builds it.')
        # Validate caller-provided local acceptance without creating/changing it.
        # The helper defines the current exact terms and trusted external path.
        accept_license.terms()
        checks['existing_exact_license_acceptance'] = accept_license.check(accept_license.directory())
        require(checks['existing_exact_license_acceptance'],
                'Existing explicit acceptance of the current exact terms is required; this harness never accepts.')
        for case in selected:
            try:
                result = run_case(case, exe, folder / case, capture_tests)
            except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
                result = {'case': case, 'passed': False, 'checks': {'case_completed': False}, 'error': str(error)}
                case_folder = folder / case
                case_folder.mkdir(exist_ok=True)
                capture_tests.write_json(case_folder / 'manifest.json', result)
            results.append(result)
            for name, passed in result['checks'].items():
                checks[case + ':' + name] = passed is True
            record['cases'].append({'case': case, 'passed': result['passed'],
                'manifest': case + '/manifest.json',
                'sha256': capture_tests.sha256(folder / case / 'manifest.json')})
            print(json.dumps({'case': case, 'passed': result['passed'], 'checks': result['checks'],
                'observations': result.get('observations', {})}, ensure_ascii=False), flush=True)
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        record['error'] = str(error)
        checks['preflight_and_cases_completed'] = False
    after = capture_tests.source_identity(ROOT)
    checks['all_selected_cases_completed'] = len(results) == len(selected)
    checks['all_selected_cases_passed'] = bool(results) and all(result['passed'] for result in results)
    checks['source_unchanged'] = before == after
    checks['executable_unchanged'] = exe.is_file() and capture_tests.sha256(exe) == exe_before
    record.update(source_after=after, passed=all(checks.values()), state='completed')
    capture_tests.write_json(manifest, record)
    capture_tests.write_json(pointer, {'report': capture_tests.evidence_reference(manifest, ROOT),
        'passed': record['passed'], 'state': record['state'], 'case_selection': list(selected)})
    print('Evidence: ' + str(folder))
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
