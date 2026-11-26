"""Generate original Rust tables from pinned Unicode emoji sequence data.

Development only; no network access or third-party modules. See the adjacent
reference/unicode-emoji-properties manifest and README for provenance.
"""
import argparse
import hashlib
import json
from pathlib import Path

PINNED = {
    'emoji-sequences.txt': ('12cc8267dc33cbd11ed32bcf6fc5dc2ad9c7a77bae1bdfba2f41b1b9b3ead8dd', 194868),
    'emoji-zwj-sequences.txt': ('5b25441daed2322b068c5e70cda522946a4f0274df864445a1965a92e5fc5cad', 277215),
}
NAMES = ('Basic_Emoji', 'Emoji_Keycap_Sequence', 'RGI_Emoji_Modifier_Sequence',
         'RGI_Emoji_Flag_Sequence', 'RGI_Emoji_Tag_Sequence', 'RGI_Emoji_ZWJ_Sequence')


def generate(source, manifest):
    if manifest['unicode_version'] != '17.0.0' or manifest['emoji_version'] != '17.0':
        raise ValueError('Unexpected Unicode/emoji version')
    groups = {name: set() for name in NAMES}
    for filename, (digest, size) in PINNED.items():
        body = (source / filename).read_bytes()
        expected = manifest['files'][filename]
        url = 'https://www.unicode.org/Public/17.0.0/emoji/' + filename
        if expected != {'url': url, 'sha256': digest, 'bytes': size}:
            raise ValueError('Unexpected source manifest: ' + filename)
        if len(body) != size or hashlib.sha256(body).hexdigest() != digest:
            raise ValueError('Source identity mismatch: ' + filename)
        text = body.decode('utf-8')
        if '# Version: 17.0' not in text:
            raise ValueError('Source version mismatch: ' + filename)
        for line in text.splitlines():
            line = line.split('#', 1)[0].strip()
            if not line:
                continue
            codes, name, *_ = (field.strip() for field in line.split(';'))
            if name not in groups or (filename == 'emoji-zwj-sequences.txt') != (name == 'RGI_Emoji_ZWJ_Sequence'):
                raise ValueError('Unexpected sequence type: ' + name)
            if '..' in codes:
                if name != 'Basic_Emoji' or ' ' in codes:
                    raise ValueError('Unexpected sequence range')
                low, high = (int(code, 16) for code in codes.split('..'))
                if low > high:
                    raise ValueError('Reversed range')
                sequences = ((code,) for code in range(low, high + 1))
            else:
                sequences = [tuple(int(code, 16) for code in codes.split())]
            for sequence in sequences:
                if not sequence or any(code > 0x10ffff or 0xd800 <= code <= 0xdfff for code in sequence):
                    raise ValueError('Invalid sequence scalar')
                if sequence in groups[name]:
                    raise ValueError('Duplicate property sequence')
                groups[name].add(sequence)
    if not all(groups.values()):
        raise ValueError('Missing property data')
    groups['RGI_Emoji'] = set().union(*groups.values())
    members = sorted(groups['RGI_Emoji'])
    indexes = {sequence: index for index, sequence in enumerate(members)}
    lines = ['//! Generated Unicode 17.0 emoji string-property data; do not edit.',
             '//! Original generator: tools/generate_regexp_emoji_data.py.',
             '//! Source identities and Unicode license: tools/reference/unicode-emoji-properties/.']
    for filename, (digest, _) in PINNED.items():
        lines.append('//! ' + filename + ' SHA-256 ' + digest)
    lines.append('pub(super) static MEMBERS: &[&[u32]] = &[')
    for sequence in members:
        lines.append('    &[' + ','.join(hex(code) for code in sequence) + '],')
    lines.append('];')
    for number, name in enumerate(sorted(groups)):
        lines.append(f'// {name}: {len(groups[name])} exact sequences.')
        lines.append(f'static PROPERTY_{number}: &[&[u32]] = &[')
        for sequence in sorted(groups[name]):
            lines.append(f'    MEMBERS[{indexes[sequence]}],')
        lines.append('];')
    lines.append('pub(super) static PROPERTIES: &[(&str, &[&[u32]])] = &[')
    for number, name in enumerate(sorted(groups)):
        lines.append(f'    ("{name}", PROPERTY_{number}),')
    lines.append('];')
    return '\n'.join(lines) + '\n', {name: len(values) for name, values in sorted(groups.items())}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    result, counts = generate(args.source, json.loads(args.manifest.read_text(encoding='utf-8')))
    body = result.encode('utf-8')
    if args.check:
        if args.output.read_bytes() != body:
            raise SystemExit('Generated emoji tables differ')
    else:
        args.output.write_bytes(body)
    print(json.dumps(counts, sort_keys=True))


if __name__ == '__main__':
    main()
