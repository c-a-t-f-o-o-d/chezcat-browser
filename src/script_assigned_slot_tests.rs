//! Original light-DOM Slottable witness; an ordinary SLOT is not a shadow tree.
//! No slot assignment, ShadowRoot, custom element or resource shim is installed.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}
fn editor(forms: &Forms, node: usize) -> (String, usize, usize, usize, bool, Option<usize>) {
    let c = &forms.controls[&node];
    (
        c.value.text(),
        c.value.cursor,
        c.value.anchor,
        c.value.offset,
        c.dirty_value,
        c.owner,
    )
}
fn kept(
    doc: &Document,
    forms: &Forms,
    page: &mut Page,
    node: usize,
    before: &(String, usize, usize, usize, bool, Option<usize>),
) {
    assert_eq!(&editor(forms, node), before);
    assert_eq!(forms.text_default_value(node).unwrap(), "original");
    assert_eq!(doc.attr(node, "value"), Some("original"));
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (a, b, c) = page.take_image_changes();
    assert!(a.is_empty() && b.is_empty() && c.is_empty());
}

#[test]
fn slot_name_reflects_checked_content_attribute_on_genuine_elements() {
    let mut doc = crate::html::parse("<slot id=existing name=original></slot><div id=other></div>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0, "{}", page.report());
    let out = page.console_eval(&mut doc, &mut forms, r#"
var slot = document.createElement('slot'), existing = document.getElementById('existing');
slot.setAttribute('NAME', ' a ');
var descriptor = Object.getOwnPropertyDescriptor(HTMLSlotElement.prototype, 'name');
if (Object.getPrototypeOf(slot) !== HTMLSlotElement.prototype || !(existing instanceof HTMLSlotElement) ||
    Object.getPrototypeOf(HTMLSlotElement.prototype) !== HTMLElement.prototype ||
    !descriptor.enumerable || !descriptor.configurable || Object.hasOwn(slot, 'name') ||
    descriptor.get.call(slot) !== ' a ' || existing.name !== 'original') throw 'native slot reflection';
var original = slot.getAttributeNode('name'), other = document.getElementById('other');
var calls = 0, value = {toString:function(){ calls++; return ' Named '; }};
var rejected = false;
try { descriptor.set.call(other, value); } catch (error) { rejected = error instanceof TypeError; }
if (!rejected || calls !== 0 || other.hasAttribute('name')) throw 'receiver guard before coercion';
var observer = new MutationObserver(function(){});
observer.observe(slot, {attributes:true, attributeFilter:['name'], attributeOldValue:true});
descriptor.set.call(slot, value);
var changed = observer.takeRecords();
if (calls !== 1 || slot.name !== ' Named ' || slot.getAttribute('name') !== ' Named ' ||
    slot.getAttributeNode('name') !== original || original.value !== ' Named ') throw 'raw value';
if (changed.length !== 1 || changed[0].target !== slot || changed[0].oldValue !== ' a ')
    throw 'mutation record';
slot.removeAttribute('name');
if (slot.name !== '' || slot.hasAttribute('name') || original.ownerElement !== null)
    throw 'missing name default';
slot.name = null;
if (slot.name !== 'null' || slot.assignedSlot !== null || other.assignedSlot !== null)
    throw 'string conversion or ordinary tree assignment';
"#);
    assert_eq!(out.errors, 0, "{}", page.report());
    assert!(!page.has_host_tasks());
}

#[test]
fn assigned_slot_original_real_element_text_light_slot_detached_and_readonly() {
    let mut doc=crate::html::parse(concat!(
        "<!doctype html><title>Original Slottable</title><body><div id=first>",
        "<span id=element slot=chosen>Original text</span><slot id=ordinary name=chosen><span id=fallback>Fallback</span></slot></div>",
        "<div id=second></div><form id=form><input id=neighbor name=keep value=original></form>"
    ));
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let source = crate::navigation::address(
        "https://assigned-slot.test/original",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&source);
    page.set_resource_base(&source);
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );
    let neighbor = id(&doc, "neighbor");
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    {
        let e = &mut forms.controls.get_mut(&neighbor).unwrap().value;
        e.cursor = 5;
        e.anchor = 2;
        e.offset = 1;
    }
    let before = editor(&forms, neighbor);
    assert_eq!(before.5, Some(id(&doc, "form")));
    let prefix=page.console_eval(&mut doc,&mut forms,r#"
var element=document.getElementById('element'),text=element.firstChild,ordinary=document.getElementById('ordinary'),fallback=document.getElementById('fallback'),first=document.getElementById('first'),second=document.getElementById('second'),neighbor=document.getElementById('neighbor');
if(element.nodeType!==1||element.nodeName!=='SPAN'||text.nodeType!==3||text.data!=='Original text'||text.parentNode!==element||element.parentNode!==first)throw 'genuine attached Element/Text prefix';
if(element.ownerDocument!==document||text.ownerDocument!==document||ordinary.nodeName!=='SLOT'||ordinary.getAttribute('name')!=='chosen'||element.getAttribute('slot')!=='chosen'||fallback.parentNode!==ordinary||ordinary.parentNode!==first)throw 'real light-DOM slot/raw attribute prefix';
if(neighbor.value!=='edited neighbor'||neighbor.defaultValue!=='original'||neighbor.getAttribute('value')!=='original'||neighbor.form!==document.getElementById('form'))throw 'actual edited/default/native owner prefix';
document.title='Assigned slot real DOM prefix passed';
"#);
    assert_eq!(prefix.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "Assigned slot real DOM prefix passed");
    kept(&doc, &forms, &mut page, neighbor, &before);

    // Matching light-DOM names alone cannot produce an assigned slot.
    let out=page.console_eval(&mut doc,&mut forms,r#"
var assigned=element.assignedSlot;
if(assigned!==null)throw 'ordinary light-DOM Element has no assigned slot';
if(text.assignedSlot!==null||ordinary.assignedSlot!==null||fallback.assignedSlot!==null)throw 'real Text/slot/fallback are unassigned';
if(!('assignedSlot' in element)||!('assignedSlot' in text)||Object.hasOwn(element,'assignedSlot')||Object.hasOwn(text,'assignedSlot'))throw 'Slottable attribute must be inherited';
var d=Object.getOwnPropertyDescriptor(Element.prototype,'assignedSlot');
if(!d||typeof d.get!=='function'||d.get.length!==0||d.get.name!=='get assignedSlot'||d.set!==undefined||!d.enumerable||!d.configurable||d.value!==undefined)throw 'readonly Element getter descriptor';
if(d.get.call(element)!==null||d.get.call(ordinary)!==null||Object.getOwnPropertyDescriptor(Node.prototype,'assignedSlot')!==undefined||Object.getOwnPropertyDescriptor(CharacterData.prototype,'assignedSlot')!==undefined)throw 'actual receiver and finite mixin placement';
var effects=0,bait={toString:function(){effects++;throw 'readonly conversion';},valueOf:function(){effects++;throw 'readonly conversion';}};
element.assignedSlot=bait;text.assignedSlot=bait;
if(element.assignedSlot!==null||text.assignedSlot!==null||effects!==0||Object.hasOwn(element,'assignedSlot')||Object.hasOwn(text,'assignedSlot'))throw 'sloppy readonly assignment changed native or created own property';
var caught=0;try{(function(){'use strict';element.assignedSlot=bait;})();}catch(e){if(e instanceof TypeError)caught++;else throw e;}
try{(function(){'use strict';text.assignedSlot=bait;})();}catch(e){if(e instanceof TypeError)caught++;else throw e;}if(caught!==2||effects!==0)throw 'strict readonly refusal';
var comment=document.createComment('kept'),fragment=document.createDocumentFragment();
for(var other of [comment,fragment,document]){if('assignedSlot' in other||other.assignedSlot!==undefined)throw 'non-Slottable acquired assignedSlot';}
var raw=element.getAttributeNode('slot');first.removeChild(element);
if(element.assignedSlot!==null||text.assignedSlot!==null||element.parentNode!==null||element.ownerDocument!==document||element.getAttributeNode('slot')!==raw||raw.value!=='chosen')throw 'detached real owner/raw attribute';
second.appendChild(element);if(element.assignedSlot!==null||text.assignedSlot!==null||element.parentNode!==second||text.parentNode!==element||raw.value!=='chosen'||fallback.parentNode!==ordinary)throw 'reparent fabricated slot assignment or changed fallback';
var fresh=document.createElement('span'),freshText=document.createTextNode('Fresh');fresh.appendChild(freshText);
if(fresh.assignedSlot!==null||freshText.assignedSlot!==null||fresh.ownerDocument!==document)throw 'fresh detached Slottables';
fragment.appendChild(fresh);if(fresh.assignedSlot!==null||freshText.assignedSlot!==null||fresh.getRootNode()!==fragment)throw 'ordinary fragment is not a shadow root';
second.appendChild(fragment);if(fresh.assignedSlot!==null||freshText.assignedSlot!==null||fresh.parentNode!==second||fragment.childNodes.length!==0)throw 'real fragment splice fabricated assignment';
if(element.getAttribute('slot')!=='chosen'||ordinary.getAttribute('name')!=='chosen'||neighbor.value!=='edited neighbor'||neighbor.defaultValue!=='original')throw 'readback altered raw/native neighbors';
"#);
    kept(&doc, &forms, &mut page, neighbor, &before);
    assert_eq!(out.errors, 0, "{}", page.report());
}
