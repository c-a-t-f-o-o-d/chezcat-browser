"""Generate original static RegExp character-property tables from pinned Unicode 17 data."""
import argparse
import hashlib
import json
from pathlib import Path

VERSION = '17.0.0'
LAST = 0x10ffff
# Exact ECMAScript Table 66 names and aliases; UCD aliases outside this list
# (for example WSpace) are deliberately not admitted.
BINARY_TEXT = '''ASCII
ASCII_Hex_Digit AHex
Alphabetic Alpha
Any
Assigned
Bidi_Control Bidi_C
Bidi_Mirrored Bidi_M
Case_Ignorable CI
Cased
Changes_When_Casefolded CWCF
Changes_When_Casemapped CWCM
Changes_When_Lowercased CWL
Changes_When_NFKC_Casefolded CWKCF
Changes_When_Titlecased CWT
Changes_When_Uppercased CWU
Dash
Default_Ignorable_Code_Point DI
Deprecated Dep
Diacritic Dia
Emoji
Emoji_Component EComp
Emoji_Modifier EMod
Emoji_Modifier_Base EBase
Emoji_Presentation EPres
Extended_Pictographic ExtPict
Extender Ext
Grapheme_Base Gr_Base
Grapheme_Extend Gr_Ext
Hex_Digit Hex
IDS_Binary_Operator IDSB
IDS_Trinary_Operator IDST
ID_Continue IDC
ID_Start IDS
Ideographic Ideo
Join_Control Join_C
Logical_Order_Exception LOE
Lowercase Lower
Math
Noncharacter_Code_Point NChar
Pattern_Syntax Pat_Syn
Pattern_White_Space Pat_WS
Quotation_Mark QMark
Radical
Regional_Indicator RI
Sentence_Terminal STerm
Soft_Dotted SD
Terminal_Punctuation Term
Unified_Ideograph UIdeo
Uppercase Upper
Variation_Selector VS
White_Space space
XID_Continue XIDC
XID_Start XIDS'''


def records(text):
    for line in text.splitlines():
        line = line.split('#', 1)[0].strip()
        if line:
            yield [field.strip() for field in line.split(';')]


def span(text):
    ends = text.split('..')
    low, high = int(ends[0], 16), int(ends[-1], 16)
    assert 0 <= low <= high <= LAST
    return low, high


def normalize(values):
    out = []
    for low, high in sorted(values):
        assert 0 <= low <= high <= LAST
        if out and low <= out[-1][1] + 1:
            out[-1] = (out[-1][0], max(out[-1][1], high))
        else:
            out.append((low, high))
    return out


def subtract(left, right):
    out, at = [], 0
    for low, high in left:
        while at < len(right) and right[at][1] < low:
            at += 1
        current = at
        while current < len(right) and right[current][0] <= high:
            first, last = right[current]
            if low < first:
                out.append((low, first - 1))
            low = max(low, last + 1)
            if low > high:
                break
            current += 1
        if low <= high:
            out.append((low, high))
        at = current
    return out


def runs(labels):
    out = {}
    low = 0
    for point in range(1, len(labels) + 1):
        if point == len(labels) or labels[point] != labels[low]:
            out.setdefault(labels[low], []).append((low, point - 1))
            low = point
    return out


def generate(source, manifest):
    inputs = {}
    for name, record in manifest['files'].items():
        raw = (source / name).read_bytes()
        assert hashlib.sha256(raw).hexdigest() == record['sha256'], name
        assert len(raw) == record['bytes'], name
        inputs[name] = raw.decode('utf-8')
    assert manifest['unicode_version'] == VERSION
    value_aliases = list(records(inputs['PropertyValueAliases.txt']))
    category_aliases = {alias: row[1] for row in value_aliases if row[0] == 'gc' for alias in row[1:] if alias.replace('_', '').isalnum()}
    category_names = 'Lu Ll Lt Lm Lo Mn Mc Me Nd Nl No Pc Pd Ps Pe Pi Pf Po Sm Sc Sk So Zs Zl Zp Cc Cf Cs Co Cn'.split()
    category_ids = {name: index for index, name in enumerate(category_names)}
    labels = bytearray([category_ids['Cn']]) * (LAST + 1)
    mirrored = []
    pending = None
    previous = -1
    for row in records(inputs['UnicodeData.txt']):
        point = int(row[0], 16)
        assert point > previous
        previous = point
        if row[1].endswith(', First>'):
            assert pending is None
            pending = (point, row)
            continue
        if row[1].endswith(', Last>'):
            assert pending is not None
            low, first = pending
            assert first[2] == row[2] and first[9] == row[9]
            pending = None
        else:
            assert pending is None
            low = point
        labels[low:point + 1] = bytes([category_ids[row[2]]]) * (point - low + 1)
        if row[9] == 'Y':
            mirrored.append((low, point))
    assert pending is None
    grouped = runs(labels)
    categories = {name: grouped.get(index, []) for name, index in category_ids.items()}
    for initial in 'CLMNPSZ':
        categories[initial] = normalize([pair for name in category_names if name.startswith(initial) for pair in categories[name]])
    categories['LC'] = normalize(categories['Lu'] + categories['Ll'] + categories['Lt'])
    assert set(category_aliases.values()) == set(categories)
    assert sum(high - low + 1 for name in category_names for low, high in categories[name]) == LAST + 1

    scripts_rows = [row for row in value_aliases if row[0] == 'sc']
    script_aliases = {alias: row[2] for row in scripts_rows for alias in row[1:]}
    script_names = sorted(set(script_aliases.values()))
    script_ids = {name: index for index, name in enumerate(script_names)}
    assert len(script_names) <= 256
    labels = bytearray([script_ids['Unknown']]) * (LAST + 1)
    for row in records(inputs['Scripts.txt']):
        low, high = span(row[0])
        labels[low:high + 1] = bytes([script_ids[row[1]]]) * (high - low + 1)
    grouped = runs(labels)
    scripts = {name: grouped.get(index, []) for name, index in script_ids.items()}
    extensions, overrides = {}, []
    for row in records(inputs['ScriptExtensions.txt']):
        pair = span(row[0])
        overrides.append(pair)
        for alias in row[1].split():
            extensions.setdefault(script_aliases[alias], []).append(pair)
    overrides = normalize(overrides)
    script_extensions = {name: normalize(subtract(scripts[name], overrides) + extensions.get(name, [])) for name in script_names}

    binary_aliases = {alias: fields[0] for line in BINARY_TEXT.splitlines() for fields in [line.split()] for alias in fields}
    binary = {name: [] for name in binary_aliases.values()}
    for name in ['DerivedCoreProperties.txt', 'DerivedNormalizationProps.txt', 'PropList.txt', 'emoji/emoji-data.txt']:
        for row in records(inputs[name]):
            if row[1] in binary:
                binary[row[1]].append(span(row[0]))
    binary['ASCII'] = [(0, 127)]
    binary['Any'] = [(0, LAST)]
    binary['Assigned'] = subtract([(0, LAST)], categories['Cn'])
    binary['Bidi_Mirrored'] = mirrored
    binary = {name: normalize(values) for name, values in binary.items()}
    assert all(binary.values()), [name for name, values in binary.items() if not values]
    # Every admitted alias must also exist in the pinned UCD, apart from the
    # three ECMAScript synthetic properties.
    ucd_aliases = {alias: row[1] for row in records(inputs['PropertyAliases.txt']) for alias in row}
    for alias, name in binary_aliases.items():
        assert name in ('ASCII', 'Any', 'Assigned') or ucd_aliases.get(alias) == name, alias

    tables, ids = [], {}
    def table(values):
        key = tuple(values)
        if key not in ids:
            ids[key] = len(tables)
            tables.append(key)
        return ids[key]
    groups = {}
    for name, aliases, values in [('CATEGORY', category_aliases, categories), ('SCRIPT', script_aliases, scripts), ('SCRIPT_EXTENSIONS', script_aliases, script_extensions), ('BINARY', binary_aliases, binary)]:
        groups[name] = [(alias, table(values[value])) for alias, value in sorted(aliases.items())]
    pin = hashlib.sha256(json.dumps(manifest, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    lines = ['// Generated by tools/generate_regexp_property_data.py from Unicode 17.0.0.', '// Normative Unicode data © 2025 Unicode, Inc. See THIRD_PARTY_NOTICES.md.', '// Source identities: tools/reference/unicode-properties/source.json.', f'// Canonical source manifest SHA-256 {pin}', '', 'pub(super) const TABLES: &[&[(u32,u32)]] = &[']
    for pairs in tables:
        lines.append('    &[')
        lines.extend(f'        (0x{low:X},0x{high:X}),' for low, high in pairs)
        lines.append('    ],')
    lines.append('];')
    for name, rows in groups.items():
        lines.extend(['', f'pub(super) const {name}: &[(&str,usize)] = &['])
        lines.extend(f'    ("{alias}",{index}),' for alias, index in rows)
        lines.append('];')
    stats = {'categories': len(categories), 'script_values': len(script_names), 'binary_properties': len(binary), 'tables': len(tables), 'ranges': sum(map(len, tables)), 'aliases': {name: len(rows) for name, rows in groups.items()}}
    return '\n'.join(lines) + '\n', stats


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('--manifest', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    output, stats = generate(args.source, json.loads(args.manifest.read_text(encoding='utf-8')))
    if args.check:
        assert args.output.read_text(encoding='utf-8') == output, 'Generated Unicode property data differs'
    else:
        args.output.write_text(output, encoding='utf-8', newline='\n')
    print(json.dumps(stats, sort_keys=True))


if __name__ == '__main__':
    main()
