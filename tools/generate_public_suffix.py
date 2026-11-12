#!/usr/bin/env python3
"""Generate checked-in ASCII suffix tables from local MPL-2.0 PSL data.

No network access and no third-party modules. Not invoked by Cargo or ChezCat.
Run with --check to verify committed data; run without it after deliberately
replacing data/public_suffix/source.dat AND updating provenance.json/license.
The PSL supplies normalized Unicode labels. Punycode conversion applies only to
this trusted development input, not to user-supplied runtime hostnames.
"""
import argparse
import hashlib
import json
from pathlib import Path
import unicodedata

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / 'data/public_suffix'


def ascii_rule(rule):
    marker = '!' if rule.startswith('!') else '*.' if rule.startswith('*.') else ''
    value = rule[len(marker):]
    if value != unicodedata.normalize('NFC', value).lower():
        raise ValueError('PSL input must contain lowercase NFC labels: ' + rule)
    labels = []
    for label in value.split('.'):
        if not label:
            raise ValueError('Empty suffix label')
        label = label if label.isascii() else 'xn--' + label.encode('punycode').decode('ascii')
        if (len(label) > 63 or label.startswith('-') or label.endswith('-')
                or not all(c in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in label)):
            raise ValueError('Invalid suffix label: ' + rule)
        labels.append(label)
    result = '.'.join(labels)
    if len(result) > 253:
        raise ValueError('Suffix exceeds DNS bound')
    return marker, result


def generate(raw):
    exact, wildcard, exception = set(), set(), set()
    counts = {'ICANN': 0, 'PRIVATE': 0}
    section = None
    finished = set()
    for line in raw.decode('utf-8').splitlines():
        line = line.strip()
        for name in counts:
            if line == '// ===BEGIN ' + name + ' DOMAINS===':
                if section or name in finished:
                    raise ValueError('Duplicate/nested PSL section')
                section = name
                break
            if line == '// ===END ' + name + ' DOMAINS===':
                if section != name:
                    raise ValueError('Mismatched PSL section')
                finished.add(name)
                section = None
                break
        else:
            if not line or line.startswith('//'):
                continue
            if not section:
                raise ValueError('Rule outside a PSL section')
            rule = line.split()[0]
            marker, value = ascii_rule(rule)
            table = exception if marker == '!' else wildcard if marker == '*.' else exact
            if value in table:
                raise ValueError('Duplicate canonical PSL rule: ' + rule)
            table.add(value)
            counts[section] += 1
    if section or finished != {'ICANN', 'PRIVATE'} or min(counts.values()) == 0:
        raise ValueError('Incomplete PSL input')
    # Basic sanity checks fail closed on a wrong/truncated source file.
    if len(exact) < 5000 or not {'com', 'co.uk', 'github.io'} <= exact:
        raise ValueError('Incomplete public/private suffix data')
    if 'ck' not in wildcard or 'www.ck' not in exception:
        raise ValueError('Missing wildcard/exception data')
    header = ('// Generated DATA from the MPL-2.0 Public Suffix List; not external implementation code.\n'
              '// See LICENSE-AND-NOTICE.md and provenance.json in this directory.\n'
              '// Regenerate offline: python tools/generate_public_suffix.py\n'
              '// Source SHA-256: ' + hashlib.sha256(raw).hexdigest() + '\n\n')
    output = header
    for name, values in [('EXACT', exact), ('WILDCARD', wildcard), ('EXCEPTION', exception)]:
        output += '#[rustfmt::skip]\nconst ' + name + ': &[&str] = &[\n'
        output += ''.join('    "' + s + '",\n' for s in sorted(values))
        output += '];\n\n'
    output = output.rstrip() + '\n'
    report = {'source_sha256': hashlib.sha256(raw).hexdigest(), **counts,
              'exact': len(exact), 'wildcard': len(wildcard), 'exception': len(exception),
              'generated_sha256': hashlib.sha256(output.encode('utf-8')).hexdigest()}
    return output, report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    generated, report = generate((DATA / 'source.dat').read_bytes())
    encoded = generated.encode('utf-8')
    manifest = (json.dumps(report, indent=2) + '\n').encode('utf-8')
    for name, content in [('rules.rs', encoded), ('generated.json', manifest)]:
        target = DATA / name
        if args.check:
            if not target.is_file() or target.read_bytes() != content:
                raise SystemExit('Stale PSL data: ' + str(target))
        else:
            target.write_bytes(content)
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
