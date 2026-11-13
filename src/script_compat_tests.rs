use super::*;

fn page(markup: &str, scripts: &[&str]) -> (Document, Forms, Page, Outcome) {
    let mut doc = crate::html::parse(markup);
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let scripts: Vec<_> = scripts
        .iter()
        .enumerate()
        .map(|(node, source)| Script {
            node,
            source: (*source).into(),
            label: format!("compat script {node}"),
        })
        .collect();
    let mut page = Page::for_test();
    page.set_location(
        &crate::navigation::address(
            "https://www.google.com/search?q=test",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    let outcome = page.start(&mut doc, &mut forms, &scripts);
    (doc, forms, page, outcome)
}

#[test]
fn page_date_clock_is_integral_utc_and_preserves_monotonic_performance_origin() {
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    let (doc, _, page, outcome) = page(
        "<p>UTC clock</p>",
        &[r#"
var now=Date.now(),current=new Date(),mono=performance.now();
if(!Number.isInteger(now)||!Number.isInteger(current.getTime())||!Number.isInteger(mono))throw 'clock units';
if(Date!==window.Date||Date!==globalThis.Date)throw 'global Date';
if(!Number.isFinite(current.getTime())||mono<0||mono>=86400000)throw 'clock origins';
if(Date.parse(current.toISOString())!==current.getTime())throw 'UTC roundtrip';
if(JSON.parse(JSON.stringify({when:new Date(0)})).when!=='1970-01-01T00:00:00.000Z')throw 'Date JSON';
document.title=String(now);
"#],
    );
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let observed: f64 = doc.title.parse().unwrap();
    assert!(observed >= before.min(after) && observed <= before.max(after));
}

#[test]
fn page_date_clock_requires_active_scripting_and_charges_host_work() {
    let (mut doc, mut forms, mut page, outcome) = page("<p>Clock capability</p>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    page.state.host_work = 1;
    let mut bridge = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    };
    let timestamp = bridge.date_now().unwrap();
    assert!(timestamp.is_finite() && timestamp == timestamp.trunc());
    assert_eq!(bridge.state.host_work, 0);
    assert!(bridge.date_now().unwrap_err().contains("DOM work limit"));
    bridge.state.host_work = 10;
    bridge.state.enabled = false;
    assert!(bridge
        .date_now()
        .unwrap_err()
        .contains("scripting is disabled"));
    bridge.state.enabled = true;
    bridge.doc.scripting_enabled = false;
    assert!(bridge
        .date_now()
        .unwrap_err()
        .contains("scripting is disabled"));
}

#[test]
fn dataset_is_live_reflected_and_enumerable_with_stable_identity() {
    let (doc, _, page, outcome) = page(
        "<div id=p data-first-name='Ada' data-x--y='double' data-α='unicode'></div>",
        &[r#"
var p=document.getElementById('p'),d=p.dataset;
if(d!==p.dataset || d.firstName!=='Ada' || d['x-Y']!=='double' || d['α']!=='unicode')throw 'reflection';
if(Object.keys(d).join(',')!=='firstName,x-Y,α')throw 'attribute order';
d.eqid=17;d.empty='';d.nil=null;d.undef=undefined;
if(p.getAttribute('data-eqid')!=='17'|| d.nil!=='null'||d.undef!=='undefined'||!('empty' in d))throw 'conversion';
d.object={toString(){return 'converted';}};
if(d.object!=='converted')throw 'object ToString';
p.setAttribute('data-first-name','Grace');if(d.firstName!=='Grace')throw 'live read';
if(!delete d.firstName||p.hasAttribute('data-first-name')||'firstName' in d)throw 'delete';
if(d['x-y']!==undefined||!delete d['x-y']||d['x-Y']!=='double')throw 'invalid spelling alias';
var desc=Object.getOwnPropertyDescriptor(d,'eqid');
if(desc.value!=='17'||!desc.enumerable||!desc.configurable||!desc.writable)throw 'descriptor';
if(!Object.prototype.hasOwnProperty.call(d,'eqid')||!d.propertyIsEnumerable('eqid')||!('toString' in d))throw 'own or prototype';
Object.assign(d,{lastName:'Hopper'});var clone=Object.assign({},d);
if(clone.lastName!=='Hopper')throw 'assign';
Object.defineProperty(d,'defined',{value:'yes'});if(p.getAttribute('data-defined')!=='yes')throw 'define';
var keys='';for(var key in d){if(key==='eqid')delete d.empty;keys+=key+',';}
if(keys.indexOf('empty,')!==-1)throw 'for in deletion';
document.title='dataset passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "dataset passed");
}

#[test]
fn dataset_validates_keys_and_keeps_native_attribute_readonly() {
    let (doc, _, page, outcome) = page(
        "<p id=p></p>",
        &[r#"
var p=document.getElementById('p'),d=p.dataset,caught='';
try{d['bad-key']='x';}catch(e){caught+=e.name;}
try{d['bad key']='x';}catch(e){caught+=','+e.name;}
try{Object.defineProperty(d,'bad',{value:1,enumerable:false});}catch(e){caught+=','+e.name;}
if(caught!=='SyntaxError,InvalidCharacterError,TypeError')throw caught;
d['']='blank';if(p.getAttribute('data-')!=='blank')throw 'empty key';
d.toString='shadow';if(d.toString!=='shadow'||Object.keys(d).join(',')!==',toString')throw 'override builtins';
delete d.toString;if(typeof d.toString!=='function')throw 'prototype restored';
p.dataset={fake:1};if(p.dataset!==d)throw 'read only dataset';
var strictFailed=false;try{(function(){'use strict';p.dataset={};})();}catch(e){strictFailed=e.name==='TypeError';}
if(!strictFailed)throw 'strict readonly';document.title='dataset validation passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "dataset validation passed");
}

#[test]
fn navigator_expandos_methods_and_descriptors_work_without_replacing_native_facts() {
    let (doc, _, page, outcome) = page(
        "<p>navigator</p>",
        &[r#"
var agent=navigator.userAgent,original=navigator.sendBeacon;
navigator.feature={count:3};navigator.polyfill=function(a){return this===navigator&&a+this.feature.count;};
if(navigator.polyfill(4)!==7||clientInformation.feature!==navigator.feature)throw 'expando or receiver';
navigator.sendBeacon=function(a){return this===navigator&&a==='queued';};
if(!navigator.sendBeacon('queued'))throw 'method override';
if(!delete navigator.sendBeacon||navigator.sendBeacon!==original)throw 'prototype method restored';
Object.defineProperty(navigator,'custom',{value:5,writable:true,enumerable:true,configurable:true});
if(navigator.custom!==5||Object.keys(navigator).join(',')!=='feature,polyfill,custom')throw 'descriptor or enumeration';
if(!delete navigator.custom||'custom' in navigator)throw 'delete';
navigator.userAgent='fake';navigator.cookieEnabled=true;
if(navigator.userAgent!==agent||navigator.cookieEnabled!==true)throw 'native information changed';
if(Object.getOwnPropertyDescriptor(navigator,'userAgent')!==undefined)throw 'inherited native getter';
var failed=false;try{(function(){'use strict';navigator.userAgent='fake';})();}catch(e){failed=e.name==='TypeError';}
if(!failed||!delete navigator.userAgent||navigator.userAgent!==agent)throw 'protected native getter';
document.title='navigator passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "navigator passed");
}

#[test]
fn image_constructor_creates_detached_nodes_without_claiming_a_load() {
    let (doc, _, page, outcome) = page(
        "<p>images</p>",
        &[r#"
var image=new Image(13,17),other=document.createElement('img'),loaded=false;
if(image===other||!(image instanceof Image)||!(other instanceof Image)||({} instanceof Image))throw 'identity';
if(image.tagName!=='IMG'||image.nodeType!==1||image.parentNode!==null||image.parentElement!==null||image.ownerDocument!==document||image.isConnected)throw 'detached';
if(image.width!==13||image.height!==17||image.getAttribute('width')!=='13'||!image.complete||image.src!=='')throw 'initial dimensions';
image.onload=function(){loaded=true;};image.onerror=function(){loaded=true;};
image.id='detached';image.dataset.source='test';image.src='/pixel.png';
if(image.src!=='https://www.google.com/pixel.png'||image.getAttribute('src')!=='/pixel.png'||image.complete||image.currentSrc!==''||image.naturalWidth!==0||image.naturalHeight!==0)throw 'truthful source';
image.setAttribute('src','next.png');if(image.src!=='https://www.google.com/next.png')throw 'src reflection';
image.width=-1;image.height=5.9;if(image.width!==4294967295||image.height!==5)throw 'unsigned long';
image.width={valueOf(){return 18;}};if(image.width!==18)throw 'numeric conversion';
if(document.images.length!==0||document.getElementById('detached')!==null||loaded)throw 'connected or loaded';
document.title='image passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "image passed");
    assert_eq!(page.state.detached_images.len(), 2);
    assert_eq!(page.state.declined_images, 2);
    assert!(page
        .report()
        .contains("no request or load event was fabricated"));
}

#[test]
fn google_adapted_image_and_beacon_snippets_finish_and_decline_network() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<div id=css></div>",
        &[r#"
var src='/gen_204?image=1',iesg=true,cssId='css',eid='local';
window.addEventListener('load',function(){window.n&&window.n();if(document.images){new Image().src=src;}});
typeof navigator.sendBeacon!=='function'&&(navigator.sendBeacon=function(a){(new Image).src=a});
var queued=navigator.sendBeacon('/gen_204?startup=1','');if(queued!==false)throw 'fabricated queue success';
setTimeout(function(){document.getElementById(cssId).setAttribute('style','');if(navigator.sendBeacon('/gen_204?cad=sg_trbl&ei='+eid,'')!==false)throw 'queued';document.title='beacon passed';},0);
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    page.poll_timer_at(
        &mut doc,
        &mut forms,
        Instant::now() + Duration::from_secs(1),
    );
    assert_eq!(page.state.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "beacon passed");
    assert_eq!(page.state.declined_beacons, 2);
    assert_eq!(page.state.declined_images, 1);
    assert_eq!(page.state.timer_callbacks, 1);
}

#[test]
fn element_method_values_are_stable_callable_and_check_their_receiver() {
    let (doc, _, page, outcome) = page(
        "<button id=b>Button</button>",
        &[r#"
var b=document.getElementById('b'),handler=function(){},method=b.addEventListener;
if(typeof method!=='function'||method!==b.addEventListener)throw 'unstable method';
b.dataset.eqid='g1';b&&b.addEventListener?b.addEventListener('click',handler):0;
method.call(b,'click',handler);b.removeEventListener('click',handler);
if(b.getAttribute.call(b,'data-eqid')!=='g1')throw 'extracted native getter';
var failed=false;try{method.call({},'click',handler);}catch(e){failed=e.name==='TypeError';}
if(!failed)throw 'receiver';document.title='native methods passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "native methods passed");
    assert!(page.state.listeners.is_empty());
}

#[test]
fn beacon_rejects_invalid_urls_and_does_not_claim_cross_origin_transmission() {
    let (doc, _, page, outcome) = page(
        "<p>beacons</p>",
        &[r#"
var caught=0;try{navigator.sendBeacon();}catch(e){if(e.name==='TypeError')caught++;}
try{navigator.sendBeacon('file:///private');}catch(e){if(e.name==='TypeError')caught++;}
try{navigator.sendBeacon('javascript:alert(1)');}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==3)throw 'URL validation';
var source={toString(){return 'https://other.example/report';}};
if(navigator.sendBeacon(source,'body')!==false)throw 'cross origin sent';
document.title='beacon validation passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "beacon validation passed");
    assert_eq!(page.state.declined_beacons, 1);
}

#[test]
fn image_growth_is_bounded_and_following_classic_script_recovers() {
    let (doc, _, page, outcome) = page(
        "<p>node limit</p>",
        &[
            "for(var i=0;i<1000;i++){new Image();}",
            "document.title='node limit recovered';",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "node limit recovered");
    assert_eq!(page.state.detached_images.len(), MAX_EXTRA_NODES);
    assert!(page.report().contains("DOM appended-node limit reached"));
}

#[test]
fn classic_script_failure_does_not_poison_later_scripts_or_events() {
    let (doc, _, page, outcome) = page(
        "<p>budgets</p>",
        &[
            "window.addEventListener('load',function(){document.title+=' loaded';});while(true);",
            "document.title='following script';",
            "document.addEventListener('DOMContentLoaded',function(){document.title+=' ready';});",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "following script ready loaded");
    assert!(page.report().contains("step limit"));
    assert_eq!(page.state.executed, 3);
}

#[test]
fn aggregate_startup_allowance_stops_a_chain_of_hostile_classic_scripts() {
    let (doc, _, page, outcome) = page(
        "<p>budgets</p>",
        &[
            "while(true);",
            "while(true);",
            "while(true);",
            "while(true);",
            "while(true);",
            "document.title='should not run';",
        ],
    );
    assert_ne!(doc.title, "should not run");
    assert!(outcome.errors >= 2);
    assert!(page.state.executed <= 4, "{}", page.report());
    assert!(page
        .report()
        .contains("Aggregate startup work/time limit reached"));
}

#[test]
fn many_parameters_work_and_parameter_bound_remains_atomic() {
    let parameters = (0..80)
        .map(|index| format!("p{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let arguments = (0..80)
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let source=format!("function large({parameters}){{return p0+p32+p79+arguments.length;}}var arrow=({parameters})=>p79;if(large({arguments})!==191||large.apply(null,[{arguments}])!==191||arrow({arguments})!==79)throw 'large args';document.title='parameters passed';");
    let (doc, _, page, outcome) = page("<p>arguments</p>", &[&source]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "parameters passed");
    let parameters = (0..257)
        .map(|index| format!("p{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!("document.title='bad';function excessive({parameters}){{}};");
    let (doc, _, page, outcome) = self::page(
        "<p>arguments</p>",
        &[&source, "document.title='recovered';"],
    );
    assert_eq!(outcome.errors, 1);
    assert_eq!(doc.title, "recovered");
    assert!(page
        .report()
        .contains("Function parameter limit reached (256)"));
}

#[test]
fn element_methods_can_be_borrowed_and_inspection_does_not_grant_mutation() {
    let (doc, _, page, outcome) = page(
        "<a id=a href='/original'>A</a><a id=b href='/other' data-α='greek'>B</a><input id=c name=choice type=checkbox>",
        &[r#"
var a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c');
var get=a.getAttribute,set=a.setAttribute;
if(get.call(b,'href')!=='/other'||get.call(c,'type')!=='checkbox'||!c.hasAttribute('name')||b.getAttribute('data-α')!=='greek')throw 'attribute reads';
var conversions='';
set.call(b,{toString(){conversions+='n';return 'data-result';}},{toString(){conversions+='v';return 'converted';}});
if(conversions!=='nv'||b.dataset.result!=='converted'||a.hasAttribute('data-result'))throw 'conversion or receiver';
if(document.getElementById({toString(){return 'b';}})!==b||get.call(b,'missing')!==null)throw 'argument conversion';
var event=function(){};document.addEventListener.call(b,'click',event);a.removeEventListener.call(b,'click',event);
var failures=0;try{get.call({},'id');}catch(e){if(e.name==='TypeError')failures++;}
try{get.call(document,'id');}catch(e){if(e.name==='TypeError')failures++;}
try{get.call(b);}catch(e){if(e.name==='TypeError')failures++;}
try{set.call(b,'data-empty');}catch(e){if(e.name==='TypeError')failures++;}
if(failures!==4)throw 'receiver or required argument';
var premature=false;try{set.call(b,{toString(){premature=true;return 'data-empty';}});}catch(e){}
if(premature)throw 'required arguments must be checked before conversion';
document.title='read methods passed';
"#,
          "document.getElementById('b').setAttribute('href','https://other.example/');",
          "if(document.getElementById('b').getAttribute('href')!=='/other')throw 'unsafe mutation';document.title+=' safely';"],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "read methods passed safely");
    assert!(page.state.listeners.is_empty());
    assert!(page.report().contains("Supported attributes"));
}

#[test]
fn selector_queries_return_static_ordered_lists_and_match_ancestors() {
    let (doc, _, page, outcome) = page(
        "<body><section id=root class=group><p id=a class=card><span id=inner>A</span></p><p id=b class=card>B</p><input id=check type=checkbox checked></section><p id=c class=card>C</p></body>",
        &[r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),inner=document.getElementById('inner');
var list=document.querySelectorAll('.card,#b,.card'),local=root.querySelectorAll('body .card');
if(list.length!==3||local.length!==2||list[0]!==a||list[1]!==b||list[2].id!=='c'||list[3]!==undefined||list['01']!==undefined)throw 'ordered list';
if(list.item(0)!==a||list.item(-1)!==null||list.item(100)!==null||list.item({valueOf(){return 1;}})!==b)throw 'item';
if(Object.keys(list).join(',')!=='0,1,2'||!('0' in list)||'3' in list||list===document.querySelectorAll('.card'))throw 'identity or keys';
if(Object.getOwnPropertyDescriptor(list,'0').writable||Object.getOwnPropertyDescriptor(list,'length')!==undefined)throw 'descriptors';
if(!a.matches('section > p.card')||a.matches('#b')||inner.closest('.group')!==root||a.closest('p')!==a||a.closest('.absent')!==null)throw 'match or closest';
if(a.matches.call(b,'#b')!==true||a.querySelectorAll.call(root,'.card').length!==2||list.item.call(local,1)!==b)throw 'borrowed query';
var check=document.getElementById('check');check.checked=false;
if(check.matches(':checked')||document.querySelectorAll(':checked').length!==0)throw 'checked state';
var image=new Image();if(!image.matches('img')||image.closest('img')!==image||image.closest('body')!==null)throw 'detached match';
var order='',receiver={mark:7};
if(list.forEach(function(node,index,collection){if(this!==receiver||collection!==list)throw 'callback';order+=node.id+index;},receiver)!==undefined||order!=='a0b1c2')throw 'forEach';
var caught=0;try{list.forEach(null);}catch(e){if(e.name==='TypeError')caught++;}
try{list.forEach(function(){throw Error('stop');});}catch(e){if(e.message==='stop')caught++;}
try{list.item();}catch(e){if(e.name==='TypeError')caught++;}
try{root.querySelectorAll('p:unsupported');}catch(e){if(e.name==='SyntaxError')caught++;}
try{a.closest('[');}catch(e){if(e.name==='SyntaxError')caught++;}
try{a.matches();}catch(e){if(e.name==='TypeError')caught++;}
try{(function(){'use strict';list[0]=b;})();}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==7||delete list[0]||!delete list[99])throw 'errors or deletion';
for(var invalid of ['.123','#123','.-1','-1']){var syntax=false;try{a.matches(invalid);}catch(e){syntax=e.name==='SyntaxError';}if(!syntax)throw 'invalid identifier';}
if(a.matches('.--valid'))throw 'valid identifier';
list.length=0;if(list.length!==3)throw 'readonly length';
if(document.querySelector({toString(){return '#b';}},'ignored')!==b)throw 'selector conversion';
document.title='queries passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "queries passed");
}

#[test]
fn selector_snapshots_preserve_detached_nodes_and_keep_live_lookup_separate() {
    let (doc, _, page, outcome) = page(
        "<div id=root><span id=a class=card><b id=inner>old</b></span><span id=b class=card>second</span></div><p id=counter>0</p>",
        &[r#"
var root=document.getElementById('root'),list=root.querySelectorAll('.card'),a=list[0],inner=document.getElementById('inner');
a.className='changed';if(list.length!==2||root.querySelectorAll('.card').length!==1)throw 'static membership';
root.textContent='new';
if(list[0]!==a||a.isConnected||a.id!=='a'||a.textContent!=='old'||a.parentNode!==null||inner.parentNode!==a||inner.closest('span')!==a)throw 'detached identity';
if(document.getElementById('a')!==null||document.getElementById('inner')!==null||document.querySelectorAll('span').length!==0||root.querySelectorAll('*').length!==0)throw 'connected lookup';
if(a.querySelector('b')!==inner||a.querySelectorAll('b')[0]!==inner)throw 'detached subtree';
var visited='';list.forEach(function(node){visited+=node.textContent;});if(visited!=='oldsecond')throw 'snapshot contents';
inner.textContent='retained';if(a.textContent!=='retained'||root.textContent!=='new')throw 'detached mutation';
a.textContent='changed';if(a.textContent!=='changed'||inner.parentNode!==null||inner.textContent!=='retained'||root.textContent!=='new')throw 'detached replacement';
var counter=document.getElementById('counter');for(var i=0;i<1000;i++)counter.textContent=String(i);
if(counter.textContent!=='999')throw 'counter';document.title='snapshot passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "snapshot passed");
    assert_eq!(page.state.detached_nodes.len(), 6);
    assert!(page.state.retained <= page.state.retained_limit);
    let before = crate::html::parse("<div id=root><span id=a class=card><b id=inner>old</b></span><span id=b class=card>second</span></div><p id=counter>0</p>").nodes.len();
    assert_eq!(
        doc.nodes.len(),
        before + 2,
        "counter replacement must reuse its text node"
    );
}

#[test]
fn selector_snapshot_counts_and_retained_references_are_bounded() {
    let (doc, _, page, outcome) = page(
        "<p id=one>one</p>",
        &[
            "for(var i=0;i<129;i++)document.querySelectorAll('p');",
            "document.title='snapshot cap recovered';",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "snapshot cap recovered");
    assert_eq!(page.state.node_lists.len(), MAX_NODE_LISTS);
    assert!(page.report().contains("NodeList snapshot limit"));

    let markup = "<div></div>".repeat(512);
    let (mut doc, mut forms, mut page, outcome) = self::page(&markup, &[
        "var listCount=0;function collect(){document.querySelectorAll('div');listCount++;if(listCount<40)setTimeout(collect,0);}collect();",
    ]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    for _ in 0..40 {
        page.poll_timer_at(
            &mut doc,
            &mut forms,
            Instant::now() + Duration::from_secs(1),
        );
    }
    assert_eq!(page.state.node_list_references, MAX_NODE_LIST_REFERENCES);
    assert_eq!(page.state.node_lists.len(), 32);
    assert!(
        page.report().contains("NodeList retained-reference limit"),
        "{}",
        page.report()
    );
}

#[test]
fn selector_result_and_callback_work_limits_do_not_publish_partial_success() {
    let markup = "<div></div>".repeat(4097);
    let (doc, _, page, outcome) = page(
        &markup,
        &[
            "document.querySelectorAll('div');document.title='incorrect';",
            "document.title='result bound recovered';",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "result bound recovered");
    assert!(page.state.node_lists.is_empty());
    assert_eq!(page.state.node_list_references, 0);
    assert!(page.report().contains("selector result limit"));
    let (doc, _, page, outcome) = self::page("<p>one</p>", &[
        "var caught=false;try{document.querySelectorAll('p').forEach(function(){while(true);});}catch(e){caught=true;}finally{caught=true;}",
        "if(caught)throw 'budget was catchable';document.title='callback bound recovered';",
    ]);
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "callback bound recovered");
    assert!(page.report().contains("step limit"));
}

#[test]
fn detached_snapshot_ids_do_not_shadow_connected_label_targets() {
    let markup = "<div id=host><span id=field>old</span></div><input id=field><label id=label for=field>Click</label><template><input id=field></template>";
    let before = crate::html::parse(markup);
    assert!(
        crate::forms::label_targets(&before).is_empty(),
        "first connected duplicate is not labelable"
    );
    let (doc, forms, page, outcome) = page(
        markup,
        &[r#"
var host=document.getElementById('host'),saved=host.querySelectorAll('span');host.textContent='';
if(saved[0].id!=='field'||saved[0].isConnected||document.getElementById('field').tagName!=='INPUT')throw 'connected IDs';
document.title='label lookup passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "label lookup passed");
    let input = *forms.controls.keys().next().unwrap();
    let label = doc
        .nodes
        .iter()
        .enumerate()
        .find(|(node, _)| doc.attr(*node, "id") == Some("label"))
        .unwrap()
        .0;
    let labels = crate::forms::label_targets(&doc);
    assert_eq!(labels.get(&label), Some(&input));
    assert_eq!(labels.len(), 1);
}

#[test]
fn element_only_selector_methods_are_not_exposed_on_document() {
    let (doc, _, page, outcome) = page("<p>Element selectors</p>", &[
        "document.matches('p');",
        "document.closest('p');",
        "if('matches' in document||'closest' in document)throw 'wrong interface';document.title='interfaces passed';",
    ]);
    assert_eq!(outcome.errors, 2, "{}", page.report());
    assert_eq!(doc.title, "interfaces passed");
    assert!(page.report().contains("Document method"));
}

#[test]
fn dom_creation_insertion_text_and_saved_nodes_keep_their_identity() {
    let (doc, _, page, outcome) = page(
        "<html><head></head><body><div id=root><span id=a>A</span><span id=b>B</span></div><template><p id=hidden>inert</p></template></body></html>",
        &[r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b');
var box=document.createElement('DIV'),text=document.createTextNode({toString(){return 'A😀';}}),last=document.createTextNode('Z');
box.id='new';if(box.tagName!=='DIV'||box.isConnected||box.parentNode!==null)throw 'created element';
if(box.appendChild(text)!==text||box.appendChild(last)!==last||text.nodeType!==3||text.nodeName!=='#text'||text.length!==3||text.data!=='A😀')throw 'text node';
if(box.firstChild!==text||box.lastChild!==last||text.nextSibling!==last||last.previousSibling!==text||text.parentElement!==box||text.ownerDocument!==document)throw 'links';
text.data='new';if(box.textContent!=='newZ')throw 'live text';
if(root.insertBefore(box,b)!==box||document.getElementById('new')!==box||!text.isConnected||box.previousSibling!==a||box.nextSibling!==b)throw 'insert';
var saved=root.querySelectorAll('div');
root.insertBefore(box,box);root.appendChild(a);if(root.firstChild!==box||root.lastChild!==a)throw 'reorder';
if(root.removeChild(box)!==box||box.isConnected||text.isConnected||box.parentNode!==null||text.parentNode!==box||document.getElementById('new')!==null||saved[0]!==box)throw 'remove identity';
root.insertBefore(box,null);if(!box.isConnected||root.lastChild!==box||root.querySelector('#new')!==box)throw 'reinsert';
box.removeChild(text);if(text.parentNode!==null||text.data!=='new'||text.isConnected)throw 'text removal';
text.nodeValue=null;if(text.data!==''||box.textContent!=='Z')throw 'text null';
if(document.getElementById('hidden')!==null||document.querySelectorAll('template,p').length!==0)throw 'inert lookup';
document.title='DOM tree passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "DOM tree passed");
}

#[test]
fn named_form_controls_request_real_focus_and_keep_builtin_priorities() {
    let (doc, forms, mut page, outcome) = page(
        "<html><head></head><body><form id=search name=f><input id=query name=q><input name=textContent><input name=querySelector></form><input id=external name=outside form=search><form name=body></form><form name=getElementById></form><form name=other><input name=q></form><template><form name=inert><input name=q></form></template></body></html>",
        &[r#"
if(document.f!==document.getElementById('search')||!('f' in document)||!('q' in document.f))throw 'named form';
if(document.f.q!==document.getElementById('query')||document.f.query!==document.f.q||document.f.outside!==document.getElementById('external'))throw 'owned controls';
if(document.body.tagName!=='BODY'||typeof document.getElementById!=='function'||typeof document.f.querySelector!=='function'||typeof document.f.textContent!=='string')throw 'native priority';
if(document.missing!==undefined||document.inert!==undefined||document.f.missing!==undefined||'missing' in document.f)throw 'missing or inert name';
document.f&&document.f.q.focus();document.title='named focus passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "named focus passed");
    let query = forms
        .controls
        .keys()
        .copied()
        .find(|&node| doc.attr(node, "id") == Some("query"))
        .unwrap();
    assert_eq!(page.take_focus_request(), Some(query));
    assert_eq!(page.take_focus_request(), None);
}

#[test]
fn ambiguous_named_form_collections_are_explicitly_unavailable() {
    let (doc, _, page, outcome) = page(
        "<form name=duplicate></form><form name=duplicate></form><form name=f><input name=q><input name=q></form>",
        &[
            "var wrong=document.duplicate;document.title='incorrect';",
            "document.f.q.focus();document.title='incorrect';",
            "document.title='named ambiguity recovered';",
        ],
    );
    assert_eq!(outcome.errors, 2, "{}", page.report());
    assert_eq!(doc.title, "named ambiguity recovered");
    assert!(page.report().contains("live HTMLCollection"));
    assert!(page.report().contains("RadioNodeList"));
    assert_eq!(page.state.dom.pending_focus, None);
}

#[test]
fn created_script_properties_and_text_are_real_without_fabricated_loads() {
    let (doc, _, page, outcome) = page(
        "<body><div id=root></div></body>",
        &[r#"
var script=document.createElement('script'),fired=0;
if(!script.async||script.defer||script.noModule||script.src!==''||script.type!=='')throw 'script defaults';
script.src={toString(){return '/payload.js';}};script.type='text/javascript';script.nonce='nonce-1';script.charset='utf-8';
script.async=false;script.defer=true;script.noModule=true;script.crossOrigin='anonymous';script.referrerPolicy='no-referrer';
script.integrity='sha256-original';
if(script.src!=='https://www.google.com/payload.js'||script.getAttribute('src')!=='/payload.js'||script.async||!script.defer||!script.noModule||script.getAttribute('nonce')!=='nonce-1'||script.getAttribute('crossorigin')!=='anonymous')throw 'reflection';
script.setAttribute('src','next.js');script.removeAttribute('defer');if(script.src!=='https://www.google.com/next.js'||script.defer)throw 'attribute mutation';
script.onload=function(){fired++;};script.onerror=function(){fired++;};
var old=document.createTextNode('old');script.appendChild(old);if(script.text!=='old'||script.textContent!=='old')throw 'script text';
script.text='new';if(old.data!=='old'||old.parentNode!==null||script.firstChild.data!=='new')throw 'retained text';
script.textContent=null;if(script.firstChild!==null||script.text!=='')throw 'empty text';
if(script.isConnected||script.parentNode!==null||fired||document.querySelector('script')!==null)throw 'no invented load';
document.title='script objects passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "script objects passed");
}

#[test]
fn connected_created_scripts_queue_once_and_completion_keeps_native_receivers() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<html><head></head><body><div id=root></div></body></html>",
        &[r#"
var first=document.createElement('script'),second=document.createElement('script'),phases='';
window.addEventListener('load',function(e){if(e.target===first){if(e.eventPhase!==1||e.bubbles)throw 'capture phase';phases+='capture,';}},true);
first.src='/one.js';first.async=false;
first.onload=function(e){if(this!==first||e.target!==first||e.currentTarget!==first||e.eventPhase!==2||e.bubbles||window.loadedValue!==17)throw 'load receiver';phases+='target';document.title=phases;};
document.head.appendChild(first);document.head.appendChild(first);
document.head.appendChild(second);second.src='/two.js';
second.onerror=function(e){if(this!==second||e.target!==second||e.bubbles||this.isConnected||this.parentNode!==null)throw 'detached error receiver';document.title+=' + error';};
document.head.removeChild(second);
var data=document.createElement('script');data.type='application/json';data.id='data';data.text='never executed';document.getElementById('root').appendChild(data);
if(!first.isConnected||first.parentNode!==document.head||second.isConnected||!data.isConnected||document.querySelector('script')!==first||document.getElementById('data')!==data||document.getElementById('root').textContent!=='')throw 'physical or inert tree';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let mut requests = page.take_dynamic_scripts();
    assert_eq!(
        requests.len(),
        2,
        "same script must prepare once and data block never prepares"
    );
    let first = requests.pop_front().unwrap();
    let second = requests.pop_front().unwrap();
    assert!(first.error.is_none(), "{:?}", first.error);
    assert!(second.error.is_none(), "{:?}", second.error);
    assert_eq!(first.target.display(), "https://www.google.com/one.js");
    assert_eq!(second.target.display(), "https://www.google.com/two.js");
    let loaded = page.complete_dynamic_script(
        &mut doc,
        &mut forms,
        &first,
        Ok(Script {
            node: first.node,
            source: "window.loadedValue=17;".into(),
            label: "original local fixture".into(),
        }),
    );
    assert_eq!(loaded.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "capture,target");
    let failed = page.complete_dynamic_script(
        &mut doc,
        &mut forms,
        &second,
        Err("fixture fetch failure".into()),
    );
    assert_eq!(failed.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "capture,target + error");
    assert!(page.take_dynamic_scripts().is_empty());
}

#[test]
fn dom_tree_rejections_preserve_existing_controls_resources_and_parent_links() {
    let (doc, forms, page, outcome) = page(
        "<html><head></head><body><div id=root><span id=child>old</span></div><span id=outside>outside</span><form id=form><input id=query value=original></form><img id=image src=pixel.png></body></html>",
        &[
            "var root=document.getElementById('root'),child=document.getElementById('child'),outside=document.getElementById('outside');child.appendChild(root);",
            "root.insertBefore(document.createElement('p'),outside);",
            "document.body.removeChild(document.getElementById('form'));",
            "document.body.appendChild(document.createElement('input'));",
            "document.body.appendChild(document.getElementById('image'));",
            "var text=document.createTextNode('safe');text.appendChild(document.createElement('span'));",
            "if(root.parentNode!==document.body||child.parentNode!==root||root.textContent!=='old'||outside.parentNode!==document.body||document.getElementById('query').value!=='original'||document.images.length!==1||text.data!=='safe')throw 'partial mutation';document.title='mutation limits recovered';",
        ],
    );
    assert_eq!(outcome.errors, 6, "{}", page.report());
    assert_eq!(doc.title, "mutation limits recovered");
    assert_eq!(forms.controls.len(), 1);
    assert!(page.report().contains("HierarchyRequestError"));
    assert!(page.report().contains("NotFoundError"));
    assert!(page.report().contains("loaded resources"));
}

#[test]
fn dom_creation_validates_arguments_and_borrowed_native_receivers() {
    let (doc, _, page, outcome) = page(
        "<body><div id=p></div></body>",
        &[r#"
var p=document.getElementById('p'),text=document.createTextNode('x'),bad=0;
try{document.createElement('bad name');}catch(e){if(e.name==='InvalidCharacterError')bad++;}
try{document.createElement();}catch(e){if(e.name==='TypeError')bad++;}
try{document.createTextNode();}catch(e){if(e.name==='TypeError')bad++;}
try{p.appendChild({});}catch(e){if(e.name==='TypeError')bad++;}
try{p.insertBefore(text);}catch(e){if(e.name==='TypeError')bad++;}
try{p.appendChild.call({},text);}catch(e){if(e.name==='TypeError')bad++;}
var other=document.createElement('span');p.appendChild.call(other,text);
if(bad!==6||text.parentNode!==other||other.firstChild!==text||p.firstChild!==null)throw 'receiver or validation';
text.nodeType=1;if(text.nodeType!==3)throw 'readonly';document.title='DOM arguments passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "DOM arguments passed");
}

#[test]
fn general_dom_creation_shares_the_existing_node_growth_bound() {
    let (doc, _, page, outcome) = page("<p>bounds</p>", &[
        "for(var i=0;i<1000;i++){if(i%3===0)new Image();else if(i%3===1)document.createElement('div');else document.createTextNode('');}",
        "document.title='DOM node cap recovered';",
    ]);
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "DOM node cap recovered");
    assert_eq!(page.state.dom.created.len(), MAX_EXTRA_NODES);
    assert!(page.report().contains("appended-node limit"));
}

#[test]
fn dynamic_dom_depth_is_bounded_and_next_script_can_run() {
    let (doc, _, page, outcome) = page("<p>depth</p>", &[
        "var tree=document.createElement('div'),tail=tree;for(var i=0;i<150;i++){var child=document.createElement('div');tail.appendChild(child);tail=child;}",
        "if(tree.isConnected||tail.parentNode===null)throw 'invalid detached tree';document.title='DOM depth recovered';",
    ]);
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "DOM depth recovered");
    assert!(page.report().contains("tree depth limit"));
}

#[test]
fn tag_collections_are_live_scoped_ordered_and_keep_native_member_priorities() {
    let (doc, _, page, outcome) = page(
        "<body><div id=root><p id=one name=alpha>A</p><p id=two>B</p></div><p id=outside>C</p><img id=image><template><p id=inert>hidden</p></template></body>",
        &[r#"
var root=document.getElementById('root'),one=document.getElementById('one'),two=document.getElementById('two');
var all=document.getElementsByTagName('P'),local=root.getElementsByTagName('p'),saved=root.querySelectorAll('p');
if(all.length!==3||local.length!==2||all[2].id!=='outside'||local.item(0)!==one||local[2]!==undefined||local.item(-1)!==null||local.item(100)!==null)throw 'initial collection';
if(local.namedItem('alpha')!==one||local.one!==one||local.namedItem('')!==null||local.namedItem('inert')!==null||local.namedItem({toString(){return 'two';}})!==two)throw 'named collection';
if(local.item({valueOf(){return 1;}})!==two||Object.keys(local).join(',')!=='0,1'||!('one' in local)||'2' in local)throw 'index reflection';
if(Object.getOwnPropertyDescriptor(local,'0').writable||Object.getOwnPropertyDescriptor(local,'length')!==undefined)throw 'index descriptor';
var child=document.createElement('p');child.id='new';root.insertBefore(child,one);
if(local.length!==3||all.length!==4||local[0]!==child||saved.length!==2||saved[0]!==one)throw 'live insert versus snapshot';
root.appendChild(one);if(local[2]!==one||local.namedItem('new')!==child)throw 'live order';
child.id='renamed';if(local.namedItem('new')!==null||local.namedItem('renamed')!==child)throw 'live rename';
child.id='length';if(local.length!==3||local.namedItem('length')!==child)throw 'length priority';
child.id='item';if(typeof local.item!=='function'||local.namedItem('item')!==child)throw 'method priority';
if(local.item.call(document.images,0)!==document.getElementById('image')||document.images.item.call(local,0)!==child)throw 'borrowed collection method';
local.length=0;local[0]=null;if(local.length!==3||local[0]!==child)throw 'readonly collection';
var bad=0;try{local.item();}catch(e){if(e.name==='TypeError')bad++;}
try{local.namedItem();}catch(e){if(e.name==='TypeError')bad++;}
try{root.getElementsByTagName();}catch(e){if(e.name==='TypeError')bad++;}
try{local.item.call({},0);}catch(e){if(e.name==='TypeError')bad++;}
if(bad!==4)throw 'collection receivers';
root.textContent='replacement';if(local.length!==0||all.length!==1||saved[0]!==one||one.parentNode!==null)throw 'live removal';
root.appendChild(child);if(local.length!==1||local[0]!==child||all.length!==2||root.getElementsByTagName('*').length!==1)throw 'retained live root';
document.title='live tag collections passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "live tag collections passed");
}

#[test]
fn parsed_script_nonce_lookup_is_real_without_activating_inert_contents() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<html><head><script id=seed nonce=YWJj>original_source_only()</script><template><script id=inert nonce=bad>inert_source()</script></template></head><body><p>visible</p></body></html>",
        &[r#"
var scripts=document.getElementsByTagName('SCRIPT'),seed=document.querySelector('script[nonce]');
if(scripts.length!==1||scripts[0]!==seed||seed!==document.getElementById('seed')||seed.nonce!=='YWJj'||seed.getAttribute('nonce')!=='YWJj'||!seed.isConnected)throw 'actual nonce';
if(document.getElementsByTagName('HEAD')[0]!==document.head||document.head.getElementsByTagName('script')[0]!==seed||document.body.getElementsByTagName('script').length!==0)throw 'root scoping';
if(seed.textContent!=='original_source_only()'||seed.text!=='original_source_only()'||document.head.textContent!==''||document.querySelectorAll('script').length!==1||document.getElementById('inert')!==null)throw 'inert source';
var added=document.createElement('script');added.nonce=seed.nonce;added.src='/nonce-script.js';document.head.appendChild(added);
if(scripts.length!==2||scripts[1]!==added||added.nonce!==seed.nonce)throw 'live script nonce';
document.title='nonce lookup passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "nonce lookup passed");
    let requests = page.take_dynamic_scripts();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].nonce.as_deref(), Some("YWJj"));
    let seed = (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some("seed"))
        .unwrap();
    page.begin_job();
    Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    }
    .queue_dynamic_script(seed)
    .unwrap();
    assert!(
        page.take_dynamic_scripts().is_empty(),
        "parsed script must never enter the dynamic queue"
    );
    let styles = crate::css::compute(&doc, &HashMap::new());
    assert_eq!(styles[seed].display, crate::css::Display::None);
}

#[test]
fn parsed_script_tree_mutation_is_rejected_without_partial_source_changes() {
    let (doc, _, page, outcome) = page(
        "<head><script id=seed nonce=YWJj>original()</script></head><body></body>",
        &[
            "var seed=document.querySelector('script[nonce]');seed.appendChild(document.createTextNode('changed()'));",
            "document.head.removeChild(seed);",
            "if(seed.textContent!=='original()'||seed.parentNode!==document.head||!seed.isConnected)throw 'source changed';document.title='parsed script protected';",
        ],
    );
    assert_eq!(outcome.errors, 2, "{}", page.report());
    assert_eq!(doc.title, "parsed script protected");
}

#[test]
fn live_tag_collections_keep_retention_and_repeated_walks_bounded() {
    let (doc, _, page, outcome) = page(
        "<p>collection caps</p>",
        &[
            "for(var i=0;i<1000;i++)document.getElementsByTagName('tag'+i);",
            "document.title='collection cap recovered';",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "collection cap recovered");
    assert_eq!(page.state.dom.collections.len(), 128);
    assert!(page.report().contains("tag-collection limit"));

    let markup = format!("<body>{}</body>", "<div>value</div>".repeat(1000));
    let (doc, _, page, outcome) = self::page(
        &markup,
        &[
            "var live=document.getElementsByTagName('div');for(var i=0;i<1000;i++)live.length;",
            "document.title='live work recovered';",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "live work recovered");
    assert!(page.report().contains("DOM work limit"));
}

#[test]
fn dynamic_function_callbacks_keep_document_string_compilation_policy() {
    for (header, allowed) in [
        ("script-src 'nonce-original-function-fixture'", false),
        ("script-src 'nonce-original-function-fixture' 'unsafe-eval'", true),
        ("default-src 'none'; script-src-elem 'nonce-original-function-fixture' 'unsafe-eval'", false),
        ("script-src 'nonce-original-function-fixture' 'unsafe-eval', script-src 'nonce-original-function-fixture'", false),
    ] {
        let mut policy = crate::csp::PolicySet::default();
        policy.add_header(header);
        let mut doc = crate::html::parse("<p id=out></p>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let location = crate::navigation::address(
            "http://127.0.0.1:1/function-policy",
            &crate::navigation::Location::home(),
        ).unwrap();
        page.set_location(&location);
        let source = r#"
var log='';
function attempt(label){
try{
var body="document.getElementById('out').textContent+=label;";
var compiled=label==='timer'?new Function('label',body):Function('label',body);
compiled(label);log+=label+':allowed;';
}catch(error){if(error.name!=='EvalError')throw error;log+=label+':blocked;';}
document.title=log;
}
attempt('script');
Promise.resolve().then(function(){attempt('promise');});
setTimeout(function(){attempt('timer');},0);
"#;
        assert!(policy.allows_inline(0, Some("original-function-fixture"), source).is_ok());
        let scripts = [Script { node: 0, label: "original Function CSP fixture".into(), source: source.into() }];
        let outcome = page.start_with_policy(&mut doc, &mut forms, &scripts, &policy);
        assert_eq!(outcome.errors, 0, "{header}: {}", page.report());
        let result = if allowed { "allowed" } else { "blocked" };
        assert_eq!(doc.title, format!("script:{result};promise:{result};"));
        let outcome = page.poll_timer_at(&mut doc, &mut forms, Instant::now() + Duration::from_secs(1)).unwrap();
        assert_eq!(outcome.errors, 0, "{header}: {}", page.report());
        assert_eq!(doc.title, format!("script:{result};promise:{result};timer:{result};"));
        let out = doc.nodes.iter().enumerate().find_map(|(id, _)| (doc.attr(id, "id") == Some("out")).then_some(id)).unwrap();
        let text = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }.text_content(out).unwrap();
        assert_eq!(text, if allowed { "scriptpromisetimer" } else { "" });
    }
}


#[test]
fn dynamic_crossorigin_mode_is_snapshotted_and_cross_origin_is_still_denied() {
    let (_, _, mut page, outcome) = page("<html><head></head><body></body></html>", &[r#"
var first=document.createElement('script');first.async=false;first.crossOrigin='anonymous';first.src='/one.js';
document.head.appendChild(first);first.removeAttribute('crossorigin');
var second=document.createElement('script');second.async=false;second.crossOrigin='use-credentials';second.src='/two.js';document.head.appendChild(second);
var third=document.createElement('script');third.async=false;third.crossOrigin='anonymous';third.src='https://other.test/blocked.js';document.head.appendChild(third);
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let requests = page.take_dynamic_scripts();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].same_origin_only && requests[0].error.is_none());
    assert!(requests[1].same_origin_only && requests[1].error.is_none());
    assert!(requests[2].same_origin_only);
    assert!(requests[2].error.as_deref().unwrap().contains("CORS-mode"));
}
