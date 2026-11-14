"""Exercise the actual executable's input driver on original local fixtures.

This is controlled harness validation, never evidence of live Google results.
"""
from datetime import datetime, timezone
import argparse
import http.server
import json
import re
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
    parser.add_argument("--attribute-nodes",action="store_true",help="exercise real Attr replacement, native disabled state and applied style")
    args=parser.parse_args()
    fixture=ROOT/"tools/fixtures/offline-event-namespace.html"
    extra=fixture.read_text(encoding="utf-8") if args.event_namespace else ""
    if args.attribute_nodes:
        extra+='''<div id="attr-target" title="old">Waiting for attribute nodes</div><script>
(function(){
var target=document.getElementById('attr-target'),old=target.getAttributeNode('title');
var incoming=document.createAttribute('TITLE');incoming.value='replacement';
if(target.setAttributeNode(incoming)!==old||old.ownerElement!==null||old.value!=='old'||incoming.ownerElement!==target||target.getAttributeNode('title')!==incoming)throw 'Attr identity';
var first=document.createAttributeNS('urn:original','one:token');first.value='first';target.setAttributeNodeNS(first);
var next=document.createAttributeNS('urn:original','two:token');next.value='next';
if(target.setAttributeNode(next)!==first||first.ownerElement!==null||target.getAttributeNS('urn:original','token')!=='next'||target.getAttribute('one:token')!==null)throw 'expanded name';
var button=document.getElementsByName('send').item(0),disabled=document.createAttribute('disabled');
button.setAttributeNode(disabled);if(!button.disabled)throw 'native disabled';button.removeAttributeNode(disabled);if(button.disabled)throw 'native enabled';
var style=document.createAttribute('style');style.value='display:block;width:133px;color:#445566';target.setAttributeNode(style);
if(target.clientWidth!==133)throw 'applied geometry';target.textContent='Attribute nodes ready';
})();</script>'''
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
            if path.path == "/fixture-parser.js":
                body=b"window.fixtureParserLoads=(window.fixtureParserLoads||0)+1;"
                self.send_response(200)
                self.send_header("Content-Type","application/javascript")
                self.send_header("Content-Length",str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
            if path.path == "/fixture-token.css":
                body=(ROOT/'tools/fixtures/token-attribute.css').read_bytes()
                self.send_response(200)
                self.send_header("Content-Type","text/css")
                self.send_header("Content-Length",str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
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
    record["runner_sha256"]=capture_tests.sha256(Path(__file__))
    if args.attribute_nodes:
        import hashlib
        record["attribute_fixture_sha256"]=hashlib.sha256(extra.encode("utf-8")).hexdigest()
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
    def stage(text, prefix):
        line=next((line for line in text.splitlines() if line.startswith(prefix)), "")
        return dict(re.findall(r"(\w+)=([^\s]+)",line))
    def paired(path):
        text=path.read_text(encoding="utf-8")
        current=stage(text,"Stage document=")
        layout=stage(text,"Stage layout=")
        canvas=stage(text,"Stage canvas=")
        ppm=path.with_name(path.name.removesuffix(".activity.txt")+".ppm")
        return ("Stage paired_ppm_written=1" in text and ppm.is_file() and ppm.stat().st_size>32
                and current.get("document", "unknown")!="unknown"
                and canvas.get("document")==current.get("document")==layout.get("layout_document")
                and canvas.get("dom_commit")==current.get("dom_commit")
                and canvas.get("layout")==layout.get("layout")!="unknown"
                and canvas.get("layout_dom")==layout.get("layout_dom") and canvas.get("page_view")=="1")
    activities=list(evidence.glob("*.activity.txt"))
    checks["stage_snapshots_match_generated_ppm_and_document_layout"]=bool(activities) and all(paired(path) for path in activities)
    first= snapshot("first-results","activity.txt")
    arrived=snapshot("destination","activity.txt")
    first_doc=stage(first,"Stage document=")
    first_layout=stage(first,"Stage layout=")
    hit=stage(arrived,"Stage last_link_hit ")
    arrived_doc=stage(arrived,"Stage document=")
    checks["stage_real_activation_preserves_originating_document_and_layout"]=(
        int(first_layout.get("viewport_link_regions","0"))>0 and hit.get("matched_press")=="1"
        and hit.get("node","unknown")!="unknown" and hit.get("document")==first_doc.get("document")
        and hit.get("layout")==first_layout.get("layout")
        and arrived_doc.get("document")!=hit.get("document")
        and first_doc.get("navigation_committed","unknown")!="unknown")
    if args.event_namespace:
        initial=(evidence/"00-initial.layout.txt").read_text(encoding="utf-8") if (evidence/"00-initial.layout.txt").is_file() else ""
        checks["event_namespace_startup_rendered"]=all(('text="'+word+'"') in initial for word in ("Event","namespace","ready"))
        checks["fresh_location_instance_owner_lookup_rendered"]=all(('text="'+word+'"') in initial for word in ("Location","instance","ready"))
        checks["real_namespaced_html_creation_rendered"]=all(('text="'+word+'"') in initial for word in ("Namespaced","HTML","ready"))
        checks["parsed_script_source_reflection_rendered"]=all(('text="'+word+'"') in initial for word in ("Parsed","script","URL","ready"))
        checks["measured_document_timing_rendered"]=all(('text="'+word+'"') in initial for word in ("Measured","document","timing","ready"))
        checks["unavailable_animation_detection_rendered"]=all(('text="'+word+'"') in initial for word in ("Unavailable","animation","detected"))
        checks["real_token_selector_style_and_query_rendered"]=all(('text="'+word+'"') in initial for word in ("Token","selector","ready"))
        first_pixels=(evidence/'00-initial.ppm').read_bytes().split(b'\n',3)[3]
        checks["token_selector_real_style_background_painted"]=sum(first_pixels[index:index+3]==bytes([49,95,154]) for index in range(0,len(first_pixels),3))>100
        checks["token_stylesheet_fetched_once_per_document"]=sum(row==["search","/fixture-token.css"] for row in seen)==sum(row[0]=="search" and urllib.parse.urlsplit(row[1]).path in ["/","/search"] for row in seen)
        timings=[re.search(r'Measured document timing \(integer epoch ms\): navigationStart=(\d+) requestStart=(\d+) responseStart=(\d+) responseEnd=(\d+) finalHeadersParsed=(\d+)',path.read_text(encoding='utf-8')) for path in activities]
        checks["actual_native_timing_measurements_reported"]=bool(timings) and all(match and 0<int(match[1])<=int(match[2])<=int(match[3])<=int(match[4]) and int(match[5])>=int(match[3]) for match in timings)
        checks["parsed_script_fetch_once_per_document_and_reading_never_refetches"]=sum(row==["search","/fixture-parser.js"] for row in seen)==sum(row[0]=="search" and urllib.parse.urlsplit(row[1]).path in ["/","/search"] for row in seen)
        checks["native_ui_keyboard_rendered"]=all(('text="'+word+'"') in snapshot("ui-keyboard","layout.txt") for word in ("Native","UI","observed"))
        checks["fragment_and_real_image_completion_rendered"]=all(('text="'+word+'"') in initial for word in ("Fragment","image","loaded"))
        checks["fragment_image_requested_and_inert_script_unfetched"]=["search","/fixture-image.ppm"] in seen and not any(row[1].startswith("/fixture-inert.js") for row in seen)
        checks["backed_history_rewrite_rendered"]=all(('text="'+word+'"') in initial for word in ("History","entry","ready"))
        activities=list(evidence.glob("*.activity.txt"))
        checks["subsystem_has_no_script_errors"]=bool(activities) and all("0 error(s)." in path.read_text(encoding="utf-8") for path in activities)
    if args.attribute_nodes:
        checks["attribute_nodes_identity_namespace_native_control_and_geometry_rendered"]=all(('text="'+word+'"') in snapshot("first-results","layout.txt") for word in ("Attribute","nodes","ready"))
        checks["attribute_nodes_have_no_script_errors"]=all("0 error(s)." in path.read_text(encoding="utf-8") for path in activities)
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
