//! Original real Bridge NodeLists; ordinary iteration controls precede Host spread.
use super::*;

fn id(doc:&Document,name:&str)->usize {
    (0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(name)).unwrap()
}
fn editor_state(forms:&Forms,node:usize)->(String,usize,usize,usize,bool,Option<usize>) {
    let c=&forms.controls[&node];
    (c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.owner)
}
fn no_resources(page:&mut Page) {
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let(a,b,c)=page.take_image_changes();assert!(a.is_empty()&&b.is_empty()&&c.is_empty());
}

fn original_spread(empty:bool) {
    let mut doc=crate::html::parse(concat!(
        "<!doctype html><title>Original NodeList iteration</title><body>",
        "<form id=owner><input id=neighbor name=keep value=original></form>",
        "<div id=root>lead<span id=a class=item>A</span><span id=b class=item>B</span>tail</div>"
    ));
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    let location=crate::navigation::address("https://node-list-iteration.test/original",&crate::navigation::Location::home()).unwrap();
    page.set_location(&location);page.set_resource_base(&location);
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0,"{}",page.report());
    let neighbor=id(&doc,"neighbor");
    forms.script_set_value(neighbor,"edited neighbor").unwrap();
    {let e=&mut forms.controls.get_mut(&neighbor).unwrap().value;e.cursor=5;e.anchor=2;e.offset=1;}
    let kept_editor=editor_state(&forms,neighbor);
    assert!(kept_editor.4);assert_eq!(kept_editor.5,Some(id(&doc,"owner")));
    assert_eq!(forms.text_default_value(neighbor).unwrap(),"original");

    let prefix=page.console_eval(&mut doc,&mut forms,r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),neighbor=document.getElementById('neighbor');
var snapshot=Element.prototype.querySelectorAll.call(root,'.item'),emptySnapshot=root.querySelectorAll('.third'),children=root.childNodes;
if(snapshot.length!==2||snapshot[0]!==a||snapshot[1]!==b||snapshot.item(0)!==a||snapshot.item(1)!==b||snapshot.item(2)!==null||snapshot[2]!==undefined||emptySnapshot.length!==0||emptySnapshot.item(0)!==null)throw 'real snapshot length/item/identity prefix';
if(children!==root.childNodes||children.length!==4||children[1]!==a||children[2]!==b||children.item(1)!==a)throw 'real live childNodes prefix';
var lead=children[0],tail=children[3];
if(lead.nodeType!==3||lead.data!=='lead'||tail.nodeType!==3||tail.data!=='tail'||lead.ownerDocument!==document||tail.ownerDocument!==document)throw 'retained physical Text identities';
var third=document.createElement('span');third.id='third';third.className='item third';third.textContent='C';
if(root.appendChild(third)!==third||third.parentNode!==root||third.ownerDocument!==document)throw 'real ordinary append ownership';
var fresh=root.querySelectorAll('.item');
if(snapshot.length!==2||snapshot[0]!==a||snapshot[1]!==b||emptySnapshot.length!==0||fresh===snapshot||fresh.length!==3||fresh[2]!==third||root.querySelectorAll('.third').item(0)!==third)throw 'static snapshot versus fresh query after mutation';
if(children!==root.childNodes||children.length!==5||children[0]!==lead||children[1]!==a||children[2]!==b||children[3]!==tail||children[4]!==third||root.lastChild!==third)throw 'live list must follow real physical append';
if(document.querySelectorAll('.item').length!==3||document.getElementById('third')!==third||neighbor.value!=='edited neighbor'||neighbor.defaultValue!=='original'||neighbor.getAttribute('value')!=='original'||neighbor.form!==document.getElementById('owner'))throw 'Document/native raw/default/owner prefix';
var conversions=0,rejected=0;
try{snapshot.item.call({}, {valueOf:function(){conversions++;return 0;}});throw 'NodeList-shaped receiver accepted';}catch(error){if(error.name!=='TypeError')throw error;rejected++;}
if(conversions||rejected!==1)throw 'existing NodeList brand must precede conversion';
var literal=[a,b],literalExpanded=[...literal],literalConcatenated=[].concat(...literal),literalEmpty=[...[]];
if(literalExpanded.length!==2||literalExpanded[0]!==a||literalExpanded[1]!==b||literalConcatenated.length!==2||literalConcatenated[0]!==a||literalConcatenated[1]!==b||literalEmpty.length!==0)throw 'ordinary array/spread/concat positive controls';
document.title='NodeList iteration positive prefix passed';
"#);
    assert_eq!(prefix.errors,0,"Positive prefix failed: {}",page.report());
    assert_eq!(doc.title,"NodeList iteration positive prefix passed");
    assert_eq!(editor_state(&forms,neighbor),kept_editor);
    assert_eq!(forms.text_default_value(neighbor).unwrap(),"original");
    assert_eq!(doc.attr(neighbor,"value"),Some("original"));
    no_resources(&mut page);

    let root=id(&doc,"root");let a=id(&doc,"a");let b=id(&doc,"b");let third=id(&doc,"third");
    let kept_children=doc.nodes[root].children.clone();
    assert_eq!(kept_children.len(),5);assert_eq!(kept_children[1],a);assert_eq!(kept_children[2],b);assert_eq!(kept_children[4],third);
    assert!(matches!(&doc.nodes[kept_children[0]].kind,NodeKind::Text(text) if text=="lead"));
    assert!(matches!(&doc.nodes[kept_children[3]].kind,NodeKind::Text(text) if text=="tail"));
    // Existing main-list owners and invalid-handle refusal are positive
    // security controls; an iterator must not bypass this same read boundary.
    {
        let bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
        for index in 0..bridge.state.node_lists.len() {
            assert_eq!(bridge.native_access_owner(NODE_LIST_BASE+index as u32),Some(WINDOW));
        }
        let invalid=NODE_LIST_BASE+u32::try_from(bridge.state.node_lists.len()).unwrap();
        assert!(!bridge.node_list_receiver(invalid));
        assert_eq!(bridge.native_access_owner(invalid),None);
    }
    let kept_nodes=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    let selection=if empty {"var selected=emptySnapshot;"} else {"var selected=snapshot;"};
    let expected=if empty {"if(expanded.length!==0)throw 'real empty snapshot spread';"} else {"if(expanded.length!==2||expanded[0]!==a||expanded[1]!==b)throw 'real nonempty static snapshot spread';"};
    let source=format!(r#"
{selection}
var expanded=[...selected];
{expected}
var concatenated=[].concat(...Element.prototype.querySelectorAll.call(root,'.item'));
var emptyConcatenated=[].concat(...Element.prototype.querySelectorAll.call(root,'.missing'));
var liveExpanded=[...children];
if(concatenated.length!==3||concatenated[0]!==a||concatenated[1]!==b||concatenated[2]!==third||emptyConcatenated.length!==0)throw 'canonical real NodeList call spread/concat';
if(liveExpanded.length!==5||liveExpanded[0]!==lead||liveExpanded[1]!==a||liveExpanded[2]!==b||liveExpanded[3]!==tail||liveExpanded[4]!==third||snapshot.length!==2||emptySnapshot.length!==0)throw 'live childNodes iteration or static snapshot retention';
"#);
    let spread=page.console_eval(&mut doc,&mut forms,&source);
    assert_eq!(format!("{:?}",doc.nodes),kept_nodes);
    assert_eq!(doc.nodes[root].children,kept_children);
    assert_eq!(page.state.dom_commits,commits);
    assert_eq!(editor_state(&forms,neighbor),kept_editor);
    assert_eq!(forms.text_default_value(neighbor).unwrap(),"original");
    assert_eq!(doc.attr(neighbor,"value"),Some("original"));
    no_resources(&mut page);
    assert_eq!(spread.errors,0,"empty={empty}\n{}",page.report());
}

#[test]
fn node_list_iteration_original_empty_real_snapshot_spread_after_positive_prefix() {
    original_spread(true);
}

#[test]
fn node_list_iteration_original_nonempty_real_snapshot_spread_after_positive_prefix() {
    original_spread(false);
}
