#!/usr/bin/env python3
"""Check original QOI and Netpbm decoders against authored pixels and Pillow.

Development only: Pillow and ImageMagick never become browser dependencies.
Fixtures are generated from geometric patterns; no downloaded image is used.
References: https://qoiformat.org/qoi-specification.pdf and
https://netpbm.sourceforge.net/doc/ (PBM, PGM, PPM and PAM manuals).
"""
from __future__ import annotations

import argparse
from io import BytesIO
import json
from pathlib import Path
import random
import shutil
import struct
import subprocess
import tempfile

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
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
        Err(error)=>{ println!("ERR {} {}",calls.get(),error);std::process::exit(2); }
    }
}
'''


def qoi_file(width, height, payload, channels=4, colorspace=0):
    return b'qoif' + struct.pack('>II', width, height) + bytes([channels, colorspace]) + payload + b'\0' * 7 + b'\1'


def sample_pixels(width, height, maximum, channels):
    return [tuple(((x * 47 + y * 79 + c * 139) * (maximum + 1) // 256) % (maximum + 1)
                  for c in range(channels))
            for y in range(height) for x in range(width)]


def expected_rgba(samples, maximum):
    result = bytearray()
    for sample in samples:
        values = [(n * 255 + maximum // 2) // maximum for n in sample]
        result.extend(values[:3] if len(values) >= 3 else values[:1] * 3)
        result.append(values[-1] if len(values) in (2, 4) else 255)
    return bytes(result)


def netpbm(kind, width, height, samples, maximum=255, comments=True):
    channels = len(samples[0])
    comment = b'# Original ChezCat fixture\n' if comments else b''
    if kind == 7:
        tuples = {1:'GRAYSCALE', 2:'GRAYSCALE_ALPHA', 3:'RGB', 4:'RGB_ALPHA'}
        header = (b'P7\n' + comment +
                  f'WIDTH {width}\nHEIGHT {height}\nDEPTH {channels}\nMAXVAL {maximum}\nTUPLTYPE {tuples[channels]}\nENDHDR\n'.encode())
    else:
        header = f'P{kind}\n'.encode() + comment + f'{width} {height}\n'.encode()
        if kind not in (1, 4):
            header += f'{maximum}\n'.encode()
    if kind == 1:
        return header + b'\n'.join(bytes(49 - samples[y * width + x][0] for x in range(width))
                                   for y in range(height)) + b'\n'
    if kind == 4:
        out = bytearray((width + 7) // 8 * height)
        for y in range(height):
            for x in range(width):
                out[y * ((width + 7) // 8) + x // 8] |= (1 - samples[y * width + x][0]) << (7 - x % 8)
        return header + out
    values = [n for sample in samples for n in sample]
    if kind <= 3:
        return header + ' '.join(map(str, values)).encode() + b'\n'
    return header + (bytes(values) if maximum < 256 else struct.pack('>' + 'H' * len(values), *values))


def write_demos(folder):
    folder.mkdir(parents=True, exist_ok=True)
    width, height = 240, 144
    rgba = []
    gray = []
    for y in range(height):
        for x in range(width):
            inside = ((x - 120) / 104) ** 2 + ((y - 72) / 58) ** 2 < 1
            color = ((66, 199, 188) if ((x // 24 + y // 24) & 1) else (255, 194, 86))
            alpha = 255 if inside else 0
            rgba.append((*color, alpha))
            gray.append((int(65535 * x / (width - 1)),))
    image = Image.new('RGBA', (width, height))
    image.putdata(rgba)
    image.save(folder / 'format-pattern.qoi')
    (folder / 'format-alpha.pam').write_bytes(netpbm(7, width, height, rgba))
    (folder / 'format-gray16.pgm').write_bytes(netpbm(5, width, height, gray, 65535))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--work-dir', type=Path)
    parser.add_argument('--write-demos', action='store_true')
    args = parser.parse_args()
    work = args.work_dir or Path(tempfile.mkdtemp(prefix='chezcat-simple-images-'))
    work.mkdir(parents=True, exist_ok=True)
    source = PROBE
    for key, name in [('IMAGE', 'image.rs'), ('JPEG', 'jpeg.rs'), ('COMPRESSION', 'compression.rs')]:
        source = source.replace('path=' + key, 'path=' + json.dumps(str(ROOT / 'src' / name)))
    (work / 'probe.rs').write_text(source, encoding="utf-8")
    executable = work / ('probe.exe' if __import__('os').name == 'nt' else 'probe')
    subprocess.run(['rustc', '--edition=2021', '-O', str(work / 'probe.rs'), '-o', str(executable)], check=True)
    input_path, output_path = work / 'input.img', work / 'pixels.bin'
    counts = {'qoi':0, 'netpbm':0, 'external_oracle':0, 'rejections':0, 'mutations':0, 'cancellations':0}
    maximum_difference = 0
    seeds = []

    def run(data, stop=0):
        input_path.write_bytes(data)
        process = subprocess.run([str(executable), str(input_path), str(output_path), str(stop)],
                                 capture_output=True, text=True, timeout=5)
        if process.returncode not in (0, 2):
            raise AssertionError(f'Probe crash: {process.returncode}: {process.stdout} {process.stderr}')
        return process

    def valid(name, data, size, rgba, family, oracle=False, tolerance=0):
        nonlocal maximum_difference
        process = run(data)
        assert process.returncode == 0, (name, process.stdout)
        raw = output_path.read_bytes()
        assert struct.unpack('<II', raw[:8]) == size, name
        bgra = raw[8:]
        got = bytearray(len(bgra))
        got[0::4], got[1::4], got[2::4], got[3::4] = bgra[2::4], bgra[1::4], bgra[0::4], bgra[3::4]
        assert len(got) == len(rgba), name
        difference = max((abs(a-b) for a,b in zip(got,rgba)), default=0)
        assert difference <= tolerance, f'{name}: pixel mismatch, maximum channel difference {difference}'
        maximum_difference = max(maximum_difference,difference)
        counts[family] += 1
        counts['external_oracle'] += int(oracle)
        seeds.append(data)

    def bad(name, data, stop=0):
        process = run(data, stop)
        assert process.returncode == 2, (name, process.stdout)
        if stop:
            assert 'cancelled' in process.stdout, (name, process.stdout)
            counts['cancellations'] += 1
        else:
            counts['rejections'] += 1

    rng = random.Random(280028)
    for width, height in [(1, 1), (2, 7), (9, 5), (31, 17), (64, 64), (129, 65)]:
        for mode in ('RGB', 'RGBA'):
            for pattern in range(6):
                components = 3 if mode == 'RGB' else 4
                samples = []
                for y in range(height):
                    for x in range(width):
                        if pattern == 0:
                            sample = [66, 199, 188, 255]
                        elif pattern == 1:
                            sample = [0, 0, 0, (x + y) % 256]
                        elif pattern == 2:
                            sample = [(x + c + y) & 255 for c in range(4)]
                        elif pattern == 3:
                            sample = [(x * 7 + y * 5 + c * 11) & 255 for c in range(4)]
                        elif pattern == 4:
                            sample = [(255 if (x // 3 + y // 2 + c) % 2 else 0) for c in range(4)]
                        else:
                            sample = [rng.randrange(256) for _ in range(4)]
                        samples.append(tuple(sample[:components]))
                image = Image.new(mode, (width, height))
                image.putdata(samples)
                out = BytesIO()
                image.save(out, format='QOI')
                data = out.getvalue()
                reference = Image.open(BytesIO(data)).convert('RGBA').tobytes()
                valid(f'QOI {mode} {width}x{height} pattern{pattern}', data, (width,height), reference, 'qoi', True)
                linear = data[:13] + b'\1' + data[14:]
                valid('QOI linear colorspace samples', linear, (width,height), reference, 'qoi', True)

    # The initial index is transparent black, distinct from the opaque current pixel.
    valid('QOI initial index', qoi_file(1,1,b'\0'), (1,1), bytes(4), 'qoi')
    valid('QOI maximal initial run', qoi_file(62,1,b'\xfd'), (62,1), bytes([0,0,0,255])*62, 'qoi')
    for width,height in [(1,1),(9,3),(31,17)]:
        for kind in (1,4):
            samples = [((x+y)%2,) for y in range(height) for x in range(width)]
            data = netpbm(kind,width,height,samples,1)
            reference = Image.open(BytesIO(data)).convert('RGBA').tobytes()
            valid(f'P{kind} packed/unpacked {width}',data,(width,height),reference,'netpbm',True)
        for maximum in (1,7,255,1000,65535):
            for kind in (2,3,5,6,7):
                depths = (1,2,3,4) if kind == 7 else (3 if kind in (3,6) else 1,)
                for depth in depths:
                    samples = sample_pixels(width,height,maximum,depth)
                    data = netpbm(kind,width,height,samples,maximum)
                    reference = expected_rgba(samples,maximum)
                    valid(f'P{kind} {width} max{maximum} depth{depth}',data,(width,height),reference,'netpbm')
    # Raw data starts immediately after one whitespace separator; do not eat a
    # legal first sample that happens to be whitespace or the comment marker.
    for separator in (b'\r',b'\n',b' ',b'\t',b'\v',b'\f'):
        for sample in (10,13,32,35):
            data = b'P5\n1 1\n255'+separator+bytes([sample])
            valid('Netpbm raster boundary',data,(1,1),bytes([sample]*3+[255]),'netpbm')
    for data in (b'P5\n1\v1\n255\n\x42', b'P7\nWIDTH\v1\nHEIGHT 1\nDEPTH 1\nMAXVAL 255\nENDHDR\n\x42'):
        valid('Netpbm vertical tab in header',data,(1,1),bytes([66,66,66,255]),'netpbm')
    data=b'P6\n1 1\n255# maximum\n\x42\xc7\xbc'
    valid('Netpbm final comment',data,(1,1),bytes([66,199,188,255]),'netpbm')
    data=b'P3\n1 1\n255\n66 199 188\nP3\n1 1\n255\n0 0 0\n'
    valid('Netpbm first image in stream',data,(1,1),bytes([66,199,188,255]),'netpbm')
    for depth in (1,2):
        samples=[tuple([n%2]*depth) for n in range(6)]
        data=netpbm(7,3,2,samples,1).replace(b'GRAYSCALE',b'BLACKANDWHITE')
        valid('PAM bilevel with optional alpha',data,(3,2),expected_rgba(samples,1),'netpbm')

    # ImageMagick provides an independent PAM/16-bit Netpbm decode where present.
    convert = shutil.which('magick') or shutil.which('convert')
    if convert:
        for kind,depth in [(5,1),(6,3),(7,1),(7,2),(7,3),(7,4)]:
            for maximum in (255,65535):
                samples=sample_pixels(11,7,maximum,depth)
                data=netpbm(kind,11,7,samples,maximum)
                path=work / ('reference.pam' if kind==7 else 'reference.pnm')
                path.write_bytes(data)
                oracle=subprocess.run([convert,str(path),'-depth','8','RGBA:-'],capture_output=True,check=True).stdout
                # ImageMagick truncates 16-bit output where ChezCat rounds to
                # nearest. Authored-sample checks above still require equality.
                valid(f'ImageMagick P{kind} depth{depth} max{maximum}',data,(11,7),oracle,'netpbm',True,1 if maximum>255 else 0)

    valid_seed=qoi_file(1,1,bytes([255,66,199,188,170]))
    for n in range(len(valid_seed)):
        bad(f'truncated QOI at{n}',valid_seed[:n])
    bad('QOI zero width',qoi_file(0,1,b''))
    bad('QOI oversize',qoi_file(16385,1,b'\xc0'))
    bad('QOI missing pixel',qoi_file(1,1,b''))
    bad('QOI excessive run',qoi_file(1,1,b'\xfd'))
    bad('QOI truncated luma',qoi_file(1,1,b'\x80'))
    bad('QOI extra chunk',qoi_file(1,1,b'\xc0\xc0'))
    bad('QOI repeated index',qoi_file(2,1,b'\0\0'))
    bad('QOI wrong channels',qoi_file(1,1,b'\xc0',2))
    bad('QOI wrong colorspace',qoi_file(1,1,b'\xc0',4,2))
    for malformed in [b'P5\n0 1\n255\n\0',b'P6\n1 1\n0\n\0\0\0',b'P5\n1 1\n65536\n\0\0',
                      b'P2\n1 1\n7\n8',b'P5\n1 1\n7\n\x08',b'P3\n1 1\n255\n1 2',
                      b'P1\n1 1\n2',b'P5\n16385 1\n255\n',b'P6\n1 1\n255\n\0\0',
                      b'P7\n'+b' '*65536+b'\nENDHDR\n',
                      b'P7\nWIDTH 1\nWIDTH 2\nENDHDR\n',
                      b'P7\nWIDTH 1\nHEIGHT 1\nDEPTH 5\nMAXVAL 255\nENDHDR\n'+bytes(5),
                      b'P7\nWIDTH 1\nHEIGHT 1\nDEPTH 3\nMAXVAL 255\nTUPLTYPE\nENDHDR\n'+bytes(3),
                      b'P7\nWIDTH 1\nHEIGHT 1\nDEPTH 3\nMAXVAL 255\nTUPLTYPE LAB\nENDHDR\n'+bytes(3)]:
        bad('malformed Netpbm',malformed)
    for data in [qoi_file(8192,1,bytes([0xfd])*132+bytes([0xc7])),
                 b'P1\n8192 1\n'+b'1'*8192,
                 netpbm(7,128,64,sample_pixels(128,64,255,4))]:
        bad('decoder cancellation',data,2)
    for _ in range(240):
        data=bytearray(rng.choice(seeds))
        for _ in range(rng.randrange(1,7)):
            if data:
                data[rng.randrange(len(data))]=rng.randrange(256)
        if rng.random()<0.3:
            del data[rng.randrange(len(data)):]
        run(data)
        counts['mutations']+=1
    if args.write_demos:
        write_demos(ROOT / 'pages' / 'images')
        for name in ('format-pattern.qoi','format-alpha.pam','format-gray16.pgm'):
            path=ROOT / 'pages' / 'images' / name
            if path.suffix=='.pam':
                if convert:
                    reference=subprocess.run([convert,str(path),'-depth','8','RGBA:-'],capture_output=True,check=True).stdout
                    valid(name,path.read_bytes(),(240,144),reference,'netpbm',True)
            elif path.suffix=='.qoi':
                reference=Image.open(path).convert('RGBA').tobytes()
                valid(name,path.read_bytes(),(240,144),reference,'qoi',True)
            else:
                valid(name,path.read_bytes(),(240,144),expected_rgba([(int(65535*x/239),) for y in range(144) for x in range(240)],65535),'netpbm')
    report={'passed':True,'counts':counts,'production_dispatcher':True,
            'external_decoders':'Pillow QOI/PBM; ImageMagick PAM/PGM/PPM when available',
            'max_channel_difference':maximum_difference,'work_directory':str(work)}
    if args.output:
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text(json.dumps(report,indent=2)+'\n', encoding="utf-8")
    print(json.dumps(report,indent=2))


if __name__ == '__main__':
    main()
