//! Native CDATA is CharacterData/Text; XML character legality is separate.
use super::*;
impl Bridge<'_>{
    pub(in crate::scripting) fn validate_cdata_factory(&self,owner:u32,args:&[Value])->Result<(),(&'static str,String)>{
        let xml=if owner==DOCUMENT{self.doc.kind==crate::html::DocumentKind::Xml}else{self.state.inert_documents.node(owner).is_some_and(|node|matches!(&node.kind,Kind::Document(doc) if doc.kind==crate::html::DocumentKind::Xml))};
        if !xml{return Err(("NotSupportedError","HTML documents cannot create CDATA sections.".into()));}
        let invalid=match args.first(){Some(Value::String(text))=>text.contains("]]>"),Some(Value::StringUnits(units))=>units.windows(3).any(|s|s==[0x5d,0x5d,0x3e]),_=>false};
        if invalid{return Err(("InvalidCharacterError","CDATA factory data contains its closing delimiter.".into()));}Ok(())
    }
    pub(in crate::scripting) fn create_cdata_section(&mut self,owner:u32,args:&[Value])->Result<Value,String>{
        self.validate_cdata_factory(owner,args).map_err(|(_,message)|message)?;
        let data=self.dom_text(args.first().ok_or("createCDATASection requires one argument.")?)?;
        if owner==DOCUMENT{self.create_dom_node(NodeKind::Cdata(data))}else{self.append_inert_source_leaf(owner,None,Node{kind:NodeKind::Cdata(data),namespace:crate::html::Namespace::None,children:Vec::new()},0).map(Value::Host)}
    }
}

#[cfg(test)]
mod storage_tests{
 use super::*;
 #[test]
 fn native_cdata_actual_owned_storage_stable_slot_and_inert_capacity_fallback(){
  let mut doc=crate::html::parse("<!doctype html><html><body></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
  let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let Value::Host(owner)=bridge.create_xml_document(MAIN_IMPLEMENTATION,&[Value::Null,Value::String(String::new())]).unwrap()else{panic!("real XML")};
  let backing=|b:&Bridge<'_>|b.doc.nodes.capacity()*size_of::<Node>()+b.state.node_state_bytes()+b.state.node_sets_storage_bytes()+b.state.inert_documents.backing_bytes();
  for parked in [true,false]{
   if !parked{bridge.state.node_limit=bridge.doc.nodes.len();}
   let before=(bridge.state.retained,backing(&bridge),bridge.doc.nodes.len());let text=String::from("payload");let payload=text.capacity()+size_of::<Node>()+2*size_of::<usize>();
   let Value::Host(handle)=bridge.create_cdata_section(owner,&[Value::String(text)]).unwrap()else{panic!("real CDATA")};
   assert_eq!(bridge.state.retained-before.0,payload+backing(&bridge)-before.1);assert_eq!(bridge.inert_node_type(handle),Some(4));assert_eq!(bridge.state.inert_documents.node(handle).unwrap().owner,owner);assert_eq!(bridge.inert_text_value(handle).unwrap().scalar().unwrap(),"payload");
   assert_eq!(bridge.doc.nodes.len(),before.2+usize::from(parked));assert_eq!(node_index(handle)==Some(before.2),parked);
   if parked{assert!(!bridge.state.dom.connected[before.2]);}
  }
 }
}
