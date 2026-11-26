//! Checked noscript fragment tokenizer state follows the actual scripting flag.
use super::*;
fn node(d:&Document,name:&str)->usize{let v:Vec<_>=d.nodes.iter().enumerate().filter_map(|(n,x)|matches!(&x.kind,NodeKind::Element{tag,..}if tag==name).then_some(n)).collect();assert_eq!(v.len(),1,"{name}");v[0]}
#[test]
fn noscript_fragment_context_enabled_context_is_literal_and_cannot_escape(){
 let source="<img id=fake src=missing>&amp;</noscript><b>tail</b>\r\n\0";
 let d=parse_fragment(source,"noscript",true).unwrap();assert!(d.scripting_enabled);assert_eq!(d.nodes.len(),2);assert_eq!(d.nodes[0].children,[1]);assert!(matches!(&d.nodes[1].kind,NodeKind::Text(t)if t==&source.replace("\r\n","\n").replace('\0',"\u{fffd}")));
 let empty=parse_fragment("","noscript",true).unwrap();assert_eq!(empty.nodes.len(),1);assert!(empty.nodes[0].children.is_empty());
}
#[test]
fn noscript_fragment_context_nested_enabled_raw_boundary_and_following_nodes(){
 for context in ["div","template"]{let raw="<img id=fake src=missing><script>bad()</script>&amp;</noscriptmore>";let source=format!("<noscript id=actual>{raw}</NOSCRIPT ><p id=after>After</p>");let d=parse_fragment(&source,context,true).unwrap();assert!(d.scripting_enabled);let n=node(&d,"noscript");let [child]=d.nodes[n].children.as_slice()else{panic!("actual raw text")};assert!(matches!(&d.nodes[*child].kind,NodeKind::Text(t)if t==raw));assert!(!d.nodes.iter().any(|x|matches!(&x.kind,NodeKind::Element{tag,..}if tag=="img"||tag=="script")));assert_eq!(d.attr(node(&d,"p"),"id"),Some("after"));}
}
#[test]
fn noscript_fragment_context_disabled_context_and_nested_markup_keep_real_elements(){
 let d=parse_fragment("<b id=real>&amp;</b>","noscript",false).unwrap();assert!(!d.scripting_enabled);assert_eq!(d.nodes[0].children,[node(&d,"b")]);assert_eq!(d.attr(node(&d,"b"),"id"),Some("real"));assert!(matches!(&d.nodes[d.nodes[node(&d,"b")].children[0]].kind,NodeKind::Text(t)if t=="&"));
 let nested=parse_fragment("<noscript><b id=real>Fallback</b></noscript>","div",false).unwrap();assert!(nested.nodes[node(&nested,"noscript")].children.contains(&node(&nested,"b")));
 assert!(parse_fragment("\0","noscript",false).is_err());
}
#[test]
fn noscript_fragment_context_actual_raw_storage_cancel_refusal_and_recovery(){
 let calls=std::cell::Cell::new(0);let source="z".repeat(128*1024);let check=||{calls.set(calls.get()+1);if calls.get()>=3{Err("cancel raw fragment".into())}else{Ok(())}};
 assert_eq!(parse_internal(&source,Some("noscript"),true,Some(LoadedTree{nodes:8,check_cancel:&check}),None).unwrap_err(),"cancel raw fragment");assert_eq!(calls.get(),3);
 let check=||Ok(());assert!(parse_internal("raw",Some("noscript"),true,Some(LoadedTree{nodes:1,check_cancel:&check}),None).unwrap_err().contains("node limit"));
 let good=parse_fragment("<noscript><b>literal</b></noscript><p>Recovered</p>","div",true).unwrap();assert_eq!(good.nodes[node(&good,"noscript")].children.len(),1);assert_eq!(good.nodes[node(&good,"p")].children.len(),1);
}
