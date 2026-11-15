"""Offline App/input source changes, real decoder pixels and stable-state order.

No network, audio device, native desktop or YouTube playback claim.
"""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import uuid
import capture_tests

ROOT = Path(__file__).resolve().parents[1]


def main(source=None, reference_paths=None):
    folder = ROOT / 'results/verification/autonomy' / (datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ') + '-source-replacement-journey-' + uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    script = folder / 'input.tsv'
    script.write_text('snapshot\tinitial\nmedia\tPlay\nwait\t650\nmedia\tPause\nsnapshot\toldpaused\nclick\treplace\nsnapshot\treplaced\nwait\t400\nsnapshot\tfrozen\nmedia\tPlay\nwait\t650\nmedia\tPause\nsnapshot\tnewpaused\nclick\trestore\nsnapshot\trestored\nclick\tclear\nsnapshot\tempty\nclick\tmissing\nsnapshot\tmissing\nclick\treplace\nsnapshot\trecovered\nlink\tLeave\nsnapshot\tnavigated\n', encoding='utf-8', newline='\n')
    before = capture_tests.source_identity(ROOT)
    out = folder / 'journey'
    row = capture_tests.execute([str(ROOT / 'ChezCat.exe'), str(source if source is not None else ROOT / 'tools/fixtures/source-replacement.html'), '--dev-input-script', str(script), '--dev-input-output', str(out), '--javascript', '--no-profile'], folder / 'command.log', ROOT, 120)
    checks = {'process_completed': row['state'] == 'completed' and row['exit_code'] == 0}
    stages = {}
    reference_paths = reference_paths if reference_paths is not None else {name: ROOT / 'tools/fixtures' / name for name in ['scene-intra.rgba', 'scene-replacement.rgba']}
    references = {name: Path(path).read_bytes() for name, path in reference_paths.items()}
    for name in ['initial', 'oldpaused', 'replaced', 'frozen', 'newpaused', 'restored', 'empty', 'missing', 'recovered', 'navigated']:
        file = next(out.glob('*-' + name + '.activity.txt'), None)
        if not file:
            checks[name + '_snapshot'] = False
            continue
        text = file.read_text(encoding='utf-8')
        layout = file.with_name(file.name.replace('.activity.txt', '.layout.txt')).read_text(encoding='utf-8')
        visible = ' '.join(re.findall(r'text="([^"]*)"', layout))
        checks[name + '_zero_script_errors'] = '0 error(s).' in text
        checks[name + '_no_audio_device'] = 'Audio owner opened=false retained_buffers=0 busy=false' in text
        checks[name + '_paired_pixels'] = 'Stage paired_ppm_written=1' in text
        if name == 'navigated':
            checks['navigation_cleanup'] = 'HTML video binding' not in text and 'Local video: none' in text
            continue
        if name in ['empty', 'missing']:
            network = 0 if name == 'empty' else 3
            checks[name + '_honest_empty_state'] = bool(re.search(r'HTML video binding node=\d+ ready_state=0 network_state=' + str(network), text)) and 'Local video: none' in text
            checks[name + '_visible_state'] = ('Source emptied' if name == 'empty' else 'Source failed') in visible
            continue
        match = re.search(r'Media playing=(\w+) ended=(\w+) time_ms=(\d+) duration_ms=(\d+) frame=(\d+) frames=(\d+) pending=(.*?) canvas=(\d+)x(\d+) generation=(\d+) error=(.*)', text)
        rect = re.search(r'Media rendered rect=Some\(\((-?\d+), (-?\d+), (\d+), (\d+)\)\)', text)
        checks[name + '_decoded_state'] = bool(match and rect and re.search(r'HTML video binding node=\d+ ready_state=4 network_state=1 width=64 height=48', text))
        if not checks[name + '_decoded_state']:
            continue
        x, y, w, h = map(int, rect.groups())
        ppm = file.with_name(file.name.replace('.activity.txt', '.ppm')).read_bytes().split(b'\n', 3)
        width, height = map(int, ppm[1].split())
        pixels = ppm[3]
        valid = w == 480 and h == 360 and x >= 0 and y >= 0 and x + w <= width and y + h <= height and len(pixels) == width * height * 3
        reference = references['scene-intra.rgba' if name in ['initial', 'oldpaused', 'restored'] else 'scene-replacement.rgba']
        samples, expected = bytearray(), bytearray()
        if valid:
            for sy in range(41):
                for sx in range(64):
                    dx, dy = int((sx + .5) * w / 64), int((sy + .5) * h / 48)
                    offset = ((y + dy) * width + x + dx) * 3
                    samples.extend(pixels[offset:offset + 3])
                    offset = ((int(match[5]) * 48 + sy) * 64 + sx) * 4
                    expected.extend(reference[offset:offset + 3])
        error = sum(abs(a-b) for a, b in zip(samples, expected)) / len(expected) if expected else None
        checks[name + '_independent_reference_pixels'] = valid and error is not None and error < 6
        checks[name + '_settled'] = match[1] == 'false' and match[7] == 'None' and match[11] == 'None'
        stages[name] = {'frame': int(match[5]), 'time_ms': int(match[3]), 'rgb_error': error, 'sample_sha256': hashlib.sha256(samples).hexdigest()}
        if name in ['replaced', 'frozen', 'recovered']:
            checks[name + '_reset_time_and_frame'] = match[3] == '0' and match[5] == '0'
            checks[name + '_actual_fifo_observers'] = 'before;after;' in layout
    if all(name in stages for name in ['initial', 'oldpaused', 'replaced', 'frozen', 'newpaused', 'restored']):
        checks['original_and_new_playback_advanced'] = stages['oldpaused']['frame'] > 0 and stages['newpaused']['frame'] > 0
        checks['new_source_different_pixels'] = stages['initial']['sample_sha256'] != stages['replaced']['sample_sha256']
        checks['replacement_stays_paused'] = stages['replaced']['sample_sha256'] == stages['frozen']['sample_sha256']
        checks['attr_restore_actual_original_pixels'] = stages['initial']['sample_sha256'] == stages['restored']['sample_sha256']
    checks['source_unchanged'] = before == capture_tests.source_identity(ROOT)
    capture_tests.write_json(folder / 'manifest.json', {'scope': __doc__, 'executable_sha256': capture_tests.sha256(ROOT / 'ChezCat.exe'), 'source_before': before, 'source_after': capture_tests.source_identity(ROOT), 'run': row, 'checks': checks, 'stages': stages})
    print(json.dumps(checks, indent=2))
    print('Evidence: ' + str(folder))
    return 0 if all(checks.values()) else 1


if __name__ == '__main__':
    raise SystemExit(main())
