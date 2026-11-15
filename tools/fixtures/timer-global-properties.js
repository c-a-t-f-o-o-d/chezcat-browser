(function () {
    function check(ok, message) { if (!ok) throw Error(message); }
    var originalSet = setTimeout, originalClear = clearTimeout;
    check(window === self && self === globalThis, 'global aliases');
    check(originalSet === window.setTimeout && originalSet === self.setTimeout &&
          originalSet === globalThis.setTimeout, 'set aliases');
    check(originalClear === window.clearTimeout && originalClear === self.clearTimeout,
          'clear aliases');
    var descriptor = Object.getOwnPropertyDescriptor(window, 'setTimeout');
    check(descriptor.value === originalSet && descriptor.writable && descriptor.enumerable &&
          descriptor.configurable && !('get' in descriptor), 'initial descriptor');
    check(Object.hasOwn(window, 'setTimeout') &&
          Object.prototype.propertyIsEnumerable.call(window, 'setTimeout'), 'own predicates');
    var calls = 0;
    var replacement = function (value) { calls++; return value; };
    window.setTimeout = replacement;
    check(setTimeout === replacement && self.setTimeout === replacement &&
          globalThis.setTimeout === replacement, 'window assignment aliases');
    check(setTimeout(11) === 11 && calls === 1, 'identifier replacement called');
    check(window['setTimeout'](14) === 14 && (self?.setTimeout)(15) === 15 &&
          globalThis.setTimeout?.(16) === 16, 'computed and optional calls');
    setTimeout = originalSet;
    check(window.setTimeout === originalSet, 'identifier assignment aliases');
    var reads = 0, writes = 0, last;
    Object.defineProperty(window, 'setTimeout', {
        get: function () { check(this === window, 'getter receiver'); reads++; return replacement; },
        set: function (value) { check(this === window, 'setter receiver'); writes++; last = value; },
        configurable: true
    });
    check('setTimeout' in window, 'presence');
    check(Object.hasOwn(window, 'setTimeout'), 'own accessor');
    check(typeof Object.getOwnPropertyDescriptor(window, 'setTimeout').get === 'function', 'descriptor');
    check(reads === 0, 'inspection must not invoke getter');
    check(setTimeout(12) === 12 && reads === 1, 'identifier accessor');
    check(window.setTimeout(13) === 13 && reads === 2, 'member accessor');
    setTimeout = originalSet;
    check(writes === 1 && last === originalSet && reads === 2, 'identifier setter');
    Object.defineProperty(window, 'setTimeout', { value: originalSet, writable: true,
        enumerable: true, configurable: true });
    check(delete window.setTimeout, 'delete configurable operation');
    check(typeof setTimeout === 'undefined' && window.setTimeout === undefined &&
          !('setTimeout' in self) && !Object.hasOwn(globalThis, 'setTimeout'), 'no resurrection');
    check(Object.getOwnPropertyDescriptor(window, 'setTimeout') === undefined, 'deleted descriptor');
    window.setTimeout = originalSet;
    clearTimeout = replacement;
    check(window.clearTimeout === replacement && self.clearTimeout === replacement, 'clear replacement');
    clearTimeout = originalClear;
    check((function (setTimeout) { return setTimeout(8); })(replacement) === 8, 'local shadow');
    check(setTimeout === originalSet && clearTimeout === originalClear, 'restore');
    return 'ok';
})();
