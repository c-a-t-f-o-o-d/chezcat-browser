//! Presentation readback must come from the live rendering cascade.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut d=crate::html::parse("<html><head><style>#parent{color:#123456;--layout:flex}.changed{color:#40a020!important}#node{display:var(--layout)}</style></head><body><div id=parent><div id=node>Actual</div></div></body></html>");
    d.scripting_enabled = true;
    let mut f = Forms::new(&d);
    let mut p = Page::with_limits(crate::javascript::limits::BROWSER);
    p.set_geometry(crate::script_geometry::Geometry::new(
        640,
        480,
        1.0,
        &HashMap::new(),
        HashMap::new(),
        &crate::fonts::Book::empty(),
    ));
    assert_eq!(p.start(&mut d, &mut f, &[]).errors, 0);
    (d, f, p)
}
fn run(d: &mut Document, f: &mut Forms, p: &mut Page, s: &str) {
    let out = p.console_eval(d, f, s);
    assert_eq!(out.errors, 0, "{}", p.report());
}
#[test]
fn computed_presentation_live_display_and_color_match_actual_cascade_and_detachment() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var parent=document.getElementById('parent'),node=document.getElementById('node'),style=getComputedStyle(node);if(style===getComputedStyle(node)||style.display!=='flex'||style.color!=='rgb(18, 52, 86)'||style.getPropertyValue('DISPLAY')!=='flex'||style.getPropertyValue('color')!==style.color)throw 'real inherited presentation';parent.className='changed';parent.style.setProperty('--layout','none');if(style.color!=='rgb(64, 160, 32)'||style.display!=='none')throw 'live cascade presentation';node.style.color='#e03040';node.style.display='block';if(style.color!=='rgb(224, 48, 64)'||style.display!=='block')throw 'actual inline presentation';parent.removeChild(node);if(style.display!==''||style.color!=='')throw 'detached presentation fabricated cascade';parent.appendChild(node);if(style.display!=='block'||style.color!=='rgb(224, 48, 64)')throw 'reattached presentation';
"#,
    );
}
#[test]
fn computed_presentation_scalar_serialization_pays_exact_copy_and_failed_recompute_does_not_reset_limits(
) {
    let (mut d, mut f, mut p) = fixture();
    run(&mut d,&mut f,&mut p,"var node=document.getElementById('node'),style=getComputedStyle(node);if(style.color!=='rgb(18, 52, 86)'||style.display!=='flex')throw 'warm actual presentation';");
    let Value::Host(handle) = p
        .runtime
        .execute(
            "style",
            &mut Bridge {
                doc: &mut d,
                forms: &mut f,
                state: &mut p.state,
            },
        )
        .unwrap()
    else {
        panic!("computed declaration")
    };
    p.begin_job();
    let mut bridge = Bridge {
        doc: &mut d,
        forms: &mut f,
        state: &mut p.state,
    };
    bridge.state.copy_work = "rgb(18, 52, 86)".len() - 1;
    assert!(bridge.get(handle, "color").is_err());
    assert_eq!(bridge.state.copy_work, 0);
    bridge.state.copy_work = "rgb(18, 52, 86)".len();
    assert!(matches!(bridge.get(handle,"color").unwrap(),Value::String(s) if s=="rgb(18, 52, 86)"));
    assert_eq!(bridge.state.copy_work, 0);
    bridge.state.host_work = 0;
    bridge.state.geometry.as_mut().unwrap().invalidate();
    assert!(bridge.get(handle, "display").is_err());
    assert_eq!(bridge.state.host_work, 0);
}

#[test]
fn computed_presentation_retains_supported_list_and_table_group_keywords() {
    let (mut d, mut f, mut p) = fixture();
    run(&mut d,&mut f,&mut p,"var li=document.createElement('li');document.body.appendChild(li);if(getComputedStyle(li).display!=='list-item')throw 'real UA list display';li.style.display='block';if(getComputedStyle(li).display!=='block')throw 'explicit block lost UA override';var group=document.createElement('div');document.body.appendChild(group);group.style.display='table-header-group';var style=getComputedStyle(group);if(style.display!=='table-header-group')throw 'table header display alias';group.style.display='table-footer-group';if(style.display!=='table-footer-group')throw 'live table footer alias';");
}
