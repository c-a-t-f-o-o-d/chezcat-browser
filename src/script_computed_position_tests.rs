//! Original position readback from the same admitted cascade as real geometry.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}
fn fixture() -> (Document, Forms, Page, usize) {
    let mut doc = crate::html::parse(concat!(
        "<!doctype html><title>Computed position</title><head><link id=sheet rel=stylesheet href=theme.css></head><body>",
        "<section id=parent class=parent><div id=target class=probe style='width:19px;height:7px;position:static'>Original</div>",
        "<div id=inherited class=inherited>Inherited</div><div id=normal>Normal</div><div id=initial class=initial>Initial</div><div id=unset class=unset>Unset</div></section>",
        "<div id=control>Unaffected</div><form id=form><input id=neighbor value=original></form>"
    ));
    doc.scripting_enabled = true;
    let sheet = id(&doc, "sheet");
    let mut sheets = HashMap::new();
    sheets.insert(sheet, concat!(
        "body{margin:0}.parent{position:relative}.parent.changed{position:absolute}",
        ".probe{position:relative!important;left:17px;top:11px;width:80px!important;height:24px!important;overflow:hidden;--position:lower;--Position:upper}",
        ".probe.changed{position:absolute!important}.probe.hidden{display:none;position:fixed!important}",
        ".inherited{position:inherit}.initial{position:absolute;position:initial}.unset{position:fixed;position:unset}",
        "#control{width:40px;height:20px;overflow:visible;--proof:control}",
        "@media(max-width:699px){.probe{position:fixed!important}}"
    ).to_owned());
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let source = crate::navigation::address(
        "https://computed-position.test/original",
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
    (doc, forms, page, sheet)
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
fn computed_position_original_real_layout_live_hidden_detached_media_and_bounded_reads() {
    let (mut doc, mut forms, mut page, sheet) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var target = document.getElementById('target'),
    parent = document.getElementById('parent'),
    control = document.getElementById('control'),
    saved = getComputedStyle(target),
    read = saved.getPropertyValue,
    rect = target.getBoundingClientRect();
if (target.clientWidth !== 80 || target.clientHeight !== 24 || rect.x !== 17 || rect.y !== 11
    || control.clientWidth !== 40 || control.clientHeight !== 20)
    throw 'real admitted-sheet position geometry prefix';
if (target.style.position !== 'static' || saved.overflow !== 'hidden'
    || read.call(saved, '--position').trim() !== 'lower'
    || getComputedStyle(control).overflow !== 'visible'
    || getComputedStyle(control).getPropertyValue('--proof').trim() !== 'control')
    throw 'raw inline and unaffected overflow/custom positives';
if (target.parentNode !== parent || target.ownerDocument !== document)
    throw 'real target owner/topology';
document.title = 'Computed position real layout prefix passed';
"#,
    );
    assert_eq!(doc.title, "Computed position real layout prefix passed");

    // First new read follows actual rendered geometry and existing style APIs.
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var first = saved.position;
if (first !== 'relative' || read.call(saved, 'position') !== 'relative')
    throw 'actual external position winner';
target.className = 'probe changed';
if (saved.position !== 'absolute' || saved.overflow !== 'hidden'
    || target.getAttribute('style') !== 'width:19px;height:7px;position:static')
    throw 'retained live class cascade versus raw declaration';
target.className = 'probe hidden';
if (saved.position !== 'fixed' || target.clientWidth !== 0 || !target.isConnected)
    throw 'display none must retain computed position';
target.className = 'probe';
parent.removeChild(target);
if (saved.position !== '' || read.call(saved, 'POSITION') !== '' || target.ownerDocument !== document)
    throw 'detached declaration must be empty';
parent.appendChild(target);
if (saved.position !== 'relative' || target.parentNode !== parent)
    throw 'real retained declaration reconnection';
try { saved.setProperty('position', 'fixed'); throw 'computed position written'; }
catch (e) { if (e.name !== 'NoModificationAllowedError') throw e; }
try { saved.removeProperty('position'); throw 'computed position removed'; }
catch (e) { if (e.name !== 'NoModificationAllowedError') throw e; }
"#,
    );
    page.update_geometry(640, 600, 1.0, HashMap::new());
    run(&mut doc, &mut forms, &mut page, "if (saved.position !== 'fixed' || saved.overflow !== 'hidden') throw 'retained actual media change';");
    page.state.geometry.as_mut().unwrap().set_stylesheet(
        sheet,
        Some("#target{position:relative!important;overflow:clip;--position:replacement}"),
    );
    run(&mut doc, &mut forms, &mut page, "if (saved.position !== 'relative' || saved.overflow !== 'clip' || read.call(saved, '--position').trim() !== 'replacement') throw 'real sheet replacement reused stale position';");

    let commits = page.state.dom_commits;
    let target = id(&doc, "target");
    {
        let mut bridge = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        };
        let Value::Host(handle) = bridge
            .create_computed_style(&[Value::Host(node_handle(target))])
            .unwrap()
        else {
            panic!("genuine retained declaration");
        };
        assert!(
            matches!(bridge.get(handle, "position").unwrap(), Value::String(value) if value == "relative")
        );
        bridge.state.copy_work = "relative".len() - 1;
        assert!(bridge.get(handle, "position").is_err());
        assert_eq!(bridge.state.copy_work, 0);
        bridge.state.copy_work = "relative".len();
        assert!(
            matches!(bridge.get(handle, "position").unwrap(), Value::String(value) if value == "relative")
        );
        assert_eq!(
            bridge.state.copy_work, 0,
            "charge serialized bytes exactly once"
        );
        bridge.state.host_work = 0;
        assert!(bridge.get(handle, "position").is_err());
    }
    assert_eq!(page.state.dom_commits, commits);
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "if (saved.position !== 'relative') throw 'read budget refusal poisoned later job';",
    );
    let out = page.console_eval(&mut doc, &mut forms, "saved.position = 'fixed';");
    assert_eq!(out.errors, 1, "{}", page.report());
    assert_eq!(page.state.dom_commits, commits);
    run(&mut doc, &mut forms, &mut page, "if (saved.position !== 'relative' || target.style.position !== 'static') throw 'readonly assignment changed actual declaration';");
}

#[test]
fn computed_position_protocol_names_conversion_inheritance_and_distinct_css_keys() {
    let (mut doc, mut forms, mut page, _) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var target = document.getElementById('target'),
    parent = document.getElementById('parent'),
    saved = getComputedStyle(target),
    read = saved.getPropertyValue;
if (!('position' in saved) || 'Position' in saved || 'POSITION' in saved
    || saved.position !== 'relative' || read.call(saved, 'pOsItIoN') !== 'relative')
    throw 'exact attribute case versus ASCII CSS property case';
if (read.call(saved, '--position').trim() !== 'lower' || read.call(saved, '--Position').trim() !== 'upper')
    throw 'ordinary position name folded custom property case';
var inherited = getComputedStyle(document.getElementById('inherited'));
if (inherited.position !== 'relative' || getComputedStyle(document.getElementById('normal')).position !== 'static'
    || getComputedStyle(document.getElementById('initial')).position !== 'static'
    || getComputedStyle(document.getElementById('unset')).position !== 'static')
    throw 'actual inherit/initial/unset and natural noninheritance';
parent.className = 'parent changed';
if (inherited.position !== 'absolute') throw 'retained inherited parent mutation';
var conversions = 0, key = {toString: function () {
    conversions++; target.className = 'probe changed'; return 'POSITION';
}};
if (read.call(saved, key) !== 'absolute' || conversions !== 1 || saved.overflow !== 'hidden')
    throw 'name conversion must precede current cascade';
try { read.call(saved, Symbol()); throw 'Symbol CSS name accepted'; }
catch (e) { if (!(e instanceof TypeError)) throw e; }
var fresh = document.createElement('div'), detached = getComputedStyle(fresh);
if (detached.position !== '' || detached.getPropertyValue('POSITION') !== '')
    throw 'fresh detached declaration';
document.body.appendChild(fresh);
if (detached.position !== 'static') throw 'fresh connected initial position';
"#,
    );
    let commits = page.state.dom_commits;
    for statement in [
        "read.call(saved, 'background-position');",
        "read.call(saved, 'object-position');",
        "saved.Position;",
    ] {
        let out = page.console_eval(&mut doc, &mut forms, statement);
        assert_eq!(out.errors, 1, "{statement}\n{}", page.report());
        assert_eq!(page.state.dom_commits, commits);
        kept(&doc, &forms, &mut page);
    }
    run(&mut doc, &mut forms, &mut page, "if (saved.position !== 'absolute' || saved.overflow !== 'hidden') throw 'unsupported different CSS key changed position';");
}
