//! Original passive custom-host content and native mutation boundaries.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc=crate::html::parse("<html><body><original-card id=host><span id=old>old</span><nested-card id=nested>nested</nested-card></original-card><div id=ordinary></div><protected-card id=protected><input id=field value=seed></protected-card></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(
        &crate::navigation::address(
            "https://custom-content.test/source",
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
fn custom_host_content_native_text_replacement_records_range_and_saved_identity() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var host=document.getElementById('host'),old=host.firstChild,nested=old.nextSibling,list=host.childNodes,range=new Range(),observer=new MutationObserver(function(){}),set=Object.getOwnPropertyDescriptor(Node.prototype,'textContent').set;range.setStart(old.firstChild,1);range.setEnd(nested.firstChild,3);observer.observe(host,{childList:true});var conversions=0;set.call(host,{toString:function(){conversions++;return 'A<&';}});var records=observer.takeRecords();if(conversions!==1||host.textContent!=='A<&'||host.innerHTML!=='A&lt;&amp;'||list.length!==1||host.firstChild.nodeType!==3||old.parentNode!==null||nested.parentNode!==null||old.textContent!=='old'||nested.textContent!=='nested'||old.isConnected||nested.isConnected)throw 'actual native replacement';if(records.length!==1||records[0].target!==host||records[0].removedNodes.length!==2||records[0].removedNodes[0]!==old||records[0].removedNodes[1]!==nested||records[0].addedNodes.length!==1||records[0].addedNodes[0]!==host.firstChild||records[0].previousSibling!==null||records[0].nextSibling!==null)throw 'whole-content mutation';if(range.startContainer!==host||range.startOffset||range.endContainer!==host||range.endOffset||!range.collapsed)throw 'actual Range retirement';
for(var tag of ['section','main','nav','article','table','slot']){var passive=document.createElement(tag);passive.textContent='passive';if(passive.firstChild.data!=='passive'||passive.textContent!=='passive')throw 'shared passive container '+tag;}
var made=document.createElement('created-card');made.textContent='detached';if(made.firstChild.data!=='detached'||made.isConnected)throw 'detached native';document.body.appendChild(made);made.textContent=null;if(made.childNodes.length||made.textContent!=='')throw 'empty replacement';
"#,
    );
}
#[test]
fn custom_host_content_inert_import_promotion_and_ordinary_parent_remove_passive_custom_tree() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var template=document.createElement('template');template.innerHTML='<staged-card><nested-card>staged</nested-card></staged-card>';var staged=template.content.firstChild,owner=staged.ownerDocument;staged.textContent='inert';if(staged.textContent!=='inert'||staged.ownerDocument!==owner)throw 'inert native';var imported=document.importNode(template.content,true),copy=imported.firstChild;copy.textContent='imported';document.getElementById('ordinary').appendChild(imported);if(copy.ownerDocument!==document||copy.textContent!=='imported')throw 'import owner';var reservable=document.createElement('template');reservable.innerHTML='<promoted-card>reserved</promoted-card>';var promoted=reservable.content.firstChild;document.body.appendChild(reservable.content);promoted.textContent='promoted';if(promoted.ownerDocument!==document||promoted.firstChild.data!=='promoted')throw 'promoted alias';var old=copy;document.getElementById('ordinary').textContent='replacement';if(old.parentNode!==null||old.textContent!=='imported'||document.getElementById('ordinary').firstChild.data!=='replacement')throw 'ordinary parent passive custom retirement';
"#,
    );
    run(&mut doc,&mut forms,&mut page,"var savedLeaf=staged.firstChild;document.body.appendChild(template.content);if(staged.ownerDocument!==document||staged.firstChild!==savedLeaf||savedLeaf.ownerDocument!==document||savedLeaf.data!=='inert'||!savedLeaf.isConnected||template.content.firstChild!==null)throw 'stable new leaf promotion';");
}

#[test]
fn inert_leaf_identity_replacement_creation_clone_observers_and_range() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var template=document.createElement('template');template.innerHTML='<leaf-card>old</leaf-card>';var root=template.content.firstChild,owner=root.ownerDocument,old=root.firstChild,list=root.childNodes,obs=new MutationObserver(function(){});obs.observe(root,{childList:true,subtree:true,characterData:true,characterDataOldValue:true});root.textContent='new';var leaf=root.firstChild;leaf.authorMarker=17;var records=obs.takeRecords();if(records.length!==1||records[0].addedNodes[0]!==leaf||records[0].removedNodes[0]!==old||leaf.ownerDocument!==owner||list[0]!==leaf)throw 'inert record identity';leaf.data='new text';var made=owner.createTextNode('made'),copy=made.cloneNode(true);if(made===copy||copy.data!=='made'||copy.ownerDocument!==owner)throw 'new inert clone';root.appendChild(made);root.appendChild(copy);obs.takeRecords();var dest=document.getElementById('ordinary'),range=new Range();range.selectNodeContents(dest);range.collapse(false);var destObs=new MutationObserver(function(){});destObs.observe(dest,{childList:true});dest.appendChild(template.content);var added=destObs.takeRecords();if(added.length!==1||added[0].addedNodes[0]!==root||root.firstChild!==leaf||leaf.authorMarker!==17||list[0]!==leaf||list[1]!==made||list[2]!==copy||leaf.ownerDocument!==document||made.ownerDocument!==document||copy.ownerDocument!==document||!leaf.isConnected||old.parentNode!==null||old.ownerDocument!==owner||range.startContainer!==dest||range.startOffset!==0)throw 'native identity activation';leaf.data='live';var changed=obs.takeRecords();if(changed.length!==1||changed[0].target!==leaf||changed[0].oldValue!=='new text'||root.textContent!=='livemademade')throw 'promoted observer';range.selectNodeContents(leaf);range.setStart(leaf,2);leaf.data='li';if(range.startContainer!==leaf||range.startOffset!==0||range.endOffset!==0)throw 'promoted range';
"#);
}

#[test]
fn inert_leaf_identity_native_quota_fallback_and_retained_refusal_are_atomic() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var t=document.createElement('template');t.innerHTML='<fallback-card>old</fallback-card>';var root=t.content.firstChild,old=root.firstChild,obs=new MutationObserver(function(){});obs.observe(root,{childList:true});");
    let limit = page.state.retained_limit;
    let before = (doc.nodes.len(), page.state.dom_commits);
    page.state.retained_limit = page.state.retained;
    assert_eq!(page.console_eval(&mut doc,&mut forms,"root.textContent='blocked';").errors,1);
    assert_eq!((doc.nodes.len(), page.state.dom_commits),before);
    page.state.retained_limit = limit;
    run(&mut doc,&mut forms,&mut page,"if(root.firstChild!==old||old.parentNode!==root||obs.takeRecords().length)throw 'failed replacement publication';");
    let node_limit = page.state.node_limit;
    page.state.node_limit = doc.nodes.len();
    run(&mut doc,&mut forms,&mut page,"root.textContent='staged';var saved=root.firstChild,owner=saved.ownerDocument,records=obs.takeRecords();if(records.length!==1||records[0].addedNodes[0]!==saved||records[0].removedNodes[0]!==old)throw 'fallback observer identity';");
    assert_eq!(doc.nodes.len(),before.0);
    page.state.node_limit = node_limit;
    let before = (doc.nodes.len(), page.state.dom_commits);
    assert_eq!(page.console_eval(&mut doc,&mut forms,"document.body.appendChild(t.content);").errors,1);
    assert_eq!((doc.nodes.len(), page.state.dom_commits),before);
    run(&mut doc,&mut forms,&mut page,"if(root.firstChild!==saved||saved.ownerDocument!==owner||saved.data!=='staged'||obs.takeRecords().length)throw 'fallback promotion remapped';root.textContent='recovered';var fresh=root.firstChild;document.body.appendChild(t.content);if(root.firstChild!==fresh||fresh.ownerDocument!==document||fresh.data!=='recovered'||saved.parentNode!==null||saved.ownerDocument!==owner)throw 'fresh job recovery';");
}

#[test]
fn inert_element_identity_created_html_svg_style_and_text_keep_saved_handles() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var t=document.createElement('template'),content=t.content,owner=content.ownerDocument,root=owner.createElement('made-card'),span=owner.createElement('span'),text=owner.createTextNode('real'),svg=owner.createElementNS('http://www.w3.org/2000/svg','svg'),rect=owner.createElementNS('http://www.w3.org/2000/svg','rect'),style=owner.createElement('style');root.setAttribute('id','made-root');span.setAttribute('class','label');style.textContent='#made-root{color:#123456}';span.appendChild(text);root.appendChild(span);svg.setAttribute('width','8');svg.setAttribute('height','8');rect.setAttribute('width','8');rect.setAttribute('height','8');rect.setAttribute('fill','#123456');svg.appendChild(rect);root.appendChild(svg);content.appendChild(style);content.appendChild(root);var list=root.childNodes,attr=rect.getAttributeNode('fill');root.authorState={node:root};var obs=new MutationObserver(function(){});obs.observe(root,{childList:true,subtree:true,attributes:true});document.getElementById('ordinary').appendChild(content);if(document.getElementById('made-root')!==root||root.authorState.node!==root||root.ownerDocument!==document||span.ownerDocument!==document||text.ownerDocument!==document||svg.ownerDocument!==document||rect.ownerDocument!==document||style.ownerDocument!==document||root.firstChild!==span||span.firstChild!==text||list[1]!==svg||svg.firstChild!==rect||rect.getAttributeNode('fill')!==attr||attr.ownerElement!==rect||!text.isConnected)throw 'created inert native identities';rect.setAttribute('fill','#654321');var records=obs.takeRecords();if(records.length!==1||records[0].target!==rect||records[0].attributeName!=='fill')throw 'saved created observer';
"#);
    assert!(page.take_dynamic_scripts().is_empty());
}

#[test]
fn inert_element_identity_late_handlers_and_native_controls_refuse_then_recover() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var t=document.createElement('template'),content=t.content,owner=content.ownerDocument,root=owner.createElement('made-card');root.textContent='safe';root.setAttribute('onclick','blocked()');content.appendChild(root);var saved=root.firstChild;var observer=new MutationObserver(function(){});observer.observe(document.body,{childList:true});");
    let before = (doc.nodes.len(), page.state.dom_commits);
    assert_eq!(page.console_eval(&mut doc,&mut forms,"document.body.appendChild(content);").errors,1);
    assert_eq!((doc.nodes.len(), page.state.dom_commits),before);
    run(&mut doc,&mut forms,&mut page,"if(content.firstChild!==root||root.ownerDocument!==owner||root.firstChild!==saved||observer.takeRecords().length)throw 'late handler activation';root.removeAttribute('onclick');document.body.appendChild(content);if(root.ownerDocument!==document||root.firstChild!==saved||saved.ownerDocument!==document)throw 'late handler removal recovery';var input=owner.createElement('input');content.appendChild(input);observer.takeRecords();");
    let before = (doc.nodes.len(), page.state.dom_commits, forms.controls.len());
    assert_eq!(page.console_eval(&mut doc,&mut forms,"document.body.appendChild(content);").errors,1);
    assert_eq!((doc.nodes.len(), page.state.dom_commits, forms.controls.len()),before);
    run(&mut doc,&mut forms,&mut page,"if(content.firstChild!==input||input.ownerDocument!==owner||observer.takeRecords().length)throw 'native control prefix';");
    assert!(page.take_dynamic_scripts().is_empty());
}

#[test]
fn inert_clone_identity_deep_shallow_fragment_and_saved_descendants_promote() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var t=document.createElement('template');t.innerHTML='<clone-card id="cloned"><span>original</span><!--comment--><svg width="8" height="8"><rect width="8" height="8" fill="red"/></svg></clone-card>';var original=t.content.firstChild,owner=original.ownerDocument;original.authorState=17;var clone=t.content.cloneNode(true),root=clone.firstChild,span=root.firstChild,text=span.firstChild,comment=span.nextSibling,svg=comment.nextSibling,rect=svg.firstChild,list=root.childNodes,attr=rect.getAttributeNode('fill');if(root===original||root.authorState!==undefined||text===original.firstChild.firstChild||root.ownerDocument!==owner)throw 'real independent clone';text.data='changed';var shallow=root.cloneNode(false);if(shallow.firstChild!==null||shallow.getAttribute('id')!=='cloned'||shallow.ownerDocument!==owner)throw 'shallow native clone';clone.appendChild(shallow);root.bindingState={node:root};var observer=new MutationObserver(function(){});observer.observe(root,{subtree:true,characterData:true,characterDataOldValue:true});document.getElementById('ordinary').appendChild(clone);if(root.firstChild!==span||span.firstChild!==text||list[1]!==comment||list[2]!==svg||svg.firstChild!==rect||rect.getAttributeNode('fill')!==attr||root.bindingState.node!==root||root.ownerDocument!==document||text.ownerDocument!==document||comment.ownerDocument!==document||rect.ownerDocument!==document||shallow.ownerDocument!==document||!comment.isConnected||original.ownerDocument!==owner||original.textContent!=='original')throw 'saved clone identities';text.data='active';var records=observer.takeRecords();if(records.length!==1||records[0].target!==text||records[0].oldValue!=='changed')throw 'cloned observer target';
"#);
}

#[test]
fn inert_clone_identity_nested_content_and_late_handler_refusal_keep_sources() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var t=document.createElement('template');t.innerHTML='<template><nested-clone>inner</nested-clone></template>';var source=t.content.firstChild,cloned=source.cloneNode(true),content=cloned.content,root=content.firstChild,leaf=root.firstChild,owner=root.ownerDocument;root.setAttribute('onclick','blocked()');");
    let before = (doc.nodes.len(),page.state.dom_commits);
    assert_eq!(page.console_eval(&mut doc,&mut forms,"document.body.appendChild(content);").errors,1);
    assert_eq!((doc.nodes.len(),page.state.dom_commits),before);
    run(&mut doc,&mut forms,&mut page,"if(content.firstChild!==root||root.firstChild!==leaf||root.ownerDocument!==owner||source.content.firstChild===root||source.content.textContent!=='inner')throw 'failed clone activation';root.removeAttribute('onclick');document.body.appendChild(content);if(root.firstChild!==leaf||leaf.ownerDocument!==document||cloned.content!==content||content.firstChild!==null||source.content.textContent!=='inner')throw 'nested clone recovery';");
}

#[test]
fn template_import_identity_outer_slots_content_aliases_and_cross_owner_are_real() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var other=document.implementation.createHTMLDocument('other'),source=other.createElement('template');source.innerHTML='<import-card id="imported"><span>source</span><!--comment--><svg width="8" height="8"><rect width="8" height="8" fill="red"/></svg></import-card>';var oldOwner=source.content.ownerDocument,original=source.content.firstChild,copy=document.importNode(source,true),content=copy.content,root=content.firstChild,span=root.firstChild,text=span.firstChild,comment=span.nextSibling,svg=comment.nextSibling,rect=svg.firstChild,list=root.childNodes,owner=content.ownerDocument,attr=rect.getAttributeNode('fill');if(copy.ownerDocument!==document||owner===oldOwner||root.ownerDocument!==owner||text.ownerDocument!==owner||rect.ownerDocument!==owner||root===original)throw 'appropriate imported owner';root.authorState={node:root};var obs=new MutationObserver(function(){});obs.observe(root,{subtree:true,characterData:true,characterDataOldValue:true});document.getElementById('ordinary').appendChild(copy);document.getElementById('ordinary').appendChild(content);if(copy.content!==content||copy.firstChild!==null||content.firstChild!==null||document.getElementById('imported')!==root||root.firstChild!==span||span.firstChild!==text||list[1]!==comment||list[2]!==svg||svg.firstChild!==rect||rect.getAttributeNode('fill')!==attr||root.authorState.node!==root||root.ownerDocument!==document||text.ownerDocument!==document||comment.ownerDocument!==document||rect.ownerDocument!==document||!rect.isConnected||original.ownerDocument!==oldOwner||original.textContent!=='source')throw 'reserved import identities collided';text.data='active';var records=obs.takeRecords();if(records.length!==1||records[0].target!==text||records[0].oldValue!=='source')throw 'imported observer';var repeat=source.cloneNode(true);if(repeat.ownerDocument!==other||repeat.content.firstChild===original)throw 'inert source clone changed owner';
"#);
}
#[test]
fn custom_host_content_protected_controls_and_resources_refuse_without_publication() {
    let (mut doc, mut forms, mut page) = fixture();
    let field = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("field"))
        .unwrap();
    forms.script_set_value(field, "edited").unwrap();
    let before = (
        doc.nodes.len(),
        page.state.dom_commits,
        forms.controls[&field].value.text(),
    );
    run(&mut doc,&mut forms,&mut page,"var protectedHost=document.getElementById('protected'),child=protectedHost.firstChild,observer=new MutationObserver(function(){}),range=new Range();range.selectNode(child);observer.observe(protectedHost,{childList:true});");
    let out = page.console_eval(&mut doc, &mut forms, "protectedHost.textContent='blocked';");
    assert_eq!(out.errors, 1, "{}", page.report());
    assert_eq!(
        (
            doc.nodes.len(),
            page.state.dom_commits,
            forms.controls[&field].value.text()
        ),
        before
    );
    run(&mut doc,&mut forms,&mut page,"if(protectedHost.firstChild!==child||child.parentNode!==protectedHost||child.value!=='edited'||observer.takeRecords().length||range.startContainer!==protectedHost||range.startOffset!==0||range.endOffset!==1)throw 'protected native prefix publication';");
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
}
#[test]
fn custom_host_content_node_admission_and_foreign_namespace_keep_real_tree() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "var host=document.getElementById('host'),first=host.firstChild,list=host.childNodes;",
    );
    let count = doc.nodes.len();
    let commits = page.state.dom_commits;
    let limit = page.state.node_limit;
    page.state.node_limit = count;
    let out = page.console_eval(&mut doc, &mut forms, "host.textContent='unpublished';");
    assert_eq!(out.errors, 1, "{}", page.report());
    assert_eq!(doc.nodes.len(), count);
    assert_eq!(page.state.dom_commits, commits);
    page.state.node_limit = limit;
    run(&mut doc,&mut forms,&mut page,"if(host.firstChild!==first||list.length!==2||first.textContent!=='old')throw 'failed prefix';host.textContent='recovered';if(host.firstChild.data!=='recovered')throw 'fresh job';");
    let node = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("host"))
        .unwrap();
    doc.nodes[node].namespace = crate::html::Namespace::Svg;
    let children = doc.nodes[node].children.clone();
    let mut bridge = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    };
    assert!(bridge.set_text_content(node, "foreign".into()).is_err());
    assert_eq!(bridge.doc.nodes[node].children, children);
}
