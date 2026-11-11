#!/usr/bin/env python3
"""Independent finite 0.40 grammar, named-RegExp, Date and object-spread reviews.

Original inputs and native Node references are frozen before implementation
outcomes. Browser-global mode is explicit per fixture and identical on both
engines and the baseline. Early errors and fatal limits are separate assertions,
not matching capability counts. No network requests or downloaded code run.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import subprocess
import sys
import tempfile

sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[1]
HELPER=ROOT/'tools/check_javascript.py'
CASES=ROOT/'tools/review_cases_040.json'
BOUNDS=ROOT/'tools/review_bounds_040.json'
REFERENCE=ROOT/'tools/reference/review-040/original-node-references.json'

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()

def source_manifest(root):
    paths=list(root.glob('src/**/*.rs'))+[root/n for n in ('Cargo.toml','Cargo.lock','build.rs') if (root/n).is_file()]
    files={p.relative_to(root).as_posix():digest(p) for p in sorted(paths)}
    joined=''.join(f'{n}\0{v}\n' for n,v in files.items())
    return {'sha256':hashlib.sha256(joined.encode()).hexdigest(),'files':files}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root',type=Path,default=ROOT)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--label',default='current')
    parser.add_argument('--suite',choices=['grammar-regexp','date','object-spread'],default='grammar-regexp')
    parser.add_argument('--group',choices=['defaults','destructuring','named-regexp','date','object-spread'],action='append')
    parser.add_argument('--allow-gaps',action='store_true')
    parser.add_argument('--rustc',default='rustc')
    parser.add_argument('--node',default='node')
    options=parser.parse_args()
    spec=importlib.util.spec_from_file_location('review_helper_040',HELPER)
    helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
    root=options.source_root.resolve();before=source_manifest(root)
    supplement=options.suite!='grammar-regexp'
    case_file=ROOT/'tools'/f"{options.suite.replace('-','_')}_review_cases_040.json" if supplement else CASES
    bounds_file=case_file if supplement else BOUNDS
    reference_file=ROOT/'tools/reference/review-040'/f'{options.suite}-original-node-references.json' if supplement else REFERENCE
    input_hashes={path:digest(path) for path in [Path(__file__),HELPER,case_file,bounds_file,reference_file]}
    spec_file=ROOT/'tools/reference/review-040/date-current-spec-expectations.json'
    current_spec={}
    if options.suite=='date':
        input_hashes[spec_file]=digest(spec_file)
        spec_data=json.loads(spec_file.read_text(encoding="utf-8"))
        assert spec_data['fixture_sha256']==digest(case_file)
        current_spec={c['id']:c for c in spec_data['cases']}
    cases=json.loads(case_file.read_text(encoding="utf-8"))['cases']
    bounds=json.loads(bounds_file.read_text(encoding="utf-8"))['bounds' if supplement else 'cases']
    if supplement:
        cases=[{**c,'kind':c.get('kind','comparison'),'group':options.suite} for c in cases]
        bounds=[{**c,'group':options.suite} for c in bounds]
    originals={c['id']:c for c in json.loads(reference_file.read_text(encoding="utf-8"))['cases']}
    if options.group:
        cases=[c for c in cases if c['group'] in options.group]
        bounds=[c for c in bounds if c['group'] in options.group]
    if not cases:parser.error('No original comparisons or early-error fixtures selected.')
    results=[];early=[];limits=[]
    with tempfile.TemporaryDirectory(prefix='chezcat-review-040-') as directory:
        directory=Path(directory);probe=directory/'probe';rust=directory/'probe.rs'
        rust.write_text(helper.PROBE.replace('JAVASCRIPT_SOURCE',json.dumps(str(root/'src/javascript.rs'))), encoding="utf-8")
        subprocess.run([options.rustc,'--edition=2021','-O',str(rust),'-o',str(probe)],capture_output=True,text=True,timeout=180,check=True)
        for case in cases:
            source=case['source'];mode=case['browser_global']
            assert hashlib.sha256(source.encode()).hexdigest()==case['source_sha256']
            assert originals[case['id']]['source']==source and originals[case['id']]['browser_global']==mode
            reference=helper.run_node(options.node,source,browser=mode)
            assert reference==originals[case['id']]['reference'],('Reference changed',case['id'])
            actual=helper.run_own(probe,source,browser=mode)
            if case['kind']=='comparison':
                assert reference['ok'] and reference['records'] and reference['value']=='string:6f6b'
                passed=bool(actual['ok'] and helper.finished_normally(actual)
                            and actual['value']==reference['value'] and actual['records']==reference['records'])
                row={**case,'actual':actual,'reference':reference,'matches_node':passed}
                if case['id'] in current_spec:
                    expected=current_spec[case['id']]
                    assert expected['source_sha256']==case['source_sha256']
                    assert expected['expected_records']!=reference['records']
                    row['current_spec_expectation']=expected
                    row['current_spec_passed']=bool(helper.finished_normally(actual)
                        and actual['ok']==expected['expected_ok'] and actual['value']==expected['expected_value']
                        and actual['records']==expected['expected_records'])
                    row['reference_difference_observed']=bool(actual['ok'] and helper.finished_normally(actual) and not passed)
                results.append(row)
            else:
                assert not reference['ok'] and not reference['records'] and 'SyntaxError' in reference['error']
                passed=bool(not actual['ok'] and helper.finished_normally(actual) and not actual['records'])
                early.append({**case,'actual':actual,'reference':reference,'fails_before_effects':passed})
        for case in bounds:
            assert hashlib.sha256(case['source'].encode()).hexdigest()==case['source_sha256']
            actual=helper.run_own(probe,case['source'],browser=case['browser_global'])
            passed=bool(helper.finished_normally(actual) and not actual['ok'] and not actual['records']
                        and case['error_fragment'] in (actual.get('error') or '').lower())
            limits.append({**case,'actual':actual,'passed':passed,'node_run':False})
    assert source_manifest(root)==before,'Source changed during review; freeze source before reporting'
    assert all(digest(path)==sha for path,sha in input_hashes.items()),'Checker/fixture input changed during review'
    report={'label':options.label,'scope':__doc__.strip(),'suite':options.suite,
        'groups':options.group or ([options.suite] if supplement else ['defaults','destructuring','named-regexp']),
        'node':subprocess.check_output([options.node,'--version'],text=True).strip(),
        'rustc':subprocess.check_output([options.rustc,'--version'],text=True).strip(),
        'source_manifest':before,'checker_sha256':digest(Path(__file__)),'helper_sha256':digest(HELPER),
        'fixture_sha256':digest(case_file),'bounds_fixture_sha256':digest(bounds_file),'original_reference_sha256':digest(reference_file),
        'checker_and_fixture_inputs_unchanged_during_run':True,
        'cases':len(results),'matches':sum(x['matches_node'] for x in results),
        'ordinary_cases':sum(x['id'] not in current_spec for x in results),
        'ordinary_matches':sum(x['matches_node'] for x in results if x['id'] not in current_spec),
        'reference_difference_cases':sum(x['id'] in current_spec for x in results),
        'reference_differences_observed':sum(x.get('reference_difference_observed',False) for x in results),
        'current_spec_assertions_passed':sum(x.get('current_spec_passed',False) for x in results),
        'current_spec_expectations_sha256':digest(spec_file) if current_spec else None,
        'early_error_cases':len(early),'early_errors_passed':sum(x['fails_before_effects'] for x in early),
        'bound_cases':len(limits),'bounds_passed':sum(x['passed'] for x in limits),
        'results':results,'early_errors':early,'bounds':limits}
    options.output.parent.mkdir(parents=True,exist_ok=True)
    options.output.write_text(json.dumps(report,indent=2)+'\n', encoding="utf-8")
    print(json.dumps({k:report[k] for k in ['label','groups','cases','matches','ordinary_cases','ordinary_matches','reference_difference_cases','reference_differences_observed','current_spec_assertions_passed','early_error_cases','early_errors_passed','bound_cases','bounds_passed']},indent=2))
    complete=(report['ordinary_matches']==report['ordinary_cases']
        and report['current_spec_assertions_passed']==report['reference_difference_cases']
        and report['reference_differences_observed']==report['reference_difference_cases']
        and report['early_errors_passed']==len(early) and report['bounds_passed']==len(limits))
    return 0 if options.allow_gaps or complete else 1

if __name__=='__main__':raise SystemExit(main())
