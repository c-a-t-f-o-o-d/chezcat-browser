// Original early-error fixture. Every rejected program must fail before effects.
var seen = 0, errors = 0;
for (var caseSource of [
  'f((1,))', 'f(,1)', 'f(1,,2)', '(a + )', '(a]', 'a[(b,c)',
  'a?.x = 1', 'a?.x++', '++a?.x', 'new a?.()',
  'a ?? b || c', 'a || b ?? c', 'a ?? b && c', 'a && b ?? c',
  '-2 ** 2', '2 ** -3 ** 2', '(a + b) = 1', '1++',
  '(a,b) => { "use strict"; var a; let a; }',
  'function f(a,a){ "use strict"; }',
  'function f(a=1){ "use strict"; }',
  'function strictDelete(){"use strict"; delete (x);}',
  'function strictAssign(){"use strict"; arguments = 1;}'
]) {
  try { eval('seen++;\n' + caseSource); } catch (error) {
    if (error.name !== 'SyntaxError') throw error;
    errors++;
  }
  if (seen !== 0) throw 'effects before syntax rejection';
}
if (errors !== 23) throw 'early error count';
'ok';
