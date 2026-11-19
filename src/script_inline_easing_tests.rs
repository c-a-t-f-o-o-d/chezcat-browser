//! Specified easing declarations do not create an animation or renderer clock.
use super::*;

fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<!doctype html><title>Original inline easing</title><body><div id=tile style='color:red;animation-timing-function:EASE-IN; width:24px;animation-timing-function:wrong'></div><form id=f><input id=neighbor name=keep value=seed></form>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}

#[test]
fn inline_easing_real_detached_probe_and_live_canonical_declaration() {
    let (mut doc, mut forms, mut page) = fixture();
    let outcome = page.console_eval(&mut doc, &mut forms, r#"
var el=document.getElementById('tile'),s=el.style,fresh=document.createElement('div'),empty=fresh.style;
if(s.color!=='red'||s.width!=='24px'||el.ownerDocument!==document||fresh.parentNode!==null)throw 'existing style and owner controls';
empty.animationTimingFunction='';
empty.animationTimingFunction='linear';
if(empty.animationTimingFunction!=='linear'||empty.getPropertyValue('ANIMATION-TIMING-FUNCTION')!=='linear'||fresh.getAttribute('style')!=='animation-timing-function:linear;')throw 'real detached easing probe';
if(s.animationTimingFunction!=='ease-in'||s.length!==3||s[1]!=='animation-timing-function')throw 'parsed valid winner, invalid trailing declaration and index';
s.animationTimingFunction='CUBIC-BEZIER(.25,+.1, .25, 1)';
if(s.animationTimingFunction!=='cubic-bezier(0.25, 0.1, 0.25, 1)'||s.color!=='red'||s.width!=='24px'||s.length!==3||s[1]!=='animation-timing-function')throw 'canonical replacement preserves neighbors and index';
s.setProperty('animation-timing-function','steps(4,end), EASE-OUT','important');
if(s.animationTimingFunction!=='steps(4), ease-out'||s.getPropertyValue('animation-timing-function')!=='steps(4), ease-out')throw 'canonical list and priority';
if(s.removeProperty('animation-timing-function')!=='steps(4), ease-out'||s.animationTimingFunction!==''||s.length!==2||s[0]!=='color'||s[1]!=='width')throw 'remove returns actual prior declaration';
s.animationTimingFunction='step-start';
if(s.animationTimingFunction!=='steps(1, start)'||s[2]!=='animation-timing-function')throw 'step keyword canonical serialization';
var keys=[];for(var key in empty)keys.push(key);
if(keys.indexOf('animationTimingFunction')<0||keys.indexOf('animation-timing-function')<0||!('animationTimingFunction' in empty)||Object.hasOwn(empty,'animationTimingFunction')||Object.keys(empty).join(',')!=='0')throw 'honest inherited supported member and own declaration index';
empty.setProperty('animation-timing-function','', 'not-a-priority');
if(empty.animationTimingFunction!==''||empty.length!==0)throw 'empty removal ignores irrelevant priority';
empty.animationTimingFunction='ease';empty.animationTimingFunction=null;
if(empty.animationTimingFunction!==''||empty.length!==0)throw 'legacy null assignment removes the declaration';
empty.animationTimingFunction='linear';empty.animationTimingFunction=undefined;
if(empty.animationTimingFunction!=='linear')throw 'undefined converts to an invalid CSS value, preserving the prior declaration';
if(typeof el.animate!=='undefined')throw 'inline metadata must not manufacture an animation API';
"#);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
}

#[test]
fn inline_easing_invalid_values_conversion_and_native_neighbors_are_atomic() {
    let (mut doc, mut forms, mut page) = fixture();
    let neighbor = (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("neighbor")).unwrap();
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    { let e = &mut forms.controls.get_mut(&neighbor).unwrap().value; e.cursor = 5; e.anchor = 2; e.offset = 1; }
    let e = &forms.controls[&neighbor];
    let kept = (e.value.text(), e.value.cursor, e.value.anchor, e.value.offset, e.dirty_value, e.owner);
    let outcome = page.console_eval(&mut doc, &mut forms, r#"
var el=document.getElementById('tile'),s=el.style;s.animationTimingFunction='linear';var raw=el.getAttribute('style');
for(var bad of ['not-easing','steps(0)','steps(1,jump-none)','cubic-bezier(2,0,1,1)','linear,unknown','linear(0,1)','var(--easing)']){
  s.animationTimingFunction=bad;
  if(s.animationTimingFunction!=='linear'||el.getAttribute('style')!==raw)throw 'invalid value changed prior declaration';
}
s.setProperty('animation-timing-function','ease','invalid-priority');
if(el.getAttribute('style')!==raw)throw 'invalid priority published';
var calls=0;s.animationTimingFunction={toString:function(){calls++;return ' EASE ';}};
if(calls!==1||s.animationTimingFunction!=='ease')throw 'DOMString exactly once';
var sentinel={},caught=false;raw=el.getAttribute('style');
try{s.animationTimingFunction={toString:function(){throw sentinel;}};}catch(e){caught=e===sentinel;}
if(!caught||el.getAttribute('style')!==raw||document.getElementById('neighbor').value!=='edited neighbor'||document.getElementById('neighbor').defaultValue!=='seed')throw 'author exception/native neighbor';
"#);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let e = &forms.controls[&neighbor];
    assert_eq!((e.value.text(), e.value.cursor, e.value.anchor, e.value.offset, e.dirty_value, e.owner), kept);
    assert_eq!(doc.attr(neighbor, "value"), Some("seed"));
}

#[test]
fn inline_easing_existing_policy_refusals_leave_actual_page_attributes_unchanged() {
    // Host capability refusals are escaped faults, not catchable author throws.
    // Keep these normal Page entry witnesses separate from CSS invalid-value
    // no-ops and genuine catchable DOMString author exceptions above.
    for bad in ["linear !important", "linear; width:0px", "url(https://unavailable.invalid/x)", "linear\\x"] {
        let (mut doc, mut forms, mut page) = fixture();
        let node = (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("tile")).unwrap();
        let raw = doc.attr(node, "style").unwrap().to_owned();
        let commits = page.state.dom_commits;
        let outcome = page.console_eval(&mut doc, &mut forms, &format!("document.getElementById('tile').style.animationTimingFunction={bad:?};"));
        assert_eq!(outcome.errors, 1, "{bad}: {}", page.report());
        assert_eq!(doc.attr(node, "style"), Some(raw.as_str()));
        assert_eq!(page.state.dom_commits, commits);
    }
}

#[test]
fn inline_easing_css_spelling_whitespace_and_noop_preserve_real_raw_declarations() {
    let mut doc = crate::html::parse(concat!(
        "<!doctype html><title>Raw easing</title><body>",
        "<div id=nbsp style='ANIMATION-TIMING-FUNCTION:ease;animation-timing-function:linear&#160;;color:red'></div>",
        "<div id=vt style='animation-timing-function:ease-in;animation-timing-function:linear&#11;;width:12px'></div>",
        "<div id=camel style='animationTimingFunction:linear;color:red'></div>",
        "<div id=leading style='animation-timing-function:ease; &#11;animation-timing-function:linear'></div>",
        "<div id=same style='/* keep */ animation-timing-function: ease ; color:red'></div>"
    ));
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    let prefix = page.console_eval(&mut doc, &mut forms, r#"
var nbsp=document.getElementById('nbsp'),vt=document.getElementById('vt'),camel=document.getElementById('camel');
if(nbsp.style.animationTimingFunction!=='ease'||nbsp.style.length!==2||nbsp.style[0]!=='animation-timing-function'||nbsp.style[1]!=='color')throw 'NBSP must not hide the earlier valid winner or index';
if(vt.style.animationTimingFunction!=='ease-in'||vt.style.length!==2||vt.style[0]!=='animation-timing-function')throw 'vertical tab is not CSS whitespace';
if(camel.style.animationTimingFunction!==''||camel.style.length!==1||camel.style[0]!=='color')throw 'raw camelCase is not a CSS property';
var leading=document.getElementById('leading').style;
if(leading.animationTimingFunction!=='ease'||leading.length!==1||leading[0]!=='animation-timing-function')throw 'vertical tab must not become declaration-name trivia';
nbsp.style.color='blue';
if(nbsp.style.color!=='blue'||nbsp.style.animationTimingFunction!=='ease'||nbsp.getAttribute('style').indexOf('ANIMATION-TIMING-FUNCTION:ease')<0)throw 'uppercase parsed easing must permit unrelated checked writes';
nbsp.setAttribute('style','ANIMATION-TIMING-FUNCTION: EASE-IN; color:green');
if(nbsp.style.animationTimingFunction!=='ease-in'||nbsp.style.color!=='green'||nbsp.style[0]!=='animation-timing-function')throw 'setAttribute accepts exact ASCII-insensitive CSS spelling';
var old=vt.getAttribute('style');
vt.style.animationTimingFunction='linear\u00a0';
if(vt.getAttribute('style')!==old||vt.style.animationTimingFunction!=='ease-in')throw 'IDL NBSP is invalid and must preserve the prior declaration';
"#);
    assert_eq!(prefix.errors, 0, "{}", page.report());
    let vt = (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("vt")).unwrap();
    let raw = doc.attr(vt, "style").unwrap().to_owned();
    let commits = page.state.dom_commits;
    // Vertical tab also hits the established generic control-character policy.
    // Its escaped fault is distinct from the invalid-but-safe NBSP CSS value.
    let refused = page.console_eval(&mut doc, &mut forms, "vt.style.animationTimingFunction='linear\\u000b';");
    assert_eq!(refused.errors, 1, "{}", page.report());
    assert_eq!(doc.attr(vt, "style"), Some(raw.as_str()));
    assert_eq!(page.state.dom_commits, commits);
    let same = (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("same")).unwrap();
    let raw = doc.attr(same, "style").unwrap().to_owned();
    let commits = page.state.dom_commits;
    let noop = page.console_eval(&mut doc, &mut forms, "var same=document.getElementById('same');same.style.animationTimingFunction='EASE';if(same.style.animationTimingFunction!=='ease')throw 'same canonical readback';");
    assert_eq!(noop.errors, 0, "{}", page.report());
    assert!(!noop.layout_changed);
    assert_eq!(doc.attr(same, "style"), Some(raw.as_str()));
    assert_eq!(page.state.dom_commits, commits);
    let changed = page.console_eval(&mut doc, &mut forms, "same.style.setProperty('animation-timing-function','ease','important');if(same.getAttribute('style').indexOf('!important')<0||same.style.color!=='red')throw 'changed priority must write';");
    assert_eq!(changed.errors, 0, "{}", page.report());
    assert!(page.state.dom_commits > commits);
    let raw = doc.attr(same, "style").unwrap().to_owned();
    let commits = page.state.dom_commits;
    let noop = page.console_eval(&mut doc, &mut forms, "same.style.setProperty('animation-timing-function',' EASE ','IMPORTANT');");
    assert_eq!(noop.errors, 0, "{}", page.report());
    assert!(!noop.layout_changed);
    assert_eq!(doc.attr(same, "style"), Some(raw.as_str()));
    assert_eq!(page.state.dom_commits, commits);
    let refused = page.console_eval(&mut doc, &mut forms, "same.setAttribute('style','animationTimingFunction:linear; color:blue');");
    assert_eq!(refused.errors, 1);
    assert_eq!(doc.attr(same, "style"), Some(raw.as_str()));
    assert_eq!(page.state.dom_commits, commits);
}

#[test]
fn inline_easing_copy_or_work_refusal_preserves_raw_and_commit_identity() {
    for copies in [false, true] {
        let (mut doc, mut forms, mut page) = fixture();
        let node = (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("tile")).unwrap();
        let raw = doc.attr(node, "style").unwrap().to_owned();
        let commits = page.state.dom_commits;
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        if copies { bridge.state.copy_work = 0; } else { bridge.state.host_work = 0; }
        assert!(bridge.set_style(node, "animation-timing-function", "cubic-bezier(0, 1e100, 1, -1e100)", "").is_err());
        assert_eq!(bridge.doc.attr(node, "style"), Some(raw.as_str()));
        assert_eq!(bridge.state.dom_commits, commits);
    }
}

#[test]
fn inline_easing_empty_removal_prepays_each_candidate_and_refuses_atomically() {
    let (mut doc, mut forms, mut page) = fixture();
    let node = (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("tile")).unwrap();
    let raw = doc.attr(node, "style").unwrap().to_owned();
    let commits = page.state.dom_commits;
    let normal_work = page.state.host_work;
    let copies = page.state.copy_work;
    let limit = page.state.limits.host.string_bytes;
    page.state.host_work = 0;
    assert!(script_inline_style::replace(&raw, "animation-timing-function", "", "", limit, &mut page.state).is_err());
    assert_eq!(page.state.copy_work, copies, "pure validation does not allocate before its debit");
    assert_eq!(doc.attr(node, "style"), Some(raw.as_str()));
    assert_eq!(page.state.dom_commits, commits);
    {
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        // Enough for ordinary scans, one unit short of the actual candidate's
        // validation: the checked removal must stop before DOM publication.
        bridge.state.host_work = 1 + raw.len() * 2 + crate::css::easing_source_cost("EASE-IN".len()).0 - 1;
        assert!(bridge.set_style(node, "animation-timing-function", "", "").is_err());
        assert_eq!(bridge.state.host_work, 0);
        assert_eq!(bridge.doc.attr(node, "style"), Some(raw.as_str()));
        assert_eq!(bridge.state.dom_commits, commits);
        bridge.state.host_work = normal_work;
        bridge.set_style(node, "animation-timing-function", "", "").unwrap();
        assert!(!bridge.doc.attr(node, "style").unwrap().contains("animation-timing-function"));
        assert!(bridge.state.dom_commits > commits);
    }
    let unrelated = "width:24px;".repeat(100);
    page.state.host_work = unrelated.len() * 5 + 10;
    assert!(script_inline_style::easing_declaration(&unrelated, &mut page.state, script_inline_style::EasingText::Cssom).unwrap().is_none());
    assert!(page.state.host_work > 0, "unrelated declarations pay only the ordinary linear scan");
}

#[test]
fn inline_easing_noop_compares_precise_specified_numbers_not_rounded_readback() {
    let mut doc = crate::html::parse("<!doctype html><body><div id=precise style='animation-timing-function:cubic-bezier(.1234561,0,1,1);color:red'></div>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    let node = (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("precise")).unwrap();
    let raw = doc.attr(node, "style").unwrap().to_owned();
    let commits = page.state.dom_commits;
    let changed = page.console_eval(&mut doc, &mut forms, r#"
var precise=document.getElementById('precise'),view=precise.style.animationTimingFunction;
if(view!=='cubic-bezier(0.123456, 0, 1, 1)')throw 'ordinary CSSOM presentation';
precise.style.animationTimingFunction='cubic-bezier(.1234564,0,1,1)';
if(precise.style.animationTimingFunction!==view||precise.style.color!=='red')throw 'presentation collision is not specified equality';
if(precise.getAttribute('style').indexOf('0.1234564')<0)throw 'new precise number must survive in actual storage';
"#);
    assert_eq!(changed.errors, 0, "{}", page.report());
    assert_ne!(doc.attr(node, "style"), Some(raw.as_str()));
    assert!(page.state.dom_commits > commits);
    let raw = doc.attr(node, "style").unwrap().to_owned();
    let commits = page.state.dom_commits;
    let noop = page.console_eval(&mut doc, &mut forms, "precise.style.animationTimingFunction='CUBIC-BEZIER(+.12345640, 0.0, 1e0, 1)';");
    assert_eq!(noop.errors, 0, "{}", page.report());
    assert!(!noop.layout_changed);
    assert_eq!(doc.attr(node, "style"), Some(raw.as_str()));
    assert_eq!(page.state.dom_commits, commits);
    let changed = page.console_eval(&mut doc, &mut forms, "precise.style.animationTimingFunction='cubic-bezier(.1234561,0,1,1)';if(precise.getAttribute('style').indexOf('0.1234561')<0||precise.style.animationTimingFunction!==view)throw 'return to distinct old precise number';");
    assert_eq!(changed.errors, 0, "{}", page.report());
    assert_ne!(doc.attr(node, "style"), Some(raw.as_str()));
    assert!(page.state.dom_commits > commits);
}
