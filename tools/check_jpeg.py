#!/usr/bin/env python3
"""Independent JPEG checks and original demo image generation.

Development only: Pillow/libjpeg creates and decodes the comparison images.
Neither is linked into or invoked by ChezCat. The Rust probe calls our own
JPEG decoder, including its cancellation callback and allocation limits.

Run: python tools/check_jpeg.py [--rustc PATH] [--output report.json]
Regenerate the four original demo assets: python tools/check_jpeg.py --write-demos

Primary references: ITU-T T.81 (JPEG), CIPA Exif orientation, and Pillow's
documented JPEG encoder options. No external implementation source is used.
"""

from __future__ import annotations

import argparse
from collections import Counter
from dataclasses import dataclass
from io import BytesIO
import json
import math
import os
from pathlib import Path
import random
import struct
import subprocess
import tempfile
import time

from PIL import Image, ImageChops, ImageDraw, ImageOps, features
import PIL


ROOT = Path(__file__).resolve().parents[1]
PROBE = r'''
#![allow(dead_code)]
#[path=JPEG] mod jpeg;
#[path=IMAGE] mod image;
#[path=COMPRESSION] mod compression;
use std::cell::Cell;
use std::io::{self, BufRead, Write};
fn main() {
    let mut output = io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let line = line.unwrap(); let fields: Vec<_> = line.split('\t').collect();
        let stop: usize = fields[2].parse().unwrap();
        let count = Cell::new(0usize);
        let check = || { let current=count.get()+1; count.set(current);
            if stop!=0 && current>=stop { Err("cancelled".to_string()) } else { Ok(()) } };
        let data = std::fs::read(fields[0]).unwrap();
        let started = std::time::Instant::now();
        match jpeg::decode(&data, &check) {
            Ok(bitmap) => {
                let micros=started.elapsed().as_micros();
                assert_eq!(bitmap.pixels.len(),bitmap.width*bitmap.height);
                let mut image=io::BufWriter::new(std::fs::File::create(fields[1]).unwrap());
                write!(image,"P6\n{} {}\n255\n",bitmap.width,bitmap.height).unwrap();
                for pixel in bitmap.pixels {
                    assert_eq!(pixel>>24,255,"JPEG output must be opaque");
                    image.write_all(&[(pixel>>16) as u8,(pixel>>8) as u8,pixel as u8]).unwrap();
                }
                image.flush().unwrap();
                writeln!(output,"OK\t{}\t{}\t{}\t{}",bitmap.width,bitmap.height,count.get(),micros).unwrap();
            }
            Err(error) => writeln!(output,"ERR\t{}\t{}",count.get(),error.replace(['\t','\n','\r']," ")).unwrap(),
        }
        output.flush().unwrap();
    }
}
'''


@dataclass
class Case:
    label: str
    data: bytes
    reference: Image.Image | None
    group: str
    max_error: int = 3
    mean_error: float = 0.75
    stop: int = 0
    error: str | None = None


def encoded(image, **options):
    stream = BytesIO()
    image.save(stream, 'JPEG', **options)
    return stream.getvalue()


def reference(data):
    with Image.open(BytesIO(data)) as image:
        # libjpeg deliberately uses nearest-neighbor chroma expansion for very
        # narrow images. Our decoder consistently uses centered interpolation.
        # Recover the independently decoded native chroma samples from those
        # repeated pixels and let Pillow's separate resampler center them.
        # No large pixel tolerance is used to conceal the different filters.
        frame = next((item for item in markers(data) if item[0] in [0xC0, 0xC1, 0xC2]), None)
        factors = [] if frame is None else [data[frame[2] + 7 + 3 * i] for i in range(data[frame[2] + 5])]
        different_filter = image.width > 2 or (factors == [0x22, 0x11, 0x11] and image.height > 2)
        if image.width <= 4 and different_filter and factors in [[0x21, 0x11, 0x11], [0x22, 0x11, 0x11]]:
            image.draft('YCbCr', image.size)
            image.load()
            assert image.mode == 'YCbCr'
            width, height = image.size
            ratio_y = 2 if factors[0] == 0x22 else 1
            chroma_width, chroma_height = math.ceil(width / 2), math.ceil(height / ratio_y)
            channels = list(image.split())
            for index in [1, 2]:
                native = Image.new('L', (chroma_width, chroma_height))
                native.putdata([channels[index].getpixel((x * 2, y * ratio_y))
                                for y in range(chroma_height) for x in range(chroma_width)])
                channels[index] = native.resize((chroma_width * 2, chroma_height * ratio_y),
                                                Image.Resampling.BILINEAR).crop((0, 0, width, height))
            converted = Image.merge('YCbCr', channels).convert('RGB')
            converted.info.update(image.info)
            return ImageOps.exif_transpose(converted)
        return ImageOps.exif_transpose(image).convert('RGB')


def segment(marker, payload):
    return bytes([255, marker]) + struct.pack('>H', len(payload) + 2) + payload


def markers(data):
    """Locate marker envelopes, including restart markers, without decoding entropy."""
    position = 0
    while position < len(data) - 1:
        if data[position] != 255:
            position += 1
            continue
        start = position
        while position < len(data) and data[position] == 255:
            position += 1
        if position == len(data):
            break
        marker = data[position]
        position += 1
        if marker == 0:
            continue
        if marker in [0x01, 0xD8, 0xD9] or 0xD0 <= marker <= 0xD7:
            yield marker, start, position, position
            continue
        if position + 2 > len(data):
            break
        length = int.from_bytes(data[position:position + 2], 'big')
        end = position + length
        if length < 2 or end > len(data):
            break
        yield marker, start, position + 2, end
        position = end


def patch(data, offset, replacement):
    return data[:offset] + replacement + data[offset + len(replacement):]


def without(data, marker):
    spans = [(start, end) for code, start, _, end in markers(data) if code == marker]
    for start, end in reversed(spans):
        data = data[:start] + data[end:]
    return data


def pattern(width, height, kind):
    rng = random.Random(0x81 + width * 997 + height)
    pixels = []
    for y in range(height):
        for x in range(width):
            if kind == 'flat':
                rgb = (73, 137, 211)
            elif kind == 'smooth':
                rgb = (round(255 * x / max(1, width - 1)),
                       round(255 * y / max(1, height - 1)),
                       round(127 + 100 * math.sin(x * 0.17 + y * 0.13)))
            elif kind == 'edges':
                rgb = ((255 if x >= width // 2 else 0),
                       (255 if y >= height // 2 else 0),
                       (255 if (x + y) % 7 < 3 else 0))
            elif kind == 'checker':
                rgb = (255, 0, 80) if (x + y) % 2 else (0, 235, 255)
            else:
                rgb = (rng.randrange(256), rng.randrange(256), rng.randrange(256))
            pixels.append(rgb)
    image = Image.new('RGB', (width, height))
    image.putdata(pixels)
    return image


def exif_orientation(orientation, endian='little'):
    prefix = '<' if endian == 'little' else '>'
    tiff = (b'II' if endian == 'little' else b'MM') + struct.pack(prefix + 'HIH', 42, 8, 1)
    tiff += struct.pack(prefix + 'HHI', 0x0112, 3, 1)
    tiff += struct.pack(prefix + 'H', orientation) + b'\0\0' + bytes(4)
    return b'Exif\0\0' + tiff


def entropy(bits):
    bits += '1' * (-len(bits) % 8)
    raw = bytes(int(bits[index:index + 8], 2) for index in range(0, len(bits), 8))
    return raw.replace(b'\xff', b'\xff\0')


def simple_coefficients(index, coefficient, quant=3):
    """Our original one-block encoder, using tiny explicit Huffman tables.

    This isolates every AC position independently. The expected output still
    comes from libjpeg rather than a second copy of the browser's IDCT.
    """
    category = abs(coefficient).bit_length()
    zeros = index - 1
    symbol = ((zeros % 16) << 4) | category
    ac = bytes([0x10, 0, 3] + [0] * 14 + [0, symbol, 0xF0])
    dc = bytes([0, 1] + [0] * 15 + [0])
    tables = segment(0xDB, bytes([0] + [quant] * 64)) + segment(0xC4, dc + ac)
    frame = segment(0xC0, bytes([8, 0, 8, 0, 8, 1, 1, 0x11, 0]))
    scan = segment(0xDA, bytes([1, 1, 0, 0, 63, 0]))
    magnitude = coefficient if coefficient >= 0 else coefficient + (1 << category) - 1
    bits = '0' + '10' * (zeros // 16) + '01' + format(magnitude, f'0{category}b')
    if index < 63:
        bits += '00'
    return b'\xff\xd8' + tables + frame + scan + entropy(bits) + b'\xff\xd9'


def flat_sampling(width, height, factors, restart=0):
    """Encode a known neutral image with arbitrary integral sampling factors."""
    tables = segment(0xDB, bytes([0] + [1] * 64))
    tables += segment(0xC4, bytes([0, 1] + [0] * 15 + [0, 0x10, 1] + [0] * 15 + [0]))
    components = b''.join(bytes([index + 1, (h << 4) | v, 0]) for index, (h, v) in enumerate(factors))
    frame = segment(0xC0, bytes([8]) + struct.pack('>HHB', height, width, len(factors)) + components)
    scan = segment(0xDA, bytes([len(factors)]) + b''.join(bytes([i + 1, 0]) for i in range(len(factors))) + bytes([0, 63, 0]))
    maximum_h, maximum_v = max(h for h, _ in factors), max(v for _, v in factors)
    count = math.ceil(width / (8 * maximum_h)) * math.ceil(height / (8 * maximum_v))
    per_mcu = sum(h * v for h, v in factors)
    body = bytearray()
    for start in range(0, count, restart or count):
        block_count = min(restart or count, count - start)
        body += entropy('00' * per_mcu * block_count)
        if restart and start + block_count < count:
            body += bytes([255, 0xD0 + ((start // restart) % 8)])
    return (b'\xff\xd8' + tables + frame + (segment(0xDD, struct.pack('>H', restart)) if restart else b'')
            + scan + body + b'\xff\xd9')


def landscape(width=700, height=260):
    image = Image.new('RGB', (width, height))
    pixels = image.load()
    for y in range(height):
        fraction = y / max(1, height - 1)
        for x in range(width):
            ridge1 = height * (0.53 + 0.13 * math.sin(x / width * 8 + 0.2) + 0.04 * math.sin(x / width * 29))
            ridge2 = height * (0.74 + 0.10 * math.sin(x / width * 11 + 1.8))
            ridge3 = height * (0.91 + 0.06 * math.sin(x / width * 15 - 0.8))
            color = (round(29 + 30 * fraction), round(65 + 65 * fraction), round(91 + 50 * fraction))
            if ((x - width * 0.77) / (height * 0.13)) ** 2 + ((y - height * 0.24) / (height * 0.13)) ** 2 < 1:
                color = (255, 203, 122)
            if y > ridge1:
                color = (31, 133, 137)
            if y > ridge2:
                color = (77, 182, 159)
            if y > ridge3:
                color = (178, 224, 187)
            pixels[x, y] = color
    return image


def write_demos():
    folder = ROOT / 'pages/images'
    folder.mkdir(parents=True, exist_ok=True)
    picture = landscape()
    images = {
        'jpeg-landscape.jpg': encoded(picture, quality=88, subsampling=2, optimize=True),
        'jpeg-progressive.jpg': encoded(picture, quality=88, subsampling=2, progressive=True),
    }
    gray = pattern(320, 180, 'smooth').convert('L')
    draw = ImageDraw.Draw(gray)
    for radius in [70, 54, 38, 22]:
        draw.ellipse((160 - radius, 90 - radius, 160 + radius, 90 + radius), outline=245, width=3)
    images['jpeg-gray.jpg'] = encoded(gray, quality=91, optimize=True)
    upright = landscape(320, 180)
    draw = ImageDraw.Draw(upright)
    draw.polygon([(36, 22), (14, 52), (28, 52), (28, 89), (44, 89), (44, 52), (58, 52)], fill=(255, 244, 211))
    # Stored sideways, with Exif6 asking the decoder to rotate90 degrees clockwise.
    sideways = upright.transpose(Image.Transpose.ROTATE_90)
    images['jpeg-oriented.jpg'] = encoded(sideways, quality=91, subsampling=0, exif=exif_orientation(6))
    for name, data in images.items():
        (folder / name).write_bytes(data)
    print(json.dumps({'demo_assets': [{'name': name, 'bytes': len(data), 'display_size': reference(data).size}
                                     for name, data in images.items()]}, indent=2))


def cases():
    result = []

    def good(label, data, group='pixels', expected=None, max_error=3, mean_error=0.75):
        result.append(Case(label, data, reference(data) if expected is None else expected,
                           group, max_error, mean_error))

    def bad(label, data, error=None, group='malformed'):
        result.append(Case(label, bytes(data), None, group, error=error))

    modes = [('gray', 0), ('ycbcr444', 0), ('ycbcr422', 1), ('ycbcr420', 2), ('rgb', 0)]
    sizes = [(1, 1), (2, 3), (3, 5), (4, 7), (5, 3), (7, 9), (8, 8), (9, 7),
             (15, 17), (16, 16), (17, 19), (33, 31), (65, 49)]
    kinds = ['flat', 'smooth', 'edges', 'checker', 'noise']
    for size_index, (width, height) in enumerate(sizes):
        for quality_index, quality in enumerate([12, 75, 95]):
            kind = kinds[(size_index + quality_index) % len(kinds)]
            original = pattern(width, height, kind)
            for mode, sampling in modes:
                for progressive in [False, True]:
                    image = original.convert('L') if mode == 'gray' else original
                    data = encoded(image, quality=quality, subsampling=sampling,
                                   progressive=progressive, optimize=quality_index == 1,
                                   keep_rgb=mode == 'rgb')
                    good(f'{mode}/{width}x{height}/{kind}/q{quality}/progressive{progressive}', data,
                         'progressive' if progressive else 'sequential',
                         max_error=1 if mode in ['gray', 'rgb'] else 4 if sampling else 3)

    original = pattern(65, 49, 'noise')
    # Retain a case where independent integer IDCT and interpolation rounding
    # together differ by two chroma levels, or four blue levels after conversion.
    for progressive in [False, True]:
        good(f'subsampled-rounding/33x31/progressive{progressive}',
             encoded(pattern(33, 31, 'noise'), quality=75, subsampling=1,
                     optimize=True, progressive=progressive), 'rounding', max_error=4)
    for mode, sampling in modes:
        for progressive in [False, True]:
            for setting, interval in [('restart_marker_blocks', 1), ('restart_marker_blocks', 7),
                                      ('restart_marker_rows', 1)]:
                data = encoded(original.convert('L') if mode == 'gray' else original,
                               quality=87, subsampling=sampling, keep_rgb=mode == 'rgb',
                               progressive=progressive, **{setting: interval})
                assert b'\xff\xdd' in data and b'\xff\xd0' in data
                good(f'restart/{mode}/progressive{progressive}/{setting}{interval}', data, 'restart',
                     max_error=1 if mode in ['gray', 'rgb'] else 4 if sampling else 3)

    for position in range(1, 64):
        for value in [-37, 37]:
            good(f'isolated-ac/{position}/{value}', simple_coefficients(position, value),
                 'isolated-coefficient', max_error=1)
    for factors in [[(1, 1), (1, 1), (1, 1)], [(2, 1), (1, 1), (1, 1)],
                    [(2, 2), (1, 1), (1, 1)], [(4, 1), (1, 1), (1, 1)],
                    [(1, 2), (1, 1), (1, 1)], [(1, 4), (1, 1), (1, 1)],
                    [(3, 1), (1, 1), (1, 1)], [(1, 3), (1, 1), (1, 1)],
                    [(2, 2), (2, 1), (2, 1)]]:
        for interval in [0, 2]:
            good(f'sampling/{factors}/restart{interval}', flat_sampling(73, 41, factors, interval),
                 'sampling', expected=Image.new('RGB', (73, 41), (128, 128, 128)), max_error=0, mean_error=0)

    for orientation in range(1, 9):
        for endian in ['little', 'big']:
            good(f'exif/{orientation}/{endian}', encoded(original, quality=91, subsampling=0,
                 exif=exif_orientation(orientation, endian)), 'orientation')

    normal = encoded(original, quality=91, subsampling=0)
    locations = list(markers(normal))
    sof = next(item for item in locations if item[0] == 0xC0)
    sos = next(item for item in locations if item[0] == 0xDA)
    dqt = next(item for item in locations if item[0] == 0xDB)
    dht = next(item for item in locations if item[0] == 0xC4)
    extended = patch(normal, sof[1] + 1, b'\xc1')
    good('extended-sequential', extended, 'metadata')
    precision16 = extended
    for code, start, payload, end in reversed(list(markers(precision16))):
        if code == 0xDB:
            tables = precision16[payload:end]
            assert len(tables) == 65 and tables[0] >> 4 == 0
            table = bytes([tables[0] | 0x10]) + b''.join(bytes([0, value]) for value in tables[1:])
            precision16 = precision16[:start] + segment(0xDB, table) + precision16[end:]
    good('extended-16bit-quantization', precision16, 'metadata')
    bad('baseline-16bit-quantization', patch(precision16, next(m[1] for m in markers(precision16) if m[0] == 0xC1) + 1, b'\xc0'))
    good('ignored-app-comment', normal[:2] + segment(0xE3, b'Original ignored data\0\xff')
         + segment(0xFE, b'ChezCat JPEG fixture') + normal[2:], 'metadata')
    good('marker-fill', normal[:2] + b'\xff\xff' + normal[2:], 'metadata')
    good('no-jfif', without(normal, 0xE0), 'metadata')
    odd_ids = normal
    for index, value in enumerate([17, 42, 99]):
        odd_ids = patch(odd_ids, sof[2] + 6 + index * 3, bytes([value]))
        odd_ids = patch(odd_ids, sos[2] + 1 + index * 2, bytes([value]))
    good('nondefault-component-identifiers', odd_ids, 'metadata')
    rgb_ids = normal
    for index, value in enumerate(b'RGB'):
        rgb_ids = patch(rgb_ids, sof[2] + 6 + index * 3, bytes([value]))
        rgb_ids = patch(rgb_ids, sos[2] + 1 + index * 2, bytes([value]))
    good('jfif-takes-priority-over-rgb-identifiers', rgb_ids, 'metadata')
    rgb = encoded(original, quality=91, subsampling=0, keep_rgb=True)
    good('rgb-identifiers-without-adobe', without(rgb, 0xEE), 'metadata', max_error=1)
    bad('truncated-jfif', normal[:2] + segment(0xE0, b'JFIF\0') + normal[2:])

    # A single representative roughly one-megapixel image in both coding modes
    # records optimized decoder timing without process/file-I/O overhead.
    large = pattern(1024, 768, 'smooth')
    for progressive in [False, True]:
        good(f'performance/1024x768/progressive{progressive}', encoded(large, quality=87,
             subsampling=2, progressive=progressive), 'performance', max_error=4)

    # Exact output and dimension boundaries are accepted; one beyond each is rejected.
    for width, height in [(16384, 1), (1, 16384), (2048, 2048)]:
        image = Image.new('L', (width, height), 128)
        good(f'boundary/{width}x{height}', encoded(image, quality=75), 'bounds', max_error=0, mean_error=0)
    for width, height in [(0, 1), (1, 0), (16385, 1), (1, 16385), (2049, 2048), (65535, 65535)]:
        bad(f'dimension/{width}x{height}', patch(normal, sof[2] + 1, struct.pack('>HH', height, width)), group='bounds')
    bad('input-limit', b'\xff\xd8' + bytes(32 * 1024 * 1024 - 1), group='bounds')
    bad('marker-limit', normal[:2] + segment(0xFE, b'') * 65537 + normal[2:], group='bounds')

    tiny = encoded(Image.new('L', (1, 1), 128), quality=75, optimize=True)
    for length in range(len(tiny)):
        bad(f'truncated/{length}', tiny[:length], group='truncated')
    for suffix in [b'\0', b'garbage', b'\xff\xd8', tiny]:
        bad(f'trailing/{len(suffix)}', tiny + suffix)
    bad('invalid-soi', b'xx' + normal[2:])
    bad('only-soi-eoi', b'\xff\xd8\xff\xd9')
    bad('duplicate-frame', normal[:sof[1]] + normal[sof[1]:sof[3]] + normal[sof[1]:])
    bad('missing-quantization', without(normal, 0xDB))
    bad('missing-huffman', without(normal, 0xC4))
    bad('missing-frame', without(normal, 0xC0))
    bad('missing-scan', normal[:sos[1]] + b'\xff\xd9')
    bad('empty-entropy', normal[:sos[3]] + b'\xff\xd9')
    for code in [0xC3, 0xC5, 0xC6, 0xC9, 0xCA, 0xCF]:
        bad(f'unsupported-frame/{code}', patch(normal, sof[1] + 1, bytes([code])), group='unsupported')
    bad('unsupported-precision12', patch(extended, sof[2], b'\x0c'), group='unsupported')
    good('adobe-cmyk', encoded(original.convert('CMYK')), 'color')
    for code, start, payload, end in [sof, sos, dqt, dht]:
        for length in [0, 1, 65535]:
            bad(f'marker-length/{code}/{length}', patch(normal, start + 2, struct.pack('>H', length)))
    bad('zero-quantizer', patch(normal, dqt[2] + 1, b'\0'))
    bad('quantizer-id', patch(normal, dqt[2], b'\x04'))
    bad('quantizer-precision', patch(normal, dqt[2], b'\x20'))
    bad('huffman-oversubscribed', patch(normal, dht[2] + 1, b'\x03'))
    bad('huffman-id', patch(normal, dht[2], b'\x04'))
    bad('huffman-class', patch(normal, dht[2], b'\x20'))
    bad('zero-components', patch(normal, sof[2] + 5, b'\0'))
    bad('duplicate-frame-id', patch(normal, sof[2] + 9, normal[sof[2] + 6:sof[2] + 7]))
    bad('sampling-zero', patch(normal, sof[2] + 7, b'\0'))
    bad('sampling-five', patch(normal, sof[2] + 7, b'\x51'))
    bad('sampling-too-many-mcu-blocks', patch(normal, sof[2] + 7, b'\x44'))
    bad('undefined-quantizer', patch(normal, sof[2] + 8, b'\x03'))
    bad('zero-scan-components', patch(normal, sos[2], b'\0'))
    bad('unknown-scan-component', patch(normal, sos[2] + 1, b'\x7f'))
    bad('duplicate-scan-component', patch(normal, sos[2] + 3, normal[sos[2] + 1:sos[2] + 2]))
    bad('undefined-scan-table', patch(normal, sos[2] + 2, b'\x33'))
    bad('baseline-spectral-start', patch(normal, sos[3] - 3, b'\x01'))
    bad('baseline-spectral-end', patch(normal, sos[3] - 2, b'\x3e'))
    bad('baseline-successive-approximation', patch(normal, sos[3] - 1, b'\x01'))

    restart = encoded(original, quality=91, restart_marker_blocks=1)
    first_restart = next(m for m in markers(restart) if m[0] == 0xD0)
    bad('restart-wrong-sequence', patch(restart, first_restart[1] + 1, b'\xd4'))
    bad('restart-removed', restart[:first_restart[1]] + restart[first_restart[3]:])
    bad('restart-without-interval', without(restart, 0xDD))

    progressive = encoded(original, quality=91, progressive=True)
    first_scan = next(m for m in markers(progressive) if m[0] == 0xDA)
    bad('progressive-invalid-dc-band', patch(progressive, first_scan[3] - 2, b'\x3f'))
    bad('progressive-invalid-approximation', patch(progressive, first_scan[3] - 1, b'\xf0'))
    bad('progressive-refinement-before-initial', patch(progressive, first_scan[3] - 1, b'\x10'))
    bad('progressive-mismatched-refinement', patch(progressive, first_scan[3] - 1, b'\x20'))
    after_scan = next(m[1] for m in markers(progressive) if m[1] >= first_scan[3] and not 0xD0 <= m[0] <= 0xD7)
    bad('progressive-duplicate-initial-scan', progressive[:after_scan] + progressive[first_scan[1]:after_scan] + progressive[after_scan:])
    return result


def difference(actual, wanted):
    assert actual.size == wanted.size, (actual.size, wanted.size)
    histogram = ImageChops.difference(actual, wanted).histogram()
    total = sum(histogram)
    maximum = max(index % 256 for index, count in enumerate(histogram) if count)
    mean = sum((index % 256) * count for index, count in enumerate(histogram)) / total
    rms = math.sqrt(sum((index % 256) ** 2 * count for index, count in enumerate(histogram)) / total)
    return maximum, mean, rms


def run(probe, directory, checks, start=0):
    commands = []
    paths = []
    for number, case in enumerate(checks, start):
        source, output = directory / f'{number}.jpg', directory / f'{number}.ppm'
        source.write_bytes(case.data)
        output.unlink(missing_ok=True)
        assert '\t' not in str(source) and '\n' not in str(source)
        commands.append(f'{source}\t{output}\t{case.stop}\n')
        paths.append(output)
    process = subprocess.run([str(probe)], input=''.join(commands), capture_output=True, text=True, timeout=180)
    assert process.returncode == 0, process.stderr
    lines = process.stdout.splitlines()
    assert len(lines) == len(checks), (len(lines), len(checks), process.stderr)
    reports, failures = [], []
    for case, output, line in zip(checks, paths, lines):
        fields = line.split('\t')
        record = {'case': case.label, 'group': case.group, 'expected': 'image' if case.reference is not None else 'error'}
        try:
            if case.reference is None:
                assert fields[0] == 'ERR', f'Unexpected success: {line}'
                assert not output.exists(), 'Error created an output image'
                if case.error:
                    assert case.error.lower() in fields[2].lower(), line
                record.update(error=fields[2], callbacks=int(fields[1]))
            else:
                assert fields[0] == 'OK', line
                with Image.open(output) as decoded:
                    assert (int(fields[1]), int(fields[2])) == case.reference.size
                    maximum, mean, rms = difference(decoded.convert('RGB'), case.reference)
                record.update(max_error=maximum, mean_error=round(mean, 6), rms_error=round(rms, 6),
                              callbacks=int(fields[3]), decode_microseconds=int(fields[4]))
                assert maximum <= case.max_error and mean <= case.mean_error, (
                    f'Pixel error max={maximum}, mean={mean:.4f}, rms={rms:.4f}; '
                    f'limits max={case.max_error}, mean={case.mean_error}')
            record['passed'] = True
        except AssertionError as error:
            record.update(passed=False, failure=str(error))
            failures.append(record)
        reports.append(record)
    return reports, failures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rustc', default=os.environ.get('RUSTC', 'rustc'))
    parser.add_argument('--output', type=Path)
    parser.add_argument('--write-demos', action='store_true', help='Regenerate only the four original demo JPEGs')
    parser.add_argument('--keep', type=Path, help='Keep probe inputs/outputs here to investigate a failed comparison')
    args = parser.parse_args()
    if args.write_demos:
        write_demos()
        return
    started = time.monotonic()
    checks = cases()
    with tempfile.TemporaryDirectory(prefix='chezcat-jpeg-') as temporary:
        directory = args.keep.resolve() if args.keep else Path(temporary)
        directory.mkdir(parents=True, exist_ok=True)
        source = directory / 'jpeg_probe.rs'
        source.write_text(PROBE.replace('path=JPEG', 'path=' + json.dumps(str(ROOT / 'src/jpeg.rs')))
                         .replace('path=IMAGE', 'path=' + json.dumps(str(ROOT / 'src/image.rs')))
                         .replace('path=COMPRESSION', 'path=' + json.dumps(str(ROOT / 'src/compression.rs'))), encoding='utf-8')
        probe = directory / ('jpeg_probe.exe' if os.name == 'nt' else 'jpeg_probe')
        subprocess.run([args.rustc, '--edition=2021', '-O', str(source), '-o', str(probe)], check=True)
        reports, failures = run(probe, directory, checks)
        candidates = [(index, item) for index, item in enumerate(reports) if item.get('passed') and item.get('expected') == 'image']
        if candidates:
            index, item = max(candidates, key=lambda pair: pair[1].get('callbacks', 0))
            calls = item['callbacks']
            stops = sorted(set(n for n in [1, 2, 5, 10, calls // 4, calls // 2, calls - 1] if 0 < n <= calls))
            cancelled = [Case(f'cancellation/{stop}-of-{calls}', checks[index].data, None, 'cancellation',
                              stop=stop, error='cancelled') for stop in stops]
            extra, bad = run(probe, directory, cancelled, len(checks))
            reports.extend(extra)
            failures.extend(bad)
    report = {'pillow': PIL.__version__, 'libjpeg_turbo': features.version('libjpeg_turbo'),
              'elapsed_seconds': round(time.monotonic() - started, 3),
              'total': len(reports), 'passed': len(reports) - len(failures),
              'groups': dict(Counter(item['group'] for item in reports)),
              'pixel_maximum': max((item.get('max_error', 0) for item in reports), default=0),
              'pixel_mean_maximum': max((item.get('mean_error', 0) for item in reports), default=0),
              'pixel_limits': {'gray_and_rgb': 1, 'ycbcr444': 3, 'subsampled_ycbcr': 4,
                               'mean_absolute_error': 0.75},
              'narrow_sampling_reference': 'Pillow native chroma samples plus independent centered Pillow bilinear resampling',
              'results': reports}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({key: value for key, value in report.items() if key != 'results'}, indent=2))
    if failures:
        print(json.dumps({'failures': failures[:30]}, indent=2))
        raise SystemExit(1)


if __name__ == '__main__':
    main()
