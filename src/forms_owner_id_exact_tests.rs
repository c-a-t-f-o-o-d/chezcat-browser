//! Native form ownership uses actual ID/form code units.
use super::*;
fn node(doc:&Document,id:&str)->usize{(0..doc.nodes.len()).find(|&n|doc.attr(n,"data-key")==Some(id)).unwrap()}
fn record(doc:&mut Document,n:usize,key:&str,units:&[u16]){let NodeKind::Element{attrs,..}=&mut doc.nodes[n].kind else{unreachable!()};attrs.iter_mut().find(|a|a.name.html_is(key)).unwrap().value=crate::dom_string::DomString::from_units(units,4096).unwrap();}
#[test]
fn form_owner_id_exact_literal_and_raw_external_keys_choose_actual_forms(){
 for raw_key in [false,true]{
  let mut doc=crate::html::parse("<!doctype html><html><body><form data-key=raw id='A�z'></form><form data-key=literal id='A�z'></form><input data-key=field form='A�z' name=q value=ready></body></html>");
  let raw=node(&doc,"raw");let literal=node(&doc,"literal");let field=node(&doc,"field");record(&mut doc,raw,"id",&[65,0xd800,122]);if raw_key{record(&mut doc,field,"form",&[65,0xd800,122]);}
  let forms=Forms::new(&doc);assert!(!forms.limited);assert_eq!(forms.controls[&field].owner,Some(if raw_key{raw}else{literal}),"raw key={raw_key}");
 }
}
#[test]
fn form_owner_id_exact_nonform_blockers_and_explicit_misses_do_not_fall_back(){
 let mut doc=crate::html::parse("<!doctype html><html><body><p data-key=alias id='A�z'></p><form data-key=literal id='A�z'></form><form data-key=outer id=outer><input data-key=field form='A�z'><input data-key=missing form=missing><input data-key=implicit></form></body></html>");
 let alias=node(&doc,"alias");let literal=node(&doc,"literal");let outer=node(&doc,"outer");let field=node(&doc,"field");let missing=node(&doc,"missing");let implicit=node(&doc,"implicit");record(&mut doc,alias,"id",&[65,0xd800,122]);
 let forms=Forms::new(&doc);assert_eq!(forms.controls[&field].owner,Some(literal));assert_eq!(forms.controls[&missing].owner,None);assert_eq!(forms.controls[&implicit].owner,Some(outer));
 record(&mut doc,alias,"id",&"A�z".encode_utf16().collect::<Vec<_>>());let forms=Forms::new(&doc);assert_eq!(forms.controls[&field].owner,None,"first exact nonform remains a blocker");
}
