#!/usr/bin/env python3
"""Independent loopback HTTP/HTTPS persistence checks for the actual Rust loader.

Python's socket/ssl servers count connections, authenticated handshakes and HTTP
requests. Original fixtures cover framing, poisoned/stale streams, POST replay,
origin/trust isolation, cancellation, and renewed deadlines. No public sites or
third-party packages are used. The optional baseline comparison serves identical
HTML/CSS/JS/images with a declared artificial connection-setup delay; timings are
fixture measurements, not predictions for any public website.
"""
from __future__ import annotations

import argparse
from collections import Counter
import gzip
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import socket
import ssl
import subprocess
import tempfile
import threading
import time

import check_loading

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tools/https-demo'
BODY = b'CONNECTION_REUSE_OK'
PROBE = r'''
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
fn show(response: &http::Response) {
    for event in &response.events { println!("{event}"); }
}
fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let base = url::parse(&args[1])?.0;
    let trust = trust::Store::from_file(std::path::Path::new(&args[2]))?;
    let cancel = AtomicBool::new(false);
    let pool = http::Pool::new(&cancel, &trust);
    let get = |path: &str, mode| -> Result<http::Response, String> {
        let target = base.resolve(path)?.0;
        let response = pool.fetch(&target, 65536, None, mode)?;
        show(&response);
        Ok(response)
    };
    let normal = http::FetchMode::Normal;
    let check = |response: http::Response| {
        assert!(response.status == 204 && response.body.is_empty()
            || response.status == 200 && response.body == b"CONNECTION_REUSE_OK");
    };
    match args[3].as_str() {
        "sequence" => {
            for path in ["/prime", "/chunked", "/gzip", "/empty", "/ok"] { check(get(path, normal)?); }
        }
        "two" | "idle" => {
            get(&args[4], normal)?;
            if args[3] == "idle" { std::thread::sleep(Duration::from_millis(150)); }
            check(get("/ok", normal)?);
        }
        "error" | "retry" | "retry-fail" | "post" => {
            check(get("/prime", normal)?);
            let response = if args[3] == "post" {
                pool.fetch_request(&base.resolve("/post")?.0,
                    &http::Request::FormPost { body: b"fixture=one_submission".to_vec(), origin: Some(base.clone()) },
                    65536, None, normal)
            } else { get(&args[4], normal) };
            if args[3] == "retry" { check(response?); }
            else {
                let error = response.expect_err("hostile/stale fixture unexpectedly succeeded");
                println!("EXPECTED ERROR {error}");
            }
            if args[3] != "retry-fail" { check(get("/ok", normal)?); }
        }
        "full" => {
            check(get("/same", normal)?);
            check(get("/same", normal)?);
            check(get("/same", http::FetchMode::FullReload)?);
        }
        "deadline" => {
            check(get("/prime", normal)?);
            std::thread::sleep(Duration::from_millis(20_050));
            check(get("/ok", normal)?);
        }
        "origins" => {
            check(get("/prime", normal)?);
            let other = url::parse(&args[4])?.0;
            check(pool.fetch(&other.resolve("/ok")?.0, 65536, None, normal)?);
            let error = pool.fetch(&other.resolve("/blocked")?.0, 65536, Some(&base), normal).unwrap_err();
            assert!(error.contains("Cross-origin"), "{error}");
            check(get("/ok", normal)?);
        }
        "hosts" => {
            check(get("/prime", normal)?);
            let alias = url::parse(&base.display().replacen("localhost", "127.0.0.1", 1))?.0;
            check(pool.fetch(&alias.resolve("/alias")?.0, 65536, None, normal)?);
            let error = pool.fetch(&alias.resolve("/blocked")?.0, 65536, Some(&base), normal).unwrap_err();
            assert!(error.contains("Cross-origin"), "{error}");
            check(get("/ok", normal)?);
        }
        "trust" => {
            check(get("/prime", normal)?);
            let other = trust::Store::from_file(std::path::Path::new(&args[4]))?;
            let isolated = http::Pool::new(&cancel, &other);
            let error = isolated.fetch(&base.resolve("/untrusted")?.0, 65536, None, normal).unwrap_err();
            assert!(error.contains("certificate"), "{error}");
            check(get("/ok", normal)?);
        }
        "cancel" => {
            let started = Instant::now();
            std::thread::scope(|scope| {
                let mut workers = Vec::new();
                for _ in 0..4 {
                    let pool = &pool;
                    let target = base.resolve("/cancel").unwrap().0;
                    workers.push(scope.spawn(move || pool.fetch(&target, 65536, None, normal)));
                }
                std::thread::sleep(Duration::from_millis(300));
                cancel.store(true, Ordering::Release);
                for worker in workers {
                    let error = worker.join().unwrap().unwrap_err();
                    assert!(error.contains("cancel"), "{error}");
                }
            });
            assert!(started.elapsed() < Duration::from_secs(2));
            println!("CANCEL_MS {}", started.elapsed().as_millis());
        }
        "load" | "load-repeat" => {
            for _ in 0..if args[3] == "load-repeat" { 2 } else { 1 } {
                let location = navigation::address(&base.resolve("/page")?.0.display(), &navigation::Location::home())?;
                let loaded = loader::load(&location, &cancel, &|_| {}, &trust, normal, true)?;
                assert_eq!(loaded.images.len(), 12);
                assert_eq!(loaded.styles.len(), 1);
                assert_eq!(loaded.scripts.len(), 1);
                assert_eq!(loaded.skipped, 0);
                for event in loaded.events { println!("{event}"); }
            }
        }
        _ => panic!("unknown probe mode"),
    }
    println!("PASS");
    Ok(())
}
fn main() { if let Err(error) = run() { eprintln!("ERROR {error}"); std::process::exit(1); } }
'''


def compile_probe(directory: Path, rustc: str) -> Path:
    source = '#![allow(dead_code)]\n' + '\n'.join(
        f'#[path={json.dumps(str(ROOT / "src" / (name + ".rs")))}] mod {name};'
        for name in check_loading.MODULES)
    path = directory / 'reuse_probe.rs'
    path.write_text(source + '\n' + PROBE, encoding='utf-8')
    binary = directory / ('reuse_probe.exe' if os.name == 'nt' else 'reuse_probe')
    subprocess.run([rustc, '--edition=2021', '-O', str(path), '-o', str(binary)], check=True)
    return binary


def tls_context(profile):
    if profile == 'http':
        return None
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    version = ssl.TLSVersion.TLSv1_3 if profile == 'tls13' else ssl.TLSVersion.TLSv1_2
    context.minimum_version = context.maximum_version = version
    if profile != 'tls13':
        context.set_ciphers('ECDHE-RSA-AES256-SHA384' if profile == 'tls12_cbc' else 'ECDHE-RSA-AES128-GCM-SHA256')
    context.set_ecdh_curve('X25519' if profile == 'tls13' else 'prime256v1')
    context.set_alpn_protocols(['http/1.1'])
    context.load_cert_chain(FIXTURES / 'server-chain.pem', FIXTURES / 'server-key.pem')
    return context


class Server(ThreadingHTTPServer):
    daemon_threads = True
    block_on_close = False

    def __init__(self, profile, delay=0):
        super().__init__(('127.0.0.1', 0), Handler)
        self.profile, self.context, self.delay = profile, tls_context(profile), delay
        self.lock, self.stop_event = threading.Lock(), threading.Event()
        self.connections = self.handshakes = self.active = self.peak = 0
        self.records, self.peers, self.errors = [], [], []
        self.thread = threading.Thread(target=self.serve_forever, kwargs={'poll_interval': .02}, daemon=True)
        self.thread.start()

    @property
    def base(self):
        return f'{"http" if self.context is None else "https"}://localhost:{self.server_port}'

    def handle_error(self, request, client_address):
        # Deliberate certificate rejection and truncated connections are cases.
        pass

    def report(self):
        with self.lock:
            return {'connections': self.connections, 'tls_handshakes': self.handshakes,
                    'peak_connections': self.peak, 'requests': list(self.records), 'server_errors': list(self.errors)}

    def close(self):
        self.stop_event.set()
        self.shutdown()
        with self.lock:
            peers = list(self.peers)
        for peer in peers:
            try:
                peer.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            peer.close()
        self.server_close()
        self.thread.join(timeout=2)


class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def setup(self):
        self.request.settimeout(24)
        self.request.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        with self.server.lock:
            self.server.connections += 1
            self.connection_id = self.server.connections
            self.server.active += 1
            self.server.peak = max(self.server.peak, self.server.active)
        if self.server.delay:
            time.sleep(self.server.delay)
        try:
            if self.server.context is not None:
                self.request = self.server.context.wrap_socket(self.request, server_side=True)
                with self.server.lock:
                    self.server.handshakes += 1
            with self.server.lock:
                self.server.peers.append(self.request)
            super().setup()
        except Exception:
            with self.server.lock:
                self.server.active -= 1
            raise

    def finish(self):
        try:
            super().finish()
        except OSError:
            pass
        finally:
            with self.server.lock:
                self.server.active -= 1

    def log_message(self, *args):
        pass

    def abrupt(self):
        self.close_connection = True
        if isinstance(self.connection, ssl.SSLSocket):
            raw = socket.socket(fileno=self.connection.detach())
            raw.shutdown(socket.SHUT_RDWR)
            raw.close()
        else:
            self.connection.shutdown(socket.SHUT_RDWR)

    def notify_close(self):
        self.close_connection = True
        if isinstance(self.connection, ssl.SSLSocket):
            try:
                self.connection.unwrap().close()
            except OSError:
                pass
        else:
            self.connection.shutdown(socket.SHUT_RDWR)

    def do_POST(self):
        self.do_GET()

    def do_GET(self):
        body = self.rfile.read(int(self.headers.get('Content-Length', 0)))
        with self.server.lock:
            self.server.records.append({'path': self.path, 'method': self.command,
                'connection': self.connection_id, 'host': self.headers.get('Host'),
                'cache_control': self.headers.get('Cache-Control'), 'pragma': self.headers.get('Pragma'),
                'body_bytes': len(body)})
            occurrence = sum(item['path'] == self.path for item in self.server.records)
        if self.path == '/cancel':
            self.server.stop_event.wait(3)
            self.close_connection = True
            return
        if self.path == '/post' or self.path == '/stale-always' or self.path == '/stale-get' and occurrence == 1:
            self.abrupt()
            return
        if self.path == '/stale-notify' and occurrence == 1:
            self.notify_close()
            return
        if self.path == '/poison':
            assert isinstance(self.connection, ssl.SSLSocket)
            raw = socket.socket(fileno=self.connection.detach())
            raw.sendall(b'\x17\x03\x03\x00\x20' + bytes(32))
            raw.shutdown(socket.SHUT_RDWR)
            raw.close()
            self.close_connection = True
            return
        hostile = {
            '/partial': b'H',
            '/short': b'HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nshort',
            '/ambiguous': b'HTTP/1.1 200 OK\r\nContent-Length: 4\r\nTransfer-Encoding: chunked\r\n\r\nbody',
            '/bad-coding': b'HTTP/1.1 200 OK\r\nContent-Length: 4\r\nContent-Encoding: gzip\r\n\r\nbad!',
        }
        if self.path in hostile:
            self.connection.sendall(hostile[self.path])
            self.abrupt()
            return
        if self.path == '/redirect':
            self.connection.sendall(b'HTTP/1.1 302 Found\r\nLocation: /ok\r\nContent-Length: 99999\r\n\r\nignored')
            return
        if self.path == '/http10':
            self.connection.sendall(b'HTTP/1.0 200 OK\r\nConnection: keep-alive\r\nContent-Length: '
                                    + str(len(BODY)).encode() + b'\r\n\r\n' + BODY)
            return
        if self.path == '/close-framed':
            self.connection.sendall(b'HTTP/1.1 200 OK\r\n\r\n' + BODY)
            self.notify_close()
            return
        if self.path == '/tls-buffer':
            payload = b'x' * 8151
            wire = b'HTTP/1.1 200 OK\r\nContent-Length: 8151\r\n\r\n' + payload
            assert len(wire) == 8192
            self.connection.sendall(wire + b'UNSOLICITED')
            return
        if self.path == '/chunked':
            self.connection.sendall(b'HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n'
                + f'{len(BODY):x}'.encode() + b'\r\n' + BODY + b'\r\n0\r\nX-Fixture: done\r\n\r\n')
            return
        status, mime, response = 200, 'text/plain', BODY
        extra = []
        if self.path == '/empty':
            status, response = 204, b''
        elif self.path == '/gzip':
            response = gzip.compress(BODY, mtime=0)
            extra.append(('Content-Encoding', 'gzip'))
        elif self.path == '/connection-close':
            extra.extend([('Connection', 'keep-alive'), ('Connection', 'cLoSe')])
        elif self.path == '/page':
            mime = 'text/html'
            response = (b'<!doctype html><title>Persistent resources</title><link rel="stylesheet" href="/style.css">'
                b'<script src="/code.js"></script><p>Original local fixture</p>'
                + b''.join(f'<img src="/image-{i}.ppm">'.encode() for i in range(12)))
        elif self.path == '/style.css':
            mime, response = 'text/css', b'p{color:#203040}'
        elif self.path == '/code.js':
            mime, response = 'text/javascript', b'var originalConnectionFixture=1;'
        elif self.path.startswith('/image-'):
            mime, response = 'image/x-portable-pixmap', b'P3\n1 1\n255\n32 64 96\n'
            time.sleep(.01)
        self.send_response(status)
        if status != 204:
            self.send_header('Content-Length', str(len(response)))
            self.send_header('Content-Type', mime)
        for key, value in extra:
            self.send_header(key, value)
        self.end_headers()
        self.wfile.write(response + (b'UNSOLICITED' if self.path == '/extra' else b''))
        if self.path in ['/pending', '/closed-idle']:
            time.sleep(.05)
            if self.path == '/pending':
                self.connection.sendall(b'UNSOLICITED')
            else:
                self.abrupt()


def run_case(binary, profile, mode, path='', *, delay=0, extra_server=False, legacy=False):
    server, second = Server(profile, delay), None
    try:
        if extra_server:
            second = Server(profile)
            path = second.base
        command = [str(binary), server.base, str(FIXTURES / 'demo-roots.pem'), mode]
        if legacy:
            command = [str(binary), server.base + '/page', str(FIXTURES / 'demo-roots.pem'), 'normal']
        elif path:
            command.append(path)
        started = time.monotonic()
        process = subprocess.run(command, capture_output=True, text=True, timeout=30)
        elapsed = time.monotonic() - started
        assert process.returncode == 0, (profile, mode, path, process.stdout, process.stderr)
        result = {'profile': profile, 'mode': mode, 'path': path, 'elapsed_ms': round(elapsed * 1000, 3),
                  **server.report(), 'output': process.stdout}
        if second:
            result['second_origin'] = second.report()
        return result
    finally:
        server.close()
        if second:
            second.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rustc', default='rustc')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--baseline-source', type=Path)
    parser.add_argument('--skip-deadline', action='store_true', help='skip the 20-second deadline renewal case')
    args = parser.parse_args()
    results, benchmarks = [], []
    with tempfile.TemporaryDirectory(prefix='own-browser-persistence-') as temporary:
        directory = Path(temporary)
        binary = compile_probe(directory, args.rustc)
        for profile in ['http', 'tls13', 'tls12_gcm', 'tls12_cbc']:
            result = run_case(binary, profile, 'sequence')
            assert result['connections'] == 1 and len(result['requests']) == 5, result
            assert result['output'].count('HTTP pool reused') == 4, result
            results.append(result)
            for path in ['/connection-close', '/extra', '/http10', '/close-framed', '/tls-buffer']:
                result = run_case(binary, profile, 'two', path)
                assert result['connections'] == 2 and len(result['requests']) == 2, result
                results.append(result)
            for path in ['/pending', '/closed-idle']:
                result = run_case(binary, profile, 'idle', path)
                assert result['connections'] == 2 and len(result['requests']) == 2, result
                assert 'idle connection with pending data or a closed transport' in result['output'], result
                assert 'retrying once' not in result['output'], result
                results.append(result)
            result = run_case(binary, profile, 'two', '/redirect')
            assert result['connections'] == 2 and len(result['requests']) == 3, result
            results.append(result)
            for path in ['/partial', '/short', '/ambiguous', '/bad-coding'] + ([] if profile == 'http' else ['/poison']):
                result = run_case(binary, profile, 'error', path)
                assert result['connections'] == 2 and len(result['requests']) == 3, result
                assert 'retrying once' not in result['output'], result
                results.append(result)
            for path in ['/stale-get', '/stale-notify']:
                result = run_case(binary, profile, 'retry', path)
                assert result['connections'] == 2 and len(result['requests']) == 4, result
                assert result['output'].count('retrying once') == 1, result
                results.append(result)
            result = run_case(binary, profile, 'retry-fail', '/stale-always')
            assert result['connections'] == 2 and len(result['requests']) == 3, result
            assert result['output'].count('retrying once') == 1, result
            results.append(result)
            result = run_case(binary, profile, 'post')
            assert result['connections'] == 2 and len(result['requests']) == 3, result
            assert [r['method'] for r in result['requests']] == ['GET', 'POST', 'GET'], result
            assert 'retrying once' not in result['output'], result
            results.append(result)
            result = run_case(binary, profile, 'full')
            assert result['connections'] == 1 and len(result['requests']) == 3, result
            assert result['requests'][-1]['cache_control'] == result['requests'][-1]['pragma'] == 'no-cache', result
            results.append(result)
            result = run_case(binary, profile, 'origins', extra_server=True)
            assert result['connections'] == 1 and len(result['requests']) == 2, result
            assert result['second_origin']['connections'] == 1 and len(result['second_origin']['requests']) == 1, result
            results.append(result)
            result = run_case(binary, profile, 'hosts')
            assert result['connections'] == 2 and len(result['requests']) == 3, result
            assert result['requests'][0]['connection'] == result['requests'][2]['connection'], result
            assert result['requests'][1]['connection'] != result['requests'][0]['connection'], result
            results.append(result)
            result = run_case(binary, profile, 'cancel')
            assert result['connections'] == result['peak_connections'] == 3, result
            assert len(result['requests']) == 3, result
            results.append(result)
            result = run_case(binary, profile, 'load-repeat')
            assert result['connections'] == 6 and len(result['requests']) == 30, result
            assert all(value == 2 for value in Counter(r['path'] for r in result['requests']).values()), result
            results.append(result)
            if profile != 'http':
                result = run_case(binary, profile, 'trust', str(FIXTURES / 'ec-ca.pem'))
                assert result['connections'] == 2 and len(result['requests']) == 2, result
                results.append(result)
            print(f'PASS {profile}: persistence, discarded bodies, bounded retries, POST, origins, cancellation and load isolation', flush=True)
        if not args.skip_deadline:
            result = run_case(binary, 'tls12_cbc', 'deadline')
            assert result['connections'] == 1 and len(result['requests']) == 2, result
            results.append(result)
            print('PASS renewed deadline on the same TLS connection after the first request deadline expired', flush=True)
        if args.baseline_source:
            baseline = directory / 'baseline'
            check_loading.compile_probe(args.baseline_source.resolve(), baseline, args.rustc)
            for profile in ['http', 'tls13', 'tls12_cbc']:
                before = run_case(baseline, profile, 'load', delay=.08, legacy=True)
                after = run_case(binary, profile, 'load', delay=.08)
                assert len(before['requests']) == len(after['requests']) == 15
                assert before['connections'] == 15 and after['connections'] == 3, (before, after)
                assert before['elapsed_ms'] > after['elapsed_ms'], (before, after)
                benchmarks.append({'setup_delay_ms': 80, 'before': before, 'after': after})
                print(f'PASS {profile} fixture: {before["connections"]} -> {after["connections"]} connections; '
                      f'{before["elapsed_ms"]:.1f} -> {after["elapsed_ms"]:.1f} ms', flush=True)
    report = {'checks': len(results), 'results': results, 'benchmarks': benchmarks,
              'source_sha256': {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
                               for name in ['src/http.rs', 'src/tls.rs', 'src/tls12.rs', 'src/loader.rs', 'tools/check_connection_reuse.py']}}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(f'PASS {len(results)} independent connection reuse checks; {len(benchmarks)} baseline comparisons')


if __name__ == '__main__':
    main()
