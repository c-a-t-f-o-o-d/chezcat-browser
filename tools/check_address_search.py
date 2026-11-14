"""Offline actual-executable address and settings journey; loopback only."""
from datetime import datetime, timezone
import http.server
import json
from pathlib import Path
import threading
import urllib.parse
import uuid
import capture_tests

ROOT=Path(__file__).resolve().parents[1]

def main():
    folder=ROOT/'results/verification/autonomy'/(datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-address-search-'+uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    seen=[]
    class Site(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            seen.append(self.path)
            query=urllib.parse.parse_qs(urllib.parse.urlsplit(self.path).query)
            import html
            text=html.escape(query.get('q',[''])[0])
            body=('<!doctype html><meta charset=utf-8><title>Original address search fixture</title>'
                  '<form action="/search"><input name=q><button>Search</button></form>'
                  +('<h1>Result for '+text+'</h1><a href="/destination">Original result destination</a>' if query else '<p>Original local home</p>')
                  +('<p>Destination reached by result activation.</p>' if self.path=='/destination' else '')).encode('utf-8')
            self.send_response(200);self.send_header('Content-Type','text/html; charset=utf-8');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
        def log_message(self,*_):pass
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Site)
    worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start()
    base=f'http://127.0.0.1:{server.server_port}'
    before=capture_tests.source_identity(ROOT)
    record={'scope':__doc__,'executable_sha256':capture_tests.sha256(ROOT/'ChezCat.exe'),'source_before':before,'runs':[]}
    def run(name,actions):
        script=folder/(name+'.tsv');script.write_text(actions,encoding='utf-8')
        row=capture_tests.execute([str(ROOT/'ChezCat.exe'),base+'/', '--dev-input-script',str(script),'--dev-input-output',str(folder/name),'--profile-dir',str(folder/'profile'),'--javascript'],folder/(name+'.log'),ROOT,180)
        record['runs'].append(row)
        return row
    try:
        first=run('journey',f'provider\t{base}/search\nsettings\tOpen\nsettings\tDuckDuckGo\nsettings\tCancel\nsettings\tOpen\nsettings\tBing\nsnapshot\tprovider-options\nsettings\tApply\naddress\tC++ & C# café 🦀\nsnapshot\tbefore-enter\nkey\tEnter\nsnapshot\tresults\nlink-target\t/destination\nsnapshot\tdestination\nback\nsnapshot\tback\naddress\tmeow\nkey\tEnter\nsnapshot\tsecond\nsettings\tOpen\nsettings\tNone\nsettings\tApply\nsnapshot\tnone-saved\n')
        saved=(folder/'profile/profile.txt').read_text(encoding='utf-8') if (folder/'profile/profile.txt').is_file() else ''
        mark=len(seen)
        disabled=run('none-restart',f'provider\t{base}/search\naddress\tprivate ordinary text\nkey\tEnter\n')
        disabled_requests=seen[mark:]
        explicit=run('explicit-restart',f'provider\t{base}/search\naddress\t{base}/explicit\nkey\tEnter\nsnapshot\texplicit\n')
    finally:
        server.shutdown();server.server_close();worker.join(timeout=3)
    def layout(label):
        path=next((folder/'journey').glob('*-'+label+'.layout.txt'),None)
        return path.read_text(encoding='utf-8') if path else ''
    checks={
        'actual_input_journey_completed':first['state']=='completed',
        'unicode_punctuation_one_encoded_q':'/search?q=C%2B%2B+%26+C%23+caf%C3%A9+%F0%9F%A6%80' in seen,
        'before_enter_has_no_result':'Result' not in layout('before-enter'),
        'results_rendered':'C++' in layout('results') and 'href=Some' in layout('results'),
        'destination_activated':'Destination' in layout('destination') and '/destination' in seen,
        'one_back_returns_results':'C++' in layout('back'),
        'different_address_query_rendered':'meow' in layout('second') and '/search?q=meow' in seen,
        'none_saved_and_reopened':'search_engine\tNone\n' in saved and disabled['state']=='failed',
        'none_never_searches':not any('/search?' in request for request in disabled_requests),
        'none_still_navigates_explicit_url':explicit['state']=='completed' and '/explicit' in seen,
        'source_unchanged':before==capture_tests.source_identity(ROOT),
    }
    record.update(checks=checks,requests=seen,source_after=capture_tests.source_identity(ROOT))
    capture_tests.write_json(folder/'manifest.json',record)
    print(json.dumps(checks,indent=2));print('Evidence: '+str(folder))
    return 0 if all(checks.values()) else 1

if __name__=='__main__':raise SystemExit(main())
