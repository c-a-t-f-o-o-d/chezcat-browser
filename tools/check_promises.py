#!/usr/bin/env python3
"""Independent bounded Promise and microtask comparisons (development only).

Original finite programs run in fresh ChezCat and Node processes. ChezCat uses
explicit host microtask checkpoints. Node uses its real Promise/queueMicrotask
queues and reports at setImmediate; injecting a host queueMicrotask into a VM
with an isolated afterEvaluate Promise queue would test two different queues.
Each subprocess has an eight-second hard deadline. Node's initial script also
has a one-second VM timeout. Infinite and resource-bound programs run only in
ChezCat. No downloaded scripts, page captures, or networking are used.
"""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
CASE_FILE = ROOT / "tools/promise_cases.json"
NEXT_JOB = "\n//---NEXT JOB---\n"

PROBE = r'''
#![allow(dead_code)]
#[path = JAVASCRIPT_SOURCE]
mod javascript;
use javascript::{Host, Runtime, Value};
use std::io::{self, Read};

#[derive(Default)]
struct Recorder { records: Vec<String> }
fn representation(value: &Value) -> String {
    match value {
        Value::Undefined => "undefined".into(), Value::Null => "null".into(),
        Value::Bool(value) => format!("boolean:{value}"),
        Value::Number(value) if value.is_nan() => "number:NaN".into(),
        Value::Number(value) => format!("number:{:016x}", value.to_bits()),
        Value::String(value) => format!("string:{}", value.as_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>()),
        #[cfg(chezcat_string_units)]
        Value::StringUnits(units) => format!("string-units:{}", units.iter().map(|unit| format!("{unit:04x}")).collect::<String>()),
        Value::Symbol(_) => "symbol".into(), Value::Function(_) => "function".into(),
        Value::Object(_) => "object".into(), Value::Host(id) => format!("host:{id}"),
    }
}
impl Host for Recorder {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Ok(Value::Undefined) }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("No writable review fields".into()) }
    fn call(&mut self, object: u32, method: &str, args: &[Value]) -> Result<Value, String> {
        if object == 1 && method == "record" {
            if self.records.len() >= 512 { return Err("Review record limit".into()); }
            self.records.push(representation(args.first().unwrap_or(&Value::Undefined)));
            Ok(Value::Undefined)
        } else { Err("Unknown review method".into()) }
    }
}
fn main() {
    let mut source = String::new(); io::stdin().read_to_string(&mut source).unwrap();
    let mut runtime = Runtime::with_limits(javascript::limits::Limits {
        heap: javascript::limits::REGRESSION_HEAP,
        steps: 250_000, copy_bytes: 8 * 1024 * 1024,
        regexp_work: 2_000_000, job_seconds: 1,
        ..javascript::limits::BROWSER
    });
    runtime.define_global("test", Value::Host(1)).unwrap();
    let mut host = Recorder::default();
    let mut diagnostics: Vec<String> = Vec::new();
    let mut result = Ok(Value::Undefined);
    for job in source.split("\n//---NEXT JOB---\n") {
        runtime.begin_job();
        result = runtime.execute(job, &mut host);
        #[cfg(chezcat_microtasks)]
        match runtime.microtask_checkpoint(&mut host) {
            Ok(messages) => diagnostics.extend(messages),
            Err(error) => { result = Err(error); }
        }
        if result.is_err() { break; }
    }
    match result {
        Ok(value) => println!("OK\t{}", representation(&value)),
        Err(error) => println!("ERROR\t{}", error.replace('\n', " ")),
    }
    for value in host.records { println!("RECORD\t{value}"); }
    for value in diagnostics { println!("DIAGNOSTIC\t{}", value.replace('\n', " ")); }
}
'''

NODE_DRIVER = r'''
(() => {
const fs = require('fs'), vm = require('vm');
const source = fs.readFileSync(0, 'utf8');
const records = [], diagnostics = [];
function repr(value) {
    if(value===undefined)return 'undefined'; if(value===null)return 'null';
    if(typeof value==='boolean')return 'boolean:'+value;
    if(typeof value==='number') { if(Number.isNaN(value))return 'number:NaN'; const b=Buffer.alloc(8);b.writeDoubleBE(value);return 'number:'+b.toString('hex'); }
    if (typeof value === 'string') {
        const utf8 = Buffer.from(value, 'utf8');
        if (utf8.toString('utf8') === value) return 'string:' + utf8.toString('hex');
        return 'string-units:' + Buffer.from(value, 'utf16le').swap16().toString('hex');
    }
    if(typeof value==='symbol')return 'symbol'; if(typeof value==='function')return 'function';
    if(value===globalThis.test)return 'host:1'; return typeof value;
}
globalThis.test={record(value){if(records.length>=512)throw Error('Review record limit');records.push(repr(value));}};
process.on('unhandledRejection',reason=>diagnostics.push({kind:'unhandled_rejection',value:repr(reason)}));
process.on('uncaughtException',error=>diagnostics.push({kind:'uncaught_exception',value:repr(error)}));
let ok=true,error=null,completion,at=0;
const jobs=source.split('\n//---NEXT JOB---\n');
function next() {
    if(at>=jobs.length || !ok) {
        console.log(JSON.stringify({ok,records,diagnostics,error,value:ok?repr(completion):null}));
        return;
    }
    try { completion=new vm.Script(jobs[at++]).runInThisContext({timeout:1000}); }
    catch(e) {ok=false;error=String(e&&e.name)+': '+String(e&&e.message);}
    setImmediate(next);
}
next();
})();
'''


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_manifest(root):
    files = {p.relative_to(root).as_posix(): digest(p) for p in sorted(root.glob("src/**/*.rs"))}
    joined = "".join(f"{name}\0{value}\n" for name, value in files.items())
    return {"sha256": hashlib.sha256(joined.encode()).hexdigest(), "files": files}


def compile_probe(source_root, directory, rustc="rustc"):
    path = directory / "promise-probe.rs"
    executable = directory / ("promise-probe.exe" if os.name == "nt" else "promise-probe")
    javascript = source_root / "src/javascript.rs"
    path.write_text(PROBE.replace("JAVASCRIPT_SOURCE", json.dumps(str(javascript))), encoding="utf-8")
    available = "pub fn microtask_checkpoint(" in (source_root / "src/promises.rs").read_text(encoding="utf-8") if (source_root / "src/promises.rs").is_file() else False
    flags = ["--cfg", "chezcat_microtasks"] if available else []
    value_definition = javascript.read_text(encoding="utf-8").partition("pub enum Value {")[2].partition("\n}")[0]
    if "StringUnits(Vec<u16>)" in value_definition:
        flags.extend(["--cfg", "chezcat_string_units"])
    subprocess.run([rustc, "--edition=2021", "-O", *flags, str(path), "-o", str(executable)],
                   capture_output=True, text=True, timeout=120, check=True)
    return executable, available


def run_own(probe, source):
    started = time.monotonic()
    try:
        result = subprocess.run([str(probe)], input=source, capture_output=True, text=True, encoding="utf-8", timeout=8)
    except subprocess.TimeoutExpired:
        return {"ok": False, "timeout": True, "records": [], "diagnostics": [], "seconds": time.monotonic()-started}
    if result.returncode:
        return {"ok": False, "crash": result.returncode, "records": [], "diagnostics": [], "stderr": result.stderr, "seconds": time.monotonic()-started}
    lines = result.stdout.splitlines()
    valid = bool(lines and (lines[0].startswith("OK\t") or lines[0].startswith("ERROR\t")))
    return {"ok": bool(valid and lines[0].startswith("OK\t")),
            "value": lines[0][3:] if valid and lines[0].startswith("OK\t") else None,
            "error": lines[0][6:] if valid and lines[0].startswith("ERROR\t") else None,
            "protocol_error": not valid,
            "records": [line[7:] for line in lines if line.startswith("RECORD\t")],
            "diagnostics": [line[11:] for line in lines if line.startswith("DIAGNOSTIC\t")],
            "seconds": time.monotonic()-started}


def run_node(node, source):
    result = subprocess.run([node, "-e", NODE_DRIVER], input=source, capture_output=True,
                            text=True, encoding="utf-8", timeout=8, check=True)
    value = json.loads(result.stdout)
    if result.stderr:
        value["stderr"] = result.stderr
    return value


def finished_normally(value):
    return not any(value.get(key) for key in ("timeout", "crash", "protocol_error", "tool_error"))


def compare(actual, reference, expected_diagnostics=0):
    return bool(reference.get("ok") and finished_normally(actual) and actual.get("ok")
                and actual.get("value") == reference.get("value")
                and actual["records"] == reference["records"]
                and len(actual["diagnostics"]) == len(reference["diagnostics"]) == expected_diagnostics)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--label", default="current")
    parser.add_argument("--node", default="node")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--case-prefix", action="append", default=[])
    parser.add_argument("--allow-gaps", action="store_true")
    parser.add_argument("--references-only", action="store_true")
    options = parser.parse_args()
    data = json.loads(CASE_FILE.read_text(encoding="utf-8"))
    cases = [case for case in data["cases"] if not options.case_prefix or any(case["id"].startswith(p) for p in options.case_prefix)]
    negatives = [] if options.references_only else data["negative_cases"]
    if not 1 <= len(cases) <= 128 or len({case["id"] for case in cases}) != len(cases):
        parser.error("Expected 1-128 distinct Promise comparisons")
    if any(len(case["source"].encode()) > 128*1024 for case in cases+negatives):
        parser.error("Promise fixture exceeds 128 KiB")
    root = options.source_root.resolve()
    before = source_manifest(root)
    results, bounds = [], []
    try:
        node_version = subprocess.check_output([options.node,"--version"],text=True,timeout=10).strip()
        rust_version = subprocess.check_output([options.rustc,"--version"],text=True,timeout=10).strip()
        references = [run_node(options.node, case["source"]) for case in cases]
        for case, reference in zip(cases, references):
            assert reference["ok"] and reference["value"] == "string:6f6b" and reference["records"], case["id"]
            assert len(reference["diagnostics"]) == case.get("expected_diagnostics", 0), case["id"]
        checkpoint_available = False
        if options.references_only:
            results = [{"id":case["id"], "source_sha256":hashlib.sha256(case["source"].encode()).hexdigest(), "reference":ref}
                       for case,ref in zip(cases,references)]
        else:
            with tempfile.TemporaryDirectory(prefix="chezcat-promises-") as directory:
                probe, checkpoint_available = compile_probe(root, Path(directory), options.rustc)
                for case, reference in zip(cases, references):
                    actual = run_own(probe, case["source"])
                    results.append({"id":case["id"], "source_sha256":hashlib.sha256(case["source"].encode()).hexdigest(),
                        "passed":compare(actual,reference,case.get("expected_diagnostics",0)), "actual":actual,"reference":reference})
                for case in negatives:
                    actual = run_own(probe,case["source"])
                    passed = checkpoint_available and finished_normally(actual) and not actual["ok"] and not actual["records"]
                    bounds.append({"id":case["id"],"source_sha256":hashlib.sha256(case["source"].encode()).hexdigest(),"passed":bool(passed),"actual":actual})
        if before != source_manifest(root):
            raise ValueError("Source changed during Promise check; this is not a frozen-source report")
    except (OSError, subprocess.SubprocessError, ValueError, AssertionError) as error:
        print("Promise checker failed: " + str(error))
        if getattr(error,"stderr",None): print(error.stderr)
        return 2
    report = {"label":options.label,"scope":__doc__.strip(),"node":node_version,"rustc":rust_version,
        "source_manifest":before,"checker_sha256":digest(Path(__file__)),"fixture_sha256":digest(CASE_FILE),
        "checkpoint_available":checkpoint_available,"references_only":options.references_only,
        "comparisons":len(results),"passed":sum(case.get("passed",False) for case in results),
        "negative_cases":len(bounds),"negative_passed":sum(case["passed"] for case in bounds),"results":results,"negatives":bounds}
    options.output.parent.mkdir(parents=True,exist_ok=True)
    options.output.write_text(json.dumps(report,indent=2)+"\n", encoding="utf-8")
    print(json.dumps({key:report[key] for key in ("label","comparisons","passed","negative_cases","negative_passed","checkpoint_available","references_only")},indent=2))
    if options.references_only or options.allow_gaps: return 0
    return 0 if report["passed"]==len(results) and report["negative_passed"]==len(bounds) else 1


if __name__ == "__main__":
    raise SystemExit(main())
