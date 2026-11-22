"""Original offline About panel and retained-page App/input journey.

Uses actual native menu/control/key paths and captures only the typed safe-copy
App action. No OS clipboard, network, profile, build or implicit acceptance.
Retained legal text identity and changed software pixels are separate checks;
this is not proof of native clipboard delivery or every rendered legal glyph.
"""
import argparse
import os
from datetime import datetime, timezone
import hashlib
import importlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib
import uuid

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tools/fixtures/about-retained.html'
ACTIONS = ['link-target\t#marker', 'snapshot\tbefore', 'about\tOpen',
    'snapshot\tbuild', 'about\tLicense', 'snapshot\tlicense', 'about\tEnd',
    'snapshot\tlicense-end', 'about\tHome', 'snapshot\tlicense-home',
    'about\tScope', 'snapshot\tscope', 'about\tNotices', 'snapshot\tnotices',
    'about\tBuild', 'snapshot\tbuild-return', 'about\tCopy', 'snapshot\tcopy',
    'about\tClose', 'snapshot\trestored']


def require(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def git_metadata(repo, args, limit):
    # Development verification only. Neither this checker nor the collector
    # fetches missing objects; the installed runtime never invokes Git.
    environment = {key:value for key,value in os.environ.items() if key not in (
        'GIT_DIR','GIT_WORK_TREE','GIT_INDEX_FILE','GIT_COMMON_DIR','GIT_OBJECT_DIRECTORY',
        'GIT_ALTERNATE_OBJECT_DIRECTORIES','GIT_SHALLOW_FILE','GIT_NAMESPACE')}
    row = subprocess.run(['git', '--no-optional-locks', '--no-replace-objects', '--no-lazy-fetch',
        '-c', 'core.fsmonitor=false', '-c', 'core.untrackedCache=false', *args], cwd=repo,
        stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=3, check=False, env=environment)
    require(row.returncode == 0 and len(row.stdout) <= limit, 'Unavailable bounded source metadata')
    return row.stdout


def source_context(repo):
    git = lambda args, limit: git_metadata(repo, args, limit)
    try:
        top = Path(git(['rev-parse', '--show-toplevel'], 4096).decode().strip()).resolve()
        require(top == repo, 'Parent repository is not this source checkout')
        revision = git(['rev-parse', 'HEAD'], 128).decode().strip()
        require(re.fullmatch('[0-9a-f]{40}', revision), 'Invalid source revision')
        dirty = bool(git(['status', '--porcelain=v1', '--untracked-files=normal', '--ignore-submodules=all'], 65536))
        return revision, ('Git working tree dirty at build check' if dirty else 'Git working tree clean at build check')
    except (OSError, ValueError, UnicodeError, subprocess.SubprocessError):
        path = repo / 'SOURCE_REVISION'
        raw = path.read_bytes() if path.is_file() and path.stat().st_size <= 128 else b''
        text = raw.decode('ascii', errors='replace').strip()
        return (text, 'Exported source revision; modified state unverified') if re.fullmatch('[0-9a-f]{40}', text) else (None, None)


def public_main_ordinal(repo, source):
    try:
        require(source is not None and re.fullmatch('[0-9a-f]{40}', source), 'Unknown ordinal source')
        git = lambda args, limit: git_metadata(repo, args, limit).decode('utf-8').strip()
        require(Path(git(['rev-parse', '--show-toplevel'], 4096)).resolve() == repo, 'Parent source repository')
        require(git(['config', '--get', 'remote.origin.url'], 4096) in (
            'https://github.com/c-a-t-f-o-o-d/chezcat-browser.git', 'https://github.com/c-a-t-f-o-o-d/chezcat-browser',
            'git@github.com:c-a-t-f-o-o-d/chezcat-browser.git', 'ssh://git@github.com/c-a-t-f-o-o-d/chezcat-browser.git'), 'Not the public source remote')
        require(git(['rev-parse', '--is-shallow-repository'], 16) == 'false', 'Incomplete source history')
        common = repo / git(['rev-parse', '--git-common-dir'], 4096)
        require(not (common/'info/grafts').exists(), 'Rewritten source ancestry')
        tip = git(['rev-parse', '--verify', 'refs/remotes/origin/main^{commit}'], 128)
        require(re.fullmatch('[0-9a-f]{40}', tip), 'Invalid public main ref')
        # Independently count the tail through S in newest-first output. Do not
        # substitute main's current total or ordinary ancestor membership.
        chain = git(['rev-list', '--first-parent', tip], 8*1024*1024).splitlines()
        require(chain and chain[0] == tip and all(re.fullmatch('[0-9a-f]{40}', item) for item in chain), 'Incomplete first-parent output')
        require(chain.count(source) == 1, 'Source is not on public main first-parent history')
        require(git(['rev-parse', '--verify', 'refs/remotes/origin/main^{commit}'], 128) == tip, 'Public main changed during inspection')
        return len(chain) - chain.index(source)
    except (OSError, ValueError, UnicodeError, subprocess.SubprocessError):
        return None


def read_snapshot(output, label, helper):
    selected = list(output.glob('*-' + label + '.activity.txt'))
    require(len(selected) == 1, 'One paired snapshot required: ' + label)
    stem = selected[0].name.removesuffix('.activity.txt')
    paths = {kind: output / (stem + '.' + kind) for kind in ('activity.txt', 'layout.txt', 'source.html', 'ppm')}
    require(all(p.is_file() for p in paths.values()), 'Paired snapshot files missing')
    activity = paths['activity.txt'].read_text('utf-8')
    d, g, c = [helper.fields(activity, prefix) for prefix in ('Stage document=', 'Stage layout=', 'Stage canvas=')]
    require('Stage paired_ppm_written=1' in activity
        and all(row.get(key) not in (None, 'unknown') for row, key in [(d,'document'),(g,'layout'),(c,'canvas')])
        and c.get('document') == d.get('document') == g.get('layout_document')
        and c.get('dom_commit') == d.get('dom_commit') == g.get('layout_dom')
        and c.get('layout') == g.get('layout') and c.get('layout_dom') == g.get('layout_dom')
        and c.get('layout_document') == d.get('document') and c.get('page_view') == '1', 'Paired page identities differ')
    width, height, _ = helper.ppm(paths['ppm'].read_bytes())
    require(width == int(c['width']) and height == int(c['height']), 'Canvas extent differs from paired pixels')
    extra = {}
    about = output / (stem + '.about.txt')
    state = output / (stem + '.about-state.txt')
    if about.is_file() or state.is_file():
        require(about.is_file() and state.is_file(), 'Paired native text/state evidence missing')
        require(about.stat().st_size <= 2*1024*1024 and state.stat().st_size <= 4096, 'Native text/state evidence exceeds finite allowance')
        text = about.read_bytes();fields = {}
        for line in state.read_text('utf-8').splitlines():
            key, sep, value = line.partition('=')
            require(sep and key not in fields, 'Invalid native state field')
            fields[key] = value
        require(set(fields) == {'scope','section','text_bytes','text_sha256','scroll','text_height','focus'}, 'Unexpected native state fields')
        require(fields['scope'] == 'owned App panel and payload; native clipboard not exercised'
            and int(fields['text_bytes']) == len(text) and fields['text_sha256'] == digest(text), 'Native text bytes/hash differ')
        require(int(fields['scroll']) >= 0 and int(fields['text_height']) > 0, 'Invalid native text geometry')
        extra = {'about': text, 'about_state': fields, 'about_paths': [about, state]}
    return {'activity': activity, 'paths': paths, 'document': d, 'layout': g, 'canvas': c,
            'hashes': {kind: digest(path.read_bytes()) for kind, path in paths.items()}, **extra}


def safe_identity(data, version, license_version, license_hash, expected_source, expected_state, expected_ordinal=None):
    require(len(data) <= 2048 and data.endswith(b'\n') and b'\r' not in data, 'Safe-copy encoding/size differs')
    lines = data.decode('ascii').splitlines()
    require(len(lines) == 6 and lines[0].startswith('ChezCat '), 'Product display label differs')
    display = lines[0][len('ChezCat '):]
    prefixes = ['Build: ', 'Source revision: ', 'Source state: ', 'License: ', 'License SHA-256: ']
    require(all(line.startswith(prefix) for line, prefix in zip(lines[1:], prefixes)), 'Safe-copy field allowlist differs')
    values = [line[len(prefix):] for line, prefix in zip(lines[1:], prefixes)]
    kind, revision, state, terms, terms_hash = values
    unavailable = 'Development build (public main ordinal unavailable)'
    declared = 'Declared tagged build v' + version + ' (publication unverified)'
    require(kind in ('Development build', unavailable, declared), 'Invalid build kind')
    require(revision == 'unknown' or re.fullmatch('[0-9a-f]{40}', revision), 'Invalid embedded source revision')
    states = {'Git working tree clean at build check', 'Git working tree dirty at build check',
        'Git working-tree state unknown', 'Exported source revision; modified state unverified', 'Source provenance unavailable'}
    require(state in states and (revision != 'unknown' or state == 'Source provenance unavailable'), 'Invalid embedded source state')
    require(kind != declared or (revision != 'unknown' and state != 'Git working tree dirty at build check'), 'False declared build state')
    require(expected_source is not None and revision == expected_source, 'Embedded source does not match selected source')
    if expected_state is not None:
        require(state == expected_state, 'Embedded tree state differs from unchanged build checkout')
    suffix = version + ('.' if '+' in version else '+') + 'dev.'
    ordinal = None
    if kind == declared:
        require(display == version, 'Declared tag changed the Cargo version')
    elif kind == unavailable:
        require(display == suffix + ('unknown' if revision == 'unknown' else 'g' + revision[:12]), 'Invalid SHA-only development fallback')
        require(expected_state is None or state != 'Git working tree clean at build check' or expected_ordinal is None,
                'Complete clean public-main build lost its ordinal')
    else:
        match = re.fullmatch(re.escape(suffix) + r'([1-9][0-9]*)\.g' + re.escape(revision[:12]), display)
        require(match is not None and state == 'Git working tree clean at build check', 'Invalid numeric development label')
        ordinal = int(match.group(1))
        require(ordinal <= 2**64-1 and ordinal == expected_ordinal, 'Ordinal does not end at the embedded public-main source')
    require(terms == 'CSAPUL ' + license_version and terms_hash == license_hash, 'Separate exact license identity differs')
    # The closed format already forbids additional fields; these controls also
    # make page/location leakage immediately legible in a failed private run.
    for forbidden in ('C:\\', '/Users/', 'http://', 'https://', '@', 'profile', 'receipt',
                      'history', 'Original A1 retained', 'about-retained.html'):
        require(forbidden not in data.decode('ascii'), 'Non-build material entered safe copy')
    return {'product_version': version, 'display_version': display, 'development_ordinal': ordinal, 'build': kind, 'source_revision': revision,
            'source_state': state, 'license_version': license_version, 'license_sha256': license_hash}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=ROOT)
    parser.add_argument('--exe', type=Path)
    parser.add_argument('--expected-source', help='exact compiled source SHA when independently known; default matching checkout/export')
    args = parser.parse_args();repo = args.repo.resolve();exe = (args.exe or repo / 'ChezCat.exe').resolve()
    sys.path.insert(0, str(repo / 'tools'))
    capture = importlib.import_module('capture_tests');accept = importlib.import_module('accept_license')
    helper = importlib.import_module('check_background_harness');pe = importlib.import_module('check_windows_pe')
    accept.terms();require(accept.check(accept.directory()), 'Existing exact-term acceptance required; runner never accepts')
    require(exe.is_file(), 'Existing executable required')
    provenance = json.loads(FIXTURE.with_suffix('.json').read_bytes())
    fixture_bytes = FIXTURE.read_bytes()
    require(len(fixture_bytes) == provenance['bytes'] and digest(fixture_bytes) == provenance['sha256'], 'Original local fixture identity differs')
    source, expected_state = source_context(repo)
    if args.expected_source is not None:
        require(re.fullmatch('[0-9a-f]{40}', args.expected_source), 'Expected source must be one full SHA')
        if args.expected_source != source:expected_state = None
        source = args.expected_source
    version = tomllib.loads((repo/'Cargo.toml').read_text('utf-8'))['package']['version']
    binary = pe.PE(exe)
    versions = [pe.version_info(data) for path, data in binary.resources.items() if path[0] == 16]
    require(len(versions) == 1 and versions[0]['strings'].get('ProductVersion') == version
        and versions[0]['strings'].get('FileVersion') == version, 'Embedded PE and Cargo full product versions differ')
    legal = {'License': (repo/'LICENSE').read_bytes(), 'Scope': (repo/'LICENSE_SCOPE.md').read_bytes(),
             'Notices': (repo/'THIRD_PARTY_NOTICES.md').read_bytes()}
    folder = capture.verification_root(repo) / 'runs'/(datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-about-journey-'+uuid.uuid4().hex[:8]);folder.mkdir(parents=True)
    before_source = capture.source_identity(repo);exe_hash = capture.sha256(exe)
    checks = {};record = {'schema_version': 1, 'scope': __doc__, 'source_before': before_source,
        'executable_sha256': exe_hash, 'runner_sha256': capture.sha256(Path(__file__)),
        'fixture_sha256': capture.sha256(FIXTURE), 'license_accepted_by_driver': False,
        'network_requests': 0, 'native_clipboard_called_by_driver': False, 'checks': checks,
        'legal_text_identity': {section:{'bytes':len(data),'sha256':digest(data)} for section,data in legal.items()}}
    output = folder/'journey';script = folder/'input.tsv';script.write_text('\n'.join(ACTIONS)+'\n',encoding='utf-8',newline='\n')
    try:
        run = capture.execute([str(exe), str(FIXTURE), '--dev-input-script', str(script),
            '--dev-input-output', str(output), '--no-profile', '--no-javascript', '--images'], folder/'command.log', repo, 120)
        record['run'] = run;checks['process_completed'] = run['state'] == 'completed' and run['exit_code'] == 0
        labels = ['before','build','license','license-end','license-home','scope','notices','build-return','copy','restored']
        stages = {label: read_snapshot(output,label,helper) for label in labels}
        baseline = stages['before']
        initial = read_snapshot(output, 'initial', helper)
        initial_layout = initial['paths']['layout.txt'].read_text('utf-8')
        checks['original_anchor_words_have_real_fragment_target'] = all(
            ('text="'+word+'" href=Some("#marker")') in initial_layout
            for word in ('Original','marker','anchor'))
        checks['actual_original_local_document_loaded'] = (baseline['hashes']['source.html'] == capture.sha256(FIXTURE)
            and 'Page title: Original About retained page' in baseline['activity'])
        checks['real_initial_page_scroll'] = int(baseline['canvas']['scroll']) > 0
        checks['underlying_page_source_layout_document_scroll_retained'] = all(
            stage['hashes']['source.html'] == baseline['hashes']['source.html']
            and stage['hashes']['layout.txt'] == baseline['hashes']['layout.txt']
            and all(stage['document'].get(key) == baseline['document'].get(key) for key in ('document','dom_commit','navigation_queued','navigation_committed'))
            and stage['layout']['layout'] == baseline['layout']['layout']
            and stage['canvas']['scroll'] == baseline['canvas']['scroll'] for stage in stages.values())
        identity = safe_identity(stages['build']['about'], version, accept.VERSION, digest(legal['License']), source, expected_state,
                                 public_main_ordinal(repo, source))
        record['observed_identity'] = identity
        checks['build_full_version_source_and_separate_license_identity'] = True
        for label in ('build','build-return','copy'):
            checks[label+'_exact_build_text'] = stages[label]['about'] == stages['build']['about'] and stages[label]['about_state']['section'] == 'Build'
        for label, section in [('license','License'),('license-end','License'),('license-home','License'),('scope','Scope'),('notices','Notices')]:
            checks[label+'_exact_complete_embedded_text'] = stages[label]['about'] == legal[section] and stages[label]['about_state']['section'] == section
        checks['legal_native_text_focus_and_copy_native_focus'] = (all(stages[label]['about_state']['focus'] == 'AboutText'
            for label in ('license','license-end','license-home','scope','notices'))
            and stages['copy']['about_state']['focus'] == 'AboutCopy')
        checks['actual_license_end_home_scroll_and_pixels'] = (int(stages['license-end']['about_state']['scroll']) > 0
            and int(stages['license-home']['about_state']['scroll']) == 0
            and stages['license-end']['hashes']['ppm'] != stages['license-home']['hashes']['ppm'])
        panel_labels = [label for label in labels if label not in ('before','restored')]
        checks['native_panel_actually_painted'] = all(stages[label]['hashes']['ppm'] != baseline['hashes']['ppm'] for label in panel_labels)
        checks['distinct_native_sections_visible'] = len({stages[label]['hashes']['ppm'] for label in ('build','license','scope','notices')}) == 4
        checks['close_returns_exact_owner_page_pixels'] = (stages['restored']['hashes']['ppm'] == baseline['hashes']['ppm']
            and 'about' not in stages['restored'] and 'about' not in baseline)
        copy_index = ACTIONS.index('about\tCopy') + 1
        copies = list(output.glob('about-copy-*.txt'))
        checks['one_actual_typed_copy_payload_exact_and_safe'] = (len(copies) == 1 and copies[0].name == 'about-copy-%02d.txt'%copy_index
            and copies[0].read_bytes() == stages['build']['about'])
        log = (output/'input.log').read_text('utf-8')
        checks['native_clipboard_not_called_and_scripts_disabled'] = ('Captured actual CopyBuildInformation App payload; native clipboard was not called.' in log
            and 'JavaScript enabled: false' in log and 'FAILED:' not in log)
        checks['no_http_navigation_or_resource_activity'] = not any(re.search(r'(?m)^(?:GET|POST|HEAD) https?://', stage['activity']) for stage in stages.values())
        record['stages'] = {label: {'files': {p.name:{'bytes':p.stat().st_size,'sha256':capture.sha256(p)} for p in list(stage['paths'].values())+stage.get('about_paths',[])},
            'document':stage['document'],'layout':stage['layout'],'canvas':stage['canvas'],'about_state':stage.get('about_state')} for label,stage in stages.items()}
        record['copy_files'] = {p.name:{'bytes':p.stat().st_size,'sha256':capture.sha256(p)} for p in copies}
    except (OSError,ValueError,KeyError,TypeError,UnicodeError) as error:
        checks['journey_finished'] = False;record.update(error_type=type(error).__name__,error=str(error))
    record['source_after'] = capture.source_identity(repo)
    checks['source_unchanged'] = before_source == record['source_after'];checks['executable_unchanged'] = exe_hash == capture.sha256(exe)
    record['passed'] = bool(checks) and all(v is True for v in checks.values());record['state'] = 'completed'
    capture.write_json(folder/'manifest.json',record);print(json.dumps(checks,indent=2));print('Evidence: '+str(folder))
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
