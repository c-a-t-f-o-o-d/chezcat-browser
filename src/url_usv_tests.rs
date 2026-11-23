//! Original USVString regressions for the finite URL and query APIs.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("URL conversion must not read a native object")
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("URL conversion must not write a native object")
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("URL conversion must not call a native object")
    }
}

fn succeeds(source: &str) {
    let mut runtime = Runtime::for_test();
    assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_text(), "ok");
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn url_usv_sequence_conversion_preserves_pairs_and_replaces_each_isolated_unit() {
    succeeds(r#"
var cases=[
    ['\ud800','\ufffd'],['\udbff','\ufffd'],
    ['\udc00','\ufffd'],['\udfff','\ufffd'],
    ['\ud800\udc00','\ud800\udc00'],['\udbff\udfff','\udbff\udfff'],
    ['\ud83d\ude00','😀'],['\udc00\ud800','\ufffd\ufffd'],
    ['\ud800\ud801','\ufffd\ufffd'],['\udc00\udc01','\ufffd\ufffd'],
    ['\ud800A\udc00','\ufffdA\ufffd'],
    ['\ud800\ud800\udc00','\ufffd\ud800\udc00'],
    ['\ud800\udc00\udfff','\ud800\udc00\ufffd'],
    ['A\0\ud800B','A\0\ufffdB'],['\ufffd','\ufffd'],['plain','plain']
];
for(var i=0;i<cases.length;i++){
    var input=cases[i][0],expected=cases[i][1];
    var query=new URLSearchParams([[input,input]]);
    if(query.size!==1||query.keys().next().value!==expected||
        query.values().next().value!==expected||query.get(input)!==expected||
        query.get(expected)!==expected)throw 'USVString code-unit pattern';
}
'ok';
"#);
}

#[test]
fn url_usv_constructor_and_static_operations_convert_input_and_base() {
    succeeds(r#"
var input='child\ud800?q=\udfff#\ud800';
var base='https://example.org/\udfff/';
var expected='https://example.org/%EF%BF%BD/child%EF%BF%BD?q=%EF%BF%BD#%EF%BF%BD';
if(new URL(input,base).href!==expected||URL.parse(input,base).href!==expected||
    !URL.canParse(input,base))throw 'input and base USVString';
if(new URL('https://example.org/\ud800').pathname!=='/%EF%BF%BD'||
    URL.parse('https://example.org/\udfff').pathname!=='/%EF%BF%BD'||
    !URL.canParse('https://example.org/\ud800'))throw 'absolute URL conversion';
if(URL.parse('\ud800')!==null||URL.canParse('\ud800'))throw 'relative URL still needs base';
var rejected=false;
try{new URL('\ud800');}catch(error){rejected=error.name==='TypeError';}
if(!rejected)throw 'constructor malformed URL';
'ok';
"#);
}

#[test]
fn url_usv_constructor_conversion_is_once_only_ordered_and_uses_live_base() {
    succeeds(r#"
var operations=[function(input,base){return new URL(input,base);},
                function(input,base){return URL.parse(input,base);},
                function(input,base){return URL.canParse(input,base);}];
for(var i=0;i<operations.length;i++){
    var trace='',input={},base={path:'https://wrong.test/'},inputCalls=0,baseCalls=0;
    input[Symbol.toPrimitive]=function(hint){
        if(this!==input||hint!=='string')throw 'input hint or receiver';
        inputCalls++;trace+='I';base.path='https://example.org/\udfff/';return 'x\ud800';
    };
    base[Symbol.toPrimitive]=function(hint){
        if(this!==base||hint!=='string')throw 'base hint or receiver';
        baseCalls++;trace+='B';return this.path;
    };
    var result=operations[i](input,base);
    if(trace!=='IB'||inputCalls!==1||baseCalls!==1)throw 'input/base conversion order';
    if(i===2?result!==true:result.href!=='https://example.org/%EF%BF%BD/x%EF%BF%BD')
        throw 'base changed before its conversion';
}
'ok';
"#);
}

#[test]
fn url_usv_component_setters_preserve_query_identity_and_percent_encoding() {
    succeeds(r#"
var url=new URL('https://example.org/old?old=1'),query=url.searchParams;
url.username='\ud800';url.password='\udfff';url.pathname='/A\ud800😀';
url.search='q=\udfff';url.hash='f\ud800';
if(url.username!=='%EF%BF%BD'||url.password!=='%EF%BF%BD'||
    url.pathname!=='/A%EF%BF%BD%F0%9F%98%80'||url.search!=='?q=%EF%BF%BD'||
    url.hash!=='#f%EF%BF%BD'||query!==url.searchParams||query.get('q')!=='\ufffd')
    throw 'component conversion or query identity';
url.href='http://example.net/\udfff?\ud800=\udfff';
if(url.href!=='http://example.net/%EF%BF%BD?%EF%BF%BD=%EF%BF%BD'||
    query!==url.searchParams||query.get('\ud800')!=='\ufffd')throw 'href replacement';
query.append('extra','\ud800');
if(url.search!=='?%EF%BF%BD=%EF%BF%BD&extra=%EF%BF%BD')throw 'attached query update';
'ok';
"#);
}

#[test]
fn url_usv_setter_conversion_reads_current_url_after_reentry() {
    succeeds(r#"
var url=new URL('https://example.org/old?old=1'),query=url.searchParams,trace='',key={};
key[Symbol.toPrimitive]=function(hint){
    if(this!==key||hint!=='string')throw 'setter hint or receiver';
    trace+='S';url.href='https://other.test/fresh?keep=1';return '?q=\ud800';
};
url.search=key;
if(trace!=='S'||url.href!=='https://other.test/fresh?q=%EF%BF%BD'||
    query!==url.searchParams||query.get('q')!=='\ufffd'||query.has('keep'))
    throw 'setter converted against stale URL';
var path={toString:function(){trace+='P';url.hash='\udfff';return '/new\ud800';}};
url.pathname=path;
if(trace!=='SP'||url.href!=='https://other.test/new%EF%BF%BD?q=%EF%BF%BD#%EF%BF%BD'||
    query!==url.searchParams)throw 'ordinary ToString fallback and current URL';
'ok';
"#);
}

#[test]
fn url_usv_query_string_initialization_keeps_duplicates_after_replacement() {
    succeeds(r#"
var query=new URLSearchParams('?\ud800=one&\udfff=two&\ufffd=three&space=\ud800+x&pair=😀');
if(query.size!==5||query.getAll('\ufffd').join('|')!=='one|two|three'||
    query.get('space')!=='\ufffd x'||query.get('pair')!=='😀')throw 'string query values';
if(query.toString()!=='%EF%BF%BD=one&%EF%BF%BD=two&%EF%BF%BD=three&space=%EF%BF%BD+x&pair=%F0%9F%98%80')
    throw 'form encoding after USVString replacement';
if(new URLSearchParams('\0=\ud800').toString()!=='%00=%EF%BF%BD')throw 'query NUL remains data';
'ok';
"#);
}

#[test]
fn url_usv_sequence_names_and_values_convert_once_without_collapsing_pairs() {
    succeeds(r#"
var trace='';
function token(label,text){
    var object={};
    object[Symbol.toPrimitive]=function(hint){
        if(this!==object||hint!=='string')throw 'sequence hint or receiver';
        trace+=label;return text;
    };
    return object;
}
var query=new URLSearchParams([
    [token('A','\ud800'),token('B','\udfff')],
    [token('C','\ufffd'),token('D','second')],
    [token('E','\udc00'),token('F','third')]
]);
if(trace!=='ABCDEF'||query.size!==3||query.getAll('\ufffd').join('|')!=='\ufffd|second|third')
    throw 'sequence conversion order or duplicate retention';
if(query.toString()!=='%EF%BF%BD=%EF%BF%BD&%EF%BF%BD=second&%EF%BF%BD=third')
    throw 'sequence serialization';
'ok';
"#);
}

#[test]
fn url_usv_record_reads_original_keys_and_collisions_keep_first_position_last_value() {
    succeeds(r#"
var high='\ud800',low='\udfff',record={},trace='';
Object.defineProperty(record,high,{enumerable:true,get:function(){
    if(this!==record)throw 'high getter receiver';trace+='H';return 'high';
}});
Object.defineProperty(record,'middle',{enumerable:true,get:function(){trace+='M';return 'middle';}});
Object.defineProperty(record,'\ufffd',{enumerable:true,get:function(){trace+='R';return 'replacement';}});
Object.defineProperty(record,low,{enumerable:true,get:function(){
    if(this!==record)throw 'low getter receiver';trace+='L';return low;
}});
var query=new URLSearchParams(record);
if(trace!=='HMRL'||query.size!==2||query.get('\ufffd')!=='\ufffd'||
    query.toString()!=='%EF%BF%BD=%EF%BF%BD&middle=middle')throw 'converted record collision';
var only={};only[high]='raw original';
if(new URLSearchParams(only).get('\ufffd')!=='raw original')throw 'converted key used for Get';
if(Object.keys(record).length!==4||record[high]!=='high'||record['\ufffd']!=='replacement')
    throw 'record keys changed by conversion';
'ok';
"#);
}

#[test]
fn url_usv_record_uses_snapshot_keys_and_current_enumerable_descriptors() {
    succeeds(r#"
var first='first\ud800',gone='gone\ud800',made='made\udfff',hidden='hidden\ud800',added='added\udfff';
var record={},trace='';
Object.defineProperty(record,first,{enumerable:true,get:function(){
    trace+='F';delete record[gone];
    Object.defineProperty(record,made,{enumerable:true});
    Object.defineProperty(record,hidden,{enumerable:false});
    record[added]='late';return '\ud800';
}});
Object.defineProperty(record,gone,{enumerable:true,configurable:true,get:function(){throw 'deleted getter';}});
Object.defineProperty(record,made,{enumerable:false,configurable:true,get:function(){trace+='M';return '\udfff';}});
Object.defineProperty(record,hidden,{enumerable:true,configurable:true,get:function(){throw 'hidden getter';}});
Object.setPrototypeOf(record,{inherited:'not own'});
Object.defineProperty(record,Symbol('hidden'),{value:'not enumerable'});
var query=new URLSearchParams(record);
if(trace!=='FM'||query.size!==2||query.toString()!=='first%EF%BF%BD=%EF%BF%BD&made%EF%BF%BD=%EF%BF%BD'||
    query.has(added)||query.has('inherited'))throw 'record snapshot and live descriptor';
'ok';
"#);
}

#[test]
fn url_usv_query_methods_convert_names_values_and_optional_value_filters() {
    succeeds(r#"
var high='\ud800',low='\udfff',query=new URLSearchParams();
query.append(high,low);query.append('\ufffd','other');
if(query.get(high)!=='\ufffd'||query.get(low)!=='\ufffd'||
    query.getAll(high).join('|')!=='\ufffd|other'||!query.has(low,high)||
    query.has(high,'missing'))throw 'converted query lookup';
query.delete(low,high);
if(query.size!==1||query.get(high)!=='other')throw 'converted optional delete';
query.set(low,high);
if(query.size!==1||query.get('\ufffd')!=='\ufffd'||!query.has(high,undefined))throw 'converted set';
query.append(high,'second');query.delete(low,undefined);
if(query.size!==0||query.get(high)!==null||query.getAll(high).length!==0)throw 'undefined optional filter';
'ok';
"#);
}

#[test]
fn url_usv_query_method_conversion_is_ordered_once_and_observes_reentrant_slots() {
    succeeds(r#"
var query=new URLSearchParams('before=1'),trace='',name={},value={},nameCalls=0,valueCalls=0;
name[Symbol.toPrimitive]=function(hint){
    if(this!==name||hint!=='string')throw 'method name conversion';
    nameCalls++;trace+='K';query.append('side','key');return '\ud800';
};
value[Symbol.toPrimitive]=function(hint){
    if(this!==value||hint!=='string')throw 'method value conversion';
    valueCalls++;trace+='V';query.append('side','value');return '\udfff';
};
query.set(name,value);
if(trace!=='KV'||nameCalls!==1||valueCalls!==1||
    query.toString()!=='before=1&side=key&side=value&%EF%BF%BD=%EF%BF%BD')
    throw 'method conversion snapshot';
var lookup={toString:function(){trace+='G';query.set('\ufffd','fresh');return '\ud800';}};
if(query.get(lookup)!=='fresh'||trace!=='KVG')throw 'get reads after conversion';
var filter={toString:function(){trace+='F';query.set('\ufffd','\udfff');return '\ud800';}};
if(!query.has('\udfff',filter)||trace!=='KVGF')throw 'optional filter reads current slots';
'ok';
"#);
}

#[test]
fn url_usv_symbol_conversion_still_throws_without_partial_mutation() {
    succeeds(r#"
function typeError(action){
    var caught=false;try{action();}catch(error){caught=error.name==='TypeError';}
    if(!caught)throw 'expected Symbol conversion TypeError';
}
var symbol=Symbol('not text'),query=new URLSearchParams('keep=old'),url=new URL('https://example.org/old');
typeError(function(){new URL(symbol);});
typeError(function(){URL.parse(symbol);});
typeError(function(){URL.canParse(symbol);});
typeError(function(){new URL('/x',symbol);});
typeError(function(){new URLSearchParams(symbol);});
typeError(function(){new URLSearchParams([[symbol,'value']]);});
typeError(function(){query.append('\ud800',symbol);});
typeError(function(){query.get(symbol);});
typeError(function(){query.has('keep',symbol);});
typeError(function(){query.delete('keep',symbol);});
typeError(function(){url.href=symbol;});
if(query.toString()!=='keep=old'||url.href!=='https://example.org/old')throw 'Symbol mutation';
'ok';
"#);
}

#[test]
fn url_usv_receiver_and_required_argument_checks_precede_author_conversion() {
    succeeds(r#"
var calls=0,input={};
input[Symbol.toPrimitive]=function(){calls++;return '\ud800';};
function typeError(action){
    var caught=false;try{action();}catch(error){caught=error.name==='TypeError';}
    if(!caught)throw 'expected receiver or argument TypeError';
}
typeError(function(){URLSearchParams.prototype.append.call({},input,input);});
typeError(function(){URLSearchParams.prototype.get.call({},input);});
typeError(function(){Object.getOwnPropertyDescriptor(URL.prototype,'href').set.call({},input);});
var query=new URLSearchParams();
typeError(function(){query.append(input);});
typeError(function(){query.set(input);});
typeError(function(){URL(input);});
if(calls!==0||query.size!==0)throw 'conversion preceded API checks';
'ok';
"#);
}

#[test]
fn url_usv_record_symbol_key_conversion_precedes_its_getter() {
    succeeds(r#"
var record={},symbol=Symbol('enumerable'),trace='',caught=false;
Object.defineProperty(record,'\ud800',{enumerable:true,get:function(){trace+='R';return '\udfff';}});
Object.defineProperty(record,symbol,{enumerable:true,get:function(){trace+='S';return 'forbidden';}});
try{new URLSearchParams(record);}catch(error){caught=error.name==='TypeError';}
if(!caught||trace!=='R')throw 'record Symbol Get preceded key conversion';
var skipped={};Object.defineProperty(skipped,Symbol('nonenum'),{get:function(){throw 'nonenumerable Symbol';}});
skipped['\ud800']='\udfff';
if(new URLSearchParams(skipped).toString()!=='%EF%BF%BD=%EF%BF%BD')throw 'nonenumerable Symbol skipped';
'ok';
"#);
}

#[test]
fn url_usv_conversion_errors_keep_identity_and_prior_effects_without_later_callbacks() {
    succeeds(r#"
var marker={},trace='',input={},base={toString:function(){trace+='B';return 'https://example.org/';}};
input[Symbol.toPrimitive]=function(){trace+='I';throw marker;};
var caught=false;try{new URL(input,base);}catch(error){caught=error===marker;}
if(!caught||trace!=='I')throw 'URL conversion exception';
var query=new URLSearchParams('keep=old'),name={},value={};
name[Symbol.toPrimitive]=function(){trace+='K';query.append('side','kept');return '\ud800';};
value[Symbol.toPrimitive]=function(){trace+='V';throw marker;};
caught=false;try{query.append(name,value);}catch(error){caught=error===marker;}
if(!caught||trace!=='IKV'||query.toString()!=='keep=old&side=kept')throw 'method exception and prior effects';
var record={};
Object.defineProperty(record,'\ud800',{enumerable:true,get:function(){trace+='G';return value;}});
Object.defineProperty(record,'later',{enumerable:true,get:function(){throw 'later getter';}});
caught=false;try{new URLSearchParams(record);}catch(error){caught=error===marker;}
if(!caught||trace!=='IKVGV')throw 'record value exception';
'ok';
"#);
}

#[test]
fn url_usv_pair_conversion_failure_closes_inner_then_outer_and_keeps_original_throw() {
    succeeds(r#"
var marker={},replacement={},trace='',bad={};
bad[Symbol.toPrimitive]=function(hint){if(hint!=='string')throw 'hint';trace+='V';throw marker;};
var pair={};pair[Symbol.iterator]=function(){
    var step=0;return {
        next:function(){step++;trace+='n';return step===1?{value:'\ud800',done:false}:{value:bad,done:false};},
        return:function(){trace+='p';throw replacement;}
    };
};
var source={};source[Symbol.iterator]=function(){
    var step=0;return {
        next:function(){step++;trace+='N';return step===1?{value:pair,done:false}:{done:true};},
        return:function(){trace+='o';return {};}
    };
};
var result='unpublished',caught=false;
try{result=new URLSearchParams(source);}catch(error){caught=error===marker;}
if(!caught||result!=='unpublished'||trace!=='NnnVpo')throw 'nested iterator close';
'ok';
"#);
}

#[test]
fn url_usv_completed_pair_iteration_and_wrong_length_keep_distinct_close_behavior() {
    succeeds(r#"
var innerClosed=0,outerClosed=0,conversions=0;
function source(count){
    var pair={};pair[Symbol.iterator]=function(){
        var step=0;return {
            next:function(){
                if(step++>=count)return {done:true};
                return {value:{toString:function(){conversions++;return '\ud800';}},done:false};
            },
            return:function(){innerClosed++;return {};}
        };
    };
    var input={};input[Symbol.iterator]=function(){
        var step=0;return {
            next:function(){return step++===0?{value:pair,done:false}:{done:true};},
            return:function(){outerClosed++;return {};}
        };
    };
    return input;
}
var query=new URLSearchParams(source(2));
if(query.toString()!=='%EF%BF%BD=%EF%BF%BD'||conversions!==2||innerClosed!==0||outerClosed!==0)
    throw 'completed iterable closed';
var caught=false;try{new URLSearchParams(source(3));}catch(error){caught=error.name==='TypeError';}
if(!caught||conversions!==5||innerClosed!==0||outerClosed!==1)throw 'wrong-length completed pair';
'ok';
"#);
}
