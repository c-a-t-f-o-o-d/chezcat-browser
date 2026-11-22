"""Original offline executable animation-wrapper/input/renderer journey.

Real document scheduling and cancellation; no public request, native desktop,
device, performance/conformance certification or live-site acceptance claim.
"""
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import uuid
import capture_tests

ROOT=Path(__file__).resolve().parents[1]


def main():
    folder=capture_tests.verification_root(ROOT) / 'runs'/(
        datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-animation-journey-'+uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    destination=folder/'destination.html'
    destination.write_text('<!doctype html><title>Original animation destination</title><p>Destination remains unchanged</p>',encoding='utf-8')
    script=folder/'input.tsv'
    script.write_text('click\trun\nwait\t150\nsnapshot\tcomplete\nclick\tpending\nsnapshot\tpending\naddress\t'+str(destination)+'\nkey\tEnter\nsnapshot\tnavigated\nwait\t100\nsnapshot\tsettled\n',encoding='utf-8')
    before=capture_tests.source_identity(ROOT)
    exe=ROOT/'ChezCat.exe';expected_exe=capture_tests.sha256(exe);out=folder/'journey'
    row=capture_tests.execute([str(exe),str(ROOT/'tools/fixtures/animation-wrappers.html'),
        '--dev-input-script',str(script),'--dev-input-output',str(out),
        '--width','640','--height','480','--javascript','--no-profile'],folder/'command.log',ROOT,120)
    def read(stage,suffix):
        path=next(out.glob('*-'+stage+'.'+suffix),None)
        return path.read_bytes() if path else b''
    def text(stage,suffix='activity.txt'):
        return read(stage,suffix).decode('utf-8',errors='replace')
    def green_pixels(stage):
        parts=read(stage,'ppm').split(b'\n',3)
        if len(parts)!=4 or parts[0]!=b'P6' or parts[2]!=b'255':return -1
        try:width,height=map(int,parts[1].split())
        except ValueError:return -1
        pixels=parts[3]
        if width<=0 or height<=0 or len(pixels)!=width*height*3:return -1
        return sum(pixels[index:index+3]==b'\x22\xaa\x44' for index in range(0,len(pixels),3))
    complete=text('complete');pending=text('pending');navigated=text('navigated');settled=text('settled')
    layout=text('complete','layout.txt')
    frames=re.search(r'Animation frames: (\d+) pending callback\(s\), (\d+) executed\.',complete)
    active=re.search(r'Animation frames: (\d+) pending callback\(s\), (\d+) executed\.',pending)
    checks={
        'process_completed':row['state']=='completed' and row.get('exit_code')==0,
        'initial_fixture_visible':'Page title: Original animation wrapper fixture' in text('initial'),
        'wrapped_native_callbacks_and_microtask_order':'Page title: Animation wrapper complete wcvwfamdr' in complete,
        'real_frame_batch_drained':frames is not None and tuple(map(int,frames.groups()))==(0,3),
        'actual_visible_callback_text':all('text="'+word+'"' in layout for word in ['Animation','wrapper','complete']),
        'applied_callback_pixels':green_pixels('initial')==0 and green_pixels('complete')>=180*24,
        'paired_render_output':all('Stage paired_ppm_written=1' in text(stage) for stage in ['complete','pending','navigated','settled']),
        'no_wrapper_script_errors':re.search(r'\d+ script\(s\), \d+ active listener\(s\), 0 error\(s\)\.',complete) is not None,
        'subsequent_real_frame_work_pending':active is not None and int(active[1])==1 and int(active[2])>3,
        'actual_navigation_destination':'Page title: Original animation destination' in navigated,
        'navigation_remains_destination_after_settling':'Page title: Original animation destination' in settled and 'Page title: Pending original frame' not in settled,
        'destination_text_rendered':all('text="'+word+'"' in text('settled','layout.txt') for word in ['Destination','remains','unchanged']),
        'old_document_has_no_pending_frame_in_destination':'Animation frames: 1 pending' not in settled,
        'source_unchanged':before==capture_tests.source_identity(ROOT),
        'executable_unchanged':expected_exe==capture_tests.sha256(exe),
    }
    capture_tests.write_json(folder/'manifest.json',{'scope':__doc__,
        'source_before':before,'source_after':capture_tests.source_identity(ROOT),
        'executable_sha256':expected_exe,'run':row,'checks':checks,
        'control_destination_sha256':capture_tests.sha256(destination)})
    print(json.dumps(checks,indent=2));print('Evidence: '+str(folder))
    return 0 if all(checks.values()) else 1


if __name__=='__main__':raise SystemExit(main())
