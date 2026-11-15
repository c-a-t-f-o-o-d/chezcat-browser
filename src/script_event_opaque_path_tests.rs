//! Original native/opaque Event path lookup and native-method owner regressions.
use super::*;

fn page(source:&str)->(Document,Forms,Page) {
    let mut doc=crate::html::parse("<html><head></head><body><div id=parent><button id=button type=button>Run</button></div></body></html>");
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);
    let mut page=Page::for_test();
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://example.test/page").unwrap().0),path:None,root:None,fragment:String::new()});
    let outcome=page.start(&mut doc,&mut forms,&[Script {node:0,label:"Original opaque Event path and owner fixture".into(),source:source.into()}]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    (doc,forms,page)
}

fn evaluate(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str) {
    let outcome=page.console_eval(doc,forms,source);
    assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn native_event_first_child_method_read_keeps_real_owner_path_identity_and_expiry() {
    let (mut doc,mut forms,mut page)=page(r#"
var RootFunction=Function,P=Event.prototype,canonicalPath=P.composedPath;
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var ChildFunction=frame.contentWindow.Function;
var read=ChildFunction('e','return [e.initEvent,e.initUIEvent,e.preventDefault,e.stopPropagation,e.stopImmediatePropagation,e.composedPath];');
var button=document.getElementById('button'),parent=document.getElementById('parent'),savedNative,savedMethods,nativeCalls=0;
var expected=[button,parent,document.body,document.documentElement,document,window];
button.addEventListener('keydown',function(e){
 nativeCalls++;savedNative=e;
 // This is the first lookup of these actual native-event handles' methods.
 savedMethods=read(e);var again=read(e);
 for(var i=0;i<savedMethods.length;i++){
  if(Object.getPrototypeOf(savedMethods[i])!==RootFunction.prototype||Object.getPrototypeOf(savedMethods[i])===ChildFunction.prototype||savedMethods[i]!==again[i])throw 'native first/cached lookup owner Function';
 }
 if(savedMethods[5]!==canonicalPath||e.composedPath!==canonicalPath||!('composedPath' in e)||Object.getOwnPropertyDescriptor(e,'composedPath')!==undefined)throw 'native actual inherited path method';
 var path=e.composedPath();
 if(!Array.isArray(path)||path.length!==expected.length)throw 'native complete light-DOM path';
 for(var i=0;i<path.length;i++)if(path[i]!==expected[i])throw 'native actual path identity';
 path[0]=window;if(e.composedPath()[0]!==button)throw 'native independent path arrays';
 if(!e.isTrusted||!e.composed)throw 'actual native input state';
});
"#);
    let node=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("button")).unwrap();
    page.begin_job();
    let outcome=page.keyboard(&mut doc,&mut forms,KeyboardInput {down:true,target:Some(node),key:"a".into(),code:"KeyA".into(),key_code:65,..KeyboardInput::default()});
    assert_eq!(outcome.errors,0,"{}",page.report());
    let outcome=page.console_eval(&mut doc,&mut forms,"savedNative.composedPath;");
    assert_eq!(outcome.errors,1,"Expired native handles must not gain a prototype tail.");
    evaluate(&mut page,&mut doc,&mut forms,r#"
var converted=0,badType={toString:function(){converted++;return 'forbidden';}},caught=0;
try{savedMethods[5].call(savedNative);}catch(e){if(e.name==='TypeError')caught++;}
try{savedMethods[0].call(savedNative,badType);}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==2||converted!==0||nativeCalls!==1)throw 'expired native receiver refused before coercion';
var live=document.createEvent('UIEvent');savedMethods[1].call(live,'real-borrow',false,false,null,9);
if(live.type!=='real-borrow'||live.detail!==9||live.isTrusted||live.composedPath!==canonicalPath||live.composedPath().length)throw 'saved native UI method borrows only live genuine receiver';
"#);
}

#[test]
fn opaque_event_path_is_canonical_inherited_and_keeps_foreign_and_dispatch_boundaries() {
    let (mut doc,mut forms,mut page)=page(r#"
var RootFunction=Function,P=Event.prototype,canonicalPath=P.composedPath;
var ui=document.createEvent('UIEvent'),mouse=document.createEvent('MouseEvent'),custom=document.createEvent('CustomEvent'),payload={value:97};
ui.initUIEvent('opaque',true,true,window,3);mouse.initMouseEvent('opaque',true,true);custom.initCustomEvent('opaque',true,true,payload);
for(var e of [ui,mouse,custom]){
 if(e.composedPath!==canonicalPath||Object.getPrototypeOf(e.composedPath)!==RootFunction.prototype||!('composedPath' in e)||Object.getOwnPropertyDescriptor(e,'composedPath')!==undefined)throw 'opaque real Event path tail';
 var path=e.composedPath();if(!Array.isArray(path)||path.length||path===e.composedPath())throw 'opaque undispatched independent empty path';
}
P.composedPath=function(){return this===ui?'actual-override':'other';};
if(ui.composedPath()!=='actual-override')throw 'real prototype lookup observes override';P.composedPath=canonicalPath;
var button=document.getElementById('button'),parent=document.getElementById('parent'),delivered=0;
var expected=[button,parent,document.body,document.documentElement,document,window];
button.addEventListener('opaque',function(e){
 if(e!==ui&&e!==custom)throw 'unexpected opaque delivery';
 var path=e.composedPath();if(path.length!==expected.length)throw 'opaque complete path';
 for(var i=0;i<path.length;i++)if(path[i]!==expected[i])throw 'opaque actual path identity';
 delivered++;if(e===custom&&e.detail!==payload)throw 'real CustomEvent detail';
});
if(!button.dispatchEvent(ui)||!button.dispatchEvent(custom)||delivered!==2||ui.composedPath().length||custom.composedPath().length)throw 'actual opaque dispatch path cleanup';
var refused=false;try{button.dispatchEvent(mouse);}catch(e){refused=e.name==='NotSupportedError';}
if(!refused||delivered!==2||mouse.target!==null)throw 'Mouse dispatch remains explicitly deferred';
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var ChildFunction=frame.contentWindow.Function,read=ChildFunction('e','return e.composedPath;');
var savedRead=ChildFunction('e','p','return p.call(e);'),savedInit=ui.initUIEvent;
var callInit=ChildFunction('e','f','t','return f.call(e,t,false,false,null,0);');
var converted=0,type={toString:function(){converted++;return 'changed';}};
if(read(ui)!==canonicalPath||savedRead(ui,canonicalPath).length)throw 'same-origin child path lookup';
"#);
    for source in ["Object.getPrototypeOf(ui);","Object.getPrototypeOf(mouse);","Object.getPrototypeOf(custom);"] {
        let outcome=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(outcome.errors,1,"Immediate subclass prototypes stay explicitly opaque.");
    }
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://other.test/").unwrap().0),path:None,root:None,fragment:String::new()});
    for source in ["read(ui);","savedRead(ui,canonicalPath);","callInit(ui,savedInit,type);"] {
        let outcome=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(outcome.errors,1,"A cached canonical method grants no foreign access.");
        evaluate(&mut page,&mut doc,&mut forms,"if(converted!==0||ui.type!=='opaque')throw 'foreign native access before coercion or mutation';");
    }
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://example.test/page").unwrap().0),path:None,root:None,fragment:String::new()});
    evaluate(&mut page,&mut doc,&mut forms,"callInit(ui,savedInit,type);if(converted!==1||ui.type!=='changed'||read(ui)!==canonicalPath||ui.composedPath().length||custom.detail!==payload)throw 'actual same-origin path/method recovery';");
}
