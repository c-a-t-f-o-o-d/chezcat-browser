"""Original finite media-query viewport App/input/offscreen journey.

Checks full viewport thresholds, desktop hover, independent pixels, physical
zoom and genuine dynamic LINK/Page geometry. Owned loopback only; no native
desktop input, public-service claim, source build or implicit acceptance.
"""
import argparse
from datetime import datetime, timezone
import importlib
import json
from pathlib import Path
import re
import sys
import uuid

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tools/fixtures/media-viewport'
# The available CSS width includes the reserved scrollbar. Layout line width
# is separate and must never substitute for this media-query environment.
CASES = {
    'narrow': (1023, 100, 'narrow'),
    'wide': (1024, 100, 'wide'),
    'zoom-wide': (1280, 125, 'wide'),
    # 1279/1.25 = 1023.2: neither min1024 nor max1023 matches.
    'zoom-narrow': (1279, 125, 'unconditional'),
    'dynamic': (1024, 100, 'wide'),
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def original_control(size):
    # Original components, not a copy of any renderer's resizing algorithm.
    pixels = bytes(component for y in range(size) for x in range(size) for component in (
        (19 + 41*x + 13*y) % 251, (47 + 17*x + 61*y) % 251,
        (83 + 73*x + 29*y) % 251))
    return ('P6\n%d %d\n255\n' % (size, size)).encode() + pixels, pixels


def exact_blocks(pixels, width, height, reference, size):
    found, offset = [], 0
    row_bytes = size * 3
    while True:
        offset = pixels.find(reference[:3], offset)
        if offset < 0:
            return found
        if offset % 3 == 0:
            x, y = (offset // 3) % width, (offset // 3) // width
            if x + size <= width and y + size <= height and all(
                pixels[((y+r)*width+x)*3:((y+r)*width+x+size)*3]
                == reference[r*row_bytes:(r+1)*row_bytes] for r in range(size)):
                found.append((x, y))
                if len(found) > 1:
                    return found
        offset += 1


def snapshot(output, label, helper, reference, size):
    matches = list(output.glob('*-' + label + '.activity.txt'))
    require(len(matches) == 1, 'One actual paired snapshot required: ' + label)
    stem = matches[0].name.removesuffix('.activity.txt')
    files = {kind: output / (stem + '.' + kind) for kind in ('activity.txt', 'layout.txt', 'source.html', 'ppm')}
    require(all(p.is_file() for p in files.values()), 'Paired evidence missing')
    text = files['activity.txt'].read_text('utf-8')
    d, g, c = [helper.fields(text, prefix) for prefix in ('Stage document=', 'Stage layout=', 'Stage canvas=')]
    require('Stage paired_ppm_written=1' in text and d.get('document') not in (None, 'unknown')
        and g.get('layout') not in (None, 'unknown') and c.get('canvas') not in (None, 'unknown')
        and c.get('document') == d.get('document') == g.get('layout_document')
        and c.get('layout') == g.get('layout') and c.get('dom_commit') == d.get('dom_commit')
        and c.get('layout_dom') == g.get('layout_dom') == d.get('dom_commit')
        and c.get('layout_document') == d.get('document') and c.get('page_view') == '1', 'Actual paired identities differ')
    w, h, pixels = helper.ppm(files['ppm'].read_bytes())
    found = exact_blocks(pixels, w, h, reference, size)
    require(len(found) == 1, 'Independent original control must match exactly once at native pixel size')
    return {'activity': text, 'layout': files['layout.txt'].read_text('utf-8'),
            'source_sha256': helper.digest(files['source.html'].read_bytes()),
            'width': w, 'height': h, 'pixels': pixels, 'control': found[0],
            'stamps': [d, g, c], 'files': {p.name: {'bytes': p.stat().st_size,
                'sha256': helper.digest(p.read_bytes())} for p in files.values()}}


def rectangles(s, zoom):
    pixels, w, h = s['pixels'], s['width'], s['height']
    x, cy = s['control']
    scale = zoom / 100
    def px(value):
        result = value * scale
        require(result == int(result), 'Fixture dimensions must map to exact physical pixels')
        return int(result)
    def solid(y, width, height, rgb):
        return (min(x, y) >= 0 and x + width < w and y + height <= h and all(
            pixels[((y+r)*w+x)*3:((y+r)*w+x+width)*3] == rgb*width
            and pixels[((y+r)*w+x+width)*3:((y+r)*w+x+width+1)*3] == b'\xff\xff\xff'
            for r in range(height)))
    # Find the separate fixed base independently of any conditional layout.
    base_matches = [y for y in range(max(0, cy-px(16))+1)
                    if solid(y, px(64), px(16), b'\x1a\x7b\x3c')]
    require(len(base_matches) == 1, 'One exact independent base rectangle required')
    top = base_matches[0]
    states = {}
    for state, nav, target, rgb in [('wide', 0, 32, b'\x00\x88\x44'),
            ('narrow', 48, 96, b'\xaa\x22\x00'), ('unconditional', 48, 160, b'\xaa\x22\x00')]:
        target_y = top + px(16+nav)
        links_present = all('text="' + text + '"' in s['layout'] for text in ('FirstOriginalLink', 'SecondOriginalLink'))
        links_absent = all(text not in s['layout'] for text in ('FirstOriginalLink', 'SecondOriginalLink'))
        states[state] = {
            'target_rectangle_pixels': solid(target_y, px(target), px(32), rgb),
            'following_rectangle_pixels': solid(target_y+px(32), px(48), px(8), b'\x22\x44\x88'),
            'independent_control_relative_geometry': cy == target_y+px(32+8+8),
            'navigation_text_visibility': links_absent if nav == 0 else links_present,
        }
    return states, [x, top]


def run_case(name, repo, exe, folder, capture, helper, origin_helper):
    width, zoom, expected = CASES[name]
    size = 4 if zoom == 100 else 5
    image, reference = original_control(size)
    base = (FIXTURES / 'base.css').read_bytes()
    conditional = (FIXTURES / 'conditional.css').read_bytes()
    template = (FIXTURES / 'page.html').read_text('utf-8')
    dynamic = '''<button type="button" name="apply" id="apply">Apply original conditional stylesheet</button>
<script>
var loads=0;
document.getElementById('apply').onclick=function(input){
 if(!input.isTrusted)throw Error('trusted original input');
 var link=document.createElement('link');link.rel='stylesheet';link.href='/conditional.css';
 link.onload=function(){loads++;if(loads!==1)throw Error('duplicate LINK load');
  if(document.getElementById('target').clientWidth!==32||document.getElementById('navigation').clientHeight!==0)throw Error('current media geometry');
  document.getElementById('state').textContent='Original media geometry 32 and hidden navigation';document.title='Original media active';};
 link.onerror=function(){throw Error('original conditional source');};document.head.appendChild(link);
};
</script>''' if name == 'dynamic' else ''
    html = template.replace('@@CONDITIONAL@@', '' if dynamic else '<link rel="stylesheet" href="/conditional.css">').replace('@@DYNAMIC@@', dynamic).encode()
    payloads = {'/': (html, 'text/html; charset=utf-8'), '/base.css': (base, 'text/css; charset=utf-8'),
                '/conditional.css': (conditional, 'text/css; charset=utf-8'), '/control.ppm': (image, 'image/x-portable-pixmap')}
    def serve(_origin, path):
        body, mime = payloads.get(path, (b'Unknown original fixture route', 'text/plain'))
        return (200 if path in payloads else 404), mime, body, []
    folder.mkdir();origin = origin_helper.Origin('A', serve);output = folder / 'journey'
    script = folder / 'input.tsv';script.write_text('snapshot\tbefore\nclick\tapply\nsnapshot\tchecked\n' if dynamic else 'snapshot\tchecked\n', encoding='utf-8', newline='\n')
    checks, observations = {}, {}
    try:
        origin.start()
        run = capture.execute([str(exe), origin.url+'/', '--width', str(width), '--height', '760', '--zoom', str(zoom),
            '--dev-input-script', str(script), '--dev-input-output', str(output), '--no-profile', '--images',
            '--javascript' if dynamic else '--no-javascript'], folder / 'command.log', repo, 120)
        checks['process_completed'] = run['state'] == 'completed' and run['exit_code'] == 0
        s = snapshot(output, 'checked', helper, reference, size)
        states, base_origin = rectangles(s, zoom)
        checks.update(states[expected])
        checks['independent_base_and_native_size_control_pixels'] = True
        checks['actual_physical_canvas_width'] = s['width'] == width
        checks['paired_document_layout_canvas'] = True
        if dynamic:
            before = snapshot(output, 'before', helper, reference, size);before_states, _ = rectangles(before, zoom)
            checks['before_genuine_input_is_unconditional'] = all(before_states['unconditional'].values())
            checks['same_document_and_original_source_after_dynamic_LINK'] = before['stamps'][0]['document'] == s['stamps'][0]['document'] and before['source_sha256'] == s['source_sha256']
            summaries = re.findall(r'(?m)^\d+ script\(s\).*?(\d+) error\(s\)', s['activity'])
            checks['genuine_LINK_current_Page_geometry_zero_errors'] = bool(summaries) and summaries[-1] == '0' and 'Page title: Original media active' in s['activity']
            observations['before_files'] = before['files']
        else:
            checks['javascript_explicitly_disabled'] = 'JavaScript enabled: false' in (output / 'input.log').read_text('utf-8')
        checks['one_exact_response_per_original_route'] = (len(origin.records) == len(payloads)
            and all(sum(r['path'] == path for r in origin.records) == 1 for path in payloads)
            and all(r['completed'] and r['status'] == 200 and r['mime'] == payloads[r['path']][1]
                and r['sha256'] == helper.digest(payloads[r['path']][0]) for r in origin.records))
        observations.update(run=run, source_sha256=s['source_sha256'], snapshots=s['files'], stamps=s['stamps'],
            physical_width=width, zoom_percent=zoom, expected_css_viewport_width=width/(zoom/100),
            expected_state=expected, actual_paint_states=states, base_origin=base_origin,
            control_origin=s['control'], control_pixels=size)
    except (OSError, ValueError, KeyError, TypeError) as error:
        checks['case_finished'] = False;observations.update(error_type=type(error).__name__, error=str(error))
    finally:
        origin.close()
    checks['owned_worker_stopped'] = not origin.worker.is_alive()
    checks['owned_socket_closed'] = origin.server.socket.fileno() == -1
    return {'checks': checks, 'passed': bool(checks) and all(v is True for v in checks.values()),
            'responses': origin.records, **observations}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=ROOT)
    parser.add_argument('--exe', type=Path)
    parser.add_argument('--case', action='append', choices=CASES)
    args = parser.parse_args();repo = args.repo.resolve();exe = (args.exe or repo / 'ChezCat.exe').resolve()
    sys.path.insert(0, str(repo / 'tools'))
    capture = importlib.import_module('capture_tests');accept = importlib.import_module('accept_license')
    helper = importlib.import_module('check_background_harness');origin = importlib.import_module('check_stylesheet_origin_harness')
    accept.terms();require(accept.check(accept.directory()), 'Existing explicit exact-term receipt required; runner never accepts')
    require(exe.is_file(), 'Existing executable required')
    provenance = json.loads((FIXTURES / 'provenance.json').read_bytes())
    for name, expected in provenance['files'].items():
        body = (FIXTURES / name).read_bytes()
        require(len(body) == expected['bytes'] and helper.digest(body) == expected['sha256'], 'Original fixture identity differs: ' + name)
    folder = capture.verification_root(repo) / 'runs' / (datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-media-viewport-'+uuid.uuid4().hex[:8]);folder.mkdir(parents=True)
    before = capture.source_identity(repo);exesha = capture.sha256(exe)
    record = {'schema_version': 1, 'scope': __doc__, 'source_before': before, 'executable_sha256': exesha,
              'runner_sha256': capture.sha256(Path(__file__)), 'fixture_files': {p.name: capture.sha256(p) for p in FIXTURES.iterdir() if p.is_file()},
              'license_accepted_by_driver': False, 'public_requests': 0, 'cases': {}}
    for name in args.case or CASES:record['cases'][name] = run_case(name, repo, exe, folder / name, capture, helper, origin)
    record['source_after'] = capture.source_identity(repo)
    parser_sources = [r.get('source_sha256') for name, r in record['cases'].items() if name != 'dynamic']
    record['checks'] = {'source_unchanged': before == record['source_after'], 'executable_unchanged': exesha == capture.sha256(exe),
        'same_original_parser_source_across_viewports': not parser_sources or (None not in parser_sources and len(set(parser_sources)) == 1),
        'all_cases_passed': all(r['passed'] for r in record['cases'].values())}
    record['passed'] = all(record['checks'].values());record['state'] = 'completed';capture.write_json(folder / 'manifest.json', record)
    print(json.dumps({'checks': record['checks'], 'cases': {n: r['checks'] for n, r in record['cases'].items()}}, indent=2))
    print('Evidence: '+str(folder));return 0 if record['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
