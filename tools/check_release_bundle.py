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
        privacy=patch.object(package.publication,'ROOT',self.root);privacy.start();self.addCleanup(privacy.stop)
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
    def fixture(self,ignore_license=False,omit_scope=False,scope_digest=None,scope_text=None,legacy_history=False,private_path=None,unsafe_owner=False,extra_files=None):
        files={'Cargo.toml':b'[package]\nname="controlled-fixture"\nversion="0.78.0"\n',
          'LICENSE':b'Controlled fixture bytes; not a grant for ChezCat',
          '.gitignore':b'ChezCat.exe\nresults/\n**/__pycache__/\n'+(b'LICENSE\n' if ignore_license else b''),
          '.gitattributes':b'*.cmd text eol=crlf\nSOURCE_REVISION export-subst\n',
          'SOURCE_REVISION':b'$Format:%H$\n',
          'README.md':b'Controlled fixture', 'THIRD_PARTY_NOTICES.md':b'Controlled fixture',
          'assets/chezcat.ico':b'controlled', 'assets/chezcat-icon.png':b'controlled',
          'pages/start.html':b'<p>controlled</p>', 'trust/roots.pem':b'controlled',
          'notices/fixture.txt':b'controlled','data/public_suffix/fixture.txt':b'controlled'}
        for name in package.MANDATORY-{'LICENSE_SCOPE.md'}:
            files.setdefault(name,b'Controlled public runtime documentation fixture.\n')
        if not omit_scope:
            files['LICENSE_SCOPE.md']=scope_text if scope_text is not None else (
                'Scope: containing-source-revision\nLicense-SHA256: '+(scope_digest or hashlib.sha256(files['LICENSE']).hexdigest())+'\n').encode()
        if legacy_history:files['licensing/COVERED_HISTORY.txt']=b'Obsolete controlled history trap.\n'
        if private_path:files[private_path]=b'Controlled private payload trap, not private data.\n'
        files.update(extra_files or {})
        for name,data in files.items():self.write(name,data)
        subprocess.run(['git','-c','core.autocrlf=false','init','-q'],cwd=self.root,check=True)
        subprocess.run(['git','config','core.autocrlf','false'],cwd=self.root,check=True)
        subprocess.run(['git','add','.'],cwd=self.root,check=True)
        fixture_name='Adam Maddox' if unsafe_owner else 'Controlled test fixture'
        fixture_env=dict(os.environ,GIT_AUTHOR_NAME=fixture_name,GIT_COMMITTER_NAME=fixture_name,
            GIT_AUTHOR_EMAIL='fixture@example.test',GIT_COMMITTER_EMAIL='fixture@example.test')
        fixture_env.pop('GIT_AUTHOR_DATE',None);fixture_env.pop('GIT_COMMITTER_DATE',None)
        subprocess.run(['git','-c','commit.gpgsign=false','commit','-qm','Controlled parentless snapshot fixture only'],cwd=self.root,env=fixture_env,check=True)
        actual=subprocess.check_output(['git','show','-s','--format=%an%x09%ae%x09%cn%x09%ce','HEAD'],cwd=self.root,text=True).strip()
        self.assertEqual(actual,'\t'.join([fixture_name,'fixture@example.test',fixture_name,'fixture@example.test']))
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
    def test_new_license_policies_are_mandatory_runtime_payloads(self):
        _,commit=self.fixture();entries=package.tree(commit)
        for name in ['docs/LICENSE_ACCEPTANCE.md','docs/CONTRIBUTOR_PRIVACY.md','licensing/CHANGES-1.1.md']:
            with self.subTest(missing=name):
                incomplete=dict(entries);del incomplete[name]
                with self.assertRaisesRegex(ValueError,'Mandatory tracked runtime inputs'):
                    package.runtime_names(incomplete)
    def test_mozilla_trust_provenance_and_notices_are_mandatory(self):
        _,commit=self.fixture();entries=package.tree(commit)
        for name in ['trust/roots.pem','trust/mozilla/certdata.txt','trust/mozilla/LICENSE-MPL-2.0.txt',
                     'trust/mozilla/PROVENANCE.json','trust/mozilla/policy.json','trust/mozilla/selection.json']:
            with self.subTest(missing=name):
                incomplete=dict(entries);del incomplete[name]
                with self.assertRaisesRegex(ValueError,'Mandatory tracked runtime inputs'):
                    package.runtime_names(incomplete)

    def test_missing_containing_scope_is_rejected(self):
        files,_=self.fixture(omit_scope=True)
        with self.assertRaisesRegex(ValueError,'containing-revision license scope'):
            package.runtime_files(hashlib.sha256(files['LICENSE']).hexdigest(),'v0.78.0')
    def test_scope_digest_must_match_exact_approved_license(self):
        files,_=self.fixture(scope_digest='0'*64)
        with self.assertRaisesRegex(ValueError,'exact approved LICENSE digest'):
            package.runtime_files(hashlib.sha256(files['LICENSE']).hexdigest(),'v0.78.0')
    def test_retired_history_file_cannot_reenter_snapshot(self):
        files,_=self.fixture(legacy_history=True)
        with self.assertRaisesRegex(ValueError,'covered-history|publication|forbidden|retired|Private working material'):
            package.runtime_files(hashlib.sha256(files['LICENSE']).hexdigest(),'v0.78.0')
    def test_scope_requires_unique_complete_designation_and_no_stale_source_id(self):
        digest=hashlib.sha256(b'Controlled fixture bytes; not a grant for ChezCat').hexdigest()
        valid='Scope: containing-source-revision\nLicense-SHA256: '+digest+'\n'
        cases=[('missing',('License-SHA256: '+digest+'\n').encode()),
          ('ancestry',valid.replace('containing-source-revision','source-ancestry').encode()),
          ('duplicate-scope',(valid+'Scope: containing-source-revision\n').encode()),
          ('duplicate-license',(valid+'License-SHA256: '+digest+'\n').encode()),
          ('malformed',valid.replace('Scope:',' Scope :').encode()),
          ('stale-source',(valid+'Old containing commit: '+'a'*40+'\n').encode()),
          ('uppercase-stale-source',(valid+'Old containing commit: '+'A'*40+'\n').encode())]
        for label,payload in cases:
            with self.subTest(case=label),tempfile.TemporaryDirectory(prefix='chezcat-snapshot-scope-') as folder:
                previous=self.root;self.root=Path(folder)
                try:
                    with patch.object(package,'ROOT',self.root),patch.object(package.publication,'ROOT',self.root):
                        self.fixture(scope_text=payload)
                        with self.assertRaisesRegex(ValueError,'scope|Scope'):
                            package.runtime_files(digest,'v0.78.0')
                finally:self.root=previous
    def test_actual_parentless_packager_emits_exact_source_and_runtime(self):
        files,commit=self.fixture();approved=hashlib.sha256(files['LICENSE']).hexdigest()
        self.assertEqual(package.git('rev-list','HEAD').decode().splitlines(),[commit])
        self.assertEqual(package.git('rev-list','--parents','-n','1','HEAD').decode().split(),[commit])
        folder=self.root/'results/actual-parentless';folder.mkdir(parents=True)
        runtime=folder/'runtime.zip';source=folder/'source.zip'
        with contextlib.redirect_stdout(io.StringIO()):
            for path,extra in [(runtime,[]),(source,['--source'])]:
                with patch('sys.argv',['build_release_bundle.py',*extra,'--output',str(path),'--tag','v0.78.0','--approved-license-sha256',approved]):
                    self.assertEqual(package.main(),0)
                self.assertEqual(path.with_suffix('.zip.sha256').read_text().split()[0],hashlib.sha256(path.read_bytes()).hexdigest())
        payload,checks=distribution.validate_archives(runtime,source,'v0.78.0',commit,approved)
        self.assertTrue(all(checks.values()))
        self.assertEqual(payload['LICENSE_SCOPE.md'],files['LICENSE_SCOPE.md'])
        self.assertNotIn('licensing/COVERED_HISTORY.txt',payload)
        source_payload=distribution.archive_payload(source,'ChezCat-source')
        self.assertEqual(source_payload['SOURCE_REVISION'].strip(),commit.encode())
        self.assertEqual(source_payload['LICENSE'],files['LICENSE'])
        self.assertNotIn('ChezCat.exe',source_payload)
        self.assertNotIn('notices/__pycache__/private.pyc',source_payload)
        self.assertEqual(package.git('status','--porcelain'),b'')
    def test_private_tracked_report_is_refused_before_packaging(self):
        files,_=self.fixture(private_path='docs/SESSION.md')
        with self.assertRaisesRegex(ValueError,'Private working material'):
            package.runtime_files(hashlib.sha256(files['LICENSE']).hexdigest(),'v0.78.0')
    def test_tracked_acceptance_receipt_is_refused_before_runtime_and_source_zip(self):
        receipt=json.dumps({'schema_version':1,'license_version':'1.1','license_sha256':'0'*64,'accepted_scope':'personal','accepted_at_utc':100}).encode()
        annotated=json.dumps({**json.loads(receipt),'context':'Controlled additional-context trap'}).encode()
        for name,payload in [('docs/local-state.json',receipt),('assets/local-state.dat',annotated)]:
            with self.subTest(path=name),tempfile.TemporaryDirectory(prefix='chezcat-private-receipt-') as folder:
                previous=self.root;self.root=Path(folder)
                try:
                    with patch.object(package,'ROOT',self.root),patch.object(package.publication,'ROOT',self.root):
                        files,commit=self.fixture(extra_files={name:payload})
                        with self.assertRaisesRegex(ValueError,'Private license acceptance receipt'):
                            package.publication.check_receipts(commit)
                        for label,args in [('runtime',[]),('source',['--source']),('review-source',['--review-source'])]:
                            output=self.root/'results'/('controlled-review-'+label+'.zip')
                            argv=['build_release_bundle.py',*args,'--output',str(output),'--tag','v0.78.0','--approved-license-sha256',hashlib.sha256(files['LICENSE']).hexdigest()]
                            with patch('sys.argv',argv),self.assertRaisesRegex(ValueError,'Private license acceptance receipt'):
                                package.main()
                            self.assertFalse(output.exists())
                            self.assertFalse(output.with_suffix('.zip.sha256').exists())
                finally:self.root=previous
    def test_original_reference_json_and_generated_receipt_source_remain_packagable(self):
        reference=json.dumps({'schema_version':1,'kind':'original-reference','license_sha256':'example','accepted_at_utc':0}).encode()
        generated=b'def make_controlled_fixture():\n    return {"schema_version":1,"license_version":"1.1","license_sha256":"controlled","accepted_scope":"personal","accepted_at_utc":100}\n'
        files,commit=self.fixture(extra_files={'docs/reference.json':reference,'assets/receipt.json':b'{"kind":"original-fixture","pixels":[1,2,3]}','tools/receipt_fixture.py':generated})
        self.assertTrue(package.publication.check_receipts(commit))
        for label,args,prefix in [('runtime',[],'ChezCat'),('source',['--source'],'ChezCat-source')]:
            output=self.root/'results'/('public-reference-'+label+'.zip')
            argv=['build_release_bundle.py',*args,'--output',str(output),'--tag','v0.78.0','--approved-license-sha256',hashlib.sha256(files['LICENSE']).hexdigest()]
            with patch('sys.argv',argv),contextlib.redirect_stdout(io.StringIO()):self.assertEqual(package.main(),0)
            with zipfile.ZipFile(output) as archive:
                self.assertEqual(archive.read(prefix+'/docs/reference.json'),reference)
                if label=='source':
                    self.assertEqual(archive.read(prefix+'/assets/receipt.json'),files['assets/receipt.json'])
                    self.assertEqual(archive.read(prefix+'/tools/receipt_fixture.py'),generated)
    def test_receipt_content_detection_handles_encoding_and_stale_values(self):
        value={'schema_version':0,'license_version':'1.0','license_sha256':'stale','accepted_scope':'unknown','accepted_at_utc':None}
        for data in [json.dumps(value).encode(),json.dumps(value).encode('utf-16'),json.dumps(value).replace('schema_version','\\u0073chema_version').encode(),json.dumps({**value,'context':'Controlled additional field'}).encode()]:
            with self.subTest(payload=data[:20]),self.assertRaisesRegex(ValueError,'Private license acceptance receipt'):
                package.publication.check_receipt_payload(data)
        self.assertTrue(package.publication.check_receipt_payload(b'{"receipt":"ordinary original reference"}'))
        self.assertTrue(package.publication.check_receipt_payload(b'const RECEIPT_FIELDS = ["schema_version", "license_version", "license_sha256", "accepted_scope", "accepted_at_utc"];'))
    def test_owner_metadata_without_verified_noreply_is_refused(self):
        with patch.dict(os.environ,{'GIT_AUTHOR_EMAIL':package.publication.OWNER_EMAIL,'GIT_COMMITTER_EMAIL':package.publication.OWNER_EMAIL}):
            files,_=self.fixture(unsafe_owner=True)
        with self.assertRaisesRegex(ValueError,'verified GitHub noreply'):
            package.runtime_files(hashlib.sha256(files['LICENSE']).hexdigest(),'v0.78.0')
    def test_tag_version_and_stale_verification_commit_are_rejected(self):
        files,_=self.fixture();approved=hashlib.sha256(files['LICENSE']).hexdigest()
        with self.assertRaisesRegex(ValueError,'Tag must match exact Cargo version'):
            package.runtime_files(approved,'v0.78.1')
        path=self.root/'results/fixture.json';report=json.loads(path.read_text());report['source_commit']='0'*40
        path.write_text(json.dumps(report))
        with self.assertRaisesRegex(ValueError,'current passing verification source identity'):
            package.runtime_files(approved,'v0.78.0')
    def archives(self,commit):
        # Deliberately non-executable fixture bytes: static archive checks only.
        entries=package.tree(commit);payload={name:package.blob(entries[name][2]) for name in package.runtime_names(entries)}
        payload['ChezCat.exe']=(self.root/'ChezCat.exe').read_bytes()
        identity={'source_commit':commit,'clean':True,'tag':'v0.78.0',
          'license_sha256':hashlib.sha256(payload['LICENSE']).hexdigest(),'license_scope':'containing-source-revision',
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
    def test_distribution_without_current_acceptance_never_starts_extracted_runtime(self):
        result=subprocess.CompletedProcess([],1,stdout=b'',stderr=b'Controlled missing-receipt fixture')
        with patch.object(distribution.subprocess,'run',return_value=result) as check, patch.object(distribution.capture_tests,'execute') as native:
            with self.assertRaisesRegex(ValueError,'explicitly accepted current-license receipt'):
                distribution.exercise_runtime(self.root,self.root)
            native.assert_not_called()
            self.assertEqual(check.call_args.args[0][-1],'--check')
            self.assertNotIn('--accept-license',check.call_args.args[0])
            self.assertEqual(check.call_args.kwargs['timeout'],30)
    def test_distribution_missing_acceptance_never_extracts_runtime(self):
        runtime=self.root/'runtime.zip';source=self.root/'source.zip';output=self.root/'report.json'
        runtime.write_bytes(b'controlled archive input');source.write_bytes(b'controlled source input')
        with patch.object(distribution,'validate_archives',return_value=({'LICENSE_SCOPE.md':b'controlled scope'},{})), patch.object(distribution,'require_license_acceptance',side_effect=ValueError('Controlled missing acceptance')), patch.object(distribution,'extract_runtime') as extract, patch.object(distribution,'exercise_runtime') as exercise, contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(distribution.main(['--runtime',str(runtime),'--source',str(source),'--output',str(output),'--tag','v0.78.0','--commit','0'*40,'--license-sha256','0'*64]),1)
        extract.assert_not_called();exercise.assert_not_called()
        report=json.loads(output.read_text());self.assertFalse(report['passed'])
        self.assertEqual(report['error'],'Controlled missing acceptance')
        self.assertNotIn('evidence',report)
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
        extra_info=runtime.with_name('extra-info.zip');identity=json.loads(payload['build-info.json']);identity['private-extra']='controlled trap'
        self.archive(extra_info,'ChezCat',{**payload,'build-info.json':json.dumps(identity).encode()})
        with self.assertRaisesRegex(ValueError,'missing or extra fields'):
            distribution.validate_archives(extra_info,source,'v0.78.0',commit,approved)
        duplicate_info=runtime.with_name('duplicate-info.zip')
        data=b'{"source_commit":"wrong",'+payload['build-info.json'][1:]
        self.archive(duplicate_info,'ChezCat',{**payload,'build-info.json':data})
        with self.assertRaisesRegex(ValueError,'Duplicate runtime build-info JSON key'):
            distribution.validate_archives(duplicate_info,source,'v0.78.0',commit,approved)
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
    def test_source_payload_and_runtime_snapshot_identity_tampering_are_rejected(self):
        files,commit=self.fixture();runtime,source,payload=self.archives(commit);approved=hashlib.sha256(files['LICENSE']).hexdigest()
        source_payload=distribution.archive_payload(source,'ChezCat-source')
        changed=source.with_name('unintended-source.zip')
        self.archive(changed,'ChezCat-source',{**source_payload,'unintended.txt':b'Controlled extra source payload.'})
        with self.assertRaisesRegex(ValueError,'Source archive paths or payload bytes'):
            distribution.validate_archives(runtime,changed,'v0.78.0',commit,approved)
        for key,value in [('source_commit','0'*40),('license_scope','source-ancestry')]:
            with self.subTest(field=key):
                identity=json.loads(payload['build-info.json']);identity[key]=value
                changed=runtime.with_name(key+'.zip');self.archive(changed,'ChezCat',{**payload,'build-info.json':json.dumps(identity).encode()})
                with self.assertRaisesRegex(ValueError,'Runtime build identity disagrees'):
                    distribution.validate_archives(changed,source,'v0.78.0',commit,approved)
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
