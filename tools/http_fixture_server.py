#!/usr/bin/env python3
"""Development-only HTTP fixtures. The browser does not use Python.

Run from any folder: python tools/http_fixture_server.py
Open http://127.0.0.1:8765/ in ChezCat. Press Ctrl+C to stop.
The server binds only to IPv4 loopback and serves the project's pages folder.
"""

from __future__ import annotations

import argparse
import gzip
import html
import math
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import sys
import time
from urllib.parse import parse_qs, parse_qsl, unquote, urlsplit
import zlib


PROJECT_ROOT = Path(__file__).resolve().parents[1]
MAX_ASSET_BYTES = 32 * 1024 * 1024
MIME_TYPES = {
    ".html": "text/html; charset=utf-8",
    ".htm": "text/html; charset=utf-8",
    ".css": "text/css; charset=utf-8",
    ".js": "text/javascript; charset=utf-8",
    ".png": "image/png",
    ".apng": "image/apng",
    ".bmp": "image/bmp",
    ".ppm": "image/x-portable-pixmap",
    ".pbm": "image/x-portable-bitmap",
    ".pgm": "image/x-portable-graymap",
    ".pnm": "image/x-portable-anymap",
    ".pam": "image/x-portable-arbitrarymap",
    ".qoi": "image/qoi",
    ".tif": "image/tiff",
    ".tiff": "image/tiff",
    ".tga": "image/x-tga",
    ".pcx": "image/x-pcx",
    ".jpg": "image/jpeg",
    ".jpeg": "image/jpeg",
    ".gif": "image/gif",
    ".svg": "image/svg+xml",
    ".ico": "image/x-icon",
    ".cur": "image/x-icon",
    ".webp": "image/webp",
    ".ttf": "font/ttf",
    ".woff": "font/woff",
}


def page(title: str, message: str, marker: str = "HTTP_FIXTURE_OK") -> bytes:
    return (
        "<!doctype html><html><head><meta charset='utf-8'>"
        f"<title>{html.escape(title)}</title>"
        "<link rel='stylesheet' href='/pages/styles.css'></head>"
        "<body><main><div class='card'>"
        f"<h1>{html.escape(title)}</h1><p>{html.escape(message)}</p>"
        f"<pre>{html.escape(marker)}</pre>"
        "<p><a href='/pages/http-demo.html'>Back to HTTP demo</a></p>"
        "</div></main></body></html>"
    ).encode("utf-8")


class FixtureServer(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True


class FixtureHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    server_version = "ChezCatFixture/0.30"
    sys_version = ""

    def setup(self) -> None:
        super().setup()
        self.connection.settimeout(10)

    def do_GET(self) -> None:
        self.handle_fixture(send_body=True)

    def do_HEAD(self) -> None:
        self.handle_fixture(send_body=False)

    def do_POST(self) -> None:
        # Original bounded loopback fixture, never a proxy or an Internet scan.
        try:
            if self.headers.get('Transfer-Encoding') is not None:
                raise ValueError('Chunked request bodies are not supported by this fixture.')
            lengths = self.headers.get_all('Content-Length', [])
            if len(lengths) != 1 or not lengths[0].isascii() or not lengths[0].isdigit():
                raise ValueError('One Content-Length is required.')
            length = int(lengths[0])
            if not 0 <= length <= 128 * 1024:
                raise ValueError('Request body exceeds 128 KiB.')
            if self.headers.get_content_type() != 'application/x-www-form-urlencoded':
                self.reply(page('Unsupported form encoding', 'Use application/x-www-form-urlencoded.'), True, 415)
                return
            body = self.rfile.read(length)
            if len(body) != length:
                raise ValueError('Truncated form body.')
            parsed = urlsplit(self.path)
            if parsed.path != '/form-result':
                self.reply(page('Fixture not found', 'POST is supported at /form-result.'), True, 404)
                return
            encoded = body.decode('ascii')
            pairs = parse_qsl(encoded, keep_blank_values=True, max_num_fields=512, encoding='utf-8', errors='strict')
            values = '\n'.join(f'{name} = {value}' for name, value in pairs) or '(no named values)'
            headers = '\n'.join(f'{name}: {value}' for name, value in self.headers.items())
            result = (
                '<!doctype html><meta charset="utf-8"><title>POST form received</title>'
                '<link rel="stylesheet" href="/pages/styles.css"><main><h1>POST form received</h1>'
                '<p>The loopback server received the following request. No external scanner was contacted.</p>'
                f'<h2>Request</h2><pre>POST {html.escape(self.path)}</pre>'
                f'<h2>Form values</h2><pre>{html.escape(values)}</pre>'
                f'<h2>Encoded body</h2><pre>{html.escape(encoded)}</pre>'
                f'<h2>Request headers</h2><pre>{html.escape(headers)}</pre>'
                '<p>F5 or Ctrl+F5 asks before sending these form values again.</p>'
                '<p><a href="/pages/browser-post-demo.html">Return to the form</a> &middot; '
                '<a href="/browser-report">Inspect a GET request</a></p></main>'
            ).encode('utf-8')
            self.reply(result, True)
        except (UnicodeError, ValueError) as error:
            self.reply(page('Bad form request', str(error)), True, 400)
        except (BrokenPipeError, ConnectionResetError, TimeoutError):
            self.close_connection = True

    def log_message(self, fmt: str, *args: object) -> None:
        # Keep terminal control characters out of developer logs.
        message = (fmt % args).encode("unicode_escape").decode("ascii")
        print(f"{self.log_date_time_string()} {message}", file=sys.stderr)

    def send_fixture_headers(
        self,
        status: int,
        content_type: str = "text/html; charset=utf-8",
        length: int | None = None,
        extra: tuple[tuple[str, str], ...] = (),
    ) -> None:
        self.send_response(status)
        if status not in (204, 304):
            self.send_header("Content-Type", content_type)
        if length is not None and status not in (204, 304):
            self.send_header("Content-Length", str(length))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Connection", "close")
        for name, value in extra:
            self.send_header(name, value)
        self.end_headers()
        self.close_connection = True

    def reply(
        self,
        body: bytes,
        send_body: bool,
        status: int = 200,
        content_type: str = "text/html; charset=utf-8",
        extra: tuple[tuple[str, str], ...] = (),
    ) -> None:
        self.send_fixture_headers(status, content_type, len(body), extra)
        if send_body and status not in (204, 304):
            self.wfile.write(body)

    def redirect(self, location: str, send_body: bool, status: int = 302) -> None:
        body = page("Redirect response", f"Destination: {location}", "HTTP_REDIRECT")
        self.reply(body, send_body, status, extra=(("Location", location),))

    def chunked(self, body: bytes, send_body: bool, compressed: bool = False) -> None:
        headers = [("Transfer-Encoding", "chunked"), ("Trailer", "X-Fixture-Complete")]
        if compressed:
            body = gzip.compress(body, mtime=0)
            headers.append(("Content-Encoding", "gzip"))
        self.send_fixture_headers(200, extra=tuple(headers))
        if not send_body:
            return
        position = 0
        sizes = (1, 7, 31, 113)
        chunk_number = 0
        while position < len(body):
            chunk = body[position : position + sizes[chunk_number % len(sizes)]]
            extension = ";fixture=yes" if chunk_number == 0 else ""
            self.wfile.write(f"{len(chunk):X}{extension}\r\n".encode("ascii"))
            self.wfile.write(chunk + b"\r\n")
            position += len(chunk)
            chunk_number += 1
        self.wfile.write(b"0\r\nX-Fixture-Complete: yes\r\n\r\n")

    def handle_fixture(self, send_body: bool) -> None:
        try:
            self.route(send_body)
        except (BrokenPipeError, ConnectionResetError, TimeoutError):
            # Stopping navigation intentionally closes a request early.
            self.close_connection = True

    def route(self, send_body: bool) -> None:
        if len(self.path) > 8_192:
            self.reply(page("Request too long", "The fixture URL is too long."), send_body, 414)
            return
        try:
            parsed = urlsplit(self.path)
            path = unquote(parsed.path, errors="strict")
            query = parse_qs(parsed.query, max_num_fields=32)
        except (UnicodeError, ValueError):
            self.reply(page("Bad request", "Invalid URL."), send_body, 400)
            return

        if path == "/":
            self.redirect("/pages/http-demo.html", send_body)
        elif path.startswith("/compressed-pages/"):
            self.asset("/pages/" + path[len("/compressed-pages/"):], send_body, compressed=True)
        elif path.startswith("/pages/"):
            self.asset(path, send_body)
        elif path == "/blocking-redirect.js":
            self.redirect("/pages/blocking/advert.js", send_body)
        elif path in ("/plain", "/identity"):
            self.reply(page("Plain HTTP works", "This response uses Content-Length framing."), send_body)
        elif path in ("/chunked", "/chunked-gzip"):
            compressed = path == "/chunked-gzip"
            marker = "HTTP_CHUNKED_GZIP_OK" if compressed else "HTTP_CHUNKED_OK"
            self.chunked(page("Chunked response", "Chunk extensions and a trailer are present.", marker), send_body, compressed)
        elif path in ("/gzip", "/deflate"):
            coding = path[1:]
            body = page(f"{coding} response", "The body was compressed before transmission.", f"HTTP_{coding.upper()}_OK")
            body = gzip.compress(body, mtime=0) if coding == "gzip" else zlib.compress(body)
            self.reply(body, send_body, extra=(("Content-Encoding", coding),))
        elif path in ("/gzip-members", "/gzip-stacked"):
            body = page("Compressed content works", "The browser decodes the content before parsing this HTML.", "HTTP_COMPRESSION_OK")
            if path == "/gzip-members":
                middle = len(body) // 2
                body = gzip.compress(body[:middle], mtime=0) + gzip.compress(body[middle:], mtime=0)
                coding = "gzip"
            else:
                body = zlib.compress(gzip.compress(body, mtime=0))
                coding = "gzip, deflate"
            self.reply(body, send_body, extra=(("Content-Encoding", coding),))
        elif path == "/encoding":
            self.reply(page("Requested encodings", self.headers.get("Accept-Encoding", "(none)"), "HTTP_ENCODING_HEADER"), send_body)
        elif path in ("/gzip-bad-crc", "/gzip-oversized", "/raw-deflate", "/brotli"):
            body = page("Compression rejection", "This deliberately invalid or unsupported response should not render.")
            coding = "gzip"
            if path == "/gzip-bad-crc":
                body = bytearray(gzip.compress(body, mtime=0))
                body[-8] ^= 1
                body = bytes(body)
            elif path == "/gzip-oversized":
                body = gzip.compress(b"x" * (2 * 1024 * 1024 + 1), mtime=0)
            elif path == "/raw-deflate":
                compressor = zlib.compressobj(wbits=-15)
                body = compressor.compress(body) + compressor.flush()
                coding = "deflate"
            else:
                coding = "br"
            self.reply(body, send_body, extra=(("Content-Encoding", coding),))
        elif path == "/close-delimited":
            self.send_fixture_headers(200)
            if send_body:
                self.wfile.write(page("Connection-close framing", "No Content-Length or Transfer-Encoding header was sent.", "HTTP_CLOSE_OK"))
        elif path == "/redirect":
            self.redirect("/pages/http-demo.html?from=redirect#checks", send_body)
        elif path in ("/redirect/301", "/redirect/302", "/redirect/303", "/redirect/307", "/redirect/308"):
            self.redirect("/plain", send_body, int(path.rsplit("/", 1)[1]))
        elif path == "/redirect-relative/start":
            self.redirect("../pages/second.html", send_body)
        elif path.startswith("/redirect-chain/"):
            try:
                count = int(path.rsplit("/", 1)[1])
            except ValueError:
                count = -1
            if not 0 <= count <= 20:
                self.reply(page("Bad chain length", "Use a number from 0 to 20."), send_body, 400)
            else:
                self.redirect(f"/redirect-chain/{count - 1}" if count else "/plain", send_body)
        elif path == "/redirect-loop":
            self.redirect("/redirect-loop", send_body)
        elif path == "/redirect-https":
            self.redirect("https://example.org/", send_body)
        elif path == "/redirect-file":
            self.redirect("file:///not-allowed.html", send_body)
        elif path in ("/status/204", "/status/304"):
            self.send_fixture_headers(int(path.rsplit("/", 1)[1]))
        elif path in ("/status/400", "/status/403", "/status/404", "/status/500", "/status/503"):
            status = int(path.rsplit("/", 1)[1])
            self.reply(page(f"HTTP status {status}", "This response deliberately has an error status and a readable HTML body.", f"HTTP_STATUS_{status}"), send_body, status)
        elif path.startswith("/charset/"):
            self.charset(path, send_body)
        elif path == "/query":
            self.reply(page("Query preserved", parsed.query or "No query was supplied.", "HTTP_QUERY_OK"), send_body)
        elif path == "/browser-report":
            headers = '\n'.join(f'{name}: {value}' for name, value in self.headers.items())
            body = (
                '<!doctype html><meta charset="utf-8"><title>Observed request headers</title>'
                '<link rel="stylesheet" href="/pages/styles.css"><main><h1>Observed request headers</h1>'
                '<p>These are the actual headers received by the local test server. '
                'This is a request inspection, not a fingerprint uniqueness score or a network scan.</p>'
                f'<pre>{html.escape(headers)}</pre>'
                '<p><a href="/pages/browser-tests.html">Browser checks</a> &middot; '
                '<a href="/pages/browser-post-demo.html">Try a POST form</a></p></main>'
            ).encode('utf-8')
            self.reply(body, send_body)
        elif path == "/form-result":
            pairs = parse_qsl(parsed.query, keep_blank_values=True, max_num_fields=64)
            values = "\n".join(f"{name} = {value}" for name, value in pairs) or "No named values were sent."
            body = (
                "<!doctype html><meta charset='utf-8'><title>Form received</title>"
                "<link rel='stylesheet' href='/pages/styles.css'><main>"
                "<h1 id='received'>Form received</h1><p>The loopback server received these GET values, in order:</p>"
                f"<pre>{html.escape(values)}</pre><h2>Request query</h2><pre>{html.escape(parsed.query)}</pre>"
                "<p><a href='/pages/forms.html'>Try the form again</a></p>"
                "<p><a href='/pages/http-demo.html'>HTTP demo</a></p></main>"
            ).encode("utf-8")
            self.reply(body, send_body)
        elif path in ("/slow", "/slow-stream"):
            try:
                seconds = float(query.get("seconds", ["2"])[0])
                seconds = min(max(seconds, 0.0), 8.0) if math.isfinite(seconds) else 2.0
            except ValueError:
                seconds = 2.0
            body = page("Slow response completed", "Navigate away while this loads to check stale-response handling.", "HTTP_SLOW_OK")
            if path == "/slow":
                time.sleep(seconds)
                self.reply(body, send_body)
            else:
                self.send_fixture_headers(200, length=len(body))
                if send_body:
                    for start in range(0, len(body), 64):
                        self.wfile.write(body[start : start + 64])
                        self.wfile.flush()
                        time.sleep(seconds / max(1, (len(body) + 63) // 64))
        elif path == "/truncated":
            self.send_fixture_headers(200, length=100)
            if send_body:
                self.wfile.write(b"short")
        elif path == "/bad-chunk":
            self.send_fixture_headers(200, extra=(("Transfer-Encoding", "chunked"),))
            if send_body:
                self.wfile.write(b"not-hex\r\ninvalid\r\n0\r\n\r\n")
        elif path == "/conflicting-length":
            self.send_fixture_headers(200, extra=(("Content-Length", "8"), ("Content-Length", "9")))
            if send_body:
                self.wfile.write(b"12345678")
        else:
            self.reply(page("Fixture not found", f"No fixture at {path}"), send_body, 404)

    def charset(self, path: str, send_body: bool) -> None:
        if path == "/charset/utf-8":
            self.reply(page("UTF-8 response", "Decoded text: caf\u00e9, na\u00efve, \u00a35, \u20ac.", "HTTP_UTF8_OK"), send_body)
        elif path in ("/charset/windows-1252", "/charset/iso-8859-1"):
            label = path.rsplit("/", 1)[1]
            message = "Decoded text: caf\u00e9, na\u00efve, \u00a35."
            if label == "windows-1252":
                message += " Smart quotes: \u201cHello\u201d; euro: \u20ac."
            body = page(label, message, "HTTP_LEGACY_CHARSET_OK").decode("utf-8")
            # Header and meta agree; avoid letting an embedded UTF-8 declaration
            # hide a broken transport-charset implementation.
            body = body.replace("charset='utf-8'", f"charset='{label}'").encode(label)
            self.reply(body, send_body, content_type=f"text/html; charset={label}")
        elif path == "/charset/unsupported":
            self.reply(page("Unsupported charset fixture", "A clear unsupported-charset result is expected."), send_body, content_type="text/html; charset=x-fixture-unknown")
        else:
            self.reply(page("Charset fixture not found", path), send_body, 404)

    def asset(self, path: str, send_body: bool, compressed: bool = False) -> None:
        relative = path[len("/pages/") :]
        parts = relative.split("/")
        if not relative or "\\" in relative or any(part.startswith(".") for part in parts):
            self.reply(page("Asset path rejected", "Choose a file inside pages."), send_body, 403)
            return
        root = PROJECT_ROOT / "pages"
        try:
            file = (root / relative).resolve()
            file.relative_to(root.resolve())
            content_type = MIME_TYPES.get(file.suffix.lower())
            if content_type is None or not file.is_file():
                raise FileNotFoundError
            with file.open("rb") as stream:
                body = stream.read(MAX_ASSET_BYTES + 1)
            if len(body) > MAX_ASSET_BYTES:
                self.reply(page("Asset too large", "The fixture asset exceeds 32 MiB."), send_body, 413)
                return
            extra = ()
            if compressed:
                coding = "deflate" if file.suffix.lower() in (".png", ".bmp", ".ppm", ".jpg", ".jpeg") else "gzip"
                body = zlib.compress(body) if coding == "deflate" else gzip.compress(body, mtime=0)
                extra = (("Content-Encoding", coding),)
            self.reply(body, send_body, content_type=content_type, extra=extra)
        except ValueError:
            self.reply(page("Asset path rejected", "The path leaves the pages folder."), send_body, 403)
        except OSError:
            self.reply(page("Asset not found", path), send_body, 404)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=8765, help="loopback port (default: 8765; 0 chooses a free port)")
    args = parser.parse_args()
    if not 0 <= args.port <= 65535:
        parser.error("--port must be between 0 and 65535")
    try:
        server = FixtureServer(("127.0.0.1", args.port), FixtureHandler)
    except OSError as error:
        print(f"Cannot start fixture server: {error}", file=sys.stderr)
        return 1
    with server:
        port = server.server_address[1]
        print(f"ChezCat HTTP demo: http://127.0.0.1:{port}/", flush=True)
        print("Open that address in ChezCat. Keep this terminal open; Ctrl+C stops the server.", flush=True)
        print("Development fixture only. Python is not part of the browser runtime.", flush=True)
        try:
            server.serve_forever(poll_interval=0.25)
        except KeyboardInterrupt:
            print("\nStopped.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
