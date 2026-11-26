//! Candidate metadata over real inert tree links, without validation or UI.
use super::*;
impl Bridge<'_>{
    fn eligibility_lookup(&mut self)->Result<(),String>{
        let work=self.state.inert_documents.adopted.lookup_work().checked_mul(16).and_then(|n|n.checked_add(1)).ok_or("Inert eligibility lookup work overflow.")?;
        self.state.work(work)
    }
    fn eligibility_attributes(&mut self,object:u32)->Result<(),String>{
        self.eligibility_lookup()?;
        let attrs=self.inert_element_attributes(object).ok_or("Inert candidate attributes are unavailable.")?;
        let work=attrs.iter().try_fold(1usize,|total,a|a.name.bytes().checked_add(a.value.logical_bytes()).and_then(|n|n.checked_add(1)).and_then(|n|n.checked_mul(8)).and_then(|n|total.checked_add(n)).ok_or("Inert candidate attribute work overflow."))?;
        self.state.work(work)
    }
    pub(in crate::scripting) fn inert_control_eligibility_get(&mut self,object:u32,class:&str)->Result<Value,String>{
        self.eligibility_attributes(object)?;
        let mut disabled=self.inert_element_attributes(object).unwrap().iter().any(|a|a.name.html_is("disabled"));
        let mut below=object;let mut ancestor=self.inert_observation_parent(object);
        let parent_select=ancestor.is_some_and(|p|self.inert_interface(p)==Some("HTMLSelectElement"));
        let mut in_datalist=false;
        for _ in 0..self.inert_node_count(){
            let Some(current)=ancestor else{
                script_text_content::deadline(self.state)?;
                let name=match class{"HTMLInputElement"=>"input","HTMLButtonElement"=>"button","HTMLSelectElement"=>"select","HTMLTextAreaElement"=>"textarea",_=>return Err("Inert candidate interface is unavailable.".into())};
                return Ok(Value::Bool(forms::validation_candidate_attributes(name,self.inert_element_attributes(object).unwrap(),disabled,in_datalist,parent_select)));
            };
            self.eligibility_lookup()?;
            if self.state.inert_documents.node(current).is_none(){return Err("Inert candidate ancestor is unavailable.".into());}
            let interface=self.inert_interface(current);
            in_datalist|=interface==Some("HTMLDataListElement");
            if interface==Some("HTMLFieldSetElement"){
                self.eligibility_attributes(current)?;
                if self.inert_element_attributes(current).unwrap().iter().any(|a|a.name.html_is("disabled")){
                    let count=self.inert_children(current).ok_or("Inert fieldset children are unavailable.")?.len();let mut first_legend=None;
                    for at in 0..count{
                        self.eligibility_lookup()?;
                        let child=self.inert_children(current).unwrap()[at];
                        if self.state.inert_documents.node(child).is_none(){return Err("Inert fieldset child is unavailable.".into());}
                        if self.inert_interface(child)==Some("HTMLLegendElement"){first_legend=Some(child);break;}
                    }
                    disabled|=first_legend!=Some(below);
                }
            }
            below=current;ancestor=self.inert_observation_parent(current);
        }
        Err("Cyclic inert candidate ancestry.".into())
    }
}
