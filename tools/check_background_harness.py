"""Original external-CSS background App/input/offscreen loopback journey.

Requires the exact painted single-layer background, an independent working img
control and unchanged layout geometry. Verifies actual response bytes, relative
case-sensitive CSS URL resolution and paired document/layout/canvas pixels.
No public-service, native-desktop, full CSS-image or screenshot-parity claim.
Uses an existing executable and caller-provided exact-term acceptance only;
this driver neither builds the application nor records acceptance.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import http.server
import json
from pathlib import Path
import re
import threading
import uuid

import accept_license
import capture_tests


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tools/fixtures/css-background'
ROUTES = {
    '/': ('index.html', 'text/html; charset=utf-8'),
    '/styles/theme.css': ('styles/theme.css', 'text/css; charset=utf-8'),
    '/styles/Pixels/CornerBG.ppm': ('styles/Pixels/CornerBG.ppm', 'image/x-portable-pixmap'),
    '/images/control.ppm': ('images/control.ppm', 'image/x-portable-pixmap'),
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def fields(activity, prefix):
    line = next((line for line in activity.splitlines() if line.startswith(prefix)), '')
    return dict(re.findall(r'(\w+)=([^\s]+)', line))


def ppm(data):
    require(len(data) <= 256 * 1024 * 1024, 'Pixel evidence exceeds the finite allowance.')
    parts = data.split(b'\n', 3)
    require(len(parts) == 4 and parts[0] == b'P6' and parts[2] == b'255', 'Expected original P6 pixel evidence.')
    dimensions = parts[1].split()
    require(len(dimensions) == 2, 'Invalid pixel dimensions.')
    width, height = map(int, dimensions)
    require(width > 0 and height > 0 and width * height <= 64 * 1024 * 1024
        and len(parts[3]) == width * height * 3, 'Invalid bounded pixel extent.')
    return width, height, parts[3]


def find_control(pixels, width, height, reference):
    found, offset = [], 0
    while True:
        offset = pixels.find(reference[:3], offset)
        if offset < 0:
            return found
        if offset % 3 == 0:
            x, y = (offset // 3) % width, (offset // 3) // width
            if x + 4 <= width and y + 4 <= height and all(
                pixels[((y + row) * width + x) * 3:((y + row) * width + x + 4) * 3]
                == reference[row * 12:(row + 1) * 12] for row in range(4)):
                found.append((x, y))
                if len(found) > 1:
                    return found
        offset += 1


def run_case(exe, folder, checks):
    provenance = json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8'))
    payloads = {}
    for route, (name, mime) in ROUTES.items():
        body = (FIXTURES / name).read_bytes()
        expected = provenance['files'][name]
        require(len(body) == expected['bytes'] and digest(body) == expected['sha256'],
            'Original fixture identity mismatch: ' + name)
        payloads[route] = (body, mime)
    checks['original_fixture_hashes'] = True
    records = []

    class Handler(http.server.BaseHTTPRequestHandler):
        protocol_version = 'HTTP/1.0'

        def setup(self):
            super().setup()
            self.connection.settimeout(5)

        def do_GET(self):
            selected = payloads.get(self.path)
            body, mime = selected or (b'Unknown original fixture route.', 'text/plain; charset=utf-8')
            row = {'method': 'GET', 'target': self.path, 'status': 200 if selected else 404,
                'mime': mime, 'bytes': len(body), 'sha256': digest(body),
                'bytes_written': 0, 'completed': False}
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
                records.append(row)

        def log_message(self, *_):
            pass

    # One owned non-daemon worker serves the four tiny responses sequentially.
    # Every accepted socket has a finite timeout; no detached request threads.
    script = folder / 'input.tsv'
    script.write_text('wait\t750\nsnapshot\tpainted\n', encoding='utf-8', newline='\n')
    output = folder / 'journey'
    server = http.server.HTTPServer(('127.0.0.1', 0), Handler)
    worker = threading.Thread(target=server.serve_forever,
        kwargs={'poll_interval': .1}, daemon=False)
    try:
        worker.start()
        run = capture_tests.execute([str(exe), f'http://127.0.0.1:{server.server_port}/',
            '--dev-input-script', str(script), '--dev-input-output', str(output),
            '--javascript', '--no-profile'], folder / 'command.log', ROOT, 120)
    finally:
        if worker.ident is not None:
            server.shutdown()
        server.server_close()
        if worker.ident is not None:
            worker.join(timeout=6)
    checks['owned_server_worker_stopped'] = not worker.is_alive()
    checks['owned_server_socket_closed'] = server.socket.fileno() == -1
    checks['process_completed'] = run['state'] == 'completed' and run['exit_code'] == 0
    checks['each_original_route_requested_once'] = (len(records) == len(ROUTES)
        and all(sum(row['target'] == route for row in records) == 1 for route in ROUTES))
    checks['exact_response_mime_bytes_hash_and_completion'] = bool(records) and all(
        row['target'] in payloads and row['status'] == 200 and row['method'] == 'GET'
        and row['mime'] == payloads[row['target']][1]
        and row['bytes'] == len(payloads[row['target']][0])
        and row['sha256'] == digest(payloads[row['target']][0])
        and row['completed'] and row['bytes_written'] == row['bytes'] for row in records)
    capture_tests.write_json(folder / 'responses.json', records)
    snapshots = sorted(output.glob('*-painted.activity.txt'))
    checks['one_painted_snapshot'] = len(snapshots) == 1
    require(checks['one_painted_snapshot'], 'Expected exactly one actual painted snapshot.')
    activity_path = snapshots[0]
    stem = activity_path.name.removesuffix('.activity.txt')
    layout_path = activity_path.with_name(stem + '.layout.txt')
    source_path = activity_path.with_name(stem + '.source.html')
    pixel_path = activity_path.with_name(stem + '.ppm')
    activity = activity_path.read_text(encoding='utf-8')
    checks['paired_evidence_files_present'] = all(path.is_file() for path in [layout_path, source_path, pixel_path])
    require(checks['paired_evidence_files_present'], 'Actual paired source/layout/pixels are required.')
    layout = layout_path.read_text(encoding='utf-8')
    source = source_path.read_text(encoding='utf-8')
    document = fields(activity, 'Stage document=')
    geometry = fields(activity, 'Stage layout=')
    canvas = fields(activity, 'Stage canvas=')
    checks['paired_document_layout_canvas_stamps'] = (
        'Stage paired_ppm_written=1' in activity
        and all(record.get(key) not in [None, 'unknown'] for record, keys in [
            (document, ['document', 'dom_commit']),
            (geometry, ['layout', 'layout_document', 'layout_dom']),
            (canvas, ['document', 'dom_commit', 'layout', 'layout_document', 'layout_dom', 'page_view'])]
            for key in keys)
        and document.get('document', 'unknown') != 'unknown'
        and canvas.get('document') == document.get('document') == geometry.get('layout_document')
        and canvas.get('dom_commit') == document.get('dom_commit')
        and canvas.get('layout') == geometry.get('layout') != 'unknown'
        and canvas.get('layout_dom') == geometry.get('layout_dom') == document.get('dom_commit')
        and canvas.get('layout_document') == document.get('document')
        and canvas.get('page_view') == '1')
    checks['zero_script_errors'] = '0 error(s).' in activity
    checks['measured_geometry_observed_in_actual_page'] = (
        'Page title: C1 geometry 32x24 / 4x4' in activity
        and 'Geometry target32x24 control4x4' in ' '.join(re.findall(r'text="([^"\n]*)"', layout)))
    checks['actual_original_html_source'] = ('styles/theme.css' in source
        and 'images/control.ppm' in source and 'id="target"' in source)
    width, height, pixels = ppm(pixel_path.read_bytes())
    cw, ch, control = ppm(payloads['/images/control.ppm'][0])
    bw, bh, background = ppm(payloads['/styles/Pixels/CornerBG.ppm'][0])
    require((cw, ch, bw, bh) == (4, 4, 4, 4), 'Original independent pixel references changed.')
    found = find_control(pixels, width, height, control)
    checks['independent_img_control_exact_pixels_once'] = len(found) == 1
    checks['target_position_from_independent_control_and_geometry'] = False
    checks['background_pixels_and_uncovered_color_exact'] = False
    observations = {'requests': records, 'control_matches': [list(point) for point in found],
        'document': document, 'layout': geometry, 'canvas': canvas}
    if len(found) == 1:
        x, control_y = found[0]
        y = control_y - 28  # Measured 24px target plus the original 4px margin.
        valid = x >= 0 and y >= 0 and x + 32 <= width and y + 24 <= height
        checks['target_position_from_independent_control_and_geometry'] = valid
        if valid:
            crop = b''.join(pixels[((y + row) * width + x) * 3:((y + row) * width + x + 32) * 3]
                for row in range(24))
            expected = bytearray(b'\x11\x22\x33' * (32 * 24))
            for row in range(4):
                expected[row * 32 * 3:row * 32 * 3 + 12] = background[row * 12:(row + 1) * 12]
            checks['background_pixels_and_uncovered_color_exact'] = crop == bytes(expected)
            observations.update(target_origin=[x, y], control_origin=[x, control_y],
                target_crop_sha256=digest(crop), expected_target_sha256=digest(expected))
    return {'run': run, 'observations': observations,
        'fixture_files': provenance['files'], 'snapshot_files': {
            path.name: {'bytes': path.stat().st_size, 'sha256': capture_tests.sha256(path)}
            for path in [activity_path, layout_path, source_path, pixel_path]}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--exe', type=Path, default=ROOT / 'ChezCat.exe',
        help='existing product executable; never built by this driver')
    args = parser.parse_args()
    exe = args.exe.resolve()
    folder = capture_tests.verification_root(ROOT) / 'runs' / (
        datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
        + '-background-journey-' + uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    manifest = folder / 'manifest.json'
    pointer = ROOT / 'results/verification/latest-background.json'
    pointer.parent.mkdir(parents=True, exist_ok=True)
    checks = {'existing_executable': False, 'existing_exact_license_acceptance': False,
        'journey_completed': False}
    record = {'schema_version': 1, 'passed': False, 'state': 'running', 'scope': __doc__,
        'checks': checks, 'source_before': {}, 'source_after': {}, 'executable_sha256': None,
        'runner_sha256': capture_tests.sha256(Path(__file__)),
        'license_accepted_by_driver': False, 'public_service_requests_made_by_driver': 0}
    capture_tests.write_json(manifest, record)
    capture_tests.write_json(pointer, {'report': capture_tests.evidence_reference(manifest, ROOT), 'passed': False, 'state': 'running'})
    try:
        record['source_before'] = capture_tests.source_identity(ROOT)
        checks['existing_executable'] = exe.is_file()
        require(checks['existing_executable'], 'Select an existing executable; this harness never builds it.')
        record['executable_sha256'] = capture_tests.sha256(exe)
        accept_license.terms()
        checks['existing_exact_license_acceptance'] = accept_license.check(accept_license.directory())
        require(checks['existing_exact_license_acceptance'], 'Existing explicit exact-term acceptance is required; this harness never accepts.')
        record.update(run_case(exe, folder, checks))
        checks['journey_completed'] = True
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        record['error_type'] = type(error).__name__
        record['error'] = str(error)
    record['source_after'] = capture_tests.source_identity(ROOT)
    checks['source_unchanged'] = record['source_before'] == record['source_after']
    checks['executable_unchanged'] = exe.is_file() and capture_tests.sha256(exe) == record['executable_sha256']
    record.update(passed=bool(checks) and all(value is True for value in checks.values()), state='completed')
    capture_tests.write_json(manifest, record)
    capture_tests.write_json(pointer, {'report': capture_tests.evidence_reference(manifest, ROOT), 'passed': record['passed'], 'state': record['state']})
    print(json.dumps(checks, indent=2))
    print('Evidence: ' + str(folder))
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
