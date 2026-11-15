"""Offline actual authored HTML video, native controls, DOM events and frame pixels.

App/input/offscreen only; no native desktop, audio, network or YouTube claim.
Reference pixels are the retained independently decoded original local fixture.
"""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import uuid
import capture_tests

ROOT = Path(__file__).resolve().parents[1]


def main():
    folder = ROOT / 'results/verification/autonomy' / (datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ') + '-authored-video-journey-' + uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    script = folder / 'authored.tsv'
    script.write_text('snapshot\tmetadata\nmedia\tPlay\nwait\t650\nmedia\tPause\nsnapshot\tpaused\nwait\t500\nsnapshot\tfrozen\nclick\treload\nsnapshot\treloaded\nmedia\tSeek:2200\nsnapshot\tseeked\nmedia\tPlay\nwait\t1200\nsnapshot\tended\nmedia\tPlay\nsnapshot\treplay\nmedia\tStop\nsnapshot\tstopped\nlink\tLeave\nsnapshot\tnavigated\n', encoding='utf-8', newline='\n')
    before = capture_tests.source_identity(ROOT)
    out = folder / 'journey'
    row = capture_tests.execute([str(ROOT / 'ChezCat.exe'), str(ROOT / 'tools/fixtures/authored-video.html'), '--dev-input-script', str(script), '--dev-input-output', str(out), '--javascript', '--no-profile'], folder / 'command.log', ROOT, 120)
    checks = {'process_completed': row['state'] == 'completed' and row['exit_code'] == 0}
    stages = {}
    reference = (ROOT / 'tools/fixtures/scene-intra.rgba').read_bytes()
    for name in ['metadata', 'paused', 'frozen', 'reloaded', 'seeked', 'ended', 'replay', 'stopped', 'navigated']:
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
                    offset = ((state['frame'] * 48 + sy) * 64 + sx) * 4
                    expected.extend(reference[offset:offset + 3])
        state['reference_mean_rgb_error'] = sum(abs(a-b) for a, b in zip(samples, expected)) / len(expected) if expected else None
        state['frame_sample_sha256'] = hashlib.sha256(samples).hexdigest()
        checks[name + '_independent_decoded_pixels'] = bool(expected) and len(samples) == len(expected) and state['reference_mean_rgb_error'] < 6
        checks[name + '_no_pending_or_decode_error'] = state['error'] == 'None' and state['pending'] == 'None'
        stages[name] = state
    if all(name in stages for name in ['metadata', 'paused', 'frozen', 'seeked', 'ended', 'replay', 'stopped']):
        p, a, b, s, e, r, t = (stages[name] for name in ['metadata', 'paused', 'frozen', 'seeked', 'ended', 'replay', 'stopped'])
        checks.update(initial_real_metadata=p['frame'] == 0 and p['time_ms'] == 0 and not p['playing'],
                      real_playback_advanced=0 < a['frame'] < 5 and not a['playing'],
                      pause_froze_time_and_pixels=a['time_ms'] == b['time_ms'] and a['frame_sample_sha256'] == b['frame_sample_sha256'],
                      seek_settled_actual_frame=s['frame'] == 4 and 2150 < s['time_ms'] < 2250 and not s['playing'],
                      end_requires_actual_final_frame=e['frame'] == 5 and e['time_ms'] == 3000 and e['ended'] and not e['playing'] and e['events'].count('ended') == 1,
                      real_replay=r['playing'] and not r['ended'] and r['frame'] == 0 and r['time_ms'] < r['duration_ms'],
                      actual_stop_restored_first_frame=not t['playing'] and not t['ended'] and t['time_ms'] == 0 and t['frame_sample_sha256'] == p['frame_sample_sha256'])
    checks['source_unchanged'] = before == capture_tests.source_identity(ROOT)
    capture_tests.write_json(folder / 'manifest.json', {'scope': __doc__, 'executable_sha256': capture_tests.sha256(ROOT / 'ChezCat.exe'), 'source_before': before, 'source_after': capture_tests.source_identity(ROOT), 'run': row, 'checks': checks, 'stages': stages})
    print(json.dumps(checks, indent=2))
    print('Evidence: ' + str(folder))
    return 0 if all(checks.values()) else 1


if __name__ == '__main__':
    raise SystemExit(main())
