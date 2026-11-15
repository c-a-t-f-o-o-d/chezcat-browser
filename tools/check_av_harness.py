"""Real local Matroska A/V through App controls and paired renderer pixels.

Default is offline metadata/seek/EOF-selection/Stop/navigation, no device.
--device explicitly tests local WinMM sample-clock synchronization and drain.
Neither mode proves physical audibility or native desktop/locked presentation.
"""
import argparse,hashlib,json,re,uuid
from datetime import datetime,timezone
from pathlib import Path
import capture_tests
ROOT=Path(__file__).resolve().parents[1]
def frame_at(clip, milliseconds):
    """Independent timestamp selection; never use the player's frame report."""
    return max(index for index, frame in enumerate(clip['frames'])
        if frame['timestamp'] * clip['scale'] * 1000 <= milliseconds * clip['rate'])


def fixture_reference(stem, display_profile=None):
    stem = Path(stem)
    clip = json.loads(stem.with_suffix('.json').read_text(encoding='utf-8'))
    original = stem.with_suffix('.rgba').read_bytes()
    if hashlib.sha256(original).hexdigest() != clip['reference_rgba_sha256']:
        raise ValueError('Original independent reference identity mismatch.')
    if display_profile is None:
        return clip, original, clip['reference_rgba_sha256'], None
    profile_path = Path(display_profile)
    profile = json.loads(profile_path.read_text(encoding='utf-8'))
    if (profile['retained_default_rgba_sha256'] != clip['reference_rgba_sha256']
        or profile['input_yuv_sha256'] != capture_tests.sha256(stem.with_suffix('.yuv'))
        or Path(profile['reference_rgba']).name != profile['reference_rgba']):
        raise ValueError('Explicit presentation profile/input identity mismatch.')
    reference = (stem.parent / profile['reference_rgba']).read_bytes()
    if hashlib.sha256(reference).hexdigest() != profile['reference_rgba_sha256']:
        raise ValueError('Explicit independent display reference identity mismatch.')
    return clip, reference, profile['reference_rgba_sha256'], profile


def main(fixture_stem=None, video_fixture_stem=None, *, device=None, display_profile=None):
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--device',action='store_true');args=parser.parse_args([] if device is not None else None)
    if device is not None:args.device=device
    fixture_stem=Path(fixture_stem) if fixture_stem is not None else ROOT/'tools/fixtures/scene-av'
    video_fixture_stem=Path(video_fixture_stem) if video_fixture_stem is not None else ROOT/'tools/fixtures/scene-intra'
    clip,reference,reference_sha256,profile=fixture_reference(video_fixture_stem,display_profile)
    last_frame=len(clip['frames'])-1;native_seek_frame=frame_at(clip,2200)
    folder=ROOT/'results/verification/autonomy'/(datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-av-'+('device' if args.device else 'metadata')+'-journey-'+uuid.uuid4().hex[:8]);folder.mkdir(parents=True)
    actions='snapshot\tready\n'
    if args.device:actions+='media\tPlay\nwait\t350\nmedia\tPause\nsnapshot\tpaused\nwait\t400\nsnapshot\tfrozen\n'
    actions+='media\tSeek:2200\nsnapshot\tseeked\n'
    if args.device:actions+='media\tPlay\nwait\t1200\nsnapshot\tended\nmedia\tPlay\nsnapshot\treplay\n'
    else:actions+='media\tSeek:3000\nsnapshot\teof-selected\n'
    actions+='media\tStop\nsnapshot\tstopped\naddress\t'+str(ROOT/'pages/second.html')+'\nkey\tEnter\nwait\t150\nsnapshot\tnavigated\n'
    script=folder/'input.tsv';script.write_bytes(actions.encode('utf-8'));out=folder/'journey';before=capture_tests.source_identity(ROOT)
    run=capture_tests.execute([str(ROOT/'ChezCat.exe'),str(fixture_stem.with_suffix('.mkv')),'--dev-input-script',str(script),'--dev-input-output',str(out),'--no-profile'],folder/'command.log',ROOT,120)
    checks={'process_completed':run['state']=='completed'};stages={}
    names=['ready','paused','frozen','seeked','ended','replay','stopped','navigated'] if args.device else ['ready','seeked','eof-selected','stopped','navigated']
    for name in names:
        file=next(out.glob('*-'+name+'.activity.txt'),None)
        if file is None:checks[name+'_snapshot']=False;continue
        text=file.read_text(encoding='utf-8');ppm=file.with_name(file.name.replace('.activity.txt','.ppm'))
        checks[name+'_paired_pixels']='Stage paired_ppm_written=1' in text and ppm.is_file()
        owner=re.search(r'Audio owner opened=(\w+) retained_buffers=(\d+) busy=(\w+)',text)
        if name=='navigated':checks['navigation_cleared_both_players']='Local audio: none' in text and 'Local video: none' in text;checks['navigation_closed_device_buffers']=bool(owner) and owner.groups()==('false','0','false');continue
        v=re.search(r'Media playing=(\w+) ended=(\w+) time_ms=(\d+) duration_ms=(\d+) frame=(\d+) frames=(\d+) pending=(.*?) canvas=(\d+)x(\d+) generation=(\d+) error=(.*)',text)
        a=re.search(r'Audio playing=(\w+) ended=(\w+) pending=(\w+) time_ms=(\d+) duration_ms=(\d+) frame=(\d+) frames=(\d+) generation=(\d+) error=(.*)',text)
        d=re.search(r'Audio device opened=(\w+) queued=(\d+) submitted=(\d+) retired=(\d+) position_unit=(.*?) retained_buffers=(\d+)',text)
        rect=re.search(r'Media rendered rect=Some\(\((\d+), (\d+), (\d+), (\d+)\)\)',text)
        if not(v and a and d and rect):checks[name+'_states']=False;continue
        vv,aa,dd=v.groups(),a.groups(),d.groups()
        state={'playing':vv[0]=='true','ended':vv[1]=='true','time_ms':int(vv[2]),'video_frame':int(vv[4]),'video_pending':vv[6],'video_error':vv[10],'audio_playing':aa[0]=='true','audio_pending':aa[2]=='true','audio_time_ms':int(aa[3]),'audio_frame':int(aa[5]),'audio_frames':int(aa[6]),'audio_error':aa[8],'opened':dd[0]=='true','queued':int(dd[1]),'submitted':int(dd[2]),'retired':int(dd[3]),'unit':dd[4]}
        checks[name+'_shared_sample_position']=state['time_ms']==state['audio_time_ms'] and state['playing']==state['audio_playing']
        checks[name+'_no_errors']=state['audio_error']=='None' and state['video_error']=='None'
        checks[name+'_frame_matches_pcm_time']=state['video_frame']==frame_at(clip,state['time_ms'])
        if not args.device:checks[name+'_no_device_opened']=not state['opened'] and bool(owner) and owner.groups()==('false','0','false')
        if name not in ['replay']:checks[name+'_completed_controls']=state['video_pending']=='None' and not state['audio_pending']
        x,y,w,h=map(int,rect.groups());header=ppm.read_bytes().split(b'\n',3);width,height=map(int,header[1].split());pixels=header[3]
        checks[name+'_visible_frame']=w==64 and h==48 and x+w<=width and y+h<=height and len(pixels)==width*height*3
        crop=b''.join(pixels[((y+row)*width+x)*3:((y+row)*width+x+w)*3] for row in range(h));rgba=reference[state['video_frame']*64*48*4:(state['video_frame']+1)*64*48*4];rgb=bytes(c for i,c in enumerate(rgba) if i%4!=3)
        state['frame_rgb_sha256']=hashlib.sha256(crop).hexdigest();state['reference_mean_rgb_error']=sum(abs(a-b) for a,b in zip(crop,rgb))/len(rgb)
        checks[name+'_real_decoded_pixels']=len(crop)==len(rgb) and state['reference_mean_rgb_error']<4
        if profile is not None:
            checks[name+'_explicit_display_profile_pixels']=(len(crop)==len(rgb)
                and state['reference_mean_rgb_error']<profile['pixel_tolerance']['mean_rgb_error_less_than']
                and max((abs(a-b) for a,b in zip(crop,rgb)),default=256)<=profile['pixel_tolerance']['maximum_channel_error_at_most'])
        stages[name]=state
    if 'seeked' in stages:checks['seek_decoded_frame_four']=stages['seeked']['video_frame']==native_seek_frame and 2160<stages['seeked']['time_ms']<2240 and not stages['seeked']['playing']
    if 'ready' in stages:checks['initial_paused']=not stages['ready']['playing'] and not stages['ready']['opened'] and stages['ready']['time_ms']==0
    if 'eof-selected' in stages:checks['metadata_eof_is_not_completed_drain']=not stages['eof-selected']['ended'] and stages['eof-selected']['time_ms']==3000 and stages['eof-selected']['video_frame']==last_frame
    if 'stopped' in stages:checks['stop_cleared_both_timelines']=not stages['stopped']['playing'] and not stages['stopped']['ended'] and stages['stopped']['time_ms']==0 and stages['stopped']['audio_frame']==0 and stages['stopped']['queued']==0
    if args.device and all(n in stages for n in ['paused','frozen','ended','replay']):
        p,f,e,r=(stages[n] for n in ['paused','frozen','ended','replay'])
        checks.update(real_device_sample_clock=p['opened'] and p['unit'] in ['Some(1)','Some(2)','Some(4)'] and p['audio_frame']>0 and p['video_frame']>0,pause_preserved_time_and_pixels=not p['playing'] and not f['playing'] and p['audio_frame']==f['audio_frame'] and p['frame_rgb_sha256']==f['frame_rgb_sha256'],confirmed_shared_end=e['ended'] and not e['playing'] and e['audio_frame']==e['audio_frames'] and e['queued']==0 and e['submitted']>0 and e['retired']==e['submitted'] and e['video_frame']==last_frame,replay_restarted=r['playing'] and r['submitted']>0 and r['audio_frame']<r['audio_frames'] and r['time_ms']<2000)
    checks['source_unchanged']=before==capture_tests.source_identity(ROOT)
    capture_tests.write_json(folder/'manifest.json',{'scope':__doc__,'device_attempt':args.device,'physical_audibility_verified':False,'executable_sha256':capture_tests.sha256(ROOT/'ChezCat.exe'),'source_before':before,'source_after':capture_tests.source_identity(ROOT),'run':run,'checks':checks,'stages':stages});print(json.dumps(checks,indent=2));print('Evidence: '+str(folder));return 0 if all(checks.values()) else 1
if __name__=='__main__':raise SystemExit(main())
