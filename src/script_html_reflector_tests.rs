//! Canonical inherited HTML reflections and ordinary author descriptor overrides.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut d=crate::html::parse("<html><body><chezcat-example id=target></chezcat-example><input id=neighbor value=seed></body></html>");
    d.scripting_enabled = true;
    let mut f = Forms::new(&d);
    let mut p = Page::with_limits(crate::javascript::limits::BROWSER);
    assert_eq!(p.start(&mut d, &mut f, &[]).errors, 0);
    (d, f, p)
}
fn run(d: &mut Document, f: &mut Forms, p: &mut Page, s: &str) {
    let out = p.console_eval(d, f, s);
    assert_eq!(out.errors, 0, "{}", p.report());
}
#[test]
fn html_reflector_hidden_prototype_and_reactive_own_descriptors_keep_native_attribute_access() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('target'),descriptor=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'hidden'),reads=0,writes=0,state=false;
if(!descriptor||!descriptor.enumerable||!descriptor.configurable||descriptor.get.name!=='get hidden'||descriptor.get.length!==0||descriptor.set.name!=='set hidden'||descriptor.set.length!==1||node.hasOwnProperty('hidden'))throw 'real inherited hidden descriptor';
Object.defineProperty(node,'hidden',{get:function(){reads++;return state;},set:function(value){writes++;state=value;},enumerable:true,configurable:true});node.hidden=true;if(!node.hidden||writes!==1||reads!==1||descriptor.get.call(node)!==false||node.hasAttribute('hidden'))throw 'reactive own property independent of native reflection';
descriptor.set.call(node,true);if(!descriptor.get.call(node)||!node.hasAttribute('hidden')||writes!==1)throw 'saved native setter';delete node.hidden;if(!node.hidden||Object.hasOwn(node,'hidden'))throw 'delete restores native reflection';node.hidden=false;if(node.hasAttribute('hidden')||node.hidden)throw 'native removal';
Object.defineProperty(node,'hidden',{value:17,writable:true,configurable:true});node.hidden=23;if(node.hidden!==23||descriptor.get.call(node)!==false)throw 'ordinary data hidden';delete node.hidden;if(node.hidden!==false||document.getElementById('neighbor').value!=='seed')throw 'native neighbor and property restore';
"#,
    );
}
#[test]
fn html_reflector_common_string_descriptors_have_real_attribute_values_and_brand_order() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('target'),proto=HTMLElement.prototype,calls=0;
for(var name of ['title','lang','dir']){var descriptor=Object.getOwnPropertyDescriptor(proto,name);if(!descriptor||!descriptor.enumerable||!descriptor.configurable||Object.hasOwn(node,name))throw 'HTML string reflection descriptor';descriptor.set.call(node,{toString:function(){calls++;return name==='dir'?'RTL':'mixed';}});if(node.getAttribute(name)!==(name==='dir'?'RTL':'mixed')||descriptor.get.call(node)!==(name==='dir'?'rtl':'mixed'))throw 'real native reflected text';}
if(calls!==3)throw 'one DOMString conversion';var get=Object.getOwnPropertyDescriptor(proto,'hidden').get,set=Object.getOwnPropertyDescriptor(proto,'hidden').set,coercions=0;for(var receiver of [document,document.createTextNode('x'),Object.create(proto)]){var caught=false;try{set.call(receiver,{toString:function(){coercions++;return '';}});}catch(e){caught=e instanceof TypeError;}if(!caught||coercions)throw 'hidden brand before conversion';}
"#,
    );
}

#[test]
fn html_reflector_hidden_nullable_union_and_single_string_hint_conversion() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('target'),d=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'hidden'),calls=0;
for(var v of [false,0,-0,NaN,'',null,undefined]){d.set.call(node,true);d.set.call(node,v);if(d.get.call(node)!==false||node.hasAttribute('hidden'))throw 'false union removes';}
for(var v of [true,1,-1,Infinity,'false','0',BigInt(0)]){d.set.call(node,v);if(d.get.call(node)!==true||node.getAttribute('hidden')!=='')throw 'truthy union sets';}
node.hidden='UnTiL-FoUnD';if(node.getAttribute('hidden')!=='until-found'||node.hidden!=='until-found')throw 'canonical until-found setter';node.setAttribute('hidden','UNTIL-FOUND');if(node.hidden!=='until-found')throw 'case-insensitive getter';
node.hidden={toString:function(){calls++;return 'until-found';},valueOf:function(){throw 'wrong conversion hint';}};if(calls!==1||node.hidden!=='until-found')throw 'one string hint conversion';
var caught=false;try{node.hidden=Symbol('x');}catch(e){caught=e instanceof TypeError;}if(!caught||node.hidden!=='until-found')throw 'symbol fails without mutation';d.set.call(node);if(node.hidden!==false)throw 'missing argument nullable';
"#,
    );
}
#[test]
fn html_reflector_inert_sources_observers_and_wrong_svg_brand() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var inert=document.implementation.createHTMLDocument('inert'),node=inert.createElement('div'),hidden=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'hidden'),observer=new MutationObserver(function(){});
observer.observe(node,{attributes:true,attributeOldValue:true});hidden.set.call(node,true);node.title='native';var records=observer.takeRecords();if(records.length!==2||records[0].target!==node||records[0].attributeName!=='hidden'||records[0].oldValue!==null||records[1].attributeName!=='title'||node.getAttribute('title')!=='native'||!hidden.get.call(node))throw 'inert source and notification';
Object.defineProperty(node,'hidden',{value:22,writable:true,configurable:true});node.hidden=23;if(observer.takeRecords().length!==0||hidden.get.call(node)!==true)throw 'shadow no native record';delete node.hidden;node.hidden=false;records=observer.takeRecords();if(records.length!==1||records[0].oldValue!==''||node.hasAttribute('hidden'))throw 'native remove observation';
var svg=document.createElementNS('http://www.w3.org/2000/svg','svg'),calls=0,caught=false;try{hidden.set.call(svg,{toString:function(){calls++;return '';}});}catch(e){caught=e instanceof TypeError;}if(!caught||calls!==0)throw 'SVG not HTML reflection receiver';
"#,
    );
}
