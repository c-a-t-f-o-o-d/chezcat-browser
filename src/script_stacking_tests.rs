//! Exact specified/computed z values over actual renderer snapshots.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut d=crate::html::parse("<html><head><style>#parent{z-index:20}#node{position:relative;--Z:7;z-index:var(--Z)}</style></head><body><div id=parent><div id=node></div></div></body></html>");
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
fn stacking_cssom_exact_keywords_coercion_priority_variable_case_and_readback() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('node'),style=node.style,computed=getComputedStyle(node),calls=0,obs=new MutationObserver(function(){});obs.observe(node,{attributes:true,attributeOldValue:true});if(computed.zIndex!=='7')throw 'initial z snapshot';style.zIndex={toString:function(){calls++;return '-00042';}};if(calls!==1||style.zIndex!=='-42'||computed.getPropertyValue('z-index')!=='-42'||obs.takeRecords().length!==1)throw 'exact native z conversion';var before=node.getAttribute('style');style.zIndex='1.5';style.zIndex='NaN';if(node.getAttribute('style')!==before||obs.takeRecords().length)throw 'invalid z changed state';var caught=false;try{style.zIndex=Symbol('bad');}catch(e){caught=e instanceof TypeError;}if(!caught||obs.takeRecords().length)throw 'symbol z changed state';style.zIndex=null;if(style.zIndex!==''||computed.zIndex!=='7')throw 'null z removal';style.cssText='z-index:+000000000000000000000000003;';if(style.zIndex!=='3'||computed.zIndex!=='3')throw 'whole z block';style.zIndex='var(--Z)';if(style.zIndex!=='var(--Z)'||computed.zIndex!=='7')throw 'z custom name case';
"#,
    );
    let huge = format!("1{}", "0".repeat(80));
    run(&mut d,&mut f,&mut p,&format!("style.zIndex='{huge}';if(style.zIndex!=='{huge}'||computed.zIndex!=='{huge}')throw 'large exact z';"));
}
#[test]
fn stacking_cssom_inherit_revert_layer_invalid_variable_and_readonly_state() {
    let (mut d, mut f, mut p) = fixture();
    run(&mut d,&mut f,&mut p,"var node=document.getElementById('node'),style=node.style,computed=getComputedStyle(node);style.zIndex='inherit';if(computed.zIndex!=='20')throw 'explicit z inheritance';style.zIndex='revert-layer';if(computed.zIndex!=='7')throw 'z sheet checkpoint';style.zIndex='var(--missing)';if(computed.zIndex!=='auto')throw 'z invalid computed initial';");
    let out = p.console_eval(&mut d, &mut f, "computed.zIndex='99';");
    assert_eq!(out.errors, 1);
    run(
        &mut d,
        &mut f,
        &mut p,
        "if(computed.zIndex!=='auto')throw 'computed z changed';",
    );
}
#[test]
fn stacking_cssom_retained_refusal_preserves_style_and_observation_then_recovers() {
    let (mut d, mut f, mut p) = fixture();
    run(&mut d,&mut f,&mut p,"var node=document.getElementById('node'),before=node.getAttribute('style'),obs=new MutationObserver(function(){});obs.observe(node,{attributes:true,attributeOldValue:true});");
    let limit = p.state.retained_limit;
    p.state.retained_limit = p.state.retained;
    let generation = p.state.dom_commits;
    let out = p.console_eval(&mut d, &mut f, "node.style.zIndex='99';");
    assert_eq!(out.errors, 1);
    assert_eq!(p.state.dom_commits, generation);
    p.state.retained_limit = limit;
    run(&mut d,&mut f,&mut p,"if(node.getAttribute('style')!==before||obs.takeRecords().length)throw 'z refusal leaked';node.style.zIndex='99';if(node.style.zIndex!=='99'||obs.takeRecords().length!==1)throw 'z recovery';");
}
