#!/usr/bin/env python3
"""Verify area reduction through ChezCat's actual loader, layout and painter.

All source pixels are original patterns generated below. The independent
oracle integrates floating-point pixel rectangles directly in two dimensions;
it does not reuse the Rust sampler or its separable/integer implementation.
Pillow only writes the source PNGs and reads rendered PPMs. It is development
tooling, never part of the browser runtime. All browser runs use --no-profile
and --no-javascript against a temporary loopback server.

Regenerate the included original demonstration asset:
    python tools/check_image_downsampling.py --write-demo
Run validation against a freshly built executable:
    python tools/check_image_downsampling.py --browser target/release/own-browser
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import subprocess
import tempfile
import time

from PIL import Image, ImageChops

from check_image_presentation import encoded, marker_bottom, serving

ROOT = Path(__file__).resolve().parents[1]
MATTE = (50, 75, 101)


def pattern(width, height):
    image = Image.new('RGBA', (width, height))
    image.putdata([((x * 61 + y * 11) % 256, (x * 13 + y * 97) % 256,
                    (x * 71 + y * 29) % 256, (x * 47 + y * 59) % 256)
                   for y in range(height) for x in range(width)])
    return image


def demo_pattern():
    image = Image.new('RGB', (768, 288))
    navy, teal, cream = (11, 21, 60), (66, 199, 188), (255, 243, 220)
    image.putdata([
        (teal if x % 4 == 0 else navy) if y < 96 else
        (cream if y % 4 == 0 else navy) if y < 192 else
        (cream if (x + y) % 2 else teal)
        for y in range(288) for x in range(768)])
    return image


def weights(offset, source, destination):
    if destination < source:
        low, high = offset * source / destination, (offset + 1) * source / destination
        return [(index, (min(high, index + 1) - max(low, index)) / (high - low))
                for index in range(source) if min(high, index + 1) > max(low, index)]
    center = min(source - 1, max(0, (offset + 0.5) * source / destination - 0.5))
    first = math.floor(center)
    fraction = center - first
    return [(first, 1 - fraction), (min(first + 1, source - 1), fraction)]


def reference(image, size, matte):
    source = image.convert('RGBA')
    sx, sy = source.size
    width, height = size
    pixels = source.load()
    columns = [weights(x, sx, width) for x in range(width)]
    rows = [weights(y, sy, height) for y in range(height)]
    output = []
    for row in rows:
        for column in columns:
            terms = [(pixels[x, y], wx * wy) for y, wy in row for x, wx in column]
            alpha = math.fsum(pixel[3] / 255 * weight for pixel, weight in terms)
            channels = [math.fsum(pixel[c] * pixel[3] / 255 * weight for pixel, weight in terms)
                        + matte[c] * (1 - alpha) for c in range(3)]
            output.append(tuple(max(0, min(255, math.floor(value + 0.5))) for value in channels))
    result = Image.new('RGB', size)
    result.putdata(output)
    return result


def page(name, size, rendering='auto', fitting='fill', position='center'):
    width, height = size
    return (f'<!doctype html><title>ChezCat downsampling check</title>'
            '<style>body{margin:0;padding:0;background:white}'
            '.marker{height:3px;margin:0;padding:0;background:#f311a7}'
            'img{display:block;margin:0;padding:0;border:0;background:#324b65}</style>'
            '<body><div class="marker"></div>'
            f'<img src="/{name}.png" width="{width}" height="{height}" '
            f'style="image-rendering:{rendering};object-fit:{fitting};object-position:{position}">'
            '<div class="marker"></div>').encode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--browser', type=Path)
    parser.add_argument('--output', type=Path, default=Path('image-downsampling-report.json'))
    parser.add_argument('--work-dir', type=Path)
    parser.add_argument('--write-demo', action='store_true')
    args = parser.parse_args()
    if args.write_demo:
        target = ROOT / 'pages/images/area-pattern.png'
        demo_pattern().save(target)
        print(json.dumps(dict(original_asset=str(target), sha256=hashlib.sha256(target.read_bytes()).hexdigest())))
    if args.browser is None:
        assert args.write_demo, '--browser is required for validation'
        return
    browser = args.browser.resolve()
    version = subprocess.run([str(browser), '--help'], capture_output=True, text=True, check=True).stdout.splitlines()[0]
    sources = {'alpha':pattern(47, 31), 'demo':demo_pattern()}
    stripes = Image.new('RGBA', (96, 72))
    stripes.putdata([(255, 0, 0, 255) if x % 3 == 0 else (0, 0, 255, 0)
                     for y in range(72) for x in range(96)])
    sources['stripes'] = stripes
    # The full four-megapixel limit in both anisotropic directions. These
    # constant-color originals have an exact compositing oracle without a
    # second expensive resampling pass in the development checker.
    sources['wide'] = Image.new('RGBA', (16384, 256), (80, 120, 160, 128))
    sources['tall'] = Image.new('RGBA', (256, 16384), (80, 120, 160, 128))
    routes = {f'/{name}.png':('image/png', encoded(image)) for name, image in sources.items()}
    cases = []
    for mode in ('auto', 'smooth', 'high-quality'):
        cases.append((f'demo-{mode}', 'demo', (192, 72), mode, 'fill', 'center', None))
    for size in [(7, 5), (1, 1), (46, 30), (19, 67), (81, 13), (1, 90), (91, 1), (47, 31)]:
        cases.append((f'alpha-{size[0]}x{size[1]}', 'alpha', size, 'auto', 'fill', 'center', None))
    cases.append(('transparent-stripes', 'stripes', (8, 6), 'auto', 'fill', 'center', None))
    # Fit/position works on the scaled image before its object box clips it.
    covered = reference(sources['demo'], (64, 24), MATTE).crop((40, 0, 64, 24))
    cases.append(('cover-right', 'demo', (24, 24), 'smooth', 'cover', 'right top', covered))
    # Keep the explicit nearest-neighbor path distinct from smooth reduction.
    sharp = sources['demo'].resize((192, 72), Image.Resampling.NEAREST)
    cases.append(('crisp-edges', 'demo', (192, 72), 'crisp-edges', 'fill', 'center', sharp))
    for name, source, size in [('maximum-wide-mixed', 'wide', (1, 300)),
                               ('maximum-tall-mixed', 'tall', (300, 1)),
                               ('maximum-to-one', 'wide', (1, 1))]:
        cases.append((name, source, size, 'auto', 'fill', 'center', Image.new('RGB', size, (65, 98, 131))))
    for name, source, size, mode, fitting, position, _ in cases:
        routes['/' + name] = ('text/html', page(source, size, mode, fitting, position))
    checks = []
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix='chezcat-downsampling-') as temporary:
        work = args.work_dir or Path(temporary)
        work.mkdir(parents=True, exist_ok=True)
        with serving(routes) as (_, base):
            for name, source, size, mode, fitting, position, expected in cases:
                target = work / (name + '.ppm')
                command = [str(browser), base + '/' + name, '--render', str(target),
                           '--width', '800', '--height', '450', '--no-profile', '--no-javascript']
                run_started = time.monotonic()
                result = subprocess.run(command, capture_output=True, text=True, timeout=20)
                run_seconds = time.monotonic() - run_started
                assert result.returncode == 0, (name, result.stdout, result.stderr)
                image = Image.open(target).convert('RGB')
                top = marker_bottom(image)
                actual = image.crop((0, top, size[0], top + size[1]))
                expected = expected or reference(sources[source], size, MATTE)
                maximum = max(high for _, high in ImageChops.difference(actual, expected).getextrema())
                allowance = 2 if (size[0] > sources[source].width or size[1] > sources[source].height) else 1
                assert maximum <= allowance, (name, 'maximum channel difference', maximum)
                checks.append(dict(case=name, source_size=sources[source].size, displayed_size=size,
                                   rendering=mode, object_fit=fitting, position=position,
                                   maximum_channel_error=maximum, permitted_error=allowance,
                                   browser_run_seconds=round(run_seconds, 3),
                                   oracle='Pillow nearest' if mode == 'crisp-edges' else
                                   'exact constant-color alpha composite' if source in ('wide', 'tall') else
                                   'independent direct area/bilinear integration'))
    report = dict(browser_version=version, checks=checks, passed=len(checks),
                  elapsed_seconds=round(time.monotonic() - started, 3),
                  scope='Actual loopback App renders; original fixtures; no personal profile; display-space RGBA, no ICC or linear-light claim')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
