// Original local fixture: no network, site source, credentials or access checks.
var trace = '';
var object = {
  value: 9,
  get method() {
    trace += 'g';
    return function (v) { 'use strict'; trace += this === object ? 'r' : 'u'; return this === object ? this.value + v : -1; };
  }
};
function argument() { trace += 'a'; return 4; }
if (((object.method))(argument()) !== 13 || trace !== 'gar') throw 'group receiver';
trace = '';
if (((object?.method))(argument()) !== 13 || trace !== 'gar') throw 'grouped optional receiver';
trace = '';
if ((0, object.method)(argument()) !== -1 || trace !== 'gau') throw 'sequence GetValue';
trace = '';
if (object?.method?.(argument()) !== 13 || trace !== 'gar') throw 'optional receiver';
var missing = null, count = 0;
if (missing?.[count++]?.(count++) !== undefined || count !== 0) throw 'short circuit';
try { (missing?.method)(count++); } catch (error) { if (error.name !== 'TypeError') throw error; }
if (count !== 1) throw 'group ends chain';
var receiver = { x: 11 }, previous = receiver;
function key() { receiver = { x: 22 }; return 'x'; }
if (receiver[(key())] !== 11 || previous.x !== 11 || receiver.x !== 22) throw 'computed snapshot';
function total(a, b, c) { return a + b + c; }
if (total((1, 2), ...[3, 4],) !== 9) throw 'comma spread trailing';
var a = 0, b = 0;
if ((a = b = true ? 2 + 3 * 4 : 0) !== 14 || a !== 14 || b !== 14) throw 'assignment conditional';
if (2 ** 3 ** 2 !== 512 || (-2) ** 2 !== 4 || 2 ** -2 !== 0.25) throw 'power';
if (((null ?? 3) || 7) !== 3 || (null ?? (false || 4)) !== 4) throw 'coalescing';
function Box(v) { this.value = v; }
if (new Box((3 + 4)).value !== 7 || new (Box)(8)?.value !== 8) throw 'new';
var arrow = ((x = total(1, 2, 3)) => ((x + 1)));
if (arrow() !== 7 || `${total((1), 2, 3)}` !== '6') throw 'helper reentry';
var same = { x: 1 };
((same.x)) += total(1, 2, 3);
if (same.x !== 7 || delete (same?.x) !== true || 'x' in same) throw 'reference update';
var order = '';
var dst = { x: 0 };
function assignKey() { order += 'k'; return 'x'; }
function assignValue() { order += 'v'; return 7; }
dst[(assignKey())] = assignValue();
if (order !== 'kv' || dst.x !== 7) throw 'key expression order';
var truth = true ? ('x' in { x: 1 }) : false;
for (var active = true ? 'x' in { x: 1 } : false; active; active = false) { count++; }
if (!truth || count !== 2) throw 'In contexts';
'ok';
