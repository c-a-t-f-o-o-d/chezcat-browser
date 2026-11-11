#!/usr/bin/env python3
"""Independent frozen 0.41 String wrapper, operator and HTTP(S) URL review.

No network. Original finite programs and Node references were frozen before
implementation outcomes. Early errors and intentional limits are separate.
Current specification/reference differences never count as Node matches.
"""
from pathlib import Path
import argparse,hashlib,importlib.util,json,subprocess,sys,tempfile
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[1]
HELPER=ROOT/'tools/verification-0.41/check_javascript-review-helper.py'
CASES=ROOT/'tools/review_cases_041.json'
REFERENCE=ROOT/'tools/verification-0.41/original-node-references.json'
SPEC=ROOT/'tools/verification-0.41/current-spec-expectations.json'
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def source_manifest(root):
    paths=list(root.glob('src/**/*.rs'))+[root/n for n in ('Cargo.toml','Cargo.lock','build.rs') if (root/n).is_file()]
    files={p.relative_to(root).as_posix():digest(p) for p in sorted(paths)}
    return {'sha256':hashlib.sha256(''.join(f'{n}\0{v}\n' for n,v in files.items()).encode()).hexdigest(),'files':files}
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root',type=Path,default=ROOT)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--label',default='current')
    parser.add_argument('--group',choices=['string-wrapper','operators','url'],action='append')
    parser.add_argument('--allow-gaps',action='store_true')
    parser.add_argument('--rustc',default='rustc');parser.add_argument('--node',default='node')
    args=parser.parse_args();root=args.source_root.resolve();before=source_manifest(root)
    spec=importlib.util.spec_from_file_location('frozen_review041',HELPER)
    helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
    inputs=[Path(__file__),HELPER,CASES,REFERENCE]
    exceptions={}
    if SPEC.exists():
        inputs.append(SPEC);document=json.loads(SPEC.read_text());assert document['fixture_sha256']==digest(CASES)
        exceptions={c['id']:c for c in document['cases']}
    input_hashes={p:digest(p) for p in inputs}
    cases=json.loads(CASES.read_text())['cases'];original=json.loads(REFERENCE.read_text())
    assert original['fixture_sha256']==digest(CASES) and original['helper_sha256']==digest(HELPER)
    originals={c['id']:c for c in original['cases']};assert not original['invalid_preconditions']
    if args.group:cases=[c for c in cases if c['group'] in args.group]
    rows=[];early=[]
    with tempfile.TemporaryDirectory(prefix='chezcat-review041-') as folder:
        folder=Path(folder);probe=folder/'probe';rust=folder/'probe.rs'
        rust.write_text(helper.PROBE.replace('JAVASCRIPT_SOURCE',json.dumps(str(root/'src/javascript.rs'))))
        built=subprocess.run([args.rustc,'--edition=2021','-O',str(rust),'-o',str(probe)],capture_output=True,text=True,timeout=180)
        if built.returncode:raise RuntimeError(built.stderr)
        for case in cases:
            assert hashlib.sha256(case['source'].encode()).hexdigest()==case['source_sha256']
            original_case=originals[case['id']];assert all(original_case[k]==v for k,v in case.items())
            reference=helper.run_node(args.node,case['source'],browser=case['browser_global'])
            assert reference==original_case['reference'],('Node reference changed',case['id'])
            actual=helper.run_own(probe,case['source'],browser=case['browser_global'])
            row={**case,'actual':actual,'reference':reference}
            if case['kind']=='early-error':
                assert not reference['ok'] and not reference['records'] and 'SyntaxError' in reference['error']
                row['fails_before_effects']=bool(helper.finished_normally(actual) and not actual['ok'] and not actual['records'])
                early.append(row);continue
            assert reference['ok'] and reference['records'] and reference['value']=='string:6f6b'
            row['matches_node']=bool(helper.finished_normally(actual) and actual['ok'] and actual['records']==reference['records'] and actual['value']==reference['value'])
            if case['id'] in exceptions:
                expected=exceptions[case['id']];assert expected['source_sha256']==case['source_sha256'];assert expected['expected_records']!=reference['records']
                row['current_spec_expectation']=expected
                row['current_spec_passed']=bool(helper.finished_normally(actual) and actual['ok']==expected['expected_ok'] and actual['value']==expected['expected_value'] and actual['records']==expected['expected_records'])
                row['reference_difference_observed']=bool(helper.finished_normally(actual) and actual['ok'] and not row['matches_node'])
            rows.append(row)
    after=source_manifest(root);assert before==after,'Source changed during review'
    assert all(digest(p)==sha for p,sha in input_hashes.items()),'Checker or fixtures changed during review'
    report={'label':args.label,'scope':__doc__.strip(),'groups':args.group or ['string-wrapper','operators','url'],'source_manifest':before,'source_manifest_after':after,
        'checker_sha256':digest(Path(__file__)),'helper_sha256':digest(HELPER),'fixture_sha256':digest(CASES),'original_reference_sha256':digest(REFERENCE),
        'current_spec_expectations_sha256':digest(SPEC) if SPEC.exists() else None,
        'checker_and_fixture_inputs_unchanged_during_run':True,'node':subprocess.check_output([args.node,'--version'],text=True).strip(),'rustc':subprocess.check_output([args.rustc,'--version'],text=True).strip(),
        'cases':len(rows),'matches':sum(x['matches_node'] for x in rows),'ordinary_cases':sum(x['id'] not in exceptions for x in rows),'ordinary_matches':sum(x['matches_node'] for x in rows if x['id'] not in exceptions),
        'reference_difference_cases':sum(x['id'] in exceptions for x in rows),'reference_differences_observed':sum(x.get('reference_difference_observed',False) for x in rows),'current_spec_assertions_passed':sum(x.get('current_spec_passed',False) for x in rows),
        'early_error_cases':len(early),'early_errors_passed':sum(x['fails_before_effects'] for x in early),'results':rows,'early_errors':early}
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n')
    summary={k:v for k,v in report.items() if k in ['label','groups','cases','matches','ordinary_cases','ordinary_matches','reference_difference_cases','reference_differences_observed','current_spec_assertions_passed','early_error_cases','early_errors_passed']}
    print(json.dumps(summary,indent=2))
    complete=(report['ordinary_cases']==report['ordinary_matches'] and report['reference_difference_cases']==report['current_spec_assertions_passed'] and report['reference_difference_cases']==report['reference_differences_observed'] and report['early_error_cases']==report['early_errors_passed'])
    return 0 if args.allow_gaps or complete else 1
if __name__=='__main__':raise SystemExit(main())
