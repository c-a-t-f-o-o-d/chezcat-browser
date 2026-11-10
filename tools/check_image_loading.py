#!/usr/bin/env python3
"""Exercise image loading/playback through the actual ChezCat executable.

Only an ephemeral 127.0.0.1 HTTP server is contacted. This checks the nine 0.28
demo assets as embedded images and MIME-selected direct image documents, request
reuse, disabled images, the resource report, and native animation with JavaScript
disabled. Pixel regions are compared with independent Pillow/ImageMagick/FFmpeg
decodes. These are development checks only, never browser dependencies.

Run: python tools/check_image_loading.py --browser target/release/own-browser
       --output image-loading-report.json --work-dir image-loading-artifacts
"""
from __future__ import annotations

import argparse
from collections import Counter
from contextlib import contextmanager
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from io import BytesIO
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import threading
import time

from PIL import Image, ImageChops, ImageStat


ROOT = Path(__file__).resolve().parents[1]
ASSETS = [
    ('format-animated.gif', 'image/gif', 0),
    ('format-animated.apng', 'image/apng', 2),
    ('format-animated.webp', 'image/webp', 2),
    ('format-lossy.webp', 'image/webp', 2),
    ('format-tiled.tiff', 'image/tiff', 1),
    ('format-cmyk.jpg', 'image/jpeg', 3),
    ('format-pattern.qoi', 'image/qoi', 1),
    ('format-alpha.pam', 'image/x-portable-arbitrarymap', 1),
    ('format-gray16.pgm', 'image/x-portable-graymap', 1),
]
WIDTH = 800
TOOLBAR = 68
STATUS = 32
MARKER = (243, 17, 167)
OFF = 'JavaScript is OFF. Ctrl+J enables it and reloads the page.'


def opaque(image):
    image = image.convert('RGBA')
    return Image.alpha_composite(Image.new('RGBA', image.size, (255, 255, 255, 255)), image).convert('RGB')


def references():
    result = {}
    converter = shutil.which('magick') or shutil.which('convert')
    ffmpeg = shutil.which('ffmpeg')
    for name, mime, tolerance in ASSETS:
        path = ROOT / 'pages/images' / name
        data = path.read_bytes()
        if name.endswith('.apng'):
            if not ffmpeg:
                raise RuntimeError('FFmpeg is required for the independent APNG reference.')
            # This fixture combines OVER with PREVIOUS disposal. Pillow leaves
            # disposed pixels behind; FFmpeg independently composes these frames
            # correctly (also checked by tools/check_animation.py).
            decoded = Image.open(BytesIO(data))
            size = decoded.width * decoded.height * 4
            raw = subprocess.run([ffmpeg, '-hide_banner', '-loglevel', 'error',
                                  '-ignore_loop', '1', '-i', str(path), '-fps_mode',
                                  'passthrough', '-f', 'rawvideo', '-pix_fmt', 'rgba', '-'],
                                 capture_output=True, check=True, timeout=15).stdout
            assert len(raw) == size * decoded.n_frames
            frames = [opaque(Image.frombytes('RGBA', decoded.size, raw[at:at + size]))
                      for at in range(0, len(raw), size)]
            oracle = 'FFmpeg'
        elif name.endswith('.pam'):
            if not converter:
                raise RuntimeError('ImageMagick is required for the independent PAM reference.')
            width = int(re.search(rb'^WIDTH\s+(\d+)', data, re.M)[1])
            height = int(re.search(rb'^HEIGHT\s+(\d+)', data, re.M)[1])
            raw = subprocess.run([converter, str(path), '-depth', '8', 'rgba:-'],
                                 capture_output=True, check=True, timeout=15).stdout
            assert len(raw) == width * height * 4
            frames = [opaque(Image.frombytes('RGBA', (width, height), raw))]
            oracle = 'ImageMagick'
        else:
            decoded = Image.open(BytesIO(data))
            if name.endswith('.pgm'):
                # The original gray16 demo has MAXVAL 65535. Pillow exposes
                # those decoded integer samples without an 8-bit display scale.
                gray = Image.new('L', decoded.size)
                gray.putdata([(value * 255 + 32767) // 65535
                              for value in decoded.get_flattened_data()])
                frames = [gray.convert('RGB')]
            else:
                frames = []
                for index in range(getattr(decoded, 'n_frames', 1)):
                    decoded.seek(index)
                    frames.append(opaque(decoded))
            oracle = 'Pillow'
        result[name] = {'data': data, 'mime': mime, 'tolerance': tolerance,
                        'frames': frames, 'oracle': oracle,
                        'sha256': hashlib.sha256(data).hexdigest()}
    return result


def comparison(actual, expected):
    assert actual.size == expected.size, (actual.size, expected.size)
    difference = ImageChops.difference(actual, expected)
    maximum = max(high for _, high in difference.getextrema())
    mean = sum(ImageStat.Stat(difference).mean) / 3
    return maximum, mean


def match_frame(actual, asset):
    choices = [(comparison(actual, frame), index) for index, frame in enumerate(asset['frames'])]
    (maximum, mean), index = min(choices)
    assert maximum <= asset['tolerance'] and mean <= 0.8, (maximum, mean, asset['tolerance'])
    return {'frame': index, 'maximum_channel_error': maximum, 'mean_channel_error': mean}


def marker_bottom(image):
    pixels = image.load()
    rows = [y for y in range(TOOLBAR, min(180, image.height))
            if all(pixels[x, y] == MARKER for x in (0, 17, WIDTH // 2, WIDTH - 20))]
    assert len(rows) == 3 and rows == list(range(rows[0], rows[0] + 3)), rows
    return rows[-1] + 1


def locate_direct(image, asset):
    """Find the unscaled viewer picture, independently of header font metrics."""
    width, height = asset['frames'][0].size
    center = (image.width - width) // 2
    anchors = [(x * (width - 1) // 3, y * (height - 1) // 3)
               for y in range(4) for x in range(4)]
    pixels = image.load()
    candidates = []
    for frame in asset['frames']:
        expected = [frame.getpixel(point) for point in anchors]
        scores = []
        for y in range(TOOLBAR + 8, min(330, image.height - STATUS - height) + 1):
            for x in range(max(0, center - 12), min(image.width - width, center + 12) + 1):
                score = sum(abs(a - b) for (dx, dy), rgb in zip(anchors, expected)
                            for a, b in zip(pixels[x + dx, y + dy], rgb))
                scores.append((score, x, y))
        for _, x, y in sorted(scores)[:3]:
            box = (x, y, x + width, y + height)
            candidates.append((comparison(image.crop(box), frame), box))
    (maximum, mean), box = min(candidates)
    assert maximum <= asset['tolerance'] and mean <= 0.8, ('direct viewer picture not found', maximum, mean)
    return box


@contextmanager
def serving(assets):
    markup = ('<!doctype html><title>Image transport checks</title>'
              '<style>body{margin:0;background:#fff}img{display:block;margin:0;border:0;background:#fff}</style>'
              '<script src="/must-not-run.js"></script><body>'
              '<div style="height:3px;background:#f311a7"></div>')
    for name, _, _ in ASSETS:
        width, height = assets[name]['frames'][0].size
        markup += (f'<img src="/asset/{name}" width="{width}" height="{height}" alt="disabled">') * 2
    markup = markup.encode()
    control = ('<style>body{margin:24px}pre{font-size:14px;white-space:pre-wrap}</style>'
               f'<body><pre>{OFF}</pre></body>').encode()

    class Handler(BaseHTTPRequestHandler):
        protocol_version = 'HTTP/1.1'

        def log_message(self, *_):
            pass

        def do_GET(self):
            status, mime, data = 200, 'text/html; charset=utf-8', markup
            if self.path == '/all':
                pass
            elif self.path == '/off-reference':
                data = control
            elif self.path == '/must-not-run.js':
                mime, data = 'text/javascript', b'throw new Error("Unexpected script execution");'
            else:
                name = self.path.removeprefix('/asset/') if self.path.startswith('/asset/') else None
                if self.path.startswith('/direct/'):
                    try:
                        name = ASSETS[int(self.path.removeprefix('/direct/'))][0]
                    except (ValueError, IndexError):
                        name = None
                if name in assets:
                    mime, data = assets[name]['mime'], assets[name]['data']
                else:
                    status, mime, data = 404, 'text/plain', b'Not found'
            with self.server.lock:
                self.server.records.append({'path': self.path, 'status': status,
                                            'mime': mime, 'bytes': len(data)})
            self.send_response(status)
            self.send_header('Content-Type', mime)
            self.send_header('Content-Length', str(len(data)))
            self.send_header('Connection', 'close')
            self.end_headers()
            self.wfile.write(data)
            self.close_connection = True

    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    server.daemon_threads = True
    server.records, server.lock = [], threading.Lock()
    thread = threading.Thread(target=server.serve_forever, kwargs={'poll_interval': 0.05}, daemon=True)
    thread.start()
    try:
        yield server, f'http://127.0.0.1:{server.server_address[1]}'
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=3)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--browser', type=Path, required=True)
    parser.add_argument('--output', type=Path, default=Path('image-loading-report.json'))
    parser.add_argument('--work-dir', type=Path)
    args = parser.parse_args()
    browser = args.browser.resolve()
    if not browser.is_file():
        parser.error('--browser must name the built executable')
    assets = references()
    records, results = [], []
    started = time.monotonic()
    page_height = TOOLBAR + STATUS + 8 + sum(asset['frames'][0].height * 2 for asset in assets.values())
    expected = Counter(['/all'] + ['/asset/' + name for name, _, _ in ASSETS])
    with tempfile.TemporaryDirectory(prefix='chezcat-image-loading-') as temporary:
        work = args.work_dir or Path(temporary)
        work.mkdir(parents=True, exist_ok=True)
        with serving(assets) as (server, base):
            def render(label, path, *, height=480, wait=0, extra=(), requests=None):
                with server.lock:
                    server.records.clear()
                target = work / (label + '.ppm')
                command = [str(browser), base + path, '--render', str(target), '--width', str(WIDTH),
                           '--height', str(height), '--no-profile', '--no-javascript', '--wait-ms', str(wait), *extra]
                process = subprocess.run(command, capture_output=True, text=True, timeout=30)
                assert process.returncode == 0, (label, process.stdout, process.stderr)
                with server.lock:
                    fetched = list(server.records)
                assert all(item['status'] == 200 for item in fetched), (label, fetched)
                assert not any(item['path'] == '/must-not-run.js' for item in fetched), label
                if requests is not None:
                    assert Counter(item['path'] for item in fetched) == requests, (label, fetched)
                image = Image.open(target).convert('RGB')
                assert image.size == (WIDTH, height), (label, image.size)
                image.save(work / (label + '.png'))
                target.unlink()
                records.append({'run': label, 'wait_ms': wait, 'requests': fetched,
                                'render_sha256': hashlib.sha256(image.tobytes()).hexdigest()})
                return image

            embedded = []
            for wait in (0, 250):
                rendered = render(f'embedded-{wait}', '/all', height=page_height, wait=wait, requests=expected)
                y = marker_bottom(rendered)
                crops = {}
                for name, _, _ in ASSETS:
                    asset = assets[name]
                    width, height = asset['frames'][0].size
                    first = rendered.crop((0, y, width, y + height))
                    second = rendered.crop((0, y + height, width, y + 2 * height))
                    assert first.tobytes() == second.tobytes(), (name, 'repeated references differ')
                    result = match_frame(first, asset)
                    results.append({'case': 'embedded-reference-pixels-and-one-fetch', 'image': name,
                                    'wait_ms': wait, 'oracle': asset['oracle'], **result})
                    crops[name] = first
                    y += 2 * height
                embedded.append((rendered, crops))
            for name, _, _ in ASSETS:
                first, later = embedded[0][1][name], embedded[1][1][name]
                delta = ImageChops.difference(first, later)
                changed = sum(pixel != (0, 0, 0) for pixel in delta.get_flattened_data())
                if len(assets[name]['frames']) > 1:
                    assert changed > 100, (name, 'HTTP-embedded animation did not advance', changed)
                    results.append({'case': 'embedded-animation-without-javascript', 'image': name,
                                    'changed_image_pixels': changed})
                else:
                    assert changed == 0, (name, 'static image changed after waiting')

            disabled = render('images-disabled', '/all', height=page_height,
                              extra=('--no-images',), requests=Counter(['/all']))
            marker_bottom(disabled)
            assert comparison(disabled.crop((0, TOOLBAR, WIDTH, page_height - STATUS)),
                              embedded[0][0].crop((0, TOOLBAR, WIDTH, page_height - STATUS)))[1] > 5
            results.append({'case': 'images-disabled', 'image_fetches': 0, 'javascript_fetches': 0})

            # The CLI resource view is visual. Compare its actual OFF line with
            # the same renderer's ordinary text control, instead of guessing
            # success from an exit code or depending on an OCR installation.
            control = render('off-text-reference', '/off-reference', requests=Counter(['/off-reference']))
            region = control.crop((0, TOOLBAR, WIDTH, control.height - STATUS))
            box = ImageChops.difference(region, Image.new('RGB', region.size, 'white')).getbbox()
            assert box is not None
            text = region.crop(box)
            resource = render('resource-report', '/all', height=840,
                              extra=('--resources', '--scroll', '8192'), requests=expected)
            matched = any(resource.crop((box[0], y, box[0] + text.width, y + text.height)).tobytes() == text.tobytes()
                          for y in range(TOOLBAR, resource.height - STATUS - text.height + 1))
            assert matched, 'The rendered resource report does not show the JavaScript OFF control text.'
            results.append({'case': 'resource-report', 'javascript_off_line_visible': True,
                            'javascript_fetches': 0, 'image_fetches': len(ASSETS)})

            for index, (name, mime, _) in enumerate(ASSETS):
                path = f'/direct/{index}'  # No extension: Content-Type selects the viewer.
                asset = assets[name]
                direct = render(f'direct-{index}-0', path, requests=Counter([path]))
                box = locate_direct(direct, asset)
                first = direct.crop(box)
                result = match_frame(first, asset)
                assert records[-1]['requests'][0]['mime'] == mime
                results.append({'case': 'direct-image-document', 'image': name, 'mime': mime,
                                'image_rectangle': box, 'network_fetches': 1, **result})
                if len(asset['frames']) > 1:
                    later = render(f'direct-{index}-250', path, wait=250, requests=Counter([path])).crop(box)
                    result = match_frame(later, asset)
                    changed = sum(pixel != (0, 0, 0) for pixel in ImageChops.difference(first, later).get_flattened_data())
                    assert changed > 100, (name, 'direct image animation did not advance', changed)
                    results.append({'case': 'direct-animation-without-javascript', 'image': name,
                                    'changed_image_pixels': changed, 'network_fetches': 1, **result})

    report = {'browser_runs': len(records), 'checks': len(results), 'passed': len(results),
              'loopback_only': True, 'javascript_enabled': False,
              'elapsed_seconds': round(time.monotonic() - started, 3),
              'browser_sha256': hashlib.sha256(browser.read_bytes()).hexdigest(),
              'assets': {name: {'sha256': asset['sha256'], 'mime': asset['mime'],
                                'reference_frames': len(asset['frames'])} for name, asset in assets.items()},
              'checks_detail': results, 'runs': records}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: v for k, v in report.items() if k not in ('assets', 'checks_detail', 'runs')}, indent=2))


if __name__ == '__main__':
    main()
