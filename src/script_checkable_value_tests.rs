use super::*;

#[test]
fn checkable_value_shared_raw_reflection_preserves_dirty_checkedness_and_group() {
    let (mut doc, mut forms, mut page) = fixture("");
    eval(&mut doc, &mut forms, &mut page, r#"
        var fresh=document.createElement('input');fresh.type='radio';fresh.name='vote';
        fresh.id='created';slot.appendChild(fresh);fresh.checked=true;box.checked=false;
        function reflect(input) {
            if(input.value!=='on'||input.defaultValue!==''||input.hasAttribute('value'))
                throw 'absent default/on value';
            input.setAttribute('value','raw\r\nvalue');
            var attr=input.getAttributeNode('value');
            if(attr.value!=='raw\r\nvalue'||input.value!==attr.value||input.defaultValue!==attr.value)
                throw 'raw control value must preserve line breaks';
            input.defaultValue='';
            if(input.value!==''||attr.value!==''||input.getAttributeNode('value')!==attr)
                throw 'present empty value or identity';
            input.value='IDL value';
            if(attr.nodeValue!=='IDL value'||input.defaultValue!==attr.nodeValue)
                throw 'live setter must reflect raw content';
            input.value=null;
            if(attr.value!==''||input.value!=='')throw 'legacy null to empty string';
            input.defaultValue=null;
            if(attr.value!=='null'||input.value!=='null')throw 'defaultValue DOMString conversion';
            input.value='IDL value';
            input.setAttributeNS('urn:original:checkable','m:value','metadata');
            if(input.value!=='IDL value'||input.getAttributeNS('urn:original:checkable','value')!=='metadata')
                throw 'unrelated expanded name';
            var incoming=document.createAttribute('value');incoming.value='adopted';
            if(input.setAttributeNode(incoming)!==attr||attr.ownerElement!==null||incoming.ownerElement!==input)
                throw 'replacement Attr ownership';
            incoming.textContent='final';
            if(input.value!=='final'||input.getAttributeNodeNS(null,'value')!==incoming)
                throw 'attached Attr write';
            if(input.removeAttributeNode(incoming)!==incoming||incoming.ownerElement!==null||
               incoming.value!=='final'||input.value!=='on'||input.defaultValue!=='')
                throw 'detachment or absent value';
        }
        reflect(fresh);reflect(box);
        if(!fresh.checked||fresh.form!==formA||old.checked||peer.checked||!other.checked||box.checked)
            throw 'value writes disturbed the checked group';
    "#);
    for name in ["created", "box"] {
        let c = &forms.controls[&id(&doc, name)];
        assert!(c.dirty_checked);
        assert!(!c.dirty_value);
    }
    assert_eq!(query(&doc, &forms, "a"), "vote=on");
    neighbor_preserved(&doc, &forms);
}

#[test]
fn checkable_value_native_submission_and_reset_use_current_attribute() {
    let (mut doc, mut forms, mut page) = fixture("");
    eval(&mut doc, &mut forms, &mut page, r#"
        slot.appendChild(box);box.name='choice';old.setAttribute('value','radio payload');
        box.setAttribute('value','');var alias=old.getAttributeNode('value');
    "#);
    assert_eq!(query(&doc, &forms, "a"), "vote=radio+payload&choice=");
    eval(&mut doc, &mut forms, &mut page, r#"
        peer.checked=true;box.checked=false;alias.nodeValue='replacement';
        box.removeAttribute('value');
    "#);
    assert_eq!(query(&doc, &forms, "a"), "vote=peer");
    forms.reset_in_document(&doc, id(&doc, "a")).unwrap();
    assert_eq!(query(&doc, &forms, "a"), "vote=replacement&choice=on");
    eval(&mut doc, &mut forms, &mut page, r#"
        if(!old.checked||peer.checked||!box.checked||!other.checked||alias.ownerElement!==old||
           old.value!=='replacement'||box.value!=='on')throw 'post-reset native state';
    "#);
    neighbor_preserved(&doc, &forms);
}

#[test]
fn checkable_value_failed_growth_preserves_native_submission_and_attached_attribute() {
    let (mut doc, mut forms, mut page) = fixture("");
    eval(&mut doc, &mut forms, &mut page, r#"
        var retainedValue=old.getAttributeNode('value');
        var incoming=document.createAttribute('value');
        old.checked=true;
    "#);
    let oversized = "x".repeat(forms::MAX_VALUE_UNITS + 1);
    eval(&mut doc, &mut forms, &mut page, &format!("incoming.value='{oversized}';"));
    page.state.rebuild(&doc).unwrap();
    let before = snapshot(&doc, &forms, &page);
    page.state.retained_limit = before.retained;
    for script in [
        "old.value='a much longer reflected value';",
        "old.defaultValue='a much longer default';",
        "retainedValue.nodeValue='a much longer attached attribute';",
        "old.setAttributeNode(incoming);",
    ] {
        assert_eq!(page.console_eval(&mut doc, &mut forms, script).errors, 1, "{script}\n{}",page.report());
        assert_eq!(snapshot(&doc, &forms, &page), before, "{script}");
        assert_eq!(query(&doc, &forms, "a"), "vote=old", "{script}");
    }
    neighbor_preserved(&doc, &forms);
}
