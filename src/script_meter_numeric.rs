//! Native METER numeric metadata over actual null-namespace attributes.
//! https://html.spec.whatwg.org/multipage/form-elements.html#the-meter-element
use super::*;
pub(super) const VALUE_GET:&str="\0HTMLMeterElement.get.value";
pub(super) const VALUE_SET:&str="\0HTMLMeterElement.set.value";
pub(super) const MIN_GET:&str="\0HTMLMeterElement.get.min";
pub(super) const MIN_SET:&str="\0HTMLMeterElement.set.min";
pub(super) const MAX_GET:&str="\0HTMLMeterElement.get.max";
pub(super) const MAX_SET:&str="\0HTMLMeterElement.set.max";
pub(super) const LOW_GET:&str="\0HTMLMeterElement.get.low";
pub(super) const LOW_SET:&str="\0HTMLMeterElement.set.low";
pub(super) const HIGH_GET:&str="\0HTMLMeterElement.get.high";
pub(super) const HIGH_SET:&str="\0HTMLMeterElement.set.high";
pub(super) const OPTIMUM_GET:&str="\0HTMLMeterElement.get.optimum";
pub(super) const OPTIMUM_SET:&str="\0HTMLMeterElement.set.optimum";
pub(super) fn operation(method:&str)->bool{matches!(method,VALUE_GET|VALUE_SET|MIN_GET|MIN_SET|MAX_GET|MAX_SET|LOW_GET|LOW_SET|HIGH_GET|HIGH_SET|OPTIMUM_GET|OPTIMUM_SET)}
impl Bridge<'_>{
 pub(super) fn meter_numeric_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLMeterElement")}
 pub(super) fn meter_numeric_attribute(&self,object:u32,property:&str)->Option<&'static str>{if !self.meter_numeric_receiver(object){return None;}match property{"value"=>Some("value"),"min"=>Some("min"),"max"=>Some("max"),"low"=>Some("low"),"high"=>Some("high"),"optimum"=>Some("optimum"),_=>None}}
 pub(super) fn meter_numeric_member(&self,object:u32,property:&str)->bool{self.meter_numeric_attribute(object,property).is_some()}
 pub(super) fn validate_meter_numeric(&self,object:u32)->Result<(),(&'static str,String)>{if !self.meter_numeric_receiver(object){return Err(("TypeError","Meter numeric metadata requires a genuine HTML meter element.".into()));}self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child meter metadata requires its lifecycle hooks.".into()));}Ok(())}
 pub(super) fn meter_numeric_get(&mut self,object:u32,method:&str)->Result<Value,String>{
  self.validate_meter_numeric(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let min=self.html_number_attribute(object,"min")?.1.unwrap_or(0.0);
  let number=if method==MIN_GET{min}else{
   let max=self.html_number_attribute(object,"max")?.1.unwrap_or(1.0).max(min);
   match method{
    MAX_GET=>max,
    VALUE_GET=>self.html_number_attribute(object,"value")?.1.unwrap_or(0.0).clamp(min,max),
    LOW_GET=>self.html_number_attribute(object,"low")?.1.unwrap_or(min).clamp(min,max),
    HIGH_GET=>{let low=self.html_number_attribute(object,"low")?.1.unwrap_or(min).clamp(min,max);self.html_number_attribute(object,"high")?.1.unwrap_or(max).clamp(low,max)},
    OPTIMUM_GET=>self.html_number_attribute(object,"optimum")?.1.unwrap_or_else(||min.midpoint(max)).clamp(min,max),
    _=>return Err("Unknown meter numeric getter.".into())
   }
  };script_text_content::deadline(self.state)?;Ok(Value::Number(number))
 }
 pub(super) fn meter_numeric_set(&mut self,object:u32,property:&str,value:&Value)->Result<(),String>{
  self.validate_meter_numeric(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let Value::Number(number)=value else{return Err("Meter numeric setters require number conversion.".into());};
  if !number.is_finite(){return Err("Meter numeric setters require finite doubles.".into());}
  let key=self.meter_numeric_attribute(object,property).ok_or("Unknown meter numeric setter.")?;
  let text=self.text(value)?;self.state.copies(key.len())?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text(key)?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(DomString::from(text)))}else{let node=self.node(object)?;self.public_attribute(node,name,Some(text))}
 }
}
