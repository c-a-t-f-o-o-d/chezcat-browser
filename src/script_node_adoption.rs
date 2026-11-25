//! Checked adoption moves actual records and preserves their public handles.
use super::*;
struct Tree{nodes:Vec<u32>,native:bool,old_document:u32,parent:Option<u32>}
#[cfg(test)]
mod storage_tests{
 use super::*;
 fn fixture()->(Document,Forms,Page){let mut doc=crate::html::parse("<!doctype html><html><body><div id=source><span>data</span><!--marker--></div></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)}
 #[test]
 fn node_adoption_native_owner_index_and_real_child_vectors_are_charged_without_payload_copy(){
  let(mut doc,mut forms,mut page)=fixture();let source=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{id:Some(id),..} if id=="source")).unwrap();let text=doc.nodes[doc.nodes[source].children[0]].children[0];let pointer=doc.nodes[text].kind.character_data().unwrap().as_ptr();let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let Value::Host(target)=b.create_xml_document(MAIN_IMPLEMENTATION,&[Value::Null,Value::String(String::new())]).unwrap()else{panic!("target")};let before=(b.state.retained,b.state.inert_documents.backing_bytes(),b.doc.nodes.len());let handles=b.adoption_tree(node_handle(source),target).unwrap().nodes;
  let result=b.adopt_node(target,&[Value::Host(node_handle(source))]).unwrap();assert!(matches!(result,Value::Host(handle) if handle==node_handle(source)));let payload=handles.iter().map(|handle|b.state.inert_documents.node(*handle).unwrap().children.capacity()*size_of::<u32>()).sum::<usize>();assert_eq!(b.state.retained-before.0,payload+b.state.inert_documents.backing_bytes()-before.1);assert_eq!(b.doc.nodes.len(),before.2);assert_eq!(b.doc.nodes[text].kind.character_data().unwrap().as_ptr(),pointer);for handle in handles{assert_eq!(b.state.inert_documents.node(handle).unwrap().owner,target);}
 }
 #[test]
 fn node_adoption_storage_copy_refusal_and_inert_zero_growth_keep_real_source_then_recover(){
  let(mut doc,mut forms,mut page)=fixture();let source=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{id:Some(id),..} if id=="source")).unwrap();let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let Value::Host(target)=b.create_xml_document(MAIN_IMPLEMENTATION,&[Value::Null,Value::String(String::new())]).unwrap()else{panic!("target")};let parent=b.state.tree.parents[source].unwrap();let before=(b.state.retained,b.state.inert_documents.backing_bytes(),b.state.dom_commits,b.doc.nodes[parent].children.clone());let limit=b.state.retained_limit;let copies=b.state.copy_work;
  for storage in [true,false]{if storage{b.state.retained_limit=b.state.retained;}else{b.state.copy_work=0;}assert!(b.adopt_node(target,&[Value::Host(node_handle(source))]).is_err());assert_eq!((b.state.retained,b.state.inert_documents.backing_bytes(),b.state.dom_commits,b.doc.nodes[parent].children.clone()),before);assert_eq!(b.state.tree.parents[source],Some(parent));assert!(b.dom_accessible(source));b.state.retained_limit=limit;b.state.copy_work=copies;}
  assert!(b.adopt_node(target,&[Value::Host(node_handle(source))]).is_ok());let Value::Host(other)=b.create_xml_document(MAIN_IMPLEMENTATION,&[Value::Null,Value::String(String::new())]).unwrap()else{panic!("other target")};let retained=b.state.retained;b.state.retained_limit=retained;assert!(b.adopt_node(other,&[Value::Host(node_handle(source))]).is_ok());assert_eq!(b.state.retained,retained);assert_eq!(b.state.inert_documents.node(node_handle(source)).unwrap().owner,other);
 }
}
impl Bridge<'_>{
 fn adoption_source(&self,source:u32)->Result<(bool,u32,Option<u32>),(&'static str,String)>{
  if let Some(node)=self.state.inert_documents.node(source){return Ok((false,node.owner,node.parent));}
  let index=node_index(source).filter(|&node|self.dom_accessible(node)).ok_or(("NotSupportedError","Adoption of child-frame or unavailable node records is not implemented.".into()))?;
  Ok((true,DOCUMENT,self.state.tree.parents.get(index).copied().flatten().map(|parent|if parent==0{DOCUMENT}else{node_handle(parent)})))
 }
 fn adoption_tree(&self,source:u32,document:u32)->Result<Tree,(&'static str,String)>{
  let(native,old_document,parent)=self.adoption_source(source)?;let failure=|message:&str|("NotSupportedError",message.to_owned());
  let mut nodes=Vec::new();let mut pending=Vec::new();pending.try_reserve_exact(1).map_err(|_|failure("Cannot reserve adoption root."))?;pending.push((source,parent));
  let maximum=self.doc.nodes.len().saturating_add(self.state.inert_documents.entries.len()).saturating_add(self.state.inert_documents.adopted.len());
  while let Some((handle,expected))=pending.pop(){
   if nodes.len()>=maximum{return Err(failure("Adoption source topology is inconsistent."));}
   if old_document!=document&&self.template_receiver(handle){return Err(failure("Adopting template hosts requires their complete template-owner and adopting steps."));}
   let children=if native{
    let index=node_index(handle).filter(|&node|self.dom_accessible(node)).ok_or_else(||failure("Adoption descendant is inaccessible."))?;
    let item=&self.doc.nodes[index];
    if matches!(item.namespace,crate::html::Namespace::UnknownForeign){return Err(failure("Adoption requires known namespace provenance."));}
    if self.doc.native_focus.get()==Some(index){return Err(failure("Adopting the native focused subtree requires focus lifecycle integration."));}
    if self.forms.controls.contains_key(&index)||matches!(&item.kind,NodeKind::Element{tag,..} if matches!(tag.as_str(),"html"|"head"|"body"|"title"|"base"|"meta"|"form"|"fieldset"|"legend"|"input"|"button"|"select"|"option"|"textarea"|"img"|"iframe"|"object"|"embed"|"script"|"style"|"link"|"canvas"|"video"|"audio"|"source"|"template") || tag=="optgroup" && item.namespace!=crate::html::Namespace::Html){
     return Err(failure("Adopting native structural, resource or control state requires its dedicated lifecycle hooks."));
    }
    if item.namespace==crate::html::Namespace::Svg&&!crate::html::svg_dom_transfer(item){return Err(failure("Adopting unsupported SVG native state is unavailable."));}
    if let Some(id)=self.doc.attr(index,"id").filter(|id|!id.is_empty()){if self.forms.controls.keys().any(|&control|self.doc.attr(control,"form")==Some(id)){return Err(failure("Adopting an id used by a native form association requires form ownership integration."));}}
    let actual=self.state.tree.parents.get(index).copied().flatten().map(|parent|if parent==0{DOCUMENT}else{node_handle(parent)});if actual!=expected{return Err(failure("Native adoption parent changed."));}
    item.children.len()
   }else{
    let node=self.state.inert_documents.node(handle).filter(|node|node.owner==old_document&&node.parent==expected).ok_or_else(||failure("Inert adoption owner or parent is inconsistent."))?;
    if matches!(self.inert_import_source(handle),Some(ImportSource::Parsed{node:Node{namespace:crate::html::Namespace::UnknownForeign,..},..})){return Err(failure("Adoption requires known namespace provenance."));}
    node.children.len()
   };
   nodes.try_reserve(1).map_err(|_|failure("Cannot reserve adoption node plan."))?;nodes.push(handle);pending.try_reserve(children).map_err(|_|failure("Cannot reserve adoption descendants."))?;
   if native{for &child in self.doc.nodes[node_index(handle).unwrap()].children.iter().rev(){pending.push((node_handle(child),Some(handle)));}}
   else{for &child in self.inert_children(handle).unwrap().iter().rev(){pending.push((child,Some(handle)));}}
  }
  nodes.sort_unstable();if nodes.windows(2).any(|pair|pair[0]==pair[1]){return Err(failure("Adoption repeats a native identity."));}
  Ok(Tree{nodes,native,old_document,parent})
 }
 pub(in crate::scripting) fn validate_node_adoption(&self,document:u32,args:&[Value])->Result<(),(&'static str,String)>{
  if document!=DOCUMENT&&!self.inert_document(document){return Err(("TypeError","adoptNode requires a genuine Document.".into()));}
  let Some(Value::Host(source))=args.first().filter(|value|matches!(value,Value::Host(handle) if self.get_root_receiver(*handle)))else{return Err(("TypeError","adoptNode requires one genuine Node.".into()));};
  if *source==DOCUMENT||self.inert_document(*source)||self.active_frame_document(*source){return Err(("NotSupportedError","A Document cannot be adopted.".into()));}
  if self.attr_receiver(*source){return Ok(());}
  let(native,old,parent)=self.adoption_source(*source)?;if old==document&&parent.is_none(){return Ok(());}
  if native{let mut ancestor=parent;let mut steps=0usize;while let Some(handle)=ancestor{steps+=1;if steps>self.doc.nodes.len(){return Err(("NotSupportedError","Native adoption ancestor topology is inconsistent.".into()));}let index=if handle==DOCUMENT{0}else{node_index(handle).ok_or(("NotSupportedError","Native adoption ancestor is unavailable.".into()))?};let name=tag(self.doc,index);if self.forms.controls.contains_key(&index)||matches!(name,"title"|"script"|"style"|"template"|"iframe"|"object"|"select"|"option"|"optgroup"|"textarea"|"video"|"audio"|"source"){return Err(("NotSupportedError","Adopting from a native control or resource subtree requires its content hooks.".into()));}ancestor=self.state.tree.parents.get(index).copied().flatten().map(|parent|if parent==0{DOCUMENT}else{node_handle(parent)});}}
  self.adoption_tree(*source,document).map(|_|())
 }
 pub(in crate::scripting) fn adopt_node(&mut self,document:u32,args:&[Value])->Result<Value,String>{
  self.validate_node_adoption(document,args).map_err(|(_,message)|message)?;let Value::Host(source)=args[0]else{unreachable!()};
  if let Some(id)=script_attr::index(source).filter(|&id|id<self.state.attributes.len()){if self.state.attributes[id].document!=document{self.state.copies(size_of::<u32>())?;self.state.attributes[id].document=document;self.state.dom_committed();}return Ok(Value::Host(source));}
  let(native,old,parent)=self.adoption_source(source).map_err(|(_,message)|message)?;if old==document&&parent.is_none(){return Ok(Value::Host(source));}
  if native{self.state.rebuild(self.doc)?;self.validate_node_adoption(document,args).map_err(|(_,message)|message)?;}
  let tree=self.adoption_tree(source,document).map_err(|(_,message)|message)?;
  let count=tree.nodes.len();let lookup=(usize::BITS-count.leading_zeros()) as usize+1;self.state.work(count.checked_mul(3*(lookup+3)).ok_or("Adoption traversal work overflow.")?)?;self.state.copies(count.checked_mul(3*(lookup+6)).and_then(|n|n.checked_mul(size_of::<u32>())).ok_or("Adoption traversal copy overflow.")?)?;
  let mut indices=Vec::new();if tree.native{indices.try_reserve_exact(count).map_err(|_|"Cannot reserve adoption Range node references.")?;self.state.copies(count.checked_mul(size_of::<usize>()).ok_or("Adoption Range references overflow.")?)?;indices.extend(tree.nodes.iter().map(|&handle|node_index(handle).unwrap()));}
  let attr_changes=if tree.old_document!=document{self.prepare_attr_document_rebind(|owner|tree.nodes.binary_search(&owner).is_ok(),lookup,document)?}else{Vec::new()};
  let slot=if let Some(parent)=tree.parent{let length=if tree.native{self.doc.nodes[if parent==DOCUMENT{0}else{node_index(parent).unwrap()}].children.len()}else{self.inert_children(parent).ok_or("Missing adoption source parent.")?.len()};self.state.work(length)?;let at=if tree.native{self.doc.nodes[if parent==DOCUMENT{0}else{node_index(parent).unwrap()}].children.iter().position(|&child|node_handle(child)==source)}else{self.inert_children(parent).unwrap().iter().position(|&child|child==source)}.ok_or("Missing adoption child link.")?;self.state.copies((length-at-1).checked_mul(if tree.native{size_of::<usize>()}else{size_of::<u32>()}).ok_or("Adoption removal movement overflow.")?)?;Some(at)}else{None};
  let mut records=Vec::new();let mut payload=0usize;
  if tree.native&&document!=DOCUMENT{
   records.try_reserve_exact(count).map_err(|_|"Cannot reserve native adoption records.")?;
   for &handle in &tree.nodes{let index=node_index(handle).unwrap();let item=&self.doc.nodes[index];let mut children=Vec::new();children.try_reserve_exact(item.children.len()).map_err(|_|"Cannot reserve adopted child links.")?;children.extend(item.children.iter().copied().map(node_handle));payload=payload.checked_add(children.capacity()*size_of::<u32>()).ok_or("Adopted child payload overflow.")?;self.state.copies(children.len().checked_mul(size_of::<u32>()).ok_or("Adopted child copy overflow.")?)?;
    let kind=if matches!(item.kind,NodeKind::DocumentFragment){Kind::Fragment{template:None}}else{Kind::Parsed{source:ParsedSource::Loaded(index),initial_template:None}};let parent=if handle==source{None}else{self.state.tree.parents[index].map(node_handle)};records.push((handle,InertNode{kind,owner:document,parent,children}));
   }
  }
  let (growth,added)=if records.is_empty(){(None,0)}else{self.prepare_adopted_index(tree.nodes.iter().copied(),self.state.retained_limit.saturating_sub(self.state.retained).checked_sub(payload).ok_or("Adoption payload allowance exhausted.")?)?};
  if tree.native{self.state.prepare_range_changes(1,count)?;}else if let Some(parent)=tree.parent{self.prepare_document_children_sync(parent,0)?;}
  self.state.replace_storage(0,payload.checked_add(added).ok_or("Adoption storage overflow.")?,0)?;
  // Every fallible allocation and move is admitted. Publish source removal
  // and owner changes without callbacks between these native record writes.
  if tree.native{
   if let Some(parent)=tree.parent{let index=if parent==DOCUMENT{0}else{node_index(parent).unwrap()};self.state.ranges_remove(index,slot.unwrap(),1,&indices);self.doc.nodes[index].children.remove(slot.unwrap());}
   let connected=self.state.dom.connected[node_index(source).unwrap()];
   self.state.tree.parents[node_index(source).unwrap()]=None;
   if document!=DOCUMENT{self.state.inert_documents.adopted.commit(growth);for(handle,record)in records{self.state.inert_documents.adopted.insert(handle,record);}}
   for index in indices{self.state.dom.connected[index]=false;self.state.tree.active[index]=false;if document==DOCUMENT{self.state.detached_nodes.insert(index);}else{self.state.detached_nodes.remove(&index);}}
   self.state.dirty=true;if connected{self.state.layout_changed();}
  }else{
   if let Some(parent)=tree.parent{self.state.inert_documents.node_mut(parent).unwrap().children.remove(slot.unwrap());self.sync_document_children(parent);}
   for &handle in &tree.nodes{let node=self.state.inert_documents.node_mut(handle).unwrap();node.owner=document;if handle==source{node.parent=None;}}
  }
  self.commit_attr_document_rebind(&attr_changes,document);self.state.dom_committed();Ok(Value::Host(source))
 }
 pub(in crate::scripting) fn adoption_changes(&mut self,args:&[Value])->Result<Vec<crate::javascript::NativeChildListChange>,String>{
  let Some(Value::Host(source))=args.first()else{return Ok(Vec::new());};if self.attr_receiver(*source){return Ok(Vec::new());}if self.state.inert_documents.node(*source).is_none(){self.state.rebuild(self.doc)?;}
  let(native,_,parent)=self.adoption_source(*source).map_err(|(_,message)|message)?;let Some(parent)=parent else{return Ok(Vec::new());};
  let (length,at,previous,next)=if native{let siblings=&self.doc.nodes[if parent==DOCUMENT{0}else{node_index(parent).unwrap()}].children;let at=siblings.iter().position(|&child|node_handle(child)==*source).ok_or("Missing observed adoption link.")?;(siblings.len(),at,at.checked_sub(1).map(|i|node_handle(siblings[i])),siblings.get(at+1).copied().map(node_handle))}else{let siblings=self.inert_children(parent).ok_or("Missing observed inert adoption parent.")?;let at=siblings.iter().position(|child|child==source).ok_or("Missing observed inert adoption link.")?;(siblings.len(),at,at.checked_sub(1).map(|i|siblings[i]),siblings.get(at+1).copied())};let _=at;
  self.state.work(length)?;self.state.copies(size_of::<crate::javascript::NativeChildListChange>()+size_of::<u32>())?;let mut removed=Vec::new();removed.try_reserve_exact(1).map_err(|_|"Cannot reserve adopted removal identity.")?;removed.push(*source);let mut changes=Vec::new();changes.try_reserve_exact(1).map_err(|_|"Cannot reserve adoption removal record.")?;changes.push(crate::javascript::NativeChildListChange{target:parent,added:Vec::new(),removed,previous,next});Ok(changes)
 }
}
