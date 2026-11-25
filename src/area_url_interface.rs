//! Canonical URL component operations for HTMLAreaElement.
macro_rules! component {($name:literal)=>{($name,concat!("\0HTMLAreaElement.get.",$name),Some(concat!("\0HTMLAreaElement.set.",$name)))};}
pub(crate) const MEMBERS:&[(&str,&str,Option<&str>)]=&[
 ("origin","\0HTMLAreaElement.get.origin",None),
 component!("protocol"),component!("username"),component!("password"),component!("host"),component!("hostname"),component!("port"),component!("pathname"),component!("search"),component!("hash"),
];
pub(crate) fn operation(method:&str)->Option<(&'static str,bool)>{
 MEMBERS.iter().find_map(|&(name,get,set)|if method==get{Some((name,false))}else if set==Some(method){Some((name,true))}else{None})
}
