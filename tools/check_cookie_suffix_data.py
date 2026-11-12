#!/usr/bin/env python3
"""Offline data/model checks. Optional system libpsl is a development oracle ONLY.

This does not compile or execute ChezCat Rust and cannot validate native cookie
state transitions. The full Cargo tests remain the required native gate.
"""
import argparse
import ctypes
import ctypes.util
import hashlib
import json
from pathlib import Path
import tempfile
import zipfile

from generate_public_suffix import ROOT, DATA, ascii_rule, generate


def tables():
    result = {'': set(), '*.': set(), '!': set()}
    for line in (DATA / 'source.dat').read_text(encoding='utf-8').splitlines():
        if not line or line.startswith('//'):
            continue
        marker, rule = ascii_rule(line)
        result[marker].add(rule)
    return result


def model(host, rules):
    # Mirrors the Rust byte-suffix scan, not a Rust execution.
    best = host.rfind('.') + 1
    start, previous = 0, None
    while True:
        suffix = host[start:]
        if suffix in rules['!']:
            best = start + suffix.find('.') + 1
            break
        if suffix in rules['']:
            best = min(best, start)
        if suffix in rules['*.']:
            best = min(best, previous if previous is not None else start)
        dot = suffix.find('.')
        if dot < 0:
            break
        previous, start = start, start + dot + 1
    return None if best == 0 else host[host.rfind('.', 0, best - 1) + 1:]


def oracle(host, rules):
    # Independent rule-oriented longest-match calculation, only for small
    # witness sets (exhaustively compares rules, not the candidate suffix loop).
    matching = []
    exceptions = []
    for rule in rules['!']:
        if host == rule or host.endswith('.' + rule):
            exceptions.append(rule.split('.')[1:])
    if exceptions:
        suffix_labels = max(map(len, exceptions))
    else:
        # Conservative wildcard-base protection is explicit in the Rust policy.
        for rule in rules[''] | rules['*.']:
            if host == rule or host.endswith('.' + rule):
                matching.append(len(rule.split('.')))
        for rule in rules['*.']:
            if host.endswith('.' + rule):
                matching.append(len(rule.split('.')) + 1)
        suffix_labels = max(matching, default=1)
    labels = host.split('.')
    return '.'.join(labels[-suffix_labels - 1:]) if len(labels) > suffix_labels else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'results/verification/cookies/suffix-data.json')
    args = parser.parse_args()
    raw = (DATA / 'source.dat').read_bytes()
    generated, data_report = generate(raw)
    assert (DATA / 'rules.rs').read_bytes() == generated.encode('utf-8')
    metadata = json.loads((DATA / 'provenance.json').read_text(encoding='utf-8'))
    archive = ROOT / 'tools/reference/public-suffix/openjdk-public-suffix.dat'
    assert hashlib.sha256(archive.read_bytes()).hexdigest() == metadata['distribution_archive_sha256']
    reconstructed = set()
    with zipfile.ZipFile(archive) as z:
        assert z.testzip() is None
        for name in z.namelist():
            for line in z.read(name).splitlines():
                assert line[0] in (0, 1)
                reconstructed.add(ascii_rule(line[1:].decode('utf-8')))
    rules = tables()
    actual = {(mark, rule) for mark, values in rules.items() for rule in values}
    assert actual == reconstructed
    hosts = {'com', 'co.uk', 'localhost', 'x.unknown-suffix', 'a.b.unknown-suffix',
             'example.com', 'a.example.com', 'a.github.io', 'b.github.io',
             'www.ck', 'a.www.ck', 'a.ck', 'x.a.ck', 'kawasaki.jp',
             'city.kawasaki.jp', 'a.city.kawasaki.jp', 'a.kawasaki.jp',
             'xn--p1ai', 'x.xn--p1ai', 'a.' * 120 + 'example.com'}
    witnesses = sorted(hosts)
    for host in witnesses:
        assert model(host, rules) == oracle(host, rules), host
    for marker, values in rules.items():
        for rule in values:
            hosts.update([rule, 'x.' + rule, 'y.x.' + rule])
    hosts = sorted(h for h in hosts if len(h) <= 253)
    native = {'available': False, 'cases': 0, 'scope': 'Independent libpsl on the same data, not ChezCat execution'}
    name = ctypes.util.find_library('psl')
    if name:
        library = ctypes.CDLL(name)
        library.psl_load_file.argtypes = [ctypes.c_char_p]
        library.psl_load_file.restype = ctypes.c_void_p
        library.psl_registrable_domain.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        library.psl_registrable_domain.restype = ctypes.c_char_p
        library.psl_free.argtypes = [ctypes.c_void_p]
        with tempfile.TemporaryDirectory(prefix='chezcat-psl-oracle-') as folder:
            normalized = Path(folder) / 'ascii.dat'
            normalized.write_text('\n'.join(sorted(mark + value for mark, value in actual)) + '\n', encoding='ascii')
            context = library.psl_load_file(str(normalized).encode())
            assert context, 'libpsl could not read the fixture data'
            try:
                for host in hosts:
                    value = library.psl_registrable_domain(context, host.encode('ascii'))
                    expected = value.decode('ascii') if value else None
                    assert model(host, rules) == expected, (host, model(host, rules), expected)
            finally:
                library.psl_free(context)
        native.update(available=True, cases=len(hosts), passed=True)
    report = {'passed': True, 'data': data_report, 'rules_preserved': len(actual),
              'independent_rule_model_witnesses': len(witnesses), 'native_reference': native,
              'chezcat_rust_executed': False, 'freshness': metadata['freshness']}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
