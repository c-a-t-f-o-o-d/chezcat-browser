#!/usr/bin/env python3
"""Exercise ChezCat's activity/source CLI using only Python's standard library.

Uses original local/loopback fixtures and synthetic profiles. No public websites
or personal profiles are opened. All fixture files are temporary; --output is
the JSON verification record. This is a development tool, not a browser runtime
dependency. Works with a native Linux build or the Windows executable.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile
import threading
import time


HTML = b'''<!doctype html><title>ChezCat activity fixture</title>
<style>body{background:#fff;color:#223344}p{font-size:18px}</style>
<body><p id="state">Before timer</p><script src="fixture.js"></script></body>'''
SCRIPT = b'''setTimeout(function(){
document.getElementById("state").textContent="Timer settled";
throw new Error("ACTIVITY_TIMER_MARKER");
},25);
throw new Error("ACTIVITY_INLINE_MARKER");'''
FLOW_HTML = b'''<!doctype html><html><head><title>Bootstrap</title></head><body><script>
document.cookie="visible=two; Path=/; SameSite=Lax";
var boot=document.createElement("script");boot.src="/boot.js";
document.head.appendChild(boot);
</script></body></html>'''
FLOW_SCRIPT = b'''if(document.cookie!=="visible=two")throw Error("COOKIE_VISIBILITY");
location.replace("/search?q=test");'''
TIMER_HTML = b'''<!doctype html><title>Before navigation</title><body><script>
setTimeout(function(){location.assign("/search?q=test");},25);
</script></body>'''
RESULT_HTML = b'''<!doctype html><title>Actual results</title><body><p id="state">Original results</p>
<script>document.getElementById("state").textContent="Changed results";</script></body>'''
PROFILE = (b'OwnBrowser profile 1\njavascript\tfalse\nimages\ttrue\n'
           b'default_zoom\t125\nhome\t\n'
           b'bookmark\tSaved fixture\thttps://example.invalid/saved\n')
HISTORY = (b'OwnBrowser history 1\nenabled\ttrue\nclear_on_exit\ttrue\n'
           b'retention_days\t1\nnext_id\t2\n'
           b'visit\t1\t1\tOld fixture\thttps://example.invalid/old\n')


def require(condition, message):
    if not condition:
        raise AssertionError(message)


@contextmanager
def serving():
    requests = []
    request_cookies = []

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_GET(self):
            requests.append(self.path)
            request_cookies.append((self.path, self.headers.get('Cookie', '')))
            routes = {'/fixture.html': ('text/html', HTML),
                      '/fixture.js': ('text/javascript', SCRIPT),
                      '/bootstrap': ('text/html', FLOW_HTML),
                      '/boot.js': ('text/javascript', FLOW_SCRIPT),
                      '/timer': ('text/html', TIMER_HTML),
                      '/search?q=test': ('text/html', RESULT_HTML)}
            mime, body = routes.get(self.path, ('text/plain', b'Not found'))
            self.send_response(200 if self.path in routes else 404)
            self.send_header('Content-Type', mime)
            self.send_header('Content-Length', str(len(body)))
            self.send_header('Connection', 'close')
            if self.path == '/bootstrap':
                self.send_header('Set-Cookie', 'private=one; Path=/; HttpOnly; SameSite=Lax')
            self.end_headers()
            self.wfile.write(body)

    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    server.daemon_threads = True
    thread = threading.Thread(target=server.serve_forever,
                              kwargs={'poll_interval': 0.02}, daemon=True)
    thread.start()
    try:
        yield f'http://127.0.0.1:{server.server_port}', requests, request_cookies
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=3)


def exercise(browser, root, results):
    page = root / 'fixture.html'
    page.write_bytes(HTML)
    (root / 'fixture.js').write_bytes(SCRIPT)
    explicit_profile = root / 'explicit-profile'
    fake_appdata = root / 'synthetic-localappdata'
    profile_files = []
    for folder in (explicit_profile, fake_appdata / 'OwnBrowser'):
        folder.mkdir(parents=True)
        for name, content in (('profile.txt', PROFILE), ('history.txt', HISTORY)):
            target = folder / name
            target.write_bytes(content)
            profile_files.append((target, content))
    # A Windows regression cannot touch the real default profile even if the
    # code accidentally stops treating report-only mode as headless.
    environment = dict(os.environ, LOCALAPPDATA=str(fake_appdata))

    def run(name, arguments, success=True):
        command = [str(browser), *map(str, arguments)]
        process = subprocess.run(command, cwd=root, env=environment,
                                 capture_output=True, text=True, encoding='utf-8',
                                 errors='replace', timeout=30)
        results['runs'].append({'name': name, 'returncode': process.returncode,
                                'stderr': process.stderr[-4000:]})
        require((process.returncode == 0) == success,
                f'{name}: unexpected exit {process.returncode}: {process.stderr}')
        return process

    def passed(name):
        results['checks'].append(name)

    def unchanged():
        require(all(path.read_bytes() == content for path, content in profile_files),
                'A synthetic profile or history file changed')

    with serving() as (origin, requests, request_cookies):
        report = root / 'loopback.txt'
        run('report-only HTTP page', [origin + '/fixture.html', '--no-profile',
                                     '--activity-report', report, '--wait-ms', '150'])
        text = report.read_text(encoding='utf-8')
        results['browser_version'] = text.splitlines()[0]
        for expected in ('Page title: ChezCat activity fixture', 'fixture.js',
                         'ACTIVITY_INLINE_MARKER', 'ACTIVITY_TIMER_MARKER',
                         '1 timer callback(s) executed', 'HTTP 200',
                         'DOCUMENT: ' + origin + '/fixture.html'):
            require(expected in text, f'Report omits {expected!r}')
        require(requests == ['/fixture.html', '/fixture.js'],
                f'Unexpected loopback requests: {requests}')
        passed('report-only captures real resource activity, script error, and timer error')

        common = [page, '--no-profile', '--wait-ms', '150', '--width', '480', '--height', '320']
        plain, paired = root / 'plain.ppm', root / 'paired.ppm'
        run('render-only', [*common, '--render', plain])
        run('report with render', [*common, '--render', paired,
                                  '--activity-report', root / 'paired.txt',
                                  '--save-source', root / 'paired.html'])
        require(plain.read_bytes().startswith(b'P6\n480 320\n255\n'), 'Invalid PPM output')
        require(plain.read_bytes() == paired.read_bytes(),
                'Generating an activity report changed the rendered image')
        require((root / 'paired.html').read_bytes() == HTML,
                'Source export saved DOM mutations instead of loaded source')
        results['render_sha256'] = hashlib.sha256(plain.read_bytes()).hexdigest()
        passed('report plus source plus render is pixel-identical to render-only and preserves original HTML')

        source = root / 'source-only.html'
        run('source-only implicit profile isolation', [page, '--save-source', source,
                                                      '--wait-ms', '150'])
        require(source.read_bytes() == HTML, 'Source-only export changed the original HTML')
        unchanged()
        passed('source-only is headless and preserves original source, profile and history')

        before = len(requests)
        report, source = root / 'flow.txt', root / 'flow.html'
        run('cookie and inserted-script navigation', [origin + '/bootstrap',
             '--activity-report', report, '--save-source', source, '--wait-ms', '500'])
        text = report.read_text(encoding='utf-8')
        require(requests[before:] == ['/bootstrap', '/boot.js', '/search?q=test'],
                f'Inserted-script navigation made unexpected requests: {requests[before:]}')
        for path, cookies in request_cookies[before + 1:]:
            require('private=one' in cookies and 'visible=two' in cookies,
                    f'{path}: HTTP and script cookies were not transmitted')
        require('Page title: Actual results' in text and '0 error(s)' in text,
                'Bootstrap navigation did not reach clean results')
        require('DOCUMENT: ' + origin + '/search?q=test' in text,
                'Report does not identify the final document')
        require(source.read_bytes() == RESULT_HTML,
                'Source export does not contain the final document original HTML')
        require('private=one' not in text and 'visible=two' not in text,
                'Cookie values leaked into resource activity')
        passed('real inserted-script navigation shares cookies and exports the final document')

        before = len(requests)
        run('timer navigation while settling', [origin + '/timer', '--save-source', source,
                                               '--activity-report', report, '--wait-ms', '500'])
        require(requests[before:] == ['/timer', '/search?q=test'],
                'Timer-triggered navigation was not serviced during settling')
        require(source.read_bytes() == RESULT_HTML and
                'Page title: Actual results' in report.read_text(encoding='utf-8'),
                'Timer navigation export describes the abandoned page')
        unchanged()
        passed('timer settling services real navigation and exports its final source')

        missing = root / 'missing-page.html'
        for paired_render in (False, True):
            report = root / f'failure-{paired_render}.txt'
            image = root / 'failure.ppm'
            failed_source = root / 'failure.html'
            arguments = [missing, '--no-profile', '--activity-report', report,
                         '--save-source', failed_source]
            if paired_render:
                arguments += ['--render', image]
            run(f'failed navigation, render={paired_render}', arguments, success=False)
            text = report.read_text(encoding='utf-8')
            require(missing.name in text and 'LAST FAILED NAVIGATION:' in text,
                    'Failed navigation report does not identify the attempted file')
            require(not image.exists(), 'Failed navigation emitted a success image')
            require(not failed_source.exists(), 'Failed navigation emitted unrelated source')
        passed('navigation failures save diagnostic reports and exit unsuccessfully')

        report = root / 'explicit.txt'
        run('explicit profile', [page, '--profile-dir', explicit_profile,
                                 '--activity-report', report, '--wait-ms', '150'])
        text = report.read_text(encoding='utf-8')
        require('JavaScript enabled: false' in text and 'ACTIVITY_INLINE_MARKER' not in text,
                'Explicit profile preference was not honored')
        unchanged()
        run('session override', [page, '--profile-dir', explicit_profile, '--javascript',
                                 '--activity-report', report, '--wait-ms', '150'])
        require('ACTIVITY_TIMER_MARKER' in report.read_text(encoding='utf-8'),
                'Session JavaScript override did not run the timer')
        unchanged()
        passed('explicit profile is read without rewriting settings or history')

        run('implicit profile isolation', [page, '--activity-report', report,
                                           '--wait-ms', '150'])
        require('ACTIVITY_TIMER_MARKER' in report.read_text(encoding='utf-8'),
                'Headless report read the synthetic default profile')
        unchanged()
        passed('report-only ignores the default profile and preserves history bytes')

        target = root / 'invalid.txt'
        invalid = [
            ('missing report path', ['--activity-report'], 'requires a text file path'),
            ('duplicate report', ['--activity-report', target, '--activity-report',
                                  root / 'second.txt'], 'at most once'),
            ('connection-report conflict', ['--activity-report', target, '--diagnose',
                                            origin + '/fixture.html', '--report',
                                            root / 'connection.txt'], 'separately'),
            ('profile conflict', ['--activity-report', target, '--profile-dir',
                                  explicit_profile, '--no-profile'], 'not both'),
            ('wait out of range', ['--activity-report', target, '--wait-ms', '10001'],
             'between 0 and 10000'),
            ('detached inspector without render', ['--activity-report', target,
                                                   '--inspector', 'network',
                                                   '--inspector-window'], 'requires --render'),
            ('output path collision', ['--activity-report', target, '--render', target],
             'different'),
            ('relative output alias', ['--activity-report', target, '--render', target.name],
             'different'),
            ('missing source path', ['--save-source'], 'requires an HTML file path'),
            ('duplicate source', ['--save-source', target, '--save-source', root / 'second.html'],
             'at most once'),
            ('source report collision', ['--save-source', target, '--activity-report', target.name],
             'different'),
            ('source render collision', ['--save-source', target, '--render', target], 'different'),
            ('source diagnostic conflict', ['--save-source', target, '--diagnose',
                                            origin + '/fixture.html', '--report',
                                            root / 'connection.txt'], 'separately'),
        ]
        if os.name == 'nt':
            invalid.append(('Windows output case alias',
                            ['--activity-report', target, '--render',
                             target.with_name(target.name.upper())], 'different'))
        request_count = len(requests)
        for name, arguments, expected in invalid:
            process = run(name, arguments, success=False)
            require(expected in process.stderr, f'{name}: unhelpful error: {process.stderr}')
            require(not target.exists(), f'{name}: invalid request created an output file')
            passed(name + ' is rejected')
        require(len(requests) == request_count, 'Invalid options caused a network request')
        unchanged()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--browser', type=Path, required=True)
    parser.add_argument('--output', type=Path, default=Path('activity-report-checks.json'))
    args = parser.parse_args()
    browser = args.browser.resolve()
    require(browser.is_file(), '--browser must name a native ChezCat executable')
    result = {'tool': 'check_activity_report.py', 'platform': platform.platform(),
              'browser_sha256': hashlib.sha256(browser.read_bytes()).hexdigest(),
              'checks': [], 'runs': []}
    start = time.monotonic()
    try:
        with tempfile.TemporaryDirectory(prefix='chezcat-activity-check-') as temporary:
            exercise(browser, Path(temporary), result)
        result['passed'] = True
    except (AssertionError, OSError, subprocess.TimeoutExpired) as error:
        result['passed'] = False
        result['error'] = str(error)
    result['elapsed_seconds'] = round(time.monotonic() - start, 3)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(f"{'PASS' if result['passed'] else 'FAIL'}: {len(result['checks'])} checks, "
          f"{len(result['runs'])} browser runs; {args.output}")
    if not result['passed']:
        print(result['error'])
    return 0 if result['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
