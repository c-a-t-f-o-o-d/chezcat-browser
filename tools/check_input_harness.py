"""Exercise the actual executable's input driver on original local fixtures.

This is controlled harness validation, never evidence of live Google results.
"""
from datetime import datetime, timezone
import argparse
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
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event-namespace",action="store_true",help="include the original local Event/namespace subsystem fixture")
    args=parser.parse_args()
    fixture=ROOT/"tools/fixtures/offline-event-namespace.html"
    extra=fixture.read_text(encoding="utf-8") if args.event_namespace else ""
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
            if path.path == "/fixture-image.ppm":
                body=b"P6\n2 1\n255\n"+bytes([12,34,56,78,90,123])
                self.send_response(200)
                self.send_header("Content-Type","image/x-portable-pixmap")
                self.send_header("Content-Length",str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
            result = path.path == "/search"
            html = '<!doctype html><title>Original fixture ' + ("results" if result else "home") + '</title><form action="/search"><input name="q"><button name="send">Search</button></form>'
            if result:
                html += f'<p><a href="http://127.0.0.1:{destination.server_port}/destination">Original fixture result</a></p>'
            html += extra
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
    actions="fill\tq\talpha\n"+("snapshot\tui-keyboard\n" if args.event_namespace else "")+"key\tEnter\nsnapshot\tfirst-results\nlink\tOriginal\nsnapshot\tdestination\nback\nsnapshot\tback\nfill\tq\tbeta + café\nkey\tEnter\nsnapshot\tsecond-results\n"
    script.write_text(actions, encoding="utf-8")
    executable = ROOT / "ChezCat.exe"
    evidence = folder / "journey"
    command = [str(executable), f"http://127.0.0.1:{search.server_port}/", "--dev-input-script", str(script),
               "--dev-input-output", str(evidence), "--javascript", "--no-profile"]
    before = capture_tests.source_identity(ROOT)
    record = {"scope": __doc__, "source_before": before, "executable_sha256": capture_tests.sha256(executable)}
    if args.event_namespace:
        record["original_subsystem_fixture_sha256"]=capture_tests.sha256(fixture)
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
    def snapshot(label,suffix):
        path=next(evidence.glob("*-"+label+"."+suffix),None)
        return path.read_text(encoding="utf-8") if path else ""
    checks = {
        "input_command_completed": record["command"]["state"] == "completed",
        "both_form_queries_and_cross_origin_destination_requested": expected,
        "first_results_rendered": all(('text="' + word + '"') in snapshot("first-results","layout.txt") for word in ("Original", "fixture", "result")),
        "destination_loaded": "Independent fixture destination" in snapshot("destination","source.html"),
        "back_returns_to_results": "Original fixture results" in snapshot("back","source.html"),
        "second_query_typed": 'beta + café' in (evidence / "input.log").read_text(encoding="utf-8") if (evidence / "input.log").exists() else False,
        "source_unchanged": before == record["source_after"],
    }
    if args.event_namespace:
        initial=(evidence/"00-initial.layout.txt").read_text(encoding="utf-8") if (evidence/"00-initial.layout.txt").is_file() else ""
        checks["event_namespace_startup_rendered"]=all(('text="'+word+'"') in initial for word in ("Event","namespace","ready"))
        checks["native_ui_keyboard_rendered"]=all(('text="'+word+'"') in snapshot("ui-keyboard","layout.txt") for word in ("Native","UI","observed"))
        checks["fragment_and_real_image_completion_rendered"]=all(('text="'+word+'"') in initial for word in ("Fragment","image","loaded"))
        checks["fragment_image_requested_and_inert_script_unfetched"]=["search","/fixture-image.ppm"] in seen and not any(row[1].startswith("/fixture-inert.js") for row in seen)
        activities=list(evidence.glob("*.activity.txt"))
        checks["subsystem_has_no_script_errors"]=bool(activities) and all("0 error(s)." in path.read_text(encoding="utf-8") for path in activities)
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
