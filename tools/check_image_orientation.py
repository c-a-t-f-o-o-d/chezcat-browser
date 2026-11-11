#!/usr/bin/env python3
"""Independent Exif geometry and browser presentation checks, using original fixtures.

Pillow/libwebp/libjpeg are development-only encoders/oracles. ChezCat continues
to decode and orient every production pixel with its own Rust code. No public
website, saved profile, or script is used by this checker.
"""
from __future__ import annotations

import argparse
from collections import Counter
from io import BytesIO
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import time
import zlib

from PIL import Image, ImageChops

from check_image_presentation import serving, page, marker_bottom

ROOT = Path(__file__).resolve().parents[1]
TRANSPOSE = {
    2: Image.Transpose.FLIP_LEFT_RIGHT, 3: Image.Transpose.ROTATE_180,
    4: Image.Transpose.FLIP_TOP_BOTTOM, 5: Image.Transpose.TRANSPOSE,
    6: Image.Transpose.ROTATE_270, 7: Image.Transpose.TRANSVERSE,
    8: Image.Transpose.ROTATE_90,
}
PROBE = r'''
#![allow(dead_code)]
#[path=COMPRESSION] mod compression;
#[path=JPEG] mod jpeg;
#[path=IMAGE] mod image;
use std::io::{Read,Write};
fn main() {
 let mut input=Vec::new(); std::io::stdin().read_to_end(&mut input).unwrap();
 let asset=match image::decode_asset_checked(&input,&||Ok(())) {
  Ok(asset)=>asset, Err(error)=>{eprintln!("{error}");std::process::exit(2)}
 };
 let mut out=std::io::BufWriter::new(std::io::stdout().lock());
 let mut number=|value:u32|out.write_all(&value.to_le_bytes()).unwrap();
 number(asset.bitmap.width as u32); number(asset.bitmap.height as u32);
 number(asset.animation.as_ref().map_or(0,|a|a.frames.len()) as u32);
 number(asset.animation.as_ref().and_then(|a|a.plays).unwrap_or(0));
 number(asset.retained_bytes().unwrap() as u32);
 fn pixels(out:&mut impl Write,bitmap:&image::Bitmap) {
  for &p in &bitmap.pixels {out.write_all(&[(p>>16)as u8,(p>>8)as u8,p as u8,(p>>24)as u8]).unwrap()}
 }
 pixels(&mut out,&asset.bitmap);
 if let Some(animation)=asset.animation {for frame in &animation.frames {
  out.write_all(&frame.delay_ms.to_le_bytes()).unwrap(); pixels(&mut out,&frame.bitmap);
 }}
}
'''


def encode(image, format, **options):
    stream = BytesIO()
    image.save(stream, format=format, **options)
    return stream.getvalue()


def tiff(orientation, little):
    endian = '<' if little else '>'
    return ((b'II' if little else b'MM') + struct.pack(endian+'HIH', 42, 8, 1)
            + struct.pack(endian+'HHI', 0x112, 3, 1)
            + struct.pack(endian+'H', orientation) + b'\0'*6)


def png_chunk(kind, payload):
    return struct.pack('>I', len(payload)) + kind + payload + struct.pack('>I', zlib.crc32(kind+payload))


def riff_chunk(kind, payload):
    return kind + struct.pack('<I', len(payload)) + payload + b'\0'*(len(payload) & 1)


def metadata(data, payload, size):
    if data.startswith(b'\x89PNG'):
        return data[:33] + png_chunk(b'eXIf', payload) + data[33:]
    if data.startswith(b'\xff\xd8'):
        payload = b'Exif\0\0' + payload
        return data[:2] + b'\xff\xe1' + struct.pack('>H', len(payload)+2) + payload + data[2:]
    assert data[8:12] == b'WEBP'
    chunks, at, extended = [], 12, False
    while at < len(data):
        kind, length = data[at:at+4], struct.unpack_from('<I', data, at+4)[0]
        body = data[at+8:at+8+length]
        at += 8 + length + (length & 1)
        if kind == b'EXIF':
            continue
        if kind == b'VP8X':
            body = bytes([body[0] | 8]) + body[1:]
            extended = True
        chunks.append(riff_chunk(kind, body))
    if not extended:
        width, height = size
        chunks.insert(0, riff_chunk(b'VP8X', b'\x18\0\0\0' + (width-1).to_bytes(3, 'little')
                                   + (height-1).to_bytes(3, 'little')))
    chunks.append(riff_chunk(b'EXIF', payload))
    body = b'WEBP' + b''.join(chunks)
    return b'RIFF' + struct.pack('<I', len(body)) + body


def transpose(image, orientation):
    return image.copy() if orientation == 1 else image.transpose(TRANSPOSE[orientation])


def exact(actual, expected, label):
    assert actual.size == expected.size, (label, actual.size, expected.size)
    assert actual.tobytes() == expected.tobytes(), (label, 'pixel difference')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rustc', default='rustc')
    parser.add_argument('--browser', type=Path)
    parser.add_argument('--work-dir', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    started = time.monotonic()
    checks, browser_checks = [], []
    with tempfile.TemporaryDirectory(prefix='chezcat-orientation-') as temporary:
        work = (args.work_dir or Path(temporary)).resolve()
        work.mkdir(parents=True, exist_ok=True)
        source = PROBE
        for module in ('compression', 'jpeg', 'image'):
            source = source.replace(module.upper(), json.dumps(str(ROOT/'src'/(module+'.rs'))))
        (work/'probe.rs').write_text(source, encoding="utf-8")
        probe = work/'probe'
        subprocess.run([args.rustc, '--edition', '2021', '-O', str(work/'probe.rs'), '-o', str(probe)], check=True)

        def decoded(data):
            result = subprocess.run([str(probe)], input=data, capture_output=True, timeout=15)
            assert result.returncode == 0, result.stderr.decode()
            width, height, count, plays, retained = struct.unpack_from('<5I', result.stdout)
            size, at = (width, height), 20
            length = width*height*4
            poster = Image.frombytes('RGBA', size, result.stdout[at:at+length]); at += length
            frames = []
            for _ in range(count):
                delay, = struct.unpack_from('<I', result.stdout, at); at += 4
                frame = Image.frombytes('RGBA', size, result.stdout[at:at+length]); at += length
                frames.append((frame, delay))
            assert at == len(result.stdout)
            return poster, frames, plays, retained

        original = Image.new('RGBA', (96, 64))
        original.putdata([((x*3+17) % 256, (y*5+31) % 256, (x*7+y*11+43) % 256, 255)
                          for y in range(64) for x in range(96)])
        alpha = original.copy()
        alpha.putalpha(Image.frombytes('L', alpha.size,
                       bytes(80 + (x+y)%176 for y in range(64) for x in range(96))))
        frames = [original.copy() for _ in range(3)]
        frames[1].paste((23, 211, 97, 255), (20, 10, 44, 30))
        frames[2].paste((199, 29, 211, 255), (6, 40, 28, 60))
        sources = {
            'png': encode(alpha, 'PNG'),
            'webp-lossless': encode(alpha, 'WEBP', lossless=True, exact=True),
            'webp-lossy': encode(alpha, 'WEBP', quality=85),
            'jpeg': encode(original.convert('RGB'), 'JPEG', quality=90, subsampling=0),
            'apng': encode(frames[0], 'PNG', save_all=True, append_images=frames[1:],
                           duration=[120, 180, 240], loop=1, disposal=[0, 1, 2], blend=[0, 0, 1]),
            'animated-webp': encode(frames[0], 'WEBP', lossless=True, exact=True, save_all=True,
                                    append_images=frames[1:], duration=[120, 180, 240], loop=1),
        }
        for kind, data in sources.items():
            baseline, animation, plays, retained = decoded(data)
            independent = Image.open(BytesIO(data))
            if kind not in ('jpeg', 'webp-lossy'):
                exact(baseline, independent.convert('RGBA'), (kind, 'independent poster'))
                for index, (frame, _) in enumerate(animation):
                    independent.seek(index)
                    exact(frame, independent.convert('RGBA'), (kind, 'independent composed frame', index))
            for little in (False, True):
                for orientation in range(1, 9):
                    for prefix in ((False, True) if 'webp' in kind else (False,)):
                        exif = (b'Exif\0\0' if prefix else b'') + tiff(orientation, little)
                        value = metadata(data, exif, baseline.size)
                        poster, actual_frames, actual_plays, actual_retained = decoded(value)
                        label = (kind, little, orientation, prefix)
                        exact(poster, transpose(baseline, orientation), label)
                        assert len(actual_frames) == len(animation) and actual_plays == plays, label
                        assert actual_retained == retained, (label, 'retained pixel accounting')
                        for (actual, delay), (expected, expected_delay) in zip(actual_frames, animation):
                            exact(actual, transpose(expected, orientation), label)
                            assert delay == expected_delay, (label, 'frame delay')
                        checks.append(dict(format=kind, endian='II' if little else 'MM', orientation=orientation,
                                           exif_identifier=prefix, canvas=poster.size, frames=len(animation)))
            for invalid in (b'', b'not exif', tiff(0, True), tiff(9, False),
                            tiff(6, True)[:17], b'II*\0\xff\xff\xff\xff'):
                poster, actual_frames, actual_plays, actual_retained = decoded(metadata(data, invalid, baseline.size))
                exact(poster, baseline, (kind, 'invalid metadata must remain inert'))
                assert len(actual_frames) == len(animation) and actual_plays == plays and actual_retained == retained
                checks.append(dict(format=kind, invalid_metadata_ignored=True, metadata_bytes=len(invalid)))

        if args.browser:
            routes, references = {}, {}
            # Include real animation playback after metadata transformation. A
            # single play terminates deterministically at the last frame.
            for kind in ('png', 'webp-lossless', 'apng', 'animated-webp'):
                data = sources[kind]
                baseline, animation, _, _ = decoded(data)
                for orientation in (6, 8):
                    path = f'/{kind}-{orientation}'
                    upright = transpose(animation[-1][0] if animation else baseline, orientation)
                    rgba = Image.alpha_composite(Image.new('RGBA', upright.size, 'white'), upright)
                    references[path] = rgba.convert('RGB')
                    routes[path] = ('image/webp' if 'webp' in kind else 'image/png',
                                    metadata(data, tiff(orientation, orientation == 6), baseline.size))
                    routes[path+'-page'] = ('text/html', page('<img style="background:white" src="'+path+'">'))
            with serving(routes) as (server, base):
                for path, reference in references.items():
                    for direct in (False, True):
                        target_path = path if direct else path+'-page'
                        label = target_path[1:] + ('-direct' if direct else '-embedded')
                        with server.lock:
                            server.records.clear()
                        output = work/(label+'.ppm')
                        subprocess.run([str(args.browser.resolve()), base+target_path, '--render', str(output),
                                        '--width', '800', '--height', '650', '--no-profile', '--no-javascript',
                                        '--wait-ms', '700'], capture_output=True, check=True, timeout=20)
                        rendered = Image.open(output).convert('RGB')
                        with server.lock:
                            requests = Counter(item['path'] for item in server.records)
                        assert requests == Counter([path] if direct else [target_path, path]), (label, requests)
                        if direct:
                            # Independent anchor location, followed by full pixel
                            # equality, avoids assuming internal viewer layout.
                            candidates = []
                            anchors = [(0, 0), (reference.width//2, reference.height//2),
                                       (reference.width-1, reference.height-1)]
                            for y in range(125, 340):
                                for x in range(280, 410):
                                    if all(rendered.getpixel((x+ax, y+ay)) == reference.getpixel((ax, ay))
                                           for ax, ay in anchors):
                                        candidates.append((x, y))
                            assert candidates, (label, 'upright direct image absent')
                            x, y = next((x, y) for x, y in candidates
                                        if rendered.crop((x, y, x+reference.width, y+reference.height)).tobytes()
                                        == reference.tobytes())
                        else:
                            x, y = 0, marker_bottom(rendered)
                            exact(rendered.crop((x, y, x+reference.width, y+reference.height)), reference, label)
                            # Natural dimensions affect the following empty page.
                            assert rendered.getpixel((0, y+reference.height)) == (255, 255, 255), label
                            assert rendered.getpixel((reference.width, y)) == (255, 255, 255), label
                        rendered.save(work/(label+'.png')); output.unlink()
                        browser_checks.append(dict(case=label, upright_size=reference.size, image_rectangle=[x,y],
                                                   animation_finished='animated' in path or 'apng' in path,
                                                   maximum_channel_error=0, http_requests=sum(requests.values())))
        report = dict(passed=True, decoder_checks=len(checks), browser_checks=len(browser_checks),
                      independent_reference='Pillow transforms and PNG/APNG/libwebp lossless composited pixels; lossy decoder geometry is checked against its unrotated output',
                      loopback_only=True, elapsed_seconds=round(time.monotonic()-started, 3),
                      results=checks, presentation=browser_checks)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2)+'\n', encoding="utf-8")
        print(json.dumps({k:v for k,v in report.items() if k not in ('results','presentation')}, indent=2))


if __name__ == '__main__':
    main()
