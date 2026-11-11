#!/usr/bin/env python3
"""Original frozen 0.41 URL/query comparisons, separate from the legacy suite.
Node is a development oracle only; no browser network request is made.
--freeze creates expectations before implementation; normal runs never rewrite them.
"""
import argparse, hashlib, importlib.util, json, pathlib, subprocess, tempfile, time
HERE = pathlib.Path(__file__).resolve().parent
CASES = [
('url-components', "var u=new URL('HTTPS://User:Pass@EXAMPLE.org:443/a/../b c?x=hello world#two words');test.record([u.href,u.origin,u.protocol,u.username,u.password,u.host,u.hostname,u.port,u.pathname,u.search,u.hash].join('|'))"),
('url-relative', "var b='https://example.org/a/b?old=yes#old';for(var s of ['next','../up','./','/root','?x=2','#part','','../../..','//other.test:8443/p'])test.record(new URL(s,b).href)"),
('url-same-scheme', "for(var s of ['https:next','https:/next','https://next','http:next','https:'])test.record(new URL(s,'https://example.org/a/b').href)"),
('url-dot-encoding', "for(var p of ['/a/%2e/b','/a/.%2E/c','/a/%2E%2e/','/a//b/../','/././','/a/.../b'])test.record(new URL(p,'https://example.org').href)"),
('url-hosts', "for(var s of ['http://127.0.0.1:80','https://[2001:0DB8:0:0:0:0:0:1]:443/a','https://exam%70le.org./']){var u=new URL(s);test.record(u.href);test.record(u.hostname)}"),
('url-ipv4-forms', "for(var s of ['http://127.1','http://0x7f000001','http://0177.0.0.1','http://2130706433'])test.record(new URL(s).href)"),
('url-normalize-input', r"test.record(new URL(' \tHTTPS:\\example.org\\a\\..\\b\r\n?x=1#h ').href)"),
('url-userinfo', "var u=new URL('https://one@two:p:a@EXAMPLE.org/');test.record(u.href);u.username='a@/: b';u.password='c:@/ ';test.record(u.href);test.record(u.origin)"),
('url-empty-query-hash', "var u=new URL('https://example.org/?#');test.record([u.href,u.search,u.hash].join('|'));u.search='';test.record(u.href);u.hash='';test.record(u.href);u.search='?';u.hash='#';test.record(u.href);u.searchParams.sort();test.record(u.href)"),
('url-setters', "var u=new URL('https://example.org:8080/a?q=old#old');u.protocol='http:';u.hostname='OTHER.test';u.port='80';u.pathname='x y/../z?x#y';u.search='a=hello world&x=~';u.hash='a b#c';test.record(u.href);test.record(u.searchParams.get('a'))"),
('url-host-setters', "var u=new URL('https://example.org:8080/');u.host='other.test';test.record(u.href);u.host='third.test:443';test.record(u.href);u.hostname='[2001:db8::2]';test.record(u.href);u.hostname='bad:name';test.record(u.href);u.port='123abc';test.record(u.href);u.port='999999';test.record(u.href)"),
('url-href-live', "var u=new URL('https://example.org/?a=1&a=2'),p=u.searchParams;u.href='http://other.test/b?b=3#p';test.record(p===u.searchParams);test.record(p.toString());p.append('c','a b');test.record(u.href);u.search='?z=4';test.record(p.toString())"),
('url-live-canonicalization', "var u=new URL('https://example.org/?b=two%20words&a=~'),p=u.searchParams;test.record(u.href);p.sort();test.record(u.href);p.delete('a');test.record(u.href);p.delete('b');test.record(u.href)"),
('url-string-json', "var u=new URL('https://example.org/a?x=1#f');test.record(String(u));test.record(u.toString());test.record(u.toJSON());test.record(JSON.stringify({u:u}))"),
('url-invalid', "for(var s of ['','/relative','https://','https://bad host/','https://x:65536','https://[::bad::]','https://x%23y/']){try{new URL(s);test.record('bad')}catch(e){test.record(e.name)}}"),
('url-static', "test.record(URL.canParse('../a','https://example.org/x'));test.record(URL.canParse('bad'));test.record(URL.parse('bad'));test.record(URL.parse('/next','https://example.org').href);try{URL.canParse()}catch(e){test.record(e.name)}"),
('url-convert-order', "var log='',a={toString(){log+='a';return '/x'}},b={toString(){log+='b';return 'https://example.org'}};test.record(new URL(a,b).href);test.record(log);try{new URL({toString(){throw 8}},b)}catch(e){test.record(e)}test.record(log)"),
('url-brand-errors', "var d=Object.getOwnPropertyDescriptor(URL.prototype,'href');try{d.get.call({})}catch(e){test.record(e.name)}try{d.set.call({},'https://x.test')}catch(e){test.record(e.name)}try{URL('https://x.test')}catch(e){test.record(e.name)}test.record(URL.name);test.record(URL.length);test.record(d.enumerable);test.record(d.configurable)"),
('params-empty-input', "for(var v of [undefined,null,1,true,'','?','??a=1'])test.record(new URLSearchParams(v).toString())"),
('params-parse', "var p=new URLSearchParams('?a=1&&a=2&bare&=empty&x=a=b&plus=a+b&encoded=%26%3D');test.record(p.toString());test.record(p.size);test.record(p.getAll('a').join('|'));test.record(p.get('missing'));test.record(p.get('x'))"),
('params-encoding', r"var p=new URLSearchParams();p.append('a b+','é😀~!*()');p.append('line','a\r\nb');test.record(p.toString());test.record(new URLSearchParams(p.toString()).get('a b+'))"),
('params-malformed-utf8', "for(var s of ['x=%','x=%GG','x=%FF','x=%E2%82','x=%E2%28%A1','x=%EF%BB%BF','x=%00','x=%C0%AF']){var p=new URLSearchParams(s);test.record(p.get('x'));test.record(p.toString())}"),
('params-record', "var proto={inherited:'no'},o=Object.create(proto);o.z=3;o['2']='two';o['1']='one';Object.defineProperty(o,'hidden',{value:7});o.a=null;o.b=undefined;test.record(new URLSearchParams(o).toString())"),
('params-record-getters', "var log='',o={get a(){log+='a';return {toString(){log+='A';return 'one'}}},get b(){log+='b';return 'two'}};test.record(new URLSearchParams(o).toString());test.record(log)"),
('params-iterable', "var p=new URLSearchParams([['x',1],['x',2],['y',false]]);test.record(p.toString());test.record(new URLSearchParams(p).toString());test.record(new URLSearchParams(new Map([['a','b'],['c','d']])).toString())"),
('params-bad-pairs', "for(var x of [[[1]],[[1,2,3]],[null],['ab']]){try{new URLSearchParams(x);test.record('bad')}catch(e){test.record(e.name)}}"),
('params-get-set', "var p=new URLSearchParams('a=1&b=2&a=3&c=4');test.record(p.set('a','x'));test.record(p.toString());p.set('z','y');test.record(p.toString());test.record(p.getAll('none').length);test.record(p.has('b'));test.record(p.has('b','2'));test.record(p.has('b','3'));test.record(p.has('b',undefined))"),
('params-delete-value', "var p=new URLSearchParams('a=1&a=2&a=1&b=3');test.record(p.delete('a','1'));test.record(p.toString());p.delete('a',undefined);test.record(p.toString());p.delete('missing');test.record(p.toString())"),
('params-stable-sort', "var p=new URLSearchParams([['z','0'],['a','1'],['a','2'],['😀','3'],['\uE000','4'],['é','5'],['A','6']]);test.record(p.sort());test.record(p.toString());test.record(Array.from(p.keys()).join('|'))"),
('params-iterators', "var p=new URLSearchParams('a=1&b=2');test.record(p.entries===p[Symbol.iterator]);test.record(Array.from(p).map(x=>x.join(':')).join('|'));test.record(Array.from(p.keys()).join('|'));test.record(Array.from(p.values()).join('|'));var i=p.entries();test.record(i[Symbol.iterator]()===i);test.record(i.next().value.join(':'));test.record(i.next().value.join(':'));test.record(i.next().done);p.append('c','3');test.record(i.next().done)"),
('params-live-iterator', "var p=new URLSearchParams('a=1&b=2'),i=p.entries();test.record(i.next().value.join(':'));p.delete('a');p.append('c','3');test.record(i.next().value.join(':'));test.record(i.next().done)"),
('params-foreach', "var p=new URLSearchParams('a=1&b=2'),receiver={},log='';p.forEach(function(v,k,o){log+=k+v+(o===p)+(this===receiver);if(k==='a')p.append('c','3')},receiver);test.record(log)"),
('params-coercion-mutation', "var p=new URLSearchParams('a=1');p.set({toString(){p.append('b','2');return 'a'}},{toString(){p.append('c','3');return 'x'}});test.record(p.toString())"),
('params-frozen', "var p=new URLSearchParams('a=1');Object.freeze(p);p.append('b','2');test.record(p.toString());var u=new URL('https://example.org/');Object.freeze(u);u.search='?q=yes';test.record(u.href)"),
('params-brands', "try{URLSearchParams()}catch(e){test.record(e.name)}try{URLSearchParams.prototype.get.call({},'x')}catch(e){test.record(e.name)}try{Object.getOwnPropertyDescriptor(URLSearchParams.prototype,'size').get.call({})}catch(e){test.record(e.name)}test.record(URLSearchParams.length);test.record(URLSearchParams.prototype.append.length);test.record(URLSearchParams.prototype.delete.length)"),
('params-required', "var p=new URLSearchParams;for(var name of ['append','set','get','getAll','has','delete']){try{p[name]();test.record('bad')}catch(e){test.record(e.name)}}try{p.append('a')}catch(e){test.record(e.name)}try{p.forEach(1)}catch(e){test.record(e.name)}"),
('params-symbol-values', "var p=new URLSearchParams;try{p.append(Symbol(),'a')}catch(e){test.record(e.name)}try{new URLSearchParams({a:Symbol()})}catch(e){test.record(e.name)}test.record(p.size)"),
('params-coercion-throw', "var p=new URLSearchParams('a=1'),o={toString(){throw 23}};try{p.append('b',o)}catch(e){test.record(e)}test.record(p.toString())"),
('url-invalid-setter-atomic', "var u=new URL('https://example.org/a?x=1');u.hostname='bad host';u.port='-1';test.record(u.href);try{u.href='/relative'}catch(e){test.record(e.name)}test.record(u.href)"),
('url-query-followup', "var u=new URL('/search?q=first','https://example.org/'),p=u.searchParams;p.set('q','second term');p.append('start','10');var next=new URL('?q=third',u);test.record(u.href);test.record(next.href);test.record(next.searchParams.get('q'))"),
]
BOUNDS = [
('url-scheme-boundary', "new URL('data:text/plain,hello')", 'only HTTP(S)'),
('url-idna-boundary', "new URL('https://é.test/')", 'IDNA'),
('url-source-bound', "new URL('https://example.org/'+ 'x'.repeat(16384))", 'URL length limit'),
('params-source-bound', "new URLSearchParams('x'.repeat(65537))", 'URLSearchParams byte limit'),
('params-entry-bound', "new URLSearchParams('a=1&'.repeat(1025))", 'URLSearchParams entry limit'),
('url-attached-query-bound', "var u=new URL('https://example.org/');u.searchParams.set('x','a'.repeat(16384))", 'URL length limit'),
('params-recursive-coercion', "var p=new URLSearchParams,o={toString(){return p.get(o)}};p.append(o,'x')", 'limit'),
]

def digest(data): return hashlib.sha256(data).hexdigest()
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--root',type=pathlib.Path,default=HERE.parent);ap.add_argument('--freeze',action='store_true');ap.add_argument('--output',type=pathlib.Path,required=True);ap.add_argument('--expect',type=pathlib.Path,default=HERE/'url-041'/'frozen.json');ap.add_argument('--skip-bounds',action='store_true');ap.add_argument('--original-bounds',action='store_true');a=ap.parse_args()
    spec=importlib.util.spec_from_file_location('helper',a.root/'tools'/'check_javascript.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
    frozen={'schema':1,'fixture_sources_sha256':digest(json.dumps(CASES,ensure_ascii=False).encode()),'bounds_sources_sha256':digest(json.dumps(BOUNDS,ensure_ascii=False).encode()),'helper_sha256':digest((a.root/'tools'/'check_javascript.py').read_bytes()),'ordinary':[],'bounds':[{'name':n,'source':s,'diagnostic':d} for n,s,d in BOUNDS]}
    if a.freeze:
        if a.expect.exists(): raise SystemExit('Refusing to overwrite frozen expectations')
        for name,source in CASES:
            n=subprocess.run(['node','-e',m.NODE_DRIVER],input=source,text=True,capture_output=True,timeout=10,check=True);v=json.loads(n.stdout)
            if not v['ok']: raise SystemExit(f'Node failed {name}: {v}')
            frozen['ordinary'].append({'name':name,'source':source,'node':v})
        a.expect.parent.mkdir(parents=True,exist_ok=True);a.expect.write_text(json.dumps(frozen,indent=2,ensure_ascii=False)+'\n')
    expected=json.loads(a.expect.read_text())
    if expected['fixture_sources_sha256']!=frozen['fixture_sources_sha256'] or expected['bounds_sources_sha256']!=frozen['bounds_sources_sha256']: raise SystemExit('Frozen fixture source mismatch')
    bounds=BOUNDS
    if not a.original_bounds and (a.expect.parent/'bounds-corrected.json').exists():
        corrected=json.loads((a.expect.parent/'bounds-corrected.json').read_text());bounds=[(c['name'],c['source'],c['diagnostic']) for c in corrected['bounds']]
    output={'frozen_sha256':digest(a.expect.read_bytes()),'source_sha256':digest((a.root/'src'/'javascript.rs').read_bytes()),'ordinary':[],'bounds':[]}
    with tempfile.TemporaryDirectory(prefix='chezcat-url-041-') as t:
        p=pathlib.Path(t);rs=p/'probe.rs';exe=p/'probe';rs.write_text(m.PROBE.replace('JAVASCRIPT_SOURCE',json.dumps(str((a.root/'src'/'javascript.rs').resolve()))));subprocess.run(['rustc','--edition=2021','-O',str(rs),'-o',str(exe)],check=True,timeout=180)
        for case in expected['ordinary']:
            r=subprocess.run([str(exe)],input=case['source'],text=True,capture_output=True,timeout=10,check=True);lines=r.stdout.splitlines();records=[l.split('\t',1)[1] for l in lines if l.startswith('RECORD\t')];head=lines[0] if lines else '';match=head=='OK\t'+case['node']['value'] and records==case['node']['records'];output['ordinary'].append({'name':case['name'],'match':match,'actual':lines,'expected':case['node']})
        if not a.skip_bounds:
            for name,source,diagnostic in bounds:
                r=subprocess.run([str(exe)],input=source,text=True,capture_output=True,timeout=10,check=True);output['bounds'].append({'name':name,'pass':r.stdout.startswith('ERROR\t') and diagnostic in r.stdout,'actual':r.stdout.strip()})
    output['ordinary_matching']=sum(x['match'] for x in output['ordinary']);output['ordinary_total']=len(output['ordinary']);output['bounds_passing']=sum(x['pass'] for x in output['bounds']);output['bounds_total']=len(output['bounds']);output['bound_sources']='original' if a.original_bounds else 'bounds-corrected.json';a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(output,indent=2,ensure_ascii=False)+'\n');print(json.dumps({k:v for k,v in output.items() if not isinstance(v,list)}));print('Failures:',', '.join(x['name'] for x in output['ordinary'] if not x['match']),', '.join(x['name'] for x in output['bounds'] if not x['pass']))
if __name__=='__main__':main()
