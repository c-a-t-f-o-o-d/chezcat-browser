//! Genuine inert observer records, tree boundaries and native microtask delivery.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<html><body><div id=mount></div></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{}", page.report());
}
#[test]
fn inert_observation_real_owner_subtrees_attributes_moves_and_fragment_records() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var owner=document.implementation.createHTMLDocument('observed'),a=owner.createElement('div'),b=owner.createElement('div'),moving=owner.createElement('span');owner.body.appendChild(a);owner.body.appendChild(b);
var obs=new MutationObserver(function(){throw 'takeRecords must drain callback';});obs.observe(owner,{childList:true,subtree:true,attributes:true,attributeOldValue:true});
a.appendChild(moving);moving.setAttribute('data-note','one');moving.setAttribute('data-note','two');b.appendChild(moving);b.removeChild(moving);var records=obs.takeRecords();
if(records.length!==6||records[0].target!==a||records[0].addedNodes[0]!==moving||records[1].type!=='attributes'||records[1].target!==moving||records[1].attributeName!=='data-note'||records[1].oldValue!==null||records[2].oldValue!=='one'||records[3].target!==a||records[3].removedNodes[0]!==moving||records[4].target!==b||records[4].addedNodes[0]!==moving||records[5].removedNodes[0]!==moving)throw 'actual inert ordered records';
var fragment=owner.createDocumentFragment(),text=owner.createTextNode('fragment');fragment.appendChild(text);fragment.appendChild(moving);obs.observe(fragment,{childList:true});b.appendChild(fragment);records=obs.takeRecords();if(records.length!==2||records[0].target!==fragment||records[0].removedNodes.length!==2||records[0].removedNodes[0]!==text||records[0].removedNodes[1]!==moving||records[1].target!==b||records[1].addedNodes[0]!==text||records[1].addedNodes[1]!==moving||fragment.childNodes.length)throw 'actual inert donor and insertion';
"#,
    );
    let generation = page.state.dom_commits;
    let out = page.console_eval(&mut doc, &mut forms, "b.appendChild(b);");
    assert_eq!(out.errors, 1);
    assert_eq!(page.state.dom_commits, generation);
    run(&mut doc,&mut forms,&mut page,"if(obs.takeRecords().length!==0||moving.parentNode!==b)throw 'refused mutation fabricated records';");
}
#[test]
fn inert_observation_template_boundaries_and_microtask_order_are_real() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var template=document.createElement('template');template.innerHTML='<div id="inside"></div>';document.getElementById('mount').appendChild(template);var content=template.content,inside=content.firstChild,owner=content.ownerDocument,child=owner.createElement('span'),trace='';
var outer=new MutationObserver(function(){throw 'template contents are not host descendants';});outer.observe(document,{childList:true,attributes:true,subtree:true});
var inner=new MutationObserver(function(records,self){if(trace!=='before|'||records.length!==2||records[0].target!==inside||records[0].addedNodes[0]!==child||records[1].target!==child||records[1].attributeName!=='data-note'||self!==inner)throw 'actual inert callback order and values';trace+='observer|';});inner.observe(content,{childList:true,attributes:true,subtree:true});queueMicrotask(function(){trace+='before|';});inside.appendChild(child);child.setAttribute('data-note','real');queueMicrotask(function(){if(trace!=='before|observer|')throw 'native microtask order';trace+='after|';});if(trace!=='')throw 'observer ran synchronously';
"#,
    );
    run(&mut doc,&mut forms,&mut page,"if(trace!=='before|observer|after|'||outer.takeRecords().length!==0)throw 'genuine delivery/template boundary';");
}

#[test]
fn inert_observation_character_data_old_values_clones_and_refusal_recovery() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var seed=document.createElement('template');seed.innerHTML='<div>original<!--comment--></div>';var root=seed.content.firstChild,text=root.firstChild,comment=text.nextSibling,owner=text.ownerDocument,copy=text.cloneNode(true),fresh=owner.createTextNode('fresh');root.appendChild(fresh);
var obs=new MutationObserver(function(){throw 'drained records';});obs.observe(root,{subtree:true,characterData:true,characterDataOldValue:true});text.data='changed';comment.nodeValue=null;fresh.textContent='native';var records=obs.takeRecords();if(records.length!==3||records[0].target!==text||records[0].oldValue!=='original'||records[1].target!==comment||records[1].oldValue!=='comment'||records[2].target!==fresh||records[2].oldValue!=='fresh'||records[0].type!=='characterData'||text.data!=='changed'||comment.data!==''||copy.data!=='original'||fresh.data!=='native'||text.ownerDocument!==owner)throw 'real inert CharacterData and independent clone';
"#,
    );
    let limit = page.state.retained_limit;
    page.state.retained_limit = page.state.retained;
    let commits = page.state.dom_commits;
    let failed = page.console_eval(&mut doc, &mut forms, "text.data='blocked payload';");
    assert_eq!(failed.errors, 1);
    assert_eq!(page.state.dom_commits, commits);
    page.state.retained_limit = limit;
    run(&mut doc,&mut forms,&mut page,"if(text.data!=='changed'||obs.takeRecords().length)throw 'refusal queued an inert record';text.textContent='recovered';var result=obs.takeRecords();if(result.length!==1||result[0].oldValue!=='changed'||text.data!=='recovered')throw 'inert CharacterData recovery';");
}

#[test]
fn inert_observation_saved_registrations_follow_real_promotion() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var t=document.createElement('template');t.innerHTML='<div id="observed"><span></span></div>';var content=t.content,node=content.firstChild,child=node.firstChild,owner=node.ownerDocument;
var observer=new MutationObserver(function(){throw 'drained promotion records';});observer.observe(node,{attributes:true,attributeOldValue:true,subtree:true});var donor=new MutationObserver(function(){throw 'drained donor';});donor.observe(content,{childList:true});var main=new MutationObserver(function(){throw 'drained main';});main.observe(document.getElementById('mount'),{childList:true});
document.getElementById('mount').appendChild(content);node.setAttribute('data-note','connected');child.setAttribute('data-note','descendant');var records=observer.takeRecords(),removed=donor.takeRecords(),added=main.takeRecords();if(records.length!==2||records[0].target!==node||records[1].target!==child||removed.length!==1||removed[0].removedNodes[0]!==node||added.length!==1||added[0].addedNodes[0]!==node||node.ownerDocument!==document||owner===document||!node.isConnected)throw 'registered identities through actual promotion';
"#,
    );
}
