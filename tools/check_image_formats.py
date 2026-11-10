#!/usr/bin/env python3
"""Check expanded PNG, BMP and ICO decoding against original pixels and Pillow.
Pillow/ImageMagick are development-only comparison tools. The browser uses no
external decoder. Run: python tools/check_image_formats.py --output report.json
For generated workshop assets use tools/make_image_workshop.py instead.
"""
from pathlib import Path
from PIL import Image
from io import BytesIO
import zlib,struct,subprocess,json,random
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

OUT=ROOT/'raster-fixtures'; OUT.mkdir(exist_ok=True)
PASSES=((0,0,8,8),(4,0,8,8),(0,4,4,8),(2,0,4,4),(0,2,2,4),(1,0,2,2),(0,1,1,2))
def chunk(kind,body):return struct.pack('>I',len(body))+kind+body+struct.pack('>I',zlib.crc32(kind+body))
def paeth(a,b,c):
 p=a+b-c; ds=(abs(p-a),abs(p-b),abs(p-c));return (a,b,c)[ds.index(min(ds))]
def pack_samples(values,depth):
 if depth==16:return b''.join(struct.pack('>H',v) for v in values)
 if depth==8:return bytes(values)
 out=bytearray((len(values)*depth+7)//8)
 for i,v in enumerate(values):out[i*depth//8]|=v<<(8-depth-i*depth%8)
 return bytes(out)
def png(w,h,color,depth,interlace,samples,palette=None,trns=None):
 channels={0:1,2:3,3:1,4:2,6:4}[color];distance=max(1,(channels*depth+7)//8);scan=bytearray()
 for pi,(x,y,dx,dy) in enumerate(PASSES if interlace else [(0,0,1,1)]):
  if x>=w or y>=h:continue
  prev=None
  for row in range(y,h,dy):
   raw=pack_samples([v for col in range(x,w,dx) for v in samples[row*w+col]],depth)
   if prev is None:prev=bytes(len(raw))
   kind=(row+pi)%5;filtered=bytearray()
   for i,v in enumerate(raw):
    a=raw[i-distance] if i>=distance else 0;b=prev[i];c=prev[i-distance] if i>=distance else 0
    prediction=(0,a,b,(a+b)//2,paeth(a,b,c))[kind];filtered.append((v-prediction)&255)
   scan.append(kind);scan.extend(filtered);prev=raw
 out=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,depth,color,0,0,int(interlace)))
 if palette:out+=chunk(b'PLTE',bytes(v for p in palette for v in p[:3]))
 if trns is not None:out+=chunk(b'tRNS',trns)
 encoded=zlib.compress(bytes(scan),9);mid=len(encoded)//2
 return out+chunk(b'IDAT',encoded[:mid])+chunk(b'IDAT',encoded[mid:])+chunk(b'IEND',b'')
def ours(data):
 p=subprocess.run([str(PROBE)],input=data,capture_output=True,timeout=3)
 assert p.returncode==0,p.stderr.decode()
 w,h=struct.unpack('<II',p.stdout[:8]);assert len(p.stdout)==8+4*w*h
 return Image.frombytes('RGBA',(w,h),p.stdout[8:])
def scale(v,depth):return (v*255+((1<<depth)-1)//2)//((1<<depth)-1)
def png_expected(samples,color,depth,palette,transparent):
 pixels=[]
 for sample in samples:
  if color==3: rgba=palette[sample[0]]
  else:
   v=[scale(n,depth) for n in sample]
   if color in (0,4):rgba=(v[0],v[0],v[0],v[1] if color==4 else 0 if sample==transparent else 255)
   else:rgba=tuple(v[:3])+(v[3] if color==6 else 0 if sample==transparent else 255,)
  pixels.extend(rgba)
 return bytes(pixels)
def bmp(w,h,depth,palette,rows,compression=0,masks=None,header=40,top=False):
 if header==12:
  dib=struct.pack('<IHHHH',12,w,h,1,depth);pal=bytes(v for p in palette for v in (p[2],p[1],p[0]));extra=b''
 else:
  dib=bytearray(header);struct.pack_into('<IiiHHIIiiII',dib,0,header,w,-h if top else h,1,depth,compression,len(rows),0,0,len(palette),0)
  extra=b''
  if masks:
   packed=struct.pack('<'+'I'*len(masks),*masks)
   if header==40:extra=packed
   else:dib[40:40+len(packed)]=packed
  pal=bytes(v for p in palette for v in (p[2],p[1],p[0],0))
 data=bytes(dib)+extra+pal+rows;offset=14+len(dib)+len(extra)+len(pal)
 return struct.pack('<2sIHHI',b'BM',len(data)+14,0,0,offset)+data

def run():
 results=[];fixtures=[]
 def check(name,data,expected,size,oracle=True,tolerance=0):
  (OUT/name).write_bytes(data);fixtures.append(data);decoded=ours(data)
  assert decoded.size==size,(name,decoded.size,size)
  assert decoded.tobytes()==expected,('analytic pixels',name)
  record={'name':name,'analytic':'exact'}
  if oracle:
   try:
    reference=Image.open(BytesIO(data));reference.load()
    if reference.mode in ('I','I;16','I;16B','I;16L'):
     ref=bytes(v for p in getattr(reference,'get_flattened_data',reference.getdata)() for v in (scale(p,16),)*3+(255,))
    else:ref=reference.convert('RGBA').tobytes()
    delta=max(abs(a-b) for a,b in zip(ref,expected)) if len(ref)==len(expected) else 999
    if delta>tolerance:
     alt=subprocess.run(['convert',str(OUT/name),'-depth','8','rgba:-'],capture_output=True,check=True).stdout
     other=max(abs(a-b) for a,b in zip(alt,expected)) if len(alt)==len(expected) else 999
     assert other<=max(1,tolerance),('both oracle mismatch',name,delta,other,tolerance)
     record['pillow_difference']=delta;record['imagemagick_max_channel_difference']=other
    else:record['pillow_max_channel_difference']=delta
   except (OSError,ValueError) as e:record['pillow_unsupported']=str(e)
  results.append(record)
 for color,depths in ((0,(1,2,4,8,16)),(2,(8,16)),(3,(1,2,4,8)),(4,(8,16)),(6,(8,16))):
  for depth in depths:
   maxsample=(1<<depth)-1;channels={0:1,2:3,3:1,4:2,6:4}[color]
   palette=[((i*53)%256,(i*137)%256,(i*29)%256,255 if i%3 else 90) for i in range(1<<depth)] if color==3 else None
   for interlace in (False,True):
    for w,h in ((1,1),(2,3),(7,5),(17,13),(32,9)):
     samples=[tuple(((x*1031+y*517+c*139+(x*y*17))&maxsample) for c in range(channels)) for y in range(h) for x in range(w)]
     trns=bytes(p[3] for p in palette) if palette else None;transparent=None
     data=png(w,h,color,depth,interlace,samples,palette,trns)
     expected=png_expected(samples,color,depth,palette,transparent)
     check(f'c{color}-d{depth}-i{int(interlace)}-{w}x{h}.png',data,expected,(w,h),tolerance=int(depth==16))
   if color in (0,2):
    w,h=7,5;samples=[tuple(((x*1031+y*517+c*139)&maxsample) for c in range(channels)) for y in range(h) for x in range(w)]
    transparent=samples[9];trns=b''.join(struct.pack('>H',v) for v in transparent)
    # Full-precision transparency is asserted analytically; Pillow reduces certain
    # 16-bit tRNS images to 8-bit too early and is not this feature's oracle.
    check(f'transparency-c{color}-d{depth}.png',png(w,h,color,depth,True,samples,None,trns),png_expected(samples,color,depth,None,transparent),(w,h),oracle=depth!=16)
 for depth in (1,4,8,16,24,32):
  w,h=19,7;palette=[((i*53)%256,(i*137)%256,(i*29)%256,255) for i in range(1<<depth)] if depth<=8 else []
  for top in (False,True):
   rows=[];expected=[]
   for y in range(h):
    row=[];colors=[]
    for x in range(w):
     if depth<=8:v=(x+7*y)%(1<<depth);row.append(v);colors.extend(palette[v])
     elif depth==16:
      r,g,b=x%32,(y*4)%32,(x*3+y)%32;row.extend(struct.pack('<H',(r<<10)|(g<<5)|b));colors.extend((scale(r,5),scale(g,5),scale(b,5),255))
     else:
      r,g,b=x*11,y*37,(x+y)*7;row.extend((b,g,r));colors.extend((r,g,b,255))
      if depth==32:row.append((x*7)%256)
    raw=pack_samples(row,depth) if depth<=8 else bytes(row);raw+=bytes((-len(raw))%4);rows.append(raw);expected.extend(colors)
   check(f'rgb{depth}-top{int(top)}.bmp',bmp(w,h,depth,palette,b''.join(rows if top else rows[::-1]),top=top),bytes(expected),(w,h),tolerance=int(depth==16))
   if not top and depth in (1,4,8,24):check(f'core{depth}.bmp',bmp(w,h,depth,palette,b''.join(rows[::-1]),header=12),bytes(expected),(w,h))
 for depth,masks,comp,headers in ((16,(0xf800,0x07e0,0x001f),3,(40,52,108,124)),(16,(0x7c00,0x3e0,0x1f,0x8000),6,(40,56,108,124)),(32,(0xff0000,0xff00,0xff,0xff000000),3,(56,108,124)),(32,(0x3ff00000,0xffc00,0x3ff,0xc0000000),6,(56,108,124))):
  w,h=19,7;rows=[];expected=[]
  for y in range(h):
   row=bytearray()
   for x in range(w):
    value=0;rgba=[]
    for i,mask in enumerate(masks):
     shift=(mask&-mask).bit_length()-1;maxv=mask>>shift;n=(x*37+y*113+i*17)%(maxv+1);value|=n<<shift;rgba.append((n*255+maxv//2)//maxv)
    row.extend(value.to_bytes(depth//8,'little'));expected.extend(rgba+([255] if len(rgba)==3 else []))
   row+=bytes((-len(row))%4);rows.append(row)
  for header in headers:check(f'bitfields{depth}-c{comp}-h{header}-a{len(masks)}.bmp',bmp(w,h,depth,[],b''.join(rows[::-1]),compression=comp,masks=masks,header=header),bytes(expected),(w,h),tolerance=1)
 palette=[(i*17,255-i*17,i*11,255) for i in range(16)]
 for depth,comp in ((4,2),(8,1)):
  # Bottom row absolute mode, middle delta with missing pixels, top encoded runs.
  run=(bytes([0,7])+ (bytes([0x12,0x34,0x56,0x70]) if depth==4 else bytes([1,2,3,4,5,6,7,0]))+bytes([0,0,0,2,2,0,3,0x55 if depth==4 else 5,0,0,7,0x66 if depth==4 else 6,0,1]))
  expected=bytes(v for idx in [6]*7+[0,0,5,5,5,0,0]+[1,2,3,4,5,6,7] for v in palette[idx]);check(f'rle{depth}.bmp',bmp(7,3,depth,palette,run,compression=comp),expected,(7,3))
 # Existing original project icon: verifies highest-resolution entry selection.
 icon=REPO/'assets/chezcat.ico'
 if icon.exists():
  original=Image.open(icon);original.load();check('original-chezcat.ico',icon.read_bytes(),original.convert('RGBA').tobytes(),original.size)
 rng=random.Random(0xC4E2)
 malformed=[]
 for i in range(240):
  src=fixtures[i%len(fixtures)];data=bytearray(src)
  if i%3==0:data=data[:rng.randrange(len(data))]
  else:
   for j in range(1+i%4):data[rng.randrange(len(data))]=rng.randrange(256)
  p=subprocess.run([str(PROBE)],input=data,capture_output=True,timeout=3)
  assert p.returncode in (0,2),('panic',i,p.stderr)
  if p.returncode==0:
   w,h=struct.unpack('<II',p.stdout[:8]);assert 0<w<=16384 and 0<h<=16384 and w*h<=4194304 and len(p.stdout)==8+w*h*4
  malformed.append(p.returncode)
 report={'generated_valid':len(results),'pillow_comparable':sum('pillow_max_channel_difference'in v for v in results),'pillow_unsupported':sum('pillow_unsupported'in v for v in results),'malformed_mutations':len(malformed),'results':results}
 (ROOT/'raster-compare-results.json').write_text(json.dumps(report,indent=2)+'\n')
 if args.output:
  args.output.parent.mkdir(parents=True, exist_ok=True);args.output.write_text(json.dumps(report,indent=2)+'\n')
 print(json.dumps({k:v for k,v in report.items() if k!='results'},indent=2))
if __name__=='__main__':run()
