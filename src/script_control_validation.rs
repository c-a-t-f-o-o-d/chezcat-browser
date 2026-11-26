//! Per-control required-state validation shares synchronous native dispatch.
use super::*;
impl Bridge<'_>{
    pub(super) fn control_validation_member(&self,object:u32,key:&str)->bool{
        matches!(key,"validationMessage"|"checkValidity"|"reportValidity")&&self.control_eligibility_class(object).is_some()
    }
    pub(super) fn control_validation_authority(&self,object:u32)->Result<(),(&'static str,String)>{
        if self.control_eligibility_class(object).is_none(){return Err(("TypeError","Control validation requires a genuine supported HTML control.".into()));}
        self.native_node_access(object)?;
        if self.is_frame_handle(object)||self.inert_interface(object).is_some(){return Err(("NotSupportedError","This control requires its native validation state and event lifecycle backend.".into()));}
        Ok(())
    }
    pub(super) fn validate_control_validation(&self,object:u32,class:&str)->Result<(),(&'static str,String)>{
        if self.control_eligibility_class(object)!=Some(class){return Err(("TypeError","Control validation requires its genuine concrete HTML interface.".into()));}
        self.control_validation_authority(object)
    }
    fn native_control_missing_typed(&mut self,object:u32)->Result<bool,(&'static str,String)>{
        self.control_validation_authority(object)?;
        if !matches!(self.control_eligibility_get(object).map_err(|e|("Error",e))?,Value::Bool(true)){return Ok(false);}
        self.state.work(self.state.output_custom_errors.lookup_work()).map_err(|e|("Error",e))?;
        if self.state.output_custom_errors.get(&object).is_some(){return Ok(true);}
        let node=self.node(object).map_err(|e|("Error",e))?;
        let c=self.forms.controls.get(&node).ok_or_else(||("NotSupportedError","This candidate requires its admitted native validation state.".into()))?;
        if c.kind==Kind::Unsupported
            ||c.kind==Kind::Text&&self.doc.attr(node,"pattern").is_some(){
            return Err(("NotSupportedError","This candidate requires a broader native constraint-validation implementation.".into()));
        }
        let count=self.forms.controls.len();let work=count.checked_mul(2).and_then(|n|n.checked_add(1)).ok_or_else(||("Error","Control validation group work overflow.".into()))?;
        let work=work.checked_add(self.forms.length_validation_work(self.doc,node)).ok_or_else(||("Error","Length validation work overflow.".into()))?;
        self.state.work(work).map_err(|e|("Error",e))?;
        let(long,short)=self.forms.length_validation_flags(self.doc,node);let missing=self.forms.control_value_missing(&self.forms.controls[&node])||long||short;script_text_content::deadline(self.state).map_err(|e|("Error",e))?;Ok(missing)
    }
    fn native_control_missing(&mut self,object:u32)->Result<bool,String>{self.native_control_missing_typed(object).map_err(|(_,e)|e)}
    pub(super) fn prepare_native_control_validation(&mut self,object:u32,report:Option<bool>)->Result<crate::javascript::NativeControlValidation,(&'static str,String)>{
        let missing=self.native_control_missing_typed(object)?;
        match report{
            None=>self.control_validation_message_value(object,missing).map(crate::javascript::NativeControlValidation::Message).map_err(|e|("Error",e)),
            Some(report)=>self.control_validation_snapshot(object,report,missing).map(crate::javascript::NativeControlValidation::Dispatch).map_err(|e|("Error",e)),
        }
    }
    pub(super) fn control_validation_message(&mut self,object:u32)->Result<Value,String>{
        let missing=self.native_control_missing(object)?;self.control_validation_message_value(object,missing)
    }
    fn control_validation_message_value(&mut self,object:u32,missing:bool)->Result<Value,String>{
        if !missing{return self.string("");}
        self.state.work(self.state.output_custom_errors.lookup_work())?;
        if let Some(text)=self.state.output_custom_errors.get(&object){
            let bytes=text.logical_bytes();if bytes>self.state.limits.host.string_bytes{return Err("Custom validation message exceeds the DOM string allowance.".into());}
            self.state.copies(bytes)?;script_text_content::deadline(self.state)?;
            return Self::dom_string_copy(self.state.output_custom_errors.get(&object).unwrap());
        }
        let node=self.node(object)?;self.state.work(self.forms.length_validation_work(self.doc,node))?;
        let(prefix,name)=self.forms.validation_message_parts(self.doc,node);
        let bytes=prefix.len().checked_add(name.len()).and_then(|n|n.checked_add(1)).ok_or("Control validation message size overflow.")?;
        if bytes>self.state.limits.host.string_bytes{return Err("Control validation message exceeds the DOM string allowance.".into());}
        self.state.copies(bytes)?;script_text_content::deadline(self.state)?;
        let mut text=String::new();text.try_reserve_exact(bytes).map_err(|_|"Could not reserve control validation message.")?;
        text.push_str(prefix);text.push_str(name);text.push('.');Ok(Value::String(text))
    }
    pub(super) fn begin_native_control_validation(&mut self,object:u32,report:bool)->Result<usize,String>{
        let missing=self.native_control_missing(object)?;self.control_validation_snapshot(object,report,missing)
    }
    fn control_validation_snapshot(&mut self,object:u32,report:bool,missing:bool)->Result<usize,String>{
        let node=self.node(object)?;
        let available=self.state.retained_limit.saturating_sub(self.state.retained);
        let snapshot=forms::ValidationSnapshot::candidate(node,missing,available)?;
        self.publish_validation_snapshot(object,report,snapshot,available)
    }
}
