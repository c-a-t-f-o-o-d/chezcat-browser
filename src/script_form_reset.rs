//! FORM.reset synchronously dispatches before preparing current native controls.
//! https://html.spec.whatwg.org/multipage/forms.html#dom-form-reset
use super::*;
impl Bridge<'_>{
    pub(super) fn validate_form_reset(&self,object:u32)->Result<(),(&'static str,String)>{
        if !self.form_length_receiver(object){return Err(("TypeError","Reset requires a genuine HTML form.".into()));}
        self.native_node_access(object)?;
        if self.is_frame_handle(object)||self.inert_interface(object).is_some(){
            return Err(("NotSupportedError","This form requires its native reset backend and tree lifecycle integration.".into()));
        }
        Ok(())
    }
    pub(super) fn begin_native_form_reset(&mut self,object:u32)->Result<Option<usize>,String>{
        self.validate_form_reset(object).map_err(|(_,message)|message)?;
        let form=self.node(object)?;
        self.state.work(self.state.event_lookup_work(5))?;
        self.state.work(self.state.synthetic_dispatches.len())?;
        script_text_content::deadline(self.state)?;
        if self.state.synthetic_dispatches.iter().any(|d|d.reset_form==Some(form)){return Ok(None);}
        let fields=[("bubbles",Value::Bool(true)),("cancelable",Value::Bool(true))];
        let Value::Host(event)=self.construct_base_event(WINDOW,&[Value::String("reset".into())],Some(&fields))? else{unreachable!()};
        let token=match self.begin_synthetic_dispatch(object,&[Value::Host(event)]){
            Ok(token)=>token,Err(error)=>{self.rollback_base_event(event)?;return Err(error);}
        };
        self.state.synthetic_dispatches[token].reset_form=Some(form);
        let index=self.created_event_index(event).unwrap();self.state.created_events[index].trusted=true;
        Ok(Some(token))
    }
    pub(super) fn complete_native_form_reset(&mut self,object:u32)->Result<(),String>{
        self.validate_form_reset(object).map_err(|(_,message)|message)?;
        self.state.rebuild(self.doc)?;
        let owner=self.node(object)?;
        let count=self.forms.controls.len();
        let work=count.checked_mul(count).and_then(|n|n.checked_add(self.doc.nodes.len()))
            .and_then(|n|n.checked_add(self.forms.default_scan_work())).ok_or("Form reset work overflow.")?;
        self.state.work(work)?;script_text_content::deadline(self.state)?;
        // OUTPUT uses child-list replacement and observation transactions. Its
        // joint synchronous reset commit is separate from the native editor
        // transaction; refuse it explicitly rather than leaving a stale value.
        for node in 0..self.doc.nodes.len(){
            let target=node_handle(node);
            if self.html_output_receiver(target)&&matches!(self.control_form_owner_get(target)?,Value::Host(form)if form==object){
                return Err("Owned OUTPUT requires synchronous joint reset observation integration; no native controls were reset.".into());
            }
        }
        let available=self.state.retained_limit.saturating_sub(self.state.retained);
        let copies=self.forms.default_scan_work().checked_mul(4).ok_or("Form reset copy overflow.")?;
        self.state.copies(copies)?;
        let plan=forms::ResetPlan::prepare(self.forms,self.doc,owner,available)?;
        script_text_content::deadline(self.state)?;
        self.state.replace_bytes(0,plan.bytes())?;
        plan.commit(self.forms);
        self.state.layout_changed();
        Ok(())
    }
}
