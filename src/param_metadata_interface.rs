//! The remaining obsolete Param DOMString attributes share exact reflection.
pub(crate) const MEMBERS:&[(&str,&str,&str,&str)]=&[
 ("value","value","\0HTMLParamElement.get.value","\0HTMLParamElement.set.value"),
 ("type","type","\0HTMLParamElement.get.type","\0HTMLParamElement.set.type"),
 ("valueType","valuetype","\0HTMLParamElement.get.valueType","\0HTMLParamElement.set.valueType"),
];
pub(crate) fn attribute(property:&str)->Option<&'static str>{MEMBERS.iter().find(|&&(name,_,_,_)|name==property).map(|&(_,attribute,_,_)|attribute)}
pub(crate) fn operation(method:&str)->Option<(&'static str,bool)>{MEMBERS.iter().find_map(|&(name,_,get,set)|if method==get{Some((name,false))}else if method==set{Some((name,true))}else{None})}
