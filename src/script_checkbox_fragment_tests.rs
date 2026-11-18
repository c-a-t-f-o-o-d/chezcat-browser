//! Original native checkbox fragment admission, lifetime and atomicity controls.
use super::*;

const MIXED: &str = "  <link/><table></table><a href='/a'>a</a><input type='checkbox'/>";

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}
fn current() -> crate::navigation::Location {
    crate::navigation::address("https://checkbox.test/start", &crate::navigation::Location::home()).unwrap()
}
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<html><body><input id=literal type=checkbox value=yes checked><form id=a action='/a'><input id=neighbor name=keep value=seed><div id=host><input id=old name=old value=old-default></div><input name=tail value=after></form><form id=b action='/b'><div id=dest></div><fieldset disabled><legend><span id=allowed></span></legend><div id=blocked></div><legend><span id=second-legend></span></legend></fieldset></form><template id=inert></template><svg id=foreign></svg></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    page.set_location(&current());
    let result = page.start(&mut doc, &mut forms, &[Script {
        node: 0, label: "Original native checkbox fragment".into(),
        source: "var neighbor=document.getElementById('neighbor'),old=document.getElementById('old'),host=document.getElementById('host'),dest=document.getElementById('dest');neighbor.value='neighbor edited';old.value='old edited';if(!document.getElementById('literal').checked||document.getElementById('literal').value!=='yes'||neighbor.defaultValue!=='seed'||old.form!==document.getElementById('a'))throw 'native prefix';".into(),
    }]);
    assert_eq!(result.errors, 0, "{}", page.report());
    let edit = &mut forms.controls.get_mut(&id(&doc, "neighbor")).unwrap().value;
    edit.cursor = 2; edit.anchor = 5; edit.offset = 1;
    (doc, forms, page)
}
fn eval(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let result = page.console_eval(doc, forms, source);
    assert_eq!(result.errors, 0, "{source}\n{}", page.report());
}
fn query(doc: &Document, forms: &Forms, owner: &str) -> String {
    forms.submit_request(doc, id(doc, owner), None, &current(), &current()).unwrap()
        .location.url.unwrap().query.unwrap_or_default()
}
fn editor(forms: &Forms, node: usize) -> (String, usize, usize, usize, bool, Option<usize>) {
    let c = &forms.controls[&node];
    (c.value.text(), c.value.cursor, c.value.anchor, c.value.offset, c.dirty_value, c.owner)
}
fn no_resources(page: &mut Page) {
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    assert!(page.take_image_changes().2.is_empty());
    assert!(!page.has_navigation_request());
}

#[test]
fn checkbox_fragment_admits_real_native_checked_default_on_and_explicit_empty_values() {
    let (mut doc, mut forms, mut page) = fixture();
    let kept = editor(&forms, id(&doc, "neighbor"));
    eval(&mut doc, &mut forms, &mut page, r#"
        host.innerHTML='<input id=fresh type=cHeCkBoX name=choice value=yes checked required readonly tabindex=+2tail>';
        var fresh=document.getElementById('fresh');
        if(fresh.parentNode!==host||fresh.ownerDocument!==document||!fresh.isConnected||
            fresh.form!==document.getElementById('a')||!fresh.checked||fresh.value!=='yes'||
            fresh.getAttribute('type')!=='cHeCkBoX'||fresh.getAttribute('checked')!=='')throw 'connected native checkbox';
        var detached=document.createElement('div');
        detached.innerHTML='<input id=bare type=checkbox name=bare checked><input id=empty type=checkbox name=empty value="">';
        var bare=detached.firstChild,empty=bare.nextSibling;
        if(bare.ownerDocument!==document||bare.parentNode!==detached||bare.isConnected||bare.form!==null||
            !bare.checked||bare.value!=='on'||bare.getAttribute('value')!==null||
            empty.checked||empty.value!==''||empty.getAttribute('value')!=='')throw 'detached default-on and empty';
        if(old.form!==null||old.isConnected||old.value!=='old edited'||old.defaultValue!=='old-default')throw 'saved old control';
    "#);
    let fresh = &forms.controls[&id(&doc, "fresh")];
    assert_eq!(fresh.kind, Kind::Checkbox);
    assert!(fresh.checked && fresh.readonly && fresh.required && !fresh.dirty_value);
    assert_eq!(fresh.tab_index, 2);
    let literal = &forms.controls[&id(&doc, "literal")];
    assert_eq!((fresh.value.text(), fresh.caption.as_str(), fresh.checked), (literal.value.text(), literal.caption.as_str(), literal.checked));
    for name in ["bare", "empty"] {
        let c = &forms.controls[&id(&doc, name)];
        assert_eq!(c.kind, Kind::Checkbox);
        assert_eq!(c.owner, None);
    }
    assert_eq!(query(&doc, &forms, "a"), "keep=neighbor+edited&choice=yes&tail=after");
    assert_eq!(editor(&forms, id(&doc, "neighbor")), kept);
    no_resources(&mut page);
}

#[test]
fn checkbox_fragment_live_state_survives_fragment_moves_fieldsets_retirement_and_reset() {
    let (mut doc, mut forms, mut page) = fixture();
    let kept = editor(&forms, id(&doc, "neighbor"));
    eval(&mut doc, &mut forms, &mut page, r#"
        var detached=document.createElement('div');
        detached.innerHTML='<input id=checked type=checkbox name=choice value=yes checked><input id=unchecked type=checkbox name=bare>';
        var check=detached.firstChild,unchecked=check.nextSibling,fragment=document.createDocumentFragment();
        var parsed=document.getElementById('literal'), parsedValue=parsed.getAttributeNode('value'), checkValue=check.getAttributeNode('value');
        document.getElementById('allowed').appendChild(parsed);
        parsed.checked=false;parsed.value='live';
        check.checked=false;check.value='live';unchecked.checked=true;
        if(parsedValue.value!=='live'||checkValue.value!=='live'||parsed.checked||check.checked)throw 'default/on value and live checkedness';
        fragment.appendChild(detached);
        if(check.getRootNode()!==fragment||check.isConnected||check.form!==null)throw 'detached fragment';
        dest.appendChild(fragment);
        if(fragment.childNodes.length!==0||check.form!==document.getElementById('b')||
            !check.isConnected||check.ownerDocument!==document||check.checked||check.value!=='live'||!unchecked.checked)throw 'connected move';
    "#);
    let parsed = id(&doc, "literal");
    let check = id(&doc, "checked");
    let unchecked = id(&doc, "unchecked");
    let identity = page.runtime.execute("check;", &mut Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }).unwrap();
    assert!(matches!(identity, Value::Host(_)));
    assert_eq!(query(&doc, &forms, "b"), "bare=on");
    for (destination, disabled) in [("blocked", true), ("allowed", false), ("second-legend", true), ("dest", false)] {
        eval(&mut doc, &mut forms, &mut page, &format!("document.getElementById('{destination}').appendChild(detached);if(check.value!=='live'||check.checked||!unchecked.checked||check.ownerDocument!==document)throw 'moved live state';"));
        assert_eq!(forms.controls[&check].disabled, disabled);
        assert_eq!(forms.controls[&unchecked].disabled, disabled);
        assert_eq!(forms.controls[&check].owner, Some(id(&doc, "b")));
    }
    eval(&mut doc, &mut forms, &mut page, "dest.innerHTML='<p>replacement</p>';if(check.form!==null||check.isConnected||check.value!=='live'||check.checked||detached.parentNode!==null)throw 'retired native subtree';dest.appendChild(detached);");
    forms.reset(id(&doc, "b"));
    // Checkbox value is in default/on mode: assignment changed the content
    // attribute, while the checked IDL assignment changed only checkedness.
    for node in [parsed, check] {
        assert_eq!(forms.controls[&node].owner, Some(id(&doc, "b")));
        assert!(forms.controls[&node].checked);
        assert_eq!(forms.controls[&node].value.text(), "live");
        assert_eq!(doc.attr(node, "value"), Some("live"));
        assert_eq!(doc.attr(node, "checked"), Some(""));
    }
    assert!(!forms.controls[&unchecked].checked);
    assert_eq!(forms.controls[&unchecked].value.text(), "on");
    assert_eq!(doc.attr(unchecked, "value"), None);
    assert_eq!(query(&doc, &forms, "b"), "choice=live");
    assert_eq!(doc.attr(unchecked, "checked"), None);
    eval(&mut doc, &mut forms, &mut page, "if(parsedValue!==parsed.getAttributeNode('value')||checkValue!==check.getAttributeNode('value')||!parsed.checked||!check.checked)throw 'reset changed Attr identity or failed to restore checkedness';");
    assert_eq!(editor(&forms, id(&doc, "neighbor")), kept);
    no_resources(&mut page);
}

#[test]
fn checkbox_fragment_exact_original_mixed_markup_is_positive_detached_and_connected() {
    let (mut doc, mut forms, mut page) = fixture();
    let kept = editor(&forms, id(&doc, "neighbor"));
    let old = id(&doc, "old");
    let old_editor = editor(&forms, old);
    for target in ["document.createElement('div')", "host"] {
        eval(&mut doc, &mut forms, &mut page, &format!(r#"
            var holder={target};holder.innerHTML={MIXED:?};
            var list=holder.childNodes, mixedBox=list[4];
            if(list.length!==5||list[0].nodeType!==3||list[0].nodeValue!=='  '||
               list[1].nodeName!=='LINK'||list[2].nodeName!=='TABLE'||list[3].nodeName!=='A'||
               mixedBox.nodeName!=='INPUT'||list[3].getAttribute('href')!=='/a'||
               mixedBox.value!=='on'||mixedBox.checked)throw 'exact original mixed fragment';
            if(mixedBox.ownerDocument!==document||mixedBox.parentNode!==holder||
               mixedBox.isConnected!==holder.isConnected)throw 'mixed ownership';
        "#));
        let expected_owner = if target == "host" { Some(id(&doc, "a")) } else { None };
        let holder = if target == "host" { id(&doc, "host") } else {
            doc.nodes.iter().rposition(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag=="div") && node.children.len()==5).unwrap()
        };
        let input = doc.nodes[holder].children[4];
        assert_eq!(forms.controls[&input].kind, Kind::Checkbox);
        assert_eq!(forms.controls[&input].owner, expected_owner);
        assert_eq!(editor(&forms, id(&doc, "neighbor")), kept);
        no_resources(&mut page);
    }
    let old_after = editor(&forms, old);
    assert_eq!((old_after.0, old_after.1, old_after.2, old_after.3, old_after.4),
        (old_editor.0, old_editor.1, old_editor.2, old_editor.3, old_editor.4));
    assert_eq!(old_after.5, None);
    assert_eq!(forms.text_default_value(old).unwrap(), "old-default");
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    nodes: String, parents: Vec<Option<usize>>, connected: Vec<bool>, created: HashSet<usize>, detached: HashSet<usize>,
    controls: Vec<String>, attributes: Vec<(Option<usize>, String, String)>, retained: usize, commits: Option<u64>,
}
impl Snapshot {
    fn capture(doc: &Document, forms: &Forms, page: &Page) -> Self {
        Self { nodes: format!("{:?}", doc.nodes), parents: page.state.tree.parents.clone(),
            connected: page.state.dom.connected.clone(), created: page.state.dom.created.clone(), detached: page.state.detached_nodes.clone(),
            controls: forms.controls.iter().map(|(&node,c)| format!("{node}:{:?}:{:?}:{:?}:{:?}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}", c.kind,c.owner,c.name,c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.checked,c.disabled,c.readonly,c.required,c.max_length,c.tab_index)).collect(),
            attributes: page.state.attributes.iter().map(|attr|(attr.owner,attr.name.qualified.clone(),attr.value.clone())).collect(),
            retained: page.state.retained, commits: page.state.dom_commits }
    }
}

#[test]
fn checkbox_fragment_later_unsupported_child_context_and_resource_failures_are_atomic() {
    let (mut doc, mut forms, mut page) = fixture();
    eval(&mut doc, &mut forms, &mut page, "host.innerHTML='<input id=keep-check type=checkbox name=kept value=default checked>';var check=document.getElementById('keep-check');check.checked=false;check.value='edited';");
    page.state.rebuild(&doc).unwrap();
    let before = Snapshot::capture(&doc, &forms, &page);
    let host = id(&doc, "host");
    for source in [
        "<input type=checkbox><input type=radio>", "<input type=checkbox><input type=hidden>",
        "<input type=checkbox><input type=file>", "<input type=checkbox><select multiple><option>x</select>",
        "<input type=checkbox><link rel=stylesheet href=/never.css>", "<input type=checkbox><iframe></iframe>",
        "<input type=checkbox><span onclick='x=1'>handler</span>", "<input type=checkbox><form></form>",
        "<input type=checkbox form=a>", "<input type=' checkbox '>", "<input type=checkbox><svg><input></svg>",
    ] {
        let error = Bridge {doc: &mut doc, forms: &mut forms, state: &mut page.state}.set_inner_html(host, source);
        assert!(error.is_err(), "{source}");
        assert_eq!(Snapshot::capture(&doc, &forms, &page), before, "{source}");
        no_resources(&mut page);
    }
    for source in ["document.getElementById('inert').innerHTML='<input type=checkbox>';", "check.innerHTML='<input type=checkbox>';", "document.getElementById('foreign').innerHTML='<input type=checkbox>';", "check.type='radio';"] {
        let result = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(result.errors, 1, "{source}: {}", page.report());
        assert_eq!(Snapshot::capture(&doc, &forms, &page), before, "{source}");
        no_resources(&mut page);
    }
}

#[test]
fn checkbox_fragment_native_count_value_payload_and_retention_limits_publish_no_prefix() {
    for (existing, source) in [
        ("<input>".repeat(forms::MAX_CONTROLS - 2), "<input type=checkbox><input type=checkbox>".to_owned()),
        (String::new(), format!("<input type=checkbox><input type=checkbox value='{}'>", "🦀".repeat(forms::MAX_VALUE_UNITS / 2 + 1))),
        ((0..31).map(|_|format!("<input value='{}'>", "x".repeat(forms::MAX_VALUE_UNITS))).collect(), format!("<input type=checkbox value='{}'>", "y".repeat(forms::MAX_VALUE_UNITS))),
        (String::new(), format!("<input type=checkbox><input type=checkbox name='{}'>", "n".repeat(8193))),
    ] {
        let mut doc = crate::html::parse(&format!("<body><form id=a><input id=neighbor value=seed>{existing}<div id=host>kept</div></form></body>"));
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let result = page.start(&mut doc, &mut forms, &[]);
        assert_eq!(result.errors, 0);
        forms.controls.get_mut(&id(&doc,"neighbor")).unwrap().value.set("edited");
        page.state.rebuild(&doc).unwrap();
        let before = Snapshot::capture(&doc,&forms,&page);
        let host = id(&doc,"host");
        assert!(Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}.set_inner_html(host,&source).is_err());
        assert_eq!(Snapshot::capture(&doc,&forms,&page),before);
        no_resources(&mut page);
    }
    for mode in ["retained", "nodes", "work", "copy"] {
        let (mut doc,mut forms,mut page)=fixture();
        page.state.rebuild(&doc).unwrap();
        let before=Snapshot::capture(&doc,&forms,&page);
        match mode {
            "retained"=>page.state.retained_limit=page.state.retained,
            "nodes"=>page.state.node_limit=doc.nodes.len(),
            "work"=>page.state.host_work=0,
            _=>page.state.copy_work=0,
        }
        let host=id(&doc,"host");
        assert!(Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}.set_inner_html(host,"<input type=checkbox name=choice checked>").is_err(),"{mode}");
        assert_eq!(Snapshot::capture(&doc,&forms,&page),before,"{mode}");
        no_resources(&mut page);
    }
}
