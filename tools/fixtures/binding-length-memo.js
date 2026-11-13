(function () {
    var text = 'ABCD', alias = text;
    if (text.length !== 4 || text.length !== 4) throw 'cold/warm';
    text = '😀';
    if (text.length !== 2 || alias.length !== 4) throw 'equal byte replacement';
    text = ''; if (text.length !== 0 || text.length !== 0) throw 'empty';
    function mapped(value) {
        var before = value.length;
        arguments[0] = '😀';
        return before === 4 && value.length === 2;
    }
    if (!mapped('ABCD')) throw 'mapped arguments';
    function make() {
        var value = 'ABCD';
        return {read: function () { return value.length; },
                write: function (next) { value = next; }};
    }
    var box = make();
    if (box.read() !== 4 || box.read() !== 4) throw 'closure read';
    box.write('é'); if (box.read() !== 1) throw 'closure replacement';
    function local(value) { return value.length + value.length; }
    for (var i = 0; i < 100; i++) {
        if (local(i % 2 ? '😀' : 'ABCD') !== (i % 2 ? 4 : 8)) throw 'reused environment';
    }
    text = 'ABCD';
    var old = text[{toString: function () { text = '😀'; return 'length'; }}];
    if (old !== 4 || text.length !== 2) throw 'receiver before key';
    var count = 0, object = {get length() { count++; return count; }};
    if (object.length !== 1 || object.length !== 2 || count !== 2) throw 'getter';
    var missing = null;
    if (missing?.length !== undefined) throw 'optional';
    return 'ok';
})();
