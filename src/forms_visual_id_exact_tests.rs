//! Exact native label association keys retain tree-order identity.
use super::*;
fn node(doc:&Document,id:&str)->usize{(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some(id)).unwrap()}
fn record(doc:&mut Document,n:usize,name:&str,units:&[u16]){let NodeKind::Element{attrs,..}=&mut doc.nodes[n].kind else{unreachable!()};attrs.iter_mut().find(|a|a.name.html_is(name)).unwrap().value=crate::dom_string::DomString::from_units(units,4096).unwrap();}
#[test]
fn visual_id_exact_label_keys_distinguish_raw_from_literal_and_follow_first_tree_match(){
    let mut doc=crate::html::parse("<!doctype html><body><label id=label for='A�z'>label</label><input id='A�z'><input id=second></body>");let label=node(&doc,"label");let first=node(&doc,"A�z");let second=node(&doc,"second");record(&mut doc,first,"id",&[65,0xd800,122]);record(&mut doc,second,"id",&"A�z".encode_utf16().collect::<Vec<_>>());
    assert_eq!(label_targets(&doc).get(&label),Some(&second));record(&mut doc,label,"for",&[65,0xd800,122]);assert_eq!(label_targets(&doc).get(&label),Some(&first));record(&mut doc,second,"id",&[65,0xd800,122]);assert_eq!(label_targets(&doc).get(&label),Some(&first));
}
#[test]
fn visual_id_exact_label_noncontrol_first_match_and_explicit_misses_do_not_fall_back(){
    let mut doc=crate::html::parse("<!doctype html><body><label id=label for='A�z'><input id=implicit></label><div id='A�z'></div><input id=second></body>");let label=node(&doc,"label");let blocker=node(&doc,"A�z");let second=node(&doc,"second");record(&mut doc,blocker,"id",&[65,0xd800,122]);record(&mut doc,second,"id",&"A�z".encode_utf16().collect::<Vec<_>>());assert_eq!(label_targets(&doc).get(&label),Some(&second));
    record(&mut doc,label,"for",&[65,0xd800,122]);assert!(!label_targets(&doc).contains_key(&label));record(&mut doc,label,"for",&[65,0xdfff,122]);assert!(!label_targets(&doc).contains_key(&label));
}
