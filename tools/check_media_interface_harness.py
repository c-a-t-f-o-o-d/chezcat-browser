"""Genuine media prototypes driving the existing original authored-video journey.

Owned loopback HTTP/App/input/offscreen only. Reuses every original video clock,
source-lifetime, event/Promise, native/script control and independent RGBA pixel
assertion unchanged. Adds actual canonical interface operations despite ordinary
own-property shadows. Initial blank child roots are inspected, never played.
No public service, native desktop/input, device, PCM, build or automatic license
acceptance. Cross-origin parent navigation remains separate unit evidence.
"""
import argparse
import contextlib
from datetime import datetime, timezone
import io
import json
from pathlib import Path
import re
import sys
import uuid


def require(value, message):
    if not value:
        raise ValueError(message)


def visible_markers(folder, label):
    paths=list((folder/'journey').glob('*-'+label+'.activity.txt'))
    require(len(paths)==1,'Expected one original actual stage: '+label)
    activity=paths[0].read_text(encoding='utf-8')
    stem=paths[0].name.removesuffix('.activity.txt')
    layout=paths[0].with_name(stem+'.layout.txt').read_text(encoding='utf-8')
    source=paths[0].with_name(stem+'.source.html').read_text(encoding='utf-8')
    visible=' '.join(re.findall(r'text="([^"\n]*)"',layout))
    markers=dict(re.findall(r'\b(iface_[a-z_]+)=(\d+)',visible))
    def fields(prefix):
        line=next((line for line in activity.splitlines() if line.startswith(prefix)),'')
        return dict(re.findall(r'(\w+)=([^\s]+)',line))
    document,geometry,canvas=(fields(prefix) for prefix in ['Stage document=','Stage layout=','Stage canvas='])
    paired=('Stage paired_ppm_written=1' in activity and document.get('document','unknown')!='unknown'
        and canvas.get('document')==document.get('document')==geometry.get('layout_document')
        and canvas.get('dom_commit')==document.get('dom_commit') and canvas.get('layout')==geometry.get('layout')!='unknown'
        and canvas.get('layout_dom')==geometry.get('layout_dom') and canvas.get('page_view')=='1')
    return activity,source,visible,markers,paired


def interface_checks(folder):
    stages={}
    checks={}
    labels=['metadata','reloaded','js-issued','js-settled','js-frozen',
            'js-seek-settled','js-seek-frozen','js-playing','js-play-paused',
            'js-play-frozen','stopped']
    for label in labels:
        activity,source,visible,markers,paired=visible_markers(folder,label)
        stages[label]=markers
        checks[label+':genuine_interfaces_and_wrong_brand_controls']=(
            markers.get('iface_roots')=='1' and markers.get('iface_brands')=='1'
            and markers.get('iface_brand_promise')=='1' and markers.get('iface_child_roots')=='1'
            and markers.get('iface_stub_calls')=='0')
        checks[label+':actual_readable_interface_result']='Real media interfaces' in visible and 'Canonical media controls' in visible
        checks[label+':current_script_errors_zero']='0 error(s).' in activity
        checks[label+':no_audio_device_open']='Audio owner opened=false retained_buffers=0 busy=false' in activity
        checks[label+':same_document_dom_layout_ppm_pair']=paired
        checks[label+':actual_authored_interface_source']='id="interface-state"' in source or 'id=interface-state' in source or 'id="interface-state"' in source.replace("'",'"')
    checks['canonical_load_retained_raw_source_and_bypassed_own_shadows']=stages['reloaded'].get('iface_load')=='1'
    checks['canonical_pause_bypassed_own_method_and_froze_real_player']=all(stages[label].get('iface_pause')=='1' for label in ['js-issued','js-settled','js-frozen'])
    checks['canonical_time_setter_bypassed_own_data_and_selected_real_frame']=all(stages[label].get('iface_seek')=='1' for label in ['js-seek-settled','js-seek-frozen'])
    checks['canonical_play_bypassed_own_method_and_fulfilled_real_promise']=all(stages[label].get('iface_play')=='1' for label in ['js-playing','js-play-paused','js-play-frozen'])
    return checks,stages


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo',type=Path,default=Path(__file__).resolve().parents[1])
    parser.add_argument('--exe',type=Path)
    args=parser.parse_args()
    root=args.repo.resolve()
    sys.path.insert(0,str(root/'tools'))
    import accept_license
    import capture_tests
    import check_authored_video_harness as authored
    import check_network_media_harness as network
    require(authored.ROOT.resolve()==root and network.ROOT.resolve()==root,'Checker imports must belong to the selected repository.')
    executable=(args.exe or root/'ChezCat.exe').resolve()
    folder=capture_tests.verification_root(root) / 'autonomy'/(datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-media-interface-journey-'+uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    before=capture_tests.source_identity(root)
    exe_before=capture_tests.sha256(executable) if executable.is_file() else None
    record={'schema_version':1,'passed':False,'state':'running','scope':__doc__,
        'source_before':before,'source_after':before,'executable_sha256':exe_before,
        'license_accepted_by_driver':False,'os_trust_changed':False,'public_service_requests_made_by_driver':0,
        'runner_sha256':capture_tests.sha256(Path(__file__)),'checks':{},'case':None}
    manifest=folder/'manifest.json'
    capture_tests.write_json(manifest,record)
    checks=record['checks']
    try:
        require(exe_before is not None,'Build/select the actual product executable before this journey.')
        accept_license.terms()
        checks['existing_exact_license_acceptance']=accept_license.check(accept_license.directory())
        require(checks['existing_exact_license_acceptance'],'Existing explicit acceptance is required; this driver never accepts terms.')
        fixture=root/'tools/fixtures/media-interface.html'
        payloads=network.fixture_payloads()
        payloads.pop('/authored-video.html')
        payloads['/media-interface.html']=('text/html; charset=utf-8',fixture.read_bytes())
        proof={}

        def runner(source=None,trust_file=None):
            captured=io.StringIO()
            with contextlib.redirect_stdout(captured):
                status=authored.main(source=source,trust_file=trust_file,exe=executable)
            output=captured.getvalue()
            print(output,end='')
            evidence=re.findall(r'^Evidence: (.+)$',output,re.M)
            require(len(evidence)==1,'Expected exactly one reused checker evidence record.')
            child=Path(evidence[0].strip()).resolve()
            require(child.is_relative_to(capture_tests.verification_root(root)),'Original evidence must remain in selected product verification.')
            extra,markers=interface_checks(child)
            proof.update({'passed':status==0 and all(extra.values()),'checks':extra,'markers':markers,
                'authored_manifest':capture_tests.evidence_reference(child/'manifest.json', root),
                'authored_manifest_sha256':capture_tests.sha256(child/'manifest.json')})
            capture_tests.write_json(folder/'interface-proof.json',proof)
            return 0 if proof['passed'] else 1

        result=network.run_case('http',folder/'http',authored_runner=runner,served_payloads=payloads,
            entry_path='/media-interface.html',route_checks={
                'original_interface_page_served':('/media-interface.html',1),
                'original_clip_fetched_and_refetched':('/scene-intra.ivf',2),
                'actual_leave_destination_served':('/authored-video-next.html',1)},scope=__doc__)
        record['case']={'manifest':'http/manifest.json','sha256':capture_tests.sha256(folder/'http/manifest.json'),'passed':result['passed']}
        checks.update({'original:'+name:value is True for name,value in result['checks'].items()})
        checks.update({'interface:'+name:value is True for name,value in proof['checks'].items()})
        checks['exact_executable_and_source_identity']=(result['executable_sha256']==exe_before
            and result['source_before']==result['source_after']==before)
        checks['independent_original_fixture_identity']=(capture_tests.sha256(root/'tools/fixtures/scene-intra.ivf')=='8d92ca698ea73c0fa07ee6151fa5e35e5e5d480ac21a65e8aae7dae822bfa684'
            and capture_tests.sha256(root/'tools/fixtures/scene-intra.rgba')=='cdc6a81119b059305ffeaa8db53e2fd6a55e9ff5f487a88255b861f5184f7a6f')
        record['fixture_sha256']=capture_tests.sha256(fixture)
        record['interface_proof']={'manifest':'interface-proof.json','sha256':capture_tests.sha256(folder/'interface-proof.json')}
    except (OSError,ValueError,KeyError,TypeError,RuntimeError) as error:
        record['error']=str(error)
        checks['complete_interface_journey']=False
    after=capture_tests.source_identity(root)
    checks['source_unchanged']=after==before
    checks['executable_unchanged']=exe_before is not None and capture_tests.sha256(executable)==exe_before
    record.update(source_after=after,passed=bool(checks) and all(checks.values()),state='completed')
    capture_tests.write_json(manifest,record)
    print(json.dumps(checks,indent=2))
    print('Evidence: '+str(folder))
    return 0 if record['passed'] else 1


if __name__=='__main__':
    raise SystemExit(main())
