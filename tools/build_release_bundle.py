"""Local source/runtime packaging; this tool never publishes artifacts."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import zipfile
import capture_tests
import check_publication as publication
import verify_offline

ROOT=Path(__file__).resolve().parents[1]
MAX_BYTES=512*1024*1024
MANDATORY={'README.md','LICENSE','LICENSE_SCOPE.md','THIRD_PARTY_NOTICES.md',
 'CONTRIBUTING.md','CONTRIBUTORS.md','SECURITY.md','CHANGELOG.md','HISTORY_IMPORT.md',
 'ENDORSED_BUILDS.md','Run.cmd','assets/chezcat.ico','assets/chezcat-icon.png',
 'licensing/CONTRIBUTOR_AGREEMENT.md','licensing/CHANGES-1.1.md',
 'docs/LICENSE_ACCEPTANCE.md','docs/CONTRIBUTOR_PRIVACY.md',
 'docs/CAPABILITIES.md','docs/BUILDING.md','docs/ARCHITECTURE.md','docs/ROADMAP.md',
 'docs/FEATURE_CONTRACTS.md','docs/RELEASING.md','docs/BRANDING.md',
 'docs/ENDORSEMENT_POLICY.md',
 'trust/roots.pem','trust/mozilla/certdata.txt','trust/mozilla/LICENSE-MPL-2.0.txt',
 'trust/mozilla/PROVENANCE.json','trust/mozilla/policy.json','trust/mozilla/selection.json'}
PREFIXES=('pages/','trust/','notices/','data/public_suffix/','docs/','licensing/')
def sha(data):return hashlib.sha256(data).hexdigest()
def git(*args):
    top=Path(subprocess.check_output(['git','rev-parse','--show-toplevel'],cwd=ROOT,text=True).strip()).resolve()
    if top!=ROOT.resolve():raise ValueError('Packaging requires this exact Git root, not a parent repository.')
    return subprocess.check_output(['git',*args],cwd=ROOT)
def require_clean():
    if git('status','--porcelain'):raise ValueError('Packaging requires a clean product checkout.')
    return git('rev-parse','HEAD').decode().strip()
def tree(commit):
    rows={}
    for row in git('ls-tree','-rz',commit).split(b'\0'):
        if row:
            meta,name=row.split(b'\t',1);mode,kind,oid=meta.split();rows[name.decode('utf-8')]=(mode,kind,oid.decode())
    return rows
def blob(oid):
    if int(git('cat-file','-s',oid))>MAX_BYTES:raise ValueError('Artifact input exceeds the finite byte allowance.')
    return git('cat-file','blob',oid)

def runtime_names(entries):
    selected={name for name in entries if name in MANDATORY or name.startswith(PREFIXES)}
    if not MANDATORY.issubset(selected):raise ValueError('Mandatory tracked runtime inputs are missing.')
    for name in selected:
        mode,kind,_=entries[name]
        if mode not in [b'100644',b'100755'] or kind!=b'blob':raise ValueError('Runtime inputs must be tracked regular files.')
    return selected

def approve_scope(entries,approved,commit):
    # This designation belongs to the selected containing snapshot. Neither a
    # parent nor a list of withdrawn public identities is a packaging input.
    if any(name.casefold()=='licensing/covered_history.txt' for name in entries):
        raise ValueError('Retired covered-history input is forbidden in a snapshot release.')
    name='LICENSE_SCOPE.md'
    if name not in entries or entries[name][0] not in [b'100644',b'100755'] or entries[name][1]!=b'blob':
        raise ValueError('Committed containing-revision license scope is required.')
    data=blob(entries[name][2])
    if len(data)>1024*1024:raise ValueError('License scope exceeds its finite byte allowance.')
    scope=data.decode('utf-8')
    lines=scope.splitlines()
    markers=[line for line in lines if re.match(r'^\s*Scope\s*:',line,re.I)]
    licenses=[line for line in lines if re.match(r'^\s*License-SHA256\s*:',line,re.I)]
    if markers!=['Scope: containing-source-revision']:
        raise ValueError('License scope needs one exact containing-source-revision designation.')
    digests=set(re.findall(r'(?<![0-9a-f])[0-9a-f]{64}(?![0-9a-f])',scope))
    if licenses!=['License-SHA256: '+approved] or digests!={approved}:
        raise ValueError('License scope needs one exact approved LICENSE digest.')
    if re.search(r'(?<![0-9a-f])[0-9a-f]{40}(?![0-9a-f])',scope,re.I):
        raise ValueError('Containing-revision scope must not pin stale source or ancestry identities.')
    return {name:data}

def approve_license(approved,tag,commit=None):
    local=ROOT/'LICENSE'
    if not local.is_file():raise ValueError('No operative LICENSE: distribution is not approved.')
    if local.stat().st_size>1024*1024:raise ValueError('Unexpected license size.')
    data=local.read_bytes()
    if b'DRAFT' in data or b'UNAPPROVED' in data:raise ValueError('Draft license cannot authorize packaging.')
    if not re.fullmatch('[0-9a-f]{64}',approved or '') or sha(data)!=approved:raise ValueError('Approved license hash is required and must match.')
    commit=commit or git('rev-parse','HEAD').decode().strip();entries=tree(commit)
    publication.check_tree(entries)
    publication.check_receipts(commit)
    publication.check_metadata(commit)
    if 'LICENSE' not in entries or entries['LICENSE'][0] not in [b'100644',b'100755'] or entries['LICENSE'][1]!=b'blob':raise ValueError('Approved LICENSE must be a tracked regular file.')
    if blob(entries['LICENSE'][2])!=data:raise ValueError('Approved license must match exact committed bytes.')
    version=re.search(rb'^version\s*=\s*"([^"]+)"',blob(entries['Cargo.toml'][2]),re.M)[1].decode()
    if tag!='v'+version or not re.fullmatch(r'v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?',tag or ''):raise ValueError('Tag must match exact Cargo version.')
    approve_scope(entries,approved,commit)
    return entries

def report_path(name):
    return capture_tests.evidence_path(name,ROOT)

def verified(commit):
    pointer=json.loads((ROOT/'results/verification/latest-offline.json').read_text(encoding='utf-8'))
    report=json.loads(report_path(pointer['report']).read_text(encoding='utf-8'))
    info=json.loads(report_path(pointer['build_info']).read_text(encoding='utf-8'))
    source=capture_tests.source_identity(ROOT)
    # The index can hide edits with assume-unchanged/skip-worktree. HEAD, status
    # and mutually consistent reports alone do not prove committed input bytes.
    if verify_offline.committed_source_identity(commit,ROOT)!=source:raise ValueError('Compilation inputs differ from committed bytes.')
    if not report['passed'] or report['source_commit']!=commit or report['source_before']!=source or report['source_after']!=source:raise ValueError('A current passing verification source identity is required.')
    if info['dirty'] is not False or info['source_commit']!=commit or info['source_identity']!=source or info['executable_sha256']!=report['executable_sha256']:raise ValueError('A clean exact build identity is required.')
    return report,info,source

def runtime_files(approved,tag,commit=None):
    entries=approve_license(approved,tag,commit)
    commit=commit or git('rev-parse','HEAD').decode().strip()
    verified(commit)
    selected=[]
    for name in runtime_names(entries):
        path=ROOT/name
        if path.is_symlink() or not path.resolve().is_relative_to(ROOT.resolve()):raise ValueError('Runtime input escaped the source root.')
        selected.append(path)
    if not (ROOT/'ChezCat.exe').is_file():raise ValueError('Build the exact executable first.')
    return sorted([ROOT/'ChezCat.exe',*selected])

def main():
    parser=argparse.ArgumentParser(description=__doc__);modes=parser.add_mutually_exclusive_group();modes.add_argument('--source',action='store_true');modes.add_argument('--review-source',action='store_true');parser.add_argument('--output',type=Path,required=True);parser.add_argument('--approved-license-sha256');parser.add_argument('--tag');args=parser.parse_args()
    commit=require_clean();out=args.output.resolve()
    if out.exists():raise ValueError('Choose a new output path; existing artifacts are preserved.')
    review=args.review_source
    if review and 'review' not in out.name.lower():raise ValueError('Unapproved review archives must be named explicitly as review material.')
    if args.source or review:
        if review:publication.check_receipts(commit)
        else:approve_license(args.approved_license_sha256,args.tag,commit)
        out.parent.mkdir(parents=True,exist_ok=True)
        subprocess.run(['git','archive','--format=zip','--prefix=ChezCat-source/','--output',str(out),commit],cwd=ROOT,check=True)
        manifest=None
    else:
        files=runtime_files(args.approved_license_sha256,args.tag,commit);entries=tree(commit);report,info,source=verified(commit)
        payloads={};total=0
        for path in files:
            name=path.relative_to(ROOT).as_posix()
            if name=='ChezCat.exe':
                if path.stat().st_size>MAX_BYTES:raise ValueError('Executable exceeds artifact allowance.')
                data=path.read_bytes()
                if sha(data)!=report['executable_sha256']:raise ValueError('Executable differs from verified bytes.')
            else:data=blob(entries[name][2])
            total+=len(data)
            if total>MAX_BYTES:raise ValueError('Runtime artifact exceeds the finite byte allowance.')
            payloads[name]=data
        if require_clean()!=commit or capture_tests.source_identity(ROOT)!=source:raise ValueError('Source changed during packaging.')
        manifest={name:sha(data) for name,data in payloads.items()}
        identity={'source_commit':commit,'clean':True,'compiler':info['compiler'],'license_sha256':args.approved_license_sha256,'license_scope':'containing-source-revision','tag':args.tag,'files':manifest,'self_hash_excluded':True}
        out.parent.mkdir(parents=True,exist_ok=True)
        with zipfile.ZipFile(out,'x',compression=zipfile.ZIP_DEFLATED) as z:
            for name,data in payloads.items():z.writestr('ChezCat/'+name,data)
            z.writestr('ChezCat/build-info.json',json.dumps(identity,indent=2)+'\n')
    with zipfile.ZipFile(out) as z:
        if z.testzip():raise ValueError('Archive CRC failure.')
        for item in z.infolist():
            name=Path(item.filename)
            if name.is_absolute() or '..' in name.parts:raise ValueError('Unsafe archive path.')
            if (args.source or review) and name.suffix.lower() in ['.exe','.dll','.pdb']:raise ValueError('Application binary in source archive.')
        if manifest and any(sha(z.read('ChezCat/'+name))!=digest for name,digest in manifest.items()):raise ValueError('Archive payload differs from verified manifest.')
    digest=sha(out.read_bytes());out.with_suffix(out.suffix+'.sha256').write_text(digest+'  '+out.name+'\n',encoding='utf-8')
    out.with_suffix(out.suffix+'.review.json').write_text(json.dumps({'source_commit':commit,'sha256':digest,'local_review_only':review,'publication_authorized':False},indent=2)+'\n',encoding='utf-8')
    print('Local artifact: '+str(out));print('SHA256: '+digest);print('UNAPPROVED LOCAL REVIEW SOURCE.' if review else 'No publication authorization is implied.')
    return 0
if __name__=='__main__':
    try:raise SystemExit(main())
    except (ValueError,OSError,KeyError,subprocess.CalledProcessError) as error:print(str(error),file=sys.stderr);raise SystemExit(1)
