#!/usr/bin/env python3
"""Force each supported TLS cipher and server proof through the full browser.

Development only. Uses an independent OpenSSL command-line server and the public
localhost test identities. No Python packages, OS trust changes or public-network
requests are needed. The browser itself never invokes OpenSSL.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import json
from pathlib import Path
import re
import shutil
import socket
import subprocess
import tempfile
import time


FIXTURES = Path(__file__).resolve().parent / "https-demo"
SUITES = ("TLS_CHACHA20_POLY1305_SHA256", "TLS_AES_128_GCM_SHA256", "TLS_AES_256_GCM_SHA384")
IDENTITIES = (
    ("rsa", "rsa_pss_rsae_sha256"),
    ("rsa", "rsa_pss_rsae_sha384"),
    ("rsa", "rsa_pss_rsae_sha512"),
    ("p256", "ecdsa_secp256r1_sha256"),
    ("p384", "ecdsa_secp384r1_sha384"),
)


def split_chain(identity, directory):
    prefix = "server" if identity == "rsa" else "server-" + identity
    encoded = (FIXTURES / (prefix + "-chain.pem")).read_bytes()
    blocks = re.findall(rb"-----BEGIN CERTIFICATE-----.*?-----END CERTIFICATE-----", encoded, re.S)
    assert len(blocks) == 2, "Expected a localhost leaf and one intermediate"
    leaf, chain = directory / (identity + "-leaf.pem"), directory / (identity + "-issuer.pem")
    leaf.write_bytes(blocks[0] + b"\n")
    chain.write_bytes(blocks[1] + b"\n")
    return leaf, chain, FIXTURES / (prefix + "-key.pem")


@contextmanager
def server(openssl, suite, signature, files, log_path):
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    leaf, chain, key = files
    with log_path.open("wb") as log:
        process = subprocess.Popen([
            openssl, "s_server", "-tls1_3", "-ciphersuites", suite,
            "-groups", "X25519", "-sigalgs", signature,
            "-cert", str(leaf), "-cert_chain", str(chain), "-key", str(key),
            "-accept", f"127.0.0.1:{port}", "-alpn", "http/1.1",
            "-num_tickets", "0", "-www", "-quiet",
        ], stdin=subprocess.DEVNULL, stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 5
            while True:
                if process.poll() is not None:
                    raise RuntimeError("OpenSSL server exited: " + log_path.read_text(errors="replace"))
                try:
                    with socket.create_connection(("127.0.0.1", port), timeout=0.1):
                        break
                except OSError:
                    if time.monotonic() >= deadline:
                        raise RuntimeError("OpenSSL server did not start within five seconds")
                    time.sleep(0.02)
            yield port
        finally:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=3)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--browser", required=True, type=Path)
    parser.add_argument("--openssl", default=shutil.which("openssl"), help="OpenSSL executable (default: PATH)")
    parser.add_argument("--output", type=Path, help="Optional JSON report path")
    args = parser.parse_args()
    browser = args.browser.resolve()
    if not browser.is_file():
        parser.error("--browser must name the built browser executable")
    if not args.openssl:
        parser.error("Install the OpenSSL command-line program or pass --openssl PATH")
    version = subprocess.check_output([args.openssl, "version"], text=True, timeout=10).strip()
    results = []
    with tempfile.TemporaryDirectory(prefix="chezcat-suites-") as temporary:
        directory = Path(temporary)
        identities = {name: split_chain(name, directory) for name in ("rsa", "p256", "p384")}
        for suite in SUITES:
            for identity, signature in IDENTITIES:
                label = suite + "-" + signature
                output = directory / (label + ".ppm")
                log = directory / (label + ".log")
                with server(args.openssl, suite, signature, identities[identity], log) as port:
                    result = subprocess.run([
                        str(browser), "--trust-file", str(FIXTURES / "demo-roots.pem"),
                        f"https://localhost:{port}/", "--render", str(output),
                    ], capture_output=True, text=True, timeout=35)
                    assert result.returncode == 0, (label, result.stderr, log.read_text(errors="replace"))
                    with output.open("rb") as stream:
                        assert stream.read(3) == b"P6\n", (label, "Missing rendered HTTP response")
                results.append({"suite": suite, "certificate_verify": signature, "passed": True})
                print("  PASS " + suite + " / " + signature, flush=True)
    report = {"passed": len(results), "server": version, "cases": results}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"PASS: {len(results)} full-browser cipher/signature combinations against {version}")


if __name__ == "__main__":
    main()
