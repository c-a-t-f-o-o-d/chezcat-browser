//! Original TEXTAREA clone state, own-child defaults, and publication controls.
use super::*;

const CONTENT: &str = "\nfirst &amp; <b>literal</b> &#13;&#10;last";
const RAW: &str = "first & <b>literal</b> \r\nlast";
const API: &str = "first & <b>literal</b> \nlast";

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}

fn last_id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .rfind(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}

fn location() -> crate::navigation::Location {
    crate::navigation::address(
        "https://textarea-clone.test/source",
        &crate::navigation::Location::home(),
    )
    .unwrap()
}

fn eval(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let result = page.console_eval(doc, forms, source);
    assert_eq!(result.errors, 0, "{source}\n{}", page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (added, removed, requested) = page.take_image_changes();
    assert!(added.is_empty() && removed.is_empty() && requested.is_empty());
}

fn fixture(markup: &str) -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(markup);
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    page.set_location(&location());
    page.set_resource_base(&location());
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );
    (doc, forms, page)
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

fn edit_selection(forms: &mut Forms, node: usize) {
    let value = &mut forms.controls.get_mut(&node).unwrap().value;
    value.cursor = 2;
    value.anchor = 5;
    value.offset = 1;
}

#[test]
fn textarea_clone_parsed_deep_div_copies_native_values_flags_and_independent_plain_siblings() {
    let (mut doc, mut forms, mut page) = fixture(&format!(
        "<html><body><form id=a><input id=neighbor value=seed><div id=source title=original><span id=plain>before</span><textarea id=pristine name=p value=ignored maxlength=3 readonly required tabindex=+2tail>{CONTENT}</textarea><textarea id=dirty name=d>dirty default</textarea><span id=tail>after</span></div></form><form id=b><div id=dest></div></form></body></html>"
    ));
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        &format!(
            r#"
        var source=document.getElementById('source'),p=document.getElementById('pristine');
        var d=document.getElementById('dirty'),neighbor=document.getElementById('neighbor');
        var pText=p.firstChild,dText=d.firstChild,plain=source.firstChild,tail=source.lastChild;
        neighbor.value='edited neighbor';d.value='edited\rraw\r\nvalue';
        if(p.value!=={API:?}||p.defaultValue!=={RAW:?}||p.firstChild.nodeValue!=={RAW:?}||
           d.value!=='edited\nraw\nvalue'||d.defaultValue!=='dirty default'||
           source.childNodes.length!==4||plain.nextSibling!==p||p.nextSibling!==d||
           d.nextSibling!==tail||p.form!==document.getElementById('a')||
           p.ownerDocument!==document||d.ownerDocument!==document)throw 'parsed physical/native prefix';
    "#
        ),
    );
    let neighbor = id(&doc, "neighbor");
    let dirty = id(&doc, "dirty");
    edit_selection(&mut forms, neighbor);
    edit_selection(&mut forms, dirty);
    let kept = editor(&forms, neighbor);
    let original = editor(&forms, dirty);
    assert!(!forms.controls[&id(&doc, "pristine")].dirty_value);
    assert!(original.4);
    // The first newly admitted operation matches a real deep DIV containing
    // native TEXTAREA controls, rather than cloning an isolated surrogate.
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        &format!(
            r#"
        var copy=source.cloneNode(true),cp=copy.childNodes[1],cd=copy.childNodes[2];
        if(copy===source||copy.parentNode!==null||copy.isConnected||copy.ownerDocument!==document||
           copy.childNodes.length!==4||cp===p||cd===d||cp.parentNode!==copy||cd.parentNode!==copy||
           cp.form!==null||cd.form!==null||cp.ownerDocument!==document||cd.ownerDocument!==document)
            throw 'independent detached native clone';
        if(copy.firstChild===plain||copy.lastChild===tail||copy.firstChild.textContent!=='before'||
           copy.lastChild.textContent!=='after'||copy.title!=='original'||
           cp.firstChild===pText||cd.firstChild===dText||cp.firstChild.parentNode!==cp||
           cp.firstChild.nodeValue!=={RAW:?})throw 'actual copied children and siblings';
        if(cp.value!==p.value||cp.defaultValue!=={RAW:?}||cp.getAttribute('value')!=='ignored'||
           cp.getAttribute('maxlength')!=='3'||cd.value!==d.value||cd.defaultValue!=='dirty default')
            throw 'live value versus own DOM default';
        cd.value='copy only';copy.firstChild.textContent='copied plain';
        if(d.value!=='edited\nraw\nvalue'||d.firstChild!==dText||d.defaultValue!=='dirty default'||
           plain.textContent!=='before'||p.firstChild!==pText||document.getElementById('dirty')!==d)
            throw 'clone mutation reached original';
    "#
        ),
    );
    let clone_p = last_id(&doc, "pristine");
    let clone_d = last_id(&doc, "dirty");
    assert_ne!(clone_p, id(&doc, "pristine"));
    let c = &forms.controls[&clone_p];
    assert_eq!(
        (c.kind, c.owner, c.dirty_value),
        (Kind::TextArea, None, false)
    );
    assert_eq!(
        (c.readonly, c.required, c.max_length, c.tab_index),
        (true, true, 3, 2)
    );
    assert!(forms.controls[&clone_d].dirty_value);
    assert_eq!(editor(&forms, dirty), original);
    assert_eq!(editor(&forms, neighbor), kept);
}

#[test]
fn textarea_clone_fragment_deep_and_shallow_copy_dirty_flags_but_reset_uses_each_own_text() {
    let (mut doc, mut forms, mut page) = fixture(
        "<html><body><form id=a><input id=neighbor value=seed><div id=host></div></form><form id=b><div id=dest></div></form></body></html>"
    );
    let markup = format!("<textarea id=pristine name=p>{CONTENT}</textarea><textarea id=dirty name=d>old default</textarea><textarea id=equal name=e>equal</textarea>");
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        &format!(
            r#"
        var host=document.getElementById('host'),dest=document.getElementById('dest');
        host.innerHTML={markup:?};
        var p=host.childNodes[0],d=host.childNodes[1],equal=host.childNodes[2];
        var neighbor=document.getElementById('neighbor');neighbor.value='edited neighbor';
        d.value='live\r\nvalue';equal.value='equal';
        if(p.value!=={API:?}||p.defaultValue!=={RAW:?}||d.value!=='live\nvalue'||
           d.defaultValue!=='old default'||p.form!==document.getElementById('a')||
           equal.value!=='equal'||equal.defaultValue!=='equal')throw 'fragment native prefix';
    "#
        ),
    );
    let neighbor = id(&doc, "neighbor");
    edit_selection(&mut forms, neighbor);
    let kept = editor(&forms, neighbor);
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        &format!(
            r#"
        var pDeep=p.cloneNode(true),pShallow=p.cloneNode(false);
        var dDeep=d.cloneNode(true),dShallow=d.cloneNode();
        var eDeep=equal.cloneNode(true),eShallow=equal.cloneNode(false);
        if(pDeep.defaultValue!=={RAW:?}||pShallow.defaultValue!==''||pShallow.textContent!==''||
           pShallow.childNodes.length!==0||pShallow.value!=={API:?}||pDeep.value!=={API:?})
            throw 'pristine shallow raw value distinct from empty default';
        if(dDeep.defaultValue!=='old default'||dShallow.defaultValue!==''||
           dDeep.value!=='live\nvalue'||dShallow.value!=='live\nvalue'||
           eDeep.value!=='equal'||eShallow.value!=='equal')throw 'dirty copied live values';
        var clones=[pDeep,pShallow,dDeep,dShallow,eDeep,eShallow];
        for(var node of clones){{
            if(node.form!==null||node.parentNode!==null||node.isConnected||node.ownerDocument!==document)
                throw 'standalone copied native ownership';
            dest.appendChild(node);
            if(node.form!==document.getElementById('b')||!node.isConnected)throw 'independent clone owner';
        }}
    "#
        ),
    );
    let clones: Vec<_> = doc
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(node, _)| {
            forms
                .controls
                .get(&node)
                .filter(|c| c.owner == Some(id(&doc, "b")))
                .map(|_| node)
        })
        .collect();
    assert_eq!(clones.len(), 6);
    for (&node, dirty) in clones.iter().zip([false, false, true, true, true, true]) {
        assert_eq!(forms.controls[&node].dirty_value, dirty);
        let value = &forms.controls[&node].value;
        assert!(value.cursor <= value.chars.len() && value.anchor <= value.chars.len());
    }
    forms.reset(id(&doc, "b"));
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        &format!(
            r#"
        if(pDeep.value!=={API:?}||dDeep.value!=='old default'||eDeep.value!=='equal'||
           pShallow.value!==''||dShallow.value!==''||eShallow.value!=='')throw 'reset used source default';
        if(p.value!=={API:?}||d.value!=='live\nvalue'||equal.value!=='equal'||
           p.form!==document.getElementById('a')||d.form!==document.getElementById('a'))
            throw 'clone reset changed original control';
        var fragment=document.createDocumentFragment();fragment.appendChild(dDeep);
        if(dDeep.form!==null||dDeep.isConnected||dDeep.defaultValue!=='old default')throw 'clone fragment removal';
        host.appendChild(fragment);dDeep.value='moved copy';
        if(fragment.childNodes.length!==0||dDeep.form!==document.getElementById('a')||
           dDeep.value!=='moved copy'||d.value!=='live\nvalue')throw 'independent clone move';
    "#
        ),
    );
    for &node in &clones {
        if forms.controls[&node].owner == Some(id(&doc, "b")) {
            assert!(!forms.controls[&node].dirty_value);
        }
    }
    assert_eq!(editor(&forms, neighbor), kept);
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    nodes: String,
    parents: Vec<Option<usize>>,
    active: Vec<bool>,
    connected: Vec<bool>,
    created: HashSet<usize>,
    detached: HashSet<usize>,
    attributes: Vec<(Option<usize>, String, String)>,
    controls: Vec<String>,
    retained: usize,
    commits: Option<u64>,
}

fn snapshot(doc: &Document, forms: &Forms, page: &Page) -> Snapshot {
    Snapshot {
        nodes: format!("{:?}", doc.nodes),
        parents: page.state.tree.parents.clone(),
        active: page.state.tree.active.clone(),
        connected: page.state.dom.connected.clone(),
        created: page.state.dom.created.clone(),
        detached: page.state.detached_nodes.clone(),
        attributes: page
            .state
            .attributes
            .iter()
            .map(|a| (a.owner, a.name.qualified.clone(), a.value.clone()))
            .collect(),
        controls: forms
            .controls
            .iter()
            .map(|(&node, c)| {
                format!(
                    "{node}:{:?}:{:?}:{:?}:{:?}:{:?}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
                    c.kind,
                    c.name,
                    c.owner,
                    c.value.chars,
                    c.caption,
                    c.value.cursor,
                    c.value.anchor,
                    c.value.offset,
                    c.dirty_value,
                    c.disabled,
                    c.readonly,
                    c.required,
                    c.checked,
                    c.max_length,
                    c.tab_index
                )
            })
            .collect(),
        retained: page.state.retained,
        commits: page.state.dom_commits,
    }
}

#[test]
fn textarea_clone_later_unsupported_child_rolls_back_plain_and_native_staging() {
    for later in [
        "<input type=radio>",
        "<script type=application/json>{}</script>",
    ] {
        let (mut doc, mut forms, mut page) = fixture(&format!(
            "<html><body><form id=a><input id=neighbor value=seed><div id=source title=original><span>plain prefix</span><textarea id=area name=q>native default</textarea>{later}</div></form></body></html>"
        ));
        eval(
            &mut doc,
            &mut forms,
            &mut page,
            r#"
            var source=document.getElementById('source'),area=document.getElementById('area');
            var neighbor=document.getElementById('neighbor'),text=area.firstChild;
            var attr=source.getAttributeNode('title');
            area.value='edited source';neighbor.value='edited neighbor';
            if(area.form!==document.getElementById('a')||area.parentNode!==source||
               area.defaultValue!=='native default'||text.parentNode!==area||
               source.firstChild.textContent!=='plain prefix')throw 'atomic source prefix';
            var shallow=source.cloneNode(false);
            if(shallow.childNodes.length!==0||shallow.title!=='original'||shallow.parentNode!==null)
                throw 'unrelated shallow path';
        "#,
        );
        edit_selection(&mut forms, id(&doc, "area"));
        edit_selection(&mut forms, id(&doc, "neighbor"));
        page.state.rebuild(&doc).unwrap();
        let before = snapshot(&doc, &forms, &page);
        let result = page.console_eval(&mut doc, &mut forms, "source.cloneNode(true);");
        assert_eq!(result.errors, 1, "{later}\n{}", page.report());
        assert_eq!(snapshot(&doc, &forms, &page), before, "{later}");
        eval(
            &mut doc,
            &mut forms,
            &mut page,
            r#"
            if(source.childNodes[1]!==area||area.firstChild!==text||area.value!=='edited source'||
               area.defaultValue!=='native default'||area.form!==document.getElementById('a')||
               source.getAttributeNode('title')!==attr||attr.ownerElement!==source||
               neighbor.value!=='edited neighbor')throw 'failed clone changed live native/Attr identities';
        "#,
        );
    }
}

#[test]
fn textarea_clone_control_node_retained_and_work_limits_publish_no_partial_native_copy() {
    for mode in ["controls", "nodes", "retained", "work", "copy"] {
        let filler = if mode == "controls" {
            "<input>".repeat(forms::MAX_CONTROLS - 3)
        } else {
            String::new()
        };
        let (mut doc, mut forms, mut page) = fixture(&format!(
            "<html><body><form id=a><input id=neighbor value=seed>{filler}<div id=source title=original><span>plain prefix</span><textarea id=first name=one>first default</textarea><textarea id=last name=two>last default</textarea></div></form></body></html>"
        ));
        eval(
            &mut doc,
            &mut forms,
            &mut page,
            r#"
            var source=document.getElementById('source'),first=document.getElementById('first');
            var last=document.getElementById('last'),neighbor=document.getElementById('neighbor');
            var text=first.firstChild,attr=source.getAttributeNode('title');
            first.value='first edited';last.value='last edited';neighbor.value='edited neighbor';
            if(source.childNodes.length!==3||first.form!==document.getElementById('a')||
               last.form!==first.form||first.defaultValue!=='first default'||
               text.parentNode!==first)throw 'quota admitted source prefix';
        "#,
        );
        forms
            .validate_submission(&doc, id(&doc, "a"), None)
            .expect("quota setup exceeded existing native admission");
        edit_selection(&mut forms, id(&doc, "first"));
        edit_selection(&mut forms, id(&doc, "neighbor"));
        page.state.rebuild(&doc).unwrap();
        let before = snapshot(&doc, &forms, &page);
        match mode {
            "controls" => assert_eq!(forms.controls.len(), forms::MAX_CONTROLS),
            // Seven copied nodes are required; the last textarea's text node
            // exceeds this allowance after earlier descendants were staged.
            "nodes" => page.state.node_limit = doc.nodes.len() + 6,
            "retained" => page.state.retained_limit = page.state.retained,
            "work" => page.state.host_work = 0,
            _ => page.state.copy_work = 0,
        }
        let source = id(&doc, "source");
        let result = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        }
        .clone_node(node_handle(source), true);
        assert!(result.is_err(), "{mode}");
        assert_eq!(snapshot(&doc, &forms, &page), before, "{mode}");
        assert!(!page.has_navigation_request());
        assert!(page.take_dynamic_scripts().is_empty());
        assert!(page.take_dynamic_stylesheets().is_empty());
        assert!(page.take_image_changes().2.is_empty());
    }
}
