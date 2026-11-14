#!/usr/bin/env python3
"""Original responsive-image cases against the actual Rust selector.

Development only; Python's standard library and rustc suffice. No requests are
made and no browser engine is embedded. Expected values follow WHATWG HTML's
srcset/sizes/picture algorithms, CSS Values 4, and the implemented Media Queries
4 subset. Unsupported-feature checks deliberately assert a documented fallback,
not support for the omitted feature. Run from any directory:

    python tools/check_responsive_images.py --output responsive-report.json

The selector's captured viewport is a document-load snapshot. This does not
claim resize refetch, lazy loading, AVIF support, or a complete media engine.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import html
import json
import math
from pathlib import Path
import random
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]


@dataclass
class Case:
    name: str
    markup: str
    expected: str | None
    density: float | None = None
    width: float = 800
    height: float = 600
    dpr: float = 1
    source: str | None = None
    diagnostic: bool = False


def img(srcset='', sizes='', src='', **attrs):
    values = dict(attrs)
    if srcset:
        values['srcset'] = srcset
    if sizes:
        values['sizes'] = sizes
    if src:
        values['src'] = src
    return '<img ' + ' '.join(f'{k}="{html.escape(str(v), quote=True)}"' for k, v in values.items()) + '>'


def picture(source, fallback='fallback.png'):
    return '<picture>' + source + img(src=fallback) + '</picture>'


def cases():
    out = []
    def add(name, markup, expected, density=None, **kw):
        out.append(Case(name, markup, expected, density, **kw))

    add('ordinary-src', img(src=' ordinary.png '), 'ordinary.png', 1)
    add('missing-src', img(), None)
    add('empty-src', '<img src=" \n\t">', None)
    add('only-2x-no-src', img('two.png 2x'), 'two.png', 2)
    add('src-supplies-1x', img('two.png 2x', src='one.png'), 'one.png', 1)
    add('explicit-1x-replaces-src', img('first.png 1x, other.png 2x', src='unused.png'), 'first.png', 1)
    add('implicit-1x-replaces-src', img('first.png, other.png 2x', src='unused.png'), 'first.png', 1)
    add('duplicate-density-first-wins', img('first.png 2x, other.png 2x'), 'first.png', 2)
    add('duplicate-width-first-wins', img('first.png 800w, other.png 800w'), 'first.png', 1)
    add('sort-density', img('large.png 3x, small.png 1x, middle.png 2x'), 'middle.png', 2, dpr=1.5)
    add('highest-if-no-enough-density', img('one.png 1x, two.png 2x'), 'two.png', 2, dpr=4)
    add('valid-zero-density', img('zero.png 0x'), 'zero.png', 0)
    add('src-outweighs-zero-density', img('zero.png 0x', src='one.png'), 'one.png', 1)
    add('fractional-density', img('half.png .5x, one.png 1x'), 'half.png', .5, dpr=.5)
    add('exponent-density', img('two.png 2e0x, ten.png 1e1x'), 'two.png', 2)
    add('negative-zero-density', img('zero.png -0x'), 'zero.png', 0)
    add('huge-finite-density', img('huge.png 1e100x'), 'huge.png', 1e100)
    add('html-entity-url', '<img srcset="first.png?x=1&amp;y=2 1x, two.png 2x">', 'first.png?x=1&y=2', 1)
    add('unicode-url', img('pictures/猫.png 1x, two.png 2x'), 'pictures/猫.png', 1)
    add('data-url-comma', img('data:image/png;base64,AAAA 1x, two.png 2x'), 'data:image/png;base64,AAAA', 1)
    add('unescaped-data-comma', img('data:image/svg+xml,%3Csvg%3E,%3C/svg%3E 1x, two.png 2x'), 'data:image/svg+xml,%3Csvg%3E,%3C/svg%3E', 1)
    add('no-whitespace-comma-in-url', img('one.png,two.png 1x, three.png 2x'), 'one.png,two.png', 1)
    add('url-trailing-commas', img('first.png,,, two.png 2x'), 'first.png', 1)
    add('url-final-comma', img('last.png,'), 'last.png', 1)
    add('leading-empty-candidates', img(', , first.png 1x, , two.png 2x'), 'first.png', 1)
    add('parenthesized-descriptor-comma', img('bad.png future(a,b), good.png 1x'), 'good.png', 1, diagnostic=True)
    add('unclosed-descriptor-recovery', img('bad.png future(a,b), good.png 1x, broken.png future(', src='unused.png'), 'good.png', 1, diagnostic=True)
    add('width-height-pair', img('pair.png 400w 300h', '400px'), 'pair.png', 1)
    add('height-width-order', img('pair.png 300h 400w', '400px'), 'pair.png', 1)
    add('mixed-candidate-kinds', img('half.png 400w, one.png 1x', '800px', src='unused.png'), 'one.png', 1)
    add('normalization-duplicate-keeps-first', img('width.png 800w, density.png 1x', '800px'), 'width.png', 1)
    add('no-width-src-fallback', img('small.png 400w', '800px', 'unused.png'), 'small.png', .5)
    add('default-size-100vw', img('small.png 400w, large.png 800w', src='unused.png'), 'large.png', 1)
    add('zero-size-infinite-density', img('zero.png 400w', '0'), 'zero.png', math.inf)
    add('zero-size-first-normalized-candidate', img('small.png 400w, large.png 800w', '0px'), 'small.png', math.inf)

    bad = ['-1x', '+1x', '1.x', '1e+x', 'Infinityx', 'NaNx', '1e999x', '0w', '-1w', '+1w',
           '1.5w', '1e2w', '1W', '1X', '1q', '1x 2x', '1w 2w', '100w 2x', '2x 100w',
           '1h', '100w 0h', '100w 1h 2h', '1x 2h', '1h 2x', '(1x)', '100w broken',
           '1\u00a0x', '1x\x0b2x', '1x 1x 1x 1x 1x']
    for index, descriptor in enumerate(bad):
        add(f'invalid-descriptor-{index}', img(f'bad.png {descriptor}, good.png 1x', src='unused.png'), 'good.png', 1, diagnostic=True)

    # A single 800w candidate exposes the exact normalized density independently
    # of candidate ranking. Defaults are an 800 x 600 CSS viewport at 1x.
    lengths = {'400px': 400, '50vw': 400, '50vh': 300, '50vmin': 300, '50vmax': 400,
               '25em': 400, '25rem': 400, '1in': 96, '2.54cm': 96, '25.4mm': 96,
               '101.6q': 96, '72pt': 96, '6pc': 96, '+400px': 400, '4e2px': 400,
               'calc(50vw - 20px)': 380, 'calc(50vw + 20px)': 420,
               'calc(100vw/2)': 400, 'calc(2*200px)': 400,
               'calc(200px - -200px)': 400, 'calc(2 * (100px + 100px))': 400,
               'min(600px, 50vw)': 400, 'max(200px, 50vw)': 400,
               'clamp(200px, 50vw, 600px)': 400, 'clamp(600px, 50vw, 200px)': 600,
               'min(600px, max(200px, 50vw))': 400,
               '(max-width:800px) 400px, 800px': 400,
               '(max-width:799px) 100px, 400px': 400,
               '(width:800px)400px': 400,
               'not (width:801px) 400px, 800px': 400,
               '(width:800px)/**/400px': 400,
               '400px, 800px': 400,
               'AUTO, 400px': 400,
               'calc(100px + /* comment */ 300px)': 400}
    for text, size in lengths.items():
        add('sizes-' + text, img('one.png 800w', text), 'one.png', 800 / size)
    for text in ['calc(-20px)', 'calc(20px - 40px)', 'min(-1px, 20px)', 'clamp(-10px, -2px, -1px)']:
        add('negative-math-clamps-' + text, img('zero.png 800w', text), 'zero.png', math.inf)
    for text in ['-20px', '50%', '400', 'calc(0)', 'var(--size)', 'env(foo)', 'calc(10px*20px)',
                 'calc(10px / 0)', 'calc(10px+20px)', 'calc(10px +20px)', 'calc(10px+ 20px)',
                 'calc(10px/**/+/**/20px)', 'calc (400px)', 'calc/**/(400px)', '1.px',
                 'calc(- (400px))', '10px+20px', '(400px)', 'min()', 'clamp(1px, 2px)',
                 '10px 400px', 'screen and (width:800px) 400px', '(unknown:yes) 400px']:
        add('invalid-size-fallback-' + text, img('one.png 800w', text), 'one.png', 1, diagnostic=True)
    add('invalid-early-size-recovery', img('one.png 800w', '50%, (width:800px) 400px, 800px'), 'one.png', 2, diagnostic=True)
    add('unknown-size-media-recovery', img('one.png 800w', '(prefers-color-scheme:dark) 100px, 400px'), 'one.png', 2, diagnostic=True)

    for width in [320, 500, 800, 1200]:
        for dpr in [1, 1.25, 1.5, 2, 3]:
            # Intended source-size: full viewport <=500, otherwise half viewport.
            slot = width if width <= 500 else width / 2
            options = [320, 640, 1280]
            needed = slot * dpr
            chosen = next((w for w in options if w >= needed), options[-1])
            add(f'viewport-width-grid-{width}-{dpr}', img('s.png 320w, m.png 640w, l.png 1280w', '(max-width:500px) 100vw, 50vw'),
                {320:'s.png',640:'m.png',1280:'l.png'}[chosen], chosen / slot, width=width, dpr=dpr)

    source = '<source srcset="chosen.png 1x, sharp.png 2x" type="image/png">'
    add('picture-basic', picture(source), 'chosen.png', 1, source='source')
    add('picture-density', picture(source), 'sharp.png', 2, dpr=2, source='source')
    add('picture-order-first-wins', picture(source + '<source srcset="later.png">'), 'chosen.png', 1)
    add('picture-after-img-ignored', '<picture><img src="fallback.png"><source srcset="late.png"></picture>', 'fallback.png', 1, source='img')
    add('picture-nested-img-not-direct-child', '<picture><source srcset="unused.png"><span><img src="fallback.png"></span></picture>', 'fallback.png', 1)
    add('picture-img-fallback-after-invalid-source', picture('<source srcset="bad.png -1x">'), 'fallback.png', 1, diagnostic=True)
    add('picture-empty-source-skipped', picture('<source srcset=""><source srcset="yes.png">'), 'yes.png', 1)
    add('picture-src-attribute-not-srcset', picture('<source src="unused.png">'), 'fallback.png', 1)
    add('picture-supported-mime-parameters', picture('<source type="IMAGE/PNG; ignored=value" srcset="yes.png">'), 'yes.png', 1)
    add('picture-source-own-sizes', '<picture><source srcset="yes.png 800w" sizes="400px"><img src="unused.png" sizes="800px"></picture>', 'yes.png', 2)
    for mime in ['image/avif', 'image/jxl', 'image/heic', 'image/heif', 'video/webm', 'text/plain', '']:
        add('picture-unsupported-' + mime, picture(f'<source type="{mime}" srcset="unsupported.bin">'+source), 'chosen.png', 1, diagnostic=True)
    for mime in ['image/png', 'image/jpeg', 'image/gif', 'image/bmp', 'image/webp', 'image/tiff',
                 'image/svg+xml', 'image/x-icon', 'image/qoi', 'image/x-portable-anymap',
                 'image/tga', 'image/x-tga', 'image/pcx', 'image/x-pcx']:
        add('picture-supported-' + mime, picture(f'<source type="{mime}" srcset="yes.bin">'), 'yes.bin', 1)

    yes = ['screen', 'all', 'not print', 'only screen', 'screen and (width:800px)',
           '(min-width:50em)', '(max-width:800px)', '(width)', '(height)', '(orientation)',
           '(aspect-ratio:4/3)', '(orientation:landscape)', '(resolution:1dppx)',
           '(resolution:96dpi)', '(resolution:37.79527559055118dpcm)',
           '(width >= 800px)', '(800px <= width)', '(500px < width <= 800px)',
           '(1000px > width >= 800px)', '(calc(400px * 2) <= width)',
           '(width:calc(400px * 2))', '(width:800px) and (height:600px)',
           '(width:801px) or (height:600px)', '((width:800px) and (height:600px))',
           '(not (width:799px)) and (height:600px)', 'screen and not (width:799px)',
           'print, screen', '(unknown:yes), (width:800px)', '(width:800px)and (height:600px)',
           '(width:800px) and/**/(height:600px)', '/* comment */']
    no = ['print', 'not screen', '(width:799px)', '(height:599px)', '(orientation:portrait)',
          '(min-resolution:2dppx)', '(width < 800px)', '(799px >= width)', '(800px < width <= 1000px)',
          'not screen and (width:800px)', '(width:799px) and (height:600px)']
    invalid = ['(min-width)', 'screen or (width:800px)', 'screen and (width:799px) or (height:600px)',
               '(width:800px) and not (height:599px)', 'not (width:799px) and (height:600px)',
               'not not (width:800px)', 'not(width:799px)', '(width:800px) and(height:600px)',
               '(width:800px) and (height:600px) or (width:1px)', '(width:800px)garbage',
               '(width > = 799px)', '(800px <= width >= 100px)', '(prefers-color-scheme:dark)',
               'not (prefers-color-scheme:dark)', '(width:10px+790px)', '(width:800.px)',
               '(width:calc (800px))', 'screen and', 'only (width:800px)']
    for index, text in enumerate(yes + no + invalid):
        expected = 'yes.png' if text in yes else 'fallback.png'
        add('picture-media-' + str(index) + '-' + text, picture('<source srcset="yes.png" media="'+html.escape(text, quote=True)+'">'), expected, 1, diagnostic=text in invalid)

    add('srcset-attribute-limit', img('x' * (64 * 1024 + 1), src='fallback.png'), 'fallback.png', 1, diagnostic=True)
    add('sizes-attribute-limit', img('one.png 800w', 'x' * 4097), 'one.png', 1, diagnostic=True)
    many = ', '.join(f'choice{n}.png {n}x' for n in range(1, 130))
    add('candidate-limit', img(many), 'choice64.png', 64, dpr=100, diagnostic=True)
    # The environment is itself bounded to64x; the first128 candidates remain.
    sources = '<source type="image/avif" srcset="unused.avif">' * 64 + '<source srcset="late.png">'
    add('picture-source-limit', picture(sources), 'fallback.png', 1, diagnostic=True)
    add('media-nesting-limit', picture('<source srcset="bad.png" media="'+'('*40+'width:800px'+')'*40+'">'), 'fallback.png', 1, diagnostic=True)
    add('sizes-token-limit', img('one.png 800w', 'calc('+ '1px + '*200+'1px)'), 'one.png', 1, diagnostic=True)
    return out


def compile_probe(directory: Path, rustc: str):
    source = directory / 'responsive_probe.rs'
    probe = directory / ('responsive-probe.exe' if __import__('os').name == 'nt' else 'responsive-probe')
    source.write_text('''#![allow(dead_code)]
#[path=HTML_PATH] mod html;
#[path=SELECTOR_PATH] mod image_selection;
use std::io::{self,BufRead};
fn decode(s:&str)->String { String::from_utf8((0..s.len()).step_by(2).map(|i|u8::from_str_radix(&s[i..i+2],16).unwrap()).collect()).unwrap() }
fn hex(s:&str)->String {s.bytes().map(|b|format!("{b:02x}")).collect()}
fn main() {
 for line in io::stdin().lock().lines() {
  let line=line.unwrap();let values:Vec<_>=line.splitn(4,'\\t').collect();
  let document=html::parse(&decode(values[3]));
  let environment=image_selection::Environment{viewport_width:values[0].parse().unwrap(),viewport_height:values[1].parse().unwrap(),density:values[2].parse().unwrap()};
  let node=document.nodes.iter().position(|n|matches!(&n.kind,html::NodeKind::Element{tag,..}if tag=="img")).unwrap();
  let selected=image_selection::Selector::new(&document,environment).select(node);
  let notes=selected.diagnostics.iter().map(|s|hex(s)).collect::<Vec<_>>().join(",");
  if let Some(image)=selected.image {
   let tag=match &document.nodes[image.source_node].kind {html::NodeKind::Element{tag,..}=>tag.as_str(),_=>"?"};
   println!("{}\\t{}\\t{}\\t{}\\t{}",hex(&image.url),image.density,tag,image.responsive,notes);
  }else{println!("-\\t1\\t-\\tfalse\\t{notes}");}
 }
}
'''.replace('HTML_PATH', json.dumps(str(ROOT / 'src/html.rs'))).replace('SELECTOR_PATH', json.dumps(str(ROOT / 'src/image_selection.rs'))), encoding='utf-8')
    build = subprocess.run([rustc, '--edition=2021', '-O', str(source), '-o', str(probe)], capture_output=True, text=True, timeout=90)
    if build.returncode:
        raise RuntimeError(build.stderr)
    return probe


def run(probe: Path, samples):
    payload = ''.join(f'{c.width}\t{c.height}\t{c.dpr}\t{c.markup.encode().hex()}\n' for c in samples)
    result = subprocess.run([str(probe)], input=payload, text=True, capture_output=True, timeout=30)
    if result.returncode:
        raise RuntimeError(result.stderr)
    lines = result.stdout.splitlines()
    assert len(lines) == len(samples), (len(lines), len(samples))
    parsed = []
    for line in lines:
        url, density, source, responsive, notes = line.split('\t')
        parsed.append({'url': None if url == '-' else bytes.fromhex(url).decode(), 'density': float(density),
                       'source': source, 'responsive': responsive == 'true',
                       'diagnostics': [bytes.fromhex(n).decode() for n in notes.split(',') if n]})
    return parsed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--work-dir', type=Path)
    parser.add_argument('--rustc', default=shutil.which('rustc') or 'rustc')
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='chezcat-responsive-') as temporary:
        directory = (args.work_dir or Path(temporary)).resolve()
        directory.mkdir(parents=True, exist_ok=True)
        probe = compile_probe(directory, args.rustc)
        samples = cases()
        results = run(probe, samples)
        failures = []
        report = []
        for c, r in zip(samples, results):
            okay = r['url'] == c.expected
            if c.density is not None:
                okay &= math.isclose(r['density'], c.density, rel_tol=1e-12, abs_tol=1e-12)
            if c.source is not None:
                okay &= r['source'] == c.source
            if c.diagnostic:
                okay &= bool(r['diagnostics'])
            okay &= len(r['diagnostics']) <= 8 and all(len(n) < 180 for n in r['diagnostics'])
            entry = {'case': c.name, 'passed': bool(okay), 'selected': r['url'],
                     'density': str(r['density']) if math.isinf(r['density']) else r['density'],
                     'source': r['source'], 'diagnostics': r['diagnostics']}
            report.append(entry)
            if not okay:
                failures.append({'case': c.name, 'expected_url': c.expected,
                                 'expected_density': c.density, 'actual': r})
        # Deterministic mutation checks exercise the bounded tokenizer and
        # repeated delimiter recovery; they assert no crashes or leaked payloads.
        rng = random.Random(290029)
        malformed = []
        alphabet = 'abc012345 ,()xwh.-+e\t\r\n\x0b\u00a0猫'
        for n in range(512):
            text = ''.join(rng.choice(alphabet) for _ in range(rng.randrange(1, 600)))
            malformed.append(Case(f'mutation-{n}', img('private-payload-'+text, text[:120], 'fallback.png'), None))
        mutations = run(probe, malformed)
        assert all(len(r['diagnostics']) <= 8 for r in mutations)
        assert all('private-payload' not in note for r in mutations for note in r['diagnostics'])
        summary = {'passed': not failures, 'normative_and_subset_cases': len(samples),
                   'bounded_mutations': len(mutations), 'network_requests': 0,
                   'sources': ['https://html.spec.whatwg.org/multipage/images.html',
                               'https://www.w3.org/TR/css-values-4/',
                               'https://www.w3.org/TR/mediaqueries-4/'],
                   'results': report, 'failures': failures}
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(summary, indent=2) + '\n', encoding='utf-8')
        if failures:
            print(json.dumps(failures, indent=2))
            raise SystemExit(1)
        print(f'PASS: {len(samples)} responsive-selection cases; {len(mutations)} bounded mutations; no network requests.')


if __name__ == '__main__':
    main()
