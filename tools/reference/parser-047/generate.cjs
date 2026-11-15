// Development only. Independent Acorn parser supplies frozen AST expectations
// for original locally generated expressions; no reference implementation code
// or runtime dependency is copied into ChezCat. Native tests read the TSV only.
const fs = require('fs');
const path = require('path');
const vm = require('vm');
const crypto = require('crypto');
const acorn = require(process.argv[2] || 'acorn');
const out = process.argv[3] || __dirname;
fs.mkdirSync(out, {recursive: true});
const args = values => values.map(x => x.type === 'SpreadElement' ? '...'+normal(x.argument) : normal(x)).join(',');
function normal(n) {
  switch(n.type) {
    case 'ParenthesizedExpression': return normal(n.expression);
    case 'Identifier': return `n(${n.name})`;
    case 'ThisExpression': return 'this';
    case 'Literal': return `${typeof n.value === 'string' ? 's' : 'v'}(${String(n.value)})`;
    case 'UnaryExpression': return `u(${n.operator},${normal(n.argument)})`;
    case 'UpdateExpression': return `update(${n.operator},${n.prefix},${normal(n.argument)})`;
    case 'BinaryExpression': case 'LogicalExpression': return `b(${n.operator},${normal(n.left)},${normal(n.right)})`;
    case 'AssignmentExpression': return `a(${n.operator},${normal(n.left)},${normal(n.right)})`;
    case 'ConditionalExpression': return `q(${normal(n.test)},${normal(n.consequent)},${normal(n.alternate)})`;
    case 'SequenceExpression': return n.expressions.map(normal).reduce((a,b)=>`sqr(${a},${b})`);
    case 'MemberExpression': return `m(${normal(n.object)},${n.computed ? normal(n.property) : 's('+n.property.name+')'})`;
    case 'CallExpression': return `c(${normal(n.callee)},[${args(n.arguments)}])`;
    case 'NewExpression': return `new(${normal(n.callee)},[${args(n.arguments)}])`;
    case 'ChainExpression': {
      function collect(x) {
        if(x.type === 'MemberExpression') { const p=collect(x.object); p.steps.push({type:'m',optional:x.optional,value:x.computed?normal(x.property):'s('+x.property.name+')'}); return p; }
        if(x.type === 'CallExpression') { const p=collect(x.callee); p.steps.push({type:'c',optional:x.optional,value:'['+args(x.arguments)+']'}); return p; }
        return {base:normal(x), steps:[]};
      }
      const p=collect(n.expression);
      const split=p.steps.findIndex(s=>s.optional);
      if(split<0) throw new Error('chain without optional step');
      for(const s of p.steps.slice(0,split)) p.base=`${s.type}(${p.base},${s.value})`;
      return `chain(${p.base},[${p.steps.slice(split).map(s=>`${s.type}(${s.optional},${s.value})`).join(',')}])`;
    }
    default: throw new Error('unexpected oracle node '+n.type);
  }
}
const sources = new Set();
const add = x => sources.add(x+';');
const binary=['+','-','*','/','%','**','<<','>>','>>>','<','<=','>','>=','in','instanceof','==','!=','===','!==','&','^','|','&&','||','??'];
for(const a of binary) for(const b of binary) add(`a ${a} b ${b} c`);
for(const lhs of ['a','obj.x','obj[(a,b)]','((a))']) for(const op of ['=','+=','-=','*=','/=','%=','**=','<<=','>>=','>>>=','&=','|=','^=','&&=','||=','??=']) for(const rhs of ['b=c','b?c:d','f((b+c),...args)','b**c**d']) add(`${lhs} ${op} ${rhs}`);
for(const x of [
 'a?b:c?d:e','a?b?c:d:e','a=b?c:d=e','a,b=c?d:e,f',
 'f((a,b),c,)','f(...a,)','f((obj.x))(a,b)','((obj.method))(x)',
 '(0,obj.method)(x)','(obj?.method)(x)','(obj.method)?.(x)',
 'a?.b.c?.(d)[e]','a.b(c)?.d[e]?.(f(...g))','(a?.b).c?.d',
 '(a?.b)?.c','a?.[(b,c)]?.(d)','new f().g(a)','new new f()()',
 'new (a?.b)(c).d','typeof a?.b','delete (a?.b)','typeof (a+b)',
 '++a**b','a++**b','a**-b','(-a)**b','new f[a+b](c,d)',
 'a?.b(c)?.d((e,f),...g)','((a+b)*c)**d','(a??b)||c','a??(b&&c)',
 'a[(b=c,d)](e?f:g)','a?b=c,d:e','(a?b:c),d','a?b:(c,d)',
 '(a,b)[c]','a[(b,c)]','a?.b?.c?.d','a(...b)(c,...d)',
 'new f', 'new f.g', 'new (f(a))', 'new f().g.h',
]) add(x);
let seed=47;
const rng=n=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed%n;};
const atoms=['a','b','obj.x','1','2','3','true','false','null','undefined','this'];
function expression(depth) {
 if(!depth) return atoms[rng(atoms.length)];
 switch(rng(9)) {
 case 0:return `(${expression(depth-1)})`;
 case 1:return `f(${expression(depth-1)},${expression(depth-1)})`;
 case 2:return `obj[(${expression(depth-1)})]`;
 case 3:return `(${expression(depth-1)}) ${binary[rng(binary.length)]} (${expression(depth-1)})`;
 case 4:return `(${expression(depth-1)})?(${expression(depth-1)}):(${expression(depth-1)})`;
 case 5:return `a=(${expression(depth-1)})`;
 case 6:return `(${expression(depth-1)},${expression(depth-1)})`;
 case 7:return `obj?.[(${expression(depth-1)})]?.((${expression(depth-1)}))`;
 default:return `!(${expression(depth-1)})`;
 }
}
for(let i=0;i<512;i++) add(expression(3));
const accepted=[], rejected=[];
for(const source of sources) {
 try {
  new vm.Script(source); // Independent native Node syntax check as well.
  const ast=acorn.parse(source,{ecmaVersion:'latest',preserveParens:true});
  if(ast.body.length!==1||ast.body[0].type!=='ExpressionStatement') throw new Error('not one expression');
  accepted.push([source,normal(ast.body[0].expression)]);
 } catch(error) { rejected.push({source,name:error.name,message:error.message}); }
}
const text='# Original expression corpus; AST oracle: Acorn '+acorn.version+'; Node '+process.version+'\n'+accepted.map(x=>x.join('\t')).join('\n')+'\n';
fs.writeFileSync(path.join(out,'ast-witnesses.tsv'),text);
fs.writeFileSync(path.join(out,'provenance.json'),JSON.stringify({schema_version:1,scope:'Original syntax corpus; no ChezCat execution',node:process.version,acorn:acorn.version,seed:47,accepted:accepted.length,rejected:rejected.length,sha256:crypto.createHash('sha256').update(text).digest('hex'),sources:['https://tc39.es/ecma262/multipage/ecmascript-language-expressions.html','https://github.com/acornjs/acorn'],rejected_cases:rejected},null,2)+'\n');
console.log(JSON.stringify({accepted:accepted.length,rejected:rejected.length,node:process.version,acorn:acorn.version}));
