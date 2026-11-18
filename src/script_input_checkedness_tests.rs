//! INPUT checkedness is independent of whether its type renders a check mark.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}
fn source() -> crate::navigation::Location {
    crate::navigation::address(
        "https://checkedness.test/source",
        &crate::navigation::Location::home(),
    )
    .unwrap()
}
fn eval(doc: &mut Document, forms: &mut Forms, page: &mut Page, js: &str) {
    assert_eq!(
        page.console_eval(doc, forms, js).errors,
        0,
        "{js}\n{}",
        page.report()
    );
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    assert!(page.take_image_changes().2.is_empty());
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
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<html><body><form id=f action='/result'><input id=neighbor name=keep value=seed><input id=text name=q value=default checked><input id=hidden type=hidden name=h value=hidden><input id=submit type=submit value=Send><input id=reset type=reset value=Reset><input id=button type=button value=Button><input id=image type=image value=Image><input id=box type=checkbox name=c value=yes checked><input id=r1 type=radio name=r value=one checked><input id=r2 type=radio name=r value=two><textarea id=area>area</textarea><button id=actual type=button>Actual</button><div id=host></div></form><form id=g><div id=dest></div></form></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    page.set_location(&source());
    page.set_resource_base(&source());
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        var text=document.getElementById('text'),neighbor=document.getElementById('neighbor');
        var box=document.getElementById('box'),r1=document.getElementById('r1'),r2=document.getElementById('r2');
        var d=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'checked'),get=d.get,set=d.set;
        neighbor.value='edited neighbor';text.value='edited text';
        if(!d.enumerable||!d.configurable||get.call(text)!==true||box.checked!==true||
           Object.getOwnPropertyDescriptor(text,'checked')!==undefined)throw 'canonical native prefix';
    "#,
    );
    let value = &mut forms.controls.get_mut(&id(&doc, "neighbor")).unwrap().value;
    value.cursor = 2;
    value.anchor = 5;
    value.offset = 1;
    (doc, forms, page)
}
fn query(doc: &Document, forms: &Forms) -> String {
    forms
        .submit_request(doc, id(doc, "f"), None, &source(), &source())
        .unwrap()
        .location
        .url
        .unwrap()
        .query
        .unwrap_or_default()
}

#[test]
fn input_checkedness_noncheckable_state_attributes_and_reset_preserve_editor_and_submission() {
    let (mut doc, mut forms, mut page) = fixture();
    let kept = editor(&forms, id(&doc, "neighbor"));
    let text_editor = editor(&forms, id(&doc, "text"));
    let before_query = query(&doc, &forms);
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        text.removeAttribute('checked');if(text.checked)throw 'pristine removal';
        text.setAttribute('checked','default');if(!text.checked)throw 'pristine addition';
        set.call(text,true); // Equal assignment still sets dirty checkedness.
        var attr=text.getAttributeNode('checked');
        text.removeAttribute('checked');if(!text.checked||attr.ownerElement!==null)throw 'dirty removal';
        set.call(text,false);text.setAttribute('checked','next default');
        if(text.checked||text.getAttribute('checked')!=='next default'||text.value!=='edited text')
            throw 'dirty addition changed current or editor value';
        for(var name of ['hidden','submit','reset','button','image']){
            var input=document.getElementById(name),value=input.value;
            if(!('checked' in input)||get.call(input)!==false)throw 'noncheckable initial checkedness';
            set.call(input,Symbol('truth'));
            input.setAttribute('checked','');input.removeAttribute('checked');
            if(!input.checked||input.value!==value||Object.getOwnPropertyDescriptor(input,'checked')!==undefined)
                throw 'independent inherited boolean state';
        }
        if(document.querySelector('#text:checked')!==null||document.querySelector('#hidden:checked')!==null||
           document.querySelector('#box:checked')!==box)throw 'checked selector admitted wrong type';
        var bad=0;
        for(var other of [document.getElementById('area'),document.getElementById('actual'),{},HTMLInputElement.prototype]){
            try{set.call(other,true);}catch(e){if(e instanceof TypeError)bad++;}
        }
        if(bad!==4)throw 'wrong element acquired native checkedness';
    "#,
    );
    assert!(forms.controls[&id(&doc, "text")].dirty_checked);
    assert_eq!(editor(&forms, id(&doc, "text")), text_editor);
    assert_eq!(editor(&forms, id(&doc, "neighbor")), kept);
    assert_eq!(query(&doc, &forms), before_query);
    forms.reset(id(&doc, "f"));
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        if(!text.checked||text.getAttribute('checked')!=='next default'||text.value!=='default')throw 'reset current/default state';
        text.removeAttribute('checked');if(text.checked)throw 'reset did not clear dirty checkedness';
        for(var name of ['hidden','submit','reset','button','image']){
            if(document.getElementById(name).checked)throw 'reset noncheckable checkedness';
        }
    "#,
    );
    assert!(!forms.controls[&id(&doc, "text")].dirty_checked);
}

#[test]
fn input_checkedness_checkbox_radio_defaults_and_native_activation_keep_special_behavior() {
    let (mut doc, mut forms, mut page) = fixture();
    let kept = editor(&forms, id(&doc, "neighbor"));
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        box.removeAttribute('checked');if(box.checked)throw 'checkbox pristine removal';
        box.setAttribute('checked','');if(!box.checked)throw 'checkbox pristine addition';
        set.call(box,false);box.removeAttribute('checked');box.setAttribute('checked','');
        if(box.checked)throw 'checkbox dirty attribute sync';
        set.call(r2,true);
        if(r1.checked||!r2.checked)throw 'radio exclusivity';
        r1.setAttribute('checked','same presence');
        if(r1.checked||!r2.checked)throw 'existing attribute value reselected radio';
        r1.removeAttribute('checked');r1.setAttribute('checked','');
        if(!r1.checked||r2.checked)throw 'pristine radio default transition';
        r2.setAttribute('checked','');if(r2.checked)throw 'dirty radio default transition';
    "#,
    );
    assert!(!forms.controls[&id(&doc, "r1")].dirty_checked);
    assert!(forms.controls[&id(&doc, "r2")].dirty_checked);
    assert_eq!(editor(&forms, id(&doc, "neighbor")), kept);
    forms.reset(id(&doc, "f"));
    assert!(forms.controls[&id(&doc, "box")].checked);
    assert!(!forms.controls[&id(&doc, "box")].dirty_checked);
    assert!(!forms.controls[&id(&doc, "r1")].checked);
    assert!(forms.controls[&id(&doc, "r2")].checked);
    let box_node = id(&doc, "box");
    forms.toggle(box_node);
    assert!(forms.controls[&box_node].dirty_checked);
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        box.removeAttribute('checked');box.setAttribute('checked','');
        if(box.checked)throw 'native activation lost dirty checkedness';
    "#,
    );
}

#[test]
fn input_checkedness_detached_fragment_hidden_metadata_and_failed_admission_keep_real_state() {
    let (mut doc, mut forms, mut page) = fixture();
    let kept = editor(&forms, id(&doc, "neighbor"));
    let original_count = forms.controls.len();
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        var readFirst=document.createElement('input');readFirst.id='read-first';
        if(readFirst.checked!==false||get.call(readFirst)!==false)throw 'fresh checked read';
    "#,
    );
    assert_eq!(
        forms.controls.len(),
        original_count,
        "a pristine boolean read admitted a native editor"
    );
    assert!(!forms.controls.contains_key(&id(&doc, "read-first")));
    let retained_before_form = page.state.retained;
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        if(readFirst.form!==null||!('form' in readFirst)||readFirst.parentNode!==null||
           readFirst.isConnected||readFirst.ownerDocument!==document)throw 'fresh allocation-free form owner';
    "#,
    );
    assert_eq!(
        forms.controls.len(),
        original_count,
        "a form-owner read admitted a native editor"
    );
    assert!(!forms.controls.contains_key(&id(&doc, "read-first")));
    assert_eq!(page.state.retained, retained_before_form);
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        readFirst.type='checkbox';
        if(readFirst.type!=='checkbox'||readFirst.checked!==false||readFirst.value!=='on')
            throw 'boolean observation blocked initial native type choice';
    "#,
    );
    assert_eq!(forms.controls.len(), original_count + 1);
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        var fresh=document.createElement('input');fresh.id='fresh';fresh.setAttribute('checked','');
        if(get.call(fresh)!==true||fresh.form!==null||fresh.isConnected)throw 'fresh checked default';
        var hooks=0;set.call(fresh,{valueOf:function(){hooks++;throw 'coercion';}});
        fresh.removeAttribute('checked');
        if(!fresh.checked||hooks||fresh.value!=='')throw 'fresh dirty boolean without coercion';
        document.getElementById('dest').appendChild(fresh);
        if(fresh.form!==document.getElementById('g')||!fresh.checked)throw 'moved fresh state';
        var hidden=document.getElementById('hidden');set.call(hidden,true);hidden.value='replacement';
        hidden.setAttribute('checked','');hidden.removeAttribute('checked');
        if(!hidden.checked||hidden.value!=='replacement')throw 'hidden metadata replacement lost flag';
        var holder=document.createElement('div');holder.innerHTML='<input id=fragment checked value=default>';
        var fragmentInput=holder.firstChild;
        if(!fragmentInput.checked||fragmentInput.form!==null)throw 'fragment initial state';
        set.call(fragmentInput,false);fragmentInput.removeAttribute('checked');fragmentInput.setAttribute('checked','');
        if(fragmentInput.checked||fragmentInput.defaultValue!=='default')throw 'fragment dirty checkedness';
        var pending=document.createElement('input');pending.id='pending';
    "#,
    );
    assert!(forms.controls[&id(&doc, "hidden")].dirty_checked);
    assert!(forms.controls[&id(&doc, "fresh")].dirty_checked);
    assert!(forms.controls[&id(&doc, "fragment")].dirty_checked);
    assert_eq!(editor(&forms, id(&doc, "neighbor")), kept);
    let pending = id(&doc, "pending");
    let count = forms.controls.len();
    let nodes = format!("{:?}", doc.nodes);
    let retained = page.state.retained;
    page.state.host_work = 0;
    assert!(Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state
    }
    .set(node_handle(pending), "checked", Value::Bool(true))
    .is_err());
    assert_eq!(forms.controls.len(), count);
    assert!(!forms.controls.contains_key(&pending));
    assert_eq!(format!("{:?}", doc.nodes), nodes);
    assert_eq!(page.state.retained, retained);
    assert_eq!(editor(&forms, id(&doc, "neighbor")), kept);
    forms.reset(id(&doc, "g"));
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        "if(fresh.checked||fresh.getAttribute('checked')!==null)throw 'moved default/reset';",
    );
}
