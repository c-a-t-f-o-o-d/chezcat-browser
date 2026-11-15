(function () {
    var values = [];
    for (var i = 0; i < 12000; i++) values[i] = i & 255;
    var keys = Object.keys(values);
    var selected = values.slice(8190, 8210);
    var copied = Array.from(values);
    var sparse = [];
    sparse.length = 100000;
    sparse[99999] = 9;
    var names = Object.getOwnPropertyNames(sparse);
    if (values.length !== 12000 || values[11999] !== (11999 & 255) ||
        keys.length !== 12000 || keys[11999] !== '11999' ||
        selected.length !== 20 || selected[0] !== (8190 & 255) ||
        copied.length !== 12000 || copied[11999] !== values[11999] ||
        sparse.length !== 100000 || names.join(',') !== '99999,length') {
        throw new Error('heap-capacity values');
    }
    sparse.length = 0;
    if (sparse.hasOwnProperty(99999)) throw new Error('heap-capacity shrink');
    return 'ok';
}());
