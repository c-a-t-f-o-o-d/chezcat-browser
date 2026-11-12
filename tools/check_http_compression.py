#!/usr/bin/env python3
"""Independent full-browser HTTP content-decoding checks.

Python's standard-library gzip/zlib encoders and ssl server are development-only
oracles. The browser never invokes them. Every request stays on loopback, and
the HTTPS server uses the separate public P-256 demo identity/trust file.

The server keeps the same URL while changing its response encoding. Successful
document, stylesheet, image and source-view renders must be byte-identical to
their uncompressed controls. Invalid top-level responses must produce no PPM.
"""

from __future__ import annotations

import argparse
from collections import Counter
from contextlib import contextmanager
import gzip
import hashlib
from http.server import BaseHTTPRequestHandler
import json
from pathlib import Path
import ssl
import struct
import subprocess
import tempfile
import threading
import zlib

from http_fixture_server import FixtureServer
from https_fixture_server import FIXTURES, TlsFixtureServer, context


DOCUMENT_LIMIT = 2 * 1024 * 1024
DOCUMENT = """<!doctype html><html><head><meta charset="utf-8">
<title>Content decoding check</title><link rel="stylesheet" href="/style.css">
</head><body><h1>Compressed pages work.</h1>
<p>Same HTML, CSS and PNG pixels. Accents: café, niño, résumé.</p>
<img src="/picture.png" width="160" height="96" alt="Compression image">
<p>HTTP framing describes encoded bytes; decoding happens before rendering.</p>
</body></html>""".encode("utf-8")
STYLE = b"body{background:#e9eef4;color:#233747;padding:24px;font-size:18px}h1{color:#147846}img{margin:12px}"


def png_chunk(kind, payload):
    return (struct.pack(">I", len(payload)) + kind + payload
            + struct.pack(">I", zlib.crc32(kind + payload)))


def picture():
    # A tiny original RGB fixture; its purple pixels differ from every CSS color.
    row = b"\x93\x47\xe7\x31\xca\xce\xf4\xc4\x4e"
    return (b"\x89PNG\r\n\x1a\n"
            + png_chunk(b"IHDR", struct.pack(">IIBBBBB", 3, 2, 8, 2, 0, 0, 0))
            + png_chunk(b"IDAT", zlib.compress(b"\0" + row + b"\0" + row[::-1]))
            + png_chunk(b"IEND", b""))


PICTURE = picture()
CONTENT = {"/document": (DOCUMENT, "text/html; charset=utf-8"),
           "/style.css": (STYLE, "text/css; charset=utf-8"),
           "/picture.png": (PICTURE, "image/png")}
EXPECTED_REQUESTS = Counter(CONTENT.keys())


def gz(data):
    return gzip.compress(data, mtime=0)


def oversized_encoded_document():
    # Valid gzip members with optional comments make the encoded representation
    # exceed 2 MiB while its final decoded document remains small. Wrapping it
    # again tests the intermediate-layer bound independently of final size.
    parts = []
    for index in range(64):
        data = DOCUMENT if index == 63 else b""
        raw = zlib.compressobj(wbits=-15)
        compressed = raw.compress(data) + raw.flush()
        parts.append(b"\x1f\x8b\x08\x10\0\0\0\0\0\xff" + b"x" * 33_000 + b"\0"
                     + compressed + struct.pack("<II", zlib.crc32(data), len(data)))
    result = b"".join(parts)
    assert len(result) > DOCUMENT_LIMIT and gzip.decompress(result) == DOCUMENT
    return result


def encoded_document(mode, oversized):
    body, headers, framing = DOCUMENT, [], "length"
    if mode in ("identity", "assets-only"):
        pass
    elif mode == "identity-header":
        headers = [("Content-Encoding", "identity")]
    elif mode == "empty-only":
        headers = [("Content-Encoding", "," * 31)]  # Exactly 32 empty elements.
    elif mode in ("gzip", "all-compressed", "chunked-gzip", "close-gzip"):
        body, headers = gz(body), [("Content-Encoding", "gzip")]
        if mode == "chunked-gzip":
            framing = "chunked"
        elif mode == "close-gzip":
            framing = "close"
    elif mode == "x-gzip":
        body, headers = gz(body), [("Content-Encoding", "x-gzip")]
    elif mode == "deflate":
        body, headers = zlib.compress(body), [("Content-Encoding", "deflate")]
    elif mode == "concatenated-gzip":
        split = len(body) // 2
        body, headers = gz(body[:split]) + gz(body[split:]), [("Content-Encoding", "gzip")]
    elif mode in ("stacked", "multiple-headers"):
        body = zlib.compress(gz(body))
        headers = ([("Content-Encoding", "gzip, deflate")] if mode == "stacked" else
                   [("Content-Encoding", "GZiP"), ("content-encoding", "DeFlAtE")])
    elif mode == "empty-elements":
        body, headers = gz(body), [("Content-Encoding", "," * 32 + "gzip")]
    elif mode == "four-codings":
        body, headers = gz(zlib.compress(gz(body))), [("Content-Encoding", "gzip, deflate, identity, gzip")]
    elif mode in ("bad-gzip-crc", "bad-gzip-size", "truncated-gzip", "gzip-trailing", "wrong-content-length"):
        body, headers = gz(body), [("Content-Encoding", "gzip")]
        if mode == "bad-gzip-crc":
            body = body[:-8] + bytes([body[-8] ^ 1]) + body[-7:]
        elif mode == "bad-gzip-size":
            body = body[:-4] + bytes([body[-4] ^ 1]) + body[-3:]
        elif mode == "truncated-gzip":
            body = body[:-3]
        elif mode == "gzip-trailing":
            body += b"trailing bytes"
        else:
            framing = "wrong-length"
    elif mode in ("bad-zlib-adler", "truncated-zlib", "zlib-trailing", "raw-deflate"):
        body, headers = zlib.compress(body), [("Content-Encoding", "deflate")]
        if mode == "bad-zlib-adler":
            body = body[:-1] + bytes([body[-1] ^ 1])
        elif mode == "truncated-zlib":
            body = body[:-2]
        elif mode == "zlib-trailing":
            body += b"trailing bytes"
        else:
            raw = zlib.compressobj(wbits=-15)
            body = raw.compress(DOCUMENT) + raw.flush()
    elif mode == "unsupported-br":
        headers = [("Content-Encoding", "br")]
    elif mode == "unknown-layer":
        body, headers = gz(body), [("Content-Encoding", "gzip, br")]
    elif mode == "too-many-layers":
        for _ in range(5):
            body = gz(body)
        headers = [("Content-Encoding", ", ".join(["gzip"] * 5))]
    elif mode == "too-many-empty":
        body, headers = gz(body), [("Content-Encoding", "," * 33 + "gzip")]
    elif mode == "decoded-limit":
        body, headers = gz(b"x" * (DOCUMENT_LIMIT + 1)), [("Content-Encoding", "gzip")]
    elif mode == "encoded-limit":
        body, headers = oversized, [("Content-Encoding", "gzip")]
    elif mode == "intermediate-limit":
        body, headers = gz(oversized), [("Content-Encoding", "gzip, gzip")]
    else:
        raise AssertionError("Unknown fixture mode: " + mode)
    return body, headers, framing


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt, *args):
        pass

    def do_GET(self):
        self.connection.settimeout(10)
        self.close_connection = True
        mode = self.server.mode
        self.server.requests.append(self.path)
        self.server.accepted.append(self.headers.get("Accept-Encoding", ""))
        if isinstance(self.connection, ssl.SSLSocket):
            self.server.negotiated.append((self.connection.version(), self.connection.cipher()[0]))
        if self.path not in CONTENT:
            self.send_error(404)
            return
        body, mime = CONTENT[self.path]
        headers, framing = [], "length"
        if self.path == "/document":
            body, headers, framing = encoded_document(mode, self.server.oversized)
        elif mode in ("assets-only", "all-compressed"):
            coding = "gzip" if self.path == "/style.css" else "deflate"
            body = gz(body) if coding == "gzip" else zlib.compress(body)
            headers = [("Content-Encoding", coding)]
        try:
            self.send_response(200)
            self.send_header("Content-Type", mime)
            self.send_header("Connection", "close")
            if framing in ("length", "wrong-length"):
                self.send_header("Content-Length", str(len(body) + (framing == "wrong-length")))
            elif framing == "chunked":
                self.send_header("Transfer-Encoding", "chunked")
                self.send_header("Trailer", "X-Decoding-Check")
            for name, value in headers:
                self.send_header(name, value)
            self.end_headers()
            if framing == "chunked":
                # Deliberately split gzip header, DEFLATE blocks and trailer.
                sizes, offset, count = (1, 2, 7, 19, 53), 0, 0
                while offset < len(body):
                    chunk = body[offset:offset + sizes[count % len(sizes)]]
                    self.wfile.write(f"{len(chunk):x};n={count}\r\n".encode() + chunk + b"\r\n")
                    offset += len(chunk)
                    count += 1
                self.wfile.write(b"0\r\nX-Decoding-Check: complete\r\n\r\n")
            else:
                self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError, TimeoutError, ssl.SSLError):
            # Invalid/oversized responses intentionally make the client stop.
            pass


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


POSITIVE = ("identity", "identity-header", "gzip", "x-gzip", "deflate", "chunked-gzip",
            "close-gzip", "concatenated-gzip", "stacked", "multiple-headers", "empty-elements",
            "empty-only", "four-codings", "assets-only", "all-compressed")
NEGATIVE = ("bad-gzip-crc", "bad-gzip-size", "truncated-gzip", "bad-zlib-adler", "truncated-zlib",
            "gzip-trailing", "zlib-trailing", "raw-deflate", "unsupported-br", "unknown-layer",
            "too-many-layers", "too-many-empty", "decoded-limit", "encoded-limit",
            "intermediate-limit", "wrong-content-length")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--browser", required=True, type=Path)
    parser.add_argument("--output", type=Path, help="Optional JSON report path")
    args = parser.parse_args()
    browser = args.browser.resolve()
    if not browser.is_file():
        parser.error("--browser must name the built browser executable")
    results, controls = [], {}
    oversized = oversized_encoded_document()
    with tempfile.TemporaryDirectory(prefix="chezcat-content-decoding-") as temporary:
        directory = Path(temporary)
        for protocol in ("http", "https"):
            if protocol == "https":
                server = TlsFixtureServer(("127.0.0.1", 0), Handler,
                    context(FIXTURES / "server-p256-chain.pem", FIXTURES / "server-p256-key.pem"))
            else:
                server = FixtureServer(("127.0.0.1", 0), Handler)
            server.oversized = oversized
            server.negotiated = []
            url = f"{protocol}://localhost:{server.server_address[1]}/document"

            def render(mode, source=False, success=True):
                label = protocol + "-" + mode + ("-source" if source else "")
                server.mode, server.requests, server.accepted = mode, [], []
                image = directory / (label + ".ppm")
                command = [str(browser), "--trust-file", str(FIXTURES / "demo-roots.pem"),
                           url, "--render", str(image), "--width", "1080", "--height", "760"]
                if source:
                    command.append("--source")
                process = subprocess.run(command, capture_output=True, text=True, timeout=35)
                encodings = [{value.strip().lower() for value in line.split(",")} for line in server.accepted]
                assert encodings and all({"gzip", "deflate"} <= value and "br" not in value
                                         for value in encodings), (label, server.accepted)
                if success:
                    assert process.returncode == 0, (label, process.stderr)
                    pixels = image.read_bytes()
                    assert pixels.startswith(b"P6\n1080 760\n255\n"), label
                    assert Counter(server.requests) == EXPECTED_REQUESTS, (label, server.requests)
                    if mode == "identity":
                        controls[(protocol, source)] = pixels
                        if not source:
                            assert pixels.count(b"\x14\x78\x46") > 100, "CSS control failed to paint"
                            assert pixels.count(b"\x93\x47\xe7") > 100, "PNG control failed to paint"
                    else:
                        assert pixels == controls[(protocol, source)], (label, "Pixels differ from identity control")
                    detail = {"pixel_sha256": hashlib.sha256(pixels).hexdigest(), "requests": len(server.requests)}
                else:
                    assert process.returncode != 0 and not image.exists(), (label, "Unexpected successful render")
                    assert server.requests == ["/document"], (label, "Assets requested after invalid document", server.requests)
                    assert process.stderr.strip(), (label, "Missing rejection explanation")
                    if mode.endswith("limit"):
                        assert "limit" in process.stderr.lower(), (label, process.stderr)
                    detail = {"error": process.stderr.strip()}
                results.append({"case": label, "passed": True, **detail})
                print("  PASS " + label, flush=True)

            with serving(server):
                for mode in POSITIVE:
                    render(mode)
                render("identity", source=True)
                render("gzip", source=True)
                for mode in NEGATIVE:
                    render(mode, success=False)
            if protocol == "https":
                assert server.negotiated and all(v == "TLSv1.3" for v, _ in server.negotiated)
    report = {"passed": len(results), "pixel_comparisons": 2 * (len(POSITIVE) - 1 + 1),
              "python_ssl": ssl.OPENSSL_VERSION, "cases": results}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"PASS: {len(results)} HTTP/HTTPS content-decoding scenarios; {report['pixel_comparisons']} exact pixel comparisons")


if __name__ == "__main__":
    main()
