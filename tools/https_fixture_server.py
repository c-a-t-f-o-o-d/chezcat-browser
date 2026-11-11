#!/usr/bin/env python3
"""Optional loopback HTTPS demo server; never used by the browser itself.

Uses Python's ssl/OpenSSL server to independently exercise our Rust TLS client.
The bundled key and CA are public TEST fixtures. Never install them as OS roots
or add them to trust/roots.pem. Run the browser with the demo's --trust-file.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import ssl
import subprocess

from http_fixture_server import FixtureHandler, FixtureServer, page


FIXTURES = Path(__file__).resolve().parent / "https-demo"
PROJECT = FIXTURES.parent.parent


class TlsFixtureServer(FixtureServer):
    def __init__(self, address, handler, context: ssl.SSLContext):
        self.context = context
        super().__init__(address, handler)

    def get_request(self):
        stream, address = super().get_request()
        stream.settimeout(10)
        try:
            return self.context.wrap_socket(stream, server_side=True), address
        except OSError:
            stream.close()
            raise

    def shutdown_request(self, request):
        # EOF-delimited HTTPS responses need authenticated close_notify.
        # Plain TCP EOF alone must not convince a client that a body is complete.
        if isinstance(request, ssl.SSLSocket):
            try:
                request.settimeout(2)
                request = request.unwrap()
            except OSError:
                request.close()
                return
        super().shutdown_request(request)


class TlsFixtureHandler(FixtureHandler):
    server_version = "ChezCatHttpsFixture/0.30"

    def route(self, send_body: bool) -> None:
        path = self.path.split("?", 1)[0]
        if path == "/":
            self.redirect("/pages/https-demo.html", send_body)
        elif path == "/redirect-http":
            self.redirect("http://127.0.0.1:8765/plain", send_body)
        elif path == "/redirect":
            self.redirect("/pages/https-demo.html?from=redirect#checks", send_body)
        elif path == "/secure":
            protocol = self.connection.version()
            self.reply(page("HTTPS works", f"This page arrived through our authenticated {protocol} connection.", "HTTPS_FIXTURE_OK"), send_body)
        else:
            super().route(send_body)


def context(certfile: Path, keyfile: Path, tls_version: str = "1.3") -> ssl.SSLContext:
    result = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    version = ssl.TLSVersion.TLSv1_2 if tls_version == "1.2" else ssl.TLSVersion.TLSv1_3
    result.minimum_version = result.maximum_version = version
    if tls_version == "1.2":
        result.set_ciphers("ECDHE+AESGCM")
        result.set_ecdh_curve("prime256v1")
    else:
        result.set_ecdh_curve("X25519")
    result.set_alpn_protocols(["http/1.1"])
    result.load_cert_chain(str(certfile), str(keyfile))
    return result


def demo_browser_command(browser: Path | None, port: int) -> list[str]:
    """Explicit test trust for this process only; never change normal trust data."""
    executable = (browser or PROJECT / "ChezCat.exe").resolve()
    trust_file = (FIXTURES / "demo-roots.pem").resolve()
    if not executable.is_file():
        raise OSError(f"Browser executable not found: {executable}. Build it first or use --server-only.")
    if not trust_file.is_file():
        raise OSError(f"Demo trust file not found: {trust_file}. Extract the complete ChezCat folder.")
    return [str(executable), "--trust-file", str(trust_file), f"https://localhost:{port}/"]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=8766)
    parser.add_argument("--identity", choices=("rsa", "p256", "p384"), default="p256")
    parser.add_argument("--tls-version", choices=("1.2", "1.3"), default="1.3")
    parser.add_argument("--cert", type=Path)
    parser.add_argument("--key", type=Path)
    parser.add_argument("--open-browser", dest="open_browser", action="store_true",
                        help="open a new browser process using only the isolated demo trust file")
    parser.add_argument("--server-only", dest="open_browser", action="store_false",
                        help="start only the server; overrides the Windows launcher's --open-browser")
    parser.add_argument("--browser", type=Path,
                        help="browser executable for --open-browser (default: ChezCat.exe)")
    parser.set_defaults(open_browser=False)
    args = parser.parse_args()
    if not 0 <= args.port <= 65535:
        parser.error("--port must be between 0 and 65535")
    if bool(args.cert) != bool(args.key):
        parser.error("--cert and --key must be provided together")
    if args.open_browser and args.cert:
        parser.error("--open-browser uses the bundled demo trust roots; use --server-only with custom certificates")
    prefix = "server" if args.identity == "rsa" else "server-" + args.identity
    cert = args.cert or FIXTURES / (prefix + "-chain.pem")
    key = args.key or FIXTURES / (prefix + "-key.pem")
    try:
        server = TlsFixtureServer(("127.0.0.1", args.port), TlsFixtureHandler, context(cert, key, args.tls_version))
    except (OSError, ValueError) as error:
        parser.exit(1, f"Cannot start the local HTTPS fixture: {error}\nNo browser was opened.\n")
    with server:
        port = server.server_address[1]
        print(f"HTTPS fixture server: https://localhost:{port}/", flush=True)
        print(f"Certificate: {cert.name}", flush=True)
        print(f"Protocol: TLS {args.tls_version}", flush=True)
        if args.open_browser:
            try:
                command = demo_browser_command(args.browser, port)
                subprocess.Popen(command, cwd=PROJECT)
            except OSError as error:
                parser.exit(1, f"Cannot open the demo browser: {error}\n")
            print("Opened a NEW browser window with the isolated demo trust file.", flush=True)
        else:
            print(f"Open the matching demo browser: .\\tools\\demos\\Run-Https-Browser.cmd {port}", flush=True)
        print("Demo trust: tools/https-demo/demo-roots.pem (local fixtures only).", flush=True)
        print("An already-open normal browser does not trust these test certificates.", flush=True)
        print("For Google, EFF and GRC, start ChezCat.exe normally in a separate window.", flush=True)
        print("Loopback only. Public TEST key. Press Ctrl+C to stop.", flush=True)
        try:
            server.serve_forever(poll_interval=0.2)
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
