// Original neutral UTF-16 character-read fixture; no captured site code.
function equal(actual, expected, label) {
    if (actual !== expected && !(actual !== actual && expected !== expected)) throw label;
}
var positions = [undefined, NaN, 0, -0, -0.75, 0.9, null, false, '', 'nope',
    true, '1', 1.9, 2, '2.9', -1, 3, 1e300, Infinity, -Infinity];
var characters = ['A', 'A', 'A', 'A', 'A', 'A', 'A', 'A', 'A', 'A',
    'B', 'B', 'B', 'C', 'C', '', '', '', '', ''];
var codes = [65, 65, 65, 65, 65, 65, 65, 65, 65, 65,
    66, 66, 66, 67, 67, NaN, NaN, NaN, NaN, NaN];
for (var i = 0; i < positions.length; i++) {
    equal('ABC'.charAt(positions[i]), characters[i], 'ASCII charAt position');
    equal('ABC'.charCodeAt(positions[i]), codes[i], 'ASCII charCodeAt position');
    equal('ABC'.codePointAt(positions[i]), characters[i] === '' ? undefined : codes[i],
        'ASCII codePointAt position');
    equal(''.charAt(positions[i]), '', 'empty charAt');
    equal(''.charCodeAt(positions[i]), NaN, 'empty charCodeAt');
    equal(''.codePointAt(positions[i]), undefined, 'empty codePointAt');
}
equal('ABC'.charAt(), 'A', 'omitted charAt');
equal('ABC'.charCodeAt(), 65, 'omitted charCodeAt');
equal('ABC'.codePointAt(), 65, 'omitted codePointAt');
var bmp = 'éΩ中';
equal(bmp.charAt(0), 'é', 'BMP start');
equal(bmp.charAt(1), 'Ω', 'BMP middle');
equal(bmp.charAt(2), '中', 'BMP end');
equal(bmp.charCodeAt(2), 20013, 'BMP code');
equal(bmp.codePointAt(1), 937, 'BMP point');
var astral = 'A😀ΩZ';
equal(astral.charCodeAt(1), 55357, 'high surrogate code');
equal(astral.charCodeAt(2), 56832, 'low surrogate code');
equal(astral.codePointAt(1), 128512, 'paired point');
equal(astral.codePointAt(2), 56832, 'low surrogate point');
equal(astral.charAt(3), 'Ω', 'post-pair index');
equal(astral.charAt(4), 'Z', 'astral tail');
equal(astral.charCodeAt(5), NaN, 'astral past end');
equal(astral.codePointAt(5), undefined, 'astral point past end');
var method = String.prototype.charCodeAt, order = '', calls = 0;
Object.defineProperty(String.prototype, 'fixtureCharacter', {
    configurable: true,
    get: function () {
        'use strict'; order += 'g'; calls++;
        equal(this, 'ABC', 'getter receiver');
        return method;
    }
});
function argument() { order += 'a'; return 1; }
equal('ABC'.fixtureCharacter(argument()), 66, 'getter method');
equal(order, 'ga', 'getter before arguments');
equal(calls, 1, 'one getter lookup');
delete String.prototype.fixtureCharacter;
function replace() {
    String.prototype.charCodeAt = function () { 'use strict'; return this === 'ABC' ? 99 : 0; };
    return 1;
}
equal('ABC'.charCodeAt(replace()), 66, 'resolved before mutation');
equal('ABC'.charCodeAt(0), 99, 'overridden method');
String.prototype.charCodeAt = method;
equal(('ABC'?.charCodeAt)(2), 67, 'grouped optional receiver');
equal('ABC'?.charCodeAt?.(2), 67, 'optional call');
'ok';
