"""Finite original VP8 I/P executable acceptance through the real App renderer.

All 24 frame targets are visited nonsequentially across two GOPs. Existing
authored Play/Promise/pause/seek and source-lifecycle journeys are reused intact.
Matroska defaults to selected PCM positions without opening a device; explicit
--device additionally requests the existing WinMM acknowledgement journey.
Controlled loopback HTTP(S) reuses the existing network journey. Offscreen UI
evidence does not prove physical display/audibility, public-service compatibility,
full codec support or byte-exact YUV (the separate original codec probe does).
"""
import argparse
import contextlib
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import uuid

import accept_license
import capture_tests
import check_authored_video_harness as authored
import check_source_replacement_harness as replacement
import check_av_harness as av
import check_network_media_harness as network


ROOT = Path(__file__).resolve().parents[1]
ORDER = (0, 11, 12, 23, 5, 17, 1, 13, 10, 22, 2, 14, 9, 21, 3, 15, 8, 20, 4, 16, 7, 19, 6, 18)


def require(value, message):
    if not value:
        raise ValueError(message)


def reused(name, folder, callback):
    log = folder / (name + '.log')
    with log.open('w', encoding='utf-8', newline='\n') as output:
        with contextlib.redirect_stdout(output):
            status = callback()
    evidence = re.findall(r'^Evidence: (.+)$', log.read_text(encoding='utf-8'), re.M)
    require(len(evidence) == 1, 'Reused checker must emit exactly one evidence record.')
    path = Path(evidence[0].strip()).resolve() / 'manifest.json'
    require(path.is_relative_to(capture_tests.verification_root(ROOT)), 'Evidence escaped verification directory.')
    result = json.loads(path.read_text(encoding='utf-8'))
    require(status == 0 and bool(result['checks']) and all(value is True for value in result['checks'].values()),
        name + ' failed; original evidence retained.')
    return result | {'manifest': capture_tests.evidence_reference(path, ROOT), 'manifest_sha256': capture_tests.sha256(path)}


def stage_pages(folder, fixture_dir):
    stage = folder / 'fixture'
    stage.mkdir()
    originals = ROOT / 'tools/fixtures'
    (stage / 'scene-inter.ivf').write_bytes((fixture_dir / 'scene-inter.ivf').read_bytes())
    (stage / 'scene-intra.ivf').write_bytes((originals / 'scene-intra.ivf').read_bytes())
    (stage / 'authored-video-next.html').write_bytes((originals / 'authored-video-next.html').read_bytes())
    text = (originals / 'authored-video.html').read_text(encoding='utf-8')
    require(text.count('src="scene-intra.ivf"') == 1, 'Authored page source anchor changed.')
    (stage / 'authored-inter-video.html').write_text(text.replace('src="scene-intra.ivf"',
        'src="scene-inter.ivf"'), encoding='utf-8', newline='\n')
    text = (originals / 'source-replacement.html').read_text(encoding='utf-8')
    text = text.replace('scene-intra.ivf', 'PRIMARY_FIXTURE_PLACEHOLDER')
    text = text.replace('scene-replacement.ivf', 'scene-intra.ivf')
    text = text.replace('PRIMARY_FIXTURE_PLACEHOLDER', 'scene-inter.ivf')
    (stage / 'source-inter-replacement.html').write_text(text, encoding='utf-8', newline='\n')
    return stage


def all_frames(kind, folder, fixture_dir, clip, reference, profile):
    folder.mkdir()
    duration = clip['duration_ms']
    starts = [frame['timestamp'] * clip['scale'] * 1000 // clip['rate'] for frame in clip['frames']]
    requested = {f'frame-{index:02}': (index, (starts[index] + (starts[index + 1] if index + 1 < len(starts) else duration)) // 2)
        for index in ORDER}
    actions = ['snapshot\tmetadata']
    for label, (_, milliseconds) in requested.items():
        actions.extend((f'media\tSeek:{milliseconds}', 'snapshot\t' + label))
    actions.extend((f'media\tSeek:{duration}', 'snapshot\teof-selected',
        'media\tStop', 'snapshot\tstopped',
        'address\t' + str(ROOT / 'pages/second.html'), 'key\tEnter', 'snapshot\tnavigated'))
    require(len(actions) <= 64, 'Finite input action budget exceeded.')
    script = folder / 'input.tsv'
    script.write_text('\n'.join(actions) + '\n', encoding='utf-8', newline='\n')
    source = fixture_dir / ('scene-inter.ivf' if kind == 'ivf' else 'scene-inter-av.mkv')
    output = folder / 'journey'
    run = capture_tests.execute([str(ROOT / 'ChezCat.exe'), str(source), '--dev-input-script',
        str(script), '--dev-input-output', str(output), '--no-profile'], folder / 'command.log', ROOT, 120)
    checks = {'process_completed': run['state'] == 'completed' and run['exit_code'] == 0,
        'all_original_frame_targets_once': sorted(index for index, _ in requested.values()) == list(range(24)),
        'backward_and_cross_gop_targets': any(right < left for left, right in zip(ORDER, ORDER[1:]))
            and 11 in ORDER and 12 in ORDER and 23 in ORDER}
    stages = {}
    targets = {'metadata': (0, 0), **requested, 'eof-selected': (23, duration), 'stopped': (0, 0)}
    for label, (expected_index, target_ms) in targets.items():
        path = next(output.glob('*-' + label + '.activity.txt'), None)
        if path is None:
            checks[label + ':snapshot'] = False
            continue
        text = path.read_text(encoding='utf-8')
        match = re.search(r'Media playing=(\w+) ended=(\w+) time_ms=(\d+) duration_ms=(\d+) frame=(\d+) frames=(\d+) pending=(.*?) canvas=(\d+)x(\d+) generation=(\d+) error=(.*)', text)
        rect = re.search(r'Media rendered rect=Some\(\((-?\d+), (-?\d+), (\d+), (\d+)\)\)', text)
        if not match or not rect:
            checks[label + ':real_state_and_rect'] = False
            continue
        state = {'playing': match[1] == 'true', 'ended': match[2] == 'true', 'time_ms': int(match[3]),
            'frame': int(match[5]), 'frames': int(match[6]), 'pending': match[7], 'error': match[11]}
        checks[label + ':settled_requested_frame'] = (state['frame'] == expected_index
            and state['frames'] == 24 and not state['playing'] and state['pending'] == 'None' and state['error'] == 'None')
        if label in requested:
            end = starts[expected_index + 1] if expected_index + 1 < len(starts) else duration
            checks[label + ':requested_timestamp_interval'] = starts[expected_index] <= state['time_ms'] < end
        else:
            checks[label + ':reset_or_eof_time'] = state['time_ms'] == target_ms
        x, y, width, height = map(int, rect.groups())
        ppm_path = path.with_name(path.name.replace('.activity.txt', '.ppm'))
        ppm = ppm_path.read_bytes().split(b'\n', 3)
        canvas_width, canvas_height = map(int, ppm[1].split())
        pixels = ppm[3]
        valid = (width, height) == (64, 48) and x >= 0 and y >= 0 and x + width <= canvas_width and y + height <= canvas_height and len(pixels) == canvas_width * canvas_height * 3
        crop = b''.join(pixels[((y + row) * canvas_width + x) * 3:((y + row) * canvas_width + x + width) * 3] for row in range(height)) if valid else b''
        rgba = reference[expected_index * 64 * 48 * 4:(expected_index + 1) * 64 * 48 * 4]
        rgb = bytes(value for index, value in enumerate(rgba) if index % 4 != 3)
        differences = [abs(a - b) for a, b in zip(crop, rgb)]
        mean = sum(differences) / len(rgb) if len(crop) == len(rgb) else None
        maximum = max(differences, default=256)
        state.update(expected_frame=expected_index, requested_ms=target_ms,
            mean_rgb_error=mean, maximum_channel_error=maximum)
        checks[label + ':tight_independent_presentation_pixels'] = (valid and 'Stage paired_ppm_written=1' in text
            and mean is not None and mean < profile['pixel_tolerance']['mean_rgb_error_less_than']
            and maximum <= profile['pixel_tolerance']['maximum_channel_error_at_most'])
        owner = 'Audio owner opened=false retained_buffers=0 busy=false' in text
        checks[label + ':no_audio_device_opened'] = owner
        if kind == 'matroska':
            audio = re.search(r'Audio playing=(\w+) ended=(\w+) pending=(\w+) time_ms=(\d+) duration_ms=(\d+) frame=(\d+) frames=(\d+) generation=(\d+) error=(.*)', text)
            checks[label + ':shared_selected_pcm_position'] = bool(audio and audio[1] == 'false'
                and audio[3] == 'false' and int(audio[4]) == state['time_ms']
                and int(audio[6]) * 1000 // 48000 == state['time_ms']
                and int(audio[7]) == 144000 and audio[9] == 'None')
        stages[label] = state
    cleared = next(output.glob('*-navigated.activity.txt'), None)
    text = cleared.read_text(encoding='utf-8') if cleared else ''
    checks['navigation_cleared_players_and_owner'] = (cleared is not None
        and 'Local video: none' in text and 'Local audio: none' in text
        and 'Audio owner opened=false retained_buffers=0 busy=false' in text)
    result = {'kind': kind, 'checks': checks, 'passed': all(checks.values()), 'run': run,
        'stages': stages, 'input_actions': len(actions), 'scope': __doc__,
        'pixel_tolerance': profile['pixel_tolerance'],
        'audio_device_opened_by_this_case': False,
        'advancing_acknowledged_pcm_playback_verified': False}
    capture_tests.write_json(folder / 'manifest.json', result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture-dir', type=Path, default=ROOT / 'tools/fixtures')
    parser.add_argument('--display-profile', type=Path,
        help='static display-profile sidecar; defaults to selected fixture directory')
    parser.add_argument('--device', action='store_true', help='explicit opt-in existing WinMM sample-clock journey; no physical-audibility claim')
    args = parser.parse_args()
    fixture_dir = args.fixture_dir.resolve()
    folder = capture_tests.verification_root(ROOT) / 'runs' / (datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ') + '-interframe-journey-' + uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    before = capture_tests.source_identity(ROOT)
    exe = capture_tests.sha256(ROOT / 'ChezCat.exe')
    checks, cases = {}, []
    record = {'schema_version': 1, 'passed': False, 'scope': __doc__, 'checks': checks,
        'cases': cases, 'source_before': before, 'source_after': before, 'executable_sha256': exe,
        'license_accepted_by_driver': False, 'device_opt_in': args.device,
        'byte_exact_yuv_verified_by_this_driver': False, 'physical_audibility_verified': False}
    manifest = folder / 'manifest.json'
    capture_tests.write_json(manifest, record)
    try:
        accept_license.terms()
        checks['existing_exact_acceptance'] = accept_license.check(accept_license.directory())
        require(checks['existing_exact_acceptance'], 'Existing explicit acceptance required; this driver never accepts.')
        profile = args.display_profile or fixture_dir / 'scene-inter-display.json'
        clip, reference, _, display = authored.fixture_reference(fixture_dir / 'scene-inter', profile)
        record['display_profile_sha256'] = capture_tests.sha256(profile)
        checks['fixed_original_two_gop_profile'] = (clip['width'] == 64 and clip['height'] == 48
            and clip['duration_ms'] == 3000 and clip['keyframe_indices'] == [0, 12]
            and len(clip['frames']) == 24 and all(row['version'] == 0 and row['show'] for row in clip['frames'])
            and len(reference) == 24 * 64 * 48 * 4
            and capture_tests.sha256(fixture_dir / 'scene-inter.ivf') == clip['ivf_sha256'])
        require(checks['fixed_original_two_gop_profile'], 'Original fixture profile/identity changed.')
        checks['fixed_centered_display_tolerance'] = display['pixel_tolerance'] == {
            'mean_rgb_error_less_than': 0.5, 'maximum_channel_error_at_most': 2}
        require(checks['fixed_centered_display_tolerance'], 'This milestone requires the fixed mean<0.5/max<=2 display tolerance.')
        av_metadata = json.loads((fixture_dir / 'scene-inter-av.json').read_text(encoding='utf-8'))
        checks['independent_matroska_pcm_identities'] = (capture_tests.sha256(fixture_dir / 'scene-inter-av.mkv') == av_metadata['mkv_sha256']
            and capture_tests.sha256(fixture_dir / 'scene-inter-av.pcm') == av_metadata['pcm_sha256'])
        require(checks['independent_matroska_pcm_identities'], 'Original A/V identity mismatch.')
        stage = stage_pages(folder, fixture_dir)
        calls = [
            ('authored', lambda: authored.main(source=stage / 'authored-inter-video.html', fixture_stem=fixture_dir / 'scene-inter', display_profile=profile)),
            ('source-lifecycle', lambda: replacement.main(source=stage / 'source-inter-replacement.html', reference_paths={
                'scene-intra.rgba': fixture_dir / 'scene-inter-centered.rgba', 'scene-replacement.rgba': ROOT / 'tools/fixtures/scene-intra.rgba'})),
            ('av-existing', lambda: av.main(fixture_stem=fixture_dir / 'scene-inter-av', video_fixture_stem=fixture_dir / 'scene-inter', device=args.device, display_profile=profile)),
        ]
        for name, callback in calls:
            result = reused(name, folder, callback)
            checks.update({name + ':' + key: value is True for key, value in result['checks'].items()})
            checks[name + ':source_and_executable_identity'] = result['source_before'] == result['source_after'] == before and result['executable_sha256'] == exe
            cases.append({'name': name, 'manifest': result['manifest'], 'sha256': result['manifest_sha256']})
        for kind in ['ivf', 'matroska']:
            result = all_frames(kind, folder / kind, fixture_dir, clip, reference, display)
            checks.update({kind + ':' + key: value is True for key, value in result['checks'].items()})
            cases.append({'name': kind, 'manifest': kind + '/manifest.json', 'sha256': capture_tests.sha256(folder / kind / 'manifest.json')})
        for transport in ['http', 'https']:
            name='network-'+transport
            result=network.run_case(transport,folder/name,fixture_stem=fixture_dir/'scene-inter',display_profile=profile)
            checks.update({name+':'+key:value is True for key,value in result['checks'].items()})
            checks[name+':source_and_executable_identity']=(result['source_before']==result['source_after']==before and result['executable_sha256']==exe)
            cases.append({'name':name,'manifest':name+'/manifest.json','sha256':capture_tests.sha256(folder/name/'manifest.json')})
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        record['error'] = str(error)
        checks['journey_completed'] = False
    after = capture_tests.source_identity(ROOT)
    checks['all_seven_cases_present'] = len(cases) == 7
    checks['source_unchanged'] = before == after
    checks['executable_unchanged'] = capture_tests.sha256(ROOT / 'ChezCat.exe') == exe
    record.update(source_after=after, passed=all(checks.values()))
    capture_tests.write_json(manifest, record)
    print(json.dumps(checks, indent=2))
    print('Evidence: ' + str(folder))
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
