//! Stable, bounded Attr identities. Expanded names identify entries; qualified
//! names select the first entry for the ordinary HTML attribute APIs.
use super::*;
use crate::html::{AttributeName, AttributeNamespace};
use crate::dom_string::DomStringRef;

pub(super) const ATTR_BASE: u32 = 0x8200_0000;
pub(super) fn node_method(method:&str)->bool{matches!(method,"getAttributeNode"|"getAttributeNodeNS"|"setAttributeNode"|"setAttributeNodeNS"|"removeAttributeNode")}
pub(super) fn factory_operation(method:&str)->Option<bool>{match method{crate::javascript::DOCUMENT_CREATE_ATTRIBUTE=>Some(false),crate::javascript::DOCUMENT_CREATE_ATTRIBUTE_NS=>Some(true),_=>None}}
#[cfg(test)]
mod factory_storage_tests{
 use super::*;
 #[test]
 fn inert_attribute_factory_exact_owner_name_backing_copy_refusal_and_recovery(){
  let mut doc=crate::html::parse("<!doctype html><html><body></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let Value::Host(owner)=b.create_xml_document(script_inert_documents::MAIN_IMPLEMENTATION,&[Value::Null,Value::String(String::new())]).unwrap()else{panic!("owner")};
  let backing=|b:&Bridge<'_>|b.state.attributes.capacity()*size_of::<Attr>()+b.state.attached_attributes.capacity()*size_of::<usize>();
  for (namespaced,args)in[(false,vec![Value::String("MiXeD".into())]),(true,vec![Value::String("urn:original".into()),Value::String("p:Case".into())]),(true,vec![Value::StringUnits(vec![0xd800,0x41,0xdfff]),Value::String("p:Units".into())]),(true,vec![Value::String("urn:original".into()),Value::StringUnits(vec![0xd800,0x3a,0x41,0xdfff])]),(false,vec![Value::StringUnits(vec![0x41,0xd800,0x42])])]{let before=(b.state.retained,backing(&b));let Value::Host(handle)=b.create_attribute_for_document(owner,&args,namespaced).unwrap()else{panic!("real Attr")};let attr=&b.state.attributes[index(handle).unwrap()];assert_eq!(attr.document,owner);assert_eq!(attr.owner,None);assert_eq!(b.state.retained-before.0,attr.name.owned_bytes()+attr.value.capacity()+backing(&b)-before.1);}
  let before=(b.state.retained,backing(&b),b.state.attributes.len(),b.state.attached_attributes.len(),b.state.dom_commits);let limit=b.state.retained_limit;let copies=b.state.copy_work;
  for storage in [true,false]{if storage{b.state.retained_limit=b.state.retained;}else{b.state.copy_work=0;}assert!(b.create_attribute_for_document(owner,&[Value::String("Fresh".into())],false).is_err());assert!(b.create_attribute_for_document(owner,&[Value::StringUnits(vec![0xd800,0x41,0xdfff]),Value::String("p:Refused".into())],true).is_err());assert_eq!((b.state.retained,backing(&b),b.state.attributes.len(),b.state.attached_attributes.len(),b.state.dom_commits),before);b.state.retained_limit=limit;b.state.copy_work=copies;}
  let Value::Host(handle)=b.create_attribute_for_document(owner,&[Value::String("Fresh".into())],false).unwrap()else{panic!("recovery")};let attr=&b.state.attributes[index(handle).unwrap()];assert_eq!(attr.document,owner);assert_eq!(attr.name.qualified,"Fresh");
 }
 #[test]
 fn attribute_clone_actual_name_value_registry_backing_storage_copy_refusal_and_recovery(){
  let mut doc=crate::html::parse("<html><body></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let Value::Host(source)=b.create_attribute_for_document(DOCUMENT,&[Value::StringUnits(vec![0xd800]),Value::StringUnits(vec![0xd800,0x3a,0x41,0xdfff])],true).unwrap()else{panic!("source")};b.attr_set(source,"value",Value::StringUnits(vec![0xd800,0x41,0xdfff])).unwrap();let source_id=index(source).unwrap();let backing=|b:&Bridge<'_>|b.state.attributes.capacity()*size_of::<Attr>()+b.state.attached_attributes.capacity()*size_of::<usize>();let before=(b.state.retained,backing(&b),b.state.attributes.len(),b.state.attached_attributes.len(),b.state.dom_commits);let limit=b.state.retained_limit;let copies=b.state.copy_work;
  for storage in [true,false]{if storage{b.state.retained_limit=before.0;}else{b.state.copy_work=0;}assert!(b.clone_attribute_node(source,None).is_err());assert_eq!((b.state.retained,backing(&b),b.state.attributes.len(),b.state.attached_attributes.len(),b.state.dom_commits),before);assert_eq!(b.state.attributes[source_id].owner,None);assert!(b.state.attributes[source_id].value.code_units().eq([0xd800,0x41,0xdfff]));b.state.retained_limit=limit;b.state.copy_work=copies;}
  let Value::Host(copy)=b.clone_attribute_node(source,None).unwrap()else{panic!("copy")};assert_ne!(copy,source);let cloned=&b.state.attributes[index(copy).unwrap()];assert_eq!(cloned.owner,None);assert_eq!(cloned.document,DOCUMENT);assert!(cloned.name.same_expanded_name(&b.state.attributes[source_id].name));assert!(cloned.value.code_units().eq([0xd800,0x41,0xdfff]));assert_eq!(b.state.retained,before.0+cloned.name.owned_bytes()+cloned.value.capacity()+backing(&b)-before.1);assert_eq!(b.state.attached_attributes.len(),before.3);
 }

}
pub(super) fn member_operation(method:&str)->Option<(&str,bool)>{let(name,setter)=if let Some(name)=method.strip_prefix("\0Attr.get."){(name,false)}else if method=="\0Attr.set.value"{("value",true)}else{return None;};matches!(name,"namespaceURI"|"prefix"|"localName"|"name"|"value"|"ownerElement"|"specified").then_some((name,setter))}
const ATTR_HANDLES: usize = 0x0100_0000;

pub(super) struct Attr {
    pub(super) owner: Option<u32>,
    pub(super) document: u32,
    pub(super) name: AttributeName,
    // Attached values stay in the element. Removal moves the actual DOMString here.
    pub(super) value: DomString,
}

pub(super) fn index(handle: u32) -> Option<usize> {
    (ATTR_BASE..ATTR_BASE + ATTR_HANDLES as u32).contains(&handle).then(|| (handle - ATTR_BASE) as usize)
}

// Inert construction can retain any DOMString. Native materialization admits
// exact IDs, common HTML metadata and ordinary names through native consumers.
pub(super) fn passive_name_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{
    namespace==crate::html::Namespace::Html && name.html_is("name")
        && !matches!(tag,"slot"|"meter"|"progress"|"data"|"q") && (matches!(tag,"output"|"fieldset"|"form")||script_clone_node::ordinary_clone_element(tag))
}
pub(super) fn html_label_or_output_for_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{
    namespace==crate::html::Namespace::Html && matches!(tag,"label"|"output") && name.html_is("for")
}
pub(super) fn html_time_datetime_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="time"&&name.html_is("datetime")}
pub(super) fn html_area_alt_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="area"&&name.html_is("alt")}
pub(super) fn html_optgroup_label_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="optgroup"&&name.html_is("label")}
pub(super) fn html_option_label_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="option"&&name.html_is("label")}
pub(super) fn html_option_value_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="option"&&name.html_is("value")}
pub(super) fn html_select_name_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="select"&&name.html_is("name")}
pub(super) fn html_button_name_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="button"&&name.html_is("name")}
pub(super) fn html_input_name_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="input"&&name.html_is("name")}
pub(super) fn html_textarea_name_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="textarea"&&name.html_is("name")}
pub(super) fn html_area_coords_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="area"&&name.html_is("coords")}
pub(super) fn html_area_shape_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="area"&&name.html_is("shape")}
pub(super) fn html_area_download_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="area"&&name.html_is("download")}
pub(super) fn html_area_type_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="area"&&name.html_is("type")}
pub(super) fn html_area_hreflang_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="area"&&name.html_is("hreflang")}
pub(super) fn html_area_target_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="area"&&name.html_is("target")}
pub(super) fn html_area_rel_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="area"&&name.html_is("rel")}
pub(super) fn html_area_href_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="area"&&name.html_is("href")}
pub(super) fn html_area_referrer_policy_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="area"&&name.html_is("referrerpolicy")}
pub(super) fn html_area_ping_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="area"&&name.html_is("ping")}
pub(super) fn html_map_name_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="map"&&name.html_is("name")}
pub(super) fn html_param_metadata_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="param"&&(name.html_is("value")||name.html_is("type")||name.html_is("valuetype"))}
pub(super) fn html_param_name_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="param"&&name.html_is("name")}
pub(super) fn html_mod_datetime_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&matches!(tag,"ins"|"del")&&name.html_is("datetime")}
pub(super) fn html_div_align_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="div"&&name.html_is("align")}
pub(super) fn html_legend_align_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="legend"&&name.html_is("align")}
pub(super) fn html_paragraph_align_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="p"&&name.html_is("align")}
pub(super) fn html_hr_align_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="hr"&&name.html_is("align")}
pub(super) fn html_hr_color_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="hr"&&name.html_is("color")}
pub(super) fn html_font_size_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="font"&&name.html_is("size")}
pub(super) fn html_font_face_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="font"&&name.html_is("face")}
pub(super) fn html_font_color_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="font"&&name.html_is("color")}
pub(super) fn html_br_clear_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="br"&&name.html_is("clear")}
pub(super) fn html_hr_size_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="hr"&&name.html_is("size")}
pub(super) fn html_hr_width_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="hr"&&name.html_is("width")}
pub(super) fn html_ul_compact_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="ul"&&name.html_is("compact")}
pub(super) fn html_dir_compact_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="dir"&&name.html_is("compact")}
pub(super) fn html_menu_compact_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="menu"&&name.html_is("compact")}
pub(super) fn html_ol_compact_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="ol"&&name.html_is("compact")}
pub(super) fn html_hr_no_shade_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="hr"&&name.html_is("noshade")}
pub(super) fn html_ol_reversed_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="ol"&&name.html_is("reversed")}
pub(super) fn html_area_no_href_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="area"&&name.html_is("nohref")}
pub(super) fn html_option_disabled_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="option"&&name.html_is("disabled")}
pub(super) fn html_select_disabled_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="select"&&name.html_is("disabled")}
pub(super) fn html_textarea_disabled_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="textarea"&&name.html_is("disabled")}
pub(super) fn html_textarea_required_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="textarea"&&name.html_is("required")}
pub(super) fn html_textarea_read_only_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="textarea"&&name.html_is("readonly")}
pub(super) fn html_select_required_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="select"&&name.html_is("required")}
pub(super) fn html_option_default_selected_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="option"&&name.html_is("selected")}
pub(super) fn html_optgroup_disabled_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="optgroup"&&name.html_is("disabled")}
pub(super) fn html_dl_compact_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="dl"&&name.html_is("compact")}
pub(super) fn html_ul_type_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="ul"&&name.html_is("type")}
pub(super) fn html_li_type_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="li"&&name.html_is("type")}
pub(super) fn html_ol_type_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="ol"&&name.html_is("type")}
pub(super) fn html_li_value_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="li"&&name.html_is("value")}
pub(super) fn html_pre_width_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&matches!(tag,"pre"|"listing"|"xmp")&&name.html_is("width")}
pub(super) fn html_textarea_dimensions_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="textarea"&&(name.html_is("cols")||name.html_is("rows"))}
pub(super) fn html_textarea_max_length_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="textarea"&&name.html_is("maxlength")}
pub(super) fn html_textarea_min_length_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="textarea"&&name.html_is("minlength")}
pub(super) fn html_textarea_wrap_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="textarea"&&name.html_is("wrap")}
pub(super) fn html_textarea_autocomplete_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="textarea"&&name.html_is("autocomplete")}
pub(super) fn html_select_autocomplete_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="select"&&name.html_is("autocomplete")}
pub(super) fn html_input_autocomplete_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="input"&&name.html_is("autocomplete")}
pub(super) fn html_input_form_action_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="input"&&name.html_is("formaction")}
pub(super) fn html_input_form_method_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="input"&&name.html_is("formmethod")}
pub(super) fn html_input_form_enctype_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="input"&&name.html_is("formenctype")}
pub(super) fn html_button_form_action_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="button"&&name.html_is("formaction")}
pub(super) fn html_button_form_method_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="button"&&name.html_is("formmethod")}
pub(super) fn html_button_form_enctype_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="button"&&name.html_is("formenctype")}
pub(super) fn html_form_autocomplete_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="form"&&name.html_is("autocomplete")}
pub(super) fn html_form_method_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="form"&&name.html_is("method")}
pub(super) fn html_form_action_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="form"&&name.html_is("action")}
pub(super) fn html_form_enctype_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="form"&&name.html_is("enctype")}
pub(super) fn html_form_accept_charset_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="form"&&name.html_is("accept-charset")}
pub(super) fn html_form_rel_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="form"&&name.html_is("rel")}
pub(super) fn html_form_target_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="form"&&name.html_is("target")}
pub(super) fn html_button_form_target_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="button"&&name.html_is("formtarget")}
pub(super) fn html_input_form_target_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="input"&&name.html_is("formtarget")}
pub(super) fn html_form_no_validate_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="form"&&name.html_is("novalidate")}
pub(super) fn html_button_form_no_validate_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="button"&&name.html_is("formnovalidate")}
pub(super) fn html_input_form_no_validate_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="input"&&name.html_is("formnovalidate")}
pub(super) fn html_ol_start_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="ol"&&name.html_is("start")}
pub(super) fn html_heading_align_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&matches!(tag,"h1"|"h2"|"h3"|"h4"|"h5"|"h6")&&name.html_is("align")}
pub(super) fn html_data_value_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="data"&&name.html_is("value")}
pub(super) fn html_meter_number_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="meter"&&["value","min","max","low","high","optimum"].iter().any(|key|name.html_is(key))}
pub(super) fn html_progress_number_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{namespace==crate::html::Namespace::Html&&tag=="progress"&&(name.html_is("value")||name.html_is("max"))}
pub(super) fn html_output_form_attribute(namespace:crate::html::Namespace,tag:&str,name:&AttributeName)->bool{
    namespace==crate::html::Namespace::Html && matches!(tag,"output"|"fieldset") && name.html_is("form")
}
pub(super) fn html_aria_attribute(namespace:crate::html::Namespace,name:&AttributeName)->bool{
    namespace==crate::html::Namespace::Html
        && crate::javascript::aria_reflection_interface::ATTRIBUTES.iter().any(|(_,key,_,_)|name.html_is(key))
}
pub(super) fn materializable_value(attr:&crate::html::Attribute,namespace:crate::html::Namespace,tag:&str)->bool{
    attr.value.raw_units().is_none()
        ||html_aria_attribute(namespace,&attr.name)
        ||html_label_or_output_for_attribute(namespace,tag,&attr.name)
        ||html_output_form_attribute(namespace,tag,&attr.name)
        ||html_time_datetime_attribute(namespace,tag,&attr.name)
        ||(html_area_alt_attribute(namespace,tag,&attr.name)||(html_optgroup_label_attribute(namespace,tag,&attr.name)||(html_option_label_attribute(namespace,tag,&attr.name)||(html_option_value_attribute(namespace,tag,&attr.name)||(((html_select_name_attribute(namespace,tag,&attr.name)||html_button_name_attribute(namespace,tag,&attr.name))||html_input_name_attribute(namespace,tag,&attr.name))||html_textarea_name_attribute(namespace,tag,&attr.name))))))
        ||html_area_coords_attribute(namespace,tag,&attr.name)
        ||html_area_shape_attribute(namespace,tag,&attr.name)
        ||html_area_download_attribute(namespace,tag,&attr.name)
        ||html_area_type_attribute(namespace,tag,&attr.name)
        ||html_area_hreflang_attribute(namespace,tag,&attr.name)
        ||html_area_target_attribute(namespace,tag,&attr.name)
        ||html_area_rel_attribute(namespace,tag,&attr.name)
        ||html_area_href_attribute(namespace,tag,&attr.name)
        ||html_area_referrer_policy_attribute(namespace,tag,&attr.name)
        ||html_area_ping_attribute(namespace,tag,&attr.name)
        ||(html_map_name_attribute(namespace,tag,&attr.name)||(html_param_name_attribute(namespace,tag,&attr.name)||html_param_metadata_attribute(namespace,tag,&attr.name)))
        ||html_mod_datetime_attribute(namespace,tag,&attr.name)
        ||html_data_value_attribute(namespace,tag,&attr.name)
        ||html_heading_align_attribute(namespace,tag,&attr.name)
        ||html_ul_type_attribute(namespace,tag,&attr.name)
        ||html_li_type_attribute(namespace,tag,&attr.name)
        ||html_ol_type_attribute(namespace,tag,&attr.name)
        ||html_li_value_attribute(namespace,tag,&attr.name)
        ||(html_pre_width_attribute(namespace,tag,&attr.name)||(html_textarea_dimensions_attribute(namespace,tag,&attr.name)||(html_textarea_max_length_attribute(namespace,tag,&attr.name)||(html_textarea_min_length_attribute(namespace,tag,&attr.name)||(html_textarea_wrap_attribute(namespace,tag,&attr.name)||(html_textarea_autocomplete_attribute(namespace,tag,&attr.name)||(html_select_autocomplete_attribute(namespace,tag,&attr.name)||((html_input_autocomplete_attribute(namespace,tag,&attr.name)||((html_input_form_action_attribute(namespace,tag,&attr.name)||(html_input_form_method_attribute(namespace,tag,&attr.name)||html_input_form_enctype_attribute(namespace,tag,&attr.name)))||(html_button_form_action_attribute(namespace,tag,&attr.name)||(html_button_form_method_attribute(namespace,tag,&attr.name)||html_button_form_enctype_attribute(namespace,tag,&attr.name)))))||(((html_form_autocomplete_attribute(namespace,tag,&attr.name)||((html_form_method_attribute(namespace,tag,&attr.name)||html_form_action_attribute(namespace,tag,&attr.name))||html_form_enctype_attribute(namespace,tag,&attr.name)))||((html_form_accept_charset_attribute(namespace,tag,&attr.name)||html_form_rel_attribute(namespace,tag,&attr.name))||((html_form_target_attribute(namespace,tag,&attr.name)||html_button_form_target_attribute(namespace,tag,&attr.name))||html_input_form_target_attribute(namespace,tag,&attr.name))))||((html_form_no_validate_attribute(namespace,tag,&attr.name)||html_button_form_no_validate_attribute(namespace,tag,&attr.name))||html_input_form_no_validate_attribute(namespace,tag,&attr.name)))))))))))
        ||html_ol_start_attribute(namespace,tag,&attr.name)
        ||html_ul_compact_attribute(namespace,tag,&attr.name)
        ||html_dir_compact_attribute(namespace,tag,&attr.name)
        ||html_menu_compact_attribute(namespace,tag,&attr.name)
        ||html_ol_compact_attribute(namespace,tag,&attr.name)
        ||html_hr_no_shade_attribute(namespace,tag,&attr.name)
        ||html_ol_reversed_attribute(namespace,tag,&attr.name)
        ||(html_area_no_href_attribute(namespace,tag,&attr.name)||(((html_option_disabled_attribute(namespace,tag,&attr.name)||((html_select_disabled_attribute(namespace,tag,&attr.name)||(html_textarea_disabled_attribute(namespace,tag,&attr.name)||(html_textarea_required_attribute(namespace,tag,&attr.name)||html_textarea_read_only_attribute(namespace,tag,&attr.name))))||html_select_required_attribute(namespace,tag,&attr.name)))||html_option_default_selected_attribute(namespace,tag,&attr.name))||html_optgroup_disabled_attribute(namespace,tag,&attr.name)))
        ||html_dl_compact_attribute(namespace,tag,&attr.name)
        ||html_paragraph_align_attribute(namespace,tag,&attr.name)
        ||html_hr_align_attribute(namespace,tag,&attr.name)
        ||html_hr_color_attribute(namespace,tag,&attr.name)
        ||html_font_size_attribute(namespace,tag,&attr.name)
        ||html_font_face_attribute(namespace,tag,&attr.name)
        ||html_font_color_attribute(namespace,tag,&attr.name)
        ||html_br_clear_attribute(namespace,tag,&attr.name)
        ||html_hr_size_attribute(namespace,tag,&attr.name)
        ||html_hr_width_attribute(namespace,tag,&attr.name)
        ||html_legend_align_attribute(namespace,tag,&attr.name)
        ||html_div_align_attribute(namespace,tag,&attr.name)
        ||html_meter_number_attribute(namespace,tag,&attr.name)
        ||html_progress_number_attribute(namespace,tag,&attr.name)
        ||namespace==crate::html::Namespace::Html&&["id","class","title","lang","dir"].iter().any(|name|attr.name.html_is(name))
        ||passive_name_attribute(namespace,tag,&attr.name)
        ||attr.name.namespace_uri().is_some_and(|uri|uri=="http://www.w3.org/2000/xmlns/")
        ||attr.name.namespace.is_empty()&&attr.name.qualified.starts_with("data-")
}

fn owned_name_bytes(name:&AttributeName)->usize {
    name.owned_bytes()
}
fn copy_text(text:&str)->Result<String,String>{
    let mut copied=String::new();copied.try_reserve_exact(text.len()).map_err(|_|"Attr text storage reservation failed.")?;
    copied.push_str(text);Ok(copied)
}
pub(super) fn copy_name(name:&AttributeName)->Result<AttributeName,String>{
    Ok(AttributeName{qualified:name.qualified.try_clone()?,namespace:AttributeName::namespace_storage(name.namespace.first().map(|ns|Ok::<_,String>(AttributeNamespace{uri:ns.uri.try_clone()?,prefix:ns.prefix.as_ref().map(DomString::try_clone).transpose()?,local:ns.local.try_clone()?})).transpose()?)?})
}
struct Reservation {records:Option<Vec<Attr>>,index:Option<Vec<usize>>,added:usize}
pub(super) enum InputPreparation {Pending,Ready(Option<forms::InputPlan>)}
impl State {
    pub(super) fn attribute_lookup_work(&mut self)->Result<(),String>{self.work((usize::BITS-self.attached_attributes.len().leading_zeros()) as usize+1)}
    pub(super) fn attribute_position(&self,node:usize,name:&AttributeName)->Result<usize,usize>{self.attribute_owner_position(node_handle(node),name)}
    pub(super) fn attribute_owner_position(&self,owner:u32,name:&AttributeName)->Result<usize,usize>{
        self.attached_attributes.binary_search_by(|&id|{
            let attr=&self.attributes[id];
            (attr.owner.unwrap(),attr.name.namespace_uri(),attr.name.local()).cmp(&(owner,name.namespace_uri(),name.local()))
        })
    }
}

fn property(name: &str) -> bool {
    dom_tree::node_constant(name).is_some() || matches!(name,
        "name" | "localName" | "nodeName" | "namespaceURI" | "prefix" | "value"
        | "nodeValue" | "textContent" | "ownerElement" | "ownerDocument" | "specified"
        | "nodeType" | "parentNode" | "parentElement" | "firstChild" | "lastChild"
        | "nextSibling" | "previousSibling" | "isConnected" | "isDefaultNamespace" | "compareDocumentPosition")
}

pub(super) fn namespace(args:&[Value])->Result<Option<DomStringRef<'_>>,String>{
    match args.first(){Some(Value::Null|Value::Undefined)=>Ok(None),Some(Value::String(s))=>Ok((!s.is_empty()).then_some(DomStringRef::Scalar(s))),Some(Value::StringUnits(u))=>Ok((!u.is_empty()).then_some(DomStringRef::Raw(u))),_=>Err("Attribute operation requires a converted nullable DOMString namespace.".into())}
}

fn name_text<'a>(name: &'a AttributeName, property: &str) -> Option<&'a DomString> {
    match property {
        "localName" => Some(name.local()),
        "namespaceURI" => None,
        "prefix" => name.prefix(),
        _ => Some(&name.qualified),
    }
}

impl Bridge<'_> {
    pub(super) fn canonical_attribute_node_member(&self,object:u32,property:&str)->bool{node_method(property)&&(self.inert_node_type(object)==Some(1)||self.attr_element_receiver(object))&&!self.form_named_control(object,property)}
    pub(super) fn set_inert_attribute_node(&mut self,object:u32,args:&[Value])->Result<Value,String>{
        self.validate_attr_attachment(object,args).map_err(|(_,message)|message)?;let Value::Host(handle)=args[0]else{unreachable!()};let incoming=index(handle).unwrap();if self.state.attributes[incoming].owner==Some(object){return Ok(Value::Host(handle));}
        let Value::Host(document)=self.inert_get(object,"ownerDocument")?else{return Err("Inert attachment document is unavailable.".into());};
        let attr=&self.state.attributes[incoming];let released=attr.value.capacity();self.state.copies(attr.name.bytes().checked_add(attr.value.used_bytes()).ok_or("Inert Attr input copy overflow.")?)?;let name=copy_name(&self.state.attributes[incoming].name)?;let value=self.state.attributes[incoming].value.try_clone()?;
        let attrs=self.element_attribute_source(object).ok_or("Inert attachment Element is unavailable.")?;let work=attrs.iter().fold(attrs.len(),|n,attr|n.saturating_add(attr.name.bytes()));self.state.work(work)?;let slot=self.element_attribute_source(object).unwrap().iter().position(|attr|attr.name.same_expanded_name(&name));self.state.attribute_lookup_work()?;let position=self.state.attribute_owner_position(object,&name);let old_id=position.ok().map(|at|self.state.attached_attributes[at]);let new_old=slot.is_some()&&old_id.is_none();
        let (old_name,old_value)=if let Some(slot)=slot{let old=&self.element_attribute_source(object).unwrap()[slot];self.state.copies(old.value.used_bytes().checked_add(if new_old{old.name.bytes()}else{0}).ok_or("Inert Attr old copy overflow.")?)?;let old=&self.element_attribute_source(object).unwrap()[slot];(if new_old{Some(copy_name(&old.name)?)}else{None},old.value.try_clone()?)}else{(None,DomString::new())};
        let added=old_value.capacity().checked_add(old_name.as_ref().map_or(0,owned_name_bytes)).ok_or("Inert Attr identity payload overflow.")?;let retained=self.state.retained;let occupied=retained.checked_add(added).and_then(|n|n.checked_sub(released)).ok_or("Inert Attr identity accounting overflow.")?;let reservation=self.prepare_attribute_storage(usize::from(new_old),usize::from(old_id.is_none()),occupied)?;let adjusted=occupied.checked_add(reservation.added).ok_or("Inert Attr backing accounting overflow.")?;
        self.state.copies(self.state.attached_attributes.len().checked_mul(2*size_of::<usize>()).and_then(|n|n.checked_add(usize::from(new_old)*size_of::<Attr>())).and_then(|n|n.checked_add(2*size_of::<u32>()+size_of::<usize>())).ok_or("Inert Attr index movement overflow.")?)?;
        self.state.retained=adjusted;if let Err(error)=self.write_inert_attribute_node_value(object,name,value){self.state.retained=retained;return Err(error);}
        // The source and every identity buffer are admitted. No callbacks or
        // fallible work separate owner/value/index publication.
        self.commit_attribute_storage(reservation);let insert=match position{Ok(at)=>{self.state.attached_attributes.remove(at);at},Err(at)=>at};
        let result=if let Some(old)=old_id{self.state.attributes[old].owner=None;self.state.attributes[old].value=old_value;Value::Host(ATTR_BASE+old as u32)}else if let Some(name)=old_name{let old=ATTR_BASE+self.state.attributes.len() as u32;self.state.attributes.push(Attr{owner:None,document,name,value:old_value});Value::Host(old)}else{Value::Null};
        self.state.attributes[incoming].owner=Some(object);self.state.attributes[incoming].document=document;self.state.attributes[incoming].value=DomString::new();self.state.attached_attributes.insert(insert,incoming);Ok(result)
    }
    pub(super) fn canonical_attr_member(&self,object:u32,property:&str)->bool{self.attr_receiver(object)&&matches!(property,"namespaceURI"|"prefix"|"localName"|"name"|"value"|"ownerElement"|"specified")}
    pub(super) fn canonical_attribute_factory_member(&self,object:u32,property:&str)->bool{matches!(property,"createAttribute"|"createAttributeNS")&&(object==DOCUMENT||self.inert_document(object))}
    pub(super) fn validate_attribute_factory(&self,document:u32,args:&[Value],namespaced:bool)->Result<(),(&'static str,String)>{
        if document!=DOCUMENT&&!self.inert_document(document){return Err(("TypeError","Attribute creation requires a genuine Document.".into()));}
        if let Some(value)=args.get(usize::from(namespaced)){let qualified=script_attribute_names::converted_name(value)?;if namespaced{script_attribute_names::validate_namespace_dom(args.first().unwrap(),qualified)?;}else if !script_attribute_names::valid_attribute_name_dom(qualified){return Err(("InvalidCharacterError","Invalid attribute local name.".into()));}}

        Ok(())
    }
    pub(super) fn prepare_attr_document_rebind(&mut self,mut matches_owner:impl FnMut(u32)->bool,lookup:usize,document:u32)->Result<Vec<usize>,String>{
        let length=self.state.attached_attributes.len();self.state.work(length.checked_mul(lookup.saturating_add(1)).ok_or("Attr document lookup work overflow.")?)?;
        let mut changes=Vec::new();
        for at in 0..length{let id=self.state.attached_attributes[at];let attr=&self.state.attributes[id];if attr.document!=document&&attr.owner.is_some_and(&mut matches_owner){self.state.copies(size_of::<usize>()+size_of::<u32>())?;changes.try_reserve(1).map_err(|_|"Cannot reserve Attr document changes.")?;changes.push(id);}}
        Ok(changes)
    }
    pub(super) fn commit_attr_document_rebind(&mut self,changes:&[usize],document:u32){for &id in changes{self.state.attributes[id].document=document;}}
    fn prepare_attribute_storage(&mut self,records:usize,index:usize,occupied:usize)->Result<Reservation,String>{
        let length=self.state.attributes.len();
        #[cfg(test)]
        if records!=0&&self.state.limits.test_entry_arenas&&length>=self.state.limits.host.attribute_identities {
            return Err(format!("Attr identity limit reached ({} per document).",self.state.limits.host.attribute_identities));
        }
        if records!=0&&length>=ATTR_HANDLES{return Err("Attr native identity space exhausted.".into());}
        let available=self.state.retained_limit.checked_sub(occupied).ok_or("DOM retained Attr payload allowance exhausted.")?;
        let index_min=if index!=0&&self.state.attached_attributes.len()==self.state.attached_attributes.capacity(){size_of::<usize>()}else{0};
        let (record_backing,record_bytes)=retained_storage::grow(&self.state.attributes,records,available.checked_sub(index_min).ok_or("DOM retained Attr/index allowance exhausted.")?)?;
        let (index_backing,index_bytes)=retained_storage::grow(&self.state.attached_attributes,index,available-record_bytes)?;
        if record_backing.is_some(){self.state.copies(length*size_of::<Attr>())?;}
        if index_backing.is_some(){self.state.copies(self.state.attached_attributes.len()*size_of::<usize>())?;}
        Ok(Reservation{records:record_backing,index:index_backing,added:record_bytes+index_bytes})
    }
    fn commit_attribute_storage(&mut self,reservation:Reservation){
        if let Some(mut next)=reservation.records{next.append(&mut self.state.attributes);self.state.attributes=next;}
        if let Some(mut next)=reservation.index{next.append(&mut self.state.attached_attributes);self.state.attached_attributes=next;}
    }

    pub(super) fn create_attribute(&mut self,args:&[Value],namespaced:bool)->Result<Value,String> {
        self.create_attribute_for_document(DOCUMENT,args,namespaced)
    }
    fn prepare_attribute_name(&mut self,args:&[Value],namespaced:bool)->Result<AttributeName,String>{
        let qualified=self.dom_text(args.get(usize::from(namespaced)).ok_or("Attribute creation requires a name.")?)?;
        if !namespaced{return Ok(AttributeName::unqualified_dom(qualified));}
        script_attribute_names::validate_namespace_dom(args.first().unwrap(),qualified.view()).map_err(|(_,message)|message)?;
        let Some(uri)=namespace(args)?else{return Ok(AttributeName::unqualified_dom(qualified));};
        self.state.copies(uri.copy_bytes()?.checked_add(qualified.used_bytes()).ok_or("Attribute expanded name copy overflow.")?)?;
        let(prefix,local)=qualified.view().split_colon();let prefix=prefix.map(DomStringRef::try_owned).transpose()?;let local=local.try_owned()?;let uri=uri.try_owned()?;
        Ok(AttributeName{qualified,namespace:AttributeName::namespace_storage(Some(AttributeNamespace{uri,prefix,local}))?})
    }
    pub(super) fn create_attribute_for_document(&mut self,document:u32,args:&[Value],namespaced:bool)->Result<Value,String>{
        self.validate_attribute_factory(document,args,namespaced).map_err(|(_,message)|message)?;
        let mut name=self.prepare_attribute_name(args,namespaced)?;
        if !namespaced&&self.inert_document_kind_for_node(document).unwrap_or(self.doc.kind)==crate::html::DocumentKind::Html{name.qualified.make_ascii_lowercase();}
        let payload=owned_name_bytes(&name);
        let occupied=self.state.retained.checked_add(payload).ok_or("Attr retained storage overflow.")?;
        let reservation=self.prepare_attribute_storage(1,0,occupied)?;let added=reservation.added;
        self.commit_attribute_storage(reservation);self.state.retained=occupied+added;
        let handle=ATTR_BASE+self.state.attributes.len() as u32;
        self.state.attributes.push(Attr{owner:None,document,name,value:DomString::new()});self.state.dom_committed();
        Ok(Value::Host(handle))
    }

    /// An Attr is a native leaf: copy the actual name/value, never its owner.
    /// New identity backing and payload are admitted before publication.
    pub(super) fn clone_attribute_node(&mut self,source:u32,document:Option<u32>)->Result<Value,String>{
        let id=index(source).filter(|&id|id<self.state.attributes.len()).ok_or("Attr clone requires a genuine Attr.")?;
        let attr=&self.state.attributes[id];let document=document.unwrap_or(attr.document);if document!=DOCUMENT&&!self.inert_document(document){return Err("Attr clone document is unavailable.".into());}
        let owner=attr.owner;let name_bytes=attr.name.bytes();
        let slot=if let Some(owner)=owner{let attrs=self.element_attribute_source(owner).ok_or("Attr clone owner is unavailable.")?;let work=attrs.iter().fold(attrs.len(),|n,a|n.saturating_add(a.name.bytes()));self.state.work(work)?;Some(self.element_attribute_source(owner).unwrap().iter().position(|a|a.name.same_expanded_name(&self.state.attributes[id].name)).ok_or("Attr clone owner value is missing.")?)}else{None};
        let value=if let(Some(owner),Some(slot))=(owner,slot){&self.element_attribute_source(owner).unwrap()[slot].value}else{&self.state.attributes[id].value};
        self.state.copies(name_bytes.checked_add(value.used_bytes()).and_then(|n|n.checked_add(size_of::<Attr>())).ok_or("Attr clone copy overflow.")?)?;
        let name=copy_name(&self.state.attributes[id].name)?;let value=if let(Some(owner),Some(slot))=(owner,slot){self.element_attribute_source(owner).unwrap()[slot].value.try_clone()?}else{self.state.attributes[id].value.try_clone()?};
        let payload=name.owned_bytes().checked_add(value.capacity()).ok_or("Attr clone payload overflow.")?;let occupied=self.state.retained.checked_add(payload).ok_or("Attr clone retained overflow.")?;let reservation=self.prepare_attribute_storage(1,0,occupied)?;let added=reservation.added;
        self.commit_attribute_storage(reservation);self.state.retained=occupied+added;let handle=ATTR_BASE+self.state.attributes.len() as u32;self.state.attributes.push(Attr{owner:None,document,name,value});self.state.dom_committed();Ok(Value::Host(handle))
    }
    pub(super) fn validate_attr_attachment(&self,object:u32,args:&[Value])->Result<(),(&'static str,String)> {
        let Some(Value::Host(handle))=args.first() else {return Err(("TypeError","setAttributeNode requires a genuine Attr.".into()));};
        let Some(attr)=index(*handle).and_then(|id|self.state.attributes.get(id)) else {return Err(("TypeError","setAttributeNode requires a genuine Attr.".into()));};
        if attr.owner.is_some() && attr.owner!=Some(object) {return Err(("InUseAttributeError","Attr already belongs to another Element.".into()));}
        Ok(())
    }

    /// The element value/effects commit once through the existing checked path.
    /// All identity/name/value staging precedes it; nothing fallible follows it.
    pub(super) fn set_attribute_node(&mut self,object:u32,args:&[Value])->Result<Value,String> {
        if self.inert_node_type(object)==Some(1){return self.set_inert_attribute_node(object,args);}
        self.validate_attr_attachment(object,args).map_err(|(_,message)|message)?;
        let node=self.attr_element_node(object)?;
        let Value::Host(handle)=args[0] else {unreachable!()};let incoming=index(handle).unwrap();
        if self.state.attributes[incoming].owner==Some(node_handle(node)) {return Ok(Value::Host(handle));}
        let attr=&self.state.attributes[incoming];
        let active_name=attr.name.namespace.is_empty() && !attr.name.qualified.contains(':');
        if active_name && (self.dom_script(node) && matches!(attr.name.qualified.as_str(),"src"|"type"|"nonce"|"integrity"|"crossorigin"|"referrerpolicy"|"fetchpriority"|"async"|"defer"|"nomodule")
            || self.dom_stylesheet(node) && matches!(attr.name.qualified.as_str(),"href"|"rel"|"type"|"media"|"as"|"nonce"|"integrity"|"crossorigin"|"referrerpolicy"|"fetchpriority"|"disabled")) {
            return Err("Attribute-node adoption for active script/stylesheet settings is not implemented; no attribute was changed.".into());
        }
        let name_bytes=attr.name.bytes();let value_bytes=attr.value.used_bytes();let value_storage=attr.value.capacity();
        self.state.copies(name_bytes.checked_mul(2).and_then(|n|n.checked_add(value_bytes)).ok_or("Attribute copy accounting overflow.")?)?;
        let element_name=copy_name(&self.state.attributes[incoming].name)?;let write_name=copy_name(&element_name)?;let value=self.state.attributes[incoming].value.try_clone()?;
        let slot=self.attribute_slot(node,element_name.namespace_uri().map(DomString::view),element_name.local().view())?;
        self.state.attribute_lookup_work()?;
        let position=self.state.attribute_position(node,&element_name);
        let old_id=position.ok().map(|position|self.state.attached_attributes[position]);
        let (old_name,old_value,old_name_storage,old_value_storage,old_id_storage)=if let Some(slot)=slot {
            let NodeKind::Element{attrs,id,..}=&self.doc.nodes[node].kind else {unreachable!()};let old=&attrs[slot];
            self.state.copies(old.name.bytes().checked_add(old.value.used_bytes()).ok_or("Attribute copy accounting overflow.")?)?;
            (Some(copy_name(&old.name)?),old.value.try_clone()?,old.name.owned_bytes(),old.value.capacity(),if element_name.html_is("id"){id.as_ref().map_or(0,String::capacity)}else{0})
        } else {(None,DomString::new(),0,0,0)};
        let new_old_identity=slot.is_some()&&old_id.is_none();
        let removed=value_storage.checked_add(old_name_storage).ok_or("Attribute accounting overflow.")?;
        let copied_name_storage=element_name.owned_bytes();
        let added=old_value.capacity().checked_add(if slot.is_some(){copied_name_storage}else{0}).and_then(|n|n.checked_add(if new_old_identity{old_name.as_ref().map_or(0,owned_name_bytes)}else{0})).ok_or("Attribute accounting overflow.")?;
        let retained_before=self.state.retained;
        let public_old=old_value_storage.checked_add(old_name_storage).and_then(|bytes|bytes.checked_add(old_id_storage)).ok_or("Attribute accounting overflow.")?;
        let public_new=value.capacity().checked_add(if slot.is_some(){old_name_storage}else{write_name.owned_bytes()}).and_then(|bytes|bytes.checked_add(if element_name.html_is("id"){value.len()}else{0})).ok_or("Attribute accounting overflow.")?;
        let total_removed=removed.checked_add(public_old).ok_or("Attribute accounting overflow.")?;
        let total_added=added.checked_add(public_new).ok_or("Attribute accounting overflow.")?;
        let retained_after=if total_added>=total_removed {retained_before.checked_add(total_added-total_removed)}else{retained_before.checked_sub(total_removed-total_added)}.ok_or("Attribute retained accounting overflow/underflow.")?;
        if retained_after>self.state.retained_limit {return Err("DOM retained attribute growth allowance reached.".into());}
        let input=self.prepare_input_attribute(node,&element_name,Some(&value))?;
        let native_bytes=input.as_ref().map_or(0,forms::InputPlan::retained_bytes);
        let dom_backing=if slot.is_none(){let NodeKind::Element{attrs,..}=&self.doc.nodes[node].kind else{unreachable!()};if attrs.len()==attrs.capacity(){size_of::<crate::html::Attribute>()}else{0}}else{0};
        let occupied=retained_after.checked_add(native_bytes).and_then(|bytes|bytes.checked_add(dom_backing)).ok_or("Attr/native INPUT/element retained storage overflow.")?;
        let reservation=self.prepare_attribute_storage(usize::from(new_old_identity),usize::from(old_id.is_none()),occupied)?;
        let backing=reservation.added;
        self.state.copies(self.state.attached_attributes.len().saturating_mul(2*size_of::<usize>()))?;
        self.state.copies(added)?;
        // Stage the complete identity transfer before the checked element/native
        // write. Its final quota check must include both identity growth and
        // any newly admitted native INPUT storage. A shrinking element write
        // can offset a temporary positive transfer balance.
        self.state.retained=if added+backing>=removed {retained_before.checked_add(added+backing-removed)}else{retained_before.checked_sub(removed-added-backing)}
            .ok_or("Attribute retained transfer overflow/underflow.")?;
        if let Err(error)=self.public_dom_attribute(node,write_name,Some(value),InputPreparation::Ready(input)) {self.state.retained=retained_before;return Err(error);}
        if let Some(slot)=slot {let NodeKind::Element{attrs,..}=&mut self.doc.nodes[node].kind else {unreachable!()};attrs[slot].name=element_name;}
        self.commit_attribute_storage(reservation);
        let insert=match position{Ok(position)=>{self.state.attached_attributes.remove(position);position},Err(position)=>position};
        let result=if let Some(old_id)=old_id {
            self.state.attributes[old_id].owner=None;self.state.attributes[old_id].value=old_value;
            Value::Host(ATTR_BASE+old_id as u32)
        } else if let Some(old_name)=old_name {
            let handle=ATTR_BASE+self.state.attributes.len() as u32;
            self.state.attributes.push(Attr{owner:None,document:DOCUMENT,name:old_name,value:old_value});Value::Host(handle)
        } else {Value::Null};
        self.state.attributes[incoming].owner=Some(node_handle(node));self.state.attributes[incoming].document=DOCUMENT;self.state.attributes[incoming].value=DomString::new();
        self.state.attached_attributes.insert(insert,incoming);
        Ok(result)
    }
    pub(super) fn attr_native_member(&self, name: &str) -> bool {
        property(name) || dom_tree::native_node_member(name)
    }
    pub(super) fn attr_receiver(&self, handle: u32) -> bool {
        index(handle).is_some_and(|id| self.state.attributes.get(id).is_some())
    }

    pub(super) fn attr_readonly(&self, handle: u32, name: &str) -> bool {
        self.attr_receiver(handle) && property(name)
            && !matches!(name, "value" | "nodeValue" | "textContent" | "isDefaultNamespace" | "compareDocumentPosition")
    }

    pub(super) fn attr_has(&self, handle: u32, name: &str) -> Result<bool, String> {
        if !self.attr_receiver(handle) { return Err("Invalid Attr handle.".into()); }
        Ok(property(name) || self.method(handle, name).is_some())
    }

    fn attribute_slot(&mut self, node: usize, namespace: Option<DomStringRef<'_>>, local: DomStringRef<'_>) -> Result<Option<usize>, String> {
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { return Err("Not an Element.".into()); };
        self.state.work(attrs.len())?;
        self.state.copies(attrs.iter().map(|attr| attr.name.bytes()).sum())?;
        Ok(attrs.iter().position(|attr| attr.name.matches_names(namespace, local)))
    }

    pub(super) fn raw_attribute_slot(&mut self, node: usize, qualified: &DomString) -> Result<Option<usize>, String> {
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { return Err("Not an Element.".into()); };
        self.state.work(attrs.len())?;
        self.state.copies(attrs.iter().map(|attr| attr.name.qualified.len()).sum())?;
        Ok(attrs.iter().position(|attr| attr.name.qualified == *qualified))
    }

    pub(super) fn method_qualified_attribute_name(&mut self, node: usize, _method: &str, mut name: DomString)
        -> Result<DomString, String>
    {
        // Keep the existing qualified-name normalization for each operation.
        if self.doc.nodes[node].namespace==crate::html::Namespace::Html&&self.doc.kind==crate::html::DocumentKind::Html{name.make_ascii_lowercase();}Ok(name)
    }

    pub(super) fn attribute_observation_plan(&mut self, object: u32, method: &str, args: &[Value])
        -> Result<Option<crate::javascript::NativeAttributeChange>, String>
    {
        if matches!(method,"setAttributeNode"|"setAttributeNodeNS"|"removeAttributeNode"){
            let Some(Value::Host(handle))=args.first() else{return Ok(None);};let Some(id)=index(*handle).filter(|&id|self.state.attributes.get(id).is_some())else{return Ok(None);};
            let bytes=self.state.attributes[id].name.bytes();self.state.copies(bytes)?;let name=copy_name(&self.state.attributes[id].name)?;
            return self.attribute_change_for_name(object,&name).map(Some);
        }
        if self.inert_node_type(object)==Some(1){return self.inert_attribute_observation_plan(object,method,args);}
        if !self.attr_element_receiver(object) || self.is_frame_handle(object) { return Ok(None); }
        let node = self.attr_element_node(object)?;
        let (slot, local, uri, removal) = match method {
            "setAttribute" | "removeAttribute" => {
                let Some(name) = args.first() else { return Ok(None); };
                let text = self.dom_text(name)?;
                let qualified = self.method_qualified_attribute_name(node, method, text)?;
                let slot = self.raw_attribute_slot(node, &qualified)?;
                (slot, qualified, None, method == "removeAttribute")
            }
            "setAttributeNS" | "removeAttributeNS" => {
                let Some(name) = args.get(1) else { return Ok(None); };
                let uri = namespace(args)?;
                let mut local = self.dom_text(name)?;
                if method == "setAttributeNS" && uri.is_some() {
                    if let (Some(_),name)=local.view().split_colon(){self.state.copies(name.copy_bytes()?)?;local=name.try_owned()?;}
                }
                let slot = self.attribute_slot(node, uri, local.view())?;
                (slot, local, uri, method == "removeAttributeNS")
            }
            _ => return Ok(None),
        };
        if removal && slot.is_none() { return Ok(None); }
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else {
            return Err("Attribute observation requires a genuine Element.".into());
        };
        let prior = slot.and_then(|index| attrs.get(index));
        let local=prior.map_or(local.view(),|attr|attr.name.local().view());
        let uri = prior.map_or(uri, |attr| attr.name.namespace_uri().map(DomString::view));
        let old = prior.map(|attr| &attr.value);
        self.state.copies(local.copy_bytes()?.saturating_add(uri.map(DomStringRef::copy_bytes).transpose()?.unwrap_or(0)).saturating_add(old.map_or(0,DomString::logical_bytes)))?;
        Ok(Some(crate::javascript::NativeAttributeChange {
            local_name: local.try_owned()?,
            namespace: uri.map(DomStringRef::try_owned).transpose()?,
            old_value: old.map(Self::dom_string_copy).transpose()?,
        }))
    }

    pub(super) fn copy_attribute_name(&mut self, node: usize, slot: usize) -> Result<AttributeName, String> {
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { return Err("Not an Element.".into()); };
        let name = &attrs.get(slot).ok_or("Attribute slot is unavailable.")?.name;
        self.state.copies(name.bytes())?;
        name.copy()
    }

    pub(super) fn attr_get(&mut self, handle: u32, name: &str) -> Result<Value, String> {
        if name=="childNodes"{return self.create_attribute_child_list(handle);}
        let id = index(handle).ok_or("Invalid Attr handle.")?;
        let attr = self.state.attributes.get(id).ok_or("Invalid Attr handle.")?;
        if let Some(value) = dom_tree::node_constant(name) { return Ok(Value::Number(f64::from(value))); }
        if name=="namespaceURI"{let Some(uri)=attr.name.namespace_uri()else{return Ok(Value::Null);};let bytes=uri.logical_bytes();if bytes>self.state.limits.host.string_bytes{return Err("Attr namespace exceeds the host allowance.".into());}self.state.copies(bytes)?;return Self::dom_string_copy(self.state.attributes[id].name.namespace_uri().unwrap());}
        match name {
            "name" | "localName" | "nodeName" | "namespaceURI" | "prefix" => {
                let Some(text) = name_text(&attr.name, name) else { return Ok(Value::Null); };
                if text.logical_bytes() > self.state.limits.host.string_bytes { return Err("Attr name exceeds the host string bound.".into()); }
                self.state.copies(text.logical_bytes())?;
                Self::dom_string_copy(name_text(&self.state.attributes[id].name, name).unwrap())
            }
            "value" | "nodeValue" | "textContent" => {
                let location=if let Some(owner)=attr.owner {
                    let attrs=self.element_attribute_source(owner).ok_or("Invalid Attr owner.")?;
                    let work=attrs.len();let names=attrs.iter().map(|attr|attr.name.bytes()).sum();
                    self.state.work(work)?;self.state.copies(names)?;
                    let slot=self.element_attribute_source(owner).ok_or("Invalid Attr owner.")?.iter().position(|entry|entry.name.same_expanded_name(&self.state.attributes[id].name)).ok_or("Attached Attr is missing from its owner.")?;
                    Some((owner,slot))
                }else{None};
                let text=if let Some((owner,slot))=location{&self.element_attribute_source(owner).ok_or("Invalid Attr owner.")?[slot].value}else{&self.state.attributes[id].value};
                let length=text.logical_bytes();if length>self.state.limits.host.string_bytes{return Err("Attr value exceeds the host string bound.".into());}
                self.state.copies(length)?;
                let text=if let Some((owner,slot))=location{&self.element_attribute_source(owner).ok_or("Invalid Attr owner.")?[slot].value}else{&self.state.attributes[id].value};
                Self::dom_string_copy(text)
            }
            "ownerElement" => Ok(attr.owner.map_or(Value::Null,Value::Host)),
            "ownerDocument" => {
                Ok(Value::Host(attr.document))
            },
            "nodeType" => Ok(Value::Number(2.0)),
            "specified" => Ok(Value::Bool(true)),
            "isConnected" => Ok(Value::Bool(false)),
            "parentNode" | "parentElement" | "firstChild" | "lastChild"
            | "previousSibling" | "nextSibling" => Ok(Value::Null),
            "then" | "" => Ok(Value::Undefined),
            _ => Err(format!("Attr property {name:?} is not implemented.")),
        }
    }

    pub(super) fn attr_set(&mut self, handle: u32, property: &str, value: Value) -> Result<(), String> {
        let id = index(handle).filter(|&id| self.state.attributes.get(id).is_some()).ok_or("Invalid Attr handle.")?;
        if !matches!(property, "value" | "nodeValue" | "textContent") {
            return Err("Attr property is read-only or unsupported.".into());
        }
        let value = self.dom_text(&value)?;
        if let Some(owner) = self.state.attributes[id].owner {
            self.state.copies(self.state.attributes[id].name.bytes())?;
            let name=copy_name(&self.state.attributes[id].name)?;
            if self.inert_node_type(owner)==Some(1){self.write_inert_dom_attribute(owner,name,Some(value),true)?;}
            else{let node=node_index(owner).ok_or("Attr owner is unavailable.")?;self.public_dom_attribute(node,name,Some(value),InputPreparation::Pending)?;}
        } else {
            self.state.replace_storage(self.state.attributes[id].value.capacity(), value.capacity(),value.used_bytes())?;
            self.state.attributes[id].value = value;
            self.state.dom_committed();
        }
        Ok(())
    }

    pub(super) fn attr_element_node(&mut self, object: u32) -> Result<usize, String> {
        let node = self.node(object)?;
        if !matches!(self.doc.nodes[node].namespace, crate::html::Namespace::Html | crate::html::Namespace::Svg) {
            return Err("Attr namespace provenance for foreign parser contexts is unavailable.".into());
        }
        Ok(node)
    }

    pub(super) fn lookup_attribute_node_at(&mut self, node: usize, slot: Option<usize>) -> Result<Value, String> {
        self.lookup_owner_attribute_node_at(node_handle(node),slot)
    }
    pub(super) fn lookup_owner_attribute_node_at(&mut self,owner:u32,slot:Option<usize>)->Result<Value,String>{
        let Some(slot)=slot else{return Ok(Value::Null);};
        let name=&self.element_attribute_source(owner).ok_or("Invalid Attr owner.")?.get(slot).ok_or("Missing attribute slot.")?.name;
        let copy_work=name.bytes();
        self.state.attribute_lookup_work()?;
        let name=&self.element_attribute_source(owner).ok_or("Invalid Attr owner.")?[slot].name;
        let position=self.state.attribute_owner_position(owner,name);
        let position=match position{Ok(position)=>return Ok(Value::Host(ATTR_BASE+self.state.attached_attributes[position] as u32)),Err(position)=>position};
        self.state.copies(copy_work)?;
        let name=copy_name(&self.element_attribute_source(owner).ok_or("Invalid Attr owner.")?[slot].name)?;
        let document=if self.inert_node_type(owner).is_some(){let Value::Host(document)=self.inert_get(owner,"ownerDocument")?else{return Err("Inert Attr document is unavailable.".into());};document}else{DOCUMENT};
        let occupied=self.state.retained.checked_add(owned_name_bytes(&name)).ok_or("Attr retained storage overflow.")?;
        let reservation=self.prepare_attribute_storage(1,1,occupied)?;let added=reservation.added;
        self.state.copies((self.state.attached_attributes.len()-position)*size_of::<usize>())?;
        self.commit_attribute_storage(reservation);self.state.retained=occupied+added;
        let id=self.state.attributes.len();let handle=ATTR_BASE+id as u32;
        self.state.attributes.push(Attr{owner:Some(owner),document,name,value:DomString::new()});self.state.attached_attributes.insert(position,id);Ok(Value::Host(handle))
    }

    pub(super) fn get_attribute_node(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        if self.inert_node_type(object)==Some(1){
            let mut text=self.dom_text(args.first().ok_or("getAttributeNode requires a name.")?)?;
            if self.inert_document_kind_for_node(object)==Some(crate::html::DocumentKind::Html)&&matches!(self.inert_get(object,"namespaceURI")?,Value::String(ref uri) if uri=="http://www.w3.org/1999/xhtml"){text.make_ascii_lowercase();}
            let attrs=self.element_attribute_source(object).ok_or("Inert Element attributes are unavailable.")?;
            let work=attrs.iter().fold(attrs.len(),|work,attr|work.saturating_add(attr.name.qualified.len()));
            self.state.work(work)?;let slot=self.element_attribute_source(object).ok_or("Inert Element attributes are unavailable.")?.iter().position(|attr|attr.name.qualified==text);
            return self.lookup_owner_attribute_node_at(object,slot);
        }
        let node = self.attr_element_node(object)?;
        let mut text = self.dom_text(args.first().ok_or("getAttributeNode requires a name.")?)?;
        if self.doc.nodes[node].namespace==crate::html::Namespace::Html&&self.doc.kind==crate::html::DocumentKind::Html{text.make_ascii_lowercase();}
        let slot = self.raw_attribute_slot(node, &text)?;
        self.lookup_attribute_node_at(node, slot)
    }

    pub(super) fn get_attribute_ns(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        if self.inert_node_type(object)==Some(1){
            let namespace=namespace(args)?;let local=self.dom_text(args.get(1).ok_or("getAttributeNS requires two arguments.")?)?;
            let attrs=self.inert_element_attributes(object).ok_or("Inert attributes are unavailable.")?;let work=attrs.iter().fold(attrs.len(),|n,a|n.saturating_add(a.name.bytes()));self.state.work(work)?;
            let slot=self.inert_element_attributes(object).unwrap().iter().position(|a|a.name.matches_names(namespace,local.view()));let Some(slot)=slot else{return Ok(Value::Null);};
            let length=self.inert_element_attributes(object).unwrap()[slot].value.logical_bytes();if length>self.state.limits.host.string_bytes{return Err("Inert attribute value exceeds the host allowance.".into());}self.state.copies(length)?;
            return Self::dom_string_copy(&self.inert_element_attributes(object).unwrap()[slot].value);
        }
        let node = self.attr_element_node(object)?;
        let ns = namespace(args)?;
        let local = self.dom_text(args.get(1).ok_or("getAttributeNS requires two arguments.")?)?;
        let slot = self.attribute_slot(node, ns, local.view())?;
        let Some(slot) = slot else { return Ok(Value::Null); };
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { unreachable!() };
        let text = &attrs[slot].value;
        if text.logical_bytes() > self.state.limits.host.string_bytes { return Err("Attribute value exceeds the host string bound.".into()); }
        self.state.copies(text.logical_bytes())?;
        Self::dom_string_copy(text)
    }

    pub(super) fn get_attribute_node_ns(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        if self.inert_node_type(object)==Some(1){
            let uri=namespace(args)?;let local=self.dom_text(args.get(1).ok_or("getAttributeNodeNS requires a local name.")?)?;
            let attrs=self.element_attribute_source(object).ok_or("Inert Element attributes are unavailable.")?;
            let work=attrs.iter().fold(attrs.len(),|work,attr|work.saturating_add(attr.name.bytes()));self.state.work(work)?;let slot=self.element_attribute_source(object).ok_or("Inert Element attributes are unavailable.")?.iter().position(|attr|attr.name.matches_names(uri,local.view()));
            return self.lookup_owner_attribute_node_at(object,slot);
        }

        let node = self.attr_element_node(object)?;
        let local = self.dom_text(args.get(1).ok_or("getAttributeNodeNS requires two arguments.")?)?;
        let slot = self.attribute_slot(node, namespace(args)?, local.view())?;
        self.lookup_attribute_node_at(node, slot)
    }

    pub(super) fn set_attribute_ns(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        let value=self.dom_text(args.get(2).ok_or("setAttributeNS requires three arguments.")?)?;
        let name=self.prepare_attribute_name(args,true)?;
        if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))?;return Ok(Value::Undefined);}
        let node=self.attr_element_node(object)?;
        self.public_dom_attribute(node,name,Some(value),InputPreparation::Pending)?;
        Ok(Value::Undefined)
    }

    pub(super) fn remove_attribute_ns(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        if self.inert_node_type(object)==Some(1){
            let namespace=namespace(args)?;let local=self.dom_text(args.get(1).ok_or("removeAttributeNS requires two arguments.")?)?;
            let attrs=self.inert_element_attributes(object).ok_or("Inert attributes are unavailable.")?;let work=attrs.iter().fold(attrs.len(),|n,a|n.saturating_add(a.name.bytes()));self.state.work(work)?;
            let slot=self.inert_element_attributes(object).unwrap().iter().position(|a|a.name.matches_names(namespace,local.view()));let Some(slot)=slot else{return Ok(Value::Undefined);};
            let bytes=self.inert_element_attributes(object).unwrap()[slot].name.bytes();self.state.copies(bytes)?;let name=copy_name(&self.inert_element_attributes(object).unwrap()[slot].name)?;
            self.write_inert_attribute_ns(object,name,None)?;return Ok(Value::Undefined);
        }
        let node = self.attr_element_node(object)?;
        let local = self.dom_text(args.get(1).ok_or("removeAttributeNS requires two arguments.")?)?;
        let slot = self.attribute_slot(node, namespace(args)?, local.view())?;
        self.remove_attribute_at(node, slot)
    }

    pub(super) fn remove_attribute_at(&mut self, node: usize, slot: Option<usize>) -> Result<Value, String> {
        if let Some(slot) = slot {
            let name = self.copy_attribute_name(node, slot)?;
            self.public_attribute(node, name, None)?;
        }
        Ok(Value::Undefined)
    }

    pub(super) fn validate_attr_removal(&self, object: u32, args: &[Value]) -> Result<(), (&'static str, String)> {
        let Some(Value::Host(handle)) = args.first() else { return Err(("TypeError", "removeAttributeNode requires an Attr.".into())); };
        let Some(attr) = index(*handle).and_then(|id| self.state.attributes.get(id)) else {
            return Err(("TypeError", "removeAttributeNode requires an Attr.".into()));
        };
        if Some(object) != attr.owner || attr.owner.is_none() {
            return Err(("NotFoundError", "Attr does not belong to this Element.".into()));
        }
        Ok(())
    }

    pub(super) fn remove_attribute_node(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        self.validate_attr_removal(object, args).map_err(|(_, message)| message)?;
        let Value::Host(handle) = args[0] else { unreachable!() };
        let id = index(handle).unwrap();
        if self.inert_node_type(object)==Some(1){self.state.copies(self.state.attributes[id].name.bytes())?;let name=copy_name(&self.state.attributes[id].name)?;self.write_inert_dom_attribute_ns(object,name,None)?;return Ok(Value::Host(handle));}
        self.state.copies(self.state.attributes[id].name.bytes())?;
        let attr = &self.state.attributes[id];
        let name = attr.name.copy()?;
        let owner=node_index(attr.owner.unwrap()).filter(|_|self.inert_node_type(attr.owner.unwrap()).is_none()).ok_or("Inert Attr removal requires its inert owner path.")?;
        self.public_attribute(owner, name, None)?;
        Ok(Value::Host(handle))
    }

    // Resolve a raw QName or expanded name before entering this policy boundary.
    // Only real lower-case null-namespace HTML attributes have native effects.
    pub(super) fn html_id_attribute(&self,node:usize,name:&AttributeName)->bool{
        self.doc.nodes[node].namespace==crate::html::Namespace::Html && name.html_is("id")
    }
    pub(super) fn html_passive_name_attribute(&self,node:usize,name:&AttributeName)->bool{
        passive_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),name)
    }
    pub(super) fn public_dom_attribute(&mut self,node:usize,name:AttributeName,value:Option<DomString>,input:InputPreparation)->Result<(),String>{
        if value.as_ref().is_some_and(|v|v.raw_units().is_some()) {
            if !self.html_id_attribute(node,&name) && !self.html_passive_name_attribute(node,&name) && !self.meta_content_attribute(node,&name) && !self.resource_charset_attribute(node,&name) && !self.html_class_attribute(node,&name) && !self.html_text_reflection_attribute(node,&name) && !html_aria_attribute(self.doc.nodes[node].namespace,&name) && !html_label_or_output_for_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_output_form_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_time_datetime_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !(html_area_alt_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_optgroup_label_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_option_label_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_option_value_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(((html_select_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_button_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||html_input_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||html_textarea_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))))) && !html_area_coords_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_shape_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_download_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_type_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_hreflang_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_target_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_rel_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_href_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_referrer_policy_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_ping_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !(html_map_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_param_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_param_metadata_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))) && !html_mod_datetime_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_data_value_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_heading_align_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_ul_type_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_li_type_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_ol_type_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_li_value_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !(html_pre_width_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_textarea_dimensions_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_textarea_max_length_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_textarea_min_length_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_textarea_wrap_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_textarea_autocomplete_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_select_autocomplete_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||((html_input_autocomplete_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||((html_input_form_action_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_input_form_method_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_input_form_enctype_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))||(html_button_form_action_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_button_form_method_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_button_form_enctype_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))))||(((html_form_autocomplete_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||((html_form_method_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_form_action_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||html_form_enctype_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))||((html_form_accept_charset_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_form_rel_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||((html_form_target_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_button_form_target_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||html_input_form_target_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))))||((html_form_no_validate_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_button_form_no_validate_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||html_input_form_no_validate_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))))))))))) && !html_ol_start_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_ul_compact_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_dir_compact_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_menu_compact_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_ol_compact_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_hr_no_shade_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_ol_reversed_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !(html_area_no_href_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(((html_option_disabled_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||((html_select_disabled_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_textarea_disabled_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_textarea_required_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_textarea_read_only_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))))||html_select_required_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))||html_option_default_selected_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||html_optgroup_disabled_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))) && !html_dl_compact_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_paragraph_align_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_hr_align_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_hr_color_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_font_size_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_font_face_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_font_color_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_br_clear_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_hr_size_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_hr_width_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_legend_align_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_div_align_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_meter_number_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_progress_number_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !name.namespace_uri().is_some_and(|uri|uri=="http://www.w3.org/2000/xmlns/") && (self.doc.nodes[node].namespace!=crate::html::Namespace::Html || !name.namespace.is_empty() || !name.qualified.starts_with("data-")) {return Err("Unpaired attribute DOMString integration currently requires supported HTML ID or passive metadata.".into());}
            self.attribute_dom_named_prepared(node,name,value,input)
        }else{self.public_attribute_prepared(node,name,value.map(|v|v.into_parts().0),input)}
    }
    pub(super) fn public_attribute(&mut self, node: usize, name: AttributeName, value: Option<String>) -> Result<(), String> {
        self.public_attribute_prepared(node,name,value,InputPreparation::Pending)
    }
    fn public_attribute_prepared(&mut self,node:usize,name:AttributeName,value:Option<String>,input:InputPreparation)->Result<(),String>{
        if self.html_passive_name_attribute(node,&name){return self.attribute_named_prepared(node,name,value,input);}
        if self.doc.nodes[node].namespace==crate::html::Namespace::Svg{
            if let Some(value)=value.as_deref(){self.state.work(name.bytes().saturating_add(value.len()))?;if !crate::html::svg_dom_attribute(tag(self.doc,node),&name,value){return Err("SVG attribute is outside the supported passive/local-reference policy; no attribute was changed.".into());}if name.namespace.is_empty()&&name.qualified=="style"{validate_style(value,self.state.limits.host.string_bytes)?;}}
            return self.attribute_named_prepared(node,name,value,input);
        }

        // Non-ASCII spellings are ordinary data, including literal replacement
        // characters. They cannot name any native ASCII attribute effect.
        if !name.namespace.is_empty() || !name.qualified.is_ascii() || name.qualified.contains(':') || name.qualified.bytes().any(|c| c.is_ascii_uppercase()) {
            return self.attribute_named_prepared(node, name, value,input);
        }
        let key = name.qualified.as_str();
        if self.accepts_input_type(node, &name, value.as_deref()) { return self.attribute_named_prepared(node, name, value,input); }
        if self.hidden_input_attribute(node,&name,&value)? {return Ok(());}
        let media_source=key=="src"&&self.media_element_receiver(node_handle(node));
        let media_controls=key=="controls"&&self.media_interface_receiver(node_handle(node));
        let style_metadata=matches!(key,"type"|"media")&&self.created_html_style(node);
        if self.stylesheet_attribute_change(node, key, value.as_deref())? { return Ok(()); }
        if self.resource_charset_attribute(node,&name) { return self.attribute_named_prepared(node,name,value,input); }
        if self.dom_script(node) && matches!(key, "src" | "type" | "charset" | "nonce" | "integrity" | "crossorigin" | "referrerpolicy" | "fetchpriority" | "async" | "defer" | "nomodule") {
            let queue = key == "src" && self.state.dom.connected[node];
            self.attribute_named_prepared(node, name, value,input)?;
            if queue { self.queue_dynamic_script(node)?; }
            return Ok(());
        }
        let image_attribute = tag(self.doc, node) == "img" && matches!(key, "src" | "alt" | "width" | "height")
            || key=="align" && self.html_text_reflection(node,"align").is_some()
            || key=="usemap" && self.html_text_reflection(node,"useMap").is_some()
            || self.html_spacing_property(node, key)
            || key == "border" && self.html_border_property(node)
            || key == "ismap" && self.html_image_map_property(node)
            || key == "srcdoc" && self.iframe_srcdoc_property(node);
        // Anchor content attributes carry no navigation authority. Only an
        // accepted activation resolves their current raw values.
        let anchor_attribute = self.doc.nodes[node].namespace == crate::html::Namespace::Html
            && tag(self.doc, node) == "a" && matches!(key, "href" | "download");
        let placeholder_attribute=key=="placeholder"&&self.placeholder_interface(node_handle(node)).is_some();
        let input_attribute = matches!(key,"name"|"value") && (self.native_text_input(node) || self.fresh_text_input(node))
            || matches!(key,"name"|"value") && (self.owned_native_checkbox(node) || self.owned_native_radio(node))
            || key == "checked" && self.input_receiver(node_handle(node));
        let button_attribute = matches!(key,"name"|"value"|"type") && (self.native_button(node) || self.fresh_button(node));
        let content_editable = key == "contenteditable" && self.content_editable_receiver(node_handle(node));
        let canvas_attribute = matches!(key, "width" | "height") && crate::html_canvas::element(self.doc, node);
        let slot_name = key == "name" && self.html_slot_receiver(node_handle(node));
        let sandbox_attribute=key=="sandbox"&&self.iframe_sandbox_receiver(node_handle(node));
        // Custom hyphenated HTML names are ordinary DOM data, including names
        // used by attribute selectors. The standard hyphenated native names
        // retain their explicit policy; foreign attributes retain their paths.
        // https://html.spec.whatwg.org/multipage/indices.html#attributes-3
        let custom_attribute = self.doc.nodes[node].namespace == crate::html::Namespace::Html
            && key.contains('-') && !matches!(key,"accept-charset"|"http-equiv");
        // Nonce text follows existing CSP snapshots/queues; changing metadata
        // does not rerun a parsed script. Attribution reporting is unavailable:
        // preserve its content attribute without advertising an IDL API or
        // generating an attribution/report request.
        let tab_index=key=="tabindex"&&self.doc.nodes[node].namespace==crate::html::Namespace::Html;
        if !matches!(key, "id" | "class" | "style" | "title" | "hidden" | "disabled" | "role" | "lang" | "dir" | "nonce" | "attributionsrc")
            && !inert_attribute_name(key) && !placeholder_attribute && !image_attribute && !anchor_attribute && !input_attribute && !button_attribute && !content_editable
            && !canvas_attribute && !media_source && !media_controls && !style_metadata && !slot_name && !custom_attribute && !sandbox_attribute && !tab_index && !self.meta_content_attribute(node, &name)
            && !html_label_or_output_for_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_output_form_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_time_datetime_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !(html_area_alt_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_optgroup_label_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_option_label_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_option_value_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(((html_select_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_button_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||html_input_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||html_textarea_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))))) && !html_area_coords_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_shape_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_download_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_type_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_hreflang_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_target_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_rel_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_href_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_referrer_policy_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_area_ping_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !(html_map_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_param_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_param_metadata_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))) && !html_mod_datetime_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_data_value_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_heading_align_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_ul_type_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_li_type_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_ol_type_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_li_value_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !(html_pre_width_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_textarea_dimensions_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_textarea_max_length_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_textarea_min_length_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_textarea_wrap_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_textarea_autocomplete_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_select_autocomplete_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||((html_input_autocomplete_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||((html_input_form_action_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_input_form_method_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_input_form_enctype_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))||(html_button_form_action_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_button_form_method_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_button_form_enctype_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))))||(((html_form_autocomplete_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||((html_form_method_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_form_action_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||html_form_enctype_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))||((html_form_accept_charset_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_form_rel_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||((html_form_target_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_button_form_target_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||html_input_form_target_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))))||((html_form_no_validate_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_button_form_no_validate_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||html_input_form_no_validate_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))))))))))) && !html_ol_start_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_ul_compact_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_dir_compact_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_menu_compact_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_ol_compact_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_hr_no_shade_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_ol_reversed_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !(html_area_no_href_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(((html_option_disabled_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||((html_select_disabled_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_textarea_disabled_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(html_textarea_required_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||html_textarea_read_only_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))))||html_select_required_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))||html_option_default_selected_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||html_optgroup_disabled_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))) && !html_dl_compact_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_paragraph_align_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_hr_align_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_hr_color_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_font_size_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_font_face_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_font_color_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_br_clear_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_hr_size_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_hr_width_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_legend_align_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_div_align_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_meter_number_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !html_progress_number_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) {
            return Err(format!("Supported attributes exclude {key:?} on {} in this native/metadata subset; no attribute was changed.",tag(self.doc,node)));
        }
        if key == "src" && image_attribute {
            return self.image_source_change(node, value);
        }
        if key == "style" {
            if let Some(value) = value.as_ref() { validate_style(value, self.state.limits.host.string_bytes)?; }
        }
        self.attribute_named_prepared(node, name, value,input)
    }

    pub(super) fn attr_default_namespace(&mut self, object: u32) -> Option<Result<Option<&'static str>, String>> {
        let id = index(object)?;
        Some((|| {
            let owner = self.state.attributes.get(id).ok_or("Invalid Attr handle.")?.owner;
            if let Some(owner) = owner {let node=node_index(owner).filter(|_|self.inert_node_type(owner).is_none()).ok_or("Inert Attr namespace traversal is unavailable.")?;dom_tree::default_namespace(self.doc, node, None, self.state)}
            else { self.state.work(1)?; Ok(None) }
        })())
    }
}

#[cfg(test)]
#[path = "script_anchor_attribute_tests.rs"]
mod anchor_attribute_tests;

#[cfg(test)]
#[path = "script_hidden_input_tests.rs"]
mod hidden_input_tests;

#[cfg(test)]
#[path="script_attr_storage_tests.rs"]
mod storage_tests;

#[cfg(test)]
#[path="script_custom_attribute_tests.rs"]
mod custom_attribute_tests;
