//! Native ProcessingInstruction pseudo-attribute operations.
use super::Value;
pub(crate) const METHODS:[(&str,&str,usize);7]=[
 ("hasAttributes","\0PI.hasAttributes",0),("getAttributeNames","\0PI.getAttributeNames",0),
 ("getAttribute","\0PI.getAttribute",1),("setAttribute","\0PI.setAttribute",2),
 ("removeAttribute","\0PI.removeAttribute",1),("toggleAttribute","\0PI.toggleAttribute",1),("hasAttribute","\0PI.hasAttribute",1)];
pub(crate) fn operation(method:&str)->Option<&'static str>{METHODS.iter().find(|(_,m,_)|*m==method).map(|(n,_,_)|*n)}
pub(crate) fn arity(method:&str)->usize{METHODS.iter().find(|(_,m,_)|*m==method).map_or(0,|(_,_,n)|*n)}
pub(crate) fn conversions(method:&str)->&'static[(usize,bool)]{match operation(method){Some("setAttribute")=>&[(0,true),(1,true)],Some("getAttribute"|"removeAttribute"|"toggleAttribute"|"hasAttribute")=>&[(0,true)],_=>&[]}}
pub enum Prepared{Names(Vec<crate::dom_string::DomString>),Result(Value),Write{data:Value,result:Value}}
