//! Existing CSSOM and slottable contracts with actual mutation delivery.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut d = crate::html::parse(
        "<html><body><div id=mount style='color:#123456' lang=en></div></body></html>",
    );
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
fn style_slot_notifications_cssom_and_existing_reflections_have_real_old_attributes() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('mount'),obs=new MutationObserver(function(){throw 'drained inline records';});obs.observe(node,{attributes:true,attributeOldValue:true,attributeFilter:['style','lang']});var before=node.getAttribute('style');node.style.color='#40a020';var first=node.getAttribute('style');node.style.setProperty('opacity','0.5');var second=node.getAttribute('style');if(node.style.removeProperty('opacity')!=='0.5')throw 'actual old CSS value';node.lang='fr';var records=obs.takeRecords();if(records.length!==4||records[0].target!==node||records[0].attributeName!=='style'||records[0].oldValue!==before||records[1].oldValue!==first||records[2].oldValue!==second||records[3].attributeName!=='lang'||records[3].oldValue!=='en'||node.getAttribute('lang')!=='fr')throw 'CSSOM and reflected attribute records';var saved=node.getAttribute('style');node.style.color='#40a020';node.style.setProperty('color','#40a020');node.style.removeProperty('width');node.style.opacity='invalid';if(obs.takeRecords().length||node.getAttribute('style')!==saved)throw 'no-op CSSOM fabricated mutation';
"#,
    );
}
#[test]
fn style_slot_notifications_identical_callback_writes_keep_scheduled_peers_without_loop() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('mount'),calls=0,peers=0;var first=new MutationObserver(function(records){calls++;if(calls!==1||records.length!==1||records[0].attributeName!=='style')throw 'CSS observer loop';node.style.color='#40a020';});var second=new MutationObserver(function(records){peers++;if(peers!==1||records.length!==1)throw 'CSS observer lost peer';node.style.setProperty('color','#40a020');});first.observe(node,{attributes:true});second.observe(node,{attributes:true});node.style.color='#40a020';if(calls||peers)throw 'synchronous style callback';
"#,
    );
    run(&mut d,&mut f,&mut p,"if(calls!==1||peers!==1||first.takeRecords().length||second.takeRecords().length)throw 'actual style callback/peer counts';");
}
#[test]
fn style_slot_notifications_inert_slot_name_and_slottable_getters_use_real_owner_data() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var owner=document.implementation.createHTMLDocument('slots'),slot=owner.createElement('slot'),span=owner.createElement('span'),text=owner.createTextNode('text');slot.setAttribute('name','old');owner.body.appendChild(slot);owner.body.appendChild(span);span.appendChild(text);var descriptor=Object.getOwnPropertyDescriptor(HTMLSlotElement.prototype,'name'),obs=new MutationObserver(function(){throw 'drained inert slot records';});obs.observe(owner,{attributes:true,attributeOldValue:true,subtree:true,attributeFilter:['name']});if(descriptor.get.call(slot)!=='old'||slot.name!=='old')throw 'real inert slot getter';var calls=0;descriptor.set.call(slot,{toString:function(){calls++;return 'new';}});slot.name=null;span.setAttribute('slot','null');var records=obs.takeRecords();if(calls!==1||records.length!==2||records[0].target!==slot||records[0].attributeName!=='name'||records[0].oldValue!=='old'||records[1].oldValue!=='new'||slot.name!=='null'||slot.getAttribute('name')!=='null'||slot.assignedSlot!==null||span.assignedSlot!==null||text.assignedSlot!==null)throw 'real slot reflection and ordinary-tree assignment';var copy=document.importNode(slot,true);if(copy.name!=='null'||copy.ownerDocument!==document||copy.assignedSlot!==null)throw 'real imported slot';var failed=false;try{descriptor.set.call(span,{toString:function(){calls++;return 'bad';}});}catch(e){failed=e instanceof TypeError;}if(!failed||calls!==1)throw 'slot brand before conversion';
"#,
    );
}

#[test]
fn style_slot_notifications_refusal_preserves_style_data_and_pending_records() {
    let (mut d, mut f, mut p) = fixture();
    run(&mut d,&mut f,&mut p,"var node=document.getElementById('mount'),before=node.getAttribute('style'),obs=new MutationObserver(function(){throw 'refused style callback';});obs.observe(node,{attributes:true,attributeOldValue:true});");
    let limit = p.state.retained_limit;
    p.state.retained_limit = p.state.retained;
    let generation = p.state.dom_commits;
    let source = format!("node.style.fontFamily='{}';", "OriginalFamily".repeat(80));
    let out = p.console_eval(&mut d, &mut f, &source);
    assert_eq!(out.errors, 1);
    assert_eq!(p.state.dom_commits, generation);
    p.state.retained_limit = limit;
    run(&mut d,&mut f,&mut p,"if(node.getAttribute('style')!==before||obs.takeRecords().length)throw 'style refusal leaked data or record';node.style.color='#40a020';var records=obs.takeRecords();if(records.length!==1||records[0].oldValue!==before||records[0].target!==node)throw 'style observer recovery';");
}
