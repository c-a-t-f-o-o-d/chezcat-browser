#!/usr/bin/env python3
"""Independent optional-chain/nullish review frozen before implementation.

Original finite Node programs, explicit success/completion/record checks, and
syntax errors that must fail before effects. The existing 160-probe inventory
is unchanged. Logical assignment and exponentiation are outside this review.
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
HELPER=ROOT/"tools/verification-0.39/check_javascript-inventory-helper.py"

CASES=[
    ("nullish-distinguishes-falsy", r"""
for(var value of [null,undefined,false,0,'',NaN])test.record(value??19);
"""),
    ("optional-skips-computed-keys-and-call-arguments", r"""
var count=0,a=null,b;test.record(a?.[count++]);test.record(a?.method(count++));test.record(b?.(count++));test.record(count);
"""),
    ("optional-contiguous-chain-only", r"""
var empty=null,full={a:{b:21}};test.record(empty?.a.b);test.record(full?.a.b);
try{({a:null})?.a.b;}catch(e){test.record(e.name);}
"""),
    ("optional-call-preserves-method-receiver", r"""
var o={value:22,method:function(x){test.record(this===o);return this.value+x;}};
test.record(o?.method(1));test.record(o.method?.(2));test.record(o?.['method']?.(3));
"""),
    ("optional-parentheses-end-short-circuit", r"""
var a=null,count=0;test.record((a?.x));
try{(a?.x).y;}catch(e){test.record(e.name);}
try{(a?.x)(count++);}catch(e){test.record(e.name);}test.record(count);
"""),
    ("optional-parentheses-preserve-receiver-reference", r"""
var o={key:26,method:function(){'use strict';return this===undefined?'none':this.key;}};
test.record((o?.method)());test.record((o.method)?.());test.record((0,o?.method)());
"""),
    ("optional-callee-validation-and-getter-order", r"""
var log='',a={get method(){log+='g';return undefined;}};function arg(){log+='a';return 1;}
test.record(a.method?.(arg()));test.record(log);
try{({method:0}).method?.(arg());}catch(e){test.record(e.name);}test.record(log);
"""),
    ("optional-delete-reference-and-skipped-effects", r"""
var count=0,o={x:1},p=Object.create({inherited:2});
test.record(delete null?.[count++]);test.record(delete o?.x);test.record('x' in o);test.record(delete p?.inherited);test.record(p.inherited);test.record(count);
Object.defineProperty(o,'fixed',{value:3,configurable:false});test.record(delete o?.fixed);
"""),
    ("nullish-expression-order-and-parenthesized-mixing", r"""
var log='';function read(k,v){log+=k;return v;}
test.record(read('a',0)??read('b',2));test.record(read('c',null)??read('d',undefined)??read('e',27));test.record(log);
test.record((false||null)??28);test.record((1&&0)??29);test.record(null??(false||30));test.record(0??true?31:32);
"""),
    ("optional-getter-and-call-evaluated-once", r"""
var log='',o={get method(){log+='g';return function(){log+=this===o?'r':'x';return {x:33};};}};
test.record(o.method?.()?.x);test.record(log);
"""),
    ("optional-eval-call-is-indirect", r"""
var marker=34;function read(){var marker=35;return eval?.('marker');}test.record(read());test.record(marker);
"""),
    ("optional-unresolvable-name-still-throws", r"""
try{missing_optional_review_name?.x;}catch(e){test.record(e.name);}
try{missing_optional_review_name?.();}catch(e){test.record(e.name);}
var count=0;test.record(null?.()[count++]);test.record(count);
"""),
]

EARLY_ERRORS=[
    ("nullish-unparenthesized-or", "var a=1,b=2,c=3;a??b||c;"),
    ("nullish-unparenthesized-and", "var a=1,b=2,c=3;a&&b??c;"),
    ("optional-assignment-is-invalid", "var a={};a?.x=1;"),
    ("optional-constructor-chain-is-invalid", "function C(){}new C?.();"),
    ("optional-tagged-template-is-invalid", "function tag(){}tag?.`x`;"),
]


def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def source_manifest(root):
    paths=list(root.glob("src/**/*.rs"))+[root/n for n in ("Cargo.toml","Cargo.lock","build.rs") if (root/n).is_file()]
    files={p.relative_to(root).as_posix():digest(p) for p in sorted(paths)}
    joined="".join(f"{n}\0{v}\n" for n,v in files.items())
    return {"sha256":hashlib.sha256(joined.encode()).hexdigest(),"files":files}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root",type=Path,default=ROOT)
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--label",default="current")
    parser.add_argument("--allow-gaps",action="store_true")
    options=parser.parse_args()
    spec=importlib.util.spec_from_file_location("optional_review_helper",HELPER)
    helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
    root=options.source_root.resolve();before=source_manifest(root);results=[];boundaries=[]
    with tempfile.TemporaryDirectory(prefix="chezcat-optional-review-") as directory:
        directory=Path(directory);probe=directory/"probe";rust=directory/"probe.rs"
        rust.write_text(helper.PROBE.replace("JAVASCRIPT_SOURCE",json.dumps(str(root/"src/javascript.rs"))))
        subprocess.run(["rustc","--edition=2021","-O",str(rust),"-o",str(probe)],capture_output=True,text=True,timeout=120,check=True)
        for name,source in CASES:
            source=source.strip()+"\n'ok';\n";reference=helper.run_node("node",source);actual=helper.run_own(probe,source)
            assert reference['ok'] and reference['records'] and reference['value']=='string:6f6b',name
            matches=bool(actual['ok'] and helper.finished_normally(actual) and actual['value']==reference['value'] and actual['records']==reference['records'])
            results.append({'id':name,'source':source,'source_sha256':hashlib.sha256(source.encode()).hexdigest(),'actual':actual,'reference':reference,'matches_node':matches})
        for name,source in EARLY_ERRORS:
            source="test.record('must not execute');\n"+source+"\n'ok';\n"
            reference=helper.run_node("node",source);actual=helper.run_own(probe,source)
            assert not reference['ok'] and not reference['records'],name
            failed=bool(not actual['ok'] and helper.finished_normally(actual) and not actual['records'])
            boundaries.append({'id':name,'source':source,'source_sha256':hashlib.sha256(source.encode()).hexdigest(),'actual':actual,'reference':reference,'fails_before_effects':failed})
    assert source_manifest(root)==before,'Source changed during review'
    report={'label':options.label,'scope':__doc__.strip(),'node':subprocess.check_output(['node','--version'],text=True).strip(),
        'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),'source_manifest':before,'checker_sha256':digest(Path(__file__)),
        'helper_sha256':digest(HELPER),'cases':len(results),'matches':sum(x['matches_node'] for x in results),
        'early_error_cases':len(boundaries),'early_errors_passed':sum(x['fails_before_effects'] for x in boundaries),'results':results,'early_errors':boundaries}
    options.output.parent.mkdir(parents=True,exist_ok=True);options.output.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:report[k] for k in ['label','cases','matches','early_error_cases','early_errors_passed']},indent=2))
    return 0 if options.allow_gaps or (report['matches']==len(results) and report['early_errors_passed']==len(boundaries)) else 1


if __name__=='__main__':raise SystemExit(main())
