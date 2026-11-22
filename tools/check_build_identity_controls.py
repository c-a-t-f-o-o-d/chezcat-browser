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
        'tools/check_about_harness.py',
        'LICENSE','THIRD_PARTY_NOTICES.md','assets/chezcat.ico','resources/chezcat.rc',
        'resources/chezcat.res','resources/chezcat-x64.o')
KEYS={'CHEZCAT_EMBEDDED_SOURCE_REVISION','CHEZCAT_EMBEDDED_SOURCE_ORIGIN',
      'CHEZCAT_EMBEDDED_TREE_STATE','CHEZCAT_EMBEDDED_DECLARED_TAG','CHEZCAT_EMBEDDED_MAIN_ORDINAL'}
PROBE='''#[allow(dead_code)] mod crypto_hash;
#[allow(dead_code)] mod license_acceptance;
#[allow(dead_code)] mod build_identity;
fn main(){print!("{}",build_identity::current().safe_copy());}
'''
IDENTITY={'GIT_AUTHOR_NAME':'Controlled metadata fixture','GIT_COMMITTER_NAME':'Controlled metadata fixture',
          'GIT_AUTHOR_EMAIL':'fixture@users.noreply.github.com',
          'GIT_COMMITTER_EMAIL':'fixture@users.noreply.github.com'}
CLEAR=('GIT_DIR','GIT_WORK_TREE','GIT_INDEX_FILE','GIT_OBJECT_DIRECTORY',
       'GIT_ALTERNATE_OBJECT_DIRECTORIES','GIT_COMMON_DIR','GIT_SHALLOW_FILE','GIT_NAMESPACE',
       'GIT_CONFIG_COUNT','CHEZCAT_DECLARED_BUILD_TAG',*KEYS)

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
    out=(args.output or capture_tests.verification_root(root)/'runs'/(datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-build-identity-controls-'+uuid.uuid4().hex[:8])).resolve()
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
    def script_case(name,path,script_bin,origin,tree,revision='',missing_git=False,ordinal='',extra_env=None):
        env={'CARGO_MANIFEST_DIR':str(path),'CARGO_PKG_VERSION':VERSION,'CARGO_CFG_TARGET_OS':'windows','CARGO_CFG_TARGET_ARCH':'x86_64','CARGO_CFG_TARGET_ENV':'msvc'}
        if missing_git:env['PATH']=str(empty_path)
        env.update(extra_env or {})
        data=emitted(execute(name,[script_bin],path,env));result['cases'][name]={'kind':'actual collector, not Cargo watcher proof','emitted':data,'missing_git_path':missing_git};save()
        expected={'CHEZCAT_EMBEDDED_SOURCE_REVISION':revision,'CHEZCAT_EMBEDDED_SOURCE_ORIGIN':origin,'CHEZCAT_EMBEDDED_TREE_STATE':tree,'CHEZCAT_EMBEDDED_DECLARED_TAG':'','CHEZCAT_EMBEDDED_MAIN_ORDINAL':ordinal}
        check(name,data==expected,{'expected':expected,'actual':data})
        return data
    def probe(name,path,exe,revision,tree,missing_git=False,ordinal=None):
        text=execute(name,[exe],path,{'PATH':str(empty_path)} if missing_git else {})
        expected_state={'clean':'Git working tree clean at build check','dirty':'Git working tree dirty at build check'}[tree]
        display=VERSION+('.' if '+' in VERSION else '+')+'dev.'+(str(ordinal)+'.' if ordinal is not None else '')+'g'+revision[:12]
        kind='Development build' if ordinal is not None else 'Development build (public main ordinal unavailable)'
        check(name,text.startswith(f'ChezCat {display}\n') and f'Source revision: {revision}\n' in text and f'Source state: {expected_state}\n' in text and f'Build: {kind}\n' in text,{'expected_revision':revision,'expected_tree':tree,'expected_ordinal':ordinal,'actual_native_probe':text})
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
        # Metadata-only changes must update an otherwise unchanged source S.
        # No remote is contacted: this original fixture assigns local public
        # tracking refs so the actual collector can prove its input rules.
        public_origin='https://github.com/c-a-t-f-o-o-d/chezcat-browser.git'
        local_git('cargo-public-origin',cargo,'remote','add','origin',public_origin)
        local_git('cargo-public-main-ref',cargo,'update-ref','refs/remotes/origin/main',fourth)
        execute('cargo-numbered-main-build',command,cargo,timeout=180);probe('runtime-numbered-main-source',cargo,exe,fourth,'clean',ordinal=4)
        probe('runtime-numbered-main-without-runtime-git',cargo,exe,fourth,'clean',missing_git=True,ordinal=4)
        local_git('cargo-main-ref-moves-before-source',cargo,'update-ref','refs/remotes/origin/main',second,fourth)
        execute('cargo-main-ref-only-rebuild',command,cargo,timeout=180);probe('runtime-source-not-on-main-falls-back',cargo,exe,fourth,'clean')
        local_git('cargo-main-ref-restored',cargo,'update-ref','refs/remotes/origin/main',fourth,second)
        execute('cargo-main-ref-restored-rebuild',command,cargo,timeout=180);probe('runtime-main-ref-restored',cargo,exe,fourth,'clean',ordinal=4)
        shallow=cargo/'.git/shallow';check('fixture-started-with-complete-history',not shallow.exists())
        write(shallow,fourth+'\n')
        execute('cargo-shallow-created-rebuild',command,cargo,timeout=180);probe('runtime-shallow-source-does-not-restart-numbering',cargo,exe,fourth,'clean')
        shallow.unlink()
        execute('cargo-shallow-removed-rebuild',command,cargo,timeout=180);probe('runtime-complete-history-restores-number',cargo,exe,fourth,'clean',ordinal=4)
        local_git('cargo-pack-public-main',cargo,'pack-refs','--all')
        main_loose=cargo/'.git/refs/remotes/origin/main'
        check('fixture-public-main-is-packed',not main_loose.exists() and f'{fourth} refs/remotes/origin/main' in packed.read_text(encoding='utf-8'))
        execute('cargo-packed-public-main-build',command,cargo,timeout=180);probe('runtime-packed-public-main',cargo,exe,fourth,'clean',ordinal=4)
        packed_bytes=packed.read_bytes();old_record=f'{fourth} refs/remotes/origin/main'.encode();new_record=f'{second} refs/remotes/origin/main'.encode()
        check('fixture-packed-main-record-is-unique',packed_bytes.count(old_record)==1)
        packed.write_bytes(packed_bytes.replace(old_record,new_record))
        execute('cargo-packed-main-only-rebuild',command,cargo,timeout=180);probe('runtime-packed-main-change-invalidates-number',cargo,exe,fourth,'clean')
        packed.write_bytes(packed_bytes)
        execute('cargo-packed-main-restored-rebuild',command,cargo,timeout=180);probe('runtime-packed-main-restores-number',cargo,exe,fourth,'clean',ordinal=4)
        numbered_stat=exe.stat();numbered_sha=sha(exe)
        execute('cargo-numbered-main-cached-build',command,cargo,timeout=180)
        check('cargo-numbered-metadata-does-not-force-rebuild',sha(exe)==numbered_sha and exe.stat().st_mtime_ns==numbered_stat.st_mtime_ns)

        graph=fixture('ordinal-graph');a=initialize(graph,'ordinal-graph')
        local_git('ordinal-public-origin',graph,'remote','add','origin',public_origin)
        local_git('ordinal-main-root',graph,'update-ref','refs/remotes/origin/main',a)
        script_case('collector-public-root-is-one',graph,script_bin,'git','clean',a,ordinal='1')
        local_git('ordinal-main-second',graph,'commit','--allow-empty','-m','Original public second checkpoint')
        b=local_git('ordinal-second-source',graph,'rev-parse','HEAD');local_git('ordinal-main-second-ref',graph,'update-ref','refs/remotes/origin/main',b,a)
        before_tip=script_case('collector-main-second-is-two',graph,script_bin,'git','clean',b,ordinal='2')
        local_git('ordinal-feature-branch',graph,'checkout','-b','original-feature')
        local_git('ordinal-feature-commit',graph,'commit','--allow-empty','-m','Original feature checkpoint')
        feature=local_git('ordinal-feature-source',graph,'rev-parse','HEAD')
        script_case('collector-unmerged-feature-has-no-main-ordinal',graph,script_bin,'git','clean',feature)
        script_case('collector-ambient-number-is-not-proof',graph,script_bin,'git','clean',feature,extra_env={'CHEZCAT_EMBEDDED_MAIN_ORDINAL':'999'})
        local_git('ordinal-return-main',graph,'checkout','metadata-control')
        local_git('ordinal-merge-feature',graph,'merge','--no-ff','original-feature','-m','Original public feature integration')
        merged=local_git('ordinal-merged-source',graph,'rev-parse','HEAD');local_git('ordinal-merged-main-ref',graph,'update-ref','refs/remotes/origin/main',merged,b)
        script_case('collector-main-merge-counts-one-first-parent-step',graph,script_bin,'git','clean',merged,ordinal='3')
        local_git('ordinal-checkout-merged-feature-head',graph,'checkout','original-feature')
        local_git('ordinal-feature-really-is-main-ancestor',graph,'merge-base','--is-ancestor',feature,merged)
        script_case('collector-merged-second-parent-is-not-main-path',graph,script_bin,'git','clean',feature)
        local_git('ordinal-detached-historical-main',graph,'checkout','--detach',b)
        after_tip=script_case('collector-historical-main-count-ends-at-source',graph,script_bin,'git','clean',b,ordinal='2')
        check('local-and-detached-source-count-agree-after-main-advances',before_tip==after_tip)
        script_case('collector-ambient-repository-cannot-supply-source-or-count',graph,script_bin,'git','clean',b,ordinal='2',
                    extra_env={'GIT_DIR':str(parent/'.git'),'GIT_WORK_TREE':str(graph),'GIT_COMMON_DIR':str(parent/'.git')})
        local_git('ordinal-hidden-assume-unchanged',graph,'update-index','--assume-unchanged','src/build_identity.rs')
        script_case('collector-hidden-state-has-no-number',graph,script_bin,'git','unknown',b)
        local_git('ordinal-clear-assume-unchanged',graph,'update-index','--no-assume-unchanged','src/build_identity.rs')
        local_git('ordinal-hidden-skip-worktree',graph,'update-index','--skip-worktree','src/build_identity.rs')
        script_case('collector-skip-worktree-has-no-number',graph,script_bin,'git','unknown',b)
        local_git('ordinal-clear-skip-worktree',graph,'update-index','--no-skip-worktree','src/build_identity.rs')
        write(graph/'original-untracked.txt','Original dirty-source ordinal control.\n')
        script_case('collector-dirty-main-has-no-number',graph,script_bin,'git','dirty',b)
        (graph/'original-untracked.txt').unlink()
        local_git('ordinal-other-origin',graph,'remote','set-url','origin','https://example.invalid/other-source.git')
        script_case('collector-other-origin-cannot-supply-public-number',graph,script_bin,'git','clean',b)
        local_git('ordinal-restore-public-origin',graph,'remote','set-url','origin',public_origin)
        local_git('ordinal-alternate-main-is-not-used',graph,'update-ref','refs/remotes/alternate/main',merged)
        local_git('ordinal-local-main-is-not-used',graph,'update-ref','refs/heads/main',merged)
        local_git('ordinal-remove-public-main',graph,'update-ref','-d','refs/remotes/origin/main',merged)
        script_case('collector-missing-public-ref-cannot-use-other-ancestry',graph,script_bin,'git','clean',b)
        local_git('ordinal-restore-public-main',graph,'update-ref','refs/remotes/origin/main',merged)
        write(graph/'.git/shallow',b+'\n')
        script_case('collector-shallow-history-never-starts-at-one',graph,script_bin,'git','clean',b)
        script_case('collector-ambient-shallow-path-cannot-hide-incomplete-history',graph,script_bin,'git','clean',b,
                    extra_env={'GIT_SHALLOW_FILE':str(out/'not-the-repository-shallow-file')})
        (graph/'.git/shallow').unlink()
        local_git('ordinal-local-replacement-parent',graph,'replace','--graft',b)
        script_case('collector-replacement-refs-do-not-rewrite-public-count',graph,script_bin,'git','clean',b,ordinal='2')
        local_git('ordinal-remove-local-replacement',graph,'replace','-d',b)
        write(graph/'.git/info/grafts',b+'\n')
        script_case('collector-graft-history-is-unverifiable',graph,script_bin,'git','clean',b)
        (graph/'.git/info/grafts').unlink()
        import check_about_harness as about_controls
        clean_state='Git working tree clean at build check';dirty_state='Git working tree dirty at build check'
        unavailable='Development build (public main ordinal unavailable)';license_hash=sha(root/'LICENSE')
        prefix=VERSION+('.' if '+' in VERSION else '+')+'dev.'
        def copy_payload(label,kind='Development build',revision=b,state=clean_state):
            return f'ChezCat {label}\nBuild: {kind}\nSource revision: {revision}\nSource state: {state}\nLicense: CSAPUL {accept_license.VERSION}\nLicense SHA-256: {license_hash}\n'.encode('ascii')
        accepted=about_controls.safe_identity(copy_payload(prefix+'2.g'+b[:12]),VERSION,accept_license.VERSION,license_hash,b,clean_state,2)
        check('about-checker-verifies-source-bound-number',accepted['development_ordinal']==2 and accepted['product_version']==VERSION)
        fallback=about_controls.safe_identity(copy_payload(prefix+'g'+feature[:12],unavailable,feature),VERSION,accept_license.VERSION,license_hash,feature,clean_state,None)
        check('about-checker-keeps-feature-sha-fallback',fallback['development_ordinal'] is None and fallback['source_revision']==feature)
        declared='Declared tagged build v'+VERSION+' (publication unverified)'
        tagged=about_controls.safe_identity(copy_payload(VERSION,declared),VERSION,accept_license.VERSION,license_hash,b,clean_state,2)
        check('about-checker-keeps-declared-cargo-version',tagged['display_version']==VERSION and tagged['development_ordinal'] is None)
        invalid_copies={
            'wrong-main-count':(copy_payload(prefix+'3.g'+b[:12]),clean_state),
            'main-tip-sha-instead-of-source':(copy_payload(prefix+'2.g'+merged[:12]),clean_state),
            'leading-zero-count':(copy_payload(prefix+'02.g'+b[:12]),clean_state),
            'zero-count':(copy_payload(prefix+'0.g'+b[:12]),clean_state),
            'overflow-count':(copy_payload(prefix+'18446744073709551616.g'+b[:12]),clean_state),
            'dirty-number':(copy_payload(prefix+'2.g'+b[:12],state=dirty_state),dirty_state),
            'lost-verifiable-number':(copy_payload(prefix+'g'+b[:12],unavailable),clean_state),
            'arbitrary-extra-field':(copy_payload(prefix+'2.g'+b[:12])+b'Unexpected field: refused\n',clean_state),
            'abbreviated-authoritative-source':(copy_payload(prefix+'2.g'+b[:12],revision=b[:12]),clean_state),
            'dirty-declared-tag':(copy_payload(VERSION,declared,state=dirty_state),dirty_state),
        }
        for label,(payload,state) in invalid_copies.items():
            refused=False
            try:about_controls.safe_identity(payload,VERSION,accept_license.VERSION,license_hash,b,state,2)
            except ValueError:refused=True
            check('about-checker-refuses-'+label,refused)
        result['fixture_commits']={'first':first,'second':second,'third_empty':third,'fourth_reference_only':fourth,'unrelated_parent':parent_sha,
                                  'public_graph_root':a,'public_graph_second':b,'feature_second_parent':feature,'public_graph_merge':merged}
        check('canonical-inputs-unchanged',all(sha(root/name)==value for name,value in inputs.items()))
        check('canonical-executable-unchanged',before_exe==(sha(root/'ChezCat.exe') if (root/'ChezCat.exe').is_file() else None))
        result['passed']=bool(result['checks']) and all(result['checks'].values());save()
    except Exception as error:
        result['error']=str(error);result['passed']=False;save();print('Metadata controls failed; complete local evidence: '+str(out/'verification.json'));return 1
    print(json.dumps({'passed':result['passed'],'checks':result['checks'],'verification':str(out/'verification.json'),'scope':'Actual build-script collector plus original real offline Cargo rebuild/runtime controls; no public/runtime browser/service action.'}))
    return 0 if result['passed'] else 1

if __name__=='__main__':raise SystemExit(main())
