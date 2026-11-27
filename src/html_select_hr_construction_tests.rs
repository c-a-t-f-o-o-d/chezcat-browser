//! Actual SELECT scope closes implied OPTION/OPTGROUP ends before HR insertion.
use super::*;
fn node(d:&Document,id:&str)->usize{(0..d.nodes.len()).find(|&n|d.attr(n,"id")==Some(id)).unwrap()}
fn tag(d:&Document,n:usize)->&str{match &d.nodes[n].kind{NodeKind::Element{tag,..}=>tag,_=>""}}
const SOURCE:&str="<select id=menu><optgroup id=group label=Group disabled><option id=first value=a>A<hr id=separator><option id=second value=b selected>B</select><p id=after>After</p>";
fn actual_tree(d:&Document){let menu=node(d,"menu");let group=node(d,"group");let first=node(d,"first");let hr=node(d,"separator");let second=node(d,"second");assert_eq!(d.nodes[menu].children,vec![group,hr,second]);assert_eq!(d.nodes[group].children,vec![first]);assert_eq!(d.nodes[first].children.len(),1);assert_eq!(tag(d,hr),"hr");assert_eq!(d.nodes[hr].namespace,Namespace::Html);assert!(d.nodes[hr].children.is_empty());assert_eq!(d.nodes[second].children.len(),1);assert_eq!(d.nodes[menu].children.iter().filter(|&&n|tag(d,n)=="hr").count(),1);}
#[test]
fn select_hr_construction_loaded_document_closes_option_and_group_before_void_separator(){
 for scripting in [false,true]{let d=parse_loaded_document_with_scripting(SOURCE,scripting,&||Ok(())).unwrap();actual_tree(&d);let after=node(&d,"after");assert!(!d.nodes[node(&d,"menu")].children.contains(&after));assert_eq!(d.attr(node(&d,"second"),"selected"),Some(""));}
}
#[test]
fn select_hr_construction_checked_actual_select_fragment_keeps_exact_siblings_and_void_flags(){
 for scripting in [false,true]{for source in [SOURCE.to_owned(),SOURCE.replace("<hr id=separator>","<hr id=separator data-label='A&amp;B' />")]{let d=parse_fragment(&source,"div",scripting).unwrap();actual_tree(&d);assert_eq!(d.nodes[0].children,vec![node(&d,"menu"),node(&d,"after")]);if source.contains("data-label"){assert_eq!(d.attr(node(&d,"separator"),"data-label"),Some("A&B"));}}}
}
#[test]
fn select_hr_construction_only_actual_select_scope_changes_ends_and_retains_fragment_guards(){
 let d=parse_loaded_document_with_scripting("<div><optgroup id=g><option id=o>A<hr id=h>B</option></optgroup></div>",true,&||Ok(())).unwrap();assert!(d.nodes[node(&d,"o")].children.contains(&node(&d,"h")));assert_eq!(d.nodes[node(&d,"g")].children,vec![node(&d,"o")]);
 for(source,context)in[("<option>A<hr><option>B","select"),("<option>A<hr>","optgroup"),("A<hr>","option"),("<select><option><b>rich<hr></select>","div"),("<select><option>A<hr><script>bad()</script></select>","div"),("<select><option>A<hr><input></select>","div"),("<select><option>A</hr></select>","div")]{assert!(parse_fragment(source,context,true).is_err(),"{context}: {source}");}
 let too_many=format!("<select>{}</select>","<hr>".repeat(MAX_NODES));assert!(parse_fragment(&too_many,"div",true).is_err());
}
#[test]
fn select_hr_construction_native_form_option_group_disabledness_and_selected_value_follow_tree(){
 let d=parse_loaded_document_with_scripting(SOURCE,true,&||Ok(())).unwrap();let forms=crate::forms::Forms::new(&d);let menu=node(&d,"menu");let state=forms.controls[&menu].select.as_ref().unwrap();assert_eq!(state.options.len(),2);assert_eq!(state.options[0].node,node(&d,"first"));assert_eq!(state.options[0].group,Some(node(&d,"group")));assert!(state.options[0].disabled);assert_eq!(state.options[1].node,node(&d,"second"));assert_eq!(state.options[1].group,None);assert!(!state.options[1].disabled);assert_eq!(state.selected,Some(1));assert_eq!(state.options[1].value,"b");assert_eq!(state.options[1].label,"B");assert_eq!(state.rows.len(),3,"HR is not an OPTION or an additional selectable native row");
}
