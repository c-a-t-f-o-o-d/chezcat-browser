"""Original development generator for Unicode 17 ECMAScript simple folding."""
import argparse
import hashlib
import json
from pathlib import Path

DIGEST = 'ff8d8fefbf123574205085d6714c36149eb946d717a0c585c27f0f4ef58c4183'
SIZE = 87539
ARCHIVE = '2066d1909b2ea93916ce092da1c0ee4808ea3ef8407c94b4f14f5b7eb263d28e'


def generate(source, manifest):
    expected = {
        'unicode_version': '17.0.0',
        'archive': {'url': 'https://www.unicode.org/Public/17.0.0/ucd/UCD.zip', 'sha256': ARCHIVE},
        'file': {'url': 'https://www.unicode.org/Public/17.0.0/ucd/CaseFolding.txt', 'sha256': DIGEST, 'bytes': SIZE},
    }
    if manifest != expected:
        raise ValueError('Unexpected Unicode folding manifest')
    body = (source / 'CaseFolding.txt').read_bytes()
    if len(body) != SIZE or hashlib.sha256(body).hexdigest() != DIGEST:
        raise ValueError('Unicode folding source identity mismatch')
    if not body.startswith(b'# CaseFolding-17.0.0.txt'):
        raise ValueError('Unicode folding version mismatch')
    pairs = {}
    for line in body.decode('utf-8').splitlines():
        fields = [field.strip() for field in line.split('#', 1)[0].split(';')]
        if len(fields) <= 2:
            continue
        if fields[1] not in ('C', 'S', 'F', 'T'):
            raise ValueError('Unknown folding status')
        if fields[1] not in ('C', 'S'):
            continue
        source_code = int(fields[0], 16)
        target = fields[2].split()
        if len(target) != 1 or source_code in pairs:
            raise ValueError('Invalid simple folding record')
        target_code = int(target[0], 16)
        if any(code > 0x10ffff or 0xd800 <= code <= 0xdfff for code in (source_code, target_code)):
            raise ValueError('Invalid folding scalar')
        pairs[source_code] = target_code
    if len(pairs) != 1512 or any(pairs.get(target, target) != target for target in pairs.values()):
        raise ValueError('Unexpected mapping count or noncanonical target')
    # The shared reference matcher measures admitted slices in UTF16 units.
    # Reject future data changes which invalidate that width invariant.
    if any((source_code > 0xffff) != (target > 0xffff) for source_code, target in pairs.items()):
        raise ValueError('Folding changes UTF16 width')
    lines = ['//! Generated Unicode 17.0.0 simple/common case-fold mappings.',
             '//! Original generator: tools/generate_regexp_folding_data.py.',
             '//! Source/license: tools/reference/unicode-case-folding/.',
             '//! CaseFolding.txt SHA-256 ' + DIGEST,
             'pub(super) static PAIRS: &[(u32,u32)] = &[']
    lines.extend(f'    ({source_code:#x},{target:#x}),' for source_code, target in sorted(pairs.items()))
    lines.append('];')
    return '\n'.join(lines) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    body = generate(args.source, json.loads(args.manifest.read_text(encoding='utf-8'))).encode('utf-8')
    if args.check:
        if args.output.read_bytes() != body:
            raise SystemExit('Generated folding tables differ')
    else:
        args.output.write_bytes(body)
    print('Unicode17 simple/common mappings: 1512; idempotent; UTF16 width preserved')


if __name__ == '__main__':
    main()
