//! Current native topology, inert ownership and atomic structural query results.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut d=crate::html::parse("<html><body><div id=mount><span id=a></span><!--c--><b id=b></b><span id=c> </span></div><input id=neighbor value=seed></body></html>");
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
fn structural_selector_queries_follow_mutations_and_keep_saved_snapshots() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var mount=document.getElementById('mount'),a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c');var saved=mount.querySelectorAll(':nth-child(odd)');if(saved.length!==2||saved[0]!==a||saved[1]!==c||!c.matches('span:last-of-type')||!a.matches(':empty')||c.matches(':empty'))throw 'original structural queries';
mount.insertBefore(c,a);if(mount.querySelector(':first-child')!==c||mount.querySelector('span:nth-of-type(2)')!==a||mount.querySelector(':last-child')!==b||saved[0]!==a||saved[1]!==c)throw 'real current topology versus static snapshot';
var caught=false;try{mount.querySelector('span,:nth-child(2 n)');}catch(e){caught=e.name==='SyntaxError';}if(!caught||saved[0]!==a||document.getElementById('neighbor').value!=='seed')throw 'invalid whole list/native neighbor';
"#,
    );
}
#[test]
fn structural_selector_inert_template_and_fragment_queries_share_real_element_positions() {
    let (mut d, mut f, mut p) = fixture();
    run(
        &mut d,
        &mut f,
        &mut p,
        r#"
var template=document.createElement('template');template.innerHTML='<div id=i></div><!--comment--><span id=j> </span><div id=k></div>';var content=template.content,i=content.querySelector('#i'),j=content.querySelector('#j'),k=content.querySelector('#k');
if(content.querySelector('div:nth-child(3)')!==k||content.querySelector(':only-of-type')!==j||!i.matches(':first-child:empty')||j.matches(':empty')||k.closest(':last-of-type')!==k)throw 'real inert structural positions';
content.insertBefore(k,i);if(content.querySelector('div:first-child')!==k||content.querySelector('div:last-of-type')!==i)throw 'inert mutation invalidation';
var fragment=document.createDocumentFragment(),child=document.createElement('div');fragment.appendChild(document.createComment('ignored'));fragment.appendChild(child);if(fragment.querySelector(':only-child')!==child||!child.matches(':first-child:last-child'))throw 'ordinary fragment siblings';
"#,
    );
}
