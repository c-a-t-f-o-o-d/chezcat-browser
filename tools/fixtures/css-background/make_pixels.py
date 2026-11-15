"""Author two original opaque asymmetric PPM fixtures; no external tool/data."""
from pathlib import Path
import hashlib
import json


ROOT = Path(__file__).resolve().parent
control = [(182,39,213),(21,172,187),(246,127,31),(37,59,193),
    (17,83,149),(229,67,113),(11,239,193),(157,101,23),
    (97,173,43),(219,31,181),(59,109,233),(241,197,71),
    (31,53,79),(107,229,137),(193,149,47),(71,17,211)]
background = [(241,17,19),(13,227,31),(29,43,233),(247,223,7),
    (239,113,17),(23,211,193),(157,31,229),(71,223,113),
    (37,83,149),(251,251,251),(5,5,5),(193,101,47),
    (211,41,137),(83,173,239),(149,229,53),(227,71,191)]
manifest = {}
for name, colors in [('images/control.ppm', control), ('styles/Pixels/CornerBG.ppm', background)]:
    body = b'P6\n4 4\n255\n' + bytes(channel for color in colors for channel in color)
    path = ROOT / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(body)
    manifest[name] = {'width':4,'height':4,'rgb':[list(color) for color in colors],
        'bytes':len(body),'sha256':hashlib.sha256(body).hexdigest()}
(ROOT/'pixels.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8',newline='\n')
print('Authored two original 4x4 P6 images; no product execution.')
