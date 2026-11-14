#!/usr/bin/env python3
"""Regenerate the original geometric image-format workshop fixtures.
Python standard library only; no existing artwork, codec source or runtime image
libraries are used. These are test assets for ChezCat's original decoders.
Run: python tools/make_image_workshop.py
"""
from pathlib import Path
import math
import struct
import zlib
ROOT = Path(__file__).resolve().parents[1]
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

def lossless_webp(width, height, rgba):
    # A deliberately simple original test encoder: canonical 8-bit literals.
    # It creates a valid VP8L stream without using an image codec dependency.
    data=bytearray();at=0
    def put(value,count):
        nonlocal at
        for shift in range(count):
            if at//8==len(data):data.append(0)
            data[at//8]|=((value>>shift)&1)<<(at%8);at+=1
    def tree(alphabet):
        put(0,1);put(8,4)
        for code in (17,18,0,1,2,3,4,5,16,6,7,8):put(int(code==8 or (code==0 and alphabet>256)),3)
        put(0,1)
        if alphabet>256:
            for i in range(alphabet):put(int(i<256),1)
    put(width-1,14);put(height-1,14);put(1,1);put(0,3)
    put(0,1);put(0,1);put(0,1)
    tree(280)
    for _ in range(3):tree(256)
    put(1,1);put(0,1);put(0,1);put(0,1) # Single distance symbol zero.
    for r,g,b,a in rgba:
        for component in (g,r,b,a):
            for shift in reversed(range(8)):put((component>>shift)&1,1)
    payload=b'\x2f'+data
    chunk=b'VP8L'+struct.pack('<I',len(payload))+payload+(b'\0' if len(payload)%2 else b'')
    return b'RIFF'+struct.pack('<I',len(chunk)+4)+b'WEBP'+chunk

def icon(entries, cursor=False):
    directory=bytearray(struct.pack('<HHH',0,2 if cursor else 1,len(entries)))
    offset=6+16*len(entries)
    for size,data in entries:
        directory.extend(struct.pack('<BBBBHHII',size%256,size%256,0,0,4 if cursor else 1,4 if cursor else 32,len(data),offset));offset+=len(data)
    return bytes(directory)+b''.join(data for size,data in entries)

def make():
    target=ROOT/'pages/images';target.mkdir(exist_ok=True)
    width,height=240,144
    palette=[(11,21,60,255),(20,47,79,255),(29,84,111,255),(36,124,139,255),
      (42,169,164,255),(66,199,188,255),(142,224,203,255),(223,246,220,255),
      (255,243,220,255),(255,222,161,255),(255,191,111,255),(239,144,88,255),
      (214,99,80,255),(162,65,83,255),(105,45,84,255),(54,34,74,180)]
    samples=[((x//20+y//18)%16,) for y in range(height) for x in range(width)]
    (target/'format-adam7-palette.png').write_bytes(png(width,height,3,4,True,samples,palette,bytes(p[3] for p in palette)))
    samples=[]
    for y in range(height):
        for x in range(width):
            dx,dy=(x-120)/112,(y-72)/64; radius=math.hypot(dx,dy)
            alpha=max(0,min(1,(1-radius)*3));r=x/239;g=y/143;b=(1-r)*.65+.2
            samples.append(tuple(round(max(0,min(1,v))*65535) for v in (r,g,b,alpha)))
    (target/'format-rgba16.png').write_bytes(png(width,height,6,16,False,samples))
    rows=[];webp_pixels=[]
    for y in range(height):
        row=bytearray()
        for x in range(width):
            color=(26+x*20//239,42+y*50//143,85+y*50//143)
            if (x-183)**2+(y-37)**2<24**2:color=(255,221,160)
            if y>95+int(13*math.sin(x/36)):color=(42,169,164)
            if y>120+int(12*math.cos(x/47)):color=(11,61,77)
            webp_pixels.append(color+(255,))
            r,g,b=color;word=(round(r*31/255)<<11)|(round(g*63/255)<<5)|round(b*31/255)
            row.extend(struct.pack('<H',word))
        rows.append(bytes(row))
    (target/'format-lossless.webp').write_bytes(lossless_webp(width,height,webp_pixels))
    (target/'format-rgb565.bmp').write_bytes(bmp(width,height,16,[],b''.join(rows[::-1]),compression=3,masks=(0xf800,0x7e0,0x1f)))
    rle=bytearray()
    for y in reversed(range(height)):
        for tile in range(12):rle.extend((20,(tile+y//18)%16))
        rle.extend((0,0))
    rle.extend((0,1))
    (target/'format-rle8.bmp').write_bytes(bmp(width,height,8,palette,bytes(rle),compression=1))
    def cat(size):
        pixels=[]
        for y in range(size):
            for x in range(size):
                u,v=(x+.5)/size,(y+.5)/size
                inside=((u-.5)**2/0.32**2+(v-.55)**2/0.3**2<1) or (.2<u<.42 and .14<v<.43 and u+v>.42) or (.58<u<.8 and .14<v<.43 and 1-u+v>.42)
                color=(66,199,188,255) if inside else (0,0,0,0)
                if inside and ((u-.38)**2+(v-.51)**2<.025**2 or (u-.62)**2+(v-.51)**2<.025**2):color=(11,21,60,255)
                if inside and abs(u-.5)<.028 and .58<v<.63:color=(255,191,111,255)
                pixels.append(color)
        return pixels
    small=png(32,32,6,8,False,cat(32));pixels=cat(64)
    dib=struct.pack('<IiiHHIIiiII',40,64,128,1,32,0,64*64*4+8*64,0,0,0,0)
    dib+=bytes(v for y in reversed(range(64)) for r,g,b,a in pixels[y*64:(y+1)*64] for v in (b,g,r,a))
    dib+=bytes(8*64)
    (target/'format-icon.ico').write_bytes(icon([(32,small),(64,dib)]))
    (target/'format-icon.cur').write_bytes(icon([(64,dib)],cursor=True))
    (target/'format-icon-png.ico').write_bytes(icon([(64,png(64,64,6,8,True,pixels))]))
    vector='''<svg xmlns="http://www.w3.org/2000/svg" width="240" height="144" viewBox="0 0 240 144">
<defs><linearGradient id="sky" x2="0" y2="1"><stop stop-color="#0b153c"/><stop offset="1" stop-color="#1d546f"/></linearGradient><radialGradient id="glow" fx="35%" fy="30%"><stop stop-color="#fff3dc"/><stop offset="1" stop-color="#ffbf6f"/></radialGradient><path id="star" d="M0-4L1-1 4 0 1 1 0 4-1 1-4 0-1-1Z"/></defs>
<rect width="240" height="144" rx="14" fill="url(#sky)"/><g fill="#fff3dc" opacity=".8"><use href="#star" x="30" y="26"/><use href="#star" x="95" y="17"/><use href="#star" x="136" y="43"/></g><circle cx="192" cy="35" r="23" fill="url(#glow)"/>
<path d="M0 108C38 68 70 130 118 94S203 79 240 96V144H0Z" fill="#2aa9a4"/><path d="M0 132Q65 99 113 124T240 118V144H0Z" fill="#173e51"/>
<g transform="translate(64 74) rotate(-7)"><path d="M-20 12L-19-20-5-10Q3-13 10-9L23-23 24 11C29 40-25 40-20 12Z" fill="#fff3dc" stroke="#0b153c" stroke-width="3" stroke-linejoin="round"/><g fill="#0b153c"><ellipse cx="-7" cy="8" rx="2" ry="4"/><ellipse cx="12" cy="8" rx="2" ry="4"/></g><path d="M0 16L4 20 8 16Z" fill="#e58858"/><path d="M4 20q-4 7-9 1m9-1q4 7 9 1" fill="none" stroke="#0b153c" stroke-width="2" stroke-linecap="round"/></g></svg>'''
    (target/'format-vector.svg').write_text(vector+'\n',encoding='utf-8')
    print('Wrote 9 original workshop fixtures to pages/images/.')
if __name__=='__main__':make()
