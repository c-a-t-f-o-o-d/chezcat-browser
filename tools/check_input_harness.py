"""Exercise the actual executable's input driver on original local fixtures.

This is controlled harness validation, never evidence of live Google results.
"""
from datetime import datetime, timezone
import http.server
import json
from pathlib import Path
import sys
import threading
import urllib.parse
import uuid

import capture_tests

ROOT = Path(__file__).resolve().parents[1]


def main():
    folder = ROOT / "results/verification/autonomy" / (
        datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ") + "-input-fixture-" + uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    seen = []

    class Destination(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            seen.append(["destination", self.path])
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            self.wfile.write(b"<!doctype html><title>Independent fixture destination</title><p>Controlled destination loaded.</p>")

        def log_message(self, *_):
            pass

    destination = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Destination)

    class Search(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            seen.append(["search", self.path])
            path = urllib.parse.urlsplit(self.path)
            result = path.path == "/search"
            html = '<!doctype html><title>Original fixture ' + ("results" if result else "home") + '</title><form action="/search"><input name="q"><button name="send">Search</button></form>'
            if result:
                html += f'<p><a href="http://127.0.0.1:{destination.server_port}/destination">Original fixture result</a></p>'
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            self.wfile.write(html.encode())

        def log_message(self, *_):
            pass

    search = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Search)
    servers = [search, destination]
    threads = [threading.Thread(target=server.serve_forever, daemon=True) for server in servers]
    for thread in threads:
        thread.start()
    script = folder / "input.tsv"
    script.write_text("fill\tq\talpha\nkey\tEnter\nsnapshot\tfirst-results\nlink\tOriginal\nsnapshot\tdestination\nback\nsnapshot\tback\nfill\tq\tbeta + café\nkey\tEnter\nsnapshot\tsecond-results\n", encoding="utf-8")
    executable = ROOT / "ChezCat.exe"
    evidence = folder / "journey"
    command = [str(executable), f"http://127.0.0.1:{search.server_port}/", "--dev-input-script", str(script),
               "--dev-input-output", str(evidence), "--javascript", "--no-profile"]
    before = capture_tests.source_identity(ROOT)
    record = {"scope": __doc__, "source_before": before, "executable_sha256": capture_tests.sha256(executable)}
    try:
        record["command"] = capture_tests.execute(command, folder / "command.log", ROOT, 180)
    finally:
        for server in servers:
            server.shutdown()
            server.server_close()
        for thread in threads:
            thread.join(timeout=3)
    record["requests"] = seen
    record["source_after"] = capture_tests.source_identity(ROOT)
    # Enter selects the named submit button, so its successful empty value is
    # part of the form payload. Assert that actual browser behavior exactly.
    expected = ["search", "/search?q=alpha&send="] in seen and ["destination", "/destination"] in seen and ["search", "/search?q=beta+%2B+caf%C3%A9&send="] in seen
    checks = {
        "input_command_completed": record["command"]["state"] == "completed",
        "both_form_queries_and_cross_origin_destination_requested": expected,
        "first_results_rendered": all(('text="' + word + '"') in (evidence / "03-first-results.layout.txt").read_text(encoding="utf-8") for word in ("Original", "fixture", "result")) if (evidence / "03-first-results.layout.txt").exists() else False,
        "destination_loaded": "Independent fixture destination" in (evidence / "05-destination.source.html").read_text(encoding="utf-8") if (evidence / "05-destination.source.html").exists() else False,
        "back_returns_to_results": "Original fixture results" in (evidence / "07-back.source.html").read_text(encoding="utf-8") if (evidence / "07-back.source.html").exists() else False,
        "second_query_typed": 'beta + café' in (evidence / "input.log").read_text(encoding="utf-8") if (evidence / "input.log").exists() else False,
        "source_unchanged": before == record["source_after"],
    }
    record["checks"] = checks
    record["passed"] = all(checks.values())
    record["files"] = {str(path.relative_to(folder)): {"bytes": path.stat().st_size, "sha256": capture_tests.sha256(path)}
                       for path in folder.rglob("*") if path.is_file()}
    capture_tests.write_json(folder / "manifest.json", record)
    print(json.dumps(checks, indent=2, ensure_ascii=False))
    print("Evidence: " + str(folder))
    return 0 if record["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
