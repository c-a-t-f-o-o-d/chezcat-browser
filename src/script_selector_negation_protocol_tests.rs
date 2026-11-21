//! Invalid nested selector lists must not publish partial native snapshots.
use super::*;
#[test]
fn selector_negation_dom_invalid_nested_member_keeps_query_and_native_state_atomic() {
    let mut doc=crate::html::parse("<!doctype html><title>Negation atomicity</title><body><p id=safe class=safe>Safe</p><p id=target class=target>Target</p><form id=f><input id=neighbor name=keep value=original></form>");
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0,"{}",page.report());
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("neighbor")).unwrap();forms.script_set_value(node,"edited neighbor").unwrap();
    {let c=forms.controls.get_mut(&node).unwrap();c.value.cursor=5;c.value.anchor=2;c.value.offset=1;}
    let c=&forms.controls[&node];let editor=(c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.owner);
    let prefix=page.console_eval(&mut doc,&mut forms,"var safe=document.getElementById('safe'),target=document.getElementById('target'),saved=document.querySelectorAll('.safe,.target');if(saved.length!==2||saved[0]!==safe||saved[1]!==target)throw 'existing query prefix';");
    assert_eq!(prefix.errors,0,"{}",page.report());
    let count=page.state.node_lists.len();let refs=page.state.node_list_references;let nodes=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    let outcome=page.console_eval(&mut doc,&mut forms,r#"
var refused=0;
for(var selector of ['.safe,.target:not(.missing,:active)','.safe,.target:not(:not(.missing,:active))','.safe,.target:not(::before)']){
 try{document.querySelectorAll(selector);throw 'invalid list published a partial match';}catch(error){if(error.name!=='SyntaxError')throw error;refused++;}
}
if(refused!==3||saved.length!==2||saved[0]!==safe||saved[1]!==target||document.getElementById('neighbor').value!=='edited neighbor'||document.getElementById('neighbor').defaultValue!=='original')throw 'invalid list changed saved state';
"#);
    assert_eq!(outcome.errors,0,"{}",page.report());assert_eq!(page.state.node_lists.len(),count);assert_eq!(page.state.node_list_references,refs);
    assert_eq!(format!("{:?}",doc.nodes),nodes);assert_eq!(page.state.dom_commits,commits);
    let c=&forms.controls[&node];assert_eq!((c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.owner),editor);
    assert_eq!(forms.text_default_value(node).unwrap(),"original");assert_eq!(doc.attr(node,"value"),Some("original"));
    assert!(!page.has_navigation_request());assert!(page.take_dynamic_scripts().is_empty());assert!(page.take_dynamic_stylesheets().is_empty());
    let(a,b,c)=page.take_image_changes();assert!(a.is_empty()&&b.is_empty()&&c.is_empty());
}
