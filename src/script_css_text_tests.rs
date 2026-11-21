//! Whole native declaration replacement, independent of App IO.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut d=crate::html::parse("<html><body><div id=mount style='color:#123456'></div><input id=neighbor value=seed></body></html>");
    d.scripting_enabled = true;
    let mut f = Forms::new(&d);
    let mut p = Page::with_limits(crate::javascript::limits::BROWSER);
    assert_eq!(p.start(&mut d, &mut f, &[]).errors, 0);
    (d, f, p)
}
fn run(d: &mut Document, f: &mut Forms, p: &mut Page, s: &str) {
    let o = p.console_eval(d, f, s);
    assert_eq!(o.errors, 0, "{}", p.report());
}
#[test]
fn css_text_replaces_real_declarations_priorities_and_preserves_native_handles() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('mount'),style=node.style,attr=node.getAttributeNode('style');
if(!('cssText' in style)||typeof style.cssText!=='string')throw 'real cssText member';
style.cssText='width: 80px; color: red !important; color: blue; height: 12px; nonsense: yes; width: invalid;';
if(node.style!==style||node.getAttributeNode('style')!==attr||style.width!=='80px'||style.height!=='12px'||style.color!=='red'||style.cssText.indexOf('!important')<0||style.cssText.indexOf('nonsense')>=0||style.cssText.indexOf('invalid')>=0)throw 'whole declaration parsing and identity';
style.cssText='--size: 25vw; width: calc(var(--size) + 2px);';
if(style.height!==''||style.color!==''||style.getPropertyValue('--size')!=='25vw'||style.width!=='calc(var(--size) + 2px)')throw 'replace versus append and custom tokens';
style.cssText=null;if(style.cssText!==''||node.getAttribute('style')!==''||node.getAttributeNode('style')!==attr)throw 'null clears real attribute';
"#,
    );
}
#[test]
fn css_text_forwarding_coerces_once_and_observes_actual_old_attribute() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('mount'),kept=node.style,old=node.getAttribute('style'),calls=0;
var obs=new MutationObserver(function(){});obs.observe(node,{attributes:true,attributeOldValue:true});
var descriptor=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'style');
descriptor.set.call(node,{toString:function(){calls++;return 'width: 60px; color: #20a040;';}});
var records=obs.takeRecords();if(calls!==1||node.style!==kept||kept.width!=='60px'||records.length!==1||records[0].target!==node||records[0].oldValue!==old||records[0].attributeName!=='style')throw 'native forwarding and observer';
var threw=false,before=kept.cssText;try{kept.cssText=Symbol('bad');}catch(e){threw=e instanceof TypeError;}if(!threw||kept.cssText!==before||obs.takeRecords().length)throw 'symbol conversion is atomic';
node.style='height: 18px;';if(node.style!==kept||kept.height!=='18px'||kept.width!=='')throw 'ordinary PutForwards';
"#,
    );
}
#[test]
fn css_text_retained_refusal_preserves_whole_block_and_observer_queue() {
    let (mut d, mut f, mut p) = fixture();
    run(&mut d,&mut f,&mut p,"var node=document.getElementById('mount'),before=node.getAttribute('style'),obs=new MutationObserver(function(){});obs.observe(node,{attributes:true,attributeOldValue:true});");
    let limit = p.state.retained_limit;
    p.state.retained_limit = p.state.retained;
    let generation = p.state.dom_commits;
    let out = p.console_eval(
        &mut d,
        &mut f,
        "node.style.cssText='width:80px;color:#40a020;height:30px';",
    );
    assert_eq!(out.errors, 1);
    assert_eq!(p.state.dom_commits, generation);
    p.state.retained_limit = limit;
    run(&mut d,&mut f,&mut p,"if(node.getAttribute('style')!==before||obs.takeRecords().length)throw 'cssText refusal leaked';node.style.cssText='width:80px;';if(node.style.width!=='80px'||obs.takeRecords().length!==1)throw 'cssText recovery';");
}

#[test]
fn css_text_shorthand_overlap_keeps_cascade_order_and_coercion_reads_latest_attribute() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('mount'),style=node.style,obs=new MutationObserver(function(){});obs.observe(node,{attributes:true,attributeOldValue:true});
style.cssText='margin-left:1px; margin:10px; margin-left:2px; gap:4px 6px;';
if(style.cssText.indexOf('margin:')>style.cssText.indexOf('margin-left:')||style.marginLeft!=='2px'||style.gap!=='4px 6px')throw 'overlapping shorthand order';
obs.takeRecords();var count=0;
style.cssText={toString:function(){count++;node.setAttribute('style','width:17px');return 'width:42px';}};
var records=obs.takeRecords();if(count!==1||records.length!==2||records[1].oldValue!=='width:17px'||style.width!=='42px')throw 'conversion happens before mutation preflight';
"#,
    );
}

#[test]
fn css_text_preserves_precise_metadata_urls_and_atomic_resource_refusal() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var node=document.getElementById('mount'),style=node.style,obs=new MutationObserver(function(){});obs.observe(node,{attributes:true,attributeOldValue:true});
style.cssText='animation-timing-function:cubic-bezier(.1234564,0,1,1); background-image:url("Kept/Original.PPM?A;B!C");';
if(node.getAttribute('style').indexOf('0.1234564')<0||style.animationTimingFunction!=='cubic-bezier(0.123456, 0, 1, 1)'||style.cssText.indexOf('0.1234564')>=0||style.backgroundImage.indexOf('Kept/Original.PPM?A;B!C')<0)throw 'precise stored value and real CSSOM serialization';
obs.takeRecords();var before=node.getAttribute('style');
"#,
    );
    let refused = p.console_eval(
        &mut d,
        &mut f,
        "style.cssText='width:30px;background-image:url(a),url(b);color:red;';",
    );
    assert_eq!(refused.errors, 1);
    run(&mut d,&mut f,&mut p,"if(node.getAttribute('style')!==before||obs.takeRecords().length)throw 'unsupported resources partially replaced block';");
}
