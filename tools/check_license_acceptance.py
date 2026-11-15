"""Actual offline CLI/native-control acceptance; no physical input or live service."""
import argparse
from datetime import datetime,timezone
import hashlib,http.server,json,os
from pathlib import Path
import stat,sys,tempfile,threading,uuid
import accept_license
import capture_tests

ROOT=Path(__file__).resolve().parents[1]
DIGEST=accept_license.DIGEST
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--exe',type=Path,default=ROOT/'ChezCat.exe');args=parser.parse_args()
    exe=args.exe.resolve();before=capture_tests.source_identity(ROOT)
    folder=ROOT/'results/verification/autonomy'/(datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-license-'+uuid.uuid4().hex[:8]);folder.mkdir(parents=True)
    checks={};runs=[];seen=[];ui_records=[]
    class Fixture(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            seen.append(self.path);body=b'<!doctype html><title>Licensed offline fixture</title><p>Real licensed page</p>'
            self.send_response(200);self.send_header('Content-Type','text/html; charset=utf-8');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
        def log_message(self,*_):pass
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Fixture);thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
    url=f'http://127.0.0.1:{server.server_port}/licensed'
    def require(name,condition):
        checks[name]=bool(condition)
        if not condition:raise AssertionError(name)
    def execute(name,argv,env=None,cwd=ROOT,success=True):
        old={key:os.environ.get(key) for key in (env or {})}
        try:
            os.environ.update(env or {})
            row=capture_tests.execute(argv,folder/(name+'.log'),cwd,60)
        finally:
            for key,value in old.items():
                if value is None:os.environ.pop(key,None)
                else:os.environ[key]=value
        runs.append({'name':name,**row});require(name+'-exit',((row['state']=='completed' and row['exit_code']==0) if success else (row['state']=='failed' and row['exit_code']==1)));return (folder/(name+'.log')).read_bytes()
    try:
        with tempfile.TemporaryDirectory(prefix='chezcat-license-original-') as temporary:
            temp=Path(temporary)
            def native(name,actions,scope=None,old_receipt=None,readonly=False):
                case=temp/name;case.mkdir();receipt=case/'consent';profile=case/'profile';profile.mkdir()
                profile_bytes=b'OwnBrowser profile 1\njavascript\ttrue\nimages\ttrue\ndefault_zoom\t100\nhome\t\n'
                (profile/'profile.txt').write_bytes(profile_bytes)
                if old_receipt is not None:receipt.mkdir();(receipt/'receipt.json').write_bytes(old_receipt)
                if readonly:os.chmod(receipt/'receipt.json',stat.S_IREAD)
                script=case/'keys.tsv';script.write_text('license\t'+DIGEST+'\n'+actions,encoding='utf-8')
                report=case/'native.json';activity=case/'activity.txt';count=len(seen)
                try:
                    execute(name,[str(exe),url,'--activity-report',str(activity),'--profile-dir',str(profile),
                        '--license-receipt-dir',str(receipt),'--license-ui-script',str(script),'--license-ui-report',str(report)],success=not readonly)
                    ui=json.loads(report.read_text());ui_records.append(ui)
                    require(name+'-real-full-native-controls',all(ui.get(key) is True for key in ['scripted_offscreen','native_full_text_matches','native_readonly','native_edit_and_buttons','native_summary_and_identity']))
                    require(name+'-exact-identity',ui['license_version']=='1.1' and ui['license_sha256']==DIGEST and ui['license_bytes']==len((ROOT/'LICENSE').read_bytes()) and ui['initial_focus']=='decline')
                    require(name+'-profile-preserved',(profile/'profile.txt').read_bytes()==profile_bytes)
                    if scope and not readonly:
                        data=json.loads((receipt/'receipt.json').read_text());require(name+'-receipt-scope',accept_license.valid(data) and data['accepted_scope']==scope)
                        require(name+'-real-post-consent-page',len(seen)>count and 'Page title: Licensed offline fixture' in activity.read_text())
                    else:
                        require(name+'-no-network-before-consent',len(seen)==count and not activity.exists())
                        require(name+'-receipt-preserved',((receipt/'receipt.json').read_bytes()==old_receipt) if old_receipt is not None else not (receipt/'receipt.json').exists())
                finally:
                    if readonly:os.chmod(receipt/'receipt.json',stat.S_IREAD|stat.S_IWRITE)
                return receipt
            missing=temp/'missing';count=len(seen)
            execute('missing-headless',[str(exe),url,'--activity-report',str(temp/'missing.txt'),'--license-receipt-dir',str(missing)],success=False)
            require('missing-headless-no-network',len(seen)==count and not missing.exists())
            require('exact-offline-license-display',execute('view-license',[str(exe),'--license'])==(ROOT/'LICENSE').read_bytes())
            execute('help-without-consent',[str(exe),'--help']);require('view-help-no-network',len(seen)==count)
            execute('wrong-digest',[str(exe),'--accept-license','0'*64,'--license-scope','personal','--license-receipt-dir',str(missing)],success=False)
            require('wrong-digest-no-receipt',not missing.exists() and len(seen)==count)
            native('default-enter-declines','expect-focus\tdecline\nkey\tEnter\n')
            native('escape-declines','key\tEscape\n')
            receipt=native('personal-keyboard','expect-focus\tdecline\nkey\tTab\nexpect-focus\ttext\nkey\tCtrlEnd\nkey\tCtrlHome\nkey\tTab\nexpect-focus\tpersonal\nkey\tEnter\n','personal')
            original=(receipt/'receipt.json').read_bytes();count=len(seen)
            execute('same-text-reuse',[str(exe),url,'--activity-report',str(temp/'reuse.txt'),'--license-receipt-dir',str(receipt)])
            require('same-text-no-rewrite',(receipt/'receipt.json').read_bytes()==original and len(seen)>count)
            native('organization-keyboard','focus\torganization\nkey\tSpace\n','organization')
            stale=json.loads(original);stale['license_version']='1.0';stale['license_sha256']='632232a42c046628de0fd8c497e89f02055e1e919860bfa9fcd8fe3b5eb06d65'
            native('changed-terms-decline','key\tEnter\n',old_receipt=json.dumps(stale).encode())
            native('changed-terms-accept','choose\tpersonal\n','personal',old_receipt=json.dumps(stale).encode())
            native('corrupt-reaccept','choose\torganization\n','organization',old_receipt=b'{corrupt')
            native('unwritable-preserves','choose\tpersonal\n','personal',old_receipt=original,readonly=True)
            external=temp/'source-consent'
            execute('explicit-source-org',[sys.executable,str(ROOT/'tools/accept_license.py'),'--accept-license',DIGEST,'--license-scope','organization','--receipt-dir',str(external)])
            execute('source-check',[sys.executable,str(ROOT/'tools/accept_license.py'),'--check','--receipt-dir',str(external)])
            execute('source-runtime-compatible',[str(exe),url,'--activity-report',str(temp/'org.txt'),'--license-receipt-dir',str(external)])
            require('source-org-scope',json.loads((external/'receipt.json').read_text())['accepted_scope']=='organization')
            (external/'receipt.json').write_text((external/'receipt.json').read_text().replace('license_version','license_\\u0076ersion'),encoding='utf-8')
            execute('escaped-schema-source-refusal',[sys.executable,str(ROOT/'tools/accept_license.py'),'--check','--receipt-dir',str(external)],success=False)
            execute('escaped-schema-runtime-refusal',[str(exe),url,'--activity-report',str(temp/'escaped.txt'),'--license-receipt-dir',str(external)],success=False)
            home=temp/'home';appdata=home/'AppData/Local';appdata.mkdir(parents=True)
            execute('launch-from-home',[str(exe),'--accept-license',DIGEST,'--license-scope','personal'],env={'LOCALAPPDATA':str(appdata),'CHEZCAT_LICENSE_RECEIPT_DIR':str(appdata/'ChezCat/License')},cwd=home)
            require('home-default-storage-usable',accept_license.valid(json.loads((appdata/'ChezCat/License/receipt.json').read_text())))
            profile_receipt=appdata/'OwnBrowser/License';count=len(seen)
            execute('default-profile-runtime-refusal',[str(exe),'--accept-license',DIGEST,'--license-scope','personal','--license-receipt-dir',str(profile_receipt)],env={'LOCALAPPDATA':str(appdata)},success=False)
            execute('default-profile-source-refusal',[sys.executable,str(ROOT/'tools/accept_license.py'),'--accept-license',DIGEST,'--license-scope','personal','--receipt-dir',str(profile_receipt)],env={'LOCALAPPDATA':str(appdata)},success=False)
            require('default-profile-no-receipt',not profile_receipt.exists() and len(seen)==count)
            copied=folder/'copied';copied.mkdir();(copied/'receipt.json').write_bytes(original)
            execute('copied-checkout-refusal',[str(exe),url,'--activity-report',str(temp/'copy.txt'),'--license-receipt-dir',str(copied)],cwd=ROOT/'pages',success=False)
            require('copied-checkout-no-network',len(seen)==count)
            script=temp/'bad.tsv';script.write_text('license\t'+'0'*64+'\nchoose\tpersonal\n')
            execute('wrong-ui-digest',[str(exe),url,'--activity-report',str(temp/'bad.txt'),'--license-receipt-dir',str(missing),'--license-ui-script',str(script),'--license-ui-report',str(temp/'bad-ui.json')],success=False)
            require('wrong-ui-digest-no-consent',not missing.exists() and len(seen)==count)
            require('offline-only-fixtures',all(path=='/licensed' for path in seen))
    except Exception as error:
        capture_tests.write_json(folder/'failure.json',{'type':type(error).__name__,'message':str(error)})
    finally:server.shutdown();server.server_close();thread.join(timeout=5)
    after=capture_tests.source_identity(ROOT)
    checks['source_unchanged']=before==after
    passed=all(checks.values()) and not (folder/'failure.json').exists()
    manifest={'passed':passed,'checks':checks,'runs':runs,'ui':ui_records,'source_before':before,'source_after':after,'executable_sha256':capture_tests.sha256(exe),'scope':'Actual CLI and owned native EDIT/BUTTON/input handler acceptance with controlled loopback navigation. No physical input, live service, outside-user signature, phone notification or DRM claim.'}
    capture_tests.write_json(folder/'manifest.json',manifest);print('Evidence: '+str(folder));print('Acceptance checks: '+str(len(checks))+'; passed='+str(passed));return 0 if passed else 1
if __name__=='__main__':raise SystemExit(main())
