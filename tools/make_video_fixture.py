"""Author a tiny original intra-only VP8/IVF scene; encoder is development-only.

The shipped browser and offline checks use the checked-in bytes, never FFmpeg.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]
def scene():
    pixels=bytearray()
    for frame in range(6):
        for y in range(48):
            for x in range(64):
                color=(24+x*2,32+y*2,96)
                if 4+frame*7<=x<20+frame*7 and 14<=y<30:color=(232,48+frame*16,24)
                if y>=40:color=(24,180,40+frame*24)
                pixels.extend(color)
    return bytes(pixels)
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--ffmpeg',default=shutil.which('ffmpeg'));args=parser.parse_args()
    if not args.ffmpeg:parser.error('Choose an installed development encoder with --ffmpeg')
    out=ROOT/'tools/fixtures/scene-intra.ivf';raw=scene()
    options={'creationflags':subprocess.CREATE_NO_WINDOW} if sys.platform=='win32' else {}
    command=[args.ffmpeg,'-hide_banner','-loglevel','error','-f','rawvideo','-pixel_format','rgb24','-video_size','64x48','-framerate','2','-i','pipe:0','-an','-c:v','libvpx','-pix_fmt','yuv420p','-g','1','-auto-alt-ref','0','-lag-in-frames','0','-deadline','best','-cpu-used','0','-crf','8','-b:v','100k','-threads','1','-f','ivf','-y',str(out)]
    subprocess.run(command,input=raw,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=30,check=True,**options)
    reference=subprocess.run([args.ffmpeg,'-hide_banner','-loglevel','error','-i',str(out),'-f','rawvideo','-pix_fmt','rgba','pipe:1'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=30,check=True,**options).stdout
    data=out.read_bytes();offset=32;frames=[]
    while offset<len(data):
        size,tick=struct.unpack_from('<IQ',data,offset);offset+=12;payload=data[offset:offset+size];offset+=size
        assert len(payload)==size and payload[0]&1==0
        rgba=reference[len(frames)*64*48*4:(len(frames)+1)*64*48*4]
        frames.append({'timestamp':tick,'bytes':size,'payload_sha256':hashlib.sha256(payload).hexdigest(),'reference_rgba_sha256':hashlib.sha256(rgba).hexdigest()})
    assert len(frames)==6 and len(reference)==6*64*48*4
    out.with_suffix('.rgba').write_bytes(reference)
    metadata={'scope':__doc__,'source_rgb_sha256':hashlib.sha256(raw).hexdigest(),'ivf_sha256':hashlib.sha256(data).hexdigest(),'reference_rgba_sha256':hashlib.sha256(reference).hexdigest(),'width':64,'height':48,'rate':struct.unpack_from('<I',data,16)[0],'scale':struct.unpack_from('<I',data,20)[0],'frames':frames,'encoder_command':['ffmpeg']+command[1:-1]+['tools/fixtures/scene-intra.ivf']}
    out.with_suffix('.json').write_text(json.dumps(metadata,indent=2)+'\n',encoding='utf-8');print('Original IVF:',len(data),'bytes;',len(frames),'real VP8 keyframes')
if __name__=='__main__':main()
