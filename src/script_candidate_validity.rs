//! Live candidate flags share native control state and checked error storage.
//! https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#the-constraint-validation-api
use super::*;
impl Bridge<'_>{
    pub(super) fn validate_candidate_validity_flag(&self,object:u32,key:&str)->Result<(),(&'static str,String)>{
        self.control_validation_authority(object)?;
        if key=="customError"||key=="valid"&&self.state.output_custom_errors.get(&object).is_some()
            ||self.control_eligibility_class(object)==Some("HTMLButtonElement"){return Ok(());}
        let node=node_index(object).filter(|&n|n<self.doc.nodes.len()).ok_or_else(||("NotSupportedError","This validity flag requires its native control node.".into()))?;
        let c=self.forms.controls.get(&node).ok_or_else(||("NotSupportedError","This validity flag requires its admitted native control state.".into()))?;
        if c.kind==Kind::Unsupported||key!="valueMissing"&&matches!(c.kind,Kind::Text|Kind::TextArea)
            &&(self.doc.attr(node,"minlength").is_some()||self.doc.attr(node,"maxlength").is_some()
                ||c.kind==Kind::Text&&self.doc.attr(node,"pattern").is_some()){
            return Err(("NotSupportedError","This validity flag requires a broader native constraint-validation implementation.".into()));
        }
        Ok(())
    }
    pub(super) fn candidate_validity_flag(&mut self,object:u32,key:&str)->Result<Value,String>{
        self.validate_candidate_validity_flag(object,key).map_err(|(_,e)|e)?;
        // Canonical valid reads also inspect the error during receiver/call
        // validation. Admit an upper bound for those three checks and this read.
        let lookups=if key=="valid"{4}else{1};
        self.state.work(self.state.output_custom_errors.lookup_work().checked_mul(lookups).ok_or("Validity lookup work overflow.")?)?;
        script_text_content::deadline(self.state)?;
        let custom=self.state.output_custom_errors.get(&object).is_some();
        if key=="customError"{return Ok(Value::Bool(custom));}
        if key=="valid"&&custom{return Ok(Value::Bool(false));}
        self.state.rebuild(self.doc)?;let node=self.node(object)?;
        self.state.work(forms::validation_attribute_work(self.doc,node)?)?;
        if self.control_eligibility_class(object)==Some("HTMLButtonElement"){
            return Ok(Value::Bool(key=="valid"));
        }
        let count=self.forms.controls.len();
        self.state.work(count.checked_mul(2).and_then(|n|n.checked_add(1)).ok_or("Validity group work overflow.")?)?;
        let missing=self.forms.constraint_value_missing(&self.forms.controls[&node]);
        script_text_content::deadline(self.state)?;
        Ok(Value::Bool(match key{"valueMissing"=>missing,"valid"=>!missing,_=>false}))
    }
}
