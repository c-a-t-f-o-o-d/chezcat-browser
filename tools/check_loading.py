#!/usr/bin/env python3
"""Independent loopback checks for staged display, reuse and bounded image loading.

Uses the actual Rust loader and Python's HTTP/ssl servers. No public sites or
third-party packages are used. --baseline-source optionally compares an older
source tree with identical fixtures; timings describe these fixtures only.
"""
from __future__ import annotations

import argparse
import hashlib
from collections import Counter
from functools import lru_cache
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import ssl
import struct
import subprocess
import tempfile
import threading
import time
import zlib


ROOT = Path(__file__).resolve().parents[1]
MODULES = ('blocking compression crypto_aead crypto_aes_gcm crypto_ec crypto_hash crypto_rsa '
           'crypto_sha512 crypto_x25519 csp css editor font fonts forms html http image jpeg '
           'loader navigation random tls tls_crypto trust url x509').split()
LEGACY_PROBE = r'''
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let location = navigation::address(&args[1], &navigation::Location::home()).unwrap();
    let trust = std::sync::Arc::new(trust::Store::from_file(std::path::Path::new(&args[2])).unwrap());
    if args[3] == "replace" {
        let worker = loader::Worker::new(std::sync::Arc::new(|| {}), trust).unwrap();
        worker.submit(1, location.clone(), http::FetchMode::Normal, false);
        std::thread::sleep(std::time::Duration::from_millis(150));
        worker.submit(2, navigation::resolve("/fresh", &location).unwrap(), http::FetchMode::Normal, false);
        let started = std::time::Instant::now();
        loop {
            if let (_, Some(result)) = worker.poll() {
                assert_eq!(result.id, 2, "a replaced navigation completed");
                assert_eq!(result.result.unwrap().document.title, "Fresh page");
                println!("replacement_passed=true");
                return;
            }
            assert!(started.elapsed() < std::time::Duration::from_secs(10));
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    let mode = if args[3] == "full" { http::FetchMode::FullReload } else { http::FetchMode::Normal };
    let started = std::time::Instant::now();
    let loaded = loader::load(&location, &std::sync::atomic::AtomicBool::new(false), &|_| {}, &trust, mode, true).unwrap();
    if args[3] == "progressive" { println!("FIRST_DISPLAY_MS {}", started.elapsed().as_secs_f64()*1000.0); }
    let buffers: std::collections::HashMap<usize, usize> = loaded.images.values().map(|image| (image.pixels.as_ptr() as usize, image.pixels.len() * 4)).collect();
    let pixel_bytes: usize = buffers.values().sum();
    println!("images={} styles={} scripts={} skipped={} pixel_buffers={} pixel_bytes={}", loaded.images.len(), loaded.styles.len(), loaded.scripts.len(), loaded.skipped, buffers.len(), pixel_bytes);
    for event in loaded.events { println!("{event}"); }
}
'''


PROBE = r'''
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let location = navigation::address(&args[1], &navigation::Location::home()).unwrap();
    let trust = std::sync::Arc::new(trust::Store::from_file(std::path::Path::new(&args[2])).unwrap());
    if args[3] == "replace" || args[3] == "progressive" {
        let worker = loader::Worker::new(std::sync::Arc::new(|| {}), trust).unwrap();
        let started = std::time::Instant::now();
        worker.submit(1, location.clone(), http::FetchMode::Normal, args[3] != "replace");
        let replacing = args[3] == "replace";
        if replacing {
            std::thread::sleep(std::time::Duration::from_millis(150));
            worker.submit(2, navigation::resolve("/fresh", &location).unwrap(), http::FetchMode::Normal, false);
        }
        let id = if replacing { 2 } else { 1 };
        let mut loaded = None;
        let mut image_deliveries = 0;
        loop {
            let poll = worker.poll();
            if let Some((received_id, document)) = poll.document {
                assert_eq!(received_id, id, "a replaced document was delivered");
                assert!(loaded.is_none(), "document delivered twice");
                assert!(document.images.is_empty(), "images belong to later deliveries");
                if replacing { assert_eq!(document.document.title, "Fresh page"); }
                else {
                    assert_eq!(document.styles.len(), 1);
                    assert_eq!(document.scripts.len(), 1);
                    println!("FIRST_DISPLAY_MS {}", started.elapsed().as_secs_f64()*1000.0);
                }
                loaded = Some(document);
            }
            if let Some((received_id, update)) = poll.images {
                assert_eq!(received_id, id, "replaced image delivery");
                assert!(!replacing, "fresh fixture has no image requests");
                let document = loaded.as_mut().expect("document must precede images");
                document.images.extend(update.images);
                document.events.extend(update.events);
                document.skipped += update.skipped;
                image_deliveries += 1;
            }
            if let Some(result) = poll.completed {
                assert_eq!(result.id, id, "a replaced navigation completed");
                let end = result.result.unwrap();
                let mut document = loaded.take().expect("document must precede completion");
                document.events.extend(end.events);
                assert_eq!(document.skipped, end.skipped);
                if replacing { println!("replacement_passed=true"); }
                else {
                    assert_eq!(document.images.len(), 8);
                    assert!(image_deliveries >= 2 && image_deliveries <= 4);
                    println!("FULL_LOAD_MS {}", started.elapsed().as_secs_f64()*1000.0);
                    println!("image_deliveries={image_deliveries}");
                    for event in document.events { println!("{event}"); }
                }
                return;
            }
            assert!(started.elapsed() < std::time::Duration::from_secs(10));
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    let mode = if args[3] == "full" { http::FetchMode::FullReload } else { http::FetchMode::Normal };
    let loaded = loader::load(&location, &std::sync::atomic::AtomicBool::new(false), &|_| {}, &trust, mode, true).unwrap();
    let buffers: std::collections::HashMap<usize, usize> = loaded.images.values().map(|image| (image.pixels.as_ptr() as usize, image.pixels.len() * 4)).collect();
    let pixel_bytes: usize = buffers.values().sum();
    println!("images={} styles={} scripts={} skipped={} pixel_buffers={} pixel_bytes={}", loaded.images.len(), loaded.styles.len(), loaded.scripts.len(), loaded.skipped, buffers.len(), pixel_bytes);
    for event in loaded.events { println!("{event}"); }
}
'''


def compile_probe(root, destination, rustc):
    source = '#![allow(dead_code)]\n' + '\n'.join(
        f'#[path={json.dumps(str(root / "src" / (name + ".rs")))}] mod {name};'
        for name in MODULES if (root / 'src' / (name + '.rs')).is_file())
    path = destination.with_suffix('.rs')
    probe = PROBE if 'pub struct Poll {' in (root / 'src/loader.rs').read_text() else LEGACY_PROBE
    path.write_text(source + '\n' + probe, encoding='utf-8')
    subprocess.run([rustc, '--edition=2021', '-O', str(path), '-o', str(destination)], check=True)


@lru_cache(maxsize=4)
def solid_png(width, height):
    """Compact original fixtures exercise decoded memory without using the body budget."""
    def chunk(kind, payload):
        return (struct.pack('>I', len(payload)) + kind + payload
                + struct.pack('>I', zlib.crc32(kind + payload) & 0xffffffff))
    raster = (b'\0' + b'\x14\x3c\x64' * width) * height
    return (b'\x89PNG\r\n\x1a\n'
            + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(raster)) + chunk(b'IEND', b''))


class Server(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, context=None):
        self.context = context
        self.lock = threading.Lock()
        self.records = []
        self.active = self.peak = 0
        super().__init__(('127.0.0.1', 0), Handler)

    def process_request_thread(self, request, address):
        try:
            request.settimeout(5)
            if self.context:
                request = self.context.wrap_socket(request, server_side=True)
            self.finish_request(request, address)
        except (OSError, ssl.SSLError):
            pass
        finally:
            self.shutdown_request(request)


class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *args):
        pass

    def do_GET(self):
        server = self.server
        with server.lock:
            server.records.append({'path': self.path, 'cache_control': self.headers.get('Cache-Control'),
                                   'pragma': self.headers.get('Pragma')})
            server.active += 1
            server.peak = max(server.peak, server.active)
        try:
            self.reply()
        except (OSError, ssl.SSLError):
            pass
        finally:
            with server.lock:
                server.active -= 1

    def reply(self):
        status, mime, extra = 200, 'text/html;charset=utf-8', {}
        if self.path == '/repeat':
            body = ('<link rel=stylesheet href=/style.css><style>p{color:red}</style>'
                    '<link rel=stylesheet href="/style.css#again"><p>Repeated assets</p>'
                    + '<img src=/pixel.ppm>' * 40 + '<img src=/unsupported.gif>' * 60
                    + '<img src="/pixel.ppm#last"><script src=/code.js></script><script src=/code.js></script>').encode()
        elif self.path == '/grc-shape':
            body = ('<link rel=stylesheet href=/style.css><p>Repeated unsupported image fixture</p>'
                    + ''.join(f'<img src=/unsupported-{number}.gif>' * 20 for number in range(3))).encode()
        elif self.path == '/parallel':
            body = ('<img src=/slow-1.ppm><img src=/slow-2.ppm><img src="/slow-2.ppm#same">'
                    '<img src=/slow-3.ppm><link rel=stylesheet href=/style.css>'
                    '<img src=/slow-4.ppm><img src=/slow-5.ppm><img src=/slow-6.ppm>'
                    '<script src=/code.js></script><img src=/slow-7.ppm><img src=/slow-8.ppm>').encode()
        elif self.path == '/staged':
            body = ('<img src=/slow-1.ppm><img src=/slow-2.ppm><img src=/slow-3.ppm>'
                    '<link rel=stylesheet href=/style.css><script src=/code.js></script>'
                    '<p>The document is ready before the images.</p>'
                    + ''.join(f'<img src=/slow-{n}.ppm>' for n in range(4,9))).encode()
        elif self.path == '/policy':
            host = '127.0.0.1'  # Test documents always use localhost.
            body = (f'<img src="http://{host}:{self.server.server_port}/must-not-fetch">'
                    '<img src=/foreign-redirect><img src=/foreign-redirect>'
                    '<template><img src=/must-not-fetch></template><img src=/pixel.ppm>').encode()
        elif self.path == '/image-inputs':
            body = ('<input type=image src=/pixel.ppm><img src="/pixel.ppm#same">'
                    '<input type=ImAgE src="/pixel.ppm#button">'
                    '<input src=/must-not-fetch><input type="" src=/must-not-fetch>'
                    '<input type=text src=/must-not-fetch><input type="image " src=/must-not-fetch>'
                    '<template><input type=image src=/must-not-fetch></template>'
                    '<noscript><input type=image src=/must-not-fetch></noscript>'
                    f'<input type=image src="http://127.0.0.1:{self.server.server_port}/must-not-fetch">'
                    '<input type=image src=/foreign-redirect><img src=/foreign-redirect>').encode()
        elif self.path == '/foreign-redirect':
            status = 302
            extra['Location'] = f'http://127.0.0.1:{self.server.server_port}/must-not-fetch'
            body = b''
        elif self.path == '/budget':
            body = ''.join(f'<img src=/big-{number}.gif>' for number in range(5)).encode()
        elif self.path in ('/many-icons', '/attempt-limit'):
            count = 36 if self.path == '/many-icons' else 50
            body = ''.join(f'<img src=/tiny-{number}.ppm>' for number in range(count)).encode()
        elif self.path == '/pixel-budget':
            body = (''.join(f'<img src=/raster-{number}.png>' for number in range(4))
                    + '<img src=/raster-over.png><img src=/raster-fit.png>'
                    '<img src=/after-pixel-budget.ppm><img src="/raster-0.png#reused">').encode()
        elif self.path.startswith('/raster-'):
            heights = {'/raster-3.png': 2047, '/raster-over.png': 2, '/raster-fit.png': 1}
            mime, body = 'image/png', solid_png(2048, heights.get(self.path, 2048))
        elif self.path.startswith('/big-'):
            mime, body = 'image/gif', b'G' * (8 * 1024 * 1024)
        elif self.path == '/cancel':
            body = b'<img src=/cancel-1.ppm><img src=/cancel-2.ppm><img src=/cancel-3.ppm>'
        elif self.path == '/fresh':
            body = b'<title>Fresh page</title><p>New navigation</p>'
        elif self.path == '/style.css':
            mime, body = 'text/css', b'p{color:#123456}'
        elif self.path == '/code.js':
            mime, body = 'text/javascript', b'var repeatedScript = 1;'
        elif self.path == '/unsupported.gif' or self.path.startswith('/unsupported-'):
            time.sleep(0.03)
            mime, body = 'image/gif', b'GIF89a-not-an-implemented-format'
        elif self.path.endswith('.ppm'):
            time.sleep(0.6 if self.path.startswith('/cancel-') else 0.12 if self.path.startswith('/slow-') else 0.03)
            mime, body = 'image/x-portable-pixmap', b'P3\n1 1\n255\n20 60 100\n'
        else:
            raise AssertionError(f'Unexpected fixture request {self.path}')
        self.send_response(status)
        self.send_header('Content-Type', mime)
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Connection', 'close')
        for key, value in extra.items():
            self.send_header(key, value)
        self.end_headers()
        self.wfile.write(body)
        self.close_connection = True


def tls_context():
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = context.maximum_version = ssl.TLSVersion.TLSv1_2
    context.set_ciphers('ECDHE-RSA-AES256-SHA384')
    context.set_ecdh_curve('prime256v1')
    context.set_alpn_protocols(['http/1.1'])
    fixtures = ROOT / 'tools/https-demo'
    context.load_cert_chain(fixtures / 'server-chain.pem', fixtures / 'server-key.pem')
    return context


def run_case(binary, path, mode='normal', secure=False):
    server = Server(tls_context() if secure else None)
    thread = threading.Thread(target=server.serve_forever, kwargs={'poll_interval': 0.02}, daemon=True)
    thread.start()
    try:
        started = time.monotonic()
        process = subprocess.run([str(binary), f'{"https" if secure else "http"}://localhost:{server.server_port}{path}',
                                  str(ROOT / 'tools/https-demo/demo-roots.pem'), mode],
                                 capture_output=True, text=True, timeout=25)
        elapsed = time.monotonic() - started
        assert process.returncode == 0, process.stderr
        with server.lock:
            records, peak = list(server.records), server.peak
        return {'case': path, 'mode': mode, 'tls12_cbc': secure, 'elapsed_seconds': round(elapsed, 4),
                'requests': len(records), 'peak_requests': peak, 'records': records, 'output': process.stdout}
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=3)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rustc', default='rustc')
    parser.add_argument('--baseline-source', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    results, benchmarks = [], []
    with tempfile.TemporaryDirectory(prefix='own-browser-loading-') as temporary:
        directory = Path(temporary)
        suffix = '.exe' if os.name == 'nt' else ''
        current = directory / ('current' + suffix)
        compile_probe(ROOT, current, args.rustc)
        for secure in (False, True):
            for mode in ('normal', 'full'):
                result = run_case(current, '/repeat', mode, secure)
                counts = Counter(record['path'] for record in result['records'])
                assert counts == Counter({'/repeat': 1, '/style.css': 1, '/pixel.ppm': 1, '/unsupported.gif': 1, '/code.js': 2}), counts
                assert 'images=41 styles=2 scripts=2 skipped=60 pixel_buffers=1' in result['output']
                assert 'Resource attempt limit reached' not in result['output']
                if mode == 'full':
                    assert all(r['cache_control'] == 'no-cache' and r['pragma'] == 'no-cache' for r in result['records'])
                else:
                    assert all(r['cache_control'] is None and r['pragma'] is None for r in result['records'])
                results.append(result)
            result = run_case(current, '/parallel', secure=secure)
            paths = [r['path'] for r in result['records']]
            assert Counter(paths) == Counter(['/parallel', '/style.css', '/code.js'] + [f'/slow-{n}.ppm' for n in range(1, 9)]), paths
            assert paths[1:3] == ['/style.css', '/code.js'], paths
            assert set(paths[3:6]) == {'/slow-1.ppm', '/slow-2.ppm', '/slow-3.ppm'}, paths
            assert set(paths[6:9]) == {'/slow-4.ppm', '/slow-5.ppm', '/slow-6.ppm'}, paths
            assert 2 <= result['peak_requests'] <= 3, result
            assert 'images=9 styles=1 scripts=1 skipped=0 pixel_buffers=8' in result['output']
            results.append(result)
            result = run_case(current, '/staged', 'progressive', secure)
            assert 'DISPLAY READY:' in result['output'] and 'FIRST_DISPLAY_MS' in result['output']
            assert 2 <= result['peak_requests'] <= 3
            results.append(result)
            result = run_case(current, '/policy', secure=secure)
            counts = Counter(r['path'] for r in result['records'])
            assert counts == Counter({'/policy': 1, '/foreign-redirect': 1, '/pixel.ppm': 1}), counts
            results.append(result)
            result = run_case(current, '/image-inputs', secure=secure)
            counts = Counter(r['path'] for r in result['records'])
            assert counts == Counter({'/image-inputs': 1, '/pixel.ppm': 1, '/foreign-redirect': 1}), counts
            assert 'images=3 styles=0 scripts=0 skipped=3 pixel_buffers=1 pixel_bytes=4' in result['output']
            assert 'earlier attempt in this document load failed' in result['output']
            results.append(result)
            result = run_case(current, '/many-icons', secure=secure)
            assert Counter(r['path'] for r in result['records']) == Counter(
                ['/many-icons'] + [f'/tiny-{n}.ppm' for n in range(36)])
            assert 'images=36 styles=0 scripts=0 skipped=0 pixel_buffers=36 pixel_bytes=144' in result['output']
            assert 'Unique image count limit' not in result['output']
            assert 1 <= result['peak_requests'] <= 3
            results.append(result)
            result = run_case(current, '/attempt-limit', secure=secure)
            assert Counter(r['path'] for r in result['records']) == Counter(
                ['/attempt-limit'] + [f'/tiny-{n}.ppm' for n in range(48)])
            assert 'images=48 styles=0 scripts=0 skipped=2 pixel_buffers=48 pixel_bytes=192' in result['output']
            assert 'Resource attempt limit reached (48)' in result['output']
            assert 1 <= result['peak_requests'] <= 3
            results.append(result)
        result = run_case(current, '/pixel-budget')
        assert Counter(r['path'] for r in result['records']) == Counter(
            ['/pixel-budget'] + [f'/raster-{n}.png' for n in range(4)]
            + ['/raster-over.png', '/raster-fit.png'])
        assert 'images=6 styles=0 scripts=0 skipped=2 pixel_buffers=5 pixel_bytes=67108864' in result['output']
        assert 'Decoded image budget exceeded (64 MiB)' in result['output']
        assert 'Decoded image budget is full (64 MiB)' in result['output']
        assert 'reused decoded image from this document load' in result['output']
        assert 1 <= result['peak_requests'] <= 3
        results.append(result)
        result = run_case(current, '/budget')
        assert Counter(r['path'] for r in result['records']) == Counter(['/budget'] + [f'/big-{n}.gif' for n in range(4)])
        assert 'Page resource download budget is full (32 MiB)' in result['output']
        assert 1 <= result['peak_requests'] <= 3
        results.append(result)
        result = run_case(current, '/cancel', 'replace')
        assert 'replacement_passed=true' in result['output']
        assert '/fresh' in [r['path'] for r in result['records']]
        results.append(result)
        if args.baseline_source:
            baseline = directory / ('baseline' + suffix)
            compile_probe(args.baseline_source.resolve(), baseline, args.rustc)
            for secure in (False, True):
                for path in ('/grc-shape', '/parallel'):
                    before = run_case(baseline, path, secure=secure)
                    after = run_case(current, path, secure=secure)
                    benchmarks.append({'case': path, 'tls12_cbc': secure,
                                       'before_seconds': before['elapsed_seconds'], 'after_seconds': after['elapsed_seconds'],
                                       'before_requests': before['requests'], 'after_requests': after['requests'],
                                       'before_peak': before['peak_requests'], 'after_peak': after['peak_requests']})
                before = run_case(baseline, '/staged', 'progressive', secure)
                after = run_case(current, '/staged', 'progressive', secure)
                def first_display(result):
                    return float(next(line.split()[1] for line in result['output'].splitlines() if line.startswith('FIRST_DISPLAY_MS ')))
                benchmarks.append({'case': '/staged', 'tls12_cbc': secure,
                                   'before_first_display_ms': round(first_display(before),3),
                                   'after_first_display_ms': round(first_display(after),3),
                                   'before_seconds': before['elapsed_seconds'], 'after_seconds': after['elapsed_seconds'],
                                   'before_requests': before['requests'], 'after_requests': after['requests']})

    report = {'checks': len(results), 'passed': len(results), 'benchmarks': benchmarks, 'results': results,
              'source_sha256': {name: hashlib.sha256((ROOT / 'src' / name).read_bytes()).hexdigest()
                                for name in ('loader.rs', 'http.rs', 'image.rs')},
              'timing_scope': 'Controlled loopback fixtures with artificial image delays; not a live GRC measurement.'}
    if args.output:
        args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items() if k != 'results'}, indent=2))


if __name__ == '__main__':
    main()
