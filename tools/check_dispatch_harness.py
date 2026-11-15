"""Offline executable check of synchronous dispatch stack guard and recovery.

Also checks the original legacy CustomEvent detail/cancellation fixture through
two real button activations in a second App/input process. No desktop input or
public request; the driver never creates license acceptance.
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
    after=capture_tests.source_identity(ROOT)
    checks['both_cases_same_source_identity']=before==after==custom['source_before']==custom['source_after']
    checks['both_cases_same_executable_identity']=executable_before==capture_tests.sha256(exe)==custom['executable_sha256']==custom['executable_after_sha256']
    capture_tests.write_json(folder/'manifest.json',{'scope':__doc__,'passed':all(checks.values()),'source_before':before,'source_after':after,'exe_sha256':executable_before,'executable_sha256':executable_before,'stack_reserve_bytes':reserve,'stack_commit_bytes':commit,'run':row,'custom_event':custom,'checks':checks});print(json.dumps(checks,indent=2));print('Evidence: '+str(folder));return 0 if all(checks.values()) else 1
if __name__=='__main__':raise SystemExit(main())
