"""Normal Windows verification: fixed local fixtures, no device or public probes."""
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import uuid
import capture_tests

ROOT=Path(__file__).resolve().parents[1]
JOURNEYS=[['check_input_harness.py'],['check_input_harness.py','--event-namespace'],
 ['check_input_harness.py','--attribute-nodes'],['check_address_search.py'],
 ['check_stylesheet_harness.py'],['check_dispatch_harness.py'],['check_video_harness.py'],
 ['check_audio_harness.py'],['check_av_harness.py'],['check_authored_video_harness.py'],['check_media_interface_harness.py'],['check_media_controls_harness.py'],
 ['check_source_replacement_harness.py'],['check_animation_harness.py'],['check_license_acceptance.py'],
 ['check_navigation_harness.py'],['check_network_media_harness.py'],
 ['check_network_media_tls_harness.py'],['check_interframe_harness.py'],['check_background_harness.py'],
 ['check_authored_av_harness.py'],['check_network_authored_av_harness.py']]

def committed_source_identity(commit,root=None):
    """Read immutable input blobs, including edits hidden by index flags."""
    root=root or ROOT
    def included(name):
        return name in ['Cargo.toml','Cargo.lock','build.rs','LICENSE','rust-toolchain','rust-toolchain.toml','.cargo/config','.cargo/config.toml'] or name.startswith(('src/','data/','resources/','assets/','pages/','trust/','tools/fixtures/','tools/reference/'))
    entries=[]
    for row in subprocess.check_output(['git','ls-tree','-rz',commit],cwd=root).split(b'\0'):
        if not row:continue
        meta,raw_name=row.split(b'\t',1);mode,kind,oid=meta.split();name=raw_name.decode('utf-8')
        if included(name):
            if mode not in [b'100644',b'100755'] or kind!=b'blob':raise ValueError('Compilation inputs must be tracked regular files.')
            entries.append((name,oid))
    result={}
    with subprocess.Popen(['git','cat-file','--batch'],cwd=root,stdin=subprocess.PIPE,stdout=subprocess.PIPE) as process:
        for name,oid in entries:
            process.stdin.write(oid+b'\n');process.stdin.flush()
            header=process.stdout.readline().split();size=int(header[2])
            digest=hashlib.sha256()
            while size:
                part=process.stdout.read(min(size,1024*1024))
                if not part:raise ValueError('Truncated committed input.')
                digest.update(part);size-=len(part)
            if process.stdout.read(1)!=b'\n':raise ValueError('Invalid committed input framing.')
            result[name]=digest.hexdigest()
        process.stdin.close()
        if process.wait()!=0:raise ValueError('Cannot read committed inputs.')
    return result

def git_state():
    try:
        top=Path(subprocess.check_output(['git','rev-parse','--show-toplevel'],cwd=ROOT,text=True,stderr=subprocess.DEVNULL).strip()).resolve()
        if top!=ROOT.resolve():raise ValueError('not this repository')
        return (subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
                bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT)))
    except (OSError,subprocess.CalledProcessError,ValueError):
        revision=(ROOT/'SOURCE_REVISION').read_text(encoding='utf-8').strip()
        return (revision if re.fullmatch('[0-9a-f]{40}',revision) else None,None)

def build_identity(source):
    commit,dirty=git_state()
    version=re.search(r'^version\s*=\s*"([^"]+)"',(ROOT/'Cargo.toml').read_text(encoding='utf-8'),re.M)[1]
    return {'source_commit':commit,'dirty':dirty,'cargo_version':version,
      'compiler':subprocess.check_output(['rustup','run','1.99.1','rustc','-vV'],cwd=ROOT,text=True),
      'python':sys.version.split()[0],'source_identity':source,
      'source_map_sha256':hashlib.sha256(json.dumps(source,sort_keys=True,separators=(',',':')).encode()).hexdigest(),
      'executable_sha256':capture_tests.sha256(ROOT/'ChezCat.exe'),
      'scope':'Build identity only; no reproducibility or security certification.'}

def main():
    folder=ROOT/'results/verification/offline'/(
        datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-'+uuid.uuid4().hex[:8])
    folder.mkdir(parents=True); runs=[]
    pointer=ROOT/'results/verification/latest-offline.json'
    relative=str((folder/'verification.json').relative_to(ROOT))
    initial_commit,initial_dirty=git_state()
    capture_tests.write_json(folder/'verification.json',{'passed':False,'state':'running','runs':[],'source_commit':initial_commit})
    capture_tests.write_json(pointer,{'report':relative,'build_info':str((folder/'build-info.json').relative_to(ROOT))})
    before=capture_tests.source_identity(ROOT)
    overrides=[name for name in ['RUST_TEST_THREADS','RUST_MIN_STACK','RUSTUP_TOOLCHAIN','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','RUSTC','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','CARGO_TARGET_DIR','CARGO_BUILD_TARGET'] if os.environ.get(name)]
    if initial_dirty is True or overrides:
        capture_tests.write_json(folder/'verification.json',{'passed':False,'source_commit':initial_commit,'state':'refused','reason':'Clean normal verification requires no tracked edits or listed build/test overrides.','override_names':overrides,'dirty':initial_dirty})
        print('Refused dirty/overridden verification; latest pointer invalidated.',flush=True);return 1
    if initial_dirty is False and committed_source_identity(initial_commit)!=before:
        capture_tests.write_json(folder/'verification.json',{'passed':False,'source_commit':initial_commit,'state':'refused','reason':'Compilation inputs differ from committed bytes, including hidden or untracked edits.'})
        print('Refused source/commit mismatch; latest pointer invalidated.',flush=True);return 1
    def run(label,argv,timeout=600):
        row=capture_tests.execute(argv,folder/(label+'.log'),ROOT,timeout)
        row={'name':label,**row};print(label+': '+row['state'],flush=True);return row
    for label,argv in [('license-acceptance',[sys.executable,'tools/accept_license.py','--check']),
       ('publication-boundary',[sys.executable,'tools/check_publication.py']),
       ('publication-controls',[sys.executable,'tools/check_publication_controls.py']),
       ('artifact-guards',[sys.executable,'tools/check_release_bundle.py']),
       ('version-parser-controls',[sys.executable,'tools/check_product_version.py']),
       ('capture-controls',[sys.executable,'tools/check_capture_tests.py','--output',str(folder/'capture-controls.json')]),
       ('normal',[sys.executable,'tools/capture_tests.py','--toolchain','1.99.1']),
       ('release',['rustup','run','1.99.1','cmd.exe','/d','/c','Build.cmd']),
       ('pe',[sys.executable,'tools/check_windows_pe.py','--output',str(folder/'pe.json')])]:
        row=run(label,argv,900);runs.append(row)
        if row['state']!='completed' or row['exit_code']!=0:
            capture_tests.write_json(folder/'verification.json',{'passed':False,'source_commit':initial_commit,'runs':runs,'source_before':before});return 1
    expected_exe=capture_tests.sha256(ROOT/'ChezCat.exe')
    def journey(item):
        index,args=item;return run('journey-'+str(index),[sys.executable,'tools/'+args[0],*args[1:]],180)
    with ThreadPoolExecutor(max_workers=4) as pool:runs.extend(pool.map(journey,enumerate(JOURNEYS)))
    journey_records=[]
    for row in runs:
        if not row['name'].startswith('journey-'):continue
        log=(folder/(row['name']+'.log')).read_text(encoding='utf-8',errors='replace')
        evidence=re.findall(r'Evidence: (.+)',log)
        if len(evidence)!=1:journey_records.append({'valid':False});continue
        path=Path(evidence[0].strip()).resolve()
        if not path.is_relative_to(ROOT.resolve()):journey_records.append({'valid':False});continue
        manifest=json.loads((path/'manifest.json').read_text(encoding='utf-8'))
        valid=(manifest.get('executable_sha256',manifest.get('exe_sha256'))==expected_exe and manifest['source_before']==before and manifest['source_after']==before and all(manifest['checks'].values()))
        journey_records.append({'valid':valid,'manifest':str((path/'manifest.json').relative_to(ROOT)), 'sha256':capture_tests.sha256(path/'manifest.json')})
    after=capture_tests.source_identity(ROOT);final_commit,final_dirty=git_state()
    passed=(before==after and final_commit==initial_commit and final_dirty is not True and capture_tests.sha256(ROOT/'ChezCat.exe')==expected_exe and len(journey_records)==len(JOURNEYS) and all(r['valid'] for r in journey_records) and all(r['state']=='completed' and r['exit_code']==0 for r in runs))
    identity=build_identity(after);capture_tests.write_json(folder/'build-info.json',identity)
    report={'passed':passed,'runs':runs,'journeys':journey_records,'source_before':before,'source_after':after,
       'executable_sha256':identity['executable_sha256'],'source_commit':identity['source_commit'],
       'scope':f'Normal suite, release, PE and {len(JOURNEYS)} fixed local App/input journeys. No public service/audio device/native desktop claim.'}
    capture_tests.write_json(folder/'verification.json',report)
    (ROOT/'results/verification/latest-offline.json').write_text(json.dumps({'report':str((folder/'verification.json').relative_to(ROOT)),'build_info':str((folder/'build-info.json').relative_to(ROOT))},indent=2)+'\n',encoding='utf-8')
    print('Report: '+str(folder/'verification.json'));return 0 if passed else 1

if __name__=='__main__':raise SystemExit(main())
