"""Original video App/input/offscreen journey over loopback HTTP and HTTPS.

Reuses every authored-video clock, Promise/event, native/script control and
independent decoded-frame assertion. HTTPS uses the retained public TEST CA for
this browser process only. No public service, OS trust, audio device, native
desktop, cross-origin, streaming/range or general media conformance claim.
Caller-provided exact-term acceptance is checked, never created by this driver.
"""
import argparse
import contextlib
from datetime import datetime, timezone
import hashlib
import http.server
import json
from pathlib import Path
import re
import ssl
import threading
import uuid

import accept_license
import capture_tests
import check_authored_video_harness as authored
from https_fixture_server import FIXTURES, TlsFixtureServer, context


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / 'tools/fixtures'
TRANSPORTS = ('http', 'https')


def require(value, message):
    if not value:
        raise ValueError(message)


def fixture_payloads(fixture_stem=None):
    fixture_stem=Path(fixture_stem) if fixture_stem is not None else FIXTURE_ROOT/'scene-intra'
    return {
        '/authored-video.html': ('text/html; charset=utf-8',
            (FIXTURE_ROOT / 'authored-video.html').read_bytes()),
        '/scene-intra.ivf': ('video/x-ivf',
            fixture_stem.with_suffix('.ivf').read_bytes()),
        '/authored-video-next.html': ('text/html; charset=utf-8',
            (FIXTURE_ROOT / 'authored-video-next.html').read_bytes()),
    }


def run_case(transport, folder, fixture_stem=None, display_profile=None, *,
             authored_runner=None, served_payloads=None, entry_path='/authored-video.html',
             route_checks=None, scope=None):
    folder.mkdir()
    payloads = fixture_payloads(fixture_stem) if served_payloads is None else served_payloads
    if transport not in TRANSPORTS or entry_path not in payloads:
        raise ValueError('Expected one controlled HTTP(S) fixture entry.')
    records = []
    lock = threading.Lock()

    class Handler(http.server.BaseHTTPRequestHandler):
        protocol_version = 'HTTP/1.1'

        def setup(self):
            super().setup()
            self.connection.settimeout(10)

        def do_GET(self):
            selected = payloads.get(self.path)
            mime, body = selected or ('text/plain; charset=utf-8', b'Unknown original fixture route.')
            row = {
                'method': 'GET', 'target': self.path,
                'status': 200 if selected else 404, 'mime': mime,
                'bytes': len(body), 'sha256': hashlib.sha256(body).hexdigest(),
                'bytes_written': 0, 'completed': False,
                'range_requested': self.headers.get('Range') is not None,
                'if_range_requested': self.headers.get('If-Range') is not None,
                'tls': None,
            }
            if isinstance(self.connection, ssl.SSLSocket):
                cipher = self.connection.cipher()
                row['tls'] = {'version': self.connection.version(),
                    'cipher': cipher[0] if cipher else None,
                    'alpn': self.connection.selected_alpn_protocol()}
            try:
                self.close_connection = True
                self.send_response(row['status'])
                self.send_header('Content-Type', mime)
                self.send_header('Content-Length', str(len(body)))
                self.send_header('Cache-Control', 'no-store')
                self.send_header('Connection', 'close')
                self.end_headers()
                row['bytes_written'] = self.wfile.write(body)
                self.wfile.flush()
                row['completed'] = row['bytes_written'] == len(body)
            except (OSError, ValueError) as error:
                row['error_type'] = type(error).__name__
            finally:
                with lock:
                    records.append(row)

        def log_message(self, *_):
            pass

    if transport == 'https':
        tls = context(FIXTURES / 'server-p256-chain.pem',
            FIXTURES / 'server-p256-key.pem', '1.3')
        server = TlsFixtureServer(('127.0.0.1', 0), Handler, tls)
        host = 'localhost'
        trust_file = FIXTURES / 'demo-roots.pem'
    else:
        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        server.daemon_threads = True
        host = '127.0.0.1'
        trust_file = None
    # Join bounded request handlers too, including TLS close_notify cleanup.
    server.daemon_threads = False
    server.block_on_close = True
    worker = threading.Thread(target=server.serve_forever,
        kwargs={'poll_interval': .1}, daemon=True)
    worker.start()
    source = f'{transport}://{host}:{server.server_port}{entry_path}'
    child_log = folder / 'authored-checker.log'
    try:
        with child_log.open('w', encoding='utf-8', newline='\n') as output:
            with contextlib.redirect_stdout(output):
                if authored_runner is None:
                    status = authored.main(source=source, trust_file=trust_file, fixture_stem=fixture_stem, display_profile=display_profile)
                else:
                    status = authored_runner(source=source, trust_file=trust_file)
    finally:
        server.shutdown()
        server.server_close()
        worker.join(timeout=3)
    with lock:
        responses = list(records)
    capture_tests.write_json(folder / 'responses.json', responses)
    evidence = re.findall(r'^Evidence: (.+)$', child_log.read_text(encoding='utf-8'), re.M)
    require(len(evidence) == 1, 'The reused authored checker must emit exactly one evidence record.')
    child_folder = Path(evidence[0].strip()).resolve()
    require(child_folder.is_relative_to(capture_tests.verification_root(ROOT)),
        'Authored evidence must stay inside the product verification directory.')
    child_path = child_folder / 'manifest.json'
    child = json.loads(child_path.read_text(encoding='utf-8'))
    checks = {'authored_checker_passed': status == 0 and bool(child.get('checks'))
        and all(value is True for value in child['checks'].values()),
        'caller_source_and_process_trust_retained': child.get('source') == source
            and child.get('trust_file_sha256') == (capture_tests.sha256(trust_file) if trust_file else None),
        'server_worker_stopped': not worker.is_alive(),
        'only_exact_original_routes': bool(responses) and all(row['target'] in payloads for row in responses),
        'full_fetch_without_range': bool(responses) and all(not row['range_requested'] for row in responses)}
    required = route_checks if route_checks is not None else {
        'page_served': ('/authored-video.html', 1),
        'clip_served_and_reloaded': ('/scene-intra.ivf', 2),
        'leave_destination_served': ('/authored-video-next.html', 1)}
    checks.update({name: sum(row['target'] == route for row in responses) >= minimum
                   for name, (route, minimum) in required.items()})
    checks['exact_served_response_bytes_mime_and_hash'] = bool(responses) and all(
        row['target'] in payloads and row['status'] == 200 and row['completed'] is True
        and row['mime'] == payloads[row['target']][0]
        and row['bytes'] == row['bytes_written'] == len(payloads[row['target']][1])
        and row['sha256'] == hashlib.sha256(payloads[row['target']][1]).hexdigest()
        for row in responses)
    checks['actual_tls13_on_every_response' if transport == 'https' else 'plain_http_without_tls'] = (
        bool(responses) and all(row['tls'] is not None and row['tls']['version'] == 'TLSv1.3'
            and bool(row['tls']['cipher']) for row in responses)
        if transport == 'https' else bool(responses) and all(row['tls'] is None for row in responses))
    checks.update({'authored:' + name: value is True for name, value in child['checks'].items()})
    result = {'transport': transport, 'passed': all(checks.values()), 'checks': checks,
        'source': source, 'authored_manifest': capture_tests.evidence_reference(child_path, ROOT),
        'authored_manifest_sha256': capture_tests.sha256(child_path),
        'source_before': child['source_before'], 'source_after': child['source_after'],
        'executable_sha256': child['executable_sha256'], 'responses': responses,
        'trust_file_sha256': capture_tests.sha256(trust_file) if trust_file else None,
        'scope': __doc__ if scope is None else scope, 'license_accepted_by_driver': False,
        'os_trust_changed': False, 'public_service_requests_made_by_driver': 0}
    capture_tests.write_json(folder / 'manifest.json', result)
    return result


def main():
    argparse.ArgumentParser(description=__doc__).parse_args()
    folder = capture_tests.verification_root(ROOT) / 'runs' / (
        datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
        + '-network-media-journey-' + uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    before = capture_tests.source_identity(ROOT)
    exe_before = capture_tests.sha256(ROOT / 'ChezCat.exe') if (ROOT / 'ChezCat.exe').is_file() else None
    checks = {'existing_executable': exe_before is not None, 'existing_exact_license_acceptance': False}
    record = {'schema_version': 1, 'passed': False, 'state': 'running',
        'scope': __doc__, 'checks': checks, 'cases': [],
        'source_before': before, 'source_after': before,
        'executable_sha256': exe_before,
        'runner_sha256': capture_tests.sha256(Path(__file__)),
        'authored_checker_sha256': capture_tests.sha256(Path(authored.__file__)),
        'license_accepted_by_driver': False, 'os_trust_changed': False,
        'public_service_requests_made_by_driver': 0}
    manifest = folder / 'manifest.json'
    pointer = ROOT / 'results/verification/latest-network-media.json'
    pointer.parent.mkdir(parents=True, exist_ok=True)
    capture_tests.write_json(manifest, record)
    capture_tests.write_json(pointer, {'report': capture_tests.evidence_reference(manifest, ROOT),
        'passed': False, 'state': 'running'})
    try:
        require(checks['existing_executable'], 'Build the existing product executable before running this journey.')
        accept_license.terms()
        checks['existing_exact_license_acceptance'] = accept_license.check(accept_license.directory())
        require(checks['existing_exact_license_acceptance'],
            'Existing explicit acceptance is required; this driver never accepts terms.')
        for transport in TRANSPORTS:
            try:
                result = run_case(transport, folder / transport)
            except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
                result = {'transport': transport, 'passed': False,
                    'checks': {'case_completed': False}, 'error': str(error)}
                case_folder = folder / transport
                case_folder.mkdir(exist_ok=True)
                capture_tests.write_json(case_folder / 'manifest.json', result)
            record['cases'].append({'transport': transport, 'passed': result['passed'],
                'manifest': transport + '/manifest.json',
                'sha256': capture_tests.sha256(folder / transport / 'manifest.json')})
            checks.update({transport + ':' + name: value is True for name, value in result['checks'].items()})
            if 'source_before' in result:
                checks[transport + ':exact_executable_and_source_identity'] = (
                    result['executable_sha256'] == exe_before
                    and result['source_before'] == result['source_after'] == before)
            print(json.dumps({'transport': transport, 'passed': result['passed']}, indent=2), flush=True)
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        record['error'] = str(error)
    after = capture_tests.source_identity(ROOT)
    checks['source_unchanged'] = after == before
    checks['executable_unchanged'] = exe_before is not None and capture_tests.sha256(ROOT / 'ChezCat.exe') == exe_before
    checks['both_complete_transports'] = len(record['cases']) == len(TRANSPORTS)
    record.update(source_after=after, passed=all(checks.values()), state='completed')
    capture_tests.write_json(manifest, record)
    capture_tests.write_json(pointer, {'report': capture_tests.evidence_reference(manifest, ROOT),
        'passed': record['passed'], 'state': record['state']})
    print(json.dumps(checks, indent=2))
    print('Evidence: ' + str(folder))
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
