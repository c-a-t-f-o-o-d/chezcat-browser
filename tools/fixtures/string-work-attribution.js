// Original semantic witness. Instrumentation must not change these observations.
var text = 'A\u00e9\ud83d\ude00Z', saved = text, keys = 0, reads = 0, calls = 0;
function key() { keys++; text = 'changed'; return 'length'; }
if (text[key()] !== 5 || text !== 'changed' || keys !== 1) throw 'receiver snapshot';
if (saved.length !== 5 || saved['length'] !== 5 || saved?.length !== 5) throw 'length';
if (saved[0] !== 'A' || saved[1] !== '\u00e9' || saved[4] !== 'Z') throw 'indices';
if (saved.charCodeAt(2) !== 55357 || saved.charCodeAt(3) !== 56832) throw 'UTF16';
if (saved.codePointAt(2) !== 128512 || saved.charAt(4) !== 'Z') throw 'characters';
Object.defineProperty(String.prototype, 'measured', {get: function () {
    'use strict'; reads++; if (this !== saved) throw 'getter receiver'; return this.length;
}, configurable: true});
if (saved.measured !== 5 || reads !== 1) throw 'getter ordering';
if (!delete String.prototype.measured) throw 'getter cleanup';
var converted = {toString: function () { calls++; return saved; }};
if (String(converted) !== saved || calls !== 1) throw 'conversion';
var absent = null;
if (absent?.[key()] !== undefined || keys !== 1) throw 'optional short circuit';
if (typeof string_work_failure_report !== 'undefined') throw 'diagnostic API exposed';
'ok';
