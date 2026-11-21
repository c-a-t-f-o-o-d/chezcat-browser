//! Ordinary author fields on genuine CharacterData keep native data separate.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(
        "<html><body><p id=loaded>seed<!--comment--></p><div id=mount></div></body></html>",
    );
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(
        &crate::navigation::address(
            "https://node-properties.test/source",
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
fn character_data_author_properties_main_text_comment_instruction_are_real_vm_fields() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var text=document.getElementById('loaded').firstChild,comment=text.nextSibling,created=document.createTextNode('fresh'),instruction=document.createProcessingInstruction('target','payload'),symbol=Symbol('author');
function author(node){var state={node:node};node.bindingState=state;if(node.bindingState!==state||node.bindingState.node!==node)throw 'actual author object';node[symbol]=37;Object.defineProperty(node,'authorAccessor',{configurable:true,get:function(){if(this!==node)throw 'receiver';return this.bindingState;}});if(node.authorAccessor!==state||node[symbol]!==37||Object.keys(node).indexOf('bindingState')<0||Object.getOwnPropertySymbols(node).indexOf(symbol)<0)throw 'real descriptor and keys';if(node.data===undefined||node.nodeValue!==node.data)throw 'native data routes';var before=node.data;node.bindingState='\ud800';if(node.bindingState.charCodeAt(0)!==0xd800||node.data!==before)throw 'VM string versus native DOM';node.bindingState=state;return state;}
for(var node of [text,comment,created,instruction])author(node);
var clone=created.cloneNode(true);if(clone.bindingState!==undefined||clone.data!=='fresh'||Object.getPrototypeOf(clone)!==Text.prototype)throw 'clone copies DOM not JS fields';if(created.parentNode!==null)throw 'real detached node';document.getElementById('mount').appendChild(created);if(created.bindingState.node!==created||created.ownerDocument!==document)throw 'connection preserves metadata';
"#,
    );
}
#[test]
fn character_data_author_properties_inert_created_parsed_imported_and_staged_promotion_aliases() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var template=document.createElement('template');template.innerHTML='<p>inert<!--comment--></p>';var parent=template.content.firstElementChild,text=parent.firstChild,comment=text.nextSibling,owner=text.ownerDocument,created=owner.createTextNode('made'),instruction=owner.createProcessingInstruction('target','data');
for(var node of [text,comment,created,instruction]){var state={node:node};Object.defineProperty(node,'bindingState',{value:state,writable:true,configurable:true,enumerable:true});if(node.bindingState!==state||node.bindingState.node!==node||node.data===undefined)throw 'inert native owner';}
var imported=document.importNode(text,true);if(imported.bindingState!==undefined||imported.data!=='inert'||imported.ownerDocument!==document)throw 'import independent author state';imported.bindingState={held:imported};document.body.appendChild(imported);var cloned=text.cloneNode(true);if(cloned.bindingState!==undefined||cloned.data!=='inert')throw 'inert clone independent author state';var saved=text.bindingState;document.body.appendChild(template.content);if(text.ownerDocument!==document||text.bindingState!==saved||text.bindingState.node!==text)throw 'promoted staged alias';
"#,
    );
}

#[test]
fn character_data_author_properties_do_not_change_native_aliases_observation_or_reserved_fields() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var text=document.getElementById('loaded').firstChild,state={},get=Object.getOwnPropertyDescriptor(Node.prototype,'textContent').get,observer=new MutationObserver(function(){});observer.observe(text,{characterData:true,characterDataOldValue:true});text.bindingState=state;Object.defineProperty(text,'authorValue',{value:37,configurable:true});if(observer.takeRecords().length||text.data!=='seed'||text.nodeValue!=='seed'||get.call(text)!=='seed')throw 'author data changed DOM';text.data='changed';var records=observer.takeRecords();if(records.length!==1||records[0].target!==text||records[0].oldValue!=='seed'||text.nodeValue!=='changed'||get.call(text)!=='changed'||text.bindingState!==state)throw 'native character data';text.nodeValue='second';observer.takeRecords();text.textContent='third';if(text.data!=='third'||text.bindingState!==state||observer.takeRecords().length!==1)throw 'native aliases after backing';var effects=0,bad={toString:function(){effects++;return 'bad';}};
"#,
    );
    // The existing native setter refuses this reserved field. Author backing
    // must not turn that refusal into an own property or coerce the value.
    let out = page.console_eval(&mut doc, &mut forms, "text.length=bad;");
    assert_eq!(out.errors, 1, "{}", page.report());
    assert!(page.report().contains("Element is inert or unavailable"));
    run(&mut doc, &mut forms, &mut page, "if(text.length!==5||effects!==0||Object.keys(text).indexOf('length')>=0||text.bindingState!==state||text.data!=='third')throw 'reserved native length guard';");
}
#[test]
fn character_data_author_properties_saved_foreign_operations_revalidate_before_author_code() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var text=document.getElementById('loaded').firstChild,hits=0,state={};Object.defineProperty(text,'bindingState',{configurable:true,get:function(){hits++;return state;},set:function(value){hits++;state=value;}});var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,read=w.Function('node','return node.bindingState;'),write=w.Function('node','value','node.bindingState=value;');if(read(text)!==state||hits!==1)throw 'same origin author read';write(text,37);if(state!==37||hits!==2)throw 'same origin author write';
"#,
    );
    page.set_location(
        &crate::navigation::address(
            "https://other-node-properties.test/",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    for source in ["read(text);", "write(text,41);"] {
        let out = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(out.errors, 1, "{source}: {}", page.report());
        assert!(page.report().contains("origin") || page.report().contains("Security"));
    }
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "if(hits!==2||state!==37||text.data!=='seed')throw 'denied author effects';",
    );
}
#[test]
fn character_data_author_properties_vm_quota_refusal_keeps_native_data_and_recovers() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "var text=document.getElementById('loaded').firstChild,state={};text.bindingState=state;",
    );
    let generation = page.state.dom_commits;
    let limit = page.runtime.event_property_allowance_for_test(0);
    let out = page.console_eval(&mut doc, &mut forms, "text.unpublishedState=17;");
    page.runtime.event_property_allowance_for_test(limit);
    assert_eq!(out.errors, 1, "{}", page.report());
    assert_eq!(page.state.dom_commits, generation);
    run(&mut doc,&mut forms,&mut page,"if(Object.prototype.hasOwnProperty.call(text,'unpublishedState')||text.bindingState!==state||text.data!=='seed')throw 'failed property publication';text.recoveredState=41;if(text.recoveredState!==41)throw 'next job recovery';");
}
