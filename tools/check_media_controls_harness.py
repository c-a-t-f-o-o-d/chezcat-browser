"""Actual owned App/input controls reflection, bar pixels, hits and key focus.

One original IVF clip over owned loopback HTTP. No device, desktop input, live
service, automatic acceptance or decoder replacement. Existing media journeys
remain separate regressions. Uses original independent decoded-frame references.
"""
from datetime import datetime, timezone
import argparse
import hashlib
import json
from pathlib import Path
import re
import uuid
import accept_license
import capture_tests
import check_authored_video_harness as authored
import check_network_media_harness as network

ROOT=Path(__file__).resolve().parents[1]
LABELS=['metadata','hidden-playing','hidden-advanced','hidden-paused','hidden-probed',
    'shown','shown-unfocused','restored-paused','restored-frozen']
VISIBLE={'metadata','shown','shown-unfocused','restored-paused','restored-frozen'}
# Pair moving evidence with its pause before output I/O. The restored-play
# interval also includes 250 ms settling after Play and after wait: 750 ms
# exceeds a 500 ms original frame without exhausting the three-second clip.
SCRIPT='snapshot\tmetadata\nclick\tplay-hide\nsnapshot\thidden-playing\nwait\t700\nsnapshot-click\thidden-advanced\tpause\nsnapshot\thidden-paused\nmedia-probe\thidden\nmedia-surface\nkey\tSpace\nsnapshot\thidden-probed\nclick\tshow\nsnapshot\tshown\nmedia-probe\tvisible\nmedia-surface\nkey\tSpace\nsnapshot\tshown-unfocused\nmedia\tPlay\nwait\t250\nkey\tSpace\nsnapshot\trestored-paused\nwait\t350\nsnapshot\trestored-frozen\n'

def require(condition,message):
    if not condition:raise ValueError(message)

def stage(folder,label,clip,reference):
    paths=list(folder.glob('*-'+label+'.activity.txt'))
    require(len(paths)==1,'Expected one actual controls stage: '+label)
    file=paths[0];stem=file.name.removesuffix('.activity.txt')
    files={suffix:file.with_name(stem+'.'+suffix) for suffix in ['activity.txt','layout.txt','source.html','ppm']}
    text=file.read_text(encoding='utf-8');layout=files['layout.txt'].read_text(encoding='utf-8')
    source=files['source.html'].read_text(encoding='utf-8')
    visible=' '.join(re.findall(r'text="([^"\n]*)"',layout))
    markers=dict(re.findall(r'\b(controls_[a-z_]+)=([^\s]+)',visible))
    def fields(prefix):
        line=next((line for line in text.splitlines() if line.startswith(prefix)),'')
        return dict(re.findall(r'(\w+)=([^\s]+)',line))
    document,geometry,canvas=(fields(prefix) for prefix in ['Stage document=','Stage layout=','Stage canvas='])
    media=re.search(r'Media playing=(\w+) ended=(\w+) time_ms=(\d+) duration_ms=(\d+) frame=(\d+) frames=(\d+) pending=(.*?) canvas=(\d+)x(\d+) generation=(\d+) error=(.*)',text)
    binding=re.search(r'HTML video binding node=(\d+) ready_state=(\d+) network_state=(\d+) width=(\d+) height=(\d+)',text)
    rect=re.search(r'Media rendered rect=Some\(\((-?\d+), (-?\d+), (\d+), (\d+)\)\)',text)
    require(media is not None and binding is not None and rect is not None,'Actual bound player/frame/rect missing: '+label)
    state={'playing':media[1]=='true','ended':media[2]=='true','time_ms':int(media[3]),'duration_ms':int(media[4]),
        'frame':int(media[5]),'frames':int(media[6]),'pending':media[7],'generation':int(media[10]),'error':media[11],
        'node':int(binding[1]),'markers':markers,'document':document,'geometry':geometry,'canvas':canvas,'visible':visible}
    raw=files['ppm'].read_bytes().split(b'\n',3)
    require(len(raw)==4 and raw[0]==b'P6' and raw[2]==b'255','Actual paired P6 image required.')
    width,height=map(int,raw[1].split());pixels=raw[3]
    require(width>0 and height>0 and len(pixels)==width*height*3,'Complete actual P6 pixels required.')
    x,y,w,h=map(int,rect.groups())
    require(w==400 and h==300 and x>=0 and y>=0 and x+w<=width and y+h<=height,'Authored image must be inside the actual canvas.')
    require(0<=state['frame']<len(clip['frames']),'Actual decoded frame outside original reference.')
    def rgb(px,py):
        offset=(py*width+px)*3;return pixels[offset:offset+3]
    def error_and_hash(rows):
        samples=bytearray();expected=bytearray()
        for sy in rows:
            for sx in range(64):
                samples.extend(rgb(x+int((sx+.5)*w/64),y+int((sy+.5)*h/48)))
                offset=((state['frame']*48+sy)*64+sx)*4;expected.extend(reference[offset:offset+3])
        require(len(samples)==len(expected) and len(expected)>0,'Complete independent pixel samples required.')
        return sum(abs(a-b) for a,b in zip(samples,expected))/len(expected),hashlib.sha256(samples).hexdigest()
    top_error,top_hash=error_and_hash(range(41));bottom_error,bottom_hash=error_and_hash(range(41,48))
    bar_points=[rgb(x+2,y+h-42),rgb(x+2,y+h-2),rgb(x+w-2,y+h-2),rgb(x+w//2,y+h-2)]
    bar_painted=all(value==bytes([22,45,61]) for value in bar_points)
    paired=('Stage paired_ppm_written=1' in text and document.get('document','unknown')!='unknown'
        and canvas.get('document')==document.get('document')==geometry.get('layout_document')
        and canvas.get('dom_commit')==document.get('dom_commit')==geometry.get('layout_dom')
        and canvas.get('layout')==geometry.get('layout')!='unknown' and canvas.get('page_view')=='1'
        and canvas.get('layout_dom')==geometry.get('layout_dom') and canvas.get('layout_document')==document.get('document'))
    summaries=re.findall(r'^(\d+) script\(s\), (\d+) active listener\(s\), (\d+) error\(s\)\.$',text,re.M)
    checks={'paired_document_dom_layout_pixels':paired,'current_zero_interpreter_errors':len(summaries)==1 and int(summaries[0][0])>=1 and summaries[0][2]=='0',
        'no_audio_owner':'Audio owner opened=false retained_buffers=0 busy=false' in text,
        'real_metadata':binding.groups()[1:]==('4','1','64','48') and state['duration_ms']==3000 and state['frames']==len(clip['frames']),
        'no_decode_error':state['error']=='None','independent_selected_frame_pixels':top_error<6,
        'actual_bar_pixels' if label in VISIBLE else 'actual_exposed_bottom_pixels':bar_painted if label in VISIBLE else not bar_painted and bottom_error<6,
        'actual_original_controls_source':all(value in source for value in ['setControls.call(video,object)','setControls.call(video,false)','video.controls=true',"Object.defineProperty(video,'controls'",'promise.then(function(value)',"sourceAttr=video.getAttributeNode('src')"]),
        'source_metadata_not_reloaded':markers.get('controls_metadata')=='1'}
    expected_frame=authored.frame_at(clip,state['time_ms']);state['independent_frame_at_time']=expected_frame
    checks['independent_frame_timeline']=(0<=expected_frame-state['frame']<=1) if state['playing'] else state['frame']==expected_frame
    checks['separate_visibility_phase_markers']=markers.get('controls_hidden')==('0' if label=='metadata' else '1') and markers.get('controls_shown')==('1' if label in VISIBLE and label!='metadata' else '0')
    if label!='metadata':
        checks['canonical_boolean_shadow_and_real_promise']=all(markers.get(name)==value for name,value in {
            'controls_hidden':'1','controls_boolean':'1','controls_coercions':'0','controls_shadow_reads':'2',
            'controls_shadow_writes':'0','controls_play_sync':'1','controls_play_fulfilled':'1','controls_play_rejected':'0',
            'controls_playing_events':'1','controls_order':'sync,playing,then'}.items())
    state.update(top_error=top_error,bottom_error=bottom_error,top_sha256=top_hash,bottom_sha256=bottom_hash,bar_painted=bar_painted,
        files={f.name:{'sha256':capture_tests.sha256(f),'bytes':f.stat().st_size} for f in files.values()})
    return checks,state

def app_case(exe,folder,source,clip,reference):
    folder.mkdir();before=capture_tests.source_identity(ROOT);checks={'complete_controls_case':False};stages={}
    record={'scope':__doc__,'source':source,'trust_file_sha256':None,'source_before':before,'source_after':before,
        'executable_sha256':capture_tests.sha256(exe),'checks':checks,'stages':stages,'license_accepted_by_driver':False}
    try:
        script=folder/'input.tsv';script.write_text(SCRIPT,encoding='utf-8',newline='\n');output=folder/'journey'
        run=capture_tests.execute([str(exe),source,'--dev-input-script',str(script),'--dev-input-output',str(output),'--javascript','--no-profile'],folder/'command.log',ROOT,120)
        record['run']=run;checks['process_completed']=run['state']=='completed' and run['exit_code']==0
        for label in LABELS:
            part,value=stage(output,label,clip,reference);checks.update({label+':'+name:passed for name,passed in part.items()});stages[label]=value
        checks['same_actual_document_and_player_node']=len({value['document']['document'] for value in stages.values()})==1 and len({value['node'] for value in stages.values()})==1
        checks['hidden_real_clock_and_decoded_output_advance']=all(stages[name]['playing'] and not stages[name]['ended'] for name in ['hidden-playing','hidden-advanced']) and 0<stages['hidden-playing']['time_ms']<stages['hidden-advanced']['time_ms']<3000 and stages['hidden-advanced']['frame']>stages['hidden-playing']['frame'] and stages['hidden-advanced']['top_sha256']!=stages['hidden-playing']['top_sha256']
        held=[stages[name] for name in ['hidden-paused','hidden-probed','shown','shown-unfocused']]
        checks['hide_show_hit_and_unfocused_key_preserve_frozen_player']=all(not value['playing'] and not value['ended'] and value['pending']=='None' for value in held) and len({(value['time_ms'],value['frame'],value['generation'],value['top_sha256']) for value in held})==1
        checks['restored_focused_native_key_pauses_and_freezes']=all(not stages[name]['playing'] and not stages[name]['ended'] and stages[name]['pending']=='None' for name in ['restored-paused','restored-frozen']) and stages['restored-paused']['time_ms']==stages['restored-frozen']['time_ms']>held[0]['time_ms'] and stages['restored-paused']['frame']==stages['restored-frozen']['frame']>held[0]['frame'] and stages['restored-paused']['top_sha256']==stages['restored-frozen']['top_sha256']!=held[0]['top_sha256'] and stages['restored-paused']['generation']==stages['restored-frozen']['generation']
        checks['visible_then_hidden_then_restored_snapshots']=stages['metadata']['bar_painted'] and all(not stages[name]['bar_painted'] for name in ['hidden-playing','hidden-advanced','hidden-paused','hidden-probed']) and all(stages[name]['bar_painted'] for name in ['shown','shown-unfocused','restored-paused','restored-frozen'])
        log=(output/'input.log').read_text(encoding='utf-8')
        checks['actual_native_hit_queries_and_page_surface_inputs']='Media native hit probe expected=hidden hit=None' in log and 'Media native hit probe expected=visible hit=Some(Toggle)' in log and log.count('Pointer native video surface')==2 and log.count('key\tSpace')==3 and 'Pointer native media control=Play' in log
        checks['complete_controls_case']=True
    except (OSError,ValueError,KeyError,TypeError,RuntimeError) as error:
        record['error_type']=type(error).__name__;record['error']=str(error)
    record['source_after']=capture_tests.source_identity(ROOT);checks['source_unchanged']=before==record['source_after']
    checks['executable_unchanged']=record['executable_sha256']==capture_tests.sha256(exe)
    record['passed']=bool(checks) and all(value is True for value in checks.values())
    capture_tests.write_json(folder/'manifest.json',record);print('Evidence: '+str(folder));return record

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--exe',type=Path);args=parser.parse_args()
    exe=(args.exe or ROOT/'ChezCat.exe').resolve();folder=capture_tests.verification_root(ROOT) / 'runs'/(datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+'-media-controls-journey-'+uuid.uuid4().hex[:8]);folder.mkdir(parents=True)
    checks={'complete_journey':False};record={'scope':__doc__,'checks':checks,'passed':False,'license_accepted_by_driver':False}
    try:
        accept_license.terms();require(accept_license.check(accept_license.directory()),'Existing explicit acceptance required; driver never accepts.')
        clip,reference,_,_=authored.fixture_reference(ROOT/'tools/fixtures/scene-intra')
        checks['original_clip_identity']=capture_tests.sha256(ROOT/'tools/fixtures/scene-intra.ivf')==clip['ivf_sha256'] and len(reference)==len(clip['frames'])*64*48*4
        require(checks['original_clip_identity'],'Original encoded clip/reference identity mismatch.')
        fixture=ROOT/'tools/fixtures/media-controls.html';metadata=json.loads(fixture.with_suffix('.json').read_text(encoding='utf-8'))['files'][fixture.name]
        require(fixture.stat().st_size==metadata['bytes'] and capture_tests.sha256(fixture)==metadata['sha256'],'Original controls fixture identity mismatch.')
        child={}
        def runner(*,source,trust_file):
            nonlocal child
            require(trust_file is None,'Owned plain HTTP needs no trust override.')
            child=app_case(exe,folder/'http'/'app',source,clip,reference)
            return 0 if child['passed'] else 1
        payloads={'/media-controls.html':('text/html; charset=utf-8',fixture.read_bytes()),'/scene-intra.ivf':('video/x-ivf',(ROOT/'tools/fixtures/scene-intra.ivf').read_bytes())}
        result=network.run_case('http',folder/'http',authored_runner=runner,served_payloads=payloads,entry_path='/media-controls.html',route_checks={'controls_page_served':('/media-controls.html',1),'original_video_served':('/scene-intra.ivf',1)},scope=__doc__)
        checks.update({'transport:'+name:passed is True for name,passed in result['checks'].items()})
        checks['one_full_media_fetch_without_reload']=sum(row['target']=='/scene-intra.ivf' for row in result['responses'])==1
        checks['source_and_selected_executable_unchanged']=result['source_before']==result['source_after'] and result['executable_sha256']==capture_tests.sha256(exe)
        checks['complete_journey']=bool(child) and child['passed'] is True
        record.update(source_before=result['source_before'],source_after=result['source_after'],executable_sha256=result['executable_sha256'],child_manifest='http/app/manifest.json',child_manifest_sha256=capture_tests.sha256(folder/'http/app/manifest.json'))
    except (OSError,ValueError,KeyError,TypeError,RuntimeError) as error:
        record['error_type']=type(error).__name__;record['error']=str(error)
    record['passed']=bool(checks) and all(value is True for value in checks.values());capture_tests.write_json(folder/'manifest.json',record)
    print(json.dumps(checks,indent=2));print('Evidence: '+str(folder));return 0 if record['passed'] else 1

if __name__=='__main__':raise SystemExit(main())
