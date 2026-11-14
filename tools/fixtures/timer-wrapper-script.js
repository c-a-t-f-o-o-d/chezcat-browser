var timerOrder = 'script', timerToken = { count: 0 };
var savedSetTimeout = setTimeout, savedClearTimeout = clearTimeout;
window.setTimeout = function () {
    timerOrder += ':wrap';
    return savedSetTimeout.apply(window, arguments);
};
clearTimeout = function (id) {
    timerOrder += ':cancel';
    return savedClearTimeout.call(window, id);
};
var cancelledTimer = self.setTimeout(function () { throw Error('cancelled timer ran'); }, 0);
globalThis.clearTimeout(cancelledTimer);
setTimeout(function (object, label) {
    if (this !== window || object !== timerToken || label !== 'payload') throw Error('timer identity');
    object.count++;
    timerOrder += ':timer';
    Promise.resolve().then(function () {
        timerOrder += ':microtask';
        document.title = timerOrder + ':' + object.count;
        document.getElementById('out').textContent = document.title;
    });
}, 0, timerToken, 'payload');
// A saved original remains callable after both public slots are removed.
delete window.setTimeout;
delete self.clearTimeout;
if (typeof setTimeout !== 'undefined' || typeof clearTimeout !== 'undefined') throw Error('hidden timer binding');
var savedBoundSet = savedSetTimeout.bind(window);
var anotherCancelledTimer = savedBoundSet(function () { throw Error('saved cancellation failed'); }, 0);
savedClearTimeout.apply(window, [anotherCancelledTimer]);
