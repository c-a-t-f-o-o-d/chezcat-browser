// Original indexed-read semantics. No captured website code or network access.
var inputs = ['', 'ABC', 'éΩ中', 'A😀Z', 'Aé😀Z', '\u0000\u007f\u0800Z'];
for (var n = 0; n < inputs.length; n++) {
    var text = inputs[n];
    for (var i = 0; i < text.length; i++) {
        var unit = text.charCodeAt(i);
        // The current UTF-8 string representation rejects isolated surrogates.
        // Native fatal-boundary tests cover those reads; this fixture compares
        // only outputs that the browser actually supports.
        if (unit >= 0xD800 && unit <= 0xDFFF) continue;
        if (text[i] !== text.charAt(i)) throw 'indexed character';
        if (text['' + i] !== text.charAt(i)) throw 'string key';
        if (text?.[i] !== text.charAt(i)) throw 'optional index';
        if (new String(text)[i] !== text[i]) throw 'wrapper control';
    }
    if (text[text.length] !== undefined || text[100000] !== undefined) throw 'end';
}
if ('abc'[-0] !== 'a') throw 'numeric negative zero';
for (var key of ['-0', '-1', '01', '1.0', '1e0', '4294967295']) {
    if ('abc'[key] !== undefined) throw 'nonindex';
}
var source = 'abc', order = '', oldReceiver;
Object.defineProperty(String.prototype, '8', {
    configurable: true,
    get: function() { 'use strict'; order += 'g'; oldReceiver = this; return 'fallback'; }
});
var computed = {toString: function() {order += 'k'; source = 'replacement'; return '8';}};
if (source[computed] !== 'fallback' || oldReceiver !== 'abc' || order !== 'kg') throw 'snapshot';
if (source !== 'replacement') throw 'key effect';
delete String.prototype['8'];
var calls = 0;
Object.defineProperty(String.prototype, '1', {
    configurable: true,
    get: function() { 'use strict'; calls++; oldReceiver = this; return 17; }
});
if ('ab'[1] !== 'b' || calls !== 0) throw 'own index precedence';
if ('a'[1] !== 17 || calls !== 1 || oldReceiver !== 'a') throw 'missing index getter';
if (''?.[1] !== 17 || calls !== 2 || oldReceiver !== '') throw 'empty fallback';
delete String.prototype['1'];
Object.defineProperty(String.prototype, '5', {
    configurable: true,
    get: function() { 'use strict'; oldReceiver = this; calls++; return 29; }
});
// UTF-16 end is five, while UTF-8 byte length is eight: scan then fall back.
if ('Aé😀Z'[5] !== 29 || oldReceiver !== 'Aé😀Z' || calls !== 3) throw 'Unicode end';
delete String.prototype['5'];
String.prototype['01'] = 'named';
if ('ab'['01'] !== 'named' || 'ab'[1] !== 'b') throw 'named control';
delete String.prototype['01'];
var originalCharAt = String.prototype.charAt;
String.prototype.charAt = function() { throw 'must not call charAt'; };
if ('abc'[1] !== 'b') throw 'override independence';
String.prototype.charAt = originalCharAt;
source = 'abc'; order = '';
computed = {toString: function() {order += 'k'; source = 'XYZ'; return '1';}};
if (source[computed] !== 'b' || source !== 'XYZ' || order !== 'k') throw 'own snapshot';
var marker = {}, caught = false;
computed = {toString: function() {throw marker;}};
try { source[computed]; } catch (e) { caught = e === marker; }
if (!caught) throw 'key exception';
var skipped = 0, absent = null;
if (absent?.[(skipped++, 0)] !== undefined || skipped !== 0) throw 'optional short circuit';
var symbol = Symbol('index');
String.prototype[symbol] = 31;
if ('abc'[symbol] !== 31) throw 'symbol control';
delete String.prototype[symbol];
'ok';
