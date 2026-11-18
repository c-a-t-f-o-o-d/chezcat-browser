//! Original sibling topology with retained Text and an explicitly created Comment.
use super::*;

fn id(doc:&Document,name:&str)->usize {
    (0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(name)).unwrap()
}

#[test]
fn element_siblings_original_interleaved_text_and_created_comment_keep_real_identity() {
    let mut doc=crate::html::parse("<!doctype html><title>Element sibling seed</title><body><div id=box>lead<span id=a>A</span>middle<span id=b>B</span>tail</div>");
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    let location=crate::navigation::address("https://element-sibling.test/source",&crate::navigation::Location::home()).unwrap();
    page.set_location(&location);page.set_resource_base(&location);
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0,"{}",page.report());
    let prefix=page.console_eval(&mut doc,&mut forms,r#"
var box=document.getElementById('box'),a=document.getElementById('a'),b=document.getElementById('b');
var lead=box.firstChild,middle=a.nextSibling,tail=b.nextSibling,marker=document.createComment('between sibling elements');
box.insertBefore(marker,b);
if(lead.nodeType!==3||lead.data!=='lead'||middle.nodeType!==3||middle.data!=='middle'||tail.nodeType!==3||tail.data!=='tail')throw 'retained physical Text prefix';
if(marker.nodeType!==8||marker.data!=='between sibling elements'||marker.ownerDocument!==document||marker.parentNode!==box)throw 'genuine created Comment prefix';
if(a.nextSibling!==middle||middle.nextSibling!==marker||marker.nextSibling!==b||b.previousSibling!==marker||marker.previousSibling!==middle||middle.previousSibling!==a)throw 'old physical sibling identities';
if(box.firstChild!==lead||lead.previousSibling!==null||tail.nextSibling!==null||a.parentNode!==box||b.parentNode!==box)throw 'old physical boundary prefix';
document.title='Element sibling physical prefix reached';
"#);
    assert_eq!(prefix.errors,0,"{}",page.report());
    assert_eq!(doc.title,"Element sibling physical prefix reached");
    let box_node=id(&doc,"box");
    let children=doc.nodes[box_node].children.clone();
    assert_eq!(children.len(),6);
    assert!(matches!(&doc.nodes[children[0]].kind,NodeKind::Text(value) if value=="lead"));
    assert_eq!(children[1],id(&doc,"a"));
    assert!(matches!(&doc.nodes[children[2]].kind,NodeKind::Text(value) if value=="middle"));
    assert!(matches!(&doc.nodes[children[3]].kind,NodeKind::Comment(value) if value=="between sibling elements"));
    assert_eq!(children[4],id(&doc,"b"));
    assert!(matches!(&doc.nodes[children[5]].kind,NodeKind::Text(value) if value=="tail"));

    let outcome=page.console_eval(&mut doc,&mut forms,r#"
if(a.nextElementSibling!==b||b.previousElementSibling!==a)throw 'Element siblings must skip Text and Comment';
if(middle.previousElementSibling!==a||middle.nextElementSibling!==b||marker.previousElementSibling!==a||marker.nextElementSibling!==b)throw 'CharacterData uses the same real sibling tree';
if(a.previousElementSibling!==null||b.nextElementSibling!==null||lead.previousElementSibling!==null||lead.nextElementSibling!==a||tail.previousElementSibling!==b||tail.nextElementSibling!==null)throw 'element sibling boundary nulls';
if('nextElementSibling' in document||'previousElementSibling' in document||document.nextElementSibling!==undefined||document.previousElementSibling!==undefined)throw 'Document does not implement this mixin';
if(a.nextSibling!==middle||middle.nextSibling!==marker||marker.nextSibling!==b||b.previousSibling!==marker||marker.parentNode!==box)throw 'element-only reads changed physical topology';
document.title='Element sibling identities passed';
"#);
    assert_eq!(doc.nodes[box_node].children,children);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"Element sibling identities passed");
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let(changed,removed,pending)=page.take_image_changes();
    assert!(changed.is_empty()&&removed.is_empty()&&pending.is_empty());
}
