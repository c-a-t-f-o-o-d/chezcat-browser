//! Native checkedness, group ownership and bounded publication of cloned radios.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .expect(name)
}

fn eval(doc: &mut Document, forms: &mut Forms, page: &mut Page, script: &str) {
    let out = page.console_eval(doc, forms, script);
    assert_eq!(out.errors, 0, "{script}\n{}", page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (added, removed, requested) = page.take_image_changes();
    assert!(added.is_empty() && removed.is_empty() && requested.is_empty());
}

fn fixture(extra: &str) -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(&format!(
        "<html><body><form id=a><input id=neighbor name=note value=seed><div id=container><input id=one type=radio name=vote checked value=alpha><input id=two type=radio name=vote value=beta></div></form><form id=b><input id=other type=radio name=vote checked><div id=dest></div></form>{extra}</body></html>"
    ));
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let location = crate::navigation::address(
        "https://radio-clone.test/source",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&location);
    page.set_resource_base(&location);
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
        function byId(id){return document.getElementById(id);}
        var one=byId('one'),two=byId('two'),other=byId('other'),ownerA=byId('a'),ownerB=byId('b'),dest=byId('dest');
        var neighbor=byId('neighbor');neighbor.value='edited neighbor';
        if(!one.checked||two.checked||!other.checked||one.form!==ownerA||other.form!==ownerB)throw 'parsed groups';
    "#,
    );
    let edit = &mut forms.controls.get_mut(&id(&doc, "neighbor")).unwrap().value;
    edit.cursor = 5;
    edit.anchor = 2;
    edit.offset = 1;
    (doc, forms, page)
}

fn kept(doc: &Document, forms: &Forms) {
    let n = id(doc, "neighbor");
    let c = &forms.controls[&n];
    assert_eq!(c.value.text(), "edited neighbor");
    assert_eq!((c.value.cursor, c.value.anchor, c.value.offset), (5, 2, 1));
    assert!(c.dirty_value);
    assert_eq!(c.owner, Some(id(doc, "a")));
    assert_eq!(forms.text_default_value(n).unwrap(), "seed");
}

#[test]
fn radio_clone_created_double_deep_preserves_checked_and_creates_independent_detached_groups() {
    let (mut doc, mut forms, mut page) = fixture("");
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        var fragment=document.createDocumentFragment(),holder=document.createElement('div'),r=document.createElement('input');
        fragment.appendChild(holder);
        r.setAttribute('type','RaDiO');r.setAttribute('checked','checked');r.setAttribute('name','vote');holder.appendChild(r);
        var first=holder.cloneNode(true),again=first.cloneNode(true),a=first.lastChild,b=again.lastChild;
        if(!b.checked||!a.checked||!r.checked||a===r||b===a||first===again||r.getRootNode()!==fragment||
           a.getRootNode()!==first||b.getRootNode()!==again||a.form!==null||b.form!==null||
           a.ownerDocument!==document||b.ownerDocument!==document||first.parentNode!==null||again.parentNode!==null)
            throw 'detached double clone';
        var raw=b.getAttributeNode('checked');
        if(raw===r.getAttributeNode('checked')||raw.ownerElement!==b||raw.value!=='checked'||
           b.getAttribute('type')!=='RaDiO'||b.name!=='vote'||!b.defaultChecked||b.value!=='on')
            throw 'copied raw/default attributes';
        a.checked=false;a.checked=true;
        if(!r.checked||!a.checked||!b.checked||!one.checked||!other.checked)throw 'detached roots interfered';
        dest.appendChild(again);if(b.form!==ownerB||!b.checked||other.checked||!one.checked||!a.checked||!r.checked)
            throw 'connected clone did not join destination group';
        other.checked=true;
        if(b.checked||!other.checked||!one.checked||!a.checked||!r.checked||!b.defaultChecked)
            throw 'shared native selection escaped new group';
    "#,
    );
    forms.reset_in_document(&doc, id(&doc, "b")).unwrap();
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        if(!b.checked||other.checked||!a.checked||!r.checked||!one.checked||two.checked||b.form!==ownerB)
            throw 'destination reset did not use clone raw checked in tree order';
    "#,
    );
    kept(&doc, &forms);
}

#[test]
fn radio_clone_shallow_and_deep_preserve_dirty_and_pristine_checkedness_separately_from_defaults() {
    let (mut doc, mut forms, mut page) = fixture(
        "<fieldset disabled><input id=inherited type=radio required tabindex=3></fieldset>",
    );
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        one.checked=false;two.checked=true;
        var enabled=byId('inherited').cloneNode(false);enabled.id='enabled-copy';
        var d1=one.cloneNode(true),s1=one.cloneNode(),d2=two.cloneNode(true),s2=two.cloneNode(false);
        var clean=other.cloneNode(false),dc=byId('container').cloneNode(true),withinOne=dc.firstChild,withinTwo=dc.lastChild;
        for(var clone of [d1,s1,withinOne]){
            if(clone.checked||!clone.defaultChecked||clone.value!=='alpha'||clone.getAttribute('checked')===null||
               clone.ownerDocument!==document||clone.form!==null)throw 'unchecked live with checked default';
            clone.defaultChecked=false;clone.defaultChecked=true;
            if(clone.checked||!clone.defaultChecked)throw 'clone lost dirty false checkedness';
        }
        for(var clone of [d2,s2,withinTwo]){
            if(!clone.checked||clone.defaultChecked||clone.value!=='beta'||clone.getAttribute('checked')!==null||
               clone.ownerDocument!==document||clone.form!==null)throw 'checked live with unchecked default';
            clone.defaultChecked=true;clone.defaultChecked=false;
            if(!clone.checked||clone.defaultChecked)throw 'clone lost dirty true checkedness';
        }
        clean.defaultChecked=false;
        if(clean.checked||!other.checked||!other.defaultChecked||one.checked||!two.checked||
           !one.defaultChecked||two.defaultChecked||withinOne.getRootNode()!==dc||withinTwo.getRootNode()!==dc)
            throw 'pristine clone or source independence';
        withinOne.checked=true;
        if(withinTwo.checked||!withinOne.checked||one.checked||!two.checked||d1.checked||!s2.checked)
            throw 'cloned siblings did not share just their own root';
        dest.appendChild(dc);
        if(!withinOne.checked||other.checked||withinOne.form!==ownerB||withinTwo.form!==ownerB)throw 'physical owner';
    "#,
    );
    assert!(forms.controls[&id(&doc, "inherited")].disabled);
    let enabled = &forms.controls[&id(&doc, "enabled-copy")];
    assert_eq!(
        (
            enabled.disabled,
            enabled.required,
            enabled.tab_index,
            enabled.owner
        ),
        (false, true, 3, None)
    );
    forms.reset_in_document(&doc, id(&doc, "b")).unwrap();
    eval(&mut doc,&mut forms,&mut page,
        "if(!withinOne.checked||withinTwo.checked||other.checked||one.checked||!two.checked)throw 'independent clone reset';");
    kept(&doc, &forms);
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    nodes: String,
    parents: Vec<Option<usize>>,
    active: Vec<bool>,
    connected: Vec<bool>,
    detached: HashSet<usize>,
    created: HashSet<usize>,
    attrs: Vec<(Option<usize>, String, String)>,
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
        detached: page.state.detached_nodes.iter().collect(),
        created: page.state.dom.created.iter().collect(),
        attrs: page
            .state
            .attributes
            .iter()
            .map(|attr| (attr.owner.and_then(node_index), attr.name.qualified.clone(), attr.value.scalar().unwrap().to_owned()))
            .collect(),
        controls: forms
            .controls
            .iter()
            .map(|(node, c)| {
                format!(
                    "{:?}",
                    (
                        node,
                        c.kind,
                        c.group_root,
                        c.owner,
                        &c.name,
                        c.checked,
                        c.dirty_checked,
                        c.dirty_value,
                        c.value.text(),
                        (c.value.cursor, c.value.anchor, c.value.offset),
                    )
                )
            })
            .collect(),
        retained: page.state.retained,
        commits: page.state.dom_commits,
    }
}

fn staged(extra: &str, later: &str) -> (Document, Forms, Page) {
    let (mut doc,mut forms,mut page)=fixture(&format!(
        "<div id=source title=original><input id=choice type=radio checked name=other value=kept><textarea id=area>plain default</textarea>{later}</div>{extra}"
    ));
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        var source=byId('source'),choice=byId('choice'),area=byId('area'),raw=choice.getAttributeNode('checked');
        choice.checked=false;area.value='editing text';var label=source.getAttributeNode('title');
        if(!choice.defaultChecked||choice.checked||raw.ownerElement!==choice||area.defaultValue!=='plain default')
            throw 'mixed native clone setup';
    "#,
    );
    page.state.rebuild(&doc).unwrap();
    (doc, forms, page)
}

fn clone_source(doc: &mut Document, forms: &mut Forms, page: &mut Page) -> Result<Value, String> {
    let source = id(doc, "source");
    Bridge {
        doc,
        forms,
        state: &mut page.state,
    }
    .clone_node(node_handle(source), true)
}

#[test]
fn radio_clone_later_protected_descendant_rolls_back_native_staging_and_preserves_source_handles() {
    for later in [
        "<span><input type=checkbox></span>",
        "<span><script type=application/json>{}</script></span>",
    ] {
        let (mut doc, mut forms, mut page) = staged("", later);
        eval(&mut doc,&mut forms,&mut page,
            "var shell=source.cloneNode(false);if(shell.firstChild!==null||shell.title!=='original')throw 'shallow container';");
        page.state.rebuild(&doc).unwrap();
        let before = snapshot(&doc, &forms, &page);
        let out = page.console_eval(&mut doc, &mut forms, "source.cloneNode(true);");
        assert_eq!(out.errors, 1, "{later}\n{}", page.report());
        assert_eq!(snapshot(&doc, &forms, &page), before, "{later}");
        eval(
            &mut doc,
            &mut forms,
            &mut page,
            r#"
            if(choice.checked||!choice.defaultChecked||raw.ownerElement!==choice||choice.getAttributeNode('checked')!==raw||
               source.getAttributeNode('title')!==label||area.value!=='editing text'||area.defaultValue!=='plain default'||
               source.firstChild!==choice||choice.nextSibling!==area||!one.checked||two.checked||!other.checked)
                throw 'refusal changed native or DOM state';
        "#,
        );
        kept(&doc, &forms);
    }
}

#[test]
fn radio_clone_exact_node_native_storage_and_work_boundaries_publish_once_or_restore_all_state() {
    let (mut doc, mut forms, mut page) = staged("", "");
    page.state.rebuild(&doc).unwrap();
    let base = doc.nodes.len();
    let native_count = forms.controls.len();
    let old_bytes = page.state.retained;
    let old_capacity = doc.nodes.capacity();
    let old_states=page.state.node_state_bytes();
    let minimum=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.main_node_minimum(4).unwrap();
    let old_work = page.state.host_work;
    let old_copies = page.state.copy_work;
    let result = clone_source(&mut doc, &mut forms, &mut page).unwrap();
    assert!(matches!(result,Value::Host(handle) if handle==node_handle(base)));
    assert_eq!(doc.nodes.len() - base, 4);
    assert_eq!(forms.controls.len() - native_count, 2);
    let spent_work = old_work - page.state.host_work;
    let spent_copies = old_copies - page.state.copy_work;
    // A selected exact budget can omit the default geometric spare slots.
    // Existing-record movement/init work is independent of that spare capacity.
    let retained = page.state.retained - old_bytes
        -(doc.nodes.capacity()-old_capacity)*size_of::<Node>()
        -(page.state.node_state_bytes()-old_states)+minimum;
    let dom_bytes: usize = doc.nodes[base..]
        .iter()
        .map(|n| {
            node_bytes(n) + child_link_bytes(&n.children)
                + match &n.kind {
                    NodeKind::Element { tag, href, .. } => {
                        tag.len() + href.as_ref().map_or(0, String::len)
                    }
                    _ => 0,
                }
        })
        .sum();
    let native_bytes: usize = doc.nodes[base]
        .children
        .iter()
        .zip(["kept", "plain default"])
        .map(|(node, initial)| {
            let control = &forms.controls[node];
            control.name.len()
                + control.caption.len()
                + initial.len()
                + control.value.chars.len() * std::mem::size_of::<char>()
        })
        .sum();
    assert!(
        retained >= dom_bytes + native_bytes
            && spent_copies >= dom_bytes + native_bytes
            && spent_work > 1
    );
    for mode in ["nodes", "controls", "retained", "work", "copies", "exact"] {
        let filler = if mode == "controls" {
            "<input>".repeat(forms::MAX_CONTROLS - native_count - 1)
        } else {
            String::new()
        };
        let (mut doc, mut forms, mut page) = staged(&filler, "");
        page.state.rebuild(&doc).unwrap();
        let base = doc.nodes.len();
        let before = snapshot(&doc, &forms, &page);
        page.state.node_limit = base + 4 - usize::from(mode == "nodes");
        page.state.retained_limit = before.retained + retained - usize::from(mode == "retained");
        page.state.host_work = spent_work - usize::from(mode == "work");
        page.state.copy_work = spent_copies - usize::from(mode == "copies");
        if mode == "controls" {
            assert_eq!(forms.controls.len() + 1, forms::MAX_CONTROLS);
            page.state.host_work = usize::MAX;
            page.state.copy_work = usize::MAX;
            page.state.retained_limit = usize::MAX;
        }
        let result = clone_source(&mut doc, &mut forms, &mut page);
        if mode == "exact" {
            assert!(matches!(result,Ok(Value::Host(handle)) if handle==node_handle(base)));
            assert_eq!(
                (doc.nodes.len(), forms.controls.len()),
                (base + 4, native_count + 2)
            );
            assert_eq!(page.state.retained, page.state.retained_limit);
            assert_eq!((page.state.host_work, page.state.copy_work), (0, 0));
            let cloned_choice = doc.nodes[base].children[0];
            let c = &forms.controls[&cloned_choice];
            assert_eq!(
                (c.kind, c.group_root, c.owner, c.checked, c.dirty_checked),
                (Kind::Radio, base, None, false, true)
            );
            assert!(doc.attr(cloned_choice, "checked").is_some());
        } else {
            let error = result.err().unwrap_or_else(||panic!("{mode}: over-limit clone succeeded"));
            let boundary = match mode {
                "nodes" => "node limit",
                "controls" => "control limit",
                "retained" => "retained",
                "work" => "DOM work limit",
                "copies" => "string work limit",
                _ => unreachable!(),
            };
            assert!(error.contains(boundary), "{mode}: {error}");
            assert_eq!(snapshot(&doc, &forms, &page), before, "{mode}");
        }
        kept(&doc, &forms);
    }
}
