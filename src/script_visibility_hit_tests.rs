//! Actual specified/computed inherited state with native CSSOM transactions.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut d=crate::html::parse("<html><head><style>#parent{visibility:hidden;pointer-events:none}#node{--V:visible;--P:auto;visibility:var(--V);pointer-events:var(--P)}</style></head><body><div id=parent><div id=node></div></div></body></html>");
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
fn visibility_hit_cssom_converts_once_preserves_variable_case_and_observes_real_attributes() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('node'),style=node.style,computed=getComputedStyle(node),obs=new MutationObserver(function(){}),calls=0;obs.observe(node,{attributes:true,attributeOldValue:true});if(computed.visibility!=='visible'||computed.pointerEvents!=='auto')throw 'real variable state';
style.visibility={toString:function(){calls++;style.pointerEvents='NoNe';return 'HiDdEn';}};var records=obs.takeRecords();if(calls!==1||style.visibility!=='hidden'||style.pointerEvents!=='none'||computed.visibility!=='hidden'||records.length!==2||records[1].oldValue.indexOf('pointer-events:none')<0)throw 'native conversion and real old style';
style.visibility='bad value';style.pointerEvents='visiblePainted';if(obs.takeRecords().length||style.visibility!=='hidden'||style.pointerEvents!=='none')throw 'invalid finite values changed state';var caught=false;try{style.visibility=Symbol('bad');}catch(e){caught=e instanceof TypeError;}if(!caught||obs.takeRecords().length)throw 'symbol visibility is atomic';
style.visibility=null;if(style.visibility!==''||computed.visibility!=='visible')throw 'null removal';style.cssText='visibility:HiDdEn;pointer-events:AuTo;';if(style.visibility!=='hidden'||style.pointerEvents!=='auto'||computed.getPropertyValue('visibility')!=='hidden'||computed.getPropertyValue('pointer-events')!=='auto')throw 'whole declaration keywords';
style.visibility='var(--V)';style.pointerEvents='var(--P)';if(style.visibility!=='var(--V)'||style.pointerEvents!=='var(--P)'||computed.visibility!=='visible'||computed.pointerEvents!=='auto')throw 'case-sensitive custom names';
"#,
    );
}
#[test]
fn visibility_hit_inline_revert_layer_invalid_variables_and_readonly_computed_state_are_real() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('node'),style=node.style,computed=getComputedStyle(node);style.visibility='hidden';style.visibility='revert-layer';style.pointerEvents='none';style.pointerEvents='revert-layer';if(computed.visibility!=='visible'||computed.pointerEvents!=='auto')throw 'stylesheet checkpoint';style.visibility='var(--missing)';style.pointerEvents='var(--missing)';if(computed.visibility!=='hidden'||computed.pointerEvents!=='none')throw 'invalid inherited computed fallback';
"#,
    );
    let out = p.console_eval(&mut d, &mut f, "computed.visibility='visible';");
    assert_eq!(out.errors, 1);
    run(&mut d,&mut f,&mut p,"if(computed.visibility!=='hidden'||computed.pointerEvents!=='none')throw 'computed write changed real state';");
}
#[test]
fn visibility_hit_retained_refusal_keeps_style_and_pending_records_and_recovers() {
    let (mut d, mut f, mut p) = fixture();
    run(&mut d,&mut f,&mut p,"var node=document.getElementById('node'),before=node.getAttribute('style'),obs=new MutationObserver(function(){});obs.observe(node,{attributes:true,attributeOldValue:true});");
    let limit = p.state.retained_limit;
    p.state.retained_limit = p.state.retained;
    let generation = p.state.dom_commits;
    let out = p.console_eval(&mut d, &mut f, "node.style.visibility='hidden';");
    assert_eq!(out.errors, 1);
    assert_eq!(p.state.dom_commits, generation);
    p.state.retained_limit = limit;
    run(&mut d,&mut f,&mut p,"if(node.getAttribute('style')!==before||obs.takeRecords().length)throw 'visibility refusal leaked';node.style.visibility='hidden';if(node.style.visibility!=='hidden'||obs.takeRecords().length!==1)throw 'visibility recovery';");
}
