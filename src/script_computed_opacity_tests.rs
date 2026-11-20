//! Original live opacity readback through the real admitted rendering cascade.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some(name))
        .unwrap()
}

fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(concat!(
        "<!doctype html><html id=root><head><link id=sheet rel=stylesheet href=theme.css></head><body>",
        "<div id=target class=probe style='opacity:.2;--opacity:kept'></div>",
        "<div id=parent><div id=inherited style='opacity:inherit'></div><div id=normal></div></div>",
        "<form id=form><input id=neighbor value=original></form></body></html>"
    ));
    doc.scripting_enabled = true;
    let sheets = HashMap::from([(
        id(&doc, "sheet"),
        concat!(
            "html{opacity:40%}.probe{width:80px;height:24px;opacity:60%!important;overflow:clip}",
            ".probe.changed{opacity:25%!important}.probe.hidden{display:none;opacity:0!important}",
            "#parent{opacity:.3}#parent.changed{opacity:.7}",
            "@media(max-width:699px){.probe{opacity:90%!important}}"
        )
        .into(),
    )]);
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let source = crate::navigation::address(
        "https://computed-opacity.test/original",
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
    forms
        .script_set_value(id(&doc, "neighbor"), "edited neighbor")
        .unwrap();
    (doc, forms, page)
}

fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    assert_eq!(
        page.console_eval(doc, forms, source).errors,
        0,
        "{source}\n{}",
        page.report()
    );
    let n = id(doc, "neighbor");
    assert_eq!(forms.controls[&n].value.text(), "edited neighbor");
    assert!(forms.controls[&n].dirty_value);
    assert_eq!(forms.text_default_value(n).unwrap(), "original");
    assert_eq!(forms.controls[&n].owner, Some(id(doc, "form")));
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
}

#[test]
fn opacity_inline_cssom_preserves_raw_declared_and_computed_values() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var root=document.documentElement, style=root.style, computed=getComputedStyle(root);
var original='color:red; OPACITY:25%; opacity:125% !important; opacity:bad';
root.setAttribute('style',original);
if(root.getAttribute('style')!==original || style.opacity!=='1.25' ||
   style.getPropertyValue('OPACITY')!=='1.25' || computed.opacity!=='1')
    throw 'raw attribute, important specified winner, or computed clamp';
var conversions=0;
style.opacity={toString:function(){conversions++;return '25%';}};
if(conversions!==1 || style.opacity!=='0.25' || computed.opacity!=='0.25' ||
   style.color!=='red' || root.getAttribute('style').indexOf('opacity:0.25')<0)
    throw 'converted setter, live computed style or neighboring declaration';
style.setProperty('OPACITY','-2','important');
if(style.opacity!=='-2' || computed.opacity!=='0')throw 'negative specified opacity';
"#);
    let before = page.dom_commit_generation();
    run(&mut doc, &mut forms, &mut page, r#"
var savedAttribute=root.getAttribute('style');
style.opacity='40px'; style.setProperty('opacity','five','important');
style.setProperty('opacity','0.8','urgent');
if(root.getAttribute('style')!==savedAttribute || style.opacity!=='-2')
    throw 'invalid value or priority replaced the declaration';
"#);
    assert_eq!(page.dom_commit_generation(), before);
    run(&mut doc, &mut forms, &mut page, r#"
style.opacity=null;
if(style.opacity!=='' || computed.opacity!=='0.4' || style.color!=='red')
    throw 'removing opacity did not expose the stylesheet';
root.setAttribute('style','opacity:bad;color:blue');
if(style.opacity!=='' || computed.opacity!=='0.4' || style.color!=='blue' ||
   root.getAttribute('style')!=='opacity:bad;color:blue')throw 'invalid raw declaration';
"#);
}

#[test]
fn opacity_inline_style_enumeration_uses_the_same_valid_important_winner() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var root=document.documentElement, style=root.style, computed=getComputedStyle(root);
var raw='color:red; OPACITY:125% /* winner */ !important;width:6px /* keep */;opacity:bad !important;background-color:blue;opacity:85%';
root.setAttribute('style',raw);
var desc=Object.getOwnPropertyDescriptor(style,'1'), names=[];
for(var key in style)names.push(key);
if(style.length!==4 || style.item(1)!=='opacity' || style[2]!=='width' ||
   !desc || desc.value!=='opacity' || desc.writable || !desc.enumerable || !desc.configurable ||
   Object.hasOwn(style,'opacity') || names.indexOf('opacity')<0 || Object.keys(style).join(',')!=='0,1,2,3')
    throw 'numeric own indexed winner and inherited property';
if(style.getPropertyValue('OPACITY')!=='1.25' || style.opacity!=='1.25' ||
   computed.opacity!=='1' || root.getAttribute('style')!==raw)throw 'specified priority or premature mutation';
style.opacity='25%';
if(style.opacity!=='0.25' || computed.opacity!=='0.25' || style[1]!=='opacity' || style[2]!=='width' ||
   root.getAttribute('style')!=='color:red;opacity:0.25;width:6px /* keep */;background-color:blue;')
    throw 'replacement moved the winning slot or rewrote a neighbor';
if(style.removeProperty('OPACITY')!=='0.25' || style.length!==3 || style.item(1)!=='width' ||
   style.opacity!=='' || computed.opacity!=='0.4')throw 'remove did not update the same slot';
"#);
}

#[test]
fn opacity_inline_url_refusals_preserve_existing_style_and_native_neighbor() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,
        "var root=document.documentElement;root.setAttribute('style','color:red;opacity:25%');");
    let before = page.dom_commit_generation();
    for source in [
        "root.style.opacity='url(/unused-opacity)';",
        "root.setAttribute('style','color:blue;opacity:url(/unused-opacity)');",
        "root.style.setProperty('opacity','var(--a, URL(/unused-opacity))');",
    ] {
        assert_eq!(page.console_eval(&mut doc,&mut forms,source).errors, 1, "{source}");
        assert_eq!(page.dom_commit_generation(), before);
        run(&mut doc,&mut forms,&mut page,r#"
if(root.getAttribute('style')!=='color:red;opacity:25%' || root.style.opacity!=='0.25' ||
   getComputedStyle(root).opacity!=='0.25')throw 'refused value changed state';
"#);
    }
}

#[test]
fn computed_opacity_original_root_read_live_cascade_and_noninheritance() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var target=document.getElementById('target'), parent=document.getElementById('parent');
if(target.clientWidth!==80 || target.clientHeight!==24 || target.getAttribute('style')!=='opacity:.2;--opacity:kept')
    throw 'real geometry and raw inline prefix';
document.title='Opacity prefix passed';
"#,
    );
    assert_eq!(doc.title, "Opacity prefix passed");
    // This exact ordinary root read is the independently observed first fault.
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
if(getComputedStyle(document.documentElement).getPropertyValue('opacity')!=='0.4')
    throw 'root computed opacity';
var saved=getComputedStyle(target),read=saved.getPropertyValue;
if(saved.opacity!=='0.6'||read.call(saved,'oPaCiTy')!=='0.6'||saved.overflow!=='clip'
   ||read.call(saved,'--opacity').trim()!=='kept'||getComputedStyle(parent).opacity!=='0.3'
   ||getComputedStyle(document.getElementById('normal')).opacity!=='1'
   ||getComputedStyle(document.getElementById('inherited')).opacity!=='0.3')
    throw 'admitted sheet, numeric percentage, custom and noninheritance';
parent.className='changed';
if(getComputedStyle(document.getElementById('inherited')).opacity!=='0.7')throw 'live explicit inheritance';
target.className='probe changed';
if(saved.opacity!=='0.25'||target.getAttribute('style')!=='opacity:.2;--opacity:kept')throw 'live class cascade';
target.className='probe hidden';
if(saved.opacity!=='0'||target.clientWidth!==0)throw 'display none retains computed opacity';
target.className='probe';
document.body.removeChild(target);
if(saved.opacity!==''||read.call(saved,'OPACITY')!=='')throw 'detached readback';
document.body.appendChild(target);
if(saved.opacity!=='0.6'||getComputedStyle(document.getElementById('neighbor')).opacity!=='1')throw 'reconnection or native neighbor';
if(!('opacity' in saved)||'Opacity' in saved)throw 'attribute case';
try{read.call({},'opacity');throw 'wrong receiver accepted';}catch(e){if(!(e instanceof TypeError))throw e;}
try{saved.setProperty('opacity','0');throw 'computed mutation';}catch(e){if(e.name!=='NoModificationAllowedError')throw e;}
document.title='Opacity live passed';
"#,
    );
    assert_eq!(doc.title, "Opacity live passed");
    page.update_geometry(640, 600, 1.0, HashMap::new());
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "if(saved.opacity!=='0.9')throw 'live viewport cascade';",
    );
}

#[test]
fn computed_opacity_cached_read_charges_exact_serialization_and_refuses_exhaustion() {
    let (mut doc, mut forms, mut page) = fixture();
    let node = id(&doc, "target");
    let commits = page.state.dom_commits;
    let mut bridge = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    };
    let Value::Host(handle) = bridge
        .create_computed_style(&[Value::Host(node_handle(node))])
        .unwrap()
    else {
        panic!("actual declaration");
    };
    assert!(matches!(bridge.get(handle,"opacity").unwrap(),Value::String(v) if v=="0.6"));
    bridge.state.copy_work = 2;
    assert!(bridge.get(handle, "opacity").is_err());
    assert_eq!(bridge.state.copy_work, 0);
    bridge.state.copy_work = 3;
    assert!(matches!(bridge.get(handle,"opacity").unwrap(),Value::String(v) if v=="0.6"));
    assert_eq!(bridge.state.copy_work, 0, "one exact output copy debit");
    bridge.state.host_work = 0;
    assert!(bridge.get(handle, "opacity").is_err());
    assert_eq!(bridge.state.dom_commits, commits);
}
