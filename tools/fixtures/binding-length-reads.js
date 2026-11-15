// Original 0.43 binding-length regressions. No captured site code.
function same(actual, expected, label) {
    if (actual !== expected) throw label + ': ' + actual;
}
function pending(a, b) { return arguments.length; }
function named(arguments) { return arguments.length; }
function overwritten(a) { arguments = 'A😀Z'; return arguments['length']; }
function mapped(value) {
    var first = value.length;
    arguments[0] = '😀';
    var second = value['length'];
    arguments[0] = {length: 7};
    return first + ':' + second + ':' + value.length;
}
function unmapped(value) {
    'use strict';
    arguments[0] = 'a';
    return value.length;
}
function captured() {
    let word = 'A😀';
    return function (next) {
        var before = word.length;
        word = next;
        return before + ':' + word['length'];
    };
}
same(pending(1, 2), 2, 'lazy arguments');
same(named('😀'), 2, 'named arguments');
same(overwritten(1), 4, 'replaced arguments');
same(mapped('abcd'), '4:2:7', 'mapped parameter and nonstring fallback');
same(unmapped('A😀Z'), 4, 'strict unmapped arguments');
var closure = captured();
same(closure('é'), '3:1', 'closure replacement');
same(closure('😀'), '1:2', 'closure reread');
var shadow = 'outer';
{ let shadow = '😀'; same((shadow).length, 2, 'shadowed binding'); }
same(shadow['length'], 5, 'outer binding');
var subject = 'A😀Z', keyCalls = 0;
function key() { keyCalls++; subject = 'é'; return 'length'; }
same(subject[key()], 4, 'receiver snapshot before key call');
same(keyCalls, 1, 'one key call');
same(subject.length, 1, 'later binding read');
subject = 'abcd';
var coercions = 0, keyObject = {};
keyObject[Symbol.toPrimitive] = function (hint) {
    same(hint, 'string', 'key hint');
    coercions++; subject = '😀'; return 'length';
};
same(subject[keyObject], 4, 'receiver snapshot before key coercion');
same(coercions, 1, 'one key coercion');
same(subject?.length, 2, 'optional chain fallback');
var reads = 0, holder = {get length() { reads++; return 9; }};
same(holder.length, 9, 'object getter fallback');
same(reads, 1, 'one getter read');
var errors = 0;
try { absentLengthBinding.length; } catch (e) {
    same(e.name, 'ReferenceError', 'missing name'); errors++;
}
try { let later = later.length; } catch (e) {
    same(e.name, 'ReferenceError', 'TDZ'); errors++;
}
var nullValue = null, undefinedValue;
try { nullValue.length; } catch (e) {
    same(e.name, 'TypeError', 'null fallback'); errors++;
}
try { undefinedValue['length']; } catch (e) {
    same(e.name, 'TypeError', 'undefined fallback'); errors++;
}
same(errors, 4, 'all error paths');
'ok';
