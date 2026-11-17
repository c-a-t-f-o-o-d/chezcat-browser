"""Original finite two-origin stylesheet App/resource/pixel journey.

Checks real parser and dynamic LINK loading, cache authorization, response policy,
source retirement and independent geometry/pixels through owned loopback origins.
No public hosts, builds, profile, acceptance creation or desktop input; no broader
CSSOM, stylesheet CORS, service-usability or full browser-conformance claim.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import http.server
import importlib
import json
from pathlib import Path
import re
import sys
import threading
import uuid

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tools/fixtures/stylesheet-origins'
CASES = ('parser', 'redirect', 'wrong-mime', 'corp', 'csp', 'coep', 'integrity',
         'cache-mode', 'cache-nonce-roundtrip', 'dynamic-replace')
REFERENCE = bytes((19, 47, 83, 101, 137, 173, 211, 37, 71, 251, 113, 17,
                   61, 157, 229, 149, 23, 197, 7, 181, 127, 97, 241, 53,
                   223, 89, 163, 43, 199, 11, 131, 67, 239, 179, 151, 29,
                   31, 109, 193, 167, 227, 73, 109, 13, 211, 241, 59, 139))
PPM = b'P6\n4 4\n255\n' + REFERENCE


def require(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def cookie_names(header):
    # These booleans refer only to fixed original fixture cookie names; no
    # Cookie or Set-Cookie values are retained in the report.
    names = {part.strip().split('=', 1)[0] for part in (header or '').split(';')}
    return {'initial': 'InitialSheetCookie' in names,
            'response': 'StylesheetResponse' in names}


class Origin:
    def __init__(self, role, handler):
        self.role, self.records, self.release = role, [], threading.Event()
        self.admitted = threading.Event()
        outer = self

        class Handler(http.server.BaseHTTPRequestHandler):
            protocol_version = 'HTTP/1.0'

            def setup(self):
                super().setup()
                self.connection.settimeout(3)

            def do_GET(self):
                row = {'origin': role, 'path': self.path, 'cookies': cookie_names(self.headers.get('Cookie')),
                       'completed': False, 'bytes_written': 0}
                outer.records.append(row)
                try:
                    status, mime, body, headers = handler(outer, self.path)
                    row.update(status=status, mime=mime, bytes=len(body), sha256=sha(body))
                    self.close_connection = True
                    self.send_response(status)
                    if mime is not None:
                        self.send_header('Content-Type', mime)
                    self.send_header('Content-Length', str(len(body)))
                    self.send_header('Connection', 'close')
                    self.send_header('Cache-Control', 'no-store')
                    for key, value in headers:
                        self.send_header(key, value)
                    self.end_headers()
                    row['bytes_written'] = self.wfile.write(body)
                    self.wfile.flush()
                    row['completed'] = row['bytes_written'] == len(body)
                except (OSError, ValueError) as error:
                    row['error_type'] = type(error).__name__

            def log_message(self, *_):
                pass

        self.server = http.server.HTTPServer(('127.0.0.1', 0), Handler)
        self.url = 'http://127.0.0.1:' + str(self.server.server_port)
        self.worker = threading.Thread(target=self.server.serve_forever,
            kwargs={'poll_interval': .05}, daemon=False)

    def start(self):
        self.worker.start()

    def close(self):
        self.release.set()
        if self.worker.ident is not None:
            self.server.shutdown()
        self.server.server_close()
        if self.worker.ident is not None:
            self.worker.join(timeout=6)


def case_payload(name, a, b):
    link = '<link rel="stylesheet" href="' + b + '/foreign.css">'
    policy, dynamic = '', ''
    reset = '<style nonce="Allow">#target{width:26px;height:14px;background-color:#bb3311}#tail{width:5px;height:5px;background-color:#df9b12}</style>'
    if name == 'parser':
        link += link
    elif name == 'redirect':
        link = '<link rel="stylesheet" href="/redirect.css">'
    elif name == 'csp':
        policy = '<meta http-equiv="Content-Security-Policy" content="style-src \'self\'">'
    elif name == 'integrity':
        link = '<link rel="stylesheet" href="/integrity.css" integrity="sha256-unverified">'
    elif name == 'cache-mode':
        link += reset + '<link rel="stylesheet" href="' + b + '/foreign.css" crossorigin="anonymous">'
    elif name == 'cache-nonce-roundtrip':
        policy = '<meta http-equiv="Content-Security-Policy" content="style-src \'self\' \'nonce-Allow\'">'
        link = '<link rel="stylesheet" href="/roundtrip.css" nonce="Allow">' + reset + '<link rel="stylesheet" href="/roundtrip.css">'
    elif name == 'dynamic-replace':
        link = ''
        dynamic = '''<button type="button" name="start" id="start">Start original stylesheet replacement</button>
<script>
var loads=0,errors=0;
document.getElementById('start').onclick=function(input){
 if(!input.isTrusted)throw Error('trusted original input');
 var sheet=document.createElement('link');sheet.rel='stylesheet';sheet.href='@@B@@/held.css';
 sheet.onload=function(){loads++;if(loads!==1||errors!==0)throw Error('stale LINK outcome');
  if(document.getElementById('target').clientWidth!==91)throw Error('fresh CSS geometry');
  if(('sheet' in sheet)||('cssRules' in sheet)||('styleSheets' in document)||document.styleSheets!==undefined)throw Error('CSS source exposure');
  document.getElementById('state').textContent='Original replacement complete 1 load 0 errors';
  document.title='B1 replacement complete';};
 sheet.onerror=function(){errors++;throw Error('unexpected current LINK error');};
 document.head.appendChild(sheet);
 var admitted=document.createElement('img');admitted.onload=function(){
  sheet.href='/fresh.css';var release=document.createElement('img');
  release.src='/release.ppm';document.body.appendChild(release);
 };admitted.src='/admitted.ppm';document.body.appendChild(admitted);
};
</script>'''.replace('@@B@@', b)
    html = (FIXTURES / 'page.html').read_text('utf-8')
    return html.replace('@@POLICY@@', policy).replace('@@LINKS@@', link).replace('@@DYNAMIC@@', dynamic).encode()


def paired(output, label, pixels_helper):
    paths = list(output.glob('*-' + label + '.activity.txt'))
    require(len(paths) == 1, 'One paired snapshot required: ' + label)
    activity_path = paths[0]
    stem = activity_path.name.removesuffix('.activity.txt')
    files = {kind: activity_path.with_name(stem + '.' + kind)
             for kind in ('activity.txt', 'layout.txt', 'source.html', 'ppm')}
    require(all(path.is_file() for path in files.values()), 'Paired files missing')
    text = files['activity.txt'].read_text('utf-8')
    d, g, c = [pixels_helper.fields(text, prefix) for prefix in ('Stage document=', 'Stage layout=', 'Stage canvas=')]
    require('Stage paired_ppm_written=1' in text and d.get('document') not in (None, 'unknown')
        and c.get('canvas') not in (None, 'unknown')
        and c.get('document') == d.get('document') == g.get('layout_document')
        and c.get('dom_commit') == d.get('dom_commit') == g.get('layout_dom')
        and c.get('layout') == g.get('layout') not in (None, 'unknown')
        and c.get('layout_dom') == g.get('layout_dom')
        and c.get('layout_document') == d.get('document') and c.get('page_view') == '1', 'Paired identities differ')
    w, h, pixels = pixels_helper.ppm(files['ppm'].read_bytes())
    found = pixels_helper.find_control(pixels, w, h, REFERENCE)
    require(len(found) == 1, 'Independent original img control must match exactly once')
    return {'activity': text, 'layout': files['layout.txt'].read_text('utf-8'),
            'source': files['source.html'].read_text('utf-8'), 'width': w, 'height': h,
            'pixels': pixels, 'control': found[0], 'stamps': [d, g, c],
            'files': {path.name: {'sha256': sha(path.read_bytes()), 'bytes': path.stat().st_size}
                      for path in files.values()}}


def geometry_pixels(snapshot, applied):
    # Locate from the independently encoded img control, then verify full exact
    # solid rectangles AND adjacent uncovered pixels; width/height are not an
    # arbitrary occurrence of a color somewhere on the canvas.
    x, cy = snapshot['control'];w, h = snapshot['width'], snapshot['height'];p = snapshot['pixels']
    tw, th, tailw, tailh = (91, 27, 11, 9) if applied else (26, 14, 5, 5)
    top = cy - (18 + th + tailh + 6)
    def rectangle(y, width, height, color):
        if min(x, y) < 0 or x + width + 1 >= w or y + height > h:
            return False
        return all(p[((y + row) * w + x) * 3:((y + row) * w + x + width) * 3] == color * width
                   and p[((y + row) * w + x + width) * 3:((y + row) * w + x + width + 1) * 3] == b'\xff\xff\xff'
                   for row in range(height))
    return {'same_origin_geometry_pixels': rectangle(top, 64, 18, b'\x1a\x7b\x3c'),
            'target_geometry_pixels': rectangle(top + 18, tw, th, b'\x23\x56\xa9' if applied else b'\xbb\x33\x11'),
            'tail_geometry_pixels': rectangle(top + 18 + th, tailw, tailh, b'\xbc\x19\xd7' if applied else b'\xdf\x9b\x12')}


def run_case(name, repo, exe, folder, capture, helper):
    folder.mkdir()
    same = (FIXTURES / 'same.css').read_bytes();foreign = (FIXTURES / 'foreign.css').read_bytes()
    seed = ('Set-Cookie', 'InitialSheetCookie=present; Path=/; SameSite=Strict; HttpOnly')
    response_cookie = ('Set-Cookie', 'StylesheetResponse=present; Path=/; SameSite=Strict; HttpOnly')
    page = b''
    def serve(origin, path):
        extra = []
        if origin.role == 'A':
            if path == '/':return 200, 'text/html; charset=utf-8', page, [seed] + ([('Cross-Origin-Embedder-Policy', 'require-corp')] if name == 'coep' else [])
            if path == '/same.css':return 200, 'text/css', same, []
            if path == '/control.ppm':return 200, 'image/x-portable-pixmap', PPM, []
            if path == '/admitted.ppm':
                require(b.admitted.wait(5), 'Original held CSS was never admitted')
                return 200, 'image/x-portable-pixmap', b'P6\n1 1\n255\n\x03\x05\x07', []
            if path == '/release.ppm':
                b.release.set();return 200, 'image/x-portable-pixmap', b'P6\n1 1\n255\n\x03\x05\x07', []
            if path == '/redirect.css':return 302, 'text/plain', b'', [('Location', b.url + '/styles/final.css')]
            if path == '/roundtrip.css':return 302, 'text/plain', b'', [('Location', b.url + '/bridge.css')]
            if path in ('/returned.css', '/fresh.css', '/integrity.css'):return 200, 'text/css', foreign, []
        else:
            if path == '/bridge.css':return 302, 'text/plain', b'', [('Location', a.url + '/returned.css')]
            if path == '/held.css':
                b.admitted.set()
                require(b.release.wait(5), 'Original release marker did not arrive')
                return 302, 'text/plain', b'', [('Location', b.url + '/retired-final.css')]
            if path in ('/foreign.css', '/styles/final.css', '/retired-final.css'):
                mime = 'text/html' if name == 'wrong-mime' else 'text/css; charset=utf-8'
                if name == 'corp':extra.append(('Cross-Origin-Resource-Policy', 'same-origin'))
                body = foreign
                if path == '/styles/final.css':
                    body += b'\n#target{background-image:url("Pixels/Unfetchable.ppm");background-repeat:no-repeat}\n'
                return 200, mime, body, extra + [response_cookie]
        return 404, 'text/plain', b'Unknown original fixture route', []
    a, b = Origin('A', serve), Origin('B', serve)
    page = case_payload(name, a.url, b.url)
    actions = 'wait\t600\nsnapshot\tchecked\n'
    if name == 'dynamic-replace':actions = 'click\tstart\nwait\t600\nsnapshot\tchecked\n'
    script = folder / 'input.tsv';script.write_text(actions, encoding='utf-8', newline='\n')
    out = folder / 'journey'
    checks, data = {}, {}
    try:
        a.start();b.start()
        run = capture.execute([str(exe), a.url + '/', '--dev-input-script', str(script),
            '--dev-input-output', str(out), '--no-profile', '--images',
            '--javascript' if name == 'dynamic-replace' else '--no-javascript'], folder / 'command.log', repo, 120)
        checks['process_completed'] = run['state'] == 'completed' and run['exit_code'] == 0
        s = paired(out, 'checked', helper)
        applied = name in ('parser', 'redirect', 'dynamic-replace')
        paint_states = {key: geometry_pixels(s, value) for key, value in [('applied', True), ('unapplied', False)]}
        checks.update(paint_states['applied' if applied else 'unapplied'])
        checks['paired_document_layout_canvas_pixels'] = True
        checks['original_source_identity'] = all(value in s['source'] for value in ('id="same"', 'id="target"', 'id="tail"', 'id="control"'))
        own = a.records;foreign_rows = b.records
        expected_foreign = name not in ('csp', 'coep', 'integrity')
        checks['foreign_contact_matches_policy'] = bool(foreign_rows) == expected_foreign
        checks['no_cors_real_cookie_include'] = (bool(foreign_rows) and all(row['cookies']['initial'] for row in foreign_rows)
                                               if expected_foreign else not foreign_rows)
        controls = [r for r in own if r['path'] == '/control.ppm']
        checks['document_cookie_reaches_independent_control'] = len(controls) == 1 and controls[0]['cookies']['initial']
        if name in ('parser', 'redirect', 'wrong-mime', 'corp'):
            checks['eligible_response_cookie_survives_publication_result'] = len(controls) == 1 and controls[0]['cookies']['response']
        if name == 'parser':checks['ordinary_duplicate_reuse_one_foreign_get'] = [r['path'] for r in foreign_rows] == ['/foreign.css']
        if name == 'redirect':
            # The existing rejection omits the resolved image URL. These
            # observations prove the foreign base retains document image policy;
            # exact relative-directory resolution is covered by native fixtures.
            checks['redirected_foreign_css_preserves_image_origin_boundary'] = (
                'CSS /redirect.css: loaded ' + b.url + '/styles/final.css' in s['activity'].splitlines()
                and len(re.findall(r'(?m)^BACKGROUND IMAGE node \d+: Cross-origin dynamic image request blocked\.$', s['activity'])) == 1
                and not any('Unfetchable.ppm' in r['path'] for r in own + foreign_rows))
        if name == 'integrity':checks['integrity_refused_before_network'] = not any(r['path'] == '/integrity.css' for r in own)
        if name == 'cache-mode':checks['marked_owner_does_not_reuse_or_refetch_foreign'] = [r['path'] for r in foreign_rows] == ['/foreign.css']
        if name == 'cache-nonce-roundtrip':
            checks['nonce_roundtrip_foreign_hop_once'] = [r['path'] for r in foreign_rows] == ['/bridge.css']
            checks['nonce_roundtrip_returned_sheet_once'] = sum(r['path'] == '/returned.css' for r in own) == 1
        if name == 'dynamic-replace':
            checks['held_old_source_really_contacted'] = any(r['path'] == '/held.css' for r in foreign_rows)
            checks['retired_redirect_never_contacted'] = not any(r['path'] == '/retired-final.css' for r in foreign_rows)
            checks['real_admission_release_markers_and_fresh_sheet'] = all(sum(r['path'] == path for r in own) == 1 for path in ('/admitted.ppm', '/release.ppm', '/fresh.css'))
            checks['exact_current_outcome_and_no_source_exposure'] = 'Page title: B1 replacement complete' in s['activity'] and 'Original replacement complete 1 load 0 errors' in ' '.join(re.findall(r'text="([^"\n]*)"', s['layout']))
            summaries = re.findall(r'(?m)^\d+ script\(s\).*?(\d+) error\(s\)', s['activity'])
            checks['current_interpreter_zero_errors'] = bool(summaries) and summaries[-1] == '0'
        else:checks['javascript_explicitly_disabled'] = 'JavaScript enabled: false' in (out / 'input.log').read_text('utf-8')
        checks['complete_known_responses'] = all(r.get('status') != 404 and r.get('completed') for r in own + foreign_rows if not (name == 'dynamic-replace' and r['path'] == '/held.css'))
        data = {'run': run, 'snapshots': s['files'], 'stamps': s['stamps'], 'control_origin': s['control'],
                'served_html_sha256': sha(page), 'expected_applied': applied, 'observed_paint_states': paint_states}
    except (OSError, ValueError, KeyError, TypeError) as error:
        checks['case_finished'] = False;data['error_type'] = type(error).__name__;data['error'] = str(error)
    finally:
        a.release.set();b.release.set();a.close();b.close()
    checks['owned_servers_stopped'] = not a.worker.is_alive() and not b.worker.is_alive()
    checks['owned_sockets_closed'] = a.server.socket.fileno() == -1 and b.server.socket.fileno() == -1
    return {'checks': checks, 'passed': bool(checks) and all(v is True for v in checks.values()),
            'responses': a.records + b.records, **data}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=ROOT, help='source/runtime root; defaults to the product checkout')
    parser.add_argument('--exe', type=Path, help='existing copied executable; never built or modified')
    parser.add_argument('--case', action='append', choices=CASES, help='finite case; default all')
    args = parser.parse_args();repo = args.repo.resolve();exe = (args.exe or repo / 'ChezCat.exe').resolve()
    sys.path.insert(0, str(repo / 'tools'))
    capture = importlib.import_module('capture_tests');accept = importlib.import_module('accept_license')
    helper = importlib.import_module('check_background_harness')
    accept.terms();require(accept.check(accept.directory()), 'Caller-provided exact-term receipt required; no implicit acceptance')
    require(exe.is_file(), 'Existing executable required')
    provenance = json.loads((FIXTURES / 'provenance.json').read_bytes())
    for name, expected in provenance['files'].items():
        body = (FIXTURES / name).read_bytes()
        require(len(body) == expected['bytes'] and sha(body) == expected['sha256'], 'Original fixture identity differs: ' + name)
    folder = capture.verification_root(repo) / 'autonomy' / (datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ') + '-two-origin-stylesheets-' + uuid.uuid4().hex[:8]);folder.mkdir(parents=True)
    before = capture.source_identity(repo);exe_hash = capture.sha256(exe)
    record = {'schema_version': 1, 'scope': __doc__, 'source_before': before, 'executable_sha256': exe_hash,
              'runner_sha256': sha(Path(__file__).read_bytes()), 'fixture_files': {p.name: sha(p.read_bytes()) for p in sorted(FIXTURES.iterdir()) if p.is_file()},
              'license_accepted_by_driver': False, 'public_requests': 0, 'cases': {}}
    for name in args.case or CASES:record['cases'][name] = run_case(name, repo, exe, folder / name, capture, helper)
    record['source_after'] = capture.source_identity(repo)
    record['checks'] = {'source_unchanged': before == record['source_after'], 'executable_unchanged': exe_hash == capture.sha256(exe),
                        'all_cases_passed': all(r['passed'] for r in record['cases'].values())}
    record['passed'] = all(record['checks'].values());record['state'] = 'completed'
    capture.write_json(folder / 'manifest.json', record)
    print(json.dumps({'checks': record['checks'], 'cases': {n: r['checks'] for n, r in record['cases'].items()}}, indent=2))
    print('Evidence: ' + str(folder))
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
