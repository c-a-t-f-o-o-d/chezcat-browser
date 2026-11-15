#!/usr/bin/env python3
"""Independent TIFF and four-component JPEG checks for ChezCat.

Development only. Generated pixels and TIFF containers are original. Pillow's
libtiff/libjpeg and ImageMagick are independent encoders/decoders, never browser
dependencies. No external codec implementation or downloaded sample is used.

TIFF 6.0: https://www.itu.int/itudoc/itu-t/com16/tiff-fx/docs/tiff6.pdf
TIFF/JPEG: https://libtiff.gitlab.io/libtiff/specification/technote2.html
"""
from __future__ import annotations

import argparse
from io import BytesIO
import hashlib
import json
from pathlib import Path
import random
import shutil
import struct
import subprocess
import tempfile
import zlib

from PIL import Image, ImageOps, TiffImagePlugin, features

ROOT = Path(__file__).resolve().parents[1]
PROBE = r'''
#![allow(dead_code)]
#[path=IMAGE] mod image;
#[path=JPEG] mod jpeg;
#[path=COMPRESSION] mod compression;
use std::io::Write;
use std::cell::Cell;
fn main() {
 let args:Vec<_>=std::env::args().collect();
 let data=std::fs::read(&args[1]).unwrap();
 let calls=Cell::new(0usize);
 let stop:usize=args.get(3).and_then(|s|s.parse().ok()).unwrap_or(0);
 let check=||{calls.set(calls.get()+1);if stop>0&&calls.get()>=stop{Err("cancelled".into())}else{Ok(())}};
 match image::decode_checked(&data,&check) {
  Ok(b)=>{
   assert_eq!(b.pixels.len(),b.width*b.height);
   let mut out=std::io::BufWriter::new(std::fs::File::create(&args[2]).unwrap());
   out.write_all(&(b.width as u32).to_le_bytes()).unwrap();
   out.write_all(&(b.height as u32).to_le_bytes()).unwrap();
   for pixel in b.pixels{out.write_all(&pixel.to_le_bytes()).unwrap()}
   println!("OK {}",calls.get());
  },
  Err(error)=>{println!("ERR {} {}",calls.get(),error);std::process::exit(2)}
 }
}
'''


def pixels(image):
    return list(image.get_flattened_data())


def lzw_encode(raw):
    table = {bytes([i]): i for i in range(256)}
    next_code = 258
    out = bytearray()
    pending = count = 0
    width, decoder_next, previous = 9, 258, False

    def emit(code):
        nonlocal pending, count, width, decoder_next, previous
        pending = (pending << width) | code
        count += width
        while count >= 8:
            count -= 8
            out.append((pending >> count) & 255)
        pending &= (1 << count) - 1
        if code == 256:
            width, decoder_next, previous = 9, 258, False
        elif code != 257:
            if previous and decoder_next < 4096:
                decoder_next += 1
                if width < 12 and decoder_next == (1 << width) - 1:
                    width += 1
            previous = True

    emit(256)
    word = b''
    for byte in raw:
        candidate = word + bytes([byte])
        if candidate in table:
            word = candidate
        else:
            emit(table[word])
            if next_code < 4096:
                table[candidate] = next_code
                next_code += 1
                if next_code == 4095:
                    emit(256)
                    table = {bytes([i]): i for i in range(256)}
                    next_code = 258
            word = bytes([byte])
    if word:
        emit(table[word])
    emit(257)
    if count:
        out.append((pending << (8 - count)) & 255)
    return bytes(out)


def packbits_encode(raw):
    out = bytearray()
    at = 0
    while at < len(raw):
        run = 1
        while run < 128 and at + run < len(raw) and raw[at + run] == raw[at]:
            run += 1
        if run >= 3:
            out.extend([(1 - run) & 255, raw[at]])
            at += run
        else:
            start = at
            at += run
            while at < len(raw) and at - start < 128:
                if raw[at:at + 3] == bytes([raw[at]]) * 3:
                    break
                at += 1
            out.append(at - start - 1)
            out.extend(raw[start:at])
    return bytes(out)


def tiff(width, height, samples, *, depth=8, photo=2, compression=1,
         little=True, planar=1, predictor=1, orientation=1, extra=None,
         tiled=False, rows=7, palette=None, fill=1):
    """Original classic-TIFF container with authored samples; returns bytes."""
    order = '<' if little else '>'
    spp = len(samples[0])
    maximum = (1 << depth) - 1
    bw, bh = (16, 16) if tiled else (width, min(rows, height))
    across, down = (width + bw - 1) // bw, (height + bh - 1) // bh
    chunks = []
    for plane in range(spp if planar == 2 else 1):
        for ty in range(down):
            for tx in range(across):
                raw = bytearray()
                for y in range(bh if tiled else min(bh, height - ty * bh)):
                    row = []
                    previous = [0] * (1 if planar == 2 else spp)
                    for x in range(bw):
                        px, py = tx * bw + x, ty * bh + y
                        value = samples[py * width + px] if px < width and py < height else (0,) * spp
                        value = (value[plane],) if planar == 2 else value
                        for c, component in enumerate(value):
                            encoded = (component - previous[c]) & maximum if predictor == 2 else component
                            previous[c] = component
                            row.append(encoded)
                    if depth == 16:
                        raw.extend(struct.pack(order + 'H' * len(row), *row))
                    else:
                        packed = bytearray((len(row) * depth + 7) // 8)
                        for i, value in enumerate(row):
                            bit = i * depth
                            packed[bit // 8] |= value << (8 - depth - bit % 8)
                        if fill == 2:
                            packed = bytearray(int(f'{b:08b}'[::-1], 2) for b in packed)
                        raw.extend(packed)
                chunks.append({1:lambda b:b, 5:lzw_encode, 8:zlib.compress,
                               32946:zlib.compress, 32773:packbits_encode}[compression](bytes(raw)))
    tags = {256:(4,[width]), 257:(4,[height]), 258:(3,[depth]*spp),
            259:(3,[compression]), 262:(3,[photo]), 266:(3,[fill]),
            274:(3,[orientation]), 277:(3,[spp]), 284:(3,[planar]),
            317:(3,[predictor]), 339:(3,[1]*spp)}
    offset_tag, length_tag = (324,325) if tiled else (273,279)
    tags[offset_tag] = (4,[0]*len(chunks))
    tags[length_tag] = (4,list(map(len,chunks)))
    if tiled:
        tags[322], tags[323] = (4,[bw]), (4,[bh])
    else:
        tags[278] = (4,[bh])
    if extra is not None:
        tags[338] = (3,[extra])
    if palette is not None:
        tags[320] = (3,palette)
    if photo == 5:
        tags[332], tags[334] = (3,[1]), (3,[4])
    table_end = 8 + 2 + len(tags)*12 + 4
    lengths = {tag:len(values)*({3:2,4:4}[kind]) for tag,(kind,values) in tags.items()}
    extra_size = sum((n+1)//2*2 for n in lengths.values() if n>4)
    position = table_end + extra_size
    offsets = []
    for chunk in chunks:
        offsets.append(position)
        position += len(chunk)
    tags[offset_tag] = (4,offsets)
    result = bytearray(b'II' if little else b'MM')
    result.extend(struct.pack(order+'HIH',42,8,len(tags)))
    external = bytearray()
    for tag,(kind,values) in sorted(tags.items()):
        raw = struct.pack(order + {3:'H',4:'I'}[kind]*len(values),*values)
        result.extend(struct.pack(order+'HHI',tag,kind,len(values)))
        if len(raw) <= 4:
            result.extend(raw.ljust(4,b'\0'))
        else:
            result.extend(struct.pack(order+'I',table_end+len(external)))
            external.extend(raw)
            if len(external)%2:
                external.append(0)
    result.extend(struct.pack(order+'I',0))
    return bytes(result+external+b''.join(chunks))


def expected_samples(width, height, values, depth, photo, *, extra=None, orientation=1, palette=None):
    maximum = (1 << depth) - 1
    out = []
    def scale(v):return (v*255+maximum//2)//maximum
    for value in values:
        alpha = value[-1] if extra in (1,2) else maximum
        def straight(v):
            return 0 if extra==1 and not alpha else min(maximum,(v*maximum+alpha//2)//alpha) if extra==1 else v
        if photo in (0,1):
            g = maximum-value[0] if photo==0 else value[0]
            rgb = (scale(straight(g)),)*3
        elif photo==2:
            rgb = tuple(scale(straight(v)) for v in value[:3])
        elif photo==3:
            n=1<<depth
            rgb = tuple((palette[value[0]+c*n]*255+32767)//65535 for c in range(3))
        elif photo==5:
            k = straight(value[3])
            rgb = tuple(scale(((maximum-straight(v))*(maximum-k)+maximum//2)//maximum) for v in value[:3])
        out.append((*rgb,scale(alpha)))
    image=Image.new('RGBA',(width,height));image.putdata(out)
    modes={2:Image.Transpose.FLIP_LEFT_RIGHT,3:Image.Transpose.ROTATE_180,4:Image.Transpose.FLIP_TOP_BOTTOM,
           5:Image.Transpose.TRANSPOSE,6:Image.Transpose.ROTATE_270,7:Image.Transpose.TRANSVERSE,8:Image.Transpose.ROTATE_90}
    return image.transpose(modes[orientation]) if orientation!=1 else image


def make_cases():
    cases=[]
    width,height=35,21
    randoms=random.Random(812830)
    rgb=Image.new('RGB',(width,height));rgb.putdata([tuple(randoms.randrange(256) for _ in range(3))for _ in range(width*height)])
    for mode in ['1','L','LA','P','RGB','RGBA','CMYK','I;16','I;16B']:
        original = rgb.convert(mode) if mode not in ('I;16','I;16B') else Image.frombytes(mode,(width,height),b''.join(struct.pack('<H' if mode=='I;16' else '>H',(i*113)%65536)for i in range(width*height)))
        for compression in ['raw','packbits','tiff_lzw','tiff_adobe_deflate','jpeg']:
            if compression=='jpeg' and mode not in ('L','RGB','CMYK'):continue
            stream=BytesIO();original.save(stream,'TIFF',compression=compression)
            data=stream.getvalue()
            if mode in ('I;16','I;16B'):
                decoded=Image.open(BytesIO(data));decoded.load()
                reference=expected_samples(width,height,[(p,)for p in pixels(decoded)],16,1)
            else:
                reference=Image.open(BytesIO(data)).convert('RGBA')
            cases.append((f'pillow-{mode.replace(";","_")}-{compression}',data,reference,3 if compression=='jpeg' else 1,'pillow'))
    for size in ((1,1),(7,13),(35,21),(257,65),(8192,40)):
        w,h=size
        original=Image.new('1',size)
        if w>1000:
            # Every terminating length and long/multiple make-up runs occur
            # across these alternating strips of black and white.
            original.putdata([int((x//(64*(y%39+1)+(y%64)))%2==0)*255 for y in range(h)for x in range(w)])
        else:
            original.putdata([255 if ((x+2*y)*(x-y))%11<6 else 0 for y in range(h)for x in range(w)])
        for compression,options in [('tiff_ccitt',0),('group3',0),('group3',1),('group3',4),('group3',5),('group4',0)]:
            for fill in (1,2):
                stream=BytesIO()
                original.save(stream,'TIFF',compression=compression,tiffinfo={266:fill,262:(fill+options)%2,292:options})
                data=stream.getvalue()
                reference=Image.open(BytesIO(data)).convert('RGBA')
                cases.append((f'fax-{w}x{h}-{compression}-{options}-{fill}',data,reference,0,'pillow-libtiff'))
    lengths=list(range(64))+list(range(64,2561,64))+[3200]
    original=Image.new('1',(3200,2*len(lengths)))
    original.putdata([255 if x<(lengths[y//2] if y%2==0 else 3200-lengths[y//2])else 0
                      for y in range(original.height)for x in range(3200)])
    for compression in ('tiff_ccitt','group3','group4'):
        for fill in (1,2):
            stream=BytesIO();original.save(stream,'TIFF',compression=compression,tiffinfo={266:fill,292:5})
            data=stream.getvalue()
            cases.append((f'fax-all-run-lengths-{compression}-{fill}',data,Image.open(BytesIO(data)).convert('RGBA'),0,'pillow-libtiff'))
    stream=BytesIO();rgb.save(stream,'TIFF',save_all=True,append_images=[rgb.transpose(Image.Transpose.FLIP_LEFT_RIGHT)])
    cases.append(('multipage-first-image',stream.getvalue(),rgb.convert('RGBA'),0,'pillow'))
    # Custom containers exercise byte order, partial edge tiles, plane ordering,
    # sample packing and prediction independently of the decoder implementation.
    descriptions=[]
    for depth in (1,2,4,8,16):
        maximum=(1<<depth)-1
        vals=[((x*7+y*3)&maximum,)for y in range(height)for x in range(width)]
        descriptions.append((f'gray{depth}',vals,depth,1,None,None))
        descriptions.append((f'white{depth}',vals,depth,0,None,None))
        if depth<=8:
            palette=[((i*257+c*12345)&65535)for c in range(3)for i in range(1<<depth)]
            descriptions.append((f'palette{depth}',vals,depth,3,None,palette))
    for depth in (8,16):
        maximum=(1<<depth)-1
        for photo,channels,label in [(2,3,'rgb'),(5,4,'cmyk')]:
            vals=[tuple((x*(31+c*13)+y*(23+c*19))&maximum for c in range(channels))for y in range(height)for x in range(width)]
            descriptions.append((label+str(depth),vals,depth,photo,None,None))
            alpha=[(*v,(i*7919+maximum//2)&maximum) for i,v in enumerate(vals)]
            descriptions.append((label+str(depth)+'a',alpha,depth,photo,2,None))
            associated=[(*(v*a[-1]//maximum for v in a[:-1]),a[-1])for a in alpha]
            descriptions.append((label+str(depth)+'premult',associated,depth,photo,1,None))
    for i,(label,vals,depth,photo,extra,palette) in enumerate(descriptions):
        for variant in range(4):
            kwargs=dict(depth=depth,photo=photo,extra=extra,palette=palette,
                        little=variant%2==0,planar=1+(variant//2),
                        predictor=1+(variant//2),tiled=variant>=2,
                        compression=[1,32773,5,8][variant],orientation=(i+variant)%8+1)
            data=tiff(width,height,vals,**kwargs)
            reference=expected_samples(width,height,vals,depth,photo,extra=extra,orientation=kwargs['orientation'],palette=palette)
            cases.append((f'custom-{label}-{variant}',data,reference,0,'analytical'))
    # Dictionary width changes, a full table, KwKwK and both Deflate tag values.
    for label,raw in [('noise',bytes(randoms.randrange(256)for _ in range(32768))),('runs',b'\x33'*32768)]:
        for compression in (5,32946,32773):
            vals=[(b,)for b in raw]
            data=tiff(256,128,vals,photo=1,compression=compression,rows=128)
            ref=Image.frombytes('L',(256,128),raw).convert('RGBA')
            cases.append((f'dictionary-{label}-{compression}',data,ref,0,'analytical'))
    vals=[(i%2,)for i in range(35*21)]
    cases.append(('reversed-one-bit',tiff(35,21,vals,depth=1,photo=1,fill=2),expected_samples(35,21,vals,1,1),0,'analytical'))
    for orientation in range(1,9):
        stream=BytesIO();rgb.save(stream,'TIFF',compression='tiff_lzw',tiffinfo={274:orientation,317:2})
        data=stream.getvalue();cases.append((f'pillow-orientation-{orientation}',data,Image.open(BytesIO(data)).convert('RGBA'),0,'pillow'))
    for progressive in (False,True):
        for quality in (35,75,97):
            for size in ((1,1),(7,13),(35,21),(129,91)):
                original=rgb.resize(size).convert('CMYK')
                stream=BytesIO();original.save(stream,'JPEG',quality=quality,progressive=progressive)
                data=stream.getvalue();ref=ImageOps.exif_transpose(Image.open(BytesIO(data))).convert('RGBA')
                cases.append((f'cmyk-{progressive}-{quality}-{size[0]}x{size[1]}',data,ref,3,'pillow'))
    return cases,rgb


def parse_output(path):
    data=path.read_bytes();width,height=struct.unpack_from('<II',data)
    values=struct.unpack_from('<'+'I'*(width*height),data,8)
    assert len(data)==8+4*width*height
    result=Image.new('RGBA',(width,height))
    result.putdata([((p>>16)&255,(p>>8)&255,p&255,p>>24)for p in values])
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rustc',default='rustc')
    parser.add_argument('--output',type=Path,default=Path('results/verification/tiff-report.json'))
    parser.add_argument('--artifacts',type=Path)
    parser.add_argument('--write-demos',action='store_true')
    args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='chezcat-tiff-') as temporary:
        work=args.artifacts or Path(temporary)
        work.mkdir(parents=True,exist_ok=True)
        source=PROBE.replace('IMAGE',json.dumps(str(ROOT/'src/image.rs'))).replace('JPEG',json.dumps(str(ROOT/'src/jpeg.rs'))).replace('COMPRESSION',json.dumps(str(ROOT/'src/compression.rs')))
        (work/'probe.rs').write_text(source, encoding="utf-8")
        probe=work/'probe'
        subprocess.run([args.rustc,'--edition=2021','-O',str(work/'probe.rs'),'-o',str(probe)],check=True)
        cases,rgb=make_cases()
        converter=shutil.which('magick') or shutil.which('convert')
        if converter:
            source=work/'ycck-source.tif';rgb.convert('CMYK').save(source)
            for progressive in (False,True):
                for sampling in ('1x1','2x1','2x2'):
                    target=work/f'ycck-{progressive}-{sampling}.jpg'
                    subprocess.run([converter,str(source),'-sampling-factor',sampling,'-quality','89','-interlace','Plane' if progressive else 'None',str(target)],check=True,capture_output=True)
                    data=target.read_bytes();index=data.index(b'Adobe')
                    assert data[index+11]==2
                    cases.append((target.stem,data,Image.open(target).convert('RGBA'),5,'imagemagick-pillow'))
            source=work/'jpeg-tiff-source.png';rgb.save(source)
            for mode in ('RGB','YCbCr','Gray','CMYK'):
                for tiled in (False,True):
                    target=work/f'jpeg-tiff-{mode}-{tiled}.tif'
                    options=['-define','tiff:tile-geometry=16x16'] if tiled else ['-define','tiff:rows-per-strip=16']
                    subprocess.run([converter,str(source),'-colorspace',mode,'-compress','JPEG',*options,str(target)],check=True,capture_output=True)
                    data=target.read_bytes();reference=Image.open(target).convert('RGBA')
                    cases.append((target.stem,data,reference,3,'imagemagick-libtiff'))
        reports=[]
        for name,data,reference,tolerance,origin in cases:
            path=work/(name+('.jpg'if data.startswith(b'\xff\xd8')else'.tif'));path.write_bytes(data)
            out=work/(name+'.rgba')
            run=subprocess.run([str(probe),str(path),str(out)],capture_output=True,text=True,timeout=20)
            assert run.returncode==0,(name,run.stdout,run.stderr)
            actual=parse_output(out)
            assert actual.size==reference.size,(name,actual.size,reference.size)
            diff=[abs(x-y)for a,b in zip(pixels(actual),pixels(reference))for x,y in zip(a,b)]
            maximum=max(diff,default=0)
            assert maximum<=tolerance,(name,maximum,tolerance)
            reports.append({'case':name,'reference':origin,'maximum_channel_error':maximum,'mean_channel_error':sum(diff)/len(diff)})
            # Author-generated TIFF containers are also decoded independently
            # where Pillow has a matching pixel-mode implementation.
            if origin=='analytical':
                try:
                    independent=Image.open(BytesIO(data));independent.load()
                    if independent.mode in ('1','L','P','RGB','RGBA','CMYK') and '16' not in name and 'premult' not in name and 'cmyk8a' not in name:
                        independent=independent.convert('RGBA')
                        errors=[abs(x-y)for a,b in zip(pixels(independent),pixels(reference))for x,y in zip(a,b)]
                        assert independent.size==reference.size and max(errors,default=0)<=1,(name,'Pillow disagreement',max(errors,default=0))
                        reports[-1]['pillow_cross_check']=True
                except (OSError,ValueError):
                    if name.startswith('dictionary-'):
                        raise
                    reports[-1]['pillow_cross_check']='unsupported reference pixel mode'
        # Targeted errors and mutation smoke checks use the actual dispatcher.
        negatives=[]
        sample=next(data for name,data,_,_,_ in cases if name=='custom-rgb8-0')
        def reject(name,data,stop=0):
            path=work/'negative.tif';path.write_bytes(data)
            run=subprocess.run([str(probe),str(path),str(work/'negative.rgba'),str(stop)],capture_output=True,text=True,timeout=10)
            assert run.returncode==2,(name,run.returncode,run.stdout,run.stderr)
            assert run.stdout.startswith('ERR '),(name,run.stdout)
            negatives.append({'case':name,'message':run.stdout.strip()})
        reject('cancel-before-parse',sample,1)
        reject('cancel-during-raster',sample,4)
        reject('bigtiff',b'II+\0\x08\0\0\0'+b'\0'*8)
        for n in (0,1,2,4,7,8,9,10,15,len(sample)-1):reject(f'truncated-{n}',sample[:n])
        # Corrupt scalar fields without accidentally making a different valid
        # image. These field IDs are deliberately present in the fixture.
        def field_value(tag,value,data=sample):
            changed=bytearray(data)
            order='<' if changed[:2]==b'II'else'>'
            at=struct.unpack_from(order+'I',changed,4)[0]
            for i in range(struct.unpack_from(order+'H',changed,at)[0]):
                row=at+2+12*i
                if struct.unpack_from(order+'H',changed,row)[0]==tag:
                    kind=struct.unpack_from(order+'H',changed,row+2)[0]
                    count=struct.unpack_from(order+'I',changed,row+4)[0]
                    target=row+8 if count*{3:2,4:4,5:8}[kind]<=4 else struct.unpack_from(order+'I',changed,row+8)[0]
                    struct.pack_into(order+('H'if kind==3 else'I'),changed,target,value)
                    return bytes(changed)
            raise AssertionError(tag)
        for tag,value in [(256,0),(256,16385),(259,2),(259,6),(262,8),(266,3),(274,9),(277,6),(284,3),(317,3),(339,3)]:
            reject(f'field-{tag}-{value}',field_value(tag,value))
        for name,data,_,_,_ in cases:
            if name=='fax-35x21-group3-1-1':
                reject('fax-unsupported-uncompressed-mode',field_value(292,3,data))
                reject('fax-row-overflow',field_value(256,1,data))
                reject('fax-invalid-photometric',field_value(262,3,data))
                reject('fax-cancel-during-raster',data,8)
            if name=='jpeg-tiff-YCbCr-True':
                reject('tiff-jpeg-custom-reference-level',field_value(532,12,data))
                reject('tiff-jpeg-mismatched-subsampling',field_value(530,4,data))
        for name,data,_,_,_ in cases:
            if name.startswith('cmyk-'):
                altered=bytearray(data);adobe=altered.index(b'Adobe');altered[adobe+11]=1
                reject('four-component-wrong-adobe-transform',bytes(altered))
                break
        rng=random.Random(280028)
        mutation_outcomes={'accepted':0,'rejected':0}
        for index in range(360):
            _,seed,_,_,_=cases[index%len(cases)]
            changed=bytearray(seed)
            if index%3==0: changed=changed[:rng.randrange(len(changed))]
            else:
                for _ in range(1+index%5):
                    at=rng.randrange(len(changed));changed[at]^=rng.randrange(1,256)
            path=work/'mutation.tif';path.write_bytes(changed)
            run=subprocess.run([str(probe),str(path),str(work/'mutation.rgba')],capture_output=True,text=True,timeout=10)
            assert run.returncode in (0,2),('mutation',index,run.returncode,run.stderr)
            mutation_outcomes['accepted'if run.returncode==0 else'rejected']+=1
        if args.write_demos:
            w,h=192,128
            values=[((15+x*2)%256,(35+y*2)%256,(90+x+y)%256)for y in range(h)for x in range(w)]
            (ROOT/'pages/images/format-tiled.tiff').write_bytes(tiff(w,h,values,tiled=True,compression=5,predictor=2))
            demo=Image.new('RGB',(w,h));demo.putdata(values)
            demo.convert('CMYK').save(ROOT/'pages/images/format-cmyk.jpg',quality=90,progressive=True)
        report={'valid_cases':len(reports),'targeted_rejections':len(negatives),'malformed_mutations':360,
                'mutation_outcomes':mutation_outcomes,'pillow':Image.__version__,
                'libtiff':features.version('libtiff'),'libjpeg':features.version('jpg'),
                'tiff_source_sha256':hashlib.sha256((ROOT/'src/image_tiff.rs').read_bytes()).hexdigest(),
                'jpeg_source_sha256':hashlib.sha256((ROOT/'src/jpeg.rs').read_bytes()).hexdigest(),
                'cases':reports,'rejections':negatives}
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text(json.dumps(report,indent=2)+'\n', encoding="utf-8")
        print(json.dumps({k:v for k,v in report.items()if k not in ('cases','rejections')},indent=2))


if __name__=='__main__':
    main()
