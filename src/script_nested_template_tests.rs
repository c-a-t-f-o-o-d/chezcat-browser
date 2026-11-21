//! Original nested-template construction, coercion, owner and storage checks.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(
        "<html><body><div id=outer><div id=target>Original</div></div></body></html>",
    );
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(
        &crate::navigation::address(
            "https://nested-template-owned.test/source",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
}
#[test]
fn nested_template_fragment_replacement_has_distinct_content_owners_and_real_serialization() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var t=document.createElement('template'),root=t.content,list=root.childNodes;
t.innerHTML='<div id="outer">Outer<template id="nested"><span id="inner">Inner</span><template><b>Deep</b></template></template><i>Tail</i></div>';
var d=root.firstChild,n=d.children[0],c=n.content,deep=c.lastChild;
if(t.content!==root||root.childNodes!==list||list.length!==1||d.parentNode!==root||d.textContent!=='OuterTail'||n.localName!=='template'||n.childNodes.length!==0||n.firstChild!==null||n.textContent!==''||n.content!==c||c===root||c.parentNode!==null||c.ownerDocument!==root.ownerDocument||c.firstChild.parentNode!==c||c.textContent!=='Inner'||deep.content.textContent!=='Deep'||deep.content.ownerDocument!==c.ownerDocument||document.getElementById('inner')!==null)throw 'separate template content topology';
var expected='<div id="outer">Outer<template id="nested"><span id="inner">Inner</span><template><b>Deep</b></template></template><i>Tail</i></div>';if(t.innerHTML!==expected||n.innerHTML!=='<span id="inner">Inner</span><template><b>Deep</b></template>')throw 'actual nested serialization';
var copy=t.cloneNode(true),nc=copy.content.firstChild.children[0];if(copy.content===root||nc===n||nc.content===c||copy.innerHTML!==expected||nc.content.ownerDocument!==c.ownerDocument||nc.content.firstChild===c.firstChild)throw 'deep clone independent content';
var old=c.firstChild;n.innerHTML='<em>Replaced</em><template><strong>New deep</strong></template>';if(n.content!==c||old.parentNode!==null||old.textContent!=='Inner'||c.firstChild.textContent!=='Replaced'||c.lastChild.content.textContent!=='New deep'||copy.innerHTML!==expected)throw 'nested replacement and old identities';
var retained=n.content,retainedChild=c.firstChild;t.innerHTML='<template><u>Fresh</u></template>';if(t.content!==root||list.length!==1||d.parentNode!==null||n.content!==retained||retainedChild.parentNode!==retained||t.innerHTML!=='<template><u>Fresh</u></template>')throw 'outer replacement retained nested content';
"##,
    );
    assert!(!page.has_host_tasks());
}
#[test]
fn nested_template_fragment_scopes_do_not_close_outer_paragraphs_lists_or_formatting() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var t=document.createElement('template');t.innerHTML='<p>Before<template><div>Inner</div></template>After</p><ul><li>A<template><li>B</li></template>C</li></ul><b>Outer<template><b>Inner</b></template>Tail</b>';
var c=t.content,p=c.firstChild,ul=c.children[1],b=c.lastChild;
if(p.localName!=='p'||p.textContent!=='BeforeAfter'||p.childNodes.length!==3||p.children[0].content.firstChild.localName!=='div'||ul.children.length!==1||ul.firstChild.textContent!=='AC'||ul.firstChild.children[0].content.firstChild.textContent!=='B'||b.textContent!=='OuterTail'||b.firstChild.nextSibling.content.textContent!=='Inner')throw 'fresh template scope';
t.innerHTML='<div>Before<template></div><span>Inside</span></template>After</div>';if(t.content.childNodes.length!==1||t.content.firstChild.textContent!=='BeforeAfter'||t.content.firstChild.children[0].content.firstChild.localName!=='span')throw 'end tag cannot escape template scope';
"##,
    );
}
#[test]
fn nested_template_resources_stay_inert_and_unsupported_recovery_is_atomic() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var t=document.createElement('template');t.innerHTML='<template><script>document.title="never";</script><img src="https://unrequested.test/nested.png"><style>body{background:red}</style></template>';var root=t.content,n=root.firstChild,c=n.content,old=t.innerHTML;
if(c.childNodes.length!==3||c.firstChild.textContent!=='document.title="never";'||c.children[1].localName!=='img'||c.children[2].localName!=='style'||c.isConnected||c.firstChild.isConnected)throw 'inert nested resources';
"##,
    );
    assert!(!page.has_host_tasks());
    let retained = page.state.retained;
    let nodes = doc.nodes.len();
    let out = page.console_eval(
        &mut doc,
        &mut forms,
        "t.innerHTML='<template><table><tr><td>not implemented</td></tr></table></template>';",
    );
    assert_eq!(out.errors, 1);
    assert_eq!(page.state.retained, retained);
    assert_eq!(doc.nodes.len(), nodes);
    run(&mut doc,&mut forms,&mut page,"if(t.content!==root||root.firstChild!==n||n.content!==c||t.innerHTML!==old)throw 'failed parse changed identity';");
}
#[test]
fn nested_template_retained_refusal_preserves_existing_indexes_and_recovers() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var t=document.createElement('template');t.innerHTML='<template><b>Kept</b></template>';var c=t.content,n=c.firstChild,f=n.content,old=t.innerHTML;");
    let Value::Host(handle) = page
        .runtime
        .execute(
            "t;",
            &mut Bridge {
                doc: &mut doc,
                forms: &mut forms,
                state: &mut page.state,
            },
        )
        .unwrap()
    else {
        panic!("native template handle")
    };
    let before = page.state.retained;
    let limit = page.state.retained_limit;
    let nodes = doc.nodes.len();
    page.state.retained_limit = before;
    let result = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    }
    .set_template_html(
        handle,
        "<template><i>Refused</i><template><u>Deep</u></template></template>",
    );
    assert!(result.is_err());
    page.state.retained_limit = limit;
    assert_eq!(doc.nodes.len(), nodes);
    assert_eq!(page.state.retained, before);
    run(&mut doc,&mut forms,&mut page,"if(t.innerHTML!==old||t.content!==c||c.firstChild!==n||n.content!==f)throw 'unchanged';t.innerHTML='<template><i>Recovered</i></template>';if(t.content.firstChild.content.textContent!=='Recovered')throw 'recovery';");
}
