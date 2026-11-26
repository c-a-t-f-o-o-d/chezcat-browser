//! Read-only native candidate eligibility over current attributes and ancestry.
//! https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#dom-cva-willvalidate
use super::*;
pub(super) fn interface(method:&str)->Option<&'static str>{match method{
    "\0HTMLInputElement.get.willValidate"=>Some("HTMLInputElement"),
    "\0HTMLButtonElement.get.willValidate"=>Some("HTMLButtonElement"),
    "\0HTMLSelectElement.get.willValidate"=>Some("HTMLSelectElement"),
    "\0HTMLTextAreaElement.get.willValidate"=>Some("HTMLTextAreaElement"),_=>None,
}}
impl Bridge<'_>{
    pub(super) fn control_eligibility_class(&self,object:u32)->Option<&'static str>{
        self.concrete_html_interface(object).or_else(||self.inert_interface(object)).filter(|c|matches!(*c,"HTMLInputElement"|"HTMLButtonElement"|"HTMLSelectElement"|"HTMLTextAreaElement"))
    }
    pub(super) fn control_eligibility_member(&self,object:u32,name:&str)->bool{name=="willValidate"&&self.control_eligibility_class(object).is_some()}
    pub(super) fn validate_control_eligibility(&self,object:u32,class:&str)->Result<(),(&'static str,String)>{
        if self.control_eligibility_class(object)!=Some(class){return Err(("TypeError","Candidate eligibility requires its genuine control interface.".into()));}
        self.native_node_access(object)?;
        if self.is_frame_handle(object)||self.inert_interface(object).is_some(){return Err(("NotSupportedError","This control requires its native eligibility and tree lifecycle backend.".into()));}
        Ok(())
    }
    pub(super) fn control_eligibility_get(&mut self,object:u32)->Result<Value,String>{
        let class=self.control_eligibility_class(object).ok_or("Candidate eligibility receiver is unavailable.")?;
        self.validate_control_eligibility(object,class).map_err(|(_,e)|e)?;
        self.state.work(1)?;script_text_content::deadline(self.state)?;
        self.state.rebuild(self.doc)?;let node=self.node(object)?;
        self.state.work(forms::validation_attribute_work(self.doc,node)?)?;
        let mut remaining=self.state.host_work;
        let disabled=forms::disabled_with_work(self.doc,node,&self.state.tree.parents,&mut |amount|{
            if let Some(next)=remaining.checked_sub(amount){remaining=next;true}else{remaining=0;false}
        });self.state.host_work=remaining;
        let disabled=disabled.ok_or("Candidate disabled ancestry or work allowance is unavailable.")?;
        let parent=self.state.tree.parents.get(node).copied().flatten();
        let parent_select=parent.is_some_and(|p|self.doc.nodes[p].namespace==crate::html::Namespace::Html&&tag(self.doc,p)=="select");
        let mut ancestor=parent;let mut in_datalist=false;let mut walked=0usize;
        while let Some(current)=ancestor{
            walked=walked.checked_add(1).filter(|&n|n<=self.doc.nodes.len()).ok_or("Invalid candidate ancestry.")?;self.state.work(1)?;
            if self.doc.nodes.get(current).is_none(){return Err("Candidate ancestor is unavailable.".into());}
            if self.doc.nodes[current].namespace==crate::html::Namespace::Html&&tag(self.doc,current)=="datalist"{in_datalist=true;break;}
            ancestor=self.state.tree.parents.get(current).copied().flatten();
        }
        script_text_content::deadline(self.state)?;
        Ok(Value::Bool(forms::validation_candidate(self.doc,node,disabled,in_datalist,parent_select)))
    }
}
