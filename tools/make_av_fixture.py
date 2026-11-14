"""Author original finite Matroska VP8/PCM16 using only Python stdlib.

VP8 packets and independent RGB reference reuse the original scene fixture.
The quiet three-second PCM signal is authored here. No runtime helper is used.
"""
from pathlib import Path
import hashlib,json,math,struct
ROOT=Path(__file__).resolve().parents[1]
def size(n):
    for width in range(1,9):
        if n<(1<<(7*width))-1:return ((1<<(7*width))|n).to_bytes(width,'big')
    raise ValueError('element too large')
def element(i,b):return i.to_bytes((i.bit_length()+7)//8,'big')+size(len(b))+b
def uint(i,n):return element(i,n.to_bytes(max(1,(n.bit_length()+7)//8),'big'))
def main():
    source=(ROOT/'tools/fixtures/scene-intra.ivf').read_bytes();at=32;frames=[]
    while at<len(source):
        n,t=struct.unpack_from('<IQ',source,at);at+=12;frames.append(source[at:at+n]);at+=n
    assert len(frames)==6
    samples=b''.join(struct.pack('<hh',round(2800*min(1,n/2400,(143999-n)/2400)*math.sin(2*math.pi*220*n/48000)),round(2300*min(1,n/2400,(143999-n)/2400)*math.sin(2*math.pi*330*n/48000))) for n in range(144000))
    header=element(0x1a45dfa3,uint(0x4286,1)+uint(0x42f7,1)+uint(0x42f2,4)+uint(0x42f3,8)+element(0x4282,b'matroska')+uint(0x4287,4)+uint(0x4285,2))
    info=element(0x1549a966,uint(0x2ad7b1,1000000)+element(0x4489,struct.pack('>d',3000))+element(0x4d80,b'ChezCat original fixture')+element(0x5741,b'ChezCat original fixture'))
    v=uint(0xd7,1)+uint(0x73c5,1)+uint(0x83,1)+uint(0x9c,0)+element(0x86,b'V_VP8')+uint(0x23e383,500000000)+element(0xe0,uint(0xb0,64)+uint(0xba,48))
    a=uint(0xd7,2)+uint(0x73c5,2)+uint(0x83,2)+uint(0x9c,0)+element(0x86,b'A_PCM/INT/LIT')+element(0xe1,element(0xb5,struct.pack('>d',48000))+uint(0x9f,2)+uint(0x6264,16))
    tracks=element(0x1654ae6b,element(0xae,v)+element(0xae,a));clusters=b''
    for i,payload in enumerate(frames):
        blocks=element(0xa3,b'\x81'+struct.pack('>hB',0,0x80)+payload)
        for part in range(4):
            start=(i*4+part)*6000*4
            blocks+=element(0xa3,b'\x82'+struct.pack('>hB',part*125,0x80)+samples[start:start+24000])
        clusters+=element(0x1f43b675,uint(0xe7,i*500)+blocks)
    data=header+element(0x18538067,info+tracks+clusters)
    out=ROOT/'tools/fixtures/scene-av.mkv';out.write_bytes(data)
    meta={'scope':__doc__,'mkv_sha256':hashlib.sha256(data).hexdigest(),'pcm_sha256':hashlib.sha256(samples).hexdigest(),'video_packet_sha256':[hashlib.sha256(p).hexdigest() for p in frames],'duration_ms':3000,'video_frames':6,'pcm_frames':144000,'sample_rate':48000,'channels':2,'bits':16,'audio_packet_frames':6000}
    out.with_suffix('.json').write_bytes((json.dumps(meta,indent=2)+'\n').encode('utf-8'));print('Original Matroska:',len(data),'bytes')
if __name__=='__main__':main()
