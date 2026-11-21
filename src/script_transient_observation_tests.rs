//! Real detached subtrees remain observable until their notification boundary.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut d=crate::html::parse("<html><body><div id=left><span id=child>old</span></div><div id=right><b id=peer></b></div></body></html>");
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
fn transient_observation_detached_attribute_and_text_records_expire_after_drained_notification() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var left=document.getElementById('left'),child=document.getElementById('child'),text=child.firstChild,obs=new MutationObserver(function(){throw 'drained subtree records';});obs.observe(left,{childList:true,attributes:true,attributeOldValue:true,characterData:true,characterDataOldValue:true,subtree:true});left.removeChild(child);child.setAttribute('data-note','detached');text.data='changed';var records=obs.takeRecords();if(records.length!==3||records[0].target!==left||records[0].removedNodes[0]!==child||records[1].target!==child||records[1].attributeName!=='data-note'||records[1].oldValue!==null||records[2].target!==text||records[2].oldValue!=='old')throw 'real detached subtree records';
"#,
    );
    run(&mut d,&mut f,&mut p,"child.setAttribute('data-note','after');text.data='after';if(obs.takeRecords().length)throw 'transient survived notification';");
}
#[test]
fn transient_observation_without_child_records_expires_before_later_microtask() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var left=document.getElementById('left'),child=document.getElementById('child'),calls=0,obs=new MutationObserver(function(records){calls++;if(records.length!==1||records[0].oldValue!==null)throw 'only synchronous detached attribute';});obs.observe(left,{attributes:true,attributeOldValue:true,subtree:true});left.removeChild(child);child.setAttribute('data-note','before');queueMicrotask(function(){child.setAttribute('data-note','after');});if(calls)throw 'synchronous detached delivery';
"#,
    );
    run(
        &mut d,
        &mut f,
        &mut p,
        "if(calls!==1||obs.takeRecords().length)throw 'attribute-only transient lifetime';",
    );
    run(&mut d,&mut f,&mut p,"left.appendChild(child);left.removeChild(child);queueMicrotask(function(){child.setAttribute('data-note','late without any pending record');});");
    run(&mut d,&mut f,&mut p,"if(calls!==1||obs.takeRecords().length)throw 'empty notification failed to remove transients';");
}
#[test]
fn transient_observation_nested_inert_removals_propagate_original_sources_once() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var owner=document.implementation.createHTMLDocument('inert'),root=owner.createElement('div'),leaf=owner.createElement('span');root.appendChild(leaf);owner.body.appendChild(root);var obs=new MutationObserver(function(){throw 'drained inert transient';});obs.observe(owner,{childList:true,attributes:true,subtree:true});obs.observe(root,{childList:true,attributes:true,subtree:true});owner.body.removeChild(root);root.removeChild(leaf);leaf.setAttribute('data-note','real');var records=obs.takeRecords();if(records.length!==3||records[0].target!==owner.body||records[1].target!==root||records[1].removedNodes[0]!==leaf||records[2].target!==leaf||leaf.parentNode!==null)throw 'nested transient propagation/deduplication';
"#,
    );
    run(&mut d,&mut f,&mut p,"leaf.setAttribute('data-note','after');if(obs.takeRecords().length)throw 'inert transient cleanup';");
}
#[test]
fn transient_observation_option_updates_and_disconnect_remove_only_actual_sources() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var left=document.getElementById('left'),right=document.getElementById('right'),child=document.getElementById('child'),peer=document.getElementById('peer'),obs=new MutationObserver(function(){throw 'drained source update';});obs.observe(left,{attributes:true,subtree:true});obs.observe(right,{attributes:true,subtree:true});left.removeChild(child);right.removeChild(peer);obs.observe(left,{attributes:true,subtree:false});child.setAttribute('data-note','ignored');peer.setAttribute('data-note','retained');var records=obs.takeRecords();if(records.length!==1||records[0].target!==peer)throw 'observe update removed wrong transient source';left.appendChild(child);obs.observe(left,{attributes:true,subtree:true});left.removeChild(child);obs.disconnect();child.setAttribute('data-note','disconnected');if(obs.takeRecords().length)throw 'disconnect retained transient source';
"#,
    );
}

#[test]
fn transient_observation_moves_and_whole_content_replacement_keep_actual_removed_nodes() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var left=document.getElementById('left'),right=document.getElementById('right'),child=document.getElementById('child'),obs=new MutationObserver(function(){throw 'drained move records';});obs.observe(left,{attributes:true,subtree:true});right.appendChild(child);child.setAttribute('data-note','moved');var records=obs.takeRecords();if(records.length!==1||records[0].target!==child)throw 'moving source transient';var old=document.createElement('span');old.textContent='old';left.appendChild(old);left.innerHTML='<b>New</b>';old.setAttribute('data-note','replaced');records=obs.takeRecords();if(records.length!==1||records[0].target!==old)throw 'markup removal transient';
var seed=document.createElement('template');seed.innerHTML='<span>Inert</span>';var content=seed.content,inert=content.firstChild;obs.observe(content,{attributes:true,subtree:true});seed.innerHTML='<b>Replacement</b>';inert.setAttribute('data-note','actual');records=obs.takeRecords();if(records.length!==1||records[0].target!==inert||inert.parentNode!==null)throw 'template replacement transient';
"#,
    );
    run(&mut d,&mut f,&mut p,"child.setAttribute('data-note','after');old.setAttribute('data-note','after');inert.setAttribute('data-note','after');if(obs.takeRecords().length)throw 'moved/replaced source did not expire';");
}
#[test]
fn transient_observation_refused_replacement_publishes_no_transient_or_record() {
    let (mut d, mut f, mut p) = fixture();
    run(&mut d,&mut f,&mut p,"var left=document.getElementById('left'),child=document.getElementById('child'),obs=new MutationObserver(function(){throw 'drained refusal';});obs.observe(left,{attributes:true,subtree:true});");
    let limit = p.state.retained_limit;
    p.state.retained_limit = p.state.retained;
    let generation = p.state.dom_commits;
    let out = p.console_eval(
        &mut d,
        &mut f,
        "left.innerHTML='<span>Refused replacement</span>';",
    );
    assert_eq!(out.errors, 1);
    assert_eq!(p.state.dom_commits, generation);
    p.state.retained_limit = limit;
    run(&mut d,&mut f,&mut p,"if(left.firstChild!==child||obs.takeRecords().length)throw 'refused replacement changed actual tree';left.removeChild(child);child.setAttribute('data-note','recovered');var records=obs.takeRecords();if(records.length!==1||records[0].target!==child)throw 'transient refusal recovery';");
    run(&mut d,&mut f,&mut p,"child.setAttribute('data-note','after');if(obs.takeRecords().length)throw 'failed preflight left orphan transient source';");
}
