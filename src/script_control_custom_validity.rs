//! Native candidate errors reuse the checked shared per-element error store.
use super::*;
pub(super) fn snapshot_work(state:&State,controls:usize)->Result<usize,String>{
    if state.output_custom_errors.len()==0{return Ok(0);}
    controls.checked_mul(state.output_custom_errors.lookup_work()).ok_or("Custom validation lookup work overflow.".into())
}
impl Page{
    pub(crate) fn control_validation_report_message(&self,doc:&Document,forms:&Forms,node:usize)->String{
        self.state.output_custom_errors.get(&node_handle(node)).map_or_else(||forms.validation_message(doc,node),|message|message.render_projection().to_owned())
    }
}
impl Bridge<'_>{
    pub(super) fn control_custom_validity_member(&self,object:u32,key:&str)->bool{key=="setCustomValidity"&&self.control_eligibility_class(object).is_some()}
    pub(super) fn validate_control_custom_validity(&self,object:u32,class:&str)->Result<(),(&'static str,String)>{
        if self.control_eligibility_class(object)!=Some(class){return Err(("TypeError","Custom validity requires its genuine concrete HTML control interface.".into()));}
        self.control_validation_authority(object)
    }
    pub(super) fn control_custom_validity_call(&mut self,object:u32,args:&[Value])->Result<Value,String>{
        let class=self.control_eligibility_class(object).ok_or("Custom validity interface is unavailable.")?;
        self.validate_control_custom_validity(object,class).map_err(|(_,e)|e)?;
        self.state.work(1)?;script_text_content::deadline(self.state)?;
        self.write_control_custom_error(object,args)
    }
}
