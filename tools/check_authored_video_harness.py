"""Actual authored HTML video, native controls, DOM events and frame pixels.

The default source is the original local fixture. A caller may select a
controlled loopback HTTP(S) source and process-only test trust without changing
the actions or assertions. App/input/offscreen only; no public-service, native
desktop, audio-device or YouTube claim. Reference pixels are the retained
independently decoded original fixture.
"""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import uuid
import capture_tests

ROOT = Path(__file__).resolve().parents[1]


def frame_at(clip, milliseconds):
    """Independent timestamp selection; never use the player's frame report."""
    return max(index for index, frame in enumerate(clip['frames'])
        if frame['timestamp'] * clip['scale'] * 1000 <= milliseconds * clip['rate'])


def fixture_reference(stem, display_profile=None):
    stem = Path(stem)
    clip = json.loads(stem.with_suffix('.json').read_text(encoding='utf-8'))
    original = stem.with_suffix('.rgba').read_bytes()
    if hashlib.sha256(original).hexdigest() != clip['reference_rgba_sha256']:
        raise ValueError('Original independent reference identity mismatch.')
    if display_profile is None:
        return clip, original, clip['reference_rgba_sha256'], None
    profile_path = Path(display_profile)
    profile = json.loads(profile_path.read_text(encoding='utf-8'))
    if (profile['retained_default_rgba_sha256'] != clip['reference_rgba_sha256']
        or profile['input_yuv_sha256'] != capture_tests.sha256(stem.with_suffix('.yuv'))
        or Path(profile['reference_rgba']).name != profile['reference_rgba']):
        raise ValueError('Explicit presentation profile/input identity mismatch.')
    reference = (stem.parent / profile['reference_rgba']).read_bytes()
    if hashlib.sha256(reference).hexdigest() != profile['reference_rgba_sha256']:
        raise ValueError('Explicit independent display reference identity mismatch.')
    return clip, reference, profile['reference_rgba_sha256'], profile


def main(source=None, trust_file=None, fixture_stem=None, display_profile=None, exe=None):
    folder = capture_tests.verification_root(ROOT) / 'autonomy' / (datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ') + '-authored-video-journey-' + uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    script = folder / 'authored.tsv'
    # Keep the original 450 ms decode opportunity. Capture moving evidence in
    # memory, then pause through trusted App input before output I/O/settling.
    script.write_text('snapshot\tmetadata\nmedia\tPlay\nwait\t650\nmedia\tPause\nsnapshot\tpaused\nwait\t500\nsnapshot\tfrozen\nclick\treload\nsnapshot\treloaded\nmedia\tPlay\nwait\t650\nclick\tjs-pause\nsnapshot\tjs-issued\nwait\t1000\nsnapshot\tjs-settled\nwait\t750\nsnapshot\tjs-frozen\nclick\tjs-seek\nwait\t1000\nsnapshot\tjs-seek-settled\nwait\t750\nsnapshot\tjs-seek-frozen\nclick\tjs-play\nwait\t450\nsnapshot-click\tjs-playing\tjs-pause\nwait\t1000\nsnapshot\tjs-play-paused\nwait\t750\nsnapshot\tjs-play-frozen\nmedia\tSeek:2200\nsnapshot\tseeked\nmedia\tPlay\nwait\t1200\nsnapshot\tended\nmedia\tPlay\nsnapshot\treplay\nmedia\tStop\nsnapshot\tstopped\nlink\tLeave\nsnapshot\tnavigated\n', encoding='utf-8', newline='\n')
    before = capture_tests.source_identity(ROOT)
    out = folder / 'journey'
    source = str(source if source is not None else ROOT / 'tools/fixtures/authored-video.html')
    executable = Path(exe) if exe is not None else ROOT / 'ChezCat.exe'
    command = [str(executable), source, '--dev-input-script', str(script), '--dev-input-output', str(out), '--javascript', '--no-profile']
    if trust_file is not None:
        command.extend(['--trust-file', str(trust_file)])
    row = capture_tests.execute(command, folder / 'command.log', ROOT, 120)
    checks = {'process_completed': row['state'] == 'completed' and row['exit_code'] == 0}
    stages = {}
    fixture_stem = Path(fixture_stem) if fixture_stem is not None else ROOT / 'tools/fixtures/scene-intra'
    clip, reference, reference_sha256, profile = fixture_reference(fixture_stem, display_profile)
    last_frame = len(clip['frames']) - 1
    script_seek_frame = frame_at(clip, 1250)
    native_seek_frame = frame_at(clip, 2200)
    checks['independent_reference_identity'] = hashlib.sha256(reference).hexdigest() == reference_sha256
    checks['original_clip_identity'] = capture_tests.sha256(fixture_stem.with_suffix('.ivf')) == clip['ivf_sha256']
    for name in ['metadata', 'paused', 'frozen', 'reloaded', 'js-issued', 'js-settled', 'js-frozen', 'js-seek-settled', 'js-seek-frozen', 'js-playing', 'js-play-paused', 'js-play-frozen', 'seeked', 'ended', 'replay', 'stopped', 'navigated']:
        file = next(out.glob('*-' + name + '.activity.txt'), None)
        if file is None:
            checks[name + '_snapshot'] = False
            continue
        text = file.read_text(encoding='utf-8')
        checks[name + '_zero_script_errors'] = '0 error(s).' in text
        checks[name + '_paired_pixels'] = 'Stage paired_ppm_written=1' in text and file.with_name(file.name.replace('.activity.txt', '.ppm')).is_file()
        if name == 'navigated':
            checks['navigation_cleared_binding_and_player'] = 'HTML video binding' not in text and 'Local video: none' in text and 'Audio owner opened=false retained_buffers=0 busy=false' in text
            checks['actual_destination_rendered'] = 'Video' in file.with_name(file.name.replace('.activity.txt', '.layout.txt')).read_text(encoding='utf-8')
            continue
        match = re.search(r'Media playing=(\w+) ended=(\w+) time_ms=(\d+) duration_ms=(\d+) frame=(\d+) frames=(\d+) pending=(.*?) canvas=(\d+)x(\d+) generation=(\d+) error=(.*)', text)
        binding = re.search(r'HTML video binding node=(\d+) ready_state=(\d+) network_state=(\d+) width=(\d+) height=(\d+)', text)
        rect = re.search(r'Media rendered rect=Some\(\((-?\d+), (-?\d+), (\d+), (\d+)\)\)', text)
        checks[name + '_real_element_binding'] = bool(binding and binding.groups()[1:5] == ('4', '1', '64', '48') and 'authored HTML video' in text and '<video ' in file.with_name(file.name.replace('.activity.txt', '.source.html')).read_text(encoding='utf-8'))
        if not match or not rect:
            checks[name + '_frame_state'] = False
            continue
        state = dict(playing=match[1] == 'true', ended=match[2] == 'true', time_ms=int(match[3]), duration_ms=int(match[4]), frame=int(match[5]), error=match[11], pending=match[7])
        state['events'] = re.search(r'Media lifecycle dropped=\d+ (.*)', text)[1].split(',')
        state['generation'] = int(match[10])
        state['node'] = int(binding[1]) if binding else None
        reference_frame = state['frame']
        if name in ['js-issued', 'js-settled', 'js-frozen', 'js-play-paused', 'js-play-frozen']:
            layout = file.with_name(file.name.replace('.activity.txt', '.layout.txt')).read_text(encoding='utf-8')
            visible = ' '.join(re.findall(r'text="([^"\n]*)"', layout))
            observed = dict(re.findall(r'\b(js_pause_sync|js_pause_microtask|js_pause_ms|js_pause_events)=(\d+)', visible))
            state['js_pause'] = observed
            checks[name + '_genuine_js_pause_observed'] = (observed.get('js_pause_sync') == '1'
                and observed.get('js_pause_microtask') == '1' and observed.get('js_pause_events') == '1'
                and 'js_pause_ms' in observed and abs(int(observed['js_pause_ms']) - state['time_ms']) <= 1)
            checks[name + '_paused_before_end'] = (not state['playing'] and not state['ended']
                and 0 < state['time_ms'] < state['duration_ms']
                and re.search(r'HTML video binding[^\n]*\bpaused=true\b', text) is not None)
        if name in ['js-playing', 'js-play-paused', 'js-play-frozen']:
            layout = file.with_name(file.name.replace('.activity.txt', '.layout.txt')).read_text(encoding='utf-8')
            visible = ' '.join(re.findall(r'text="([^"\n]*)"', layout))
            observed = dict(re.findall(r'\b(js_play_sync|js_play_fulfilled|js_play_events|js_play_start_ms)=(\d+)', visible))
            order = re.search(r'\bjs_play_order=([a-z,]+)', visible)
            state['js_play'] = observed
            state['js_play_order'] = order[1] if order else ''
            checks[name + '_genuine_js_play_fulfilled_once'] = (observed.get('js_play_sync') == '1'
                and observed.get('js_play_fulfilled') == '1' and observed.get('js_play_events') == '1'
                and observed.get('js_play_start_ms') == '1250')
            checks[name + '_playing_event_precedes_promise_then'] = state['js_play_order'] == 'sync,playing,then'
            if name == 'js-playing':
                checks['js_play_real_clock_advancing'] = (state['playing'] and not state['ended']
                    and 1250 < state['time_ms'] < state['duration_ms']
                    and re.search(r'HTML video binding[^\n]*\bpaused=false\b', text) is not None
                    and re.search(r'HTML video binding[^\n]*\bseeking=false\b', text) is not None)
                target = max(index for index, frame in enumerate(clip['frames'])
                    if frame['timestamp'] * clip['scale'] * 1000 <= state['time_ms'] * clip['rate'])
                state['expected_frame_from_moving_time'] = target
                # A moving snapshot can straddle a timer/decode boundary. Check
                # an actually advanced original frame, never a future timestamp,
                # allowing at most the immediately preceding presentation frame.
                checks['js_play_presented_frame_matches_original_timeline'] = (script_seek_frame < state['frame'] <= target
                    and target - state['frame'] <= 1)
        if name in ['js-seek-settled', 'js-seek-frozen']:

            layout = file.with_name(file.name.replace('.activity.txt', '.layout.txt')).read_text(encoding='utf-8')
            visible = ' '.join(re.findall(r'text="([^"\n]*)"', layout))
            observed = dict(re.findall(r'\b(js_seek_sync|js_seek_ms|js_seek_complete)=(\d+)', visible))
            order = re.search(r'\bjs_seek_order=([a-z,]+)', visible)
            events = order[1].split(',') if order else []
            state['js_seek'] = observed
            state['js_seek_events'] = events
            checks[name + '_genuine_js_seek_observed'] = (observed.get('js_seek_sync') == '1'
                and observed.get('js_seek_ms') == '1250' and observed.get('js_seek_complete') == '1')
            checks[name + '_ordered_seek_completion_events'] = (len(events) >= 3 and events[0] == 'seeking'
                and events.count('seeking') == 1 and events.count('seeked') == 1
                and events[-2:] == ['timeupdate', 'seeked'] and all(event in ['seeking', 'timeupdate', 'seeked'] for event in events))
            checks[name + '_real_paused_target'] = (not state['playing'] and not state['ended']
                and state['time_ms'] == 1250 and state['frame'] == script_seek_frame
                and re.search(r'HTML video binding[^\n]*\bpaused=true\b', text) is not None
                and re.search(r'HTML video binding[^\n]*\bseeking=false\b', text) is not None)
        if name in ['js-settled', 'js-frozen', 'js-seek-settled', 'js-seek-frozen', 'js-play-paused', 'js-play-frozen']:
            # Derive the target from original timestamp/timebase metadata,
            # independently of the player's displayed frame report.
            expected_frame = max(index for index, frame in enumerate(clip['frames'])
                if frame['timestamp'] * clip['scale'] * 1000 <= state['time_ms'] * clip['rate'])
            state['expected_frame_from_frozen_time'] = expected_frame
            checks[name + '_frame_matches_frozen_time'] = state['frame'] == expected_frame
            reference_frame = expected_frame
        if name == 'reloaded':
            checks['reload_preserved_raw_attr_identity'] = all(('text="' + word + '"') in file.with_name(file.name.replace('.activity.txt', '.layout.txt')).read_text(encoding='utf-8') for word in ['Raw','source','retained','after','load'])
            checks['reload_reset_real_player'] = not state['playing'] and state['time_ms'] == 0 and state['frame'] == 0
        x, y, w, h = map(int, rect.groups())
        ppm = file.with_name(file.name.replace('.activity.txt', '.ppm')).read_bytes().split(b'\n', 3)
        width, height = map(int, ppm[1].split())
        pixels = ppm[3]
        checks[name + '_actual_inline_rect'] = w == 480 and h == 360 and x >= 0 and y >= 0 and x + w <= width and y + h <= height and len(pixels) == width * height * 3
        # Sample independent source pixel centers; exclude the bottom native bar.
        samples, expected = bytearray(), bytearray()
        if checks[name + '_actual_inline_rect']:
            for sy in range(41):
                for sx in range(64):
                    dx, dy = int((sx + .5) * w / 64), int((sy + .5) * h / 48)
                    offset = ((y + dy) * width + x + dx) * 3
                    samples.extend(pixels[offset:offset + 3])
                    offset = ((reference_frame * 48 + sy) * 64 + sx) * 4
                    expected.extend(reference[offset:offset + 3])
        state['reference_mean_rgb_error'] = sum(abs(a-b) for a, b in zip(samples, expected)) / len(expected) if expected else None
        state['frame_sample_sha256'] = hashlib.sha256(samples).hexdigest()
        checks[name + '_independent_decoded_pixels'] = bool(expected) and len(samples) == len(expected) and state['reference_mean_rgb_error'] < 6
        if name in ['js-issued', 'js-playing']:
            # Pending decode is valid at an issued pause or moving snapshot.
            checks[name + '_no_decode_error'] = state['error'] == 'None'
        else:
            checks[name + '_no_pending_or_decode_error'] = state['error'] == 'None' and state['pending'] == 'None'
        stages[name] = state
    if all(name in stages for name in ['metadata', 'paused', 'frozen', 'seeked', 'ended', 'replay', 'stopped']):
        p, a, b, s, e, r, t = (stages[name] for name in ['metadata', 'paused', 'frozen', 'seeked', 'ended', 'replay', 'stopped'])
        replay_frame_valid = r['frame'] == 0
        if fixture_stem.resolve() != (ROOT / 'tools/fixtures/scene-intra').resolve():
            # A faster fixture can advance during normal harness settling.
            # Prove replay is in the first GOP with a real time-derived frame;
            # keep the original fixture's exact frame-zero assertion intact.
            keys = clip.get('keyframe_indices', [0])
            boundary = (clip['frames'][keys[1]]['timestamp'] * clip['scale'] * 1000 / clip['rate']
                if len(keys) > 1 else r['duration_ms'])
            target = frame_at(clip, r['time_ms'])
            replay_frame_valid = (r['time_ms'] < boundary and 0 <= r['frame'] <= target
                and target - r['frame'] <= 1)
        checks.update(initial_real_metadata=p['frame'] == 0 and p['time_ms'] == 0 and not p['playing'],
                      real_playback_advanced=0 < a['frame'] < last_frame and not a['playing'],
                      pause_froze_time_and_pixels=a['time_ms'] == b['time_ms'] and a['frame_sample_sha256'] == b['frame_sample_sha256'],
                      seek_settled_actual_frame=s['frame'] == native_seek_frame and 2150 < s['time_ms'] < 2250 and not s['playing'],
                      end_requires_actual_final_frame=e['frame'] == last_frame and e['time_ms'] == 3000 and e['ended'] and not e['playing'] and e['events'].count('ended') == 1,
                      real_replay=r['playing'] and not r['ended'] and replay_frame_valid and r['time_ms'] < r['duration_ms'],
                      actual_stop_restored_first_frame=not t['playing'] and not t['ended'] and t['time_ms'] == 0 and t['frame_sample_sha256'] == p['frame_sample_sha256'])
    checks['js_pause_all_snapshots_present'] = all(name in stages for name in ['js-issued', 'js-settled', 'js-frozen'])
    if checks['js_pause_all_snapshots_present']:
        issued, settled, frozen = (stages[name] for name in ['js-issued', 'js-settled', 'js-frozen'])
        checks['js_pause_froze_real_clock_at_issue'] = issued['time_ms'] == settled['time_ms'] == frozen['time_ms']
        checks['js_pause_preserved_source_generation'] = (issued['node'] == settled['node'] == frozen['node']
            and issued['generation'] == settled['generation'] == frozen['generation'])
        checks['js_pause_settled_pixels_remain_frozen'] = (settled['frame'] == frozen['frame']
            and settled['frame_sample_sha256'] == frozen['frame_sample_sha256'])
    checks['js_seek_all_snapshots_present'] = all(name in stages for name in ['js-seek-settled', 'js-seek-frozen'])
    if checks['js_seek_all_snapshots_present']:
        settled, frozen = (stages[name] for name in ['js-seek-settled', 'js-seek-frozen'])
        checks['js_seek_stays_paused_at_real_target'] = settled['time_ms'] == frozen['time_ms'] == 1250
        checks['js_seek_frozen_source_generation'] = (settled['node'] == frozen['node']
            and settled['generation'] == frozen['generation'])
        checks['js_seek_decoded_pixels_stay_frozen'] = settled['frame_sample_sha256'] == frozen['frame_sample_sha256']
    checks['js_play_all_snapshots_present'] = all(name in stages for name in ['js-playing', 'js-play-paused', 'js-play-frozen'])
    if checks['js_play_all_snapshots_present']:
        playing, paused, frozen = (stages[name] for name in ['js-playing', 'js-play-paused', 'js-play-frozen'])
        checks['js_play_real_pause_froze_later_time'] = (playing['time_ms'] <= paused['time_ms']
            and paused['time_ms'] == frozen['time_ms'])
        checks['js_play_pause_retained_source_generation'] = (playing['node'] == paused['node'] == frozen['node']
            and playing['generation'] == paused['generation'] == frozen['generation'])
        checks['js_play_paused_decoded_pixels_frozen'] = paused['frame_sample_sha256'] == frozen['frame_sample_sha256']
    checks['source_unchanged'] = before == capture_tests.source_identity(ROOT)
    capture_tests.write_json(folder / 'manifest.json', {'scope': __doc__, 'source': source, 'trust_file_sha256': capture_tests.sha256(trust_file) if trust_file is not None else None, 'executable_sha256': capture_tests.sha256(executable), 'source_before': before, 'source_after': capture_tests.source_identity(ROOT), 'run': row, 'checks': checks, 'stages': stages})
    print(json.dumps(checks, indent=2))
    print('Evidence: ' + str(folder))
    return 0 if all(checks.values()) else 1


if __name__ == '__main__':
    raise SystemExit(main())
