// Original environment-lifetime fixture. No downloaded site code.
var saved, getter, capturedThrow, mapped, capturedDefault;
{
    let value = 10;
    saved = function () { value += 1; return value; };
    getter = {get read() { return value; }};
}
var n = 0;
while (n < 128) { let scratch = n; { const next = scratch + 1; n = next; } }
if (n !== 128 || saved() !== 11 || getter.read !== 11) throw 'block capture';
var closures = [];
for (let i = 0; i < 4; i++) { closures.push(function () { return i; }); }
if (closures[0]() !== 0 || closures[3]() !== 3) throw 'iteration capture';
try { let answer = 42; throw function () { return answer; }; }
catch (e) { capturedThrow = e; }
if (capturedThrow() !== 42) throw 'thrown closure';
function makeArguments(a) { return arguments; }
mapped = makeArguments(7);
mapped[0] = 9;
if (mapped[0] !== 9) throw 'mapped arguments';
function makeDefault(a = 3, next = function () { return ++a; }) { var a = 99; return next; }
capturedDefault = makeDefault();
if (capturedDefault() !== 4 || capturedDefault() !== 5) throw 'parameter capture';
var fromEval;
{ let outer = 20; eval('let inner=2; fromEval=function(){return outer+inner;};'); }
if (fromEval() !== 22) throw 'eval capture';
function bodyReturn(a) { { let local = a; return function () { return local; }; } }
if (bodyReturn(13)() !== 13) throw 'return through block';
var caught = 0, final = 0;
for (var j = 0; j < 128; j++) {
    try { let temporary = j; throw 1; }
    catch (e) { caught += e; }
    finally { final++; }
}
if (caught !== 128 || final !== 128 || saved() !== 12) throw 'catch cleanup';
'ok';
