//! Ordered qualified attribute names from the real element storage.
use super::*;
impl Bridge<'_> {
    pub(super) fn ordinary_attribute_method(&self,object:u32,property:&str)->bool{
        self.attribute_method_member(object,property)&&!self.form_named_control(object,property)
    }
    pub(super) fn attribute_method_member(&self,object:u32,property:&str)->bool{
        matches!(property,"getAttributeNS"|"getAttributeNodeNS"|"setAttributeNS"|"removeAttributeNS"|"getAttribute"|"getAttributeNode"|"getAttributeNames"|"hasAttribute"|"removeAttribute"|"setAttribute")&&self.attribute_names_receiver(object)
    }
    pub(super) fn attribute_names_receiver(&self,object:u32)->bool {
        self.inert_node_type(object)==Some(1)||self.attr_element_receiver(object)
    }
    pub(super) fn element_attribute_source(&self,object:u32)->Option<&[crate::html::Attribute]>{
        if self.inert_node_type(object).is_some(){return self.inert_element_attributes(object);}
        let node=self.doc.nodes.get(node_index(object)?)?;
        if let NodeKind::Element{attrs,..}=&node.kind{Some(attrs)}else{None}
    }
    pub(super) fn attribute_names(&mut self,object:u32)->Result<Vec<DomString>,String>{
        if !self.attribute_names_receiver(object){return Err("getAttributeNames requires a genuine Element.".into());}
        let attrs=self.element_attribute_source(object).ok_or("Element attribute storage is unavailable.")?;
        let count=attrs.len();let text=attrs.iter().map(|attr|attr.name.qualified.used_bytes()).sum::<usize>();
        self.state.work(count.saturating_add(text))?;self.state.copies(text.saturating_add(count.saturating_mul(size_of::<DomString>())))?;
        let available=self.state.retained_limit.saturating_sub(self.state.retained);
        count.checked_mul(size_of::<DomString>()).and_then(|bytes|bytes.checked_add(text)).filter(|bytes|*bytes<=available).ok_or("Attribute name view scratch allowance exhausted.")?;
        let mut names=Vec::new();names.try_reserve_exact(count).map_err(|_|"Could not reserve attribute name view.")?;
        let mut bytes=names.capacity().checked_mul(size_of::<DomString>()).filter(|bytes|*bytes<=available).ok_or("Attribute name view allocation exceeds allowance.")?;
        for index in 0..count {
            let name=&self.element_attribute_source(object).ok_or("Element attributes are unavailable.")?[index].name.qualified;
            if name.used_bytes()>available.saturating_sub(bytes){return Err("Attribute name text allowance exhausted.".into());}
            let copied=name.try_clone()?;
            bytes=bytes.checked_add(copied.capacity()).filter(|bytes|*bytes<=available).ok_or("Attribute name text allocation exceeds allowance.")?;
            names.push(copied);
        }
        Ok(names)
    }
}
