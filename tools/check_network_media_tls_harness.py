"""Actual App media-source TLS refusals over controlled loopback origins.

A readable HTTP document selects an explicitly CORS-enabled HTTPS clip. Verify
that an untrusted issuer or mismatched IP SAN prevents HTTP media transmission,
decoded player installation and playback. Uses original public TEST certificates,
process-only trust and existing explicit acceptance; no OS trust/network changes.
"""
from datetime import datetime, timezone
import hashlib
import http.server
import json
from pathlib import Path
import re
import threading
import uuid

import accept_license
import capture_tests
from https_fixture_server import FIXTURES, TlsFixtureServer, context

ROOT=Path(__file__).resolve().parents[1]


def run_case(name,folder):
    folder.mkdir()
    lock=threading.Lock()
    state={'tls_attempts':0,'tls_http':[],'document_http':[],'tls_failures':0}
    clip=(ROOT/'tools/fixtures/scene-intra.ivf').read_bytes()

    class MediaHandler(http.server.BaseHTTPRequestHandler):
        protocol_version='HTTP/1.1'
        def setup(self):
            super().setup()
            self.connection.settimeout(3)
        def do_GET(self):
            self.connection.settimeout(3)
            with lock: state['tls_http'].append(self.path)
            self.send_response(200)
            self.send_header('Content-Type','video/x-ivf')
            self.send_header('Content-Length',str(len(clip)))
            self.send_header('Access-Control-Allow-Origin',self.headers.get('Origin','null'))
            self.send_header('Access-Control-Allow-Credentials','true')
            self.send_header('Connection','close')
            self.end_headers();self.wfile.write(clip);self.close_connection=True
        def log_message(self,*_): pass

    class MediaServer(TlsFixtureServer):
        daemon_threads=False
        block_on_close=True
        def get_request(self):
            with lock: state['tls_attempts']+=1
            try: return super().get_request()
            except OSError:
                with lock: state['tls_failures']+=1
                raise

    address='127.0.0.1'
    host=address if name=='identity' else 'localhost'
    wrong_san=ROOT/'tools/fixtures/media-tls-wrong-san'
    certificate=wrong_san/'wrong-san-chain.pem' if name=='identity' else FIXTURES/'server-p256-chain.pem'
    key=wrong_san/'wrong-san-key.pem' if name=='identity' else FIXTURES/'server-p256-key.pem'
    tls=context(certificate,key,'1.3')
    secure=MediaServer((address,0),MediaHandler,tls)
    source=f'https://{host}:{secure.server_port}/scene-intra.ivf'
    page=("<!doctype html><html><head><meta http-equiv=\"Content-Security-Policy\" content=\"media-src *\">"
          "<title>Media TLS refusal fixture</title></head><body><h1>Readable source document</h1>"
          f'<video id="media" src="{source}" crossorigin="anonymous" controls width="480" height="360"></video>'
          '<p id="outcome">Waiting for media</p><script>var v=document.getElementById("media");'
          'v.addEventListener("error",function(){document.getElementById("outcome").textContent="Media TLS source rejected";});'
          '</script></body></html>').encode()

    class PageHandler(http.server.BaseHTTPRequestHandler):
        protocol_version='HTTP/1.1'
        def setup(self):
            super().setup()
            self.connection.settimeout(3)
        def do_GET(self):
            self.connection.settimeout(3)
            with lock: state['document_http'].append(self.path)
            self.send_response(200)
            self.send_header('Content-Type','text/html; charset=utf-8')
            self.send_header('Content-Length',str(len(page)))
            self.send_header('Connection','close')
            self.end_headers();self.wfile.write(page);self.close_connection=True
        def log_message(self,*_): pass

    plain=http.server.ThreadingHTTPServer(('127.0.0.1',0),PageHandler)
    plain.daemon_threads=False;plain.block_on_close=True
    servers=[secure,plain]
    workers=[threading.Thread(target=server.serve_forever,kwargs={'poll_interval':.05},daemon=True) for server in servers]
    for worker in workers: worker.start()
    script=folder/'input.tsv';script.write_text('snapshot\trejected\n',encoding='utf-8',newline='\n')
    output=folder/'app'
    trust_file=wrong_san/'root.pem' if name=='identity' else ROOT/'trust/roots.pem'
    url=f'http://127.0.0.1:{plain.server_port}/page'
    try:
        row=capture_tests.execute([str(ROOT/'ChezCat.exe'),url,'--trust-file',str(trust_file),
            '--javascript','--no-profile','--dev-input-script',str(script),'--dev-input-output',str(output)],folder/'command.log',ROOT,75)
    finally:
        for server in servers: server.shutdown()
        for server in servers: server.server_close()
        for worker in workers: worker.join(timeout=3)
    activity=next(output.glob('*-rejected.activity.txt'),None)
    text=activity.read_text(encoding='utf-8') if activity else ''
    layout=activity.with_name(activity.name.replace('.activity.txt','.layout.txt')) if activity else None
    visible=' '.join(re.findall(r'text="([^"\n]*)"',layout.read_text(encoding='utf-8'))) if layout and layout.is_file() else ''
    error=('subjectAltName does not match the requested host' in text) if name=='identity' else (
        'does not reach a supplied trusted root' in text or 'No supplied trusted root validates' in text)
    with lock: recorded=dict(state)
    checks={'process_completed':row['state']=='completed' and row['exit_code']==0,
        'actual_tls_source_attempt':recorded['tls_attempts']==1,
        'no_authenticated_media_http':not recorded['tls_http'],
        'only_source_document_requested':recorded['document_http']==['/page'],
        'exact_tls_refusal_reason':error,
        'bound_media_remains_unready':re.search(r'HTML video binding[^\n]*ready_state=0 network_state=3 width=0 height=0[^\n]*source_pending=false error=Some\(',text) is not None,
        'no_decoded_player':'Local video: none' in text,
        'actual_error_event_rendered':'Media TLS source rejected' in visible,
        'no_script_error':'0 error(s).' in text,
        'paired_canvas':bool(activity and activity.with_name(activity.name.replace('.activity.txt','.ppm')).is_file()),
        'owned_servers_stopped':all(not worker.is_alive() for worker in workers)}
    result={'passed':all(checks.values()),'checks':checks,'run':row,'source_document':url,
        'media_source':source,'trust_file_sha256':capture_tests.sha256(trust_file),
        'certificate_sha256':capture_tests.sha256(certificate),
        'fixture_page_sha256':hashlib.sha256(page).hexdigest(),'server':recorded,'scope':__doc__}
    capture_tests.write_json(folder/'manifest.json',result)
    return result


def main():
    folder=capture_tests.verification_root(ROOT) / 'runs'/(
        datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-media-tls-refusal-'+uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    before=capture_tests.source_identity(ROOT)
    exe=capture_tests.sha256(ROOT/'ChezCat.exe')
    checks={'existing_exact_acceptance':False}
    cases=[]
    try:
        accept_license.terms()
        checks['existing_exact_acceptance']=accept_license.check(accept_license.directory())
        if not checks['existing_exact_acceptance']: raise ValueError('Existing explicit license acceptance is required.')
        for name in ['untrusted','identity']:
            result=run_case(name,folder/name)
            cases.append({'name':name,'passed':result['passed'],'manifest':name+'/manifest.json'})
            checks.update({name+':'+key:value is True for key,value in result['checks'].items()})
    except (OSError,ValueError,KeyError,RuntimeError) as error:
        checks['case_error']=False
        capture_tests.write_json(folder/'error.json',{'error':str(error)})
    after=capture_tests.source_identity(ROOT)
    checks['both_cases_completed']=len(cases)==2
    checks['source_unchanged']=before==after
    checks['executable_unchanged']=exe==capture_tests.sha256(ROOT/'ChezCat.exe')
    capture_tests.write_json(folder/'manifest.json',{'passed':all(checks.values()),'checks':checks,
        'cases':cases,'source_before':before,'source_after':after,'executable_sha256':exe,
        'runner_sha256':capture_tests.sha256(Path(__file__)),'scope':__doc__,
        'license_accepted_by_driver':False,'os_trust_changed':False,'public_service_requests':0})
    print(json.dumps(checks,indent=2));print('Evidence: '+str(folder))
    return 0 if all(checks.values()) else 1


if __name__=='__main__': raise SystemExit(main())
