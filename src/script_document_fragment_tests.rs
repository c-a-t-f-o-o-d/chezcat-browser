//! Original genuine fragment identity, live tree, batch and native-model controls.
use super::*;
fn fixture(source:&str)->(Document,Forms,Page) {
    fixture_page(source,Page::for_test())
}
fn fixture_page(source:&str,mut page:Page)->(Document,Forms,Page) {
    let mut doc=crate::html::parse("<html><body><form id=f><input id=neighbor name=keep value=seed><div id=host></div><span id=after>After</span></form><div id=dest></div><svg id=foreign></svg></body></html>");
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);
    page.set_location(&crate::navigation::address("https://fragment.test/source",&crate::navigation::Location::home()).unwrap());
    let outcome=page.start(&mut doc,&mut forms,&[Script{node:0,label:"Original DocumentFragment".into(),source:source.into()}]);
    assert_eq!(outcome.errors,0,"{}",page.report());(doc,forms,page)
}
fn id(doc:&Document,name:&str)->usize {(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(name)).unwrap()}
fn eval(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str) {
    let outcome=page.console_eval(doc,forms,source);assert_eq!(outcome.errors,0,"{source}\n{}",page.report());
}
#[test]
fn document_fragment_factory_and_constructor_have_genuine_node_identity() {
    let(_,_,_)=fixture(r#"
var factory=document.createDocumentFragment,f=factory.call(document),g=new DocumentFragment(),effects=0;
if(factory.length!==0||DocumentFragment.length!==0||f===g||f.nodeType!==11||f.nodeName!=='#document-fragment')throw 'genuine type';
if(Object.getPrototypeOf(f)!==DocumentFragment.prototype||Object.getPrototypeOf(DocumentFragment.prototype)!==Node.prototype||!(f instanceof DocumentFragment)||!(f instanceof Node)||f instanceof Element)throw 'actual Node subtype';
if(f.ownerDocument!==document||f.parentNode!==null||f.parentElement!==null||f.nodeValue!==null||f.isConnected||f.getRootNode()!==f||f.childNodes.length!==0||f.textContent!=='')throw 'detached metadata';
if(f.tagName!==undefined||f.attributes!==undefined||f.style!==undefined||f.setAttribute!==undefined)throw 'no Element alias';
factory.call(document,{toString:function(){effects++;throw 'ignored extra';}});if(effects)throw 'extra conversion';
var caught=0;try{DocumentFragment();}catch(e){if(e instanceof TypeError)caught++;}try{factory.call({});}catch(e){if(e instanceof TypeError)caught++;}if(caught!==2)throw 'constructor/factory brands';
f.marker=7;if(f.marker!==7||Object.keys(f).join(',')!=='marker')throw 'ordinary fragment properties';
"#);
}
#[test]
fn document_fragment_children_are_live_mixed_nodes_with_real_parent_and_root() {
    let(_,_,_)=fixture(r#"
var f=document.createDocumentFragment(),list=f.childNodes,text=document.createTextNode('Original'),comment=document.createComment('kept'),span=document.createElement('span');span.textContent='Child';
if(f.appendChild(text)!==text||f.appendChild(comment)!==comment||f.appendChild(span)!==span)throw 'ordinary returns';
if(list!==f.childNodes||list.length!==3||list[0]!==text||list[1]!==comment||list[2]!==span||f.firstChild!==text||f.lastChild!==span)throw 'live children';
if(text.parentNode!==f||text.parentElement!==null||span.parentNode!==f||span.parentElement!==null||text.nextSibling!==comment||span.previousSibling!==comment)throw 'physical parents/siblings';
if(f.textContent!=='OriginalChild'||text.getRootNode()!==f||span.firstChild.getRootNode()!==f||comment.isConnected)throw 'fragment roots/text';
if(f.removeChild(comment)!==comment||list.length!==2||comment.getRootNode()!==comment)throw 'ordinary remove';
"#);
}
#[test]
fn document_fragment_append_splices_every_child_and_empties_same_donor_list() {
    let(doc,_,_)=fixture(r#"
var f=new DocumentFragment(),list=f.childNodes,text=document.createTextNode('One'),a=document.createElement('a');a.id='inserted';a.setAttribute('href','#target');a.textContent='Two';f.appendChild(text);f.appendChild(a);
var dest=document.getElementById('dest');if(dest.appendChild(f)!==f||f.childNodes!==list||list.length!==0||f.parentNode!==null||f.isConnected||f.getRootNode()!==f)throw 'donor remains detached';
if(dest.childNodes.length!==2||dest.firstChild!==text||dest.lastChild!==a||a.parentNode!==dest||a.parentElement!==dest||a.getRootNode()!==document||!a.isConnected||document.getElementById('inserted')!==a)throw 'real moved children';
"#);
    let fragment=doc.nodes.iter().position(|node|matches!(node.kind,NodeKind::DocumentFragment)).unwrap();
    let page=crate::layout::build_viewport(&doc,400,300,1.0,&HashMap::new(),&HashMap::new());
    assert!(page.border_rects[fragment].is_none());assert!(!page.controls.iter().any(|control|control.node==fragment));
}
#[test]
fn document_fragment_nested_donors_insert_order_and_empty_replace_are_real() {
    let(_,_,_)=fixture(r#"
var outer=new DocumentFragment(),inner=new DocumentFragment(),innerList=inner.childNodes,a=document.createElement('span'),b=document.createTextNode('B');a.id='a';inner.appendChild(a);inner.appendChild(b);
if(outer.appendChild(inner)!==inner||innerList.length!==0||outer.childNodes.length!==2||a.parentNode!==outer||a.getRootNode()!==outer)throw 'nested donor splice';
var dest=document.getElementById('dest'),old=document.createElement('p');dest.appendChild(old);
if(dest.insertBefore(outer,old)!==outer||dest.childNodes[0]!==a||dest.childNodes[1]!==b||dest.childNodes[2]!==old||outer.childNodes.length!==0)throw 'batch insert order';
var replacement=new DocumentFragment(),c=document.createComment('replacement'),d=document.createElement('span');replacement.appendChild(c);replacement.appendChild(d);
if(dest.replaceChild(replacement,old)!==old||old.parentNode!==null||replacement.childNodes.length!==0||dest.childNodes[2]!==c||dest.childNodes[3]!==d)throw 'batch replace';
var empty=new DocumentFragment();if(dest.replaceChild(empty,a)!==a||a.getRootNode()!==a||dest.firstChild!==b||empty.childNodes.length!==0)throw 'empty replacement removes';
if(dest.insertBefore(empty,null)!==empty||dest.childNodes.length!==3)throw 'empty insertion no op';
"#);
}
#[test]
fn document_fragment_native_text_controls_keep_editor_and_projected_owner() {
    let(mut doc,mut forms,mut page)=fixture("document.getElementById('host').innerHTML='<section id=group><input id=dynamic name=q value=default></section>';var group=document.getElementById('group'),fragment=new DocumentFragment();document.getElementById('dynamic').value='edited';");
    let input=id(&doc,"dynamic");let neighbor=id(&doc,"neighbor");let form=id(&doc,"f");
    forms.controls.get_mut(&input).unwrap().value.cursor=1;forms.controls.get_mut(&input).unwrap().value.anchor=3;
    let preserved=(forms.controls[&neighbor].value.text(),forms.controls[&neighbor].value.cursor,forms.controls[&neighbor].owner);
    eval(&mut doc,&mut forms,&mut page,"fragment.appendChild(group);if(group.parentNode!==fragment||group.getRootNode()!==fragment||document.getElementById('dynamic')!==null)throw 'detached real group';");
    assert_eq!(forms.controls[&input].owner,None);assert_eq!(forms.controls[&input].value.text(),"edited");
    eval(&mut doc,&mut forms,&mut page,"document.getElementById('f').insertBefore(fragment,document.getElementById('after'));if(group.getRootNode()!==document||fragment.childNodes.length!==0)throw 'reconnected group';");
    assert_eq!(forms.controls[&input].owner,Some(form));assert_eq!((forms.controls[&input].value.cursor,forms.controls[&input].value.anchor),(1,3));assert!(forms.controls[&input].dirty_value);
    assert_eq!((forms.controls[&neighbor].value.text(),forms.controls[&neighbor].value.cursor,forms.controls[&neighbor].owner),preserved);
    let current=crate::navigation::address("https://fragment.test/source",&crate::navigation::Location::home()).unwrap();
    assert_eq!(forms.submit_request(&doc,form,None,&current,&current).unwrap().location.url.unwrap().query.as_deref(),Some("keep=seed&q=edited"));
}
#[test]
fn document_fragment_failed_batch_never_publishes_a_prefix_or_loses_donor_children() {
    let(mut doc,mut forms,mut page)=fixture("var fragment=new DocumentFragment(),first=document.createElement('span'),script=document.createElement('script');first.id='first';fragment.appendChild(first);fragment.appendChild(script);var dest=document.getElementById('dest');");
    let fragment=doc.nodes.iter().position(|node|matches!(node.kind,NodeKind::DocumentFragment)).unwrap();let dest=id(&doc,"dest");
    let donor=doc.nodes[fragment].children.clone();let destination=doc.nodes[dest].children.clone();let commits=page.state.dom_commits;
    for source in ["dest.appendChild(fragment);","dest.insertBefore(fragment,null);","document.getElementById('foreign').appendChild(fragment);","fragment.appendChild(fragment);"] {
        let outcome=page.console_eval(&mut doc,&mut forms,source);assert_eq!(outcome.errors,1,"{source}\n{}",page.report());
        assert_eq!(doc.nodes[fragment].children,donor);assert_eq!(doc.nodes[dest].children,destination);assert_eq!(page.state.dom_commits,commits);
        assert!(page.take_dynamic_scripts().is_empty());assert!(page.take_dynamic_stylesheets().is_empty());
    }
}
#[test]
fn document_fragment_native_borrowing_and_child_factory_boundary_keep_real_owners() {
    let(mut doc,mut forms,mut page)=fixture_page(r#"
var f=new DocumentFragment(),a=document.createElement('span'),dest=document.getElementById('dest');
dest.appendChild.call(f,a);if(a.parentNode!==f)throw 'Node method borrowed to fragment';f.appendChild.call(dest,f);if(a.parentNode!==dest||f.childNodes.length!==0)throw 'Node method borrowed from fragment';
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var child=frame.contentWindow;
"#,Page::with_limits(crate::javascript::limits::BROWSER));
    let before=doc.nodes.len();
    let outcome=page.console_eval(&mut doc,&mut forms,"new child.DocumentFragment();");
    assert_eq!(outcome.errors,1,"{}",page.report());assert_eq!(doc.nodes.len(),before);assert!(page.report().contains("Child DocumentFragment allocation"),"{}",page.report());
}
#[test]
fn document_fragment_node_and_staging_allowances_refuse_without_partial_splice() {
    let(mut doc,mut forms,mut page)=fixture("var fragment=new DocumentFragment(),a=document.createElement('span');a.id='retained-child';fragment.appendChild(a);");
    let donor=doc.nodes.iter().position(|node|matches!(node.kind,NodeKind::DocumentFragment)).unwrap();let dest=id(&doc,"dest");let children=doc.nodes[donor].children.clone();
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let count=bridge.doc.nodes.len();bridge.state.node_limit=count;
    assert!(bridge.create_dom_node(NodeKind::DocumentFragment).is_err());assert_eq!(bridge.doc.nodes.len(),count);
    bridge.state.host_work=0;
    assert!(bridge.splice_document_fragment(dest,"appendChild",&[Value::Host(node_handle(donor))]).is_err());
    assert_eq!(bridge.doc.nodes[donor].children,children);assert!(bridge.doc.nodes[dest].children.is_empty());
}
