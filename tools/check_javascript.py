#!/usr/bin/env python3
"""Independent checks for ChezCat's original JavaScript subset.

Development only: Node.js supplies comparison results for selected language
behavior. Node is never linked into, bundled with, or invoked by ChezCat.
The probe compiles src/javascript.rs directly with rustc; no Rust crates or Python
packages are installed or required. All programs below are local test fixtures.

Run from any directory:
    python tools/check_javascript.py [--rustc PATH] [--node PATH] [--output report.json]
    python tools/check_javascript.py --case-prefix fromcharcode- --case-prefix unicode-pair

Primary reference: https://tc39.es/ecma262/
These targeted comparisons are not the ECMAScript conformance suite or a claim
that modern websites work. The remaining cases check explicit bounds, unsupported
features, and parse-before-execution behavior that differ from a full JS engine.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]

PROBE = r'''
#![allow(dead_code)]
#[path = JAVASCRIPT_SOURCE]
mod javascript;

use javascript::{Host, Runtime, Value};
use std::collections::HashMap;
use std::io::{self, Read};

#[derive(Default)]
struct Recorder {
    records: Vec<String>,
    fields: HashMap<String, Value>,
    callback: Option<Value>,
}

fn representation(value: &Value) -> String {
    match value {
        Value::Undefined => "undefined".into(),
        Value::Null => "null".into(),
        Value::Bool(value) => format!("boolean:{value}"),
        Value::Number(value) if value.is_nan() => "number:NaN".into(),
        Value::Number(value) => format!("number:{:016x}", value.to_bits()),
        Value::String(value) => format!("string:{}", value.as_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>()),
        Value::Symbol(_) => "symbol".into(),
        Value::Function(_) => "function".into(),
        Value::Object(_) => "object".into(),
        Value::Host(id) => format!("host:{id}"),
    }
}

impl Host for Recorder {
    // Fixed test-host entropy makes the capability deterministic. The comparison
    // below checks the required range/type, not a particular random sequence.
    fn random(&mut self) -> Result<f64, String> { Ok(0.125) }
    fn allow_eval(&mut self) -> Result<bool, String> {
        if self.fields.get("denyEvalCapability").is_some_and(Value::truthy) {
            Err("Eval capability denied by review host".into())
        } else {
            Ok(!self.fields.get("blockEvalPolicy").is_some_and(Value::truthy))
        }
    }
    fn has(&mut self, object: u32, property: &str) -> Result<bool, String> {
        match object {
            1 => Ok(matches!(property, "number" | "record" | "callback") || self.fields.contains_key(property)),
            2 => Ok(false),
            _ => Err("Unknown review object".into()),
        }
    }
    fn get(&mut self, object: u32, property: &str) -> Result<Value, String> {
        if object != 1 { return Err("Unknown review object".into()); }
        if property == "number" { return Ok(Value::Number(11.0)); }
        Ok(self.fields.get(property).cloned().unwrap_or(Value::Undefined))
    }
    fn set(&mut self, object: u32, property: &str, value: Value) -> Result<(), String> {
        if object != 1 { return Err("Unknown review object".into()); }
        self.fields.insert(property.into(), value);
        Ok(())
    }
    fn call(&mut self, object: u32, method: &str, args: &[Value]) -> Result<Value, String> {
        if object != 1 { return Err("Unknown review object".into()); }
        match method {
            "record" => {
                self.records.push(representation(args.first().unwrap_or(&Value::Undefined)));
                Ok(Value::Undefined)
            }
            "callback" => {
                self.callback = args.first().cloned();
                Ok(Value::Undefined)
            }
            _ => Err(format!("Unknown review method: {method}")),
        }
    }
}

fn main() {
    let mut source = String::new();
    io::stdin().read_to_string(&mut source).unwrap();
    let mut runtime = Runtime::new();
    runtime.define_global("test", Value::Host(1)).unwrap();
    let mut host = Recorder::default();
    let args: Vec<_> = std::env::args().skip(1).collect();
    let handler = args.iter().any(|arg| arg == "--handler");
    let browser = args.iter().any(|arg| arg == "--browser-global");
    if browser {
        for name in ["window", "self", "globalThis"] {
            runtime.define_global(name, Value::Host(2)).unwrap();
        }
        runtime.bind_browser_global(2).unwrap();
    }
    let mut result = if handler { runtime.compile_handler(&source) }
        else if browser {
            let mut result = Ok(Value::Undefined);
            for job in source.split("\n//---NEXT SCRIPT---\n") {
                runtime.begin_job();
                result = runtime.execute(job, &mut host);
                if result.is_err() { break; }
            }
            result
        } else { runtime.execute(&source, &mut host) };
    if result.is_ok() && args.iter().any(|arg| arg == "--invoke") {
        let callback = if handler { result.as_ref().ok().cloned() } else { host.callback.clone() };
        if let Some(callback) = callback {
            result = runtime.invoke(callback, &[Value::Number(5.0)], Value::Host(1), &mut host);
        } else {
            result = Err("No callback registered".into());
        }
    }
    match result {
        Ok(value) => println!("OK\t{}", representation(&value)),
        Err(error) => println!("ERROR\t{}", error.replace('\n', " ")),
    }
    for value in host.records { println!("RECORD\t{value}"); }
}
'''

NODE_DRIVER = r'''
const fs = require('fs');
const source = fs.readFileSync(0, 'utf8');
const records = [];
let browserWindow;
function repr(value) {
    if (value === undefined) return 'undefined';
    if (value === null) return 'null';
    if (typeof value === 'boolean') return 'boolean:' + value;
    if (typeof value === 'number') {
        if (Number.isNaN(value)) return 'number:NaN';
        const bytes = Buffer.alloc(8); bytes.writeDoubleBE(value);
        return 'number:' + bytes.toString('hex');
    }
    if (typeof value === 'string') return 'string:' + Buffer.from(value).toString('hex');
    if (typeof value === 'symbol') return 'symbol';
    if (typeof value === 'function') return 'function';
    if (value === test) return 'host:1';
    if (value === browserWindow) return 'host:2';
    return typeof value;
}
const test = {number:11, record:value=>records.push(repr(value)), callback:fn=>test.savedCallback=fn};
globalThis.test = test;
let ok = true, error = null, completion;
try {
    if (process.argv.includes('--handler')) {
        const handler = new Function('event', source);
        completion = handler;
        if (process.argv.includes('--invoke')) completion = handler.call(test, 5);
    } else if (process.argv.includes('--browser-global')) {
        const vm = require('vm');
        const sandbox = {test};
        sandbox.window = sandbox.self = sandbox.globalThis = sandbox;
        const context = vm.createContext(sandbox);
        browserWindow = vm.runInContext('window', context);
        for (const job of source.split('\n//---NEXT SCRIPT---\n')) completion = vm.runInContext(job, context);
    } else {
        completion = eval(source);
        if (process.argv.includes('--invoke')) completion = test.savedCallback.call(test,5);
    }
} catch (e) { ok = false; error = e.name + ': ' + e.message; }
console.log(JSON.stringify({ok, records, error, value: ok ? repr(completion) : null}));
'''

CASES = [
    ('precedence', 'test.record(2+3*4);test.record((2+3)*4);test.record(17%5);'),
    ('division', 'test.record(1/0);test.record(-1/0);test.record(0/0);test.record(-0);'),
    ('string-addition', 'test.record("a"+2+3);test.record(2+3+"a");test.record(null+1);test.record(undefined+1);'),
    ('loose-equality', 'test.record(null==undefined);test.record(null==0);test.record(""==0);test.record(false=="0");'),
    ('strict-equality', 'test.record(1===1);test.record(1==="1");test.record(0===-0);test.record((0/0)===(0/0));'),
    ('relational', 'test.record("10"<"2");test.record("10"<2);test.record((0/0)<2);test.record((0/0)>=2);'),
    ('logical-values', 'test.record(0||"fallback");test.record(""&&123);test.record("yes"&&42);test.record(null||false);'),
    ('short-circuit', 'let x=0;false&&(x=1);true||(x=2);test.record(x);'),
    ('ternary', 'let x=0;test.record(true?++x:++x);test.record(false?9:2+3*4);test.record(x);'),
    ('compound-update', 'let x=2;test.record(x++);test.record(++x);x*=3;x-=2;x/=5;x%=2;test.record(x);'),
    ('string-compound', 'let x="a";x+=2;test.record(x);'),
    ('nested-if', 'let x=0;if(true){if(false)x=1;else x=2;}test.record(x);'),
    ('while-break', 'let x=0;while(true){x++;if(x===4)break;}test.record(x);'),
    ('for-continue', 'let sum=0;for(let i=0;i<6;i++){if(i===3)continue;sum+=i;}test.record(sum);'),
    ('empty-for-body', 'let i=0;for(;i<4;i++);test.record(i);'),
    ('nested-loops', 'let n=0;for(let i=0;i<3;i++){for(let j=0;j<4;j++){if(j===2)break;n++;}}test.record(n);'),
    ('function-return', 'function f(x){if(x>2)return x*2;return 0;}test.record(f(4));test.record(f(1));'),
    ('function-hoist', 'test.record(f(4));function f(x){return x*x;}'),
    ('function-expression', 'let f=function(x){return x+1;};test.record(f(3));'),
    ('closure', 'function make(){let n=0;return function(){n++;return n;};}let c=make();test.record(c());test.record(c());'),
    ('closure-shadow', 'let x=1;function make(){let x=8;return function(){return ++x;};}let f=make();test.record(f());test.record(x);'),
    ('recursive-function', 'function fact(n){if(n<2)return 1;return n*fact(n-1);}test.record(fact(6));'),
    ('named-function-expression', 'let f=function sum(n){if(n===0)return 0;return n+sum(n-1);};test.record(f(4));'),
    ('let-block-scope', 'let x=3;{let x=7;test.record(x);}test.record(x);'),
    ('var-block-scope', 'var x=3;{var x=7;}test.record(x);'),
    ('var-hoisting', 'var x=8;function f(){test.record(x);var x=4;test.record(x);}f();'),
    ('closure-loop-let', 'let a;let b;for(let i=0;i<2;i++){if(i===0)a=function(){return i;};else b=function(){return i;};}test.record(a());test.record(b());'),
    ('host-values', 'test.value=3;test.value+=4;test.record(test.value);test.record(test.number);'),
    ('host-this-callback', 'test.callback(function(value){test.record(this===test);test.record(this.number+value);});', True),
    ('return-newline', 'function f(){return\n17;}test.record(f());'),
    ('asi-var', 'let x=2\nlet y=3\ntest.record(x+y)'),
    ('basic-escapes', 'test.record("a\\nb\\tc\\r\\\"d");test.record("\\u00e9");'),
    ('unicode-pair', 'test.record("\\uD83D\\uDE00");test.record("😀".length);'),
    ('string-length', 'test.record("abc".length);test.record("café😀".length);'),
    ('string-search', 'test.record("a😀bc😀".indexOf("b"));test.record("a😀bc😀".indexOf("😀",2));test.record("abc".includes("bc"));'),
    ('string-prefix-suffix', 'test.record("abc".startsWith("b",1));test.record("abc".endsWith("b",2));test.record("abc".endsWith(""));'),
    ('string-trim-case', 'test.record("  AbC  ".trim().toLowerCase());test.record("café".toUpperCase());'),
    ('number-coercion', 'test.record(Number(""));test.record(Number("  -1.5e2 "));test.record(Number(true));test.record(Number(null));'),
    ('number-radix', 'test.record(Number("0x10"));test.record(Number("0b11"));test.record(Number("0o10"));test.record(Number("-0x1"));'),
    ('number-special', 'test.record(Number("Infinity"));test.record(Number("infinity"));test.record(Number("1x"));'),
    ('boolean-string', 'test.record(Boolean("false"));test.record(Boolean(0));test.record(String(null));test.record(String(undefined));'),
    ('parse-int', 'test.record(parseInt("  -17px"));test.record(parseInt("11",2));test.record(parseInt("0xff"));test.record(parseInt("08"));'),
    ('parse-float', 'test.record(parseFloat(" -1.25e2tail"));test.record(parseFloat("0x10"));test.record(parseFloat("1e"));test.record(parseFloat("Infinity"));'),
    ('isnan', 'test.record(isNaN("no"));test.record(isNaN(""));test.record(isNaN(undefined));test.record(isNaN(null));'),
    ('const-mutation-error', 'const x=3;x=4;test.record(x);'),
    ('lexical-tdz', 'let x=8;{test.record(x);let x=4;}'),
    ('duplicate-let-error', 'let x=1;let x=2;test.record(x);'),
]

# Regressions found during independent review of the first interpreter pass.
CASES.extend([
    ('suffix-explicit-undefined', 'test.record("abc".endsWith("c",undefined));test.record("abc".endsWith("c",null));'),
    ('named-function-var-shadow', 'let f=function named(){var named=4;return named;};test.record(f());'),
    ('named-function-let-shadow', 'let f=function named(){let named=5;return named;};test.record(f());'),
    ('named-function-param-shadow', 'let f=function named(named){return named;};test.record(f(6));'),
    ('null-call-no-argument-effects', 'null.method(test.record("must not run"));'),
    ('handler-compile-only', 'test.record("must wait for click");return false;', False, True),
    ('handler-event-and-this', 'test.record(event);test.record(this.number);return false;', True, True),
])

# Original object/array implementation: compare identity, sparse storage,
# receiver binding, and evaluation order against a separate JavaScript engine.
CASES.extend([
    ('object-literals', 'let title="own";let key="x";let o={title, [key+1]:7, "two words":4, 3:9,};test.record(o.title);test.record(o.x1);test.record(o["two words"]);test.record(o[3]);test.record(o.missing);'),
    ('object-key-evaluation-order', 'let i=0;let o={[++i]:i++, [++i]:i++};test.record(i);test.record(o[1]);test.record(o[3]);'),
    ('object-identity-alias', 'let o={child:{value:1}};let alias=o;alias.child.value+=4;test.record(o===alias);test.record(o.child.value);test.record(o==={child:{value:5}});test.record(Boolean(o));test.record(typeof o);'),
    ('object-circular-identity', 'let a={};let b={parent:a};a.child=b;test.record(a.child.parent===a);test.record(a===a);test.record(Object(a)===a);'),
    ('object-key-order', 'let o={z:1,10:2,2:3,a:4, "01":5, 4294967295:6};o.z=7;o[1]=8;test.record(Object.keys(o).join("|"));'),
    ('object-duplicate-keys', 'let n=0;let o={a:++n,b:++n,a:++n};test.record(n);test.record(o.a);test.record(Object.keys(o).join());'),
    ('object-ordinary-method', 'let o={count:3,add(n){this.count+=n;return this.count;}};test.record(o.add(4));test.record(o["add"](2));test.record(o.count);'),
    ('object-computed-method', 'let name="add";let o={count:5,[name](n){return this.count+n;}};test.record(o.add(8));'),
    ('object-method-callee-snapshot', 'let o={method:function(x){return "first:"+x;}};function change(){o.method=function(){return "second";};return 3;}test.record(o.method(change()));test.record(o.method());'),
    ('object-missing-method-args', 'let o={};function add(){test.record("argument");o.f=function(){return 4;};}o.f(add());'),
    ('object-own-method-shadow', 'let o={toString(){return "own";},hasOwnProperty(){return "override";}};test.record(o.toString());test.record(o.hasOwnProperty("x"));'),
    ('object-own-undefined-shadow', 'let o={toString:undefined};o.toString(test.record("argument"));'),
    ('object-own-property', 'let o={x:undefined};test.record(o.hasOwnProperty("x"));test.record(o.hasOwnProperty("toString"));test.record(o.hasOwnProperty("absent"));test.record(o.valueOf()===o);test.record(o.toString());'),
    ('object-property-update-order', 'let i=0;let o={0:4};test.record(o[i++]++);test.record(i);test.record(o[0]);o[0]*=2;test.record(o[0]);'),
    ('array-literals-and-holes', 'let a=[1,,undefined,null,];test.record(a.length);test.record(a[1]);test.record(a.hasOwnProperty(1));test.record(a.hasOwnProperty(2));test.record(a.hasOwnProperty("length"));test.record(Object.keys(a).join());test.record(a.join("|"));test.record([,].length);test.record([,,].length);test.record([1,].length);'),
    ('array-alias-and-index-growth', 'let a=[];let b=a;b[3]="four";test.record(a===b);test.record(a.length);test.record(a.join());test.record(Object.keys(a).join());a["01"]="named";a["4294967295"]="named2";test.record(a.length);test.record(Object.keys(a).join("|"));'),
    ('array-length-truncate-grow', 'let a=[1,2,3];a.note=4;a.length=1;test.record(a[2]);test.record(a.length);a.length="4";test.record(a.join());test.record(a.hasOwnProperty(2));test.record(a.note);test.record(Object.keys(a).join());'),
    ('array-push-pop', 'let a=[1,,3];test.record(a.push(4,5));test.record(a.pop());test.record(a.pop());test.record(a.pop());test.record(a.pop());test.record(a.pop());test.record(a.pop());test.record(a.length);test.record(a.push());'),
    ('array-slice-sparse-alias', 'let child={n:2};let a=[child,,3,4];let b=a.slice(0,3);test.record(b.length);test.record(b.hasOwnProperty(1));test.record(b[0]===child);b[0].n=7;test.record(a[0].n);test.record(a.slice(-2).join());test.record(a.slice(3,1).length);test.record(a.slice(0,undefined).length);'),
    ('array-slice-conversion', 'let a=[1,2,3];test.record(a.slice(-99,Infinity).join());test.record(a.slice(NaN,1.9).join());test.record(a.slice(Infinity).length);test.record(a.slice(-Infinity,-1).join());'),
    ('array-indexof-includes', 'let a=[,NaN,undefined,3,3];test.record(a.indexOf(undefined));test.record(a.includes(undefined));test.record(a.indexOf(NaN));test.record(a.includes(NaN));test.record(a.indexOf(3,-1));test.record(a.includes(3,Infinity));test.record(a.indexOf(3,-Infinity));'),
    ('array-indexof-identity', 'let o={};let a=[o];test.record(a.indexOf(o));test.record(a.indexOf({}));test.record(a.includes(o));test.record(a.includes({}));'),
    ('array-nested-join', 'let a=[1,[2,,3],null,undefined,{},false];test.record(a.join("/"));test.record(a.toString());test.record([].join());'),
    ('array-circular-join', 'let a=[1];a.push(a);a.push(3);test.record(a.join("|"));let b=["b"];let c=["c",b];b.push(c);test.record(b.join());'),
    ('array-intrinsic-callee-snapshot', 'let a=[];function replace(){a.push=function(){return 99;};return 7;}test.record(a.push(replace()));test.record(a[0]);test.record(a.push());'),
    ('array-custom-method', 'let a=[1,2];a.total=function(){return this[0]+this[1];};test.record(a.total());test.record(Array.isArray(a));test.record(Object.keys(a).join());'),
    ('array-tostring-own-join', 'let a=[1,2];a.join=function(){return "custom:"+this.length;};test.record(a.toString());a.join=null;test.record(a.toString());'),
    ('object-array-factories', 'let o=Object();let a=Array(3);test.record(Object(null).toString());test.record(Array.isArray(o));test.record(Array.isArray(a));test.record(Array.isArray(null));test.record(a.length);test.record(a.hasOwnProperty(0));test.record(Array("3").join());test.record(Array(1,2,3).join());test.record(Object(a)===a);'),
    ('function-call-object-receiver', 'let o={n:7};function f(x,y){return this.n+x+y;}test.record(f.call(o,2,3));test.record(f.apply(o,[4,5]));'),
    ('function-call-strict-this', 'function f(){"use strict";return this;}test.record(f.call(null));test.record(f.call(undefined));test.record(f.call(3));test.record(f.apply("text",null));'),
    ('function-apply-arraylike', 'function f(x,y,z){return x+":"+y+":"+z;}test.record(f.apply(null,{0:"a",2:"c",length:3}));test.record(f.apply(null,{0:"a",length:1.8}));test.record(f.apply(null,{}));test.record(f.apply(null,undefined));'),
    ('function-apply-holes', 'function f(a,b,c){test.record(a);test.record(b);test.record(c);}f.apply(null,[1,,3]);'),
    ('function-call-host-this', 'function f(n){return this.number+n;}test.record(f.call(test,5));'),
    ('function-call-builtin', 'test.record(Number.call(null,"7"));test.record(String.apply(null,[42]));test.record(Array.call(null,1,2).join());'),
    ('object-retained-callback', 'let state={n:1,values:[2,3]};test.callback(function(n){state.n+=n;state.values.push(state.n);test.record(state.values.join());});', True),
    ('invalid-array-length', 'let a=[1,2];a.length=-1;test.record(a.length);'),
    ('invalid-array-factory', 'Array(1.5);'),
    ('noncallable-own-property', 'let o={f:2};o.f(test.record("argument"));'),
])
CASES.extend([
    ('throw-primitive-catch', 'try{throw "caught";}catch(e){test.record(e);}test.record("after");'),
    ('throw-null-undefined', 'try{throw null;}catch(e){test.record(e);}try{throw undefined;}catch(e){test.record(e);}'),
    ('throw-object-identity', 'let o={n:3};try{throw o;}catch(e){test.record(e===o);e.n++;}test.record(o.n);'),
    ('throw-host-identity', 'try{throw test;}catch(e){test.record(e===test);}'),
    ('throw-across-functions', 'function f(){throw 7;}function g(){return f();}try{g();}catch(e){test.record(e);}'),
    ('catch-lexical-scope', 'let e=1;let f;try{throw 2;}catch(e){f=()=>e;{let e=3;test.record(e);}}test.record(f());test.record(e);'),
    ('catch-var-initializer', 'try{throw 1;}catch(e){var e=2;test.record(e);}test.record(e);'),
    ('catch-optional-binding', 'try{throw 9;}catch{test.record("handled");}'),
    ('try-var-hoisting', 'function f(){test.record(x);test.record(y);test.record(z);try{var x=1;throw 4;}catch(e){var y=e;}finally{var z=3;}test.record(x+y+z);}f();'),
    ('finally-return-preserves', 'function f(){try{return 1;}finally{test.record("finally");}}test.record(f());'),
    ('finally-return-replaces', 'function f(){try{return 1;}finally{return 2;}}test.record(f());'),
    ('finally-throw-replaces', 'try{try{throw 1;}finally{throw 2;}}catch(e){test.record(e);}'),
    ('finally-return-replaces-throw', 'function f(){try{throw 1;}finally{return 2;}}test.record(f());'),
    ('finally-catch-throws', 'try{try{throw 1;}catch(e){throw e+1;}finally{test.record("cleanup");}}catch(e){test.record(e);}'),
    ('finally-nested-order', 'try{try{throw 1;}finally{test.record("inner");}}catch(e){test.record(e);}finally{test.record("outer");}'),
    ('finally-loop-control', 'for(let i=0;i<4;i++){try{if(i===0)continue;if(i===2)break;test.record(i);}finally{test.record("f"+i);}}'),
    ('finally-continue-over-return', 'function f(){for(let i=0;i<2;i++){try{return 7;}finally{if(i===0)continue;}}}test.record(f());'),
    ('finally-break-over-throw', 'for(let i=0;i<2;i++){try{throw 4;}finally{break;}}test.record("done");'),
    ('catch-reference-error', 'try{missingName;}catch(e){test.record(e.name);test.record(typeof e.message);}'),
    ('catch-null-type-error', 'try{null.field;}catch(e){test.record(e.name);}try{undefined.method();}catch(e){test.record(e.name);}'),
    ('catch-noncallable-type-error', 'let o={f:2};try{o.f(test.record("arg"));}catch(e){test.record(e.name);}'),
    ('catch-tdz-const-errors', 'try{let n=n;}catch(e){test.record(e.name);}const n=1;try{n=2;}catch(e){test.record(e.name);}test.record(n);'),
    ('catch-array-range-error', 'let a=[1];try{a.length=-1;}catch(e){test.record(e.name);}test.record(a.length);try{Array(1.5);}catch(e){test.record(e.name);}'),
    ('error-values', 'let e=TypeError("bad");test.record(e.name);test.record(e.message);test.record(e.toString());test.record(Object.keys(e).join());e.message="changed";test.record(Object.keys(e).join());test.record(e.toString());'),
    ('error-defaults', 'let e=Error();test.record(e.name);test.record(e.message);test.record(e.hasOwnProperty("message"));test.record(e.hasOwnProperty("name"));test.record(e.toString());e.name="";test.record(e.toString());'),
    ('arrow-single-expression', 'let double=x=>x*2;test.record(double(4));'),
    ('arrow-empty-multi-parameters', 'let f=()=>7;let sum=(a,b,)=>a+b;test.record(f());test.record(sum(2,3));'),
    ('arrow-object-and-block', 'let f=n=>({n:n});let g=(n)=>{let v=f(n);return v.n+1;};test.record(g(4));'),
    ('arrow-nested-closures', 'let f=a=>b=>a+b;test.record(f(3)(4));'),
    ('arrow-lexical-this', 'let o={n:7,make:function(){return ()=>this.n;}};let f=o.make();test.record(f());test.record(f.call({n:9}));test.record(f.apply({n:12},[]));'),
    ('arrow-lexical-primitive-this', 'function make(){"use strict";return ()=>this;}let f=make.call(7);test.record(f.call(9));test.record(make.call(null)());'),
    ('arrow-host-lexical-this', 'test.callback(function(n){let f=()=>this.number+n;test.record(f.call(null));});', True),
    ('arrow-loop-closures', 'let a=[];for(let i=0;i<3;i++){a.push(()=>i);}test.record(a[0]());test.record(a[2]());'),
    ('uri-component-encoding', 'test.record(encodeURIComponent("a b/c?x=1&y=é😀"));test.record(encodeURIComponent("-_.!~*\'()"));test.record(encodeURIComponent());test.record(encodeURIComponent(null));'),
    ('uri-encoding', 'test.record(encodeURI("https://example.com/a b?q=é#😀"));test.record(encodeURI(";/?:@&=+$,#[]%"));'),
    ('uri-decoding', 'test.record(decodeURIComponent("a%20b%2fc%3Fq%3D%C3%A9%F0%9F%98%80"));test.record(decodeURI("a%20b%2fc%3Fq%3D%C3%A9%F0%9F%98%80"));test.record(decodeURIComponent("a+b"));test.record(decodeURI("%25%00"));'),
    ('uri-malformed-catch', 'let a=["%","%2","%GG","%C0%AF","%ED%A0%80","%F4%90%80%80","%E2%82","%80"];for(let i=0;i<a.length;i++){try{decodeURIComponent(a[i]);}catch(e){test.record(e.name);}}'),
])
CASES.extend([
    ('sequence-evaluation-order', 'let n=0;test.record((n=1,n+=2,n*3));test.record(n);'),
    ('sequence-return-and-throw', 'function f(){return 1,2;}test.record(f());try{throw 1,3;}catch(e){test.record(e);}'),
    ('sequence-argument-boundaries', 'function f(a,b){return a+":"+b;}test.record(f((1,2),3));let a=[(1,2),3];test.record(a.join());'),
    ('sequence-declaration-and-object-boundaries', 'let a=(1,2),b=3;let o={a:(4,5),b:6,[(7,8)]:9};test.record(a+b);test.record(o.a+o.b+o[8]);'),
    ('sequence-loop-initializer-and-advance', 'let i,j;for(i=0,j=3;i<3;i++,j+=2){test.record(i+j);}test.record(j);'),
    ('sequence-conditional', 'let x=0;test.record(true?(x++,4):(x++,5));test.record((false?4:5,6));test.record(x);'),
    ('sequence-member-key', 'let o={b:7};test.record(o["a","b"]);'),
    ('sequence-loses-method-receiver', 'let o={f:function(){"use strict";return this;}};test.record((0,o.f)());test.record(o.f()===o);'),
    ('sequence-arrow-body', 'let f=()=>1,g=()=>2;test.record(f()+g());let h=()=> (1,3);test.record(h());'),
])

# Isolated classic-script globals in a Node vm context provide an independent
# reference for Window aliases and declarations, including multiple scripts.
# The recorder is an opaque host object in both implementations.
CASES.extend([
    ('regex-test-and-properties', 'let r=/ab+/gims;test.record(r.test("ABBB"));test.record(r.lastIndex);test.record(r.source);test.record(r.flags);test.record(r.global);test.record(r.ignoreCase);test.record(r.multiline);test.record(r.dotAll);test.record(r.unicode);test.record(r.toString());'),
    ('regex-first-match', 'let m=/a(b+)c/.exec("xxabbbczz");test.record(m[0]);test.record(m[1]);test.record(m.index);test.record(m.input);test.record(m.groups);test.record(m.length);'),
    ('regex-leftmost-alternation', 'test.record(/a|ab/.exec("ab")[0]);test.record(/ab|a/.exec("ab")[0]);test.record(/b|a/.exec("a b").index);'),
    ('regex-greedy-and-lazy', 'test.record(/a.*b/.exec("a1b2b")[0]);test.record(/a.*?b/.exec("a1b2b")[0]);test.record(/a.+?b/.exec("ab1b")[0]);'),
    ('regex-backtrack', 'let m=/(a+)(a)/.exec("aaaa");test.record(m[1]);test.record(m[2]);test.record(/a{2,4}a/.exec("aaaaa")[0]);'),
    ('regex-counted', 'test.record(/a{2}/.exec("aaa")[0]);test.record(/a{2,}/.exec("aaaa")[0]);test.record(/a{0,2}?/.exec("aa")[0]);test.record(/a{0}/.test("b"));'),
    ('regex-grouped-counted', 'test.record(/(?:ab){2,3}/.exec("abababab")[0]);test.record(/(?:ab){2,3}?/.exec("abababab")[0]);'),
    ('regex-optional', 'let m=/(a)?b/.exec("b");test.record(m[0]);test.record(m[1]);test.record(m.length);'),
    ('regex-repeated-captures-reset', 'let m=/(a|(b))+/.exec("ba");test.record(m[0]);test.record(m[1]);test.record(m[2]);'),
    ('regex-nested-capture-reset', 'let m=/(z)((a+)?(b+)?(c))*/.exec("zaacbbbcac");test.record(m[0]);test.record(m[1]);test.record(m[2]);test.record(m[3]);test.record(m[4]);test.record(m[5]);'),
    ('regex-empty-repetition', 'test.record(/(a?)*/.exec("")[1]);test.record(/(a?)+/.exec("")[1]);test.record(/(?:)*/.exec("")[0]);test.record(/(a?){0,2}/.exec("")[1]);test.record(/(a?){1,2}/.exec("")[1]);'),
    ('regex-global-state', 'let r=/a/g;test.record(r.test("aba"));test.record(r.lastIndex);test.record(r.exec("aba").index);test.record(r.lastIndex);test.record(r.test("aba"));test.record(r.lastIndex);'),
    ('regex-global-empty-exec', 'let r=/(?:)/g;test.record(r.exec("ab").index);test.record(r.lastIndex);test.record(r.exec("ab").index);'),
    ('regex-last-index-conversion', 'let r=/a/g;r.lastIndex="1.9";test.record(r.exec("ba").index);r.lastIndex=-4;test.record(r.exec("a").index);r.lastIndex=Infinity;test.record(r.test("a"));test.record(r.lastIndex);'),
    ('regex-nonglobal-index', 'let r=/a/;r.lastIndex=99;test.record(r.exec("ba").index);test.record(r.lastIndex);'),
    ('regex-sticky', 'let r=/a/y;test.record(r.test("ba"));test.record(r.lastIndex);r.lastIndex=1;test.record(r.test("ba"));test.record(r.lastIndex);'),
    ('regex-sticky-global-match', 'test.record("aa ba".match(/a/gy).join(","));'),
    ('regex-character-classes', r'test.record(/[a-c]+/.exec("xxabcaz")[0]);test.record(/[^a-c]+/.exec("abcXYZ")[0]);test.record(/[\d_]+/.exec("x12_3z")[0]);'),
    ('regex-empty-classes', 'test.record(/[]/.test("x"));test.record(/[^]/.test("\\n"));test.record(/[^]/.test(""));'),
    ('regex-class-escapes', r'test.record(/\d+\D\w+\W\s+\S/.test("12:ab! \tX"));test.record(/[\D]+/.exec("1ab2")[0]);test.record(/[\S]+/.exec(" a ")[0]);'),
    ('regex-class-hyphen-bracket-slash', r'test.record(/[-a]+/.test("-a"));test.record(/[a-]+/.test("a-"));test.record(/[\]\/]+/.test("]/"));'),
    ('regex-case-insensitive-range', 'test.record(/[a-z]+/i.exec("ABC")[0]);test.record(/[Z-a]+/i.exec("Az")[0]);test.record(/[^a-z]+/i.exec("ABC123")[0]);'),
    ('regex-word-boundary', r'test.record(/\bcat\b/.test("a cat!"));test.record(/\bcat\b/.test("cats"));test.record(/\Bcat/.test("scat"));test.record(/\b/.test(""));'),
    ('regex-line-anchors', r'test.record(/^b$/m.exec("a\nb\nc").index);test.record(/a$/.test("a\n"));test.record(/^b/.test("a\nb"));'),
    ('regex-dotall-and-line-separators', r'test.record(/a.b/.test("a\nb"));test.record(/a.b/s.test("a\nb"));test.record(/^b/m.exec("a\u2028b").index);'),
    ('regex-hex-unicode-control', r'test.record(/\x61\u0062\cC/.test("ab\u0003"));test.record(/[\b]/.test("\b"));test.record(/\0/.test("\0"));'),
    ('regex-utf16-index', 'let m=/x/.exec("😀x");test.record(m.index);test.record(m.input);test.record(/../.test("😀"));test.record(/😀/.test("😀"));'),
    ('regex-factory', 'let r=RegExp("a+","gi");test.record(r.test("AA"));test.record(r.flags);test.record(RegExp(r)===r);let copy=RegExp(r,"m");test.record(copy===r);test.record(copy.flags);test.record(copy.lastIndex);'),
    ('regex-factory-source', r'test.record(RegExp().source);test.record(RegExp("a/b\nc").source);test.record(/a\/b/.source);'),
    ('regex-factory-syntax-catch', 'try{RegExp("[");}catch(e){test.record(e.name);}try{RegExp("a","gg");}catch(e){test.record(e.name);}try{RegExp("a","z");}catch(e){test.record(e.name);}'),
    ('regex-own-properties', 'let r=/a/;test.record(Object.keys(r).join());test.record(r.hasOwnProperty("lastIndex"));let m=r.exec("a");test.record(Object.keys(m).join());'),
    ('regex-literal-fresh-identity', 'function make(){return /a/g;}let a=make(),b=make();test.record(a===b);a.lastIndex=8;test.record(b.lastIndex);'),
    ('regex-search-preserves-index', 'let r=/b/g;r.lastIndex=9;test.record("aba".search(r));test.record(r.lastIndex);test.record("a".search(/z/));test.record("abc".search());test.record("abc".search("b"));'),
    ('regex-search-sticky', 'test.record("ba".search(/a/y));test.record("ab".search(/a/y));'),
    ('regex-match-global', 'let r=/a+/g;r.lastIndex=8;let m="aa ba".match(r);test.record(m.join("|"));test.record(m.index);test.record(r.lastIndex);test.record("bb".match(r));'),
    ('regex-match-captures', 'let m="xa12".match(/a(\u005cd+)/);test.record(m[0]);test.record(m[1]);test.record(m.index);'),
    ('regex-match-empty', 'test.record("ab".match(/(?:)/g).join("|"));test.record("".match(/(?:)/g).length);'),
    ('regex-replace-global', 'test.record("a+b+c".replace(/\u005c+/g," "));test.record("aaa".replace(/a/,"x"));test.record("aaa".replace(/a/g,"x"));'),
    ('regex-replace-captures', 'test.record("abc123def".replace(/([a-z]+)([0-9]+)/,"$2-$1-$$-$&"));'),
    ('regex-replace-prefix-suffix', 'test.record("abc".replace(/b/,"$`:$&:$\u0027"));'),
    ('regex-replace-undefined-capture', 'test.record("b".replace(/(a)?b/,"x$1y"));'),
    ('regex-replace-numbered-tokens', 'test.record("ab".replace(/(a)(b)/,"$0:$01:$10:$20:$3:$99"));'),
    ('regex-replace-empty-progress', 'test.record("ab".replace(/(?:)/g,"-"));test.record("".replace(/(?:)/g,"-"));'),
    ('regex-replace-state', 'let r=/a/g;r.lastIndex=8;test.record("aba".replace(r,"x"));test.record(r.lastIndex);let s=/b/y;s.lastIndex=1;test.record("abc".replace(s,"x"));test.record(s.lastIndex);'),
    ('string-replace-literal', 'test.record("a.b.c".replace(".","-"));test.record("abc".replace("b","[$&]"));test.record("abc".replace("","x"));test.record("abc".replace("z","x"));'),
    ('regex-division-goals', 'let r=/a/;let x=12/3/2;x/=2;test.record(x);test.record(r.test("a"));test.record(({}).x);test.record((function(){return 8;})()/2);'),
    ('regex-after-controls', 'if(true)/a/.test("a")&&test.record(1);if(false){}else /b/.test("b")&&test.record(2);while(false)/c/.test("c");for(let i=0;i<1;i++)/d/.test("d")&&test.record(3);'),
    ('regex-after-statement-block', '{} /a/.test("a")&&test.record(1);function f(){} /b/.test("b")&&test.record(2);try{}catch(e){} /c/.test("c")&&test.record(3);'),
    ('regex-division-keyword-properties', 'let o={if:()=>8,catch:()=>10,while:()=>12,for:()=>14,function:()=>16};test.record(o.if()/2);test.record(o.catch()/2);test.record(o.while()/2);test.record(o.for()/2);test.record(o.function()/2);'),
    ('regex-after-prefix-newline-update', 'let a=2;a\n++/x/.lastIndex;test.record(a);'),
    ('regex-after-asi-function-block', 'let a=2;a\nfunction f(){return 3;} /x/.test("x")&&test.record(f());a\n{} /y/.test("y")&&test.record(a);'),
    ('regex-function-expression-division', 'let f=function(){return 4;};test.record(f()/2);if(false){let n=function(){return 8;}/2;}test.record(3);'),
    ('regex-return-and-arrow', 'function f(){return /x/;}let g=()=>/y/;test.record(f().test("x"));test.record(g().test("y"));let h=()=>({r:/z/});test.record(h().r.test("z"));'),
    ('regex-comments-and-escapes', r'let x=8/*comment*//2;test.record(x);test.record(/[/]/.test("/"));test.record(/\/\//.test("//"));test.record(/\/*/.test("///"));'),
    ('large-language-string', 'let text="' + 'a'*40000 + '";test.record(text.length);test.record(text+"b"===text+"b");'),
    ('regex-large-simple-repetition', 'let text="' + 'a'*20000 + '";test.record(/^a+$/.test(text));test.record(/a+?b/.test("aaab"));'),
])

BROWSER_CASES = [
    ('global-expando-alias', 'window.google={n:1};test.record(google===window.google);google.n++;test.record(self.google.n);globalThis.google.n++;test.record(google.n);'),
    ('global-existing-assignment', 'window.answer=1;answer+=4;test.record(window.answer);answer++;test.record(globalThis.answer);'),
    ('global-var-properties', 'var n=4;test.record(window.n);window.n=7;test.record(n);function add(n){return n+1;}test.record(window.add(4));'),
    ('global-lexical-separate', 'window.value=3;let value=8;const fixed=9;test.record(value);test.record(window.value);test.record(window.fixed);window.value++;test.record(value);test.record(window.value);'),
    ('global-lexical-tdz', 'window.value=3;try{test.record(value);}catch(e){test.record(e.name);}let value=8;test.record(window.value);'),
    ('global-undefined-detection', 'test.record(typeof neverDefined);test.record(window.neverDefined);try{neverDefined;}catch(e){test.record(e.name);}window.defined=undefined;test.record(defined);'),
    ('global-function-callee-snapshot', 'window.f=function(n){return "first:"+n;};function change(){window.f=function(){return "second";};return 4;}test.record(window.f(change()));test.record(f());'),
    ('global-this-and-arrow', 'test.record(this===window);let f=()=>this;test.record(f());test.record(f.call({}));window.f=function(){return this===window;};test.record(window.f());'),
    ('global-builtin-aliases', 'test.record(window.parseInt===parseInt);test.record(window.encodeURIComponent("a b"));test.record(window.undefined);test.record(window.NaN);test.record(window.Infinity);'),
    ('global-var-hoist-from-expando-across-scripts', 'window.existing=5;\n//---NEXT SCRIPT---\nvar existing;test.record(existing);test.record(window.existing);existing++;test.record(window.existing);'),
    ('global-function-over-expando-across-scripts', 'window.f=4;\n//---NEXT SCRIPT---\ntest.record(window.f());function f(){return 7;}test.record(f===window.f);'),
    ('global-lexical-over-expando-across-scripts', 'window.existing=5;\n//---NEXT SCRIPT---\nlet existing=8;test.record(existing);test.record(window.existing);'),
    ('global-closure-across-scripts', 'window.state={n:3};var increment=()=>++state.n;\n//---NEXT SCRIPT---\ntest.record(increment());test.record(window.increment());'),
]

CASES.extend([
    ('in-own-values', 'let o={yes:undefined,no:false,zero:0,empty:""};test.record("yes" in o);test.record("no" in o);test.record("zero" in o);test.record("empty" in o);test.record("missing" in o);'),
    ('in-key-conversion', 'let o={undefined:1,null:2,true:3,"0":4,"NaN":5};test.record(undefined in o);test.record(null in o);test.record(true in o);test.record(-0 in o);test.record(NaN in o);'),
    ('in-precedence', 'let o={2:1,x:2};test.record(1<<1 in o === true);test.record("x" in o && !("y" in o));test.record("x" in o ? 7 : 8);'),
    ('in-order-and-primitive-right', 'let order="";function left(){order+="L";return "x";}function right(){order+="R";return {x:0};}test.record(left() in right());test.record(order);try{"length" in "abc";}catch(e){test.record(e.name);}try{"x" in null;}catch(e){test.record(e.name);}try{"x" in 4;}catch(e){test.record(e.name);}'),
    ('in-array-holes-and-builtins', 'let a=[,undefined];test.record(0 in a);test.record(1 in a);test.record("length" in a);test.record("push" in a);test.record("toString" in a);a.length=0;test.record(1 in a);'),
    ('in-regexp-error-methods', 'let r=/a/g,e=Error("problem");test.record("lastIndex" in r);test.record("source" in r);test.record("exec" in r);test.record("message" in e);test.record("name" in e);test.record("hasOwnProperty" in {});'),
    ('in-known-function-members', 'test.record("call" in function(){});test.record("apply" in parseInt);test.record("keys" in Object);test.record("isArray" in Array);test.record("missing" in Object);'),
    ('in-host-presence-with-undefined', 'test.own=undefined;test.record("own" in test);test.record("number" in test);test.record("missing" in test);'),
    ('for-in-object-key-order', 'let o={b:1,"10":2,a:3,"2":4,"01":5,"4294967295":6,"4294967294":7},s="";for(let k in o){s+=k+":";}test.record(s);'),
    ('for-in-array-sparse', 'let a=[10,,undefined,40];a.extra=5;let s="";for(var k in a){s+=k+":"+a[k]+";";}test.record(s);test.record(k);'),
    ('for-in-strings-primitives', 'let s="";for(const k in "A💡Z"){s+=k;}for(let k in null){s+="bad";}for(let k in undefined){s+="bad";}for(let k in 42){s+="bad";}for(let k in false){s+="bad";}for(let k in function(){}){s+="bad";}test.record(s);'),
    ('for-in-let-closures', 'let f=[];for(let k in {a:1,b:2,c:3}){f.push(()=>k);}test.record(f[0]());test.record(f[1]());test.record(f[2]());test.record(typeof k);'),
    ('for-in-const-closures', 'let f=[];for(const k in {a:1,b:2}){f.push(function(){return k;});try{k="no";}catch(e){test.record(e.name);}}test.record(f[0]());test.record(f[1]());'),
    ('for-in-var-hoist-closures', 'test.record(typeof k);let f=[];for(var k in {a:1,b:2}){f.push(()=>k);}test.record(k);test.record(f[0]());test.record(f[1]());'),
    ('for-in-lexical-right-tdz', 'let x={a:1};try{for(let x in x){}}catch(e){test.record(e.name);}test.record(x.a);'),
    ('for-in-lexical-body-shadow', 'let k="outer";for(const k in {a:1,b:2}){{let k="nested";test.record(k);}test.record(k);}test.record(k);'),
    ('for-in-catch-var-binding', 'try{throw "old";}catch(k){for(var k in {a:1,b:2}){}test.record(k);}test.record(k);'),
    ('for-in-reference-evaluation', 'let n=0,o={},s="";function sourceObject(){s+="R";return {a:1,b:2};}function index(){s+="L";return n++;}for(o[index()] in sourceObject()){s+="B";}test.record(s);test.record(o[0]);test.record(o[1]);test.record(n);'),
    ('for-in-empty-does-not-write', 'const k="keep";for(k in {}){}test.record(k);let n=0,o={};for(o[n++] in null){}test.record(n);'),
    ('for-in-deletion-and-addition', 'let a=[1,2,3,4],s="";for(let k in a){s+=k;a.length=2;a.extra=9;}test.record(s);test.record(a.extra);'),
    ('for-in-current-values', 'let o={a:1,b:2},s="";for(let k in o){o.b=7;o.c=8;s+=k+o[k];}test.record(s);'),
    ('for-in-break-continue-finally', 'let s="";for(let k in {a:1,b:2,c:3}){try{if(k==="a")continue;if(k==="c")break;s+=k;}finally{s+="F";}}test.record(s);'),
    ('for-in-classic-head-parentheses', 'let o={x:1};for(let yes=("x" in o);yes;yes=false){test.record(yes);}for(var yes=true ? "x" in o : false;yes;yes=false){test.record(yes);}'),
    ('for-in-classic-head-containers', 'let o={x:1};for(let a=["x" in o];a.length;a.length=0){test.record(a[0]);}for(let a={x:"x" in o};a.x;a.x=false){test.record(a.x);}'),
    ('for-in-classic-head-call-and-key', 'let o={x:1};function id(x){return x;}for(var y=id("x" in o);y;y=false){test.record(y);}for(let y=o["x" in o ? "x":"none"];y;y=0){test.record(y);}'),
    ('for-in-classic-head-function-and-arrow', 'for(var f=x=>("a" in {a:1});;){test.record(f());break;}for(var f=x=>{return "a" in {a:1};};;){test.record(f());break;}for(var f=function(){return "a" in {a:1};};;){test.record(f());break;}'),
    ('for-in-parenthesized-assignment-target', 'let k,s="";for((k) in {a:1,b:2})s+=k;test.record(s);'),
    ('labelled-block-break', 'let s="";exit:{s+="a";{s+="b";break exit;}s+="bad";}test.record(s);'),
    ('labelled-if-break', 'let s="";exit:if(true){s+="yes";break exit;s+="bad";}test.record(s);'),
    ('labelled-loop-chain', 'let s="";outer:inner:for(let i=0;i<4;i++){if(i===1)continue outer;if(i===3)break inner;s+=i;}test.record(s);'),
    ('labelled-outer-continue', 'let s="";outer:for(let i=0;i<3;i++){for(let j=0;j<3;j++){s+=i+":"+j+";";if(j===1)continue outer;}}test.record(s);'),
    ('labelled-for-in-continue', 'let s="";outer:for(const a in {a:1,b:2}){for(const b in {x:1,y:2}){s+=a+b;continue outer;}}test.record(s);'),
    ('labelled-while-break', 'let s="",n=0;outer:while(n<5){n++;while(true){s+=n;if(n===3)break outer;break;}}test.record(s);'),
    ('labelled-finally-replaces-continue', 'let s="";outer:for(let i=0;i<3;i++){try{s+=i;continue outer;}finally{if(i===1)break outer;s+="f";}}test.record(s);'),
    ('labelled-finally-replaces-break', 'let s="",n=0;outer:while(n<3){n++;try{break outer;}finally{s+=n;continue outer;}}test.record(s);'),
    ('labelled-break-through-finally', 'let s="";exit:{try{s+="t";break exit;}finally{s+="f";}s+="bad";}test.record(s);'),
    ('label-reuse-after-scope', 'let s="";same:{s+="a";break same;}same:{s+="b";break same;}function f(){same:{return "c";}}test.record(s+f());'),
    ('label-regex-after-block', 'label:{}test.record(/a/.test("a"));other: /b/.test("b");test.record((true?{x:6}:{x:2}).x/2);'),
    ('label-regex-after-automatic-semicolon', 'let x=1\nlabel:{} /a/.test("a");test.record(x);let o={value:true\n? {x:6}\n: {x:2}};test.record(o.value.x/2);'),
    ('label-regex-after-jump-newline', 'outer:{while(true){break outer\n/a/.test("a");}}test.record("ok");let n=0;loop:while(n++<1){continue loop\n/a/.test("a");}test.record(n);'),
    ('label-regex-after-return-newline', 'function f(){return\nouter:{} /a/.test("a");}function g(){return\n{} /a/.test("a");}function h(){return/*\n*/outer:{} /a/.test("a");}test.record(f());test.record(g());test.record(h());'),
    ('label-break-newline-no-target', 'let n=0;outer:while(true){n++;break\nouter;}test.record(n);'),
    ('label-function-boundary-valid', 'let s="";outer:{function f(){outer:{return 7;}}s+=f();break outer;}test.record(s);'),
])

BROWSER_CASES.extend([
    ('global-in-ordinary-and-lexical', 'var own=undefined;let lexical=7;window.expando=undefined;test.record("own" in window);test.record("expando" in window);test.record("lexical" in window);test.record("missing" in window);test.record("parseInt" in window);test.record("undefined" in window);'),
    ('global-for-in-var-hoist', 'test.record("key" in window);for(var key in {a:1,b:2}){}test.record(window.key);'),
])

COMPLETION_CASES = [
    ('label-empty-completion', '7;label:{break label;}'),
    ('label-value-completion', '7;label:{9;break label;}'),
    ('label-inner-loop-value-completion', '7;label:{while(true){8;break label;}}'),
    ('label-inner-loop-empty-completion', '7;label:{while(true){break label;}}'),
    ('label-if-break-completion', '7;label:{9;if(true)break label;}'),
    ('label-conditional-empty-completion', '7;label:if(true){break label;}'),
    ('label-chain-completion', '7;outer:inner:{9;break outer;}'),
    ('label-chain-inner-completion', '7;outer:inner:{9;break inner;}'),
    ('label-for-continue-completion', '7;outer:for(let i=0;i<3;i++){i;continue outer;}'),
    ('label-for-finally-completion', '7;outer:for(let i=0;i<3;i++){try{i;break outer;}finally{99;}}'),
    ('label-finally-break-completion', '7;label:{try{8;}finally{break label;}}'),
    ('for-in-empty-completion', '7;for(var k in {}){}'),
    ('for-in-key-completion', '7;for(var k in {a:1,b:2}){k;}'),
    ('for-in-break-value-completion', '7;for(var k in {a:1}){9;break;}'),
    ('for-in-continue-value-completion', '7;for(var k in {a:1,b:2}){k;continue;}'),
    ('for-in-outer-break-completion', '7;outer:for(var a in {a:1}){9;for(var b in {b:1}){break outer;}}'),
]

CASES.extend([
    ('do-body-before-test', 'let n=0,s="";do{s+="B";n++;}while((s+="T",n<3));test.record(s);do{n++;}while(false);test.record(n);'),
    ('do-continue-tests-condition', 'let n=0,t=0;do{n++;continue;}while(++t<3);test.record(n);test.record(t);'),
    ('do-break-skips-condition', 'let n=0;do{break;}while(++n);test.record(n);'),
    ('do-labelled-finally', 'let n=0,s="";outer:do{n++;try{if(n===1)continue outer;if(n===3)break outer;s+=n;}finally{s+="f";}}while(n<5);test.record(s);test.record(n);'),
    ('do-finally-replaces-break', 'let n=0;outer:do{try{break outer;}finally{n++;if(n<3)continue outer;}}while(n<5);test.record(n);'),
    ('do-var-hoists', 'test.record(n);do{var n=7;}while(false);test.record(n);'),
    ('do-asi-and-regexp', 'let n=0;do n++;while(false)test.record(n);do{}while(false)/a/.test("a");test.record(/b/.test("b"));'),
    ('do-empty-and-nested', 'let n=0;do;while(n++<2);test.record(n);let s="";outer:do{do{s+="a";break outer;}while(true);}while(true);test.record(s);'),
    ('compound-bitwise-numbers', 'let n=13;test.record(n&=6);test.record(n|=9);test.record(n^=7);test.record(n<<=3);test.record(n>>=2);test.record(n>>>=1);'),
    ('compound-bitwise-conversions', 'let a="4294967295",b=-1,c=0/0,d=-0;a>>>=0;b<<=33;c|="7";d^=false;test.record(a);test.record(b);test.record(c);test.record(d);'),
    ('compound-bitwise-reference-once', 'let s="",o={x:7};function base(){s+="b";return o;}function key(){s+="k";return "x";}function rhs(){s+="r";o.x=100;return 2;}test.record(base()[key()]<<=rhs());test.record(o.x);test.record(s);'),
    ('compound-bitwise-right-associative', 'let a=1,b=2;test.record(a|=b<<=2);test.record(a);test.record(b);'),
    ('compound-bitwise-regexp-goal', 'let n=1;n|=/a/.test("a");n<<=/b/.test("b");test.record(n);'),
    ('compound-bitwise-const-throws', 'const n=1;let s="";try{n|=(s+="r",2);}catch(e){test.record(e.name);}test.record(s);test.record(n);'),
    ('for-of-array-and-holes', 'let a=[1,,3],s="";for(const value of a)s+=String(value)+";";test.record(s);'),
    ('for-of-live-array-values', 'let a=[1,2,3],s="";for(var value of a){s+=value;if(value===1){a[1]=8;a.push(4);}}test.record(s);test.record(value);'),
    ('for-of-shrink-array', 'let a=[1,2,3],s="";for(let value of a){s+=value;a.length=1;}test.record(s);'),
    ('for-of-string-codepoints', 'let s="";for(const ch of "A😀é")s+=ch+":"+ch.length+";";test.record(s);'),
    ('for-of-empty-and-noniterables', 'const x="keep";for(x of []){}test.record(x);for(const item of [null,undefined,1,false,{}]){try{for(let x of item){test.record("bad");}}catch(e){test.record(e.name);}}'),
    ('for-of-lexical-closures', 'let f=[],outer=9;for(let outer of [1,2,3])f.push(()=>outer);test.record(outer);test.record(f[0]()+f[1]()+f[2]());'),
    ('for-of-const-closures', 'let f=[];for(const x of [4,5]){f.push(()=>x);try{x=1;}catch(e){test.record(e.name);}}test.record(f[0]());test.record(f[1]());'),
    ('for-of-var-shared-closure', 'let f=[];for(var x of [1,2,3])f.push(()=>x);test.record(f[0]());test.record(f[2]());'),
    ('for-of-right-side-tdz', 'let x=[1];try{for(let x of x){}}catch(e){test.record(e.name);}test.record(x[0]);'),
    ('for-of-right-side-closure-tdz', 'let f;for(let x of (f=()=>x,[1])){}try{f();}catch(e){test.record(e.name);}'),
    ('for-of-target-order', 'let s="",i=0,o={};function sourceItems(){s+="R";return [4,5];}function key(){s+="L";return i++;}for(o[key()] of sourceItems())s+="B";test.record(s);test.record(o[0]);test.record(o[1]);'),
    ('for-of-empty-target-not-evaluated', 'let i=0,o={};for(o[i++] of []){}test.record(i);'),
    ('for-of-var-catch-target', 'var x="outer";try{throw "catch";}catch(x){for(var x of [1,2]){}test.record(x);}test.record(x);'),
    ('for-of-labels-and-finally', 'let s="";outer:for(const x of [1,2]){for(const y of [3,4]){try{s+=x+":"+y+";";continue outer;}finally{s+="f";}}}test.record(s);'),
    ('for-of-contextual-of-regexp', 'let of=0,s="";for(of of /a/.exec("a"))s+=of;test.record(s);for(var n=of.length/2;n<1;n++)test.record(n);'),
    ('for-of-rhs-parenthesized-comma', 'let n=0;for(let x of (n++,[2,3]))n+=x;test.record(n);'),
    ('new-array-forms', 'let a=new Array,b=new Array(3),c=new Array(1,2,3);test.record(a.length);test.record(b.length);test.record(0 in b);test.record(c.join("-"));test.record(new Array().length);'),
    ('new-object-and-error', 'let source={x:1};test.record(new Object(source)===source);test.record(new Object().toString());let e=new TypeError("broken");test.record(e.name);test.record(e.message);test.record(e.toString());'),
    ('new-regexp-distinct-and-reset', 'let a=/a/g;a.lastIndex=2;let b=new RegExp(a);test.record(a===b);test.record(a.lastIndex);test.record(b.lastIndex);test.record(b.source);test.record(b.flags);test.record(RegExp(a)===a);'),
    ('new-regexp-flags-and-member', 'let c={R:RegExp,A:Array};test.record(new c.R("a+","i").test("AA"));test.record(new c.A(2).length);test.record((new RegExp).source);'),
    ('new-callee-resolved-before-arguments', 'let c={A:Array};let result=new c.A((c.A=Object,2));test.record(result.length);test.record(c.A===Object);'),
    ('new-nonconstructor-arguments-evaluate', 'let s="";try{new (3)((s+="a"),(s+="b"));}catch(e){test.record(e.name);}test.record(s);try{new (()=>1)();}catch(e){test.record(e.name);}'),
    ('new-builtin-not-constructors', 'try{new parseInt("3");}catch(e){test.record(e.name);}try{new eval("test.record(\"bad\")");}catch(e){test.record(e.name);}'),
    ('new-nested-precedence', 'test.record((new Array(1,2)).slice(1)[0]);try{new new Object;}catch(e){test.record(e.name);}let C=Array;test.record(new (true?C:Object)(2).length);'),
    ('eval-non-string-identity', 'let o={x:1};test.record(eval(o)===o);test.record(eval());test.record(eval(3));test.record(eval(null));'),
    ('eval-direct-local-read-write', 'function f(){let x=2;test.record(eval("x+=3;x"));test.record(x);test.record((eval)("x*2"));}f();'),
    ('eval-var-function-and-lexical-scope', 'function f(){eval("var x=3;function g(){return x;}let hidden=9;");test.record(x);test.record(g());test.record(typeof hidden);}f();'),
    ('eval-lexical-closure', 'let f=eval("let n=2;function step(){return ++n;}step");test.record(f());test.record(f());'),
    ('eval-strict-source-isolates-vars', 'function f(){let x=2;test.record(eval("\"use strict\";var hidden=4;x+=hidden;x"));test.record(x);test.record(typeof hidden);}f();'),
    ('eval-strict-caller-isolates-vars', 'function f(){"use strict";eval("var hidden=3");test.record(typeof hidden);test.record(eval("let n=4;n"));}f();'),
    ('eval-strict-inherited-by-nested-functions', 'function f(){"use strict";return function(){eval("var hidden=3");return typeof hidden;};}test.record(f()());'),
    ('eval-direct-this-and-extra-arguments', 'let o={x:8,f:function(){let n=0;test.record(eval("this.x",n++));test.record(n);}};o.f();'),
    ('eval-callee-shadowing-and-rebinding', 'function f(eval){return eval("text");}test.record(f(x=>x+"!"));function g(){let original=eval;let evalLocal=original;return evalLocal(4);}test.record(g());'),
    ('eval-parse-atomic-catchable', 'try{eval("test.record(99);let = ;");}catch(e){test.record(e.name);}test.record("after");'),
    ('eval-syntax-return-and-loop-boundary', 'function f(){try{eval("return 4;");}catch(e){test.record(e.name);}}f();outer:while(true){try{eval("break outer;");}catch(e){test.record(e.name);}break;}'),
    ('eval-var-lexical-conflicts', 'function f(){let x=2;try{eval("test.record(99);var x=3;");}catch(e){test.record(e.name);}test.record(x);{let y=4;try{eval("function y(){}");}catch(e){test.record(e.name);}}}f();'),
    ('eval-var-catch-binding', 'function f(){var e=1;try{throw 2;}catch(e){eval("var e=3;");test.record(e);}test.record(e);}f();'),
    ('eval-strict-directive-recognition', 'function f(){eval("(\"use strict\");var a=1;");eval("\"use\\x20strict\";var b=2;");eval("\"other\";\"use strict\";var c=3;");test.record(a);test.record(b);test.record(typeof c);}f();'),
    ('eval-nested-strict-restoration', 'function f(){eval("\"use strict\";eval(\"var x=3\");");eval("var y=4;");test.record(y);test.record(typeof x);}f();'),
    ('eval-thrown-identity-and-finally', 'let marker={n:1};try{eval("try{throw marker;}finally{marker.n++;}");}catch(e){test.record(e===marker);test.record(e.n);}'),
    ('eval-completion-values', 'test.record(eval("3;{}"));test.record(eval("let x=2;"));test.record(eval("do{7;break;}while(true)"));test.record(eval("for(var x of [1,2])x;"));'),
    ('regexp-lookaround-basic', 'test.record(/(?=a)a/.test("a"));test.record(/a(?!b)/.test("ac"));test.record(/(?<=a)b/.exec("ab")[0]);test.record(/(?<!a)b/.test("cb"));'),
])

BROWSER_CASES.extend([
    ('eval-indirect-global-vs-direct-local', 'var x=1;function f(){let x=7;let alias=eval;test.record(eval("x"));test.record(alias("x"));test.record((0,eval)("x"));test.record(window.eval("x"));test.record(eval.call(null,"x"));test.record(eval.apply(null,["x"]));}f();'),
    ('eval-indirect-declarations-and-this', 'function f(){let alias=eval;alias("var created=3;let local=4;");test.record(alias("this")===window);test.record(typeof local);}f();test.record(window.created);'),
    ('eval-indirect-does-not-inherit-strict', 'function f(){"use strict";let alias=eval;alias("var created=9;");}f();test.record(created);'),
    ('eval-global-lexical-conflict', 'let x=2;try{(0,eval)("var x=3;");}catch(e){test.record(e.name);}test.record(x);'),
    ('eval-this-follows-function-strictness', 'function f(){return eval("this");}function g(){"use strict";return eval("this");}test.record(f()===window);test.record(f.call(null)===window);test.record(g());test.record(g.call(null));test.record(g.call(7));'),
])

CASES.extend([
    ('eval-duplicate-declaration-syntax', 'try{eval("let x;let x;");}catch(e){test.record(e.name);}try{eval("let x;var x;");}catch(e){test.record(e.name);}'),
    ('eval-lexical-syntax-errors', 'for(const source of ["/*", "var x=\\"bad", "var x=0x;", "var r=/[z-a]/;"]){try{eval(source);}catch(e){test.record(e.name);}}'),
    ('eval-strict-binding-early-errors', 'for(const source of ["var eval;", "eval=3;", "eval++;", "++eval;", "try{}catch(eval){}", "for(eval of []){}", "function f(arguments){}"]){try{eval("\\"use strict\\";"+source);}catch(e){test.record(e.name);}}'),
    ('for-of-async-target-grammar', 'let async=0;try{eval("for(async of [1]){}");}catch(e){test.record(e.name);}for((async) of [2]){}test.record(async);'),
])

COMPLETION_CASES.extend([
    ('do-empty-completion', '7;do{}while(false);'),
    ('do-value-completion', '7;do{9;}while(false);'),
    ('do-continue-value-completion', '7;do{9;continue;}while(false);'),
    ('do-finally-break-completion', '7;outer:do{try{8;}finally{break outer;}}while(true);'),
    ('for-of-empty-completion', '7;for(var x of []){}'),
    ('for-of-value-completion', '7;for(var x of [1,2])x;'),
    ('for-of-break-value-completion', '7;for(var x of [1,2]){9;break;}'),
])


CASES.extend([
    ('switch-strict-values', 'let s="";for(let x of [0,"0",false,null,undefined,NaN]){switch(x){case 0:s+="n";break;case "0":s+="s";break;case false:s+="b";break;case null:s+="z";break;case undefined:s+="u";break;case NaN:s+="bad";break;default:s+="d";}}test.record(s);'),
    ('switch-object-identity', 'let o={},s="";switch(o){case {}:s="bad";break;case o:s="same";break;}test.record(s);'),
    ('switch-default-after-no-match', 'let s="";switch(9){case 1:s+="one";break;default:s+="default";}test.record(s);'),
    ('switch-default-before-match', 'let s="";switch(2){default:s+="d";case 1:s+="a";case 2:s+="b";case 3:s+="c";}test.record(s);'),
    ('switch-default-before-no-match', 'let s="";switch(9){case 1:s+="a";default:s+="d";case 2:s+="b";case 3:s+="c";}test.record(s);'),
    ('switch-matching-before-default-falls-through', 'let s="";switch(1){case 1:s+="a";default:s+="d";case 2:s+="b";}test.record(s);'),
    ('switch-test-evaluation-order', 'let s="";function v(n){s+=n;return n;}switch(v(2)){case v(1):s+="A";break;default:s+="D";case v(2):s+="B";case v(3):s+="C";}test.record(s);'),
    ('switch-discriminant-once', 'let n=0;switch(++n){case 1:case 2:test.record(n);break;}test.record(n);'),
    ('switch-all-case-tests-before-default', 'let s="";function v(n){s+=n;return n;}switch(9){case v(1):s+="A";default:s+="D";case v(2):s+="B";}test.record(s);'),
    ('switch-shared-lexical-scope', 'let x="outer";switch(1){case 1:let x=7;case 2:test.record(x);}test.record(x);'),
    ('switch-discriminant-outside-tdz', 'let x=1;switch(x){case 1:let x=2;test.record(x);}test.record(x);'),
    ('switch-case-expression-tdz', 'let x=1;try{switch(1){case x:let x=2;}}catch(e){test.record(e.name);}'),
    ('switch-skipped-declaration-tdz', 'try{switch(2){case 1:let x=2;break;case 2:test.record(x);}}catch(e){test.record(e.name);}'),
    ('switch-var-hoisted-unselected-case', 'test.record(x);switch(9){case 1:var x=2;}test.record(x);'),
    ('switch-function-instantiated-before-tests', '"use strict";switch(3){case f():test.record(f());break;case 9:function f(){return 3;}}test.record(typeof f);'),
    ('switch-loop-continue-and-break', 'let s="";for(let i=0;i<4;i++){switch(i){case 0:continue;case 1:s+="a";break;case 2:s+="b";default:s+="d";}s+="x";}test.record(s);'),
    ('switch-nested-break-target', 'let s="";switch(1){case 1:for(let i=0;i<2;i++){s+=i;break;}switch(2){case 2:s+="i";break;}s+="o";break;default:s+="bad";}test.record(s);'),
    ('switch-labelled-break-finally', 'let s="";out:switch(1){case 1:try{s+="a";break out;}finally{s+="f";}default:s+="bad";}test.record(s);'),
    ('switch-labelled-continue-finally', 'let s="";out:for(let i=0;i<2;i++){switch(i){case 0:try{continue out;}finally{s+="f";}case 1:s+="b";}}test.record(s);'),
    ('switch-return-through-finally', 'function f(){switch(1){case 1:try{return 7;}finally{test.record("finally");}}}test.record(f());'),
    ('switch-catches-duplicate-lexicals-before-effects', 'for(let text of ["switch(1){case 1:let x;case 2:let x;}","switch(1){case 1:let x;case 2:var x;}","switch(1){default:;default:;}"]){try{eval("test.record(99);"+text);}catch(e){test.record(e.name);}}'),
    ('switch-regexp-lexical-goals', 'switch(1){case /a/.test("a")?1:2:/b/.test("b")&&test.record("b");break;default:/c/.test("c");}/d/.test("d")&&test.record("d");'),
    ('switch-case-ternary-object-and-function', 'switch(2){case false?1:{x:2}.x:test.record("object");break;}switch(3){case (function(){return 3;})():test.record("function");}'),
    ('switch-regexp-after-case-function', '"use strict";switch(1){case 1:function f(){} /a/.test("a")&&test.record("regexp");}'),
    ('switch-nested-case-test', 'let n=1;switch(n){case (function(){switch(2){case 2:return 1;}return 9;})():test.record("nested");}'),
    ('switch-empty-clauses', 'switch(9){case 0:case 1:default:}switch(1){}test.record("done");'),
    ('json-builtins-and-alias', 'test.record(typeof JSON);test.record(typeof JSON.parse);test.record(typeof JSON.stringify);test.record(Object.keys(JSON).length);let p=JSON.parse,s=JSON.stringify;test.record(s(p("[1,true,null]")));test.record(p.call(null,"2"));'),
    ('json-primitive-input-conversion', 'test.record(JSON.parse(null));test.record(JSON.parse(true));test.record(JSON.parse(1.5));try{JSON.parse();}catch(e){test.record(e.name);}'),
    ('json-parse-numbers', 'for(let text of ["0","-0","1.25","-12.5e+2","1e-400","-1e-400","1e400","-1e400","9007199254740993"]){test.record(JSON.parse(text));}'),
    ('json-parse-nested', 'let o=JSON.parse("{\\"a\\":[1,{\\"b\\":false}],\\"z\\":null}");test.record(o.a.length);test.record(o.a[1].b);test.record(o.z);'),
    ('json-parse-duplicate-key-order', 'let o=JSON.parse("{\\"b\\":1,\\"2\\":2,\\"a\\":3,\\"1\\":4,\\"b\\":5}");test.record(Object.keys(o).join("|"));test.record(o.b);test.record(JSON.stringify(o));'),
    ('json-parse-constructor-is-own-data', 'let o=JSON.parse("{\\"constructor\\":3,\\"prototype\\":4}");test.record(o.constructor);test.record(o.prototype);test.record(JSON.stringify(o));'),
    ('json-parse-whitespace', 'test.record(JSON.parse(" \\t\\r\\n[1,2] \\r\\n")[1]);'),
    ('json-parse-string-escapes', 'test.record(JSON.parse("\\"a\\\\n\\\\r\\\\t\\\\b\\\\f\\\\/\\\\\\\\\\\\\\"\\\\u0041\\""));'),
    ('json-parse-paired-surrogates', 'test.record(JSON.parse("\\"\\\\ud83d\\\\ude00\\""));test.record(JSON.parse("\\"\\\\u0000é😀\\"").length);'),
    ('json-parse-invalid-catchable', 'for(let text of ["", "undefined", "NaN", "Infinity", "+1", "01", "-01", "1.", "1e", "1e+", "[1,]", "{\\"a\\":1,}", "[,,]", "{a:1}", "[1 2]", "true false", "\ufeff1", "/*x*/1", "\\"\\u0000\\"", "\\"\\\\uQ123\\"", "\\"\\\\x01\\""]){try{JSON.parse(text);test.record("bad");}catch(e){test.record(e.name);}}'),
    ('json-parse-noncallable-reviver-ignored', 'test.record(JSON.stringify(JSON.parse("[1]",{})));test.record(JSON.parse("2",null));test.record(JSON.parse("3",false));'),
    ('json-stringify-primitives', 'for(let x of [null,true,false,0,-0,1.25,1e-7,1e21,NaN,Infinity,-Infinity,"é😀"]){test.record(JSON.stringify(x));}'),
    ('json-stringify-root-undefined-functions', 'test.record(JSON.stringify());test.record(JSON.stringify(undefined));test.record(JSON.stringify(function(){}));'),
    ('json-stringify-object-omissions', 'test.record(JSON.stringify({a:undefined,b:function(){},c:null,d:1}));'),
    ('json-stringify-array-holes', 'let a=[,undefined,function(){},NaN,Infinity,-Infinity,-0];a.extra=1;test.record(JSON.stringify(a));'),
    ('json-stringify-key-order', 'let o={z:1,"10":2,"2":3,a:4};o.z=5;test.record(JSON.stringify(o));'),
    ('json-stringify-shared-not-circular', 'let o={x:1};test.record(JSON.stringify([o,o,{a:o}]));'),
    ('json-stringify-circular-catchable', 'let o={};o.self=o;try{JSON.stringify(o);}catch(e){test.record(e.name);}let a=[];a.push(a);try{JSON.stringify(a);}catch(e){test.record(e.name);}'),
    ('json-stringify-nested-pretty', 'test.record(JSON.stringify({a:[1,{b:2}],c:{},d:[]},null,2));'),
    ('json-stringify-space-numeric-bounds', 'for(let n of [-1,0,2.9,20,NaN,Infinity])test.record(JSON.stringify([1],null,n));'),
    ('json-stringify-space-string-bounds', 'test.record(JSON.stringify({a:1},null,"0123456789abcdefgh"));test.record(JSON.stringify([1],null,"😀😀😀😀😀more"));'),
    ('json-stringify-inert-optional-arguments', 'for(let r of [null,undefined,false,3,"x",{}])test.record(JSON.stringify({a:1},r,true));'),
    ('json-stringify-noncallable-tojson', 'test.record(JSON.stringify({toJSON:1,a:2}));let a=[1];a.toJSON=3;test.record(JSON.stringify(a));'),
    ('json-stringify-nonenumerable-builtins', 'test.record(JSON.stringify(JSON));test.record(JSON.stringify(new Error("hidden")));test.record(JSON.stringify(/x/g));'),
    ('json-roundtrip-escaped-keys', 'let o={"a\\nb":1,"\\"\\\\":2};test.record(JSON.stringify(JSON.parse(JSON.stringify(o))));'),
    ('json-stringify-control-characters', 'test.record(JSON.stringify("\\u0000\\u0001\\b\\f\\n\\r\\t\\"\\\\/"));'),
    ('json-array-parsing-keeps-independent-identities', 'let a=JSON.parse("[{},{}]");test.record(a[0]===a[1]);a[0].x=1;test.record(a[1].x);'),
    ('json-parser-is-not-eval', 'for(let s of ["(1)","1+2","test.record(99)","[function(){}]"]){try{JSON.parse(s);}catch(e){test.record(e.name);}}'),
])

COMPLETION_CASES.extend([
    ('switch-empty-completion', '7;switch(1){}'),
    ('switch-no-match-completion', '7;switch(1){case 2:9;}'),
    ('switch-break-empty-completion', '7;switch(1){case 1:break;}'),
    ('switch-break-value-completion', '7;switch(1){case 1:9;break;}'),
    ('switch-fallthrough-completion', '7;switch(1){case 1:9;default:3;case 2:4;}'),
    ('switch-try-finally-completion', '7;switch(1){case 1:try{9;}finally{break;}}'),
])

# Small real-site compatibility additions: exercise the newly implemented
# numeric, intrinsic-instance, deletion and string behaviors against Node.
CASES.extend([
    ('math-signed-zero-rounding', 'test.record(Math.round(-0.5));test.record(Math.round(0.49999999999999994));test.record(Math.round(-1.5));test.record(Math.min(0,-0));test.record(Math.max(-0,0));test.record(Math.sign(-0));test.record(Math.abs(-0));'),
    ('math-coercion-and-integer-helpers', 'test.record(Math.floor("3.9"));test.record(Math.ceil(null));test.record(Math.trunc(-2.8));test.record(Math.clz32(0));test.record(Math.clz32(1));test.record(Math.imul(4294967295,5));test.record(Math.fround(1.337));'),
    ('math-empty-nan-and-power', 'test.record(Math.min());test.record(Math.max());test.record(Math.floor());test.record(Math.min(1,NaN));test.record(Math.pow(1,Infinity));test.record(Math.pow(1,NaN));test.record(Math.pow(NaN,0));test.record(Math.pow(-0,-3));'),
    ('math-exact-function-boundaries', 'test.record(Math.hypot(3,4));test.record(Math.hypot(NaN,Infinity));test.record(Math.log2(8));test.record(Math.sqrt(-0));test.record(Math.sin(-0));test.record(Math.cos(0));test.record(Math.atan2(-0,1));test.record(Math.tanh(Infinity));'),
    ('math-namespace-constants-and-callables', 'let pi=Math.PI,ceil=Math.ceil;Math.PI=0;test.record(Math.PI===pi);test.record(Object.keys(Math).join());test.record(ceil(1.2));try{(function(){"use strict";Math.PI=0;})();}catch(e){test.record(e.name);}test.record(delete Math.PI);'),
    ('math-random-host-range', 'let n=Math.random();test.record(typeof n);test.record(n>=0&&n<1);'),
    ('intrinsic-instanceof', 'test.record([] instanceof Array);test.record([] instanceof Object);test.record(/a/ instanceof RegExp);test.record(new TypeError("x") instanceof Error);test.record(new TypeError() instanceof RangeError);test.record(Math.floor instanceof Object);test.record(3 instanceof Number);test.record(null instanceof (()=>0));'),
    ('instanceof-invalid-constructor', 'try{({}) instanceof 3;}catch(e){test.record(e.name);}try{({}) instanceof (()=>0);}catch(e){test.record(e.name);}try{({}) instanceof Math.floor;}catch(e){test.record(e.name);}'),
    ('delete-own-properties-and-array-holes', 'let a=[1,2,3],o={first:1,last:2};test.record(delete a[1]);test.record(a.length);test.record(1 in a);test.record(a.join(","));test.record(delete o.first);test.record(Object.keys(o).join());let keys="";for(let k in o){keys+=k;delete o[k];}test.record(keys);test.record(Object.keys(o).length);'),
    ('delete-protected-and-evaluation', 'let a=[1],n=0;test.record(delete a.length);test.record(delete /x/.lastIndex);test.record(delete (n+=1));test.record(n);test.record(delete "abc"[0]);test.record(delete "abc"[8]);try{(function(){"use strict";delete Math.PI;})();}catch(e){test.record(e.name);}try{delete null.x;}catch(e){test.record(e.name);}'),
    ('delete-strict-identifier-parse-error', 'try{eval("\\"use strict\\";delete absent;");}catch(e){test.record(e.name);}'),
    ('string-split-limits-and-empty', 'test.record(JSON.stringify("a,,b,".split(",")));test.record(JSON.stringify("abc".split()));test.record("abc".split(undefined,0).length);test.record("".split("").length);test.record("abc".split("",2).join(","));test.record("xnullz".split(null).join("-"));test.record("a,b,c".split(",",-1).join());test.record("a,b".split(",",Infinity).length);'),
    ('string-split-regexp-captures-and-zero-matches', 'test.record(JSON.stringify("a1b2c".split(/(\\d)/)));test.record(JSON.stringify("a,b;c".split(/(,)|(;)/)));test.record("ab".split(/(?:)/).join("|"));test.record("abc".split(/x*/).join("|"));test.record("".split(/a/).length);test.record("".split(/(?:)/).length);let r=/,/g;r.lastIndex=2;test.record("a,b,c".split(r).join("|"));test.record(r.lastIndex);'),
    ('string-slice-and-utf16-positions', 'test.record("abc".slice(-2));test.record("abc".slice(2,1));test.record("abc".substring(2,1));test.record("abc".substring(-2,2));test.record("abc".slice(0,undefined));test.record("abc".charAt(-0.5));test.record("abc".charAt(Infinity));test.record("abc".charCodeAt(9));test.record("😀".charCodeAt(0));test.record("😀".codePointAt(0));test.record("😀".codePointAt(1));test.record("😀".codePointAt(2));test.record("😀x".slice(2));'),
])

BROWSER_CASES.extend([
    ('delete-global-expando-and-var-binding', 'window.temp=4;var fixed=5;test.record(delete window.temp);test.record(typeof window.temp);test.record(delete window.fixed);test.record(fixed);test.record(delete absentGlobal);'),
])

NEGATIVE = [
    ('infinite-while', 'while(true){}'),
    ('infinite-for', 'for(;;){}'),
    ('recursive-limit', 'function f(){return f();}f();'),
    ('string-growth', 'let x="x";while(true){x=x+x;}'),
    ('deep-parens', 'test.record(' + '(' * 4000 + '1' + ')' * 4000 + ');'),
    ('deep-unary', 'test.record(' + '!' * 4000 + '1);'),
    ('long-binary-chain', 'test.record(' + '+'.join('1' for _ in range(12000)) + ');'),
    ('malformed-atomic', 'test.record(1);function {broken'),
    ('unsupported-getter-atomic', 'test.record(1);let x={get field(){return 2;}};'),
    ('host-constructor-escape', 'test.constructor.constructor("return process")();'),
]

# The 0.30 runtime raised the shared parameter/call allowance to 256. Check the
# accepted boundary as well as rejection just above it; the former 33/40-value
# negative fixtures no longer describe the production bound.
_BOUNDARY_PARAMS = ','.join('p' + str(i) for i in range(256))
_BOUNDARY_ARGS = ','.join(str(i) for i in range(256))
CASES.append(('parameter-and-call-boundary-256',
    'function boundary(' + _BOUNDARY_PARAMS + '){return arguments.length+p255;}'
    'test.record(boundary(' + _BOUNDARY_ARGS + '));'
    'test.record(boundary.apply(null,[' + _BOUNDARY_ARGS + ']));'
    'test.record(new Array(' + _BOUNDARY_ARGS + ').length);'))

NEGATIVE.extend([
    ('long-prefix-search', 'let text="' + 'a'*32000 + '";let needle="' + 'a'*15999 + 'b";text.indexOf(needle);'),
    ('handler-wrapper-escape-atomic', '});test.record("must not run");(function(){', True),
    ('handler-invocation-escape-atomic', '})();test.record("must not run");(function(event){', True),
    ('prototype-literal-atomic', 'test.record(1);let x={__proto__:{}};'),
    ('prototype-mutation', 'let o={};o["__proto__"]={};'),
    ('unsupported-custom-join-coercion', 'let a=[{toString(){return "custom";}}];a.join();'),
    ('array-huge-index', 'let a=[];a[4294967294]=1;'),
    ('array-huge-length', 'let a=[];a.length=4294967295;'),
    ('array-factory-huge-length', 'Array(4294967295);'),
    ('array-join-string-growth', 'let a=Array(4096);a.join("0123456789012345678901234567890123456789");'),
    ('array-deep-join', 'let a=[];for(var n=0;n<40;n++){a=[a];}a.join();'),
    ('apply-argument-limit', 'function f(){}f.apply(null,Array(257));'),
    ('apply-huge-arraylike', 'function f(){}f.apply(null,{length:Infinity});'),
    ('object-arena-limit', 'while(true){let o={};}'),
    ('property-name-limit', 'let key="' + 'a'*257 + '";let o={};o[key]=1;'),
    ('deep-object-atomic', 'test.record(1);let o=' + '{x:'*100 + '0' + '}'*100 + ';'),
])

NEGATIVE.extend([
    ('throw-newline-atomic', 'test.record("must not run");throw\n1;'),
    ('try-without-handler-atomic', 'test.record("must not run");try{}'),
    ('catch-lexical-conflict-atomic', 'test.record("must not run");try{}catch(e){let e;}'),
    ('catch-function-conflict-atomic', 'test.record("must not run");try{}catch(e){function e(){}}'),
    ('arrow-newline-atomic', 'test.record("must not run");let f=(a)\n=>a;'),
    ('arrow-duplicate-atomic', 'test.record("must not run");let f=(a,a)=>a;'),
    ('arrow-default-unsupported-atomic', 'test.record("must not run");let f=(a=1)=>a;'),
    ('limit-cannot-be-caught-atomic', 'try{while(true){}}catch(e){test.record("caught");}finally{test.record("finally");}'),
    ('limit-cannot-be-returned-atomic', 'function f(){try{while(true){}}finally{return 7;}}test.record(f());'),
    ('recursion-cannot-be-caught-atomic', 'function f(){try{return f();}catch(e){return 7;}finally{test.record("finally");}}test.record(f());'),
    ('unsupported-feature-cannot-be-caught-atomic', 'try{let o={};o.__proto__;}catch(e){test.record("caught");}finally{test.record("finally");}'),
    ('arrow-retained-this-bound-atomic', 'function make(){"use strict";return ()=>this;}let a=[];let text="'+'a'*32000+'";for(let i=0;i<80;i++){a.push(make.call(text));}test.record("done");'),
    ('uri-expanded-string-limit', 'encodeURIComponent("'+'é'*24000+'");'),
    ('long-sequence-chain', 'test.record(' + ','.join('1' for _ in range(12000)) + ');'),
    ('arrow-recursive-limit', 'let f=()=>f();f();'),
    ('catch-destructuring-unsupported-atomic', 'test.record(1);try{}catch({message}){}'),
])

NEGATIVE.extend([
    ('regex-named-group-unsupported-atomic', 'test.record(1);let r=/(?<named>a)/;'),
    ('regex-backreference-unsupported-atomic', r'test.record(1);let r=/(a)\1/;'),
    ('regex-unicode-flag-unsupported-atomic', 'test.record(1);let r=/a/u;'),
    ('regex-duplicate-flags-atomic', 'test.record(1);let r=/a/gg;'),
    ('regex-unterminated-atomic', 'test.record(1);let r=/a;'),
    ('regex-unterminated-class-atomic', 'test.record(1);let r=/[a/;'),
    ('regex-invalid-range-atomic', 'test.record(1);let r=/[z-a]/;'),
    ('regex-invalid-quantifier-atomic', 'test.record(1);let r=/*a/;'),
    ('regex-pattern-limit-atomic', 'test.record(1);let r=/'+'a'*4097+'/;'),
    ('regex-nesting-limit-atomic', 'test.record(1);let r=/'+'(?:'*40+'a'+')'*40+'/;'),
    ('regex-capture-limit-atomic', 'test.record(1);let r=/'+'(a)'*17+'/;'),
    ('regex-empty-repeat-compile-work-atomic', 'test.record(1);/(?:(?:(?:){1000}){1000}){1000}/;'),
    ('regex-dynamic-compile-work-cannot-catch-atomic', "try{RegExp('(?:(?:(?:){1000}){1000}){1000}');}catch(e){test.record(1);}finally{test.record(2);}"),
    ('regex-bytecode-limit-atomic', 'test.record(1);let r=/(?:abcdefghijklmnop){1000}/;'),
    ('regex-work-limit-cannot-catch-atomic', 'try{/(a+)+$/.test("'+'a'*80+'!");}catch(e){test.record("caught");}finally{test.record("finally");}'),
    ('regex-lone-surrogate-result', 'test.record(/./.exec("😀")[0]);'),
    ('regex-non-ascii-ignorecase-unsupported-atomic', 'test.record(1);let r=/é/i;'),
])


NEGATIVE.extend([
    ('unknown-label-atomic', 'test.record(1);label:{break missing;}'),
    ('duplicate-active-label-atomic', 'test.record(1);label:{label:{}}'),
    ('continue-non-loop-label-atomic', 'test.record(1);label:{while(true){continue label;}}'),
    ('break-across-function-atomic', 'test.record(1);label:{function f(){break label;}}'),
    ('continue-across-arrow-atomic', 'test.record(1);label:while(false){let f=()=>{continue label;};}'),
    ('labelled-const-atomic', 'test.record(1);label:const x=1;'),
    ('labelled-function-unsupported-atomic', 'test.record(1);label:function f(){}'),
    ('label-chain-limit-atomic', 'test.record(1);'+''.join(f'label{i}:' for i in range(65))+'{}'),
    ('labelled-infinite-limit-cannot-catch-atomic', 'try{label:while(true){continue label;}}catch(e){test.record(1);}finally{test.record(2);}'),
    ('for-in-multiple-bindings-atomic', 'test.record(1);for(var a,b in {}){}'),
    ('for-in-lexical-initializer-atomic', 'test.record(1);for(let a=1 in {}){}'),
    ('for-in-var-initializer-unsupported-atomic', 'test.record(1);for(var a=1 in {}){}'),
    ('for-in-arrow-no-in-atomic', 'test.record(1);for(var f=x=>"a" in {a:1};;)break;'),
    ('for-in-invalid-target-atomic', 'test.record(1);for(a+b in {}){}'),
    ('for-in-lexical-var-conflict-atomic', 'test.record(1);for(let a in {}){var a;}'),
    ('for-in-host-enumeration-cannot-catch-atomic', 'try{for(let k in test){}}catch(e){test.record(1);}finally{test.record(2);}'),
    ('for-in-string-key-limit-atomic', 'for(let key in "'+'x'*4097+'"){test.record(1);}'),
    ('for-in-snapshot-work-limit-atomic', 'let o={};for(var i=0;i<1000;i++)o["key"+i]=1;try{while(true){for(let k in o){break;}}}catch(e){test.record(1);}finally{test.record(2);}'),
    ('in-prototype-access-unsupported-atomic', 'test.record("__proto__" in {});'),
])

NEGATIVE.extend([
    ('do-infinite-cannot-catch-atomic', 'try{do{}while(true);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('do-deep-parser-atomic', 'test.record(1);'+'do{'*100+';'+'}while(false);'*100),
    ('do-missing-condition-atomic', 'test.record(1);do{}while();'),
    ('for-of-initializer-atomic', 'test.record(1);for(let x=1 of []){}'),
    ('for-of-multiple-bindings-atomic', 'test.record(1);for(var x,y of []){}'),
    ('for-of-unparenthesized-comma-atomic', 'test.record(1);for(var x of [],[]){}'),
    ('for-of-lexical-var-conflict-atomic', 'test.record(1);for(let x of []){var x;}'),
    ('for-of-host-capability-cannot-catch-atomic', 'try{for(let x of test){}}catch(e){test.record(1);}finally{test.record(2);}'),
    ('for-of-growth-bounded-atomic', 'let a=[1];try{for(var x of a)a.push(1);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('for-of-iteration-environment-bounded-atomic', 'try{for(let x of Array(4096)){}}catch(e){test.record(1);}finally{test.record(2);}'),
    ('new-parse-depth-atomic', 'test.record(1);'+'new '*1000+'Array;'),
    ('new-argument-limit-atomic', 'test.record(1);new Array('+','.join('1' for _ in range(257))+');'),
    ('new-wrapper-constructor-cannot-catch-atomic', 'try{new String("x");}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-missing-host-capability-atomic', 'test.denyEvalCapability=true;try{eval("test.record(1)");}catch(e){test.record(2);}finally{test.record(3);}'),
    ('eval-infinite-cannot-catch-atomic', 'try{eval("while(true){}");}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-recursive-depth-cannot-catch-atomic', 'var code="eval(code)";try{eval(code);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-retained-program-limit-atomic', 'try{for(var i=0;i<200;i++)eval("1");}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-retained-source-limit-atomic', 'var code='+json.dumps('/*'+' '*100000+'*/1;')+';try{for(var i=0;i<30;i++)eval(code);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-retained-ast-limit-atomic', 'var code='+json.dumps('if(false){'+'1;'*4000+'}')+';try{for(var i=0;i<40;i++)eval(code);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-parser-limit-cannot-catch-atomic', 'try{eval('+json.dumps('!'*4000+'1;')+');}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-arrow-parameter-limit-cannot-catch-atomic', 'try{eval('+json.dumps('('+','.join('a'+str(i) for i in range(257))+')=>1')+');}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-unsupported-syntax-cannot-catch-atomic', 'try{eval("class Unsupported {}");}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-unsupported-accessor-cannot-catch-atomic', 'try{eval("({get x(){return 1;}})");}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-unsupported-optional-chain-cannot-catch-atomic', 'try{eval("let o={};o?.x");}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-regexp-budget-cannot-catch-atomic', 'try{while(true)eval('+json.dumps('/(a+)+$/.test("'+'a'*80+'!");')+');}catch(e){test.record(1);}finally{test.record(2);}'),
])



NEGATIVE.extend([
    ('switch-duplicate-default-atomic', 'test.record(1);switch(1){default:;default:;}'),
    ('switch-duplicate-case-let-atomic', 'test.record(1);switch(1){case 1:let x;case 2:let x;}'),
    ('switch-lexical-var-conflict-atomic', 'test.record(1);switch(1){case 1:let x;default:if(false){var x;}}'),
    ('switch-continue-not-loop-atomic', 'test.record(1);switch(1){case 1:continue;}'),
    ('switch-continue-label-not-loop-atomic', 'test.record(1);s:switch(1){case 1:continue s;}'),
    ('switch-function-break-boundary-atomic', 'test.record(1);switch(1){case 1:function f(){break;}}'),
    ('switch-arrow-break-boundary-atomic', 'test.record(1);switch(1){case 1:let f=()=>{break;};}'),
    ('switch-sloppy-function-annex-b-unsupported-atomic', 'test.record(1);switch(1){case 1:function f(){return 9;}}'),
    ('switch-deep-parser-atomic', 'test.record(1);'+'switch(1){case 1:'*100+';'+'}'*100),
    ('switch-fatal-shared-loop-budget-atomic', 'try{while(true){switch(1){case 1:break;}}}catch(e){test.record(1);}finally{test.record(2);}'),
    ('json-reviver-unsupported-atomic', 'try{JSON.parse("1",function(){test.record(1);});}catch(e){test.record(2);}finally{test.record(3);}'),
    ('json-replacer-function-unsupported-atomic', 'try{JSON.stringify(1,function(){test.record(1);});}catch(e){test.record(2);}'),
    ('json-replacer-list-unsupported-atomic', 'try{JSON.stringify({x:1},["x"]);}catch(e){test.record(1);}'),
    ('json-tojson-hook-unsupported-atomic', 'try{JSON.stringify({toJSON(){test.record(1);return 2;}});}catch(e){test.record(2);}'),
    ('json-host-object-unsupported-atomic', 'try{JSON.stringify(test);}catch(e){test.record(1);}'),
    ('json-implicit-input-coercion-unsupported-atomic', 'try{JSON.parse({toString(){test.record(1);return "1";}});}catch(e){test.record(2);}'),
    ('json-lone-surrogate-unsupported-atomic', 'try{JSON.parse("\\"\\\\ud800\\"");}catch(e){test.record(1);}'),
    ('json-prototype-key-unsupported-atomic', 'try{JSON.parse('+json.dumps('{"__proto__":{"x":1}}')+');}catch(e){test.record(1);}'),
    ('json-property-name-limit-atomic', 'try{JSON.parse('+json.dumps(json.dumps({'x'*257:1}))+');}catch(e){test.record(1);}'),
    ('json-array-length-limit-atomic', 'try{JSON.parse('+json.dumps('['+','.join('1' for _ in range(4097))+']')+');}catch(e){test.record(1);}'),
    ('json-object-identity-limit-atomic', 'try{JSON.parse('+json.dumps('['+','.join('{}' for _ in range(4095))+']')+');}catch(e){test.record(1);}'),
    ('json-object-property-limit-atomic', 'try{JSON.parse('+json.dumps(json.dumps({str(i):1 for i in range(4097)},separators=(',',':')))+');}catch(e){test.record(1);}'),
    ('json-deep-parser-atomic', 'try{JSON.parse('+json.dumps('['*1000+'0'+']'*1000)+');}catch(e){test.record(1);}'),
    ('json-deep-stringifier-atomic', 'let a=[],b=a;for(var i=0;i<100;i++){let c=[];b.push(c);b=c;}try{JSON.stringify(a);}catch(e){test.record(1);}'),
    ('json-output-string-limit-atomic', 'let a=Array(4096),s="0123456789012345678901234567890123456789";for(var i=0;i<a.length;i++)a[i]=s;try{JSON.stringify(a);}catch(e){test.record(1);}'),
    ('json-shared-work-limit-atomic', 'let a=Array(4096);try{while(true)JSON.stringify(a);}catch(e){test.record(1);}finally{test.record(2);}'),
])

NEGATIVE.extend([
    ('string-split-array-bound-atomic', 'try{"'+'x'*4097+'".split("");}catch(e){test.record(1);}'),
    ('string-split-surrogate-unsupported-atomic', 'try{"😀".split("");}catch(e){test.record(1);}'),
])


# Iteration 0.25: ordinary prototypes, constructor identities, callable method
# properties, data descriptors, and the sparse-array padding seen in Search.
CASES.extend([
    ('prototype-root-identities', 'test.record(Object.getPrototypeOf({})===Object.prototype);test.record(Object.getPrototypeOf(Object.prototype));test.record(Object.getPrototypeOf(function(){})===Function.prototype);test.record(typeof Function.prototype);test.record(Function.prototype());test.record(Object.getPrototypeOf(Function.prototype)===Object.prototype);'),
    ('prototype-intrinsic-identities', 'test.record(Object.getPrototypeOf([])===Array.prototype);test.record(Object.getPrototypeOf(Array.prototype)===Object.prototype);test.record(Object.getPrototypeOf(/a/)===RegExp.prototype);test.record(Object.getPrototypeOf(TypeError())===TypeError.prototype);test.record(Object.getPrototypeOf(TypeError.prototype)===Error.prototype);test.record(Object.getPrototypeOf(Math)===Object.prototype);'),
    ('prototype-primitives', 'test.record(Object.getPrototypeOf("x")===String.prototype);test.record(Object.getPrototypeOf(1)===Number.prototype);test.record(Object.getPrototypeOf(false)===Boolean.prototype);try{Object.getPrototypeOf(null);}catch(e){test.record(e.name);}try{Object.getPrototypeOf(undefined);}catch(e){test.record(e.name);}'),
    ('prototype-custom-lookup', 'let p={x:1,m(){return this.y;}};let o=Object.create(p);o.y=3;test.record(o.x);test.record(o.m());test.record("x" in o);test.record(o.hasOwnProperty("x"));p.x=5;test.record(o.x);o.x=7;test.record(p.x);test.record(o.x);delete o.x;test.record(o.x);'),
    ('prototype-null-root', 'let o=Object.create(null);o.x=1;test.record(Object.getPrototypeOf(o));test.record(o.toString);test.record("toString" in o);test.record(o instanceof Object);test.record(Object.keys(o).join());test.record(Object.prototype.hasOwnProperty.call(o,"x"));'),
    ('prototype-change', 'let a={x:1},b={x:2},o=Object.create(a);test.record(Object.setPrototypeOf(o,b)===o);test.record(o.x);test.record(Object.getPrototypeOf(o)===b);test.record(Object.setPrototypeOf(o,null)===o);test.record(o.x);test.record(o.toString);'),
    ('prototype-cycle-rejected', 'let a={},b=Object.create(a);try{Object.setPrototypeOf(a,b);}catch(e){test.record(e.name);}test.record(Object.getPrototypeOf(a)===Object.prototype);try{Object.setPrototypeOf(b,b);}catch(e){test.record(e.name);}test.record(Object.getPrototypeOf(b)===a);'),
    ('prototype-immutable-root', 'test.record(Object.setPrototypeOf(Object.prototype,null)===Object.prototype);try{Object.setPrototypeOf(Object.prototype,{});}catch(e){test.record(e.name);}test.record(Object.getPrototypeOf(Object.prototype));'),
    ('prototype-create-invalid', 'try{Object.create(3);}catch(e){test.record(e.name);}try{Object.create(undefined);}catch(e){test.record(e.name);}try{Object.setPrototypeOf({},3);}catch(e){test.record(e.name);}try{Object.setPrototypeOf(null,{});}catch(e){test.record(e.name);}'),
    ('prototype-set-primitive', 'test.record(Object.setPrototypeOf(3,{}));test.record(Object.setPrototypeOf("x",null));test.record(Object.setPrototypeOf(false,{}));try{Object.setPrototypeOf(3,4);}catch(e){test.record(e.name);}'),
    ('prototype-function-links', 'function F(){}let p={v:4};Object.setPrototypeOf(F,p);test.record(F.v);test.record(typeof F.call);test.record(F instanceof Function);test.record(F instanceof Object);test.record(typeof F.prototype);Object.setPrototypeOf(F,Function.prototype);test.record(F instanceof Function);test.record(F());'),
    ('prototype-function-as-parent', 'function F(){}F.x=9;let o=Object.create(F);test.record(o.x);test.record(Object.getPrototypeOf(o)===F);test.record(F.isPrototypeOf(o));test.record(Object.prototype.isPrototypeOf.call(F,o));'),
    ('prototype-function-cycle', 'function F(){}let o=Object.create(F);try{Object.setPrototypeOf(F,o);}catch(e){test.record(e.name);}test.record(Object.getPrototypeOf(F)===Function.prototype);'),
    ('prototype-isprototypeof', 'let a={},b=Object.create(a),c=Object.create(b);test.record(a.isPrototypeOf(c));test.record(b.isPrototypeOf(c));test.record(c.isPrototypeOf(c));test.record(a.isPrototypeOf(null));test.record(Object.prototype.isPrototypeOf.call(null,3));try{Object.prototype.isPrototypeOf.call(null,{});}catch(e){test.record(e.name);}'),
    ('prototype-inherited-undefined', 'let o=Object.create({x:undefined});test.record(o.x);test.record("x" in o);test.record("y" in o);test.record(Object.keys(o).length);'),
    ('prototype-function-own-properties', 'function F(){}F.a=1;F["b"]=2;test.record(F.a);test.record(Object.keys(F).join());test.record(F.hasOwnProperty("prototype"));test.record(F.hasOwnProperty("call"));test.record(delete F.a);test.record(F.a);test.record(Object.keys(F).join());test.record(delete F.prototype);'),
    ('prototype-ordinary-constructor', 'function Box(x){this.x=x;}Box.prototype.double=function(){return this.x*2;};let a=new Box(7);test.record(a.x);test.record(a.double());test.record(a instanceof Box);test.record(a instanceof Object);test.record(a.constructor===Box);test.record(Box.prototype.constructor===Box);test.record(Object.getPrototypeOf(a)===Box.prototype);'),
    ('prototype-constructor-return', 'let p={x:4};function A(){this.x=1;return p;}function B(){this.x=2;return 3;}function C(){return function(){return 9;};}test.record(new A()===p);test.record(new B().x);test.record(new C()());'),
    ('prototype-constructor-null-and-primitives', 'function F(){}F.prototype=null;test.record(Object.getPrototypeOf(new F())===Object.prototype);F.prototype=3;test.record(Object.getPrototypeOf(new F())===Object.prototype);try{({}) instanceof F;}catch(e){test.record(e.name);}test.record(1 instanceof F);'),
    ('prototype-replacement-instanceof', 'function F(){}let first=new F();let original=F.prototype;F.prototype={x:3};let second=new F();test.record(first instanceof F);test.record(second instanceof F);test.record(second.x);test.record(Object.getPrototypeOf(first)===original);'),
    ('prototype-constructor-inheritance', 'function A(x){this.x=x;}A.prototype.value=function(){return this.x;};function B(x){A.call(this,x);}B.prototype=Object.create(A.prototype);B.prototype.constructor=B;let b=new B(5);test.record(b.value());test.record(b instanceof B);test.record(b instanceof A);test.record(b instanceof Object);test.record(b.constructor===B);'),
    ('prototype-constructor-argument-order', 'let log="";function F(x){log+="C";this.x=x;}function get(){log+="G";return F;}let o=new (get())((log+="A",3));test.record(log);test.record(o.x);'),
    ('prototype-constructor-callable-parent', 'function P(){}P.x=2;function F(){}F.prototype=P;let o=new F();test.record(o.x);test.record(Object.getPrototypeOf(o)===P);test.record(o instanceof F);'),
    ('prototype-arrow-and-method-nonconstructible', 'let f=()=>1,o={m(){return 2;}};test.record(f.hasOwnProperty("prototype"));test.record(o.m.hasOwnProperty("prototype"));try{new o.m();}catch(e){test.record(e.name);}try{new f();}catch(e){test.record(e.name);}'),
    ('prototype-function-metadata', 'function named(a,b){}test.record(named.name);test.record(named.length);test.record(named.propertyIsEnumerable("name"));test.record(named.propertyIsEnumerable("prototype"));test.record(named.prototype.propertyIsEnumerable("constructor"));named.name="changed";test.record(named.name);test.record(named.hasOwnProperty("length"));'),
    ('prototype-function-metadata-descriptors', 'function f(x){}let d=Object.getOwnPropertyDescriptor(f,"prototype");test.record(d.value===f.prototype);test.record(d.writable);test.record(d.enumerable);test.record(d.configurable);d=Object.getOwnPropertyDescriptor(f,"name");test.record(d.writable);test.record(d.enumerable);test.record(d.configurable);'),
    ('prototype-intrinsic-descriptors', 'let d=Object.getOwnPropertyDescriptor(Object,"prototype");test.record(d.value===Object.prototype);test.record(d.writable);test.record(d.enumerable);test.record(d.configurable);test.record(Object.keys(Object.prototype).join());test.record(Object.keys(Function.prototype).join());'),
    ('prototype-instanceof-chain-not-tag', 'let a=[];test.record(a instanceof Array);Object.setPrototypeOf(a,null);test.record(a instanceof Array);test.record(a instanceof Object);test.record(Array.isArray(a));let b=Object.create(Array.prototype);test.record(b instanceof Array);test.record(Array.isArray(b));'),
    ('prototype-method-value-and-identity', 'test.record(typeof Object.keys);test.record(typeof Array.isArray);test.record(typeof [].join);test.record(typeof (function(){}).call);test.record([].join===Array.prototype.join);test.record({}.hasOwnProperty===Object.prototype.hasOwnProperty);let keys=Object.keys;test.record(keys({a:1}).join());'),
    ('prototype-borrowed-object-methods', 'let own=Object.prototype.hasOwnProperty,tag=Object.prototype.toString;test.record(own.call({x:1},"x"));test.record(own.call("abc",1));test.record(own.call("abc","length"));test.record(tag.call([]));test.record(tag.call(function(){}));test.record(tag.call(null));test.record(tag.call(undefined));test.record(tag.call(/a/));test.record(tag.call(TypeError()));'),
    ('prototype-property-enumerability', 'let o=Object.create({a:1});o.b=2;test.record(o.propertyIsEnumerable("a"));test.record(o.propertyIsEnumerable("b"));test.record([1].propertyIsEnumerable("length"));test.record(Object.prototype.propertyIsEnumerable.call("abc",1));test.record(Object.prototype.propertyIsEnumerable.call("abc","length"));'),
    ('prototype-borrowed-array-methods', 'let a=[1,,3],join=Array.prototype.join,slice=Array.prototype.slice;test.record(join.call(a,"|"));test.record(slice.call(a,1).join());test.record(Array.prototype.push.call(a,4));test.record(a.join());'),
    ('prototype-borrowed-string-methods', 'let slice=String.prototype.slice;test.record(slice.call("abc",1));test.record(String.prototype.toString.call("x"));test.record(String.prototype.charCodeAt.call("abc",1));test.record(String.prototype.toString());test.record(String.prototype.indexOf.call(12345,"34"));'),
    ('prototype-string-indexed-reads', 'test.record("abc"[1]);test.record("abc"[9]);test.record("abc"["01"]);test.record("é"[0]);test.record("abc"[0.0]);'),
    ('prototype-borrowed-number-boolean-methods', 'test.record(Number.prototype.valueOf());test.record(Boolean.prototype.valueOf());test.record(Number.prototype.valueOf.call(3));test.record(Boolean.prototype.toString.call(true));try{Number.prototype.valueOf.call(false);}catch(e){test.record(e.name);}'),
    ('prototype-inherited-method-argument-order', 'let p={m(){return 1;}},o=Object.create(p);function change(){p.m=function(){return 2;};}test.record(o.m(change()));test.record(o.m());'),
    ('prototype-primitive-method-argument-order', 'let old=String.prototype.slice;function change(){String.prototype.slice=function(){"use strict";return "changed";};}test.record("abc".slice(1,change()));test.record("abc".slice(1));String.prototype.slice=old;'),
    ('prototype-for-in-order', 'let p={p:1,2:2},o=Object.create(p);o.a=3;o[1]=4;let keys="";for(let k in o)keys+=k+",";test.record(keys);test.record(Object.keys(o).join());'),
    ('prototype-for-in-shadow', 'let p={a:1,b:2,c:3},o=Object.create(p);o.b=4;Object.defineProperty(o,"a",{value:9});let keys="";for(let k in o)keys+=k+",";test.record(keys);'),
    ('prototype-for-in-function', 'function F(){}F.a=1;let p={b:2};Object.setPrototypeOf(F,p);let keys="";for(let k in F)keys+=k+",";test.record(keys);'),
    ('prototype-array-inherited-holes', 'Array.prototype[1]="x";let a=Array(3);test.record(a.join("|"));test.record(a.indexOf("x"));test.record(a.includes("x"));let b=a.slice();test.record(b.hasOwnProperty(1));test.record(b.join("|"));delete Array.prototype[1];'),
    ('prototype-descriptor-defaults', 'let o={};test.record(Object.defineProperty(o,"x",{value:3})===o);let d=Object.getOwnPropertyDescriptor(o,"x");test.record(d.value);test.record(d.writable);test.record(d.enumerable);test.record(d.configurable);o.x=4;test.record(o.x);test.record(delete o.x);'),
    ('prototype-descriptor-redefine', 'let o={x:1};Object.defineProperty(o,"x",{value:2});let d=Object.getOwnPropertyDescriptor(o,"x");test.record(d.value);test.record(d.writable);test.record(d.enumerable);test.record(d.configurable);Object.defineProperty(o,"x",{writable:false});o.x=9;test.record(o.x);Object.defineProperty(o,"x",{value:4,writable:true});test.record(o.x);'),
    ('prototype-descriptor-nonconfigurable', 'let o={};Object.defineProperty(o,"x",{value:3});Object.defineProperty(o,"x",{value:3});try{Object.defineProperty(o,"x",{value:4});}catch(e){test.record(e.name);}try{Object.defineProperty(o,"x",{writable:true});}catch(e){test.record(e.name);}try{Object.defineProperty(o,"x",{enumerable:true});}catch(e){test.record(e.name);}test.record(o.x);'),
    ('prototype-descriptor-samevalue', 'let o={};Object.defineProperty(o,"nan",{value:NaN});Object.defineProperty(o,"nan",{value:NaN});Object.defineProperty(o,"z",{value:-0});try{Object.defineProperty(o,"z",{value:0});}catch(e){test.record(e.name);}test.record(1/o.z);'),
    ('prototype-descriptor-strict-assignment', 'let o={};Object.defineProperty(o,"x",{value:3});try{(function(){"use strict";o.x=4;})();}catch(e){test.record(e.name);}try{(function(){"use strict";delete o.x;})();}catch(e){test.record(e.name);}'),
    ('prototype-descriptor-inherited-readonly', 'let p={};Object.defineProperty(p,"x",{value:3});let o=Object.create(p);o.x=4;test.record(o.x);test.record(o.hasOwnProperty("x"));try{(function(){"use strict";o.x=4;})();}catch(e){test.record(e.name);}Object.defineProperty(o,"x",{value:7});test.record(o.x);'),
    ('prototype-descriptor-inherited-fields', 'let attributes=Object.create({value:3,writable:true,enumerable:true});let o={};Object.defineProperty(o,"x",attributes);test.record(o.x);test.record(Object.keys(o).join());o.x=4;test.record(o.x);'),
    ('prototype-descriptor-absent', 'let p={x:1},o=Object.create(p);test.record(Object.getOwnPropertyDescriptor(o,"x"));test.record(Object.getOwnPropertyDescriptor(o,"missing"));'),
    ('prototype-descriptors-create', 'let o=Object.create(null,{x:{value:3,enumerable:true},y:{value:4}});test.record(o.x);test.record(o.y);test.record(Object.keys(o).join());test.record(Object.getPrototypeOf(o));'),
    ('prototype-descriptors-many', 'let o={};test.record(Object.defineProperties(o,{a:{value:1,enumerable:true},b:{value:2}})===o);test.record(o.a);test.record(o.b);test.record(Object.keys(o).join());'),
    ('prototype-descriptors-snapshot-before-write', 'let t={};Object.defineProperties(t,{value:{value:7},result:t});test.record(t.value);test.record(t.result===undefined);test.record(t.hasOwnProperty("result"));'),
    ('prototype-array-push-inherited-readonly', 'let p={};Object.defineProperty(p,"0",{value:8});let a=[];Object.setPrototypeOf(a,p);try{Array.prototype.push.call(a,1);}catch(e){test.record(e.name);}test.record(a.length);test.record(Object.prototype.hasOwnProperty.call(a,"0"));test.record(a[0]);'),
    ('prototype-descriptors-prevalidate', 'let o={};try{Object.defineProperties(o,{a:{value:1},b:3});}catch(e){test.record(e.name);}test.record(o.a);'),
    ('prototype-descriptor-function', 'function F(){}Object.defineProperty(F,"field",{value:7,enumerable:true,writable:true});test.record(F.field);test.record(Object.keys(F).join());Object.defineProperty(F,"name",{value:"renamed"});test.record(F.name);'),
    ('prototype-object-assign', 'let p={hidden:1},s=Object.create(p);s.a=2;let t={};test.record(Object.assign(t,null,s,{b:3})===t);test.record(t.a);test.record(t.b);test.record(t.hidden);test.record(Object.keys(t).join());'),
    ('prototype-object-assign-readonly', 'let t={};Object.defineProperty(t,"x",{value:1});try{Object.assign(t,{a:2,x:3,b:4});}catch(e){test.record(e.name);}test.record(t.a);test.record(t.x);test.record(t.b);'),
    ('prototype-sparse-padding-7824', 'let a=Array(7824);test.record(a.length);test.record(Object.keys(a).length);test.record(a.join("\\n").length);'),
    ('prototype-sparse-high-index', 'let a=Array(8192);a[8191]="x";test.record(Object.keys(a).join());test.record(a.length);test.record(a.slice(8190).join("|"));test.record(a.pop());test.record(a.length);'),
    ('prototype-sparse-join-empty', 'test.record(Array(8192).join(""));test.record(Array(7824).join("-").length);'),
    ('prototype-search-padding-expression', 'test.record(Array(Math.random()*7824|0).join("\\n").length>=0);'),
    ('prototype-symbol-style-constructor', 'let Token=function(text,description){this.j=text;Object.defineProperty(this,"description",{configurable:true,writable:true,value:description});};Token.prototype.toString=function(){return this.j;};let a=new Token("token_1","name");test.record(a.toString());test.record(a.description);test.record(a instanceof Token);test.record(Object.keys(a).join());'),
])
BROWSER_CASES.extend([
    ('prototype-window-constructor-guard', 'function Token(){return this instanceof Token;}test.record(Token());test.record(new Token() instanceof Token);test.record(window instanceof Token);try{window instanceof (()=>0);}catch(e){test.record(e.name);}'),
    ('prototype-window-script-owned-data', 'Object.defineProperty(window,"polyfillField",{value:3,writable:true,configurable:true});test.record(polyfillField);test.record(window.polyfillField);window.polyfillField=4;test.record(polyfillField);test.record(Object.getOwnPropertyDescriptor(window,"polyfillField").value);test.record(delete window.polyfillField);test.record(typeof polyfillField);'),
    ('prototype-window-script-owned-descriptor-batch', 'Object.defineProperties(window,{polyA:{value:1},polyB:{value:2,configurable:true}});test.record(polyA);test.record(polyB);test.record(delete window.polyA);test.record(delete window.polyB);'),
])
NEGATIVE.extend([
    ('prototype-dynamic-function-disabled-atomic', 'try{Function("test.record(1)")();}catch(e){test.record(2);}finally{test.record(3);}'),
    ('prototype-dynamic-function-indirect-disabled-atomic', 'try{({}).constructor.constructor("test.record(1)")();}catch(e){test.record(2);}'),
    ('prototype-host-chain-disabled-atomic', 'try{Object.create(test);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-host-target-disabled-atomic', 'try{Object.setPrototypeOf(test,{});}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-host-descriptor-disabled-atomic', 'try{Object.defineProperty(test,"number",{value:3});}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-array-descriptor-disabled-atomic', 'try{Object.defineProperty([],"0",{value:1});}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-chain-work-bound-atomic', 'let o={};try{for(let n=0;n<5000;n++){o=Object.create(o);o.missing;}}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-descriptor-retained-bound-atomic', 'let o={},s="'+'x'*32000+'";try{for(let n=0;n<100;n++)Object.defineProperty(o,"p"+n,{value:s});}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-constructor-recursion-bound-atomic', 'function F(){return new F();}try{new F();}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-borrowed-apply-recursion-bound-atomic', 'let a=[];a[0]=Function.prototype.apply;a[1]=a;try{Function.prototype.apply.apply(Function.prototype.apply,a);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-inherited-join-conversion-disabled-atomic', 'let p={toString(){return "custom";}},o=Object.create(p);try{[o].join();}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-regexp-virtual-descriptor-disabled-atomic', 'try{Object.getOwnPropertyDescriptor(/a/,"source");}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-sparse-array-upper-bound-atomic', 'try{Array(8193);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-sparse-high-index-bound-atomic', 'let a=[];try{a[8192]=1;}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-sparse-large-separator-bound-atomic', 'try{Array(8192).join("012345678901234567890");}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-sparse-stored-property-bound-atomic', 'let a=Array(8192);try{for(let n=0;n<5000;n++)a[n]=1;}catch(e){test.record(1);}finally{test.record(2);}'),
])


# Iteration 0.26: property-key conversion is observable user code. The order
# checks intentionally distinguish plain assignment, read/modify/write, and
# each builtin's own argument-versus-receiver coercion order.
CASES.extend([
    ('dense-function-over-old-token-cap', 'function unused(){'+'0;'*20000+'}test.record(typeof unused);'),
    ('dense-eval-over-old-token-cap', 'eval('+json.dumps('function unused(){'+'0;'*20000+'}')+');test.record(typeof unused);'),
    ('key-ordinary-tostring-read', 'let key={toString(){return "x";}},o={x:7};test.record(o[key]);test.record(key in o);'),
    ('key-conversion-prefers-string', 'let calls="",key={toString(){calls+="S";return "x";},valueOf(){calls+="V";return "y";}},o={x:1,y:2};test.record(o[key]);test.record(calls);'),
    ('key-valueof-fallback', 'let calls="",key={toString(){calls+="S";return {};},valueOf(){calls+="V";return 3;}},o={3:9};test.record(o[key]);test.record(calls);'),
    ('key-skip-noncallable-method', 'let key={toString:3,valueOf(){return "x";}};test.record({x:4}[key]);key.toString=null;test.record({x:5}[key]);'),
    ('key-null-prototype-fallback', 'let key=Object.create(null);key.valueOf=function(){return "x";};test.record({x:6}[key]);'),
    ('key-conversion-original-receiver-noargs', 'let key={name:"chosen",toString(a,b){test.record(this===key);test.record(a);test.record(b);return this.name;}};test.record({chosen:8}[key]);'),
    ('key-inherited-tostring', 'let key=Object.create({toString(){return this.name;}});key.name="x";test.record({x:9}[key]);'),
    ('key-default-object-conversion', 'let o={};o[{}]=1;test.record(o[{}]);test.record(Object.keys(o).join());'),
    ('key-array-and-regexp-conversion', 'let o={};o[[1,2]]=3;o[/x/]=4;test.record(o["1,2"]);test.record(o["/x/"]);test.record(o[[1,2]]);'),
    ('key-function-custom-conversion', 'function key(){}key.toString=function(){return "fn";};test.record({fn:4}[key]);test.record(key in {fn:1});'),
    ('key-nonprimitive-result-fallback', 'let key={toString(){return test;},valueOf(){return "x";}};test.record({x:7}[key]);'),
    ('key-returned-primitives', 'let key={},o={};for(let x of [null,undefined,true,false,-0,NaN,Infinity]){key.toString=function(){return x;};o[key]=x;}test.record(Object.keys(o).join());test.record(o["null"]);test.record(o["undefined"]);test.record(o["0"]);test.record(o["NaN"]);'),
    ('key-null-return-stops-conversion', 'let key={toString(){return null;},valueOf(){test.record("wrong");return "x";}};test.record({null:2}[key]);'),
    ('key-tostring-mutates-valueof', 'let key={toString(){this.valueOf=function(){return "next";};return this;},valueOf(){return "wrong";}};test.record({next:8}[key]);'),
    ('key-tostring-mutates-prototype', 'let p={valueOf(){return "new";}};let key={toString(){Object.setPrototypeOf(this,p);return this;}};test.record({new:9}[key]);'),
    ('key-tostring-deletes-fallback', 'let key=Object.create(null);key.toString=function(){delete this.valueOf;return this;};key.valueOf=function(){return "x";};try{({x:1})[key];}catch(e){test.record(e.name);}'),
    ('key-tostring-thrown-value', 'let marker={},key={toString(){throw marker;},valueOf(){test.record("wrong");return "x";}};try{({})[key];}catch(e){test.record(e===marker);}'),
    ('key-valueof-thrown-value', 'let key={toString(){return this;},valueOf(){throw 17;}};try{({})[key];}catch(e){test.record(e);}'),
    ('key-nonprimitive-failure-catchable', 'let key={toString(){return {};},valueOf(){return function(){};}};try{({})[key];}catch(e){test.record(e.name);}test.record("after");'),
    ('key-no-method-failure-catchable', 'let key=Object.create(null);try{({})[key];}catch(e){test.record(e.name);}'),
    ('key-read-after-conversion-effects', 'let o={x:1},key={toString(){o.x=2;return "x";}};test.record(o[key]);'),
    ('key-assignment-rhs-before-conversion', 'let log="",key={toString(){log+="K";return "x";}},o={};o[key]=(log+="R",3);test.record(log);test.record(o.x);'),
    ('key-assignment-rhs-changes-conversion', 'let key={toString(){return "old";}},o={};o[key]=(key.toString=function(){return "new";},4);test.record(o.old);test.record(o.new);'),
    ('key-assignment-rhs-throw-skips-conversion', 'let log="",key={toString(){log+="K";return "x";}},o={};function rhs(){log+="R";throw 3;}try{o[key]=rhs();}catch(e){test.record(e);}test.record(log);'),
    ('key-compound-converts-read-and-write', 'let log="",n=0,key={toString(){log+="K";return n++===0?"a":"b";}},o={a:2,b:8};o[key]+=(log+="R",3);test.record(log);test.record(o.a);test.record(o.b);test.record(n);'),
    ('key-update-converts-read-and-write', 'let n=0,key={toString(){return n++===0?"a":"b";}},o={a:2,b:8};test.record(o[key]++);test.record(o.a);test.record(o.b);test.record(n);n=0;test.record(++o[key]);test.record(n);'),
    ('key-call-before-arguments-and-receiver', 'let log="",o={m(){log+="M";test.record(this===o);}},key={toString(){log+="K";return "m";}};o[key](log+="A");test.record(log);'),
    ('key-call-resolves-before-arguments-mutate', 'let o={m(){return 1;}},key={toString(){return "m";}};function rhs(){o.m=function(){return 2;};}test.record(o[key](rhs()));test.record(o[key]());'),
    ('key-computed-literal-order', 'let log="",key={toString(){log+="K";return "x";}};let o={[key]:(log+="V",7)};test.record(log);test.record(o.x);'),
    ('key-computed-literal-throw-skips-value', 'let log="",key={toString(){throw 5;}};try{({[key]:(log+="V",7)});}catch(e){test.record(e);}test.record(log);'),
    ('key-delete-converts-once', 'let n=0,key={toString(){n++;return "x";}},o={x:1};test.record(delete o[key]);test.record(n);test.record(o.x);'),
    ('key-delete-effects-before-delete', 'let key={toString(){o.x=2;return "x";}},o={};test.record(delete o[key]);test.record(o.x);'),
    ('key-null-read-skips-conversion', 'let log="",key={toString(){log+="K";return "x";}};function k(){log+="E";return key;}try{null[k()];}catch(e){test.record(e.name);}test.record(log);'),
    ('key-null-assignment-evaluates-rhs', 'let log="",key={toString(){log+="K";return "x";}};function k(){log+="E";return key;}try{null[k()]=(log+="R",1);}catch(e){test.record(e.name);}test.record(log);'),
    ('key-null-call-and-delete-skip-conversion', 'let log="",key={toString(){log+="K";return "x";}};try{null[key](log+="A");}catch(e){test.record(e.name);}try{delete null[key];}catch(e){test.record(e.name);}test.record(log);'),
    ('key-in-operands-before-conversion', 'let log="",key={toString(){log+="K";return "x";}};function left(){log+="L";return key;}function right(){log+="R";return {x:1};}test.record(left() in right());test.record(log);'),
    ('key-in-invalid-rhs-skips-conversion', 'let log="",key={toString(){log+="K";return "x";}};try{key in 3;}catch(e){test.record(e.name);}test.record(log);'),
    ('key-in-presence-after-conversion', 'let o={},key={toString(){o.x=1;return "x";}};test.record(key in o);'),
    ('key-descriptor-conversion', 'let o={},key={toString(){return "field";}};Object.defineProperty(o,key,{value:7,enumerable:true});test.record(o.field);test.record(Object.getOwnPropertyDescriptor(o,key).value);test.record(o.hasOwnProperty(key));test.record(o.propertyIsEnumerable(key));'),
    ('key-define-before-descriptor-snapshot', 'let attrs={value:3},o={},key={toString(){attrs.value=8;return "x";}};Object.defineProperty(o,key,attrs);test.record(o.x);'),
    ('key-getdescriptor-observes-effects', 'let o={x:1},key={toString(){o.x=2;return "x";}};test.record(Object.getOwnPropertyDescriptor(o,key).value);'),
    ('key-define-invalid-target-before-key', 'let log="",key={toString(){log+="K";return "x";}};try{Object.defineProperty(3,key,{value:1});}catch(e){test.record(e.name);}test.record(log);'),
    ('key-ownmethods-before-null-receiver', 'let log="",key={toString(){log+="K";return "x";}};try{Object.prototype.hasOwnProperty.call(null,key);}catch(e){test.record(e.name);}try{Object.prototype.propertyIsEnumerable.call(undefined,key);}catch(e){test.record(e.name);}test.record(log);'),
    ('key-ownmethods-key-exception-wins', 'let marker={},key={toString(){throw marker;}};try{Object.prototype.hasOwnProperty.call(null,key);}catch(e){test.record(e===marker);}try{Object.prototype.propertyIsEnumerable.call(null,key);}catch(e){test.record(e===marker);}'),
    ('key-ownmethods-observe-conversion-effects', 'let o={},key={toString(){o.x=1;return "x";}};test.record(o.hasOwnProperty(key));test.record(o.propertyIsEnumerable(key));'),
    ('key-ordinary-native-method-callback', 'let key={toString:Object.prototype.toString},o={"[object Object]":1};test.record(o[key]);key.toString=Math.floor;o.NaN=2;test.record(o[key]);'),
    ('key-polyfill-style-iterator', 'function Token(text){this.j=text;}Token.prototype.toString=function(){return this.j;};let key=new Token("custom_iterator_1");Object.defineProperty(Array.prototype,key,{configurable:true,writable:true,value:function(){return this.length;}});let a=[1,2];test.record(a[key]());test.record(key in a);test.record(a.hasOwnProperty(key));test.record(Array.prototype.hasOwnProperty(key));test.record(Object.getOwnPropertyDescriptor(Array.prototype,key).enumerable);test.record(delete Array.prototype[key]);'),
])
BROWSER_CASES.extend([
    ('key-window-script-descriptor', 'let key={toString(){return "customKey26";}};Object.defineProperty(window,key,{value:9,configurable:true});test.record(customKey26);test.record(window[key]);test.record(Object.getOwnPropertyDescriptor(window,key).value);test.record(key in window);test.record(delete window[key]);test.record(typeof customKey26);'),
])
NEGATIVE.extend([
    ('dense-token-cap-atomic', 'test.record(1);'+';'*131072),
    ('dense-ast-cap-atomic', 'test.record(1);'+'0\n'*65536),
    ('dense-eval-string-cap-atomic', 'try{eval('+json.dumps('test.record(1);'+';'*131072)+');}catch(e){test.record(2);}finally{test.record(3);}'),
    ('key-host-value-unsupported-atomic', 'try{let o={};o[test]=1;}catch(e){test.record(1);}finally{test.record(2);}'),
    ('key-host-value-read-unsupported-atomic', 'try{({})[test];}catch(e){test.record(1);}finally{test.record(2);}'),
    ('key-callback-host-error-fatal-atomic', 'let key={toString(){return test.unavailable();}};try{({})[key];}catch(e){test.record(1);}finally{test.record(2);}'),
    ('key-callback-work-bound-atomic', 'let key={toString(){while(true){}}};try{({})[key];}catch(e){test.record(1);}finally{test.record(2);}'),
    ('key-callback-recursive-read-bound-atomic', 'let o={},key={toString(){return o[key];}};try{o[key];}catch(e){test.record(1);}finally{test.record(2);}'),
    ('key-callback-recursive-descriptor-bound-atomic', 'let o={},key={toString(){return Object.getOwnPropertyDescriptor(o,key);}};try{o[key];}catch(e){test.record(1);}finally{test.record(2);}'),
    ('key-callback-recursive-ownmethod-bound-atomic', 'let o={},key={toString(){return o.hasOwnProperty(key);}};try{o[key];}catch(e){test.record(1);}finally{test.record(2);}'),
    ('key-converted-name-bound-atomic', 'let key={toString(){return "'+'x'*257+'";}};try{({})[key];}catch(e){test.record(1);}finally{test.record(2);}'),
    ('key-prototype-access-remains-blocked-atomic', 'let key={toString(){return "__proto__";}};try{({})[key];}catch(e){test.record(1);}finally{test.record(2);}'),
])

# Fixed binary-data regression cases: view identity, coercion/order, buffer
# bounds, descriptors and callbacks are compared with an independent engine.
CASES.extend([
    ('buffer-constructor-zero', 'let b=new ArrayBuffer();test.record(b.byteLength);test.record(b.maxByteLength);test.record(b.resizable);test.record(b.detached);test.record(new ArrayBuffer(3.9).byteLength);test.record(new ArrayBuffer(null).byteLength);test.record(new ArrayBuffer(NaN).byteLength);test.record(new ArrayBuffer(-0.9).byteLength);'),
    ('binary-constructor-new-required', 'for(let c of [ArrayBuffer,Uint8Array,DataView]){try{c(1);}catch(e){test.record(e.name);}}'),
    ('binary-constructor-prototype-chains', 'let a=new Uint16Array(3);test.record(a instanceof Uint16Array);test.record(a instanceof Object);test.record(a instanceof Uint8Array);test.record(Object.getPrototypeOf(a)===Uint16Array.prototype);test.record(Object.getPrototypeOf(Uint16Array)===Object.getPrototypeOf(Uint8Array));test.record(Object.getPrototypeOf(Uint16Array.prototype)===Object.getPrototypeOf(Uint8Array.prototype));test.record(Uint8Array.prototype.toString===Array.prototype.toString);test.record(Object.getPrototypeOf(Uint8Array).name);'),
    ('binary-constructor-lengths', 'test.record(ArrayBuffer.length);test.record(DataView.length);test.record(Uint8Array.length);test.record(Float64Array.length);test.record(Uint8Array.prototype.set.length);test.record(Uint8Array.prototype.slice.length);test.record(Uint8Array.from.length);test.record(Uint8Array.of.length);test.record(DataView.prototype.getInt32.length);test.record(DataView.prototype.setFloat64.length);'),
    ('binary-tags-and-isview', 'let b=new ArrayBuffer(8),a=new Uint16Array(b),d=new DataView(b);for(let v of [b,a,d,{},[],null,undefined,Uint8Array.prototype]){test.record(Object.prototype.toString.call(v));test.record(ArrayBuffer.isView(v));}'),
    ('binary-typed-kinds', 'for(let c of [Int8Array,Uint8Array,Uint8ClampedArray,Int16Array,Uint16Array,Int32Array,Uint32Array,Float16Array,Float32Array,Float64Array]){let a=new c(3);test.record(c.name);test.record(a.BYTES_PER_ELEMENT);test.record(c.BYTES_PER_ELEMENT);test.record(a.length);test.record(a.byteLength);test.record(a.byteOffset);test.record(a.buffer.byteLength);test.record(a.join());}'),
    ('binary-integer-wrap', 'for(let c of [Int8Array,Uint8Array,Int16Array,Uint16Array,Int32Array,Uint32Array]){let a=new c([-1,-0,0,1,127,128,255,256,32767,32768,65535,65536,2147483647,2147483648,4294967295,4294967296,4294967297,NaN,Infinity,-Infinity,2.9,-2.9]);test.record(a.join());}'),
    ('binary-clamp-even', 'test.record(new Uint8ClampedArray([NaN,Infinity,-Infinity,-1,-0,0,0.5,1.5,2.5,3.5,253.5,254.5,254.9,255,256]).join());'),
    ('binary-float-conversion', 'for(let c of [Float16Array,Float32Array,Float64Array]){let a=new c([0,-0,1.1,-2.25,NaN,Infinity,-Infinity,1e-40,65520]);for(let v of a)test.record(v);}'),
    ('binary-half-rounding-edges', 'let a=new Float16Array([0.0000000298023223876953125,0.00000002980232238769532,0.0000000894069671630859375,0.000061005353927612305,0.00006103515625,1.00048828125,1.0004882812500002,1.00146484375,65504,65519.99999999,65520,-0.0000000298023223876953125]);for(let v of a)test.record(v);'),
    ('binary-shared-buffer-views', 'let b=new ArrayBuffer(16),a=new Uint8Array(b),w=new Uint16Array(b,2,3),d=new DataView(b,2,6);d.setUint16(0,4660,true);test.record(a.join());test.record(w[0]);test.record(w.byteOffset);test.record(w.buffer===a.buffer);w[1]=22136;test.record(d.getUint16(2,true));test.record(d.buffer===b);'),
    ('binary-copy-versus-view', 'let a=new Uint8Array([1,2,3,4]),b=new Uint8Array(a),s=a.slice(1,3),v=a.subarray(1,3);test.record(b.buffer!==a.buffer);test.record(s.buffer!==a.buffer);test.record(v.buffer===a.buffer);v[0]=9;test.record(a.join());test.record(b.join());test.record(s.join());test.record(v.byteOffset);'),
    ('binary-subarray-negative', 'let a=new Uint16Array([1,2,3,4]);for(let v of [a.subarray(),a.subarray(-2),a.subarray(-9,-1),a.subarray(3,1),a.subarray(Infinity),a.subarray(-Infinity,Infinity)]){test.record(v.join());test.record(v.byteOffset);test.record(v.buffer===a.buffer);}'),
    ('binary-buffer-slice', 'let b=new Uint8Array([1,2,3,4]).buffer;for(let v of [b.slice(),b.slice(-2),b.slice(-9,-1),b.slice(3,1),b.slice(Infinity)]){test.record(new Uint8Array(v).join());test.record(v!==b);}'),
    ('binary-coercion-and-source-length', 'let order="";let a=new Uint8Array({length:{valueOf(){order+="l";return 2.9;}},0:{valueOf(){order+="a";return 258;}},1:{valueOf(){order+="b";return -1;}});test.record(order);test.record(a.join());test.record(new Uint8Array("3").length);test.record(new Uint8Array(false).length);test.record(new Uint8Array({}).length);'),
    ('binary-constructor-source-typed-internal-length', 'let a=new Uint8Array([4,5,6]);Object.defineProperty(a,"length",{value:0});test.record(a.length);test.record(new Uint16Array(a).join());let b=new Uint8Array(4);b.set(a,1);test.record(b.join());'),
    ('binary-null-sources-and-bounds', 'for(let f of [()=>new ArrayBuffer(-1),()=>new ArrayBuffer(Infinity),()=>new Uint8Array(-2),()=>new DataView({}),()=>new DataView(new ArrayBuffer(2),3),()=>new Uint16Array(new ArrayBuffer(5)),()=>new Uint16Array(new ArrayBuffer(8),1),()=>new Uint16Array(new ArrayBuffer(8),2,4),()=>new Uint8Array(2).set(null),()=>Uint8Array.from(null)]){try{f();}catch(e){test.record(e.name);}}'),
    ('binary-outofrange-indexes', 'let a=new Uint8Array([3,4]);for(let k of ["-0","-1","0.5","2","NaN","Infinity"]){test.record(a[k]);a[k]=7;test.record(k in a);test.record(a.hasOwnProperty(k));test.record(delete a[k]);}a["01"]=8;a["1.0"]=9;test.record(a["01"]);test.record(a["1.0"]);test.record(Object.keys(a).join());test.record(a.join());'),
    ('binary-index-conversion-before-invalid-range', 'let a=new Uint8Array(1),n=0,v={valueOf(){n++;return 8;}};a[7]=v;a[-1]=v;a["-0"]=v;a[0]=v;test.record(n);test.record(a[0]);'),
    ('binary-index-prototype-no-fallback', 'let p={0:90,2:92,"-1":93,"01":94},a=new Uint8Array([1,2]);Object.setPrototypeOf(a,p);test.record(a[0]);test.record(a[2]);test.record(a[-1]);test.record(a["01"]);test.record("2" in a);'),
    ('binary-typed-enumeration', 'let a=new Uint8Array([4,7]);a.z=3;a.a=2;test.record(Object.keys(a).join());let keys="";for(let k in a)keys+=k+",";test.record(keys);test.record(JSON.stringify(a));test.record(JSON.stringify(a.buffer));test.record(Array.isArray(a));'),
    ('binary-typed-loop-live-values', 'let a=new Uint8Array([1,2,3]);for(let v of a){test.record(v);if(v===1)a[1]=8;}'),
    ('binary-numeric-descriptors', 'let a=new Uint8Array([3]);let d=Object.getOwnPropertyDescriptor(a,"0");test.record(d.value);test.record(d.writable);test.record(d.enumerable);test.record(d.configurable);test.record(delete a[0]);Object.defineProperty(a,"0",{value:258});test.record(a[0]);test.record(a.propertyIsEnumerable("0"));test.record(Object.getOwnPropertyDescriptor(a,"1"));for(let k of ["writable","enumerable","configurable"]){let d={};d[k]=false;try{Object.defineProperty(a,"0",d);}catch(e){test.record(e.name);}}'),
    ('binary-getter-descriptors-receivers', 'let b=new ArrayBuffer(7),p=Object.getPrototypeOf(Uint8Array.prototype);for(let pair of [[ArrayBuffer.prototype,"byteLength",b],[p,"length",new Uint8Array(3)],[DataView.prototype,"byteOffset",new DataView(b,2)]]){let d=Object.getOwnPropertyDescriptor(pair[0],pair[1]);test.record(typeof d.get);test.record(d.set);test.record(d.enumerable);test.record(d.configurable);test.record(d.get.call(pair[2]));try{d.get.call({});}catch(e){test.record(e.name);}}'),
    ('binary-readonly-fields-and-shadow', 'let a=new Uint8Array(3),b=a.buffer;a.length=9;b.byteLength=20;test.record(a.length);test.record(b.byteLength);try{function f(){"use strict";a.length=5;}f();}catch(e){test.record(e.name);}Object.defineProperty(a,"length",{value:6});test.record(a.length);test.record(a.slice().length);'),
    ('binary-set-overlap-same-kind', 'let a=new Uint8Array([1,2,3,4,5]);test.record(a.set(a.subarray(0,3),1));test.record(a.join());a.set(a.subarray(1,4),0);test.record(a.join());'),
    ('binary-set-overlap-different-kind', 'let b=new ArrayBuffer(8),a=new Uint8Array(b),w=new Uint16Array(b);w[0]=257;w[1]=514;w[2]=771;w[3]=1028;a.set(w,1);test.record(a.join());'),
    ('binary-set-strings-arraylike', 'let a=new Uint8Array(6);a.set("27",1);a.set({0:259,1:-2,length:2.9},3);test.record(a.join());test.record(a.set(12));test.record(a.join());'),
    ('binary-set-bounds-order', 'let a=new Uint8Array(2),log="",s={length:{valueOf(){log+="l";return 3;}},0:{valueOf(){log+="v";return 1;}}};try{a.set(s,{valueOf(){log+="o";return 0;}});}catch(e){test.record(e.name);}test.record(log);test.record(a.join());'),
    ('binary-fill-conversion-order', 'let a=new Int16Array([1,2,3,4]),log="";let v={valueOf(){log+="v";return 7;}},s={valueOf(){log+="s";return -3;}},e={valueOf(){log+="e";return -1;}};test.record(a.fill(v,s,e)===a);test.record(log);test.record(a.join());test.record(a.fill(8,3,1).join());'),
    ('binary-copywithin', 'let a=new Uint8Array([1,2,3,4,5]);test.record(a.copyWithin(1,0,3)===a);test.record(a.join());test.record(a.copyWithin(-2,-3).join());test.record(a.copyWithin(Infinity).join());'),
    ('binary-at-index-search', 'let a=new Float64Array([NaN,-0,2,2,4]);test.record(a.at(-1));test.record(a.at(99));test.record(a.at(NaN));test.record(a.indexOf(NaN));test.record(a.includes(NaN));test.record(a.indexOf(0));test.record(a.indexOf("2"));test.record(a.indexOf(2,-3));test.record(a.lastIndexOf(2));test.record(a.lastIndexOf(2,undefined));test.record(a.lastIndexOf(2,Infinity));test.record(a.lastIndexOf(2,-Infinity));test.record(a.includes(4,-1));'),
    ('binary-empty-search', 'let a=new Uint8Array(0),n=0,o={valueOf(){n++;return 0;}};test.record(a.includes(1,o));test.record(a.indexOf(1,o));test.record(a.lastIndexOf(1,o));test.record(n);'),
    ('binary-join-coercion', 'let a=new Uint8Array([1,2,3]);test.record(a.join());test.record(a.toString());test.record(a.join(null));test.record(a.join({toString(){return "/";}}));'),
    ('binary-callback-index-and-receiver', 'let a=new Uint8Array([1,2,3]),ctx={sum:0};test.record(a.forEach(function(v,i,x){this.sum+=v+i;test.record(x===a);},ctx));test.record(ctx.sum);test.record(a.map((v,i)=>v*100+i).join());test.record(a.filter(v=>v!==2).join());test.record(a.every(v=>v>0));test.record(a.some(v=>v===2));'),
    ('binary-callback-live-mutation', 'let a=new Uint8Array([1,2,3]);test.record(a.map((v,i)=>{if(i===0)a[1]=7;return v+1;}).join());test.record(a.join());'),
    ('binary-callback-find-direction', 'let a=new Uint8Array([2,4,3,4]);test.record(a.find(v=>v===4));test.record(a.findIndex(v=>v===4));test.record(a.findLast(v=>v===4));test.record(a.findLastIndex(v=>v===4));test.record(a.find(v=>false));test.record(a.findIndex(v=>false));'),
    ('binary-reduce', 'let a=new Uint8Array([2,3,4]);test.record(a.reduce((s,v,i,x)=>s+v+i+(x===a?1:0),10));test.record(a.reduce((s,v)=>s-v));test.record(a.reduceRight((s,v)=>s-v));test.record(new Uint8Array(0).reduce((s,v)=>s+v,undefined));try{new Uint8Array(0).reduce((s,v)=>s+v);}catch(e){test.record(e.name);}'),
    ('binary-empty-callbacks', 'let a=new Uint8Array(0);test.record(a.every(()=>false));test.record(a.some(()=>true));test.record(a.map(()=>1).length);test.record(a.filter(()=>true).length);for(let m of ["map","filter","every","some","find","findLast","reduce","forEach"]){try{a[m](2);}catch(e){test.record(e.name);}}'),
    ('binary-default-sort', 'let a=new Float64Array([NaN,4,-0,0,-3,NaN,Infinity,-Infinity]);test.record(a.sort()===a);for(let v of a)test.record(v);test.record(new Int16Array([100,20,3,-1]).sort().join());'),
    ('binary-copying-reverse-sort-with', 'let a=new Uint8Array([3,1,2]);test.record(a.toReversed().join());test.record(a.toSorted().join());test.record(a.with(-1,259).join());test.record(a.join());test.record(a.reverse()===a);test.record(a.join());try{a.with(3,1);}catch(e){test.record(e.name);}'),
    ('binary-typed-static', 'test.record(Uint8Array.of(1,258,-1).join());test.record(Float64Array.of().length);test.record(Uint16Array.from([2,3,4],(v,i)=>v+i).join());test.record(Uint8Array.from("25").join());test.record(Uint8Array.from("😀9").join());test.record(Uint8Array.from({0:7,1:8,length:2}).join());'),
    ('binary-typed-static-mapper-this', 'let ctx={n:3};test.record(Uint8Array.from([1,2],function(v,i){test.record(this===ctx);return v*this.n+i;},ctx).join());try{Uint8Array.from(null,1);}catch(e){test.record(e.name);}'),
    ('binary-data-view-default-endian', 'let b=new ArrayBuffer(12),d=new DataView(b,2,8),a=new Uint8Array(b);test.record(d.setUint32(0,305419896));test.record(a.join());test.record(d.getUint32(0));test.record(d.getUint32(0,true));d.setInt16(4,-2,true);test.record(d.getInt16(4,true));test.record(d.getUint16(4,true));test.record(d.byteOffset);test.record(d.byteLength);'),
    ('binary-data-view-all-types', 'let b=new ArrayBuffer(16),d=new DataView(b);for(let name of ["Int8","Uint8","Int16","Uint16","Int32","Uint32","Float16","Float32","Float64"]){for(let little of [false,true]){d["set"+name](1,-23.5,little);test.record(d["get"+name](1,little));}}'),
    ('binary-data-view-float-negative-zero', 'let d=new DataView(new ArrayBuffer(8));for(let name of ["Float16","Float32","Float64"]){d["set"+name](0,-0,true);test.record(d["get"+name](0,true));d["set"+name](0,Infinity);test.record(d["get"+name](0));d["set"+name](0,NaN);test.record(d["get"+name](0));}'),
    ('binary-data-view-bounds-order', 'let d=new DataView(new ArrayBuffer(3)),log="";try{d.setUint32({valueOf(){log+="i";return 0;}},{valueOf(){log+="v";return 2;}});}catch(e){test.record(e.name);}test.record(log);for(let f of [()=>d.getUint32(0),()=>d.getUint8(-1),()=>d.getUint16(2),()=>d.setUint8(Infinity,1),()=>DataView.prototype.getInt8.call({})]){try{f();}catch(e){test.record(e.name);}}'),
    ('binary-readonly-constants', 'test.record(Number.EPSILON);test.record(Number.MAX_SAFE_INTEGER);test.record(Number.MIN_SAFE_INTEGER);test.record(Number.MAX_VALUE);test.record(Number.MIN_VALUE);test.record(Number.NaN);test.record(Number.POSITIVE_INFINITY);test.record(Number.NEGATIVE_INFINITY);Number.EPSILON=9;test.record(Number.EPSILON);test.record(Object.getOwnPropertyDescriptor(Uint8Array,"BYTES_PER_ELEMENT").writable);'),
    ('number-static-noncoercing-tests', 'for(let v of [0,-0,1.2,NaN,Infinity,-Infinity,9007199254740991,9007199254740992,"3",null,undefined,true,{}]){test.record(Number.isFinite(v));test.record(Number.isNaN(v));test.record(Number.isInteger(v));test.record(Number.isSafeInteger(v));}test.record(Number.parseInt===parseInt);test.record(Number.parseFloat===parseFloat);'),
])

NEGATIVE.extend([
    ('binary-buffer-byte-bound-atomic','try{new ArrayBuffer(4194305);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('binary-typed-byte-bound-atomic','try{new Float64Array(524289);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('binary-numeric-host-conversion-atomic','try{new Uint8Array([test]);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('binary-native-host-source-atomic','try{new Uint8Array(test);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('binary-native-host-set-source-atomic','try{new Uint8Array(2).set(test);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('binary-native-host-dataview-number-atomic','try{new DataView(new ArrayBuffer(8)).setFloat64(0,test);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('binary-resizable-explicitly-unsupported-atomic','try{new ArrayBuffer(4,{maxByteLength:8});}catch(e){test.record(1);}finally{test.record(2);}'),
    ('binary-enumeration-bound-atomic','let a=new Uint8Array(4097);try{Object.keys(a);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('binary-callback-loop-bound-atomic','let a=new Uint8Array(2);try{a.map(()=>{while(true){}});}catch(e){test.record(1);}finally{test.record(2);}'),
    ('binary-valueof-recursion-bound-atomic','let a=new Uint8Array(1),v={valueOf(){a[0]=v;return 1;}};try{a[0]=v;}catch(e){test.record(1);}finally{test.record(2);}'),
    ('binary-copy-work-bound-atomic','let a=new Uint8Array(1024);try{while(true)a.copyWithin(0);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('binary-output-string-bound-atomic','let a=new Uint8Array(65536);try{a.fill(255);a.join();}catch(e){test.record(1);}finally{test.record(2);}'),
    ('binary-custom-species-unsupported-atomic','let a=new Uint8Array(2);a.constructor={};try{a.slice();}catch(e){test.record(1);}finally{test.record(2);}'),
])


CASES.extend([
    ('binary-from-iterable-snapshot', 'let a=[1,2,3];test.record(Uint8Array.from(a,(v,i)=>{if(i===0)a[1]=8;return v;}).join());test.record(a.join());let b=new Uint8Array([4,5,6]);Object.defineProperty(b,"length",{value:0});test.record(Uint8Array.from(b).join());'),
    ('binary-constructor-iterable-snapshot', 'let a=[{valueOf(){a[1]=9;return 1;}},2];test.record(new Uint8Array(a).join());test.record(a[1]);'),
    ('binary-native-iterators', 'let a=new Uint8Array([5,7]);for(let m of ["keys","values","entries"]){let i=a[m]();test.record(Object.prototype.toString.call(i));test.record(JSON.stringify(i.next()));test.record(JSON.stringify(i.next()));test.record(JSON.stringify(i.next()));test.record(JSON.stringify(i.next()));}'),
    ('binary-iterator-views-live', 'let a=new Uint8Array([1,2,3]),i=a.values();test.record(i.next().value);a[1]=8;for(let v of i)test.record(v);test.record(i.next().done);'),
    ('collection-constructor-basics', 'for(let c of [Map,Set]){let a=new c();test.record(a.size);test.record(a instanceof c);test.record(Object.getPrototypeOf(a)===c.prototype);test.record(Object.prototype.toString.call(a));test.record(c.length);test.record(new c(null).size);try{c();}catch(e){test.record(e.name);}try{new c(8);}catch(e){test.record(e.name);}}'),
    ('collection-map-identity-and-types', 'let o={},p={},f=()=>1,m=new Map();test.record(m.set(o,1)===m);m.set(p,2);m.set(f,3);m.set("1",4);m.set(1,5);m.set(undefined,6);m.set(null,7);m.set(false,8);test.record(m.size);for(let k of [o,p,f,"1",1,undefined,null,false,{}]){test.record(m.has(k));test.record(m.get(k));}test.record(m.set(o,9).size);test.record(m.get(o));'),
    ('collection-map-samevaluezero', 'let m=new Map([[NaN,1],[-0,2],[0,3],[NaN,4]]);test.record(m.size);test.record(m.get(NaN));test.record(m.get(-0));for(let k of m.keys())test.record(k);'),
    ('collection-map-constructor-entries', 'let m=new Map([["a",1],["b",2],["a",3],{}]);test.record(m.size);for(let pair of m)test.record(JSON.stringify(pair));test.record(new Map(m).get("a"));try{new Map([1]);}catch(e){test.record(e.name);}'),
    ('collection-map-delete-clear', 'let m=new Map([["a",1],["b",2]]);test.record(m.delete("a"));test.record(m.delete("a"));test.record(m.has("a"));test.record(m.size);test.record(m.clear());test.record(m.size);test.record(m.has("b"));test.record(m.set("z",9).get("z"));'),
    ('collection-map-order-update-reinsert', 'let m=new Map([["a",1],["b",2],["c",3]]);m.set("a",7);m.delete("b");m.set("b",8);for(let p of m)test.record(p.join());'),
    ('collection-map-iterators', 'let m=new Map([["a",1],["b",2]]);for(let method of ["keys","values","entries"]){let i=m[method]();test.record(Object.prototype.toString.call(i));test.record(JSON.stringify(i.next()));test.record(JSON.stringify(i.next()));test.record(JSON.stringify(i.next()));}'),
    ('collection-map-iterator-live-and-closed', 'let m=new Map([[1,"a"],[2,"b"]]),i=m.keys();test.record(i.next().value);m.delete(2);m.set(3,"c");test.record(i.next().value);test.record(i.next().done);m.set(4,"d");test.record(i.next().done);'),
    ('collection-map-clear-active-iterator', 'let m=new Map([[1,2],[3,4]]),i=m.entries();test.record(i.next().value.join());m.clear();m.set(5,6);test.record(i.next().value.join());test.record(i.next().done);'),
    ('collection-map-foreach-this-and-order', 'let m=new Map([["a",1],["b",2]]),ctx={sum:0};test.record(m.forEach(function(v,k,x){test.record(k);test.record(x===m);this.sum+=v;},ctx));test.record(ctx.sum);'),
    ('collection-map-foreach-mutation', 'let m=new Map([[1,1],[2,2],[3,3]]);m.forEach((v,k)=>{test.record(k);if(k===1){m.delete(2);m.set(4,4);}});let n=new Map([[1,1],[2,2]]);n.forEach((v,k)=>{test.record(k);if(k===1){n.clear();n.set(7,7);}});'),
    ('collection-set-identity-and-values', 'let o={},s=new Set([o,o,{},1,"1",NaN,NaN,-0,0]);test.record(s.size);test.record(s.has(o));test.record(s.has({}));test.record(s.has(NaN));test.record(s.add(7)===s);test.record(s.delete(1));test.record(s.size);test.record(Set.prototype.keys===Set.prototype.values);'),
    ('collection-set-string-and-typed', 'for(let x of new Set("a😀aa😀b"))test.record(x);for(let x of new Set(new Uint8Array([1,2,1])))test.record(x);test.record(new Set(new Set([3,4,3])).size);'),
    ('collection-set-entries-and-foreach', 'let s=new Set([2,4]);for(let p of s.entries())test.record(p.join());let ctx={n:0};test.record(s.forEach(function(v,k,x){test.record(v===k);test.record(x===s);this.n+=v;},ctx));test.record(ctx.n);'),
    ('collection-set-order-delete-reinsert', 'let s=new Set([1,2,3]);s.add(1);s.delete(2);s.add(2);for(let v of s)test.record(v);test.record(s.clear());test.record(s.size);'),
    ('collection-method-validation', 'for(let f of [()=>Map.prototype.get.call({}),()=>Map.prototype.set.call(new Set(),1,2),()=>Set.prototype.add.call(new Map(),1),()=>new Map().forEach(2),()=>new Set().forEach(2),()=>new Map().keys().next.call({}),()=>new Set().keys().next.call(new Map().keys())]){try{f();}catch(e){test.record(e.name);}}'),
    ('collection-size-getter-and-shadow', 'let m=new Map([[1,2]]),d=Object.getOwnPropertyDescriptor(Map.prototype,"size");test.record(d.get.call(m));test.record(d.set);test.record(d.enumerable);test.record(d.configurable);m.size=8;test.record(m.size);Object.defineProperty(m,"size",{value:9});test.record(m.size);test.record(m.get(1));try{d.get.call(new Set());}catch(e){test.record(e.name);}'),
    ('collection-no-own-data-properties', 'let m=new Map([["a",1]]),s=new Set(["a"]);test.record(Object.keys(m).join());test.record(Object.keys(s).join());test.record(JSON.stringify(m));test.record(m.a);m.x=2;test.record(Object.keys(m).join());test.record(m.size);'),
    ('collection-constructor-adder-lookup', 'let original=Map.prototype.set;Map.prototype.set=function(k,v){test.record(k);return original.call(this,k,v+1);};let m=new Map([["a",1],["b",2]]);test.record(m.get("b"));Map.prototype.set=1;try{new Map([]);}catch(e){test.record(e.name);}test.record(new Map(null).size);'),
    ('collection-iterator-self-consumption', 'let s=new Set([1,2,3]),i=s.values();test.record(i.next().value);test.record(new Set(i).size);test.record(i.next().done);'),
    ('collection-binary-from-set', 'test.record(Uint8Array.from(new Set([3,1,2,1])).join());test.record(new Uint16Array(new Set([3,1,2])).join());test.record(Uint8Array.from(new Set([2,3]).values(),v=>v+1).join());'),
    ('collection-array-native-iterators', 'let a=[3,,5];for(let p of a.entries())test.record(JSON.stringify(p));let i=a.values();test.record(i.next().value);a.push(7);test.record(i.next().value);test.record(i.next().value);test.record(i.next().value);test.record(i.next().done);a.push(9);test.record(i.next().done);'),
    ('collection-map-host-identity', 'let m=new Map([[test,3]]);test.record(m.get(test));test.record(m.has(test));test.record(m.delete(test));'),
    ('large-script-small-function-work-accounting', 'let padding="'+('p'*80000)+'";function f(a){var b=a+1;return b;}let n=0;for(let i=0;i<150;i++)n+=f(i);test.record(n);'),
])
NEGATIVE.extend([
    ('collection-map-entry-limit-atomic', 'try{let m=new Map();for(let i=0;i<4097;i++)m.set(i,i);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('collection-set-entry-limit-atomic', 'try{let m=new Set();for(let i=0;i<4097;i++)m.add(i);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('collection-callback-infinite-additions-atomic', 'let s=new Set([0]);try{s.forEach(v=>s.add(v+1));}catch(e){test.record(1);}finally{test.record(2);}'),
    ('collection-callback-revisit-work-bound-atomic', 'let m=new Map([[0,1]]);try{m.forEach((v,k)=>{m.delete(k);m.set(k,v);});}catch(e){test.record(1);}finally{test.record(2);}'),
    ('collection-adder-recursion-bound-atomic', 'Set.prototype.add=function(v){return new Set([v]);};try{new Set([1]);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('collection-host-iteration-unsupported-atomic', 'try{new Set(test);}catch(e){test.record(1);}finally{test.record(2);}'),
])


CASES.extend([
    ('encoding-constructor-and-properties', 'let e=new TextEncoder(),d=new TextDecoder();test.record(e.encoding);test.record(d.encoding);test.record(d.fatal);test.record(d.ignoreBOM);test.record(e instanceof TextEncoder);test.record(d instanceof TextDecoder);test.record(Object.prototype.toString.call(e));test.record(Object.prototype.toString.call(d));test.record(TextEncoder.length);test.record(TextDecoder.length);test.record(e.encode.length);test.record(e.encodeInto.length);test.record(d.decode.length);'),
    ('encoding-constructor-new-required', 'for(let c of [TextEncoder,TextDecoder]){try{c();}catch(e){test.record(e.name);}}'),
    ('encoding-utf8-roundtrip', 'let e=new TextEncoder(),d=new TextDecoder();for(let s of ["","abc","café","日本語","😀","A😀éZ","\u0000\u007f\u0080\u07ff\u0800\uffff"]){let a=e.encode(s);test.record(a instanceof Uint8Array);test.record(a.join());test.record(d.decode(a));}'),
    ('encoding-encoder-coercion', 'let e=new TextEncoder(),d=new TextDecoder();for(let v of [undefined,null,123,true,{toString(){return "ok";}}])test.record(d.decode(e.encode(v)));test.record(e.encode().length);'),
    ('encoding-encodeinto-boundaries', 'let e=new TextEncoder();for(let n of [0,1,2,3,4,5,6,7,8]){let a=new Uint8Array(n);a.fill(9);test.record(JSON.stringify(e.encodeInto("A😀éZ",a)));test.record(a.join());}'),
    ('encoding-encodeinto-shared-view', 'let a=new Uint8Array(8);a.fill(7);let v=a.subarray(2,6);test.record(JSON.stringify(new TextEncoder().encodeInto("éa",v)));test.record(a.join());'),
    ('encoding-encodeinto-validation', 'let e=new TextEncoder(),n=0,s="abc";for(let a of [new Uint8ClampedArray(3),new Int8Array(3),new ArrayBuffer(3),{},null]){try{e.encodeInto(s,a);}catch(x){test.record(x.name);}}test.record(n);try{e.encodeInto();}catch(x){test.record(x.name);}try{TextEncoder.prototype.encode.call({},s);}catch(x){test.record(x.name);}test.record(n);'),
    ('encoding-buffer-sources', 'let a=new Uint8Array([120,65,66,67,121]),d=new TextDecoder();test.record(d.decode(a.buffer));test.record(d.decode(a.subarray(1,4)));test.record(d.decode(new DataView(a.buffer,1,3)));test.record(d.decode(new Uint16Array(new Uint8Array([65,66,67,68]).buffer)));test.record(d.decode());test.record(d.decode(undefined));'),
    ('encoding-input-validation', 'let d=new TextDecoder();for(let a of [null,"abc",{},[65],1]){try{d.decode(a);}catch(e){test.record(e.name);}}for(let o of [1,"x",true]){try{new TextDecoder("utf-8",o);}catch(e){test.record(e.name);}try{d.decode(undefined,o);}catch(e){test.record(e.name);}}'),
    ('encoding-labels', 'for(let label of ["UTF8"," \tutf-8\r\n","unicode-1-1-utf-8","unicode11utf8","x-unicode20utf8","UTF-16","unicodeFFFE","ucs-2","ASCII","iso-8859-1","latin1","windows-1252"]){test.record(new TextDecoder(label).encoding);}for(let label of ["","bad-codec",null,"utf-8\u00a0"]){try{new TextDecoder(label);}catch(e){test.record(e.name);}}'),
    ('encoding-options-and-readonly', 'let d=new TextDecoder("utf8",{fatal:1,ignoreBOM:"yes"});test.record(d.fatal);test.record(d.ignoreBOM);d.encoding="x";d.fatal=false;test.record(d.encoding);test.record(d.fatal);test.record(new TextDecoder(undefined,null).encoding);test.record(new TextEncoder("not-used").encoding);'),
    ('encoding-native-getter-descriptors', 'for(let p of [[TextEncoder.prototype,"encoding",new TextEncoder()],[TextDecoder.prototype,"fatal",new TextDecoder("utf8",{fatal:true})]]){let d=Object.getOwnPropertyDescriptor(p[0],p[1]);test.record(d.enumerable);test.record(d.configurable);test.record(d.set);test.record(d.get.call(p[2]));try{d.get.call({});}catch(e){test.record(e.name);}}test.record(Object.getOwnPropertyDescriptor(TextEncoder.prototype,"encode").enumerable);'),
    ('encoding-utf8-invalid-sequences', 'let d=new TextDecoder();for(let a of [[128],[192,175],[224,128,175],[237,160,128],[244,144,128,128],[245,128,128,128],[226,130],[226,130,65],[226,65,130],[240,159,152],[239,191,191],[0,255,65]])test.record(d.decode(new Uint8Array(a)));'),
    ('encoding-utf8-fatal', 'let d=new TextDecoder("utf-8",{fatal:true});for(let a of [[255],[226,130],[237,160,128],[192,175],[244,144,128,128]]){try{d.decode(new Uint8Array(a));}catch(e){test.record(e.name);}test.record(d.decode(new Uint8Array([65])));}'),
    ('encoding-utf8-stream-every-split', 'let a=new TextEncoder().encode("Aé日😀Z");for(let n=0;n<=a.length;n++){let d=new TextDecoder();let s=d.decode(a.subarray(0,n),{stream:true});s+=d.decode(a.subarray(n));test.record(s);}'),
    ('encoding-utf8-stream-single-byte', 'let d=new TextDecoder(),s="";for(let b of new TextEncoder().encode("é😀日"))s+=d.decode(new Uint8Array([b]),{stream:true});s+=d.decode();test.record(s);'),
    ('encoding-utf8-stream-truncated-flush', 'let d=new TextDecoder();test.record(d.decode(new Uint8Array([226,130]),{stream:true}));test.record(d.decode());test.record(d.decode(new Uint8Array([65])));let f=new TextDecoder("utf8",{fatal:true});test.record(f.decode(new Uint8Array([226]),{stream:true}));try{f.decode();}catch(e){test.record(e.name);}test.record(f.decode(new Uint8Array([66])));'),
    ('encoding-utf8-stream-error-recovery', 'let d=new TextDecoder("utf8",{fatal:true});try{d.decode(new Uint8Array([65,255,66]),{stream:true});}catch(e){test.record(e.name);}test.record(d.decode(new Uint8Array([67]),{stream:true}));test.record(d.decode());'),
    ('encoding-bom-default-and-ignore', 'let a=new Uint8Array([239,187,191,65,239,187,191]);test.record(new TextDecoder().decode(a));test.record(new TextDecoder("utf8",{ignoreBOM:true}).decode(a));let d=new TextDecoder();test.record(d.decode(a));test.record(d.decode(a));'),
    ('encoding-bom-split-and-state', 'for(let cut=0;cut<5;cut++){let a=new Uint8Array([239,187,191,65]),d=new TextDecoder();test.record(d.decode(a.subarray(0,cut),{stream:true})+d.decode(a.subarray(cut)));}let d=new TextDecoder();test.record(d.decode(new Uint8Array(0),{stream:true}));test.record(d.decode(new Uint8Array([239]),{stream:true}));test.record(d.decode(new Uint8Array([187,191]),{stream:true}));test.record(d.decode(new Uint8Array([239,187,191,66])));'),
    ('encoding-utf16-le-be', 'for(let pair of [["utf-16le",[255,254,65,0,61,216,0,222,233,0]],["utf-16be",[254,255,0,65,216,61,222,0,0,233]]]){let a=new Uint8Array(pair[1]);for(let n=0;n<=a.length;n++){let d=new TextDecoder(pair[0]);test.record(d.decode(a.subarray(0,n),{stream:true})+d.decode(a.subarray(n)));}test.record(new TextDecoder(pair[0],{ignoreBOM:true}).decode(a));}'),
    ('encoding-utf16-invalid', 'let d=new TextDecoder("utf-16le");for(let a of [[65],[0,216],[0,216,65],[0,216,65,0],[0,220,65,0],[0,216,0,216],[0,216,0,220]])test.record(d.decode(new Uint8Array(a)));'),
    ('encoding-utf16-fatal-and-flush', 'let d=new TextDecoder("utf-16be",{fatal:true});test.record(d.decode(new Uint8Array([216]),{stream:true}));test.record(d.decode(new Uint8Array([61,222]),{stream:true}));test.record(d.decode(new Uint8Array([0])));try{d.decode(new Uint8Array([220,0]));}catch(e){test.record(e.name);}test.record(d.decode(new Uint8Array([0,65])));'),
    ('encoding-windows1252-all-bytes', 'let a=new Uint8Array(256);for(let i=0;i<256;i++)a[i]=i;test.record(new TextDecoder("windows-1252").decode(a));test.record(new TextDecoder("ascii",{fatal:true}).decode(new Uint8Array([128,129,141,143,144,157,255])));'),
    ('encoding-stream-input-copied', 'let a=new Uint8Array([226,130]),d=new TextDecoder();test.record(d.decode(a,{stream:true}));a[0]=65;a[1]=66;test.record(d.decode(new Uint8Array([172])));'),
])
NEGATIVE.extend([
    ('encoding-host-coercion-unsupported-atomic','try{new TextEncoder().encode(test);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('encoding-host-options-unsupported-atomic','try{new TextDecoder("utf8",test);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('encoding-coercion-recursion-bound-atomic','let e=new TextEncoder(),v={toString(){return e.encode(v);}};try{e.encode(v);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('encoding-decoder-output-bound-atomic','let a=new Uint8Array(131073);a.fill(65);try{new TextDecoder().decode(a);}catch(e){test.record(1);}finally{test.record(2);}'),
])


CASES.extend([
    ('integrity-preventextensions', 'let a={x:1};test.record(Object.isExtensible(a));test.record(Object.preventExtensions(a)===a);test.record(Object.isExtensible(a));a.y=2;a.x=3;test.record(a.y);test.record(a.x);test.record(delete a.x);test.record(Object.isSealed(a));test.record(Object.isFrozen(a));'),
    ('integrity-seal', 'let a={x:1};test.record(Object.seal(a)===a);test.record(Object.isSealed(a));test.record(Object.isFrozen(a));a.x=2;a.y=3;test.record(a.x);test.record(a.y);test.record(delete a.x);let d=Object.getOwnPropertyDescriptor(a,"x");test.record(d.writable);test.record(d.configurable);Object.defineProperty(a,"x",{value:4,writable:false});test.record(a.x);test.record(Object.isFrozen(a));'),
    ('integrity-freeze', 'let a={x:1,child:{v:2}};test.record(Object.freeze(a)===a);test.record(Object.isFrozen(a));test.record(Object.isSealed(a));a.x=9;a.child.v=3;test.record(a.x);test.record(a.child.v);try{Object.defineProperty(a,"x",{value:8});}catch(e){test.record(e.name);}Object.defineProperty(a,"x",{value:1});test.record(a.x);'),
    ('integrity-strict-writes', 'let a=Object.freeze({x:1}),b=Object.preventExtensions({x:1});for(let f of [()=>Object.defineProperty(b,"y",{value:1}),()=>Object.assign(b,{y:2}),()=>Object.setPrototypeOf(b,{})]){try{f();}catch(e){test.record(e.name);}}function strict(){"use strict";try{a.x=3;}catch(e){test.record(e.name);}try{b.y=3;}catch(e){test.record(e.name);}try{delete a.x;}catch(e){test.record(e.name);}}strict();test.record(Object.setPrototypeOf(b,Object.getPrototypeOf(b))===b);'),
    ('integrity-inherited-write', 'let p=Object.freeze({x:1}),o=Object.create(p);o.x=2;test.record(o.x);test.record(o.hasOwnProperty("x"));Object.defineProperty(o,"x",{value:3});test.record(o.x);'),
    ('integrity-primitives', 'for(let v of [undefined,null,false,3,"abc"]){test.record(Object.preventExtensions(v)===v);test.record(Object.seal(v)===v);test.record(Object.freeze(v)===v);test.record(Object.isExtensible(v));test.record(Object.isSealed(v));test.record(Object.isFrozen(v));}'),
    ('integrity-array-sealed-length', 'let a=Object.seal([1,2,3]);a[0]=7;a[3]=4;a.length=1;test.record(a.join());test.record(a.length);test.record(Object.isSealed(a));test.record(Object.isFrozen(a));try{a.pop();}catch(e){test.record(e.name);}try{a.push(8);}catch(e){test.record(e.name);}test.record(a.length);a.length=5;test.record(a.length);'),
    ('integrity-array-frozen-length', 'let a=Object.freeze([1,2]),e=Object.freeze([]);a[0]=3;a.length=0;test.record(a.join());test.record(a.length);test.record(Object.getOwnPropertyDescriptor(a,"length").writable);test.record(Object.isFrozen(a));for(let f of [()=>a.pop(),()=>a.push(3),()=>e.pop(),()=>e.push()]){try{f();}catch(e){test.record(e.name);}}'),
    ('integrity-array-nonextensible-holes', 'let a=Object.preventExtensions([1,,3]);a[1]=2;test.record(a.join());a.length=5;test.record(a.length);a.length=1;test.record(a.length);test.record(a.join());'),
    ('integrity-collection-data-is-not-property-data', 'let m=Object.freeze(new Map()),s=Object.freeze(new Set());m.set(1,2);s.add(3);test.record(m.get(1));test.record(s.has(3));test.record(Object.isFrozen(m));test.record(Object.isFrozen(s));'),
    ('integrity-buffer-data-remains-writable', 'let b=Object.freeze(new ArrayBuffer(4)),d=Object.freeze(new DataView(b));d.setUint32(0,42);test.record(d.getUint32(0));test.record(Object.isFrozen(b));test.record(Object.isFrozen(d));'),
    ('integrity-typed-array-nonempty-rejects-seal-freeze', 'for(let method of ["seal","freeze"]){let a=new Uint8Array([1,2]);try{Object[method](a);}catch(e){test.record(e.name);}test.record(Object.isExtensible(a));test.record(Object.isSealed(a));test.record(Object.isFrozen(a));a[0]=7;a.extra=2;test.record(a[0]);test.record(a.extra);}test.record(Object.isFrozen(Object.freeze(new Uint8Array(0))));'),
    ('integrity-functions', 'function f(){return 7;}Object.freeze(f);f.custom=2;test.record(f());test.record(f.custom);test.record(Object.isFrozen(f));test.record(delete f.prototype);'),
])
BROWSER_CASES.extend([
    ('global-language-constructor-descriptor', 'let d=Object.getOwnPropertyDescriptor(window,"Map");test.record(d.value===Map);test.record(d.enumerable);test.record(d.writable);test.record(d.configurable);Object.defineProperty(window,"Map",{value:function Replacement(){},writable:true,configurable:true});test.record(Map===window.Map);test.record(Map.name);test.record(Object.getOwnPropertyDescriptor(window,"Map").enumerable);'),
    ('global-language-readonly-and-strict', 'Object.defineProperty(window,"Map",{value:7,writable:false});Map=8;window.Map=9;test.record(Map);try{function f(){"use strict";Map=3;}f();}catch(e){test.record(e.name);}test.record(Object.getOwnPropertyDescriptor(window,"Map").writable);'),
    ('global-language-configurable-delete', 'test.record(delete window.Map);test.record(typeof Map);test.record("Map" in window);test.record(Object.getOwnPropertyDescriptor(window,"Map"));window.Map=6;test.record(Map);test.record(delete window.Map);'),
    ('global-language-delete-identifier', 'test.record(delete Map);test.record(typeof Map);test.record(delete Array);test.record(typeof Array);'),
    ('global-language-constants-descriptors', 'for(let name of ["NaN","Infinity","undefined"]){let d=Object.getOwnPropertyDescriptor(window,name);test.record(d.writable);test.record(d.configurable);test.record(d.enumerable);test.record(delete window[name]);}NaN=3;Infinity=7;undefined=8;test.record(Number.isNaN(NaN));test.record(Infinity);test.record(undefined);try{Object.defineProperty(window,"NaN",{value:3});}catch(e){test.record(e.name);}Object.defineProperty(window,"NaN",{value:NaN});'),
    ('global-language-descriptor-batch', 'Object.defineProperties(window,{Map:{value:3},Set:{value:4,writable:false}});test.record(Map);test.record(Set);Set=5;test.record(Set);'),
    ('global-var-preserves-language-descriptor', 'Object.defineProperty(window,"Map",{value:5,writable:false});\n//---NEXT SCRIPT---\nvar Map=6;test.record(Map);test.record(Object.getOwnPropertyDescriptor(window,"Map").writable);'),
])
NEGATIVE.extend([
    ('integrity-host-object-unsupported-atomic','try{Object.freeze(test);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('integrity-host-inspection-unsupported-atomic','try{Object.isFrozen(test);}catch(e){test.record(1);}finally{test.record(2);}'),
])


CASES.extend([
    ('array-extra-shift-unshift', 'let a=[1,,3];test.record(a.shift());test.record(a.length);test.record(a.hasOwnProperty(0));test.record(a.join());test.record(a.unshift(7,8));test.record(a.join());test.record(a.hasOwnProperty(2));test.record([].shift());test.record([].unshift());'),
    ('array-extra-shift-inherited', 'Array.prototype[1]=9;let a=[1,,3];test.record(a.shift());test.record(a[0]);test.record(a.hasOwnProperty(0));test.record(a.join());delete Array.prototype[1];'),
    ('array-extra-splice-basic', 'let a=[1,2,3,4];test.record(a.splice(1,2,8,9,10).join());test.record(a.join());test.record(a.splice(-2,1).join());test.record(a.join());test.record(a.splice(1).join());test.record(a.join());test.record(a.splice().join());test.record(a.join());'),
    ('array-extra-splice-holes', 'let a=[1,,3,,5];let b=a.splice(1,3,7);test.record(b.length);test.record(Object.keys(b).join());test.record(b.join());test.record(a.join());let c=[1,,3];c.splice(0,0,8,9);test.record(c.length);test.record(Object.keys(c).join());test.record(c.join());'),
    ('array-extra-splice-coercion-order', 'let a=[1,2,3],s="";test.record(a.splice({valueOf(){s+="a";return 1;}},{valueOf(){s+="b";return 1;}},9).join());test.record(s);test.record(a.join());test.record([1,2].splice(0,undefined).length);'),
    ('array-extra-map-filter-foreach', 'let a=[1,,3],ctx={sum:0};test.record(a.forEach(function(v,i,x){this.sum+=v+i;test.record(x===a);},ctx));test.record(ctx.sum);let m=a.map((v,i)=>v*2+i);test.record(m.length);test.record(m.join());test.record(Object.keys(m).join());test.record(a.filter(v=>true).join());'),
    ('array-extra-callback-mutation', 'let a=[1,2,3];test.record(a.map((v,i)=>{if(i===0){a[1]=8;a.push(4);delete a[2];}return v;}).join());test.record(a.join());'),
    ('array-extra-some-every', 'test.record([1,2,3].some(v=>v===2));test.record([1,2,3].every(v=>v>0));test.record([].every(()=>false));test.record([].some(()=>true));let calls=0;test.record(Array(3).every(()=>{calls++;return false;}));test.record(calls);'),
    ('array-extra-find-holes-and-direction', 'let a=[,2,3,2];test.record(a.find((v,i)=>{test.record(i);return v===undefined;}));test.record(a.findIndex(v=>v===undefined));test.record(a.findLastIndex(v=>v===2));test.record(a.findLast(v=>v<3));test.record(a.findIndex(()=>false));'),
    ('array-extra-reduce-holes-and-direction', 'let a=[,2,,3];test.record(a.reduce((s,v,i)=>s+v+i,1));test.record(a.reduce((s,v)=>s-v));test.record(a.reduceRight((s,v)=>s-v));test.record([].reduce((s,v)=>s+v,undefined));try{Array(3).reduce((s,v)=>s+v);}catch(e){test.record(e.name);}'),
    ('array-extra-callback-rejection', 'for(let name of ["map","filter","forEach","some","every","find","findIndex","findLast","findLastIndex","reduce","reduceRight"]){try{[][name](2);}catch(e){test.record(e.name);}}'),
    ('array-extra-at-lastindexof', 'let a=[1,,2,1,undefined];test.record(a.at(-2));test.record(a.at(99));test.record(a.lastIndexOf(1));test.record(a.lastIndexOf(1,undefined));test.record(a.lastIndexOf(undefined));test.record([,].lastIndexOf(undefined));test.record(a.lastIndexOf(1,-3));test.record(a.lastIndexOf(1,Infinity));'),
    ('array-extra-fill-object-and-range', 'let o={n:1},a=Array(4);test.record(a.fill(o,1,-1)===a);test.record(a[1]===a[2]);test.record(Object.keys(a).join());test.record(a.length);a.fill(7,3,1);test.record(a[3]);'),
    ('array-extra-copywithin-holes', 'let a=[1,,3,4,5];test.record(a.copyWithin(1,0,3)===a);test.record(a.join());test.record(Object.keys(a).join());test.record(a.copyWithin(0,-2).join());'),
    ('array-extra-reverse-holes', 'let a=[1,,3,,];test.record(a.reverse()===a);test.record(a.length);test.record(a.join());test.record(Object.keys(a).join());'),
    ('array-extra-concat-spread-holes', 'let o={x:1},a=[1,,3],b=a.concat([4,,6],o,new Uint8Array([7,8]));test.record(b.length);test.record(Object.keys(b).join());test.record(b[6]===o);test.record(b[7] instanceof Uint8Array);test.record(a.length);'),
    ('array-extra-sort-lexical-undefined-holes', 'let a=[30,3,,undefined,200,4];test.record(a.sort()===a);test.record(a.join());test.record(Object.keys(a).join());test.record(["😀","\uffff","a","Z"].sort().join());'),
    ('array-extra-sort-comparator-stable', 'let a=[{v:2,n:"a"},{v:1,n:"b"},{v:2,n:"c"},{v:1,n:"d"}];a.sort((a,b)=>a.v-b.v);test.record(a.map(x=>x.n).join());test.record([3,1,2].sort((a,b)=>b-a).join());test.record([2,1].sort(()=>NaN).join());'),
    ('array-extra-sort-comparator-return-object', 'test.record([3,1,2].sort((a,b)=>({valueOf(){return a-b;}})).join());try{[1,2].sort(null);}catch(e){test.record(e.name);}let a=[2,1];try{a.sort(()=>{throw 9;});}catch(e){test.record(e);}test.record(a.join());'),
    ('array-extra-copying-methods-dense-holes', 'let a=[3,,1];let b=a.toReversed(),c=a.toSorted(),d=a.with(1,8),e=a.toSpliced(1,1,9,7);test.record(b.join());test.record(Object.keys(b).join());test.record(c.join());test.record(Object.keys(c).join());test.record(d.join());test.record(e.join());test.record(a.join());test.record(Object.keys(a).join());'),
    ('array-extra-copying-coercion-and-errors', 'test.record([1,2,3].with(-1,9).join());test.record([1,2,3].toSpliced().join());test.record([1,2,3].toSpliced(1).join());try{[].with(0,1);}catch(e){test.record(e.name);}try{[1].toSorted(false);}catch(e){test.record(e.name);}'),
    ('array-extra-integrity-mutations-throw', 'for(let f of [()=>Object.freeze([]).shift(),()=>Object.freeze([]).unshift(),()=>Object.freeze([1,2]).splice(1,1),()=>Object.freeze([2,1]).sort(),()=>Object.freeze([1,2]).reverse(),()=>Object.freeze([1,2]).fill(3),()=>Object.freeze([1,2]).copyWithin(1,0)]){try{f();}catch(e){test.record(e.name);}}test.record(Object.freeze([2,1]).toSorted().join());'),
    ('array-extra-method-arities', 'for(let name of ["shift","unshift","splice","map","reverse","copyWithin","toReversed","toSpliced","with","sort","reduce"])test.record(Array.prototype[name].length);'),
    ('binary-comparator-sort', 'let a=new Uint16Array([3,1,2]);test.record(a.sort((x,y)=>y-x)===a);test.record(a.join());test.record(a.toSorted((x,y)=>x-y).join());test.record(a.join());test.record(new Float64Array([3,1,2]).sort((x,y)=>({valueOf(){return x-y;}})).join());'),
    ('integrity-frozen-regexp-state', 'let r=Object.freeze(/a/g);try{r.test("a");}catch(e){test.record(e.name);}test.record(r.lastIndex);test.record(Object.freeze(/a/).test("a"));try{"a".match(r);}catch(e){test.record(e.name);}'),
])
NEGATIVE.extend([
    ('array-extra-callback-infinite-bound-atomic','try{[1].map(()=>{while(true){}});}catch(e){test.record(1);}finally{test.record(2);}'),
    ('array-extra-sort-callback-infinite-bound-atomic','try{[2,1].sort(()=>{while(true){}});}catch(e){test.record(1);}finally{test.record(2);}'),
    ('array-extra-splice-length-bound-atomic','let a=Array(8192);try{a.splice(0,0,1);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('array-extra-dense-fill-property-bound-atomic','let a=Array(8192);try{a.fill(1);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('binary-sort-callback-recursion-bound-atomic','let a=new Uint8Array([2,1]);function compare(){return a.sort(compare);}try{a.sort(compare);}catch(e){test.record(1);}finally{test.record(2);}'),
])


# Arguments are real ordinary/exotic objects, not arrays or copied parameter lists.
CASES.extend([
    ('arguments-basic-shape', 'function f(a){test.record(arguments.length);test.record(arguments[0]);test.record(arguments[1]);test.record(Array.isArray(arguments));test.record(Object.prototype.toString.call(arguments));test.record(Object.getPrototypeOf(arguments)===Object.prototype);test.record(Object.keys(arguments).join());test.record(arguments.callee===f);test.record(arguments.caller);}f(3,4);'),
    ('arguments-mapped-in-both-directions', 'function f(a,b){a=7;test.record(arguments[0]);arguments[1]=9;test.record(b);arguments[0]++;test.record(a);b++;test.record(arguments[1]);}f(1,2);'),
    ('arguments-strict-unmapped', 'function f(a){"use strict";a=7;test.record(arguments[0]);arguments[0]=9;test.record(a);try{arguments.callee;}catch(e){test.record(e.name);}test.record(Object.prototype.toString.call(arguments));}f(1);'),
    ('arguments-strict-callee-poison-in-sloppy-caller', 'var a=(function(){"use strict";return arguments;})(3);for(let op of [()=>a.callee,()=>{"use strict";a.callee=9;}]){try{op();}catch(e){test.record(e.name);}}let d=Object.getOwnPropertyDescriptor(a,"callee");test.record(d.get===d.set);test.record(d.get.length);test.record(d.get.name);test.record(d.enumerable);test.record(d.configurable);test.record(Object.isExtensible(d.get));try{d.set.call({});}catch(e){test.record(e.name);}'),
    ('arguments-strict-accessor-simple-descriptor', 'var a=(function(){"use strict";return arguments;})();test.record(Object.defineProperty(a,"callee",{})===a);test.record(Object.defineProperty(a,"callee",{enumerable:false,configurable:false})===a);try{Object.defineProperty(a,"callee",{value:1});}catch(e){test.record(e.name);}'),
    ('arguments-own-descriptors', 'function f(x){x=7;for(let n of ["0","length","callee"]){let d=Object.getOwnPropertyDescriptor(arguments,n);test.record(d.enumerable);test.record(d.writable);test.record(d.configurable);}test.record(Object.getOwnPropertyDescriptor(arguments,"0").value);}f(1);'),
    ('arguments-delete-breaks-map', 'function f(a){delete arguments[0];a=9;test.record(arguments[0]);arguments[0]=7;test.record(a);test.record(arguments[0]);test.record(arguments.length);}f(1);'),
    ('arguments-readonly-descriptor-snapshots', 'function f(a){a=7;Object.defineProperty(arguments,"0",{writable:false});a=9;test.record(arguments[0]);test.record(a);arguments[0]=2;test.record(arguments[0]);test.record(Object.getOwnPropertyDescriptor(arguments,"0").writable);}f(1);'),
    ('arguments-define-value-and-writable-break', 'function f(a){Object.defineProperty(arguments,"0",{value:7});test.record(a);Object.defineProperty(arguments,"0",{value:9,writable:false});test.record(a);a=3;test.record(arguments[0]);}f(1);'),
    ('arguments-nonconfigurable-map-stays-live', 'function f(a){Object.defineProperty(arguments,"0",{configurable:false});test.record(delete arguments[0]);a=9;test.record(arguments[0]);arguments[0]=7;test.record(a);}f(1);'),
    ('arguments-freeze-snapshots-and-disconnects', 'function f(a){a=7;Object.freeze(arguments);a=9;arguments[0]=4;test.record(arguments[0]);test.record(a);test.record(Object.isFrozen(arguments));}f(1);'),
    ('arguments-seal-and-prevent-extension-preserve-map', 'function f(a){Object.seal(arguments);a=9;test.record(arguments[0]);arguments[0]=7;test.record(a);test.record(Object.isSealed(arguments));test.record(Object.isFrozen(arguments));test.record(delete arguments[0]);}f(1);function g(x){Object.preventExtensions(arguments);x=5;test.record(arguments[0]);arguments[2]=7;test.record(arguments[2]);}g(2);'),
    ('arguments-length-and-extra-index-independent', 'function f(a){arguments.length=0;test.record(a);test.record(arguments[0]);arguments[4]=8;test.record(arguments.length);test.record(arguments[4]);test.record(Object.keys(arguments).join());}f(1);'),
    ('arguments-missing-and-extra-parameters', 'function f(a,b){test.record(arguments.length);arguments[1]=7;test.record(b);b=9;test.record(arguments[1]);}f(1);function g(a){arguments[2]=7;test.record(arguments.length);test.record(arguments[2]);}g(1,2,3);'),
    ('arguments-inherited-read-does-not-alias-write', 'function f(a){let o=Object.create(arguments);a=9;test.record(o[0]);o[0]=7;test.record(a);test.record(o[0]);test.record(arguments[0]);}f(1);'),
    ('arguments-arrow-captures-outer-live-object', 'function f(a){let g=()=>arguments;a=7;return g;}let g=f(1,2);let a=g();test.record(a[0]);test.record(a.length);test.record(a===g());a[0]=9;test.record(g()[0]);'),
    ('arguments-arrow-and-nested-normal-shadow', 'function f(a){let arrow=()=>arguments[0];function nested(){return arguments[0];}test.record(arrow(9));test.record(nested(8));test.record(arguments[0]);}f(7);'),
    ('arguments-parameter-shadows-object', 'function f(arguments){test.record(arguments);return ()=>arguments;}test.record(f(7)());test.record(((arguments)=>arguments)(9));'),
    ('arguments-hoisted-function-and-lexical-shadows', 'function f(){test.record(typeof arguments);function arguments(){return 3;}test.record(arguments());}f(1);function g(){let arguments=7;return arguments;}test.record(g(1));'),
    ('arguments-var-keeps-object-and-assignment-replaces', 'function f(a){var arguments;test.record(arguments[0]);arguments=7;test.record(arguments);a=9;test.record(arguments);}f(1);function g(){var arguments=8;test.record(arguments);}g(1);'),
    ('arguments-direct-eval-and-closures', 'function f(a){test.record(eval("arguments[0]"));eval("a=7");test.record(arguments[0]);return ()=>eval("arguments[0]");}let f2=f(1);test.record(f2());'),
    ('arguments-duplicate-final-parameter-only', 'function f(a,a){a=7;test.record(arguments[0]);test.record(arguments[1]);arguments[0]=9;test.record(a);arguments[1]=8;test.record(a);}f(1,2);'),
    ('arguments-duplicate-missing-final-parameter', 'function f(a,a){test.record(a);a=7;test.record(arguments[0]);arguments[0]=9;test.record(a);}f(1);'),
    ('arguments-apply-and-slice', 'function sum(a,b,c){return a+b+c;}function f(){test.record(sum.apply(null,arguments));test.record(Array.prototype.slice.call(arguments,1).join());test.record(Array.prototype.join.call(arguments,"|"));}f(1,2,3);'),
    ('arguments-live-iterators-and-collections', 'function f(a,b){let values=[];for(let x of arguments){values.push(x);if(values.length===1){arguments[1]=9;arguments[2]=7;arguments.length=3;}}test.record(values.join());test.record(new Set(arguments).size);test.record(new Uint8Array(arguments).join());test.record(Uint8Array.from(arguments).join());}f(1,2);'),
    ('arguments-array-map-and-mutators', 'function f(a,b){test.record(Array.prototype.map.call(arguments,x=>x*2).join());test.record(Array.prototype.shift.call(arguments));test.record(a);test.record(b);test.record(arguments.length);test.record(Array.prototype.unshift.call(arguments,7));test.record(a);test.record(b);test.record(Array.prototype.join.call(arguments));}f(1,2);'),
    ('arguments-mapped-object-json', 'function f(a){a=9;test.record(JSON.stringify(arguments));Object.defineProperty(arguments,"0",{enumerable:false});test.record(JSON.stringify(arguments));}f(1,2);'),
    ('arguments-recursive-call-identities', 'function f(n){let here=arguments;if(n>0){let nested=f(n-1);test.record(nested!==here);test.record(here[0]);}return here;}test.record(f(2)[0]);'),
    ('arraylike-borrowed-generic-reads', 'let o={0:3,2:1,length:3};test.record(Array.prototype.slice.call(o).join());test.record(Array.prototype.map.call(o,x=>x*2).join());test.record(Array.prototype.filter.call(o,x=>x>1).join());test.record(Array.prototype.reduce.call(o,(a,b)=>a+b,0));test.record(Array.prototype.includes.call(o,undefined));test.record(Array.prototype.indexOf.call(o,undefined));test.record(Array.prototype.at.call(o,-1));'),
    ('arraylike-borrowed-generic-writes', 'let o={0:3,1:1,length:2};test.record(Array.prototype.push.call(o,2));test.record(Array.prototype.pop.call(o));test.record(Array.prototype.unshift.call(o,5));test.record(Array.prototype.shift.call(o));test.record(Array.prototype.join.call(o));Array.prototype.splice.call(o,1,1,8,7);test.record(Array.prototype.join.call(o));Array.prototype.reverse.call(o);test.record(Array.prototype.join.call(o));'),
    ('arraylike-length-conversion-once', 'let calls=0,o={0:3,1:1,length:{valueOf(){calls++;return 2;}}};test.record(Array.prototype.map.call(o,x=>x+1).join());test.record(calls);test.record(Array.prototype.slice.call(o,0).join());test.record(calls);test.record(Array.prototype.toSorted.call(o).join());test.record(calls);'),
    ('arraylike-concat-does-not-read-length', 'let o={length:{valueOf(){throw 7;}}};let a=Array.prototype.concat.call(o,[1]);test.record(a[0]===o);test.record(a[1]);'),
    ('arraylike-sort-validation-before-length', 'let o={length:{valueOf(){throw 7;}}};try{Array.prototype.sort.call(o,3);}catch(e){test.record(e.name);}'),
    ('arraylike-generic-iterator-live-length', 'let o={0:"a",length:1},it=Array.prototype.values.call(o);test.record(it.next().value);o[1]="b";o.length=2;test.record(it.next().value);test.record(it.next().done);o.length=3;test.record(it.next().done);'),
    ('arraylike-borrowed-typed-mutators', 'let t=new Uint8Array([10,2,3]);test.record(Array.prototype.sort.call(t).join());test.record(Array.prototype.map.call(t,x=>x+1).join());try{Array.prototype.pop.call(t);}catch(e){test.record(e.name);}test.record(t.join());'),
    ('arraylike-join-converts-ordinary-values', 'let o={0:{toString(){return "cat";}},1:null,2:"tab",length:3};test.record(Array.prototype.join.call(o,{toString(){return "|";}}));'),
])
NEGATIVE.extend([
    ('arguments-strict-duplicate-parameter-atomic', 'test.record(1);function f(a,a){"use strict";}'),
    ('arguments-method-duplicate-parameter-atomic', 'test.record(1);var o={f(a,a){}};'),
    ('arguments-host-still-opaque-atomic', 'function f(a){try{Object.getPrototypeOf(arguments[0]);}catch(e){test.record(1);}finally{test.record(2);}}f(test);'),
    ('arraylike-native-host-borrowing-unsupported-atomic', 'try{Array.prototype.slice.call(test);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('arraylike-length-bound-atomic', 'try{Array.prototype.slice.call({length:8193});}catch(e){test.record(1);}finally{test.record(2);}'),
    ('arraylike-length-coercion-bound-atomic', 'try{Array.prototype.values.call({length:{valueOf(){while(true){}}}}).next();}catch(e){test.record(1);}finally{test.record(2);}'),
])


# 0.32: real Search logs exposed flat conditional dispatch depth and Error
# concatenation. These are original semantic fixtures, not copied site scripts.
# Ordinary conversions are compared for order, receiver, hint and exceptions;
# the negative fixtures retain explicit capability and resource boundaries.
CASES.extend([
    ('conditional-flat-false-chain', 'let n=0;test.record(' + '(++n<0)?missing():'*512 + 'n);test.record(n);'),
    ('conditional-flat-true-chain', 'test.record(' + 'true?'*512 + '7' + ':missing()'*512 + ');'),
    ('conditional-state-dispatch', 'let u=511,total=0;for(;u!==-1;)' + ''.join('u=='+str(i)+'?(total=u,u=-1):' for i in range(512)) + 'missing();test.record(total);'),
    ('conditional-right-associativity-and-assignment', 'let a=0,b=0;test.record(true?false?1:2:3);test.record(false?1:true?2:3);test.record(a=b=false?5:7);test.record(a);test.record(b);test.record(false?a=1:b=3);test.record(a);test.record(b);'),
    ('conditional-arrows-and-in-true-arm', 'let f=true?(x=>x+1):(x=>x-1),n=0;for(let ok=true?"a" in {a:1}:false;ok;ok=false)n=f(4);test.record(n);test.record((false?x=>x+1:x=>x-1)(8));'),
    ('conditional-inherited-in-context', 'let n=0;for(let ok=false?false:true?"x" in {x:1}:false;ok;ok=false)n++;test.record(n);'),
    ('conditional-comma-middle-is-invalid', 'test.record("must not run");true?1,2:3;'),
    ('conditional-invalid-assignment-target', 'test.record("must not run");(true?1:2)=3;'),
    ('ordinary-default-object-concatenation', 'test.record("value="+{});test.record({}+7);test.record([]+[]);test.record([1,2]+3);test.record(String(/a/g));'),
    ('ordinary-error-concatenation', 'for(let e of [Error(),Error("bad"),TypeError("broken"),new RangeError("bound")]){test.record("error="+e);test.record(String(e));}'),
    ('ordinary-default-and-string-hints', 'let log="",o={valueOf(){log+="v";return 7;},toString(){log+="s";return "text";}};test.record(""+o);test.record(String(o));test.record(Number(o));test.record(log);'),
    ('ordinary-operand-evaluation-before-conversion', 'let log="",a={valueOf(){log+="a";return 2;}};function b(){log+="b";return {valueOf(){log+="c";return 3;}};}test.record(a+b());test.record(log);'),
    ('ordinary-live-conversion-fallback', 'let log="",o={valueOf(){log+="v";this.toString=function(){log+="s";return "new";};return this;},toString(){return "old";}};test.record(""+o);test.record(log);'),
    ('ordinary-noncallable-and-nonprimitive-methods', 'for(let o of [{valueOf:3,toString(){return "ok";}},{valueOf(){return {};},toString(){return {}; }},Object.create(null)]){try{test.record(""+o);}catch(e){test.record(e.name);}}'),
    ('ordinary-conversion-receiver-and-throws', 'let marker={},o={valueOf(){test.record(this===o);throw marker;}};try{o+1;}catch(e){test.record(e===marker);}o={toString(){throw 9;}};try{String(o);}catch(e){test.record(e);}'),
    ('ordinary-loose-equality-coercion-boundary', 'let calls=0,o={valueOf(){calls++;return 0;}};for(let v of [0,"0",false,null,undefined,{},o])test.record(o==v);test.record(calls);test.record(o===0);test.record(o!==o);'),
    ('ordinary-relational-conversion-order', 'let log="",a={valueOf(){log+="a";return "11";}},b={valueOf(){log+="b";return "2";}};test.record(a<b);test.record(a>b);test.record(a<=b);test.record(a>=b);test.record(log);'),
    ('ordinary-unary-binary-update', 'let o={valueOf(){return 4;}};test.record(+o);test.record(-o);test.record(~o);test.record(o*2);test.record(o<<1);test.record(o&3);let a=o;test.record(a++);test.record(a);let b=o;test.record(--b);'),
    ('ordinary-compound-conversion-order', 'let log="",o={x:{valueOf(){log+="v";return 2;}}};function rhs(){log+="r";o.x=99;return 3;}test.record(o.x+=rhs());test.record(o.x);test.record(log);'),
    ('ordinary-function-custom-conversion', 'function f(){}f.valueOf=function(){return 8;};f.toString=function(){return "fn";};test.record(f+2);test.record(String(f));test.record(f==8);'),
    ('ordinary-error-live-name-message', 'let e=Error("old"),log="";e.name={toString(){log+="n";e.message={toString(){log+="m";return "changed";}};return "Custom";}};test.record(String(e));test.record(log);'),
    ('ordinary-error-borrowed-and-constructor', 'test.record(new Error({toString(){return "message";}}).message);for(let o of [{},{name:"",message:{toString(){return "only";}}},{name:{toString(){return "N";}},message:""}])test.record(Error.prototype.toString.call(o));'),
    ('ordinary-numeric-string-builtins', 'let o={valueOf(){return 6;},toString(){return "12.5tail";}};test.record(Number(o));test.record(isNaN(o));test.record(parseInt(o));test.record(parseFloat(o));test.record(encodeURIComponent({toString(){return "a b";}}));test.record(decodeURIComponent({toString(){return "a%20b";}}));'),
    ('ordinary-parseint-input-before-radix', 'let log="";test.record(parseInt({toString(){log+="s";return "11";}},{valueOf(){log+="r";return 2;}}));test.record(log);'),
    ('ordinary-generic-string-method-receiver', 'test.record(String.prototype.trim.call({toString(){return "  cat  ";}}));test.record(String.prototype.toUpperCase.call({toString(){return "cat";}}));'),
    ('ordinary-math-conversion-order-and-nan', 'let log="";test.record(Math.abs({valueOf(){return -2;}}));test.record(Math.min({valueOf(){log+="a";return NaN;}},{valueOf(){log+="b";return 1;}}));test.record(log);'),
    ('ordinary-former-property-key-arithmetic', 'let key={toString(){test.record("convert");return "x";}};try{test.record(key+"");}catch(e){test.record(e.name);}finally{test.record("finally");}'),
])

NEGATIVE.extend([
    ('conditional-pending-frame-bound-atomic', 'test.record(1);'+'false?0:'*4097+'1;'),
    ('conditional-shared-pending-frame-bound-atomic', 'test.record(1);'+'false?0:'*2048+'('+'false?0:'*2049+'1);'),
    ('assignment-pending-frame-bound-atomic', 'test.record(1);var x;'+'x='*4097+'1;'),
    ('ordinary-error-conversion-cycle-bound-atomic', 'let e=Error();e.name=e;try{String(e);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('ordinary-valueof-conversion-cycle-bound-atomic', 'let o={valueOf(){return +o;}};try{o+1;}catch(e){test.record(1);}finally{test.record(2);}'),
    ('ordinary-conversion-loop-bound-atomic', 'let o={toString(){while(true){}}};try{String(o);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('ordinary-native-host-coercion-still-denied-atomic', 'try{""+test;}catch(e){test.record(1);}finally{test.record(2);}'),
])



CASES.extend([
    ('large-bootstrap-source', '/*'+'original bundle '*26000+'*/test.record(7);'),
    ('dense-bootstrap-tokens', ';'*70000+'test.record(9);'),
])

# Flat else-if chains are statements, not nested condition expressions. These
# original fixtures independently check their branch semantics against Node;
# no captured page or access-check program is reproduced or executed here.
def else_if_fixture(branches, fallback=None):
    source = " else ".join("if(" + condition + ")" + body
                           for condition, body in branches)
    return source if fallback is None else source + " else " + fallback


CASES.extend([
    ('else-if-150-condition-order-selected-body',
     'function choose(target){let seen=[],bodies=0,result=-1;'
     + else_if_fixture([(f'(seen.push({i}),target==={i})', f'{{bodies++;result={i};}}')
                        for i in range(150)], '{bodies++;result=150;}')
     + 'test.record(seen.join());test.record(bodies);test.record(result);}'
       'choose(0);choose(75);choose(149);choose(150);'),
    ('else-if-1000-last-branch',
     'let value=999,result=-1;'
     + else_if_fixture([(f'value==={i}', f'result={i};') for i in range(1000)], 'missing();')
     + 'test.record(result);'),
    ('else-if-1000-no-match-no-fallback',
     'let seen=0;'
     + else_if_fixture([('(++seen)<0', 'missing();') for _ in range(1000)])
     + 'test.record(seen);'),
    ('else-if-unselected-branches-stay-inert',
     'let value=73;'
     + else_if_fixture([(f'value==={i}', 'test.record("selected");' if i == 73 else 'missing();')
                        for i in range(150)], 'missing();')),
    ('else-if-dangling-else-binds-nearest-if',
     'function choose(value,inner){'
     + else_if_fixture([(f'value==={i}', 'test.record("earlier");') for i in range(149)]
                       + [('value===149', 'if(inner)test.record("inner true");else test.record("inner false");')],
                       'test.record("outer else");')
     + '}choose(149,false);choose(149,true);choose(150,false);'),
    ('else-if-dangling-else-inner-chain-with-outer-else',
     'function choose(outer,value){if(outer)'
     + else_if_fixture([(f'value==={i}', f'test.record({i});') for i in range(150)], 'test.record("inner fallback");')
     + 'else test.record("outer fallback");}'
       'choose(true,149);choose(true,150);choose(false,149);'),
    ('else-if-labelled-break-preserves-finally',
     'let value=149,trace="";done:{try{'
     + else_if_fixture([(f'value==={i}', '{trace+="selected";break done;}') for i in range(150)], 'missing();')
     + '}finally{trace+="-finally";}trace+="-unreachable";}test.record(trace);'),
    ('else-if-labelled-if-break-target',
     'let value=149,trace="";done:'
     + else_if_fixture([(f'value==={i}', '{trace="selected";break done;}') for i in range(150)], 'missing();')
     + 'test.record(trace);'),
    ('else-if-labelled-fallback-is-not-flattened-away',
     'let value=150,trace="";'
     + else_if_fixture([(f'value==={i}', 'missing();') for i in range(150)],
                       'tail:if(value===150){trace="tail";break tail;}else missing();')
     + 'test.record(trace);'),
    ('else-if-continue-preserves-finally',
     'let sum=0;outer:for(let pass=0;pass<3;pass++){let value=149;try{'
     + else_if_fixture([(f'value==={i}', '{sum+=pass;continue outer;}') for i in range(150)], 'missing();')
     + '}finally{sum+=10;}sum+=1000;}test.record(sum);'),
    ('else-if-return-preserves-finally',
     'function choose(value){try{'
     + else_if_fixture([(f'value==={i}', f'return {i};') for i in range(150)], 'return -1;')
     + '}finally{test.record("cleanup");}}test.record(choose(149));test.record(choose(150));'),
    ('else-if-throw-reaches-catch-and-finally',
     'let value=149;try{'
     + else_if_fixture([(f'value==={i}', f'throw {i};') for i in range(150)], 'throw -1;')
     + '}catch(error){test.record(error);}finally{test.record("cleanup");}'),
    ('else-if-condition-throw-skips-remaining-work',
     'let seen=0,bodies=0;function check(i){seen++;if(i===73)throw "condition";return false;}try{'
     + else_if_fixture([(f'check({i})', '{bodies++;}') for i in range(150)], '{bodies++;}')
     + '}catch(error){test.record(error);}test.record(seen);test.record(bodies);'),
    ('else-if-selected-block-retains-lexical-scope',
     'let value=149,x="outer";'
     + else_if_fixture([(f'value==={i}', f'{{let x={i};test.record(x);}}') for i in range(150)])
     + 'test.record(x);'),
    ('else-if-unselected-var-is-still-hoisted',
     'function choose(){test.record(value);'
     + else_if_fixture([('false', f'var value={i};') for i in range(150)])
     + 'test.record(value);}choose();'),
])

COMPLETION_CASES.extend([
    ('else-if-selected-expression-completion',
     '7;let value=149;'
     + else_if_fixture([(f'value==={i}', f'{{{i};}}') for i in range(150)], '{999;}')),
    ('else-if-fallback-expression-completion',
     '7;' + else_if_fixture([('false', '{3;}') for _ in range(150)], '{42;}')),
    ('else-if-no-match-completion-is-undefined',
     '7;' + else_if_fixture([('false', '{3;}') for _ in range(150)])),
    ('else-if-selected-empty-block-completion-is-undefined',
     '7;let value=149;'
     + else_if_fixture([(f'value==={i}', '{}') for i in range(150)], '{42;}')),
    ('else-if-selected-var-completion-is-undefined',
     '7;let value=149;'
     + else_if_fixture([(f'value==={i}', '{var result=4;}') for i in range(150)], '{42;}')),
    ('else-if-labelled-break-value-completion',
     '7;let value=149;done:{'
     + else_if_fixture([(f'value==={i}', '{9;break done;}') for i in range(150)], '{42;}')
     + '}'),
])

NEGATIVE.extend([
    ('else-if-flat-chain-bound-atomic',
     'test.record(1);' + else_if_fixture([('false', ';') for _ in range(4097)])),
    ('else-if-shared-pending-branch-bound-atomic',
     'test.record(1);'
     + else_if_fixture([('false', ';') for _ in range(2048)],
                       '{' + else_if_fixture([('false', ';') for _ in range(2049)]) + '}')),
    ('else-if-nested-block-depth-atomic',
     'test.record(1);'
     + else_if_fixture([('false', ';') for _ in range(150)], '{' * 65 + ';' + '}' * 65)),
    ('else-if-nested-condition-depth-atomic',
     'test.record(1);'
     + else_if_fixture([('false', ';') for _ in range(150)]
                       + [('(' * 65 + 'true' + ')' * 65, ';')])),
    ('else-if-nested-then-depth-atomic',
     'test.record(1);'
     + else_if_fixture([('false', ';') for _ in range(150)], 'if(true)' * 65 + ';')),
])


# Function bodies execute separately from the expressions that create them.
# These original fixtures put a 20-call body inside 44 surrounding calls: each
# unit is shallow enough on its own, but parsing both on one native stack used
# to reject the source. No captured page or access-check code is used here.
def nested_keep_calls(expression, count=44):
    return 'keep(' * count + expression + ')' * count


_KEEP_FUNCTION = 'function keep(value){return value;}'
_DEFERRED_RETURN = 'return ' + nested_keep_calls('7', 20) + ';'
_DEFERRED_FACTORY = nested_keep_calls('function(){' + _DEFERRED_RETURN + '}')

CASES.extend([
    ('deferred-function-call-argument-independent-depth',
     _KEEP_FUNCTION + 'var factory=' + _DEFERRED_FACTORY + ';test.record(factory());'),
    ('deferred-block-arrow-call-argument-independent-depth',
     _KEEP_FUNCTION + 'var factory='
     + nested_keep_calls('()=>{' + _DEFERRED_RETURN + '}')
     + ';test.record(factory());'),
    ('deferred-method-receiver-and-nonconstructibility',
     _KEEP_FUNCTION + 'var object='
     + nested_keep_calls('{value:7,read(){return ' + nested_keep_calls('this.value', 20) + ';}}')
     + ';test.record(object.read());test.record(object.read.hasOwnProperty("prototype"));'
       'try{new object.read();}catch(error){test.record(error.name);}'),
    ('deferred-function-closure-and-private-name',
     _KEEP_FUNCTION + 'function make(initial){let current=initial;return '
     + nested_keep_calls('function step(amount){current+=' + nested_keep_calls('amount', 20) + ';return current;}')
     + ';}var next=make(3);test.record(next(4));test.record(next(2));test.record(typeof step);'),
    ('deferred-function-inherited-strict-this-and-arguments',
     _KEEP_FUNCTION + 'function make(){"use strict";return '
     + nested_keep_calls('function(value){value=' + nested_keep_calls('8', 20)
                         + ';test.record(arguments[0]);return this;}')
     + ';}var read=make();test.record(read.call(null,3));'),
    ('deferred-function-local-strictness-does-not-leak',
     _KEEP_FUNCTION + 'var strict='
     + nested_keep_calls('function(){"use strict";return ' + nested_keep_calls('this', 20) + ';}')
     + ';function sloppy(value,value){return value;}'
       'test.record(strict.call(null));test.record(sloppy(1,2));'),
    ('deferred-block-arrow-lexical-this-and-arguments',
     _KEEP_FUNCTION + 'function make(value){return '
     + nested_keep_calls('(extra)=>{return ' + nested_keep_calls('this.offset+arguments[0]+extra', 20) + ';}')
     + ';}var next=make.call({offset:5},2);test.record(next.call({offset:99},3));'),
    ('deferred-function-hoist-and-declaration-binding',
     _KEEP_FUNCTION + 'var factory='
     + nested_keep_calls('function(){local();return ' + nested_keep_calls('local', 20)
                         + ';function local(){local=7;}}')
     + ';test.record(factory());'),
    ('deferred-function-and-arrow-for-head-allow-in',
     _KEEP_FUNCTION + 'for(var factory='
     + nested_keep_calls('function(){return ' + nested_keep_calls('"key" in {key:7}', 20) + ';}')
     + ';;){test.record(factory());break;}for(var factory='
     + nested_keep_calls('()=>{return ' + nested_keep_calls('"key" in {key:7}', 20) + ';}')
     + ';;){test.record(factory());break;}'),
    ('deferred-function-label-reuse-and-outer-restoration',
     _KEEP_FUNCTION + 'outer:for(var pass=0;pass<1;pass++){var factory='
     + nested_keep_calls('function(){var count=0;outer:for(var i=0;i<3;i++){count++;continue outer;}'
                         + 'return ' + nested_keep_calls('count', 20) + ';}')
     + ';test.record(factory());continue outer;}test.record(pass);'),
    ('deferred-function-brace-tokens-in-strings-regex-and-comments',
     _KEEP_FUNCTION + 'var factory='
     + nested_keep_calls('function(){var text="}{",pattern=/[{}]/;/* } { */'
                         'var object={read(){return "{";}};test.record(pattern.test(text));'
                         + 'return ' + nested_keep_calls('object.read()', 20) + ';}')
     + ';test.record(factory());'),
    ('deferred-function-handler-wrapper',
     _KEEP_FUNCTION + 'var factory=' + _DEFERRED_FACTORY + ';test.record(factory());', True, True),
    ('deferred-function-direct-eval',
     'eval(' + json.dumps(_KEEP_FUNCTION + 'var factory=' + _DEFERRED_FACTORY
                          + ';test.record(factory());') + ');'),
    ('deferred-function-lexical-nesting-boundary-64',
     'function nested(){' * 64 + 'return 7;' + '}' * 64 + 'test.record("parsed");'),
    ('deferred-function-many-sibling-bodies',
     ''.join(f'function sibling{i}(){{return {i};}}' for i in range(96))
     + 'test.record(sibling0());test.record(sibling95());'),
    ('deferred-function-early-errors-remain-syntax-errors',
     'for(let snippet of ' + json.dumps([
         'test.record("must not run");var f=function(a,a){"use strict";};',
         'test.record("must not run");function eval(){"use strict";}',
         'test.record("must not run");var f=function arguments(){"use strict";};',
         '"use strict";test.record("must not run");function outer(){function inner(eval){}}',
         'test.record("must not run");var f=()=>{"use strict";arguments=7;};',
         'test.record("must not run");function unused(){let same;let same;}',
         'test.record("must not run");var object={method(a,a){}};',
         'test.record("must not run");var f=(a,a)=>{};',
     ]) + '){try{eval(snippet);}catch(error){test.record(error.name);}}'),
])

NEGATIVE.extend([
    ('deferred-function-unused-invalid-body-atomic',
     'test.record(1);' + _KEEP_FUNCTION + 'var unused='
     + nested_keep_calls('function(){return 7;var invalid=;}') + ';'),
    ('deferred-function-unused-lexical-conflict-atomic',
     'test.record(1);' + _KEEP_FUNCTION + 'var unused='
     + nested_keep_calls('function(){let same;var same;}') + ';'),
    ('deferred-function-strict-duplicate-parameters-atomic',
     'test.record(1);' + _KEEP_FUNCTION + 'var unused='
     + nested_keep_calls('function(a,a){"use strict";}') + ';'),
    ('deferred-function-strict-declaration-name-atomic',
     'test.record(1);' + _KEEP_FUNCTION + 'var unused='
     + nested_keep_calls('function(){function eval(){"use strict";}}') + ';'),
    ('deferred-function-strict-expression-name-atomic',
     'test.record(1);' + _KEEP_FUNCTION + 'var unused='
     + nested_keep_calls('function arguments(){"use strict";}') + ';'),
    ('deferred-function-inherited-strict-parameter-atomic',
     'test.record(1);' + _KEEP_FUNCTION + 'var unused='
     + nested_keep_calls('function(){"use strict";function inner(eval){}}') + ';'),
    ('deferred-function-break-cannot-cross-boundary-atomic',
     'test.record(1);' + _KEEP_FUNCTION + 'outer:{var unused='
     + nested_keep_calls('function(){break outer;}') + ';}'),
    ('deferred-arrow-continue-cannot-cross-boundary-atomic',
     'test.record(1);' + _KEEP_FUNCTION + 'outer:while(false){var unused='
     + nested_keep_calls('()=>{continue outer;}') + ';}'),
    ('deferred-function-unlabelled-break-cannot-cross-boundary-atomic',
     'test.record(1);' + _KEEP_FUNCTION + 'while(false){var unused='
     + nested_keep_calls('function(){break;}') + ';}'),
    ('deferred-function-call-depth-within-body-stays-bounded-atomic',
     'test.record(1);' + _KEEP_FUNCTION + 'var unused=function(){return '
     + nested_keep_calls('7', 65) + ';};'),
    ('deferred-function-top-level-call-depth-stays-bounded-atomic',
     'test.record(1);' + _KEEP_FUNCTION + nested_keep_calls('7', 65) + ';'),
    ('deferred-function-block-depth-within-body-stays-bounded-atomic',
     'test.record(1);function unused(){' + '{' * 65 + ';' + '}' * 65 + '}'),
    ('deferred-function-lexical-nesting-limit-atomic',
     'test.record(1);' + 'function nested(){' * 65 + 'return 7;' + '}' * 65),
    ('deferred-block-arrow-lexical-nesting-limit-atomic',
     'test.record(1);' + 'var nested=()=>{' * 65 + 'return 7;' + '};' * 65),
    ('deferred-function-eval-preserves-active-depth-seed-atomic',
     _KEEP_FUNCTION + 'try{'
     + nested_keep_calls('eval(' + json.dumps('function unused(){return '
                                             + nested_keep_calls('7', 30) + ';}1;') + ')', 40)
     + ';}catch(error){test.record("caught");}finally{test.record("finally");}'),
])


# These are original Symbol fixtures, including explicit disposal and an
# ordinary iterator polyfill. They do not execute captured page source or claim
# support for using declarations, automatic disposal, or native @@iterator.
CASES.extend([
    ('symbol-unique-identity-type-and-truthiness',
     'let a=Symbol("same"),b=Symbol("same");test.record(typeof a);test.record(a);'
     'test.record(a===a);test.record(a===b);test.record(a==b);test.record(a=="Symbol(same)");'
     'test.record(Boolean(a));test.record(!a);test.record(a==null);'),
    ('symbol-optional-descriptions',
     'for(let s of [Symbol(),Symbol(undefined),Symbol(""),Symbol("café😀"),Symbol(null),Symbol(7)])'
     '{test.record(s.description);test.record(String(s));}'),
    ('symbol-explicit-string-and-prototype-methods',
     'let s=Symbol("query");test.record(String(s));test.record(s.toString());'
     'test.record(Symbol.prototype.toString.call(s));test.record(s.valueOf()===s);'
     'test.record(Symbol.prototype.valueOf.call(s)===s);'),
    ('symbol-implicit-text-conversion-errors',
     'let s=Symbol("query");for(let action of [()=>""+s,()=>s+"",()=>"x".concat(s),'
     '()=>encodeURIComponent(s),()=>parseInt(s),()=>Symbol(s),()=>Symbol.for(s)])'
     '{try{action();}catch(error){test.record(error.name);}}'),
    ('symbol-implicit-numeric-conversion-errors',
     'let s=Symbol();for(let action of [()=>Number(s),()=>+s,()=>-s,()=>s*2,()=>s<1,'
     '()=>1<s,()=>isNaN(s),()=>Math.abs(s),()=>{let x=s;return x++;}])'
     '{try{action();}catch(error){test.record(error.name);}}'),
    ('symbol-description-object-conversion-order',
     'let log="",s=Symbol({toString(){log+="description";return "value";}});'
     'test.record(log);test.record(s.description);'
     'for(let action of [()=>String({toString(){return s;}}),()=>Symbol({toString(){return s;}})])'
     '{try{action();}catch(error){test.record(error.name);}}'),
    ('symbol-registry-identity-and-keys',
     'let a=Symbol.for("shared"),b=Symbol.for("shared");test.record(a===b);'
     'test.record(a===Symbol("shared"));test.record(Symbol.keyFor(a));'
     'test.record(Symbol.keyFor(Symbol("shared")));test.record(Symbol.keyFor(Symbol.dispose));'
     'test.record(Symbol.keyFor(Symbol.for("")));test.record(Symbol.keyFor(Symbol.for(undefined)));'
     'test.record(Symbol.for({toString(){return "shared";}})===a);'),
    ('symbol-constructor-and-keyfor-type-errors',
     'for(let action of [()=>new Symbol(),()=>Symbol.keyFor("x"),()=>Symbol.keyFor(7),'
     '()=>Symbol.keyFor({}),()=>Symbol.keyFor(undefined)])'
     '{try{action();}catch(error){test.record(error.name);}}'),
    ('symbol-prototype-method-receiver-errors',
     'for(let receiver of [undefined,null,1,"symbol",{}])'
     '{try{Symbol.prototype.toString.call(receiver);}catch(error){test.record(error.name);}'
     'try{Symbol.prototype.valueOf.call(receiver);}catch(error){test.record(error.name);}}'),
    ('symbol-description-getter-descriptor',
     'let descriptor=Object.getOwnPropertyDescriptor(Symbol.prototype,"description");'
     'test.record(typeof descriptor.get);test.record(descriptor.set);'
     'test.record(descriptor.enumerable);test.record(descriptor.configurable);'
     'test.record(descriptor.get.call(Symbol("name")));test.record(descriptor.get.call(Symbol()));'
     'try{descriptor.get.call({});}catch(error){test.record(error.name);}'),
    ('symbol-dispose-stable-identity-and-descriptor',
     'let s=Symbol.dispose,d=Object.getOwnPropertyDescriptor(Symbol,"dispose");'
     'test.record(typeof s);test.record(s===Symbol.dispose);test.record(s.description);'
     'test.record(String(s));test.record(s===Symbol("Symbol.dispose"));'
     'test.record(s===Symbol.for("Symbol.dispose"));test.record(d.value===s);'
     'test.record(d.writable);test.record(d.enumerable);test.record(d.configurable);'
     'Symbol.dispose=Symbol();test.record(Symbol.dispose===s);test.record(delete Symbol.dispose);'
     'try{(function(){"use strict";Symbol.dispose=Symbol();})();}catch(error){test.record(error.name);}'),
    ('symbol-property-keys-do-not-alias-strings-or-each-other',
     'let a=Symbol("same"),b=Symbol("same"),o={};o[a]=1;o[b]=2;o[String(a)]=3;'
     'test.record(o[a]);test.record(o[b]);test.record(o["Symbol(same)"]);'
     'test.record(a in o);test.record(Symbol("same") in o);test.record(Object.keys(o).join());'
     'test.record(delete o[a]);test.record(a in o);test.record(o[b]);'),
    ('symbol-property-key-object-conversion',
     'let s=Symbol("key"),log="",key={toString(){log+="k";return s;}},o={};'
     'o[key]=7;test.record(o[key]);test.record(key in o);'
     'test.record(Object.getOwnPropertyDescriptor(o,key).value);test.record(log);'),
    ('symbol-property-enumeration-and-insertion-order',
     'let a=Symbol("a"),b=Symbol("b"),o={visible:1};o[a]=2;'
     'Object.defineProperty(o,b,{value:3,configurable:true});'
     'let keys=Object.getOwnPropertySymbols(o);test.record(keys.length);'
     'test.record(keys[0]===a);test.record(keys[1]===b);'
     'test.record(Object.keys(o).join());'
     'let names=[];for(let key in o)names.push(key);test.record(names.join());'
     'test.record(o.hasOwnProperty(a));test.record(o.propertyIsEnumerable(a));'
     'test.record(o.propertyIsEnumerable(b));delete o[a];o[a]=4;'
     'keys=Object.getOwnPropertySymbols(o);test.record(keys[0]===b);test.record(keys[1]===a);'),
    ('symbol-prototype-inheritance-and-shadowing',
     'let s=Symbol("method"),prototype={};prototype[s]=function(){return this.value;};'
     'let o=Object.create(prototype);o.value=7;test.record(o[s]());test.record(s in o);'
     'test.record(o.hasOwnProperty(s));o[s]=function(){return 9;};test.record(o[s]());'
     'test.record(o.hasOwnProperty(s));delete o[s];test.record(o[s]());'),
    ('symbol-property-descriptors-and-readonly-writes',
     'let s=Symbol("fixed"),o={};Object.defineProperty(o,s,{value:7,enumerable:true});'
     'let d=Object.getOwnPropertyDescriptor(o,s);test.record(d.value);test.record(d.writable);'
     'test.record(d.enumerable);test.record(d.configurable);o[s]=9;test.record(o[s]);'
     'test.record(delete o[s]);try{Object.defineProperty(o,s,{value:9});}'
     'catch(error){test.record(error.name);}try{(function(){"use strict";o[s]=9;})();}'
     'catch(error){test.record(error.name);}'),
    ('symbol-defineproperties-uses-symbol-descriptor-keys',
     'let a=Symbol("a"),b=Symbol("b"),descriptors={visible:{value:1,enumerable:true}};'
     'descriptors[a]={value:2,enumerable:true,writable:true,configurable:true};'
     'Object.defineProperty(descriptors,b,{value:{value:3},enumerable:false});'
     'let o=Object.defineProperties({},descriptors);test.record(o.visible);test.record(o[a]);'
     'test.record(o[b]);test.record(Object.getOwnPropertySymbols(o)[0]===a);'),
    ('symbol-object-assign-copies-only-enumerable-symbols',
     'let a=Symbol("a"),b=Symbol("b"),input={visible:1};input[a]=2;'
     'Object.defineProperty(input,b,{value:3});let output=Object.assign({},input);'
     'test.record(output.visible);test.record(output[a]);test.record(b in output);'
     'test.record(Object.getOwnPropertySymbols(output).length);'
     'test.record(Object.getOwnPropertyDescriptor(output,a).writable);'),
    ('symbol-freeze-seal-and-preventextensions',
     'let s=Symbol("state"),o={};o[s]=1;Object.freeze(o);o[s]=2;'
     'test.record(o[s]);test.record(delete o[s]);test.record(Object.isFrozen(o));'
     'test.record(Object.getOwnPropertyDescriptor(o,s).writable);'
     'let sealed={};sealed[s]=3;Object.seal(sealed);sealed[s]=4;test.record(sealed[s]);'
     'test.record(delete sealed[s]);test.record(Object.isSealed(sealed));'
     'let closed=Object.preventExtensions({});closed[s]=5;test.record(s in closed);'
     'try{Object.defineProperty(closed,s,{value:5});}catch(error){test.record(error.name);}'),
    ('symbol-function-expando-keys-and-receivers',
     'let s=Symbol("method"),hidden=Symbol("hidden");function f(){}'
     'f[s]=function(){return this===f;};Object.defineProperty(f,hidden,{value:7});'
     'test.record(f[s]());test.record(f[hidden]);test.record(Object.getOwnPropertySymbols(f).length);'
     'test.record(Object.keys(f).join());test.record(delete f[s]);test.record(s in f);'),
    ('symbol-array-properties-preserve-indexing-and-length',
     'let s=Symbol("index"),a=[7,8];a[s]=9;test.record(a.length);test.record(a[0]);'
     'test.record(a[s]);test.record(Object.keys(a).join());'
     'test.record(Object.getOwnPropertySymbols(a)[0]===s);a.push(10);test.record(a.length);'),
    ('symbol-explicit-dispose-prototype-call',
     'function Disposable(){this.closed=false;}Disposable.prototype[Symbol.dispose]=function(){this.closed=true;};'
     'let item=new Disposable();test.record(item.closed);test.record(Symbol.dispose in item);'
     'item[Symbol.dispose]();test.record(item.closed);'
     'test.record(item.hasOwnProperty(Symbol.dispose));'),
    ('symbol-map-and-set-identity',
     'let a=Symbol("same"),b=Symbol("same"),map=new Map(),set=new Set();'
     'map.set(a,1);map.set(b,2);map.set(Symbol.for("shared"),3);'
     'test.record(map.get(a));test.record(map.get(b));test.record(map.get(Symbol.for("shared")));'
     'test.record(map.size);set.add(a);set.add(a);set.add(b);test.record(set.size);'
     'test.record(set.has(Symbol("same")));test.record(map.delete(a));test.record(map.has(b));'),
    ('symbol-json-omission-and-array-null',
     'let s=Symbol("hidden"),o={value:s,visible:7};o[s]=9;'
     'test.record(JSON.stringify(s));test.record(JSON.stringify(o));'
     'test.record(JSON.stringify([s,7]));test.record(JSON.stringify({nested:[s]}));'),
    ('symbol-throw-and-return-preserve-identity',
     'let s=Symbol("thrown");function identity(value){return value;}test.record(identity(s)===s);'
     'try{throw s;}catch(error){test.record(error===s);test.record(typeof error);test.record(error);}'),
    ('symbol-ordinary-iterator-polyfill-explicit-next',
     'if(!Symbol.iterator)Symbol.iterator=Symbol("iterator");'
     'if(!Array.prototype[Symbol.iterator])Array.prototype[Symbol.iterator]=function(){'
     'let list=this,index=0;return {next(){return index<list.length?'
     '{value:list[index++],done:false}:{value:undefined,done:true};}};};'
     'let iterator=[7,8][Symbol.iterator]();test.record(iterator.next().value);'
     'test.record(iterator.next().value);test.record(iterator.next().done);'),
    ('symbol-implicit-binary-text-regexp-and-join-api-errors',
     'let s=Symbol("input");for(let action of [()=>new Uint8Array([s]),'
     '()=>new Uint8Array(1).fill(s),()=>new TextEncoder().encode(s),()=>RegExp(s),'
     '()=>/x/.test(s),()=>[s].join(),()=>["x","y"].join(s)])'
     '{try{action();test.record("accepted");}catch(error){test.record(error.name);}}'),
])

# Numeric operators must reject the left Symbol before converting the right
# object. Addition and relational operators first convert both operands to
# primitives, so their observable valueOf call still occurs before TypeError.
# These fifteen sources mirror the independent original coercion-order audit.
COMPLETION_CASES.extend([
    ('symbol-operator-coercion-order-' + name,
     'var calls=0,kind="";try{Symbol() ' + operator
     + ' {valueOf(){calls++;return 1;}};}catch(error){kind=error.name;}calls+"|"+kind;')
    for name, operator in [
        ('subtract', '-'),
        ('multiply', '*'),
        ('divide', '/'),
        ('remainder', '%'),
        ('shift-left', '<<'),
        ('shift-right', '>>'),
        ('unsigned-shift-right', '>>>'),
        ('bitwise-and', '&'),
        ('bitwise-xor', '^'),
        ('bitwise-or', '|'),
        ('add', '+'),
        ('less-than', '<'),
        ('greater-than', '>'),
        ('less-or-equal', '<='),
        ('greater-or-equal', '>='),
    ]
])

BROWSER_CASES.extend([
    ('symbol-browser-global-constructor-alias',
     'test.record(window.Symbol===Symbol);test.record(self.Symbol===Symbol);'
     'test.record(typeof globalThis.Symbol.dispose);'),
    ('symbol-browser-registry-and-identity-across-jobs',
     'var shared=Symbol.for("query"),fresh=Symbol("query");window[shared]=7;'
     '\n//---NEXT SCRIPT---\n'
     'test.record(Symbol.for("query")===shared);test.record(Symbol.for("query")===fresh);'
     'test.record(window[shared]);test.record(Symbol.keyFor(shared));'),
    ('symbol-browser-global-property-descriptor-and-delete',
     'let s=Symbol("field");window[s]=7;window[String(s)]=8;'
     'test.record(window[s]);test.record(window[String(s)]);'
     'test.record(Object.getOwnPropertyDescriptor(window,s).value);'
     'test.record(Object.getOwnPropertySymbols(window)[0]===s);'
     'test.record(delete window[s]);test.record(s in window);'),
])

COMPLETION_CASES.append(('symbol-completion-value', 'Symbol("completion");'))

NEGATIVE.extend([
    ('symbol-identity-limit-cannot-catch-atomic',
     'try{for(var i=0;i<4097;i++)Symbol();}catch(error){test.record("caught");}'
     'finally{test.record("finally");}'),
    ('symbol-registry-limit-cannot-catch-atomic',
     'try{for(var i=0;i<4097;i++)Symbol.for("key"+i);}catch(error){test.record("caught");}'
     'finally{test.record("finally");}'),
])


# Original descriptor-accessor fixtures. Object-literal getter/setter syntax
# and restricted array-index/length descriptors keep their separate negatives.
CASES.extend([
    ('accessor-descriptor-creation-is-lazy',
     'try{Object.defineProperty({},"x",{get:function(){test.record(1);}});}'
     'catch(e){test.record(2);}finally{test.record(3);}'),
    ('accessor-own-getter-setter-arguments-and-assignment-value',
     'let stored=3,o={};Object.defineProperty(o,"value",{'
     'get:function(){test.record(this===o);test.record(arguments.length);return stored;},'
     'set:function(value){test.record(this===o);test.record(arguments.length);stored=value;return 99;}});'
     'test.record(o.value);test.record(o.value=7);test.record(stored);'),
    ('accessor-descriptor-fields-without-invocation',
     'let calls=0,o={},getter=function(){calls++;return 7;},setter=function(value){calls++;};'
     'Object.defineProperty(o,"x",{get:getter,set:setter});let d=Object.getOwnPropertyDescriptor(o,"x");'
     'test.record(d.get===getter);test.record(d.set===setter);test.record(d.enumerable);'
     'test.record(d.configurable);test.record(d.hasOwnProperty("value"));'
     'test.record(d.hasOwnProperty("writable"));test.record(calls);'),
    ('accessor-missing-getter-or-setter',
     'let stored=0,o={};Object.defineProperty(o,"writeOnly",{set:function(value){stored=value;}});'
     'Object.defineProperty(o,"readOnly",{get:function(){return 7;}});'
     'test.record(o.writeOnly);test.record(o.writeOnly=3);test.record(stored);'
     'o.readOnly=8;test.record(o.readOnly);try{(function(){"use strict";o.readOnly=8;})();}'
     'catch(error){test.record(error.name);}'),
    ('accessor-prototype-receiver-and-own-shadow',
     'let prototype={};Object.defineProperty(prototype,"value",{configurable:true,'
     'get:function(){return this.stored;},set:function(value){this.stored=value;}});'
     'let a=Object.create(prototype),b=Object.create(prototype);a.value=3;b.value=7;'
     'test.record(a.value);test.record(b.value);test.record(a.hasOwnProperty("value"));'
     'Object.defineProperty(a,"value",{value:9,configurable:true});test.record(a.value);'
     'delete a.value;test.record(a.value);'),
    ('accessor-symbol-keys-and-inherited-receiver',
     'let key=Symbol("value"),prototype={};Object.defineProperty(prototype,key,{'
     'get:function(){return this.stored;},set:function(value){this.stored=value;},enumerable:true});'
     'let o=Object.create(prototype);o[key]=7;test.record(o[key]);'
     'test.record(Object.getOwnPropertySymbols(prototype)[0]===key);'
     'test.record(Object.getOwnPropertyDescriptor(prototype,key).get.call({stored:9}));'
     'test.record(Object.keys(prototype).join());'),
    ('accessor-function-property-receiver',
     'function f(){}let stored=0;Object.defineProperty(f,"value",{'
     'get:function(){test.record(this===f);return stored;},'
     'set:function(value){test.record(this===f);stored=value;}});'
     'f.value=7;test.record(f.value);'),
    ('accessor-object-create-with-descriptors',
     'let prototype={base:3},key=Symbol("extra"),descriptors={value:{'
     'get:function(){return this.base+4;},enumerable:true}};'
     'descriptors[key]={get:function(){return this.base+6;}};'
     'let o=Object.create(prototype,descriptors);test.record(o.value);test.record(o[key]);'
     'test.record(Object.keys(o).join());'),
    ('accessor-partial-update-and-explicit-undefined',
     'let stored=3,o={},setter=function(value){stored=value;};'
     'Object.defineProperty(o,"x",{get:function(){return stored;},set:setter,configurable:true});'
     'Object.defineProperty(o,"x",{get:function(){return stored*2;}});o.x=4;test.record(o.x);'
     'test.record(Object.getOwnPropertyDescriptor(o,"x").set===setter);'
     'Object.defineProperty(o,"x",{get:undefined});test.record(o.x);o.x=5;test.record(stored);'),
    ('accessor-data-accessor-data-transitions',
     'let o={x:3};Object.defineProperty(o,"x",{get:function(){return 7;}});'
     'let d=Object.getOwnPropertyDescriptor(o,"x");test.record(o.x);test.record(d.enumerable);'
     'test.record(d.configurable);test.record(d.hasOwnProperty("writable"));'
     'Object.defineProperty(o,"x",{value:9});d=Object.getOwnPropertyDescriptor(o,"x");'
     'test.record(o.x);test.record(d.writable);test.record(d.enumerable);'
     'test.record(d.configurable);test.record(d.hasOwnProperty("get"));'),
    ('accessor-nonconfigurable-identity-and-transition-errors',
     'let o={},getter=function(){return 7;};Object.defineProperty(o,"x",{get:getter});'
     'Object.defineProperty(o,"x",{get:getter,set:undefined});'
     'for(let descriptor of [{get:function(){return 7;}},{set:function(){}},{value:7},'
     '{enumerable:true},{configurable:true}]){try{Object.defineProperty(o,"x",descriptor);}'
     'catch(error){test.record(error.name);}}test.record(o.x);test.record(delete o.x);'),
    ('accessor-invalid-callables-and-mixed-descriptors',
     'let o={};for(let descriptor of [{get:null},{get:1},{set:false},{set:{}},'
     '{get:undefined,value:7},{set:undefined,writable:false}])'
     '{try{Object.defineProperty(o,"x",descriptor);}catch(error){test.record(error.name);}}'
     'test.record(o.hasOwnProperty("x"));'),
    ('accessor-inherited-descriptor-fields',
     'let attributes=Object.create({get:function(){return this.base;},enumerable:true});'
     'let o={base:7};Object.defineProperty(o,"x",attributes);test.record(o.x);'
     'test.record(Object.keys(o).join());'),
    ('accessor-descriptor-field-get-order',
     'let log="",attributes={},o={};function field(name,value){Object.defineProperty(attributes,name,'
     '{get:function(){log+=name+";";return value;}});}'
     'field("enumerable",true);field("configurable",true);'
     'field("get",function(){return 7;});field("set",function(){});'
     'Object.defineProperty(o,"x",attributes);test.record(log);test.record(o.x);'),
    ('accessor-getter-and-setter-throws-reach-catch-finally',
     'let o={},log="";Object.defineProperty(o,"x",{get:function(){log+="g";throw "read";},'
     'set:function(value){log+="s";throw value;}});'
     'try{o.x;}catch(error){test.record(error);}finally{log+="f";}'
     'try{o.x="write";}catch(error){test.record(error);}finally{log+="f";}test.record(log);'),
    ('accessor-compound-update-evaluation-order',
     'let stored=2,log="",o={};Object.defineProperty(o,"x",{'
     'get:function(){log+="g";return stored;},set:function(value){log+="s";stored=value;}});'
     'function key(){log+="k";return "x";}function right(){log+="r";return 3;}'
     'test.record(o[key()]+=right());test.record(log);log="";'
     'test.record(o[key()]++);test.record(stored);test.record(log);'),
    ('accessor-method-lookup-before-arguments',
     'let log="",o={},first=function(value){log+="c";test.record(this===o);return value;};'
     'Object.defineProperty(o,"method",{get:function(){log+="g";return first;},configurable:true});'
     'function argument(){log+="a";Object.defineProperty(o,"method",{value:function(){return 99;}});return 7;}'
     'test.record(o.method(argument()));test.record(log);test.record(o.method());'),
    ('accessor-noncallable-method-still-evaluates-arguments',
     'let log="",o={};Object.defineProperty(o,"method",{get:function(){log+="g";return 7;}});'
     'function argument(){log+="a";}try{o.method(argument());}catch(error){test.record(error.name);}'
     'test.record(log);'),
    ('accessor-assign-source-getter-and-target-setter',
     'let log="",input={},output={};Object.defineProperty(input,"x",{enumerable:true,'
     'get:function(){log+="g";test.record(this===input);return 7;}});'
     'Object.defineProperty(output,"x",{set:function(value){log+="s";test.record(this===output);'
     'this.stored=value;}});test.record(Object.assign(output,input)===output);'
     'test.record(output.stored);test.record(log);'),
    ('accessor-assign-own-key-snapshot-and-live-values',
     'let input={};Object.defineProperty(input,"first",{enumerable:true,get:function(){'
     'delete input.second;input.third=30;input.late=40;return 10;}});input.second=20;input.third=3;'
     'let output=Object.assign({},input);test.record(JSON.stringify(output));'
     'test.record(output.hasOwnProperty("second"));test.record(output.hasOwnProperty("late"));'),
    ('accessor-defineproperties-prevalidates-before-target-writes',
     'let target={},descriptors={first:{value:7}};'
     'Object.defineProperty(descriptors,"second",{enumerable:true,get:function(){'
     'test.record(target.hasOwnProperty("first"));throw "descriptor";}});'
     'try{Object.defineProperties(target,descriptors);}catch(error){test.record(error);}'
     'test.record(target.hasOwnProperty("first"));'),
    ('accessor-defineproperties-own-key-snapshot-and-live-descriptors',
     'let target={},descriptors={};Object.defineProperty(descriptors,"first",{enumerable:true,'
     'get:function(){test.record(Object.keys(target).join());delete descriptors.second;'
     'descriptors.third={value:30,enumerable:true};descriptors.late={value:40,enumerable:true};'
     'return {value:10,enumerable:true};}});descriptors.second={value:20,enumerable:true};'
     'descriptors.third={value:3,enumerable:true};Object.defineProperties(target,descriptors);'
     'test.record(JSON.stringify(target));test.record(target.hasOwnProperty("late"));'),
    ('accessor-enumeration-and-has-do-not-invoke-getters',
     'let calls=0,o={};Object.defineProperty(o,"x",{enumerable:true,get:function(){calls++;return 7;}});'
     'test.record(Object.keys(o).join());let names=[];for(let name in o)names.push(name);'
     'test.record(names.join());test.record("x" in o);test.record(o.hasOwnProperty("x"));'
     'test.record(o.propertyIsEnumerable("x"));test.record(calls);test.record(o.x);test.record(calls);'),
    ('accessor-json-getter-values-and-own-key-snapshot',
     'let o={};Object.defineProperty(o,"first",{enumerable:true,get:function(){'
     'delete o.second;o.third=30;o.late=40;return 10;}});o.second=20;o.third=3;'
     'test.record(JSON.stringify(o));'),
    ('accessor-json-getter-exception',
     'let o={};Object.defineProperty(o,"x",{enumerable:true,get:function(){throw "json read";}});'
     'try{JSON.stringify(o);}catch(error){test.record(error);}'),
    ('accessor-freeze-and-seal-retain-callable-setters',
     'let stored=1,o={};Object.defineProperty(o,"x",{configurable:true,enumerable:true,'
     'get:function(){return stored;},set:function(value){stored=value;}});'
     'Object.freeze(o);o.x=7;test.record(o.x);test.record(Object.isFrozen(o));'
     'let d=Object.getOwnPropertyDescriptor(o,"x");test.record(d.configurable);'
     'test.record(d.hasOwnProperty("writable"));test.record(delete o.x);'
     'let sealed={};Object.defineProperty(sealed,"x",{configurable:true,get:function(){return 9;}});'
     'Object.seal(sealed);test.record(Object.isSealed(sealed));test.record(Object.isFrozen(sealed));'
     'test.record(sealed.x);'),
])

CASES.extend([
    ('accessor-primitive-receiver-' + name,
     'let value=' + value + ',key=Symbol("receiver");'
     'let getter=function(){"use strict";return this;};'
     'Object.defineProperty(' + constructor + '.prototype,"fixtureAccessor",{get:getter,configurable:true});'
     'Object.defineProperty(' + constructor + '.prototype,key,{get:getter,configurable:true});'
     'test.record(value.fixtureAccessor===value);test.record(value[key]===value);'
     'test.record(typeof value.fixtureAccessor);')
    for name, constructor, value in [
        ('string', 'String', '"text"'),
        ('number', 'Number', '7'),
        ('boolean', 'Boolean', 'true'),
        ('symbol', 'Symbol', 'Symbol("primitive")'),
    ]
])

CASES.extend([
    ('accessor-mapped-arguments-conversion-disconnects-parameter',
     'function check(parameter){let args=arguments,stored=3;Object.defineProperty(args,"0",{'
     'get:function(){test.record(this===args);return stored;},'
     'set:function(value){test.record(this===args);stored=value;}});'
     'parameter=9;test.record(args[0]);args[0]=7;test.record(parameter);test.record(args[0]);}check(1);'),
    ('accessor-mapped-arguments-undefined-getter-disconnects-parameter',
     'function check(parameter){let args=arguments;Object.defineProperty(args,"0",{get:undefined});'
     'parameter=9;test.record(args[0]);args[0]=7;test.record(parameter);'
     'test.record(Object.getOwnPropertyDescriptor(args,"0").hasOwnProperty("get"));}check(1);'),
])

BROWSER_CASES.extend([
    # Node's contextified VM globals expose a different accessor receiver than
    # its non-contextified globals. Real Window receiver assertions therefore
    # live in the App fixture; this comparison checks names and values only.
    ('accessor-browser-global-name-alias-and-delete',
     'let stored=3;Object.defineProperty(window,"fixtureAccessor",{configurable:true,'
     'get:function(){return stored;},set:function(value){stored=value;}});'
     'fixtureAccessor=7;test.record(fixtureAccessor);test.record(window.fixtureAccessor);'
     'test.record(delete window.fixtureAccessor);test.record(typeof fixtureAccessor);'),
    ('accessor-browser-closure-across-script-jobs',
     'let stored=1;window.holder={};Object.defineProperty(holder,"value",{'
     'get:function(){return stored;},set:function(value){stored=value;}});'
     '\n//---NEXT SCRIPT---\nholder.value=7;test.record(holder.value);test.record(stored);'),
])

NEGATIVE.extend([
    ('accessor-getter-recursion-cannot-catch-atomic',
     'let o={};Object.defineProperty(o,"x",{get:function(){return o.x;}});'
     'try{o.x;}catch(error){test.record("caught");}finally{test.record("finally");}'),
    ('accessor-setter-recursion-cannot-catch-atomic',
     'let o={};Object.defineProperty(o,"x",{set:function(value){o.x=value;}});'
     'try{o.x=7;}catch(error){test.record("caught");}finally{test.record("finally");}'),
    ('accessor-getter-loop-cannot-catch-atomic',
     'let o={};Object.defineProperty(o,"x",{get:function(){while(true){}}});'
     'try{o.x;}catch(error){test.record("caught");}finally{test.record("finally");}'),
    ('accessor-native-getter-cycle-cannot-catch-atomic',
     'let a=[];Object.defineProperty(a,"join",{get:Array.prototype.toString});'
     'try{a.join;}catch(error){test.record("caught");}finally{test.record("finally");}'),
])


CASES.extend([
    ('fromcharcode-empty-and-ascii',
     'test.record(String.fromCharCode());test.record(String.fromCharCode(65));'
     'test.record(String.fromCharCode(67,104,101,122,67,97,116));'),
    ('fromcharcode-truncation-and-modulo',
     'test.record(String.fromCharCode(65.9,65537,-65535,-1,131137));'
     'test.record(String.fromCharCode(0.9,-0.9,1e20,-1e20,1e308,-1e308,9007199254740991));'),
    ('fromcharcode-primitive-conversions',
     'test.record(String.fromCharCode(undefined,null,false,true,"65","",NaN,Infinity,-Infinity,-0));'),
    ('fromcharcode-bmp-and-paired-surrogates',
     'test.record(String.fromCharCode(0,127,128,2047,2048,8364,65535));'
     'let value=String.fromCharCode(0xD83D,0xDE00,0xDBFF,0xDFFF);'
     'test.record(value);test.record(value.length);'),
    ('fromcharcode-argument-and-primitive-conversion-order',
     'let log="";function argument(name,code){log+=name;return {'
     'valueOf(){log+=name.toUpperCase();return {};},toString(){log+=code;return String(code);}};}'
     'test.record(String.fromCharCode(argument("a",65),argument("b",66)));test.record(log);'),
    ('fromcharcode-abrupt-conversion-before-later-coercion-and-decoding',
     'let log="";function right(){log+="expression";return {valueOf(){log+="conversion";return 66;}};}'
     'try{String.fromCharCode(Symbol(),right());}catch(error){test.record(error.name);}test.record(log);'
     'try{String.fromCharCode(0xD800,Symbol());}catch(error){test.record(error.name);}'),
    ('fromcharcode-borrowed-call-apply-and-argument-boundary',
     'let convert=String.fromCharCode;test.record(convert.call(null,65,66));'
     'test.record(convert.apply({ignored:true},[67,68]));'
     'test.record(convert.apply(null,Array(256).fill(65)).length);'),
    ('fromcharcode-native-method-descriptor-and-nonconstructor',
     'let convert=String.fromCharCode,descriptor=Object.getOwnPropertyDescriptor(String,"fromCharCode");'
     'test.record(convert.name);test.record(convert.length);test.record(descriptor.value===convert);'
     'test.record(descriptor.writable);test.record(descriptor.enumerable);test.record(descriptor.configurable);'
     'try{new convert(65);}catch(error){test.record(error.name);}'),
])

NEGATIVE.extend([
    ('fromcharcode-unpaired-high-surrogate-unsupported-atomic',
     'try{String.fromCharCode(65,0xD800);}catch(error){test.record("caught");}'
     'finally{test.record("finally");}'),
    ('fromcharcode-unpaired-low-surrogate-unsupported-atomic',
     'try{String.fromCharCode(0xDC00);}catch(error){test.record("caught");}'
     'finally{test.record("finally");}'),
])


CASES.extend([
    ('hasown-nullish-target-before-key-conversion',
     'let log="";function argument(){log+="argument;";return {'
     'toString(){log+="key;";return "field";}};}'
     'try{Object.hasOwn(null,argument());}catch(error){test.record(error.name);}'
     'try{Object.hasOwn(undefined,argument());}catch(error){test.record(error.name);}'
     'try{Object.hasOwn();}catch(error){test.record(error.name);}test.record(log);'
     'try{Object.prototype.hasOwnProperty.call(null,argument());}'
     'catch(error){test.record(error.name);}test.record(log);'),
    ('hasown-ordinary-null-prototype-shadow-and-symbol-keys',
     'let key=Symbol("own"),parent={inherited:1},object=Object.create(parent);'
     'object.hasOwnProperty=null;object.field=undefined;object[key]=7;'
     'test.record(Object.hasOwn(object,"field"));test.record(Object.hasOwn(object,"inherited"));'
     'test.record(Object.hasOwn(object,"hasOwnProperty"));test.record(Object.hasOwn(object,key));'
     'test.record(Object.hasOwn(object,Symbol("own")));'
     'let bare=Object.create(null);bare.field=1;test.record(Object.hasOwn(bare,"field"));'
     'test.record(Object.hasOwn(bare,"toString"));'),
    ('hasown-primitives-and-array-holes',
     'test.record(Object.hasOwn("cat",0));test.record(Object.hasOwn("cat",3));'
     'test.record(Object.hasOwn("cat","length"));test.record(Object.hasOwn("cat","trim"));'
     'test.record(Object.hasOwn(7,"toString"));test.record(Object.hasOwn(true,"valueOf"));'
     'test.record(Object.hasOwn(Symbol("s"),"description"));'
     'let array=[,undefined];test.record(Object.hasOwn(array,0));'
     'test.record(Object.hasOwn(array,1));test.record(Object.hasOwn(array,"length"));'),
    ('hasown-getter-not-read-and-key-conversion',
     'let log="",object={},symbol=Symbol("field"),marker={};object[symbol]=3;'
     'Object.defineProperty(object,"field",{get:function(){throw marker;}});'
     'let key={toString(){log+="string;";return "field";},valueOf(){log+="number;";return 1;}};'
     'test.record(Object.hasOwn(object,key));test.record(log);'
     'test.record(Object.hasOwn(object,{toString(){return symbol;}}));'
     'try{Object.hasOwn(object,{toString(){throw marker;}});}'
     'catch(error){test.record(error===marker);}'),
    ('hasown-native-metadata-and-borrowed-call',
     'let own=Object.hasOwn,descriptor=Object.getOwnPropertyDescriptor(Object,"hasOwn");'
     'test.record(own.name);test.record(own.length);test.record(descriptor.value===own);'
     'test.record(descriptor.writable);test.record(descriptor.enumerable);'
     'test.record(descriptor.configurable);test.record(own.call(null,{field:1},"field"));'
     'test.record(own.apply(7,[{field:1},"absent"]));'
     'try{new own({},"field");}catch(error){test.record(error.name);}'),
    ('base64-latin1-roundtrip-and-original-vectors',
     'test.record(btoa(""));test.record(atob(""));test.record(btoa("ChezCat"));'
     'test.record(atob("Q2hlekNhdA=="));'
     'let bytes=String.fromCharCode(0,1,127,128,254,255),encoded=btoa(bytes);'
     'test.record(encoded);test.record(atob(encoded)===bytes);test.record(atob(encoded));'),
    ('base64-forgiving-whitespace-unpadded-and-pad-bits',
     'for(let text of ["Zg","Zg==","Zh==","Zm8","Zm8=","Zm9","Zm9v",'
     '" \\tZ\\ng\\r=\\f= "," \\t\\n\\f\\r"]){test.record(atob(text));}'),
    ('base64-invalid-alphabet-length-and-padding',
     'for(let text of ["A","Zg=","Zm9v=","AA=A","=AAA","AAAA====",'
     '"AA-_=","AA\\u00a0==","AA\\v=="]){'
     'try{atob(text);test.record("unexpected");}catch(error){test.record(error.name);}}'),
    ('base64-invalid-latin1-and-symbol-conversions',
     'for(let text of ["\\u0100","a😀"]){'
     'try{btoa(text);test.record("unexpected");}catch(error){test.record(error.name);}}'
     'try{btoa(Symbol("input"));}catch(error){test.record(error.name);}'
     'try{atob(Symbol("input"));}catch(error){test.record(error.name);}'),
    ('base64-string-coercion-callback-order-and-abrupt',
     'let log="",marker={};function input(text){return {'
     'toString(){log+="string;";return {};},valueOf(){log+="value;";return text;}};}'
     'test.record(btoa(input("cat")));test.record(atob(input("Y2F0")));test.record(log);'
     'try{btoa({toString(){throw marker;}});}catch(error){test.record(error===marker);}'
     'try{atob({toString(){throw marker;}});}catch(error){test.record(error===marker);}'),
    ('base64-function-metadata-borrowed-call-and-primitives',
     'test.record(atob.name);test.record(atob.length);test.record(btoa.name);test.record(btoa.length);'
     'test.record(btoa.call(null,"cat"));test.record(atob.apply({ignored:true},["Y2F0"]));'
     'test.record(btoa(undefined));test.record(btoa(null));test.record(btoa(42));test.record(btoa(true));'
     'try{atob();}catch(error){test.record(error.name);}'
     'try{btoa();}catch(error){test.record(error.name);}'),
])


def run_own(probe, source, invoke=False, handler=False, browser=False):
    started = time.monotonic()
    flags = (["--invoke"] if invoke else []) + (["--handler"] if handler else []) + (["--browser-global"] if browser else [])
    try:
        completed = subprocess.run(
            [str(probe), *flags], input=source, capture_output=True,
            text=True, encoding="utf-8", timeout=8,
        )
    except subprocess.TimeoutExpired:
        return {"ok": False, "timeout": True, "records": [],
                "seconds": time.monotonic() - started}
    if completed.returncode:
        return {"ok": False, "crash": completed.returncode, "records": [],
                "stderr": completed.stderr, "seconds": time.monotonic() - started}
    lines = completed.stdout.splitlines()
    valid = bool(lines and (lines[0].startswith("OK\t") or lines[0].startswith("ERROR\t")))
    return {
        "ok": bool(lines and lines[0].startswith("OK\t")),
        "value": lines[0][3:] if lines and lines[0].startswith("OK\t") else None,
        "error": None if lines and lines[0].startswith("OK\t") else (lines[0] if lines else "no output"),
        "protocol_error": not valid,
        "records": [line[7:] for line in lines if line.startswith("RECORD\t")],
        "seconds": time.monotonic() - started,
    }


def run_node(node, source, invoke=False, handler=False, browser=False):
    flags = (["--invoke"] if invoke else []) + (["--handler"] if handler else []) + (["--browser-global"] if browser else [])
    command = [node, "-e", NODE_DRIVER] + (["--", *flags] if flags else [])
    completed = subprocess.run(
        command, input=source, capture_output=True, text=True,
        encoding="utf-8", timeout=8, check=True,
    )
    return json.loads(completed.stdout)


def finished_normally(result):
    return not any(result.get(key) for key in ("crash", "timeout", "protocol_error"))


# 0.34 original callable/object foundation comparisons.
CASES.extend([
    ('bind-captured-receiver-chain', "function f(a,b,c){'use strict';return this.x+a+b+c;}let x={x:1},g=f.bind(x,2),h=g.bind({x:100},3);x.x=10;test.record(h(4));test.record(h.call({x:999},5));test.record(h.apply(null,[6]));"),
    ('bind-native-pop', "let a=[1,2,3],p=Array.prototype.pop.bind(a);test.record(p());test.record(p.call([]));test.record(a.join(','));"),
    ('bind-closure-and-arrow', 'function make(x){return function(a){return x+a;}.bind(null,3);}test.record(make(7)());function outer(){let f=()=>this;return f.bind({wrong:1})();}let o={};test.record(outer.call(o)===o);'),
    ('bind-native-construction', "let B=Array.bind(null,2,3),a=new B();test.record(a.join(','));test.record(a instanceof Array);test.record(a instanceof B);test.record(B.hasOwnProperty('prototype'));"),
    ('bind-construction-target-prototype', 'function F(a,b){this.sum=a+b;}let B=F.bind({sum:100},2),C=B.bind(null,3);B.prototype={fake:1};let a=new C();test.record(a.sum);test.record(a instanceof F);test.record(a instanceof B);test.record(a instanceof C);F.prototype={changed:1};test.record((new B(4)).changed);test.record(a instanceof B);'),
    ('bind-nonconstructible-and-callable-rejection', "let caught=[];try{new ((()=>1).bind(null))();}catch(e){caught.push(e.name);}try{Function.prototype.bind.call({});}catch(e){caught.push(e.name);}try{new (Array.prototype.pop.bind([]))();}catch(e){caught.push(e.name);}test.record(caught.join(','));"),
    ('bind-metadata-descriptors', "function bindingTarget(a,b,c){}let a=bindingTarget.bind(null,1),b=a.bind(null,2);test.record(a.name);test.record(a.length);test.record(b.name);test.record(b.length);let d=Object.getOwnPropertyDescriptor(a,'length');test.record(d.writable+','+d.enumerable+','+d.configurable);test.record(a.toString());"),
    ('bind-metadata-read-order', "function f(){}let log='',old=Object.getPrototypeOf(f),next={};Object.defineProperty(f,'length',{get:function(){log+='l';Object.setPrototypeOf(f,next);return 4.9;},configurable:true});Object.defineProperty(f,'name',{get:function(){log+='n';return 'sample';},configurable:true});let b=Function.prototype.bind.call(f,null,1,2);test.record(log);test.record(b.name);test.record(b.length);test.record(Object.getPrototypeOf(b)===old);"),
    ('bind-own-length-and-nonnumeric-metadata', "function f(){}let p={};Object.setPrototypeOf(f,p);delete f.length;Object.defineProperty(p,'length',{get:function(){throw 'inherited';}});test.record(Function.prototype.bind.call(f,null).length);Object.defineProperty(f,'length',{value:Infinity,configurable:true});test.record(Function.prototype.bind.call(f,null,1).length);Object.defineProperty(f,'length',{value:'12',configurable:true});Object.defineProperty(f,'name',{value:5,configurable:true});let b=Function.prototype.bind.call(f,null);test.record(b.length);test.record(b.name);"),
    ('bind-indirect-eval', "var value=1;function f(){var value=2;return eval.bind(null)('value');}test.record(f());"),
    ('bind-metadata-abrupt-completion', "function f(){}let marker={},seen='';Object.defineProperty(f,'length',{get:function(){seen+='l';throw marker;},configurable:true});Object.defineProperty(f,'name',{get:function(){seen+='n';return 'x';},configurable:true});try{f.bind(null);}catch(e){test.record(e===marker);}test.record(seen);"),
    ('bind-null-prototype-target', "function f(a){'use strict';return this+a;}Object.setPrototypeOf(f,null);let b=Function.prototype.bind.call(f,4,5);test.record(b());test.record(Object.getPrototypeOf(b)===null);"),
    ('regex-function-replacer-supported', 'test.record("a".replace(/a/,()=>"b"));'),
    ('replace-callback-literal-utf16-receiver-and-arity', "\nlet calls=0;\ntest.record('😀cat-cat'.replace('cat',function(match,offset,input){'use strict';calls++;test.record(this===undefined);test.record(arguments.length);test.record(match);test.record(offset);test.record(input);return '$&';}));\ntest.record(calls);\ntest.record('abc'.replace('absent',function(){throw 'unexpected callback';}));\ntest.record('abc'.replace('',function(match,offset,input){return offset+input;}));\n"),
    ('replace-callback-regexp-unmatched-captures', "\nlet calls=0;\ntest.record('ab'.replace(/(a)|(b)/g,function(match,a,b,offset,input){'use strict';test.record(this===undefined);test.record(arguments.length);test.record(match);test.record(a===undefined);test.record(b===undefined);test.record(offset);test.record(input);calls++;return '['+match+']';}));\ntest.record(calls);\n"),
    ('replace-callback-global-snapshot-lastindex-and-freeze', "\nlet expression=/a/g,seen=0;expression.lastIndex=99;\ntest.record('aa'.replace(expression,function(match,offset,input){test.record(expression.lastIndex);test.record(offset);if(seen===0){expression.lastIndex=37;Object.freeze(expression);}seen++;return '$&';}));\ntest.record(seen);test.record(expression.lastIndex);\nlet missing=/z/g;missing.lastIndex=4;test.record('aa'.replace(missing,function(){throw 'unexpected callback';}));test.record(missing.lastIndex);\n"),
    ('replace-callback-empty-progress-and-sticky-index', "\nlet empty=/(?:)/g,positions='';test.record('ab'.replace(empty,function(match,offset){positions+=offset;return '-';}));test.record(positions);test.record(empty.lastIndex);\nlet sticky=/a/y;sticky.lastIndex=1;test.record('ba'.replace(sticky,function(match,offset){test.record(offset);test.record(sticky.lastIndex);sticky.lastIndex=8;return 'x';}));test.record(sticky.lastIndex);\nlet plain=/a/;plain.lastIndex=5;test.record('ba'.replace(plain,function(){return 'x';}));test.record(plain.lastIndex);\n"),
    ('replace-callback-result-conversion-and-abrupt-order', "\nlet log='',expression=/a/g;\ntest.record('aa'.replace(expression,function(match,index){log+='c'+index;return {toString:function(){log+='s'+index;return index;},valueOf:function(){throw 'wrong hint';}};}));test.record(log);\nlet marker=Symbol('failure'),calls=0;\ntry{'aa'.replace(expression,function(){calls++;throw marker;});}catch(error){test.record(error===marker);}test.record(calls);test.record(expression.lastIndex);\ncalls=0;try{'aa'.replace(expression,function(){calls++;return Symbol();});}catch(error){test.record(error.name);}test.record(calls);test.record(expression.lastIndex);\ntry{'a'.replace('a',function(){return {toString:function(){throw marker;}};});}catch(error){test.record(error===marker);}\ntest.record('a'.replace('a',function(){return undefined;}));test.record('a'.replace('a',function(){return null;}));\n"),
    ('replace-template-preservation-and-literal-conversion-order', "\ntest.record('abc'.replace(/(b)/,'$$:$&:$`:$\\':$1:$2'));\ntest.record('ab'.replace(/(a)|(b)/g,'<$1$2>'));\nlet log='',search={toString:function(){log+='s';return 'a';}},replacement={toString:function(){log+='r';return '[$&]';}};\ntest.record('a'.replace(search,replacement));test.record(log);log='';\ntry{'a'.replace(Symbol(),replacement);}catch(error){test.record(error.name);}test.record(log);\ntest.record('undefined'.replace(undefined,function(){return 'matched';}));\n"),
])

# These cases intentionally compare the returned completion value.
COMPLETION_CASES.extend([
    ('object-inspection-order-and-symbols', "\nvar p={inherited:42},o=Object.create(p),key=Symbol('private');\no.tail='old';o[10]='ten';o[2]='two';o['01']='leading';\nObject.defineProperty(o,'hidden',{value:9});o[key]='symbol';delete o.tail;o.tail='new';\nvar d=Object.getOwnPropertyDescriptors(o);\n[Object.getOwnPropertyNames(o).join(','),JSON.stringify(Object.values(o)),JSON.stringify(Object.entries(o)),Object.getOwnPropertySymbols(d)[0]===key,d[key].value,d.hidden.enumerable,d.hidden.writable,d.hidden.configurable].join('|');\n"),
    ('object-inspection-primitives-and-null', "\nvar methods=['getOwnPropertyNames','getOwnPropertyDescriptors','values','entries'],caught=[];\nfor(var i=0;i<methods.length;i++){try{Object[methods[i]](null);}catch(e){caught.push(e.name);}try{Object[methods[i]]();}catch(e){caught.push(e.name);}}\nvar d=Object.getOwnPropertyDescriptors('Z\\u00f1');\n[caught.join(','),Object.getOwnPropertyNames('Z\\u00f1').join(','),JSON.stringify(Object.values('Z\\u00f1')),JSON.stringify(Object.entries('Z\\u00f1')),d[1].value,d[1].writable,d[1].enumerable,d[1].configurable,d.length.value,d.length.enumerable,Object.entries(5).length,Object.values(true).length,Object.getOwnPropertyNames(Symbol()).length].join('|');\n"),
    ('object-inspection-getter-shape-changes', "\nvar log='';function makeSubject(){var p={},o=Object.create(p);Object.defineProperty(p,'removed',{get:function(){throw 'inherited';}});\nObject.defineProperty(o,'first',{enumerable:true,get:function(){log+='f';delete o.removed;Object.defineProperty(o,'quiet',{enumerable:false});Object.defineProperty(o,'later',{enumerable:true});o.newKey=7;return 2;}});\nObject.defineProperty(o,'removed',{value:3,configurable:true,enumerable:true});\nObject.defineProperty(o,'quiet',{enumerable:true,configurable:true,get:function(){throw 'quiet';}});\nObject.defineProperty(o,'later',{enumerable:false,configurable:true,get:function(){log+='l';return this===o?8:99;}});return o;}\nvar values=Object.values(makeSubject()),entries=Object.entries(makeSubject());\n[JSON.stringify(values),JSON.stringify(entries),log].join('|');\n"),
    ('object-inspection-descriptor-getters-and-throw', "\nvar calls=0,marker={},o={},s=Symbol('get'),get=function(){calls++;throw marker;},set=function(v){calls+=v;};\nObject.defineProperty(o,'data',{value:4,enumerable:true,writable:true,configurable:true});\nObject.defineProperty(o,'access',{get:get,set:set,enumerable:true,configurable:true});Object.defineProperty(o,s,{get:get});\nvar names=Object.getOwnPropertyNames(o),d=Object.getOwnPropertyDescriptors(o),before=calls,caught=0;\ntry{Object.values(o);}catch(e){if(e===marker)caught++;}try{Object.entries(o);}catch(e){if(e===marker)caught++;}\n[names.join(','),before,d.access.get===get,d.access.set===set,Object.hasOwn(d.access,'value'),d[s].get===get,d[s].enumerable,Object.getPrototypeOf(d)===Object.prototype,caught,calls].join('|');\n"),
    ('object-inspection-result-data-properties', "\nvar calls=0,o={value:11};\nObject.defineProperty(Object.prototype,'0',{configurable:true,set:function(){calls++;}});\nObject.defineProperty(Object.prototype,'value',{configurable:true,set:function(){calls++;}});\nvar values=Object.values(o),entries=Object.entries(o),d=Object.getOwnPropertyDescriptors(o);\nvar correct=values[0]===11&&entries[0][0]==='value'&&entries[0][1]===11&&d.value.value===11;\ndelete Object.prototype['0'];delete Object.prototype.value;\n[correct,calls,Object.getOwnPropertyDescriptor(values,'0').writable,Object.getOwnPropertyDescriptor(d,'value').enumerable].join('|');\n"),
    ('object-inspection-arrays-functions-typed', "\nvar a=[1,,3];a.extra=6;var d=Object.getOwnPropertyDescriptors(a);function inspect(a,b,c){}inspect.extra=9;var f=Object.getOwnPropertyDescriptors(inspect);\n[Object.getOwnPropertyNames(a).join(','),JSON.stringify(Object.entries(a)),d.length.value,d.length.writable,d.length.enumerable,d.length.configurable,JSON.stringify(Object.values(new Uint8Array([4,5]))),f.name.value,f.length.value,f.prototype.value===inspect.prototype,Object.getOwnPropertyNames(inspect).indexOf('prototype')>=0,JSON.stringify(Object.entries(inspect))].join('|');\n"),
])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustc", default=os.environ.get("RUSTC", "rustc"), help="Rust compiler executable")
    parser.add_argument("--node", default="node", help="Node.js executable (development comparison only)")
    parser.add_argument("--output", type=Path, help="Write the complete JSON report here")
    parser.add_argument("--case-prefix", action="append", default=[], metavar="PREFIX",
                        help="Select case names starting with PREFIX; repeat to select several groups")
    options = parser.parse_args()
    if any(not prefix for prefix in options.case_prefix):
        parser.error("Each --case-prefix value must be non-empty.")
    def selected(collection):
        return [case for case in collection if not options.case_prefix
                or any(case[0].startswith(prefix) for prefix in options.case_prefix)]
    cases = selected(CASES)
    browser_cases = selected(BROWSER_CASES)
    completion_cases = selected(COMPLETION_CASES)
    negative_cases = selected(NEGATIVE)
    selected_counts = {"positive": len(cases), "browser": len(browser_cases),
                       "completion": len(completion_cases), "negative": len(negative_cases)}
    if not sum(selected_counts.values()):
        parser.error("No cases match the supplied --case-prefix filters.")
    source_path = ROOT / "src" / "javascript.rs"
    try:
        rust_version = subprocess.check_output([options.rustc, "--version"], text=True, timeout=10).strip()
        node_version = subprocess.check_output([options.node, "--version"], text=True, timeout=10).strip()
        results = []
        negatives = []
        with tempfile.TemporaryDirectory(prefix="own-browser-js-check-") as directory:
            temporary = Path(directory)
            probe_source = temporary / "probe.rs"
            probe = temporary / ("probe.exe" if os.name == "nt" else "probe")
            probe_source.write_text(
                PROBE.replace("JAVASCRIPT_SOURCE", json.dumps(str(source_path), ensure_ascii=False)),
                encoding="utf-8",
            )
            subprocess.run(
                [options.rustc, "--edition=2021", "-O", str(probe_source), "-o", str(probe)],
                check=True, capture_output=True, text=True, encoding="utf-8", timeout=120,
            )
            for case in cases:
                name, source, *mode = case
                invoke = bool(mode and mode[0])
                handler = bool(len(mode) > 1 and mode[1])
                expected = run_node(options.node, source, invoke, handler)
                actual = run_own(probe, source, invoke, handler)
                passed = (finished_normally(actual) and actual["ok"] == expected["ok"]
                          and actual["records"] == expected["records"])
                results.append({"case": name, "passed": passed, "actual": actual, "expected": expected})
            for name, source in browser_cases:
                expected = run_node(options.node, source, browser=True)
                actual = run_own(probe, source, browser=True)
                passed = (finished_normally(actual) and actual["ok"] == expected["ok"]
                          and actual["records"] == expected["records"])
                results.append({"case": name, "passed": passed, "actual": actual, "expected": expected})
            for name, source in completion_cases:
                expected = run_node(options.node, source)
                actual = run_own(probe, source)
                passed = (finished_normally(actual) and actual["ok"] == expected["ok"]
                          and actual["records"] == expected["records"]
                          and actual["value"] == expected["value"])
                results.append({"case": name, "passed": passed, "actual": actual, "expected": expected})
            for case in negative_cases:
                name, source, *mode = case
                actual = run_own(probe, source, handler=bool(mode and mode[0]))
                passed = finished_normally(actual) and not actual["ok"]
                if name.endswith("-atomic"):
                    passed = passed and not actual["records"]
                negatives.append({"case": name, "passed": passed, "actual": actual})
    except (OSError, subprocess.SubprocessError, ValueError) as error:
        detail = getattr(error, "stderr", None)
        print(f"JavaScript checker could not finish: {error}", file=sys.stderr)
        if detail:
            print(detail, file=sys.stderr)
        print("Install Python 3, Rust, and Node.js for these development checks; the browser itself does not require Node.js.", file=sys.stderr)
        return 2

    report = {
        "case_filter": {"prefixes": options.case_prefix, "selected": selected_counts},
        "rustc": rust_version,
        "node": node_version,
        "source_sha256": hashlib.sha256(source_path.read_bytes()).hexdigest(),
        "json_source_sha256": hashlib.sha256((ROOT / "src/json.rs").read_bytes()).hexdigest(),
        "regexp_source_sha256": hashlib.sha256((ROOT / "src/regexp.rs").read_bytes()).hexdigest(),
        "prototypes_source_sha256": hashlib.sha256((ROOT / "src/prototypes.rs").read_bytes()).hexdigest(),
        "binary_source_sha256": hashlib.sha256((ROOT / "src/binary_data.rs").read_bytes()).hexdigest(),
        "arguments_source_sha256": hashlib.sha256((ROOT / "src/arguments.rs").read_bytes()).hexdigest(),
        "array_ops_source_sha256": hashlib.sha256((ROOT / "src/array_ops.rs").read_bytes()).hexdigest(),
        "collections_source_sha256": hashlib.sha256((ROOT / "src/collections.rs").read_bytes()).hexdigest(),
        "text_encoding_source_sha256": hashlib.sha256((ROOT / "src/text_encoding.rs").read_bytes()).hexdigest(),
        "symbols_source_sha256": hashlib.sha256((ROOT / "src/symbols.rs").read_bytes()).hexdigest(),
        "base64_source_sha256": hashlib.sha256((ROOT / "src/javascript_base64.rs").read_bytes()).hexdigest(),
        "bound_functions_source_sha256": hashlib.sha256((ROOT / "src/bound_functions.rs").read_bytes()).hexdigest(),
        "object_inspection_source_sha256": hashlib.sha256((ROOT / "src/object_inspection.rs").read_bytes()).hexdigest(),
        "string_replacement_source_sha256": hashlib.sha256((ROOT / "src/string_replacement.rs").read_bytes()).hexdigest(),
        "oracle_cases": len(results),
        "oracle_passed": sum(item["passed"] for item in results),
        "negative_cases": len(negatives),
        "negative_passed": sum(item["passed"] for item in negatives),
        "results": results,
        "negatives": negatives,
    }
    if options.output:
        options.output.parent.mkdir(parents=True, exist_ok=True)
        options.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    summary = {key: value for key, value in report.items() if key not in ("results", "negatives")}
    print(json.dumps(summary, indent=2))
    failed = [item for item in results + negatives if not item["passed"]]
    for item in failed:
        print(json.dumps(item, indent=2))
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
