"""Explicit preparation of the fixed development corpus; never run by the browser."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import sys
from urllib.parse import urlparse
from urllib.request import Request, urlopen

import capture_tests
import foundation_corpus as corpus

def prepare(cache,offline=False):
    cache=capture_tests.checked_output_path(cache)
    if cache.is_relative_to(corpus.ROOT) or corpus.ROOT.is_relative_to(cache):
        raise ValueError('Choose separate external corpus storage')
    inputs=json.loads(corpus.INPUTS.read_bytes())['inputs']
    selection=json.loads(corpus.LOCK.read_bytes())
    if corpus.digest(corpus.canonical(inputs))!=selection['input_manifest_sha256']:
        raise ValueError('Frozen input manifest changed')
    for item in inputs:
        relative=PurePosixPath(item['path'])
        if relative.is_absolute() or '..' in relative.parts or item['repository'] not in ['wpt','html5lib-tests','html-build']:
            raise ValueError('Unsafe corpus path')
        target=capture_tests.checked_output_path(cache/item['repository']/relative)
        if not target.is_relative_to(cache):raise ValueError('Corpus path escaped cache')
        if target.is_file():data=target.read_bytes()
        else:
            if offline:raise ValueError('Required pinned input missing: '+item['path'])
            parsed=urlparse(item['url'])
            if parsed.scheme!='https' or parsed.hostname!='raw.githubusercontent.com' or item['revision'] not in parsed.path.split('/'):
                raise ValueError('Input URL is outside its fixed public revision')
            request=Request(item['url'],headers={'User-Agent':'ChezCat-foundation-data'})
            with urlopen(request,timeout=30) as response:data=response.read(item['bytes']+1)
        blob=hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data,usedforsecurity=False).hexdigest()
        if len(data)!=item['bytes'] or blob!=item['git_blob'] or corpus.digest(data)!=item['sha256']:
            raise ValueError('Pinned corpus bytes differ: '+item['path'])
        if not target.is_file():
            target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
    print('Verified pinned development inputs:',len(inputs),'files,',sum(item['bytes'] for item in inputs),'bytes')

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--cache',type=Path,required=True);parser.add_argument('--offline',action='store_true')
    args=parser.parse_args()
    try:prepare(args.cache,args.offline)
    except (OSError,ValueError) as error:print('Corpus preparation blocked: '+str(error),file=sys.stderr);return 2
    return 0
if __name__=='__main__':raise SystemExit(main())
