#!/usr/bin/env python3
"""Original static VP8L checks against Pillow/libwebp, plus forced bitstream features.
These are development-only dependencies; the browser uses original Rust decoding.
Run: python tools/check_webp.py [--rustc PATH] [--output report.json]
"""
from pathlib import Path
from PIL import Image
from io import BytesIO
import random,subprocess,struct,json,time
import argparse
import os
import tempfile
REPO = Path(__file__).resolve().parents[1]
arguments = argparse.ArgumentParser(description=__doc__)
arguments.add_argument('--rustc', default='rustc')
arguments.add_argument('--output', type=Path, help='Save the complete JSON check report')
arguments.add_argument('--artifacts', type=Path, help='Keep generated files and comparison images here')
args = arguments.parse_args()
_temporary = tempfile.TemporaryDirectory(prefix='chezcat-image-check-')
ROOT = args.artifacts.resolve() if args.artifacts else Path(_temporary.name)
ROOT.mkdir(parents=True, exist_ok=True)
PROBE = ROOT / ('probe.exe' if os.name == 'nt' else 'probe')
RUST_PROBE = '#![allow(dead_code)]\n#[path=COMPRESSION] mod compression;\n#[path=JPEG] mod jpeg;\n#[path=IMAGE] mod image;\nuse std::io::{Read,Write};\nfn main() {\n let mut data=Vec::new(); std::io::stdin().read_to_end(&mut data).unwrap();\n match image::decode(&data) {\n  Ok(bitmap)=> { let mut out=std::io::stdout().lock(); out.write_all(&(bitmap.width as u32).to_le_bytes()).unwrap(); out.write_all(&(bitmap.height as u32).to_le_bytes()).unwrap(); for p in bitmap.pixels { out.write_all(&[((p>>16)&255)as u8,((p>>8)&255)as u8,(p&255)as u8,(p>>24)as u8]).unwrap(); } },\n  Err(error)=>{eprintln!("{error}");std::process::exit(2);}\n }\n}\n'
for module in ('image', 'compression', 'jpeg'):
    RUST_PROBE = RUST_PROBE.replace(module.upper(), json.dumps(str(REPO / 'src' / (module + '.rs'))))
(ROOT / 'probe.rs').write_text(RUST_PROBE, encoding='utf-8')
subprocess.run([args.rustc, '--edition', '2021', '-O', str(ROOT / 'probe.rs'), '-o', str(PROBE)], check=True)


OUT=ROOT/'webp-oracle';OUT.mkdir(exist_ok=True)
rng=random.Random(273)
images=[]
for w,h in ((1,1),(1,31),(33,1),(7,5),(19,17),(127,73),(256,256)):
 images.append((f'noise-{w}x{h}',Image.frombytes('RGBA',(w,h),rng.randbytes(w*h*4))))
 images.append((f'gradient-{w}x{h}',Image.frombytes('RGBA',(w,h),bytes(v for y in range(h) for x in range(w) for v in (x*3%256,y*5%256,(x*3+y*5)%256,255)))))
 for colors in (2,4,8,16,33,256):
  palette=[tuple(rng.randrange(256) for _ in range(4)) for i in range(colors)]
  samples=[palette[(x//3+7*(y//4))%colors] for y in range(h) for x in range(w)]
  images.append((f'palette{colors}-{w}x{h}',Image.frombytes('RGBA',(w,h),bytes(v for p in samples for v in p))))
# Distinct entropy regions and highly correlated cross-channel data.
w,h=384,256;data=bytearray()
for y in range(h):
 for x in range(w):
  if x<96:p=(20+x+y%3,2*x,200-x,255)
  elif x<192:p=(255,255,255,255) if (x+y)%7 else (0,0,0,0)
  elif x<288:p=tuple(rng.randrange(256) for _ in range(4))
  else:p=((x*3+y)%256,(x*9+y*3)%256,(x*21+y*7)%256,(x+y)%256)
  data.extend(p)
images.append(('entropy-regions',Image.frombytes('RGBA',(w,h),bytes(data))))

# Original literal-only writer follows the wire specification. libwebp independently
# decodes these forced feature cases, including modes its encoder may rarely select.
class Writer:
 def __init__(self):self.data=bytearray();self.at=0
 def put(self,value,count):
  for shift in range(count):
   if self.at//8==len(self.data):self.data.append(0)
   self.data[self.at//8]|=((value>>shift)&1)<<(self.at%8);self.at+=1
 def code(self,value,count):
  for shift in reversed(range(count)):self.put((value>>shift)&1,1)
 def leaf(self,value):self.put(1,1);self.put(0,1);self.put(1,1);self.put(value,8)
 def full(self,alphabet):
  self.put(0,1);self.put(8,4) # Twelve code-length-code descriptions.
  order=[17,18,0,1,2,3,4,5,16,6,7,8]
  for code in order:self.put(int(code==8 or (code==0 and alphabet>256)),3)
  self.put(0,1)
  if alphabet>256:
   for i in range(alphabet):self.put(int(i<256),1)
 def two(self,alphabet,a,b):
  self.put(0,1);self.put(0,4)
  for v in (0,0,1,1):self.put(v,3)
  self.put(0,1)
  for i in range(alphabet):self.put(int(i in (a,b)),1)
 def header(self,w,h):self.put(w-1,14);self.put(h-1,14);self.put(1,1);self.put(0,3)

def image_literals(writer,w,h,values,main):
 writer.put(0,1)
 if main:writer.put(0,1)
 writer.full(280)
 for _ in range(3):writer.full(256)
 writer.leaf(0)
 for a,r,g,b in values:
  for v in (g,r,b,a):writer.code(v,8)

def webp_chunk(kind,payload):return kind+struct.pack('<I',len(payload))+payload+(b'\0' if len(payload)%2 else b'')
def wrap_lossless(payload):
 body=b'WEBP'+webp_chunk(b'VP8L',payload)
 return b'RIFF'+struct.pack('<I',len(body))+body

def forced(w,h,transforms,mode=0,colors=16):
 writer=Writer();writer.header(w,h);coded=w
 for kind in transforms:
  writer.put(1,1);writer.put(kind,2)
  if kind in (0,1):
   writer.put(0,3);tw=(coded+3)//4;th=(h+3)//4
   values=[(255,0,mode,0) if kind==0 else (255,(i*13+255)%256,(i*7+128)%256,(i*5+255)%256) for i in range(tw*th)]
   image_literals(writer,tw,th,values,False)
  elif kind==3:
   writer.put(colors-1,8)
   values=[((i*17+35)%256,(i*43+37)%256,(i*31+59)%256,(i*29+21)%256) for i in range(colors)]
   image_literals(writer,colors,1,values,False)
   pack=8 if colors<=2 else 4 if colors<=4 else 2 if colors<=16 else 1;coded=(coded+pack-1)//pack
 writer.put(0,1)
 values=[((x*17+y*31)%256,(x*13+y*19)%256,(x*29+y*37)%256,(x*31+y*43)%256) for y in range(h) for x in range(coded)]
 image_literals(writer,coded,h,values,True)
 return wrap_lossless(b'\x2f'+writer.data)

def compare_data(name,data):
 reference=Image.open(BytesIO(data)).convert('RGBA');p=subprocess.run([str(PROBE)],input=data,capture_output=True,timeout=5)
 assert p.returncode==0,(name,p.stderr.decode());w,h=struct.unpack('<II',p.stdout[:8]);assert (w,h)==reference.size
 assert p.stdout[8:]==reference.tobytes(),('forced feature mismatch',name)
 (OUT/(name+'.webp')).write_bytes(data)
 return {'case':name,'bytes':len(data),'forced_feature':True}

forced_results=[]
for w,h in ((1,9),(7,5),(19,17),(32,9)):
 for mode in range(14):forced_results.append(compare_data(f'forced-predictor{mode}-{w}x{h}',forced(w,h,[0],mode)))
for colors in (2,4,8,16,17,255):forced_results.append(compare_data(f'forced-palette{colors}',forced(19,17,[3],colors=colors)))
import itertools
for order in itertools.permutations(range(4)):
 forced_results.append(compare_data('forced-order-'+''.join(map(str,order)),forced(19,17,order,mode=11)))
# Two spatial entropy groups; no literal bits because all trees are single leaves.
w=Writer();w.header(8,4);w.put(0,1);w.put(0,1);w.put(1,1);w.put(0,3)
image_literals(w,2,1,[(255,0,0,0),(255,0,1,0)],False)
for red in (16,200):
 for c in (5,red,32,255,0):w.leaf(c)
forced_results.append(compare_data('forced-two-entropy-groups',wrap_lossless(b'\x2f'+w.data)))
# Cache insertion/read and overlapping LZ copy use actual coded symbols.
for use_cache in (False,True):
 w=Writer();w.header(4,1);w.put(0,1);w.put(int(use_cache),1)
 if use_cache:w.put(1,4)
 w.put(0,1);color=0xff100520;index=((color*0x1e35a7bd)&0xffffffff)>>31
 symbol=280+index if use_cache else 258
 w.two(282 if use_cache else 280,5,symbol)
 for c in (16,32,255,1):w.leaf(c)
 w.put(0,1)
 for _ in range(3 if use_cache else 1):w.put(1,1)
 forced_results.append(compare_data('forced-cache' if use_cache else 'forced-lz-overlap',wrap_lossless(b'\x2f'+w.data)))

results=list(forced_results);encoded=[]
for name,im in images:
 for method in (0,4,6):
  start=time.monotonic();buf=BytesIO();im.save(buf,format='WEBP',lossless=True,exact=True,method=method,quality=80);data=buf.getvalue()
  reference=Image.open(BytesIO(data)).convert('RGBA');(OUT/f'{name}-m{method}.webp').write_bytes(data);encoded.append(data)
  p=subprocess.run([str(PROBE)],input=data,capture_output=True,timeout=5)
  if p.returncode:
   print(name,method,'FAILED',p.stderr.decode(),flush=True);raise AssertionError(name)
  w,h=struct.unpack('<II',p.stdout[:8]);assert (w,h)==im.size,(name,'dimensions')
  if p.stdout[8:]!=reference.tobytes():
   ours=Image.frombytes('RGBA',(w,h),p.stdout[8:]);ours.save(OUT/f'{name}-m{method}-ours.png');reference.save(OUT/f'{name}-m{method}-reference.png')
   i=next(i for i,(a,b) in enumerate(zip(p.stdout[8:],reference.tobytes())) if a!=b)
   print(name,method,'PIXEL MISMATCH',i,p.stdout[8+i],reference.tobytes()[i]);raise AssertionError(name)
  results.append({'case':name,'method':method,'bytes':len(data),'elapsed_s':round(time.monotonic()-start,4)})
 print(name,'passed all 3 encoders',flush=True)
# Extended static container and inert EXIF/XMP metadata. Lossy and animated WebP
# have their own independent oracle checks in check_webp_lossy.py.
buf=BytesIO();images[-1][1].save(buf,format='WEBP',lossless=True,exact=True,exif=b'Exif\0\0II*\0\x08\0\0\0\0\0\0\0\0\0',xmp=b'<x:xmpmeta xmlns:x="adobe:ns:meta/">ChezCat original fixture</x:xmpmeta>')
results.append(compare_data('extended-metadata',buf.getvalue()))
# A syntactically encoded entropy map whose group identifier exceeds our resource budget.
w=Writer();w.header(4,4);w.put(0,1);w.put(0,1);w.put(1,1);w.put(0,3)
image_literals(w,1,1,[(255,2,0,0)],False)
proc=subprocess.run([str(PROBE)],input=wrap_lossless(b'\x2f'+w.data),capture_output=True,timeout=5)
assert proc.returncode==2 and b'entropy-group limit' in proc.stderr
for i in range(600):
 data=bytearray(encoded[i%len(encoded)])
 if i<300:
  if i%3==0:data=data[:rng.randrange(len(data))]
  else:
   for _ in range(1+i%3):data[rng.randrange(len(data))]=rng.randrange(256)
 else:
  start=data.find(b'VP8L');size=struct.unpack('<I',data[start+4:start+8])[0];payload=bytearray(data[start+8:start+8+size])
  if i%3==0:payload=payload[:rng.randrange(len(payload))]
  else:
   for _ in range(1+i%3):payload[rng.randrange(len(payload))]=rng.randrange(256)
  data=wrap_lossless(bytes(payload))
 p=subprocess.run([str(PROBE)],input=data,capture_output=True,timeout=5)
 assert p.returncode in (0,2),(i,p.returncode,p.stderr)
 if p.returncode==0:
  w,h=struct.unpack('<II',p.stdout[:8]);assert w*h<=4194304 and len(p.stdout)==8+w*h*4
report={'valid_cases':len(results),'independent_reference':'Pillow/libwebp lossless encoding/decoding','malformed_mutations':600,'results':results}
(ROOT/'webp-compare-results.json').write_text(json.dumps(report,indent=2)+'\n', encoding="utf-8")
if args.output:
 args.output.parent.mkdir(parents=True, exist_ok=True);args.output.write_text(json.dumps(report,indent=2)+'\n', encoding="utf-8")
print('PASS',len(results),'VP8L comparisons (89 forced features, 171 libwebp encoded, 1 extended metadata) plus 600 malformed mutations and explicit resource-limit checks.')
