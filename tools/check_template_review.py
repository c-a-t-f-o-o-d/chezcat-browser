#!/usr/bin/env python3
"""Independent bounded untagged-template review, frozen before implementation.

Four original positive programs are also separate inventory samples. The fifth
checks cooked UTF-16 pairs. Deliberate unsupported shapes and syntax errors are
kept outside match percentages and must fail before any record is emitted.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / "tools/check_javascript.py"

CASES = [
    ("template-cooked-text-and-lines", "Cooked escapes and multiline interpolation", r"""
var n=4;
test.record(`tick:\` dollar:\${literal} slash:\\ \x41\u0042 ${n}`);
test.record(`one
two:${n+1}`);
"""),
    ("template-nested-expression-lexing", "Nested templates, braces, comments and regex literals", r"""
var n=3;
test.record(`start ${`inner:${n+2}`} ${({x:'}'}).x} end`);
test.record(`fn:${(function(){return 'a}';})()} comment:${1 /* } */ +2} regex:${/}/.test('}')}`);
"""),
    ("template-conversion-and-abrupt-order", "String conversion precedes later expressions and abrupt completion stops them", r"""
var log='',a={toString:function(){log+='s';return 'A';},valueOf:function(){log+='v';return 1;}};
function later(){log+='e';return 'B';}
test.record(`${a}${later()}`);test.record(log);
var ran=false;try{`${{toString:function(){throw 13;}}}${ran=true}`;}catch(e){test.record(e);}test.record(ran);
"""),
    ("template-expression-reference-and-precedence", "Interpolation preserves reference updates, expression precedence and function returns", r"""
var n=0,o={x:7};test.record(`${n++}:${o['x']}:${true?2:3}:${(4+5)*2}`);test.record(n);
function render(v){return `(${v})`;}test.record(render(8));
"""),
    ("template-cooked-scalar-and-surrogate-pair", "Cooked Unicode scalar escapes and valid surrogate pairs", r"""
var n=2;test.record(`\u{1F408}:\uD83D\uDC08:${n}`);
"""),
]

BOUNDARIES = [
    ("template-tagged-unsupported", "intentional", True, r"""
test.record('must not execute');function tag(strings,value){return strings[0]+value;}tag`answer:${2}`;
"""),
    ("template-lone-surrogate-unsupported", "intentional", True, r"""
test.record('must not execute');`\uD800:${2}`;
"""),
    ("template-invalid-hex-escape", "syntax", False, r"""
test.record('must not execute');`\xQ1:${2}`;
"""),
    ("template-empty-interpolation", "syntax", False, r"""
test.record('must not execute');`left ${} right`;
"""),
]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_manifest(root):
    paths=list(root.glob("src/**/*.rs"))+[root/n for n in ("Cargo.toml","Cargo.lock","build.rs") if (root/n).is_file()]
    files={p.relative_to(root).as_posix():digest(p) for p in sorted(paths)}
    joined="".join(f"{n}\0{v}\n" for n,v in files.items())
    return {"sha256":hashlib.sha256(joined.encode()).hexdigest(),"files":files}


def program(source):
    return source.strip()+"\n'ok';\n"


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root",type=Path,default=ROOT)
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--label",default="current")
    parser.add_argument("--allow-gaps",action="store_true")
    options=parser.parse_args()
    spec=importlib.util.spec_from_file_location("template_review_helper",HELPER)
    helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
    root=options.source_root.resolve();before=source_manifest(root);results=[];boundaries=[]
    with tempfile.TemporaryDirectory(prefix="chezcat-template-review-") as directory:
        directory=Path(directory);probe=directory/"probe";rust=directory/"probe.rs"
        rust.write_text(helper.PROBE.replace("JAVASCRIPT_SOURCE",json.dumps(str(root/"src/javascript.rs"))), encoding="utf-8")
        subprocess.run(["rustc","--edition=2021","-O",str(rust),"-o",str(probe)],capture_output=True,text=True,timeout=120,check=True)
        for name,label,source in CASES:
            source=program(source);reference=helper.run_node("node",source);actual=helper.run_own(probe,source)
            assert reference['ok'] and reference['records'] and reference['value']=='string:6f6b',name
            matches=bool(actual['ok'] and helper.finished_normally(actual) and actual['value']==reference['value'] and actual['records']==reference['records'])
            results.append({'id':name,'label':label,'source':source,'source_sha256':hashlib.sha256(source.encode()).hexdigest(),'actual':actual,'reference':reference,'matches_node':matches})
        for name,kind,node_ok,source in BOUNDARIES:
            source=program(source);reference=helper.run_node("node",source);actual=helper.run_own(probe,source)
            assert reference['ok']==node_ok,name
            failed=bool(not actual['ok'] and helper.finished_normally(actual) and not actual['records'])
            boundaries.append({'id':name,'kind':kind,'source':source,'source_sha256':hashlib.sha256(source.encode()).hexdigest(),'actual':actual,'reference':reference,'fails_before_effects':failed})
    assert source_manifest(root)==before,'Source changed during review'
    report={'label':options.label,'scope':__doc__.strip(),'node':subprocess.check_output(['node','--version'],text=True).strip(),
        'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),'source_manifest':before,
        'checker_sha256':digest(Path(__file__)),'helper_sha256':digest(HELPER),'cases':len(results),'matches':sum(x['matches_node'] for x in results),
        'boundary_cases':len(boundaries),'boundaries_passed':sum(x['fails_before_effects'] for x in boundaries),'results':results,'boundaries':boundaries}
    options.output.parent.mkdir(parents=True,exist_ok=True);options.output.write_text(json.dumps(report,indent=2)+'\n', encoding="utf-8")
    print(json.dumps({k:report[k] for k in ['label','cases','matches','boundary_cases','boundaries_passed']},indent=2))
    return 0 if options.allow_gaps or (report['matches']==len(results) and report['boundaries_passed']==len(boundaries)) else 1


if __name__=='__main__':
    raise SystemExit(main())
