#!/usr/bin/env python3
"""Check responsive image selection and presentation through the real App.

Runs isolated headless ChezCat instances against an ephemeral loopback server.
Sources are authored geometric images. Pillow supplies independent PNG/TGA/PCX
encoding/decoding and simple crop/scale references, never a runtime dependency.
No public site is visited, no personal profile is opened, no JavaScript runs.
"""
from __future__ import annotations

import argparse
import base64
from collections import Counter
from contextlib import contextmanager
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from io import BytesIO
import json
from pathlib import Path
import subprocess
import tempfile
import threading
import time
from urllib.parse import quote_from_bytes

from PIL import Image, ImageChops
from check_windows_pe import package_version

ROOT = Path(__file__).resolve().parents[1]
TOOLBAR, STATUS = 102, 32
MARKER = (243, 17, 167)
MATTE = (25, 45, 63)
RED, GREEN, BLUE, PURPLE = (221, 31, 53), (37, 199, 83), (41, 71, 223), (113, 29, 197)


def encoded(image, format='PNG'):
    out = BytesIO()
    image.save(out, format=format)
    return out.getvalue()


def page(content):
    return ('<!doctype html><title>ChezCat image presentation check</title>'
            '<style>body{margin:0;padding:0;background:#fff}'
            'img{display:block;margin:0;padding:0;border:0;background:#192d3f;image-rendering:crisp-edges}'
            '.marker{height:3px;margin:0;background:#f311a7}</style>'
            '<script src="/must-not-run.js"></script><body>'
            '<div class="marker"></div>' + content).encode()


def marker_bottom(image):
    pixels = image.load()
    rows = [y for y in range(TOOLBAR, min(130, image.height))
            if all(pixels[x, y] == MARKER for x in (0, 17, image.width // 2, image.width - 20))]
    assert rows and rows[:3] == list(range(rows[0], rows[0] + 3)), rows
    end = rows[0] + 1
    while end in rows:
        end += 1
    return end


def exact(actual, expected, label):
    assert actual.size == expected.size, (label, actual.size, expected.size)
    extrema = ImageChops.difference(actual, expected).getextrema()
    maximum = max(high for _, high in extrema)
    assert maximum == 0, (label, 'maximum channel difference', maximum)


@contextmanager
def serving(routes):
    class Handler(BaseHTTPRequestHandler):
        protocol_version = 'HTTP/1.1'

        def log_message(self, *_):
            pass

        def do_GET(self):
            route = routes.get(self.path)
            mime, data = route if route else ('text/plain', b'Not found')
            status = 200 if route else 404
            with self.server.lock:
                self.server.records.append(dict(path=self.path, status=status, mime=mime, bytes=len(data)))
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
    thread = threading.Thread(target=server.serve_forever, kwargs={'poll_interval':0.02}, daemon=True)
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
    parser.add_argument('--work-dir', type=Path)
    parser.add_argument('--output', type=Path, default=Path('results/verification/image-presentation-report.json'))
    args = parser.parse_args()
    browser = args.browser.resolve()
    assert browser.is_file(), '--browser must name the built executable'
    version = subprocess.run([str(browser), '--help'], capture_output=True, text=True, check=True).stdout.splitlines()[0]
    expected = 'ChezCat ' + package_version()[0]
    assert version == expected, ('Expected current source version', expected, version)
    started = time.monotonic()
    routes, checks, runs = {}, [], []
    assets = {
        '/red.png':Image.new('RGB', (160, 80), RED),
        '/green.png':Image.new('RGB', (320, 160), GREEN),
        '/blue.png':Image.new('RGB', (640, 320), BLUE),
        '/small.png':Image.new('RGB', (20, 10), PURPLE),
    }
    pattern = Image.new('RGB', (80, 40))
    pattern.putdata([((x // 10 * 29 + 17) & 255, (y // 10 * 59 + 41) & 255,
                      (x // 10 * 31 + y // 10 * 37 + 53) & 255)
                     for y in range(40) for x in range(80)])
    assets['/pattern.png'] = pattern
    for path, image in assets.items():
        routes[path] = ('image/png', encoded(image))
    for path in ('/unused.png', '/unused.avif', '/after-img.png', '/unknown-media.png'):
        routes[path] = ('image/png', encoded(Image.new('RGB', (5, 5), 'yellow')))
    routes['/must-not-run.js'] = ('text/javascript', b'throw new Error("Unexpected script execution");')
    picture = ('<picture><source type="image/avif" srcset="/unused.avif">'
               '<source type="image/png" media="(min-width:900px)" srcset="/green.png 1x">'
               '<source type="image/png" media="(unknown-condition:1)" srcset="/unknown-media.png">'
               '<source type="image/png" srcset="/red.png 1x, /blue.png 2x">'
               '<img src="/unused.png"><source srcset="/after-img.png"></picture>')
    routes['/picture'] = ('text/html', page(picture))
    routes['/density'] = ('text/html', page('<img srcset="/green.png 2x">'))
    routes['/zoom'] = ('text/html', page('<img src="/unused.png" srcset="/red.png 1x, /green.png 2x">'))
    routes['/width-set'] = ('text/html', page('<img src="/unused.png" srcset="/red.png 160w, /green.png 320w, /blue.png 640w" sizes="50vw">'))
    routes['/sizes-media'] = ('text/html', page('<img srcset="/red.png 160w, /green.png 320w, /blue.png 640w" sizes="(min-width:900px) 480px, 160px">'))
    routes['/sizes-calc'] = ('text/html', page('<img srcset="/red.png 160w, /green.png 320w, /blue.png 640w" sizes="calc(50vw - 80px)">'))
    data_bytes = routes['/small.png'][1]
    base64_uri = 'data:image/png;base64,' + base64.b64encode(data_bytes).decode()
    percent_uri = 'data:image/png,' + quote_from_bytes(data_bytes, safe='')
    data_markup = ''.join(f'<img src="{uri}">' for uri in (base64_uri, base64_uri + '#same', percent_uri, percent_uri))
    data_markup += f'<picture><source type="image/png" srcset="{base64_uri} 2x"><img src="/unused.png"></picture>'
    routes['/embedded-data'] = ('text/html', page(data_markup))

    # References use fixed, integer-aligned transformations and independent
    # Pillow nearest-neighbor resizing, so no browser rasterization is copied.
    fit_cases = [
        ('fill', 'center', '/pattern.png', (40, 40), (0, 0)),
        ('fill', '10px 5px', '/pattern.png', (40, 40), (10, 5)),
        ('contain', 'center', '/pattern.png', (40, 20), (0, 10)),
        ('contain', 'right bottom', '/pattern.png', (40, 20), (0, 20)),
        ('contain', 'right 0px bottom 5px', '/pattern.png', (40, 20), (0, 15)),
        ('cover', 'center', '/pattern.png', (80, 40), (-20, 0)),
        ('cover', 'left top', '/pattern.png', (80, 40), (0, 0)),
        ('cover', 'right bottom', '/pattern.png', (80, 40), (-40, 0)),
        ('none', 'center', '/pattern.png', (80, 40), (-20, 0)),
        ('scale-down', 'center', '/pattern.png', (40, 20), (0, 10)),
        ('scale-down', 'left top', '/small.png', (20, 10), (0, 0)),
    ]
    fitting = ''
    for fit, position, source, _, _ in fit_cases:
        fitting += f'<img src="{source}" width="40" height="40" style="object-fit:{fit};object-position:{position}"><div class="marker"></div>'
    fitting += '<img srcset="/pattern.png 2x" width="40" height="40" style="object-fit:none;object-position:center"><div class="marker"></div>'
    fitting += ('<input type="image" src="/pattern.png" width="40" height="40" '
                'style="display:block;width:40px;height:40px;margin:0;padding:0;border:0;'
                'object-fit:cover;object-position:right bottom"><div class="marker"></div>')
    routes['/fitting'] = ('text/html', page(fitting))
    # Geometry/decoder fixtures above explicitly preserve sharp pixels. These
    # separate cases exercise the default smooth path against Pillow's
    # independent bilinear enlargement, including the CSS property plumbing.
    smooth_modes = ('auto', 'smooth', 'high-quality')
    routes['/smooth'] = ('text/html', page(''.join(
        f'<img src="/pattern.png" width="160" height="80" style="image-rendering:{mode}"><div class="marker"></div>'
        for mode in smooth_modes)))

    # Both extensionless direct image documents and image elements must route
    # the new formats by Content-Type, without an extra image request.
    legacy = {}
    for name, mime in [('format-alpha.tga', 'image/x-tga'), ('format-planar.pcx', 'image/x-pcx')]:
        data = (ROOT / 'pages/images' / name).read_bytes()
        rgba = Image.open(BytesIO(data)).convert('RGBA')
        reference = Image.alpha_composite(Image.new('RGBA', rgba.size, 'white'), rgba).convert('RGB')
        path = '/direct-' + name.split('.')[-1]
        routes[path] = (mime, data)
        legacy[path] = reference
    legacy_markup = ''.join(f'<img src="{path}" width="240" height="144" style="background:white">' * 2 for path in legacy)
    routes['/legacy'] = ('text/html', page(legacy_markup))
    disabled_markup = picture + data_markup + legacy_markup
    routes['/disabled'] = ('text/html', page(disabled_markup))

    with tempfile.TemporaryDirectory(prefix='chezcat-image-presentation-') as temporary:
        work = args.work_dir or Path(temporary)
        work.mkdir(parents=True, exist_ok=True)
        with serving(routes) as (server, base):
            def render(label, path, expected, width=800, height=650, extra=()):
                with server.lock:
                    server.records.clear()
                target = work / (label + '.ppm')
                command = [str(browser), base + path, '--render', str(target), '--width', str(width),
                           '--height', str(height), '--no-profile', '--no-javascript', *extra]
                process = subprocess.run(command, capture_output=True, text=True, timeout=20)
                assert process.returncode == 0, (label, process.stdout, process.stderr)
                with server.lock:
                    fetched = list(server.records)
                assert Counter(item['path'] for item in fetched) == Counter([path, *expected]), (label, fetched, expected)
                assert all(item['status'] == 200 for item in fetched), (label, fetched)
                image = Image.open(target).convert('RGB')
                assert image.size == (width, height)
                image.save(work / (label + '.png'))
                target.unlink()
                runs.append(dict(name=label, requests=fetched, render_sha256=hashlib.sha256(image.tobytes()).hexdigest()))
                return image

            def one(label, path, selected, color, size, width=800, extra=()):
                image = render(label, path, [selected], width=width, extra=extra)
                y = marker_bottom(image)
                # Read the full-width block independently: the application's
                # content viewport reserves scrollbar space inside the window.
                viewport = sum(image.getpixel((x, y - 1)) == MARKER for x in range(image.width))
                if size == 'half-viewport':
                    size = (viewport // 2, viewport // 4)
                elif size == 'half-minus-80':
                    size = (viewport // 2 - 80, viewport // 4 - 40)
                expected = Image.new('RGB', (size[0] + 4, size[1] + 4), 'white')
                expected.paste(Image.new('RGB', size, color), (0, 0))
                exact(image.crop((0, y, expected.width, y + expected.height)), expected, label)
                checks.append(dict(case=label, selected=selected, natural_display_size=size, rendered_viewport_width=viewport,
                                   image_http_requests=1, maximum_channel_error=0))

            one('picture-media-small', '/picture', '/red.png', RED, (160, 80), width=800)
            one('picture-media-large', '/picture', '/green.png', GREEN, (320, 160), width=960)
            one('density-corrected-2x', '/density', '/green.png', GREEN, (160, 80))
            one('zoom-selects-2x', '/zoom', '/green.png', GREEN, (320, 160), extra=('--zoom', '200'))
            one('width-srcset-640', '/width-set', '/green.png', GREEN, 'half-viewport', width=640)
            one('width-srcset-960', '/width-set', '/blue.png', BLUE, 'half-viewport', width=960)
            one('sizes-media-small', '/sizes-media', '/red.png', RED, (160, 80), width=800)
            one('sizes-media-large', '/sizes-media', '/blue.png', BLUE, (480, 240), width=960)
            one('sizes-calc', '/sizes-calc', '/green.png', GREEN, 'half-minus-80', width=800)

            embedded = render('embedded-data', '/embedded-data', [])
            y = marker_bottom(embedded)
            for index, size in enumerate([(20, 10)] * 4 + [(10, 5)]):
                expected = Image.new('RGB', (24, size[1]), 'white')
                expected.paste(Image.new('RGB', size, PURPLE), (0, 0))
                exact(embedded.crop((0, y, 24, y + size[1])), expected, 'embedded data ' + str(index))
                y += size[1]
            checks.append(dict(case='embedded-base64-percent-and-repeated-references', image_count=5,
                               image_http_requests=0, maximum_channel_error=0))

            fitted = render('object-fitting', '/fitting', ['/pattern.png', '/small.png'], height=800)
            y = marker_bottom(fitted)
            for fit, position, source, size, offset in fit_cases:
                expected = Image.new('RGB', (40, 40), MATTE)
                resized = assets[source].resize(size, Image.Resampling.NEAREST)
                expected.paste(resized, offset)
                surround = Image.new('RGB', (90, 40), 'white')
                surround.paste(expected, (0, 0))
                label = f'{fit} at {position} from {source}'
                exact(fitted.crop((0, y, 90, y + 40)), surround, label)
                exact(fitted.crop((0, y + 40, 90, y + 43)), Image.new('RGB', (90, 3), MARKER), label + ' clipping')
                checks.append(dict(case='object-fit', fit=fit, position=position, maximum_channel_error=0))
                y += 43
            expected = Image.new('RGB', (40, 40), MATTE)
            expected.paste(pattern.resize((40, 20), Image.Resampling.NEAREST), (0, 10))
            exact(fitted.crop((0, y, 40, y + 40)), expected, 'object-fit none uses density-corrected dimensions')
            checks.append(dict(case='object-fit-none-density-corrected', maximum_channel_error=0))
            y += 43
            exact(fitted.crop((0, y, 40, y + 40)), pattern.crop((40, 0, 80, 40)), 'image submit uses object-fit and position')
            checks.append(dict(case='image-submit-object-fit-cover-and-position', maximum_channel_error=0))

            smoothed = render('smooth-image-rendering', '/smooth', ['/pattern.png'], height=650)
            y = marker_bottom(smoothed)
            reference = pattern.resize((160, 80), Image.Resampling.BILINEAR)
            for mode in smooth_modes:
                actual = smoothed.crop((0, y, 160, y + 80))
                maximum = max(high for _, high in ImageChops.difference(actual, reference).getextrema())
                assert maximum <= 2, ('bilinear enlargement', mode, maximum)
                checks.append(dict(case='smooth-image-rendering', value=mode,
                                   maximum_channel_error=maximum, oracle='Pillow bilinear'))
                y += 83

            image = render('legacy-embedded', '/legacy', list(legacy), height=800)
            y = marker_bottom(image)
            for path, reference in legacy.items():
                for _ in range(2):
                    exact(image.crop((0, y, 240, y + 144)), reference, path)
                    y += 144
                checks.append(dict(case='legacy-embedded-repeat', source=path, image_http_requests=1,
                                   maximum_channel_error=0, oracle='Pillow'))
            for path, reference in legacy.items():
                image = render(path[1:], path, [])
                # The viewer centers an unscaled image. Locate by a small
                # anchor set, then require equality of every pixel.
                anchors = [(x, y) for x in (20, 80, 160, 220) for y in (20, 60, 100, 130)]
                matches = []
                for top in range(TOOLBAR + 8, 330):
                    for left in range(268, 293):
                        if all(image.getpixel((left + x, top + y)) == reference.getpixel((x, y)) for x, y in anchors):
                            matches.append((left, top))
                assert matches, ('Direct image pixels absent', path)
                boxes = [(left, top, left + 240, top + 144) for left, top in matches]
                good = [box for box in boxes if ImageChops.difference(image.crop(box), reference).getbbox() is None]
                assert good, ('Direct image full pixel mismatch', path)
                checks.append(dict(case='legacy-direct-MIME', source=path, image_rectangle=good[0],
                                   total_http_requests=1, maximum_channel_error=0, oracle='Pillow'))

            disabled = render('images-disabled', '/disabled', [], height=1100, extra=('--no-images',))
            region = disabled.crop((0, TOOLBAR, disabled.width, disabled.height - STATUS))
            colors = set(getattr(region, 'get_flattened_data', region.getdata)())
            assert not colors.intersection({RED, GREEN, BLUE, PURPLE, (66, 199, 188), (255, 194, 86)}), 'Disabled images were painted'
            checks.append(dict(case='images-disabled-including-data-URIs', image_http_requests=0,
                               source_image_colors_painted=0))

    report = dict(passed=True, browser_runs=len(runs), checks=len(checks), loopback_only=True,
                  javascript_enabled=False, profile_access=False, browser_version=version,
                  browser_sha256=hashlib.sha256(browser.read_bytes()).hexdigest(),
                  elapsed_seconds=round(time.monotonic() - started, 3), checks_detail=checks, runs=runs)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n', encoding="utf-8")
    print(json.dumps({k:v for k,v in report.items() if k not in ('checks_detail', 'runs')}, indent=2))


if __name__ == '__main__':
    main()
