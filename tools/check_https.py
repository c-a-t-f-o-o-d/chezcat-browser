#!/usr/bin/env python3
"""Independent full-browser HTTPS integration checks.

Development only: Python 3 ssl/OpenSSL serves TLS; cryptography generates signed
negative certificate fixtures. Neither is used by the browser. Build the browser
first and pass --browser target/release/chezcat (or ChezCat.exe).
All servers and requests stay on loopback. No CA is installed anywhere.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
from datetime import datetime, timedelta, timezone
import ipaddress
import json
from pathlib import Path
import ssl
import subprocess
import tempfile
import threading

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.x509.oid import ExtendedKeyUsageOID, NameOID

from http_fixture_server import FixtureHandler, FixtureServer
from https_fixture_server import FIXTURES, TlsFixtureHandler, TlsFixtureServer, context


class QuietHandler(TlsFixtureHandler):
    def log_message(self, fmt, *args):
        pass

    def route(self, send_body):
        self.server.requests.append(self.path)
        self.server.negotiated.append((self.connection.version(), self.connection.cipher()[0]))
        if self.path == "/policy":
            body = (
                "<title>HTTPS resource policy</title><h1>Only this origin</h1>"
                "<link rel='stylesheet' href='/pages/styles.css'>"
                "<img src='/pages/images/landscape.png'>"
                f"<img src='http://127.0.0.1:{self.server.plain_port}/blocked-image'>"
                f"<link rel='stylesheet' href='http://127.0.0.1:{self.server.plain_port}/blocked-style'>"
                f"<img src='https://127.0.0.1:{self.server.server_address[1]}/foreign-host'>"
            ).encode()
            self.reply(body, send_body)
        elif self.path == "/downgrade":
            self.redirect(f"http://127.0.0.1:{self.server.plain_port}/blocked-redirect", send_body)
        elif self.path == "/raw-eof":
            self.send_fixture_headers(200)
            self.wfile.write(b"<title>Truncated TLS</title><p>No authenticated close</p>")
            self.wfile.flush()
            self.connection.fixture_raw_eof = True
        else:
            super().route(send_body)


class CheckServer(TlsFixtureServer):
    def shutdown_request(self, request):
        if getattr(request, "fixture_raw_eof", False):
            # Intentionally violate TLS closure without discarding HTTP framing.
            FixtureServer.shutdown_request(self, request)
        else:
            super().shutdown_request(request)


class PlainHandler(FixtureHandler):
    def log_message(self, fmt, *args):
        pass

    def route(self, send_body):
        self.server.requests.append(self.path)
        if self.path == "/upgrade":
            self.redirect(f"https://localhost:{self.server.tls_port}/secure", send_body)
        else:
            super().route(send_body)


@contextmanager
def serving(server):
    thread = threading.Thread(target=server.serve_forever, kwargs={"poll_interval": 0.05}, daemon=True)
    thread.start()
    try:
        yield server
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=3)


def make_certificates(directory):
    now = datetime.now(timezone.utc)
    start, end = now - timedelta(days=1), now + timedelta(days=30)
    root_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    leaf_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    rogue_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    name = lambda value: x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, value)])
    root_name = name("ChezCat ephemeral integration CA")

    def builder(subject, public_key, issuer, before=start, after=end, ca=False):
        return (x509.CertificateBuilder().subject_name(subject).issuer_name(issuer)
                .public_key(public_key).serial_number(x509.random_serial_number())
                .not_valid_before(before).not_valid_after(after)
                .add_extension(x509.BasicConstraints(ca=ca, path_length=0 if ca else None), True)
                .add_extension(x509.KeyUsage(digital_signature=not ca, content_commitment=False,
                    key_encipherment=False, data_encipherment=False, key_agreement=False,
                    key_cert_sign=ca, crl_sign=ca, encipher_only=None, decipher_only=None), True))

    root = builder(root_name, root_key.public_key(), root_name, ca=True).sign(root_key, hashes.SHA256())
    root_path = directory / "root.pem"
    root_path.write_bytes(root.public_bytes(serialization.Encoding.PEM))
    key_path = directory / "leaf-key.pem"
    key_path.write_bytes(leaf_key.private_bytes(serialization.Encoding.PEM,
        serialization.PrivateFormat.PKCS8, serialization.NoEncryption()))
    cases = {}
    for kind in ("valid", "wrong-name", "expired", "future", "no-san", "forged-signature"):
        before, after = start, end
        if kind == "expired":
            before, after = now - timedelta(days=20), now - timedelta(days=10)
        if kind == "future":
            before, after = now + timedelta(days=10), now + timedelta(days=20)
        leaf = builder(name("localhost"), leaf_key.public_key(), root_name, before, after)
        if kind != "no-san":
            identities = [x509.DNSName("wrong.invalid")] if kind == "wrong-name" else [
                x509.DNSName("localhost"), x509.IPAddress(ipaddress.ip_address("127.0.0.1"))]
            leaf = leaf.add_extension(x509.SubjectAlternativeName(identities), False)
        leaf = leaf.add_extension(x509.ExtendedKeyUsage([ExtendedKeyUsageOID.SERVER_AUTH]), False)
        leaf = leaf.sign(rogue_key if kind == "forged-signature" else root_key, hashes.SHA256())
        file = directory / f"{kind}.pem"
        file.write_bytes(leaf.public_bytes(serialization.Encoding.PEM))
        cases[kind] = file
    # Model a widely deployed certificate shape: the server appends a root
    # cross-signed by an older CA, but the client already trusts the same root
    # key directly. The older CA deliberately is not in this trust file.
    for suffix, cross_before, cross_after in [
        ("cross-sign", start, end),
        ("expired-cross-sign", now - timedelta(days=20), now - timedelta(days=10)),
    ]:
        cross = builder(root_name, root_key.public_key(), name("Untrusted legacy CA"),
                        cross_before, cross_after, ca=True).sign(rogue_key, hashes.SHA256())
        for leaf_kind in ("valid", "wrong-name", "forged-signature"):
            kind = leaf_kind + "-" + suffix
            file = directory / f"{kind}.pem"
            file.write_bytes(cases[leaf_kind].read_bytes() + cross.public_bytes(serialization.Encoding.PEM))
            cases[kind] = file
    return root_path, key_path, cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--browser", required=True, type=Path)
    parser.add_argument("--output", type=Path, help="Optional JSON report path")
    args = parser.parse_args()
    browser = args.browser.resolve()
    if not browser.is_file():
        parser.error("--browser must name the built browser executable")
    results = []
    with tempfile.TemporaryDirectory(prefix="chezcat-https-") as temporary:
        directory = Path(temporary)

        def run(label, url, trust, success=True, contains=None, extra=()):
            image = directory / f"{label}.ppm"
            process = subprocess.run([str(browser), "--trust-file", str(trust), url,
                "--render", str(image), *extra], capture_output=True, text=True, timeout=35)
            if success:
                assert process.returncode == 0, (label, process.stderr)
                with image.open("rb") as stream:
                    assert stream.read(3) == b"P6\n", label
            else:
                assert process.returncode != 0 and not image.exists(), (label, "unexpected success")
                if contains:
                    assert contains.lower() in process.stderr.lower(), (label, process.stderr)
            results.append({"case": label, "passed": True,
                            "error": process.stderr.strip() if not success else None})

        plain = FixtureServer(("127.0.0.1", 0), PlainHandler)
        plain.requests = []
        secure = CheckServer(("127.0.0.1", 0), QuietHandler,
                             context(FIXTURES / "server-chain.pem", FIXTURES / "server-key.pem"))
        secure.requests, secure.negotiated = [], []
        secure.plain_port, plain.tls_port = plain.server_address[1], secure.server_address[1]
        base = f"https://localhost:{plain.tls_port}"
        trust = FIXTURES / "ca.pem"
        with serving(plain), serving(secure):
            for label, path in [("demo", "/"), ("plain-body", "/secure"),
                ("chunked", "/chunked"), ("authenticated-eof", "/close-delimited"),
                ("redirect", "/redirect"), ("gallery", "/pages/gallery.html"),
                ("get-values", "/form-result?q=caf%C3%A9+%2B&x=one&x=two"),
                ("http-status", "/status/404"), ("resource-policy", "/policy")]:
                run(label, base + path, trust)
            run("ip-san", f"https://127.0.0.1:{plain.tls_port}/secure", trust)
            run("bare-host-cli-uses-https", f"localhost:{plain.tls_port}/secure", trust)
            run("bare-ip-cli-uses-https", f"127.0.0.1:{plain.tls_port}/secure", trust)
            run("resources-view", base + "/secure", trust, extra=("--resources",))
            run("source-view", base + "/secure", trust, extra=("--source",))
            run("http-upgrade", f"http://127.0.0.1:{secure.plain_port}/upgrade", trust)
            run("downgrade-blocked", base + "/downgrade", trust, False, "HTTPS redirect")
            run("truncated-http", base + "/truncated", trust, False, "body")
            run("unauthenticated-eof", base + "/raw-eof", trust, False, "close_notify")
            before = len(secure.requests)
            run("untrusted-ca", base + "/secure", FIXTURES.parents[1] / "trust/roots.pem", False, "trusted root")
            assert len(secure.requests) == before, "HTTP was sent before a trusted certificate"
            assert plain.requests == ["/upgrade"], plain.requests
            assert "/foreign-host" not in secure.requests, secure.requests
            assert all(v == "TLSv1.3" and c in {"TLS_CHACHA20_POLY1305_SHA256",
                       "TLS_AES_128_GCM_SHA256", "TLS_AES_256_GCM_SHA384"}
                       for v, c in secure.negotiated), secure.negotiated

        for identity in ("p256", "p384"):
            server = CheckServer(("127.0.0.1", 0), QuietHandler,
                context(FIXTURES / f"server-{identity}-chain.pem",
                        FIXTURES / f"server-{identity}-key.pem"))
            server.requests, server.negotiated = [], []
            base = f"https://localhost:{server.server_address[1]}"
            with serving(server):
                run(identity + "-demo", base + "/", FIXTURES / "demo-roots.pem")
                assert "/pages/styles.css" in server.requests, server.requests
                assert "/pages/images/landscape.png" in server.requests, server.requests
                run(identity + "-resources", base + "/secure", FIXTURES / "demo-roots.pem",
                    extra=("--resources",))
                before = len(server.requests)
                run(identity + "-untrusted", base + "/secure", FIXTURES / "ca.pem",
                    False, "trusted root")
                assert len(server.requests) == before, "HTTP was sent before an EC root was trusted"
                assert all(version == "TLSv1.3" for version, _ in server.negotiated)

        root, key, certificates = make_certificates(directory)
        for kind, certificate in certificates.items():
            server = CheckServer(("127.0.0.1", 0), QuietHandler, context(certificate, key))
            server.requests, server.negotiated = [], []
            with serving(server):
                valid = kind == "valid" or kind.startswith("valid-")
                run("certificate-" + kind, f"https://localhost:{server.server_address[1]}/secure",
                    root, valid, "certificate" if not valid else None)
                if not valid:
                    assert not server.requests, (kind, "HTTP leaked before certificate validation")

        old = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        old.minimum_version = old.maximum_version = ssl.TLSVersion.TLSv1_2
        old.set_ciphers("ECDHE-RSA-AES128-GCM-SHA256:ECDHE-RSA-AES256-GCM-SHA384")
        old.set_ecdh_curve("prime256v1")
        old.load_cert_chain(str(FIXTURES / "server-chain.pem"), str(FIXTURES / "server-key.pem"))
        server = CheckServer(("127.0.0.1", 0), QuietHandler, old)
        server.requests, server.negotiated = [], []
        with serving(server):
            run("tls12-supported", f"https://localhost:{server.server_address[1]}/secure", trust)
            assert server.requests == ["/secure", "/pages/styles.css"], server.requests
            assert all(version == "TLSv1.2" for version, _ in server.negotiated)

    report = {"passed": len(results), "server": ssl.OPENSSL_VERSION, "cases": results}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"PASS: {len(results)} browser HTTPS scenarios against {ssl.OPENSSL_VERSION}")
    for result in results:
        print("  " + result["case"])


if __name__ == "__main__":
    main()
