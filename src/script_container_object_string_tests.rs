//! Original ordinary HTML ancestor strings; tags follow the real interface.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
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
fn kept(
    doc: &Document,
    forms: &Forms,
    page: &mut Page,
    node: usize,
    before: &(String, usize, usize, usize, bool, Option<usize>),
) {
    assert_eq!(&editor(forms, node), before);
    assert_eq!(forms.text_default_value(node).unwrap(), "original");
    assert_eq!(doc.attr(node, "value"), Some("original"));
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (a, b, c) = page.take_image_changes();
    assert!(a.is_empty() && b.is_empty() && c.is_empty());
}

#[test]
fn container_object_string_original_parsed_ancestors_real_interfaces_and_ordinary_restoration() {
    let mut doc = crate::html::parse(concat!(
        "<!doctype html><html id=html><head><title>Original container strings</title></head><body id=body>",
        "<main id=main><div id=container class=original><button id=button type=button>Original button</button>",
        "<ul id=list><li>Original item</li></ul></div></main>",
        "<section id=unpromoted>Unpromoted interface</section><a id=anchor href='../destination?q=one#part'>Original link</a>",
        "<form id=form><input id=neighbor name=keep value=original></form></body></html>"
    ));
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let source = crate::navigation::address(
        "https://container-string.test/path/page",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&source);
    page.set_resource_base(&source);
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );
    let neighbor = id(&doc, "neighbor");
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    {
        let e = &mut forms.controls.get_mut(&neighbor).unwrap().value;
        e.cursor = 5;
        e.anchor = 2;
        e.offset = 1;
    }
    let before = editor(&forms, neighbor);
    assert_eq!(before.5, Some(id(&doc, "form")));
    let prefix = page.console_eval(
        &mut doc,
        &mut forms,
        r#"
var div = document.getElementById('container'),
    main = document.getElementById('main'),
    body = document.getElementById('body'),
    html = document.getElementById('html'),
    button = document.getElementById('button'),
    list = document.getElementById('list'),
    neighbor = document.getElementById('neighbor'),
    stringify = Object.prototype.toString,
    tag = Symbol.toStringTag;
if (div.nodeName !== 'DIV'
    || main.nodeName !== 'MAIN'
    || body !== document.body
    || html !== document.documentElement
    || body.nodeName !== 'BODY'
    || html.nodeName !== 'HTML'
    || div.parentNode !== main
    || main.parentNode !== body
    || body.parentNode !== html
    || html.parentNode !== document)
    throw 'real parsed ancestor chain';
if (button.parentNode !== div
    || list.parentNode !== div
    || button.toString() !== '[object HTMLButtonElement]'
    || list.toString() !== '[object HTMLUListElement]'
    || button.toString !== stringify
    || list.toString !== stringify)
    throw 'qualified neighboring specialized interfaces';
if (div.ownerDocument !== document
    || main.ownerDocument !== document
    || div.getAttribute('class') !== 'original'
    || neighbor.value !== 'edited neighbor'
    || neighbor.defaultValue !== 'original'
    || neighbor.form !== document.getElementById('form'))
    throw 'real owner/raw/native editor prefix';
document.title = 'Container string real ancestor prefix passed';
"#,
    );
    assert_eq!(prefix.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "Container string real ancestor prefix passed");
    kept(&doc, &forms, &mut page, neighbor, &before);

    let out = page.console_eval(
        &mut doc,
        &mut forms,
        r#"
var divTag = div.toString();
if (divTag !== '[object HTMLDivElement]')
    throw 'direct real DIV string';
if (main.toString() !== '[object HTMLElement]'
    || body.toString() !== '[object HTMLBodyElement]'
    || html.toString() !== '[object HTMLHtmlElement]')
    throw 'actual generic/specialized ancestor interface tags';

var DP = HTMLDivElement.prototype,
    BP = HTMLBodyElement.prototype,
    HP = HTMLHtmlElement.prototype,
    GP = HTMLElement.prototype;
if (Object.getPrototypeOf(div) !== DP
    || Object.getPrototypeOf(body) !== BP
    || Object.getPrototypeOf(html) !== HP
    || Object.getPrototypeOf(main) !== GP
    || Object.getPrototypeOf(DP) !== GP
    || Object.getPrototypeOf(BP) !== GP
    || Object.getPrototypeOf(HP) !== GP)
    throw 'canonical concrete and generic prototype identities';
if (div.constructor !== HTMLDivElement
    || body.constructor !== HTMLBodyElement
    || html.constructor !== HTMLHtmlElement
    || main.constructor !== HTMLElement
    || !(div instanceof HTMLDivElement)
    || main instanceof HTMLDivElement
    || !(body instanceof HTMLBodyElement)
    || !(html instanceof HTMLHtmlElement))
    throw 'actual constructor brands';
for (var pair of [[div, 'HTMLDivElement'], [main, 'HTMLElement'], [body, 'HTMLBodyElement'], [html, 'HTMLHtmlElement']]) {
    var node = pair[0];
    if (node.toString !== stringify
        || node[tag] !== pair[1]
        || stringify.call(node) !== '[object ' + pair[1] + ']'
        || Object.hasOwn(node, 'toString'))
        throw 'ordinary inherited Object method and tag';
}


var DD = Object.getOwnPropertyDescriptor(DP, tag),
    GD = Object.getOwnPropertyDescriptor(GP, tag);
for (var entry of [[DP, 'HTMLDivElement'], [BP, 'HTMLBodyElement'], [HP, 'HTMLHtmlElement']]) {
    var d = Object.getOwnPropertyDescriptor(entry[0], tag);
    if (!d
        || d.value !== entry[1]
        || d.writable
        || d.enumerable
        || !d.configurable
        || Object.getOwnPropertyDescriptor(entry[0], 'toString') !== undefined)
        throw 'real concrete tag descriptor/method inheritance';
}
var own = function () {
    return this === div ? 'author-own' : 'wrong';
};
div.toString = own;
if (div.toString() !== 'author-own'
    || div.toString !== own
    || stringify.call(div) !== divTag
    || main.toString !== stringify)
    throw 'ordinary own stringifier shadow';
delete div.toString;
if (div.toString !== stringify
    || div.toString() !== divTag)
    throw 'own method deletion';

var order = '',
    marker = {};
Object.defineProperty(DP, 'toString', {
    configurable: true,
    get: function () {
        order += 'G';
        if (this !== div)
            throw 'getter receiver';
        return function (value) {
            order += 'C';
            if (this !== div || value !== marker)
                throw 'method receiver/value';
            return 'author-prototype';
        };
    }
});
function argument() {
    order += 'A';
    return marker;
}
if (div.toString(argument()) !== 'author-prototype'
    || order !== 'GAC'
    || main.toString !== stringify
    || button.toString !== stringify)
    throw 'inherited getter before arguments without neighbor change';
delete DP.toString;
if (div.toString !== stringify)
    throw 'prototype method deletion';
Object.defineProperty(div, tag, {
    configurable: true,
    value: 'AuthorContainer'
});
if (div.toString() !== '[object AuthorContainer]'
    || stringify.call(div) !== '[object AuthorContainer]'
    || DP[tag] !== 'HTMLDivElement')
    throw 'ordinary own tag override';
delete div[tag];
delete DP[tag];
if (div.toString() !== '[object HTMLElement]'
    || main.toString() !== '[object HTMLElement]')
    throw 'deleted concrete tag must expose actual parent';
Object.defineProperty(DP, tag, DD);
Object.defineProperty(GP, tag, {
    configurable: true,
    value: 'AuthorHTMLElement'
});
if (main.toString() !== '[object AuthorHTMLElement]'
    || div.toString() !== divTag
    || body.toString() !== '[object HTMLBodyElement]')
    throw 'generic ancestor must use canonical HTMLElement tag';
Object.defineProperty(GP, tag, GD);
main.removeChild(div);
if (div.parentNode !== null
    || div.ownerDocument !== document
    || div.toString() !== divTag
    || button.parentNode !== div
    || list.parentNode !== div)
    throw 'detached parsed subtree identity/brand';
main.appendChild(div);
if (div.parentNode !== main
    || document.getElementById('container') !== div
    || div.toString() !== divTag
    || main.toString() !== '[object HTMLElement]')
    throw 'real subtree reattachment';

var detached = document.createElement('div'),
    detachedMain = document.createElement('main');
if (detached.toString() !== divTag
    || Object.getPrototypeOf(detached) !== DP
    || detachedMain.toString() !== '[object HTMLElement]'
    || Object.getPrototypeOf(detachedMain) !== GP
    || detached.parentNode !== null
    || detached.ownerDocument !== document)
    throw 'genuine fresh detached interfaces';
main.appendChild(detached);
main.appendChild(detachedMain);
if (detached.toString() !== divTag
    || detachedMain.toString() !== '[object HTMLElement]')
    throw 'fresh connected interface identity';

var anchor = document.getElementById('anchor'),
    unpromoted = document.getElementById('unpromoted');
if (anchor.href !== 'https://container-string.test/destination?q=one#part'
    || anchor.getAttribute('href') !== '../destination?q=one#part'
    || location.toString() !== location.href
    || location.toString === stringify
    || window.toString() !== '[object Window]')
    throw 'existing URL/Location/Window path changed';
if (div.toString() !== divTag
    || main.toString() !== '[object HTMLElement]'
    || button.toString() !== '[object HTMLButtonElement]'
    || list.toString() !== '[object HTMLUListElement]'
    || neighbor.value !== 'edited neighbor'
    || neighbor.defaultValue !== 'original')
    throw 'restoration changed admitted neighbors';
"#,
    );
    kept(&doc, &forms, &mut page, neighbor, &before);
    assert_eq!(out.errors, 0, "{}", page.report());
    let commits = page.state.dom_commits;
    for source in [
        "Object.getPrototypeOf(unpromoted);",
        "unpromoted[Symbol.toStringTag];",
        "stringify.call(unpromoted);",
        "unpromoted.toString();",
    ] {
        let out = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(out.errors, 1, "{source}\n{}", page.report());
        assert_eq!(page.state.dom_commits, commits);
        kept(&doc, &forms, &mut page, neighbor, &before);
    }
    let out = page.console_eval(
        &mut doc,
        &mut forms,
        r#"
if (div.toString() !== '[object HTMLDivElement]'
    || main.toString() !== '[object HTMLElement]'
    || div.getAttribute('class') !== 'original')
    throw 'unpromoted refusal poisoned actual container';
"#,
    );
    assert_eq!(out.errors, 0, "{}", page.report());
    assert_eq!(page.state.dom_commits, commits);
    kept(&doc, &forms, &mut page, neighbor, &before);
}

#[test]
fn container_object_string_assignment_tail_keeps_location_and_attr_write_boundaries() {
    let mut doc = crate::html::parse("<!doctype html><body><div id=container>Original</div>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::new();
    let source = crate::navigation::address(
        "https://container-string.test/setters",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&source);
    page.set_resource_base(&source);
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );

    let out = page.console_eval(&mut doc, &mut forms, r#"
var frame = document.createElement('iframe');
frame.hidden = true;
document.body.appendChild(frame);
var child = frame.contentWindow,
    childBody = frame.contentDocument.body,
    record = {calls: 0},
    value = {kept: 37};
var install = child.Function('record',
    'var P = HTMLElement.prototype, D = Object.getOwnPropertyDescriptor(P, "toString");' +
    'Object.defineProperty(P, "toString", {configurable: true, set: function(v) {' +
    'record.calls++; record.receiver = this; record.value = v; }});' +
    'return {write: function(node, v) {node.toString = v;}, restore: function() {' +
    'delete P.toString; if (D) Object.defineProperty(P, "toString", D);}};');
var access = install(record);
access.write(childBody, value);
if (record.calls !== 1 || record.receiver !== childBody || record.value !== value
    || Object.hasOwn(childBody, 'toString'))
    throw 'normal admitted child container setter';
access.restore();
if (childBody.toString() !== '[object HTMLBodyElement]' || childBody.ownerDocument !== frame.contentDocument)
    throw 'child container canonical restoration';

var locationWrites = 0, attrWrites = 0,
    attr = document.getElementById('container').getAttributeNode('id');
Object.defineProperty(Object.prototype, 'prototype', {
    configurable: true,
    set: function(v) { locationWrites++; }
});
Object.defineProperty(Node.prototype, 'unimplementedAuthorKey', {
    configurable: true,
    set: function(v) { attrWrites++; }
});
"#);
    assert_eq!(out.errors, 0, "{}", page.report());
    let commits = page.state.dom_commits;
    for statement in [
        "location.prototype = value;",
        "attr.unimplementedAuthorKey = value;",
    ] {
        let out = page.console_eval(&mut doc, &mut forms, statement);
        assert_eq!(out.errors, 1, "{statement}\n{}", page.report());
        assert_eq!(page.state.dom_commits, commits);
        assert!(!page.has_navigation_request());
    }
    let out = page.console_eval(
        &mut doc,
        &mut forms,
        r#"
if (locationWrites !== 0 || attrWrites !== 0 || record.calls !== 1
    || location.href !== 'https://container-string.test/setters'
    || attr.value !== 'container' || attr.ownerElement !== document.getElementById('container'))
    throw 'unsupported writes invoked author setters or changed native state';
delete Object.prototype.prototype;
delete Node.prototype.unimplementedAuthorKey;
if (childBody.toString() !== '[object HTMLBodyElement]')
    throw 'refused unrelated writes poisoned normal container';
"#,
    );
    assert_eq!(out.errors, 0, "{}", page.report());
    assert_eq!(page.state.dom_commits, commits);
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (a, b, c) = page.take_image_changes();
    assert!(a.is_empty() && b.is_empty() && c.is_empty());
}
