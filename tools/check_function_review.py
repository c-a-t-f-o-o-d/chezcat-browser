#!/usr/bin/env python3
"""Independent bounded Function-constructor review frozen before implementation.

Every comparison uses the pinned helper's existing browser-global mode on both
engines, including the exact 0.38 baseline. Host CSP/capability and infinite-body
checks are separate ChezCat-only assertions, never Node matches. The 160-probe
inventory and its fixed 112-probe cohort remain unchanged.
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
    ("function-constructor-call-and-new", r"""
var a=Function('x','y','return x+y;'),b=new Function('x','return x*2;');
test.record(a(11,12));test.record(b(13));test.record(a instanceof Function);test.record(b instanceof Function);
"""),
    ("function-constructor-global-not-caller-scope", r"""
var outer=14;let lexical=15;function build(){var outer=16;let lexical=17;return Function('return outer+lexical;');}
test.record(build()());test.record(outer);test.record(lexical);
"""),
    ("function-constructor-caller-and-body-strictness", r"""
function build(){'use strict';return Function('return this===globalThis;');}test.record(build()());
test.record(Function('"use strict";return this===undefined;')());
try{Function('a','a','"use strict";return a;');}catch(e){test.record(e.name);}
test.record(Function('a','a','return a;')(18,19));
"""),
    ("function-constructor-string-conversion-order", r"""
var log='';function text(mark,value){return {toString:function(){log+=mark;return value;}};}
var f=Function(text('p','a'),text('q','b'),text('r','return a+b;'));test.record(log);test.record(f(20,21));
log='';var marker={};try{Function({toString:function(){log+='x';throw marker;}},text('y','return 1;'));}catch(e){test.record(e===marker);}test.record(log);
"""),
    ("function-constructor-separate-parameter-body-parsing", r"""
for(var pair of [['a/*','*/return 1;'],['a) { return 1; } //','return 2;'],['a','} test.record("injected"); {']]){
try{Function(pair[0],pair[1]);test.record('unexpected success');}catch(e){test.record(e.name);}}
"""),
    ("function-constructor-comment-boundaries", r"""
test.record(Function('a//parameter comment','return a;//body comment')(22));
test.record(Function('a/*comment*/,b','return a+b;')(23,24));
"""),
    ("function-constructor-body-early-errors", r"""
var effects=0;try{Function('effects=1;return )');}catch(e){test.record(e.name);}test.record(effects);
try{Function('...rest','"use strict";return rest.length;');}catch(e){test.record(e.name);}
"""),
    ("function-constructor-metadata-and-anonymous-name", r"""
var f=Function('a,b','return typeof anonymous;');test.record(f.name);test.record(f.length);test.record(f());
test.record(Object.getPrototypeOf(f)===Function.prototype);test.record(f.prototype.constructor===f);
for(var key of ['name','length','prototype']){var d=Object.getOwnPropertyDescriptor(f,key);test.record(d.writable+','+d.enumerable+','+d.configurable);}
"""),
    ("function-constructor-parameter-closure", r"""
var factory=Function('start','return function(delta){start+=delta;return start;};'),step=factory(25);
test.record(step(2));test.record(step(3));test.record(factory(10)(1));
"""),
    ("function-constructor-new-and-borrowed-call", r"""
var C=Function('value','this.value=value;'),o=new C(31);test.record(o.value);test.record(o instanceof C);
var read=Function('extra','return this.value+extra;');test.record(read.call(o,2));test.record(read.bind(o,3)());
"""),
    ("function-constructor-comma-and-rest-parameters", r"""
test.record(Function('a,b','c','return a+b+c;')(1,2,3));var f=Function('first','...rest','return first+rest.length;');
test.record(f.length);test.record(f(4,5,6));test.record(Function()());
"""),
]

HOST_CASES=[
    ("function-policy-denial-after-conversion", r"""
test.blockEvalPolicy=true;var source={toString:function(){test.record('converted');return 'test.record("body ran")';}};
try{Function(source)();}catch(e){test.record(e.name);}test.record('continued');
""", True, ['string:636f6e766572746564','string:4576616c4572726f72','string:636f6e74696e756564'], None),
    ("function-capability-denial-is-fatal", r"""
test.denyEvalCapability=true;var source={toString:function(){test.record('converted');return 'return 1;';}};
try{Function(source);}catch(e){test.record('must not catch');}test.record('must not continue');
""", False, ['string:636f6e766572746564'], 'capability'),
    ("function-body-shares-job-bounds", r"""
Function('while(true){}')();test.record('must not continue');
""", False, [], 'limit'),
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
    spec=importlib.util.spec_from_file_location("function_review_helper",HELPER)
    helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
    root=options.source_root.resolve();before=source_manifest(root);results=[];host_results=[]
    with tempfile.TemporaryDirectory(prefix="chezcat-function-review-") as directory:
        directory=Path(directory);probe=directory/"probe";rust=directory/"probe.rs"
        rust.write_text(helper.PROBE.replace("JAVASCRIPT_SOURCE",json.dumps(str(root/"src/javascript.rs"))))
        subprocess.run(["rustc","--edition=2021","-O",str(rust),"-o",str(probe)],capture_output=True,text=True,timeout=120,check=True)
        for name,source in CASES:
            source=source.strip()+"\n'ok';\n";reference=helper.run_node("node",source,browser=True);actual=helper.run_own(probe,source,browser=True)
            assert reference['ok'] and reference['records'] and reference['value']=='string:6f6b',name
            matches=bool(actual['ok'] and helper.finished_normally(actual) and actual['value']==reference['value'] and actual['records']==reference['records'])
            results.append({'id':name,'source':source,'source_sha256':hashlib.sha256(source.encode()).hexdigest(),'actual':actual,'reference':reference,'matches_node':matches})
        for name,source,ok,records,error_fragment in HOST_CASES:
            source=source.strip()+"\n'ok';\n";actual=helper.run_own(probe,source,browser=True)
            passed=bool(helper.finished_normally(actual) and actual['ok']==ok and actual['records']==records and (not ok or actual['value']=='string:6f6b') and (error_fragment is None or error_fragment in (actual.get('error') or '').lower()))
            host_results.append({'id':name,'source':source,'source_sha256':hashlib.sha256(source.encode()).hexdigest(),'actual':actual,
                'expected_ok':ok,'expected_records':records,'expected_error_fragment':error_fragment,'passed':passed,'node_run':False})
    assert source_manifest(root)==before,'Source changed during review'
    report={'label':options.label,'scope':__doc__.strip(),'browser_global_mode':True,'node':subprocess.check_output(['node','--version'],text=True).strip(),
        'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),'source_manifest':before,'checker_sha256':digest(Path(__file__)),
        'helper_sha256':digest(HELPER),'cases':len(results),'matches':sum(x['matches_node'] for x in results),
        'host_cases':len(host_results),'host_passed':sum(x['passed'] for x in host_results),'results':results,'host_results':host_results}
    options.output.parent.mkdir(parents=True,exist_ok=True);options.output.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:report[k] for k in ['label','cases','matches','host_cases','host_passed']},indent=2))
    return 0 if options.allow_gaps or (report['matches']==len(results) and report['host_passed']==len(host_results)) else 1


if __name__=='__main__':raise SystemExit(main())
