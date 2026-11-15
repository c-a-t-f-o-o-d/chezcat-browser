"""Controlled packaging guards; no real license adoption, release, or network."""
import hashlib
import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
import zipfile
from unittest.mock import patch
import build_release_bundle as package
import check_distribution as distribution
import verify_offline as verification

class Guards(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='chezcat-package-guards-');self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name)
        self.patcher=patch.object(package,'ROOT',self.root);self.patcher.start();self.addCleanup(self.patcher.stop)
    def write(self,name,data):
        p=self.root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data)
    def test_no_operative_license_refuses_runtime(self):
        with self.assertRaisesRegex(ValueError,'No operative LICENSE'):package.runtime_files('0'*64,'v0.78.0')
    def test_draft_is_not_an_adoption(self):
        self.write('LICENSE',b'UNAPPROVED DRAFT controlled fixture, not proposed project terms')
        with self.assertRaisesRegex(ValueError,'Draft license'):package.runtime_files('0'*64,'v0.78.0')
    def test_exact_owner_approved_hash_is_required(self):
        self.write('LICENSE',b'Controlled fixture bytes; not a grant for ChezCat')
        with self.assertRaisesRegex(ValueError,'Approved license hash'):package.runtime_files('0'*64,'v0.78.0')
    def fixture(self,ignore_license=False,omit_scope=None,scope_digest=None,history=None):
        files={'Cargo.toml':b'[package]\nname="controlled-fixture"\nversion="0.78.0"\n',
          'LICENSE':b'Controlled fixture bytes; not a grant for ChezCat',
          '.gitignore':b'ChezCat.exe\nresults/\n**/__pycache__/\n'+(b'LICENSE\n' if ignore_license else b''),
          '.gitattributes':b'*.cmd text eol=crlf\nSOURCE_REVISION export-subst\n',
          'SOURCE_REVISION':b'$Format:%H$\n',
          'README.md':b'Controlled fixture', 'THIRD_PARTY_NOTICES.md':b'Controlled fixture',
          'assets/chezcat.ico':b'controlled', 'assets/chezcat-icon.png':b'controlled',
          'pages/start.html':b'<p>controlled</p>', 'trust/roots.pem':b'controlled',
          'notices/fixture.txt':b'controlled','data/public_suffix/fixture.txt':b'controlled'}
        for name in package.MANDATORY-{'LICENSE_SCOPE.md','licensing/COVERED_HISTORY.txt'}:
            files.setdefault(name,b'Controlled public runtime documentation fixture.\n')
        for name,data in files.items():self.write(name,data)
        subprocess.run(['git','-c','core.autocrlf=false','init','-q'],cwd=self.root,check=True)
        subprocess.run(['git','config','core.autocrlf','false'],cwd=self.root,check=True)
        subprocess.run(['git','add','.'],cwd=self.root,check=True)
        subprocess.run(['git','-c','user.name=Controlled test fixture','-c','user.email=fixture@example.test','-c','commit.gpgsign=false','commit','-qm','Controlled packaging fixture only'],cwd=self.root,check=True)
        base=package.git('rev-parse','HEAD').decode().strip()
        scope=('Exact LICENSE SHA-256: '+(scope_digest or hashlib.sha256(files['LICENSE']).hexdigest())+'\nSee licensing/COVERED_HISTORY.txt\n').encode()
        for name,data in {'LICENSE_SCOPE.md':scope,'licensing/COVERED_HISTORY.txt':((history if history is not None else base)+'\n').encode()}.items():
            if name!=omit_scope:files[name]=data;self.write(name,data)
        subprocess.run(['git','add','.'],cwd=self.root,check=True)
        subprocess.run(['git','-c','user.name=Controlled test fixture','-c','user.email=fixture@example.test','-c','commit.gpgsign=false','commit','-qm','Controlled finite license scope fixture only'],cwd=self.root,check=True)
        subprocess.run(['git','tag','v0.78.0'],cwd=self.root,check=True)
        self.write('ChezCat.exe',b'Controlled bytes; never treated as a verified PE by this test')
        self.write('notices/__pycache__/private.pyc',b'private trap fixture')
        self.write('results/verification/latest-offline.json',json.dumps({'report':'results/fixture.json','build_info':'results/build.json'}).encode())
        commit=package.git('rev-parse','HEAD').decode().strip();source=package.capture_tests.source_identity(self.root);exe=hashlib.sha256((self.root/'ChezCat.exe').read_bytes()).hexdigest()
        self.write('results/fixture.json',json.dumps({'passed':True,'source_commit':commit,'executable_sha256':exe,'source_before':source,'source_after':source}).encode())
        self.write('results/build.json',json.dumps({'dirty':False,'source_commit':commit,'source_identity':source,'executable_sha256':exe,'compiler':'Controlled fixture, not a build certification'}).encode())
        self.assertEqual(package.git('status','--porcelain'),b'')
        return files,commit
    def test_runtime_selection_cannot_include_ignored_private_files(self):
        files,_=self.fixture()
        selected=package.runtime_files(hashlib.sha256(files['LICENSE']).hexdigest(),'v0.78.0')
        names={p.relative_to(self.root).as_posix() for p in selected}
        self.assertIn('pages/start.html',names);self.assertIn('ChezCat.exe',names)
        self.assertNotIn('notices/__pycache__/private.pyc',names)
        self.assertFalse(any(n.startswith('results/') for n in names))
        self.assertTrue(package.MANDATORY.issubset(names))
        self.assertIn('Run.cmd',names);self.assertIn('docs/BUILDING.md',names)
        self.assertIn('licensing/CONTRIBUTOR_AGREEMENT.md',names)
    def test_missing_scope_or_history_is_rejected(self):
        # Separate fixtures keep the committed absence meaningful.
        for name in ['LICENSE_SCOPE.md','licensing/COVERED_HISTORY.txt']:
            with self.subTest(missing=name),tempfile.TemporaryDirectory(prefix='chezcat-scope-guard-') as folder:
                previous=self.root;self.root=Path(folder)
                try:
                    with patch.object(package,'ROOT',self.root):
                        files,_=self.fixture(omit_scope=name)
                        with self.assertRaisesRegex(ValueError,'scope and covered history'):
                            package.runtime_files(hashlib.sha256(files['LICENSE']).hexdigest(),'v0.78.0')
                finally:self.root=previous
    def test_scope_digest_must_match_exact_approved_license(self):
        files,_=self.fixture(scope_digest='0'*64)
        with self.assertRaisesRegex(ValueError,'exact approved LICENSE digest'):
            package.runtime_files(hashlib.sha256(files['LICENSE']).hexdigest(),'v0.78.0')
    def test_history_revision_must_be_reachable(self):
        files,_=self.fixture(history='a'*40)
        with self.assertRaisesRegex(ValueError,'outside the selected source ancestry'):
            package.runtime_files(hashlib.sha256(files['LICENSE']).hexdigest(),'v0.78.0')
    def test_history_requires_finite_exact_unique_commit_lines(self):
        _,commit=self.fixture();entries=package.tree(commit)
        original=package.blob
        for payload in [b'',b'not-a-commit\n',(commit+'\n'+commit+'\n').encode()]:
            with self.subTest(payload_bytes=len(payload)),patch.object(package,'blob',side_effect=lambda oid:payload if oid==entries['licensing/COVERED_HISTORY.txt'][2] else original(oid)):
                with self.assertRaisesRegex(ValueError,'finite|duplicate'):
                    package.approve_scope(entries,hashlib.sha256((self.root/'LICENSE').read_bytes()).hexdigest(),commit)
    def archives(self,commit):
        # Deliberately non-executable fixture bytes: static archive checks only.
        entries=package.tree(commit);payload={name:package.blob(entries[name][2]) for name in package.runtime_names(entries)}
        payload['ChezCat.exe']=(self.root/'ChezCat.exe').read_bytes()
        identity={'source_commit':commit,'clean':True,'tag':'v0.78.0',
          'license_sha256':hashlib.sha256(payload['LICENSE']).hexdigest(),
          'compiler':'Controlled fixture, not a build certification','self_hash_excluded':True,
          'files':{name:hashlib.sha256(data).hexdigest() for name,data in payload.items()}}
        payload['build-info.json']=json.dumps(identity).encode()
        folder=self.root/'results/archive-guards';folder.mkdir(parents=True)
        runtime=folder/'runtime.zip';source=folder/'source.zip'
        self.archive(runtime,'ChezCat',payload)
        self.archive(source,'ChezCat-source',distribution.source_export(entries,commit))
        return runtime,source,payload
    def archive(self,path,prefix,payload):
        with zipfile.ZipFile(path,'x',compression=zipfile.ZIP_STORED) as archive:
            for name,data in payload.items():archive.writestr(prefix+'/'+name,data)
    def test_distribution_static_checks_cover_complete_payload_without_running_fake_exe(self):
        files,commit=self.fixture();runtime,source,_=self.archives(commit)
        payload,checks=distribution.validate_archives(runtime,source,'v0.78.0',commit,hashlib.sha256(files['LICENSE']).hexdigest())
        self.assertTrue(all(checks.values()));self.assertTrue(package.MANDATORY.issubset(payload))
        self.assertEqual(payload['LICENSE'],files['LICENSE'])
        self.assertNotIn('tools/check_input_harness.py',payload);self.assertFalse(any(n.startswith('src/') for n in payload))
    def test_distribution_rejects_extra_payload_and_changed_executable(self):
        files,commit=self.fixture();runtime,source,payload=self.archives(commit);approved=hashlib.sha256(files['LICENSE']).hexdigest()
        extra=runtime.with_name('extra.zip');self.archive(extra,'ChezCat',{**payload,'private-extra.txt':b'controlled trap'})
        with self.assertRaisesRegex(ValueError,'missing or extra'):
            distribution.validate_archives(extra,source,'v0.78.0',commit,approved)
        changed=runtime.with_name('changed.zip');self.archive(changed,'ChezCat',{**payload,'ChezCat.exe':b'changed controlled trap'})
        with self.assertRaisesRegex(ValueError,'manifest'):
            distribution.validate_archives(changed,source,'v0.78.0',commit,approved)
    def test_distribution_rejects_traversal_duplicate_names_and_crc_corruption(self):
        for name in ['ChezCat/../escape','ChezCat/ads:stream','ChezCat/NUL.txt']:
            with self.subTest(path=name),tempfile.NamedTemporaryFile(dir=self.root,suffix='.zip',delete=False) as stream:
                path=Path(stream.name)
            with zipfile.ZipFile(path,'w') as archive:archive.writestr(name,b'controlled')
            with self.assertRaises(ValueError):distribution.archive_payload(path,'ChezCat')
        backslash=self.root/'backslash.zip';self.archive(backslash,'ChezCat',{'back/slash':b'controlled'})
        backslash.write_bytes(backslash.read_bytes().replace(b'ChezCat/back/slash',b'ChezCat/back\\slash'))
        with self.assertRaisesRegex(ValueError,'Unsafe'):distribution.archive_payload(backslash,'ChezCat')
        duplicate=self.root/'duplicate.zip'
        with zipfile.ZipFile(duplicate,'w') as archive:
            archive.writestr('ChezCat/A.txt',b'a');archive.writestr('ChezCat/a.txt',b'b')
        with self.assertRaisesRegex(ValueError,'case-colliding'):distribution.archive_payload(duplicate,'ChezCat')
        corrupt=self.root/'corrupt.zip';self.archive(corrupt,'ChezCat',{'a.txt':b'payload'})
        data=bytearray(corrupt.read_bytes());data[data.index(b'payload')]^=1;corrupt.write_bytes(data)
        with self.assertRaises(zipfile.BadZipFile):distribution.archive_payload(corrupt,'ChezCat')
    def test_ignored_license_cannot_authorize_distribution(self):
        files,_=self.fixture(ignore_license=True)
        with self.assertRaisesRegex(ValueError,'tracked regular file'):
            package.runtime_files(hashlib.sha256(files['LICENSE']).hexdigest(),'v0.78.0')
    def test_dirty_build_report_is_rejected_even_after_source_reverted(self):
        _,commit=self.fixture()
        path=self.root/'results/build.json';info=json.loads(path.read_text());info['dirty']=True
        path.write_text(json.dumps(info))
        with self.assertRaisesRegex(ValueError,'clean exact build identity'):package.verified(commit)
    def test_stale_source_map_cannot_authorize_current_executable(self):
        _,commit=self.fixture()
        path=self.root/'results/fixture.json';report=json.loads(path.read_text());report['source_before']['Cargo.toml']='0'*64
        path.write_text(json.dumps(report))
        with self.assertRaisesRegex(ValueError,'current passing verification source identity'):package.verified(commit)
    def test_new_refused_run_invalidates_old_passing_pointer(self):
        self.fixture()
        for name in ['RUST_TEST_THREADS','RUST_MIN_STACK']:
            with self.subTest(override=name),patch.object(verification,'ROOT',self.root),patch.dict(os.environ,{name:'1'}),contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(verification.main(),1)
            pointer=json.loads((self.root/'results/verification/latest-offline.json').read_text())
            report=json.loads((self.root/pointer['report']).read_text())
            self.assertFalse(report['passed']);self.assertEqual(report['state'],'refused')
            self.assertIn(name,report['override_names'])
    def test_source_archive_does_not_inherit_parent_git_identity(self):
        self.fixture();child=self.root/'unpacked-source';child.mkdir()
        (child/'SOURCE_REVISION').write_text('a'*40+'\n')
        with patch.object(verification,'ROOT',child):self.assertEqual(verification.git_state(),('a'*40,None))
        with patch.object(package,'ROOT',child),self.assertRaisesRegex(ValueError,'exact Git root'):package.git('rev-parse','HEAD')
    def test_hidden_index_edit_cannot_claim_committed_source(self):
        _,commit=self.fixture()
        subprocess.run(['git','update-index','--assume-unchanged','Cargo.toml'],cwd=self.root,check=True)
        self.write('Cargo.toml',(self.root/'Cargo.toml').read_bytes()+b'# hidden edit\n')
        self.assertEqual(package.git('status','--porcelain'),b'')
        with self.assertRaisesRegex(ValueError,'differ from committed bytes'):package.verified(commit)
    def test_ignored_compilation_input_cannot_claim_committed_source(self):
        _,commit=self.fixture();self.write('pages/__pycache__/untracked.pyc',b'controlled ignored input')
        self.assertEqual(package.git('status','--porcelain'),b'')
        with self.assertRaisesRegex(ValueError,'differ from committed bytes'):package.verified(commit)

if __name__=='__main__':unittest.main(verbosity=2)
