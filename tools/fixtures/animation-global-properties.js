(function () {
    function check(ok, message) { if (!ok) throw Error(message); }
    check(window === self && self === globalThis, 'global aliases');
    for (var name of ['requestAnimationFrame', 'cancelAnimationFrame']) {
        var original = window[name], calls = 0, reads = 0, writes = 0, last;
        check(original === self[name] && original === globalThis[name], 'operation aliases');
        var d = Object.getOwnPropertyDescriptor(window, name);
        check(d.value === original && d.writable && d.enumerable && d.configurable &&
              original.name === name && original.length === 1, 'operation descriptor');
        check(Object.hasOwn(window, name) && Object.prototype.propertyIsEnumerable.call(window, name), 'own predicates');
        var replacement = function (x) { calls++; return x; };
        window[name] = replacement;
        check(self[name](3) === 3 && globalThis[name]?.(4) === 4 && calls === 2, 'replacement calls');
        Object.defineProperty(window, name, {
            get: function () { check(this === window, 'getter this'); reads++; return replacement; },
            set: function (x) { check(this === window, 'setter this'); writes++; last = x; },
            configurable: true
        });
        check(name in window && Object.hasOwn(window, name) &&
              typeof Object.getOwnPropertyDescriptor(window, name).get === 'function' && reads === 0, 'inspection');
        check(window[name](5) === 5 && reads === 1, 'accessor call');
        window[name] = original;
        check(last === original && writes === 1 && reads === 1, 'accessor setter');
        Object.defineProperty(window, name, {value: original, writable: true, enumerable: true, configurable: true});
        check(delete window[name] && window[name] === undefined && !(name in window) &&
              Object.getOwnPropertyDescriptor(window, name) === undefined, 'no hidden restoration');
        Object.prototype[name] = replacement;
        check(name in window && !Object.hasOwn(window, name) && window[name](6) === 6, 'inherited lookup');
        window[name] = original;
        check(Object.hasOwn(window, name) && window[name] === original, 'own restoration');
        delete Object.prototype[name];
    }
    var frame = requestAnimationFrame, cancel = cancelAnimationFrame;
    requestAnimationFrame = function () { return 17; };
    cancelAnimationFrame = function () { return 19; };
    check(window.requestAnimationFrame() === 17 && self.cancelAnimationFrame() === 19, 'identifier writes');
    window.requestAnimationFrame = frame;
    window.cancelAnimationFrame = cancel;
    check(requestAnimationFrame === frame && cancelAnimationFrame === cancel, 'identifier restoration');
    var catches = 0;
    Object.defineProperty(window, 'requestAnimationFrame', {value: 7, writable: false, configurable: false});
    requestAnimationFrame = 8;
    check(requestAnimationFrame === 7, 'sloppy readonly');
    try { (function () { 'use strict'; requestAnimationFrame = 8; })(); } catch (e) { if (e.name !== 'TypeError') throw e; catches++; }
    try { requestAnimationFrame(function () {}); } catch (e) { if (e.name !== 'TypeError') throw e; catches++; }
    check(!(delete window.requestAnimationFrame), 'nonconfigurable delete');
    try { (function () { 'use strict'; delete window.requestAnimationFrame; })(); } catch (e) { if (e.name !== 'TypeError') throw e; catches++; }
    check(catches === 3 && cancelAnimationFrame === cancel, 'readonly errors');
    return 'animation-properties-ok';
})();
