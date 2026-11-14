"""Offline executable check of synchronous dispatch stack guard and recovery.

Uses original local input/renderer fixture; no desktop input or public request.
"""
from datetime import datetime, timezone
import json
from pathlib import Path
import struct
import uuid
import capture_tests

ROOT=Path(__file__).resolve().parents[1]
def main():
    folder=ROOT/'results/verification/autonomy'/(datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-dispatch-journey-'+uuid.uuid4().hex[:8]);folder.mkdir(parents=True)
    script=folder/'input.tsv';script.write_text('click\ttarget\nsnapshot\ttarget\nclick\tdeep\nsnapshot\tguarded\nclick\trecover\nsnapshot\trecovered\n',encoding='utf-8');out=folder/'journey'
    before=capture_tests.source_identity(ROOT);exe=ROOT/'ChezCat.exe';data=exe.read_bytes();pe=struct.unpack_from('<I',data,60)[0];reserve,commit=struct.unpack_from('<QQ',data,pe+24+72)
    row=capture_tests.execute([str(exe),str(ROOT/'tools/fixtures/dispatch-stack.html'),'--dev-input-script',str(script),'--dev-input-output',str(out),'--javascript','--no-profile'],folder/'command.log',ROOT,120)
    def read(name,suffix):
        file=next(out.glob('*-'+name+'.'+suffix),None);return file.read_text(encoding='utf-8') if file else ''
    guarded=read('guarded','activity.txt');recovered=read('recovered','activity.txt');layout=read('recovered','layout.txt')
    target=read('target','activity.txt');target_layout=read('target','layout.txt')
    checks={'process_survived_nested_guard':row['state']=='completed','measured_native_reserve':reserve==8*1024*1024 and 0<commit<reserve,'guard_reached':'Function call limit reached' in guarded or 'evaluation nesting' in guarded,'continued_real_input_recovered':'Page title: Nested dispatch recovered' in recovered,'real_recovered_text_rendered':all(('text="'+word+'"') in layout for word in ['Nested','dispatch','recovered']),'paired_pixels_written':'Stage paired_ppm_written=1' in guarded and 'Stage paired_ppm_written=1' in recovered,'source_unchanged':before==capture_tests.source_identity(ROOT)}
    checks.update({'standalone_target_real_dispatch':'Page title: Standalone EventTarget delivered' in target,'standalone_target_text_rendered':all(('text="'+word+'"') in target_layout for word in ['Standalone','EventTarget','delivered']),'standalone_target_pixels_written':'Stage paired_ppm_written=1' in target})
    capture_tests.write_json(folder/'manifest.json',{'scope':__doc__,'source_before':before,'source_after':capture_tests.source_identity(ROOT),'exe_sha256':capture_tests.sha256(exe),'stack_reserve_bytes':reserve,'stack_commit_bytes':commit,'run':row,'checks':checks});print(json.dumps(checks,indent=2));print('Evidence: '+str(folder));return 0 if all(checks.values()) else 1
if __name__=='__main__':raise SystemExit(main())
