//! Genuine text-control hints are separate from native editing/default data.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut d=crate::html::parse("<html><body><input id=input name=q value=seed><textarea id=area name=note>default</textarea></body></html>");
    d.scripting_enabled = true;
    let mut f = Forms::new(&d);
    let mut p = Page::with_limits(crate::javascript::limits::BROWSER);
    assert_eq!(p.start(&mut d, &mut f, &[]).errors, 0);
    (d, f, p)
}
fn run(d: &mut Document, f: &mut Forms, p: &mut Page, s: &str) {
    let out = p.console_eval(d, f, s);
    assert_eq!(out.errors, 0, "{}", p.report());
}
#[test]
fn native_placeholder_fresh_probe_and_edited_controls_keep_real_data_independent() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var probe=document.createElement('textarea');probe.placeholder='a';if(probe.placeholder!=='a'||probe.textContent!==''||probe.getAttribute('placeholder')!=='a'||probe.placeholder===probe.textContent)throw 'genuine placeholder probe';
var input=document.getElementById('input'),area=document.getElementById('area');input.value='edited input';area.value='edited area';input.placeholder='Input hint';area.placeholder='Area hint';if(input.placeholder!=='Input hint'||area.placeholder!=='Area hint'||input.value!=='edited input'||area.value!=='edited area'||input.defaultValue!=='seed'||area.textContent!=='default')throw 'placeholder changed actual control data';
var attr=area.getAttributeNode('placeholder');area.setAttribute('placeholder','attribute hint');if(attr.value!=='attribute hint'||area.placeholder!=='attribute hint')throw 'real placeholder Attr alias';area.removeAttribute('placeholder');if(area.placeholder!==''||attr.ownerElement!==null||area.value!=='edited area')throw 'placeholder removal';
"#,
    );
}
#[test]
fn native_placeholder_canonical_brands_conversion_observation_and_inert_copy_are_real() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var input=document.getElementById('input'),area=document.getElementById('area'),I=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'placeholder'),T=Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'placeholder');if(!I.enumerable||!I.configurable||I.get.length!==0||I.set.length!==1||!T.get||!T.set)throw 'canonical placeholder descriptors';var calls=0,failed=false;try{T.set.call(input,{toString:function(){calls++;return 'bad';}});}catch(e){failed=e instanceof TypeError;}if(!failed||calls)throw 'placeholder brand before conversion';T.set.call(area,{toString:function(){calls++;return 'native hint';}});if(calls!==1||T.get.call(area)!=='native hint')throw 'placeholder conversion/real getter';input.placeholder=null;if(input.placeholder!=='null')throw 'DOMString placeholder';
var owner=document.implementation.createHTMLDocument('inert'),node=owner.createElement('textarea'),obs=new MutationObserver(function(){throw 'drained hint records';});obs.observe(node,{attributes:true,attributeOldValue:true});node.placeholder='inert hint';node.placeholder='updated';var copy=document.importNode(node,true),clone=node.cloneNode(true),records=obs.takeRecords();if(records.length!==2||records[0].target!==node||records[0].attributeName!=='placeholder'||records[0].oldValue!==null||records[1].oldValue!=='inert hint'||copy.placeholder!=='updated'||copy.ownerDocument!==document||clone.placeholder!=='updated'||clone.ownerDocument!==owner||node.textContent!=='')throw 'real inert placeholder ownership and records';
"#,
    );
}

#[test]
fn native_placeholder_retained_refusal_and_symbol_conversion_keep_editors_and_records() {
    let (mut d, mut f, mut p) = fixture();
    run(&mut d,&mut f,&mut p,"var area=document.getElementById('area'),input=document.getElementById('input'),obs=new MutationObserver(function(){throw 'drained hint records';});area.value='edited area';input.value='edited input';area.placeholder='kept';obs.observe(area,{attributes:true,attributeOldValue:true});var failed=false;try{area.placeholder=Symbol('hint');}catch(e){failed=e instanceof TypeError;}if(!failed||area.placeholder!=='kept'||obs.takeRecords().length)throw 'symbol placeholder conversion';");
    let limit = p.state.retained_limit;
    p.state.retained_limit = p.state.retained;
    let generation = p.state.dom_commits;
    let source = format!("area.placeholder='{}';", "Original hint ".repeat(80));
    let out = p.console_eval(&mut d, &mut f, &source);
    assert_eq!(out.errors, 1);
    assert_eq!(p.state.dom_commits, generation);
    p.state.retained_limit = limit;
    run(&mut d,&mut f,&mut p,"if(area.placeholder!=='kept'||area.value!=='edited area'||input.value!=='edited input'||obs.takeRecords().length)throw 'hint admission changed editor or record';area.placeholder='recovered';var records=obs.takeRecords();if(records.length!==1||records[0].attributeName!=='placeholder'||records[0].oldValue!=='kept'||area.placeholder!=='recovered')throw 'genuine placeholder recovery';");
}
