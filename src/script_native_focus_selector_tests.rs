//! Host focus state is shared by CSS/queries and cannot be set by synthetic events.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut d = crate::html::parse(
        "<html><body><div id=wrap><input id=a value=seed></div><input id=b></body></html>",
    );
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
fn native_focus_selectors_share_host_transition_and_ignore_synthetic_focus_events() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var a=document.getElementById('a'),b=document.getElementById('b'),wrap=document.getElementById('wrap'),seen=0,obs=new MutationObserver(function(){});obs.observe(document.body,{attributes:true,subtree:true});
a.addEventListener('focusin',function(e){if(e.isTrusted){seen++;if(!a.matches(':focus')||!wrap.matches(':focus-within')||!a.matches(':focus:focus-within')||wrap.matches(':focus:focus-within')||b.matches(':focus'))throw 'committed native focus state';}});
"#,
    );
    let a = (0..d.nodes.len())
        .find(|&n| d.attr(n, "id") == Some("a"))
        .unwrap();
    let b = (0..d.nodes.len())
        .find(|&n| d.attr(n, "id") == Some("b"))
        .unwrap();
    let generation = p.state.dom_commits;
    p.begin_job();
    assert_eq!(
        p.focus_transition(&mut d, &mut f, None, Some(a)).errors,
        0,
        "{}",
        p.report()
    );
    run(&mut d,&mut f,&mut p,"b.dispatchEvent(new FocusEvent('focusin',{bubbles:true}));if(seen!==1||!a.matches(':focus')||b.matches(':focus')||obs.takeRecords().length)throw 'synthetic focus changed host state or attributes';");
    assert_eq!(p.state.dom_commits, generation);
    p.begin_job();
    assert_eq!(
        p.focus_transition(&mut d, &mut f, Some(a), Some(b)).errors,
        0,
        "{}",
        p.report()
    );
    run(&mut d,&mut f,&mut p,"if(a.matches(':focus')||wrap.matches(':focus-within')||!b.matches(':focus'))throw 'native focus moved';b.parentNode.removeChild(b);if(b.matches(':focus')||document.body.matches(':focus-within'))throw 'detached stale focus';document.body.appendChild(b);if(b.matches(':focus'))throw 'reinserted focus resurrected';");
}
