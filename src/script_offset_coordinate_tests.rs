//! Original CSSOM offset coordinates from real border and padding edges.
//! https://drafts.csswg.org/cssom-view/#dom-htmlelement-offsetleft
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}

fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(concat!(
        "<!doctype html><html id=root><head><title>Original offset coordinates</title>",
        "<link id=sheet rel=stylesheet href=theme.css></head><body id=body>",
        "<div id=outer class=positioned><div id=middle><button id=toggle type=button>Original toggle</button>",
        "<ul id=menu class=menu title='retained raw title'><li id=item>Original choice</li></ul></div></div>",
        "<div id=other class=other></div><div id=fixed class=fixed>Fixed</div>",
        "<form id=form><input id=neighbor value=original></form><div id=filler></div>",
        "</body></html>"
    ));
    doc.scripting_enabled = true;
    let sheets = HashMap::from([(
        id(&doc, "sheet"),
        concat!(
            "body{margin:0}.positioned{position:relative;left:20px;top:10px;width:200px;height:120px;",
            "padding:7px 13px 9px 11px;border-left:3px solid black;border-top:5px solid black;",
            "border-right:4px solid black;border-bottom:6px solid black}",
            ".menu{position:absolute;left:30px;top:40px;width:80px;height:36px;margin:0;padding:0;border:2px solid black}",
            ".menu.negative{left:-11px;top:-13px}.gone{display:none}",
            ".other{position:absolute;left:300px;top:50px;width:150px;height:100px;padding:4px;",
            "border-left:9px solid black;border-top:7px solid black}",
            ".fixed{position:fixed;left:17px;top:19px;width:30px;height:20px}#filler{height:1800px}"
        )
        .to_owned(),
    )]);
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let location = crate::navigation::address(
        "https://offset-coordinate.test/original",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&location);
    page.set_resource_base(&location);
    page.set_stylesheet_usage(crate::css::stylesheet_usage(&doc, &sheets));
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
    let control = &forms.controls[&id(doc, "neighbor")];
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
    assert_eq!(
        forms.text_default_value(id(doc, "neighbor")).unwrap(),
        "original"
    );
    assert_eq!(doc.attr(id(doc, "neighbor"), "value"), Some("original"));
    assert_eq!(
        doc.attr(id(doc, "menu"), "title"),
        Some("retained raw title")
    );
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
fn offset_coordinates_original_menu_border_edge_relative_to_real_parent_padding_edge() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var outer = document.getElementById('outer'), middle = document.getElementById('middle'),
    menu = document.getElementById('menu'), item = document.getElementById('item'),
    toggle = document.getElementById('toggle'), other = document.getElementById('other'),
    fixed = document.getElementById('fixed'), body = document.body, root = document.documentElement,
    parentBox = outer.getBoundingClientRect(), menuBox = menu.getBoundingClientRect();
if (outer.nodeName !== 'DIV' || menu.nodeName !== 'UL' || toggle.nodeName !== 'BUTTON'
    || menu.parentNode !== middle || middle.parentNode !== outer || outer.parentNode !== body
    || item.parentNode !== menu || menu.firstChild !== item || toggle.parentNode !== middle
    || menu.ownerDocument !== document || menu.offsetParent !== outer || outer.offsetParent !== body
    || root !== document.getElementById('root') || document.getElementById('neighbor').form !== document.getElementById('form'))
    throw 'actual parsed menu/native/containing-parent identity prefix';
if (parentBox.x !== 20 || parentBox.y !== 10 || parentBox.width !== 231 || parentBox.height !== 147
    || outer.clientLeft !== 3 || outer.clientTop !== 5 || outer.clientWidth !== 224 || outer.clientHeight !== 136
    || menuBox.x !== 53 || menuBox.y !== 55 || menuBox.width !== 84 || menuBox.height !== 40
    || menu.clientLeft !== 2 || menu.clientTop !== 2 || menu.clientWidth !== 80 || menu.clientHeight !== 36
    || toggle.clientWidth <= 0 || menu.getAttribute('class') !== 'menu'
    || menu.getAttribute('title') !== 'retained raw title' || document.getElementById('neighbor').value !== 'edited neighbor')
    throw 'actual asymmetric border/padding, global rectangle and native editor prefix';

var first = menu.offsetLeft;
if (first !== 30 || menu.offsetTop !== 40)
    throw 'menu border edge relative to parent padding edge, not global or content coordinates';
menu.className = 'menu negative';
if (menu.getBoundingClientRect().x !== 12 || menu.getBoundingClientRect().y !== 2
    || menu.offsetLeft !== -11 || menu.offsetTop !== -13 || menu.offsetParent !== outer)
    throw 'negative offsets retain their sign and real ancestor';
menu.className = 'menu';
other.appendChild(menu);
if (menu.parentNode !== other || menu.offsetParent !== other || menu.firstChild !== item
    || menu.getBoundingClientRect().x !== 339 || menu.getBoundingClientRect().y !== 97
    || menu.offsetLeft !== 30 || menu.offsetTop !== 40 || menu.ownerDocument !== document)
    throw 'live reparent uses the new padding edge and keeps genuine node identity';
middle.appendChild(menu);
if (menu.offsetParent !== outer || menu.offsetLeft !== 30 || menu.offsetTop !== 40)
    throw 'original containing parent restored';
if (fixed.offsetParent !== null || fixed.getBoundingClientRect().x !== 17 || fixed.getBoundingClientRect().y !== 19
    || fixed.offsetLeft !== 17 || fixed.offsetTop !== 19)
    throw 'null-parent fixed coordinates use the initial containing block';
menu.className = 'menu gone';
if (menu.offsetParent !== null || menu.offsetLeft !== 0 || menu.offsetTop !== 0)
    throw 'display none removes the associated box';
menu.className = 'menu';
middle.removeChild(menu);
if (menu.parentNode !== null || menu.offsetParent !== null || menu.offsetLeft !== 0 || menu.offsetTop !== 0)
    throw 'detached authored offsets do not fabricate a layout box';
middle.appendChild(menu);
if (menu.firstChild !== item || menu.getAttribute('class') !== 'menu'
    || menu.getAttribute('title') !== 'retained raw title' || menu.offsetLeft !== 30 || menu.offsetTop !== 40
    || body.offsetLeft !== 0 || body.offsetTop !== 0 || root.offsetLeft !== 0 || root.offsetTop !== 0)
    throw 'raw attributes, original identity and rendered body/root coordinates';
"#,
    );
    page.set_geometry_scroll(240);
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
if (root.scrollTop !== 240 || menu.getBoundingClientRect().y !== -185
    || menu.offsetLeft !== 30 || menu.offsetTop !== 40 || menu.offsetParent !== outer
    || fixed.offsetLeft !== 17 || fixed.offsetTop !== 19 || fixed.getBoundingClientRect().y !== 19)
    throw 'viewport scrolling changes client rectangles but not document-relative offset arithmetic';
"#,
    );
}

#[test]
fn offset_coordinates_protocol_readonly_html_getters_and_real_child_owner_boundaries() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var P = HTMLElement.prototype, menu = document.getElementById('menu'),
    text = document.getElementById('item').firstChild, comment = document.createComment('kept'),
    effects = 0, marker = {valueOf: function () { effects++; return 1; }, toString: function () { effects++; return '1'; }};
for (var key of ['offsetLeft', 'offsetTop']) {
    var D = Object.getOwnPropertyDescriptor(P, key), expected = key === 'offsetLeft' ? 30 : 40;
    if (!D || typeof D.get !== 'function' || D.get.name !== 'get ' + key || D.get.length !== 0
        || D.set !== undefined || D.value !== undefined || !D.enumerable || !D.configurable
        || Object.getOwnPropertyDescriptor(Element.prototype, key) !== undefined
        || Object.getOwnPropertyDescriptor(Node.prototype, key) !== undefined)
        throw 'canonical readonly HTMLElement offset descriptor';
    if (!(key in menu) || Object.hasOwn(menu, key) || D.get.call(menu) !== expected)
        throw 'ordinary inherited getter returns actual coordinate';
    var rejected = 0;
    for (var receiver of [null, undefined, {}, P, Element.prototype, document, text, comment]) {
        try { D.get.call(receiver); }
        catch (e) { if (e instanceof TypeError) rejected++; else throw e; }
    }
    if (rejected !== 8) throw 'non-HTMLElement borrowed receiver';
    menu[key] = marker;
    if (menu[key] !== expected || effects !== 0 || Object.hasOwn(menu, key))
        throw 'sloppy readonly assignment retains native coordinate without conversion';
    try { (function () { 'use strict'; menu[key] = marker; })(); throw 'strict readonly assignment accepted'; }
    catch (e) { if (!(e instanceof TypeError)) throw e; }
    var calls = 0;
    Object.defineProperty(P, key, {configurable: true, get: function () {
        if (this !== menu) throw 'author getter receiver';
        calls++; return marker;
    }});
    if (menu[key] !== marker || calls !== 1 || D.get.call(menu) !== expected)
        throw 'ordinary prototype replacement versus saved native getter';
    delete P[key];
    if (key in menu || menu[key] !== undefined) throw 'deleted getter has no native fallback';
    Object.defineProperty(P, key, D);
    Object.defineProperty(menu, key, {configurable: true, value: marker});
    if (menu[key] !== marker || D.get.call(menu) !== expected) throw 'own shadow versus native getter';
    delete menu[key];
    if (menu[key] !== expected || Object.hasOwn(menu, key)) throw 'canonical restoration';
}
var mainX = Object.getOwnPropertyDescriptor(P, 'offsetLeft').get,
    mainY = Object.getOwnPropertyDescriptor(P, 'offsetTop').get,
    frame = document.createElement('iframe');
frame.hidden = true;
document.body.appendChild(frame);
var child = frame.contentWindow, childDoc = frame.contentDocument, childBody = childDoc.body,
    childRoot = childDoc.documentElement, F = child.Function,
    childX = F('return Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetLeft").get;')(),
    childY = F('return Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetTop").get;')(),
    childRead = F('node', 'return node.offsetLeft + node.offsetTop;'),
    childOwn = F('return window === self && document.body.offsetLeft === 0 && document.body.offsetTop === 0 && document.body.ownerDocument === document;');
if (childBody.nodeName !== 'BODY' || childRoot.nodeName !== 'HTML' || childBody.parentNode !== childRoot
    || childBody.ownerDocument !== childDoc || childRoot === document.documentElement)
    throw 'actual blank child body and distinct root identities';
if (childX === mainX || childY === mainY || Object.getPrototypeOf(childX) !== F.prototype
    || Object.getPrototypeOf(childY) !== F.prototype || childX.call(menu) !== 30 || childY.call(menu) !== 40
    || childRead(menu) !== 70 || !childOwn())
    throw 'per-realm getter identity and supported same-origin borrowing';
if (mainX.call(childBody) !== 0 || mainY.call(childBody) !== 0
    || childX.call(childBody) !== 0 || childY.call(childBody) !== 0)
    throw 'actual body has unconditional zero coordinates';
var refused = 0;
for (var getter of [mainX, mainY, childX, childY]) {
    for (var node of [childRoot, childDoc.head]) {
        try { getter.call(node); }
        catch (e) { if (e.name !== 'NotSupportedError') throw e; refused++; }
    }
}
try { childRoot.offsetLeft; }
catch (e) { if (e.name !== 'NotSupportedError') throw e; refused++; }
try { childDoc.head.offsetTop; }
catch (e) { if (e.name !== 'NotSupportedError') throw e; refused++; }
if (refused !== 10 || effects !== 0 || !childOwn() || menu.offsetLeft !== 30 || menu.offsetTop !== 40)
    throw 'child root is not body and missing child renderer never fabricates coordinates';
"#,
    );
    let commits = page.state.dom_commits;
    let nodes = format!("{:?}", doc.nodes);
    let base = crate::navigation::Location::home();
    page.set_location(
        &crate::navigation::address("https://different-offset-coordinate.test/changed", &base)
            .unwrap(),
    );
    for source in [
        "childBody.offsetLeft;",
        "childBody.offsetTop;",
        "mainX.call(childBody);",
        "mainY.call(childBody);",
        "childX.call(menu);",
        "childY.call(menu);",
        "childRead(menu);",
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
        run(
            &mut doc,
            &mut forms,
            &mut page,
            "if (!childOwn() || window !== self || menu.offsetLeft !== 30 || menu.offsetTop !== 40 || effects !== 0) throw 'origin refusal and owner restoration';",
        );
    }
}

#[test]
fn offset_coordinates_body_detached_and_unmeasured_main_renderer_boundaries() {
    let mut doc = crate::html::parse(
        "<!doctype html><html><head></head><body><div id=connected>Actual connected box</div></body></html>",
    );
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    let outcome = page.console_eval(
        &mut doc,
        &mut forms,
        r#"
var detached = document.createElement('div');
if (document.body.offsetLeft !== 0 || document.body.offsetTop !== 0
    || detached.offsetLeft !== 0 || detached.offsetTop !== 0 || detached.parentNode !== null)
    throw 'actual body and detached boxes need no fabricated renderer';
"#,
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let commits = page.state.dom_commits;
    for source in [
        "document.documentElement.offsetLeft;",
        "document.documentElement.offsetTop;",
        "document.getElementById('connected').offsetLeft;",
        "document.getElementById('connected').offsetTop;",
    ] {
        page.clear_console();
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
        assert!(
            page.report().contains("requires measured offset geometry")
                && page.report().contains("geometry-attached=false")
                && page.report().contains("No coordinate was fabricated"),
            "{source}\n{}",
            page.report()
        );
        assert_eq!(page.state.dom_commits, commits);
    }
}

#[test]
fn offset_coordinates_named_form_reads_keep_ancestral_setter_assignment() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var form = document.getElementById('form'), field = document.getElementById('neighbor'),
    P = HTMLElement.prototype, originalName = field.getAttribute('name'), conversions = 0,
    marker = {valueOf: function () { conversions++; return 19; }, toString: function () { conversions++; return '19'; }};
if (originalName !== null || field.form !== form || field.value !== 'edited neighbor')
    throw 'actual native named-control prefix';
for (var key of ['offsetLeft', 'offsetParent']) {
    field.name = key;
    var D = Object.getOwnPropertyDescriptor(P, key), expected = key === 'offsetParent' ? document.body : 0,
        calls = 0, gets = 0, receiver, assigned;
    if (form[key] !== field || !(key in form) || D.get.call(form) !== expected)
        throw 'real named read masks the canonical getter but does not change form layout';
    Object.defineProperty(P, key, {configurable: true, get: function () {
        gets++; return marker;
    }, set: function (value) {
        calls++; receiver = this; assigned = value;
    }});
    form[key] = marker;
    if (calls !== 1 || receiver !== form || assigned !== marker || conversions !== 0
        || form[key] !== field || gets !== 0 || D.get.call(form) !== expected)
        throw 'named read precedence and ordinary ancestral setter preserve raw receiver/value';
    delete P[key];
    if (form[key] !== field || !(key in form)) throw 'prototype deletion keeps the actual named control';
    Object.defineProperty(P, key, D);
    if (form[key] !== field || D.get.call(form) !== expected) throw 'canonical getter restored';
}
field.removeAttribute('name');
if (field.getAttribute('name') !== originalName || field.form !== form || field.value !== 'edited neighbor'
    || conversions !== 0 || form.offsetLeft !== 0 || form.offsetParent !== document.body)
    throw 'original raw name, native state and unmasked form geometry restored';
"#,
    );
}
