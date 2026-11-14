#!/usr/bin/env python3
"""Exercise classic-script inclusion through the actual ChezCat App over TLS.

All HTML/JavaScript is original and both HTTPS origins are loopback peers. The
bundled test CA is supplied only through --trust-file; no trust store is changed.
Python's independent TLS server checks real request cookies and forbidden hits.
These checks do not establish compatibility with a live public search service.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import ssl
import subprocess
import tempfile
import threading


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tools" / "https-demo"
SOURCE_LIMIT = 512 * 1024


class Server(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, secure=True):
        self.records = []
        self.peer = None
        self.secure = secure
        self.lock = threading.Lock()
        super().__init__(("127.0.0.1", 0), Handler)
        if secure:
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.minimum_version = context.maximum_version = ssl.TLSVersion.TLSv1_2
            context.set_ciphers("ECDHE-RSA-AES128-GCM-SHA256")
            context.set_ecdh_curve("prime256v1")
            context.load_cert_chain(FIXTURES / "server-chain.pem", FIXTURES / "server-key.pem")
            context.set_alpn_protocols(["http/1.1"])
            self.socket = context.wrap_socket(self.socket, server_side=True)

    @property
    def origin(self):
        return f"{'https' if self.secure else 'http'}://localhost:{self.server_port}"


def dynamic(target, title, attribute="", fail=False):
    result = "error" if fail else "load"
    return f"""<!doctype html><html><head><title>Pending script</title></head><body>
<script nonce=Owner>
var delivered=0;var s=document.createElement('script');s.src={json.dumps(target)};
{attribute}
s.on{result}=function(){{
  if(document.cookie!=='owner=one')throw Error('cross-origin cookie mutation');
  if(s.textContent!=='')throw Error('network source inserted into DOM');
  document.title={json.dumps(title)};
}};
s.on{'load' if fail else 'error'}=function(){{throw Error('unexpected script event');}};
document.head.appendChild(s);
</script></body></html>"""


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def do_GET(self):
        with self.server.lock:
            self.server.records.append({"path": self.path, "cookie": self.headers.get("Cookie", "")})
        status, mime, extra = 200, "text/html", []
        peer = self.server.peer.origin
        if self.path == "/parser":
            extra.append(("Set-Cookie", "owner=one; Secure; Path=/"))
            body = f"""<!doctype html><title>Parser pending</title><body>
<script src='{peer}/asset.js'></script><script>
if(remote!==7 || document.cookie!=='owner=one')throw Error('parser script or cookie policy');
document.title='Parser cross-origin OK';</script></body></html>"""
        elif self.path in ("/dynamic", "/redirect", "/csp-direct", "/csp-redirect",
                           "/cors", "/integrity", "/mime", "/large", "/oversize",
                           "/corp", "/corp-site-allow", "/corp-site-deny", "/corp-cross",
                           "/corp-redirect", "/coep", "/coep-credentialless", "/coep-same",
                           "/coep-report", "/redirect-downgrade"):
            extra.append(("Set-Cookie", "owner=one; Secure; Path=/"))
            target, title, attribute, fail = peer + "/asset.js", "Dynamic cross-origin OK", "", False
            if self.path == "/redirect":
                target, title = "/leave.js", "Redirect credentials OK"
            elif self.path.startswith("/csp-"):
                extra.append(("Content-Security-Policy", "script-src 'self' 'nonce-Owner'"))
                target = "/leave.js" if self.path.endswith("redirect") else peer + "/forbidden.js"
                title, fail = "CSP refusal OK", True
            elif self.path == "/cors":
                attribute, title, fail = "s.crossOrigin='anonymous';", "CORS refusal OK", True
            elif self.path == "/integrity":
                attribute, title, fail = "s.integrity='sha256-unimplemented';", "Integrity refusal OK", True
            elif self.path == "/mime":
                target, title, fail = peer + "/not-js.js", "MIME refusal OK", True
            elif self.path == "/large":
                target, title = peer + "/large.js", "Large script OK"
            elif self.path == "/oversize":
                target, title, fail = peer + "/oversize.js", "Source limit OK", True
            elif self.path == "/corp":
                target, title, fail = peer + "/corp.js", "CORP refusal OK", True
            elif self.path in ("/corp-site-allow", "/corp-site-deny"):
                target, title = peer + "/corp-site.js", "CORP same-site OK"
                if self.path.endswith("deny"):
                    target, title, fail = target.replace("localhost", "127.0.0.1"), "CORP site refusal OK", True
            elif self.path == "/corp-cross":
                target, title = peer + "/corp-cross.js", "CORP cross-origin OK"
            elif self.path == "/corp-redirect":
                target, title, fail = peer + "/corp-bounce.js", "CORP redirect refusal OK", True
            elif self.path in ("/coep", "/coep-credentialless", "/coep-same", "/coep-report"):
                header = "Cross-Origin-Embedder-Policy-Report-Only" if self.path.endswith("report") else "Cross-Origin-Embedder-Policy"
                extra.append((header, "credentialless" if self.path.endswith("credentialless") else "require-corp"))
                if self.path == "/coep-same":
                    target, title = "/same.js", "COEP same-origin OK"
                elif self.path == "/coep-report":
                    title = "COEP report-only OK"
                else:
                    title, fail = "COEP refusal OK", True
            elif self.path == "/redirect-downgrade":
                target, title, fail = peer + "/downgrade.js", "Downgrade redirect refusal OK", True
            body = dynamic(target, title, attribute, fail)
        elif self.path == "/parser-csp-redirect":
            extra.append(("Content-Security-Policy", "script-src 'self' 'nonce-Owner'"))
            body = "<title>Parser CSP pending</title><script src=/leave.js></script><script nonce=Owner>document.title='Parser CSP redirect OK';</script>"
        elif self.path == "/parser-corp":
            body = f"<title>Parser CORP OK</title><script src='{peer}/corp.js'></script>"
        elif self.path == "/parser-coep":
            extra.append(("Cross-Origin-Embedder-Policy", "require-corp"))
            body = f"<title>Parser COEP OK</title><script src='{peer}/forbidden.js'></script>"
        elif self.path == "/parser-cors":
            body = f"<title>Parser CORS OK</title><script crossorigin=anonymous src='{peer}/forbidden.js'></script>"
        elif self.path == "/mixed":
            extra.append(("Set-Cookie", "owner=one; Secure; Path=/"))
            body = dynamic(peer.replace("https:", "http:") + "/forbidden.js", "Mixed content refusal OK", fail=True)
        elif self.path == "/leave.js":
            status = 302
            extra.append(("Location", peer + "/bounce.js"))
            body = ""
        elif self.path == "/bounce.js":
            status = 302
            extra.extend((("Location", peer + "/returned.js"), ("Set-Cookie", "thirdparty=bad; Secure; Path=/")))
            body = ""
        elif self.path in ("/corp.js", "/corp-site.js", "/corp-cross.js", "/corp-bounce.js"):
            mime = "text/javascript"
            extra.append(("Cross-Origin-Resource-Policy", "same-site" if self.path == "/corp-site.js" else "cross-origin" if self.path == "/corp-cross.js" else "same-origin"))
            body = "var remote=7;"
            if self.path == "/corp-bounce.js":
                status, body = 302, ""
                extra.append(("Location", peer + "/returned.js"))
        elif self.path == "/same.js":
            mime, body = "text/javascript", "var remote=7;"
        elif self.path in ("/asset.js", "/returned.js"):
            mime = "text/javascript"
            extra.append(("Set-Cookie", "thirdparty=bad; Secure; Path=/"))
            body = "var remote=7;"
        elif self.path == "/not-js.js":
            body = "throw Error('MIME restriction failed');"
        elif self.path == "/large.js":
            mime = "text/javascript"
            body = "/*" + "x" * 370000 + "*/var largeSourceAccepted=1;"
        elif self.path == "/oversize.js":
            mime = "text/javascript"
            body = "/*" + "x" * SOURCE_LIMIT + "*/throw Error('source limit failed');"
        elif self.path == "/downgrade.js":
            status = 302
            extra.append(("Location", peer.replace("https:", "http:") + "/forbidden.js"))
            body = ""
        else:
            status, body = 404, "Unexpected fixture request"
        data = body.encode()
        self.send_response(status)
        self.send_header("Content-Type", mime + "; charset=utf-8")
        for name, value in extra:
            self.send_header(name, value)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        self.end_headers()
        try:
            self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError, ssl.SSLError):
            pass
        self.close_connection = True


@contextmanager
def serving():
    first, other = Server(), Server()
    first.peer, other.peer = other, first
    workers = [threading.Thread(target=server.serve_forever, daemon=True) for server in (first, other)]
    for worker in workers:
        worker.start()
    try:
        yield first, other
    finally:
        for server in (first, other):
            server.shutdown()
            server.server_close()
        for worker in workers:
            worker.join(timeout=3)


def require(ok, message):
    if not ok:
        raise AssertionError(message)


def exercise(browser, folder, results):
    with serving() as (first, other):
        def run(path, title, first_paths, other_paths, diagnostic=None, trust=None, succeeds=True):
            first.records.clear()
            other.records.clear()
            report = folder / (path.removeprefix("/") + ".txt")
            process = subprocess.run([str(browser), first.origin + path, "--trust-file",
                str(trust or FIXTURES / "ca.pem"), "--no-profile", "--activity-report", str(report),
                "--wait-ms", "350"], capture_output=True, text=True, timeout=30)
            text = report.read_text(encoding="utf-8") if report.exists() else ""
            require((process.returncode == 0) == succeeds, f"{path}: {process.stderr}")
            if title:
                require("Page title: " + title in text, f"{path}: missing title: {text}")
            require([entry["path"] for entry in first.records] == first_paths,
                    f"{path}: first-origin requests: {first.records}")
            require([entry["path"] for entry in other.records] == other_paths,
                    f"{path}: third-party requests: {other.records}")
            require(all(not entry["cookie"] for entry in other.records), f"{path}: third-party cookie sent")
            if diagnostic:
                require(diagnostic in text, f"{path}: missing diagnostic {diagnostic!r}: {text[-4000:]}")
            results["checks"].append({"case": path, "passed": True, "requests": first_paths + other_paths,
                                      "report_sha256": hashlib.sha256(text.encode()).hexdigest()})
            return text

        run("/parser", "Parser cross-origin OK", ["/parser"], ["/asset.js"])
        run("/dynamic", "Dynamic cross-origin OK", ["/dynamic"], ["/asset.js"])
        run("/redirect", "Redirect credentials OK", ["/redirect", "/leave.js", "/returned.js"], ["/bounce.js"])
        require(first.records[1]["cookie"] == "owner=one", "Same-origin starting hop lost its cookie")
        require(first.records[2]["cookie"] == "", "Redirect back restored credentials")
        run("/csp-direct", "CSP refusal OK", ["/csp-direct"], [], "Content Security Policy")
        run("/csp-redirect", "CSP refusal OK", ["/csp-redirect", "/leave.js"], [], "Content Security Policy")
        run("/parser-csp-redirect", "Parser CSP redirect OK", ["/parser-csp-redirect", "/leave.js"], [], "Content Security Policy")
        run("/cors", "CORS refusal OK", ["/cors"], [], "CORS-mode")
        run("/parser-cors", "Parser CORS OK", ["/parser-cors"], [], "CORS-mode")
        run("/integrity", "Integrity refusal OK", ["/integrity"], [], "Subresource Integrity")
        run("/mixed", "Mixed content refusal OK", ["/mixed"], [], "mixed content")
        run("/mime", "MIME refusal OK", ["/mime"], ["/not-js.js"], "JavaScript Content-Type")
        run("/large", "Large script OK", ["/large"], ["/large.js"])
        run("/oversize", "Source limit OK", ["/oversize"], ["/oversize.js"], "limit")
        run("/corp", "CORP refusal OK", ["/corp"], ["/corp.js"], "Cross-Origin-Resource-Policy")
        run("/corp-site-allow", "CORP same-site OK", ["/corp-site-allow"], ["/corp-site.js"])
        run("/corp-site-deny", "CORP site refusal OK", ["/corp-site-deny"], ["/corp-site.js"], "same-site requires")
        run("/corp-cross", "CORP cross-origin OK", ["/corp-cross"], ["/corp-cross.js"])
        run("/corp-redirect", "CORP redirect refusal OK", ["/corp-redirect"], ["/corp-bounce.js"], "Cross-Origin-Resource-Policy")
        run("/parser-corp", "Parser CORP OK", ["/parser-corp"], ["/corp.js"], "Cross-Origin-Resource-Policy")
        run("/coep", "COEP refusal OK", ["/coep"], [], "Cross-Origin-Embedder-Policy")
        run("/coep-credentialless", "COEP refusal OK", ["/coep-credentialless"], [], "Cross-Origin-Embedder-Policy")
        run("/coep-same", "COEP same-origin OK", ["/coep-same", "/same.js"], [])
        run("/coep-report", "COEP report-only OK", ["/coep-report"], ["/asset.js"])
        run("/parser-coep", "Parser COEP OK", ["/parser-coep"], [], "Cross-Origin-Embedder-Policy")
        run("/redirect-downgrade", "Downgrade redirect refusal OK", ["/redirect-downgrade"], ["/downgrade.js"], "unencrypted HTTP")
        # The same servers remain unreachable with a different bundled test root.
        run("/untrusted", None, [], [], trust=FIXTURES / "ec-ca.pem", succeeds=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--browser", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    options = parser.parse_args()
    browser = options.browser.resolve()
    result = {"tool": Path(__file__).name, "browser_sha256": hashlib.sha256(browser.read_bytes()).hexdigest(),
              "checks": [], "passed": False, "scope": "Original loopback TLS fixtures; no live Google validation"}
    try:
        with tempfile.TemporaryDirectory(prefix="chezcat-script-policy-") as directory:
            exercise(browser, Path(directory), result)
        result["passed"] = True
    except (AssertionError, OSError, subprocess.SubprocessError) as error:
        result["error"] = str(error)
    if options.output:
        options.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result["passed"] else 1)


if __name__ == "__main__":
    main()
