//! Checked standalone optgroup fragment scopes.
use super::*;
fn tag(d:&Document,n:usize)->&str{match &d.nodes[n].kind{NodeKind::Element{tag,..}=>tag,_=>""}}
#[test]
fn optgroup_fragments_balanced_metadata_text_and_comments(){for context in ["div","template","body"]{let d=parse_fragment("<optgroup id=g label='A&amp;B' disabled>Text<!--marker--></optgroup><span>After</span>",context,false).unwrap();let children=&d.nodes[0].children;assert_eq!(children.len(),2);let group=children[0];assert_eq!(tag(&d,group),"optgroup");assert_eq!(d.nodes[group].namespace,Namespace::Html);assert_eq!(d.attr(group,"label"),Some("A&B"));assert_eq!(d.attr(group,"disabled"),Some(""));assert_eq!(d.nodes[group].children.len(),2);assert!(matches!(&d.nodes[d.nodes[group].children[0]].kind,NodeKind::Text(t) if t.as_str()=="Text"));assert!(matches!(&d.nodes[d.nodes[group].children[1]].kind,NodeKind::Comment(t) if t.as_str()=="marker"));assert_eq!(tag(&d,children[1]),"span");}}
#[test]
fn optgroup_fragments_omitted_group_endings_keep_siblings(){let d=parse_fragment("<optgroup label=First>One<optgroup label=Second>Two</optgroup><p>After</p>","div",false).unwrap();let c=&d.nodes[0].children;assert_eq!(c.len(),3);assert_eq!(d.attr(c[0],"label"),Some("First"));assert_eq!(d.attr(c[1],"label"),Some("Second"));assert_eq!(d.nodes[c[0]].children.len(),1);assert_eq!(d.nodes[c[1]].children.len(),1);assert_eq!(tag(&d,c[2]),"p");}
#[test]
fn optgroup_fragments_balanced_passive_child_keeps_real_scope(){let d=parse_fragment("<div id=outer><optgroup label=Group><span>Child</span></optgroup><b>After</b></div>","template",true).unwrap();let outer=d.nodes[0].children[0];let group=d.nodes[outer].children[0];assert_eq!(d.attr(outer,"id"),Some("outer"));assert_eq!(tag(&d,group),"optgroup");assert_eq!(tag(&d,d.nodes[group].children[0]),"span");assert_eq!(tag(&d,d.nodes[outer].children[1]),"b");}
#[test]
fn optgroup_fragments_unsupported_unrelated_scope_pops_still_refuse(){for source in ["<optgroup><div><optgroup>","<optgroup><div></optgroup>","<optgroup><select></optgroup>","</optgroup>","<optgroup><option>One</option></optgroup>"]{let e=parse_fragment(source,"div",false).unwrap_err();assert!(e.contains("nonconventional select recovery"),"{source}: {e}");}}

#[test]
fn optgroup_fragments_preserve_script_nonce_metadata_and_comment_boundaries(){
 let d=parse_fragment("<optgroup><!--<optgroup><select>--><script nonce=a nonce=b>unexecuted()</script></optgroup><script nonce=c>later()</script>","div",true).unwrap();
 let group=d.nodes[0].children[0];assert_eq!(tag(&d,group),"optgroup");assert_eq!(d.nodes[group].children.len(),2);let first=d.nodes[group].children[1];let second=d.nodes[0].children[1];assert_eq!(tag(&d,first),"script");assert_eq!(tag(&d,second),"script");assert!(d.nonce_unsafe_nodes.contains(&first));assert!(!d.nonce_unsafe_nodes.contains(&second));assert_eq!(d.attr(second,"nonce"),Some("c"));
}
