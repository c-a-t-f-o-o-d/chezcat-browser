//! Atomic whole-content records use real replacement identities and separate roots.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut d =
        crate::html::parse("<html><body><div id=mount><b>Old</b><i>Tail</i></div></body></html>");
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
fn content_observation_main_text_and_markup_have_actual_added_removed_snapshots() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var mount=document.getElementById('mount'),first=mount.firstChild,last=mount.lastChild,obs=new MutationObserver(function(){throw 'drained records';});obs.observe(mount,{childList:true});mount.textContent='New';var text=mount.firstChild;mount.innerHTML='<span>Markup</span><!--kept-->';var span=mount.firstChild,comment=mount.lastChild;mount.textContent=null;var records=obs.takeRecords();if(records.length!==3||records[0].target!==mount||records[0].removedNodes[0]!==first||records[0].removedNodes[1]!==last||records[0].addedNodes[0]!==text||records[1].removedNodes[0]!==text||records[1].addedNodes[0]!==span||records[1].addedNodes[1]!==comment||records[2].removedNodes[0]!==span||records[2].removedNodes[1]!==comment||records[2].addedNodes.length||mount.childNodes.length||text.data!=='New')throw 'real main content snapshots';mount.textContent='';if(obs.takeRecords().length)throw 'empty replacement fabricated mutation';
"#,
    );
}
#[test]
fn content_observation_template_and_inert_fragment_replacements_keep_native_boundaries() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var t=document.createElement('template');t.innerHTML='<span>Old</span>';var content=t.content,old=content.firstChild,owner=content.ownerDocument,obs=new MutationObserver(function(){throw 'drained content records';}),outer=new MutationObserver(function(){throw 'template host boundary';});outer.observe(t,{childList:true,subtree:true});obs.observe(content,{childList:true});t.innerHTML='<section><template><b>Nested</b></template></section><svg><rect width="2" height="3"/></svg>';var section=content.firstChild,svg=content.lastChild;content.textContent='Actual';var text=content.firstChild;var records=obs.takeRecords();if(records.length!==2)throw 'template records length '+records.length;if(records[0].target!==content||records[0].removedNodes[0]!==old||records[0].addedNodes[0]!==section||records[0].addedNodes[1]!==svg||records[1].removedNodes[0]!==section||records[1].removedNodes[1]!==svg||records[1].addedNodes[0]!==text)throw 'template record identities';if(text.data!=='Actual')throw 'inert text data';if(text.ownerDocument!==owner)throw 'inert text owner';if(old.parentNode!==null)throw 'old template parent';if(outer.takeRecords().length)throw 'template host boundary';var copy=document.importNode(content,true);document.getElementById('mount').appendChild(copy);if(document.getElementById('mount').lastChild.data!=='Actual')throw 'actual inert replacement import';
"#,
    );
}

#[test]
fn content_observation_conversion_reentry_and_refusal_leave_real_snapshots_atomic() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var mount=document.getElementById('mount'),obs=new MutationObserver(function(){throw 'drained conversion records';});obs.observe(mount,{childList:true});var calls=0,during;mount.textContent={toString:function(){calls++;mount.innerHTML='<span>During</span>';during=mount.firstChild;return 'After';}};var after=mount.firstChild,records=obs.takeRecords();if(calls!==1||records.length!==2||records[0].addedNodes[0]!==during||records[1].removedNodes[0]!==during||records[1].addedNodes[0]!==after||after.data!=='After')throw 'conversion before actual content preflight';
"#,
    );
    let limit = p.state.retained_limit;
    p.state.retained_limit = p.state.retained;
    let commits = p.state.dom_commits;
    let nodes = d.nodes.len();
    let out = p.console_eval(&mut d, &mut f, "mount.textContent='blocked replacement';");
    assert_eq!(out.errors, 1);
    assert_eq!(p.state.dom_commits, commits);
    assert_eq!(d.nodes.len(), nodes);
    p.state.retained_limit = limit;
    run(&mut d,&mut f,&mut p,"if(mount.firstChild!==after||after.data!=='After'||obs.takeRecords().length)throw 'refused replacement leaked';mount.textContent='Recovery';var records=obs.takeRecords();if(records.length!==1||records[0].removedNodes[0]!==after||records[0].addedNodes[0]!==mount.firstChild||mount.textContent!=='Recovery')throw 'content preflight recovery';");
}

#[test]
fn content_observation_template_inert_identity_fallback_and_refusal_are_atomic() {
    let (mut d, mut f, mut p) = fixture();
    run(&mut d,&mut f,&mut p,"var seed=document.createElement('template');seed.innerHTML='<b>Old</b>';var content=seed.content,old=content.firstChild,obs=new MutationObserver(function(){throw 'drained fallback';});obs.observe(content,{childList:true});");
    let node_limit = p.state.node_limit;
    p.state.node_limit = d.nodes.len();
    run(&mut d,&mut f,&mut p,"seed.innerHTML='<section><template><b>Nested</b></template></section><span>Tail</span>';var first=content.firstChild,last=content.lastChild,records=obs.takeRecords();if(records.length!==1||records[0].removedNodes[0]!==old||records[0].addedNodes[0]!==first||records[0].addedNodes[1]!==last||first.localName!=='section'||first.ownerDocument!==content.ownerDocument)throw 'actual fallback identities';");
    p.state.node_limit = node_limit;
    let retained_limit = p.state.retained_limit;
    p.state.retained_limit = p.state.retained;
    let generation = p.state.dom_commits;
    let before = d.nodes.len();
    let out = p.console_eval(&mut d, &mut f, "seed.innerHTML='<span>Refused</span>';");
    assert_eq!(out.errors, 1);
    assert_eq!(p.state.dom_commits, generation);
    assert_eq!(d.nodes.len(), before);
    p.state.retained_limit = retained_limit;
    run(&mut d,&mut f,&mut p,"if(content.firstChild!==first||content.lastChild!==last||obs.takeRecords().length)throw 'template refusal leaked records or nodes';seed.innerHTML='<span>Recovery</span>';var records=obs.takeRecords();if(records.length!==1||records[0].removedNodes[0]!==first||records[0].removedNodes[1]!==last||records[0].addedNodes[0]!==content.firstChild)throw 'template replacement recovery';");
}

#[test]
fn content_observation_main_fragment_and_inert_element_keep_guards_and_copy_independence() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var owner=document.implementation.createHTMLDocument('text'),element=owner.createElement('div'),old=owner.createTextNode('Old');element.appendChild(old);var obs=new MutationObserver(function(){throw 'drained text replacements';});obs.observe(element,{childList:true});element.textContent='New';var current=element.firstChild,copy=element.cloneNode(true);element.textContent=null;var records=obs.takeRecords();if(records.length!==2||records[0].removedNodes[0]!==old||records[0].addedNodes[0]!==current||records[1].removedNodes[0]!==current||records[1].addedNodes.length||old.parentNode!==null||current.ownerDocument!==owner||copy.textContent!=='New'||element.childNodes.length)throw 'inert Element text replacement';
var fragment=document.createDocumentFragment(),span=document.createElement('span');fragment.appendChild(span);obs.observe(fragment,{childList:true});fragment.textContent='Fragment';var text=fragment.firstChild;fragment.textContent='';records=obs.takeRecords();if(records.length!==2||records[0].removedNodes[0]!==span||records[0].addedNodes[0]!==text||records[1].removedNodes[0]!==text||records[1].addedNodes.length||text.data!=='Fragment'||text.ownerDocument!==document||fragment.childNodes.length)throw 'main fragment text records';
var protectedFragment=document.createDocumentFragment(),input=document.createElement('input');input.value='edited';protectedFragment.appendChild(input);obs.observe(protectedFragment,{childList:true});
"#,
    );
    let generation = p.state.dom_commits;
    let before = d.nodes.len();
    let out = p.console_eval(&mut d, &mut f, "protectedFragment.textContent='blocked';");
    assert_eq!(out.errors, 1);
    assert_eq!(p.state.dom_commits, generation);
    assert_eq!(d.nodes.len(), before);
    run(&mut d,&mut f,&mut p,"if(protectedFragment.firstChild!==input||input.value!=='edited'||obs.takeRecords().length)throw 'native-control text retirement guard';");
}
