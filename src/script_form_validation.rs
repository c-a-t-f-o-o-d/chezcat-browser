//! Checked synchronous validation for supported native required-field states.
//! https://html.spec.whatwg.org/multipage/forms.html#dom-form-checkvalidity
use super::*;
pub(super) struct Pending{
    form:u32,
    snapshot:forms::ValidationSnapshot,
    next:usize,
    current:Option<usize>,
    unhandled:Option<usize>,
    report:bool,
}
impl Page{
    pub(crate) fn take_validation_report(&mut self)->Option<Option<usize>>{self.state.validation_report.take()}
}
impl Bridge<'_>{
    pub(super) fn form_validation_member(&self,object:u32,name:&str)->bool{
        matches!(name,"checkValidity"|"reportValidity")&&self.form_length_receiver(object)&&!self.form_named_control(object,name)
    }
    fn form_validation_authority(&self,object:u32)->Result<(),(&'static str,String)>{
        if !self.form_length_receiver(object){return Err(("TypeError","Validation requires a genuine HTML form.".into()));}
        self.native_node_access(object)?;
        if self.is_frame_handle(object)||self.inert_interface(object).is_some(){return Err(("NotSupportedError","This form requires its native validation backend and tree lifecycle hooks.".into()));}
        Ok(())
    }
    pub(super) fn validate_form_validation(&self,object:u32)->Result<(),(&'static str,String)>{
        self.form_validation_authority(object)?;
        let owner=node_index(object).unwrap();
        // Refuse an unavailable constraint implementation instead of returning
        // a fabricated positive validity result for that candidate.
        for c in self.forms.controls.values(){
            if c.owner!=Some(owner)||c.disabled||matches!(c.kind,Kind::Text|Kind::TextArea)&&c.readonly{continue;}
            if c.kind==Kind::Unsupported{
                return Err(("NotSupportedError","This candidate requires a broader native constraint-validation implementation.".into()));
            }
        }
        Ok(())
    }
    fn validation_dispatch_authority(&self,object:u32)->Result<(),(&'static str,String)>{
        if self.control_eligibility_class(object).is_some(){self.control_validation_authority(object)}else{self.form_validation_authority(object)}
    }
    pub(super) fn begin_native_form_validation(&mut self,object:u32,report:bool)->Result<usize,String>{
        if self.control_eligibility_class(object).is_some(){return self.begin_native_control_validation(object,report);}
        self.validate_form_validation(object).map_err(|(_,e)|e)?;
        let count=self.forms.controls.len();
        let work=count.checked_mul(count).and_then(|n|self.doc.nodes.len().checked_mul(2).and_then(|m|n.checked_add(m))).ok_or("Form validation work overflow.")?;
        let work=work.checked_add(self.forms.validation_metadata_work(self.doc)?).ok_or("Validation metadata work overflow.")?;
        let work=work.checked_add(script_control_custom_validity::snapshot_work(self.state,count)?).ok_or("Custom validation work overflow.")?;
        self.state.work(work)?;script_text_content::deadline(self.state)?;
        self.state.rebuild(self.doc)?;
        let owner=self.node(object)?;let mut root=owner;let mut walked=0usize;
        while let Some(parent)=self.state.tree.parents[root]{
            walked=walked.checked_add(1).filter(|&n|n<=self.doc.nodes.len()).ok_or("Invalid form validation ancestry.")?;
            self.state.work(1)?;root=parent;
        }
        let available=self.state.retained_limit.saturating_sub(self.state.retained);
        let snapshot=self.forms.required_snapshot_with_budget(self.doc,owner,root,available,&mut |node|self.state.output_custom_errors.get(&node_handle(node)).is_some(),&mut self.state.host_work,&mut self.state.copy_work)?;
        self.publish_validation_snapshot(object,report,snapshot,available)
    }
    pub(super) fn publish_validation_snapshot(&mut self,object:u32,report:bool,snapshot:forms::ValidationSnapshot,available:usize)->Result<usize,String>{
        let storage=snapshot.storage_bytes();self.state.copies(snapshot.peak_bytes)?;
        let before=self.state.form_validations.capacity();let slot=std::mem::size_of::<Pending>();
        let (replacement,growth)=if self.state.form_validations.len()==before{
            let available=available.checked_sub(storage).and_then(|n|n.checked_sub(before.checked_mul(slot)?)).ok_or("Form validation frame storage allowance exhausted.")?;
            let growth=before.max(4).min(available/slot);
            if growth==0{return Err("Form validation frame storage allowance exhausted.".into());}
            let capacity=before.checked_add(growth).ok_or("Form validation frame capacity overflow.")?;
            self.state.copies(self.state.form_validations.len().checked_mul(slot).ok_or("Form validation frame copy overflow.")?)?;
            retained_storage::capacity(&self.state.form_validations,capacity,available)?
        }else{(None,0)};
        script_text_content::deadline(self.state)?;
        let extra=storage.checked_add(growth).ok_or("Form validation retained storage overflow.")?;
        self.state.replace_storage(0,extra,0)?;
        if let Some(mut replacement)=replacement{replacement.append(&mut self.state.form_validations);self.state.form_validations=replacement;}
        let token=self.state.form_validations.len();self.state.form_validations.push(Pending{form:object,snapshot,next:0,current:None,unhandled:None,report});Ok(token)
    }
    fn validation_index(&self,token:usize)->Result<usize,String>{
        if token.checked_add(1)!=Some(self.state.form_validations.len()){return Err("Form validation invocation is unavailable.".into());}Ok(token)
    }
    pub(super) fn next_native_form_validation_event(&mut self,token:usize)->Result<Option<usize>,String>{
        let index=self.validation_index(token)?;let pending=&self.state.form_validations[index];
        if pending.current.is_some(){return Err("The previous invalid event is incomplete.".into());}
        let object=pending.form;let Some(&node)=pending.snapshot.nodes.get(pending.next)else{return Ok(None);};
        self.validation_dispatch_authority(object).map_err(|(_,e)|e)?;
        self.native_node_access(node_handle(node)).map_err(|(_,e)|e)?;
        self.state.work(1)?;script_text_content::deadline(self.state)?;
        let fields=[("cancelable",Value::Bool(true))];
        let Value::Host(event)=self.construct_base_event(WINDOW,&[Value::String("invalid".into())],Some(&fields))?else{unreachable!()};
        let event_token=match self.begin_synthetic_dispatch(node_handle(node),&[Value::Host(event)]){
            Ok(token)=>token,Err(error)=>{self.rollback_base_event(event)?;return Err(error);}
        };
        let event_index=self.created_event_index(event).unwrap();self.state.created_events[event_index].trusted=true;
        let pending=&mut self.state.form_validations[index];pending.next+=1;pending.current=Some(node);Ok(Some(event_token))
    }
    pub(super) fn native_form_validation_event_result(&mut self,token:usize,accepted:bool)->Result<(),String>{
        let index=self.validation_index(token)?;let pending=&mut self.state.form_validations[index];
        let node=pending.current.take().ok_or("No invalid event result is pending.")?;
        if accepted&&pending.unhandled.is_none(){pending.unhandled=Some(node);}Ok(())
    }
    pub(super) fn complete_native_form_validation(&mut self,token:usize)->Result<bool,String>{
        let index=self.validation_index(token)?;let pending=&self.state.form_validations[index];
        if pending.current.is_some()||pending.next!=pending.snapshot.nodes.len(){return Err("Form validation is incomplete.".into());}
        let form=pending.form;let report=pending.report;let unhandled=pending.unhandled;let valid=pending.snapshot.nodes.is_empty();
        self.validation_dispatch_authority(form).map_err(|(_,e)|e)?;script_text_content::deadline(self.state)?;
        if report{self.state.validation_report=Some(unhandled);if let Some(node)=unhandled{
            if self.state.dom.connected.get(node).copied().unwrap_or(false)&&self.forms.controls.get(&node).is_some_and(|c|c.focusable()){self.state.dom.pending_focus=Some(node);}
        }}
        Ok(valid)
    }
    pub(super) fn finish_native_form_validation(&mut self,token:usize){
        if token.checked_add(1)==Some(self.state.form_validations.len()){
            let pending=self.state.form_validations.pop().unwrap();self.state.retained-=pending.snapshot.storage_bytes();
        }
    }
}
