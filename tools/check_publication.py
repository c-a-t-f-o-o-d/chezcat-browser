"""Small offline publication boundary: tracked private reports and owner metadata."""
import argparse
import fnmatch
import json
from pathlib import Path, PurePosixPath
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]
OWNER_EMAIL='21222662+c-a-t-f-o-o-d@users.noreply.github.com'
RECEIPT_FIELDS={'schema_version','license_version','license_sha256','accepted_scope','accepted_at_utc'}
# Official receipts are at most 2048 bytes. Allow generously padded JSON while
# bounding parsing and one batched Git read independently of filenames.
MAX_RECEIPT_JSON=64*1024
MAX_RECEIPT_SCAN_BYTES=64*1024*1024
MAX_RECEIPT_SCAN_FILES=20000
PRIVATE_EXACT={'AGENTS.md','INSTRUCTIONS.md','STATUS.md','docs/AUTONOMY_STATE.md',
 'docs/SESSION.md','docs/STATUS.md','docs/WORKFLOW.md','licensing/COVERED_HISTORY.txt',
 'tools/url-041/baseline.json','tools/url-041/current.json','tools/url-041/first-replay.json',
 'tools/url-041/edge-review-results.json','tools/url-041/rust-tests.txt',
 'tools/url-041/URL_API_NOTES_0.41.txt'}
PRIVATE_PREFIXES=('results/','dist/','recovery/','private/','.codex/','.vscode/','docs/prompts/')

def check_tree(entries):
    for name in entries:
        normalized=name.replace('\\','/').casefold()
        if normalized in {p.casefold() for p in PRIVATE_EXACT} or normalized.startswith(tuple(p.casefold() for p in PRIVATE_PREFIXES)) or fnmatch.fnmatchcase(normalized,'tools/release-checks-*.json'):
            raise ValueError('Private working material or raw validation report in publication tree.')
        if PurePosixPath(normalized).is_absolute() or '..' in PurePosixPath(normalized).parts:
            raise ValueError('Unsafe tracked publication path.')
    return True

def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT)

def check_receipt_payload(data):
    if len(data)>MAX_RECEIPT_JSON:return True
    try:value=json.loads(data)
    except (ValueError,UnicodeError,RecursionError):return True
    # Extra context and stale/corrupt values do not make local state public. A source
    # file generating a fixture is not itself a top-level receipt JSON object.
    if isinstance(value,dict) and RECEIPT_FIELDS<=value.keys():
        raise ValueError('Private license acceptance receipt state is forbidden in publication payloads.')
    return True

def check_receipts(commit):
    objects={}
    for row in git('ls-tree','-rlz',commit).split(b'\0'):
        if not row:continue
        meta,_=row.split(b'\t',1);mode,kind,oid,size=meta.split()
        if mode not in (b'100644',b'100755') or kind!=b'blob':continue
        size=int(size)
        if size<=MAX_RECEIPT_JSON:objects[oid]=size
    if len(objects)>MAX_RECEIPT_SCAN_FILES or sum(objects.values())>MAX_RECEIPT_SCAN_BYTES:
        raise ValueError('Private acceptance-state scan exceeds its finite allowance.')
    if not objects:return True
    # One Git batch avoids spawning a process for every ordinary source file.
    payload=subprocess.check_output(['git','cat-file','--batch'],cwd=ROOT,
        input=b''.join(oid+b'\n' for oid in objects))
    at=0
    for oid,size in objects.items():
        end=payload.find(b'\n',at)
        if end<0 or payload[at:end]!=oid+b' blob '+str(size).encode():
            raise ValueError('Cannot verify committed acceptance-state scan input.')
        at=end+1
        if payload[at+size:at+size+1]!=b'\n':
            raise ValueError('Truncated committed acceptance-state scan input.')
        check_receipt_payload(payload[at:at+size]);at+=size+1
    if at!=len(payload):raise ValueError('Unexpected committed acceptance-state scan output.')
    return True

def check_identity(name,email):
    owner=name.casefold().replace(' ','') in ('adammaddox','adam','c-a-t-f-o-o-d')
    if owner and email!=OWNER_EMAIL:raise ValueError('Owner author/committer/tagger must use verified GitHub noreply identity.')

def check_metadata(commit):
    # Examine actual commit objects, including author fields preserved by cherry-pick.
    for line in git('log',commit,'--format=%an%x09%ae%x09%cn%x09%ce').decode('utf-8').splitlines():
        an,ae,cn,ce=line.split('\t');check_identity(an,ae);check_identity(cn,ce)
    return True

def check(ref='HEAD'):
    check_metadata(ref)
    roots=git('rev-list','--max-parents=0',ref).decode().splitlines()
    if len(roots)!=1:raise ValueError('Publication must retain a single snapshot lineage.')
    for commit in git('rev-list',ref).decode().splitlines():
        entries=[p.decode('utf-8') for p in git('ls-tree','-rz','--name-only',commit).split(b'\0') if p]
        check_tree(entries)
        check_receipts(commit)
        if git('show',commit+':SOURCE_REVISION')!=b'$Format:%H$\n':
            raise ValueError('Committed SOURCE_REVISION must remain the literal export template.')
    scope=git('show',roots[0]+':LICENSE_SCOPE.md')
    if b'\nScope: containing-source-revision\n' not in scope:raise ValueError('Root must designate its containing snapshot.')
    # Tags can otherwise reintroduce a personal address independently of commits.
    for row in git('for-each-ref','refs/tags','--format=%(objecttype)%09%(taggername)%09%(taggeremail)').decode().splitlines():
        kind,name,email=row.split('\t')
        if kind=='tag':check_identity(name,email.strip('<>'))
    return True

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--ref',default='HEAD');args=parser.parse_args()
    try:check(args.ref)
    except (ValueError,OSError,subprocess.CalledProcessError) as error:
        print('Publication boundary failed: '+str(error),file=sys.stderr);raise SystemExit(1)
    print('Publication paths, snapshot template and owner metadata: PASS (not an exhaustive privacy audit).')
