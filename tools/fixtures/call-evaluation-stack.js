// Original call-order witnesses for the 0.47 stack-frame repair.
function require(value, label) { if (!value) throw label; }
var trace = '';
var original = {
    value: 7,
    get run() {
        trace += 'g';
        return function (argument) {
            'use strict';
            trace += 'r';
            require(this === original, 'member receiver');
            return this.value + argument;
        };
    }
};
var replacement = {run: function () { return -1; }};
var current = original;
var key = {toString: function () { trace += 'k'; current = replacement; return 'run'; }};
function argument() {
    trace += 'a';
    delete original.run;
    original.run = replacement.run;
    return 2;
}
require(current[key](argument()) === 9, 'snapshotted method');
require(trace === 'kgar' && current === replacement, 'key/getter/argument/call order');

trace = '';
var symbol = Symbol('method');
var receiver = {value: 11};
receiver[symbol] = function (value) {
    'use strict';
    trace += 'r';
    require(this === receiver, 'symbol receiver');
    return this.value + value;
};
require(receiver[symbol]((trace += 'a', receiver[symbol] = replacement.run, 3)) === 14,
        'symbol method snapshot');
require(trace === 'ar', 'symbol argument order');

var marker = {}, after = 0;
var throwing = {get run() { throw marker; }};
var caught;
try { throwing.run(after++); } catch (error) { caught = error; }
require(caught === marker && after === 0, 'getter throw identity');
try { (0)(after++); } catch (error) { require(error.name === 'TypeError', 'callability'); }
require(after === 1, 'arguments before noncallable error');
try { null[(after++, 'run')](after++); } catch (error) {
    require(error.name === 'TypeError', 'null receiver');
}
require(after === 2, 'key expression before null error, arguments after');

function direct() { var localOnly = 19; return eval('localOnly + 1'); }
function indirect() { var privateOnly = 23; return (0, eval)('typeof privateOnly'); }
require(direct() === 20 && indirect() === 'undefined', 'direct eval context');

trace = '';
var iterable = {};
iterable[Symbol.iterator] = function () {
    trace += 'i';
    var index = 0;
    return {next: function () {
        trace += 'n';
        return index++ === 0 ? {value: 2, done: false} : {done: true};
    }};
};
function sum(a, b) { trace += 'c'; return a + b; }
require(sum(...iterable, (trace += 'a', 3)) === 5, 'spread values');
require(trace === 'innac', 'spread expansion order');

var optional = null;
require(optional?.(after++) === undefined && after === 2, 'optional short circuit');
var grouped = {value: 5, run: function (value) {
    'use strict';
    require(this === grouped, 'grouped chain receiver');
    return this.value + value;
}};
require((grouped?.run)(4) === 9, 'grouped chain call');
'ok';
