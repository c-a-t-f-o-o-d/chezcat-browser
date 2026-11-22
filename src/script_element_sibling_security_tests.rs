//! Real child-owner and finite native refusal/host-work controls.
use super::*;

fn location(source:&str)->crate::navigation::Location {
    crate::navigation::Location {url:Some(crate::url::parse(source).unwrap().0),path:None,root:None,fragment:String::new()}
}
fn fixture(markup:&str)->(Document,Forms,Page) {
    let mut doc=crate::html::parse(markup);doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    page.set_location(&location("https://sibling-security.test/source"));
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0,"{}",page.report());(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str) {
    let out=page.console_eval(doc,forms,source);assert_eq!(out.errors,0,"{source}\n{}",page.report());
    assert!(!page.has_navigation_request());assert!(page.take_dynamic_scripts().is_empty());assert!(page.take_dynamic_stylesheets().is_empty());
}
fn id(doc:&Document,name:&str)->usize {(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some(name)).unwrap()}

#[test]
fn element_siblings_child_native_identity_and_cached_getters_revalidate_actual_origin() {
    let(mut doc,mut forms,mut page)=fixture("<!doctype html><title>Child sibling guard</title><body><span id=a></span><span id=b></span>");
    run(&mut doc,&mut forms,&mut page,r#"
var a=document.getElementById('a'),b=document.getElementById('b');
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,childDoc=w.document,head=childDoc.head,body=childDoc.body;
var get=Object.getOwnPropertyDescriptor(Element.prototype,'nextElementSibling').get,previous=Object.getOwnPropertyDescriptor(Element.prototype,'previousElementSibling').get;
var childGet=Object.getOwnPropertyDescriptor(w.Element.prototype,'nextElementSibling').get;
var childRead=w.Function('node','return node.nextElementSibling;'),savedRead=w.Function('getter','node','return getter.call(node);');
var own=w.Function('return document.head.nextElementSibling===document.body&&document.body.previousElementSibling===document.head&&document.documentElement.previousElementSibling===null&&window===self;');
if(head.nextElementSibling!==body||body.previousElementSibling!==head||get.call(head)!==body||previous.call(body)!==head||childGet.call(head)!==body||childRead(a)!==b||savedRead(get,a)!==b||!own())throw 'real same-origin child sibling identity';
"#);
    page.set_location(&location("https://different-sibling.test/changed"));
    for source in ["head.nextElementSibling;","get.call(head);","childGet.call(head);","childRead(a);","savedRead(get,a);"] {
        page.clear_console();let allocations=page.runtime.window_allocation_state_for_test();
        let out=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(out.errors,1,"{source}\n{}",page.report());
        assert!(page.report().contains("same-origin"),"{source}\n{}",page.report());
        assert_eq!(page.runtime.window_allocation_state_for_test(),allocations,"{source}");
        run(&mut doc,&mut forms,&mut page,"if(!own()||a.nextElementSibling!==b||document.body.nodeType!==1||window!==self)throw 'owner recovery after sibling refusal';");
    }
}

#[test]
fn element_siblings_distinguish_template_hosts_from_unsupported_inert_or_foreign_nodes() {
    let(mut doc,mut forms,mut page)=fixture("<!doctype html><title>Sibling refusals</title><body><div><span id=a></span><template id=template><span id=inert></span></template><span id=b></span></div><div><span id=blocked></span><object><span id=hidden></span></object><span id=other></span></div><div><span id=before></span><svg id=foreign>foreign text</svg><span id=after></span></div>");
    run(&mut doc, &mut forms, &mut page, r#"
var a=document.getElementById('a'),b=document.getElementById('b'),template=document.getElementById('template');
var blocked=document.getElementById('blocked'),before=document.getElementById('before'),after=document.getElementById('after');
var foreign=document.getElementById('foreign'),foreignText=foreign.firstChild,text=document.createTextNode('kept');
if(a.nextSibling!==template||a.nextElementSibling!==template||template.nextElementSibling!==b||
    template.content.firstChild.id!=='inert'||document.getElementById('inert')!==null||
    foreignText.nodeType!==3||text.data!=='kept')throw 'template host and real namespace boundaries';
"#);
    let snapshot=format!("{:?}",doc.nodes);
    for source in ["blocked.nextElementSibling;","before.nextElementSibling;","foreign.nextElementSibling;","foreignText.nextElementSibling;"] {
        page.clear_console();let out=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(out.errors,1,"{source}\n{}",page.report());
        assert_eq!(format!("{:?}",doc.nodes),snapshot,"{source}");
        run(&mut doc,&mut forms,&mut page,"if(a.nextSibling!==template||template.nextElementSibling!==b||text.data!=='kept'||document.title!=='Sibling refusals'||document.getElementById('after')!==after||document.getElementById('hidden')!==null)throw 'sibling refusal altered existing boundaries';");
    }
    run(&mut doc,&mut forms,&mut page,"text.arbitrarySiblingState=17;if(text.arbitrarySiblingState!==17||text.data!=='kept')throw 'ordinary CharacterData state';");
    assert_eq!(format!("{:?}",doc.nodes),snapshot);
    let bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let text=bridge.doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Text(data) if data=="kept")).unwrap();
    assert!(bridge.expando_owner(node_handle(text)).is_some());
}

#[test]
fn element_siblings_host_work_failure_is_fatal_and_preserves_native_editor_and_following_jobs() {
    let(mut doc,mut forms,mut page)=fixture("<!doctype html><title>Sibling work</title><body><div id=box><span id=a></span>middle<span id=b></span></div><form id=f><input id=neighbor name=neighbor value=original></form>");
    let neighbor=id(&doc,"neighbor");forms.script_set_value(neighbor,"edited neighbor").unwrap();
    forms.controls.get_mut(&neighbor).unwrap().value.cursor=5;forms.controls.get_mut(&neighbor).unwrap().value.anchor=2;
    let c=&forms.controls[&neighbor];let editor=(c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.owner);
    run(&mut doc,&mut forms,&mut page,"var a=document.getElementById('a'),b=document.getElementById('b'),caught=0,finalized=0,after=0;if(a.nextElementSibling!==b)throw 'sibling quota prefix';");
    let snapshot=format!("{:?}",doc.nodes);let limit=page.state.limits.host_work;
    page.state.limits.host_work=0;page.clear_console();
    let out=page.console_eval(&mut doc,&mut forms,"try{a.nextElementSibling;after++;}catch(error){caught++;}finally{finalized++;}after++;");
    assert_eq!(out.errors,1,"{}",page.report());assert!(page.runtime.invocation_failed_fatally());
    assert_eq!(format!("{:?}",doc.nodes),snapshot);
    let c=&forms.controls[&neighbor];assert_eq!((c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.owner),editor);
    assert_eq!(forms.text_default_value(neighbor).unwrap(),"original");
    page.state.limits.host_work=limit;
    run(&mut doc,&mut forms,&mut page,"if(caught||finalized||after||a.nextElementSibling!==b||b.previousElementSibling!==a||document.getElementById('neighbor').value!=='edited neighbor')throw 'fatal sibling work or recovery';");
}

#[test]
fn element_siblings_precharge_full_vector_and_refuse_malformed_parent_identity() {
    let(mut doc,mut forms,mut page)=fixture("<!doctype html><body><div id=box><span id=a></span>middle<span id=b></span></div>");
    let a=id(&doc,"a");let parent=id(&doc,"box");
    page.state.rebuild(&doc).unwrap();
    let snapshot=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    let mut depth=1;let mut current=a;
    while let Some(ancestor)=page.state.tree.parents[current] {depth+=1;current=ancestor;}
    let allowance=page.state.host_work;page.state.host_work=depth;
    {let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};assert!(bridge.element_sibling(node_handle(a),true).is_err());}
    assert_eq!(format!("{:?}",doc.nodes),snapshot);assert_eq!(page.state.dom_commits,commits);
    page.state.host_work=allowance;
    doc.nodes[parent].children.push(a);
    {let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let error=bridge.element_sibling(node_handle(a),true).unwrap_err();assert!(error.contains("Repeated"),"{error}");}
    doc.nodes[parent].children.pop();
    let saved=page.state.tree.parents[a];page.state.tree.parents[a]=Some(a);
    {let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let error=bridge.element_sibling(node_handle(a),true).unwrap_err();assert!(error.contains("Cyclic"),"{error}");}
    page.state.tree.parents[a]=saved;
    run(&mut doc,&mut forms,&mut page,"var a=document.getElementById('a'),b=document.getElementById('b');if(a.nextElementSibling!==b)throw 'sibling identity recovery';");
}
