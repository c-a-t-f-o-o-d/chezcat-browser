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
// Both host implementations return undefined. Expression-bodied callbacks used
// to expose Array.push's length / the saved function only in the Node driver.
const test = {number:11, record:value=>{records.push(repr(value));}, callback:fn=>{test.savedCallback=fn;}};
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
    ('unsupported-async-method-atomic', 'test.record(1);let x={async field(){return 2;}};'),
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
    ('limit-cannot-be-caught-atomic', 'try{while(true){}}catch(e){test.record("caught");}finally{test.record("finally");}'),
    ('limit-cannot-be-returned-atomic', 'function f(){try{while(true){}}finally{return 7;}}test.record(f());'),
    ('recursion-cannot-be-caught-atomic', 'function f(){try{return f();}catch(e){return 7;}finally{test.record("finally");}}test.record(f());'),
    ('unsupported-feature-cannot-be-caught-atomic', 'try{let o={};o.__proto__;}catch(e){test.record("caught");}finally{test.record("finally");}'),
    ('arrow-retained-this-bound-atomic', 'function make(){"use strict";return ()=>this;}let a=[];let text="'+'a'*32000+'";for(let i=0;i<80;i++){a.push(make.call(text));}test.record("done");'),
    ('uri-expanded-string-limit', 'encodeURIComponent("'+'é'*24000+'");'),
    ('long-sequence-chain', 'test.record(' + ','.join('1' for _ in range(12000)) + ');'),
    ('arrow-recursive-limit', 'let f=()=>f();f();'),
])

NEGATIVE.extend([
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
    ('eval-missing-host-capability-atomic', 'test.denyEvalCapability=true;try{eval("test.record(1)");}catch(e){test.record(2);}finally{test.record(3);}'),
    ('eval-infinite-cannot-catch-atomic', 'try{eval("while(true){}");}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-recursive-depth-cannot-catch-atomic', 'var code="eval(code)";try{eval(code);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-retained-program-limit-atomic', 'try{for(var i=0;i<200;i++)eval("1");}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-retained-source-limit-atomic', 'var code='+json.dumps('/*'+' '*100000+'*/1;')+';try{for(var i=0;i<30;i++)eval(code);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-retained-ast-limit-atomic', 'var code='+json.dumps('if(false){'+'1;'*4000+'}')+';try{for(var i=0;i<40;i++)eval(code);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-parser-limit-cannot-catch-atomic', 'try{eval('+json.dumps('!'*4000+'1;')+');}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-arrow-parameter-limit-cannot-catch-atomic', 'try{eval('+json.dumps('('+','.join('a'+str(i) for i in range(257))+')=>1')+');}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-unsupported-syntax-cannot-catch-atomic', 'try{eval("class Unsupported {}");}catch(e){test.record(1);}finally{test.record(2);}'),
    ('eval-unsupported-async-method-cannot-catch-atomic', 'try{eval("({async x(){return 1;}})");}catch(e){test.record(1);}finally{test.record(2);}'),
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
    ('json-host-object-unsupported-atomic', 'try{JSON.stringify(test);}catch(e){test.record(1);}'),
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
    ('prototype-host-chain-disabled-atomic', 'try{Object.create(test);}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-host-target-disabled-atomic', 'try{Object.setPrototypeOf(test,{});}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-host-descriptor-disabled-atomic', 'try{Object.defineProperty(test,"number",{value:3});}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-chain-work-bound-atomic', 'let o={};try{for(let n=0;n<5000;n++){o=Object.create(o);o.missing;}}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-descriptor-retained-bound-atomic', 'let o={},s="'+'x'*32000+'";try{for(let n=0;n<100;n++)Object.defineProperty(o,"p"+n,{value:s});}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-constructor-recursion-bound-atomic', 'function F(){return new F();}try{new F();}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-borrowed-apply-recursion-bound-atomic', 'let a=[];a[0]=Function.prototype.apply;a[1]=a;try{Function.prototype.apply.apply(Function.prototype.apply,a);}catch(e){test.record(1);}finally{test.record(2);}'),
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


# Original reduced fixtures for a nonrecursive binding name at the parser edge.
# The eval fixture creates an uncalled function; no captured site code is used.
_BINDING_LEAF_FIXTURES = [
    ROOT / 'tools/fixtures/parser-binding-identifier.js',
    ROOT / 'tools/fixtures/parser-binding-identifier-eval.js',
]
CASES.extend([
    ('binding-leaf-group-boundary', _BINDING_LEAF_FIXTURES[0].read_text(encoding='utf-8')),
    ('binding-leaf-indirect-eval-boundary', _BINDING_LEAF_FIXTURES[1].read_text(encoding='utf-8')),
    ('binding-leaf-rest-names-boundary',
     'var f=' + '(' * 61
     + 'function(value,extra,...rest){return [value,extra,rest.length].join("|");}'
     + ')' * 61 + ';f(7,8,9);'),
    ('binding-leaf-array-name-boundary',
     'var f=' + '(' * 60 + 'function([value]){return value;}' + ')' * 60 + ';f([7]);'),
])
_BINDING_LEAF_LIMIT_CASES = [
    ('binding-leaf-extra-group-limit-atomic', 62, 'value'),
    ('binding-leaf-array-pattern-limit-atomic', 61, '[value]'),
    ('binding-leaf-object-pattern-limit-atomic', 61, '{value}'),
    ('binding-leaf-default-initializer-limit-atomic', 61, 'value=7'),
]
NEGATIVE.extend(
    (name, 'test.record("early");var f=' + '(' * groups
     + 'function(' + parameters + '){return 7;}' + ')' * groups + ';')
    for name, groups, parameters in _BINDING_LEAF_LIMIT_CASES
)
NEGATIVE.append((
    'binding-leaf-assignment-reference-limit-atomic',
    'test.record("early");var target={};' + '(' * 61
    + '[target.value]=[7]' + ')' * 61 + ';',
))


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


# Original descriptor-accessor fixtures. Literal accessors and array index/length
# descriptors also have independent positive coverage in the 0.35 group below.
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


# 0.35 original literal-accessor and array-descriptor comparisons.
COMPLETION_CASES.extend([
    ('literal-accessor-descriptors', "\n            var calls=0, o={get x(){calls++;return this.n;},set x(v){this.n=v;},n:2};\n            var d=Object.getOwnPropertyDescriptor(o,'x');\n            if(calls!==0 || !d.enumerable || !d.configurable || 'value' in d || 'writable' in d)throw 'accessor descriptor';\n            if(d.get.length!==0 || d.set.length!==1 || d.get.name!=='get x' || d.set.name!=='set x')throw 'metadata';\n            if(Object.hasOwn(d.get,'prototype') || Object.hasOwn(d.set,'prototype'))throw 'constructor prototype';\n            var constructed=0;try{new d.get();}catch(e){if(e.name==='TypeError')constructed++;}\n            try{new d.set();}catch(e){if(e.name==='TypeError')constructed++;}\n            if(constructed!==2)throw 'constructible accessor';\n            var nd=Object.getOwnPropertyDescriptor(d.get,'name');\n            var ld=Object.getOwnPropertyDescriptor(d.set,'length');\n            if(nd.enumerable||nd.writable||!nd.configurable||ld.enumerable||ld.writable||!ld.configurable)throw 'metadata descriptors';\n            o.x=7;if(o.x!==7 || calls!==1)throw 'get/set pair';\n            o={get x(){throw 'obsolete getter';},set x(v){this.n=v;},get x(){return this.n+1;}};\n            o.x=8;if(o.x!==9)throw 'getter replaces one half';\n            o={set x(v){throw 'obsolete setter';},get x(){return this.n;},set x(v){this.n=v+2;}};\n            o.x=8;if(o.x!==10)throw 'setter replaces one half';\n            o={get x(){throw 'getter ran';},x:5};d=Object.getOwnPropertyDescriptor(o,'x');\n            if(o.x!==5 || !d.writable || !d.enumerable || !d.configurable || 'get' in d)throw 'accessor to data';\n            o={x:3,get x(){return 9;}};d=Object.getOwnPropertyDescriptor(o,'x');\n            if(o.x!==9 || d.set!==undefined || 'value' in d)throw 'data to accessor';\n            o={get x(){return 1;},x:3,set x(v){this.n=v;}};d=Object.getOwnPropertyDescriptor(o,'x');\n            if(d.get!==undefined || o.x!==undefined)throw 'data clears old accessor';\n            o.x=4;if(o.n!==4)throw 'new setter';\n        \n'ok';"),
    ('literal-accessor-keys-receivers', "\n            var get=2,set=3,async=4,o={get,set,async,getter(){return this.get;},get(){return 5;},set(v){return v+1;},async(){return 7;}};\n            if(o.get()!==5 || o.set(8)!==9 || o.async()!==7 || o.get.name!=='get' || o.set.name!=='set')throw 'ordinary named methods';\n            var sym=Symbol('field'),empty=Symbol(''),anonymous=Symbol();\n            o={get 'a b'(){return 1;},set 4(v){this.n=v;},get true(){return 2;},get [sym](){return 3;},set [sym](v){this.n=v;},get [empty](){return 4;},get [anonymous](){return 5;}};\n            if(o['a b']!==1 || o.true!==2 || o[sym]!==3)throw 'property names';\n            o[4]=7;if(o.n!==7)throw 'numeric setter';\n            var d=Object.getOwnPropertyDescriptor(o,sym);\n            if(d.get.name!=='get [field]' || d.set.name!=='set [field]')throw 'symbol function names';\n            if(Object.getOwnPropertyDescriptor(o,empty).get.name!=='get []' || Object.getOwnPropertyDescriptor(o,anonymous).get.name!=='get ')throw 'empty symbol function names';\n            o[sym]=9;if(o.n!==9)throw 'symbol setter';\n            var child=Object.create(o);child[4]=11;if(child.n!==11 || o.n!==9)throw 'inherited setter receiver';\n            function make(n){return {get value(){return n;},set value(v){n=v;}};}\n            var a=make(2),b=make(3);a.value=8;if(a.value!==8 || b.value!==3)throw 'closures';\n            var getter=Object.getOwnPropertyDescriptor({get value(){return this.n;}},'value').get;\n            if(getter.call({n:12})!==12)throw 'borrowed getter';\n            if(({get pattern(){return /a+/.test('aaa');}}).pattern!==true)throw 'regexp lexical goal';\n        \n'ok';"),
    ('literal-accessor-definition-order', "\n            var order='',key={toString(){order+='k';return 'value';}},sym=Symbol('key');\n            var o={get [(order+='g',key)](){order+='G';return this.n;},set [(order+='s',key)](v){order+='S';this.n=v;},n:(order+='n',1)};\n            if(order!=='gkskn')throw 'key evaluation order';\n            o.value=8;if(o.value!==8 || order!=='gksknSG')throw 'accessor call order';\n            var thrown={},seen=false;\n            try{o={get [{toString(){throw thrown;}}](){return 1;},later:(order+='bad',2)};}catch(e){seen=e===thrown;}\n            if(!seen || order!=='gksknSG')throw 'key abrupt completion';\n            Object.defineProperty(Object.prototype,'literalTrap',{set:function(){throw 'inherited setter';},configurable:true});\n            Object.defineProperty(Object.prototype,sym,{set:function(){throw 'inherited symbol setter';},configurable:true});\n            o={literalTrap:1,[sym]:2};delete Object.prototype.literalTrap;delete Object.prototype[sym];\n            if(o.literalTrap!==1 || o[sym]!==2 || !Object.hasOwn(o,'literalTrap'))throw 'own data properties';\n            o={get [sym](){throw 'obsolete symbol getter';},[sym]:4};\n            if(o[sym]!==4 || !Object.getOwnPropertyDescriptor(o,sym).writable)throw 'symbol data override';\n            o={get b(){return 1;},a:2,set b(v){},get 3(){return 3;},get 1(){return 1;}};\n            if(Object.keys(o).join(',')!=='1,3,b,a')throw 'property ordering';\n            var count=0;Object.getOwnPropertyDescriptors({get x(){count++;return 1;}});\n            if(count!==0)throw 'descriptor inspection invoked getter';\n        \n'ok';"),
    ('literal-accessor-strict-exceptions', "\n            var thrown={},seen=0,o={get x(){throw thrown;},set x(v){throw v;}};\n            try{o.x;}catch(e){if(e===thrown)seen++;}\n            try{o.x=thrown;}catch(e){if(e===thrown)seen++;}\n            if(seen!==2)throw 'exception identity';\n            o={get x(){return 1;}};o.x=3;if(o.x!==1)throw 'sloppy missing setter';\n            seen=false;try{(function(){'use strict';o.x=3;})();}catch(e){seen=e.name==='TypeError';}\n            if(!seen)throw 'strict missing setter';\n            var globalReceiver=(function(){return this;})();\n            var strict=Object.getOwnPropertyDescriptor({get x(){'use strict';return this;}},'x').get;\n            var sloppy=Object.getOwnPropertyDescriptor({get x(){return this;}},'x').get;\n            if(strict.call(null)!==null || strict.call(7)!==7 || sloppy.call(null)!==globalReceiver)throw 'this semantics';\n            function outer(){'use strict';return Object.getOwnPropertyDescriptor({get x(){return this;}},'x').get;}\n            if(outer().call(undefined)!==undefined)throw 'inherited strict mode';\n            var x='outer',getter=Object.getOwnPropertyDescriptor({get x(){return x;}},'x').get;\n            if(getter()!=='outer')throw 'name must not create lexical binding';\n        \n'ok';"),
    ('literal-accessor-eval-arity', "\n            var untouched=0,seen=false;\n            try{eval('untouched=1;({get x(v){}});');}catch(e){seen=e.name==='SyntaxError';}\n            if(!seen || untouched!==0)throw 'catchable arity syntax error';\n        \n'ok';"),
])
CASES.extend([
    ('literal-accessor-executes-on-read', 'test.record(1);var obj={get field(){test.record(2);return 3;}};test.record(obj.field);'),
    ('literal-accessor-eval-enabled', 'var obj=eval("({get x(){return 7;}})");test.record(obj.x);'),
    ('array-descriptor-index-default-attributes', 'var a=[];Object.defineProperty(a,"0",{value:4});a[0]=9;var d=Object.getOwnPropertyDescriptor(a,"0");test.record(a[0]);test.record(a.length);test.record(d.writable);test.record(d.enumerable);test.record(d.configurable);'),
])

COMPLETION_CASES.append(('array-descriptors-define-data-accessors-and-ordered-names', "\nvar a=[], hits=0, key=Symbol('side');\nObject.defineProperty(a,'3',{value:'third',writable:true,enumerable:true,configurable:true});\nif(a.length!==4||0 in a||a[3]!=='third')throw 'index growth';\nObject.defineProperty(a,'1',{get:function(){hits++;return this[3];},configurable:true});\nvar d=Object.getOwnPropertyDescriptors(a);\nif(hits||d[1].get.call(a)!=='third'||hits!==1||d[1].enumerable||'value' in d[1])throw 'accessor descriptor';\nObject.defineProperty(a,'1',{set:function(v){this[3]=v;}});a[1]='changed';\nif(a[1]!=='changed'||hits!==2)throw 'merged accessor';\nObject.defineProperty(a,'1',{value:'first',writable:true,enumerable:true});\nObject.defineProperty(a,'01',{value:'ordinary'});a[key]='symbol';a.note=7;\nif(a[1]!=='first'||a.length!==4||Object.keys(a).join('|')!=='1|3|note')throw 'attributes';\nif(Object.getOwnPropertyNames(a).join('|')!=='1|3|length|01|note')throw 'name ordering';\nObject.defineProperty(a,'length',{value:6});d=Object.getOwnPropertyDescriptor(a,'length');\nif(d.value!==6||!d.writable||d.enumerable||d.configurable||Object.keys(a).indexOf('length')!==-1)throw 'length descriptor';\nObject.defineProperty(a,'length',{value:1});\nif(a.length!==1||1 in a||3 in a||a['01']!=='ordinary'||a[key]!=='symbol'||a.note!==7)throw 'shrink';\n'ok';\n"))

COMPLETION_CASES.append(('array-descriptors-shrink-descending-and-lock-after-blocked-deletion', "\nvar a=[0,1,2,3,4,5,6,7,8,9,10],caught=0;\nObject.defineProperty(a,'2',{configurable:false});\nObject.defineProperty(a,'7',{configurable:false});\ntry{Object.defineProperty(a,'length',{value:1,writable:false});}catch(e){if(e.name==='TypeError')caught++;}\nif(caught!==1||a.length!==8||a[7]!==7||8 in a||10 in a||!(6 in a)||!(2 in a))throw 'descending partial shrink';\nif(Object.getOwnPropertyDescriptor(a,'length').writable)throw 'lock after failure';\na[0]='ok';a[8]='absent';a.length=0;\nif(a[0]!=='ok'||8 in a||a.length!==8)throw 'locked writes';\nvar b=[0,1,2,3];Object.defineProperty(b,'2',{configurable:false});b.length=1;\nif(b.length!==3||3 in b||!(1 in b))throw 'sloppy shrink';\ntry{(function(){'use strict';b.length=0;})();}catch(e){if(e.name==='TypeError')caught++;}\nif(caught!==2||b.length!==3||Object.getOwnPropertyDescriptor(b,'length').writable!==true)throw 'strict shrink';\n'ok';\n"))

COMPLETION_CASES.append(('array-descriptors-readonly-length-allows-existing-indices-and-same-definitions', "\nvar a=[1,,3],caught=0, calls=0;\nObject.defineProperty(a,'length',{writable:false});\nObject.defineProperty(a,'0',{value:9});Object.defineProperty(a,'1',{value:2,configurable:true});\na[0]=8;a[3]=4;a.length={valueOf:function(){calls++;return 0;}};\nif(a.length!==3||a[0]!==8||a[1]!==2||3 in a||calls)throw 'readonly writes';\nObject.defineProperty(a,'length',{value:'3',writable:false});Object.defineProperty(a,'length',{});\ntry{Object.defineProperty(a,'3',{value:4});}catch(e){if(e.name==='TypeError')caught++;}\ntry{Object.defineProperty(a,'length',{writable:true});}catch(e){if(e.name==='TypeError')caught++;}\ntry{(function(){'use strict';a.length=3;})();}catch(e){if(e.name==='TypeError')caught++;}\nif(caught!==3)throw 'readonly definitions';\nvar b=Object.preventExtensions([4,5]);Object.defineProperty(b,'length',{value:4});\ntry{Object.defineProperty(b,'3',{value:6});}catch(e){if(e.name==='TypeError')caught++;}\nObject.defineProperty(b,'0',{value:7});Object.defineProperty(b,'length',{value:1});\nif(caught!==4||b.length!==1||b[0]!==7||1 in b)throw 'nonextensible';\nvar z=[];Object.freeze(z);Object.defineProperty(z,'length',{value:-0});\nif(1/z.length!==Infinity||!Object.isFrozen(z))throw 'zero same value';\nvar child=Object.create(a);child.length=10;\nif(child.hasOwnProperty('length')||child.length!==3)throw 'inherited readonly length';\n'ok';\n"))

COMPLETION_CASES.append(('array-descriptors-length-conversion-runs-twice-and-rechecks-live-attributes', "\nvar a=[0,1,2], calls=0;\nObject.defineProperty(a,'length',{value:{valueOf:function(){calls++;if(calls===1){a.length=6;return 4294967297;}return 1;}}});\nif(calls!==2||a.length!==1||1 in a)throw 'two conversions';\ncalls=0;a.length={valueOf:function(){calls++;return calls===1?2.5:2;}};\nif(calls!==2||a.length!==2||1 in a)throw 'assignment conversion';\ncalls=0;Object.defineProperty(a,'length',{value:{valueOf:function(){calls++;if(calls===2)Object.freeze(a);return 2;}}});\nif(calls!==2||!Object.isFrozen(a)||a.length!==2)throw 'live descriptor';\nvar b=[0], n=0;\nb.length={valueOf:function(){n++;if(n===2)Object.freeze(b);return 1;}};\nif(n!==2||!Object.isFrozen(b)||b.length!==1)throw 'assignment live same value';\nvar c=[0,1], count=0, caught=0;\ntry{Object.defineProperty(c,'length',{value:{valueOf:function(){count++;return count;}}});}catch(e){if(e.name==='RangeError')caught++;}\nif(count!==2||caught!==1||c.length!==2)throw 'mismatched conversions';\n'ok';\n"))

COMPLETION_CASES.append(('array-descriptors-reject-invalid-lengths-and-preserve-exception-order', "\nvar a=[1,2], invalid=[NaN,Infinity,-Infinity,-1,1.5,4294967296,undefined,'bad'], caught=0;\nfor(var i=0;i<invalid.length;i++){try{Object.defineProperty(a,'length',{value:invalid[i]});}catch(e){if(e.name==='RangeError')caught++;}}\nif(caught!==invalid.length||a.length!==2||a[1]!==2)throw 'invalid lengths';\nvar calls=0, marker=Symbol('throw'), value={valueOf:function(){calls++;if(calls===2)throw marker;return 1;}};\ntry{Object.defineProperty(a,'length',{value:value,enumerable:true});}catch(e){if(e===marker)caught++;}\nif(calls!==2||caught!==9||a.length!==2)throw 'coercion before attributes';\ncalls=0;\ntry{Object.defineProperty(a,'length',{value:value,get:function(){return 1;}});}catch(e){if(e.name==='TypeError')caught++;}\nif(calls||caught!==10)throw 'descriptor validation before value conversion';\ntry{Object.defineProperty(a,'length',{get:function(){return 1;}});}catch(e){if(e.name==='TypeError')caught++;}\ntry{Object.defineProperty(a,'length',{enumerable:true});}catch(e){if(e.name==='TypeError')caught++;}\ntry{Object.defineProperty(a,'length',{configurable:true});}catch(e){if(e.name==='TypeError')caught++;}\ntry{Object.defineProperty(a,'length',{value:Symbol('length')});}catch(e){if(e.name==='TypeError')caught++;}\nif(caught!==14||a.length!==2||a[1]!==2)throw 'attributes';\n'ok';\n"))

COMPLETION_CASES.append(('array-descriptors-mutators-throw-and-preserve-partial-changes', "\nvar a=[1,2], caught=0;Object.defineProperty(a,'length',{writable:false});\ntry{a.pop();}catch(e){if(e.name==='TypeError')caught++;}\nif(caught!==1||a.length!==2||1 in a||a[0]!==1)throw 'pop deletion before length';\ntry{a.push(3);}catch(e){if(e.name==='TypeError')caught++;}\ntry{a.push();}catch(e){if(e.name==='TypeError')caught++;}\nif(caught!==3||a.length!==2||2 in a)throw 'push failure';\nvar b=[1,2,3];Object.defineProperty(b,'1',{writable:false});\ntry{b.splice(0,1);}catch(e){if(e.name==='TypeError')caught++;}\nif(caught!==4||b.length!==3||b[0]!==2||b[1]!==2||b[2]!==3)throw 'splice partial';\nvar c=[1,2];Object.defineProperty(c,'1',{configurable:false});\ntry{c.pop();}catch(e){if(e.name==='TypeError')caught++;}\nif(caught!==5||c.length!==2||c[1]!==2)throw 'pop nonconfigurable';\nvar hits=0,d=Object.create({});Object.setPrototypeOf(d,Array.prototype);\nObject.defineProperty(d,'0',{set:function(v){hits+=v;},configurable:true});d.length=0;\nif(Array.prototype.push.call(d,5)!==1||hits!==5||d.length!==1)throw 'generic inherited accessor';\n'ok';\n"))

COMPLETION_CASES.append(('array-descriptors-define-properties-snapshots-before-partial-application', "\nvar a=[0], fields={}, order='';\nObject.defineProperty(fields,'2',{enumerable:true,get:function(){order+='i';return {value:2,enumerable:true,configurable:true};}});\nObject.defineProperty(fields,'length',{enumerable:true,get:function(){order+='l';if(a.length!==1)throw 'early definition';return {value:2,writable:false};}});\nObject.defineProperties(a,fields);\nif(order!=='il'||a.length!==2||2 in a||Object.getOwnPropertyDescriptor(a,'length').writable)throw 'snapshot and shrink';\nvar b=[];Object.defineProperty(b,'1',{value:NaN});\nObject.defineProperty(b,'1',{value:NaN});\nvar caught=0;try{Object.defineProperty(b,'1',{value:0});}catch(e){if(e.name==='TypeError')caught++;}\nObject.defineProperty(b,'0',{value:-0});\ntry{Object.defineProperty(b,'0',{value:0});}catch(e){if(e.name==='TypeError')caught++;}\nif(caught!==2||b.length!==2||1/b[0]!==-Infinity)throw 'nonconfigurable SameValue';\nObject.defineProperty(b,'4294967295',{value:4});Object.defineProperty(b,'-0',{value:5});\nif(b.length!==2||b['4294967295']!==4||b['-0']!==5)throw 'non-index names';\n'ok';\n"))

# 0.36 original primitive-wrapper, JSON and conversion comparisons.
COMPLETION_CASES.extend([
    ('wrappers-construct-scalar-slots-with-stable-intrinsic-prototypes', "\nvar n=new Number('12'),b=new Boolean(0),s=Symbol('value'),w=Object(s);\nif(typeof n!=='object'||n.valueOf()!==12||n.toString()!=='12'||!(n instanceof Number)||!(n instanceof Object))throw 'Number wrapper';\nif(typeof b!=='object'||b.valueOf()!==false||b.toString()!=='false'||!Boolean(b)||!(b instanceof Boolean))throw 'Boolean wrapper';\nif(typeof w!=='object'||w.valueOf()!==s||!(w instanceof Symbol)||Object.getPrototypeOf(w)!==Symbol.prototype)throw 'Symbol wrapper';\nif(Object(n)!==n||new Object(n)!==n||Object(3)===Object(3)||Object(false)===Object(false)||Object(s)===Object(s))throw 'identity';\nif(new Object(4).valueOf()!==4||new Object(false).valueOf()!==false||new Object(s).valueOf()!==s)throw 'Object construction';\nif(new Number().valueOf()!==0||1/new Number(-0).valueOf()!==-Infinity||new Boolean().valueOf()!==false)throw 'defaults';\nif(Object.keys(n).length||Object.getOwnPropertyNames(n).length||Object.getOwnPropertySymbols(w).length)throw 'slots leaked as properties';\nvar bound=Number.bind(null,'17'),boundValue=new bound();\nif(boundValue.valueOf()!==17||!(boundValue instanceof Number)||!(boundValue instanceof bound))throw 'bound constructor';\n'ok';\n"),
    ('wrapper-brands-are-internal-and-prototype-slots-are-real', "\nif(Number.prototype.valueOf()!==0||Boolean.prototype.valueOf()!==false)throw 'prototype slots';\nif(Object.getPrototypeOf(4)!==Number.prototype||Object.getPrototypeOf(false)!==Boolean.prototype)throw 'primitive prototypes';\nvar n=Object(3),b=Object(true),s=Object(Symbol('tag')),fake=Object.create(Number.prototype),caught=0;\nif(Object.prototype.toString.call(n)!=='[object Number]'||Object.prototype.toString.call(b)!=='[object Boolean]'||Object.prototype.toString.call(s)!=='[object Symbol]'||Object.prototype.toString.call(fake)!=='[object Object]')throw 'brand tags';\nfor(var value of [fake,{},b,Symbol(),null,undefined]){try{Number.prototype.valueOf.call(value);}catch(e){if(e.name==='TypeError')caught++;}}\ntry{Boolean.prototype.valueOf.call(n);}catch(e){if(e.name==='TypeError')caught++;}\ntry{Symbol.prototype.valueOf.call(Symbol.prototype);}catch(e){if(e.name==='TypeError')caught++;}\nif(caught!==8)throw 'forged brands';\nn.value=99;Object.setPrototypeOf(n,null);\nif(Number.prototype.valueOf.call(n)!==3||Object.prototype.toString.call(n)!=='[object Number]')throw 'slot survives prototype change';\nif(Number.prototype.toString.length!==1||Boolean.prototype.toString.length!==0)throw 'method metadata';\n'ok';\n"),
    ('wrappers-box-each-sloppy-call-and-preserve-strict-and-lexical-receivers', "\nfunction receiver(){return this;}function strictReceiver(){'use strict';return this;}\nvar a=receiver.call(5),b=receiver.apply(5,[]),s=Symbol('receiver');\nif(typeof a!=='object'||a===b||a.valueOf()!==5||b.valueOf()!==5)throw 'fresh number receivers';\nif(receiver.call(false).valueOf()!==false||receiver.call(s).valueOf()!==s)throw 'other scalar receivers';\nif(strictReceiver.call(5)!==5||strictReceiver.call(false)!==false||strictReceiver.call(s)!==s||strictReceiver.call(null)!==null)throw 'strict receivers';\nvar bound=receiver.bind(5),first=bound();if(bound()===first||first.valueOf()!==5)throw 'bound receiver freshness';\nfunction make(){return ()=>this;}var closure=make.call(false),held=closure();\nif(closure.call(8)!==held||held.valueOf()!==false)throw 'lexical boxed receiver';\nNumber.prototype.fixtureMethod=function(){this.extra=9;return this;};\nvar boxed=(3).fixtureMethod();delete Number.prototype.fixtureMethod;\nif(boxed.valueOf()!==3||boxed.extra!==9||(3).extra!==undefined)throw 'method boxed receiver';\nif(Object.prototype.valueOf.call(4).valueOf()!==4||Object.prototype.valueOf.call(false).valueOf()!==false||Object.prototype.valueOf.call(s).valueOf()!==s)throw 'generic valueOf boxing';\n'ok';\n"),
    ('wrapper-expandos-descriptors-assign-and-integrity-use-ordinary-storage', "\nvar key=Symbol('field'),n=Object(6);n.x=1;n[key]=2;\nObject.defineProperty(n,'hidden',{value:3});Object.defineProperty(n,'twice',{get:function(){return this.valueOf()*2;},enumerable:true});\nif(Object.keys(n).join(',')!=='x,twice'||Object.getOwnPropertyNames(n).join(',')!=='x,hidden,twice'||Object.getOwnPropertySymbols(n)[0]!==key)throw 'own storage';\nif(Object.getOwnPropertyDescriptor(n,'hidden').writable||n.twice!==12)throw 'descriptors';\nObject.freeze(n);n.x=8;if(n.x!==1||n.valueOf()!==6||!Object.isFrozen(n))throw 'integrity';\nvar target=Object.assign(4,null,5,true,Symbol(),{a:7,[key]:9});\nif(target.valueOf()!==4||target.a!==7||target[key]!==9||Object.getPrototypeOf(target)!==Number.prototype)throw 'assign boxed target';\nvar copied=Object.assign({},target);if(copied.a!==7||copied[key]!==9)throw 'assign boxed source';\nvar hits=0,name={toString:function(){hits++;return 'x';}};\nif(Object.getOwnPropertyDescriptor(1,name)!==undefined||Object.getOwnPropertyDescriptor(false,name)!==undefined||Object.getOwnPropertyDescriptor(Symbol(),name)!==undefined||hits!==3)throw 'temporary descriptor inspection';\nvar caught=0;try{Object.defineProperty(1,'x',{value:2});}catch(e){if(e.name==='TypeError')caught++;}\ntry{Object.assign(null,{x:2});}catch(e){if(e.name==='TypeError')caught++;}\nif(caught!==2)throw 'invalid destinations';\n'ok';\n"),
    ('primitive-assignments-observe-setters-and-strict-failures-without-mutation', "\nvar n=3,b=false,s=Symbol('write'),caught=0;n.x=1;b.x=1;s.x=1;\nif(n.x!==undefined||b.x!==undefined||s.x!==undefined)throw 'primitive retained expando';\nfor(var value of [n,b,s]){try{(function(){'use strict';this.x=1;}).call(value);}catch(e){if(e.name==='TypeError')caught++;}}\nif(caught!==3)throw 'strict primitive write';\nvar seen,received;\nObject.defineProperty(Number.prototype,'fixtureSetter',{set:function(value){'use strict';seen=this;received=value;},configurable:true});\nn.fixtureSetter=7;delete Number.prototype.fixtureSetter;\nif(seen!==3||received!==7)throw 'setter original primitive';\nObject.defineProperty(Boolean.prototype,'fixtureSetter',{set:function(value){seen=this;this.extra=value;},configurable:true});\nb.fixtureSetter=8;delete Boolean.prototype.fixtureSetter;\nif(typeof seen!=='object'||seen.valueOf()!==false||seen.extra!==8||b.extra!==undefined)throw 'setter sloppy wrapper';\nObject.defineProperty(Number.prototype,'fixtureData',{value:4,writable:true,configurable:true});\nn.fixtureData=9;if(Number.prototype.fixtureData!==4||n.fixtureData!==4)throw 'prototype data mutation';\ndelete Number.prototype.fixtureData;\n'ok';\n"),
    ('wrapper-conversions-keep-valueof-overrides-boolean-truthiness-and-order', "\nvar calls=0,marker={},n=new Number({valueOf:function(){calls++;return 8;}});\nif(calls!==1||n.valueOf()!==8)throw 'constructor conversion';\nvar b=new Boolean({valueOf:function(){throw 'Boolean conversion';}});\nif(b.valueOf()!==true||Number(Object(false))!==0||String(Object(true))!=='true'||Number(Object(7))!==7)throw 'wrapper conversions';\nn.valueOf=function(){return 9;};n.toString=function(){return 'text';};\nif(Number(n)!==9||String(n)!=='text'||n+1!==10)throw 'ordinary override ordering';\nvar caught=false;try{new Number({valueOf:function(){throw marker;}});}catch(e){caught=e===marker;}\nif(!caught)throw 'constructor exception';\ncalls=0;var radix={valueOf:function(){calls++;return 10.9;}};\nif(Number.prototype.toString.call(Object(12),radix)!=='12'||calls!==1)throw 'decimal radix conversion';\ncalls=0;if(Boolean.prototype.toString.call(Object(false),radix)!=='false'||calls!==0)throw 'Boolean ignores arguments';\ntry{Number.prototype.toString.call({},radix);}catch(e){if(e.name!=='TypeError')throw e;}\nif(calls!==0)throw 'brand before radix';\nvar failures=0;for(var value of [0,1,37,NaN,Infinity,null]){try{Number.prototype.toString.call(n,value);}catch(e){if(e.name==='RangeError')failures++;}}\nif(failures!==6||Number.prototype.toString.call(n,undefined)!=='8')throw 'radix validation';\nif(Number.prototype.toString.call(Object(NaN),16)!=='NaN'||Number.prototype.toString.call(Object(Infinity),2)!=='Infinity'||Number.prototype.toString.call(Object(-Infinity),36)!=='-Infinity'||Number.prototype.toString.call(Object(-0),2)!=='0')throw 'radix independent values';\n'ok';\n"),
    ('wrapper-json-unboxes-numeric-and-boolean-wrappers-but-keeps-symbol-objects', '\nvar n=new Number(7),b=new Boolean(false),s=Object(Symbol(\'field\'));\nn.self=n;b.self=b;s.field=3;\nif(JSON.stringify(n)!==\'7\'||JSON.stringify(b)!==\'false\'||JSON.stringify(s)!==\'{"field":3}\')throw \'wrapper values\';\nif(JSON.stringify([n,b,s,new Number(NaN),new Number(Infinity),new Number(-0)])!==\'[7,false,{"field":3},null,null,0]\')throw \'array wrappers\';\nif(JSON.stringify({number:n,boolean:b})!==\'{"number":7,"boolean":false}\')throw \'object wrappers\';\nif(JSON.stringify(Number.prototype)!==\'0\'||JSON.stringify(Boolean.prototype)!==\'false\')throw \'intrinsic slots\';\nvar read=0;Object.defineProperty(n,\'toJSON\',{get:function(){read++;return undefined;}});\nObject.defineProperty(b,\'toJSON\',{get:function(){read++;return undefined;}});\nif(JSON.stringify([n,b])!==\'[7,false]\'||read!==2)throw \'toJSON lookup\';\ns.self=s;var caught=false;try{JSON.stringify(s);}catch(e){caught=e.name===\'TypeError\';}\nif(!caught)throw \'symbol wrapper cycle\';\n\'ok\';\n'),
    ('wrapper-json-wrapper-conversion-observes-hooks-and-preserves-exception-order', '\nvar n=Object(3),b=Object(false),s=Object(Symbol(\'field\')),log=\'\';\nObject.defineProperty(n,\'toJSON\',{get:function(){log+=\'j\';return 0;},configurable:true});\nn[Symbol.toPrimitive]=function(hint){if(this!==n)throw \'receiver\';log+=hint;return \'9\';};\nb[Symbol.toPrimitive]=function(){throw \'Boolean hook\';};b.valueOf=function(){throw \'Boolean valueOf\';};\nObject.defineProperty(s,Symbol.toPrimitive,{value:function(){throw \'Symbol hook\';}});s.kept=2;\nif(JSON.stringify([n,b,s])!==\'[9,false,{"kept":2}]\'||log!==\'jnumber\')throw \'unboxing policy\';\nvar marker={},caught=0;\nn[Symbol.toPrimitive]=function(){throw marker;};\ntry{JSON.stringify(n);}catch(e){if(e===marker)caught++;}\nObject.defineProperty(n,\'toJSON\',{get:function(){throw marker;},configurable:true});\ntry{JSON.stringify(n);}catch(e){if(e===marker)caught++;}\nif(caught!==2)throw \'exception identity\';\n\'ok\';\n'),
    ('wrapper-json-boxed-numeric-indentation-converts-before-omitted-root', '\nvar gap=Object(2),log=\'\',value={a:1};\ngap[Symbol.toPrimitive]=function(hint){if(this!==gap)throw \'gap receiver\';log+=hint;return 2.8;};\nif(JSON.stringify(value,null,gap)!==\'{\\n  "a": 1\\n}\'||log!==\'number\')throw \'number gap\';\nvar ignored=Object(true);ignored[Symbol.toPrimitive]=function(){throw \'ignored gap\';};\nif(JSON.stringify(value,null,ignored)!==\'{"a":1}\')throw \'Boolean gap\';\nif(JSON.stringify(value,null,new Number(Infinity))!==\'{\\n          "a": 1\\n}\')throw \'gap clamp\';\nif(JSON.stringify(value,null,new Number(NaN))!==\'{"a":1}\')throw \'NaN gap\';\nvar marker={},seen=false;gap[Symbol.toPrimitive]=function(){throw marker;};\ntry{JSON.stringify(undefined,null,gap);}catch(e){seen=e===marker;}\nif(!seen)throw \'gap conversion before omitted root\';\n\'ok\';\n'),
    ('conversion-protocol-preserves-hints-receiver-and-operator-order', "\nvar hints=[],o={};o[Symbol.toPrimitive]=function(hint){\nif(this!==o||arguments.length!==1)throw 'receiver or arity';hints.push(hint);return hint==='string'?'field':6;};\nif(o+1!==7||1+o!==7||!(o==6)||!(6==o))throw 'default conversions';\nif(+o!==6||o-1!==5||!(o<7)||!(7>o))throw 'number conversions';\nif(String(o)!=='field'||({field:9})[o]!==9)throw 'string conversions';\nif(hints.join(',')!=='default,default,default,default,number,number,number,number,string,string')throw 'hint sequence';\nvar count=hints.length;if(o==null||o===6||!Boolean(o)||hints.length!==count)throw 'unneeded conversion';\nvar order='',a={},b={};a[Symbol.toPrimitive]=function(h){order+='a'+h;return 2;};\nb[Symbol.toPrimitive]=function(h){order+='b'+h;return 3;};\nif(a+b!==5||order!=='adefaultbdefault')throw 'left to right';\norder='';try{Symbol('left')-b;}catch(e){if(e.name!=='TypeError')throw e;}\nif(order!=='')throw 'numeric left rejection';\ntry{Symbol('left')+b;}catch(e){if(e.name!=='TypeError')throw e;}\nif(order!=='bdefault')throw 'addition converts both';\n'ok';\n"),
    ('conversion-protocol-reads-once-falls-back-and-propagates-errors', "\nvar reads=0,calls=0,parent={},child=Object.create(parent);\nObject.defineProperty(parent,Symbol.toPrimitive,{get:function(){\nif(this!==child)throw 'lookup receiver';reads++;return function(h){if(this!==child)throw 'call receiver';calls++;return 8;};},configurable:true});\nif(Number(child)!==8||reads!==1||calls!==1)throw 'lookup once';\nvar order='',fallback={valueOf(){order+='v';return 3;},toString(){order+='s';return 'field';}};\nfallback[Symbol.toPrimitive]=null;\nif(+fallback!==3||String(fallback)!=='field'||order!=='vs')throw 'null fallback';\nObject.defineProperty(fallback,Symbol.toPrimitive,{get:function(){order+='g';this.valueOf=function(){order+='n';return 4;};return undefined;}});\nif(+fallback!==4||order!=='vsgn')throw 'live fallback';\nvar marker={},bad={},caught=0;\nObject.defineProperty(bad,Symbol.toPrimitive,{get:function(){throw marker;},configurable:true});\ntry{String(bad);}catch(e){if(e===marker)caught++;}\nObject.defineProperty(bad,Symbol.toPrimitive,{value:7});\ntry{+bad;}catch(e){if(e.name==='TypeError')caught++;}\nObject.defineProperty(bad,Symbol.toPrimitive,{value:function(){throw marker;}});\ntry{+bad;}catch(e){if(e===marker)caught++;}\nObject.defineProperty(bad,Symbol.toPrimitive,{value:function(){return {};}});\ntry{+bad;}catch(e){if(e.name==='TypeError')caught++;}\nObject.defineProperty(bad,Symbol.toPrimitive,{value:function(){return function(){};}});\ntry{String(bad);}catch(e){if(e.name==='TypeError')caught++;}\nif(caught!==5)throw 'abrupt completions';\n'ok';\n"),
    ('conversion-protocol-symbol-intrinsic-and-wrapper-branding', "\nvar key=Symbol.toPrimitive,d=Object.getOwnPropertyDescriptor(Symbol,'toPrimitive');\nif(typeof key!=='symbol'||key.description!=='Symbol.toPrimitive'||d.writable||d.enumerable||d.configurable)throw 'key descriptor';\nvar hook=Symbol.prototype[key],hd=Object.getOwnPropertyDescriptor(Symbol.prototype,key);\nif(hook.name!=='[Symbol.toPrimitive]'||hook.length!==1||hd.writable||hd.enumerable||!hd.configurable)throw 'hook metadata';\nvar s=Symbol('token'),boxed=Object(s),poison={toString(){throw 'hint conversion';}};\nif(hook.call(s,poison)!==s||hook.call(boxed,poison)!==s||boxed.valueOf()!==s||boxed.toString()!=='Symbol(token)'||boxed.description!=='token')throw 'Symbol branding';\nvar o={};o[boxed]=7;if(o[s]!==7||Object.getOwnPropertySymbols(o)[0]!==s)throw 'boxed key';\nvar returns={};returns[key]=function(h){return s;};o[returns]=8;\nif(o[s]!==8||!(returns==s))throw 'Symbol result';\nvar caught=0;for(var action of [()=>String(boxed),()=>Number(boxed),()=>String(returns),()=>Number(returns),()=>hook.call(Symbol.prototype),()=>hook.call(Object.create(boxed)),()=>hook.call({})]){\ntry{action();}catch(e){if(e.name==='TypeError')caught++;}}\nif(caught!==7)throw 'invalid Symbol conversion';\nSymbol.toPrimitive=Symbol('replacement');if(Symbol.toPrimitive!==key||delete Symbol.toPrimitive)throw 'immutable key';\n'ok';\n"),
    ('conversion-protocol-native-callers-and-cycle-aware-array-join', "\nvar hints=[],o={};o[Symbol.toPrimitive]=function(h){hints.push(h);return h==='string'?'7':65;};\nif(Math.floor(o)!==65||String.fromCharCode(o)!=='A'||parseInt(o,10)!==7||new Uint8Array([o])[0]!==65||[o].join('|')!=='7')throw 'native callers';\nif(hints.join(',')!=='number,number,string,number,string')throw 'native hints';\nvar a=[1,2,3],calls=0,n={};n[Symbol.toPrimitive]=function(h){if(h!=='number')throw h;calls++;return 1;};\nObject.defineProperty(a,'length',{value:n});if(calls!==2||a.length!==1||1 in a)throw 'array length double conversion';\nvar boxed=new Number(8);boxed[Symbol.toPrimitive]=function(h){return h==='string'?'eight':4;};\nif([boxed].join()!=='eight'||Number(boxed)!==4||[new Number(3),new Boolean(false)].join('|')!=='3|false')throw 'wrapper join';\nvar cycle=[];cycle.push(cycle);if(cycle.join('!')!=='')throw 'plain array cycle';\ncycle[Symbol.toPrimitive]=function(h){if(h!=='string')throw h;return 'cycle';};\nif([cycle].join()!=='cycle')throw 'array exotic conversion';\n'ok';\n"),
    ('conversion-protocol-primitive-symbol-setters-use-original-receiver', "\nvar key=Symbol('setter'),symbol=Symbol('receiver'),seen=[];\nfunction setter(value){'use strict';seen.push(typeof this);seen.push(this===value);}\nObject.defineProperty(Number.prototype,key,{set:setter,configurable:true});\nObject.defineProperty(Boolean.prototype,key,{set:setter,configurable:true});\nObject.defineProperty(Symbol.prototype,key,{set:setter,configurable:true});\n(7)[key]=7;(false)[key]=false;symbol[key]=symbol;\ndelete Number.prototype[key];delete Boolean.prototype[key];delete Symbol.prototype[key];\nif(seen.join('|')!=='number|true|boolean|true|symbol|true')throw 'original primitive receivers';\nvar caught=0;for(var value of [7,false,symbol]){\nvalue[key]=1;if(value[key]!==undefined)throw 'retained primitive property';\ntry{(function(){'use strict';value[key]=2;})();}catch(e){if(e.name==='TypeError')caught++;}}\nif(caught!==3)throw 'strict primitive assignments';\n'ok';\n"),
])

# 0.37: original iterator/array-like regression fixtures.
COMPLETION_CASES.extend([
    ('apply-length-observes-hooks-and-index-order', "\nvar order='',length={};\nlength[Symbol.toPrimitive]=function(hint){if(hint!=='number'||this!==length)throw 'length hint';order+='c';return 2.9;};\nvar list={get length(){order+='l';return length;},get 0(){order+='a';this[1]='changed';return 'first';},1:'second'};\nfunction read(a,b){order+='f';if(arguments.length!==2)throw 'length truncation';return a+':'+b;}\nif(read.apply(null,list)!=='first:changed'||order!=='lcaf')throw 'apply order';\nfunction callable(a,b){};callable[0]='function';callable[1]='source';\nif(read.apply(null,callable)!=='function:source')throw 'callable array-like';\nvar marker={},touched=0,broken={get length(){return {valueOf(){throw marker;}};},get 0(){touched++;}};\ntry{read.apply(null,broken);throw 'missed length error';}catch(e){if(e!==marker)throw e;}\nif(touched!==0)throw 'early index read';\nvar caught=0;for(var candidate of [4,true,'text',Symbol('source')]){\ntry{read.apply(null,candidate);}catch(e){if(e.name==='TypeError')caught++;}}\ntry{read.apply(null,{length:Symbol('length')});}catch(e){if(e.name==='TypeError')caught++;}\nif(caught!==5)throw 'array-like validation';\n'ok';\n"),
])

COMPLETION_CASES.extend([
    ('iterator-protocol-intrinsic-aliases-descriptors-and-native-receivers', "\nvar k=Symbol.iterator,d=Object.getOwnPropertyDescriptor(Symbol,'iterator');\nif(typeof k!=='symbol'||k.description!=='Symbol.iterator'||d.writable||d.enumerable||d.configurable)throw 'key descriptor';\nif(Array.prototype[k]!==Array.prototype.values||Map.prototype[k]!==Map.prototype.entries||Set.prototype[k]!==Set.prototype.values||Set.prototype.keys!==Set.prototype.values)throw 'native aliases';\nif(Uint8Array.prototype[k]!==Uint8Array.prototype.values||Array.prototype[k].name!=='values'||Array.prototype[k].length!==0)throw 'array metadata';\nvar it=[2].values(),proto=Object.getPrototypeOf(Object.getPrototypeOf(it));\nif(it[k]()!==it||proto[k].call(9)!==9||proto[k].name!=='[Symbol.iterator]'||proto[k].length!==0)throw 'iterator identity';\nvar pd=Object.getOwnPropertyDescriptor(proto,k);if(pd.enumerable||!pd.writable||!pd.configurable)throw 'iterator descriptor';\nvar err='';try{it.next.call({});}catch(e){err=e.name;}if(err!=='TypeError')throw 'brand';\nfunction args(a){var old=Array.prototype.values;Array.prototype.values=function(){throw 'replacement';};try{var d=Object.getOwnPropertyDescriptor(arguments,k);if(d.value!==old||d.enumerable||!d.writable||!d.configurable)throw 'arguments alias';var n=0;for(var x of arguments)n+=x;return n;}finally{Array.prototype.values=old;}}\nif(args(3,4)!==7)throw 'arguments iteration';\ntest.record('ok');\n\n'ok';"),
    ('iterator-protocol-caches-next-and-orders-done-before-value', "\nvar order='',steps=0,iter={};\nObject.defineProperty(iter,'next',{get:function(){order+='n';return function(){if(this!==iter||arguments.length!==0)throw 'next receiver';steps++;if(steps===1)Object.defineProperty(iter,'next',{value:function(){throw 'reread';}});return {get done(){order+='d';return steps>2;},get value(){order+='v';return steps;}};};},configurable:true});\nvar items={};Object.defineProperty(items,Symbol.iterator,{get:function(){order+='g';return function(){if(this!==items||arguments.length!==0)throw 'method receiver';order+='i';return iter;};}});\nvar total=0;for(var x of items)total+=x;\nif(total!==3||order!=='gindvdvd')throw order;\nvar functionResult=function(){};functionResult.done=true;\nvar functionIterator=function(){};functionIterator.next=function(){return functionResult;};\nvar callableItems={};callableItems[Symbol.iterator]=function(){return functionIterator;};for(var x of callableItems)throw 'done function';\ntest.record('ok');\n\n'ok';"),
    ('iterator-protocol-closes-break-return-outer-continue-and-target-throws', "\nvar closes='',count=0;function values(label){var it={next(){return {done:false,value:++count};},return(){closes+=label;return {};}};it[Symbol.iterator]=function(){return this;};return it;}\nfor(var x of values('b')){if(x===1)continue;break;}\nfunction stop(){for(var x of values('r'))return x;}stop();\nvar rounds=0;outer:for(var o of values('o')){for(var i of values('i')){rounds++;if(rounds===1)continue outer;break outer;}}\nvar marker={},target={set x(v){throw marker;}};try{for(target.x of values('t')){throw 'body';}}catch(e){if(e!==marker)throw 'target exception';}\nif(closes!=='briiot')throw closes;\ntest.record('ok');\n\n'ok';"),
    ('iterator-protocol-close-error-precedence-and-live-return-lookup', "\nvar original={},closing={},count=0;\nfunction create(mode){var it={next(){return {done:false,value:1};}};it[Symbol.iterator]=function(){return this;};Object.defineProperty(it,'return',{get:function(){count++;if(mode===0)throw closing;if(mode===1)return function(){throw closing;};if(mode===2)return function(){return 1;};return 7;}});return it;}\nfor(var mode=0;mode<4;mode++){\ntry{for(var x of create(mode))throw original;}catch(e){if(e!==original)throw 'throw precedence';}\ntry{for(var x of create(mode))break;}catch(e){if(mode<2?e!==closing:e.name!=='TypeError')throw 'break precedence';}\nfunction exit(){for(var x of create(mode))return 7;}try{exit();throw 'no close error';}catch(e){if(mode<2?e!==closing:e.name!=='TypeError')throw 'return precedence';}\n}\nif(count!==12)throw 'close count';\nvar it={next(){return {value:1,done:false};}};it[Symbol.iterator]=function(){return this;};var seen='';\nfor(var x of it){it.return=function(){if(this!==it||arguments.length!==0)throw 'return receiver';seen='live';return function(){};};break;}\nif(seen!=='live')throw 'return was cached';\ntest.record('ok');\n\n'ok';"),
    ('iterator-protocol-step-and-lookup-failures-do-not-close', "\nvar marker={},closes=0;\nfunction check(mode){var it={return(){closes++;return {};}};it[Symbol.iterator]=function(){return this;};\nObject.defineProperty(it,'next',{get:function(){if(mode===0)throw marker;if(mode===1)return 3;return function(){if(mode===2)throw marker;if(mode===3)return 4;if(mode===4)return {get done(){throw marker;}};return {done:false,get value(){throw marker;}};};}});\ntry{for(var x of it)throw 'body';throw 'missing failure';}catch(e){if(mode===1||mode===3){if(e.name!=='TypeError')throw 'step type';}else if(e!==marker)throw 'step identity';}}\nfor(var mode=0;mode<6;mode++)check(mode);\nvar bad={};bad[Symbol.iterator]=function(){return 3;};try{for(var x of bad){};throw 'accepted primitive iterator';}catch(e){if(e.name!=='TypeError')throw e;}\nvar done={next(){return {done:true,get value(){throw 'value read on done';}};},return(){closes++;return {};}};done[Symbol.iterator]=function(){return this;};for(var x of done)throw 'done body';\nif(closes!==0)throw 'closed failed step';\ntest.record('ok');\n\n'ok';"),
    ('iterator-protocol-native-live-iteration-overrides-and-string-code-points', "\nvar a=[1,,3],got='';for(var x of a){got+=String(x)+',';if(a.length===3)a.push(4);}\nif(got!=='1,undefined,3,4,')throw got;\nvar it;var aLike={length:2,get 0(){return it.next().value;},1:9};it=Array.prototype.values.call(aLike);\nif(it.next().value!==9||!it.next().done)throw 'reentrant advance';\nvar single={next(){return {done:false,value:8};}};single[Symbol.iterator]=function(){return this;};\na[Symbol.iterator]=function(){return single;};var first=0;for(var x of a){first=x;break;}if(first!==8)throw 'array override';\nvar m=new Map([[1,2]]);m[Symbol.iterator]=function(){return [7].values();};if(new Set(m).has(7)!==true)throw 'collection override';\nvar t=new Uint8Array([1,2]);t[Symbol.iterator]=function(){return [6,5].values();};var seen='';for(var x of t)seen+=x;if(seen!=='65'||Uint8Array.from(t).join()!=='6,5')throw 'typed override';\nvar points='',lengths='';for(var ch of 'aé𐀀'){points+=ch;lengths+=ch.length;}\nif(points!=='aé𐀀'||lengths!=='112')throw 'string points';\nvar si=String.prototype[Symbol.iterator].call({toString(){return 'xy';}});if(si[Symbol.iterator]()!==si||si.next().value!=='x'||si.next().value!=='y'||!si.next().done||!si.next().done)throw 'string iterator';\ntest.record('ok');\n\n'ok';"),
    ('iterator-protocol-collection-consumers-close-entry-and-adder-errors', "\nvar closes=0,marker={};function items(v){var it={next(){return {done:false,value:v};},return(){closes++;return {};}};it[Symbol.iterator]=function(){return this;};return it;}\ntry{new Map(items(3));}catch(e){if(e.name!=='TypeError')throw e;}\ntry{new Map(items({get 0(){throw marker;}}));}catch(e){if(e!==marker)throw e;}\nvar old=Map.prototype.set;Map.prototype.set=function(){throw marker;};try{new Map(items([1,2]));}catch(e){if(e!==marker)throw e;}Map.prototype.set=old;\nvar old=Set.prototype.add;Set.prototype.add=function(){throw marker;};try{new Set(items(1));}catch(e){if(e!==marker)throw e;}Set.prototype.add=old;\nif(closes!==4)throw 'collection close';\nvar custom={};custom[Symbol.iterator]=function(){return [3,4].values();};if(new Uint8Array(custom).join()!=='3,4'||Uint8Array.from(custom,function(v,i){return v+i;}).join()!=='3,5')throw 'typed custom consumption';\ntest.record('ok');\n\n'ok';"),
    ('iterable-builtins-array-from-native-and-array-like-values', "\nvar sparse=Array(3);sparse[1]=4;\nvar dense=Array.from(sparse,function(v,i){return v===undefined?'hole'+i:v;});\nif(dense.join('|')!=='hole0|4|hole2'||!Object.hasOwn(dense,'0'))throw 'holes';\nif(Array.from('A😀').join('|')!=='A|😀')throw 'Unicode iterator';\nif(Array.from(new Set([3,2,3])).join(',')!=='3,2')throw 'Set';\nif(Array.from(new Map([['x',2],['y',3]])).map(function(e){return e.join(':');}).join(',')!=='x:2,y:3')throw 'Map';\nif(Array.from(new Uint8Array([7,8])).join(',')!=='7,8')throw 'typed';\nvar receiver={base:5};var mapped=Array.from({0:2,2:4,length:3},function(v,i){if(this!==receiver)throw 'mapper receiver';return v===undefined?'empty':this.base+v+i;},receiver);\nif(mapped.join('|')!=='7|empty|11')throw 'array-like';\nif(Array.from(4).length||Array.from(false).length||Array.from(Symbol('s')).length)throw 'scalar';\nif(Array.from.length!==1||Array.from.name!=='from'||Object.fromEntries.length!==1||Object.fromEntries.name!=='fromEntries')throw 'metadata';\nvar d=Object.getOwnPropertyDescriptor(Array,'from');\nif(d.enumerable||!d.writable||!d.configurable)throw 'descriptor';\nvar errors=0;for(var x of [null,undefined]){try{Array.from(x);}catch(e){if(e.name==='TypeError')errors++;}}\nif(errors!==2)throw 'nullish';\n\ntest.record('ok');\n'ok';\n\n'ok';"),
    ('iterable-builtins-array-from-captures-method-before-constructor', "\nvar trace='',index=0,input={},iter={};\nObject.defineProperty(iter,'next',{get:function(){trace+='N';return function(){trace+='X';var n=index++;return {get done(){trace+='D';return n>0;},get value(){trace+='V';return 6;}};};}});\nObject.defineProperty(input,Symbol.iterator,{configurable:true,get:function(){trace+='I';return function(){trace+='M';if(this!==input)throw 'iterator receiver';return iter;};}});\nfunction C(){trace+='C';Object.defineProperty(input,Symbol.iterator,{value:function(){throw 'new method';}});}\nObject.defineProperty(C.prototype,'length',{set:function(n){trace+='L';this.total=n;}});\nvar result=Array.from.call(C,input,function(v,i){trace+='F';return v+i;});\nif(trace!=='ICMNXDVFXDL'||result[0]!==6||result.total!==1||!(result instanceof C))throw trace;\nvar touched=0;var bad={get [Symbol.iterator](){touched++;throw 'read';}};\ntry{Array.from(bad,null);throw 'accepted mapper';}catch(e){if(e.name!=='TypeError')throw e;}\nif(touched!==0)throw 'mapper validation order';\n\ntest.record('ok');\n'ok';\n\n'ok';"),
    ('iterable-builtins-array-from-constructors-and-scalar-receivers', "\nvar trace='',input={0:'a',1:'b',get length(){trace+='L';return {[Symbol.toPrimitive]:function(h){trace+=h;return 2.8;}};}};\nfunction C(n){trace+='C'+n;this.constructorLength=n;}\nvar result=Array.from.call(C,input);\nif(trace!=='LnumberC2'||result.constructorLength!==2||result.length!==2||result[1]!=='b')throw trace;\nfunction Bound(prefix,n){this.prefix=prefix;this.n=n;}\nvar bound=Bound.bind(null,'kept'),made=Array.from.call(bound,{length:2,0:7,1:8});\nif(made.prefix!=='kept'||made.n!==2||made.length!==2||!(made instanceof Bound))throw 'bound constructor';\nvar arrow=()=>{};\nif(!Array.isArray(Array.from.call(arrow,{length:1,0:4})))throw 'nonconstructor fallback';\nObject.defineProperty(Number.prototype,'length',{get:function(){'use strict';if(typeof this!=='object'||this.valueOf()!==7)throw 'length receiver';return 1;}});\nObject.defineProperty(Number.prototype,'0',{get:function(){'use strict';if(typeof this!=='object'||this.valueOf()!==7)throw 'index receiver';return 9;}});\nif(Array.from(7).join()!=='9')throw 'boxed scalar';\nvar list=[1,2];list[Symbol.iterator]=null;\nif(Array.from(list).join()!=='1,2')throw 'array-like fallback';\n\ntest.record('ok');\n'ok';\n\n'ok';"),
    ('iterable-builtins-array-from-closes-only-active-consumers', "\nvar closed=0,marker={kind:'mapper'};\nfunction make(mode){var n=0;var iter={next:function(){return {done:n++>0,value:5};},return:function(){closed++;if(mode==='throw')throw 'close';if(mode==='primitive')return 1;return {};}};return {[Symbol.iterator]:function(){return iter;}};}\nfor(var mode of ['normal','throw','primitive']){closed=0;try{Array.from(make(mode),function(){throw marker;});throw 'accepted mapping';}catch(e){if(e!==marker)throw 'exception precedence';}if(closed!==1)throw 'mapper close';}\nclosed=0;function Locked(){return Object.preventExtensions({});}\ntry{Array.from.call(Locked,make('normal'));throw 'defined locked';}catch(e){if(e.name!=='TypeError')throw e;}\nif(closed!==1)throw 'definition close';\nfor(var phase of ['next','done','value']){closed=0;var iter={next:function(){if(phase==='next')throw marker;return {get done(){if(phase==='done')throw marker;return false;},get value(){throw marker;}};},return:function(){closed++;return {};}};\ntry{Array.from({[Symbol.iterator]:function(){return iter;}});throw 'accepted iterator';}catch(e){if(e!==marker)throw e;}if(closed!==0)throw 'step closed';}\nclosed=0;function LengthFail(){}Object.defineProperty(LengthFail.prototype,'length',{set:function(){throw marker;}});\ntry{Array.from.call(LengthFail,make('normal'));throw 'accepted length';}catch(e){if(e!==marker)throw e;}\nif(closed!==0)throw 'completed iterator closed';\n\ntest.record('ok');\n'ok';\n\n'ok';"),
    ('iterable-builtins-from-entries-reads-pairs-before-key-conversion', "\nvar trace='',key={[Symbol.toPrimitive]:function(h){trace+='K'+h;return 'answer';}},entry={get 0(){trace+='0';return key;},get 1(){trace+='1';return 7;}};\nvar setters=0;Object.defineProperty(Object.prototype,'answer',{set:function(){setters++;},configurable:true});\nvar symbol=Symbol('key'),result=Object.fromEntries([entry,['a',1],[symbol,5],['a',9]]);\nif(trace!=='01Kstring'||setters!==0||result.answer!==7||result.a!==9||result[symbol]!==5)throw trace;\nvar d=Object.getOwnPropertyDescriptor(result,'answer');if(!d.enumerable||!d.writable||!d.configurable||d.get!==undefined)throw 'own data';\nif(Object.keys(result).join()!=='answer,a'||Object.getOwnPropertySymbols(result)[0]!==symbol)throw 'keys';\nfunction pair(){}pair[0]='function';pair[1]=11;\nvar ignored=false;var other=Object.fromEntries.call(function(){ignored=true;},[pair]);\nif(ignored||other.function!==11||Object.getPrototypeOf(other)!==Object.prototype)throw 'generic entry or receiver';\n\ntest.record('ok');\n'ok';\n\n'ok';"),
    ('iterable-builtins-from-entries-closes-entry-and-conversion-failures', "\nvar closed=0,marker={kind:'entry'},phase='primitive';\nfunction make(entry){var once=false;return {[Symbol.iterator]:function(){return {next:function(){if(phase==='next')throw marker;return {done:once,get value(){once=true;return entry;}};},return:function(){closed++;throw 'close';}};}};}\ntry{Object.fromEntries(make(3));throw 'accepted primitive';}catch(e){if(e.name!=='TypeError')throw e;}\nif(closed!==1)throw 'primitive close';\nfor(phase of ['zero','one','key']){closed=0;var entry={get 0(){if(phase==='zero')throw marker;return {[Symbol.toPrimitive]:function(){throw marker;}};},get 1(){if(phase==='one')throw marker;return 5;}};\ntry{Object.fromEntries(make(entry));throw 'accepted entry';}catch(e){if(e!==marker)throw 'entry precedence';}if(closed!==1)throw 'entry close';}\nvar invalid=0;for(var value of [null,undefined,3,{},'x']){try{Object.fromEntries(value);}catch(e){if(e.name==='TypeError')invalid++;}}\nif(invalid!==5)throw 'input validation';\n\ntest.record('ok');\n'ok';\n\n'ok';"),
    ('iterable-builtins-scalar-descriptor-sources-have-no-own-keys', "\nvar calls=0;Object.defineProperty(Number.prototype,'x',{enumerable:true,get:function(){calls++;throw 'inherited descriptor';}});\nvar target={kept:4};\nif(Object.defineProperties(target,5)!==target||Object.defineProperties(target,false)!==target||Object.defineProperties(target,Symbol('d'))!==target)throw 'identity';\nvar prototype={p:8};var created=Object.create(prototype,7);\nif(Object.getPrototypeOf(created)!==prototype||Object.keys(created).length||calls||target.kept!==4)throw 'empty own sources';\nvar count=0;for(var bad of [null,undefined]){try{Object.defineProperties(target,bad);}catch(e){if(e.name==='TypeError')count++;}}\ntry{Object.defineProperties(3,5);}catch(e){if(e.name==='TypeError')count++;}\nif(count!==3)throw 'object requirements';\n\ntest.record('ok');\n'ok';\n\n'ok';"),
])
NEGATIVE.extend([
    ('iterator-custom-infinite-result-budget-atomic', "var item={done:false,value:1},flow={};flow[Symbol.iterator]=function(){return {next(){return item;},return(){test.record('cleanup');return {};}};};try{for(var value of flow){}}catch(e){test.record('caught');}finally{test.record('finally');}"),
    ('iterator-close-fatal-budget-atomic', "var flow={};flow[Symbol.iterator]=function(){return {next(){return {value:1,done:false};},return(){while(true){}}};};try{for(var value of flow){throw 'body';}}catch(e){test.record('caught');}finally{test.record('finally');}"),
])

# 0.38: original spread/rest semantic and bounds regressions.
COMPLETION_CASES.extend([
    ('rest-parameters-bind-fresh-arrays-and-exclude-rest-from-length', "\nfunction all(...tail){return tail;}\nfunction fixed(a,b,...tail){if(a!==1||b!==2)throw 'fixed bindings';return tail;}\nif(all.length!==0||fixed.length!==2)throw 'function lengths';\nvar first=all(3,4),second=all(3,4),empty=all();\nif(!Array.isArray(first)||first.join('|')!=='3|4'||first===second||empty.length!==0||empty===all())throw 'fresh arrays';\nfirst[0]=9;if(second[0]!==3)throw 'independent array';\nif(fixed(1,2,5,6).join('|')!=='5|6'||fixed(1,2).length!==0)throw 'rest range';\nfunction short(a,b,...tail){return a+':'+b+':'+tail.length;}\nif(short(7)!=='7:undefined:0')throw 'missing fixed argument';\nvar descriptor=Object.getOwnPropertyDescriptor(second,'0');\nif(!descriptor.writable||!descriptor.enumerable||!descriptor.configurable||second[Symbol.iterator]!==Array.prototype.values)throw 'array shape';\n'ok';\n"),
    ('rest-parameters-methods-constructors-bind-and-arrow-lexical-context', "\nfunction Pack(head,...tail){this.head=head;this.tail=tail;}\nvar Bound=Pack.bind(null,7),instance=new Bound(8,9);\nif(Pack.length!==1||Bound.length!==0||!(instance instanceof Pack)||instance.head!==7||instance.tail.join()!=='8,9')throw 'bound construction';\nvar object={n:10,method(head,...tail){return this.n+head+tail[0];}};\nif(object.method.length!==1||object.method(2,3)!==15||object.method.call({n:20},2,3)!==25)throw 'method rest';\nfunction outer(head,...tail){return (...inner)=>this.n+':'+arguments[0]+':'+tail.join()+':'+inner.join();}\nvar arrow=outer.call({n:5},6,7,8);\nif(arrow.length!==0||arrow.call({n:99},1,2)!=='5:6:7,8:1,2')throw 'lexical context';\nvar expression=(head,...tail)=>head+tail.length;\nif(expression.length!==1||expression(4,5,6)!==6)throw 'expression arrow';\nvar caught=0;try{new arrow();}catch(e){if(e.name==='TypeError')caught++;}\ntry{new object.method();}catch(e){if(e.name==='TypeError')caught++;}\nif(caught!==2)throw 'nonconstructible rest methods';\n'ok';\n"),
    ('rest-parameters-arguments-are-unmapped-with-restricted-callee', "\nfunction inspect(a,...tail){\nvar original=arguments;a=7;tail[0]=8;\nif(original[0]!==1||original[1]!==2)throw 'mapped parameter';\noriginal[0]=3;original[1]=4;\nif(a!==7||tail[0]!==8||original.length!==2)throw 'mapped argument';\nvar d=Object.getOwnPropertyDescriptor(original,'callee'),caught=0;\nif(typeof d.get!=='function'||typeof d.set!=='function'||d.enumerable||d.configurable)throw 'callee descriptor';\ntry{original.callee;}catch(e){if(e.name==='TypeError')caught++;}\ntry{d.set.call(original,1);}catch(e){if(e.name==='TypeError')caught++;}\nif(caught!==2)throw 'callee restrictions';\narguments=9;if(arguments!==9)throw 'sloppy arguments binding';return original;\n}\nvar kept=inspect(1,2);if(kept[0]!==3||kept[1]!==4)throw 'retained arguments';\nfunction mapped(a){a=7;if(arguments[0]!==7)throw 'simple mapping';arguments[0]=8;return a;}\nif(mapped(1)!==8)throw 'simple mappings retained';\n'ok';\n"),
    ('rest-parameters-arguments-names-and-body-bindings-keep-scope-rules', "\nfunction named(...arguments){if(!Array.isArray(arguments))throw 'named rest';return ()=>arguments;}\nvar arrow=named(2,3);if(arrow().join()!=='2,3')throw 'rest arguments shadow';\nfunction lexical(a,...tail){let arguments=8;return ()=>arguments+tail[0]+a;}\nif(lexical(1,2)()!==11)throw 'body lexical arguments';\nfunction replaced(...tail){var tail;return tail;}\nif(replaced(4,5).join()!=='4,5')throw 'var rest binding';\nfunction nested(...tail){return function(){return tail;};}\nvar one=nested(1),two=nested(2);if(one()[0]!==1||two()[0]!==2)throw 'closure identity';\nfunction overwrite(...tail){function tail(){return 4;}return tail();}\nif(overwrite(8)!==4)throw 'body function replacement';\n'ok';\n"),
    ('rest-parameters-strict-context-and-directive-prologue-are-distinct', '\nfunction strictOuter(){\'use strict\';return function(a,...tail){\nif(this!==undefined)throw \'inherited strict receiver\';\na=4;if(arguments[0]!==1)throw \'strict arguments\';return tail;\n};}\nif(strictOuter()(1,2,3).join()!==\'2,3\')throw \'strict inheritance\';\nfunction harmless(...tail){(\'use strict\');return tail.length;}\nfunction escaped(...tail){\'use\\x20strict\';return tail.length;}\nfunction late(...tail){var x=1;\'use strict\';return tail.length+x;}\nif(harmless(1)!==1||escaped(1,2)!==2||late(1)!==2)throw \'non-directives\';\nvar caught=0;for(var text of [\n"function f(...tail){\'use strict\';}",\n"\'use strict\';function f(...tail){\'use strict\';}",\n"var f=(...tail)=>{\'use strict\';};",\n"var o={m(...tail){\'other\';\'use strict\';}}",\n"\'use strict\';function f(...arguments){}",\n"\'use strict\';function f(...eval){}"\n]){try{eval(text);}catch(e){if(e.name===\'SyntaxError\')caught++;}}\nif(caught!==6)throw \'strict early errors\';\n\'ok\';\n'),
    ('rest-parameters-invalid-grammar-is-atomic', 'var ran=false,caught=0;for(var text of ["function f(a,...a){}", "function f(a,a,...tail){}", "function f(...tail,next){}", "function f(...tail,){}", "function f(...tail=[]){}", "var f=(a,...a)=>a;", "var f=(...tail,)=>tail;", "var o={set x(...tail){}};", "var o={get x(...tail){}};", "function f(...tail){let tail;}", "function outer(){function invalid(...tail){\'use strict\';}}"]){try{eval("ran=true;"+text);}catch(e){if(e.name===\'SyntaxError\')caught++;}if(ran)throw \'partial parse execution\';}if(caught!==11)throw \'invalid rest grammar\';\n\'ok\';'),
    ('spread-arrays-preserve-holes-native-values-and-own-definitions', "\nvar a=[,1,...Array(2),,];\nif(a.length!==5||Object.hasOwn(a,0)||!Object.hasOwn(a,2)||!Object.hasOwn(a,3)||Object.hasOwn(a,4))throw 'elisions and spread holes';\nif(a[2]!==undefined||a[1]!==1)throw 'array values';\nif([...new Set([2,2,3]),...new Uint8Array([4,5])].join()!=='2,3,4,5')throw 'native values';\nif([...new Map([['key',9]])][0][1]!==9)throw 'map entries';\nvar points=[...'A😀é'];if(points.length!==3||points[1].length!==2||points.join('')!=='A😀é')throw 'code points';\nvar original=[1,2];original[Symbol.iterator]=function(){return ['changed'].values();};\nif([0,...original,...[],3].join()!=='0,changed,3')throw 'override and empty spread';\nvar hits=0;Object.defineProperty(Array.prototype,'0',{set:function(){hits++;},configurable:true});\nvar own=[...[7]];delete Array.prototype[0];\nvar d=Object.getOwnPropertyDescriptor(own,'0');\nif(hits!==0||own[0]!==7||!d.writable||!d.enumerable||!d.configurable)throw 'own definition';\n'ok';\n"),
    ('spread-calls-preserve-callee-resolution-and-receivers', "\nvar order='',obj={n:4,get run(){order+='g';return function(a,b,c){order+='c';return this.n+a+b+c;};}};\nvar input={};input[Symbol.iterator]=function(){order+='i';Object.defineProperty(obj,'run',{value:function(){throw 'replaced method';},configurable:true});return [2].values();};\nfunction mark(v){order+=v;return v;}\nif(obj.run(mark(1),...input,mark(3))!==10||order!=='g1i3c')throw 'callee order';\nvar key=Symbol('method');obj[key]=function(v){if(this!==obj)throw 'symbol receiver';return v;};\nif(obj[key](...[8])!==8)throw 'symbol method';\nfunction plain(a){'use strict';if(this!==undefined)throw 'plain receiver';return a;}\nif(plain(...[6])!==6||String.fromCharCode(...[65,66])!=='AB')throw 'plain and native calls';\nvar bound=function(a,b){return this.n+a+b;}.bind(obj,1);\nif(bound(...[2])!==7)throw 'bound receiver';\nvar iterated=0,items={};items[Symbol.iterator]=function(){iterated++;return [1].values();};\nvar notCallable={f:3};try{notCallable.f(...items);throw 'missing call error';}catch(e){if(e.name!=='TypeError')throw e;}\nif(iterated!==1)throw 'arguments before callable check';\ntry{null.f(...items);throw 'missing receiver error';}catch(e){if(e.name!=='TypeError')throw e;}\nif(iterated!==1)throw 'receiver error before arguments';\n'ok';\n"),
    ('spread-constructors-preserve-resolution-bound-arguments-and-native-behavior', "\nvar order='';function Pair(a,b){order+='c';this.sum=a+b;}\nvar holder={get C(){order+='g';return Pair;}},items={};\nitems[Symbol.iterator]=function(){order+='i';holder.C=0;return [2,3].values();};\nvar pair=new holder.C(...items);\nif(pair.sum!==5||!(pair instanceof Pair)||order!=='gic')throw 'constructor resolution';\nvar Bound=Pair.bind(null,4),other=new Bound(...[5]);\nif(other.sum!==9||!(other instanceof Pair)||!(other instanceof Bound))throw 'bound construction';\nvar sparse=new Array(...[3]);if(sparse.length!==3||Object.hasOwn(sparse,0))throw 'native Array constructor';\nif(new Number(...[8]).valueOf()!==8||new Map(...[[['key',9]]]).get('key')!==9)throw 'other native constructors';\nvar seen=0,flow={};flow[Symbol.iterator]=function(){seen++;return [].values();};\nvar Arrow=()=>0;try{new Arrow(...flow);throw 'missing new error';}catch(e){if(e.name!=='TypeError')throw e;}\nvar Invalid=0;try{new Invalid(...flow);throw 'missing invalid error';}catch(e){if(e.name!=='TypeError')throw e;}\nif(seen!==2)throw 'new evaluates arguments before constructability';\n'ok';\n"),
    ('spread-custom-protocol-caches-next-and-preserves-live-values', "\nvar log='',input={},it,count=0;\nObject.defineProperty(input,Symbol.iterator,{get:function(){log+='g';if(this!==input)throw 'lookup receiver';return function(){\nif(this!==input||arguments.length!==0)throw 'iterator receiver';log+='i';\nit={};Object.defineProperty(it,'next',{configurable:true,get:function(){log+='n';if(this!==it)throw 'next lookup';return function(){\nif(this!==it||arguments.length!==0)throw 'step receiver';count++;Object.defineProperty(it,'next',{value:function(){throw 'uncached next';}});\nreturn {get done(){log+='d';return count>2;},get value(){log+='v';return count;}};\n};}});return it;};}});\nif([0,...input,3].join()!=='0,1,2,3'||log!=='gindvdvd')throw 'custom read order';\nvar live=[1,2];Object.defineProperty(live,'0',{get:function(){live.push(3);return 1;},configurable:true});\nif([...live].join()!=='1,2,3')throw 'live native length';\nfunction fromArguments(){return [...arguments].join(':');}\nif(fromArguments(7,8)!=='7:8')throw 'arguments iterator';\nif([...[]].length!==0||(function(){return arguments.length;})(...[],...[])!==0)throw 'empty consumption';\n'ok';\n"),
    ('spread-step-failures-preserve-exceptions-and-do-not-close', "\nvar marker={},closed=0,later=0,called=0;\nfunction make(phase){var input={};Object.defineProperty(input,Symbol.iterator,{get:function(){\nif(phase==='lookup')throw marker;return function(){if(phase==='call')throw marker;return {\nget next(){if(phase==='nextget')throw marker;return function(){if(phase==='next')throw marker;if(phase==='result')return 0;\nreturn {get done(){if(phase==='done')throw marker;return false;},get value(){throw marker;}};};},\nreturn(){closed++;return {};}\n};};}});return input;}\nfunction touch(){later++;return 0;}function Target(){called++;}\nfor(var phase of ['lookup','call','nextget','next','result','done','value']){\ntry{[0,...make(phase),touch()];throw 'array missed error';}catch(e){if(phase==='result'?e.name!=='TypeError':e!==marker)throw e;}\ntry{Target(0,...make(phase),touch());throw 'call missed error';}catch(e){if(phase==='result'?e.name!=='TypeError':e!==marker)throw e;}\ntry{new Target(0,...make(phase),touch());throw 'new missed error';}catch(e){if(phase==='result'?e.name!=='TypeError':e!==marker)throw e;}\n}\nif(closed!==0||later!==0||called!==0)throw 'unexpected continuation or cleanup';\nvar failures=0;for(var bad of [null,undefined,{},3,false,Symbol('bad')]){\ntry{[...bad];}catch(e){if(e.name==='TypeError')failures++;}}\nif(failures!==6)throw 'noniterable validation';\n'ok';\n"),
    ('spread-argument-and-array-limits-remain-fatal-and-bounded', "\nfunction size(){return arguments.length;}\nif(size(...Array(256),...[])!==256||size(1,...Array(255))!==256)throw 'argument boundary';\nvar dense=[...Array(4096)];if(dense.length!==4096||!Object.hasOwn(dense,4095))throw 'property boundary';\n'ok';\n"),
])
NEGATIVE.extend([
    ('spread-expanded-argument-bound-atomic', "function f(){test.record('called');}try{f(...Array(257));}catch(e){test.record('caught');}finally{test.record('finally');}"),
    ('spread-dense-property-bound-atomic', "try{[...Array(4097)];}catch(e){test.record('caught');}finally{test.record('finally');}"),
    ('spread-native-host-gate-atomic', "try{[...test];}catch(e){test.record('caught');}finally{test.record('finally');}"),
])

# 0.39: JSON callbacks and input conversion now use their original positive
# expectations. Keep the former negative IDs and exact source programs for audit.
COMPLETION_CASES.extend([
    ('json-reviver-unsupported-atomic', 'try{JSON.parse("1",function(){test.record(1);});}catch(e){test.record(2);}finally{test.record(3);}'),
    ('json-replacer-function-unsupported-atomic', 'try{JSON.stringify(1,function(){test.record(1);});}catch(e){test.record(2);}'),
    ('json-replacer-list-unsupported-atomic', 'try{JSON.stringify({x:1},["x"]);}catch(e){test.record(1);}'),
    ('json-tojson-hook-unsupported-atomic', 'try{JSON.stringify({toJSON(){test.record(1);return 2;}});}catch(e){test.record(2);}'),
    ('json-implicit-input-coercion-unsupported-atomic', 'try{JSON.parse({toString(){test.record(1);return "1";}});}catch(e){test.record(2);}'),
])

# 0.39: frozen original common-builtins comparisons and fatal resource bounds.
COMPLETION_CASES.extend([
    ('flat-holes-depth-and-nonarrays', "\nvar a=[,1,[,2,[3,,4]],undefined],zero=a.flat(0),one=a.flat(),deep=a.flat(Infinity);\ntest.record(zero.length);test.record(Object.hasOwn(zero,0));test.record(zero[1]===a[2]);\ntest.record(one.length);test.record(one[2]===a[2][2]);test.record(deep.join('|'));\ntest.record(a.flat(-3).length);test.record(a.flat(NaN).length);test.record(a.flat(1.9).length);\nvar o={0:'x',length:1};test.record([o,new Uint8Array([2,3])].flat().length);\n'ok';\n"),
    ('flat-live-getters-depth-conversion-and-inherited-indices', "\nvar log='',inner=[2],a=[1,inner,3],depth={valueOf(){log+='d';a[0]=8;return 2;}};\nObject.defineProperty(a,'1',{get:function(){log+='g';inner.push(4);delete a[2];return inner;}});\ntest.record(a.flat(depth).join());test.record(log);\nvar parent={1:[7]},o=Object.create(parent);o[0]=[5];o.length=3;\ntest.record(Array.prototype.flat.call(o).join());test.record(Object.hasOwn(o,1));\n'ok';\n"),
    ('flatmap-callback-order-receiver-holes-and-mutation', "\nvar a=[,1,2,3],seen=[],receiver={n:10};\nvar result=a.flatMap(function(value,index,original){'use strict';seen.push(index+':'+value+':'+(this===receiver)+':'+(original===a));if(index===1){a[2]=8;a.push(9);delete a[3];}return [,value,this.n];},receiver);\ntest.record(result.join());test.record(seen.join('|'));test.record(result.length);\nvar nested=[1].flatMap(function(){return [[2]];});test.record(Array.isArray(nested[0]));\nvar calls=0;try{Array.prototype.flatMap.call({get length(){calls++;return 0;}},42);}catch(e){test.record(e.name);}test.record(calls);\n'ok';\n"),
    ('flat-species-constructor-own-properties-and-order', "\nvar log='',target={},a=[1,[2]],holder={};\nObject.defineProperty(a,'constructor',{get:function(){log+='c';return holder;}});\nObject.defineProperty(holder,Symbol.species,{get:function(){log+='s';return function(n){log+='n'+n;return target;};}});\nObject.setPrototypeOf(target,{set 0(v){throw 'inherited setter';}});\nvar result=a.flat({valueOf(){log+='d';return 1;}}),d=Object.getOwnPropertyDescriptor(result,'0');\ntest.record(result===target);test.record(result[0]);test.record(result[1]);test.record(result.length);test.record(log);test.record(d.writable&&d.enumerable&&d.configurable);\ntest.record(Object.getOwnPropertyDescriptor(Array,Symbol.species).get.call({x:1}).x);\n'ok';\n"),
    ('flat-species-fallbacks-and-abrupt-completion', "\nvar a=[1],caught=0;a.constructor=undefined;test.record(Array.isArray(a.flat()));a.constructor={};test.record(Array.isArray(a.flat()));\na.constructor={};a.constructor[Symbol.species]=null;test.record(Array.isArray(a.flat()));\na.constructor=3;try{a.flat();}catch(e){if(e.name==='TypeError')caught++;}\na.constructor={};a.constructor[Symbol.species]=()=>({});try{a.flat();}catch(e){if(e.name==='TypeError')caught++;}\nvar marker={},visited=0;a.constructor={};Object.defineProperty(a.constructor,Symbol.species,{get:function(){throw marker;}});\ntry{a.flatMap(function(){visited++;return 1;});}catch(e){test.record(e===marker);}test.record(visited);test.record(caught);\n'ok';\n"),
    ('replaceall-literal-callback-positions-and-nonoverlap', "\nvar calls=[],receiver;\ntest.record('😀aba'.replaceAll('a',function(match,index,text){'use strict';calls.push(match+':'+index+':'+text+':'+arguments.length+':'+(this===undefined));return '$&';}));\ntest.record(calls.join('|'));test.record('aaaaa'.replaceAll('aa','x'));\ntest.record('ab'.replaceAll('',function(match,index,text){return index;}));\ntest.record(''.replaceAll('',()=>'-'));test.record('abc'.replaceAll('missing',function(){throw 'unexpected';}));\n'ok';\n"),
    ('replaceall-templates-conversion-and-generic-receiver', "\ntest.record('aba'.replaceAll('a','[$$][$&][$`][$\\']'));\nvar order='',receiver={toString(){order+='o';return 'aba';}},search={toString(){order+='s';return 'a';}},replacement={toString(){order+='r';return '<$&>';}};\ntest.record(String.prototype.replaceAll.call(receiver,search,replacement));test.record(order);\ntest.record(String.prototype.replaceAll.call(121,1,()=>9));\ntest.record('undefined undefined'.replaceAll(undefined,'x'));test.record('null null'.replaceAll(null,'x'));\n'ok';\n"),
    ('replaceall-global-regexp-snapshot-and-nonglobal-validation', "\nvar r=/(a)|(b)/g,seen=[];r.lastIndex=19;\ntest.record('ab'.replaceAll(r,function(match,a,b,index,input){'use strict';seen.push(match+':'+a+':'+b+':'+index+':'+arguments.length+':'+r.lastIndex);r.lastIndex=7;return index;}));\ntest.record(seen.join('|'));test.record(r.lastIndex);\nvar order='',receiver={toString(){order+='receiver';return 'aaa';}},replacement={toString(){order+='replacement';return 'x';}};\ntry{String.prototype.replaceAll.call(receiver,/a/,replacement);}catch(e){test.record(e.name);}test.record(order);\n'ok';\n"),
    ('replaceall-return-conversion-and-abrupt-order', "\nvar log='';test.record('aa'.replaceAll('a',function(match,index){log+='c'+index;return {toString(){log+='s'+index;return index;}};}));test.record(log);\nvar marker={},calls=0;try{'aaa'.replaceAll('a',function(){calls++;throw marker;});}catch(e){test.record(e===marker);}test.record(calls);\ntry{'a'.replaceAll('a',()=>Symbol());}catch(e){test.record(e.name);}\ntest.record('aa'.replaceAll('a',()=>undefined));test.record('aa'.replaceAll('a',()=>null));\n'ok';\n"),
    ('json-reviver-postorder-holder-delete-and-root', '\nvar order=[],root;\nvar value=JSON.parse(\'{"b":[1,2],"a":3}\',function(key,value,context){order.push(key+\':\'+arguments.length+\':\'+(Array.isArray(this)?\'array\':\'object\'));if(key===\'0\')return undefined;if(key===\'1\')return value+5;if(key===\'a\')return undefined;if(key===\'\'){root=this;return value;}return value;});\ntest.record(order.join(\'|\'));test.record(root[\'\']===value);test.record(JSON.stringify(value));test.record(value.b.length);test.record(Object.hasOwn(value.b,0));\ntest.record(JSON.parse(\'2\',function(k,v){return k===\'\'?v+4:v;}));test.record(JSON.parse(\'2\',function(){}));\n\'ok\';\n'),
    ('json-reviver-source-context-and-duplicate-members', '\nvar sources=[],contexts=[];\nvar result=JSON.parse(\'{"n":1.20e+2,"s":"a\\\\nb","z":-0,"n":5,"a":[true,null]}\',function(k,v,c){contexts.push(c);sources.push(k+\':\'+Object.hasOwn(c,\'source\')+\':\'+c.source);return v;});\ntest.record(sources.join(\'|\'));test.record(contexts[0]!==contexts[1]);test.record(Object.getPrototypeOf(contexts[0])===Object.prototype);\ntest.record(JSON.stringify(result));\n\'ok\';\n'),
    ('json-reviver-mutations-snapshots-and-ignored-definition-failure', '\nvar seen=[];\nvar result=JSON.parse(\'{"first":1,"later":2,"child":{"x":3}}\',function(k,v,c){\nif(k===\'first\'){this.later=9;this.child={x:3};this.added=4;Object.defineProperty(this,\'first\',{value:8,configurable:false});return undefined;}\nseen.push(k+\':\'+v+\':\'+Object.hasOwn(c,\'source\'));return v;});\ntest.record(JSON.stringify(result));test.record(seen.join(\'|\'));\nvar array=JSON.parse(\'[1,2]\',function(k,v){if(k===\'0\'){Object.freeze(this);return 8;}return v;});test.record(JSON.stringify(array));\n\'ok\';\n'),
    ('json-stringify-replacer-tojson-order-and-holder', "\nvar order=[],leaf={toJSON:function(key){order.push('j:'+key+':'+(this===leaf));return {v:2};}},root={a:leaf,b:3};\ntest.record(JSON.stringify(root,function(key,value){order.push('r:'+key+':'+(this===root));if(key==='v')return value+5;if(key==='b')return undefined;return value;}));\ntest.record(order.join('|'));test.record(JSON.stringify(undefined,function(key,value){return 4;}));\nvar f=function(){};f.toJSON=function(k){return k+'function';};test.record(JSON.stringify({f:f}));\n'ok';\n"),
    ('json-stringify-property-list-order-duplicates-and-inheritance', "\nvar parent={inherited:7},o=Object.create(parent);o.b=2;o.a=1;o[2]=9;Object.defineProperty(o,'hidden',{value:4});\nvar boxed=Object(2),calls=0;boxed.toString=function(){calls++;return '2';};\ntest.record(JSON.stringify(o,['b','a','b',boxed,'hidden','inherited',true,{},Symbol()]));test.record(calls);\ntest.record(JSON.stringify({a:{b:2,a:1},b:3},['b','a']));test.record(JSON.stringify([1,2],['1']));\n'ok';\n"),
    ('json-stringify-live-getters-omissions-and-cycles', "\nvar log='',o={};Object.defineProperty(o,'a',{enumerable:true,get:function(){log+='a';delete o.b;o.c=3;return 1;}});o.b=2;\ntest.record(JSON.stringify(o));test.record(log);\ntest.record(JSON.stringify({s:Symbol(),f:function(){},u:undefined,a:[Symbol(),function(){},undefined]}));\nvar a={};a.self=a;test.record(JSON.stringify(a,function(k,v){return k==='self'?'stopped':v;}));\nvar marker={},calls=0;try{JSON.stringify({a:1,b:2},function(k,v){calls++;if(k==='a')throw marker;return v;});}catch(e){test.record(e===marker);}test.record(calls);\n'ok';\n"),
    ('json-conversion-order-and-reentrant-callbacks', '\nvar order=\'\',jsonTextSource={toString(){order+=\'s\';return \'{"x":2}\';}};\ntest.record(JSON.parse(jsonTextSource,function(k,v,c){order+=k;return v;}));test.record(order);\norder=\'\';var list=[];Object.defineProperty(list,0,{get:function(){order+=\'l\';return \'a\';}});list.length=1;\nvar gap=Object(2);gap.valueOf=function(){order+=\'g\';return 2;};var value={toJSON:function(){order+=\'j\';return {a:1};}};\ntest.record(JSON.stringify(value,list,gap));test.record(order);\ntest.record(JSON.stringify({a:1},function(k,v){if(k===\'a\')return JSON.parse(JSON.stringify(v+1));return v;}));\n\'ok\';\n'),
])
NEGATIVE.extend([
    ('flat-infinite-cycle-depth-bound-atomic', "var a=[];a[0]=a;try{a.flat(Infinity);}catch(e){test.record('caught');}finally{test.record('finally');}"),
    ('flat-output-property-bound-atomic', "var a=Array(2050).fill(1);try{[a,a].flat();}catch(e){test.record('caught');}finally{test.record('finally');}"),
    ('replaceall-match-bound-atomic', "var text='a';for(var i=0;i<14;i++)text+=text;try{text.replaceAll('a',function(){test.record('callback');return '';});}catch(e){test.record('caught');}finally{test.record('finally');}"),
    ('json-reviver-created-cycle-bound-atomic', 'try{JSON.parse(\'{"a":1,"b":{}}\',function(k,v){if(k===\'a\')this.b=this;return v;});}catch(e){test.record(\'caught\');}finally{test.record(\'finally\');}'),
    ('json-tojson-recursion-bound-atomic', "var a={toJSON(){return JSON.stringify(this);}};try{JSON.stringify(a);}catch(e){test.record('caught');}finally{test.record('finally');}"),
])

# 0.39: non-Unicode literal closing delimiters required by nested template regexps.
COMPLETION_CASES.extend([
    ('regexp-legacy-closing-delimiters-literal', "test.record(/}/.test('}'));test.record(/]/.test(']'));test.record(/}]/.test('a}]b'));test.record(/}]/.test('}x]'));test.record(new RegExp('}').source);test.record(new RegExp(']').source);'ok';"),
    ('regexp-legacy-closing-delimiters-quantified-captures', "var r=/^(})]{2,3}$/;for(var text of ['}]]}','}]]]}','}]}','}]]]]}']){var m=r.exec(text);test.record(m?m[0]:null);if(m)test.record(m[1]);}test.record('a}b]]c'.replace(/[}]]*/g,'x'));'ok';"),
    ('regexp-legacy-closing-delimiters-preserve-errors', "for(var pattern of ['*}','+]','?]','}**','}a{2,1}','}(']){try{new RegExp(pattern);test.record('accepted');}catch(e){test.record(e.name);}}test.record(/^}+$/.test('}}'));test.record(/^]+$/.test(']]'));'ok';"),
])

# 0.39: preserve old join sources as positive conversions; original bounded cycle checks remain.
COMPLETION_CASES.extend([
    ('unsupported-custom-join-coercion', 'let a=[{toString(){return "custom";}}];a.join();'),
    ('prototype-inherited-join-conversion-disabled-atomic', 'let p={toString(){return "custom";}},o=Object.create(p);try{[o].join();}catch(e){test.record(1);}finally{test.record(2);}'),
    ('join-ordinary-method-order-and-fallback', "var order='',first={get toString(){order+='g';return function(){order+='s';return {};};},get valueOf(){order+='v';return function(){order+='V';return 12;};}};\ntest.record([first].join('|'));test.record(order);\nvar parent={toString:function(){return this.name;}},child=Object.create(parent);child.name='inherited';test.record([child].join());\n'ok';\n"),
    ('join-custom-function-and-bound-function-elements', "function f(){};f.toString=function(){return this===f?'function custom':'bad';};\nvar b=f.bind(null);b[Symbol.toPrimitive]=function(h){return h+':bound';};\ntest.record([f,b].join('|'));\n'ok';\n"),
    ('join-ignores-unneeded-valueof-and-custom-nested-join', "var reads=0,plain={};Object.defineProperty(plain,'valueOf',{get:function(){reads++;throw 'unexpected';}});\ntest.record([plain].join());test.record(reads);\nvar nested=[1,2];nested.join=function(){return this.length+':custom';};test.record([nested].join());\n'ok';\n"),
    ('join-length-captured-before-separator-coercion', "var a=[1,2,3],separator={toString:function(){a.length=0;return '|';}};test.record(a.join(separator));\nvar b=[1],separator2={toString:function(){b.push(2,3);return ':';}};test.record(b.join(separator2));\n'ok';\n"),
    ('join-live-index-conversion-mutation-and-abrupt-identity', "var a=[{toString:function(){delete a[1];a[2]=9;a.push(10);return 'first';}},2,3];test.record(a.join('|'));test.record(a.length);\nvar marker={},later=0;try{[{toString:function(){throw marker;}},{toString:function(){later++;return 'wrong';}}].join();}catch(e){test.record(e===marker);}test.record(later);\n'ok';\n"),
    ('join-cycle-through-ordinary-conversion-and-exotic-hook', "var a=[];a[0]={toString:function(){return a.join(':');}};a[1]=2;test.record(a.join('|'));\nvar o=[];o[0]=o;o[Symbol.toPrimitive]=Array.prototype.join;test.record([o].join());\n'ok';\n"),
    ('join-catchable-conversion-errors-and-cleanup', "var a=[{toString:function(){return {};},valueOf:function(){return {};}}],caught=false;try{a.join();}catch(e){caught=e.name==='TypeError';}test.record(caught);\na[0]=3;test.record(a.join());var s={toString:function(){return Symbol();}};try{[s].join();}catch(e){test.record(e.name);}\n'ok';\n"),
    ('join-existing-native-cycles-and-depth-control', "var a=[1];a[1]=a;a[2]=3;test.record(a.join('|'));\nvar b=[],c=[b];b[0]=c;test.record([b,b].join('|'));\nvar deep=['end'];for(var i=0;i<24;i++)deep=[deep];test.record(deep.join());\n'ok';\n"),
])
NEGATIVE.extend([
    ('join-callback-work-bound-atomic', "var a=[{toString:function(){while(true){}}}];try{a.join();}catch(e){test.record('caught');}finally{test.record('finally');}"),
    ('join-conversion-recursion-bound-atomic', "var o={toString:function(){return [o].join();}};try{[o].join();}catch(e){test.record('caught');}finally{test.record('finally');}"),
])

# 0.39: enabled Function and optional syntax; preserve old program IDs and sources.
COMPLETION_CASES.extend([
    ('eval-unsupported-optional-chain-cannot-catch-atomic', 'try{eval("let o={};o?.x");}catch(e){test.record(1);}finally{test.record(2);}'),
    ('prototype-dynamic-function-disabled-atomic', 'try{Function("test.record(1)")();}catch(e){test.record(2);}finally{test.record(3);}'),
    ('prototype-dynamic-function-indirect-disabled-atomic', 'try{({}).constructor.constructor("test.record(1)")();}catch(e){test.record(2);}'),
])

# 0.39: original template and optional-chain semantic programs.
COMPLETION_CASES.extend([
    ('template-basic-values-empty-and-comma', "var x=3;test.record(`value ${x+1}`);test.record(``);test.record(`plain`);\ntest.record(`${undefined}|${null}|${true}|${-0}|${3.5}`);\nvar calls=0;test.record(`x${calls++,calls+4}y${calls++}z`);test.record(calls);\n'ok';\n"),
    ('template-conversion-hint-receiver-and-left-order', "var log='',item={};Object.defineProperty(item,Symbol.toPrimitive,{get:function(){log+='g';return function(h){log+=h;if(this!==item)throw 'receiver';return 'value';};}});\nfunction next(){log+='n';return 5;}test.record(`head${item}mid${next()}tail`);test.record(log);\nvar fallback={toString:function(){log+='s';return 'text';},valueOf:function(){throw 'number hint';}};\ntest.record(`${fallback}`);test.record(log);\n'ok';\n"),
    ('template-abrupt-conversion-stops-later-substitutions', "var marker={},after=0,bad={toString:function(){throw marker;}};\ntry{`${bad}${after++}`;}catch(e){test.record(e===marker);}test.record(after);\ntry{`${Symbol('x')}${after++}`;}catch(e){test.record(e.name);}test.record(after);\nvar object={};object[Symbol.toPrimitive]=function(){return {};};\ntry{`${object}`;}catch(e){test.record(e.name);}\n'ok';\n"),
    ('template-nested-objects-functions-and-closures', "var suffix='!';function make(x){return function(y){return `outer:${`${x+y}:${({n:3}).n}`}${suffix}`;};}\ntest.record(make(2)(4));test.record(`${function(){return {text:'}'};}().text}`);\ntest.record(`${(()=>({x:8}))().x}`);test.record(`${({get text(){return 'getter';}}).text}`);\n'ok';\n"),
    ('template-regexp-division-and-comment-delimiters', "test.record(`${/}/.test('}')}|${/[{}]/.test('{')}|${/\\$\\{/.test('${')}`);\ntest.record(`${({n:8}).n / 2}::${(function(){return 9;})()/3}`);\ntest.record(`${1/* } ` ${ */+2}`);test.record(`${// } ` ${\n4}`);test.record(`${`6` / 2}`);test.record(`12` / 3);\n'ok';\n"),
    ('template-methods-indices-and-construction', "test.record(`hello ${2}`.toUpperCase());test.record(`abcd`[2]);\nvar object={[`key${1}`]:7};test.record(object.key1);\nfunction C(v){this.value=v;}test.record(new C(`arg${9}`).value);\nfunction use(v){return v;}test.record(use(`a${1}`,`ignored`));\n'ok';\n"),
    ('template-cooked-escapes-and-codepoints', "test.record(`a\\n\\r\\t\\b\\f\\v\\0z`);test.record(`\\x41\\u0042\\u{43}\\u{1F600}`);\ntest.record(`\\uD83D\\uDE00`);test.record(`\\u{D83D}\\u{DE00}`);\ntest.record(`back\\`tick \\\\ \\${literal} \\q`);test.record(`a\\\nb`);\n'ok';\n"),
    ('template-raw-line-terminator-cooking', "test.record(`a\rb\r\nc\nd\u2028e\u2029f`);test.record(`a\\\r\nb\\\rc`);\n'ok';\n"),
    ('template-in-grammar-and-control-flow', "var target={x:1},seen='';for(var key=`${'x' in target}`;key==='true';key='done'){seen=key;}\ntest.record(seen);test.record(`${'x' in target}`);test.record(`${true?{v:'yes'}:{v:'no'}}`);\nswitch(`a${1}`){case `a${1}`:test.record('matched');break;default:throw 'switch';}\n'ok';\n"),
    ('template-dynamic-eval-and-function-bodies', "var x=7;test.record(eval('`value ${x+1}`'));test.record(Function('x','return `dynamic ${x*2}`;')(6));\nfunction closure(x){return ()=>`closed ${x}`;}test.record(closure(3)());\n'ok';\n"),
    ('template-malformed-syntax-is-atomic', "var ran=false,caught=0;for(var text of ['`unfinished','`${}`','`${1','`${(1}`','`${1 2}`','`\\\\1`','`\\\\8`','`\\\\01`','`\\\\x0G`','`\\\\u{110000}`','`\\\\u{}`']){\ntry{eval('ran=true;'+text);}catch(e){if(e.name==='SyntaxError')caught++;}if(ran)throw 'partial execution';}\ntest.record(caught);\n'ok';\n"),
    ('template-eval-created-function-expression-bodies', "var x=7;test.record(eval('`value ${x+1}`'));test.record(eval('(function(x){return `dynamic ${x*2}`;})')(6));function closure(x){return ()=>`closed ${x}`;}test.record(closure(3)());'ok';"),
    ('optional-chains-skip-only-nullish-contiguous-parts-and-effects', "\nvar count=0,a=null,b;\nif(a?.[count++]!==undefined||a?.method(count++)!==undefined||b?.(count++)!==undefined||count!==0)throw 'skipped effects';\nif(a?.x.y.z!==undefined||a?.()[count++]!==undefined||count!==0)throw 'whole chain';\nif(({a:{b:21}})?.a.b!==21)throw 'present';\nif((0)?.toString()!=='0'||false?.valueOf()!==false||''?.length!==0)throw 'falsy receivers';\nvar errors=0;try{({a:null})?.a.b;}catch(e){if(e.name==='TypeError')errors++;}\ntry{(a?.x).y;}catch(e){if(e.name==='TypeError')errors++;}\ntry{(a?.x)(count++);}catch(e){if(e.name==='TypeError')errors++;}\nif(errors!==3||count!==1)throw 'grouping boundary';'ok';\n"),
    ('optional-chains-preserve-references-receivers-and-lookup-order', "\nvar log='',o={value:4,get method(){log+='g';return function(x){'use strict';log+=this===o?'r':'x';return this===undefined?'none':this.value+x;};}};\nfunction arg(){log+='a';return 2;}\nif(o?.method(arg())!==6||log!=='gar')throw 'member order';log='';\nif(o.method?.(arg())!==6||log!=='gar')throw 'optional call order';log='';\nif((o?.method)(arg())!==6||log!=='gar')throw 'grouped reference';log='';\nif((o.method)?.(arg())!==6||log!=='gar')throw 'ordinary grouped reference';log='';\nif((0,o?.method)()!=='none'||log!=='gx')throw 'sequence strips reference';\nvar key={toString(){log+='k';return 'method';}};log='';\nif(o?.[key]?.(arg())!==6||log!=='kgar')throw 'key conversion';\nvar s=Symbol('m');o[s]=function(){return this===o;};if(!o?.[s]?.())throw 'symbol receiver';'ok';\n"),
    ('optional-calls-get-callee-before-arguments-and-check-callability-after-them', "\nvar log='',a={get method(){log+='g';return undefined;}};\nfunction arg(){log+='a';return 1;}\nif(a.method?.(arg())!==undefined||log!=='g')throw 'nullish call';\nvar errors=0;try{({method:0}).method?.(arg());}catch(e){if(e.name==='TypeError')errors++;}\nif(errors!==1||log!=='ga')throw 'callability order';\nvar marker={},after=0,o={get method(){throw marker;}};\ntry{o.method?.(after++);}catch(e){if(e===marker)errors++;}\nif(after!==0||errors!==2)throw 'getter exception';\nvar live={n:3,get m(){return function(v){return this.n+v;};}};\nif(live?.m((live.m=0,4))!==7)throw 'resolved callee';\nfunction args(a,b){return a+b;}if(args?.(...[2,3])!==5)throw 'spread';'ok';\n"),
    ('optional-delete-uses-final-reference-without-invoking-getters', "\nvar reads=0,keys=0,o={get x(){reads++;return 1;}},p=Object.create({inherited:2});\nif(!delete null?.[keys++]||keys!==0)throw 'null delete';\nif(!delete o?.x||reads!==0||'x' in o)throw 'getter-free delete';\nObject.defineProperty(o,'y',{get:function(){reads++;return 2;},configurable:true});\nif(!delete (o?.y)||reads!==0||'y' in o)throw 'grouped delete';\nif(!delete p?.inherited||p.inherited!==2)throw 'inherited delete';\nObject.defineProperty(o,'fixed',{value:3,configurable:false});if(delete o?.fixed)throw 'nonconfigurable';\nvar caught=false;function strict(){'use strict';delete o?.fixed;}try{strict();}catch(e){caught=e.name==='TypeError';}\nif(!caught)throw 'strict delete';\nvar key={toString(){keys++;return 'z';}};o.z=4;if(!delete o?.[key]||keys!==1||'z' in o)throw 'delete key';'ok';\n"),
    ('optional-chains-leave-unresolvable-names-and-indirect-eval-observable', "\nvar errors=0;try{missingOptionalName?.x;}catch(e){if(e.name==='ReferenceError')errors++;}\ntry{missingOptionalName?.();}catch(e){if(e.name==='ReferenceError')errors++;}\ntry{typeof missingOptionalName?.x;}catch(e){if(e.name==='ReferenceError')errors++;}\nvar marker=34;function read(){var marker=35;return eval?.('marker');}\nif(read()!==34||marker!==34||errors!==3)throw 'name or eval';\nvar o={eval:eval};function other(){var marker=36;return o?.eval('marker');}\nif(other()!==34)throw 'member indirect eval';'ok';\n"),
    ('nullish-operators-preserve-falsy-values-order-and-parenthesized-precedence', "\nif((null??19)!==19||(undefined??20)!==20||(false??1)!==false||(0??1)!==0||(''??1)!==''||!Number.isNaN(NaN??1))throw 'nullish values';\nvar log='';function read(k,v){log+=k;return v;}\nif((read('a',0)??read('b',2))!==0||(read('c',null)??read('d',undefined)??read('e',27))!==27||log!=='acde')throw 'order';\nif(((false||null)??28)!==28||((1&&0)??29)!==0||(null??(false||30))!==30||(0??true?31:32)!==32)throw 'precedence';\nif(((null??3)||4)!==3||(false&&(null??4))!==false)throw 'other grouping';\nvar obj={value:0};if((obj?.value??7)!==0||(null?.value??7)!==7)throw 'chain combination';'ok';\n"),
    ('optional-lexical-goals-keep-decimal-conditionals-reserved-names-and-regexps', "\nif((true?.3:0)!==0.3||(false?.3:0)!==0)throw 'decimal lookahead';\nvar o={return:6,function:8,if:function(v){return v;}};\nif(o?.return / 2!==3||o?.function / 2!==4||!o?.if(/}/).test('}'))throw 'lexical goals';\nif(`${o?.return??0}`!=='6'||`${null?.return??0}`!=='0')throw 'template combination';\nfunction C(v){this.value=v;}var holder={C:C};\nif(new C(4)?.value!==4||new (holder?.C)(5).value!==5)throw 'valid construction';'ok';\n"),
    ('optional-and-nullish-early-errors-are-atomic-and-catchable', "\nvar ran=false,count=0;for(var text of ['a??b||c','a||b??c','a??b&&c','a&&b??c','a?.x=1','a?.x++','++a?.x','new a?.()','a?.`x`','a?.x`x`']){\ntry{eval('ran=true;'+text);}catch(e){if(e.name==='SyntaxError')count++;}if(ran)throw 'partial execution';}\nif(count!==10)throw 'early errors';'ok';\n"),
])
BROWSER_CASES.extend([
    ('template-no-substitution-is-not-strict-directive', "function f(){`use strict`;return this===undefined;}test.record(f());\nfunction g(...args){`use strict`;return args.length;}test.record(g(1,2));\nfunction h(){'other';`use strict`;return this===undefined;}test.record(h());\nfunction actual(){'use strict';return `${this}`;}test.record(actual());\n'ok';\n"),
])
NEGATIVE.extend([
    ('template-tagged-boundary-atomic', "test.record('before');function tag(){};tag`text`;"),
    ('template-tagged-newline-boundary-atomic', "test.record('before');function tag(){};tag\n`text`;"),
    ('template-lone-surrogate-boundary-atomic', "test.record('before');`\\uD800`;"),
    ('template-nesting-boundary-atomic', "test.record('before');`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${1}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`"),
    ('template-substitution-boundary-atomic', "test.record('before');`${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}${1}`;"),
    ('template-result-bound-by-doubling-fatal', "var x='x';for(var i=0;i<17;i++)x+=x;try{`${x}a`;}catch(e){test.record('caught');}finally{test.record('finally');}"),
    ('optional-chain-step-bound-atomic', "test.record('must not execute');null?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x?.x"),
    ('optional-call-budget-fatal-atomic', "var o={f(){while(true){}}};try{o?.f();}catch(e){test.record('caught');}finally{test.record('finally');}"),
])
# Preserve the original output-bound fixture ID while asserting fatal atomicity.
ATOMIC_NEGATIVE_CASES = {"template-result-bound-by-doubling-fatal"}

# 0.40: exact prior programs become positive after their supported capability lands.
COMPLETION_CASES.extend([
    ('arrow-default-unsupported-atomic', 'test.record("must not run");let f=(a=1)=>a;'),
    ('regex-named-group-unsupported-atomic', 'test.record(1);let r=/(?<named>a)/;'),
    ('regex-backreference-unsupported-atomic', r'test.record(1);let r=/(a)\1/;'),
])

COMPLETION_CASES.extend([
    ('named-groups-shape-order-optional', "var m=/(?<word>a)(?<optional>b)?(c)/.exec('ac');test.record(m.join('|'));test.record(m.index);test.record(m.input);test.record(Object.getPrototypeOf(m.groups)===null);test.record(Object.keys(m.groups).join(','));test.record(m.groups.word);test.record(m.groups.optional);var d=Object.getOwnPropertyDescriptor(m.groups,'word');test.record(d.writable&&d.enumerable&&d.configurable);'ok';"),
    ('named-groups-duplicates-disjoint-alternatives', "var r=/^(?:(?<x>a)|(?<x>b))\\k<x>$/;test.record(r.exec('aa').groups.x);test.record(r.exec('bb').groups.x);test.record(r.test('ab'));test.record(/^(?:(?<x>a)|(?<x>b))*$/.exec('ab').groups.x);'ok';"),
    ('named-groups-reference-forward-self-undefined', "test.record(/^\\k<x>(?<x>a)$/.test('a'));test.record(/^(?<x>\\k<x>a)$/.test('a'));test.record(/^(?<x>a)?\\k<x>b$/.test('b'));test.record(/^(?<x>ab)\\k<x>$/.test('abab'));'ok';"),
    ('named-groups-lookbehind-lookahead', "var a=/(?<=\\k<x>(?<x>ab))c/.exec('ababc');test.record(a[0]);test.record(a.index);test.record(a.groups.x);var b=/(?=(?<x>a+))\\k<x>/.exec('aaa');test.record(b[0]);test.record(b.groups.x);'ok';"),
    ('named-groups-numbered-reference-interoperability', "test.record(/^(?<x>a)(b)\\2\\1$/.test('abba'));test.record(/^\\1(a)$/.test('a'));test.record(/^(a)?\\1b$/.test('b'));test.record(/^(a|ab)\\1$/.exec('abab')[1]);'ok';"),
    ('named-groups-ascii-ignorecase', "var r=/^(?<x>aB)\\k<x>$/i;test.record(r.test('Abab'));test.record(r.test('aBaB'));test.record(r.test('aBac'));'ok';"),
    ('named-groups-replacement-templates', "test.record('ab a'.replace(/(?<x>a)(?<y>b)?/g,'[$<x>:$<y>:$<missing>:$1:$2:$$]'));test.record('a'.replace(/a/,'$<x>'));test.record('a'.replace(/(?<x>a)/,'$<x'));'ok';"),
    ('named-groups-replacement-callback-snapshots', "var r=/(?<x>a)|(?<y>b)/g,seen=[],last;test.record('ab'.replaceAll(r,function(m,a,b,index,input,groups){'use strict';test.record(this===undefined);test.record(arguments.length);test.record(Object.getPrototypeOf(groups)===null);test.record(groups!==last);last=groups;seen.push(index+':'+r.lastIndex+':'+Object.keys(groups).join(','));r.lastIndex=7;return groups.x||groups.y;}));test.record(seen.join('|'));'ok';"),
    ('named-groups-key-names-and-escapes', "var m=/(?<\\u0061>a)(?<$b>b)(?<_c>c)/.exec('abc');test.record(Object.keys(m.groups).join(','));test.record(m.groups.a+m.groups.$b+m.groups._c);test.record(RegExp('(?<a>a)\\\\k<\\\\u0061>').test('aa'));'ok';"),
    ('named-groups-null-prototype-special-properties', "var m=/(?<constructor>a)(?<toString>b)/.exec('ab');test.record(Object.getPrototypeOf(m.groups)===null);test.record(m.groups.constructor);test.record(m.groups.toString);test.record(Object.keys(m.groups).join(','));'ok';"),
    ('named-groups-global-sticky-search-split', "var r=/(?<x>a)/gy;r.lastIndex=1;var m=r.exec('ba');test.record(m.groups.x);test.record(r.lastIndex);test.record(r.exec('ba'));test.record(r.lastIndex);test.record('baa'.match(/(?<x>a)/g).join(','));test.record('ba'.match(/(?<x>a)/).groups.x);test.record('ba'.search(/(?<x>a)/));test.record('bac'.split(/(?<x>a)/).join(','));'ok';"),
    ('named-groups-legacy-k-and-classes', "test.record(/\\k<x>/.test('k<x>'));test.record(/[\\k]/.test('k'));test.record(/\\\\k(?<x>a)/.test('\\\\ka'));for(var p of ['(?<x>a)\\\\k<missing>','(?<x>a)\\\\k','(?<x>a)[\\\\k]']){try{RegExp(p);test.record('bad');}catch(e){test.record(e.name);}}'ok';"),
    ('named-groups-invalid-duplicates-and-names', "for(var p of ['(?<x>a)(?<x>b)','(?<x>(?<x>a))','(?<x>a)|(?<x>b)(?<x>c)','(?<>a)','(?<1x>a)','(?<a-b>a)','(?<x>a)\\\\k<no>']){try{RegExp(p);test.record('bad');}catch(e){test.record(e.name);}}'ok';"),
    ('named-groups-case-sensitive-unicode-content', "var m=/^(?<x>猫é)\\k<x>$/.exec('猫é猫é');test.record(m[0]);test.record(m.groups.x);test.record('猫é'.replace(/(?<x>猫)(?<y>é)/,'$<y>$<x>'));'ok';"),
    ('named-groups-negative-assertion-capture-clearing', "var m=/(?!(?<x>a))(?<y>b)/.exec('b');test.record(m.groups.x);test.record(m.groups.y);test.record(/^(?:(?<x>a)|(?<x>b))+\\k<x>$/.test('abb'));test.record(/^(?:(?<x>a)|(?<x>b))+\\k<x>$/.test('aba'));'ok';"),
])

NEGATIVE.extend([
    ('named-nonascii-name-unsupported-atomic', "test.record('bad');/(?<猫>a)/;"),
    ('named-protected-name-unsupported-atomic', "test.record('bad');/(?<__proto__>a)/;"),
    ('named-long-name-bound-atomic', "test.record('bad');/(?<aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa>x)/;"),
    ('named-capture-count-bound-atomic', "test.record('bad');/(?<g0>a)(?<g1>a)(?<g2>a)(?<g3>a)(?<g4>a)(?<g5>a)(?<g6>a)(?<g7>a)(?<g8>a)(?<g9>a)(?<g10>a)(?<g11>a)(?<g12>a)(?<g13>a)(?<g14>a)(?<g15>a)(?<g16>a)/;"),
    ('named-nonascii-ignorecase-reference-unsupported-atomic', "try{/(?<x>.)\\k<x>/i.test('éé');}catch(e){test.record('caught');}finally{test.record('finally');}"),
    ('named-backreference-work-bound-atomic', "var r=/(?<x>a+)\\k<x>/;try{while(true)r.test('aaaaaaaaaaaaaaaa');}catch(e){test.record('caught');}finally{test.record('finally');}"),
])

# 0.40: original frozen grammar and UTC Date programs, with complete result checks.
COMPLETION_CASES.extend([
    ('catch-destructuring-unsupported-atomic', 'test.record(1);try{}catch({message}){}'),
    ('defaults-order-undefined-only', "var order='';function f(a=(order+='a',2),b=(order+='b',a+3),c){return [a,b,c].join(':');}test.record(f());test.record(order);test.record(f(null,0,false));test.record(order);test.record(f.length);'ok';"),
    ('defaults-later-and-self-tdz', "var out='';function a(x=x){return x;}function b(x=y,y=2){return x;}try{a();}catch(e){out+=e.name;}try{b();}catch(e){out+=':'+e.name;}test.record(out);'ok';"),
    ('defaults-separate-body-scope', "var x=8;function f(a=x,read=()=>a){var x=99;var a=4;return [a,read(),x];}test.record(f().join());'ok';"),
    ('defaults-unmapped-arguments', "function f(a=3,b=arguments.length){a=7;arguments[0]=9;return [a,b,arguments.length,arguments[0]];}test.record(f(1).join());test.record(f().join());'ok';"),
    ('defaults-arguments-before-body-shadow', "function f(a=arguments[0]){let arguments=5;return [a,arguments];}test.record(f(3).join());'ok';"),
    ('defaults-arrow-method-constructor', "function Box(x=4){this.x=x;}var obj={v:6,m(x=this.v){return x;}};var f=(a=2,...r)=>a+r.length;test.record(new Box().x);test.record(obj.m());test.record(f(undefined,7,8));test.record(f.length);'ok';"),
    ('defaults-eval-parameter-environment', "function f(a=eval('var hidden=4; hidden'),b=()=>hidden){var hidden=9;return [a,b(),hidden];}test.record(f().join());var result='';function g(a=eval('var a=4')){}try{g();}catch(e){result=e.name;}test.record(result);'ok';"),
    ('defaults-function-initializer-name', "function f(a=function(){},b=()=>1,c=function named(){}){return [a.name,b.name,c.name];}test.record(f().join());'ok';"),
    ('defaults-dynamic-function', "var f=Function('a=3','b=a+2','return a*b;');test.record(f());test.record(f(4));test.record(f.length);'ok';"),
    ('defaults-reentrant-throw', "var n=0;function f(a=(n++,function(){throw 'bad';})()){n+=10;}try{f();}catch(e){test.record(e);}test.record(n);'ok';"),
    ('patterns-nested-bindings', "let [a,,b=4,...tail]=[1,2,undefined,5,6];let {x:y,z:{n=7}={},...rest}={x:3,k:9};test.record([a,b,tail.join(),y,n,rest.k].join('|'));'ok';"),
    ('patterns-assignment-value', "var a,b,o={};var input=[2,3];var result=([a,o.x]=input);({y:b=4}={});test.record(result===input);test.record([a,b,o.x].join());'ok';"),
    ('patterns-parameter-rest', "function f({x=3}={},[a,...b]=[5,6],...rest){return [x,a,b.join(),rest.length];}test.record(f().join('|'));test.record(f.length);'ok';"),
    ('patterns-iterator-close', "var log='',src={};src[Symbol.iterator]=function(){var i=0;return {next(){log+='n';return {value:++i,done:false};},return(){log+='r';return {};}};};let [first]=src;test.record(first);test.record(log);'ok';"),
    ('patterns-loop-catch', "var f=[];for(let [a,b] of [[1,2],[3,4]])f.push(()=>a+b);try{throw {x:6};}catch({x}){test.record(x);}test.record(f[0]());test.record(f[1]());'ok';"),
    ('harness-recorder-returns-undefined', "test.record(test.record('record'));test.record(test.callback(function(){}));'ok';"),
    ('date-timestamp-construction', "for(var v of [0,1,-1,1.9,-1.9,null,true,false,undefined,NaN,Infinity,-Infinity])test.record(new Date(v).getTime());\ntest.record(+new Date(1234));test.record(new Date(0) instanceof Date);test.record(new Date(0) instanceof Object);\n'ok';\n"),
    ('date-timeclip-and-iso-extremes', "for(var v of [-8640000000000000,8640000000000000]){var d=new Date(v);test.record(d.getTime());test.record(d.toISOString());}\nfor(var v of [-8640000000000001,8640000000000001,1e30])test.record(isNaN(new Date(v).getTime()));\ntest.record(1/new Date(-0).getTime());test.record(1/new Date(-0.5).getTime());\n'ok';\n"),
    ('date-iso-default-fields-and-offsets', "for(var s of ['2024','2024-02','2024-02-29','2024T12:34Z','2024-02T12:34Z','2024-02-29T12:34:56.789Z','2024-02-29T12:34+05:30','2024-02-29T12:34:56.789-03:45','2024-02-29T24:00:00.000Z']){\ntest.record(Date.parse(s));test.record(new Date(s).toISOString());}\n'ok';\n"),
    ('date-iso-invalid-fields', "for(var s of ['garbage','2024-00-01','2024-13-01','2024-01-00','2024-01-32','2024-01-01T25:00Z','2024-01-01T24:01Z','2024-01-01T12:60Z','2024-01-01T12:00:60Z','2024-01-01T12:00+24:00','2024-01-01T12:00+00:60','2024-01-01T12:00Zx'])test.record(isNaN(Date.parse(s)));\n'ok';\n"),
    ('date-expanded-years-and-leap-centuries', "for(var s of ['0000-01-01T00:00:00.000Z','0001-01-01T00:00:00.000Z','-000001-12-31T23:59:59.999Z','+010000-01-01T00:00:00.000Z','1900-03-01T00:00:00.000Z','2000-02-29T00:00:00.000Z']){\nvar d=new Date(s);test.record(d.toISOString());test.record(d.getUTCFullYear());test.record(d.getUTCDay());}\n'ok';\n"),
    ('date-utc-defaults-and-normalization', "for(var a of [[],[1970],[1970,0],[0,0,1],[99,0,1],[100,0,1],[-1,0,1],[1970,-1,1],[1969,12,1],[1970,0,0],[2000,1,30],[1970,0,1,24,-60,60,-1],[1970,undefined]]){\nvar n=Date.UTC.apply(null,a);test.record(n);if(!isNaN(n))test.record(new Date(n).toISOString());}\n'ok';\n"),
    ('date-utc-component-getters', "for(var v of [-1,0,951827696789]){var d=new Date(v);test.record([d.getUTCFullYear(),d.getUTCMonth(),d.getUTCDate(),d.getUTCDay(),d.getUTCHours(),d.getUTCMinutes(),d.getUTCSeconds(),d.getUTCMilliseconds()].join(','));}\nvar invalid=new Date(NaN);test.record(isNaN(invalid.getUTCFullYear()));test.record(isNaN(invalid.getUTCDay()));\n'ok';\n"),
    ('date-copy-preserves-internal-slot', "var d=new Date(9876),log='';d.valueOf=function(){throw 'valueOf';};d.toString=function(){throw 'toString';};\nObject.defineProperty(d,Symbol.toPrimitive,{get:function(){throw 'primitive';}});\nvar copy=new Date(d);test.record(copy.getTime());test.record(copy!==d);Object.setPrototypeOf(d,null);test.record(new Date(d).getTime());\n'ok';\n"),
    ('date-constructor-conversion-default-hint', "var log='',a={};a[Symbol.toPrimitive]=function(h){log+=h;return '1970-01-02T00:00Z';};test.record(new Date(a).getTime());test.record(log);\nvar marker={};try{new Date({valueOf:function(){throw marker;}});}catch(e){test.record(e===marker);}\ntry{new Date(Symbol());}catch(e){test.record(e.name);}\n'ok';\n"),
    ('date-parse-string-conversion', "var log='',o={toString:function(){log+='s';return '1970-01-02';},valueOf:function(){log+='v';return 0;}};\ntest.record(Date.parse(o));test.record(log);test.record(isNaN(Date.parse()));\nvar marker={};try{Date.parse({toString:function(){throw marker;}});}catch(e){test.record(e===marker);}\ntry{Date.parse(Symbol());}catch(e){test.record(e.name);}\n'ok';\n"),
    ('date-settime-and-frozen-slot', "var d=new Date(0);Object.freeze(d);test.record(d.setTime({valueOf:function(){return 1234.9;}}));test.record(d.getTime());\ntest.record(d.setTime(-1.9));test.record(d.toISOString());test.record(isNaN(d.setTime()));test.record(d.toJSON());\n'ok';\n"),
    ('date-utc-setter-normalization', "var d=new Date('2000-02-29T12:34:56.789Z');\nfor(var a of [['setUTCDate',0],['setUTCMonth',14,3],['setUTCFullYear',2024,1,29],['setUTCHours',25,-2,70,1001],['setUTCMinutes',-1,61,1002],['setUTCSeconds',-1,1003],['setUTCMilliseconds',-1]]){\nvar fn=a.shift();test.record(d[fn].apply(d,a));test.record(d.toISOString());}\n'ok';\n"),
    ('date-utc-full-year-revives-invalid', "var d=new Date(NaN);test.record(d.setUTCFullYear(2000));test.record(d.toISOString());\nd.setTime(NaN);test.record(d.setUTCFullYear(0,1,29));test.record(d.toISOString());\ntest.record(isNaN(d.setUTCFullYear(undefined)));test.record(d.toJSON());\n'ok';\n"),
    ('date-brand-check-before-conversion', "var log='',v={valueOf:function(){log+='v';return 0;}};\nfor(var name of ['getTime','getUTCFullYear','toISOString','toUTCString','setTime','setUTCMonth']){try{Date.prototype[name].call({},v);}catch(e){test.record(e.name);}}\ntest.record(log);try{Date.prototype.valueOf();}catch(e){test.record(e.name);}test.record(Object.prototype.toString.call(Date.prototype));\n'ok';\n"),
    ('date-json-generic-and-invalid', "test.record(JSON.stringify({good:new Date(0),bad:new Date(NaN)}));\nvar o={valueOf:function(){return 1;},toISOString:function(){return 'custom';}};test.record(Date.prototype.toJSON.call(o));\nvar a={valueOf:function(){return Infinity;},get toISOString(){throw 'must not read';}};test.record(Date.prototype.toJSON.call(a));\ntry{Date.prototype.toJSON.call({valueOf:function(){return 1;},toISOString:2});}catch(e){test.record(e.name);}\n'ok';\n"),
    ('date-json-live-getter-and-conversion-order', "var log='',o={};o[Symbol.toPrimitive]=function(h){log+=h;return 'text';};Object.defineProperty(o,'toISOString',{get:function(){log+='g';return function(){log+='c';test.record(this===o);return 42;};}});\ntest.record(Date.prototype.toJSON.call(o));test.record(log);\nvar d=new Date(0);d.toISOString=function(){return 'overridden';};test.record(JSON.stringify([d]));\n'ok';\n"),
    ('date-primitive-hook-generic-hints', "var fn=Date.prototype[Symbol.toPrimitive],log='',o={toString:function(){log+='s';return 'S';},valueOf:function(){log+='v';return 2;}};\ntest.record(fn.call(o,'default'));test.record(fn.call(o,'string'));test.record(fn.call(o,'number'));test.record(log);\nfor(var hint of [undefined,null,1,{},'wrong']){try{fn.call(o,hint);}catch(e){test.record(e.name);}}\ntry{fn.call(3,'number');}catch(e){test.record(e.name);}\n'ok';\n"),
    ('date-prototype-metadata-and-brand', "var d=new Date(0);test.record(Object.getPrototypeOf(d)===Date.prototype);test.record(Object.prototype.toString.call(d));test.record(Object.keys(d).length);test.record(Date.length);test.record(Date.now.length);test.record(Date.UTC.length);test.record(Date.parse.length);\nfor(var n of ['setUTCFullYear','setUTCMonth','setUTCHours','setUTCMinutes','setUTCSeconds','setUTCMilliseconds','toJSON'])test.record(Date.prototype[n].length);\nvar p=Object.getOwnPropertyDescriptor(Date.prototype,Symbol.toPrimitive);test.record(p.writable+','+p.enumerable+','+p.configurable);test.record(p.value.name);\nvar C=Date.bind(null,42),v=new C();test.record(v.getTime());test.record(v instanceof C);\n'ok';\n"),
    ('date-utc-string-roundtrip', "for(var s of ['1900-01-01T00:00:00Z','1970-01-01T00:00:00Z','2000-02-29T12:34:56Z','2400-12-31T23:59:59Z']){var d=new Date(s),text=d.toUTCString();test.record(text);test.record(Date.parse(text)===d.getTime());}\ntest.record(new Date(NaN).toUTCString());test.record(new Date(NaN).toString());test.record(Date.prototype.toGMTString===Date.prototype.toUTCString);\n'ok';\n"),
    ('date-utc-conversion-order-even-invalid', "var log='';function n(mark,value){return {valueOf:function(){log+=mark;return value;}};}\ntest.record(isNaN(Date.UTC(n('y',NaN),n('m',0),n('d',1),n('h',0),n('i',0),n('s',0),n('f',0))));test.record(log);\nlog='';try{Date.UTC(n('y',2000),Symbol(),n('d',1));}catch(e){test.record(e.name);}test.record(log);\nvar d=new Date(NaN);log='';test.record(isNaN(d.setUTCHours(n('h',1),n('m',2),n('s',3),n('f',4))));test.record(log);\n'ok';\n"),
])

NEGATIVE.extend([
    ('defaults-duplicate-list', "test.record('unreachable');function f(a=1,a){}"),
    ('defaults-strict-directive', "test.record('unreachable');function f(a=1){'use strict';}"),
    ('defaults-rest-initializer', "test.record('unreachable');function f(...a=[]){}"),
    ('defaults-arrow-duplicate', "test.record('unreachable');var f=(a,a=2)=>a;"),
    ('defaults-setter-trailing-comma', "test.record('unreachable');var o={set x(a=1,){}};"),
    ('patterns-rest-trailing-comma', "test.record('unreachable');let [...a,]=[];"),
    ('patterns-object-rest-pattern', "test.record('unreachable');let {...{a}}={};"),
    ('date-clock-default-deny', "try{Date.now();}catch(e){test.record('caught');}test.record('continued');"),
    ('date-constructor-clock-default-deny', "try{new Date();}catch(e){test.record('caught');}test.record('continued');"),
    ('date-local-constructor-explicit-unsupported', "try{new Date(2024,0,1);}catch(e){test.record('caught');}test.record('continued');"),
    ('date-local-time-parse-explicit-unsupported', "try{Date.parse('2024-01-01T12:00');}catch(e){test.record('caught');}test.record('continued');"),
    ('date-local-getter-explicit-unsupported', "try{new Date(0).getFullYear();}catch(e){test.record('caught');}test.record('continued');"),
    ('date-local-string-explicit-unsupported', "try{String(new Date(0));}catch(e){test.record('caught');}test.record('continued');"),
    ('date-local-function-explicit-unsupported', "try{Date(0);}catch(e){test.record('caught');}test.record('continued');"),
    ('date-object-allocation-bound', "try{for(var i=0;i<10000;i++)new Date(i);}catch(e){test.record('caught');}test.record('continued');"),
])
ATOMIC_NEGATIVE_CASES.update({'defaults-arrow-duplicate', 'date-local-constructor-explicit-unsupported', 'defaults-setter-trailing-comma', 'patterns-object-rest-pattern', 'date-clock-default-deny', 'patterns-rest-trailing-comma', 'date-local-getter-explicit-unsupported', 'defaults-strict-directive', 'date-local-string-explicit-unsupported', 'date-constructor-clock-default-deny', 'date-object-allocation-bound', 'defaults-duplicate-list', 'date-local-function-explicit-unsupported', 'date-local-time-parse-explicit-unsupported', 'defaults-rest-initializer'})

# Explicit current-ECMA vs Node24 reference difference, never counted as a Node match.
SPEC_DIFFERENCE_CASES = [
    ('date-utc-setters-snapshot-before-coercion', "var d=new Date('2000-01-02T03:04:05.006Z'),arg={valueOf:function(){d.setTime(0);return 8;}};\ntest.record(d.setUTCSeconds(arg));test.record(d.toISOString());\nvar invalid=new Date(NaN),arg2={valueOf:function(){invalid.setTime(0);return 2;}};test.record(isNaN(invalid.setUTCDate(arg2)));test.record(invalid.getTime());\n'ok';\n", ['number:426b8e13d388c000', 'string:323030302d30312d30325430333a30343a30382e3030365a', 'boolean:true', 'number:0000000000000000']),
]

# Original 0.40 object-spread sources use the same browser-global mode as their frozen baseline.
BROWSER_CASES.extend([
    ('spread-object-nullish-primitives-order', "var s=Symbol('k'),o={a:1,[s]:4};var r={before:0,...null,...undefined,...true,...3,...'ab',...o,a:9,after:2};test.record(Object.keys(r).join());test.record([r[0],r[1],r.a,r[s],r.after].join());'ok';"),
    ('spread-object-live-enumeration', "var log='',o={get a(){log+='a';delete o.b;o.c=3;return 1;},b:2},s=Symbol('s');o[s]=4;Object.defineProperty(o,'hidden',{value:5});Object.setPrototypeOf(o,{inherited:6});var r={...o};test.record(log);test.record(Object.keys(r).join());test.record(r[s]);test.record(r.c);test.record(r.inherited);'ok';"),
    ('spread-object-own-data-over-accessor', "var calls=0;Object.defineProperty(Object.prototype,'trap',{set(v){calls++;},configurable:true});var r={set trap(v){calls++;},...{trap:7}};delete Object.prototype.trap;var d=Object.getOwnPropertyDescriptor(r,'trap');test.record(r.trap);test.record(calls);test.record(d.writable&&d.enumerable&&d.configurable);test.record(d.set);'ok';"),
    ('spread-object-order-and-freshness', "var log='',source={get x(){log+='g';return 2;}};function make(){log+='s';return source;}var a={before:(log+='a',1),...make(),after:(log+='b',3)},b={...a};test.record(log);test.record(a!==b);test.record(Object.keys(b).join());test.record(b.x);'ok';"),
    ('spread-object-getter-throw', "var marker={},log='',o={get a(){log+='a';throw marker;},get b(){log+='b';return 2;}};try{var r={...o,later:(log+='z',3)};}catch(e){test.record(e===marker);}test.record(log);'ok';"),
])

# 0.41: retain original fixture bytes and compare complete outcomes. These
# development-only files and their original Node references were frozen before
# implementation; independent review inputs remain separate.
_041 = ROOT / "tools/reference/review-041"
_FIXTURE_PATHS_041 = [
    _041 / 'string-objects-cases.json',
    _041 / 'object-is-original-node.json',
    _041 / 'string-objects-boundary-adjudication.json',
    _041 / 'operator-supplement-bound-adjudication.json',
    ROOT / 'tools/operator_supplement_cases_041.json',
    ROOT / 'tools/reference/url-041/frozen.json',
    ROOT / 'tools/reference/url-041/edge-review-frozen.json',
    ROOT / 'tools/reference/url-041/bounds-corrected.json',
]
_FIXTURE_PATHS_041.extend(_BINDING_LEAF_FIXTURES)
_PRIMITIVE_RECEIVER_FIXTURE = ROOT / 'tools/fixtures/primitive-receiver-lookup.js'
_FIXTURE_PATHS_041.append(_PRIMITIVE_RECEIVER_FIXTURE)
COMPLETION_CASES.append(('primitive-receiver-lookup', _PRIMITIVE_RECEIVER_FIXTURE.read_text(encoding='utf-8')))
_UTF16_CHARACTER_FIXTURE = ROOT / 'tools/fixtures/utf16-character-reads.js'
_FIXTURE_PATHS_041.append(_UTF16_CHARACTER_FIXTURE)
COMPLETION_CASES.append(('utf16-character-reads', _UTF16_CHARACTER_FIXTURE.read_text(encoding='utf-8')))
# 0.43: retained design acceptance plus original binding/fallback coverage.
for _name in ['string-work-design', 'binding-length-reads']:
    _fixture = ROOT / ('tools/fixtures/' + _name + '.js')
    _FIXTURE_PATHS_041.append(_fixture)
    COMPLETION_CASES.append((_name, _fixture.read_text(encoding='utf-8')))
# 0.44: bounded primitive index reads, retaining normal reference semantics.
_INDEX_READ_FIXTURE = ROOT / 'tools/fixtures/primitive-string-index-reads.js'
_FIXTURE_PATHS_041.append(_INDEX_READ_FIXTURE)
COMPLETION_CASES.append(('primitive-string-index-reads', _INDEX_READ_FIXTURE.read_text(encoding='utf-8')))
_ATTRIBUTION_FIXTURE = ROOT / 'tools/fixtures/string-work-attribution.js'
COMPLETION_CASES.append(('string-work-attribution', _ATTRIBUTION_FIXTURE.read_text(encoding='utf-8')))

_FIXTURE_HASHES_041 = {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in _FIXTURE_PATHS_041}
_string_objects_041 = json.loads((_041 / "string-objects-cases.json").read_text(encoding="utf-8"))['cases']
_object_is_041 = json.loads((_041 / "object-is-original-node.json").read_text(encoding="utf-8"))['cases']
_operators_041 = json.loads((ROOT / "tools/operator_supplement_cases_041.json").read_text(encoding="utf-8"))
COMPLETION_CASES.append(('new-wrapper-constructor-cannot-catch-atomic', 'try{new String("x");}catch(e){test.record(1);}finally{test.record(2);}'))
COMPLETION_CASES.extend((row['id'], row['source']) for row in _string_objects_041 if row['kind'] == 'comparison')
COMPLETION_CASES.extend((row['id'], row['source']) for row in _object_is_041)
COMPLETION_CASES.extend((row['id'], row['source']) for row in _operators_041['cases'] if row['kind'] == 'comparison' and 'current_spec_records' not in row)
SPEC_DIFFERENCE_METADATA = {}
for _row in _operators_041['cases']:
    if 'current_spec_records' in _row:
        SPEC_DIFFERENCE_CASES.append((_row['id'], _row['source'], _row['current_spec_records']))
        SPEC_DIFFERENCE_METADATA[_row['id']] = {
            'scope': 'New assignment operators cache computed keys per current GetValue; raw Node24 differences remain separate from matches.',
            'primary_reference': 'https://tc39.es/ecma262/multipage/ecmascript-data-types-and-values.html#sec-getvalue',
        }
_limits_041 = [row for row in _string_objects_041 if row['kind'] == 'boundary']
_limits_041 += [row for row in _operators_041['cases'] if row['kind'] == 'early-error']
_limits_041 += _operators_041['bounds']
NEGATIVE.extend((row['id'], row['source']) for row in _limits_041)
ATOMIC_NEGATIVE_CASES.update(row['id'] for row in _limits_041)

# The forty original URL programs retain their frozen source and completion
# values. Corrected bound inputs are documented separately by the owner checker.
_url_041 = json.loads((ROOT / "tools/reference/url-041/frozen.json").read_text(encoding="utf-8"))
COMPLETION_CASES.extend((row['name'], row['source']) for row in _url_041['ordinary'])
_url_edges_041 = json.loads((ROOT / 'tools/reference/url-041/edge-review-frozen.json').read_text(encoding="utf-8"))['cases']
COMPLETION_CASES.extend((row['name'], row['source']) for row in _url_edges_041)
_url_bounds_041 = json.loads((ROOT / 'tools/reference/url-041/bounds-corrected.json').read_text(encoding="utf-8"))['bounds']
NEGATIVE.extend((row['name'], row['source']) for row in _url_bounds_041)
ATOMIC_NEGATIVE_CASES.update(row['name'] for row in _url_bounds_041)
NEGATIVE_DIAGNOSTICS_041 = {row['name']: row['diagnostic'] for row in _url_bounds_041}
NEGATIVE_DIAGNOSTICS_041.update(
    (name, 'Parser nesting limit reached (64)')
    for name, *_ in NEGATIVE if name.startswith('binding-leaf-')
)
NEGATIVE_DIAGNOSTICS_041.update({row['id']: row['error_contains'] for row in _string_objects_041 if row['kind'] == 'boundary'})
NEGATIVE_DIAGNOSTICS_041.update({row['id']: row['error_fragment'] for row in _operators_041['bounds']})
_string_bound_041 = json.loads((_041 / 'string-objects-boundary-adjudication.json').read_text(encoding="utf-8"))
NEGATIVE_DIAGNOSTICS_041[_string_bound_041['id']] = _string_bound_041['accepted_error_contains']
for _row in json.loads((_041 / 'operator-supplement-bound-adjudication.json').read_text(encoding="utf-8"))['cases']:
    NEGATIVE_DIAGNOSTICS_041[_row['id']] = _row['adjudicated_error_fragment']

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
    spec_difference_cases = selected(SPEC_DIFFERENCE_CASES)
    selected_counts = {"positive": len(cases), "browser": len(browser_cases),
                       "completion": len(completion_cases), "negative": len(negative_cases),
                       "spec_difference": len(spec_difference_cases)}
    if not sum(selected_counts.values()):
        parser.error("No cases match the supplied --case-prefix filters.")
    source_path = ROOT / "src" / "javascript.rs"
    def source_manifest():
        return {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
                for path in sorted((ROOT / "src").glob("*.rs"))}
    source_before = source_manifest()
    checker_before = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    try:
        rust_version = subprocess.check_output([options.rustc, "--version"], text=True, timeout=10).strip()
        node_version = subprocess.check_output([options.node, "--version"], text=True, timeout=10).strip()
        results = []
        negatives = []
        spec_differences = []
        with tempfile.TemporaryDirectory(prefix="chezcat-js-check-") as directory:
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
            for name, source, expected_records in spec_difference_cases:
                reference = run_node(options.node, source)
                actual = run_own(probe, source)
                passed = (finished_normally(actual) and actual["ok"]
                          and actual["records"] == expected_records
                          and actual["value"] == "string:6f6b")
                spec_differences.append({
                    "case": name, "passed": passed, "actual": actual,
                    "node_reference": reference,
                    "current_spec_records": expected_records,
                    "matches_node": (finished_normally(actual) and actual["ok"] == reference["ok"]
                                     and actual["records"] == reference["records"]
                                     and actual["value"] == reference["value"]),
                    **SPEC_DIFFERENCE_METADATA.get(name, {
                        "scope": "Date slot snapshot precedes argument conversion; excluded from Node-match totals.",
                        "primary_reference": "https://tc39.es/ecma262/multipage/numbers-and-dates.html#sec-date.prototype.setutcseconds",
                    }),
                })
            for case in negative_cases:
                name, source, *mode = case
                actual = run_own(probe, source, handler=bool(mode and mode[0]))
                passed = finished_normally(actual) and not actual["ok"]
                if name.endswith("-atomic") or name in ATOMIC_NEGATIVE_CASES:
                    passed = passed and not actual["records"]
                diagnostic = NEGATIVE_DIAGNOSTICS_041.get(name)
                if diagnostic:
                    passed = passed and diagnostic.lower() in (actual.get("error") or "").lower()
                negatives.append({"case": name, "passed": passed, "actual": actual,
                                  **({"expected_diagnostic": diagnostic} if diagnostic else {})})
    except (OSError, subprocess.SubprocessError, ValueError) as error:
        detail = getattr(error, "stderr", None)
        print(f"JavaScript checker could not finish: {error}", file=sys.stderr)
        if detail:
            print(detail, file=sys.stderr)
        print("Install Python 3, Rust, and Node.js for these development checks; the browser itself does not require Node.js.", file=sys.stderr)
        return 2

    source_after = source_manifest()
    fixture_after = {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in _FIXTURE_PATHS_041}
    fixture_unchanged = _FIXTURE_HASHES_041 == fixture_after
    checker_after = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
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
        "object_literals_source_sha256": hashlib.sha256((ROOT / "src/object_literals.rs").read_bytes()).hexdigest(),
        "array_descriptors_source_sha256": hashlib.sha256((ROOT / "src/array_descriptors.rs").read_bytes()).hexdigest(),
        "primitive_wrappers_source_sha256": hashlib.sha256((ROOT / "src/primitive_wrappers.rs").read_bytes()).hexdigest(),
        "primitive_conversion_source_sha256": hashlib.sha256((ROOT / "src/primitive_conversion.rs").read_bytes()).hexdigest(),
        "iterator_protocol_source_sha256": hashlib.sha256((ROOT / "src/iterator_protocol.rs").read_bytes()).hexdigest(),
        "iterable_builtins_source_sha256": hashlib.sha256((ROOT / "src/iterable_builtins.rs").read_bytes()).hexdigest(),
        "spread_source_sha256": hashlib.sha256((ROOT / "src/spread.rs").read_bytes()).hexdigest(),
        "rest_tests_source_sha256": hashlib.sha256((ROOT / "src/rest_parameters.rs").read_bytes()).hexdigest(),
        "promises_source_sha256": hashlib.sha256((ROOT / "src/promises.rs").read_bytes()).hexdigest(),
        "template_literals_source_sha256": hashlib.sha256((ROOT / "src/template_literals.rs").read_bytes()).hexdigest(),
        "function_constructor_source_sha256": hashlib.sha256((ROOT / "src/function_constructor.rs").read_bytes()).hexdigest(),
        "optional_chaining_source_sha256": hashlib.sha256((ROOT / "src/optional_chaining.rs").read_bytes()).hexdigest(),
        "parameters_source_sha256": hashlib.sha256((ROOT / "src/parameters.rs").read_bytes()).hexdigest(),
        "patterns_source_sha256": hashlib.sha256((ROOT / "src/binding_patterns.rs").read_bytes()).hexdigest(),
        "named_regexp_source_sha256": hashlib.sha256((ROOT / "src/named_regexp.rs").read_bytes()).hexdigest(),
        "date_source_sha256": hashlib.sha256((ROOT / "src/date.rs").read_bytes()).hexdigest(),
        "source_files_before": source_before,
        "source_files_after": source_after,
        "source_files_unchanged_during_run": source_before == source_after,
        "checker_sha256": checker_before,
        "checker_unchanged_during_run": checker_before == checker_after,
        "fixture_inputs_before": _FIXTURE_HASHES_041,
        "fixture_inputs_after": fixture_after,
        "fixture_inputs_unchanged_during_run": fixture_unchanged,
        "oracle_cases": len(results),
        "oracle_passed": sum(item["passed"] for item in results),
        "negative_cases": len(negatives),
        "negative_passed": sum(item["passed"] for item in negatives),
        "spec_difference_cases": len(spec_differences),
        "spec_difference_assertions_passed": sum(item["passed"] for item in spec_differences),
        "results": results,
        "negatives": negatives,
        "spec_differences": spec_differences,
    }
    if options.output:
        options.output.parent.mkdir(parents=True, exist_ok=True)
        options.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    summary = {key: value for key, value in report.items()
               if key not in ("results", "negatives", "spec_differences", "source_files_before", "source_files_after")}
    print(json.dumps(summary, indent=2))
    failed = [item for item in results + negatives + spec_differences if not item["passed"]]
    for item in failed:
        print(json.dumps(item, indent=2))
    return 1 if failed or source_before != source_after or checker_before != checker_after or not fixture_unchanged else 0


if __name__ == "__main__":
    raise SystemExit(main())
