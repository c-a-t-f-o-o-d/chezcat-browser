//! Synchronous reset holds native replacements across ordered OUTPUT steps.
//! https://html.spec.whatwg.org/multipage/forms.html#dom-form-reset
use super::*;
pub(super) struct PendingReset{
    owner:u32,
    plan:forms::ResetPlan,
    outputs:Vec<usize>,
    next:usize,
    storage:usize,
}
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
        if self.state.form_reset_pending.as_ref().is_some_and(|p|p.owner==object)
            ||self.state.synthetic_dispatches.iter().any(|d|d.reset_form==Some(form)){return Ok(None);}
        let fields=[("bubbles",Value::Bool(true)),("cancelable",Value::Bool(true))];
        let Value::Host(event)=self.construct_base_event(WINDOW,&[Value::String("reset".into())],Some(&fields))? else{unreachable!()};
        let token=match self.begin_synthetic_dispatch(object,&[Value::Host(event)]){
            Ok(token)=>token,Err(error)=>{self.rollback_base_event(event)?;return Err(error);}
        };
        self.state.synthetic_dispatches[token].reset_form=Some(form);
        let index=self.created_event_index(event).unwrap();self.state.created_events[index].trusted=true;
        Ok(Some(token))
    }
    pub(super) fn prepare_native_form_reset(&mut self,object:u32)->Result<(),String>{
        self.validate_form_reset(object).map_err(|(_,message)|message)?;
        if self.state.form_reset_pending.is_some(){return Err("A native reset replacement is already pending.".into());}
        self.state.rebuild(self.doc)?;
        let owner=self.node(object)?;
        let count=self.forms.controls.len();
        let work=count.checked_mul(count).and_then(|n|n.checked_add(self.doc.nodes.len()))
            .and_then(|n|n.checked_add(self.forms.default_scan_work())).ok_or("Form reset work overflow.")?;
        self.state.work(work)?;script_text_content::deadline(self.state)?;
        let available=self.state.retained_limit.saturating_sub(self.state.retained);
        let copies=self.forms.default_scan_work().checked_mul(4).ok_or("Form reset copy overflow.")?;
        self.state.copies(copies)?;
        let plan=forms::ResetPlan::prepare(self.forms,self.doc,owner,available)?;
        let storage=plan.storage_bytes()?;
        script_text_content::deadline(self.state)?;
        self.state.replace_storage(0,storage,0)?;
        // The plan remains charged while collection scratch and every OUTPUT
        // default copy/text replacement coexist with its unpublished backing.
        let result=(||{
            let outputs=self.form_reset_outputs(owner)?;
            let scratch=script_text_replacement::bytes(&[&outputs],0)?;
            let total=storage.checked_add(scratch).ok_or("Form reset pending storage overflow.")?;
            self.state.replace_storage(0,scratch,0)?;
            Ok((outputs,total))
        })();
        match result{
            Ok((outputs,storage))=>{self.state.form_reset_pending=Some(PendingReset{owner:object,plan,outputs,next:0,storage});Ok(())}
            Err(error)=>{self.state.retained-=storage;Err(error)}
        }
    }
    pub(super) fn next_native_form_reset_output(&mut self,object:u32)->Result<Option<u32>,String>{
        loop{
            self.state.work(1)?;script_text_content::deadline(self.state)?;
            let pending=self.state.form_reset_pending.as_mut().filter(|p|p.owner==object).ok_or("Native reset plan is unavailable.")?;
            let Some(&node)=pending.outputs.get(pending.next)else{return Ok(None)};
            pending.next+=1;
            let target=node_handle(node);
            // Share the checked owner recheck with native reset activation.
            if self.form_reset_output_owned(object,node)?{return Ok(Some(target));}
        }
    }
    pub(super) fn complete_native_form_reset(&mut self,object:u32)->Result<(),String>{
        self.validate_form_reset(object).map_err(|(_,message)|message)?;
        script_text_content::deadline(self.state)?;
        if !self.state.form_reset_pending.as_ref().is_some_and(|p|p.owner==object){return Err("Native reset plan is unavailable.".into());}
        let pending=self.state.form_reset_pending.take().unwrap();
        let bytes=pending.plan.bytes();pending.plan.commit(self.forms);
        self.state.retained-=pending.storage-bytes;
        self.state.layout_changed();Ok(())
    }
    pub(super) fn finish_native_form_reset(&mut self,object:u32){
        if self.state.form_reset_pending.as_ref().is_some_and(|p|p.owner==object){
            let pending=self.state.form_reset_pending.take().unwrap();self.state.retained-=pending.storage;
        }
    }
}
