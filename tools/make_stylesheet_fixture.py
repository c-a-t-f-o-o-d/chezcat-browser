"""Original compressible CSS with meaningful tail rules; no third-party assets."""
import gzip
import hashlib
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
def source():
    return ('/*'+'original fixture padding '*65000+'*/\n'+''.join(f'.padding{i}{{height:1px}}\n' for i in range(4096))+'#target{display:block;width:173px;height:29px;color:#123456}\n').encode('utf-8')
if __name__=='__main__':
    body=source();encoded=gzip.compress(body,mtime=0)
    (ROOT/'tools/fixtures/large-tail.css.gz').write_bytes(encoded)
    (ROOT/'tools/fixtures/large-tail.css.json').write_text(json.dumps({'scope':__doc__,'decoded_bytes':len(body),'decoded_sha256':hashlib.sha256(body).hexdigest(),'gzip_bytes':len(encoded),'gzip_sha256':hashlib.sha256(encoded).hexdigest(),'padding_rules':4096,'tail':'#target{display:block;width:173px;height:29px;color:#123456}'},indent=2)+'\n',encoding='utf-8')
    print('Original stylesheet:',len(body),'decoded bytes;',len(encoded),'gzip bytes')
