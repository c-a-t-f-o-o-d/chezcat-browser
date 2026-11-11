#!/usr/bin/env python3
"""Compare original lossy WebP and animation fixtures against independent codecs.

Pillow and the independent reference decoder are development-only oracles;
the browser contains its own Rust VP8 and alpha decoders with no dependencies.
"""
import argparse
import io
import json
import os
from pathlib import Path
import random
import struct
import subprocess
import tempfile
import shutil
import math
from PIL import Image
from PIL import ImageDraw

REPO = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rustc', default='rustc')
parser.add_argument('--artifacts', type=Path)
parser.add_argument('--output', type=Path)
parser.add_argument('--quick', action='store_true')
parser.add_argument('--write-demos', action='store_true', help='Regenerate the two original ChezCat workshop WebP scenes')
args = parser.parse_args()
temporary = tempfile.TemporaryDirectory(prefix='chezcat-vp8-check-')
root = args.artifacts.resolve() if args.artifacts else Path(temporary.name)
root.mkdir(parents=True, exist_ok=True)
probe = root / ('vp8-probe.exe' if os.name == 'nt' else 'vp8-probe')
source = '''#![allow(dead_code)]
#[path=COMPRESSION] mod compression;
#[path=JPEG] mod jpeg;
#[path=IMAGE] mod image;
use std::io::{Read,Write};
fn main(){let mut d=Vec::new();std::io::stdin().read_to_end(&mut d).unwrap();
if std::env::args().any(|a|a=="--animation") {
    let asset=match image::decode_asset_checked(&d,&||Ok(())){Ok(a)=>a,Err(e)=>{eprintln!("{e}");std::process::exit(2);}};
    let mut o=std::io::stdout().lock();let frames=asset.animation.as_ref();
    for n in [asset.bitmap.width,asset.bitmap.height,frames.map_or(1,|a|a.plays.unwrap_or(0)) as usize,frames.map_or(0,|a|a.frames.len()),usize::from(asset.notice.is_some())]{o.write_all(&(n as u32).to_le_bytes()).unwrap();}
    if let Some(a)=frames{for f in &a.frames{o.write_all(&f.delay_ms.to_le_bytes()).unwrap();for &p in &f.bitmap.pixels{o.write_all(&[(p>>16)as u8,(p>>8)as u8,p as u8,(p>>24)as u8]).unwrap();}}}
    return;
}
match image::decode(&d){Ok(b)=>{let mut o=std::io::stdout().lock();o.write_all(&(b.width as u32).to_le_bytes()).unwrap();o.write_all(&(b.height as u32).to_le_bytes()).unwrap();for p in b.pixels{o.write_all(&[(p>>16)as u8,(p>>8)as u8,p as u8,(p>>24)as u8]).unwrap();}},Err(e)=>{eprintln!("{e}");std::process::exit(2);}}}
'''
for module in ('compression','jpeg','image'):
    source=source.replace(module.upper(),json.dumps(str(REPO/'src'/(module+'.rs'))))
(root/'vp8-probe.rs').write_text(source, encoding="utf-8")
subprocess.run([args.rustc, '--edition', '2021', '-O', str(root/'vp8-probe.rs'), '-o', str(probe)], check=True)
rng = random.Random(2828)
fixtures = []
sizes = [(1,1),(3,5),(16,16),(17,19),(47,33)] if args.quick else [(1,1),(1,37),(41,1),(3,5),(16,16),(17,19),(47,33),(128,79),(257,161)]
for w,h in sizes:
    fixtures.append((f'gray-{w}x{h}',Image.new('RGB',(w,h),(127,127,127))))
    fixtures.append((f'gradient-{w}x{h}',Image.frombytes('RGB',(w,h),bytes(v for y in range(h) for x in range(w) for v in ((x*13+y*7)%256,(y*11+x*3)%256,(x*5+y*17)%256)))))
    fixtures.append((f'noise-{w}x{h}',Image.frombytes('RGB',(w,h),rng.randbytes(w*h*3))))
    fixtures.append((f'alpha-{w}x{h}',Image.frombytes('RGBA',(w,h),bytes(v for y in range(h) for x in range(w) for v in ((x*21+y*3)%256,(y*23+x*13)%256,(x*11+y*7)%256,(x*17+y*31)%256)))))
results = []
qualities = [0,75,100] if args.quick else [0,10,35,75,95,100]
methods = [4] if args.quick else [0,4,6]
for name, image in fixtures:
    for quality in qualities:
        for method in methods:
            label = f'{name}-q{quality}-m{method}'
            encoded = io.BytesIO()
            image.save(encoded,format='WEBP',lossless=False,quality=quality,method=method,exact=True)
            data = encoded.getvalue()
            (root/(label+'.webp')).write_bytes(data)
            reference = Image.open(io.BytesIO(data)).convert('RGBA').tobytes()
            run = subprocess.run([str(probe)],input=data,capture_output=True,timeout=15)
            result = {'name':label,'bytes':len(data)}
            if run.returncode:
                result.update(error=run.stderr.decode(errors='replace').strip(),exit=run.returncode)
            elif len(run.stdout)<8 or struct.unpack('<II',run.stdout[:8])!=image.size or len(run.stdout)!=len(reference)+8:
                result['error'] = 'Incorrect output dimensions or pixel count.'
            else:
                difference = [abs(a-b) for a,b in zip(reference,run.stdout[8:])]
                result.update(maximum=max(difference),mean=sum(difference)/len(difference),different=sum(d!=0 for d in difference))
                if result['maximum']>1:
                    Image.frombytes('RGBA',image.size,run.stdout[8:]).save(root/(label+'-actual.png'))
                    Image.frombytes('RGBA',image.size,reference).save(root/(label+'-reference.png'))
            results.append(result)
            if 'error' in result or result.get('maximum',0)>1: print(json.dumps(result),flush=True)
def chunk(kind, payload):
    return kind+struct.pack('<I',len(payload))+payload+(b'\0' if len(payload)&1 else b'')

def riff(chunks):
    body=b'WEBP'+chunks
    return b'RIFF'+struct.pack('<I',len(body))+body

def chunks(data):
    at=12
    while at<len(data):
        size=struct.unpack('<I',data[at+4:at+8])[0]
        yield data[at:at+4],data[at+8:at+8+size]
        at+=8+size+(size&1)

def packed24(n):
    return n.to_bytes(3,'little')

def extended(w,h,flags):
    return chunk(b'VP8X',bytes([flags,0,0,0])+packed24(w-1)+packed24(h-1))

def encoded_image(image, **options):
    out=io.BytesIO()
    image.save(out,format='WEBP',**options)
    return out.getvalue()

def extra_case(label, data, expected_alpha=None):
    (root/(label+'.webp')).write_bytes(data)
    reference=Image.open(io.BytesIO(data)).convert('RGBA')
    run=subprocess.run([str(probe)],input=data,capture_output=True,timeout=15)
    result={'name':label,'bytes':len(data)}
    if run.returncode:
        result.update(error=run.stderr.decode(errors='replace').strip(),exit=run.returncode)
    elif len(run.stdout)!=reference.width*reference.height*4+8 or struct.unpack('<II',run.stdout[:8])!=reference.size:
        result['error']='Incorrect output dimensions or pixel count.'
    else:
        difference=[abs(a-b) for a,b in zip(reference.tobytes(),run.stdout[8:])]
        result.update(maximum=max(difference),mean=sum(difference)/len(difference),different=sum(d!=0 for d in difference))
        if expected_alpha is not None and run.stdout[11::4]!=expected_alpha:
            result['error']='ALPH output does not match the original exact alpha samples.'
    results.append(result)
    if 'error' in result or result.get('maximum',0)>1:
        print(json.dumps(result),flush=True)

# Exercise each alpha filter, both raw and VP8L entropy-coded alpha, and the
# level-reduction metadata bit. The filter bytes are independently generated.
w,h=23,19
color=Image.frombytes('RGB',(w,h),bytes(v for y in range(h) for x in range(w) for v in (x*9%256,y*13%256,(x*7+y*5)%256)))
color_payload=next(payload for kind,payload in chunks(encoded_image(color,quality=75)) if kind==b'VP8 ')
alpha=bytes((x*47+y*91+(255 if (x+y)%7==0 else 0))%256 for y in range(h) for x in range(w))
for filtering in range(4):
    residual=bytearray()
    for y in range(h):
        for x in range(w):
            at=y*w+x
            left=alpha[at-1] if x else 0
            above=alpha[at-w] if y else 0
            if filtering==0: prediction=0
            elif filtering==1: prediction=left if x else above
            elif filtering==2: prediction=above if y else left
            elif not y: prediction=left
            elif not x: prediction=above
            else: prediction=max(0,min(255,left+above-alpha[at-w-1]))
            residual.append((alpha[at]-prediction)%256)
    for compressed in (False,True):
        if compressed:
            green=Image.frombytes('RGB',(w,h),bytes(v for a in residual for v in (0,a,0)))
            encoded=next(payload for kind,payload in chunks(encoded_image(green,lossless=True,exact=True)) if kind==b'VP8L')[5:]
        else: encoded=bytes(residual)
        for reduced in (False,True):
            flags=int(compressed)|(filtering<<2)|(int(reduced)<<4)
            data=riff(extended(w,h,16)+chunk(b'ALPH',bytes([flags])+encoded)+chunk(b'VP8 ',color_payload))
            extra_case(f'alpha-filter{filtering}-compression{int(compressed)}-reduced{int(reduced)}',data,alpha)

# libvpx is a second, independent encoder. Profiles and token partitions can
# select paths libwebp's still-image encoder rarely chooses.
vpx_cases=0
if shutil.which('ffmpeg'):
    w,h=65,67
    samples=random.Random(287).randbytes(w*h*3)
    for profile in range(4):
        for partitions in (1,2,4,8):
            for quantizer in (12,42):
                command=['ffmpeg','-v','error','-f','rawvideo','-pix_fmt','rgb24','-s',f'{w}x{h}','-i','pipe:0','-frames:v','1','-c:v','libvpx','-pix_fmt','yuv420p','-slices',str(partitions),'-threads','1','-profile:v',str(profile),'-sharpness','7','-crf',str(quantizer),'-qmin',str(quantizer),'-qmax',str(quantizer),'-f','ivf','pipe:1']
                run=subprocess.run(command,input=samples,capture_output=True,timeout=30)
                if run.returncode: raise AssertionError(run.stderr.decode())
                size=struct.unpack('<I',run.stdout[32:36])[0]
                extra_case(f'libvpx-profile{profile}-partitions{partitions}-q{quantizer}',riff(chunk(b'VP8 ',run.stdout[44:44+size])))
                vpx_cases+=1

animation_results=[]
frames=[]
for index in range(9):
    image=Image.new('RGBA',(64,48),(0,0,0,0))
    draw=ImageDraw.Draw(image)
    draw.rectangle((index*5,4+index,25+index*4,30+index),fill=(index*25,230-index*15,70+index*7,255 if index%3==0 else 127))
    draw.line((0,index*4,63,47-index*3),fill=(255,10,180,255),width=2)
    frames.append(image)
for lossless in (True,False):
    for minimize in (False,True):
        for loop in (0,1,3):
            label=f'animation-lossless{lossless}-minimize{minimize}-loop{loop}'
            data=encoded_image(frames[0],save_all=True,append_images=frames[1:],lossless=lossless,quality=75,duration=[27+i*11 for i in range(9)],loop=loop,minimize_size=minimize)
            (root/(label+'.webp')).write_bytes(data)
            run=subprocess.run([str(probe),'--animation'],input=data,capture_output=True,timeout=15)
            assert run.returncode==0,(label,run.stderr)
            width,height,plays,number,notice=struct.unpack('<IIIII',run.stdout[:20])
            assert (width,height,plays,number,notice)==(64,48,loop,9,0),(label,'animation metadata')
            reference=Image.open(io.BytesIO(data));position=20
            for index in range(number):
                delay=struct.unpack('<I',run.stdout[position:position+4])[0];position+=4
                actual=run.stdout[position:position+width*height*4];position+=len(actual)
                reference.seek(index);expected=reference.convert('RGBA').tobytes()
                assert delay==reference.info['duration'],(label,index,'delay')
                # Hidden RGB at alpha0 is not visible and reference codecs may
                # normalize it. Alpha is always compared, including alpha0.
                difference=[abs(a-b) for i,(a,b) in enumerate(zip(actual,expected)) if i%4==3 or expected[i//4*4+3]!=0]
                result={'name':label,'frame':index,'maximum':max(difference),'mean':sum(difference)/len(difference)}
                animation_results.append(result)
                if result['maximum']>1: print(json.dumps(result),flush=True)
            assert position==len(run.stdout)

# Hand-assembled, original mixed-codec frames force source-over, partial-alpha
# destinations, disposal, and subrectangles instead of relying on an encoder
# to choose those operations. libwebp independently assembles the canvases.
specifications=[(Image.new('RGB',(40,32),(10,40,100)),0,0,2,17,False),
                (Image.new('RGBA',(16,12),(240,30,60,85)),4,8,0,43,True),
                (Image.new('RGBA',(12,18),(50,180,250,170)),8,6,1,61,False),
                (Image.new('RGBA',(12,12),(150,200,40,128)),6,4,0,79,True)]
body=extended(40,32,18)+chunk(b'ANIM',struct.pack('<IH',0,2))
for image,left,top,flags,delay,lossless in specifications:
    header=b''.join(packed24(n) for n in (left//2,top//2,image.width-1,image.height-1,delay))+bytes([flags])
    frame_chunks=b''.join(chunk(kind,payload) for kind,payload in chunks(encoded_image(image,lossless=lossless,quality=75,exact=True)) if kind in (b'VP8 ',b'VP8L',b'ALPH'))
    body+=chunk(b'ANMF',header+frame_chunks)
data=riff(body);(root/'animation-mixed-composite.webp').write_bytes(data)
run=subprocess.run([str(probe),'--animation'],input=data,capture_output=True,timeout=15)
assert run.returncode==0,run.stderr
assert struct.unpack('<IIIII',run.stdout[:20])==(40,32,2,4,0)
reference=Image.open(io.BytesIO(data));position=20
for index in range(4):
    delay=struct.unpack('<I',run.stdout[position:position+4])[0];position+=4
    actual=run.stdout[position:position+40*32*4];position+=len(actual)
    reference.seek(index);expected=reference.convert('RGBA').tobytes()
    difference=[abs(a-b) for i,(a,b) in enumerate(zip(actual,expected)) if i%4==3 or expected[i//4*4+3]!=0]
    assert delay==reference.info['duration']
    result={'name':'animation-mixed-composite','frame':index,'maximum':max(difference),'mean':sum(difference)/len(difference)}
    animation_results.append(result)
    if result['maximum']>1:print(json.dumps(result),flush=True)

# Structural damage must remain an error; a failed decode must never panic.
malformed=[]
baseline=(root/'gradient-47x33-q75-m4.webp').read_bytes()
payload=next(payload for kind,payload in chunks(baseline) if kind==b'VP8 ')
for cut in (0,1,2,3,5,9,10,len(payload)//3,len(payload)//2):
    malformed.append((f'truncated-vp8-{cut}',riff(chunk(b'VP8 ',payload[:cut]))))
for name,offset,value in [('interframe',0,payload[0]|1),('invisible',0,payload[0]&~16),('reserved-profile',0,(payload[0]&~14)|8),('sync',3,0),('zero-width',6,0)]:
    corrupt=bytearray(payload);corrupt[offset]=value
    if name=='zero-width':corrupt[7]=0
    malformed.append((name,riff(chunk(b'VP8 ',corrupt))))
malformed.append(('missing-alpha',riff(extended(23,19,16)+chunk(b'ALPH',b'')+chunk(b'VP8 ',color_payload))))
malformed.append(('alpha-before-canvas',riff(chunk(b'ALPH',b'\0')+chunk(b'VP8 ',color_payload))))
malformed.append(('duplicate-image',riff(chunk(b'VP8 ',payload)+chunk(b'VP8 ',payload))))
malformed.append(('animation-missing-header',riff(extended(1,1,2))))
for label,data in malformed:
    run=subprocess.run([str(probe)],input=data,capture_output=True,timeout=15)
    assert run.returncode==2,(label,run.returncode,run.stderr)
mutations=300 if not args.quick else 60
for index in range(mutations):
    data=bytearray(payload)
    if index%3==0:data=data[:rng.randrange(len(data))]
    else:
        for _ in range(1+index%3):data[rng.randrange(len(data))]=rng.randrange(256)
    run=subprocess.run([str(probe)],input=riff(chunk(b'VP8 ',data)),capture_output=True,timeout=15)
    assert run.returncode in (0,2),(index,run.returncode,run.stderr)
    if run.returncode==0:
        w,h=struct.unpack('<II',run.stdout[:8])
        assert w*h<=4*1024*1024 and len(run.stdout)==w*h*4+8

# Reproducible original workshop art; no downloaded photographs, artwork, font,
# icon or third-party test corpus is included in these fixtures.
demo_checks=[]
if args.write_demos:
    demo_frames=[]
    for index in range(12):
        scene=Image.new('RGBA',(320,180),(0,0,0,0))
        draw=ImageDraw.Draw(scene)
        draw.rounded_rectangle((4,4,315,175),radius=20,fill=(11,21,60,255),outline=(66,199,188,255),width=2)
        draw.polygon([(18,158),(92,79),(172,158)],fill=(66,199,188,255))
        draw.polygon([(99,158),(195,61),(302,158)],fill=(157,148,225,255))
        draw.polygon([(158,158),(236,97),(302,158)],fill=(255,146,124,255))
        x=41+index*19
        y=38+round(10*math.sin(index*math.pi/6))
        draw.ellipse((x-13,y-13,x+13,y+13),fill=(255,243,220,255))
        draw.ellipse((x-21,y-21,x+21,y+21),outline=(66,199,188,150),width=2)
        for step in range(8):
            xx=24+step*37
            draw.line((xx,163,xx+18,163),fill=(255,243,220,180),width=2)
        demo_frames.append(scene)
    demos=REPO/'pages/images'
    static=encoded_image(demo_frames[3],lossless=False,quality=85,method=6,exact=True)
    animated=encoded_image(demo_frames[0],save_all=True,append_images=demo_frames[1:],lossless=False,quality=85,method=6,duration=90,loop=0,minimize_size=True)
    extra_case('workshop-lossy',static)
    run=subprocess.run([str(probe),'--animation'],input=animated,capture_output=True,timeout=15)
    assert run.returncode==0,run.stderr
    w,h,plays,number,notice=struct.unpack('<IIIII',run.stdout[:20])
    assert (w,h,plays,number,notice)==(320,180,0,12,0)
    reference=Image.open(io.BytesIO(animated));position=20
    for index in range(number):
        delay=struct.unpack('<I',run.stdout[position:position+4])[0];position+=4
        actual=run.stdout[position:position+w*h*4];position+=len(actual)
        reference.seek(index);expected=reference.convert('RGBA').tobytes()
        maximum=max(abs(a-b) for i,(a,b) in enumerate(zip(actual,expected)) if i%4==3 or expected[i//4*4+3]!=0)
        assert maximum<=1 and delay==90,(index,maximum,delay)
        demo_checks.append({'frame':index,'maximum':maximum,'delay':delay})
    (demos/'format-lossy.webp').write_bytes(static)
    (demos/'format-animated.webp').write_bytes(animated)

summary = {'cases':len(results),'libvpx_cases':vpx_cases,'animation_frames':len(animation_results),'demo_frames':len(demo_checks),'malformed_rejections':len(malformed),'mutations':mutations,'errors':sum('error' in r for r in results),'over_tolerance':sum(r.get('maximum',0)>1 for r in results+animation_results),'maximum':max((r.get('maximum',0) for r in results+animation_results),default=0),'results':results,'animation_results':animation_results,'demo_checks':demo_checks}
if args.output:
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(summary,indent=2)+'\n', encoding="utf-8")
print(json.dumps({k:v for k,v in summary.items() if k not in ('results','animation_results','demo_checks')}))
raise SystemExit(bool(summary['errors'] or summary['over_tolerance']))
