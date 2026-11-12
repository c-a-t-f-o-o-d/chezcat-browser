// Original neutral property-lookup fixture. No captured site code.
var text = 'A😀Z';
if (text.length !== 4 || text.charCodeAt(1) !== 55357 ||
    text.charCodeAt(2) !== 56832 || text.codePointAt(1) !== 128512 ||
    text.charAt(0) !== 'A' || text[3] !== 'Z') throw 'UTF-16 reads';
if (text.fixtureMissing !== undefined || text.fixtureMissing?.() !== undefined)
    throw 'missing property';
String.prototype.fixtureData = 17;
if (text.fixtureData !== 17) throw 'prototype data';
delete String.prototype.fixtureData;
var numericGets = 0, setterArguments = 0;
Object.defineProperty(String.prototype, '0', {
    configurable: true, get: function () { throw 'in-range index shadowed'; }
});
Object.defineProperty(String.prototype, '9', {
    configurable: true, get: function () {
        'use strict'; numericGets++;
        if (this !== text) throw 'out-of-range getter receiver';
        return 99;
    }
});
Object.defineProperty(String.prototype, 'fixtureSetterOnly', {
    configurable: true, set: function () { throw 'unexpected setter'; }
});
if (text[0] !== 'A' || text[9] !== 99 || numericGets !== 1 ||
    text.fixtureSetterOnly !== undefined ||
    text.fixtureSetterOnly?.(setterArguments++) !== undefined || setterArguments !== 0)
    throw 'numeric or setter-only lookup';
delete String.prototype['0']; delete String.prototype['9'];
delete String.prototype.fixtureSetterOnly;

var originalSlice = String.prototype.slice;
function replaceSlice() {
    String.prototype.slice = function () { 'use strict'; return this + ':override'; };
}
if ('abc'.slice(1, replaceSlice()) !== 'bc' || 'abc'.slice() !== 'abc:override')
    throw 'method selected before arguments';
String.prototype.slice = originalSlice;

var log = '', gets = 0, current = 'abc';
Object.defineProperty(String.prototype, 'fixtureCall', {
    configurable: true,
    get: function () {
        'use strict';
        log += 'g'; gets++;
        if (this !== 'abc') throw 'getter receiver';
        Object.defineProperty(String.prototype, 'fixtureCall', {
            configurable: true, value: function () { throw 'second lookup'; }
        });
        return function (value) {
            'use strict';
            log += 'c';
            if (this !== 'abc') throw 'call receiver changed';
            return this + value;
        };
    }
});
function receiver() { log += 'r'; return current; }
var key = { toString: function () { log += 'k'; return 'fixtureCall'; } };
function argument() {
    log += 'a'; current = 'changed';
    Object.defineProperty(String.prototype, 'fixtureCall', {
        configurable: true, value: function () { return 'replacement'; }
    });
    return '!';
}
if (receiver()[key](argument()) !== 'abc!' || log !== 'rkgac' || gets !== 1)
    throw 'single lookup and evaluation order';
if ('abc'.fixtureCall() !== 'replacement') throw 'prototype mutation';
delete String.prototype.fixtureCall;

Object.defineProperty(Object.prototype, 'fixtureInherited', {
    configurable: true, get: function () { 'use strict'; return this; }
});
var wrapped = Object('abc');
if ('abc'.fixtureInherited !== 'abc' || wrapped.fixtureInherited !== wrapped)
    throw 'inherited strict receiver';
delete Object.prototype.fixtureInherited;

Object.defineProperty(String.prototype, 'fixtureSloppy', {
    configurable: true, get: function () { return this; }
});
var first = 'abc'.fixtureSloppy, second = 'abc'.fixtureSloppy;
if (typeof first !== 'object' || first === second || first.valueOf() !== 'abc' ||
    second.valueOf() !== 'abc') throw 'fresh sloppy getter wrappers';
if (wrapped.fixtureSloppy !== wrapped) throw 'existing wrapper identity';
delete String.prototype.fixtureSloppy;

String.prototype.fixtureMethod = function () { 'use strict'; return this; };
if (('abc'.fixtureMethod)() !== 'abc' || 'abc'?.fixtureMethod?.() !== 'abc' ||
    ('abc'?.fixtureMethod)() !== 'abc' || (0, 'abc'.fixtureMethod)() !== undefined)
    throw 'grouped and optional references';
if (wrapped.fixtureMethod() !== wrapped) throw 'wrapper method receiver';
delete String.prototype.fixtureMethod;

var symbol = Symbol('fixture'), symbolGets = 0;
Object.defineProperty(String.prototype, symbol, {
    configurable: true,
    get: function () {
        'use strict'; symbolGets++;
        if (this !== 'abc') throw 'symbol getter receiver';
        return function (n) { 'use strict'; return this + n; };
    }
});
if ('abc'[symbol](3) !== 'abc3' || 'abc'?.[symbol]?.(4) !== 'abc4' || symbolGets !== 2)
    throw 'symbol lookup';
delete String.prototype[symbol];

var marker = {}, caught = 0, argumentsRun = 0, throws = 0;
Object.defineProperty(String.prototype, 'fixtureThrow', {
    configurable: true, get: function () { 'use strict'; throws++; throw marker; }
});
try { 'abc'.fixtureThrow(argumentsRun++); } catch (error) { if (error === marker) caught++; }
try { 'abc'.fixtureThrow?.(argumentsRun++); } catch (error) { if (error === marker) caught++; }
if (caught !== 2 || throws !== 2 || argumentsRun !== 0) throw 'getter abrupt completion';
delete String.prototype.fixtureThrow;
'ok';
