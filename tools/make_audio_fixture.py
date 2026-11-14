"""Author an original quiet stereo PCM scene using Python's standard library."""
import hashlib
import io
import json
import math
from pathlib import Path
import struct
import wave

ROOT=Path(__file__).resolve().parents[1]
def main():
    samples=bytearray()
    for frame in range(96_000):
        t=frame/48_000;envelope=min(1,frame/2400,(95_999-frame)/2400)
        samples.extend(struct.pack('<hh',round(3000*envelope*math.sin(2*math.pi*220*t)),round(2500*envelope*math.sin(2*math.pi*330*t))))
    file=io.BytesIO()
    with wave.open(file,'wb') as output:output.setnchannels(2);output.setsampwidth(2);output.setframerate(48_000);output.writeframes(samples)
    data=file.getvalue();out=ROOT/'tools/fixtures/original-tone.wav';out.write_bytes(data)
    out.with_suffix('.json').write_bytes((json.dumps({'scope':__doc__,'frames':96_000,'channels':2,'sample_rate':48_000,'bits':16,'duration_seconds':2,'wav_sha256':hashlib.sha256(data).hexdigest(),'pcm_sha256':hashlib.sha256(samples).hexdigest()},indent=2)+'\n').encode('utf-8'));print('Original PCM WAVE:',len(data),'bytes')
if __name__=='__main__':main()
