#!/usr/bin/env python3
"""Check the original TGA/PCX readers using authored pixels and Pillow.

Development only: no Python or external image decoder ships in the browser.
Every input is generated here from geometric/numeric patterns, not downloaded.
Format references: Truevision TGA File Format Specification 2.0 (1991), and
ZSoft PCX Technical Reference Manual revision 5, format sections only.
"""
from __future__ import annotations

import argparse
from io import BytesIO
import json
import os
from pathlib import Path
import random
import shutil
import struct
import subprocess
import tempfile

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
TGA_END = b'TRUEVISION-XFILE.\0'
PROBE = r'''
#![allow(dead_code)]
#[path=IMAGE] mod image;
#[path=JPEG] mod jpeg;
#[path=COMPRESSION] mod compression;
use std::cell::Cell;
use std::io::Write;
fn main() {
    let args:Vec<_>=std::env::args().collect();
    let bytes=std::fs::read(&args[1]).unwrap();
    let stop=args.get(3).and_then(|s|s.parse::<usize>().ok()).unwrap_or(0);
    let calls=Cell::new(0usize);
    let check=|| { calls.set(calls.get()+1);
        if stop>0 && calls.get()>=stop { Err("cancelled".into()) } else { Ok(()) } };
    match image::decode_asset_checked(&bytes,&check) {
        Ok(asset)=>{
            assert!(asset.animation.is_none());
            let b=asset.bitmap;
            assert_eq!(b.width*b.height,b.pixels.len());
            let mut out=std::fs::File::create(&args[2]).unwrap();
            out.write_all(&(b.width as u32).to_le_bytes()).unwrap();
            out.write_all(&(b.height as u32).to_le_bytes()).unwrap();
            for p in &b.pixels {out.write_all(&p.to_le_bytes()).unwrap();}
            println!("OK {}",calls.get());
        },
        Err(error)=>{println!("ERR {} {}",calls.get(),error);std::process::exit(2);}
    }
}
'''


def tga_rle(samples):
    """Independent fixture encoder: alternate repeat and literal packets."""
    result = bytearray()
    at = 0
    while at < len(samples):
        stop = at + 1
        while stop < len(samples) and samples[stop] == samples[at] and stop - at < 128:
            stop += 1
        if stop - at > 1:
            result.append(0x80 | (stop - at - 1))
            result.extend(samples[at])
        else:
            while stop < len(samples) and stop - at < 128:
                if stop + 1 < len(samples) and samples[stop] == samples[stop + 1]:
                    break
                stop += 1
            result.append(stop - at - 1)
            result.extend(b''.join(samples[at:stop]))
        at = stop
    return bytes(result)


def tga_file(width, height, samples, depth, *, kind=2, origin=0x20, attributes=0,
             palette=None, first=0, palette_depth=24, rle=False, footer=True,
             alpha_type=None, identifier=b''):
    header = bytearray(18)
    header[0:3] = bytes([len(identifier), int(palette is not None), kind + (8 if rle else 0)])
    if palette is not None:
        struct.pack_into('<HHB', header, 3, first, len(palette), palette_depth)
    struct.pack_into('<HHBB', header, 12, width, height, depth, origin | attributes)
    # Input samples are in normal top-left presentation order.
    stored = [samples[(height - 1 - y if not origin & 0x20 else y) * width +
                      (width - 1 - x if origin & 0x10 else x)]
              for y in range(height) for x in range(width)]
    body = header + identifier + b''.join(palette or [])
    body.extend(b''.join(tga_rle(stored[y * width:(y + 1) * width]) for y in range(height))
                if rle else b''.join(stored))
    extension = 0
    if alpha_type is not None:
        assert footer
        extension = len(body)
        area = bytearray(495)
        struct.pack_into('<H', area, 0, 495)
        area[494] = alpha_type
        body.extend(area)
    if footer:
        body.extend(struct.pack('<II', extension, 0) + TGA_END)
    return bytes(body)


def pcx_rle(row):
    result = bytearray()
    at = 0
    while at < len(row):
        stop = at + 1
        while stop < len(row) and stop - at < 63 and row[stop] == row[at]:
            stop += 1
        count = stop - at
        if count > 1 or row[at] >= 192:
            result.extend((0xc0 | count, row[at]))
        else:
            result.append(row[at])
        at = stop
    return bytes(result)


def pcx_file(width, height, samples, *, bits=8, planes=3, rle=True,
             palette=None, padding=0, gray=False, version=5):
    line = ((width * bits + 7) // 8 + 1) & ~1
    line += padding
    header = bytearray(128)
    header[:4] = bytes([10, version, int(rle), bits])
    struct.pack_into('<HHHH', header, 4, 5, 9, width + 4, height + 8)
    struct.pack_into('<HH', header, 12, 96, 96)
    palette = palette or [(n, n, n) for n in range(256)]
    for n, color in enumerate(palette[:16]):
        header[16 + 3 * n:19 + 3 * n] = bytes(color)
    header[65] = planes
    struct.pack_into('<HH', header, 66, line, 2 if gray else 1)
    body = bytearray(header)
    for y in range(height):
        row = bytearray([0xa5] * (line * planes))
        for p in range(planes):
            if bits < 8:
                row[p * line:p * line + (width * bits + 7) // 8] = bytes((width * bits + 7) // 8)
            for x in range(width):
                sample = samples[y * width + x]
                if bits == 8:
                    row[p * line + x] = sample[p] if planes == 3 else sample
                else:
                    value = (sample >> (p * bits)) & ((1 << bits) - 1)
                    row[p * line + x * bits // 8] |= value << (8 - bits - x * bits % 8)
        body.extend(pcx_rle(row) if rle else row)
    if bits == 8 and planes == 1 and not gray:
        body.append(12)
        body.extend(b''.join(bytes(p) for p in palette))
    return bytes(body)


def rgba_bytes(samples):
    return b''.join(bytes(p if len(p) == 4 else (*p, 255)) for p in samples)


def write_demos(folder):
    width, height = 240, 144
    rgba = []
    rgb = []
    for y in range(height):
        for x in range(width):
            inside = ((x - 120) / 108) ** 2 + ((y - 72) / 62) ** 2 < 1
            color = (66, 199, 188) if (x // 24 + y // 24) % 2 else (255, 194, 86)
            rgba.append((*color, 255 if inside else 0))
            rgb.append(color if inside else (23, 47, 62))
    folder.mkdir(parents=True, exist_ok=True)
    samples = [bytes((b, g, r, a)) for r, g, b, a in rgba]
    (folder / 'format-alpha.tga').write_bytes(tga_file(width, height, samples, 32,
                                                     attributes=8, rle=True, alpha_type=3))
    (folder / 'format-planar.pcx').write_bytes(pcx_file(width, height, rgb))
    return [('format-alpha.tga', rgba_bytes(rgba)), ('format-planar.pcx', rgba_bytes(rgb))]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--work-dir', type=Path)
    parser.add_argument('--write-demos', action='store_true')
    args = parser.parse_args()
    work = args.work_dir or Path(tempfile.mkdtemp(prefix='chezcat-legacy-images-'))
    work.mkdir(parents=True, exist_ok=True)
    source = PROBE
    for key, name in [('IMAGE', 'image.rs'), ('JPEG', 'jpeg.rs'), ('COMPRESSION', 'compression.rs')]:
        source = source.replace('path=' + key, 'path=' + json.dumps(str(ROOT / 'src' / name)))
    (work / 'probe.rs').write_text(source, encoding="utf-8")
    executable = work / ('probe.exe' if os.name == 'nt' else 'probe')
    subprocess.run(['rustc', '--edition=2021', '-O', str(work / 'probe.rs'), '-o', str(executable)], check=True)
    input_path, output_path = work / 'input.img', work / 'pixels.bin'
    counts = dict(tga=0, pcx=0, pillow_oracle=0, imagemagick_oracle=0,
                  rejections=0, cancellations=0, mutations=0)
    seeds = []
    checks = []
    rng = random.Random(290029)

    def run(data, stop=0):
        input_path.write_bytes(data)
        process = subprocess.run([str(executable), str(input_path), str(output_path), str(stop)],
                                 capture_output=True, text=True, timeout=5)
        assert process.returncode in (0, 2), (process.returncode, process.stdout, process.stderr)
        return process

    def valid(name, data, size, rgba, family, oracle=None, tolerance=0):
        process = run(data)
        assert process.returncode == 0, (name, process.stdout)
        raw = output_path.read_bytes()
        assert struct.unpack('<II', raw[:8]) == size, name
        bgra = raw[8:]
        got = bytearray(len(bgra))
        got[0::4], got[1::4], got[2::4], got[3::4] = bgra[2::4], bgra[1::4], bgra[0::4], bgra[3::4]
        assert len(got) == len(rgba), name
        delta = max((abs(a - b) for a, b in zip(got, rgba)), default=0)
        assert delta <= tolerance, f'{name}: maximum channel difference {delta}'
        counts[family] += 1
        if oracle:
            counts[oracle] += 1
        checks.append(dict(name=name, difference=delta))
        seeds.append(data)

    def bad(name, data, stop=0):
        process = run(data, stop)
        assert process.returncode == 2, (name, process.stdout)
        if stop:
            assert 'cancelled' in process.stdout, (name, process.stdout)
        counts['cancellations' if stop else 'rejections'] += 1

    # Independent Pillow encoders/decoders cover standard writer output and
    # palettes, RLE packet boundaries, odd widths and padded PCX planes.
    for width, height in [(1, 1), (7, 3), (31, 17), (128, 2), (129, 5)]:
        for mode in ('RGB', 'RGBA', 'L', 'LA', 'P', '1'):
            components = len(mode) if mode in ('RGB', 'RGBA', 'LA') else 1
            samples = [tuple((x * 23 + y * 67 + c * 83) & 255 for c in range(components))
                       for y in range(height) for x in range(width)]
            image = Image.new(mode, (width, height))
            image.putdata(samples if components > 1 else [p[0] & 128 if mode == '1' else p[0] for p in samples])
            if mode == 'P':
                image.putpalette([n for i in range(256) for n in (i, (i * 7) & 255, 255 - i)])
            for family in ('tga', 'pcx'):
                if family == 'pcx' and mode in ('RGBA', 'LA') or family == 'tga' and mode == '1':
                    continue
                for compressed in ((False, True) if family == 'tga' else (True,)):
                    stream = BytesIO()
                    image.save(stream, format=family.upper(), **({'compression':'tga_rle'} if compressed and family == 'tga' else {}))
                    data = stream.getvalue()
                    reference = Image.open(BytesIO(data)).convert('RGBA').tobytes()
                    valid(f'Pillow {family} {mode} {width}x{height} rle={compressed}', data,
                          (width, height), reference, family, 'pillow_oracle')

    # Authored TGA data exercises all origins, true-color depths, gray-alpha,
    # nonzero palette origins, 16-bit indices and explicit alpha semantics.
    for width, height in [(3, 2), (129, 3)]:
        for depth in (15, 16, 24, 32):
            for origin in (0, 0x10, 0x20, 0x30):
                for compressed in (False, True):
                    samples, rgba = [], []
                    for n in range(width * height):
                        if depth <= 16:
                            r, g, b = n % 32, (n * 7) % 32, (n * 19) % 32
                            a = 255 if depth == 15 or n % 3 else 0
                            samples.append(struct.pack('<H', (int(a != 0) << 15) | r << 10 | g << 5 | b))
                            rgba.append(tuple((v * 255 + 15) // 31 for v in (r, g, b)) + (a,))
                        else:
                            r, g, b, a = (n * 29) & 255, (n * 43) & 255, (n * 71) & 255, (n * 97) & 255
                            samples.append(bytes((b, g, r, a))[:depth // 8])
                            rgba.append((r, g, b, a if depth == 32 else 255))
                    attrs = {15:0, 16:1, 24:0, 32:8}[depth]
                    data = tga_file(width, height, samples, depth, origin=origin, attributes=attrs,
                                    rle=compressed, footer=compressed, identifier=b'ChezCat')
                    valid(f'TGA {depth}bpp origin{origin} rle{compressed}', data,
                          (width, height), rgba_bytes(rgba), 'tga')
    for depth in (8, 16):
        samples = [bytes([n * 19, n * 17])[:depth // 8] for n in range(12)]
        expected = bytes(v for n in range(12) for v in [n * 19] * 3 + [n * 17 if depth == 16 else 255])
        for compressed in (False, True):
            data = tga_file(4, 3, samples, depth, kind=3, attributes=8 if depth == 16 else 0, rle=compressed)
            valid(f'TGA gray {depth} rle{compressed}', data, (4, 3), expected, 'tga')
    for entry_depth in (15, 16, 24, 32):
        for index_depth, first in [(8, 17), (16, 1000)]:
            palette, colors = [], []
            for n in range(7):
                if entry_depth <= 16:
                    palette.append(struct.pack('<H', (n % 2) << 15 | n << 10 | (31 - n) << 5 | n * 3))
                    colors.append(tuple((v * 255 + 15) // 31 for v in (n, 31 - n, n * 3)) +
                                  ((255 if entry_depth == 15 else (n % 2) * 255),))
                else:
                    colors.append((n * 37, n * 13, 255 - n * 31, n * 41 if entry_depth == 32 else 255))
                    r, g, b, a = colors[-1]
                    palette.append(bytes((b, g, r, a))[:entry_depth // 8])
            samples = [bytes([first + n % 7]) if index_depth == 8 else struct.pack('<H', first + n % 7) for n in range(15)]
            data = tga_file(5, 3, samples, index_depth, kind=1, palette=palette, first=first,
                            palette_depth=entry_depth, attributes={15:0,16:1,24:0,32:8}[entry_depth], rle=True)
            valid(f'TGA indexed{index_depth} palette{entry_depth} first{first}', data, (5, 3),
                  rgba_bytes([colors[n % 7] for n in range(15)]), 'tga')
    for alpha_type in range(5):
        data = tga_file(3, 1, [bytes((0, 0, 64, 128)), bytes((0, 99, 0, 0)), bytes((255, 0, 0, 255))],
                        32, attributes=8, alpha_type=alpha_type)
        if alpha_type < 3:
            rgba = [(64, 0, 0, 255), (0, 99, 0, 255), (0, 0, 255, 255)]
        elif alpha_type == 3:
            rgba = [(64, 0, 0, 128), (0, 99, 0, 0), (0, 0, 255, 255)]
        else:
            rgba = [(128, 0, 0, 128), (0, 0, 0, 0), (0, 0, 255, 255)]
        valid(f'TGA alpha type{alpha_type}', data, (3, 1), rgba_bytes(rgba), 'tga')
    # Repeat packets can cross rows in old writers; total pixels stay bounded.
    data = tga_file(17, 9, [b'\x66\x55\x44'] * 153, 24, rle=True)
    data = data[:18] + tga_rle([b'\x66\x55\x44'] * 153) + data[-26:]
    valid('TGA repeated packet across rows', data, (17, 9), bytes([68, 85, 102, 255]) * 153, 'tga')
    tga_seed = tga_file(3, 2, [b'\x77\x55\x33'] * 6, 24, rle=True)

    palette = [(n, (n * 7) & 255, 255 - n) for n in range(256)]
    for bits, planes in [(1, 1), (1, 2), (1, 3), (1, 4), (2, 1), (2, 2), (4, 1), (8, 1), (8, 3)]:
        for compressed in (False, True):
            for width in (1, 7, 17):
                height = 5
                samples = [((n * 17) & 255, (n * 61) & 255, (n * 97) & 255) if planes == 3 and bits == 8
                           else (n * 7) % (1 << (bits * planes)) for n in range(width * height)]
                colors = (samples if bits == 8 and planes == 3 else
                          [(255 * n,) * 3 for n in samples] if bits == planes == 1 else
                          [palette[n] for n in samples])
                data = pcx_file(width, height, samples, bits=bits, planes=planes,
                               rle=compressed, palette=palette, padding=4)
                valid(f'PCX bits{bits} planes{planes} width{width} rle{compressed}', data,
                      (width, height), rgba_bytes(colors), 'pcx')
    gray = [n * 17 for n in range(15)]
    data = pcx_file(5, 3, gray, planes=1, gray=True)
    valid('PCX explicit gray without trailing palette', data, (5, 3), rgba_bytes([(n,) * 3 for n in gray]), 'pcx')
    pcx_seed = pcx_file(3, 2, [(20, 40, 60)] * 6)

    # A second oracle checks hand-authored common layouts independently.
    convert = shutil.which('magick') or shutil.which('convert')
    if convert:
        for family, data, size in [('tga', tga_seed, (3, 2)), ('pcx', pcx_seed, (3, 2))]:
            path = work / ('oracle.' + family)
            path.write_bytes(data)
            reference = subprocess.run([convert, str(path), '-depth', '8', 'RGBA:-'],
                                       capture_output=True, check=True).stdout
            valid('ImageMagick ' + family, data, size, reference, family, 'imagemagick_oracle')

    # Rejections are through the production sniffer, including the historical
    # TGA/CUR first-four-byte collision and metadata/trailing-palette bounds.
    for family, data in [('tga', tga_seed), ('pcx', pcx_seed)]:
        # Complete original TGA pixels remain valid when the optional footer is removed.
        raster_end = len(data) - 26 if family == 'tga' else len(data)
        for length in range(raster_end):
            bad(f'{family} truncated at{length}', data[:length])
    def changed(data, at, value):
        out = bytearray(data)
        out[at:at + len(value)] = value
        return out
    for at, value in [(12, b'\0\0'), (14, b'\0\0'), (12, struct.pack('<H', 16385)),
                      (12, struct.pack('<HH', 4096, 4096)), (17, b'\x60'), (16, b'\x0c'),
                      (17, b'\x28'), (1, b'\x02'), (18, b'\xff')]:
        bad('malformed TGA header/packet', changed(tga_seed, at, value))
    bad('TGA extra raster byte', tga_seed[:-26] + b'\0' + tga_seed[-26:])
    bad('TGA footer extension beyond end', changed(tga_seed, len(tga_seed) - 26, struct.pack('<I', 0xffffffff)))
    bad('TGA footer directory in header', changed(tga_seed, len(tga_seed) - 22, struct.pack('<I', 5)))
    invalid_palette = tga_file(1, 1, [b'\x05'], 8, kind=1, first=4, palette=[b'\0\0\0'])
    bad('TGA index outside color map', invalid_palette)
    for alpha_type in (5, 255):
        bad('TGA unsupported alpha type', tga_file(1, 1, [b'\0\0\0\xff'], 32, alpha_type=alpha_type))
    for at, value in [(64, b'\x01'), (66, b'\0\0'), (66, b'\x03\0'), (66, b'\x02\0'),
                      (4, struct.pack('<H', 60000)), (8, struct.pack('<H', 16390)),
                      (65, b'\x04'), (128, b'\xc0'), (128, b'\xff')]:
        bad('malformed PCX header/run', changed(pcx_seed, at, value))
    bad('PCX trailing garbage', pcx_seed + b'\0')
    bad('PCX overlong padded expansion', pcx_file(1, 1, [(1, 2, 3)])[:10] + struct.pack('<H', 16392) +
        changed(pcx_seed, 66, struct.pack('<H', 65534))[12:128])
    indexed = pcx_file(5, 3, gray, planes=1)
    bad('PCX missing color palette', indexed[:-769])
    bad('PCX palette marker missing', changed(indexed, len(indexed) - 769, b'\x0b'))
    bad('PCX truncated palette', indexed[:-1])
    bad('PCX unsupported implicit old palette', changed(indexed, 1, b'\x03'))

    for data in [tga_seed, pcx_seed,
                 tga_file(129, 129, [b'\0\0\0'] * (129 * 129), 24, rle=True),
                 pcx_file(129, 129, [(8, 16, 32)] * (129 * 129)),
                 pcx_file(1, 2, [(8, 16, 32)] * 2, padding=65530)]:
        bad('legacy decoder cancellation at entry', data, 1)
        bad('legacy decoder cancellation after dispatch', data, 4)
    padded = pcx_file(1, 2, [(8, 16, 32)] * 2, padding=65530)
    bad('PCX cancellation while expanding long padded scanline', padded, 15)
    for _ in range(400):
        data = bytearray(rng.choice(seeds))
        for _ in range(rng.randrange(1, 8)):
            if data:
                data[rng.randrange(len(data))] = rng.randrange(256)
        if data and rng.random() < 0.3:
            del data[rng.randrange(len(data)):]
        run(data)
        counts['mutations'] += 1
    if args.write_demos:
        for name, reference in write_demos(ROOT / 'pages' / 'images'):
            path = ROOT / 'pages' / 'images' / name
            valid(name, path.read_bytes(), (240, 144), reference, path.suffix[1:])
            pillow = Image.open(path).convert('RGBA').tobytes()
            valid(name + ' Pillow comparison', path.read_bytes(), (240, 144), pillow,
                  path.suffix[1:], 'pillow_oracle')
    report = dict(passed=True, counts=counts, production_dispatcher=True,
                  max_channel_difference=max(c['difference'] for c in checks),
                  work_directory=str(work), checks=checks)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + '\n', encoding="utf-8")
    print(json.dumps({k:v for k, v in report.items() if k != 'checks'}, indent=2))


if __name__ == '__main__':
    main()
