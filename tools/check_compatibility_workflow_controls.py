"""Focused offline controls for the scorecard's first-failure classification."""
import unittest
from pathlib import Path
from types import SimpleNamespace
import tempfile
from unittest.mock import patch
import urllib.parse
import check_compatibility_workflows as scorecard
import check_modern_compatibility_workflows as modern


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

    def title_stage(self, header):
        """Synthetic paired artifacts; no executable, network or browser run."""
        with tempfile.TemporaryDirectory() as scratch:
            folder = Path(scratch)
            activity = header + '\n\n' + (
                'Stage document=1 dom_commit=1\n'
                'Stage layout=1 layout_document=1 layout_dom=1 page_view=1\n'
                'Stage canvas=1 document=1 dom_commit=1 layout=1 layout_document=1 layout_dom=1 page_view=1 width=2 height=2\n'
                'Stage paired_ppm_written=1\n1 script(s), 0 active listener(s), 0 error(s).\n'
                'Page title: Later diagnostic text must not replace the header\n')
            (folder / '00-live.activity.txt').write_text(activity, encoding='utf-8')
            (folder / '00-live.layout.txt').write_text('', encoding='utf-8')
            (folder / '00-live.source.html').write_text('<title>Original source title</title>', encoding='utf-8')
            (folder / '00-live.ppm').write_bytes(b'P6\n2 2\n255\n' + bytes(12))
            return scorecard.read_stage(folder, 'live')

    def test_stage_title_uses_live_activity_and_keeps_literal_entities(self):
        result = self.title_stage('ChezCat test activity report\nStatus: complete\nPage title: Live &amp; title 雪\nJavaScript enabled: true')
        self.assertTrue(result['paired'])
        self.assertEqual(result['title'], 'Live &amp; title 雪')

    def test_missing_or_ambiguous_live_title_never_falls_back_to_original_source(self):
        for header in ('ChezCat test activity report\nStatus: complete',
                       'ChezCat test activity report\nPage title: First\nPage title: Ambiguous'):
            with self.subTest(header=header):
                self.assertIsNone(self.title_stage(header)['title'])

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


class ModernScoreControls(unittest.TestCase):
    """Synthetic score facts only; no executable, HTTP server or browser runs."""
    def facts(self):
        base = {'present': True, 'paired': True, 'javascript_enabled': True,
                'zero_errors': True, 'first_root': None, 'later_script_errors': [],
                'original_control_exact_pixels': [24, 200, 4, 4],
                'canvas': {'width': '1280', 'height': '800'}}
        return {'loaded': dict(base, visible='jQuery initialized version=4.0.0'),
                'updated': dict(base, visible='jQuery trusted update amber 雪 jQuery asynchronous update complete')}

    def scores(self, stages):
        requests = [{'target': path, 'completed': True, 'status': 200} for path in
                    ('/jquery/', '/jquery-app.js', '/modern.css', '/control.ppm', '/jquery-4.0.0.js')]
        return modern.score('jquery', 1280, stages, requests)

    def test_actions_require_control_names_and_correct_tags(self):
        valid = '<input id="modern-name" name="modern-name"><button name="jquery-apply"></button>'
        modern.check_action_targets('jquery', valid)
        for invalid in (valid.replace(' name="modern-name"', ''),
                        valid + '<input name="modern-name">',
                        valid.replace('<input ', '<p '),
                        valid.replace('name="modern-name"', 'name="modern-name" name="duplicate"')):
            with self.subTest(source=invalid), self.assertRaises(ValueError):
                modern.check_action_targets('jquery', invalid)

    def test_actual_rectangle_contract_passes_complete_synthetic_score(self):
        self.assertTrue(all(row['state'] == 'Passed' for row in self.scores(self.facts()).values()))

    def test_missing_false_and_malformed_controls_refuse_render(self):
        for value in (None, False, True, [], [24, 200, 4], [-1, 200, 4, 4],
                      [24, 200, 4, 5], [True, 200, 4, 4], {'x': 24}):
            with self.subTest(value=value):
                stages = self.facts()
                stages['loaded']['original_control_exact_pixels'] = value
                self.assertEqual(self.scores(stages)['render']['state'], 'Failed')
        stages = self.facts()
        del stages['loaded']['original_control_exact_pixels']
        self.assertEqual(self.scores(stages)['render']['state'], 'Failed')

    def test_control_must_fit_actual_canvas_and_valid_dimensions(self):
        for rectangle, canvas in (([1278, 200, 4, 4], {'width': '1280', 'height': '800'}),
                                  ([24, 798, 4, 4], {'width': '1280', 'height': '800'}),
                                  ([24, 200, 4, 4], {}),
                                  ([24, 200, 4, 4], {'width': 'unknown', 'height': '800'}),
                                  ([24, 200, 4, 4], {'width': 1280.0, 'height': '800'})):
            with self.subTest(rectangle=rectangle, canvas=canvas):
                stages = self.facts()
                stages['loaded'].update(original_control_exact_pixels=rectangle, canvas=canvas)
                self.assertEqual(self.scores(stages)['render']['state'], 'Failed')

    def test_snapshot_error_refuses_render_even_with_stale_positive_facts(self):
        stages = self.facts()
        stages['loaded']['snapshot_error'] = 'Control PPM unavailable'
        self.assertEqual(self.scores(stages)['render']['state'], 'Failed')
        stages['loaded'] = {'present': False, 'snapshot_error': 'Control PPM unavailable', 'first_root': None}
        scores = self.scores(stages)
        self.assertEqual(scores['render']['state'], 'Failed')
        self.assertEqual(scores['initialization']['state'], 'Failed')

    def test_initialization_cascade_preserves_only_observed_load_and_render(self):
        stages = self.facts()
        stages['loaded'].update(visible='jQuery initialization pending', zero_errors=False,
                                first_root='library.js: JavaScript error at line 1, column 1: actual runtime failure.')
        stages['updated'].update(zero_errors=False, first_root='Uncaught exception: jQuery is undefined')
        scores = self.scores(stages)
        self.assertEqual(scores['load']['state'], 'Passed')
        self.assertEqual(scores['render']['state'], 'Passed')
        self.assertEqual(scores['parse']['state'], 'Not established')
        for name in ('initialization', 'input', 'async'):
            self.assertEqual(scores[name]['state'], 'Failed')

    def test_runtime_syntaxerror_kind_is_not_renamed_parser_failure(self):
        stages = self.facts()
        stages['loaded'].update(visible='Initialization pending', zero_errors=False,
                                first_root='Last escaped engine fault: stage=execute kind=SyntaxError')
        self.assertEqual(self.scores(stages)['parse']['state'], 'Not established')

    def test_explicit_parser_fault_is_retained_if_engine_provides_it(self):
        stages = self.facts()
        stages['loaded'].update(visible='Initialization pending', zero_errors=False,
                                first_root='Last escaped engine fault: stage=parse kind=fatal')
        self.assertEqual(self.scores(stages)['parse']['state'], 'Failed')


if __name__ == '__main__':
    unittest.main()
