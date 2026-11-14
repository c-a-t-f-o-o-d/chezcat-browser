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
from unittest.mock import patch
import build_release_bundle as package
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
    def fixture(self,ignore_license=False):
        files={'Cargo.toml':b'[package]\nname="controlled-fixture"\nversion="0.78.0"\n',
          'LICENSE':b'Controlled fixture bytes; not a grant for ChezCat',
          '.gitignore':b'ChezCat.exe\nresults/\n**/__pycache__/\n'+(b'LICENSE\n' if ignore_license else b''),
          'README.md':b'Controlled fixture', 'THIRD_PARTY_NOTICES.md':b'Controlled fixture',
          'assets/chezcat.ico':b'controlled', 'assets/chezcat-icon.png':b'controlled',
          'pages/start.html':b'<p>controlled</p>', 'trust/roots.pem':b'controlled',
          'notices/fixture.txt':b'controlled','data/public_suffix/fixture.txt':b'controlled'}
        for name,data in files.items():self.write(name,data)
        subprocess.run(['git','-c','core.autocrlf=false','init','-q'],cwd=self.root,check=True)
        subprocess.run(['git','config','core.autocrlf','false'],cwd=self.root,check=True)
        subprocess.run(['git','add','.'],cwd=self.root,check=True)
        subprocess.run(['git','-c','user.name=Controlled test fixture','-c','user.email=fixture@example.test','-c','commit.gpgsign=false','commit','-qm','Controlled packaging fixture only'],cwd=self.root,check=True)
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
        with patch.object(verification,'ROOT',self.root),patch.dict(os.environ,{'RUST_TEST_THREADS':'1'}),contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(verification.main(),1)
        pointer=json.loads((self.root/'results/verification/latest-offline.json').read_text())
        report=json.loads((self.root/pointer['report']).read_text())
        self.assertFalse(report['passed']);self.assertEqual(report['state'],'refused')
        self.assertIn('RUST_TEST_THREADS',report['override_names'])
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
