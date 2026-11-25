//! Original controls for cloning ordinary nodes in their owning document.
use super::*;

fn fixture(markup: &str) -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(markup);
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let location = crate::navigation::address(
        "https://clone.test/original", &crate::navigation::Location::home(),
    ).unwrap();
    page.set_location(&location);
    page.set_resource_base(&location);
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0, "{}", page.report());
    (doc, forms, page)
}

fn eval(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    assert_eq!(page.console_eval(doc, forms, source).errors, 0, "{source}\n{}", page.report());
}

#[derive(Debug, PartialEq, Eq)]
struct ArenaSnapshot {
    nodes: usize,
    created: usize,
    detached: usize,
    parent_slots: usize,
    active_slots: usize,
    connected_slots: usize,
    attribute_handles: usize,
    retained_bytes: usize,
    dom_commits: Option<u64>,
}

fn arena(doc: &Document, page: &Page) -> ArenaSnapshot {
    let s = &page.state;
    ArenaSnapshot {
        nodes: doc.nodes.len(),
        created: s.dom.created.len(),
        detached: s.detached_nodes.len(),
        parent_slots: s.tree.parents.len(),
        active_slots: s.tree.active.len(),
        connected_slots: s.dom.connected.len(),
        attribute_handles: s.attributes.len(),
        retained_bytes: s.retained,
        dom_commits: s.dom_commits,
    }
}

#[test]
fn clone_node_detached_nav_method_brand_and_boolean_argument() {
    let (mut doc, mut forms, mut page) = fixture("<html><body></body></html>");
    eval(&mut doc, &mut forms, &mut page, r#"
        var nav = document.createElement('nav'), copy = nav.cloneNode(true);
        var method = Node.prototype.cloneNode, d = Object.getOwnPropertyDescriptor(Node.prototype, 'cloneNode');
        if (method !== nav.cloneNode || method !== d.value || method.length !== 0 ||
            method.name !== 'cloneNode' || !d.writable || !d.enumerable || !d.configurable)
            throw 'Node clone method descriptor';
        if (copy === nav || copy.nodeName !== 'NAV' || copy.localName !== 'nav' ||
            copy.ownerDocument !== document || copy.parentNode !== null || copy.isConnected ||
            copy.getRootNode() !== copy || copy.childNodes.length || !(copy instanceof HTMLElement))
            throw 'empty detached HTML nav';
        nav.appendChild(document.createTextNode('part'));
        var hits = 0, truth = {valueOf:function(){hits++;throw 'coercion';},
            toString:function(){hits++;throw 'coercion';}};
        if (method.call(nav).firstChild !== null || method.call(nav, 0).firstChild !== null ||
            method.call(nav, truth, truth).textContent !== 'part' || hits)
            throw 'optional boolean deep';
        var rejected = false;
        try { method.call({nodeType:1}, truth); } catch(e) { rejected = e instanceof TypeError; }
        if (!rejected || hits || nav.parentNode !== null) throw 'receiver brand';
    "#);
}

#[test]
fn clone_node_deep_tree_copies_ordered_attributes_and_children_with_independent_identities() {
    let (mut doc, mut forms, mut page) = fixture(concat!(
        "<html><body><section id=source title=seed>prefix<span>tail</span></section>",
        "<div id=destination></div></body></html>",
    ));
    eval(&mut doc, &mut forms, &mut page, r#"
        var source = document.getElementById('source'), list = source.childNodes;
        var middle = document.createComment('middle');
        source.insertBefore(middle, source.lastChild);
        if (list.length !== 3 || list[1] !== middle || middle.parentNode !== source)
            throw 'source mixed-node fixture';
        source.setAttributeNS('urn:clone-test', 'm:mark', 'meta');
        var originalAttr = source.getAttributeNodeNS('urn:clone-test','mark');
        source.copyMarker = 8;
        var calls = 0, ev = document.createEvent('Event');
        ev.initEvent('clone-check',false,false);
        source.addEventListener('clone-check',function(){calls++;});
        var shallow = source.cloneNode(), copy = source.cloneNode(true), children = copy.childNodes;
        if (shallow === copy || shallow.firstChild !== null || shallow.title !== 'seed' ||
            shallow.getAttributeNS('urn:clone-test','mark') !== 'meta') throw 'shallow attributes';
        if (copy.parentNode !== null || copy.getRootNode() !== copy || copy.isConnected ||
            copy.ownerDocument !== document || copy.id !== 'source' || source.parentNode !== document.body ||
            !source.isConnected || document.getElementById('source') !== source || copy.copyMarker !== undefined)
            throw 'source and clone topology';
        var clonedAttr = copy.getAttributeNodeNS('urn:clone-test','mark');
        if (copy.attributes.length !== 3 || copy.attributes.item(0).name !== 'id' ||
            copy.attributes.item(1).name !== 'title' || copy.attributes.item(2) !== clonedAttr ||
            clonedAttr === originalAttr || clonedAttr.ownerElement !== copy || clonedAttr.name !== 'm:mark' ||
            clonedAttr.prefix !== 'm' || clonedAttr.localName !== 'mark' || clonedAttr.value !== 'meta' ||
            originalAttr.ownerElement !== source) throw 'ordered independent Attr handles';
        if (children === list || children.length !== 3 || copy.firstChild === source.firstChild ||
            children[0].data !== 'prefix' || children[1].nodeType !== 8 || children[1].data !== 'middle' ||
            children[1].previousSibling !== children[0] || children[1].nextSibling !== children[2] ||
            children[2].parentNode !== copy || children[2].firstChild === list[2].firstChild ||
            children[2].firstChild.getRootNode() !== copy || children[2].firstChild.data !== 'tail')
            throw 'mixed deep children';
        copy.dispatchEvent(ev);
        if (calls !== 0) throw 'a registered listener was copied';
        source.dispatchEvent(ev);
        if (calls !== 1) throw 'original listener lost';
        clonedAttr.value = 'clone only'; children[0].data = 'changed'; source.title = 'source only';
        if (originalAttr.value !== 'meta' || source.firstChild.data !== 'prefix' ||
            copy.title !== 'seed' || copy.firstChild.data !== 'changed') throw 'independent changes';
        var dest = document.getElementById('destination'); dest.appendChild(copy);
        if (copy.parentNode !== dest || !copy.isConnected || !children[2].isConnected ||
            source.parentNode !== document.body || list.length !== 3) throw 'independent reattachment';
    "#);
}

#[test]
fn clone_node_fragment_text_and_comment_preserve_kind_owner_and_live_lists() {
    let (mut doc, mut forms, mut page) = fixture("<html><body></body></html>");
    eval(&mut doc, &mut forms, &mut page, r#"
        var frag = document.createDocumentFragment(), text = document.createTextNode('a雪'),
            comment = document.createComment('b'), list = frag.childNodes;
        frag.appendChild(text); frag.appendChild(comment);
        var minimal = frag.cloneNode(false), deep = frag.cloneNode(true);
        if (!(deep instanceof DocumentFragment) || deep === frag || deep.isConnected ||
            deep.parentNode !== null || deep.ownerDocument !== document || minimal.childNodes.length ||
            deep.childNodes.length !== 2 || deep.firstChild === text || deep.lastChild === comment ||
            deep.firstChild.parentNode !== deep || deep.lastChild.previousSibling !== deep.firstChild ||
            deep.firstChild.ownerDocument !== document || deep.firstChild.nodeValue !== 'a雪' ||
            deep.lastChild.nodeValue !== 'b' || deep.lastChild.getRootNode() !== deep)
            throw 'cloned donor fragment';
        var textCopy = text.cloneNode(true), commentCopy = comment.cloneNode();
        if (textCopy.nodeType !== 3 || textCopy.nodeValue !== 'a雪' ||
            commentCopy.nodeType !== 8 || commentCopy.nodeValue !== 'b' ||
            textCopy.parentNode !== null || commentCopy.parentNode !== null) throw 'standalone node copies';
        document.body.appendChild(deep); commentCopy.data = 'outside';
        if (deep.childNodes.length || frag.childNodes !== list || list.length !== 2 ||
            frag.firstChild !== text || comment.data !== 'b' || document.body.lastChild === comment)
            throw 'only cloned fragment spliced';
    "#);
}

#[test]
fn clone_node_protected_deep_descendants_fail_atomically_while_shallow_clone_succeeds() {
    let (mut doc, mut forms, mut page) = fixture(concat!(
        "<html><body><form id=f><div id=controls><span>first</span>",
        "<input type=checkbox name=protected><input id=edit name=keep value=seed></div></form>",
        "<div id=resources><b>first</b><script type=application/json>{}</script></div></body></html>",
    ));
    eval(&mut doc, &mut forms, &mut page, r#"
        var controls = document.getElementById('controls'), resources = document.getElementById('resources'),
            edit = document.getElementById('edit'), form = document.getElementById('f');
        edit.value = 'keep edited';
        var controlCopy = controls.cloneNode(false), resourceCopy = resources.cloneNode();
        if (controlCopy.firstChild || resourceCopy.firstChild || controlCopy.id !== 'controls' ||
            resourceCopy.id !== 'resources' || controlCopy.parentNode !== null) throw 'shallow protected descendants';
    "#);
    let editor = (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("edit")).unwrap();
    let value = &mut forms.controls.get_mut(&editor).unwrap().value;
    value.cursor = 2;
    value.anchor = 6;
    value.offset = 1;
    for operation in ["controls.cloneNode(true);", "resources.cloneNode(true);"] {
        let before = arena(&doc, &page);
        let total_controls = forms.controls.len();
        assert_eq!(page.console_eval(&mut doc, &mut forms, operation).errors, 1, "{operation}\n{}", page.report());
        assert_eq!(arena(&doc, &page), before, "{operation}");
        assert_eq!(forms.controls.len(), total_controls);
        let control = &forms.controls[&editor];
        assert!(control.dirty_value);
        assert_eq!((control.value.cursor, control.value.anchor, control.value.offset), (2, 6, 1));
        assert!(page.take_dynamic_scripts().is_empty());
        assert!(page.take_dynamic_stylesheets().is_empty());
        assert!(!page.has_navigation_request());
    }
    eval(&mut doc, &mut forms, &mut page, r#"
        if (controls.lastChild !== edit || edit.form !== form || !edit.isConnected ||
            edit.value !== 'keep edited' || edit.defaultValue !== 'seed' ||
            resources.firstChild.nodeName !== 'B' || resources.lastChild.nodeName !== 'SCRIPT')
            throw 'protected sources or live native state changed';
    "#);
}

#[test]
fn clone_node_deep_allocation_limit_does_not_publish_a_partial_tree() {
    let (mut doc, mut forms, mut page) = fixture(
        "<html><body><div id=source><span>text</span></div></body></html>",
    );
    eval(&mut doc, &mut forms, &mut page, r#"
        var source = document.getElementById('source'), tail = document.createComment('tail');
        source.appendChild(tail);
        if (source.childNodes.length !== 2 || source.lastChild !== tail ||
            source.firstChild.firstChild.data !== 'text') throw 'four-node source fixture';
    "#);
    let limit = page.state.node_limit;
    let before = arena(&doc, &page);
    page.state.node_limit = doc.nodes.len() + 3; // The copy requires four new node identities.
    let failed = page.console_eval(&mut doc, &mut forms, "source.cloneNode(true);");
    assert_eq!(failed.errors, 1, "{}", page.report());
    assert_eq!(arena(&doc, &page), before);
    page.state.node_limit = limit;
    eval(&mut doc, &mut forms, &mut page, r#"
        var result = source.cloneNode(true);
        if (result.childNodes.length !== 2 || result.firstChild.firstChild.data !== 'text' ||
            result.lastChild.data !== 'tail' || source.firstChild === result.firstChild)
            throw 'retry did not produce the complete independent tree';
    "#);
    assert_eq!(doc.nodes.len(), before.nodes + 4);
}
