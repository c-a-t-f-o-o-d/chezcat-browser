#!/usr/bin/env python3
"""Check production HTTP framing against independent local TLS servers.

Requires Rust and Python's standard-library ssl/OpenSSL. No external Python
packages or public network requests are used. The small Rust probe imports the
browser's actual HTTP/TLS source, with the bundled public TEST CA supplied only
to that process. No CA is installed anywhere.

Complete Content-Length, chunked and bodyless messages survive an abrupt TCP
close. Unframed or short bodies remain errors without authenticated TLS closure.
The request-sensitive case models a server that selects chunking only when the
client permits HTTP/1.1 persistence; the browser does not pool connections.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import socket
import ssl
import subprocess
import tempfile
import threading


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tools" / "https-demo"
BODY = b"HTTP_CLOSURE_FIXTURE_OK"
FORM = b"fixture=private-body-value"
MODULES = [
    "http", "url", "compression", "random", "tls", "trust", "x509", "tls_crypto",
    "crypto_hash", "crypto_sha512", "crypto_aead", "crypto_aes_gcm",
    "crypto_x25519", "crypto_rsa", "crypto_ec",
]
PROBE = r'''
fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let target = url::parse(&args[1])?.0;
    let trust = trust::Store::from_file(std::path::Path::new(&args[2]))?;
    let request = if args[3] == "POST" {
        http::Request::FormPost {
            body: b"fixture=private-body-value".to_vec(),
            origin: Some(target.clone()),
        }
    } else { http::Request::Get };
    let response = http::fetch_request(&target, &request, 65536, None,
        &std::sync::atomic::AtomicBool::new(false), &trust, http::FetchMode::Normal)?;
    if response.status == 204 {
        if !response.body.is_empty() { return Err("204 contained a body".into()); }
    } else if response.status != 200 || response.body != b"HTTP_CLOSURE_FIXTURE_OK" {
        return Err(format!("Unexpected HTTP response: {} / {} bytes",
                           response.status, response.body.len()));
    }
    println!("PASS status={} bytes={}", response.status, response.body.len());
    for event in response.events { println!("{event}"); }
    Ok(())
}
fn main() {
    if let Err(error) = run() { eprintln!("ERROR {error}"); std::process::exit(1); }
}
'''


def compile_probe(directory: Path, rustc: str) -> Path:
    source = "#![allow(dead_code)]\n" + "\n".join(
        f"#[path={json.dumps(str(ROOT / 'src' / (name + '.rs')))}] mod {name};"
        for name in MODULES)
    path = directory / "probe.rs"
    path.write_text(source + "\n" + PROBE, encoding="utf-8")
    binary = directory / ("probe.exe" if os.name == "nt" else "probe")
    subprocess.run([rustc, "--edition=2021", "-O", str(path), "-o", str(binary)], check=True)
    return binary


def tls_context(profile: str) -> ssl.SSLContext:
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    version = ssl.TLSVersion.TLSv1_3 if profile == "tls13" else ssl.TLSVersion.TLSv1_2
    context.minimum_version = context.maximum_version = version
    if profile != "tls13":
        context.set_ciphers("ECDHE-RSA-AES256-SHA384" if profile == "tls12_cbc"
                            else "ECDHE-RSA-AES128-GCM-SHA256")
    context.set_ecdh_curve("X25519" if profile == "tls13" else "prime256v1")
    context.load_cert_chain(FIXTURES / "server-chain.pem", FIXTURES / "server-key.pem")
    context.set_alpn_protocols(["http/1.1"])
    return context


def request(stream: ssl.SSLSocket) -> tuple[str, str, dict[str, str], bytes]:
    wire = b""
    while b"\r\n\r\n" not in wire:
        part = stream.recv(4096)
        if not part:
            raise EOFError("Client closed before request headers")
        wire += part
        assert len(wire) < 32768, "Oversized fixture request"
    head, body = wire.split(b"\r\n\r\n", 1)
    lines = head.decode("ascii").split("\r\n")
    method, target, version = lines[0].split(" ")
    assert version == "HTTP/1.1"
    headers = dict(line.lower().split(": ", 1) for line in lines[1:])
    length = int(headers.get("content-length", "0"))
    assert length < 1024
    while len(body) < length:
        part = stream.recv(length - len(body))
        if not part:
            raise EOFError("Client closed before request body")
        body += part
    return method, target, headers, body


def response(case: str, target: str, headers: dict[str, str]) -> tuple[bytes, bool]:
    prefix = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n"
    length = b"Content-Length: " + str(len(BODY)).encode() + b"\r\n\r\n" + BODY
    chunk = (b"Transfer-Encoding: chunked\r\n\r\n" + f"{len(BODY):x}".encode()
             + b"\r\n" + BODY + b"\r\n0\r\nX-End: ignored\r\n\r\n")
    if case == "length_complete":
        return prefix + length, False
    if case == "chunked_complete":
        return prefix + chunk, False
    if case == "bodyless_complete":
        return b"HTTP/1.1 204 No Content\r\n\r\n", False
    if case == "default_persistence":
        return prefix + (b"\r\n" + BODY if headers.get("connection") == "close" else chunk), False
    if case.startswith("unframed_"):
        return prefix + b"Connection: close\r\n\r\n" + BODY, case == "unframed_authenticated"
    if case == "headers_partial":
        return prefix + b"Content-Length: 100\r\n", False
    if case in ("length_partial", "post_partial"):
        return prefix + b"Content-Length: 100\r\n\r\n" + BODY, False
    if case == "chunked_partial":
        return (prefix + b"Transfer-Encoding: chunked\r\n\r\n" + f"{len(BODY):x}".encode()
                + b"\r\n" + BODY + b"\r\n"), False
    if case == "no_http_bytes":
        return b"", False
    if case == "redirect_partial":
        if target.startswith("/start?"):
            return (b"HTTP/1.1 302 Found\r\nLocation: /result?token=private-query-value\r\n"
                    b"Content-Length: 0\r\n\r\n"), False
        return prefix + b"Content-Length: 100\r\n\r\n" + BODY, False
    raise ValueError(case)


def check_case(binary: Path, profile: str, case: str, success: bool) -> dict:
    context = tls_context(profile)
    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen(5)
    listener.settimeout(0.05)
    port = listener.getsockname()[1]
    done = threading.Event()
    observed = {"connections": 0, "methods": [], "server_errors": []}

    def serve() -> None:
        while not done.is_set():
            try:
                raw, _ = listener.accept()
            except socket.timeout:
                continue
            observed["connections"] += 1
            try:
                raw.settimeout(5)
                with context.wrap_socket(raw, server_side=True) as stream:
                    observed["protocol"] = stream.version()
                    observed["cipher"] = stream.cipher()[0]
                    method, target, headers, body = request(stream)
                    observed["methods"].append(method)
                    assert "connection" not in headers, "Request unnecessarily forces connection closure"
                    assert body == (FORM if method == "POST" else b"")
                    wire, notify = response(case, target, headers)
                    # Several TLS records force the client to preserve parser
                    # state across reads rather than receiving one full message.
                    for offset in range(0, len(wire), 23):
                        stream.sendall(wire[offset:offset + 23])
                    if notify:
                        try:
                            stream.unwrap().close()
                        except OSError:
                            # The peer can send its closure and close the socket
                            # before OpenSSL finishes waiting for that alert.
                            pass
                    else:
                        transport = socket.socket(fileno=stream.detach())
                        transport.shutdown(socket.SHUT_RDWR)
                        transport.close()
            except Exception as error:
                observed["server_errors"].append(str(error))
                raw.close()

    thread = threading.Thread(target=serve, daemon=True)
    thread.start()
    method = "POST" if case == "post_partial" else "GET"
    try:
        result = subprocess.run([str(binary), f"https://localhost:{port}/start?token=private-query-value",
                                 str(FIXTURES / "ca.pem"), method],
                                capture_output=True, text=True, timeout=15)
    finally:
        done.set()
        thread.join(timeout=6)
        listener.close()
    label = f"{profile}/{case}"
    assert not thread.is_alive(), (label, "Fixture did not stop")
    assert not observed["server_errors"], (label, observed)
    expected_requests = 2 if case == "redirect_partial" else 1
    assert observed["connections"] == expected_requests, (label, "Unexpected replay", observed)
    assert observed["methods"] == [method] * expected_requests, (label, observed)
    assert (result.returncode == 0) == success, (label, result.stdout, result.stderr)
    if success:
        assert "HTTP framing:" in result.stdout, (label, result.stdout)
    else:
        error = result.stderr
        assert "without close_notify" in error, (label, error)
        assert "HTTP response progress: stage=" in error, (label, error)
        assert "Connection activity:" in error and "TLS 1." in error, (label, error)
        assert "HTTP request sent; awaiting response." in error, (label, error)
        assert "private-query-value" not in error and "private-body-value" not in error, (label, error)
        if case == "redirect_partial":
            assert "HTTP 302 redirect to" in error, (label, error)
            assert error.count("HTTP request sent; awaiting response.") == 2, (label, error)
    print(f"PASS {label}", flush=True)
    return {"case": label, "passed": True, "expected_success": success,
            "error": result.stderr.strip() if not success else None, **observed}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustc", default="rustc", help="Rust compiler executable")
    parser.add_argument("--output", type=Path, help="Optional JSON report path")
    args = parser.parse_args()
    cases = {
        "length_complete": True, "chunked_complete": True, "bodyless_complete": True,
        "unframed_authenticated": True, "default_persistence": True,
        "unframed_abrupt": False, "headers_partial": False, "length_partial": False,
        "chunked_partial": False, "no_http_bytes": False, "post_partial": False,
        "redirect_partial": False,
    }
    with tempfile.TemporaryDirectory(prefix="own-browser-http-closure-") as temporary:
        binary = compile_probe(Path(temporary), args.rustc)
        results = [check_case(binary, profile, case, success)
                   for profile in ("tls12_gcm", "tls12_cbc", "tls13")
                   for case, success in cases.items()]
    report = {"passed": len(results), "total": len(results), "cases": results}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"All {len(results)} HTTP/TLS closure checks passed.")


if __name__ == "__main__":
    main()
