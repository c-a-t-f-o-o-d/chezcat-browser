#!/usr/bin/env python3
"""Compare bounded regexp captures with Node's independent UTF-16 indices.

Development only: Node supplies an oracle; it is never used by ChezCat.
Requires rustc and Node on PATH, with no downloaded packages. This targeted
lookaround check complements check_javascript.py; it is not a conformance suite.
Primary reference: https://tc39.es/ecma262/#sec-compileassertion
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
PROBE = r'''
#![allow(dead_code)]
#[path = SOURCE]
mod regexp;
use std::io::{self, BufRead};
fn decode(text: &str) -> String {
    let bytes = (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i+2], 16).unwrap()).collect();
    String::from_utf8(bytes).unwrap()
}
fn main() {
    for row in io::stdin().lock().lines() {
        let row = row.unwrap(); let parts: Vec<_> = row.split('\t').collect();
        let pattern = decode(parts[0]); let input = decode(parts[2]);
        let mut budget = parts[4].parse().unwrap();
        let result = regexp::Regex::compile(&pattern, parts[1], &mut budget).and_then(|re| {
            re.find(&input.encode_utf16().collect::<Vec<_>>(), parts[3].parse().unwrap(), re.flags.sticky, &mut budget)
        });
        match result {
            Ok(None) => println!("NONE"),
            Ok(Some(m)) => println!("{}", m.captures.iter().map(|c| c.map_or("-".into(), |(a,b)| format!("{a}:{b}"))).collect::<Vec<_>>().join(",")),
            Err(regexp::Error::Syntax(m)) => println!("SYNTAX:{m}"),
            Err(regexp::Error::Unsupported(m)) => println!("UNSUPPORTED:{m}"),
            Err(regexp::Error::Limit(m)) => println!("LIMIT:{m}"),
        }
    }
}
'''
NODE = r'''
const cases = JSON.parse(require('fs').readFileSync(0, 'utf8'));
for (const [pattern, flags, text, start] of cases) {
    const re = new RegExp(pattern, flags.replace(/[gyd]/g, '') + (flags.includes('y') ? 'yd' : 'gd'));
    re.lastIndex = start;
    const m = re.exec(text);
    console.log(m ? m.indices.map(c => c ? c.join(':') : '-').join(',') : 'NONE');
}
'''

def cases():
    rows = []
    # Every operator is checked for a match, a failed assertion and an empty edge.
    for pattern in [r'a(?=b)', r'a(?!b)', r'(?<=a)b', r'(?<!a)b',
                    r'(?=(a+))a', r'(?!(a+))b', r'(?<=(a+))b', r'(?<!(a+))b',
                    r'(?<=([ab]+)([bc]+))$', r'(?<=([ab]+?)([bc]+?))$',
                    r'(?=(a|ab))ab', r'(?<=(a|ab))c',
                    r'(?<=a(?=b))b', r'(?=(?<=a)b)b',
                    r'(?=(a)?b)b', r'(?<=(a)?b)c',
                    r'(?=(a)){0,2}a', r'(?=(a))+a', r'(?=(a))*a',
                    r'(?:(?=(a))a|(?=(b))b)+',
                    r'(?<=((?:ab|a)+))b', r'(?<=((?:ab|a)+?))b']:
        for text in ['', 'a', 'b', 'ab', 'abc', 'aabb', 'bababc']:
            rows.append((pattern, '', text, 0))
    rows.extend([
        (r'(?<=(a{1,3})(a+))b', '', 'aaaaab', 0),
        (r'(?<=(a{1,3}?)(a+?))b', '', 'aaaaab', 0),
        (r'(?<=((a|b)+))c', '', 'ababc', 0),
        (r'(?<=((a|b)+?))c', '', 'ababc', 0),
        (r'(?!(a(b)?))c|(a)(?=b)', '', 'ca', 0),
        (r'(a(?=(b))|b)+', '', 'abb', 0),
        (r'(?=(a))?a', '', 'a', 0),
        (r'(?=(a)){2}a', '', 'a', 0),
        (r'(?=(a)){2,3}?a', '', 'a', 0),
        (r'(?<=(a|ab))c', '', 'abc', 0),
        (r'(?<=(ab|a))c', '', 'abc', 0),
        (r'(?<=(a+)(?=b))b', '', 'aaab', 0),
        (r'(?<=\b(a+))b', '', ' aaab', 0),
        (r'(?<!\b)a', '', 'aa', 0),
        (r'(?<=^a)b', '', 'ab', 0),
        (r'(?<=^a)b', 'm', 'z\nab', 0),
        (r'(?=b$)b', 'm', 'ab\nc', 0),
        (r'(?<=a.)b', 's', 'a\nb', 0),
        (r'(?<=a)b(?=c)', 'i', 'ABC', 0),
        (r'(?<=a)b', 'y', 'ab', 1),
        (r'(?<=a)b', 'y', 'xab', 1),
        (r'(?<=a)b', 'g', 'abab', 2),
        (r'(?<!.)$', '', '', 0),
        (r'(?<=.)$', '', '😀', 0),
        (r'(?<=(..))b', '', '😀b', 0),
        (r'(?=(..))..', '', '😀b', 0),
        (r'(?<=(\ud83d))\ude00', '', '😀', 0),
        (r'(?<=\x00)a', '', '\x00a', 0),
        (r'(?=(?!(b))a)a', '', 'a', 0),
        (r'(?<=(?!(b))a)b', '', 'ab', 0),
        (r'(?<=(a(?<=a)))b', '', 'ab', 0),
    ])
    return rows

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rustc', default='rustc')
    parser.add_argument('--node', default='node')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    comparisons = cases()
    bounded = [
        ('lookbehind-cannot-be-quantified', '(?<=a)*', '', '', 0, 2_000_000, 'SYNTAX:'),
        ('unfinished-lookahead', '(?=a', '', '', 0, 2_000_000, 'SYNTAX:'),
        ('named-groups-remain-explicit', '(?<name>a)', '', 'a', 0, 2_000_000, 'UNSUPPORTED:'),
        ('modifiers-remain-explicit', '(?i:a)', '', 'a', 0, 2_000_000, 'UNSUPPORTED:'),
        ('backreferences-remain-explicit', r'(a)\1', '', 'aa', 0, 2_000_000, 'UNSUPPORTED:'),
        ('nested-lookahead-bound', '(?=' * 33 + 'a' + ')' * 33, '', 'a', 0, 2_000_000, 'LIMIT:'),
        ('assertions-share-work', '(?=(a+)+b)', '', 'a' * 24, 0, 20_000, 'LIMIT:'),
        ('backward-assertions-share-work', '(?<=(a+)+b)c', '', 'a' * 24 + 'c', 0, 100, 'LIMIT:'),
        ('compilation-spends-same-budget', '(?=a)a', '', 'a', 0, 4, 'LIMIT:'),
    ]
    with tempfile.TemporaryDirectory(prefix='own-browser-regexp-') as directory:
        directory = Path(directory)
        source = directory / 'probe.rs'
        binary = directory / ('probe.exe' if os.name == 'nt' else 'probe')
        source.write_text(PROBE.replace('SOURCE', json.dumps(str(ROOT / 'src/regexp.rs'), ensure_ascii=False)), encoding='utf-8')
        subprocess.run([args.rustc, '--edition=2021', '-O', str(source), '-o', str(binary)], check=True)
        tests = [(*row, 2_000_000) for row in comparisons] + [row[1:6] for row in bounded]
        serialized = ''.join(f'{p.encode().hex()}\t{f}\t{s.encode().hex()}\t{start}\t{budget}\n'
                             for p, f, s, start, budget in tests)
        actual = subprocess.run([str(binary)], input=serialized, text=True, capture_output=True, check=True, timeout=30).stdout.splitlines()
        expected = subprocess.run([args.node, '-e', NODE], input=json.dumps(comparisons), text=True, capture_output=True, check=True, timeout=30).stdout.splitlines()
    failures = []
    if len(actual) != len(tests) or len(expected) != len(comparisons):
        raise RuntimeError('Probe returned an incomplete result list')
    for row, got, want in zip(comparisons, actual, expected):
        if got != want:
            failures.append({'case': row, 'actual': got, 'expected': want})
    for row, got in zip(bounded, actual[len(comparisons):]):
        if not got.startswith(row[-1]):
            failures.append({'case': row[0], 'actual': got, 'expected_prefix': row[-1]})
    report = {'comparison_count': len(comparisons), 'boundary_count': len(bounded),
              'regexp_sha256': hashlib.sha256((ROOT / 'src/regexp.rs').read_bytes()).hexdigest(),
              'failures': failures, 'passed': not failures}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report, indent=2))
    return int(bool(failures))

if __name__ == '__main__':
    raise SystemExit(main())
