"""Original local PCM viewer through real App controls and renderer.

Default: metadata/seek/Stop/navigation with NO device opened (offline CI).
--device: coordinated local real WinMM playback/clock/drain; no audibility or A/V
synchronization claim. No external decoder, public request or desktop input.
"""
import argparse
from datetime import datetime,timezone
import json
from pathlib import Path
import re
import uuid
import capture_tests

ROOT=Path(__file__).resolve().parents[1]
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--device',action='store_true');args=parser.parse_args()
    folder=ROOT/'results/verification/autonomy'/(datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-pcm-'+('device' if args.device else 'metadata')+'-journey-'+uuid.uuid4().hex[:8]);folder.mkdir(parents=True)
    actions='snapshot\tready\n'
    if args.device:actions+='media\tPlay\nwait\t350\nmedia\tPause\nsnapshot\tpaused\nwait\t400\nsnapshot\tfrozen\nmedia\tSeek:1300\nsnapshot\tseeked\nmedia\tPlay\nwait\t1000\nsnapshot\tended\nmedia\tPlay\nsnapshot\treplay\n'
    else:actions+='media\tSeek:750\nsnapshot\tseeked\n'
    actions+='media\tStop\nsnapshot\tstopped\naddress\t'+str(ROOT/'pages/second.html')+'\nkey\tEnter\nwait\t150\nsnapshot\tnavigated\n'
    script=folder/'input.tsv';script.write_bytes(actions.encode('utf-8'));out=folder/'journey';before=capture_tests.source_identity(ROOT)
    row=capture_tests.execute([str(ROOT/'ChezCat.exe'),str(ROOT/'tools/fixtures/original-tone.wav'),'--dev-input-script',str(script),'--dev-input-output',str(out),'--no-profile'],folder/'command.log',ROOT,120)
    checks={'process_completed':row['state']=='completed'};stages={}
    names=['ready','paused','frozen','seeked','ended','replay','stopped','navigated'] if args.device else ['ready','seeked','stopped','navigated']
    for name in names:
        file=next(out.glob('*-'+name+'.activity.txt'),None)
        if file is None:checks[name+'_snapshot']=False;continue
        text=file.read_text(encoding='utf-8');owner=re.search(r'Audio owner opened=(\w+) retained_buffers=(\d+) busy=(\w+)',text)
        checks[name+'_paired_pixels']='Stage paired_ppm_written=1' in text and file.with_name(file.name.replace('.activity.txt','.ppm')).is_file()
        if name=='navigated':checks['navigation_cleared_player']='Local audio: none' in text;checks['navigation_closed_device_and_buffers']=bool(owner) and owner.groups()==('false','0','false');continue
        state=re.search(r'Audio playing=(\w+) ended=(\w+) pending=(\w+) time_ms=(\d+) duration_ms=(\d+) frame=(\d+) frames=(\d+) generation=(\d+) error=(.*)',text)
        device=re.search(r'Audio device opened=(\w+) queued=(\d+) submitted=(\d+) retired=(\d+) position_unit=(.*?) retained_buffers=(\d+)',text)
        if not state or not device:checks[name+'_state']=False;continue
        values=state.groups();native=device.groups();record={'playing':values[0]=='true','ended':values[1]=='true','pending':values[2]=='true','time_ms':int(values[3]),'duration_ms':int(values[4]),'frame':int(values[5]),'frames':int(values[6]),'error':values[8],'opened':native[0]=='true','queued':int(native[1]),'submitted':int(native[2]),'retired':int(native[3]),'unit':native[4]}
        checks[name+'_completed_without_error']=not record['pending'] and record['error']=='None';stages[name]=record
        if not args.device:checks[name+'_no_device_opened']=not record['opened'] and bool(owner) and owner.groups()==('false','0','false')
    if 'ready' in stages:checks['initial_paused_at_zero']=not stages['ready']['playing'] and not stages['ready']['opened'] and stages['ready']['frame']==0
    if 'seeked' in stages:checks['actual_seek_selected_frames']=not stages['seeked']['playing'] and (1260<stages['seeked']['time_ms']<1340 if args.device else 710<stages['seeked']['time_ms']<790)
    if 'stopped' in stages:checks['stop_rewound_without_queued_audio']=not stages['stopped']['playing'] and stages['stopped']['frame']==0 and stages['stopped']['queued']==0
    if args.device and all(name in stages for name in ['paused','frozen','ended','replay']):
        p,f,e,r=(stages[name] for name in ['paused','frozen','ended','replay'])
        checks.update(real_device_opened=p['opened'] and p['unit'] in ['Some(1)','Some(2)','Some(4)'],real_sample_clock_advanced=0<p['frame']<p['frames'] and p['submitted']>p['frame'],pause_preserved_sample_position=not p['playing'] and not f['playing'] and p['frame']==f['frame'] and p['time_ms']==f['time_ms'],real_device_drained_end=e['ended'] and not e['playing'] and e['frame']==e['frames'] and e['submitted']>0 and e['retired']==e['submitted'] and e['queued']==0,replay_restarted_with_real_submission=r['playing'] and 0<r['frame']<r['frames'] and r['submitted']>0)
    checks['source_unchanged']=before==capture_tests.source_identity(ROOT)
    capture_tests.write_json(folder/'manifest.json',{'scope':__doc__,'device_attempt':args.device,'physical_audibility_verified':False,'synchronized_av_verified':False,'exe_sha256':capture_tests.sha256(ROOT/'ChezCat.exe'),'source_before':before,'source_after':capture_tests.source_identity(ROOT),'run':row,'checks':checks,'stages':stages});print(json.dumps(checks,indent=2));print('Evidence: '+str(folder));return 0 if all(checks.values()) else 1
if __name__=='__main__':raise SystemExit(main())
