#!/usr/bin/env python3
"""Independent GIF decoder checks and original workshop image generation.

Pillow encodes and decodes the comparison images only during development. The
compiled Rust probe uses ChezCat's actual image dispatcher and GIF decoder.
No Python, Pillow or external GIF decoder is used by the browser.

Run: python tools/check_gif.py [--rustc PATH] [--output report.json]
     python tools/check_gif.py --write-demos

Primary format reference: https://www.w3.org/Graphics/GIF/spec-gif89a.txt
"""

from __future__ import annotations

import argparse
from collections import Counter
from dataclasses import dataclass
from io import BytesIO
import json
from pathlib import Path
import random
import struct
import subprocess
import tempfile
import time

import PIL
from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parents[1]
PROBE = r'''
#![allow(dead_code)]
#[path=IMAGE] mod image;
#[path=JPEG] mod jpeg;
#[path=COMPRESSION] mod compression;
use std::cell::Cell;
use std::io::{self, BufRead, Write};
fn main() {
    let mut output=io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let line=line.unwrap(); let fields:Vec<_>=line.split('\t').collect();
        let stop:usize=fields[2].parse().unwrap(); let count=Cell::new(0usize);
        let check=||{count.set(count.get()+1);
            if stop!=0 && count.get()>=stop {Err("cancelled".to_string())} else {Ok(())}};
        let data=std::fs::read(fields[0]).unwrap();
        let start=std::time::Instant::now();
        match image::decode_checked(&data,&check) {
            Ok(bitmap)=>{
                assert_eq!(bitmap.pixels.len(),bitmap.width*bitmap.height);
                let mut bytes=Vec::with_capacity(bitmap.pixels.len()*4);
                for color in bitmap.pixels {
                    bytes.extend_from_slice(&[(color>>16)as u8,(color>>8)as u8,color as u8,(color>>24)as u8]);
                }
                std::fs::write(fields[1],bytes).unwrap();
                writeln!(output,"OK\t{}\t{}\t{}\t{}",bitmap.width,bitmap.height,count.get(),start.elapsed().as_micros()).unwrap();
            }
            Err(error)=>writeln!(output,"ERR\t{}\t{}",count.get(),error.replace(['\t','\r','\n']," ")).unwrap(),
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
    error: str | None = None
    stop: int = 0


def encoded(image, **options):
    out = BytesIO()
    image.save(out, 'GIF', **options)
    return out.getvalue()


def decoded(data):
    with Image.open(BytesIO(data)) as image:
        image.seek(0)
        return image.convert('RGBA')


def image_position(data):
    pos = 13 + (3 * (1 << ((data[10] & 7) + 1)) if data[10] & 128 else 0)
    while data[pos] != 0x2c:
        assert data[pos] == 0x21
        pos += 2
        while data[pos]:
            pos += 1 + data[pos]
        pos += 1
    return pos


def table_end(data, pos):
    return pos + 10 + (3 * (1 << ((data[pos+9] & 7) + 1)) if data[pos+9] & 128 else 0)


def subblocks(data, sizes=(255,)):
    out = bytearray()
    pos = n = 0
    while pos < len(data):
        size = min(sizes[n % len(sizes)], len(data) - pos)
        out.append(size)
        out.extend(data[pos:pos+size])
        pos += size
        n += 1
    return bytes(out) + b'\0'


def repack_blocks(data, sizes, padding=b''):
    pos = table_end(data, image_position(data)) + 1
    start = pos
    raw = bytearray()
    while data[pos]:
        length = data[pos]
        raw.extend(data[pos+1:pos+1+length])
        pos += length + 1
    return data[:start] + subblocks(raw + padding, sizes) + data[pos+1:]


def raw_gif(width, height, codes, minimum=2, palette=None, screen=None,
            offset=(0, 0), interlaced=False, transparent=None):
    """Small original explicit-code fixture writer; Pillow validates successes.

    `codes` may be (integer,width) pairs, so malformed and dictionary-full cases
    do not depend on the production decoder's code-width calculations.
    """
    if palette is None:
        palette = [(17, 33, 49), (210, 80, 40), (20, 180, 90), (50, 90, 210)]
    count = 2
    while count < len(palette):
        count *= 2
    palette = list(palette) + [(0, 0, 0)] * (count-len(palette))
    size = count.bit_length() - 2
    screen = screen or (width, height)
    out = bytearray(b'GIF89a' + struct.pack('<HHBBB', *screen, 0x80 | size, 0, 0))
    out.extend(bytes(v for rgb in palette for v in rgb))
    if transparent is not None:
        out.extend(b'\x21\xf9\x04\x01\0\0' + bytes([transparent]) + b'\0')
    out.extend(b',' + struct.pack('<HHHHB', *offset, width, height, 0x40 if interlaced else 0))
    out.append(minimum)
    packed = bytearray()
    buffer = bits = 0
    for item in codes:
        code, width_bits = item if isinstance(item, tuple) else (item, minimum+1)
        buffer |= code << bits
        bits += width_bits
        while bits >= 8:
            packed.append(buffer & 255)
            buffer >>= 8
            bits -= 8
    if bits:
        packed.append(buffer)
    out.extend(subblocks(packed))
    out.append(0x3b)
    return bytes(out)


def cases():
    result = []
    def good(label, data, group='pillow', reference=None):
        result.append(Case(label, data, decoded(data) if reference is None else reference, group))
    def bad(label, data, error=None, group='reject', stop=0):
        result.append(Case(label, data, None, group, error, stop))

    randomizer = random.Random(17903)
    palette = [v for n in range(256) for v in ((n*17) % 256, (n*71) % 256, (n*137) % 256)]
    for colors in (2, 4, 16, 128, 256):
        for width, height in ((1,1), (3,17), (32,23), (257,131)):
            image = Image.new('P', (width,height))
            image.putpalette(palette)
            image.putdata([randomizer.randrange(colors) for _ in range(width*height)])
            for interlaced in (False, True):
                data = encoded(image, optimize=True, interlace=interlaced)
                good(f'palette{colors}-{width}x{height}-interlace{interlaced}', data)
                if width == 32:
                    good(f'transparent{colors}-interlace{interlaced}', encoded(image, optimize=False, interlace=interlaced, transparency=colors-1))

    for pattern in ('constant', 'alternating', 'rows', 'random'):
        image = Image.new('P', (512,256))
        image.putpalette(palette)
        values = [0 if pattern=='constant' else i%2 if pattern=='alternating'
                  else (i//512)%256 if pattern=='rows' else randomizer.randrange(256)
                  for i in range(512*256)]
        image.putdata(values)
        data = encoded(image, optimize=False, interlace=False)
        good('dictionary-'+pattern, data, 'lzw')
        if pattern == 'random':
            small=encoded(image.crop((0,0,128,128)),optimize=False,interlace=False)
            good('one-byte-subblocks', repack_blocks(small, (1,)), 'blocks')
            good('uneven-subblocks', repack_blocks(data, (1,2,7,255,3,17)), 'blocks')
        if pattern == 'constant':
            base = data

    simple = raw_gif(3, 1, [4, 0, 6, 5])
    good('kwkwk-special-entry', simple, 'lzw')
    good('no-initial-clear', raw_gif(1,1,[1,5]), 'lzw')
    good('frequent-clears', raw_gif(4,1,[4,0,4,1,4,2,4,3,5]), 'lzw')
    good('consecutive-clears', raw_gif(1,1,[4,4,4,3,5]), 'lzw')
    # Original legacy-encoder regressions. The complete raster still requires
    # checked GIF sub-block framing and the outer trailer. Only whole ZERO
    # bytes may follow EOI; an omitted/partial EOI is tolerated solely when a
    # real zero-length data terminator follows all expected output pixels.
    one_pixel = raw_gif(1,1,[4,0,5])
    byte_aligned_end = raw_gif(3,1,[4,0,4,1,4,2,4,5])
    for label, data in (('partial-byte-eoi', one_pixel), ('byte-aligned-eoi', byte_aligned_end)):
        good(label+'-zero-padding', repack_blocks(data,(255,),b'\0'), 'compatibility')
        good(label+'-zero-padding-subblocks', repack_blocks(data,(1,2,7),b'\0'*520), 'compatibility')
        bad(label+'-nonzero-tail', repack_blocks(data,(255,),b'\1'), 'Extra GIF')
        bad(label+'-nonzero-later-subblock', repack_blocks(data,(1,),b'\0\0\1'), 'Extra GIF')
    no_end = raw_gif(1,1,[4,0])
    partial_end = raw_gif(1,1,[(4,3),(0,3),(1,2)])
    good('complete-raster-omitted-eoi', no_end, 'compatibility')
    good('complete-raster-partial-eoi', partial_end, 'compatibility')
    for label, data in (('no-end', no_end), ('partial-end', partial_end),
                        ('padded-end', repack_blocks(one_pixel,(255,),b'\0'))):
        bad(label+'-missing-trailer', data[:-1], 'Truncated')
        bad(label+'-missing-terminator', data[:-2]+b';', 'Truncated')
        bad(label+'-raw-eof', data[:-2], 'Truncated')
    bad('short-raster-without-eoi', raw_gif(2,1,[4,0]), 'end code')
    bad('complete-raster-then-invalid-code', raw_gif(1,1,[4,0,7]), 'undefined dictionary')
    bad('complete-raster-then-extra-pixel', raw_gif(1,1,[4,0,0]), 'exceeds its image')
    bad('padded-declared-block-truncated', one_pixel[:-2]+b'\x05\0\0', 'Truncated')
    padding_cancel = one_pixel[:-2]+subblocks(b'\0'*16384)+b';'
    bad('cancel-in-zero-padding', padding_cancel, 'cancelled', 'cancel', stop=6)
    bad('zero-padding-subblock-limit', one_pixel[:-2]+b'\x01\0'*65536+b'\0;', 'sub-block limit', 'bounds')
    p = image_position(one_pixel)
    good('later-complete-raster-omitted-eoi', one_pixel[:-1]+no_end[p:], 'compatibility')
    good('later-zero-padding', one_pixel[:-1]+repack_blocks(one_pixel,(1,),b'\0\0')[p:], 'compatibility')
    bad('later-short-raster-without-eoi', raw_gif(2,1,[4,0,4,0,5])[:-1]+raw_gif(2,1,[4,0])[p:], 'end code')
    # Explicit code widths independently traverse every dictionary width and
    # keep using a full dictionary without sending clear, as GIF requires.
    codes = [(256,9)]
    width_bits = 9
    available = 258
    for n in range(6000):
        codes.append((n % 256,width_bits))
        if n and available < 4096:
            available += 1
            if available == 1 << width_bits and width_bits < 12:
                width_bits += 1
    codes.append((257,width_bits))
    full = raw_gif(100,60,codes,minimum=8,palette=list(zip(palette[::3],palette[1::3],palette[2::3])))
    good('full-dictionary-deferred-clear', full, 'lzw')

    for height in (1,2,3,4,5,7,8,9,17):
        rows = [y for start,step in ((0,8),(4,8),(2,4),(1,2)) for y in range(start,height,step)]
        codes = [value for y in rows for value in (4,y%4)] + [5]
        good(f'explicit-interlace-{height}', raw_gif(1,height,codes,interlaced=True), 'interlace')

    for transparent in (None,0,1):
        data = raw_gif(2,2,[4,0,4,1,4,1,4,0,5],screen=(5,4),offset=(2,1),transparent=transparent)
        good('offset-'+str(transparent), data, 'canvas')
    background = bytearray(raw_gif(1,1,[4,0,5],screen=(3,3),offset=(1,1)))
    background[11] = 2
    # Pillow initializes an opaque uncovered screen from palette index0, even
    # when the GIF declares a different background. Our documented policy uses
    # the declared global background, per the logical-screen descriptor.
    reference = Image.new('RGBA',(3,3),(20,180,90,255)); reference.putpixel((1,1),(17,33,49,255))
    good('declared-background', bytes(background), 'spec-background', reference)
    local = bytearray(simple)
    position = image_position(local)
    table = local[13:position]
    local = local[:13] + local[position:]
    local[10] &= 0x7f
    local[22] |= 0x81
    local = local[:23] + table + local[23:]
    good('local-table-only', bytes(local), 'palette')
    overridden=bytearray(simple)
    p=image_position(overridden)
    overridden[p+9] |= 0x81
    overridden=overridden[:p+10]+bytes(reversed(table))+overridden[p+10:]
    good('local-table-overrides-global',bytes(overridden),'palette')
    good('gif87a', b'GIF87a' + simple[6:], 'format')

    before = simple[:image_position(simple)]
    after = simple[image_position(simple):]
    comment = b'\x21\xfe' + subblocks(b'A comment with an embedded trailer ; and image marker ,')
    # Application identifier must be exactly 11 bytes.
    application = b'\x21\xff\x0b' + b'OWNBROWS001' + subblocks(b'ignored application payload')
    good('comments-and-application', before+comment+application+after, 'blocks')
    transparent = raw_gif(1,1,[4,0,5],transparent=0)
    p = image_position(transparent)
    good('control-survives-comment', transparent[:p]+comment+transparent[p:], 'blocks')

    first = Image.new('P',(40,32),0); first.putpalette(palette)
    second = first.copy(); ImageDraw.Draw(second).rectangle((8,8,25,25),fill=5)
    third = first.copy(); ImageDraw.Draw(third).rectangle((3,3,18,21),fill=9)
    for disposal in (0,1,2,3):
        animation = encoded(first, save_all=True, append_images=[second,third], duration=[70,80,90], loop=0, disposal=disposal, optimize=True)
        good('animation-first-frame-disposal'+str(disposal), animation, 'animation')
    for cut in range(len(simple)):
        bad('truncated-'+str(cut), simple[:cut], group='truncated')
    for suffix in (b'\0', b'junk', simple):
        bad('trailing-'+str(len(suffix)), simple+suffix, 'trailing data')
    for size in (0,1,9,255):
        data = bytearray(simple); data[table_end(data,image_position(data))] = size
        bad('minimum-code-size-'+str(size),bytes(data),'minimum code size')
    for width,height in ((0,1),(1,0),(16385,1),(4096,4096)):
        data=bytearray(simple);data[6:10]=struct.pack('<HH',width,height)
        bad(f'logical-bounds-{width}-{height}',bytes(data),'dimension limit')
    data=bytearray(simple);p=image_position(data);data[p+1:p+3]=struct.pack('<H',3)
    bad('outside-screen',bytes(data),'outside the logical screen')
    data=bytearray(simple);data[11]=7
    bad('invalid-background',bytes(data),'background index')
    data=raw_gif(1,1,[4,3,5],palette=[(0,0,0),(255,255,255)])
    bad('invalid-pixel-index',data,'pixel index')
    # Transparent indices do not perform color lookups; an unused transparent
    # index outside the active table does not invalidate visible image samples.
    good('unused-transparent-index',raw_gif(1,1,[4,0,5],transparent=7),'palette')
    bad('undefined-first-code',raw_gif(1,1,[4,6,5]),'undefined dictionary')
    bad('undefined-later-code',raw_gif(2,1,[4,0,7,5]),'undefined dictionary')
    bad('short-raster',raw_gif(2,1,[4,0,5]),'shorter')
    bad('long-raster',raw_gif(1,1,[4,0,1,5]),'exceeds its image')
    p=image_position(simple)
    bad('unknown-extension',simple[:p]+b'\x21\xef\0'+simple[p:],'extension type')
    bad('plaintext-extension',simple[:p]+b'\x21\x01'+simple[p:],'plain-text')
    bad('bad-application-size',simple[:p]+b'\x21\xff\x0a'+simple[p:],'11 bytes')
    control=b'\x21\xf9\x04\x01\0\0\0\0'
    bad('duplicate-control',simple[:p]+control+control+simple[p:],'duplicate')
    bad('orphan-control',simple[:-1]+control+b';','unused')
    bad('reserved-disposal',simple[:p]+control[:3]+b'\x10'+control[4:]+simple[p:],'reserved')
    bad('reserved-control-bits',simple[:p]+control[:3]+b'\x80'+control[4:]+simple[p:],'reserved')
    bad('missing-control-terminator',simple[:p]+control[:-1]+b'\x02'+simple[p:],'terminator')
    bad('reserved-image-bits',simple[:p+9]+bytes([simple[p+9]|8])+simple[p+10:],'reserved')
    # Later images are validated even though only frame0 is retained.
    bad('later-image-bad-code',simple[:-1]+raw_gif(1,1,[4,7,5])[p:],'undefined')
    bad('later-image-truncated',simple[:-1]+simple[p:-3],'Truncated')
    bad('image-count',simple[:p]+simple[p:-1]*257+b';','exceeds 256','bounds')
    bad('subblock-count',simple[:p]+b'\x21\xfe'+b'\x01a'*65537+b'\0'+simple[p:],'sub-block limit','bounds')
    bad('input-size',b'GIF89a'+b'\0'*(32*1024*1024),'32 MiB','bounds')
    # A long stream of clear codes performs no output; the shared work budget
    # must still stop it. Use four-bit codes and 255-byte blocks below the block cap.
    work_header=raw_gif(1,1,[(8,4),(0,4),(9,4)],minimum=3)
    code_position=table_end(work_header,image_position(work_header))+1
    work_bomb=work_header[:code_position]+subblocks(b'\x88'*12_000_000+b'\x90')+b';'
    bad('clear-code-work-limit',work_bomb,'work limit','bounds')
    for stop in (1,2,3,5,20):
        bad('cancel-'+str(stop),base,'cancelled','cancel',stop=stop)
    return result


def write_demos():
    folder=ROOT/'pages/images'; folder.mkdir(exist_ok=True)
    palette=[24,57,74, 143,226,198, 238,248,252, 244,165,78] + [0]*756
    image=Image.new('P',(240,128),0); image.putpalette(palette)
    draw=ImageDraw.Draw(image)
    draw.rectangle((16,16,223,111),outline=1,width=3)
    for x in range(24,212,28): draw.rectangle((x,28,x+15,58),fill=2)
    draw.rectangle((24,78,209,98),fill=3)
    image.save(folder/'gif-palette.gif',optimize=False,interlace=False)
    image.save(folder/'gif-interlaced.gif',optimize=False,interlace=True)
    transparent=Image.new('P',(160,128),0); transparent.putpalette(palette)
    draw=ImageDraw.Draw(transparent)
    draw.ellipse((18,12,142,116),fill=1)
    draw.ellipse((42,36,118,92),fill=0)
    draw.rectangle((68,4,92,124),fill=3)
    transparent.save(folder/'gif-transparent.gif',transparency=0,optimize=False)
    frames=[]
    for active in range(3):
        frame=Image.new('P',(192,96),0); frame.putpalette(palette)
        draw=ImageDraw.Draw(frame)
        for index in range(3):
            x=16+index*58
            draw.rectangle((x,24,x+43,71),fill=1 if index==active else 2)
        frames.append(frame)
    frames[0].save(folder/'gif-first-frame.gif',save_all=True,append_images=frames[1:],duration=500,loop=0,optimize=False)
    return [str(folder/name) for name in ('gif-palette.gif','gif-interlaced.gif','gif-transparent.gif','gif-first-frame.gif')]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rustc',default='rustc')
    parser.add_argument('--output',type=Path)
    parser.add_argument('--write-demos',action='store_true')
    args=parser.parse_args()
    if args.write_demos:
        print(json.dumps({'demo_files':write_demos()},indent=2))
    checks=cases()
    results=[]
    with tempfile.TemporaryDirectory(prefix='chezcat-gif-') as directory:
        directory=Path(directory)
        source=PROBE
        for name in ('image','jpeg','compression'):
            source=source.replace(name.upper(),json.dumps(str(ROOT/'src'/f'{name}.rs')))
        (directory/'probe.rs').write_text(source, encoding="utf-8")
        executable=directory/('probe.exe' if __import__('os').name=='nt' else 'probe')
        subprocess.run([args.rustc,'--edition=2021','-O',str(directory/'probe.rs'),'-o',str(executable)],check=True)
        process=subprocess.Popen([str(executable)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
        try:
            for index,case in enumerate(checks):
                path=directory/f'{index}.gif'; path.write_bytes(case.data)
                output=directory/f'{index}.rgba'
                start=time.monotonic()
                process.stdin.write(f'{path}\t{output}\t{case.stop}\n');process.stdin.flush()
                line=process.stdout.readline().rstrip('\n')
                parts=line.split('\t')
                ok=parts[0]=='OK'
                if case.reference is None:
                    assert not ok and parts[0]=='ERR', (case.label,line)
                    assert case.error is None or case.error in line, (case.label,line,case.error)
                else:
                    assert ok, (case.label,line)
                    assert (int(parts[1]),int(parts[2]))==case.reference.size, (case.label,line)
                    actual=output.read_bytes(); expected=case.reference.tobytes()
                    assert actual==expected, (case.label,'pixel mismatch',next((i for i,(a,b) in enumerate(zip(actual,expected)) if a!=b),None))
                results.append({'case':case.label,'group':case.group,'bytes':len(case.data),
                                'elapsed_seconds':round(time.monotonic()-start,6),'result':line})
        finally:
            process.stdin.close()
            if process.poll() is None: process.wait(timeout=5)
        assert process.returncode==0
    report={'checks':len(results),'passed':len(results),'groups':dict(Counter(x['group'] for x in results)),
            'pillow':PIL.__version__,'comparison':'Exact first-frame RGBA pixels against Pillow, except the explicit declared-background fixture which follows the GIF logical-screen descriptor.',
            'results':results}
    if args.output:
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text(json.dumps(report,indent=2)+'\n', encoding="utf-8")
    print(json.dumps({key:value for key,value in report.items() if key!='results'},indent=2))


if __name__=='__main__':
    main()
