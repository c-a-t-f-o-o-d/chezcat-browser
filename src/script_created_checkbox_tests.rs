use super::*;

#[test]
fn created_checkbox_reflects_type_and_preserves_native_state_on_insert_move_and_reset() {
    let (mut doc, mut forms, mut page) = fixture("<input id=parsed type=checkbox>", r#"
        var neighbor=document.getElementById('neighbor');neighbor.value='neighbor edited';
        var input=document.createElement('input'),parsed=document.getElementById('parsed');
        var shell=document.createElement('div');shell.innerHTML='<input type=checkbox>';
        var fragmentCheck=shell.firstChild,calls=0;
        input.type={toString:function(){calls++;return 'ChEcKbOx';}};
        if(calls!==1||input.type!=='checkbox'||input.outerHTML!=='<input type="ChEcKbOx">'||
            input.value!=='on'||input.value!==parsed.value||input.value!==fragmentCheck.value||
            input.getAttribute('value')!==null||input.checked!==false||input.checked!==fragmentCheck.checked||
            input.ownerDocument!==document||input.parentNode!==null||input.form!==null)throw 'detached checkbox';
        input.id='created';input.name='choice';input.checked=true;
        var typeAttr=input.getAttributeNode('type');input.setAttribute('TYPE','CHECKBOX');
        if(input.getAttributeNode('type')!==typeAttr||typeAttr.value!=='CHECKBOX'||!input.checked)throw 'same type';
        typeAttr.value='cHeCkBoX';
        var fragment=document.createDocumentFragment();fragment.appendChild(input);
        document.getElementById('shell').appendChild(fragment);
        if(fragment.firstChild!==null||input.form!==document.getElementById('a')||!input.isConnected||
            input.name!=='choice'||input.getAttribute('checked')!==null||!input.checked)throw 'connected state';
    "#);
    let node = id(&doc, "created");
    let neighbor = id(&doc, "neighbor");
    let edit = &mut forms.controls.get_mut(&neighbor).unwrap().value;
    edit.cursor = 2; edit.anchor = 5; edit.offset = 1;
    let preserved = editor(&forms, neighbor);
    assert_eq!(forms.controls[&node].kind, Kind::Checkbox);
    assert_eq!(forms.controls[&node].caption, forms.controls[&id(&doc,"parsed")].caption);
    assert_eq!(query(&doc, &forms, "a"), "keep=neighbor+edited&choice=on&tail=after");
    let count = forms.controls.len();
    eval(&mut doc, &mut forms, &mut page, r#"
        document.getElementById('dest').appendChild(input);
        input.value='yes';input.setAttribute('name','vote');input.type='checkbox';
        if(input.form!==document.getElementById('b')||input.getAttributeNode('type')!==typeAttr||
            input.name!=='vote'||input.getAttribute('value')!=='yes'||!input.checked)throw 'moved state';
    "#);
    assert_eq!(query(&doc, &forms, "a"), "keep=neighbor+edited&tail=after");
    assert_eq!(query(&doc, &forms, "b"), "vote=yes");
    forms.reset(id(&doc, "b"));
    eval(&mut doc, &mut forms, &mut page, "if(input.checked||input.value!=='yes'||input.getAttribute('checked')!==null||typeAttr.ownerElement!==input)throw 'reset state';");
    assert_eq!(query(&doc, &forms, "b"), "");
    assert_eq!(forms.controls.len(), count);
    assert_eq!(editor(&forms, neighbor), preserved);
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "seed");
    no_resources(&mut page);
}

#[test]
fn created_checkbox_attribute_routes_require_real_lowercase_null_namespace_type() {
    let (doc, forms, mut page) = fixture("", r#"
        var raw=document.createElement('input');raw.id='raw';raw.setAttribute('TYPE','CheckBox');
        var ns=document.createElementNS('http://www.w3.org/1999/xhtml','input');ns.id='ns';
        ns.setAttributeNS('urn:local','m:type','checkbox');ns.setAttributeNS(null,'TYPE','checkbox');
        if(ns.type!=='text'||ns.getAttributeNS(null,'type')!==null)throw 'metadata selected type';
        ns.setAttributeNS(null,'type','CHECKBOX');
        var byNode=document.createElement('input');byNode.id='by-node';
        var first=document.createAttribute('type');first.value='cHecKBox';
        if(byNode.setAttributeNode(first)!==null||byNode.getAttributeNode('type')!==first)throw 'admission Attr';
        byNode.checked=true;
        var second=document.createAttributeNS(null,'type');second.value='checkbox';
        if(byNode.setAttributeNodeNS(second)!==first||first.ownerElement!==null||first.value!=='cHecKBox'||
            second.ownerElement!==byNode||second!==byNode.getAttributeNodeNS(null,'type')||!byNode.checked||
            ns.getAttributeNS('urn:local','type')!=='checkbox'||ns.getAttributeNS(null,'TYPE')!=='checkbox')throw 'Attr identity';
    "#);
    for name in ["raw","ns","by-node"] {
        let control = &forms.controls[&id(&doc, name)];
        assert_eq!(control.kind, Kind::Checkbox);
        assert_eq!(control.owner, None);
        assert_eq!(control.value.text(), "on");
    }
    no_resources(&mut page);
}

#[test]
fn created_checkbox_admission_charges_native_and_attribute_bytes_together() {
    for mode in ["retained", "copy", "work", "capacity"] {
        // The fixture already has two native controls besides this markup.
        let others = if mode == "capacity" { "<input>".repeat(forms::MAX_CONTROLS - 2) } else { String::new() };
        let (mut doc, mut forms, mut page) = fixture(&others,
            "var input=document.createElement('input');input.id='created';var incoming=document.createAttribute('type');incoming.value='CheckBox';document.getElementById('neighbor').value='neighbor edited';");
        let node = id(&doc,"created");
        let handle = script_attr::ATTR_BASE;
        assert_eq!(page.state.attributes.len(), 1);
        page.state.rebuild(&doc).unwrap();
        let before = Snapshot::capture(&doc, &forms, &page);
        // Incoming Attr already owns its value. Adoption retains the element
        // name, owner-index slot and the checkbox's default plus two editor
        // characters.
        let growth = "type".len() + std::mem::size_of::<usize>() + "on".len() + 2 * std::mem::size_of::<char>();
        match mode {
            "retained" => page.state.retained_limit = before.retained + growth - 1,
            "copy" => page.state.copy_work = 0,
            "work" => page.state.host_work = 0,
            _ => assert_eq!(forms.controls.len(), forms::MAX_CONTROLS),
        }
        let adopt = |doc: &mut Document, forms: &mut Forms, page: &mut Page| {
            Bridge { doc, forms, state:&mut page.state }.set_attribute_node(node_handle(node), &[Value::Host(handle)])
        };
        assert!(adopt(&mut doc, &mut forms, &mut page).is_err(), "{mode}");
        assert_eq!(Snapshot::capture(&doc, &forms, &page), before, "{mode}");
        if mode == "retained" {
            page.state.retained_limit += 1;
            assert!(matches!(adopt(&mut doc, &mut forms, &mut page), Ok(Value::Null)));
            assert_eq!(page.state.retained, before.retained + growth);
            assert_eq!(forms.controls[&node].kind, Kind::Checkbox);
            assert_eq!(page.state.attributes[0].owner, Some(node));
        }
        no_resources(&mut page);
    }
}

#[test]
fn created_checkbox_attr_identity_refusal_and_invalid_type_changes_preserve_checkedness() {
    let (mut doc, mut forms, mut page) = fixture("", r#"
        var input=document.createElement('input');input.id='created';input.type='checkbox';input.checked=true;
        var incoming=document.createAttribute('type');incoming.value='CHECKBOX';
        document.getElementById('neighbor').value='neighbor edited';
    "#);
    page.state.rebuild(&doc).unwrap();
    page.state.limits.host.attribute_identities = page.state.attributes.len();
    let before = Snapshot::capture(&doc, &forms, &page);
    for source in ["input.setAttributeNode(incoming);", "input.removeAttribute('type');",
        "input.type='radio';","input.setAttribute('type',' checkbox ');","input.setAttributeNS(null,'type','text');"] {
        let result = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(result.errors, 1, "{source}\n{}", page.report());
        assert_eq!(Snapshot::capture(&doc, &forms, &page), before, "{source}");
    }
    no_resources(&mut page);
}
