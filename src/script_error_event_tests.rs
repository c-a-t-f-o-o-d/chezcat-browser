//! Original ErrorEvent construction, coercion, owner and storage checks.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(
        "<html><body><div id=outer><div id=target>Original</div></div></body></html>",
    );
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(
        &crate::navigation::address(
            "https://error-event-owned.test/source",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
}
#[test]
fn error_event_fields_defaults_brands_usv_filename_and_saved_roots() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var C=ErrorEvent,marker={message:'real'},e=new C('error',{message:'broken',filename:'source.js',lineno:-1,colno:4294967297,error:marker,bubbles:true,cancelable:true,composed:true});
if(Object.getPrototypeOf(C)!==Event||Object.getPrototypeOf(C.prototype)!==Event.prototype||Object.getPrototypeOf(e)!==C.prototype||!(e instanceof C)||!(e instanceof Event)||e instanceof UIEvent||e instanceof CustomEvent||Object.prototype.toString.call(e)!=='[object ErrorEvent]')throw 'genuine ancestry';
if(e.message!=='broken'||e.filename!=='source.js'||e.lineno!==4294967295||e.colno!==1||e.error!==marker||!e.bubbles||!e.cancelable||!e.composed||e.isTrusted)throw 'real fields';
var d=new C('error');if(d.message!==''||d.filename!==''||d.lineno!==0||d.colno!==0||d.error!==undefined||d.bubbles||d.cancelable||d.composed||d.isTrusted)throw 'defaults';
var u=new C('error',{message:'\ud800',filename:'\ud800\udc00\ud800'});if(u.message.charCodeAt(0)!==0xd800||u.filename!=='\ud800\udc00\ufffd')throw 'DOMString versus USVString';
var g=Object.getOwnPropertyDescriptor(C.prototype,'error'),pd=Object.getOwnPropertyDescriptor(C,'prototype');if(g.set!==undefined||!g.enumerable||!g.configurable||g.get.name!=='get error'||g.get.length!==0||C.length!==1||pd.writable||pd.configurable||pd.enumerable)throw 'descriptors';
var bad=0;for(var f of [function(){C('x');},function(){new C();},function(){g.get.call({});},function(){g.get.call(new Event('x'));},function(){new C('x',3);},function(){document.createEvent('ErrorEvent');},function(){(function(){'use strict';e.error=17;})();}]){try{f();}catch(x){bad++;}}if(bad!==7||e.error!==marker)throw 'readonly brand and factory refusal';
e.initEvent('reset',false,false);if(e.message!=='broken'||e.filename!=='source.js'||e.error!==marker||e.lineno!==4294967295)throw 'base initializer preserved fields';
delete window.ErrorEvent;if(typeof ErrorEvent!=='undefined'||g.get.call(e)!==marker||!(e instanceof C))throw 'saved roots';window.ErrorEvent=17;if(!(d instanceof C))throw 'saved constructor';
"#,
    );
}
#[test]
fn error_event_dictionary_order_any_identity_and_unsigned_numeric_conversion() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var order='',marker={toString:function(){throw 'any was coerced';}},init={get bubbles(){order+='b';return false;},get cancelable(){order+='c';return true;},get composed(){order+='o';return false;},get colno(){order+='C';return {valueOf:function(){order+='N';return -1;}};},get error(){order+='e';return marker;},get filename(){order+='f';return {toString:function(){order+='F';return 'path';}};},get lineno(){order+='l';return 2;},get message(){order+='m';return {toString:function(){order+='M';return 'broken';}};}};
var e=new ErrorEvent({toString:function(){order+='T';return 'error';}},init);if(order!=='TbcoCNefFlmM'||e.error!==marker||e.colno!==4294967295||e.lineno!==2)throw 'ordered dictionary';
for(var v of [NaN,Infinity,-Infinity,4294967296])if(new ErrorEvent('x',{lineno:v,colno:v}).lineno!==0)throw 'uint32';
var later=0,bad=0;for(var v of [Symbol('s'),BigInt(1)]){try{new ErrorEvent('x',{colno:v,get error(){later++;return marker;}});}catch(x){if(!(x instanceof TypeError))throw x;bad++;}}if(bad!==2||later!==0)throw 'numeric conversion before next getter';
var exact={},caught=false;try{new ErrorEvent('x',{filename:{toString:function(){throw exact;}}});}catch(x){caught=x===exact;}if(!caught)throw 'conversion exception identity';
for(var value of [undefined,null,true,17,'text',Symbol('s'),BigInt(2),function(){return 9;}]){var n=new ErrorEvent('error',{error:value});if(n.error!==value)throw 'uncoerced any';}
"#,
    );
}
#[test]
fn error_event_synthetic_dispatch_reports_original_payload_and_cancels_normally() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var target=document.getElementById('target'),outer=document.getElementById('outer'),marker=new TypeError('real'),order='';
outer.addEventListener('error',function(e){if(!(e instanceof ErrorEvent)||e.error!==marker||e.target!==target||e.currentTarget!==outer||e.eventPhase!==3||e.isTrusted)throw 'actual propagation';order+='outer';});
target.addEventListener('error',function(e){if(e.message!=='failed'||e.filename!=='source'||e.lineno!==4||e.colno!==7||e.error!==marker)throw 'retained failure';order+='target;';e.preventDefault();});
var e=new ErrorEvent('error',{message:'failed',filename:'source',lineno:4,colno:7,error:marker,bubbles:true,cancelable:true});if(target.dispatchEvent(e)!==false||order!=='target;outer'||!e.defaultPrevented||e.currentTarget!==null||e.target!==target||e.eventPhase!==0)throw 'dispatch state';
var count=0;window.addEventListener('customerror',function(e){if(e.error!==marker||e.currentTarget!==window)throw 'window callback';count++;});if(!window.dispatchEvent(new ErrorEvent('customerror',{error:marker}))||count!==1)throw 'real window event';
"#,
    );
}
#[test]
fn error_event_child_owner_and_origin_guards() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,C=w.ErrorEvent,marker={},e=new C('error',{error:marker,message:'child'}),get=Object.getOwnPropertyDescriptor(C.prototype,'error').get;
if(C===ErrorEvent||e instanceof ErrorEvent||!(e instanceof C)||!(e instanceof w.Event)||get.call(e)!==marker||e.message!=='child')throw 'real child owner';
"#,
    );
    page.set_location(
        &crate::navigation::address(
            "https://foreign.test/",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    let out = page.console_eval(&mut doc, &mut forms, "get.call(e);");
    assert_eq!(out.errors, 1, "{}", page.report());
    assert!(page.report().contains("origin") || page.report().contains("Security"));
}
#[test]
fn error_event_payload_and_backing_failures_refund_and_recover() {
    let (mut doc, mut forms, mut page) = fixture();
    let before = page.state.retained;
    let count = page.state.created_events.len();
    let limit = page.state.retained_limit;
    let copies = page.state.copy_work;
    let fields = [
        ("message", Value::String("real".into())),
        ("filename", Value::StringUnits(vec![0xd800])),
        ("error", Value::String("failure".into())),
    ];
    for allowance in [0, 256] {
        page.state.retained_limit = before + allowance;
        let result = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        }
        .construct_error_event(WINDOW, &[Value::String("error".into())], Some(&fields));
        assert!(result.is_err());
        assert_eq!(page.state.retained, before);
        assert_eq!(page.state.created_events.len(), count);
    }
    page.state.retained_limit = limit;
    page.state.copy_work = 0;
    assert!(Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state
    }
    .construct_error_event(WINDOW, &[Value::String("error".into())], Some(&fields))
    .is_err());
    assert_eq!(page.state.retained, before);
    assert_eq!(page.state.created_events.len(), count);
    page.state.copy_work = copies;
    run(&mut doc,&mut forms,&mut page,"var e=new ErrorEvent('error',{message:'recovered',error:17});if(e.error!==17||e.message!=='recovered')throw 'recovery';");
}
