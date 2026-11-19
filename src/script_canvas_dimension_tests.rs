//! Original context-free canvas dimension reflection reduction.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}

fn eval(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    assert_eq!(
        page.console_eval(doc, forms, source).errors,
        0,
        "{source}\n{}",
        page.report()
    );
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    assert!(page.take_image_changes().2.is_empty());
}

#[test]
fn canvas_dimensions_chained_assignment_reflects_real_raw_attributes_without_a_context() {
    let mut doc = crate::html::parse(
        "<html><body><form id=f><input id=neighbor name=keep value=seed>\
         <canvas id=parsed width='0020' height='10'></canvas>\
         </form></body></html>",
    );
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        var neighbor = document.getElementById('neighbor');
        var parsed = document.getElementById('parsed');
        var canvas = document.createElement('canvas');
        neighbor.value = 'edited neighbor';
        if (parsed.nodeType !== 1 || parsed.tagName !== 'CANVAS' ||
            parsed.ownerDocument !== document || parsed.parentNode !== document.getElementById('f') ||
            parsed.getAttribute('width') !== '0020' || parsed.getAttribute('height') !== '10')
            throw 'parsed identity and raw attributes';
        if (canvas.nodeType !== 1 || canvas.tagName !== 'CANVAS' ||
            canvas.ownerDocument !== document || canvas.parentNode !== null ||
            canvas.getAttribute('width') !== null || canvas.getAttribute('height') !== null ||
            neighbor.value !== 'edited neighbor' || neighbor.defaultValue !== 'seed')
            throw 'fresh canvas and native neighbor';
        "#,
    );
    let neighbor = id(&doc, "neighbor");
    let value = &mut forms.controls.get_mut(&neighbor).unwrap().value;
    value.cursor = 2;
    value.anchor = 5;
    value.offset = 1;
    let control = &forms.controls[&neighbor];
    let before = (
        control.value.text(),
        control.value.cursor,
        control.value.anchor,
        control.value.offset,
        control.dirty_value,
        control.owner,
    );
    let native_count = forms.controls.len();

    // The first new operation is the independently observed assignment. No
    // rendering context is requested, so a dimension test cannot pass by
    // substituting an unsupported context implementation.
    eval(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
        canvas.width = canvas.height = 1;
        if (canvas.width !== 1 || canvas.height !== 1 ||
            canvas.getAttribute('width') !== '1' || canvas.getAttribute('height') !== '1')
            throw 'chained reflected dimensions';
        if (parsed.width !== 20 || parsed.height !== 10 ||
            parsed.getAttribute('width') !== '0020' || parsed.getAttribute('height') !== '10')
            throw 'parsed numeric dimensions retain raw spelling';
        var pristine = document.createElement('canvas');
        if (pristine.width !== 300 || pristine.height !== 150 ||
            pristine.getAttribute('width') !== null || pristine.getAttribute('height') !== null)
            throw 'missing dimensions use defaults without creating attributes';
        if (canvas.ownerDocument !== document || canvas.parentNode !== null ||
            neighbor.value !== 'edited neighbor' || neighbor.defaultValue !== 'seed')
            throw 'ownership and neighboring native state';
        "#,
    );
    let after = &forms.controls[&neighbor];
    assert_eq!(forms.controls.len(), native_count);
    assert_eq!(
        (
            after.value.text(),
            after.value.cursor,
            after.value.anchor,
            after.value.offset,
            after.dirty_value,
            after.owner,
        ),
        before
    );
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "seed");
}
