// Original neutral acceptance fixture for future string-work changes.
// Equal UTF-8 byte counts must not be mistaken for equal UTF-16 lengths.
function equal(actual, expected, label) {
    if (actual !== expected) throw label;
}
var texts = ['', 'abcd', 'éAB', 'éé', '😀', 'A😀ΩZ'];
var lengths = [0, 4, 3, 2, 2, 5];
for (var i = 0; i < texts.length; i++) {
    var current = texts[i];
    equal(current.length, lengths[i], 'literal or array string length');
    equal(current['length'], lengths[i], 'computed length');
    equal(current.length, lengths[i], 'repeat length');
}

var source = 'A😀Z', alias = source;
source = 'éé';
equal(source.length, 2, 'reassigned binding');
equal(alias, 'A😀Z', 'independent alias contents');
equal(alias.length, 4, 'independent alias length');
equal((alias + 'Ω').length, 5, 'concatenated length');
equal(alias.slice(1, 3), '😀', 'complete-pair slice');
equal(alias.slice(1, 3).length, 2, 'slice length');
equal(alias.substring(1, 3).length, 2, 'substring length');
equal(String(2026).length, 4, 'primitive conversion length');
equal(String.fromCharCode(0xD83D, 0xDE00, 0x3A9).length, 3,
    'unit constructor length');
var wrapped = new String(alias);
alias = 'éAB';
equal(wrapped.length, 4, 'wrapper snapshot length');
equal(wrapped.valueOf(), 'A😀Z', 'wrapper snapshot contents');
equal(alias.length, 3, 'same-byte-count replacement');

var subject = 'A😀Z';
var key = {toString: function () { subject = 'é'; return 'length'; }};
equal(subject[key], 4, 'receiver evaluated before key conversion');
equal(subject.length, 1, 'key conversion effect retained');
var reads = 0, holder = {};
Object.defineProperty(holder, 'text', {
    get: function () { reads++; return reads === 1 ? 'abcd' : '😀'; }
});
equal(holder.text.length, 4, 'first accessor result length');
equal(holder.text.length, 2, 'new accessor result length');
equal(reads, 2, 'no cached accessor result');

var log = '', gets = 0, live = 'A😀Z';
Object.defineProperty(String.prototype, 'fixtureLengthCall', {
    configurable: true,
    get: function () {
        'use strict'; log += 'g'; gets++;
        equal(this, 'A😀Z', 'getter receiver');
        return function (n) {
            'use strict'; log += 'c';
            equal(this, 'A😀Z', 'call receiver snapshot');
            return this.length + n;
        };
    }
});
function receiver() { log += 'r'; return live; }
function argument() { log += 'a'; live = 'é'; return 5; }
equal(receiver().fixtureLengthCall(argument()), 9, 'getter method result');
equal(log, 'rgac', 'receiver getter argument call order');
equal(gets, 1, 'one getter evaluation');
equal(live.length, 1, 'argument mutation retained');
delete String.prototype.fixtureLengthCall;
'ok';
