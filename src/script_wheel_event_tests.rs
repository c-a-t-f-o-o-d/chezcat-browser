//! Original WheelEvent construction, coercion, owner and storage checks.
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
            "https://wheel-owned.test/source",
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
fn wheel_event_genuine_constructor_fields_and_inherited_mouse_state() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var W=WheelEvent,e=new W('wheel',{deltaX:-1.25,deltaY:2.5,deltaZ:-0,deltaMode:4294967295,momentum:true,clientX:17.5,screenY:32,button:2,buttons:3,ctrlKey:true,modifierAltGraph:true,view:window,detail:7,bubbles:true,cancelable:true});
if(Object.getPrototypeOf(W)!==MouseEvent||Object.getPrototypeOf(W.prototype)!==MouseEvent.prototype||Object.getPrototypeOf(e)!==W.prototype||e.constructor!==W||!(e instanceof MouseEvent)||!(e instanceof UIEvent)||!(e instanceof Event)||Object.prototype.toString.call(e)!=='[object WheelEvent]')throw 'genuine ancestry';
if(e.deltaX!==-1.25||e.deltaY!==2.5||!Object.is(e.deltaZ,-0)||e.deltaMode!==4294967295||!e.momentum||e.clientX!==17.5||e.screenY!==32||e.button!==2||e.buttons!==3||!e.ctrlKey||!e.getModifierState('AltGraph')||e.view!==window||e.detail!==7||!e.bubbles||!e.cancelable||e.isTrusted)throw 'retained native fields';
var d=new W('wheel');if(d.deltaX!==0||d.deltaY!==0||d.deltaZ!==0||d.deltaMode!==0||d.momentum||d.clientX!==0||d.view!==null)throw 'defaults';
for(var pair of [['DOM_DELTA_PIXEL',0],['DOM_DELTA_LINE',1],['DOM_DELTA_PAGE',2]]){var k=pair[0],v=pair[1],cd=Object.getOwnPropertyDescriptor(W,k);if(W[k]!==v||W.prototype[k]!==v||e[k]!==v||cd.writable||cd.configurable||!cd.enumerable)throw 'constants';}
var g=Object.getOwnPropertyDescriptor(W.prototype,'deltaY'),pd=Object.getOwnPropertyDescriptor(W,'prototype'),gd=Object.getOwnPropertyDescriptor(window,'WheelEvent');
if(g.set!==undefined||!g.enumerable||!g.configurable||g.get.name!=='get deltaY'||g.get.length!==0||W.length!==1||pd.writable||pd.configurable||pd.enumerable||!gd.writable||!gd.configurable||gd.enumerable)throw 'descriptors';
var bad=0;for(var f of [function(){W('x');},function(){new W();},function(){g.get.call({});},function(){g.get.call(new MouseEvent('x'));},function(){new W('x',1);},function(){document.createEvent('WheelEvent');},function(){(function(){'use strict';e.deltaY=8;})();}]){try{f();}catch(x){bad++;}}if(bad!==7||e.deltaY!==2.5)throw 'readonly and refusals';
e.initMouseEvent('reset',false,true,null,3,1,2,4,5,false,true,false,false,1,null);if(e.deltaY!==2.5||e.deltaX!==-1.25||!e.momentum||e.clientX!==4||e.detail!==3||e.type!=='reset')throw 'inherited initializer';
var refused=false;try{document.getElementById('target').dispatchEvent(e);}catch(x){refused=x.name==='NotSupportedError';}if(!refused||e.target!==null||e.isTrusted)throw 'preserved mouse dispatch guard';
delete window.WheelEvent;if(typeof WheelEvent!=='undefined'||!(e instanceof W)||g.get.call(e)!==2.5)throw 'saved roots';window.WheelEvent=17;if(!(d instanceof W))throw 'saved constructor';
"#,
    );
}
#[test]
fn wheel_event_dictionary_order_finite_deltas_and_unsigned_mode() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var order='',init={get bubbles(){order+='b';return true;},get cancelable(){order+='c';return false;},get composed(){order+='o';return true;},get detail(){order+='d';return 2;},get view(){order+='v';return null;},get clientX(){order+='C';return 4;},get deltaMode(){order+='m';return {valueOf:function(){order+='M';return -1;}};},get deltaX(){order+='x';return {valueOf:function(){order+='X';return -2.5;}};},get deltaY(){order+='y';return 3;},get deltaZ(){order+='z';return -0;},get momentum(){order+='p';return true;}};
var e=new WheelEvent({toString:function(){order+='T';return 'wheel';}},init);if(order!=='TbcodvCmMxXyzp'||e.deltaMode!==4294967295||e.deltaX!==-2.5||!e.momentum)throw 'dictionary order and conversions';
var late=0,bad=0;for(var v of [NaN,Infinity,-Infinity,Symbol('s'),BigInt(1)]){try{new WheelEvent('bad',{deltaX:v,get deltaY(){late++;return 1;}});}catch(x){if(!(x instanceof TypeError))throw x;bad++;}}if(bad!==5||late!==0)throw 'finite conversion before later getter';
for(var v of [NaN,Infinity,-Infinity,4294967296]){if(new WheelEvent('x',{deltaMode:v}).deltaMode!==0)throw 'modulo uint32';}
var marker={},same=false;try{new WheelEvent('x',{deltaY:{valueOf:function(){throw marker;}}});}catch(x){same=x===marker;}if(!same)throw 'conversion exception identity';
"#,
    );
}
#[test]
fn wheel_event_child_owner_and_saved_native_getter_obey_origin() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,W=w.WheelEvent,e=new W('wheel',{deltaY:3,view:w}),get=Object.getOwnPropertyDescriptor(W.prototype,'deltaY').get;
if(W===WheelEvent||e instanceof WheelEvent||!(e instanceof W)||!(e instanceof w.MouseEvent)||e.view!==w||get.call(e)!==3)throw 'child owner';
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
    assert!(
        page.report().contains("origin") || page.report().contains("Security"),
        "{}",
        page.report()
    );
}
#[test]
fn wheel_event_payload_refusal_and_base_failure_refund_without_publication() {
    let (mut doc, mut forms, mut page) = fixture();
    let before = page.state.retained;
    let count = page.state.created_events.len();
    let limit = page.state.retained_limit;
    let copies = page.state.copy_work;
    for allowance in [0, 256] {
        page.state.retained_limit = before + allowance;
        let r = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        }
        .construct_wheel_event(WINDOW, &[Value::String("wheel".into())], None);
        assert!(r.is_err());
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
    .construct_wheel_event(WINDOW, &[Value::String("wheel".into())], None)
    .is_err());
    assert_eq!(page.state.retained, before);
    assert_eq!(page.state.created_events.len(), count);
    page.state.copy_work = copies;
    run(&mut doc,&mut forms,&mut page,"var recovered=new WheelEvent('wheel',{deltaY:9});if(recovered.deltaY!==9)throw 'recovery';");
}
