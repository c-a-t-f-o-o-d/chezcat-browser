//! Original native TEXTAREA fragment, RCDATA, lifetime, and atomicity controls.
use super::*;

const CONTENT: &str =
    "\nfirst &amp; <b>literal</b> &lt;/textarea&gt; <img src=/never-rcdata.png> &#13;&#10;last";
const RAW: &str = "first & <b>literal</b> </textarea> <img src=/never-rcdata.png> \r\nlast";
const VALUE: &str = "first & <b>literal</b> </textarea> <img src=/never-rcdata.png> \nlast";

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}

fn current() -> crate::navigation::Location {
    crate::navigation::address(
        "https://textarea-fragment.test/start",
        &crate::navigation::Location::home(),
    )
    .unwrap()
}

fn eval(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let result = page.console_eval(doc, forms, source);
    assert_eq!(result.errors, 0, "{source}\n{}", page.report());
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

fn no_effects(page: &mut Page) {
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (changed, removed, requested) = page.take_image_changes();
    assert!(changed.is_empty() && removed.is_empty() && requested.is_empty());
}

fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(&format!(
        "<html><body><form id=a action='/a'><input id=neighbor name=keep value=seed><div id=host><input id=old name=old value=old-default></div><input name=tail value=after></form><form id=b action='/b'><textarea id=reference name=reference value=ignored maxlength=3 readonly required tabindex=+2tail>{CONTENT}</textarea><div id=dest></div><fieldset disabled><legend><span id=allowed></span></legend><div id=blocked></div><legend><span id=second-legend></span></legend></fieldset></form><textarea id=associated form=a>associated</textarea><template id=inert></template><svg id=foreign></svg></body></html>"
    ));
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    page.set_location(&current());
    page.set_resource_base(&current());
    let result = page.start(
        &mut doc,
        &mut forms,
        &[Script {
            node: 0,
            label: "Original native TEXTAREA fragment".into(),
            source: r#"
            var neighbor=document.getElementById('neighbor'),old=document.getElementById('old');
            var host=document.getElementById('host'),dest=document.getElementById('dest');
            var reference=document.getElementById('reference');
            neighbor.value='neighbor edited';old.value='old edited';
            if(reference.nodeName!=='TEXTAREA'||reference.ownerDocument!==document||
               reference.form!==document.getElementById('b')||neighbor.defaultValue!=='seed'||
               old.defaultValue!=='old-default')throw 'parsed/native prefix';
            var oldAttribute=old.getAttributeNode('value');
        "#
            .into(),
        }],
    );
    assert_eq!(result.errors, 0, "{}", page.report());
    for (name, cursor, anchor) in [("neighbor", 2, 5), ("old", 1, 3)] {
        let edit = &mut forms.controls.get_mut(&id(&doc, name)).unwrap().value;
        edit.cursor = cursor;
        edit.anchor = anchor;
        edit.offset = 1;
    }
    let reference = id(&doc, "reference");
    assert_eq!(forms.controls[&reference].kind, Kind::TextArea);
    assert_eq!(forms.controls[&reference].value.text(), VALUE);
    assert_eq!(doc.nodes[reference].children.len(), 1);
    assert!(
        matches!(&doc.nodes[doc.nodes[reference].children[0]].kind, NodeKind::Text(text) if text == RAW)
    );
    (doc, forms, page)
}

fn query(doc: &Document, forms: &Forms, owner: &str) -> String {
    forms
        .submit_request(doc, id(doc, owner), None, &current(), &current())
        .unwrap()
        .location
        .url
        .unwrap()
        .query
        .unwrap_or_default()
}

#[test]
fn textarea_fragment_detached_rcdata_retains_text_and_matches_parsed_native_defaults() {
    let (mut doc, mut forms, mut page) = fixture();
    let kept = editor(&forms, id(&doc, "neighbor"));
    let old = editor(&forms, id(&doc, "old"));
    let markup = format!("<textarea id=fresh name=q value=ignored maxlength=3 readonly required tabindex=+2tail>{CONTENT}</textarea><textarea id=empty></textarea><textarea id=leading>\n\nsecond</textarea><span id=after-fragment>after</span>");
    // The first new operation is genuine native TEXTAREA fragment admission.
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        &format!(
            r#"
        var detached=document.createElement('div');detached.innerHTML={markup:?};
        var area=detached.firstChild,empty=area.nextSibling,leading=empty.nextSibling;
        if(area.ownerDocument!==document||area.parentNode!==detached||area.isConnected||
           area.form!==null||area.value!=={VALUE:?}||area.value!==reference.value||
           area.defaultValue!=={RAW:?}||area.defaultValue!==reference.defaultValue||
           area.textContent!=={RAW:?})throw 'detached native/default value';
        if(area.childNodes.length!==1||area.firstChild.nodeType!==3||
           area.firstChild.nodeValue!=={RAW:?}||area.firstChild.parentNode!==area||
           area.firstChild.ownerDocument!==document)throw 'retained decoded RCDATA';
        if(area.getAttribute('value')!=='ignored'||area.getAttribute('maxlength')!=='3'||
           area.getAttribute('name')!=='q')throw 'raw attributes';
        if(empty.value!==''||empty.defaultValue!==''||empty.textContent!==''||
           empty.childNodes.length!==0)throw 'empty textarea';
        if(leading.value!=='\nsecond'||leading.defaultValue!=='\nsecond'||
           leading.nextSibling.nodeName!=='SPAN')throw 'one initial LF and closing tag';
    "#
        ),
    );
    let fresh = id(&doc, "fresh");
    let c = &forms.controls[&fresh];
    let parsed = &forms.controls[&id(&doc, "reference")];
    assert_eq!(c.kind, Kind::TextArea);
    assert_eq!(c.owner, None);
    assert!(!c.dirty_value);
    assert_eq!(
        (
            c.value.text(),
            c.max_length,
            c.tab_index,
            c.readonly,
            c.required
        ),
        (
            parsed.value.text(),
            parsed.max_length,
            parsed.tab_index,
            parsed.readonly,
            parsed.required
        )
    );
    assert_eq!(
        (c.max_length, c.tab_index, c.readonly, c.required),
        (3, 2, true, true)
    );
    assert_eq!(editor(&forms, id(&doc, "neighbor")), kept);
    assert_eq!(editor(&forms, id(&doc, "old")), old);
    no_effects(&mut page);
}

#[test]
fn textarea_fragment_whole_moves_preserve_dirty_selection_default_and_reset_equivalence() {
    let (mut doc, mut forms, mut page) = fixture();
    let kept = editor(&forms, id(&doc, "neighbor"));
    let markup =
        format!("<section id=group><textarea id=fresh name=q>{CONTENT}</textarea></section>");
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        &format!(
            r#"
        host.innerHTML={markup:?};
        var group=document.getElementById('group'),area=document.getElementById('fresh');
        var text=area.firstChild;
        if(area.form!==document.getElementById('a')||!area.isConnected)throw 'initial owner';
        area.value='edited\r\narea';reference.value='edited\r\narea';
        if(area.value!=='edited\narea'||area.value!==reference.value||
           area.defaultValue!=={RAW:?}||area.textContent!=={RAW:?}||
           text!==area.firstChild)throw 'dirty native value with untouched default text';
    "#
        ),
    );
    let fresh = id(&doc, "fresh");
    let text_node = doc.nodes[fresh].children[0];
    let edit = &mut forms.controls.get_mut(&fresh).unwrap().value;
    edit.cursor = 2;
    edit.anchor = 7;
    edit.offset = 1;
    let edited = editor(&forms, fresh);
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        var fragment=document.createDocumentFragment();fragment.appendChild(group);
        if(area.form!==null||area.isConnected||area.getRootNode()!==fragment||
           area.firstChild!==text)throw 'genuine fragment move';
        dest.appendChild(fragment);
        if(fragment.childNodes.length!==0||area.form!==document.getElementById('b')||
           !area.isConnected||area.ownerDocument!==document||area.firstChild!==text)
            throw 'connected native move';
    "#,
    );
    assert_eq!(
        query(&doc, &forms, "b"),
        "reference=edited%0D%0Aarea&q=edited%0D%0Aarea"
    );
    for (destination, disabled) in [
        ("blocked", true),
        ("allowed", false),
        ("second-legend", true),
        ("dest", false),
    ] {
        eval(
            &mut doc,
            &mut forms,
            &mut page,
            &format!(
                r#"
            document.getElementById('{destination}').appendChild(group);
            if(area.value!=='edited\narea'||area.firstChild!==text||
               area.ownerDocument!==document)throw 'moved native state';
        "#
            ),
        );
        let after = editor(&forms, fresh);
        assert_eq!(
            (&after.0, after.1, after.2, after.3, after.4),
            (&edited.0, edited.1, edited.2, edited.3, edited.4)
        );
        assert_eq!(after.5, Some(id(&doc, "b")));
        assert_eq!(forms.controls[&fresh].disabled, disabled);
        assert_eq!(doc.nodes[fresh].children, [text_node]);
    }
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        dest.innerHTML='<p>replacement</p>';
        if(group.parentNode!==null||area.isConnected||area.form!==null||
           area.value!=='edited\narea'||area.firstChild!==text)throw 'retired native subtree';
        dest.appendChild(group);
    "#,
    );
    forms.reset(id(&doc, "b"));
    let reset = editor(&forms, fresh);
    let parsed = editor(&forms, id(&doc, "reference"));
    assert_eq!(reset, parsed);
    assert_eq!(reset.0, VALUE);
    assert!(!reset.4);
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        &format!(
            r#"
        if(area.defaultValue!=={RAW:?}||area.defaultValue!==reference.defaultValue||
           area.firstChild!==text||text.nodeValue!=={RAW:?})throw 'reset changed DOM default or identity';
    "#
        ),
    );
    assert_eq!(editor(&forms, id(&doc, "neighbor")), kept);
    no_effects(&mut page);
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    nodes: String,
    parents: Vec<Option<usize>>,
    connected: Vec<bool>,
    created: HashSet<usize>,
    detached: HashSet<usize>,
    controls: Vec<String>,
    attributes: Vec<(Option<usize>, String, String)>,
    retained: usize,
    commits: Option<u64>,
}

impl Snapshot {
    fn capture(doc: &Document, forms: &Forms, page: &Page) -> Self {
        Self {
            nodes: format!("{:?}", doc.nodes),
            parents: page.state.tree.parents.clone(),
            connected: page.state.dom.connected.clone(),
            created: page.state.dom.created.iter().collect(),
            detached: page.state.detached_nodes.iter().collect(),
            controls: forms
                .controls
                .iter()
                .map(|(&node, c)| {
                    format!(
                        "{node}:{:?}:{:?}:{:?}:{:?}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
                        c.kind,
                        c.owner,
                        c.name,
                        c.value.text(),
                        c.value.cursor,
                        c.value.anchor,
                        c.value.offset,
                        c.dirty_value,
                        c.checked,
                        c.disabled,
                        c.readonly,
                        c.required,
                        c.max_length,
                        c.tab_index
                    )
                })
                .collect(),
            attributes: page
                .state
                .attributes
                .iter()
                .map(|a| (a.owner, a.name.qualified.clone(), a.value.clone()))
                .collect(),
            retained: page.state.retained,
            commits: page.state.dom_commits,
        }
    }
}

#[test]
fn textarea_fragment_later_refused_member_keeps_prior_native_handles_and_state_atomic() {
    let (mut doc, mut forms, mut page) = fixture();
    page.state.rebuild(&doc).unwrap();
    let before = Snapshot::capture(&doc, &forms, &page);
    let host = id(&doc, "host");
    for suffix in [
        "<input type=file>",
        "<input type=radio>",
        "<select multiple><option>x</option></select>",
        "<link rel=stylesheet href=/never.css>",
        "<iframe></iframe>",
        "<span onclick='x=1'>handler</span>",
    ] {
        let source = format!(
            "<textarea id=never name=q>safe &amp; <img src=/not-a-resource></textarea>{suffix}"
        );
        let result = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        }
        .set_inner_html(host, &source);
        assert!(result.is_err(), "{source}");
        assert_eq!(Snapshot::capture(&doc, &forms, &page), before, "{source}");
        no_effects(&mut page);
    }
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        if(host.firstChild!==old||old.parentNode!==host||
           old.form!==document.getElementById('a')||old.value!=='old edited'||
           old.defaultValue!=='old-default'||old.getAttributeNode('value')!==oldAttribute||
           oldAttribute.ownerElement!==old||oldAttribute.value!=='old-default'||
           document.getElementById('never')!==null)throw 'refusal changed saved native handle';
    "#,
    );
}

#[test]
fn textarea_fragment_native_interiors_associations_and_contexts_remain_guarded() {
    let (mut doc, mut forms, mut page) = fixture();
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        host.innerHTML='<textarea id=fresh name=q>default</textarea>';
        var area=document.getElementById('fresh');area.value='edited';
    "#,
    );
    page.state.rebuild(&doc).unwrap();
    let before = Snapshot::capture(&doc, &forms, &page);
    for source in [
        "area.innerHTML='new default';",
        "area.textContent='new default';",
        "area.defaultValue='new default';",
        "area.firstChild.data='new default';",
        "area.firstChild.nodeValue='new default';",
        "area.firstChild.textContent='new default';",
        "document.getElementById('foreign').innerHTML='<textarea>text</textarea>';",
        "host.innerHTML='<textarea form=a>text</textarea>';",
        "host.innerHTML='<textarea>unclosed';",
        "host.appendChild(document.getElementById('associated'));",
    ] {
        let result = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(result.errors, 1, "{source}\n{}", page.report());
        assert_eq!(Snapshot::capture(&doc, &forms, &page), before, "{source}");
        no_effects(&mut page);
    }
    let controls=forms.controls.len();
    eval(&mut doc,&mut forms,&mut page,"document.getElementById('inert').innerHTML='<textarea id=inactive name=ghost>text</textarea>';if(document.getElementById('inactive')!==null||area.value!=='edited'||neighbor.value!=='neighbor edited')throw 'template activated native control';");
    assert_eq!(forms.controls.len(),controls);
    no_effects(&mut page);

}

#[test]
fn textarea_fragment_control_value_and_publication_quotas_commit_no_prefix() {
    for (existing, source) in [
        (
            "<input>".repeat(forms::MAX_CONTROLS - 3),
            "<textarea>first fits</textarea><textarea>second exceeds count</textarea>".to_owned(),
        ),
        (
            String::new(),
            format!(
                "<textarea>first fits</textarea><textarea>{}</textarea>",
                "🦀".repeat(forms::MAX_VALUE_UNITS / 2 + 1)
            ),
        ),
        (
            (0..31)
                .map(|_| {
                    format!(
                        "<textarea>{}</textarea>",
                        "x".repeat(forms::MAX_VALUE_UNITS)
                    )
                })
                .collect(),
            format!(
                "<textarea>{}</textarea>",
                "y".repeat(forms::MAX_VALUE_UNITS)
            ),
        ),
    ] {
        let mut doc = crate::html::parse(&format!(
            "<html><body><form id=a><input id=neighbor value=seed>{existing}<div id=host><input id=old value=old-default></div></form></body></html>"
        ));
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let result = page.start(&mut doc, &mut forms, &[Script {
            node: 0,
            label: "Original TEXTAREA quota positive controls".into(),
            source: "var old=document.getElementById('old');old.value='edited';var saved=old.getAttributeNode('value');document.getElementById('neighbor').value='neighbor edited';".into(),
        }]);
        assert_eq!(result.errors, 0, "{}", page.report());
        forms
            .validate_submission(&doc, id(&doc, "a"), None)
            .expect("quota setup already exceeded native admission");
        let old = id(&doc, "old");
        forms.controls.get_mut(&old).unwrap().value.cursor = 2;
        forms.controls.get_mut(&old).unwrap().value.anchor = 4;
        page.state.rebuild(&doc).unwrap();
        let before = Snapshot::capture(&doc, &forms, &page);
        let host = id(&doc, "host");
        assert!(Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state
        }
        .set_inner_html(host, &source)
        .is_err());
        assert_eq!(Snapshot::capture(&doc, &forms, &page), before);
        eval(
            &mut doc,
            &mut forms,
            &mut page,
            r#"
            if(document.getElementById('host').firstChild!==old||old.value!=='edited'||
               old.getAttributeNode('value')!==saved||saved.ownerElement!==old||
               saved.value!=='old-default')throw 'quota changed saved child or Attr';
        "#,
        );
        no_effects(&mut page);
    }
    for mode in ["retained", "nodes", "work", "copy"] {
        let (mut doc, mut forms, mut page) = fixture();
        page.state.rebuild(&doc).unwrap();
        let before = Snapshot::capture(&doc, &forms, &page);
        match mode {
            "retained" => page.state.retained_limit = page.state.retained,
            "nodes" => page.state.node_limit = doc.nodes.len(),
            "work" => page.state.host_work = 0,
            _ => page.state.copy_work = 0,
        }
        let host = id(&doc, "host");
        assert!(
            Bridge {
                doc: &mut doc,
                forms: &mut forms,
                state: &mut page.state
            }
            .set_inner_html(host, "<textarea name=q>new default</textarea>")
            .is_err(),
            "{mode}"
        );
        assert_eq!(Snapshot::capture(&doc, &forms, &page), before, "{mode}");
        no_effects(&mut page);
    }
}
