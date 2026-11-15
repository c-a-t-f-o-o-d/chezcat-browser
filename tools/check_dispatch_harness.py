"""Offline executable check of synchronous dispatch stack guard and recovery.

Also checks the original legacy CustomEvent detail/cancellation fixture through
two real button activations in a second App/input process. A third process
checks genuine base Event construction/dispatch with two trusted activations,
canonical owner roots, descriptors, stored slots, path copies and cancellation.
A fourth owned HTTP page checks genuine UIEvent conversion/dispatch, native
keyboard ancestry and child owner roots through real App input. A fifth owned
HTTP page checks finite MouseEvent constructors and genuine slots while Mouse
dispatch remains refused. No desktop input or
public request; the driver never creates license acceptance. A sixth owned HTTP
page constructs and dispatches real CustomEvent instances, retaining any detail,
canonical initialization and child owner roots through two trusted base inputs.
"""
from datetime import datetime, timezone
import json
from pathlib import Path
import struct
import re
import accept_license
import uuid
import capture_tests

ROOT=Path(__file__).resolve().parents[1]

def custom_event_case(exe, folder):
    """A second owned real App/input process; no nested Evidence output."""
    folder.mkdir()
    before = capture_tests.source_identity(ROOT)
    executable = capture_tests.sha256(exe)
    fixture = ROOT / 'tools/fixtures/legacy-custom-event.html'
    checks = {'custom_case_completed': False, 'custom_existing_exact_acceptance': False}
    record = {'scope': 'Original legacy CustomEvent positive dispatch, cancellation and shared detail identity through real App input.',
        'source_before': before, 'executable_sha256': executable,
        'fixture_sha256': capture_tests.sha256(fixture), 'checks': checks,
        'license_accepted_by_driver': False, 'stages': {}}
    try:
        accept_license.terms()
        checks['custom_existing_exact_acceptance'] = accept_license.check(accept_license.directory())
        if not checks['custom_existing_exact_acceptance']:
            raise ValueError('Caller-provided exact-term acceptance required; driver never accepts.')
        script = folder / 'input.tsv'
        script.write_text('snapshot\tbefore\nclick\temit\nsnapshot\tfirst\nclick\temit\nsnapshot\tsecond\n', encoding='utf-8', newline='\n')
        output = folder / 'journey'
        run = capture_tests.execute([str(exe), str(fixture), '--dev-input-script', str(script),
            '--dev-input-output', str(output), '--javascript', '--no-profile'], folder / 'command.log', ROOT, 120)
        record['run'] = run
        checks['custom_process_completed'] = run['state'] == 'completed' and run['exit_code'] == 0
        for label, expected in [
            ('before', 'Waiting for a real button activation.'),
            ('first', 'Received 1 original notification(s) with the same detail object.'),
            ('second', 'Received 2 original notification(s) with the same detail object.'),
        ]:
            matches = list(output.glob('*-' + label + '.activity.txt'))
            if len(matches) != 1:
                raise ValueError('Expected one real CustomEvent snapshot: ' + label)
            activity_path = matches[0]
            prefix = activity_path.name.removesuffix('.activity.txt')
            layout_path = activity_path.with_name(prefix + '.layout.txt')
            source_path = activity_path.with_name(prefix + '.source.html')
            ppm_path = activity_path.with_name(prefix + '.ppm')
            activity = activity_path.read_text(encoding='utf-8')
            layout = layout_path.read_text(encoding='utf-8')
            source = source_path.read_text(encoding='utf-8')
            def stamp(prefix):
                line = next((line for line in activity.splitlines() if line.startswith(prefix)), '')
                return dict(re.findall(r'(\w+)=([^\s]+)', line))
            document, geometry, canvas = (stamp(prefix) for prefix in ['Stage document=', 'Stage layout=', 'Stage canvas='])
            raw = ppm_path.read_bytes().split(b'\n', 3)
            valid_pixels = (len(raw) == 4 and raw[0] == b'P6' and raw[2] == b'255'
                and list(map(int, raw[1].split())) == [int(canvas['width']), int(canvas['height'])]
                and int(canvas['width']) > 0 and int(canvas['height']) > 0
                and len(raw[3]) == int(canvas['width']) * int(canvas['height']) * 3)
            checks['custom_' + label + '_paired_document_layout_pixels'] = (
                'Stage paired_ppm_written=1' in activity and valid_pixels
                and all(record.get(key) not in [None, 'unknown'] for record, keys in [
                    (document, ['document', 'dom_commit']),
                    (geometry, ['layout', 'layout_document', 'layout_dom']),
                    (canvas, ['document', 'dom_commit', 'layout', 'layout_document', 'layout_dom', 'page_view'])]
                    for key in keys)
                and document.get('document', 'unknown') != 'unknown'
                and canvas.get('document') == document.get('document') == geometry.get('layout_document')
                and canvas.get('dom_commit') == document.get('dom_commit')
                and canvas.get('layout') == geometry.get('layout') != 'unknown'
                and canvas.get('layout_dom') == geometry.get('layout_dom') == document.get('dom_commit')
                and canvas.get('layout_document') == document.get('document') and canvas.get('page_view') == '1')
            # Status may retain an old summary. Only the anchored interpreter
            # report is authoritative for this snapshot's current error count.
            summaries = re.findall(r'^(\d+) script\(s\), (\d+) active listener\(s\), (\d+) error\(s\)\.$', activity, re.M)
            checks['custom_' + label + '_current_interpreter_zero_errors'] = (
                len(summaries) == 1 and int(summaries[0][0]) >= 1 and int(summaries[0][2]) == 0)
            visible = ' '.join(re.findall(r'text="([^"\n]*)"', layout))
            checks['custom_' + label + '_actual_visible_result'] = expected in visible
            checks['custom_' + label + '_original_guarded_source'] = all(value in source for value in [
                "document.createEvent('CustomEvent')", 'event.initCustomEvent(name,true,true,detail)',
                'if(!input.isTrusted)', 'event.isTrusted||event.target!==document||event.detail!==lastPayload',
                'event.detail.observed=true', 'event.preventDefault()', 'if(accepted||!lastPayload.observed)'])
            record['stages'][label] = {'document': document, 'layout': geometry, 'canvas': canvas,
                'interpreter_summaries': [list(value) for value in summaries], 'visible_text': visible,
                'files': {path.name: {'bytes': path.stat().st_size, 'sha256': capture_tests.sha256(path)}
                    for path in [activity_path, layout_path, source_path, ppm_path]}}
        stages = record['stages']
        checks['custom_same_actual_document_across_both_activations'] = (
            stages['before']['document']['document'] == stages['first']['document']['document'] == stages['second']['document']['document'])
        checks['custom_each_activation_committed_visible_dom_change'] = (
            stages['before']['document']['dom_commit'] != stages['first']['document']['dom_commit']
            and stages['first']['document']['dom_commit'] != stages['second']['document']['dom_commit'])
        def ppm_hash(label):
            return next(value['sha256'] for name, value in stages[label]['files'].items() if name.endswith('.ppm'))
        checks['custom_each_received_count_changed_actual_pixels'] = ppm_hash('before') != ppm_hash('first') != ppm_hash('second')
        checks['custom_case_completed'] = True
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        record['error_type'] = type(error).__name__
        record['error'] = str(error)
    record['source_after'] = capture_tests.source_identity(ROOT)
    record['executable_after_sha256'] = capture_tests.sha256(exe)
    checks['custom_source_unchanged'] = before == record['source_after']
    checks['custom_executable_unchanged'] = executable == record['executable_after_sha256']
    record['passed'] = bool(checks) and all(value is True for value in checks.values())
    return record

def base_event_case(exe, folder, *, source=None):
    """A third owned real App/input process; no nested Evidence output."""
    folder.mkdir()
    before = capture_tests.source_identity(ROOT)
    executable = capture_tests.sha256(exe)
    fixture = ROOT / 'tools/fixtures/base-event.html'
    source = str(source if source is not None else fixture)
    checks = {'event_case_completed': False, 'event_existing_exact_acceptance': False}
    record = {'scope': 'Original genuine base Event constructor/descriptors/brands, stored-slot dispatch and cancellation, path copies, legacy factory and child realm through real App input.',
        'source': source, 'trust_file_sha256': None,
        'source_before': before, 'executable_sha256': executable,
        'fixture_sha256': capture_tests.sha256(fixture), 'checks': checks,
        'license_accepted_by_driver': False, 'stages': {}}
    try:
        accept_license.terms()
        checks['event_existing_exact_acceptance'] = accept_license.check(accept_license.directory())
        if not checks['event_existing_exact_acceptance']:
            raise ValueError('Caller-provided exact-term acceptance required; driver never accepts.')
        identity = json.loads((ROOT / 'tools/fixtures/base-event.json').read_text(encoding='utf-8'))['files']['base-event.html']
        checks['event_exact_original_fixture_identity'] = fixture.stat().st_size == identity['bytes'] and capture_tests.sha256(fixture) == identity['sha256']
        if not checks['event_exact_original_fixture_identity']:
            raise ValueError('Original base Event fixture identity mismatch.')
        script = folder / 'input.tsv'
        script.write_text('snapshot\tbefore\nclick\temit\nsnapshot\tfirst\nclick\temit\nsnapshot\tsecond\n', encoding='utf-8', newline='\n')
        output = folder / 'journey'
        run = capture_tests.execute([str(exe), source, '--dev-input-script', str(script),
            '--dev-input-output', str(output), '--javascript', '--no-profile'], folder / 'command.log', ROOT, 120)
        record['run'] = run
        checks['event_process_completed'] = run['state'] == 'completed' and run['exit_code'] == 0
        for label, expected in [
            ('before', 'Waiting for two real Event activations.'),
            ('first', 'Completed 1 real Event activation(s): constructed and legacy events canceled; child realm verified.'),
            ('second', 'Completed 2 real Event activation(s): constructed and legacy events canceled; child realm verified.'),
        ]:
            matches = list(output.glob('*-' + label + '.activity.txt'))
            if len(matches) != 1:
                raise ValueError('Expected one real base Event snapshot: ' + label)
            activity_path = matches[0]
            prefix = activity_path.name.removesuffix('.activity.txt')
            layout_path = activity_path.with_name(prefix + '.layout.txt')
            source_path = activity_path.with_name(prefix + '.source.html')
            ppm_path = activity_path.with_name(prefix + '.ppm')
            activity = activity_path.read_text(encoding='utf-8')
            layout = layout_path.read_text(encoding='utf-8')
            source = source_path.read_text(encoding='utf-8')
            def stamp(prefix):
                line = next((line for line in activity.splitlines() if line.startswith(prefix)), '')
                return dict(re.findall(r'(\w+)=([^\s]+)', line))
            document, geometry, canvas = (stamp(prefix) for prefix in ['Stage document=', 'Stage layout=', 'Stage canvas='])
            raw = ppm_path.read_bytes().split(b'\n', 3)
            valid_pixels = (len(raw) == 4 and raw[0] == b'P6' and raw[2] == b'255'
                and list(map(int, raw[1].split())) == [int(canvas['width']), int(canvas['height'])]
                and int(canvas['width']) > 0 and int(canvas['height']) > 0
                and len(raw[3]) == int(canvas['width']) * int(canvas['height']) * 3)
            checks['event_' + label + '_paired_document_layout_pixels'] = (
                'Stage paired_ppm_written=1' in activity and valid_pixels
                and all(record.get(key) not in [None, 'unknown'] for record, keys in [
                    (document, ['document', 'dom_commit']),
                    (geometry, ['layout', 'layout_document', 'layout_dom']),
                    (canvas, ['document', 'dom_commit', 'layout', 'layout_document', 'layout_dom', 'page_view'])]
                    for key in keys)
                and document.get('document', 'unknown') != 'unknown'
                and canvas.get('document') == document.get('document') == geometry.get('layout_document')
                and canvas.get('dom_commit') == document.get('dom_commit')
                and canvas.get('layout') == geometry.get('layout') != 'unknown'
                and canvas.get('layout_dom') == geometry.get('layout_dom') == document.get('dom_commit')
                and canvas.get('layout_document') == document.get('document') and canvas.get('page_view') == '1')
            # Status may retain an old summary. Only the anchored interpreter
            # report is authoritative for this snapshot's current error count.
            summaries = re.findall(r'^(\d+) script\(s\), (\d+) active listener\(s\), (\d+) error\(s\)\.$', activity, re.M)
            checks['event_' + label + '_current_interpreter_zero_errors'] = (
                len(summaries) == 1 and int(summaries[0][0]) >= 1 and int(summaries[0][2]) == 0)
            visible = ' '.join(re.findall(r'text="([^"\n]*)"', layout))
            checks['event_' + label + '_actual_visible_result'] = expected in visible
            checks['event_' + label + '_original_guarded_source'] = all(value in source for value in [
                "new C('base-route',{bubbles:true,cancelable:true,composed:true})",
                "Object.getOwnPropertyDescriptor(event,'isTrusted')",
                "getType.call(event)!=='base-route'", "button.dispatchEvent(event)!==false",
                "event.returnValue=false", "first[0]=window", "document.createEvent('Events')",
                "factory.initEvent('factory-route',false,true)", "new ChildEvent('child-'+turns",
                "if(!input.isTrusted||!input.composed", "pathChecks!==turns*5"])
            checks['event_' + label + '_actual_document_title'] = (
                'Page title: ' + ('Original base Event waiting' if label == 'before'
                    else 'Original Event activation ' + ('1' if label == 'first' else '2') + ' passed')) in activity
            if label != 'before':
                count = 1 if label == 'first' else 2
                markers = dict(re.findall(r'\b(constructed|canceled|legacy|child|native|path|brands)=(\d+)', visible))
                checks['event_' + label + '_all_real_semantic_guards_completed_once_per_activation'] = (
                    all(markers.get(name) == str(count) for name in ['constructed', 'canceled', 'legacy', 'child', 'native'])
                    and markers.get('path') == str(count * 5) and markers.get('brands') == '2')
            record['stages'][label] = {'document': document, 'layout': geometry, 'canvas': canvas,
                'interpreter_summaries': [list(value) for value in summaries], 'visible_text': visible,
                'files': {path.name: {'bytes': path.stat().st_size, 'sha256': capture_tests.sha256(path)}
                    for path in [activity_path, layout_path, source_path, ppm_path]}}
        stages = record['stages']
        checks['event_same_actual_document_across_both_activations'] = (
            stages['before']['document']['document'] == stages['first']['document']['document'] == stages['second']['document']['document'])
        checks['event_each_activation_committed_visible_dom_change'] = (
            stages['before']['document']['dom_commit'] != stages['first']['document']['dom_commit']
            and stages['first']['document']['dom_commit'] != stages['second']['document']['dom_commit'])
        def ppm_hash(label):
            return next(value['sha256'] for name, value in stages[label]['files'].items() if name.endswith('.ppm'))
        checks['event_each_completed_activation_changed_actual_pixels'] = len({ppm_hash(label) for label in ['before', 'first', 'second']}) == 3
        checks['event_case_completed'] = True
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        record['error_type'] = type(error).__name__
        record['error'] = str(error)
    record['source_after'] = capture_tests.source_identity(ROOT)
    record['executable_after_sha256'] = capture_tests.sha256(exe)
    checks['event_source_unchanged'] = before == record['source_after']
    checks['event_executable_unchanged'] = executable == record['executable_after_sha256']
    record['passed'] = bool(checks) and all(value is True for value in checks.values())
    return record

def base_event_http_case(exe, folder):
    """Serve only the original base page through the existing owned HTTP seam."""
    import check_network_media_harness as network
    before = capture_tests.source_identity(ROOT)
    executable = capture_tests.sha256(exe)
    base = {'source_before': before, 'source_after': before,
        'executable_sha256': executable, 'executable_after_sha256': executable,
        'source': None, 'trust_file_sha256': None,
        'checks': {'event_http_case_completed': False}, 'passed': False}
    try:
        fixture = ROOT / 'tools/fixtures/base-event.html'
        payload = fixture.read_bytes()
        identity = json.loads((ROOT / 'tools/fixtures/base-event.json').read_text(encoding='utf-8'))['files']['base-event.html']
        if len(payload) != identity['bytes'] or capture_tests.sha256(fixture) != identity['sha256']:
            raise ValueError('Original base Event HTTP payload identity mismatch.')

        def run_authored(*, source, trust_file):
            nonlocal base
            if trust_file is not None:
                raise ValueError('The base Event case uses only owned plain HTTP and no trust override.')
            child = folder / 'authored-event'
            base = base_event_case(exe, child, source=source)
            capture_tests.write_json(child / 'manifest.json', base)
            print('Evidence: ' + str(child))
            return 0 if base['passed'] else 1

        transport = network.run_case('http', folder,
            authored_runner=run_authored,
            served_payloads={'/base-event.html': ('text/html; charset=utf-8', payload)},
            entry_path='/base-event.html',
            route_checks={'base_event_original_document_served': ('/base-event.html', 1)},
            scope='Original genuine base Event two-activation case over owned loopback HTTP; committed web origin for the real child context. No public service or trust override.')
        base['transport'] = transport
        base['checks'].update({'event_http:' + name: value is True
            for name, value in transport['checks'].items()})
        base['checks']['event_http_no_range_or_if_range'] = bool(transport['responses']) and all(
            row['range_requested'] is False and row['if_range_requested'] is False
            for row in transport['responses'])
        base['checks']['event_http_same_source_executable_and_no_trust_override'] = (
            transport['source_before'] == transport['source_after'] == before
            and transport['executable_sha256'] == executable
            and transport['source'] == base['source'] and transport['trust_file_sha256'] is None)
        base['checks']['event_http_case_completed'] = True
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        base['checks']['event_http_case_completed'] = False
        base['transport_error_type'], base['transport_error'] = type(error).__name__, str(error)
    base['passed'] = bool(base['checks']) and all(value is True for value in base['checks'].values())
    return base

def ui_event_case(exe, folder, *, source=None):
    """A fourth owned real App/input process; no nested Evidence output."""
    folder.mkdir()
    before = capture_tests.source_identity(ROOT)
    executable = capture_tests.sha256(exe)
    fixture = ROOT / 'tools/fixtures/ui-event.html'
    source = str(source if source is not None else fixture)
    checks = {'ui_event_case_completed': False, 'ui_event_existing_exact_acceptance': False}
    record = {'scope': 'Original genuine UIEvent dictionary conversion, canonical prototypes, real dispatch/cancellation, native keyboard UI ancestry and child owner through real App input.',
        'source': source, 'trust_file_sha256': None,
        'source_before': before, 'executable_sha256': executable,
        'fixture_sha256': capture_tests.sha256(fixture), 'checks': checks,
        'license_accepted_by_driver': False, 'stages': {}}
    try:
        accept_license.terms()
        checks['ui_event_existing_exact_acceptance'] = accept_license.check(accept_license.directory())
        if not checks['ui_event_existing_exact_acceptance']:
            raise ValueError('Caller-provided exact-term acceptance required; driver never accepts.')
        identity = json.loads((ROOT / 'tools/fixtures/ui-event.json').read_text(encoding='utf-8'))['files']['ui-event.html']
        checks['ui_event_exact_original_fixture_identity'] = fixture.stat().st_size == identity['bytes'] and capture_tests.sha256(fixture) == identity['sha256']
        if not checks['ui_event_exact_original_fixture_identity']:
            raise ValueError('Original UIEvent fixture identity mismatch.')
        script = folder / 'input.tsv'
        script.write_text('snapshot\tbefore\nfill\tkeys\ta\nsnapshot\tkeyboard\nclick\temit\nsnapshot\tfirst\nclick\temit\nsnapshot\tsecond\n', encoding='utf-8', newline='\n')
        output = folder / 'journey'
        run = capture_tests.execute([str(exe), source, '--dev-input-script', str(script),
            '--dev-input-output', str(output), '--javascript', '--no-profile'], folder / 'command.log', ROOT, 120)
        record['run'] = run
        checks['ui_event_process_completed'] = run['state'] == 'completed' and run['exit_code'] == 0
        for label, expected in [
            ('before', 'Waiting for native UI keys and two real activations.'),
            ('keyboard', 'Native UI keyboard down/up verified.'),
            ('first', 'Completed 1 real UIEvent activation(s): ordered construction, cancellation and child owner verified.'),
            ('second', 'Completed 2 real UIEvent activation(s): ordered construction, cancellation and child owner verified.'),
        ]:
            matches = list(output.glob('*-' + label + '.activity.txt'))
            if len(matches) != 1:
                raise ValueError('Expected one real UIEvent snapshot: ' + label)
            activity_path = matches[0]
            prefix = activity_path.name.removesuffix('.activity.txt')
            layout_path = activity_path.with_name(prefix + '.layout.txt')
            source_path = activity_path.with_name(prefix + '.source.html')
            ppm_path = activity_path.with_name(prefix + '.ppm')
            activity = activity_path.read_text(encoding='utf-8')
            layout = layout_path.read_text(encoding='utf-8')
            source = source_path.read_text(encoding='utf-8')
            def stamp(prefix):
                line = next((line for line in activity.splitlines() if line.startswith(prefix)), '')
                return dict(re.findall(r'(\w+)=([^\s]+)', line))
            document, geometry, canvas = (stamp(prefix) for prefix in ['Stage document=', 'Stage layout=', 'Stage canvas='])
            raw = ppm_path.read_bytes().split(b'\n', 3)
            valid_pixels = (len(raw) == 4 and raw[0] == b'P6' and raw[2] == b'255'
                and list(map(int, raw[1].split())) == [int(canvas['width']), int(canvas['height'])]
                and int(canvas['width']) > 0 and int(canvas['height']) > 0
                and len(raw[3]) == int(canvas['width']) * int(canvas['height']) * 3)
            checks['ui_event_' + label + '_paired_document_layout_pixels'] = (
                'Stage paired_ppm_written=1' in activity and valid_pixels
                and all(record.get(key) not in [None, 'unknown'] for record, keys in [
                    (document, ['document', 'dom_commit']),
                    (geometry, ['layout', 'layout_document', 'layout_dom']),
                    (canvas, ['document', 'dom_commit', 'layout', 'layout_document', 'layout_dom', 'page_view'])]
                    for key in keys)
                and document.get('document', 'unknown') != 'unknown'
                and canvas.get('document') == document.get('document') == geometry.get('layout_document')
                and canvas.get('dom_commit') == document.get('dom_commit')
                and canvas.get('layout') == geometry.get('layout') != 'unknown'
                and canvas.get('layout_dom') == geometry.get('layout_dom') == document.get('dom_commit')
                and canvas.get('layout_document') == document.get('document') and canvas.get('page_view') == '1')
            # Status may retain an old summary. Only the anchored interpreter
            # report is authoritative for this snapshot's current error count.
            summaries = re.findall(r'^(\d+) script\(s\), (\d+) active listener\(s\), (\d+) error\(s\)\.$', activity, re.M)
            checks['ui_event_' + label + '_current_interpreter_zero_errors'] = (
                len(summaries) == 1 and int(summaries[0][0]) >= 1 and int(summaries[0][2]) == 0)
            visible = ' '.join(re.findall(r'text="([^"\n]*)"', layout))
            checks['ui_event_' + label + '_actual_visible_result'] = expected in visible
            checks['ui_event_' + label + '_original_guarded_source'] = all(value in source for value in [
                "var C=UIEvent,P=C.prototype", "var event=new C({toString:function(){log+='T';return 'ui-route';}},init)",
                "if(log!=='TBCODN')", "if(log!=='TBCODNV'", "getDetail.call(event)!==1",
                "button.dispatchEvent(event)!==false", "Object.defineProperty(event,'detail'",
                "var factory=document.createEvent('UIEvents')", "P.initUIEvent.call(factory,'ui-factory'",
                "var child=new ChildC('child-ui'", "getView.call(child)!==childWindow",
                "if(!input.isTrusted||!input.composed", "getWhich.call(event)!==65",
                "P.initUIEvent.call(event,'ignored'", "typeof MouseEvent!=='function'", "typeof CustomEvent!=='function'"])
            expected_title = {'before': 'Original UIEvent waiting', 'keyboard': 'Original UIEvent keyboard passed',
                'first': 'Original UIEvent activation 1 passed', 'second': 'Original UIEvent activation 2 passed'}[label]
            checks['ui_event_' + label + '_actual_document_title'] = 'Page title: ' + expected_title in activity
            if label != 'before':
                checks['ui_event_' + label + '_actual_native_keyboard_UI_proof_visible'] = 'Native UI keyboard down/up verified.' in visible
            if label in ['first', 'second']:
                count = 1 if label == 'first' else 2
                markers = dict(re.findall(r'\b(ui|canceled|legacy|child|native|key_down|key_up|order|brands)=([^\s]+)', visible))
                checks['ui_event_' + label + '_all_real_semantic_guards_completed_once_per_activation'] = (
                    all(markers.get(name) == str(count) for name in ['ui', 'canceled', 'legacy', 'child', 'native'])
                    and markers.get('key_down') == markers.get('key_up') == '1'
                    and markers.get('order') == 'TBCODNV' and markers.get('brands') == '2')
            record['stages'][label] = {'document': document, 'layout': geometry, 'canvas': canvas,
                'interpreter_summaries': [list(value) for value in summaries], 'visible_text': visible,
                'files': {path.name: {'bytes': path.stat().st_size, 'sha256': capture_tests.sha256(path)}
                    for path in [activity_path, layout_path, source_path, ppm_path]}}
        stages = record['stages']
        checks['ui_event_same_actual_document_across_both_activations'] = (
            stages['before']['document']['document'] == stages['keyboard']['document']['document'] == stages['first']['document']['document'] == stages['second']['document']['document'])
        checks['ui_event_each_activation_committed_visible_dom_change'] = (
            stages['before']['document']['dom_commit'] != stages['keyboard']['document']['dom_commit']
            and stages['keyboard']['document']['dom_commit'] != stages['first']['document']['dom_commit']
            and stages['first']['document']['dom_commit'] != stages['second']['document']['dom_commit'])
        def ppm_hash(label):
            return next(value['sha256'] for name, value in stages[label]['files'].items() if name.endswith('.ppm'))
        checks['ui_event_each_completed_activation_changed_actual_pixels'] = len({ppm_hash(label) for label in ['before', 'keyboard', 'first', 'second']}) == 4
        checks['ui_event_case_completed'] = True
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        record['error_type'] = type(error).__name__
        record['error'] = str(error)
    record['source_after'] = capture_tests.source_identity(ROOT)
    record['executable_after_sha256'] = capture_tests.sha256(exe)
    checks['ui_event_source_unchanged'] = before == record['source_after']
    checks['ui_event_executable_unchanged'] = executable == record['executable_after_sha256']
    record['passed'] = bool(checks) and all(value is True for value in checks.values())
    return record

def ui_event_http_case(exe, folder):
    """Serve only the original UI page through the existing owned HTTP seam."""
    import check_network_media_harness as network
    before = capture_tests.source_identity(ROOT)
    executable = capture_tests.sha256(exe)
    base = {'source_before': before, 'source_after': before,
        'executable_sha256': executable, 'executable_after_sha256': executable,
        'source': None, 'trust_file_sha256': None,
        'checks': {'ui_event_http_case_completed': False}, 'passed': False}
    try:
        fixture = ROOT / 'tools/fixtures/ui-event.html'
        payload = fixture.read_bytes()
        identity = json.loads((ROOT / 'tools/fixtures/ui-event.json').read_text(encoding='utf-8'))['files']['ui-event.html']
        if len(payload) != identity['bytes'] or capture_tests.sha256(fixture) != identity['sha256']:
            raise ValueError('Original UIEvent HTTP payload identity mismatch.')

        def run_authored(*, source, trust_file):
            nonlocal base
            if trust_file is not None:
                raise ValueError('The UIEvent case uses only owned plain HTTP and no trust override.')
            child = folder / 'authored-event'
            base = ui_event_case(exe, child, source=source)
            capture_tests.write_json(child / 'manifest.json', base)
            print('Evidence: ' + str(child))
            return 0 if base['passed'] else 1

        transport = network.run_case('http', folder,
            authored_runner=run_authored,
            served_payloads={'/ui-event.html': ('text/html; charset=utf-8', payload)},
            entry_path='/ui-event.html',
            route_checks={'ui_event_original_document_served': ('/ui-event.html', 1)},
            scope='Original genuine UIEvent native-keyboard and two-activation case over owned loopback HTTP; committed web origin for the real child context. No public service or trust override.')
        base['transport'] = transport
        base['checks'].update({'ui_event_http:' + name: value is True
            for name, value in transport['checks'].items()})
        base['checks']['ui_event_http_no_range_or_if_range'] = bool(transport['responses']) and all(
            row['range_requested'] is False and row['if_range_requested'] is False
            for row in transport['responses'])
        base['checks']['ui_event_http_same_source_executable_and_no_trust_override'] = (
            transport['source_before'] == transport['source_after'] == before
            and transport['executable_sha256'] == executable
            and transport['source'] == base['source'] and transport['trust_file_sha256'] is None)
        base['checks']['ui_event_http_case_completed'] = True
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        base['checks']['ui_event_http_case_completed'] = False
        base['transport_error_type'], base['transport_error'] = type(error).__name__, str(error)
    base['passed'] = bool(base['checks']) and all(value is True for value in base['checks'].values())
    return base

def mouse_event_case(exe, folder, *, source=None):
    """A fifth owned real App/input process; no nested Evidence output."""
    folder.mkdir()
    before = capture_tests.source_identity(ROOT)
    executable = capture_tests.sha256(exe)
    fixture = ROOT / 'tools/fixtures/mouse-event.html'
    source = str(source if source is not None else fixture)
    checks = {'mouse_event_case_completed': False, 'mouse_event_existing_exact_acceptance': False}
    record = {'scope': 'Original finite MouseEvent constructor/prototype/slots, modifiers and child owner; actual trusted base inputs and explicit synthetic Mouse dispatch refusal.',
        'source': source, 'trust_file_sha256': None,
        'source_before': before, 'executable_sha256': executable,
        'fixture_sha256': capture_tests.sha256(fixture), 'checks': checks,
        'license_accepted_by_driver': False, 'stages': {}}
    try:
        accept_license.terms()
        checks['mouse_event_existing_exact_acceptance'] = accept_license.check(accept_license.directory())
        if not checks['mouse_event_existing_exact_acceptance']:
            raise ValueError('Caller-provided exact-term acceptance required; driver never accepts.')
        identity = json.loads((ROOT / 'tools/fixtures/mouse-event.json').read_text(encoding='utf-8'))['files']['mouse-event.html']
        checks['mouse_event_exact_original_fixture_identity'] = fixture.stat().st_size == identity['bytes'] and capture_tests.sha256(fixture) == identity['sha256']
        if not checks['mouse_event_exact_original_fixture_identity']:
            raise ValueError('Original MouseEvent fixture identity mismatch.')
        script = folder / 'input.tsv'
        script.write_text('snapshot\tbefore\nclick\temit\nsnapshot\tfirst\nclick\temit\nsnapshot\tsecond\n', encoding='utf-8', newline='\n')
        output = folder / 'journey'
        run = capture_tests.execute([str(exe), source, '--dev-input-script', str(script),
            '--dev-input-output', str(output), '--javascript', '--no-profile'], folder / 'command.log', ROOT, 120)
        record['run'] = run
        checks['mouse_event_process_completed'] = run['state'] == 'completed' and run['exit_code'] == 0
        for label, expected in [
            ('before', 'Waiting for two real MouseEvent constructor activations.'),
            ('first', 'Completed 1 real MouseEvent constructor activation(s): slots and child owner verified; synthetic dispatch refused.'),
            ('second', 'Completed 2 real MouseEvent constructor activation(s): slots and child owner verified; synthetic dispatch refused.'),
        ]:
            matches = list(output.glob('*-' + label + '.activity.txt'))
            if len(matches) != 1:
                raise ValueError('Expected one real MouseEvent snapshot: ' + label)
            activity_path = matches[0]
            prefix = activity_path.name.removesuffix('.activity.txt')
            layout_path = activity_path.with_name(prefix + '.layout.txt')
            source_path = activity_path.with_name(prefix + '.source.html')
            ppm_path = activity_path.with_name(prefix + '.ppm')
            activity = activity_path.read_text(encoding='utf-8')
            layout = layout_path.read_text(encoding='utf-8')
            source = source_path.read_text(encoding='utf-8')
            def stamp(prefix):
                line = next((line for line in activity.splitlines() if line.startswith(prefix)), '')
                return dict(re.findall(r'(\w+)=([^\s]+)', line))
            document, geometry, canvas = (stamp(prefix) for prefix in ['Stage document=', 'Stage layout=', 'Stage canvas='])
            raw = ppm_path.read_bytes().split(b'\n', 3)
            valid_pixels = (len(raw) == 4 and raw[0] == b'P6' and raw[2] == b'255'
                and list(map(int, raw[1].split())) == [int(canvas['width']), int(canvas['height'])]
                and int(canvas['width']) > 0 and int(canvas['height']) > 0
                and len(raw[3]) == int(canvas['width']) * int(canvas['height']) * 3)
            checks['mouse_event_' + label + '_paired_document_layout_pixels'] = (
                'Stage paired_ppm_written=1' in activity and valid_pixels
                and all(record.get(key) not in [None, 'unknown'] for record, keys in [
                    (document, ['document', 'dom_commit']),
                    (geometry, ['layout', 'layout_document', 'layout_dom']),
                    (canvas, ['document', 'dom_commit', 'layout', 'layout_document', 'layout_dom', 'page_view'])]
                    for key in keys)
                and document.get('document', 'unknown') != 'unknown'
                and canvas.get('document') == document.get('document') == geometry.get('layout_document')
                and canvas.get('dom_commit') == document.get('dom_commit')
                and canvas.get('layout') == geometry.get('layout') != 'unknown'
                and canvas.get('layout_dom') == geometry.get('layout_dom') == document.get('dom_commit')
                and canvas.get('layout_document') == document.get('document') and canvas.get('page_view') == '1')
            # Status may retain an old summary. Only the anchored interpreter
            # report is authoritative for this snapshot's current error count.
            summaries = re.findall(r'^(\d+) script\(s\), (\d+) active listener\(s\), (\d+) error\(s\)\.$', activity, re.M)
            checks['mouse_event_' + label + '_current_interpreter_zero_errors'] = (
                len(summaries) == 1 and int(summaries[0][0]) >= 1 and int(summaries[0][2]) == 0)
            visible = ' '.join(re.findall(r'text="([^"\n]*)"', layout))
            checks['mouse_event_' + label + '_actual_visible_result'] = expected in visible
            checks['mouse_event_' + label + '_original_guarded_source'] = all(value in source for value in [
                "var C=MouseEvent,P=C.prototype", "var event=new C({toString:function(){order+='T';return 'click';}},options)",
                "if(order!=='TDBUX')", "if(order!=='TDBUXRS'", "event.screenX!==7.5", "event.buttons!==5",
                "for(var name of modifierNames)", "event.getModifierState(name)", "getClient.call(event)!==1.5",
                "getClient.call(input)", "input instanceof C", "error.name==='NotSupportedError'",
                "event.target!==null", "P.initMouseEvent.call(event,'legacy-mouse'",
                "var factory=document.createEvent('MouseEvents')", "var child=new ChildC('child-mouse'",
                "getView.call(child)!==childWindow", "typeof KeyboardEvent!=='undefined'", "typeof CustomEvent!=='function'"])
            title = 'Original MouseEvent waiting' if label == 'before' else 'Original MouseEvent activation ' + ('1' if label == 'first' else '2') + ' passed'
            checks['mouse_event_' + label + '_actual_document_title'] = 'Page title: ' + title in activity
            if label in ['first', 'second']:
                count = 1 if label == 'first' else 2
                markers = dict(re.findall(r'\b(mouse|double|modifiers|refused|native|native_brand|legacy|child|which|order|brands)=([^\s]+)', visible))
                checks['mouse_event_' + label + '_real_slots_owner_and_refusals_verified_once_per_input'] = (
                    all(markers.get(name) == str(count) for name in ['mouse', 'double', 'native', 'native_brand', 'legacy', 'child'])
                    and markers.get('refused') == str(count * 2) and markers.get('modifiers') == '14'
                    and markers.get('which') == '3' and markers.get('order') == 'TDBUXRS' and markers.get('brands') == '2')
            record['stages'][label] = {'document': document, 'layout': geometry, 'canvas': canvas,
                'interpreter_summaries': [list(value) for value in summaries], 'visible_text': visible,
                'files': {path.name: {'bytes': path.stat().st_size, 'sha256': capture_tests.sha256(path)}
                    for path in [activity_path, layout_path, source_path, ppm_path]}}
        stages = record['stages']
        checks['mouse_event_same_actual_document_across_both_activations'] = (
            stages['before']['document']['document'] == stages['first']['document']['document'] == stages['second']['document']['document'])
        checks['mouse_event_each_activation_committed_visible_dom_change'] = (
            stages['before']['document']['dom_commit'] != stages['first']['document']['dom_commit']
            and stages['first']['document']['dom_commit'] != stages['second']['document']['dom_commit'])
        def ppm_hash(label):
            return next(value['sha256'] for name, value in stages[label]['files'].items() if name.endswith('.ppm'))
        checks['mouse_event_each_completed_activation_changed_actual_pixels'] = len({ppm_hash(label) for label in ['before', 'first', 'second']}) == 3
        checks['mouse_event_case_completed'] = True
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        record['error_type'] = type(error).__name__
        record['error'] = str(error)
    record['source_after'] = capture_tests.source_identity(ROOT)
    record['executable_after_sha256'] = capture_tests.sha256(exe)
    checks['mouse_event_source_unchanged'] = before == record['source_after']
    checks['mouse_event_executable_unchanged'] = executable == record['executable_after_sha256']
    record['passed'] = bool(checks) and all(value is True for value in checks.values())
    return record

def mouse_event_http_case(exe, folder):
    """Serve only the original Mouse page through the existing owned HTTP seam."""
    import check_network_media_harness as network
    before = capture_tests.source_identity(ROOT)
    executable = capture_tests.sha256(exe)
    base = {'source_before': before, 'source_after': before,
        'executable_sha256': executable, 'executable_after_sha256': executable,
        'source': None, 'trust_file_sha256': None,
        'checks': {'mouse_event_http_case_completed': False}, 'passed': False}
    try:
        fixture = ROOT / 'tools/fixtures/mouse-event.html'
        payload = fixture.read_bytes()
        identity = json.loads((ROOT / 'tools/fixtures/mouse-event.json').read_text(encoding='utf-8'))['files']['mouse-event.html']
        if len(payload) != identity['bytes'] or capture_tests.sha256(fixture) != identity['sha256']:
            raise ValueError('Original MouseEvent HTTP payload identity mismatch.')

        def run_authored(*, source, trust_file):
            nonlocal base
            if trust_file is not None:
                raise ValueError('The MouseEvent case uses only owned plain HTTP and no trust override.')
            child = folder / 'authored-event'
            base = mouse_event_case(exe, child, source=source)
            capture_tests.write_json(child / 'manifest.json', base)
            print('Evidence: ' + str(child))
            return 0 if base['passed'] else 1

        transport = network.run_case('http', folder,
            authored_runner=run_authored,
            served_payloads={'/mouse-event.html': ('text/html; charset=utf-8', payload)},
            entry_path='/mouse-event.html',
            route_checks={'mouse_event_original_document_served': ('/mouse-event.html', 1)},
            scope='Original finite MouseEvent constructor and dispatch-refusal two-activation case over owned loopback HTTP; committed web origin for the real child context. No public service or trust override.')
        base['transport'] = transport
        base['checks'].update({'mouse_event_http:' + name: value is True
            for name, value in transport['checks'].items()})
        base['checks']['mouse_event_http_no_range_or_if_range'] = bool(transport['responses']) and all(
            row['range_requested'] is False and row['if_range_requested'] is False
            for row in transport['responses'])
        base['checks']['mouse_event_http_same_source_executable_and_no_trust_override'] = (
            transport['source_before'] == transport['source_after'] == before
            and transport['executable_sha256'] == executable
            and transport['source'] == base['source'] and transport['trust_file_sha256'] is None)
        base['checks']['mouse_event_http_case_completed'] = True
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        base['checks']['mouse_event_http_case_completed'] = False
        base['transport_error_type'], base['transport_error'] = type(error).__name__, str(error)
    base['passed'] = bool(base['checks']) and all(value is True for value in base['checks'].values())
    return base

def custom_event_interface_case(exe, folder, *, source=None):
    """A sixth owned real App/input process; no nested Evidence output."""
    folder.mkdir()
    before = capture_tests.source_identity(ROOT)
    executable = capture_tests.sha256(exe)
    fixture = ROOT / 'tools/fixtures/custom-event-interface.html'
    source = str(source if source is not None else fixture)
    checks = {'custom_interface_case_completed': False, 'custom_interface_existing_exact_acceptance': False}
    record = {'scope': 'Original genuine CustomEvent interface constructor/descriptors/brands, stored-slot dispatch and cancellation, path copies, legacy factory and child realm through real App input.',
        'source': source, 'trust_file_sha256': None,
        'source_before': before, 'executable_sha256': executable,
        'fixture_sha256': capture_tests.sha256(fixture), 'checks': checks,
        'license_accepted_by_driver': False, 'stages': {}}
    try:
        accept_license.terms()
        checks['custom_interface_existing_exact_acceptance'] = accept_license.check(accept_license.directory())
        if not checks['custom_interface_existing_exact_acceptance']:
            raise ValueError('Caller-provided exact-term acceptance required; driver never accepts.')
        identity = json.loads((ROOT / 'tools/fixtures/custom-event-interface.json').read_text(encoding='utf-8'))['files']['custom-event-interface.html']
        checks['custom_interface_exact_original_fixture_identity'] = fixture.stat().st_size == identity['bytes'] and capture_tests.sha256(fixture) == identity['sha256']
        if not checks['custom_interface_exact_original_fixture_identity']:
            raise ValueError('Original CustomEvent interface fixture identity mismatch.')
        script = folder / 'input.tsv'
        script.write_text('snapshot\tbefore\nclick\temit\nsnapshot\tfirst\nclick\temit\nsnapshot\tsecond\n', encoding='utf-8', newline='\n')
        output = folder / 'journey'
        run = capture_tests.execute([str(exe), source, '--dev-input-script', str(script),
            '--dev-input-output', str(output), '--javascript', '--no-profile'], folder / 'command.log', ROOT, 120)
        record['run'] = run
        checks['custom_interface_process_completed'] = run['state'] == 'completed' and run['exit_code'] == 0
        for label, expected in [
            ('before', 'Waiting for two real CustomEvent constructor activations.'),
            ('first', 'Completed 1 real CustomEvent activation(s): constructed and legacy events canceled; child detail verified.'),
            ('second', 'Completed 2 real CustomEvent activation(s): constructed and legacy events canceled; child detail verified.'),
        ]:
            matches = list(output.glob('*-' + label + '.activity.txt'))
            if len(matches) != 1:
                raise ValueError('Expected one real CustomEvent interface snapshot: ' + label)
            activity_path = matches[0]
            prefix = activity_path.name.removesuffix('.activity.txt')
            layout_path = activity_path.with_name(prefix + '.layout.txt')
            source_path = activity_path.with_name(prefix + '.source.html')
            ppm_path = activity_path.with_name(prefix + '.ppm')
            activity = activity_path.read_text(encoding='utf-8')
            layout = layout_path.read_text(encoding='utf-8')
            source = source_path.read_text(encoding='utf-8')
            def stamp(prefix):
                line = next((line for line in activity.splitlines() if line.startswith(prefix)), '')
                return dict(re.findall(r'(\w+)=([^\s]+)', line))
            document, geometry, canvas = (stamp(prefix) for prefix in ['Stage document=', 'Stage layout=', 'Stage canvas='])
            raw = ppm_path.read_bytes().split(b'\n', 3)
            valid_pixels = (len(raw) == 4 and raw[0] == b'P6' and raw[2] == b'255'
                and list(map(int, raw[1].split())) == [int(canvas['width']), int(canvas['height'])]
                and int(canvas['width']) > 0 and int(canvas['height']) > 0
                and len(raw[3]) == int(canvas['width']) * int(canvas['height']) * 3)
            checks['custom_interface_' + label + '_paired_document_layout_pixels'] = (
                'Stage paired_ppm_written=1' in activity and valid_pixels
                and all(record.get(key) not in [None, 'unknown'] for record, keys in [
                    (document, ['document', 'dom_commit']),
                    (geometry, ['layout', 'layout_document', 'layout_dom']),
                    (canvas, ['document', 'dom_commit', 'layout', 'layout_document', 'layout_dom', 'page_view'])]
                    for key in keys)
                and document.get('document', 'unknown') != 'unknown'
                and canvas.get('document') == document.get('document') == geometry.get('layout_document')
                and canvas.get('dom_commit') == document.get('dom_commit')
                and canvas.get('layout') == geometry.get('layout') != 'unknown'
                and canvas.get('layout_dom') == geometry.get('layout_dom') == document.get('dom_commit')
                and canvas.get('layout_document') == document.get('document') and canvas.get('page_view') == '1')
            # Status may retain an old summary. Only the anchored interpreter
            # report is authoritative for this snapshot's current error count.
            summaries = re.findall(r'^(\d+) script\(s\), (\d+) active listener\(s\), (\d+) error\(s\)\.$', activity, re.M)
            checks['custom_interface_' + label + '_current_interpreter_zero_errors'] = (
                len(summaries) == 1 and int(summaries[0][0]) >= 1 and int(summaries[0][2]) == 0)
            visible = ' '.join(re.findall(r'text="([^"\n]*)"', layout))
            checks['custom_interface_' + label + '_audio_owner_closed'] = 'Audio owner opened=false retained_buffers=0 busy=false' in activity
            checks['custom_interface_' + label + '_actual_visible_result'] = expected in visible
            checks['custom_interface_' + label + '_original_guarded_source'] = all(value in source for value in [
                "var C=CustomEvent,P=C.prototype", "current=new C({toString:function(){order+='T';return 'constructor-route';}},options)",
                "if(order!=='TBCOD'", "current.detail!==lastPayload", "getDetail.call(current)!==lastPayload",
                "button.dispatchEvent(current)!==false", "P.initCustomEvent.call(event,{toString:function()",
                "Event.prototype.initEvent.call(current,'base-reset')", "publishOriginal('original-route',lastPayload)",
                "event.initCustomEvent(name,true,true,detail)", "if(accepted||!lastPayload.observed)",
                "var child=new ChildC('child-'+turns,{detail:childPayload})", "child.detail!==childPayload",
                "if(!input.isTrusted||!input.composed", "pathChecks!==turns*5"])
            checks['custom_interface_' + label + '_actual_document_title'] = (
                'Page title: ' + ('Original CustomEvent waiting' if label == 'before'
                    else 'Original CustomEvent activation ' + ('1' if label == 'first' else '2') + ' passed')) in activity
            if label != 'before':
                count = 1 if label == 'first' else 2
                markers = dict(re.findall(r'\b(constructed|canceled|legacy|child|native|path|brands|reinitialized)=(\d+)', visible))
                checks['custom_interface_' + label + '_all_real_semantic_guards_completed_once_per_activation'] = (
                    all(markers.get(name) == str(count) for name in ['constructed', 'canceled', 'legacy', 'child', 'native', 'reinitialized'])
                    and markers.get('path') == str(count * 5) and markers.get('brands') == '2'
                    and 'order=TBCOD' in visible)
            record['stages'][label] = {'document': document, 'layout': geometry, 'canvas': canvas,
                'interpreter_summaries': [list(value) for value in summaries], 'visible_text': visible,
                'files': {path.name: {'bytes': path.stat().st_size, 'sha256': capture_tests.sha256(path)}
                    for path in [activity_path, layout_path, source_path, ppm_path]}}
        stages = record['stages']
        checks['custom_interface_same_actual_document_across_both_activations'] = (
            stages['before']['document']['document'] == stages['first']['document']['document'] == stages['second']['document']['document'])
        checks['custom_interface_each_activation_committed_visible_dom_change'] = (
            stages['before']['document']['dom_commit'] != stages['first']['document']['dom_commit']
            and stages['first']['document']['dom_commit'] != stages['second']['document']['dom_commit'])
        def ppm_hash(label):
            return next(value['sha256'] for name, value in stages[label]['files'].items() if name.endswith('.ppm'))
        checks['custom_interface_each_completed_activation_changed_actual_pixels'] = len({ppm_hash(label) for label in ['before', 'first', 'second']}) == 3
        checks['custom_interface_case_completed'] = True
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        record['error_type'] = type(error).__name__
        record['error'] = str(error)
    record['source_after'] = capture_tests.source_identity(ROOT)
    record['executable_after_sha256'] = capture_tests.sha256(exe)
    checks['custom_interface_source_unchanged'] = before == record['source_after']
    checks['custom_interface_executable_unchanged'] = executable == record['executable_after_sha256']
    record['passed'] = bool(checks) and all(value is True for value in checks.values())
    return record

def custom_event_interface_http_case(exe, folder):
    """Serve only the original base page through the existing owned HTTP seam."""
    import check_network_media_harness as network
    before = capture_tests.source_identity(ROOT)
    executable = capture_tests.sha256(exe)
    base = {'source_before': before, 'source_after': before,
        'executable_sha256': executable, 'executable_after_sha256': executable,
        'source': None, 'trust_file_sha256': None,
        'checks': {'custom_interface_http_case_completed': False}, 'passed': False}
    try:
        fixture = ROOT / 'tools/fixtures/custom-event-interface.html'
        payload = fixture.read_bytes()
        identity = json.loads((ROOT / 'tools/fixtures/custom-event-interface.json').read_text(encoding='utf-8'))['files']['custom-event-interface.html']
        if len(payload) != identity['bytes'] or capture_tests.sha256(fixture) != identity['sha256']:
            raise ValueError('Original CustomEvent interface HTTP payload identity mismatch.')

        def run_authored(*, source, trust_file):
            nonlocal base
            if trust_file is not None:
                raise ValueError('The CustomEvent interface case uses only owned plain HTTP and no trust override.')
            child = folder / 'authored-event'
            base = custom_event_interface_case(exe, child, source=source)
            capture_tests.write_json(child / 'manifest.json', base)
            print('Evidence: ' + str(child))
            return 0 if base['passed'] else 1

        transport = network.run_case('http', folder,
            authored_runner=run_authored,
            served_payloads={'/custom-event-interface.html': ('text/html; charset=utf-8', payload)},
            entry_path='/custom-event-interface.html',
            route_checks={'base_custom_interface_original_document_served': ('/custom-event-interface.html', 1)},
            scope='Original genuine CustomEvent constructor/detail/dispatch two-activation case over owned loopback HTTP; committed web origin for the real child context. No public service or trust override.')
        base['transport'] = transport
        base['checks'].update({'custom_interface_http:' + name: value is True
            for name, value in transport['checks'].items()})
        base['checks']['custom_interface_http_no_range_or_if_range'] = bool(transport['responses']) and all(
            row['range_requested'] is False and row['if_range_requested'] is False
            for row in transport['responses'])
        base['checks']['custom_interface_http_same_source_executable_and_no_trust_override'] = (
            transport['source_before'] == transport['source_after'] == before
            and transport['executable_sha256'] == executable
            and transport['source'] == base['source'] and transport['trust_file_sha256'] is None)
        base['checks']['custom_interface_http_case_completed'] = True
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        base['checks']['custom_interface_http_case_completed'] = False
        base['transport_error_type'], base['transport_error'] = type(error).__name__, str(error)
    base['passed'] = bool(base['checks']) and all(value is True for value in base['checks'].values())
    return base

def main():
    folder=ROOT/'results/verification/autonomy'/(datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-dispatch-journey-'+uuid.uuid4().hex[:8]);folder.mkdir(parents=True)
    script=folder/'input.tsv';script.write_text('click\tinterface\nsnapshot\tinterface\nclick\ttarget\nsnapshot\ttarget\nclick\tdeep\nsnapshot\tguarded\nclick\trecover\nsnapshot\trecovered\n',encoding='utf-8');out=folder/'journey'
    before=capture_tests.source_identity(ROOT);exe=ROOT/'ChezCat.exe';executable_before=capture_tests.sha256(exe);data=exe.read_bytes();pe=struct.unpack_from('<I',data,60)[0];reserve,commit=struct.unpack_from('<QQ',data,pe+24+72)
    row=capture_tests.execute([str(exe),str(ROOT/'tools/fixtures/dispatch-stack.html'),'--dev-input-script',str(script),'--dev-input-output',str(out),'--javascript','--no-profile'],folder/'command.log',ROOT,120)
    def read(name,suffix):
        file=next(out.glob('*-'+name+'.'+suffix),None);return file.read_text(encoding='utf-8') if file else ''
    guarded=read('guarded','activity.txt');recovered=read('recovered','activity.txt');layout=read('recovered','layout.txt')
    target=read('target','activity.txt');target_layout=read('target','layout.txt')
    interface=read('interface','activity.txt');interface_layout=read('interface','layout.txt')
    checks={'process_survived_nested_guard':row['state']=='completed','measured_native_reserve':reserve==8*1024*1024 and 0<commit<reserve,'guard_reached':'Function call limit reached' in guarded or 'evaluation nesting' in guarded,'continued_real_input_recovered':'Page title: Nested dispatch recovered' in recovered,'real_recovered_text_rendered':all(('text="'+word+'"') in layout for word in ['Nested','dispatch','recovered']),'paired_pixels_written':'Stage paired_ppm_written=1' in guarded and 'Stage paired_ppm_written=1' in recovered,'source_unchanged':before==capture_tests.source_identity(ROOT)}
    checks.update({'standalone_target_real_dispatch':'Page title: Standalone EventTarget delivered' in target,'standalone_target_text_rendered':all(('text="'+word+'"') in target_layout for word in ['Standalone','EventTarget','delivered']),'standalone_target_pixels_written':'Stage paired_ppm_written=1' in target})
    checks.update({'element_prototype_real_extension':'Page title: Element prototype extension delivered' in interface,'element_prototype_text_rendered':all(('text="'+word+'"') in interface_layout for word in ['Element','prototype','extension','delivered']),'element_prototype_pixels_written':'Stage paired_ppm_written=1' in interface})
    custom=custom_event_case(exe,folder/'custom-event')
    checks.update(custom['checks'])
    base=base_event_http_case(exe,folder/'base-event')
    checks.update(base['checks'])
    ui=ui_event_http_case(exe,folder/'ui-event')
    checks.update(ui['checks'])
    mouse=mouse_event_http_case(exe,folder/'mouse-event')
    checks.update(mouse['checks'])
    custom_interface=custom_event_interface_http_case(exe,folder/'custom-event-interface')
    checks.update(custom_interface['checks'])
    after=capture_tests.source_identity(ROOT)
    checks['both_cases_same_source_identity']=before==after==custom['source_before']==custom['source_after']
    checks['both_cases_same_executable_identity']=executable_before==capture_tests.sha256(exe)==custom['executable_sha256']==custom['executable_after_sha256']
    checks['all_three_cases_same_source_identity']=before==after==custom['source_before']==custom['source_after']==base['source_before']==base['source_after']
    checks['all_three_cases_same_executable_identity']=executable_before==capture_tests.sha256(exe)==custom['executable_sha256']==custom['executable_after_sha256']==base['executable_sha256']==base['executable_after_sha256']
    checks['all_four_cases_same_source_identity']=before==after==custom['source_before']==custom['source_after']==base['source_before']==base['source_after']==ui['source_before']==ui['source_after']
    checks['all_four_cases_same_executable_identity']=executable_before==capture_tests.sha256(exe)==custom['executable_sha256']==custom['executable_after_sha256']==base['executable_sha256']==base['executable_after_sha256']==ui['executable_sha256']==ui['executable_after_sha256']
    checks['all_five_cases_same_source_identity']=before==after==custom['source_before']==custom['source_after']==base['source_before']==base['source_after']==ui['source_before']==ui['source_after']==mouse['source_before']==mouse['source_after']
    checks['all_five_cases_same_executable_identity']=executable_before==capture_tests.sha256(exe)==custom['executable_sha256']==custom['executable_after_sha256']==base['executable_sha256']==base['executable_after_sha256']==ui['executable_sha256']==ui['executable_after_sha256']==mouse['executable_sha256']==mouse['executable_after_sha256']
    checks['all_six_cases_same_source_identity']=checks['all_five_cases_same_source_identity'] and before==after==custom_interface['source_before']==custom_interface['source_after']
    checks['all_six_cases_same_executable_identity']=checks['all_five_cases_same_executable_identity'] and executable_before==custom_interface['executable_sha256']==custom_interface['executable_after_sha256']
    capture_tests.write_json(folder/'manifest.json',{'scope':__doc__,'passed':all(checks.values()),'source_before':before,'source_after':after,'exe_sha256':executable_before,'executable_sha256':executable_before,'stack_reserve_bytes':reserve,'stack_commit_bytes':commit,'run':row,'custom_event':custom,'base_event':base,'ui_event':ui,'mouse_event':mouse,'custom_event_interface':custom_interface,'checks':checks});print(json.dumps(checks,indent=2));print('Evidence: '+str(folder));return 0 if all(checks.values()) else 1
if __name__=='__main__':raise SystemExit(main())
