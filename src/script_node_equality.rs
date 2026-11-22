//! DOM equality reads native facts, never author getters or serialization.
//! https://dom.spec.whatwg.org/#concept-node-equals
use super::*;
use crate::dom_string::DomStringRef;
use crate::html::{Attribute,AttributeName,DocumentTypeData};
pub(in crate::scripting) const SAME:&str="\0Node.isSameNode";
pub(in crate::scripting) const EQUAL:&str="\0Node.isEqualNode";
pub(in crate::scripting) fn operation(method:&str)->bool{matches!(method,SAME|EQUAL)}

enum View<'a>{
 Document(bool), Fragment,
 Element{namespace:Option<DomStringRef<'a>>,prefix:Option<&'a DomString>,local:DomStringRef<'a>,attrs:&'a [Attribute]},
 Attr(&'a AttributeName,&'a DomString),
 Data(u8,Option<&'a str>,&'a DomString), Doctype(&'a DocumentTypeData),
}
fn units(a:DomStringRef<'_>,b:DomStringRef<'_>)->bool{a.code_units().eq(b.code_units())}
fn nullable(a:Option<DomStringRef<'_>>,b:Option<DomStringRef<'_>>)->bool{match(a,b){(None,None)=>true,(Some(a),Some(b))=>units(a,b),_=>false}}
fn attr_key(a:&AttributeName,b:&AttributeName)->std::cmp::Ordering{a.namespace_uri().cmp(&b.namespace_uri()).then_with(||a.local().cmp(b.local()))}
fn parsed(node:&Node)->Result<View<'_>,String>{Ok(match &node.kind{
 NodeKind::Element{tag,expanded_name,attrs,..}=>{use crate::html::element_names as names;let namespace=match names::namespace(node.namespace,expanded_name)?{names::NamespaceRef::Null=>None,names::NamespaceRef::Static(s)=>Some(DomStringRef::Scalar(s)),names::NamespaceRef::Owned(s)=>Some(s.view())};View::Element{namespace,prefix:names::prefix(node.namespace,expanded_name)?,local:names::local(node.namespace,tag,expanded_name)?,attrs}},
 NodeKind::Text(data)=>View::Data(3,None,data),NodeKind::Cdata(data)=>View::Data(4,None,data),NodeKind::Comment(data)=>View::Data(8,None,data),NodeKind::ProcessingInstruction{target,data}=>View::Data(7,Some(target),data),NodeKind::DocumentType(data)=>View::Doctype(data),NodeKind::DocumentFragment=>View::Fragment,
})}
impl View<'_>{
 fn work(&self)->Result<usize,String>{let mut n=1usize;let mut add=|bytes:usize|->Result<(),String>{n=n.checked_add(bytes).ok_or("Node equality data work overflow.")?;Ok(())};match self{
  Self::Element{namespace,prefix,local,attrs}=>{if let Some(s)=namespace{add(s.logical_bytes())?;}if let Some(s)=prefix{add(s.used_bytes())?;}add(local.logical_bytes())?;let sorting=(usize::BITS-attrs.len().leading_zeros())as usize+1;for attr in *attrs{add(attr.name.bytes().checked_mul(sorting+2).and_then(|v|v.checked_add(attr.value.used_bytes())).ok_or("Node equality attribute work overflow.")?)?;}},
  Self::Attr(name,data)=>{add(name.bytes())?;add(data.used_bytes())?;},Self::Data(_,target,data)=>{add(target.map_or(0,str::len))?;add(data.used_bytes())?;},Self::Doctype(d)=>{add(d.name.used_bytes())?;add(d.public_id.used_bytes())?;add(d.system_id.used_bytes())?;},_=>{}
 }Ok(n)}
 fn attribute_count(&self)->usize{if let Self::Element{attrs,..}=self{attrs.len()}else{0}}
}
impl Bridge<'_>{
 pub(in crate::scripting) fn validate_node_equality(&self,object:u32,args:&[Value])->Result<(),(&'static str,String)>{
  self.equality_access(object)?;match args.first(){None=>Err(("TypeError","Node equality requires a nullable Node argument.".into())),Some(Value::Null|Value::Undefined)=>Ok(()),Some(Value::Host(other))=>self.equality_access(*other),_=>Err(("TypeError","Node equality requires a genuine Node or null.".into()))}
 }
 fn equality_access(&self,object:u32)->Result<(),(&'static str,String)>{
  if self.inert_node_type(object).is_some(){if self.state.navigation_incumbent.is_some_and(|window|window!=WINDOW){self.frame_window_access(self.state.navigation_incumbent,WINDOW).map_err(|e|("SecurityError",e))?;}return Ok(());}
  self.contains_node_identity(object).map(|_|())
 }
 fn equality_view(&self,object:u32)->Result<View<'_>,String>{
  if let Some(id)=script_attr::index(object).filter(|_|self.attr_receiver(object)){let attr=&self.state.attributes[id];let value=if let Some(owner)=attr.owner{&self.element_attribute_source(owner).ok_or("Missing equality Attr owner.")?.iter().find(|entry|entry.name.same_expanded_name(&attr.name)).ok_or("Missing equality attached Attr.")?.value}else{&attr.value};return Ok(View::Attr(&attr.name,value));}
  if object==DOCUMENT{return Ok(View::Document(false));}
  if let Some(item)=self.state.inert_documents.node(object){return Ok(match &item.kind{
   Kind::Document(doc)=>View::Document(doc.xml_interface),Kind::TemplateDocument{..}=>View::Document(false),Kind::Fragment{..}=>View::Fragment,Kind::Parsed{source,..}=>return parsed(self.parsed_source(*source).ok_or("Missing equality snapshot.")?.0),Kind::Element(tag)=>View::Element{namespace:Some(DomStringRef::Scalar(HTML_NAMESPACE)),prefix:None,local:DomStringRef::Scalar(tag),attrs:&[]},Kind::Text(data)=>View::Data(3,None,data),Kind::ProcessingInstruction{target,data}=>View::Data(7,Some(target),data),Kind::Doctype(data)=>View::Doctype(data),
  });}
  if let Some((owner,node))=self.frame_contains_node(object){if node==0{return Ok(View::Document(false));}return parsed(self.contains_child_document(owner)?.nodes.get(node).ok_or("Missing equality child Node.")?);}
  parsed(self.doc.nodes.get(node_index(object).ok_or("Missing equality Node.")?).ok_or("Missing equality Node index.")?)
 }
 fn equality_children(&mut self,object:u32)->Result<Vec<u32>,String>{
  if let Some((owner,node))=self.frame_contains_node(object){let count=self.contains_child_document(owner)?.nodes[node].children.len();self.state.work(count+1)?;self.state.copies(count.checked_mul(size_of::<u32>()).ok_or("Node equality child metadata overflow.")?)?;let mut result=Vec::new();result.try_reserve_exact(count).map_err(|_|"Cannot reserve equality child links.")?;for &child in &self.contains_child_document(owner)?.nodes[node].children{result.push(self.frame_child_node_handle(owner,child)?);}return Ok(result);}
  self.normalize_children(object)
 }
 pub(in crate::scripting) fn node_equality(&mut self,object:u32,args:&[Value],same:bool)->Result<Value,String>{
  self.validate_node_equality(object,args).map_err(|(_,e)|e)?;self.state.work(1)?;let Some(Value::Host(other))=args.first()else{return Ok(Value::Bool(false));};if same||object==*other{return Ok(Value::Bool(object==*other));}self.state.rebuild(self.doc)?;
  let mut stack=Vec::new();self.state.copies(size_of::<(u32,u32)>())?;stack.try_reserve_exact(1).map_err(|_|"Cannot reserve Node equality stack.")?;stack.push((object,*other));let maximum=self.doc.nodes.len().checked_add(self.inert_node_count()).and_then(|n|n.checked_add(self.state.blank_frame_nodes)).and_then(|n|n.checked_add(self.state.attributes.len())).and_then(|n|n.checked_add(1)).ok_or("Node equality traversal bound overflow.")?;let mut visited=0usize;
  while let Some((a,b))=stack.pop(){visited=visited.checked_add(1).filter(|&n|n<=maximum).ok_or("Invalid or cyclic Node equality tree.")?;self.equality_access(a).map_err(|(_,e)|e)?;self.equality_access(b).map_err(|(_,e)|e)?;
   // Attached Attr lookup visits the owner's actual attribute list. Charge
   // that scan before borrowing its current value, including long name keys.
   for handle in [a,b]{if let Some(attr)=script_attr::index(handle).and_then(|id|self.state.attributes.get(id)){if let Some(owner)=attr.owner{let cost=self.element_attribute_source(owner).ok_or("Missing equality Attr owner.")?.iter().try_fold(1usize,|n,a|n.checked_add(a.name.bytes()).and_then(|n|n.checked_add(1)).ok_or("Node equality Attr lookup work overflow."))?;self.state.work(cost)?;}}}
   let cost=self.equality_view(a)?.work()?.checked_add(self.equality_view(b)?.work()?).ok_or("Node equality work overflow.")?;let count=self.equality_view(b)?.attribute_count();self.state.work(cost)?;self.state.copies(count.checked_mul(size_of::<usize>()).ok_or("Node equality attribute index overflow.")?)?;
   let equal=match(self.equality_view(a)?,self.equality_view(b)?){
    (View::Document(a),View::Document(b))=>a==b,(View::Fragment,View::Fragment)=>true,
    (View::Data(ak,at,ad),View::Data(bk,bt,bd))=>ak==bk&&at==bt&&ad==bd,
    (View::Doctype(a),View::Doctype(b))=>a.name==b.name&&a.public_id==b.public_id&&a.system_id==b.system_id,
    (View::Attr(an,av),View::Attr(bn,bv))=>attr_key(an,bn).is_eq()&&av==bv,
    (View::Element{namespace:an,prefix:ap,local:al,attrs:aa},View::Element{namespace:bn,prefix:bp,local:bl,attrs:ba})=>{
     if !nullable(an,bn)||ap!=bp||!units(al,bl)||aa.len()!=ba.len(){false}else{let mut order=Vec::new();order.try_reserve_exact(ba.len()).map_err(|_|"Cannot reserve Node equality attribute index.")?;order.extend(0..ba.len());order.sort_unstable_by(|&a,&b|attr_key(&ba[a].name,&ba[b].name));aa.iter().all(|attr|order.binary_search_by(|&at|attr_key(&ba[at].name,&attr.name)).is_ok_and(|at|ba[order[at]].value==attr.value))}
    },_=>false};if !equal{return Ok(Value::Bool(false));}
   let ac=self.equality_children(a)?;let bc=self.equality_children(b)?;if ac.len()!=bc.len(){return Ok(Value::Bool(false));}self.state.copies(ac.len().checked_mul(size_of::<(u32,u32)>()).ok_or("Node equality stack storage overflow.")?)?;stack.try_reserve(ac.len()).map_err(|_|"Cannot grow Node equality stack.")?;stack.extend(ac.into_iter().zip(bc).rev());
  }Ok(Value::Bool(true))
 }
}

#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn node_equality_metadata_refusal_is_read_only_and_recovers(){
  let mut doc=crate::html::parse("<html><body></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);assert_eq!(page.console_eval(&mut doc,&mut forms,"var a=document.createElement('div');a.setAttribute('data-x','native');a.appendChild(document.createTextNode('units'));var b=a.cloneNode(true);").errors,0);
  let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let Value::Host(a)=page.runtime.execute("a",&mut bridge).unwrap()else{panic!("a")};let Value::Host(b)=page.runtime.execute("b",&mut bridge).unwrap()else{panic!("b")};bridge.state.rebuild(bridge.doc).unwrap();let before=(bridge.state.retained,bridge.doc.nodes.len(),bridge.state.inert_documents.entries.len(),bridge.state.attributes.len(),bridge.state.dom_commits);let copies=bridge.state.copy_work;bridge.state.copy_work=0;assert!(bridge.node_equality(a,&[Value::Host(b)],false).is_err());assert_eq!((bridge.state.retained,bridge.doc.nodes.len(),bridge.state.inert_documents.entries.len(),bridge.state.attributes.len(),bridge.state.dom_commits),before);assert!(matches!(bridge.node_equality(a,&[Value::Host(a)],true).unwrap(),Value::Bool(true)));bridge.state.copy_work=copies;assert!(matches!(bridge.node_equality(a,&[Value::Host(b)],false).unwrap(),Value::Bool(true)));assert_eq!((bridge.state.retained,bridge.doc.nodes.len(),bridge.state.inert_documents.entries.len(),bridge.state.attributes.len(),bridge.state.dom_commits),before);
 }
}
