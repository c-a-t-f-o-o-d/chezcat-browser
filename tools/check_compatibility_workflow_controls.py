"""Focused offline controls for the scorecard's first-failure classification."""
import unittest
from pathlib import Path
from types import SimpleNamespace
import tempfile
from unittest.mock import patch
import urllib.parse
import check_compatibility_workflows as scorecard


class DiagnosticControls(unittest.TestCase):
    def test_actual_original_fixture_manifest_matches_retained_bytes(self):
        identities = scorecard.fixture_identity()
        required = {name + suffix for name in ('w1', 'w2', 'w3') for suffix in ('.html', '.js')}
        self.assertTrue(required.issubset(identities))

    def test_absence_sentinel_is_not_a_failure(self):
        self.assertEqual(scorecard.script_failures('Last escaped engine fault: stage=unknown kind=unknown\n'), [])

    def test_actual_fault_with_unknown_stage_is_retained(self):
        line = 'Last escaped engine fault: stage=unknown kind=TypeError'
        self.assertEqual(scorecard.script_failures(line), [line])

    def test_first_script_error_and_later_messages_keep_their_order(self):
        first = 'library.js: JavaScript error at line 2, column 1: original failure.'
        later = 'application.js: Uncaught exception: ReferenceError: dependency unavailable.'
        activity = first + '\n' + later + '\nLast escaped engine fault: stage=unknown kind=unknown\n'
        self.assertEqual(scorecard.script_failures(activity), [first, later])

    def reference_exception_case(self, app_error=None):
        """Synthetic score facts only: no App, socket, browser or package runs."""
        class Fixture:
            def __init__(self, *_):
                paths = ['/w1/', '/workflow.css', '/control.ppm', '/w1.js',
                    '/w1/search?q=' + urllib.parse.quote(scorecard.FIRST),
                    '/w1/search?q=' + urllib.parse.quote(scorecard.SECOND)]
                self.rows = [{'target': value, 'completed': True, 'status': 200} for value in paths]
                self.server = SimpleNamespace(server_port=12345)
                self.stopped = False

            def __enter__(self):
                return self

            def __exit__(self, *_):
                self.stopped = True

        loaded = {'visible': 'W1 async ready', 'original_control_exact_pixels': [4, 4, 4, 4]}
        facts = {'loaded': loaded, 'first-results': {'visible': scorecard.FIRST},
            'detail': {'visible': 'Original detail page'},
            'back': {'title': 'W1 results ' + scorecard.FIRST},
            'second-results': {'visible': scorecard.SECOND}}
        for value in facts.values():
            value.update(present=True, paired=True, javascript_enabled=True,
                zero_errors=app_error is None, first_root=app_error)
        execute = unittest.mock.Mock(return_value={'state': 'completed', 'exit_code': 0})
        with tempfile.TemporaryDirectory() as scratch:
            folder = Path(scratch) / 'case'
            with patch.object(scorecard, 'Fixture', Fixture), \
                    patch.object(scorecard, 'read_stage', side_effect=lambda _, label: facts[label]), \
                    patch.object(scorecard, 'reference_initial', side_effect=ValueError('Reference digest changed')) as reference:
                result = scorecard.run_case('W1', 1280, Path('not-executed.exe'), folder, {},
                    SimpleNamespace(execute=execute), {'explicit': 'reference'})
            execute.assert_called_once()
            reference.assert_called_once()
        return result

    def test_reference_exception_preserves_successful_app_score(self):
        result = self.reference_exception_case()
        self.assertTrue(result['passed'])
        self.assertIsNone(result['first_failed_stage'])
        self.assertEqual(len(result['snapshots']), 5)
        self.assertEqual(result['reference_initial']['error_type'], 'ValueError')
        self.assertEqual(result['reference_initial']['error'], 'Reference digest changed')
        self.assertFalse(result['reference_initial']['passed'])
        self.assertEqual(result['reference_interaction'], 'Not run')

    def test_reference_exception_does_not_hide_actual_app_error(self):
        failure = 'fixture.js: JavaScript error at line 1, column 1: actual App failure.'
        result = self.reference_exception_case(failure)
        self.assertFalse(result['passed'])
        self.assertFalse(result['checks']['all_current_scripts_zero_errors'])
        self.assertEqual(result['first_ordered_script_failure'], failure)
        self.assertFalse(result['reference_initial']['passed'])


if __name__ == '__main__':
    unittest.main()
