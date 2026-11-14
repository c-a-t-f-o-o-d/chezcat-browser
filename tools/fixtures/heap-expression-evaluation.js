// Original evaluation-order fixtures, independent of public-site scripts.
var order = '';
function mark(name, value) { order += name; return value; }
var object = {value: 5, get method() {
    order += 'g';
    return function(a, b) { 'use strict'; order += 'c'; return this.value + a + b; };
}};
var key = {toString: function() { order += 'k'; return 'method'; }};
if (object[key](mark('a', 2), mark('b', 3)) !== 10 || order !== 'kgabc') throw 'member order';
order = '';
if (object?.[key]?.(mark('a', 2), mark('b', 3)) !== 10 || order !== 'kgabc') throw 'optional order';
order = '';
if ((object?.method)(mark('a', 2), mark('b', 3)) !== 10 || order !== 'gabc') throw 'grouped receiver';
order = '';
var changed = object;
var answer = changed.method((changed = {value: 90}, 2), 3);
if (answer !== 10 || order !== 'gc') throw 'receiver snapshot';
order = '';
var missing = null;
if (missing?.[mark('key', 'method')]?.(mark('arg', 1)) !== undefined || order !== '') throw 'nullish skipping';
var caught = '';
try { (missing?.method)(mark('a', 1)); } catch (error) { caught = error.name; }
if (caught !== 'TypeError' || order !== 'a') throw 'grouping boundary';
order = '';
var stored = 0;
var target = {get value() {order += 'g'; return stored;}, set value(v) {order += 's'; stored = v;}};
if ((target.value ||= mark('r', 7)) !== 7 || stored !== 7 || order !== 'grs') throw 'logical assignment';
order = '';
if ((target.value ||= mark('r', 8)) !== 7 || order !== 'g') throw 'logical skipping';
order = '';
var before = target.value++;
if (before !== 7 || stored !== 8 || order !== 'gs') throw 'update reference';
var s = Symbol('method');
object[s] = function(x) { 'use strict'; return this === object ? x : -1; };
if (object[s](11) !== 11 || (object?.[s])(12) !== 12) throw 'symbol receiver';
var deletes = 0;
var removable = {get value() {deletes++; return 8;}};
if (!delete removable?.value || deletes !== 0 || Object.hasOwn(removable, 'value')) throw 'delete getter';
order = '';
function collect(a, b, c) {return a + ':' + b + ':' + c;}
if (collect(mark('a', 1), ...mark('s', [2]), mark('b', 3)) !== '1:2:3' || order !== 'asb') throw 'spread order';
order = '';
function Box(x) {this.x = x; order += 'c';}
if ((new Box(mark('a', 9))).x !== 9 || order !== 'ac') throw 'constructor order';
order = '';
try { new (mark('t', 0))(mark('a', 1)); } catch (error) {caught = error.name;}
if (caught !== 'TypeError' || order !== 'ta') throw 'nonconstructor arguments';
function local() {var secret = 17; return eval('secret') + (0, eval)('typeof secret').length;}
if (local() !== 26) throw 'direct eval';
var side = 0;
if ((0 && ++side) !== 0 || ('yes' || ++side) !== 'yes' || (false ?? ++side) !== false || side !== 0) throw 'short circuit';
if ((true ? (side = 2, side + 3) : ++side) !== 5 || side !== 2) throw 'conditional sequence';
var marker = {};
try { object.method(1, (function() {throw marker;})()); } catch (error) {if (error !== marker) throw 'throw identity';}
var aliased = object;
aliased[mark('q', 'value')] = (aliased = {value: 91}, 6);
if (object.value !== 6 || aliased.value !== 91) throw 'assignment snapshot';
'ok';
