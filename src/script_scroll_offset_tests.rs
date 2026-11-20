//! Original element scroll-offset readback over real positioned light DOM.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}

fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(concat!(
        "<!doctype html><html id=root><head><title>Original scroll offsets</title>",
        "<link id=sheet rel=stylesheet href=theme.css></head><body id=body>",
        "<div id=outer class=positioned><button id=toggle type=button>Original toggle</button>",
        "<ul id=menu><li>Original choice</li></ul></div>",
        "<div id=visible class=visible><div class=extent>Visible overflow</div></div>",
        "<div id=clipped class=clipped><div class=extent>Clipped overflow</div></div>",
        "<div id=scroller class=unknown><div class=extent>Unimplemented element scrolling</div></div>",
        "<div id=no-box class=no-box>Hidden</div>",
        "<form id=form><input id=neighbor value=original></form><div id=filler></div>",
        "</body></html>"
    ));
    doc.scripting_enabled = true;
    let sheets = HashMap::from([(
        id(&doc, "sheet"),
        concat!(
            "body{margin:0}.positioned{position:relative;left:20px;top:10px;width:200px;height:120px}",
            "#menu{position:absolute;left:30px;top:40px;width:80px;height:36px;margin:0;padding:0}",
            "#visible,#clipped,#scroller{width:80px;height:20px}",
            ".visible{overflow:visible}.clipped{overflow:clip}.unknown{overflow:hidden}",
            ".extent{width:160px;height:70px}.no-box{display:none}#filler{height:1800px}"
        )
        .to_owned(),
    )]);
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let location = crate::navigation::address(
        "https://scroll-offset.test/original",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&location);
    page.set_resource_base(&location);
    page.set_stylesheet_sources(&doc, &sheets);
    page.set_geometry(crate::script_geometry::Geometry::new(
        800,
        600,
        1.0,
        &sheets,
        HashMap::new(),
        &crate::fonts::Book::empty(),
    ));
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );
    let neighbor = id(&doc, "neighbor");
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    let editor = &mut forms.controls.get_mut(&neighbor).unwrap().value;
    editor.cursor = 5;
    editor.anchor = 2;
    editor.offset = 1;
    (doc, forms, page)
}

fn kept(doc: &Document, forms: &Forms, page: &mut Page) {
    let node = id(doc, "neighbor");
    let control = &forms.controls[&node];
    assert_eq!(
        (
            control.value.text(),
            control.value.cursor,
            control.value.anchor,
            control.value.offset,
            control.dirty_value,
            control.owner,
        ),
        (
            "edited neighbor".into(),
            5,
            2,
            1,
            true,
            Some(id(doc, "form"))
        )
    );
    assert_eq!(forms.text_default_value(node).unwrap(), "original");
    assert_eq!(doc.attr(node, "value"), Some("original"));
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (added, removed, changed) = page.take_image_changes();
    assert!(added.is_empty() && removed.is_empty() && changed.is_empty());
}

fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let outcome = page.console_eval(doc, forms, source);
    assert_eq!(outcome.errors, 0, "{source}\n{}", page.report());
    kept(doc, forms, page);
}

#[test]
fn scroll_offsets_original_positioned_menu_ancestor_then_live_root_and_unknown_scope() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var outer = document.getElementById('outer'), menu = document.getElementById('menu'),
    toggle = document.getElementById('toggle'), visible = document.getElementById('visible'),
    clipped = document.getElementById('clipped'), root = document.documentElement,
    body = document.body, box = outer.getBoundingClientRect(), menuBox = menu.getBoundingClientRect();
if (document.compatMode !== 'CSS1Compat' || root.nodeName !== 'HTML' || body.nodeName !== 'BODY'
    || outer.nodeName !== 'DIV' || menu.nodeName !== 'UL' || toggle.nodeName !== 'BUTTON'
    || menu.parentNode !== outer || toggle.parentNode !== outer || outer.parentNode !== body
    || body.parentNode !== root || outer.ownerDocument !== document || menu.ownerDocument !== document)
    throw 'real parsed positioned DIV and UL physical ownership';
if (box.x !== 20 || box.y !== 10 || box.width !== 200 || box.height !== 120
    || menuBox.x !== 50 || menuBox.y !== 50 || menuBox.width !== 80 || menuBox.height !== 36
    || menu.clientWidth !== 80 || menu.clientHeight !== 36 || toggle.clientWidth <= 0
    || document.getElementById('neighbor').value !== 'edited neighbor')
    throw 'actual rectangle and native editor positive prefix';

var first = outer.scrollLeft;
if (first !== 0 || outer.scrollTop !== 0) throw 'positioned ordinary ancestor starts at zero';
if (menu.scrollLeft !== 0 || menu.scrollTop !== 0 || visible.scrollLeft !== 0 || visible.scrollTop !== 0
    || clipped.scrollLeft !== 0 || clipped.scrollTop !== 0)
    throw 'ordinary visible and clip overflow do not become scrolling elements';
if (root.scrollLeft !== 0 || root.scrollTop !== 0 || body.scrollLeft !== 0 || body.scrollTop !== 0)
    throw 'standards root and body initial offsets';
var detached = document.createElement('div'), noBox = document.getElementById('no-box');
if (detached.ownerDocument !== document || detached.parentNode !== null
    || detached.scrollLeft !== 0 || detached.scrollTop !== 0
    || noBox.scrollLeft !== 0 || noBox.scrollTop !== 0)
    throw 'detached and no-box elements have zero offsets';
"#,
    );

    // This is actual viewport state supplied by the App, not an author expando.
    page.set_geometry_scroll(240);
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
if (root.scrollTop !== 240 || root.scrollLeft !== 0 || body.scrollTop !== 0 || body.scrollLeft !== 0
    || outer.scrollTop !== 0 || outer.scrollLeft !== 0 || menu.scrollTop !== 0 || menu.scrollLeft !== 0
    || outer.getBoundingClientRect().y !== -230)
    throw 'current viewport offset belongs to standards root, not nested ordinary elements';
"#,
    );
    page.set_geometry_scroll(36);
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "if (root.scrollTop !== 36 || outer.scrollTop !== 0 || outer.getBoundingClientRect().y !== -26) throw 'live viewport state';",
    );

    // No per-element scrolling machinery is claimed by this finite slice.
    let commits = page.state.dom_commits;
    for source in [
        "document.getElementById('scroller').scrollLeft;",
        "document.getElementById('scroller').scrollTop;",
    ] {
        page.clear_console();
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
        assert!(page.report().to_ascii_lowercase().contains("scroll"));
        assert_eq!(page.state.dom_commits, commits);
        kept(&doc, &forms, &mut page);
    }
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "if (root.scrollTop !== 36 || outer.scrollLeft !== 0 || outer.scrollTop !== 0 || menu.parentNode !== outer) throw 'refusal preserves measured state';",
    );
}

#[test]
fn scroll_offsets_protocol_element_getters_setters_brands_conversion_and_ordinary_lookup() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var P = Element.prototype, outer = document.getElementById('outer'),
    menu = document.getElementById('menu'), text = menu.firstChild.firstChild,
    comment = document.createComment('kept'), conversions = 0,
    value = {valueOf: function () { conversions++; return 17; }};
for (var key of ['scrollLeft', 'scrollTop']) {
    var D = Object.getOwnPropertyDescriptor(P, key);
    if (!D || typeof D.get !== 'function' || D.get.name !== 'get ' + key || D.get.length !== 0
        || typeof D.set !== 'function' || D.set.name !== 'set ' + key || D.set.length !== 1
        || !D.enumerable || !D.configurable || D.value !== undefined || D.writable !== undefined
        || Object.getOwnPropertyDescriptor(Node.prototype, key) !== undefined
        || Object.getOwnPropertyDescriptor(HTMLElement.prototype, key) !== undefined)
        throw 'canonical Element scroll accessor descriptor';
    if (!(key in outer) || Object.hasOwn(outer, key) || D.get.call(outer) !== 0)
        throw 'inherited genuine Element getter';
    var rejected = 0;
    for (var receiver of [null, undefined, {}, P, document, text, comment]) {
        try { D.get.call(receiver); }
        catch (e) { if (e instanceof TypeError) rejected++; else throw e; }
        try { D.set.call(receiver, value); }
        catch (e) { if (e instanceof TypeError) rejected++; else throw e; }
    }
    if (rejected !== 14 || conversions !== 0) throw 'brand precedes setter value conversion';
    if (D.set.call(outer, value) !== undefined || D.get.call(outer) !== 0
        || conversions !== 1 || Object.hasOwn(outer, key))
        throw 'converted no-op on real non-scrollable element';
    conversions = 0;
    var marker = {}, same = false;
    try { D.set.call(outer, {valueOf: function () { throw marker; }}); }
    catch (e) { same = e === marker; }
    if (!same || D.get.call(outer) !== 0) throw 'abrupt numeric conversion keeps native state';
    var symbolRejected = false;
    try { D.set.call(outer, Symbol()); }
    catch (e) { symbolRejected = e instanceof TypeError; }
    if (!symbolRejected) throw 'Symbol is not a numeric scroll offset';
    var detached = document.createElement('div');
    D.set.call(detached, 23);
    if (D.get.call(detached) !== 0 || detached.parentNode !== null)
        throw 'detached setter has no scrolling box';

    var gets = 0, sets = 0, assigned;
    Object.defineProperty(P, key, {configurable: true, get: function () {
        gets++; return marker;
    }, set: function (v) {
        if (this !== outer) throw 'setter receiver';
        sets++; assigned = v;
    }});
    outer[key] = value;
    if (outer[key] !== marker || gets !== 1 || sets !== 1 || assigned !== value
        || conversions !== 0 || D.get.call(outer) !== 0 || Object.hasOwn(outer, key))
        throw 'ordinary author accessors retain receiver and unconverted value';
    delete P[key];
    if (key in outer || outer[key] !== undefined) throw 'deleted accessor has no native ghost';
    Object.defineProperty(P, key, D);
    Object.defineProperty(outer, key, {configurable: true, writable: true, value: marker});
    if (outer[key] !== marker || D.get.call(outer) !== 0) throw 'ordinary own shadow';
    delete outer[key];
    if (outer[key] !== 0 || Object.hasOwn(outer, key)) throw 'canonical getter restored';
}
if (menu.parentNode !== outer || document.getElementById('neighbor').value !== 'edited neighbor')
    throw 'protocol leaves actual DOM and native neighbor intact';
"#,
    );
}

#[test]
fn scroll_offsets_named_form_reads_and_borrowed_child_owner_guards() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var form = document.getElementById('form'), field = document.getElementById('neighbor'),
    outer = document.getElementById('outer'), P = Element.prototype,
    originalName = field.getAttribute('name'), conversions = 0,
    marker = {valueOf: function () { conversions++; return 9; }};
if (field.form !== form || field.value !== 'edited neighbor' || originalName !== null)
    throw 'actual parsed form and edited native input prefix';
for (var key of ['scrollLeft', 'scrollTop']) {
    field.name = key;
    var D = Object.getOwnPropertyDescriptor(P, key), calls = 0, reads = 0, assigned;
    if (form[key] !== field || !(key in form) || D.get.call(form) !== 0)
        throw 'named read and canonical real-form getter';
    Object.defineProperty(P, key, {configurable: true, get: function () {
        reads++; return marker;
    }, set: function (value) {
        if (this !== form) throw 'named-form ancestral setter receiver';
        calls++; assigned = value;
    }});
    form[key] = marker;
    if (calls !== 1 || assigned !== marker || conversions !== 0
        || form[key] !== field || reads !== 0 || D.get.call(form) !== 0)
        throw 'named reads override the prototype while assignment uses its setter';
    D.set.call(form, marker);
    if (calls !== 1 || conversions !== 1 || D.get.call(form) !== 0 || form[key] !== field)
        throw 'canonical borrowed setter bypasses the named read and author setter';
    conversions = 0;
    Object.defineProperty(P, key, D);
    if (form[key] !== field) throw 'canonical restoration keeps named read';
    delete P[key];
    if (form[key] !== field || !(key in form)) throw 'prototype deletion keeps actual named control';
    Object.defineProperty(P, key, D);
}
field.removeAttribute('name');
if (field.getAttribute('name') !== originalName || field.form !== form || field.value !== 'edited neighbor')
    throw 'raw name and native input restored';

var frame = document.createElement('iframe');
frame.hidden = true;
document.body.appendChild(frame);
var child = frame.contentWindow, childBody = frame.contentDocument.body, F = child.Function,
    mainD = Object.getOwnPropertyDescriptor(P, 'scrollTop'),
    childD = F('return Object.getOwnPropertyDescriptor(Element.prototype, "scrollTop");')(),
    childRead = F('node', 'return node.scrollTop;'),
    childWrite = F('node', 'value', 'node.scrollTop = value; return node.scrollTop;'),
    childOwn = F('return window === self && document.body.ownerDocument === document && document.body.firstChild === null;');
if (mainD.get === childD.get || mainD.set === childD.set
    || Object.getPrototypeOf(childD.get) !== F.prototype || Object.getPrototypeOf(childD.set) !== F.prototype
    || childD.get.call(outer) !== 0 || childRead(outer) !== 0 || !childOwn())
    throw 'genuine per-realm accessors and same-origin borrowing';
if (childD.set.call(outer, marker) !== undefined || conversions !== 1
    || childWrite(outer, marker) !== 0 || conversions !== 2)
    throw 'child-owned setter operates on real main non-scrollable element';
conversions = 0;
var refused = 0;
for (var getter of [mainD.get, childD.get]) {
    try { getter.call(childBody); }
    catch (e) { if (e.name !== 'NotSupportedError') throw e; refused++; }
}
for (var setter of [mainD.set, childD.set]) {
    try { setter.call(childBody, marker); }
    catch (e) { if (e.name !== 'NotSupportedError') throw e; refused++; }
}
try { childBody.scrollTop = marker; }
catch (e) { if (e.name !== 'NotSupportedError') throw e; refused++; }
if (refused !== 5 || conversions !== 0 || !childOwn() || mainD.get.call(outer) !== 0)
    throw 'same-origin child geometry boundary precedes conversion and restores owner';
"#,
    );

    let commits = page.state.dom_commits;
    let nodes = format!("{:?}", doc.nodes);
    let frames = page.state.blank_frames.len();
    let base = crate::navigation::Location::home();
    page.set_location(
        &crate::navigation::address("https://different-scroll-offset.test/changed", &base).unwrap(),
    );
    for source in [
        "childBody.scrollTop;",
        "childBody.scrollTop = marker;",
        "mainD.get.call(childBody);",
        "mainD.set.call(childBody, marker);",
        "childD.get.call(outer);",
        "childD.set.call(outer, marker);",
        "childRead(outer);",
        "childWrite(outer, marker);",
    ] {
        page.clear_console();
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
        assert!(
            page.report().contains("same-origin"),
            "{source}\n{}",
            page.report()
        );
        assert_eq!(page.state.dom_commits, commits);
        assert_eq!(format!("{:?}", doc.nodes), nodes);
        assert_eq!(page.state.blank_frames.len(), frames);
        kept(&doc, &forms, &mut page);
        run(
            &mut doc,
            &mut forms,
            &mut page,
            "if (conversions !== 0 || !childOwn() || window !== self || mainD.get.call(outer) !== 0) throw 'origin refusal preserves callback and owner state';",
        );
    }
    page.set_location(
        &crate::navigation::address("https://scroll-offset.test/original", &base).unwrap(),
    );
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "if (childRead(outer) !== 0 || childD.get.call(outer) !== 0 || !childOwn() || conversions !== 0) throw 'same-origin access restored';",
    );
    assert_eq!(page.state.dom_commits, commits);
}
