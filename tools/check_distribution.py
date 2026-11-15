"""Verify exact release ZIPs, then exercise only the extracted offline runtime.

Run from the exact clean tagged source checkout. The full output/evidence stays
private; its publication member is the bounded public-safe release record.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path, PurePosixPath
import posixpath
import re
import stat
import subprocess
import sys
import urllib.parse
import uuid
import zipfile
import zlib

import build_release_bundle as package
import capture_tests

MAX_BYTES = package.MAX_BYTES
MAX_ENTRIES = 20000
STANDARD_CHECKS = {
    'input_command_completed', 'both_form_queries_and_cross_origin_destination_requested',
    'first_results_rendered', 'destination_loaded', 'back_returns_to_results',
    'second_query_typed', 'source_unchanged',
    'stage_snapshots_match_generated_ppm_and_document_layout',
    'stage_real_activation_preserves_originating_document_and_layout',
}


def require(value, message):
    if not value:
        raise ValueError(message)


def archive_payload(path, prefix):
    """Bound header sizes and reject ambiguous Windows extraction names first."""
    require(path.is_file() and path.stat().st_size <= MAX_BYTES, 'Archive is missing or exceeds its byte allowance.')
    payload = {}
    directories = set()
    seen = set()
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        require(0 < len(entries) <= MAX_ENTRIES, 'Archive entry allowance exceeded or archive is empty.')
        total = 0
        for item in entries:
            # On Windows ZipInfo normalizes backslashes in filename; retain
            # the original decoded spelling to reject that ambiguity as well.
            name = item.orig_filename
            parts = name.rstrip('/').split('/')
            require(name == item.filename and name.startswith(prefix + '/') and '\\' not in name and '\0' not in name
                    and not PurePosixPath(name).is_absolute()
                    and all(part not in ('', '.', '..') and ':' not in part and not part.endswith(('.', ' ')) for part in parts),
                    'Unsafe or unexpected archive path.')
            require(name == '/'.join(parts) + ('/' if item.is_dir() else ''), 'Noncanonical archive path.')
            require(all(not re.fullmatch(r'(?i)(?:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\..*)?', part) for part in parts),
                    'Windows reserved archive path.')
            key = name.rstrip('/').casefold()
            require(key not in seen, 'Duplicate or case-colliding archive entry.')
            seen.add(key)
            kind = stat.S_IFMT(item.external_attr >> 16)
            require(kind in (0, stat.S_IFREG, stat.S_IFDIR), 'Archive links or special files are forbidden.')
            require(not (item.flag_bits & 1), 'Encrypted archive entries are forbidden.')
            relative = name[len(prefix) + 1:]
            if item.is_dir():
                require(kind in (0, stat.S_IFDIR), 'Archive directory type disagrees.')
                directories.add(relative.rstrip('/'))
                continue
            require(kind != stat.S_IFDIR, 'Archive file type disagrees.')
            total += item.file_size
            require(0 <= item.file_size <= MAX_BYTES and total <= MAX_BYTES, 'Expanded archive byte allowance exceeded.')
            with archive.open(item) as stream:
                data = stream.read(item.file_size + 1)
                require(len(data) == item.file_size and stream.read(1) == b'', 'Archive entry length mismatch.')
            # ZipExtFile checks CRC while reaching EOF; malformed CRC raises.
            payload[relative] = data
        parents = {str(parent) for name in payload for parent in PurePosixPath(name).parents if str(parent) != '.'}
        require(directories <= parents | {''}, 'Archive contains extra directories.')
    return payload


def source_export(entries, commit):
    attributes = package.blob(entries['.gitattributes'][2]).decode('utf-8') if '.gitattributes' in entries else ''
    cmd_crlf = bool(re.search(r'^\*\.cmd\s+text\s+eol=crlf\s*$', attributes, re.M))
    subst = bool(re.search(r'^SOURCE_REVISION\s+export-subst\s*$', attributes, re.M))
    result = {}
    for name, (mode, kind, oid) in entries.items():
        require(mode in (b'100644', b'100755') and kind == b'blob', 'Source export must contain tracked regular files only.')
        require(PurePosixPath(name).suffix.lower() not in ('.exe', '.dll', '.pdb'), 'Application binary in source archive.')
        data = package.blob(oid)
        if name == 'SOURCE_REVISION' and subst:
            data = data.replace(b'$Format:%H$', commit.encode('ascii'))
        if name.endswith('.cmd') and cmd_crlf:
            data = data.replace(b'\r\n', b'\n').replace(b'\n', b'\r\n')
        result[name] = data
    require(result.get('SOURCE_REVISION', b'').strip() == commit.encode('ascii'), 'SOURCE_REVISION must export the exact selected commit.')
    return result


def document_links(payload):
    for name, data in payload.items():
        if not name.endswith('.md'):
            continue
        for target in re.findall(r'\]\(([^\s)]+)\)', data.decode('utf-8')):
            parsed = urllib.parse.urlsplit(target)
            if parsed.scheme or parsed.netloc or not parsed.path:
                continue
            resolved = posixpath.normpath(posixpath.join(posixpath.dirname(name), urllib.parse.unquote(parsed.path)))
            require(not resolved.startswith('../') and resolved in payload, 'Broken local runtime documentation link: ' + name + ' -> ' + target)


def validate_archives(runtime, source, tag, commit, approved):
    require(re.fullmatch('[0-9a-f]{40}', commit) and re.fullmatch('[0-9a-f]{64}', approved), 'Exact commit and license digest are required.')
    require(re.fullmatch(r'v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?', tag), 'Exact release tag is required.')
    require(package.require_clean() == commit, 'Checker requires the exact clean source checkout.')
    require(package.git('rev-parse', 'refs/tags/' + tag + '^{commit}').decode().strip() == commit, 'Release tag does not identify the selected commit.')
    entries = package.approve_license(approved, tag, commit)
    report, info, _ = package.verified(commit)
    source_data = archive_payload(source, 'ChezCat-source')
    expected_source = source_export(entries, commit)
    require(source_data == expected_source, 'Source archive paths or payload bytes differ from the selected Git export.')
    runtime_data = archive_payload(runtime, 'ChezCat')
    expected_names = package.runtime_names(entries) | {'ChezCat.exe'}
    require(set(runtime_data) == expected_names | {'build-info.json'}, 'Runtime archive has missing or extra payload files.')
    require(len(runtime_data['build-info.json']) <= 1024 * 1024, 'Unexpected runtime build-info size.')
    identity = json.loads(runtime_data['build-info.json'])
    files = {name: package.sha(data) for name, data in runtime_data.items() if name != 'build-info.json'}
    require(identity.get('files') == files and identity.get('self_hash_excluded') is True, 'Runtime build manifest does not cover the exact payload.')
    require(identity.get('source_commit') == commit and identity.get('clean') is True
            and identity.get('tag') == tag and identity.get('license_sha256') == approved
            and identity.get('compiler') == info['compiler'], 'Runtime build identity disagrees with the selected verified release.')
    require(files['ChezCat.exe'] == report['executable_sha256'] == info['executable_sha256'], 'Runtime executable differs from the verified build.')
    for name in expected_names - {'ChezCat.exe'}:
        require(runtime_data[name] == package.blob(entries[name][2]), 'Runtime payload differs from committed bytes: ' + name)
    require(package.sha(runtime_data['LICENSE']) == approved, 'Runtime license bytes differ from approval.')
    document_links(runtime_data)
    return runtime_data, {
        'tag_commit': True, 'license_scope': True, 'source_payload': True,
        'runtime_payload': True, 'build_identity': True, 'runtime_document_links': True,
    }


def extract_runtime(payload, folder):
    folder.mkdir()
    for name, data in payload.items():
        path = folder.joinpath(*PurePosixPath(name).parts)
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open('xb') as stream:
            stream.write(data)


def exercise_runtime(folder, evidence):
    root = package.ROOT
    executable = folder / 'ChezCat.exe'
    ppm = evidence / 'startup.ppm'
    activity = evidence / 'startup.activity.txt'
    startup = capture_tests.execute([str(executable), str(folder / 'pages/start.html'),
        '--render', str(ppm), '--activity-report', str(activity), '--no-profile'],
        evidence / 'startup.log', folder, 120)
    require(startup['state'] == 'completed' and startup.get('exit_code') == 0, 'Extracted offline startup did not complete.')
    pixels = ppm.read_bytes()
    header = pixels.split(b'\n', 3)
    require(len(header) == 4 and header[0] == b'P6' and header[2] == b'255', 'Startup did not produce a real PPM.')
    width, height = map(int, header[1].split())
    require(0 < width <= 4096 and 0 < height <= 4096 and len(header[3]) == width * height * 3
            and len(set(header[3])) > 1 and 'Page title: ChezCat' in activity.read_text(encoding='utf-8'),
            'Startup pixels/activity do not show the bundled local home page.')
    # Import the trusted checkout's runner, without placing tools or source in
    # the runtime bundle. The standard mode does not read optional fixture files.
    code = ('import pathlib,sys; sys.path.insert(0,sys.argv[1]); '
            'import check_input_harness as h; h.ROOT=pathlib.Path(sys.argv[2]); '
            'sys.argv=["check_input_harness.py"]; raise SystemExit(h.main())')
    journey = capture_tests.execute([sys.executable, '-B', '-c', code, str(root / 'tools'), str(folder)],
        evidence / 'original-journey.log', folder, 240)
    require(journey['state'] == 'completed' and journey.get('exit_code') == 0, 'Extracted standard App/input journey did not complete.')
    matches = re.findall(r'Evidence: (.+)', (evidence / 'original-journey.log').read_text(encoding='utf-8'))
    require(len(matches) == 1, 'Standard journey did not identify one evidence directory.')
    manifest_path = Path(matches[0].strip()).resolve() / 'manifest.json'
    require(manifest_path.is_relative_to(folder.resolve()), 'Journey evidence escaped the extracted runtime.')
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    checks = manifest['checks']
    require(STANDARD_CHECKS <= set(checks) and all(value is True for value in checks.values())
            and manifest['passed'] is True and manifest['executable_sha256'] == capture_tests.sha256(executable)
            and manifest['command']['state'] == 'completed' and manifest['command']['exit_code'] == 0
            and manifest['source_before'] == manifest['source_after'], 'Standard original journey evidence failed identity or behavioral checks.')
    return {'startup': startup, 'journey': journey, 'manifest': str(manifest_path),
            'manifest_sha256': capture_tests.sha256(manifest_path), 'journey_checks': len(checks)}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['runtime', 'source', 'output']:
        parser.add_argument('--' + name, type=Path, required=True)
    for name in ['tag', 'commit', 'license-sha256']:
        parser.add_argument('--' + name, required=True)
    args = parser.parse_args(argv)
    output = args.output.resolve()
    require(not output.exists(), 'Preserve the existing distribution report; choose a new output path.')
    output.parent.mkdir(parents=True, exist_ok=True)
    checks = {'tag_commit': False, 'license_scope': False, 'source_payload': False,
        'runtime_payload': False, 'build_identity': False, 'runtime_document_links': False,
        'offline_startup': False, 'original_app_input_journey': False, 'extracted_payload_unchanged': False}
    publication = {'schema_version': 1, 'passed': False, 'tag': args.tag,
        'source_commit': args.commit, 'license_sha256': args.license_sha256,
        'license_scope_sha256': None, 'covered_history_sha256': None, 'artifacts': {},
        'validation': {'offline_startup': False, 'original_app_input_journey': False}}
    result = {'schema_version': 1, 'passed': False, 'checks': checks, 'publication': publication,
        'scope': 'Exact tagged archive bytes and one extracted offline startup/standard local App-input journey; no public site, device, desktop, reproducibility or security certification.'}
    try:
        runtime = args.runtime.resolve(); source = args.source.resolve()
        for name, path in [('runtime', runtime), ('source', source)]:
            require(path.is_file() and path.stat().st_size <= MAX_BYTES, 'Archive is missing or too large.')
            publication['artifacts'][name] = {'name': path.name, 'sha256': capture_tests.sha256(path), 'bytes': path.stat().st_size}
        payload, static_checks = validate_archives(runtime, source, args.tag, args.commit, args.license_sha256)
        checks.update(static_checks)
        publication['license_scope_sha256'] = package.sha(payload['LICENSE_SCOPE.md'])
        publication['covered_history_sha256'] = package.sha(payload['licensing/COVERED_HISTORY.txt'])
        evidence = output.parent / (output.stem + '-evidence-' + uuid.uuid4().hex[:8])
        evidence.mkdir()
        folder = evidence / 'ChezCat'
        extract_runtime(payload, folder)
        result['evidence'] = exercise_runtime(folder, evidence)
        checks['offline_startup'] = checks['original_app_input_journey'] = True
        checks['extracted_payload_unchanged'] = all((folder / name).read_bytes() == data for name, data in payload.items())
        require(checks['extracted_payload_unchanged'], 'Extracted runtime payload changed during exercise.')
        result['passed'] = publication['passed'] = all(checks.values())
        publication['validation'] = {'offline_startup': True, 'original_app_input_journey': True}
    except (ValueError, OSError, KeyError, TypeError, UnicodeError, RuntimeError,
            subprocess.SubprocessError, zipfile.BadZipFile, zlib.error, KeyboardInterrupt) as error:
        result['error'] = str(error)
    capture_tests.write_json(output, result)
    print('Distribution report: ' + str(output))
    return 0 if result['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
