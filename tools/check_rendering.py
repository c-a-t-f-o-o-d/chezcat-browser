#!/usr/bin/env python3
"""Independent full-browser image, layout and JavaScript transport checks.

Development only. Python ssl/OpenSSL serves loopback HTTPS; Pillow independently
decodes the original JPEG fixtures for pixel comparisons. Neither is a browser
dependency. Build the browser first and pass --browser PATH.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import gzip
import json
from pathlib import Path
import ssl
import subprocess
import tempfile
import threading

from PIL import Image, ImageChops, ImageOps, ImageStat, features

from http_fixture_server import FixtureHandler, FixtureServer, PROJECT_ROOT
from https_fixture_server import FIXTURES, TlsFixtureHandler, TlsFixtureServer, context


IMAGES = ('jpeg-landscape.jpg', 'jpeg-progressive.jpg', 'jpeg-gray.jpg', 'jpeg-oriented.jpg')


class Recording:
    def log_message(self, *args):
        pass

    def route(self, send_body):
        if self.path == '/pixel-check':
            body = ("<title>JPEG transport pixels</title><style>body{margin:0}img{display:block;margin:0}</style>"
                    + ''.join(f'<img src="/compressed-pages/images/{name}">' for name in IMAGES)).encode()
            self.reply(gzip.compress(body, mtime=0), send_body,
                       extra=(("Content-Encoding", "gzip"),))
        else:
            super().route(send_body)

    def asset(self, path, send_body, compressed=False):
        super().asset(path, send_body, compressed=self.server.compressed)

    def send_fixture_headers(self, status, content_type='text/html; charset=utf-8', length=None, extra=()):
        self.server.records.append({'path': self.path, 'accept': self.headers.get('Accept-Encoding'),
                                    'coding': dict(extra).get('Content-Encoding'), 'status': status})
        super().send_fixture_headers(status, content_type, length, extra)


class Plain(Recording, FixtureHandler):
    pass


class Secure(Recording, TlsFixtureHandler):
    pass


@contextmanager
def serving(server):
    thread = threading.Thread(target=server.serve_forever, kwargs={'poll_interval': 0.05}, daemon=True)
    thread.start()
    try:
        yield server
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=3)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--browser', type=Path, required=True)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    browser = args.browser.resolve()
    if not browser.is_file():
        parser.error('--browser must name the built executable')
    results = []
    runs = 0
    with tempfile.TemporaryDirectory(prefix='chezcat-rendering-') as temporary:
        directory = Path(temporary)
        for protocol in ('http', 'https'):
            server = (FixtureServer(('127.0.0.1', 0), Plain) if protocol == 'http' else
                      TlsFixtureServer(('127.0.0.1', 0), Secure,
                                       context(FIXTURES / 'server-p256-chain.pem', FIXTURES / 'server-p256-key.pem')))
            base = f'{protocol}://localhost:{server.server_address[1]}'
            with serving(server):
                def render(path, width=1080, height=900, zoom=100, scroll=0, extra=()):
                    nonlocal runs
                    runs += 1
                    destination = directory / f'render-{runs}.ppm'
                    server.records = []
                    process = subprocess.run([str(browser), '--trust-file', str(FIXTURES / 'demo-roots.pem'),
                                              base + path, '--render', str(destination), '--width', str(width),
                                              '--height', str(height), '--zoom', str(zoom), '--scroll', str(scroll), *extra],
                                             capture_output=True, text=True, timeout=35)
                    assert process.returncode == 0, (protocol, path, process.stderr)
                    assert all(r['accept'] == 'gzip, deflate' and r['status'] == 200 for r in server.records), server.records
                    return destination

                for page in ('jpeg-demo.html', 'position-demo.html', 'compat-demo.html', 'javascript-demo.html'):
                    path = '/compressed-pages/' + page
                    expected = {path: 'gzip', '/compressed-pages/styles.css': 'gzip'}
                    if page == 'jpeg-demo.html':
                        expected.update(('/compressed-pages/images/' + name, 'deflate') for name in IMAGES)
                    if page == 'javascript-demo.html':
                        expected['/compressed-pages/scripts/javascript-demo.js'] = 'gzip'
                    for width, zoom, scroll in ((1080, 100, 550), (480, 150, 1000)):
                        pair = []
                        for compressed in (False, True):
                            server.compressed = compressed
                            image = render(path, width=width, zoom=zoom, scroll=scroll)
                            assert {r['path'] for r in server.records} == set(expected), server.records
                            assert all(r['coding'] == (expected[r['path']] if compressed else None) for r in server.records), server.records
                            pair.append(image.read_bytes())
                        assert pair[0] == pair[1], (protocol, page, width, zoom, 'encoded representation changed pixels')
                        results.append({'case': 'compressed-vs-identity', 'protocol': protocol, 'page': page,
                                        'width': width, 'zoom': zoom, 'passed': True})
                    server.compressed = True
                    render(path, extra=('--resources',))
                    assert {r['path'] for r in server.records} == set(expected), server.records
                    results.append({'case': 'resource-inspection', 'protocol': protocol, 'page': page, 'passed': True})
                    if page == 'javascript-demo.html':
                        enabled = render(path).read_bytes()
                        disabled = render(path, extra=('--no-javascript',)).read_bytes()
                        assert {r['path'] for r in server.records} == {path, '/compressed-pages/styles.css'}, server.records
                        assert enabled != disabled, (protocol, 'disabled scripts did not change demo rendering')
                        results.append({'case': 'disabled-script-not-fetched-and-fallback-visible', 'protocol': protocol, 'passed': True})

                server.compressed = True
                actual = Image.open(render('/pixel-check', height=1100)).convert('RGB')
                assert len(server.records) == 5, server.records
                assert server.records[0]['coding'] == 'gzip', server.records
                assert all(r['coding'] == 'deflate' for r in server.records[1:]), server.records
                y = 68  # ChezCat's page viewport starts below its 68px toolbar.
                for name in IMAGES:
                    with Image.open(PROJECT_ROOT / 'pages/images' / name) as encoded:
                        expected = ImageOps.exif_transpose(encoded).convert('RGB')
                    width, height = expected.size
                    pixels = actual.crop((0, y, width, y + height))
                    difference = ImageChops.difference(pixels, expected)
                    maximum = max(high for low, high in difference.getextrema())
                    mean = sum(ImageStat.Stat(difference).mean) / 3
                    bound = 1 if name == 'jpeg-gray.jpg' else 4
                    assert maximum <= bound and mean <= 0.75, (protocol, name, maximum, mean)
                    results.append({'case': 'jpeg-reference-pixels', 'protocol': protocol, 'image': name,
                                    'width': width, 'height': height, 'maximum_channel_error': maximum,
                                    'mean_channel_error': mean, 'passed': True})
                    y += height

    report = {'browser_runs': runs, 'checks': len(results), 'passed': len(results),
              'identical_comparisons': sum(r['case'] == 'compressed-vs-identity' for r in results),
              'jpeg_pixel_comparisons': sum(r['case'] == 'jpeg-reference-pixels' for r in results),
              'openssl': ssl.OPENSSL_VERSION, 'pillow': Image.__version__,
              'libjpeg_turbo': features.version_feature('libjpeg_turbo'), 'results': results}
    if args.output:
        args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items() if k != 'results'}, indent=2))


if __name__ == '__main__':
    main()
