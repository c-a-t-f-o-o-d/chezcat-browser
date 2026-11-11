#!/usr/bin/env python3
"""Supplemental original Promise abrupt-completion and reference review.

Kept separate from the 40 fixed Promise comparisons and compatibility inventory.
Uses the same bounded native-queue helper; observed current-spec reference
differences remain visible instead of being included in Node-match totals.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import struct
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / "tools/verification-0.39/check_promises-inventory-helper.py"
PROMISE_SPEC = "https://tc39.es/ecma262/multipage/control-abstraction-objects.html"
TRY_SPEC = PROMISE_SPEC + "#sec-promise.try"

CASES = [
    ("aggregate-error-own-data-and-prototype", """
var original=Object.getOwnPropertyDescriptor(Error.prototype,'message'),writes=0;
Object.defineProperty(Error.prototype,'message',{get:function(){throw 'inherited message';},set:function(){writes++;},configurable:true});
var e=AggregateError([1,2],'details',{cause:3});
Object.defineProperty(Error.prototype,'message',original);
test.record(writes);test.record(e instanceof AggregateError);test.record(e instanceof Error);
test.record(e.message);test.record(e.cause);test.record(e.errors.join(','));
for(var key of ['message','cause','errors']){var d=Object.getOwnPropertyDescriptor(e,key);test.record(d.writable+','+d.enumerable+','+d.configurable);}
""", 0),
    ("aggregate-error-conversion-cause-iteration-order", """
var log='',errors={},options=Object.create({get cause(){log+='c';return 4;}});
Object.defineProperty(errors,Symbol.iterator,{get:function(){log+='i';return function(){var n=0;return {next:function(){log+='n';return n++?{done:true}:{value:5,done:false};}};};}});
var e=new AggregateError(errors,{toString:function(){log+='m';return 'message';}},options);
test.record(log);test.record(e.message);test.record(e.cause);test.record(e.errors[0]);
""", 0),
    ("aggregate-error-step-abrupt-does-not-close", """
function run(mode){var marker={},closed=0,source={};source[Symbol.iterator]=function(){return {next:function(){if(mode===0)throw marker;return {get done(){if(mode===1)throw marker;return false;},get value(){throw marker;}};},return:function(){closed++;return {};}};};try{new AggregateError(source);}catch(e){test.record(e===marker);}test.record(closed);}
run(0);run(1);run(2);
""", 0),
    ("promise-combinator-step-abrupt-does-not-close", """
function run(method,mode){var marker={},closed=0,source={};source[Symbol.iterator]=function(){return {next:function(){if(mode===0)throw marker;return {get done(){if(mode===1)throw marker;return false;},get value(){throw marker;}};},return:function(){closed++;return {};}};};Promise[method](source).catch(function(e){test.record(method+':'+mode);test.record(e===marker);test.record(closed);});}
for(var method of ['all','race','allSettled','any'])for(var mode=0;mode<3;mode++)run(method,mode);
test.record('queued');
""", 0),
    ("promise-combinator-close-preserves-original-throw", """
var original={},closing={},closed=0,source={};source[Symbol.iterator]=function(){return {next:function(){return {value:1,done:false};},get return(){closed++;throw closing;}};};
function C(executor){return new Promise(executor);}C.resolve=function(){return {get then(){throw original;}};};
Promise.all.call(C,source).catch(function(e){test.record(e===original);test.record(closed);});test.record('queued');
""", 0),
    ("promise-resolve-getter-before-iterator", """
var log='',marker={},source={};Object.defineProperty(source,Symbol.iterator,{get:function(){log+='i';throw 'wrong';}});
function C(executor){return new Promise(executor);}Object.defineProperty(C,'resolve',{get:function(){log+='r';throw marker;}});
Promise.race.call(C,source).catch(function(e){test.record(e===marker);test.record(log);});test.record(log);
""", 0),
    ("promise-identity-before-constructor-validation", """
var p=Promise.resolve(2),holder={};p.constructor=holder;test.record(Promise.resolve.call(holder,p)===p);
try{Promise.resolve.call(holder,3);}catch(e){test.record(e.name);}
""", 0),
    ("promise-capability-undefined-first-call", """
function C(executor){executor(undefined,undefined);var self=this;executor(function(v){self.value=v;},function(v){self.error=v;});}
var p=Promise.resolve.call(C,7);test.record(p instanceof C);test.record(p.value);
""", 0),
    ("promise-try-ordinary-control", """
test.record(Promise.try.name);test.record(Promise.try.length);test.record(Promise.try.hasOwnProperty('prototype'));
Promise.try(function(a,b){'use strict';test.record(this===undefined);test.record(arguments.length);return a+b;},4,5).then(function(v){test.record(v);});test.record('queued');
""", 0),
    ("promise-try-rejection-control", """
var marker={};Promise.try(function(){throw marker;}).catch(function(e){test.record(e===marker);});
Promise.try(7).catch(function(e){test.record(e.name);});test.record('queued');
""", 0),
]

DIFFERENCES = [
    ("promise-try-callback-before-constructor", """
var log='';function C(executor){log+='constructor;';return new Promise(executor);}
Promise.try.call(C,function(){log+='callback;';return 10;}).then(function(v){test.record(v);});test.record(log);
""", ["callback;constructor;", 10], 0, [TRY_SPEC],
     "Current Promise.try calls the callback before PromiseResolve creates a capability for its normal result."),
    ("promise-try-returned-promise-identity", """
var p=Promise.resolve(11),q=Promise.try(function(){return p;});test.record(q===p);q.then(function(v){test.record(v);});
""", [True, 11], 0, [TRY_SPEC],
     "The normal path uses PromiseResolve, which returns the existing Promise when its constructor matches."),
    ("promise-try-object-nonconstructor-side-effect", """
var called=false;try{Promise.try.call({},function(){called=true;return 12;});}catch(e){test.record(e.name);}test.record(called);
""", ["TypeError", True], 0, [TRY_SPEC],
     "An object receiver passes the initial type check; callback effects precede later capability constructor validation."),
    ("promise-custom-resolve-throw-reports-job-error", """
function C(executor){executor(function(){throw 8;},function(){test.record('wrong reject');});}
var p=Promise.resolve(9);p.constructor={};p.constructor[Symbol.species]=C;
p.then(function(v){test.record(v);});queueMicrotask(function(){test.record('continued');});test.record('queued');
""", ["queued", 9, "continued"], 1,
     [PROMISE_SPEC + "#sec-newpromisereactionjob", "https://html.spec.whatwg.org/multipage/webappapis.html#hostenqueuepromisejob"],
     "NewPromiseReactionJob step 9 directly returns the capability Resolve call, including an abrupt completion; HTML HostEnqueuePromiseJob reports that exception. Node 24 instead invokes the custom Reject in this original program. ChezCat reports one bounded checkpoint diagnostic and continues the queue."),
]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def representation(value):
    if type(value) is bool: return "boolean:"+str(value).lower()
    if isinstance(value,str): return "string:"+value.encode().hex()
    return "number:"+struct.pack(">d",value).hex()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root",type=Path,default=ROOT)
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--label",default="current")
    options=parser.parse_args()
    spec=importlib.util.spec_from_file_location("promise_review_helper",HELPER)
    helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
    root=options.source_root.resolve();before=helper.source_manifest(root)
    results=[];differences=[]
    with tempfile.TemporaryDirectory(prefix="chezcat-promise-review-") as directory:
        probe,available=helper.compile_probe(root,Path(directory))
        assert available
        for name,source,count in CASES:
            source=source.strip()+"\n'ok';\n"
            reference=helper.run_node("node",source);actual=helper.run_own(probe,source)
            assert reference['ok'] and reference['records'] and len(reference['diagnostics'])==count,name
            results.append({'id':name,'source':source,'source_sha256':hashlib.sha256(source.encode()).hexdigest(),
                'actual':actual,'reference':reference,'matches_node':helper.compare(actual,reference,count)})
        for name,source,expected,diagnostics,primary_urls,basis in DIFFERENCES:
            source=source.strip()+"\n'ok';\n"
            reference=helper.run_node("node",source);actual=helper.run_own(probe,source)
            assert reference['ok'] and not reference['diagnostics'],name
            conforms=bool(actual['ok'] and helper.finished_normally(actual) and actual['value']=='string:6f6b'
                          and len(actual['diagnostics'])==diagnostics and actual['records']==[representation(x) for x in expected])
            differences.append({'id':name,'source':source,'source_sha256':hashlib.sha256(source.encode()).hexdigest(),
                'actual':actual,'reference':reference,'matches_node':helper.compare(actual,reference),
                'expected_current_spec_records':expected,'expected_current_spec_diagnostics':diagnostics,
                'chezcat_follows_current_spec':conforms,'primary_urls':primary_urls,'basis':basis})
    assert helper.source_manifest(root)==before,'Source changed during review'
    report={'label':options.label,'scope':__doc__.strip(),'node':subprocess.check_output(['node','--version'],text=True).strip(),
        'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),'source_manifest':before,
        'reviewer_sha256':digest(Path(__file__)),'helper_sha256':digest(HELPER),'ordinary_cases':len(results),
        'ordinary_matches':sum(x['matches_node'] for x in results),'reference_difference_cases':len(differences),
        'observed_reference_differences':sum(not x['matches_node'] for x in differences),
        'current_spec_assertions_passed':sum(x['chezcat_follows_current_spec'] for x in differences),
        'results':results,'reference_differences':differences}
    options.output.parent.mkdir(parents=True,exist_ok=True);options.output.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:report[k] for k in ['label','ordinary_cases','ordinary_matches','reference_difference_cases','observed_reference_differences','current_spec_assertions_passed']},indent=2))
    return 0 if report['ordinary_matches']==len(results) and report['current_spec_assertions_passed']==len(differences) else 1


if __name__=='__main__':
    raise SystemExit(main())
