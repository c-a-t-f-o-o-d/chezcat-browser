"""Original paused authored Matroska/PCM journey over loopback HTTP and TLS.

Reuses the finite authored A/V App/input/offscreen assertions and existing
bounded HTTP/TLS fixture server. Proves real metadata, PCM sample selection,
independent decoded display pixels, same-element IVF/MKV replacement and
navigation teardown. TLS uses the public TEST CA for this browser process only.
No audio device, advancing PCM, audibility, public service, native desktop,
streaming/range, general container or web-platform conformance claim.
Existing explicit exact-term acceptance is checked; never created here.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import sys
import uuid

ROOT = Path(__file__).resolve().parents[1]


def require(value, message):
    if not value:
        raise ValueError(message)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=ROOT)
    parser.add_argument('--exe', type=Path)
    args = parser.parse_args(argv)
    repo = args.repo.resolve()
    exe = (args.exe or repo / 'ChezCat.exe').resolve()
    sys.path.insert(0, str(repo / 'tools'))
    import accept_license
    import capture_tests
    import check_authored_av_harness as authored
    import check_network_media_harness as network
    require(network.ROOT.resolve() == repo, 'Fixture server must belong to the selected product repository.')
    folder = capture_tests.verification_root(repo) / 'autonomy' / (
        datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
        + '-network-authored-av-' + uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    before = capture_tests.source_identity(repo)
    exe_sha = capture_tests.sha256(exe) if exe.is_file() else None
    checks = {'existing_executable': exe_sha is not None, 'existing_exact_license_acceptance': False}
    record = {'schema_version': 1, 'passed': False, 'state': 'running', 'scope': __doc__,
        'checks': checks, 'cases': [], 'source_before': before, 'source_after': before,
        'executable_sha256': exe_sha, 'driver_sha256': capture_tests.sha256(Path(__file__)),
        'authored_checker_sha256': capture_tests.sha256(Path(authored.__file__)),
        'transport_checker_sha256': capture_tests.sha256(Path(network.__file__)),
        'device_attempt': False, 'advancing_acknowledged_pcm_playback_verified': False,
        'physical_audibility_verified': False, 'license_accepted_by_driver': False,
        'os_trust_changed': False, 'service_acceptance_claimed': False,
        'public_service_requests_made_by_driver': 0}
    manifest = folder / 'manifest.json'
    pointer = repo / 'results/verification/latest-network-authored-av.json'
    pointer.parent.mkdir(parents=True, exist_ok=True)
    capture_tests.write_json(manifest, record)
    capture_tests.write_json(pointer, {'report': capture_tests.evidence_reference(manifest, repo),
        'passed': False, 'state': 'running'})
    try:
        require(checks['existing_executable'], 'Build the existing product executable before this journey.')
        accept_license.terms()
        checks['existing_exact_license_acceptance'] = accept_license.check(accept_license.directory())
        require(checks['existing_exact_license_acceptance'],
            'Existing explicit exact-term acceptance required; this driver never accepts.')
        fixtures = repo / 'tools/fixtures'
        fixture_manifest = json.loads((fixtures / 'authored-av.json').read_text(encoding='utf-8'))
        routes = {
            '/authored-av.html': ('authored-av.html', 'text/html; charset=utf-8'),
            '/scene-inter-av.mkv': ('scene-inter-av.mkv', 'video/x-matroska'),
            '/scene-inter.ivf': ('scene-inter.ivf', 'video/x-ivf'),
            '/authored-av-next.html': ('authored-av-next.html', 'text/html; charset=utf-8')}
        payloads = {}
        for route, (name, mime) in routes.items():
            expected = fixture_manifest['files'][name]
            body = (fixtures / name).read_bytes()
            require(len(body) == expected['bytes']
                and hashlib.sha256(body).hexdigest() == expected['sha256'],
                'Original served fixture identity mismatch: ' + name)
            payloads[route] = (mime, body)
        checks['exact_original_served_fixtures_before_contact'] = True

        def run_authored(*, source, trust_file):
            return authored.main(source=source, trust_file=trust_file,
                argv=['--repo', str(repo), '--exe', str(exe)])

        for transport in network.TRANSPORTS:
            try:
                result = network.run_case(transport, folder / transport,
                    authored_runner=run_authored, served_payloads=payloads,
                    entry_path='/authored-av.html', scope=__doc__, route_checks={
                        'authored_document_served': ('/authored-av.html', 1),
                        'Matroska_fully_fetched_and_reselected': ('/scene-inter-av.mkv', 2),
                        'IVF_replacement_fully_fetched': ('/scene-inter.ivf', 1),
                        'leave_destination_served': ('/authored-av-next.html', 1)})
                checks[transport + ':actual_same_source_and_executable'] = (
                    result['source_before'] == result['source_after'] == before
                    and result['executable_sha256'] == exe_sha)
                checks[transport + ':no_range_or_if_range_request'] = bool(result['responses']) and all(
                    row['range_requested'] is False and row['if_range_requested'] is False
                    for row in result['responses'])
            except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
                result = {'transport': transport, 'passed': False,
                    'checks': {'case_completed': False}, 'error_type': type(error).__name__, 'error': str(error)}
                case_folder = folder / transport
                case_folder.mkdir(exist_ok=True)
                capture_tests.write_json(case_folder / 'manifest.json', result)
            record['cases'].append({'transport': transport, 'passed': result['passed'],
                'manifest': transport + '/manifest.json',
                'sha256': capture_tests.sha256(folder / transport / 'manifest.json')})
            checks.update({transport + ':' + name: value is True
                for name, value in result['checks'].items()})
            print(json.dumps({'transport': transport, 'passed': result['passed']}, indent=2), flush=True)
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        record['error_type'], record['error'] = type(error).__name__, str(error)
    after = capture_tests.source_identity(repo)
    checks['source_unchanged'] = after == before
    checks['executable_unchanged'] = exe_sha is not None and exe.is_file() and capture_tests.sha256(exe) == exe_sha
    checks['both_complete_transports'] = len(record['cases']) == len(network.TRANSPORTS)
    record.update(source_after=after, passed=bool(checks) and all(value is True for value in checks.values()), state='completed')
    capture_tests.write_json(manifest, record)
    capture_tests.write_json(pointer, {'report': capture_tests.evidence_reference(manifest, repo),
        'passed': record['passed'], 'state': record['state']})
    print(json.dumps(checks, indent=2))
    print('Evidence: ' + str(folder))
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
