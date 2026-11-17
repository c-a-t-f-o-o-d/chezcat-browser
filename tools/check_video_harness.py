"""Offline real-container video playback through native App controls and renderer.

No desktop input, external decoder, network service or HTMLMediaElement claim.
"""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import uuid
import capture_tests

ROOT=Path(__file__).resolve().parents[1]

def main():
    folder=capture_tests.verification_root(ROOT) / 'autonomy'/(datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-local-video-journey-'+uuid.uuid4().hex[:8]);folder.mkdir(parents=True)
    script=folder/'video.tsv'
    script.write_text('snapshot\tposter\nmedia\tPlay\nwait\t450\nmedia\tPause\nsnapshot\tpaused\nwait\t600\nsnapshot\tfrozen\nmedia\tSeek:2200\nsnapshot\tseeked\nmedia\tPlay\nwait\t1200\nsnapshot\tended\nmedia\tPlay\nsnapshot\treplay\nmedia\tStop\nsnapshot\tstopped\naddress\t'+str(ROOT/'pages/second.html')+'\nkey\tEnter\nsnapshot\tnavigated\n',encoding='utf-8')
    before=capture_tests.source_identity(ROOT);out=folder/'journey'
    row=capture_tests.execute([str(ROOT/'ChezCat.exe'),str(ROOT/'tools/fixtures/scene-intra.ivf'),'--dev-input-script',str(script),'--dev-input-output',str(out),'--no-profile'],folder/'command.log',ROOT,120)
    checks={'process_completed':row['state']=='completed'};stages={}
    reference=(ROOT/'tools/fixtures/scene-intra.rgba').read_bytes()
    for name in ['poster','paused','frozen','seeked','ended','replay','stopped','navigated']:
        file=next(out.glob('*-'+name+'.activity.txt'),None)
        if file is None:checks[name+'_snapshot']=False;continue
        activity=file.read_text(encoding='utf-8');match=re.search(r'Media playing=(\w+) ended=(\w+) time_ms=(\d+) duration_ms=(\d+) frame=(\d+) frames=(\d+) pending=(.*?) canvas=(\d+)x(\d+) generation=(\d+) error=(.*)',activity)
        if name=='navigated':checks['navigation_cleared_video']='Local video: none' in activity;continue
        if not match:checks[name+'_media_state']=False;continue
        values=match.groups();state={'playing':values[0]=='true','ended':values[1]=='true','time_ms':int(values[2]),'duration_ms':int(values[3]),'frame':int(values[4]),'pending':values[6],'error':values[10]}
        state['events']=re.search(r'Media lifecycle dropped=\d+ (.*)',activity).group(1).split(',')
        rect=re.search(r'Media rendered rect=Some\(\((\d+), (\d+), (\d+), (\d+)\)\)',activity)
        if not rect:checks[name+'_visible_frame']=False;stages[name]=state;continue
        x,y,w,h=map(int,rect.groups());ppm=file.with_name(file.name.replace('.activity.txt','.ppm')).read_bytes();header=ppm.split(b'\n',3);width,height=map(int,header[1].split());pixels=header[3]
        checks[name+'_visible_frame']=w==64 and h==48 and x+w<=width and y+h<=height and len(pixels)==width*height*3
        crop=b''.join(pixels[((y+dy)*width+x)*3:((y+dy)*width+x+w)*3] for dy in range(h));state['frame_rgb_sha256']=hashlib.sha256(crop).hexdigest()
        expected=reference[state['frame']*64*48*4:(state['frame']+1)*64*48*4];expected_rgb=bytes(channel for index,channel in enumerate(expected) if index%4!=3)
        mean=sum(abs(a-b) for a,b in zip(crop,expected_rgb))/len(expected_rgb);state['reference_mean_rgb_error']=mean
        checks[name+'_actual_rendered_decoded_pixels']=len(crop)==len(expected_rgb) and mean<4
        checks[name+'_no_decode_error']=state['error']=='None' and state['pending']=='None';stages[name]=state
    if all(name in stages for name in ['poster','paused','frozen','seeked','ended','replay','stopped']):
        p,a,b,s,e,r,t=(stages[name] for name in ['poster','paused','frozen','seeked','ended','replay','stopped'])
        checks.update(initial_paused=p['frame']==0 and not p['playing'] and p['time_ms']==0,
            real_playback_advanced=not a['playing'] and 0<a['frame']<5,
            pause_preserved_time_and_pixels=not b['playing'] and a['time_ms']==b['time_ms'] and a['frame_rgb_sha256']==b['frame_rgb_sha256'],
            seek_real_frame=s['frame']==4 and 2150<s['time_ms']<2250 and not s['playing'],
            ended_on_real_last_frame=e['ended'] and not e['playing'] and e['frame']==5 and e['time_ms']==3000 and e['events'].count('ended')==1,
            replay_restarted=r['playing'] and r['time_ms']<2000 and r['frame']<5,
            stop_restored_original_pixels=t['time_ms']==0 and not t['playing'] and not t['ended'] and t['frame_rgb_sha256']==p['frame_rgb_sha256'],
            distinct_actual_frames=len({p['frame_rgb_sha256'],s['frame_rgb_sha256'],e['frame_rgb_sha256']})==3)
    checks['source_unchanged']=before==capture_tests.source_identity(ROOT)
    capture_tests.write_json(folder/'manifest.json',{'scope':__doc__,'executable_sha256':capture_tests.sha256(ROOT/'ChezCat.exe'),'source_before':before,'source_after':capture_tests.source_identity(ROOT),'run':row,'checks':checks,'stages':stages})
    print(json.dumps(checks,indent=2));print('Evidence: '+str(folder));return 0 if all(checks.values()) else 1

if __name__=='__main__':raise SystemExit(main())
