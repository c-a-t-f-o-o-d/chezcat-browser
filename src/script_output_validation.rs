//! OUTPUT is not submittable and therefore is never a validation candidate.
//! https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#the-constraint-validation-api
use super::*;
pub(super) const WILL_GET:&str="\0HTMLOutputElement.get.willValidate";
pub(super) const MESSAGE_GET:&str="\0HTMLOutputElement.get.validationMessage";
pub(super) const CHECK:&str="\0HTMLOutputElement.checkValidity";
pub(super) const REPORT:&str="\0HTMLOutputElement.reportValidity";
pub(super) fn operation(method:&str)->bool{matches!(method,WILL_GET|MESSAGE_GET|CHECK|REPORT)}
impl Bridge<'_>{
 pub(super) fn output_validation_member(&self,object:u32,property:&str)->bool{matches!(property,"willValidate"|"validationMessage"|"checkValidity"|"reportValidity")&&self.html_output_receiver(object)}
 pub(super) fn output_validation_readonly(&self,object:u32,property:&str)->bool{matches!(property,"willValidate"|"validationMessage")&&self.html_output_receiver(object)}
 pub(super) fn output_validation(&mut self,object:u32,method:&str)->Result<Value,String>{
  self.validate_output_metadata(object).map_err(|(_,message)|message)?;
  self.state.work(1)?;
  script_text_content::deadline(self.state)?;
  // No validity-state lookup, event dispatch, author conversion or UI action:
  // none is part of the noncandidate branches of these native algorithms.
  match method{WILL_GET=>Ok(Value::Bool(false)),MESSAGE_GET=>Ok(Value::String(String::new())),CHECK|REPORT=>Ok(Value::Bool(true)),_=>Err("Unknown OUTPUT validation operation.".into())}
 }
}
