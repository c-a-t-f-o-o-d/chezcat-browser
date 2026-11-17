"""Original public-summary privacy/type controls; no Rust/browser/network execution."""
from contextlib import redirect_stdout
from datetime import datetime, timedelta, timezone
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import capture_tests
import public_ci_summary as public


class Controls(unittest.TestCase):

    def setUp(self):
        selection = patch.dict(os.environ)
        selection.start();self.addCleanup(selection.stop)
        os.environ.pop('CHEZCAT_VERIFICATION_ROOT', None)
        os.environ.pop('CARGO_TARGET_DIR', None)

    def test_external_report_uses_source_git_and_never_projects_private_locations(self):
        with tempfile.TemporaryDirectory(prefix='owned-summary-routing-') as temporary:
            base = Path(temporary);root = base / 'source';external = base / 'private verification'
            pointer = root / 'results/verification/latest-offline.json';pointer.parent.mkdir(parents=True)
            name = '20261115T000000.000000Z-23456789'
            report = external / 'offline' / name / 'verification.json';report.parent.mkdir(parents=True)
            report.write_text(json.dumps({'passed': False, 'source_commit': '1' * 40,
                'private_path': str(external), 'runs': [{'name': 'normal', 'state': 'failed', 'command': [str(external)]}]}))
            pointer.write_text(json.dumps({'report': 'verification:offline/' + name + '/verification.json'}))
            with patch.dict(os.environ, {'CHEZCAT_VERIFICATION_ROOT': str(external)}), patch.object(public.subprocess, 'check_output', return_value=b'1' * 40 + b'\n') as source_git, patch.object(public, 'public_basenames', return_value=set()):
                value = public.summary(root)
            self.assertEqual(value['stages'], [{'name': 'normal', 'state': 'failed'}])
            self.assertEqual(source_git.call_args.kwargs['cwd'], root)
            self.assertFalse(value['passed'])
            self.assertNotIn(str(external), json.dumps(value))
            self.assertEqual(value['native_tests']['state'], 'unavailable')
            # A malformed/missing external reference cannot fall back to a legacy report.
            legacy = root / 'results/verification/offline' / name / 'verification.json';legacy.parent.mkdir(parents=True)
            legacy.write_text(json.dumps({'passed': True, 'source_commit': '1' * 40, 'runs': [{'name': 'pe', 'state': 'completed'}]}))
            for reference in ('verification:../offline/' + name + '/verification.json', str(legacy), 'verification:offline/20000101T000000.000000Z-00000000/verification.json'):
                pointer.write_text(json.dumps({'report': reference}))
                with patch.dict(os.environ, {'CHEZCAT_VERIFICATION_ROOT': str(external)}), patch.object(public.subprocess, 'check_output', return_value=b'1' * 40 + b'\n'):
                    self.assertEqual(public.summary(root)['stages'], [])

    def test_offline_failure_and_refusal_reports_bind_commit_before_commands(self):
        import verify_offline as verification
        for dirty in [False, True]:
            with self.subTest(dirty=dirty), tempfile.TemporaryDirectory(prefix='public-summary-identity-') as temporary:
                root = Path(temporary)
                source = {'src/main.rs': 'a' * 64}
                def execute(command, log, cwd, timeout):
                    pointer = json.loads((root / 'results/verification/latest-offline.json').read_bytes())
                    running = json.loads((root / pointer['report']).read_bytes())
                    self.assertEqual(running['source_commit'], '1' * 40)
                    failed = 'tools/capture_tests.py' in command
                    return {'state': 'failed' if failed else 'completed', 'exit_code': 1 if failed else 0}
                with patch.object(verification, 'ROOT', root), patch.object(verification, 'git_state', return_value=('1' * 40, dirty)), patch.object(verification, 'committed_source_identity', return_value=source), patch.object(verification.capture_tests, 'source_identity', return_value=source), patch.object(verification.capture_tests, 'execute', side_effect=execute), redirect_stdout(io.StringIO()):
                    self.assertEqual(verification.main(), 1)
                pointer = json.loads((root / 'results/verification/latest-offline.json').read_bytes())
                final = json.loads((root / pointer['report']).read_bytes())
                self.assertEqual(final['source_commit'], '1' * 40)
                self.assertFalse(final['passed'])
                if dirty:
                    self.assertEqual(final['state'], 'refused')
                else:
                    self.assertEqual(final['runs'][-1]['name'], 'normal')

    def test_trust_data_stages_publish_only_fixed_names_and_states(self):
        secret='CONTROLLED_PRIVATE_TRUST_REASON'
        with tempfile.TemporaryDirectory(prefix='public-trust-stage-') as temporary:
            root=Path(temporary)
            location='results/verification/offline/20261115T000000.000000Z-23456789/verification.json'
            target=root/location;target.parent.mkdir(parents=True)
            pointer=root/'results/verification/latest-offline.json'
            pointer.write_bytes(json.dumps({'report':location}).encode())
            target.write_bytes(json.dumps({'passed':False,'source_commit':'1'*40,'runs':[
                {'name':'trust-data','state':'completed','error':secret},
                {'name':'trust-data-controls','state':'failed','error':secret},
                {'name':secret,'state':'completed'},
                {'name':'trust-data','state':secret}]}).encode())
            with patch.object(public.subprocess,'check_output',return_value=b'1'*40+b'\n'), patch.object(public,'find_native',return_value=None), patch.object(public,'public_basenames',return_value=set()):
                result=public.summary(root)
            self.assertEqual(result['stages'],[{'name':'trust-data','state':'completed'},{'name':'trust-data-controls','state':'failed'}])
            self.assertFalse(result['passed'])
            self.assertNotIn(secret,json.dumps(result))

    def test_build_identity_stage_publishes_only_fixed_name_and_state(self):
        secret='CONTROLLED_PRIVATE_METADATA_REASON'
        source_path='C:/controlled-private/source'
        fixture_sha='e'*40
        log='controlled-private-build.log'
        with tempfile.TemporaryDirectory(prefix='public-metadata-stage-') as temporary:
            root=Path(temporary)
            location='results/verification/offline/20261115T000000.000000Z-23456789/verification.json'
            target=root/location;target.parent.mkdir(parents=True)
            (root/'results/verification/latest-offline.json').write_bytes(json.dumps({'report':location}).encode())
            target.write_bytes(json.dumps({'passed':False,'source_commit':'1'*40,'runs':[
                {'name':'build-identity-controls','state':'completed','error':secret,'source_root':source_path,'fixture_sha':fixture_sha,'log':log},
                {'name':secret,'state':'completed'},
                {'name':'build-identity-controls','state':secret}]}).encode())
            with patch.object(public.subprocess,'check_output',return_value=b'1'*40+b'\n'), patch.object(public,'find_native',return_value=None), patch.object(public,'public_basenames',return_value=set()):
                result=public.summary(root)
            self.assertEqual(result['stages'],[{'name':'build-identity-controls','state':'completed'}])
            self.assertFalse(result['passed'])
            encoded=json.dumps(result)
            for forbidden in (secret,source_path,fixture_sha,log,'source_root','fixture_sha'):
                self.assertNotIn(forbidden,encoded)

    def test_hostile_strings_unknown_identifiers_and_metadata_never_project(self):
        secret = 'https://private.invalid/path?token=CONTROLLED_SECRET'
        manifest = {'schema_version': 1, 'state': 'not_passed', 'error': secret,
                    'environment': {'PRIVATE': secret}, 'source_before': {'C:/private/file': secret},
                    'steps': [{'name': 'normal', 'state': 'failed', 'error': secret, 'log': secret,
                        'analysis': {'summary': {'state': 'FAILED', 'passed': 1, 'failed': 1, 'ignored': 0, 'measured': 0, 'filtered': 0},
                            'announced_tests': 2, 'failed_test_names': ['known_module::known_second', secret,
                                'unknown_module::unknown_test', 'PRIVATE_PREFIX::known_second', 'x' * 513, {'secret': secret}]}}]}
        result = public.native_projection(manifest, {'known_second'})
        self.assertEqual(result['normal']['failed_test_basenames'], ['known_second'])
        self.assertEqual(result['normal']['excluded_identifier_count'], 4)
        self.assertFalse(result['normal']['failed_identifiers_complete'])
        encoded = json.dumps(result)
        for private in [secret, 'CONTROLLED_SECRET', 'C:/private', 'PRIVATE_PREFIX', 'unknown_module', 'known_module', 'environment']:
            self.assertNotIn(private, encoded)
        self.assertFalse(result['normal_test_gate_passed'])

    def test_counts_are_strict_bounded_integers_not_booleans_or_strings(self):
        for invalid in [True, '1', -1, 10_000_001, None, {'private': 'text'}]:
            row = {'state': 'failed', 'analysis': {'announced_tests': invalid,
                    'summary': {'state': 'FAILED', 'passed': 1, 'failed': invalid, 'ignored': 0, 'measured': 0, 'filtered': 0}}}
            result = public.project_step(row, set())
            self.assertNotIn('summary', result)
            self.assertNotIn('announced_tests', result)
        self.assertEqual(public.project_step({'state': {'private': 'text'}}, set())['state'], 'unavailable')

    def test_oversized_identifiers_and_bad_native_shape_are_omitted(self):
        row = {'state': 'failed', 'analysis': {'failed_test_names': ['a::known_second'] * (public.MAX_ROWS + 1)}}
        result = public.project_step(row, {'known_second'})
        self.assertEqual(result['failed_test_basenames'], [])
        self.assertFalse(result['failed_identifiers_complete'])
        self.assertEqual(public.native_projection({'schema_version': True, 'state': 'passed'}, set())['state'], 'unavailable')
        self.assertNotIn('normal', public.native_projection({'schema_version': 1, 'state': {}, 'steps': []}, set()))

    def test_basename_registry_reads_committed_source_and_only_test_declarations(self):
        source = b'#[test]\nfn known_second() {}\n--\n#[test] fn same_line() {}\n--\nfn not_a_test() {}\n// #[test]\nfn comment_only() {}\n'
        with patch.object(public.subprocess, 'run', return_value=SimpleNamespace(returncode=0, stdout=source)) as run:
            self.assertEqual(public.public_basenames(Path.cwd()), {'known_second', 'same_line'})
            command = run.call_args.args[0]
            self.assertIn('HEAD', command)
            self.assertEqual(command[-1], 'src')

    def test_bounded_json_duplicate_keys_and_pointer_escape_refuse(self):
        with tempfile.TemporaryDirectory(prefix='public-summary-controls-') as temporary:
            root = Path(temporary)
            path = root / 'controlled.json'
            path.write_bytes(b'{"schema_version":1,"schema_version":2}')
            self.assertIsNone(public.read_json(path, root))
            path.write_bytes(b' ' * (public.MAX_JSON + 1))
            self.assertIsNone(public.read_json(path, root))
            pointer = root / 'results/verification/latest-offline.json'
            pointer.parent.mkdir(parents=True)
            pointer.write_bytes(json.dumps({'report': '../../controlled.json'}).encode())
            with patch.object(public.subprocess, 'check_output', return_value=b'1' * 40 + b'\n'), patch.object(public, 'public_basenames', side_effect=AssertionError('Unsafe pointer must not continue')):
                self.assertFalse(public.summary(root)['passed'])

    def test_stale_or_unbound_report_never_receives_current_commit_diagnostics(self):
        with tempfile.TemporaryDirectory(prefix='public-summary-stale-') as temporary:
            root = Path(temporary)
            folder = root / 'results/verification/offline/20261115T000000.000000Z-23456789'
            folder.mkdir(parents=True)
            pointer = root / 'results/verification/latest-offline.json'
            pointer.write_bytes(json.dumps({'report': str((folder / 'verification.json').relative_to(root))}).encode())
            for old in ['2' * 40, None]:
                report = {'passed': True, 'source_commit': old, 'executable_sha256': 'a' * 64,
                          'journeys': [{'valid': True}], 'runs': [{'name': 'normal', 'state': 'failed'}]}
                (folder / 'verification.json').write_bytes(json.dumps(report).encode())
                with patch.object(public.subprocess, 'check_output', return_value=b'1' * 40 + b'\n'), patch.object(public, 'public_basenames', side_effect=AssertionError('Stale input must not continue')):
                    result = public.summary(root)
                self.assertFalse(result['passed'])
                self.assertEqual(result['stages'], [])
                self.assertEqual(result['passing_journeys'], 0)
                self.assertIsNone(result['executable_sha256'])
                self.assertEqual(result['native_tests']['state'], 'unavailable')
            empty_failed_line = {'state': 'failed', 'analysis': {'failed_test_names': [], 'summary': {
                'state': 'FAILED', 'passed': 1, 'failed': 1, 'ignored': 0, 'measured': 0, 'filtered': 0}}}
            self.assertFalse(public.project_step(empty_failed_line, {'known_second'})['failed_identifiers_complete'])

    def test_real_controlled_normal_failure_preserves_serial_success_separately(self):
        # These are original Python log producers, not native test results.
        child = '''import sys
print('running 2 tests')
print('test controlled::known_first ... ok')
if sys.argv[1] == 'failure':
 print('test controlled::known_second ... FAILED')
 print('PRIVATE_ERROR https://private.invalid/?token=CONTROLLED_SECRET')
 print('test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s')
 sys.exit(101)
print('test controlled::known_second ... ok')
print('test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s')
'''
        with tempfile.TemporaryDirectory(prefix='public-summary-failure-') as temporary:
            root = Path(temporary)
            (root / 'src').mkdir()
            (root / 'src/main.rs').write_bytes(b'// Original controlled test source\n')
            producer = root / 'producer.py'
            producer.write_bytes(child.encode())
            capture = root / 'results/verification/windows-tests/20261115T000000.000000Z-12345678'
            before = datetime.now(timezone.utc) - timedelta(seconds=1)
            with redirect_stdout(io.StringIO()):
                native = capture_tests.capture(root, capture, [('normal', [sys.executable, str(producer), 'failure']),
                    ('serial', [sys.executable, str(producer), 'pass'])], 20)
            after = datetime.now(timezone.utc) + timedelta(seconds=1)
            folder = root / 'results/verification/offline/20261115T000000.000000Z-23456789'
            folder.mkdir(parents=True)
            report = {'passed': False, 'source_commit': '1' * 40, 'source_before': native['source_before'], 'runs': [{'name': 'normal', 'state': 'failed',
                        'started_utc': before.isoformat(), 'finished_utc': after.isoformat(), 'error': 'PRIVATE_ERROR'}]}
            (folder / 'verification.json').write_bytes(json.dumps(report).encode())
            (root / 'results/verification/latest-offline.json').write_bytes(json.dumps({'report': str((folder / 'verification.json').relative_to(root))}).encode())
            with patch.object(public.subprocess, 'check_output', return_value=b'1' * 40 + b'\n'), patch.object(public, 'public_basenames', return_value={'known_second'}):
                result = public.summary(root)
            self.assertFalse(result['passed'])
            self.assertEqual(result['source_commit'], '1' * 40)
            self.assertFalse(result['native_tests']['normal_test_gate_passed'])
            self.assertEqual(result['native_tests']['normal']['summary']['failed'], 1)
            self.assertEqual(result['native_tests']['normal']['failed_test_basenames'], ['known_second'])
            self.assertEqual(result['native_tests']['serial']['summary']['passed'], 2)
            report.update(passed=True, source_commit='1' * 40)
            (folder / 'verification.json').write_bytes(json.dumps(report).encode())
            with patch.object(public.subprocess, 'check_output', return_value=b'1' * 40 + b'\n'), patch.object(public, 'public_basenames', return_value={'known_second'}):
                self.assertFalse(public.summary(root)['passed'])
            encoded = json.dumps(result)
            for forbidden in ['PRIVATE_ERROR', 'CONTROLLED_SECRET', 'https://', str(root), 'source_before', 'command', 'environment', '.log']:
                self.assertNotIn(forbidden, encoded)


    def test_abort_metadata_projects_only_strict_bounded_numbers_and_booleans(self):
        row = {'state': 'failed', 'exit_code': 101, 'error': 'CONTROLLED_SECRET',
               'analysis': {'harness_started': True, 'completed_test_lines': 37,
                            'native_exit_statuses': ['0xC00000FD'], 'failed_test_names': []}}
        result = public.project_step(row, set())
        self.assertEqual(result['exit_code'], 101)
        self.assertTrue(result['harness_started'])
        self.assertEqual(result['completed_test_lines'], 37)
        self.assertEqual(result['native_exit_statuses'], ['0xc00000fd'])
        self.assertFalse(result['failed_identifiers_complete'])
        self.assertNotIn('summary', result)
        self.assertNotIn('CONTROLLED_SECRET', json.dumps(result))
        for code in [-(1 << 31), -1, 0, (1 << 31) - 1]:
            self.assertEqual(public.project_step({'exit_code': code}, set())['exit_code'], code)
        for invalid in [True, False, '101', -(1 << 31) - 1, 1 << 31, 101.0, None, {'private': 'text'}]:
            self.assertNotIn('exit_code', public.project_step({'exit_code': invalid}, set()))
        for invalid in [0, 1, 'true', None, {'private': 'text'}]:
            result = public.project_step({'analysis': {'harness_started': invalid}}, set())
            self.assertNotIn('harness_started', result)
        for invalid in [True, '1', -1, public.MAX_COUNT + 1, None, {'private': 'text'}]:
            result = public.project_step({'analysis': {'completed_test_lines': invalid}}, set())
            self.assertNotIn('completed_test_lines', result)
        for invalid in ['0xc00000fd', ['0xc00000fd PRIVATE'], ['c00000fd'], ['0x1'],
                        ['0x100000000'], ['0x0000000g'], [0xc00000fd], ['0xc00000fd'] * 9]:
            result = public.project_step({'analysis': {'native_exit_statuses': invalid}}, set())
            self.assertNotIn('native_exit_statuses', result)

    def test_reported_panic_projection_uses_only_committed_known_basenames(self):
        secret = 'CONTROLLED_SECRET'
        names = ['private_module::known_abort', 'PRIVATE_PREFIX::known_panic',
                 'https://private.invalid/?token=' + secret, 'C:/private/source.rs',
                 'private_module::unknown', 'x' * 513, {'private': secret}, 'known_panic PRIVATE']
        row = {'state': 'failed', 'analysis': {'panicking_test_names': names,
               'panicking_names_limited': True, 'error': secret, 'log': 'C:/private/raw.log'}}
        result = public.project_step(row, {'known_abort', 'known_panic'})
        self.assertEqual(result['panicking_test_basenames'], ['known_abort', 'known_panic'])
        self.assertTrue(result['panicking_names_limited'])
        self.assertEqual(result['failed_test_basenames'], [])
        self.assertNotIn('summary', result)
        encoded = json.dumps(result)
        for private in [secret, 'private_module', 'PRIVATE_PREFIX', 'https://', 'C:/private', 'unknown', '.log']:
            self.assertNotIn(private, encoded)
        for invalid in [names + ['known_abort'], secret, {'private': secret}]:
            self.assertNotIn('panicking_test_basenames', public.project_step({'analysis': {'panicking_test_names': invalid}}, {'known_abort'}))
        self.assertNotIn('panicking_names_limited', public.project_step({'analysis': {'panicking_names_limited': secret}}, set()))

    def test_controlled_abort_without_summary_keeps_numeric_status_and_serial_diagnostic_separate(self):
        # An original Python producer models incomplete native output; this is
        # not evidence of the cause of any hosted Rust process failure.
        child = '''import os, sys
print('running 2 tests', flush=True)
print('test private_module::known_first ... ok', flush=True)
if sys.argv[1] == 'abort':
 print("thread 'private_module::known_abort' (123) has overflowed its stack", flush=True)
 print('CONTROLLED_SECRET C:/private/source.rs https://private.invalid/?token=secret', flush=True)
 print('process did not exit (exit code: 0xc00000fd, STATUS_STACK_OVERFLOW)', flush=True)
 os._exit(71)
print('test private_module::known_abort ... ok', flush=True)
print('test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;', flush=True)
'''
        with tempfile.TemporaryDirectory(prefix='public-abort-diagnostics-') as temporary:
            root = Path(temporary)
            (root / 'src').mkdir()
            (root / 'src/main.rs').write_text('// Original controlled source\n', encoding='utf-8')
            producer = root / 'producer.py'
            producer.write_text(child, encoding='utf-8')
            with redirect_stdout(io.StringIO()):
                native = capture_tests.capture(root, root / 'capture',
                    [('normal', [sys.executable, str(producer), 'abort']),
                     ('serial', [sys.executable, str(producer), 'pass'])], 20)
            result = public.native_projection(native, {'known_first', 'known_abort'})
            self.assertFalse(result['normal_test_gate_passed'])
            normal = result['normal']
            self.assertEqual(normal['state'], 'failed')
            self.assertEqual(normal['exit_code'], 71)
            self.assertTrue(normal['harness_started'])
            self.assertEqual(normal['completed_test_lines'], 1)
            self.assertEqual(normal['native_exit_statuses'], ['0xc00000fd'])
            self.assertEqual(normal['panicking_test_basenames'], ['known_abort'])
            self.assertEqual(normal['failed_test_basenames'], [])
            self.assertFalse(normal['failed_identifiers_complete'])
            self.assertNotIn('summary', normal)
            self.assertEqual(result['serial']['summary']['passed'], 2)
            self.assertIn('not proof', result['panic_scope'])
            encoded = json.dumps(result)
            for private in ['CONTROLLED_SECRET', 'private_module', 'C:/private', 'https://', str(root), '.log', 'environment', 'command']:
                self.assertNotIn(private, encoded)


if __name__ == '__main__':
    unittest.main()
