//! Readonly live builtin form-control count from actual root/owner authority.
//! https://html.spec.whatwg.org/multipage/forms.html#dom-form-length
use super::*;
pub(in crate::scripting) const GET:&str="\0HTMLFormElement.get.length";
impl Bridge<'_>{
 pub(in crate::scripting) fn form_length_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLFormElement")}
 pub(in crate::scripting) fn form_length_member(&self,object:u32,property:&str)->bool{property=="length"&&self.form_length_receiver(object)&&!self.form_named_control(object,property)}
 pub(in crate::scripting) fn validate_form_length(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.form_length_receiver(object){return Err(("TypeError","Form length requires a genuine HTML form.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child form counts require their tree lifecycle hooks.".into()));}
  Ok(())
 }
 fn form_length_listed(&mut self,object:u32)->Result<bool,String>{
  let Some((_,tag,_))=self.label_element_facts(object)else{return Ok(false)};
  self.state.work(tag.len().saturating_add(1))?;
  let(namespace,tag,_)=self.label_element_facts(object).unwrap();
  if namespace!=Namespace::Html||!matches!(tag,"button"|"fieldset"|"input"|"object"|"output"|"select"|"textarea"){return Ok(false);}
  if tag!="input"{return Ok(true);}
  let Some(at)=self.label_attribute_slot(object,"type")?else{return Ok(true)};
  self.state.work(6)?;
  let image=self.label_element_facts(object).unwrap().2[at].value.code_units().map(|u|if (0x41..=0x5a).contains(&u){u+0x20}else{u}).eq("image".encode_utf16());
  Ok(!image)
 }
 pub(in crate::scripting) fn form_length_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_form_length(object).map_err(|(_,message)|message)?;
  self.state.work(1)?;script_text_content::deadline(self.state)?;self.state.rebuild(self.doc)?;
  let maximum=self.doc.nodes.len().checked_add(self.inert_node_count()).and_then(|n|n.checked_add(1)).ok_or("Form count tree bound overflow.")?;
  let lookup=self.state.inert_documents.adopted.lookup_work().checked_add(8).and_then(|n|n.checked_mul(8)).ok_or("Form count lookup work overflow.")?;
  let mut root=object;let mut steps=0usize;
  while root!=DOCUMENT{
   steps=steps.checked_add(1).filter(|&n|n<=maximum).ok_or("Invalid or cyclic form root ancestry.")?;self.state.work(lookup)?;
   let Some(parent)=self.form_count_parent(root)?else{break};if !(self.form_count_physical_record(parent)){self.native_node_access(parent).map_err(|(_,message)|message)?;}root=parent;
  }
  // A parent/sibling walk holds no outer traversal allocation while the shared
  // explicit-ID owner lookup admits its own paid scratch vector.
  let mut current=root;let mut visited=0usize;let mut length=0usize;
  loop{
   visited=visited.checked_add(1).filter(|&n|n<=maximum).ok_or("Invalid or cyclic form count tree.")?;self.state.work(lookup)?;script_text_content::deadline(self.state)?;
   if !(self.form_count_physical_record(current)){self.native_node_access(current).map_err(|(_,message)|message)?;}
   if self.form_length_listed(current)?&&matches!(self.control_form_count_owner(current)?,Value::Host(owner)if owner==object){length=length.checked_add(1).ok_or("Form count overflow.")?;}
   if self.label_element_facts(current).is_some_and(|(ns,tag,_)|ns==Namespace::Html&&tag=="iframe"){
    for at in 0..self.label_child_count(current)?{self.state.work(lookup)?;script_text_content::deadline(self.state)?;if self.label_element_facts(self.label_child_at(current,at)?).is_some(){return Err("Legacy iframe fallback topology requires its native text ownership hooks.".into());}}
   }
   if !self.template_receiver(current)&&self.label_child_count(current)?>0{self.state.work(lookup)?;current=self.label_child_at(current,0)?;continue;}
   let mut cursor=current;let mut ascents=0usize;
   loop{
    if cursor==root{script_text_content::deadline(self.state)?;return Ok(Value::Number(length as f64));}
    ascents=ascents.checked_add(1).filter(|&n|n<=maximum).ok_or("Invalid form count traversal ancestry.")?;self.state.work(lookup)?;
    let parent=self.form_count_parent(cursor)?.ok_or("Form count edge lost its root.")?;if !(self.form_count_physical_record(parent)){self.native_node_access(parent).map_err(|(_,message)|message)?;}
    let count=self.label_child_count(parent)?;let mut found=None;
    for at in 0..count{script_text_content::deadline(self.state)?;self.state.work(lookup)?;if self.label_child_at(parent,at)?==cursor{found=Some(at);break;}}
    let at=found.ok_or("Form count parent edge is unavailable.")?;
    if at+1<count{self.state.work(lookup)?;current=self.label_child_at(parent,at+1)?;break;}
    cursor=parent;
   }
  }
 }
}
