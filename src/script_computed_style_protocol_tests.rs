//! Protocol/security controls separate from the frozen original renderer RED.
use super::*;

fn fixture() -> (Document, Forms, Page) {
    let mut doc=crate::html::parse("<!doctype html><head><link id=sheet rel=stylesheet href=theme.css></head><body><section id=parent><div id=target class=one>Original</div></section><form id=f><input id=neighbor value=original></form>");
    doc.scripting_enabled = true;
    let sheet = doc
        .nodes
        .iter()
        .position(|node| matches!(&node.kind,NodeKind::Element{id:Some(id),..} if id=="sheet"))
        .unwrap();
    let mut sheets = HashMap::new();
    sheets.insert(
        sheet,
        ".one{--value:first}.two{--value:second}.fresh{--own:detached}".into(),
    );
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let location = crate::navigation::address(
        "https://computed-style.test/protocol",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&location);
    page.set_resource_base(&location);
    page.set_geometry(crate::script_geometry::Geometry::new(
        800,
        600,
        1.0,
        &sheets,
        HashMap::new(),
        &crate::fonts::Book::empty(),
    ));
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
}

#[test]
fn computed_style_protocol_fresh_handles_conversion_brands_readonly_and_replaceable_window_operation(
) {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var target=document.getElementById('target'),g=getComputedStyle,a=g(target),b=g(target),read=a.getPropertyValue,hits=0,marker={};
if(a===b||a===target.style||typeof read!=='function'||read.name!=='getPropertyValue'||read.length!==1||g.length!==1||g.name!=='getComputedStyle')throw 'fresh genuine declaration/method shape';
if(read.call(b,'--value').trim()!=='first'||g(target,null).getPropertyValue('--value').trim()!=='first'||g(target,undefined).getPropertyValue('--value').trim()!=='first')throw 'borrowed same-owner declaration and null pseudo';
var caught=0,effect={toString:function(){hits++;throw marker;}};
for(var receiver of [null,undefined,{},window,target,target.style,document]){try{read.call(receiver,effect);}catch(e){if(e instanceof TypeError)caught++;else throw e;}}
if(caught!==7||hits!==0)throw 'declaration brand before DOMString effects';
caught=0;for(var bad of [null,undefined,{},window,document,target.firstChild]){try{g(bad,{toString:function(){hits++;return '';}});}catch(e){if(e instanceof TypeError)caught++;else throw e;}}
if(caught!==6||hits!==0)throw 'Element brand before pseudo conversion';
try{g.call({},target,effect);throw 'bad Window accepted';}catch(e){if(!(e instanceof TypeError))throw e;}
try{read.call(a);throw 'required property missing';}catch(e){if(!(e instanceof TypeError))throw e;}
try{g();throw 'required Element missing';}catch(e){if(!(e instanceof TypeError))throw e;}
try{read.call(a,Symbol());throw 'Symbol converted';}catch(e){if(!(e instanceof TypeError))throw e;}
var key={toString:function(){hits++;target.className='two';return '--value';}};
if(read.call(a,key).trim()!=='second'||hits!==1)throw 'name conversion once before live cascade';
try{read.call(a,effect);throw 'abrupt conversion ignored';}catch(e){if(e!==marker)throw e;}
if(hits!==2||read.call(a,'--value').trim()!=='second')throw 'same abrupt value and recovery';
var pseudo={toString:function(){hits++;target.className='one';return '';}};
if(g(target,pseudo).getPropertyValue('--value').trim()!=='first'||hits!==3)throw 'pseudo conversion before current cascade';
try{a.setProperty('--value','forbidden');throw 'computed declaration written';}catch(e){if(e.name!=='NoModificationAllowedError')throw e;}
try{a.removeProperty('--value');throw 'computed declaration removed';}catch(e){if(e.name!=='NoModificationAllowedError')throw e;}
if(read.call(a,'--value').trim()!=='first'||target.getAttribute('style')!==null)throw 'readonly refusal altered author style';
try{new read('--value');throw 'native read constructed';}catch(e){if(!(e instanceof TypeError))throw e;}
try{new g(target);throw 'Window operation constructed';}catch(e){if(!(e instanceof TypeError))throw e;}
getComputedStyle=function(){return 'replacement';};if(window.getComputedStyle()!=='replacement'||g(target).getPropertyValue('--value').trim()!=='first')throw 'replaceable global with saved native';
delete window.getComputedStyle;if(typeof getComputedStyle!=='undefined')throw 'deleted Window operation ghost';window.getComputedStyle=g;
"#,
    );
}

#[test]
fn computed_style_protocol_retained_declaration_is_empty_detached_and_live_after_reconnection() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var target=document.getElementById('target'),parent=document.getElementById('parent'),saved=getComputedStyle(target),read=saved.getPropertyValue;
if(read.call(saved,'--value').trim()!=='first')throw 'attached original';
parent.removeChild(target);target.className='two';
if(read.call(saved,'--value')!==''||getComputedStyle(target).getPropertyValue('--value')!==''||target.ownerDocument!==document)throw 'disconnected declaration is empty';
parent.appendChild(target);if(read.call(saved,'--value').trim()!=='second')throw 'retained declaration did not reconnect';
var detached=document.createElement('section');detached.className='fresh';var d=getComputedStyle(detached);
if(d.getPropertyValue('--own')!=='')throw 'fresh detached computed stylesheet value leaked';document.body.appendChild(detached);
if(d.getPropertyValue('--own').trim()!=='detached')throw 'fresh declaration did not become connected';
"#,
    );
}

#[test]
fn computed_style_protocol_child_boundaries_and_cached_foreign_access_precede_coercion() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var target=document.getElementById('target'),saved=getComputedStyle(target),read=saved.getPropertyValue,g=getComputedStyle,hits=0;
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,child=frame.contentDocument.body,F=w.Function;
try{g(child,{toString:function(){hits++;return '';}});throw 'child renderer fabricated';}catch(e){if(e.name!=='NotSupportedError')throw e;}
try{g.call(w,target);throw 'child Window renderer fabricated';}catch(e){if(e.name!=='NotSupportedError')throw e;}
if(hits!==0||read.call(saved,'--value').trim()!=='first')throw 'child boundary leaked conversion or changed main';
var foreignRead=F('read','saved','key','return read.call(saved,key);'),foreignGet=F('g','target','pseudo','return g(target,pseudo);');
if(foreignRead(read,saved,'--value').trim()!=='first')throw 'real same-origin borrowed access';
"#,
    );
    let commits = page.state.dom_commits;
    let original = crate::navigation::address(
        "https://computed-style.test/protocol",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    let foreign = crate::navigation::address(
        "https://other-style.test/page",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&foreign);
    for source in [
        "foreignRead(read,saved,{toString:function(){hits++;return '--value';}});",
        "foreignGet(g,target,{toString:function(){hits++;return '';}});",
    ] {
        let out = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(out.errors, 1, "{}", page.report());
        assert_eq!(page.state.dom_commits, commits);
    }
    page.set_location(&original);
    run(&mut doc,&mut forms,&mut page,"if(hits!==0||read.call(saved,'--value').trim()!=='first')throw 'foreign refusal leaked coercion or owner context';");
}

#[test]
fn computed_style_protocol_atomic_handle_admission_and_explicit_unavailable_reads() {
    let (mut doc, mut forms, mut page) = fixture();
    let node = doc
        .nodes
        .iter()
        .position(|node| matches!(&node.kind,NodeKind::Element{id:Some(id),..} if id=="target"))
        .unwrap();
    let retained = page.state.retained;
    let handles = page.state.computed_styles.len();
    let commits = page.state.dom_commits;
    page.state.retained_limit = retained;
    {
        let mut bridge = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        };
        assert!(bridge
            .create_computed_style(&[Value::Host(node_handle(node))])
            .is_err());
    }
    assert_eq!(page.state.retained, retained);
    assert_eq!(page.state.computed_styles.len(), handles);
    assert_eq!(page.state.dom_commits, commits);
    page.state.retained_limit = retained + 1024;
    run(&mut doc,&mut forms,&mut page,"var target=document.getElementById('target'),saved=getComputedStyle(target),read=saved.getPropertyValue;if(read.call(saved,'--missing')!=='')throw 'actual absent';");
    for source in [
        "read.call(saved,'color');",
        "read.call(saved,'--bad name');",
        "saved.cssText;",
        "saved.color='red';",
    ] {
        let out = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(out.errors, 1, "{source}\n{}", page.report());
        assert_eq!(page.state.dom_commits, commits);
    }
    page.state.geometry = None;
    let out = page.console_eval(&mut doc, &mut forms, "read.call(saved,'--value');");
    assert_eq!(out.errors, 1, "{}", page.report());
    assert_eq!(page.state.dom_commits, commits);
    assert_eq!(doc.attr(node, "style"), None);
}
