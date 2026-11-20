//! Original overflow readback through the actual renderer's admitted cascade.
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
    node: usize,
    before: &(String, usize, usize, usize, bool, Option<usize>),
) {
    assert_eq!(&editor(forms, node), before);
    assert_eq!(forms.text_default_value(node).unwrap(), "original");
    assert_eq!(doc.attr(node, "value"), Some("original"));
}
fn no_effects(page: &mut Page) {
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (a, b, c) = page.take_image_changes();
    assert!(a.is_empty() && b.is_empty() && c.is_empty());
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
    no_effects(page);
}

#[test]
fn computed_overflow_original_real_sheet_axes_live_readonly_detached_and_shared_limits() {
    let mut doc=crate::html::parse(concat!(
        "<!doctype html><title>Original computed overflow</title><head><link id=sheet rel=stylesheet href=theme.css></head><body>",
        "<section id=parent class=ancestor><div id=target class='panel clipped' style='width:19px;height:7px;overflow:visible'>Original clipped</div>",
        "<div id=inherited class='panel inherited'>Inherited</div><div id=initial class='panel reset'>Initial</div><div id=unset class='panel unset'>Unset</div>",
        "<div id=axis-inherit class='panel axis-inherit'>Axis inherit</div></section>",
        "<div id=visible class='panel open'>Visible</div><div id=clip class='panel clip-pair'>Independent clip</div>",
        "<div id=mixed class='panel mixed'>Mixed axes</div><div id=pair class='panel pair'>Mixed shorthand</div><div id=ordered class='panel ordered'>Axis order</div>",
        "<form id=form><input id=neighbor name=keep value=original></form>"
    ));
    doc.scripting_enabled = true;
    let sheet = id(&doc, "sheet");
    let mut sheets = HashMap::new();
    sheets.insert(sheet,concat!(
        ".panel{width:143px !important;height:23px !important}",
        ".ancestor{overflow:hidden}.ancestor.changed{overflow:visible}",
        ".clipped{overflow:hidden !important;--proof:hidden}.open{overflow:visible !important;--proof:visible}",
        ".inherited{overflow:inherit}.reset{overflow:hidden;overflow:initial}.unset{overflow:hidden;overflow:unset}",
        ".axis-inherit{overflow-x:inherit;overflow-y:initial}",
        ".clip-pair{overflow-x:visible;overflow-y:clip}",
        ".mixed{overflow-x:hidden;overflow-y:visible}.pair{overflow:hidden visible}",
        ".ordered{overflow:hidden;overflow-x:visible;overflow-y:hidden}"
    ).to_owned());
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let location = crate::navigation::address(
        "https://computed-overflow.test/original",
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
    {
        let e = &mut forms.controls.get_mut(&neighbor).unwrap().value;
        e.cursor = 5;
        e.anchor = 2;
        e.offset = 1;
    }
    let before = editor(&forms, neighbor);
    assert_eq!(before.5, Some(id(&doc, "form")));
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var target=document.getElementById('target'),visible=document.getElementById('visible'),parent=document.getElementById('parent'),neighbor=document.getElementById('neighbor');
if(target.clientWidth!==143||target.clientHeight!==23||visible.clientWidth!==143||visible.clientHeight!==23)throw 'real admitted-sheet geometry before overflow read';
if(target.getAttribute('style')!=='width:19px;height:7px;overflow:visible'||getComputedStyle(target).getPropertyValue('--proof').trim()!=='hidden'||getComputedStyle(visible).getPropertyValue('--proof').trim()!=='visible')throw 'real external hidden/visible controls and raw inline positive';
if(target.parentNode!==parent||target.ownerDocument!==document||neighbor.value!=='edited neighbor'||neighbor.defaultValue!=='original'||neighbor.form!==document.getElementById('form'))throw 'real DOM/native editor owner prefix';
document.title='Computed overflow real cascade prefix passed';
"#,
    );
    assert_eq!(doc.title, "Computed overflow real cascade prefix passed");
    kept(&doc, &forms, neighbor, &before);

    let out=page.console_eval(&mut doc,&mut forms,r#"
var saved=getComputedStyle(target),first=saved.overflow;
if(first!=='hidden')throw 'direct overflow must read actual sheet winner';
var read=saved.getPropertyValue;if(saved.overflowX!=='hidden'||saved.overflowY!=='hidden'||read.call(saved,'overflow')!=='hidden'||read.call(saved,'overflow-x')!=='hidden'||read.call(saved,'OVERFLOW-Y')!=='hidden'||read.call(saved,'OvErFlOw')!=='hidden')throw 'camel-case/hyphen/case readback and shorthand collapse';
var open=getComputedStyle(visible);if(open.overflow!=='visible'||open.overflowX!=='visible'||open.overflowY!=='visible')throw 'actual visible control';
var clip=getComputedStyle(document.getElementById('clip'));if(clip.overflow!=='visible clip'||clip.overflowX!=='visible'||clip.overflowY!=='clip')throw 'visible/clip axes stay independent';
var mixed=getComputedStyle(document.getElementById('mixed')),pair=getComputedStyle(document.getElementById('pair')),ordered=getComputedStyle(document.getElementById('ordered'));
if(mixed.overflow!=='hidden auto'||mixed.overflowX!=='hidden'||mixed.overflowY!=='auto'||pair.overflow!=='hidden auto'||ordered.overflow!=='auto hidden'||ordered.getPropertyValue('OVERFLOW-X')!=='auto')throw 'mixed axes computed coercion/shorthand/declaration order';
var inherited=getComputedStyle(document.getElementById('inherited')),initial=getComputedStyle(document.getElementById('initial')),unset=getComputedStyle(document.getElementById('unset')),axis=getComputedStyle(document.getElementById('axis-inherit'));
if(inherited.overflow!=='hidden'||initial.overflow!=='visible'||unset.overflow!=='visible'||axis.overflow!=='hidden auto')throw 'actual inherited/initial/noninherited unset/axis CSS-wide values';
parent.className='ancestor changed';target.className='panel open';
if(saved.overflow!=='visible'||saved.overflowX!=='visible'||read.call(saved,'OVERFLOW-Y')!=='visible'||inherited.overflow!=='visible'||axis.overflow!=='visible')throw 'retained declaration ignored same-task element/ancestor class';
var conversions=0,key={toString:function(){conversions++;target.className='panel clipped';return 'OVERFLOW-X';}};
if(read.call(saved,key)!=='hidden'||conversions!==1||saved.overflow!=='hidden')throw 'DOMString conversion before fresh cascade';
try{read.call(saved,Symbol());throw 'Symbol property accepted';}catch(e){if(!(e instanceof TypeError))throw e;}
try{saved.setProperty('overflow','visible');throw 'computed overflow written';}catch(e){if(e.name!=='NoModificationAllowedError')throw e;}
try{saved.removeProperty('overflow');throw 'computed overflow removed';}catch(e){if(e.name!=='NoModificationAllowedError')throw e;}
if(saved.overflow!=='hidden'||target.getAttribute('style')!=='width:19px;height:7px;overflow:visible')throw 'read-only operation changed author or computed style';
parent.removeChild(target);if(saved.overflow!==''||saved.overflowX!==''||saved.overflowY!==''||read.call(saved,'OVERFLOW')!=='')throw 'retained detached declaration must be empty';
parent.appendChild(target);if(saved.overflow!=='hidden'||target.ownerDocument!==document)throw 'retained declaration failed real reconnection';
var fresh=document.createElement('div');fresh.className='panel clipped';var detached=getComputedStyle(fresh);
if(detached.overflow!==''||detached.getPropertyValue('overflow-x')!=='')throw 'fresh detached declaration leaked sheet';document.body.appendChild(fresh);
if(detached.overflow!=='hidden'||detached.overflowX!=='hidden')throw 'fresh declaration ignored real connection';
"#);
    kept(&doc, &forms, neighbor, &before);
    no_effects(&mut page);
    assert_eq!(out.errors, 0, "{}", page.report());

    // Direct real Bridge reads isolate its final finite string-copy allowance
    // from the VM's independent argument/source copy work. The cache is warm.
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
            panic!("real computed declaration handle");
        };
        assert!(
            matches!(bridge.get(handle,"overflow").unwrap(),Value::String(value) if value=="hidden")
        );
        let copy_before = bridge.state.copy_work;
        assert!(
            matches!(bridge.get(handle,"overflow").unwrap(),Value::String(value) if value=="hidden")
        );
        assert_eq!(
            bridge.state.copy_work,
            copy_before - "hidden".len(),
            "warm enum read must charge the returned bytes exactly once"
        );
        bridge.state.copy_work = 0;
        assert!(bridge
            .get(handle, "overflow")
            .unwrap_err()
            .to_ascii_lowercase()
            .contains("string"));
        bridge.state.copy_work = copy_before;
        bridge.state.host_work = 0;
        assert!(bridge
            .get(handle, "overflow")
            .unwrap_err()
            .to_ascii_lowercase()
            .contains("work"));
    }
    assert_eq!(page.state.dom_commits, commits);
    kept(&doc, &forms, neighbor, &before);
    no_effects(&mut page);
    run(&mut doc,&mut forms,&mut page,"if(saved.overflow!=='hidden'||read.call(saved,'OVERFLOW-Y')!=='hidden')throw 'budget failure poisoned later job';");
    let out = page.console_eval(&mut doc, &mut forms, "saved.overflow='visible';");
    assert_eq!(out.errors, 1, "{}", page.report());
    assert_eq!(page.state.dom_commits, commits);
    kept(&doc, &forms, neighbor, &before);
    no_effects(&mut page);
    run(&mut doc,&mut forms,&mut page,"if(saved.overflow!=='hidden'||target.getAttribute('style')!=='width:19px;height:7px;overflow:visible')throw 'direct write changed real overflow';");
}
