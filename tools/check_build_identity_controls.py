"""Original offline controls over the actual build script and tiny Cargo probe.

Only new owned fixture repositories/output are changed. No network, acceptance
creation, product executable replacement or global Git configuration. Direct
collector checks and real Cargo rebuilds establish separate bounded behaviors.
"""
from pathlib import Path
from datetime import datetime, timezone
import uuid
import argparse,contextlib,hashlib,json,os,re,shutil,sys,tomllib

ROOT=Path(__file__).resolve().parents[1]
VERSION='0.1.0+metadata-fixture'
INPUTS=('build.rs','src/build_identity.rs','src/license_acceptance.rs','src/crypto_hash.rs',
        'LICENSE','THIRD_PARTY_NOTICES.md','assets/chezcat.ico','resources/chezcat.rc',
        'resources/chezcat.res','resources/chezcat-x64.o')
KEYS={'CHEZCAT_EMBEDDED_SOURCE_REVISION','CHEZCAT_EMBEDDED_SOURCE_ORIGIN',
      'CHEZCAT_EMBEDDED_TREE_STATE','CHEZCAT_EMBEDDED_DECLARED_TAG'}
PROBE='''#[allow(dead_code)] mod crypto_hash;
#[allow(dead_code)] mod license_acceptance;
#[allow(dead_code)] mod build_identity;
fn main(){print!("{}",build_identity::current().safe_copy());}
'''
IDENTITY={'GIT_AUTHOR_NAME':'Controlled metadata fixture','GIT_COMMITTER_NAME':'Controlled metadata fixture',
          'GIT_AUTHOR_EMAIL':'fixture@users.noreply.github.com',
          'GIT_COMMITTER_EMAIL':'fixture@users.noreply.github.com'}
CLEAR=('GIT_DIR','GIT_WORK_TREE','GIT_INDEX_FILE','GIT_OBJECT_DIRECTORY',
       'GIT_ALTERNATE_OBJECT_DIRECTORIES','GIT_CONFIG_COUNT','CHEZCAT_DECLARED_BUILD_TAG',*KEYS)

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def write(path,text):path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(text.encode('utf-8'))
@contextlib.contextmanager
def environment(changes):
    previous=dict(os.environ)
    try:
        for key in CLEAR:os.environ.pop(key,None)
        os.environ.update(IDENTITY)
        os.environ.update(GIT_CONFIG_NOSYSTEM='1',GIT_CONFIG_GLOBAL=os.devnull,GIT_TERMINAL_PROMPT='0',GIT_OPTIONAL_LOCKS='0')
        os.environ.update(changes)
        yield
    finally:os.environ.clear();os.environ.update(previous)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo',default=ROOT,type=Path)
    parser.add_argument('--output',type=Path,help='new owned output directory within selected verification storage (legacy repo/results by default)')
    parser.add_argument('--toolchain',default='1.99.1-x86_64-pc-windows-msvc')
    args=parser.parse_args();root=args.repo.resolve(strict=True)
    sys.path.insert(0,str(root/'tools'));import capture_tests
    out=(args.output or capture_tests.verification_root(root)/'autonomy'/(datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-build-identity-controls-'+uuid.uuid4().hex[:8])).resolve()
    results=(root/'results').resolve()
    selected=capture_tests.verification_root(root)
    out=capture_tests.checked_output_path(out)
    allowed=selected if os.environ.get('CHEZCAT_VERIFICATION_ROOT') is not None else results
    if out==allowed or not out.is_relative_to(allowed):raise ValueError('Select a new owned output directory within selected verification storage.')
    if out.exists():raise ValueError('Output already exists; retain the previous evidence.')
    receipt=os.environ.get('CHEZCAT_LICENSE_RECEIPT_DIR')
    if not receipt:raise ValueError('Select the existing explicit external receipt; this helper never accepts terms.')
    sys.path.insert(0,str(root/'tools'));import accept_license
    accept_license.terms()
    if not accept_license.check(accept_license.directory()):raise ValueError('Existing exact-term external acceptance check failed; no consent is created.')
    rustup=shutil.which('rustup');git=shutil.which('git')
    if not rustup or not git:raise ValueError('Existing installed rustup and Git are required; no installation is attempted.')
    out.mkdir(parents=True);logs=out/'logs';logs.mkdir();empty_path=out/'empty-path';empty_path.mkdir();hooks=out/'empty-hooks';hooks.mkdir()
    sys.path.insert(0,str(root/'tools'));import capture_tests
    capture_tests.MAX_LOG_BYTES=4*1024*1024
    inputs={name:sha(root/name) for name in INPUTS};before_exe=sha(root/'ChezCat.exe') if (root/'ChezCat.exe').is_file() else None
    original_version=tomllib.loads((root/'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
    result={'schema_version':1,'passed':False,'scope':__doc__,'runner_sha256':sha(Path(__file__)),'source_root':str(root),'source_input_sha256':inputs,'product_source_version':original_version,'fixture_product_version':VERSION,'commands':[],'checks':{},'cases':{}}
    def save():capture_tests.write_json(out/'verification.json',result)
    def check(name,condition,detail=None):
        result['checks'][name]=bool(condition)
        if not condition and detail is not None:result.setdefault('failures',{})[name]=detail
        save()
    number=0
    def execute(name,command,cwd,env=None,timeout=60):
        nonlocal number;number+=1
        with environment(env or {}):row=capture_tests.execute([str(x) for x in command],logs/f'{number:02}-{name}.log',cwd,timeout)
        result['commands'].append({'name':name,**row});save()
        if row.get('state')!='completed' or row.get('exit_code')!=0:raise RuntimeError('Owned command failed: '+name+'; see retained local log.')
        return (logs/row['log']).read_text(encoding='utf-8')
    def fixture(name,revision='$Format:%H$\n',parent=None):
        path=(parent/name) if parent else out/name;path.mkdir()
        for relative in INPUTS:
            dest=path/relative;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(root/relative,dest)
            if sha(dest)!=inputs[relative]:raise ValueError('Fixture input changed while copying: '+relative)
        write(path/'SOURCE_REVISION',revision)
        write(path/'Cargo.toml',f'[package]\nname="chezcat-metadata-fixture"\nversion="{VERSION}"\nedition="2021"\nbuild="build.rs"\n\n[[bin]]\nname="metadata_probe"\npath="src/main.rs"\n\n[dependencies]\n')
        write(path/'Cargo.lock',f'version = 4\n\n[[package]]\nname = "chezcat-metadata-fixture"\nversion = "{VERSION}"\n')
        write(path/'src/main.rs',PROBE)
        return path
    git_prefix=[git,'-c','core.hooksPath='+str(hooks),'-c','commit.gpgsign=false','-c','core.fsmonitor=false','-c','core.untrackedCache=false']
    def local_git(name,path,*args):return execute(name,[*git_prefix,*args],path,timeout=30).strip()
    def initialize(path,name):
        local_git(name+'-init',path,'init','--initial-branch=metadata-control')
        local_git(name+'-add',path,'add','--all')
        local_git(name+'-commit',path,'commit','-m','Original controlled metadata fixture')
        value=local_git(name+'-head',path,'rev-parse','HEAD')
        if not re.fullmatch('[0-9a-f]{40}',value):raise ValueError('Fixture Git did not return its exact SHA.')
        return value
    def emitted(text):
        values={}
        for line in text.splitlines():
            if not line.startswith('cargo:rustc-env='):continue
            key,value=line[len('cargo:rustc-env='):].split('=',1)
            if key in KEYS:
                if key in values:raise ValueError('Duplicate actual build-script metadata key.')
                values[key]=value
        if set(values)!=KEYS:raise ValueError('Actual build script did not emit the exact expected metadata fields.')
        return values
    def script_case(name,path,script_bin,origin,tree,revision='',missing_git=False):
        env={'CARGO_MANIFEST_DIR':str(path),'CARGO_PKG_VERSION':VERSION,'CARGO_CFG_TARGET_OS':'windows','CARGO_CFG_TARGET_ARCH':'x86_64','CARGO_CFG_TARGET_ENV':'msvc'}
        if missing_git:env['PATH']=str(empty_path)
        data=emitted(execute(name,[script_bin],path,env));result['cases'][name]={'kind':'actual collector, not Cargo watcher proof','emitted':data,'missing_git_path':missing_git};save()
        expected={'CHEZCAT_EMBEDDED_SOURCE_REVISION':revision,'CHEZCAT_EMBEDDED_SOURCE_ORIGIN':origin,'CHEZCAT_EMBEDDED_TREE_STATE':tree,'CHEZCAT_EMBEDDED_DECLARED_TAG':''}
        check(name,data==expected,{'expected':expected,'actual':data})
        return data
    def probe(name,path,exe,revision,tree,missing_git=False):
        text=execute(name,[exe],path,{'PATH':str(empty_path)} if missing_git else {})
        expected_state={'clean':'Git working tree clean at build check','dirty':'Git working tree dirty at build check'}[tree]
        check(name,f'ChezCat {VERSION}\n' in text and f'Source revision: {revision}\n' in text and f'Source state: {expected_state}\n' in text and 'Build: Development build\n' in text,{'expected_revision':revision,'expected_tree':tree,'actual_native_probe':text})
        return text
    save()
    try:
        execute('existing-external-acceptance-check',[sys.executable,root/'tools/accept_license.py','--check'],root)
        bootstrap=fixture('build-script-inputs');binary_dir=out/'compiled-script';binary_dir.mkdir();script_bin=binary_dir/('actual_build_script.exe' if os.name=='nt' else 'actual_build_script')
        execute('compile-actual-build-script',[rustup,'run',args.toolchain,'rustc','--edition=2021','--crate-name','actual_chezcat_build_script',bootstrap/'build.rs','-o',script_bin],bootstrap,{'CARGO_PKG_VERSION':VERSION},120)
        cargo=fixture('cargo-fixture');first=initialize(cargo,'cargo-fixture')
        script_case('collector-missing-git-placeholder',bootstrap,script_bin,'unknown','unknown',missing_git=True)
        parent=out/'nested-parent';parent.mkdir();write(parent/'README.md','Original parent fixture: child exports must not adopt this source identity.\n');parent_sha=initialize(parent,'nested-parent')
        nested=fixture('exported-source',parent=parent)
        data=script_case('collector-nested-parent-placeholder',nested,script_bin,'unknown','unknown');check('nested-parent-source-not-adopted',data['CHEZCAT_EMBEDDED_SOURCE_REVISION']!=parent_sha)
        write(nested/'SOURCE_REVISION',first+'\n')
        script_case('collector-expanded-export-with-parent',nested,script_bin,'exported','unknown',first)
        script_case('collector-expanded-export-without-git',nested,script_bin,'exported','unknown',first,missing_git=True)
        target=out/'cargo-target';exe=target/'debug'/('metadata_probe.exe' if os.name=='nt' else 'metadata_probe')
        command=[rustup,'run',args.toolchain,'cargo','build','--offline','--locked','--target-dir',target]
        execute('cargo-initial-clean-build',command,cargo,timeout=180);probe('runtime-initial-clean',cargo,exe,first,'clean')
        initial_stat=exe.stat();initial_sha=sha(exe)
        execute('cargo-unchanged-second-build',command,cargo,timeout=180);probe('runtime-unchanged-second-build',cargo,exe,first,'clean')
        check('cargo-unchanged-build-preserves-executable',sha(exe)==initial_sha and exe.stat().st_mtime_ns==initial_stat.st_mtime_ns,{'before_sha256':initial_sha,'after_sha256':sha(exe),'before_mtime_ns':initial_stat.st_mtime_ns,'after_mtime_ns':exe.stat().st_mtime_ns})
        changed=cargo/'src/build_identity.rs';clean_bytes=changed.read_bytes();changed.write_bytes(clean_bytes+b'\n// Original metadata-control tracked dirty edit.\n')
        execute('cargo-tracked-edit-rebuild',command,cargo,timeout=180);probe('runtime-tracked-edit-is-dirty',cargo,exe,first,'dirty')
        changed.write_bytes(clean_bytes)
        execute('cargo-restored-clean-rebuild',command,cargo,timeout=180);probe('runtime-restored-is-clean',cargo,exe,first,'clean')
        with (cargo/'Cargo.toml').open('ab') as file:file.write(b'\n# Original second metadata checkpoint; product version unchanged.\n')
        local_git('cargo-second-add',cargo,'add','Cargo.toml');local_git('cargo-second-commit',cargo,'commit','-m','Original second controlled metadata checkpoint');second=local_git('cargo-second-head',cargo,'rev-parse','HEAD');check('fixture-second-commit-is-distinct',bool(re.fullmatch('[0-9a-f]{40}',second)) and second!=first)
        execute('cargo-second-commit-rebuild',command,cargo,timeout=180);probe('runtime-second-commit-is-current-clean',cargo,exe,second,'clean')
        # Packing removes the previously watched loose ref. The actual build
        # script must refresh dependencies once, then permit a cached build.
        local_git('cargo-pack-refs',cargo,'pack-refs','--all')
        loose=cargo/'.git/refs/heads/metadata-control';packed=cargo/'.git/packed-refs'
        check('packed-fixture-really-has-no-loose-branch',not loose.exists() and packed.is_file() and second in packed.read_text(encoding='utf-8'))
        execute('cargo-packed-refs-rebuild',command,cargo,timeout=180);probe('runtime-packed-refs-is-current-clean',cargo,exe,second,'clean')
        packed_stat=exe.stat();packed_sha=sha(exe)
        execute('cargo-packed-refs-unchanged-build',command,cargo,timeout=180);probe('runtime-packed-refs-unchanged-clean',cargo,exe,second,'clean')
        check('cargo-packed-refs-unchanged-build-preserves-executable',sha(exe)==packed_sha and exe.stat().st_mtime_ns==packed_stat.st_mtime_ns,{'before_sha256':packed_sha,'after_sha256':sha(exe),'before_mtime_ns':packed_stat.st_mtime_ns,'after_mtime_ns':exe.stat().st_mtime_ns})
        # This empty commit changes only the branch reference. Record HEAD and
        # index bytes/timestamps rather than pretending they stayed unchanged.
        def metadata_identity(path):return {'sha256':sha(path),'mtime_ns':path.stat().st_mtime_ns}
        unchanged_paths={name:cargo/'.git'/name for name in ('HEAD','index')}
        before_empty={name:metadata_identity(path) for name,path in unchanged_paths.items()}
        local_git('cargo-empty-third-commit',cargo,'commit','--allow-empty','-m','Original empty third controlled metadata checkpoint')
        third=local_git('cargo-third-head',cargo,'rev-parse','HEAD')
        after_empty={name:metadata_identity(path) for name,path in unchanged_paths.items()}
        result['cases']['packed-empty-commit-watchers']={'before_head_index':before_empty,'after_head_index':after_empty,'loose_branch_created':loose.is_file(),'packed_ref_sha256':sha(packed)};save()
        check('fixture-empty-third-commit-is-distinct',bool(re.fullmatch('[0-9a-f]{40}',third)) and third not in (first,second))
        # Normal git commit may refresh the index even for an empty commit.
        # Retain the observation without claiming isolated ref-watch coverage.
        result.setdefault('coverage_observations',{})['empty_commit_head_index_unchanged']={'observed':before_empty==after_empty,'isolated_reference_dependency_established':before_empty==after_empty,'before':before_empty,'after':after_empty};save()
        check('fixture-empty-commit-created-new-loose-ref',loose.is_file() and loose.read_text(encoding='utf-8').strip()==third)
        execute('cargo-empty-third-commit-rebuild',command,cargo,timeout=180);probe('runtime-empty-third-commit-is-current-clean',cargo,exe,third,'clean')
        # A genuine same-tree commit object plus expected-old reference update
        # changes the branch without normal commit's index-refresh behavior.
        # Measure both operations together; do not restore or forge timestamps.
        # Repack after the third rebuild, then really rebuild with that loose
        # dependency absent before creating the fourth reference. This exercises
        # directory creation watching instead of retaining an existing-file watch.
        local_git('cargo-pack-third-before-reference-creation',cargo,'pack-refs','--all')
        check('fixture-before-fourth-has-no-loose-branch',not loose.exists() and packed.is_file() and third in packed.read_text(encoding='utf-8'))
        execute('cargo-third-packed-reference-rebuild',command,cargo,timeout=180)
        probe('runtime-third-packed-reference-is-current-clean',cargo,exe,third,'clean')
        tree=local_git('cargo-third-tree',cargo,'rev-parse','HEAD^{tree}')
        if not re.fullmatch('[0-9a-f]{40}',tree):raise ValueError('Fixture Git did not return the exact tree identity.')
        before_ref_only={name:metadata_identity(path) for name,path in unchanged_paths.items()}
        fourth=local_git('cargo-isolated-fourth-commit-tree',cargo,'commit-tree',tree,'-p',third,'-m','Original fourth controlled isolated reference checkpoint')
        if not re.fullmatch('[0-9a-f]{40}',fourth):raise ValueError('Fixture Git did not return the exact fourth commit identity.')
        local_git('cargo-isolated-fourth-update-ref',cargo,'update-ref','refs/heads/metadata-control',fourth,third)
        after_ref_only={name:metadata_identity(path) for name,path in unchanged_paths.items()}
        actual_fourth=local_git('cargo-isolated-fourth-head',cargo,'rev-parse','HEAD')
        fourth_tree=local_git('cargo-isolated-fourth-tree',cargo,'rev-parse','HEAD^{tree}')
        result['cases']['isolated-reference-fourth-commit']={'before_head_index':before_ref_only,'after_head_index':after_ref_only,'expected_old':third,'new_commit':fourth,'old_tree':tree,'new_tree':fourth_tree,'actual_head':actual_fourth,'loose_branch_created':loose.is_file()};save()
        check('fixture-fourth-reference-commit-is-genuine-distinct-same-tree',fourth not in (first,second,third) and actual_fourth==fourth and fourth_tree==tree)
        check('fixture-isolated-reference-update-keeps-head-and-index-unchanged',before_ref_only==after_ref_only,{'before':before_ref_only,'after':after_ref_only})
        check('fixture-isolated-reference-update-creates-new-loose-ref',loose.is_file() and loose.read_text(encoding='utf-8').strip()==fourth)
        execute('cargo-isolated-fourth-reference-rebuild',command,cargo,timeout=180);probe('runtime-isolated-fourth-reference-is-current-clean',cargo,exe,fourth,'clean')
        probe('runtime-needs-no-git',cargo,exe,fourth,'clean',missing_git=True)
        result['fixture_commits']={'first':first,'second':second,'third_empty':third,'fourth_reference_only':fourth,'unrelated_parent':parent_sha}
        check('canonical-inputs-unchanged',all(sha(root/name)==value for name,value in inputs.items()))
        check('canonical-executable-unchanged',before_exe==(sha(root/'ChezCat.exe') if (root/'ChezCat.exe').is_file() else None))
        result['passed']=bool(result['checks']) and all(result['checks'].values());save()
    except Exception as error:
        result['error']=str(error);result['passed']=False;save();print('Metadata controls failed; complete local evidence: '+str(out/'verification.json'));return 1
    print(json.dumps({'passed':result['passed'],'checks':result['checks'],'verification':str(out/'verification.json'),'scope':'Actual build-script collector plus original real offline Cargo rebuild/runtime controls; no public/runtime browser/service action.'}))
    return 0 if result['passed'] else 1

if __name__=='__main__':raise SystemExit(main())
