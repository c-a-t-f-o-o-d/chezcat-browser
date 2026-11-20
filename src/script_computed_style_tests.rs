//! Original computed custom-property readback through the real renderer binding.
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
fn native_kept(
    doc: &Document,
    forms: &Forms,
    node: usize,
    kept: &(String, usize, usize, usize, bool, Option<usize>),
) {
    assert_eq!(&editor(forms, node), kept);
    assert_eq!(forms.text_default_value(node).unwrap(), "original");
    assert_eq!(doc.attr(node, "value"), Some("original"));
}
fn no_resources(page: &mut Page) {
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (a, b, c) = page.take_image_changes();
    assert!(a.is_empty() && b.is_empty() && c.is_empty());
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
    no_resources(page);
}

#[test]
fn computed_style_original_real_sheet_chain_live_handle_viewport_and_copy_failure() {
    let mut doc = crate::html::parse(concat!(
        "<!doctype html><title>Computed custom-property seed</title><head>",
        "<link id=sheet rel=stylesheet href=theme.css></head><body>",
        "<section id=parent style='--origin:parent'><div id=target class=probe ",
        "style='width:23px;--position:inline;--inline-only:own'>Original target</div></section>",
        "<form id=f><input id=neighbor name=keep value=original></form>"
    ));
    doc.scripting_enabled = true;
    let sheet = id(&doc, "sheet");
    // This is the same admitted external-sheet map and Geometry entry point
    // App supplies after loading. No network, alternate cascade or inline stub.
    let mut sheets = HashMap::new();
    sheets.insert(
        sheet,
        concat!(
            ".probe{width:143px !important;--position:external !important;",
            "--Alias:var(--origin);--final:var(--Alias);--Case:Upper;--case:lower;",
            "--fallback:var(--not-present,fallback);--cycle:var(--cycle)}",
            ".probe.changed{--position:changed !important}",
            "#parent.later{--origin:later !important}",
            "@media(min-width:700px){.probe{--viewport:wide}}",
            "@media(max-width:699px){.probe{width:97px !important;--viewport:narrow}}"
        )
        .to_owned(),
    );
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let location = crate::navigation::address(
        "https://computed-style.test/original",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&location);
    page.set_resource_base(&location);
    page.set_stylesheet_sources(&doc, &sheets);
    page.set_geometry(crate::script_geometry::Geometry::new(
        1280,
        800,
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
    {
        let e = &mut forms.controls.get_mut(&neighbor).unwrap().value;
        e.cursor = 5;
        e.anchor = 2;
        e.offset = 1;
    }
    let kept = editor(&forms, neighbor);
    assert_eq!(kept.5, Some(id(&doc, "f")));
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var target=document.getElementById('target'),parent=document.getElementById('parent'),neighbor=document.getElementById('neighbor');
if(target.clientWidth!==143||target.getAttribute('style')!=='width:23px;--position:inline;--inline-only:own'||target.getAttribute('class')!=='probe')throw 'real admitted-sheet geometry and raw inline positive';
if(target.parentNode!==parent||target.ownerDocument!==document||neighbor.value!=='edited neighbor'||neighbor.defaultValue!=='original'||neighbor.form!==document.getElementById('f'))throw 'real DOM/native owner prefix';
document.title='Computed style actual cascade prefix passed';
"#,
    );
    assert_eq!(doc.title, "Computed style actual cascade prefix passed");
    native_kept(&doc, &forms, neighbor, &kept);

    // First new API boundary: all literal geometry/raw/native positives above
    // must complete before this can fail for missing getComputedStyle.
    let first=page.console_eval(&mut doc,&mut forms,r#"
var saved=getComputedStyle(target),read=saved.getPropertyValue;
if(saved===target.style||read.call(saved,'--position').trim()!=='external')throw 'computed sheet winner must differ from raw inline';
if(saved.getPropertyValue('--final').trim()!=='parent'||saved.getPropertyValue('--fallback').trim()!=='fallback'||saved.getPropertyValue('--Case').trim()!=='Upper'||saved.getPropertyValue('--case').trim()!=='lower')throw 'computed var chain/fallback/custom-name case';
if(saved.getPropertyValue('--absent')!==''||saved.getPropertyValue('--cycle')!=='')throw 'actual missing and guaranteed-invalid custom values';
target.className='probe changed';parent.className='later';
if(read.call(saved,'--position').trim()!=='changed'||read.call(saved,'--final').trim()!=='later'||saved.getPropertyValue('--viewport').trim()!=='wide')throw 'retained declaration must read same-task current cascade';
if(target.getAttribute('style')!=='width:23px;--position:inline;--inline-only:own'||neighbor.value!=='edited neighbor'||neighbor.defaultValue!=='original')throw 'computed read altered raw/native values';
"#);
    native_kept(&doc, &forms, neighbor, &kept);
    no_resources(&mut page);
    assert_eq!(first.errors, 0, "{}", page.report());

    page.update_geometry(640, 800, 1.0, HashMap::new());
    run(&mut doc,&mut forms,&mut page,"if(read.call(saved,'--viewport').trim()!=='narrow'||target.clientWidth!==97)throw 'saved declaration ignored actual viewport invalidation';");
    page.state.geometry.as_mut().unwrap().set_stylesheet(
        sheet,
        Some(".probe{width:211px !important;--position:sheet !important}"),
    );
    run(&mut doc,&mut forms,&mut page,"if(read.call(saved,'--position').trim()!=='sheet'||saved.getPropertyValue('--final')!==''||target.clientWidth!==211)throw 'saved declaration retained retired sheet values';");
    native_kept(&doc, &forms, neighbor, &kept);

    // A real missing property above is empty. Exhaustion while copying a
    // present computed value must be a fatal refusal, never another empty value.
    let copy_limit = page.state.limits.copy_bytes;
    page.state.limits.copy_bytes = 0;
    page.clear_console();
    let exhausted = page.console_eval(&mut doc, &mut forms, "read.call(saved,'--position');");
    assert_eq!(exhausted.errors, 1, "{}", page.report());
    assert!(page.runtime.invocation_failed_fatally());
    let report = page.report().to_ascii_lowercase();
    assert!(
        report.contains("copy") || report.contains("string work"),
        "{}",
        page.report()
    );
    native_kept(&doc, &forms, neighbor, &kept);
    no_resources(&mut page);
    page.state.limits.copy_bytes = copy_limit;
    run(&mut doc,&mut forms,&mut page,"if(read.call(saved,'--position').trim()!=='sheet')throw 'copy refusal poisoned later computed read';");
    native_kept(&doc, &forms, neighbor, &kept);
}
