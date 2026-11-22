use super::*;

#[path = "script_checkable_value_tests.rs"]
mod checkable_value_tests;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).expect(name)
}

fn fixture(extra: &str) -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(&format!("<html><body>
        <form id=a action='/a'><input id=original name=vote type=radio value=old checked>
        <input id=peer name=vote type=radio value=peer><div id=slot></div></form>
        <form id=b action='/b'><input id=other name=vote type=radio value=other checked>
        <input id=neighbor name=keep value=seed><div id=destination></div></form>
        <input id=free type=radio name=vote checked><input id=box type=checkbox checked>
        <template id=inert></template><svg id=foreign></svg>{extra}</body></html>"));
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let loc = location();
    page.set_location(&loc);
    page.set_resource_base(&loc);
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    eval(&mut doc, &mut forms, &mut page, r#"
        function byId(id){return document.getElementById(id);}
        var formA=byId('a'),formB=byId('b'),old=byId('original'),peer=byId('peer'),other=byId('other'),free=byId('free');
        var neighbor=byId('neighbor'),slot=byId('slot'),dest=byId('destination'),box=byId('box');
        neighbor.value='edited';
    "#);
    let editor = &mut forms.controls.get_mut(&id(&doc,"neighbor")).unwrap().value;
    editor.cursor = 2; editor.anchor = 4; editor.offset = 1;
    (doc, forms, page)
}

fn location() -> crate::navigation::Location {
    crate::navigation::address("https://radio.test/start", &crate::navigation::Location::home()).unwrap()
}

fn eval(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    assert_eq!(page.console_eval(doc, forms, source).errors, 0, "{source}\n{}", page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    assert!(page.take_image_changes().2.is_empty());
}

fn query(doc: &Document, forms: &Forms, form: &str) -> String {
    forms.submit_request(doc, id(doc, form), None, &location(), &location()).unwrap()
        .location.url.unwrap().query.unwrap_or_default()
}

fn neighbor_preserved(doc: &Document, forms: &Forms) {
    let node = id(doc, "neighbor");
    let c = &forms.controls[&node];
    assert_eq!(c.value.text(), "edited");
    assert_eq!((c.value.cursor, c.value.anchor, c.value.offset), (2, 4, 1));
    assert_eq!(forms.text_default_value(node).unwrap(), "seed");
    assert_eq!(c.owner, Some(id(doc, "b")));
    assert!(c.dirty_value);
}

#[test]
fn created_radio_content_attribute_order_fragment_connection_native_selection_and_reset() {
    let (mut doc, mut forms, mut page) = fixture("");
    eval(&mut doc, &mut forms, &mut page, r#"
        var fresh=document.createElement('input');fresh.id='fresh';
        fresh.setAttribute('type','RaDiO');fresh.setAttribute('checked','checked');fresh.setAttribute('name','vote');
        var alias=fresh.getAttributeNode('type'),checkedAttr=fresh.getAttributeNode('checked');
        if(fresh.type!=='radio'||fresh.name!=='vote'||fresh.value!=='on'||!fresh.checked||fresh.form!==null||
           fresh.getAttribute('value')!==null||fresh.getAttribute('type')!=='RaDiO'||!old.checked)throw 'fresh radio defaults';
        var region=document.createElement('div'),fragment=document.createDocumentFragment();
        region.appendChild(fresh);fragment.appendChild(region);
        if(fresh.getRootNode()!==fragment||fresh.isConnected||fresh.form!==null||!old.checked||!fresh.checked)throw 'detached root';
        slot.appendChild(fragment);alias.value='RADIO';
        if(fresh.form!==formA||!fresh.isConnected||fresh.getAttributeNode('type')!==alias||fresh.getAttributeNode('checked')!==checkedAttr||
           !fresh.checked||old.checked||peer.checked||!other.checked||!free.checked||!box.checked)throw 'connected group';
    "#);
    let fresh = id(&doc,"fresh");
    assert_eq!(forms.controls[&fresh].kind, Kind::Radio);
    assert_eq!(query(&doc,&forms,"a"), "vote=on");
    assert!(!forms.controls[&fresh].dirty_checked);
    forms.toggle(id(&doc,"peer"));
    assert!(!forms.controls[&fresh].checked);
    assert!(!forms.controls[&fresh].dirty_checked);
    assert_eq!(query(&doc,&forms,"a"), "vote=peer");
    forms.toggle(fresh);
    assert!(forms.controls[&fresh].dirty_checked);
    eval(&mut doc, &mut forms, &mut page, r#"
        fresh.checked=false;fresh.removeAttribute('checked');fresh.setAttribute('checked','');
        if(fresh.checked||checkedAttr.ownerElement!==null||checkedAttr.value!=='checked')throw 'dirty checkedness or removed Attr';
    "#);
    forms.reset_in_document(&doc, id(&doc,"a")).unwrap();
    assert_eq!(query(&doc,&forms,"a"), "vote=on");
    assert!(!forms.controls[&fresh].dirty_checked);
    assert_eq!(query(&doc,&forms,"b"), "vote=other&keep=edited");
    neighbor_preserved(&doc,&forms);
}

#[test]
fn created_radio_group_membership_tracks_root_owner_nonempty_name_and_dirty_state() {
    let (mut doc, mut forms, mut page) = fixture("");
    eval(&mut doc, &mut forms, &mut page, r#"
        var left=document.createDocumentFragment(),right=document.createDocumentFragment(),calls=0;
        var first=document.createElement('input'),second=document.createElement('input');
        first.setAttributeNS('urn:meta','m:type','radio');first.setAttributeNS(null,'TYPE','radio');
        if(first.type!=='text')throw 'namespaced or differently cased type acquired native state';
        first.type={toString:function(){calls++;return 'RaDiO';}};second.setAttributeNS(null,'type','radio');
        first.name='vote';second.setAttribute('name','vote');first.checked=true;second.checked=true;
        left.appendChild(first);right.appendChild(second);
        if(calls!==1||!first.checked||!second.checked||!free.checked||first.form!==null||second.form!==null)throw 'equal names in distinct roots';
        left.appendChild(second);
        if(first.checked||!second.checked||!free.checked||second.getRootNode()!==left)throw 'checked radio joining shared detached root';
        first.name='VOTE';first.checked=true;
        if(!first.checked||!second.checked)throw 'case sensitive group names';
        var nameAttr=first.getAttributeNode('name');nameAttr.value='vote';
        if(!first.checked||second.checked||first.getAttributeNode('name')!==nameAttr)throw 'checked rename must join and win';
        first.name='';second.removeAttribute('name');second.checked=true;
        if(!first.checked||!second.checked)throw 'empty names do not group';
        first.name='vote';second.name='vote';if(first.checked||!second.checked)throw 'checked name insertion';
        slot.appendChild(left);
        if(second.form!==formA||first.form!==formA||!second.checked||old.checked||!other.checked||!free.checked)throw 'form connection';
        dest.appendChild(second);
        if(second.form!==formB||!second.checked||other.checked||!free.checked||old.checked)throw 'owner change';
        right.appendChild(second);
        if(second.form!==null||!second.checked||second.getRootNode()!==right||!free.checked)throw 'detached after connection';
        document.body.appendChild(second);
        if(second.form!==null||!second.checked||free.checked||second.getRootNode()!==document)throw 'ownerless document group';
        if(!box.checked)throw 'checkbox was grouped';
    "#);
    neighbor_preserved(&doc,&forms);
}

#[test]
fn created_radio_form_reset_uses_current_document_order_after_move() {
    let (mut doc, mut forms, mut page) = fixture("");
    eval(&mut doc, &mut forms, &mut page, r#"
        var fresh=document.createElement('input');fresh.type='radio';fresh.setAttribute('checked','');fresh.name='vote';fresh.id='fresh';
        slot.appendChild(fresh);formA.insertBefore(fresh,old);
        peer.checked=true;
        if(!peer.checked||fresh.checked||old.checked||old.getAttribute('checked')===null||fresh.getAttribute('checked')===null)throw 'reset setup';
    "#);
    forms.reset_in_document(&doc, id(&doc,"a")).unwrap();
    assert!(forms.controls[&id(&doc,"original")].checked, "Last checked default in tree order wins");
    for name in ["fresh","peer"] { assert!(!forms.controls[&id(&doc,name)].checked); }
    assert_eq!(query(&doc,&forms,"a"), "vote=old");
    assert!(forms.controls[&id(&doc,"other")].checked);
    neighbor_preserved(&doc,&forms);
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    nodes: String,
    parents: Vec<Option<usize>>,
    connected: Vec<bool>,
    controls: Vec<String>,
    attributes: Vec<(Option<usize>, Vec<u16>, String)>,
    retained: usize,
    commits: Option<u64>,
}

fn snapshot(doc: &Document, forms: &Forms, page: &Page) -> Snapshot {
    Snapshot {
        nodes: format!("{:?}", doc.nodes),
        parents: page.state.tree.parents.clone(),
        connected: page.state.dom.connected.clone(),
        controls: forms.controls.iter().map(|(node, c)| format!("{:?}", (
            node, c.kind, c.owner, c.group_root, &c.name, c.checked, c.dirty_checked,
            c.value.text(), c.dirty_value, (c.value.cursor, c.value.anchor, c.value.offset),
        ))).collect(),
        attributes: page.state.attributes.iter()
            .map(|a| (a.owner.and_then(node_index), a.name.qualified.code_units().collect::<Vec<u16>>(), a.value.scalar().unwrap().to_owned())).collect(),
        retained: page.state.retained,
        commits: page.state.dom_commits,
    }
}

#[test]
fn created_radio_active_type_refusals_preserve_control_group_and_attr_handles() {
    let (mut doc, mut forms, mut page) = fixture("");
    eval(&mut doc, &mut forms, &mut page, r#"
        var r=document.createElement('input');r.id='created';r.type='radio';r.name='vote';
        slot.appendChild(r);r.checked=true;
        var typeAttr=r.getAttributeNode('type'),unattached=document.createAttribute('type');
        unattached.value='range';
    "#);
    page.state.rebuild(&doc).unwrap();
    let before = snapshot(&doc,&forms,&page);
    for script in [
        "neighbor.type='radio';", "box.type='radio';", "r.removeAttribute('type');",
        "r.type='text';", "r.setAttributeNS(null,'type','checkbox');", "typeAttr.value=' radio ';",
        "r.setAttributeNode(unattached);",
        "slot.innerHTML='<input type=text><input type=radio>';",
    ] {
        assert_eq!(page.console_eval(&mut doc, &mut forms, script).errors, 1, "{script}\n{}", page.report());
        assert_eq!(snapshot(&doc,&forms,&page), before, "{script}");
    }
    eval(&mut doc, &mut forms, &mut page, r#"
        if(!r.checked||r.form!==formA||old.checked||peer.checked||!other.checked||!free.checked||
           r.getAttributeNode('type')!==typeAttr||typeAttr.value!=='radio'||unattached.ownerElement!==null)
           throw 'failed type operation changed radio or Attr';
    "#);
    neighbor_preserved(&doc,&forms);
}

#[test]
fn created_radio_attribute_admission_preflights_control_capacity_and_attribute_storage() {
    for mode in ["retained", "copy", "work", "capacity"] {
        let extra = if mode == "capacity" { "<input>".repeat(forms::MAX_CONTROLS - 6) } else { String::new() };
        let (mut doc, mut forms, mut page) = fixture(&extra);
        eval(&mut doc, &mut forms, &mut page, r#"
            var fresh=document.createElement('input');fresh.id='limited';
            var incoming=document.createAttribute('type');incoming.value='RaDiO';
        "#);
        let node = id(&doc, "limited");
        assert!(!forms.controls.contains_key(&node));
        assert_eq!(page.state.attributes.len(), 1);
        page.state.rebuild(&doc).unwrap();
        let before = snapshot(&doc,&forms,&page);
        let original_limit = page.state.retained_limit;
        match mode {
            "retained" => page.state.retained_limit = before.retained,
            "copy" => page.state.copy_work = 0,
            "work" => page.state.host_work = 0,
            _ => assert_eq!(forms.controls.len(), forms::MAX_CONTROLS),
        }
        let adopt = |doc: &mut Document, forms: &mut Forms, page: &mut Page| {
            Bridge { doc, forms, state: &mut page.state }
                .set_attribute_node(node_handle(node), &[Value::Host(script_attr::ATTR_BASE)])
        };
        assert!(adopt(&mut doc, &mut forms, &mut page).is_err(), "{mode}");
        assert_eq!(snapshot(&doc,&forms,&page), before, "{mode}");
        if mode == "retained" {
            page.state.retained_limit = original_limit;
            assert!(matches!(adopt(&mut doc, &mut forms, &mut page), Ok(Value::Null)));
            assert_eq!(page.state.attributes[0].owner.and_then(node_index), Some(node));
            assert_eq!(forms.controls[&node].kind, Kind::Radio);
            assert_eq!(forms.controls[&node].value.text(), "on");
        }
        neighbor_preserved(&doc,&forms);
    }
}

#[test]
fn created_radio_rejected_name_growth_cannot_clear_checked_peer_before_publication() {
    let (mut doc, mut forms, mut page) = fixture("");
    eval(&mut doc, &mut forms, &mut page, r#"
        var r=document.createElement('input');r.id='created';r.type='radio';r.name='x';
        slot.appendChild(r);r.checked=true;var nameAttr=r.getAttributeNode('name');
        if(!r.checked||!old.checked)throw 'rename setup';
    "#);
    let node = id(&doc, "created");
    page.state.rebuild(&doc).unwrap();
    let before = snapshot(&doc, &forms, &page);
    let original_limit = page.state.retained_limit;
    page.state.retained_limit = before.retained;
    assert!(Bridge {doc:&mut doc, forms:&mut forms, state:&mut page.state}
        .public_attribute(node,crate::html::AttributeName::unqualified("name".into()),Some("vote".into())).is_err());
    assert_eq!(snapshot(&doc,&forms,&page), before);
    page.state.retained_limit = original_limit;
    eval(&mut doc, &mut forms, &mut page, r#"
        if(nameAttr.ownerElement!==r||nameAttr.value!=='x'||!r.checked||!old.checked)throw 'rejected name storage';
        nameAttr.value='vote';
        if(r.getAttributeNode('name')!==nameAttr||r.name!=='vote'||!r.checked||old.checked||
           !other.checked||!free.checked)throw 'successful name storage';
    "#);
    neighbor_preserved(&doc,&forms);
}

#[test]
fn created_radio_parsed_replace_child_in_out_preserves_identity_and_projected_group() {
    let (mut doc,mut forms,mut page)=fixture("");
    eval(&mut doc,&mut forms,&mut page,r#"
        var placeholder=document.createElement('span');placeholder.textContent='old child';
        dest.appendChild(placeholder);var checkedAttr=old.getAttributeNode('checked');
        if(!old.checked||!other.checked||old.form!==formA||other.form!==formB)
            throw 'parsed replacement prefix';
        if(dest.replaceChild(old,placeholder)!==placeholder||old.parentNode!==dest||
           old.form!==formB||!old.checked||other.checked||placeholder.parentNode!==null||
           old.ownerDocument!==document||old.getAttributeNode('checked')!==checkedAttr)
            throw 'parsed native radio replaced into another form';
        peer.checked=true;slot.appendChild(placeholder);
        if(slot.replaceChild(old,placeholder)!==placeholder||old.form!==formA||
           !old.checked||peer.checked||other.checked||!free.checked||checkedAttr.ownerElement!==old)
            throw 'parsed native radio replaced back out';
        var shell=document.createElement('section');shell.id='replacement-shell';shell.appendChild(old);
        var detached=document.createElement('div');detached.appendChild(placeholder);
        if(detached.replaceChild(shell,placeholder)!==placeholder||old.form!==null||
           old.isConnected||!old.checked||!free.checked||old.getRootNode()!==detached)
            throw 'detached wrapper radio replacement';
        if(detached.replaceChild(placeholder,shell)!==shell||shell.parentNode!==null||
           old.getRootNode()!==shell||!old.checked||!free.checked||old.form!==null)
            throw 'detached replacement removal';
    "#);
    let old=id(&doc,"original");
    assert_eq!(forms.controls[&old].group_root,id(&doc,"replacement-shell"));
    assert_eq!(forms.controls[&old].owner,None);
    assert!(!forms.controls[&old].dirty_checked);
    neighbor_preserved(&doc,&forms);
}

#[test]
fn created_radio_detached_form_reset_uses_its_own_root_and_current_tree_order_once() {
    let (mut doc,mut forms,mut page)=fixture("");
    eval(&mut doc,&mut forms,&mut page,r#"
        var detachedForm=document.createElement('form');detachedForm.id='detached-form';
        detachedForm.appendChild(old);detachedForm.appendChild(peer);peer.setAttribute('checked','');
        detachedForm.appendChild(old);
        if(old.form!==detachedForm||peer.form!==detachedForm||old.isConnected||peer.isConnected||
           detachedForm.firstChild!==peer||detachedForm.lastChild!==old||old.checked||!peer.checked||
           !free.checked||!other.checked)throw 'detached form ownership/order prefix';
    "#);
    let detached_form=id(&doc,"detached-form");
    forms.reset_in_document(&doc,detached_form).unwrap();
    eval(&mut doc,&mut forms,&mut page,r#"
        if(!old.checked||peer.checked||!free.checked||!other.checked||
           old.form!==detachedForm||peer.form!==detachedForm)throw 'detached form reset used document/arena order';
        old.removeAttribute('checked');
        if(old.checked||peer.checked)throw 'detached reset left checkedness dirty';
    "#);
    assert_eq!(forms.controls[&id(&doc,"original")].group_root,detached_form);
    assert_eq!(forms.controls[&id(&doc,"peer")].group_root,detached_form);
    neighbor_preserved(&doc,&forms);
}

#[test]
fn created_radio_simultaneous_subtree_moves_resolve_each_projected_group_independently() {
    let (mut doc,mut forms,mut page)=fixture(
        "<div id=moving><input id=one type=radio name=one checked><input id=two type=radio name=two checked></div>"
    );
    eval(&mut doc,&mut forms,&mut page,r#"
        var moving=byId('moving'),one=byId('one'),two=byId('two');
        var targetOne=document.createElement('input'),targetTwo=document.createElement('input');
        targetOne.type='radio';targetOne.name='one';targetOne.checked=true;
        targetTwo.type='radio';targetTwo.name='two';targetTwo.checked=true;
        dest.appendChild(targetOne);dest.appendChild(targetTwo);
        if(!one.checked||!two.checked||!targetOne.checked||!targetTwo.checked||
           one.form!==null||two.form!==null||targetOne.form!==formB||targetTwo.form!==formB)
            throw 'two distinct initial group pairs';
        dest.appendChild(moving);
        if(!one.checked||!two.checked||targetOne.checked||targetTwo.checked||
           one.form!==formB||two.form!==formB||!other.checked||!free.checked)
            throw 'simultaneous projected group winners';
        var fragment=document.createDocumentFragment();fragment.appendChild(moving);
        if(!one.checked||!two.checked||targetOne.checked||targetTwo.checked||
           one.form!==null||two.form!==null||one.getRootNode()!==fragment||
           two.getRootNode()!==fragment||!other.checked||!free.checked)
            throw 'simultaneous detached group preservation';
    "#);
    assert!(!forms.controls[&id(&doc,"one")].dirty_checked);
    assert!(!forms.controls[&id(&doc,"two")].dirty_checked);
    neighbor_preserved(&doc,&forms);
}
