//! Owned expanded-name metadata. HTML/SVG names without prefixes use their
//! existing lossless static namespace classification. Explicit null/other URIs
//! and prefixes retain one fallibly allocated native record, never a JS shadow.
use super::{Namespace,DomString};
use crate::dom_string::DomStringRef;

#[derive(Debug)]
pub(crate) enum ElementNamespaceValue {Html,Svg,Null,Other(DomString)}
#[derive(Debug)]
pub(crate) struct ExpandedElementName {
    pub(crate) namespace:ElementNamespaceValue,
    pub(crate) prefix:Option<DomString>,
    /// Exact local units when the existing tag string is only a render projection.
    pub(crate) local:Option<DomString>,
}
pub(crate) enum NamespaceRef<'a>{Null,Static(&'static str),Owned(&'a DomString)}
pub(crate) fn namespace(class:Namespace,names:&[ExpandedElementName])->Result<NamespaceRef<'_>,String>{
    if names.len()>1{return Err("An Element has multiple expanded-name records.".into());}
    if let Some(name)=names.first(){
        if name.namespace.classification()!=class{return Err("Element namespace classification differs from its owned record.".into());}
        return Ok(match &name.namespace{ElementNamespaceValue::Html=>NamespaceRef::Static("http://www.w3.org/1999/xhtml"),ElementNamespaceValue::Svg=>NamespaceRef::Static("http://www.w3.org/2000/svg"),ElementNamespaceValue::Null=>NamespaceRef::Null,ElementNamespaceValue::Other(uri)=>NamespaceRef::Owned(uri)});
    }
    class.element_uri().map(NamespaceRef::Static).ok_or_else(||"Namespace lookup for unsupported foreign parser context has no native URI record.".into())
}
pub(crate) fn prefix(class:Namespace,names:&[ExpandedElementName])->Result<Option<&DomString>,String>{namespace(class,names)?;Ok(names.first().and_then(|name|name.prefix.as_ref()))}
pub(crate) fn local<'a>(class:Namespace,tag:&'a str,names:&'a [ExpandedElementName])->Result<DomStringRef<'a>,String>{namespace(class,names)?;Ok(names.first().and_then(|name|name.local.as_ref()).map_or(DomStringRef::Scalar(tag),DomString::view))}
pub(crate) fn local_matches(tag:&str,names:&[ExpandedElementName],query:&str,fold:bool)->bool{let actual=names.first().and_then(|name|name.local.as_ref()).map_or(DomStringRef::Scalar(tag),DomString::view);let lower=|u:u16|if fold&&(0x41..=0x5a).contains(&u){u+0x20}else{u};actual.code_units().map(lower).eq(query.encode_utf16().map(lower))}
pub(crate) fn raw_name(names:&[ExpandedElementName])->bool{names.first().is_some_and(|name|name.local.as_ref().is_some_and(|s|s.raw_units().is_some())||name.prefix.as_ref().is_some_and(|s|s.raw_units().is_some()))}
impl ElementNamespaceValue {
    pub(crate) fn classification(&self)->Namespace{match self{Self::Html=>Namespace::Html,Self::Svg=>Namespace::Svg,Self::Null|Self::Other(_)=>Namespace::Other}}
    fn owned_bytes(&self)->usize{match self{Self::Other(value)=>value.capacity(),_=>0}}
    fn used_bytes(&self)->usize{match self{Self::Other(value)=>value.used_bytes(),_=>0}}
    fn try_clone(&self)->Result<Self,String>{Ok(match self{Self::Html=>Self::Html,Self::Svg=>Self::Svg,Self::Null=>Self::Null,Self::Other(value)=>Self::Other(value.try_clone()?)})}
}
impl ExpandedElementName {
    pub(crate) fn owned_bytes(&self)->usize{self.namespace.owned_bytes()+self.prefix.as_ref().map_or(0,DomString::capacity)+self.local.as_ref().map_or(0,DomString::capacity)}
    pub(crate) fn used_bytes(&self)->usize{self.namespace.used_bytes()+self.prefix.as_ref().map_or(0,DomString::used_bytes)+self.local.as_ref().map_or(0,DomString::used_bytes)}
    pub(crate) fn try_clone(&self)->Result<Self,String>{Ok(Self{namespace:self.namespace.try_clone()?,prefix:self.prefix.as_ref().map(DomString::try_clone).transpose()?,local:self.local.as_ref().map(DomString::try_clone).transpose()?})}
}
fn copy_text(text:&str)->Result<String,String>{let mut copy=String::new();copy.try_reserve_exact(text.len()).map_err(|_|"Cannot reserve expanded element name.")?;copy.push_str(text);Ok(copy)}
pub(crate) fn copy(source:&[ExpandedElementName])->Result<Vec<ExpandedElementName>,String>{
    let mut names=Vec::new();names.try_reserve_exact(source.len()).map_err(|_|"Cannot reserve expanded element metadata.")?;
    for name in source{names.push(name.try_clone()?);}Ok(names)
}
pub(crate) fn owned_bytes(names:&Vec<ExpandedElementName>)->usize{names.capacity()*std::mem::size_of::<ExpandedElementName>()+names.iter().map(ExpandedElementName::owned_bytes).sum::<usize>()}
pub(crate) fn copied_bytes(names:&[ExpandedElementName])->usize{names.len()*std::mem::size_of::<ExpandedElementName>()+names.iter().map(ExpandedElementName::used_bytes).sum::<usize>()}
// Name/namespace validation precedes this allocation-only preparation.
pub(crate) fn prepare(namespace:Option<DomString>,qualified:String)->Result<(String,Vec<ExpandedElementName>,Namespace),String>{prepare_dom(namespace,qualified.into())}
pub(crate) fn prepare_dom(namespace:Option<DomString>,qualified:DomString)->Result<(String,Vec<ExpandedElementName>,Namespace),String>{
    let namespace=match namespace {
        None=>ElementNamespaceValue::Null,
        Some(value) if value.logical_bytes()==0=>ElementNamespaceValue::Null,
        Some(value) if value=="http://www.w3.org/1999/xhtml"=>ElementNamespaceValue::Html,
        Some(value) if value=="http://www.w3.org/2000/svg"=>ElementNamespaceValue::Svg,
        Some(value)=>ElementNamespaceValue::Other(value),
    };
    let class=namespace.classification();
    let(prefix,local)=qualified.view().split_colon();prepare_parts(namespace,prefix.map(DomStringRef::try_owned).transpose()?,local.try_owned()?)
}
pub(crate) fn prepare_local(namespace:Option<DomString>,local:DomString)->Result<(String,Vec<ExpandedElementName>,Namespace),String>{let namespace=match namespace{None=>ElementNamespaceValue::Null,Some(s) if s.logical_bytes()==0=>ElementNamespaceValue::Null,Some(s) if s=="http://www.w3.org/1999/xhtml"=>ElementNamespaceValue::Html,Some(s) if s=="http://www.w3.org/2000/svg"=>ElementNamespaceValue::Svg,Some(s)=>ElementNamespaceValue::Other(s)};prepare_parts(namespace,None,local)}
fn prepare_parts(namespace:ElementNamespaceValue,prefix:Option<DomString>,local:DomString)->Result<(String,Vec<ExpandedElementName>,Namespace),String>{
    let class=namespace.classification();let tag=copy_text(local.render_projection())?;let exact=if local.raw_units().is_some(){Some(local)}else{None};let mut expanded=Vec::new();if prefix.is_some()||exact.is_some()||class==Namespace::Other{expanded.try_reserve_exact(1).map_err(|_|"Cannot reserve native element name record.")?;expanded.push(ExpandedElementName{namespace,prefix,local:exact});}Ok((tag,expanded,class))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_expanded_names_owned_uri_prefix_local_and_independent_copy_keep_units(){
        let uri=DomString::from_units(&[0xd800],5).unwrap();let(local,names,class)=prepare(Some(uri),"p:MiXeD".into()).unwrap();
        assert_eq!(local,"MiXeD");assert_eq!(class,Namespace::Other);assert_eq!(names.len(),1);assert_eq!(names[0].prefix.as_deref(),Some("p"));
        let ElementNamespaceValue::Other(uri)=&names[0].namespace else{panic!("actual URI")};assert_eq!(uri.raw_units(),Some(&[0xd800][..]));
        let copied=copy(&names).unwrap();let ElementNamespaceValue::Other(other)=&copied[0].namespace else{panic!("copied URI")};
        assert_eq!(uri,other);assert_ne!(uri.raw_units().unwrap().as_ptr(),other.raw_units().unwrap().as_ptr());
        assert_eq!(owned_bytes(&copied),copied_bytes(&copied));assert_eq!(copied[0].prefix.as_deref(),Some("p"));
    }
    #[test]
    fn native_expanded_names_null_and_static_namespaces_remain_distinct(){
        let(local,null,class)=prepare(None,"Mixed".into()).unwrap();assert_eq!(local,"Mixed");assert_eq!(class,Namespace::Other);assert!(matches!(null[0].namespace,ElementNamespaceValue::Null));
        let(_,html,class)=prepare(Some("http://www.w3.org/1999/xhtml".into()),"div".into()).unwrap();assert_eq!(class,Namespace::Html);assert!(html.is_empty());
        let(_,svg,class)=prepare(Some("http://www.w3.org/2000/svg".into()),"s:circle".into()).unwrap();assert_eq!(class,Namespace::Svg);assert!(matches!(svg[0].namespace,ElementNamespaceValue::Svg));assert_eq!(svg[0].prefix.as_deref(),Some("s"));
        let(_,other,class)=prepare(Some("HTTP://www.w3.org/1999/xhtml".into()),"div".into()).unwrap();assert_eq!(class,Namespace::Other);assert!(matches!(other[0].namespace,ElementNamespaceValue::Other(_)));
    }
}
