#!/usr/bin/env python3
"""Check one TLS 1.3 HelloRetryRequest against an independent loopback OpenSSL server.

Development only: Python standard library, OpenSSL CLI and a host Rust compiler.
The Rust probe is reused from check_tls_handshake.py without importing its Python
dependencies. It compiles the actual browser TLS source, authenticates the local
fixture, exchanges HTTP and requires authenticated close_notify. OpenSSL is never
invoked or linked by the browser. No public-network request or trust-store change
is made. This is a bounded interoperability check, not a TLS conformance suite.
"""

from __future__ import annotations

import argparse
import ast
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tools" / "https-demo"
MODULES = (
    "crypto_hash", "crypto_sha512", "crypto_rsa", "crypto_ec", "crypto_aead",
    "crypto_aes_gcm", "crypto_x25519", "tls_crypto", "x509", "tls",
)
TLS_MODULES = MODULES + ("crypto_p256_ecdh", "tls12", "tls12_crypto", "tls12_cbc", "tls_retry")
SUITES = (
    ("TLS_CHACHA20_POLY1305_SHA256", 0x1303),
    ("TLS_AES_128_GCM_SHA256", 0x1301),
    ("TLS_AES_256_GCM_SHA384", 0x1302),
)
RETRY_RANDOM = bytes.fromhex("cf21ad74e59a6111be1d8c021e65b891c2a211167abb8c5e079e09e2c8a8339c")


def compile_probe(directory, rustc):
    tree = ast.parse((ROOT / "tools" / "check_tls_handshake.py").read_text(encoding="utf-8"))
    assignments = [node for node in tree.body if isinstance(node, ast.Assign)
                   and any(isinstance(target, ast.Name) and target.id == "PROBE" for target in node.targets)]
    assert len(assignments) == 1, "Expected one original TLS probe template"
    source = ast.literal_eval(assignments[0].value)
    for name in MODULES:
        source = source.replace("__" + name.upper() + "__", json.dumps(str(ROOT / "src" / (name + ".rs"))))
    path = directory / "probe.rs"
    executable = directory / ("probe.exe" if os.name == "nt" else "probe")
    path.write_text(source, encoding="utf-8")
    subprocess.run([rustc, "--edition=2021", "-O", "-C", "overflow-checks=yes", str(path), "-o", str(executable)],
                   check=True, timeout=60)
    return executable


def split_identity(directory):
    blocks = re.findall(rb"-----BEGIN CERTIFICATE-----.*?-----END CERTIFICATE-----",
                        (FIXTURES / "server-chain.pem").read_bytes(), re.S)
    assert len(blocks) == 2, "Expected a localhost leaf and one intermediate"
    leaf, chain = directory / "leaf.pem", directory / "issuer.pem"
    leaf.write_bytes(blocks[0] + b"\n")
    chain.write_bytes(blocks[1] + b"\n")
    return leaf, chain


@contextmanager
def server(openssl, suite, group, stateless, directory, identity, label):
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    trace = directory / (label + ".trace")
    log_path = directory / (label + ".log")
    command = [openssl, "s_server", "-tls1_3", "-ciphersuites", suite,
               "-groups", group, "-sigalgs", "rsa_pss_rsae_sha256",
               "-cert", str(identity[0]), "-cert_chain", str(identity[1]),
               "-key", str(FIXTURES / "server-key.pem"),
               "-accept", f"127.0.0.1:{port}", "-alpn", "http/1.1",
               "-num_tickets", "0", "-WWW", "-quiet", "-msg", "-msgfile", str(trace)]
    if stateless:
        command.append("-stateless")
    with log_path.open("wb") as log:
        process = subprocess.Popen(command, cwd=directory, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 5
            while True:
                if process.poll() is not None:
                    raise RuntimeError("OpenSSL exited: " + log_path.read_text(errors="replace")[-2000:])
                try:
                    with socket.create_connection(("127.0.0.1", port), timeout=0.1):
                        break
                except OSError:
                    if time.monotonic() >= deadline:
                        raise RuntimeError("OpenSSL did not start within five seconds")
                    time.sleep(0.02)
            yield port, trace, log_path
        finally:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=3)


def trace_messages(path):
    messages, current = [], None
    for line in path.read_text(errors="replace").splitlines():
        header = re.match(r"^(<<<|>>>) .*?, Handshake \[length [0-9a-fA-F]+\], (\w+)\s*$", line)
        if header:
            current = [header.group(1), header.group(2), bytearray()]
            messages.append(current)
        elif current is not None and re.fullmatch(r"\s*(?:[0-9a-fA-F]{2}\s+)*[0-9a-fA-F]{2}\s*", line):
            current[2].extend(bytes.fromhex(line))
        else:
            current = None
    return [(direction, name, bytes(data)) for direction, name, data in messages]


def hello_fields(message, client):
    assert message[0] == (1 if client else 2)
    assert int.from_bytes(message[1:4], "big") + 4 == len(message)
    assert message[4:6] == b"\x03\x03"
    offset = 38
    session_length = message[offset]
    offset += 1 + session_length
    if client:
        size = int.from_bytes(message[offset:offset + 2], "big")
        offset += 2 + size
        offset += 1 + message[offset]
        suite = None
    else:
        suite = int.from_bytes(message[offset:offset + 2], "big")
        assert message[offset + 2] == 0
        offset += 3
    prefix = message[4:offset]
    size = int.from_bytes(message[offset:offset + 2], "big")
    offset += 2
    assert offset + size == len(message)
    extensions = {}
    while offset < len(message):
        kind = int.from_bytes(message[offset:offset + 2], "big")
        size = int.from_bytes(message[offset + 2:offset + 4], "big")
        offset += 4
        assert kind not in extensions and offset + size <= len(message)
        extensions[kind] = message[offset:offset + size]
        offset += size
    return prefix, extensions, suite


def key_shares(extension):
    assert int.from_bytes(extension[:2], "big") == len(extension) - 2
    result, offset = [], 2
    while offset < len(extension):
        group = int.from_bytes(extension[offset:offset + 2], "big")
        size = int.from_bytes(extension[offset + 2:offset + 4], "big")
        offset += 4
        assert offset + size <= len(extension)
        result.append((group, extension[offset:offset + size]))
        offset += size
    return result


def verify_trace(path, wire_suite, group, stateless, trusted):
    messages = trace_messages(path)
    clients = [data for direction, name, data in messages if direction == "<<<" and name == "ClientHello"]
    servers = [data for direction, name, data in messages if direction == ">>>" and name == "ServerHello"]
    if stateless and len(clients) == len(servers) == 1 and servers[0][6:38] != RETRY_RANDOM:
        _, first, _ = hello_fields(clients[0], True)
        _, final, final_suite = hello_fields(servers[0], False)
        shares = key_shares(first[51])
        assert group == "X25519" and len(shares) == 1 and shares[0][0] == 29 and len(shares[0][1]) == 32
        assert final_suite == wire_suite and final[43] == b"\x03\x04" and final[51][:2] == b"\x00\x1d"
        return {"hello_retry_requests": 0, "client_hellos": 1, "initial_group": "X25519", "final_group": group,
                "cookie_echo_verified": False, "skip_reason": "OpenSSL -stateless with its HTTP server did not issue HelloRetryRequest; cookie interoperability was not exercised"}
    assert len(clients) == 2 and len(servers) == 2, "Expected exactly two ClientHellos and two ServerHellos"
    assert servers[0][6:38] == RETRY_RANDOM and servers[1][6:38] != RETRY_RANDOM, "Expected exactly one HelloRetryRequest"
    before, first, _ = hello_fields(clients[0], True)
    after, second, _ = hello_fields(clients[1], True)
    _, retry, retry_suite = hello_fields(servers[0], False)
    _, final, final_suite = hello_fields(servers[1], False)
    assert before == after, "ClientHello fixed fields changed on retry"
    assert retry_suite == final_suite == wire_suite, "Retry/final cipher mismatch"
    assert retry[43] == final[43] == b"\x03\x04", "Expected TLS 1.3"
    assert [(kind, value) for kind, value in first.items() if kind not in (44, 51)] == [
        (kind, value) for kind, value in second.items() if kind not in (44, 51)
    ], "Unaffected extensions changed"
    initial = key_shares(first[51])
    assert len(initial) == 1 and initial[0][0] == 29 and len(initial[0][1]) == 32, "Initial share must stay X25519"
    selected = 23 if group == "P-256" else 29
    shares = key_shares(second[51])
    assert len(shares) == 1 and shares[0][0] == selected
    assert len(shares[0][1]) == (65 if selected == 23 else 32)
    if selected == 23:
        assert retry[51] == b"\x00\x17" and shares[0][1][0] == 4
    else:
        assert 51 not in retry and second[51] == first[51], "Cookie-only retry changed X25519 share"
    assert int.from_bytes(final[51][:2], "big") == selected
    cookie = retry.get(44)
    if stateless:
        assert cookie and len(cookie) > 2 and second.get(44) == cookie, "Cookie was not echoed exactly"
        assert int.from_bytes(cookie[:2], "big") == len(cookie) - 2
    else:
        assert cookie is None and 44 not in second
    finished = sum(direction == "<<<" and name == "Finished" for direction, name, _ in messages)
    assert finished == (1 if trusted else 0), "Unexpected client Finished count"
    return {"hello_retry_requests": 1, "initial_group": "X25519", "final_group": group,
            "client_hellos": len(clients), "client_finished": finished,
            "cookie_bytes": len(cookie) - 2 if cookie else 0, "cookie_echo_verified": bool(cookie),
            "unchanged_client_hello_fields": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustc", default=os.environ.get("RUSTC", "rustc"))
    parser.add_argument("--openssl", default=shutil.which("openssl"))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if not args.openssl:
        parser.error("OpenSSL CLI is required for the development checker")
    version = subprocess.check_output([args.openssl, "version"], text=True, timeout=10).strip()
    results = []
    with tempfile.TemporaryDirectory(prefix="chezcat-tls-retry-") as temporary:
        directory = Path(temporary)
        (directory / "secure").write_text("HTTPS_FIXTURE_OK\n", encoding="ascii")
        identity = split_identity(directory)
        probe = compile_probe(directory, args.rustc)
        cases = [(suite, wire, "P-256", False, True) for suite, wire in SUITES]
        cases += [(SUITES[0][0], SUITES[0][1], "P-256", False, False),
                  (SUITES[0][0], SUITES[0][1], "X25519", True, True)]
        for index, (suite, wire, group, stateless, trusted) in enumerate(cases):
            label = f"case-{index}"
            result = {"suite": suite, "requested_group": group, "stateless_cookie": stateless,
                      "trusted_certificate": trusted, "passed": False}
            try:
                with server(args.openssl, suite, group, stateless, directory, identity, label) as (port, trace, log):
                    root = FIXTURES / ("ca.pem" if trusted else "ec-ca.pem")
                    completed = subprocess.run([str(probe), str(port), str(root), os.urandom(64).hex()],
                                               capture_output=True, text=True, timeout=20)
                    result["probe_stdout"] = completed.stdout.strip()
                    result["probe_stderr"] = completed.stderr.strip()
                    if trusted:
                        assert completed.returncode == 0 and "AUTH " in completed.stdout and "PASS HTTP marker" in completed.stdout, "Authenticated HTTP failed"
                    else:
                        assert completed.returncode != 0 and "AUTH " not in completed.stdout and "PASS " not in completed.stdout, "Untrusted certificate accepted"
                        assert "trust" in completed.stderr.lower(), "Failure was not trust validation"
                result.update(verify_trace(trace, wire, group, stateless, trusted))
                result["authenticated_http"] = trusted
                if result.get("skip_reason"):
                    result["skipped"] = True
                else:
                    result["passed"] = True
            except Exception as error:
                result["error"] = str(error)
                if "log" in locals() and log.exists():
                    result["server_log_tail"] = log.read_text(errors="replace")[-2000:]
            results.append(result)
            status = "PASS " if result["passed"] else "SKIP " if result.get("skipped") else "FAIL "
            print(status + suite + f" group={group} cookie={stateless} trusted={trusted}", flush=True)
    report = {"passed": sum(result["passed"] for result in results), "total": len(results),
              "skipped": sum(bool(result.get("skipped")) for result in results),
              "openssl": version, "scope": "Loopback original fixture; actual Rust TLS source; no browser UI or public network",
              "source_sha256": {name + ".rs": hashlib.sha256((ROOT / "src" / (name + ".rs")).read_bytes()).hexdigest()
                                for name in TLS_MODULES}, "cases": results}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    if report["passed"] + report["skipped"] != report["total"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
