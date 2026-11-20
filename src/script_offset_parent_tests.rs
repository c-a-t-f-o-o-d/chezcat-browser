//! Original HTMLElement.offsetParent identity over actual rendered light DOM.
use super::*;

#[test]
fn offset_parent_root_body_and_detached_need_no_renderer() {
    let mut doc = crate::html::parse("<html><head></head><body><div id=c></div></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    let outcome = page.console_eval(
        &mut doc,
        &mut forms,
        r#"
if (document.documentElement.offsetParent !== null || document.body.offsetParent !== null ||
    document.createElement('span').offsetParent !== null) throw 'box-independent null';
"#,
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let outcome = page.console_eval(
        &mut doc,
        &mut forms,
        "document.getElementById('c').offsetParent;",
    );
    assert_eq!(outcome.errors, 1);
    assert!(page.report().contains("requires the document renderer"));
}

#[test]
fn offset_parent_saved_getter_revalidates_both_document_owners() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var frame = document.createElement('iframe');
frame.hidden = true;
document.body.appendChild(frame);
var F = frame.contentWindow.Function, childBody = frame.contentDocument.body;
var menu = document.getElementById('list'), outer = document.getElementById('outer');
var mainGetter = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetParent').get;
var childRead = F('node', 'return node.offsetParent;');
var childOwn = F('return document.body.offsetParent === null;');
if (childRead(menu) !== outer || mainGetter.call(childBody) !== null || !childOwn())
    throw 'same-origin offset parent';
"#,
    );
    let commits = page.state.dom_commits;
    let base = crate::navigation::Location::home();
    page.set_location(
        &crate::navigation::address("https://second-offset.test/page", &base).unwrap(),
    );
    for source in [
        "childRead(menu);",
        "mainGetter.call(childBody);",
        "childBody.offsetParent;",
    ] {
        page.clear_console();
        let out = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(out.errors, 1, "{source}\n{}", page.report());
        assert!(
            page.report().contains("same-origin"),
            "{source}\n{}",
            page.report()
        );
        assert_eq!(page.state.dom_commits, commits);
        run(
            &mut doc,
            &mut forms,
            &mut page,
            "if (!childOwn()) throw 'child owner lost';",
        );
    }
    page.set_location(
        &crate::navigation::address("https://offset-parent.test/original", &base).unwrap(),
    );
    run(&mut doc, &mut forms, &mut page, "if (childRead(menu) !== outer || mainGetter.call(childBody) !== null) throw 'owner restoration';");
    assert_eq!(page.state.dom_commits, commits);
}

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(concat!(
        "<!doctype html><html id=root><head><title>Original offset parent</title><link id=sheet rel=stylesheet href=theme.css></head><body id=body>",
        "<div id=outer class=positioned><div id=middle><button id=button type=button>Toggle</button>",
        "<ul id=list class=menu><li>Original item</li></ul><div id=plain>Static child</div></div></div>",
        "<div id=other class=positioned>Other parent</div><div id=fixed class=fixed>Fixed</div>",
        "<div id=hidden class=hidden><div id=hidden-child>Hidden child</div></div>",
        "<table id=table><tbody><tr><td id=cell><div id=table-child>Cell child</div></td></tr></tbody></table>",
        "<form id=form><input id=neighbor value=original></form></body></html>"
    ));
    doc.scripting_enabled = true;
    let mut sheets = HashMap::new();
    sheets.insert(
        id(&doc, "sheet"),
        concat!(
            "body{margin:0}.positioned{position:relative}",
            "#outer{left:20px;top:10px;width:200px;height:120px}",
            "#other{width:200px;height:120px}",
            ".menu{position:absolute;left:30px;top:40px;width:80px;height:36px;margin:0;padding:0}",
            ".fixed{position:fixed;left:5px;top:5px;width:20px;height:10px}",
            ".hidden{display:none}#table-child{width:40px;height:20px}"
        )
        .to_owned(),
    );
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let source = crate::navigation::address(
        "https://offset-parent.test/original",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&source);
    page.set_resource_base(&source);
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
            control.owner
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
    let (a, b, c) = page.take_image_changes();
    assert!(a.is_empty() && b.is_empty() && c.is_empty());
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
    kept(doc, forms, page);
}

#[test]
fn offset_parent_original_real_menu_box_then_live_ancestor_identity_and_null_cases() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var outer = document.getElementById('outer'), middle = document.getElementById('middle'),
    menu = document.getElementById('list'), button = document.getElementById('button'),
    plain = document.getElementById('plain'), other = document.getElementById('other'),
    body = document.getElementById('body'), root = document.getElementById('root'),
    parentBox = outer.getBoundingClientRect(), menuBox = menu.getBoundingClientRect();
if (outer.nodeName !== 'DIV' || menu.nodeName !== 'UL' || button.nodeName !== 'BUTTON'
    || menu.parentNode !== middle || middle.parentNode !== outer || outer.parentNode !== body
    || body.parentNode !== root || root !== document.documentElement)
    throw 'actual parsed HTML/button/list parent chain';
if (parentBox.x !== 20 || parentBox.y !== 10 || parentBox.width !== 200 || parentBox.height !== 120
    || menuBox.x !== 50 || menuBox.y !== 50 || menu.clientWidth !== 80 || menu.clientHeight !== 36
    || button.clientWidth <= 0 || button.toString() !== '[object HTMLButtonElement]'
    || menu.toString() !== '[object HTMLUListElement]')
    throw 'real positioned ancestor/native control/menu geometry prefix';
if (menu.ownerDocument !== document || document.getElementById('neighbor').value !== 'edited neighbor')
    throw 'real owner/native editor prefix';
document.title = 'Offset parent real layout prefix passed';
"#,
    );
    assert_eq!(doc.title, "Offset parent real layout prefix passed");

    // First new operation reads the actual UL, after its real layout positives.
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var first = menu.offsetParent;
if (first !== outer || menu.offsetParent !== first || button.offsetParent !== outer || plain.offsetParent !== outer)
    throw 'nearest actual positioned ancestor identity';
middle.className = 'positioned';
if (menu.offsetParent !== middle || plain.offsetParent !== middle)
    throw 'new nearer containing ancestor must win live';
middle.className = ''; outer.className = '';
if (menu.offsetParent !== body || plain.offsetParent !== body)
    throw 'ordinary static ancestors must fall through to real body';
outer.className = 'positioned';
if (menu.offsetParent !== outer || body.offsetParent !== null || root.offsetParent !== null)
    throw 'restored containing ancestor/root/body cases';
if (document.getElementById('table-child').offsetParent !== document.getElementById('cell'))
    throw 'static child uses nearest real static table cell';
if (document.getElementById('fixed').offsetParent !== null
    || document.getElementById('hidden').offsetParent !== null
    || document.getElementById('hidden-child').offsetParent !== null)
    throw 'viewport fixed or no associated box must be null';
menu.className = 'menu hidden';
if (menu.offsetParent !== null || menu.clientWidth !== 0) throw 'same retained hidden menu';
menu.className = 'menu';
middle.removeChild(menu);
if (menu.offsetParent !== null || menu.parentNode !== null || menu.ownerDocument !== document)
    throw 'detached retained menu has no box';
other.appendChild(menu);
if (menu.offsetParent !== other || menu.parentNode !== other || document.getElementById('list') !== menu)
    throw 'real reparented identity/cascade';
middle.appendChild(menu);
if (menu.offsetParent !== outer || menu.parentNode !== middle)
    throw 'original containing ancestor restored';
var fresh = document.createElement('div');
if (fresh.offsetParent !== null || fresh.ownerDocument !== document) throw 'fresh detached HTMLElement';
outer.appendChild(fresh);
if (fresh.offsetParent !== outer) throw 'same fresh element attached to actual containing ancestor';
"#,
    );
}

#[test]
fn offset_parent_protocol_canonical_getter_readonly_overrides_brands_and_child_scope() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var P = HTMLElement.prototype, D = Object.getOwnPropertyDescriptor(P, 'offsetParent'),
    menu = document.getElementById('list'), outer = document.getElementById('outer'),
    text = document.getElementById('plain').firstChild, comment = document.createComment('kept');
if (!D || typeof D.get !== 'function' || D.get.name !== 'get offsetParent' || D.get.length !== 0
    || D.set !== undefined || !D.enumerable || !D.configurable || D.value !== undefined
    || Object.getOwnPropertyDescriptor(Element.prototype, 'offsetParent') !== undefined
    || Object.getOwnPropertyDescriptor(Node.prototype, 'offsetParent') !== undefined)
    throw 'canonical readonly HTMLElement descriptor';
if (!('offsetParent' in menu) || Object.hasOwn(menu, 'offsetParent') || D.get.call(menu) !== outer)
    throw 'inherited getter returns genuine parent';
var rejected = 0;
for (var receiver of [null, undefined, {}, P, Element.prototype, document, text, comment]) {
    try { D.get.call(receiver); }
    catch (e) { if (e instanceof TypeError) rejected++; else throw e; }
}
if (rejected !== 8) throw 'getter accepted a non-HTMLElement receiver';
var effects = 0, value = {toString: function () { effects++; throw 'readonly conversion'; }};
menu.offsetParent = value;
if (menu.offsetParent !== outer || effects !== 0 || Object.hasOwn(menu, 'offsetParent'))
    throw 'sloppy readonly assignment';
try { (function () { 'use strict'; menu.offsetParent = value; })(); throw 'strict assignment accepted'; }
catch (e) { if (!(e instanceof TypeError)) throw e; }
var calls = 0;
Object.defineProperty(P, 'offsetParent', {configurable: true, get: function () {
    calls++; return value;
}});
if (menu.offsetParent !== value || calls !== 1 || D.get.call(menu) !== outer)
    throw 'ordinary prototype override versus canonical native getter';
delete P.offsetParent;
if ('offsetParent' in menu || menu.offsetParent !== undefined) throw 'deleted getter ghost';
Object.defineProperty(P, 'offsetParent', D);
Object.defineProperty(menu, 'offsetParent', {configurable: true, value: value});
if (menu.offsetParent !== value || D.get.call(menu) !== outer) throw 'ordinary own shadow';
delete menu.offsetParent;
if (menu.offsetParent !== outer) throw 'canonical restoration';

var frame = document.createElement('iframe');
frame.hidden = true;
document.body.appendChild(frame);
var child = frame.contentWindow, childBody = frame.contentDocument.body,
    F = child.Function, childGetter = F('return Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetParent").get;')();
if (childGetter === D.get || Object.getPrototypeOf(childGetter) !== F.prototype
    || childGetter.call(menu) !== outer)
    throw 'fresh canonical getter in genuine owner realm';
if (D.get.call(childBody) !== null || childBody.offsetParent !== null ||
    childGetter.call(frame.contentDocument.documentElement) !== null)
    throw 'child root and body have null offset parents';
try { D.get.call(frame.contentDocument.head); throw 'child renderer fabricated'; }
catch (e) { if (e.name !== 'NotSupportedError') throw e; }
try { frame.contentDocument.head.offsetParent; throw 'direct child renderer fabricated'; }
catch (e) { if (e.name !== 'NotSupportedError') throw e; }
if (effects !== 0 || menu.offsetParent !== outer || menu.parentNode !== document.getElementById('middle'))
    throw 'scope/readonly checks changed actual main identity';
"#,
    );
}
