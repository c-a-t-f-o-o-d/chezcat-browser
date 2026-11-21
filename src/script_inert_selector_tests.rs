//! Original inert-selector construction, coercion, owner and storage checks.
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
            "https://inert-selector-owned.test/source",
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
fn inert_selector_real_descendant_order_scope_ancestors_and_static_identity() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var t=document.createElement('template');t.innerHTML='<section id="outer" class="zone"><div id="inside"><span id="deep" class="hit"></span></div>text<span id="tail" class="hit"></span><template><b class="hidden">Nested</b></template></section>';var root=t.content,outer=root.firstChild,inside=outer.firstElementChild,deep=inside.firstChild,tail=outer.children[1];
var nodes=root.querySelectorAll('.hit');if(!(nodes instanceof NodeList)||nodes.length!==2||nodes[0]!==deep||nodes[1]!==tail||nodes.item(2)!==null||root.querySelector('.hit')!==deep||outer.querySelector('#outer')!==null||inside.querySelector('.zone .hit')!==deep||root.querySelector('.hidden')!==null||root.querySelector('template').content.querySelector('b').textContent!=='Nested')throw 'physical order and template scope';
if(!deep.matches('.zone .hit')||deep.closest('section')!==outer||outer.closest('.zone')!==outer||root.querySelector(':root')!==null||deep.closest('body')!==null)throw 'real detached ancestors';
inside.removeChild(deep);if(nodes.length!==2||nodes[0]!==deep||deep.parentNode!==null||root.querySelectorAll('.hit').length!==1||deep.matches('.zone .hit')||deep.closest('section')!==null||!deep.matches('span'))throw 'static membership real mutable nodes';
var seen=[];for(var n of nodes)seen.push(n);if(seen.length!==2||seen[0]!==deep||seen[1]!==tail)throw 'native snapshot iteration';
"##,
    );
}
#[test]
fn inert_document_fragment_queries_use_current_attributes_root_and_owner() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var owner=document.implementation.createHTMLDocument('inert'),body=owner.body,frag=owner.createDocumentFragment(),a=owner.createElement('div'),b=owner.createElement('span');a.setAttribute('id','a');a.setAttribute('class','first');b.setAttribute('data-mode','yes');a.appendChild(b);frag.appendChild(a);
if(owner.querySelector('#a')!==null||frag.querySelector('div > span')!==b||a.querySelector('[data-mode="yes"]')!==b||b.closest('#a')!==a)throw 'real disconnected inert factories';
body.appendChild(frag);if(owner.querySelector(':root')!==owner.documentElement||owner.querySelector('body > #a')!==a||owner.querySelectorAll('span')[0]!==b||frag.querySelectorAll('*').length!==0||a.ownerDocument!==owner)throw 'current inert document topology';
b.setAttribute('data-mode','no');if(owner.querySelector('[data-mode="yes"]')!==null||!b.matches('[data-mode="no"]'))throw 'current attribute source';
var q=DocumentFragment.prototype.querySelector,all=Document.prototype.querySelectorAll;if(q.call(frag,'span')!==null||all.call(owner,'span')[0]!==b||q.name!=='querySelector'||q.length!==1||all.name!=='querySelectorAll'||all.length!==1)throw 'canonical defining-interface methods';
var touched=0,rejected=0,selector={toString:function(){touched++;return '*';}};for(var call of [function(){q.call(a,selector);},function(){all.call(frag,selector);},function(){Document.prototype.querySelector.call(a,selector);},function(){Element.prototype.querySelector.call(frag,selector);}]){try{call();}catch(e){if(e.name!=='TypeError')throw e;rejected++;}}if(rejected!==4||touched!==0)throw 'defining interface before conversion';
"##,
    );
}
#[test]
fn inert_selector_conversion_mutations_brand_and_syntax_errors_are_ordered() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var t=document.createElement('template');t.innerHTML='<div><span id="old"></span></div>';var f=t.content,e=f.firstChild,old=e.firstChild,calls=0;
var q=f.querySelectorAll,source={toString:function(){calls++;e.removeChild(old);return 'span';}};var list=q.call(f,source);if(calls!==1||list.length!==0||old.parentNode!==null)throw 'conversion before actual snapshot';
var rejected=0,arg={toString:function(){calls++;return '*';}};for(var call of [function(){q.call({},arg);},function(){Element.prototype.matches.call(f,arg);},function(){q.call(f);},function(){q.call(f,Symbol('bad'));}]){try{call();}catch(x){if(!(x instanceof TypeError))throw x;rejected++;}}if(rejected!==4||calls!==1)throw 'brand required args and scalar conversion';
var syntax=0;for(var selector of ['', '[', 'div >', ':scope', ':unknown']){try{q.call(f,selector);}catch(x){if(x.name!=='SyntaxError')throw x;syntax++;}}if(syntax!==5||f.querySelector('div')!==e)throw 'finite selector grammar and recovery';
var exact={},same=false;try{q.call(f,{toString:function(){throw exact;}});}catch(x){same=x===exact;}if(!same)throw 'conversion exception identity';
"##,
    );
}
#[test]
fn inert_selector_move_clone_and_import_keep_native_query_identities() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var t=document.createElement('template');t.innerHTML='<div><template><span id="stamp" on-click="data">Actual stamp</span></template></div>';var n=t.content.querySelector('template'),content=n.content,source=content.querySelector('[on-click]'),snapshot=content.querySelectorAll('span');
source.removeAttribute('on-click');var owner=content.ownerDocument,fragment=owner.createDocumentFragment();fragment.insertBefore(source,null);if(content.querySelector('span')!==null||fragment.querySelector('#stamp')!==source||snapshot[0]!==source||source.parentNode!==fragment||source.ownerDocument!==owner)throw 'real inert move';
var clone=fragment.cloneNode(true),copy=clone.querySelector('#stamp');if(copy===source||copy.parentNode!==clone||clone.querySelectorAll('span')[0]!==copy||copy.textContent!=='Actual stamp')throw 'real clone query';
var imported=document.importNode(clone,true);document.getElementById('target').appendChild(imported);var active=document.querySelector('#stamp');if(active===copy||active===source||active.ownerDocument!==document||!active.isConnected||copy.parentNode!==clone||snapshot[0]!==source||fragment.querySelector('#stamp')!==source)throw 'real deep import ownership';
"##,
    );
}
#[test]
fn inert_selector_scratch_and_snapshot_refusal_publish_nothing_and_recover() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var t=document.createElement('template');t.innerHTML='<span class=hit></span><span class=hit></span>';var f=t.content;");
    let Value::Host(handle) = page
        .runtime
        .execute(
            "f;",
            &mut Bridge {
                doc: &mut doc,
                forms: &mut forms,
                state: &mut page.state,
            },
        )
        .unwrap()
    else {
        panic!("real native fragment")
    };
    let before = page.state.retained;
    let count = page.state.node_lists.len();
    let limit = page.state.retained_limit;
    let copies = page.state.copy_work;
    let commits = page.state.dom_commits;
    page.state.retained_limit = before;
    assert!(Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state
    }
    .inert_selector_query(handle, "querySelectorAll", ".hit")
    .is_err());
    assert_eq!(page.state.retained, before);
    assert_eq!(page.state.node_lists.len(), count);
    assert_eq!(page.state.dom_commits, commits);
    page.state.retained_limit = limit;
    page.state.copy_work = 0;
    assert!(Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state
    }
    .inert_selector_query(handle, "querySelectorAll", ".hit")
    .is_err());
    assert_eq!(page.state.retained, before);
    assert_eq!(page.state.node_lists.len(), count);
    page.state.copy_work = copies;
    run(&mut doc,&mut forms,&mut page,"var result=f.querySelectorAll('.hit');if(result.length!==2||result[0]!==f.firstChild||result[1]!==f.lastChild)throw 'recovery';");
    assert!(page.state.retained > before);
}
