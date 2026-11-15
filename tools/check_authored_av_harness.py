"""Original finite authored A/V App/input/offscreen journey.

Owned offscreen App/input/renderer only. Original multiplexed MKV metadata,
paused seek/sample selection, independent displayed pixels, same-element IVF
replacement and MKV reselection. The default source is the retained local page;
a caller may provide controlled loopback HTTP(S) and process-only test trust.
Native Play runs ONLY for the retained IVF regression. No device, physical
input, automatic license acceptance, build, public service, advancing PCM or
physical audibility claim.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import sys
import uuid

ROOT = Path(__file__).resolve().parents[1]
LABELS = ['mkv-ready', 'mkv-seek', 'mkv-frozen', 'mkv-eof-selected', 'mkv-eof-frozen',
          'ivf-ready', 'ivf-paused', 'ivf-seek', 'ivf-frozen',
          'mkv-restored', 'mkv-restored-seek', 'mkv-restored-frozen', 'navigated']
MEDIA = re.compile(r'Media playing=(\w+) ended=(\w+) time_ms=(\d+) duration_ms=(\d+) frame=(\d+) frames=(\d+) pending=(.*?) canvas=(\d+)x(\d+) generation=(\d+) error=(.*)')
AUDIO = re.compile(r'Audio playing=(\w+) ended=(\w+) pending=(\w+) time_ms=(\d+) duration_ms=(\d+) frame=(\d+) frames=(\d+) generation=(\d+) error=(.*)')
DEVICE = re.compile(r'Audio device opened=(\w+) queued=(\d+) submitted=(\d+) retired=(\d+) position_unit=(.*?) retained_buffers=(\d+)')
BINDING = re.compile(r'HTML video binding node=(\d+) ready_state=(\d+) network_state=(\d+) width=(\d+) height=(\d+) paused=(\w+) ended=(\w+) seeking=(\w+) current_ms=(\d+) duration_ms=Some\((\d+)\) source_pending=(\w+) error=(.*)')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def fields(text, prefix):
    line = next((line for line in text.splitlines() if line.startswith(prefix)), '')
    return dict(re.findall(r'(\w+)=([^\s]+)', line))


def snapshot(output, label):
    paths = list(output.glob('*-' + label + '.activity.txt'))
    if len(paths) != 1:
        raise ValueError('Expected one actual snapshot: ' + label)
    path = paths[0]
    stem = path.name.removesuffix('.activity.txt')
    activity = path.read_text(encoding='utf-8')
    layout = path.with_name(stem + '.layout.txt').read_text(encoding='utf-8')
    source = path.with_name(stem + '.source.html').read_text(encoding='utf-8')
    ppm_bytes = path.with_name(stem + '.ppm').read_bytes()
    ppm = ppm_bytes.split(b'\n', 3)
    if len(ppm) != 4 or ppm[0] != b'P6' or ppm[2] != b'255':
        raise ValueError('Expected actual paired P6 pixels.')
    width, height = map(int, ppm[1].split())
    if width <= 0 or height <= 0 or width * height > 64 * 1024 * 1024 or len(ppm[3]) != width * height * 3:
        raise ValueError('Invalid bounded actual pixel extent.')
    visible = ' '.join(re.findall(r'text="([^"\n]*)"', layout))
    markers = dict(re.findall(r'\b(mode|selected|errors|metadata|ready|network|paused|ended|current_ms|duration_ms|raw|play_events|playing_events|ended_events|seek_done|seek_target_ms|seek_order)=([^\s]+)', visible))
    document, geometry, canvas = (fields(activity, prefix) for prefix in ['Stage document=', 'Stage layout=', 'Stage canvas='])
    paired = ('Stage paired_ppm_written=1' in activity and document.get('document', 'unknown') != 'unknown'
        and canvas.get('document') == document.get('document') == geometry.get('layout_document')
        and canvas.get('dom_commit') == document.get('dom_commit') and canvas.get('layout') == geometry.get('layout') != 'unknown'
        and canvas.get('layout_dom') == geometry.get('layout_dom') and canvas.get('page_view') == '1')
    return {'activity': activity, 'markers': markers, 'paired': paired, 'source': source,
        'document': document, 'geometry': geometry, 'canvas': canvas,
        'width': width, 'height': height, 'pixels': ppm[3], 'ppm_sha256': digest(ppm_bytes)}


def crop(stage, rect):
    x, y, width, height = rect
    if x < 0 or y < 0 or width <= 0 or height <= 0 or x + width > stage['width'] or y + height > stage['height']:
        raise ValueError('Original frame is outside actual paired surface.')
    return b''.join(stage['pixels'][((y + row) * stage['width'] + x) * 3:
                                   ((y + row) * stage['width'] + x + width) * 3]
                    for row in range(height))


def frame_at(clip, milliseconds):
    return max(index for index, frame in enumerate(clip['frames'])
               if frame['timestamp'] * clip['scale'] * 1000 <= milliseconds * clip['rate'])


def main(*, source=None, trust_file=None, argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=ROOT)
    parser.add_argument('--exe', type=Path)
    args = parser.parse_args(argv)
    repo = args.repo.resolve()
    exe = (args.exe or repo / 'ChezCat.exe').resolve()
    fixtures = repo / 'tools/fixtures'
    source = str(source if source is not None else fixtures / 'authored-av.html')
    sys.path.insert(0, str(repo / 'tools'))
    import accept_license
    import capture_tests
    folder = repo / 'results/verification/autonomy' / (datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ') + '-authored-av-' + uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    before = capture_tests.source_identity(repo)
    exe_sha = capture_tests.sha256(exe)
    checks = {'existing_exact_acceptance': False, 'actual_journey_completed': False}
    observations, snapshots = {}, {}
    record = {'scope': __doc__, 'source': source,
        'trust_file_sha256': capture_tests.sha256(trust_file) if trust_file is not None else None,
        'service_acceptance_claimed': False, 'device_attempt': False,
        'advancing_acknowledged_pcm_playback_verified': False, 'physical_audibility_verified': False,
        'decoded_pcm_bytes_verified_by_this_journey': False,
        'license_accepted_by_driver': False, 'public_requests_by_driver': 0,
        'source_before': before, 'executable_sha256': exe_sha, 'checks': checks,
        'observations': observations}
    try:
        accept_license.terms()
        checks['existing_exact_acceptance'] = accept_license.check(accept_license.directory())
        if not checks['existing_exact_acceptance']:
            raise ValueError('Existing explicit exact-term acceptance required; driver never accepts.')
        fixture_manifest = json.loads((fixtures / 'authored-av.json').read_text(encoding='utf-8'))
        for name, expected in fixture_manifest['files'].items():
            if Path(name).name != name:
                raise ValueError('Fixture name must be an immediate original file.')
            data = (fixtures / name).read_bytes()
            if len(data) != expected['bytes'] or digest(data) != expected['sha256']:
                raise ValueError('Original fixture identity mismatch: ' + name)
        checks['exact_original_fixture_hashes'] = True
        clip = json.loads((fixtures / 'scene-inter.json').read_text(encoding='utf-8'))
        av = json.loads((fixtures / 'scene-inter-av.json').read_text(encoding='utf-8'))
        profile = json.loads((fixtures / 'scene-inter-display.json').read_text(encoding='utf-8'))
        reference = (fixtures / 'scene-inter-centered.rgba').read_bytes()
        if (digest(reference) != profile['reference_rgba_sha256'] or len(reference) != 64 * 48 * 4 * 24
                or profile['pixel_tolerance'] != {'mean_rgb_error_less_than': .5, 'maximum_channel_error_at_most': 2}
                or (clip['width'], clip['height'], len(clip['frames']), clip['rate'], clip['scale']) != (64, 48, 24, 8, 1)
                or (av['sample_rate'], av['pcm_frames'], av['duration_ms']) != (48000, 144000, 3000)
                or capture_tests.sha256(fixtures / 'scene-inter-av.mkv') != av['mkv_sha256']):
            raise ValueError('Exact original codec/display/PCM profile identity mismatch.')
        checks['original_independent_display_and_pcm_profile'] = True
        actions = ('wait\t750\nsnapshot\tmkv-ready\nclick\tseek\nwait\t750\nsnapshot\tmkv-seek\n'
            'wait\t350\nsnapshot\tmkv-frozen\nclick\tselect-eof\nwait\t750\nsnapshot\tmkv-eof-selected\n'
            'wait\t350\nsnapshot\tmkv-eof-frozen\nclick\tselect-ivf\nwait\t750\nsnapshot\tivf-ready\n'
            'media\tPlay\nwait\t450\nmedia\tPause\nwait\t400\nsnapshot\tivf-paused\n'
            'click\tseek\nwait\t750\nsnapshot\tivf-seek\nwait\t350\nsnapshot\tivf-frozen\n'
            'click\tselect-mkv\nwait\t750\nsnapshot\tmkv-restored\nclick\tseek\nwait\t750\n'
            'snapshot\tmkv-restored-seek\nwait\t350\nsnapshot\tmkv-restored-frozen\nlink\tLeave\nsnapshot\tnavigated\n')
        if len(actions.splitlines()) > 64:
            raise ValueError('Owned input action budget exceeded.')
        script = folder / 'input.tsv'
        script.write_text(actions, encoding='utf-8', newline='\n')
        output = folder / 'journey'
        command = [str(exe), source, '--dev-input-script', str(script),
            '--dev-input-output', str(output), '--javascript', '--no-profile']
        if trust_file is not None:
            command.extend(['--trust-file', str(trust_file)])
        run = capture_tests.execute(command, folder / 'command.log', repo, 120)
        record['run'] = run
        checks['process_completed'] = run['state'] == 'completed' and run['exit_code'] == 0
        for label in LABELS:
            stage = snapshot(output, label)
            snapshots[label] = stage
            text, marker = stage['activity'], stage['markers']
            checks[label + ':paired_pixels_and_actual_identity'] = stage['paired']
            checks[label + ':zero_script_errors'] = '0 error(s).' in text
            checks[label + ':no_native_audio_owner'] = 'Audio owner opened=false retained_buffers=0 busy=false' in text
            observed = {'markers': marker, 'ppm_sha256': stage['ppm_sha256'],
                'document': stage['document'], 'geometry': stage['geometry'], 'canvas': stage['canvas']}
            observations[label] = observed
            if label == 'navigated':
                checks['navigation_cleared_binding_both_players_and_owned_resources'] = (
                    'HTML video binding' not in text and 'Local video: none' in text
                    and 'Local audio: none' in text and 'Original A/V player document left' in stage['source'])
                continue
            checks[label + ':one_authored_video_node'] = stage['source'].lower().count('<video ') == 1
            media, binding = MEDIA.search(text), BINDING.search(text)
            rendered = re.search(r'Media rendered rect=Some\(\((-?\d+), (-?\d+), (\d+), (\d+)\)\)', text)
            if not media or not binding or not rendered:
                raise ValueError('Missing actual bound player or rendered geometry: ' + label)
            video = {'playing': media[1] == 'true', 'ended': media[2] == 'true', 'time_ms': int(media[3]),
                'duration_ms': int(media[4]), 'frame': int(media[5]), 'frames': int(media[6]),
                'pending': media[7], 'generation': int(media[10]), 'error': media[11]}
            observed['video'] = video
            observed['binding'] = {'node': int(binding[1]), 'ready': int(binding[2]), 'network': int(binding[3]),
                'paused': binding[6] == 'true', 'ended': binding[7] == 'true', 'seeking': binding[8] == 'true',
                'time_ms': int(binding[9]), 'duration_ms': int(binding[10]), 'source_pending': binding[11] == 'true',
                'error': binding[12]}
            expected_frame = frame_at(clip, video['time_ms'])
            x, y, width, height = map(int, rendered.groups())
            actual = crop(stage, (x, y, 64, 48))
            rgba = reference[expected_frame * 64 * 48 * 4:(expected_frame + 1) * 64 * 48 * 4]
            expected = bytes(value for index, value in enumerate(rgba) if index % 4 != 3)
            errors = [abs(a - b) for a, b in zip(actual, expected)]
            observed.update(expected_frame=expected_frame, frame_rect=[x, y, 64, 48],
                frame_sha256=digest(actual), reference_mean_rgb_error=sum(errors) / len(errors),
                reference_max_channel_error=max(errors))
            checks[label + ':actual_authored_box_and_original_frame_geometry'] = (
                (width, height) == (480, 360) and (media[8], media[9]) == ('64', '48')
                and (binding[4], binding[5]) == ('64', '48'))
            checks[label + ':strict_independent_decoded_display_pixels'] = (
                len(actual) == len(expected) == 64 * 48 * 3 and bool(errors)
                and observed['reference_mean_rgb_error'] < .5 and max(errors) <= 2)
            checks[label + ':paused_real_time_frame_and_completed_controls'] = (
                not video['playing'] and video['pending'] == 'None' and video['error'] == 'None'
                and video['duration_ms'] == 3000 and video['frames'] == 24 and video['frame'] == expected_frame
                and binding.groups()[5:8] == ('true', 'false', 'false') and binding.groups()[10:12] == ('false', 'None')
                and int(binding[9]) == video['time_ms'] and int(binding[10]) == 3000
                and marker.get('paused') == '1' and marker.get('ended') == '0'
                and marker.get('errors') == '0' and marker.get('metadata') == '1'
                and int(marker.get('ready', '-1')) >= 2 and marker.get('duration_ms') == '3000'
                and abs(int(marker.get('current_ms', '-999')) - video['time_ms']) <= 1)
            if label.startswith('mkv'):
                audio, device = AUDIO.search(text), DEVICE.search(text)
                if not audio or not device:
                    raise ValueError('Missing actual PCM player/ownership state: ' + label)
                observed['audio'] = {'playing': audio[1] == 'true', 'ended': audio[2] == 'true',
                    'pending': audio[3] == 'true', 'time_ms': int(audio[4]), 'duration_ms': int(audio[5]),
                    'frame': int(audio[6]), 'frames': int(audio[7]), 'generation': int(audio[8]), 'error': audio[9]}
                observed['audio_device'] = {'opened': device[1] == 'true', 'queued': int(device[2]),
                    'submitted': int(device[3]), 'retired': int(device[4]), 'position_unit': device[5],
                    'retained_buffers': int(device[6])}
                expected_audio = min(av['pcm_frames'], video['time_ms'] * av['sample_rate'] // 1000)
                checks[label + ':real_multiplexed_sample_selection_and_shared_position'] = (
                    audio.groups()[:3] in [('false', 'false', 'false'), ('false', 'true', 'false')]
                    and int(audio[4]) == video['time_ms'] and int(audio[5]) == 3000
                    and int(audio[6]) == expected_audio and int(audio[7]) == 144000 and audio[9] == 'None')
                checks[label + ':no_device_submission_drain_or_audio_buffers'] = (
                    device.groups() == ('false', '0', '0', '0', 'None', '0'))
                checks[label + ':paused_only_source_never_played_or_claimed_drained_end'] = (
                    marker.get('mode') == 'mkv' and marker.get('raw') == 'scene-inter-av.mkv'
                    and marker.get('play_events') == marker.get('playing_events') == marker.get('ended_events') == '0'
                    and not video['ended'])
                if 'eof' in label:
                    checks[label + ':metadata_eof_selected_not_completed_drain'] = (
                        video['time_ms'] == 3000 and video['frame'] == 23 and int(audio[6]) == 144000
                        and 'Media timeline_eof=true' in text and int(device[3]) == 0 and not video['ended'])
            else:
                checks[label + ':IVF_replacement_removed_PCM_without_opening_device'] = (
                    'Local audio: none' in text and AUDIO.search(text) is None and DEVICE.search(text) is None
                    and marker.get('mode') == 'ivf' and marker.get('raw') == 'scene-inter.ivf')
        checks['one_document_and_same_video_element_survived_source_replacements'] = (
            len({observations[label]['binding']['node'] for label in LABELS[:-1]}) == 1
            and len({snapshots[label]['document']['document'] for label in LABELS[:-1]}) == 1
            and snapshots['navigated']['document']['document'] != snapshots['mkv-ready']['document']['document'])
        checks['original_IVF_native_Play_regression_advanced_without_PCM'] = (
            0 < observations['ivf-paused']['video']['time_ms'] < 3000
            and snapshots['ivf-paused']['markers'].get('play_events') == '1'
            and snapshots['ivf-paused']['markers'].get('playing_events') == '1')
        for ready in ['mkv-ready', 'ivf-ready', 'mkv-restored']:
            checks[ready + ':selected_source_starts_at_real_metadata_zero'] = (
                observations[ready]['video']['time_ms'] == observations[ready]['video']['frame'] == 0)
        checks['IVF_then_MKV_reselected_same_node_with_fresh_metadata_and_PCM_zero'] = (
            snapshots['ivf-ready']['markers'].get('selected') == '1'
            and snapshots['mkv-restored']['markers'].get('selected') == '2'
            and observations['mkv-restored']['audio']['frame'] == 0)
        for settled, frozen in [('mkv-seek', 'mkv-frozen'), ('ivf-seek', 'ivf-frozen'),
                                 ('mkv-restored-seek', 'mkv-restored-frozen')]:
            a, b = observations[settled], observations[frozen]
            order = snapshots[settled]['markers'].get('seek_order', '').split(',')
            checks[settled + ':genuine_seek_event_order_and_one_completion'] = (
                snapshots[settled]['markers'].get('seek_done') == '1'
                and snapshots[settled]['markers'].get('seek_target_ms') == '1250'
                and len(order) >= 3 and order[0] == 'seeking' and order[-1] == 'seeked'
                and all(event == 'timeupdate' for event in order[1:-1]))
            checks[settled + ':1250ms_video10_and_frozen_generation_pixels'] = (
                a['video']['time_ms'] == b['video']['time_ms'] == 1250
                and a['video']['frame'] == b['video']['frame'] == 10
                and a['video']['generation'] == b['video']['generation']
                and a['frame_sha256'] == b['frame_sha256'])
            if settled.startswith('mkv'):
                checks[settled + ':PCM60000_frozen_with_selected_video'] = (
                    a['audio']['frame'] == b['audio']['frame'] == 60000
                    and a['audio']['time_ms'] == b['audio']['time_ms'] == 1250
                    and a['audio']['generation'] == b['audio']['generation'])
        checks['EOF_selected_pixels_and_sample_offset_remain_frozen_without_drain'] = (
            observations['mkv-eof-selected']['frame_sha256'] == observations['mkv-eof-frozen']['frame_sha256']
            and observations['mkv-eof-selected']['audio']['frame'] == observations['mkv-eof-frozen']['audio']['frame'] == 144000)
        checks['actual_journey_completed'] = True
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, ZeroDivisionError) as error:
        record['error_type'], record['error'] = type(error).__name__, str(error)
    record['source_after'] = capture_tests.source_identity(repo)
    checks['source_unchanged'] = before == record['source_after']
    checks['executable_unchanged'] = exe.is_file() and capture_tests.sha256(exe) == exe_sha
    record['driver_sha256'] = capture_tests.sha256(Path(__file__))
    record['passed'] = bool(checks) and all(value is True for value in checks.values())
    capture_tests.write_json(folder / 'manifest.json', record)
    print(json.dumps(checks, indent=2))
    print('Evidence: ' + str(folder))
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
