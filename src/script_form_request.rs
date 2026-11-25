//! Synchronous native requestSubmit shares the current job and navigation queue.
//! https://html.spec.whatwg.org/multipage/forms.html#dom-form-requestsubmit
use super::*;
pub(super) struct Pending{form:u32,submitter:Option<usize>}
impl Page{
    pub(crate) fn set_native_submission(&mut self,owner:Option<usize>)->Option<usize>{std::mem::replace(&mut self.state.native_submission,owner)}
}
impl Bridge<'_>{
    pub(super) fn form_request_member(&self,object:u32,name:&str)->bool{
        name=="requestSubmit"&&self.form_length_receiver(object)&&!self.form_named_control(object,name)
    }
    fn request_authority(&self,object:u32)->Result<(),(&'static str,String)>{
        if !self.form_length_receiver(object){return Err(("TypeError","Submission requires a genuine HTML form.".into()));}
        self.native_node_access(object)?;
        if self.is_frame_handle(object)||self.inert_interface(object).is_some(){return Err(("NotSupportedError","This form requires its native submission backend and tree lifecycle hooks.".into()));}
        Ok(())
    }
    pub(super) fn validate_form_request(&self,object:u32,args:&[Value])->Result<Option<usize>,(&'static str,String)>{
        self.request_authority(object)?;
        let Some(target)=self.submit_event_argument(WINDOW,args.first())?else{return Ok(None)};
        let Some(node)=node_index(target)else{return Err(("NotSupportedError","This submitter requires its native form backend.".into()));};
        let Some(control)=self.forms.controls.get(&node).filter(|c|matches!(c.kind,Kind::Submit|Kind::Image))else{return Err(("TypeError","requestSubmit requires a submit button or null.".into()));};
        if control.owner!=node_index(object){return Err(("NotFoundError","The submit button belongs to a different form.".into()));}
        Ok(Some(node))
    }
    pub(super) fn begin_native_form_request(&mut self,object:u32,args:&[Value])->Result<Option<usize>,String>{
        let submitter=self.validate_form_request(object,args).map_err(|(_,e)|e)?;
        self.state.work(self.state.form_requests.len().saturating_add(1))?;script_text_content::deadline(self.state)?;
        self.state.rebuild(self.doc)?;let owner=self.node(object)?;
        if !self.state.dom.connected.get(owner).copied().unwrap_or(false)
            ||self.state.native_submission==Some(owner)||self.state.form_requests.iter().any(|p|p.form==object){return Ok(None);}
        let before=self.state.form_requests.capacity();let slot=std::mem::size_of::<Pending>();
        let (replacement,growth)=if before==self.state.form_requests.len(){
            let available=self.state.retained_limit.saturating_sub(self.state.retained).checked_sub(before.checked_mul(slot).ok_or("Submission storage overflow.")?).ok_or("Submission frame storage allowance exhausted.")?;
            let growth=before.max(4).min(available/slot);if growth==0{return Err("Submission frame storage allowance exhausted.".into());}
            let capacity=before.checked_add(growth).ok_or("Submission frame capacity overflow.")?;
            self.state.copies(self.state.form_requests.len().checked_mul(slot).ok_or("Submission frame copy overflow.")?)?;
            retained_storage::capacity(&self.state.form_requests,capacity,available)?
        }else{(None,0)};
        script_text_content::deadline(self.state)?;self.state.replace_storage(0,growth,0)?;
        if let Some(mut replacement)=replacement{replacement.append(&mut self.state.form_requests);self.state.form_requests=replacement;}
        let token=self.state.form_requests.len();self.state.form_requests.push(Pending{form:object,submitter});Ok(Some(token))
    }
    fn request_index(&self,token:usize)->Result<usize,String>{
        if token.checked_add(1)!=Some(self.state.form_requests.len()){return Err("Submission invocation is unavailable.".into());}Ok(token)
    }
    pub(super) fn native_form_request_validation(&mut self,token:usize)->Result<Option<usize>,(&'static str,String)>{
        let index=self.request_index(token).map_err(|e|("Error",e))?;
        let p=&self.state.form_requests[index];let form=p.form;let submitter=p.submitter;let owner=node_index(form).unwrap();
        self.request_authority(form)?;
        if self.doc.attr(owner,"novalidate").is_some()||submitter.is_some_and(|node|self.doc.attr(node,"formnovalidate").is_some()){
            // A successful interactive validation clears only an old required
            // report. A no-validate submission does not run that algorithm.
            return Ok(None);
        }
        self.validate_form_validation(form)?;
        self.begin_native_form_validation(form,true).map(Some).map_err(|e|("Error",e))
    }
    pub(super) fn native_form_request_event(&mut self,token:usize)->Result<usize,String>{
        let p=&self.state.form_requests[self.request_index(token)?];let form=p.form;let submitter=p.submitter;
        self.request_authority(form).map_err(|(_,e)|e)?;script_text_content::deadline(self.state)?;
        let fields=[("bubbles",Value::Bool(true)),("cancelable",Value::Bool(true)),("submitter",submitter.map_or(Value::Null,|n|Value::Host(node_handle(n))))];
        let Value::Host(event)=self.construct_submit_event(WINDOW,&[Value::String("submit".into())],Some(&fields))?else{unreachable!()};
        let dispatch=match self.begin_synthetic_dispatch(form,&[Value::Host(event)]){Ok(token)=>token,Err(error)=>{self.rollback_base_event(event)?;return Err(error);}};
        let index=self.created_event_index(event).unwrap();self.state.created_events[index].trusted=true;Ok(dispatch)
    }
    pub(super) fn complete_native_form_request(&mut self,token:usize)->Result<(),String>{
        let p=&self.state.form_requests[self.request_index(token)?];let form=p.form;let submitter=p.submitter;
        self.request_authority(form).map_err(|(_,e)|e)?;script_text_content::deadline(self.state)?;
        self.state.rebuild(self.doc)?;
        self.queue_form_submission(node_index(form).unwrap(),submitter,true)?;Ok(())
    }
    pub(super) fn finish_native_form_request(&mut self,token:usize){
        if token.checked_add(1)==Some(self.state.form_requests.len()){self.state.form_requests.pop();}
    }
}
