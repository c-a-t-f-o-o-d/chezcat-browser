//! Original inert-owner fragment, topology and admission checks.
use super::*;
fn page()->(Document,Forms,Page){
    let mut doc=crate::html::parse_loaded_document("<html><body><template id=seed><span id=item>text<!--marker--><b>bold</b></span></template><div id=mount></div></body></html>",&||Ok(())).unwrap();doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn execute(doc:&mut Document,forms:&mut Forms,page:&mut Page,script:&str){assert_eq!(page.console_eval(doc,forms,script).errors,0,"{}",page.report());}
#[test]
fn inert_owner_fragment_moves_real_mixed_children_live_views_and_imports(){
    let(mut doc,mut forms,mut page)=page();execute(&mut doc,&mut forms,&mut page,r#"
var content=document.getElementById('seed').content,owner=content.ownerDocument,f=owner.createDocumentFragment(),e=content.firstChild,all=content.childNodes,items=content.children,children=f.children;
if(!(f instanceof DocumentFragment)||f.nodeType!==11||f.nodeName!=='#document-fragment'||f.ownerDocument!==owner||f.parentNode!==null||f.isConnected||f.childNodes.length!==0||owner.createDocumentFragment.length!==0)throw 'genuine inert owner fragment '+[f instanceof DocumentFragment,f.nodeType,f.nodeName,f.ownerDocument===owner,f.parentNode,f.isConnected,f.childNodes.length,owner.createDocumentFragment.length].join('|');
var calls=0;owner.createDocumentFragment({toString:function(){calls++;throw 'ignored';}});if(calls!==0)throw 'zero argument factory';
var borrowed=document.createDocumentFragment.call(owner),main=owner.createDocumentFragment.call(document),other=document.implementation.createHTMLDocument('other'),otherFragment=other.createDocumentFragment();
if(borrowed.ownerDocument!==owner||main.ownerDocument!==document||otherFragment.ownerDocument!==other||!(otherFragment instanceof DocumentFragment))throw 'genuine borrowed factory owners';
var invalid=false;try{owner.createDocumentFragment.call({});}catch(error){invalid=error instanceof TypeError;}if(!invalid)throw 'factory brand';
if(f.appendChild(e)!==e||all.length!==0||items.length!==0||f.firstChild!==e||e.parentNode!==f||e.ownerDocument!==owner||f.children!==children||children[0]!==e||e.getRootNode()!==f)throw 'real child move';
var text=e.childNodes[0],comment=e.childNodes[1],bold=e.childNodes[2],nested=owner.createDocumentFragment();nested.appendChild(text);nested.appendChild(comment);
if(e.childNodes.length!==1||e.firstChild!==bold||nested.childNodes[0]!==text||nested.childNodes[1]!==comment||comment.nodeType!==8)throw 'mixed source movement';
if(e.appendChild(nested)!==nested||nested.childNodes.length!==0||text.parentNode!==e||comment.parentNode!==e||e.childNodes[0]!==bold||e.childNodes[1]!==text||e.childNodes[2]!==comment||comment.data!=='marker')throw 'atomic fragment splice';
if(e.removeChild(text)!==text||text.parentNode!==null||text.ownerDocument!==owner||text.getRootNode()!==text)throw 'real detach';e.appendChild(text);
content.appendChild(f);if(children.length!==0||content.firstChild!==e||e.parentNode!==content)throw 'restore genuine template content';f.appendChild(content);
var clone=f.cloneNode(true),copy=document.importNode(f,true);document.getElementById('mount').appendChild(copy);
if(copy.childNodes.length!==0||!document.getElementById('item').isConnected||f.firstChild!==e||e.isConnected||clone.firstChild===e||clone.firstChild.ownerDocument!==owner||clone.firstChild.childNodes[1].data!=='marker')throw 'genuine clone import and activation';
"#);
}
#[test]
fn inert_owner_fragment_fresh_tree_reorder_aliases_and_receiver_refusals(){
    let(mut doc,mut forms,mut page)=page();execute(&mut doc,&mut forms,&mut page,r#"
var owner=document.getElementById('seed').content.ownerDocument,f=owner.createDocumentFragment(),a=owner.createElement('div'),b=owner.createElement('span'),text=owner.createTextNode('fresh'),view=f.children;
a.setAttribute('data-note','one');var note=a.attributes[0];f.appendChild(a);f.appendChild(b);a.appendChild(text);f.appendChild(a);
if(view[0]!==b||view[1]!==a||text.parentNode!==a||a.firstChild!==text||note.ownerElement!==a||note.ownerDocument!==owner)throw 'fresh tree reorder aliases';
var caught=false;try{a.removeChild(b);}catch(error){caught=error.name==='NotFoundError';}if(!caught||b.parentNode!==f)throw 'remove real child only';
var other=document.implementation.createHTMLDocument('other'),foreign=other.createElement('p');caught=false;try{f.appendChild(foreign);}catch(error){caught=error.name==='NotSupportedError';}if(!caught||foreign.parentNode!==null||foreign.ownerDocument!==other||f.children.length!==2)throw 'cross owner boundary';
var otherFragment=other.createDocumentFragment();caught=false;try{otherFragment.appendChild(other.body);}catch(error){caught=error.name==='NotSupportedError';}if(!caught||other.body.parentNode!==other.documentElement||otherFragment.childNodes.length!==0)throw 'document structure boundary';
caught=false;try{Node.prototype.appendChild.call({},a);}catch(error){caught=error instanceof TypeError;}if(!caught||a.parentNode!==f)throw 'native receiver guard';
f.appendChild(owner.createDocumentFragment());if(f.children.length!==2)throw 'empty donor';
"#);
}
#[test]
fn inert_owner_fragment_child_growth_refusal_preserves_source_links_and_recovers(){
    let(mut doc,mut forms,mut page)=page();execute(&mut doc,&mut forms,&mut page,"var content=document.getElementById('seed').content,owner=content.ownerDocument,donor=owner.createDocumentFragment(),child=content.firstChild;donor.appendChild(child);");
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let arena=&bridge.state.inert_documents;
    let (child,parent)=arena.entries.iter().enumerate().find_map(|(index,entry)|if let Entry::Node(node)=entry{if node.parent.is_some()&&matches!(node.kind,Kind::Parsed{source:ParsedSource::Loaded(_),..})&&node.children.len()==3{Some((FIRST_ENTRY+index as u32,node.parent.unwrap()))}else{None}}else{None}).unwrap();
    let owner=bridge.state.inert_documents.node(parent).unwrap().owner;
    let Value::Host(target)=bridge.inert_create_node(owner,"createDocumentFragment",&[]).unwrap()else{panic!("fragment")};
    let retained=bridge.state.retained;let limit=bridge.state.retained_limit;let generation=bridge.state.dom_commits;
    // Ancestry/resource faults retain the existing escaped host-error path.
    assert!(bridge.mutate_inert_children(child,"appendChild",&[Value::Host(parent)]).unwrap_err().contains("cycle"));
    assert_eq!(bridge.state.retained,retained);assert_eq!(bridge.state.dom_commits,generation);
    assert_eq!(bridge.state.inert_documents.node(parent).unwrap().children,[child]);
    bridge.state.retained_limit=retained;
    assert!(bridge.mutate_inert_children(target,"appendChild",&[Value::Host(parent)]).is_err());
    assert_eq!(bridge.state.retained,retained);assert_eq!(bridge.state.dom_commits,generation);
    assert_eq!(bridge.state.inert_documents.node(parent).unwrap().children,[child]);assert_eq!(bridge.state.inert_documents.node(child).unwrap().parent,Some(parent));assert!(bridge.state.inert_documents.node(target).unwrap().children.is_empty());
    bridge.state.retained_limit=limit;
    let copy_work=bridge.state.copy_work;bridge.state.copy_work=0;
    assert!(bridge.mutate_inert_children(target,"appendChild",&[Value::Host(parent)]).unwrap_err().contains("work limit"));
    assert_eq!(bridge.state.retained,retained);assert_eq!(bridge.state.dom_commits,generation);
    assert_eq!(bridge.state.inert_documents.node(parent).unwrap().children,[child]);assert_eq!(bridge.state.inert_documents.node(target).unwrap().children.capacity(),0);
    bridge.state.copy_work=copy_work;
    let host_work=bridge.state.host_work;bridge.state.host_work=0;
    assert!(bridge.mutate_inert_children(target,"appendChild",&[Value::Host(parent)]).unwrap_err().contains("work limit"));
    assert_eq!(bridge.state.inert_documents.node(child).unwrap().parent,Some(parent));assert_eq!(bridge.state.dom_commits,generation);bridge.state.host_work=host_work;
    bridge.mutate_inert_children(target,"appendChild",&[Value::Host(parent)]).unwrap();
    assert!(bridge.state.inert_documents.node(parent).unwrap().children.is_empty());assert_eq!(bridge.state.inert_documents.node(child).unwrap().parent,Some(target));
}
