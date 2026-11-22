"""Offline executable check of complete gzip CSS and actual tail layout/paint."""
from datetime import datetime, timezone
import http.server
import json
from pathlib import Path
import threading
import uuid
import capture_tests

ROOT=Path(__file__).resolve().parents[1]

def main():
    folder=capture_tests.verification_root(ROOT) / 'runs'/(datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-stylesheet-journey-'+uuid.uuid4().hex[:8]);folder.mkdir(parents=True)
    seen=[];encoded=(ROOT/'tools/fixtures/large-tail.css.gz').read_bytes()
    class Site(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            seen.append(self.path)
            if self.path=='/large.css':
                body=encoded;mime='text/css; charset=utf-8'
            else:
                dynamic=self.path=='/dynamic'
                markup='<head>'+('' if dynamic else '<link rel=stylesheet href=/large.css><link rel=stylesheet href=/large.css>')+'</head><body><div id=target>Complete tail marker</div><script>'
                verify="function verifyTail(){var target=document.getElementById('target');if(target.clientWidth!==173||target.clientHeight!==29)throw 'CSS tail geometry';document.title='Complete CSS applied';}"
                script="var sheet=document.createElement('link');sheet.rel='stylesheet';sheet.href='/large.css';sheet.onload=verifyTail;sheet.onerror=function(){throw 'CSS request';};document.head.appendChild(sheet);" if dynamic else 'window.onload=verifyTail;'
                body=('<!doctype html><meta charset=utf-8><title>Waiting</title>'+markup+verify+script+'</script></body>').encode('utf-8');mime='text/html; charset=utf-8'
            self.send_response(200);self.send_header('Content-Type',mime)
            if self.path=='/large.css':self.send_header('Content-Encoding','gzip')
            self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
        def log_message(self,*_):pass
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Site);worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start()
    before=capture_tests.source_identity(ROOT);record={'scope':__doc__,'executable_sha256':capture_tests.sha256(ROOT/'ChezCat.exe'),'source_before':before,'runs':[]};checks={}
    try:
        for route in ['initial','dynamic']:
            script=folder/(route+'.tsv');script.write_text('wait\t500\nsnapshot\tstyled\n',encoding='utf-8')
            start=len(seen);out=folder/route
            row=capture_tests.execute([str(ROOT/'ChezCat.exe'),f'http://127.0.0.1:{server.server_port}/{route}','--dev-input-script',str(script),'--dev-input-output',str(out),'--javascript','--no-profile'],folder/(route+'.log'),ROOT,180);record['runs'].append(row)
            def read(suffix):
                path=next(out.glob('*-styled.'+suffix),None)
                return path.read_bytes() if path else b''
            activity=read('activity.txt').decode('utf-8');layout=read('layout.txt').decode('utf-8');ppm=read('ppm');parts=ppm.split(b'\n',3);pixels=parts[3] if len(parts)==4 else b''
            checks[route+'_completed']=row['state']=='completed'
            checks[route+'_geometry_checked_before_load']= 'Page title: Complete CSS applied' in activity and '0 error(s)' in activity
            checks[route+'_real_tail_text']= 'text="Complete"' in layout and 'text="tail"' in layout
            checks[route+'_real_tail_pixels']=any(pixels[index:index+3]==b'\x12\x34\x56' for index in range(0,len(pixels),3))
            checks[route+'_one_gzip_fetch']=seen[start:].count('/large.css')==1
            if route=='initial':checks['initial_cached_complete_reuse']='reused stylesheet from this document load' in activity
    finally:server.shutdown();server.server_close();worker.join(timeout=3)
    checks['source_unchanged']=before==capture_tests.source_identity(ROOT);record.update(checks=checks,requests=seen,source_after=capture_tests.source_identity(ROOT));capture_tests.write_json(folder/'manifest.json',record)
    print(json.dumps(checks,indent=2));print('Evidence: '+str(folder));return 0 if all(checks.values()) else 1

if __name__=='__main__':raise SystemExit(main())
