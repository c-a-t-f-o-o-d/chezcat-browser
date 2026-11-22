"""Pinned native HTML data cohort. No browser-WPT or conformance pass is inferred.

Inputs are prepared explicitly outside the source tree; run never downloads.
This adapter reads WPT tree data and html5lib tokenizer JSON, not engine code.
"""
import argparse
from collections import Counter
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import time

import capture_tests

ROOT=Path(__file__).resolve().parents[1]
LOCK=ROOT/'tools/reference/html-foundation/selection.json'
INPUTS=ROOT/'tools/reference/html-foundation/inputs.json'
WITNESSES=ROOT/'tools/reference/html-foundation/witnesses.json'
PROTOCOL='CHEZCAT-NATIVE-PARSER-1'
WORKER='html::foundation_adapter::native_parser_data_adapter'
HTML='http://www.w3.org/1999/xhtml'
SVG='http://www.w3.org/2000/svg'
MATH='http://www.w3.org/1998/Math/MathML'

def digest(data):return hashlib.sha256(data).hexdigest()
def canonical(value):return json.dumps(value,sort_keys=True,ensure_ascii=True,separators=(',',':')).encode()
def write_json(path,value):
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(value,indent=2,ensure_ascii=True)+'\n',encoding='utf-8')

class FixtureError(ValueError):pass

@dataclass
class Case:
    identity: str
    kind: str
    repository: str
    path: str
    ordinal: int
    source: str
    expected: object
    scripting: str='off'
    entry: str='document'
    namespace: str=HTML
    context: str=''
    initial_state: str='Data state'
    last_start_tag: object=None
    errors: object=None
    cohort: str='required'
    requires_scripts: bool=False

    def record(self):
        try:utf8=self.source.encode('utf-8')
        except UnicodeError:utf8=None
        return dict(id=self.identity,kind=self.kind,repository=self.repository,path=self.path,
            ordinal=self.ordinal,input_utf16_sha256=digest(self.source.encode('utf-16-le','surrogatepass')),
            input_utf16_code_units=len(self.source.encode('utf-16-le','surrogatepass'))//2,
            input_utf8_sha256=digest(utf8) if utf8 is not None else None,input_utf8_bytes=len(utf8) if utf8 is not None else None,
            expected_sha256=digest(canonical(self.expected)),scripting=self.scripting,entry=self.entry,
            namespace=self.namespace,context=self.context,initial_state=self.initial_state,
            last_start_tag=self.last_start_tag,errors=self.errors,cohort=self.cohort,
            requires_scripts=self.requires_scripts)

def tree_cases(path,data):
    try:lines=data.decode('utf-8').split('\n')
    except UnicodeError as error:raise FixtureError(f'{path}: invalid UTF-8 fixture') from error
    cursor=0;ordinal=0;cases=[]
    while cursor<len(lines):
        while cursor<len(lines) and lines[cursor]=='':cursor+=1
        if cursor==len(lines):break
        if lines[cursor]=='#description':
            cursor+=1
            while cursor<len(lines) and lines[cursor]!='#data':cursor+=1
        if cursor==len(lines) or lines[cursor]!='#data':raise FixtureError(f'{path}:{cursor+1}: missing #data')
        cursor+=1;start=cursor
        while cursor<len(lines) and lines[cursor]!='#errors':cursor+=1
        if cursor==len(lines):raise FixtureError(f'{path}: missing #errors')
        source='\n'.join(lines[start:cursor]);cursor+=1;errors=[];context=None;scripting=None
        markers={'#new-errors','#document-fragment','#script-on','#script-off','#document'}
        while cursor<len(lines) and lines[cursor] not in markers:
            if not lines[cursor]:raise FixtureError(f'{path}: blank error record')
            errors.append(lines[cursor]);cursor+=1
        if cursor<len(lines) and lines[cursor]=='#new-errors':
            cursor+=1
            while cursor<len(lines) and lines[cursor] not in markers:
                if not lines[cursor]:raise FixtureError(f'{path}: blank new-error record')
                errors.append(lines[cursor]);cursor+=1
        if cursor<len(lines) and lines[cursor]=='#document-fragment':
            cursor+=1
            if cursor>=len(lines):raise FixtureError(f'{path}: missing fragment context')
            context=lines[cursor];cursor+=1
        if cursor<len(lines) and lines[cursor] in {'#script-on','#script-off'}:
            scripting=lines[cursor].removeprefix('#script-');cursor+=1
        if cursor>=len(lines) or lines[cursor]!='#document':raise FixtureError(f'{path}: missing #document')
        cursor+=1;start=cursor
        while cursor<len(lines):
            if lines[cursor]=='':
                end=cursor
                while end<len(lines) and lines[end]=='':end+=1
                if end==len(lines) or lines[end] in {'#data','#description'}:break
            cursor+=1
        expected='\n'.join(lines[start:cursor])
        if cursor>start:expected+='\n'
        ordinal+=1
        namespace=HTML;name=context or ''
        if name.startswith('svg '):namespace=SVG;name=name[4:]
        elif name.startswith('math '):namespace=MATH;name=name[5:]
        if context is not None and not name:raise FixtureError(f'{path}: empty fragment context')
        cohort='supplementary-pending-spec' if PurePosixPath(path).name.startswith('pending-spec-') else 'required'
        for script in [scripting] if scripting else ['off','on']:
            entry='fragment' if context is not None else 'document'
            identity=f'wpt:{path}#{ordinal:04d}:{entry}:{script}:{namespace}:{name}'
            cases.append(Case(identity,'tree','wpt',path,ordinal,source,expected,script,entry,namespace,name,
                errors={'count':len(errors)},cohort=cohort,
                requires_scripts=PurePosixPath(path).name.startswith('scripted_') and script=='on'))
    if not ordinal:raise FixtureError(f'{path}: no tree cases')
    return cases

def unescape(value):
    if isinstance(value,str):return re.sub(r'\\u([0-9a-fA-F]{4})',lambda m:chr(int(m[1],16)),value)
    if isinstance(value,list):return [unescape(v) for v in value]
    if isinstance(value,dict):return {k:unescape(v) for k,v in value.items()}
    return value

def tokenizer_cases(path,data):
    try:root=json.loads(data)
    except (ValueError,UnicodeError) as error:raise FixtureError(f'{path}: malformed tokenizer JSON') from error
    key='xmlViolationTests' if PurePosixPath(path).name=='xmlViolation.test' else 'tests'
    if not isinstance(root,dict) or not isinstance(root.get(key),list):raise FixtureError(f'{path}: missing tokenizer cases')
    cases=[]
    for ordinal,test in enumerate(root[key],1):
        if not isinstance(test.get('input'),str) or not isinstance(test.get('output'),list):raise FixtureError(f'{path}#{ordinal}: invalid tokenizer case')
        source=test['input'];expected=test['output']
        if test.get('doubleEscaped'):source=unescape(source);expected=unescape(expected)
        cohort='supplementary-xml-infoset' if key=='xmlViolationTests' else 'supplementary-pending-spec' if PurePosixPath(path).name=='pendingSpecChanges.test' else 'required'
        states=test.get('initialStates',['Data state'])
        if not isinstance(states,list) or not states or any(not isinstance(s,str) for s in states):raise FixtureError(f'{path}#{ordinal}: invalid initial states')
        for state in states:
            cases.append(Case(f'html5lib-tests:{path}#{ordinal:04d}:{state}','tokenizer','html5lib-tests',path,ordinal,
                source,expected,initial_state=state,last_start_tag=test.get('lastStartTag'),errors=test.get('errors'),cohort=cohort))
    return cases

def checked_inputs(cache,inputs):
    cache=capture_tests.checked_output_path(cache)
    for item in inputs:
        relative=PurePosixPath(item['path'])
        if relative.is_absolute() or '..' in relative.parts:raise FixtureError('Unsafe pinned input path')
        path=cache/item['repository']/relative
        if not path.is_file():raise FixtureError('Missing pinned input: '+item['path'])
        data=path.read_bytes()
        if len(data)!=item['bytes'] or digest(data)!=item['sha256']:raise FixtureError('Pinned input hash mismatch: '+item['path'])
    return cache

def enumerate_cases(cache,inputs):
    cases=[]
    for item in sorted(inputs,key=lambda x:(x['repository'],x['path'])):
        path=item['path']
        if item['repository']=='wpt' and path.startswith('html/syntax/parsing/resources/') and path.endswith('.dat'):
            cases.extend(tree_cases(path,(cache/'wpt'/path).read_bytes()))
        elif item['repository']=='html5lib-tests' and path.startswith('tokenizer/') and path.endswith('.test'):
            cases.extend(tokenizer_cases(path,(cache/'html5lib-tests'/path).read_bytes()))
        elif item['repository']=='wpt' and path.startswith('dom/nodes/') and path.endswith(('.html','.xhtml')):
            cases.append(Case('browser-wpt:'+path+'#all-unchanged-assertions','browser','wpt',path,1,'',item['sha256']))
    originals=json.loads(WITNESSES.read_bytes())
    for index,test in enumerate(originals['cases'],1):
        cases.append(Case('original:'+test['id'],test['kind'],'original','witnesses.json',index,test['input'],test['expected'],
            scripting=test.get('scripting','off'),entry=test.get('entry','document'),context=test.get('context',''),errors=None))
    if len({c.identity for c in cases})!=len(cases):raise FixtureError('Duplicate selected case identity')
    return cases

def freeze(cache,materialized):
    if LOCK.exists() or INPUTS.exists():raise FixtureError('A cohort already exists; do not replace a frozen denominator')
    inputs=json.loads(Path(materialized).read_bytes())['inputs'];cache=checked_inputs(cache,inputs)
    cases=enumerate_cases(cache,inputs);records=[c.record() for c in cases]
    coverage=json.loads((ROOT/'docs/HTML_FOUNDATION_COVERAGE.json').read_bytes())
    inventory={key:coverage[key] for key in ['inventory_counts','parser_states_and_modes','element_families','input_states','encoding_names_and_labels','legacy_seed','custom_unknown_elements','related_programs']}
    selection=dict(schema_version=1,selection_id='html-foundation-f00-20261121-v1',
        rule='All pinned WPT dat cases, both scripting modes when unspecified, all html5lib tokenizer states; pending-spec/XML-infoset cohorts separately supplementary; all original witnesses; fixed F01 browser files with all unchanged assertions',
        record_count=len(cases),native_case_count=sum(c.kind!='browser' for c in cases),
        browser_file_count=sum(c.kind=='browser' for c in cases),browser_subtest_count=None,
        case_catalog_sha256=digest(canonical(records)),counts=dict(Counter(c.kind for c in cases)),
        cohorts=dict(Counter(c.cohort for c in cases)),inventory_sha256=digest(canonical(inventory)),
        input_manifest_sha256=digest(canonical(inputs)),witnesses_sha256=digest(WITNESSES.read_bytes()),
        enumerator_sha256=digest(Path(__file__).read_bytes()),normative_pins=coverage['input_pins'],
        inventory_source_revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        inventory_path='docs/HTML_FOUNDATION_COVERAGE.json',
        browser_subtests='Not enumerated/executed: unchanged browser harness and loaded/API entry dependency owned by F06/F10; every assertion in the pinned file selection remains required.',
        supplementary_not_selected=['html5lib encoding and serializer inputs are pinned but not executed by this initial tokenizer/tree cohort; select case-level cohorts in F02/F07'],
        limitations=['Native tree output is not browser WPT','Parse diagnostics and token-stream entry absence remain required harness blocks, never passes','Scripted parser integration remains owned by F10','Native scalar parser inputs do not establish DOMString UTF-16 conformance'])
    write_json(INPUTS,dict(schema_version=1,inputs=inputs))
    write_json(LOCK,selection)
    print(json.dumps({k:selection[k] for k in ['record_count','native_case_count','browser_file_count','counts','cohorts','case_catalog_sha256','inventory_sha256']}))

def request_text(case):
    values=[PROTOCOL,case.kind,case.entry,case.scripting]
    values.extend(value.encode('utf-8').hex() for value in [case.namespace,case.context,case.source])
    return '\n'.join(values)+'\n'

def execute(exe,case,folder,timeout=5):
    request=folder/(digest(case.identity.encode())+'.req')
    try:request.write_bytes(request_text(case).encode('ascii'))
    except UnicodeError:return dict(outcome='harness_blocked',reason='F01: native scalar parser entry cannot accept unpaired UTF-16 input')
    env=dict(os.environ,CHEZCAT_FOUNDATION_REQUEST=str(request),CHEZCAT_VERIFICATION_ROOT=str(folder))
    try:
        result=subprocess.run([str(exe),WORKER,'--exact','--ignored','--test-threads=1'],env=env,
            stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=timeout)
    except subprocess.TimeoutExpired as error:return dict(outcome='timeout',reason='native worker deadline',raw=(error.stdout or b'').decode('utf-8','replace'))
    except OSError as error:return dict(outcome='harness_blocked',reason='missing/unlaunchable native worker: '+str(error))
    if result.returncode:return dict(outcome='crash',reason='native worker nonzero exit',exit_code=result.returncode,raw=result.stdout.decode('utf-8','replace'))
    if b'1 passed' not in result.stdout or b'running 1 test' not in result.stdout:return dict(outcome='harness_blocked',reason='native adapter test did not execute exactly once')
    path=request.with_suffix('.out')
    if not path.is_file():return dict(outcome='harness_blocked',reason='native result missing')
    try:
        header,status,diagnostics,payload=path.read_text(encoding='ascii').splitlines()
        if header!=PROTOCOL:raise ValueError('protocol')
        value=bytes.fromhex(payload).decode('utf-8')
    except (ValueError,UnicodeError) as error:return dict(outcome='harness_blocked',reason='malformed native response: '+str(error))
    return dict(status=status,value=value,diagnostics=diagnostics)

def classify(case,result):
    if 'outcome' in result:return result
    status=result['status']
    if status in ['fail','harness_blocked']:return dict(outcome=status,reason=result['value'])
    if status not in ['tree','page']:return dict(outcome='harness_blocked',reason='unexpected native result kind')
    match=result['value']==case.expected
    if not match:return dict(outcome='fail',reason='native output mismatch',actual=result['value'],comparison='fail')
    if case.errors is not None and result['diagnostics']!='diagnostics-supported':
        return dict(outcome='harness_blocked',reason='F04: required parse diagnostics unavailable',comparison='pass')
    return dict(outcome='pass',comparison='pass')

def run(cache,exe):
    lock=json.loads(LOCK.read_bytes());inputs=json.loads(INPUTS.read_bytes())['inputs']
    cache=checked_inputs(cache,inputs);cases=enumerate_cases(cache,inputs)
    if digest(canonical([c.record() for c in cases]))!=lock['case_catalog_sha256'] or len(cases)!=lock['record_count']:raise FixtureError('Frozen case denominator changed')
    if digest(canonical(inputs))!=lock['input_manifest_sha256'] or digest(WITNESSES.read_bytes())!=lock['witnesses_sha256']:raise FixtureError('Frozen inputs changed')
    if digest(Path(__file__).read_bytes())!=lock['enumerator_sha256']:raise FixtureError('Frozen adapter changed; review the cohort before measurement')
    source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    if subprocess.check_output(['git','status','--porcelain'],cwd=ROOT):raise FixtureError('Baseline requires a clean committed source')
    exe=Path(exe).resolve()
    if source.encode() not in exe.read_bytes():raise FixtureError('Test executable does not embed the selected source')
    folder=capture_tests.verification_root()/(datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')+'-foundation-f00-'+source[:8])
    folder.mkdir(parents=True);probe=Case('adapter-capabilities','capabilities','original','',0,'','')
    response=execute(exe,probe,folder)
    if response.get('status')!='capabilities':raise FixtureError('Native capability probe failed: '+str(response))
    capabilities=json.loads(response['value']);counts=Counter();projections=Counter();records=[]
    with (folder/'results.jsonl').open('w',encoding='utf-8') as output:
        for index,case in enumerate(cases):
            if case.kind=='browser':result=dict(outcome='harness_blocked',reason='F06/F10: required unchanged browser harness/API entry unavailable; no subtest count claimed')
            elif case.kind=='tokenizer' and not capabilities['tokenizer']:result=dict(outcome='harness_blocked',reason='F04: native token-stream/initial-state/diagnostic entry unavailable')
            elif case.requires_scripts and not capabilities['script_integration']:result=dict(outcome='harness_blocked',reason='F10: required parser script integration unavailable')
            else:result=classify(case,execute(exe,case,folder))
            record=dict(case.record(),**result);output.write(json.dumps(record,ensure_ascii=True)+'\n');output.flush()
            counts[result['outcome']]+=1
            if 'comparison' in result:projections[result['comparison']]+=1
            records.append(record)
            if (index+1)%100==0:print('Recorded',index+1,'of',len(cases),dict(counts),flush=True)
    summary=dict(schema_version=1,source=source,dirty=False,platform=sys.platform,target='x86_64-pc-windows-msvc',
        executable_sha256=capture_tests.sha256(exe),adapter_sha256=digest(Path(__file__).read_bytes()),
        native_adapter_sha256=digest((ROOT/'src/html_foundation_adapter_tests.rs').read_bytes()),
        selection_sha256=digest(LOCK.read_bytes()),case_catalog_sha256=lock['case_catalog_sha256'],
        input_manifest_sha256=lock['input_manifest_sha256'],record_count=len(cases),
        native_case_count=lock['native_case_count'],browser_file_count=lock['browser_file_count'],browser_subtest_count=None,
        native_enumeration_complete=len(records)==lock['record_count'],browser_enumeration_complete=False,
        outcomes=dict(counts),native_output_comparisons=dict(projections),effective_guards=capabilities,
        scope='Native parser/DOM baseline; not unchanged browser WPT, Windows App, package closure or overall conformance',
        closure_blocked=any(r['cohort']=='required' and r['outcome']!='pass' for r in records))
    write_json(folder/'summary.json',summary);print(json.dumps(summary));print('Output',folder)
    return 1 if summary['closure_blocked'] else 0

def main():
    parser=argparse.ArgumentParser(description=__doc__);sub=parser.add_subparsers(dest='command',required=True)
    freeze_args=sub.add_parser('freeze');freeze_args.add_argument('--cache',required=True);freeze_args.add_argument('--materialized',required=True)
    run_args=sub.add_parser('run');run_args.add_argument('--cache',required=True);run_args.add_argument('--exe',required=True)
    args=parser.parse_args()
    try:
        if args.command=='freeze':freeze(args.cache,args.materialized);return 0
        return run(args.cache,args.exe)
    except (OSError,ValueError,subprocess.SubprocessError) as error:
        print('Foundation adapter blocked: '+str(error),file=sys.stderr);return 2
if __name__=='__main__':raise SystemExit(main())
