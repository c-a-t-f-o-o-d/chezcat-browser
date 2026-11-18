//! The reflected HTML editing state and its ordinary, realm-owned accessor.
use super::*;

#[test]
fn content_editable_reflects_own_raw_state_on_html_elements_and_detached_attrs() {
    let (mut doc, mut forms, mut page) = fixture_with(
        "<section id=parent contenteditable='TrUe'><span id=child></span></section>",
    );
    run(&mut doc, &mut forms, &mut page, r#"
var outer=document.getElementById('parent'),child=document.getElementById('child');
if(outer.contentEditable!=='true'||outer.getAttribute('contenteditable')!=='TrUe'||
   child.contentEditable!=='inherit'||child.hasAttribute('contenteditable'))throw 'own state under editable parent';
var detached=document.createElement('span');
for(var entry of [['','true'],['FaLsE','false'],['PlAiNtExT-OnLy','plaintext-only'],['inherit','inherit'],
                  [' true','inherit'],['false ','inherit'],['not-a-state','inherit']]){
    detached.setAttribute('CONTENTEDITABLE',entry[0]);
    if(detached.contentEditable!==entry[1]||detached.getAttribute('contenteditable')!==entry[0])
        throw 'raw enumeration or normalization';
}
var attr=detached.getAttributeNodeNS(null,'contenteditable');
for(var entry of [['TrUe','true'],['FALSE','false'],['PLAINtext-ONLY','plaintext-only']]){
    detached.contentEditable=entry[0];
    if(detached.contentEditable!==entry[1]||attr.value!==entry[1]||
       detached.getAttributeNode('contenteditable')!==attr)throw 'setter canonicalization or Attr identity';
}
detached.contentEditable='INHERIT';
if(detached.hasAttribute('contenteditable')||detached.contentEditable!=='inherit'||
   attr.ownerElement!==null||attr.value!=='plaintext-only')throw 'inherit removal or detached value';
attr.value='';child.setAttributeNode(attr);
if(attr.ownerElement!==child||child.contentEditable!=='true')throw 'reattached empty Attr';
child.setAttributeNS(null,'contenteditable',' invalid ');
outer.contentEditable='false';
if(child.contentEditable!=='inherit'||attr.value!==' invalid '||outer.contentEditable!=='false')
    throw 'invalid own state or parent change';
neighbor.contentEditable='true';button.contentEditable='false';
if(neighbor.contentEditable!=='true'||button.contentEditable!=='false'||neighbor.form!==document.getElementById('form'))
    throw 'native form element reflection';
"#);
}

#[test]
fn content_editable_namespace_lookalikes_and_shadowed_instance_leave_native_state_distinct() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var el=document.createElement('div'),P=HTMLElement.prototype;
var d=Object.getOwnPropertyDescriptor(P,'contentEditable'),get=d.get,set=d.set;
if(!d.enumerable||!d.configurable||'value' in d||'writable' in d||
   get.name!=='get contentEditable'||get.length!==0||set.name!=='set contentEditable'||set.length!==1||
   !('contentEditable' in el)||Object.hasOwn(el,'contentEditable')||Object.hasOwn(Element.prototype,'contentEditable'))
    throw 'declaring interface or descriptor';
el.setAttributeNS('urn:mode','meta:contenteditable','false');el.setAttributeNS(null,'CONTENTEDITABLE','true');
var ns=el.getAttributeNodeNS('urn:mode','contenteditable'),upper=el.getAttributeNodeNS(null,'CONTENTEDITABLE');
if(get.call(el)!=='inherit'||set.call(el,'TRUE')!==undefined||el.getAttributeNS(null,'contenteditable')!=='true')
    throw 'namespace or case selected as native slot';
Object.defineProperty(el,'contentEditable',{value:'shadow',writable:true,configurable:true});
el.contentEditable='author';set.call(el,'FALSE');
if(el.contentEditable!=='author'||get.call(el)!=='false'||el.getAttribute('contenteditable')!=='false')throw 'own shadow';
delete el.contentEditable;el.contentEditable='inherit';
if(el.contentEditable!=='inherit'||el.hasAttribute('contenteditable')||upper.value!=='true'||ns.value!=='false'||
   upper.ownerElement!==el||ns.ownerElement!==el)throw 'removal damaged unrelated attributes';
"#);
}

#[test]
fn content_editable_checks_receiver_and_converts_once_before_validating_the_value() {
    let (mut doc, mut forms, mut page) = fixture_with("<svg id=shape></svg>");
    run(&mut doc, &mut forms, &mut page, r#"
var el=document.createElement('div'),d=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'contentEditable');
var set=d.set,get=d.get,marker={},calls=0;
var value={toString:function(){calls++;return 'FALSE';},valueOf:function(){throw marker;}};
for(var bad of [null,{},Object.create(HTMLElement.prototype),document,document.createTextNode('x'),document.getElementById('shape')]){
    var count=0;
    try{set.call(bad,value);}catch(e){if(!(e instanceof TypeError))throw e;count++;}
    try{get.call(bad);}catch(e){if(!(e instanceof TypeError))throw e;count++;}
    if(count!==2||calls!==0)throw 'receiver before conversion';
}
set.call(el,value);
var saved=el.getAttributeNode('contenteditable');
if(calls!==1||get.call(el)!=='false'||saved.value!=='false')throw 'one conversion';
for(var bad of ['', ' true', 'false ', 'invalid', null, undefined]){
    var typed=false;
    try{el.contentEditable=bad;}catch(e){typed=e instanceof DOMException&&e.name==='SyntaxError'&&e.code===12;}
    if(!typed||saved.value!=='false'||saved!==el.getAttributeNode('contenteditable'))throw 'invalid setter or partial mutation';
}
var symbolError=false,same=false;
try{set.call(el,Symbol());}catch(e){symbolError=e instanceof TypeError;}
try{set.call(el,{toString:function(){throw marker;}});}catch(e){same=e===marker;}
if(!symbolError||!same||el.contentEditable!=='false'||saved.value!=='false')throw 'abrupt conversion mutated Attr';
set.call(el,{toString:function(){calls++;el.removeAttribute('contenteditable');return 'TRUE';}});
if(calls!==2||saved.ownerElement!==null||saved.value!=='false'||get.call(el)!=='true'||
   el.getAttributeNode('contenteditable')===saved)throw 'post-conversion state';
"#);
}

#[test]
fn content_editable_realm_owned_accessors_check_live_origin_and_error_identity() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var w=frame.contentWindow,child=w.document.body,root=document.createElement('span');
var parentDesc=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'contentEditable');
var childDesc=Object.getOwnPropertyDescriptor(w.HTMLElement.prototype,'contentEditable');
var cg=childDesc.get,cs=childDesc.set,ChildDOM=w.DOMException,conversions=0;
var arg={toString:function(){conversions++;return 'true';}};
if(cg===parentDesc.get||cs===parentDesc.set||Object.getPrototypeOf(cs)!==w.Function.prototype||
   child.contentEditable!=='inherit'||parentDesc.get.call(child)!=='inherit')throw 'child interface or real receiver';
cs.call(root,'plainTEXT-only');
var typed=false;try{cs.call(root,'invalid');}catch(e){typed=Object.getPrototypeOf(e)===ChildDOM.prototype&&e.code===12;}
if(!typed||cg.call(root)!=='plaintext-only'||root.contentEditable!=='plaintext-only')throw 'borrowed accessor or exception owner';
"#);
    let commits = page.state.dom_commits;
    page.set_location(&location("https://other-dom-string.test/changed"));
    for source in ["cg.call(root);", "cs.call(root,arg);"] {
        let result = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(result.errors, 1, "{source}\n{}", page.report());
        assert!(page.report().contains("same-origin"), "{}", page.report());
        assert_eq!(page.state.dom_commits, commits);
    }
    page.set_location(&location("https://dom-string.test/path/page"));
    run(&mut doc, &mut forms, &mut page, r#"
if(conversions!==0||cg.call(root)!=='plaintext-only')throw 'refused borrowed call produced effects';
cs.call(root,arg);if(conversions!==1||cg.call(root)!=='true')throw 'restored owner realm';
"#);
}
