//! Original loopback search workflow fixtures. These exercise the real App,
//! form events, HTTP navigation, rendered links and history. They are not
//! captured Google responses and do not establish live Google compatibility.
use super::*;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[path = "app/tests/linked_result_image.rs"]
mod linked_result_image;

struct Site {
    address: SocketAddr,
    stopped: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<String>>>,
    posts: Arc<Mutex<Vec<(String, String)>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Site {
    fn new(result_origin: Option<String>) -> Self {
        Self::fixture(result_origin, false)
    }

    fn with_linked_result_image() -> Self {
        Self::fixture(None, true)
    }

    fn fixture(result_origin: Option<String>, linked_result_image: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let ending = stopped.clone();
        let seen = requests.clone();
        let posts = Arc::new(Mutex::new(Vec::new()));
        let post_requests = posts.clone();
        let worker = std::thread::spawn(move || {
            for incoming in listener.incoming() {
                let Ok(mut stream) = incoming else { break };
                if ending.load(Ordering::Relaxed) {
                    break;
                }
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut byte = [0u8];
                while bytes.len() < 16384 && !bytes.ends_with(b"\r\n\r\n") {
                    if stream.read_exact(&mut byte).is_err() {
                        break;
                    }
                    bytes.push(byte[0]);
                }
                if !bytes.ends_with(b"\r\n\r\n") {
                    continue;
                }
                let header = String::from_utf8_lossy(&bytes);
                let mut request = header.lines().next().unwrap().split_whitespace();
                let method = request.next().unwrap();
                assert!(matches!(method, "GET" | "POST"));
                let path = request.next().unwrap().to_owned();
                seen.lock().unwrap().push(path.clone());
                if method == "POST" {
                    let length = header
                        .lines()
                        .filter_map(|line| line.split_once(':'))
                        .find_map(|(key, value)| {
                            key.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    assert!(length <= crate::http::MAX_FORM_BYTES);
                    let mut body = vec![0; length];
                    stream.read_exact(&mut body).unwrap();
                    post_requests
                        .lock()
                        .unwrap()
                        .push((path.clone(), String::from_utf8(body).unwrap()));
                }
                if linked_result_image && path == "/thumb.ppm" {
                    let body = linked_result_image::THUMBNAIL;
                    let header = format!(
                        "HTTP/1.1 200 Fixture\r\nContent-Type: image/x-portable-pixmap\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len(),
                    );
                    let _ = stream.write_all(header.as_bytes()).and_then(|_| stream.write_all(body));
                    continue;
                }
                let linked_response = if linked_result_image {
                    linked_result_image::response(&path)
                } else { None };
                let (status, extra, body) = if let Some(body) = linked_response {
                    (200, String::new(), body)
                } else if path == "/post-result?keep=1" {
                    (
                        200,
                        String::new(),
                        "<title>Script POST result</title><h1>POST received</h1>".to_owned(),
                    )
                } else if path == "/result" {
                    (200, String::new(), "<title>External fixture result</title><h1>Visited the actual result page</h1>".to_owned())
                } else if path == "/visit?entry=original" || path.starts_with("/form-redirect?") {
                    (
                        302,
                        format!("Location: {}/result\r\n", result_origin.as_ref().unwrap()),
                        String::new(),
                    )
                } else if path.starts_with("/home-") || path.starts_with("/search-") {
                    let area = path.starts_with("/home-area") || path.starts_with("/search-area");
                    let results = path.starts_with("/search-");
                    let query = path
                        .split_once('?')
                        .map_or("", |(_, query)| query)
                        .split('&')
                        .find_map(|entry| entry.strip_prefix("q="))
                        .unwrap_or("");
                    let value = decode(query);
                    let scripted = path.contains("-script") || path.contains("-post");
                    let post = path.contains("-post");
                    let long_dispatch = path.contains("-long");
                    let deferred_body = path.contains("-deep");
                    let symbol_bootstrap = path.contains("-symbol");
                    let accessor_bootstrap = path.contains("-accessor");
                    let callable_bootstrap = path.contains("-callable");
                    let descriptor_bootstrap = path.contains("-descriptors");
                    let wrapper_bootstrap = path.contains("-wrappers");
                    let iterator_bootstrap = path.contains("-iterators");
                    let spread_bootstrap = path.contains("-spread");
                    let promise_bootstrap = path.contains("-promise");
                    let builtin_bootstrap = path.contains("-builtins");
                    let template_bootstrap = path.contains("-templates");
                    let function_bootstrap = path.contains("-functions");
                    let optional_bootstrap = path.contains("-optional");
                    let defaults_bootstrap = path.contains("-defaults");
                    let named_regexp_bootstrap = path.contains("-namedregexp");
                    let patterns_bootstrap = path.contains("-patterns");
                    let dates_bootstrap = path.contains("-dates");
                    let object_spread_bootstrap = path.contains("-objectspread");
                    let string_object_bootstrap = path.contains("-stringobjects");
                    let operators_bootstrap = path.contains("-operators");
                    let url_object_bootstrap = path.contains("-urlobjects");
                    let query_params_bootstrap = path.contains("-queryparams");
                    let combined_041_bootstrap = path.contains("-combined041");
                    let kind = if string_object_bootstrap {
                        "area-script-stringobjects"
                    } else if operators_bootstrap {
                        "area-script-operators"
                    } else if url_object_bootstrap {
                        "area-script-urlobjects"
                    } else if query_params_bootstrap {
                        "area-script-queryparams"
                    } else if combined_041_bootstrap {
                        "area-script-combined041"
                    } else if object_spread_bootstrap {
                        "area-script-objectspread"
                    } else if defaults_bootstrap {
                        "area-script-defaults"
                    } else if named_regexp_bootstrap {
                        "area-script-namedregexp"
                    } else if patterns_bootstrap {
                        "area-script-patterns"
                    } else if dates_bootstrap {
                        "area-script-dates"
                    } else if function_bootstrap {
                        "area-script-functions"
                    } else if optional_bootstrap {
                        "area-script-optional"
                    } else if template_bootstrap {
                        "area-script-templates"
                    } else if builtin_bootstrap {
                        "area-script-builtins"
                    } else if promise_bootstrap {
                        "area-script-promise"
                    } else if spread_bootstrap {
                        "area-script-spread"
                    } else if iterator_bootstrap {
                        "area-script-iterators"
                    } else if wrapper_bootstrap {
                        "area-script-wrappers"
                    } else if descriptor_bootstrap {
                        "area-script-descriptors"
                    } else if callable_bootstrap {
                        "area-script-callable"
                    } else if accessor_bootstrap {
                        "area-script-accessor"
                    } else if symbol_bootstrap {
                        "area-script-symbol"
                    } else if deferred_body {
                        "area-script-deep"
                    } else if long_dispatch {
                        "area-script-long"
                    } else if scripted {
                        "area-script"
                    } else if area {
                        "area"
                    } else {
                        "input"
                    };
                    let required = if path.contains("-required") {
                        "required"
                    } else {
                        ""
                    };
                    let field = if area {
                        format!(
                            "<textarea id=q name=q {required} rows=2 cols=32>{}</textarea>",
                            escape(&value)
                        )
                    } else {
                        format!(
                            "<input id=q name=q type=search value='{}' size=32>",
                            escape(&value)
                        )
                    };
                    let results_html = if results {
                        format!("<main id=results><h1>Results for {}</h1><h2><a href='/visit?entry=original'>Original fixture result</a></h2><p>Independent text shown beneath the clickable result.</p></main>", escape(&value))
                    } else {
                        "<p>Search the original loopback fixture.</p>".to_owned()
                    };
                    let title = if results {
                        "Fixture results"
                    } else {
                        "Fixture home"
                    };
                    let method = if post { "post" } else { "get" };
                    let action = if path.contains("-cross") {
                        format!("{}/result", result_origin.as_ref().unwrap())
                    } else if path.contains("-redirect") {
                        "/form-redirect".to_owned()
                    } else if post {
                        "/post-result?keep=1".to_owned()
                    } else {
                        format!("/search-{kind}?discard=old")
                    };
                    let submit_script = if scripted {
                        "document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};document.f.q.onkeydown=function(event){if(event.key==='Enter'){event.preventDefault();this.form.submit();document.f.q.value='changed after submit';}};"
                    } else {
                        ""
                    };
                    let bootstrap = if string_object_bootstrap {
                        r#"var key=new String('Enter'),shape=Object.getOwnPropertyDescriptor(key,0);
if(key.valueOf()!=='Enter'||shape.value!=='E'||shape.writable||shape.configurable||!shape.enumerable)throw Error('String key descriptor');
if(Object.keys(key).join(',')!=='0,1,2,3,4'||Array.from(key).join('')!=='Enter')throw Error('String key enumeration');
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
document.f.q.onkeydown=function(event){if(event.key===key.valueOf()){event.preventDefault();
var boxed=new String(this.value);boxed.ignored=7;
if(JSON.parse(JSON.stringify(boxed))!==this.value||String.prototype.valueOf.call(boxed)!==this.value)throw Error('String query snapshot');
this.form.submit();this.value='changed after submit';}};
document.f.setAttribute('data-stringobjects','ready');"#.to_owned()
                    } else if operators_bootstrap {
                        r#"var config={},skipped=0;config.key||='Enter';config.limit??=2**3**2;config.key||=(skipped++,'wrong');
if(config.limit!==512||skipped!==0||!Object.is((-0)**3,-0))throw Error('operator bootstrap');
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
document.f.q.onkeydown=function(event){if(event.key===config.key){event.preventDefault();
var snapshot=null,enabled=true,power=2;snapshot??=this.value;enabled&&=snapshot===this.value;power**=3;
if(!enabled||power!==8||snapshot!==this.value)throw Error('operator query snapshot');
this.form.submit();this.value='changed after submit';}};
document.f.setAttribute('data-operators','ready');"#.to_owned()
                    } else if url_object_bootstrap {
                        r#"if(window.URL!==URL||window.URLSearchParams!==URLSearchParams)throw Error('URL global surface');
var here=new URL(location.href),current=here.searchParams;
if(current!==here.searchParams||here.origin!==location.origin)throw Error('URL current document');
document.f.q.onkeydown=function(event){if(event.key==='Enter'){event.preventDefault();
var target=new URL(this.form.getAttribute('action'),location.href),params=target.searchParams;
target.search='';if(params!==target.searchParams)throw Error('URL stable query identity');
params.append('q',this.value);params.append('source','fixture');
if(target.searchParams.get('q')!==this.value||new URL(target.href).searchParams.get('q')!==this.value)throw Error('URL query snapshot');
location.href=target.href;this.value='changed after submit';}};
document.f.setAttribute('data-urlobjects','ready');"#.to_owned()
                    } else if query_params_bootstrap {
                        r#"var config=new URLSearchParams([['key','Enter'],['duplicate','first'],['duplicate','second']]);
if(config.getAll('duplicate').join(',')!=='first,second'||config.size!==3)throw Error('query bootstrap');
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
document.f.q.onkeydown=function(event){if(event.key===config.get('key')){event.preventDefault();
var params=new URLSearchParams({q:this.value,source:'fixture'}),decoded=new URLSearchParams(params.toString()),copied=Object.fromEntries(decoded);
if(copied.q!==this.value||copied.source!=='fixture'||Array.from(decoded.keys()).join(',')!=='q,source')throw Error('query encoding snapshot');
this.value=copied.q;this.form.submit();this.value='changed after submit';}};
document.f.setAttribute('data-queryparams','ready');"#.to_owned()
                    } else if combined_041_bootstrap {
                        r#"var config={};config.key??=new String('Enter');
Promise.resolve({key:config.key}).then(function(settings){
document.f.q.onkeydown=function(event){if(event.key===settings.key.valueOf()){event.preventDefault();var input=this,query=new String(input.value);
Promise.resolve(query).then(function(value){var target=new URL(input.form.getAttribute('action'),location.href);target.search='';
var params=target.searchParams;params.set('q',value);params.set('source','fixture');
if(params.get('q')!==input.value||2**3!==8)throw Error('Promise URL snapshot');
location.href=target.toString();input.value='changed after submit';});}};
document.f.setAttribute('data-combined041','ready');});"#.to_owned()
                    } else if object_spread_bootstrap {
                        r#"var marker=Symbol('query'),base={get key(){return 'Enter';}};
Object.defineProperty(base,'ignored',{get(){throw Error('hidden spread getter');}});
var config={...null,...base,...undefined,...{enabled:true}};
if(config.key!=='Enter'||!config.enabled||Object.hasOwn(config,'ignored'))throw Error('spread configuration');
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
document.f.q.onkeydown=function(event){if(event.key===config.key){event.preventDefault();
var input=this,reads=0,source={get query(){reads++;return input.value;},[marker]:'kept'};
var packet={query:'old',...source,tail:1},shape=Object.getOwnPropertyDescriptor(packet,'query');
if(reads!==1||shape.value!==this.value||!shape.writable||!shape.enumerable||!shape.configurable||packet[marker]!=='kept')throw Error('spread query snapshot');
this.form.submit();this.value='changed after submit';}};
document.f.setAttribute('data-objectspread','ready');"#.to_owned()
                    } else if defaults_bootstrap {
                        r#"function keyword(first='Ent',last='er'){return first+last;}
var key=keyword();
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
document.f.q.onkeydown=function(event,query=this.value,read=()=>query){
var query='body binding';
if(event.key===key){event.preventDefault();if(read()!==this.value||arguments.length!==1)throw Error('parameter snapshot');this.form.submit();this.value='changed after submit';}
};
if(document.f.q.onkeydown.length!==1)throw Error('default function length');
document.f.setAttribute('data-defaults',keyword('rea','dy'));"#.to_owned()
                    } else if named_regexp_bootstrap {
                        r#"var key=/^(?<first>Ent)(?<last>er)$/.exec('Enter').groups;
if(Object.getPrototypeOf(key)!==null||key.first+key.last!=='Enter')throw Error('named groups bootstrap');
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
document.f.q.onkeydown=function(event){
if(event.key===key.first+key.last){event.preventDefault();var snapshot=/^(?<query>.*)$/s.exec(this.value).groups.query;
if(snapshot!==this.value||this.value.replace(/(?<query>.*)/s,'$<query>')!==snapshot)throw Error('named query snapshot');
this.form.submit();this.value='changed after submit';}
};
document.f.setAttribute('data-namedregexp','ready');"#.to_owned()
                    } else if patterns_bootstrap {
                        r#"var {keys:[keyword],options:{enabled=true}={}}={keys:['Enter'],options:{}};
var [first,...tail]=['re','a','dy'];if(!enabled||first+tail.join('')!=='ready')throw Error('patterns bootstrap');
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
function snapshot({query=value}={},value='missing'){return query;}
document.f.q.onkeydown=function(event){var {key}=event;
if(key===keyword){event.preventDefault();var {value:query}=this, [copy]=[query];
if(snapshot({query:copy})!==this.value)throw Error('pattern query snapshot');
this.form.submit();this.value='changed after submit';}
};
document.f.setAttribute('data-patterns',first+tail.join(''));"#.to_owned()
                    } else if dates_bootstrap {
                        r#"var stamp=new Date(Date.UTC(2024,1,29,12,34,56,789));
if(stamp.toISOString()!=='2024-02-29T12:34:56.789Z'||Date.parse(stamp.toISOString())!==stamp.getTime())throw Error('Date bootstrap');
if(typeof Date.now()!=='number'||!Number.isFinite(new Date().getTime()))throw Error('Date clock');
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
document.f.q.onkeydown=function(event){if(event.key==='Enter'){event.preventDefault();var packet=JSON.stringify({query:this.value,stamp:stamp});var parsed=JSON.parse(packet);
if(parsed.query!==this.value||parsed.stamp!==stamp.toISOString())throw Error('Date query snapshot');
this.form.submit();this.value='changed after submit';}};
document.f.setAttribute('data-dates','ready');"#.to_owned()
                    } else if function_bootstrap {
                        r#"var keyword=Function('first','last','return first+last;')('Ent','er');
var Packet=new Function('value','this.value=value;');
if(Function('return this;')()!==window||Function('return typeof anonymous;')()!=='undefined')throw Error('Function global scope');
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
function install(){
var hidden='caller scope';
document.f.q.onkeydown=Function('event',"if(typeof hidden!=='undefined')throw Error('captured caller');if(event.key===keyword){event.preventDefault();var packet=new Packet(this.value);if(packet.value!==this.value)throw Error('Function query snapshot');this.form.submit();this.value='changed after submit';}");
}
install();document.f.setAttribute('data-functions',Function("return 'ready';")());"#.to_owned()
                    } else if optional_bootstrap {
                        r#"var config={search:{key:'Enter'}},absent=null,touches=0;
var keyword=config?.search?.key??'Escape',ignored=absent?.[touches++]?.(touches++);
if(touches!==0||ignored!==undefined||(0??9)!==0||(false??true)!==false)throw Error('optional short circuit');
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
document.f.q.onkeydown=function(event){
if(event?.key===keyword){
event.preventDefault();var input=this,snapshot=input?.value??'missing';
if(snapshot!==input.value)throw Error('optional query snapshot');
input?.form?.submit();input.value='changed after submit';
}};
document.f.setAttribute('data-optional',config?.missing?.value??'ready');"#.to_owned()
                    } else if template_bootstrap {
                        r#"var key=`${'Ent'}er`,seen='';
function describe(query){return `query:${{toString(){seen+='convert';return query;}}}:${`length=${query.length}`}`;}
if(`${/}/.test('}')} ${({value:'ready'}).value}`!=='true ready')throw Error('template delimiters');
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
document.f.q.onkeydown=function(event){
if(event.key===key){
event.preventDefault();var snapshot=this.value;seen='';
var description=describe(snapshot);
if(description!=='query:'+snapshot+':length='+snapshot.length||seen!=='convert')throw Error('template conversion');
this.form.submit();this.value=`changed ${'after'} submit`;
}};
document.f.setAttribute('data-templates',`${'rea'}dy`);"#.to_owned()
                    } else if builtin_bootstrap {
                        r#"var config=JSON.parse('{"key":"Enter","discard":0}',function(key,value){if(key==='discard')return undefined;return value;});
var keys=[['q'],['source']].flat();
if(keys.join(',')!=='q,source'||Object.hasOwn(config,'discard'))throw Error('JSON/flat bootstrap');
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
document.f.q.onkeydown=function(event){
if(event.key===config.key){
event.preventDefault();var snapshot=this.value,calls=0;
var clean=('#'+snapshot+'#').replaceAll('#',function(match,offset,text){
if(match!=='#'||text!=='#'+snapshot+'#'||offset!==(calls===0?0:snapshot.length+1))throw Error('replaceAll callback');calls++;return '';
});
if(calls!==2||clean!==snapshot)throw Error('replacement snapshot');
var packet={q:{toJSON(key){if(key!=='q')throw Error('toJSON holder');return clean;}},source:'fixture',discard:1};
var wire=JSON.stringify(packet,function(key,value){if(key==='discard')return undefined;return value;});
var parsed=JSON.parse(wire);
var names=keys.flatMap(function(key){return [key,Object.hasOwn(parsed,key)];});
if(names.join(',')!=='q,true,source,true'||parsed.q!==snapshot||Object.hasOwn(parsed,'discard'))throw Error('JSON roundtrip');
this.form.submit();this.value='changed after submit';
}};
document.f.setAttribute('data-builtins','ready');"#.to_owned()
                    } else if promise_bootstrap {
                        r#"var bootstrapOrder='executor';
var settings={then(resolve){queueMicrotask(function(){resolve({key:'Enter'});});}};
Promise.resolve(settings).then(function(config){
if(bootstrapOrder!=='executor:script')throw Error('synchronous Promise reaction');
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
document.f.q.onkeydown=function(event){
if(event.key===config.key){
event.preventDefault();var input=this;var query=input.value;
Promise.resolve(query).then(function(value){
if(input.value!==value)throw Error('Promise query snapshot');
input.form.submit();input.value='changed after submit';
});
}};
return 'ready';
}).finally(function(){document.f.setAttribute('data-promise','ready');});
bootstrapOrder+=':script';"#
                            .to_owned()
                    } else if spread_bootstrap {
                        // Original syntax fixture executes array/call/new spread
                        // and ordinary/arrow/method rest during actual searches.
                        r#"function options(...pairs){return Object.fromEntries(pairs);}
var config=options(['key','Enter'],['ready',true]);
var combine=(...parts)=>parts.join('');
function Packet(...parts){this.query=combine(...parts);}
if(options.length!==0||Packet.length!==0||combine.length!==0||[...config.key].join('')!=='Enter')throw Error('spread/rest bootstrap');
function querySequence(value){var iterable={};iterable[Symbol.iterator]=function(){var sent=false;return {next(){if(sent)return {done:true};sent=true;return {value:value,done:false};}};};return iterable;}
var handlers={submit(event,...values){
if(this!==document.f.q||values.length!==1)throw Error('rest handler receiver');
var fragments=[...querySequence(values[0]),...[]];
var packet=new Packet(...fragments);
if(packet.query!==this.value||!config.ready)throw Error('spread query snapshot');
this.form.submit();document.f.q.value='changed after submit';
}};
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
document.f.q.onkeydown=function(event){if(event.key===config.key){event.preventDefault();handlers.submit.call(this,event,...[this.value]);}};
document.f.setAttribute('data-spread','ready');"#
                            .to_owned()
                    } else if iterator_bootstrap {
                        // Original protocol fixture runs the iterable helpers
                        // and early-exit cleanup through actual search events.
                        r#"var closeCount=0;
function sequence(values){
var source={};source[Symbol.iterator]=function(){var index=0;return {
next(){return index<values.length?{value:values[index++],done:false}:{done:true};},
return(){closeCount++;return {done:true};}
};};return source;}
var config=Object.fromEntries(sequence([['key','Enter'],['enabled',true]]));
var keys=Array.from(new Set([config.key,config.key]),function(value,index){if(index!==0)throw Error('unique key');return value;});
var first;for(var key of sequence(keys)){first=key;break;}
if(first!=='Enter'||closeCount!==1||!config.enabled)throw Error('iterator bootstrap');
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
document.f.q.onkeydown=function(event){
if(event.key===keys[0]){
event.preventDefault();
var packet=Object.fromEntries(sequence([['q',this.value],['source','fixture']]));
var query=Array.from(sequence([packet.q]),function(value,index){if(index!==0)throw Error('query index');return value;}).join('');
var before=closeCount;
for(var item of sequence([query,'unused'])){if(item!==this.value)throw Error('query snapshot');break;}
if(closeCount!==before+1)throw Error('query iterator cleanup');
this.form.submit();document.f.q.value='changed after submit';
}};
document.f.setAttribute('data-iterators','ready');"#
                            .to_owned()
                    } else if wrapper_bootstrap {
                        // Independent wrapper/conversion page. Query coercion
                        // runs in the actual event handler on both searches.
                        r#"var limit=new Number(1), enabled=new Boolean(true), key=Object(Symbol('search-state')), state={};
state[key]='ready';
if(state[key]!=='ready'||JSON.stringify({limit:limit,enabled:enabled})!=='{"limit":1,"enabled":true}')throw Error('wrapper bootstrap');
function boxedReceiver(){return typeof this==='object'&&this.valueOf()===7;}
if(!boxedReceiver.call(7))throw Error('receiver boxing');
var query={};
query[Symbol.toPrimitive]=function(hint){if(hint!=='string'||this!==query)throw Error('query conversion');return document.f.q.value;};
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
document.f.q.onkeydown=function(event){
if(event.key==='Enter'){
event.preventDefault();
if(String(query)!==this.value||!enabled.valueOf())throw Error('query snapshot');
this.form.submit();document.f.q.value='changed after submit';
}};
document.f.setAttribute('data-wrappers','ready');"#
                            .to_owned()
                    } else if descriptor_bootstrap {
                        // Original accessor/array fixture: the actual event
                        // handler reads the query through script accessors.
                        r#"var queryState={
get query(){return document.f.q.value;},
set query(value){document.f.q.value=value;}
};
var keys=['Enter','old','unused'];
Object.defineProperty(keys,'length',{value:1,writable:false});
Object.defineProperty(keys,'0',{get:function(){return 'Enter';},enumerable:true,configurable:true});
var shape=Object.getOwnPropertyDescriptors(keys);
if(keys.length!==1||shape.length.writable||Object.hasOwn(keys,'1')||typeof shape[0].get!=='function')throw Error('array descriptor bootstrap');
var queryShape=Object.getOwnPropertyDescriptor(queryState,'query');
if(typeof queryShape.get!=='function'||typeof queryShape.set!=='function'||!queryShape.enumerable||!queryShape.configurable)throw Error('literal accessor bootstrap');
document.f.onsubmit=function(){throw Error('script submit dispatched a submit event');};
document.f.q.onkeydown=function(event){
if(event.key===keys[0]){
event.preventDefault();
if(queryState.query!==this.value)throw Error('accessor query snapshot');
this.form.submit();queryState.query='changed after submit';
}};
document.f.setAttribute('data-descriptors','ready');"#
                            .to_owned()
                    } else if callable_bootstrap {
                        // Original standard-helper bootstrap. The resulting
                        // bound handler must survive actual event dispatch.
                        r#"var hiddenReads=0,settings={first:'search',second:'ready'};
function hiddenSetting(){hiddenReads++;return 'hidden';}
Object.defineProperty(settings,'hidden',{get:hiddenSetting,configurable:true});
var descriptors=Object.getOwnPropertyDescriptors(settings);
if(hiddenReads!==0||descriptors.hidden.get!==hiddenSetting||descriptors.first.value!=='search')throw Error('descriptor bootstrap');
var entries=Object.entries(settings),values=Object.values(settings);
if(entries.length!==2||entries[0][0]!=='first'||entries[0][1]!=='search'||entries[1][0]!=='second'||values.join('-')!=='search-ready'||hiddenReads!==0)throw Error('enumeration bootstrap');
var replacementCalls=0;
var label=values.join('-').replace(/(^|-)([a-z])/g,function(match,separator,letter,offset,original){
'use strict';
if(this!==undefined||arguments.length!==5||original!=='search-ready'||offset!==(replacementCalls===0?0:6))throw Error('replacement bootstrap');
replacementCalls++;return separator+letter.toUpperCase();});
if(label!=='Search-Ready'||replacementCalls!==2)throw Error('replacement result');
document.f.onsubmit=function(){throw Error('bound script submit dispatched a submit event');};
function readyEnter(marker,event){
if(marker!=='Search-Ready'||this!==document.f.q)throw Error('bound event receiver');
if(event.key==='Enter'){event.preventDefault();this.form.submit();document.f.q.value='changed after submit';}}
document.f.q.onkeydown=readyEnter.bind(document.f.q,label);
document.f.setAttribute('data-callable','ready');"#
                            .to_owned()
                    } else if accessor_bootstrap {
                        // Original passive-support probe. The inert event is
                        // registered and removed; only its option is queried.
                        format!(
                            r#"var passiveQueried=false,passiveReads=0;
var probeOptions=Object.defineProperty({{}},'passive',{{get:function(){{
if(this!==probeOptions)throw Error('option getter receiver');
passiveQueried=true;passiveReads++;return true;}}}});
function emptyFunction(){{}}
window.addEventListener('test',emptyFunction,probeOptions);
window.removeEventListener('test',emptyFunction,probeOptions);
if(!passiveQueried||passiveReads!==1)throw Error('passive option query');
Object.defineProperty(navigator,'fixtureAccessor',{{configurable:true,get:function(){{
if(this!==navigator)throw Error('native expando getter receiver');return passiveQueried;}}}});
if(!navigator.fixtureAccessor)throw Error('native expando getter');
var windowAccessorValue=0;
Object.defineProperty(window,'fixtureWindowAccessor',{{configurable:true,
get:function(){{if(this!==window)throw Error('window getter receiver');return windowAccessorValue;}},
set:function(value){{if(this!==window)throw Error('window setter receiver');windowAccessorValue=value;}}}});
fixtureWindowAccessor=1;if(window.fixtureWindowAccessor!==1)throw Error('window member read');
window.fixtureWindowAccessor=2;if(fixtureWindowAccessor!==2)throw Error('window name read');
delete window.fixtureWindowAccessor;
document.f.setAttribute('data-accessor','ready');
{submit_script}"#
                        )
                    } else if symbol_bootstrap {
                        // Original explicit calls: no automatic disposal or
                        // native iterator protocol is needed by this fixture.
                        format!(
                            r#"if(!Symbol.iterator)Symbol.iterator=Symbol('fixture iterator');
if(!Array.prototype[Symbol.iterator])Array.prototype[Symbol.iterator]=function(){{
var list=this,index=0;return {{next:function(){{return index<list.length?
{{value:list[index++],done:false}}:{{value:undefined,done:true}};}}}};}};
var iterator=[3,4][Symbol.iterator]();
if(iterator.next().value!==3||iterator.next().value!==4||!iterator.next().done)throw Error('iterator initialization');
function SearchSession(){{this.closed=false;}}
SearchSession.prototype[Symbol.dispose]=function(){{this.closed=true;}};
var session=new SearchSession();session[Symbol.dispose]();
if(!session.closed)throw Error('explicit disposal');
var symbolState=Symbol('search fixture state');navigator[symbolState]=session;
if(navigator[symbolState]!==session)throw Error('symbol native expando');
document.f.setAttribute('data-symbol','ready');
{submit_script}"#
                        )
                    } else if deferred_body {
                        // The original initializer is created inside 44 call
                        // arguments, then runs a separate 20-call expression.
                        // Its body must finish before the Enter handler exists.
                        format!(
                            "function keep(value){{return value;}}var initialize={}function(){{var mode={}7{};if(mode!==7)throw Error('function initialization');document.f.setAttribute('data-function-body','ready');{submit_script}}}{};initialize();",
                            "keep(".repeat(44),
                            "keep(".repeat(20),
                            ")".repeat(20),
                            ")".repeat(44)
                        )
                    } else if long_dispatch {
                        // Original dispatcher fixture: no captured third-party
                        // source, challenge code, challenge URL or response.
                        let mut source = String::from("function selectSearchMode(mode){");
                        for arm in 0..300 {
                            if arm > 0 {
                                source.push_str("else ");
                            }
                            source.push_str(&format!("if(mode==={arm})return {arm};"));
                        }
                        source.push_str("else return -1;}if(selectSearchMode(299)!==299)throw Error('dispatcher initialization');document.f.setAttribute('data-dispatch','ready');");
                        source
                    } else {
                        String::new()
                    };
                    let enter_script = if deferred_body
                        || symbol_bootstrap
                        || accessor_bootstrap
                        || callable_bootstrap
                        || descriptor_bootstrap
                        || wrapper_bootstrap
                        || iterator_bootstrap
                        || spread_bootstrap
                        || promise_bootstrap
                        || builtin_bootstrap
                        || template_bootstrap
                        || function_bootstrap
                        || optional_bootstrap
                        || defaults_bootstrap
                        || named_regexp_bootstrap
                        || patterns_bootstrap
                        || dates_bootstrap
                        || object_spread_bootstrap
                    {
                        ""
                    } else {
                        submit_script
                    };
                    let policy = if path.contains("-csp") {
                        "Content-Security-Policy: form-action 'none'\r\n"
                    } else {
                        ""
                    };
                    let body = format!(
                        r#"<!doctype html><html><head><title>{title}</title>
<style>body{{margin:20px}}h1{{font-size:24px}} textarea,input{{width:320px}} #results{{display:block}}</style></head><body>
<form id=f name=f method={method} action='{action}'>
{field}<input type=hidden name=source value=fixture><button id=send name=go value=search>Search</button></form>
{results_html}<script>
var changes=0;
document.f.q.oninput=function(){{changes++;}};
document.f.q.onkeydown=function(event){{if(event.key==='Enter' && this.value==='prevent newline')event.preventDefault();}};
document.f.onsubmit=function(){{if(document.f.q.value==='cancel')return false;}};
{bootstrap}
{enter_script}
</script></body></html>"#
                    );
                    (200, policy.to_owned(), body)
                } else {
                    (
                        404,
                        String::new(),
                        "<title>Fixture route missing</title>".to_owned(),
                    )
                };
                let response = format!("HTTP/1.1 {status} Fixture\r\nContent-Type: text/html;charset=utf-8\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let _ = stream.write_all(response.as_bytes());
            }
        });
        Self {
            address,
            stopped,
            requests,
            posts,
            worker: Some(worker),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    fn seen(&self, path: &str) -> bool {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .any(|request| request == path)
    }
}

impl Drop for Site {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.address);
        self.worker.take().unwrap().join().unwrap();
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&#39;")
}

fn decode(encoded: &str) -> String {
    let mut result = Vec::new();
    let mut bytes = encoded.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let a = bytes.next().unwrap();
            let b = bytes.next().unwrap();
            let hex = |value: u8| (value as char).to_digit(16).unwrap() as u8;
            result.push(hex(a) * 16 + hex(b));
        } else {
            result.push(if byte == b'+' { b' ' } else { byte });
        }
    }
    String::from_utf8(result).unwrap()
}

fn id(app: &App, id: &str) -> usize {
    app.document
        .nodes
        .iter()
        .enumerate()
        .find_map(|(node, _)| (app.document.attr(node, "id") == Some(id)).then_some(node))
        .unwrap()
}

fn click_control(app: &mut App, id_value: &str) {
    let node = id(app, id_value);
    let rect = app
        .page
        .controls
        .iter()
        .find(|rect| rect.node == node)
        .unwrap();
    let (x, y) = (rect.x + 6, rect.y + app.content_top() - app.scroll + 6);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
}

fn edit_query(app: &mut App, value: &str) {
    click_control(app, "q");
    assert!(app.focus == Focus::Control(id(app, "q")));
    app.key(0x41, true, false, false);
    app.paste(value);
    assert_eq!(app.forms.controls[&id(app, "q")].value.text(), value);
}

fn wait(app: &mut App) {
    app.wait_for_load();
    assert!(!app.failed, "{}", app.status);
    if let Some(script) = &app.scripting {
        assert_eq!(script.error_count(), 0, "{}", script.report());
    }
}

fn run_search_flow(area: bool, scripted: bool, bootstrap: &str) {
    let long_dispatch = bootstrap == "long";
    let deferred_body = bootstrap == "deep";
    let symbol_bootstrap = bootstrap == "symbol";
    let accessor_bootstrap = bootstrap == "accessor";
    let callable_bootstrap = bootstrap == "callable";
    let descriptor_bootstrap = bootstrap == "descriptors";
    let wrapper_bootstrap = bootstrap == "wrappers";
    let iterator_bootstrap = bootstrap == "iterators";
    let spread_bootstrap = bootstrap == "spread";
    let promise_bootstrap = bootstrap == "promise";
    let builtin_bootstrap = bootstrap == "builtins";
    let template_bootstrap = bootstrap == "templates";
    let function_bootstrap = bootstrap == "functions";
    let optional_bootstrap = bootstrap == "optional";
    let defaults_bootstrap = bootstrap == "defaults";
    let named_regexp_bootstrap = bootstrap == "namedregexp";
    let patterns_bootstrap = bootstrap == "patterns";
    let dates_bootstrap = bootstrap == "dates";
    let object_spread_bootstrap = bootstrap == "objectspread";
    let string_object_bootstrap = bootstrap == "stringobjects";
    let operators_bootstrap = bootstrap == "operators";
    let url_object_bootstrap = bootstrap == "urlobjects";
    let query_params_bootstrap = bootstrap == "queryparams";
    let combined_041_bootstrap = bootstrap == "combined041";
    let result = Site::new(None);
    let search = Site::new(Some(result.url("")));
    let kind = if string_object_bootstrap {
        "area-script-stringobjects"
    } else if operators_bootstrap {
        "area-script-operators"
    } else if url_object_bootstrap {
        "area-script-urlobjects"
    } else if query_params_bootstrap {
        "area-script-queryparams"
    } else if combined_041_bootstrap {
        "area-script-combined041"
    } else if object_spread_bootstrap {
        "area-script-objectspread"
    } else if defaults_bootstrap {
        "area-script-defaults"
    } else if named_regexp_bootstrap {
        "area-script-namedregexp"
    } else if patterns_bootstrap {
        "area-script-patterns"
    } else if dates_bootstrap {
        "area-script-dates"
    } else if function_bootstrap {
        "area-script-functions"
    } else if optional_bootstrap {
        "area-script-optional"
    } else if template_bootstrap {
        "area-script-templates"
    } else if builtin_bootstrap {
        "area-script-builtins"
    } else if promise_bootstrap {
        "area-script-promise"
    } else if spread_bootstrap {
        "area-script-spread"
    } else if iterator_bootstrap {
        "area-script-iterators"
    } else if wrapper_bootstrap {
        "area-script-wrappers"
    } else if descriptor_bootstrap {
        "area-script-descriptors"
    } else if callable_bootstrap {
        "area-script-callable"
    } else if accessor_bootstrap {
        "area-script-accessor"
    } else if symbol_bootstrap {
        "area-script-symbol"
    } else if deferred_body {
        "area-script-deep"
    } else if long_dispatch {
        "area-script-long"
    } else if scripted {
        "area-script"
    } else if area {
        "area"
    } else {
        "input"
    };
    let submitter = if scripted { "" } else { "&go=search" };
    let mut app = App::new();
    app.resize(1000, 700);
    app.open_address(&search.url(&format!("/home-{kind}")));
    wait(&mut app);
    assert_eq!(app.document.title, "Fixture home");
    if long_dispatch {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-dispatch"),
            Some("ready")
        );
    }
    if deferred_body {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-function-body"),
            Some("ready")
        );
    }
    if symbol_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-symbol"),
            Some("ready")
        );
    }
    if accessor_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-accessor"),
            Some("ready")
        );
    }
    if callable_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-callable"),
            Some("ready")
        );
    }
    if descriptor_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-descriptors"),
            Some("ready")
        );
    }
    if wrapper_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-wrappers"),
            Some("ready")
        );
    }
    if iterator_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-iterators"),
            Some("ready")
        );
    }
    if spread_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-spread"),
            Some("ready")
        );
    }
    if promise_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-promise"),
            Some("ready")
        );
    }
    if builtin_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-builtins"),
            Some("ready")
        );
    }
    if template_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-templates"),
            Some("ready")
        );
    }
    if function_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-functions"),
            Some("ready")
        );
    }
    if optional_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-optional"),
            Some("ready")
        );
    }
    for (active, attribute) in [
        (defaults_bootstrap, "data-defaults"),
        (named_regexp_bootstrap, "data-namedregexp"),
        (patterns_bootstrap, "data-patterns"),
        (dates_bootstrap, "data-dates"),
        (object_spread_bootstrap, "data-objectspread"),
        (string_object_bootstrap, "data-stringobjects"),
        (operators_bootstrap, "data-operators"),
        (url_object_bootstrap, "data-urlobjects"),
        (query_params_bootstrap, "data-queryparams"),
        (combined_041_bootstrap, "data-combined041"),
    ] {
        if active {
            assert_eq!(app.document.attr(id(&app, "f"), attribute), Some("ready"));
        }
    }
    edit_query(&mut app, "alpha");
    if area && !scripted {
        click_control(&mut app, "send");
    } else {
        app.key(0x0d, false, false, false);
    }
    wait(&mut app);
    let first = format!("/search-{kind}?q=alpha&source=fixture{submitter}");
    assert!(search.seen(&first));
    assert_eq!(app.location.display(), search.url(&first));
    assert_eq!(app.document.title, "Fixture results");
    let rendered = app
        .page
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(rendered.contains("Original fixture result"), "{rendered}");
    assert!(rendered.contains("Independent text shown"), "{rendered}");
    let run = app
        .page
        .runs
        .iter()
        .find(|run| run.link.is_some() && run.text.contains("Original"))
        .unwrap();
    let (x, y) = (run.x + 3, run.y + app.content_top() - app.scroll + 3);
    assert_eq!(
        app.page.link_at(x, y - app.content_top() + app.scroll),
        Some("/visit?entry=original")
    );
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    wait(&mut app);
    assert!(search.seen("/visit?entry=original"));
    assert!(result.seen("/result"));
    assert_eq!(app.location.display(), result.url("/result"));
    assert_eq!(app.document.title, "External fixture result");
    app.key(0x25, false, false, true); // Actual Alt+Left browser action.
    wait(&mut app);
    assert_eq!(app.location.display(), search.url(&first));
    edit_query(&mut app, "beta + café");
    if scripted {
        app.key(0x0d, false, false, false);
    } else {
        click_control(&mut app, "send");
    }
    wait(&mut app);
    let second = format!("/search-{kind}?q=beta+%2B+caf%C3%A9&source=fixture{submitter}");
    assert!(search.seen(&second));
    assert_eq!(app.location.display(), search.url(&second));
    assert_eq!(
        app.forms.controls[&id(&app, "q")].value.text(),
        "beta + café"
    );
    assert_eq!(app.document.title, "Fixture results");
    if long_dispatch {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-dispatch"),
            Some("ready")
        );
    }
    if deferred_body {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-function-body"),
            Some("ready")
        );
    }
    if symbol_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-symbol"),
            Some("ready")
        );
    }
    if accessor_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-accessor"),
            Some("ready")
        );
    }
    if callable_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-callable"),
            Some("ready")
        );
    }
    if descriptor_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-descriptors"),
            Some("ready")
        );
    }
    if wrapper_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-wrappers"),
            Some("ready")
        );
    }
    if iterator_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-iterators"),
            Some("ready")
        );
    }
    if spread_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-spread"),
            Some("ready")
        );
    }
    if promise_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-promise"),
            Some("ready")
        );
    }
    if builtin_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-builtins"),
            Some("ready")
        );
    }
    if template_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-templates"),
            Some("ready")
        );
    }
    if function_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-functions"),
            Some("ready")
        );
    }
    if optional_bootstrap {
        assert_eq!(
            app.document.attr(id(&app, "f"), "data-optional"),
            Some("ready")
        );
    }
    for (active, attribute) in [
        (defaults_bootstrap, "data-defaults"),
        (named_regexp_bootstrap, "data-namedregexp"),
        (patterns_bootstrap, "data-patterns"),
        (dates_bootstrap, "data-dates"),
        (object_spread_bootstrap, "data-objectspread"),
        (string_object_bootstrap, "data-stringobjects"),
        (operators_bootstrap, "data-operators"),
        (url_object_bootstrap, "data-urlobjects"),
        (query_params_bootstrap, "data-queryparams"),
        (combined_041_bootstrap, "data-combined041"),
    ] {
        if active {
            assert_eq!(app.document.attr(id(&app, "f"), attribute), Some("ready"));
        }
    }
    let image = app.draw();
    assert_eq!(image.width, 1000);
    assert_eq!(image.height, 700);
}

#[test]
fn input_search_renders_results_follows_cross_origin_result_and_searches_again() {
    run_search_flow(false, false, "");
}

#[test]
fn textarea_search_renders_results_follows_cross_origin_result_and_searches_again() {
    run_search_flow(true, false, "");
}

#[test]
fn textarea_enter_edits_lines_and_submit_cancellation_preserves_the_current_document() {
    let search = Site::new(None);
    let mut app = App::new();
    app.open_address(&search.url("/home-area"));
    wait(&mut app);
    edit_query(&mut app, "first");
    app.key(0x0d, false, false, false);
    app.paste("second");
    let q = id(&app, "q");
    assert_eq!(app.forms.controls[&q].value.text(), "first\nsecond");
    assert_eq!(app.location.display(), search.url("/home-area"));
    assert_eq!(search.requests.lock().unwrap().len(), 1);
    edit_query(&mut app, "prevent newline");
    app.key(0x0d, false, false, false);
    assert_eq!(app.forms.controls[&q].value.text(), "prevent newline");
    edit_query(&mut app, "cancel");
    click_control(&mut app, "send");
    assert!(!app.is_loading());
    assert_eq!(app.document.title, "Fixture home");
    assert_eq!(search.requests.lock().unwrap().len(), 1);
    edit_query(&mut app, "allowed");
    click_control(&mut app, "send");
    wait(&mut app);
    assert!(search.seen("/search-area?q=allowed&source=fixture&go=search"));
}

#[test]
fn textarea_scripted_enter_submits_snapshots_and_subsequent_searches() {
    run_search_flow(true, true, "");
}

#[test]
fn script_form_submit_bypasses_required_but_stop_discards_the_queued_request() {
    let search = Site::new(None);
    let mut app = App::new();
    app.open_address(&search.url("/home-area-script-required"));
    wait(&mut app);
    click_control(&mut app, "send");
    assert!(
        !app.is_loading(),
        "native button must retain required-field validation"
    );
    assert_eq!(search.requests.lock().unwrap().len(), 1);
    click_control(&mut app, "q");
    app.key(0x0d, false, false, false);
    assert!(
        app.is_loading(),
        "script submit should queue an empty required field"
    );
    app.stop_loading();
    app.poll_load();
    assert_eq!(
        search.requests.lock().unwrap().len(),
        1,
        "Stop must discard the unsent snapshot"
    );
    assert_eq!(
        app.location.display(),
        search.url("/home-area-script-required")
    );
    edit_query(&mut app, "");
    app.key(0x0d, false, false, false);
    wait(&mut app);
    assert!(search.seen("/search-area-script?q=&source=fixture"));
}

#[test]
fn form_action_csp_blocks_native_submits_with_scripting_enabled_or_disabled_and_script_submits() {
    let search = Site::new(None);
    for scripts in [true, false] {
        let mut app = App::new();
        app.set_javascript_enabled(scripts);
        app.open_address(&search.url("/home-area-csp"));
        wait(&mut app);
        edit_query(&mut app, "private");
        let count = search.requests.lock().unwrap().len();
        click_control(&mut app, "send");
        app.poll_load();
        assert!(!app.is_loading());
        assert_eq!(search.requests.lock().unwrap().len(), count);
        assert!(
            app.status.contains("Content Security Policy"),
            "{}",
            app.status
        );
    }
    let mut app = App::new();
    app.open_address(&search.url("/home-area-script-csp"));
    wait(&mut app);
    edit_query(&mut app, "private");
    let count = search.requests.lock().unwrap().len();
    app.key(0x0d, false, false, false);
    app.poll_load();
    assert_eq!(search.requests.lock().unwrap().len(), count);
    let page = app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(), 1, "{}", page.report());
    assert!(
        page.report().contains("Content Security Policy"),
        "{}",
        page.report()
    );
}

#[test]
fn scripted_post_preserves_body_and_reload_confirmation() {
    let search = Site::new(None);
    let mut app = App::new();
    app.open_address(&search.url("/home-area-post"));
    wait(&mut app);
    edit_query(&mut app, "café +&");
    app.key(0x0d, false, false, false);
    wait(&mut app);
    assert_eq!(app.document.title, "Script POST result");
    assert!(app.current_request.is_post());
    assert_eq!(
        search.posts.lock().unwrap().as_slice(),
        &[(
            "/post-result?keep=1".into(),
            "q=caf%C3%A9+%2B%26&source=fixture".into()
        )]
    );
    app.reload();
    assert!(
        app.resubmission.is_some(),
        "scripted POST must not silently replay on reload"
    );
    assert_eq!(search.posts.lock().unwrap().len(), 1);
}

#[test]
fn script_form_cross_origin_action_and_redirect_make_no_forbidden_request() {
    let sink = Site::new(None);
    let search = Site::new(Some(sink.url("")));
    for path in ["/home-area-script-cross", "/home-area-script-redirect"] {
        let mut app = App::new();
        app.open_address(&search.url(path));
        wait(&mut app);
        edit_query(&mut app, "private");
        app.key(0x0d, false, false, false);
        app.wait_for_load();
        assert_eq!(app.location.display(), search.url(path));
        assert!(
            sink.requests.lock().unwrap().is_empty(),
            "automatic form work must never reach the other origin"
        );
        if path.ends_with("cross") {
            let page = app.scripting.as_ref().unwrap();
            assert_eq!(page.error_count(), 1, "{}", page.report());
            assert!(
                page.report().contains("scheme, host and port"),
                "{}",
                page.report()
            );
        } else {
            assert!(search.seen("/form-redirect?q=private&source=fixture"));
            assert!(app.failed, "the origin-changing redirect must fail");
            assert!(app.status.contains("origin"), "{}", app.status);
        }
    }
}

#[test]
fn long_else_if_bootstrap_initializes_scripted_search_and_repeated_search() {
    run_search_flow(true, true, "long");
}

#[test]
fn deferred_function_bootstrap_initializes_scripted_search_and_repeated_search() {
    run_search_flow(true, true, "deep");
}

#[test]
fn symbol_bootstrap_initializes_scripted_search_and_repeated_search() {
    run_search_flow(true, true, "symbol");
}

#[test]
fn passive_accessor_probe_initializes_scripted_search_and_repeated_search() {
    run_search_flow(true, true, "accessor");
}

#[test]
fn callable_object_helpers_bootstrap_bound_search_and_repeated_search() {
    run_search_flow(true, true, "callable");
}

#[test]
fn literal_accessors_and_array_descriptors_bootstrap_search_and_repeated_search() {
    run_search_flow(true, true, "descriptors");
}

#[test]
fn primitive_wrappers_and_conversion_hooks_bootstrap_search_and_repeated_search() {
    run_search_flow(true, true, "wrappers");
}

#[test]
fn iterator_protocol_and_iterable_helpers_bootstrap_search_and_repeated_search() {
    run_search_flow(true, true, "iterators");
}

#[test]
fn spread_and_rest_bootstrap_search_and_repeated_search() {
    run_search_flow(true, true, "spread");
}

#[test]
fn promise_thenables_and_microtasks_submit_search_and_repeated_search() {
    run_search_flow(true, true, "promise");
}

#[test]
fn common_builtins_callbacks_prepare_search_and_repeated_search() {
    run_search_flow(true, true, "builtins");
}

#[test]
fn template_interpolation_prepares_search_and_repeated_search() {
    run_search_flow(true, true, "templates");
}

#[test]
fn dynamic_function_handlers_submit_search_and_repeated_search() {
    run_search_flow(true, true, "functions");
}

#[test]
fn optional_chaining_and_nullish_values_submit_search_and_repeated_search() {
    run_search_flow(true, true, "optional");
}

#[test]
fn default_parameter_closures_submit_search_and_repeated_search() {
    run_search_flow(true, true, "defaults");
}

#[test]
fn named_regexp_groups_prepare_search_and_repeated_search() {
    run_search_flow(true, true, "namedregexp");
}

#[test]
fn destructuring_parameters_and_bindings_submit_search_and_repeated_search() {
    run_search_flow(true, true, "patterns");
}

#[test]
fn utc_dates_and_json_callbacks_submit_search_and_repeated_search() {
    run_search_flow(true, true, "dates");
}

#[test]
fn object_spread_getters_and_symbols_submit_search_and_repeated_search() {
    run_search_flow(true, true, "objectspread");
}

#[test]
fn string_objects_snapshot_queries_for_search_and_repeated_search() {
    run_search_flow(true, true, "stringobjects");
}

#[test]
fn logical_assignments_and_power_submit_search_and_repeated_search() {
    run_search_flow(true, true, "operators");
}

#[test]
fn url_objects_navigate_results_follow_links_and_search_again() {
    run_search_flow(true, true, "urlobjects");
}

#[test]
fn url_search_params_roundtrip_queries_and_repeated_search() {
    run_search_flow(true, true, "queryparams");
}

#[test]
fn promise_string_url_pipeline_navigates_and_searches_again() {
    run_search_flow(true, true, "combined041");
}


#[test]
fn address_search_enter_uses_one_query_component_and_typing_does_not_contact_provider() {
    let destination=Site::new(None);let site=Site::new(Some(destination.url("")));
    let mut app=App::new();app.open_address(&site.url("/home-input"));wait(&mut app);
    app.input_search_provider=Some(crate::search::local_fixture(&site.url("/search-input")).unwrap());
    for engine in [crate::profile::SearchEngine::Google,crate::profile::SearchEngine::Bing,crate::profile::SearchEngine::DuckDuckGo] {
        let mut data=app.profile.data().clone();data.settings.search_engine=engine;app.profile.save(data).unwrap();
        let count=site.requests.lock().unwrap().len();
        app.key(0x4c,true,false,false);assert!(app.focus==Focus::Address);
        let query=format!("C++ & C# café 🦀 {}",engine.name());
        for ch in query.chars() {app.text_input(ch);}
        assert_eq!(app.address.text(),query);
        assert!(app.pending.is_none());assert_eq!(site.requests.lock().unwrap().len(),count);
        app.key(0x0d,false,false,false);wait(&mut app);
        assert!(site.seen(&format!("/search-input?q=C%2B%2B+%26+C%23+caf%C3%A9+%F0%9F%A6%80+{}",engine.name())));
        assert_eq!(site.requests.lock().unwrap().len(),count+1);
        assert!(app.page.runs.iter().any(|run|run.text.contains("C++")),"Real fixture result text is rendered");
    }
}
