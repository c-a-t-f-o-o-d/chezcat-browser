//! Original translation controls over genuine style declarations and geometry.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(concat!(
        "<!doctype html><html><head><title>Original translation</title><link id=sheet rel=stylesheet href=theme.css></head><body>",
        "<div id=parent><div id=target style='color:#123456'></div></div>",
        "<div id=control></div><form id=form><input id=neighbor name=neighbor value=original></form></body></html>"
    ));
    doc.scripting_enabled = true;
    let sheets = HashMap::from([(id(&doc, "sheet"), concat!(
        "body{margin:0}#parent{position:relative;width:300px;height:160px}",
        "#target{position:absolute;left:40px;top:30px;width:80px;height:40px;padding:4px 8px;border:2px solid #112233;transform:translateX(5px)}",
        "#target.shifted{transform:translate(31px,17px)!important}",
        "#control{position:absolute;left:220px;top:20px;width:40px;height:20px}",
        ".fresh{width:10px;height:10px}"
    ).to_owned())]);
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let location = crate::navigation::address(
        "https://translate.test/original",
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
    let c = &forms.controls[&node];
    assert_eq!(
        (
            c.value.text(),
            c.value.cursor,
            c.value.anchor,
            c.value.offset,
            c.dirty_value,
            c.owner
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
fn translate_style_object_assign_domstring_declaration_methods_and_raw_priority() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var target=document.getElementById('target'),style=target.style,attr=target.getAttributeNode('style'),conversions=0;
var bare=document.createElement('div');bare.style.transform=null;
if(bare.style.removeProperty('transform')!==''||bare.hasAttribute('style'))throw 'absent removal created an attribute';
var inline=document.createElement('span');document.body.appendChild(inline);inline.style.transform='translateX(0.1px)';
if(getComputedStyle(inline).transform!=='matrix(1, 0, 0, 1, 0.1, 0)')throw 'absolute computed translation needs no painted reference box';
Object.assign(style,{transform:{toString:function(){conversions++;return 'TrAnSlAtE(+1.5PX,-2E1px)';}}});
if(conversions!==1||style.transform!=='translate(1.5px, -20px)'||style.getPropertyValue('TRANSFORM')!==style.transform)throw 'real Object.assign must perform one DOMString conversion and typed serialization';
if(target.getAttributeNode('style')!==attr||attr.ownerElement!==target||attr.value!==target.getAttribute('style'))throw 'style Attr identity lost';
var order='',key={toString:function(){order+='k';return 'TRANSFORM';}},value={toString:function(){order+='v';return 'translate(11px, 13px)';}},priority={toString:function(){order+='p';return 'IMPORTANT';}};
style.setProperty(key,value,priority);
if(order!=='kvp'||style.transform!=='translate(11px, 13px)'||target.getAttribute('style').indexOf('transform:translate(11px, 13px) !important;')<0)throw 'declaration DOMString conversion order or raw priority';
var before=target.getAttribute('style'),marker={},caught=0;
try{style.setProperty(key,{toString:function(){order+='!';throw marker;}},priority);}catch(e){if(e!==marker)throw e;caught++;}
if(caught!==1||order!=='kvpk!'||target.getAttribute('style')!==before)throw 'abrupt value conversion must skip priority and preserve declaration';
try{style.transform={toString:function(){conversions++;throw marker;}};}catch(e){if(e!==marker)throw e;caught++;}
if(caught!==2||conversions!==2||target.getAttribute('style')!==before)throw 'direct setter abrupt conversion';
try{style.transform=Symbol('not a CSS value');throw 'Symbol value accepted';}catch(e){if(!(e instanceof TypeError))throw e;}
if(target.getAttribute('style')!==before)throw 'Symbol changed author style';
var removed=style.removeProperty({toString:function(){order+='r';return 'TRANSFORM';}});
if(order!=='kvpk!r'||removed!=='translate(11px, 13px)'||style.transform!=='')throw 'converted removal and returned specified value';
target.setAttribute('style','color:#123456;transform:translateX(7px) !important;transform:translateY(9px)');
before=target.getAttribute('style');
if(style.transform!=='translateX(7px)'||style.getPropertyValue('transform')!==style.transform||target.getAttribute('style')!==before)throw 'important specified winner must not rewrite raw duplicates';
style.color='#654321';
if(style.transform!=='translateX(7px)'||target.getAttribute('style').indexOf('transform:translateX(7px) !important;')<0)throw 'unrelated property update lost raw important declaration';
style.setProperty('transform','translateY(12px)','important');before=target.getAttribute('style');
style.setProperty('transform','translateX(99px)','urgent');
if(target.getAttribute('style')!==before||style.transform!=='translateY(12px)')throw 'invalid priority must be a no-op';
style.setProperty('transform','','urgent');
if(style.transform!==''||getComputedStyle(target).transform!=='matrix(1, 0, 0, 1, 5, 0)')throw 'empty value removes regardless of priority and exposes real sheet';
"#,
    );
}

#[test]
fn translate_style_invalid_css_is_noop_and_unsupported_values_refuse_atomically() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var target=document.getElementById('target'),style=target.style;
style.transform='translate(7px, 9px)';var raw=target.getAttribute('style');
for(var bad of ['banana','translate()','translate(1)','translate(1px,)','translate(1.px)','translate(1px 2px)']){
 style.transform=bad;style.setProperty('transform',bad,'important');
 if(style.transform!=='translate(7px, 9px)'||target.getAttribute('style')!==raw)throw 'invalid CSS changed declaration';
}
style.transform=undefined;
if(target.getAttribute('style')!==raw)throw 'undefined DOMString must be ignored as invalid CSS';
"#,
    );
    let target = id(&doc, "target");
    let raw = doc.attr(target, "style").unwrap().to_owned();
    let commits = page.dom_commit_generation();
    for operation in [
        "style.transform='rotate(10deg)';",
        "style.setProperty('transform','scale(2)');",
        "Object.assign(style,{transform:'translate3d(1px,2px,3px)'});",
        "style.transform='translate(10001px)';",
    ] {
        let source = format!("var caught=false,finished=false,after=false;try{{{operation}}}catch(e){{caught=true;}}finally{{finished=true;}}after=true;");
        let outcome = page.console_eval(&mut doc, &mut forms, &source);
        assert_eq!(outcome.errors, 1, "{operation}\n{}", page.report());
        assert!(
            page.runtime.invocation_failed_fatally(),
            "Capability refusal must remain fatal"
        );
        assert_eq!(doc.attr(target, "style"), Some(raw.as_str()));
        assert_eq!(page.dom_commit_generation(), commits);
        kept(&doc, &forms, &mut page);
        run(&mut doc, &mut forms, &mut page,
            "if(caught||finished||after||style.transform!=='translate(7px, 9px)')throw 'unsupported value escaped refusal or damaged next job';");
    }
}

#[test]
fn translate_style_live_computed_matrix_percent_reference_box_and_real_geometry() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var target=document.getElementById('target'),saved=getComputedStyle(target),read=saved.getPropertyValue;
if(saved.transform!=='matrix(1, 0, 0, 1, 5, 0)'||read.call(saved,'TRANSFORM')!==saved.transform)throw 'initial external translation matrix';
target.style.transform='translate(25%, -50%)';
var r=target.getBoundingClientRect();
if(target.clientWidth!==96||target.clientHeight!==48||r.width!==100||r.height!==52||r.x!==65||r.y!==4)throw 'percent translation must use actual border reference box';
if(target.style.transform!=='translate(25%, -50%)'||saved.transform!=='matrix(1, 0, 0, 1, 25, -26)')throw 'specified percent versus resolved live matrix';
target.style.width='180px';r=target.getBoundingClientRect();
if(r.width!==200||r.x!==90||saved.transform!=='matrix(1, 0, 0, 1, 50, -26)')throw 'retained computed declaration ignored width mutation';
target.style.height='80px';r=target.getBoundingClientRect();
if(r.height!==92||r.y!==-16||saved.transform!=='matrix(1, 0, 0, 1, 50, -46)')throw 'retained computed declaration ignored height mutation';
target.className='shifted';r=target.getBoundingClientRect();
if(saved.transform!=='matrix(1, 0, 0, 1, 31, 17)'||r.x!==71||r.y!==47)throw 'actual important stylesheet winner missing';
target.style.setProperty('transform','translateX(2px)','important');
if(saved.transform!=='matrix(1, 0, 0, 1, 2, 0)')throw 'inline important priority lost';
if(target.style.removeProperty('transform')!=='translateX(2px)'||saved.transform!=='matrix(1, 0, 0, 1, 31, 17)')throw 'removal failed to expose live sheet';
target.className='';
if(saved.transform!=='matrix(1, 0, 0, 1, 5, 0)'||getComputedStyle(document.getElementById('control')).transform!=='none')throw 'unaffected identity or stylesheet restoration';
try{saved.setProperty('transform','none');throw 'computed declaration changed';}catch(e){if(e.name!=='NoModificationAllowedError')throw e;}
try{saved.removeProperty('transform');throw 'computed declaration removed';}catch(e){if(e.name!=='NoModificationAllowedError')throw e;}
"#,
    );
}

#[test]
fn translate_style_detached_retained_declaration_reconnect_and_nullable_removal() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var target=document.getElementById('target'),parent=target.parentNode,style=target.style,saved=getComputedStyle(target);
style.transform='translate(9px, 12px)';parent.removeChild(target);
if(style.transform!=='translate(9px, 12px)'||saved.transform!==''||saved.getPropertyValue('TRANSFORM')!==''||target.ownerDocument!==document)throw 'detached raw style or empty computed declaration';
style.transform='translateX(4px)';parent.appendChild(target);
if(saved.transform!=='matrix(1, 0, 0, 1, 4, 0)'||target.getBoundingClientRect().x!==44)throw 'retained style and computed declaration did not reconnect';
style.transform=null;
if(style.transform!==''||saved.transform!=='matrix(1, 0, 0, 1, 5, 0)')throw 'LegacyNullToEmptyString direct removal';
style.transform='translateY(3px)';style.setProperty('transform',null,'urgent');
if(style.transform!==''||saved.transform!=='matrix(1, 0, 0, 1, 5, 0)')throw 'nullable declaration removal must precede priority validation';
var fresh=document.createElement('div');fresh.className='fresh';fresh.style.transform='translateX(6px)';var detached=getComputedStyle(fresh);
if(fresh.style.transform!=='translateX(6px)'||detached.transform!=='')throw 'fresh detached native style';
parent.appendChild(fresh);
if(detached.transform!=='matrix(1, 0, 0, 1, 6, 0)')throw 'fresh connected real style';
if(fresh.style.removeProperty('TRANSFORM')!=='translateX(6px)'||detached.transform!=='none')throw 'fresh removal did not return specified value';
"#,
    );
}

#[test]
fn translate_style_budgets_refuse_before_publication_and_cached_reads_do_not_refill() {
    for copy_limit in [false, true] {
        let (mut doc, mut forms, mut page) = fixture();
        let target = id(&doc, "target");
        let raw = doc.attr(target, "style").unwrap().to_owned();
        let commits = page.dom_commit_generation();
        {
            let mut bridge = Bridge {
                doc: &mut doc,
                forms: &mut forms,
                state: &mut page.state,
            };
            if copy_limit {
                bridge.state.copy_work = 0;
            } else {
                bridge.state.host_work = 0;
            }
            for _ in 0..2 {
                let error = bridge
                    .set_style(target, "transform", "translate(11px, 13px)", "")
                    .unwrap_err();
                assert!(
                    error.contains(if copy_limit {
                        "string work limit"
                    } else {
                        "DOM work limit"
                    }),
                    "{error}"
                );
                assert_eq!(bridge.doc.attr(target, "style"), Some(raw.as_str()));
                assert_eq!(bridge.state.dom_commits, commits);
                assert_eq!(
                    if copy_limit {
                        bridge.state.copy_work
                    } else {
                        bridge.state.host_work
                    },
                    0
                );
            }
        }
        run(&mut doc, &mut forms, &mut page,
            "var target=document.getElementById('target');Object.assign(target.style,{transform:'translate(11px, 13px)'});if(getComputedStyle(target).transform!=='matrix(1, 0, 0, 1, 11, 13)')throw 'real new-job recovery';");
        let commits = page.dom_commit_generation();
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
                panic!("genuine computed declaration");
            };
            assert!(
                matches!(bridge.get(handle, "transform").unwrap(), Value::String(value) if value == "matrix(1, 0, 0, 1, 11, 13)")
            );
            bridge.state.copy_work = 0;
            assert!(bridge.get(handle, "transform").is_err());
            assert!(bridge.get(handle, "transform").is_err());
            assert_eq!(bridge.state.copy_work, 0);
            assert_eq!(bridge.state.dom_commits, commits);
        }
        run(&mut doc, &mut forms, &mut page,
            "if(getComputedStyle(target).transform!=='matrix(1, 0, 0, 1, 11, 13)')throw 'cached read refusal poisoned next job';");
    }
}
