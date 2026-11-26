//! Fixed native boundary records; dictionary access stays in the interpreter.
//! https://dom.spec.whatwg.org/#interface-staticrange
use super::*;
impl Bridge<'_> {
    pub(in crate::scripting) fn validate_static_range_member(&self,name:&str,value:&Value)->Result<(),(&'static str,String)>{
        if matches!(value,Value::Undefined) {return Err(("TypeError",format!("StaticRange requires {name}.")));}
        if matches!(name,"startContainer"|"endContainer") {
            if !matches!(value,Value::Host(node) if *node==DOCUMENT || node_index(*node).is_some_and(|index|self.dom_accessible(index)) || self.inert_node_type(*node).is_some() || self.attr_receiver(*node) || self.frame_node_interface(*node).is_some()) {
                return Err(("TypeError","StaticRange container must be a genuine Node.".into()));
            }
        } else if !matches!(value,Value::Number(_)) {return Err(("TypeError","StaticRange offset must be converted numeric data.".into()));}
        Ok(())
    }
    pub(in crate::scripting) fn validate_static_range_fields(&self,fields:Option<&[(&str,Value)]>)->Result<(),(&'static str,String)>{
        let fields=fields.ok_or(("TypeError","StaticRange requires a complete dictionary.".into()))?;
        for &(name,ref value) in fields {self.validate_static_range_member(name,value)?;}
        for name in ["startContainer","endContainer"] {
            let Some((_,Value::Host(node)))=fields.iter().find(|(key,_)|*key==name) else {return Err(("TypeError","StaticRange requires both containers.".into()));};
            if self.attr_receiver(*node) || self.inert_node_type(*node)==Some(10) || node_index(*node).and_then(|index|self.doc.nodes.get(index)).is_some_and(|node|matches!(node.kind,NodeKind::DocumentType(_))) {
                return Err(("InvalidNodeTypeError","StaticRange cannot use Attr or DocumentType containers.".into()));
            }
        }
        for name in ["startContainer","endContainer"] {
            let Some((_,Value::Host(node)))=fields.iter().find(|(key,_)|*key==name) else {unreachable!()};
            self.range_node(*node).map_err(|message|("NotSupportedError",message))?;
        }
        Ok(())
    }
    pub(in crate::scripting) fn create_static_range(&mut self,owner:u32,fields:Option<&[(&str,Value)]>)->Result<Value,String>{
        let fields=fields.ok_or("StaticRange dictionary was not prepared.")?;
        let value=|name|fields.iter().find(|(key,_)|*key==name).map(|(_,value)|value).ok_or("StaticRange member is missing.");
        let Value::Host(start)=value("startContainer")? else {return Err("StaticRange start is not native.".into());};
        let Value::Host(end)=value("endContainer")? else {return Err("StaticRange end is not native.".into());};
        let record=Record{fixed:true,owner,start:Point{node:*start,offset:range_offset(value("startOffset")?)?},end:Point{node:*end,offset:range_offset(value("endOffset")?)?}};
        self.copy_range(record)
    }
}
