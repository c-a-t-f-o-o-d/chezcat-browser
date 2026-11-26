"""Small offline publication boundary for public source and release inputs."""
import argparse
import json
from pathlib import Path, PurePosixPath
import subprocess
import sys
import asset_metadata

ROOT=Path(__file__).resolve().parents[1]
OWNER_EMAIL='21222662+c-a-t-f-o-o-d@users.noreply.github.com'
RECEIPT_FIELDS={'schema_version','license_version','license_sha256','accepted_scope','accepted_at_utc'}
# Official receipts are at most 2048 bytes. Allow generously padded JSON while
# bounding parsing and one batched Git read independently of filenames.
MAX_RECEIPT_JSON=64*1024
MAX_RECEIPT_SCAN_BYTES=64*1024*1024
MAX_RECEIPT_SCAN_FILES=20000
# Public records have an explicit purpose. Source, assets and reference inputs
# retain their established directories; new record categories require review.
PUBLIC_ROOT_FILES={'.editorconfig','.gitattributes','.gitignore','Build.cmd',
 'CHANGELOG.md','CONTRIBUTING.md','CONTRIBUTORS.md','Cargo.lock','Cargo.toml',
 'ENDORSED_BUILDS.md','HISTORY_IMPORT.md','LICENSE','LICENSE_SCOPE.md','README.md',
 'Run.cmd','SECURITY.md','SOURCE_REVISION','THIRD_PARTY_NOTICES.md','build.rs',
 'build_dev.cmd','rust-toolchain','rust-toolchain.toml'}
PUBLIC_DOC_FILES={'ARCHITECTURE.md','BRANDING.md','BUILDING.md','CAPABILITIES.md',
 'COMPATIBILITY.md','CONTRIBUTOR_PRIVACY.md','ENDORSEMENT_POLICY.md',
 'FEATURE_CONTRACTS.md','HTML_FOUNDATION_COVERAGE.json',
 'LICENSE_ACCEPTANCE.md','README.md','RELEASING.md',
 'ROADMAP.md','compatibility.json'}
RETIRED_DOC_PATHS={'docs/'+name.casefold() for name in (
 'CSS_FOUNDATION_PLAN.md','CSS_FOUNDATION_COVERAGE.json','CSS_STANDARDS_INVENTORY.json',
 'FEATURE_CONTRACTS_DETAIL.md','HTML_FOUNDATION_PLAN.md')}
# Retired documents may remain only in this already-published ancestry.
RETIRED_DOC_HISTORY_REVISION='8bbd41a76b501d323def28a28d06d59cde9d0bf8'
PUBLIC_LICENSE_FILES={'CHANGES-1.1.md','CONTRIBUTOR_AGREEMENT.md'}
PUBLIC_TOOL_DATA_FILES={'compatibility_cases.json','date_review_cases_040.json',
 'object_spread_review_cases_040.json','operator_supplement_cases_041.json',
 'promise_cases.json','review_bounds_040.json','review_cases_040.json',
 'review_cases_041.json','string_integration_review_cases_041.json'}
PUBLIC_SOURCE_DIRS={'assets','data','notices','pages','resources','src','trust'}
PUBLIC_TOOL_DIRS={'demos','fixtures','https-demo','reference'}
PUBLIC_CONFIGURATION_FILES={'.cargo/config','.cargo/config.toml',
 '.github/pull_request_template.md'}
PUBLIC_EXACT={name.casefold() for name in PUBLIC_ROOT_FILES|PUBLIC_CONFIGURATION_FILES}
PUBLIC_EXACT.update('docs/'+name.casefold() for name in PUBLIC_DOC_FILES)
PUBLIC_EXACT.update('licensing/'+name.casefold() for name in PUBLIC_LICENSE_FILES)
PUBLIC_EXACT.update('tools/'+name.casefold() for name in PUBLIC_TOOL_DATA_FILES)
MAX_ASSET_SCAN_FILES=256
MAX_ASSET_SCAN_BYTES=64*1024*1024

def check_tree(entries,*,historical=False):
    for name in entries:
        if not isinstance(name,str) or not name or '\\' in name or ':' in name or any(ord(char)<32 or ord(char)==127 for char in name):
            raise ValueError('Unsafe tracked publication path.')
        normalized=name.casefold();parts=normalized.split('/')
        if any(part in ('','.','..') for part in parts) or PurePosixPath(normalized).is_absolute():
            raise ValueError('Unsafe tracked publication path.')
        if any(part.startswith('.') for part in parts[1:]):
            raise ValueError('Hidden local state is not a publication input.')
        if historical and normalized in RETIRED_DOC_PATHS:continue
        if normalized in PUBLIC_EXACT:continue
        if len(parts)>1 and parts[0] in PUBLIC_SOURCE_DIRS:continue
        if parts[0]=='tools':
            if len(parts)==2 and PurePosixPath(parts[1]).suffix in ('.py','.rs'):continue
            if len(parts)>2 and parts[1] in PUBLIC_TOOL_DIRS:continue
        if len(parts)==3 and parts[0]=='.github':
            if parts[1]=='workflows' and PurePosixPath(parts[2]).suffix in ('.yml','.yaml'):continue
            if parts[1]=='issue_template' and PurePosixPath(parts[2]).suffix=='.md':continue
        raise ValueError('Unapproved publication path category.')
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

def check_assets(commit):
    """Inspect selected branding inputs, not immutable history or codec fixtures."""
    entries=[];objects={}
    for row in git('ls-tree','-rlz',commit,'--','assets/','pages/images/chezcat-icon.png').split(b'\0'):
        if not row:continue
        meta,name=row.split(b'\t',1);mode,kind,oid,size=meta.split()
        if mode not in (b'100644',b'100755') or kind!=b'blob':
            raise ValueError('Branding assets must be tracked regular files.')
        size=int(size)
        if size>asset_metadata.MAX_ASSET_BYTES:
            raise ValueError('Branding asset exceeds its finite inspection allowance.')
        entries.append((name.decode('utf-8'),oid));objects[oid]=size
    if len(entries)>MAX_ASSET_SCAN_FILES or sum(objects.values())>MAX_ASSET_SCAN_BYTES:
        raise ValueError('Branding asset scan exceeds its finite allowance.')
    if not objects:return True
    payload=subprocess.check_output(['git','cat-file','--batch'],cwd=ROOT,
        input=b''.join(oid+b'\n' for oid in objects))
    at=0;contents={}
    for oid,size in objects.items():
        end=payload.find(b'\n',at)
        if end<0 or payload[at:end]!=oid+b' blob '+str(size).encode():
            raise ValueError('Cannot verify committed branding asset scan input.')
        at=end+1
        if payload[at+size:at+size+1]!=b'\n':
            raise ValueError('Truncated committed branding asset scan input.')
        contents[oid]=payload[at:at+size];at+=size+1
    if at!=len(payload):raise ValueError('Unexpected committed branding asset scan output.')
    for name,oid in entries:asset_metadata.check_payload(name,contents[oid])
    icons={name:contents[oid] for name,oid in entries
           if name in ('assets/chezcat-icon.png','pages/images/chezcat-icon.png')}
    if len(icons)==2 and len(set(icons.values()))!=1:
        raise ValueError('Branding icon copies must have identical bytes.')
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
    candidate=git('rev-parse','--verify',ref+'^{commit}').decode().strip()
    check_metadata(candidate)
    check_assets(candidate)
    roots=git('rev-list','--max-parents=0',candidate).decode().splitlines()
    if len(roots)!=1:raise ValueError('Publication must retain a single snapshot lineage.')
    retired_history=None
    for commit in git('rev-list',candidate).decode().splitlines():
        entries=[p.decode('utf-8') for p in git('ls-tree','-rz','--name-only',commit).split(b'\0') if p]
        historical=False
        if commit!=candidate and any(path.casefold() in RETIRED_DOC_PATHS for path in entries):
            if retired_history is None:
                try:retired_history=set(git('rev-list',RETIRED_DOC_HISTORY_REVISION).decode().splitlines())
                except subprocess.CalledProcessError as error:
                    raise ValueError('Cannot verify retired-document ancestry; complete the recorded source history before publication.') from error
            historical=commit in retired_history
        check_tree(entries,historical=historical)
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
