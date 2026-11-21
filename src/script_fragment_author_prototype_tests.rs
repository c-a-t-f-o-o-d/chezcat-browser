//! Original fragment wrappers retain native Node identity and real topology.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<html><body><div id=mount></div></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(
        &crate::navigation::address(
            "https://fragment-prototype.test/source",
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
fn fragment_author_prototype_main_legacy_and_static_changes_keep_native_aliases_and_splice() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var fragment=document.createDocumentFragment(),canonical=DocumentFragment.prototype,state={},reads=0,nativeFirst=Object.getOwnPropertyDescriptor(Node.prototype,'firstChild').get;
function Wrapper(){}Wrapper.prototype=Object.create(canonical,{firstAlias:{get:nativeFirst},binding:{get:function(){reads++;if(this!==fragment)throw 'receiver';return this.state;}}});
fragment.state=state;fragment.__proto__=Wrapper.prototype;
if(Object.getPrototypeOf(fragment)!==Wrapper.prototype||fragment.__proto__!==Wrapper.prototype||!(fragment instanceof Wrapper)||!(fragment instanceof DocumentFragment)||fragment.binding!==state||reads!==1)throw 'real fragment overlay';
var child=document.createElement('span');child.id='original';fragment.appendChild(child);var saved=fragment.childNodes;if(fragment.firstAlias!==child||fragment.querySelector('#original')!==child||child.parentNode!==fragment)throw 'native aliases';
var next=Object.create(Wrapper.prototype,{flag:{value:37}});if(Object.setPrototypeOf(fragment,next)!==fragment||fragment.flag!==37||fragment.firstAlias!==child)throw 'static replacement';
var clone=fragment.cloneNode(true);if(Object.getPrototypeOf(clone)!==canonical||clone.state!==undefined||clone.firstChild===child||clone.firstChild.id!=='original')throw 'clone DOM not author prototype';
document.getElementById('mount').appendChild(fragment);if(saved.length||fragment.firstAlias!==null||fragment.state!==state||Object.getPrototypeOf(fragment)!==next||child.parentNode!==document.getElementById('mount'))throw 'donor splice retains wrapper';
Object.setPrototypeOf(fragment,canonical);if(fragment instanceof Wrapper||fragment.binding!==undefined||fragment.state!==state)throw 'canonical restoration';
"#,
    );
}
#[test]
fn fragment_author_prototype_inert_template_owner_and_import_keep_distinct_prototypes() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var template=document.createElement('template');template.innerHTML='<span id=original>staged</span>';var fragment=template.content,owner=fragment.ownerDocument,created=owner.createDocumentFragment(),P=Object.create(DocumentFragment.prototype,{binding:{get:function(){return this.state;}}});
for(var node of [fragment,created]){node.state={held:node};Object.setPrototypeOf(node,P);if(node.binding.held!==node||Object.getPrototypeOf(node)!==P||!(node instanceof DocumentFragment)||node.ownerDocument!==owner)throw 'inert native wrapper';}
var imported=document.importNode(fragment,true);if(imported.state!==undefined||Object.getPrototypeOf(imported)!==DocumentFragment.prototype||imported.firstChild===fragment.firstChild)throw 'import native constructor';document.body.appendChild(imported);document.getElementById('mount').appendChild(fragment);if(fragment.childNodes.length||Object.getPrototypeOf(fragment)!==P||fragment.binding.held!==fragment||created.ownerDocument!==owner)throw 'inert donor identity';
"#,
    );
}
#[test]
fn fragment_author_prototype_node_owner_connected_accessors_shadow_without_changing_native_facts() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var owner=Object.getOwnPropertyDescriptor(Node.prototype,'ownerDocument'),connected=Object.getOwnPropertyDescriptor(Node.prototype,'isConnected');if(!owner||!connected||typeof owner.get!=='function'||owner.set!==undefined||!owner.enumerable||!owner.configurable||connected.get.length!==0)throw 'genuine Node getters';
var fragment=document.createDocumentFragment(),child=document.createElement('span'),text=document.createTextNode('native');child.appendChild(text);fragment.appendChild(child);if(owner.get.call(fragment)!==document||connected.get.call(fragment)||connected.get.call(child)||owner.get.call(text)!==document||owner.get.call(document)!==null||!connected.get.call(document))throw 'native metadata';
var host=document.getElementById('mount'),P=Object.create(DocumentFragment.prototype,{ownerDocument:{get:function(){return this.host.ownerDocument;}},isConnected:{get:function(){return this.host.isConnected;}}});fragment.host=host;Object.setPrototypeOf(fragment,P);if(fragment.ownerDocument!==document||!fragment.isConnected||connected.get.call(fragment)||fragment.parentNode!==null)throw 'author metadata versus native container';
document.body.appendChild(fragment);if(!connected.get.call(child)||!connected.get.call(text)||connected.get.call(fragment))throw 'real connection after splice';
for(var getter of [owner.get,connected.get]){var caught=0;try{getter.call({});}catch(e){if(e instanceof TypeError)caught++;else throw e;}if(caught!==1)throw 'brand';}
"#,
    );
}
