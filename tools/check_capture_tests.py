#!/usr/bin/env python3
"""Controlled checks of test-log orchestration, not Rust/browser execution."""

import argparse
from contextlib import redirect_stdout
import hashlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import capture_tests as runner


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = r'''
import json, os, sys, time
from pathlib import Path
case = sys.argv[1]
if case == 'version':
    print('Controlled compiler version fixture; not rustc')
    sys.exit(0)
if case == 'compile-fail':
    print('error: controlled compilation failure', file=sys.stderr)
    sys.exit(101)
print('running 2 tests', flush=True)
print('test controlled::first ... ok', flush=True)
if case == 'abort':
    print('controlled fatal native diagnostic', file=sys.stderr, flush=True)
    print("process did not exit (exit code: 0xc0000409, STATUS_STACK_BUFFER_OVERRUN)",
          file=sys.stderr, flush=True)
    os._exit(71)
if case == 'timeout':
    time.sleep(30)
if case == 'mutate':
    Path('src/main.rs').write_text('// edited during check\n', encoding='utf-8')
if case == 'no-summary':
    sys.exit(0)
if case == 'failure':
    print('test controlled::second ... FAILED', flush=True)
    print('test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s')
    sys.exit(101)
print('test controlled::second ... ok', flush=True)
print('test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s')
'''


class CaptureTests(unittest.TestCase):
    def test_source_identity_hashes_embedded_suffix_data(self):
        with tempfile.TemporaryDirectory(prefix="chezcat-cookie-inputs-") as temporary:
            root = Path(temporary)
            path = root / "data/public_suffix/rules.rs"
            path.parent.mkdir(parents=True)
            path.write_text('// first data snapshot\n', encoding="utf-8")
            first = runner.source_identity(root)
            self.assertIn("data/public_suffix/rules.rs", first)
            path.write_text('// second data snapshot\n', encoding="utf-8")
            second = runner.source_identity(root)
            self.assertNotEqual(first["data/public_suffix/rules.rs"],
                                second["data/public_suffix/rules.rs"])

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='chezcat-test-diagnostics-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'project with spaces'
        self.root.mkdir()
        (self.root / 'src').mkdir()
        (self.root / 'src/main.rs').write_text('// controlled source\n', encoding='utf-8')
        (self.root / 'Cargo.toml').write_text('[package]\nname="chezcat"\n', encoding='utf-8')
        self.child = self.root / 'controlled child.py'
        self.child.write_text(FIXTURE, encoding='utf-8')

    def command(self, case):
        return [sys.executable, str(self.child), case]

    def run_case(self, normal, serial='pass', name='run'):
        plan = [('compiler', self.command('version')),
                ('normal', self.command(normal)), ('serial', self.command(serial))]
        with redirect_stdout(io.StringIO()):
            return runner.capture(self.root, self.root / name, plan, 20)

    def test_normal_success_has_no_serial_retry(self):
        result = self.run_case('pass')
        self.assertTrue(result['normal_test_gate_passed'])
        self.assertEqual([r['name'] for r in result['steps']], ['compiler', 'normal'])
        self.assertTrue(result['source_unchanged'])
        for row in result['steps']:
            path = self.root / 'run' / row['log']
            self.assertEqual(row['sha256'], hashlib.sha256(path.read_bytes()).hexdigest())

    def test_abort_log_survives_and_serial_pass_does_not_erase_it(self):
        result = self.run_case('abort')
        self.assertFalse(result['normal_test_gate_passed'])
        self.assertEqual(result['state'], 'not_passed')
        self.assertTrue(runner.passed(result['steps'][2]))
        normal = result['steps'][1]
        self.assertEqual(normal['exit_code'], 71)
        self.assertFalse(normal['analysis']['complete_success_summary'])
        self.assertEqual(normal['analysis']['native_exit_statuses'], ['0xc0000409'])
        self.assertIn('controlled fatal native diagnostic',
                      (self.root / 'run/normal.log').read_text(encoding='utf-8'))

    def test_assertion_failure_retains_both_attempts(self):
        result = self.run_case('failure', 'failure')
        self.assertFalse(result['normal_test_gate_passed'])
        self.assertEqual(len(result['steps']), 3)
        for row in result['steps'][1:]:
            self.assertEqual(row['analysis']['failed_test_names'], ['controlled::second'])
            self.assertEqual(row['analysis']['summary']['failed'], 1)

    def test_missing_summary_is_not_success(self):
        result = self.run_case('no-summary')
        self.assertEqual(result['steps'][1]['exit_code'], 0)
        self.assertFalse(result['normal_test_gate_passed'])
        self.assertTrue(runner.passed(result['steps'][2]))

    def test_compile_failure_does_not_retry_an_unstarted_harness(self):
        result = self.run_case('compile-fail')
        self.assertFalse(result['normal_test_gate_passed'])
        self.assertEqual(len(result['steps']), 2)
        self.assertFalse(result['steps'][1]['analysis']['harness_started'])

    def test_source_edit_cannot_receive_passing_gate(self):
        result = self.run_case('mutate')
        self.assertTrue(runner.passed(result['steps'][1]))
        self.assertFalse(result['source_unchanged'])
        self.assertFalse(result['normal_test_gate_passed'])

    def test_launch_failure_is_recorded(self):
        plan = [('compiler', [str(self.root / 'nonexistent-tool')])]
        with redirect_stdout(io.StringIO()):
            result = runner.capture(self.root, self.root / 'missing', plan, 20)
        self.assertFalse(result['normal_test_gate_passed'])
        self.assertEqual(result['steps'][0]['state'], 'launch_error')
        self.assertTrue((self.root / 'missing/summary.txt').exists())

    def test_timeout_keeps_partial_log_and_does_not_retry(self):
        # The timeout check does not require the interpreter to start within a
        # fragile fraction of a second; an empty partial log is valid evidence.
        log = self.root / 'timeout.log'
        row = runner.execute(self.command('timeout'), log, self.root, 3)
        self.assertEqual(row['state'], 'timeout')
        self.assertTrue(log.exists())
        self.assertEqual(row['sha256'], runner.sha256(log))

    def test_output_limit_cannot_hide_a_success(self):
        log = self.root / 'large.log'
        code = "import os; os.write(1,b'x'*16384)"
        with patch.object(runner, 'MAX_LOG_BYTES', 1024):
            row = runner.execute([sys.executable, '-c', code], log, self.root, 20)
            analysis = runner.analyze(log)
        self.assertEqual(row['state'], 'output_limit')
        self.assertTrue(analysis['analysis_truncated'])
        self.assertFalse(analysis['complete_success_summary'])

    def test_empty_filtered_ignored_or_inconsistent_summaries_are_not_a_gate(self):
        log = self.root / 'summary.log'
        for start, summary in [
            (0, '0 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out;'),
            (2, '1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out;'),
            (3, '2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;'),
        ]:
            log.write_text(f'running {start} tests\ntest result: ok. {summary}\n', encoding='utf-8')
            self.assertFalse(runner.analyze(log)['complete_success_summary'])

    def test_compiler_artifact_identity_is_retained_without_execution(self):
        executable = self.root / 'fake test executable.exe'
        executable.write_bytes(b'controlled identity only')
        log = self.root / 'artifact.log'
        item = {'reason': 'compiler-artifact', 'target': {'name': 'chezcat'},
                'profile': {'test': True}, 'executable': str(executable)}
        log.write_text(json.dumps(item) + '\n', encoding='utf-8')
        result = runner.analyze(log)
        self.assertEqual(result['test_executables'][0]['sha256'], runner.sha256(executable))
        self.assertFalse(result['harness_started'])

    def test_command_keeps_existing_flags_and_does_not_change_limits(self):
        plan = runner.commands('stable')
        self.assertEqual(plan[0][1], ['rustup', 'run', 'stable', 'rustc', '-Vv'])
        self.assertEqual(plan[1][1], ['rustup', 'run', 'stable', 'cargo', 'test',
                         '--offline', '--locked', '--bin', 'chezcat', '--message-format=json',
                         '--', '--no-capture', '--color=never'])
        self.assertEqual(plan[2][1], plan[1][1] + ['--test-threads=1'])

    def test_distinct_run_folders_keep_prior_logs(self):
        self.run_case('failure', name='first')
        before = (self.root / 'first/normal.log').read_bytes()
        self.run_case('pass', name='second')
        self.assertEqual((self.root / 'first/normal.log').read_bytes(), before)
        with self.assertRaises(FileExistsError):
            self.run_case('pass', name='second')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'results/verification/test-capture-runner.json')
    args = parser.parse_args()
    log = io.StringIO()
    result = unittest.TextTestRunner(stream=log, verbosity=2).run(
        unittest.defaultTestLoader.loadTestsFromTestCase(CaptureTests))
    report = {'scope': 'Controlled Python subprocess and parsing tests; no Rust or Windows runtime executed.',
              'tests_run': result.testsRun, 'failures': len(result.failures),
              'errors': len(result.errors), 'skipped': len(result.skipped),
              'passed': result.testsRun - len(result.failures) - len(result.errors) - len(result.skipped),
              'successful': result.wasSuccessful(), 'log': log.getvalue()}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(log.getvalue(), end='')
    return 0 if result.wasSuccessful() else 1


if __name__ == '__main__':
    raise SystemExit(main())
